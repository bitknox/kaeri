//! The ops: what one device did, in a form any other device can apply.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A hybrid logical clock stamp: wall-clock milliseconds in the high 48 bits and a counter in the
/// low 16. Compared as a number; ties break by the writer's device id.
pub type Stamp = u64;

pub const COUNTER_BITS: u32 = 16;

/// The stamp for a wall-clock time in seconds, as events recorded before sync was on get theirs.
pub fn stamp_of_secs(secs: i64) -> Stamp {
    (secs.max(0) as u64 * 1000) << COUNTER_BITS
}

/// Seconds since the epoch a stamp was made at.
pub fn secs_of_stamp(stamp: Stamp) -> i64 {
    ((stamp >> COUNTER_BITS) / 1000) as i64
}

/// The next stamp for a clock at `wall_ms` that has seen `last`: never behind anything seen.
pub fn tick(last: Stamp, wall_ms: i64) -> Stamp {
    let wall = (wall_ms.max(0) as u64) << COUNTER_BITS;
    wall.max(last + 1)
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// One change, numbered by the device that made it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op {
    #[serde(rename = "d")]
    pub device: String,
    #[serde(rename = "s")]
    pub seq: u64,
    #[serde(rename = "t")]
    pub stamp: Stamp,
    #[serde(rename = "b")]
    pub body: OpBody,
}

/// What a change was. A `Reg` is a last-writer-wins register; the rest have rules of their own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "snake_case")]
pub enum OpBody {
    /// A watch event. Its id is `device:seq` of this op.
    Watch {
        anilist_id: u32,
        episode: String,
        position_secs: f64,
        duration_secs: Option<f64>,
        watched_secs: f64,
        provider_id: Option<String>,
        translation: Option<String>,
        completed: bool,
        at: i64,
        /// The trackers this watch was queued for on the device that watched.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        queued_for: Vec<String>,
        /// The newest unwatch of this episode the watching device had seen when it finished
        /// it (a stamp, 0 for none): an unwatch newer than that never knew of this watch.
        #[serde(default, skip_serializing_if = "is_zero")]
        seen_unwatch: u64,
    },
    /// Episodes taken back, naming the event ids this device saw and removed. A watch the
    /// unwatch never saw is not named, so it survives.
    Unwatch { anilist_id: u32, episode: String, removes: Vec<String> },
    /// A tracker's count adopted: episodes `1..=upto` count as watched.
    Adopt { anilist_id: u32, upto: u32, at: i64 },
    /// A last-writer-wins register. `kind` and `key` name it, `value` is `None` once cleared.
    Reg { kind: String, key: String, value: Option<String> },
    /// The highest episode a tracker is known to hold for a title: a max register.
    TrackerMark { tracker: String, anilist_id: u32, episode: u32 },
    /// What this device holds downloaded. Each device writes only its own.
    DownloadState { anilist_id: u32, episode: String, done: bool },
    /// A device joined, with its X25519 public key (hex).
    DeviceAdd { device: String, name: String, pubkey: String },
    /// A device left. Wins over any add of the same device.
    DeviceRemove { device: String },
    /// What compaction leaves of a superseded op, so that numbering has no gaps.
    Noop,
}

/// The highest op number held from each device.
pub type VersionVector = BTreeMap<String, u64>;

/// Something the store cannot apply itself, because it lives in the config or the sources: the
/// engine carries it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    pub kind: String,
    pub key: String,
    pub value: Option<String>,
}

/// A watch kept against an unwatch it had not seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncNote {
    pub id: i64,
    pub anilist_id: u32,
    pub episode: String,
    pub unwatch_device: String,
    pub watch_device: String,
    pub at: i64,
}

/// A write a received op asks of the device's own tables, which the personal transaction cannot
/// make: done by [`Store::apply_ops`] right after it commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeviceWrite {
    /// A cached resolution that a received match change made stale.
    ClearResolution { anilist_id: i64, provider_id: String },
    /// Keep next's let-go list: `at` is `None` to bring the episode back.
    LetGo { anilist_id: i64, episode: String, at: Option<i64> },
}

/// What applying a batch did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// Ops newly applied.
    pub ops: usize,
    /// Ops skipped because they were already held.
    pub duplicates: usize,
    /// Ops held back because an earlier one from the same device is missing.
    pub gaps: usize,
    pub effects: Vec<Effect>,
    pub notes: Vec<SyncNote>,
    pub titles: Vec<u32>,
    pub(crate) device: Vec<DeviceWrite>,
}

/// Register kinds the store applies itself.
pub mod kinds {
    pub const HIDE: &str = "hide";
    pub const STATUS: &str = "status";
    pub const SCORE: &str = "score";
    pub const SHOW_SETTING: &str = "show_setting";
    pub const TRACK_MEMORY: &str = "track_memory";
    pub const PIN: &str = "pin";
    pub const MATCH: &str = "match";
    pub const LET_GO: &str = "let_go";
    /// Engine kinds, returned as [`super::Effect`]s.
    pub const SETTING: &str = "setting";
    pub const SOURCE_ORDER: &str = "source_order";
    pub const SOURCE_OFF: &str = "source_off";
    pub const STORE: &str = "store";
    /// Applied to the device list: key is the device id.
    pub const DEVICE_NAME: &str = "device_name";
    pub const DEVICE_TRACKERS: &str = "device_trackers";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stamp_is_never_behind_what_was_seen() {
        let seen = tick(0, 5_000);
        // A clock that went back an hour.
        let next = tick(seen, 5_000 - 3_600_000);
        assert!(next > seen);
        // And one an hour fast cannot be overtaken by the wall clock alone, only by seeing it.
        assert_eq!(secs_of_stamp(stamp_of_secs(1_700_000_000)), 1_700_000_000);
    }

    #[test]
    fn ops_round_trip_through_json() {
        let op = Op {
            device: "ab12".into(),
            seq: 3,
            stamp: 9,
            body: OpBody::Unwatch {
                anilist_id: 1,
                episode: "5".into(),
                removes: vec!["ab12:2".into()],
            },
        };
        let json = serde_json::to_string(&op).unwrap();
        assert_eq!(serde_json::from_str::<Op>(&json).unwrap(), op);
    }
}
