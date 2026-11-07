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
    /// The source a download came from; `None` for a file in a folder.
    pub source: Option<String>,
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
    /// The title as it shows when no language is chosen: [`Self::title_in`] with English.
    pub title: Option<String>,
    /// Every form of the title kept, for [`Self::title_in`].
    pub titles: BookTitles,
    /// AniList's publishing status: `RELEASING`, `FINISHED`, `CANCELLED`.
    pub status: Option<String>,
}

/// Which form of a title to show first, as the title language setting resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitleForm {
    #[default]
    English,
    Romaji,
    Native,
}

/// A work's titles as the catalogue keeps them: AniList's forms, or a plain list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookTitles {
    pub english: Option<String>,
    pub romaji: Option<String>,
    pub native: Option<String>,
    /// The first of a plain list, or a `display` title, kept where no form is known.
    pub other: Option<String>,
}

impl BookTitles {
    /// Read a `titles` column: a JSON list of strings or of objects with a `title`, or an
    /// object of AniList's forms.
    pub fn parse(titles: &str) -> Self {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(titles) else {
            return Self::default();
        };
        let text = |v: Option<&serde_json::Value>| {
            v.and_then(|t| t.as_str()).filter(|t| !t.is_empty()).map(str::to_owned)
        };
        if let Some(list) = value.as_array() {
            let first = list.first();
            let other = text(first).or_else(|| text(first.and_then(|f| f.get("title"))));
            return Self { other, ..Self::default() };
        }
        Self {
            english: text(value.get("english")),
            romaji: text(value.get("romaji")),
            native: text(value.get("native")),
            other: text(value.get("display")),
        }
    }

    /// The form to show: `first` when kept, then English, romaji and native, as anime
    /// titles fall back.
    pub fn pick(&self, first: TitleForm) -> Option<&str> {
        let chosen = match first {
            TitleForm::English => &self.english,
            TitleForm::Romaji => &self.romaji,
            TitleForm::Native => &self.native,
        };
        [chosen, &self.english, &self.romaji, &self.native, &self.other]
            .into_iter()
            .find_map(|form| form.as_deref())
    }
}

impl BookWorkKind {
    /// The title in `first`, falling back as [`BookTitles::pick`] does.
    pub fn title_in(&self, first: TitleForm) -> Option<String> {
        self.titles.pick(first).map(str::to_owned)
    }

    /// A webtoon: tagged as a long strip, or from Korea, China or Taiwan, where comics are
    /// made as strips.
    pub fn webtoon(&self) -> bool {
        self.format.as_deref() == Some("webtoon")
            || self.origin.as_deref().is_some_and(|o| matches!(o, "KR" | "CN" | "TW"))
    }

    /// No more chapters are coming: the series is finished or was cancelled.
    pub fn ended(&self) -> bool {
        matches!(self.status.as_deref(), Some("FINISHED" | "CANCELLED"))
    }
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
                        source: None,
                    })
                })?
                .collect::<rusqlite::Result<_>>()?
            };
            let mut stmt = c.prepare(
                "SELECT path, chapter, pages_total, source_id FROM book_download
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
                    source: r.get(3)?,
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

    /// Record a work's kind and title as the catalogue would, for the scenes whose books are
    /// made on the spot. An earlier row of the work is replaced.
    pub fn seed_book_work(
        &self,
        work: &str,
        medium: &str,
        format: Option<&str>,
        origin: Option<&str>,
        title: &str,
        now: i64,
    ) -> Result<()> {
        let titles = serde_json::to_string(&[title]).unwrap_or_else(|_| "[]".to_owned());
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO book_work (work, medium, format, origin, titles, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(work) DO UPDATE SET medium = excluded.medium,
                    format = excluded.format, origin = excluded.origin,
                    titles = excluded.titles, fetched_at = excluded.fetched_at",
                params![work, medium, format, origin, titles, now],
            )?;
            Ok(())
        })
    }

    /// What the catalogue holds of a work's kind, when it has fetched the work.
    pub fn book_work_kind(&self, work: &str) -> Result<Option<BookWorkKind>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT medium, format, origin, titles, status FROM book_work WHERE work = ?1",
                [work],
                |r| {
                    let titles = BookTitles::parse(&r.get::<_, String>(3)?);
                    Ok(BookWorkKind {
                        medium: r.get(0)?,
                        format: r.get(1)?,
                        origin: r.get(2)?,
                        title: titles.pick(TitleForm::English).map(str::to_owned),
                        titles,
                        status: r.get(4)?,
                    })
                },
            )
            .optional()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_work_kind_is_titled_from_a_list_or_the_catalogue_object() {
        let first_title = |titles: &str| BookTitles::parse(titles).pick(TitleForm::English).map(str::to_owned);
        assert_eq!(first_title(r#"["Tide Lantern"]"#).as_deref(), Some("Tide Lantern"));
        assert_eq!(
            first_title(r#"{"romaji":"Shiokaze","english":null,"native":null}"#).as_deref(),
            Some("Shiokaze"),
        );
        assert_eq!(
            first_title(r#"{"romaji":"Shiokaze","english":"Sea Breeze"}"#).as_deref(),
            Some("Sea Breeze"),
        );
        assert_eq!(first_title("{}"), None);
    }

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
            source: None,
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
    }

    #[test]
    fn a_title_shows_in_the_chosen_language_and_falls_back() {
        let list = |t: &str| BookTitles::parse(t).pick(TitleForm::Romaji).map(str::to_owned);
        assert_eq!(list(r#"["Lantern Street"]"#).as_deref(), Some("Lantern Street"));
        assert_eq!(list(r#"[{"title":"Moth Garden"}]"#).as_deref(), Some("Moth Garden"));
        assert_eq!(
            list(r#"{"display":"The Lantern Ferry"}"#).as_deref(),
            Some("The Lantern Ferry")
        );

        let all = BookTitles::parse(
            r#"{"romaji":"Touro no Watashibune","english":"The Lantern Ferry","native":"灯籠の渡し舟","synonyms":[]}"#,
        );
        assert_eq!(all.pick(TitleForm::English), Some("The Lantern Ferry"));
        assert_eq!(all.pick(TitleForm::Romaji), Some("Touro no Watashibune"));
        assert_eq!(all.pick(TitleForm::Native), Some("灯籠の渡し舟"));

        // A missing or empty form falls back English, then romaji, then native.
        let some =
            BookTitles::parse(r#"{"romaji":"Garasu no Kajuen","english":"","native":null}"#);
        assert_eq!(some.pick(TitleForm::English), Some("Garasu no Kajuen"));
        assert_eq!(some.pick(TitleForm::Native), Some("Garasu no Kajuen"));
        let native =
            BookTitles::parse(r#"{"romaji":null,"english":null,"native":"硝子の果樹園"}"#);
        assert_eq!(native.pick(TitleForm::Romaji), Some("硝子の果樹園"));
        assert_eq!(BookTitles::parse(r#"{"romaji":null}"#).pick(TitleForm::English), None);
        assert_eq!(BookTitles::parse("not json").pick(TitleForm::English), None);
    }

    #[test]
    fn a_work_kind_reads_the_catalogue_titles() {
        let store = Store::open_in_memory().unwrap();
        store.seed_book_work("a:1", "novel", None, None, "Seeded", 0).unwrap();
        let kind = store.book_work_kind("a:1").unwrap().unwrap();
        assert_eq!(kind.title.as_deref(), Some("Seeded"));
        assert_eq!(kind.title_in(TitleForm::Native).as_deref(), Some("Seeded"));
    }
}
