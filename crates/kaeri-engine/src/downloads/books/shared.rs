//! A chapter's kept pages while its download fetches them, shared with a reader that opens the
//! chapter meanwhile.
//!
//! Each page is fetched by whichever of the two asks for it first. The other waits for it and
//! takes it from the kept pages, told by [`SharedPages::changed`] as each lands, so the source is
//! asked for each page once. A fetch runs on its own: it lands and is kept for the other one even
//! when the side that asked stops waiting for it (a reader closed, or past its wait). One that
//! fails is fetched once more by the side that waited on it. A reader past its wait asks again
//! for itself, as it would without a download. Both fetch from the chapter as the download
//! opened it, with the page addresses the last fetch had, so the source is not asked for the
//! chapter again either.
//!
//! There is one share per pages folder: a download that starts again while a reader still holds
//! the pages takes up the same share.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, Weak},
};

use kaeri_model::reading::ReadingFailure;
use tokio::sync::watch;

use super::{KeptPages, io_failure};
use crate::read_sources::{BookSources, Opened};

/// How a fetch of a page ended, once it has.
type Fetch = watch::Receiver<Option<Result<Vec<u8>, ReadingFailure>>>;

pub(crate) struct SharedPages {
    /// How many pages the chapter has.
    total: u32,
    state: Mutex<Sharing>,
    /// Told when a page lands or a fetch of one ends.
    changed: tokio::sync::Notify,
}

struct Sharing {
    kept: KeptPages,
    /// The chapter as the download opened it, with the page addresses the last fetch had.
    opened: Opened,
    /// The fetches on their way, by page: the latest one of each.
    fetching: HashMap<u32, Fetch>,
    /// Pages that came but could not be kept, and why.
    unkept: HashMap<u32, ReadingFailure>,
    /// The download is done with the pages: they go once nothing holds them.
    let_go: bool,
    /// The folder went to a fetch of the chapter in another number of pages: nothing more is
    /// kept here and the folder is left as it is.
    left: bool,
}

impl std::fmt::Debug for SharedPages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedPages").field("dir", &self.state().kept.dir()).finish()
    }
}

impl SharedPages {
    fn new(kept: KeptPages, opened: Opened, total: u32) -> Self {
        let state = Sharing {
            kept,
            opened,
            fetching: HashMap::new(),
            unkept: HashMap::new(),
            let_go: false,
            left: false,
        };
        Self { total, state: Mutex::new(state), changed: tokio::sync::Notify::new() }
    }

    fn state(&self) -> MutexGuard<'_, Sharing> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The chapter as the download opened it, with the page addresses the last fetch had.
    pub(crate) fn opened(&self) -> Opened {
        self.state().opened.clone()
    }

    pub(crate) fn has(&self, index: u32) -> bool {
        self.state().kept.has(index)
    }

    pub(crate) fn read(&self, index: u32) -> std::io::Result<Vec<u8>> {
        self.state().kept.read(index)
    }

    /// Why page `index` came but was not kept, when it was not.
    pub(crate) fn unkept(&self, index: u32) -> Option<ReadingFailure> {
        self.state().unkept.get(&index).cloned()
    }

    /// The download is done with the pages, finished or cancelled: they go once nothing holds
    /// them, and a reader still fetching keeps them until then.
    pub(crate) fn let_go(&self) {
        self.state().let_go = true;
    }

    /// Page `index`: as kept, or once a fetch of it on its way lands, or fetched and kept. With
    /// `again`, a fetch on its way is not waited on: the page is asked for once more. A page
    /// another fetch failed to bring is asked for once more too.
    pub(crate) async fn page(
        self: &Arc<Self>,
        sources: &BookSources,
        index: u32,
        now: i64,
        again: bool,
    ) -> Result<Vec<u8>, ReadingFailure> {
        let mut again = again;
        let mut mine: Option<Fetch> = None;
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let (path, fetch) = {
                let mut state = self.state();
                match state.kept.path(index).map(Path::to_path_buf) {
                    Some(path) => (Some(path), None),
                    None => {
                        let fetch = match (&mine, state.fetching.get(&index)) {
                            (Some(fetch), _) => fetch.clone(),
                            (None, Some(fetch)) if !again => fetch.clone(),
                            _ => {
                                let fetch = self.start(&mut state, sources, index, now);
                                mine = Some(fetch.clone());
                                fetch
                            }
                        };
                        (None, Some(fetch))
                    }
                }
            };
            if let Some(path) = path {
                match std::fs::read(&path) {
                    Ok(bytes) => return Ok(bytes),
                    // Gone from under it: fetched again.
                    Err(_) => {
                        self.state().kept.forget(index);
                        continue;
                    }
                }
            }
            let Some(mut fetch) = fetch else { continue };
            tokio::select! {
                // Another fetch may have brought it.
                () = &mut changed => {}
                ended = fetch.wait_for(Option::is_some) => {
                    let ended = ended.ok().and_then(|e| e.clone());
                    match ended {
                        Some(Ok(bytes)) => return Ok(bytes),
                        Some(Err(failure)) if mine.is_some() => return Err(failure),
                        None if mine.is_some() => {
                            let reason = format!("page {index} was not fetched");
                            return Err(ReadingFailure::Other { reason });
                        }
                        // Another's fetch failed: this one asks once more itself.
                        _ => again = true,
                    }
                }
            }
        }
    }

    /// Fetch page `index` on its own: it lands and is kept whether or not anyone still waits.
    fn start(
        self: &Arc<Self>,
        state: &mut Sharing,
        sources: &BookSources,
        index: u32,
        now: i64,
    ) -> Fetch {
        let (tx, fetch) = watch::channel(None);
        state.fetching.insert(index, fetch.clone());
        let mut opened = state.opened.clone();
        let (pages, sources, this) = (Arc::clone(self), sources.clone(), fetch.clone());
        tokio::spawn(async move {
            let before = opened.clone();
            let came = sources.page(&mut opened, index as usize, now).await;
            pages.landed(index, &this, &came, (&before, opened));
            let _ = tx.send(Some(came));
            pages.changed.notify_waiters();
        });
        fetch
    }

    /// A fetch of page `index` ended: the page is kept when it came, and fresh page addresses
    /// the fetch asked for serve the fetches after it.
    fn landed(
        &self,
        index: u32,
        this: &Fetch,
        came: &Result<Vec<u8>, ReadingFailure>,
        (before, after): (&Opened, Opened),
    ) {
        let mut state = self.state();
        if state.fetching.get(&index).is_some_and(|f| f.same_channel(this)) {
            state.fetching.remove(&index);
        }
        if crate::reading::feed::moved(&after, before) {
            state.opened = after;
        }
        let Ok(bytes) = came else { return };
        if state.left || state.kept.has(index) {
            return;
        }
        match state.kept.put(index, bytes) {
            Ok(()) => {
                state.unkept.remove(&index);
            }
            Err(e) => {
                tracing::warn!(error = %e, "a shared page was not kept");
                let failure = io_failure(&e, state.kept.dir());
                state.unkept.insert(index, failure);
            }
        }
    }
}

impl Drop for SharedPages {
    fn drop(&mut self) {
        let state = self.state();
        if state.let_go && !state.left {
            KeptPages::remove(state.kept.dir());
        }
    }
}

type Share = (PathBuf, Weak<SharedPages>);

/// The pages each chapter's folder shares now, while anything holds them.
#[derive(Default)]
pub(crate) struct Shares(Mutex<Vec<Share>>);

fn find(list: &[Share], dir: &Path) -> Option<Arc<SharedPages>> {
    list.iter().find(|(d, _)| d == dir).and_then(|(_, p)| p.upgrade())
}

impl Shares {
    fn list(&self) -> MutexGuard<'_, Vec<Share>> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The pages kept in `dir`, while anything holds them.
    pub(crate) fn of(&self, dir: &Path) -> Option<Arc<SharedPages>> {
        find(&self.list(), dir)
    }

    /// The pages of a chapter of `total` pages kept in `dir`, for its download: the share a
    /// reader still holds when there is one, or the folder's pages opened.
    pub(crate) fn open(
        &self,
        dir: &Path,
        total: u32,
        opened: Opened,
    ) -> std::io::Result<Arc<SharedPages>> {
        let mut list = self.list();
        list.retain(|(_, p)| p.strong_count() > 0);
        if let Some(live) = find(&list, dir) {
            let mut state = live.state();
            if live.total == total {
                state.let_go = false;
                state.opened = opened;
                drop(state);
                return Ok(live);
            }
            state.left = true;
            drop(state);
            list.retain(|(d, _)| d != dir);
        }
        let kept = KeptPages::open(dir, total)?;
        if kept.len() > 0 {
            tracing::info!(kept = kept.len(), total, "a chapter fetch picks up its pages");
        }
        let pages = Arc::new(SharedPages::new(kept, opened, total));
        list.push((dir.to_path_buf(), Arc::downgrade(&pages)));
        Ok(pages)
    }
}
