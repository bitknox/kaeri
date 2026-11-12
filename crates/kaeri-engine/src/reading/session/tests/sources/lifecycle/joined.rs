//! A chapter of pictures opened while its download runs shares the download's pages: those kept
//! show at once, the rest come from whichever of the two asks first, and the source is asked for
//! the chapter and each page once.

use kaeri_store::book_downloads::BookDownloadState as State;

use super::*;
use crate::downloads::books::{Lent, Setup};

const WAIT: Duration = Duration::from_secs(20);

fn keeping_ready() -> Setup {
    Setup {
        keep_ready: true,
        new_chapters: kaeri_core::config::NewChapters::Reading,
        ..Setup::default()
    }
}

/// Chapter 1 of [`MANY`] open with chapters kept ready, `held` pages of chapter 2 held, and its
/// download waiting on the first of them.
async fn downloading(source: &Arc<Many>, held: &[usize]) -> Harness {
    *source.held_pages.lock().unwrap() = held.iter().map(|p| ("ch-2".to_owned(), *p)).collect();
    let mut h = many(source.clone());
    h.services.book_sources.downloads.configure(keeping_ready());
    h.open_many(Some("1"));
    timeout(WAIT, h.leaves_of("1")).await.expect("laid out");
    let first = held[0];
    h.wait_for(|_| source.asked_in("ch-2").iter().any(|(p, _)| *p == first)).await;
    assert_eq!(h.download_state("2"), Some(State::Downloading));
    h
}

impl Many {
    fn let_pages(&self) {
        self.held_pages.lock().unwrap().clear();
        self.changed.notify_waiters();
    }

    fn contents_of(&self, chapter: &str) -> usize {
        self.contents.lock().unwrap().iter().filter(|c| *c == chapter).count()
    }

    /// Every page of `chapter` asked for once.
    fn each_once(&self, chapter: &str) -> bool {
        let asked = self.asked_in(chapter);
        asked.len() == self.pages && asked.iter().all(|(_, n)| *n == 1)
    }
}

impl Harness {
    fn download_state(&self, chapter: &str) -> Option<State> {
        self.services.store.book_download(MANY, chapter).unwrap().map(|r| r.state)
    }

    /// Carry what comes until `done` holds, for [`WAIT`] at most.
    async fn wait_for(&mut self, done: impl Fn(&Self) -> bool) {
        let start = tokio::time::Instant::now();
        while !done(self) && start.elapsed() < WAIT {
            self.pump(Duration::from_millis(10)).await;
        }
        assert!(done(self), "never came");
    }

    /// Whether every page of the open chapter's feed has landed.
    fn fed(&self) -> bool {
        let s = self.reader.session.as_ref().unwrap();
        s.feed.as_ref().is_some_and(|f| f.chapter == s.chapter().id && f.complete())
    }

    fn open_chapter(&mut self, chapter: &str) {
        let session = self.session();
        self.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new(chapter) });
    }
}

/// Chapter 2 opens while its download waits on page 4: the three pages it kept show at once,
/// and once the rest come, the source was asked for the chapter and each page once.
#[tokio::test]
async fn a_chapter_opened_while_its_download_runs_shows_its_kept_pages_at_once() {
    let source = Arc::new(Many::new(4, 6));
    let mut h = downloading(&source, &[3]).await;
    h.open_chapter("2");
    let leaves = timeout(WAIT, h.leaves_of("2")).await.expect("laid out");
    assert!(!leaves.is_empty());
    assert_eq!(h.download_state("2"), Some(State::Downloading), "shown before it was done");
    assert_eq!(source.asked_in("ch-2").iter().find(|(p, _)| *p == 3), Some(&(3, 1)));
    source.let_pages();
    h.wait_for(|h| h.fed() && h.download_state("2") == Some(State::Done)).await;
    assert!(source.each_once("ch-2"), "asked again: {:?}", source.asked_in("ch-2"));
    assert_eq!(source.contents_of("ch-2"), 1, "the chapter asked for again");
    let kept = h.services.book_sources.downloads.first_kept_page(MANY, "2");
    assert_eq!(kept, None, "the download's pages stayed");
}

/// A jump to a page neither has yet fetches it at once, and once: the download takes it from
/// the reader when it gets there.
#[tokio::test]
async fn a_jump_while_the_download_runs_asks_for_the_page_once() {
    let source = Arc::new(Many::new(4, 12));
    let mut h = downloading(&source, &[2, 3, 4, 5, 6, 7, 8, 9]).await;
    h.open_chapter("2");
    let leaves = timeout(WAIT, h.leaves_of("2")).await.expect("laid out");
    let leaf = leaves.iter().find(|l| l.parts.iter().any(|p| p.page == 7)).unwrap().index;
    h.shown(leaf, TurnHow::Jump).await;
    h.wait_for(|_| source.asked_in("ch-2").iter().any(|(p, _)| *p == 7)).await;
    source.let_pages();
    h.wait_for(|h| h.fed() && h.download_state("2") == Some(State::Done)).await;
    assert!(source.each_once("ch-2"), "asked again: {:?}", source.asked_in("ch-2"));
    assert_eq!(source.contents_of("ch-2"), 1);
}

/// The download fails on a page the reader waits on: the reader asks for that page once more
/// and for the pages still missing once, and reads the whole chapter.
#[tokio::test]
async fn a_download_that_fails_partway_leaves_the_reader_the_missing_pages() {
    let source = Arc::new(Many::new(4, 8));
    let mut h = downloading(&source, &[3]).await;
    h.open_chapter("2");
    timeout(WAIT, h.leaves_of("2")).await.expect("laid out");
    source.fail_once.lock().unwrap().insert(("ch-2".to_owned(), 3));
    source.let_pages();
    h.wait_for(|h| h.fed() && h.download_state("2") == Some(State::Failed)).await;
    let asked = source.asked_in("ch-2");
    let twice: Vec<usize> = asked.iter().filter(|(_, n)| *n > 1).map(|(p, _)| *p).collect();
    assert_eq!(twice, vec![3], "asked again: {asked:?}");
    assert_eq!((asked.len(), asked[3].1), (8, 2), "{asked:?}");
}

/// The reader closed while the download runs: the download fetches what the reader had not,
/// each page once, and its pages go once it is done.
#[tokio::test]
async fn a_reader_closed_while_the_download_runs_leaves_it_the_rest() {
    let source = Arc::new(Many::new(4, 6));
    let mut h = downloading(&source, &[3]).await;
    h.open_chapter("2");
    timeout(WAIT, h.leaves_of("2")).await.expect("laid out");
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    source.let_pages();
    h.wait_for(|h| h.download_state("2") == Some(State::Done)).await;
    assert!(source.each_once("ch-2"), "asked again: {:?}", source.asked_in("ch-2"));
    let kept = h.services.book_sources.downloads.first_kept_page(MANY, "2");
    assert_eq!(kept, None, "the download's pages stayed");
}

/// The download is taken out of the queue while its chapter is read: the reader keeps the pages
/// it shared and fetches the rest itself.
#[tokio::test]
async fn a_download_cancelled_while_its_chapter_is_read_leaves_the_reader_its_pages() {
    let source = Arc::new(Many::new(4, 8));
    let mut h = downloading(&source, &[3]).await;
    h.open_chapter("2");
    timeout(WAIT, h.leaves_of("2")).await.expect("laid out");
    let lent = Lent::of(&h.services, &h.tx);
    h.services.book_sources.downloads.cancel(&lent, &WorkId::new(MANY), &[ChapterId::new("2")]);
    source.let_pages();
    h.wait_for(|h| h.fed() && h.download_state("2") != Some(State::Downloading)).await;
    assert!(source.each_once("ch-2"), "asked again: {:?}", source.asked_in("ch-2"));
}

/// A chapter opened while its keep-ready download waits in a held queue is fetched by the
/// reader, and leaves the queue: once the queue runs, the source is not asked for it again.
#[tokio::test]
async fn a_chapter_the_reader_fetches_leaves_the_keep_ready_queue() {
    let source = Arc::new(Many::new(5, 3));
    let mut h = many(source.clone());
    let downloads = h.services.book_sources.downloads.clone();
    downloads.configure(Setup { hold: true, ..keeping_ready() });
    h.open_many(Some("1"));
    timeout(WAIT, h.leaves_of("1")).await.expect("laid out");
    h.wait_for(|h| h.download_state("2") == Some(State::Queued)).await;
    h.open_chapter("2");
    timeout(WAIT, h.leaves_of("2")).await.expect("laid out");
    h.wait_for(|h| h.download_state("2") == Some(State::Removed)).await;
    h.wait_for(|h| h.download_state("5") == Some(State::Queued)).await;
    downloads.configure(keeping_ready());
    downloads.start(&Lent::of(&h.services, &h.tx));
    h.wait_for(|h| h.download_state("5") == Some(State::Done)).await;
    for chapter in ["ch-2", "ch-3", "ch-4", "ch-5"] {
        assert!(source.each_once(chapter), "{chapter}: {:?}", source.asked_in(chapter));
    }
}
