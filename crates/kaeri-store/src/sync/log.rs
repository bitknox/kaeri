//! Writing ops, the version vector, and reading the log back.

use rusqlite::{OptionalExtension, Transaction, params};

use super::{Op, OpBody, Stamp, Store, VersionVector, op};
use crate::{Result, StoreError};

pub(crate) fn meta(tx: &rusqlite::Connection, key: &str) -> rusqlite::Result<Option<String>> {
    tx.query_row("SELECT value FROM sync_meta WHERE key = ?1", [key], |r| r.get(0)).optional()
}

pub(crate) fn set_meta(
    tx: &rusqlite::Connection,
    key: &str,
    value: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO sync_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

/// The device id sync writes under, when sync is on.
pub(crate) fn device(c: &rusqlite::Connection) -> rusqlite::Result<Option<String>> {
    if meta(c, "enabled")?.as_deref() != Some("1") {
        return Ok(None);
    }
    meta(c, "device")
}

pub(crate) fn hlc(c: &rusqlite::Connection) -> rusqlite::Result<Stamp> {
    Ok(meta(c, "hlc")?.and_then(|v| v.parse().ok()).unwrap_or(0))
}

pub(crate) fn raise_hlc(c: &rusqlite::Connection, stamp: Stamp) -> rusqlite::Result<()> {
    if stamp > hlc(c)? {
        set_meta(c, "hlc", &stamp.to_string())?;
    }
    Ok(())
}

/// Write an op with `stamp` as the next number of `device`'s.
pub(crate) fn push_op(
    tx: &Transaction<'_>,
    device: &str,
    stamp: Stamp,
    body: &OpBody,
) -> Result<Op> {
    let seq: u64 = meta(tx, "seq")?.and_then(|v| v.parse().ok()).unwrap_or(0) + 1;
    let json = serde_json::to_string(body)
        .map_err(|source| StoreError::Encode { what: "sync op", source })?;
    tx.execute(
        "INSERT INTO sync_op (device, seq, stamp, body, received) VALUES (?1, ?2, ?3, ?4, 0)",
        params![device, seq as i64, stamp as i64, json],
    )?;
    set_meta(tx, "seq", &seq.to_string())?;
    raise_hlc(tx, stamp)?;
    Ok(Op { device: device.to_owned(), seq, stamp, body: body.clone() })
}

pub(crate) fn row_to_op(r: &rusqlite::Row<'_>) -> rusqlite::Result<Op> {
    let body: String = r.get(3)?;
    Ok(Op {
        device: r.get(0)?,
        seq: r.get::<_, i64>(1)? as u64,
        stamp: r.get::<_, i64>(2)? as u64,
        body: serde_json::from_str(&body).unwrap_or(OpBody::Noop),
    })
}

impl Store {
    /// The id this device writes ops under, when sync is on.
    pub fn sync_device(&self) -> Result<Option<String>> {
        self.with_personal(|c| Ok(device(c)?))
    }

    /// The id this device wrote ops under, whether sync is on or not: the one
    /// [`Self::sync_enable`] takes again after sync was turned off.
    pub fn sync_device_kept(&self) -> Result<Option<String>> {
        self.with_personal(|c| Ok(meta(c, "device")?))
    }

    /// Turn sync on for this device as `device_id`, and turn today's history and choices into the
    /// device's first ops, in one transaction. Idempotent: calling it again, even after
    /// [`Self::sync_disable`], keeps the log and its numbering.
    pub fn sync_enable(&self, device_id: &str) -> Result<()> {
        let wall = self.sync.wall_ms();
        // The let-go list is the device's table; it joins the first ops from here.
        let dismissed: Vec<(i64, String, i64)> = self.with_conn(|c| {
            let mut stmt =
                c.prepare("SELECT anilist_id, episode, at FROM download_dismissed")?;
            Ok(stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<rusqlite::Result<_>>()?)
        })?;
        self.with_personal_tx(|tx| {
            if let Some(existing) = meta(tx, "device")?
                && existing != device_id
            {
                return Err(StoreError::UnsupportedFormat(format!(
                    "sync already belongs to device {existing}"
                )));
            }
            let first = meta(tx, "backfilled")?.is_none();
            set_meta(tx, "device", device_id)?;
            set_meta(tx, "enabled", "1")?;
            if first {
                backfill(tx, device_id, wall, &dismissed)?;
                set_meta(tx, "backfilled", "1")?;
            }
            Ok(())
        })
    }

    /// Stop writing ops. Everything held stays, so turning sync on again carries on.
    pub fn sync_disable(&self) -> Result<()> {
        self.with_personal(|c| Ok(set_meta(c, "enabled", "0")?))
    }

    /// Start over as `device_id`, a new id, with sync off: for a device its group removed, whose
    /// old id the group keeps removed for good. Everything held stays, and the old id's ops stay
    /// in the log; [`Self::sync_enable`] with the new id then makes today's history and choices
    /// that id's first ops, as on a device new to sync.
    pub fn sync_new_device(&self, device_id: &str) -> Result<()> {
        self.with_personal_tx(|tx| {
            set_meta(tx, "device", device_id)?;
            set_meta(tx, "enabled", "0")?;
            // The new id's ops count from one, as the others expect of a device new to them.
            set_meta(tx, "seq", "0")?;
            tx.execute("DELETE FROM sync_meta WHERE key = 'backfilled'", [])?;
            Ok(())
        })
    }

    /// Write an op for a change made in `tx`. `None` when sync is off.
    pub(crate) fn emit(&self, tx: &Transaction<'_>, body: OpBody) -> Result<Option<Op>> {
        let Some(device) = device(tx)? else { return Ok(None) };
        let stamp = op::tick(hlc(tx)?, self.sync.wall_ms());
        push_op(tx, &device, stamp, &body).map(Some)
    }

    /// Write a register change made in `tx`: the op, and the register's own record of it.
    pub(crate) fn emit_reg(
        &self,
        tx: &Transaction<'_>,
        kind: &str,
        key: &str,
        value: Option<&str>,
    ) -> Result<Option<Op>> {
        let body = OpBody::Reg {
            kind: kind.to_owned(),
            key: key.to_owned(),
            value: value.map(str::to_owned),
        };
        let Some(op) = self.emit(tx, body)? else { return Ok(None) };
        super::apply::set_reg(tx, kind, key, op.stamp, &op.device, value)?;
        Ok(Some(op))
    }

    /// Record a register change the engine made (a shared setting, the source order, a source
    /// switched off, a plugin store joined). No-op when sync is off. `at_ms` dates it, for a
    /// value that was set before sync was on; `None` is now.
    pub fn sync_register(
        &self,
        kind: &str,
        key: &str,
        value: Option<&str>,
        at_ms: Option<i64>,
    ) -> Result<()> {
        self.with_personal_tx(|tx| {
            let Some(device) = device(tx)? else { return Ok(()) };
            let stamp = match at_ms {
                // A dated value never goes ahead of the clock's own stamps, and never replaces
                // a newer one: only the register's rule decides.
                Some(ms) => op::tick(0, ms),
                None => op::tick(hlc(tx)?, self.sync.wall_ms()),
            };
            let body = OpBody::Reg {
                kind: kind.to_owned(),
                key: key.to_owned(),
                value: value.map(str::to_owned),
            };
            let op = push_op(tx, &device, stamp, &body)?;
            super::apply::set_reg(tx, kind, key, op.stamp, &op.device, value)?;
            Ok(())
        })
    }

    /// Say what this device holds downloaded.
    pub fn sync_download_state(
        &self,
        anilist_id: u32,
        episode: &str,
        done: bool,
    ) -> Result<()> {
        self.with_personal_tx(|tx| {
            self.emit(
                tx,
                OpBody::DownloadState { anilist_id, episode: episode.to_owned(), done },
            )?;
            Ok(())
        })
    }

    /// Say a tracker holds `episode` of a title.
    pub fn sync_tracker_mark(
        &self,
        tracker: &str,
        anilist_id: u32,
        episode: u32,
    ) -> Result<()> {
        self.with_personal_tx(|tx| {
            let Some(_) = self.emit(
                tx,
                OpBody::TrackerMark { tracker: tracker.to_owned(), anilist_id, episode },
            )?
            else {
                return Ok(());
            };
            tx.execute(
                "INSERT INTO sync_mark (tracker, anilist_id, episode) VALUES (?1, ?2, ?3)
                 ON CONFLICT(tracker, anilist_id) DO UPDATE SET episode = MAX(episode, ?3)",
                params![tracker, anilist_id, episode],
            )?;
            Ok(())
        })
    }

    /// The highest op number held from each device, this one included.
    pub fn version_vector(&self) -> Result<VersionVector> {
        self.with_personal(|c| {
            let mut stmt = c.prepare("SELECT device, MAX(seq) FROM sync_op GROUP BY device")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            let mut vector = VersionVector::new();
            for row in rows {
                let (device, seq) = row?;
                vector.insert(device, seq as u64);
            }
            Ok(vector)
        })
    }

    /// Every op the other side lacks, going by its `vector`, oldest first per device, at most
    /// `limit`. Includes ops held from other devices: a device relays what it holds.
    pub fn ops_after(&self, vector: &VersionVector, limit: usize) -> Result<Vec<Op>> {
        self.with_personal(|c| {
            let mut devices =
                c.prepare("SELECT DISTINCT device FROM sync_op ORDER BY device")?;
            let devices: Vec<String> =
                devices.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            let mut out = Vec::new();
            for device in devices {
                if out.len() >= limit {
                    break;
                }
                let have = vector.get(&device).copied().unwrap_or(0) as i64;
                let mut stmt = c.prepare(
                    "SELECT device, seq, stamp, body FROM sync_op
                      WHERE device = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3",
                )?;
                let rows = stmt
                    .query_map(params![device, have, (limit - out.len()) as i64], row_to_op)?;
                for row in rows {
                    out.push(row?);
                }
            }
            Ok(out)
        })
    }

    /// A register's value, when it is set.
    pub fn sync_reg_value(&self, kind: &str, key: &str) -> Result<Option<String>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT value FROM sync_reg WHERE kind = ?1 AND key = ?2",
                [kind, key],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
        })
    }

    /// Every set register of a kind, as `(key, value)`.
    pub fn sync_regs(&self, kind: &str) -> Result<Vec<(String, String)>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT key, value FROM sync_reg WHERE kind = ?1 AND value IS NOT NULL ORDER BY key",
            )?;
            Ok(stmt
                .query_map([kind], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?)
        })
    }

    /// Set a title's status or score as the person set it, keeping the register's rule.
    pub fn set_synced_status(&self, anilist_id: u32, status: Option<&str>) -> Result<()> {
        self.with_personal_tx(|tx| {
            self.emit_reg(tx, op::kinds::STATUS, &anilist_id.to_string(), status)?;
            Ok(())
        })
    }

    pub fn set_synced_score(&self, anilist_id: u32, score: Option<f32>) -> Result<()> {
        self.with_personal_tx(|tx| {
            self.emit_reg(
                tx,
                op::kinds::SCORE,
                &anilist_id.to_string(),
                score.map(|s| s.to_string()).as_deref(),
            )?;
            Ok(())
        })
    }

    /// The status and score other devices (or this one) set for a title.
    pub fn synced_status(&self, anilist_id: u32) -> Result<Option<String>> {
        self.sync_reg_value(op::kinds::STATUS, &anilist_id.to_string())
    }

    pub fn synced_score(&self, anilist_id: u32) -> Result<Option<f32>> {
        Ok(self
            .sync_reg_value(op::kinds::SCORE, &anilist_id.to_string())?
            .and_then(|s| s.parse().ok()))
    }

    /// Which device a title's newest event came from, and when: for "stopped at 12:04 on Study
    /// Mac". `None` for a title with no events, or whose newest came from a tracker.
    pub fn last_event_origin(&self, anilist_id: u32) -> Result<Option<(String, i64)>> {
        self.with_personal(|c| {
            let row: Option<(Option<String>, i64)> = c
                .query_row(
                    "SELECT uid, at FROM watch_event
                      WHERE anilist_id = ?1 AND provider_id IS NOT 'tracker'
                      ORDER BY COALESCE(stamp, at * 65536000) DESC, COALESCE(uid, '') DESC LIMIT 1",
                    [anilist_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            Ok(row.and_then(|(uid, at)| {
                uid.and_then(|u| u.split_once(':').map(|(d, _)| (d.to_owned(), at)))
            }))
        })
    }

    /// A bookkeeping value the transports keep beside the log (a read cursor, a published
    /// number). Not synced.
    pub fn sync_meta_get(&self, key: &str) -> Result<Option<String>> {
        self.with_personal(|c| Ok(meta(c, &format!("x.{key}"))?))
    }

    pub fn sync_meta_set(&self, key: &str, value: &str) -> Result<()> {
        self.with_personal(|c| Ok(set_meta(c, &format!("x.{key}"), value)?))
    }

    /// Forget a bookkeeping value.
    pub fn sync_meta_remove(&self, key: &str) -> Result<()> {
        self.with_personal(|c| {
            c.execute("DELETE FROM sync_meta WHERE key = ?1", [format!("x.{key}")])?;
            Ok(())
        })
    }

    /// Forget every bookkeeping value whose key starts with `prefix`, as when a transport's place
    /// changes and its cursors no longer mean anything.
    pub fn sync_meta_clear(&self, prefix: &str) -> Result<usize> {
        self.with_personal(|c| {
            let like = format!(
                "x.{}%",
                prefix.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
            );
            Ok(c.execute("DELETE FROM sync_meta WHERE key LIKE ?1 ESCAPE '\\'", [like])?)
        })
    }

    /// One device's ops after `after`, oldest first, at most `limit`.
    pub fn ops_of(&self, device: &str, after: u64, limit: usize) -> Result<Vec<Op>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT device, seq, stamp, body FROM sync_op
                  WHERE device = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3",
            )?;
            Ok(stmt
                .query_map(params![device, after as i64, limit as i64], row_to_op)?
                .collect::<rusqlite::Result<_>>()?)
        })
    }

    /// Note when a device was last heard from, in unix seconds.
    pub fn sync_touch_peer(&self, device: &str, at: i64) -> Result<()> {
        self.with_personal(|c| {
            c.execute(
                "INSERT INTO sync_peer (device, last_seen) VALUES (?1, ?2)
                 ON CONFLICT(device) DO UPDATE SET last_seen = MAX(COALESCE(last_seen, 0), ?2)",
                params![device, at],
            )?;
            Ok(())
        })
    }

    /// Say another device holds `trackers`, as its register would: for a fixture build that
    /// shows a device's trackers without the device writing them.
    pub fn sync_fixture_trackers(&self, device: &str, trackers: &[&str]) -> Result<()> {
        let json = serde_json::to_string(trackers)
            .map_err(|source| StoreError::Encode { what: "trackers", source })?;
        self.with_personal(|c| {
            c.execute(
                "INSERT INTO sync_peer (device, trackers) VALUES (?1, ?2)
                 ON CONFLICT(device) DO UPDATE SET trackers = excluded.trackers",
                params![device, json],
            )?;
            Ok(())
        })
    }

    /// Ops held, of every device.
    pub fn sync_op_count(&self) -> Result<u64> {
        self.with_personal(|c| {
            Ok(c.query_row("SELECT COUNT(*) FROM sync_op", [], |r| r.get::<_, i64>(0))? as u64)
        })
    }
}

/// Turn what the database holds into this device's first ops.
fn backfill(
    tx: &Transaction<'_>,
    device: &str,
    wall_ms: i64,
    dismissed: &[(i64, String, i64)],
) -> Result<()> {
    // Events, oldest first, each given the id its op will have.
    let events: Vec<(i64, OpBody, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT id, anilist_id, episode, position_secs, duration_secs, watched_secs,
                    provider_id, translation, completed, at
               FROM watch_event WHERE uid IS NULL ORDER BY at, id",
        )?;
        stmt.query_map([], |r| {
            let at: i64 = r.get(9)?;
            Ok((
                r.get::<_, i64>(0)?,
                OpBody::Watch {
                    anilist_id: r.get(1)?,
                    episode: r.get(2)?,
                    position_secs: r.get(3)?,
                    duration_secs: r.get(4)?,
                    watched_secs: r.get(5)?,
                    provider_id: r.get(6)?,
                    translation: r.get(7)?,
                    completed: r.get::<_, i64>(8)? != 0,
                    at,
                    queued_for: Vec::new(),
                    seen_unwatch: 0,
                },
                at,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?
    };
    for (id, body, at) in events {
        // Ordered by time, and the counter keeps two events of one second in order.
        let stamp = op::tick(hlc(tx)?, 0).max(op::stamp_of_secs(at));
        let op = push_op(tx, device, stamp, &body)?;
        tx.execute(
            "UPDATE watch_event SET uid = ?2, stamp = ?3 WHERE id = ?1",
            params![id, format!("{device}:{}", op.seq), stamp as i64],
        )?;
    }

    // Registers from the per-show tables, dated by their own `updated_at`.
    type Backfill = (String, String, Option<String>, i64);
    let mut regs: Vec<Backfill> = Vec::new();
    let mut collect = |sql: &str,
                       f: &dyn Fn(&rusqlite::Row<'_>) -> rusqlite::Result<Backfill>|
     -> rusqlite::Result<()> {
        let mut stmt = tx.prepare(sql)?;
        for row in stmt.query_map([], |r| f(r))? {
            regs.push(row?);
        }
        Ok(())
    };
    collect("SELECT anilist_id, setting, value, updated_at FROM show_setting", &|r| {
        Ok((
            op::kinds::SHOW_SETTING.into(),
            format!("{}/{}", r.get::<_, i64>(0)?, r.get::<_, String>(1)?),
            Some(r.get(2)?),
            r.get(3)?,
        ))
    })?;
    collect("SELECT anilist_id, translation, memory, updated_at FROM track_memory", &|r| {
        Ok((
            op::kinds::TRACK_MEMORY.into(),
            format!("{}/{}", r.get::<_, i64>(0)?, r.get::<_, String>(1)?),
            Some(r.get(2)?),
            r.get(3)?,
        ))
    })?;
    collect(
        "SELECT anilist_id, provider_id, release_id, release_title, created_at FROM provider_preference",
        &|r| {
            Ok((
                op::kinds::PIN.into(),
                r.get::<_, i64>(0)?.to_string(),
                Some(pin_json(&r.get::<_, String>(1)?, r.get(2)?, r.get(3)?)),
                r.get(4)?,
            ))
        },
    )?;
    collect(
        "SELECT anilist_id, provider_id, provider_key, title, created_at FROM mapping_override",
        &|r| {
            Ok((
                op::kinds::MATCH.into(),
                format!("{}/{}", r.get::<_, i64>(0)?, r.get::<_, String>(1)?),
                Some(match_json(&r.get::<_, String>(2)?, r.get(3)?)),
                r.get(4)?,
            ))
        },
    )?;
    for (anilist_id, episode, at) in dismissed {
        regs.push((
            op::kinds::LET_GO.into(),
            format!("{anilist_id}/{episode}"),
            Some("1".into()),
            *at,
        ));
    }
    for (kind, key, value, at) in regs {
        // Dated by when the choice was made, so a device that joins later with a newer choice
        // wins, and a long-forgotten one does not beat it.
        let stamp = op::tick(0, at * 1000);
        let body = OpBody::Reg { kind: kind.clone(), key: key.clone(), value: value.clone() };
        let op = push_op(tx, device, stamp, &body)?;
        super::apply::set_reg(tx, &kind, &key, op.stamp, device, value.as_deref())?;
    }

    // Titles hidden from Continue: hidden after their newest event.
    let hidden: Vec<(i64, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT p.anilist_id, COALESCE(MAX(e.stamp), p.updated_at * 65536000)
               FROM watch_progress p LEFT JOIN watch_event e ON e.anilist_id = p.anilist_id
              WHERE p.hidden = 1 GROUP BY p.anilist_id",
        )?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
    };
    for (id, newest) in hidden {
        let stamp = op::tick(newest as u64, wall_ms.min(0));
        let body = OpBody::Reg {
            kind: op::kinds::HIDE.into(),
            key: id.to_string(),
            value: Some("1".into()),
        };
        let op = push_op(tx, device, stamp, &body)?;
        super::apply::set_reg(
            tx,
            op::kinds::HIDE,
            &id.to_string(),
            op.stamp,
            device,
            Some("1"),
        )?;
    }
    Ok(())
}

pub(crate) fn pin_json(
    provider: &str,
    release: Option<String>,
    title: Option<String>,
) -> String {
    serde_json::json!({ "p": provider, "r": release, "t": title }).to_string()
}

pub(crate) fn match_json(key: &str, title: Option<String>) -> String {
    serde_json::json!({ "k": key, "t": title }).to_string()
}
