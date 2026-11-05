//! The reading scenes: the medium question each asks, and reading refused while it ships dark.
#![cfg(feature = "testing")]

use std::{sync::Arc, thread, time::Duration};

use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiScene};
use kaeri_model::{
    command::Command,
    event::Event,
    medium::{KaeriIsFor, MediumAsk, MediumCommand, MediumView},
    reading::{ReadingCommand, ReadingEvent, ReadingFailure},
};

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

fn start(scene: FfiScene) -> (tempfile::TempDir, Arc<FfiEngine>) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_scene_on(
        dirs,
        Arc::new(Quiet),
        scene,
        kaeri_engine::clock::Clock::default(),
    )
    .expect("the engine starts");
    (root, engine)
}

fn medium(engine: &Arc<FfiEngine>) -> MediumView {
    engine.send(Command::Medium(MediumCommand::Load));
    let read = read_until(engine, |event| matches!(event, Event::Medium(_)));
    read.into_iter()
        .rev()
        .find_map(|event| match event {
            Event::Medium(view) => Some(view),
            _ => None,
        })
        .expect("the medium")
}

#[test]
fn each_scene_asks_its_own_question() {
    let (_root, engine) = start(FfiScene::ReadingFirstRun);
    assert_eq!(medium(&engine).ask, Some(MediumAsk::FirstRun));
    engine.shutdown();

    let (_root, engine) = start(FfiScene::ReadingUpdateSheet);
    let view = medium(&engine);
    assert_eq!((view.ask, view.answer), (Some(MediumAsk::Updated), KaeriIsFor::Watch));
    engine.shutdown();

    let (_root, engine) = start(FfiScene::ReadingJoinAsk);
    assert_eq!(
        medium(&engine).ask,
        Some(MediumAsk::Joined {
            devices: vec!["Kim's iPad".into()],
            answer: KaeriIsFor::Read
        })
    );
    engine.send(Command::Medium(MediumCommand::Answer {
        ask: MediumAsk::Joined { devices: vec!["Kim's iPad".into()], answer: KaeriIsFor::Read },
        answer: KaeriIsFor::Read,
    }));
    let view = medium(&engine);
    assert_eq!((view.ask, view.answer), (None, KaeriIsFor::Read));
    engine.shutdown();

    let (_root, engine) = start(FfiScene::ReadingHome);
    let view = medium(&engine);
    assert!(view.reading_available && view.ask.is_none());
    assert_eq!(view.answer, KaeriIsFor::Both);
    engine.shutdown();
}

#[test]
fn the_dark_scene_is_watch_and_refuses_reading() {
    let (_root, engine) = start(FfiScene::ReadingDark);
    let view = medium(&engine);
    assert!(!view.reading_available);
    assert_eq!((view.answer, view.ask), (KaeriIsFor::Watch, None));
    engine.send(Command::Reading(ReadingCommand::LoadUpdates));
    let read = read_until(&engine, |event| matches!(event, Event::Reading(_)));
    assert!(read.iter().any(|event| matches!(
        event,
        Event::Reading(ReadingEvent::Failed { failure: ReadingFailure::ReadingOff, .. })
    )));
    engine.shutdown();
}
