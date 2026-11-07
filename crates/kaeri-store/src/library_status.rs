//! Where each title is on the signed-in tracker's list, as its last answer said: see
//! `0008_library_status`. A copy of that answer, so a title's page can say which of its related
//! titles were watched without a request of its own.

use kaeri_core::ids::AnilistId;
use rusqlite::OptionalExtension;

use crate::{Result, Store};

/// Every title with a status: the tracker's list, and under it what a person set on a device
/// (the sync register), for the titles the tracker's list does not have. The tracker wins; the
/// register fills the gaps.
pub(crate) const MERGED: &str = "(SELECT anilist_id, status FROM library_status
     UNION ALL
     SELECT CAST(key AS INTEGER), value FROM sync_reg
      WHERE kind = 'status'
        AND value IN ('CURRENT', 'REPEATING', 'PLANNING', 'COMPLETED', 'PAUSED', 'DROPPED')
        AND CAST(key AS INTEGER) NOT IN (SELECT anilist_id FROM library_status))";

impl Store {
    /// Put `tracker`'s whole list in place of what was kept of it: each title and its status,
    /// as the tracker words it (`COMPLETED`). A title no longer on the list is on none.
    pub fn replace_library_statuses(
        &self,
        tracker: &str,
        statuses: &[(AnilistId, &str)],
        at: i64,
    ) -> Result<()> {
        self.with_personal_tx(|tx| {
            // Every row, whichever tracker wrote it: there is one list, the Library's.
            tx.execute("DELETE FROM library_status", [])?;
            let mut insert = tx.prepare(
                "INSERT INTO library_status (anilist_id, status, tracker_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(anilist_id) DO UPDATE SET status = excluded.status",
            )?;
            for (id, status) in statuses {
                insert.execute(rusqlite::params![id.get(), status, tracker, at])?;
            }
            Ok(())
        })
    }

    /// Put one title on `tracker`'s list as kept, ahead of the tracker's next answer: a status
    /// set here and queued for it, as the tracker words it (`PLANNING`).
    pub fn set_library_status(
        &self,
        tracker: &str,
        id: AnilistId,
        status: &str,
        at: i64,
    ) -> Result<()> {
        self.with_personal(|c| {
            c.execute(
                "INSERT INTO library_status (anilist_id, status, tracker_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(anilist_id) DO UPDATE
                    SET status = excluded.status, updated_at = excluded.updated_at",
                rusqlite::params![id.get(), status, tracker, at],
            )?;
            Ok(())
        })
    }

    /// The shows being watched: on the list as watching or rewatching, when `listed` says the
    /// list is a signed-in tracker's, and watched here since `since`, unless the list has them
    /// dropped or paused, or they were taken off Continue. The most recently watched here
    /// first, then by id. The new-episode check follows the same shows.
    pub fn watching(&self, since: i64, listed: bool) -> Result<Vec<AnilistId>> {
        self.with_personal(|c| {
            let mut statement = c.prepare(&format!(
                "SELECT id FROM (
                     SELECT s.anilist_id AS id, COALESCE(p.updated_at, 0) AS at
                       FROM {MERGED} s
                       LEFT JOIN watch_progress p ON p.anilist_id = s.anilist_id
                      WHERE ?2 AND s.status IN ('CURRENT', 'REPEATING')
                     UNION ALL
                     SELECT p.anilist_id, p.updated_at FROM watch_progress p
                      WHERE p.hidden = 0 AND p.updated_at >= ?1
                        AND NOT (?2 AND EXISTS (
                            SELECT 1 FROM {MERGED} s
                             WHERE s.anilist_id = p.anilist_id
                               AND s.status IN ('DROPPED', 'PAUSED')))
                 )
                 GROUP BY id
                 ORDER BY MAX(at) DESC, id",
            ))?;
            let ids = statement
                .query_map(rusqlite::params![since, listed], |r| r.get::<_, u32>(0))?
                .map(|id| id.map(AnilistId::new))
                .collect::<rusqlite::Result<_>>()?;
            Ok(ids)
        })
    }

    /// The score a person gave `id`: the tracker's when the list has one (`tracked`), else what a
    /// device set.
    pub fn score_for(&self, id: AnilistId, tracked: Option<f32>) -> Result<Option<f32>> {
        match tracked {
            Some(score) => Ok(Some(score)),
            None => self.synced_score(id.get()),
        }
    }

    /// Whether any title has a status set on a device, for a list that has no tracker behind it.
    pub fn has_synced_statuses(&self) -> Result<bool> {
        Ok(!self.sync_regs(crate::sync::op::kinds::STATUS)?.is_empty())
    }

    /// The titles the list has with `status` (as the tracker words it: `PLANNING`), newest ids
    /// first: the newest titles are the likeliest to be airing.
    pub fn library_with_status(&self, status: &str) -> Result<Vec<AnilistId>> {
        self.with_personal(|c| {
            let mut statement = c.prepare(&format!(
                "SELECT anilist_id FROM {MERGED} WHERE status = ?1 ORDER BY anilist_id DESC",
            ))?;
            let ids = statement
                .query_map([status], |r| r.get::<_, u32>(0))?
                .map(|id| id.map(AnilistId::new))
                .collect::<rusqlite::Result<_>>()?;
            Ok(ids)
        })
    }

    /// Where `id` is on the list, as the tracker words it, when it is on it: the tracker's list
    /// first, and for a title it does not have, what a person set on a device.
    pub fn library_status(&self, id: AnilistId) -> Result<Option<String>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                &format!("SELECT status FROM {MERGED} WHERE anilist_id = ?1"),
                [id.get()],
                |r| r.get(0),
            )
            .optional()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: AnilistId = AnilistId::new(900_001);
    const TWO: AnilistId = AnilistId::new(900_003);

    #[test]
    fn a_list_is_kept_and_read_back_by_title() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.library_status(ONE).unwrap(), None, "nothing pulled yet");
        store
            .replace_library_statuses("anilist", &[(ONE, "COMPLETED"), (TWO, "PLANNING")], 10)
            .unwrap();
        assert_eq!(store.library_status(ONE).unwrap().as_deref(), Some("COMPLETED"));
        assert_eq!(store.library_status(TWO).unwrap().as_deref(), Some("PLANNING"));
    }

    #[test]
    fn a_status_set_here_is_kept_over_the_lists_until_the_next_answer() {
        let store = Store::open_in_memory().unwrap();
        store.replace_library_statuses("anilist", &[(ONE, "CURRENT")], 10).unwrap();
        // Over the tracker's word for a title on the list, and the sync register's below it.
        store.set_synced_status(ONE.get(), Some("PLANNING")).unwrap();
        assert_eq!(store.library_status(ONE).unwrap().as_deref(), Some("CURRENT"));
        store.set_library_status("anilist", ONE, "PLANNING", 11).unwrap();
        store.set_library_status("anilist", TWO, "PAUSED", 11).unwrap();
        assert_eq!(store.library_status(ONE).unwrap().as_deref(), Some("PLANNING"));
        assert_eq!(store.library_with_status("PAUSED").unwrap(), vec![TWO]);

        // The tracker's next answer is the list again.
        store.replace_library_statuses("anilist", &[(ONE, "COMPLETED")], 12).unwrap();
        assert_eq!(store.library_status(ONE).unwrap().as_deref(), Some("COMPLETED"));
        assert_eq!(store.library_status(TWO).unwrap(), None);
    }

    #[test]
    fn the_shows_being_watched_are_the_lists_and_the_ones_watched_here_lately() {
        let store = Store::open_in_memory().unwrap();
        const THREE: AnilistId = AnilistId::new(900_005);
        const FOUR: AnilistId = AnilistId::new(900_006);
        const FIVE: AnilistId = AnilistId::new(900_002);
        store
            .replace_library_statuses(
                "anilist",
                &[(ONE, "CURRENT"), (TWO, "REPEATING"), (THREE, "PAUSED"), (FOUR, "COMPLETED")],
                10,
            )
            .unwrap();
        assert_eq!(store.watching(100, true).unwrap(), [ONE, TWO], "by id, nothing watched");
        assert!(store.watching(100, false).unwrap().is_empty(), "nobody signed in to it");

        let watch = |id, at| {
            let mut event = crate::WatchEvent::new(id, "1", 30.0);
            event.at = at;
            store.record_event(&event).unwrap();
        };
        watch(TWO, 150);
        // Watched here lately: one paused on the list, one completed, one not on it at all.
        watch(THREE, 200);
        watch(FOUR, 210);
        watch(FIVE, 220);
        // And one watched too long ago.
        watch(AnilistId::new(900_003), 50);
        assert_eq!(store.watching(100, true).unwrap(), [FIVE, FOUR, TWO, ONE]);
        assert_eq!(store.watching(100, false).unwrap(), [FIVE, FOUR, THREE, TWO]);
        store.hide_from_continue(FIVE).unwrap();
        assert_eq!(store.watching(100, false).unwrap(), [FOUR, THREE, TWO]);
    }

    #[test]
    fn the_tracker_wins_and_the_register_fills_the_gaps() {
        const THREE: AnilistId = AnilistId::new(900_005);
        let store = Store::open_in_memory().unwrap();
        store.sync_enable("me").unwrap();
        store.replace_library_statuses("anilist", &[(ONE, "CURRENT")], 10).unwrap();
        // A device set ONE to completed and TWO to dropped; the tracker's list has only ONE.
        store.set_synced_status(ONE.get(), Some("COMPLETED")).unwrap();
        store.set_synced_status(TWO.get(), Some("DROPPED")).unwrap();
        store.set_synced_status(THREE.get(), Some("not a status")).unwrap();
        assert_eq!(store.library_status(ONE).unwrap().as_deref(), Some("CURRENT"), "tracker");
        assert_eq!(store.library_status(TWO).unwrap().as_deref(), Some("DROPPED"), "filled");
        assert_eq!(store.library_status(THREE).unwrap(), None, "a word it does not know");
        assert_eq!(
            store.library_statuses().unwrap(),
            [(ONE, "CURRENT".to_owned()), (TWO, "DROPPED".to_owned())]
        );
        assert_eq!(store.library_with_status("DROPPED").unwrap(), [TWO]);
        assert!(store.library_with_status("COMPLETED").unwrap().is_empty());
        // Dropped on a device: not followed, even with watching done here lately.
        let mut event = crate::WatchEvent::new(TWO, "1", 30.0);
        event.at = 200;
        store.record_event(&event).unwrap();
        assert_eq!(store.watching(100, true).unwrap(), [ONE]);
        // The tracker's list gaining TWO ends the filling for it.
        store
            .replace_library_statuses("anilist", &[(ONE, "CURRENT"), (TWO, "CURRENT")], 20)
            .unwrap();
        assert_eq!(store.library_status(TWO).unwrap().as_deref(), Some("CURRENT"));
    }

    #[test]
    fn a_score_is_the_trackers_and_the_register_fills_the_gap() {
        let store = Store::open_in_memory().unwrap();
        store.sync_enable("me").unwrap();
        store.set_synced_score(ONE.get(), Some(8.5)).unwrap();
        assert_eq!(store.score_for(ONE, Some(7.0)).unwrap(), Some(7.0), "the tracker's");
        assert_eq!(store.score_for(ONE, None).unwrap(), Some(8.5), "the register fills it");
        assert_eq!(store.score_for(TWO, None).unwrap(), None);
    }

    #[test]
    fn each_answer_replaces_the_last_whole() {
        let store = Store::open_in_memory().unwrap();
        store
            .replace_library_statuses("anilist", &[(ONE, "CURRENT"), (TWO, "PLANNING")], 10)
            .unwrap();
        store.replace_library_statuses("anilist", &[(ONE, "COMPLETED")], 20).unwrap();
        assert_eq!(store.library_status(ONE).unwrap().as_deref(), Some("COMPLETED"));
        assert_eq!(store.library_status(TWO).unwrap(), None, "taken off the list");
    }
}
