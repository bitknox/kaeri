use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kaeri_core::{
    ProviderError,
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, PageRef, SeriesDetail,
        SeriesHit, SourceChapter,
    },
    config::Paths,
    media::Medium,
    traits::{BookSource, PageError},
};
use tokio::sync::mpsc;

use super::*;

const WORK: &str = "a:900501";

/// A page the sniffer takes for a PNG.
fn page() -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    bytes.extend_from_slice(&[0; 64]);
    bytes
}

/// A source with a list a test shapes: chapters of 3 pages, `text-` ones of text, and `gone`.
struct Shelf {
    manifest: BookSourceManifest,
    listed: Mutex<Vec<SourceChapter>>,
    fetched: Mutex<u32>,
}

#[async_trait]
impl BookSource for Shelf {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, "The Lantern Ferry"))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok(self.listed.lock().unwrap().clone())
    }

    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        if chapter.contains("gone") {
            return Err(ProviderError::NotFound);
        }
        if chapter.starts_with("text-") {
            return Ok(ChapterContent::Text("<p>The ferry left at dusk.</p>".into()));
        }
        Ok(ChapterContent::Images(
            (1..=3)
                .map(|n| PageRef::new(format!("https://pages.example/{chapter}/{n}")))
                .collect(),
        ))
    }

    async fn fetch_page(&self, _: &PageRef) -> Result<Vec<u8>, PageError> {
        *self.fetched.lock().unwrap() += 1;
        Ok(page())
    }
}

struct Kit {
    lent: Lent,
    shelf: Arc<Shelf>,
    queue: BookDownloads,
    events: mpsc::UnboundedReceiver<Event>,
    _dir: tempfile::TempDir,
}

impl Kit {
    fn store(&self) -> &Store {
        self.lent.sources.store()
    }

    fn setup(&self, setup: Setup) {
        self.queue.configure(setup);
    }

    async fn settle(&self) {
        for _ in 0..500 {
            if !*self.queue.0.running.lock().unwrap() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the queue did not settle");
    }

    fn states(&self) -> Vec<(String, ChapterDownloadState)> {
        let mut rows: Vec<_> = self.queue.states(self.store(), WORK).into_iter().collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows
    }

    fn drain(&mut self) -> Vec<ReadingEvent> {
        let mut out = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            if let Event::Reading(event) = event {
                out.push(event);
            }
        }
        out
    }

    fn read(&self, chapter: &str, at: i64) {
        let mut event = kaeri_store::reading::ReadEvent::new(WORK, chapter, at);
        event.completed = true;
        self.store().record_read(&event, Vec::new()).unwrap();
    }
}

fn kit(chapters: &[&str]) -> Kit {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open_in_memory().unwrap();
    let paths = Paths {
        config_file: dir.path().join("config.toml"),
        data_dir: dir.path().join("data"),
        cache_dir: dir.path().join("cache"),
    };
    let tokens = kaeri_track::TokenStore::new(dir.path().join("tokens"));
    let sources = BookSources::new(store.clone(), tokens, reqwest::Client::new(), &paths);
    let manifest = BookSourceManifest {
        id: "shelf".into(),
        name: "Shelf".into(),
        version: "1".into(),
        kind: BookSourceKind::Native,
        media: vec![Medium::Manga],
        languages: vec!["en".into()],
        allowed_hosts: Vec::new(),
        capabilities: Vec::new(),
    };
    let listed = chapters.iter().map(|id| SourceChapter::new(*id, *id)).collect();
    let shelf =
        Arc::new(Shelf { manifest, listed: Mutex::new(listed), fetched: Mutex::new(0) });
    sources.registry.put(shelf.clone());
    store.set_book_match("shelf", "s-1", WORK, 100, true, false, 1).unwrap();
    store.seed_book_work(WORK, "manga", None, None, "The Lantern Ferry", 1).unwrap();
    let (tx, events) = mpsc::unbounded_channel();
    let queue = sources.downloads.clone();
    let lent = Lent {
        sources,
        plugins: BookPlugins::default(),
        clock: Clock::default(),
        tx: Outbox::from(tx),
    };
    // As the engine sets it up once the front end says Wi-Fi.
    queue.configure(Setup::default());
    Kit { lent, shelf, queue, events, _dir: dir }
}

fn ids(list: &[&str]) -> Vec<ChapterId> {
    list.iter().map(|c| ChapterId::new(*c)).collect()
}

#[tokio::test]
async fn a_chapter_downloads_into_a_cbz_the_shelf_reads() {
    let mut kit = kit(&["1", "2", "3"]);
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::NextUnread { n: 2 }).await;
    kit.settle().await;

    assert_eq!(
        kit.states(),
        [("1".into(), ChapterDownloadState::Done), ("2".into(), ChapterDownloadState::Done)]
    );
    let shelf = kit.store().shelf(WORK).unwrap();
    assert_eq!(shelf.len(), 2);
    assert_eq!((shelf[0].format.as_str(), shelf[0].pages), ("cbz", Some(3)));
    // The file is a whole CBZ the books crate opens, with its three pages.
    match kaeri_books::open(
        std::path::Path::new(&shelf[0].path),
        &kaeri_books::Limits::default(),
    )
    .unwrap()
    {
        kaeri_books::Book::Comic(comic) => assert_eq!(comic.pages().len(), 3),
        _ => panic!("a downloaded chapter of pages opens as a comic"),
    }
    assert!(
        std::fs::read_dir(kit.queue.dir().join(safe(WORK)))
            .unwrap()
            .all(|f| { f.unwrap().path().extension().is_some_and(|e| e == "cbz") })
    );

    // The meter counts what is on disk, and the last event says Done for both.
    let view = kit.queue.view(kit.store());
    assert_eq!(view.meter.chapters, 2);
    assert!(view.meter.used > 0);
    assert_eq!(view.rows.len(), 2);
    assert_eq!(view.rows[0].title, "The Lantern Ferry");
    assert!(view.rows.iter().all(|r| r.state == ChapterDownloadState::Done));
    let events = kit.drain();
    assert!(events.iter().any(|e| matches!(e, ReadingEvent::Downloads(_))));
    // The chapter list is told again, with the downloads on its rows.
    let relisted = events.iter().rev().find_map(|e| match e {
        ReadingEvent::Chapters { volumes, .. } => Some(volumes.clone()),
        _ => None,
    });
    let rows: Vec<_> = relisted.unwrap().into_iter().flat_map(|g| g.chapters).collect();
    assert_eq!(rows[0].download, Some(ChapterDownloadState::Done));
    assert_eq!(rows[2].download, None);
}

#[tokio::test]
async fn downloads_finished_is_said_once_the_asked_chapters_are_done_and_only_when_on() {
    // Off: the run ends without a word.
    let mut kit = kit(&["1", "2", "3"]);
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::NextUnread { n: 1 }).await;
    kit.settle().await;
    assert!(!kit.drain().iter().any(|e| matches!(e, ReadingEvent::DownloadsDone { .. })));

    // On: one notice for the run, naming the series and counting its chapters.
    kit.setup(Setup { tell_done: true, ..Setup::default() });
    kit.queue
        .download(
            &kit.lent,
            WorkId::new(WORK),
            DownloadPick::Chapters { ids: ids(&["2", "3"]) },
        )
        .await;
    kit.settle().await;
    let said: Vec<_> = kit
        .drain()
        .into_iter()
        .filter_map(|e| match e {
            ReadingEvent::DownloadsDone { work, body, .. } => Some((work, body)),
            _ => None,
        })
        .collect();
    assert_eq!(
        said,
        [(
            Some(WorkId::new(WORK)),
            Msg::Reading(ReadingMsg::ReadingDownloadsDoneChapters {
                title: "The Lantern Ferry".into(),
                count: 2
            })
        )]
    );
}

#[tokio::test]
async fn the_total_counts_parts_apart_and_every_series_line_says_its_size() {
    let kit = kit(&["1", "2"]);
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::Unread).await;
    kit.settle().await;
    let view = kit.queue.view(kit.store());
    let words = |line: &[Msg]| line.iter().map(Msg::id).collect::<Vec<_>>();
    assert_eq!(words(&view.total), ["reading-chapters", "reading-dl-size"]);
    assert_eq!(words(&view.series[0].line)[..2], ["reading-chapters", "reading-dl-size"]);

    // A light novel's downloads are parts in the total.
    kit.store().seed_book_work(WORK, "novel", None, None, "The Lantern Ferry", 2).unwrap();
    let view = kit.queue.view(kit.store());
    assert_eq!(words(&view.total), ["reading-dl-parts", "reading-dl-size"]);
}

#[tokio::test]
async fn a_text_chapter_downloads_as_a_single_chapter_epub() {
    let kit = kit(&[]);
    kit.shelf.listed.lock().unwrap().push(SourceChapter::new("text-1", "1"));
    kit.queue
        .download(&kit.lent, WorkId::new(WORK), DownloadPick::Chapters { ids: ids(&["1"]) })
        .await;
    kit.settle().await;
    let shelf = kit.store().shelf(WORK).unwrap();
    assert_eq!((shelf.len(), shelf[0].format.as_str()), (1, "epub"));
    assert!(std::fs::metadata(&shelf[0].path).unwrap().len() > 0);
}

#[tokio::test]
async fn nothing_is_fetched_on_cellular_and_rows_wait_for_wifi_until_it_returns() {
    let kit = kit(&["1", "2"]);
    let cellular = Setup { hold: true, wifi: true, ..Setup::default() };
    kit.setup(cellular);
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::Unread).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(*kit.shelf.fetched.lock().unwrap(), 0);
    assert!(kit.states().iter().all(|(_, s)| *s == ChapterDownloadState::WaitsForWifi));

    // Offline holds too, but the rows are only queued.
    kit.setup(Setup { hold: true, wifi: false, ..Setup::default() });
    assert!(kit.states().iter().all(|(_, s)| *s == ChapterDownloadState::Queued));

    // Wi-Fi is back: the queue goes on by itself.
    assert!(kit.queue.configure(Setup::default()));
    kit.queue.start(&kit.lent);
    kit.settle().await;
    assert!(kit.states().iter().all(|(_, s)| *s == ChapterDownloadState::Done));
}

#[test]
fn setup_follows_the_network_the_config_and_the_edition() {
    let mut config = Config::default();
    let wifi = Some(Network::default());
    let cell = Some(Network { cellular: true, ..Network::default() });
    let on = |network, config: &Config| {
        Setup::of(AppState::Foreground, network, config, AppEdition::Full)
    };
    assert_eq!(on(wifi, &config), Setup { wifi_only: true, ..Setup::default() });
    assert_eq!((on(cell, &config).hold, on(cell, &config).wifi), (true, true));
    // Before the front end says the network, nothing is fetched, whatever the setting: it may
    // be cellular. The rows only wait, as nothing says it is for Wi-Fi yet.
    let unsaid = on(None, &config);
    assert!(unsaid.hold && !unsaid.wifi && !unsaid.offline);
    assert!(!on(cell, &config).mobile, "held for Wi-Fi, so nothing comes over mobile data");
    config.reading.downloads_on_cellular = true;
    assert!(!on(cell, &config).hold);
    assert!(on(None, &config).hold);
    assert!(on(cell, &config).mobile && !on(cell, &config).wifi_only);
    assert!(!on(wifi, &config).mobile);
    let low = Some(Network { constrained: true, ..Network::default() });
    assert!(on(low, &config).hold && on(low, &config).wifi);
    let offline = Some(Network { offline: true, ..Network::default() });
    assert!(on(offline, &config).hold && !on(offline, &config).wifi);
    assert!(on(offline, &config).offline);
    let away =
        Setup::of(AppState::Background { playing: false }, wifi, &config, AppEdition::Full);
    assert!(away.hold && !away.wifi);
    let window = Setup::of(AppState::BackgroundDownloads, wifi, &config, AppEdition::Full);
    assert!(!window.hold);

    // Keeping ready is on for the e-reader build unless the person turned it off.
    let reader = |config: &Config| {
        Setup::of(AppState::Foreground, wifi, config, AppEdition::ReadOnly).keep_ready
    };
    assert!(reader(&config));
    assert!(!on(wifi, &config).keep_ready);
    config.reading.keep_chapters_ready = Some(false);
    assert!(!reader(&config));
    config.reading.keep_chapters_ready = Some(true);
    assert!(on(wifi, &config).keep_ready);
}

#[tokio::test]
async fn a_source_that_fails_marks_the_chapter_and_the_rest_go_on() {
    let kit = kit(&["1", "3"]);
    kit.shelf.listed.lock().unwrap().push(SourceChapter::new("gone", "2"));
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::Unread).await;
    kit.settle().await;
    assert_eq!(
        kit.states(),
        [
            ("1".into(), ChapterDownloadState::Done),
            ("2".into(), ChapterDownloadState::Failed { failure: ReadingFailure::NotFound }),
            ("3".into(), ChapterDownloadState::Done),
        ]
    );
    // Asking again queues the failed one once more.
    kit.queue
        .download(&kit.lent, WorkId::new(WORK), DownloadPick::Chapters { ids: ids(&["2"]) })
        .await;
    kit.settle().await;
    assert!(matches!(
        kit.states()[1].1,
        ChapterDownloadState::Failed { failure: ReadingFailure::NotFound }
    ));
}

#[tokio::test]
async fn a_full_device_stops_the_queue_with_the_chapter_that_met_it() {
    let kit = kit(&["1", "2"]);
    *kit.queue.0.free.lock().unwrap() = Some(KEEP_FREE - 1);
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::Unread).await;
    kit.settle().await;
    assert_eq!(
        kit.states(),
        [
            ("1".into(), ChapterDownloadState::Failed { failure: ReadingFailure::DiskFull }),
            ("2".into(), ChapterDownloadState::Queued),
        ]
    );
    assert_eq!(kit.queue.view(kit.store()).meter.free, KEEP_FREE - 1);
    assert_eq!(*kit.shelf.fetched.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_pick_names_unread_chapters_a_volume_or_the_ones_asked_for() {
    let kit = kit(&["1", "2", "3", "4"]);
    kit.shelf.listed.lock().unwrap().iter_mut().for_each(|c| {
        c.volume = Some(if c.id.as_str() < "3" { "1".into() } else { "2".into() });
    });
    kit.read("1", 5);
    let work = WorkId::new(WORK);
    kit.setup(Setup { hold: true, ..Setup::default() });

    kit.queue.download(&kit.lent, work.clone(), DownloadPick::NextUnread { n: 2 }).await;
    let queued: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(queued, ["2", "3"]);

    kit.queue
        .download(&kit.lent, work.clone(), DownloadPick::Volume { volume: "2".into() })
        .await;
    let queued: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(queued, ["2", "3", "4"]);
}

#[tokio::test]
async fn cancel_takes_queued_chapters_out_and_leaves_finished_ones() {
    let kit = kit(&["1", "2", "3"]);
    let work = WorkId::new(WORK);
    kit.queue
        .download(&kit.lent, work.clone(), DownloadPick::Chapters { ids: ids(&["1"]) })
        .await;
    kit.settle().await;
    kit.setup(Setup { hold: true, ..Setup::default() });
    kit.queue
        .download(&kit.lent, work.clone(), DownloadPick::Chapters { ids: ids(&["2", "3"]) })
        .await;
    kit.queue.cancel(&kit.lent, &work, &ids(&["3"]));
    let left: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(left, ["1", "2"]);
    kit.queue.cancel(&kit.lent, &work, &[]);
    let left: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(left, ["1"]);
}

#[tokio::test]
async fn delete_removes_files_and_rows_and_empty_means_the_whole_series() {
    let kit = kit(&["1", "2", "3"]);
    let work = WorkId::new(WORK);
    kit.queue.download(&kit.lent, work.clone(), DownloadPick::Unread).await;
    kit.settle().await;
    let paths: Vec<String> =
        kit.store().shelf(WORK).unwrap().into_iter().map(|f| f.path).collect();
    assert_eq!(paths.len(), 3);

    assert_eq!(kit.queue.delete(&kit.lent, &work, &ids(&["2"])), 1);
    assert!(!std::path::Path::new(&paths[1]).exists());
    assert!(std::path::Path::new(&paths[0]).exists());
    assert_eq!(kit.queue.view(kit.store()).meter.chapters, 2);

    assert_eq!(kit.queue.delete(&kit.lent, &work, &[]), 2);
    assert!(paths.iter().all(|p| !std::path::Path::new(p).exists()));
    assert!(kit.queue.view(kit.store()).rows.is_empty());
}

#[tokio::test]
async fn delete_after_reading_lets_go_of_read_chapters_but_never_a_bookmarked_one() {
    let kit = kit(&["1", "2", "3"]);
    let work = WorkId::new(WORK);
    kit.queue.download(&kit.lent, work.clone(), DownloadPick::Unread).await;
    kit.settle().await;
    kit.store().set_bookmark(WORK, "2", true, 3).unwrap();
    kit.read("1", 10);
    kit.read("2", 11);

    // With the setting off, a read chapter stays.
    assert_eq!(kit.queue.sweep(&kit.lent, Some(WORK)), 0);
    kit.setup(Setup { delete_after: true, ..Setup::default() });
    assert_eq!(kit.queue.sweep(&kit.lent, None), 1);
    let left: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(left, ["2", "3"]);
    // Taking the bookmark off lets the next sweep go.
    kit.store().set_bookmark(WORK, "2", false, 12).unwrap();
    assert_eq!(kit.queue.sweep(&kit.lent, None), 1);
    let left: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(left, ["3"]);
}

#[tokio::test]
async fn keeping_ready_queues_the_next_three_and_reading_past_one_lets_it_go() {
    let mut kit = kit(&["1", "2", "3", "4", "5", "6", "7"]);
    let work = WorkId::new(WORK);

    // Off: reading a chapter fetches nothing.
    kit.queue.keep_ready(&kit.lent, &work, &ChapterId::new("1")).await;
    assert!(kit.states().is_empty());

    kit.setup(Setup { keep_ready: true, ..Setup::default() });
    kit.queue.keep_ready(&kit.lent, &work, &ChapterId::new("2")).await;
    kit.settle().await;
    let ready: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(ready, ["3", "4", "5"]);
    assert!(kit.store().book_downloads(Some(WORK)).unwrap().iter().all(|d| d.keep_ready));
    kit.drain();

    // Reading chapter 3 through lets it go, whatever delete after reading says, and tops up.
    kit.read("3", 20);
    assert_eq!(kit.queue.sweep(&kit.lent, Some(WORK)), 1);
    kit.queue.keep_ready(&kit.lent, &work, &ChapterId::new("3")).await;
    kit.settle().await;
    let ready: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(ready, ["4", "5", "6"]);

    // A chapter let go of by hand is not fetched again on its own, but is when asked for.
    kit.queue.delete(&kit.lent, &work, &ids(&["6"]));
    kit.queue.keep_ready(&kit.lent, &work, &ChapterId::new("3")).await;
    kit.settle().await;
    let ready: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(ready, ["4", "5"]);
    kit.queue.download(&kit.lent, work, DownloadPick::Chapters { ids: ids(&["6"]) }).await;
    kit.settle().await;
    let row = kit.store().book_download(WORK, "6").unwrap().unwrap();
    assert!(
        !row.keep_ready && row.state == kaeri_store::book_downloads::BookDownloadState::Done
    );
}

#[tokio::test]
async fn a_chapter_asked_for_is_not_let_go_of_by_reading_past_it_without_the_setting() {
    let kit = kit(&["1", "2"]);
    let work = WorkId::new(WORK);
    kit.queue.download(&kit.lent, work, DownloadPick::Unread).await;
    kit.settle().await;
    kit.read("1", 9);
    assert_eq!(kit.queue.sweep(&kit.lent, Some(WORK)), 0);
    assert_eq!(kit.states().len(), 2);
}

#[tokio::test]
async fn downloads_cut_short_are_queued_again_and_half_files_go() {
    let kit = kit(&["1"]);
    let work = WorkId::new(WORK);
    kit.setup(Setup { hold: true, ..Setup::default() });
    kit.queue.download(&kit.lent, work, DownloadPick::Unread).await;
    let row = kit.store().next_book_download().unwrap().unwrap();
    kit.store().book_download_progress(row.id, 1, Some(3), 5).unwrap();
    let dir = kit.queue.dir().join(safe(WORK));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("1.part"), b"half").unwrap();

    kit.queue.recover(kit.store(), 6);
    assert!(!dir.join("1.part").exists());
    assert_eq!(kit.states()[0].1, ChapterDownloadState::Queued);
}

#[tokio::test]
async fn turning_reading_off_with_the_deletion_asked_clears_every_download() {
    let kit = kit(&["1", "2"]);
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::Unread).await;
    kit.settle().await;
    assert_eq!(kit.queue.delete_all(&kit.lent), 2);
    assert!(kit.store().shelf(WORK).unwrap().is_empty());
    assert!(std::fs::read_dir(kit.queue.dir().join(safe(WORK))).unwrap().next().is_none());
}

#[test]
fn failures_survive_the_row() {
    for failure in [
        ReadingFailure::Offline,
        ReadingFailure::DiskFull,
        ReadingFailure::NotFound,
        ReadingFailure::PageFetch { status: 403 },
        ReadingFailure::SourceDown { source: "shelf".into() },
        ReadingFailure::Blocked { source: "shelf".into(), reason: "slow down: later".into() },
        ReadingFailure::SignInNeeded { what: "shelf".into() },
    ] {
        assert_eq!(decode(&encode(&failure)), failure);
    }
}

#[test]
fn ids_with_slashes_make_safe_distinct_file_names() {
    assert_eq!(safe("a:900501"), "a~3a900501");
    assert_ne!(safe("v2/1"), safe("v2_1"));
    assert!(!safe("x/-/extra").contains('/'));
}

fn msg_ids(parts: &[kaeri_model::Msg]) -> Vec<&'static str> {
    parts.iter().map(kaeri_model::Msg::id).collect()
}

#[tokio::test]
async fn each_series_says_what_it_takes_and_delete_read_spares_bookmarks() {
    let kit = kit(&["1", "2", "3", "4"]);
    let work = WorkId::new(WORK);
    kit.queue.download(&kit.lent, work.clone(), DownloadPick::NextUnread { n: 3 }).await;
    kit.settle().await;
    kit.store().set_bookmark(WORK, "2", true, 3).unwrap();
    kit.read("1", 10);
    kit.read("2", 11);

    let view = kit.queue.view(kit.store());
    let [series] = view.series.as_slice() else { panic!("one series: {:?}", view.series) };
    assert_eq!((series.chapters, series.read, series.queued), (3, 1, 0), "the bookmark stays");
    assert_eq!(series.bytes, view.meter.used, "every byte is the one series'");
    assert_eq!(series.title, "The Lantern Ferry");
    assert_eq!(
        msg_ids(&series.line),
        ["reading-chapters", "reading-dl-size", "reading-dl-read"]
    );
    let one = view.meter.used / 3;
    assert!(view.meter.read_bytes >= 2 * one - 2 && view.meter.read_bytes < view.meter.used);
    assert_eq!(msg_ids(&view.total), ["reading-chapters", "reading-dl-size"]);

    // Delete read: the read chapter goes whatever the settings say, the bookmarked one stays.
    assert_eq!(kit.queue.delete_read(&kit.lent, &work), 1);
    let left: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(left, ["2", "3"]);
    let view = kit.queue.view(kit.store());
    assert_eq!(msg_ids(&view.series[0].line), ["reading-chapters", "reading-dl-size"]);
}

#[test]
fn a_series_waiting_or_failing_says_so_in_its_line() {
    let kit = kit(&["1", "2"]);
    let store = kit.store();
    let ask = |chapter| BookDownloadAsk {
        work: WORK,
        chapter,
        source_id: "shelf",
        version_label: None,
        keep_ready: false,
    };
    store.queue_book_download(&ask("1"), 1).unwrap();
    store.queue_book_download(&ask("2"), 1).unwrap();
    kit.setup(Setup { hold: true, wifi: true, ..Setup::default() });
    let view = kit.queue.view(store);
    assert_eq!(msg_ids(&view.series[0].line), ["reading-dl-queued", "reading-dl-waits-wifi"]);
    assert_eq!(view.series[0].queued, 2);
    assert!(view.total.is_empty(), "nothing downloaded yet");

    let row = store.book_download(WORK, "2").unwrap().unwrap();
    store.book_download_failed(row.id, &encode(&ReadingFailure::Offline), 2).unwrap();
    let view = kit.queue.view(store);
    assert_eq!(msg_ids(&view.series[0].line), ["reading-dl-failed", "reading-failed-offline"]);
    assert_eq!(view.series[0].failed, ids(&["2"]), "Retry asks for these again");
}

fn listed(id: &str, pages: Option<u32>) -> ChapterRow {
    ChapterRow {
        id: ChapterId::new(id),
        label: id.into(),
        state: ChapterReadState::Unread,
        number: Some(id.into()),
        title: None,
        volume: None,
        published: None,
        pages,
        new_chapter: false,
        download: None,
        versions: 1,
        bookmarked: false,
        version_label: None,
        read_at: None,
        line: Vec::new(),
        gone_from_source: false,
    }
}

#[tokio::test]
async fn the_estimate_counts_pages_at_the_size_past_pages_took_and_names_the_network() {
    let kit = kit(&["1", "2", "3"]);
    let work = WorkId::new(WORK);
    let rows = [listed("1", Some(10)), listed("2", None), listed("3", Some(40))];
    let pick = DownloadPick::Chapters { ids: ids(&["1", "2", "3"]) };

    // Nothing downloaded before: the usual page.
    kit.setup(Setup { wifi_only: true, ..Setup::default() });
    let first = kit.queue.estimate(kit.store(), &work, pick.clone(), &rows, Some("shelf"));
    assert_eq!(first.chapters, 3);
    assert_eq!(
        first.bytes,
        (10 + 20 + 40) * 350_000,
        "a chapter of unknown length is 20 pages"
    );
    assert_eq!(msg_ids(&first.line), ["reading-estimate-size", "reading-estimate-wifi"]);

    // Chapter 1 downloaded, three pages: its pages set the size, and it costs nothing more.
    kit.queue
        .download(&kit.lent, work.clone(), DownloadPick::Chapters { ids: ids(&["1"]) })
        .await;
    kit.settle().await;
    let done = kit.store().book_download(WORK, "1").unwrap().unwrap();
    let page = done.bytes / 3;
    kit.setup(Setup { mobile: true, ..Setup::default() });
    let next = kit.queue.estimate(kit.store(), &work, pick.clone(), &rows, Some("shelf"));
    assert_eq!(next.chapters, 2);
    assert_eq!(next.bytes, (3 + 40) * page, "an unknown length is the usual length here");
    assert_eq!(msg_ids(&next.line), ["reading-estimate-size", "reading-estimate-mobile"]);

    kit.setup(Setup { hold: true, wifi: true, wifi_only: true, ..Setup::default() });
    let waiting = kit.queue.estimate(kit.store(), &work, pick, &rows, Some("shelf"));
    assert_eq!(msg_ids(&waiting.line)[1], "reading-estimate-waits-wifi");
    let none = DownloadPick::Chapters { ids: ids(&["1"]) };
    let nothing = kit.queue.estimate(kit.store(), &work, none, &rows, Some("shelf"));
    assert_eq!((nothing.chapters, nothing.bytes, nothing.line.len()), (0, 0, 0));
}

#[tokio::test]
async fn a_downloaded_chapter_the_source_dropped_stays_listed_and_opens() {
    let kit = kit(&["1", "2", "3", "4"]);
    kit.queue
        .download(
            &kit.lent,
            WorkId::new(WORK),
            DownloadPick::Chapters { ids: ids(&["2", "3"]) },
        )
        .await;
    kit.settle().await;
    kit.read("2", 1);
    // The source drops 2, which is downloaded, and 4, which is not.
    kit.shelf.listed.lock().unwrap().retain(|c| c.id != "2" && c.id != "4");

    let (_, volumes) = kit.lent.sources.chapters(&WorkId::new(WORK), None, 2).await.unwrap();
    let rows: Vec<_> = volumes.into_iter().flat_map(|g| g.chapters).collect();
    let listed: Vec<(&str, bool)> =
        rows.iter().map(|r| (r.id.as_str(), r.gone_from_source)).collect();
    assert_eq!(listed, [("1", false), ("2", true), ("3", false)], "in its place, 4 gone");
    let gone = &rows[1];
    assert_eq!(gone.download, Some(ChapterDownloadState::Done));
    assert_eq!(gone.state, ChapterReadState::Read);

    // The file is still on the shelf the reader opens from.
    let shelf = kit.store().shelf(WORK).unwrap();
    assert!(shelf.iter().any(|f| f.chapter.as_deref() == Some("2")), "{shelf:?}");
}
