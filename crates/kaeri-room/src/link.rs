//! What the engine sends to a room and hears from it, whichever transport carries it: the relay
//! pool over Nostr, or the in-process fake (feature `testing`). Bodies go out whole; sealing,
//! padding, relays and dedupe are the transport's.

use tokio::sync::mpsc;

use crate::wire::{Body, Mid};

/// One relay as this device finds it.
#[derive(Debug, Clone, PartialEq)]
pub struct RelayState {
    pub url: String,
    pub healthy: bool,
    pub rtt_ms: Option<u32>,
}

/// What the transport hears.
#[derive(Debug, Clone, PartialEq)]
pub enum Heard {
    /// An event from anyone, this device's own echo included (the first copy only).
    Body(Box<Body>),
    /// How many relays are up, after any change.
    Up { relays_up: u32 },
    /// Each relay's health, after any change.
    Relays(Vec<RelayState>),
    /// The background status reached a relay, or the wait for it ran out.
    Flushed,
    /// Testing only: treat `mid` as heard from `ms` earlier than it was, so a scene can show
    /// Reconnecting without waiting 40 s.
    Backdate { mid: Mid, ms: i64 },
    /// Testing only: this device's own player is in `s`, begun `ms_ago`, on `ep` (the room's
    /// when `None`), `behind` seconds behind the room. For scenes of the device's own states.
    Pose {
        s: crate::wire::StateCode,
        ms_ago: i64,
        ep: Option<crate::wire::Ep>,
        behind: Option<f64>,
        ue: bool,
    },
}

/// What the engine sends.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    Body(Body),
    /// Move to a new key epoch: new tag, new keys, same relays.
    Rekey([u8; 16]),
    /// Send the background status and say [`Heard::Flushed`].
    Flush(Body),
    /// Close and open every socket again (foreground, a network change).
    Refresh,
    /// Use these relays.
    Relays(Vec<String>),
}

/// A room's connection.
pub struct Link {
    pub out: mpsc::UnboundedSender<Out>,
    pub heard: mpsc::UnboundedReceiver<Heard>,
    /// The relays in use.
    pub relays: Vec<String>,
}

/// Why a room can't be reached.
#[derive(Debug, Clone, PartialEq)]
pub enum Unreachable {
    /// None of the relays answered.
    NoRelay,
}

/// The relays a new room uses: the settings' list, or the built-in defaults. A testing build's
/// fake room puts its own in front of both, so nothing reaches a public relay.
pub fn relays_for_new_room(chosen: &[String], defaults: &[&str]) -> Vec<String> {
    #[cfg(any(test, feature = "testing"))]
    if let Some(fake) = crate::fake::relays() {
        return fake;
    }
    if chosen.is_empty() {
        defaults.iter().map(|r| (*r).to_owned()).collect()
    } else {
        chosen.to_vec()
    }
}

/// Connect to the room with `secret` on `relays`.
pub async fn open(secret: [u8; 16], relays: Vec<String>) -> Result<Link, Unreachable> {
    #[cfg(any(test, feature = "testing"))]
    if let Some(link) = crate::fake::open(secret, &relays) {
        return link;
    }
    let _ = (secret, relays);
    // The Nostr relay pool comes with the transport; until then only the fake answers.
    Err(Unreachable::NoRelay)
}
