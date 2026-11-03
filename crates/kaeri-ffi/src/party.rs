//! Watch parties, for a front end: the invite link a party is shared by, read and written by
//! the model's rules so every app makes and opens the same links, and, in a testing build, a
//! Syncplay server on the loopback interface for the Fixtures apps to hold a party on offline.
//!
//! The party itself is the engine's, through `Command::PartyJoin` and the `Party*` events.

use kaeri_model::party::{PartyInvite, PartyShare, PartyTarget};

/// The link that invites someone to `invite`'s room, to play its episode there:
/// `kaeri://party?server=…&room=…&title=…&episode=…`.
#[uniffi::export]
pub fn party_invite_link(invite: PartyInvite) -> String {
    invite.link()
}

/// The invite a link opened carries, or `None` for a link that is not a whole one.
#[uniffi::export]
pub fn parse_party_invite(link: String) -> Option<PartyInvite> {
    PartyInvite::parse(&link)
}

/// The https invite for a room, `https://kaeri.app/party#v=2&…`, which every app shares.
#[uniffi::export]
pub fn party_share_link(share: PartyShare) -> String {
    share.https_link()
}

/// The `kaeri://party#…` form of the same invite, for a device with the app.
#[uniffi::export]
pub fn party_share_app_link(share: PartyShare) -> String {
    share.app_link()
}

/// What pasted text or an opened link names: an https invite, an `kaeri://` link or a
/// room code. `None` for anything else.
#[uniffi::export]
pub fn parse_party_target(text: String) -> Option<PartyTarget> {
    PartyTarget::parse(&text)
}

/// A room code tidied as typed (case, spaces, dashes), or `None` when it is not one.
#[uniffi::export]
pub fn clean_party_code(text: String) -> Option<String> {
    kaeri_model::party::clean_party_code(&text)
}

#[cfg(all(feature = "party", feature = "testing"))]
pub use fake::{FfiChatFixture, FfiFakeParty};

#[cfg(all(feature = "party", feature = "testing"))]
mod fake {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex, PoisonError},
    };

    use kaeri_model::{Msg, party::Reaction};
    use kaeri_party::{
        fake::{FakeMember, FakeOptions, FakeServer},
        reaction,
    };
    use tokio::runtime::Handle;

    use crate::{engine::FfiEngine, start::FfiError};

    /// A Syncplay server on 127.0.0.1 with members the test or the Fixtures app plays, so a
    /// party can be joined, steered and left with no network. Testing builds only.
    #[derive(uniffi::Object)]
    pub struct FfiFakeParty {
        inner: Mutex<Option<Inner>>,
        handle: Handle,
    }

    struct Inner {
        server: FakeServer,
        members: HashMap<String, FakeMember>,
    }

    /// The chat states the Fixtures apps take screenshots of, from the watch together chat board.
    /// The members are Mika, Ren and Sol, added to the room when they are not there; Noa is the
    /// device. Testing builds only.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
    pub enum FfiChatFixture {
        /// Chat on and working, as a server is: undoes the states below for the next join.
        Quiet,
        /// Playing: Mika "ok starting", Ren "the handoff in the rain, I knew it", Sol "wait, who
        /// is the new runner" and Mika "this part again", and reactions: Sol claps at 3:24, Ren
        /// gasps at 7:48, Ren and Mika laugh at 12:04.
        Conversation,
        /// Six lines at once, for the fold line: "3 more in chat".
        Burst,
        /// The lobby's conversation: "snacks ready?", "two minutes, making tea", and a line
        /// with a link written as plain text.
        Lobby,
        /// A server with chat turned off. Set before joining: the welcome says it.
        ChatOff,
        /// A server that takes chat and sends it to no one, so a send is never echoed and, five
        /// seconds on, is "Couldn't send".
        NotSent,
        /// A server that takes messages of 40 characters, for the count at 20 left. Set before
        /// joining.
        ShortMessages,
    }

    #[uniffi::export]
    impl FfiEngine {
        /// A party server for the Fixtures app's watch party: join it with
        /// `Command::PartyJoin` and its `server()`. Testing builds only.
        pub fn start_fake_party(&self) -> Result<Arc<FfiFakeParty>, FfiError> {
            let handle = self.runtime_handle();
            let server = handle
                .block_on(handle.spawn(FakeServer::start(FakeOptions::default())))
                .map_err(|e| FfiError::engine(Msg::StartFailed { reason: e.to_string() }))?
                .map_err(|e| FfiError::engine(Msg::StartFailed { reason: e.to_string() }))?;
            let inner = Inner { server, members: HashMap::new() };
            Ok(Arc::new(FfiFakeParty { inner: Mutex::new(Some(inner)), handle }))
        }
    }

    impl FfiFakeParty {
        fn with<T>(&self, read: impl FnOnce(&mut Inner) -> T) -> Option<T> {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            let _entered = self.handle.enter();
            inner.as_mut().map(read)
        }

        fn member(&self, name: &str, act: impl FnOnce(&FakeMember)) {
            self.with(|inner| inner.members.get(name).map(act));
        }
    }

    #[uniffi::export]
    impl FfiFakeParty {
        /// Where to join it, as `host:port`.
        pub fn server(&self) -> String {
            self.with(|inner| inner.server.address()).unwrap_or_default()
        }

        /// `name` comes into `room`, played by the server.
        pub fn add_member(&self, room: String, name: String) {
            self.with(|inner| {
                let member = inner.server.member(&room, &name);
                inner.members.insert(name, member);
            });
        }

        /// The member opens a file, `duration` seconds long.
        pub fn member_opens(&self, name: String, file: String, duration: f64) {
            self.member(&name, |member| member.open(&file, duration));
        }

        pub fn member_ready(&self, name: String, ready: bool) {
            self.member(&name, |member| member.ready(ready));
        }

        /// The member sets the room's playlist to `files`, current entry first.
        pub fn member_sets_playlist(&self, name: String, files: Vec<String>) {
            self.member(&name, |member| {
                let refs: Vec<&str> = files.iter().map(String::as_str).collect();
                member.set_playlist(&refs);
            });
        }

        /// The member moves the room to playlist entry `index`.
        pub fn member_sets_index(&self, name: String, index: u32) {
            self.member(&name, |member| member.set_index(index as usize));
        }

        pub fn member_pauses(&self, name: String) {
            self.member(&name, FakeMember::pause);
        }

        pub fn member_plays(&self, name: String) {
            self.member(&name, FakeMember::play);
        }

        /// The member seeks to `to` seconds.
        pub fn member_seeks(&self, name: String, to: f64) {
            self.member(&name, |member| member.seek(to));
        }

        pub fn member_says(&self, name: String, message: String) {
            self.member(&name, |member| member.chat(&message));
        }

        /// The member reacts at `at` seconds, as a Syncplay line.
        pub fn member_reacts(&self, name: String, reaction: Reaction, at: f64) {
            let kind = match reaction {
                Reaction::Laugh => kaeri_party::Reaction::Laugh,
                Reaction::Gasp => kaeri_party::Reaction::Gasp,
                Reaction::Clap => kaeri_party::Reaction::Clap,
                Reaction::Cry => kaeri_party::Reaction::Cry,
            };
            self.member(&name, |member| member.chat(&reaction::encode(kind, at)));
        }

        /// The server turns chat on or off, and announces `max_length` characters per message,
        /// for members who join after this.
        pub fn set_chat(&self, on: bool, max_length: Option<u32>) {
            self.with(|inner| inner.server.set_chat(on, max_length));
        }

        /// The server takes chat and sends it to no one: a sender never sees its line come back.
        pub fn swallow_chat(&self, swallow: bool) {
            self.with(|inner| inner.server.swallow_chat(swallow));
        }

        /// The chat lines the server took from the device, as `name: text`.
        pub fn chat_taken(&self) -> Vec<String> {
            self.with(|inner| {
                inner
                    .server
                    .said()
                    .into_iter()
                    .map(|(who, text)| format!("{who}: {text}"))
                    .collect()
            })
            .unwrap_or_default()
        }

        /// Put the chat in one of the board's states: see [`FfiChatFixture`]. Mika, Ren and Sol
        /// come into `room` when they are not there.
        pub fn chat_fixture(&self, room: String, state: FfiChatFixture) {
            self.with(|inner| {
                inner.server.set_chat(true, None);
                inner.server.swallow_chat(false);
            });
            for name in ["Mika", "Ren", "Sol"] {
                if self.with(|inner| inner.members.contains_key(name)) != Some(true) {
                    self.add_member(room.clone(), name.into());
                }
            }
            let say = |who: &str, text: &str| self.member_says(who.into(), text.into());
            let react = |who: &str, kind: Reaction, at: f64| {
                self.member_reacts(who.into(), kind, at);
            };
            match state {
                FfiChatFixture::Quiet => {}
                FfiChatFixture::Conversation => {
                    say("Mika", "ok starting");
                    react("Sol", Reaction::Clap, 204.0);
                    say("Ren", "the handoff in the rain, I knew it");
                    react("Ren", Reaction::Gasp, 468.0);
                    say("Sol", "wait, who is the new runner");
                    react("Ren", Reaction::Laugh, 724.0);
                    react("Mika", Reaction::Laugh, 724.0);
                    say("Mika", "this part again");
                }
                FfiChatFixture::Burst => {
                    for (who, text) in [
                        ("Mika", "ok starting"),
                        ("Ren", "ready"),
                        ("Sol", "one sec"),
                        ("Mika", "this part again"),
                        ("Ren", "the handoff in the rain, I knew it"),
                        ("Sol", "wait, who is the new runner"),
                    ] {
                        say(who, text);
                    }
                }
                FfiChatFixture::Lobby => {
                    say("Mika", "snacks ready?");
                    say("Ren", "two minutes, making tea");
                    say("Sol", "the soundtrack is at music.example/ost");
                }
                FfiChatFixture::ChatOff => self.set_chat(false, None),
                FfiChatFixture::NotSent => self.swallow_chat(true),
                FfiChatFixture::ShortMessages => self.set_chat(true, Some(40)),
            }
        }

        pub fn member_leaves(&self, name: String) {
            self.with(|inner| inner.members.remove(&name).map(|member| member.leave()));
        }

        /// The server goes away, as one lost does: every connection dropped and none taken.
        pub fn go_down(&self, down: bool) {
            self.with(|inner| {
                inner.server.set_down(down);
                if down {
                    inner.server.drop_connections();
                }
            });
        }

        pub fn stop(&self) {
            let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner).take();
            let _entered = self.handle.enter();
            drop(inner);
        }
    }
}
