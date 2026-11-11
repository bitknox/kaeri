//! Back to page: offered only after the person's own jump of more than two pages, never for a
//! place the app chose, and gone at the next turn.

use super::*;

/// Two chapters of twelve pages, read in `mode`.
fn twelve_pages_in(store: Store, mode: &str) -> Harness {
    let h = harness_of(store, "manga", |books| {
        (1..=2)
            .map(|n| {
                let path = books.join(format!("Chapter {n}.cbz"));
                cbz(&path, 12);
                ShelfFile {
                    path: path.display().to_string(),
                    chapter: None,
                    label: format!("Chapter {n}"),
                    volume: None,
                    number_milli: Some(n * 1000),
                    format: "cbz".into(),
                    pages: Some(12),
                    comicinfo: None,
                    source: None,
                }
            })
            .collect()
    });
    h.services.store.set_series_setting(WORK, "mode", Some(mode), true, 2).unwrap();
    h
}

fn twelve_pages(mode: &str) -> Harness {
    twelve_pages_in(Store::open_in_memory().unwrap(), mode)
}

#[tokio::test(start_paused = true)]
async fn a_jump_of_one_or_two_pages_offers_nothing() {
    let mut h = twelve_pages("paged");
    h.open(OpenAt::Start).await;
    h.shown(4, TurnHow::Turn).await;
    h.shown(5, TurnHow::Jump).await;
    h.shown(3, TurnHow::Jump).await;
    assert_eq!(offers(&h.drain()), []);
}

#[tokio::test(start_paused = true)]
async fn a_jump_of_three_pages_or_more_offers_the_page_it_left_until_the_next_turn() {
    let mut h = twelve_pages("paged");
    h.open(OpenAt::Start).await;
    h.shown(1, TurnHow::Turn).await;
    h.shown(4, TurnHow::Jump).await;
    assert_eq!(offers(&h.drain()), [Some(2)]);
    // A second jump keeps the first place.
    h.shown(10, TurnHow::Jump).await;
    assert_eq!(offers(&h.drain()), []);
    h.shown(11, TurnHow::Turn).await;
    assert_eq!(offers(&h.drain()), [None]);
    h.shown(10, TurnHow::Key).await;
    assert_eq!(offers(&h.drain()), []);
}

#[tokio::test(start_paused = true)]
async fn using_it_goes_back_and_takes_it_away() {
    let mut h = twelve_pages("paged");
    h.open(OpenAt::Start).await;
    h.shown(1, TurnHow::Turn).await;
    h.shown(9, TurnHow::Jump).await;
    assert_eq!(offers(&h.drain()), [Some(2)]);
    h.send(ReaderCommand::BackTo { session: h.session() });
    let said = h.drain();
    assert_eq!(offers(&said), [None]);
    let to = said.iter().find_map(|e| match e {
        ReaderEvent::GoTo { place, .. } => Some(place.clone()),
        _ => None,
    });
    assert_eq!(to.map(|p| (p.chapter.0, p.page)), Some(("1".into(), Some(1))));
    // The app's landing there offers nothing back.
    h.shown(1, TurnHow::Placed).await;
    h.send(ReaderCommand::BackTo { session: h.session() });
    let said = h.drain();
    assert_eq!(offers(&said), []);
    assert!(!said.iter().any(|e| matches!(e, ReaderEvent::GoTo { .. })), "{said:?}");
}

#[tokio::test(start_paused = true)]
async fn a_resume_offers_nothing() {
    let mut h = twelve_pages("paged");
    let mut kept = ReadEvent::new(WORK, "1", 1);
    kept.page = Some(9);
    h.services.store.record_read(&kept, Vec::new()).unwrap();
    let (view, _, _) = h.open(OpenAt::Resume).await;
    assert_eq!(view.start.page, Some(9));
    h.shown(9, TurnHow::Placed).await;
    // Start over is the app's move too.
    h.send(ReaderCommand::StartOver { session: h.session() });
    h.shown(0, TurnHow::Placed).await;
    assert_eq!(offers(&h.drain()), []);
}

#[tokio::test(start_paused = true)]
async fn a_new_layout_offers_nothing() {
    let mut h = twelve_pages("paged");
    h.open(OpenAt::Start).await;
    h.shown(8, TurnHow::Turn).await;
    let session = h.session();
    let wide = ReaderSurface { width_px: 1800, two_pages: true, ..surface() };
    h.send(ReaderCommand::Surface { session, surface: wide });
    h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;
    h.shown(4, TurnHow::Placed).await;
    assert_eq!(offers(&h.drain()), []);
}

/// A place this device kept while sync was on stays its own after sync is turned off: opening an
/// earlier chapter stays where asked, with no Moved notice.
#[tokio::test(start_paused = true)]
async fn this_devices_place_from_before_sync_was_off_moves_nothing() {
    let mine = Store::open_in_memory().unwrap();
    mine.sync_enable("phone").unwrap();
    let mut ahead = ReadEvent::new(WORK, "2", 2);
    ahead.page = Some(9);
    mine.record_read(&ahead, Vec::new()).unwrap();
    assert_eq!(kept_device(&mine).as_deref(), Some("phone"), "kept under this device's id");
    mine.sync_disable().unwrap();
    let mut h = twelve_pages_in(mine, "paged");
    let work = WorkId::new(WORK);
    let chapter = Some(ChapterId::new("1"));
    h.send(ReaderCommand::Open { work, chapter, at: OpenAt::Start, surface: surface() });
    h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;
    h.shown(0, TurnHow::Placed).await;
    let said = h.drain();
    assert!(
        !said.iter().any(|e| matches!(e, ReaderEvent::GoTo { .. } | ReaderEvent::Moved { .. })),
        "{said:?}"
    );
    assert_eq!(h.reader.session.as_ref().unwrap().chapter().id.as_str(), "1");
}

fn kept_device(store: &Store) -> Option<String> {
    store.read_chapters(WORK).unwrap().into_iter().find(|c| c.chapter == "2")?.device
}

#[tokio::test(start_paused = true)]
async fn another_devices_place_and_going_back_from_it_offer_nothing() {
    let mine = Store::open_in_memory().unwrap();
    mine.sync_enable("phone").unwrap();
    let desk = Store::open_in_memory().unwrap();
    desk.set_sync_clock(|| 4_000_000_000_000);
    desk.sync_enable("desk").unwrap();
    mine.sync_device_add("desk", "Desk", "").unwrap();
    let mut h = twelve_pages_in(mine.clone(), "paged");
    h.open(OpenAt::Start).await;
    h.shown(1, TurnHow::Turn).await;
    let mut ahead = ReadEvent::new(WORK, "1", 2);
    ahead.page = Some(9);
    desk.record_read(&ahead, Vec::new()).unwrap();
    let ops = desk.ops_after(&mine.version_vector().unwrap(), usize::MAX).unwrap();
    mine.apply_ops(ops).unwrap();
    h.synced();
    let to = h
        .until(|e| match e {
            ReaderEvent::GoTo { place, .. } => Some(place.page),
            _ => None,
        })
        .await;
    assert_eq!(to, Some(9));
    h.shown(9, TurnHow::Placed).await;
    h.send(ReaderCommand::GoBack { session: h.session() });
    h.shown(1, TurnHow::Placed).await;
    assert_eq!(offers(&h.drain()), []);
}

#[tokio::test(start_paused = true)]
async fn a_strip_offers_after_a_jump_and_scrolling_to_another_image_takes_it_away() {
    let mut h = twelve_pages("strip");
    let (view, _, _) = h.open(OpenAt::Start).await;
    assert_eq!(view.mode, ReaderMode::Strip);
    let session = h.session();
    let chapter = view.chapter.id.clone();
    let settled = |image| ReaderCommand::StripAt {
        session,
        chapter: chapter.clone(),
        at: StripPos { image, offset: 0.2 },
        foot_visible: false,
    };
    h.send(settled(1));
    h.shown(2, TurnHow::Jump).await;
    assert_eq!(offers(&h.drain()), [], "one image on");
    h.shown(10, TurnHow::Jump).await;
    assert_eq!(offers(&h.drain()), [Some(3)]);
    // Settling on the image it landed on keeps it; the next image takes it away.
    h.send(settled(10));
    assert_eq!(offers(&h.drain()), []);
    h.send(settled(11));
    assert_eq!(offers(&h.drain()), [None]);
}

#[tokio::test(start_paused = true)]
async fn the_next_chapter_takes_it_away() {
    let mut h = twelve_pages("paged");
    h.open(OpenAt::Start).await;
    h.shown(1, TurnHow::Turn).await;
    h.shown(9, TurnHow::Jump).await;
    assert_eq!(offers(&h.drain()), [Some(2)]);
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    h.until(|e| matches!(e, ReaderEvent::Opened(_)).then_some(())).await;
    h.shown(0, TurnHow::Placed).await;
    h.send(ReaderCommand::BackTo { session });
    let said = h.drain();
    assert!(!said.iter().any(|e| matches!(e, ReaderEvent::GoTo { .. })), "{said:?}");
}
