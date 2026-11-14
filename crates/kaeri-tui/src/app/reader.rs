//! The reader: a chapter over the whole window, as the engine lays it out.
//!
//! The terminal opens a session with `ReaderCommand::Open`, then follows what the engine sends:
//! the chapter (`Opened`), its leaves, each page's file, the end page's facts and notices. Each
//! turn is reported with `Shown`, and pages the reader is about to need are asked for with
//! `NeedPage`. Page files are decoded off the loop by whoever runs the app: [`App::take_page_jobs`]
//! hands them over and [`App::page_decoded`] takes the pictures back.
//!
//! The surface is the stage in pixels, sent again whenever the window, the chrome or the page
//! count changes, so the engine lays the chapter out again at the same place.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use kaeri_core::{
    books::{ChapterId, WorkId},
    media::Medium,
};
use kaeri_i18n::t;
use kaeri_model::{
    book_sources::{WriteBackProblem, WriteBackResult},
    command::Command,
    notice::NoticeKind,
    reader::{
        Leaf, NoNext, OpenAt, PageEncoding, PageFile, PageFormats, PageHalf, PixelRect,
        ReadDirection, ReaderAction, ReaderCommand, ReaderEvent, ReaderNotice,
        ReaderSettingKey, ReaderSettingsView, ReaderSurface, ReaderValue, ReaderView,
        ReaderWait, SettingScope, TurnHow, reader_notice_hold,
    },
    reading::{
        ChapterReadState, ChapterRow, DownloadPick, ReadingCommand, ReadingFailure, VolumeGroup,
    },
};

use super::{
    App, info,
    reader_view::{Strip, Zoom},
    say,
    series::downloaded,
};
use crate::{
    image::Graphics,
    keymap::{Action, Binding, Context, said},
    nav::StageView,
};

/// The smallest window the reader shows a page in; below it, it says it needs a larger one.
pub const MIN_COLUMNS: u16 = 40;
pub const MIN_ROWS: u16 = 12;
/// Two pages side by side need this many stage rows: below it each page is too short to read.
pub const TWO_PAGE_ROWS: u16 = 24;
/// Rows the chrome takes: the header and its rule, the status rule and the status line.
pub const CHROME_ROWS: u16 = 4;
/// Nothing shows for this long while a chapter or a page is on its way.
pub const PLATE_AFTER: Duration = Duration::from_millis(600);
/// The wait's seconds join its line from here.
pub const SECONDS_AFTER: Duration = Duration::from_secs(4);
/// How long the terminal's own notices stay, as the engine's short ones do.
const NOTICE_MS: u32 = 5_000;
/// The width of a page before its picture is known, over its height.
const PAGE_RATIO: f32 = 2.0 / 3.0;

/// A decode handed out: the part, the file's path and crop it was made from, and the box in
/// pixels the picture was shrunk to fit.
type Decoding = (PartKey, String, Option<PixelRect>, (u32, u32));

/// No bound on a page's size, across or down.
pub const UNBOUNDED: (u32, u32) = (u32::MAX, u32::MAX);

/// Whether a picture `size` big, shrunk to fit `had`, comes out sharper decoded again to fit
/// `now`: it was held back by its box, and its shape fits the new box more than a pixel wider.
fn sharper(size: (u32, u32), had: (u32, u32), now: (u32, u32)) -> bool {
    let held = size.0.saturating_add(1) >= had.0 || size.1.saturating_add(1) >= had.1;
    let shape = f64::from(size.0) / f64::from(size.1.max(1));
    let fits = f64::from(now.0).min(f64::from(now.1) * shape);
    held && fits > f64::from(size.0) + 1.0
}

/// One part of a leaf, as its file is kept: a page, or one half of a split spread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartKey {
    pub page: u32,
    pub half: Option<PageHalf>,
}

/// A page file to decode off the loop, and the key its picture comes back under.
#[derive(Debug, Clone, PartialEq)]
pub struct PageJob {
    pub key: String,
    pub file: PageFile,
    /// Turned a quarter clockwise once cropped: a spread on its side on a one-page screen.
    pub rotate: bool,
    /// Cut to this half when the file holds the whole spread and no crop says where.
    pub half: Option<PageHalf>,
    /// The most pixels across and down the picture is drawn at, once cropped and turned: it is
    /// shrunk to fit, never grown.
    pub within: (u32, u32),
}

/// A notice on the status rule: the engine's, or the terminal's own.
#[derive(Debug, Clone, PartialEq)]
pub struct ShownNotice {
    pub text: String,
    /// The key that acts on it, with what it does: "g start over".
    pub action: Option<ReaderAction>,
    /// Gone after this; `None` stays until replaced.
    pub until: Option<Instant>,
}

/// What the page between chapters says, as the engine sends it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EndFacts {
    pub next: Option<ChapterRow>,
    pub why_none: Option<NoNext>,
    pub next_source: Option<String>,
    pub skip_to: Option<ChapterRow>,
    pub extras: u32,
    /// The trackers that took the read, once the chapter counted as read.
    pub read: Option<Vec<String>>,
    /// What became of the progress sent to the book server: "Saved to books.home.example".
    pub server: Option<String>,
    /// The next chapter's first page, decoded, with its size.
    pub preview: Option<(ChapterId, (u32, u32))>,
}

/// The reader's state while it has the window.
#[derive(Debug, Clone)]
pub struct Reader {
    pub work: WorkId,
    /// The series' name, from the screen it was opened from until the engine sends its own.
    pub title: String,
    /// The chapter asked for, and its number as the screen it was opened from knew it.
    pub asked: Option<ChapterId>,
    pub asked_number: Option<String>,
    /// The open session; 0 until `Opened`.
    pub session: u64,
    /// When the open, or the chapter in this session, was asked for; `None` once it opened.
    pub opening: Option<Instant>,
    /// The chapter the engine says it is opening, and what it waits on.
    pub opening_chapter: Option<ChapterRow>,
    pub wait: Option<ReaderWait>,
    pub view: Option<ReaderView>,
    pub leaves: Vec<Leaf>,
    /// The leaf in view.
    pub at: usize,
    /// The page a new layout keeps, while the leaf it was kept on is still in view.
    held_page: Option<u32>,
    /// When the leaf in view started waiting for its pictures.
    pub shown_at: Instant,
    files: HashMap<PartKey, PageFile>,
    /// Decoded pictures in the image store, with their size once cropped and turned.
    pub pictures: HashMap<PartKey, (u32, u32)>,
    /// The box each picture was shrunk to fit, to decode it again sharper when the box grows.
    within: HashMap<PartKey, (u32, u32)>,
    /// Keys handed out for decoding, with the part and the file each is for; `None` is the
    /// next preview.
    pub(super) decoding: HashMap<String, Option<Decoding>>,
    asked_pages: HashSet<u32>,
    /// Files that failed to decode and were asked for once more.
    asked_again: HashSet<String>,
    pub failures: HashMap<u32, ReadingFailure>,
    /// Why the chapter could not open; the screen says it in place of the page.
    pub chapter_failure: Option<ReadingFailure>,
    pub end: EndFacts,
    pub notice: Option<ShownNotice>,
    /// The page a jump left, while the engine offers Back to page.
    pub back_to: Option<u32>,
    /// The header and status line hidden.
    pub bare: bool,
    /// The surface last sent.
    pub surface: Option<ReaderSurface>,
    /// The surface the open went out with, to send again once the session is known.
    open_surface: Option<ReaderSurface>,
    pub settings: Option<ReaderSettingsView>,
    /// The settings were asked for with `,`, for the settings overlay to open on.
    pub settings_asked: bool,
    /// The computer is offline, as the screen it was opened from or the engine said.
    pub offline: bool,
    /// The series' chapters, oldest first, for `r` after the last one and `e`.
    pub chapters: Vec<ChapterRow>,
    /// At fit width, the row of the page at the top of the stage.
    pub scroll: u32,
    /// The page zoomed in, while it is.
    pub zoom: Option<Zoom>,
    /// A strip's place and the chapters it ran on from.
    pub strip: Strip,
    /// The chapter a strip ran on into, until its leaves come.
    pub flowing: Option<ChapterId>,
    /// The page to keep in view when the mode changed, for the engine's next layout.
    pub switched: Option<u32>,
    /// A light novel's text, once the engine sent it.
    pub novel: Option<super::NovelReading>,
}

impl Reader {
    fn opening(work: WorkId, title: String, asked: Option<ChapterId>, now: Instant) -> Self {
        Self {
            work,
            title,
            asked,
            asked_number: None,
            session: 0,
            opening: Some(now),
            opening_chapter: None,
            wait: None,
            view: None,
            leaves: Vec::new(),
            at: 0,
            held_page: None,
            shown_at: now,
            files: HashMap::new(),
            pictures: HashMap::new(),
            within: HashMap::new(),
            decoding: HashMap::new(),
            asked_pages: HashSet::new(),
            asked_again: HashSet::new(),
            failures: HashMap::new(),
            chapter_failure: None,
            end: EndFacts::default(),
            notice: None,
            back_to: None,
            bare: false,
            surface: None,
            open_surface: None,
            settings: None,
            settings_asked: false,
            offline: false,
            chapters: Vec::new(),
            scroll: 0,
            zoom: None,
            strip: Strip::default(),
            flowing: None,
            switched: None,
            novel: None,
        }
    }

    /// The open chapter's pages put away when a strip runs on into the next chapter: its
    /// pictures stay for the trail above, and the next chapter's come in their place.
    pub(super) fn forget_pages(&mut self) {
        self.leaves.clear();
        self.at = 0;
        self.held_page = None;
    }

    /// The chapter open, or the one opening.
    pub fn chapter(&self) -> Option<&ChapterRow> {
        self.view.as_ref().map(|v| &v.chapter).or(self.opening_chapter.as_ref())
    }

    /// The open chapter's number as the board says it, "14", else its label.
    pub fn chapter_number(&self) -> Option<String> {
        self.chapter()
            .map(|c| c.number.clone().unwrap_or_else(|| c.label.clone()))
            .or_else(|| self.asked_number.clone())
    }

    /// The series' name: the engine's once it opened.
    pub fn series_title(&self) -> &str {
        self.view.as_ref().map_or(self.title.as_str(), |v| v.title.as_str())
    }

    pub fn rtl(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.direction == ReadDirection::RightToLeft)
    }

    pub fn medium(&self) -> Option<Medium> {
        self.view.as_ref().map(|v| v.medium)
    }

    /// Still opening: nothing laid out to show yet and nothing failed.
    pub fn is_opening(&self) -> bool {
        self.chapter_failure.is_none()
            && self.flowing.is_none()
            && (self.view.is_none() || (self.leaves.is_empty() && self.novel.is_none()))
    }

    /// The leaf in view is the page between chapters.
    pub fn on_end(&self) -> bool {
        if let Some(novel) = &self.novel {
            return novel.end;
        }
        self.leaves.get(self.at).is_some_and(|l| l.end_page)
    }

    pub fn leaf(&self) -> Option<&Leaf> {
        self.leaves.get(self.at)
    }

    /// The story's pages, without the end page.
    pub fn story_pages(&self) -> u32 {
        self.leaves
            .iter()
            .filter(|l| !l.end_page)
            .flat_map(|l| l.parts.iter().map(|p| p.page + 1))
            .max()
            .unwrap_or(0)
    }

    /// The pages in view, from 1: one, or the first and last of two side by side.
    pub fn pages_in_view(&self) -> Option<(u32, Option<u32>)> {
        let leaf = self.leaf().filter(|l| !l.end_page)?;
        let first = leaf.parts.iter().map(|p| p.page).min()? + 1;
        let last = leaf.parts.iter().map(|p| p.page).max()? + 1;
        Some((first, (last != first).then_some(last)))
    }

    /// Two pages side by side in the leaf in view, or in any leaf of the chapter.
    pub fn shows_two(&self) -> bool {
        self.leaves.iter().any(|l| {
            let mut pages = l.parts.iter().map(|p| p.page);
            pages.next().is_some_and(|first| pages.any(|p| p != first))
        })
    }

    /// The picture of a part, by its key in the image store.
    pub fn picture_key(&self, part: PartKey) -> String {
        let chapter = self.chapter().map_or("", |c| c.id.0.as_str());
        let half = match part.half {
            None => "",
            Some(PageHalf::Left) => "l",
            Some(PageHalf::Right) => "r",
        };
        format!("reader/{}/{chapter}/{}{half}", self.session, part.page) // i18n-skip
    }

    pub fn preview_key(&self, chapter: &ChapterId) -> String {
        format!("reader/{}/next/{}", self.session, chapter.0) // i18n-skip
    }

    /// The part's file, once its page is made.
    pub fn file(&self, part: PartKey) -> Option<&PageFile> {
        self.files.get(&part)
    }

    /// A page's width over its height, for a plate the size the page will be: its picture,
    /// its file, the engine's word, or a page's usual shape.
    pub fn part_size(&self, part: &kaeri_model::reader::LeafPart) -> (f32, f32) {
        let key = PartKey { page: part.page, half: part.half };
        if let Some((w, h)) = self.pictures.get(&key) {
            return (*w as f32, *h as f32);
        }
        let from_file = self.files.get(&key).and_then(|f| {
            let (w, h) = f.crop.map_or((f.width, f.height), |c| (c.width, c.height));
            (w > 0 && h > 0).then_some((w as f32, h as f32))
        });
        let (w, h) = from_file.unwrap_or_else(|| {
            let ratio = part.ratio.filter(|r| *r > 0.0).unwrap_or(PAGE_RATIO);
            (ratio * 1000.0, 1000.0)
        });
        if part.rotate && from_file.is_some() { (h, w) } else { (w, h) }
    }

    /// The usual page's width over its height, for whether two fit side by side.
    fn page_ratio(&self) -> f32 {
        self.leaves
            .iter()
            .flat_map(|l| &l.parts)
            .find(|p| !p.rotate && p.half.is_none())
            .map(|p| {
                let (w, h) = self.part_size(p);
                w / h
            })
            .unwrap_or(PAGE_RATIO)
    }

    /// The layout the page count is set to, as the settings say: auto, single or double.
    fn page_layout(&self) -> String {
        let rows =
            self.settings.as_ref().or(self.view.as_ref().and_then(|v| v.settings.as_ref()));
        rows.and_then(|s| s.rows.iter().find(|r| r.key == ReaderSettingKey::PageLayout))
            .and_then(|r| match &r.value {
                ReaderValue::Choice { name } => Some(name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "auto".to_owned()) // i18n-skip: a setting's value
    }

    /// The leaves to have pages for around `index`: it, two on and one back.
    fn wanted(&self, index: usize) -> Vec<usize> {
        let count = self.leaves.len();
        if self.strips() && !self.strip.near.is_empty() {
            return self.strip.near.iter().copied().filter(|i| *i < count).collect();
        }
        let ahead = index..(index + 3).min(count);
        ahead.chain(index.checked_sub(1).filter(|i| *i < count)).collect()
    }

    /// The page a new layout keeps: the one kept last time while its leaf is in view, else the
    /// leaf's first.
    fn kept_page(&self) -> Option<u32> {
        let leaf = self.leaf()?;
        self.held_page
            .filter(|held| leaf.parts.iter().any(|p| p.page == *held))
            .or_else(|| leaf.parts.iter().map(|p| p.page).min())
    }

    fn leaf_of(&self, page: u32) -> Option<usize> {
        self.leaves.iter().position(|l| l.parts.iter().any(|p| p.page == page))
    }

    fn last_page_leaf(&self) -> usize {
        self.leaves.iter().rposition(|l| !l.end_page).unwrap_or(0)
    }

    /// Whether the read of the open chapter is counted: the engine said so, or its row did.
    fn is_read(&self) -> bool {
        let Some(chapter) = self.chapter() else { return false };
        let row = self.chapters.iter().find(|c| c.id == chapter.id).unwrap_or(chapter);
        row.state == ChapterReadState::Read
    }
}

/// What the reader's screen shows, from its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReaderScreen {
    TooSmall,
    Opening,
    ChapterFailed,
    ImagesOff,
    Pages,
    End,
}

impl App {
    /// The reader, while it has the window.
    pub fn reader_here(&self) -> Option<&Reader> {
        match self.nav.current() {
            StageView::Reader(work) => self.reader.as_ref().filter(|r| r.work == *work),
            _ => None,
        }
    }

    pub(super) fn reader_mut(&mut self) -> Option<&mut Reader> {
        match self.nav.current() {
            StageView::Reader(work) => {
                let work = work.clone();
                self.reader.as_mut().filter(|r| r.work == work)
            }
            _ => None,
        }
    }

    /// Which of the reader's screens is up.
    pub fn reader_screen(&self) -> Option<ReaderScreen> {
        let reader = self.reader_here()?;
        let (w, h) = self.window.unwrap_or((80, 24));
        Some(if w < MIN_COLUMNS || h < MIN_ROWS {
            ReaderScreen::TooSmall
        } else if reader.chapter_failure.is_some() {
            ReaderScreen::ChapterFailed
        } else if reader.is_opening() {
            ReaderScreen::Opening
        } else if !self.images.engine().is_enabled() && reader.medium() != Some(Medium::Novel) {
            ReaderScreen::ImagesOff
        } else if reader.on_end() {
            ReaderScreen::End
        } else {
            ReaderScreen::Pages
        })
    }

    /// Open the reader on `work`: at `chapter`, or where reading stopped when it is none.
    ///
    /// The one way into the reader, from a Continue line on Home and from the series page. The
    /// reader takes the window over the screen it came from, which Back returns to with its
    /// cursor where it was. Offline, a chapter that isn't on this computer says so instead.
    pub(super) fn open_reader(
        &mut self,
        work: WorkId,
        chapter: Option<ChapterId>,
    ) -> Option<Command> {
        let page = self.series_here().filter(|p| p.work == work);
        let row = chapter
            .as_ref()
            .and_then(|id| page.and_then(|p| p.rows().iter().find(|r| r.id == *id)))
            .cloned();
        let offline = page.is_some_and(super::SeriesPage::offline) || self.offline;
        if offline && let Some(row) = row.as_ref().filter(|r| !downloaded(r)) {
            let number = row.number.clone().unwrap_or_else(|| row.label.clone());
            self.push_toast(info(t::tui_reader_end_offline(&number)));
            return None;
        }
        let title = self.reader_title(&work);
        let now = self.clock.now_instant();
        self.images.forget_prefix("reader/"); // i18n-skip: a key prefix
        let mut reader = Reader::opening(work.clone(), title, chapter.clone(), now);
        reader.asked_number = row.as_ref().map(|r| r.number.clone().unwrap_or(r.label.clone()));
        reader.offline = offline;
        if let Some(page) = page {
            let mut rows = page.rows().to_vec();
            rows.reverse();
            reader.chapters = rows;
        }
        // Before the first frame, or while the window is too small: a classic terminal's stage,
        // sent again as soon as a frame says the window's size.
        let surface =
            self.surface_for(&reader).unwrap_or_else(|| self.surface_in(&reader, (80, 24)));
        reader.surface = Some(surface);
        reader.open_surface = Some(surface);
        self.reader = Some(reader);
        self.nav.push(StageView::Reader(work.clone()));
        Some(Command::Reader(ReaderCommand::Open {
            work,
            chapter,
            at: OpenAt::Resume,
            surface,
        }))
    }

    /// The series' name as the screen it is opened from has it.
    fn reader_title(&self, work: &WorkId) -> String {
        if let Some(view) = self
            .reading
            .series
            .as_ref()
            .filter(|p| p.work == *work)
            .and_then(|p| p.view.as_ref())
        {
            return view.titles.display.clone();
        }
        self.read_items()
            .into_iter()
            .find_map(|line| match line {
                super::ReadLine::Work { work: w, title, .. } if w == *work => Some(title),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// The most pixels across and down a page is drawn at in the window the last frame was
    /// drawn in: the stage, times the zoom when zoomed in; at fit width the stage's width and a
    /// strip's column, however long the page. A page is shrunk to fit before it is kept.
    pub(super) fn page_within(&self, reader: &Reader) -> (u32, u32) {
        let Some((w, h)) = self.window else { return UNBOUNDED };
        let surface = self.surface_in(reader, (w, h));
        if reader.strips() {
            let per_column = surface.width_px / u32::from(w.max(1));
            let cols = crate::app::strip_cols(reader, w);
            return (u32::from(cols) * per_column, u32::MAX);
        }
        if let Some(zoom) = reader.zoom {
            let level = zoom.level.max(1.0);
            let grown = |px: u32| (px as f32 * level).ceil() as u32;
            return (grown(surface.width_px), grown(surface.height_px));
        }
        if reader.fits_width() {
            return (surface.width_px, u32::MAX);
        }
        (surface.width_px, surface.height_px)
    }

    /// The surface for the window the last frame was drawn in, `None` while it is too small.
    fn surface_for(&self, reader: &Reader) -> Option<ReaderSurface> {
        let (w, h) = self.window?;
        (w >= MIN_COLUMNS && h >= MIN_ROWS).then(|| self.surface_in(reader, (w, h)))
    }

    /// The stage of a `w` by `h` window in pixels: the cells times the font for an image
    /// protocol, one pixel across and two down for halfblocks.
    fn surface_in(&self, reader: &Reader, (w, h): (u16, u16)) -> ReaderSurface {
        let rows = if reader.bare { h } else { h.saturating_sub(CHROME_ROWS) };
        let engine = self.images.engine();
        let (width_px, height_px) = match engine.graphics() {
            Graphics::Native => {
                let (fw, fh) = engine.font_size();
                (u32::from(w) * u32::from(fw), u32::from(rows) * u32::from(fh))
            }
            // Halfblocks put two pixels in a cell, one over the other.
            Graphics::Halfblocks | Graphics::Disabled => (u32::from(w), u32::from(rows) * 2),
        };
        // A strip and a page at fit width show one page across.
        let two_pages = rows >= TWO_PAGE_ROWS
            && !reader.strips()
            && reader.choice(ReaderSettingKey::Fit) != Some("width") // i18n-skip: a value
            && 2.0 * height_px as f32 * reader.page_ratio() <= width_px as f32;
        ReaderSurface {
            width_px,
            height_px,
            scale: 1.0,
            eink: false,
            two_pages,
            dark: !matches!(self.palette.variant, crate::theme::Variant::Light),
            decodes: Some(PageFormats {
                jpeg: true,
                png: true,
                webp: true,
                gif: true,
                avif: false,
            }),
            zoom: reader.zoom.map_or(1.0, |z| z.level),
            text: true,
        }
    }

    /// The window the last frame was drawn in. In the reader, a new surface goes to the engine
    /// when it changes the stage, so the chapter is laid out again at the same place.
    pub(super) fn reader_window(&mut self, window: (u16, u16)) {
        self.keep_strip_place(window);
        self.window = Some(window);
        self.send_surface();
        self.novel_fit();
    }

    /// Send the surface again when it changed: a resize, the chrome hidden or shown, one page
    /// or two. Nothing goes while the window is too small; the place is kept for when it grows.
    pub(super) fn send_surface(&mut self) {
        // A larger box, at fit width or zoomed in, wants its pages decoded sharper.
        self.decode_in_view();
        let Some(reader) = self.reader_here() else { return };
        let Some(surface) = self.surface_for(reader) else { return };
        if reader.surface.as_ref() == Some(&surface) {
            return;
        }
        let session = reader.session;
        if let Some(reader) = self.reader_mut() {
            reader.surface = Some(surface);
        }
        if session != 0 {
            self.commands.push(Command::Reader(ReaderCommand::Surface { session, surface }));
        }
    }

    /// An event of the reader's.
    pub(super) fn apply_reader(&mut self, event: ReaderEvent) {
        let now = self.clock.now_instant();
        if let ReaderEvent::Opened(view) = &event {
            let open = self.reader.as_ref().is_some_and(|r| r.work == view.work)
                && self.reader_here().is_some();
            if !open {
                // Left before the chapter opened: the session that opened late is closed now.
                self.commands
                    .push(Command::Reader(ReaderCommand::Close { session: view.session }));
                return;
            }
        }
        // The box a page is drawn in, for a next chapter's first page.
        let within = self.reader_here().map_or(UNBOUNDED, |r| self.page_within(r));
        let images = &self.images;
        let Some(reader) = self.reader.as_mut() else { return };
        let mine = |session: u64| session != 0 && session == reader.session;
        let current = |reader: &Reader, chapter: &ChapterId| {
            reader.view.as_ref().is_some_and(|v| v.chapter.id == *chapter)
        };
        match event {
            ReaderEvent::Opened(view) => {
                let fresh = reader.session != view.session
                    || reader.view.as_ref().is_none_or(|v| v.chapter.id != view.chapter.id);
                // A strip that ran on into this chapter keeps the pictures of the one above.
                let flowed = reader.flowing.as_ref() == Some(&view.chapter.id);
                if fresh && !flowed {
                    images.forget_prefix("reader/"); // i18n-skip: a key prefix
                    reader.strip = Strip::default();
                    reader.zoom = None;
                    reader.scroll = 0;
                    reader.flowing = None;
                }
                if fresh {
                    reader.leaves.clear();
                    reader.files.clear();
                    reader.pictures.clear();
                    reader.within.clear();
                    if !flowed {
                        reader.decoding.clear();
                    }
                    reader.asked_pages.clear();
                    reader.asked_again.clear();
                    reader.failures.clear();
                    reader.end = EndFacts::default();
                    reader.back_to = None;
                    reader.at = 0;
                    reader.held_page = None;
                    reader.novel = None;
                }
                reader.session = view.session;
                reader.opening = None;
                reader.wait = None;
                reader.chapter_failure = None;
                reader.end.next = view.next.clone();
                reader.settings = view.settings.clone().or(reader.settings.take());
                reader.view = Some(*view);
                let work = reader.work.clone();
                // The open went out before the session was known; the window may have changed.
                let sent = reader.open_surface.take();
                if sent.is_some()
                    && sent != reader.surface
                    && let Some(surface) = reader.surface
                {
                    let session = reader.session;
                    self.commands
                        .push(Command::Reader(ReaderCommand::Surface { session, surface }));
                }
                // The series' chapters, for `e` and for reading again from the start.
                self.commands.push(Command::Reading(ReadingCommand::LoadChapters {
                    work,
                    edition: None,
                }));
                self.say_blocks_once();
                self.send_surface();
            }
            ReaderEvent::Opening { chapter, wait } => {
                if reader.opening.is_some() || reader.view.is_none() {
                    if reader.wait != wait {
                        reader.wait = wait;
                    }
                    if chapter.is_some() {
                        reader.opening_chapter = chapter;
                    }
                }
            }
            ReaderEvent::Leaves { session, chapter, leaves } if mine(session) => {
                if current(reader, &chapter) {
                    self.take_leaves(leaves);
                }
            }
            ReaderEvent::PageReady { session, chapter, page, half, file } if mine(session) => {
                if current(reader, &chapter) {
                    let key = PartKey { page, half };
                    let old = reader.files.insert(key, file.clone());
                    // The picture from the old file shows until the new one is decoded.
                    if old.is_some_and(|old| old.path != file.path || old.crop != file.crop) {
                        reader.pictures.remove(&key);
                    }
                    reader.failures.remove(&page);
                    self.decode_in_view();
                }
            }
            ReaderEvent::PageFailed { session, chapter, page, failure } if mine(session) => {
                if current(reader, &chapter) {
                    reader.failures.insert(page, failure);
                }
            }
            ReaderEvent::EndPage { session, next, why_none, next_source, skip_to, extras }
                if mine(session) =>
            {
                reader.end.next = next;
                reader.end.why_none = why_none;
                reader.end.next_source = next_source;
                reader.end.skip_to = skip_to;
                reader.end.extras = extras;
                if why_none == Some(NoNext::Offline) {
                    reader.offline = true;
                }
            }
            ReaderEvent::ChapterRead { work, chapter, trackers } => {
                if reader.work == work && current(reader, &chapter) {
                    reader.end.read = Some(trackers.clone());
                    if let Some(view) = reader.view.as_mut() {
                        view.chapter.state = ChapterReadState::Read;
                    }
                }
                if reader.work == work {
                    if let Some(row) = reader.chapters.iter_mut().find(|c| c.id == chapter) {
                        row.state = ChapterReadState::Read;
                    }
                    self.strip_read(&chapter, &trackers);
                }
            }
            ReaderEvent::ProgressSent { work, chapter, host, result, .. } => {
                if reader.work == work && current(reader, &chapter) {
                    reader.end.server = server_line(&result, &host);
                }
            }
            ReaderEvent::NextPreview { session, chapter, file } if mine(session) => {
                let key = reader.preview_key(&chapter);
                reader.decoding.insert(key.clone(), None);
                reader.end.preview = None;
                self.page_jobs.push(PageJob { key, file, rotate: false, half: None, within });
            }
            ReaderEvent::Notice(notice) if reader.session != 0 || reader.opening.is_some() => {
                // The blocks notice gives way, and comes back once this one has gone.
                let blocks = t::tui_reader_blocks_notice();
                let live = reader.notice.as_ref().filter(|n| n.until.is_none_or(|u| u > now));
                if live.is_some_and(|n| n.text == blocks) {
                    self.blocks_owed = true;
                }
                reader.notice = shown_notice(&notice, now);
            }
            ReaderEvent::BackToPage { session, page } if mine(session) => {
                reader.back_to = page;
            }
            ReaderEvent::NovelOpen(open) if mine(open.session) => self.novel_open(open),
            ReaderEvent::GoTo { session, place } if mine(session) && reader.novel.is_some() => {
                if let (Some(pos), Some(novel)) = (place.novel, reader.novel.as_mut()) {
                    novel.go_to(pos);
                    self.novel_fit();
                }
            }
            ReaderEvent::GoTo { session, place } if mine(session) => {
                if let Some(at) = place.page.and_then(|page| reader.leaf_of(page))
                    && let Some(task) = self.show_leaf(at, TurnHow::Placed)
                {
                    self.commands.push(task);
                }
            }
            ReaderEvent::Settings(view) if reader.medium().is_none_or(|m| m == view.medium) => {
                reader.settings = Some(view);
                self.follow_settings();
                self.novel_fit();
            }
            _ => {}
        }
    }

    /// The chapter could not open, before or after `Opened`: said in place of the page, with
    /// the whole error in Recent messages.
    pub(super) fn reader_failed(&mut self, failure: ReadingFailure) {
        let Some(reader) = self.reader_mut() else { return };
        reader.opening = None;
        reader.wait = None;
        reader.chapter_failure = Some(failure.clone());
        let words = say(&failure.msg());
        let chapter = reader.chapter_number();
        let heading = chapter.map_or_else(String::new, |c| t::tui_reader_chapter_failed(&c));
        self.log(NoticeKind::Alert, format!("{heading}. {words}")); // i18n-skip: two sentences
    }

    /// The series' chapters, oldest first, for the reader on it.
    pub(super) fn reader_chapters(&mut self, work: &WorkId, volumes: &[VolumeGroup]) {
        if let Some(reader) = self.reader.as_mut().filter(|r| r.work == *work) {
            reader.chapters =
                volumes.iter().flat_map(|v| v.chapters.iter().cloned()).collect::<Vec<_>>();
        }
    }

    /// The device went offline or came back, as the engine says.
    pub(super) fn reader_connectivity(&mut self, offline: bool) {
        self.offline = offline;
        if let Some(reader) = self.reader.as_mut() {
            reader.offline = offline;
        }
    }

    /// The first chapter opened in a run without an image protocol says once that pages show
    /// as blocks: at once, or once the notice up has gone.
    fn say_blocks_once(&mut self) {
        if self.blocks_said || self.images.engine().graphics() != Graphics::Halfblocks {
            return;
        }
        if self.reader.as_ref().is_none_or(|r| r.medium() == Some(Medium::Novel)) {
            return;
        }
        self.blocks_said = true;
        self.blocks_owed = true;
        self.pay_blocks();
    }

    /// The blocks notice, when it is owed and the status rule is free. The resumed notice says
    /// where the chapter opened, which matters more, so the blocks notice waits for it.
    pub(super) fn pay_blocks(&mut self) {
        if !self.blocks_owed {
            return;
        }
        let now = self.clock.now_instant();
        let Some(reader) = self.reader.as_mut() else { return };
        if reader.notice.as_ref().is_some_and(|n| n.until.is_none_or(|u| u > now)) {
            return;
        }
        self.blocks_owed = false;
        reader.notice = Some(ShownNotice {
            text: t::tui_reader_blocks_notice(),
            action: None,
            until: Some(now + Duration::from_millis(u64::from(NOTICE_MS))),
        });
    }

    /// The chapter's leaves, first or laid out again: the page in view stays in view.
    fn take_leaves(&mut self, leaves: Vec<Leaf>) {
        // A strip's place, from the layout before this one.
        let strip_place = self.strip_place();
        let Some(reader) = self.reader.as_mut() else { return };
        for part in leaves.iter().flat_map(|l| &l.parts) {
            if let Some(file) = &part.file {
                let key = PartKey { page: part.page, half: part.half };
                let old = reader.files.insert(key, file.clone());
                // The picture from the old file shows until the new one is decoded.
                if old.is_some_and(|old| old.path != file.path || old.crop != file.crop) {
                    reader.pictures.remove(&key);
                }
            }
        }
        let first = reader.leaves.is_empty();
        let switched = reader.switched.take();
        if reader.strips() {
            reader.leaves = leaves;
            let kept = switched.map(|page| (page, 0.0)).or(strip_place);
            self.place_strip(first, kept);
            self.send_surface();
            return;
        }
        let start = if first {
            reader.view.as_ref().and_then(|v| v.start.page)
        } else {
            switched.or(reader.kept_page())
        };
        let index = start
            .and_then(|p| leaves.iter().position(|l| l.parts.iter().any(|part| part.page == p)))
            .unwrap_or(0)
            .min(leaves.len().saturating_sub(1));
        reader.held_page = if first { None } else { start };
        reader.leaves = leaves;
        // The first layout, or a new one: the reader put the place there, not the person.
        let held = reader.held_page;
        if let Some(task) = self.show_leaf(index, TurnHow::Placed) {
            self.commands.push(task);
        }
        if let Some(reader) = self.reader.as_mut() {
            reader.held_page = held;
        }
        self.send_surface();
    }

    /// Put leaf `index` in view: its pages asked for and decoded, and the engine told.
    fn show_leaf(&mut self, index: usize, how: TurnHow) -> Option<Command> {
        let now = self.clock.now_instant();
        let reader = self.reader.as_mut()?;
        if index >= reader.leaves.len() {
            return None;
        }
        if reader.at != index {
            reader.held_page = None;
            reader.shown_at = now;
            reader.scroll = 0;
        }
        reader.at = index;
        let session = reader.session;
        let chapter = reader.view.as_ref()?.chapter.id.clone();
        let wanted = reader.wanted(index);
        self.ask_pages(wanted);
        self.decode_in_view();
        Some(Command::Reader(ReaderCommand::Shown {
            session,
            chapter,
            leaf: index as u32,
            how,
        }))
    }

    /// Ask the engine for the pages of `leaves` it has not sent, once each.
    pub(super) fn ask_pages(&mut self, leaves: Vec<usize>) {
        if !self.images.engine().is_enabled() {
            return;
        }
        let Some(reader) = self.reader.as_mut() else { return };
        let session = reader.session;
        let Some(chapter) = reader.view.as_ref().map(|v| v.chapter.id.clone()) else { return };
        {
            let pages: Vec<u32> = leaves
                .into_iter()
                .filter_map(|i| reader.leaves.get(i))
                .flat_map(|l| l.parts.iter())
                .filter(|p| {
                    !reader.files.contains_key(&PartKey { page: p.page, half: p.half })
                        && p.file.is_none()
                })
                .map(|p| p.page)
                .filter(|p| !reader.asked_pages.contains(p))
                .collect();
            for page in pages {
                reader.asked_pages.insert(page);
                self.commands.push(Command::Reader(ReaderCommand::NeedPage {
                    session,
                    chapter: chapter.clone(),
                    page,
                }));
            }
        }
    }

    /// Hand the files of the leaves around the one in view to be decoded, once each.
    pub(super) fn decode_in_view(&mut self) {
        if !self.images.engine().is_enabled() {
            return;
        }
        let Some(within) = self.reader_here().map(|r| self.page_within(r)) else { return };
        let Some(reader) = self.reader.as_mut() else { return };
        let wanted = reader.wanted(reader.at);
        let parts: Vec<(PartKey, bool)> = wanted
            .into_iter()
            .filter_map(|i| reader.leaves.get(i))
            .flat_map(|l| l.parts.iter())
            .map(|p| (PartKey { page: p.page, half: p.half }, p.rotate))
            .collect();
        for (key, rotate) in parts {
            let had = reader.within.get(&key).copied().unwrap_or(UNBOUNDED);
            if reader.pictures.get(&key).is_some_and(|size| !sharper(*size, had, within)) {
                continue;
            }
            let Some(file) = reader.files.get(&key).cloned() else { continue };
            let image_key = reader.picture_key(key);
            // One decode of a picture at a time; one of an older file asks again when it lands.
            if reader.decoding.contains_key(&image_key) {
                continue;
            }
            reader
                .decoding
                .insert(image_key.clone(), Some((key, file.path.clone(), file.crop, within)));
            let half = key.half;
            self.page_jobs.push(PageJob { key: image_key, file, rotate, half, within });
        }
    }

    /// The page files to decode, for whoever runs the app to do off the loop.
    pub fn take_page_jobs(&mut self) -> Vec<PageJob> {
        std::mem::take(&mut self.page_jobs)
    }

    /// A page's picture, decoded, cropped and turned as its job said; `None` when the file
    /// could not be read. A file that fails is asked for once more, then the page fails.
    pub fn page_decoded(&mut self, key: &str, image: Option<image::DynamicImage>) {
        let Some(reader) = self.reader.as_mut() else { return };
        let Some(part) = reader.decoding.remove(key) else { return };
        // A picture of a chapter the strip ran on from goes on showing in the trail.
        if let Some((part, _, _, _)) = part
            && reader.picture_key(part) != key
        {
            if let Some(image) = image {
                self.images.insert_page(key, image);
            }
            return;
        }
        match (part, image) {
            (None, Some(image)) => {
                let size = (image.width(), image.height());
                self.images.insert_page(key, image);
                let next = reader.end.next.as_ref().map(|n| n.id.clone());
                if let Some(next) = next.filter(|n| reader.preview_key(n) == key) {
                    reader.end.preview = Some((next, size));
                }
            }
            (None, None) => {}
            (Some((part, path, crop, within)), Some(image)) => {
                let size = (image.width(), image.height());
                self.images.insert_page(key, image);
                // Decoded from a file since replaced, at another zoom: shown until the new one is.
                let stale =
                    reader.files.get(&part).is_some_and(|f| f.path != path || f.crop != crop);
                if stale {
                    self.decode_in_view();
                } else {
                    reader.pictures.insert(part, size);
                    reader.within.insert(part, within);
                }
            }
            (Some((part, _, _, _)), None) => {
                let path = reader.files.get(&part).map(|f| f.path.clone()).unwrap_or_default();
                let session = reader.session;
                let chapter = reader.view.as_ref().map(|v| v.chapter.id.clone());
                if reader.asked_again.insert(path)
                    && let Some(chapter) = chapter
                {
                    reader.files.remove(&part);
                    self.commands.push(Command::Reader(ReaderCommand::NeedPage {
                        session,
                        chapter,
                        page: part.page,
                    }));
                } else {
                    let file = reader.files.get(&part);
                    let format = file.map(|f| match &f.encoding {
                        PageEncoding::Original { mime } => mime.clone(),
                        PageEncoding::Raw { .. } => String::new(),
                    });
                    let path = file.map(|f| f.path.clone()).unwrap_or_default();
                    let failure =
                        ReadingFailure::Unreadable { path, format: format.unwrap_or_default() };
                    reader.failures.insert(part.page, failure);
                }
            }
        }
    }

    /// Leave the reader for the screen it was opened from, its cursor where it was.
    fn leave_reader(&mut self) -> Option<Command> {
        let reader = self.reader.take()?;
        self.images.forget_prefix("reader/"); // i18n-skip: a key prefix
        self.page_jobs.clear();
        if matches!(self.nav.current(), StageView::Reader(_)) {
            self.nav.pop();
        }
        Some(Command::Reader(ReaderCommand::Close { session: reader.session }))
    }

    /// A key while the reader has the window, ahead of everything else. `None` lets it through.
    pub(super) fn reader_key(&mut self, action: Action) -> Option<Option<Command>> {
        let screen = self.reader_screen()?;
        // The keys the reader keeps from the rest of the app: help, the palette, logs, quit.
        if Action::IN_READER.contains(&action) {
            return None;
        }
        if let Some(task) = self.novel_key(screen, action) {
            return Some(task);
        }
        if let Some(task) = self.mode_key(screen, action) {
            return Some(task);
        }
        let rtl = self.reader_here()?.rtl();
        Some(match (screen, action) {
            (_, Action::ReaderLeave) => self.leave_reader(),
            (ReaderScreen::TooSmall | ReaderScreen::Opening, _) => None,
            (ReaderScreen::ChapterFailed, Action::ReaderAgain) => self.open_again(),
            (_, Action::ReaderChapters) => self.reader_to_series(),
            (ReaderScreen::ChapterFailed | ReaderScreen::ImagesOff, _) => None,
            (_, Action::ReaderNext) => self.turn(true),
            (_, Action::ReaderPrevious) => self.turn(false),
            // As the page moves: in a right-to-left book the page to the left is the next one.
            (_, Action::ReaderLeft) => self.turn(rtl),
            (_, Action::ReaderRight) => self.turn(!rtl),
            (_, Action::ReaderFirst) => self.first_page(),
            (_, Action::ReaderLast) => {
                let last = self.reader_here()?.last_page_leaf();
                self.show_leaf(last, TurnHow::Jump)
            }
            (_, Action::ReaderNextChapter) => {
                let next = self.reader_here()?.end.next.clone();
                next.and_then(|n| self.open_chapter(n.id))
            }
            (_, Action::ReaderPreviousChapter) => {
                let previous = self.reader_here()?.view.as_ref()?.previous.clone();
                previous.and_then(|p| self.open_chapter(p.id))
            }
            (_, Action::ReaderBackToPage) => {
                let reader = self.reader_mut()?;
                reader.back_to.take()?;
                Some(Command::Reader(ReaderCommand::BackTo { session: reader.session }))
            }
            (_, Action::ReaderMarkRead) => self.reader_mark_read(),
            (_, Action::ReaderDownload) => {
                let reader = self.reader_here()?;
                let id = reader.chapter()?.id.clone();
                Some(Command::Reading(ReadingCommand::Download {
                    work: reader.work.clone(),
                    pick: DownloadPick::Chapters { ids: vec![id] },
                }))
            }
            (ReaderScreen::End, Action::ReaderBare)
                if self.reader_here()?.end.next.is_some() =>
            {
                let next = self.reader_here()?.end.next.clone()?;
                self.open_chapter(next.id)
            }
            (_, Action::ReaderBare) => {
                let reader = self.reader_mut()?;
                reader.bare = !reader.bare;
                self.send_surface();
                self.novel_fit();
                None
            }
            (_, Action::ReaderPages) => self.switch_pages(),
            (_, Action::ReaderMode) => self.switch_mode(),
            (ReaderScreen::Pages, Action::ReaderFit) => self.switch_fit(),
            (ReaderScreen::Pages, Action::ReaderZoom) => self.toggle_zoom(),
            (ReaderScreen::Pages, Action::ReaderZoomIn) => self.step_zoom(true),
            (ReaderScreen::Pages, Action::ReaderZoomOut) => self.step_zoom(false),
            (_, Action::ReaderSettings) => self.open_reader_settings(),
            (ReaderScreen::End, Action::ReaderAgain) => self.read_again(),
            (_, Action::ReaderAgain) => self.retry_page(),
            (ReaderScreen::End, Action::ReaderSkip) => {
                let skip = self.reader_here()?.end.skip_to.clone()?;
                self.open_chapter(skip.id)
            }
            _ => None,
        })
    }

    /// One leaf on or back. Forward from the end page goes on to the next chapter; back from
    /// the first page goes to the chapter before, as the engine decides.
    pub(super) fn turn(&mut self, forward: bool) -> Option<Command> {
        let reader = self.reader_here()?;
        if reader.leaves.is_empty() {
            return None;
        }
        let (at, count, session) = (reader.at, reader.leaves.len(), reader.session);
        if reader.on_end() && forward {
            let next = reader.end.next.clone()?;
            return self.open_chapter(next.id);
        }
        if forward {
            (at + 1 < count).then(|| self.show_leaf(at + 1, TurnHow::Key)).flatten()
        } else if at > 0 {
            self.show_leaf(at - 1, TurnHow::Key)
        } else {
            Some(Command::Reader(ReaderCommand::GoBack { session }))
        }
    }

    /// `g`: the chapter from its start, as the resumed notice offers, else its first page.
    fn first_page(&mut self) -> Option<Command> {
        let now = self.clock.now_instant();
        let reader = self.reader_mut()?;
        let offered = reader.notice.as_ref().is_some_and(|n| {
            n.action == Some(ReaderAction::StartOver) && n.until.is_none_or(|u| u > now)
        });
        if offered {
            reader.notice = None;
            return Some(Command::Reader(ReaderCommand::StartOver { session: reader.session }));
        }
        self.show_leaf(0, TurnHow::Jump)
    }

    /// Another chapter in the open session.
    pub(super) fn open_chapter(&mut self, chapter: ChapterId) -> Option<Command> {
        let now = self.clock.now_instant();
        let reader = self.reader_mut()?;
        let session = reader.session;
        if session == 0 {
            return None;
        }
        reader.opening = Some(now);
        reader.wait = None;
        reader.opening_chapter = reader.chapters.iter().find(|c| c.id == chapter).cloned();
        reader.asked_number = reader
            .opening_chapter
            .as_ref()
            .map(|c| c.number.clone().unwrap_or(c.label.clone()));
        reader.notice = None;
        reader.leaves.clear();
        reader.view = None;
        reader.novel = None;
        reader.asked = Some(chapter.clone());
        Some(Command::Reader(ReaderCommand::OpenChapter { session, chapter }))
    }

    /// `r` on a chapter that could not open: the open again, as it was asked.
    fn open_again(&mut self) -> Option<Command> {
        let now = self.clock.now_instant();
        let surface = self.reader_here().and_then(|r| self.surface_for(r));
        let reader = self.reader_mut()?;
        reader.chapter_failure = None;
        reader.opening = Some(now);
        reader.wait = None;
        let chapter =
            reader.asked.clone().or(reader.view.as_ref().map(|v| v.chapter.id.clone()));
        if reader.session != 0 {
            return Some(Command::Reader(ReaderCommand::Retry {
                session: reader.session,
                chapter: chapter?,
                page: None,
            }));
        }
        let surface = surface.or(reader.surface)?;
        reader.open_surface = Some(surface);
        Some(Command::Reader(ReaderCommand::Open {
            work: reader.work.clone(),
            chapter,
            at: OpenAt::Resume,
            surface,
        }))
    }

    /// `r` on a page that could not load: asked again; the turn key goes on without it.
    fn retry_page(&mut self) -> Option<Command> {
        let reader = self.reader_mut()?;
        let leaf = reader.leaves.get(reader.at)?;
        let page =
            leaf.parts.iter().map(|p| p.page).find(|p| reader.failures.contains_key(p))?;
        reader.failures.remove(&page);
        reader.asked_pages.insert(page);
        let chapter = reader.view.as_ref()?.chapter.id.clone();
        Some(Command::Reader(ReaderCommand::Retry {
            session: reader.session,
            chapter,
            page: Some(page),
        }))
    }

    /// `r` after the last chapter of a finished series: its first chapter.
    fn read_again(&mut self) -> Option<Command> {
        let reader = self.reader_here()?;
        if reader.end.why_none != Some(NoNext::Finished) {
            return None;
        }
        let first = reader.chapters.first().map(|c| c.id.clone());
        match first {
            Some(first) => self.open_chapter(first),
            None => {
                let session = reader.session;
                Some(Command::Reader(ReaderCommand::StartOver { session }))
            }
        }
    }

    /// `u`: the open chapter read, or unread again.
    fn reader_mark_read(&mut self) -> Option<Command> {
        let reader = self.reader_mut()?;
        let read = !reader.is_read();
        let view = reader.view.as_mut()?;
        view.chapter.state =
            if read { ChapterReadState::Read } else { ChapterReadState::Unread };
        let id = view.chapter.id.clone();
        if let Some(row) = reader.chapters.iter_mut().find(|c| c.id == id) {
            row.state = view.chapter.state;
        }
        Some(Command::Reading(ReadingCommand::MarkRead {
            work: reader.work.clone(),
            chapters: vec![id],
            read,
        }))
    }

    /// `e`: the series page, with the cursor on the open chapter. Opened from it, Back to it;
    /// from Home, the page opens in the reader's place.
    fn reader_to_series(&mut self) -> Option<Command> {
        let chapter = self.reader_here()?.chapter().map(|c| c.id.clone());
        let work = self.reader_here()?.work.clone();
        let close = self.leave_reader();
        if let Some(close) = close {
            self.commands.push(close);
        }
        let task = if self.series_here().is_some_and(|p| p.work == work) {
            None
        } else {
            self.open_series(work)
        };
        if let Some(chapter) = chapter
            && let Some(at) = self.series_here().and_then(|p| p.position(&chapter))
        {
            self.cursors.chapters.selected = at;
        }
        task
    }

    /// `v`: one page or two, kept on this computer through the engine's page layout setting.
    fn switch_pages(&mut self) -> Option<Command> {
        let surface_two = self.reader_here()?.surface.as_ref().is_some_and(|s| s.two_pages);
        let reader = self.reader_here()?;
        let layout = reader.page_layout();
        let two_now = layout == "double" || (layout == "auto" && surface_two); // i18n-skip
        let name = if two_now {
            "single" // i18n-skip: setting values
        } else if surface_two {
            "auto" // i18n-skip: setting values
        } else {
            "double" // i18n-skip: setting values
        }; // i18n-skip: setting values
        Some(Command::Reader(ReaderCommand::SetSetting {
            session: Some(reader.session),
            medium: reader.medium().unwrap_or(Medium::Manga),
            work: None,
            key: ReaderSettingKey::PageLayout,
            scope: SettingScope::Device,
            value: ReaderValue::Choice { name: name.to_owned() },
        }))
    }

    /// Whether the reader has something on a clock: an open or a plate waiting, a notice up.
    pub(super) fn reader_ticking(&self) -> bool {
        let Some(reader) = self.reader_here() else { return false };
        let now = self.clock.now_instant();
        reader.opening.is_some()
            || reader.strip.report.is_some()
            || reader.novel.as_ref().is_some_and(|n| n.report.is_some())
            || reader.notice.as_ref().is_some_and(|n| n.until.is_none_or(|u| u > now))
            || (self.images.engine().is_enabled()
                && reader.leaf().is_some_and(|l| {
                    l.parts.iter().any(|p| {
                        let key = PartKey { page: p.page, half: p.half };
                        !reader.pictures.contains_key(&key)
                            && !reader.failures.contains_key(&p.page)
                    })
                }))
    }

    /// The reader's notice while it shows.
    pub fn reader_notice(&self) -> Option<&ShownNotice> {
        let now = self.clock.now_instant();
        self.reader_here()?.notice.as_ref().filter(|n| n.until.is_none_or(|u| u > now))
    }

    /// The letter that runs `action` in the reader, for hints that pair keys up: `j k`.
    pub fn reader_letter(&self, action: Action) -> Option<String> {
        let mut keys = self.keymap.keys_in(action, Context::Reading);
        keys.sort();
        let letter = keys.iter().find(|b| matches!(b.code, crossterm::event::KeyCode::Char(_)));
        letter.or(keys.first()).map(Binding::render)
    }

    /// The key that runs `action` in the reader, as its hint names it: the arrow for the turn
    /// keys, else the first key the reference lists.
    pub fn reader_key_name(&self, action: Action) -> Option<String> {
        let mut keys = self.keymap.keys_in(action, Context::Reading);
        keys.sort();
        let arrow = keys.iter().find(|b| {
            matches!(b.code, crossterm::event::KeyCode::Left | crossterm::event::KeyCode::Right)
        });
        arrow.or(keys.first()).map(Binding::render)
    }
}

/// The reader's status hints for the screen up, at a window `width` wide: the turn keys in the
/// reading direction, and the keys that do something in the state the reader is in.
pub(crate) fn reader_hints(app: &App, width: usize) -> Vec<(String, &'static str)> {
    let Some(screen) = app.reader_screen() else { return Vec::new() };
    let Some(reader) = app.reader_here() else { return Vec::new() };
    if app.nav.overlay() == Some(&crate::nav::Overlay::Help) {
        return vec![("esc".to_owned(), said(t::keys_hint_close()))]; // i18n-skip: a key
    }
    if app.nav.overlay() == Some(&crate::nav::Overlay::ReaderSettings) {
        return app.reader_settings_hints();
    }
    if screen == ReaderScreen::Pages && reader.novel.is_some() {
        return app.novel_hints(width);
    }
    let (forward, back) = if reader.rtl() {
        (Action::ReaderLeft, Action::ReaderRight)
    } else {
        (Action::ReaderRight, Action::ReaderLeft)
    };
    let from_series = matches!(app.nav.under(), Some(StageView::Series(_)));
    let leave_back =
        if from_series { said(t::tui_reader_hint_series()) } else { said(t::keys_hint_back()) };
    let next = said(t::tui_reader_hint_next());
    let back_word = said(t::tui_reader_hint_back());
    let keys = said(t::keys_hint_keys());
    let chapters = said(t::series_chapters());
    let wanted: Vec<(Action, &'static str)> = match screen {
        ReaderScreen::TooSmall => vec![(Action::ReaderLeave, said(t::keys_hint_back()))],
        ReaderScreen::Opening => {
            vec![(Action::ReaderLeave, said(t::tui_reader_hint_stop())), (Action::Help, keys)]
        }
        ReaderScreen::ChapterFailed => vec![
            (Action::ReaderAgain, said(t::reader_try_again())),
            (Action::ReaderChapters, chapters),
            (Action::ReaderLeave, said(t::keys_hint_back())),
            (Action::Help, keys),
        ],
        ReaderScreen::ImagesOff => vec![
            (Action::ReaderChapters, chapters),
            (Action::ReaderLeave, said(t::keys_hint_back())),
            (Action::Help, keys),
        ],
        ReaderScreen::Pages => {
            let failed = reader
                .leaf()
                .is_some_and(|l| l.parts.iter().any(|p| reader.failures.contains_key(&p.page)));
            if !failed && let Some(hints) = mode_hints(app, reader, width, forward, back) {
                return hints;
            }
            if failed {
                vec![
                    (Action::ReaderAgain, said(t::reader_try_again())),
                    (forward, said(t::tui_reader_hint_without())),
                    (Action::Help, keys),
                ]
            } else if width >= 140 {
                // A wide window has room to say one page or two.
                let pages = if reader.shows_two() {
                    said(t::tui_reader_hint_one_page())
                } else {
                    said(t::tui_reader_hint_two_pages())
                };
                vec![
                    (forward, next),
                    (back, back_word),
                    (Action::ReaderPages, pages),
                    (Action::ReaderChapters, chapters),
                    (Action::ReaderSettings, said(t::tui_reader_hint_settings())),
                    (Action::Help, keys),
                ]
            } else if width >= 100 {
                vec![
                    (forward, next),
                    (back, back_word),
                    (Action::ReaderChapters, chapters),
                    (Action::ReaderSettings, said(t::tui_reader_hint_settings())),
                    (Action::Help, keys),
                ]
            } else if width >= 72 {
                vec![(forward, next), (back, back_word), (Action::Help, keys)]
            } else {
                vec![(forward, next), (Action::Help, keys)]
            }
        }
        ReaderScreen::End => end_hints(reader, width, chapters, leave_back, keys),
    };
    wanted
        .into_iter()
        .filter_map(|(action, label)| Some((app.reader_key_name(action)?, label)))
        .collect()
}

/// The hints at fit width, zoomed and in a strip, where keys pair up as the board writes them:
/// "j k scroll", "h j k l move". `None` for a page fitted to the window.
fn mode_hints(
    app: &App,
    reader: &Reader,
    width: usize,
    forward: Action,
    back: Action,
) -> Option<Vec<(String, &'static str)>> {
    let one = |action: Action| app.reader_key_name(action);
    let letters = |actions: &[Action]| {
        let keys: Option<Vec<String>> = actions.iter().map(|a| app.reader_letter(*a)).collect();
        keys.map(|k| k.join(" "))
    };
    let scroll = (
        letters(&[Action::ReaderScrollDown, Action::ReaderScrollUp]),
        said(t::tui_reader_hint_scroll()),
    );
    let keys = (one(Action::Help), said(t::keys_hint_keys()));
    let wanted: Vec<(Option<String>, &'static str)> = if reader.strips() {
        let mut wanted = vec![
            scroll,
            (app.reader_letter(Action::ReaderNext), said(t::tui_reader_hint_next_screen())),
        ];
        if width >= 100 && reader.end.next.is_some() {
            wanted.push((
                one(Action::ReaderNextChapter),
                said(t::tui_reader_hint_next_chapter()),
            ));
        }
        wanted.push(keys);
        wanted
    } else if reader.zoom.is_some() {
        vec![
            (
                letters(&[
                    Action::ReaderLeft,
                    Action::ReaderScrollDown,
                    Action::ReaderScrollUp,
                    Action::ReaderRight,
                ]),
                said(t::tui_reader_hint_move()),
            ),
            (
                letters(&[Action::ReaderZoomIn, Action::ReaderZoomOut]),
                said(t::tui_reader_hint_zoom()),
            ),
            (one(Action::ReaderZoom), said(t::tui_reader_hint_leave_zoom())),
            keys,
        ]
    } else if reader.fits_width() {
        let next = (one(forward), said(t::tui_reader_hint_next()));
        let fit = (one(Action::ReaderFit), said(t::tui_reader_hint_fit_screen()));
        if width >= 100 {
            vec![
                scroll,
                next,
                (one(back), said(t::tui_reader_hint_back())),
                fit,
                (one(Action::ReaderSettings), said(t::tui_reader_hint_settings())),
                keys,
            ]
        } else if width >= 72 {
            vec![scroll, next, fit, keys]
        } else {
            vec![scroll, next, keys]
        }
    } else {
        return None;
    };
    Some(wanted.into_iter().filter_map(|(key, label)| Some((key?, label))).collect())
}

/// The end page's hints: Continue and the next chapter when there is one, else what can be done
/// after the last chapter there is.
fn end_hints(
    reader: &Reader,
    width: usize,
    chapters: &'static str,
    leave_back: &'static str,
    keys: &'static str,
) -> Vec<(Action, &'static str)> {
    let wide = width >= 100;
    let mut wanted = Vec::new();
    if reader.end.next.is_some() {
        wanted.push((Action::ReaderBare, said(t::tui_read_hint_continue())));
        if let Some(skip) = &reader.end.skip_to {
            let number = skip.number.clone().unwrap_or_else(|| skip.label.clone());
            wanted.push((Action::ReaderSkip, said(t::reader_end_skip_to(&number))));
        } else {
            wanted.push((Action::ReaderNextChapter, said(t::tui_reader_hint_next_chapter())));
            wanted.push((Action::ReaderChapters, chapters));
            wanted.push((Action::ReaderLeave, leave_back));
        }
    } else {
        match reader.end.why_none {
            Some(NoNext::Finished) => {
                wanted.push((Action::ReaderAgain, said(t::tui_reader_hint_again())));
                wanted.push((Action::ReaderLeave, said(t::keys_hint_back())));
            }
            Some(NoNext::Offline) => {
                wanted.push((Action::ReaderChapters, chapters));
                wanted.push((Action::ReaderLeave, leave_back));
            }
            _ => wanted.push((Action::ReaderLeave, leave_back)),
        }
    }
    if wide {
        wanted.push((Action::Help, keys));
    }
    wanted
}

/// The terminal's own notice on the status rule, for as long as the short ones stay.
pub(super) fn own_notice(text: String, now: Instant) -> Option<ShownNotice> {
    Some(ShownNotice {
        text,
        action: None,
        until: Some(now + Duration::from_millis(u64::from(NOTICE_MS))),
    })
}

/// The engine's notice as the status rule shows it, for as long as the engine says.
fn shown_notice(notice: &ReaderNotice, now: Instant) -> Option<ShownNotice> {
    let text = say(&notice.msg);
    if text.is_empty() {
        return None;
    }
    let words = text.split_whitespace().count() as u32;
    let hold = reader_notice_hold(notice.lasts_ms, words, notice.action.is_some(), false);
    Some(ShownNotice {
        text,
        action: notice.action,
        until: hold.map(|ms| now + Duration::from_millis(u64::from(ms.max(1)))),
    })
}

/// The line that says what became of the progress sent to a book server.
fn server_line(result: &WriteBackResult, host: &str) -> Option<String> {
    let problem = match result {
        WriteBackResult::Saved => return Some(t::progress_server_saved(host)),
        WriteBackResult::Waiting { problem } | WriteBackResult::NotSaved { problem } => {
            *problem
        }
    };
    Some(match problem {
        WriteBackProblem::NotSetToReceive => return None,
        WriteBackProblem::Offline => t::progress_server_offline(host),
        WriteBackProblem::NoAnswer | WriteBackProblem::ServerError => {
            t::progress_server_retrying(host)
        }
        WriteBackProblem::RefusedKey => t::progress_server_failed(host, &t::progress_why_key()),
        WriteBackProblem::RefusedPassword => {
            t::progress_server_failed(host, &t::progress_why_password())
        }
        WriteBackProblem::SeriesGone => {
            t::progress_server_failed(host, &t::progress_why_gone())
        }
        WriteBackProblem::NoAnswerForADay => {
            t::progress_server_failed(host, &t::progress_why_gave_up())
        }
    })
}

#[cfg(test)]
pub(super) mod tests {
    use kaeri_model::{
        reader::{LeafPart, PageKind, ReadPlace, ReaderMode, UnitRect},
        reading::ChapterRow,
    };

    use super::*;
    use crate::app::fixtures::*;

    pub(in crate::app) const SESSION: u64 = 7;

    pub(in crate::app) fn row(n: u32) -> ChapterRow {
        ChapterRow {
            id: ChapterId(format!("c{n}")),
            label: format!("Chapter {n}"),
            state: ChapterReadState::InProgress { page: 2, of: Some(4) },
            number: Some(n.to_string()),
            title: None,
            volume: None,
            published: None,
            pages: Some(4),
            new_chapter: false,
            download: None,
            versions: 1,
            bookmarked: false,
            version_label: None,
            read_at: None,
            line: Vec::new(),
            gone_from_source: false,
            parts: 0,
            missing_parts: Vec::new(),
            part_groups: Vec::new(),
            other_group: false,
            group_language: None,
            other_source: None,
            on_device: false,
        }
    }

    pub(in crate::app) fn view(direction: ReadDirection) -> ReaderView {
        let work = WorkId::new("a:1");
        ReaderView {
            session: SESSION,
            work: work.clone(),
            title: "Harbor Light Recipes".into(),
            medium: Medium::Manga,
            chapter: row(14),
            mode: ReaderMode::Paged,
            direction,
            start: ReadPlace {
                work,
                chapter: ChapterId("c14".into()),
                page: Some(1),
                strip: None,
                novel: None,
            },
            next: Some(row(15)),
            previous: None,
            settings: None,
            demo_credit: None,
            progress_host: None,
        }
    }

    /// Four pages, one to a leaf, then the end page.
    pub(in crate::app) fn leaves() -> Vec<Leaf> {
        let mut leaves: Vec<Leaf> = (0..4)
            .map(|page| Leaf {
                index: page,
                parts: vec![LeafPart {
                    page,
                    kind: PageKind::Story,
                    file: None,
                    place: UnitRect::WHOLE,
                    rotate: false,
                    half: None,
                    ratio: Some(2.0 / 3.0),
                    strip: None,
                }],
                end_page: false,
            })
            .collect();
        leaves.push(Leaf { index: 4, parts: Vec::new(), end_page: true });
        leaves
    }

    /// The reader open on chapter 14 at its second page, in a window `w` by `h` drawn with
    /// halfblocks.
    pub(in crate::app) fn reading(direction: ReadDirection, w: u16, h: u16) -> App {
        kaeri_i18n::set_language("en");
        let engine = crate::image::ImageEngine::halfblocks();
        let mut a =
            App::with_images(SettingsView::default(), Palette::dark(), Keymap::new(), engine);
        a.took_frame(FrameReport { window: Some((w, h)), ..FrameReport::default() });
        let open = a.open_reader(WorkId::new("a:1"), Some(ChapterId("c14".into())));
        assert!(matches!(open, Some(Command::Reader(ReaderCommand::Open { .. }))), "{open:?}");
        a.apply(Event::Reader(ReaderEvent::Opened(Box::new(view(direction)))));
        a.apply(Event::Reader(ReaderEvent::Leaves {
            session: SESSION,
            chapter: ChapterId("c14".into()),
            leaves: leaves(),
        }));
        a
    }

    fn shown(leaf: u32, how: TurnHow) -> Option<Command> {
        Some(Command::Reader(ReaderCommand::Shown {
            session: SESSION,
            chapter: ChapterId("c14".into()),
            leaf,
            how,
        }))
    }

    pub(in crate::app) fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn the_open_starts_at_the_kept_page_and_asks_for_the_pages_around_it() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        let commands = a.take_commands();
        assert!(commands.contains(&shown(1, TurnHow::Placed).unwrap()), "{commands:?}");
        let asked: Vec<u32> = commands
            .iter()
            .filter_map(|c| match c {
                Command::Reader(ReaderCommand::NeedPage { page, .. }) => Some(*page),
                _ => None,
            })
            .collect();
        assert_eq!(asked, vec![1, 2, 3, 0], "the page, two on and one back");
        assert!(commands.contains(&Command::Reading(ReadingCommand::LoadChapters {
            work: WorkId::new("a:1"),
            edition: None,
        })));
        assert_eq!(a.screen_context(), Context::Reading);
    }

    #[test]
    fn in_right_to_left_manga_left_turns_forward_and_right_back() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        assert_eq!(a.press(key(KeyCode::Left), 20), shown(2, TurnHow::Key));
        assert_eq!(a.press(key(KeyCode::Right), 20), shown(1, TurnHow::Key));
        assert_eq!(a.press(key(KeyCode::Char(' ')), 20), shown(2, TurnHow::Key));
    }

    #[test]
    fn left_to_right_the_arrows_go_the_other_way_and_space_still_turns_forward() {
        let mut a = reading(ReadDirection::LeftToRight, 120, 34);
        a.take_commands();
        assert_eq!(a.press(key(KeyCode::Right), 20), shown(2, TurnHow::Key));
        assert_eq!(a.press(key(KeyCode::Left), 20), shown(1, TurnHow::Key));
        assert_eq!(a.press(key(KeyCode::Char(' ')), 20), shown(2, TurnHow::Key));
        // Back from the first page is the engine's to decide.
        a.press(key(KeyCode::Left), 20);
        a.press(key(KeyCode::Left), 20);
        assert_eq!(
            a.press(key(KeyCode::Left), 20),
            Some(Command::Reader(ReaderCommand::GoBack { session: SESSION }))
        );
    }

    #[test]
    fn section_digits_and_tab_do_nothing_in_the_reader() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        assert_eq!(a.press(key(KeyCode::Char('3')), 20), None);
        assert_eq!(a.press(key(KeyCode::Tab), 20), None);
        assert!(matches!(a.nav.current(), StageView::Reader(_)));
    }

    #[test]
    fn enter_hides_the_chrome_and_shows_it_again_with_the_stage_sent_each_time() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        a.press(key(KeyCode::Enter), 20);
        assert!(a.reader_here().is_some_and(|r| r.bare));
        let surface = |a: &mut App| {
            a.take_commands().into_iter().find_map(|c| match c {
                Command::Reader(ReaderCommand::Surface { surface, .. }) => Some(surface),
                _ => None,
            })
        };
        assert_eq!(surface(&mut a).map(|s| s.height_px), Some(68), "34 rows of two pixels");
        a.press(key(KeyCode::Enter), 20);
        assert!(a.reader_here().is_some_and(|r| !r.bare));
        assert_eq!(surface(&mut a).map(|s| s.height_px), Some(60));
    }

    #[test]
    fn a_resize_sends_the_new_stage_and_a_window_too_small_sends_none() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        let first = a.reader_here().and_then(|r| r.surface).expect("a surface");
        assert!(first.two_pages, "30 rows of stage with room for two");
        assert_eq!((first.width_px, first.height_px), (120, 60));
        a.take_commands();
        a.took_frame(FrameReport { window: Some((80, 24)), ..FrameReport::default() });
        let sent = a.take_commands();
        let Some(Command::Reader(ReaderCommand::Surface { session, surface })) = sent.first()
        else {
            panic!("{sent:?}")
        };
        assert_eq!(*session, SESSION);
        assert_eq!((surface.width_px, surface.height_px, surface.two_pages), (80, 40, false));
        assert_eq!(surface.decodes.map(|d| (d.jpeg, d.gif, d.avif)), Some((true, true, false)));
        assert!(surface.text && surface.zoom == 1.0);
        a.took_frame(FrameReport { window: Some((36, 10)), ..FrameReport::default() });
        assert_eq!(a.take_commands(), Vec::new());
        assert_eq!(a.reader_screen(), Some(ReaderScreen::TooSmall));
        // The same frame again says nothing new.
        a.took_frame(FrameReport { window: Some((80, 24)), ..FrameReport::default() });
        assert_eq!(a.take_commands(), Vec::new());
    }

    #[test]
    fn q_closes_the_session_and_leaves_the_reader() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        let task = a.press(key(KeyCode::Char('q')), 20);
        assert_eq!(task, Some(Command::Reader(ReaderCommand::Close { session: SESSION })));
        assert!(a.reader.is_none());
        assert!(!matches!(a.nav.current(), StageView::Reader(_)));
    }

    #[test]
    fn a_page_file_is_decoded_once_and_drawn_from_its_picture() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        let file = PageFile {
            path: "/pages/1.png".into(),
            encoding: PageEncoding::Original { mime: "image/png".into() },
            width: 400,
            height: 600,
            crop: None,
            colour: true,
            bytes: 10,
        };
        let ready = |a: &mut App| {
            a.apply(Event::Reader(ReaderEvent::PageReady {
                session: SESSION,
                chapter: ChapterId("c14".into()),
                page: 1,
                half: None,
                file: file.clone(),
            }));
        };
        ready(&mut a);
        let jobs = a.take_page_jobs();
        assert_eq!(jobs.len(), 1);
        ready(&mut a);
        assert!(a.take_page_jobs().is_empty(), "already on its way");
        let picture = image::DynamicImage::new_rgb8(4, 6);
        a.page_decoded(&jobs[0].key, Some(picture));
        let reader = a.reader_here().expect("reading");
        assert_eq!(reader.pictures.get(&PartKey { page: 1, half: None }), Some(&(4, 6)));
        assert!(a.images.has(&jobs[0].key));
    }

    #[test]
    fn a_page_is_decoded_to_fit_the_stage_and_again_sharper_at_fit_width() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        let file = PageFile {
            path: "/pages/1.png".into(),
            encoding: PageEncoding::Original { mime: "image/png".into() },
            width: 1200,
            height: 1800,
            crop: None,
            colour: true,
            bytes: 10,
        };
        a.apply(Event::Reader(ReaderEvent::PageReady {
            session: SESSION,
            chapter: ChapterId("c14".into()),
            page: 1,
            half: None,
            file,
        }));
        let jobs = a.take_page_jobs();
        // Halfblocks: a pixel a column and two a row, on a stage of 30 rows.
        assert_eq!(jobs.iter().map(|j| j.within).collect::<Vec<_>>(), vec![(120, 60)]);
        a.page_decoded(&jobs[0].key, Some(image::DynamicImage::new_rgb8(40, 60)));
        a.send_surface();
        assert!(a.take_page_jobs().is_empty(), "the picture fits the stage it was made for");
        a.press(key(KeyCode::Char('f')), 30);
        let again = a.take_page_jobs();
        assert_eq!(again.iter().map(|j| j.within).collect::<Vec<_>>(), vec![(120, u32::MAX)]);
        a.page_decoded(&again[0].key, Some(image::DynamicImage::new_rgb8(120, 180)));
        a.press(key(KeyCode::Char('f')), 30);
        assert!(a.take_page_jobs().is_empty(), "a smaller box keeps the sharper picture");
    }

    #[test]
    fn a_picture_is_decoded_again_only_when_a_new_box_shows_it_larger() {
        // Held back by its height, then a box as tall again.
        assert!(sharper((40, 60), (120, 60), (120, 120)));
        // Taller only, where its width held it back.
        assert!(!sharper((120, 40), (120, 60), (120, 600)));
        // Smaller than its box: the file is no larger than the picture.
        assert!(!sharper((30, 45), (120, 60), (1000, 1000)));
        assert!(!sharper((40, 60), (120, 60), (120, 60)));
    }

    #[test]
    fn a_file_that_will_not_decode_is_asked_for_once_more_then_fails() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        let file = PageFile {
            path: "/pages/2.png".into(),
            encoding: PageEncoding::Original { mime: "image/png".into() },
            width: 400,
            height: 600,
            crop: None,
            colour: true,
            bytes: 10,
        };
        for round in 0..2 {
            a.apply(Event::Reader(ReaderEvent::PageReady {
                session: SESSION,
                chapter: ChapterId("c14".into()),
                page: 2,
                half: None,
                file: file.clone(),
            }));
            let job = a.take_page_jobs().pop().expect("a job");
            a.page_decoded(&job.key, None);
            let asked = a.take_commands();
            if round == 0 {
                assert!(
                    matches!(
                        asked.as_slice(),
                        [Command::Reader(ReaderCommand::NeedPage { page: 2, .. })]
                    ),
                    "{asked:?}"
                );
            } else {
                assert!(asked.is_empty());
            }
        }
        assert!(a.reader_here().is_some_and(|r| r.failures.contains_key(&2)));
    }

    #[test]
    fn v_keeps_one_page_on_this_computer_through_the_page_layout_setting() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        let task = a.press(key(KeyCode::Char('v')), 20);
        assert_eq!(
            task,
            Some(Command::Reader(ReaderCommand::SetSetting {
                session: Some(SESSION),
                medium: Medium::Manga,
                work: None,
                key: ReaderSettingKey::PageLayout,
                scope: SettingScope::Device,
                value: ReaderValue::Choice { name: "single".into() },
            }))
        );
    }

    #[test]
    fn the_resumed_notice_offers_g_to_start_over() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        a.apply(Event::Reader(ReaderEvent::Notice(ReaderNotice {
            msg: kaeri_model::Msg::verbatim("Resumed at page 2"),
            action: Some(ReaderAction::StartOver),
            lasts_ms: 6_000,
        })));
        assert!(a.reader_notice().is_some());
        assert_eq!(
            a.press(key(KeyCode::Char('g')), 20),
            Some(Command::Reader(ReaderCommand::StartOver { session: SESSION }))
        );
        // Without the notice, g is the chapter's first page.
        assert_eq!(a.press(key(KeyCode::Char('g')), 20), shown(0, TurnHow::Jump));
    }

    #[test]
    fn forward_on_the_end_page_opens_the_next_chapter() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.press(key(KeyCode::Char('G')), 20);
        a.press(key(KeyCode::Char(' ')), 20);
        assert_eq!(a.reader_screen(), Some(ReaderScreen::End));
        assert_eq!(
            a.press(key(KeyCode::Enter), 20),
            Some(Command::Reader(ReaderCommand::OpenChapter {
                session: SESSION,
                chapter: ChapterId("c15".into()),
            }))
        );
        assert_eq!(a.reader_screen(), Some(ReaderScreen::Opening));
    }

    #[test]
    fn a_chapter_that_cannot_open_says_so_and_r_tries_again() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        a.apply(Event::Reading(kaeri_model::reading::ReadingEvent::Failed {
            about: kaeri_model::reading::ReadingAbout::Reader,
            failure: ReadingFailure::Offline,
        }));
        assert_eq!(a.reader_screen(), Some(ReaderScreen::ChapterFailed));
        assert!(!a.logs.is_empty(), "the whole error is in Recent messages");
        assert_eq!(
            a.press(key(KeyCode::Char('r')), 20),
            Some(Command::Reader(ReaderCommand::Retry {
                session: SESSION,
                chapter: ChapterId("c14".into()),
                page: None,
            }))
        );
    }

    #[test]
    fn an_open_that_lands_after_leaving_is_closed() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.press(key(KeyCode::Char('q')), 20);
        a.take_commands();
        a.apply(Event::Reader(ReaderEvent::Opened(Box::new(view(ReadDirection::RightToLeft)))));
        assert_eq!(
            a.take_commands(),
            vec![Command::Reader(ReaderCommand::Close { session: SESSION })]
        );
    }
}
