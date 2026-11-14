//! The chapter a strip flowed on from stays above the one opening, and its pages are still made
//! when the app asks for them by its id: from a file, and from a source whose pages had not all
//! landed when the strip flowed on.

use std::time::Duration;

use tokio::time::timeout;

use super::*;

/// The page `page` of `chapter` made, past any other.
async fn made(h: &mut Harness, chapter: &str, page: u32) {
    let chapter = ChapterId::new(chapter);
    let wait = Duration::from_secs(20);
    let found = timeout(
        wait,
        h.until(|e| match e {
            ReaderEvent::PageReady { chapter: c, page: p, .. }
                if *c == chapter && *p == page =>
            {
                Some(())
            }
            _ => None,
        }),
    )
    .await;
    assert!(found.is_ok(), "page {page} of chapter {chapter} was not made");
}

#[tokio::test]
async fn a_file_chapter_kept_above_makes_the_pages_asked_by_its_id() {
    let mut h = harness(Store::open_in_memory().unwrap());
    h.services.store.set_series_setting(WORK, "mode", Some("strip"), true, 1).unwrap();
    let work = WorkId::new(WORK);
    let chapter = Some(ChapterId::new("1"));
    h.send(ReaderCommand::Open { work, chapter, at: OpenAt::Start, surface: surface() });
    h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    h.until(|e| {
        matches!(e, ReaderEvent::Leaves { chapter, .. } if chapter.as_str() == "2")
            .then_some(())
    })
    .await;
    h.drain();

    // The images of chapter 1 the fling went past are asked for once on screen above chapter 2.
    let chapter = ChapterId::new("1");
    h.send(ReaderCommand::NeedPage { session, chapter: chapter.clone(), page: 1 });
    made(&mut h, "1", 1).await;
    h.send(ReaderCommand::Retry { session, chapter, page: Some(2) });
    made(&mut h, "1", 2).await;
    // The open chapter's own are made as before.
    h.send(ReaderCommand::NeedPage { session, chapter: ChapterId::new("2"), page: 1 });
    made(&mut h, "2", 1).await;
}

#[tokio::test]
async fn a_source_chapter_kept_above_still_fetches_a_page_asked_again() {
    // Every page after the first waits until let through: the strip flows on before they land.
    let source = Arc::new(PageSource { held_from: Some(1), ..PageSource::new(&["1", "2"]) });
    let mut h = reading(source.clone());
    h.services.store.set_series_setting(READ, "mode", Some("strip"), true, 1).unwrap();
    h.open_read(Some("1")).await;
    let wait = Duration::from_secs(20);
    timeout(wait, h.leaves_of("1")).await.expect("chapter 1 opens");
    made(&mut h, "1", 0).await;
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    timeout(wait, h.leaves_of("2")).await.expect("chapter 2 opens under it");

    h.send(ReaderCommand::NeedPage { session, chapter: ChapterId::new("1"), page: 2 });
    source.gate.add_permits(16);
    made(&mut h, "1", 2).await;
}
