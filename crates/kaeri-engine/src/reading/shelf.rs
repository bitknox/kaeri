//! The chapters a work can be read from on this device, in reading order: the files in folders
//! and the finished downloads, from [`kaeri_store::Store::shelf`]. Sources that stream pages
//! join here when they land.

use std::path::PathBuf;

use kaeri_core::books::{ChapterId, ChapterKey, ChapterRange};
use kaeri_model::reading::{ChapterReadState, ChapterRow, VolumeGroup};
use kaeri_store::{Store, reading::ReadMarks, shelf::ShelfFile};

/// One chapter and the file it reads from.
#[derive(Debug, Clone, PartialEq)]
pub struct ShelfChapter {
    pub id: ChapterId,
    pub label: String,
    /// Its own title, when the source gave one that is more than the label again.
    pub title: Option<String>,
    pub volume: Option<String>,
    pub path: PathBuf,
    /// `cbz`, `cbr`, `cb7`, `epub`, `pdf` or `folder`.
    pub format: String,
    pub pages: Option<u32>,
    pub comicinfo: Option<String>,
}

impl ShelfChapter {
    /// Text chapters go to the novel reader; everything else is pages.
    pub fn is_text(&self) -> bool {
        self.format == "epub"
    }

    pub fn row(&self, marks: &ReadMarks) -> ChapterRow {
        let number = self.id.key().and_then(|k| k.range()).map(|r| r.to_string());
        ChapterRow {
            id: self.id.clone(),
            label: self.label.clone(),
            state: if marks.is_read(self.id.as_str()) {
                ChapterReadState::Read
            } else {
                ChapterReadState::Unread
            },
            number,
            title: self.title.clone(),
            volume: self.volume.clone(),
            published: None,
            pages: self.pages,
            new_chapter: false,
            download: None,
            versions: 1,
            bookmarked: false,
            version_label: None,
            read_at: None,
            line: Vec::new(),
        }
    }
}

fn volume_number(volume: Option<&str>) -> f64 {
    volume.and_then(|v| v.trim().parse().ok()).unwrap_or(f64::MAX)
}

/// A work's chapters in reading order. Numbers that appear in two volumes start again each
/// volume, so their keys carry the volume.
pub fn chapters(store: &Store, work: &str) -> Vec<ShelfChapter> {
    let files = store.shelf(work).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "the work's files were not read");
        Vec::new()
    });
    order(files)
}

/// Name the downloaded chapters as their source does ("Episode 63", with its title), from the
/// source's list as kept. A download knows only its chapter id until then; a file in a folder
/// keeps its own label.
pub fn name_downloads(
    chapters: &mut [ShelfChapter],
    list: &[crate::read_catalog::chapters::Kept],
) {
    for chapter in chapters.iter_mut().filter(|c| c.label == c.id.as_str()) {
        let Some(kept) = list.iter().find(|k| k.id == chapter.id.as_str()) else { continue };
        chapter.title = kept.own_title().map(str::to_owned);
        chapter.label.clone_from(&kept.label);
        if chapter.volume.is_none() {
            chapter.volume.clone_from(&kept.volume);
        }
        chapter.pages = chapter.pages.or(kept.pages);
    }
}

/// A work's files as the chapter list shows them, grouped by volume, with the person's reading
/// marked: what a series with no source reads from.
pub fn volumes(
    store: &Store,
    work: &str,
    list: Option<&[crate::read_catalog::chapters::Kept]>,
    now: i64,
    zone: &jiff::tz::TimeZone,
) -> Vec<VolumeGroup> {
    let mut chapters = chapters(store, work);
    if let Some(list) = list {
        name_downloads(&mut chapters, list);
    }
    let marks = store.read_marks(work).unwrap_or_default();
    let places = store.read_chapters(work).unwrap_or_default();
    let mut groups: Vec<VolumeGroup> = Vec::new();
    for chapter in &chapters {
        let mut row = chapter.row(&marks);
        if let Some(place) = places.iter().find(|p| p.chapter == chapter.id.as_str()) {
            row.read_at = place.read_at.filter(|_| marks.is_read(chapter.id.as_str()));
            if let (ChapterReadState::Unread, Some(page)) = (row.state, place.last_page) {
                row.state = ChapterReadState::InProgress {
                    page: kaeri_model::reading::shown_page(page),
                    of: place.of_pages,
                };
            }
        }
        match groups.iter_mut().find(|g| g.volume == chapter.volume) {
            Some(group) => group.chapters.push(row),
            None => groups.push(VolumeGroup {
                volume: chapter.volume.clone(),
                title: None,
                chapters: vec![row],
                all_read: false,
            }),
        }
    }
    for group in &mut groups {
        group.all_read =
            group.chapters.iter().all(|c| matches!(c.state, ChapterReadState::Read));
    }
    crate::read_catalog::rows::fill(
        &mut groups,
        crate::read_catalog::rows::Kept::Device,
        now,
        zone,
    );
    groups
}

fn order(files: Vec<ShelfFile>) -> Vec<ShelfChapter> {
    let number = |f: &ShelfFile| {
        f.number_milli.or_else(|| ChapterRange::parse(&f.label).map(|r| r.last().milli()))
    };
    let restarts = files.iter().any(|a| {
        files.iter().any(|b| {
            a.volume.is_some()
                && b.volume.is_some()
                && a.volume != b.volume
                && number(a).is_some()
                && number(a) == number(b)
        })
    });
    let mut sorted: Vec<(f64, i64, ShelfChapter)> = Vec::new();
    for file in files {
        let id = match &file.chapter {
            Some(chapter) => ChapterId::new(chapter.clone()),
            None => ChapterKey::of(&file.label, file.volume.as_deref(), restarts).id(),
        };
        if sorted.iter().any(|(_, _, c)| c.id == id) {
            // A chapter both in a folder and downloaded: the folder's file is read.
            continue;
        }
        let key = (volume_number(file.volume.as_deref()), number(&file).unwrap_or(i64::MAX));
        // Without a volume, the number alone orders.
        let key = if restarts { key } else { (0.0, key.1) };
        sorted.push((
            key.0,
            key.1,
            ShelfChapter {
                id,
                label: file.label,
                title: None,
                volume: file.volume,
                path: PathBuf::from(file.path),
                format: file.format,
                pages: file.pages,
                comicinfo: file.comicinfo,
            },
        ));
    }
    sorted.sort_by(|a, b| {
        a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.label.cmp(&b.2.label))
    });
    sorted.into_iter().map(|(_, _, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(label: &str, volume: Option<&str>) -> ShelfFile {
        ShelfFile {
            path: format!("/books/{label}.cbz"),
            chapter: None,
            label: label.into(),
            volume: volume.map(str::to_owned),
            number_milli: None,
            format: "cbz".into(),
            pages: None,
            comicinfo: None,
        }
    }

    #[test]
    fn chapters_sort_by_number_and_restart_per_volume_when_numbers_repeat() {
        let ids = |files| order(files).into_iter().map(|c| c.id.0).collect::<Vec<_>>();
        assert_eq!(
            ids(vec![file("Chapter 10", None), file("Chapter 2", None), file("Extra", None)]),
            ["2", "10", "x/-/extra"]
        );
        assert_eq!(
            ids(vec![
                file("Chapter 2", Some("2")),
                file("Chapter 1", Some("2")),
                file("Chapter 1", Some("1")),
            ]),
            ["v1/1", "v2/1", "v2/2"]
        );
    }

    #[test]
    fn a_download_reads_as_its_source_names_it_and_a_folder_file_keeps_its_label() {
        let download = ShelfFile {
            path: "/downloads/63.cbz".into(),
            chapter: Some("63".into()),
            label: "63".into(),
            ..file("63", None)
        };
        let mut chapters = order(vec![download, file("Chapter 64", None)]);
        let list: Vec<crate::read_catalog::chapters::Kept> = serde_json::from_str(
            r#"[{"id":"63","source_ref":"s-63","label":"Episode 63","number":"63",
                 "volume":"7","title":"The Lantern Ferry","pages":19},
                {"id":"64","source_ref":"s-64","label":"Episode 64","number":"64"}]"#,
        )
        .unwrap();
        name_downloads(&mut chapters, &list);
        let row = chapters[0].row(&ReadMarks::default());
        assert_eq!(row.label, "Episode 63");
        assert_eq!(row.title.as_deref(), Some("The Lantern Ferry"));
        assert_eq!((row.volume.as_deref(), row.pages), (Some("7"), Some(19)));
        assert_eq!(chapters[1].label, "Chapter 64", "a folder's file is named by its file");
    }
}
