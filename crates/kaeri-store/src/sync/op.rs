//! The ops: what one device did, in a form any other device can apply.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
///
/// Read through [`OpBody`]'s own `Deserialize`, which keeps a kind this version does not know as
/// [`OpBody::Unknown`] instead of failing the batch it came in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", tag = "k", rename_all = "snake_case")]
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
    /// A read event: a place in a chapter, or the chapter finished. Its id is `device:seq` of
    /// this op. `work` and `chapter` are the engine's WorkId and ChapterId strings, and only
    /// works AniList lists are written.
    Read {
        work: String,
        chapter: String,
        page: Option<u32>,
        of: Option<u32>,
        strip_image: Option<u32>,
        strip_offset: Option<f64>,
        novel_pos: Option<String>,
        completed: bool,
        source_id: Option<String>,
        at: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version_label: Option<String>,
        /// The trackers this read was queued for on the device that read it.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        queued_for: Vec<String>,
        /// The newest unread of this chapter the reading device had seen (a stamp, 0 for none).
        #[serde(default, skip_serializing_if = "is_zero")]
        seen_unread: u64,
    },
    /// Chapters taken back, naming the read events this device saw and removed.
    Unread { work: String, chapter: String, removes: Vec<String> },
    /// A watermark: every chapter at or below `upto_chapter` (in `upto_volume`, for numbers that
    /// start again each volume) counts as read, including chapters a source lists later.
    ReadAdopt { work: String, upto_chapter: String, upto_volume: Option<String>, at: i64 },
    /// Read again from the start: reads and watermarks older than this are set aside. With
    /// `undoes`, the reset of that id is undone and what it set aside comes back.
    ReadReset {
        work: String,
        at: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        undoes: Option<String>,
    },
    /// The highest chapter and volume a tracker is known to hold for a work: a max register.
    ReadMark { tracker: String, anilist_id: u32, chapter: u32, volume: Option<u32> },
    /// A device joined, with its X25519 public key (hex).
    DeviceAdd { device: String, name: String, pubkey: String },
    /// A device left. Wins over any add of the same device.
    DeviceRemove { device: String },
    /// What compaction leaves of a superseded op, so that numbering has no gaps.
    Noop,
    /// An op of a kind a newer version writes, as it arrived, its `k` included. Held and relayed
    /// to other devices unchanged, and never applied here; a version that knows the kind applies
    /// it once it runs (see [`crate::Store::apply_ops`]).
    #[serde(skip)]
    Unknown(serde_json::Value),
}

/// The `k` of every kind this version applies: anything else is read as [`OpBody::Unknown`].
const KINDS: &[&str] = &[
    "watch",
    "unwatch",
    "adopt",
    "reg",
    "tracker_mark",
    "download_state",
    "read",
    "unread",
    "read_adopt",
    "read_reset",
    "read_mark",
    "device_add",
    "device_remove",
    "noop",
];

impl Serialize for OpBody {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unknown(raw) => raw.serialize(serializer),
            known => Self::serialize(known, serializer),
        }
    }
}

impl<'de> Deserialize<'de> for OpBody {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(deserializer)?;
        match raw.get("k").and_then(serde_json::Value::as_str) {
            Some(kind) if !KINDS.contains(&kind) => Ok(Self::Unknown(raw)),
            // A known kind, or no kind at all: read as this version reads it, so a damaged op
            // is still an error.
            _ => Self::deserialize(raw).map_err(serde::de::Error::custom),
        }
    }
}

impl OpBody {
    /// Whether this version knows the op's kind, and so applies it.
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }
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

/// A finished chapter kept against an unread it had not seen: reading's [`SyncNote`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadNote {
    pub id: i64,
    pub work: String,
    pub chapter: String,
    pub unread_device: String,
    pub read_device: String,
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

/// A completed watch that arrived from another device: which device watched, and the trackers it
/// was queued for there, so a tracker it lacked can be filled by that tracker's lead device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedWatch {
    pub device: String,
    pub anilist_id: u32,
    pub episode: String,
    pub queued_for: Vec<String>,
}

/// How long a device counts as present for the lead rule, in seconds.
pub const LEAD_WINDOW_SECS: i64 = 7 * 24 * 3600;

/// A device in the group, for the lead rule: its id, the trackers it is signed in to, and when it
/// was last seen (unix seconds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeadCandidate {
    pub device: String,
    pub trackers: Vec<String>,
    pub last_seen: Option<i64>,
    pub removed: bool,
}

/// The device that fills `tracker` for watches whose origin lacked it: of the devices signed in
/// to it and seen in the last seven days (this device always counts as seen), the one with the
/// lowest id. Every device computes the same answer from the same device list and no messages.
pub fn lead_device<'a>(
    tracker: &str,
    me: &str,
    devices: &'a [LeadCandidate],
    now: i64,
) -> Option<&'a str> {
    devices
        .iter()
        .filter(|d| !d.removed && d.trackers.iter().any(|t| t == tracker))
        .filter(|d| {
            d.device == me || d.last_seen.is_some_and(|seen| now - seen <= LEAD_WINDOW_SECS)
        })
        .map(|d| d.device.as_str())
        .min()
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
    /// Ops of a kind this version does not know: held and relayed, not applied.
    pub unknown: usize,
    pub effects: Vec<Effect>,
    pub notes: Vec<SyncNote>,
    /// Reads kept against an unread they had not seen.
    pub read_notes: Vec<ReadNote>,
    pub titles: Vec<u32>,
    /// Completed watches that arrived, for the tracker lead rule.
    pub watches: Vec<ReceivedWatch>,
    /// Works whose reading changed, by WorkId.
    pub works: Vec<String>,
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
    /// Reading's, applied here. Keys: `<work>/<setting>` (mode, direction and colour only),
    /// `<work>`, `<work>/<chapter>`, `<tracker>/<work>` and `<source_id>/<series_ref>`.
    pub const READ_SERIES: &str = "read_series";
    pub const READ_STATUS: &str = "read_status";
    pub const READ_SCORE: &str = "read_score";
    pub const READ_HIDE: &str = "read_hide";
    pub const READ_BOOKMARK: &str = "read_bookmark";
    pub const READ_TRACK_MAP: &str = "read_track_map";
    /// A page the person said is not a credit page. Key `<work>/<fingerprint in hex>`.
    pub const READ_CREDIT_KEEP: &str = "read_credit_keep";
    pub const BOOK_MATCH: &str = "book_match";
    /// Engine kinds, returned as [`super::Effect`]s.
    pub const SETTING: &str = "setting";
    /// Reading's engine kinds: the book sources' order (key empty), a book source switched off
    /// (key its id), and the account's watch, read or both (key `account`).
    pub const BOOK_SOURCE_ORDER: &str = "book_source_order";
    pub const BOOK_SOURCE_OFF: &str = "book_source_off";
    pub const MEDIUM_ACCOUNT: &str = "medium_account";
    pub const SOURCE_ORDER: &str = "source_order";
    pub const SOURCE_OFF: &str = "source_off";
    pub const STORE: &str = "store";
    /// Applied to the device list: key is the device id.
    pub const DEVICE_NAME: &str = "device_name";
    pub const DEVICE_TRACKERS: &str = "device_trackers";
    /// What a device keeps for the cloud: its folder's kind and the cloud sign-ins it holds, as
    /// the engine's JSON. Kept as a register only; the engine reads it back.
    pub const DEVICE_CLOUD: &str = "device_cloud";
    /// What kind of device it is (`phone`, `tablet`, `computer`, `tv`). Key: the device id.
    pub const DEVICE_KIND: &str = "device_kind";
    /// The key the device's remote holds, `tv:<hex>` for a TV's server and `phone:<hex>` for a
    /// device that drives TVs. Key: the device id.
    pub const DEVICE_REMOTE_KEY: &str = "device_remote_key";
    /// `off` while the group holds back from the device (its Sync switch). Key: the device id.
    pub const DEVICE_SYNC: &str = "device_sync";
    /// `off` while a phone may not drive a TV it is paired with (the pair's Remote switch). Key:
    /// the two device ids, sorted, joined by `:`.
    pub const DEVICE_REMOTE: &str = "device_remote";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, trackers: &[&str], seen: Option<i64>) -> LeadCandidate {
        LeadCandidate {
            device: id.into(),
            trackers: trackers.iter().map(|t| (*t).to_owned()).collect(),
            last_seen: seen,
            removed: false,
        }
    }

    #[test]
    fn the_lead_is_the_lowest_signed_in_device_seen_this_week() {
        let now = 10_000_000;
        let devices = [
            dev("aa", &["anilist"], Some(now - LEAD_WINDOW_SECS - 1)),
            dev("bb", &["anilist", "trakt"], Some(now - 60)),
            dev("cc", &["anilist"], Some(now)),
            dev("dd", &[], Some(now)),
        ];
        // "aa" is the lowest but was last seen too long ago.
        assert_eq!(lead_device("anilist", "cc", &devices, now), Some("bb"));
        assert_eq!(lead_device("trakt", "cc", &devices, now), Some("bb"));
        assert_eq!(lead_device("simkl", "cc", &devices, now), None);
        // Every device reaches the same answer.
        assert_eq!(lead_device("anilist", "bb", &devices, now), Some("bb"));
        // This device is always present, however long since it saw itself.
        let alone = [dev("zz", &["anilist"], None)];
        assert_eq!(lead_device("anilist", "zz", &alone, now), Some("zz"));
        // A removed device never leads.
        let mut gone = dev("aa", &["anilist"], Some(now));
        gone.removed = true;
        assert_eq!(
            lead_device("anilist", "cc", &[gone, dev("cc", &["anilist"], None)], now),
            Some("cc")
        );
    }

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

    #[test]
    fn every_kind_this_version_writes_reads_back_as_known() {
        let bodies = [
            OpBody::Watch {
                anilist_id: 1,
                episode: "1".into(),
                position_secs: 1.0,
                duration_secs: None,
                watched_secs: 1.0,
                provider_id: None,
                translation: None,
                completed: false,
                at: 0,
                queued_for: Vec::new(),
                seen_unwatch: 0,
            },
            OpBody::Unwatch { anilist_id: 1, episode: "1".into(), removes: Vec::new() },
            OpBody::Adopt { anilist_id: 1, upto: 2, at: 0 },
            OpBody::Reg { kind: "k".into(), key: "1".into(), value: None },
            OpBody::TrackerMark { tracker: "t".into(), anilist_id: 1, episode: 2 },
            OpBody::DownloadState { anilist_id: 1, episode: "1".into(), done: true },
            OpBody::Read {
                work: "a:1".into(),
                chapter: "10.5".into(),
                page: Some(3),
                of: Some(20),
                strip_image: None,
                strip_offset: None,
                novel_pos: None,
                completed: false,
                source_id: None,
                at: 0,
                version_label: None,
                queued_for: Vec::new(),
                seen_unread: 0,
            },
            OpBody::Unread { work: "a:1".into(), chapter: "1".into(), removes: Vec::new() },
            OpBody::ReadAdopt {
                work: "a:1".into(),
                upto_chapter: "v2/3".into(),
                upto_volume: Some("2".into()),
                at: 0,
            },
            OpBody::ReadReset { work: "a:1".into(), at: 0, undoes: None },
            OpBody::ReadMark { tracker: "t".into(), anilist_id: 1, chapter: 10, volume: None },
            OpBody::DeviceAdd { device: "ab".into(), name: "n".into(), pubkey: "00".into() },
            OpBody::DeviceRemove { device: "ab".into() },
            OpBody::Noop,
        ];
        assert_eq!(bodies.len(), KINDS.len());
        for body in bodies {
            let json = serde_json::to_string(&body).unwrap();
            assert_eq!(serde_json::from_str::<OpBody>(&json).unwrap(), body, "{json}");
        }
    }

    #[test]
    fn an_unknown_kind_is_kept_as_it_came_and_a_damaged_known_one_is_refused() {
        let raw = r#"{"k":"chapter_read","work":"w-1","pages":[1,2],"at":{"n":1}}"#;
        let body: OpBody = serde_json::from_str(raw).unwrap();
        assert!(!body.is_known());
        let again: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&body).unwrap()).unwrap();
        assert_eq!(again, serde_json::from_str::<serde_json::Value>(raw).unwrap());
        // A kind this version knows, missing what it needs, is damage rather than the future.
        assert!(serde_json::from_str::<OpBody>(r#"{"k":"unwatch","anilist_id":1}"#).is_err());
        assert!(serde_json::from_str::<OpBody>(r#"{"anilist_id":1}"#).is_err());
    }
}
