//! Where books come from: folders, book servers and book plugins.
//!
//! The add flow offers a folder, a server or a plugin store invite, and nothing else. Store joins
//! use the store commands shows use. A server's sign-in reuses the media server flow's steps and
//! failures, with one failure of its own: the address answered and is not a book server.

use kaeri_core::{books::WorkId, ids::AnilistId, media::Medium};

use crate::{
    local::MatchChoice,
    reading::BookEditionRow,
    servers::{ServerFailure, ServerStep},
    stores::Secret,
};

/// A folder to add. Which field is set depends on the platform.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FolderRef {
    /// Desktop, and Android with all-files access.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub path: Option<String>,
    /// An Android document tree from the system folder picker.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub tree_uri: Option<String>,
    /// An Apple security-scoped bookmark, resolved by the app.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub bookmark: Option<Vec<u8>>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum BookServerKind {
    Komga,
    Kavita,
    Opds,
}

impl BookServerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Komga => "komga",
            Self::Kavita => "kavita",
            Self::Opds => "opds",
        }
    }
}

/// Why a book server's step failed: the media server flow's failures, or not a book server.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum BookServerFailure {
    Server {
        failure: ServerFailure,
    },
    /// Something answered, and it is not a book server Kaeri knows.
    NotABookServer,
}

/// Where a folder's scan stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum BookFolderScan {
    Scanning {
        found: u32,
    },
    Done,
    /// The folder is gone, or the drive is not connected.
    Missing,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BookFolderRow {
    pub id: String,
    pub name: String,
    pub scan: BookFolderScan,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub path: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub series: u32,
    /// Removed, with Undo for 8 seconds.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub removed: bool,
}

/// Books in a folder that read as one series, matched or kept as Your files.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BookGroupRow {
    pub group: String,
    pub name: String,
    pub medium: Medium,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub work: Option<WorkId>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub matched_title: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub files: u32,
    /// Kept as Your files on purpose: no match is asked for.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub kept: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BookServerRow {
    pub id: String,
    pub kind: BookServerKind,
    pub name: String,
    pub address: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub series: u32,
    /// Progress is sent back to the server.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub report_progress: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub off: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub signed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BookPluginRow {
    pub id: String,
    pub name: String,
    pub version: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub media: Vec<Medium>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub languages: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub off: bool,
    /// The store it came from.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub store: Option<String>,
}

/// Every book source, in the order they are asked.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BookSourcesView {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub folders: Vec<BookFolderRow>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub groups: Vec<BookGroupRow>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub servers: Vec<BookServerRow>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub plugins: Vec<BookPluginRow>,
    /// The ids of servers and plugins, in the order they are asked.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum BookSourceCommand {
    Load,
    AddFolder {
        folder: FolderRef,
    },
    /// Undo for 8 seconds, as for video folders.
    RemoveFolder {
        id: String,
    },
    RestoreFolder {
        id: String,
    },
    Scan,
    FindMatch {
        group: String,
        query: String,
    },
    ChooseMatch {
        group: String,
        anilist: AnilistId,
    },
    /// Keep the group as Your files.
    Keep {
        group: String,
    },
    CheckServer {
        address: String,
    },
    /// The token goes to the secure store.
    SignIn {
        address: String,
        user: String,
        password: Secret,
    },
    RemoveServer {
        id: String,
    },
    SetReportProgress {
        id: String,
        on: bool,
    },
    SetOrder {
        ids: Vec<String>,
    },
    SetOff {
        id: String,
        off: bool,
    },
    /// Which of your sources have the work, asked only when someone opens the series.
    LoadForWork {
        work: WorkId,
    },
}

impl BookSourceCommand {
    pub fn activity(&self) -> Option<String> {
        Some(match self {
            Self::AddFolder { .. } | Self::Scan => "scanning folders".into(),
            Self::FindMatch { .. } => "finding matches".into(),
            Self::CheckServer { .. } => "checking the server".into(),
            Self::SignIn { .. } => "signing in to the server".into(),
            Self::LoadForWork { .. } => "asking your sources".into(),
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum BookSourceEvent {
    View(BookSourcesView),
    /// Fix this match for a group; `episodes` on each choice counts chapters here.
    Matches {
        group: String,
        query: String,
        choices: Vec<MatchChoice>,
        offline: bool,
    },
    ServerChecked {
        address: String,
        kind: BookServerKind,
        name: String,
        version: String,
    },
    /// "Found a Komga server with 120 series".
    ServerAdded {
        id: String,
        kind: BookServerKind,
        series: u32,
    },
    ServerFailed {
        step: ServerStep,
        failure: BookServerFailure,
    },
    /// Sent as each source answers; `asking` until the last one has.
    ForWork {
        work: WorkId,
        editions: Vec<BookEditionRow>,
        asking: bool,
    },
}
