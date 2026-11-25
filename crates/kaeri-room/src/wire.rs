//! What travels in a room: the body every event seals, with the sender's status and one message.
//! Short keys, `v: 1`. The body has no field for a source, a link, a provider or a plugin: the room
//! carries what is playing, never where from (see the schema test below).

use serde::{Deserialize, Serialize};

/// The protocol version a body carries.
pub const VERSION: u8 = 1;

/// A member's id in a room: the first 8 bytes of SHA-256 of its identity key, as hex.
pub type Mid = String;

/// An episode: an AniList id and the episode, `[4242, "9"]` on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ep(pub u32, pub String);

/// What a device's own player is doing, as it sends it. Paused, Ready, Other episode,
/// Reconnecting and Left are worked out by every receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StateCode {
    /// Player open on the episode and ready for the current wait.
    #[serde(rename = "w")]
    Watching,
    #[serde(rename = "l")]
    Loading,
    #[serde(rename = "b")]
    Buffering,
    #[serde(rename = "h")]
    Behind,
    #[serde(rename = "c")]
    Choosing,
    #[serde(rename = "n")]
    NoSource,
    #[serde(rename = "x")]
    PlayerClosed,
}

impl StateCode {
    /// Whether the player is open on some episode in this state.
    pub fn player_open(self) -> bool {
        matches!(self, Self::Watching | Self::Loading | Self::Buffering | Self::Behind)
    }
}

/// A device's status, in every message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub s: StateCode,
    /// The episode its player has open.
    #[serde(default)]
    pub ep: Option<Ep>,
    /// Its player's position in seconds.
    #[serde(default)]
    pub pos: Option<f64>,
    #[serde(default)]
    pub play: bool,
    /// Milliseconds since `s` began.
    #[serde(default)]
    pub age: u32,
    /// Ready (first frame after the cue) for this wait's seq.
    #[serde(default)]
    pub rdy: Option<u64>,
    /// Milliseconds since the current playhead began playing here.
    #[serde(default)]
    pub pe: Option<u32>,
    /// This device's own Up next point has come.
    #[serde(default)]
    pub ue: bool,
    /// Catching up: never waited for.
    #[serde(default)]
    pub rejoin: bool,
    /// Don't wait for me.
    #[serde(default)]
    pub dw: bool,
    /// The app is in the background.
    #[serde(default)]
    pub bg: bool,
    /// Digest: playhead seq, relays seq, key epoch.
    #[serde(default)]
    pub dg: [u64; 3],
    /// A bit per room relay: healthy here.
    #[serde(default)]
    pub ok: u8,
    /// Relay round trip in ms.
    #[serde(default)]
    pub rtt: u16,
    /// Heartbeat period in seconds.
    #[serde(default = "default_hb")]
    pub hb: u8,
}

fn default_hb() -> u8 {
    15
}

impl Default for Status {
    fn default() -> Self {
        Self {
            s: StateCode::PlayerClosed,
            ep: None,
            pos: None,
            play: false,
            age: 0,
            rdy: None,
            pe: None,
            ue: false,
            rejoin: false,
            dw: false,
            bg: false,
            dg: [0; 3],
            ok: 0,
            rtt: 0,
            hb: default_hb(),
        }
    }
}

/// Why the playhead was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    Play,
    Pause,
    Seek,
    SkipOpening,
    Episode,
    MoveOn,
    Stall,
    CarryOn,
    UpNext,
    UpNextCancel,
}

impl Cause {
    /// A write with this cause starts a wait: its seq becomes `wseq`.
    pub fn starts_wait(self) -> bool {
        matches!(
            self,
            Self::Episode | Self::MoveOn | Self::Seek | Self::SkipOpening | Self::Stall
        )
    }
}

/// The room's Up next.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Up {
    #[default]
    None,
    /// Counting 10 s from this room position.
    Counting {
        from_pos: f64,
    },
    Cancelled,
}

/// The room's playhead: a last-writer-wins register ordered by `(seq, t, pk)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playhead {
    pub seq: u64,
    /// The writer's corrected clock, ms: ties only.
    pub t: i64,
    /// The writer's session key, hex: the last tie-break. Not on the wire (the event's pubkey).
    #[serde(skip)]
    pub pk: String,
    pub ep: Ep,
    #[serde(default)]
    pub next: Option<Ep>,
    /// The anchor, in seconds.
    pub pos: f64,
    /// Intent: play or pause.
    pub play: bool,
    pub cause: Cause,
    pub by: Mid,
    /// The seq of the last write that started a wait.
    pub wseq: u64,
    /// Carried on past in that wait.
    #[serde(default)]
    pub carry: Vec<Mid>,
    #[serde(default)]
    pub up: Up,
}

impl Playhead {
    /// Whether this write wins over `other`.
    pub fn beats(&self, other: &Playhead) -> bool {
        (self.seq, std::cmp::Reverse(self.t), std::cmp::Reverse(&self.pk))
            > (other.seq, std::cmp::Reverse(other.t), std::cmp::Reverse(&other.pk))
    }
}

/// A reaction, typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reaction {
    Laugh,
    Gasp,
    Clap,
    Cry,
}

/// A member as a sync answer lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RosterEntry {
    pub mid: Mid,
    pub name: String,
    #[serde(default)]
    pub idpk: String,
    #[serde(default)]
    pub idsig: String,
    pub st: Status,
    pub joined_ms_ago: u64,
    /// Left already (kept for the cap's count only while not Left).
    #[serde(default)]
    pub gone: bool,
}

/// One payload of a Move, sealed to one member.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveTo {
    pub mid: Mid,
    pub pk8: String,
    pub sealed: String,
}

/// What a Move payload opens to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovePayload {
    /// The new room's secret, base64url.
    pub secret: String,
    pub relays: Vec<String>,
    pub by: String,
    pub ep: Ep,
    pub now_pos: f64,
    pub playing: bool,
}

/// One message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "snake_case")]
pub enum Message {
    Heartbeat,
    Hello {
        name: String,
        app: String,
        #[serde(default)]
        caps: Vec<String>,
        #[serde(default)]
        idpk: String,
        #[serde(default)]
        idsig: String,
    },
    Status,
    Playhead(Playhead),
    SyncReq {
        nonce: String,
        #[serde(default)]
        peek: bool,
    },
    SyncAns {
        nonce: String,
        #[serde(default)]
        playhead: Option<Playhead>,
        #[serde(default)]
        relays: Vec<String>,
        #[serde(default)]
        relays_seq: u64,
        #[serde(default)]
        epoch: u64,
        #[serde(default)]
        now_pos: Option<f64>,
        #[serde(default)]
        playing_now: bool,
        /// The room has played this episode already (no Play for everyone, no count).
        #[serde(default)]
        started: bool,
        roster: Vec<RosterEntry>,
    },
    Chat {
        id: String,
        text: String,
    },
    React {
        id: String,
        reaction: Reaction,
        at: f64,
    },
    Relays {
        seq: u64,
        list: Vec<String>,
    },
    Rekey {
        seq: u64,
        to: Vec<(String, String)>,
    },
    Move {
        id: String,
        to: Vec<MoveTo>,
    },
    Leave,
}

/// The whole body, before padding and sealing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub v: u8,
    /// The session public key, hex. Must equal the event's pubkey.
    pub from: String,
    pub mid: Mid,
    /// Per session, rising by one each event.
    pub ctr: u64,
    /// The sender's corrected clock, ms.
    pub t: i64,
    pub st: Status,
    pub m: Message,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_round_trips_and_has_no_place_for_a_source() {
        let body = Body {
            v: VERSION,
            from: "ab".into(),
            mid: "01".into(),
            ctr: 3,
            t: 1000,
            st: Status {
                s: StateCode::Loading,
                ep: Some(Ep(4242, "9".into())),
                ..Status::default()
            },
            m: Message::Chat { id: "c1".into(), text: "hi".into() },
        };
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(serde_json::from_str::<Body>(&json).unwrap(), body);
        for word in ["source", "url", "provider", "plugin", "link", "stream"] {
            assert!(!json.contains(word), "{word} in {json}");
        }
    }

    #[test]
    fn ties_go_to_the_earlier_write_then_the_lower_key() {
        let at = |seq, t, pk: &str| Playhead {
            seq,
            t,
            pk: pk.into(),
            ep: Ep(1, "1".into()),
            next: None,
            pos: 0.0,
            play: true,
            cause: Cause::Play,
            by: "m".into(),
            wseq: 0,
            carry: vec![],
            up: Up::None,
        };
        assert!(at(42, 100, "b").beats(&at(42, 400, "a")));
        assert!(at(42, 100, "a").beats(&at(42, 100, "b")));
        assert!(at(43, 900, "z").beats(&at(42, 100, "a")));
    }
}
