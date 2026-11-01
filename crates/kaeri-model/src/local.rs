//! Your media: folders on this computer, the series found in them, and what each is matched to.
//!
//! The engine fills these in; the apps draw them. File names are matched to AniList folder first,
//! and a match below the confidence that is safe to act on waits in Needs a look until the viewer
//! confirms it. Nothing is written to a tracker from a match that has not been confirmed.

use kaeri_core::ids::AnilistId;

/// How far matching has gone: "Matching · 31 of 58 series".
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MatchProgress {
    pub done: u32,
    pub total: u32,
    /// AniList's limit is holding the next lookup back for this many seconds: "Waiting for
    /// AniList · 12 s". Sent once as the wait starts; the apps count it down.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub wait_seconds: Option<u32>,
}

/// What a folder's row line says while a scan runs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum FolderScan {
    /// Reading the folder: "Scanning · 1,204 files found".
    Scanning { files: u32 },
    /// Matching what was found to AniList.
    Matching { progress: MatchProgress },
}

/// A folder the viewer added.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FolderRow {
    pub path: String,
    /// Video files found in the last scan.
    pub files: u32,
    /// Whether the folder could be read at the last scan. A drive that is not plugged in is not.
    pub available: bool,
    /// The scan of this folder while it runs, `None` once it is done.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub scan: Option<FolderScan>,
    /// The summary once done: "214 episodes of 18 series · 3 need a look". Series counts the
    /// matched and kept groups, episodes their files that play.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub series: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episodes: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub needs_look: u32,
}

/// Where a group of files stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum GroupState {
    /// Matched with enough confidence, or confirmed by the viewer.
    Matched,
    /// A likely match that waits for the viewer to confirm it.
    NeedsLook,
    /// Nothing matched. The viewer can choose a match or keep the files unmatched.
    NotMatched,
    /// Kept unmatched: a title with its own metadata, never written to a tracker.
    Kept,
}

/// Why nothing was matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NotMatchedReason {
    /// AniList has nothing like the name.
    NoResult,
    /// AniList could not be asked just now. The group is tried again on the next scan.
    Offline,
}

/// The files of one series, as found.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LocalGroup {
    /// Stable across scans: the folder and the series' name in it.
    pub id: String,
    /// The name read from the folder or file names.
    pub name: String,
    pub files: u32,
    pub state: GroupState,
    /// The entry it is matched to, or the own-metadata id when kept.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub anilist: Option<AnilistId>,
    /// The matched entry's title, for the row.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub matched_title: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub reason: Option<NotMatchedReason>,
    /// The folder it was found in, one of the view's folders, so the folder page lists its own.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub folder: String,
    /// How sure the best guess is, in percent, for Needs a look: "71% match".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub confidence: Option<u8>,
}

/// A file whose name has no episode number, in a group whose other files have one. It waits in
/// Not matched, with "No episode number in the name" and Set episode, and does not play until
/// it is set.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct UnnumberedFile {
    /// The group it belongs to.
    pub group: String,
    /// The folder it was found in, one of the view's folders.
    pub folder: String,
    pub path: String,
    /// The file's name, for the row: "Making of.mkv".
    pub name: String,
    /// The folders between the group's and the file, for the row's line: "extras". Empty when
    /// the file sits in the group's own folder.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub place: String,
}

/// Everything Your media shows.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LocalView {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub folders: Vec<FolderRow>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub groups: Vec<LocalGroup>,
    /// A scan is running.
    pub scanning: bool,
    /// Files waiting for Set episode, in folder and file order.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub unnumbered: Vec<UnnumberedFile>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum LocalEvent {
    /// Your media, after any change and as a scan finds more.
    View(LocalView),
    /// The answer to [`LocalCommand::FindMatch`]: the titles Fix this match offers for `group`.
    /// `offline` when AniList could not be asked, so the list is empty for that reason.
    Matches { group: String, query: String, choices: Vec<MatchChoice>, offline: bool },
}

/// An AniList title Fix this match offers for a group: "Hoshikuzu no Tabibito Season 2 · TV ·
/// 2026 · 12 episodes · 71% match".
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MatchChoice {
    pub anilist: AnilistId,
    pub title: String,
    /// The format's label: "TV", "Movie".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub format: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub year: Option<u16>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episodes: Option<u32>,
    /// How well the name fits, in percent.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub confidence: u8,
    /// False when its episode count doesn't fit the group's files: the row is dimmed with "Has
    /// 24 episodes, this folder has 12", and can still be picked.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub fits: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum LocalCommand {
    /// Send the view.
    Load,
    /// Add a folder and scan it.
    AddFolder { path: String },
    /// Stop reading a folder. The files are not touched. What was found and decided in it is
    /// held for the Undo notice's 8 seconds.
    RemoveFolder { path: String },
    /// Undo `RemoveFolder` while the notice shows: the folder comes back with its matches and
    /// choices. After the notice it is added and scanned again.
    RestoreFolder { path: String },
    /// Scan every folder again.
    Scan,
    /// Fix this match: search AniList for a group's title. An empty `query` searches the name
    /// read from the files. Answered with [`LocalEvent::Matches`], which carries the query so an
    /// answer to an older search can be told apart. Lookups share AniList's limit.
    FindMatch { group: String, query: String },
    /// Confirm a match, or fix one, for a group. Chosen with the match picker.
    Choose { group: String, anilist: AnilistId },
    /// Keep a group's files as a title of their own.
    Keep { group: String },
    /// Set episode: which episode a file is, for a file whose name has no number. Season 0
    /// holds specials, as AniList and Jellyfin count them. A rescan keeps it.
    SetEpisode { group: String, path: String, season: u32, episode: u32 },
}
