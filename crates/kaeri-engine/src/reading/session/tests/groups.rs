//! Chapters as a source lists them: in parts, from several groups, and with a group's credit
//! page at their end.

use std::io::Cursor;

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, PageRef, SeriesDetail,
        SeriesHit, SourceChapter,
    },
    error::ProviderError,
};

use kaeri_model::{
    reader::ReaderValue,
    reading::{ChapterReadState, ChapterRow},
};

use super::*;

/// A source whose list has a chapter in two parts, chapters from two groups, and pages that
/// end with the group's credit page when `credits` is set.
struct Groups {
    manifest: BookSourceManifest,
    credits: bool,
}

impl Groups {
    fn new(credits: bool) -> Arc<Self> {
        let manifest = BookSourceManifest {
            id: "groups".into(),
            name: "Groups".into(),
            version: "1".into(),
            kind: BookSourceKind::Plugin,
            media: vec![Medium::Manga],
            languages: vec!["en".into()],
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        Arc::new(Self { manifest, credits })
    }
}

/// The list, in number order as sources give it: Group A has 1 (in two parts), 2, 3 and 5;
/// Group B has 1, 2 and 4.
fn listed() -> Vec<SourceChapter> {
    [
        ("a1a", "Chapter 1 part 1", "Group A"),
        ("a1b", "Chapter 1 part 2", "Group A"),
        ("b1", "Chapter 1", "Group B"),
        ("a2", "Chapter 2", "Group A"),
        ("b2", "Chapter 2", "Group B"),
        ("a3", "Chapter 3", "Group A"),
        ("b4", "Chapter 4", "Group B"),
        ("a5", "Chapter 5", "Group A"),
    ]
    .into_iter()
    .map(|(id, label, group)| {
        let mut c = SourceChapter::new(id, label);
        c.version_label = Some(group.into());
        c
    })
    .collect()
}

/// A page of blocks laid out by `seed`: seed 0 is the group's credit page.
fn blocks(seed: u32) -> Vec<u8> {
    let (w, h) = (300u32, 450u32);
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(977);
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let mut page = image::GrayImage::from_pixel(w, h, image::Luma([248]));
    for _ in 0..7 {
        let (x0, y0) = (next() % (w - 60), next() % (h - 80));
        let (x1, y1) = (x0 + 30 + next() % 60, y0 + 40 + next() % 80);
        let tone = (next() % 200) as u8;
        for y in y0..y1.min(h) {
            for x in x0..x1.min(w) {
                page.put_pixel(x, y, image::Luma([tone]));
            }
        }
    }
    let mut png = Vec::new();
    page.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    png
}

#[async_trait]
impl kaeri_core::traits::BookSource for Groups {
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
        Ok(listed())
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
        // A flat grey of its own for each page, or blocks with the credit page last.
        let n = page.url.bytes().last().map_or(1, |b| u32::from(b - b'0'));
        if self.credits {
            let chapter = page.url.bytes().map(u32::from).sum::<u32>();
            return Ok(blocks(if n == 3 { 0 } else { chapter * 10 + n }));
        }
        let shade = 30 + (page.url.len() as u8 % 7) * 20 + n as u8 * 10;
        let picture = image::GrayImage::from_pixel(300, 450, image::Luma([shade]));
        let mut png = Vec::new();
        picture.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        Ok(png)
    }
}

fn harness_with(source: Arc<Groups>, signed_in: bool) -> Harness {
    let h = harness_signed(
        Store::open_in_memory().unwrap(),
        "manga",
        |_| Vec::new(),
        None,
        signed_in,
    );
    h.services.book_sources.registry.put(source);
    h.services.store.set_book_match("groups", "series-1", WORK, 90, true, false, 1).unwrap();
    h
}

async fn rows(h: &Harness) -> Vec<ChapterRow> {
    let work = WorkId::new(WORK);
    let (_, volumes) = h.services.book_sources.chapters(&work, None, 1).await.unwrap();
    volumes.into_iter().flat_map(|v| v.chapters).collect()
}

/// A chapter listed in two parts is one row, opens as one chapter running through both parts,
/// and counts read once, with the tracker, only past its last part.
#[tokio::test(start_paused = true)]
async fn a_chapter_in_parts_reads_through_as_one_and_counts_once() {
    let mut h = harness_with(Groups::new(false), true);
    let rows = rows(&h).await;
    let first = &rows[0];
    assert_eq!((first.id.as_str(), first.parts, first.label.as_str()), ("1", 2, "Chapter 1"));
    assert_eq!(rows.iter().filter(|r| r.id.as_str() == "1").count(), 1);

    let (_, leaves, _) = h.open(OpenAt::Start).await;
    assert_eq!(leaves.len(), 7, "both parts' six pages and the end page");
    h.shown(3, TurnHow::Turn).await;
    h.shown(5, TurnHow::Turn).await;
    let read = h
        .until(|e| match e {
            ReaderEvent::ChapterRead { chapter, .. } => Some(chapter.clone()),
            _ => None,
        })
        .await;
    assert_eq!(read.as_str(), "1");
    let kept: Vec<String> =
        h.services.store.read_chapters(WORK).unwrap().into_iter().map(|c| c.chapter).collect();
    assert_eq!(kept, vec!["1".to_owned()]);
    let ops = h.services.store.read_outbox("anilist").unwrap();
    assert!(
        matches!(ops.as_slice(), [TrackOp::SetReadProgress { chapter: 1, .. }]),
        "one chapter for both parts: {ops:?}"
    );
}

/// The list is the preferred group's, with another group's chapter where it has none; a pick
/// of the other group lists from that one, and read marks stay on the chapter.
#[tokio::test(start_paused = true)]
async fn the_group_picked_lists_its_chapters_and_the_read_marks_stay() {
    let h = harness_with(Groups::new(false), false);
    let listed = rows(&h).await;
    let ids: Vec<&str> = listed.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["1", "2", "3", "4", "5"]);
    let from: Vec<(Option<&str>, bool)> =
        listed.iter().map(|r| (r.version_label.as_deref(), r.other_group)).collect();
    assert_eq!(from[1], (Some("Group A"), false), "the group with the most chapters");
    assert_eq!(from[3], (Some("Group B"), true), "chapter 4 only Group B has, marked");
    assert_eq!(listed[1].versions, 2);
    assert!(listed[3].line.iter().any(|m| matches!(
        m,
        Msg::Reading(ReadingMsg::ReadingRowFromGroup { group }) if group == "Group B"
    )));
    let work = WorkId::new(WORK);
    let editions = h.services.book_sources.editions(&work);
    let groups: Vec<(&str, u32, bool)> =
        editions[0].groups.iter().map(|g| (g.label.as_str(), g.chapters, g.in_use)).collect();
    assert_eq!(groups, vec![("Group A", 4, true), ("Group B", 3, false)]);
    assert!(!editions[0].group_chosen);

    let mut event = ReadEvent::new(WORK, "2", 5);
    event.completed = true;
    h.services.store.record_read(&event, Vec::new()).unwrap();
    crate::reading::choose_group(&h.services, &work, Some("Group B".into()));
    let picked = rows(&h).await;
    let ids: Vec<&str> = picked.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["1", "2", "3", "4", "5"], "each chapter keeps its place");
    assert_eq!((picked[0].version_label.as_deref(), picked[0].parts), (Some("Group B"), 1));
    assert_eq!(
        (picked[2].version_label.as_deref(), picked[2].other_group),
        (Some("Group A"), true)
    );
    assert!(matches!(picked[1].state, ChapterReadState::Read), "read in Group A, read here");
    assert!(h.services.book_sources.editions(&work)[0].group_chosen);
    // The group choice is the edition choice: its source stays the one in use.
    let pref = h.services.store.edition_pref(WORK).unwrap().unwrap();
    assert_eq!(
        (pref.source_id.as_str(), pref.version_label.as_deref()),
        ("groups", Some("Group B"))
    );

    // Back to the group with the most chapters.
    crate::reading::choose_group(&h.services, &work, None);
    assert_eq!(rows(&h).await[0].version_label.as_deref(), Some("Group A"));
}

/// The page every chapter ends on is learnt as the group's credit page by the third chapter,
/// left out of its pages and said; the person can keep it, and then it stays.
#[tokio::test(start_paused = true)]
async fn the_third_chapter_ending_on_the_credit_page_skips_it_until_kept() {
    let mut h = harness_with(Groups::new(true), false);
    rows(&h).await;
    let (_, leaves, events) = h.open(OpenAt::Start).await;
    assert_eq!(leaves.len(), 7, "the first chapter is whole: one chapter is no pattern");
    assert!(!events.iter().any(|e| matches!(e, ReaderEvent::CreditPages { .. })));
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    let leaves = h.until(|e| match e {
        ReaderEvent::Leaves { leaves, .. } => Some(leaves.clone()),
        _ => None,
    });
    assert_eq!(leaves.await.len(), 4, "two chapters are not enough");
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("3") });
    let leaves = h
        .until(|e| match e {
            ReaderEvent::Leaves { leaves, .. } => Some(leaves.clone()),
            _ => None,
        })
        .await;
    assert_eq!(leaves.len(), 3, "two pages and the end page: the third is the credit page");
    let (pages, msg) = h
        .until(|e| match e {
            ReaderEvent::CreditPages { pages, msg, .. } => Some((pages.clone(), msg.clone())),
            _ => None,
        })
        .await;
    assert_eq!(msg, Msg::Reading(ReadingMsg::ReaderCreditPagesSkipped { count: 1 }));
    assert_eq!((pages.len(), pages[0].page, pages[0].at_end), (1, 2, true));
    assert!(Path::new(&pages[0].file.path).exists(), "the page is a tap away");

    h.send(ReaderCommand::NotACreditPage { session, fingerprint: pages[0].fingerprint });
    let leaves = h
        .until(|e| match e {
            ReaderEvent::Leaves { leaves, .. } => Some(leaves.clone()),
            _ => None,
        })
        .await;
    assert_eq!(leaves.len(), 4, "kept: the page is back");
}

/// With Skip credit pages off, a chapter keeps every page.
#[tokio::test(start_paused = true)]
async fn credit_pages_stay_when_the_setting_is_off() {
    let mut h = harness_with(Groups::new(true), false);
    rows(&h).await;
    h.send(ReaderCommand::SetSetting {
        session: None,
        medium: Medium::Manga,
        work: Some(WorkId::new(WORK)),
        key: Key::SkipCreditPages,
        scope: kaeri_model::reader::SettingScope::Medium,
        value: ReaderValue::Toggle { on: false },
    });
    h.open(OpenAt::Start).await;
    let session = h.session();
    for chapter in ["2", "3"] {
        h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new(chapter) });
        let leaves = h
            .until(|e| match e {
                ReaderEvent::Leaves { leaves, .. } => Some(leaves.clone()),
                _ => None,
            })
            .await;
        assert_eq!(leaves.len(), 4, "chapter {chapter} whole");
    }
    assert!(!h.drain().iter().any(|e| matches!(e, ReaderEvent::CreditPages { .. })));
}
