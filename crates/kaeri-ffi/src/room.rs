//! The room, for a front end: the invite link a room is shared by, read and written by the
//! model's rules so every app makes and opens the same links, and, in a testing build, a room
//! with scripted members for the Fixtures apps, with one call per board frame
//! ([`FfiFakeRoom::stage`]).
//!
//! The room itself is the engine's, through `Command::Room` and the `Room*` events.

use kaeri_model::room::{RoomShare, RoomTarget};

/// What pasted text or an opened link names: a room's invite, or an older version's
/// (`old` set: "This invite is from an older version of Kaeri. Ask for a new one."). `None` for
/// anything else.
#[uniffi::export]
pub fn parse_room_target(text: String) -> Option<RoomTarget> {
    RoomTarget::parse(&text)
}

/// The `kaeri://party#…` form of an https invite, for a device with the app (the TV handoff).
/// `None` when it isn't a room's invite.
#[uniffi::export]
pub fn room_app_link(link: String) -> Option<String> {
    RoomShare::parse(&link).ok().flatten().map(|share| share.app_link())
}

#[cfg(all(feature = "room", feature = "testing"))]
pub use fake::{FfiFakeRoom, FfiRoomScene};

/// Where the room's scenes put the playhead: inside the Fixtures apps' clip, which the app's
/// player plays to its end and on to the next episode. A seek past it ends the episode, and the
/// scene can't be shot.
#[cfg(feature = "testing")]
mod scene {
    /// The Fixtures apps' clip, in seconds.
    pub const CLIP_SECS: f64 = 60.0;
    /// Where the clip's opening mark ends: Skip opening's target.
    pub const OPENING_ENDS: f64 = 20.0;
    /// Up Next's window before the end, which a scene keeps clear of.
    const UP_NEXT_SECS: f64 = 10.0;
    /// A seek's target: after the opening, well before Up Next.
    pub const SEEK_TO: f64 = OPENING_ENDS + (CLIP_SECS - UP_NEXT_SECS - OPENING_ENDS) * 0.5;
    const _: () = assert!(SEEK_TO > OPENING_ENDS && SEEK_TO < CLIP_SECS - UP_NEXT_SECS);

    /// `fraction` of the way through the clip.
    pub fn at(fraction: f64) -> f64 {
        (CLIP_SECS * fraction).round()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn every_scene_position_is_inside_the_clip() {
            assert_eq!(SEEK_TO, 35.0);
            for f in [0.15, 0.4, 0.65] {
                assert!(at(f) < CLIP_SECS - UP_NEXT_SECS);
            }
        }
    }
}

#[cfg(all(feature = "room", feature = "testing"))]
mod fake {
    use std::{sync::Arc, time::Duration};

    use kaeri_model::{
        command::Command,
        room::{Reaction, RoomCommand, RoomShare},
    };
    use kaeri_room::{
        fake::{FakeRoom, YOU},
        wire::{self, Ep, StateCode},
    };
    use tokio::runtime::Handle;

    use crate::engine::FfiEngine;

    /// The title every scene's room watches: Stardust Wayfarer (fixtures 900001), whose episodes
    /// play in the Fixtures apps. The board draws another made-up title; this one plays.
    const TITLE: u32 = 900_001;
    const TITLE_NAME: &str = "Stardust Wayfarer";
    /// The board's episode, and the one after it.
    const EPISODE: &str = "9";
    const NEXT: &str = "10";
    /// An episode no device can play yet: Natsukaze Relay (fixtures 900006) has aired 8 of 12.
    const UNAIRED: (u32, &str) = (900_006, "9");
    /// The device's name in every scene.
    const DEVICE: &str = "Ada";

    /// One scene per frame family on the room board, staged by [`FfiFakeRoom::stage`]: the
    /// device is Ada, with Mika and Ren (and Sora, Kenji and Noa in the six-person ones) in a
    /// room watching Stardust Wayfarer episode 9. Menus, dialogs, Invite, Details and Edit relays
    /// are the app's own, opened over [`FfiRoomScene::Watching`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
    pub enum FfiRoomScene {
        /// Before play: Mika and Ada Ready, Ren Loading · 4 s; nobody pressed Play for everyone.
        BeforePlay,
        /// Before play, Play pressed, waiting for Ren: "Starting when Ren is ready", 6 s.
        BeforePlayWait,
        /// Before play at 20 s: Start without Ren.
        BeforePlayCap,
        /// Everyone is ready: "Starting in 3" (`starts_at_ms`).
        Starting,
        /// Opened from an invite, not joined yet: a `RoomGlimpse` with Mika and Ren.
        Invited,
        /// The same with ten people there: Join anyway.
        InvitedFull,
        /// Watching together: everyone Watching.
        Watching,
        /// Mika paused: Mika reads Paused it.
        Paused,
        /// Paused for Ren, loading 6 s.
        WaitLoading,
        /// Paused for Ren at 20 s: Carry on.
        WaitCap,
        /// Mika carried on without Ren, who is still loading (24 s): `carried_on`.
        CarriedOn,
        /// Ada's own player is loading (6 s): "Paused for you", Don't wait for me.
        YouLoading,
        /// Ren buffering 3 s in a room of 3: the room waits.
        Buffering,
        /// Ren buffering in a room of six: no wait, Ren's row says Buffering.
        BufferingSix,
        /// Ren is choosing a source.
        Choosing,
        /// Ren has no source.
        NoSource,
        /// Ren closed the player.
        PlayerClosed,
        /// Ren's device stopped answering.
        Reconnecting,
        /// Ren is 4 s behind.
        Behind,
        /// Ren is on episode 8.
        OtherEpisode,
        /// Ren left: the row stays dimmed for 4 s.
        Left,
        /// Ada is choosing a source.
        YouChoosing,
        /// None of Ada's sources has the episode.
        YouNoSource,
        /// Ada is 4 s behind.
        YouBehind,
        /// No relay answers Ada: Reconnecting to the room.
        YouReconnecting,
        /// Six people; Sora has no source.
        Six,
        /// One of three relays isn't answering.
        RelayTrouble,
        /// Up next counts for everyone: "Up next in 8 s".
        UpNext,
        /// Ada's file reached its end first: "Waiting for Ren to finish".
        UpNextWaiting,
        /// Mika skipped the opening.
        SkippedOpening,
        /// The chat: lines, reactions, and the room's own lines.
        Chat,
        /// Mika moved the room and ticked Ada: the Go along offer (`RoomMoveOffer`).
        MoveOffer,
        /// The old room afterwards, as Ada who stayed sees it: Mika moved, Ren went along
        /// (`moved`).
        MovedAway,
        /// An invite none of whose relays answer: `RoomLeft { Unreachable }`.
        Unreachable,
    }

    /// A room with members the test or the Fixtures app plays, so it can be joined, steered and
    /// left with no network. Testing builds only.
    #[derive(uniffi::Object)]
    pub struct FfiFakeRoom {
        fake: FakeRoom,
        engine: Arc<FfiEngine>,
        handle: Handle,
    }

    #[uniffi::export]
    impl FfiEngine {
        /// An empty room with scripted members on fake relays: rooms this engine starts go there
        /// too, never to a public relay. Testing builds only.
        pub fn start_fake_room(self: Arc<Self>) -> Arc<FfiFakeRoom> {
            let handle = self.runtime_handle();
            let clock: kaeri_room::fake::Clock = Arc::new(kaeri_engine::room::now_ms);
            let fake = {
                let _entered = handle.enter();
                FakeRoom::start(clock)
            };
            Arc::new(FfiFakeRoom { fake, engine: self, handle })
        }
    }

    fn ep(episode: &str) -> Ep {
        Ep(TITLE, episode.to_owned())
    }

    fn reaction(r: Reaction) -> wire::Reaction {
        match r {
            Reaction::Laugh => wire::Reaction::Laugh,
            Reaction::Gasp => wire::Reaction::Gasp,
            Reaction::Clap => wire::Reaction::Clap,
            Reaction::Cry => wire::Reaction::Cry,
        }
    }

    impl FfiFakeRoom {
        fn run<T>(&self, f: impl FnOnce(&FakeRoom) -> T) -> T {
            let _entered = self.handle.enter();
            f(&self.fake)
        }

        fn pause(&self, ms: u64) {
            std::thread::sleep(Duration::from_millis(ms));
        }

        /// Wait up to `secs` for the device to be in the room.
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
            self.engine.send(Command::Room(RoomCommand::Lobby {
                invite: Some(self.invite("Mika".into())),
                name: Some(DEVICE.into()),
                pick: None,
                anyway: true,
            }));
        }
    }

    #[uniffi::export]
    impl FfiFakeRoom {
        /// The room's https invite, as `by` would share it.
        pub fn invite(&self, by: String) -> String {
            RoomShare {
                secret: self.fake.secret(),
                relays: self.fake.relays(),
                by: Some(by),
                title: Some(TITLE_NAME.into()),
                episode: Some(EPISODE.into()),
            }
            .https_link()
        }

        /// The fake relays' addresses.
        pub fn relays(&self) -> Vec<String> {
            self.fake.relays()
        }

        /// `name` comes into the room.
        pub fn add_member(&self, name: String) {
            self.run(|f| f.add_member(&name));
        }

        /// `name` starts the room on `episode` of the scenes' title, playing or waiting for Play
        /// for everyone.
        pub fn member_starts(&self, name: String, episode: String, play: bool) {
            let next = episode.parse::<u32>().ok().map(|n| ep(&(n + 1).to_string()));
            self.run(|f| f.member_starts(&name, ep(&episode), next, play));
        }

        pub fn member_pauses(&self, name: String) {
            self.run(|f| f.member_pauses(&name));
        }

        pub fn member_plays(&self, name: String) {
            self.run(|f| f.member_plays(&name));
        }

        pub fn member_seeks(&self, name: String, to: f64) {
            self.run(|f| f.member_seeks(&name, to));
        }

        pub fn member_skips_opening(&self, name: String, to: f64) {
            self.run(|f| f.member_skips_opening(&name, to));
        }

        pub fn member_carries_on(&self, name: String) {
            self.run(|f| f.member_carries_on(&name));
        }

        /// `name` has been opening the episode for `seconds`; the room waits until
        /// [`Self::member_ready`].
        pub fn member_loads(&self, name: String, seconds: f64) {
            self.run(|f| f.member_loads(&name, seconds));
        }

        pub fn member_ready(&self, name: String) {
            self.run(|f| f.member_ready(&name));
        }

        /// `name` has stalled for `seconds`; in a room of 5 or fewer it holds the room.
        pub fn member_stalls(&self, name: String, seconds: f64) {
            self.run(|f| f.member_stalls(&name, seconds));
        }

        pub fn member_behind(&self, name: String, seconds: f64) {
            self.run(|f| f.member_behind(&name, seconds));
        }

        pub fn member_on_episode(&self, name: String, episode: String) {
            self.run(|f| f.member_on_episode(&name, ep(&episode)));
        }

        pub fn member_chooses_source(&self, name: String) {
            self.run(|f| f.member_chooses_source(&name));
        }

        pub fn member_has_no_source(&self, name: String) {
            self.run(|f| f.member_has_no_source(&name));
        }

        pub fn member_closes_player(&self, name: String) {
            self.run(|f| f.member_closes_player(&name));
        }

        pub fn member_backgrounds(&self, name: String) {
            self.run(|f| f.member_backgrounds(&name));
        }

        /// `name`'s device goes silent: Reconnecting at once, Left after 3 minutes.
        pub fn member_drops(&self, name: String) {
            self.run(|f| f.member_drops(&name));
        }

        pub fn member_returns(&self, name: String) {
            self.run(|f| f.member_returns(&name));
        }

        pub fn member_says(&self, name: String, message: String) {
            self.run(|f| f.member_says(&name, &message));
        }

        pub fn member_reacts(&self, name: String, reaction: Reaction, at: f64) {
            self.run(|f| f.member_reacts(&name, self::reaction(reaction), at));
        }

        pub fn member_leaves(&self, name: String) {
            self.run(|f| f.member_leaves(&name));
        }

        pub fn member_new_invite(&self, name: String) {
            self.run(|f| f.member_new_invite(&name));
        }

        /// `name` moves the room, ticking `take` (names, or "you" for the device). The mover
        /// leaves first.
        pub fn member_moves(&self, name: String, take: Vec<String>) -> String {
            let take: Vec<&str> = take.iter().map(String::as_str).collect();
            self.run(|f| f.member_moves(&name, &take))
        }

        pub fn member_answers_move(&self, name: String, go: bool) {
            self.run(|f| f.member_answers_move(&name, go));
        }

        pub fn member_reaches_up_next(&self, name: String) {
            self.run(|f| f.member_reaches_up_next(&name));
        }

        pub fn member_counts_up_next(&self, name: String) {
            self.run(|f| f.member_counts_up_next(&name));
        }

        pub fn member_cancels_up_next(&self, name: String) {
            self.run(|f| f.member_cancels_up_next(&name));
        }

        /// Relay `index` (0 to 2) stops answering, or answers again.
        pub fn relay_down(&self, index: u32, down: bool) {
            self.run(|f| f.relay_down(index as usize, down));
        }

        /// Relay `index` refuses this device's events with `reason` (`rate-limited:`, `blocked:`).
        pub fn relay_refuses(&self, index: u32, reason: String) {
            self.run(|f| f.relay_refuses(index as usize, &reason));
        }

        /// The room takes chat and sends it to no one: a send is never echoed.
        pub fn swallow_chat(&self, swallow: bool) {
            self.run(|f| f.swallow_chat(swallow));
        }

        /// Stage one of the board's frames, joining the device to the room as Ada. Blocks for
        /// up to a few seconds: call it off the main thread. `false` when the device never got
        /// into the room.
        pub fn stage(&self, scene: FfiRoomScene) -> bool {
            use FfiRoomScene as S;
            let six = matches!(scene, S::BufferingSix | S::Six);
            let full = scene == S::InvitedFull;
            let mut names = vec!["Mika", "Ren"];
            if six {
                names.extend(["Sora", "Kenji", "Noa"]);
            }
            if full {
                names = vec![
                    "Mika", "Ren", "Sora", "Kenji", "Noa", "Yui", "Taro", "Hana", "Kai", "Rio",
                ];
            }
            if matches!(scene, S::MovedAway) {
                names.push("Taro");
            }
            for name in &names {
                self.run(|f| f.add_member(name));
            }
            let before_play = matches!(
                scene,
                S::BeforePlay | S::BeforePlayWait | S::BeforePlayCap | S::Starting
            );
            if before_play {
                self.run(|f| f.member_starts("Mika", ep(EPISODE), Some(ep(NEXT)), false));
            } else {
                // A room that has been watching a while: no count before play, so a pause, a
                // seek or a stall reads as one, not as "Starting when Ren is ready".
                let at = super::scene::at(0.2);
                // Your own No source and Choosing: the room watches an episode that hasn't aired,
                // so the app's own play of it fails as a real one would.
                let (room_ep, next) = if matches!(scene, S::YouNoSource | S::YouChoosing) {
                    (Ep(UNAIRED.0, UNAIRED.1.to_owned()), None)
                } else {
                    (ep(EPISODE), Some(ep(NEXT)))
                };
                self.run(|f| f.member_started("Mika", room_ep, next, at));
            }
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
                self.engine.send(Command::Room(RoomCommand::PeekInvite {
                    invite: self.invite("Mika".into()),
                }));
                return true;
            }
            self.join();
            if !self.wait_joined(8) {
                return false;
            }
            // Ada's player is ready (the app's own player takes over when it opens).
            let ready = |f: &FakeRoom| f.pose_you(StateCode::Watching, 0.0, None, None, false);
            self.run(ready);
            self.pause(200);
            match scene {
                S::BeforePlay => {}
                S::BeforePlayWait | S::BeforePlayCap => self.run(|f| f.member_plays("Mika")),
                S::Starting => self.run(|f| f.member_plays("Mika")),
                S::Invited | S::InvitedFull | S::Unreachable => {}
                S::Watching => {}
                S::Paused => self.run(|f| f.member_pauses("Mika")),
                S::WaitLoading | S::WaitCap | S::CarriedOn => {
                    self.run(|f| f.member_seeks("Mika", super::scene::SEEK_TO));
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
                    self.run(|f| f.member_seeks("Mika", super::scene::SEEK_TO));
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
                    self.run(|f| f.pose_you(StateCode::Choosing, 0.0, None, None, false))
                }
                S::YouNoSource => {
                    self.run(|f| f.pose_you(StateCode::NoSource, 0.0, None, None, false))
                }
                S::YouBehind => {
                    self.run(|f| f.pose_you(StateCode::Behind, 4.0, None, Some(4.0), false))
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
                S::SkippedOpening => {
                    self.run(|f| f.member_skips_opening("Mika", super::scene::OPENING_ENDS));
                }
                S::Chat => self.run(|f| {
                    f.member_says("Mika", "ok starting");
                    f.member_reacts("Sora", wire::Reaction::Clap, super::scene::at(0.15));
                    f.member_says("Ren", "the handoff in the rain, I knew it");
                    f.member_reacts("Ren", wire::Reaction::Gasp, super::scene::at(0.4));
                    f.member_says("Mika", "this part again");
                    f.member_reacts("Ren", wire::Reaction::Laugh, super::scene::at(0.65));
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

        /// Settle the members' rooms on the clock.
        pub fn tick(&self) {
            self.run(FakeRoom::tick);
        }
    }
}
