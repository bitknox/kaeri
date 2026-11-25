//! Stand-ins for the engine's room records, shaped like section 6 of the room spec
//! (`RoomView`, `RoomMember`, `MemberState`, ...). They go when the engine's FFI lands: the UI
//! reads these names only, so the swap is a `use` change.

/// One relay the room talks through, as Details lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct RelayHealth {
    pub url: String,
    pub healthy: bool,
    pub rtt_ms: Option<u32>,
}

/// Why the room waits for someone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitReason {
    Loading,
    Seeking,
    Buffering,
}

/// Someone the room waits for.
#[derive(Debug, Clone, PartialEq)]
pub struct PausedFor {
    pub name: String,
    pub reason: WaitReason,
    /// Unix epoch milliseconds on this device's clock.
    pub since_ms: i64,
}

/// The room's Up next card.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomUpNext {
    pub episode: String,
    pub ends_at_ms: Option<i64>,
    pub waiting_for: Vec<String>,
    pub cancelled_by: Option<String>,
}

/// This device's link to the relays.
#[derive(Debug, Clone, PartialEq)]
pub enum RoomConnection {
    Connecting,
    Connected,
    Reconnecting { relays_up: u32 },
}

/// One person's state, the word under their name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberState {
    Watching,
    Paused,
    Loading,
    Buffering,
    Behind,
    OtherEpisode,
    Choosing,
    NoSource,
    PlayerClosed,
    Reconnecting,
    Left,
}

/// One person in the room.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomMember {
    pub id: String,
    pub name: String,
    pub you: bool,
    pub state: MemberState,
    pub since_ms: Option<i64>,
    pub behind_s: Option<f32>,
    /// The episode number they are on, when it isn't the room's.
    pub episode: Option<String>,
    /// They paused the room: the badge.
    pub paused_room: bool,
    pub background: bool,
    /// Muted in this device's chat.
    pub muted: bool,
}

/// Someone the mover can tick in Move to a new room.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveCandidate {
    pub id: String,
    pub name: String,
    pub muted: bool,
    pub ticked: bool,
}

/// An offer to go along to a new room.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomMoveOffer {
    pub id: String,
    pub by: String,
    /// Who else was ticked.
    pub names: Vec<String>,
    pub title: String,
    pub episode: String,
    pub expires_at_ms: i64,
}

/// The room before play, and where an invite opened it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Before {
    /// Nobody has pressed Play for everyone yet.
    Idle,
    /// Play pressed, waiting for everyone's first frame.
    Waiting,
    /// Everyone is ready: the count before play.
    Count(u32),
}

/// The room as the engine shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomView {
    pub room_id: String,
    pub title: String,
    pub episode: String,
    pub episode_title: Option<String>,
    pub minutes: Option<u32>,
    pub name: String,
    pub connection: RoomConnection,
    pub relays: Vec<RelayHealth>,
    pub members: Vec<RoomMember>,
    /// The room's playhead plays.
    pub playing: bool,
    pub paused_by: Option<String>,
    /// The room's position, seconds.
    pub position: f64,
    pub waiting: Vec<PausedFor>,
    pub can_carry_on: bool,
    pub can_dont_wait: bool,
    /// Someone carried on during the current wait.
    pub carried_on_by: Option<String>,
    pub up_next: Option<RoomUpNext>,
    pub link: Option<String>,
    pub move_candidates: Vec<MoveCandidate>,
    /// Before play: the banner shows until everyone's first frame.
    pub before: Option<Before>,
    /// Opened from an invite, not joined yet: who invited.
    pub invited_by: Option<String>,
    /// How many people are in a room this invite opens, when past the soft cap.
    pub over_cap: bool,
}

impl RoomView {
    pub fn you(&self) -> Option<&RoomMember> {
        self.members.iter().find(|m| m.you)
    }

    /// How many relays answer.
    pub fn relays_up(&self) -> usize {
        self.relays.iter().filter(|r| r.healthy).count()
    }
}

/// One line of the room's conversation, as the column shows it. A stand-in for the engine's
/// conversation records until the room's chat is wired.
#[derive(Debug, Clone, PartialEq)]
pub enum ConvLine {
    /// What the room did: "Mika paused at 12:04".
    System(String),
    Said {
        who: String,
        text: String,
        you: bool,
    },
}
