//! The reader: one open session, the pages and leaves it shows, where the person is, and the
//! reader's settings with their scopes.
//!
//! The app reports what it painted; the engine decides what that means: the place to keep, when a
//! chapter counts as read, what to fetch next. Pages cross as files in the page cache with their
//! metadata, never as bytes.

use kaeri_core::{
    books::{ChapterId, WorkId},
    media::Medium,
};

use crate::{Msg, eink::NovelSupport, reading::ChapterRow};

// ---------------------------------------------------------------------------------------------
// Pages and leaves

/// One page image in the page cache, written under a temporary name and renamed, so a path the app
/// is given is always whole.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PageFile {
    pub path: String,
    pub encoding: PageEncoding,
    /// The file's pixels.
    pub width: u32,
    pub height: u32,
    /// What to show of the file: an original on a screen with its borders cropped.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub crop: Option<PixelRect>,
    /// The page has colour.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub colour: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub bytes: u64,
}

/// How a page file is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PageEncoding {
    /// As the book had it (JPEG, PNG, WebP, AVIF): the platform decodes it.
    Original { mime: String },
    /// Processed: pixels with no header, ready to map. A grey e-ink page is `Gray8`.
    Raw { format: PixelFormat, stride: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PixelFormat {
    Gray8,
    Rgba8,
}

/// What a page is, for laying it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PageKind {
    Story,
    /// Two pages printed as one: shown alone, split or rotated as set.
    Spread,
    Cover,
    /// A page the book marks to leave out.
    Skip,
    /// A piece of a long strip cut at a gap, on e-ink.
    StripCut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// A rectangle in a leaf, each side from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct UnitRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl UnitRect {
    pub const WHOLE: Self = Self { x: 0.0, y: 0.0, width: 1.0, height: 1.0 };
}

/// What one screen shows: one page, two side by side, or a piece of a strip. The engine lays out
/// leaves, so no app repeats the pairing rule.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Leaf {
    pub index: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub parts: Vec<LeafPart>,
    /// The page between chapters, in the flow.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub end_page: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LeafPart {
    /// The page's number in the chapter, from 0. For a strip cut on e-ink, the cut's number.
    pub page: u32,
    pub kind: PageKind,
    /// `None` until the page lands; [`ReaderEvent::PageReady`] fills it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub file: Option<PageFile>,
    /// Where in the leaf it goes.
    pub place: UnitRect,
    /// A spread turned on its side on a single-page screen.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rotate: bool,
    /// One half of a spread split on a single-page screen: the file holds only that half.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub half: Option<PageHalf>,
    /// The part's width over its height, known from the book before the file is made: a
    /// strip sizes each image with it, so nothing moves when the files land.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub ratio: Option<f32>,
}

/// Which half of a split spread a part shows, as it lies on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PageHalf {
    Left,
    Right,
}

// ---------------------------------------------------------------------------------------------
// Positions

/// A place in a novel's text: structural, never a percentage. `path` counts element children down
/// from `<body>`; `offset` counts characters in that element's text, ruby included, soft hyphens
/// left out.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NovelPos {
    pub spine: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub path: Vec<u32>,
    pub offset: u32,
}

impl NovelPos {
    /// The text form the store and sync keep: `3/0.4.1/120`.
    pub fn encode(&self) -> String {
        let path: Vec<String> = self.path.iter().map(u32::to_string).collect();
        format!("{}/{}/{}", self.spine, path.join("."), self.offset)
    }

    pub fn decode(text: &str) -> Option<Self> {
        let mut parts = text.split('/');
        let spine = parts.next()?.parse().ok()?;
        let path_text = parts.next()?;
        let offset = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        let path = if path_text.is_empty() {
            Vec::new()
        } else {
            path_text.split('.').map(str::parse).collect::<Result<_, _>>().ok()?
        };
        Some(Self { spine, path, offset })
    }
}

/// A place in a strip: which image, and how far into it, from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StripPos {
    pub image: u32,
    pub offset: f32,
}

/// Where someone is in a work.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReadPlace {
    pub work: WorkId,
    pub chapter: ChapterId,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub page: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub strip: Option<StripPos>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub novel: Option<NovelPos>,
}

/// One chapter's facts, as the store keeps them. Page counts are each device's own.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ChapterProgress {
    pub chapter: ChapterId,
    pub read: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub last_page: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub of: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub strip: Option<StripPos>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub novel: Option<NovelPos>,
    /// Unix seconds.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub last_read: Option<i64>,
}

/// A work's progress, worked out from its chapters' facts and never kept as one pointer.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SeriesProgress {
    pub work: WorkId,
    /// The newest place, by stamp.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub resume: Option<ReadPlace>,
    /// The highest chapter read.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub furthest: Option<ChapterId>,
    /// Chapters out since the last read and not read.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub unread_new: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub chapters_read: u32,
}

// ---------------------------------------------------------------------------------------------
// Settings

/// A reader setting. Which media it serves, where it is kept and whether it syncs are its own:
/// see [`ReaderSettingKey::home_scope`], [`ReaderSettingKey::series_scope`] and
/// [`ReaderSettingKey::syncs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReaderSettingKey {
    /// Paged or strip; novels paged or scrolled.
    Mode,
    /// Right to left or left to right.
    Direction,
    /// Auto, colour or greys.
    Colour,
    /// Paper, light, sepia or dark, with its background.
    Look,
    Font,
    CropBorders,
    Fit,
    /// The ground behind a page: the theme's, black or white.
    Background,
    /// Auto, single or double.
    PageLayout,
    /// Split or rotate spreads on a single-page screen.
    Spreads,
    TextSize,
    Margins,
    LineSpacing,
    Justify,
    TapLayout,
    VolumeKeysTurn,
    StripWidth,
    RefreshImage,
    RefreshText,
    RefreshList,
    GallerySpeed,
    RainbowFilter,
    Saturation,
    Contrast,
    PaperMode,
    GreyscalePages,
    QuietPhone,
    /// What the foot of the page says: pages left, or nothing.
    FootValue,
    Folio,
    /// The space between a strip's images: none or small.
    StripGap,
    /// Paper mode's parts: turn pages at once, the Paper look by day, how far the page's white
    /// is dimmed from 0 to 1, and the volume keys turning pages.
    PaperInstant,
    PaperLook,
    PaperDim,
    PaperVolume,
}

/// Where a setting's value is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SettingScope {
    /// This series, synced (the `read_series` register).
    Series,
    /// This series, on this device only.
    SeriesHere,
    /// The medium's default, synced as a shared setting.
    Medium,
    /// This device only, in its config.
    Device,
}

impl SettingScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Series => "series",
            Self::SeriesHere => "here",
            Self::Medium => "medium",
            Self::Device => "device",
        }
    }
}

impl ReaderSettingKey {
    pub const ALL: &[Self] = &[
        Self::Mode,
        Self::Direction,
        Self::Colour,
        Self::Look,
        Self::Font,
        Self::CropBorders,
        Self::Fit,
        Self::Background,
        Self::PageLayout,
        Self::Spreads,
        Self::TextSize,
        Self::Margins,
        Self::LineSpacing,
        Self::Justify,
        Self::TapLayout,
        Self::VolumeKeysTurn,
        Self::StripWidth,
        Self::RefreshImage,
        Self::RefreshText,
        Self::RefreshList,
        Self::GallerySpeed,
        Self::RainbowFilter,
        Self::Saturation,
        Self::Contrast,
        Self::PaperMode,
        Self::GreyscalePages,
        Self::QuietPhone,
        Self::FootValue,
        Self::Folio,
        Self::StripGap,
        Self::PaperInstant,
        Self::PaperLook,
        Self::PaperDim,
        Self::PaperVolume,
    ];

    /// The setting's name in the store, the config and the `read_series` register.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mode => "mode",
            Self::Direction => "direction",
            Self::Colour => "colour",
            Self::Look => "look",
            Self::Font => "font",
            Self::CropBorders => "crop",
            Self::Fit => "fit",
            Self::Background => "background",
            Self::PageLayout => "page_layout",
            Self::Spreads => "spreads",
            Self::TextSize => "text_size",
            Self::Margins => "margins",
            Self::LineSpacing => "line_spacing",
            Self::Justify => "justify",
            Self::TapLayout => "tap_layout",
            Self::VolumeKeysTurn => "volume_keys",
            Self::StripWidth => "strip_width",
            Self::RefreshImage => "refresh_image",
            Self::RefreshText => "refresh_text",
            Self::RefreshList => "refresh_list",
            Self::GallerySpeed => "gallery_speed",
            Self::RainbowFilter => "rainbow_filter",
            Self::Saturation => "saturation",
            Self::Contrast => "contrast",
            Self::PaperMode => "paper_mode",
            Self::GreyscalePages => "greyscale_pages",
            Self::QuietPhone => "quiet_phone",
            Self::FootValue => "foot",
            Self::Folio => "folio",
            Self::StripGap => "strip_gap",
            Self::PaperInstant => "paper_instant",
            Self::PaperLook => "paper_look",
            Self::PaperDim => "paper_dim",
            Self::PaperVolume => "paper_volume",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|key| key.name() == name)
    }

    /// Whether the setting is offered for `medium` at all.
    pub const fn serves(self, medium: Medium) -> bool {
        match self {
            Self::Mode | Self::Look | Self::TapLayout | Self::VolumeKeysTurn => {
                !matches!(medium, Medium::Anime)
            }
            Self::Direction
            | Self::Colour
            | Self::CropBorders
            | Self::Fit
            | Self::Background
            | Self::PageLayout
            | Self::Spreads
            | Self::StripWidth
            | Self::StripGap => matches!(medium, Medium::Manga),
            Self::Font | Self::TextSize | Self::Margins | Self::LineSpacing | Self::Justify => {
                matches!(medium, Medium::Novel)
            }
            // E-ink, colour e-ink and phone settings serve both media; the app shows them only
            // where the screen has them.
            _ => !matches!(medium, Medium::Anime),
        }
    }

    /// Where the medium's own value is kept: synced intent ([`SettingScope::Medium`]) or this
    /// device's geometry ([`SettingScope::Device`]).
    pub const fn home_scope(self) -> SettingScope {
        if self.syncs() { SettingScope::Medium } else { SettingScope::Device }
    }

    /// Where a series' own choice is kept, `None` for a setting a series cannot set. Mode,
    /// direction and colour follow you; crop borders stay on the device.
    pub const fn series_scope(self) -> Option<SettingScope> {
        match self {
            Self::Mode | Self::Direction | Self::Colour => Some(SettingScope::Series),
            Self::CropBorders => Some(SettingScope::SeriesHere),
            _ => None,
        }
    }

    /// Whether the medium's value follows the person to their other devices: the choices (mode,
    /// direction, colour, look, font). Layout stays on each device.
    pub const fn syncs(self) -> bool {
        matches!(self, Self::Mode | Self::Direction | Self::Colour | Self::Look | Self::Font)
    }
}

/// A setting's value.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReaderValue {
    Choice { name: String },
    Amount { value: f64 },
    Toggle { on: bool },
}

impl ReaderValue {
    /// The text the store and the `read_series` register keep.
    pub fn encode(&self) -> String {
        match self {
            Self::Choice { name } => name.clone(),
            Self::Amount { value } => format!("#{value}"),
            Self::Toggle { on } => if *on { "on" } else { "off" }.to_owned(),
        }
    }

    pub fn decode(text: &str) -> Self {
        match text {
            "on" => Self::Toggle { on: true },
            "off" => Self::Toggle { on: false },
            _ => match text.strip_prefix('#').and_then(|n| n.parse().ok()) {
                Some(value) => Self::Amount { value },
                None => Self::Choice { name: text.to_owned() },
            },
        }
    }
}

/// One row of the reader's settings sheet.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReaderSettingRow {
    pub key: ReaderSettingKey,
    pub value: ReaderValue,
    pub scope: SettingScope,
    /// The value was guessed: "Auto · right to left".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub detected: bool,
    /// A series' own choice: the row offers Use default.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub overridden: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub choices: Vec<ReaderValue>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReaderSettingsView {
    pub medium: Medium,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub work: Option<WorkId>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rows: Vec<ReaderSettingRow>,
}

// ---------------------------------------------------------------------------------------------
// The session

/// How the reader shows a chapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReaderMode {
    Paged,
    /// Pages one under another, each fitted to the screen.
    Vertical,
    Strip,
    /// Novels only: one long page.
    Scrolled,
}

impl ReaderMode {
    /// Pages run one under another rather than turning.
    pub const fn flows(self) -> bool {
        matches!(self, Self::Strip | Self::Vertical)
    }
}

/// Which way pages turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReadDirection {
    RightToLeft,
    LeftToRight,
    TopToBottom,
}

/// Where a session opens.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum OpenAt {
    /// The work's newest place.
    Resume,
    Start,
    Page {
        page: u32,
    },
    Place {
        place: ReadPlace,
    },
}

/// How a leaf came to be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TurnHow {
    Turn,
    Swipe,
    /// The person's own jump: the scrubber, the page grid, contents or Go to page. More than
    /// two pages from where they were, it offers [`ReaderEvent::BackToPage`].
    Jump,
    Scroll,
    Key,
    /// The app moved the reader itself: the first layout, a resume, Back to page, Start over,
    /// another device's place, or a new layout after a resize or a settings change. It neither
    /// offers Back to page nor takes it away.
    Placed,
}

/// The screen a session draws on.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReaderSurface {
    pub width_px: u32,
    pub height_px: u32,
    pub scale: f32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub eink: bool,
    /// Room for two pages side by side.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub two_pages: bool,
    /// The look is dark: on e-ink a full refresh comes every fourth text page.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub dark: bool,
}

/// What a session opened on.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReaderView {
    /// The session's number, assigned here. Reports for a closed session are dropped.
    pub session: u64,
    pub work: WorkId,
    pub title: String,
    pub medium: Medium,
    pub chapter: ChapterRow,
    pub mode: ReaderMode,
    pub direction: ReadDirection,
    pub start: ReadPlace,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next: Option<ChapterRow>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub previous: Option<ChapterRow>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub settings: Option<ReaderSettingsView>,
    /// A work of the reading demo: the reader shows its credit in a capsule, "Pepper&Carrot by
    /// David Revoy. Licensed CC BY 4.0.", and its end says how to add your own books.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub demo_credit: Option<crate::sources::DemoWorkInfo>,
    /// The host of the book server the work's chapters come from, when it receives progress:
    /// the page between chapters keeps room for "Saved to books.home.example" from the start.
    /// `None` for a work from no server, or one set not to receive.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub progress_host: Option<String>,
}

/// How a text's lines run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum WritingMode {
    Horizontal,
    VerticalRightToLeft,
    VerticalLeftToRight,
}

/// A text chapter or an EPUB, opened in the platform's web view.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NovelOpenView {
    pub session: u64,
    pub chapter: ChapterId,
    /// The scheme URL of the first document: `kaeri-book://<session>/<path>`, or Android's
    /// asset-loader address.
    pub entry: String,
    pub writing_mode: WritingMode,
    pub progression: ReadDirection,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub restore: Option<NovelPos>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub fixed_layout: bool,
    pub support: NovelSupport,
    /// The book's documents in spine order; [`NovelPos::spine`] indexes this. The host turns
    /// to the next linear one on the paginator's `end` and the previous on `start`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub spine: Vec<NovelSpineItem>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub toc: Vec<NovelTocEntry>,
    /// The paginator's settings as JSON, for `kaeriReader.configure`. The engine also serves
    /// each document with them, so a host only needs this for a live change.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub style: String,
    /// What a whole-volume page count is kept under: the book, the settings and the viewport.
    /// A [`ReaderCommand::NovelCounted`] names it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub count_key: String,
    /// The pages of each spine document, counted before under `count_key`; empty when the
    /// hidden web view should count.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub counted: Vec<u32>,
    /// E-ink, fixed layout: the folder the app renders page `n` into as `<n>.png` before it
    /// sends [`ReaderCommand::FixedRendered`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub render_dir: Option<String>,
    /// The person's reading pace, learnt from their page turns: see [`novel_minutes_left`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub words_per_minute: u32,
}

/// One document of a novel's spine.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NovelSpineItem {
    /// Its scheme URL.
    pub url: String,
    /// Out of the reading flow (notes, pop-ups): reached by links only.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub linear: bool,
    /// The words in it, two Chinese, Japanese or Korean characters counting as one: what the
    /// time left comes from.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub words: u32,
}

/// The minutes left in a novel's chapter at the person's pace, for the scrubber's "6 min": the
/// words of the document from page `page` (counted from 0) of `pages` on, read at
/// `words_per_minute`. `None` when a count is missing; at least 1 while any page is left.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub fn novel_minutes_left(
    words: u32,
    page: u32,
    pages: u32,
    words_per_minute: u32,
) -> Option<u32> {
    if words == 0 || pages == 0 || words_per_minute == 0 || page >= pages {
        return None;
    }
    let left = u64::from(words) * u64::from(pages - page) / u64::from(pages);
    let minutes = left.div_ceil(u64::from(words_per_minute)).max(1);
    Some(u32::try_from(minutes).unwrap_or(u32::MAX))
}

/// One line of a novel's contents.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NovelTocEntry {
    pub label: String,
    /// The scheme URL, with the fragment it points at.
    pub url: String,
    /// The spine document it is in.
    pub spine: u32,
    /// 0 for the top level.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub depth: u32,
}

/// What a book's scheme handler answers the web view with.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BookResource {
    pub status: u16,
    pub mime: String,
    /// The content security policy and the other headers to send; `Content-Type` included.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub headers: Vec<crate::playback::HttpHeader>,
    pub body: BookBody,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum BookBody {
    Bytes {
        data: Vec<u8>,
    },
    /// Images inside a novel arrive as files.
    File {
        path: String,
    },
}

/// What every host injects into a novel's web view.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NovelAssets {
    /// The paginator, with the stylesheet inside: one user script.
    pub script: String,
    pub stylesheet: String,
    /// The policy every book response carries, for hosts that set it themselves too.
    pub csp: String,
}

/// What an open waits on, for the opening screen's slow line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReaderWait {
    /// A source is asked for the series' chapters.
    FindingSource,
}

/// Why the end page names no next chapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NoNext {
    NotOutYet,
    CaughtUp,
    Finished,
    NoSourceHasIt,
    Offline,
}

/// What a notice's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReaderAction {
    StartOver,
    GoBack,
    ChangeMode,
}

/// A line over the page for a while: resumed, moved, mode detected.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReaderNotice {
    pub msg: Msg,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub action: Option<ReaderAction>,
    /// How long it shows. 0 on e-ink, where it clears at the next turn.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub lasts_ms: u32,
}

/// How long a reader notice stays on screen, in milliseconds, from the engine's `lasts_ms`, the
/// words the person reads in it and whether a screen reader is running. `None` keeps it until
/// it is dismissed, acted on or replaced: a notice with a button while a screen reader runs.
/// Longer text adds a second for every 10 words past 12. On e-ink, `lasts_ms` is 0 and stays 0:
/// the notice clears at the next turn.
///
/// The time counts only while the reader is in front: a host pauses it while the app is
/// inactive, and under the pointer or a screen reader's focus where it can tell.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub fn reader_notice_hold(
    lasts_ms: u32,
    words: u32,
    has_action: bool,
    screen_reader: bool,
) -> Option<u32> {
    if has_action && screen_reader {
        return None;
    }
    if lasts_ms == 0 {
        return Some(0);
    }
    let extra = words.saturating_sub(12).div_ceil(10);
    Some(lasts_ms.saturating_add(extra.saturating_mul(1_000)))
}

/// One open reader session. One at a time per window.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReaderCommand {
    /// Start a session: [`ReaderEvent::Opened`], then leaves and pages as they land, or
    /// [`ReaderEvent::NovelOpen`].
    Open {
        work: WorkId,
        chapter: Option<ChapterId>,
        at: OpenAt,
        surface: ReaderSurface,
    },
    /// Rotation, a window resize, room for two pages: new leaves.
    Surface {
        session: u64,
        surface: ReaderSurface,
    },
    /// A leaf was painted. Drives prefetch, the read rule and Back to page. A strip reports
    /// the person's jump this way too, once it has landed, besides its `StripAt`.
    Shown {
        session: u64,
        chapter: ChapterId,
        leaf: u32,
        how: TurnHow,
    },
    /// A strip settled after a scroll, at most every 500 ms.
    StripAt {
        session: u64,
        chapter: ChapterId,
        at: StripPos,
        foot_visible: bool,
    },
    /// The paginator turned a page.
    NovelAt {
        session: u64,
        chapter: ChapterId,
        pos: NovelPos,
        page: u32,
        pages: u32,
        how: TurnHow,
    },
    /// The hidden web view counted the whole volume's pages, kept per book, settings and
    /// viewport under `key`.
    NovelCounted {
        session: u64,
        key: String,
        chapter_pages: Vec<u32>,
    },
    /// E-ink only: the app rendered a fixed-layout page to the file the engine named; it comes
    /// back processed as [`ReaderEvent::PageReady`].
    FixedRendered {
        session: u64,
        chapter: ChapterId,
        page: u32,
        path: String,
    },
    /// A pinned page file is missing: it is made again.
    NeedPage {
        session: u64,
        chapter: ChapterId,
        page: u32,
    },
    /// Try a failed page, or a failed chapter with no page, again.
    Retry {
        session: u64,
        chapter: ChapterId,
        page: Option<u32>,
    },
    OpenChapter {
        session: u64,
        chapter: ChapterId,
    },
    /// "Back to page 8", from the control [`ReaderEvent::BackToPage`] offers.
    BackTo {
        session: u64,
    },
    /// From "Resumed at page 8".
    StartOver {
        session: u64,
    },
    /// From "Moved to chapter 42, page 6".
    GoBack {
        session: u64,
    },
    /// Change a setting, live: [`ReaderEvent::Settings`], and the page laid out again if it must
    /// be. A series' own choice names its `work`.
    SetSetting {
        session: Option<u64>,
        medium: Medium,
        work: Option<WorkId>,
        key: ReaderSettingKey,
        scope: SettingScope,
        value: ReaderValue,
    },
    /// Drop a series' own choice.
    UseDefault {
        medium: Medium,
        work: WorkId,
        key: ReaderSettingKey,
    },
    LoadSettings {
        medium: Medium,
        work: Option<WorkId>,
    },
    /// [`ReaderEvent::Closed`]; the last place is already written. Without a session (0, the
    /// default), it drops whatever open still waits, as when the person leaves "Finding
    /// source" before [`ReaderEvent::Opened`] named one.
    Close {
        #[cfg_attr(feature = "ffi", uniffi(default))]
        session: u64,
    },
}

impl ReaderCommand {
    /// What to call this in the activity row: opening, and nothing for a report.
    pub fn activity(&self) -> Option<String> {
        match self {
            Self::Open { .. } | Self::OpenChapter { .. } => Some("opening the chapter".into()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ReaderEvent {
    /// Once per open and per chapter opened.
    Opened(Box<ReaderView>),
    /// After opening, and after each surface or layout change; the count may grow as an archive
    /// extracts.
    Leaves {
        session: u64,
        chapter: ChapterId,
        leaves: Vec<Leaf>,
    },
    /// A page landed in the cache: the current one first, then the next once the current is
    /// painted.
    PageReady {
        session: u64,
        chapter: ChapterId,
        page: u32,
        /// The half of a split spread this file holds.
        half: Option<PageHalf>,
        file: PageFile,
    },
    PageFailed {
        session: u64,
        chapter: ChapterId,
        page: u32,
        failure: crate::reading::ReadingFailure,
    },
    /// In place of leaves, for text chapters and EPUBs.
    NovelOpen(NovelOpenView),
    /// A novel's settings changed, or its screen did: pass `style` to
    /// `kaeriReader.configure`. The place is kept; counts under the old key no longer apply.
    NovelStyle {
        session: u64,
        style: String,
        count_key: String,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        counted: Vec<u32>,
    },
    /// The read rule fired for a chapter.
    ChapterRead {
        work: WorkId,
        chapter: ChapterId,
        /// The trackers the read was queued for, by name.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        trackers: Vec<String>,
    },
    /// What became of a read chapter's progress sent to the book server it came from, for the
    /// line on the page between chapters. Sent once per chapter read, as soon as it is known;
    /// none comes for a chapter from no book server.
    ProgressSent {
        work: WorkId,
        chapter: ChapterId,
        /// The server's id, as Your sources lists it.
        server: String,
        /// The server's host, as the line names it: "Saved to books.home.example".
        host: String,
        result: crate::book_sources::WriteBackResult,
    },
    /// The last leaf is showing: what the end page names next.
    EndPage {
        session: u64,
        next: Option<ChapterRow>,
        why_none: Option<NoNext>,
        /// The source the next chapter was downloaded from, by name; `None` for a file in a
        /// folder.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        next_source: Option<String>,
    },
    /// The next chapter's first page, made while this chapter is read, for the end page and a
    /// strip's divider.
    NextPreview {
        session: u64,
        chapter: ChapterId,
        file: PageFile,
    },
    /// An open is under way: which chapter, once it is known, and what it waits on. Sent
    /// before [`ReaderEvent::Opened`], so the opening screen can say so.
    Opening {
        #[cfg_attr(feature = "ffi", uniffi(default))]
        chapter: Option<ChapterRow>,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        wait: Option<ReaderWait>,
    },
    Notice(ReaderNotice),
    /// Another device read further: the reader moved, with Go back to `back`.
    Moved {
        session: u64,
        to: ReadPlace,
        from_device: String,
        back: ReadPlace,
    },
    Settings(ReaderSettingsView),
    /// Back to page `page`, counted from 1, is offered in the slider bar while the controls
    /// are up; `None` takes it away. It comes after the person's own jump of more than two
    /// pages, or screens of a novel, and goes at the next turn, once used, or with the chapter:
    /// [`ReaderEvent::Opened`] starts without it.
    BackToPage {
        session: u64,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        page: Option<u32>,
    },
    /// Show this place in the open chapter: Back to page, Start over, Go back, or a place
    /// another device reached. A place in another chapter comes as [`ReaderEvent::Opened`]
    /// instead.
    GoTo {
        session: u64,
        place: ReadPlace,
    },
    /// After closing, or when the work was removed under the reader.
    Closed {
        session: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn novel_positions_round_trip_through_their_text() {
        let pos = NovelPos { spine: 3, path: vec![0, 4, 1], offset: 120 };
        assert_eq!(pos.encode(), "3/0.4.1/120");
        assert_eq!(NovelPos::decode("3/0.4.1/120"), Some(pos));
        let top = NovelPos { spine: 0, path: Vec::new(), offset: 0 };
        assert_eq!(NovelPos::decode(&top.encode()), Some(top));
        assert_eq!(NovelPos::decode("3/x/1"), None);
        assert_eq!(NovelPos::decode("3/1"), None);
    }

    #[test]
    fn choices_sync_and_layout_stays_on_the_device() {
        use ReaderSettingKey as K;
        for key in [K::Mode, K::Direction, K::Colour, K::Look, K::Font] {
            assert!(key.syncs(), "{key:?}");
            assert_eq!(key.home_scope(), SettingScope::Medium);
        }
        for key in
            [K::TextSize, K::Margins, K::TapLayout, K::PageLayout, K::Fit, K::CropBorders]
        {
            assert!(!key.syncs(), "{key:?}");
            assert_eq!(key.home_scope(), SettingScope::Device);
        }
        for key in [K::RefreshText, K::VolumeKeysTurn, K::PaperMode] {
            assert!(!key.syncs(), "{key:?}");
        }
        assert_eq!(K::Mode.series_scope(), Some(SettingScope::Series));
        assert_eq!(K::CropBorders.series_scope(), Some(SettingScope::SeriesHere));
        assert_eq!(K::Look.series_scope(), None);
        assert!(K::Direction.serves(Medium::Manga) && !K::Direction.serves(Medium::Novel));
        assert!(K::Font.serves(Medium::Novel) && !K::Font.serves(Medium::Manga));
        for key in K::ALL {
            assert_eq!(K::from_name(key.name()), Some(*key));
        }
    }

    #[test]
    fn values_round_trip_through_their_text() {
        for value in [
            ReaderValue::Choice { name: "rtl".into() },
            ReaderValue::Amount { value: 1.25 },
            ReaderValue::Toggle { on: true },
            ReaderValue::Toggle { on: false },
        ] {
            assert_eq!(ReaderValue::decode(&value.encode()), value);
        }
    }
}

#[cfg(test)]
mod minutes_left {
    use super::novel_minutes_left;

    #[test]
    fn minutes_come_from_the_words_still_to_read() {
        // 2,300 words at 230 a minute: ten minutes from the first page.
        assert_eq!(novel_minutes_left(2_300, 0, 10, 230), Some(10));
        assert_eq!(novel_minutes_left(2_300, 6, 10, 230), Some(4));
        // The same pages with fewer words take less time.
        assert_eq!(novel_minutes_left(460, 0, 10, 230), Some(2));
        assert_eq!(novel_minutes_left(2_300, 9, 10, 230), Some(1), "never 0 on the last page");
        assert_eq!(novel_minutes_left(2_300, 10, 10, 230), None);
        assert_eq!(novel_minutes_left(0, 0, 10, 230), None, "no count, no guess");
        assert_eq!(novel_minutes_left(2_300, 0, 0, 230), None);
    }
}

#[cfg(test)]
mod notice_hold {
    use super::reader_notice_hold;

    #[test]
    fn a_notice_holds_its_time_longer_for_long_text_and_waits_for_a_screen_reader() {
        // "Resumed at page 8 · Start over": short, its own six seconds.
        assert_eq!(reader_notice_hold(6_000, 4, true, false), Some(6_000));
        // Twelve words fit the time; each ten past them add a second.
        assert_eq!(reader_notice_hold(8_000, 12, true, false), Some(8_000));
        assert_eq!(reader_notice_hold(8_000, 13, true, false), Some(9_000));
        assert_eq!(reader_notice_hold(8_000, 32, true, false), Some(10_000));
        // A button waits for the person under a screen reader; a plain line keeps its time.
        assert_eq!(reader_notice_hold(8_000, 4, true, true), None);
        assert_eq!(reader_notice_hold(4_000, 4, false, true), Some(4_000));
        // E-ink clears at the next turn.
        assert_eq!(reader_notice_hold(0, 30, false, false), Some(0));
    }
}
