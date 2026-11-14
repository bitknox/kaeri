//! Where books come from on this device: folders, the series scanned from them, and the book
//! servers. All of it stays on the device. A server's sign-in is in the secure store under
//! `book-server/<id>`, never here.

use rusqlite::{OptionalExtension, params};

use crate::{Result, Store};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Folder {
    pub id: String,
    pub path: Option<String>,
    pub tree_uri: Option<String>,
    pub bookmark: Option<Vec<u8>>,
    pub name: Option<String>,
    pub added_at: i64,
    pub last_scan: Option<i64>,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSeries {
    pub id: String,
    pub folder: String,
    pub name: String,
    pub medium: String,
    /// The work it was matched to, when it was.
    pub work: Option<String>,
    /// `matched`, `unmatched` or `kept`.
    pub state: String,
    pub confidence: u8,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    pub path: String,
    pub series: String,
    pub volume: Option<String>,
    pub label: String,
    pub number_milli: Option<i64>,
    pub format: String,
    pub pages: Option<u32>,
    pub size: i64,
    pub mtime: i64,
    pub comicinfo: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookServer {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub address: String,
    pub user: Option<String>,
    pub report_progress: bool,
    pub signed_out: bool,
    pub last_seen: Option<i64>,
}

impl Store {
    pub fn put_book_folder(&self, folder: &Folder) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO book_folder
                     (id, path, tree_uri, bookmark, name, added_at, last_scan, available)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    path = excluded.path, tree_uri = excluded.tree_uri,
                    bookmark = excluded.bookmark, name = excluded.name,
                    last_scan = excluded.last_scan, available = excluded.available",
                params![
                    folder.id,
                    folder.path,
                    folder.tree_uri,
                    folder.bookmark,
                    folder.name,
                    folder.added_at,
                    folder.last_scan,
                    folder.available
                ],
            )?;
            Ok(())
        })
    }

    pub fn book_folders(&self) -> Result<Vec<Folder>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, path, tree_uri, bookmark, name, added_at, last_scan, available
                   FROM book_folder ORDER BY added_at, id",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(Folder {
                    id: r.get(0)?,
                    path: r.get(1)?,
                    tree_uri: r.get(2)?,
                    bookmark: r.get(3)?,
                    name: r.get(4)?,
                    added_at: r.get(5)?,
                    last_scan: r.get(6)?,
                    available: r.get(7)?,
                })
            })?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        })
    }

    /// Forget a folder with the series and files scanned from it.
    pub fn remove_book_folder(&self, id: &str) -> Result<()> {
        self.with_tx(|tx| {
            tx.execute(
                "DELETE FROM book_local_file WHERE series IN
                    (SELECT id FROM book_local_series WHERE folder = ?1)",
                [id],
            )?;
            tx.execute("DELETE FROM book_local_series WHERE folder = ?1", [id])?;
            tx.execute("DELETE FROM book_folder WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// Replace everything scanned from a folder with a new scan, keeping a series' match and
    /// the person's choice about it when the series is still there.
    pub fn replace_book_scan(
        &self,
        folder: &str,
        series: &[(LocalSeries, Vec<LocalFile>)],
        at: i64,
    ) -> Result<()> {
        self.with_tx(|tx| {
            let kept: Vec<LocalSeries> = {
                let mut stmt = tx.prepare(
                    "SELECT id, work, state, confidence, reason
                       FROM book_local_series WHERE folder = ?1",
                )?;
                stmt.query_map([folder], |r| {
                    Ok(LocalSeries {
                        id: r.get(0)?,
                        folder: folder.to_owned(),
                        name: String::new(),
                        medium: String::new(),
                        work: r.get(1)?,
                        state: r.get(2)?,
                        confidence: r.get(3)?,
                        reason: r.get(4)?,
                    })
                })?
                .collect::<std::result::Result<_, _>>()?
            };
            tx.execute(
                "DELETE FROM book_local_file WHERE series IN
                    (SELECT id FROM book_local_series WHERE folder = ?1)",
                [folder],
            )?;
            tx.execute("DELETE FROM book_local_series WHERE folder = ?1", [folder])?;
            for (s, files) in series {
                let old = kept.iter().find(|k| k.id == s.id);
                let (work, state, confidence, reason) = match old {
                    Some(k) if k.state != "unmatched" => {
                        (k.work.clone(), k.state.clone(), k.confidence, k.reason.clone())
                    }
                    _ => (s.work.clone(), s.state.clone(), s.confidence, s.reason.clone()),
                };
                tx.execute(
                    "INSERT INTO book_local_series
                         (id, folder, name, medium, work, state, confidence, reason)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![s.id, folder, s.name, s.medium, work, state, confidence, reason],
                )?;
                for f in files {
                    tx.execute(
                        "INSERT OR REPLACE INTO book_local_file
                             (path, series, volume, label, number_milli, format, pages, size,
                              mtime, comicinfo)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![
                            f.path,
                            s.id,
                            f.volume,
                            f.label,
                            f.number_milli,
                            f.format,
                            f.pages,
                            f.size,
                            f.mtime,
                            f.comicinfo
                        ],
                    )?;
                }
            }
            tx.execute(
                "UPDATE book_folder SET last_scan = ?2, available = 1 WHERE id = ?1",
                params![folder, at],
            )?;
            Ok(())
        })
    }

    pub fn set_book_folder_available(&self, id: &str, available: bool) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE book_folder SET available = ?2 WHERE id = ?1",
                params![id, available],
            )?;
            Ok(())
        })
    }

    pub fn local_book_series(&self) -> Result<Vec<LocalSeries>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, folder, name, medium, work, state, confidence, reason
                   FROM book_local_series ORDER BY name COLLATE NOCASE, id",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(LocalSeries {
                    id: r.get(0)?,
                    folder: r.get(1)?,
                    name: r.get(2)?,
                    medium: r.get(3)?,
                    work: r.get(4)?,
                    state: r.get(5)?,
                    confidence: r.get(6)?,
                    reason: r.get(7)?,
                })
            })?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        })
    }

    /// How many files each scanned series holds, by series id, in one look.
    pub fn local_book_file_counts(&self) -> Result<std::collections::HashMap<String, u32>> {
        self.with_conn(|c| {
            let mut stmt =
                c.prepare("SELECT series, COUNT(*) FROM book_local_file GROUP BY series")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)))?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        })
    }

    pub fn local_book_files(&self, series: &str) -> Result<Vec<LocalFile>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT path, series, volume, label, number_milli, format, pages, size, mtime,
                        comicinfo
                   FROM book_local_file WHERE series = ?1
                  ORDER BY number_milli IS NULL, number_milli, path",
            )?;
            let rows = stmt.query_map([series], |r| {
                Ok(LocalFile {
                    path: r.get(0)?,
                    series: r.get(1)?,
                    volume: r.get(2)?,
                    label: r.get(3)?,
                    number_milli: r.get(4)?,
                    format: r.get(5)?,
                    pages: r.get(6)?,
                    size: r.get(7)?,
                    mtime: r.get(8)?,
                    comicinfo: r.get(9)?,
                })
            })?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        })
    }

    /// Record the match for a scanned series: `state` is `matched`, `unmatched` or `kept`.
    pub fn set_local_series_match(
        &self,
        series: &str,
        work: Option<&str>,
        state: &str,
        confidence: u8,
        reason: Option<&str>,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE book_local_series
                    SET work = ?2, state = ?3, confidence = ?4, reason = ?5 WHERE id = ?1",
                params![series, work, state, confidence, reason],
            )?;
            Ok(())
        })
    }

    pub fn put_book_server(&self, server: &BookServer) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO book_server
                     (id, kind, name, address, user, report_progress, signed_out, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    kind = excluded.kind, name = excluded.name, address = excluded.address,
                    user = excluded.user, report_progress = excluded.report_progress,
                    signed_out = excluded.signed_out, last_seen = excluded.last_seen",
                params![
                    server.id,
                    server.kind,
                    server.name,
                    server.address,
                    server.user,
                    server.report_progress,
                    server.signed_out,
                    server.last_seen
                ],
            )?;
            Ok(())
        })
    }

    pub fn book_servers(&self) -> Result<Vec<BookServer>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, kind, name, address, user, report_progress, signed_out, last_seen
                   FROM book_server ORDER BY name COLLATE NOCASE, id",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(BookServer {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    name: r.get(2)?,
                    address: r.get(3)?,
                    user: r.get(4)?,
                    report_progress: r.get(5)?,
                    signed_out: r.get(6)?,
                    last_seen: r.get(7)?,
                })
            })?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        })
    }

    /// Say whether a server's sign-in is refused, on its row only: a server that is not kept
    /// is not added. Whether a row changed.
    pub fn set_book_server_signed_out(&self, id: &str, signed_out: bool) -> Result<bool> {
        self.with_conn(|c| {
            let changed = c.execute(
                "UPDATE book_server SET signed_out = ?2 WHERE id = ?1 AND signed_out != ?2",
                params![id, signed_out],
            )?;
            Ok(changed > 0)
        })
    }

    pub fn book_server(&self, id: &str) -> Result<Option<BookServer>> {
        Ok(self.book_servers()?.into_iter().find(|s| s.id == id))
    }

    pub fn remove_book_server(&self, id: &str) -> Result<()> {
        self.with_personal(|c| {
            c.execute("DELETE FROM book_source_match WHERE source_id = ?1", [id])?;
            Ok(())
        })?;
        self.with_tx(|tx| {
            tx.execute("DELETE FROM book_server WHERE id = ?1", [id])?;
            tx.execute("DELETE FROM book_chapter_cache WHERE source_id = ?1", [id])?;
            tx.execute("DELETE FROM book_series_hint WHERE source_id = ?1", [id])?;
            tx.execute("DELETE FROM book_progress_outbox WHERE server = ?1", [id])?;
            tx.execute("DELETE FROM book_progress_state WHERE server = ?1", [id])?;
            Ok(())
        })
    }

    /// The series a source holds for a work, from the matches found: most sure first.
    pub fn book_refs_for_work(&self, work: &str) -> Result<Vec<(String, String)>> {
        self.with_personal(|c| {
            let mut stmt = c.prepare(
                "SELECT source_id, series_ref FROM book_source_match
                  WHERE work = ?1 ORDER BY confirmed DESC, confidence DESC, source_id",
            )?;
            let rows = stmt.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        })
    }

    /// The work a source's series was matched to and whether the person confirmed it.
    pub fn book_match_state(
        &self,
        source_id: &str,
        series_ref: &str,
    ) -> Result<Option<(String, bool)>> {
        self.with_personal(|c| {
            Ok(c.query_row(
                "SELECT work, confirmed FROM book_source_match
                  WHERE source_id = ?1 AND series_ref = ?2",
                params![source_id, series_ref],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(id: &str, state: &str) -> (LocalSeries, Vec<LocalFile>) {
        (
            LocalSeries {
                id: id.into(),
                folder: "f".into(),
                name: id.into(),
                medium: "manga".into(),
                work: None,
                state: state.into(),
                confidence: 0,
                reason: None,
            },
            vec![LocalFile {
                path: format!("/books/{id}/v1.cbz"),
                series: id.into(),
                volume: Some("1".into()),
                label: "Volume 1".into(),
                number_milli: None,
                format: "cbz".into(),
                pages: Some(12),
                size: 100,
                mtime: 5,
                comicinfo: None,
            }],
        )
    }

    #[test]
    fn a_rescan_keeps_the_match_the_person_made_and_drops_what_is_gone() {
        let store = Store::open_in_memory().unwrap();
        store
            .put_book_folder(&Folder {
                id: "f".into(),
                path: Some("/books".into()),
                available: true,
                ..Folder::default()
            })
            .unwrap();
        store
            .replace_book_scan("f", &[series("a", "unmatched"), series("b", "unmatched")], 10)
            .unwrap();
        store.set_local_series_match("a", Some("a:1"), "matched", 90, None).unwrap();
        store.replace_book_scan("f", &[series("a", "unmatched")], 20).unwrap();
        let all = store.local_book_series().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!((all[0].work.as_deref(), all[0].state.as_str()), (Some("a:1"), "matched"));
        assert_eq!(store.local_book_files("a").unwrap().len(), 1);
        assert_eq!(store.local_book_file_counts().unwrap(), [("a".to_owned(), 1)].into());
        assert_eq!(store.book_folders().unwrap()[0].last_scan, Some(20));
        store.remove_book_folder("f").unwrap();
        assert!(store.local_book_series().unwrap().is_empty());
        assert!(store.local_book_files("a").unwrap().is_empty());
    }

    /// R7K-12. A removed folder's series stayed in Continue for good. They leave it with the
    /// folder, also when the folder went before this was fixed (its matches are left the same
    /// way), keep their places, and come back when the folder is added again. A series read on
    /// another device, which no folder held, stays.
    #[test]
    fn a_removed_folders_series_leave_continue_and_come_back_with_the_folder() {
        use crate::reading::ReadEvent;
        const WORK: &str = "l:7e57f01de7";
        const ELSEWHERE: &str = "a:900777";
        let store = Store::open_in_memory().unwrap();
        let folder = Folder {
            id: "f".into(),
            path: Some("/books".into()),
            available: true,
            ..Folder::default()
        };
        let add = |store: &Store| {
            store.put_book_folder(&folder).unwrap();
            store.replace_book_scan("f", &[series("a", "unmatched")], 10).unwrap();
            store.set_local_series_match("a", Some(WORK), "unmatched", 0, None).unwrap();
            store.set_book_match("folders", "a", WORK, 0, false, false, 10).unwrap();
        };
        add(&store);
        for (work, at) in [(WORK, 100), (ELSEWHERE, 50)] {
            let mut event = ReadEvent::new(work, "1", at);
            event.page = Some(3);
            event.of_pages = Some(12);
            store.record_read(&event, Vec::new()).unwrap();
        }
        // Continue's order is the stamps', not the test's: compare what it holds.
        let continuing = |store: &Store| -> Vec<String> {
            let mut works: Vec<String> =
                store.continue_reading(12).unwrap().into_iter().map(|w| w.work).collect();
            works.sort();
            works
        };
        assert_eq!(continuing(&store), [ELSEWHERE, WORK]);

        store.remove_book_folder("f").unwrap();
        assert_eq!(continuing(&store), [ELSEWHERE]);
        assert!(store.read_work(WORK).unwrap().is_some(), "its place is kept");

        add(&store);
        assert_eq!(continuing(&store), [ELSEWHERE, WORK]);
    }

    #[test]
    fn servers_round_trip_and_removing_one_takes_its_matches() {
        let store = Store::open_in_memory().unwrap();
        let server = BookServer {
            id: "s".into(),
            kind: "komga".into(),
            name: "Shelf".into(),
            address: "http://shelf.example".into(),
            user: Some("me".into()),
            report_progress: true,
            signed_out: false,
            last_seen: None,
        };
        store.put_book_server(&server).unwrap();
        store.set_book_match("s", "ref", "l:abc", 80, false, false, 1).unwrap();
        assert_eq!(store.book_servers().unwrap(), vec![server]);
        assert_eq!(
            store.book_refs_for_work("l:abc").unwrap(),
            [("s".to_owned(), "ref".to_owned())]
        );
        store.remove_book_server("s").unwrap();
        assert!(store.book_servers().unwrap().is_empty());
        assert!(store.book_refs_for_work("l:abc").unwrap().is_empty());
    }

    #[test]
    fn a_refused_sign_in_changes_only_a_kept_server() {
        let store = Store::open_in_memory().unwrap();
        let server = BookServer {
            id: "s".into(),
            kind: "kavita".into(),
            name: "Shelf".into(),
            address: "http://shelf.example".into(),
            user: Some("me".into()),
            report_progress: true,
            signed_out: false,
            last_seen: Some(5),
        };
        store.put_book_server(&server).unwrap();
        assert!(store.set_book_server_signed_out("s", true).unwrap());
        assert!(!store.set_book_server_signed_out("s", true).unwrap(), "already said");
        let kept = store.book_server("s").unwrap().unwrap();
        assert_eq!(kept, BookServer { signed_out: true, ..server });
        assert!(store.set_book_server_signed_out("s", false).unwrap());
        // A server removed while its source was asked is not brought back.
        assert!(!store.set_book_server_signed_out("gone", true).unwrap());
        assert!(store.book_server("gone").unwrap().is_none());
    }
}
