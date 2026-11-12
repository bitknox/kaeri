//! A session over a source as it opens and closes: an open waits for plugins still loading, a
//! chapter not listed is not swapped for another, a close leaves nothing behind, a deleted
//! download reads from the source again, and keeping chapters ready asks the source once. Its
//! pages come a few at a time, a page jumped to comes first, and a page that hangs holds only
//! itself.

use std::{collections::HashSet, sync::Mutex, time::Duration};

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, PageRef, SeriesDetail,
        SeriesHit, SourceChapter,
    },
    error::ProviderError,
    traits::PageError,
};
use tokio::time::timeout;

use super::*;

const MANY: &str = "a:900780";
const MANY_SOURCE: &str = "many-1";

/// A source of `chapters` chapters of `pages` pages each. Pages not let through wait.
struct Many {
    manifest: BookSourceManifest,
    chapters: usize,
    pages: usize,
    /// Every page fetched, as (chapter, page), in the order asked.
    fetched: Mutex<Vec<(String, usize)>>,
    /// `None`: every page comes. `Some(set)`: only these page numbers come; the rest wait.
    let_through: Mutex<Option<HashSet<usize>>>,
    /// Every page of this chapter (its id in the source) waits, whatever else comes.
    held_chapter: Mutex<Option<String>>,
    changed: tokio::sync::Notify,
}

impl Many {
    fn new(chapters: usize, pages: usize) -> Self {
        Self {
            manifest: BookSourceManifest {
                id: MANY_SOURCE.into(),
                name: "Many pages".into(),
                version: "1".into(),
                kind: BookSourceKind::Server,
                media: vec![Medium::Manga],
                languages: vec!["en".into()],
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            },
            chapters,
            pages,
            fetched: Mutex::default(),
            let_through: Mutex::new(None),
            held_chapter: Mutex::new(None),
            changed: tokio::sync::Notify::new(),
        }
    }

    fn hold_all_but(&self, pages: &[usize]) {
        *self.let_through.lock().unwrap() = Some(pages.iter().copied().collect());
        self.changed.notify_waiters();
    }

    /// Every page comes but these, which wait.
    fn hold(&self, pages: &[usize]) {
        let through: Vec<usize> = (0..self.pages).filter(|p| !pages.contains(p)).collect();
        self.hold_all_but(&through);
    }

    /// How many times page `index` was asked for.
    fn asked_of(&self, index: usize) -> usize {
        self.fetched().iter().filter(|(_, p)| *p == index).count()
    }

    fn let_all(&self) {
        *self.let_through.lock().unwrap() = None;
        self.changed.notify_waiters();
    }

    fn hold_chapter(&self, chapter: &str) {
        *self.held_chapter.lock().unwrap() = Some(chapter.to_owned());
        self.changed.notify_waiters();
    }

    fn let_chapter(&self) {
        *self.held_chapter.lock().unwrap() = None;
        self.changed.notify_waiters();
    }

    fn passes(&self, chapter: &str, index: usize) -> bool {
        self.held_chapter.lock().unwrap().as_deref() != Some(chapter)
            && self.let_through.lock().unwrap().as_ref().is_none_or(|s| s.contains(&index))
    }

    fn fetched(&self) -> Vec<(String, usize)> {
        self.fetched.lock().unwrap().clone()
    }
}

#[async_trait]
impl kaeri_core::traits::BookSource for Many {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }
    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }
    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, "The Copper Orchard"))
    }
    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok((1..=self.chapters)
            .map(|n| SourceChapter::new(format!("ch-{n}"), n.to_string()))
            .collect())
    }
    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        Ok(ChapterContent::Images(
            (0..self.pages)
                .map(|n| PageRef::new(format!("https://pages.example/{chapter}/{n}")))
                .collect(),
        ))
    }
    async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
        let mut parts = page.url.rsplit('/');
        let index: usize = parts.next().unwrap().parse().unwrap();
        let chapter = parts.next().unwrap().to_owned();
        self.fetched.lock().unwrap().push((chapter.clone(), index));
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.passes(&chapter, index) {
                break;
            }
            changed.await;
        }
        let shade = 40 + u8::try_from(index % 200).unwrap();
        let picture = image::GrayImage::from_pixel(320, 480, image::Luma([shade]));
        let mut png = Vec::new();
        picture.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        Ok(png)
    }
}

fn many(source: Arc<Many>) -> Harness {
    let h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
    h.services.book_sources.registry.put(source);
    h.services.store.set_book_match(MANY_SOURCE, "series-1", MANY, 90, true, false, 1).unwrap();
    h
}

impl Harness {
    fn open_many(&mut self, chapter: Option<&str>) {
        self.send(ReaderCommand::Open {
            work: WorkId::new(MANY),
            chapter: chapter.map(ChapterId::new),
            at: OpenAt::Start,
            surface: surface(),
        });
    }

    /// Files under the reader's novel and streamed cache, as paths within it.
    fn streamed_files(&self) -> Vec<String> {
        let root = self.services.paths.cache_dir.join("novel");
        let mut out = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push(p.strip_prefix(&root).unwrap().display().to_string());
                }
            }
        }
        out.sort();
        out
    }

    /// Carry what comes for `wait`.
    async fn pump(&mut self, wait: Duration) {
        let end = tokio::time::Instant::now() + wait;
        while let Ok(Some(envelope)) = tokio::time::timeout_at(end, self.inbox.recv()).await {
            self.carry(envelope);
        }
    }

    /// Whether a view opened among the events not yet taken.
    fn any_opened(&mut self) -> Vec<String> {
        self.drain()
            .iter()
            .filter_map(|e| match e {
                ReaderEvent::Opened(v) => Some(v.chapter.id.to_string()),
                _ => None,
            })
            .collect()
    }
}

/// A chapter of [`READ`] downloaded into `folder`, kept as done.
fn downloaded(h: &Harness, folder: &Path, chapter: &str, keep_ready: bool) -> PathBuf {
    let cbz = folder.join(format!("{chapter}.cbz"));
    super::super::cbz(&cbz, 3);
    let store = &h.services.store;
    let ask = kaeri_store::book_downloads::BookDownloadAsk {
        work: READ,
        chapter,
        source_id: SOURCE,
        version_label: None,
        keep_ready,
    };
    store.queue_book_download(&ask, 1).unwrap();
    let id = store.book_download(READ, chapter).unwrap().unwrap().id;
    store.book_download_done(id, cbz.to_str().unwrap(), Some(3), 1, 1).unwrap();
    cbz
}

/// An open asked for at launch, while the plugins still load, waits for its source, then
/// opens the chapter asked for and not a downloaded one in its place.
#[tokio::test]
async fn an_open_while_plugins_load_waits_and_opens_the_chapter_asked_for() {
    let source = Arc::new(PageSource::new(&["1", "2", "3"]));
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
    h.services.store.set_book_match(SOURCE, "series-1", READ, 90, true, false, 1).unwrap();
    let folder = tempfile::tempdir().unwrap();
    downloaded(&h, folder.path(), "3", false);
    h.services.book_plugins.expect_arrivals();

    h.send(ReaderCommand::Open {
        work: WorkId::new(READ),
        chapter: Some(ChapterId::new("2")),
        at: OpenAt::Start,
        surface: surface(),
    });
    h.pump(Duration::from_millis(300)).await;
    assert!(h.any_opened().is_empty(), "opened before the source was in");

    h.services.book_sources.registry.put(source.clone());
    h.services.book_plugins.arrivals_done();
    let view = timeout(
        Duration::from_secs(20),
        h.until(|e| match e {
            ReaderEvent::Opened(view) => Some(view.as_ref().clone()),
            _ => None,
        }),
    )
    .await
    .expect("opened once the plugins were in");
    assert_eq!(view.chapter.id.as_str(), "2");
}

/// A chapter the source does not list fails as not found; no other chapter opens for it.
#[tokio::test]
async fn a_chapter_not_listed_is_not_found_and_opens_no_other() {
    let source = Arc::new(PageSource::new(&["1", "2"]));
    let mut h = reading(source.clone());
    h.failures = true;
    h.send(ReaderCommand::Open {
        work: WorkId::new(READ),
        chapter: Some(ChapterId::new("9")),
        at: OpenAt::Start,
        surface: surface(),
    });
    let start = tokio::time::Instant::now();
    while h.failed.is_empty() && start.elapsed() < Duration::from_secs(10) {
        h.pump(Duration::from_millis(50)).await;
    }
    assert_eq!(h.failed, [ReadingFailure::NotFound]);
    assert!(h.any_opened().is_empty(), "another chapter opened");
    assert!(h.reader.session.is_none());
}

/// Leaving while a source chapter's first page is on its way leaves no pages on disk.
#[tokio::test]
async fn leaving_while_the_first_page_comes_leaves_no_files() {
    let source = Arc::new(Many::new(2, 5));
    source.hold_all_but(&[]);
    let mut h = many(source.clone());
    h.failures = true;
    h.open_many(Some("1"));
    let start = tokio::time::Instant::now();
    while source.fetched().is_empty() && start.elapsed() < Duration::from_secs(5) {
        h.pump(Duration::from_millis(20)).await;
    }
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    source.let_all();
    h.pump(Duration::from_millis(800)).await;
    let files = h.streamed_files();
    assert!(files.is_empty(), "files left behind: {files:?}");
}

/// Fifty opens closed at once, then fifty closed once laid out, hold nothing afterwards: no
/// files stay and the tasks go back to where they were.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fifty_opens_and_closes_hold_nothing() {
    let source = Arc::new(Many::new(3, 6));
    let mut h = many(source.clone());
    h.failures = true;
    let metrics = tokio::runtime::Handle::current().metrics();
    let before = metrics.num_alive_tasks();
    for _ in 0..50 {
        h.open_many(Some("1"));
        let session = h.reader.session.as_ref().map_or(0, |s| s.id);
        h.send(ReaderCommand::Close { session });
    }
    h.pump(Duration::from_millis(500)).await;
    for _ in 0..50 {
        h.open_many(Some("2"));
        timeout(Duration::from_secs(20), h.leaves_of("2")).await.expect("laid out");
        let session = h.session();
        h.send(ReaderCommand::Close { session });
    }
    h.pump(Duration::from_millis(800)).await;
    let after = metrics.num_alive_tasks();
    let files = h.streamed_files();
    assert!(h.reader.session.is_none() && h.reader.listing.is_none());
    assert!(files.is_empty(), "{} files left", files.len());
    assert!(after <= before + 2, "tasks grew from {before} to {after}");
}

/// With chapters kept ready, reading on asks the source for its list once, and the next
/// chapter's content is fetched once, by the download, not again by the reader.
#[tokio::test]
async fn keeping_chapters_ready_asks_the_source_once() {
    use kaeri_core::config::NewChapters;

    use crate::downloads::books::Setup;
    let source = Arc::new(PageSource::new(&["1", "2", "3", "4", "5", "6"]));
    let mut h = reading(source.clone());
    h.services.book_sources.downloads.configure(Setup {
        keep_ready: true,
        new_chapters: NewChapters::Reading,
        ..Setup::default()
    });
    let now = h.services.clock.now();
    h.services.book_sources.chapters(&WorkId::new(READ), None, now).await.unwrap();
    assert_eq!(source.listed(), 1);
    let wait = Duration::from_secs(20);
    h.open_read(Some("1")).await;
    timeout(wait, h.leaves_of("1")).await.unwrap();
    for leaf in 0..=3 {
        h.shown(leaf, TurnHow::Turn).await;
    }
    // Reading on once chapter 2 is kept ready: its download is done.
    let start = tokio::time::Instant::now();
    let done = |h: &Harness| {
        h.services.store.book_download(READ, "2").unwrap().is_some_and(|row| {
            row.state == kaeri_store::book_downloads::BookDownloadState::Done
        })
    };
    while !done(&h) && start.elapsed() < wait {
        h.pump(Duration::from_millis(50)).await;
    }
    assert!(done(&h), "chapter 2 was never kept ready");
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    timeout(wait, h.leaves_of("2")).await.unwrap();
    for leaf in 0..=3 {
        h.shown(leaf, TurnHow::Turn).await;
    }
    h.pump(Duration::from_secs(6)).await;
    assert_eq!(source.listed(), 1, "the list was asked for again");
    assert!(
        source.opened_of("ch-2") <= 1,
        "chapter 2 asked {} times",
        source.opened_of("ch-2")
    );
}

/// Chapter 1 of [`MANY`] open with chapters kept ready, and the keep-ready download of chapter
/// 2 held on its first page.
async fn kept_ready_and_held(source: &Arc<Many>) -> Harness {
    use kaeri_core::config::NewChapters;

    use crate::downloads::books::Setup;
    source.hold_chapter("ch-2");
    let mut h = many(source.clone());
    h.services.book_sources.downloads.configure(Setup {
        keep_ready: true,
        new_chapters: NewChapters::Reading,
        ..Setup::default()
    });
    h.open_many(Some("1"));
    timeout(Duration::from_secs(20), h.leaves_of("1")).await.expect("laid out");
    let start = tokio::time::Instant::now();
    let held = |source: &Many| source.asked_in("ch-2").first().is_some_and(|(p, _)| *p == 0);
    while !held(source) && start.elapsed() < Duration::from_secs(20) {
        h.pump(Duration::from_millis(20)).await;
    }
    assert!(held(source), "chapter 2 was never kept ready");
    h
}

impl Harness {
    /// Carry what comes until chapter `chapter` of [`MANY`]'s download is `state`.
    async fn until_download(
        &mut self,
        chapter: &str,
        state: kaeri_store::book_downloads::BookDownloadState,
    ) {
        let start = tokio::time::Instant::now();
        let reached = |h: &Self| {
            h.services
                .store
                .book_download(MANY, chapter)
                .unwrap()
                .is_some_and(|r| r.state == state)
        };
        while !reached(self) && start.elapsed() < Duration::from_secs(20) {
            self.pump(Duration::from_millis(20)).await;
        }
        assert!(reached(self), "chapter {chapter}'s download never came to {state:?}");
    }
}

/// A chapter opened while its keep-ready download runs waits for that download and opens from
/// its file: the source is asked for each of its pages once.
#[tokio::test]
async fn a_chapter_opened_while_kept_ready_joins_its_download() {
    let source = Arc::new(Many::new(4, 3));
    let mut h = kept_ready_and_held(&source).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    source.let_chapter();
    let leaves = timeout(Duration::from_secs(20), h.leaves_of("2")).await.expect("laid out");
    assert!(!leaves.is_empty(), "no pages shown");
    let s = h.reader.session.as_ref().unwrap();
    assert_eq!(s.chapter().format, "cbz", "not opened from the download");
    let asked = source.asked_in("ch-2");
    assert_eq!(asked, vec![(0, 1), (1, 1), (2, 1)], "chapter 2 asked again: {asked:?}");
}

/// A download taken out of the queue while its chapter waits on it: the reader asks the source
/// for the chapter itself.
#[tokio::test]
async fn a_download_cancelled_while_its_chapter_opens_reads_from_the_source() {
    let source = Arc::new(Many::new(4, 3));
    let mut h = kept_ready_and_held(&source).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    let lent = crate::downloads::books::Lent::of(&h.services, &h.tx);
    let work = WorkId::new(MANY);
    h.services.book_sources.downloads.cancel(&lent, &work, &[ChapterId::new("2")]);
    source.let_chapter();
    let leaves = timeout(Duration::from_secs(20), h.leaves_of("2")).await.expect("laid out");
    assert!(!leaves.is_empty(), "no pages shown");
    assert_eq!(h.reader.session.as_ref().unwrap().chapter().format, "pages");
}

/// The reader closed while its chapter waits on a download: the download goes on and the source
/// is not asked for the chapter again.
#[tokio::test]
async fn a_reader_closed_while_waiting_on_a_download_leaves_it_running() {
    let source = Arc::new(Many::new(4, 3));
    let mut h = kept_ready_and_held(&source).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    h.send(ReaderCommand::Close { session });
    source.let_chapter();
    h.until_download("2", kaeri_store::book_downloads::BookDownloadState::Done).await;
    let asked = source.asked_in("ch-2");
    assert_eq!(asked, vec![(0, 1), (1, 1), (2, 1)], "chapter 2 asked again: {asked:?}");
}

/// A chapter opened while its keep-ready download waits in a held queue is fetched by the
/// reader, and leaves the queue: once the queue runs, the source is not asked for it again.
#[tokio::test]
async fn a_chapter_the_reader_fetches_leaves_the_keep_ready_queue() {
    use kaeri_core::config::NewChapters;
    use kaeri_store::book_downloads::BookDownloadState as State;

    use crate::downloads::books::{Lent, Setup};
    let source = Arc::new(Many::new(5, 3));
    let mut h = many(source.clone());
    let setup =
        Setup { keep_ready: true, new_chapters: NewChapters::Reading, ..Setup::default() };
    h.services.book_sources.downloads.configure(Setup { hold: true, ..setup });
    h.open_many(Some("1"));
    timeout(Duration::from_secs(20), h.leaves_of("1")).await.expect("laid out");
    h.until_download("2", State::Queued).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    timeout(Duration::from_secs(20), h.leaves_of("2")).await.expect("laid out");
    h.until_download("2", State::Removed).await;
    h.until_download("5", State::Queued).await;
    let downloads = h.services.book_sources.downloads.clone();
    downloads.configure(setup);
    downloads.start(&Lent::of(&h.services, &h.tx));
    h.until_download("5", State::Done).await;
    for chapter in ["ch-2", "ch-3", "ch-4", "ch-5"] {
        let asked = source.asked_in(chapter);
        assert_eq!(asked, vec![(0, 1), (1, 1), (2, 1)], "{chapter} asked again: {asked:?}");
    }
}

/// The download of a chapter being read is deleted; crossing on and back to it in the same
/// session reads it from the source.
#[tokio::test]
async fn a_download_deleted_while_reading_opens_from_the_source() {
    let source = Arc::new(PageSource::new(&["1", "2", "3", "4"]));
    let mut h = reading(source.clone());
    let now = h.services.clock.now();
    h.services.book_sources.chapters(&WorkId::new(READ), None, now).await.unwrap();
    let folder = tempfile::tempdir().unwrap();
    downloaded(&h, folder.path(), "2", true);

    let wait = Duration::from_secs(20);
    h.open_read(Some("2")).await;
    timeout(wait, h.leaves_of("2")).await.unwrap();
    let lent = crate::downloads::books::Lent::of(&h.services, &h.tx);
    let removed = h.services.book_sources.downloads.delete(
        &lent,
        &WorkId::new(READ),
        &[ChapterId::new("2")],
    );
    assert_eq!(removed, 1);
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("3") });
    timeout(wait, h.leaves_of("3")).await.unwrap();
    h.failures = true;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    let back = timeout(wait, h.leaves_of("2")).await;
    assert!(back.is_ok(), "chapter 2 did not open again: {:?}", h.failed);
    assert!(h.failed.is_empty(), "{:?}", h.failed);
    assert_eq!(source.opened_of("ch-2"), 1, "read from the source");
}

impl Many {
    /// The pages of `chapter` asked for, as (page, times asked), in page order.
    fn asked_in(&self, chapter: &str) -> Vec<(usize, usize)> {
        let mut counts = std::collections::BTreeMap::new();
        for (c, p) in self.fetched() {
            if c == chapter {
                *counts.entry(p).or_insert(0) += 1;
            }
        }
        counts.into_iter().collect()
    }
}

impl Harness {
    /// Carry what comes until `source` has been asked for a page.
    async fn until_asked(&mut self, source: &Many) {
        let start = tokio::time::Instant::now();
        while source.fetched().is_empty() && start.elapsed() < Duration::from_secs(5) {
            self.pump(Duration::from_millis(10)).await;
        }
        assert!(!source.fetched().is_empty(), "the source was never asked");
    }
}

/// A chapter opened again while it opens, as when another device's place opens it and the
/// person opens it too, takes the open under way: each page is asked for once.
#[tokio::test]
async fn a_chapter_opened_again_while_it_opens_asks_for_each_page_once() {
    let source = Arc::new(Many::new(2, 8));
    source.hold_all_but(&[]);
    let mut h = many(source.clone());
    h.open_many(Some("2"));
    h.until_asked(&source).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    source.let_all();
    timeout(Duration::from_secs(20), h.leaves_of("2")).await.expect("laid out");
    let start = tokio::time::Instant::now();
    while source.asked_in("ch-2").len() < 8 && start.elapsed() < Duration::from_secs(10) {
        h.pump(Duration::from_millis(50)).await;
    }
    h.pump(Duration::from_millis(300)).await;
    let asked = source.asked_in("ch-2");
    assert_eq!(asked.len(), 8, "every page asked for: {asked:?}");
    assert!(asked.iter().all(|(_, n)| *n == 1), "asked for more than once: {asked:?}");
}

/// A chapter left while it opens stops: none of its pages is asked for once another chapter
/// opens.
#[tokio::test]
async fn a_chapter_left_while_it_opens_stops_fetching() {
    let source = Arc::new(Many::new(2, 12));
    source.hold_all_but(&[]);
    let mut h = many(source.clone());
    h.open_many(Some("1"));
    h.until_asked(&source).await;
    let before = source.asked_in("ch-1");
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    source.let_all();
    timeout(Duration::from_secs(20), h.leaves_of("2")).await.expect("laid out");
    h.pump(Duration::from_secs(1)).await;
    assert_eq!(source.asked_in("ch-1"), before, "the chapter left went on fetching");
}

/// A jump once the chapter is laid out asks for the page jumped to next, ahead of the pages
/// before it still to come, even while those hang.
#[tokio::test]
async fn a_jump_asks_for_the_page_jumped_to_first() {
    let source = Arc::new(Many::new(1, 40));
    source.hold_all_but(&[0, 1, 38, 39]);
    let mut h = many(source.clone());
    h.open_many(Some("1"));
    let leaves = timeout(Duration::from_secs(20), h.leaves_of("1")).await.expect("laid out");
    h.pump(Duration::from_millis(300)).await;
    let asked = source.fetched().len();
    let leaf = leaves.iter().find(|l| l.parts.iter().any(|p| p.page == 30)).unwrap().index;
    h.shown(leaf, TurnHow::Jump).await;
    h.pump(Duration::from_millis(500)).await;
    let after = source.fetched();
    let next: Vec<usize> = after[asked..].iter().map(|(_, p)| *p).collect();
    assert!(next.iter().take(3).any(|p| *p == 30), "asked after the jump: {next:?}");
}

/// Leaving a chapter for another stops fetching the one left, while the other's pages are
/// still on their way: no page of it shares the link with the page the reader waits on.
#[tokio::test]
async fn the_chapter_left_stops_fetching_its_pages() {
    let source = Arc::new(Many::new(2, 40));
    source.hold_all_but(&[0, 1, 38, 39]);
    let mut h = many(source.clone());
    h.open_many(Some("1"));
    timeout(Duration::from_secs(20), h.leaves_of("1")).await.expect("laid out");
    h.pump(Duration::from_millis(300)).await;
    // Chapter 2's pages wait, so it is not yet with the page worker.
    source.hold_chapter("ch-2");
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    h.pump(Duration::from_millis(100)).await;
    let of_one = |s: &Many| s.fetched().iter().filter(|(c, _)| c == "ch-1").count();
    let left = of_one(&source);
    source.let_all();
    h.pump(Duration::from_millis(800)).await;
    assert_eq!(of_one(&source), left, "chapter 1 was fetched on after it was left");
}

/// A place past the chapter's last page, as one kept from a longer copy of it, opens at the
/// last page, and the view says that page rather than the one asked for.
#[tokio::test]
async fn an_open_past_the_last_page_opens_at_the_last_page() {
    let source = Arc::new(Many::new(1, 5));
    let mut h = many(source.clone());
    h.send(ReaderCommand::Open {
        work: WorkId::new(MANY),
        chapter: Some(ChapterId::new("1")),
        at: OpenAt::Page { page: 99 },
        surface: surface(),
    });
    let opened = h.until(|e| match e {
        ReaderEvent::Opened(view) => Some(view.as_ref().clone()),
        _ => None,
    });
    let view = timeout(Duration::from_secs(20), opened).await.expect("opened");
    assert_eq!(view.start.page, Some(4));
    let first = h.until(|e| match e {
        ReaderEvent::PageReady { page, .. } => Some(*page),
        _ => None,
    });
    assert_eq!(timeout(Duration::from_secs(20), first).await.expect("a page"), 4);
}

/// A page that does not come holds only itself: the pages after it are asked for and land.
#[tokio::test]
async fn a_page_that_hangs_holds_only_itself() {
    let source = Arc::new(Many::new(1, 40));
    source.hold(&[2]);
    let mut h = many(source.clone());
    h.open_many(Some("1"));
    timeout(Duration::from_secs(20), h.leaves_of("1")).await.expect("laid out");
    let start = tokio::time::Instant::now();
    while source.asked_of(37) == 0 && start.elapsed() < Duration::from_secs(10) {
        h.pump(Duration::from_millis(50)).await;
    }
    let missing: Vec<usize> = (3..38).filter(|p| source.asked_of(*p) == 0).collect();
    assert!(missing.is_empty(), "never asked for: {missing:?}");
}

/// `wait`, or `None` once `limit` of real time has passed. With the clock paused, a timeout on it
/// runs out the moment the runtime has nothing to do, as while the page worker, a thread outside
/// the runtime, makes a page; this one is kept by a thread of its own.
async fn within_real<T>(limit: Duration, wait: impl Future<Output = T>) -> Option<T> {
    let (ran_out, out) = tokio::sync::oneshot::channel::<()>();
    let (done, stop) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = stop.recv_timeout(limit) {
            let _ = ran_out.send(());
        }
    });
    let got = tokio::select! {
        got = wait => Some(got),
        _ = out => None,
    };
    drop(done);
    got
}

/// A page that never comes is asked for once more after its wait, then fails as its source not
/// answering, with its own retry; the rest of the chapter still comes. The clock stops once the
/// chapter is laid out, so the waits pass at once.
#[tokio::test]
async fn a_page_that_never_comes_fails_after_one_more_try() {
    let source = Arc::new(Many::new(1, 12));
    source.hold(&[2]);
    let mut h = many(source.clone());
    h.failures = true;
    h.open_many(Some("1"));
    let leaves = timeout(Duration::from_secs(20), h.leaves_of("1")).await.expect("laid out");
    let leaf = leaves.iter().find(|l| l.parts.iter().any(|p| p.page == 2)).unwrap().index;
    tokio::time::pause();
    h.shown(leaf, TurnHow::Jump).await;
    let failure = within_real(
        Duration::from_secs(20),
        h.until(|e| match e {
            ReaderEvent::PageFailed { page: 2, failure, .. } => Some(failure.clone()),
            _ => None,
        }),
    )
    .await
    .expect("page 2 failed in time");
    assert!(matches!(failure, ReadingFailure::SourceDown { .. }), "{failure:?}");
    assert_eq!(source.asked_of(2), 2, "page 2 asked for once more");
    let missing: Vec<usize> = (3..12).filter(|p| source.asked_of(*p) == 0).collect();
    assert!(missing.is_empty(), "never asked for: {missing:?}");
}
