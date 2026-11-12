//! A chapter's kept pages while its download fetches them, shared with a reader that opens the
//! chapter meanwhile.
//!
//! Each page is fetched by whichever of the two asks for it first. The other waits for it and
//! takes it from the kept pages, told by [`SharedPages::changed`] as each lands, so the source is
//! asked for each page once. A fetch runs on its own: it lands and is kept for the other one even
//! when the side that asked stops waiting for it (a reader closed, or past its wait), as long as
//! the download still runs to want the page. With no download, a fetch nobody waits on any more
//! is cut off, a fetch a later one of its page replaced included. One that fails is fetched once
//! more by the side that waited on it, unless a newer fetch of the page is on its way, which it
//! waits on instead. A reader past its wait asks again for itself, as it would without a
//! download. Both fetch from the chapter as the download
//! opened it, with the page addresses the last fetch had, so the source is not asked for the
//! chapter again either.
//!
//! There is one share per pages folder: a download that starts again while a reader still holds
//! the pages takes up the same share. A share let go deletes its folder under the same lock a
//! download opening the folder takes, so the two never overlap.

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

/// A fetch of a page on its way.
struct Fetching {
    ended: Fetch,
    /// The sides waiting on it now.
    waiters: usize,
    task: tokio::task::AbortHandle,
}

pub(crate) struct SharedPages {
    /// How many pages the chapter has.
    total: u32,
    state: Mutex<Sharing>,
    /// Told when a page lands or a fetch of one ends.
    changed: tokio::sync::Notify,
    /// Every folder's share, locked while this one's folder is deleted.
    shares: Arc<Mutex<Vec<Share>>>,
}

struct Sharing {
    kept: KeptPages,
    /// The chapter as the download opened it, with the page addresses the last fetch had.
    opened: Opened,
    /// The fetches on their way, by page, the latest last: one a later fetch of its page
    /// replaced is still counted and cut off as any other.
    fetching: HashMap<u32, Vec<Fetching>>,
    /// Pages that came but could not be kept, and why.
    unkept: HashMap<u32, ReadingFailure>,
    /// Downloads running on these pages now: one, or none once it ended.
    downloads: usize,
    /// The download is done with the pages: they go once nothing holds them.
    let_go: bool,
    /// The folder went to a fetch of the chapter in another number of pages: nothing more is
    /// kept here and the folder is left as it is.
    left: bool,
    /// How many fetches were started, for tests to count.
    #[cfg(test)]
    started: usize,
}

impl Sharing {
    /// With no download to keep them for, the fetches nobody waits on are cut off.
    fn cut_unwanted(&mut self) {
        if self.downloads > 0 && !self.left {
            return;
        }
        self.fetching.retain(|_, fetches| {
            fetches.retain(|f| {
                let wanted = f.waiters > 0;
                if !wanted {
                    f.task.abort();
                }
                wanted
            });
            !fetches.is_empty()
        });
    }

    /// The fetch of page `index` whose end is `ended`, while it is on its way.
    fn fetch_of(&mut self, index: u32, ended: &Fetch) -> Option<&mut Fetching> {
        self.fetching.get_mut(&index)?.iter_mut().find(|f| f.ended.same_channel(ended))
    }
}

/// A side waiting on a fetch: while one waits, the fetch goes on.
struct Waiting<'a> {
    pages: &'a SharedPages,
    index: u32,
    ended: Fetch,
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        let mut state = self.pages.state();
        if let Some(f) = state.fetch_of(self.index, &self.ended) {
            f.waiters = f.waiters.saturating_sub(1);
        }
        state.cut_unwanted();
    }
}

impl std::fmt::Debug for SharedPages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedPages").field("dir", &self.state().kept.dir()).finish()
    }
}

impl SharedPages {
    fn state(&self) -> MutexGuard<'_, Sharing> {
        lock(&self.state)
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

    /// How many fetches were started.
    #[cfg(test)]
    pub(crate) fn started(&self) -> usize {
        self.state().started
    }

    /// Pages are written with `write` from now on, as a disk would.
    #[cfg(test)]
    pub(crate) fn write_with(&self, write: super::kept::Write) {
        self.state().kept.write_with(write);
    }

    /// Page `index`: as kept, or once a fetch of it on its way lands, or fetched and kept. With
    /// `again`, a fetch on its way is not waited on: the page is asked for once more. A page
    /// another fetch failed to bring is asked for once more too, unless a newer fetch of it is on
    /// its way, which is waited on instead.
    pub(crate) async fn page(
        self: &Arc<Self>,
        sources: &BookSources,
        index: u32,
        now: i64,
        again: bool,
    ) -> Result<Vec<u8>, ReadingFailure> {
        let mut mine: Option<Fetch> = None;
        // Another's fetch that failed, not to be waited on again.
        let mut failed: Option<Fetch> = None;
        let mut waiting: Option<Waiting<'_>> = None;
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let (path, fetch, joined) = {
                let mut state = self.state();
                match state.kept.path(index).map(Path::to_path_buf) {
                    Some(path) => (Some(path), None, None),
                    None => {
                        let latest = state.fetching.get(&index).and_then(|f| f.last());
                        let fetch = match (&mine, latest) {
                            (Some(fetch), _) => fetch.clone(),
                            (None, Some(f))
                                if !again
                                    && !failed
                                        .as_ref()
                                        .is_some_and(|x| x.same_channel(&f.ended)) =>
                            {
                                f.ended.clone()
                            }
                            _ => {
                                let fetch = self.start(&mut state, sources, index, now);
                                mine = Some(fetch.clone());
                                fetch
                            }
                        };
                        let held =
                            waiting.as_ref().is_some_and(|w| w.ended.same_channel(&fetch));
                        if !held && let Some(f) = state.fetch_of(index, &fetch) {
                            f.waiters += 1;
                        }
                        (None, Some(fetch.clone()), (!held).then_some(fetch))
                    }
                }
            };
            // Out of the lock: the wait it replaces lets go of its fetch.
            if let Some(ended) = joined {
                waiting = Some(Waiting { pages: self, index, ended });
            }
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
            let mut let_down = false;
            tokio::select! {
                // How the fetch waited on ended comes first, so a failure is always seen as one.
                biased;
                ended = fetch.wait_for(Option::is_some) => {
                    let ended = ended.ok().and_then(|e| e.clone());
                    match ended {
                        Some(Ok(bytes)) => return Ok(bytes),
                        Some(Err(failure)) if mine.is_some() => return Err(failure),
                        None if mine.is_some() => {
                            let reason = format!("page {index} was not fetched");
                            return Err(ReadingFailure::Other { reason });
                        }
                        _ => let_down = true,
                    }
                }
                // Another fetch may have brought it.
                () = &mut changed => {}
            }
            // Another's fetch failed: a newer one on its way is waited on, or this side asks
            // once more itself.
            if let_down {
                failed = Some(fetch);
            }
        }
    }

    /// Fetch page `index` on its own: it lands and is kept while anyone waits for it or a
    /// download runs to want it.
    fn start(
        self: &Arc<Self>,
        state: &mut Sharing,
        sources: &BookSources,
        index: u32,
        now: i64,
    ) -> Fetch {
        let (tx, ended) = watch::channel(None);
        let mut opened = state.opened.clone();
        let (pages, sources, this) = (Arc::clone(self), sources.clone(), ended.clone());
        let task = tokio::spawn(async move {
            let before = opened.clone();
            let came = sources.page(&mut opened, index as usize, now).await;
            pages.landed(index, &this, &came, (&before, opened));
            let _ = tx.send(Some(came));
            pages.changed.notify_waiters();
        });
        let fetching = Fetching { ended: ended.clone(), waiters: 0, task: task.abort_handle() };
        state.fetching.entry(index).or_default().push(fetching);
        #[cfg(test)]
        {
            state.started += 1;
        }
        ended
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
        if let Some(fetches) = state.fetching.get_mut(&index) {
            fetches.retain(|f| !f.ended.same_channel(this));
            if fetches.is_empty() {
                state.fetching.remove(&index);
            }
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
        let state = self.state.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.let_go || state.left {
            return;
        }
        let dir = state.kept.dir().to_path_buf();
        #[cfg(test)]
        before_removing(&dir);
        // Under the list, so a download opening the folder meanwhile either waits until it has
        // gone or keeps it. The list is read without taking up any share, so no share is
        // dropped under its lock.
        let list = lock(&self.shares);
        if !list.iter().any(|(d, p)| *d == dir && p.strong_count() > 0) {
            KeptPages::remove(&dir);
        }
    }
}

/// Run before a share let go takes the list to delete its folder: a test stops it there.
#[cfg(test)]
pub(crate) type BeforeRemoving = Arc<dyn Fn() + Send + Sync>;

#[cfg(test)]
pub(crate) static BEFORE_REMOVING: Mutex<Option<(PathBuf, BeforeRemoving)>> = Mutex::new(None);

#[cfg(test)]
fn before_removing(dir: &Path) {
    let hook =
        lock(&BEFORE_REMOVING).as_ref().filter(|(d, _)| d == dir).map(|(_, f)| f.clone());
    if let Some(hook) = hook {
        hook();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

type Share = (PathBuf, Weak<SharedPages>);

/// The pages each chapter's folder shares now, while anything holds them.
#[derive(Default)]
pub(crate) struct Shares(Arc<Mutex<Vec<Share>>>);

fn find(list: &[Share], dir: &Path) -> Option<Arc<SharedPages>> {
    list.iter().find(|(d, _)| d == dir).and_then(|(_, p)| p.upgrade())
}

/// The pages as a download holds them: once it ends, however it ends, the fetches nobody waits
/// on are cut off.
pub(crate) struct Downloading(Arc<SharedPages>);

impl std::ops::Deref for Downloading {
    type Target = Arc<SharedPages>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for Downloading {
    fn drop(&mut self) {
        let mut state = self.0.state();
        state.downloads = state.downloads.saturating_sub(1);
        state.cut_unwanted();
    }
}

impl Shares {
    /// The pages kept in `dir`, while anything holds them.
    pub(crate) fn of(&self, dir: &Path) -> Option<Arc<SharedPages>> {
        find(&lock(&self.0), dir)
    }

    /// The pages of a chapter of `total` pages kept in `dir`, for its download: the share a
    /// reader still holds when there is one, or the folder's pages opened.
    pub(crate) fn open(
        &self,
        dir: &Path,
        total: u32,
        opened: Opened,
    ) -> std::io::Result<Downloading> {
        let mut list = lock(&self.0);
        list.retain(|(_, p)| p.strong_count() > 0);
        if let Some(live) = find(&list, dir) {
            let mut state = live.state();
            if live.total == total {
                state.let_go = false;
                state.opened = opened;
                state.downloads += 1;
                drop(state);
                return Ok(Downloading(live));
            }
            // Marked left before it can drop here, so it takes neither the list nor the folder.
            state.left = true;
            state.cut_unwanted();
            drop(state);
            list.retain(|(d, _)| d != dir);
        }
        let kept = KeptPages::open(dir, total)?;
        if kept.len() > 0 {
            tracing::info!(kept = kept.len(), total, "a chapter fetch picks up its pages");
        }
        let state = Sharing {
            kept,
            opened,
            fetching: HashMap::new(),
            unkept: HashMap::new(),
            downloads: 1,
            let_go: false,
            left: false,
            #[cfg(test)]
            started: 0,
        };
        let pages = Arc::new(SharedPages {
            total,
            state: Mutex::new(state),
            changed: tokio::sync::Notify::new(),
            shares: Arc::clone(&self.0),
        });
        list.push((dir.to_path_buf(), Arc::downgrade(&pages)));
        Ok(Downloading(pages))
    }
}
