//! The room's scenes for the Fixtures build: the engine's fake room with scripted members, staged
//! as the board's frames, the way `FfiFakeRoom::stage` stages them for the Mac and the phones.
//! The device is Ada, with Mika and Ren (and Sora, Kenji and Noa in the six-person ones) in a
//! room watching Stardust Wayfarer episode 9. Menus, Invite, Details, Edit relays and the
//! dialogs are the app's own, opened over Watching.

use std::{
    sync::{Arc, Weak},
    time::Duration,
};

use kaeri_engine::engine::Engine;
use kaeri_model::{
    command::Command,
    room::{RoomCommand, RoomShare},
};
use kaeri_room::{
    fake::{FakeRoom, YOU},
    wire::{self, Ep, StateCode},
};

/// The title every scene's room watches: Stardust Wayfarer (fixtures 900001).
const TITLE: u32 = 900_001;
const TITLE_NAME: &str = "Stardust Wayfarer"; // i18n-skip
const EPISODE: &str = "9";
const NEXT: &str = "10";
const DEVICE: &str = "Ada";
/// Inside the Fixtures clip (60 s), as the engine's scenes keep: past the opening, clear of
/// Up next. A seek past the clip would end the episode.
const OPENING_ENDS: f64 = 20.0;
const SEEK_TO: f64 = 35.0;

/// One scene per frame family on the room board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    BeforePlay,
    BeforePlayWait,
    BeforePlayCap,
    Starting,
    Invited,
    InvitedFull,
    Watching,
    Paused,
    WaitLoading,
    WaitCap,
    CarriedOn,
    YouLoading,
    Buffering,
    BufferingSix,
    Choosing,
    NoSource,
    PlayerClosed,
    Reconnecting,
    Behind,
    OtherEpisode,
    Left,
    YouChoosing,
    YouNoSource,
    YouBehind,
    YouReconnecting,
    Six,
    RelayTrouble,
    UpNext,
    UpNextWaiting,
    SkippedOpening,
    Chat,
    MoveOffer,
    MovedAway,
    Unreachable,
}

/// The room's shots: the scene each stages, what it opens over the room, and whether it is
/// full screen.
pub const SHOTS: [(&str, Scene, Option<&str>, bool); 46] = [
    ("room-morph", Scene::Watching, None, false),
    ("room-watching", Scene::Chat, None, false),
    ("room-paused", Scene::Paused, None, false),
    ("room-wait", Scene::WaitLoading, None, false),
    ("room-cap", Scene::WaitCap, None, false),
    ("room-carry", Scene::CarriedOn, None, false),
    ("room-self-wait", Scene::YouLoading, None, false),
    ("room-buffering", Scene::Buffering, None, false),
    ("room-buffering-six", Scene::BufferingSix, None, false),
    ("room-choosing", Scene::Choosing, None, false),
    ("room-nosource", Scene::NoSource, None, false),
    ("room-closed", Scene::PlayerClosed, None, false),
    ("room-reconnecting", Scene::Reconnecting, None, false),
    ("room-behind", Scene::Behind, None, false),
    ("room-other", Scene::OtherEpisode, None, false),
    ("room-left", Scene::Left, None, false),
    ("room-self-choosing", Scene::YouChoosing, None, false),
    ("room-self-nosource", Scene::YouNoSource, None, false),
    ("room-self-behind", Scene::YouBehind, None, false),
    ("room-self-conn", Scene::YouReconnecting, None, false),
    ("room-relay-trouble", Scene::RelayTrouble, None, false),
    ("room-upnext", Scene::UpNext, None, false),
    ("room-upnext-waiting", Scene::UpNextWaiting, None, false),
    ("room-skipped", Scene::SkippedOpening, None, false),
    ("room-six", Scene::Six, None, false),
    ("room-before-idle", Scene::BeforePlay, None, false),
    ("room-before-wait", Scene::BeforePlayWait, None, false),
    ("room-before-cap", Scene::BeforePlayCap, None, false),
    ("room-before-count", Scene::Starting, None, false),
    ("room-invited", Scene::Invited, None, false),
    ("room-invited-over", Scene::InvitedFull, None, false),
    ("room-unreachable", Scene::Unreachable, None, false),
    ("room-invite", Scene::Watching, Some("invite"), false),
    ("room-details", Scene::Watching, Some("details"), false),
    ("room-details-trouble", Scene::RelayTrouble, Some("details"), false),
    ("room-edit-relays", Scene::Watching, Some("edit-relays"), false),
    ("room-leave", Scene::Watching, Some("leave"), false),
    ("room-close", Scene::Watching, Some("close"), false),
    ("room-move-pick", Scene::Watching, Some("move"), false),
    ("room-move-offer", Scene::MoveOffer, None, false),
    ("room-move-after", Scene::MovedAway, None, false),
    ("room-toggle-off", Scene::Watching, Some("toggle-off"), false),
    ("room-full-watching", Scene::Watching, None, true),
    ("room-full-wait", Scene::WaitLoading, None, true),
    ("room-full-cap", Scene::WaitCap, None, true),
    ("room-full-choosing", Scene::Choosing, None, true),
];

/// The shot's scene, what it opens, and whether it is full screen.
pub fn frame(shot: &str) -> Option<(Scene, Option<&'static str>, bool)> {
    SHOTS.iter().find(|s| s.0 == shot).map(|s| (s.1, s.2, s.3))
}

fn ep(episode: &str) -> Ep {
    Ep(TITLE, episode.to_owned())
}

/// A fake room the device joins as Ada.
pub struct Staged {
    fake: FakeRoom,
    runtime: tokio::runtime::Handle,
    engine: Weak<Engine>,
}

impl Staged {
    /// An empty room on fake relays: rooms this engine starts from now on go there too.
    pub fn start(runtime: tokio::runtime::Handle, engine: Weak<Engine>) -> Arc<Self> {
        let clock: kaeri_room::fake::Clock = Arc::new(kaeri_engine::room::now_ms);
        let fake = {
            let _entered = runtime.enter();
            FakeRoom::start(clock)
        };
        Arc::new(Self { fake, runtime, engine })
    }

    fn run<T>(&self, f: impl FnOnce(&FakeRoom) -> T) -> T {
        let _entered = self.runtime.enter();
        f(&self.fake)
    }

    fn pause(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    fn send(&self, command: Command) {
        if let Some(engine) = self.engine.upgrade() {
            engine.send(command);
        }
    }

    /// The room's invite, as `by` shares it.
    pub fn invite(&self, by: &str) -> String {
        RoomShare {
            secret: self.fake.secret(),
            relays: self.fake.relays(),
            by: Some(by.into()),
            title: Some(TITLE_NAME.into()),
            episode: Some(EPISODE.into()),
        }
        .https_link()
    }

    fn wait_joined(&self, secs: u64) -> bool {
        for _ in 0..secs * 20 {
            if self.fake.you().is_some() {
                self.pause(200);
                return true;
            }
            self.pause(50);
        }
        false
    }

    fn join(&self) {
        self.send(Command::Room(RoomCommand::Lobby {
            invite: Some(self.invite("Mika")),
            name: Some(DEVICE.into()),
            pick: None,
            anyway: true,
        }));
    }

    /// Stage `scene`. Blocks for a few seconds: call it off the main loop. `false` when the
    /// device never got into the room.
    pub fn stage(&self, scene: Scene) -> bool {
        use Scene as S;
        let six = matches!(scene, S::BufferingSix | S::Six);
        let mut names = vec!["Mika", "Ren"];
        if six {
            names.extend(["Sora", "Kenji", "Noa"]);
        }
        if scene == S::InvitedFull {
            names = vec![
                "Mika", "Ren", "Sora", "Kenji", "Noa", "Yui", "Taro", "Hana", "Kai", "Rio",
            ];
        }
        if scene == S::MovedAway {
            names.push("Taro");
        }
        for name in &names {
            self.run(|f| f.add_member(name));
        }
        let before_play =
            matches!(scene, S::BeforePlay | S::BeforePlayWait | S::BeforePlayCap | S::Starting);
        self.run(|f| f.member_starts("Mika", ep(EPISODE), Some(ep(NEXT)), !before_play));
        if matches!(scene, S::BeforePlay | S::BeforePlayWait | S::BeforePlayCap) {
            let secs = match scene {
                S::BeforePlay => 4.0,
                S::BeforePlayWait => 6.0,
                _ => 20.0,
            };
            self.run(|f| f.member_loads("Ren", secs));
        }
        if scene == S::Unreachable {
            self.run(|f| f.unreachable(true));
            self.join();
            return true;
        }
        if matches!(scene, S::Invited | S::InvitedFull) {
            self.send(Command::Room(RoomCommand::PeekInvite { invite: self.invite("Mika") }));
            return true;
        }
        self.join();
        if !self.wait_joined(8) {
            return false;
        }
        let ready = |f: &FakeRoom| f.pose_you(StateCode::Watching, 0.0, None, None, false);
        self.run(ready);
        self.pause(200);
        match scene {
            S::BeforePlay | S::Invited | S::InvitedFull | S::Unreachable | S::Watching => {}
            S::BeforePlayWait | S::BeforePlayCap | S::Starting => {
                self.run(|f| f.member_plays("Mika"));
            }
            S::Paused => self.run(|f| f.member_pauses("Mika")),
            S::WaitLoading | S::WaitCap | S::CarriedOn => {
                self.run(|f| f.member_seeks("Mika", SEEK_TO));
                let secs = match scene {
                    S::WaitLoading => 6.0,
                    S::WaitCap => 20.0,
                    _ => 24.0,
                };
                self.pause(200);
                self.run(|f| f.member_loads("Ren", secs));
                if scene == S::CarriedOn {
                    self.pause(200);
                    self.run(|f| f.member_carries_on("Mika"));
                }
            }
            S::YouLoading => {
                self.run(|f| f.member_seeks("Mika", SEEK_TO));
                self.pause(200);
                self.run(|f| f.pose_you(StateCode::Loading, 6.0, None, None, false));
            }
            S::Buffering | S::BufferingSix => self.run(|f| f.member_stalls("Ren", 3.0)),
            S::Choosing => self.run(|f| f.member_chooses_source("Ren")),
            S::NoSource => self.run(|f| f.member_has_no_source("Ren")),
            S::PlayerClosed => self.run(|f| f.member_closes_player("Ren")),
            S::Reconnecting => self.run(|f| f.member_drops("Ren")),
            S::Behind => self.run(|f| f.member_behind("Ren", 4.0)),
            S::OtherEpisode => self.run(|f| f.member_on_episode("Ren", ep("8"))),
            S::Left => self.run(|f| f.member_leaves("Ren")),
            S::YouChoosing => {
                self.run(|f| f.pose_you(StateCode::Choosing, 0.0, None, None, false));
            }
            S::YouNoSource => {
                self.run(|f| f.pose_you(StateCode::NoSource, 0.0, None, None, false));
            }
            S::YouBehind => {
                self.run(|f| f.pose_you(StateCode::Behind, 4.0, None, Some(4.0), false));
            }
            S::YouReconnecting => self.run(|f| {
                for i in 0..3 {
                    f.relay_down(i, true);
                }
            }),
            S::Six => self.run(|f| f.member_has_no_source("Sora")),
            S::RelayTrouble => self.run(|f| f.relay_down(1, true)),
            S::UpNext => self.run(|f| {
                for name in ["Mika", "Ren"] {
                    f.member_reaches_up_next(name);
                }
                f.pose_you(StateCode::Watching, 0.0, None, None, true);
                f.member_counts_up_next("Mika");
            }),
            S::UpNextWaiting => self.run(|f| {
                f.member_reaches_up_next("Mika");
                f.pose_you(StateCode::Watching, 0.0, None, None, true);
            }),
            S::SkippedOpening => self.run(|f| f.member_skips_opening("Mika", OPENING_ENDS)),
            S::Chat => self.run(|f| {
                f.member_says("Mika", "ok starting");
                f.member_says("Ren", "the handoff in the rain, I knew it");
                f.member_reacts("Ren", wire::Reaction::Gasp, 24.0);
                f.member_says("Mika", "this part again");
            }),
            S::MoveOffer => {
                self.run(|f| f.member_moves("Mika", &[YOU, "Ren"]));
            }
            S::MovedAway => {
                self.run(|f| f.member_moves("Mika", &["Ren"]));
                self.pause(200);
                self.run(|f| f.member_answers_move("Ren", true));
            }
        }
        true
    }
}
