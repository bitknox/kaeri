//! The Sources list on the scenes that seed media servers: the real engine lists each saved
//! server as a source of kind Server, so the scene's Providers event does too.
#![cfg(feature = "testing")]

use std::{sync::Arc, thread, time::Duration};

use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiScene};
use kaeri_model::{command::Command, event::Event};

/// How long a test waits for what it is waiting on before failing: everything is on loopback.
const PATIENCE: Duration = Duration::from_secs(40);

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

/// The engine on `scene` with Home's world in it, in a home of its own that lasts as long as
/// the engine.
fn started(scene: FfiScene) -> (Arc<FfiEngine>, tempfile::TempDir) {
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
    (engine, root)
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

/// The Providers event the engine sends when asked to check its sources.
fn providers(engine: &Arc<FfiEngine>) -> Vec<kaeri_model::sources::ProviderRow> {
    engine.send(Command::CheckProviders);
    read_until(engine, |e| matches!(e, Event::Providers(_)))
        .into_iter()
        .rev()
        .find_map(|e| match e {
            Event::Providers(rows) => Some(rows),
            _ => None,
        })
        .expect("a Providers event")
}

#[test]
fn the_servers_scene_lists_its_server_as_a_source() {
    let (engine, _root) = started(FfiScene::Servers);
    let rows = providers(&engine);
    let servers: Vec<_> = rows.iter().filter(|r| r.kind == "server").collect();
    assert_eq!(servers.len(), 1, "rows: {rows:?}");
    assert_eq!(servers[0].name, "Mika's Jellyfin");
}

#[test]
fn a_signed_out_server_is_not_a_source() {
    let (engine, _root) = started(FfiScene::SourcesBusy);
    let rows = providers(&engine);
    let names: Vec<_> =
        rows.iter().filter(|r| r.kind == "server").map(|r| r.name.as_str()).collect();
    // One that stopped answering is still a source: its titles stay where they were.
    assert_eq!(names, ["Mika's Jellyfin", "Attic server"], "rows: {rows:?}");
}

#[test]
fn the_busy_scene_has_a_server_that_stopped_answering_and_says_when_it_was_seen() {
    use kaeri_model::servers::{ServerCommand, ServerEvent};
    let (engine, _root) = started(FfiScene::SourcesBusy);
    let now = kaeri_engine::now_epoch();
    engine.send(Command::Server(ServerCommand::Load));
    let rows = read_until(&engine, |e| matches!(e, Event::Server(ServerEvent::Servers(_))))
        .into_iter()
        .rev()
        .find_map(|e| match e {
            Event::Server(ServerEvent::Servers(rows)) => Some(rows),
            _ => None,
        })
        .expect("the servers");
    let attic = rows.iter().find(|r| r.name == "Attic server").expect("Attic server");
    assert!(attic.not_answering && !attic.signed_out);
    let seen = attic.last_seen.expect("when it was last seen");
    assert!((now - 2 * 3_600 - 60..=now - 2 * 3_600 + 60).contains(&seen), "{seen} {now}");
    let others: Vec<_> = rows.iter().filter(|r| r.not_answering).map(|r| &r.name).collect();
    assert_eq!(others, ["Attic server"], "the scene's other servers still answer");
}

/// The Fix this match search's answer, from the engine's events.
fn search_answer(
    engine: &Arc<FfiEngine>,
    provider_id: Option<String>,
) -> (Vec<kaeri_model::sources::MatchCandidate>, Option<String>) {
    use kaeri_core::ids::AnilistId;
    engine.send(Command::ManualSearch {
        id: AnilistId::new(900_001),
        query: "Stardust".into(),
        provider_id,
    });
    let events = read_until(engine, |e| {
        matches!(e, Event::MatchChoices { .. } | Event::MatchSearchFailed { .. })
    });
    let answer = events
        .into_iter()
        .find(|e| matches!(e, Event::MatchChoices { .. } | Event::MatchSearchFailed { .. }))
        .expect("an answer");
    match answer {
        Event::MatchChoices { candidates, source, .. } => (candidates, Some(source)),
        other => panic!("the search failed: {other:?}"),
    }
}

#[test]
fn a_search_in_a_scene_finds_candidates_with_made_up_titles() {
    let (engine, _root) = started(FfiScene::Servers);
    let (candidates, _) = search_answer(&engine, None);
    assert!(!candidates.is_empty());
}

#[test]
fn a_search_on_the_scenes_server_finds_its_saved_series() {
    let (engine, _root) = started(FfiScene::Servers);
    let server =
        providers(&engine).into_iter().find(|r| r.kind == "server").expect("a server").id;
    let (candidates, source) = search_answer(&engine, Some(server));
    assert!(!candidates.is_empty(), "candidates: {candidates:?}");
    assert_eq!(source.as_deref(), Some("Mika's Jellyfin"));
}

/// Fix this match on the first group of the Library scene, with the title's other name.
#[test]
fn fix_this_match_finds_candidates_by_a_titles_other_name() {
    use kaeri_model::local::{LocalCommand, LocalEvent};
    let (engine, _root) = started(FfiScene::Library);
    engine.send(Command::Local(LocalCommand::Load));
    let group = read_until(&engine, |e| matches!(e, Event::Local(LocalEvent::View(_))))
        .into_iter()
        .rev()
        .find_map(|e| match e {
            Event::Local(LocalEvent::View(view)) => {
                view.groups.into_iter().next().map(|g| g.id)
            }
            _ => None,
        })
        .expect("a group");
    engine.send(Command::Local(LocalCommand::FindMatch {
        group,
        query: "Hoshikuzu no Tabibito".into(),
    }));
    let found = read_until(&engine, |e| matches!(e, Event::Local(LocalEvent::Matches { .. })));
    let choices = found
        .into_iter()
        .find_map(|e| match e {
            Event::Local(LocalEvent::Matches { choices, .. }) => Some(choices),
            _ => None,
        })
        .expect("an answer");
    assert!(!choices.is_empty(), "the fixture answers no candidates");
}
