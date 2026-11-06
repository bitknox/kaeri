//! Watch together with two Kaeri devices in one room, end to end: two engines as the apps run
//! them, each with the test as its hosted player, on the fake Syncplay server, with a plain
//! Syncplay member the server plays beside them. Each device only learns of the other through
//! the server, as two people's apps would.
//!
//! The players are fakes that do what libmpv does with the ops they are told: load a file,
//! pause, seek, run their playhead on in real time and reach the end. Everything else is the
//! engine as the apps have it.
#![cfg(all(feature = "testing", feature = "party"))]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use kaeri_core::ids::AnilistId;
use kaeri_ffi::{FfiDirs, FfiEngine, FfiFakeParty, FfiHost, parse_party_target};
use kaeri_model::{
    command::{Command, PlayerCommand},
    event::Event,
    hosted::{MpvValue, PlayerEnd, PlayerOp, PlayerReport},
    party::{
        PartyChatBody, PartyCommand, PartyEpisode, PartyMember, PartyReaction, PartySources,
        PartyStartKind, PartyView, Reaction,
    },
    playback::PlaybackId,
};
use kaeri_providers::{MockProvider, ProviderRegistry};
use kaeri_testkit::{fixtures::FINISHED, sources};

/// How long a step waits for every device to get where it should. All on loopback, so a wait
/// this long is a bug.
const PATIENCE: Duration = Duration::from_secs(40);

/// How long each fake file runs.
const RUNTIME: f64 = 60.0;

/// How far apart two players may be and still count as together: the Syncplay client lets a
/// member drift a little before it corrects them.
const TOGETHER: f64 = 2.5;

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

/// The file a device's player has open, as libmpv would have it.
struct Seat {
    playback: PlaybackId,
    position: f64,
    paused: bool,
    loaded: bool,
    ended: bool,
    /// When the playhead was last moved on, and last reported.
    moved: Instant,
    reported: Instant,
}

/// One device: its engine, and the test as its app and player.
struct Device {
    name: &'static str,
    engine: Arc<FfiEngine>,
    events: mpsc::Receiver<Event>,
    read: Vec<String>,
    view: Option<PartyView>,
    members: Vec<PartyMember>,
    started: Vec<(u32, String, PartyStartKind)>,
    left: Vec<Option<String>>,
    no_source: Vec<String>,
    said: Vec<String>,
    reactions: Vec<PartyReaction>,
    seat: Option<Seat>,
    /// Every playback a file was loaded for, in order.
    loads: Vec<PlaybackId>,
    /// How long this device's files run.
    runtime: f64,
    _root: tempfile::TempDir,
}

impl Device {
    fn start(name: &'static str, sources: ProviderRegistry) -> Self {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = |part: &str| root.path().join(part).display().to_string();
        let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
        let engine = FfiEngine::start_testing_with(dirs, Arc::new(Quiet), sources)
            .expect("the engine starts");
        let (tx, events) = mpsc::channel();
        let reader = Arc::clone(&engine);
        thread::spawn(move || {
            loop {
                let batch = futures::executor::block_on(reader.next_events());
                if batch.is_empty() {
                    return;
                }
                for event in batch {
                    if tx.send(event).is_err() {
                        return;
                    }
                }
            }
        });
        Self {
            name,
            engine,
            events,
            read: Vec::new(),
            view: None,
            members: Vec::new(),
            started: Vec::new(),
            left: Vec::new(),
            no_source: Vec::new(),
            said: Vec::new(),
            reactions: Vec::new(),
            seat: None,
            loads: Vec::new(),
            runtime: RUNTIME,
            _root: root,
        }
    }

    fn send(&self, command: Command) {
        self.engine.send(command);
    }

    fn party(&self, command: PartyCommand) {
        self.send(Command::Party(command));
    }

    fn report(&self, playback: PlaybackId, report: PlayerReport) {
        self.engine.send(Command::PlayerReport { playback, report });
    }

    fn property(&self, playback: PlaybackId, name: &str, json: String) {
        self.report(playback, PlayerReport::Property { name: name.into(), json });
    }

    /// The viewer, in the player: what its controls send.
    fn viewer(&self, command: PlayerCommand) {
        self.send(Command::Player(command));
    }

    /// Read what the engine said, play the ops it told the player, and move the playhead on.
    fn pump(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.take(event);
        }
        let runtime = self.runtime;
        let Some(seat) = &mut self.seat else { return };
        let now = Instant::now();
        if seat.loaded && !seat.paused && !seat.ended {
            seat.position = (seat.position + (now - seat.moved).as_secs_f64()).min(runtime);
        }
        seat.moved = now;
        if !seat.loaded || seat.ended {
            return;
        }
        let playback = seat.playback;
        let at_end = seat.position >= runtime && !seat.paused;
        if now - seat.reported >= Duration::from_millis(250) || at_end {
            seat.reported = now;
            let position = seat.position;
            if at_end {
                seat.ended = true;
            }
            self.property(playback, "time-pos", format!("{position}"));
            if at_end {
                self.report(playback, PlayerReport::Ended { reason: PlayerEnd::Eof });
            }
        }
    }

    fn take(&mut self, event: Event) {
        self.read.push(format!("{event:?}"));
        if self.read.len() > 400 {
            self.read.drain(..100);
        }
        match event {
            Event::PartyJoined(view) | Event::PartyState(view) => self.view = Some(view),
            Event::PartyMembers(members) => self.members = members,
            Event::PartyStarted { id, episode, kind } => {
                self.started.push((id.get(), episode, kind));
            }
            Event::PartyLeft { reason } => {
                self.left.push(reason.map(|r| r.english()));
                self.view = None;
                self.members.clear();
            }
            Event::PartyNoSource { episode, .. } => self.no_source.push(episode),
            Event::PartyChat(conversation) => {
                self.said = conversation
                    .lines
                    .iter()
                    .filter_map(|l| match &l.body {
                        PartyChatBody::Said { text } => Some(format!("{}: {text}", l.from)),
                        PartyChatBody::Reacted { .. } => None,
                    })
                    .collect();
            }
            Event::PartyReaction(reaction) => self.reactions.push(reaction),
            Event::PlayerOps { playback, ops } => {
                for op in ops {
                    self.op(playback, op);
                }
            }
            _ => {}
        }
    }

    /// What libmpv does with `op`, reported back as it would be.
    fn op(&mut self, playback: PlaybackId, op: PlayerOp) {
        let now = Instant::now();
        match op {
            PlayerOp::Load { options, .. } => {
                let option = |name: &str| {
                    options.iter().find(|o| o.name == name).map(|o| o.value.clone())
                };
                let start = option("start")
                    .and_then(|s| s.trim_start_matches('+').parse::<f64>().ok())
                    .unwrap_or(0.0);
                let paused = option("pause").is_some_and(|p| p == "yes");
                // The file before is replaced, as `loadfile` replaces it.
                if let Some(before) = self.seat.take()
                    && before.playback != playback
                    && !before.ended
                {
                    self.report(
                        before.playback,
                        PlayerReport::Ended { reason: PlayerEnd::Stop },
                    );
                }
                self.loads.push(playback);
                self.seat = Some(Seat {
                    playback,
                    position: start,
                    paused,
                    loaded: true,
                    ended: false,
                    moved: now,
                    reported: now,
                });
                self.report(playback, PlayerReport::FileLoaded);
                self.property(playback, "duration", format!("{}", self.runtime));
                self.property(playback, "pause", format!("{paused}"));
                self.property(playback, "time-pos", format!("{start}"));
            }
            PlayerOp::Set { property, value } if property == "pause" => {
                let MpvValue::Flag(paused) = value else { return };
                self.set_paused(playback, paused);
            }
            PlayerOp::Cycle { property } if property == "pause" => {
                let paused = self.seat.as_ref().is_some_and(|s| !s.paused);
                self.set_paused(playback, paused);
            }
            PlayerOp::Seek { seconds, absolute } => {
                let runtime = self.runtime;
                let Some(seat) = self.seat.as_mut().filter(|s| s.playback == playback) else {
                    return;
                };
                seat.position = if absolute { seconds } else { seat.position + seconds }
                    .clamp(0.0, runtime);
                seat.ended = false;
                seat.moved = now;
                let position = seat.position;
                self.property(playback, "time-pos", format!("{position}"));
            }
            PlayerOp::Quit => {
                if self.seat.as_ref().is_some_and(|s| s.playback == playback) {
                    self.seat = None;
                }
                self.report(playback, PlayerReport::Ended { reason: PlayerEnd::Quit });
                self.report(playback, PlayerReport::Closed);
            }
            PlayerOp::Close => {
                if self.seat.as_ref().is_some_and(|s| s.playback == playback) {
                    self.seat = None;
                }
                self.report(playback, PlayerReport::Closed);
            }
            _ => {}
        }
    }

    fn set_paused(&mut self, playback: PlaybackId, paused: bool) {
        let runtime = self.runtime;
        let Some(seat) = self.seat.as_mut().filter(|s| s.playback == playback) else { return };
        if seat.paused == paused {
            return;
        }
        // The playhead up to now, then held.
        let now = Instant::now();
        if !seat.paused {
            seat.position = (seat.position + (now - seat.moved).as_secs_f64()).min(runtime);
        }
        seat.moved = now;
        seat.paused = paused;
        let position = seat.position;
        self.property(playback, "pause", format!("{paused}"));
        self.property(playback, "time-pos", format!("{position}"));
    }

    /// The viewer closes the player, as the app does: the file quits, the player closes, and
    /// the playback it showed is stopped.
    fn close_player(&mut self) {
        let seat = self.seat.take().expect("a player to close");
        self.report(seat.playback, PlayerReport::Ended { reason: PlayerEnd::Quit });
        self.report(seat.playback, PlayerReport::Closed);
        self.send(Command::Control { playback: seat.playback, command: PlayerCommand::Stop });
    }

    fn position(&self) -> Option<f64> {
        self.seat.as_ref().filter(|s| s.loaded && !s.ended).map(|s| s.position)
    }

    fn playing(&self) -> bool {
        self.seat.as_ref().is_some_and(|s| s.loaded && !s.paused && !s.ended)
    }

    fn paused(&self) -> bool {
        self.seat.as_ref().is_some_and(|s| s.loaded && s.paused)
    }

    /// The file this device's member has open, as the room sees it.
    fn my_file(&self) -> Option<&str> {
        self.members.iter().find(|m| m.you).and_then(|m| m.file.as_deref())
    }

    fn member(&self, name: &str) -> Option<&PartyMember> {
        self.members.iter().find(|m| m.name == name)
    }

    fn started_with(&self, episode: &str, kind: PartyStartKind) -> bool {
        self.started
            .iter()
            .any(|(id, e, k)| *id == FINISHED.get() && e == episode && *k == kind)
    }

    /// The room started `episode` here, by the count or by following a member whose count
    /// ended first.
    fn started_episode(&self, episode: &str) -> bool {
        self.started.iter().any(|(id, e, _)| *id == FINISHED.get() && e == episode)
    }

    fn room_episode(&self) -> Option<(Option<u32>, String)> {
        let room = self.view.as_ref()?.room_episode.as_ref()?;
        Some((room.id.map(AnilistId::get), room.episode.clone()))
    }

    fn tail(&self) -> String {
        let from = self.read.len().saturating_sub(40);
        format!("{} said, last:\n  {}", self.name, self.read[from..].join("\n  "))
    }
}

/// Pump every device until `done` holds of them, or fail saying what each heard last.
fn wait(what: &str, devices: &mut [&mut Device], done: impl Fn(&[&mut Device]) -> bool) {
    let deadline = Instant::now() + PATIENCE;
    loop {
        for device in devices.iter_mut() {
            device.pump();
        }
        if done(devices) {
            return;
        }
        if Instant::now() > deadline {
            let said: Vec<String> = devices.iter().map(|d| d.tail()).collect();
            panic!("waited in vain for {what}\n{}", said.join("\n\n"));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Pump every device for `time`, for something that should not happen to have its chance.
fn hold(devices: &mut [&mut Device], time: Duration) {
    let until = Instant::now() + time;
    while Instant::now() < until {
        for device in devices.iter_mut() {
            device.pump();
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn together(a: &Device, b: &Device) -> bool {
    match (a.position(), b.position()) {
        (Some(x), Some(y)) => (x - y).abs() <= TOGETHER,
        _ => false,
    }
}

/// The fixture source without the fixture title: a member who has nothing to play it from.
fn without_the_title() -> ProviderRegistry {
    ProviderRegistry::new(vec![MockProvider::new(sources::PROVIDER_ID).arc()])
}

/// Two devices start a party and watch it through together: one starts it for an episode, the
/// other joins by its link, both get ready, and each one's pause, seek and resume reach the
/// other. One closes its player and opens it again without leaving, chat and a reaction go
/// both ways, the room moves on to the next episode together at the end, and one leaves and
/// comes back to the room where it is.
#[test]
fn two_devices_watch_an_episode_together_from_the_lobby_to_the_next() {
    let mut ada = Device::start("Ada", sources::registry());
    let mut ben = Device::start("Ben", sources::registry());
    let party: Arc<FfiFakeParty> = ada.engine.start_fake_party().expect("the fake server");
    let server = party.server();
    let room = "kite-7qm4-xr9d";

    // Ada starts a party for episode 1.
    ada.party(PartyCommand::Lobby {
        server: server.clone(),
        room: room.into(),
        name: Some("Ada".into()),
        pick: Some(PartyEpisode { id: FINISHED, episode: "1".into() }),
    });
    wait("Ada's lobby with its link", &mut [&mut ada], |d| {
        d[0].view.as_ref().is_some_and(|v| v.can_start_anyway && v.link.is_some())
    });
    let link = ada.view.as_ref().and_then(|v| v.link.clone()).unwrap();

    // Ben opens the link: the room and its episode, found in Ben's own sources.
    let target = parse_party_target(link.clone()).expect("the link leads to the room");
    assert_eq!((target.server.as_str(), target.room.as_str()), (server.as_str(), room));
    ben.party(PartyCommand::Lobby {
        server: target.server,
        room: target.room,
        name: Some("Ben".into()),
        pick: None,
    });
    wait("Ben's lobby reading Ada's episode", &mut [&mut ada, &mut ben], |d| {
        let ben = &d[1];
        ben.room_episode() == Some((Some(FINISHED.get()), "1".into()))
            && ben.view.as_ref().is_some_and(|v| v.sources == Some(PartySources::Found))
            && d[0].members.len() == 2
            && ben.members.len() == 2
    });
    let ben_view = ben.view.clone().unwrap();
    assert_eq!(ben_view.starter.as_deref(), Some("Ada"));
    assert!(!ben_view.can_start_anyway, "Ada started it, so only Ada can start it anyway");
    assert!(ben_view.next_episode.as_ref().is_some_and(|n| n.episode == "2"));

    // Ben is ready, Ada is not: nothing starts.
    ben.send(Command::PartyReady { ready: true });
    wait("Ada to see Ben ready", &mut [&mut ada, &mut ben], |d| {
        d[0].member("Ben").is_some_and(|m| m.ready == Some(true))
    });
    hold(&mut [&mut ada, &mut ben], Duration::from_millis(1500));
    assert!(ada.started.is_empty() && ben.started.is_empty(), "nobody started yet");

    // Ada is ready too: both count down and both players open on episode 1.
    ada.send(Command::PartyReady { ready: true });
    wait("both to start episode 1", &mut [&mut ada, &mut ben], |d| {
        d.iter().all(|d| d.started_episode("1") && d.playing())
    });
    assert!(
        ada.started_with("1", PartyStartKind::Start)
            || ben.started_with("1", PartyStartKind::Start),
        "{:?} {:?}",
        ada.started,
        ben.started
    );
    assert!(
        ada.read.iter().chain(&ben.read).any(|l| l.contains("starting_in: Some(3)")),
        "the count ran"
    );
    wait("each to see the other's file", &mut [&mut ada, &mut ben], |d| {
        d.iter().all(|d| d.my_file().is_some_and(|f| f.ends_with("[anilist:900001:1]")))
            && d[0].member("Ben").is_some_and(|m| m.file_matches == Some(true))
    });

    // Ben pauses: Ada's player pauses.
    hold(&mut [&mut ada, &mut ben], Duration::from_secs(2));
    ben.viewer(PlayerCommand::TogglePause);
    wait("Ada paused by Ben", &mut [&mut ada, &mut ben], |d| d[0].paused() && d[1].paused());
    // Ada seeks while paused: Ben's player goes there.
    ada.viewer(PlayerCommand::SeekTo(30.0));
    wait("Ben at Ada's seek", &mut [&mut ada, &mut ben], |d| {
        d[1].position().is_some_and(|p| (p - 30.0).abs() < 1.0)
    });
    // Ada resumes: Ben plays on with her.
    ada.viewer(PlayerCommand::TogglePause);
    wait("Ben resumed by Ada", &mut [&mut ada, &mut ben], |d| {
        d[0].playing() && d[1].playing() && together(d[0], d[1])
    });
    // Ben seeks back while playing: Ada follows.
    ben.viewer(PlayerCommand::SeekTo(12.0));
    wait("Ada at Ben's seek", &mut [&mut ada, &mut ben], |d| {
        d[0].position().is_some_and(|p| (11.0..16.0).contains(&p)) && together(d[0], d[1])
    });

    // Chat both ways, and a reaction.
    ada.party(PartyCommand::Say { text: "the rain scene is next".into() });
    ben.party(PartyCommand::Say { text: "I know".into() });
    wait("both lines on both devices", &mut [&mut ada, &mut ben], |d| {
        d.iter().all(|d| {
            d.said.iter().any(|l| l == "Ada: the rain scene is next")
                && d.said.iter().any(|l| l == "Ben: I know")
        })
    });
    ben.party(PartyCommand::React { reaction: Reaction::Gasp });
    wait("Ben's reaction on Ada's player", &mut [&mut ada, &mut ben], |d| {
        d[0].reactions.iter().any(|r| r.from == "Ben" && r.reaction == Reaction::Gasp)
    });

    // Ben closes the player: still in the room, and Ada plays on.
    ben.close_player();
    hold(&mut [&mut ada, &mut ben], Duration::from_secs(2));
    assert!(ben.left.is_empty(), "closing the player left the party: {}", ben.tail());
    assert!(ben.view.as_ref().is_some_and(|v| v.playing), "the room still plays for Ben");
    assert!(ada.member("Ben").is_some(), "Ada still has Ben in the room");
    assert!(ada.playing(), "Ada plays on: {}", ada.tail());
    // Open player: back in, where the room is.
    ben.party(PartyCommand::OpenPlayer);
    wait("Ben back with the room", &mut [&mut ada, &mut ben], |d| {
        d[1].started_with("1", PartyStartKind::Rejoin) && d[1].playing() && together(d[0], d[1])
    });

    // Near the end, the room moves on to episode 2 together.
    ada.viewer(PlayerCommand::SeekTo(RUNTIME - 3.0));
    wait("both on episode 2", &mut [&mut ada, &mut ben], |d| {
        d.iter().all(|d| {
            d.started_with("2", PartyStartKind::MoveOn)
                && d.my_file().is_some_and(|f| f.ends_with("[anilist:900001:2]"))
                && d.playing()
        })
    });
    wait("together on episode 2", &mut [&mut ada, &mut ben], |d| together(d[0], d[1]));

    // Ben leaves: Ada is told and plays on; Ben's player plays on alone.
    ben.send(Command::PartyLeave);
    wait("Ben out and Ada told", &mut [&mut ada, &mut ben], |d| {
        d[1].left == [None] && d[0].member("Ben").is_none()
    });
    assert!(ada.playing(), "Ada plays on after Ben leaves: {}", ada.tail());
    if ben.seat.is_some() {
        ben.close_player();
    }
    // Ben comes back by the same link: the room is playing episode 2, and Ben joins in there.
    hold(&mut [&mut ada, &mut ben], Duration::from_secs(3));
    let target = parse_party_target(link).unwrap();
    ben.party(PartyCommand::Lobby {
        server: target.server,
        room: target.room,
        name: Some("Ben".into()),
        pick: None,
    });
    wait("Ben back in, playing with the room", &mut [&mut ada, &mut ben], |d| {
        d[1].started_with("2", PartyStartKind::Rejoin)
            && d[1].playing()
            && d[0].member("Ben").is_some()
            && together(d[0], d[1])
    });

    party.stop();
    ada.engine.shutdown();
    ben.engine.shutdown();
}

/// A room joined while it plays, by a member who never says ready: this device follows the
/// room into the player at once. And a member who started nothing can still start a room
/// whose starter left.
#[test]
fn a_room_another_device_starts_is_followed_into_the_player() {
    let mut ada = Device::start("Ada", sources::registry());
    let mut ben = Device::start("Ben", sources::registry());
    let party = ada.engine.start_fake_party().expect("the fake server");
    let server = party.server();
    let room = "lark-3vd8-pq2m";

    ada.party(PartyCommand::Lobby {
        server: server.clone(),
        room: room.into(),
        name: Some("Ada".into()),
        pick: Some(PartyEpisode { id: FINISHED, episode: "4".into() }),
    });
    ben.party(PartyCommand::Lobby {
        server: server.clone(),
        room: room.into(),
        name: Some("Ben".into()),
        pick: None,
    });
    wait("Ben's lobby on episode 4", &mut [&mut ada, &mut ben], |d| {
        d[1].room_episode() == Some((Some(FINISHED.get()), "4".into()))
            && d[0].members.len() == 2
    });
    // Ada starts without waiting: Ben, never ready, follows into the player.
    ada.party(PartyCommand::StartAnyway);
    wait("Ben following Ada's start", &mut [&mut ada, &mut ben], |d| {
        d[0].started_with("4", PartyStartKind::Start)
            && d[0].playing()
            && d[1].started.iter().any(|(_, e, _)| e == "4")
            && d[1].playing()
            && together(d[0], d[1])
    });

    party.stop();
    ada.engine.shutdown();
    ben.engine.shutdown();
}

/// A plain Syncplay client in the room: its file name is matched to the title, and when it
/// changes the room's file both devices move to that episode; what the plain client sees of a
/// Kaeri member is a readable name, with the tag that lets another Kaeri find the episode.
#[test]
fn a_plain_syncplay_member_picks_by_file_name_and_both_devices_follow() {
    let mut ada = Device::start("Ada", sources::registry());
    let mut ben = Device::start("Ben", sources::registry());
    let party = ada.engine.start_fake_party().expect("the fake server");
    let server = party.server();
    let room = "wren-5kt2-hb7c";
    let plain = "Stardust Wayfarer - 03 (1080p).mkv";
    party.add_member(room.into(), "Pia".into());
    party.member_sets_playlist("Pia".into(), vec![plain.into()]);
    party.member_sets_index("Pia".into(), 0);
    party.member_opens("Pia".into(), plain.into(), RUNTIME);
    party.member_ready("Pia".into(), true);

    for (device, name) in [(&ada, "Ada"), (&ben, "Ben")] {
        device.party(PartyCommand::Lobby {
            server: server.clone(),
            room: room.into(),
            name: Some(name.into()),
            pick: None,
        });
    }
    wait("both matching Pia's file to episode 3", &mut [&mut ada, &mut ben], |d| {
        d.iter().all(|d| {
            d.room_episode() == Some((Some(FINISHED.get()), "3".into()))
                && d.view.as_ref().is_some_and(|v| v.sources == Some(PartySources::Found))
        })
    });
    let found = ada.view.as_ref().and_then(|v| v.room_episode.clone()).unwrap();
    assert_eq!(found.plain_by.as_deref(), Some("Pia"), "{found:?}");

    ada.send(Command::PartyReady { ready: true });
    ben.send(Command::PartyReady { ready: true });
    wait("both playing episode 3", &mut [&mut ada, &mut ben], |d| {
        d.iter().all(|d| d.started_episode("3") && d.playing())
    });
    wait("the room seeing Kaeri's file names", &mut [&mut ada, &mut ben], |d| {
        d[0].member("Ben").and_then(|m| m.file.as_deref()).is_some_and(|f| {
            f.starts_with("Stardust Wayfarer") && f.ends_with("[anilist:900001:3]")
        })
    });
    let pia = ada.member("Pia").cloned().unwrap();
    assert_eq!(pia.file.as_deref(), Some(plain));
    assert_eq!(pia.file_matches, Some(true), "Pia's file is the room's episode: {pia:?}");

    // Pia moves the room to the next file: both devices go with it.
    let next = "Stardust Wayfarer - 04 (1080p).mkv";
    party.member_sets_playlist("Pia".into(), vec![plain.into(), next.into()]);
    party.member_sets_index("Pia".into(), 1);
    wait("both moved to episode 4", &mut [&mut ada, &mut ben], |d| {
        d.iter().all(|d| {
            d.started.iter().any(|(_, e, _)| e == "4")
                && d.my_file().is_some_and(|f| f.ends_with("[anilist:900001:4]"))
        })
    });

    party.stop();
    ada.engine.shutdown();
    ben.engine.shutdown();
}

/// A member whose sources lack the room's episode: the lobby says so, the room still starts for
/// the others, and this member stays in the room, told why, without a player.
#[test]
fn a_member_without_the_episode_stays_in_the_room_while_the_others_play() {
    let mut ada = Device::start("Ada", sources::registry());
    let mut ben = Device::start("Ben", sources::registry());
    let mut cy = Device::start("Cy", without_the_title());
    let party = ada.engine.start_fake_party().expect("the fake server");
    let server = party.server();
    let room = "moss-8hd2-kq4t";

    ada.party(PartyCommand::Lobby {
        server: server.clone(),
        room: room.into(),
        name: Some("Ada".into()),
        pick: Some(PartyEpisode { id: FINISHED, episode: "5".into() }),
    });
    for (device, name) in [(&ben, "Ben"), (&cy, "Cy")] {
        device.party(PartyCommand::Lobby {
            server: server.clone(),
            room: room.into(),
            name: Some(name.into()),
            pick: None,
        });
    }
    wait(
        "Cy's lobby saying its sources lack the episode",
        &mut [&mut ada, &mut ben, &mut cy],
        |d| {
            d[2].view
                .as_ref()
                .is_some_and(|v| matches!(v.sources, Some(PartySources::Missing { .. })))
                && d[0].members.len() == 3
        },
    );
    for device in [&ada, &ben, &cy] {
        device.send(Command::PartyReady { ready: true });
    }
    wait("Ada and Ben playing, Cy told why not", &mut [&mut ada, &mut ben, &mut cy], |d| {
        d[0].playing() && d[1].playing() && d[2].no_source.iter().any(|e| e == "5")
    });
    hold(&mut [&mut ada, &mut ben, &mut cy], Duration::from_secs(2));
    assert!(cy.left.is_empty(), "Cy stays in the room: {}", cy.tail());
    assert!(cy.seat.is_none(), "Cy has nothing to play");
    assert!(ada.member("Cy").is_some());
    assert!(together(&ada, &ben));

    party.stop();
    for device in [ada, ben, cy] {
        device.engine.shutdown();
    }
}

/// Not a test: a Kaeri member named Mika for a person or a script to watch together with from
/// another app, such as the Windows testing build. Run with `--ignored` and `KAERI_PEER_DIR`
/// set to a folder. Mika starts a room on a fake server of its own (or joins
/// `KAERI_PEER_SERVER` and `KAERI_PEER_ROOM`, a real server's room, when set), writes the
/// `kaeri://` link that opens it to `link.txt`, and then does what each new line of `cmd.txt`
/// says, writing what Mika sees to `peer.log`:
///
/// `pick <episode>`, `ready`, `unready`, `start`, `pause` and `play` (Mika's player),
/// `seek <seconds>`, `end` (to three seconds before the end), `say <text>`, `react`, `close`
/// and `open` (the player), `leave`, `join`, `quit`.
#[test]
#[ignore = "a peer for another app to watch with, run by hand"]
fn a_peer_to_watch_together_with() {
    use std::{fmt::Write as _, fs, io::Write as _, path::PathBuf};

    use kaeri_model::party::PartyShare;

    let dir = PathBuf::from(std::env::var("KAERI_PEER_DIR").expect("KAERI_PEER_DIR"));
    fs::create_dir_all(&dir).unwrap();
    let mut mika = Device::start("Mika", sources::registry());
    if let Some(runtime) = std::env::var("KAERI_PEER_RUNTIME").ok().and_then(|r| r.parse().ok())
    {
        mika.runtime = runtime;
    }
    let fake = match std::env::var("KAERI_PEER_SERVER") {
        Ok(_) => None,
        Err(_) => Some(mika.engine.start_fake_party().expect("the fake server")),
    };
    let server =
        std::env::var("KAERI_PEER_SERVER").unwrap_or_else(|_| fake.as_ref().unwrap().server());
    let room = std::env::var("KAERI_PEER_ROOM").unwrap_or_else(|_| "kite-7qm4-xr9d".into());
    let episode = std::env::var("KAERI_PEER_EPISODE").unwrap_or_else(|_| "7".into());
    let link = kaeri_ffi::party_share_app_link(PartyShare {
        server: server.clone(),
        room: room.clone(),
        by: Some("Mika".into()),
        title: None,
        episode: None,
    });
    fs::write(dir.join("link.txt"), &link).unwrap();
    let cmd = dir.join("cmd.txt");
    fs::write(&cmd, "").unwrap();
    let mut log = fs::File::create(dir.join("peer.log")).unwrap();
    let began = Instant::now();
    let note = |log: &mut fs::File, line: &str| {
        let _ = writeln!(log, "{:7.2} {line}", began.elapsed().as_secs_f64());
        let _ = log.flush();
    };
    note(&mut log, &format!("link {link}"));
    let lobby = |mika: &Device, pick: Option<&str>| {
        mika.party(PartyCommand::Lobby {
            server: server.clone(),
            room: room.clone(),
            name: Some("Mika".into()),
            pick: pick.map(|e| PartyEpisode { id: FINISHED, episode: e.into() }),
        });
    };
    lobby(&mika, Some(&episode));

    let (mut done, mut said_members, mut said_view, mut said_chat) =
        (0, String::new(), String::new(), 0);
    let (mut said_started, mut said_reactions, mut said_left, mut last_seat) = (0, 0, 0, began);
    loop {
        mika.pump();
        let lines: Vec<String> =
            fs::read_to_string(&cmd).unwrap_or_default().lines().map(str::to_owned).collect();
        for line in lines.iter().skip(done) {
            note(&mut log, &format!("> {line}"));
            let (verb, rest) = line.split_once(' ').unwrap_or((line.as_str(), ""));
            match verb {
                "pick" => mika.party(PartyCommand::ChangeEpisode {
                    id: FINISHED,
                    episode: rest.trim().into(),
                }),
                "ready" => mika.send(Command::PartyReady { ready: true }),
                "unready" => mika.send(Command::PartyReady { ready: false }),
                "start" => mika.party(PartyCommand::StartAnyway),
                "pause" if !mika.paused() => mika.viewer(PlayerCommand::TogglePause),
                "play" if mika.paused() => mika.viewer(PlayerCommand::TogglePause),
                "seek" => {
                    mika.viewer(PlayerCommand::SeekTo(rest.trim().parse().unwrap_or(0.0)))
                }
                "end" => mika.viewer(PlayerCommand::SeekTo(mika.runtime - 3.0)),
                "say" => mika.party(PartyCommand::Say { text: rest.into() }),
                "react" => mika.party(PartyCommand::React { reaction: Reaction::Laugh }),
                "close" if mika.seat.is_some() => mika.close_player(),
                "open" => mika.party(PartyCommand::OpenPlayer),
                "leave" => mika.send(Command::PartyLeave),
                "join" => lobby(&mika, None),
                "quit" => {
                    if let Some(fake) = &fake {
                        fake.stop();
                    }
                    mika.engine.shutdown();
                    return;
                }
                _ => note(&mut log, "  (nothing to do)"),
            }
        }
        done = lines.len();
        // What changed, said once.
        let members = mika.members.iter().fold(String::new(), |mut out, m| {
            let _ = write!(
                out,
                "{}{} ready={:?} file={:?} len={:?} same={:?}; ",
                m.name,
                if m.you { "(me)" } else { "" },
                m.ready,
                m.file,
                m.duration.map(f64::round),
                m.file_matches
            );
            out
        });
        if members != said_members {
            note(&mut log, &format!("members: {members}"));
            said_members = members;
        }
        let view = mika.view.as_ref().map_or_else(String::new, |v| {
            format!(
                "room={:?} next={:?} playing={} starting_in={:?} starter={:?} sources={:?} ready={}",
                v.room_episode.as_ref().map(|r| (&r.title, &r.episode, &r.found)),
                v.next_episode.as_ref().map(|r| &r.episode),
                v.playing,
                v.starting_in,
                v.starter,
                v.sources,
                v.ready
            )
        });
        if view != said_view {
            note(&mut log, &format!("view: {view}"));
            said_view = view;
        }
        for started in &mika.started[said_started..] {
            note(&mut log, &format!("started: {started:?}"));
        }
        said_started = mika.started.len();
        for line in mika.said.iter().skip(said_chat) {
            note(&mut log, &format!("chat: {line}"));
        }
        said_chat = said_chat.max(mika.said.len());
        for r in &mika.reactions[said_reactions..] {
            note(&mut log, &format!("reaction: {} {:?} at {:.1}", r.from, r.reaction, r.at));
        }
        said_reactions = mika.reactions.len();
        for left in &mika.left[said_left..] {
            note(&mut log, &format!("left: {left:?}"));
        }
        said_left = mika.left.len();
        if last_seat.elapsed() >= Duration::from_secs(1) {
            last_seat = Instant::now();
            if let Some(seat) = &mika.seat {
                note(
                    &mut log,
                    &format!(
                        "player: {:.1} {}",
                        seat.position,
                        if seat.paused { "paused" } else { "playing" }
                    ),
                );
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
}
