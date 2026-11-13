//! Reading progress waiting to be told to a book server: see `0030_book_progress_outbox`.
//!
//! One row per chapter of a series on a server holds the newest mark, so a chapter read, taken
//! back and read again goes out once, as read. A row the server took is deleted, unless a newer
//! mark replaced it while it was on the way. The store keeps why a try failed as the engine names
//! it, and does not read it.

use rusqlite::{OptionalExtension, params};

use crate::{Result, Store};

/// One mark waiting for its server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookProgressItem {
    pub server: String,
    pub series: String,
    /// The chapter as the catalogue keys it, not the server's own id.
    pub chapter: String,
    pub read: bool,
    /// When it was read or taken back.
    pub at: i64,
    /// Tries that failed since it was queued or last retried by hand.
    pub attempts: u32,
    /// When the first of those failed.
    pub first_failed_at: Option<i64>,
    /// Why it waits for Try again, if it does.
    pub held: Option<String>,
}

/// How writing progress back to one server stands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookProgressState {
    /// Marks waiting, held ones included.
    pub pending: u32,
    /// Of those, the ones waiting for Try again.
    pub held: u32,
    /// When the server last took one.
    pub sent_at: Option<i64>,
    /// Why the last try failed, while none has been taken since.
    pub problem: Option<String>,
    pub problem_at: Option<i64>,
}

/// The longest a mark waits between tries. A wait set further off than this was set by a clock
/// that ran ahead, and is over; a mark stamped further than this after a newer change's own time
/// was made by such a clock, and the newer change wins.
pub const LONGEST_WAIT: i64 = 3_600;

const COLUMNS: &str = "server, series, chapter, read, at, attempts, first_failed_at, held";

fn item(r: &rusqlite::Row<'_>) -> rusqlite::Result<BookProgressItem> {
    Ok(BookProgressItem {
        server: r.get(0)?,
        series: r.get(1)?,
        chapter: r.get(2)?,
        read: r.get(3)?,
        at: r.get(4)?,
        attempts: r.get(5)?,
        first_failed_at: r.get(6)?,
        held: r.get(7)?,
    })
}

impl Store {
    /// Queue `read` for `chapter` of `series` on `server`, marked at `at`, to go at `due_at`. An
    /// older mark for the chapter is replaced; a newer one stays, unless it is stamped more than
    /// [`LONGEST_WAIT`] after this one, which only a clock that ran ahead does: the person's
    /// latest change is what the server is told. A row that waits for Try again keeps waiting,
    /// with the new mark.
    pub fn queue_book_progress(
        &self,
        server: &str,
        series: &str,
        chapter: &str,
        read: bool,
        at: i64,
        due_at: i64,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO book_progress_outbox (server, series, chapter, read, at, due_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (server, series, chapter) DO UPDATE SET
                    read = excluded.read, at = excluded.at
                  WHERE excluded.at >= book_progress_outbox.at
                     OR book_progress_outbox.at - excluded.at > ?7",
                params![server, series, chapter, read, at, due_at, LONGEST_WAIT],
            )?;
            Ok(())
        })
    }

    /// The mark waiting for `chapter` of `series` on `server`, if one is.
    pub fn book_progress_item(
        &self,
        server: &str,
        series: &str,
        chapter: &str,
    ) -> Result<Option<BookProgressItem>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                &format!(
                    "SELECT {COLUMNS} FROM book_progress_outbox
                      WHERE server = ?1 AND series = ?2 AND chapter = ?3"
                ),
                params![server, series, chapter],
                item,
            )
            .optional()?)
        })
    }

    /// Up to `limit` marks due at `now`, the oldest mark first, none that waits for Try again. A
    /// wait set further off than [`LONGEST_WAIT`] is due.
    pub fn due_book_progress(&self, now: i64, limit: u32) -> Result<Vec<BookProgressItem>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(&format!(
                "SELECT {COLUMNS} FROM book_progress_outbox
                  WHERE held IS NULL AND (due_at <= ?1 OR due_at > ?1 + ?3)
                  ORDER BY at, server, series, chapter
                  LIMIT ?2"
            ))?;
            let rows = statement.query_map(params![now, limit, LONGEST_WAIT], item)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// When the next mark is due as seen at `now`, if one waits that is not held: `now` for one
    /// whose wait is further off than [`LONGEST_WAIT`].
    pub fn next_book_progress_due(&self, now: i64) -> Result<Option<i64>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT MIN(CASE WHEN due_at > ?1 + ?2 THEN ?1 ELSE due_at END)
                   FROM book_progress_outbox WHERE held IS NULL",
                params![now, LONGEST_WAIT],
                |r| r.get(0),
            )?)
        })
    }

    /// The server took `item` at `now`: the row goes, unless a newer mark replaced it meanwhile.
    pub fn book_progress_sent(&self, item: &BookProgressItem, now: i64) -> Result<()> {
        self.with_tx(|tx| {
            tx.execute(
                "DELETE FROM book_progress_outbox
                  WHERE server = ?1 AND series = ?2 AND chapter = ?3 AND at = ?4 AND read = ?5",
                params![item.server, item.series, item.chapter, item.at, item.read],
            )?;
            tx.execute(
                "INSERT INTO book_progress_state (server, sent_at) VALUES (?1, ?2)
                 ON CONFLICT (server) DO UPDATE SET
                    sent_at = excluded.sent_at, problem = NULL, problem_at = NULL",
                params![item.server, now],
            )?;
            Ok(())
        })
    }

    /// `item` failed at `now` for `problem`: tried again at `due_at`, or, with `hold`, kept until
    /// Try again.
    pub fn book_progress_failed(
        &self,
        item: &BookProgressItem,
        problem: &str,
        hold: bool,
        due_at: i64,
        now: i64,
    ) -> Result<()> {
        self.with_tx(|tx| {
            tx.execute(
                "UPDATE book_progress_outbox SET
                    attempts = attempts + 1, due_at = ?4, problem = ?5,
                    first_failed_at = COALESCE(first_failed_at, ?6),
                    held = CASE WHEN ?7 THEN ?5 ELSE NULL END
                  WHERE server = ?1 AND series = ?2 AND chapter = ?3",
                params![item.server, item.series, item.chapter, due_at, problem, now, hold],
            )?;
            tx.execute(
                "INSERT INTO book_progress_state (server, problem, problem_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (server) DO UPDATE SET
                    problem = excluded.problem, problem_at = excluded.problem_at",
                params![item.server, problem, now],
            )?;
            Ok(())
        })
    }

    /// Try every mark for `server` at `now` with a fresh day of retries: Try again. Held marks
    /// are let go to be tried too.
    pub fn retry_book_progress(&self, server: &str, now: i64) -> Result<u32> {
        self.with_conn(|c| {
            let changed = c.execute(
                "UPDATE book_progress_outbox SET
                    due_at = ?2, attempts = 0, first_failed_at = NULL, held = NULL
                  WHERE server = ?1",
                params![server, now],
            )?;
            Ok(u32::try_from(changed).unwrap_or(u32::MAX))
        })
    }

    /// Back online: every mark not held is due at `now`, the oldest still first.
    pub fn book_progress_due_now(&self, now: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE book_progress_outbox SET due_at = ?1 WHERE held IS NULL AND due_at > ?1",
                [now],
            )?;
            Ok(())
        })
    }

    /// Forget every mark for `server` and how it went: it was removed, or set not to receive.
    pub fn forget_book_progress(&self, server: &str) -> Result<()> {
        self.with_tx(|tx| {
            tx.execute("DELETE FROM book_progress_outbox WHERE server = ?1", [server])?;
            tx.execute("DELETE FROM book_progress_state WHERE server = ?1", [server])?;
            Ok(())
        })
    }

    /// How writing back to `server` stands.
    pub fn book_progress_state(&self, server: &str) -> Result<BookProgressState> {
        self.with_conn(|c| {
            let (pending, held): (u32, u32) = c.query_row(
                "SELECT COUNT(*), COUNT(held) FROM book_progress_outbox WHERE server = ?1",
                [server],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let kept = c
                .query_row(
                    "SELECT sent_at, problem, problem_at FROM book_progress_state
                      WHERE server = ?1",
                    [server],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let (sent_at, problem, problem_at) = kept.unwrap_or((None, None, None));
            Ok(BookProgressState { pending, held, sent_at, problem, problem_at })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    #[test]
    fn the_newest_mark_for_a_chapter_wins() {
        let store = store();
        store.queue_book_progress("k", "s1", "c1", true, 100, 100).unwrap();
        store.queue_book_progress("k", "s1", "c1", false, 200, 200).unwrap();
        // A mark older than the one kept arrives late: it changes nothing.
        store.queue_book_progress("k", "s1", "c1", true, 150, 150).unwrap();
        store.queue_book_progress("k", "s1", "c2", true, 120, 120).unwrap();
        let due = store.due_book_progress(1_000, 10).unwrap();
        assert_eq!(due.len(), 2);
        assert_eq!((due[0].chapter.as_str(), due[0].read), ("c2", true));
        assert_eq!((due[1].chapter.as_str(), due[1].read, due[1].at), ("c1", false, 200));
        assert_eq!(store.book_progress_state("k").unwrap().pending, 2);
    }

    #[test]
    fn a_sent_mark_goes_unless_a_newer_one_replaced_it_on_the_way() {
        let store = store();
        store.queue_book_progress("k", "s1", "c1", true, 100, 100).unwrap();
        let sending = store.due_book_progress(100, 10).unwrap().remove(0);
        store.queue_book_progress("k", "s1", "c1", false, 110, 110).unwrap();
        store.book_progress_sent(&sending, 120).unwrap();
        let left = store.due_book_progress(1_000, 10).unwrap();
        assert_eq!(left.len(), 1);
        assert!(!left[0].read);
        store.book_progress_sent(&left[0], 130).unwrap();
        let state = store.book_progress_state("k").unwrap();
        assert_eq!((state.pending, state.sent_at, state.problem), (0, Some(130), None));
    }

    #[test]
    fn a_failure_waits_its_turn_and_a_held_one_waits_for_try_again() {
        let store = store();
        store.queue_book_progress("k", "s1", "c1", true, 100, 100).unwrap();
        store.queue_book_progress("k", "s1", "c2", true, 101, 101).unwrap();
        let due = store.due_book_progress(200, 10).unwrap();
        store.book_progress_failed(&due[0], "no-answer", false, 230, 200).unwrap();
        store.book_progress_failed(&due[1], "refused-key", true, 200, 200).unwrap();
        assert!(store.due_book_progress(229, 10).unwrap().is_empty());
        let again = store.due_book_progress(230, 10).unwrap();
        assert_eq!(again.len(), 1);
        assert_eq!((again[0].attempts, again[0].first_failed_at), (1, Some(200)));
        assert_eq!(store.next_book_progress_due(200).unwrap(), Some(230));
        let state = store.book_progress_state("k").unwrap();
        assert_eq!((state.pending, state.held), (2, 1));
        assert_eq!(state.problem.as_deref(), Some("refused-key"));

        // A new mark for a held chapter is kept, and still waits for Try again.
        store.queue_book_progress("k", "s1", "c2", false, 300, 300).unwrap();
        assert_eq!(store.due_book_progress(10_000, 10).unwrap().len(), 1);
        assert_eq!(store.retry_book_progress("k", 400).unwrap(), 2);
        let all = store.due_book_progress(400, 10).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().all(|i| i.attempts == 0 && i.held.is_none()));
    }

    #[test]
    fn back_online_brings_waiting_marks_due_and_forgetting_a_server_clears_it() {
        let store = store();
        store.queue_book_progress("k", "s1", "c1", true, 100, 100).unwrap();
        let due = store.due_book_progress(100, 10).unwrap();
        store.book_progress_failed(&due[0], "no-answer", false, 3_700, 100).unwrap();
        store.book_progress_due_now(200).unwrap();
        assert_eq!(store.due_book_progress(200, 10).unwrap().len(), 1);
        store.forget_book_progress("k").unwrap();
        assert_eq!(store.book_progress_state("k").unwrap(), BookProgressState::default());
    }
    #[test]
    fn a_wait_set_while_the_clock_ran_ahead_is_due_once_it_is_put_right() {
        let store = store();
        let ahead = 100 + 365 * 86_400;
        store.queue_book_progress("k", "s1", "c1", true, ahead, ahead).unwrap();
        let due = store.due_book_progress(ahead, 10).unwrap();
        store.book_progress_failed(&due[0], "no-answer", false, ahead + 30, ahead).unwrap();
        // The clock put right.
        assert_eq!(store.next_book_progress_due(2_000).unwrap(), Some(2_000));
        assert_eq!(store.due_book_progress(2_000, 10).unwrap().len(), 1);
        // A wait set at the right time still waits.
        store.book_progress_failed(&due[0], "no-answer", false, 2_030, 2_000).unwrap();
        assert!(store.due_book_progress(2_000, 10).unwrap().is_empty());
    }

    #[test]
    fn a_change_after_a_mark_made_while_the_clock_ran_ahead_wins() {
        let store = store();
        let ahead = 100 + 365 * 86_400;
        store.queue_book_progress("k", "s1", "c1", true, ahead, ahead).unwrap();
        // The clock put right, the chapter taken back.
        store.queue_book_progress("k", "s1", "c1", false, 2_000, 2_000).unwrap();
        let due = store.due_book_progress(2_000, 10).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!((due[0].read, due[0].at), (false, 2_000), "the latest change goes");
    }
}
