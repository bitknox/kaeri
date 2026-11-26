//! The room as the bindings drive it: an invite read, and the board's scenes staged on the fake
//! room, each reaching the state its frame draws, with no network.
#![cfg(all(feature = "testing", feature = "room"))]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiRoomScene, FfiStart, parse_room_target};
use kaeri_model::{
    command::Command,
    event::Event,
    room::{
        MemberState, OldInvite, RoomCommand, RoomConnection, RoomLeftWhy, RoomMember,
        RoomMessageKind, RoomView,
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

fn engine(root: &tempfile::TempDir) -> Arc<FfiEngine> {
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    FfiEngine::start_testing(dirs, Arc::new(Quiet), FfiStart::default())
        .expect("the engine starts")
}

/// The room's view and members once `until` holds on them.
fn until(
    engine: &Arc<FfiEngine>,
    until: impl Fn(&RoomView, &[RoomMember], &[Event]) -> bool + Send + 'static,
) -> (RoomView, Vec<RoomMember>, Vec<Event>) {
    let engine = Arc::clone(engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut view, mut members, mut read) = (None, Vec::new(), Vec::new());
        loop {
            let batch = futures::executor::block_on(engine.next_events());
            if batch.is_empty() {
                let _ = tx.send(None);
                return;
            }
            for event in batch {
                match &event {
                    Event::RoomJoined(v) | Event::RoomState(v) => view = Some(v.clone()),
                    Event::RoomMembers(m) => members.clone_from(m),
                    _ => {}
                }
                read.push(event);
            }
            if let Some(v) = &view
                && until(v, &members, &read)
            {
                let _ = tx.send(Some((v.clone(), members, read)));
                return;
            }
        }
    });
    rx.recv_timeout(PATIENCE).expect("the state in time").expect("the engine went on")
}

fn state(members: &[RoomMember], name: &str) -> Option<MemberState> {
    members.iter().find(|m| m.name == name).map(|m| m.state)
}

fn staged(scene: FfiRoomScene) -> (tempfile::TempDir, Arc<FfiEngine>) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = engine(&root);
    let room = Arc::clone(&engine).start_fake_room();
    assert!(room.stage(scene), "{scene:?} staged");
    // The fake goes with the test; its rooms stay registered for the engine.
    std::mem::forget(room);
    (root, engine)
}

#[test]
fn old_invites_and_codes_say_what_they_are() {
    let old = parse_room_target(
        "https://kaeri.app/party#v=2&s=a.example:8999&r=kite-7qm4-xr9d".into(),
    );
    assert_eq!(old.unwrap().old, Some(OldInvite::SyncplayLink));
    assert_eq!(parse_room_target("kite 7qm4 xr9d".into()).unwrap().old, Some(OldInvite::Code));
    assert_eq!(parse_room_target("hello".into()), None);
}

#[test]
fn before_play_everyone_reads_ready_and_ren_loads() {
    let (_root, engine) = staged(FfiRoomScene::BeforePlay);
    let (view, members, _) = until(&engine, |v, m, _| {
        v.before_play
            && state(m, "Ren") == Some(MemberState::Loading)
            && state(m, "Ada") == Some(MemberState::Ready)
    });
    assert!(!view.playing);
    assert_eq!(state(&members, "Mika"), Some(MemberState::Ready));
    assert!(members[0].you, "the device first");
    engine.shutdown();
}

#[test]
fn starting_counts_three_once_everyone_is_ready() {
    let (_root, engine) = staged(FfiRoomScene::Starting);
    let (view, _, _) = until(&engine, |v, _, _| v.starts_at_ms.is_some() || v.playing);
    assert!(view.starts_at_ms.is_some() || view.playing);
    until(&engine, |v, _, _| v.playing && !v.before_play);
    engine.shutdown();
}

#[test]
fn the_room_waits_for_ren_and_carry_on_comes_at_twenty_seconds() {
    let (_root, engine) = staged(FfiRoomScene::WaitCap);
    // The members list comes in its own event: wait for both to say Ren is waited for.
    let (view, members, _) = until(&engine, |v, m, _| {
        v.can_carry_on && m.iter().any(|m| m.name == "Ren" && m.waited)
    });
    assert!(!view.playing);
    assert_eq!(view.waiting.len(), 1);
    assert_eq!(view.waiting[0].name, "Ren");
    assert!(members.iter().find(|m| m.name == "Ren").unwrap().waited);
    engine.send(Command::Room(RoomCommand::CarryOn));
    let (view, _, read) = until(&engine, |v, _, _| v.playing && v.carried_on.is_some());
    assert_eq!(view.carried_on.unwrap().past, ["Ren"]);
    assert!(read.iter().any(|e| matches!(e, Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::CarriedOn { .. }))));
    engine.shutdown();
}

#[test]
fn carried_on_shows_who_is_still_coming() {
    let (_root, engine) = staged(FfiRoomScene::CarriedOn);
    let (view, _, _) = until(&engine, |v, _, _| v.carried_on.is_some());
    assert!(view.playing);
    assert_eq!(view.carried_on.unwrap().by, "Mika");
    engine.shutdown();
}

#[test]
fn every_reason_on_ren_s_row() {
    for (scene, word) in [
        (FfiRoomScene::Choosing, MemberState::Choosing),
        (FfiRoomScene::NoSource, MemberState::NoSource),
        (FfiRoomScene::PlayerClosed, MemberState::PlayerClosed),
        (FfiRoomScene::Reconnecting, MemberState::Reconnecting),
        (FfiRoomScene::Behind, MemberState::Behind),
        (FfiRoomScene::OtherEpisode, MemberState::OtherEpisode),
        (FfiRoomScene::Left, MemberState::Left),
        (FfiRoomScene::BufferingSix, MemberState::Buffering),
    ] {
        let (_root, engine) = staged(scene);
        let (view, _, _) = until(&engine, move |v, m, _| {
            state(m, "Ren") == Some(word) && v.waiting.is_empty() && v.playing
        });
        assert!(view.playing, "{scene:?}: none of these pauses the room");
        engine.shutdown();
    }
}

#[test]
fn a_stall_in_a_small_room_waits() {
    let (_root, engine) = staged(FfiRoomScene::Buffering);
    let (view, _, _) = until(&engine, |v, m, _| {
        state(m, "Ren") == Some(MemberState::Buffering) && !v.waiting.is_empty()
    });
    assert!(!view.playing);
    engine.shutdown();
}

#[test]
fn your_own_states() {
    let (_root, engine) = staged(FfiRoomScene::YouLoading);
    let (view, _, _) = until(&engine, |v, _, _| v.can_dont_wait);
    assert!(view.waiting.iter().any(|w| w.you));
    engine.shutdown();
    let (_root, engine) = staged(FfiRoomScene::YouReconnecting);
    until(&engine, |v, _, _| matches!(v.connection, RoomConnection::Reconnecting { .. }));
    engine.shutdown();
    let (_root, engine) = staged(FfiRoomScene::RelayTrouble);
    until(&engine, |v, _, _| v.relays.iter().filter(|r| r.healthy).count() == 2);
    engine.shutdown();
}

#[test]
fn up_next_counts_for_everyone() {
    let (_root, engine) = staged(FfiRoomScene::UpNext);
    let (view, _, _) =
        until(&engine, |v, _, _| v.up_next.as_ref().is_some_and(|u| u.ends_at_ms.is_some()));
    assert_eq!(view.up_next.unwrap().episode.episode, "10");
    engine.shutdown();
    let (_root, engine) = staged(FfiRoomScene::UpNextWaiting);
    let (view, _, _) =
        until(&engine, |v, _, _| v.up_next.as_ref().is_some_and(|u| !u.waiting_for.is_empty()));
    assert_eq!(view.up_next.unwrap().waiting_for, ["Ren"]);
    engine.shutdown();
}

#[test]
fn move_to_a_new_room_offers_and_the_old_room_says_who_went() {
    let (_root, engine) = staged(FfiRoomScene::MoveOffer);
    let (_, _, read) =
        until(&engine, |_, _, r| r.iter().any(|e| matches!(e, Event::RoomMoveOffer(_))));
    let offer = read
        .iter()
        .find_map(|e| match e {
            Event::RoomMoveOffer(o) => Some(o.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(offer.by, "Mika");
    assert_eq!(offer.names, ["Ren"]);
    engine.send(Command::Room(RoomCommand::AnswerMove { offer: offer.id, go: true }));
    // One wait for both: the new room's RoomJoined can come in the same batch as the old
    // room's RoomLeft, and a second wait would never see it.
    let (view, _, _) = until(&engine, |v, _, r| {
        let left =
            r.iter().position(|e| matches!(e, Event::RoomLeft { why: RoomLeftWhy::Moved }));
        let joined = r.iter().rposition(|e| matches!(e, Event::RoomJoined(_)));
        matches!((left, joined), (Some(l), Some(j)) if j > l)
            && !v.looking
            && v.room_episode.is_some()
    });
    assert!(view.moved.is_none());
    engine.shutdown();

    let (_root, engine) = staged(FfiRoomScene::MovedAway);
    let (view, _, _) =
        until(&engine, |v, _, _| v.moved.as_ref().is_some_and(|m| !m.went.is_empty()));
    let moved = view.moved.unwrap();
    assert_eq!((moved.by.as_str(), moved.went.as_slice()), ("Mika", &["Ren".to_owned()][..]));
    engine.shutdown();
}

#[test]
fn an_invite_is_looked_into_before_joining() {
    let (_root, engine) = staged(FfiRoomScene::InvitedFull);
    let engine2 = Arc::clone(&engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        loop {
            let batch = futures::executor::block_on(engine2.next_events());
            if let Some(g) = batch.iter().find_map(|e| match e {
                Event::RoomGlimpse(g) => Some(g.clone()),
                _ => None,
            }) {
                let _ = tx.send(g);
                return;
            }
        }
    });
    let glimpse = rx.recv_timeout(PATIENCE).unwrap();
    assert!(glimpse.full);
    assert_eq!(glimpse.members.len(), 10);
    assert_eq!(glimpse.by.as_deref(), Some("Mika"));
    engine.shutdown();
}

#[test]
fn an_invite_nobody_answers_says_so() {
    let (_root, engine) = staged(FfiRoomScene::Unreachable);
    let engine2 = Arc::clone(&engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        loop {
            let batch = futures::executor::block_on(engine2.next_events());
            if batch
                .iter()
                .any(|e| matches!(e, Event::RoomLeft { why: RoomLeftWhy::Unreachable }))
            {
                let _ = tx.send(());
                return;
            }
        }
    });
    rx.recv_timeout(PATIENCE).unwrap();
    engine.shutdown();
}

/// The scenes of a room past its start: each must be past "Starting when Ren is ready", in the
/// state the board draws.
fn after_play(
    scene: FfiRoomScene,
    drawn: impl Fn(&RoomView, &[RoomMember]) -> bool + Send + 'static,
) {
    let (_root, engine) = staged(scene);
    let (view, members, _) = until(&engine, move |v, m, _| !v.before_play && drawn(v, m));
    assert!(!view.before_play, "{scene:?}: {view:?} {members:?}");
    engine.shutdown();
}

#[test]
fn a_paused_room_was_watching_before_mika_paused_it() {
    after_play(FfiRoomScene::Paused, |v, _| {
        !v.playing && v.paused_by.as_deref() == Some("Mika")
    });
}

#[test]
fn a_wait_after_a_seek_is_a_wait_not_the_start() {
    for scene in [FfiRoomScene::WaitLoading, FfiRoomScene::WaitCap] {
        after_play(scene, |v, _| !v.playing && v.waiting.iter().any(|w| w.name == "Ren"));
    }
    after_play(FfiRoomScene::CarriedOn, |v, _| v.playing && v.carried_on.is_some());
    after_play(FfiRoomScene::YouLoading, |v, _| v.waiting.iter().any(|w| w.you));
}

#[test]
fn a_stall_and_your_own_behind_happen_in_a_room_already_watching() {
    after_play(FfiRoomScene::Buffering, |v, m| {
        !v.playing && state(m, "Ren") == Some(MemberState::Buffering)
    });
    after_play(FfiRoomScene::YouBehind, |_, m| state(m, "Ada") == Some(MemberState::Behind));
}

#[test]
fn you_in_a_list_of_names_is_lowercase_mid_sentence() {
    let (_root, engine) = staged(FfiRoomScene::YouLoading);
    let (_, _, read) = until(&engine, |_, _, r| {
        r.iter().any(|e| {
            matches!(e, Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::WaitingFor { .. }))
        })
    });
    let line = read
        .iter()
        .find_map(|e| match e {
            Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::WaitingFor { .. }) => {
                Some(m.text.clone())
            }
            _ => None,
        })
        .unwrap();
    assert!(line.contains("you") && !line.contains("You"), "{line}");
    engine.shutdown();
}
