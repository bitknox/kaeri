//! The sources-loading scene, as the player panel's Sources tab drives it: every source's row at
//! once, in the order set in Sources, then each one as it answers, a slow one past the others, a
//! failed one and one that took too long, and the slate after the last.
#![cfg(feature = "testing")]

use std::{sync::Arc, thread, time::Duration};

use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiScene};
use kaeri_model::{
    command::Command,
    event::Event,
    starting::{ListingMiss, ListingState, SourceListing},
};
use kaeri_testkit::fixtures::FINISHED;

/// How long the test waits for the slate: the slowest source takes 20 s to time out.
const PATIENCE: Duration = Duration::from_secs(60);

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

/// Each row in short: its source and where it is.
fn states(rows: &[SourceListing]) -> Vec<String> {
    rows.iter()
        .map(|row| {
            let state = match &row.state {
                ListingState::Asking => "asking".to_owned(),
                ListingState::Listed { releases } => format!("listed {}", releases.len()),
                ListingState::Missed { miss } => format!("{miss:?}"),
                ListingState::Off => "off".to_owned(),
            };
            format!("{} {state}", row.provider_id.trim_start_matches("fixture-"))
        })
        .collect()
}

/// The scene's engine, on folders of `root`.
fn scene(root: &tempfile::TempDir) -> Arc<FfiEngine> {
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    FfiEngine::start_scene_on(
        dirs,
        Arc::new(Quiet),
        FfiScene::SourcesLoading { media: "/tmp/episode.mkv".into() },
        kaeri_engine::clock::Clock::default(),
    )
    .expect("the engine starts")
}

#[test]
fn the_sources_tab_fills_row_by_row_as_each_source_answers() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = scene(&root);

    let began = std::time::Instant::now();
    engine.send(Command::LoadSources { id: FINISHED, episode: "7".into() });
    let read = read_until(&engine, |event| matches!(event, Event::Sources { .. }));
    let took = began.elapsed();
    let said: Vec<_> = read
        .iter()
        .filter_map(|event| match event {
            Event::SourcesProgress { id, episode, rows } => {
                assert_eq!((*id, episode.as_str()), (FINISHED, "7"));
                Some(states(rows))
            }
            _ => None,
        })
        .collect();

    // Every row at once, in the order set in Sources, the switched off one said so.
    assert_eq!(
        said[0],
        [
            "fixture asking",
            "home-server asking",
            "media-folder asking",
            "example-index asking",
            "second-example asking",
            "archive-example asking",
            "old-example off",
        ]
    );
    // Then as each answers, the slowest last.
    assert_eq!(
        said.last().unwrap(),
        &[
            "fixture NoEpisode",
            "home-server listed 1",
            "media-folder Ambiguous",
            "example-index listed 5",
            "second-example TimedOut",
            "archive-example Failed",
            "old-example off",
        ]
    );
    let order: Vec<_> = said
        .windows(2)
        .map(|pair| {
            let changed = pair[0].iter().zip(&pair[1]).position(|(a, b)| a != b);
            pair[1][changed.expect("each progress changes one row")].clone()
        })
        .collect();
    assert_eq!(
        order,
        [
            "fixture NoEpisode",
            "home-server listed 1",
            "media-folder Ambiguous",
            "example-index listed 5",
            "archive-example Failed",
            "second-example TimedOut",
        ]
    );
    // Together, so the slowest source's 20 s, not the sum of them.
    assert!(took >= Duration::from_secs(20) && took < Duration::from_secs(29), "{took:?}");

    let slate = read
        .into_iter()
        .find_map(|event| match event {
            Event::Sources { candidates, missed, .. } => Some((candidates, missed)),
            _ => None,
        })
        .expect("the slate");
    assert_eq!(slate.0.len(), 6, "Home server's one release and Example index's five");
    let missed: Vec<_> = slate.1.iter().map(|m| (m.name.as_str(), m.miss)).collect();
    assert_eq!(
        missed,
        [
            ("fixture", ListingMiss::NoEpisode),
            ("Media folder", ListingMiss::Ambiguous),
            ("Second example", ListingMiss::TimedOut),
            ("Archive example", ListingMiss::Failed),
        ]
    );
}

#[test]
fn each_source_says_its_coverage_at_once_for_the_tab_lines() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = scene(&root);

    // The tab asks for the sources' lines as it opens: only the episode playing is slow.
    let began = std::time::Instant::now();
    engine.send(Command::LoadSourceOptions(FINISHED));
    let read = read_until(&engine, |event| matches!(event, Event::SourceOptions { .. }));
    let took = began.elapsed();
    let options = read
        .into_iter()
        .find_map(|event| match event {
            Event::SourceOptions { id, options } if id == FINISHED => Some(options),
            _ => None,
        })
        .expect("the sources' lines");
    assert!(took < Duration::from_secs(5), "{took:?}");
    for listed in ["fixture-home-server", "fixture-example-index"] {
        let option = options.iter().find(|o| o.provider_id == listed).expect(listed);
        assert!(option.covered > 0 && option.covered == option.of, "{option:?}");
    }
}

#[test]
fn the_scene_opens_on_the_player_with_no_question_first() {
    use kaeri_model::medium::{KaeriIsFor, MediumCommand};
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = scene(&root);
    engine.send(Command::Medium(MediumCommand::Load));
    let read = read_until(&engine, |event| matches!(event, Event::Medium(_)));
    let Some(Event::Medium(view)) =
        read.into_iter().rev().find(|e| matches!(e, Event::Medium(_)))
    else {
        unreachable!()
    };
    // Its config is written, so without an answer in it a Fixtures app asks Watch, read or both.
    assert_eq!((view.ask, view.answer), (None, KaeriIsFor::Watch));
    engine.shutdown();
}
