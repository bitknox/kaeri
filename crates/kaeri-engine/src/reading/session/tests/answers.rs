//! Every command the app waits on gets an answer: a chapter opens, or the reader hears why
//! not. None is dropped without a word.

use std::time::Duration;

use super::*;

/// Wait at most a few seconds for the first event `pick` takes.
async fn within<T>(h: &mut Harness, pick: impl FnMut(&ReaderEvent) -> Option<T>) -> Option<T> {
    tokio::time::timeout(Duration::from_secs(5), h.until(pick)).await.ok()
}

#[tokio::test]
async fn a_chapter_no_list_names_fails_as_not_found() {
    let mut h = harness(Store::open_in_memory().unwrap());
    h.failures = true;
    h.open(OpenAt::Start).await;
    h.drain();
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("3") });
    h.drain();
    assert_eq!(h.failed, [ReadingFailure::NotFound]);
    // The session is as it was, and a chapter it lists still opens.
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    let opened =
        within(&mut h, |e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;
    assert!(opened.is_some(), "a listed chapter opens after one that was not");
}

#[tokio::test]
async fn a_chapter_put_on_the_shelf_after_the_open_opens() {
    let mut h = harness(Store::open_in_memory().unwrap());
    h.open(OpenAt::Start).await;
    h.drain();
    // A third chapter lands in the folder while the reader is open.
    let first = h.reader.session.as_ref().unwrap().chapters[0].path.clone();
    let path = first.parent().unwrap().join("Chapter 3.cbz");
    cbz(&path, 3);
    let file = ShelfFile {
        path: path.display().to_string(),
        chapter: None,
        label: "Chapter 3".into(),
        volume: None,
        number_milli: Some(3000),
        format: "cbz".into(),
        pages: Some(3),
        comicinfo: None,
        source: None,
    };
    h.services.store.put_local_file("s1", &file, 2, 2).unwrap();

    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("3") });
    let laid = within(&mut h, |e| match e {
        ReaderEvent::Leaves { chapter, .. } => Some(chapter.clone()),
        _ => None,
    })
    .await;
    assert_eq!(laid, Some(ChapterId::new("3")));
}

#[tokio::test]
async fn a_command_for_a_closed_session_hears_it_is_closed() {
    let mut h = harness(Store::open_in_memory().unwrap());
    h.open(OpenAt::Start).await;
    h.drain();
    let gone = h.session() + 7;
    for command in [
        ReaderCommand::OpenChapter { session: gone, chapter: ChapterId::new("2") },
        ReaderCommand::BackTo { session: gone },
        ReaderCommand::GoBack { session: gone },
        ReaderCommand::StartOver { session: gone },
        ReaderCommand::Retry { session: gone, chapter: ChapterId::new("1"), page: None },
    ] {
        h.send(command.clone());
        let closed = h
            .drain()
            .into_iter()
            .any(|e| matches!(e, ReaderEvent::Closed { session } if session == gone));
        assert!(closed, "{command:?} got no answer");
    }
    // A report the app does not wait on is dropped as before.
    h.send(ReaderCommand::Shown {
        session: gone,
        chapter: ChapterId::new("1"),
        leaf: 0,
        how: TurnHow::Turn,
    });
    assert!(h.drain().is_empty());
}

#[tokio::test]
async fn back_to_page_with_nothing_to_go_back_to_says_the_offer_is_gone() {
    let mut h = harness(Store::open_in_memory().unwrap());
    h.open(OpenAt::Start).await;
    h.drain();
    let session = h.session();
    h.send(ReaderCommand::BackTo { session });
    let said = h.drain();
    assert!(
        said.iter().any(|e| matches!(e, ReaderEvent::BackToPage { page: None, .. })),
        "{said:?}"
    );
}
