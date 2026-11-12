//! A chapter's kept pages while its download fetches them, shared with a reader that opens the
//! chapter meanwhile.
//!
//! Each page is fetched by whichever of the two asks for it first. The other waits for it and
//! takes it from the kept pages, told by [`SharedPages::changed`] as each lands, so the source is
//! asked for each page once. A page whose fetch fails or stops is fetched by the next one that
//! asks for it. The reader fetches from the chapter as the download opened it, so the source is
//! not asked for the chapter again either.

use std::{
    collections::HashSet,
    future::Future,
    path::Path,
    sync::{Arc, Mutex, MutexGuard, Weak},
};

use kaeri_model::reading::ReadingFailure;

use super::KeptPages;
use crate::read_sources::Opened;

pub(crate) struct SharedPages {
    /// The chapter as the download opened it.
    pub opened: Opened,
    state: Mutex<Sharing>,
    /// Told when a page lands or a fetch of one ends.
    changed: tokio::sync::Notify,
}

struct Sharing {
    kept: KeptPages,
    /// The pages being fetched now.
    fetching: HashSet<u32>,
    /// The download is done with the pages: they go once nothing holds them.
    let_go: bool,
}

impl std::fmt::Debug for SharedPages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedPages").field("chapter", &self.opened.chapter.id).finish()
    }
}

/// A page being fetched: when the fetch ends or stops, the page is free for the next to ask.
struct Claim<'a> {
    pages: &'a SharedPages,
    index: u32,
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        self.pages.state().fetching.remove(&self.index);
        self.pages.changed.notify_waiters();
    }
}

impl SharedPages {
    pub(crate) fn new(kept: KeptPages, opened: Opened) -> Self {
        let state = Sharing { kept, fetching: HashSet::new(), let_go: false };
        Self { opened, state: Mutex::new(state), changed: tokio::sync::Notify::new() }
    }

    fn state(&self) -> MutexGuard<'_, Sharing> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn has(&self, index: u32) -> bool {
        self.state().kept.has(index)
    }

    pub(crate) fn read(&self, index: u32) -> std::io::Result<Vec<u8>> {
        self.state().kept.read(index)
    }

    /// The download is done with the pages, finished or cancelled: they go once nothing holds
    /// them, and a reader still fetching keeps them until then.
    pub(crate) fn let_go(&self) {
        self.state().let_go = true;
    }

    /// Page `index`: as kept, or once another fetch of it lands, or fetched with `fetch` and
    /// kept. A page that could not be kept is still given.
    pub(crate) async fn page<F>(&self, index: u32, fetch: F) -> Result<Vec<u8>, ReadingFailure>
    where
        F: Future<Output = Result<Vec<u8>, ReadingFailure>>,
    {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let path = {
                let mut state = self.state();
                match state.kept.path(index).map(Path::to_path_buf) {
                    Some(path) => Some(path),
                    None if state.fetching.insert(index) => break,
                    None => None,
                }
            };
            if let Some(path) = path {
                match std::fs::read(&path) {
                    Ok(bytes) => return Ok(bytes),
                    // Gone from under it: fetched again once nothing else fetches it.
                    Err(_) if self.state().fetching.insert(index) => break,
                    Err(_) => {}
                }
            }
            changed.await;
        }
        let claim = Claim { pages: self, index };
        let bytes = fetch.await?;
        if let Err(e) = self.state().kept.put(index, &bytes) {
            tracing::warn!(error = %e, "a shared page was not kept");
        }
        drop(claim);
        Ok(bytes)
    }
}

impl Drop for SharedPages {
    fn drop(&mut self) {
        let state = self.state();
        if state.let_go {
            KeptPages::remove(state.kept.dir());
        }
    }
}

/// A chapter's pages shared by its download, by work and chapter.
type Share = ((String, String), Weak<SharedPages>);

/// The chapters whose pages a download shares now.
#[derive(Default)]
pub(crate) struct Shares(Mutex<Vec<Share>>);

impl Shares {
    fn list(&self) -> MutexGuard<'_, Vec<Share>> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The pages a download of `chapter` shares, while it fetches them.
    pub(crate) fn of(&self, work: &str, chapter: &str) -> Option<Arc<SharedPages>> {
        let list = self.list();
        let found = list.iter().find(|((w, c), _)| w == work && c == chapter)?;
        found.1.upgrade()
    }

    /// Share `pages` until the guard given back goes.
    pub(crate) fn share<'a>(
        &'a self,
        (work, chapter): (&str, &str),
        pages: &Arc<SharedPages>,
    ) -> Shared<'a> {
        let key = (work.to_owned(), chapter.to_owned());
        let mut list = self.list();
        list.retain(|(k, p)| *k != key && p.strong_count() > 0);
        list.push((key, Arc::downgrade(pages)));
        Shared { shares: self, pages: Arc::downgrade(pages) }
    }
}

/// A download's pages shared while it lives.
pub(crate) struct Shared<'a> {
    shares: &'a Shares,
    pages: Weak<SharedPages>,
}

impl Drop for Shared<'_> {
    fn drop(&mut self) {
        self.shares.list().retain(|(_, p)| !p.ptr_eq(&self.pages) && p.strong_count() > 0);
    }
}
