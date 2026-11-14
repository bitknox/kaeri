//! A series' chapters downloading while the person reads another chapter of it: the source is
//! asked for the series' list once, not again for each open, each close or each page that is
//! slow to come, and a download picks from the list as kept.

use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, PageRef, SeriesDetail,
        SeriesHit, SourceChapter,
    },
    error::ProviderError,
    traits::PageError,
};
use kaeri_model::{medium::KaeriIsFor, reading::DownloadPick};
use kaeri_store::book_downloads::BookDownloadState;

use super::*;
use crate::downloads::books::{Lent, Setup};

const SLOW: &str = "a:900781";
const SOURCE: &str = "paced-1";
const SERIES: &str = "series-1";

/// A source of one series of four chapters of five pages. With a `gap`, every request waits its
/// turn, one per `gap` in the order asked, as a source paced per plugin does. Pages of `c2`
/// from the third on time out after 30 seconds.
struct PacedSource {
    manifest: BookSourceManifest,
    gap: Option<Duration>,
    turn: tokio::sync::Mutex<Option<tokio::time::Instant>>,
    /// The series' details asked for.
    series: AtomicU32,
    /// The series' list asked for.
    lists: AtomicU32,
    /// The list fails as an unreachable source's does.
    lists_fail: AtomicBool,
    /// This chapter's content times out, as a plugin past its deadline answers.
    content_times_out: Option<&'static str>,
    /// Every chapter whose content was asked for, in order.
    contents: Mutex<Vec<String>>,
}

impl PacedSource {
    fn new(gap: Option<Duration>) -> Self {
        Self {
            manifest: BookSourceManifest {
                id: SOURCE.into(),
                name: "Paced shelf".into(),
                version: "1".into(),
                kind: BookSourceKind::Server,
                media: vec![Medium::Manga],
                languages: vec!["en".into()],
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            },
            gap,
            turn: tokio::sync::Mutex::new(None),
            series: AtomicU32::new(0),
            lists: AtomicU32::new(0),
            lists_fail: AtomicBool::new(false),
            content_times_out: None,
            contents: Mutex::default(),
        }
    }

    async fn turn(&self) {
        let Some(gap) = self.gap else { return };
        let mut last = self.turn.lock().await;
        if let Some(at) = *last {
            tokio::time::sleep_until(at + gap).await;
        }
        *last = Some(tokio::time::Instant::now());
    }

    /// Requests for the series page: its details and its list both ask for it.
    fn series_asks(&self) -> u32 {
        self.series.load(Ordering::SeqCst) + self.lists.load(Ordering::SeqCst)
    }

    fn contents_of(&self, chapter: &str) -> usize {
        self.contents.lock().unwrap().iter().filter(|c| *c == chapter).count()
    }
}

#[async_trait]
impl kaeri_core::traits::BookSource for PacedSource {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        self.turn().await;
        self.series.fetch_add(1, Ordering::SeqCst);
        Ok(SeriesDetail::new(id, "The Salt Clock"))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        self.turn().await;
        self.lists.fetch_add(1, Ordering::SeqCst);
        if self.lists_fail.load(Ordering::SeqCst) {
            return Err(ProviderError::Transport("no route to the source".into()));
        }
        Ok(["c1", "c2", "c3", "c4"]
            .iter()
            .enumerate()
            .map(|(i, id)| SourceChapter::new(*id, (i + 1).to_string()))
            .collect())
    }

    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        self.turn().await;
        self.contents.lock().unwrap().push(chapter.to_owned());
        if self.content_times_out == Some(chapter) {
            return Err(ProviderError::Parse("plugin timed out".into()));
        }
        Ok(ChapterContent::Images(
            (1..=5)
                .map(|n| PageRef::new(format!("https://salt.example/{chapter}/{n}")))
                .collect(),
        ))
    }

    async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
        self.turn().await;
        if let Some(n) = page.url.strip_prefix("https://salt.example/c2/")
            && n.parse::<u32>().is_ok_and(|n| n > 2)
        {
            tokio::time::sleep(Duration::from_secs(30)).await;
            return Err(PageError::Failed(ProviderError::Transport("timed out".into())));
        }
        let picture = image::GrayImage::from_pixel(320, 480, image::Luma([70]));
        let mut png = Vec::new();
        picture.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        Ok(png)
    }
}

fn paced(source: Arc<PacedSource>, setup: Setup) -> Harness {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
    h.failures = true;
    h.services.book_sources.registry.put(source);
    h.services.store.set_book_match(SOURCE, SERIES, SLOW, 90, true, false, 1).unwrap();
    h.services.book_sources.downloads.configure(setup);
    h
}

impl Harness {
    fn state_of(&self, chapter: &str) -> Option<BookDownloadState> {
        self.services.store.book_download(SLOW, chapter).unwrap().map(|d| d.state)
    }

    /// The series page under the reader asks for itself again, as the apps' pages do once the
    /// reader closes.
    fn page_again(&self) {
        let kit = crate::read_catalog::Kit::of(&self.services);
        let ask = crate::read_catalog::Ask {
            answer: KaeriIsFor::Read,
            adult: false,
            chapter_notices: false,
            now: self.services.clock.now(),
        };
        let work = WorkId::new(SLOW);
        crate::read_catalog::series::load(&kit, ask, work.clone(), &self.tx);
        crate::reading::chapters_of(kit, ask, work, Some(SOURCE.into()), &self.tx);
    }
}

/// The hunt: download the next two chapters, and while the second one's pages time out, open
/// and close the first, downloaded, 50 times, the series page asking for itself after each
/// close. Says how many series requests went out once the first chapter was downloaded.
async fn hunt(gap: Option<Duration>, keep_ready: bool) -> (Arc<PacedSource>, Harness, u32) {
    let source = Arc::new(PacedSource::new(gap));
    let mut h = paced(source.clone(), Setup { keep_ready, ..Setup::default() });
    let downloads = h.services.book_sources.downloads.clone();
    let lent = Lent::of(&h.services, &h.tx);
    let work = WorkId::new(SLOW);
    downloads.download(&lent, work.clone(), DownloadPick::NextUnread { n: 2 }).await;
    while h.state_of("1") != Some(BookDownloadState::Done) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        h.drain();
    }
    let before = source.series_asks();
    for _ in 0..50 {
        h.open_work(work.clone(), OpenAt::Start).await;
        assert_eq!(h.reader.session.as_ref().unwrap().chapter().id.as_str(), "1");
        h.shown(0, TurnHow::Placed).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        let session = h.session();
        h.send(ReaderCommand::Close { session });
        h.page_again();
        tokio::time::sleep(Duration::from_secs(2)).await;
        h.drain();
    }
    // Long enough for anything left behind to run out.
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_secs(60)).await;
        h.drain();
    }
    let after = source.series_asks() - before;
    (source, h, after)
}

/// R7-M10. The second chapter's pages time out, and the first is opened and closed 50 times:
/// the source is never asked for the series again, with or without a source's pacing, and the
/// download asked for the second chapter's content once.
#[tokio::test(start_paused = true)]
async fn a_crawling_download_and_reader_opens_ask_for_the_series_once() {
    for gap in [None, Some(Duration::from_secs(4))] {
        let (source, h, after) = hunt(gap, false).await;
        assert_eq!(after, 0, "series asks after the download started, gap {gap:?}");
        assert_eq!(source.lists.load(Ordering::SeqCst), 1, "the list, once, gap {gap:?}");
        assert_eq!(source.contents_of("c2"), 1, "gap {gap:?}");
        assert_eq!(h.state_of("2"), Some(BookDownloadState::Failed));
    }
}

/// A chapter kept ready whose download failed is not queued again by every open of the chapter
/// before it: it waits a while that grows with each failure.
#[tokio::test(start_paused = true)]
async fn a_failed_chapter_kept_ready_is_not_fetched_again_on_every_open() {
    let (source, _h, after) = hunt(Some(Duration::from_secs(4)), true).await;
    assert_eq!(after, 0);
    assert!(source.contents_of("c2") <= 2, "asked {} times", source.contents_of("c2"));
}

/// A content request that times out fails the chapter: the source is not asked for the whole
/// series on top.
#[tokio::test(start_paused = true)]
async fn a_chapter_that_times_out_does_not_list_the_series_again() {
    let mut source = PacedSource::new(None);
    source.content_times_out = Some("c1");
    let source = Arc::new(source);
    let mut h = paced(source.clone(), Setup::default());
    let lent = Lent::of(&h.services, &h.tx);
    let downloads = h.services.book_sources.downloads.clone();
    let pick = DownloadPick::Chapters { ids: vec![ChapterId::new("1")] };
    downloads.download(&lent, WorkId::new(SLOW), pick).await;
    while h.state_of("1") != Some(BookDownloadState::Failed) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        h.drain();
    }
    assert_eq!(source.lists.load(Ordering::SeqCst), 1);
    assert_eq!(source.contents_of("c1"), 1);
}

/// R8-2. A download picks from the list the series page shows, without asking the source for
/// it again; with that list old and the source not answering, it still queues from it, and the
/// list stays as it was.
#[tokio::test(start_paused = true)]
async fn a_download_picks_from_the_kept_list_and_a_failed_refresh_keeps_it() {
    let source = Arc::new(PacedSource::new(None));
    let mut h = paced(source.clone(), Setup { hold: true, ..Setup::default() });
    let lent = Lent::of(&h.services, &h.tx);
    let downloads = h.services.book_sources.downloads.clone();
    let sources = h.services.book_sources.clone();
    let work = WorkId::new(SLOW);
    let now = h.services.clock.now();

    sources.chapters(&work, None, now).await.unwrap();
    source.lists_fail.store(true, Ordering::SeqCst);
    downloads.download(&lent, work.clone(), DownloadPick::NextUnread { n: 1 }).await;
    assert_eq!(source.lists.load(Ordering::SeqCst), 1, "the fresh list serves");
    assert_eq!(h.state_of("1"), Some(BookDownloadState::Queued));

    // Two hours on, the list is old and the source does not answer.
    source.lists_fail.store(false, Ordering::SeqCst);
    sources.chapters(&work, None, now - 7200).await.unwrap();
    source.lists_fail.store(true, Ordering::SeqCst);
    downloads.download(&lent, work.clone(), DownloadPick::NextUnread { n: 1 }).await;
    assert_eq!(source.lists.load(Ordering::SeqCst), 3, "the old list is asked again");
    assert_eq!(h.state_of("2"), Some(BookDownloadState::Queued));
    h.drain();
    assert!(h.failed.is_empty(), "{:?}", h.failed);
    let kept = sources.kept_listing(&work, now, i64::MAX).expect("the list is still kept");
    assert_eq!(crate::downloads::books::flatten(kept.1).len(), 4);
    assert_eq!(
        sources.listing_of(SOURCE, SERIES),
        Some(kaeri_model::reading::EditionListing::Listed)
    );
}

/// A download with no list kept and a source that does not answer queues nothing and says why.
#[tokio::test(start_paused = true)]
async fn a_download_that_cannot_list_says_why() {
    let source = Arc::new(PacedSource::new(None));
    source.lists_fail.store(true, Ordering::SeqCst);
    let mut h = paced(source.clone(), Setup { hold: true, ..Setup::default() });
    let lent = Lent::of(&h.services, &h.tx);
    let downloads = h.services.book_sources.downloads.clone();
    downloads.download(&lent, WorkId::new(SLOW), DownloadPick::NextUnread { n: 2 }).await;
    h.drain();
    assert_eq!(h.failed.len(), 1);
    assert_eq!(h.state_of("1"), None);
}
