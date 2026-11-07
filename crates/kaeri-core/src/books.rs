//! Books: the keys a work and its chapters are known by, chapter numbers, and what a book source
//! answers with.
//!
//! A work is one series in one medium, keyed by its AniList id once matched, or by a local key
//! while it is only on this device ([`WorkKey`]). Read marks hang on a [`ChapterKey`], which leaves
//! the source, the group and (for normally numbered works) the volume out, so marks survive a
//! change of source.
//!
//! The records below the keys mirror the `source` interface of `wit/kaeri-books.wit`, as
//! [`crate::traits::Provider`]'s mirror `kaeri:provider`: folders, servers and book plugins all
//! answer with them through [`crate::traits::BookSource`].

use std::{fmt, path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{
    ids::{AnilistId, KitsuMangaId, MalMangaId, MangaUpdatesId},
    media::{Medium, Origin, ReadingFormat},
};

// ---------------------------------------------------------------------------------------------
// Works

/// What a work is known by inside the engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WorkKey {
    /// Matched to AniList. AniList numbers anime and manga in one space, so the id alone names
    /// the work.
    Anilist(AnilistId),
    /// Not matched: a work from your files, kept on this device. 32 hex characters of a hash of
    /// the medium and the normalised series name ([`WorkKey::local`]). Never reaches a tracker
    /// and never writes a sync op.
    Local(String),
}

impl WorkKey {
    /// The key for an unmatched series called `name` in `medium`: the same name, give or take
    /// case, punctuation and spacing, gives the same key on every run.
    pub fn local(medium: Medium, name: &str) -> Self {
        let normal = normalise_name(name);
        let input = format!("{}\u{1f}{normal}", medium.as_str());
        Self::Local(hash128(&input))
    }

    /// The AniList id, for a matched work.
    pub const fn anilist(&self) -> Option<AnilistId> {
        match self {
            Self::Anilist(id) => Some(*id),
            Self::Local(_) => None,
        }
    }

    /// Whether the work is only on this device.
    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }

    /// The string form that crosses to the apps and sits in the store: `a:30001`, `l:<hex>`.
    pub fn id(&self) -> WorkId {
        WorkId(self.to_string())
    }
}

impl fmt::Display for WorkKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anilist(id) => write!(f, "a:{id}"),
            Self::Local(hex) => write!(f, "l:{hex}"),
        }
    }
}

/// Why a string is not a [`WorkKey`] or a [`ChapterKey`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a {what}: {text:?}")]
pub struct KeyError {
    what: &'static str,
    text: String,
}

impl FromStr for WorkKey {
    type Err = KeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || KeyError { what: "work id", text: s.to_owned() };
        match s.split_once(':') {
            Some(("a", id)) => {
                let id: u32 = id.parse().map_err(|_| bad())?;
                Ok(Self::Anilist(AnilistId::new(id)))
            }
            Some(("l", hex))
                if hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit()) =>
            {
                Ok(Self::Local(hex.to_ascii_lowercase()))
            }
            _ => Err(bad()),
        }
    }
}

/// A work as the apps hold it: [`WorkKey`]'s string, `a:30001` or `l:<hex>`. One hashable string
/// keeps the apps' maps and navigation plain.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkId(pub String);

impl WorkId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The key it spells, `None` for a string that is not one.
    pub fn key(&self) -> Option<WorkKey> {
        self.0.parse().ok()
    }
}

impl fmt::Display for WorkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<WorkKey> for WorkId {
    fn from(key: WorkKey) -> Self {
        key.id()
    }
}

impl From<AnilistId> for WorkId {
    fn from(id: AnilistId) -> Self {
        WorkKey::Anilist(id).id()
    }
}

#[cfg(feature = "ffi")]
uniffi::custom_newtype!(WorkId, String);

/// A series name as the local key hashes it: lowercase letters and digits, one space between
/// words, bracketed tags dropped.
pub fn normalise_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0u32;
    let mut space = false;
    for c in name.chars() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            c if c.is_alphanumeric() => {
                if space && !out.is_empty() {
                    out.push(' ');
                }
                space = false;
                out.extend(c.to_lowercase());
            }
            _ => space = true,
        }
    }
    out
}

/// 32 hex characters that stay the same from one release to the next: two FNV-1a passes, as
/// [`crate::hash::stable_hash`] makes one. Not cryptographic; a key, not a secret.
fn hash128(input: &str) -> String {
    let pass = |seed: u64| {
        let mut hash = seed;
        for byte in input.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    };
    format!("{:016x}{:016x}", pass(0xcbf2_9ce4_8422_2325), pass(0x6c62_272e_07bb_0142))
}

// ---------------------------------------------------------------------------------------------
// Chapter numbers

/// A chapter's number in thousandths: 10.5 is 10 500. No float, so 10.1 stays 10.1, and no
/// decimal crate. Never negative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChapterNumber {
    milli: i64,
}

impl ChapterNumber {
    pub const fn from_milli(milli: i64) -> Self {
        Self { milli: if milli < 0 { 0 } else { milli } }
    }

    pub const fn whole_number(n: u32) -> Self {
        Self { milli: n as i64 * 1000 }
    }

    pub const fn milli(self) -> i64 {
        self.milli
    }

    /// What trackers are sent: the whole chapters, rounded down. A finished 10.5 goes out as 10,
    /// applied once where the engine builds a tracker op, never inside a tracker.
    pub fn whole(self) -> u32 {
        u32::try_from(self.milli / 1000).unwrap_or(u32::MAX)
    }

    /// Whether the number has a part after the point: 10.5, not 10.
    pub const fn is_split(self) -> bool {
        self.milli % 1000 != 0
    }

    /// A plain decimal: `12`, `10.5`, `7.25`. At most three places; more are cut. `None` for
    /// anything else.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (whole, frac) = match text.split_once('.') {
            Some((w, f)) => (w, f),
            None => (text, ""),
        };
        if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if !frac.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let whole: i64 = whole.parse().ok()?;
        let mut milli = whole.checked_mul(1000)?;
        let mut scale = 100;
        for digit in frac.bytes().take(3) {
            milli += i64::from(digit - b'0') * scale;
            scale /= 10;
        }
        Some(Self { milli })
    }
}

impl fmt::Display for ChapterNumber {
    /// The shortest decimal: `12`, `10.5`, `7.25`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.milli / 1000;
        let frac = self.milli % 1000;
        if frac == 0 {
            return write!(f, "{whole}");
        }
        let digits = format!("{frac:03}");
        write!(f, "{whole}.{}", digits.trim_end_matches('0'))
    }
}

/// A chapter's number, or a run of them a source lists as one: "12 to 13".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChapterRange {
    pub start: ChapterNumber,
    pub end: Option<ChapterNumber>,
}

impl ChapterRange {
    pub const fn single(number: ChapterNumber) -> Self {
        Self { start: number, end: None }
    }

    /// The last chapter the range covers: what counts as read once it is.
    pub fn last(self) -> ChapterNumber {
        self.end.unwrap_or(self.start)
    }

    /// Read the number out of a source's label: `10.5`, `12-13`, `Chapter 12`, `Ch. 10.5`,
    /// `Episode 63`, `Vol. 2 Chapter 3` (the chapter's 3). `None` for a label with no number,
    /// such as `Extra`.
    pub fn parse(label: &str) -> Option<Self> {
        let lower = label.to_lowercase();
        let tokens = numbers_in(&lower);
        if tokens.is_empty() {
            return None;
        }
        // A number right after a chapter word wins; then one not right after a volume word.
        let after = |words: &[&str], at: usize| {
            let before = lower[..at].trim_end().trim_end_matches(['.', ':', '#']).trim_end();
            words.iter().any(|w| before.ends_with(w))
        };
        const CHAPTER: &[&str] = &["chapter", "chap", "ch", "episode", "ep", "part", "no", "#"];
        const VOLUME: &[&str] = &["volume", "vol", "v", "book", "season"];
        let pick = tokens
            .iter()
            .position(|t| after(CHAPTER, t.at))
            .or_else(|| tokens.iter().position(|t| !after(VOLUME, t.at)))?;
        let start = tokens[pick].number;
        let end = tokens.get(pick + 1).and_then(|next| {
            let between = &lower[tokens[pick].end..next.at];
            let joined = matches!(between.trim(), "-" | "\u{2013}" | "to" | "~");
            (joined && next.number > start).then_some(next.number)
        });
        Some(Self { start, end })
    }
}

impl fmt::Display for ChapterRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.end {
            Some(end) => write!(f, "{}-{end}", self.start),
            None => write!(f, "{}", self.start),
        }
    }
}

struct NumberAt {
    at: usize,
    end: usize,
    number: ChapterNumber,
}

/// The decimal numbers in `text`, with where each starts and ends.
fn numbers_in(text: &str) -> Vec<NumberAt> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
        if let Some(number) = ChapterNumber::parse(&text[start..i]) {
            out.push(NumberAt { at: start, end: i, number });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Chapter keys

/// What read marks hang on: a chapter's identity within its work, the same whichever source or
/// group lists it.
///
/// - `12`, `10.5`, `12-13`: a numbered chapter, the number in its shortest form.
/// - `v2/3`: only for a work whose numbering restarts per volume.
/// - `x/<volume or ->/<label>`: a chapter with no number, the label lowercased with its
///   whitespace folded.
///
/// The volume stays out of a normally numbered work's keys, so a source that lists volumes and
/// one that does not agree on chapter 12.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChapterKey(String);

impl ChapterKey {
    /// A numbered chapter's key.
    pub fn numbered(range: ChapterRange) -> Self {
        Self(range.to_string())
    }

    /// A chapter's key in a work whose numbering restarts per volume.
    pub fn in_volume(volume: &str, range: ChapterRange) -> Self {
        Self(format!("v{}/{range}", volume_part(volume)))
    }

    /// The key of a chapter with no number, such as an extra.
    pub fn unnumbered(volume: Option<&str>, label: &str) -> Self {
        let volume = volume.map(volume_part).filter(|v| !v.is_empty());
        let label = label.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        Self(format!("x/{}/{label}", volume.as_deref().unwrap_or("-")))
    }

    /// The key for a chapter a source lists as `label` in `volume`, in a work whose numbering
    /// restarts per volume or not.
    pub fn of(label: &str, volume: Option<&str>, restarts_per_volume: bool) -> Self {
        match (ChapterRange::parse(label), volume) {
            (Some(range), Some(volume)) if restarts_per_volume => {
                Self::in_volume(volume, range)
            }
            (Some(range), _) => Self::numbered(range),
            (None, volume) => Self::unnumbered(volume, label),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The chapter's number, for a numbered key.
    pub fn range(&self) -> Option<ChapterRange> {
        let number = match self.0.split_once('/') {
            None => self.0.as_str(),
            Some((volume, number)) if volume.starts_with('v') => number,
            Some(_) => return None,
        };
        let (start, end) = match number.split_once('-') {
            Some((s, e)) => (s, Some(e)),
            None => (number, None),
        };
        Some(ChapterRange {
            start: ChapterNumber::parse(start)?,
            end: match end {
                Some(e) => Some(ChapterNumber::parse(e)?),
                None => None,
            },
        })
    }

    /// The volume a key names: only a restarting work's keys and unnumbered ones carry one.
    pub fn volume(&self) -> Option<&str> {
        let (head, rest) = self.0.split_once('/')?;
        match head {
            "x" => rest.split_once('/').map(|(v, _)| v).filter(|v| *v != "-"),
            v if v.starts_with('v') => Some(&v[1..]),
            _ => None,
        }
    }

    /// The id that crosses to the apps.
    pub fn id(&self) -> ChapterId {
        ChapterId(self.0.clone())
    }
}

impl fmt::Display for ChapterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ChapterKey {
    type Err = KeyError;

    /// A key as [`ChapterKey`]'s forms spell it, checked, not normalised.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let key = Self(s.to_owned());
        let valid = if let Some(rest) = s.strip_prefix("x/") {
            rest.split_once('/').is_some_and(|(v, label)| !v.is_empty() && !label.is_empty())
        } else {
            key.range().is_some_and(|r| r.to_string() == s.rsplit('/').next().unwrap_or(s))
        };
        if valid { Ok(key) } else { Err(KeyError { what: "chapter key", text: s.to_owned() }) }
    }
}

/// A volume as a key holds it: a number in its shortest form, else the label folded.
fn volume_part(volume: &str) -> String {
    let trimmed = volume.trim();
    let numeric = trimmed
        .to_lowercase()
        .trim_start_matches("volume")
        .trim_start_matches("vol.")
        .trim_start_matches("vol")
        .trim()
        .to_owned();
    match ChapterNumber::parse(&numeric) {
        Some(n) => n.to_string(),
        None => trimmed.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase(),
    }
}

/// A chapter as the apps hold it: [`ChapterKey`]'s string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChapterId(pub String);

impl ChapterId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn key(&self) -> Option<ChapterKey> {
        self.0.parse().ok()
    }
}

impl fmt::Display for ChapterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<ChapterKey> for ChapterId {
    fn from(key: ChapterKey) -> Self {
        Self(key.0)
    }
}

#[cfg(feature = "ffi")]
uniffi::custom_newtype!(ChapterId, String);

// ---------------------------------------------------------------------------------------------
// What a book source answers with

/// Where a book source's chapters come from. Drives labelling and ranking, as
/// [`crate::traits::ProviderKind`] does for shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BookSourceKind {
    /// Folders on this device.
    Folder,
    /// A Komga, Kavita or OPDS server the person added.
    Server,
    /// A `kaeri:books` WebAssembly plugin.
    Plugin,
    /// An in-process source: the test fixtures.
    Native,
}

impl BookSourceKind {
    /// Lowest first: your files, then servers, then plugins. The person's own order decides; this
    /// breaks its ties.
    pub const fn tier(self) -> u8 {
        match self {
            Self::Folder => 0,
            Self::Server => 1,
            Self::Plugin | Self::Native => 2,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Server => "server",
            Self::Plugin => "plugin",
            Self::Native => "native",
        }
    }
}

/// What a book source says of itself: the WIT `manifest`, with its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookSourceManifest {
    /// Stable machine id: config, health and the store key on it.
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: BookSourceKind,
    /// The media it has, from the manifest's `media`; a name this version does not know is left
    /// out.
    pub media: Vec<Medium>,
    /// BCP 47 tags of the languages it has.
    pub languages: Vec<String>,
    /// Hosts it may reach. The host enforces this on every fetch, page images included.
    pub allowed_hosts: Vec<String>,
    /// Behaviour it declares, an open set: unknown ones are ignored.
    pub capabilities: Vec<String>,
}

impl BookSourceManifest {
    pub fn has(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

/// The ids other services know a book by, for a source with an index of its own to answer
/// [`crate::traits::BookSource::lookup`] with no fuzzy matching. The manga ids are their own
/// types: a MyAnimeList manga id is not an anime id of the same number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BookIds {
    pub anilist: Option<AnilistId>,
    pub mal: Option<MalMangaId>,
    pub kitsu: Option<KitsuMangaId>,
    pub mangaupdates: Option<MangaUpdatesId>,
}

impl BookIds {
    pub const fn is_empty(&self) -> bool {
        self.anilist.is_none()
            && self.mal.is_none()
            && self.kitsu.is_none()
            && self.mangaupdates.is_none()
    }
}

/// What is known about the work being asked for: the WIT `title-hints`. Everything is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookHints {
    pub anilist_id: Option<AnilistId>,
    pub mal_id: Option<MalMangaId>,
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
    pub synonyms: Vec<String>,
    pub medium: Option<Medium>,
}

/// One series a source's search found: the WIT `series-hit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesHit {
    /// The source's own id for it, opaque.
    pub id: String,
    pub title: String,
    pub synonyms: Vec<String>,
    pub format: Option<ReadingFormat>,
    pub origin: Option<Origin>,
    pub year: Option<u32>,
    pub cover: Option<String>,
    pub language: Option<String>,
}

impl SeriesHit {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            synonyms: Vec::new(),
            format: None,
            origin: None,
            year: None,
            cover: None,
            language: None,
        }
    }
}

/// Where a series stands, as a source says it. An open set at the edge.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SeriesStatus {
    Releasing,
    Finished,
    Hiatus,
    Cancelled,
    Other(String),
}

impl SeriesStatus {
    pub fn parse(name: &str) -> Option<Self> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(match trimmed.to_ascii_lowercase().replace(['_', '-'], " ").as_str() {
            "releasing" | "ongoing" | "publishing" => Self::Releasing,
            "finished" | "completed" | "ended" => Self::Finished,
            "hiatus" | "on hiatus" => Self::Hiatus,
            "cancelled" | "canceled" | "discontinued" => Self::Cancelled,
            _ => Self::Other(trimmed.to_owned()),
        })
    }
}

/// How a source says its pages read. The reader takes it after the person's own choice for the
/// series and before its own guess from the book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadingHint {
    RightToLeft,
    LeftToRight,
    /// Pages one under another, each whole on the screen.
    Vertical,
    /// One long strip: a webtoon.
    Strip,
}

impl ReadingHint {
    /// `rtl`, `ltr`, `vertical`, `strip`, and the names sources give them (`webtoon`,
    /// `long strip`); anything else is no hint.
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().replace(['_', ' '], "-").as_str() {
            "rtl" | "right-to-left" => Some(Self::RightToLeft),
            "ltr" | "left-to-right" => Some(Self::LeftToRight),
            "vertical" => Some(Self::Vertical),
            "strip" | "webtoon" | "long-strip" | "longstrip" | "continuous" => {
                Some(Self::Strip)
            }
            _ => None,
        }
    }

    /// The name [`Self::parse`] reads back.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RightToLeft => "rtl",
            Self::LeftToRight => "ltr",
            Self::Vertical => "vertical",
            Self::Strip => "strip",
        }
    }
}

/// A series as its source describes it: the WIT `series-detail`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesDetail {
    pub id: String,
    pub title: String,
    pub synonyms: Vec<String>,
    pub description: Option<String>,
    pub cover: Option<String>,
    pub status: Option<SeriesStatus>,
    pub authors: Vec<String>,
    pub genres: Vec<String>,
    pub format: Option<ReadingFormat>,
    pub origin: Option<Origin>,
    pub reading_hint: Option<ReadingHint>,
    pub languages: Vec<String>,
}

impl SeriesDetail {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            synonyms: Vec::new(),
            description: None,
            cover: None,
            status: None,
            authors: Vec::new(),
            genres: Vec::new(),
            format: None,
            origin: None,
            reading_hint: None,
            languages: Vec::new(),
        }
    }
}

/// One chapter as a source lists it: the WIT `chapter`, with its number read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChapter {
    /// The source's own id for it, opaque: what [`crate::traits::BookSource::content`] takes.
    pub id: String,
    /// The source's own label, always kept for display: `10.5`, `Extra`, `Episode 63`.
    pub number_raw: String,
    /// The number read from the label, `None` for one without.
    pub number: Option<ChapterRange>,
    pub volume: Option<String>,
    pub title: Option<String>,
    /// A group or translation name, as the source gives it.
    pub version_label: Option<String>,
    /// BCP 47.
    pub language: Option<String>,
    /// When it came out, Unix seconds.
    pub published: Option<i64>,
    pub page_count: Option<u32>,
}

impl SourceChapter {
    /// A chapter listed as `label`, its number read from it.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let number_raw = label.into();
        Self {
            id: id.into(),
            number: ChapterRange::parse(&number_raw),
            number_raw,
            volume: None,
            title: None,
            version_label: None,
            language: None,
            published: None,
            page_count: None,
        }
    }

    /// A chapter built from the parts a folder or a server knows: its volume, its number as
    /// written (`10.5`), and a title or file name to fall back on. A numbered chapter reads
    /// "Chapter 10.5"; a book that is a whole volume and has no chapter number reads "Volume 2";
    /// anything else keeps `fallback`.
    pub fn of(
        id: impl Into<String>,
        volume: Option<&str>,
        number: Option<&str>,
        title: Option<&str>,
        fallback: &str,
    ) -> Self {
        let label = match (number, volume) {
            (Some(number), _) => format!("Chapter {number}"),
            (None, Some(volume)) => format!("Volume {volume}"),
            (None, None) => title.filter(|t| !t.is_empty()).unwrap_or(fallback).to_owned(),
        };
        let mut chapter = Self::new(id, label);
        // A whole-volume book has no chapter number, whatever its label parses to.
        if number.is_none() && volume.is_some() {
            chapter.number = None;
        }
        chapter.volume = volume.map(str::to_owned);
        chapter.title = title.filter(|t| !t.is_empty()).map(str::to_owned);
        chapter
    }

    /// Its key, in a work whose numbering restarts per volume or not.
    pub fn key(&self, restarts_per_volume: bool) -> ChapterKey {
        match (self.number, self.volume.as_deref()) {
            (Some(range), Some(volume)) if restarts_per_volume => {
                ChapterKey::in_volume(volume, range)
            }
            (Some(range), _) => ChapterKey::numbered(range),
            (None, volume) => ChapterKey::unnumbered(volume, &self.number_raw),
        }
    }
}

/// One page image a source names: the WIT `page`. The host fetches it, never the guest, with the
/// headers it carries, so a download can fetch it later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRef {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl PageRef {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into(), headers: Vec::new(), width: None, height: None }
    }
}

/// A whole book file on this device: an archive, an EPUB or a PDF a folder or a server hands
/// over. The engine opens it with `kaeri-books`, which reads the file's own bytes for its kind;
/// the extension is only a hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookFile {
    pub path: PathBuf,
    /// A copy the engine made for this read (a server's download), removed once done with;
    /// `false` for a file of the person's own.
    pub temporary: bool,
}

/// What a chapter is made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChapterContent {
    /// Page images, fetched by the host.
    Images(Vec<PageRef>),
    /// An XHTML body fragment, UTF-8. The engine sanitises it before any web view sees it.
    Text(String),
    /// A whole book file.
    Book(BookFile),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(text: &str) -> ChapterNumber {
        ChapterNumber::parse(text).unwrap()
    }

    #[test]
    fn chapter_numbers_keep_their_decimals_without_floats() {
        assert_eq!(n("10.5").milli(), 10_500);
        assert_eq!(n("10.5").to_string(), "10.5");
        assert_eq!(n("7.25").to_string(), "7.25");
        assert_eq!(n("12").to_string(), "12");
        assert_eq!(n("012.50").to_string(), "12.5");
        assert_eq!(n("10.5").whole(), 10);
        assert!(n("10.5").is_split());
        assert!(!n("10").is_split());
        assert!(n("10.1") < n("10.5"));
        assert_eq!(ChapterNumber::parse("ten"), None);
        assert_eq!(ChapterNumber::parse("-1"), None);
        assert_eq!(ChapterNumber::parse(""), None);
    }

    #[test]
    fn a_label_s_number_is_read_where_the_chapter_is() {
        let parse = |label| ChapterRange::parse(label).map(|r| r.to_string());
        assert_eq!(parse("10.5").as_deref(), Some("10.5"));
        assert_eq!(parse("Chapter 12").as_deref(), Some("12"));
        assert_eq!(parse("Ch. 10.5").as_deref(), Some("10.5"));
        assert_eq!(parse("Episode 63").as_deref(), Some("63"));
        assert_eq!(parse("12-13").as_deref(), Some("12-13"));
        assert_eq!(parse("Chapter 12 to 13").as_deref(), Some("12-13"));
        assert_eq!(parse("Vol. 2 Chapter 3").as_deref(), Some("3"));
        assert_eq!(parse("Vol. 2 Ch. 3: The Harbour Light").as_deref(), Some("3"));
        assert_eq!(parse("Extra"), None);
        // A range that runs backwards is a number and a title.
        assert_eq!(parse("Chapter 14 - 1 Year Later").as_deref(), Some("14"));
        assert_eq!(ChapterRange::parse("12-13").unwrap().last(), n("13"));
    }

    #[test]
    fn chapter_keys_leave_the_volume_out_unless_numbering_restarts() {
        assert_eq!(ChapterKey::of("Chapter 12", Some("Vol. 2"), false).as_str(), "12");
        assert_eq!(ChapterKey::of("Chapter 12", None, false).as_str(), "12");
        assert_eq!(ChapterKey::of("Chapter 3", Some("Vol. 02"), true).as_str(), "v2/3");
        assert_eq!(ChapterKey::of("12-13", None, false).as_str(), "12-13");
        assert_eq!(
            ChapterKey::of("  Extra   Story ", Some("3"), false).as_str(),
            "x/3/extra story"
        );
        assert_eq!(ChapterKey::of("Afterword", None, false).as_str(), "x/-/afterword");
        let key = ChapterKey::of("Chapter 3", Some("2"), true);
        assert_eq!(key.volume(), Some("2"));
        assert_eq!(key.range().map(|r| r.to_string()).as_deref(), Some("3"));
        assert_eq!(ChapterKey::of("Afterword", Some("4"), false).volume(), Some("4"));
        assert_eq!(ChapterKey::of("Extra", None, false).range(), None);
        for text in ["12", "10.5", "12-13", "v2/3", "x/-/afterword"] {
            assert_eq!(text.parse::<ChapterKey>().unwrap().as_str(), text);
        }
        for text in ["", "012", "x/", "v2/", "chapter 12"] {
            assert!(text.parse::<ChapterKey>().is_err(), "{text}");
        }
    }

    #[test]
    fn work_keys_round_trip_and_local_keys_ignore_spelling() {
        let matched = WorkKey::Anilist(AnilistId::new(30_001));
        assert_eq!(matched.to_string(), "a:30001");
        assert_eq!("a:30001".parse::<WorkKey>().unwrap(), matched);
        let local = WorkKey::local(Medium::Manga, "The Lantern Keeper [Digital]");
        assert_eq!(local, WorkKey::local(Medium::Manga, "the  lantern-keeper"));
        assert_ne!(local, WorkKey::local(Medium::Novel, "The Lantern Keeper"));
        let WorkKey::Local(hex) = &local else { panic!() };
        assert_eq!(hex.len(), 32);
        assert_eq!(local.to_string().parse::<WorkKey>().unwrap(), local);
        assert!("a:x".parse::<WorkKey>().is_err());
        assert!("l:short".parse::<WorkKey>().is_err());
        assert_eq!(
            WorkId::from(AnilistId::new(7)).key(),
            Some(WorkKey::Anilist(AnilistId::new(7)))
        );
        assert!(local.is_local() && local.anilist().is_none());
    }

    #[test]
    fn a_source_chapter_reads_its_number_and_keeps_its_label() {
        let mut chapter = SourceChapter::new("c-1", "Chapter 10.5");
        assert_eq!(chapter.number_raw, "Chapter 10.5");
        assert_eq!(chapter.number.map(|r| r.start), Some(n("10.5")));
        assert_eq!(chapter.key(false).as_str(), "10.5");
        chapter.volume = Some("2".into());
        assert_eq!(chapter.key(true).as_str(), "v2/10.5");
        assert_eq!(SourceChapter::new("c-2", "Extra").key(false).as_str(), "x/-/extra");
    }

    #[test]
    fn a_chapter_built_from_parts_reads_as_a_chapter_a_volume_or_a_title() {
        let chapter = SourceChapter::of("a", Some("2"), Some("10.5"), None, "x");
        assert_eq!(chapter.number_raw, "Chapter 10.5");
        assert_eq!(chapter.key(true).as_str(), "v2/10.5");
        let volume = SourceChapter::of("b", Some("3"), None, Some("Moonlit v03"), "x");
        assert_eq!((volume.number_raw.as_str(), volume.number), ("Volume 3", None));
        assert_eq!(volume.key(false).as_str(), "x/3/volume 3");
        let loose = SourceChapter::of("c", None, None, None, "Special");
        assert_eq!(loose.key(false).as_str(), "x/-/special");
    }

    #[test]
    fn open_sets_read_what_sources_send() {
        assert_eq!(SeriesStatus::parse("ONGOING"), Some(SeriesStatus::Releasing));
        assert_eq!(SeriesStatus::parse("on-hiatus"), Some(SeriesStatus::Hiatus));
        assert_eq!(SeriesStatus::parse("paused"), Some(SeriesStatus::Other("paused".into())));
        assert_eq!(ReadingHint::parse("RTL"), Some(ReadingHint::RightToLeft));
        assert_eq!(ReadingHint::parse("sideways"), None);
        assert_eq!(ReadingHint::parse("Long Strip"), Some(ReadingHint::Strip));
        assert_eq!(ReadingHint::parse("WEBTOON"), Some(ReadingHint::Strip));
        assert_eq!(ReadingHint::parse("left_to_right"), Some(ReadingHint::LeftToRight));
        assert_eq!(ReadingHint::parse("vertical"), Some(ReadingHint::Vertical));
        for hint in [
            ReadingHint::RightToLeft,
            ReadingHint::LeftToRight,
            ReadingHint::Vertical,
            ReadingHint::Strip,
        ] {
            assert_eq!(ReadingHint::parse(hint.as_str()), Some(hint));
        }
        assert_eq!(BookSourceKind::Folder.tier(), 0);
        assert!(BookIds::default().is_empty());
    }
}
