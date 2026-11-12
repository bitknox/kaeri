//! A source's chapter of pictures, fetched a page at a time into a folder while it is read.
//!
//! The pages at the chapter's edges come first, together: the first two and last two of each
//! part. Their fingerprints teach the series' credit pages, and the credit pages among them
//! leave the chapter before it is laid out, so its count and places match a downloaded copy's
//! and no page goes while it is read. An edge page that fails or is slow is waited on for
//! [`EDGE_WAIT`] at most; its end of the chapter keeps every page. Then the page the chapter
//! opens at, and the chapter is laid out as soon as it lands; the rest follow a few at a time,
//! from that page on and then the ones before it, each told to the page worker as it lands. A
//! page the reader waits on once the chapter is laid out (a jump, a scrub) is fetched before
//! the rest. A page is given [`PAGE_WAIT`] and asked once more before it fails, and a slow one
//! holds only itself. A page that fails is that page's failure, with its own retry: the others
//! still come. The fetching stops when nothing holds the feed, and what lands after the reader
//! closed goes at once.

use std::{
    collections::{HashMap, HashSet, VecDeque},
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
use tokio::{sync::mpsc, task::JoinSet};

use crate::{
    outbox::Outbox,
    read_sources::{BookSources, Opened},
    reading::credits::Learning,
};

/// The longest an edge page is waited on before the chapter is laid out without it, as long
/// as a novel's pictures are.
pub(crate) const EDGE_WAIT: Duration = Duration::from_secs(3);

/// How long one try at a page is waited on. A page not come by then is asked for once more,
/// then fails with its own retry.
pub(crate) const PAGE_WAIT: Duration = Duration::from_secs(15);

/// Pages fetched at once behind the first: enough that one slow page does not hold the rest.
const AT_ONCE: usize = 3;

/// More pages fetched at once for pages the reader waits on, past [`AT_ONCE`].
const WAITED_ON: usize = 2;

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

/// A chapter's parts as its source has them: where each starts among every page, from 0. A
/// download keeps these, so the chapter read from it is told its parts as one read from the
/// source is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SourceParts {
    pub parts: Vec<SourcePart>,
    pub total: u32,
    pub missing: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SourcePart {
    pub number: u32,
    /// Its first page, from 0, through every page the source has.
    pub start: u32,
    pub group: Option<String>,
    pub other_group: bool,
}

impl SourceParts {
    /// An opened chapter's parts; `None` for a chapter listed whole.
    pub(crate) fn of(opened: &Opened) -> Option<Self> {
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
                SourcePart { number: part.number, start, group, other_group }
            })
            .collect();
        Some(Self { parts, total: opened.total, missing: opened.missing.clone() })
    }

    /// As the reader is told of them: each part's first page counted without the credit pages
    /// at `skipped` (from 0, through every page the source has).
    pub(crate) fn counted(&self, skipped: &[u32]) -> FeedParts {
        let parts = self
            .parts
            .iter()
            .map(|part| {
                let before = skipped.iter().filter(|p| **p < part.start).count() as u32;
                ChapterPart {
                    number: part.number,
                    first_page: part.start - before + 1,
                    group: part.group.clone(),
                    other_group: part.other_group,
                }
            })
            .collect();
        FeedParts { parts, total: self.total, missing: self.missing.clone() }
    }
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
    /// Pages the reader waits on, for the fetcher to fetch before the rest.
    wanted: mpsc::UnboundedSender<usize>,
    /// Set once the reader left the chapter for another: see [`Feed::leave`].
    left: tokio::sync::watch::Sender<bool>,
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
    wanted: mpsc::UnboundedReceiver<usize>,
    gone: Gone,
    left: tokio::sync::watch::Receiver<bool>,
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

    /// No page is still on its way: each landed or failed.
    pub fn settled(&self) -> bool {
        self.pages.lock().is_ok_and(|p| p.iter().all(|p| !matches!(p, FeedPage::Coming)))
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

    /// The reader waits on page `index`: it is fetched next, if it is still to come.
    pub fn want(&self, index: usize) {
        if matches!(self.page(index), Some(FeedPage::Coming)) {
            let _ = self.wanted.send(index);
        }
    }

    /// The reader left the chapter for another: no more of its pages are fetched, and those
    /// on their way stop, so the chapter opening has the link to itself. The page worker may
    /// hold the feed a while longer, until the other chapter reaches it.
    pub fn leave(&self) {
        self.left.send_replace(true);
    }

    fn set(&self, index: usize, page: FeedPage) {
        if let Ok(mut pages) = self.pages.lock()
            && let Some(slot) = pages.get_mut(index)
        {
            *slot = page;
        }
    }
}

#[cfg(test)]
impl Feed {
    /// A feed of `pages` as they stand, that nothing fetches: for tests.
    pub(crate) fn standing(chapter: ChapterId, dir: PathBuf, pages: Vec<FeedPage>) -> Self {
        let (again, _) = mpsc::unbounded_channel();
        let (wanted, _) = mpsc::unbounded_channel();
        Self {
            chapter,
            dir,
            pages: Mutex::new(pages),
            credits: Vec::new(),
            parts: None,
            again,
            wanted,
            left: tokio::sync::watch::Sender::new(false),
        }
    }

    /// Page `index` landed or failed, as the fetcher would say.
    pub(crate) fn put(&self, index: usize, page: FeedPage) {
        self.set(index, page);
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
        if let FeedPage::Landed(path) = write(&credits_dir, &name, bytes.clone(), &gone).await {
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
            pages[shown] = write(&dir, &format!("{:05}", shown + 1), bytes, &gone).await;
        }
    }
    credits.sort_by_key(|c| c.page);
    let skipped: Vec<u32> = credits.iter().map(|c| c.page).collect();
    let parts = SourceParts::of(&opened).map(|p| p.counted(&skipped));
    let count = order.len();
    let first = first(u32::try_from(count).unwrap_or(u32::MAX)).min(count - 1);
    if !matches!(pages[first], FeedPage::Landed(_)) {
        let slow = pending.remove(&order[first]);
        let page =
            fetch(&sources, &mut opened, &dir, (first, order[first]), slow, now, &gone).await;
        if let FeedPage::Failed(failure) = page {
            return Err(failure);
        }
        pages[first] = page;
    }
    // Closed while the first pages came: they go with the session's folder.
    if gone.swept() {
        return Err(ReadingFailure::NotFound);
    }
    let rest: VecDeque<usize> = (first + 1..count)
        .chain(0..first)
        .filter(|i| !matches!(pages[*i], FeedPage::Landed(_)))
        .collect();
    let (again, asked) = mpsc::unbounded_channel();
    let (wanted, waited) = mpsc::unbounded_channel();
    let pages = Mutex::new(pages);
    let (left, leaving) = tokio::sync::watch::channel(false);
    let feed = Arc::new(Feed { chapter, dir, pages, credits, parts, again, wanted, left });
    let fetcher = Fetcher {
        sources,
        opened,
        order,
        pending,
        now,
        landed,
        again: asked,
        wanted: waited,
        gone,
        left: leaving,
    };
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

/// One page fetched: its index in the feed, what came, the chapter as the fetch left it (its
/// page addresses asked for again when they had expired), and who hears of it.
type Fetched = (usize, FeedPage, Opened, Option<Outbox>);

impl<F: Fn(u32, &Outbox) + Send + 'static> Fetcher<F> {
    async fn run(mut self, feed: Weak<Feed>, mut order: VecDeque<usize>, out: Outbox) {
        let mut out = Some(out);
        let mut running: JoinSet<Fetched> = JoinSet::new();
        // Pages the reader waits on, in the order it asked, and the pages being fetched.
        let mut urgent: VecDeque<usize> = VecDeque::new();
        let mut asked: HashSet<usize> = HashSet::new();
        let mut open = true;
        loop {
            // Returning drops the fetches on their way.
            if feed.strong_count() == 0 || self.gone.is() || *self.left.borrow() {
                return;
            }
            // A page waited on starts even with the others all under way.
            while running.len() < AT_ONCE + WAITED_ON
                && let Some(index) = urgent.pop_front()
            {
                if asked.insert(index) {
                    order.retain(|i| *i != index);
                    self.spawn(&mut running, &feed, index, out.clone());
                }
            }
            while running.len() < AT_ONCE
                && let Some(index) = order.pop_front()
            {
                if asked.insert(index) {
                    self.spawn(&mut running, &feed, index, out.clone());
                }
            }
            // The first pass is all under way: only retries hold a way to answer from here, so
            // an engine waiting on nothing can go idle.
            if order.is_empty() && urgent.is_empty() {
                out = None;
            }
            if running.is_empty() && !open {
                return;
            }
            tokio::select! {
                Some(done) = running.join_next(), if !running.is_empty() => {
                    let Ok((index, page, opened, told)) = done else { continue };
                    // Fresh page addresses serve the pages fetched after.
                    if moved(&opened, &self.opened) {
                        self.opened = opened;
                    }
                    let Some(feed) = feed.upgrade() else { return };
                    feed.set(index, page);
                    asked.remove(&index);
                    if let Some(told) = told {
                        (self.landed)(index as u32, &told);
                    }
                }
                wanted = self.wanted.recv(), if open => match wanted {
                    Some(index) if order.contains(&index) => {
                        urgent.retain(|i| *i != index);
                        urgent.push_back(index);
                    }
                    Some(_) => {}
                    None => open = false,
                },
                again = self.again.recv(), if open => match again {
                    Some((index, told)) => {
                        if asked.insert(index) {
                            self.spawn(&mut running, &feed, index, Some(told));
                        }
                    }
                    None => open = false,
                },
                // Seen at the top of the loop. An error means the feed is gone, also seen there.
                _ = self.left.changed() => {}
                else => return,
            }
        }
    }

    /// Fetch the feed's page `index` beside the others under way.
    fn spawn(
        &mut self,
        running: &mut JoinSet<Fetched>,
        feed: &Weak<Feed>,
        index: usize,
        told: Option<Outbox>,
    ) {
        let Some(dir) = feed.upgrade().map(|f| f.dir.clone()) else { return };
        let Some(&source) = self.order.get(index) else { return };
        let slow = self.pending.remove(&source);
        let (sources, mut opened, now, gone) =
            (self.sources.clone(), self.opened.clone(), self.now, self.gone.clone());
        running.spawn(async move {
            let page =
                fetch(&sources, &mut opened, &dir, (index, source), slow, now, &gone).await;
            (index, page, opened, told)
        });
    }
}

/// Whether a fetch asked for `after`'s page addresses again, so they differ from `before`'s.
fn moved(after: &Opened, before: &Opened) -> bool {
    after.content != before.content
        || after.parts.iter().zip(&before.parts).any(|(a, b)| a.pages != b.pages)
}

/// Fetch the feed's page `index`, the source's page `source`, into `dir`, named in reading
/// order with the type its bytes are: from its edge fetch still on its way, when there is one.
/// A try not answered within [`PAGE_WAIT`] is made once more; a page that hangs then fails as
/// its source not answering.
async fn fetch(
    sources: &BookSources,
    opened: &mut Opened,
    dir: &Path,
    (index, source): (usize, usize),
    slow: Option<Pending>,
    now: i64,
    gone: &Gone,
) -> FeedPage {
    let came = match slow {
        Some(mut slow) => match tokio::time::timeout(PAGE_WAIT, &mut slow.0).await {
            Ok(Ok(Ok(bytes))) => Ok(bytes),
            Ok(_) => within(sources, opened, source, now, 2).await,
            // The edge fetch was its first try.
            Err(_) => within(sources, opened, source, now, 1).await,
        },
        None => within(sources, opened, source, now, 2).await,
    };
    match came {
        Ok(bytes) => write(dir, &format!("{:05}", index + 1), bytes, gone).await,
        Err(failure) => FeedPage::Failed(failure),
    }
}

/// The source's page `index`, with `tries` tries of [`PAGE_WAIT`] each.
async fn within(
    sources: &BookSources,
    opened: &mut Opened,
    index: usize,
    now: i64,
    tries: u32,
) -> Result<Vec<u8>, ReadingFailure> {
    for _ in 0..tries {
        if let Ok(came) =
            tokio::time::timeout(PAGE_WAIT, sources.page(opened, index, now)).await
        {
            return came;
        }
    }
    Err(ReadingFailure::SourceDown { source: sources.source_name(&opened.source_id) })
}

/// Write a page's bytes into `dir` as `name`, with the type they are. The write runs to its
/// end even when the fetch waiting on it stops, and a page written after the session closed
/// goes with its folder.
pub(crate) async fn write(dir: &Path, name: &str, bytes: Vec<u8>, gone: &Gone) -> FeedPage {
    let Some(ext) = page_extension(&bytes) else {
        return FeedPage::Failed(ReadingFailure::Unsupported { extension: "page".into() });
    };
    let path = dir.join(format!("{name}.{ext}"));
    let temp = dir.join(format!(".{name}.part"));
    let gone = gone.clone();
    let written = tokio::task::spawn_blocking(move || {
        let written =
            std::fs::write(&temp, &bytes).and_then(|()| std::fs::rename(&temp, &path));
        if gone.swept() {
            return FeedPage::Failed(ReadingFailure::NotFound);
        }
        match written {
            Ok(()) => FeedPage::Landed(path),
            Err(e) => FeedPage::Failed(ReadingFailure::Other { reason: e.to_string() }),
        }
    });
    written
        .await
        .unwrap_or_else(|e| FeedPage::Failed(ReadingFailure::Other { reason: e.to_string() }))
}

/// The extension a page's bytes are of, by their first bytes, for the page worker.
pub(crate) fn page_extension(bytes: &[u8]) -> Option<&'static str> {
    let ext = kaeri_books::mime::sniff_image(bytes)?.strip_prefix("image/")?;
    Some(if ext == "jpeg" { "jpg" } else { ext })
}
