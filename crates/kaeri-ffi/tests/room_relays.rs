//! Two engines in one room over the real transport: Nostr relays on loopback (never a public
//! relay), every event signed, sealed, proved and checked as on a public relay. Its own test
//! binary: the in-process fake room of `tests/room.rs` would stand in for the relays.

#![cfg(all(feature = "testing", feature = "room"))]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use kaeri_core::ids::AnilistId;
use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiStart};
use kaeri_model::{
    command::Command,
    event::Event,
    room::{RoomChatBody, RoomCommand, RoomEpisode, RoomView},
};
use kaeri_room::fake_relay::FakeRelay;

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

fn engine(root: &tempfile::TempDir) -> Arc<FfiEngine> {
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    FfiEngine::start_testing(dirs, Arc::new(Quiet), FfiStart::default())
        .expect("the engine starts")
}

/// Read `engine`'s events until `done` holds on one, keeping the last room view.
fn until(
    engine: &Arc<FfiEngine>,
    done: impl Fn(&Event, Option<&RoomView>) -> bool + Send + 'static,
) -> (Option<RoomView>, Vec<Event>) {
    let engine = Arc::clone(engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut view, mut read) = (None, Vec::new());
        let started = Instant::now();
        while started.elapsed() < PATIENCE {
            let batch = futures::executor::block_on(engine.next_events());
            if batch.is_empty() {
                break;
            }
            for event in batch {
                if let Event::RoomJoined(v) | Event::RoomState(v) = &event {
                    view = Some(v.clone());
                }
                if std::env::var_os("ROOM_TRACE").is_some() {
                    match &event {
                        Event::RoomMembers(m) => eprintln!(
                            "EV members {:?}",
                            m.iter()
                                .map(|m| (m.name.as_str(), m.you, m.state))
                                .collect::<Vec<_>>()
                        ),
                        Event::RoomJoined(v) | Event::RoomState(v) => eprintln!(
                            "EV view {} {:?} playing={} by={:?} ep={:?}",
                            v.name,
                            v.connection,
                            v.playing,
                            v.paused_by,
                            v.room_episode.as_ref().map(|e| e.episode.clone())
                        ),
                        Event::RoomChat(_) | Event::RoomMessage(_) => {
                            eprintln!("EV chat/message")
                        }
                        _ => {}
                    }
                }
                let hit = done(&event, view.as_ref());
                read.push(event);
                if hit {
                    let _ = tx.send((view, read));
                    return;
                }
            }
        }
    });
    rx.recv_timeout(PATIENCE).expect("in time")
}

#[test]
fn two_engines_meet_in_a_room_over_loopback_relays() {
    // Two relays on loopback, kept going on a runtime of the test's own.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let relays: Vec<FakeRelay> =
        runtime.block_on(async { vec![FakeRelay::start().await, FakeRelay::start().await] });
    let urls: Vec<String> = relays.iter().map(FakeRelay::url).collect();

    let (root_a, root_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (ada, ren) = (engine(&root_a), engine(&root_b));
    let pick = RoomEpisode { id: AnilistId::new(900_001), episode: "9".into() };

    // Ada starts a room on Settings' relays.
    ada.send(Command::Room(RoomCommand::SetRelays { relays: urls.clone() }));
    ada.send(Command::Room(RoomCommand::Lobby {
        invite: None,
        name: Some("Ada".into()),
        pick: Some(pick),
        anyway: false,
    }));
    let (view, _) = until(&ada, |e, v| {
        matches!(e, Event::RoomJoined(_) | Event::RoomState(_))
            && v.is_some_and(|v| v.link.is_some() && v.room_episode.is_some())
    });
    let link = view.unwrap().link.unwrap();
    assert!(link.starts_with("https://kaeri.app/room#v=3&k="), "{link}");
    assert!(link.contains("&rl="), "a custom relay list travels with the invite: {link}");

    // Ren joins by the link and sees Ada; Ada sees Ren.
    ren.send(Command::Room(RoomCommand::Lobby {
        invite: Some(link),
        name: Some("Ren".into()),
        pick: None,
        anyway: false,
    }));
    until(&ren, |e, _| matches!(e, Event::RoomMembers(m) if m.iter().any(|m| m.name == "Ada")));
    until(&ada, |e, _| matches!(e, Event::RoomMembers(m) if m.iter().any(|m| m.name == "Ren")));

    // Chat goes across, sealed on the way.
    ren.send(Command::Room(RoomCommand::Say { text: "the lantern scene".into() }));
    until(
        &ada,
        |e, _| matches!(e, Event::RoomChat(c) if c.lines.iter().any(|l| matches!(&l.body, RoomChatBody::Said { text } if text == "the lantern scene") && l.from == "Ren")),
    );

    // Ren has no player here and would be waited for: Don't wait for me. Then Play for
    // everyone from Ada, and Ren's room plays.
    ren.send(Command::Room(RoomCommand::DontWait));
    ada.send(Command::Room(RoomCommand::Play { play: true }));
    until(&ren, |_, v| v.is_some_and(|v| v.playing));
    // Ren pauses; Ada's room says Ren paused it.
    ren.send(Command::Room(RoomCommand::Play { play: false }));
    until(&ada, |_, v| v.is_some_and(|v| !v.playing && v.paused_by.as_deref() == Some("Ren")));

    // What the relays saw: only the room's kind, its tag and an expiry, and a sealed body
    // that is base64 alone (so no name, title or line of chat can be read in it). The short
    // words are looked for outside the sealed body: random base64 can spell them by chance.
    for relay in &relays {
        let events = relay.events();
        assert!(!events.is_empty());
        for event in events {
            assert_eq!(event.kind.as_u16(), kaeri_room::envelope::KIND);
            assert!(
                event.content.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b)),
                "{}",
                event.content
            );
            let mut open = serde_json::to_value(&event).unwrap();
            open["content"] = serde_json::Value::Null;
            let json = open.to_string();
            for word in ["Ada", "Ren", "lantern", "Stardust", "900001"] {
                assert!(!json.contains(word), "{word} in {json}");
            }
            let tags: Vec<&str> = event
                .tags
                .iter()
                .filter_map(|t| t.as_slice().first().map(String::as_str))
                .collect();
            assert_eq!(tags, ["z", "expiration"]);
        }
    }

    ren.send(Command::Room(RoomCommand::Leave));
    until(&ada, |e, _| {
        matches!(e, Event::RoomMembers(m) if m.iter().any(|m| m.name == "Ren" && m.state == kaeri_model::room::MemberState::Left))
            || matches!(e, Event::RoomMembers(m) if !m.iter().any(|m| m.name == "Ren"))
    });
    ada.shutdown();
    ren.shutdown();
    drop(relays);
}

#[test]
fn a_newcomer_by_a_new_invite_meets_everyone_already_there() {
    // Neon: after New invite, the members already there kept proofs made for the old room id,
    // so a newcomer by the new link took none of them as bound and found itself alone.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let relays: Vec<FakeRelay> =
        runtime.block_on(async { vec![FakeRelay::start().await, FakeRelay::start().await] });
    let urls: Vec<String> = relays.iter().map(FakeRelay::url).collect();
    let roots = [(); 3].map(|()| tempfile::tempdir().unwrap());
    let (ada, mika, ren) = (engine(&roots[0]), engine(&roots[1]), engine(&roots[2]));
    let pick = RoomEpisode { id: AnilistId::new(900_001), episode: "9".into() };

    ada.send(Command::Room(RoomCommand::SetRelays { relays: urls.clone() }));
    ada.send(Command::Room(RoomCommand::Lobby {
        invite: None,
        name: Some("Ada".into()),
        pick: Some(pick),
        anyway: false,
    }));
    let (view, _) = until(&ada, |e, v| {
        matches!(e, Event::RoomJoined(_) | Event::RoomState(_))
            && v.is_some_and(|v| v.link.is_some() && v.room_episode.is_some())
    });
    let first = view.unwrap().link.unwrap();
    mika.send(Command::Room(RoomCommand::Lobby {
        invite: Some(first.clone()),
        name: Some("Mika".into()),
        pick: None,
        anyway: false,
    }));
    until(
        &mika,
        |e, _| matches!(e, Event::RoomMembers(m) if m.iter().any(|m| m.name == "Ada")),
    );
    until(
        &ada,
        |e, _| matches!(e, Event::RoomMembers(m) if m.iter().any(|m| m.name == "Mika")),
    );

    // Ada makes a new invite; Mika takes the new key.
    ada.send(Command::Room(RoomCommand::NewInvite));
    let (view, _) = until(&ada, move |_, v| {
        v.is_some_and(|v| v.link.as_ref().is_some_and(|l| *l != first))
    });
    let second = view.unwrap().link.unwrap();
    until(
        &mika,
        |e, _| matches!(e, Event::RoomMessage(m) if matches!(m.kind, kaeri_model::room::RoomMessageKind::NewInvite { .. })),
    );
    thread::sleep(Duration::from_millis(3_000));
    // Ada steps away, so Mika is the one who answers Ren (a member in the background never
    // does), from what Mika holds of Ada: Ada's first Hello under the new key went out before
    // Mika had moved to it.
    use kaeri_model::command::AppState;
    ada.send(Command::AppPhase { phase: AppState::Background { playing: false } });
    until(&ada, |e, _| matches!(e, Event::RoomFlushed));

    // Ren comes in by the new link and meets both.
    ren.send(Command::Room(RoomCommand::Lobby {
        invite: Some(second),
        name: Some("Ren".into()),
        pick: None,
        anyway: false,
    }));
    until(&ren, |e, _| {
        matches!(e, Event::RoomMembers(m)
            if m.iter().any(|m| m.name == "Ada") && m.iter().any(|m| m.name == "Mika"))
    });
    // Back, and says something at once, while its sockets are still opening again: it goes
    // out once one is up.
    ada.send(Command::AppPhase { phase: AppState::Foreground });
    // And hears each of them: neither is dropped as unproved.
    for (engine, line) in [(&ada, "from Ada"), (&mika, "from Mika")] {
        engine.send(Command::Room(RoomCommand::Say { text: line.into() }));
        until(
            &ren,
            move |e, _| matches!(e, Event::RoomChat(c) if c.lines.iter().any(|l| matches!(&l.body, RoomChatBody::Said { text } if text == line))),
        );
    }
    for engine in [ada, mika, ren] {
        engine.shutdown();
    }
    drop(relays);
}

#[test]
fn a_join_whose_invite_relays_are_down_finds_the_room_on_your_spares() {
    // Neon: joining and peeking never tried the spares, while the copy says they were tried.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let relay = runtime.block_on(FakeRelay::start());
    let live = relay.url();
    // A loopback port nothing listens on.
    let dead = {
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("ws://localhost:{}", socket.local_addr().unwrap().port())
    };
    let (root_a, root_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (ada, ren) = (engine(&root_a), engine(&root_b));
    let pick = RoomEpisode { id: AnilistId::new(900_001), episode: "9".into() };

    ada.send(Command::Room(RoomCommand::SetRelays { relays: vec![live.clone()] }));
    ada.send(Command::Room(RoomCommand::Lobby {
        invite: None,
        name: Some("Ada".into()),
        pick: Some(pick),
        anyway: false,
    }));
    let (view, _) = until(&ada, |e, v| {
        matches!(e, Event::RoomJoined(_) | Event::RoomState(_))
            && v.is_some_and(|v| v.link.is_some() && v.room_episode.is_some())
    });
    let link = view.unwrap().link.unwrap();
    // The same room, its invite naming only a relay that is down.
    let (head, _) = link.split_once("&rl=").expect("the invite names its relays");
    let link = format!("{head}&rl={}", dead.replace(':', "%3A").replace('/', "%2F"));

    // A look finds it on the spares.
    ren.send(Command::Room(RoomCommand::SetSpareRelays { relays: vec![live.clone()] }));
    ren.send(Command::Room(RoomCommand::PeekInvite { invite: link.clone() }));
    let (_, read) =
        until(&ren, |e, _| matches!(e, Event::RoomGlimpse(_) | Event::RoomLookFailed { .. }));
    assert!(
        matches!(read.last(), Some(Event::RoomGlimpse(g)) if g.members.iter().any(|m| m.name == "Ada")),
        "{:?}",
        read.last()
    );

    // And a join meets Ada there.
    ren.send(Command::Room(RoomCommand::Lobby {
        invite: Some(link),
        name: Some("Ren".into()),
        pick: None,
        anyway: false,
    }));
    until(&ren, |e, _| matches!(e, Event::RoomMembers(m) if m.iter().any(|m| m.name == "Ada")));
    ada.shutdown();
    ren.shutdown();
    drop(relay);
}
