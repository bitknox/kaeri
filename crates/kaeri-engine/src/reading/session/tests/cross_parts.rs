//! One work in two sources that split it differently: one lists chapter 7 whole, the other
//! as 7.1, 7.2 and 7.3. Read marks, places, new chapters and downloads follow the chapter
//! whichever source it was read in, and a part that comes out on its own is never a chapter
//! of its own beside the whole one.

use std::{io::Cursor, sync::Mutex};

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, PageRef, SeriesDetail,
        SeriesHit, SourceChapter,
    },
    error::ProviderError,
};
use kaeri_model::reading::{ChapterReadState, ChapterRow};
use kaeri_store::{book_downloads::BookDownloadAsk, reading::EditionPref};

use super::*;

/// A source that lists the chapters it is given, three pages each.
struct Listing {
    manifest: BookSourceManifest,
    labels: Mutex<Vec<&'static str>>,
}

impl Listing {
    fn new(id: &str, labels: &[&'static str]) -> Arc<Self> {
        let manifest = BookSourceManifest {
            id: id.into(),
            name: id.into(),
            version: "1".into(),
            kind: BookSourceKind::Plugin,
            media: vec![Medium::Manga],
            languages: vec!["en".into()],
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        Arc::new(Self { manifest, labels: Mutex::new(labels.to_vec()) })
    }

    fn lists(&self, labels: &[&'static str]) {
        *self.labels.lock().unwrap() = labels.to_vec();
    }
}

#[async_trait]
impl kaeri_core::traits::BookSource for Listing {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, id))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        let labels = self.labels.lock().unwrap().clone();
        Ok(labels
            .into_iter()
            .map(|label| SourceChapter::new(format!("{}-{label}", self.manifest.id), label))
            .collect())
    }

    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        Ok(ChapterContent::Images(
            (1..=3)
                .map(|n| PageRef::new(format!("https://pages.example/{chapter}/{n}")))
                .collect(),
        ))
    }

    async fn fetch_page(
        &self,
        page: &PageRef,
    ) -> Result<Vec<u8>, kaeri_core::traits::PageError> {
        let shade = 30 + (page.url.len() as u8 % 7) * 20;
        let picture = image::GrayImage::from_pixel(300, 450, image::Luma([shade]));
        let mut png = Vec::new();
        picture.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        Ok(png)
    }
}

/// A reader over [`WORK`] held by each source, reading from the first.
fn harness_with(sources: &[&Arc<Listing>]) -> Harness {
    let h =
        harness_signed(Store::open_in_memory().unwrap(), "manga", |_| Vec::new(), None, true);
    for (i, source) in sources.iter().enumerate() {
        h.services
            .book_sources
            .registry
            .put(Arc::clone(*source) as Arc<dyn kaeri_core::traits::BookSource>);
        let id = source.manifest.id.as_str();
        h.services
            .store
            .set_book_match(id, &format!("s-{id}"), WORK, 90, true, false, 1)
            .unwrap();
        if i == 0 {
            read_from(&h, id);
        }
    }
    h
}

fn read_from(h: &Harness, source: &str) {
    let pref = EditionPref { source_id: source.into(), version_label: None, language: None };
    h.services.store.set_edition_pref(WORK, Some(&pref)).unwrap();
}

/// The rows the series page lists from `source`.
async fn rows(h: &Harness, source: &str) -> Vec<ChapterRow> {
    let work = WorkId::new(WORK);
    let (_, mut volumes) =
        h.services.book_sources.chapters(&work, Some(source), 1).await.unwrap();
    let kit = crate::read_catalog::Kit::of(&h.services);
    crate::read_catalog::series::finish(&kit, &work, &mut volumes, 1);
    assert!(volumes.len() <= 1 && volumes.iter().all(|v| v.flat), "volumes are off: one list");
    volumes.into_iter().flat_map(|v| v.chapters).collect()
}

fn shown(rows: &[ChapterRow]) -> Vec<(&str, u32, &'static str)> {
    rows.iter()
        .map(|r| {
            let state = match r.state {
                ChapterReadState::Read => "read",
                ChapterReadState::InProgress { .. } => "part",
                ChapterReadState::Unread => "",
            };
            (r.id.as_str(), r.parts, state)
        })
        .collect()
}

/// Open `chapter` and read it to its end.
async fn read_through(h: &mut Harness, chapter: &str) {
    let work = WorkId::new(WORK);
    let chapter_id = Some(ChapterId::new(chapter));
    h.send(ReaderCommand::Open {
        work,
        chapter: chapter_id,
        at: OpenAt::Start,
        surface: surface(),
    });
    let leaves = h
        .until(|e| match e {
            ReaderEvent::Leaves { leaves, .. } => Some(leaves.clone()),
            _ => None,
        })
        .await;
    let last = leaves.iter().rposition(|l| !l.end_page).unwrap() as u32;
    h.shown(last, TurnHow::Turn).await;
    let read = h
        .until(|e| match e {
            ReaderEvent::ChapterRead { chapter, .. } => Some(chapter.clone()),
            _ => None,
        })
        .await;
    assert_eq!(read.as_str(), chapter);
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    // What this open said and was not waited for is not the next open's.
    h.drain();
}

fn tracked(h: &Harness) -> Vec<u32> {
    let ops = h.services.store.read_outbox("anilist").unwrap();
    ops.iter()
        .filter_map(|op| match op {
            TrackOp::SetReadProgress { chapter, .. } => Some(*chapter),
            _ => None,
        })
        .collect()
}

fn next(h: &Harness, source: &str) -> Option<String> {
    let list =
        crate::read_catalog::chapters::kept(&h.services.store, source, &format!("s-{source}"))
            .map(|k| k.0)
            .unwrap_or_default();
    let read = crate::read_catalog::chapters::Read::of(&h.services.store, WORK);
    read.next(&list).map(|k| k.id.clone())
}

/// (a) A lists 7, 8 and 9 whole, B as 7.1 to 7.3 and 8.1 to 8.2. A chapter read in one is
/// read in the other, what comes next is the same chapter in both, and the tracker counts
/// whole chapters.
#[tokio::test(start_paused = true)]
async fn a_chapter_read_whole_in_one_source_is_read_in_parts_in_the_other() {
    let a = Listing::new("alpha", &["7", "8", "9"]);
    let b = Listing::new("beta", &["7.1", "7.2", "7.3", "8.1", "8.2"]);
    let mut h = harness_with(&[&a, &b]);
    assert_eq!(shown(&rows(&h, "beta").await)[..2], [("7", 3, ""), ("8", 2, "")]);
    rows(&h, "alpha").await;

    read_through(&mut h, "7").await;
    assert_eq!(tracked(&h).last(), Some(&7));
    assert_eq!(shown(&rows(&h, "beta").await)[..2], [("7", 3, "read"), ("8", 2, "")]);
    assert_eq!(next(&h, "beta").as_deref(), Some("8"));
    assert_eq!(next(&h, "alpha").as_deref(), Some("8"));

    // B's 8, both parts, is A's 8.
    read_from(&h, "beta");
    read_through(&mut h, "8").await;
    let a_rows = rows(&h, "alpha").await;
    assert_eq!(shown(&a_rows), vec![("7", 1, "read"), ("8", 1, "read"), ("9", 1, "")]);
    assert_eq!(next(&h, "alpha").as_deref(), Some("9"));
    let b_rows = rows(&h, "beta").await;
    let ids: Vec<&str> = b_rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["7", "8", "9"], "A's 9 fills B's list once, never a part twice");
    assert_eq!(tracked(&h).last(), Some(&8));
}

/// (b) B has put out only 7.1 of the 7 A lists whole. It is 7's first part, read where 7 is
/// read and never new or next beside it; when 7.2 comes it joins the same row.
#[tokio::test(start_paused = true)]
async fn a_first_part_out_alone_is_the_whole_chapter_another_source_lists() {
    let a = Listing::new("alpha", &["6", "7", "8"]);
    let b = Listing::new("beta", &["6", "7.1"]);
    let mut h = harness_with(&[&a, &b]);
    rows(&h, "alpha").await;
    rows(&h, "beta").await;
    read_through(&mut h, "6").await;
    read_through(&mut h, "7").await;

    let listed = rows(&h, "beta").await;
    assert_eq!(shown(&listed), vec![("6", 1, "read"), ("7", 1, "read"), ("8", 1, "")]);
    assert!(!listed[1].new_chapter);
    // B lists nothing after 7, and never 7.1: A's 8 is next.
    assert_eq!(next(&h, "beta"), None);
    assert_eq!(next(&h, "alpha").as_deref(), Some("8"));

    b.lists(&["6", "7.1", "7.2", "8"]);
    let listed = rows(&h, "beta").await;
    assert_eq!(shown(&listed), vec![("6", 1, "read"), ("7", 2, "read"), ("8", 1, "")]);
    assert!(!listed[1].new_chapter, "7 is not new again");
    assert_eq!(tracked(&h).last(), Some(&7));
}

/// (b) With no other source to say so, a lone 7.1 is a chapter of its own, as a lone 10.5 is.
/// Read under its own number, it is not lost when 7.2 comes and the two read as 7: 7 is part
/// read, from the page after 7.1's last. How long 7.2 is is not known until it is opened.
#[tokio::test(start_paused = true)]
async fn a_part_read_under_its_own_number_counts_toward_its_chapter_once_merged() {
    let b = Listing::new("beta", &["6", "7.1"]);
    let mut h = harness_with(&[&b]);
    assert_eq!(shown(&rows(&h, "beta").await), vec![("6", 1, ""), ("7.1", 1, "")]);
    read_through(&mut h, "7.1").await;

    b.lists(&["6", "7.1", "7.2"]);
    let listed = rows(&h, "beta").await;
    assert_eq!(listed.len(), 2, "no 7.1 row beside 7");
    assert_eq!(listed[1].id.as_str(), "7");
    assert!(
        matches!(listed[1].state, ChapterReadState::InProgress { .. }),
        "{:?}",
        listed[1].state
    );
    assert!(!listed[1].new_chapter, "7 is the chapter already listed, not a new one");
    let read = crate::read_catalog::chapters::Read::of(&h.services.store, WORK);
    assert_eq!(read.in_progress.as_ref().map(|p| (p.0.as_str(), p.1)), Some(("7", Some(3))));

    // Resumed, 7 opens on 7.2's first page.
    let (view, _, _) = h.open(OpenAt::Resume).await;
    assert_eq!((view.chapter.id.as_str(), view.start.page), ("7", Some(3)));
}

/// (c) Half of B's 7 (7.1 and 7.2) read, then A's 7 opened: the place carries over, scaled to
/// A's pages.
#[tokio::test(start_paused = true)]
async fn a_place_in_one_sources_split_carries_to_the_other() {
    let a = Listing::new("alpha", &["7", "8"]);
    let b = Listing::new("beta", &["7.1", "7.2", "8"]);
    let mut h = harness_with(&[&b, &a]);
    rows(&h, "alpha").await;
    assert_eq!(shown(&rows(&h, "beta").await)[0], ("7", 2, ""));
    let (view, leaves, _) = h.open(OpenAt::Resume).await;
    assert_eq!((view.chapter.id.as_str(), leaves.len()), ("7", 7));
    h.shown(2, TurnHow::Turn).await;
    h.shown(3, TurnHow::Turn).await;
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    h.synced();
    assert_eq!(h.kept("7").and_then(|k| k.last_page), Some(3));

    read_from(&h, "alpha");
    let (view, _, _) = h.open(OpenAt::Resume).await;
    assert_eq!(view.chapter.id.as_str(), "7");
    // Page 4 of 6 is about page 2 of A's 3.
    assert_eq!(view.start.page, Some(1));
}

/// (d) A lone 7.1 downloaded is not left as a chapter "gone from source" once 7.2 comes: 7 is
/// queued in its place. A download of 7 made when it had one part is fetched again with both.
#[tokio::test(start_paused = true)]
async fn downloads_follow_a_part_into_its_chapter() {
    let b = Listing::new("beta", &["6", "7.1"]);
    let h = harness_with(&[&b]);
    rows(&h, "beta").await;
    let store = &h.services.store;
    let done = |chapter: &str| {
        let ask = BookDownloadAsk {
            work: WORK,
            chapter,
            source_id: "beta",
            version_label: None,
            keep_ready: false,
        };
        store.queue_book_download(&ask, 1).unwrap();
        let id = store.book_download(WORK, chapter).unwrap().unwrap().id;
        store.book_download_done(id, "/books/beta.cbz", Some(3), 10, 1).unwrap();
    };
    done("7.1");
    b.lists(&["6", "7.1", "7.2"]);
    let listed = rows(&h, "beta").await;
    assert!(listed.iter().all(|r| !r.gone_from_source), "7.1 is 7's, not gone");
    let queued = store.book_download(WORK, "7").unwrap().expect("7 queued");
    assert_eq!(queued.state, kaeri_store::book_downloads::BookDownloadState::Queued);

    // A whole 7 elsewhere makes the lone 7.1 the chapter 7 from the start.
    let a = Listing::new("alpha", &["6", "7"]);
    let b = Listing::new("beta", &["6", "7.1"]);
    let h = harness_with(&[&a, &b]);
    rows(&h, "alpha").await;
    rows(&h, "beta").await;
    let store = &h.services.store;
    let ask = BookDownloadAsk {
        work: WORK,
        chapter: "7",
        source_id: "beta",
        version_label: None,
        keep_ready: false,
    };
    store.queue_book_download(&ask, 1).unwrap();
    let id = store.book_download(WORK, "7").unwrap().unwrap().id;
    store.book_download_done(id, "/books/beta-7.cbz", Some(3), 10, 1).unwrap();
    b.lists(&["6", "7.1", "7.2"]);
    read_from(&h, "beta");
    rows(&h, "beta").await;
    let again = store.book_download(WORK, "7").unwrap().unwrap();
    assert_eq!(again.state, kaeri_store::book_downloads::BookDownloadState::Queued);
    assert_eq!(again.pages_total, None, "fetched again with its new part");
}

/// Extras one source numbers 7.5 and 7.6 and another 7.1 and 7.2 are the same extras under other
/// numbers: the source in use lists its own, and the other's never fill in beside them. Where
/// the source in use has none for a chapter, or numbers them the same way, the other's fill in.
#[tokio::test(start_paused = true)]
async fn extras_numbered_differently_elsewhere_never_fill_in_beside_the_ones_here() {
    let a = Listing::new("alpha", &["5", "5.5", "6", "7", "7.5", "7.6", "8", "10", "10.5"]);
    let b = Listing::new(
        "beta",
        &["5", "5.1", "6", "7", "7.1", "7.2", "8", "9", "9.5", "10", "10.5", "10.6"],
    );
    let h = harness_with(&[&a, &b]);
    rows(&h, "beta").await;
    let listed = rows(&h, "alpha").await;
    let ids: Vec<&str> = listed.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["5", "5.5", "6", "7", "7.5", "7.6", "8", "9", "9.5", "10", "10.5", "10.6"]
    );
    let filled: Vec<&str> =
        listed.iter().filter(|r| r.other_source.is_some()).map(|r| r.id.as_str()).collect();
    assert_eq!(filled, vec!["9", "9.5", "10.6"]);
}
