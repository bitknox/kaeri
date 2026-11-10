//! A source's chapter of pictures, fetched a page at a time into a folder while it is read.
//!
//! The pages at the chapter's edges come first, together: the first two and last two of each
//! part. Their fingerprints teach the series' credit pages, and the credit pages among them
//! leave the chapter before it is laid out, so its count and places match a downloaded copy's
//! and no page goes while it is read. An edge page that fails or is slow is waited on for
//! [`EDGE_WAIT`] at most; its end of the chapter keeps every page. Then the page the chapter
//! opens at, and the chapter is laid out as soon as it lands; the rest follow one after
//! another, from that page on and then the ones before it, each told to the page worker as it
//! lands. A page that fails is that page's failure, with its own retry: the others still come.
//! The fetching stops when nothing holds the feed.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use kaeri_core::books::{ChapterContent, ChapterId};
use kaeri_model::{reader::ChapterPart, reading::ReadingFailure};
use kaeri_store::book_credits::PagePrint;
use tokio::sync::mpsc;

use crate::{
    outbox::Outbox,
    read_sources::{BookSources, Opened},
    reading::credits::Learning,
};

/// The longest an edge page is waited on before the chapter is laid out without it, as long
/// as a novel's pictures are.
pub(crate) const EDGE_WAIT: Duration = Duration::from_secs(3);

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

/// A credit page left out of a feed, fetched with the edges.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedCredit {
    /// Where it was in the chapter, from 0, counting every page the source has.
    pub page: u32,
    pub at_end: bool,
    /// Its file, in a folder beside the feed's.
    pub path: PathBuf,
    pub print: u64,
}

/// A chapter in parts as the reader is told of it, credit pages left out.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedParts {
    pub parts: Vec<ChapterPart>,
    pub total: u32,
    pub missing: Vec<u32>,
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
    /// The pages as the reader counts them, credit pages left out.
    pages: Mutex<Vec<FeedPage>>,
    /// The credit pages left out, in order.
    pub credits: Vec<FeedCredit>,
    /// Its parts, for a chapter in parts.
    pub parts: Option<FeedParts>,
    /// Pages to fetch again, for the fetcher, with the way to answer once each lands.
    again: mpsc::UnboundedSender<(usize, Outbox)>,
}

/// What a feed's fetcher is lent: the source, its chapter, and who hears of each page. It holds
/// a way to answer only while it fetches, so an engine waiting on nothing can go idle.
struct Fetcher<F> {
    sources: BookSources,
    opened: Opened,
    /// For each page of the feed, its page in the source's.
    order: Vec<usize>,
    /// Edge pages still on their way, by their page in the source's.
    pending: HashMap<usize, Pending>,
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

/// Start feeding `opened`'s pictures, the reader's `chapter`, into `dir`. The edge pages are
/// fetched first and the credit pages among them left out, as `learning` says; then the page
/// `first` gives for the pages left, fetched before this returns. The feed is `Err` when that
/// page fails, as the chapter failing to open would be. The rest come behind it, `landed`
/// called with each page's index, and `out` to answer on, once it has landed or failed. Once
/// `gone` says the session closed, nothing more is fetched and what landed goes.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start(
    sources: BookSources,
    mut opened: Opened,
    (chapter, learning): (ChapterId, Learning),
    dir: PathBuf,
    first: impl FnOnce(u32) -> usize + Send,
    now: i64,
    out: Outbox,
    gone: Gone,
    landed: impl Fn(u32, &Outbox) + Send + 'static,
) -> Result<Arc<Feed>, ReadingFailure> {
    let ChapterContent::Images(list) = &opened.content else {
        return Err(ReadingFailure::NotFound);
    };
    let total = list.len();
    if total == 0 || gone.is() {
        return Err(ReadingFailure::NotFound);
    }
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| ReadingFailure::Other { reason: e.to_string() })?;
    let spans = opened.spans();
    let (edges, mut pending) = edge_pages(&sources, &mut opened, &spans, now).await;
    let mine: Vec<PagePrint> = edges
        .iter()
        .filter_map(|e| Some(PagePrint { page: e.page, at_end: e.at_end, print: e.print? }))
        .collect();
    let found = {
        let (learning, chapter, group) =
            (learning.clone(), chapter.as_str().to_owned(), opened.group.clone());
        let count = u32::try_from(total).unwrap_or(u32::MAX);
        tokio::task::spawn_blocking(move || {
            learning.keep_and_find((&chapter, group.as_deref()), &mine, count, now)
        })
        .await
        .unwrap_or_default()
    };
    // A credit page leaves only when every page at its end of the chapter, or of its part, came.
    let whole = |page: u32| {
        ends(&spans).iter().filter(|end| end.contains(&page)).all(|end| {
            end.iter().all(|p| edges.iter().any(|e| e.page == *p && e.bytes.is_some()))
        })
    };
    // Their files go beside the pages' folder, which the page worker may open as a book; a
    // credit page whose file could not be written stays in the chapter.
    let credits_dir = PathBuf::from(format!("{}-credits", dir.display()));
    let mut credits = Vec::new();
    for credit in found.into_iter().filter(|p| whole(p.page)) {
        let Some(bytes) =
            edges.iter().find(|e| e.page == credit.page).and_then(|e| e.bytes.as_ref())
        else {
            continue;
        };
        if tokio::fs::create_dir_all(&credits_dir).await.is_err() {
            break;
        }
        let name = format!("{:05}", credit.page + 1);
        if let FeedPage::Landed(path) = write(&credits_dir, &name, bytes).await {
            let (page, at_end, print) = (credit.page, credit.at_end, credit.print);
            credits.push(FeedCredit { page, at_end, path, print });
        }
    }
    let order: Vec<usize> =
        (0..total).filter(|i| !credits.iter().any(|c| c.page as usize == *i)).collect();
    let mut pages = vec![FeedPage::Coming; order.len()];
    for edge in edges {
        let Some(bytes) = edge.bytes else { continue };
        if let Some(shown) = order.iter().position(|i| *i == edge.page as usize) {
            pages[shown] = write(&dir, &format!("{:05}", shown + 1), &bytes).await;
        }
    }
    credits.sort_by_key(|c| c.page);
    let skipped: Vec<u32> = credits.iter().map(|c| c.page).collect();
    let parts = parts_of(&opened, &skipped);
    let count = order.len();
    let first = first(u32::try_from(count).unwrap_or(u32::MAX)).min(count - 1);
    if !matches!(pages[first], FeedPage::Landed(_)) {
        let slow = pending.remove(&order[first]);
        let page = fetch(&sources, &mut opened, &dir, (first, order[first]), slow, now).await;
        if let FeedPage::Failed(failure) = page {
            return Err(failure);
        }
        pages[first] = page;
    }
    // Closed while the first pages came: they go with the session's folder.
    if gone.swept() {
        return Err(ReadingFailure::NotFound);
    }
    let rest: Vec<usize> = (first + 1..count)
        .chain(0..first)
        .filter(|i| !matches!(pages[*i], FeedPage::Landed(_)))
        .collect();
    let (again, asked) = mpsc::unbounded_channel();
    let feed = Arc::new(Feed { chapter, dir, pages: Mutex::new(pages), credits, parts, again });
    let fetcher = Fetcher { sources, opened, order, pending, now, landed, again: asked, gone };
    tokio::spawn(fetcher.run(Arc::downgrade(&feed), rest, out));
    Ok(feed)
}

/// An edge page as it came, or did not in time.
struct Edge {
    page: u32,
    at_end: bool,
    bytes: Option<Vec<u8>>,
    print: Option<u64>,
}

/// An edge page's fetch that outlasted [`EDGE_WAIT`]: it goes on, and the feed takes the page
/// from it rather than asking the source twice. It stops when dropped.
struct Pending(tokio::task::JoinHandle<Result<Vec<u8>, ReadingFailure>>);

impl Drop for Pending {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Fetch a chapter's edge pages, waiting [`EDGE_WAIT`] at most, and fingerprint the ones that
/// came. The first page comes first, on the chapter's own addresses, so addresses that expired
/// are asked for again once; the rest then come together. Those still coming are given back to
/// be waited on in their turn.
async fn edge_pages(
    sources: &BookSources,
    opened: &mut Opened,
    spans: &[(u32, u32)],
    now: i64,
) -> (Vec<Edge>, HashMap<usize, Pending>) {
    let deadline = tokio::time::Instant::now() + EDGE_WAIT;
    let mut wanted = crate::reading::credits::edges(spans).into_iter();
    let lead = wanted.next();
    let lead = match lead {
        Some((page, at_end)) => {
            let got =
                tokio::time::timeout_at(deadline, sources.page(opened, page as usize, now))
                    .await;
            Some((page, at_end, got.ok().and_then(Result::ok)))
        }
        None => None,
    };
    let mut fetches: Vec<(u32, bool, Option<Pending>)> = wanted
        .map(|(page, at_end)| {
            let fetch = opened.pictures_copy().map(|mut own| {
                let sources = sources.clone();
                Pending(tokio::spawn(async move {
                    sources.page(&mut own, page as usize, now).await
                }))
            });
            (page, at_end, fetch)
        })
        .collect();
    let mut edges = Vec::new();
    let mut pending = HashMap::new();
    let lead = lead.map(|(page, at_end, bytes)| (page, at_end, None, bytes));
    let rest =
        fetches.iter_mut().map(|(page, at_end, fetch)| (*page, *at_end, Some(fetch), None));
    for (page, at_end, fetch, came) in lead.into_iter().chain(rest) {
        let mut bytes = came;
        if let Some(fetch) = fetch
            && let Some(Pending(handle)) = fetch.as_mut()
        {
            match tokio::time::timeout_at(deadline, handle).await {
                Ok(Ok(Ok(got))) => bytes = Some(got),
                // Failed: the page is fetched again in its turn.
                Ok(_) => {}
                Err(_) => {
                    if let Some(slow) = fetch.take() {
                        pending.insert(page as usize, slow);
                    }
                }
            }
        }
        let print = match bytes.clone() {
            Some(b) => tokio::task::spawn_blocking(move || kaeri_pages::print::fingerprint(&b))
                .await
                .ok()
                .flatten(),
            None => None,
        };
        edges.push(Edge { page, at_end, bytes, print });
    }
    (edges, pending)
}

/// The pages at each end of each part: its first two, and its last two.
fn ends(spans: &[(u32, u32)]) -> Vec<Vec<u32>> {
    let edge = |range: std::ops::Range<u32>| range.collect::<Vec<_>>();
    spans
        .iter()
        .flat_map(|&(start, len)| {
            let close = (start + len.saturating_sub(2)).max(start);
            [edge(start..start + len.min(2)), edge(close..start + len)]
        })
        .collect()
}

/// A chapter in parts as the reader is told of it: each part's first page counted without the
/// credit pages at `skipped` (from 0, through every page the source has).
fn parts_of(opened: &Opened, skipped: &[u32]) -> Option<FeedParts> {
    if opened.parts.len() < 2 && opened.missing.is_empty() {
        return None;
    }
    let parts = opened
        .spans()
        .into_iter()
        .zip(&opened.parts)
        .map(|((start, _), part)| {
            let group = part.chapter.version_label.clone();
            let other_group = group.is_some() && group != opened.group;
            let before = skipped.iter().filter(|p| **p < start).count() as u32;
            ChapterPart {
                number: part.number,
                first_page: start - before + 1,
                group,
                other_group,
            }
        })
        .collect();
    Some(FeedParts { parts, total: opened.total, missing: opened.missing.clone() })
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
        if self.gone.is() {
            return false;
        }
        let Some(&source) = self.order.get(index) else { return true };
        let slow = self.pending.remove(&source);
        let page =
            fetch(&self.sources, &mut self.opened, &dir, (index, source), slow, self.now).await;
        // Closed while the page came: it goes with the folder.
        if self.gone.swept() {
            return false;
        }
        let Some(feed) = feed.upgrade() else { return false };
        feed.set(index, page);
        (self.landed)(index as u32, out);
        true
    }
}

/// Fetch the feed's page `index`, the source's page `source`, into `dir`, named in reading
/// order with the type its bytes are: from its edge fetch still on its way, when there is one.
async fn fetch(
    sources: &BookSources,
    opened: &mut Opened,
    dir: &Path,
    (index, source): (usize, usize),
    slow: Option<Pending>,
    now: i64,
) -> FeedPage {
    let came = match slow {
        Some(mut slow) => match (&mut slow.0).await {
            Ok(Ok(bytes)) => Ok(bytes),
            _ => sources.page(opened, source, now).await,
        },
        None => sources.page(opened, source, now).await,
    };
    match came {
        Ok(bytes) => write(dir, &format!("{:05}", index + 1), &bytes).await,
        Err(failure) => FeedPage::Failed(failure),
    }
}

/// Write a page's bytes into `dir` as `name`, with the type they are.
async fn write(dir: &Path, name: &str, bytes: &[u8]) -> FeedPage {
    let Some(ext) = page_extension(bytes) else {
        return FeedPage::Failed(ReadingFailure::Unsupported { extension: "page".into() });
    };
    let path = dir.join(format!("{name}.{ext}"));
    let temp = dir.join(format!(".{name}.part"));
    let written = async {
        tokio::fs::write(&temp, bytes).await?;
        tokio::fs::rename(&temp, &path).await
    };
    match written.await {
        Ok(()) => FeedPage::Landed(path),
        Err(e) => FeedPage::Failed(ReadingFailure::Other { reason: e.to_string() }),
    }
}

/// The extension a page's bytes are of, by their first bytes, for the page worker.
pub(crate) fn page_extension(bytes: &[u8]) -> Option<&'static str> {
    let ext = kaeri_books::mime::sniff_image(bytes)?.strip_prefix("image/")?;
    Some(if ext == "jpeg" { "jpg" } else { ext })
}
