//! The missing-episode scene, as an app drives it: the page says episode 6 aired and no source
//! has it, Source for this title says which episodes each source has, the first Check again comes
//! back empty and the second finds it.
#![cfg(feature = "testing")]

use std::{sync::Arc, thread, time::Duration};

use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiScene, MISSING_AIRED_AGO};
use kaeri_model::{
    command::Command,
    event::Event,
    missing::{
        CheckOutcome, CoverageLine, Lacking, MissingCommand, MissingPhase, coverage_line,
        missing_episode,
    },
};
use kaeri_testkit::fixtures::AIRING;

/// How long a test waits for what it is waiting on before failing: everything is on loopback.
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

/// The engine's events until one `until` accepts, polled on a thread of its own.
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

fn checked(engine: &Arc<FfiEngine>) -> CheckOutcome {
    engine.send(Command::Missing(MissingCommand::CheckAgain { id: AIRING, episode: 6 }));
    let read = read_until(engine, |event| matches!(event, Event::EpisodeChecked(_)));
    read.into_iter()
        .find_map(|event| match event {
            Event::EpisodeChecked(check) => {
                assert_eq!((check.id, check.episode, check.asked), (AIRING, 6, true));
                Some(check.outcome)
            }
            _ => None,
        })
        .expect("a check")
}

#[test]
fn the_page_says_episode_six_is_missing_and_the_second_check_finds_it() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let clock = kaeri_engine::clock::Clock::default();
    let engine = FfiEngine::start_scene_on(
        dirs,
        Arc::new(Quiet),
        FfiScene::MissingEpisode,
        clock.clone(),
    )
    .expect("the engine starts");

    // Launch as an app does: Home first, then the page.
    engine.send(Command::LoadHome { refresh: false });
    read_until(&engine, |event| matches!(event, Event::Home(_)));
    engine.send(Command::LoadEpisodes(AIRING));
    let read = read_until(
        &engine,
        |event| matches!(event, Event::Episodes { id, rows, .. } if *id == AIRING && rows.len() >= 6),
    );
    let rows = read
        .into_iter()
        .rev()
        .find_map(|event| match event {
            Event::Episodes { id, rows, .. } if id == AIRING => Some(rows),
            _ => None,
        })
        .expect("the table");
    let missing = missing_episode(rows, 6, false, clock.now()).expect("episode 6 is missing");
    assert_eq!(
        (missing.episode, missing.phase, missing.previous),
        (6, MissingPhase::Quiet, Some(5))
    );
    let ago = clock.now() - missing.aired_at.expect("its airing time");
    assert!((MISSING_AIRED_AGO..MISSING_AIRED_AGO + 120).contains(&ago), "aired {ago} s ago");

    assert_eq!(checked(&engine), CheckOutcome::Missing, "the first check comes back empty");
    assert_eq!(checked(&engine), CheckOutcome::Found, "the second finds it");

    // Source for this title, once found: the fixture source has every episode, Yamabiko not
    // the last two, and Minato no match.
    engine.send(Command::LoadSourceOptions(AIRING));
    let read = read_until(&engine, |event| matches!(event, Event::SourceOptions { .. }));
    let options = read
        .into_iter()
        .find_map(|event| match event {
            Event::SourceOptions { options, .. } => Some(options),
            _ => None,
        })
        .expect("the options");
    let line = |id: &str| {
        let option = options.iter().find(|o| o.provider_id == id).expect("the source").clone();
        coverage_line(option, clock.now())
    };
    assert_eq!(line("fixture"), Some(CoverageLine::Episodes { first: 1, last: 6, count: 6 }));
    assert_eq!(
        line("fixture-yamabiko"),
        Some(CoverageLine::NotYet {
            first: 1,
            last: 4,
            lacking: Lacking::Pair { first: 5, second: 6 }
        })
    );
    assert_eq!(line("fixture-minato"), None, "a note in place of a count");
}
