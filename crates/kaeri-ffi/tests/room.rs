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
    hosted::{MpvValue, PlayerOp, PlayerReport},
    room::{
        MemberState, OldInvite, RoomCommand, RoomConnection, RoomEpisode, RoomLeftWhy,
        RoomMember, RoomMessageKind, RoomStartKind, RoomView, WaitReason,
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
    until_from(engine, (None, Vec::new()), until)
}

/// [`until`], starting from the view and members an earlier wait saw: a state that doesn't
/// change sends no new event.
fn until_from(
    engine: &Arc<FfiEngine>,
    (view, members): (Option<RoomView>, Vec<RoomMember>),
    until: impl Fn(&RoomView, &[RoomMember], &[Event]) -> bool + Send + 'static,
) -> (RoomView, Vec<RoomMember>, Vec<Event>) {
    let engine = Arc::clone(engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut view, mut members, mut read) = (view, members, Vec::new());
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

#[test]
fn behind_counts_the_seconds_from_a_room_well_into_the_episode() {
    let (_root, engine) = staged(FfiRoomScene::Behind);
    let (_, members, _) =
        until(&engine, |_, m, _| state(m, "Ren") == Some(MemberState::Behind));
    let ren = members.iter().find(|m| m.name == "Ren").unwrap();
    let behind = ren.behind_s.unwrap_or_default();
    assert!((3.0..=5.0).contains(&behind), "{behind}");
    engine.shutdown();
}

#[test]
fn up_next_shows_the_next_episodes_art() {
    let (_root, engine) = staged(FfiRoomScene::UpNext);
    let (view, _, _) = until(&engine, |v, _, _| {
        v.up_next.as_ref().is_some_and(|u| u.episode.cover_url.is_some())
    });
    let next = view.up_next.unwrap().episode;
    assert_eq!(next.episode, "10");
    assert!(next.banner_url.is_some(), "{next:?}");
    engine.shutdown();
}

/// What the app does on RoomStarted: play the room's episode from this device's own sources.
fn play_like_the_app(engine: &Arc<FfiEngine>, read: &[Event]) {
    let (id, episode) = read
        .iter()
        .find_map(|e| match e {
            Event::RoomStarted { id, episode, .. } => Some((*id, episode.clone())),
            _ => None,
        })
        .expect("the room started here");
    engine.send(Command::Play { id, episode, from_start: false });
}

#[test]
fn an_episode_with_no_source_here_reads_no_source_in_the_room() {
    let (_root, engine) = staged(FfiRoomScene::YouNoSource);
    let (view, members, read) =
        until(&engine, |_, _, r| r.iter().any(|e| matches!(e, Event::RoomStarted { .. })));
    play_like_the_app(&engine, &read);
    let (_, members, read) = until_from(&engine, (Some(view), members), |_, m, r| {
        state(m, "Ada") == Some(MemberState::NoSource)
            && r.iter().any(|e| matches!(e, Event::RoomNoSource { .. }))
    });
    assert_eq!(state(&members, "Ada"), Some(MemberState::NoSource));
    assert!(
        !read.iter().any(|e| matches!(e, Event::Toast(_))),
        "the dialog says it, not a toast"
    );
    engine.shutdown();

    // With the picker open for it (Android's you-choosing), the failed play leaves you Choosing
    // and says no "No source" over the picker.
    let (_root, engine) = staged(FfiRoomScene::YouChoosing);
    let (view, members, read) =
        until(&engine, |_, _, r| r.iter().any(|e| matches!(e, Event::RoomStarted { .. })));
    play_like_the_app(&engine, &read);
    thread::sleep(Duration::from_millis(3_000));
    engine.send(Command::Room(RoomCommand::Say { text: "hi".into() }));
    let (_, members, read) = until_from(&engine, (Some(view), members), |_, m, r| {
        r.iter().any(|e| matches!(e, Event::RoomChat(_))) && state(m, "Ada").is_some()
    });
    assert_eq!(state(&members, "Ada"), Some(MemberState::Choosing));
    assert!(!read.iter().any(|e| matches!(e, Event::RoomNoSource { .. })), "{read:?}");
    engine.shutdown();
}

#[test]
fn closing_the_empty_player_or_the_picker_leaves_you_no_source_not_player_closed() {
    let (_root, engine) = staged(FfiRoomScene::YouNoSource);
    let (view, members, _) =
        until(&engine, |_, m, _| state(m, "Ada") == Some(MemberState::NoSource));
    // The app closes the player it opened for the failed play: still No source.
    engine.send(Command::Room(RoomCommand::ClosePlayer));
    // The picker opens over it, and closes without a pick: back to No source.
    engine.send(Command::Room(RoomCommand::ChoosingSource { open: true }));
    let (view, members, _) = until_from(&engine, (Some(view), members), |_, m, _| {
        state(m, "Ada") == Some(MemberState::Choosing)
    });
    engine.send(Command::Room(RoomCommand::ClosePlayer));
    engine.send(Command::Room(RoomCommand::ChoosingSource { open: false }));
    let (_, members, read) = until_from(&engine, (Some(view), members), |_, m, _| {
        state(m, "Ada") != Some(MemberState::Choosing)
    });
    assert_eq!(state(&members, "Ada"), Some(MemberState::NoSource), "{read:?}");
    engine.shutdown();

    // Choosing stays Choosing when the player closes under the picker.
    let (_root, engine) = staged(FfiRoomScene::YouChoosing);
    let (view, members, _) =
        until(&engine, |_, m, _| state(m, "Ada") == Some(MemberState::Choosing));
    engine.send(Command::Room(RoomCommand::ClosePlayer));
    engine.send(Command::Room(RoomCommand::Say { text: "hi".into() }));
    let (_, members, _) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| matches!(e, Event::RoomChat(_)))
    });
    assert_eq!(state(&members, "Ada"), Some(MemberState::Choosing));
    engine.shutdown();
}

#[test]
fn paused_waiting_and_before_play_scenes_open_the_player() {
    // Android's fixture played from 0:00 while the room was at 0:12: each of these scenes must
    // tell the app to open the player, and a paused or waiting room says where.
    for scene in [
        FfiRoomScene::Paused,
        FfiRoomScene::WaitLoading,
        FfiRoomScene::WaitCap,
        FfiRoomScene::BeforePlay,
        FfiRoomScene::BeforePlayWait,
    ] {
        let (_root, engine) = staged(scene);
        let (view, _, read) =
            until(&engine, |_, _, r| r.iter().any(|e| matches!(e, Event::RoomStarted { .. })));
        let at = read.iter().find_map(|e| match e {
            Event::RoomStarted { at, .. } => Some(*at),
            _ => None,
        });
        if view.before_play {
            assert!(at.is_some(), "{scene:?}: {read:?}");
        } else {
            assert!(matches!(at, Some(Some(pos)) if pos > 0.5), "{scene:?}: {at:?}");
        }
        engine.shutdown();
    }
}

#[test]
fn play_pressed_before_the_room_ever_played_is_no_resume() {
    // Windows' before-cap frame said "Mika resumed": a room that hasn't started can't resume.
    let (_root, engine) = staged(FfiRoomScene::BeforePlayCap);
    let (view, _, read) = until(&engine, |v, _, _| v.before_play && !v.waiting.is_empty());
    let resumed: Vec<_> = read
        .iter()
        .filter(|e| {
            matches!(e, Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::Resumed { .. }))
        })
        .collect();
    assert!(resumed.is_empty(), "{resumed:?} {view:?}");
    engine.shutdown();
}

#[test]
fn a_room_started_from_a_playing_player_keeps_playing_and_says_no_resume() {
    // The morph: the player becomes the room. It must never be paused for the room's start,
    // and the room's start is no one's Resume.
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = engine(&root);
    let room = Arc::clone(&engine).start_fake_room();
    let id = kaeri_core::ids::AnilistId::new(900_001);
    engine.send(Command::Play { id, episode: "9".into(), from_start: false });
    let read = before_the_room(&engine, |e| loads(e).is_some());
    let playback = read.iter().find_map(loads).unwrap();
    let report = {
        let engine = Arc::clone(&engine);
        move |report| assert_eq!(engine.send(Command::PlayerReport { playback, report }), None)
    };
    let property = move |name: &str, json: String| {
        report(PlayerReport::Property { name: name.into(), json })
    };
    assert_eq!(
        engine.send(Command::PlayerReport { playback, report: PlayerReport::FileLoaded }),
        None
    );
    property("duration", "1440.0".into());
    property("pause", "false".into());
    let ticking = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let ticker = {
        let ticking = Arc::clone(&ticking);
        thread::spawn(move || {
            let mut pos = 30.0;
            while ticking.load(std::sync::atomic::Ordering::Relaxed) {
                property("time-pos", format!("{pos:.2}"));
                pos += 0.25;
                thread::sleep(Duration::from_millis(250));
            }
        })
    };
    thread::sleep(Duration::from_millis(600));
    engine.send(Command::Room(RoomCommand::Lobby {
        invite: None,
        name: Some("Ada".into()),
        pick: Some(RoomEpisode { id, episode: "9".into() }),
        anyway: false,
    }));
    let (view, members, mut read) = until(&engine, |v, _, _| v.room_episode.is_some());
    thread::sleep(Duration::from_secs(3));
    engine.send(Command::Room(RoomCommand::Say { text: "hi".into() }));
    let (view, _, later) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| matches!(e, Event::RoomChat(_)))
    });
    read.extend(later);
    ticking.store(false, std::sync::atomic::Ordering::Relaxed);
    ticker.join().unwrap();
    assert!(view.playing, "{view:?}");
    let said: Vec<_> = read
        .iter()
        .filter(|e| {
            matches!(e, Event::RoomMessage(m) if matches!(
                m.kind,
                RoomMessageKind::Resumed { .. } | RoomMessageKind::Paused { .. }
            ))
        })
        .collect();
    assert!(said.is_empty(), "{said:?}");
    let paused = read.iter().any(|e| {
        matches!(e, Event::PlayerOps { ops, .. } if ops.iter().any(|op| matches!(
            op,
            PlayerOp::Set { property, value: MpvValue::Flag(true) } if property == "pause"
        )))
    });
    assert!(!paused, "the player was paused for the room's start");
    std::mem::forget(room);
    engine.shutdown();
}

/// Events until one holds, before any room: [`until`] waits for a room's view.
fn before_the_room(
    engine: &Arc<FfiEngine>,
    want: impl Fn(&Event) -> bool + Send + 'static,
) -> Vec<Event> {
    let engine = Arc::clone(engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut read = Vec::new();
        loop {
            let batch = futures::executor::block_on(engine.next_events());
            if batch.is_empty() {
                return;
            }
            let found = batch.iter().any(&want);
            read.extend(batch);
            if found {
                let _ = tx.send(read);
                return;
            }
        }
    });
    rx.recv_timeout(PATIENCE).expect("the event in time")
}

fn loads(event: &Event) -> Option<kaeri_model::playback::PlaybackId> {
    match event {
        Event::PlayerOps { playback, ops }
            if ops.iter().any(|op| matches!(op, PlayerOp::Load { .. })) =>
        {
            Some(*playback)
        }
        _ => None,
    }
}

#[test]
fn a_player_reporting_the_rooms_places_late_or_stale_never_seeks() {
    // Seen on a Windows VM, where mpv's frame freezes after a resize: the player opens at the
    // room's place, reports it late and again long after, and the chat said "You jumped from
    // 0:12 to 0:12" or "from 0:35 to 0:12". Neither is a seek.
    for scene in [FfiRoomScene::WaitLoading, FfiRoomScene::Watching] {
        let (_root, engine) = staged(scene);
        let (view, members, read) = until(&engine, |_, _, r| {
            r.iter().any(|e| matches!(e, Event::RoomStarted { at: Some(_), .. }))
        });
        let at = read
            .iter()
            .find_map(|e| match e {
                Event::RoomStarted { at, .. } => *at,
                _ => None,
            })
            .unwrap();
        play_like_the_app(&engine, &read);
        let (view, members, read) = until_from(&engine, (Some(view), members), |_, _, r| {
            r.iter().any(|e| loads(e).is_some())
        });
        let playback = read.iter().find_map(loads).unwrap();
        let property = |name: &str, json: String| {
            let report = PlayerReport::Property { name: name.into(), json };
            assert_eq!(engine.send(Command::PlayerReport { playback, report }), None);
        };
        assert_eq!(
            engine.send(Command::PlayerReport { playback, report: PlayerReport::FileLoaded }),
            None
        );
        property("duration", "1440.0".into());
        let paused = !view.playing;
        property("pause", paused.to_string());
        // The file's own start, then the place it was opened at.
        property("time-pos", "0.0".into());
        thread::sleep(Duration::from_millis(300));
        property("time-pos", format!("{at:.2}"));
        // Where the room is now, a while (past the cue's window), then the frozen frame's old
        // place, then the right one again.
        let started = std::time::Instant::now();
        let room_now = |view: &RoomView| {
            let moved = if view.playing { started.elapsed().as_secs_f64() + 0.3 } else { 0.0 };
            view.position.unwrap_or(at) + moved
        };
        for _ in 0..24 {
            thread::sleep(Duration::from_millis(250));
            property("time-pos", format!("{:.2}", room_now(&view)));
        }
        property("time-pos", format!("{at:.2}"));
        thread::sleep(Duration::from_millis(300));
        property("time-pos", format!("{:.2}", room_now(&view)));
        thread::sleep(Duration::from_millis(500));
        engine.send(Command::Room(RoomCommand::Say { text: "hi".into() }));
        let (_, _, later) = until_from(&engine, (Some(view), members), |_, _, r| {
            r.iter().any(|e| matches!(e, Event::RoomChat(_)))
        });
        let seeks: Vec<_> = read
            .iter()
            .chain(&later)
            .filter(|e| {
                matches!(e, Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::Seeked { .. }))
            })
            .collect();
        assert!(seeks.is_empty(), "{scene:?}: {seeks:?}");
        engine.shutdown();
    }
}

#[test]
fn your_own_loading_scene_holds_against_your_players_reports() {
    // Apple's ownwait: the app's real player opened and reported Watching, or followed Mika's
    // seek, and the scene showed "Watching together" or your row Seeking. The scene's pose holds.
    let (_root, engine) = staged(FfiRoomScene::YouLoading);
    let (view, members, read) =
        until(&engine, |_, _, r| r.iter().any(|e| matches!(e, Event::RoomStarted { .. })));
    play_like_the_app(&engine, &read);
    let (view, members, read) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| loads(e).is_some())
    });
    let playback = read.iter().find_map(loads).unwrap();
    let property = |name: &str, json: String| {
        let report = PlayerReport::Property { name: name.into(), json };
        assert_eq!(engine.send(Command::PlayerReport { playback, report }), None);
    };
    assert_eq!(
        engine.send(Command::PlayerReport { playback, report: PlayerReport::FileLoaded }),
        None
    );
    property("duration", "1440.0".into());
    property("pause", "true".into());
    let at = view.position.unwrap_or(0.0);
    for _ in 0..8 {
        property("time-pos", format!("{at:.2}"));
        thread::sleep(Duration::from_millis(250));
    }
    engine.send(Command::Room(RoomCommand::Say { text: "hi".into() }));
    let (view, members, _) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| matches!(e, Event::RoomChat(_)))
    });
    let you = view.waiting.iter().find(|w| w.you);
    assert!(you.is_some_and(|w| w.reason == WaitReason::Loading), "{view:?}");
    assert_eq!(state(&members, "Ada"), Some(MemberState::Loading), "{members:?}");
    engine.shutdown();
}

#[test]
fn your_held_behind_scene_keeps_its_posed_amount() {
    // Linux's self-behind read "You're 15 s behind" and "Behind · 15 s" where the pose says 4 s:
    // the amount came from the app's just-opened player and the room playing on, not the pose.
    let (_root, engine) = staged(FfiRoomScene::YouBehind);
    let (view, members, read) =
        until(&engine, |_, _, r| r.iter().any(|e| matches!(e, Event::RoomStarted { .. })));
    play_like_the_app(&engine, &read);
    let (view, members, read) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| loads(e).is_some())
    });
    let playback = read.iter().find_map(loads).unwrap();
    let property = |name: &str, json: String| {
        let report = PlayerReport::Property { name: name.into(), json };
        assert_eq!(engine.send(Command::PlayerReport { playback, report }), None);
    };
    assert_eq!(
        engine.send(Command::PlayerReport { playback, report: PlayerReport::FileLoaded }),
        None
    );
    property("duration", "1440.0".into());
    property("pause", "false".into());
    // Android: a player that really plays, changes state and closes its picker as it opens.
    engine.send(Command::Room(RoomCommand::ChoosingSource { open: false }));
    engine.send(Command::Room(RoomCommand::OpenPlayer));
    for i in 0..24 {
        property("time-pos", format!("{:.2}", i as f64 * 0.25));
        match i {
            6 => property("pause", "true".into()),
            8 => property("pause", "false".into()),
            12 => property("seeking", "true".into()),
            13 => property("seeking", "false".into()),
            16 => property("paused-for-cache", "true".into()),
            17 => property("paused-for-cache", "false".into()),
            _ => {}
        }
        thread::sleep(Duration::from_millis(250));
    }
    engine.send(Command::Room(RoomCommand::Say { text: "hi".into() }));
    let (_, members, _) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| matches!(e, Event::RoomChat(_)))
    });
    let you = members.iter().find(|m| m.you).unwrap();
    assert_eq!(you.state, MemberState::Behind, "{members:?}");
    assert_eq!(you.behind_s, Some(4.0), "{members:?}");
    engine.shutdown();
}

#[test]
fn before_play_the_chat_says_no_wait() {
    // Apple's beforewait: "Paused for Ren · loading" before the room ever played. The board
    // has the header say it and no line in the chat.
    let (_root, engine) = staged(FfiRoomScene::BeforePlayWait);
    let (view, members, read) =
        until(&engine, |v, _, _| v.before_play && v.waiting.iter().any(|w| w.name == "Ren"));
    thread::sleep(Duration::from_millis(1_500));
    engine.send(Command::Room(RoomCommand::Say { text: "hi".into() }));
    let (_, _, later) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| matches!(e, Event::RoomChat(_)))
    });
    let waits: Vec<_> = read
        .iter()
        .chain(&later)
        .filter(|e| {
            matches!(e, Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::WaitingFor { .. }))
        })
        .collect();
    assert!(waits.is_empty(), "{waits:?}");
    engine.shutdown();
}

#[test]
fn a_waiters_new_start_reaches_the_line_and_the_row_together() {
    // Apple's iPad beforecap read "waited 22 s" over Ren's "Loading · 23 s". When a waiter's
    // start moves, the view and the rows are told it again, with the same start.
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = engine(&root);
    let room = Arc::clone(&engine).start_fake_room();
    assert!(room.stage(FfiRoomScene::BeforePlayCap));
    let (view, members, _) =
        until(&engine, |v, _, _| v.waiting.iter().any(|w| w.name == "Ren"));
    let before = view.waiting.iter().find(|w| w.name == "Ren").unwrap().since_ms;
    room.member_loads("Ren".into(), 30.0);
    let (view, members, _) = until_from(&engine, (Some(view), members), move |v, m, _| {
        let line = v.waiting.iter().find(|w| w.name == "Ren").map(|w| w.since_ms);
        let row = m.iter().find(|m| m.name == "Ren").and_then(|m| m.since_ms);
        line.is_some_and(|l| before - l > 5_000) && row == line
    });
    let line = view.waiting.iter().find(|w| w.name == "Ren").unwrap().since_ms;
    let row = members.iter().find(|m| m.name == "Ren").unwrap().since_ms;
    assert_eq!(Some(line), row);
    assert_eq!(view.carry_on_at_ms, Some(line + 20_000));
    std::mem::forget(room);
    engine.shutdown();
}

#[test]
fn a_slow_decoders_bursty_positions_are_never_your_seek_or_pause() {
    // Android at ca1fa1638, a phone emulator decoding in software: time-pos came in bursts and
    // climbed to the room's place in steps, and each was "You jumped"; the watching scene also
    // got "You paused" and "You resumed". Only the player's own seek or pause is the viewer's.
    let (_root, engine) = staged(FfiRoomScene::Watching);
    let (view, members, read) =
        until(&engine, |_, _, r| r.iter().any(|e| matches!(e, Event::RoomStarted { .. })));
    play_like_the_app(&engine, &read);
    let (view, members, read) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| loads(e).is_some())
    });
    let playback = read.iter().find_map(loads).unwrap();
    let property = |name: &str, json: String| {
        let report = PlayerReport::Property { name: name.into(), json };
        assert_eq!(engine.send(Command::PlayerReport { playback, report }), None);
    };
    assert_eq!(
        engine.send(Command::PlayerReport { playback, report: PlayerReport::FileLoaded }),
        None
    );
    property("duration", "1440.0".into());
    property("pause", "false".into());
    // Bursts: nothing for a while, then seconds at once, climbing in steps; the room's cues
    // land late and the stale pause state with them.
    for (pos, wait_ms) in [
        (0.0, 300),
        (2.0, 1_800),
        (3.0, 200),
        (7.0, 2_400),
        (10.0, 200),
        (19.0, 2_800),
        (23.0, 300),
        (26.0, 3_000),
        (29.0, 300),
        (31.0, 2_000),
    ] {
        property("time-pos", format!("{pos:.2}"));
        thread::sleep(Duration::from_millis(wait_ms));
    }
    engine.send(Command::Room(RoomCommand::Say { text: "hi".into() }));
    let (view, members, later) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| matches!(e, Event::RoomChat(_)))
    });
    let yours = |events: &mut dyn Iterator<Item = &Event>| -> Vec<RoomMessageKind> {
        events
            .filter_map(|e| match e {
                Event::RoomMessage(m) => Some(m.kind.clone()),
                _ => None,
            })
            .filter(|k| {
                matches!(
                    k,
                    RoomMessageKind::Seeked { .. }
                        | RoomMessageKind::Paused { .. }
                        | RoomMessageKind::Resumed { .. }
                )
            })
            .collect()
    };
    let said = yours(&mut read.iter().chain(&later));
    assert!(said.is_empty(), "{said:?}");
    // The room's catch-up seek lands as mpv says it (`seeking`); then the viewer's own seek,
    // which the player says the same way, is still the room's.
    property("seeking", "true".into());
    property("seeking", "false".into());
    thread::sleep(Duration::from_millis(300));
    property("seeking", "true".into());
    property("time-pos", "600.00".into());
    property("seeking", "false".into());
    property("time-pos", "600.30".into());
    let (_, _, read) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| {
            matches!(e, Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::Seeked { .. }))
        })
    });
    assert!(
        yours(&mut read.iter())
            .iter()
            .any(|k| matches!(k, RoomMessageKind::Seeked { to, .. } if *to > 590.0)),
        "{read:?}"
    );
    engine.shutdown();
}

#[test]
fn a_player_opening_mid_wait_never_seeks_for_you() {
    // Android's wait-loading: "You jumped from 0:35 to 0:04" between two wait lines. The player
    // opened near 0:04 (where the room started) and the room's cue put it at 0:35; its opening
    // seeks, before or just after the cue, are never yours.
    let (_root, engine) = staged(FfiRoomScene::WaitLoading);
    let (view, members, read) =
        until(&engine, |_, _, r| r.iter().any(|e| matches!(e, Event::RoomStarted { .. })));
    play_like_the_app(&engine, &read);
    let (view, members, read) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| loads(e).is_some())
    });
    let playback = read.iter().find_map(loads).unwrap();
    let property = |name: &str, json: String| {
        let report = PlayerReport::Property { name: name.into(), json };
        assert_eq!(engine.send(Command::PlayerReport { playback, report }), None);
    };
    assert_eq!(
        engine.send(Command::PlayerReport { playback, report: PlayerReport::FileLoaded }),
        None
    );
    property("duration", "1440.0".into());
    property("pause", "true".into());
    // Its own start: a seek to near 0:04 before the room's cue.
    property("seeking", "true".into());
    property("time-pos", "4.20".into());
    property("seeking", "false".into());
    for _ in 0..3 {
        property("time-pos", "4.20".into());
        thread::sleep(Duration::from_millis(250));
    }
    // The room's cue lands at 0:35, then a late seek back to the start it opened with.
    property("seeking", "true".into());
    property("time-pos", "35.00".into());
    property("seeking", "false".into());
    thread::sleep(Duration::from_millis(400));
    property("seeking", "true".into());
    property("time-pos", "4.20".into());
    property("seeking", "false".into());
    for _ in 0..8 {
        property("time-pos", "4.20".into());
        thread::sleep(Duration::from_millis(250));
    }
    engine.send(Command::Room(RoomCommand::Say { text: "hi".into() }));
    let (_, _, later) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| matches!(e, Event::RoomChat(_)))
    });
    let seeks: Vec<_> = read
        .iter()
        .chain(&later)
        .filter(|e| {
            matches!(e, Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::Seeked { to, .. } if to < 10.0))
        })
        .collect();
    assert!(seeks.is_empty(), "{seeks:?}");
    engine.shutdown();
}

#[test]
fn the_wait_line_and_the_row_count_from_one_start() {
    let (_root, engine) = staged(FfiRoomScene::WaitLoading);
    let (view, members, _) = until(&engine, |v, m, _| {
        v.waiting.iter().any(|w| w.name == "Ren")
            && state(m, "Ren") == Some(MemberState::Loading)
    });
    let wait = view.waiting.iter().find(|w| w.name == "Ren").unwrap();
    let row = members.iter().find(|m| m.name == "Ren").unwrap();
    assert_eq!(Some(wait.since_ms), row.since_ms);
    assert_eq!(view.waiting[0].since_ms, wait.since_ms, "Ren is the first waited");
    engine.shutdown();
}

#[test]
fn carry_on_never_carries_on_without_yourself() {
    // The room waits for you alone: Carry on is your own Don't wait, and names nobody.
    let (_root, engine) = staged(FfiRoomScene::YouLoading);
    until(&engine, |v, _, _| v.waiting.iter().any(|w| w.you));
    engine.send(Command::Room(RoomCommand::CarryOn));
    let (_, _, read) = until(&engine, |v, _, _| !v.waiting.iter().any(|w| w.you));
    assert!(
        !read.iter().any(|e| matches!(e, Event::RoomMessage(m) if matches!(m.kind, RoomMessageKind::CarriedOn { .. }))),
        "{read:?}"
    );
    engine.shutdown();
}

#[test]
fn your_player_never_rolls_on_alone_and_the_rooms_up_next_moves_everyone() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = engine(&root);
    let room = Arc::clone(&engine).start_fake_room();
    assert!(room.stage(FfiRoomScene::Watching));
    let (view, members, _) = until(&engine, |v, _, _| v.playing && v.room_episode.is_some());
    let id = view.room_episode.clone().unwrap().id;
    // The player reached its end and asks for the next episode, as an app's own Up next would.
    engine.send(Command::Play { id, episode: "10".into(), from_start: false });
    let (view, members, read) = until_from(&engine, (Some(view), members), |v, _, _| {
        v.up_next.as_ref().is_some_and(|u| !u.waiting_for.is_empty())
    });
    assert!(
        !read
            .iter()
            .any(|e| matches!(e, Event::RoomStarted { episode, .. } if episode == "10")),
        "{read:?}"
    );
    assert_eq!(view.room_episode.as_ref().unwrap().episode, "9");
    // Once everyone's file has reached its own Up next point, the room counts and moves on.
    room.member_reaches_up_next("Mika".into());
    room.member_reaches_up_next("Ren".into());
    let (view, members, _) = until_from(&engine, (Some(view), members), |v, _, _| {
        v.up_next.as_ref().is_some_and(|u| u.ends_at_ms.is_some())
    });
    let (view, _, read) = until_from(&engine, (Some(view), members), |v, _, r| {
        v.room_episode.as_ref().is_some_and(|e| e.episode == "10")
            && r.iter()
                .any(|e| matches!(e, Event::RoomStarted { episode, .. } if episode == "10"))
    });
    assert!(view.up_next.is_none(), "{view:?}");
    assert!(
        read.iter()
            .any(|e| matches!(e, Event::RoomStarted { kind: RoomStartKind::MoveOn, .. }))
    );
    std::mem::forget(room);
    engine.shutdown();
}

#[test]
fn a_room_is_kept_and_rejoin_comes_back_as_the_same_person() {
    let (_root, engine) = staged(FfiRoomScene::Watching);
    let (view, members, _) = until(&engine, |v, m, _| {
        v.room_episode.as_ref().is_some_and(|e| !e.title.is_empty()) && m.iter().any(|m| m.you)
    });
    let me = members.iter().find(|m| m.you).unwrap().id.clone();
    engine.send(Command::Room(RoomCommand::Leave));
    engine.send(Command::Room(RoomCommand::LoadRecent));
    let room_id = view.room_id.clone();
    let (_, _, read) = until_from(&engine, (Some(view), members), move |_, _, r| {
        r.iter().any(|e| matches!(e, Event::RoomRecent(rooms) if rooms.iter().any(|x| x.room_id == room_id)))
    });
    let kept = read
        .iter()
        .find_map(|e| match e {
            Event::RoomRecent(rooms) => rooms.first().cloned(),
            _ => None,
        })
        .unwrap();
    assert_eq!(kept.title, "Stardust Wayfarer");
    assert_eq!(kept.names, ["Mika", "Ren"]);
    engine.send(Command::Room(RoomCommand::Rejoin { room_id: kept.room_id.clone() }));
    let (_, members, _) = until(&engine, |_, m, r| {
        r.iter().any(|e| matches!(e, Event::RoomJoined(_))) && m.iter().any(|m| m.you)
    });
    assert_eq!(members.iter().find(|m| m.you).unwrap().id, me, "the same identity key");
    engine.send(Command::Room(RoomCommand::ForgetRoom { room_id: kept.room_id }));
    engine.shutdown();
}

#[test]
fn the_background_says_player_closed_once_and_the_foreground_comes_back() {
    use kaeri_model::command::AppState;
    let (_root, engine) = staged(FfiRoomScene::Watching);
    let (view, members, _) =
        until(&engine, |v, m, _| v.playing && state(m, "Ada") == Some(MemberState::Watching));
    engine.send(Command::AppPhase { phase: AppState::Background { playing: false } });
    let (view, members, read) = until_from(&engine, (Some(view), members), |_, m, r| {
        r.iter().any(|e| matches!(e, Event::RoomFlushed))
            && m.iter().any(|m| m.you && m.state == MemberState::PlayerClosed && m.background)
    });
    assert!(read.iter().any(|e| matches!(e, Event::RoomFlushed)));
    engine.send(Command::AppPhase { phase: AppState::Foreground });
    let (_, members, _) = until_from(&engine, (Some(view), members), |_, m, _| {
        m.iter().any(|m| m.you && !m.background && m.state != MemberState::PlayerClosed)
    });
    assert_ne!(state(&members, "Ada"), Some(MemberState::PlayerClosed));
    engine.shutdown();
}

#[test]
fn a_look_that_fails_never_leaves_the_room_youre_in() {
    // Linux: a failed PeekInvite sent RoomLeft(Unreachable), and the apps tore down the room
    // this device was in. A look fails as a look.
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = engine(&root);
    let room = Arc::clone(&engine).start_fake_room();
    assert!(room.stage(FfiRoomScene::Watching));
    let (view, members, _) = until(&engine, |v, _, _| v.link.is_some());
    let link = view.link.clone().unwrap();

    // Text that is no invite.
    engine.send(Command::Room(RoomCommand::PeekInvite { invite: "hello".into() }));
    let (view, members, read) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| matches!(e, Event::RoomLookFailed { .. }))
    });
    let failed = read.iter().find_map(|e| match e {
        Event::RoomLookFailed { invite, why, .. } => Some((invite.clone(), why.clone())),
        _ => None,
    });
    assert!(
        matches!(&failed, Some((Some(i), Some(RoomLeftWhy::Failed { .. }))) if i == "hello"),
        "{failed:?}"
    );

    // An invite whose relays don't answer.
    room.unreachable(true);
    engine.send(Command::Room(RoomCommand::PeekInvite { invite: link.clone() }));
    let (_, _, later) = until_from(&engine, (Some(view), members), |_, _, r| {
        r.iter().any(|e| matches!(e, Event::RoomLookFailed { .. }))
    });
    let failed = later.iter().find_map(|e| match e {
        Event::RoomLookFailed { invite, why, room_id, .. } => {
            Some((invite.clone(), why.clone(), room_id.clone()))
        }
        _ => None,
    });
    assert!(
        matches!(&failed, Some((Some(i), Some(RoomLeftWhy::Unreachable), id)) if *i == link && !id.is_empty()),
        "{failed:?}"
    );
    let left: Vec<_> =
        read.iter().chain(&later).filter(|e| matches!(e, Event::RoomLeft { .. })).collect();
    assert!(left.is_empty(), "{left:?}");
    engine.shutdown();
}

#[test]
fn a_glimpse_names_the_invite_it_answers() {
    // Android's review: a slow answer for invite A must not stand in for invite B's, so the
    // glimpse carries the invite it was asked for, as the look's failure does.
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = engine(&root);
    let room = Arc::clone(&engine).start_fake_room();
    let invite = room.invite("Mika".into());
    engine.send(Command::Room(RoomCommand::PeekInvite { invite: invite.clone() }));
    // No room joined here: no view to wait from.
    let (_, _, read) =
        until_from(&engine, (Some(RoomView::default()), Vec::new()), |_, _, r| {
            r.iter().any(|e| matches!(e, Event::RoomGlimpse(_)))
        });
    let glimpse = read.iter().find_map(|e| match e {
        Event::RoomGlimpse(g) => Some(g.clone()),
        _ => None,
    });
    assert_eq!(glimpse.and_then(|g| g.invite), Some(invite));
    engine.shutdown();
}

#[test]
fn settings_edit_the_spare_relays_and_show_made_up_hosts_in_testing_builds() {
    // Apple's Settings ▸ Relays: the board draws × on the spares, and shots never show real
    // relay hosts.
    let root = tempfile::tempdir().expect("a temporary directory");
    let engine = engine(&root);
    let relays = |engine: &Arc<FfiEngine>, command: RoomCommand| {
        engine.send(Command::Room(command));
        let (_, _, read) =
            until_from(engine, (Some(RoomView::default()), Vec::new()), |_, _, r| {
                r.iter().any(|e| matches!(e, Event::RoomRelays(_)))
            });
        read.into_iter()
            .find_map(|e| match e {
                Event::RoomRelays(r) => Some(r),
                _ => None,
            })
            .unwrap()
    };
    let shown = relays(&engine, RoomCommand::LoadRelays);
    let all = shown.defaults.iter().chain(&shown.spares).chain(&shown.default_spares);
    assert!(all.clone().all(|r| r.ends_with(".example")), "{shown:?}");
    assert_eq!(shown.spares, shown.default_spares);
    assert_eq!(shown.spares.len(), 3);

    // × on the first spare: the rest stay, in order.
    let rest = shown.spares[1..].to_vec();
    let edited = relays(&engine, RoomCommand::SetSpareRelays { relays: rest.clone() });
    assert_eq!(edited.spares, rest);
    let file =
        std::fs::read_to_string(root.path().join("config/config.toml")).unwrap_or_default();
    assert!(
        file.contains("spare_relays") && file.contains(&rest[0]),
        "kept in the file: {file}"
    );
    // None left, then the built-in ones put back.
    let none = relays(&engine, RoomCommand::SetSpareRelays { relays: Vec::new() });
    assert!(none.spares.is_empty(), "{none:?}");
    let back =
        relays(&engine, RoomCommand::SetSpareRelays { relays: none.default_spares.clone() });
    assert_eq!(back.spares, back.default_spares);
    engine.shutdown();
}
