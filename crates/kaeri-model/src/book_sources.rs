//! Where books come from: folders, book servers and book plugins.
//!
//! The add flow offers a folder, a server or a plugin store invite, and nothing else. Store joins
//! use the store commands shows use. A server's sign-in reuses the media server flow's steps and
//! failures, with one failure of its own: the address answered and is not a book server.

use kaeri_core::{books::WorkId, ids::AnilistId, media::Medium};

use crate::{
    Msg,
    local::MatchChoice,
    reading::{BookEditionRow, ReadingFailure},
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
    /// Every file the last scan met: "312 files".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub files: u32,
    /// Files the last scan set aside as books Kaeri can't open, or locked: "23 files skipped".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub skipped: u32,
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
    /// The libraries added: "212 series in 2 libraries". Nought for an OPDS catalogue.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub libraries: u32,
    /// It showed no series when last asked: "no series yet · looks again in 30 minutes".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub empty: bool,
    /// When Kaeri looks at an empty server again, as a Unix time.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rechecks_at: Option<i64>,
    /// The last four characters of the Kavita key it signs in with: "Key ••••1234".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub key_last4: Option<String>,
    /// Chapters whose progress waits for this server, held ones included.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub progress_pending: u32,
    /// A send to it is under way: "Sending 3 chapters…".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub progress_sending: bool,
    /// When it last took progress, in seconds since the epoch.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub progress_saved_at: Option<i64>,
    /// Why progress waits, while it does: the last try's failure, `Offline` while the device
    /// is, and `NotSetToReceive` for a server set not to receive progress (with no queue).
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub progress_problem: Option<WriteBackProblem>,
    /// When the last try failed.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub progress_problem_at: Option<i64>,
}

/// Why reading progress has not reached a book server yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum WriteBackProblem {
    /// The device is offline: it sends when back online.
    Offline,
    /// The server did not answer: Kaeri tries again, after 30 s, 2 min, 10 min, then hourly.
    NoAnswer,
    /// The server answered with an error: tried again as for `NoAnswer`.
    ServerError,
    /// The server refused the key. Not tried again until Try again.
    RefusedKey,
    /// The server refused the user name or password. Not tried again until Try again.
    RefusedPassword,
    /// The series is not on the server any more. Not tried again until Try again.
    SeriesGone,
    /// A day of tries without an answer. Not tried again until Try again.
    NoAnswerForADay,
    /// The server is set not to receive progress: it stays on this device.
    NotSetToReceive,
}

impl WriteBackProblem {
    /// Whether Kaeri tries again on its own: otherwise the progress waits for Try again.
    pub const fn retries(self) -> bool {
        matches!(self, Self::Offline | Self::NoAnswer | Self::ServerError)
    }

    /// The name the store keeps it by.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::NoAnswer => "no-answer",
            Self::ServerError => "server-error",
            Self::RefusedKey => "refused-key",
            Self::RefusedPassword => "refused-password",
            Self::SeriesGone => "series-gone",
            Self::NoAnswerForADay => "no-answer-for-a-day",
            Self::NotSetToReceive => "not-set-to-receive",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::Offline,
            Self::NoAnswer,
            Self::ServerError,
            Self::RefusedKey,
            Self::RefusedPassword,
            Self::SeriesGone,
            Self::NoAnswerForADay,
            Self::NotSetToReceive,
        ]
        .into_iter()
        .find(|p| p.as_str() == text)
    }
}

/// What became of one chapter's progress sent to its book server, for the page between
/// chapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum WriteBackResult {
    /// "Saved to books.home.example".
    Saved,
    /// Kept, and sent later without being asked: offline, no answer, a server error.
    Waiting { problem: WriteBackProblem },
    /// Kept until Try again: a refused key or password, the series gone, a day without an
    /// answer, or a server set not to receive.
    NotSaved { problem: WriteBackProblem },
}

impl WriteBackResult {
    pub const fn of(problem: WriteBackProblem) -> Self {
        if problem.retries() { Self::Waiting { problem } } else { Self::NotSaved { problem } }
    }
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
    /// Try again: send every chapter's progress waiting for this server now, the held ones too.
    RetryProgress {
        id: String,
    },
    SetOrder {
        ids: Vec<String>,
    },
    SetOff {
        id: String,
        off: bool,
    },
    /// Which of your sources have the work, asked only when someone opens the series. The
    /// sources without it are searched by the work's titles, and the ones still without it are
    /// listed as missing.
    LoadForWork {
        work: WorkId,
    },
    /// "Search there": look for the work in one source by words the person typed.
    SearchSourceForWork {
        work: WorkId,
        source_id: String,
        query: String,
    },
    /// The person picked one of a source's series as this work. The editions are sent again.
    ChooseSourceSeries {
        work: WorkId,
        source_id: String,
        series_ref: String,
    },
    /// Sign in to a server and say what it holds, without adding it yet. `address` may be a
    /// Kavita OPDS link (`https://host/api/opds/{key}`), whose key signs in with no user or
    /// password. Answered with `Event::CameIn`: `Found`, or `Empty` with the reason.
    Probe {
        address: String,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        user: Option<String>,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        password: Option<Secret>,
    },
    /// Add a server `Probe` found, asking only `libraries` (empty asks every one). Its sign-in
    /// goes to the secure store. Answered with `Event::CameIn(ServerAdded)` and the view.
    AddServer {
        pending: String,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        libraries: Vec<String>,
        report_progress: bool,
    },
    /// Look again at servers that showed no series, now: pull to refresh. Scan does too.
    Recheck,
    /// Fetch the reading demo's first pages ahead, as the books page shows with the demo on.
    PrefetchDemo,
}

impl BookSourceCommand {
    pub fn activity(&self) -> Option<String> {
        Some(match self {
            Self::AddFolder { .. } | Self::Scan => "scanning folders".into(),
            Self::FindMatch { .. } => "finding matches".into(),
            Self::CheckServer { .. } => "checking the server".into(),
            Self::SignIn { .. } => "signing in to the server".into(),
            Self::LoadForWork { .. } => "asking your sources".into(),
            Self::Probe { .. } => "checking the server".into(),
            Self::AddServer { .. } => "adding the server".into(),
            Self::Recheck => "looking at your servers again".into(),
            Self::SearchSourceForWork { .. } => "searching the source".into(),
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
        /// `None` when the server did not say for sure: the line then names only the kind.
        series: Option<u32>,
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
        /// "Not found in": the sources searched without a match, in the person's order.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        missing: Vec<EditionMiss>,
    },
    /// What one source found for "Search there". `failure` when it could not be asked.
    SourceSeries {
        work: WorkId,
        source_id: String,
        query: String,
        choices: Vec<SourceSeriesChoice>,
        failure: Option<ReadingFailure>,
    },
}

/// A source that was searched for a work and has no series for it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EditionMiss {
    pub source_id: String,
    pub source_name: String,
    /// Why: "No series with this name", "Not answering since 10:12".
    pub reason: Msg,
}

/// One of a source's series, offered for "Search there".
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SourceSeriesChoice {
    pub series_ref: String,
    pub title: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub year: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub language: Option<String>,
    /// How close its title is to the work's, 0 to 100.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub confidence: u8,
}
