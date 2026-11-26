//! The room: watching together with no host, over end-to-end encrypted Nostr rooms.
//!
//! - [`wire`]: the body every event seals: the sender's status and one message.
//! - [`room`]: members, statuses, the playhead register, waits and timeouts. Pure, clocked by
//!   the caller.
//! - [`link`]: what the engine sends and hears, whichever transport carries it.
//! - [`limits`]: how much one member says.
//! - `fake` (feature `testing`): an in-process room with scripted members.
//!
//! See `docs/room-protocol.md`.

pub mod clock;
pub mod drift;
pub mod envelope;
#[cfg(any(test, feature = "testing"))]
pub mod fake;
#[cfg(any(test, feature = "testing"))]
pub mod fake_relay;
pub mod keys;
pub mod limits;
pub mod link;
pub mod pool;
pub mod relay;
pub mod room;
pub mod wire;

pub use limits::SendLimits;
pub use room::{Happening, Member, Reason, Room, Waited, Word};

/// A stable 64-bit mix of two strings (FNV-1a), for the sync answer's order.
pub(crate) fn mix(a: &str, b: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in a.bytes().chain([0]).chain(b.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
