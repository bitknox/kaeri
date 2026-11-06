//! The reading catalogue as this device last heard it: each work's facts from AniList, when its
//! chapters usually come out, each source's chapter list, and the chapters already told about.
//!
//! `book_work`, `book_rhythm` and `book_chapter_cache` stay on the device; `read_notice` is
//! personal, as `episode_notice` is, so a profile is never told twice of one chapter.

use rusqlite::{OptionalExtension, params};

use crate::{Result, Store};

/// A work's facts, as the catalogue last answered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BookWork {
    pub work: String,
    pub anilist_id: Option<u32>,
    /// `manga` or `novel`.
    pub medium: String,
    /// `manga`, `webtoon`, `one-shot`, `novel`.
    pub format: Option<String>,
    /// An ISO country code: `JP`, `KR`.
    pub origin: Option<String>,
    /// JSON: the names.
    pub titles: String,
    pub cover: Option<String>,
    /// AniList's: `RELEASING`, `FINISHED`.
    pub status: Option<String>,
    pub chapters_total: Option<u32>,
    pub volumes_total: Option<u32>,
    /// JSON: the rest of what the catalogue said.
    pub meta: Option<String>,
    pub fetched_at: i64,
}

/// When a work's chapters usually come out, learnt from the dates of the chapters seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BookRhythm {
    /// From Monday as 0, when they come out on one day of the week.
    pub weekday: Option<u8>,
    /// The usual days between two chapters.
    pub every_days: Option<u32>,
    /// The newest chapter's date, Unix seconds.
    pub last_published: Option<i64>,
    pub checked_at: i64,
}

const WORK_COLUMNS: &str = "work, anilist_id, medium, format, origin, titles, cover, status,
    chapters_total, volumes_total, meta, fetched_at";

fn row_to_work(r: &rusqlite::Row<'_>) -> rusqlite::Result<BookWork> {
    Ok(BookWork {
        work: r.get(0)?,
        anilist_id: r.get(1)?,
        medium: r.get(2)?,
        format: r.get(3)?,
        origin: r.get(4)?,
        titles: r.get(5)?,
        cover: r.get(6)?,
        status: r.get(7)?,
        chapters_total: r.get(8)?,
        volumes_total: r.get(9)?,
        meta: r.get(10)?,
        fetched_at: r.get(11)?,
    })
}

impl Store {
    /// Keep a work's facts, replacing what was kept.
    pub fn put_book_work(&self, work: &BookWork) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                &format!(
                    "INSERT OR REPLACE INTO book_work ({WORK_COLUMNS})
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
                ),
                params![
                    work.work,
                    work.anilist_id,
                    work.medium,
                    work.format,
                    work.origin,
                    work.titles,
                    work.cover,
                    work.status,
                    work.chapters_total,
                    work.volumes_total,
                    work.meta,
                    work.fetched_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn book_work(&self, work: &str) -> Result<Option<BookWork>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                &format!("SELECT {WORK_COLUMNS} FROM book_work WHERE work = ?1"),
                [work],
                row_to_work,
            )
            .optional()?)
        })
    }

    /// The works kept of `works`, in no order.
    pub fn book_works(&self, works: &[String]) -> Result<Vec<BookWork>> {
        if works.is_empty() {
            return Ok(Vec::new());
        }
        self.with_conn(|c| {
            let mut stmt =
                c.prepare(&format!("SELECT {WORK_COLUMNS} FROM book_work WHERE work = ?1"))?;
            let mut found = Vec::with_capacity(works.len());
            for work in works {
                if let Some(row) = stmt.query_row([work], row_to_work).optional()? {
                    found.push(row);
                }
            }
            Ok(found)
        })
    }

    pub fn set_book_rhythm(&self, work: &str, rhythm: &BookRhythm) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO book_rhythm
                    (work, weekday, every_days, last_published, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    work,
                    rhythm.weekday,
                    rhythm.every_days,
                    rhythm.last_published,
                    rhythm.checked_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn book_rhythm(&self, work: &str) -> Result<Option<BookRhythm>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT weekday, every_days, last_published, checked_at FROM book_rhythm
                  WHERE work = ?1",
                [work],
                |r| {
                    Ok(BookRhythm {
                        weekday: r.get(0)?,
                        every_days: r.get(1)?,
                        last_published: r.get(2)?,
                        checked_at: r.get(3)?,
                    })
                },
            )
            .optional()?)
        })
    }

    /// Keep a source's chapter list for a series: JSON, as the engine writes it.
    pub fn put_book_chapters(
        &self,
        source_id: &str,
        series_ref: &str,
        chapters: &str,
        at: i64,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO book_chapter_cache (source_id, series_ref, chapters,
                    fetched_at) VALUES (?1, ?2, ?3, ?4)",
                params![source_id, series_ref, chapters, at],
            )?;
            Ok(())
        })
    }

    /// A source's chapter list for a series as kept, and when it was asked.
    pub fn book_chapters(
        &self,
        source_id: &str,
        series_ref: &str,
    ) -> Result<Option<(String, i64)>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT chapters, fetched_at FROM book_chapter_cache
                  WHERE source_id = ?1 AND series_ref = ?2",
                [source_id, series_ref],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
        })
    }

    /// Whether this profile was told of a chapter already.
    pub fn chapter_noticed(&self, work: &str, chapter: &str) -> Result<bool> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT 1 FROM read_notice WHERE work = ?1 AND chapter = ?2",
                [work, chapter],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
        })
    }

    /// Note that this profile was told of a chapter, and say whether it was new.
    pub fn notice_chapter(&self, work: &str, chapter: &str, at: i64) -> Result<bool> {
        self.with_personal_tx(|tx| {
            let added = tx.execute(
                "INSERT OR IGNORE INTO read_notice (work, chapter, notified_at)
                 VALUES (?1, ?2, ?3)",
                params![work, chapter, at],
            )?;
            Ok(added > 0)
        })
    }

    /// The works on one reading list, by the model's status name, newest change first.
    pub fn read_status_works(&self, status: &str) -> Result<Vec<String>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT work FROM read_status WHERE status = ?1 ORDER BY updated_at DESC, work",
            )?;
            Ok(stmt.query_map([status], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
        })
    }

    /// Every work with a reading list, and the list it is on.
    pub fn read_statuses(&self) -> Result<Vec<(String, String)>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT work, status FROM read_status WHERE status IS NOT NULL
                  ORDER BY updated_at DESC, work",
            )?;
            Ok(stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn works_rhythms_chapter_lists_and_notices_are_kept() {
        let store = Store::open_in_memory().unwrap();
        let work = BookWork {
            work: "a:900301".into(),
            anilist_id: Some(900_301),
            medium: "manga".into(),
            format: Some("manga".into()),
            origin: Some("JP".into()),
            titles: r#"{"display":"The Lantern Ferry"}"#.into(),
            status: Some("RELEASING".into()),
            chapters_total: None,
            fetched_at: 10,
            ..BookWork::default()
        };
        store.put_book_work(&work).unwrap();
        assert_eq!(store.book_work("a:900301").unwrap(), Some(work.clone()));
        let missing = ["a:900301".to_owned(), "a:1".to_owned()];
        assert_eq!(store.book_works(&missing).unwrap(), vec![work]);

        let rhythm = BookRhythm {
            weekday: Some(4),
            every_days: Some(7),
            last_published: Some(100),
            checked_at: 200,
        };
        store.set_book_rhythm("a:900301", &rhythm).unwrap();
        assert_eq!(store.book_rhythm("a:900301").unwrap(), Some(rhythm));

        store.put_book_chapters("shelf", "s-1", "[]", 5).unwrap();
        assert_eq!(store.book_chapters("shelf", "s-1").unwrap(), Some(("[]".into(), 5)));

        assert!(!store.chapter_noticed("a:900301", "13").unwrap());
        assert!(store.notice_chapter("a:900301", "13", 1).unwrap());
        assert!(!store.notice_chapter("a:900301", "13", 2).unwrap());
        assert!(store.chapter_noticed("a:900301", "13").unwrap());

        store.set_read_status("a:900302", Some("planning"), 1).unwrap();
        store.set_read_status("a:900303", Some("reading"), 2).unwrap();
        assert_eq!(store.read_status_works("planning").unwrap(), vec!["a:900302".to_owned()]);
        assert_eq!(store.read_statuses().unwrap().len(), 2);
    }
}
