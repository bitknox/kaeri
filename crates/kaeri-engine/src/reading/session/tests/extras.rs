//! The end page's skip past extras: offered only when the next chapter is one, it opens the
//! next whole chapter, moves on as Continue does, and leaves the extras unread.

use super::*;
use kaeri_model::reading::ChapterRow;

/// Chapters 16, 16.5, 17 and 18 of three pages in a folder.
fn with_an_extra(books: &Path) -> Vec<ShelfFile> {
    [16_000, 16_500, 17_000, 18_000]
        .into_iter()
        .map(|milli| {
            let number = if milli % 1000 == 0 {
                (milli / 1000).to_string()
            } else {
                format!("{}.{}", milli / 1000, (milli % 1000) / 100)
            };
            let path = books.join(format!("Chapter {number}.cbz"));
            cbz(&path, 3);
            ShelfFile {
                path: path.display().to_string(),
                chapter: None,
                label: format!("Chapter {number}"),
                volume: None,
                number_milli: Some(milli),
                format: "cbz".into(),
                pages: Some(3),
                comicinfo: None,
                source: None,
            }
        })
        .collect()
}

/// The end page as it is said, once the last page shows.
async fn end_page(h: &mut Harness) -> (Option<ChapterRow>, Option<ChapterRow>) {
    h.shown(2, TurnHow::Turn).await;
    h.until(|e| match e {
        ReaderEvent::EndPage { next, skip_to, .. } => Some((next.clone(), skip_to.clone())),
        _ => None,
    })
    .await
}

fn id(row: Option<ChapterRow>) -> Option<String> {
    row.map(|r| r.id.0)
}

#[tokio::test(start_paused = true)]
async fn the_end_page_offers_a_skip_past_an_extra_to_the_next_whole_chapter() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", with_an_extra);
    let (view, _, _) = h.open(OpenAt::Start).await;
    assert_eq!(view.chapter.id.as_str(), "16");

    let (next, skip_to) = end_page(&mut h).await;
    assert_eq!((id(next), id(skip_to.clone())), (Some("16.5".into()), Some("17".into())));
    let skip_to = skip_to.unwrap();
    assert_eq!(skip_to.number.as_deref(), Some("17"), "the button names its number");

    // The skip opens 17 and moves on from 16 as Continue would; 16.5 stays unread.
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: skip_to.id });
    let read = h
        .until(|e| match e {
            ReaderEvent::ChapterRead { chapter, .. } => Some(chapter.clone()),
            _ => None,
        })
        .await;
    assert_eq!(read.as_str(), "16");
    let view = h
        .until(|e| match e {
            ReaderEvent::Opened(view) => Some(view.as_ref().clone()),
            _ => None,
        })
        .await;
    assert_eq!(view.chapter.id.as_str(), "17");
    h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;

    // A normal next chapter offers no skip.
    let (next, skip_to) = end_page(&mut h).await;
    assert_eq!((id(next), id(skip_to)), (Some("18".into()), None));
    assert!(h.kept("16.5").is_none_or(|c| !c.read), "the extra stays unread");
}

#[tokio::test(start_paused = true)]
async fn an_extra_followed_only_by_extras_offers_no_skip() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |books| {
        with_an_extra(books).into_iter().take(2).collect()
    });
    h.open(OpenAt::Start).await;
    let (next, skip_to) = end_page(&mut h).await;
    assert_eq!((id(next), id(skip_to)), (Some("16.5".into()), None));
}
