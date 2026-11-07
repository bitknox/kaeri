//! Reading's catalogue in the reading scenes, against the fake AniList's made-up books: Home
//! from one request, search with its filters, a series page with its versions, and the library.
#![cfg(feature = "testing")]

use std::{sync::Arc, thread, time::Duration};

use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiScene};
use kaeri_model::{
    Msg, ReadingMsg,
    command::Command,
    event::Event,
    reading::{
        LibrarySegmentRead, ReadHomeEnd, ReadSearchQuery, ReadSpotlightCase, ReadingCommand,
        ReadingEvent, ReadingHomeView, SeriesAction, SeriesView,
    },
};

const PATIENCE: Duration = Duration::from_secs(30);

struct Quiet;

impl FfiHost for Quiet {
    fn open_url(&self, _url: String) -> bool {
        false
    }

    fn notify(&self, _title: String, _body: String) {}

    fn secret_get(&self, _id: String) -> Option<String> {
        None
    }

    fn secret_set(&self, _id: String, _value: String) -> bool {
        false
    }

    fn secret_delete(&self, _id: String) -> bool {
        true
    }
}

fn read_until(
    engine: &Arc<FfiEngine>,
    until: impl Fn(&Event) -> bool + Send + 'static,
) -> Vec<Event> {
    let engine = Arc::clone(engine);
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut read = Vec::new();
        loop {
            let batch = futures::executor::block_on(engine.next_events());
            let found = batch.iter().any(&until);
            let empty = batch.is_empty();
            read.extend(batch);
            if found || empty {
                let _ = tx.send((read, found));
                break;
            }
        }
    });
    let (read, found) = rx.recv_timeout(PATIENCE).expect("the event in time");
    assert!(found, "the engine stopped first, having said: {read:?}");
    read
}

/// A clock whose zone puts the moment the test starts at midday, a fixed offset with no
/// daylight saving: the scene's chapter out two hours ago is out today at any hour the tests
/// run, where read in the system's zone it was yesterday from midnight to 02:00.
fn at_midday() -> kaeri_engine::clock::Clock {
    use jiff::tz::{Offset, TimeZone};
    const DAY: i64 = 86_400;
    let now = jiff::Timestamp::now().as_second();
    let ahead = (DAY / 2 - now).rem_euclid(DAY);
    let offset = if ahead > DAY / 2 { ahead - DAY } else { ahead };
    let offset = i32::try_from(offset).ok().and_then(|secs| Offset::from_seconds(secs).ok());
    let zone = TimeZone::fixed(offset.expect("half a day either side of UTC is an offset"));
    kaeri_engine::clock::Clock::in_zone(zone)
}

fn start(scene: FfiScene) -> (tempfile::TempDir, Arc<FfiEngine>) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_scene_on(dirs, Arc::new(Quiet), scene, at_midday())
        .expect("the engine starts");
    (root, engine)
}

fn whole_home(engine: &Arc<FfiEngine>, refresh: bool) -> ReadingHomeView {
    engine.send(Command::Reading(ReadingCommand::LoadHome { refresh }));
    let done = |event: &Event| matches!(event, Event::Reading(ReadingEvent::Home(home)) if home.pending.is_empty() && home.end.is_some());
    let read = read_until(engine, done);
    read.into_iter()
        .rev()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Home(home)) => Some(*home),
            _ => None,
        })
        .expect("Home")
}

fn books_asked(engine: &Arc<FfiEngine>) -> usize {
    engine.testing_anilist_requests().iter().filter(|r| *r == "Books").count()
}

#[test]
fn home_comes_whole_from_one_request_and_the_queue_behind_it() {
    let (_root, engine) = start(FfiScene::ReadingHome);
    let home = whole_home(&engine, false);
    let works = |tiles: &[kaeri_model::reading::WorkTile]| -> Vec<String> {
        tiles.iter().map(|t| t.work.0.clone()).collect()
    };

    // Continue: the manga stopped on page 9 of chapter 13, and the novel.
    let continuing = works(&home.continue_reading);
    assert!(continuing.contains(&"a:900301".to_owned()), "{continuing:?}");
    assert!(continuing.contains(&"a:900302".to_owned()), "{continuing:?}");
    let manga = home.continue_reading.iter().find(|t| t.work.0 == "a:900301").unwrap();
    assert_eq!(manga.title, "The Lantern Ferry");
    // Chapter 15 came out since, but the person is part way through 13: no new-chapter dot,
    // the track shows.
    assert!(!manga.new_chapter, "chapter 13 is in progress");

    // The spotlight: the first of Continue, opening where the person stopped, rather than a
    // new chapter in a series they are behind in.
    let spot = home.spot.as_ref().expect("a spotlight");
    assert_eq!(spot.case, ReadSpotlightCase::Continuing);
    assert!(
        matches!(
            &spot.action,
            Some(SeriesAction::Continue { .. } | SeriesAction::ContinuePart { .. })
        ),
        "{:?}",
        spot.action
    );

    // This week: the manga today, as it came out, and the strip on the day it usually does.
    let today = home.week.first().expect("today");
    assert!(today.today);
    assert!(today.entries.iter().any(|e| e.tile.work.0 == "a:900301" && e.out_at.is_some()));
    let strip = home.week.iter().flat_map(|d| &d.entries).find(|e| e.tile.work.0 == "a:900303");
    assert!(strip.is_some_and(|e| e.usually), "{:?}", home.week);

    // Up next: the Planning list first.
    let up_next = works(&home.up_next);
    assert_eq!(&up_next[..2], ["a:900304", "a:900306"]);
    assert!(home.up_next[0].facts.contains(&Msg::Reading(ReadingMsg::ReadingOnPlanning)));

    // Discovery: what is coming out, and the genre shelf; never what the person reads.
    assert!(!home.releasing.is_empty());
    assert!(home.releasing.iter().all(|t| !continuing.contains(&t.work.0)));
    assert!(home.genre.is_some() && !home.genre_shelf.is_empty());
    assert!(home.releasing.iter().chain(&home.genre_shelf).all(|t| !t.adult));

    // Because you finished The Quiet Observatory: what it led to, without the person's own.
    assert_eq!(home.because_title.as_deref(), Some("The Quiet Observatory"));
    assert!(!home.because.is_empty());
    assert!(
        home.because.iter().all(|t| !continuing.contains(&t.work.0) && t.work.0 != "a:900307")
    );

    // No folder or server yet: the end offers Sources.
    assert!(home.no_sources);
    assert_eq!(home.end, Some(ReadHomeEnd::NoSources));

    // The screen's part was one request; the queue behind it asked a finished book's page and
    // a read one's, and what they suggest.
    let asked = books_asked(&engine);
    assert!((1..=5).contains(&asked), "{:?}", engine.testing_anilist_requests());

    // A second Home asks nothing: everything is kept and fresh.
    let again = whole_home(&engine, false);
    assert_eq!(books_asked(&engine), asked, "{:?}", engine.testing_anilist_requests());
    assert_eq!(again.continue_reading, home.continue_reading);
    engine.shutdown();
}

fn search(
    engine: &Arc<FfiEngine>,
    query: ReadSearchQuery,
) -> (Vec<String>, Option<u32>, Option<u32>) {
    engine.send(Command::Reading(ReadingCommand::Search { query: query.clone() }));
    let read = read_until(
        engine,
        move |event| matches!(event, Event::Reading(ReadingEvent::Search { query: q, .. }) if *q == query),
    );
    read.into_iter()
        .rev()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Search { tiles, other_scope, total, .. }) => {
                Some((tiles.iter().map(|t| t.work.0.clone()).collect(), other_scope, total))
            }
            _ => None,
        })
        .expect("a search")
}

#[test]
fn search_asks_anilist_with_its_filters() {
    let (_root, engine) = start(FfiScene::ReadingHome);
    let (found, other, total) = search(
        &engine,
        ReadSearchQuery { text: "lantern".into(), ..ReadSearchQuery::default() },
    );
    assert_eq!(found, ["a:900301"]);
    // Both media are on: the count of shows rides along.
    assert_eq!(other, Some(0));
    // The whole answer is on its one page, so the total is known.
    assert_eq!(total, Some(1));

    let strips = ReadSearchQuery { formats: vec!["webtoon".into()], ..Default::default() };
    let (found, other, _) = search(&engine, strips);
    assert!(found.contains(&"a:900303".to_owned()) && !found.contains(&"a:900301".to_owned()));
    assert_eq!(other, None);

    let novels = ReadSearchQuery { formats: vec!["novel".into()], ..Default::default() };
    let (found, ..) = search(&engine, novels);
    assert!(found.contains(&"a:900302".to_owned()), "{found:?}");
    assert!(!found.contains(&"a:900301".to_owned()), "{found:?}");

    let korean = ReadSearchQuery { origins: vec!["KR".into()], ..Default::default() };
    let (found, ..) = search(&engine, korean);
    assert!(found.contains(&"a:900303".to_owned()) && !found.contains(&"a:900302".to_owned()));
    engine.shutdown();
}

#[test]
fn a_series_page_says_where_you_are_what_else_it_is_and_its_versions() {
    let (_root, engine) = start(FfiScene::ReadingHome);
    engine.send(Command::Reading(ReadingCommand::LoadSeries {
        work: kaeri_core::books::WorkId::new("a:900301"),
    }));
    // The page, the chapters and More like this come in any order: read until all have.
    let mut read = Vec::new();
    let whole = |read: &[Event]| {
        let chapters = read
            .iter()
            .any(|event| matches!(event, Event::Reading(ReadingEvent::Chapters { .. })));
        let page = read.iter().any(|event| {
            matches!(event, Event::Reading(ReadingEvent::Series(view)) if !view.partial && !view.recommended.is_empty())
        });
        chapters && page
    };
    while !whole(&read) {
        read.extend(read_until(&engine, |event| {
            matches!(
                event,
                Event::Reading(ReadingEvent::Series(_) | ReadingEvent::Chapters { .. })
            )
        }));
    }
    let pages: Vec<SeriesView> = read
        .iter()
        .filter_map(|event| match event {
            Event::Reading(ReadingEvent::Series(view)) if !view.recommended.is_empty() => {
                Some((**view).clone())
            }
            _ => None,
        })
        .collect();
    let page = pages.last().unwrap();
    assert_eq!(page.titles.display, "The Lantern Ferry");
    assert_eq!(page.format.as_deref(), Some("manga"));
    assert_eq!(page.origin.as_deref(), Some("JP"));
    assert_eq!(page.status.as_deref(), Some("releasing"));
    assert_eq!(page.list, Some(LibrarySegmentRead::Reading));
    assert!(
        matches!(&page.action, SeriesAction::Continue { chapter, page: 10, .. } if chapter.as_str() == "13")
    );
    let show = page.other_medium.as_ref().unwrap();
    assert_eq!(show.title, "The Library Above the Clouds");
    assert_eq!(show.relation, Msg::Reading(ReadingMsg::ReadingAlsoAShow));
    assert!(!page.recommended.is_empty());
    // The scene's folder holds chapters 12 to 15, which the folder lists afresh.
    let edition = page.edition.as_ref().expect("the folder's edition");
    assert_eq!((edition.chapters, edition.latest.as_deref()), (4, Some("15")));

    let chapters = read.iter().rev().find_map(|event| match event {
        Event::Reading(ReadingEvent::Chapters { volumes, failure, .. }) => {
            Some((volumes.clone(), failure.clone()))
        }
        _ => None,
    });
    let (volumes, failure) = chapters.expect("the chapters");
    assert!(failure.is_none(), "{failure:?}");
    let rows: Vec<_> = volumes.iter().flat_map(|v| &v.chapters).collect();
    assert_eq!(rows.len(), 4);
    let newest = rows.iter().find(|r| r.id.as_str() == "15").unwrap();
    assert!(newest.new_chapter);
    // The book's own details call it "Chapter 15", which is no title of its own.
    assert_eq!(newest.title, None);

    engine.send(Command::Reading(ReadingCommand::LoadVersions {
        work: kaeri_core::books::WorkId::new("a:900301"),
        chapter: kaeri_core::books::ChapterId::new("15"),
    }));
    let read = read_until(&engine, |event| {
        matches!(event, Event::Reading(ReadingEvent::Versions { .. }))
    });
    let versions = read.iter().find_map(|event| match event {
        Event::Reading(ReadingEvent::Versions { versions, .. }) => Some(versions.clone()),
        _ => None,
    });
    let versions = versions.unwrap();
    assert_eq!(versions.len(), 1);
    assert!(!versions[0].chapter_ref.is_empty());
    engine.shutdown();
}

#[test]
fn the_lists_and_updates_come_from_what_was_kept() {
    let (_root, engine) = start(FfiScene::ReadingHome);
    engine.send(Command::Reading(ReadingCommand::LoadLibrary {
        segment: LibrarySegmentRead::Planning,
    }));
    let read = read_until(
        &engine,
        |event| matches!(event, Event::Reading(ReadingEvent::Library { tiles, .. }) if tiles.len() == 2),
    );
    assert!(read.iter().any(|event| matches!(
        event,
        Event::Reading(ReadingEvent::Library { tiles, .. })
            if tiles.iter().map(|t| t.work.0.as_str()).collect::<Vec<_>>() == ["a:900304", "a:900306"]
                || tiles.iter().map(|t| t.work.0.as_str()).collect::<Vec<_>>() == ["a:900306", "a:900304"]
    )));

    engine.send(Command::Reading(ReadingCommand::LoadUpdates));
    let read =
        read_until(&engine, |event| matches!(event, Event::Reading(ReadingEvent::Updates(_))));
    let updates = read
        .iter()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Updates(view)) => Some(view.clone()),
            _ => None,
        })
        .unwrap();
    let rows: Vec<_> = updates.days.iter().flat_map(|d| &d.rows).collect();
    assert_eq!(rows.first().map(|r| r.chapter.id.as_str()), Some("15"));
    assert!(updates.unread >= 2, "{updates:?}");
    assert!(updates.checked_at.is_some());
    engine.shutdown();
}
