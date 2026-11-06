//! The chapters a work can be read from on this device, in reading order: the files in folders
//! and the finished downloads, from [`kaeri_store::Store::shelf`]. Sources that stream pages
//! join here when they land.

use std::path::PathBuf;

use kaeri_core::books::{ChapterId, ChapterKey, ChapterRange};
use kaeri_model::reading::{ChapterReadState, ChapterRow};
use kaeri_store::{Store, reading::ReadMarks, shelf::ShelfFile};

/// One chapter and the file it reads from.
#[derive(Debug, Clone, PartialEq)]
pub struct ShelfChapter {
    pub id: ChapterId,
    pub label: String,
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
            title: None,
            volume: self.volume.clone(),
            published: None,
            pages: self.pages,
            new_chapter: false,
            download: None,
            versions: 1,
            bookmarked: false,
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
}
