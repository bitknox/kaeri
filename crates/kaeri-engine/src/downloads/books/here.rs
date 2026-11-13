//! Whether a chapter's file is on this device, and where. Sends to an e-reader, the download
//! estimate, the download queue and the reader's Download offer all ask this one question, so
//! their answers agree: a finished download whose file is still there, or a file in the
//! person's folders (a plain path or a tree the system lends, such as Android's SAF).

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use kaeri_store::{Store, book_downloads::BookDownloadState};

use crate::{read_sources::FOLDERS, reading::shelf::ShelfChapter};

/// A chapter's file on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Here {
    /// A finished download whose file is still there, with what it takes when that is known.
    Downloaded { path: PathBuf, bytes: Option<u64> },
    /// A file in the person's folders. It opens from where it is, and nothing of it downloads.
    Folder { path: PathBuf },
}

impl Here {
    /// The chapter as the reader's shelf lists it: a finished download, a file in a folder, or
    /// `None` for one a source streams.
    pub(crate) fn of_shelf(chapter: &ShelfChapter) -> Option<Self> {
        if chapter.format == crate::reading::session::SOURCE
            || chapter.path.as_os_str().is_empty()
        {
            return None;
        }
        let path = chapter.path.clone();
        Some(match chapter.source {
            Some(_) => Self::Downloaded { path, bytes: None },
            None => Self::Folder { path },
        })
    }

    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::Downloaded { path, .. } | Self::Folder { path } => path,
        }
    }

    /// What the file takes on this device, or `None` when that cannot be told.
    pub(crate) fn bytes(&self) -> Option<u64> {
        match self {
            Self::Downloaded { bytes: Some(bytes), .. } => Some(*bytes),
            _ => kaeri_books::tree::stat(self.path()).ok().map(|s| s.size),
        }
    }

    pub(crate) fn is_folder(&self) -> bool {
        matches!(self, Self::Folder { .. })
    }
}

/// Whether a chapter listed from `source` is a file in the person's folders.
pub(crate) fn in_folders(source: Option<&str>) -> bool {
    source == Some(FOLDERS)
}

/// The chapters of one work that are on this device, as the reader's shelf lists them: by the
/// chapter ids every list of the work uses, so a file in a folder is found by its chapter
/// whatever its file is called.
pub(crate) struct OnDevice {
    files: HashMap<String, Here>,
}

impl OnDevice {
    pub(crate) fn of(store: &Store, work: &str) -> Self {
        // What each finished download took, as it said when it finished.
        let took: HashMap<String, u64> = store
            .book_downloads(Some(work))
            .unwrap_or_default()
            .into_iter()
            .filter(|d| d.state == BookDownloadState::Done && d.bytes > 0)
            .map(|d| (d.chapter, d.bytes))
            .collect();
        let files = crate::reading::shelf::chapters(store, work)
            .iter()
            .filter_map(|chapter| {
                let here = match Here::of_shelf(chapter)? {
                    // A download whose file went is downloaded again rather than sent.
                    Here::Downloaded { path, .. } => {
                        let there = kaeri_books::tree::stat(&path).is_ok_and(|s| !s.folder);
                        let bytes = took.get(chapter.id.as_str()).copied();
                        there.then_some(Here::Downloaded { path, bytes })?
                    }
                    // A file gone from its folder is still named, so whatever opens it says
                    // which file is missing rather than fetching it.
                    folder => folder,
                };
                Some((chapter.id.as_str().to_owned(), here))
            })
            .collect();
        Self { files }
    }

    /// The file of `chapter` on this device, if it is here.
    pub(crate) fn file(&self, chapter: &str) -> Option<&Here> {
        self.files.get(chapter)
    }
}
