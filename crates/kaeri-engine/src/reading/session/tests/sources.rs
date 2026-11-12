//! A work read from a source: its chapters in reading order whatever order the source lists
//! them in, one list with the chapters downloaded, the list asked for once, and a chapter of
//! pictures shown as soon as the page it opens at has landed.

use std::{
    collections::HashSet,
    io::Cursor,
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
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
use tokio::time::timeout;

use super::*;

const READ: &str = "a:900779";
const SOURCE: &str = "pages-1";

/// A source holding one series: its chapters, given in reading order and listed newest first
/// when it says so, each of three pages of pictures.
struct PageSource {
    manifest: BookSourceManifest,
    labels: Vec<&'static str>,
    newest_first: bool,
    /// How many times the whole list was asked for.
    listed: AtomicU32,
    /// Every chapter whose content was asked for, in order.
    opened: Mutex<Vec<String>>,
    /// Pages from this one on wait for a permit before they come.
    held_from: Option<usize>,
    gate: tokio::sync::Semaphore,
    /// Pages that fail, by their place in the chapter.
    failing: Mutex<HashSet<usize>>,
    /// Pages printed as two, by their place in the chapter.
    wide: HashSet<usize>,
    /// The title every chapter is listed with.
    title: Option<&'static str>,
}

impl PageSource {
    fn new(labels: &[&'static str]) -> Self {
        Self {
            manifest: BookSourceManifest {
                id: SOURCE.into(),
                name: "Lantern pages".into(),
                version: "1".into(),
                kind: BookSourceKind::Server,
                media: vec![Medium::Manga],
                languages: vec!["en".into()],
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            },
            labels: labels.to_vec(),
            newest_first: false,
            listed: AtomicU32::new(0),
            opened: Mutex::default(),
            held_from: None,
            gate: tokio::sync::Semaphore::new(0),
            failing: Mutex::default(),
            wide: HashSet::new(),
            title: None,
        }
    }

    fn newest_first(self) -> Self {
        Self { newest_first: true, ..self }
    }

    fn listed(&self) -> u32 {
        self.listed.load(Ordering::SeqCst)
    }

    fn opened_of(&self, chapter: &str) -> usize {
        self.opened.lock().unwrap().iter().filter(|c| *c == chapter).count()
    }
}

#[async_trait]
impl kaeri_core::traits::BookSource for PageSource {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, "The Lantern Ledger"))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        self.listed.fetch_add(1, Ordering::SeqCst);
        let mut list: Vec<SourceChapter> = self
            .labels
            .iter()
            .map(|label| SourceChapter {
                title: self.title.map(str::to_owned),
                ..SourceChapter::new(format!("ch-{label}"), *label)
            })
            .collect();
        if self.newest_first {
            list.reverse();
        }
        Ok(list)
    }

    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        self.opened.lock().unwrap().push(chapter.to_owned());
        Ok(ChapterContent::Images(
            (0..3)
                .map(|n| PageRef::new(format!("https://pages.example/{chapter}/{n}")))
                .collect(),
        ))
    }

    async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
        let index: usize = page.url.rsplit('/').next().unwrap().parse().unwrap();
        if self.held_from.is_some_and(|from| index >= from) {
            self.gate.acquire().await.unwrap().forget();
        }
        if self.failing.lock().unwrap().contains(&index) {
            return Err(PageError::Status(404));
        }
        let (w, h) = if self.wide.contains(&index) { (640, 480) } else { (320, 480) };
        let picture = image::GrayImage::from_pixel(w, h, image::Luma([90]));
        let mut png = Vec::new();
        picture.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        Ok(png)
    }
}

/// A reader over a work only `source` holds.
fn reading(source: Arc<PageSource>) -> Harness {
    let h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
    h.services.book_sources.registry.put(source);
    h.services.store.set_book_match(SOURCE, "series-1", READ, 90, true, false, 1).unwrap();
    h
}

impl Harness {
    /// Open `chapter` of the work, or where it is read from, and wait for its view.
    async fn open_read(&mut self, chapter: Option<&str>) -> ReaderView {
        let work = WorkId::new(READ);
        let chapter = chapter.map(ChapterId::new);
        self.send(ReaderCommand::Open { work, chapter, at: OpenAt::Start, surface: surface() });
        self.until(|e| match e {
            ReaderEvent::Opened(view) => Some(view.as_ref().clone()),
            _ => None,
        })
        .await
    }

    /// The leaves of `chapter`, once they come.
    async fn leaves_of(&mut self, chapter: &str) -> Vec<Leaf> {
        let chapter = ChapterId::new(chapter);
        self.until(|e| match e {
            ReaderEvent::Leaves { chapter: c, leaves, .. } if *c == chapter => {
                Some(leaves.clone())
            }
            _ => None,
        })
        .await
    }

    /// The pages made or failed, waiting until `pages` of them have come.
    async fn pages_made(&mut self, pages: usize) -> Vec<(u32, bool)> {
        let mut made = Vec::new();
        while made.len() < pages {
            let page = self
                .until(|e| match e {
                    ReaderEvent::PageReady { page, .. } => Some((*page, true)),
                    ReaderEvent::PageFailed { page, .. } => Some((*page, false)),
                    _ => None,
                })
                .await;
            made.push(page);
        }
        made.sort_unstable();
        made
    }

    fn chapter_ids(&self) -> Vec<String> {
        let s = self.reader.session.as_ref().unwrap();
        s.chapters.iter().map(|c| c.id.to_string()).collect()
    }
}

/// A source that lists its newest chapter first is read from chapter 1 on, decimals and extras
/// in their place, in the order the series page lists them.
#[tokio::test(start_paused = true)]
async fn a_source_listing_newest_first_reads_from_chapter_one_on() {
    let labels = ["1", "2", "3", "3.5", "4", "Extra", "5", "6", "7", "8", "9", "10"];
    let mut h = reading(Arc::new(PageSource::new(&labels).newest_first()));
    let view = h.open_read(None).await;

    let order = ["1", "2", "3", "3.5", "4", "x/-/extra", "5", "6", "7", "8", "9", "10"];
    assert_eq!(h.chapter_ids(), order);
    let s = h.reader.session.as_ref().unwrap();
    assert_eq!(s.chapter().id.as_str(), "1", "read from the start opens chapter 1");
    assert_eq!(view.next.map(|c| c.id), Some(ChapterId::new("2")));
    assert_eq!(view.previous, None);

    let now = h.services.clock.now();
    let (_, groups) =
        h.services.book_sources.chapters(&WorkId::new(READ), None, now).await.unwrap();
    let page: Vec<String> =
        groups.iter().flat_map(|g| &g.chapters).map(|c| c.id.to_string()).collect();
    assert_eq!(page, order, "the series page lists them the same way");
}

/// A chapter opened from the series page reads the list the page asked for: crossing to the
/// next chapter and its preview ask the source for content alone.
#[tokio::test(start_paused = true)]
async fn an_open_after_the_series_page_asks_the_source_only_for_content() {
    let source = Arc::new(PageSource::new(&["1", "2", "3"]));
    let mut h = reading(source.clone());
    let now = h.services.clock.now();
    h.services.book_sources.chapters(&WorkId::new(READ), None, now).await.unwrap();
    assert_eq!(source.listed(), 1);

    h.open_read(Some("1")).await;
    h.leaves_of("1").await;
    // The end page, with its preview of chapter 2.
    h.shown(2, TurnHow::Turn).await;
    h.until(|e| matches!(e, ReaderEvent::NextPreview { .. }).then_some(())).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    h.leaves_of("2").await;

    assert_eq!(source.listed(), 1, "the list was asked for once, by the series page");
    assert_eq!(source.opened_of("ch-1"), 1, "one content call for the chapter opened");
}

/// A chapter the session's list does not hold, as the Chapters sheet shows once the series
/// page's fresh list has it, opens from that list rather than leaving the reader waiting.
#[tokio::test(start_paused = true)]
async fn a_chapter_newer_than_the_sessions_list_opens_from_the_fresh_one() {
    let mut h = reading(Arc::new(PageSource::new(&["1", "2", "3"])));
    h.open_read(Some("2")).await;
    h.leaves_of("2").await;
    // The source has a new chapter, and the series page lists it.
    h.services.book_sources.registry.put(Arc::new(PageSource::new(&["1", "2", "3", "4"])));
    let now = h.services.clock.now();
    h.services.book_sources.chapters(&WorkId::new(READ), None, now).await.unwrap();
    assert_eq!(h.chapter_ids(), ["1", "2", "3"]);

    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("4") });
    h.leaves_of("4").await;
    assert_eq!(h.chapter_ids(), ["1", "2", "3", "4"]);
    let s = h.reader.session.as_ref().unwrap();
    assert_eq!(s.chapter().id.as_str(), "4");
    assert_eq!(s.id, session, "the same session goes on");
}

/// With a chapter downloaded, the reader still reads the source's whole list: the download
/// from its file, the rest from the source, and the end page after it goes on.
#[tokio::test(start_paused = true)]
async fn a_downloaded_chapter_reads_among_the_sources_others() {
    let labels: Vec<&'static str> =
        (1..=12).map(|n| &*Box::leak(n.to_string().into_boxed_str())).collect();
    let source = Arc::new(PageSource::new(&labels).newest_first());
    let mut h = reading(source.clone());
    let store = h.services.store.clone();
    let now = h.services.clock.now();
    h.services.book_sources.chapters(&WorkId::new(READ), None, now).await.unwrap();
    let folder = tempfile::tempdir().unwrap();
    for n in 1..=3 {
        let page = image::GrayImage::from_pixel(320, 480, image::Luma([60 + n]));
        page.save(folder.path().join(format!("{n:03}.png"))).unwrap();
    }
    let ask = kaeri_store::book_downloads::BookDownloadAsk {
        work: READ,
        chapter: "2",
        source_id: SOURCE,
        version_label: None,
        keep_ready: true,
    };
    store.queue_book_download(&ask, 1).unwrap();
    let id = store.book_download(READ, "2").unwrap().unwrap().id;
    store.book_download_done(id, folder.path().to_str().unwrap(), Some(3), 1, 1).unwrap();

    let view = h.open_read(Some("2")).await;
    assert_eq!(h.chapter_ids().len(), 12);
    assert_eq!(view.next.map(|c| c.id), Some(ChapterId::new("3")));
    let s = h.reader.session.as_ref().unwrap();
    assert_eq!(s.chapter().path, folder.path(), "the download is read from its file");
    h.leaves_of("2").await;
    assert_eq!(source.opened_of("ch-2"), 0);

    // A chapter that is not downloaded opens from the source.
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("7") });
    h.leaves_of("7").await;
    assert_eq!(source.opened_of("ch-7"), 1);
}

/// A chapter from a source is laid out and its first page made while the rest are still on
/// their way; they are made as they land.
#[tokio::test]
async fn a_source_chapter_shows_its_first_page_before_the_rest_land() {
    let source = Arc::new(PageSource { held_from: Some(1), ..PageSource::new(&["1", "2"]) });
    let mut h = reading(source.clone());
    h.send(ReaderCommand::Open {
        work: WorkId::new(READ),
        chapter: None,
        at: OpenAt::Start,
        surface: surface(),
    });
    let wait = Duration::from_secs(20);
    let leaves = timeout(wait, h.leaves_of("1")).await.expect("laid out with pages to come");
    assert_eq!(leaves.iter().filter(|l| !l.end_page).count(), 3);
    let first = timeout(wait, h.pages_made(1)).await.expect("the first page is made");
    assert_eq!(first, [(0, true)]);

    source.gate.add_permits(2);
    h.shown(0, TurnHow::Turn).await;
    let rest = timeout(wait, h.pages_made(2)).await.expect("the rest are made as they land");
    assert_eq!(rest, [(1, true), (2, true)]);
}

/// A page that fails is that page's failure: the chapter opens, the other pages come, and a
/// retry fetches the page again.
#[tokio::test]
async fn a_page_that_fails_has_its_own_retry() {
    let source = Arc::new(PageSource::new(&["1"]));
    source.failing.lock().unwrap().insert(1);
    let mut h = reading(source.clone());
    h.failures = true;
    h.open_read(None).await;
    let wait = Duration::from_secs(20);
    timeout(wait, h.leaves_of("1")).await.expect("the chapter opens");
    h.shown(0, TurnHow::Turn).await;
    let made = timeout(wait, h.pages_made(3)).await.expect("every page answers");
    assert_eq!(made, [(0, true), (1, false), (2, true)]);

    source.failing.lock().unwrap().clear();
    let session = h.session();
    h.send(ReaderCommand::Retry { session, chapter: ChapterId::new("1"), page: Some(1) });
    let again = timeout(wait, h.pages_made(1)).await.expect("the retry answers");
    assert_eq!(again, [(1, true)]);
}

/// Pages that land in another shape than the first lay the chapter out again once all have
/// landed: a spread is split on a single screen as it would be from a file.
#[tokio::test]
async fn a_spread_that_lands_later_is_laid_out_once_all_have() {
    let mut source = PageSource::new(&["1"]);
    source.held_from = Some(1);
    source.wide.insert(2);
    let source = Arc::new(source);
    let mut h = reading(source.clone());
    h.open_read(None).await;
    let wait = Duration::from_secs(20);
    let first = timeout(wait, h.leaves_of("1")).await.expect("laid out with pages to come");
    assert_eq!(first.iter().filter(|l| !l.end_page).count(), 3);

    source.gate.add_permits(2);
    let again = timeout(wait, h.leaves_of("1")).await.expect("laid out again");
    assert_eq!(again.iter().filter(|l| !l.end_page).count(), 4, "the spread split in two");
}

/// A chapter the source gives a title is named in the reader as on the series page: by its
/// number, with the title beside it.
#[tokio::test(start_paused = true)]
async fn a_titled_chapter_keeps_its_number_in_the_reader() {
    let source =
        PageSource { title: Some("The Lantern Ferry"), ..PageSource::new(&["1", "2"]) };
    let mut h = reading(Arc::new(source));
    let view = h.open_read(None).await;
    assert_eq!(view.chapter.label, "1");
    assert_eq!(view.chapter.title.as_deref(), Some("The Lantern Ferry"));
    let next = view.next.unwrap();
    assert_eq!((next.label.as_str(), next.title.as_deref()), ("2", Some("The Lantern Ferry")));
}

/// A place kept in a split of the chapter with more pages, as another source's, resumes at the
/// same point of this one rather than at the same page number.
#[tokio::test(start_paused = true)]
async fn a_place_kept_on_another_split_resumes_at_the_same_point() {
    let mut h = reading(Arc::new(PageSource::new(&["1", "2"])));
    let event = kaeri_store::reading::ReadEvent {
        page: Some(4),
        of_pages: Some(12),
        ..kaeri_store::reading::ReadEvent::new(READ, "1", 1)
    };
    h.services.store.record_read(&event, Vec::new()).unwrap();
    h.send(ReaderCommand::Open {
        work: WorkId::new(READ),
        chapter: None,
        at: OpenAt::Resume,
        surface: surface(),
    });
    let view = h
        .until(|e| match e {
            ReaderEvent::Opened(view) => Some(view.as_ref().clone()),
            _ => None,
        })
        .await;
    assert_eq!(view.chapter.id.as_str(), "1");
    assert_eq!(view.start.page, Some(1), "a third of the way in, on three pages");
}

#[test]
fn a_kept_place_scales_to_the_chapters_length() {
    use crate::reading::session::rescaled;
    let at = |image, offset| Some(StripPos { image, offset });
    assert_eq!(rescaled(5, None, 28, 28), None, "the same split stays put");
    assert_eq!(rescaled(4, None, 12, 3), Some((1, None)));
    assert_eq!(rescaled(5, at(5, 0.5), 28, 56), Some((11, at(11, 0.0))));
    assert_eq!(rescaled(1, at(1, 0.75), 4, 2), Some((0, at(0, 0.875))));
    assert_eq!(rescaled(27, None, 28, 4), Some((3, None)), "never past the last page");
}

/// A place counted with every page moves past the credit pages left out before it, so it is the
/// same page; one counted in another split is scaled.
#[test]
fn a_kept_place_moves_past_the_credit_pages_left_out() {
    use crate::reading::session::{kept_past_credits, rescaled};
    let at = |image, offset| Some(StripPos { image, offset });
    assert_eq!(kept_past_credits(26, None, 36, (35, &[0])), Some((25, None)), "first page out");
    assert_eq!(kept_past_credits(26, None, 36, (35, &[35])), Some((26, None)), "last page out");
    assert_eq!(kept_past_credits(35, None, 36, (35, &[35])), Some((34, None)), "on the credit");
    assert_eq!(kept_past_credits(4, at(4, 0.5), 6, (4, &[0, 5])), Some((3, at(3, 0.5))));
    assert_eq!(kept_past_credits(26, None, 35, (35, &[0])), None, "counted without it already");
    assert_eq!(kept_past_credits(4, None, 12, (3, &[0])), rescaled(4, None, 12, 3));
}

/// Mark read up to here counts every chapter under it as read on the series page, not only the
/// ones read one by one.
#[tokio::test(start_paused = true)]
async fn chapters_under_a_read_mark_list_as_read() {
    let h = reading(Arc::new(PageSource::new(&["1", "2", "3", "3.5", "4", "5"])));
    let store = h.services.store.clone();
    store
        .record_read(
            &kaeri_store::reading::ReadEvent {
                completed: true,
                ..kaeri_store::reading::ReadEvent::new(READ, "1", 1)
            },
            Vec::new(),
        )
        .unwrap();
    store.adopt_read(READ, "3.5", None, 2).unwrap();
    let now = h.services.clock.now();
    let (_, groups) =
        h.services.book_sources.chapters(&WorkId::new(READ), None, now).await.unwrap();
    let read: Vec<(String, bool)> = groups
        .iter()
        .flat_map(|g| &g.chapters)
        .map(|c| (c.id.to_string(), c.state == kaeri_model::reading::ChapterReadState::Read))
        .collect();
    let want =
        [("1", true), ("2", true), ("3", true), ("3.5", true), ("4", false), ("5", false)];
    let want: Vec<(String, bool)> = want.iter().map(|(c, r)| (c.to_string(), *r)).collect();
    assert_eq!(read, want);
}

impl Harness {
    /// What is kept of `chapter` of the work read from the source.
    fn kept_of(&self, chapter: &str) -> Option<kaeri_store::reading::ReadChapter> {
        let store = &self.services.store;
        store.read_chapters(READ).unwrap().into_iter().find(|c| c.chapter == chapter)
    }

    fn strip_at(&mut self, chapter: &str, image: u32, offset: f32, foot_visible: bool) {
        let session = self.session();
        let chapter = ChapterId::new(chapter);
        let at = StripPos { image, offset };
        self.send(ReaderCommand::StripAt { session, chapter, at, foot_visible });
    }

    /// Let the place's timed write come and go.
    async fn settle(&mut self) {
        tokio::time::sleep(WRITE_EVERY * 2).await;
        self.drain();
    }
}

/// A strip flows on into the chapter after the one read, also from a source that lists its
/// newest first, and the place kept in an earlier chapter stays as it was.
#[tokio::test(start_paused = true)]
async fn crossing_on_in_a_strip_leaves_an_earlier_chapters_place_alone() {
    let source = PageSource::new(&["1", "2", "3", "4"]).newest_first();
    let mut h = reading(Arc::new(source));
    let store = h.services.store.clone();
    store.set_series_setting(READ, "mode", Some("strip"), true, 1).unwrap();
    let kept = kaeri_store::reading::ReadEvent {
        page: Some(2),
        of_pages: Some(3),
        strip_image: Some(2),
        strip_offset: Some(0.5),
        ..kaeri_store::reading::ReadEvent::new(READ, "1", 1)
    };
    store.record_read(&kept, Vec::new()).unwrap();

    let view = h.open_read(Some("2")).await;
    h.leaves_of("2").await;
    let next = view.next.expect("a chapter after 2").id;
    assert_eq!(next.as_str(), "3");
    h.strip_at("2", 2, 0.9, true);
    h.settle().await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: next });
    h.leaves_of("3").await;
    h.strip_at("3", 1, 0.2, false);
    h.settle().await;
    let session = h.session();
    h.send(ReaderCommand::Close { session });

    let one = h.kept_of("1").unwrap();
    assert_eq!((one.read, one.last_page, one.strip_image), (false, Some(2), Some(2)));
    assert!(h.kept_of("2").unwrap().read, "the chapter scrolled past counts as read");
    assert_eq!(h.kept_of("3").unwrap().last_page, Some(1));
}

/// Scrolling a strip within one image writes no place until the image changes or the reader
/// closes; the close keeps how far into the image it was.
#[tokio::test(start_paused = true)]
async fn a_strip_writes_its_place_when_the_image_changes() {
    let mut h = reading(Arc::new(PageSource::new(&["1", "2"])));
    h.services.store.set_series_setting(READ, "mode", Some("strip"), true, 1).unwrap();
    h.open_read(Some("1")).await;
    h.leaves_of("1").await;
    let offset = |h: &Harness| h.kept_of("1").and_then(|c| c.strip_offset).unwrap_or(-1.0);

    h.strip_at("1", 1, 0.25, false);
    h.settle().await;
    assert_eq!(offset(&h), 0.25);
    h.strip_at("1", 1, 0.5, false);
    h.settle().await;
    h.strip_at("1", 1, 0.75, false);
    h.settle().await;
    assert_eq!(offset(&h), 0.25, "not written again within the image");

    h.strip_at("1", 2, 0.125, false);
    h.settle().await;
    let kept = h.kept_of("1").unwrap();
    assert_eq!((kept.strip_image, kept.strip_offset), (Some(2), Some(0.125)));

    h.strip_at("1", 2, 0.5, false);
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    assert_eq!(offset(&h), 0.5, "the close keeps the place within the image");
}

mod lifecycle;
mod r5_audit;

/// Offline, the end page leads into the next chapter on this device, past the ones only the
/// source has, says it is downloaded, and opening it moves on as Continue does.
#[tokio::test(start_paused = true)]
async fn offline_the_end_page_leads_into_the_next_download() {
    let source = Arc::new(PageSource::new(&["1", "2", "3", "4", "5"]));
    let mut h = reading(source.clone());
    let store = h.services.store.clone();
    let now = h.services.clock.now();
    h.services.book_sources.chapters(&WorkId::new(READ), None, now).await.unwrap();
    let folders: Vec<_> = (0..2).map(|_| tempfile::tempdir().unwrap()).collect();
    for (chapter, folder) in ["2", "4"].into_iter().zip(&folders) {
        for n in 1..=3 {
            let page = image::GrayImage::from_pixel(320, 480, image::Luma([60 + n]));
            page.save(folder.path().join(format!("{n:03}.png"))).unwrap();
        }
        let ask = kaeri_store::book_downloads::BookDownloadAsk {
            work: READ,
            chapter,
            source_id: SOURCE,
            version_label: None,
            keep_ready: true,
        };
        store.queue_book_download(&ask, 1).unwrap();
        let id = store.book_download(READ, chapter).unwrap().unwrap().id;
        store.book_download_done(id, folder.path().to_str().unwrap(), Some(3), 1, 1).unwrap();
    }
    h.services.offline.store(true, Ordering::Relaxed);

    h.open_read(Some("2")).await;
    h.leaves_of("2").await;
    h.shown(2, TurnHow::Turn).await;
    let next = h
        .until(|e| match e {
            ReaderEvent::EndPage { next, .. } => Some(next.clone()),
            _ => None,
        })
        .await
        .expect("a next chapter");
    assert_eq!(next.id.as_str(), "4", "chapter 3 is only at the source");
    assert_eq!(next.download, Some(kaeri_model::reading::ChapterDownloadState::Done));

    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: next.id });
    let read = h
        .until(|e| match e {
            ReaderEvent::ChapterRead { chapter, .. } => Some(chapter.clone()),
            _ => None,
        })
        .await;
    assert_eq!(read.as_str(), "2", "moving on to the download marks the one read");
    h.leaves_of("4").await;
    assert_eq!(source.opened_of("ch-3") + source.opened_of("ch-4"), 0);
}

/// An end page said offline follows the connection: back online it leads into the chapter only
/// the source has, and offline again it says so.
#[tokio::test(start_paused = true)]
async fn the_end_page_follows_the_connection_while_it_is_open() {
    let source = Arc::new(PageSource::new(&["1", "2", "3"]));
    let mut h = reading(source.clone());
    let store = h.services.store.clone();
    let now = h.services.clock.now();
    h.services.book_sources.chapters(&WorkId::new(READ), None, now).await.unwrap();
    let folder = tempfile::tempdir().unwrap();
    for n in 1..=3 {
        let page = image::GrayImage::from_pixel(320, 480, image::Luma([60 + n]));
        page.save(folder.path().join(format!("{n:03}.png"))).unwrap();
    }
    let ask = kaeri_store::book_downloads::BookDownloadAsk {
        work: READ,
        chapter: "2",
        source_id: SOURCE,
        version_label: None,
        keep_ready: true,
    };
    store.queue_book_download(&ask, 1).unwrap();
    let id = store.book_download(READ, "2").unwrap().unwrap().id;
    store.book_download_done(id, folder.path().to_str().unwrap(), Some(3), 1, 1).unwrap();
    let end = async |h: &mut Harness| {
        h.until(|e| match e {
            ReaderEvent::EndPage { next, why_none, .. } => {
                Some((next.as_ref().map(|c| c.id.as_str().to_owned()), *why_none))
            }
            _ => None,
        })
        .await
    };
    h.network(true);
    h.open_read(Some("2")).await;
    h.leaves_of("2").await;
    h.shown(2, TurnHow::Turn).await;
    assert_eq!(end(&mut h).await, (None, Some(NoNext::Offline)), "3 is only at the source");

    h.network(false);
    assert_eq!(end(&mut h).await, (Some("3".into()), None), "online, 3 opens from the source");
    h.network(true);
    assert_eq!(end(&mut h).await, (None, Some(NoNext::Offline)), "offline again");
    // The same connection again says nothing new.
    h.network(true);
    assert!(!h.drain().iter().any(|e| matches!(e, ReaderEvent::EndPage { .. })));
}
