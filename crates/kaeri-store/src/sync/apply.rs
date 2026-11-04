//! Applying other devices' ops, the projection they leave, and compaction.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    Applied, Effect, LeadCandidate, Op, OpBody, ReceivedWatch, Stamp, Store, SyncNote,
    VersionVector, lead_device, log,
    op::{self, kinds},
};
use crate::Result;

/// Whether `(stamp, device)` beats what the register holds.
pub(crate) fn beats(
    tx: &Transaction<'_>,
    kind: &str,
    key: &str,
    stamp: Stamp,
    device: &str,
) -> rusqlite::Result<bool> {
    let held: Option<(i64, String)> = tx
        .query_row(
            "SELECT stamp, device FROM sync_reg WHERE kind = ?1 AND key = ?2",
            [kind, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(held.is_none_or(|(s, d)| (stamp, device) > (s as u64, d.as_str())))
}

pub(crate) fn set_reg(
    tx: &Transaction<'_>,
    kind: &str,
    key: &str,
    stamp: Stamp,
    device: &str,
    value: Option<&str>,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO sync_reg (kind, key, stamp, device, value) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(kind, key) DO UPDATE SET
            stamp = excluded.stamp, device = excluded.device, value = excluded.value",
        params![kind, key, stamp as i64, device, value],
    )?;
    Ok(())
}

/// The effective stamp of an event, for those recorded before sync was on.
const EFFECTIVE: &str = "COALESCE(stamp, at * 65536000)";

/// Rebuild a title's projection from its events: the resume point is the newest event by stamp
/// (a tracker's adopted count never is, unless it is all there is), and hidden is whether the
/// title was hidden after its newest watch.
pub(crate) fn refresh_title(tx: &Transaction<'_>, anilist_id: u32) -> rusqlite::Result<()> {
    let done: u32 = tx.query_row(
        "SELECT COUNT(DISTINCT episode) FROM watch_event WHERE anilist_id = ?1 AND completed = 1",
        [anilist_id],
        |r| r.get(0),
    )?;
    let newest: Option<(String, f64, Option<f64>, i64, i64)> = tx
        .query_row(
            &format!(
                "SELECT episode, position_secs, duration_secs, at, {EFFECTIVE} FROM watch_event
                  WHERE anilist_id = ?1
                  ORDER BY (provider_id IS 'tracker'), {EFFECTIVE} DESC, CAST(episode AS REAL) DESC,
                          COALESCE(uid, '') DESC
                  LIMIT 1"
            ),
            [anilist_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let Some((episode, position, duration, at, _)) = newest else {
        tx.execute("DELETE FROM watch_progress WHERE anilist_id = ?1", [anilist_id])?;
        return Ok(());
    };
    // Updated-at is the newest event's, whichever kind: the rail orders by it.
    let updated: i64 = tx.query_row(
        &format!("SELECT MAX({EFFECTIVE}) / 65536000 FROM watch_event WHERE anilist_id = ?1"),
        [anilist_id],
        |r| r.get(0),
    )?;
    let newest_watch: i64 = tx.query_row(
        &format!(
            "SELECT COALESCE(MAX({EFFECTIVE}), 0) FROM watch_event
              WHERE anilist_id = ?1 AND provider_id IS NOT 'tracker'"
        ),
        [anilist_id],
        |r| r.get(0),
    )?;
    let hidden: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_reg WHERE kind = 'hide' AND key = ?1
                         AND value IS NOT NULL AND stamp > ?2)",
        params![anilist_id.to_string(), newest_watch],
        |r| r.get(0),
    )?;
    let _ = at;
    tx.execute(
        "INSERT INTO watch_progress
           (anilist_id, last_episode, last_position, last_duration, episodes_done, updated_at, hidden)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(anilist_id) DO UPDATE SET
            last_episode = excluded.last_episode, last_position = excluded.last_position,
            last_duration = excluded.last_duration, episodes_done = excluded.episodes_done,
            updated_at = excluded.updated_at, hidden = excluded.hidden",
        params![anilist_id, episode, position, duration, done, updated, hidden],
    )?;
    Ok(())
}

/// Whether sync is on, read in the transaction that is about to write.
pub(crate) fn is_on(tx: &rusqlite::Connection) -> rusqlite::Result<bool> {
    Ok(log::device(tx)?.is_some())
}

/// A device: id, name, public key, removed.
pub type PeerRow = (String, Option<String>, Option<String>, bool);

impl Store {
    /// A watch recorded here (row `rowid`): its op, its id and stamp, and the projection. `false`
    /// when sync is off, and the caller keeps its own projection.
    pub(crate) fn sync_local_watch(
        &self,
        tx: &Transaction<'_>,
        rowid: i64,
        event: &crate::WatchEvent,
        queued_for: Vec<String>,
    ) -> Result<bool> {
        let body = OpBody::Watch {
            anilist_id: event.anilist_id.get(),
            episode: event.episode.clone(),
            position_secs: event.position_secs,
            duration_secs: event.duration_secs,
            watched_secs: event.watched_secs,
            provider_id: event.provider_id.clone(),
            translation: event.translation.map(|t| t.as_str().to_owned()),
            completed: event.completed,
            at: event.at,
            queued_for,
            seen_unwatch: if event.completed {
                tx.query_row(
                    "SELECT stamp FROM sync_unwatch WHERE anilist_id = ?1 AND episode = ?2",
                    params![event.anilist_id.get(), event.episode],
                    |r| r.get::<_, i64>(0),
                )
                .optional()?
                .unwrap_or(0) as u64
            } else {
                0
            },
        };
        let Some(op) = self.emit(tx, body)? else { return Ok(false) };
        tx.execute(
            "UPDATE watch_event SET uid = ?2, stamp = ?3 WHERE id = ?1",
            params![rowid, format!("{}:{}", op.device, op.seq), op.stamp as i64],
        )?;
        refresh_title(tx, event.anilist_id.get())?;
        Ok(true)
    }

    /// A device joined the group, with its public key.
    pub fn sync_device_add(&self, device: &str, name: &str, pubkey: &str) -> Result<()> {
        self.local_device_op(OpBody::DeviceAdd {
            device: device.to_owned(),
            name: name.to_owned(),
            pubkey: pubkey.to_owned(),
        })
    }

    /// A device was removed from the group. Wins over any later add of the same device.
    pub fn sync_device_remove(&self, device: &str) -> Result<()> {
        self.local_device_op(OpBody::DeviceRemove { device: device.to_owned() })
    }

    fn local_device_op(&self, body: OpBody) -> Result<()> {
        let now = self.sync.wall_ms() / 1000;
        self.with_personal_tx(|tx| {
            if let Some(op) = self.emit(tx, body)? {
                apply_one(tx, &op, now, &mut Applied::default(), &mut BTreeSet::new())?;
            }
            Ok(())
        })
    }

    /// The group's devices as the log has them: `(device, name, public key, removed)`.
    pub fn sync_peers(&self) -> Result<Vec<PeerRow>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT device, name, pubkey, removed FROM sync_peer ORDER BY device",
            )?;
            Ok(stmt
                .query_map([], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? != 0))
                })?
                .collect::<rusqlite::Result<_>>()?)
        })
    }

    /// A kept-watch note, as a pass through the folder would leave one: for a fixture build
    /// that shows the note without a second device writing it.
    pub fn sync_fixture_note(
        &self,
        anilist_id: u32,
        episode: &str,
        unwatch_device: &str,
        watch_device: &str,
        at: i64,
    ) -> Result<()> {
        self.with_personal_tx(|tx| {
            note(
                tx,
                &mut Applied::default(),
                anilist_id,
                episode,
                unwatch_device,
                watch_device,
                at,
            )?;
            Ok(())
        })
    }

    /// The devices as the lead rule sees them.
    pub fn sync_lead_candidates(&self) -> Result<Vec<LeadCandidate>> {
        self.with_personal(|c| {
            let mut stmt =
                c.prepare("SELECT device, trackers, last_seen, removed FROM sync_peer")?;
            Ok(stmt
                .query_map([], |r| {
                    let trackers: String = r.get(1)?;
                    Ok(LeadCandidate {
                        device: r.get(0)?,
                        trackers: serde_json::from_str(&trackers).unwrap_or_default(),
                        last_seen: r.get(2)?,
                        removed: r.get::<_, i64>(3)? != 0,
                    })
                })?
                .collect::<rusqlite::Result<_>>()?)
        })
    }

    /// Say which trackers this device is signed in to, so the others can work out the lead.
    pub fn sync_set_my_trackers(&self, trackers: &[String]) -> Result<()> {
        let Some(me) = self.sync_device()? else { return Ok(()) };
        let json = serde_json::to_string(trackers)
            .map_err(|source| crate::StoreError::Encode { what: "trackers", source })?;
        if self.sync_reg_value(kinds::DEVICE_TRACKERS, &me)?.as_deref() == Some(json.as_str()) {
            return Ok(());
        }
        self.sync_register(kinds::DEVICE_TRACKERS, &me, Some(&json), None)
    }

    /// Say what this device keeps for the cloud (the engine's JSON), so the others can show it and
    /// know what they can ask it for. Written only when it changed.
    pub fn sync_set_my_cloud(&self, json: &str) -> Result<()> {
        let Some(me) = self.sync_device()? else { return Ok(()) };
        if self.sync_reg_value(kinds::DEVICE_CLOUD, &me)?.as_deref() == Some(json) {
            return Ok(());
        }
        self.sync_register(kinds::DEVICE_CLOUD, &me, Some(json), None)
    }

    /// Of the watches that arrived, the `(tracker, title)` pairs this device must fill: a tracker
    /// the watching device was not queued for, that this device is signed in to, that no device
    /// has reached the episode on, and whose lead (see [`lead_device`]) is this device. Anything
    /// else is somebody else's to send, or already sent.
    pub fn sync_tracker_gaps(
        &self,
        my_trackers: &[String],
        watches: &[ReceivedWatch],
        now: i64,
    ) -> Result<Vec<(String, u32)>> {
        let Some(me) = self.sync_device()? else { return Ok(Vec::new()) };
        // This device knows its own trackers first-hand, and is always present.
        let mut devices = self.sync_lead_candidates()?;
        devices.retain(|d| d.device != me);
        devices.push(LeadCandidate {
            device: me.clone(),
            trackers: my_trackers.to_vec(),
            last_seen: Some(now),
            removed: false,
        });
        let mut out = Vec::new();
        for tracker in my_trackers {
            if lead_device(tracker, &me, &devices, now) != Some(me.as_str()) {
                continue;
            }
            for watch in watches {
                if watch.queued_for.iter().any(|t| t == tracker) {
                    continue;
                }
                let reached = watch.episode.parse::<u32>().ok();
                let marked = self.sync_tracker_mark_for(tracker, watch.anilist_id)?;
                if matches!((marked, reached), (Some(m), Some(e)) if m >= e) {
                    continue;
                }
                let pair = (tracker.clone(), watch.anilist_id);
                if !out.contains(&pair) {
                    out.push(pair);
                }
            }
        }
        Ok(out)
    }

    /// A tracker's count adopted while sync is on: one op, and an event for every episode up to
    /// it (ids named by the op), so that every device holds the same events and an unwatch can
    /// name them. Returns how many episodes were newly marked, as the plain path does.
    pub(crate) fn adopt_synced(
        &self,
        tx: &Transaction<'_>,
        _device: &str,
        anilist_id: kaeri_core::ids::AnilistId,
        upto: u32,
        at: i64,
    ) -> Result<u32> {
        let id = anilist_id.get();
        let newly: u32 = tx.query_row(
            "WITH RECURSIVE series(n) AS (
                 SELECT 1 UNION ALL SELECT n + 1 FROM series WHERE n < ?2
             )
             SELECT COUNT(*) FROM series WHERE NOT EXISTS (
                 SELECT 1 FROM watch_event
                  WHERE anilist_id = ?1 AND episode = CAST(n AS TEXT) AND completed = 1)",
            params![id, upto],
            |r| r.get(0),
        )?;
        let Some(op) = self.emit(tx, OpBody::Adopt { anilist_id: id, upto, at })? else {
            return Ok(0);
        };
        insert_adopted(tx, id, upto, at, op.stamp, &format!("a:{}:{}", op.device, op.seq))?;
        refresh_title(tx, id)?;
        Ok(newly)
    }

    /// The ids of an episode's events, for naming them in an unwatch.
    pub(crate) fn sync_event_uids(
        tx: &Transaction<'_>,
        anilist_id: u32,
        episode: &str,
        completed_only: bool,
    ) -> rusqlite::Result<Vec<String>> {
        let mut stmt = tx.prepare(&format!(
            "SELECT uid FROM watch_event WHERE anilist_id = ?1 AND episode = ?2
               AND uid IS NOT NULL {}",
            if completed_only { "AND completed = 1" } else { "" }
        ))?;
        stmt.query_map(params![anilist_id, episode], |r| r.get(0))?.collect()
    }

    /// An unwatch made here, after its deletes: the op, and tombstones so that a relayed copy of
    /// an event removed here is not put back.
    pub(crate) fn sync_local_unwatch(
        &self,
        tx: &Transaction<'_>,
        anilist_id: u32,
        episode: &str,
        removes: Vec<String>,
    ) -> Result<()> {
        let body = OpBody::Unwatch {
            anilist_id,
            episode: episode.to_owned(),
            removes: removes.clone(),
        };
        let Some(op) = self.emit(tx, body)? else { return Ok(()) };
        for uid in &removes {
            tx.execute(
                "INSERT OR IGNORE INTO sync_tomb (uid, stamp) VALUES (?1, ?2)",
                params![uid, op.stamp as i64],
            )?;
        }
        tx.execute(
            "INSERT INTO sync_unwatch (anilist_id, episode, stamp, device) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(anilist_id, episode) DO UPDATE SET stamp = excluded.stamp,
                device = excluded.device",
            params![anilist_id, episode, op.stamp as i64, op.device],
        )?;
        Ok(())
    }

    /// Apply ops from other devices, in one transaction. Idempotent and order-independent across
    /// devices; ops of one device apply in number order, and one that skips a number waits.
    pub fn apply_ops(&self, mut ops: Vec<Op>) -> Result<Applied> {
        ops.sort_by(|a, b| (&a.device, a.seq).cmp(&(&b.device, b.seq)));
        let now_secs = self.sync.wall_ms() / 1000;
        let mut report = self.with_personal_tx(|tx| {
            if !is_on(tx)? {
                return Err(crate::StoreError::Missing("sync is turned on".into()));
            }
            let mut report = Applied::default();
            let mut have: BTreeMap<String, u64> = BTreeMap::new();
            let mut titles: BTreeSet<u32> = BTreeSet::new();
            for op in ops {
                let held = match have.get(&op.device) {
                    Some(h) => *h,
                    None => {
                        let h: Option<i64> = tx.query_row(
                            "SELECT MAX(seq) FROM sync_op WHERE device = ?1",
                            [&op.device],
                            |r| r.get(0),
                        )?;
                        h.unwrap_or(0) as u64
                    }
                };
                if op.seq <= held {
                    have.insert(op.device.clone(), held);
                    report.duplicates += 1;
                    continue;
                }
                if op.seq != held + 1 {
                    have.insert(op.device.clone(), held);
                    report.gaps += 1;
                    continue;
                }
                have.insert(op.device.clone(), op.seq);
                let json = serde_json::to_string(&op.body)
                    .map_err(|source| crate::StoreError::Encode { what: "sync op", source })?;
                tx.execute(
                    "INSERT INTO sync_op (device, seq, stamp, body, received)
                     VALUES (?1, ?2, ?3, ?4, 1)",
                    params![op.device, op.seq as i64, op.stamp as i64, json],
                )?;
                log::raise_hlc(tx, op.stamp)?;
                apply_one(tx, &op, now_secs, &mut report, &mut titles)?;
                report.ops += 1;
            }
            for id in &titles {
                refresh_title(tx, *id)?;
            }
            report.titles = titles.into_iter().collect();
            Ok(report)
        })?;
        let device = std::mem::take(&mut report.device);
        if !device.is_empty() {
            self.with_conn(|c| {
                for write in &device {
                    match write {
                        super::op::DeviceWrite::ClearResolution { anilist_id, provider_id } => {
                            c.execute(
                                "DELETE FROM mapping_resolution
                                  WHERE anilist_id = ?1 AND provider_id = ?2",
                                params![anilist_id, provider_id],
                            )?;
                        }
                        super::op::DeviceWrite::LetGo { anilist_id, episode, at: Some(at) } => {
                            c.execute(
                                "INSERT OR REPLACE INTO download_dismissed (anilist_id, episode, at)
                                 VALUES (?1, ?2, ?3)",
                                params![anilist_id, episode, at],
                            )?;
                        }
                        super::op::DeviceWrite::LetGo { anilist_id, episode, at: None } => {
                            c.execute(
                                "DELETE FROM download_dismissed
                                  WHERE anilist_id = ?1 AND episode = ?2",
                                params![anilist_id, episode],
                            )?;
                        }
                    }
                }
                Ok(())
            })?;
        }
        Ok(report)
    }

    /// Replace superseded register ops that every device has acknowledged (`acked`, the smallest
    /// vector among them) with no-ops that keep their numbers, so what is kept stays small.
    /// Watch events are never touched. Returns how many ops were rewritten.
    pub fn compact(&self, acked: &VersionVector) -> Result<usize> {
        self.with_personal_tx(|tx| {
            let mut changed = 0;
            let candidates: Vec<Op> = {
                let mut stmt = tx.prepare(
                    "SELECT device, seq, stamp, body FROM sync_op WHERE body LIKE '%\"k\":\"reg\"%'",
                )?;
                stmt.query_map([], log::row_to_op)?.collect::<rusqlite::Result<_>>()?
            };
            for op in candidates {
                if op.seq > acked.get(&op.device).copied().unwrap_or(0) {
                    continue;
                }
                let OpBody::Reg { kind, key, .. } = &op.body else { continue };
                let current: Option<(i64, String)> = tx
                    .query_row(
                        "SELECT stamp, device FROM sync_reg WHERE kind = ?1 AND key = ?2",
                        [kind, key],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let live = current.is_some_and(|(s, d)| s as u64 == op.stamp && d == op.device);
                if !live {
                    tx.execute(
                        "UPDATE sync_op SET body = '{\"k\":\"noop\"}' WHERE device = ?1 AND seq = ?2",
                        params![op.device, op.seq as i64],
                    )?;
                    changed += 1;
                }
            }
            Ok(changed)
        })
    }

    /// Watches kept against an unwatch they had not seen, newer than `since` (unix seconds).
    pub fn sync_notes(&self, since: i64) -> Result<Vec<SyncNote>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT id, anilist_id, episode, unwatch_device, watch_device, at
                   FROM sync_note WHERE at >= ?1 ORDER BY id",
            )?;
            Ok(stmt
                .query_map([since], |r| {
                    Ok(SyncNote {
                        id: r.get(0)?,
                        anilist_id: r.get(1)?,
                        episode: r.get(2)?,
                        unwatch_device: r.get(3)?,
                        watch_device: r.get(4)?,
                        at: r.get(5)?,
                    })
                })?
                .collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn dismiss_sync_note(&self, id: i64) -> Result<()> {
        self.with_personal(|c| {
            Ok(c.execute("DELETE FROM sync_note WHERE id = ?1", [id]).map(|_| ())?)
        })
    }

    /// What other devices hold downloaded: `(device, anilist id, episode)`.
    pub fn remote_downloads(&self) -> Result<Vec<(String, u32, String)>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT device, anilist_id, episode FROM sync_remote_download
                  WHERE done = 1 ORDER BY device, anilist_id, episode",
            )?;
            Ok(stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<rusqlite::Result<_>>()?)
        })
    }

    /// The highest episode any device saw a tracker hold for a title.
    pub fn sync_tracker_mark_for(&self, tracker: &str, anilist_id: u32) -> Result<Option<u32>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT episode FROM sync_mark WHERE tracker = ?1 AND anilist_id = ?2",
                params![tracker, anilist_id],
                |r| r.get(0),
            )
            .optional()?)
        })
    }
}

fn parse_key(key: &str) -> Option<(u32, &str)> {
    let (id, rest) = key.split_once('/')?;
    Some((id.parse().ok()?, rest))
}

fn apply_one(
    tx: &Transaction<'_>,
    op: &Op,
    now_secs: i64,
    report: &mut Applied,
    titles: &mut BTreeSet<u32>,
) -> Result<()> {
    let at_of = |stamp: Stamp| op::secs_of_stamp(stamp);
    match &op.body {
        OpBody::Noop => {}
        OpBody::Watch {
            anilist_id,
            episode,
            position_secs,
            duration_secs,
            watched_secs,
            provider_id,
            translation,
            completed,
            at,
            seen_unwatch,
            queued_for,
        } => {
            let uid = format!("{}:{}", op.device, op.seq);
            let inserted = tx.execute(
                "INSERT OR IGNORE INTO watch_event
                   (anilist_id, episode, position_secs, duration_secs, watched_secs,
                    provider_id, translation, completed, at, uid, stamp)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11
                  WHERE NOT EXISTS (SELECT 1 FROM sync_tomb WHERE uid = ?10)",
                params![
                    anilist_id,
                    episode,
                    position_secs,
                    duration_secs,
                    watched_secs,
                    provider_id,
                    translation,
                    *completed as i32,
                    at,
                    uid,
                    op.stamp as i64
                ],
            )?;
            titles.insert(*anilist_id);
            if inserted > 0 && *completed {
                report.watches.push(ReceivedWatch {
                    device: op.device.clone(),
                    anilist_id: *anilist_id,
                    episode: episode.clone(),
                    queued_for: queued_for.clone(),
                });
                // An unwatch another device wrote, later than this watch, that did not name it.
                let unwatch: Option<(i64, String)> = tx
                    .query_row(
                        "SELECT stamp, device FROM sync_unwatch WHERE anilist_id = ?1 AND episode = ?2",
                        params![anilist_id, episode],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                if let Some((stamp, device)) = unwatch
                    && stamp as u64 > *seen_unwatch
                    && device != op.device
                {
                    note(tx, report, *anilist_id, episode, &device, &op.device, now_secs)?;
                }
            }
        }
        OpBody::Unwatch { anilist_id, episode, removes } => {
            for uid in removes {
                tx.execute(
                    "INSERT OR IGNORE INTO sync_tomb (uid, stamp) VALUES (?1, ?2)",
                    params![uid, op.stamp as i64],
                )?;
                tx.execute("DELETE FROM watch_event WHERE uid = ?1", [uid])?;
            }
            tx.execute(
                "INSERT INTO sync_unwatch (anilist_id, episode, stamp, device) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(anilist_id, episode) DO UPDATE SET stamp = excluded.stamp,
                    device = excluded.device
                  WHERE excluded.stamp > sync_unwatch.stamp",
                params![anilist_id, episode, op.stamp as i64, op.device],
            )?;
            // Completed watches by other devices that this unwatch never named.
            let kept: Vec<(String, i64)> = {
                let mut stmt = tx.prepare(&format!(
                    "SELECT uid, {EFFECTIVE} FROM watch_event
                      WHERE anilist_id = ?1 AND episode = ?2 AND completed = 1
                        AND uid IS NOT NULL AND uid NOT LIKE 'a:%'"
                ))?;
                stmt.query_map(params![anilist_id, episode], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<rusqlite::Result<_>>()?
            };
            for (uid, _) in kept {
                let watcher = uid.split_once(':').map_or("", |(d, _)| d).to_owned();
                if watcher != op.device && watch_seen_unwatch(tx, &uid)? < op.stamp {
                    note(tx, report, *anilist_id, episode, &op.device, &watcher, now_secs)?;
                }
            }
            titles.insert(*anilist_id);
        }
        OpBody::Adopt { anilist_id, upto, at } => {
            insert_adopted(
                tx,
                *anilist_id,
                *upto,
                *at,
                op.stamp,
                &format!("a:{}:{}", op.device, op.seq),
            )?;
            titles.insert(*anilist_id);
        }
        OpBody::Reg { kind, key, value } => {
            if beats(tx, kind, key, op.stamp, &op.device)? {
                set_reg(tx, kind, key, op.stamp, &op.device, value.as_deref())?;
                apply_reg(tx, kind, key, value.as_deref(), at_of(op.stamp), report, titles)?;
            }
        }
        OpBody::TrackerMark { tracker, anilist_id, episode } => {
            tx.execute(
                "INSERT INTO sync_mark (tracker, anilist_id, episode) VALUES (?1, ?2, ?3)
                 ON CONFLICT(tracker, anilist_id) DO UPDATE SET episode = MAX(episode, ?3)",
                params![tracker, anilist_id, episode],
            )?;
        }
        OpBody::DownloadState { anilist_id, episode, done } => {
            tx.execute(
                "INSERT INTO sync_remote_download (device, anilist_id, episode, done, stamp)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(device, anilist_id, episode) DO UPDATE SET
                    done = excluded.done, stamp = excluded.stamp
                  WHERE excluded.stamp > sync_remote_download.stamp",
                params![op.device, anilist_id, episode, *done as i32, op.stamp as i64],
            )?;
        }
        OpBody::DeviceAdd { device, name, pubkey } => {
            tx.execute(
                "INSERT INTO sync_peer (device, name, pubkey, added_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(device) DO UPDATE SET pubkey = excluded.pubkey,
                    name = COALESCE(name, excluded.name)",
                params![device, name, pubkey, now_secs],
            )?;
        }
        OpBody::DeviceRemove { device } => {
            // Sticky: no later add brings it back.
            tx.execute(
                "INSERT INTO sync_peer (device, removed) VALUES (?1, 1)
                 ON CONFLICT(device) DO UPDATE SET removed = 1",
                [device],
            )?;
        }
    }
    Ok(())
}

/// The unwatch a watch's device had seen, read from the op that made the event.
fn watch_seen_unwatch(tx: &Transaction<'_>, uid: &str) -> rusqlite::Result<Stamp> {
    let Some((device, seq)) = uid.split_once(':') else { return Ok(0) };
    let body: Option<String> = tx
        .query_row(
            "SELECT body FROM sync_op WHERE device = ?1 AND seq = ?2",
            params![device, seq.parse::<i64>().unwrap_or(0)],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match body.and_then(|b| serde_json::from_str::<OpBody>(&b).ok()) {
        Some(OpBody::Watch { seen_unwatch, .. }) => seen_unwatch,
        _ => 0,
    })
}

fn note(
    tx: &Transaction<'_>,
    report: &mut Applied,
    anilist_id: u32,
    episode: &str,
    unwatch_device: &str,
    watch_device: &str,
    now: i64,
) -> rusqlite::Result<()> {
    let added = tx.execute(
        "INSERT OR IGNORE INTO sync_note (anilist_id, episode, unwatch_device, watch_device, at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![anilist_id, episode, unwatch_device, watch_device, now],
    )?;
    if added > 0 {
        report.notes.push(SyncNote {
            id: tx.last_insert_rowid(),
            anilist_id,
            episode: episode.to_owned(),
            unwatch_device: unwatch_device.to_owned(),
            watch_device: watch_device.to_owned(),
            at: now,
        });
    }
    Ok(())
}

/// Insert a completed event for each of `1..=upto`, ids `<prefix>:<n>`, skipping tombstoned ones.
pub(crate) fn insert_adopted(
    tx: &Transaction<'_>,
    anilist_id: u32,
    upto: u32,
    at: i64,
    stamp: Stamp,
    prefix: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO watch_event
           (anilist_id, episode, position_secs, duration_secs, watched_secs,
            provider_id, translation, completed, at, uid, stamp)
         WITH RECURSIVE series(n) AS (
             SELECT 1 UNION ALL SELECT n + 1 FROM series WHERE n < ?2
         )
         SELECT ?1, CAST(n AS TEXT), 0.0, NULL, 0.0, 'tracker', NULL, 1, ?3,
                ?5 || ':' || n, ?4
           FROM series
          WHERE NOT EXISTS (SELECT 1 FROM sync_tomb WHERE uid = ?5 || ':' || n)",
        params![anilist_id, upto, at, stamp as i64, prefix],
    )?;
    Ok(())
}

fn apply_reg(
    tx: &Transaction<'_>,
    kind: &str,
    key: &str,
    value: Option<&str>,
    at: i64,
    report: &mut Applied,
    titles: &mut BTreeSet<u32>,
) -> Result<()> {
    match kind {
        kinds::HIDE => {
            if let Ok(id) = key.parse() {
                titles.insert(id);
            }
        }
        kinds::STATUS | kinds::SCORE => {}
        kinds::SHOW_SETTING => {
            if let Some((id, setting)) = parse_key(key) {
                match value {
                    Some(v) => tx.execute(
                        "INSERT INTO show_setting (anilist_id, setting, value, updated_at)
                         VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT(anilist_id, setting) DO UPDATE SET
                            value = excluded.value, updated_at = excluded.updated_at",
                        params![id, setting, v, at],
                    )?,
                    None => tx.execute(
                        "DELETE FROM show_setting WHERE anilist_id = ?1 AND setting = ?2",
                        params![id, setting],
                    )?,
                };
            }
        }
        kinds::TRACK_MEMORY => {
            if let Some((id, translation)) = parse_key(key) {
                match value {
                    Some(v) => tx.execute(
                        "INSERT INTO track_memory (anilist_id, translation, memory, updated_at)
                         VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT(anilist_id, translation) DO UPDATE SET
                            memory = excluded.memory, updated_at = excluded.updated_at",
                        params![id, translation, v, at],
                    )?,
                    None => tx.execute(
                        "DELETE FROM track_memory WHERE anilist_id = ?1 AND translation = ?2",
                        params![id, translation],
                    )?,
                };
            }
        }
        kinds::PIN => {
            if let Ok(id) = key.parse::<u32>() {
                match value.and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok()) {
                    Some(v) => {
                        tx.execute(
                            "INSERT INTO provider_preference
                                (anilist_id, provider_id, created_at, release_id, release_title)
                             VALUES (?1, ?2, ?3, ?4, ?5)
                             ON CONFLICT(anilist_id) DO UPDATE SET
                                provider_id = excluded.provider_id,
                                release_id = excluded.release_id,
                                release_title = excluded.release_title,
                                created_at = excluded.created_at",
                            params![
                                id,
                                v["p"].as_str().unwrap_or_default(),
                                at,
                                v["r"].as_str(),
                                v["t"].as_str()
                            ],
                        )?;
                    }
                    None => {
                        tx.execute(
                            "DELETE FROM provider_preference WHERE anilist_id = ?1",
                            [id],
                        )?;
                    }
                }
            }
        }
        kinds::MATCH => {
            if let Some((id, provider)) = parse_key(key) {
                match value.and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok()) {
                    Some(v) => {
                        tx.execute(
                            "INSERT INTO mapping_override
                                (anilist_id, provider_id, provider_key, created_at, title)
                             VALUES (?1, ?2, ?3, ?4, ?5)
                             ON CONFLICT(anilist_id, provider_id) DO UPDATE SET
                                provider_key = excluded.provider_key,
                                created_at = excluded.created_at, title = excluded.title",
                            params![
                                id,
                                provider,
                                v["k"].as_str().unwrap_or_default(),
                                at,
                                v["t"].as_str()
                            ],
                        )?;
                    }
                    None => {
                        tx.execute(
                            "DELETE FROM mapping_override WHERE anilist_id = ?1 AND provider_id = ?2",
                            params![id, provider],
                        )?;
                        report.device.push(super::op::DeviceWrite::ClearResolution {
                            anilist_id: i64::from(id),
                            provider_id: provider.to_owned(),
                        });
                    }
                }
            }
        }
        kinds::LET_GO => {
            if let Some((id, episode)) = parse_key(key) {
                report.device.push(super::op::DeviceWrite::LetGo {
                    anilist_id: i64::from(id),
                    episode: episode.to_owned(),
                    at: value.map(|_| at),
                });
            }
        }
        kinds::DEVICE_NAME => {
            tx.execute(
                "INSERT INTO sync_peer (device, name) VALUES (?1, ?2)
                 ON CONFLICT(device) DO UPDATE SET name = excluded.name",
                params![key, value],
            )?;
        }
        kinds::DEVICE_TRACKERS => {
            tx.execute(
                "INSERT INTO sync_peer (device, trackers) VALUES (?1, ?2)
                 ON CONFLICT(device) DO UPDATE SET trackers = excluded.trackers",
                params![key, value.unwrap_or("[]")],
            )?;
        }
        // The engine reads it from the register when it needs it.
        kinds::DEVICE_CLOUD => {}
        other => report.effects.push(Effect {
            kind: other.to_owned(),
            key: key.to_owned(),
            value: value.map(str::to_owned),
        }),
    }
    Ok(())
}
