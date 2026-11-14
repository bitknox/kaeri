//! The next chapter from a source, fetched ahead while the open one is read, the way the reader
//! fetches a source's pages: its first page is the end page's preview, nothing waits on it, and
//! the chapter opens from what was fetched.

use std::{
    collections::HashMap,
    io::Cursor,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookFile, BookHints, BookSourceKind, BookSourceManifest, ChapterContent, PageRef,
        SeriesDetail, SeriesHit, SourceChapter,
    },
    error::ProviderError,
    traits::PageError,
};
use kaeri_model::reader::PageFile;

use super::*;

/// A book server holding one series: its chapters of pictures name three pages each, and a
/// chapter kept as a book file opens only by downloading the whole file.
struct ShelfServer {
    manifest: BookSourceManifest,
    /// The chapters it lists, in order.
    chapters: Vec<&'static str>,
    /// The chapter kept as a book file.
    file_chapter: Option<&'static str>,
    /// Page addresses of this chapter answer 403 until it is asked for again.
    expires: Option<&'static str>,
    /// How many times each chapter's content was asked for.
    asked: Mutex<HashMap<String, u32>>,
    /// Every page address fetched, in order.
    fetched: Mutex<Vec<String>>,
    /// A whole book file was downloaded.
    downloaded: AtomicBool,
    /// Pages of this chapter wait here before they come.
    gate: Option<(&'static str, Arc<tokio::sync::Notify>)>,
    /// Pages fetched past the gate.
    passed: AtomicU32,
}

impl ShelfServer {
    fn new(chapters: Vec<&'static str>) -> Self {
        Self {
            manifest: BookSourceManifest {
                id: "shelf-1".into(),
                name: "Home shelf".into(),
                version: "1".into(),
                kind: BookSourceKind::Server,
                media: vec![Medium::Manga],
                languages: vec!["en".into()],
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            },
            chapters,
            file_chapter: None,
            expires: None,
            asked: Mutex::default(),
            fetched: Mutex::default(),
            downloaded: AtomicBool::new(false),
            gate: None,
            passed: AtomicU32::new(0),
        }
    }

    /// The page addresses fetched for `chapter`.
    fn fetched_of(&self, chapter: &str) -> Vec<String> {
        let prefix = format!("https://books.example/{chapter}/");
        self.fetched
            .lock()
            .unwrap()
            .iter()
            .filter(|u| u.starts_with(&prefix))
            .cloned()
            .collect()
    }

    fn asked_of(&self, chapter: &str) -> u32 {
        self.asked.lock().unwrap().get(chapter).copied().unwrap_or(0)
    }

    /// The pages of `chapter`, as asked for the `round`th time.
    fn pages(chapter: &str, round: u32) -> Vec<PageRef> {
        (1..=3)
            .map(|n| PageRef::new(format!("https://books.example/{chapter}/{n}?round={round}")))
            .collect()
    }
}

#[async_trait]
impl kaeri_core::traits::BookSource for ShelfServer {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, "The Paper Orchard"))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok(self
            .chapters
            .iter()
            .enumerate()
            .map(|(i, id)| SourceChapter::new(*id, (i + 1).to_string()))
            .collect())
    }

    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        let round = {
            let mut asked = self.asked.lock().unwrap();
            let round = asked.entry(chapter.to_owned()).or_default();
            *round += 1;
            *round
        };
        if self.file_chapter == Some(chapter) {
            self.downloaded.store(true, Ordering::SeqCst);
            return Ok(ChapterContent::Book(BookFile {
                path: "/nowhere/book.pdf".into(),
                temporary: true,
            }));
        }
        Ok(ChapterContent::Images(Self::pages(chapter, round)))
    }

    /// As a server answers: a chapter kept as a file has no pages to name without its download.
    async fn chapter_pages(
        &self,
        series: &str,
        chapter: &str,
    ) -> Result<Option<Vec<PageRef>>, ProviderError> {
        if self.file_chapter == Some(chapter) {
            return Ok(None);
        }
        match self.content(series, chapter).await? {
            ChapterContent::Images(pages) => Ok(Some(pages)),
            _ => Ok(None),
        }
    }

    async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
        self.fetched.lock().unwrap().push(page.url.clone());
        if let Some((chapter, gate)) = &self.gate
            && page.url.contains(&format!("/{chapter}/"))
        {
            gate.notified().await;
            self.passed.fetch_add(1, Ordering::SeqCst);
        }
        if let Some(chapter) = self.expires
            && page.url.starts_with(&format!("https://books.example/{chapter}/"))
            && page.url.ends_with("round=1")
        {
            return Err(PageError::Status(403));
        }
        let picture = image::GrayImage::from_pixel(320, 480, image::Luma([90]));
        let mut png = Vec::new();
        picture.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        Ok(png)
    }
}

const SERVED: &str = "a:900778";

/// A reader over a work only `server` holds, opened at its first chapter.
async fn opened(server: Arc<ShelfServer>) -> Harness {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
    h.services.book_sources.registry.put(server);
    h.services.store.set_book_match("shelf-1", "series-1", SERVED, 90, true, false, 1).unwrap();
    h.open_work(WorkId::new(SERVED), OpenAt::Start).await;
    h
}

impl Harness {
    /// Show `leaf` of the open chapter, near its end.
    fn near_the_end(&mut self, leaf: u32) {
        let session = self.session();
        let chapter = self.reader.session.as_ref().unwrap().chapter().id.clone();
        self.send(ReaderCommand::Shown { session, chapter, leaf, how: TurnHow::Turn });
    }

    fn next_chapter(&self) -> ChapterId {
        let s = self.reader.session.as_ref().unwrap();
        s.chapters[s.at + 1].id.clone()
    }

    async fn preview(&mut self) -> (ChapterId, PageFile) {
        self.until(|e| match e {
            ReaderEvent::NextPreview { chapter, file, .. } => {
                Some((chapter.clone(), file.clone()))
            }
            _ => None,
        })
        .await
    }
}

/// Near the end of a server's chapter, the next chapter is fetched ahead, a page at a time and
/// first page first, and that page is the end page's preview.
#[tokio::test(start_paused = true)]
async fn a_server_chapter_previews_the_next_one_from_its_first_page() {
    let server = Arc::new(ShelfServer::new(vec!["c1", "c2"]));
    let mut h = opened(server.clone()).await;
    let next = h.next_chapter();
    h.near_the_end(1);
    let (chapter, file) = h.preview().await;
    assert_eq!(chapter, next);
    assert_eq!((file.width, file.height), (320, 480));
    assert!(Path::new(&file.path).exists());
    assert_eq!(server.fetched_of("c2")[0], "https://books.example/c2/1?round=1");

    h.near_the_end(2);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let again = h.drain().iter().any(|e| matches!(e, ReaderEvent::NextPreview { .. }));
    assert!(!again, "made once per chapter");
    assert_eq!(
        server.fetched_of("c2"),
        (1..=3).map(|n| format!("https://books.example/c2/{n}?round=1")).collect::<Vec<_>>(),
        "each page once"
    );
}

/// A jump straight onto the end page, past the pages that would have asked for it, still
/// brings the next chapter's first page for the end page.
#[tokio::test(start_paused = true)]
async fn a_jump_onto_the_end_page_previews_the_next_chapter() {
    let server = Arc::new(ShelfServer::new(vec!["c1", "c2"]));
    let mut h = opened(server.clone()).await;
    let next = h.next_chapter();
    let session = h.session();
    let chapter = h.reader.session.as_ref().unwrap().chapter().id.clone();
    let story = h.reader.session.as_ref().unwrap().leaves.len() as u32;
    h.send(ReaderCommand::Shown { session, chapter, leaf: story, how: TurnHow::Jump });
    let (chapter, file) = h.preview().await;
    assert_eq!(chapter, next);
    assert!(Path::new(&file.path).exists());
    assert_eq!(server.fetched_of("c2")[0], "https://books.example/c2/1?round=1");
}

/// The chapter fetched ahead opens from what was fetched: Continue asks the source for nothing
/// more.
#[tokio::test(start_paused = true)]
async fn the_chapter_fetched_ahead_opens_without_asking_again() {
    let server = Arc::new(ShelfServer::new(vec!["c1", "c2", "c3"]));
    let mut h = opened(server.clone()).await;
    h.near_the_end(1);
    h.preview().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(server.fetched_of("c2").len(), 3);
    let asked = server.asked_of("c2");

    let session = h.session();
    let next = h.next_chapter();
    h.send(ReaderCommand::OpenChapter { session, chapter: next });
    let leaves = h
        .until(|e| match e {
            ReaderEvent::Leaves { leaves, .. } => Some(leaves.clone()),
            _ => None,
        })
        .await;
    assert_eq!(leaves.iter().filter(|l| !l.end_page).count(), 3);
    assert_eq!(server.fetched_of("c2").len(), 3, "no page fetched twice");
    assert_eq!(server.asked_of("c2"), asked, "the chapter was not asked for again");
}

/// A first page whose address has expired (403) is asked for again once with fresh addresses,
/// as the reader's own pages are.
#[tokio::test(start_paused = true)]
async fn an_expired_first_page_is_fetched_again_from_fresh_addresses() {
    let server =
        Arc::new(ShelfServer { expires: Some("c2"), ..ShelfServer::new(vec!["c1", "c2"]) });
    let mut h = opened(server.clone()).await;
    h.near_the_end(1);
    let (_, file) = h.preview().await;
    assert!(Path::new(&file.path).exists());
    assert_eq!(
        server.fetched_of("c2")[..2],
        ["https://books.example/c2/1?round=1", "https://books.example/c2/1?round=2"]
    );
    assert_eq!(server.asked_of("c2"), 2, "the chapter was asked for once more");
}

/// A next chapter kept on the server as a whole file is not downloaded for a preview: the end
/// page shows none.
#[tokio::test(start_paused = true)]
async fn a_chapter_only_a_download_opens_has_no_preview() {
    let server = Arc::new(ShelfServer {
        file_chapter: Some("c2"),
        ..ShelfServer::new(vec!["c1", "c2"])
    });
    let mut h = opened(server.clone()).await;
    h.near_the_end(1);
    h.near_the_end(2);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let events = h.drain();
    assert!(events.iter().any(|e| matches!(e, ReaderEvent::EndPage { .. })), "{events:?}");
    assert!(!events.iter().any(|e| matches!(e, ReaderEvent::NextPreview { .. })));
    assert!(!server.downloaded.load(Ordering::SeqCst), "nothing was downloaded");
    assert_eq!(server.asked_of("c2"), 0);
}

/// A preview still on its way holds nothing up: the reader goes on turning pages and saying
/// the end page, and the preview comes when its page does.
#[tokio::test(start_paused = true)]
async fn a_slow_preview_never_holds_up_the_reader() {
    let gate = Arc::new(tokio::sync::Notify::new());
    let server = Arc::new(ShelfServer {
        gate: Some(("c2", gate.clone())),
        ..ShelfServer::new(vec!["c1", "c2"])
    });
    let mut h = opened(server.clone()).await;
    h.near_the_end(1);
    h.near_the_end(2);
    let session = h.session();
    h.until(|e| {
        matches!(e, ReaderEvent::EndPage { session: s, .. } if *s == session).then_some(())
    })
    .await;
    assert_eq!(server.passed.load(Ordering::SeqCst), 0, "the preview's page is still waiting");
    let pending = h.drain();
    assert!(!pending.iter().any(|e| matches!(e, ReaderEvent::NextPreview { .. })));

    gate.notify_one();
    let (chapter, _) = h.preview().await;
    assert_eq!(chapter, h.next_chapter());
}

/// The next preview within `wall`, or none.
async fn preview_within(h: &mut Harness, wall: Duration) -> Option<(ChapterId, PageFile)> {
    let deadline = tokio::time::Instant::now() + wall;
    loop {
        let found = h.drain().into_iter().find_map(|e| match e {
            ReaderEvent::NextPreview { chapter, file, .. } => Some((chapter, file)),
            _ => None,
        });
        if found.is_some() || tokio::time::Instant::now() >= deadline {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Queue the next chapter for download, as Keep next chapters ready does.
fn queue_next(h: &Harness) -> i64 {
    // A queue that may run: the network is said and nothing holds it.
    h.services.book_sources.downloads.configure(crate::downloads::books::Setup::default());
    let chapter = h.next_chapter();
    let ask = kaeri_store::book_downloads::BookDownloadAsk {
        work: SERVED,
        chapter: chapter.as_str(),
        source_id: "shelf-1",
        version_label: None,
        keep_ready: true,
    };
    h.services.store.queue_book_download(&ask, 1).unwrap();
    h.services.store.book_download(SERVED, chapter.as_str()).unwrap().unwrap().id
}

/// A next chapter the download queue is fetching is not fetched ahead as well: the end page's
/// preview is made from the first page the download has kept, and the source is not asked.
#[tokio::test(start_paused = true)]
async fn a_chapter_being_downloaded_previews_from_the_download() {
    let server = Arc::new(ShelfServer::new(vec!["c1", "c2"]));
    let mut h = opened(server.clone()).await;
    let next = h.next_chapter();
    let id = queue_next(&h);
    h.services.store.book_download_progress(id, 1, Some(3), 1).unwrap();
    let mut png = Vec::new();
    image::GrayImage::from_pixel(200, 300, image::Luma([70]))
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let kept = h
        .services
        .book_sources
        .downloads
        .dir()
        .join(crate::downloads::books::safe(SERVED))
        .join(format!("{}.pages", crate::downloads::books::safe(next.as_str())));
    std::fs::create_dir_all(&kept).unwrap();
    std::fs::write(kept.join(format!("00000-{}-1.page", png.len())), &png).unwrap();
    h.near_the_end(1);
    let (chapter, file) = h.preview().await;
    assert_eq!(chapter, next);
    assert_eq!((file.width, file.height), (200, 300), "made from the page the download kept");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(server.fetched_of("c2").is_empty(), "{:?}", server.fetched_of("c2"));
}

/// A next chapter still waiting in the download queue after five seconds has the source asked
/// for its first page alone, so the end page does not wait on the queue. On the real clock: the
/// page is fetched and written on blocking threads, which paused time runs ahead of.
#[tokio::test]
async fn a_download_still_queued_has_the_source_asked_for_the_first_page() {
    let server = Arc::new(ShelfServer::new(vec!["c1", "c2"]));
    let mut h = opened(server.clone()).await;
    let next = h.next_chapter();
    queue_next(&h);
    h.near_the_end(1);
    tokio::time::sleep(Duration::from_secs(4)).await;
    let early = h.drain();
    assert!(!early.iter().any(|e| matches!(e, ReaderEvent::NextPreview { .. })), "{early:?}");
    assert!(server.fetched_of("c2").is_empty(), "the queue had its turn first");

    // Past five seconds the source is asked for the first page alone.
    let (chapter, file) = preview_within(&mut h, Duration::from_secs(10))
        .await
        .expect("the first page comes from the source");
    assert_eq!(chapter, next);
    assert_eq!((file.width, file.height), (320, 480));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        server.fetched_of("c2"),
        ["https://books.example/c2/1?round=1"],
        "one page alone"
    );
}

/// A next chapter downloaded by the time the end page nears previews from its file, and the
/// source is asked for nothing.
#[tokio::test(start_paused = true)]
async fn a_downloaded_next_chapter_previews_from_its_file() {
    let server = Arc::new(ShelfServer::new(vec!["c1", "c2"]));
    let mut h = opened(server.clone()).await;
    let folder = tempfile::tempdir().unwrap();
    for n in 1..=3 {
        let page = image::GrayImage::from_pixel(300, 450, image::Luma([60 + n]));
        page.save(folder.path().join(format!("{n:03}.png"))).unwrap();
    }
    let id = queue_next(&h);
    let path = folder.path().to_str().unwrap();
    h.services.store.book_download_done(id, path, Some(3), 1, 1).unwrap();
    h.near_the_end(1);
    let (_, file) = h.preview().await;
    assert_eq!((file.width, file.height), (300, 450), "made from the downloaded file");
    assert!(server.fetched_of("c2").is_empty());
}

/// R6-M9. A downloaded chapter opens and is laid out with no request to its source, and closing
/// it leaves none behind: the next chapter is fetched ahead only once the person moves in it.
#[tokio::test(start_paused = true)]
async fn a_downloaded_chapter_opens_without_asking_the_source() {
    let server = Arc::new(ShelfServer::new(vec!["c1", "c2"]));
    let mut h = opened(server.clone()).await;
    let first = h.reader.session.as_ref().unwrap().chapter().id.clone();
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    tokio::time::sleep(Duration::from_secs(2)).await;
    // The first chapter downloaded since, as a folder of its three pages.
    let folder = tempfile::tempdir().unwrap();
    for n in 1..=3 {
        let page = image::GrayImage::from_pixel(300, 450, image::Luma([60 + n]));
        page.save(folder.path().join(format!("{n:03}.png"))).unwrap();
    }
    let ask = kaeri_store::book_downloads::BookDownloadAsk {
        work: SERVED,
        chapter: first.as_str(),
        source_id: "shelf-1",
        version_label: None,
        keep_ready: false,
    };
    h.services.store.queue_book_download(&ask, 1).unwrap();
    let id = h.services.store.book_download(SERVED, first.as_str()).unwrap().unwrap().id;
    let path = folder.path().to_str().unwrap();
    h.services.store.book_download_done(id, path, Some(3), 1, 1).unwrap();
    let asked = |s: &ShelfServer| s.asked.lock().unwrap().values().sum::<u32>();
    let before = (asked(&server), server.fetched.lock().unwrap().len());

    for _ in 0..3 {
        h.open_work(WorkId::new(SERVED), OpenAt::Start).await;
        assert_eq!(h.reader.session.as_ref().unwrap().chapter().id, first);
        h.shown(0, TurnHow::Placed).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        let session = h.session();
        h.send(ReaderCommand::Close { session });
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    assert_eq!(
        (asked(&server), server.fetched.lock().unwrap().len()),
        before,
        "no request on opening a downloaded chapter"
    );

    // Moving in it fetches the next chapter ahead, as from a source's chapter.
    h.open_work(WorkId::new(SERVED), OpenAt::Start).await;
    h.near_the_end(1);
    let (chapter, _) = h.preview().await;
    assert_eq!(chapter, h.next_chapter());
    assert!(!server.fetched_of("c2").is_empty());
}
