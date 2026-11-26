//! The room: watching together as a front end shows it. The video, the people and the chat in one
//! place, end to end encrypted over public Nostr relays, with no host. The protocol is
//! `kaeri-room`'s, written up in `docs/room-protocol.md`.
//!
//! Each viewer plays from their own sources: the room carries what is playing (an AniList id and
//! an episode), never where from.
//!
//! Times sent as `*_ms` are Unix epoch milliseconds on this device's clock. An app shows seconds
//! as `now − since_ms` on its own one-second tick; the engine never sends an event per second.

use kaeri_core::ids::AnilistId;

pub use crate::party::{NoSourceWhy, Reaction};

/// The relays a new room uses when Settings has none of its own: public relays run by others.
/// Kaeri runs none of them.
pub const DEFAULT_RELAYS: [&str; 3] =
    ["wss://nos.lol", "wss://relay.damus.io", "wss://relay.primal.net"];

/// The relays a room swaps in, in this order, when one of its own stops answering.
pub const SPARE_RELAYS: [&str; 3] =
    ["wss://nostr.mom", "wss://relay.nostr.net", "wss://offchain.pub"];

/// Rooms work best with this many people or fewer. The cap is soft: a newcomer is warned and may
/// join anyway.
pub const ROOM_CAP: u32 = 10;

/// From how long a wait has run, in milliseconds, Carry on comes forward.
pub const CARRY_ON_AFTER_MS: i64 = 20_000;

/// The most characters in one chat message.
pub const CHAT_MAX_LENGTH: u32 = 500;

/// The room this device is in, as it stands: sent whole with
/// [`Event::RoomJoined`](crate::event::Event::RoomJoined) and
/// [`Event::RoomState`](crate::event::Event::RoomState) after any change.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomView {
    /// The room's id on this device, for [`RoomCommand::ForgetRoom`] and
    /// [`RoomCommand::LookIn`]. Never sent anywhere.
    pub room_id: String,
    /// This device's name in the room: the one asked for, or "Ren 2" when that was taken.
    pub name: String,
    pub connection: RoomConnection,
    /// The episode this device plays for the room, when it does.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episode: Option<RoomEpisode>,
    /// What the room watches; `None` only while joining.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub room_episode: Option<RoomEpisodeInfo>,
    /// What plays next, as the room agreed it: Up next goes there.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_episode: Option<RoomEpisodeInfo>,
    /// The https invite, `https://kaeri.app/room#v=3&k=…`. After New invite, the new one, once
    /// the rotation settled.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub link: Option<String>,
    /// The room plays now: its intent is play and nobody holds it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub playing: bool,
    /// The room hasn't played this episode yet: the video area shows the episode's banner with
    /// Play for everyone, and people read Ready once their first frame is ready.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub before_play: bool,
    /// Before play, everyone is ready and play starts at this moment: "Starting in 3".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub starts_at_ms: Option<i64>,
    /// Who paused the room, when a person did ("Mika paused"); "You" is the app's word for
    /// [`RoomView::paused_by_you`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub paused_by: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub paused_by_you: bool,
    /// Where the room is, in seconds into the episode, at [`RoomView::position_at_ms`]: moved on
    /// by the time since while [`RoomView::playing`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub position: Option<f64>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub position_at_ms: i64,
    /// Who the room waits for, the first to start waiting first: "Paused for Ren". Empty when
    /// nobody holds it. The line shows a wait once it passes 1 s
    /// ([`RoomView::wait_shown_at_ms`]), so a fast seek never flashes one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub waiting: Vec<PausedFor>,
    /// When the wait may show: the first waited member's start plus one second.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub wait_shown_at_ms: Option<i64>,
    /// When Carry on comes forward: the oldest waited member's start plus 20 s. Play during a
    /// wait is Carry on at any time.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub carry_on_at_ms: Option<i64>,
    /// Carry on is offered now (20 s have passed). Said again when it turns.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub can_carry_on: bool,
    /// This device is waited for: Don't wait for me is offered, from second 0.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub can_dont_wait: bool,
    /// Someone carried on, and those carried past are still loading: "Mika carried on · Ren joins
    /// when ready". Gone once they have joined.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub carried_on: Option<RoomCarriedOn>,
    /// Up next, once its time has come on every device.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub up_next: Option<RoomUpNext>,
    /// Whether this device's own sources have the room's episode. `None` until checked.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub sources: Option<RoomSources>,
    /// The room's relays and how each answers here, for Details and the relay trouble line.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub relays: Vec<RelayHealth>,
    /// Still reading the room: the sync answer and the episode's title.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub looking: bool,
    /// Move to a new room's list: everyone present but this device, ticked unless muted.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub move_candidates: Vec<MoveCandidate>,
    /// Someone moved this room to a new one: "Mika moved to a new room · Ada and Ren went
    /// along · this room carries on". For whoever stayed.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub moved: Option<RoomMoved>,
    /// The room's relay round trip here, in milliseconds, once measured.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub latency_ms: Option<u32>,
}

/// A member the room waits for.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PausedFor {
    /// The member's id, as [`RoomMember::id`].
    pub id: String,
    pub name: String,
    /// This device: "Paused for you".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub you: bool,
    pub reason: WaitReason,
    /// When this member started loading or stalling.
    pub since_ms: i64,
}

/// Why the room waits for someone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum WaitReason {
    /// Opening the episode.
    Loading,
    /// Catching up after a seek or Skip opening.
    Seeking,
    /// Stalled mid-episode, in a room of 5 or fewer.
    Buffering,
}

/// Someone carried on past those still loading.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomCarriedOn {
    /// Who pressed it.
    pub by: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub by_you: bool,
    /// Those carried past who haven't joined yet.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub past: Vec<String>,
}

/// The room's Up next.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomUpNext {
    pub episode: RoomEpisodeInfo,
    /// When the count ends and the next episode plays for everyone. `None` while the card waits
    /// for a longer file: "Waiting for Ren to finish".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub ends_at_ms: Option<i64>,
    /// Whose file is still playing, while the count waits.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub waiting_for: Vec<String>,
    /// Somebody pressed Cancel: the card goes, "Mika cancelled Up next".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cancelled_by: Option<String>,
}

/// One of the room's relays, as this device finds it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RelayHealth {
    /// `wss://…`
    pub url: String,
    pub healthy: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rtt_ms: Option<u32>,
}

/// A row in Move to a new room's list.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MoveCandidate {
    pub id: String,
    pub name: String,
    /// Muted on this device: starts unticked.
    pub muted: bool,
    /// Starts ticked.
    pub ticked: bool,
}

/// This room was moved to a new one, as those who stayed see it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomMoved {
    pub by: String,
    /// Who went along so far.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub went: Vec<String>,
}

/// An offer to go along to a new room: "Mika moved the room. Go along?" Answer it with
/// [`RoomCommand::AnswerMove`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomMoveOffer {
    pub id: String,
    pub by: String,
    /// Who else was ticked.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub names: Vec<String>,
    pub title: String,
    pub episode: String,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RoomConnection {
    /// Finding the room on its relays.
    #[default]
    Joining,
    Connected,
    /// No relay answers; each is tried again on its own, for as long as the device is in the
    /// room. The episode keeps playing.
    Reconnecting {
        relays_up: u32,
    },
}

/// A title's episode, as the room plays it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomEpisode {
    pub id: AnilistId,
    pub episode: String,
}

/// What the room watches, with its words.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomEpisodeInfo {
    pub id: AnilistId,
    pub episode: String,
    /// The readable title, from this device's metadata, or the invite's while it loads.
    pub title: String,
    /// The episode's own name, when known: "The Long Leg".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub name: Option<String>,
    /// The title's banner, for the room before play.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub banner_url: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
}

/// Whether this device's sources have the room's episode.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RoomSources {
    Checking,
    Found,
    /// The kind, and the lookup's own words (kept on this device, never sent).
    Missing {
        why: NoSourceWhy,
        reason: crate::Msg,
    },
}

/// One person in the room.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomMember {
    /// A stable id for the person in this room (a rejoin is the same person).
    pub id: String,
    pub name: String,
    /// This device.
    pub you: bool,
    pub state: MemberState,
    /// When the state began, for the seconds Loading and Buffering count.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub since_ms: Option<i64>,
    /// How far behind the room, in seconds, while [`MemberState::Behind`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub behind_s: Option<f32>,
    /// The episode they have open, when it isn't the room's ([`MemberState::OtherEpisode`]).
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episode: Option<RoomEpisode>,
    /// They paused the room: the row reads Paused it, with the badge.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub paused_room: bool,
    /// The room waits for them: the ring is an arc filling toward 20 s.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub waited: bool,
    /// Their app is in the background.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub background: bool,
    /// Muted on this device.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub muted: bool,
}

/// One word for each person, with its ring. What shows on a row, highest first: Left >
/// Reconnecting > their own reported state > Behind > Paused > Watching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum MemberState {
    /// The same episode, ready, with the room.
    Watching,
    /// Before play: their first frame is ready.
    Ready,
    /// Was watching, and the room is paused ([`RoomMember::paused_room`] when they did it).
    Paused,
    /// Opening the episode; seconds from `since_ms`.
    Loading,
    /// Stalled for more than 1.5 s mid-episode.
    Buffering,
    /// More than 3 s behind the room for 4 s; [`RoomMember::behind_s`].
    Behind,
    /// On another episode than the room's; [`RoomMember::episode`].
    OtherEpisode,
    /// Picking which of their sources to play from.
    Choosing,
    /// None of their sources lists the episode.
    NoSource,
    /// In the room with the player closed, or the app in the background.
    PlayerClosed,
    /// No word from their device for 40 s (75 s in the background).
    Reconnecting,
    /// Gone. The row stays, dimmed, for 4 s.
    Left,
}

/// Why a room opens a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RoomStartKind {
    /// The room's episode, opened paused at its start: the room plays when everyone is ready.
    Start,
    /// Up next moved the room on.
    MoveOn,
    /// Coming in at the room's position: a late join, Open the player, or after Carry on.
    Rejoin,
}

/// A look into a room before joining (an invite opened) or a recent room's look-in.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomGlimpse {
    /// For a recent room; `None` for an invite.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub room_id: Option<String>,
    /// Who invited, from the link.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub by: Option<String>,
    /// Who is there. Empty when nobody answered: "Nobody answered in this room".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub members: Vec<RoomMember>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub room_episode: Option<RoomEpisodeInfo>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub playing: bool,
    /// [`ROOM_CAP`] people or more are there: "Join anyway".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub full: bool,
}

/// A room kept on this device: the last ten, newest first. Rejoin works without asking anyone.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomRecent {
    pub room_id: String,
    pub id: AnilistId,
    pub title: String,
    /// The last episode the room reached.
    pub episode: String,
    /// Who was there, other than this device.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub names: Vec<String>,
    /// Seconds since the epoch.
    pub last_at: i64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
}

/// Settings ▸ Relays: the list for new rooms.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomRelaySettings {
    /// [`DEFAULT_RELAYS`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub defaults: Vec<String>,
    /// [`SPARE_RELAYS`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub spares: Vec<String>,
    /// The list set in Settings; empty means the defaults.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chosen: Vec<String>,
}

/// Why this device is no longer in the room.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RoomLeftWhy {
    /// Left on purpose.
    Left,
    /// Went along to a new room: [`Event::RoomJoined`](crate::event::Event::RoomJoined) follows
    /// for it.
    Moved,
    /// None of the link's relays answered: "Couldn't reach this room's relays. Ask for a new
    /// invite."
    Unreachable,
    /// [`ROOM_CAP`] people or more are there and the join didn't say `anyway`: "This room is
    /// full".
    Full,
    /// An invite from an older version: "This invite is from an older version of Kaeri. Ask for
    /// a new one."
    OldInvite,
    /// Anything else, worded.
    Failed { reason: crate::Msg },
}

/// What a conversation line holds.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RoomChatBody {
    /// Something typed, as written, on one line. Shown as text, links included, never tappable.
    Said { text: String },
    /// A reaction at `at` seconds into the episode; `with` names the others who made the same
    /// one at the same second.
    Reacted { reaction: Reaction, at: f64, with: Vec<String> },
    /// The room said something: "Mika paused at 12:04", "Mika carried on without Ren".
    Room { message: RoomMessage },
}

/// One line of the room's conversation.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomChatLine {
    /// Counts up from 1 through the room; a line is the same line in every snapshot.
    pub id: u64,
    /// Who; empty for the room's own lines.
    pub from: String,
    pub you: bool,
    pub body: RoomChatBody,
    /// `from` is muted here: the app shows "Ren is muted" in its place.
    pub muted: bool,
}

/// Where somebody reacted, for a tick on the seek bar.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomReactionMark {
    pub at: f64,
    pub from: String,
    pub reaction: Reaction,
}

/// The room's conversation, whole, after every change. In memory for the room only.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomConversation {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub lines: Vec<RoomChatLine>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub marks: Vec<RoomReactionMark>,
    /// A line no relay echoed within five seconds: "Couldn't send" with Try again.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub failed: Option<String>,
}

/// What this device may do in the conversation now.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomChatState {
    /// [`CHAT_MAX_LENGTH`]. The count shows at 20 left.
    pub max_length: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub muted: Vec<String>,
    /// Whole seconds Send waits: three messages in five seconds is the most.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub wait_seconds: u32,
    /// Whole seconds before the next reaction: one per two seconds.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub react_wait_seconds: u32,
    /// The terminal's Hide chat.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub hidden: bool,
}

/// A reaction coming in, for the moment on the video.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomReaction {
    pub from: String,
    pub you: bool,
    pub reaction: Reaction,
    pub at: f64,
}

/// Why a send did not go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RoomChatRefusal {
    /// Too many lately: wait this many whole seconds.
    Wait { seconds: u32 },
    /// A reaction needs the episode playing on this device.
    NotPlaying,
}

/// Something the room said: a line in the chat, and a notice on the player.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomMessage {
    pub kind: RoomMessageKind,
    /// The line, worded in the app's language: "Mika paused at 12:04".
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RoomMessageKind {
    /// "Mika started the room · Episode 9".
    Started {
        by: String,
        episode: String,
    },
    Joined {
        name: String,
    },
    Left {
        name: String,
    },
    /// "Mika paused at 12:04".
    Paused {
        by: String,
        at: f64,
    },
    Resumed {
        by: String,
    },
    /// "Mika jumped from 3:10 to 5:00".
    Seeked {
        by: String,
        from: f64,
        to: f64,
    },
    /// This player was moved to stay with the room.
    Corrected,
    EpisodeChanged {
        by: String,
        episode: String,
    },
    /// Someone who was behind joined the room's place.
    JoinedAt {
        name: String,
    },
    /// "Ada already paused": a play pressed at the same moment lost.
    AlreadyPaused {
        by: String,
    },
    /// "Paused for Ren · loading".
    WaitingFor {
        names: Vec<String>,
        reason: WaitReason,
    },
    /// "Mika carried on without Ren".
    CarriedOn {
        by: String,
        past: Vec<String>,
    },
    SkippedOpening {
        by: String,
    },
    /// "Mika made a new invite".
    NewInvite {
        by: String,
    },
    /// "Up next: episode 10 for everyone".
    UpNext {
        episode: String,
    },
    UpNextCancelled {
        by: String,
    },
    /// "Mika moved to a new room".
    MovedRoom {
        by: String,
    },
    /// "Ada went to the new room".
    WentAlong {
        name: String,
    },
    /// A name was taken: "You're Ren 2 in this room".
    Renamed {
        name: String,
    },
}

/// What the room asks of the engine, sent as
/// [`Command::Room`](crate::command::Command::Room).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RoomCommand {
    /// Join a room, or start one. With an `invite` link, join that room; without, start a new
    /// room (a new secret, on Settings' relays) watching `pick`. Starting from the player (the
    /// episode playing is `pick`) keeps it playing from where it is: the player becomes the
    /// room. `anyway` joins a room of [`ROOM_CAP`] or more. Answered with
    /// [`Event::RoomJoined`](crate::event::Event::RoomJoined), then `RoomState` and
    /// `RoomMembers` as the room changes, or `RoomLeft` with why.
    Lobby {
        #[cfg_attr(feature = "ffi", uniffi(default))]
        invite: Option<String>,
        /// The settings' name when `None`; the first name given is kept there.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        name: Option<String>,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        pick: Option<RoomEpisode>,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        anyway: bool,
    },
    /// Rejoin a recent room by its id.
    Rejoin {
        room_id: String,
    },
    /// Leave the room. What is playing goes on as a private session. Always asked first by the
    /// app.
    Leave,
    /// Play or pause the room. Play during a wait is Carry on; Play before play is Play for
    /// everyone.
    Play {
        play: bool,
    },
    /// Set the room's episode: everyone opens it and the room waits for them.
    ChangeEpisode {
        id: AnilistId,
        episode: String,
    },
    /// Carry on without those the room waits for.
    CarryOn,
    /// This device is loading and the room shouldn't wait for it.
    DontWait,
    /// Skip the opening for everyone, to `to` seconds (this device's own skip data).
    SkipOpening {
        to: f64,
    },
    /// Up next's Cancel, for everyone.
    CancelUpNext,
    /// Up next's Play next now: the room moves on at once.
    PlayNextNow,
    /// The source picker for the room's episode opened or closed: "Choosing a source".
    ChoosingSource {
        open: bool,
    },
    /// Close the player but keep the room (⌘W ▸ Keep the room, Back): "Player closed".
    ClosePlayer,
    /// Open the player again, at the room's place.
    OpenPlayer,
    /// Rotate the room's key: links nobody has used stop working. Answered with `RoomState`
    /// carrying the new `link`.
    NewInvite,
    /// Move to a new room with these member ids (at least one). Answered with `RoomJoined` for
    /// the new room; this one is left.
    MoveRoom {
        take: Vec<String>,
    },
    /// Answer a [`RoomMoveOffer`]: Go along (`go`) joins the new room and leaves this one; Stay
    /// drops the offer.
    AnswerMove {
        offer: String,
        go: bool,
    },
    /// This room's relays, for everyone in it (Details ▸ Edit relays). New invites carry them.
    EditRelays {
        relays: Vec<String>,
    },
    /// Settings ▸ Relays: the list for new rooms; empty for the defaults. Answered with
    /// `Event::RoomRelays`.
    SetRelays {
        relays: Vec<String>,
    },
    /// Ask for Settings ▸ Relays: answered with `Event::RoomRelays`.
    LoadRelays,
    /// Say something to the room. The line shows when a relay echoes it; with none in five
    /// seconds `RoomChat` carries it in `failed`.
    Say {
        text: String,
    },
    /// React at this device's playhead.
    React {
        reaction: Reaction,
    },
    /// Hide a member's lines and reactions on this device, or show them again.
    Mute {
        id: String,
        muted: bool,
    },
    /// The terminal's Hide chat.
    HideChat {
        hidden: bool,
    },
    /// The last ten rooms: answered with `Event::RoomRecent`.
    LoadRecent,
    ForgetRoom {
        room_id: String,
    },
    /// Look into a recent room without anyone seeing: answered with `Event::RoomGlimpse` or
    /// `Event::RoomLookFailed`.
    LookIn {
        room_id: String,
    },
    /// Look into an invite's room before joining ("Mika invited you"): answered with
    /// `Event::RoomGlimpse` or `Event::RoomLeft`.
    PeekInvite {
        invite: String,
    },
}

/// What a pasted link or an opened one leads to. See `parse_room_target`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RoomTarget {
    /// The invite, as the engine joins it.
    pub link: String,
    /// The room's relays when the link names its own.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub relays: Vec<String>,
    /// Who invited; only to say so.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub by: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episode: Option<String>,
    /// An invite from an older version: it can't be joined.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub old: Option<OldInvite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum OldInvite {
    /// A Syncplay room's link (`v=2`, or `kaeri://party?server=…`).
    SyncplayLink,
    /// A typed room code.
    Code,
}

/// What someone shares: the room's secret and, for the web page only, who invited and what the
/// room watches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomShare {
    /// 16 bytes.
    pub secret: [u8; 16],
    /// Only when they aren't the built-in defaults.
    pub relays: Vec<String>,
    pub by: Option<String>,
    pub title: Option<String>,
    pub episode: Option<String>,
}

/// The web page every invite points at.
const WEB_ROOM: &str = "https://kaeri.app/room";
/// The app's own scheme for the same, which the web page opens.
const APP_ROOM: &str = "kaeri://room";
/// Where the watch party's invites pointed: any link there is from an older version.
const OLD_LINKS: [&str; 3] =
    ["https://kaeri.app/party", "https://www.kaeri.app/party", "kaeri://party"];

impl RoomShare {
    fn fragment(&self) -> String {
        let mut out = format!("v=3&k={}", base64url(&self.secret));
        for (key, value) in [("by", &self.by), ("n", &self.title), ("e", &self.episode)] {
            if let Some(value) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                out.push_str(&format!("&{key}={}", encode(value)));
            }
        }
        if !self.relays.is_empty() {
            let list: Vec<String> = self.relays.iter().map(|r| encode(r)).collect();
            out.push_str(&format!("&rl={}", list.join(",")));
        }
        out
    }

    /// `https://kaeri.app/room#v=3&k=…&by=…&n=…&e=…[&rl=…]`. The fragment never reaches a
    /// server.
    pub fn https_link(&self) -> String {
        format!("{WEB_ROOM}#{}", self.fragment())
    }

    pub fn app_link(&self) -> String {
        format!("{APP_ROOM}#{}", self.fragment())
    }

    /// Read an invite: `Ok(None)` for text that isn't one, `Err(old)` for an older version's.
    pub fn parse(text: &str) -> Result<Option<Self>, OldInvite> {
        let text = text.trim();
        if OLD_LINKS.iter().any(|old| strip_prefix_ignore_case(text, old).is_some()) {
            return Err(OldInvite::SyncplayLink);
        }
        let fragment = strip_prefix_ignore_case(text, WEB_ROOM)
            .or_else(|| strip_prefix_ignore_case(text, "https://www.kaeri.app/room"))
            .or_else(|| strip_prefix_ignore_case(text, APP_ROOM))
            .map(|rest| rest.strip_prefix('/').unwrap_or(rest));
        let Some(rest) = fragment else {
            return if crate::party::clean_party_code(text).is_some() {
                Err(OldInvite::Code)
            } else {
                Ok(None)
            };
        };
        if rest.starts_with('?') {
            return Err(OldInvite::SyncplayLink);
        }
        let Some(fragment) = rest.strip_prefix('#') else { return Ok(None) };
        let mut fields = std::collections::HashMap::new();
        for pair in fragment.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            fields.insert(key, value);
        }
        match fields.get("v").copied() {
            Some("3") => {}
            Some(_) => return Err(OldInvite::SyncplayLink),
            None if fields.contains_key("s") => return Err(OldInvite::SyncplayLink),
            None => return Ok(None),
        }
        let Some(secret) = fields.get("k").and_then(|k| unbase64url(k)) else {
            return Ok(None);
        };
        let Ok(secret) = <[u8; 16]>::try_from(secret.as_slice()) else { return Ok(None) };
        let text = |key: &str| {
            fields
                .get(key)
                .and_then(|v| decode(v))
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        };
        let relays = fields
            .get("rl")
            .map(|list| {
                list.split(',')
                    .filter_map(decode)
                    .map(|r| r.trim().to_owned())
                    .filter(|r| valid_relay(r))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Some(Self { secret, relays, by: text("by"), title: text("n"), episode: text("e") }))
    }
}

impl RoomTarget {
    /// Read what a person pasted or opened. `None` for anything that isn't an invite.
    pub fn parse(text: &str) -> Option<Self> {
        match RoomShare::parse(text) {
            Ok(Some(share)) => Some(Self {
                link: share.https_link(),
                relays: share.relays,
                by: share.by,
                title: share.title,
                episode: share.episode,
                old: None,
            }),
            Ok(None) => None,
            Err(old) => Some(Self {
                link: text.trim().to_owned(),
                relays: Vec::new(),
                by: None,
                title: None,
                episode: None,
                old: Some(old),
            }),
        }
    }
}

/// A relay address a room may use: `wss://`, or plain `ws://` only for `localhost` and `.local`
/// (and the loopback address, for the test relay).
pub fn valid_relay(url: &str) -> bool {
    let host_of = |rest: &str| rest.split(['/', ':']).next().unwrap_or("").to_owned();
    if let Some(rest) = strip_prefix_ignore_case(url, "wss://") {
        return !host_of(rest).is_empty();
    }
    if let Some(rest) = strip_prefix_ignore_case(url, "ws://") {
        let host = host_of(rest).to_ascii_lowercase();
        return host == "localhost" || host == "127.0.0.1" || host.ends_with(".local");
    }
    false
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &text[prefix.len()..])
}

/// Every byte but the unreserved ones (RFC 3986) as `%XX`.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = text.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64url without padding.
pub fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(char::from(B64[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    out
}

/// Base64url back to bytes, padded or not; `None` for anything else.
pub fn unbase64url(text: &str) -> Option<Vec<u8>> {
    let text = text.trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut n = 0u32;
    let mut bits = 0;
    for c in text.bytes() {
        let v = B64.iter().position(|b| *b == c)? as u32;
        n = (n << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((n >> bits) as u8);
            n &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share() -> RoomShare {
        RoomShare {
            secret: *b"0123456789abcdef",
            relays: Vec::new(),
            by: Some("Mika".into()),
            title: Some("Natsukaze Relay".into()),
            episode: Some("9".into()),
        }
    }

    #[test]
    fn an_invite_reads_back_as_it_was_built() {
        let link = share().https_link();
        assert_eq!(
            link,
            "https://kaeri.app/room#v=3&k=MDEyMzQ1Njc4OWFiY2RlZg&by=Mika&n=Natsukaze%20Relay&e=9"
        );
        assert!(share().app_link().starts_with("kaeri://room#v=3&k="));
        assert_eq!(RoomShare::parse(&link), Ok(Some(share())));
        assert_eq!(RoomShare::parse(&share().app_link()), Ok(Some(share())));
        let custom = RoomShare { relays: vec!["wss://relay.home.example".into()], ..share() };
        assert_eq!(RoomShare::parse(&custom.https_link()), Ok(Some(custom)));
    }

    #[test]
    fn older_invites_and_codes_are_known_for_what_they_are() {
        let v2 = "https://kaeri.app/party#v=2&s=party.example:8999&r=kite-7qm4-xr9d";
        assert_eq!(RoomShare::parse(v2), Err(OldInvite::SyncplayLink));
        let oldest = "kaeri://party?server=a&room=b&title=1&episode=1";
        assert_eq!(RoomShare::parse(oldest), Err(OldInvite::SyncplayLink));
        assert_eq!(RoomShare::parse("Kite 7qm4 xr9d"), Err(OldInvite::Code));
        assert_eq!(RoomShare::parse("hello there"), Ok(None));
        assert_eq!(RoomTarget::parse("kite-7qm4-xr9d").unwrap().old, Some(OldInvite::Code));
        assert_eq!(RoomShare::parse("https://kaeri.app/room#v=3&k=short"), Ok(None));
        // A link to the watch party's old page, even a v=3 one, is from an older version.
        let party = "https://kaeri.app/party#v=3&k=MDEyMzQ1Njc4OWFiY2RlZg";
        assert_eq!(RoomShare::parse(party), Err(OldInvite::SyncplayLink));
    }

    #[test]
    fn only_secure_relays_or_local_ones() {
        assert!(valid_relay("wss://relay.home.example"));
        assert!(valid_relay("ws://localhost:7777"));
        assert!(valid_relay("ws://box.local"));
        assert!(!valid_relay("ws://relay.example"));
        assert!(!valid_relay("https://relay.example"));
    }

    #[test]
    fn base64url_round_trips() {
        for len in 0..20 {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(unbase64url(&base64url(&bytes)).unwrap(), bytes);
        }
    }
}
