//! A source's chapter of pictures, fetched a page at a time into a folder while it is read.
//!
//! The page the chapter opens at is fetched first and the chapter is laid out as soon as it
//! lands; the rest follow one after another, from that page on and then the ones before it,
//! each told to the page worker as it lands. A page that fails is that page's failure, with
//! its own retry: the others still come. The fetching stops when nothing holds the feed.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

use kaeri_core::books::{ChapterContent, ChapterId};
use kaeri_model::reading::ReadingFailure;
use tokio::sync::mpsc;

use crate::{
    outbox::Outbox,
    read_sources::{BookSources, Opened},
};

/// Says once a reader session is closed, for the feeds filling its folder: a page that lands
/// after takes the folder with it, so nothing is left behind.
#[derive(Debug, Clone)]
pub struct Gone {
    flag: Arc<AtomicBool>,
    /// The session's folder of fetched pages.
    root: PathBuf,
}

impl Gone {
    pub fn new(root: PathBuf) -> Self {
        Self { flag: Arc::default(), root }
    }

    /// The session closed: its folder goes now, and anything landing later goes too.
    pub fn set(&self) {
        self.flag.store(true, Ordering::SeqCst);
        let _ = std::fs::remove_dir_all(&self.root);
    }

    pub fn is(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Take away what was written after the close; `true` when the session is closed.
    pub fn swept(&self) -> bool {
        let gone = self.is();
        if gone {
            let _ = std::fs::remove_dir_all(&self.root);
        }
        gone
    }
}

/// One page of a feed.
#[derive(Debug, Clone, PartialEq)]
pub enum FeedPage {
    Coming,
    /// In the feed's folder, at this path.
    Landed(PathBuf),
    Failed(ReadingFailure),
}

/// A chapter's pages as they land.
#[derive(Debug)]
pub struct Feed {
    pub chapter: ChapterId,
    /// The folder the pages land in, named in reading order.
    pub dir: PathBuf,
    pages: Mutex<Vec<FeedPage>>,
    /// Pages to fetch again, for the fetcher, with the way to answer once each lands.
    again: mpsc::UnboundedSender<(usize, Outbox)>,
}

/// What a feed's fetcher is lent: the source, its chapter, and who hears of each page. It holds
/// a way to answer only while it fetches, so an engine waiting on nothing can go idle.
struct Fetcher<F> {
    sources: BookSources,
    opened: Opened,
    now: i64,
    landed: F,
    again: mpsc::UnboundedReceiver<(usize, Outbox)>,
    gone: Gone,
}

impl Feed {
    /// Every page of the feed as it stands.
    pub fn pages(&self) -> Vec<FeedPage> {
        self.pages.lock().map(|p| p.clone()).unwrap_or_default()
    }

    pub fn page(&self, index: usize) -> Option<FeedPage> {
        self.pages.lock().ok()?.get(index).cloned()
    }

    /// Every page landed.
    pub fn complete(&self) -> bool {
        self.pages.lock().is_ok_and(|p| p.iter().all(|p| matches!(p, FeedPage::Landed(_))))
    }

    /// Fetch a failed page again, answering on `out` once it lands. `false` when it had not
    /// failed.
    pub fn retry(&self, index: usize, out: &Outbox) -> bool {
        let Ok(mut pages) = self.pages.lock() else { return false };
        match pages.get_mut(index) {
            Some(page @ FeedPage::Failed(_)) => {
                *page = FeedPage::Coming;
                drop(pages);
                self.again.send((index, out.clone())).is_ok()
            }
            _ => false,
        }
    }

    fn set(&self, index: usize, page: FeedPage) {
        if let Ok(mut pages) = self.pages.lock()
            && let Some(slot) = pages.get_mut(index)
        {
            *slot = page;
        }
    }
}

/// Start feeding `opened`'s pictures, the reader's `chapter`, into `dir`, from page `first` on. The first page is
/// fetched before this returns; the feed is `Err` when it fails, as the chapter failing to open
/// would be. The rest come behind it, `landed` called with each page's index, and `out` to
/// answer on, once it has landed or failed. Once `gone` says the session closed, nothing more is
/// fetched and what landed goes.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start(
    sources: BookSources,
    mut opened: Opened,
    chapter: ChapterId,
    dir: PathBuf,
    first: usize,
    now: i64,
    out: Outbox,
    gone: Gone,
    landed: impl Fn(u32, &Outbox) + Send + 'static,
) -> Result<Arc<Feed>, ReadingFailure> {
    let ChapterContent::Images(list) = &opened.content else {
        return Err(ReadingFailure::NotFound);
    };
    let total = list.len();
    if total == 0 {
        return Err(ReadingFailure::NotFound);
    }
    if gone.is() {
        return Err(ReadingFailure::NotFound);
    }
    let first = first.min(total - 1);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| ReadingFailure::Other { reason: e.to_string() })?;
    let page = fetch(&sources, &mut opened, &dir, first, now, &gone).await;
    if let FeedPage::Failed(failure) = page {
        return Err(failure);
    }
    let (again, asked) = mpsc::unbounded_channel();
    let mut pages = vec![FeedPage::Coming; total];
    pages[first] = page;
    let feed = Arc::new(Feed { chapter, dir, pages: Mutex::new(pages), again });
    let fetcher = Fetcher { sources, opened, now, landed, again: asked, gone };
    let order: Vec<usize> = (first + 1..total).chain(0..first).collect();
    tokio::spawn(fetcher.run(Arc::downgrade(&feed), order, out));
    Ok(feed)
}

impl<F: Fn(u32, &Outbox) + Send + 'static> Fetcher<F> {
    async fn run(mut self, feed: Weak<Feed>, order: Vec<usize>, out: Outbox) {
        for index in order {
            if !self.one(&feed, index, &out).await {
                return;
            }
        }
        drop(out);
        // Then the pages asked for again, until the feed goes.
        while let Some((index, out)) = self.again.recv().await {
            if !self.one(&feed, index, &out).await {
                return;
            }
        }
    }

    /// Fetch one page into the feed; `false` once nothing holds the feed.
    async fn one(&mut self, feed: &Weak<Feed>, index: usize, out: &Outbox) -> bool {
        let Some(dir) = feed.upgrade().map(|f| f.dir.clone()) else { return false };
        let page =
            fetch(&self.sources, &mut self.opened, &dir, index, self.now, &self.gone).await;
        let Some(feed) = feed.upgrade() else { return false };
        feed.set(index, page);
        (self.landed)(index as u32, out);
        true
    }
}

/// Fetch page `index` into `dir`, named in reading order with the type its bytes are.
async fn fetch(
    sources: &BookSources,
    opened: &mut Opened,
    dir: &Path,
    index: usize,
    now: i64,
    gone: &Gone,
) -> FeedPage {
    let bytes = match sources.page(opened, index, now).await {
        Ok(bytes) => bytes,
        Err(failure) => return FeedPage::Failed(failure),
    };
    let Some(ext) = page_extension(&bytes) else {
        return FeedPage::Failed(ReadingFailure::Unsupported { extension: "page".into() });
    };
    let path = dir.join(format!("{:05}.{ext}", index + 1));
    let temp = dir.join(format!(".{:05}.part", index + 1));
    let written = async {
        tokio::fs::write(&temp, &bytes).await?;
        tokio::fs::rename(&temp, &path).await
    };
    let written = written.await;
    // Closed while the page came: it goes with the folder.
    if gone.swept() {
        return FeedPage::Failed(ReadingFailure::NotFound);
    }
    match written {
        Ok(()) => FeedPage::Landed(path),
        Err(e) => FeedPage::Failed(ReadingFailure::Other { reason: e.to_string() }),
    }
}

/// The extension a page's bytes are of, by their first bytes, for the page worker.
pub(crate) fn page_extension(bytes: &[u8]) -> Option<&'static str> {
    let ext = kaeri_books::mime::sniff_image(bytes)?.strip_prefix("image/")?;
    Some(if ext == "jpeg" { "jpg" } else { ext })
}
