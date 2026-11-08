//! What the trackers hear when a chapter or volume is read to its end in the reader.

use super::*;

/// Read the open chapter to its last page and wait for it to count as read.
async fn read_through(h: &mut Harness) -> ChapterId {
    h.shown(2, TurnHow::Turn).await;
    h.until(|e| match e {
        ReaderEvent::ChapterRead { chapter, .. } => Some(chapter.clone()),
        _ => None,
    })
    .await
}

/// Chapters 1 and 2 in volume 1, chapter 3 in volume 2, in a folder.
fn two_volumes(books: &Path) -> Vec<ShelfFile> {
    [(1, "1"), (2, "1"), (3, "2")]
        .into_iter()
        .map(|(n, volume)| {
            let path = books.join(format!("Vol {volume} Chapter {n}.cbz"));
            cbz(&path, 3);
            ShelfFile {
                path: path.display().to_string(),
                chapter: None,
                label: format!("Chapter {n}"),
                volume: Some(volume.into()),
                number_milli: Some(i64::from(n) * 1000),
                format: "cbz".into(),
                pages: Some(3),
                comicinfo: None,
                source: None,
            }
        })
        .collect()
}

/// Reading the last chapter of volume 1 to its end tells the tracker the chapter and the
/// volume it finished, not the volume only at the next read.
#[tokio::test(start_paused = true)]
async fn finishing_a_volume_tells_the_tracker_the_volume() {
    let mut h =
        harness_signed(Store::open_in_memory().unwrap(), "manga", two_volumes, None, true);
    let work = WorkId::new(WORK);
    h.services.store.set_book_match("folders", "s1", WORK, 100, true, false, 1).unwrap();
    h.services.book_sources.chapters(&work, None, 1).await.unwrap();
    h.open(OpenAt::Resume).await;
    assert_eq!(read_through(&mut h).await.as_str(), "1");
    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;
    assert_eq!(read_through(&mut h).await.as_str(), "2");
    let ops = h.services.store.read_outbox("anilist").unwrap();
    let last = ops.last().cloned();
    assert!(
        matches!(last, Some(TrackOp::SetReadProgress { chapter: 2, volume: Some(1), .. })),
        "volume 1 is finished: {ops:?}"
    );
}
