//! The reading catalogue as this device last heard it: each work's facts from AniList, when its
//! chapters usually come out, each source's chapter list and word on how a series reads, and the
//! chapters already told about.
//!
//! `book_work`, `book_rhythm`, `book_chapter_cache` and `book_series_hint` stay on the device;
//! `read_notice` is personal, as `episode_notice` is, so a profile is never told twice of one
//! chapter.

use std::collections::HashSet;

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

    /// Keep `title` as the name of a work of its own on this device (`l:`: a folder's series, or
    /// a book server's that no catalogue lists), and `cover` as its cover when one is given.
    /// Screens, downloads and sends read a work's name here. A work the catalogue describes keeps
    /// the catalogue's row. Says whether anything changed.
    pub fn name_local_work(
        &self,
        work: &str,
        medium: &str,
        title: &str,
        cover: Option<&str>,
        now: i64,
    ) -> Result<bool> {
        let titles = serde_json::to_string(&[title]).unwrap_or_else(|_| "[]".to_owned());
        self.with_conn(|c| {
            let changed = c.execute(
                "INSERT INTO book_work (work, medium, titles, cover, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(work) DO UPDATE SET titles = excluded.titles,
                    cover = COALESCE(excluded.cover, book_work.cover),
                    fetched_at = excluded.fetched_at
                  WHERE book_work.anilist_id IS NULL
                    AND (book_work.titles IS NOT excluded.titles
                         OR (excluded.cover IS NOT NULL
                             AND book_work.cover IS NOT excluded.cover))",
                params![work, medium, titles, cover, now],
            )?;
            Ok(changed > 0)
        })
    }

    /// The works of their own on this device (`l:`) that have no name kept yet and were read,
    /// are being read or were downloaded, each with a source's series matched to it: the most
    /// sure match first.
    pub fn unnamed_local_works(&self) -> Result<Vec<(String, String, String)>> {
        // Names and downloads are the device's; matches and reading are the person's.
        let (named, downloaded) = self.with_conn(|c| {
            let works = |sql: &str| -> Result<HashSet<String>> {
                let mut stmt = c.prepare(sql)?;
                let rows = stmt.query_map([], |r| r.get(0))?;
                Ok(rows.collect::<rusqlite::Result<_>>()?)
            };
            Ok((
                works("SELECT work FROM book_work WHERE work LIKE 'l:%'")?,
                works("SELECT DISTINCT work FROM book_download WHERE work LIKE 'l:%'")?,
            ))
        })?;
        let matched: Vec<(String, String, String, bool)> = self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT m.work, m.source_id, m.series_ref,
                        EXISTS (SELECT 1 FROM read_work r WHERE r.work = m.work)
                     OR EXISTS (SELECT 1 FROM read_chapter r WHERE r.work = m.work)
                     OR EXISTS (SELECT 1 FROM read_status r WHERE r.work = m.work)
                   FROM book_source_match m
                  WHERE m.work LIKE 'l:%'
                  ORDER BY m.work, m.confirmed DESC, m.confidence DESC, m.updated_at DESC",
            )?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })?;
        Ok(matched
            .into_iter()
            .filter(|(work, _, _, read)| {
                !named.contains(work) && (*read || downloaded.contains(work))
            })
            .map(|(work, source, series, _)| (work, source, series))
            .collect())
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

    /// Keep how a source says a series' pages read: `None` when it said nothing.
    pub fn put_book_series_hint(
        &self,
        source_id: &str,
        series_ref: &str,
        hint: Option<&str>,
        at: i64,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO book_series_hint (source_id, series_ref, hint, fetched_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![source_id, series_ref, hint, at],
            )?;
            Ok(())
        })
    }

    /// How a source last said a series' pages read, and when it was asked: `None` when it was
    /// never asked.
    pub fn book_series_hint(
        &self,
        source_id: &str,
        series_ref: &str,
    ) -> Result<Option<(Option<String>, i64)>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT hint, fetched_at FROM book_series_hint
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
    use crate::shelf::TitleForm;

    /// R7E-4. A work of its own on this device is named as its source names it, renamed when
    /// the source does, and never over the catalogue's row.
    #[test]
    fn a_local_work_is_named_and_the_catalogue_row_is_left_alone() {
        let store = Store::open_in_memory().unwrap();
        let title = |work: &str| {
            store.book_work_kind(work).unwrap().and_then(|k| k.title_in(TitleForm::English))
        };
        assert!(
            store.name_local_work("l:m:ferry", "manga", "The Long Ferry", None, 1).unwrap()
        );
        assert_eq!(title("l:m:ferry").as_deref(), Some("The Long Ferry"));
        assert!(
            !store.name_local_work("l:m:ferry", "manga", "The Long Ferry", None, 2).unwrap()
        );
        let cover = Some("https://books.example/cover/1");
        assert!(
            store.name_local_work("l:m:ferry", "manga", "The Long Ferry", cover, 3).unwrap()
        );
        assert!(store.name_local_work("l:m:ferry", "manga", "Long Ferry", None, 4).unwrap());
        let kept = store.book_work("l:m:ferry").unwrap().unwrap();
        assert_eq!(
            (kept.cover.as_deref(), title("l:m:ferry").as_deref()),
            (cover, Some("Long Ferry"))
        );

        let anilist = BookWork {
            work: "a:900301".into(),
            anilist_id: Some(900_301),
            medium: "manga".into(),
            titles: r#"{"display":"The Lantern Ferry"}"#.into(),
            fetched_at: 10,
            ..BookWork::default()
        };
        store.put_book_work(&anilist).unwrap();
        assert!(
            !store.name_local_work("a:900301", "manga", "Something Else", None, 11).unwrap()
        );
        assert_eq!(store.book_work("a:900301").unwrap(), Some(anilist));
    }

    /// R7E-4. The works to name again at launch: read or downloaded, with no name kept.
    #[test]
    fn unnamed_local_works_are_the_read_and_downloaded_ones() {
        let store = Store::open_in_memory().unwrap();
        for (series, work) in [("s-1", "l:m:read"), ("s-2", "l:m:named"), ("s-3", "l:m:never")]
        {
            store.set_book_match("srv-1", series, work, 50, false, false, 1).unwrap();
        }
        store.set_book_match("srv-1", "s-4", "a:7", 100, true, false, 1).unwrap();
        for work in ["l:m:read", "l:m:named", "a:7"] {
            store.set_read_status(work, Some("reading"), 1).unwrap();
        }
        store.name_local_work("l:m:named", "manga", "Named", None, 1).unwrap();
        assert_eq!(
            store.unnamed_local_works().unwrap(),
            [("l:m:read".to_owned(), "srv-1".to_owned(), "s-1".to_owned())]
        );
    }

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

        // A source's word on how a series reads, and its silence, are both kept.
        assert_eq!(store.book_series_hint("komga", "s-1").unwrap(), None);
        store.put_book_series_hint("komga", "s-1", Some("ltr"), 6).unwrap();
        assert_eq!(
            store.book_series_hint("komga", "s-1").unwrap(),
            Some((Some("ltr".into()), 6))
        );
        store.put_book_series_hint("komga", "s-1", None, 7).unwrap();
        assert_eq!(store.book_series_hint("komga", "s-1").unwrap(), Some((None, 7)));

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
