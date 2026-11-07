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

    let (_root, engine) = start(FfiScene::ReadingDemo);
    let view = medium(&engine);
    assert_eq!((view.ask, view.answer), (None, KaeriIsFor::Read));
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
fn the_downloads_scene_fills_the_screen_and_fetches_nothing_by_itself() {
    use kaeri_model::reading::ChapterDownloadState as S;
    let (_root, engine) = start(FfiScene::ReadingDownloads);
    engine.send(Command::Reading(ReadingCommand::LoadDownloads));
    let read = read_until(&engine, |event| {
        matches!(event, Event::Reading(ReadingEvent::Downloads(_)))
    });
    let view = read
        .into_iter()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Downloads(view)) => Some(view),
            _ => None,
        })
        .expect("the downloads");
    let states: Vec<_> =
        view.rows.iter().map(|r| (r.chapter.id.as_str().to_owned(), r.state.clone())).collect();
    assert_eq!(
        states,
        [
            ("14".to_owned(), S::Done),
            ("15".to_owned(), S::Downloading { done: 6, total: 20 }),
            ("16".to_owned(), S::Queued),
            ("17".to_owned(), S::Queued),
            ("4".to_owned(), S::Done),
            ("2".to_owned(), S::Failed { failure: ReadingFailure::Offline }),
        ]
    );
    assert_eq!(view.rows[0].title, "The Lantern Ferry");
    assert_eq!((view.meter.chapters, view.meter.used), (2, 10_600_000));
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

fn surface() -> kaeri_model::reader::ReaderSurface {
    kaeri_model::reader::ReaderSurface {
        width_px: 1080,
        height_px: 2340,
        scale: 2.75,
        eink: false,
        two_pages: false,
        dark: true,
    }
}

/// Opens `work` at its resume place and returns the first chapter's events up to its leaves.
fn open_reader(scene: FfiScene, work: &str) -> Vec<Event> {
    use kaeri_core::books::WorkId;
    use kaeri_model::reader::{OpenAt, ReaderCommand, ReaderEvent};
    let (_root, engine) = start(scene);
    engine.send(Command::Reader(ReaderCommand::Open {
        work: WorkId::new(work),
        chapter: None,
        at: OpenAt::Resume,
        surface: surface(),
    }));
    let read = read_until(&engine, |event| {
        matches!(
            event,
            Event::Reader(ReaderEvent::Leaves { .. } | ReaderEvent::NovelOpen(_))
                | Event::Reading(ReadingEvent::Failed { .. })
        )
    });
    engine.shutdown();
    read
}

#[test]
fn the_reader_scenes_have_generated_books_to_open() {
    use kaeri_model::reader::{ReaderEvent, ReaderMode};
    let manga = open_reader(FfiScene::ReaderMangaRtl, "a:900301");
    let leaves = manga
        .iter()
        .find_map(|e| match e {
            Event::Reader(ReaderEvent::Leaves { leaves, .. }) => Some(leaves.len()),
            _ => None,
        })
        .expect("the manga's leaves");
    assert!(leaves >= 20, "{leaves} leaves");
    let opened = manga
        .iter()
        .find_map(|e| match e {
            Event::Reader(ReaderEvent::Opened(view)) => Some(view.clone()),
            _ => None,
        })
        .expect("opened");
    assert_eq!(opened.title, "The Lantern Ferry");
    assert_eq!(opened.chapter.id.as_str(), "13");
    assert_eq!(opened.start.page, Some(9));

    let strip = open_reader(FfiScene::ReaderStrip, "a:900303");
    let opened = strip
        .iter()
        .find_map(|e| match e {
            Event::Reader(ReaderEvent::Opened(view)) => Some(view.clone()),
            _ => None,
        })
        .expect("opened");
    assert_eq!((opened.chapter.id.as_str(), opened.mode), ("2", ReaderMode::Strip));

    let novel = open_reader(FfiScene::ReaderNovel, "a:900302");
    assert!(
        novel.iter().any(|e| matches!(e, Event::Reader(ReaderEvent::NovelOpen(_)))),
        "{novel:?}"
    );
}

#[test]
fn the_series_scene_lists_the_manga_chapters_from_its_folder() {
    use kaeri_core::books::WorkId;
    let (_root, engine) = start(FfiScene::ReadingSeries);
    engine.send(Command::Reading(ReadingCommand::LoadChapters {
        work: WorkId::new("a:900301"),
        edition: None,
    }));
    let read = read_until(&engine, |event| {
        matches!(event, Event::Reading(ReadingEvent::Chapters { .. }))
    });
    let (volumes, failure) = read
        .into_iter()
        .find_map(|event| match event {
            Event::Reading(ReadingEvent::Chapters { volumes, failure, .. }) => {
                Some((volumes, failure))
            }
            _ => None,
        })
        .expect("the chapters");
    assert_eq!(failure, None, "the folder series the catalogue names is the generated one");
    assert!(volumes.iter().any(|v| !v.chapters.is_empty()), "chapters listed: {volumes:?}");
}

#[test]
fn a_reading_scene_started_again_on_its_home_keeps_the_reader_settings_changed_in_it() {
    use kaeri_core::{books::WorkId, media::Medium};
    use kaeri_model::reader::{
        OpenAt, ReaderCommand, ReaderEvent, ReaderMode, ReaderSettingKey, ReaderValue,
        SettingScope,
    };
    let root = tempfile::tempdir().expect("a temporary directory");
    let launch = || {
        let dir = |name: &str| root.path().join(name).display().to_string();
        let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
        FfiEngine::start_scene_on(
            dirs,
            Arc::new(Quiet),
            FfiScene::ReaderStrip,
            kaeri_engine::clock::Clock::default(),
        )
        .expect("the engine starts")
    };
    let strip = WorkId::new("a:900303");
    let settled = |event: &Event| matches!(event, Event::Reader(ReaderEvent::Settings(_)));

    // The scene's strip series reads as a strip; the person has it read in pages and picks
    // sepia for every manga.
    let engine = launch();
    engine.send(Command::Reader(ReaderCommand::SetSetting {
        session: None,
        medium: Medium::Manga,
        work: Some(strip.clone()),
        key: ReaderSettingKey::Mode,
        scope: SettingScope::Series,
        value: ReaderValue::Choice { name: "paged".into() },
    }));
    read_until(&engine, settled);
    engine.send(Command::Reader(ReaderCommand::SetSetting {
        session: None,
        medium: Medium::Manga,
        work: None,
        key: ReaderSettingKey::Look,
        scope: SettingScope::Medium,
        value: ReaderValue::Choice { name: "sepia".into() },
    }));
    read_until(&engine, settled);
    engine.shutdown();

    let engine = launch();
    engine.send(Command::Reader(ReaderCommand::Open {
        work: strip,
        chapter: None,
        at: OpenAt::Resume,
        surface: surface(),
    }));
    let read = read_until(&engine, |event| {
        matches!(
            event,
            Event::Reader(ReaderEvent::Leaves { .. })
                | Event::Reading(ReadingEvent::Failed { .. })
        )
    });
    let opened = read
        .iter()
        .find_map(|e| match e {
            Event::Reader(ReaderEvent::Opened(view)) => Some(view.clone()),
            _ => None,
        })
        .expect("opened");
    assert_eq!(opened.mode, ReaderMode::Paged, "the series' own choice stays");
    engine.send(Command::Reader(ReaderCommand::LoadSettings {
        medium: Medium::Manga,
        work: Some(opened.work.clone()),
    }));
    let read = read_until(&engine, settled);
    engine.shutdown();
    let look = read
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Reader(ReaderEvent::Settings(view)) => Some(view.clone()),
            _ => None,
        })
        .and_then(|view| view.rows.into_iter().find(|row| row.key == ReaderSettingKey::Look))
        .map(|row| row.value);
    assert_eq!(look, Some(ReaderValue::Choice { name: "sepia".into() }));
}
