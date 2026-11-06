//! Reading history: what was read, where each chapter was left, and the choices kept per series.
//!
//! The same shape as watching. `read_event` is an append-only log and the source of truth;
//! `read_chapter` and `read_work` are projections of it, rebuilt per work by [`refresh_work`]
//! after every change, local or received. Two more logs sit beside it: watermarks
//! (`read_adopt`: every chapter up to one counts as read) and resets (`read_reset`: Read again
//! sets older reads aside until it is undone).
//!
//! Works and chapters are the engine's `WorkId` and `ChapterId` strings. Only works AniList
//! lists (`a:<id>`) are written to the sync log, ops and registers alike; a local work stays on
//! the device.

use kaeri_core::books::ChapterId;
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{
    Result, Store,
    sync::{Op, OpBody, ReadNote, Stamp, kinds, tick},
};

/// The settings a series' own choice syncs for: the rest stay on the device.
pub const SYNCED_SERIES_SETTINGS: &[&str] = &["mode", "direction", "colour"];

/// A read event: a place in a chapter, or the chapter finished.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReadEvent {
    pub work: String,
    pub chapter: String,
    pub page: Option<u32>,
    pub of_pages: Option<u32>,
    pub strip_image: Option<u32>,
    pub strip_offset: Option<f64>,
    /// `spine/path/offset`, as the model's `NovelPos` encodes it.
    pub novel_pos: Option<String>,
    pub completed: bool,
    pub source_id: Option<String>,
    pub version_label: Option<String>,
    /// Unix seconds.
    pub at: i64,
}

impl ReadEvent {
    pub fn new(work: impl Into<String>, chapter: impl Into<String>, at: i64) -> Self {
        Self { work: work.into(), chapter: chapter.into(), at, ..Self::default() }
    }
}

/// One chapter's facts, from the events since the newest reset.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadChapter {
    pub chapter: String,
    pub read: bool,
    pub read_at: Option<i64>,
    pub last_page: Option<u32>,
    pub of_pages: Option<u32>,
    pub strip_image: Option<u32>,
    pub strip_offset: Option<f64>,
    pub novel_pos: Option<String>,
    pub stamp: Stamp,
    /// The device that wrote the newest place, when it came through sync.
    pub device: Option<String>,
}

/// A work's projection, for Continue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadWork {
    pub work: String,
    pub last_chapter: Option<String>,
    pub stamp: Stamp,
    pub reset_stamp: Stamp,
    pub adopt_milli: Option<i64>,
    pub adopt_volume: Option<String>,
    pub hidden: bool,
}

impl ReadWork {
    /// Whether the highest watermark covers `chapter`, unreads aside: [`ReadMarks`] has the answer
    /// that counts them.
    pub fn adopted(&self, chapter: &str) -> bool {
        self.adopt_milli
            .is_some_and(|milli| covers(milli, self.adopt_volume.as_deref(), chapter))
    }
}

/// What counts as read in one work, loaded once: the chapters read since the newest reset, and
/// the watermarks with the unreads that came after them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReadMarks {
    read: std::collections::BTreeSet<String>,
    /// `(upto_milli, upto_volume, stamp)` of each watermark since the newest reset.
    adopts: Vec<(i64, Option<String>, Stamp)>,
    /// The newest unread of each chapter.
    unreads: std::collections::BTreeMap<String, Stamp>,
}

impl ReadMarks {
    /// Whether `chapter` counts as read: read since the newest reset, or under a watermark set
    /// after the chapter was last marked unread.
    pub fn is_read(&self, chapter: &str) -> bool {
        if self.read.contains(chapter) {
            return true;
        }
        let unread = self.unreads.get(chapter).copied().unwrap_or(0);
        self.adopts.iter().any(|(milli, volume, stamp)| {
            *stamp > unread && covers(*milli, volume.as_deref(), chapter)
        })
    }
}

/// A series' own reader choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesSetting {
    pub setting: String,
    pub value: String,
    /// `series` (follows the person), `here` (this device only) or `auto` (the reader's guess,
    /// this device only).
    pub scope: String,
}

/// Which entry a work is on a tracker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMap {
    pub tracker: String,
    /// `None`: not on this tracker.
    pub tracker_id: Option<String>,
    pub origin: String,
    pub checked_at: i64,
}

/// A send to an e-reader not yet answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadSent {
    pub id: i64,
    pub work: String,
    pub chapters: Vec<String>,
    pub target: String,
    pub at: i64,
}

/// The source a work is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditionPref {
    pub source_id: String,
    pub version_label: Option<String>,
    pub language: Option<String>,
}

/// Whether a work's changes go to the sync log: only works AniList lists.
pub fn syncs(work: &str) -> bool {
    work.starts_with("a:")
}

/// A chapter's number in thousandths and its volume, when it has a number.
fn number_of(chapter: &str) -> Option<(i64, Option<String>)> {
    let key = ChapterId::new(chapter).key()?;
    let range = key.range()?;
    Some((range.last().milli(), key.volume().map(str::to_owned)))
}

fn volume_number(volume: &str) -> Option<f64> {
    volume.trim().parse().ok()
}

/// Whether a watermark at `milli` (in `volume`) counts `chapter` as read. A chapter with no number
/// is never covered, and numbers that start again each volume compare only within a volume.
pub fn covers(milli: i64, volume: Option<&str>, chapter: &str) -> bool {
    let Some((number, chapter_volume)) = number_of(chapter) else { return false };
    match (volume, chapter_volume.as_deref()) {
        (None, None) | (Some(_), None) => number <= milli,
        (None, Some(_)) => false,
        (Some(upto), Some(this)) => match (volume_number(upto), volume_number(this)) {
            (Some(upto), Some(this)) if this < upto => true,
            (Some(upto), Some(this)) if this > upto => false,
            _ => upto == this && number <= milli,
        },
    }
}

const EFFECTIVE: &str = "COALESCE(stamp, at * 65536000)";

/// The newest reset of a work not undone, 0 for none.
fn reset_stamp(tx: &Transaction<'_>, work: &str) -> rusqlite::Result<Stamp> {
    let stamp: Option<i64> = tx.query_row(
        "SELECT MAX(stamp) FROM read_reset WHERE work = ?1 AND undone IS NULL",
        [work],
        |r| r.get(0),
    )?;
    Ok(stamp.unwrap_or(0) as u64)
}

/// Rebuild a work's projections from its logs.
pub(crate) fn refresh_work(tx: &Transaction<'_>, work: &str) -> rusqlite::Result<()> {
    let reset = reset_stamp(tx, work)?;
    tx.execute("DELETE FROM read_chapter WHERE work = ?1", [work])?;
    tx.execute(
        &format!(
            "WITH e AS (
                 SELECT *, {EFFECTIVE} AS eff FROM read_event
                  WHERE work = ?1 AND {EFFECTIVE} > ?2
             ), ranked AS (
                 SELECT *,
                        ROW_NUMBER() OVER (PARTITION BY chapter ORDER BY eff DESC, id DESC) AS rn,
                        MAX(completed) OVER (PARTITION BY chapter) AS any_read,
                        MAX(CASE WHEN completed = 1 THEN at END)
                            OVER (PARTITION BY chapter) AS read_at
                   FROM e
             )
             INSERT INTO read_chapter
                 (work, chapter, read, read_at, last_page, of_pages, strip_image, strip_offset,
                  novel_pos, stamp, device)
             SELECT work, chapter, any_read, read_at, page, of_pages, strip_image, strip_offset,
                    novel_pos, eff,
                    CASE WHEN uid IS NULL OR instr(uid, ':') = 0 THEN NULL
                         ELSE substr(uid, 1, instr(uid, ':') - 1) END
               FROM ranked WHERE rn = 1"
        ),
        params![work, reset as i64],
    )?;

    let newest: Option<(String, i64, i64)> = tx
        .query_row(
            &format!(
                "SELECT chapter, {EFFECTIVE}, at FROM read_event
                  WHERE work = ?1 AND {EFFECTIVE} > ?2
                  ORDER BY {EFFECTIVE} DESC, id DESC LIMIT 1"
            ),
            params![work, reset as i64],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let adopt: Option<(i64, Option<String>, i64)> = tx
        .query_row(
            "SELECT upto_milli, upto_volume, stamp FROM read_adopt
              WHERE work = ?1 AND stamp > ?2
              ORDER BY upto_volume IS NOT NULL DESC, CAST(upto_volume AS REAL) DESC,
                       upto_milli DESC
              LIMIT 1",
            params![work, reset as i64],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let held_hidden: bool = tx
        .query_row("SELECT hidden FROM read_work WHERE work = ?1", [work], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?
        .is_some_and(|h| h != 0);
    let newest_stamp = newest.as_ref().map_or(0, |(_, s, _)| *s);
    // Hidden from Continue when the register says so after the newest read; with no register
    // (sync off, or a local work) as the device left it.
    let reg: Option<(i64, Option<String>)> = tx
        .query_row(
            "SELECT stamp, value FROM sync_reg WHERE kind = ?1 AND key = ?2",
            params![kinds::READ_HIDE, work],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let hidden = match reg {
        Some((stamp, value)) => value.is_some() && stamp > newest_stamp,
        None => held_hidden,
    };
    let anything = newest.is_some() || adopt.is_some() || reset > 0 || hidden;
    if !anything {
        tx.execute("DELETE FROM read_work WHERE work = ?1", [work])?;
        return Ok(());
    }
    let adopt_stamp = adopt.as_ref().map_or(0, |(_, _, s)| *s);
    let stamp = (newest_stamp.max(adopt_stamp)) as u64;
    let updated = newest.as_ref().map_or_else(|| crate::sync::secs_of_stamp(stamp), |n| n.2);
    tx.execute(
        "INSERT INTO read_work
             (work, last_chapter, stamp, reset_stamp, adopt_milli, adopt_volume, hidden, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(work) DO UPDATE SET
            last_chapter = excluded.last_chapter, stamp = excluded.stamp,
            reset_stamp = excluded.reset_stamp, adopt_milli = excluded.adopt_milli,
            adopt_volume = excluded.adopt_volume, hidden = excluded.hidden,
            updated_at = excluded.updated_at",
        params![
            work,
            newest.as_ref().map(|n| n.0.clone()),
            stamp as i64,
            reset as i64,
            adopt.as_ref().map(|a| a.0),
            adopt.and_then(|a| a.1),
            hidden as i32,
            updated
        ],
    )?;
    Ok(())
}

/// Apply a received reading op, noting reads kept against an unread they had not seen. Returns
/// the work to refresh.
pub(crate) fn apply_read_op(
    tx: &Transaction<'_>,
    op: &Op,
    now_secs: i64,
    notes: &mut Vec<ReadNote>,
) -> Result<Option<String>> {
    let uid = format!("{}:{}", op.device, op.seq);
    Ok(match &op.body {
        OpBody::Read {
            work,
            chapter,
            page,
            of,
            strip_image,
            strip_offset,
            novel_pos,
            completed,
            source_id,
            at,
            version_label,
            seen_unread,
            ..
        } => {
            let inserted = tx.execute(
                "INSERT OR IGNORE INTO read_event
                   (uid, stamp, work, chapter, page, of_pages, strip_image, strip_offset,
                    novel_pos, completed, source_id, version_label, at)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13
                  WHERE NOT EXISTS (SELECT 1 FROM sync_tomb WHERE uid = ?1)",
                params![
                    uid,
                    op.stamp as i64,
                    work,
                    chapter,
                    page,
                    of,
                    strip_image,
                    strip_offset,
                    novel_pos,
                    *completed as i32,
                    source_id,
                    version_label,
                    at
                ],
            )?;
            if inserted > 0 && *completed {
                // An unread another device wrote, later than this read, that did not name it.
                let unread: Option<(i64, String)> = tx
                    .query_row(
                        "SELECT stamp, device FROM read_unread WHERE work = ?1 AND chapter = ?2",
                        params![work, chapter],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                if let Some((stamp, device)) = unread
                    && stamp as u64 > *seen_unread
                    && device != op.device
                {
                    read_note(tx, notes, work, chapter, &device, &op.device, now_secs)?;
                }
            }
            Some(work.clone())
        }
        OpBody::Unread { work, chapter, removes } => {
            for removed in removes {
                tx.execute(
                    "INSERT OR IGNORE INTO sync_tomb (uid, stamp) VALUES (?1, ?2)",
                    params![removed, op.stamp as i64],
                )?;
                tx.execute("DELETE FROM read_event WHERE uid = ?1", [removed])?;
            }
            note_unread(tx, work, chapter, op.stamp, &op.device)?;
            // Finished reads by other devices that this unread never named.
            let kept: Vec<String> = {
                let mut stmt = tx.prepare(
                    "SELECT uid FROM read_event
                      WHERE work = ?1 AND chapter = ?2 AND completed = 1 AND uid IS NOT NULL",
                )?;
                stmt.query_map(params![work, chapter], |r| r.get(0))?
                    .collect::<rusqlite::Result<_>>()?
            };
            let mut readers = std::collections::BTreeSet::new();
            for uid in kept {
                let reader = uid.split_once(':').map_or("", |(d, _)| d).to_owned();
                if reader != op.device && read_seen_unread(tx, &uid)? < op.stamp {
                    readers.insert(reader);
                }
            }
            for reader in readers {
                read_note(tx, notes, work, chapter, &op.device, &reader, now_secs)?;
            }
            Some(work.clone())
        }
        OpBody::ReadAdopt { work, upto_chapter, upto_volume, at } => {
            insert_adopt(
                tx,
                &format!("a:{uid}"),
                work,
                upto_chapter,
                upto_volume,
                op.stamp,
                *at,
            )?;
            Some(work.clone())
        }
        OpBody::ReadReset { work, at, undoes } => {
            match undoes {
                Some(reset) => undo_reset_row(tx, reset, work, op.stamp)?,
                None => insert_reset(tx, &uid, work, op.stamp, *at)?,
            }
            Some(work.clone())
        }
        OpBody::ReadMark { tracker, anilist_id, chapter, volume } => {
            tx.execute(
                "INSERT INTO read_mark (tracker, anilist_id, chapter, volume) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(tracker, anilist_id) DO UPDATE SET
                    chapter = MAX(chapter, ?3),
                    volume = CASE WHEN ?4 IS NULL THEN volume
                                  ELSE MAX(COALESCE(volume, 0), ?4) END",
                params![tracker, anilist_id, chapter, volume],
            )?;
            None
        }
        _ => None,
    })
}

/// Keep the newest unread of a chapter, so a watermark set before it no longer counts it.
fn note_unread(
    tx: &Transaction<'_>,
    work: &str,
    chapter: &str,
    stamp: Stamp,
    device: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO read_unread (work, chapter, stamp, device) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(work, chapter) DO UPDATE SET stamp = excluded.stamp, device = excluded.device
          WHERE excluded.stamp > read_unread.stamp",
        params![work, chapter, stamp as i64, device],
    )?;
    Ok(())
}

/// The unread a read's device had seen, from the op that made the event.
fn read_seen_unread(tx: &Transaction<'_>, uid: &str) -> rusqlite::Result<Stamp> {
    let Some((device, seq)) = uid.split_once(':') else { return Ok(0) };
    let body: Option<String> = tx
        .query_row(
            "SELECT body FROM sync_op WHERE device = ?1 AND seq = ?2",
            params![device, seq.parse::<i64>().unwrap_or(0)],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match body.and_then(|b| serde_json::from_str::<OpBody>(&b).ok()) {
        Some(OpBody::Read { seen_unread, .. }) => seen_unread,
        _ => 0,
    })
}

fn read_note(
    tx: &Transaction<'_>,
    notes: &mut Vec<ReadNote>,
    work: &str,
    chapter: &str,
    unread_device: &str,
    read_device: &str,
    now: i64,
) -> rusqlite::Result<()> {
    let added = tx.execute(
        "INSERT OR IGNORE INTO read_note (work, chapter, unread_device, read_device, at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![work, chapter, unread_device, read_device, now],
    )?;
    if added > 0 {
        notes.push(ReadNote {
            id: tx.last_insert_rowid(),
            work: work.to_owned(),
            chapter: chapter.to_owned(),
            unread_device: unread_device.to_owned(),
            read_device: read_device.to_owned(),
            at: now,
        });
    }
    Ok(())
}

fn insert_adopt(
    tx: &Transaction<'_>,
    uid: &str,
    work: &str,
    upto_chapter: &str,
    upto_volume: &Option<String>,
    stamp: Stamp,
    at: i64,
) -> rusqlite::Result<()> {
    // A chapter with no number marks nothing: there is nothing to count up to.
    let Some((milli, volume)) = number_of(upto_chapter) else { return Ok(()) };
    tx.execute(
        "INSERT OR IGNORE INTO read_adopt
             (uid, work, upto_chapter, upto_milli, upto_volume, stamp, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            uid,
            work,
            upto_chapter,
            milli,
            upto_volume.clone().or(volume),
            stamp as i64,
            at
        ],
    )?;
    Ok(())
}

fn insert_reset(
    tx: &Transaction<'_>,
    uid: &str,
    work: &str,
    stamp: Stamp,
    at: i64,
) -> rusqlite::Result<()> {
    // An undo can arrive before the reset it names, from another device: the row it left keeps
    // its `undone`, and the reset's own facts fill the rest.
    tx.execute(
        "INSERT INTO read_reset (uid, work, stamp, at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(uid) DO UPDATE SET stamp = excluded.stamp, at = excluded.at",
        params![uid, work, stamp as i64, at],
    )?;
    Ok(())
}

fn undo_reset_row(
    tx: &Transaction<'_>,
    reset: &str,
    work: &str,
    stamp: Stamp,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO read_reset (uid, work, stamp, at, undone) VALUES (?1, ?2, 0, 0, ?3)
         ON CONFLICT(uid) DO UPDATE SET undone = excluded.undone",
        params![reset, work, stamp as i64],
    )?;
    Ok(())
}

/// Apply a received reading register. Returns the work to refresh.
pub(crate) fn apply_read_reg(
    tx: &Transaction<'_>,
    kind: &str,
    key: &str,
    value: Option<&str>,
    at: i64,
) -> Result<Option<String>> {
    Ok(match kind {
        kinds::READ_SERIES => {
            let Some((work, setting)) = key.split_once('/') else { return Ok(None) };
            // A setting this version does not sync is passed over.
            if !SYNCED_SERIES_SETTINGS.contains(&setting) {
                return Ok(None);
            }
            write_series_setting(tx, work, setting, value, "series", at)?;
            Some(work.to_owned())
        }
        kinds::READ_STATUS => {
            write_status(tx, key, Some(value), None, at)?;
            Some(key.to_owned())
        }
        kinds::READ_SCORE => {
            let score = value.and_then(|v| v.parse::<f64>().ok());
            write_status(tx, key, None, Some(score), at)?;
            Some(key.to_owned())
        }
        kinds::READ_HIDE => Some(key.to_owned()),
        kinds::READ_BOOKMARK => {
            let Some((work, chapter)) = key.split_once('/') else { return Ok(None) };
            write_bookmark(tx, work, chapter, value.is_some(), at)?;
            Some(work.to_owned())
        }
        kinds::READ_TRACK_MAP => {
            let Some((tracker, work)) = key.split_once('/') else { return Ok(None) };
            match value {
                Some(id) => {
                    let id = (!id.is_empty()).then_some(id);
                    tx.execute(
                        "INSERT INTO read_track_map (work, tracker, tracker_id, origin, checked_at)
                         VALUES (?1, ?2, ?3, 'hand', ?4)
                         ON CONFLICT(work, tracker) DO UPDATE SET
                            tracker_id = excluded.tracker_id, origin = 'hand',
                            checked_at = excluded.checked_at",
                        params![work, tracker, id, at],
                    )?;
                }
                None => {
                    tx.execute(
                        "DELETE FROM read_track_map WHERE work = ?1 AND tracker = ?2
                            AND origin = 'hand'",
                        params![work, tracker],
                    )?;
                }
            }
            Some(work.to_owned())
        }
        kinds::BOOK_MATCH => {
            let Some((source, series)) = key.split_once('/') else { return Ok(None) };
            match value {
                Some(work) => {
                    tx.execute(
                        "INSERT INTO book_source_match
                             (source_id, series_ref, work, confidence, confirmed, updated_at)
                         VALUES (?1, ?2, ?3, 100, 1, ?4)
                         ON CONFLICT(source_id, series_ref) DO UPDATE SET
                            work = excluded.work, confirmed = 1,
                            updated_at = excluded.updated_at",
                        params![source, series, work, at],
                    )?;
                    Some(work.to_owned())
                }
                None => {
                    tx.execute(
                        "DELETE FROM book_source_match WHERE source_id = ?1 AND series_ref = ?2",
                        params![source, series],
                    )?;
                    None
                }
            }
        }
        _ => None,
    })
}

fn write_series_setting(
    tx: &Transaction<'_>,
    work: &str,
    setting: &str,
    value: Option<&str>,
    scope: &str,
    at: i64,
) -> rusqlite::Result<()> {
    match value {
        Some(value) => tx.execute(
            "INSERT INTO read_series_setting (work, setting, value, scope, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(work, setting) DO UPDATE SET
                value = excluded.value, scope = excluded.scope, updated_at = excluded.updated_at",
            params![work, setting, value, scope, at],
        )?,
        None => tx.execute(
            "DELETE FROM read_series_setting WHERE work = ?1 AND setting = ?2",
            params![work, setting],
        )?,
    };
    Ok(())
}

/// Write a status (outer `Some`) or a score (outer `Some`), leaving the other as it is.
fn write_status(
    tx: &Transaction<'_>,
    work: &str,
    status: Option<Option<&str>>,
    score: Option<Option<f64>>,
    at: i64,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO read_status (work, status, score, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(work) DO UPDATE SET
            status = CASE WHEN ?5 THEN excluded.status ELSE status END,
            score = CASE WHEN ?6 THEN excluded.score ELSE score END,
            updated_at = excluded.updated_at",
        params![work, status.flatten(), score.flatten(), at, status.is_some(), score.is_some()],
    )?;
    tx.execute(
        "DELETE FROM read_status WHERE work = ?1 AND status IS NULL AND score IS NULL",
        [work],
    )?;
    Ok(())
}

fn write_bookmark(
    tx: &Transaction<'_>,
    work: &str,
    chapter: &str,
    on: bool,
    at: i64,
) -> rusqlite::Result<()> {
    if on {
        tx.execute(
            "INSERT OR REPLACE INTO read_bookmark (work, chapter, at) VALUES (?1, ?2, ?3)",
            params![work, chapter, at],
        )?;
    } else {
        tx.execute(
            "DELETE FROM read_bookmark WHERE work = ?1 AND chapter = ?2",
            params![work, chapter],
        )?;
    }
    Ok(())
}

/// The next stamp for a reading row written here with sync off: never behind one already held.
fn local_stamp(tx: &Transaction<'_>, wall_ms: i64) -> rusqlite::Result<Stamp> {
    let last: Option<i64> = tx.query_row(
        "SELECT MAX(m) FROM (
             SELECT MAX(stamp) AS m FROM read_event
             UNION ALL SELECT MAX(stamp) FROM read_adopt
             UNION ALL SELECT MAX(stamp) FROM read_reset
             UNION ALL SELECT MAX(stamp) FROM read_unread
         )",
        [],
        |r| r.get(0),
    )?;
    Ok(tick(last.unwrap_or(0) as u64, wall_ms))
}

fn row_to_chapter(r: &rusqlite::Row<'_>) -> rusqlite::Result<ReadChapter> {
    Ok(ReadChapter {
        chapter: r.get(0)?,
        read: r.get::<_, i64>(1)? != 0,
        read_at: r.get(2)?,
        last_page: r.get(3)?,
        of_pages: r.get(4)?,
        strip_image: r.get(5)?,
        strip_offset: r.get(6)?,
        novel_pos: r.get(7)?,
        stamp: r.get::<_, i64>(8)? as u64,
        device: r.get(9)?,
    })
}

fn row_to_work(r: &rusqlite::Row<'_>) -> rusqlite::Result<ReadWork> {
    Ok(ReadWork {
        work: r.get(0)?,
        last_chapter: r.get(1)?,
        stamp: r.get::<_, i64>(2)? as u64,
        reset_stamp: r.get::<_, i64>(3)? as u64,
        adopt_milli: r.get(4)?,
        adopt_volume: r.get(5)?,
        hidden: r.get::<_, i64>(6)? != 0,
    })
}

const WORK_COLUMNS: &str =
    "work, last_chapter, stamp, reset_stamp, adopt_milli, adopt_volume, hidden";

impl Store {
    /// Record a read event made here, with the trackers it was queued for, and its op when the
    /// work syncs.
    pub fn record_read(&self, event: &ReadEvent, queued_for: Vec<String>) -> Result<()> {
        let wall = self.sync.wall_ms();
        self.with_personal_tx(|tx| {
            let op = if syncs(&event.work) {
                let seen_unread = if event.completed {
                    tx.query_row(
                        "SELECT stamp FROM read_unread WHERE work = ?1 AND chapter = ?2",
                        params![event.work, event.chapter],
                        |r| r.get::<_, i64>(0),
                    )
                    .optional()?
                    .unwrap_or(0) as u64
                } else {
                    0
                };
                self.emit(
                    tx,
                    OpBody::Read {
                        work: event.work.clone(),
                        chapter: event.chapter.clone(),
                        page: event.page,
                        of: event.of_pages,
                        strip_image: event.strip_image,
                        strip_offset: event.strip_offset,
                        novel_pos: event.novel_pos.clone(),
                        completed: event.completed,
                        source_id: event.source_id.clone(),
                        at: event.at,
                        version_label: event.version_label.clone(),
                        queued_for,
                        seen_unread,
                    },
                )?
            } else {
                None
            };
            let (uid, stamp) = match &op {
                Some(op) => (Some(format!("{}:{}", op.device, op.seq)), op.stamp),
                None => (None, local_stamp(tx, wall)?),
            };
            tx.execute(
                "INSERT INTO read_event
                   (uid, stamp, work, chapter, page, of_pages, strip_image, strip_offset,
                    novel_pos, completed, source_id, version_label, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    uid,
                    stamp as i64,
                    event.work,
                    event.chapter,
                    event.page,
                    event.of_pages,
                    event.strip_image,
                    event.strip_offset,
                    event.novel_pos,
                    event.completed as i32,
                    event.source_id,
                    event.version_label,
                    event.at
                ],
            )?;
            // A read brings a work hidden from Continue back.
            tx.execute("UPDATE read_work SET hidden = 0 WHERE work = ?1", [&event.work])?;
            refresh_work(tx, &event.work)?;
            Ok(())
        })
    }

    /// Take chapters back: their events go, named in an unread op when the work syncs. A
    /// watermark set before the unread no longer counts the chapter; one set after it does.
    pub fn unread(&self, work: &str, chapters: &[String]) -> Result<()> {
        let wall = self.sync.wall_ms();
        self.with_personal_tx(|tx| {
            for chapter in chapters {
                let removes: Vec<String> = {
                    let mut stmt = tx.prepare(
                        "SELECT uid FROM read_event
                          WHERE work = ?1 AND chapter = ?2 AND uid IS NOT NULL",
                    )?;
                    stmt.query_map(params![work, chapter], |r| r.get(0))?
                        .collect::<rusqlite::Result<_>>()?
                };
                tx.execute(
                    "DELETE FROM read_event WHERE work = ?1 AND chapter = ?2",
                    params![work, chapter],
                )?;
                let mut kept = None;
                if syncs(work) {
                    let body = OpBody::Unread {
                        work: work.to_owned(),
                        chapter: chapter.clone(),
                        removes: removes.clone(),
                    };
                    if let Some(op) = self.emit(tx, body)? {
                        for uid in &removes {
                            tx.execute(
                                "INSERT OR IGNORE INTO sync_tomb (uid, stamp) VALUES (?1, ?2)",
                                params![uid, op.stamp as i64],
                            )?;
                        }
                        kept = Some((op.stamp, op.device));
                    }
                }
                let (stamp, device) = match kept {
                    Some(kept) => kept,
                    None => (local_stamp(tx, wall)?, String::new()),
                };
                note_unread(tx, work, chapter, stamp, &device)?;
            }
            refresh_work(tx, work)?;
            Ok(())
        })
    }

    /// Every chapter up to `upto_chapter` counts as read: Mark as read up to here, or a
    /// tracker's count adopted. A chapter with no number marks nothing.
    pub fn adopt_read(
        &self,
        work: &str,
        upto_chapter: &str,
        upto_volume: Option<&str>,
        at: i64,
    ) -> Result<()> {
        let wall = self.sync.wall_ms();
        self.with_personal_tx(|tx| {
            let volume = upto_volume.map(str::to_owned);
            let body = OpBody::ReadAdopt {
                work: work.to_owned(),
                upto_chapter: upto_chapter.to_owned(),
                upto_volume: volume.clone(),
                at,
            };
            let op = if syncs(work) { self.emit(tx, body)? } else { None };
            let (uid, stamp) = match op {
                Some(op) => (format!("a:{}:{}", op.device, op.seq), op.stamp),
                None => {
                    let stamp = local_stamp(tx, wall)?;
                    (format!("here:{stamp}"), stamp)
                }
            };
            insert_adopt(tx, &uid, work, upto_chapter, &volume, stamp, at)?;
            refresh_work(tx, work)?;
            Ok(())
        })
    }

    /// Read again from the start: older reads and watermarks are set aside. Returns the reset's
    /// id, for [`Self::undo_read_reset`].
    pub fn reset_read(&self, work: &str, at: i64) -> Result<String> {
        let wall = self.sync.wall_ms();
        self.with_personal_tx(|tx| {
            let body = OpBody::ReadReset { work: work.to_owned(), at, undoes: None };
            let op = if syncs(work) { self.emit(tx, body)? } else { None };
            let (uid, stamp) = match op {
                Some(op) => (format!("{}:{}", op.device, op.seq), op.stamp),
                None => {
                    let stamp = local_stamp(tx, wall)?;
                    (format!("here:{stamp}"), stamp)
                }
            };
            insert_reset(tx, &uid, work, stamp, at)?;
            refresh_work(tx, work)?;
            Ok(uid)
        })
    }

    /// Undo a Read again: what it set aside comes back.
    pub fn undo_read_reset(&self, work: &str, reset: &str, at: i64) -> Result<()> {
        let wall = self.sync.wall_ms();
        self.with_personal_tx(|tx| {
            let body =
                OpBody::ReadReset { work: work.to_owned(), at, undoes: Some(reset.to_owned()) };
            let op = if syncs(work) { self.emit(tx, body)? } else { None };
            let stamp = match op {
                Some(op) => op.stamp,
                None => local_stamp(tx, wall)?,
            };
            undo_reset_row(tx, reset, work, stamp)?;
            refresh_work(tx, work)?;
            Ok(())
        })
    }

    /// The newest reset of a work still in force, for Undo.
    pub fn read_reset_in_force(&self, work: &str) -> Result<Option<String>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT uid FROM read_reset WHERE work = ?1 AND undone IS NULL
                  ORDER BY stamp DESC LIMIT 1",
                [work],
                |r| r.get(0),
            )
            .optional()?)
        })
    }

    /// A work's chapters with facts, since the newest reset.
    pub fn read_chapters(&self, work: &str) -> Result<Vec<ReadChapter>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT chapter, read, read_at, last_page, of_pages, strip_image, strip_offset,
                        novel_pos, stamp, device
                   FROM read_chapter WHERE work = ?1 ORDER BY chapter",
            )?;
            Ok(stmt.query_map([work], row_to_chapter)?.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn read_work(&self, work: &str) -> Result<Option<ReadWork>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                &format!("SELECT {WORK_COLUMNS} FROM read_work WHERE work = ?1"),
                [work],
                row_to_work,
            )
            .optional()?)
        })
    }

    /// Whether a chapter counts as read: read since the newest reset, or under a watermark set
    /// after its newest unread.
    pub fn chapter_is_read(&self, work: &str, chapter: &str) -> Result<bool> {
        Ok(self.read_marks(work)?.is_read(chapter))
    }

    /// What counts as read in a work, for asking about many chapters at once.
    pub fn read_marks(&self, work: &str) -> Result<ReadMarks> {
        self.with_personal_tx(|tx| {
            let reset = reset_stamp(tx, work)?;
            let read = {
                let mut stmt = tx
                    .prepare("SELECT chapter FROM read_chapter WHERE work = ?1 AND read = 1")?;
                stmt.query_map([work], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
            };
            let adopts = {
                let mut stmt = tx.prepare(
                    "SELECT upto_milli, upto_volume, stamp FROM read_adopt
                      WHERE work = ?1 AND stamp > ?2",
                )?;
                stmt.query_map(params![work, reset as i64], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u64))
                })?
                .collect::<rusqlite::Result<_>>()?
            };
            let unreads = {
                let mut stmt =
                    tx.prepare("SELECT chapter, stamp FROM read_unread WHERE work = ?1")?;
                stmt.query_map([work], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))?
                    .collect::<rusqlite::Result<_>>()?
            };
            Ok(ReadMarks { read, adopts, unreads })
        })
    }

    /// Finished chapters kept against an unread they had not seen, newer than `since` (unix
    /// seconds).
    pub fn read_notes(&self, since: i64) -> Result<Vec<ReadNote>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT id, work, chapter, unread_device, read_device, at
                   FROM read_note WHERE at >= ?1 ORDER BY id",
            )?;
            Ok(stmt
                .query_map([since], |r| {
                    Ok(ReadNote {
                        id: r.get(0)?,
                        work: r.get(1)?,
                        chapter: r.get(2)?,
                        unread_device: r.get(3)?,
                        read_device: r.get(4)?,
                        at: r.get(5)?,
                    })
                })?
                .collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn dismiss_read_note(&self, id: i64) -> Result<()> {
        self.with_personal(|c| {
            Ok(c.execute("DELETE FROM read_note WHERE id = ?1", [id]).map(|_| ())?)
        })
    }

    /// The works for Continue, newest first: read here or elsewhere, and not hidden.
    pub fn continue_reading(&self, limit: usize) -> Result<Vec<ReadWork>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {WORK_COLUMNS} FROM read_work
                  WHERE hidden = 0 AND last_chapter IS NOT NULL
                  ORDER BY stamp DESC LIMIT ?1"
            ))?;
            Ok(stmt.query_map([limit as i64], row_to_work)?.collect::<rusqlite::Result<_>>()?)
        })
    }

    /// Hide a work from Continue until it is read again, or bring it back.
    pub fn set_read_hidden(&self, work: &str, hidden: bool) -> Result<()> {
        self.with_personal_tx(|tx| {
            tx.execute(
                "INSERT INTO read_work (work, hidden, updated_at) VALUES (?1, ?2, 0)
                 ON CONFLICT(work) DO UPDATE SET hidden = excluded.hidden",
                params![work, hidden as i32],
            )?;
            if syncs(work) {
                self.emit_reg(tx, kinds::READ_HIDE, work, hidden.then_some("1"))?;
            }
            refresh_work(tx, work)?;
            Ok(())
        })
    }

    /// A series' own reader choice, or `None` to use the default again. A choice in
    /// [`SYNCED_SERIES_SETTINGS`] follows the person when `follow` is on (the "Series choices
    /// follow you" switch); the rest stay on this device.
    pub fn set_series_setting(
        &self,
        work: &str,
        setting: &str,
        value: Option<&str>,
        follow: bool,
        at: i64,
    ) -> Result<()> {
        let synced = follow && SYNCED_SERIES_SETTINGS.contains(&setting);
        self.with_personal_tx(|tx| {
            let scope = if synced { "series" } else { "here" };
            write_series_setting(tx, work, setting, value, scope, at)?;
            if synced && syncs(work) {
                self.emit_reg(tx, kinds::READ_SERIES, &format!("{work}/{setting}"), value)?;
            }
            Ok(())
        })
    }

    /// Keep the reader's guess for a series (scope `auto`), unless the series already has a
    /// value for the setting. A guess stays on the device. Returns whether it was kept, so the
    /// guess is shown once.
    pub fn set_series_guess(
        &self,
        work: &str,
        setting: &str,
        value: &str,
        at: i64,
    ) -> Result<bool> {
        self.with_personal(|c| {
            Ok(c.execute(
                "INSERT OR IGNORE INTO read_series_setting (work, setting, value, scope, updated_at)
                 VALUES (?1, ?2, ?3, 'auto', ?4)",
                params![work, setting, value, at],
            )? > 0)
        })
    }

    pub fn series_settings(&self, work: &str) -> Result<Vec<SeriesSetting>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT setting, value, scope FROM read_series_setting WHERE work = ?1
                  ORDER BY setting",
            )?;
            Ok(stmt
                .query_map([work], |r| {
                    Ok(SeriesSetting { setting: r.get(0)?, value: r.get(1)?, scope: r.get(2)? })
                })?
                .collect::<rusqlite::Result<_>>()?)
        })
    }

    /// A work's reading list, by the model's status names.
    pub fn set_read_status(&self, work: &str, status: Option<&str>, at: i64) -> Result<()> {
        self.with_personal_tx(|tx| {
            write_status(tx, work, Some(status), None, at)?;
            if syncs(work) {
                self.emit_reg(tx, kinds::READ_STATUS, work, status)?;
            }
            Ok(())
        })
    }

    pub fn set_read_score(&self, work: &str, score: Option<f64>, at: i64) -> Result<()> {
        self.with_personal_tx(|tx| {
            write_status(tx, work, None, Some(score), at)?;
            if syncs(work) {
                let text = score.map(|s| s.to_string());
                self.emit_reg(tx, kinds::READ_SCORE, work, text.as_deref())?;
            }
            Ok(())
        })
    }

    /// A work's status and score.
    pub fn read_status(&self, work: &str) -> Result<(Option<String>, Option<f64>)> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT status, score FROM read_status WHERE work = ?1",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .unwrap_or_default())
        })
    }

    /// When a work's status or score was last set here or by a synced device.
    pub fn read_status_at(&self, work: &str) -> Result<Option<i64>> {
        self.with_personal(|c| {
            Ok(c.query_row("SELECT updated_at FROM read_status WHERE work = ?1", [work], |r| {
                r.get(0)
            })
            .optional()?)
        })
    }

    pub fn set_bookmark(&self, work: &str, chapter: &str, on: bool, at: i64) -> Result<()> {
        self.with_personal_tx(|tx| {
            write_bookmark(tx, work, chapter, on, at)?;
            if syncs(work) {
                let key = format!("{work}/{chapter}");
                self.emit_reg(tx, kinds::READ_BOOKMARK, &key, on.then_some("1"))?;
            }
            Ok(())
        })
    }

    pub fn bookmarks(&self, work: &str) -> Result<Vec<String>> {
        self.with_personal(|c| {
            let mut stmt =
                c.prepare("SELECT chapter FROM read_bookmark WHERE work = ?1 ORDER BY at")?;
            Ok(stmt.query_map([work], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
        })
    }

    /// Which entry a work is on a tracker. `tracker_id: None` is "not on this tracker". A
    /// choice made by hand (`origin` `hand`) is never replaced by a lookup, and is the only one
    /// that syncs.
    pub fn set_track_map(
        &self,
        work: &str,
        tracker: &str,
        tracker_id: Option<&str>,
        origin: &str,
        at: i64,
    ) -> Result<bool> {
        self.with_personal_tx(|tx| {
            let held: Option<String> = tx
                .query_row(
                    "SELECT origin FROM read_track_map WHERE work = ?1 AND tracker = ?2",
                    params![work, tracker],
                    |r| r.get(0),
                )
                .optional()?;
            if held.as_deref() == Some("hand") && origin != "hand" {
                return Ok(false);
            }
            tx.execute(
                "INSERT INTO read_track_map (work, tracker, tracker_id, origin, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(work, tracker) DO UPDATE SET
                    tracker_id = excluded.tracker_id, origin = excluded.origin,
                    checked_at = excluded.checked_at",
                params![work, tracker, tracker_id, origin, at],
            )?;
            if origin == "hand" && syncs(work) {
                let key = format!("{tracker}/{work}");
                self.emit_reg(tx, kinds::READ_TRACK_MAP, &key, Some(tracker_id.unwrap_or("")))?;
            }
            Ok(true)
        })
    }

    pub fn track_map(&self, work: &str) -> Result<Vec<TrackMap>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT tracker, tracker_id, origin, checked_at FROM read_track_map
                  WHERE work = ?1 ORDER BY tracker",
            )?;
            Ok(stmt
                .query_map([work], |r| {
                    Ok(TrackMap {
                        tracker: r.get(0)?,
                        tracker_id: r.get(1)?,
                        origin: r.get(2)?,
                        checked_at: r.get(3)?,
                    })
                })?
                .collect::<rusqlite::Result<_>>()?)
        })
    }

    /// The work a tracker's entry was mapped to: the other way round from [`Self::track_map`].
    pub fn track_map_work(&self, tracker: &str, tracker_id: &str) -> Result<Option<String>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT work FROM read_track_map WHERE tracker = ?1 AND tracker_id = ?2 LIMIT 1",
                params![tracker, tracker_id],
                |r| r.get(0),
            )
            .optional()?)
        })
    }

    /// The tracker's id for a work, when one is kept.
    pub fn track_map_id(&self, work: &str, tracker: &str) -> Result<Option<String>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT tracker_id FROM read_track_map WHERE work = ?1 AND tracker = ?2",
                params![work, tracker],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
        })
    }

    /// How far a work is read, as trackers count: the highest whole chapter read, a watermark
    /// counting, and the volume a watermark named. `(0, None)` for a work not read.
    pub fn read_progress(&self, work: &str) -> Result<(u32, Option<u32>)> {
        let mut milli = self
            .read_chapters(work)?
            .iter()
            .filter(|c| c.read)
            .filter_map(|c| number_of(&c.chapter))
            .map(|(milli, _)| milli)
            .max()
            .unwrap_or(0);
        let mut volume = None;
        if let Some(held) = self.read_work(work)? {
            if let Some(adopted) = held.adopt_milli {
                milli = milli.max(adopted);
            }
            volume = held.adopt_volume.as_deref().and_then(|v| v.trim().parse::<u32>().ok());
        }
        Ok((u32::try_from(milli / 1000).unwrap_or(u32::MAX), volume))
    }

    /// Which work a source's series is. `share` for a plugin's or a server's match, which
    /// follows the person when confirmed; a folder's stays on the device.
    #[allow(clippy::too_many_arguments)]
    pub fn set_book_match(
        &self,
        source_id: &str,
        series_ref: &str,
        work: &str,
        confidence: u8,
        confirmed: bool,
        share: bool,
        at: i64,
    ) -> Result<()> {
        self.with_personal_tx(|tx| {
            tx.execute(
                "INSERT INTO book_source_match
                     (source_id, series_ref, work, confidence, confirmed, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(source_id, series_ref) DO UPDATE SET
                    work = excluded.work, confidence = excluded.confidence,
                    confirmed = excluded.confirmed, updated_at = excluded.updated_at",
                params![source_id, series_ref, work, confidence, confirmed as i32, at],
            )?;
            if share && confirmed && syncs(work) {
                let key = format!("{source_id}/{series_ref}");
                self.emit_reg(tx, kinds::BOOK_MATCH, &key, Some(work))?;
            }
            Ok(())
        })
    }

    /// The work a source's series was matched to.
    pub fn book_match(&self, source_id: &str, series_ref: &str) -> Result<Option<String>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT work FROM book_source_match WHERE source_id = ?1 AND series_ref = ?2",
                params![source_id, series_ref],
                |r| r.get(0),
            )
            .optional()?)
        })
    }

    pub fn set_edition_pref(&self, work: &str, pref: Option<&EditionPref>) -> Result<()> {
        self.with_personal(|c| {
            match pref {
                Some(p) => c.execute(
                    "INSERT OR REPLACE INTO book_edition_pref
                         (work, source_id, version_label, language)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![work, p.source_id, p.version_label, p.language],
                )?,
                None => c.execute("DELETE FROM book_edition_pref WHERE work = ?1", [work])?,
            };
            Ok(())
        })
    }

    pub fn edition_pref(&self, work: &str) -> Result<Option<EditionPref>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT source_id, version_label, language FROM book_edition_pref
                  WHERE work = ?1",
                [work],
                |r| {
                    Ok(EditionPref {
                        source_id: r.get(0)?,
                        version_label: r.get(1)?,
                        language: r.get(2)?,
                    })
                },
            )
            .optional()?)
        })
    }

    /// Chapters sent to an e-reader, so the series page can ask where the person stopped.
    pub fn record_sent(
        &self,
        work: &str,
        chapters: &[String],
        target: &str,
        at: i64,
    ) -> Result<i64> {
        let json = serde_json::to_string(chapters)
            .map_err(|source| crate::StoreError::Encode { what: "sent chapters", source })?;
        self.with_personal(|c| {
            c.execute(
                "INSERT INTO read_sent (work, chapters, target, at) VALUES (?1, ?2, ?3, ?4)",
                params![work, json, target, at],
            )?;
            Ok(c.last_insert_rowid())
        })
    }

    /// The newest send of a work not yet answered.
    pub fn pending_sent(&self, work: &str) -> Result<Option<ReadSent>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT id, work, chapters, target, at FROM read_sent
                  WHERE work = ?1 AND answered = 0 ORDER BY at DESC, id DESC LIMIT 1",
                [work],
                |r| {
                    let chapters: String = r.get(2)?;
                    Ok(ReadSent {
                        id: r.get(0)?,
                        work: r.get(1)?,
                        chapters: serde_json::from_str(&chapters).unwrap_or_default(),
                        target: r.get(3)?,
                        at: r.get(4)?,
                    })
                },
            )
            .optional()?)
        })
    }

    /// Every unanswered send of a work is answered.
    pub fn answer_sent(&self, work: &str) -> Result<()> {
        self.with_personal(|c| {
            c.execute("UPDATE read_sent SET answered = 1 WHERE work = ?1", [work])?;
            Ok(())
        })
    }

    /// Say the highest chapter a tracker holds for a work: a max register, no-op with sync off.
    pub fn sync_read_mark(
        &self,
        tracker: &str,
        anilist_id: u32,
        chapter: u32,
        volume: Option<u32>,
    ) -> Result<()> {
        self.with_personal_tx(|tx| {
            let body =
                OpBody::ReadMark { tracker: tracker.to_owned(), anilist_id, chapter, volume };
            if let Some(op) = self.emit(tx, body)? {
                apply_read_op(tx, &op, 0, &mut Vec::new())?;
            }
            Ok(())
        })
    }

    /// The highest chapter and volume a tracker is known to hold for a work.
    pub fn read_mark_for(
        &self,
        tracker: &str,
        anilist_id: u32,
    ) -> Result<Option<(u32, Option<u32>)>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT chapter, volume FROM read_mark WHERE tracker = ?1 AND anilist_id = ?2",
                params![tracker, anilist_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
        })
    }
}

/// Turn the reading this database holds into this device's first ops: called once, when sync is
/// first turned on. Local works stay out.
pub(crate) fn backfill(tx: &Transaction<'_>, device: &str) -> Result<()> {
    let push = |tx: &Transaction<'_>, stamp: Stamp, body: &OpBody| -> Result<Op> {
        crate::sync::push_op(tx, device, stamp, body)
    };
    // Events, oldest first, each given the id its op will have.
    let events: Vec<(i64, i64, OpBody)> = {
        let mut stmt = tx.prepare(&format!(
            "SELECT id, {EFFECTIVE}, work, chapter, page, of_pages, strip_image, strip_offset,
                    novel_pos, completed, source_id, version_label, at
               FROM read_event WHERE uid IS NULL AND work LIKE 'a:%' ORDER BY {EFFECTIVE}, id"
        ))?;
        stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                OpBody::Read {
                    work: r.get(2)?,
                    chapter: r.get(3)?,
                    page: r.get(4)?,
                    of: r.get(5)?,
                    strip_image: r.get(6)?,
                    strip_offset: r.get(7)?,
                    novel_pos: r.get(8)?,
                    completed: r.get::<_, i64>(9)? != 0,
                    source_id: r.get(10)?,
                    version_label: r.get(11)?,
                    at: r.get(12)?,
                    queued_for: Vec::new(),
                    seen_unread: 0,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?
    };
    for (id, stamp, body) in events {
        // In order, after anything already in the log.
        let stamp = tick(crate::sync::hlc(tx)?, 0).max(stamp as u64);
        let op = push(tx, stamp, &body)?;
        tx.execute(
            "UPDATE read_event SET uid = ?2, stamp = ?3 WHERE id = ?1",
            params![id, format!("{device}:{}", op.seq), op.stamp as i64],
        )?;
    }

    // Watermarks and resets made with sync off, under the ids their ops give them.
    let adopts: Vec<(String, String, String, Option<String>, i64, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT uid, work, upto_chapter, upto_volume, stamp, at FROM read_adopt
              WHERE uid LIKE 'here:%' AND work LIKE 'a:%' ORDER BY stamp",
        )?;
        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })?
        .collect::<rusqlite::Result<_>>()?
    };
    for (uid, work, upto_chapter, upto_volume, stamp, at) in adopts {
        let body = OpBody::ReadAdopt { work, upto_chapter, upto_volume, at };
        let op = push(tx, stamp as u64, &body)?;
        tx.execute(
            "UPDATE read_adopt SET uid = ?2, stamp = ?3 WHERE uid = ?1",
            params![uid, format!("a:{device}:{}", op.seq), op.stamp as i64],
        )?;
    }
    let resets: Vec<(String, String, i64, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT uid, work, stamp, at FROM read_reset
              WHERE uid LIKE 'here:%' AND work LIKE 'a:%' AND undone IS NULL ORDER BY stamp",
        )?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    for (uid, work, stamp, at) in resets {
        let body = OpBody::ReadReset { work, at, undoes: None };
        let op = push(tx, stamp as u64, &body)?;
        tx.execute(
            "UPDATE read_reset SET uid = ?2, stamp = ?3 WHERE uid = ?1",
            params![uid, format!("{device}:{}", op.seq), op.stamp as i64],
        )?;
    }

    // Registers, dated by when each choice was made.
    type Reg = (&'static str, String, Option<String>, i64);
    let mut regs: Vec<Reg> = Vec::new();
    {
        let mut stmt = tx.prepare(
            "SELECT work, setting, value, updated_at FROM read_series_setting
              WHERE scope = 'series' AND work LIKE 'a:%'",
        )?;
        for row in stmt.query_map([], |r| {
            Ok((
                kinds::READ_SERIES,
                format!("{}/{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?),
                Some(r.get(2)?),
                r.get(3)?,
            ))
        })? {
            regs.push(row?);
        }
        let mut stmt = tx.prepare(
            "SELECT work, status, score, updated_at FROM read_status WHERE work LIKE 'a:%'",
        )?;
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<f64>>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })? {
            let (work, status, score, at) = row?;
            if status.is_some() {
                regs.push((kinds::READ_STATUS, work.clone(), status, at));
            }
            if let Some(score) = score {
                regs.push((kinds::READ_SCORE, work, Some(score.to_string()), at));
            }
        }
        let mut stmt =
            tx.prepare("SELECT work, chapter, at FROM read_bookmark WHERE work LIKE 'a:%'")?;
        for row in stmt.query_map([], |r| {
            Ok((
                kinds::READ_BOOKMARK,
                format!("{}/{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?),
                Some("1".to_owned()),
                r.get(2)?,
            ))
        })? {
            regs.push(row?);
        }
        let mut stmt = tx.prepare(
            "SELECT tracker, work, tracker_id, checked_at FROM read_track_map
              WHERE origin = 'hand' AND work LIKE 'a:%'",
        )?;
        for row in stmt.query_map([], |r| {
            Ok((
                kinds::READ_TRACK_MAP,
                format!("{}/{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?),
                Some(r.get::<_, Option<String>>(2)?.unwrap_or_default()),
                r.get(3)?,
            ))
        })? {
            regs.push(row?);
        }
    }
    for (kind, key, value, at) in regs {
        let stamp = tick(0, at * 1000);
        let body =
            OpBody::Reg { kind: kind.to_owned(), key: key.clone(), value: value.clone() };
        let op = push(tx, stamp, &body)?;
        crate::sync::set_reg(tx, kind, &key, op.stamp, device, value.as_deref())?;
    }

    // Works hidden from Continue, hidden after their newest read.
    let hidden: Vec<(String, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT work, stamp FROM read_work WHERE hidden = 1 AND work LIKE 'a:%'",
        )?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
    };
    for (work, newest) in hidden {
        let stamp = tick(newest as u64, 0);
        let body = OpBody::Reg {
            kind: kinds::READ_HIDE.to_owned(),
            key: work.clone(),
            value: Some("1".into()),
        };
        let op = push(tx, stamp, &body)?;
        crate::sync::set_reg(tx, kinds::READ_HIDE, &work, op.stamp, device, Some("1"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_watermark_covers_chapters_up_to_it_within_volumes() {
        assert!(covers(12_000, None, "12"));
        assert!(covers(12_000, None, "10.5"));
        assert!(!covers(12_000, None, "12.5"));
        assert!(!covers(12_000, None, "x/-/Extra"));
        assert!(covers(3_000, Some("2"), "v1/9"));
        assert!(covers(3_000, Some("2"), "v2/3"));
        assert!(!covers(3_000, Some("2"), "v2/4"));
        assert!(!covers(3_000, Some("2"), "v3/1"));
        assert!(!covers(30_000, None, "v2/3"));
    }

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    fn read(store: &Store, work: &str, chapter: &str, page: u32, completed: bool, at: i64) {
        let mut event = ReadEvent::new(work, chapter, at);
        event.page = Some(page);
        event.of_pages = Some(20);
        event.completed = completed;
        store.record_read(&event, Vec::new()).unwrap();
    }

    #[test]
    fn chapters_keep_their_newest_place_and_read_again_sets_them_aside() {
        let store = store();
        let work = "l:00112233445566778899aabbccddeeff";
        read(&store, work, "1", 20, true, 100);
        read(&store, work, "2", 8, false, 200);
        read(&store, work, "2", 12, false, 300);
        let chapters = store.read_chapters(work).unwrap();
        assert_eq!(chapters.len(), 2);
        assert!(chapters[0].read);
        assert_eq!((chapters[1].read, chapters[1].last_page), (false, Some(12)));
        let held = store.read_work(work).unwrap().unwrap();
        assert_eq!(held.last_chapter.as_deref(), Some("2"));

        let reset = store.reset_read(work, 400).unwrap();
        assert!(store.read_chapters(work).unwrap().is_empty());
        assert!(!store.chapter_is_read(work, "1").unwrap());
        read(&store, work, "1", 3, false, 500);
        assert_eq!(store.read_chapters(work).unwrap().len(), 1);

        store.undo_read_reset(work, &reset, 600).unwrap();
        assert!(store.chapter_is_read(work, "1").unwrap());
        assert_eq!(store.read_chapters(work).unwrap()[0].last_page, Some(3));
        assert_eq!(store.read_reset_in_force(work).unwrap(), None);
    }

    #[test]
    fn a_watermark_marks_chapters_a_source_lists_later_and_unread_takes_events_back() {
        let store = store();
        let work = "a:900101";
        store.adopt_read(work, "40", None, 100).unwrap();
        assert!(store.chapter_is_read(work, "38.5").unwrap());
        assert!(!store.chapter_is_read(work, "41").unwrap());
        read(&store, work, "41", 20, true, 200);
        assert!(store.chapter_is_read(work, "41").unwrap());
        store.unread(work, &["41".into()]).unwrap();
        assert!(!store.chapter_is_read(work, "41").unwrap());
    }

    #[test]
    fn unread_under_a_watermark_holds_until_a_newer_watermark() {
        let store = store();
        for work in ["a:900106", "l:9a01"] {
            store.adopt_read(work, "40", None, 100).unwrap();
            store.unread(work, &["12".into()]).unwrap();
            let marks = store.read_marks(work).unwrap();
            assert!(!marks.is_read("12"), "{work}");
            assert!(marks.is_read("13"), "{work}");
            store.adopt_read(work, "20", None, 300).unwrap();
            assert!(store.chapter_is_read(work, "12").unwrap(), "{work}");
            store.unread(work, &["12".into()]).unwrap();
            read(&store, work, "12", 20, true, 400);
            assert!(store.chapter_is_read(work, "12").unwrap(), "{work}");
        }
    }

    #[test]
    fn hidden_works_leave_continue_until_read_again() {
        let store = store();
        let work = "a:900102";
        read(&store, work, "1", 4, false, 100);
        assert_eq!(store.continue_reading(12).unwrap().len(), 1);
        store.set_read_hidden(work, true).unwrap();
        assert!(store.continue_reading(12).unwrap().is_empty());
        read(&store, work, "1", 5, false, 200);
        assert_eq!(store.continue_reading(12).unwrap().len(), 1);
    }

    #[test]
    fn series_choices_status_bookmarks_and_hand_mappings_are_kept() {
        let store = store();
        let work = "a:900103";
        store.set_series_setting(work, "direction", Some("rtl"), true, 10).unwrap();
        store.set_series_setting(work, "crop", Some("on"), true, 10).unwrap();
        let settings = store.series_settings(work).unwrap();
        assert_eq!(settings[0].scope, "here");
        assert_eq!(settings[1].scope, "series");
        store.set_read_status(work, Some("reading"), 10).unwrap();
        store.set_read_score(work, Some(8.5), 11).unwrap();
        assert_eq!(store.read_status(work).unwrap(), (Some("reading".into()), Some(8.5)));
        store.set_bookmark(work, "v2/3", true, 12).unwrap();
        assert_eq!(store.bookmarks(work).unwrap(), ["v2/3"]);
        assert!(store.set_track_map(work, "mal", Some("77"), "hand", 13).unwrap());
        assert!(!store.set_track_map(work, "mal", Some("78"), "search", 14).unwrap());
        assert_eq!(store.track_map(work).unwrap()[0].tracker_id.as_deref(), Some("77"));
        let sent =
            store.record_sent(work, &["4".into(), "5".into()], "Desk reader", 20).unwrap();
        assert_eq!(store.pending_sent(work).unwrap().unwrap().id, sent);
        store.answer_sent(work).unwrap();
        assert!(store.pending_sent(work).unwrap().is_none());
    }
}
