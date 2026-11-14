//! Chapters downloaded for reading, and the queue that fetches them. All of it is this device's.
//!
//! A row is `queued`, `downloading`, `done` or `failed`. `removed` is a mark, not a download:
//! it records that a chapter fetched ahead on its own was let go of by hand, so keeping chapters
//! ready does not fetch it again. A download asked for by hand takes the row over. Finished
//! downloads reach the reader through [`Store::shelf`](crate::Store::shelf).
//!
//! Two engines can have one data folder open, as an app and the terminal do. A chapter being
//! fetched is the engine's that claimed it ([`Store::claim_book_download`]), for as long as that
//! engine keeps saying so ([`LEASE_SECS`]): the other engine neither fetches it nor puts it back
//! in the queue, and takes it over only once its owner has stopped saying so.

use rusqlite::{OptionalExtension, Row, params};

use crate::{Result, Store};

/// Where a chapter's download is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookDownloadState {
    Queued,
    Downloading,
    Done,
    Failed,
    Removed,
}

impl BookDownloadState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Removed => "removed",
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "downloading" => Self::Downloading,
            "done" => Self::Done,
            "failed" => Self::Failed,
            "removed" => Self::Removed,
            _ => Self::Queued,
        }
    }
}

/// One chapter's download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookDownload {
    pub id: i64,
    pub work: String,
    pub chapter: String,
    pub source_id: String,
    pub version_label: Option<String>,
    pub state: BookDownloadState,
    pub path: Option<String>,
    pub pages_done: u32,
    pub pages_total: Option<u32>,
    pub bytes: u64,
    /// Why it failed, in the engine's own words for it.
    pub error: Option<String>,
    /// Fetched ahead on its own rather than asked for.
    pub keep_ready: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// What is asked for when a chapter is queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookDownloadAsk<'a> {
    pub work: &'a str,
    pub chapter: &'a str,
    pub source_id: &'a str,
    pub version_label: Option<&'a str>,
    pub keep_ready: bool,
}

/// How a request to queue a chapter came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Queued {
    /// A new row, or a failed one asked for again.
    Added,
    /// Already queued, fetching or done; asked for by hand, it is no longer one kept ready.
    Had,
    /// Left alone: kept ready, and let go of by hand before.
    Skipped,
}

/// How long a claim on a chapter lasts without word from the engine fetching it: it renews the
/// claim with each page and every few seconds between ([`Store::book_download_heartbeat`]).
pub const LEASE_SECS: i64 = 45;

const COLUMNS: &str = "id, work, chapter, source_id, version_label, state, path, pages_done,
    pages_total, bytes, error, keep_ready, created_at, updated_at";

/// The SQL for a claim that has lapsed at `now` (with [`LEASE_SECS`] as `?2`): its engine said
/// nothing for the length of a lease, or it runs out further off than a lease, which only a
/// clock that ran ahead sets.
fn lapsed(now: &str) -> String {
    format!("(lease_until < {now} OR lease_until > {now} + ?2)")
}

fn row_of(r: &Row<'_>) -> rusqlite::Result<BookDownload> {
    let state: String = r.get(5)?;
    Ok(BookDownload {
        id: r.get(0)?,
        work: r.get(1)?,
        chapter: r.get(2)?,
        source_id: r.get(3)?,
        version_label: r.get(4)?,
        state: BookDownloadState::parse(&state),
        path: r.get(6)?,
        pages_done: u32::try_from(r.get::<_, i64>(7)?).unwrap_or(0),
        pages_total: r.get::<_, Option<i64>>(8)?.and_then(|n| u32::try_from(n).ok()),
        bytes: u64::try_from(r.get::<_, i64>(9)?).unwrap_or(0),
        error: r.get(10)?,
        keep_ready: r.get::<_, i64>(11)? != 0,
        created_at: r.get(12)?,
        updated_at: r.get(13)?,
    })
}

impl Store {
    /// Queue a chapter. A chapter that has a row is not queued twice: a failed one is queued
    /// again, one let go of is queued again only when asked for by hand.
    pub fn queue_book_download(&self, ask: &BookDownloadAsk<'_>, now: i64) -> Result<Queued> {
        self.with_tx(|tx| {
            let had: Option<(String, i64)> = tx
                .query_row(
                    "SELECT state, keep_ready FROM book_download
                      WHERE work = ?1 AND chapter = ?2",
                    params![ask.work, ask.chapter],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let fresh = |tx: &rusqlite::Transaction<'_>| {
                tx.execute(
                    "INSERT INTO book_download
                        (work, chapter, source_id, version_label, state, keep_ready,
                         created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?6, ?6)
                     ON CONFLICT(work, chapter) DO UPDATE SET source_id = excluded.source_id,
                        version_label = excluded.version_label, state = 'queued',
                        path = NULL, pages_done = 0, pages_total = NULL, bytes = 0,
                        error = NULL, keep_ready = excluded.keep_ready,
                        owner = NULL, lease_until = 0, updated_at = excluded.updated_at",
                    params![
                        ask.work,
                        ask.chapter,
                        ask.source_id,
                        ask.version_label,
                        i64::from(ask.keep_ready),
                        now
                    ],
                )
            };
            match had {
                None => {
                    fresh(tx)?;
                    Ok(Queued::Added)
                }
                Some((state, _)) if state == "removed" => {
                    if ask.keep_ready {
                        return Ok(Queued::Skipped);
                    }
                    fresh(tx)?;
                    Ok(Queued::Added)
                }
                Some((state, _)) if state == "failed" => {
                    fresh(tx)?;
                    Ok(Queued::Added)
                }
                Some(_) => {
                    if !ask.keep_ready {
                        tx.execute(
                            "UPDATE book_download SET keep_ready = 0
                              WHERE work = ?1 AND chapter = ?2",
                            params![ask.work, ask.chapter],
                        )?;
                    }
                    Ok(Queued::Had)
                }
            }
        })
    }

    /// A row with its file's whole path, in the data folder as it is now.
    fn whole(&self, mut row: BookDownload) -> BookDownload {
        row.path = row.path.map(|p| self.full_path(p));
        row
    }

    /// The row for one chapter.
    pub fn book_download(&self, work: &str, chapter: &str) -> Result<Option<BookDownload>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                &format!(
                    "SELECT {COLUMNS} FROM book_download WHERE work = ?1 AND chapter = ?2"
                ),
                params![work, chapter],
                row_of,
            )
            .optional()?
            .map(|r| self.whole(r)))
        })
    }

    /// Every download of a work, or of all works, oldest first; marks of chapters let go of are
    /// not downloads and are left out.
    pub fn book_downloads(&self, work: Option<&str>) -> Result<Vec<BookDownload>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {COLUMNS} FROM book_download
                  WHERE state <> 'removed' AND (?1 IS NULL OR work = ?1) ORDER BY id"
            ))?;
            Ok(stmt
                .query_map([work], row_of)?
                .map(|r| r.map(|r| self.whole(r)))
                .collect::<rusqlite::Result<_>>()?)
        })
    }

    /// The marks of chapters kept ready that were let go of by hand.
    pub fn book_downloads_let_go(&self, work: &str) -> Result<Vec<String>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT chapter FROM book_download WHERE work = ?1 AND state = 'removed'",
            )?;
            Ok(stmt.query_map([work], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
        })
    }

    /// The oldest chapter waiting to be fetched.
    pub fn next_book_download(&self) -> Result<Option<BookDownload>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                &format!(
                    "SELECT {COLUMNS} FROM book_download WHERE state = 'queued'
                      ORDER BY keep_ready, id LIMIT 1"
                ),
                [],
                row_of,
            )
            .optional()?
            .map(|r| self.whole(r)))
        })
    }

    /// Claim the oldest chapter waiting to be fetched, for this engine: it is then downloading,
    /// and no other engine on the database takes it while this one renews the claim. A chapter
    /// whose fetch another engine left without word for longer than [`LEASE_SECS`] is waiting
    /// too: that engine has stopped.
    pub fn claim_book_download(&self, now: i64) -> Result<Option<BookDownload>> {
        let owner = self.owner().to_owned();
        let claimed = self.with_tx(|tx| {
            let next = tx
                .query_row(
                    &format!(
                        "SELECT {COLUMNS} FROM book_download
                          WHERE state = 'queued'
                             OR (state = 'downloading' AND {lapsed})
                          ORDER BY keep_ready, id LIMIT 1",
                        lapsed = lapsed("?1")
                    ),
                    params![now, LEASE_SECS],
                    row_of,
                )
                .optional()?;
            let Some(mut row) = next else { return Ok(None) };
            tx.execute(
                "UPDATE book_download SET state = 'downloading', owner = ?2,
                    lease_until = ?3 + ?4, updated_at = ?3 WHERE id = ?1",
                params![row.id, owner, now, LEASE_SECS],
            )?;
            row.state = BookDownloadState::Downloading;
            Ok(Some(row))
        })?;
        Ok(claimed.map(|r| self.whole(r)))
    }

    /// This engine still fetches chapter `id`: its claim goes on for [`LEASE_SECS`] more.
    pub fn book_download_heartbeat(&self, id: i64, now: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE book_download SET lease_until = ?3 + ?4
                  WHERE id = ?1 AND owner = ?2 AND state = 'downloading'",
                params![id, self.owner(), now, LEASE_SECS],
            )?;
            Ok(())
        })
    }

    /// The chapters another engine on the database is fetching now, each with when its claim
    /// runs out unless that engine renews it.
    pub fn book_downloads_held_elsewhere(&self, now: i64) -> Result<Vec<(i64, i64)>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT id, lease_until FROM book_download
                  WHERE state = 'downloading' AND owner IS NOT NULL AND owner <> ?3
                    AND NOT {lapsed}",
                lapsed = lapsed("?1")
            ))?;
            let rows = stmt.query_map(params![now, LEASE_SECS, self.owner()], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    /// Whether another engine on the database has chapter `id` now: it fetches it under a claim
    /// that has not lapsed, or has fetched it. The files of such a chapter are that engine's.
    pub fn book_download_taken(&self, id: i64, now: i64) -> Result<bool> {
        self.with_conn(|c| {
            Ok(c.query_row(
                &format!(
                    "SELECT EXISTS (SELECT 1 FROM book_download WHERE id = ?1 AND (state = 'done'
                        OR (state = 'downloading' AND owner IS NOT NULL AND owner <> ?3
                            AND NOT {lapsed})))",
                    lapsed = lapsed("?4")
                ),
                params![id, LEASE_SECS, self.owner(), now],
                |r| r.get(0),
            )?)
        })
    }

    /// A fetch began, or moved on by a page: the claim on it is renewed. A chapter another
    /// engine fetches is left as it is.
    pub fn book_download_progress(
        &self,
        id: i64,
        done: u32,
        total: Option<u32>,
        now: i64,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE book_download SET state = 'downloading', pages_done = ?2,
                    pages_total = ?3, updated_at = ?4, owner = ?5, lease_until = ?4 + ?6
                  WHERE id = ?1 AND state <> 'removed' AND (owner IS NULL OR owner = ?5)",
                params![id, done, total, now, self.owner(), LEASE_SECS],
            )?;
            Ok(())
        })
    }

    /// The file is whole.
    pub fn book_download_done(
        &self,
        id: i64,
        path: &str,
        pages: Option<u32>,
        bytes: u64,
        now: i64,
    ) -> Result<()> {
        let kept = self.kept_path(path);
        self.with_conn(|c| {
            c.execute(
                "UPDATE book_download SET state = 'done', path = ?2, pages_total = ?3,
                    pages_done = COALESCE(?3, pages_done), bytes = ?4, error = NULL,
                    owner = NULL, lease_until = 0, updated_at = ?5 WHERE id = ?1",
                params![id, kept, pages, i64::try_from(bytes).unwrap_or(i64::MAX), now],
            )?;
            Ok(())
        })
    }

    /// The file is whole, for a fetch still wanted: false, and nothing changes, when the row
    /// was cancelled or deleted while it was being fetched, or is another engine's now.
    pub fn book_download_fetched(
        &self,
        id: i64,
        path: &str,
        pages: Option<u32>,
        bytes: u64,
        now: i64,
    ) -> Result<bool> {
        let kept = self.kept_path(path);
        self.with_conn(|c| {
            let changed = c.execute(
                "UPDATE book_download SET state = 'done', path = ?2, pages_total = ?3,
                    pages_done = COALESCE(?3, pages_done), bytes = ?4, error = NULL,
                    owner = NULL, lease_until = 0, updated_at = ?5
                  WHERE id = ?1 AND state = 'downloading' AND (owner IS NULL OR owner = ?6)",
                params![
                    id,
                    kept,
                    pages,
                    i64::try_from(bytes).unwrap_or(i64::MAX),
                    now,
                    self.owner()
                ],
            )?;
            Ok(changed > 0)
        })
    }

    /// The fetch failed for `error`. A chapter another engine fetches now is left as it is.
    pub fn book_download_failed(&self, id: i64, error: &str, now: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE book_download SET state = 'failed', error = ?2, updated_at = ?3,
                    owner = NULL, lease_until = 0
                  WHERE id = ?1 AND (owner IS NULL OR owner = ?4)",
                params![id, error, now, self.owner()],
            )?;
            Ok(())
        })
    }

    /// Put a fetch back in the queue, as when the network went away under it.
    pub fn book_download_requeue(&self, id: i64, now: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE book_download SET state = 'queued', pages_done = 0, updated_at = ?2,
                    owner = NULL, lease_until = 0
                  WHERE id = ?1 AND state = 'downloading' AND (owner IS NULL OR owner = ?3)",
                params![id, now, self.owner()],
            )?;
            Ok(())
        })
    }

    /// Fetch a finished chapter again, as when its source has added a part to it since. Its
    /// file stays named on the row until the new one replaces it, and the shelf no longer lists
    /// it meanwhile, so the chapter reads from its source.
    pub fn book_download_refetch(&self, work: &str, chapter: &str, now: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE book_download SET state = 'queued', pages_done = 0, pages_total = NULL,
                        updated_at = ?3
                  WHERE work = ?1 AND chapter = ?2 AND state = 'done'",
                params![work, chapter, now],
            )?;
            Ok(())
        })
    }

    /// Fetches the last run left half done go back in the queue. Returns how many. One another
    /// engine on the database still makes, and says so, stays its own.
    pub fn book_downloads_recover(&self, now: i64) -> Result<usize> {
        self.with_conn(|c| {
            Ok(c.execute(
                &format!(
                    "UPDATE book_download SET state = 'queued', pages_done = 0, updated_at = ?1,
                        owner = NULL, lease_until = 0
                      WHERE state = 'downloading'
                        AND (owner IS NULL OR owner = ?3 OR {lapsed})",
                    lapsed = lapsed("?1")
                ),
                params![now, LEASE_SECS, self.owner()],
            )?)
        })
    }

    /// Take a chapter waiting only to be kept ready out of the queue, in one step, as
    /// [`Self::remove_book_downloads`] would: true when it was taken. One the queue has started,
    /// or one the person asked for, stays.
    pub fn unqueue_kept_ready(&self, work: &str, chapter: &str, now: i64) -> Result<bool> {
        self.with_conn(|c| {
            let taken = c.execute(
                "UPDATE book_download SET state = 'removed', path = NULL, bytes = 0,
                    pages_done = 0, error = NULL, updated_at = ?3
                  WHERE work = ?1 AND chapter = ?2 AND keep_ready = 1 AND state = 'queued'",
                params![work, chapter, now],
            )?;
            Ok(taken > 0)
        })
    }

    /// Take chapters out. A chapter fetched ahead on its own leaves a mark so it is not fetched
    /// again; any other row goes. With `chapters` empty, all of the work's. Returns the rows as
    /// they were, so their files can go.
    pub fn remove_book_downloads(
        &self,
        work: &str,
        chapters: &[String],
        only_done: bool,
        now: i64,
    ) -> Result<Vec<BookDownload>> {
        self.with_tx(|tx| {
            let mut stmt = tx.prepare(&format!(
                "SELECT {COLUMNS} FROM book_download WHERE work = ?1 AND state <> 'removed'"
            ))?;
            let rows: Vec<BookDownload> = stmt
                .query_map([work], row_of)?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .map(|r| self.whole(r))
                .filter(|r| chapters.is_empty() || chapters.contains(&r.chapter))
                .filter(|r| !only_done || r.state == BookDownloadState::Done)
                .collect();
            drop(stmt);
            for row in &rows {
                if row.keep_ready {
                    tx.execute(
                        "UPDATE book_download SET state = 'removed', path = NULL, bytes = 0,
                            pages_done = 0, error = NULL, updated_at = ?2 WHERE id = ?1",
                        params![row.id, now],
                    )?;
                } else {
                    tx.execute("DELETE FROM book_download WHERE id = ?1", [row.id])?;
                }
            }
            Ok(rows)
        })
    }

    /// Take a read chapter's download out for good, the mark of one kept ready too: reading
    /// past it is not letting go by hand.
    pub fn drop_book_download(&self, id: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM book_download WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// What finished downloads take up: the bytes, and how many chapters.
    pub fn book_download_totals(&self) -> Result<(u64, u32)> {
        self.with_conn(|c| {
            let (bytes, count): (i64, i64) = c.query_row(
                "SELECT COALESCE(SUM(bytes), 0), COUNT(*) FROM book_download
                  WHERE state = 'done'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            Ok((u64::try_from(bytes).unwrap_or(0), u32::try_from(count).unwrap_or(u32::MAX)))
        })
    }

    /// The works with downloads of a kind: `manga` or `novel` as the catalogue has them.
    pub fn book_download_works_of(&self, medium: &str) -> Result<Vec<String>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT DISTINCT d.work FROM book_download d
                   JOIN book_work w ON w.work = d.work WHERE w.medium = ?1",
            )?;
            Ok(stmt.query_map([medium], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
        })
    }

    /// Every work with a download row.
    pub fn book_download_works(&self) -> Result<Vec<String>> {
        self.with_conn(|c| {
            let mut stmt =
                c.prepare("SELECT DISTINCT work FROM book_download WHERE state <> 'removed'")?;
            Ok(stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn ask<'a>(chapter: &'a str, keep_ready: bool) -> BookDownloadAsk<'a> {
        BookDownloadAsk {
            work: "a:900401",
            chapter,
            source_id: "shelf",
            version_label: None,
            keep_ready,
        }
    }

    #[test]
    fn a_chapter_is_queued_once_and_a_failed_one_again() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.queue_book_download(&ask("1", false), 10).unwrap(), Queued::Added);
        assert_eq!(store.queue_book_download(&ask("1", false), 11).unwrap(), Queued::Had);
        let row = store.next_book_download().unwrap().unwrap();
        store.book_download_failed(row.id, "offline", 12).unwrap();
        assert!(store.next_book_download().unwrap().is_none());
        assert_eq!(store.queue_book_download(&ask("1", false), 13).unwrap(), Queued::Added);
        let again = store.book_download("a:900401", "1").unwrap().unwrap();
        assert_eq!((again.state, again.error), (BookDownloadState::Queued, None));
    }

    #[test]
    fn only_a_kept_ready_row_still_queued_is_unqueued() {
        let store = Store::open_in_memory().unwrap();
        store.queue_book_download(&ask("1", true), 1).unwrap();
        store.queue_book_download(&ask("2", true), 1).unwrap();
        store.queue_book_download(&ask("3", false), 1).unwrap();
        let first = store.next_book_download().unwrap().unwrap();
        store.book_download_progress(first.id, 0, None, 2).unwrap();
        assert_eq!(first.chapter, "3", "asked rows first");
        assert!(!store.unqueue_kept_ready("a:900401", "3", 3).unwrap(), "asked for");
        let started = store.book_download("a:900401", "1").unwrap().unwrap();
        store.book_download_progress(started.id, 0, None, 3).unwrap();
        assert!(!store.unqueue_kept_ready("a:900401", "1", 4).unwrap(), "started");
        assert!(store.unqueue_kept_ready("a:900401", "2", 4).unwrap());
        assert!(store.book_downloads(None).unwrap().iter().all(|d| d.chapter != "2"));
        assert_eq!(store.book_downloads_let_go("a:900401").unwrap(), ["2"]);
    }

    #[test]
    fn by_hand_takes_a_kept_ready_row_over_and_a_let_go_mark_holds_back_only_keeping() {
        let store = Store::open_in_memory().unwrap();
        store.queue_book_download(&ask("2", true), 1).unwrap();
        assert!(store.book_download("a:900401", "2").unwrap().unwrap().keep_ready);
        assert_eq!(store.queue_book_download(&ask("2", false), 2).unwrap(), Queued::Had);
        assert!(!store.book_download("a:900401", "2").unwrap().unwrap().keep_ready);

        store.queue_book_download(&ask("3", true), 3).unwrap();
        let gone = store.remove_book_downloads("a:900401", &["3".into()], false, 4).unwrap();
        assert_eq!(gone.len(), 1);
        assert!(store.book_downloads(None).unwrap().iter().all(|d| d.chapter != "3"));
        assert_eq!(store.book_downloads_let_go("a:900401").unwrap(), ["3"]);
        assert_eq!(store.queue_book_download(&ask("3", true), 5).unwrap(), Queued::Skipped);
        assert_eq!(store.queue_book_download(&ask("3", false), 6).unwrap(), Queued::Added);
        assert!(store.book_downloads_let_go("a:900401").unwrap().is_empty());
    }

    #[test]
    fn totals_count_finished_downloads_and_a_crash_requeues_the_rest() {
        let store = Store::open_in_memory().unwrap();
        store.queue_book_download(&ask("1", false), 1).unwrap();
        store.queue_book_download(&ask("2", false), 1).unwrap();
        let first = store.next_book_download().unwrap().unwrap();
        store.book_download_done(first.id, "/dl/1.cbz", Some(12), 4_000, 2).unwrap();
        let second = store.next_book_download().unwrap().unwrap();
        store.book_download_progress(second.id, 3, Some(9), 3).unwrap();
        assert_eq!(store.book_download_totals().unwrap(), (4_000, 1));
        assert_eq!(store.book_downloads_recover(4).unwrap(), 1);
        let again = store.book_download("a:900401", "2").unwrap().unwrap();
        assert_eq!((again.state, again.pages_done), (BookDownloadState::Queued, 0));
        // The shelf reads the finished one.
        let shelf = store.shelf("a:900401").unwrap();
        assert_eq!(shelf.len(), 1);
        assert_eq!(shelf[0].path, "/dl/1.cbz");
    }

    #[test]
    fn a_download_is_found_again_after_the_data_folder_moves() {
        let root = tempfile::tempdir().unwrap();
        let (before, after) = (root.path().join("before"), root.path().join("after"));
        let file = before.join("book-downloads").join("work").join("1.cbz");
        {
            let store = Store::open(before.join("kaeri.db")).unwrap();
            store.queue_book_download(&ask("1", false), 1).unwrap();
            let row = store.next_book_download().unwrap().unwrap();
            store.book_download_done(row.id, &file.to_string_lossy(), Some(4), 100, 2).unwrap();
            let kept: String = store
                .with_conn(|c| {
                    Ok(c.query_row("SELECT path FROM book_download", [], |r| r.get(0))?)
                })
                .unwrap();
            assert!(Path::new(&kept).is_relative(), "kept as {kept}");
        }
        std::fs::rename(&before, &after).unwrap();
        let store = Store::open(after.join("kaeri.db")).unwrap();
        let moved = after.join("book-downloads").join("work").join("1.cbz");
        let row = store.book_download("a:900401", "1").unwrap().unwrap();
        assert_eq!(row.path.as_deref(), Some(&*moved.to_string_lossy()));
        assert_eq!(store.shelf("a:900401").unwrap()[0].path, moved.to_string_lossy());
        assert_eq!(store.book_downloads(None).unwrap()[0].path, row.path);
    }

    #[test]
    fn works_are_told_apart_by_the_catalogue_s_medium() {
        let store = Store::open_in_memory().unwrap();
        store.queue_book_download(&ask("1", false), 1).unwrap();
        assert!(store.book_download_works_of("manga").unwrap().is_empty());
        store.seed_book_work("a:900401", "manga", None, None, "Moth Garden", 1).unwrap();
        assert_eq!(store.book_download_works_of("manga").unwrap(), ["a:900401"]);
        assert!(store.book_download_works_of("novel").unwrap().is_empty());
        assert_eq!(store.book_download_works().unwrap(), ["a:900401"]);
    }

    /// Two engines on one database, each with its own store, as an app and the terminal.
    fn two_engines() -> (tempfile::TempDir, Store, Store) {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("kaeri.db");
        let first = Store::open(&db).unwrap();
        let second = Store::open(&db).unwrap();
        (dir, first, second)
    }

    #[test]
    fn a_chapter_one_engine_claimed_is_not_the_other_engines_while_the_claim_lasts() {
        let (_dir, a, b) = two_engines();
        a.queue_book_download(&ask("1", false), 1).unwrap();
        a.queue_book_download(&ask("2", false), 1).unwrap();
        let mine = a.claim_book_download(100).unwrap().unwrap();
        assert_eq!((mine.chapter.as_str(), mine.state), ("1", BookDownloadState::Downloading));
        let theirs = b.claim_book_download(100).unwrap().unwrap();
        assert_eq!(theirs.chapter, "2", "the other engine takes the next one");
        // It fetches that one at once, so its own claim does not lapse below.
        assert!(b.book_download_fetched(theirs.id, "/dl/2.cbz", Some(9), 1, 100).unwrap());
        assert!(b.claim_book_download(100).unwrap().is_none());
        assert_eq!(
            b.book_downloads_held_elsewhere(100).unwrap(),
            [(mine.id, 100 + LEASE_SECS)]
        );
        assert!(b.book_download_taken(mine.id, 100).unwrap());
        assert!(!a.book_download_taken(mine.id, 100).unwrap());

        // Another engine's progress, failure or requeue leaves the row as its owner has it.
        b.book_download_progress(mine.id, 2, Some(9), 101).unwrap();
        b.book_download_failed(mine.id, "offline", 101).unwrap();
        b.book_download_requeue(mine.id, 101).unwrap();
        assert!(!b.book_download_fetched(mine.id, "/dl/1.cbz", Some(9), 1, 101).unwrap());
        let row = a.book_download("a:900401", "1").unwrap().unwrap();
        assert_eq!((row.state, row.pages_done), (BookDownloadState::Downloading, 0));

        // Its owner renews the claim with each page and between them.
        a.book_download_progress(mine.id, 3, Some(9), 130).unwrap();
        a.book_download_heartbeat(mine.id, 160).unwrap();
        assert!(b.claim_book_download(200).unwrap().is_none(), "renewed at 160");
        assert!(a.book_download_fetched(mine.id, "/dl/1.cbz", Some(9), 1, 201).unwrap());
        assert!(b.book_downloads_held_elsewhere(201).unwrap().is_empty());
    }

    #[test]
    fn a_claim_its_engine_stopped_renewing_lapses_and_is_taken_over() {
        let (_dir, a, b) = two_engines();
        a.queue_book_download(&ask("1", false), 1).unwrap();
        let mine = a.claim_book_download(100).unwrap().unwrap();
        // The other engine starts while the claim lasts: its launch leaves the row alone.
        assert_eq!(b.book_downloads_recover(110).unwrap(), 0);
        assert_eq!(
            b.book_download("a:900401", "1").unwrap().unwrap().state,
            BookDownloadState::Downloading
        );
        // The first engine was killed: nothing renews the claim, and once it lapses the other
        // engine takes the chapter, at launch or from its queue.
        let late = 100 + LEASE_SECS + 1;
        assert!(!b.book_download_taken(mine.id, late).unwrap());
        assert!(b.book_downloads_held_elsewhere(late).unwrap().is_empty());
        let taken = b.claim_book_download(late).unwrap().unwrap();
        assert_eq!(taken.id, mine.id);
        assert!(a.book_download_taken(mine.id, late).unwrap(), "it is the other engine's now");
        assert_eq!(a.book_downloads_recover(late).unwrap(), 0);

        // A claim set while the clock ran a year ahead lapses once it is put right.
        b.book_download_progress(mine.id, 0, None, late + 31_536_000).unwrap();
        assert!(a.claim_book_download(late + 1).unwrap().is_some());
    }

    #[test]
    fn a_crash_requeues_this_engines_fetches_and_those_of_none() {
        let (_dir, a, b) = two_engines();
        a.queue_book_download(&ask("1", false), 1).unwrap();
        a.queue_book_download(&ask("2", false), 1).unwrap();
        let ours = a.claim_book_download(100).unwrap().unwrap();
        b.claim_book_download(100).unwrap().unwrap();
        // A row an earlier version left downloading has no owner.
        a.with_conn(|c| {
            c.execute("UPDATE book_download SET owner = NULL WHERE id = ?1", [ours.id])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(a.book_downloads_recover(101).unwrap(), 1);
        let states: Vec<_> =
            a.book_downloads(None).unwrap().into_iter().map(|r| (r.chapter, r.state)).collect();
        assert_eq!(
            states,
            [
                ("1".to_owned(), BookDownloadState::Queued),
                ("2".to_owned(), BookDownloadState::Downloading)
            ]
        );
    }
}
