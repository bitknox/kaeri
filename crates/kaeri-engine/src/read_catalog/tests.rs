//! Reading's Home rows and Search's source groups against the fake AniList, with made-up books
//! and sources. Times are fixed or on tokio's stopped clock; nothing waits on the wall clock.

use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, HitAmount, SeriesDetail,
        SeriesHit, SourceChapter,
    },
    config::Paths,
    error::ProviderError,
    media::{Medium, ReadingFormat},
    traits::BookSource,
};
use kaeri_model::{
    Msg,
    event::Event,
    medium::KaeriIsFor,
    reading::{
        ReadAnilistState, ReadRowKind, ReadSearchQuery, ReadSourceKind, ReadSourceSearch,
        ReadSourceState, ReadingEvent, ReadingHomeView, WorkTile,
    },
};
use kaeri_store::{Store, reading::ReadEvent};
use kaeri_testkit::anilist::{FakeAniList, Operation};
use serde_json::{Value, json};
use tokio::sync::{Notify, mpsc};

use super::{Ask, Kit, home, search, tiles};
use crate::{outbox::Outbox, read_sources::BookSources};

const DAY: i64 = 86_400;
// Noon UTC on 14 November 2023, read in UTC.
const NOW: i64 = 1_700_000_000 - 1_700_000_000 % DAY + 12 * 3_600;

fn kit(fake: Option<&FakeAniList>, dir: &std::path::Path) -> Kit {
    let store = Store::open_in_memory().unwrap();
    let paths = Paths {
        config_file: dir.join("config.toml"),
        data_dir: dir.join("data"),
        cache_dir: dir.join("cache"),
    };
    let tokens = kaeri_track::TokenStore::new(dir.join("tokens"));
    let sources = BookSources::new(store.clone(), tokens, reqwest::Client::new(), &paths);
    let meta = match fake {
        Some(fake) => fake.meta(&store),
        None => {
            let http = kaeri_net::HttpClient::offline(&Default::default()).unwrap();
            kaeri_meta::Meta::new(kaeri_meta::AniList::new(http, 30), None, store.clone())
        }
    };
    Kit {
        meta,
        store,
        sources,
        plugins: crate::book_plugins::BookPlugins::default(),
        clock: crate::clock::Clock::in_zone(jiff::tz::TimeZone::UTC),
        offline: Arc::new(AtomicBool::new(false)),
    }
}

fn ask() -> Ask {
    Ask { answer: KaeriIsFor::Both, adult: false, chapter_notices: false, now: NOW }
}

fn outbox() -> (Outbox, mpsc::UnboundedReceiver<Event>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (tx.into(), rx)
}

/// A made-up book as AniList gives it, releasing unless [`finished`] says otherwise.
fn book(
    id: u32,
    title: &str,
    format: &str,
    (started, popularity): (i64, u32),
    genres: &[&str],
    recs: &[u32],
) -> Value {
    let nodes: Vec<Value> =
        recs.iter().map(|id| json!({ "mediaRecommendation": { "id": id } })).collect();
    json!({
        "id": id,
        "title": { "romaji": title, "english": title },
        "format": format,
        "status": "RELEASING",
        "startDate": { "year": started, "month": 6, "day": 1 },
        "tags": [],
        "isAdult": false,
        "popularity": popularity,
        "genres": genres,
        "synonyms": [],
        "coverImage": { "large": format!("https://covers.example/{id}.png") },
        "recommendations": { "nodes": nodes },
    })
}

fn finished(mut book: Value) -> Value {
    book["status"] = json!("FINISHED");
    book
}

/// Twenty older titles of a format, more popular than any other, so the format's Trending row
/// is full and the rows after it show what the test is about.
fn popular(format: &str, from: u32) -> Vec<Value> {
    (0..20)
        .map(|n| {
            let id = from + n;
            let title = format!("Quiet Hill Stories {}", n + 1);
            book(id, &title, format, (2005, 100_000 + n), &["Comedy"], &[])
        })
        .collect()
}

/// The next event the engine sends. A test waits at most this long for one, so a hang fails at
/// once rather than holding the run. On a stopped clock the wait moves with it, and anything due
/// sooner (the sources' 15 seconds) comes first.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

async fn next_event(rx: &mut mpsc::UnboundedReceiver<Event>) -> Event {
    tokio::time::timeout(PATIENCE, rx.recv())
        .await
        .expect("an event in time")
        .expect("the engine is still sending")
}

fn books_asked(fake: &FakeAniList) -> usize {
    fake.requests_for(Operation::Books).len()
}

async fn whole_home(rx: &mut mpsc::UnboundedReceiver<Event>) -> ReadingHomeView {
    loop {
        if let Event::Reading(ReadingEvent::Home(home)) = next_event(rx).await
            && home.pending.is_empty()
            && home.end.is_some()
        {
            return *home;
        }
    }
}

#[tokio::test]
async fn a_cold_home_asks_four_times_and_a_warm_one_not_at_all() {
    let fake = FakeAniList::start().await;
    fake.add_books(popular("MANGA", 911_000));
    fake.add_books(popular("NOVEL", 912_000));
    fake.add_books([
        book(
            910_001,
            "Iron Kite Brigade",
            "MANGA",
            (2019, 500),
            &["Action"],
            &[910_003, 919_999],
        ),
        finished(book(
            910_002,
            "The Sixth Bell of Morrow",
            "NOVEL",
            (2015, 500),
            &["Romance"],
            &[910_006],
        )),
        book(910_003, "Saltwind Diary", "MANGA", (2018, 400), &["Action"], &[]),
        book(910_004, "Letters to the Heron Clerk", "NOVEL", (2023, 400), &["Romance"], &[]),
        book(910_005, "Paper Crane Courier", "MANGA", (2023, 400), &["Action"], &[]),
        book(910_006, "The Lamplighter's Almanac", "NOVEL", (2010, 300), &["Romance"], &[]),
        book(910_007, "Ember Relay", "MANGA", (2010, 300), &["Action"], &[]),
        finished(book(910_008, "Quiet Orchard Tales", "MANGA", (2001, 300), &["Drama"], &[])),
        book(910_009, "Harbor of Small Hours", "NOVEL", (2008, 300), &["Romance"], &[]),
    ]);
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    // Reads manga, finished a light novel: manga first, then light novels.
    kit.store.set_read_status("a:910001", Some("reading"), NOW - DAY).unwrap();
    for chapter in ["1", "2", "3"] {
        let read =
            ReadEvent { completed: true, ..ReadEvent::new("a:910001", chapter, NOW - DAY) };
        kit.store.record_read(&read, Vec::new()).unwrap();
    }
    kit.store.set_read_status("a:910002", Some("completed"), NOW - 2 * DAY).unwrap();

    let (tx, mut rx) = outbox();
    home::load(&kit, ask(), false, &tx);
    let cold = whole_home(&mut rx).await;
    // The screen's request, the queue's rows, the two pages in one, and what they suggest.
    assert_eq!(books_asked(&fake), 4, "{:#?}", fake.requests_for(Operation::Books));

    let ids: Vec<&str> = cold.rows.iter().map(|r| r.id.as_str()).collect();
    let planned = [
        "trending-manga",
        "trending-novels",
        "because",
        "new-manga",
        "new-novels",
        "genre",
        "finished-manga",
        "trending-webtoons",
    ];
    assert_eq!(ids, planned, "manga read most, light novels next, webtoons never");
    let because = cold.rows.iter().find(|r| r.kind == ReadRowKind::Because).unwrap();
    assert_eq!(because.title.english(), "Because you finished The Sixth Bell of Morrow");
    assert_eq!(because.view_all, None);
    let trending = &cold.rows[0];
    assert_eq!(trending.view_all.as_ref().unwrap().sort.as_deref(), Some("TRENDING_DESC"));

    // Each title once, never the person's own, never adult.
    let mut seen = HashSet::new();
    for tile in cold.rows.iter().flat_map(|r| &r.tiles) {
        assert!(seen.insert(tile.work.0.clone()), "{} twice", tile.work.0);
        assert!(!["a:910001", "a:910002"].contains(&tile.work.as_str()));
        assert!(!tile.adult);
    }
    assert!(cold.rows.iter().all(|r| !r.pending && !r.tiles.is_empty()));
    // The fields of front ends not built against rows yet.
    assert_eq!(cold.releasing, cold.rows[0].tiles);
    assert_eq!(cold.because, because.tiles);

    // Warm: everything is kept and fresh.
    home::load(&kit, ask(), false, &tx);
    let warm = whole_home(&mut rx).await;
    assert_eq!(books_asked(&fake), 4, "a warm Home asks for nothing");
    assert_eq!(warm.rows, cold.rows);
}

#[tokio::test]
async fn a_first_run_shows_each_trending_row_then_new_manga() {
    let fake = FakeAniList::start().await;
    fake.add_books(popular("MANGA", 911_000));
    fake.add_books([book(
        910_011,
        "Moth and Lantern",
        "ONE_SHOT",
        (2023, 400),
        &["Drama"],
        &[],
    )]);
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    let (tx, mut rx) = outbox();
    home::load(&kit, ask(), false, &tx);
    let first = whole_home(&mut rx).await;
    let ids: Vec<&str> = first.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["trending-manga", "trending-webtoons", "trending-novels", "new-manga"]);
    // New manga is exactly its View all: started in the last 12 months, most popular first.
    let new = &first.rows[3];
    let query = new.view_all.as_ref().unwrap();
    assert_eq!(query.started.as_deref(), Some("12-months"));
    let asked = fake.requests_for(Operation::Books);
    let started = asked.iter().any(|r| r.variables.to_string().contains("20221114"));
    assert!(started, "the 12 months end on the day: {asked:#?}");
    assert!(first.spot.is_some(), "the popular pick comes from the Trending rows");
    assert_eq!(asked.len(), 2, "the screen's one request and the queue's one");
}

#[tokio::test]
async fn rows_with_nothing_kept_leave_when_anilist_fails() {
    let fake = FakeAniList::start().await;
    fake.fail_always(kaeri_testkit::anilist::Failure::Status(500));
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    let (tx, mut rx) = outbox();
    home::load(&kit, ask(), false, &tx);
    let home = whole_home(&mut rx).await;
    assert!(home.rows.is_empty(), "{:?}", home.rows);
    assert!(home.failed || home.offline, "one notice says so");
}

// ---------------------------------------------------------------------------------------------
// Search

/// How a test source answers.
enum Answer {
    Now(Vec<SeriesHit>),
    /// After the test lets it.
    Gated(Arc<Notify>, Vec<SeriesHit>),
    /// Fails the first time, then finds these.
    FailsOnce(Vec<SeriesHit>),
    Never,
}

struct Shelf {
    manifest: BookSourceManifest,
    answer: Answer,
    asked: AtomicU32,
}

impl Shelf {
    fn put(kit: &Kit, id: &str, name: &str, answer: Answer) -> Arc<Self> {
        Self::put_as(kit, id, name, BookSourceKind::Native, answer)
    }

    fn put_as(
        kit: &Kit,
        id: &str,
        name: &str,
        kind: BookSourceKind,
        answer: Answer,
    ) -> Arc<Self> {
        let manifest = BookSourceManifest {
            id: id.into(),
            name: name.into(),
            version: "1".into(),
            kind,
            media: vec![Medium::Manga, Medium::Novel],
            languages: vec!["en".into()],
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        let shelf = Arc::new(Self { manifest, answer, asked: AtomicU32::new(0) });
        kit.sources.registry.put(shelf.clone());
        shelf
    }
}

#[async_trait]
impl BookSource for Shelf {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        let asked = self.asked.fetch_add(1, Ordering::SeqCst);
        match &self.answer {
            Answer::Now(hits) => Ok(hits.clone()),
            Answer::Gated(gate, hits) => {
                gate.notified().await;
                Ok(hits.clone())
            }
            Answer::FailsOnce(_) if asked == 0 => {
                Err(ProviderError::Other("the shelf is closed".into()))
            }
            Answer::FailsOnce(hits) => Ok(hits.clone()),
            Answer::Never => std::future::pending().await,
        }
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, id))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok(Vec::new())
    }

    async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
        Err(ProviderError::NotFound)
    }
}

fn hit(id: &str, title: &str, format: Option<ReadingFormat>) -> SeriesHit {
    SeriesHit { format, ..SeriesHit::new(id, title) }
}

fn query(text: &str) -> ReadSearchQuery {
    ReadSearchQuery { text: text.into(), ..ReadSearchQuery::default() }
}

/// The next sources' part sent.
async fn next_part(rx: &mut mpsc::UnboundedReceiver<Event>) -> ReadSourceSearch {
    loop {
        if let Event::Reading(ReadingEvent::SourceSearch(part)) = next_event(rx).await {
            return *part;
        }
    }
}

async fn part_until(
    rx: &mut mpsc::UnboundedReceiver<Event>,
    until: impl Fn(&ReadSourceSearch) -> bool,
) -> ReadSourceSearch {
    loop {
        let part = next_part(rx).await;
        if until(&part) {
            return part;
        }
    }
}

fn state<'a>(part: &'a ReadSourceSearch, id: &str) -> &'a ReadSourceState {
    &part.groups.iter().find(|g| g.source_id == id).unwrap().state
}

#[tokio::test]
async fn a_slow_source_never_holds_up_anilist_or_the_others() {
    let fake = FakeAniList::start().await;
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    let gate = Arc::new(Notify::new());
    let lantern = vec![hit("s-1", "the lantern ferry!", Some(ReadingFormat::Manga))];
    Shelf::put_as(&kit, "home", "Home library", BookSourceKind::Server, Answer::Now(lantern));
    let unknown = vec![SeriesHit {
        amount: Some(HitAmount::Episodes(40)),
        ..hit("n-9", "Lantern Street Notes", Some(ReadingFormat::Novel))
    }];
    let gated = Answer::Gated(gate.clone(), unknown);
    // A plugin kind would be dropped here, since no plugin host loaded it.
    Shelf::put_as(&kit, "northwind", "Northwind", BookSourceKind::Folder, gated);
    kit.sources.registry.set_order(vec!["home".into(), "northwind".into()]);

    let (tx, mut rx) = outbox();
    search::search(&kit, ask(), query("lantern"), 0, &tx);
    let first = next_part(&mut rx).await;
    // The folder source is always there; the two here keep the person's order around it.
    let ours = ["home", "northwind"];
    let slots: Vec<&str> = first
        .groups
        .iter()
        .map(|g| g.source_id.as_str())
        .filter(|id| ours.contains(id))
        .collect();
    assert_eq!(slots, ours, "Your sources' order, from the first send");
    assert!(first.groups.iter().all(|g| g.state == ReadSourceState::Asking));
    assert_eq!(first.anilist, ReadAnilistState::Asking);

    // AniList and Home library land while Northwind is still asked.
    let landed = part_until(&mut rx, |p| {
        p.anilist == ReadAnilistState::Answered && *state(p, "home") == ReadSourceState::Hits
    })
    .await;
    assert_eq!(*state(&landed, "northwind"), ReadSourceState::Asking);
    assert!(!landed.done);
    // The hit is The Lantern Ferry once folded, and both are manga: it joins AniList's row.
    let home = landed.groups.iter().find(|g| g.source_id == "home").unwrap();
    assert_eq!((home.joined, home.tiles.len()), (1, 0));
    assert_eq!(landed.joins[0].work.as_str(), "a:900301");
    assert_eq!(landed.joins[0].source_names, ["Home library"]);
    assert_eq!(
        kit.store.book_match("home", "s-1").unwrap().as_deref(),
        Some("a:900301"),
        "opening the row reads from the source that had it"
    );

    gate.notify_one();
    let done = part_until(&mut rx, |p| p.done).await;
    let north = done.groups.iter().find(|g| g.source_id == "northwind").unwrap();
    assert_eq!(north.state, ReadSourceState::Hits);
    assert_eq!(north.tiles[0].title, "Lantern Street Notes", "no AniList result to join");
    // Each group names its kind, and a hit says how much the source holds.
    assert_eq!(north.kind, Some(ReadSourceKind::Folder));
    assert_eq!(home.kind, Some(ReadSourceKind::Server));
    assert_eq!(
        north.tiles[0].amount.as_ref().map(Msg::english).as_deref(),
        Some("Episodes 1 to 40")
    );
}

#[test]
fn a_hits_line_says_how_much_its_source_holds() {
    let line = |amount: HitAmount| tiles::amount(&amount).english();
    assert_eq!(line(HitAmount::Volumes(2)), "2 volumes");
    assert_eq!(line(HitAmount::Chapters(18)), "Chapters 1 to 18");
    assert_eq!(line(HitAmount::Chapters(1)), "1 chapter");
    assert_eq!(line(HitAmount::Episodes(22)), "Episodes 1 to 22");
    assert_eq!(line(HitAmount::Parts(6)), "6 parts");
    let cbz = Some("CBZ".to_owned());
    assert_eq!(line(HitAmount::Files { count: 1, format: cbz }), "1 file · CBZ");
    assert_eq!(line(HitAmount::Files { count: 3, format: None }), "3 files");

    let file = |format: &str| kaeri_store::book_sources::LocalFile {
        path: format!("/books/one.{format}"),
        series: "s".into(),
        volume: None,
        label: "One".into(),
        number_milli: None,
        format: format.into(),
        pages: None,
        size: 1,
        mtime: 0,
        comicinfo: None,
    };
    let files = crate::read_sources::files_amount;
    assert_eq!(
        files(&[file("cbz")]),
        Some(HitAmount::Files { count: 1, format: Some("CBZ".into()) })
    );
    assert_eq!(
        files(&[file("cbz"), file("epub")]),
        Some(HitAmount::Files { count: 2, format: None })
    );
    assert_eq!(files(&[]), None);
}

#[tokio::test]
async fn anilists_results_say_their_origin_and_a_total_only_when_it_is_exact() {
    let fake = FakeAniList::start().await;
    fake.add_books(popular("MANGA", 911_000));
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    let (tx, mut rx) = outbox();
    let results = async |rx: &mut mpsc::UnboundedReceiver<Event>| loop {
        if let Event::Reading(ReadingEvent::Search { tiles, has_more, total, .. }) =
            next_event(rx).await
        {
            return (tiles, has_more, total);
        }
    };
    // A few results, all on the first page: the count is exact.
    search::search(&kit, ask(), query("lantern"), 0, &tx);
    let (tiles, has_more, total) = results(&mut rx).await;
    assert!(!has_more);
    assert_eq!(total, Some(u32::try_from(tiles.len()).unwrap()));
    let ferry = tiles.iter().find(|t| t.work.as_str() == "a:900301").unwrap();
    assert_eq!(ferry.origin.as_ref().map(Msg::english).as_deref(), Some("Japan"));
    // More than a page: no number until the last page, never AniList's stand-in.
    search::search(&kit, ask(), query(""), 0, &tx);
    let (_, has_more, total) = results(&mut rx).await;
    assert!(has_more);
    assert_eq!(total, None);
}

#[tokio::test]
async fn the_demo_shows_once_joined_to_its_own_result() {
    let fake = FakeAniList::start().await;
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    kit.sources.put_demo(NOW);
    let (tx, mut rx) = outbox();
    // A word of the demo's first work, which its own search finds too.
    let series = kaeri_core::reading_demo::WORKS[0].series;
    search::search(&kit, ask(), query(series), 0, &tx);
    // The joined part comes first, then AniList's results; the sources may settle either side.
    let mut joined_before_results = false;
    let mut results: Option<Vec<WorkTile>> = None;
    let mut done: Option<ReadSourceSearch> = None;
    while results.is_none() || done.is_none() {
        match next_event(&mut rx).await {
            Event::Reading(ReadingEvent::Search { tiles, .. }) => results = Some(tiles),
            Event::Reading(ReadingEvent::SourceSearch(part)) => {
                if part.anilist == ReadAnilistState::Answered && results.is_none() {
                    joined_before_results = true;
                }
                if part.done {
                    done = Some(*part);
                }
            }
            _ => {}
        }
    }
    let (results, done) = (results.unwrap(), done.unwrap());
    let demo_works: Vec<&str> =
        results.iter().filter(|t| t.demo).map(|t| t.work.as_str()).collect();
    assert!(!demo_works.is_empty(), "{results:?}");
    let group = done.groups.iter().find(|g| g.source_id == "demo").unwrap();
    assert!(group.tiles.is_empty(), "each demo work shows once: {:?}", group.tiles);
    assert_eq!(group.joined as usize, demo_works.len());
    assert_eq!(group.kind, None, "built in: its name alone");
    assert!(joined_before_results, "the group gives its hits up before the results show");
}

#[tokio::test]
async fn a_failed_source_says_why_and_tries_again_alone() {
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(None, dir.path());
    let hits = vec![hit("w-1", "Wren and the Iron Bridge", None)];
    let shaky = Shelf::put(&kit, "shaky", "Shaky shelf", Answer::FailsOnce(hits));
    let steady = Shelf::put(&kit, "steady", "Steady shelf", Answer::Now(Vec::new()));
    let only = ReadSearchQuery { sources_only: true, ..query("wren") };

    let (tx, mut rx) = outbox();
    search::search(&kit, ask(), only.clone(), 0, &tx);
    let done = part_until(&mut rx, |p| p.done).await;
    assert!(matches!(state(&done, "shaky"), ReadSourceState::Failed { .. }), "{done:?}");
    assert_eq!(*state(&done, "steady"), ReadSourceState::Nothing);

    search::retry_source(&kit, ask(), only, "shaky".into(), &tx);
    let asking = next_part(&mut rx).await;
    assert_eq!(*state(&asking, "shaky"), ReadSourceState::Asking);
    assert_eq!(*state(&asking, "steady"), ReadSourceState::Nothing, "the others keep theirs");
    let again = part_until(&mut rx, |p| p.done).await;
    assert_eq!(*state(&again, "shaky"), ReadSourceState::Hits);
    assert_eq!(
        (shaky.asked.load(Ordering::SeqCst), steady.asked.load(Ordering::SeqCst)),
        (2, 1)
    );
}

#[tokio::test(start_paused = true)]
async fn a_source_with_no_answer_runs_out_of_time_after_fifteen_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(None, dir.path());
    Shelf::put(&kit, "silent", "Silent shelf", Answer::Never);
    let only = ReadSearchQuery { sources_only: true, ..query("heron") };
    let (tx, mut rx) = outbox();
    let started = tokio::time::Instant::now();
    search::search(&kit, ask(), only, 0, &tx);
    let done = part_until(&mut rx, |p| p.done).await;
    assert_eq!(*state(&done, "silent"), ReadSourceState::TimedOut { seconds: 15 });
    assert_eq!(started.elapsed(), std::time::Duration::from_secs(15));
}

#[tokio::test]
async fn your_sources_only_leaves_anilist_out() {
    let fake = FakeAniList::start().await;
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    let lantern = vec![hit("s-1", "The Lantern Ferry", Some(ReadingFormat::Manga))];
    Shelf::put(&kit, "home", "Home library", Answer::Now(lantern));
    let (tx, mut rx) = outbox();
    let only = ReadSearchQuery { sources_only: true, ..query("lantern") };
    search::search(&kit, ask(), only, 0, &tx);
    let done = part_until(&mut rx, |p| p.done).await;
    assert_eq!(done.anilist, ReadAnilistState::Skipped);
    assert!(done.joins.is_empty(), "nothing to join with AniList left out");
    assert_eq!(done.groups.iter().find(|g| g.source_id == "home").unwrap().tiles.len(), 1);
    assert_eq!(books_asked(&fake), 0);
    while let Ok(event) = rx.try_recv() {
        assert!(!matches!(event, Event::Reading(ReadingEvent::Search { .. })), "{event:?}");
    }
}

#[tokio::test]
async fn sources_are_still_asked_when_anilist_is_unreachable_and_never_for_a_browse() {
    let fake = FakeAniList::start().await;
    fake.fail_always(kaeri_testkit::anilist::Failure::Status(500));
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    let shelf = Shelf::put(&kit, "home", "Home library", Answer::Now(Vec::new()));
    let (tx, mut rx) = outbox();
    search::search(&kit, ask(), query("lantern"), 0, &tx);
    let done = part_until(&mut rx, |p| p.done).await;
    assert_eq!(done.anilist, ReadAnilistState::Failed);
    assert_eq!(*state(&done, "home"), ReadSourceState::Nothing);

    // No words: AniList alone.
    fake.heal();
    search::search(&kit, ask(), query(""), 0, &tx);
    loop {
        match next_event(&mut rx).await {
            Event::Reading(ReadingEvent::Search { .. }) => break,
            Event::Reading(ReadingEvent::SourceSearch(part)) => panic!("{part:?}"),
            _ => {}
        }
    }
    assert_eq!(shelf.asked.load(Ordering::SeqCst), 1);
}
