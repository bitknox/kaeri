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
        LibrarySegmentRead, ReadHomeEnd, ReadRowKind, ReadSearchQuery, ReadSpotlightCase,
        ReadingCommand, ReadingEvent, ReadingHomeView, SeriesAction, SeriesView,
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
    assert_eq!(manga.title, "Mr. Arai Runs the Night Ferry");
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

    // Discovery: manga read most, light novels next, a webtoon on Reading. The fixtures fill
    // every row the board draws, each title once and never what the person reads.
    let rows = &home.rows;
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let board = [
        "trending-manga",
        "trending-novels",
        "trending-webtoons",
        "because",
        "new-manga",
        "new-novels",
        "genre",
        "finished-manga",
    ];
    assert_eq!(ids, board, "every row of the board's person with history");
    assert_eq!(home.releasing, rows[0].tiles, "the old field shows the first Trending row");
    let shown: Vec<String> = rows.iter().flat_map(|r| works(&r.tiles)).collect();
    let once: std::collections::HashSet<&String> = shown.iter().collect();
    assert_eq!(once.len(), shown.len(), "{shown:?}");
    assert!(shown.iter().all(|w| !continuing.contains(w)), "{shown:?}");
    let own = ["a:900301", "a:900302", "a:900303", "a:900304", "a:900306", "a:900307"];
    assert!(shown.iter().all(|w| !own.contains(&w.as_str())), "{shown:?}");
    assert!(rows.iter().flat_map(|r| &r.tiles).all(|t| !t.adult));
    for row in rows {
        assert!(!row.pending && row.tiles.len() >= 5, "{} has {}", row.id, row.tiles.len());
    }

    // Because you finished The Quiet Observatory: what it led to, without the person's own.
    let because = &rows[3];
    assert_eq!(because.title.english(), "Because you finished The Quiet Observatory");
    assert_eq!(home.because_title.as_deref(), Some("The Quiet Observatory"));
    assert_eq!(home.because, because.tiles);
    assert_eq!(because.view_all, None);
    // More of the genre read most, with its View all.
    let genre = &rows[6];
    assert_eq!(genre.kind, ReadRowKind::MoreGenre);
    assert_eq!(genre.title.english(), "More slice of life");
    assert_eq!(
        genre.view_all_title.as_ref().map(|t| t.english()),
        Some("More slice of life".into())
    );
    assert_eq!(home.genre.as_deref(), Some("Slice of Life"));
    assert_eq!(
        genre.view_all.as_ref().map(|q| q.genres.clone()),
        Some(vec!["Slice of Life".into()])
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
        ReadSearchQuery { text: "ferry".into(), ..ReadSearchQuery::default() },
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
    // The page, the chapters and More like this come in any order. The edition's count follows
    // the folder's fresh list, so read until a whole page has come after the chapters.
    let mut read = Vec::new();
    let whole = |read: &[Event]| {
        let Some(chapters) = read
            .iter()
            .position(|event| matches!(event, Event::Reading(ReadingEvent::Chapters { .. })))
        else {
            return false;
        };
        read[chapters..].iter().any(|event| {
            matches!(event, Event::Reading(ReadingEvent::Series(view)) if !view.partial && !view.recommended.is_empty())
        })
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
    assert_eq!(page.titles.display, "Mr. Arai Runs the Night Ferry");
    assert_eq!(page.format.as_deref(), Some("manga"));
    assert_eq!(page.origin.as_deref(), Some("JP"));
    assert_eq!(page.status.as_deref(), Some("releasing"));
    assert_eq!(page.list, Some(LibrarySegmentRead::Reading));
    assert!(
        matches!(&page.action, SeriesAction::Continue { chapter, page: 10, .. } if chapter.as_str() == "13")
    );
    let show = page.other_medium.as_ref().unwrap();
    assert_eq!(show.title, "Mountain Library Night Shift");
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

fn search_tiles(
    engine: &Arc<FfiEngine>,
    query: ReadSearchQuery,
) -> Vec<kaeri_model::reading::WorkTile> {
    engine.send(Command::Reading(ReadingCommand::Search { query: query.clone() }));
    let read = read_until(
        engine,
        move |event| matches!(event, Event::Reading(ReadingEvent::Search { query: q, .. }) if *q == query),
    );
    read.into_iter()
        .rev()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Search { tiles, .. }) => Some(tiles),
            _ => None,
        })
        .expect("a search")
}

/// The series page of `work` once its chapters are in.
fn demo_page(engine: &Arc<FfiEngine>, work: &kaeri_core::books::WorkId) -> (SeriesView, usize) {
    engine.send(Command::Reading(ReadingCommand::LoadSeries { work: work.clone() }));
    let read = read_until(engine, |event| {
        matches!(event, Event::Reading(ReadingEvent::Chapters { .. }))
    });
    let chapters = read
        .iter()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Chapters {
                work: w, volumes, failure: None, ..
            }) if w == work => Some(volumes.iter().map(|v| v.chapters.len()).sum()),
            _ => None,
        })
        .expect("the demo's chapters, from the demo itself");
    let page = read
        .into_iter()
        .rev()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Series(view)) if view.work == *work => Some(*view),
            _ => None,
        })
        .expect("the series page");
    (page, chapters)
}

#[test]
fn the_reading_demo_opens_before_any_search_and_after_a_restart() {
    let (root, engine) = start(FfiScene::ReadingDemo);
    let info = kaeri_model::sources::reading_demo_info();
    let (comic, story) = (&info.works[0], &info.works[1]);

    // The story: its own page, as a novel, with its credit and the comic beside it.
    let (page, chapters) = demo_page(&engine, &story.work);
    assert_eq!(chapters, 1);
    assert_eq!(page.titles.display, "Kwaidan");
    assert_eq!(page.medium, kaeri_core::media::Medium::Novel);
    assert_eq!(page.demo.as_ref(), Some(story));
    assert_eq!(page.demo_other.as_ref(), Some(&comic.work));
    assert_eq!(page.cover, story.cover);
    assert_eq!(page.synopsis.as_deref(), Some(story.synopsis.as_str()));

    // Its cover comes with the app: the bundled frontispiece, whole, and nothing fetched.
    let url = page.cover.clone().expect("the story's cover");
    engine.send(Command::FetchArtwork { url: url.clone() });
    let wanted = url.clone();
    let read = read_until(
        &engine,
        move |event| matches!(event, Event::ArtworkReady { url, .. } | Event::ImageFailed { url, .. } if *url == wanted),
    );
    let bytes = read.into_iter().find_map(|event| match event {
        Event::ArtworkReady { url: u, bytes } if u == url => Some(bytes),
        _ => None,
    });
    assert_eq!(bytes.as_deref(), Some(kaeri_core::reading_demo::STORY_COVER.bytes));

    // The comic, with its cover.
    let (page, _) = demo_page(&engine, &comic.work);
    assert_eq!(page.medium, kaeri_core::media::Medium::Manga);
    assert_eq!(page.cover, comic.cover);
    assert_eq!(page.demo_other.as_ref(), Some(&story.work));
    engine.shutdown();

    // Started again on the same folders: the same works open.
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine =
        FfiEngine::start_scene_on(dirs, Arc::new(Quiet), FfiScene::ReadingDemo, at_midday())
            .expect("the engine starts again");
    let (page, chapters) = demo_page(&engine, &story.work);
    assert_eq!((page.demo.as_ref(), chapters), (Some(story), 1));
    engine.shutdown();
}

#[test]
fn a_first_run_home_shows_each_trending_row_then_new_manga() {
    // Only the demo, no history: nothing to base Because or a genre on.
    let (_root, engine) = start(FfiScene::ReadingDemo);
    let home = whole_home(&engine, false);
    let ids: Vec<&str> = home.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["trending-manga", "trending-webtoons", "trending-novels", "new-manga"]);
    assert!(home.rows.iter().all(|r| !r.pending && r.tiles.len() >= 5), "{:?}", home.rows);
    let shown: Vec<&str> =
        home.rows.iter().flat_map(|r| &r.tiles).map(|t| t.work.0.as_str()).collect();
    let once: std::collections::HashSet<&&str> = shown.iter().collect();
    assert_eq!(once.len(), shown.len(), "{shown:?}");
    engine.shutdown();
}

#[test]
fn search_finds_the_demo_after_anilist_only_while_the_demo_is_on() {
    let (_root, engine) = start(FfiScene::ReadingDemo);
    let comic = &kaeri_model::sources::reading_demo_info().works[0];
    let query =
        |text: &str| ReadSearchQuery { text: text.into(), ..ReadSearchQuery::default() };

    let found = search_tiles(&engine, query("pepper"));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].demo);
    assert_eq!(found[0].work, comic.work);
    assert_eq!(found[0].cover, comic.cover);

    // AniList's books first, then the demo's.
    let found = search_tiles(&engine, query("the"));
    let demo_at = found.iter().position(|t| t.demo).expect("the comic's chapter has a \"the\"");
    assert!(demo_at > 0, "AniList's books come first: {found:?}");
    assert!(found[..demo_at].iter().all(|t| !t.demo && t.work.0.starts_with("a:")));
    assert!(found[demo_at..].iter().all(|t| t.demo));

    // A filter the demo has nothing for, an empty query, or the novels only leave the comic out.
    let novels = ReadSearchQuery { formats: vec!["novel".into()], ..query("pepper") };
    assert!(search_tiles(&engine, novels).iter().all(|t| !t.demo));
    let browse = search_tiles(&engine, query(""));
    assert!(browse.iter().all(|t| !t.demo));

    // With the demo removed, nothing of it.
    engine.send(Command::RemoveDemo);
    read_until(&engine, |event| matches!(event, Event::Demo(state) if !state.on));
    assert!(search_tiles(&engine, query("pepper")).is_empty());
    let found = search_tiles(&engine, query("the"));
    assert!(!found.is_empty() && found.iter().all(|t| !t.demo), "{found:?}");
    engine.shutdown();
}

#[test]
fn search_without_the_demo_is_anilist_alone() {
    let (_root, engine) = start(FfiScene::ReadingHome);
    let found = search_tiles(
        &engine,
        ReadSearchQuery { text: "pepper".into(), ..ReadSearchQuery::default() },
    );
    assert!(found.is_empty(), "{found:?}");
    engine.shutdown();
}

/// Reads until a full series page (not the partial header) and its chapters have both come.
fn whole_series(
    engine: &Arc<FfiEngine>,
    work: &str,
) -> (SeriesView, Vec<kaeri_model::reading::ChapterRow>) {
    engine.send(Command::Reading(ReadingCommand::LoadSeries {
        work: kaeri_core::books::WorkId::new(work),
    }));
    let mut read = Vec::new();
    let done = |read: &[Event]| {
        let chapters = read
            .iter()
            .any(|event| matches!(event, Event::Reading(ReadingEvent::Chapters { .. })));
        let page = read.iter().any(
            |event| matches!(event, Event::Reading(ReadingEvent::Series(view)) if !view.partial),
        );
        chapters && page
    };
    while !done(&read) {
        read.extend(read_until(engine, |event| {
            matches!(
                event,
                Event::Reading(ReadingEvent::Series(_) | ReadingEvent::Chapters { .. })
            )
        }));
    }
    let page = read
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Series(view)) if !view.partial => {
                Some((**view).clone())
            }
            _ => None,
        })
        .unwrap();
    let rows = read
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Chapters { volumes, .. }) => {
                Some(volumes.iter().flat_map(|v| v.chapters.clone()).collect())
            }
            _ => None,
        })
        .unwrap();
    (page, rows)
}

#[test]
fn a_novels_button_and_cover_name_its_part_as_its_row_does() {
    let (_root, engine) = start(FfiScene::ReadingHome);
    let (page, rows) = whole_series(&engine, "a:900302");
    let home = whole_home(&engine, false);
    let SeriesAction::ContinuePart { chapter, named, percent, .. } = &page.action else {
        panic!("a novel continues a part: {:?}", page.action)
    };
    let row = rows.iter().find(|r| r.id == *chapter).expect("the part's row");
    // The button names the part with the row's own words, never a noun of its own.
    assert_eq!(named.as_deref(), Some(row.label.as_str()), "{row:?}");
    // And the row says the same percent the button does.
    let said =
        percent.map(|p| Msg::Reading(ReadingMsg::ReadingRowPercent { percent: i64::from(p) }));
    assert_eq!(row.line.first(), said.as_ref(), "{:?}", row.line);
    // Continue's cover names it the same way.
    let tile = home.continue_reading.iter().find(|t| t.work.0 == "a:900302").unwrap();
    let cover = tile.next.as_ref().expect("the cover's line").english();
    assert!(cover.starts_with(&row.label), "{cover} against {}", row.label);
    engine.shutdown();
}

#[test]
fn updates_asked_first_get_their_covers_and_medium_without_home() {
    let (_root, engine) = start(FfiScene::ReadingHome);
    // Nothing else asked yet, as after a notification opens Updates in a fresh process: the
    // view comes at once, then again once the books behind it are kept.
    engine.send(Command::Reading(ReadingCommand::LoadUpdates));
    let read = read_until(&engine, |event| {
        matches!(event, Event::Reading(ReadingEvent::Updates(view))
            if !view.days.is_empty() && view.days.iter().flat_map(|d| &d.rows).all(|r| r.tile.cover.is_some()))
    });
    let updates = read
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Updates(view)) => Some(view.clone()),
            _ => None,
        })
        .unwrap();
    let rows: Vec<_> = updates.days.iter().flat_map(|d| &d.rows).collect();
    assert!(rows.iter().all(|r| r.tile.cover.is_some()), "{rows:?}");
    engine.shutdown();
}

/// The sources' part of a search once every source has answered, failed or run out of time.
fn sources_done(
    engine: &Arc<FfiEngine>,
    query: ReadSearchQuery,
) -> kaeri_model::reading::ReadSourceSearch {
    engine.send(Command::Reading(ReadingCommand::Search { query: query.clone() }));
    let done = move |event: &Event| matches!(event, Event::Reading(ReadingEvent::SourceSearch(part)) if part.query == query && part.done);
    let read = read_until(engine, done);
    read.into_iter()
        .rev()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::SourceSearch(part)) => Some(*part),
            _ => None,
        })
        .expect("the sources' part")
}

#[test]
fn the_failing_source_scene_runs_out_of_time_on_northwind_alone() {
    use kaeri_model::reading::{ReadAnilistState, ReadSourceKind, ReadSourceState};
    let (_root, engine) = start(FfiScene::ReadingSourceFailed);
    let part = sources_done(
        &engine,
        ReadSearchQuery { text: "lantern".into(), ..ReadSearchQuery::default() },
    );
    assert_eq!(part.anilist, ReadAnilistState::Answered);
    let northwind =
        part.groups.iter().find(|g| g.source_name == "Northwind").expect("Northwind is asked");
    assert_eq!(northwind.kind, Some(ReadSourceKind::Server));
    assert_eq!(northwind.state, ReadSourceState::TimedOut { seconds: 15 });
    assert!(
        part.groups.iter().any(|g| g.source_name != "Northwind"
            && !matches!(
                g.state,
                ReadSourceState::TimedOut { .. } | ReadSourceState::Failed { .. }
            )),
        "the other sources answer: {:?}",
        part.groups
    );
    engine.shutdown();
}

#[test]
fn the_anilist_failed_scene_fails_reading_search_and_still_asks_the_sources() {
    use kaeri_model::reading::ReadAnilistState;
    let (_root, engine) = start(FfiScene::ReadingAnilistFailed);
    let part = sources_done(
        &engine,
        ReadSearchQuery { text: "lantern".into(), ..ReadSearchQuery::default() },
    );
    assert_eq!(part.anilist, ReadAnilistState::Failed);
    assert!(!part.groups.is_empty(), "the sources are still asked");
    engine.shutdown();
}
