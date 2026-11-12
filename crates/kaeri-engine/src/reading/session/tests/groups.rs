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
    /// Each chapter opens on a credit page of its own too, and has this many pages.
    opening: Option<u32>,
    /// A chapter whose last page never comes.
    stuck: Option<&'static str>,
}

impl Groups {
    /// Five pages a chapter, the first a recruitment page and the last the group's credits.
    fn both_ends() -> Self {
        let mut source = Arc::into_inner(Self::new(true)).unwrap();
        source.opening = Some(5);
        source
    }

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
        Arc::new(Self { manifest, credits, opening: None, stuck: None })
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
            (1..=self.opening.unwrap_or(3))
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
        let last = self.opening.unwrap_or(3);
        if let Some(chapter) = self.stuck
            && n == last
            && page.url.contains(&format!("/{chapter}/"))
        {
            std::future::pending::<()>().await;
        }
        if self.credits {
            let chapter = page.url.bytes().map(u32::from).sum::<u32>();
            let opening = self.opening.is_some() && n == 1;
            return Ok(blocks(match n {
                _ if n == last => 0,
                _ if opening => 1,
                _ => chapter * 10 + n,
            }));
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
    let (_, mut volumes) = h.services.book_sources.chapters(&work, None, 1).await.unwrap();
    let kit = crate::read_catalog::Kit::of(&h.services);
    crate::read_catalog::series::finish(&kit, &work, &mut volumes, 1);
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

    let (_, leaves, after) = h.open(OpenAt::Start).await;
    assert_eq!(leaves.len(), 7, "both parts' six pages and the end page");
    let marks = after
        .iter()
        .find_map(|e| match e {
            ReaderEvent::ChapterParts { parts, total, missing, .. } => {
                Some((parts.clone(), *total, missing.clone()))
            }
            _ => None,
        })
        .expect("the parts are told");
    let starts: Vec<(u32, u32, bool)> =
        marks.0.iter().map(|p| (p.number, p.first_page, p.other_group)).collect();
    assert_eq!((starts, marks.1, marks.2), (vec![(1, 1, false), (2, 4, false)], 2, vec![]));
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
    // One count of chapters wherever the edition is shown: five, not its eight releases.
    let (edition, _) = h.services.book_sources.chapters(&work, None, 1).await.unwrap();
    assert_eq!((edition.unwrap().chapters, editions[0].chapters), (5, 5));

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

/// Opening a chapter reads the chosen group's release of it, and a chapter that group lacks
/// from the next group, so what is read matches the list.
#[tokio::test(start_paused = true)]
async fn opening_reads_the_chosen_groups_release() {
    let h = harness_with(Groups::new(false), false);
    let work = WorkId::new(WORK);
    let sources = &h.services.book_sources;
    let opened = |chapter: &'static str| async move {
        let opened = sources.open(&work_id(), &ChapterId::new(chapter), None, 1).await.unwrap();
        (opened.chapter.id.clone(), opened.group.clone())
    };
    assert_eq!(opened("2").await, ("a2".to_owned(), Some("Group A".to_owned())));
    assert_eq!(opened("4").await.0, "b4", "only Group B has chapter 4");
    crate::reading::choose_group(&h.services, &work, Some("Group B".into()));
    assert_eq!(opened("2").await, ("b2".to_owned(), Some("Group B".to_owned())));
    assert_eq!(opened("1").await.0, "b1", "Group B lists chapter 1 whole");
    assert_eq!(opened("3").await.0, "a3", "Group B lacks chapter 3");
    // Downloads open the chapter in the edition they were queued for.
    let download = sources.open(&work, &ChapterId::new("2"), Some("groups"), 1).await.unwrap();
    assert_eq!(download.chapter.id, "b2", "a download is the same release as reading online");
}

fn work_id() -> WorkId {
    WorkId::new(WORK)
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

/// The leaves of `chapter`, once they come.
async fn leaves_of(h: &mut Harness, chapter: &str) -> Vec<Leaf> {
    let chapter = ChapterId::new(chapter);
    h.until(|e| match e {
        ReaderEvent::Leaves { chapter: c, leaves, .. } if *c == chapter => Some(leaves.clone()),
        _ => None,
    })
    .await
}

/// Read chapters 1 and 2 from the source, so their edges teach the group's credit pages, and
/// open chapter 3, giving its leaves and the credit pages said.
async fn third_chapter(h: &mut Harness) -> (Vec<Leaf>, Vec<kaeri_model::reader::CreditPage>) {
    rows(h).await;
    h.open(OpenAt::Start).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    leaves_of(h, "2").await;
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("3") });
    let leaves = leaves_of(h, "3").await;
    let credits = h
        .until(|e| match e {
            ReaderEvent::CreditPages { chapter, pages, .. } if chapter.as_str() == "3" => {
                Some(pages.clone())
            }
            _ => None,
        })
        .await;
    (leaves, credits)
}

/// A chapter read from the source opens without its credit pages at either end, with the same
/// count of pages as its download, so saved places and progress agree.
#[tokio::test(start_paused = true)]
async fn a_streamed_chapter_opens_without_its_credit_pages_at_both_ends() {
    let source = Arc::new(Groups::both_ends());
    let mut h = harness_with(source.clone(), false);
    let (leaves, credits) = third_chapter(&mut h).await;
    assert_eq!(leaves.len(), 4, "five pages less the two credit pages, and the end page");
    let said: Vec<(u32, bool)> = credits.iter().map(|c| (c.page, c.at_end)).collect();
    assert_eq!(said, vec![(0, false), (4, true)]);
    assert!(credits.iter().all(|c| Path::new(&c.file.path).exists()), "a tap away");

    // The same chapter downloaded, every page in its folder, opens with the same count.
    let folder = tempfile::tempdir().unwrap();
    for n in 1..=5 {
        let page = PageRef::new(format!("https://pages.example/a3/{n}"));
        let bytes = kaeri_core::traits::BookSource::fetch_page(&*source, &page).await.unwrap();
        std::fs::write(folder.path().join(format!("{n:03}.png")), bytes).unwrap();
    }
    let store = h.services.store.clone();
    let ask = kaeri_store::book_downloads::BookDownloadAsk {
        work: WORK,
        chapter: "3",
        source_id: "groups",
        version_label: Some("Group A"),
        keep_ready: true,
    };
    store.queue_book_download(&ask, 1).unwrap();
    let id = store.book_download(WORK, "3").unwrap().unwrap().id;
    store.book_download_done(id, folder.path().to_str().unwrap(), Some(5), 1, 1).unwrap();
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    h.drain();
    h.send(ReaderCommand::Open {
        work: WorkId::new(WORK),
        chapter: Some(ChapterId::new("3")),
        at: OpenAt::Start,
        surface: surface(),
    });
    let downloaded = leaves_of(&mut h, "3").await;
    let s = h.reader.session.as_ref().unwrap();
    assert_eq!(s.chapter().path, folder.path(), "read from the download");
    assert_eq!(downloaded.len(), leaves.len(), "one count, streamed or downloaded");
}

/// Credit pages learnt while a chapter is open, from the next chapter fetched ahead, leave it
/// when it opens again. Its place is kept as it counts then, so its row and the reader agree on
/// the page and the count, and the reader opens on the page read.
#[tokio::test(start_paused = true)]
async fn a_place_read_before_credit_pages_were_learnt_counts_as_the_reader_does() {
    let mut h = harness_with(Arc::new(Groups::both_ends()), false);
    rows(&h).await;
    h.open(OpenAt::Start).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    let leaves = leaves_of(&mut h, "2").await;
    assert_eq!(leaves.len(), 6, "two chapters teach no credit pages: five pages");
    // The fourth page: near the end, the next chapter is fetched ahead and its edges teach
    // both credit pages.
    h.shown(3, TurnHow::Turn).await;
    let learnt = |h: &Harness| {
        let prints = h.services.store.page_prints(WORK).unwrap();
        prints.iter().any(|p| p.chapter == "3")
    };
    let start = tokio::time::Instant::now();
    while !learnt(&h) && start.elapsed() < std::time::Duration::from_secs(20) {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        h.drain();
    }
    assert!(learnt(&h), "chapter 3 was not fetched ahead");
    h.send(ReaderCommand::Close { session });
    let kept = h.kept("2").unwrap();
    assert_eq!((kept.last_page, kept.of_pages), (Some(2), Some(3)), "page 4 of 5 is 3 of 3");
    let row = rows(&h).await.into_iter().find(|r| r.id.as_str() == "2").unwrap();
    assert_eq!(row.state, ChapterReadState::InProgress { page: 3, of: Some(3) });

    h.drain();
    let (view, leaves, notices) = h.open(OpenAt::Resume).await;
    assert_eq!(view.chapter.id.as_str(), "2");
    assert_eq!(leaves.len(), 4, "both credit pages left out");
    assert_eq!(view.start.page, Some(2), "the page read, as the row says");
    let resumed = notices.iter().any(|e| {
        matches!(e, ReaderEvent::Notice(n)
            if n.msg == Msg::Reading(ReadingMsg::ReaderResumed { page: 3 }))
    });
    assert!(resumed, "{notices:?}");
}

/// An edge page that does not come is waited on for a few seconds at most: the chapter lays out
/// without it, its end keeping every page, and the other end's credit page still left out.
#[tokio::test(start_paused = true)]
async fn a_slow_edge_page_is_waited_on_no_longer_than_the_cap() {
    let mut source = Groups::both_ends();
    source.stuck = Some("a3");
    let mut h = harness_with(Arc::new(source), false);
    rows(&h).await;
    h.open(OpenAt::Start).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    leaves_of(&mut h, "2").await;
    let asked = tokio::time::Instant::now();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("3") });
    let leaves = leaves_of(&mut h, "3").await;
    let waited = asked.elapsed();
    // Paused time runs on while the page worker lays out, so only the cap's floor is measured.
    assert!(waited >= crate::reading::feed::EDGE_WAIT, "laid out at the cap: {waited:?}");
    assert_eq!(leaves.len(), 5, "the opening credit page out, the slow end whole");
    let credits = h
        .until(|e| match e {
            ReaderEvent::CreditPages { pages, .. } => Some(pages.clone()),
            _ => None,
        })
        .await;
    let said: Vec<(u32, bool)> = credits.iter().map(|c| (c.page, c.at_end)).collect();
    assert_eq!(said, vec![(0, false)]);
}

/// A chapter in parts whose parts each end on the credit page: both leave, and the second
/// part's marker counts the pages as shown.
#[tokio::test(start_paused = true)]
async fn part_markers_count_the_pages_left_after_credit_pages() {
    let mut h = harness_with(Groups::new(true), false);
    rows(&h).await;
    h.open(OpenAt::Start).await;
    let session = h.session();
    for chapter in ["2", "3", "1"] {
        h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new(chapter) });
        leaves_of(&mut h, chapter).await;
    }
    // Chapter 1 opened again, now that its credit pages are known: its parts as said last.
    let marks = h
        .drain()
        .into_iter()
        .filter_map(|e| match e {
            ReaderEvent::ChapterParts { chapter, parts, total, .. }
                if chapter.as_str() == "1" =>
            {
                Some((parts, total))
            }
            _ => None,
        })
        .next_back()
        .expect("the parts are told");
    let starts: Vec<(u32, u32)> = marks.0.iter().map(|p| (p.number, p.first_page)).collect();
    assert_eq!((starts, marks.1), (vec![(1, 1), (2, 3)], 2), "part 2 starts on page 3 of 4");
}

/// A chapter in parts read from its download is told its parts as one read from the source is.
#[tokio::test]
async fn a_downloaded_chapter_in_parts_is_told_its_parts() {
    use kaeri_model::reading::DownloadPick;
    use kaeri_store::book_downloads::BookDownloadState;
    let mut h = harness_with(Groups::new(false), false);
    rows(&h).await;
    let downloads = &h.services.book_sources.downloads;
    downloads.configure(crate::downloads::books::Setup::default());
    let lent = crate::downloads::books::Lent::of(&h.services, &h.tx);
    let pick = DownloadPick::Chapters { ids: vec![ChapterId::new("1")] };
    downloads.download(&lent, WorkId::new(WORK), pick).await;
    let start = tokio::time::Instant::now();
    loop {
        let row = h.services.store.book_download(WORK, "1").unwrap();
        if row.as_ref().is_some_and(|r| r.state == BookDownloadState::Done) {
            break;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(20),
            "not downloaded: {row:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let (view, _, after) = h.open(OpenAt::Start).await;
    assert_eq!(view.chapter.id.as_str(), "1");
    let path = h.reader.session.as_ref().unwrap().chapter().path.clone();
    assert!(path.extension().is_some_and(|e| e == "cbz"), "read from {path:?}");
    let marks = after
        .iter()
        .find_map(|e| match e {
            ReaderEvent::ChapterParts { parts, total, missing, .. } => {
                Some((parts.clone(), *total, missing.clone()))
            }
            _ => None,
        })
        .expect("the parts are told");
    let starts: Vec<(u32, u32, bool)> =
        marks.0.iter().map(|p| (p.number, p.first_page, p.other_group)).collect();
    assert_eq!((starts, marks.1, marks.2), (vec![(1, 1, false), (2, 4, false)], 2, vec![]));
}

/// Kept at the start: the chapter is laid out again with the kept page as page 0 and every later
/// page one on, its files sent again under the new numbers from another folder, and only the
/// page at the end still left out. A client drops what it holds under the old numbers.
#[tokio::test(start_paused = true)]
async fn a_page_kept_at_the_start_comes_back_as_page_0_and_the_rest_move_one_on() {
    let source = Arc::new(Groups::both_ends());
    let mut h = harness_with(source, false);
    let (leaves, credits) = third_chapter(&mut h).await;
    assert_eq!(leaves.len(), 4);
    let start = credits.iter().find(|c| !c.at_end).unwrap().clone();
    let first = |events: Vec<ReaderEvent>| {
        events.into_iter().find_map(|e| match e {
            ReaderEvent::PageReady { page: 0, file, .. } => Some(file.path),
            _ => None,
        })
    };
    let before = first(h.drain()).expect("page 0 landed");
    let session = h.session();
    h.send(ReaderCommand::NotACreditPage { session, fingerprint: start.fingerprint });
    let leaves = leaves_of(&mut h, "3").await;
    let pages: Vec<Vec<u32>> =
        leaves.iter().map(|l| l.parts.iter().map(|p| p.page).collect()).collect();
    assert_eq!(pages, vec![vec![0], vec![1], vec![2], vec![3], vec![]], "four pages, the end");
    let left = h
        .until(|e| match e {
            ReaderEvent::CreditPages { pages, .. } => Some(pages.clone()),
            _ => None,
        })
        .await;
    assert_eq!(left.iter().map(|c| (c.page, c.at_end)).collect::<Vec<_>>(), vec![(4, true)]);
    // Made again on the pages' own threads: waited for, not a paused clock's five seconds,
    // which pass at once while those threads are still at work.
    let after = h
        .until(|e| match e {
            ReaderEvent::PageReady { page: 0, file, .. } => Some(file.path.clone()),
            _ => None,
        })
        .await;
    assert_ne!(after, before, "page 0 is another file now");
}
