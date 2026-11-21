//! A chapter read from a source keeps one folder of pages in the page cache, however often it
//! is laid out: again once its pages have landed, and again from the folder they landed in when
//! the reader comes back to it. A folder of the same name filled in a later run never shows the
//! pages kept for the earlier one.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use kaeri_model::reader::PageFile;
use tokio::time::timeout;

use super::*;

const WAIT: Duration = Duration::from_secs(20);

/// Page `page` of `chapter` as it is made, past any other.
async fn made(h: &mut Harness, chapter: &str, page: u32) -> PageFile {
    let chapter = ChapterId::new(chapter);
    let made = h.until(|e| match e {
        ReaderEvent::PageReady { chapter: c, page: p, file, .. }
            if *c == chapter && *p == page =>
        {
            Some(file.clone())
        }
        _ => None,
    });
    timeout(WAIT, made)
        .await
        .unwrap_or_else(|_| panic!("page {page} of chapter {chapter} is made"))
}

/// The cache folder a page's file is in.
fn folder(file: &PageFile) -> PathBuf {
    Path::new(&file.path).parent().expect("a page file is in a folder").to_owned()
}

/// Every folder of pages the page cache keeps.
fn cached(h: &Harness) -> Vec<PathBuf> {
    let cache = crate::reading::pages::cache_dir(&h.services.paths.cache_dir);
    let entries = std::fs::read_dir(cache).expect("the page cache is there");
    entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()
}

/// A chapter laid out again once its pages have landed, as one whose last page came as a spread
/// is, makes its pages from the folder it kept them in, rather than copying the chapter into a
/// second one.
#[tokio::test]
async fn a_chapter_laid_out_again_keeps_its_pages_in_one_folder() {
    let mut source = PageSource::new(&["1"]);
    source.held_from = Some(1);
    source.wide.insert(2);
    let source = Arc::new(source);
    let mut h = reading(source.clone());
    h.open_read(None).await;
    let first = timeout(WAIT, h.leaves_of("1")).await.expect("laid out with pages to come");
    assert_eq!(first.iter().filter(|l| !l.end_page).count(), 3);
    let before = made(&mut h, "1", 0).await;

    source.gate.add_permits(2);
    let again = timeout(WAIT, h.leaves_of("1")).await.expect("laid out again");
    assert_eq!(again.iter().filter(|l| !l.end_page).count(), 4, "the spread split in two");
    let after = made(&mut h, "1", 0).await;
    assert_eq!(folder(&after), folder(&before), "made from the folder it was kept in");
    assert_eq!(cached(&h), [folder(&before)], "one folder of the chapter's pages");
}

/// A chapter whose pages landed after it opened is read from the folder they landed in once the
/// reader comes back to it, and finds them where the cache kept them while they landed.
#[tokio::test]
async fn a_chapter_read_again_from_its_folder_finds_its_pages_kept() {
    let source = Arc::new(PageSource { held_from: Some(1), ..PageSource::new(&["1", "2"]) });
    let mut h = reading(source.clone());
    h.open_read(Some("1")).await;
    timeout(WAIT, h.leaves_of("1")).await.expect("laid out with pages to come");
    let before = made(&mut h, "1", 0).await;

    // The rest land in the shape guessed for them.
    source.gate.add_permits(16);
    let landed = async {
        loop {
            h.drain();
            let s = h.reader.session.as_ref().unwrap();
            if s.feed.as_ref().is_some_and(|f| f.complete()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    timeout(WAIT, landed).await.expect("every page of chapter 1 landed");

    let session = h.session();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
    timeout(WAIT, h.leaves_of("2")).await.expect("chapter 2 opens");
    h.drain();
    h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("1") });
    timeout(WAIT, h.leaves_of("1")).await.expect("chapter 1 opens again");
    assert_eq!(source.opened_of("ch-1"), 1, "read from its folder, not fetched again");
    let after = made(&mut h, "1", 0).await;
    assert_eq!(folder(&after), folder(&before), "its pages where the cache kept them");
}

/// Session numbers start again each run, so the first chapter of a later run is fetched into a
/// folder of the same name as the last run's first. The pages the cache kept for that folder are
/// the earlier chapter's, and are never shown for this one.
#[tokio::test]
async fn a_later_run_never_shows_the_pages_kept_for_an_earlier_runs_folder() {
    let mut h = reading(Arc::new(PageSource::new(&["1"])));
    h.open_read(None).await;
    timeout(WAIT, h.leaves_of("1")).await.expect("the chapter opens");
    let before = made(&mut h, "1", 0).await;
    let fed = |h: &Harness| {
        let s = h.reader.session.as_ref().unwrap();
        s.feed.as_ref().expect("its pages come from the source").dir.clone()
    };
    let fetched = fed(&h);
    let session = h.session();
    h.send(ReaderCommand::Close { session });

    // The app starts again, and the chapter's pages are other pictures of the same size.
    let source = PageSource { shade: 60, ..PageSource::new(&["1"]) };
    h.services.book_sources.registry.put(Arc::new(source));
    h.reader = Reader::default();
    h.drain();
    h.open_read(None).await;
    timeout(WAIT, h.leaves_of("1")).await.expect("the chapter opens again");
    assert_eq!(fed(&h), fetched, "fetched into a folder of the same name");
    let after = made(&mut h, "1", 0).await;
    assert_ne!(folder(&after), folder(&before), "kept apart from the earlier run's pages");
    let shown = image::open(&after.path).expect("the page decodes").to_luma8();
    assert_eq!(shown.get_pixel(0, 0).0, [60], "the page this run fetched");
}
