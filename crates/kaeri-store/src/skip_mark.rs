//! Where the viewer marked an episode's opening, ending or recap, and their votes on AniSkip's
//! times: see `0018_skip_marks`.
//!
//! A mark is kept as AniSkip names its kind (`op`, `ed` or `recap`) and in milliseconds, which
//! is what the playback layer reads it back as. One mark per kind per episode: marking again
//! keeps the latest.

use kaeri_core::ids::AnilistId;

use crate::{Result, Store};

/// One kind of segment the viewer marked in one episode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipMark {
    pub anilist_id: AnilistId,
    pub episode: String,
    /// AniSkip's name for the kind: `op`, `ed` or `recap`.
    pub kind: String,
    pub start_ms: i64,
    pub end_ms: i64,
    /// The episode's length when it was marked.
    pub length_ms: i64,
    pub marked_at: i64,
    /// The AniSkip time this mark was sent as, once it has been.
    pub skip_id: Option<String>,
}

impl Store {
    /// The marks for one episode, by kind.
    pub fn skip_marks(&self, anilist_id: AnilistId, episode: &str) -> Result<Vec<SkipMark>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT anilist_id, episode, kind, start_ms, end_ms, length_ms, marked_at, skip_id
                   FROM skip_marks WHERE anilist_id = ?1 AND episode = ?2 ORDER BY kind",
            )?;
            let rows = statement.query_map(rusqlite::params![anilist_id.get(), episode], |r| {
                Ok(SkipMark {
                    anilist_id: AnilistId::new(r.get(0)?),
                    episode: r.get(1)?,
                    kind: r.get(2)?,
                    start_ms: r.get(3)?,
                    end_ms: r.get(4)?,
                    length_ms: r.get(5)?,
                    marked_at: r.get(6)?,
                    skip_id: r.get(7)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Keep `mark` in place of the one for its kind, unless that one was made later: a mark
    /// from another device that arrives late does not undo a newer one.
    pub fn put_skip_mark(&self, mark: &SkipMark) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO skip_marks
                    (anilist_id, episode, kind, start_ms, end_ms, length_ms, marked_at, skip_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(anilist_id, episode, kind) DO UPDATE SET
                    start_ms  = excluded.start_ms,
                    end_ms    = excluded.end_ms,
                    length_ms = excluded.length_ms,
                    marked_at = excluded.marked_at,
                    skip_id   = excluded.skip_id
                 WHERE excluded.marked_at >= skip_marks.marked_at",
                rusqlite::params![
                    mark.anilist_id.get(),
                    mark.episode,
                    mark.kind,
                    mark.start_ms,
                    mark.end_ms,
                    mark.length_ms,
                    mark.marked_at,
                    mark.skip_id,
                ],
            )?;
            Ok(())
        })
    }

    /// Note the AniSkip time a mark was sent as.
    pub fn set_skip_mark_id(
        &self,
        anilist_id: AnilistId,
        episode: &str,
        kind: &str,
        skip_id: &str,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE skip_marks SET skip_id = ?4
                  WHERE anilist_id = ?1 AND episode = ?2 AND kind = ?3",
                rusqlite::params![anilist_id.get(), episode, kind, skip_id],
            )?;
            Ok(())
        })
    }

    /// Forget the viewer's mark of one kind in one episode. Whether there was one.
    pub fn clear_skip_mark(
        &self,
        anilist_id: AnilistId,
        episode: &str,
        kind: &str,
    ) -> Result<bool> {
        self.with_conn(|c| {
            let gone = c.execute(
                "DELETE FROM skip_marks WHERE anilist_id = ?1 AND episode = ?2 AND kind = ?3",
                rusqlite::params![anilist_id.get(), episode, kind],
            )?;
            Ok(gone > 0)
        })
    }

    /// Forget every mark the viewer made, and their votes and what waits to be sent with them:
    /// what "Clear skip times you marked" does. How many marks there were.
    pub fn clear_skip_marks(&self) -> Result<u32> {
        self.with_tx(|tx| {
            let gone = tx.execute("DELETE FROM skip_marks", [])?;
            tx.execute("DELETE FROM skip_votes", [])?;
            tx.execute("DELETE FROM aniskip_outbox", [])?;
            Ok(u32::try_from(gone).unwrap_or(u32::MAX))
        })
    }

    /// How many marks the viewer has made, for the Settings row.
    pub fn skip_mark_count(&self) -> Result<u32> {
        self.with_conn(
            |c| Ok(c.query_row("SELECT COUNT(*) FROM skip_marks", [], |r| r.get(0))?),
        )
    }

    /// Note the viewer's vote on AniSkip time `skip_id`: -1 reports it, 1 confirms it. The
    /// first vote stands, as AniSkip counts only one. Whether this one was new.
    pub fn record_skip_vote(&self, skip_id: &str, vote: i8, at: i64) -> Result<bool> {
        self.with_conn(|c| {
            let added = c.execute(
                "INSERT INTO skip_votes (skip_id, vote, at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(skip_id) DO NOTHING",
                rusqlite::params![skip_id, vote.signum(), at],
            )?;
            Ok(added > 0)
        })
    }

    /// Of `skip_ids`, the ones the viewer reported: times not to use on this device.
    pub fn reported_skips(&self, skip_ids: &[&str]) -> Result<Vec<String>> {
        if skip_ids.is_empty() {
            return Ok(Vec::new());
        }
        self.with_conn(|c| {
            let mut statement =
                c.prepare("SELECT 1 FROM skip_votes WHERE skip_id = ?1 AND vote < 0")?;
            let mut reported = Vec::new();
            for id in skip_ids {
                if statement.exists([id])? {
                    reported.push((*id).to_owned());
                }
            }
            Ok(reported)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_101);

    fn store() -> Store {
        Store::open_in_memory().expect("store")
    }

    fn mark(kind: &str, start_ms: i64, marked_at: i64) -> SkipMark {
        SkipMark {
            anilist_id: SHOW,
            episode: "3".into(),
            kind: kind.into(),
            start_ms,
            end_ms: start_ms + 90_000,
            length_ms: 1_420_000,
            marked_at,
            skip_id: None,
        }
    }

    #[test]
    fn a_mark_is_read_back_for_its_episode_only() {
        let store = store();
        store.put_skip_mark(&mark("op", 61_000, 10)).unwrap();
        store.put_skip_mark(&mark("ed", 1_300_000, 10)).unwrap();
        let read = store.skip_marks(SHOW, "3").unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[1], mark("op", 61_000, 10));
        assert!(store.skip_marks(SHOW, "4").unwrap().is_empty());
    }

    #[test]
    fn the_latest_mark_of_a_kind_wins() {
        let store = store();
        store.put_skip_mark(&mark("op", 61_000, 20)).unwrap();
        // Older, as from another device that comes late: kept out.
        store.put_skip_mark(&mark("op", 70_000, 10)).unwrap();
        assert_eq!(store.skip_marks(SHOW, "3").unwrap()[0].start_ms, 61_000);
        store.put_skip_mark(&mark("op", 75_000, 30)).unwrap();
        assert_eq!(store.skip_marks(SHOW, "3").unwrap()[0].start_ms, 75_000);
    }

    #[test]
    fn a_mark_cleared_by_kind_leaves_the_others() {
        let store = store();
        store.put_skip_mark(&mark("op", 61_000, 10)).unwrap();
        store.put_skip_mark(&mark("recap", 0, 10)).unwrap();
        assert!(store.clear_skip_mark(SHOW, "3", "op").unwrap());
        assert!(!store.clear_skip_mark(SHOW, "3", "op").unwrap());
        let left = store.skip_marks(SHOW, "3").unwrap();
        assert_eq!(left.iter().map(|m| m.kind.as_str()).collect::<Vec<_>>(), ["recap"]);
    }

    #[test]
    fn clearing_every_mark_forgets_the_votes_too() {
        let store = store();
        store.put_skip_mark(&mark("op", 61_000, 10)).unwrap();
        store.record_skip_vote("time-1", -1, 10).unwrap();
        assert_eq!(store.skip_mark_count().unwrap(), 1);
        assert_eq!(store.clear_skip_marks().unwrap(), 1);
        assert_eq!(store.skip_mark_count().unwrap(), 0);
        assert!(store.reported_skips(&["time-1"]).unwrap().is_empty());
    }

    #[test]
    fn the_first_vote_on_a_time_stands() {
        let store = store();
        assert!(store.record_skip_vote("time-1", 1, 10).unwrap());
        assert!(!store.record_skip_vote("time-1", -1, 20).unwrap());
        assert!(store.reported_skips(&["time-1"]).unwrap().is_empty());
        store.record_skip_vote("time-2", -5, 20).unwrap();
        assert_eq!(store.reported_skips(&["time-1", "time-2", "time-3"]).unwrap(), ["time-2"]);
    }

    #[test]
    fn the_id_a_mark_was_sent_as_is_kept() {
        let store = store();
        store.put_skip_mark(&mark("ed", 1_300_000, 10)).unwrap();
        store.set_skip_mark_id(SHOW, "3", "ed", "time-9").unwrap();
        assert_eq!(store.skip_marks(SHOW, "3").unwrap()[0].skip_id.as_deref(), Some("time-9"));
    }
}
