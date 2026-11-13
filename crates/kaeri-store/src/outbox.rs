//! Durable per-tracker sync queue.
//!
//! Deliberately a table and not an in-memory channel. Progress recorded on a plane, or
//! while AniList is having an outage, has to survive the process exiting — otherwise
//! "watched six episodes offline" silently evaporates, which is the single worst thing a
//! tracking client can do.
//!
//! Two properties make this safe to retry aggressively:
//!
//! - **Coalescing.** Progress is monotonic, so only the highest pending episode per title
//!   is worth sending. Bingeing twelve episodes offline leaves one queued op, not twelve.
//! - **Per-tracker cursors.** Each tracker drains independently, so a failing MAL push
//!   cannot stall AniList.

use kaeri_core::{
    ids::AnilistId,
    traits::{TrackOp, WatchStatus},
};

use rusqlite::OptionalExtension;

use crate::{Result, Store, StoreError};

/// Base delay for the retry backoff, in seconds.
const BACKOFF_BASE_SECS: i64 = 30;

/// Ceiling on the backoff. An hour is long enough to ride out an outage without leaving
/// a recovered tracker waiting for days.
///
/// Also the longest any wait in the queue may be: one set further off than this was set by a
/// clock that ran ahead, and is due at once.
pub const BACKOFF_MAX_SECS: i64 = 3_600;

/// Refusals after which an operation is abandoned.
///
/// Far away on purpose. Every discard loses a change the person made, so this must mean "no
/// amount of waiting will fix it" rather than "the network was down for a while". Only
/// [`Store::fail`] and [`Store::fail_read`] count toward it, and the drain calls them only for
/// a tracker's refusal of that one change: a network that is down, a tracker's own trouble, a
/// pause it asked for or a rejected credential never count, however long they last. At the
/// capped hourly backoff this is roughly two days of the tracker refusing the same change.
const MAX_ATTEMPTS: u32 = 50;

/// How many abandoned operations are remembered for the app to show.
const DROPPED_KEPT: usize = 100;

/// Where the abandoned operations are remembered, in `app_state`.
const DROPPED_KEY: &str = "outbox_dropped";

/// An operation the outbox gave up on: the change, and the tracker's last word on it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DroppedOp {
    pub tracker_id: String,
    pub op: TrackOp,
    /// The tracker's refusal, as it last said it.
    pub error: String,
    /// When it was given up, in seconds since the epoch.
    pub at: i64,
}

/// The claim query for `table`: due rows, oldest first, leaving out any row with an older row
/// for the same title still waiting. A later change to a title must never reach the tracker
/// before an earlier one that is backing off, or the earlier one would land last and undo it.
fn claim_sql(table: &str) -> String {
    // Due: its time has come, or it is further off than any wait the queue sets, which only a
    // clock that ran ahead when the wait was set leaves behind. Without that, a change whose push
    // failed while the clock read a year ahead waited that year after the clock was put right.
    let due = |row: &str| {
        format!(
            "({row}.next_retry <= ?2
              OR ({row}.next_retry > ?2 + {BACKOFF_MAX_SECS} AND {row}.next_retry <> {DEFERRED}))"
        )
    };
    format!(
        "SELECT id, tracker_id, op, attempts, created_at
           FROM {table} o
          WHERE tracker_id = ?1 AND {o_due}
            AND NOT EXISTS (
                SELECT 1 FROM {table} e
                 WHERE e.tracker_id = o.tracker_id AND e.anilist_id = o.anilist_id
                   AND NOT {e_due}
                   AND (e.created_at < o.created_at
                        OR (e.created_at = o.created_at AND e.id < o.id)))
          ORDER BY created_at ASC, id ASC
          LIMIT ?3",
        o_due = due("o"),
        e_due = due("e"),
    )
}

/// Count a refusal of the op `id` in `table`, and schedule its retry, or give it up and
/// remember it once it has had [`MAX_ATTEMPTS`]. True when it was given up.
fn refuse_in(
    c: &rusqlite::Connection,
    table: &str,
    id: i64,
    error: &str,
    now: i64,
) -> Result<bool> {
    let row: Option<(u32, String, String)> = c
        .query_row(
            &format!("SELECT attempts + 1, tracker_id, op FROM {table} WHERE id = ?1"),
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((attempts, tracker_id, op)) = row else { return Ok(false) };
    if attempts < MAX_ATTEMPTS {
        c.execute(
            &format!(
                "UPDATE {table} SET attempts = ?1, last_error = ?2, next_retry = ?3 + ?4
                  WHERE id = ?5"
            ),
            rusqlite::params![attempts, error, now, backoff_secs(attempts), id],
        )?;
        return Ok(false);
    }
    tracing::error!(id, attempts, %error, table, "giving up on a change the tracker keeps refusing");
    c.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [id])?;
    // Kept, so the change is not gone without a word: the app can say which change the
    // tracker would not take, and why.
    if let Ok(op) = serde_json::from_str::<TrackOp>(&op) {
        let mut kept = dropped_in(c)?;
        kept.push(DroppedOp { tracker_id, op, error: error.to_owned(), at: now });
        let over = kept.len().saturating_sub(DROPPED_KEPT);
        kept.drain(..over);
        let json = serde_json::to_string(&kept)
            .map_err(|source| StoreError::Encode { what: "dropped outbox ops", source })?;
        c.execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![DROPPED_KEY, json],
        )?;
    }
    Ok(true)
}

fn dropped_in(c: &rusqlite::Connection) -> Result<Vec<DroppedOp>> {
    let json: Option<String> = c
        .query_row("SELECT value FROM app_state WHERE key = ?1", [DROPPED_KEY], |r| r.get(0))
        .optional()?;
    Ok(json.and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default())
}

/// `next_retry` for an op held until the id mappings change.
///
/// A sentinel rather than a column: `claim_ready` already skips anything not yet due, so a
/// deferred row stays out of every batch with no change to the query and keeps its place in
/// the oldest-first order. It counts in [`Store::outbox_depth`], since it has not gone
/// anywhere, but not in [`Store::outbox_waiting`]: nothing will send it until a mapping
/// arrives, so the badge says it apart from what is on its way.
const DEFERRED: i64 = i64::MAX;

/// A queued operation.
#[derive(Debug, Clone, PartialEq)]
pub struct OutboxEntry {
    pub id: i64,
    pub tracker_id: String,
    pub op: TrackOp,
    pub attempts: u32,
    pub created_at: i64,
}

/// Delay before the next attempt after `attempts` failures.
///
/// Exponential with a cap, so a persistently broken tracker stops generating traffic but
/// still recovers on its own once it comes back.
pub fn backoff_secs(attempts: u32) -> i64 {
    BACKOFF_BASE_SECS.saturating_mul(1_i64 << attempts.min(10)).min(BACKOFF_MAX_SECS)
}

impl Store {
    /// Queue an operation for a tracker.
    ///
    /// `SetProgress` coalesces against any pending op for the same title: because progress
    /// only ever moves forward, a queued "episode 12" makes a queued "episode 7"
    /// meaningless. A lower episode is dropped outright rather than queued behind a
    /// higher one, which would otherwise walk a tracker's progress backwards.
    pub fn enqueue(&self, tracker_id: &str, op: &TrackOp, at: i64) -> Result<Option<i64>> {
        // A title Kaeri made up ids for, such as files kept unmatched, is on no tracker, so
        // nothing about it is ever queued. The demo's title has no AniList entry either.
        if op.anilist_id().is_own() || op.anilist_id() == kaeri_core::demo::DEMO_ANILIST_ID {
            return Ok(None);
        }
        let anilist_id = op.anilist_id().get();
        let payload = serde_json::to_string(op)
            .map_err(|source| StoreError::Encode { what: "TrackOp", source })?;
        if op.is_reading() {
            return self.enqueue_read(tracker_id, op, &payload, at);
        }

        self.with_personal_tx(|tx| {
            if let TrackOp::SetProgress { episode, .. } = op {
                let existing: Option<(i64, String)> = tx
                    .query_row(
                        "SELECT id, op FROM sync_outbox
                          WHERE tracker_id = ?1 AND anilist_id = ?2
                            AND json_extract(op, '$.op') = 'set_progress'
                          LIMIT 1",
                        rusqlite::params![tracker_id, anilist_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .ok();

                if let Some((existing_id, existing_json)) = existing {
                    let pending: TrackOp = serde_json::from_str(&existing_json)
                        .map_err(|source| StoreError::Encode { what: "TrackOp", source })?;
                    let pending_episode = match pending {
                        TrackOp::SetProgress { episode, .. } => episode,
                        _ => 0,
                    };
                    if *episode <= pending_episode {
                        // Already covered by a queued op at or beyond this episode.
                        return Ok(None);
                    }
                    // Only the payload changes. Resetting `attempts` and `next_retry` here
                    // meant a binge against a failing tracker re-armed an immediate retry
                    // after every episode, so the backoff this queue is built around never
                    // applied to the case it exists for; and moving `created_at` forward sent
                    // the row to the back of an oldest-first queue, so an actively-watched
                    // title could be starved behind titles nobody had touched in weeks.
                    //
                    // The row is still the same obligation — "tell the tracker where this
                    // title is" — and its history of failing to discharge it is worth keeping.
                    tx.execute(
                        "UPDATE sync_outbox SET op = ?1 WHERE id = ?2",
                        rusqlite::params![&payload, existing_id],
                    )?;
                    return Ok(Some(existing_id));
                }
            }

            tx.execute(
                "INSERT INTO sync_outbox (tracker_id, op, anilist_id, created_at, next_retry)
                 VALUES (?1, ?2, ?3, ?4, 0)",
                rusqlite::params![tracker_id, &payload, anilist_id, at],
            )?;
            Ok(Some(tx.last_insert_rowid()))
        })
    }

    /// Queue reading progress that takes a tracker's count down, when the person chose that the
    /// latest change wins: whatever progress is queued for the title gives way to it.
    pub fn requeue_read_progress(
        &self,
        tracker_id: &str,
        op: &TrackOp,
        at: i64,
    ) -> Result<Option<i64>> {
        if !matches!(op, TrackOp::SetReadProgress { .. })
            || op.anilist_id().is_own()
            || op.anilist_id() == kaeri_core::demo::DEMO_ANILIST_ID
        {
            return self.enqueue(tracker_id, op, at);
        }
        self.with_personal(|c| {
            c.execute(
                "DELETE FROM read_outbox
                  WHERE tracker_id = ?1 AND anilist_id = ?2
                    AND json_extract(op, '$.op') = 'set_read_progress'",
                rusqlite::params![tracker_id, op.anilist_id().get()],
            )?;
            Ok(())
        })?;
        self.enqueue(tracker_id, op, at)
    }

    /// Queue a reading op in its own outbox. Chapters and volumes only move forward: a queued
    /// progress takes the higher of each, and one that raises neither is dropped.
    fn enqueue_read(
        &self,
        tracker_id: &str,
        op: &TrackOp,
        payload: &str,
        at: i64,
    ) -> Result<Option<i64>> {
        let anilist_id = op.anilist_id().get();
        self.with_personal_tx(|tx| {
            if let TrackOp::SetReadProgress { anilist_id: id, chapter, volume } = op {
                let existing: Option<(i64, String)> = tx
                    .query_row(
                        "SELECT id, op FROM read_outbox
                          WHERE tracker_id = ?1 AND anilist_id = ?2
                            AND json_extract(op, '$.op') = 'set_read_progress'
                          LIMIT 1",
                        rusqlite::params![tracker_id, anilist_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                if let Some((existing_id, existing_json)) = existing {
                    let pending: TrackOp = serde_json::from_str(&existing_json)
                        .map_err(|source| StoreError::Encode { what: "TrackOp", source })?;
                    let (held_chapter, held_volume) = match pending {
                        TrackOp::SetReadProgress { chapter, volume, .. } => (chapter, volume),
                        _ => (0, None),
                    };
                    if *chapter <= held_chapter
                        && volume.unwrap_or(0) <= held_volume.unwrap_or(0)
                    {
                        return Ok(None);
                    }
                    let merged = TrackOp::SetReadProgress {
                        anilist_id: *id,
                        chapter: (*chapter).max(held_chapter),
                        volume: volume.max(&held_volume).or(held_volume),
                    };
                    let json = serde_json::to_string(&merged)
                        .map_err(|source| StoreError::Encode { what: "TrackOp", source })?;
                    tx.execute(
                        "UPDATE read_outbox SET op = ?1 WHERE id = ?2",
                        rusqlite::params![json, existing_id],
                    )?;
                    return Ok(Some(existing_id));
                }
            }
            tx.execute(
                "INSERT INTO read_outbox (tracker_id, op, anilist_id, created_at, next_retry)
                 VALUES (?1, ?2, ?3, ?4, 0)",
                rusqlite::params![tracker_id, payload, anilist_id, at],
            )?;
            Ok(Some(tx.last_insert_rowid()))
        })
    }

    /// Drop every reading op still waiting for any tracker about one title: it came off the
    /// lists, and a queued status or count would put it back. Says how many went.
    pub fn forget_read_ops(&self, anilist_id: kaeri_core::ids::AnilistId) -> Result<usize> {
        self.with_personal(|c| {
            Ok(c.execute("DELETE FROM read_outbox WHERE anilist_id = ?1", [anilist_id.get()])?)
        })
    }

    /// Reading ops waiting for a tracker, or for all of them.
    pub fn read_outbox_depth(&self, tracker_id: Option<&str>) -> Result<u32> {
        self.with_personal(|c| {
            Ok(match tracker_id {
                Some(t) => c.query_row(
                    "SELECT COUNT(*) FROM read_outbox WHERE tracker_id = ?1",
                    [t],
                    |r| r.get(0),
                )?,
                None => c.query_row("SELECT COUNT(*) FROM read_outbox", [], |r| r.get(0))?,
            })
        })
    }

    /// The reading ops queued for a tracker, oldest first.
    pub fn read_outbox(&self, tracker_id: &str) -> Result<Vec<TrackOp>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT op FROM read_outbox WHERE tracker_id = ?1 ORDER BY created_at, id",
            )?;
            let rows = stmt.query_map([tracker_id], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for json in rows {
                match serde_json::from_str(&json?) {
                    Ok(op) => out.push(op),
                    Err(e) => tracing::error!(error = %e, "an undecodable reading outbox row"),
                }
            }
            Ok(out)
        })
    }

    /// Reading ops due for sending, oldest first: [`Self::claim_ready`] for `read_outbox`.
    pub fn claim_ready_read(
        &self,
        tracker_id: &str,
        now: i64,
        limit: u32,
    ) -> Result<Vec<OutboxEntry>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(&claim_sql("read_outbox"))?;
            let rows = stmt.query_map(rusqlite::params![tracker_id, now, limit], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, u32>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?;
            let mut out = Vec::new();
            let mut undecodable: Vec<i64> = Vec::new();
            for row in rows {
                let (id, tracker, json, attempts, created_at) = row?;
                match serde_json::from_str(&json) {
                    Ok(op) => out.push(OutboxEntry { id, tracker_id: tracker, op, attempts, created_at }),
                    Err(e) => {
                        tracing::error!(id, error = %e, "discarding an undecodable reading outbox row");
                        undecodable.push(id);
                    }
                }
            }
            drop(stmt);
            for id in undecodable {
                c.execute("DELETE FROM read_outbox WHERE id = ?1", [id])?;
            }
            Ok(out)
        })
    }

    /// Remove a reading op that was sent.
    pub fn complete_read(&self, id: i64) -> Result<()> {
        self.with_personal(|c| {
            c.execute("DELETE FROM read_outbox WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// Count the tracker's refusal of a reading op, as [`Self::fail`] does. True when it was
    /// given up.
    pub fn fail_read(&self, id: i64, error: &str, now: i64) -> Result<bool> {
        self.with_personal_tx(|c| refuse_in(c, "read_outbox", id, error, now))
    }

    /// Hold a reading op until it can be addressed or the tracker may be asked again at `until`.
    /// With `None` it waits for [`Self::wake_deferred`], as a watching op waits for a mapping.
    pub fn defer_read(&self, id: i64, reason: &str, until: Option<i64>) -> Result<()> {
        self.with_personal(|c| {
            c.execute(
                "UPDATE read_outbox SET next_retry = ?1, last_error = ?2 WHERE id = ?3",
                rusqlite::params![until.unwrap_or(DEFERRED), reason, id],
            )?;
            Ok(())
        })
    }

    /// Reading ops held for a mapping, for the Accounts screen.
    pub fn read_outbox_deferred(&self, tracker_id: &str) -> Result<u32> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM read_outbox WHERE tracker_id = ?1 AND next_retry = ?2",
                rusqlite::params![tracker_id, DEFERRED],
                |r| r.get(0),
            )?)
        })
    }

    /// The highest chapter queued for a work on a tracker, if any.
    pub fn pending_read_progress(
        &self,
        tracker_id: &str,
        anilist_id: AnilistId,
    ) -> Result<Option<(u32, Option<u32>)>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT json_extract(op, '$.chapter'), json_extract(op, '$.volume')
                   FROM read_outbox
                  WHERE tracker_id = ?1 AND anilist_id = ?2
                    AND json_extract(op, '$.op') = 'set_read_progress'
                  LIMIT 1",
                rusqlite::params![tracker_id, anilist_id.get()],
                |r| Ok((r.get::<_, u32>(0)?, r.get::<_, Option<u32>>(1)?)),
            )
            .optional()?)
        })
    }

    /// Operations due for sending, oldest first.
    pub fn claim_ready(
        &self,
        tracker_id: &str,
        now: i64,
        limit: u32,
    ) -> Result<Vec<OutboxEntry>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(&claim_sql("sync_outbox"))?;
            let rows = stmt.query_map(rusqlite::params![tracker_id, now, limit], |r| {
                let json: String = r.get(2)?;
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    json,
                    r.get::<_, u32>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?;

            let mut out = Vec::new();
            let mut undecodable: Vec<i64> = Vec::new();
            for row in rows {
                let (id, tracker, json, attempts, created_at) = row?;
                match serde_json::from_str(&json) {
                    Ok(op) => out.push(OutboxEntry {
                        id,
                        tracker_id: tracker,
                        op,
                        attempts,
                        created_at,
                    }),
                    // A row we can no longer decode can never be sent — skipping it left it
                    // in the table forever, re-read on every claim and counted in the badge
                    // the user is waiting to see reach zero. Dropped instead, loudly.
                    Err(e) => {
                        tracing::error!(id, error = %e, "discarding an undecodable outbox row");
                        undecodable.push(id);
                    }
                }
            }
            drop(stmt);
            for id in undecodable {
                c.execute("DELETE FROM sync_outbox WHERE id = ?1", [id])?;
            }
            Ok(out)
        })
    }

    /// Remove a successfully-sent operation.
    pub fn complete(&self, id: i64) -> Result<()> {
        self.with_personal(|c| {
            c.execute("DELETE FROM sync_outbox WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// Count the tracker's refusal of an op and schedule a retry, or give up on one it keeps
    /// refusing. True when it was given up, which [`Self::dropped_track_ops`] then remembers.
    ///
    /// Without a ceiling, an operation that could never succeed, such as a title the tracker
    /// has deleted, would retry hourly for the life of the installation and hold the `⇅` badge
    /// above zero for good. Only refusals reach here: a failure that is not the tracker's word
    /// on this change waits through [`Self::postpone`] instead, and never counts.
    pub fn fail(&self, id: i64, error: &str, now: i64) -> Result<bool> {
        self.with_personal_tx(|c| refuse_in(c, "sync_outbox", id, error, now))
    }

    /// Hold an op until `until` without counting anything against it: a failure that says
    /// nothing about the change, such as the tracker's trouble with this one title.
    pub fn postpone(&self, id: i64, reason: &str, until: i64) -> Result<()> {
        self.with_personal(|c| {
            c.execute(
                "UPDATE sync_outbox SET next_retry = ?1, last_error = ?2 WHERE id = ?3",
                rusqlite::params![until, reason, id],
            )?;
            Ok(())
        })
    }

    /// The operations the outbox gave up on, oldest first, the last hundred of them: changes
    /// a tracker refused until the outbox stopped asking.
    pub fn dropped_track_ops(&self) -> Result<Vec<DroppedOp>> {
        self.with_personal(dropped_in)
    }

    /// Hold an op until the id mappings change.
    ///
    /// For an op the tracker cannot address yet: a title the mapping datasets carry no id
    /// for on that service. Retrying on a timer cannot help and deleting it loses the
    /// update, so it waits for [`Self::wake_deferred`], which every mapping refresh calls.
    /// Not counted as an attempt: nothing failed, and the row has to be ready to go the
    /// moment a mapping arrives rather than an hour into a backoff.
    pub fn defer(&self, id: i64, reason: &str) -> Result<()> {
        self.with_personal(|c| {
            c.execute(
                "UPDATE sync_outbox SET next_retry = ?1, last_error = ?2 WHERE id = ?3",
                rusqlite::params![DEFERRED, reason, id],
            )?;
            Ok(())
        })
    }

    /// Make every deferred op ready again, returning how many there were.
    ///
    /// Called by every mapping refresh. Public so that anything else which changes what the
    /// mappings can answer can say so too.
    pub fn wake_deferred(&self) -> Result<usize> {
        self.with_personal(wake_deferred_in)
    }

    /// Ops held for a mapping, for the Accounts screen.
    pub fn outbox_deferred(&self, tracker_id: &str) -> Result<u32> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM sync_outbox WHERE tracker_id = ?1 AND next_retry = ?2",
                rusqlite::params![tracker_id, DEFERRED],
                |r| r.get(0),
            )?)
        })
    }

    /// Ops that will go out once their tracker can be reached: the queue less what is held for
    /// a mapping. The number the `⇅` badge and the apps' sync status show as waiting.
    pub fn outbox_waiting(&self, tracker_id: &str) -> Result<u32> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM sync_outbox WHERE tracker_id = ?1 AND next_retry <> ?2",
                rusqlite::params![tracker_id, DEFERRED],
                |r| r.get(0),
            )?)
        })
    }

    /// Every queued op, held ones included: what a drain reports as remaining.
    pub fn outbox_depth(&self, tracker_id: Option<&str>) -> Result<u32> {
        self.with_personal(|c| {
            let depth = match tracker_id {
                Some(t) => c.query_row(
                    "SELECT COUNT(*) FROM sync_outbox WHERE tracker_id = ?1",
                    [t],
                    |r| r.get(0),
                )?,
                None => c.query_row("SELECT COUNT(*) FROM sync_outbox", [], |r| r.get(0))?,
            };
            Ok(depth)
        })
    }

    /// Highest queued progress for a title, if any. Used when reconciling with a remote
    /// value so an in-flight local push is not treated as absent.
    pub fn pending_progress(
        &self,
        tracker_id: &str,
        anilist_id: AnilistId,
    ) -> Result<Option<u32>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT MAX(CAST(json_extract(op, '$.episode') AS INTEGER))
                   FROM sync_outbox
                  WHERE tracker_id = ?1 AND anilist_id = ?2
                    AND json_extract(op, '$.op') = 'set_progress'",
                rusqlite::params![tracker_id, anilist_id.get()],
                |r| r.get::<_, Option<u32>>(0),
            )?)
        })
    }

    /// The statuses set here that are still on their way to `tracker_id`: for each title, the
    /// last one queued. The tracker's list says them once they are sent, so a list cut from
    /// its answer meanwhile shows them over what it says.
    pub fn pending_statuses(
        &self,
        tracker_id: &str,
    ) -> Result<std::collections::HashMap<AnilistId, WatchStatus>> {
        let ops: Vec<String> = self.with_personal(|c| {
            let mut statement = c.prepare(
                "SELECT op FROM sync_outbox
                  WHERE tracker_id = ?1 AND json_extract(op, '$.op') = 'set_status'
                  ORDER BY id",
            )?;
            let ops = statement
                .query_map([tracker_id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(ops)
        })?;
        // In the order queued, so the last one set for a title is the one kept.
        Ok(ops
            .iter()
            .filter_map(|op| match serde_json::from_str(op) {
                Ok(TrackOp::SetStatus { anilist_id, status, .. }) => Some((anilist_id, status)),
                _ => None,
            })
            .collect())
    }
}

/// [`Store::wake_deferred`] on a connection already held, so a mapping refresh can do it
/// inside its own transaction.
pub(crate) fn wake_deferred_in(c: &rusqlite::Connection) -> Result<usize> {
    let woken =
        c.execute("UPDATE sync_outbox SET next_retry = 0 WHERE next_retry = ?1", [DEFERRED])?;
    // Reading's queue waits for the same news: a mapping that arrived.
    c.execute("UPDATE read_outbox SET next_retry = 0 WHERE next_retry = ?1", [DEFERRED])?;
    Ok(woken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_core::traits::WatchStatus;

    const HOSHIKUZU: AnilistId = AnilistId::new(154_587);

    fn progress(episode: u32) -> TrackOp {
        TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode }
    }

    fn progress_of(id: u32) -> TrackOp {
        TrackOp::SetProgress { anilist_id: AnilistId::new(id), episode: 1 }
    }

    #[test]
    fn a_title_with_its_own_id_is_never_queued_for_a_tracker() {
        let store = Store::open_in_memory().unwrap();
        let own = TrackOp::SetProgress { anilist_id: AnilistId::own(3), episode: 2 };
        assert_eq!(store.enqueue("anilist", &own, 0).unwrap(), None);
        assert!(store.enqueue("anilist", &progress(1), 0).unwrap().is_some());
    }

    #[test]
    fn forgetting_a_title_drops_its_reading_ops_for_every_tracker_and_no_other() {
        let store = Store::open_in_memory().unwrap();
        let (gone, kept) = (AnilistId::new(900_401), AnilistId::new(900_402));
        let read =
            |id, chapter| TrackOp::SetReadProgress { anilist_id: id, chapter, volume: None };
        let status = TrackOp::SetReadStatus {
            anilist_id: gone,
            status: WatchStatus::Current,
            rereading: false,
            at: 1,
        };
        for tracker in ["anilist", "mangaupdates"] {
            store.enqueue(tracker, &read(gone, 4), 1).unwrap();
            store.enqueue(tracker, &status, 1).unwrap();
            store.enqueue(tracker, &read(kept, 2), 1).unwrap();
        }
        assert_eq!(store.forget_read_ops(gone).unwrap(), 4);
        assert_eq!(store.read_outbox("anilist").unwrap(), [read(kept, 2)]);
        assert_eq!(store.read_outbox_depth(None).unwrap(), 2);
    }

    #[test]
    fn queued_ops_survive_and_are_claimable() {
        let store = Store::open_in_memory().unwrap();
        store.enqueue("anilist", &progress(11), 1_000).unwrap().unwrap();

        let ready = store.claim_ready("anilist", 1_000, 10).unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].op, progress(11));
        assert_eq!(store.outbox_depth(Some("anilist")).unwrap(), 1);
    }

    #[test]
    fn nothing_about_the_demo_title_is_queued_for_a_tracker() {
        let store = Store::open_in_memory().unwrap();
        let op =
            TrackOp::SetProgress { anilist_id: kaeri_core::demo::DEMO_ANILIST_ID, episode: 1 };
        assert_eq!(store.enqueue("anilist", &op, 1_000).unwrap(), None);
        assert_eq!(store.outbox_depth(None).unwrap(), 0);
    }

    #[test]
    fn a_binge_offline_coalesces_into_one_pending_push() {
        // Twelve episodes watched with no network should not produce twelve queued ops;
        // progress is monotonic so only the furthest point matters.
        let store = Store::open_in_memory().unwrap();
        for ep in 1..=12 {
            store.enqueue("anilist", &progress(ep), 1_000 + i64::from(ep)).unwrap();
        }
        assert_eq!(store.outbox_depth(Some("anilist")).unwrap(), 1);
        let ready = store.claim_ready("anilist", 2_000, 10).unwrap();
        assert_eq!(ready[0].op, progress(12));
    }

    #[test]
    fn progress_never_walks_backwards() {
        // A stale lower episode arriving after a higher one must be dropped, not queued
        // behind it — otherwise the tracker would end up regressing.
        let store = Store::open_in_memory().unwrap();
        store.enqueue("anilist", &progress(12), 1_000).unwrap();
        let dropped = store.enqueue("anilist", &progress(7), 1_100).unwrap();
        assert_eq!(dropped, None, "lower episode should be discarded");
        assert_eq!(store.claim_ready("anilist", 2_000, 10).unwrap()[0].op, progress(12));
    }

    #[test]
    fn coalescing_keeps_the_backoff_the_row_has_earned() {
        // A binge against a failing tracker enqueues a progress op per episode, and each one
        // used to reset `attempts` and `next_retry` — so the backoff never applied to the
        // case it exists for, and the app hammered a tracker that was already refusing.
        let store = Store::open_in_memory().unwrap();
        store.enqueue("anilist", &progress(7), 1_000).unwrap();
        let id = store.claim_ready("anilist", 1_000, 10).unwrap()[0].id;
        store.fail(id, "503", 1_000).unwrap();
        assert!(store.claim_ready("anilist", 1_001, 10).unwrap().is_empty(), "backing off");

        // The next episode finishes a minute later and folds into the same row, while the
        // backoff from the failure still has time to run.
        store.enqueue("anilist", &progress(8), 1_010).unwrap();

        assert!(
            store.claim_ready("anilist", 1_010, 10).unwrap().is_empty(),
            "coalescing re-armed an immediate retry"
        );
        let entry = &store.claim_ready("anilist", 9_999, 10).unwrap()[0];
        assert_eq!(entry.op, progress(8), "and it still carries the newest episode");
        assert_eq!(entry.attempts, 1, "the row's history of failing survived");
    }

    #[test]
    fn an_operation_that_never_succeeds_is_eventually_abandoned() {
        // Nothing read `attempts` except the backoff, so an op that could never be delivered
        // retried hourly for the life of the installation and held the badge above zero.
        let store = Store::open_in_memory().unwrap();
        store.enqueue("anilist", &progress(7), 0).unwrap();
        let id = store.claim_ready("anilist", 0, 10).unwrap()[0].id;

        for attempt in 1..MAX_ATTEMPTS {
            assert!(!store.fail(id, "gone", i64::from(attempt) * 10_000).unwrap());
        }
        assert!(store.fail(id, "Not Found.", 1_000_000).unwrap(), "the last refusal gives up");

        assert_eq!(
            store.outbox_depth(Some("anilist")).unwrap(),
            0,
            "the queue should not carry an operation forever"
        );
        let kept = store.dropped_track_ops().unwrap();
        assert_eq!(
            kept,
            [DroppedOp {
                tracker_id: "anilist".into(),
                op: progress(7),
                error: "Not Found.".into(),
                at: 1_000_000,
            }],
            "the change given up is kept, with the tracker's last word on it"
        );
    }

    #[test]
    fn a_reading_op_given_up_is_kept_and_the_record_stays_bounded() {
        let store = Store::open_in_memory().unwrap();
        for n in 0..(DROPPED_KEPT as u32 + 5) {
            let op = TrackOp::SetReadProgress {
                anilist_id: AnilistId::new(990_000 + n),
                chapter: 1,
                volume: None,
            };
            let id = store.enqueue("mal", &op, 0).unwrap().unwrap();
            let given_up = (0..MAX_ATTEMPTS)
                .map(|attempt| store.fail_read(id, "refused", i64::from(attempt)).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(given_up.iter().filter(|g| **g).count(), 1);
            assert!(given_up[MAX_ATTEMPTS as usize - 1]);
        }
        assert_eq!(store.read_outbox_depth(Some("mal")).unwrap(), 0);
        let kept = store.dropped_track_ops().unwrap();
        assert_eq!(kept.len(), DROPPED_KEPT, "only the latest are kept");
        assert_eq!(kept[0].op.anilist_id(), AnilistId::new(990_005), "the oldest went first");
    }

    #[test]
    fn a_retry_set_while_the_clock_ran_ahead_is_due_once_it_is_put_right() {
        let store = Store::open_in_memory().unwrap();
        let ahead = 365 * 86_400;
        let id = store.enqueue("anilist", &progress(7), ahead).unwrap().unwrap();
        store.fail(id, "refused", ahead).unwrap();
        let newer =
            TrackOp::SetStatus { anilist_id: HOSHIKUZU, status: WatchStatus::Completed, at: 2 };
        store.enqueue("anilist", &newer, ahead + 1).unwrap();
        assert!(
            store.claim_ready("anilist", ahead + 1, 10).unwrap().is_empty(),
            "still waiting"
        );
        // The clock put right: both are due, in the order they were made.
        let due = store.claim_ready("anilist", 2_000, 10).unwrap();
        assert_eq!(due.iter().map(|e| e.op.clone()).collect::<Vec<_>>(), [progress(7), newer]);
        // A row held for a mapping stays held.
        let held = store.enqueue("anilist", &progress_of(990_002), 2_000).unwrap().unwrap();
        store.defer(held, "no id").unwrap();
        assert_eq!(store.claim_ready("anilist", 2_000, 10).unwrap().len(), 2);
    }

    #[test]
    fn a_newer_op_for_a_title_waits_behind_an_older_one_backing_off() {
        let store = Store::open_in_memory().unwrap();
        let status = |status, at| TrackOp::SetStatus { anilist_id: HOSHIKUZU, status, at };
        let older = store.enqueue("anilist", &status(WatchStatus::Current, 1), 0).unwrap();
        store.fail(older.unwrap(), "refused", 0).unwrap();
        store.enqueue("anilist", &status(WatchStatus::Completed, 2), 5).unwrap();
        let other = TrackOp::SetStatus {
            anilist_id: AnilistId::new(990_001),
            status: WatchStatus::Completed,
            at: 2,
        };
        store.enqueue("anilist", &other, 5).unwrap();

        let ready = store.claim_ready("anilist", 10, 10).unwrap();
        assert_eq!(
            ready.iter().map(|e| e.op.clone()).collect::<Vec<_>>(),
            [other],
            "the newer status waits; another title does not"
        );
        let later = store.claim_ready("anilist", backoff_secs(1) + 1, 10).unwrap();
        assert_eq!(later.len(), 3, "once the older one may go, both go, in order");
        assert_eq!(later[0].op, status(WatchStatus::Current, 1));
        assert_eq!(later[1].op, status(WatchStatus::Completed, 2));
    }

    #[test]
    fn a_deferred_op_waits_out_of_the_way_until_woken() {
        // A title with no mapping on this tracker cannot be pushed yet, and must neither block
        // the ops behind it nor be lost.
        let store = Store::open_in_memory().unwrap();
        let held = store.enqueue("mal", &progress(4), 1_000).unwrap().unwrap();
        let other = TrackOp::SetProgress { anilist_id: AnilistId::new(1), episode: 2 };
        store.enqueue("mal", &other, 1_001).unwrap();

        store.defer(held, "no mal id mapped").unwrap();
        let ready = store.claim_ready("mal", i64::MAX - 1, 10).unwrap();
        assert_eq!(ready.len(), 1, "the deferred op stays out of every batch");
        assert_eq!(ready[0].op, other);
        assert_eq!(store.outbox_depth(Some("mal")).unwrap(), 2, "and it is still queued");
        assert_eq!(store.outbox_deferred("mal").unwrap(), 1);
        assert_eq!(store.outbox_waiting("mal").unwrap(), 1, "the held op is not on its way");

        // Watching on coalesces into the held row rather than queueing a second one.
        store.enqueue("mal", &progress(5), 2_000).unwrap();
        assert_eq!(store.outbox_deferred("mal").unwrap(), 1);

        assert_eq!(store.wake_deferred().unwrap(), 1);
        let ready = store.claim_ready("mal", 2_000, 10).unwrap();
        assert!(ready.iter().any(|e| e.op == progress(5) && e.attempts == 0));
        assert_eq!(store.outbox_deferred("mal").unwrap(), 0);
        assert_eq!(store.outbox_waiting("mal").unwrap(), 2, "woken, it is on its way again");
    }

    #[test]
    fn a_mapping_refresh_wakes_what_was_deferred() {
        // Whichever path refreshes the datasets — startup, a later retry, `--refresh-data` —
        // the held ops get another look without that path having to know about them.
        let store = Store::open_in_memory().unwrap();
        let held = store.enqueue("simkl", &progress(4), 1_000).unwrap().unwrap();
        store.defer(held, "no mal id mapped").unwrap();

        store.materialise_mapping("test", 0, &[], None, 2_000).unwrap();
        assert_eq!(store.claim_ready("simkl", 2_000, 10).unwrap().len(), 1);
    }

    #[test]
    fn status_ops_are_not_coalesced_with_progress() {
        // Status is not monotonic and carries its own timestamp, so it must queue
        // separately rather than being folded into a progress update.
        let store = Store::open_in_memory().unwrap();
        store.enqueue("anilist", &progress(12), 1_000).unwrap();
        store
            .enqueue(
                "anilist",
                &TrackOp::SetStatus {
                    anilist_id: HOSHIKUZU,
                    status: WatchStatus::Completed,
                    at: 1_100,
                },
                1_100,
            )
            .unwrap();
        assert_eq!(store.outbox_depth(Some("anilist")).unwrap(), 2);
    }

    #[test]
    fn trackers_drain_independently() {
        // A broken MAL push must not stall AniList.
        let store = Store::open_in_memory().unwrap();
        store.enqueue("anilist", &progress(11), 1_000).unwrap();
        store.enqueue("mal", &progress(11), 1_000).unwrap();

        let mal = store.claim_ready("mal", 1_000, 10).unwrap();
        store.fail(mal[0].id, "503 from MAL", 1_000).unwrap();

        // MAL is now backed off, AniList is untouched and still ready.
        assert!(store.claim_ready("mal", 1_000, 10).unwrap().is_empty());
        assert_eq!(store.claim_ready("anilist", 1_000, 10).unwrap().len(), 1);
    }

    #[test]
    fn failures_back_off_exponentially_then_become_ready_again() {
        let store = Store::open_in_memory().unwrap();
        let id = store.enqueue("anilist", &progress(11), 0).unwrap().unwrap();

        store.fail(id, "network down", 0).unwrap();
        assert!(
            store.claim_ready("anilist", 10, 10).unwrap().is_empty(),
            "should be waiting out the backoff"
        );

        let ready = store.claim_ready("anilist", backoff_secs(1) + 1, 10).unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].attempts, 1);
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        assert_eq!(backoff_secs(0), 30);
        assert_eq!(backoff_secs(1), 60);
        assert_eq!(backoff_secs(2), 120);
        assert_eq!(backoff_secs(30), BACKOFF_MAX_SECS, "must not overflow or run away");
        // Monotonic up to the cap.
        for a in 0..8 {
            assert!(backoff_secs(a) <= backoff_secs(a + 1));
        }
    }

    #[test]
    fn completing_removes_the_op() {
        let store = Store::open_in_memory().unwrap();
        let id = store.enqueue("anilist", &progress(11), 0).unwrap().unwrap();
        store.complete(id).unwrap();
        assert_eq!(store.outbox_depth(None).unwrap(), 0);
    }

    #[test]
    fn pending_progress_reports_the_highest_queued_episode() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.pending_progress("anilist", HOSHIKUZU).unwrap(), None);
        store.enqueue("anilist", &progress(5), 0).unwrap();
        store.enqueue("anilist", &progress(9), 0).unwrap();
        assert_eq!(store.pending_progress("anilist", HOSHIKUZU).unwrap(), Some(9));
    }

    #[test]
    fn pending_statuses_are_the_last_set_per_title_until_sent() {
        let store = Store::open_in_memory().unwrap();
        let other = AnilistId::new(900_777);
        let status = |anilist_id, status| TrackOp::SetStatus { anilist_id, status, at: 1 };
        store.enqueue("anilist", &status(HOSHIKUZU, WatchStatus::Current), 1).unwrap();
        store.enqueue("anilist", &progress(4), 1).unwrap();
        store.enqueue("anilist", &status(HOSHIKUZU, WatchStatus::Planning), 2).unwrap();
        store.enqueue("anilist", &status(other, WatchStatus::Dropped), 2).unwrap();
        store.enqueue("kitsu", &status(other, WatchStatus::Completed), 2).unwrap();

        let pending = store.pending_statuses("anilist").unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[&HOSHIKUZU], WatchStatus::Planning, "the later one");
        assert_eq!(pending[&other], WatchStatus::Dropped, "this tracker's own");

        // Sent, it is the tracker's list that says it.
        for entry in store.claim_ready("anilist", 100, 10).unwrap() {
            store.complete(entry.id).unwrap();
        }
        assert!(store.pending_statuses("anilist").unwrap().is_empty());
    }

    #[test]
    fn ops_round_trip_through_json_unchanged() {
        // The outbox stores serialised ops, so a representation change would silently
        // orphan queued rows.
        let store = Store::open_in_memory().unwrap();
        let ops = [
            progress(3),
            TrackOp::SetStatus { anilist_id: HOSHIKUZU, status: WatchStatus::Paused, at: 7 },
            TrackOp::SetScore { anilist_id: HOSHIKUZU, score: 8.5, at: 9 },
        ];
        for (i, op) in ops.iter().enumerate() {
            store.enqueue("t", op, i as i64).unwrap();
        }
        let claimed = store.claim_ready("t", 100, 10).unwrap();
        assert_eq!(claimed.len(), 3);
        for (expected, got) in ops.iter().zip(claimed.iter()) {
            assert_eq!(&got.op, expected);
        }
    }

    #[test]
    fn reading_ops_wait_in_their_own_outbox_and_progress_only_rises() {
        let store = Store::open_in_memory().unwrap();
        let read = |chapter, volume| TrackOp::SetReadProgress {
            anilist_id: HOSHIKUZU,
            chapter,
            volume,
        };
        store.enqueue("mal", &read(12, Some(2)), 0).unwrap();
        assert_eq!(store.enqueue("mal", &read(10, Some(1)), 1).unwrap(), None);
        store.enqueue("mal", &read(14, None), 2).unwrap();
        store
            .enqueue(
                "mal",
                &TrackOp::SetReadStatus {
                    anilist_id: HOSHIKUZU,
                    status: WatchStatus::Current,
                    rereading: true,
                    at: 3,
                },
                3,
            )
            .unwrap();
        assert_eq!(store.outbox_depth(None).unwrap(), 0);
        assert!(store.claim_ready("mal", 100, 10).unwrap().is_empty());
        assert_eq!(store.read_outbox_depth(Some("mal")).unwrap(), 2);
        assert_eq!(store.read_outbox("mal").unwrap()[0], read(14, Some(2)));
    }
}
