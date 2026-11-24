//! The still clock on a scene: Home's Slow scene held at a moment shows what it shows that long
//! in, at once and however slow the machine, and moving the hold on lets the slow answers in.
//! A file of its own, as the clock is the process's.
#![cfg(feature = "testing")]

use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use kaeri_ffi::{
    FfiDirs, FfiEngine, FfiHost, FfiScene, testing_still_clock_hold,
    testing_still_clock_hold_seconds, testing_still_clock_now_ms,
};
use kaeri_model::{
    command::Command,
    event::Event,
    home::{HomeView, SectionKind},
};

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

/// The Homes the engine sends within `patience`, until one has nothing left on its way.
fn homes_within(engine: &Arc<FfiEngine>, patience: Duration) -> (Vec<HomeView>, bool) {
    let engine = Arc::clone(engine);
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        loop {
            let batch = futures::executor::block_on(engine.next_events());
            if batch.is_empty() {
                break;
            }
            for event in batch {
                if let Event::Home(home) = event
                    && tx.send(*home).is_err()
                {
                    return;
                }
            }
        }
    });
    let until = Instant::now() + patience;
    let mut homes = Vec::new();
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        let Ok(home) = rx.recv_timeout(left) else { break };
        let done = home.end.is_some() && home.pending.is_empty();
        homes.push(home);
        if done {
            return (homes, true);
        }
    }
    (homes, false)
}

#[test]
fn a_held_slow_scene_waits_for_the_hold_and_answers_once_it_moves_on() {
    assert_eq!(testing_still_clock_now_ms(), None, "the clock is off until a shot holds it");
    assert!(!testing_still_clock_hold_seconds("soon".into()));
    assert_eq!(testing_still_clock_now_ms(), None);
    // Held at 2 s: the discovery answers 4.5 s in, so it is still on its way.
    assert!(testing_still_clock_hold_seconds("2".into()));
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_home_scene_on(
        dirs,
        Arc::new(Quiet),
        FfiScene::Slow,
        kaeri_engine::clock::Clock::default(),
    )
    .expect("the engine starts");
    engine.send(Command::LoadHome { refresh: false });
    let (homes, settled) = homes_within(&engine, Duration::from_secs(8));
    assert!(!settled, "the discovery answered before its moment: {:?}", homes.last());
    let first = homes.iter().find(|h| h.section(SectionKind::Continue).is_some());
    assert!(first.is_some_and(|h| !h.pending.is_empty()), "no Continue first: {homes:?}");
    assert_eq!(testing_still_clock_now_ms(), Some(2_000), "the clock stopped at the hold");

    // On to 5 s: past the answers' 4.5 s, which come in at once.
    let moved = Instant::now();
    testing_still_clock_hold(5_000);
    let (homes, settled) = homes_within(&engine, Duration::from_secs(20));
    assert!(settled, "the discovery did not answer once the hold moved on: {:?}", homes.last());
    assert!(moved.elapsed() < Duration::from_secs(4), "it waited on the wall clock");
    let home = homes.last().unwrap();
    assert!(!home.failed, "{home:?}");
    assert_eq!(testing_still_clock_now_ms(), Some(5_000));
}
