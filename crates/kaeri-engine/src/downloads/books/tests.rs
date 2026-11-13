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

/// A source with a list a test shapes: chapters of 3 pages, `text-` ones of text, `made-` ones
/// of pages it makes itself, and `gone`.
struct Shelf {
    manifest: BookSourceManifest,
    listed: Mutex<Vec<SourceChapter>>,
    fetched: Mutex<u32>,
    /// The addresses fetched, in order.
    asked: Mutex<Vec<String>>,
    /// Pages at these addresses answer 404.
    broken: Mutex<Vec<String>>,
    /// The page at this address waits: see [`Held`].
    held: Mutex<Option<Held>>,
    /// The chapters whose pages were asked for, in order.
    contents: Mutex<Vec<String>>,
    /// Each ask of these addresses waits for the next answer here: `true` brings the page,
    /// `false` a 404.
    gates: Mutex<std::collections::HashMap<String, std::collections::VecDeque<Gate>>>,
    /// The gated addresses whose request was dropped before its answer came.
    cut: Mutex<Vec<String>>,
    /// Told on every ask and every cut.
    knock: tokio::sync::Notify,
}

type Gate = tokio::sync::oneshot::Receiver<bool>;

/// A gated request dropped before its answer came says so.
struct Cut<'a>(&'a Shelf, String, bool);

impl Drop for Cut<'_> {
    fn drop(&mut self) {
        if !self.2 {
            self.0.cut.lock().unwrap().push(self.1.clone());
            self.0.knock.notify_waiters();
        }
    }
}

impl Shelf {
    /// The next `n` asks of `url` wait for the answers sent here.
    fn gate(&self, url: &str, n: usize) -> Vec<tokio::sync::oneshot::Sender<bool>> {
        let (senders, gates): (Vec<_>, std::collections::VecDeque<_>) =
            (0..n).map(|_| tokio::sync::oneshot::channel()).unzip();
        self.gates.lock().unwrap().insert(url.to_owned(), gates);
        senders
    }

    fn times(&self, url: &str) -> usize {
        self.asked.lock().unwrap().iter().filter(|a| *a == url).count()
    }

    /// Until `done` holds, for ten seconds at most.
    async fn until(&self, done: impl Fn(&Self) -> bool) {
        let wait = async {
            loop {
                let knocked = self.knock.notified();
                tokio::pin!(knocked);
                knocked.as_mut().enable();
                if done(self) {
                    return;
                }
                knocked.await;
            }
        };
        tokio::time::timeout(Duration::from_secs(10), wait).await.expect("never came");
    }
}

/// A page held back: its address, a notify that says it was asked and one that lets it answer.
type Held = (String, Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>);

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
        self.contents.lock().unwrap().push(chapter.to_owned());
        if chapter.contains("gone") {
            return Err(ProviderError::NotFound);
        }
        if chapter.starts_with("text-") {
            return Ok(ChapterContent::Text("<p>The ferry left at dusk.</p>".into()));
        }
        if chapter.starts_with("pictured-") {
            return Ok(ChapterContent::Text(
                "<p>The ferry left at dusk.</p>\
                 <img src=\"https://pages.example/pictures/pier.png\" alt=\"The pier\"/>\
                 <img src=\"https://elsewhere.example/pictures/gull.png\"/>"
                    .into(),
            ));
        }
        if chapter.starts_with("made-") {
            return Ok(ChapterContent::Images(
                (1..=3)
                    .map(|n| {
                        PageRef::made("s-1", chapter, format!("https://pages.example/{n}"))
                    })
                    .collect(),
            ));
        }
        Ok(ChapterContent::Images(
            (1..=3)
                .map(|n| PageRef::new(format!("https://pages.example/{chapter}/{n}")))
                .collect(),
        ))
    }

    async fn fetch_page(&self, page_ref: &PageRef) -> Result<Vec<u8>, PageError> {
        *self.fetched.lock().unwrap() += 1;
        let asked = match &page_ref.made {
            Some(made) => format!("made {} {}", made.chapter, page_ref.url),
            None => page_ref.url.clone(),
        };
        self.asked.lock().unwrap().push(asked);
        self.knock.notify_waiters();
        let gate =
            self.gates.lock().unwrap().get_mut(&page_ref.url).and_then(|g| g.pop_front());
        if let Some(gate) = gate {
            let mut cut = Cut(self, page_ref.url.clone(), false);
            let answer = gate.await.unwrap_or(false);
            cut.2 = true;
            if !answer {
                return Err(PageError::Status(404));
            }
        }
        let held = self.held.lock().unwrap().clone();
        if let Some((_, asked, answer)) = held.filter(|(url, ..)| *url == page_ref.url) {
            asked.notify_one();
            answer.notified().await;
        }
        if self.broken.lock().unwrap().contains(&page_ref.url) {
            return Err(PageError::Status(404));
        }
        if page_ref.url.contains("/pictures/") {
            return Ok(crate::read_sources::pictures::ONE_PIXEL_PNG.to_vec());
        }
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
    kit_of(chapters, &[])
}

fn kit_of(chapters: &[&str], hosts: &[&str]) -> Kit {
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
        allowed_hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
        capabilities: Vec::new(),
    };
    let listed = chapters.iter().map(|id| SourceChapter::new(*id, *id)).collect();
    let shelf = Arc::new(Shelf {
        manifest,
        listed: Mutex::new(listed),
        fetched: Mutex::new(0),
        asked: Mutex::default(),
        broken: Mutex::default(),
        held: Mutex::default(),
        contents: Mutex::default(),
        gates: Mutex::default(),
        cut: Mutex::default(),
        knock: tokio::sync::Notify::new(),
    });
    sources.registry.put(shelf.clone());
    store.set_book_match("shelf", "s-1", WORK, 100, true, false, 1).unwrap();
    store.seed_book_work(WORK, "manga", None, None, "The Lantern Ferry", 1).unwrap();
    let (tx, events) = mpsc::unbounded_channel();
    let queue = sources.downloads.clone();
    let http = kaeri_net::HttpClient::offline(&Default::default()).unwrap();
    let lent = Lent {
        sources,
        plugins: BookPlugins::default(),
        clock: Clock::default(),
        tx: Outbox::from(tx),
        meta: kaeri_meta::Meta::new(kaeri_meta::AniList::new(http, 30), None, store.clone()),
        offline: Arc::default(),
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
async fn next_unread_counts_from_the_place_and_passes_over_what_is_downloaded() {
    // Chapters 1 and 2 were passed over; 3 and 4 were read, so the place is chapter 5.
    let kit = kit(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    kit.read("3", 5);
    kit.read("4", 6);
    let next = DownloadPick::NextUnread { n: 2 };
    kit.queue.download(&kit.lent, WorkId::new(WORK), next.clone()).await;
    kit.settle().await;
    let done: Vec<String> = kit.states().into_iter().map(|(id, _)| id).collect();
    assert_eq!(done, ["5", "6"]);

    // A second ask fetches the two after them, not the same two again.
    kit.queue.download(&kit.lent, WorkId::new(WORK), next.clone()).await;
    kit.settle().await;
    let done: Vec<String> = kit.states().into_iter().map(|(id, _)| id).collect();
    assert_eq!(done, ["5", "6", "7", "8"]);

    // With the rest on this device too, nothing more is asked for, not the ones passed over.
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::NextUnread { n: 5 }).await;
    kit.settle().await;
    kit.queue.download(&kit.lent, WorkId::new(WORK), next).await;
    kit.settle().await;
    let done: Vec<String> = kit.states().into_iter().map(|(id, _)| id).collect();
    assert_eq!(done, ["5", "6", "7", "8", "9"]);
}

#[tokio::test]
async fn a_chapter_of_pages_the_source_makes_downloads_as_one_it_fetches_does() {
    let kit = kit(&[]);
    kit.shelf.listed.lock().unwrap().push(SourceChapter::new("made-1", "1"));
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::NextUnread { n: 1 }).await;
    kit.settle().await;

    assert_eq!(kit.states(), [("1".into(), ChapterDownloadState::Done)]);
    // Each page went to the source with its chapter, one at a time and in order.
    assert_eq!(
        *kit.shelf.asked.lock().unwrap(),
        [
            "made made-1 https://pages.example/1",
            "made made-1 https://pages.example/2",
            "made made-1 https://pages.example/3"
        ]
    );
    let shelf = kit.store().shelf(WORK).unwrap();
    assert_eq!((shelf[0].format.as_str(), shelf[0].pages), ("cbz", Some(3)));
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
async fn a_text_chapters_pictures_on_the_plugins_hosts_go_inside_its_epub() {
    let kit = kit_of(&[], &["pages.example"]);
    kit.shelf.listed.lock().unwrap().push(SourceChapter::new("pictured-1", "1"));
    kit.queue
        .download(&kit.lent, WorkId::new(WORK), DownloadPick::Chapters { ids: ids(&["1"]) })
        .await;
    kit.settle().await;
    let shelf = kit.store().shelf(WORK).unwrap();
    assert_eq!((shelf.len(), shelf[0].format.as_str()), (1, "epub"));
    let asked = kit.shelf.asked.lock().unwrap().clone();
    assert_eq!(asked, ["https://pages.example/pictures/pier.png"], "no other host is asked");

    let limits = kaeri_books::Limits::default();
    let book = kaeri_books::open(std::path::Path::new(&shelf[0].path), &limits);
    let Ok(kaeri_books::Book::Epub(mut epub)) = book else { panic!("an epub") };
    let chapter = epub.spine().next().map(|(_, item)| item.path.clone()).unwrap();
    let picture = chapter
        .rsplit_once('/')
        .map_or("images/1.png".to_owned(), |(dir, _)| format!("{dir}/images/1.png"));
    assert_eq!(
        epub.resource(&picture).unwrap().data,
        crate::read_sources::pictures::ONE_PIXEL_PNG
    );
    let text = String::from_utf8(epub.resource(&chapter).unwrap().data).unwrap();
    assert!(text.contains("src=\"images/1.png\""), "{text}");
}

#[tokio::test]
async fn a_downloaded_chapter_keeps_the_language_its_source_gives() {
    let kit = kit(&[]);
    for (id, label) in [("text-1", "1"), ("2", "2")] {
        let mut chapter = SourceChapter::new(id, label);
        chapter.language = Some("ja".into());
        kit.shelf.listed.lock().unwrap().push(chapter);
    }
    kit.queue
        .download(
            &kit.lent,
            WorkId::new(WORK),
            DownloadPick::Chapters { ids: ids(&["1", "2"]) },
        )
        .await;
    kit.settle().await;
    let shelf = kit.store().shelf(WORK).unwrap();
    assert_eq!(shelf.len(), 2);
    let limits = kaeri_books::Limits::default();
    for file in &shelf {
        match kaeri_books::open(std::path::Path::new(&file.path), &limits).unwrap() {
            // The text is set as Japanese offline too, as it is read from the source.
            kaeri_books::Book::Epub(mut epub) => {
                assert_eq!(epub.metadata().languages, ["ja"]);
                let page = epub.spine().map(|(_, item)| item.path.clone()).next().unwrap();
                let text = String::from_utf8(epub.resource(&page).unwrap().data).unwrap();
                assert!(text.contains("xml:lang=\"ja\""), "{text}");
            }
            kaeri_books::Book::Comic(comic) => {
                assert_eq!(comic.info().and_then(|i| i.language.as_deref()), Some("ja"));
            }
            _ => panic!("a chapter downloads as an EPUB or a CBZ"),
        }
    }
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

/// The queue a full device stopped says so, as chapter 2 still shows as queued with nothing
/// fetching it, and asking again for the chapter that met it starts the queue once more.
#[tokio::test]
async fn a_queue_stopped_by_a_full_device_says_so_until_it_starts_again() {
    let kit = kit(&["1", "2"]);
    assert!(!kit.queue.view(kit.store()).stopped_full);
    *kit.queue.0.free.lock().unwrap() = Some(KEEP_FREE - 1);
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::Unread).await;
    kit.settle().await;
    let stopped = kit.queue.view(kit.store());
    assert!(stopped.stopped_full);
    // The series' row says what to do, and that Retry takes the queued chapter on with it.
    assert!(
        stopped.series[0]
            .line
            .contains(&Msg::Reading(ReadingMsg::ReadingDlNoRoom { count: 1 })),
        "{:?}",
        stopped.series[0].line
    );

    *kit.queue.0.free.lock().unwrap() = Some(KEEP_FREE * 4);
    kit.queue
        .download(&kit.lent, WorkId::new(WORK), DownloadPick::Chapters { ids: ids(&["1"]) })
        .await;
    kit.settle().await;
    assert!(!kit.queue.view(kit.store()).stopped_full);
    assert!(kit.states().iter().all(|(_, s)| *s == ChapterDownloadState::Done));
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

/// A chapter the series page lists from another source downloads from that source, in its
/// place among the rest, so Download next and reading on offline do not stop at it.
#[tokio::test]
async fn a_chapter_listed_from_another_source_downloads_from_there() {
    let mut kit = kit(&["1", "2", "3"]);
    let other = Arc::new(Shelf {
        manifest: BookSourceManifest {
            id: "other".into(),
            name: "Other shelf".into(),
            ..kit.shelf.manifest.clone()
        },
        listed: Mutex::new(
            ["1", "2", "2.5", "3"].map(|id| SourceChapter::new(id, id)).to_vec(),
        ),
        fetched: Mutex::new(0),
        asked: Mutex::default(),
        broken: Mutex::default(),
        held: Mutex::default(),
        contents: Mutex::default(),
        gates: Mutex::default(),
        cut: Mutex::default(),
        knock: tokio::sync::Notify::new(),
    });
    kit.lent.sources.registry.put(other.clone());
    kit.store().set_book_match("other", "s-2", WORK, 90, true, false, 1).unwrap();
    // Read in the first source, as picked on the series page.
    let pref = kaeri_store::reading::EditionPref {
        source_id: "shelf".into(),
        version_label: None,
        language: None,
    };
    kit.store().set_edition_pref(WORK, Some(&pref)).unwrap();
    let work = WorkId::new(WORK);
    let now = kit.lent.clock.now();
    // The other source was listed once, so the series page fills 2.5 in from its list.
    kit.lent.sources.chapters(&work, Some("other"), now).await.unwrap();
    let (source, rows) = ordered_chapters(&kit.lent.sources, &work, now).await.unwrap();
    assert_eq!(source, "shelf");
    let filled = rows.iter().find(|r| r.id.as_str() == "2.5").expect("2.5 filled in");
    assert!(filled.other_source.is_some());

    kit.queue.download(&kit.lent, work.clone(), DownloadPick::NextUnread { n: 4 }).await;
    kit.settle().await;
    let done: Vec<_> = kit.states().into_iter().map(|(c, _)| c).collect();
    assert_eq!(done, ["1", "2", "2.5", "3"]);
    assert!(kit.states().iter().all(|(_, s)| *s == ChapterDownloadState::Done));
    let row = |c: &str| kit.store().book_download(WORK, c).unwrap().unwrap().source_id;
    assert_eq!([row("2"), row("2.5")], ["shelf", "other"]);
    assert_eq!(*other.fetched.lock().unwrap(), 3, "only 2.5's pages from the other source");
    assert!(!kit.drain().iter().any(|e| matches!(e, ReadingEvent::Failed { .. })));

    // A chapter listed from a source that no longer lists it has no source to come from.
    let listers = Listers::of(&kit.lent.sources, &work, "shelf");
    let gone = ChapterRow { other_source: Some("Other shelf".into()), ..listed("2.7", None) };
    assert_eq!(listers.of_row(&gone), None);
    assert_eq!(listers.of_row(filled), Some("other"));
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

/// A chapter cancelled while its last page is fetched leaves no file and no row, and one kept
/// ready, whose row stays as removed, does not come back as downloaded.
#[tokio::test]
async fn a_chapter_cancelled_during_its_last_page_leaves_nothing_behind() {
    for keep_ready in [false, true] {
        let kit = kit(&["1", "2"]);
        let work = WorkId::new(WORK);
        let asked = Arc::new(tokio::sync::Notify::new());
        let answer = Arc::new(tokio::sync::Notify::new());
        *kit.shelf.held.lock().unwrap() =
            Some(("https://pages.example/1/3".into(), asked.clone(), answer.clone()));
        if keep_ready {
            let wanted = [Wanted { chapter: ChapterId::new("1"), version_label: None }];
            assert_eq!(kit.queue.queue(kit.store(), &work, "shelf", &wanted, true, 1), 1);
            kit.queue.start(&kit.lent);
        } else {
            let pick = DownloadPick::Chapters { ids: ids(&["1"]) };
            kit.queue.download(&kit.lent, work.clone(), pick).await;
        }
        asked.notified().await;
        kit.queue.cancel(&kit.lent, &work, &ids(&["1"]));
        answer.notify_one();
        kit.settle().await;
        assert!(kit.states().is_empty(), "keep ready {keep_ready}: {:?}", kit.states());
        assert!(kit.store().shelf(WORK).unwrap().is_empty());
        let left: Vec<_> = std::fs::read_dir(kit.queue.dir().join(safe(WORK)))
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        assert!(left.is_empty(), "keep ready {keep_ready}: files left {left:?}");
    }
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
async fn a_download_queued_before_a_relaunch_waits_for_its_plugin_to_load() {
    let kit = kit(&["1"]);
    kit.setup(Setup { hold: true, ..Setup::default() });
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::Unread).await;
    // The next launch: the queue goes on while the source's plugin is still loading.
    kit.lent.sources.registry.remove("shelf");
    kit.lent.plugins.expect_arrivals();
    assert!(kit.queue.configure(Setup::default()));
    kit.queue.start(&kit.lent);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(*kit.shelf.fetched.lock().unwrap(), 0);
    kit.lent.sources.registry.put(kit.shelf.clone());
    kit.lent.plugins.arrivals_done();
    kit.settle().await;
    assert_eq!(kit.states(), [("1".into(), ChapterDownloadState::Done)]);
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
        ReadingFailure::FileMissing { path: "/d/a:1/1.cbz".into() },
        ReadingFailure::CannotWrite {
            path: "/d/a:1/1.cbz".into(),
            problem: WriteProblem::Name,
        },
        ReadingFailure::CannotWrite { path: "/d/x".into(), problem: WriteProblem::Denied },
        ReadingFailure::CannotWrite { path: "/d/x".into(), problem: WriteProblem::Other },
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

#[test]
fn ids_whose_old_names_collided_get_files_of_their_own() {
    for (a, b) in [(":1", "\u{3a1}"), ("1.5", "1\u{2e5}"), ("x/-/a:c", "x/-/a\u{3ac}")] {
        assert_ne!(safe(a), safe(b), "{a} and {b}");
    }
}

/// A side story named in Japanese, with no number: its key is its whole label.
const LONG_KANA: &str = "ひとりぼっちのよるにきみとはなしたことをわすれないでいたいとおもうばんがいへんそのいちのつづきのおまけまんが";

#[tokio::test]
async fn an_unnumbered_chapter_with_a_long_cjk_title_downloads() {
    let kit = kit(&["1", LONG_KANA]);
    let key = kaeri_core::books::ChapterKey::unnumbered(None, LONG_KANA);
    assert!(kaeri_core::file_name::legacy(key.as_str()).len() > 255, "too long the old way");
    kit.queue
        .download(
            &kit.lent,
            WorkId::new(WORK),
            DownloadPick::Chapters { ids: vec![ChapterId::new(key.as_str())] },
        )
        .await;
    kit.settle().await;
    assert_eq!(kit.states(), [(key.as_str().to_owned(), ChapterDownloadState::Done)]);
    let shelf = kit.store().shelf(WORK).unwrap();
    let file = std::path::Path::new(&shelf[0].path);
    assert!(file.file_name().unwrap().len() <= 64 + ".cbz".len(), "{file:?}");
}

#[test]
fn a_name_the_storage_refuses_is_said_as_such_not_as_a_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let long = dir.path().join("a".repeat(300));
    let e = std::fs::create_dir_all(&long).unwrap_err();
    let ReadingFailure::CannotWrite { problem, .. } = io_failure(&e, &long) else {
        panic!("{e}: {:?}", io_failure(&e, &long));
    };
    assert_eq!(problem, WriteProblem::Name);
    let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    assert!(matches!(
        io_failure(&denied, &long),
        ReadingFailure::CannotWrite { problem: WriteProblem::Denied, .. }
    ));
    let gone = std::io::Error::from(std::io::ErrorKind::NotFound);
    assert!(matches!(io_failure(&gone, &long), ReadingFailure::FileMissing { .. }));
    let full = std::io::Error::from(std::io::ErrorKind::StorageFull);
    assert_eq!(io_failure(&full, &long), ReadingFailure::DiskFull);
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
    assert_eq!(msg_ids(&view.series[0].line), ["reading-dl-failed", "reading-dl-offline"]);
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
        parts: 1,
        other_group: false,
        missing_parts: Vec::new(),
        part_groups: Vec::new(),
        group_language: None,
        other_source: None,
        on_device: false,
    }
}

#[tokio::test]
async fn the_estimate_counts_pages_at_the_size_past_pages_took_and_names_the_network() {
    let kit = kit(&["1", "2", "3"]);
    let work = WorkId::new(WORK);
    let rows = [listed("1", Some(10)), listed("2", None), listed("3", Some(40))];
    let pick = DownloadPick::Chapters { ids: ids(&["1", "2", "3"]) };

    // Nothing downloaded before: no size to go by, so no number rather than a wrong one.
    kit.setup(Setup { wifi_only: true, ..Setup::default() });
    let first = kit.queue.estimate(kit.store(), &work, pick.clone(), &rows, Some("shelf"));
    assert_eq!(first.chapters, 3);
    assert_eq!((first.bytes, first.all_bytes), (0, 0));
    assert!(first.line.is_empty(), "{:?}", first.line);

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
    // A send of the same chapters takes chapter 1 as it is here, and the rest as estimated.
    assert_eq!(next.all_bytes, done.bytes + (3 + 40) * page);
    // Another work from the same source goes by its pages; one from another source does not.
    let other = WorkId::new("other");
    let same = kit.queue.estimate(kit.store(), &other, pick.clone(), &rows, Some("shelf"));
    assert_eq!(same.bytes, (10 + 3 + 40) * page);
    let elsewhere =
        kit.queue.estimate(kit.store(), &other, pick.clone(), &rows, Some("elsewhere"));
    assert_eq!((elsewhere.bytes, elsewhere.line.len()), (0, 0), "pages differ by source");

    kit.setup(Setup { hold: true, wifi: true, wifi_only: true, ..Setup::default() });
    let waiting = kit.queue.estimate(kit.store(), &work, pick, &rows, Some("shelf"));
    assert_eq!(msg_ids(&waiting.line)[1], "reading-estimate-waits-wifi");
    let none = DownloadPick::Chapters { ids: ids(&["1"]) };
    let nothing = kit.queue.estimate(kit.store(), &work, none, &rows, Some("shelf"));
    assert_eq!((nothing.chapters, nothing.bytes, nothing.line.len()), (0, 0, 0));
    assert_eq!(nothing.all_bytes, done.bytes, "a downloaded chapter still has its size");
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

/// The kept pages of chapter `chapter`, by index, from their names.
fn kept_pages(kit: &Kit, chapter: &str) -> Vec<std::path::PathBuf> {
    let dir = kept::pages_of(&kit.queue.dir().join(safe(WORK)).join(safe(chapter)));
    let mut pages: Vec<_> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "page"))
                .collect()
        })
        .unwrap_or_default();
    pages.sort();
    pages
}

/// Chapter 1 fetched up to its third page, which the source fails.
async fn cut_short() -> Kit {
    let kit = kit(&["1"]);
    kit.shelf.broken.lock().unwrap().push("https://pages.example/1/3".into());
    kit.queue
        .download(&kit.lent, WorkId::new(WORK), DownloadPick::Chapters { ids: ids(&["1"]) })
        .await;
    kit.settle().await;
    assert!(matches!(kit.states()[0].1, ChapterDownloadState::Failed { .. }));
    assert_eq!(kit_pages(&kit), 2, "the two pages that came are kept");
    kit.shelf.broken.lock().unwrap().clear();
    kit.shelf.asked.lock().unwrap().clear();
    kit
}

fn kit_pages(kit: &Kit) -> usize {
    kept_pages(kit, "1").len()
}

#[tokio::test]
async fn a_chapter_cut_short_fetches_only_the_pages_it_lacks() {
    let kit = cut_short().await;
    // A restart keeps the pages of a chapter still to be fetched.
    kit.queue.recover(kit.store(), 10);
    assert_eq!(kit_pages(&kit), 2);

    kit.queue
        .download(&kit.lent, WorkId::new(WORK), DownloadPick::Chapters { ids: ids(&["1"]) })
        .await;
    kit.settle().await;
    assert_eq!(kit.states(), [("1".into(), ChapterDownloadState::Done)]);
    assert_eq!(*kit.shelf.asked.lock().unwrap(), ["https://pages.example/1/3"]);
    let shelf = kit.store().shelf(WORK).unwrap();
    match kaeri_books::open(
        std::path::Path::new(&shelf[0].path),
        &kaeri_books::Limits::default(),
    )
    .unwrap()
    {
        kaeri_books::Book::Comic(comic) => assert_eq!(comic.pages().len(), 3),
        _ => panic!("a downloaded chapter of pages opens as a comic"),
    }
    // The pages go once the chapter's file is whole.
    assert!(!kept::pages_of(&kit.queue.dir().join(safe(WORK)).join(safe("1"))).exists());
}

#[tokio::test]
async fn a_kept_page_that_does_not_check_out_is_fetched_again() {
    let kit = cut_short().await;
    let pages = kept_pages(&kit, "1");
    // The first page was torn when the app stopped.
    let bytes = std::fs::read(&pages[0]).unwrap();
    std::fs::write(&pages[0], &bytes[..bytes.len() / 2]).unwrap();

    kit.queue
        .download(&kit.lent, WorkId::new(WORK), DownloadPick::Chapters { ids: ids(&["1"]) })
        .await;
    kit.settle().await;
    assert_eq!(kit.states(), [("1".into(), ChapterDownloadState::Done)]);
    assert_eq!(
        *kit.shelf.asked.lock().unwrap(),
        ["https://pages.example/1/1", "https://pages.example/1/3"]
    );
}

/// An unnumbered chapter whose old file name an earlier version could write, but that is now
/// shortened.
const LONG_LATIN: &str = "the long road home and everything that came after";

fn long_latin_key() -> String {
    kaeri_core::books::ChapterKey::unnumbered(None, LONG_LATIN).as_str().to_owned()
}

/// The pages kept for `chapter` in the folder an earlier version named.
fn legacy_pages(kit: &Kit, chapter: &str) -> std::path::PathBuf {
    let name = kaeri_core::file_name::legacy;
    kept::pages_of(&kit.queue.dir().join(name(WORK)).join(name(chapter)))
}

#[tokio::test]
async fn pages_an_earlier_version_kept_under_its_old_name_are_picked_up() {
    let key = long_latin_key();
    assert_ne!(safe(&key), kaeri_core::file_name::legacy(&key));
    let kit = kit(&[LONG_LATIN]);
    let page3 = format!("https://pages.example/{LONG_LATIN}/3");
    kit.shelf.broken.lock().unwrap().push(page3.clone());
    let pick = || DownloadPick::Chapters { ids: vec![ChapterId::new(key.as_str())] };
    kit.queue.download(&kit.lent, WorkId::new(WORK), pick()).await;
    kit.settle().await;
    assert_eq!(kept_pages(&kit, &key).len(), 2);
    // As the earlier version left them: under the old name.
    let (new, old) = (
        kept::pages_of(&kit.queue.dir().join(safe(WORK)).join(safe(&key))),
        legacy_pages(&kit, &key),
    );
    std::fs::rename(&new, &old).unwrap();
    kit.shelf.broken.lock().unwrap().clear();
    kit.shelf.asked.lock().unwrap().clear();

    kit.queue.recover(kit.store(), 10);
    assert!(!old.exists());
    assert_eq!(kept_pages(&kit, &key).len(), 2, "moved to the new name");
    // A second launch finds nothing more to move.
    kit.queue.recover(kit.store(), 11);
    assert_eq!(kept_pages(&kit, &key).len(), 2);

    kit.queue.download(&kit.lent, WorkId::new(WORK), pick()).await;
    kit.settle().await;
    assert_eq!(kit.states(), [(key.clone(), ChapterDownloadState::Done)]);
    assert_eq!(*kit.shelf.asked.lock().unwrap(), [page3]);
}

#[tokio::test]
async fn pages_under_an_old_name_two_chapters_shared_are_not_handed_to_either() {
    // `x/-/a:c` and `x/-/aά` were both `x~2f-~2fa~3ac`. The first keeps that name; the second
    // has one of its own now, and does not take pages that may be the first's.
    let (kept_name, moved) = ("x/-/a:c", "x/-/a\u{3ac}");
    let kit = kit(&[]);
    for chapter in [kept_name, moved] {
        let ask = BookDownloadAsk {
            work: WORK,
            chapter,
            source_id: "shelf",
            version_label: None,
            keep_ready: false,
        };
        kit.store().queue_book_download(&ask, 1).unwrap();
    }
    let old = legacy_pages(&kit, moved);
    assert_eq!(old, legacy_pages(&kit, kept_name));
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("count"), b"3").unwrap();

    kit.queue.recover(kit.store(), 10);
    assert!(old.join("count").exists(), "left for the chapter whose name it still is");
    assert!(!kept::pages_of(&kit.queue.dir().join(safe(WORK)).join(safe(moved))).exists());
}

#[tokio::test]
async fn a_chapter_downloaded_under_its_old_name_still_opens_from_there() {
    let key = long_latin_key();
    let kit = kit(&[LONG_LATIN]);
    let pick = DownloadPick::Chapters { ids: vec![ChapterId::new(key.as_str())] };
    kit.queue.download(&kit.lent, WorkId::new(WORK), pick).await;
    kit.settle().await;
    // As an earlier version left it: the file under the old name, and the row saying so.
    let row = kit.store().book_download(WORK, &key).unwrap().unwrap();
    let old = kit
        .queue
        .dir()
        .join(kaeri_core::file_name::legacy(WORK))
        .join(format!("{}.cbz", kaeri_core::file_name::legacy(&key)));
    std::fs::rename(row.path.as_deref().unwrap(), &old).unwrap();
    let path = old.to_string_lossy();
    kit.store().book_download_progress(row.id, 3, Some(3), 4).unwrap();
    assert!(kit.store().book_download_fetched(row.id, &path, Some(3), row.bytes, 5).unwrap());

    kit.queue.recover(kit.store(), 10);
    assert!(old.exists(), "a finished file stays where it is");
    assert_eq!(kit.states(), [(key.clone(), ChapterDownloadState::Done)]);
    let shelf = kit.store().shelf(WORK).unwrap();
    assert_eq!(std::path::Path::new(&shelf[0].path), old);
    assert!(kaeri_books::open(&old, &kaeri_books::Limits::default()).is_ok());
}

#[tokio::test]
async fn kept_pages_go_with_a_chapter_taken_out_of_the_queue() {
    let kit = cut_short().await;
    kit.queue.delete(&kit.lent, &WorkId::new(WORK), &ids(&["1"]));
    kit.queue.recover(kit.store(), 10);
    assert!(!kept::pages_of(&kit.queue.dir().join(safe(WORK)).join(safe("1"))).exists());
}

#[tokio::test]
async fn series_are_named_in_the_chosen_title_language() {
    let kit = kit(&["1"]);
    let work = WorkId::new(WORK);
    kit.queue.download(&kit.lent, work.clone(), DownloadPick::NextUnread { n: 1 }).await;
    kit.settle().await;
    let row = kaeri_store::book_catalog::BookWork {
        work: WORK.into(),
        medium: "manga".into(),
        titles:
            r#"{"romaji":"Touro no Watashibune","english":"The Lantern Ferry","native":null}"#
                .into(),
        fetched_at: 3,
        ..kaeri_store::book_catalog::BookWork::default()
    };
    kit.store().put_book_work(&row).unwrap();

    // English first until the setting is given.
    let view = kit.queue.view(kit.store());
    assert_eq!(view.series[0].title, "The Lantern Ferry");

    let titles = kaeri_meta::titles::Titles::default();
    kit.queue.use_titles(titles.clone());
    titles.choose(kaeri_core::config::TitleChoice::Romaji);
    let view = kit.queue.view(kit.store());
    assert_eq!(view.series[0].title, "Touro no Watashibune");
    assert_eq!(view.rows[0].title, "Touro no Watashibune");
}

/// A page that comes but cannot be written for a full disk fails the chapter as a full disk and
/// stops the queue, as a full device does.
#[tokio::test]
async fn a_page_that_cannot_be_kept_for_a_full_disk_stops_the_queue() {
    let kit = kit(&["1", "2"]);
    let (asked, answer) =
        (Arc::new(tokio::sync::Notify::new()), Arc::new(tokio::sync::Notify::new()));
    let held = ("https://pages.example/1/2".to_owned(), asked.clone(), answer.clone());
    *kit.shelf.held.lock().unwrap() = Some(held);
    kit.queue.download(&kit.lent, WorkId::new(WORK), DownloadPick::Unread).await;
    asked.notified().await;
    let pages = kit.queue.sharing(WORK, "1").expect("shared");
    pages.write_with(|_, _| Err(std::io::ErrorKind::StorageFull.into()));
    drop(pages);
    answer.notify_one();
    kit.settle().await;
    assert_eq!(
        kit.states(),
        [
            ("1".into(), ChapterDownloadState::Failed { failure: ReadingFailure::DiskFull }),
            ("2".into(), ChapterDownloadState::Queued),
        ]
    );
}

/// Chapter 1 opened as a download opens it, and its pages shared from a folder of their own.
async fn shared_pages(kit: &Kit) -> (shared::Downloading, PathBuf) {
    let (work, chapter) = (WorkId::new(WORK), ChapterId::new("1"));
    let opened = kit.lent.sources.open(&work, &chapter, Some("shelf"), 1).await.unwrap();
    let dir = kit._dir.path().join("1.pages");
    (kit.queue.0.shares.open(&dir, 3, opened).unwrap(), dir)
}

/// Poll `future` once: what it gives, if it is done.
async fn poll_once<F: Future>(mut future: std::pin::Pin<&mut F>) -> Option<F::Output> {
    std::future::poll_fn(|cx| {
        std::task::Poll::Ready(match future.as_mut().poll(cx) {
            std::task::Poll::Ready(out) => Some(out),
            std::task::Poll::Pending => None,
        })
    })
    .await
}

/// A fetch fails while a newer one of the same page is on its way: the side that waited on the
/// failed one waits on the newer one rather than asking a third time.
#[tokio::test]
async fn a_side_let_down_by_a_failed_fetch_waits_on_the_newer_one() {
    let kit = kit(&["1"]);
    let url = "https://pages.example/1/2";
    let mut answers = kit.shelf.gate(url, 3).into_iter();
    let (first, second, third) =
        (answers.next().unwrap(), answers.next().unwrap(), answers.next().unwrap());
    let (pages, _) = shared_pages(&kit).await;
    let sources = &kit.lent.sources;
    let mut asker = std::pin::pin!(pages.page(sources, 1, 2, false));
    assert!(poll_once(asker.as_mut()).await.is_none());
    kit.shelf.until(|s| s.times(url) == 1).await;
    let mut waiter = std::pin::pin!(pages.page(sources, 1, 2, false));
    assert!(poll_once(waiter.as_mut()).await.is_none());
    // Past its wait, a reader asks once more.
    let mut again = std::pin::pin!(pages.page(sources, 1, 2, true));
    assert!(poll_once(again.as_mut()).await.is_none());
    kit.shelf.until(|s| s.times(url) == 2).await;
    first.send(false).unwrap();
    assert!(asker.await.is_err());
    // The waiter sees the failure and takes up the newer fetch.
    assert!(poll_once(waiter.as_mut()).await.is_none());
    assert_eq!(pages.started(), 2, "asked a third time");
    let _ = third.send(true);
    second.send(true).unwrap();
    assert!(waiter.await.is_ok());
    assert!(again.await.is_ok());
}

/// A fetch nobody waits on goes on while the download runs to want its page, and is cut off
/// once the download ends; the pages folder then goes with the last holder.
#[tokio::test]
async fn a_fetch_nobody_waits_on_is_cut_off_once_no_download_wants_it() {
    let kit = kit(&["1"]);
    let url = "https://pages.example/1/2";
    let _answer = kit.shelf.gate(url, 2);
    let (pages, dir) = shared_pages(&kit).await;
    let sources = &kit.lent.sources;
    {
        let mut reader = std::pin::pin!(pages.page(sources, 1, 2, false));
        assert!(poll_once(reader.as_mut()).await.is_none());
        kit.shelf.until(|s| s.times(url) == 1).await;
    }
    // Still on its way: a side asking now waits on it.
    {
        let mut reader = std::pin::pin!(pages.page(sources, 1, 2, false));
        assert!(poll_once(reader.as_mut()).await.is_none());
    }
    assert_eq!(pages.started(), 1);
    assert!(kit.shelf.cut.lock().unwrap().is_empty());
    let held = Arc::clone(&pages);
    pages.let_go();
    drop(pages);
    kit.shelf.until(|s| s.cut.lock().unwrap().len() == 1).await;
    while Arc::strong_count(&held) > 1 {
        tokio::task::yield_now().await;
    }
    drop(held);
    assert!(!dir.exists(), "the pages stayed");
}

/// A reader past its wait asks again while the download runs, and the download then ends: the
/// request the new one replaced is cut off as well, once nobody waits on it.
#[tokio::test]
async fn a_fetch_replaced_by_a_second_try_is_cut_off_once_no_download_wants_it() {
    let kit = kit(&["1"]);
    let url = "https://pages.example/1/2";
    let _answers = kit.shelf.gate(url, 2);
    let (download, _) = shared_pages(&kit).await;
    let pages = Arc::clone(&download);
    let sources = &kit.lent.sources;
    {
        let mut first = std::pin::pin!(pages.page(sources, 1, 2, false));
        assert!(poll_once(first.as_mut()).await.is_none());
        kit.shelf.until(|s| s.times(url) == 1).await;
    }
    {
        let mut second = std::pin::pin!(pages.page(sources, 1, 2, true));
        assert!(poll_once(second.as_mut()).await.is_none());
        kit.shelf.until(|s| s.times(url) == 2).await;
        download.let_go();
        drop(download);
        // The first request goes; the second, still waited on, stays.
        kit.shelf.until(|s| !s.cut.lock().unwrap().is_empty()).await;
        assert_eq!(kit.shelf.cut.lock().unwrap().len(), 1);
    }
    kit.shelf.until(|s| s.cut.lock().unwrap().len() == 2).await;
}

/// A download opens a chapter's folder just as the share let go of before it deletes it: the
/// folder stays for the download.
#[tokio::test]
async fn a_folder_let_go_is_not_deleted_from_under_a_download_opening_it() {
    let kit = kit(&["1"]);
    let (old, dir) = shared_pages(&kit).await;
    let opened = old.opened();
    old.let_go();
    let (inside, entered) = std::sync::mpsc::channel();
    let (go, wait) = std::sync::mpsc::channel::<()>();
    let wait = Mutex::new(wait);
    let hook: shared::BeforeRemoving = Arc::new(move || {
        inside.send(()).unwrap();
        wait.lock().unwrap().recv().unwrap();
    });
    *shared::BEFORE_REMOVING.lock().unwrap() = Some((dir.clone(), hook));
    let dropping = std::thread::spawn(move || drop(old));
    entered.recv().unwrap();
    let new = kit.queue.0.shares.open(&dir, 3, opened).unwrap();
    go.send(()).unwrap();
    dropping.join().unwrap();
    *shared::BEFORE_REMOVING.lock().unwrap() = None;
    assert!(dir.join("count").exists(), "the folder went from under the download");
    drop(new);
}

/// The reader takes a chapter kept ready out of the queue just as the queue picked it: the
/// queue does not ask the source for it. One the queue has started stays, for the reader to
/// join.
#[tokio::test]
async fn a_chapter_taken_out_of_the_queue_as_it_starts_is_not_asked_for() {
    let kit = kit(&["1", "2"]);
    let ask = |chapter| kaeri_store::book_downloads::BookDownloadAsk {
        work: WORK,
        chapter,
        source_id: "shelf",
        version_label: None,
        keep_ready: true,
    };
    kit.store().queue_book_download(&ask("1"), 1).unwrap();
    let row = kit.store().next_book_download().unwrap().unwrap();
    let reader = (&WorkId::new(WORK), &ChapterId::new("1"));
    assert!(kit.queue.reader_fetches(kit.store(), &kit.lent.tx, reader, 2));
    let (go_on, done) = kit.queue.one(&kit.lent, kit.store(), row).await;
    assert!(go_on && !done);
    assert!(kit.shelf.contents.lock().unwrap().is_empty(), "asked for its pages");

    kit.store().queue_book_download(&ask("2"), 3).unwrap();
    let row = kit.store().next_book_download().unwrap().unwrap();
    kit.store().book_download_progress(row.id, 0, None, 4).unwrap();
    let reader = (&WorkId::new(WORK), &ChapterId::new("2"));
    assert!(!kit.queue.reader_fetches(kit.store(), &kit.lent.tx, reader, 5), "started");
    let state = kit.store().book_download(WORK, "2").unwrap().unwrap().state;
    assert_eq!(state, BookDownloadState::Downloading);
}
