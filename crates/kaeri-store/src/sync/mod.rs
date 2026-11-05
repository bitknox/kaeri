//! The sync log: every change as an op, written in the same transaction as the change, and the
//! rules that merge other devices' ops back in.
//!
//! Nothing is written until [`Store::sync_enable`] has run, so an install that never turns sync on
//! behaves exactly as it did. Received ops are applied through [`Store::apply_ops`], which never
//! writes ops of its own, so a received change is not sent on as new, and never queues a tracker
//! push.
//!
//! The rules, all pure functions of the set of ops held:
//!
//! - Watch events are a grow-only set keyed by id (`device:seq`). The resume point is the newest
//!   event by stamp.
//! - An unwatch removes the events it names and leaves a tombstone for each, so an event that
//!   arrives later is not put back. A watch it never saw is not named, and survives.
//! - Everything else is a register: the highest `(stamp, device)` wins, whichever order the ops
//!   arrive in.
//! - An op of a kind this version does not know, as a newer version writes, is held and relayed
//!   like any other but not applied. The version that knows its kind applies it after an update.

mod apply;
mod log;
pub mod op;

use std::sync::{Arc, PoisonError, RwLock};

pub use op::*;

pub(crate) use apply::refresh_title;
pub(crate) use log::{match_json, pin_json};

/// The device id, when sync is on, read in `tx`.
pub(crate) fn device_of(tx: &rusqlite::Connection) -> rusqlite::Result<Option<String>> {
    log::device(tx)
}

use crate::Store;

/// Wall-clock milliseconds, from the engine's clock when it sets one.
type Wall = Arc<dyn Fn() -> i64 + Send + Sync>;

pub(crate) struct SyncCtx {
    wall: RwLock<Wall>,
}

impl SyncCtx {
    pub(crate) fn new() -> Self {
        Self { wall: RwLock::new(Arc::new(system_ms)) }
    }

    pub(crate) fn wall_ms(&self) -> i64 {
        (self.wall.read().unwrap_or_else(PoisonError::into_inner))()
    }
}

fn system_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

impl Store {
    /// Read the wall clock sync stamps from `clock` instead of the system's: the engine's own
    /// clock, so tests keep off the real date. Shared by every clone of the store.
    pub fn set_sync_clock(&self, clock: impl Fn() -> i64 + Send + Sync + 'static) {
        *self.sync.wall.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(clock);
    }
}

#[cfg(test)]
mod tests;
