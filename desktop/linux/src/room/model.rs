//! The room as the column, the strip and the banner draw it, gathered from the engine's records:
//! [`kaeri_model::room::RoomView`] with the members of [`Event::RoomMembers`], or an invite's
//! [`RoomGlimpse`] before joining. The parts are the engine's own types; only the gathering is
//! this app's.
//!
//! [`Event::RoomMembers`]: kaeri_model::event::Event::RoomMembers

pub use kaeri_model::room::{
    MemberState, MoveCandidate, PausedFor, RelayHealth, RoomCarriedOn, RoomConnection,
    RoomEpisode, RoomEpisodeInfo, RoomGlimpse, RoomMember, RoomUpNext, WaitReason,
};
use kaeri_model::room::{RoomChatBody, RoomConversation};

/// The room before play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Before {
    /// Nobody has pressed Play for everyone yet.
    Idle,
    /// Play pressed, waiting for everyone's first frame.
    Waiting,
    /// Everyone is ready: the count before play, in seconds.
    Count(u32),
}

/// The room as this app draws it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoomView {
    pub room_id: String,
    /// The show's title.
    pub title: String,
    /// The room's episode number.
    pub episode: String,
    /// The episode's own name, when known.
    pub episode_title: Option<String>,
    /// The episode's length in minutes, when known.
    pub minutes: Option<u32>,
    /// This device's name in the room.
    pub name: String,
    pub connection: RoomConnection,
    pub relays: Vec<RelayHealth>,
    pub members: Vec<RoomMember>,
    pub playing: bool,
    pub paused_by: Option<String>,
    /// The room's position in seconds, at `position_at_ms`.
    pub position: f64,
    pub position_at_ms: i64,
    pub waiting: Vec<PausedFor>,
    /// When the wait may show: a second after it began.
    pub wait_shown_at_ms: Option<i64>,
    /// When Carry on comes forward.
    pub carry_on_at_ms: Option<i64>,
    pub can_carry_on: bool,
    pub can_dont_wait: bool,
    pub carried_on: Option<RoomCarriedOn>,
    pub up_next: Option<RoomUpNext>,
    pub link: Option<String>,
    pub move_candidates: Vec<MoveCandidate>,
    /// Not started yet.
    pub before_play: bool,
    /// Everyone is ready: play starts for everyone at this time.
    pub starts_at_ms: Option<i64>,
    /// Opened from an invite, not joined yet: who invited.
    pub invited_by: Option<String>,
    /// The room an invite opens has its ten already.
    pub over_cap: bool,
}

impl RoomView {
    /// The engine's room with its members.
    pub fn from_engine(view: &kaeri_model::room::RoomView, members: &[RoomMember]) -> Self {
        let room = view.room_episode.as_ref();
        Self {
            room_id: view.room_id.clone(),
            title: room.map(|r| r.title.clone()).unwrap_or_default(),
            episode: room
                .map(|r| r.episode.clone())
                .or_else(|| view.episode.as_ref().map(|e| e.episode.clone()))
                .unwrap_or_default(),
            episode_title: room.and_then(|r| r.name.clone()),
            minutes: None,
            name: view.name.clone(),
            connection: view.connection,
            relays: view.relays.clone(),
            members: members.to_vec(),
            playing: view.playing,
            paused_by: if view.paused_by_you {
                members.iter().find(|m| m.you).map(|m| m.name.clone())
            } else {
                view.paused_by.clone()
            },
            position: view.position.unwrap_or(0.0),
            position_at_ms: view.position_at_ms,
            waiting: view.waiting.clone(),
            wait_shown_at_ms: view.wait_shown_at_ms,
            carry_on_at_ms: view.carry_on_at_ms,
            can_carry_on: view.can_carry_on,
            can_dont_wait: view.can_dont_wait,
            carried_on: view.carried_on.clone(),
            up_next: view.up_next.clone(),
            link: view.link.clone(),
            move_candidates: view.move_candidates.clone(),
            before_play: view.before_play,
            starts_at_ms: view.starts_at_ms,
            invited_by: None,
            over_cap: false,
        }
    }

    /// The room an invite opens, before joining it: who invited, and who is in it.
    pub fn from_glimpse(glimpse: &RoomGlimpse) -> Self {
        let room = glimpse.room_episode.as_ref();
        Self {
            room_id: glimpse.room_id.clone().unwrap_or_default(),
            title: room.map(|r| r.title.clone()).unwrap_or_default(),
            episode: room.map(|r| r.episode.clone()).unwrap_or_default(),
            episode_title: room.and_then(|r| r.name.clone()),
            members: glimpse.members.iter().filter(|m| !m.you).cloned().collect(),
            playing: glimpse.playing,
            connection: RoomConnection::Connected,
            invited_by: Some(glimpse.by.clone().unwrap_or_default()),
            over_cap: glimpse.full,
            ..Self::default()
        }
    }

    /// The room before play, at `now_ms`.
    pub fn before(&self, now_ms: i64) -> Option<Before> {
        if !self.before_play {
            return None;
        }
        if let Some(at) = self.starts_at_ms {
            let secs = ((at - now_ms) as f64 / 1000.0).ceil().max(0.0) as u32;
            return Some(Before::Count(secs));
        }
        Some(if self.waiting.is_empty() { Before::Idle } else { Before::Waiting })
    }

    /// The room's position at `now_ms`: it runs on while the room plays.
    pub fn position_at(&self, now_ms: i64) -> f64 {
        if self.playing && self.position_at_ms > 0 {
            self.position + ((now_ms - self.position_at_ms).max(0) as f64 / 1000.0)
        } else {
            self.position
        }
    }

    /// Carry on is offered at `now_ms`.
    pub fn carry_on_now(&self, now_ms: i64) -> bool {
        self.can_carry_on || self.carry_on_at_ms.is_some_and(|at| now_ms >= at)
    }

    /// The wait shows at `now_ms`: a wait under a second never does.
    pub fn wait_shows(&self, now_ms: i64) -> bool {
        !self.waiting.is_empty() && self.wait_shown_at_ms.is_none_or(|at| now_ms >= at)
    }

    pub fn you(&self) -> Option<&RoomMember> {
        self.members.iter().find(|m| m.you)
    }

    /// How many relays answer.
    pub fn relays_up(&self) -> usize {
        self.relays.iter().filter(|r| r.healthy).count()
    }
}

/// One line of the room's conversation, as the column shows it.
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

impl ConvLine {
    /// The engine's conversation as lines, muted people's left out.
    pub fn from_engine(conversation: &RoomConversation) -> Vec<Self> {
        conversation
            .lines
            .iter()
            .filter(|l| !l.muted)
            .map(|l| match &l.body {
                RoomChatBody::Said { text } => {
                    Self::Said { who: l.from.clone(), text: text.clone(), you: l.you }
                }
                RoomChatBody::Reacted { reaction, at, .. } => {
                    Self::System(super::copy::reacted(*reaction, &l.from, *at))
                }
                RoomChatBody::Room { message } => Self::System(message.text.clone()),
            })
            .collect()
    }
}
