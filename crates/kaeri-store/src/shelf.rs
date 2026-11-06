//! The book files a work can be read from on this device: files in the folders the person added
//! and chapters downloaded. The folder scan writes the first, downloads the second; the reader
//! reads both through [`Store::shelf`].

use rusqlite::{OptionalExtension, params};

use crate::{Result, Store};

/// One file a work can be read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfFile {
    pub path: String,
    /// The chapter id, when the file was downloaded for one; files in a folder have their label.
    pub chapter: Option<String>,
    pub label: String,
    pub volume: Option<String>,
    /// The chapter number in thousandths, when the name gave one.
    pub number_milli: Option<i64>,
    /// `cbz`, `cbr`, `cb7`, `epub`, `pdf` or `folder`.
    pub format: String,
    pub pages: Option<u32>,
    /// ComicInfo, as JSON.
    pub comicinfo: Option<String>,
}

/// One series found in a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSeries {
    pub id: String,
    pub folder: String,
    pub name: String,
    /// `manga` or `novel`.
    pub medium: String,
    pub work: Option<String>,
    pub state: String,
}

/// What the catalogue knows of a work's kind, for the reader's guesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookWorkKind {
    pub medium: String,
    /// AniList's format: `MANGA`, `NOVEL`, `ONE_SHOT`.
    pub format: Option<String>,
    /// AniList's country of origin: `JP`, `KR`, `CN`, `TW`.
    pub origin: Option<String>,
    /// The first of the titles kept.
    pub title: Option<String>,
}

impl Store {
    /// The files a work can be read from: its series' files in folders, then its finished
    /// downloads.
    pub fn shelf(&self, work: &str) -> Result<Vec<ShelfFile>> {
        self.with_conn(|c| {
            let mut out: Vec<ShelfFile> = {
                let mut stmt = c.prepare(
                    "SELECT f.path, f.label, f.volume, f.number_milli, f.format, f.pages,
                            f.comicinfo
                       FROM book_local_file f JOIN book_local_series s ON s.id = f.series
                      WHERE s.work = ?1
                      ORDER BY f.path",
                )?;
                stmt.query_map([work], |r| {
                    Ok(ShelfFile {
                        path: r.get(0)?,
                        chapter: None,
                        label: r.get(1)?,
                        volume: r.get(2)?,
                        number_milli: r.get(3)?,
                        format: r.get(4)?,
                        pages: r.get(5)?,
                        comicinfo: r.get(6)?,
                    })
                })?
                .collect::<rusqlite::Result<_>>()?
            };
            let mut stmt = c.prepare(
                "SELECT path, chapter, pages_total FROM book_download
                  WHERE work = ?1 AND state = 'done' AND path IS NOT NULL ORDER BY id",
            )?;
            let downloads = stmt.query_map([work], |r| {
                let path: String = r.get(0)?;
                let chapter: String = r.get(1)?;
                let format = std::path::Path::new(&path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map_or_else(|| "folder".to_owned(), str::to_ascii_lowercase);
                Ok(ShelfFile {
                    path,
                    label: chapter.clone(),
                    chapter: Some(chapter),
                    volume: None,
                    number_milli: None,
                    format,
                    pages: r.get(2)?,
                    comicinfo: None,
                })
            })?;
            for file in downloads {
                out.push(file?);
            }
            Ok(out)
        })
    }

    /// Keep a series found in a folder.
    pub fn put_local_series(&self, series: &LocalSeries) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO book_local_series (id, folder, name, medium, work, state)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET folder = excluded.folder, name = excluded.name,
                    medium = excluded.medium, work = excluded.work, state = excluded.state",
                params![
                    series.id,
                    series.folder,
                    series.name,
                    series.medium,
                    series.work,
                    series.state
                ],
            )?;
            Ok(())
        })
    }

    /// Keep one file of a series found in a folder.
    pub fn put_local_file(
        &self,
        series: &str,
        file: &ShelfFile,
        size: u64,
        mtime: i64,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO book_local_file
                    (path, series, volume, label, number_milli, format, pages, size, mtime,
                     comicinfo)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(path) DO UPDATE SET series = excluded.series,
                    volume = excluded.volume, label = excluded.label,
                    number_milli = excluded.number_milli, format = excluded.format,
                    pages = excluded.pages, size = excluded.size, mtime = excluded.mtime,
                    comicinfo = excluded.comicinfo",
                params![
                    file.path,
                    series,
                    file.volume,
                    file.label,
                    file.number_milli,
                    file.format,
                    file.pages,
                    size as i64,
                    mtime,
                    file.comicinfo
                ],
            )?;
            Ok(())
        })
    }

    /// What the catalogue holds of a work's kind, when it has fetched the work.
    pub fn book_work_kind(&self, work: &str) -> Result<Option<BookWorkKind>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT medium, format, origin, titles FROM book_work WHERE work = ?1",
                [work],
                |r| {
                    let titles: String = r.get(3)?;
                    Ok(BookWorkKind {
                        medium: r.get(0)?,
                        format: r.get(1)?,
                        origin: r.get(2)?,
                        title: first_title(&titles),
                    })
                },
            )
            .optional()?)
        })
    }
}

/// The first title of a `titles` column: a JSON list of strings, or of objects with a `title`.
fn first_title(titles: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(titles).ok()?;
    let first = value.as_array()?.first()?;
    first.as_str().or_else(|| first.get("title").and_then(|t| t.as_str())).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_work_reads_from_its_folder_files_and_its_downloads() {
        let store = Store::open_in_memory().unwrap();
        let work = "l:5eed";
        store
            .put_local_series(&LocalSeries {
                id: "s1".into(),
                folder: "/books".into(),
                name: "Lantern Street".into(),
                medium: "manga".into(),
                work: Some(work.into()),
                state: "matched".into(),
            })
            .unwrap();
        let file = ShelfFile {
            path: "/books/Lantern Street/Chapter 2.cbz".into(),
            chapter: None,
            label: "Chapter 2".into(),
            volume: None,
            number_milli: Some(2000),
            format: "cbz".into(),
            pages: Some(20),
            comicinfo: None,
        };
        store.put_local_file("s1", &file, 1000, 1).unwrap();
        store
            .with_conn(|c| {
                c.execute(
                    "INSERT INTO book_download (work, chapter, source_id, state, path, created_at,
                        updated_at) VALUES (?1, '3', 'src', 'done', '/dl/3.cbz', 1, 1)",
                    [work],
                )?;
                Ok(())
            })
            .unwrap();
        let shelf = store.shelf(work).unwrap();
        assert_eq!(shelf.len(), 2);
        assert_eq!(shelf[0], file);
        assert_eq!((shelf[1].chapter.as_deref(), shelf[1].format.as_str()), (Some("3"), "cbz"));
        assert!(store.shelf("l:0ther").unwrap().is_empty());
        assert_eq!(first_title(r#"["Lantern Street"]"#).as_deref(), Some("Lantern Street"));
        assert_eq!(first_title(r#"[{"title":"Moth Garden"}]"#).as_deref(), Some("Moth Garden"));
    }
}
