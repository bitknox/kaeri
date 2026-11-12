//! Round 5 engine audit: a reader over a source whose plugin hands it odd data (huge page
//! counts, empty chapters, pages that are not pictures, labels without numbers), and over
//! local books with damaged pages. The reader must open or fail, never panic or wait forever.

use std::{collections::HashMap, sync::Mutex, time::Duration};

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

const ODD: &str = "a:900790";
const ODD_SOURCE: &str = "odd-1";

/// What one of the odd source's pages is.
#[derive(Clone, Copy, Debug)]
enum Bytes {
    Png,
    /// An error page served with a 200.
    Html,
    Empty,
}

/// A source whose list and pages a test sets.
struct Odd {
    manifest: BookSourceManifest,
    list: Vec<SourceChapter>,
    /// Pages per chapter id; a chapter missing here has three.
    pages: HashMap<String, usize>,
    bytes: Mutex<HashMap<(String, usize), Bytes>>,
}

impl Odd {
    fn new(list: Vec<SourceChapter>) -> Self {
        Self {
            manifest: BookSourceManifest {
                id: ODD_SOURCE.into(),
                name: "Odd pages".into(),
                version: "1".into(),
                kind: BookSourceKind::Server,
                media: vec![Medium::Manga],
                languages: vec!["en".into()],
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            },
            list,
            pages: HashMap::new(),
            bytes: Mutex::default(),
        }
    }
}

#[async_trait]
impl kaeri_core::traits::BookSource for Odd {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }
    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }
    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, "The Quiet Archive"))
    }
    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok(self.list.clone())
    }
    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        let n = self.pages.get(chapter).copied().unwrap_or(3);
        Ok(ChapterContent::Images(
            (0..n)
                .map(|i| PageRef::new(format!("https://odd.example/{chapter}/{i}")))
                .collect(),
        ))
    }
    async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
        let mut parts = page.url.rsplit('/');
        let index: usize = parts.next().unwrap().parse().unwrap();
        let chapter = parts.next().unwrap().to_owned();
        let kind = self.bytes.lock().unwrap().get(&(chapter, index)).copied();
        Ok(match kind.unwrap_or(Bytes::Png) {
            Bytes::Png => {
                let picture = image::GrayImage::from_pixel(320, 480, image::Luma([90]));
                let mut png = Vec::new();
                picture.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
                png
            }
            Bytes::Html => {
                b"<!DOCTYPE html><html><body>Too many requests</body></html>".to_vec()
            }
            Bytes::Empty => Vec::new(),
        })
    }
}

fn odd(source: Odd) -> Harness {
    let h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
    h.services.book_sources.registry.put(Arc::new(source));
    h.services.store.set_book_match(ODD_SOURCE, "series-1", ODD, 90, true, false, 1).unwrap();
    h
}

impl Harness {
    /// Carry what comes for `wait`.
    async fn carry_for(&mut self, wait: Duration) {
        let end = tokio::time::Instant::now() + wait;
        while let Ok(Some(envelope)) = tokio::time::timeout_at(end, self.inbox.recv()).await {
            self.carry(envelope);
        }
    }

    fn open_odd(&mut self, chapter: Option<&str>, surface: ReaderSurface) {
        self.send(ReaderCommand::Open {
            work: WorkId::new(ODD),
            chapter: chapter.map(ChapterId::new),
            at: OpenAt::Start,
            surface,
        });
    }

    /// The view, or the failure, within `wait`; `None` when neither came.
    async fn opened_or_failed(
        &mut self,
        wait: Duration,
    ) -> Option<Result<ReaderView, ReadingFailure>> {
        let end = tokio::time::Instant::now() + wait;
        loop {
            if let Some(f) = self.failed.pop() {
                return Some(Err(f));
            }
            if let Some(at) = self.seen.iter().position(|e| matches!(e, ReaderEvent::Opened(_)))
            {
                let Some(ReaderEvent::Opened(view)) = self.seen.remove(at) else {
                    unreachable!()
                };
                return Some(Ok(view.as_ref().clone()));
            }
            match tokio::time::timeout_at(end, self.inbox.recv()).await {
                Ok(Some(envelope)) => self.carry(envelope),
                _ => return None,
            }
        }
    }
}

fn chapter(id: &str, label: &str) -> SourceChapter {
    SourceChapter::new(id, label)
}

/// R5E-2. A chapter in parts sums its parts' page counts as the plugin gave them
/// (`read_sources.rs` `source_row`, `sum::<Option<u32>>()`). Two parts with a count near
/// `u32::MAX` overflow: a panic in a debug or QA build, a wrong count in a release one.
#[tokio::test(start_paused = true)]
#[ignore = "R5E-2: page counts of parts overflow when summed"]
async fn parts_with_huge_page_counts_still_open() {
    let mut list =
        vec![chapter("ch-1", "1"), chapter("ch-2.1", "2.1"), chapter("ch-2.2", "2.2")];
    list[1].page_count = Some(u32::MAX);
    list[2].page_count = Some(u32::MAX);
    let mut h = odd(Odd::new(list));
    h.failures = true;
    h.open_odd(Some("1"), surface());
    let opened = h.opened_or_failed(Duration::from_secs(30)).await;
    assert!(matches!(opened, Some(Ok(_))), "the reader did not open: {opened:?}");
}

/// A chapter whose source lists no pages fails to open; it never waits.
#[tokio::test(start_paused = true)]
async fn a_chapter_of_no_pages_fails_at_once() {
    let mut source = Odd::new(vec![chapter("ch-1", "1"), chapter("ch-2", "2")]);
    source.pages.insert("ch-1".into(), 0);
    let mut h = odd(source);
    h.failures = true;
    h.open_odd(Some("1"), surface());
    let opened = h.opened_or_failed(Duration::from_secs(60)).await;
    assert!(opened.is_some(), "neither opened nor failed in a minute");
}

/// Labels with no number, odd numbers and repeated ids: the reader opens on each list, and
/// lists each id once. Each odd entry gets a list of its own beside chapters 1 and 2, so a
/// failure names the entry.
#[tokio::test(start_paused = true)]
async fn odd_labels_and_repeated_ids_open() {
    let odd_ones = [
        ("ch-x", ""),
        ("ch-a", "Extra"),
        ("ch-1", "1"),
        ("ch-big", "99999999999999999999"),
        ("ch-neg", "-1"),
        ("ch-nan", "NaN"),
        ("ch-r", "3-2"),
        ("ch-d", "0.0001"),
        ("ch-u", "\u{7B2C}3\u{8A71}"),
    ];
    let mut wrong = Vec::new();
    for (id, label) in odd_ones {
        let list = vec![chapter("ch-1", "1"), chapter("ch-2", "2"), chapter(id, label)];
        let mut h = odd(Odd::new(list));
        h.failures = true;
        h.open_odd(None, surface());
        match h.opened_or_failed(Duration::from_secs(60)).await {
            Some(Ok(_)) => {
                let s = h.reader.session.as_ref().unwrap();
                let mut ids: Vec<String> =
                    s.chapters.iter().map(|c| c.id.to_string()).collect();
                let all = ids.len();
                ids.sort();
                ids.dedup();
                if ids.len() != all {
                    wrong.push(format!("{id} {label:?}: an id twice"));
                }
            }
            Some(Err(f)) => wrong.push(format!("{id} {label:?}: failed {f:?}")),
            None => wrong.push(format!("{id} {label:?}: neither opened nor failed")),
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Pages a source answers with something that is not a picture fail with their own retry, and
/// every page is answered: none waits forever.
#[tokio::test]
async fn pages_that_are_not_pictures_each_fail_once() {
    let source = Odd::new(vec![chapter("ch-1", "1"), chapter("ch-2", "2")]);
    source.bytes.lock().unwrap().insert(("ch-1".into(), 1), Bytes::Html);
    source.bytes.lock().unwrap().insert(("ch-1".into(), 2), Bytes::Empty);
    let mut h = odd(source);
    h.failures = true;
    h.open_odd(Some("1"), surface());
    let opened = h.opened_or_failed(Duration::from_secs(30)).await;
    let Some(Ok(_)) = opened else { panic!("did not open: {opened:?}") };
    let wait = Duration::from_secs(20);
    timeout(wait, h.leaves_of("1")).await.expect("the chapter is laid out");
    h.shown(0, TurnHow::Turn).await;
    let made = timeout(Duration::from_secs(60), h.pages_made(3)).await;
    let made = made.expect("a page never came nor failed");
    assert_eq!(made, [(0, true), (1, false), (2, false)]);
}

/// The first page not a picture: the chapter fails to open, it does not hang.
#[tokio::test]
async fn a_first_page_that_is_not_a_picture_fails_the_open() {
    let source = Odd::new(vec![chapter("ch-1", "1")]);
    for i in 0..3 {
        source.bytes.lock().unwrap().insert(("ch-1".into(), i), Bytes::Html);
    }
    let mut h = odd(source);
    h.failures = true;
    h.open_odd(Some("1"), surface());
    let opened = h.opened_or_failed(Duration::from_secs(60)).await;
    assert!(opened.is_some(), "neither opened nor failed in a minute");
}

/// Chapters opened back and forth faster than their pages land: the chapter last opened gets
/// every page, and the one left and opened again does too.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chapters_opened_back_and_forth_each_get_every_page() {
    let mut source = Odd::new(vec![chapter("ch-1", "1"), chapter("ch-2", "2")]);
    source.pages.insert("ch-1".into(), 8);
    source.pages.insert("ch-2".into(), 8);
    let mut h = odd(source);
    h.failures = true;
    h.open_odd(Some("1"), surface());
    let Some(Ok(_)) = h.opened_or_failed(Duration::from_secs(30)).await else {
        panic!("did not open")
    };
    let session = h.session();
    for round in 0..20 {
        let to = if round % 2 == 0 { "2" } else { "1" };
        h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new(to) });
        h.carry_for(Duration::from_millis(5)).await;
    }
    // Last opened: "1". Every page of it comes.
    h.drain();
    let made = timeout(Duration::from_secs(60), async {
        let mut pages = std::collections::BTreeSet::new();
        while pages.len() < 8 {
            let page = h
                .until(|e| match e {
                    ReaderEvent::PageReady { chapter, page, .. } if chapter.as_str() == "1" => {
                        Some(*page)
                    }
                    _ => None,
                })
                .await;
            pages.insert(page);
            let Some(s) = h.reader.session.as_ref() else { break };
            let chapter = s.chapter().id.clone();
            let leaves: Vec<u32> = (0..8).collect();
            for leaf in leaves {
                h.send(ReaderCommand::NeedPage {
                    session,
                    chapter: chapter.clone(),
                    page: leaf,
                });
            }
        }
        pages
    })
    .await;
    assert!(made.is_ok(), "chapter 1's pages stopped coming after the back and forth");
}

/// R5E-1 end to end: a local book whose second and third pages have a picture's first bytes
/// and a header that does not read, on a two-page screen.
#[tokio::test(start_paused = true)]
#[ignore = "R5E-1: two 0x0 pages pair with a NaN place"]
async fn damaged_pages_on_a_two_page_screen_get_finite_places() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |books| {
        let path = books.join("Chapter 1.cbz");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for i in 0..4u8 {
            let bytes = if i == 1 || i == 2 {
                // A JPEG's start of image and a JFIF marker, then nothing that reads.
                let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0];
                b.extend(std::iter::repeat_n(0x5A, 400));
                b
            } else {
                let page = image::GrayImage::from_pixel(300, 450, image::Luma([40 + i * 20]));
                let mut png = Vec::new();
                page.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
                png
            };
            zip.start_file(format!("{i:03}.jpg"), stored).unwrap();
            std::io::Write::write_all(&mut zip, &bytes).unwrap();
        }
        zip.finish().unwrap();
        vec![ShelfFile {
            path: path.display().to_string(),
            chapter: None,
            label: "Chapter 1".into(),
            volume: None,
            number_milli: Some(1000),
            format: "cbz".into(),
            pages: Some(4),
            comicinfo: None,
            source: None,
        }]
    });
    h.failures = true;
    let wide = ReaderSurface { width_px: 1800, height_px: 1200, two_pages: true, ..surface() };
    h.send(ReaderCommand::Open {
        work: WorkId::new(WORK),
        chapter: None,
        at: OpenAt::Start,
        surface: wide,
    });
    let leaves = timeout(
        Duration::from_secs(30),
        h.until(|e| match e {
            ReaderEvent::Leaves { leaves, .. } => Some(leaves.clone()),
            _ => None,
        }),
    )
    .await;
    let Ok(leaves) = leaves else {
        panic!("no leaves; failures {:?}; seen {:?}", h.failed, h.seen);
    };
    for leaf in &leaves {
        for part in &leaf.parts {
            let p = &part.place;
            assert!(
                [p.x, p.y, p.width, p.height].iter().all(|v| v.is_finite()),
                "leaf {} page {} placed at {p:?}",
                leaf.index,
                part.page
            );
        }
    }
}

/// R5E-5. `read_catalog/arrange.rs` `part_word` finds "part" in the label lowercased and cuts
/// the label itself at those byte offsets (`&text[..at]`, `&text[end..]`). A letter before
/// "part" whose lowercase has another length (Turkish İ, the Kelvin sign) moves every offset
/// after it, so the cut lands inside a letter and panics, or cuts the wrong text.
#[test]
#[ignore = "R5E-5: part labels with a case-changing letter are cut at the wrong byte"]
fn part_labels_with_case_changing_letters_arrange() {
    let mut panicked = Vec::new();
    for (a, b) in [
        ("İ 5 Part 1話", "İ 5 Part 2話"),
        ("\u{212A}5 Part 1", "\u{212A}5 Part 2"),
        ("İİ 7 pt. 1/2", "İİ 7 pt. 2/2"),
    ] {
        let list = vec![chapter("ch-1", "1"), chapter("ch-5a", a), chapter("ch-5b", b)];
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::read_catalog::arrange::arrange(&list, None, None);
        }));
        let mut titled = SourceChapter::new("ch-5c", "5");
        titled.title = Some(a.to_owned());
        let titled_run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::read_catalog::arrange::merged_title(&titled);
        }));
        if run.is_err() || titled_run.is_err() {
            panicked.push(a);
        }
    }
    assert!(panicked.is_empty(), "arranging panicked on {panicked:?}");
}
