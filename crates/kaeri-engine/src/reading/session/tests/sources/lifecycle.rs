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

    fn passes(&self, index: usize) -> bool {
        self.let_through.lock().unwrap().as_ref().is_none_or(|s| s.contains(&index))
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
        self.fetched.lock().unwrap().push((chapter, index));
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.passes(index) {
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
    h.pump(Duration::from_millis(500)).await;
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
    let failure = timeout(
        Duration::from_secs(120),
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
