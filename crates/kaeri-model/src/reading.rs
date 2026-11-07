//! Reading manga and novels: the catalogue, a series and its chapters, the library, updates,
//! trackers, downloads, sending to an e-reader and the desktop feed.
//!
//! A work is named by a [`WorkId`] (an AniList id, or a local key for one AniList does not list)
//! and a chapter by a [`ChapterId`], both strings across the bindings. The chapter's label is the
//! source's own words; its canonical number comes beside it. "Edition" here is a source's copy of a
//! series: one source, one language, one version label. The build's edition is
//! [`crate::medium::AppEdition`]; the two never share a name.

use kaeri_core::{
    books::{ChapterId, WorkId},
    ids::AnilistId,
    media::Medium,
};

use crate::{
    Msg, ReadingMsg,
    eink::{EinkProfile, ScreenFacts},
    jobs::JobId,
    stores::Secret,
};

// ---------------------------------------------------------------------------------------------
// Tiles and series

/// A work's names, as its page shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct WorkTitles {
    /// The name in the person's chosen title language.
    pub display: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub romaji: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub english: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub native: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub synonyms: Vec<String>,
}

/// A cover anywhere: Home, Library, Search, Updates.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct WorkTile {
    pub work: WorkId,
    pub title: String,
    pub medium: Medium,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover: Option<String>,
    /// "Webtoon", "Light novel".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub format_label: Option<String>,
    /// The track under the cover, from 0 to 1.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub progress: Option<f32>,
    /// "Chapter 44", "Usually Fridays".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next: Option<Msg>,
    /// A chapter came out that is not read: the dot, always with words.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub new_chapter: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub adult: bool,
    /// The words under the cover when [`Self::next`] is `None`, joined with " · ": "Manhwa",
    /// "Releasing", "9 volumes", "On your Planning list". [`Self::format_label`] is the format's
    /// key (`manga`, `webtoon`, `one-shot`, `novel`), for an icon or a filter.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub facts: Vec<Msg>,
}

/// One series' page.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SeriesView {
    pub work: WorkId,
    pub medium: Medium,
    pub titles: WorkTitles,
    pub action: SeriesAction,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub banner: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub format: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub origin: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub status: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub synopsis: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub genres: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub authors: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chapters_total: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub volumes_total: Option<u32>,
    /// The source in use: "Reading from … · Change".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub edition: Option<BookEditionRow>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub list: Option<LibrarySegmentRead>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub score: Option<f32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub tracking: Vec<TrackerReadRow>,
    /// "Also a show", only to someone who both watches and reads.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub other_medium: Option<CrossMediumRow>,
    /// After Send to e-reader: the page asks where you stopped.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub ask_where_stopped: Option<SentChapters>,
    /// "Usually Fridays", "About every 2 weeks".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rhythm: Option<Msg>,
    /// The facts under the title, in order: the kind ("Manhwa"), the status, the count of
    /// chapters or volumes. [`Self::format`], [`Self::origin`] and [`Self::status`] are keys:
    /// `webtoon`, `KR`, `releasing`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub facts: Vec<Msg>,
    /// The year it began.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub year: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub adult: bool,
    /// More like this: what people who liked it rated highest.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub recommended: Vec<WorkTile>,
    /// The series page is still waiting for the catalogue: the header is from what was kept.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub partial: bool,
}

/// The series page's main button.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SeriesAction {
    /// "Read chapter 88".
    Read { chapter: ChapterId, label: String },
    /// "Continue chapter 41 · page 8". The page counts from 1, as every page the engine sends.
    Continue { chapter: ChapterId, label: String, page: u32 },
    /// A light novel's part, part read: "Continue part 3 · 34%", or "Continue part 3" when how
    /// far is not known or rounds to nothing.
    ContinuePart { chapter: ChapterId, label: String, percent: Option<u8> },
    /// A light novel's part not opened yet: "Read part 3".
    ReadPart { chapter: ChapterId, label: String },
    /// Finished.
    ReadAgain,
    /// None of your sources has it.
    NoSource,
}

/// A source's copy of a series: one source, one language, one version label.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BookEditionRow {
    pub source_id: String,
    pub source_name: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub language: Option<String>,
    /// A group or translation name, as the source gives it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub version_label: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chapters: u32,
    /// The newest chapter's label.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub latest: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub preferred: bool,
    /// The series reads from this one now: the person's choice, else the one picked for them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub in_use: bool,
}

/// A volume's chapters, or the chapters in no volume.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct VolumeGroup {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub volume: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chapters: Vec<ChapterRow>,
    /// Every chapter in it is read: folded into one line.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub all_read: bool,
}

/// One chapter in a list.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ChapterRow {
    pub id: ChapterId,
    /// The source's own words: "Chapter 12", "Episode 63".
    pub label: String,
    pub state: ChapterReadState,
    /// The canonical number: "10.5".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub number: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub volume: Option<String>,
    /// Unix seconds.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub published: Option<i64>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub pages: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub new_chapter: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub download: Option<ChapterDownloadState>,
    /// More than one: the row's menu lists them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub versions: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub bookmarked: bool,
    /// The group or translation of the version listed, as the source gives it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub version_label: Option<String>,
    /// When it was read, Unix seconds.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub read_at: Option<i64>,
    /// The second line in words, as parts the row joins with " · ": "Read · 4 days ago ·
    /// Downloaded", "New · yesterday", "Not downloaded". Never empty on a series page,
    /// in Updates or in Downloads.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub line: Vec<Msg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ChapterReadState {
    Unread,
    /// The page counts from 1, as every page the engine sends.
    InProgress {
        page: u32,
        of: Option<u32>,
    },
    Read,
}

/// One chapter as one edition has it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ChapterVersion {
    pub edition: BookEditionRow,
    pub chapter_ref: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub published: Option<i64>,
}

/// The same story in the other medium.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CrossMediumRow {
    pub work: WorkId,
    pub medium: Medium,
    pub title: String,
    pub relation: Msg,
}

/// What was sent to an e-reader, so the series page can ask where you stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SentChapters {
    pub target: String,
    /// Unix seconds.
    pub sent_at: i64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chapters: Vec<ChapterId>,
}

// ---------------------------------------------------------------------------------------------
// Trackers

/// One tracker's row on a series page.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TrackerReadRow {
    pub tracker: String,
    pub chapters: u32,
    /// `None` on Kitsu.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub volumes: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub status: Option<LibrarySegmentRead>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub score: Option<f32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub divergent: bool,
    pub mapping: TrackerMapping,
    /// "Chapter 10.5 goes to trackers as 10".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub split_note: Option<Msg>,
}

/// What taking a series off one tracker's list came to.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ListRemoval {
    pub tracker: String,
    pub outcome: ListRemovalOutcome,
    /// In words, as parts the app joins with " · ": "Removed from AniList", "Still on your
    /// Kitsu list · You're offline. ...".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub line: Vec<Msg>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ListRemovalOutcome {
    Removed,
    /// The list did not hold it.
    NotOnList,
    /// Which entry it is on that tracker was never learned, so nothing was asked.
    Unmatched,
    Failed {
        failure: ReadingFailure,
    },
}

/// How a work is matched on a tracker.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TrackerMapping {
    #[default]
    Mapped,
    Looking,
    Ask {
        candidates: Vec<MapCandidate>,
    },
    /// Not on this tracker.
    Unmapped,
}

/// A tracker entry a work may be. `tracker_id` is a string so MangaUpdates' 64-bit ids cross as
/// they are.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MapCandidate {
    pub tracker_id: String,
    pub title: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub year: Option<u16>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub confidence: u8,
}

/// A reading list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum LibrarySegmentRead {
    Reading,
    Planning,
    Completed,
    Paused,
    Dropped,
    Rereading,
    /// Local files with no AniList match.
    YourFiles,
}

impl LibrarySegmentRead {
    /// The name the store and the `read_status` register keep.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reading => "reading",
            Self::Planning => "planning",
            Self::Completed => "completed",
            Self::Paused => "paused",
            Self::Dropped => "dropped",
            Self::Rereading => "rereading",
            Self::YourFiles => "your_files",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "reading" => Self::Reading,
            "planning" => Self::Planning,
            "completed" => Self::Completed,
            "paused" => Self::Paused,
            "dropped" => Self::Dropped,
            "rereading" => Self::Rereading,
            "your_files" => Self::YourFiles,
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Home, updates, library, search

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReadingHomeView {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub spotlight: Option<WorkTile>,
    /// At most 12.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub continue_reading: Vec<WorkTile>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub this_week: Vec<WorkTile>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub up_next: Vec<WorkTile>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub releasing: Vec<WorkTile>,
    /// "Because you finished …": the finished work's title and what it led to.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub because_title: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub because: Vec<WorkTile>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub genre: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub genre_shelf: Vec<WorkTile>,
    /// No book source is set up yet.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub no_sources: bool,
    /// Why [`Self::spotlight`] is there, with the words around it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub spot: Option<ReadSpotlight>,
    /// This week by day, today first: [`Self::this_week`] grouped, with when each is due.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub week: Vec<ReadWeekDay>,
    /// The parts still on their way: their shelves show as loading.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub pending: Vec<ReadHomePart>,
    /// Shown from what was kept, because AniList could not be reached.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub offline: bool,
    /// When what is shown was kept, Unix seconds, while [`Self::offline`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub offline_since: Option<i64>,
    /// A part failed and nothing was kept of it: "Couldn't load. Try again."
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub failed: bool,
    /// What the end of Home says, when it says something.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub end: Option<ReadHomeEnd>,
}

/// The series at the top of reading's Home, and why it is there. The first that applies wins:
/// a new chapter in a series you follow, the chapter you stopped in, the first of Up next, and
/// with none of those, the most popular series still coming out (said as such, never as a pick).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReadSpotlight {
    pub case: ReadSpotlightCase,
    /// The words above the title: "New chapter in a series you follow".
    pub label: Msg,
    /// Under the title: the kind, the status, the count, as [`WorkTile::facts`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub facts: Vec<Msg>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub synopsis: Option<String>,
    /// "Chapter 88 is out today", "Chapter 13 · page 9".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub line: Option<Msg>,
    /// The main button. `None`: Add to Planning, which [`Self::planned`] says is done.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub action: Option<SeriesAction>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub planned: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub banner: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReadSpotlightCase {
    NewChapter,
    Continuing,
    UpNext,
    Popular,
}

/// One day of This week.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReadWeekDay {
    /// The day's start, Unix seconds, local time.
    pub day: i64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub today: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub entries: Vec<ReadWeekEntry>,
}

/// A series due on a day of This week. Book sources give no release times ahead, so the day is
/// the one its chapters usually come out on: the tile's `next` says "Usually Fridays".
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReadWeekEntry {
    pub tile: WorkTile,
    /// When the newest chapter came out, when it was this day: "out today".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub out_at: Option<i64>,
    /// Expected from the series' rhythm, not seen yet.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub usually: bool,
    /// The chapter that came out this day, in words: "Chapter 15", or "Chapter 15 · The Far
    /// Shore" with its title. For one only expected, the chapter the series' rhythm and its
    /// newest chapter say comes next, and `None` when that is not a confident guess. A light
    /// novel's says "Part 6". The tile's `next` stays the person's place.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chapter: Option<Msg>,
}

/// A part of reading's Home that loads on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReadHomePart {
    /// Continue, This week and Up next, from this device's reading and the catalogue.
    Yours,
    /// Releasing now and the genre shelf.
    Discovery,
    /// Because you finished.
    Because,
}

/// The end of reading's Home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReadHomeEnd {
    /// Every shelf is shown.
    Everything,
    /// No book source is set up: the end offers Sources.
    NoSources,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct UpdateRow {
    pub tile: WorkTile,
    pub chapter: ChapterRow,
    pub source_name: String,
}

/// One day of Updates.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct UpdateDay {
    /// The day's start, Unix seconds, local time.
    pub day: i64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rows: Vec<UpdateRow>,
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct UpdatesView {
    /// Newest first.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub days: Vec<UpdateDay>,
    /// Unix seconds.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub checked_at: Option<i64>,
    /// The series whose source did not answer on the last check, each said once.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub failures: Vec<UpdateFailure>,
    /// A check is running.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub checking: bool,
    /// Chapters in Updates not read yet: the tab's count.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub unread: u32,
}

/// A series the check could not ask about.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct UpdateFailure {
    pub work: WorkId,
    pub title: String,
    pub source_name: String,
    pub failure: ReadingFailure,
}

/// Reading's own search: the shows' query keeps its shape.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReadSearchQuery {
    pub text: String,
    /// `None`: manga and novels.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub medium: Option<Medium>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub formats: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub origins: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub status: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub genres: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub exclude_genres: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub sort: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Downloads

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum DownloadPick {
    NextUnread { n: u32 },
    Unread,
    Volume { volume: String },
    Chapters { ids: Vec<ChapterId> },
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ChapterDownloadState {
    Queued,
    WaitsForWifi,
    Downloading { done: u32, total: u32 },
    Done,
    Failed { failure: ReadingFailure },
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BookDownloadRow {
    pub work: WorkId,
    pub title: String,
    pub chapter: ChapterRow,
    pub state: ChapterDownloadState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StorageMeter {
    /// The bytes of every downloaded chapter, for the storage summary beside the shows'.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub used: u64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub free: u64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chapters: u32,
    /// Of `used`, what chapters already read take up.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub read_bytes: u64,
}

/// One series on Downloads: what it has here and how it stands.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BookDownloadSeries {
    pub work: WorkId,
    pub title: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover: Option<String>,
    /// Chapters downloaded.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chapters: u32,
    /// What its files take up, partly downloaded ones included.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub bytes: u64,
    /// Downloaded chapters already read and not bookmarked: what Delete read clears.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub read: u32,
    /// Chapters queued, waiting or downloading.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub queued: u32,
    /// Chapters that failed; Retry asks for them again.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub failed: Vec<ChapterId>,
    /// "6 chapters · 214 MB · 2 read", "Chapter 42 · 12 of 19 pages · 2 more queued", or the
    /// failure and its cause. Parts joined with " · ".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub line: Vec<Msg>,
    /// While a chapter downloads, how far: 0 to 1, for the ring.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub progress: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BookDownloadsView {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rows: Vec<BookDownloadRow>,
    pub meter: StorageMeter,
    /// The rows by series, the one downloading first, then by when each last changed.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub series: Vec<BookDownloadSeries>,
    /// The Reading section's total: "38 chapters · 1.1 GB". Empty with nothing downloaded.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub total: Vec<Msg>,
}

/// What downloading a choice of chapters will take: "About 62 MB · over Wi-Fi".
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct DownloadEstimate {
    pub work: WorkId,
    pub pick: DownloadPick,
    /// The chosen chapters not downloaded yet.
    pub chapters: u32,
    pub bytes: u64,
    /// "About 214 MB · over Wi-Fi", as parts the app joins with " · ". Empty when nothing is
    /// left to download.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub line: Vec<Msg>,
}

/// A chapter the background check found, for the chapters notification channel.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NewChapter {
    pub work: WorkId,
    pub title: String,
    pub chapter: ChapterId,
    pub label: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover: Option<String>,
    /// For the notification's expanded picture.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub banner: Option<String>,
    /// When the source dates it, Unix seconds.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub published: Option<i64>,
    /// The chapter the person is up to, when that is before this one: how far behind they are,
    /// and the one Read opens.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_for_you: Option<ChapterId>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub source_name: Option<String>,
    /// The chapter's own title.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chapter_title: Option<String>,
}

/// What reading's search filters choose from. Fixed, so the screen shows them before anything
/// loads: see [`read_filters`].
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReadFilters {
    /// [`ReadSearchQuery::formats`]: `manga`, `webtoon`, `one-shot`, `novel`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub formats: Vec<ReadFilterChoice>,
    /// [`ReadSearchQuery::origins`]: `JP`, `KR`, `CN`, `TW`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub origins: Vec<ReadFilterChoice>,
    /// [`ReadSearchQuery::status`]: `releasing`, `finished`, `hiatus`, `cancelled`, `not-yet`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub statuses: Vec<ReadFilterChoice>,
    /// [`ReadSearchQuery::sort`], AniList's names, the first the default with no text:
    /// `POPULARITY_DESC`, `TRENDING_DESC`, `SCORE_DESC`, `START_DATE_DESC`, `TITLE_ROMAJI`. The
    /// apps word them as the shows' search does.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub sorts: Vec<String>,
    /// AniList's genres, in its own words, the adult one left out.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub genres: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReadFilterChoice {
    pub key: String,
    pub label: Msg,
}

// ---------------------------------------------------------------------------------------------
// Send to e-reader and the feed

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ExportFormat {
    FixedEpub,
    Kepub,
    Epub,
    Cbz,
}

/// An e-reader to prepare files for: its screen and what it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ExportTarget {
    pub id: String,
    pub name: String,
    pub width_px: u32,
    pub height_px: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub colour: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub formats: Vec<ExportFormat>,
    /// What this target is offered for a manga or a comic: the formats the engine will write
    /// for pages. A format outside the list is refused, so the share sheet offers these only.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub manga_formats: Vec<ExportFormat>,
    /// The same for a novel.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub novel_formats: Vec<ExportFormat>,
    /// Files over the device's limit are written in parts, near 195 MB each.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub split: bool,
    /// The target last sent to on this device.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chosen: bool,
}

/// A prepared file, ready for the share sheet. Large exports come in parts near 195 MB.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ExportFile {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub part: Option<u32>,
}

/// The local OPDS feed, on desktop builds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FeedView {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub on: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub addresses: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub port: u16,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub password_set: bool,
    /// Books the feed serves.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub serving: u32,
    /// The password e-readers ask for, for the pane to show and copy. It is a secret: its
    /// debug form is redacted, and no front end writes it to a log.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub password: Option<crate::stores::Secret>,
    /// What the feed can share, each with whether it does and how much is on that shelf.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub shares: Vec<FeedShare>,
    /// The e-readers that fetched from the feed since it started, the latest first.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub clients: Vec<FeedClient>,
    /// Why the feed is not listening although it is on, when it is not.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub pause: Option<FeedPause>,
}

/// Why a feed that is on does not listen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum FeedPause {
    /// The Mac is on no local network.
    NoNetwork,
    /// The only route is a VPN, so the feed would be reachable from outside the home.
    VpnOnly,
}

/// A shelf the feed may share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum FeedShelf {
    /// The chapters downloaded on this computer.
    Downloads,
    /// The series in the manga and comic folders.
    MangaFolders,
    /// The series in the novel folders.
    NovelFolders,
}

/// One shelf of the feed: shared or not, and what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FeedShare {
    pub shelf: FeedShelf,
    pub on: bool,
    /// Chapters on the downloads shelf, series on the folder shelves.
    pub count: u32,
}

/// An e-reader that fetched from the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FeedClient {
    /// What the e-reader called itself, from its user agent; empty when it did not.
    pub name: String,
    /// Where it asked from on the network.
    pub address: String,
    /// Unix seconds of its last request.
    pub last: i64,
    /// Files it has fetched.
    pub files: u32,
}

// ---------------------------------------------------------------------------------------------
// Failures

/// Why something about reading failed. Each has its words in [`ReadingFailure::msg`]; the apps
/// word nothing themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReadingFailure {
    Offline,
    SourceDown {
        source: String,
    },
    /// The source no longer has it.
    NotFound,
    Blocked {
        source: String,
        reason: String,
    },
    Parse {
        source: String,
    },
    PageFetch {
        status: u16,
    },
    /// A server or a tracker.
    SignInNeeded {
        what: String,
    },
    /// A folder or drive that is gone.
    FileMissing {
        path: String,
    },
    Unreadable {
        path: String,
        format: String,
    },
    Unsupported {
        extension: String,
    },
    /// DRM or an encrypted PDF.
    Protected,
    DiskFull,
    /// Below 83.
    WebViewTooOld {
        version: u32,
    },
    /// A book plugin for a newer ABI.
    NeedsNewerKaeri {
        world: String,
    },
    /// A reading command while reading is not available.
    ReadingOff,
    Other {
        reason: String,
    },
}

impl ReadingFailure {
    pub fn msg(&self) -> Msg {
        Msg::Reading(match self {
            Self::Offline => ReadingMsg::ReadingFailedOffline,
            Self::SourceDown { source } => {
                ReadingMsg::ReadingFailedSourceDown { source: source.clone() }
            }
            Self::NotFound => ReadingMsg::ReadingFailedNotFound,
            Self::Blocked { source, reason } => ReadingMsg::ReadingFailedBlocked {
                source: source.clone(),
                reason: reason.clone(),
            },
            Self::Parse { source } => ReadingMsg::ReadingFailedParse { source: source.clone() },
            Self::PageFetch { status } => {
                ReadingMsg::ReadingFailedPage { status: i64::from(*status) }
            }
            Self::SignInNeeded { what } => {
                ReadingMsg::ReadingFailedSignIn { what: what.clone() }
            }
            Self::FileMissing { path } => {
                ReadingMsg::ReadingFailedFileMissing { path: path.clone() }
            }
            Self::Unreadable { path, format } => ReadingMsg::ReadingFailedUnreadable {
                path: path.clone(),
                format: format.clone(),
            },
            Self::Unsupported { extension } => {
                ReadingMsg::ReadingFailedUnsupported { kind: extension.clone() }
            }
            Self::Protected => ReadingMsg::ReadingFailedProtected,
            Self::DiskFull => ReadingMsg::ReadingFailedDiskFull,
            Self::WebViewTooOld { version } => {
                ReadingMsg::ReadingFailedWebview { version: i64::from(*version) }
            }
            Self::NeedsNewerKaeri { world } => {
                ReadingMsg::ReadingFailedNeedsNewer { world: world.clone() }
            }
            Self::ReadingOff => ReadingMsg::ReadingFailedOff,
            Self::Other { reason } => ReadingMsg::ReadingFailedOther { reason: reason.clone() },
        })
    }
}

/// Which part of which screen a failure belongs to, so it shows in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReadingAbout {
    Home,
    Updates,
    Library,
    Search,
    Series,
    Chapters,
    Downloads,
    Export,
    Feed,
    Tracking,
    Eink,
    /// The reader, for a reader command refused before any session opened.
    Reader,
    /// Folders, servers and book plugins.
    Sources,
}

// ---------------------------------------------------------------------------------------------
// Words

/// The weekday chapters usually come out on, from Monday as 0, or a span in weeks.
pub fn rhythm(weekday: Option<u8>, every_weeks: u32) -> Option<Msg> {
    let msg = match (weekday, every_weeks) {
        (Some(day), 0 | 1) => match day {
            0 => ReadingMsg::ReadingUsuallyMondays,
            1 => ReadingMsg::ReadingUsuallyTuesdays,
            2 => ReadingMsg::ReadingUsuallyWednesdays,
            3 => ReadingMsg::ReadingUsuallyThursdays,
            4 => ReadingMsg::ReadingUsuallyFridays,
            5 => ReadingMsg::ReadingUsuallySaturdays,
            6 => ReadingMsg::ReadingUsuallySundays,
            _ => return None,
        },
        (_, 0) => return None,
        (_, weeks) => ReadingMsg::ReadingAboutEveryWeeks { weeks: i64::from(weeks) },
    };
    Some(Msg::Reading(msg))
}

/// "Chapter 44", under a cover.
pub fn next_chapter(number: &str) -> Msg {
    Msg::Reading(ReadingMsg::ReadingNextChapter { chapter: number.to_owned() })
}

/// What kind of book it is, in words: from its format key (`manga`, `webtoon`, `one-shot`,
/// `novel`), its medium and the country it is from (`JP`, `KR`).
pub fn kind_words(format: Option<&str>, medium: Medium, origin: Option<&str>) -> Msg {
    let msg = match (format, medium, origin) {
        (Some("webtoon"), ..) => ReadingMsg::ReadingKindWebtoon,
        (Some("one-shot"), ..) => ReadingMsg::ReadingKindOneShot,
        (_, Medium::Novel, Some("JP") | None) => ReadingMsg::ReadingKindLightNovel,
        (_, Medium::Novel, _) => ReadingMsg::ReadingKindNovel,
        (_, _, Some("KR")) => ReadingMsg::ReadingKindManhwa,
        (_, _, Some("CN" | "TW")) => ReadingMsg::ReadingKindManhua,
        _ => ReadingMsg::ReadingKindManga,
    };
    Msg::Reading(msg)
}

/// A status key (`releasing`, `finished`, `hiatus`, `cancelled`, `not-yet`) in words.
pub fn status_words(status: &str) -> Option<Msg> {
    Some(Msg::Reading(match status {
        "releasing" => ReadingMsg::ReadingStatusReleasing,
        "finished" => ReadingMsg::ReadingStatusFinished,
        "hiatus" => ReadingMsg::ReadingStatusHiatus,
        "cancelled" => ReadingMsg::ReadingStatusCancelled,
        "not-yet" => ReadingMsg::ReadingStatusNotYet,
        _ => return None,
    }))
}

/// "48 chapters", or for a novel, which counts volumes, "9 volumes".
pub fn count_words(medium: Medium, chapters: Option<u32>, volumes: Option<u32>) -> Option<Msg> {
    let msg = match (medium, chapters, volumes) {
        (Medium::Novel, _, Some(v)) | (_, None, Some(v)) => {
            ReadingMsg::ReadingVolumes { count: i64::from(v) }
        }
        (_, Some(c), _) => ReadingMsg::ReadingChapters { count: i64::from(c) },
        _ => return None,
    };
    Some(Msg::Reading(msg))
}

/// A page as people count it, from a place as kept: pages count from 0 in the store and from 1
/// everywhere the engine sends them. The one place the engine turns one into the other.
pub fn shown_page(kept: u32) -> u32 {
    kept.saturating_add(1)
}

/// "Chapter 13 · page 9", from the place as kept.
pub fn chapter_page(chapter: &str, page: u32) -> Msg {
    Msg::Reading(ReadingMsg::ReadingChapterPage {
        chapter: chapter.to_owned(),
        page: i64::from(shown_page(page)),
    })
}

/// How far into a light novel's part, as a percent worth saying: none when it is not known or
/// rounds to nothing, so nothing ever says "0%".
pub fn part_percent(page: Option<u32>, of: Option<u32>) -> Option<u8> {
    match (page, of) {
        (Some(page), Some(of)) if of > 0 => {
            let percent = (u64::from(page) * 100 / u64::from(of)).min(100) as u8;
            (percent > 0).then_some(percent)
        }
        _ => None,
    }
}

/// "Part 3 · 34%", or "Part 3" with no percent worth saying.
pub fn part_place(part: &str, percent: Option<u8>) -> Msg {
    Msg::Reading(match percent {
        Some(percent) => ReadingMsg::ReadingPartPercent {
            part: part.to_owned(),
            percent: i64::from(percent),
        },
        None => ReadingMsg::ReadingNextPart { part: part.to_owned() },
    })
}

/// "Chapter 44", or "Part 4" for a light novel, which reads in parts.
pub fn next_unit(medium: Medium, number: &str) -> Msg {
    match medium {
        Medium::Novel => Msg::Reading(ReadingMsg::ReadingNextPart { part: number.to_owned() }),
        _ => next_chapter(number),
    }
}

/// "Chapter 88 is new", or "Part 4 is new".
pub fn unit_new(medium: Medium, number: &str) -> Msg {
    let number = number.to_owned();
    Msg::Reading(match medium {
        Medium::Novel => ReadingMsg::ReadingPartNew { part: number },
        _ => ReadingMsg::ReadingChapterNew { chapter: number },
    })
}

/// AniList's genres for books, in its own words, without the adult one.
pub const READ_GENRES: [&str; 18] = [
    "Action",
    "Adventure",
    "Comedy",
    "Drama",
    "Ecchi",
    "Fantasy",
    "Horror",
    "Mahou Shoujo",
    "Mecha",
    "Music",
    "Mystery",
    "Psychological",
    "Romance",
    "Sci-Fi",
    "Slice of Life",
    "Sports",
    "Supernatural",
    "Thriller",
];

/// The sorts reading's search offers, in AniList's names; the first is the default.
pub const READ_SORTS: [&str; 5] =
    ["POPULARITY_DESC", "TRENDING_DESC", "SCORE_DESC", "START_DATE_DESC", "TITLE_ROMAJI"];

/// What reading's search filters choose from.
pub fn read_filters() -> ReadFilters {
    let choice = |key: &str, msg: ReadingMsg| ReadFilterChoice {
        key: key.to_owned(),
        label: Msg::Reading(msg),
    };
    ReadFilters {
        formats: vec![
            choice("manga", ReadingMsg::ReadingKindManga),
            choice("webtoon", ReadingMsg::ReadingKindWebtoon),
            choice("one-shot", ReadingMsg::ReadingKindOneShot),
            choice("novel", ReadingMsg::ReadingKindLightNovel),
        ],
        origins: vec![
            choice("JP", ReadingMsg::ReadingOriginJapan),
            choice("KR", ReadingMsg::ReadingOriginKorea),
            choice("CN", ReadingMsg::ReadingOriginChina),
            choice("TW", ReadingMsg::ReadingOriginTaiwan),
        ],
        statuses: ["releasing", "finished", "hiatus", "cancelled", "not-yet"]
            .into_iter()
            .filter_map(|key| {
                Some(ReadFilterChoice { key: key.to_owned(), label: status_words(key)? })
            })
            .collect(),
        sorts: READ_SORTS.iter().map(|s| (*s).to_owned()).collect(),
        genres: READ_GENRES.iter().map(|g| (*g).to_owned()).collect(),
    }
}

// ---------------------------------------------------------------------------------------------
// Commands and events

/// The catalogue, a series, the library, updates, trackers, downloads, export and the feed. While
/// reading is not available every one is answered with [`ReadingFailure::ReadingOff`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReadingCommand {
    /// [`ReadingEvent::Home`] at once from what was kept, and again as each part lands.
    LoadHome {
        refresh: bool,
    },
    LoadUpdates,
    /// Run the paced check of followed series now: [`ReadingEvent::Updates`] after.
    CheckUpdates,
    /// Queue every unread chapter in Updates.
    DownloadNew,
    LoadLibrary {
        segment: LibrarySegmentRead,
    },
    /// Carries the count in the other scope when both media are on.
    Search {
        query: ReadSearchQuery,
    },
    SearchMore {
        query: ReadSearchQuery,
        page: u32,
    },
    /// [`ReadingEvent::Series`], then [`ReadingEvent::Chapters`] once a source answers.
    LoadSeries {
        work: WorkId,
    },
    LoadChapters {
        work: WorkId,
        edition: Option<String>,
    },
    LoadVersions {
        work: WorkId,
        chapter: ChapterId,
    },
    /// Read marks stay.
    ChooseEdition {
        work: WorkId,
        source_id: String,
        version_label: Option<String>,
        language: Option<String>,
    },
    MarkRead {
        work: WorkId,
        chapters: Vec<ChapterId>,
        read: bool,
    },
    /// Every chapter up to this one is read: one adopt, not a mark per chapter.
    MarkReadUpTo {
        work: WorkId,
        chapter: ChapterId,
    },
    /// Fresh marks; the old ones are set aside and come back on undo.
    ReadAgain {
        work: WorkId,
    },
    /// Undo Read again.
    UndoReadAgain {
        work: WorkId,
    },
    Bookmark {
        work: WorkId,
        chapter: ChapterId,
        on: bool,
    },
    HideFromContinue {
        work: WorkId,
    },
    SetStatus {
        work: WorkId,
        status: LibrarySegmentRead,
    },
    SetScore {
        work: WorkId,
        score: f32,
    },
    LoadTracking {
        work: WorkId,
    },
    /// `tracker_id: None` means not on this tracker.
    ChooseMapping {
        work: WorkId,
        tracker: String,
        tracker_id: Option<String>,
    },
    ResolveConflict {
        work: WorkId,
        tracker: String,
        keep_local: bool,
    },
    /// Take the series off every signed-in tracker's list and off the person's lists here.
    /// Answered with [`ReadingEvent::RemovedFromLists`].
    RemoveFromLists {
        work: WorkId,
    },
    /// The password goes to the sign-in call once and is dropped. Answered with the tracker
    /// sign-in events shows use.
    ConnectMangaUpdates {
        username: String,
        password: Secret,
    },
    Download {
        work: WorkId,
        pick: DownloadPick,
    },
    DownloadCancel {
        work: WorkId,
        chapters: Vec<ChapterId>,
    },
    DownloadDelete {
        work: WorkId,
        chapters: Vec<ChapterId>,
    },
    /// Delete read: the downloads of a series' read chapters, bookmarked ones spared.
    DownloadDeleteRead {
        work: WorkId,
    },
    /// What downloading `pick` will take, from what is kept, without asking the source.
    /// Answered with [`ReadingEvent::DownloadEstimate`].
    EstimateDownload {
        work: WorkId,
        pick: DownloadPick,
    },
    LoadDownloads,
    LoadExportTargets,
    /// [`ReadingEvent::ExportProgress`], then [`ReadingEvent::ExportReady`].
    Prepare {
        work: WorkId,
        pick: DownloadPick,
        target: String,
        format: ExportFormat,
    },
    /// Stops a [`Self::Prepare`] that is running; the files it had written are removed.
    CancelPrepare {
        job: JobId,
    },
    /// The files were shared: the series page asks where you stopped.
    Shared {
        work: WorkId,
        chapters: Vec<ChapterId>,
        target: String,
    },
    /// The answer to where you stopped; `None` for didn't read.
    StoppedAt {
        work: WorkId,
        chapter: Option<ChapterId>,
        page: Option<u32>,
    },
    /// Keeps the e-reader the Send sheet starts on, on this device only.
    SetSendTarget {
        target: String,
    },
    LoadFeed,
    SetFeed {
        on: bool,
        password: Option<Secret>,
    },
    /// Shares a shelf of the feed or stops sharing it; answered with [`ReadingEvent::Feed`].
    SetFeedShare {
        shelf: FeedShelf,
        on: bool,
    },
    /// Android sends this before its first screen: [`ReadingEvent::Eink`].
    DeviceFacts {
        facts: ScreenFacts,
    },
    /// "Is this an e-ink screen?" and "Is this a colour screen?".
    AnswerEink {
        eink: Option<bool>,
        colour: Option<bool>,
    },
}

impl ReadingCommand {
    /// Where a failure in answer to this shows.
    pub const fn about(&self) -> ReadingAbout {
        match self {
            Self::LoadHome { .. } | Self::HideFromContinue { .. } => ReadingAbout::Home,
            Self::LoadUpdates | Self::CheckUpdates | Self::DownloadNew => ReadingAbout::Updates,
            Self::LoadLibrary { .. } => ReadingAbout::Library,
            Self::Search { .. } | Self::SearchMore { .. } => ReadingAbout::Search,
            Self::LoadSeries { .. }
            | Self::ChooseEdition { .. }
            | Self::ReadAgain { .. }
            | Self::UndoReadAgain { .. }
            | Self::SetStatus { .. }
            | Self::SetScore { .. }
            | Self::Shared { .. }
            | Self::StoppedAt { .. } => ReadingAbout::Series,
            Self::LoadChapters { .. }
            | Self::LoadVersions { .. }
            | Self::MarkRead { .. }
            | Self::MarkReadUpTo { .. }
            | Self::Bookmark { .. } => ReadingAbout::Chapters,
            Self::LoadTracking { .. }
            | Self::ChooseMapping { .. }
            | Self::ResolveConflict { .. }
            | Self::RemoveFromLists { .. }
            | Self::ConnectMangaUpdates { .. } => ReadingAbout::Tracking,
            Self::Download { .. }
            | Self::DownloadCancel { .. }
            | Self::DownloadDelete { .. }
            | Self::DownloadDeleteRead { .. }
            | Self::EstimateDownload { .. }
            | Self::LoadDownloads => ReadingAbout::Downloads,
            Self::LoadExportTargets
            | Self::Prepare { .. }
            | Self::CancelPrepare { .. }
            | Self::SetSendTarget { .. } => ReadingAbout::Export,
            Self::LoadFeed | Self::SetFeed { .. } | Self::SetFeedShare { .. } => {
                ReadingAbout::Feed
            }
            Self::DeviceFacts { .. } | Self::AnswerEink { .. } => ReadingAbout::Eink,
        }
    }

    /// What to call this in the activity row, or `None` for a write.
    pub fn activity(&self) -> Option<String> {
        Some(match self {
            Self::LoadHome { .. } => "loading reading home".into(),
            Self::LoadUpdates | Self::CheckUpdates => "checking for new chapters".into(),
            Self::LoadLibrary { .. } => "loading your reading list".into(),
            Self::Search { query } | Self::SearchMore { query, .. } => {
                format!("searching \u{201c}{}\u{201d}", query.text.trim())
            }
            Self::LoadSeries { .. } => "loading the series".into(),
            Self::LoadChapters { .. } | Self::LoadVersions { .. } => "loading chapters".into(),
            Self::LoadTracking { .. } => "loading tracking".into(),
            Self::ConnectMangaUpdates { .. } => "signing in to mangaupdates".into(),
            Self::Prepare { .. } => "preparing files".into(),
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReadingEvent {
    Home(Box<ReadingHomeView>),
    Updates(UpdatesView),
    Library {
        segment: LibrarySegmentRead,
        tiles: Vec<WorkTile>,
    },
    Search {
        query: ReadSearchQuery,
        page: u32,
        tiles: Vec<WorkTile>,
        has_more: bool,
        other_scope: Option<u32>,
        /// How many series AniList found in all, on the last page only: AniList gives no true
        /// count before it. `None` from the sources' own search.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        total: Option<u32>,
    },
    Series(Box<SeriesView>),
    Chapters {
        work: WorkId,
        edition: Option<BookEditionRow>,
        volumes: Vec<VolumeGroup>,
        failure: Option<ReadingFailure>,
    },
    Versions {
        work: WorkId,
        chapter: ChapterId,
        versions: Vec<ChapterVersion>,
    },
    Tracking {
        work: WorkId,
        rows: Vec<TrackerReadRow>,
    },
    /// One result per signed-in tracker that keeps a reading list.
    RemovedFromLists {
        work: WorkId,
        results: Vec<ListRemoval>,
    },
    Downloads(BookDownloadsView),
    DownloadEstimate(DownloadEstimate),
    NewChapters(Vec<NewChapter>),
    /// The chapters the person asked to download are all done, for the Downloads finished
    /// notification: sent only while it is on. `work` is the series when they were all of one.
    DownloadsDone {
        work: Option<WorkId>,
        title: Msg,
        body: Msg,
    },
    ExportTargets(Vec<ExportTarget>),
    ExportProgress {
        job: JobId,
        done: u32,
        total: u32,
        /// Chapters gathered so far, of `chapters`: what the screen says as "3 of 12". Zero
        /// `chapters` means the send does not know yet.
        chapters_done: u32,
        chapters: u32,
    },
    /// A send failed. `chapter` is the chapter it could not prepare, when it was one.
    ExportFailed {
        job: JobId,
        chapter: Option<String>,
        failure: ReadingFailure,
    },
    ExportReady {
        job: JobId,
        files: Vec<ExportFile>,
    },
    /// A send waits for Wi-Fi: it has chapters to fetch and downloads are kept to Wi-Fi. It
    /// goes on by itself once the network allows, and the next [`Self::ExportProgress`] says
    /// so; `CancelPrepare` drops it. A send left waiting when the engine stopped is started
    /// again at the next launch as a job of its own, so `job` may be one the screen never sent.
    /// `line` says why, in words.
    ExportWaiting {
        job: JobId,
        work: WorkId,
        target: String,
        line: Msg,
    },
    Feed(FeedView),
    Eink(EinkProfile),
    Failed {
        about: ReadingAbout,
        failure: ReadingFailure,
    },
}

/// The AniList id behind a work, when it has one.
pub fn anilist_of(work: &WorkId) -> Option<AnilistId> {
    match work.0.parse::<kaeri_core::books::WorkKey>().ok()? {
        kaeri_core::books::WorkKey::Anilist(id) => Some(id),
        kaeri_core::books::WorkKey::Local(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_says_its_percent_only_when_there_is_one_worth_saying() {
        assert_eq!(part_percent(Some(16), Some(47)), Some(34));
        assert_eq!(part_percent(Some(0), Some(47)), None, "never 0%");
        assert_eq!(part_percent(None, None), None);
        assert_eq!(part_percent(Some(3), Some(0)), None);
        assert_eq!(part_place("3", Some(34)).english(), "Part 3 · 34%");
        assert_eq!(part_place("3", None).english(), "Part 3");
        assert_eq!(shown_page(8), 9);
        assert_eq!(chapter_page("13", 9).english(), "Chapter 13 · page 10");
    }

    #[test]
    fn every_failure_has_its_words() {
        let failures = [
            ReadingFailure::Offline,
            ReadingFailure::SourceDown { source: "Shelf".into() },
            ReadingFailure::NotFound,
            ReadingFailure::Blocked {
                source: "Shelf".into(),
                reason: "Too many requests.".into(),
            },
            ReadingFailure::Parse { source: "Shelf".into() },
            ReadingFailure::PageFetch { status: 404 },
            ReadingFailure::SignInNeeded { what: "Komga".into() },
            ReadingFailure::FileMissing { path: "/books/a.cbz".into() },
            ReadingFailure::Unreadable { path: "/books/a.cbz".into(), format: "CBZ".into() },
            ReadingFailure::Unsupported { extension: ".djvu".into() },
            ReadingFailure::Protected,
            ReadingFailure::DiskFull,
            ReadingFailure::WebViewTooOld { version: 80 },
            ReadingFailure::NeedsNewerKaeri { world: "kaeri:books@0.2".into() },
            ReadingFailure::ReadingOff,
            ReadingFailure::Other { reason: "It broke.".into() },
        ];
        for failure in failures {
            let text = failure.msg().english();
            assert!(!text.is_empty() && !text.contains('{'), "{failure:?}: {text}");
        }
        assert_eq!(
            ReadingFailure::PageFetch { status: 404 }.msg().english(),
            "This page didn't load (error 404). Try again."
        );
    }

    #[test]
    fn rhythm_names_a_weekday_or_a_span() {
        assert_eq!(rhythm(Some(4), 1).unwrap().english(), "Usually Fridays");
        assert_eq!(rhythm(None, 2).unwrap().english(), "About every 2 weeks");
        assert_eq!(rhythm(Some(9), 1), None);
        assert_eq!(rhythm(None, 0), None);
        assert_eq!(next_chapter("10.5").english(), "Chapter 10.5");
    }

    #[test]
    fn books_are_named_by_kind_status_and_count() {
        let kind = |f, m, o| kind_words(f, m, o).english();
        assert_eq!(kind(Some("manga"), Medium::Manga, Some("KR")), "Manhwa");
        assert_eq!(kind(Some("webtoon"), Medium::Manga, Some("KR")), "Webtoon");
        assert_eq!(kind(Some("manga"), Medium::Manga, Some("TW")), "Manhua");
        assert_eq!(kind(Some("novel"), Medium::Novel, Some("JP")), "Light novel");
        assert_eq!(kind(Some("novel"), Medium::Novel, Some("CN")), "Novel");
        assert_eq!(kind(None, Medium::Manga, None), "Manga");
        assert_eq!(status_words("hiatus").unwrap().english(), "On hiatus");
        assert_eq!(count_words(Medium::Novel, None, Some(9)).unwrap().english(), "9 volumes");
        assert_eq!(
            count_words(Medium::Manga, Some(1), Some(1)).unwrap().english(),
            "1 chapter"
        );
        assert_eq!(chapter_page("13", 8).english(), "Chapter 13 · page 9", "kept from 0");
        let filters = read_filters();
        assert_eq!(filters.statuses.len(), 5);
        assert!(!filters.genres.iter().any(|g| g == crate::search::ADULT_GENRE));
        assert_eq!(filters.sorts[0], "POPULARITY_DESC");
    }

    #[test]
    fn segments_round_trip_and_works_name_their_anilist_id() {
        for segment in [
            LibrarySegmentRead::Reading,
            LibrarySegmentRead::Planning,
            LibrarySegmentRead::Completed,
            LibrarySegmentRead::Paused,
            LibrarySegmentRead::Dropped,
            LibrarySegmentRead::Rereading,
            LibrarySegmentRead::YourFiles,
        ] {
            assert_eq!(LibrarySegmentRead::parse(segment.as_str()), Some(segment));
        }
        assert_eq!(anilist_of(&WorkId("a:30001".into())), Some(AnilistId(30001)));
        assert_eq!(anilist_of(&WorkId("l:00112233445566778899aabbccddeeff".into())), None);
    }
}
