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
        Leaf, NoNext, OpenAt, PageEncoding, PageFile, PageFormats, PageHalf, ReadDirection,
        ReaderAction, ReaderCommand, ReaderEvent, ReaderNotice, ReaderSettingKey,
        ReaderSettingsView, ReaderSurface, ReaderValue, ReaderView, ReaderWait, SettingScope,
        TurnHow, reader_notice_hold,
    },
    reading::{
        ChapterReadState, ChapterRow, DownloadPick, ReadingCommand, ReadingFailure, VolumeGroup,
    },
};

use super::{App, info, say, series::downloaded};
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
    /// Keys handed out for decoding, with the part each is for; `None` is the next preview.
    decoding: HashMap<String, Option<PartKey>>,
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
        }
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
        self.chapter_failure.is_none() && (self.view.is_none() || self.leaves.is_empty())
    }

    /// The leaf in view is the page between chapters.
    pub fn on_end(&self) -> bool {
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

    fn reader_mut(&mut self) -> Option<&mut Reader> {
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
        let two_pages = rows >= TWO_PAGE_ROWS
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
            zoom: 1.0,
            text: true,
        }
    }

    /// The window the last frame was drawn in. In the reader, a new surface goes to the engine
    /// when it changes the stage, so the chapter is laid out again at the same place.
    pub(super) fn reader_window(&mut self, window: (u16, u16)) {
        self.window = Some(window);
        self.send_surface();
    }

    /// Send the surface again when it changed: a resize, the chrome hidden or shown, one page
    /// or two. Nothing goes while the window is too small; the place is kept for when it grows.
    fn send_surface(&mut self) {
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
                if fresh {
                    images.forget_prefix("reader/"); // i18n-skip: a key prefix
                    reader.leaves.clear();
                    reader.files.clear();
                    reader.pictures.clear();
                    reader.decoding.clear();
                    reader.asked_pages.clear();
                    reader.asked_again.clear();
                    reader.failures.clear();
                    reader.end = EndFacts::default();
                    reader.back_to = None;
                    reader.at = 0;
                    reader.held_page = None;
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
                    if old.is_some_and(|old| old.path != file.path || old.crop != file.crop) {
                        reader.pictures.remove(&key);
                        images.forget(&reader.picture_key(key));
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
                    reader.end.read = Some(trackers);
                    if let Some(view) = reader.view.as_mut() {
                        view.chapter.state = ChapterReadState::Read;
                    }
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
                self.page_jobs.push(PageJob { key, file, rotate: false, half: None });
            }
            ReaderEvent::Notice(notice) if reader.session != 0 || reader.opening.is_some() => {
                let shown = shown_notice(&notice, now);
                reader.notice = shown;
            }
            ReaderEvent::BackToPage { session, page } if mine(session) => {
                reader.back_to = page;
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
    /// as blocks.
    fn say_blocks_once(&mut self) {
        if self.blocks_said || self.images.engine().graphics() != Graphics::Halfblocks {
            return;
        }
        let Some(reader) = self.reader.as_mut() else { return };
        if reader.medium() == Some(Medium::Novel) {
            return;
        }
        self.blocks_said = true;
        let now = self.clock.now_instant();
        // The resumed notice says where the chapter opened, which matters more; this waits.
        if reader.notice.as_ref().is_some_and(|n| n.until.is_none_or(|u| u > now)) {
            self.blocks_said = false;
            return;
        }
        reader.notice = Some(ShownNotice {
            text: t::tui_reader_blocks_notice(),
            action: None,
            until: Some(now + Duration::from_millis(u64::from(NOTICE_MS))),
        });
    }

    /// The chapter's leaves, first or laid out again: the page in view stays in view.
    fn take_leaves(&mut self, leaves: Vec<Leaf>) {
        let Some(reader) = self.reader.as_mut() else { return };
        for part in leaves.iter().flat_map(|l| &l.parts) {
            if let Some(file) = &part.file {
                let key = PartKey { page: part.page, half: part.half };
                let old = reader.files.insert(key, file.clone());
                if old.is_some_and(|old| old.path != file.path || old.crop != file.crop) {
                    reader.pictures.remove(&key);
                    let key = reader.picture_key(key);
                    self.images.forget(&key);
                }
            }
        }
        let first = reader.leaves.is_empty();
        let start = if first {
            reader.view.as_ref().and_then(|v| v.start.page)
        } else {
            reader.kept_page()
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
        let images_on = self.images.engine().is_enabled();
        let reader = self.reader.as_mut()?;
        if index >= reader.leaves.len() {
            return None;
        }
        if reader.at != index {
            reader.held_page = None;
            reader.shown_at = now;
        }
        reader.at = index;
        let session = reader.session;
        let chapter = reader.view.as_ref()?.chapter.id.clone();
        if images_on {
            let pages: Vec<u32> = reader
                .wanted(index)
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
        self.decode_in_view();
        Some(Command::Reader(ReaderCommand::Shown {
            session,
            chapter,
            leaf: index as u32,
            how,
        }))
    }

    /// Hand the files of the leaves around the one in view to be decoded, once each.
    fn decode_in_view(&mut self) {
        if !self.images.engine().is_enabled() {
            return;
        }
        let Some(reader) = self.reader.as_mut() else { return };
        let wanted = reader.wanted(reader.at);
        let parts: Vec<(PartKey, bool)> = wanted
            .into_iter()
            .filter_map(|i| reader.leaves.get(i))
            .flat_map(|l| l.parts.iter())
            .map(|p| (PartKey { page: p.page, half: p.half }, p.rotate))
            .collect();
        for (key, rotate) in parts {
            if reader.pictures.contains_key(&key) {
                continue;
            }
            let Some(file) = reader.files.get(&key).cloned() else { continue };
            let image_key = reader.picture_key(key);
            if reader.decoding.contains_key(&image_key) {
                continue;
            }
            reader.decoding.insert(image_key.clone(), Some(key));
            self.page_jobs.push(PageJob { key: image_key, file, rotate, half: key.half });
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
        match (part, image) {
            (None, Some(image)) => {
                let size = (image.width(), image.height());
                self.images.insert(key, image);
                let next = reader.end.next.as_ref().map(|n| n.id.clone());
                if let Some(next) = next.filter(|n| reader.preview_key(n) == key) {
                    reader.end.preview = Some((next, size));
                }
            }
            (None, None) => {}
            (Some(part), Some(image)) => {
                reader.pictures.insert(part, (image.width(), image.height()));
                self.images.insert(key, image);
            }
            (Some(part), None) => {
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
                None
            }
            (_, Action::ReaderPages) => self.switch_pages(),
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
    fn turn(&mut self, forward: bool) -> Option<Command> {
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
    fn open_chapter(&mut self, chapter: ChapterId) -> Option<Command> {
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
            "single"
        } else if surface_two {
            "auto"
        } else {
            "double"
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

    /// `,`: the reader settings. They are asked for here, and the settings overlay opens on
    /// them once it is built; [`Reader::settings_asked`] says it was asked.
    fn open_reader_settings(&mut self) -> Option<Command> {
        let reader = self.reader_mut()?;
        reader.settings_asked = true;
        Some(Command::Reader(ReaderCommand::LoadSettings {
            medium: reader.medium().unwrap_or(Medium::Manga),
            work: Some(reader.work.clone()),
        }))
    }

    /// Whether the reader has something on a clock: an open or a plate waiting, a notice up.
    pub(super) fn reader_ticking(&self) -> bool {
        let Some(reader) = self.reader_here() else { return false };
        let now = self.clock.now_instant();
        reader.opening.is_some()
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
            if failed {
                vec![
                    (Action::ReaderAgain, said(t::reader_try_again())),
                    (forward, said(t::tui_reader_hint_without())),
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
