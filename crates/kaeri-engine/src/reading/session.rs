//! The open reader session: where the person is, when a chapter counts as read, resume and
//! Back to page, and moves from other devices.
//!
//! The app reports what it painted; the session decides what that means. A place is written at
//! most once a second per chapter, and always on Close and when the app leaves the
//! foreground. The read rule waits 2 s on the last leaf (or a strip's foot) and fires at once
//! on the end page or when the next chapter opens. Times come from the engine's clock; the
//! timers run on the runtime and come back as [`Internal::Reader`] wakes.

use std::{sync::Arc, time::Duration};

use kaeri_core::{
    books::{ChapterId, WorkId},
    ids::AnilistId,
    media::Medium,
    traits::TrackOp,
};
use kaeri_model::{
    Msg, ReadingMsg,
    eink::{EinkProfile, ScreenFacts},
    event::Event,
    reader::{
        Leaf, NoNext, OpenAt, ReadDirection, ReadPlace, ReaderAction, ReaderCommand,
        ReaderEvent, ReaderMode, ReaderNotice, ReaderSettingKey as Key, ReaderSettingsView,
        ReaderSurface, ReaderView, StripPos, TurnHow,
    },
    reading::{ReadingAbout, ReadingEvent, ReadingFailure},
};
use kaeri_pages::{ColourChoice, Look, Target, layout::SpreadOnSingle};
use kaeri_store::reading::ReadEvent;
use tokio::time::Instant;

use super::{
    eink,
    pages::{Laid, OpenJob, Pages, Plan, Shared},
    settings::{self, Layers, Screen},
    shelf::{self, ShelfChapter},
};
use crate::{
    config::ConfigState,
    outbox::{Internal, Outbox, ReaderWake},
    services::Services,
};

/// How long the last leaf shows before the chapter counts as read.
pub const READ_AFTER: Duration = Duration::from_secs(2);
/// Places are written at most this often per chapter.
pub const WRITE_EVERY: Duration = Duration::from_secs(1);
const RESUMED_MS: u32 = 6_000;
const MOVED_MS: u32 = 8_000;
const BACK_MS: u32 = 8_000;

/// What the reader needs from the rest of the engine for one command.
pub struct Cx<'a> {
    pub config: &'a mut ConfigState,
    pub services: &'a Services,
    pub tx: &'a Outbox,
    pub screen: Screen,
    pub eink: Option<EinkProfile>,
}

impl Cx<'_> {
    fn send(&self, event: ReaderEvent) {
        let _ = self.tx.send(Event::Reader(event));
    }

    fn now(&self) -> i64 {
        self.services.clock.now()
    }

    fn layers(&self, medium: Medium, work: Option<&str>) -> Layers {
        Layers::read(
            &self.services.store,
            &self.config.effective().reading,
            medium,
            work,
            self.screen,
        )
    }

    pub fn settings_view(&self, medium: Medium, work: Option<&WorkId>) -> ReaderSettingsView {
        let rows = self.layers(medium, work.map(WorkId::as_str)).rows();
        ReaderSettingsView { medium, work: work.cloned(), rows }
    }
}

/// The reader: at most one open session, the page worker it uses, and what the screen is.
#[derive(Default)]
pub struct Reader {
    next: u64,
    pub session: Option<Session>,
    pages: Option<Pages>,
    /// What Android said about its screen, before its first screen.
    pub facts: Option<ScreenFacts>,
    /// The profile from those facts and the person's answers.
    pub eink: Option<EinkProfile>,
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader").field("session", &self.session.as_ref().map(|s| s.id)).finish()
    }
}

/// One open session.
#[derive(Debug)]
pub struct Session {
    pub id: u64,
    pub work: WorkId,
    pub medium: Medium,
    title: String,
    chapters: Vec<ShelfChapter>,
    /// The open chapter's place in `chapters`.
    at: usize,
    surface: ReaderSurface,
    layers: Layers,
    generation: u64,
    laid: Shared,
    /// What the session took from the last layout.
    leaves: Vec<Leaf>,
    cuts: Vec<StripPos>,
    pages_total: u32,
    mode: ReaderMode,
    direction: ReadDirection,
    /// `Opened` went out for the open chapter.
    opened: bool,
    start: ReadPlace,
    /// Where the person is, from the last report.
    place: Option<ReadPlace>,
    /// Show "Resumed at page N" once the leaves are out.
    resumed: Option<u32>,
    /// A guess from the source's hint, kept and shown when the chapter is laid out.
    hinted_strip: bool,
    /// The place before a jump, for Back to page.
    back_to: Option<ReadPlace>,
    /// The place before another device moved the reader, for Go back.
    moved_back: Option<ReadPlace>,
    read_fired: bool,
    end_page_said: bool,
    read_timer: u64,
    flush_timer: u64,
    flush_pending: bool,
    dirty: bool,
    last_write: Option<Instant>,
}

fn place(work: &WorkId, chapter: &ChapterId, page: Option<u32>) -> ReadPlace {
    ReadPlace { work: work.clone(), chapter: chapter.clone(), page, strip: None, novel: None }
}

fn notice(
    msg: ReadingMsg,
    action: Option<ReaderAction>,
    lasts_ms: u32,
    eink: bool,
) -> ReaderEvent {
    ReaderEvent::Notice(ReaderNotice {
        msg: Msg::Reading(msg),
        action,
        // On e-ink a notice clears at the next turn, drawn in the page's own update.
        lasts_ms: if eink { 0 } else { lasts_ms },
    })
}

fn after(tx: &Outbox, wait: Duration, session: u64, wake: ReaderWake) {
    let weak = tx.downgrade();
    tokio::spawn(async move {
        tokio::time::sleep(wait).await;
        if let Some(tx) = weak.upgrade() {
            let _ = tx.internal(Internal::Reader { session, wake });
        }
    });
}

impl Reader {
    /// The context a command runs in.
    pub fn cx<'a>(
        &self,
        config: &'a mut ConfigState,
        services: &'a Services,
        tx: &'a Outbox,
    ) -> Cx<'a> {
        let screen = Screen::new(self.eink.as_ref(), config.handheld());
        Cx { config, services, tx, screen, eink: self.eink.clone() }
    }

    /// The e-ink profile again, from the facts and the answers kept in `reading.eink`.
    pub fn detect(&mut self, config: &kaeri_core::config::Config) -> Option<EinkProfile> {
        let facts = self.facts.as_ref()?;
        let eink = &config.reading.eink;
        let profile = eink::detect(facts, eink::EinkAnswers::from_config(eink), eink);
        self.eink = Some(profile.clone());
        Some(profile)
    }

    fn pages(&mut self, cx: &Cx<'_>) -> Pages {
        self.pages
            .get_or_insert_with(|| {
                let dir = super::pages::cache_dir(&cx.services.paths.cache_dir);
                Pages::start(dir, cx.tx.downgrade())
            })
            .clone()
    }

    /// A reader command, with reading on.
    pub fn command(&mut self, cx: &mut Cx<'_>, command: ReaderCommand) {
        match command {
            ReaderCommand::Open { work, chapter, at, surface } => {
                self.open(cx, work, chapter, at, surface)
            }
            ReaderCommand::LoadSettings { medium, work } => {
                cx.send(ReaderEvent::Settings(cx.settings_view(medium, work.as_ref())));
            }
            ReaderCommand::SetSetting { session, medium, work, key, scope, value } => {
                let now = cx.now();
                let paths = cx.services.paths.clone();
                if let Err(e) = settings::set(
                    cx.config,
                    &paths,
                    &cx.services.store,
                    medium,
                    work.as_ref().map(WorkId::as_str),
                    key,
                    scope,
                    &value,
                    now,
                ) {
                    tracing::warn!(error = %e, "a reader setting was not kept");
                }
                self.settings_changed(cx, session, medium, work, key);
            }
            ReaderCommand::UseDefault { medium, work, key } => {
                if let Err(e) =
                    settings::use_default(&cx.services.store, work.as_str(), key, cx.now())
                {
                    tracing::warn!(error = %e, "a series setting was not dropped");
                }
                let session = self.session.as_ref().filter(|s| s.work == work).map(|s| s.id);
                self.settings_changed(cx, session, medium, Some(work), key);
            }
            ReaderCommand::Close { session } => {
                if self.session.as_ref().is_some_and(|s| s.id == session) {
                    self.close(cx);
                }
                cx.send(ReaderEvent::Closed { session });
            }
            other => {
                let pages = self.pages(cx);
                let Some(s) =
                    self.session.as_mut().filter(|s| Some(s.id) == session_of(&other))
                else {
                    // Reports for a closed session are dropped.
                    return;
                };
                s.report(cx, &pages, other);
            }
        }
    }

    fn settings_changed(
        &mut self,
        cx: &mut Cx<'_>,
        session: Option<u64>,
        medium: Medium,
        work: Option<WorkId>,
        key: Key,
    ) {
        cx.send(ReaderEvent::Settings(cx.settings_view(medium, work.as_ref())));
        let pages = self.pages(cx);
        if let Some(s) = self
            .session
            .as_mut()
            .filter(|s| Some(s.id) == session || Some(&s.work) == work.as_ref())
        {
            s.layers = cx.layers(s.medium, Some(s.work.as_str()));
            if settings::relays(key) {
                let page = s.page().unwrap_or(0);
                s.lay_out(cx, &pages, page);
            }
        }
    }

    fn close(&mut self, cx: &Cx<'_>) {
        if let Some(mut s) = self.session.take() {
            s.read_timer += 1;
            s.flush(cx);
            if let Some(pages) = &self.pages {
                pages.close(s.id);
            }
        }
    }

    fn open(
        &mut self,
        cx: &mut Cx<'_>,
        work: WorkId,
        chapter: Option<ChapterId>,
        at: OpenAt,
        surface: ReaderSurface,
    ) {
        self.close(cx);
        self.next += 1;
        let id = self.next;
        let store = &cx.services.store;
        let chapters = shelf::chapters(store, work.as_str());
        let kind = store.book_work_kind(work.as_str()).ok().flatten();
        let medium = match (kind.as_ref().map(|k| k.medium.as_str()), chapters.first()) {
            (Some("novel"), _) => Medium::Novel,
            (None, Some(c)) if c.is_text() => Medium::Novel,
            _ => Medium::Manga,
        };
        let Some((index, page, resumed)) =
            start_of(cx, &work, &chapters, chapter.as_ref(), &at)
        else {
            let failure = ReadingFailure::NotFound;
            let _ = cx.tx.send(Event::Reading(ReadingEvent::Failed {
                about: ReadingAbout::Reader,
                failure,
            }));
            return;
        };
        let layers = cx.layers(medium, Some(work.as_str()));
        let start = match &at {
            OpenAt::Place { place } if place.chapter == chapters[index].id => place.clone(),
            _ => place(&work, &chapters[index].id, Some(page)),
        };
        // The source's word comes before the pages' shape: a work from Korea or China is read
        // as a strip unless the series says otherwise.
        let hinted_strip = !layers.series_has(Key::Mode)
            && medium == Medium::Manga
            && kind
                .as_ref()
                .and_then(|k| k.origin.as_deref())
                .is_some_and(|o| matches!(o, "KR" | "CN" | "TW"));
        let mut session = Session {
            id,
            work,
            medium,
            title: kind.and_then(|k| k.title).unwrap_or_default(),
            chapters,
            at: index,
            surface,
            layers,
            generation: 0,
            laid: Arc::default(),
            leaves: Vec::new(),
            cuts: Vec::new(),
            pages_total: 0,
            mode: ReaderMode::Paged,
            direction: ReadDirection::RightToLeft,
            opened: false,
            start,
            place: None,
            resumed: resumed.then_some(page),
            hinted_strip,
            back_to: None,
            moved_back: None,
            read_fired: false,
            end_page_said: false,
            read_timer: 0,
            flush_timer: 0,
            flush_pending: false,
            dirty: false,
            last_write: None,
        };
        let pages = self.pages(cx);
        session.lay_out(cx, &pages, page);
        self.session = Some(session);
    }

    /// A timer ran out or the worker laid a chapter out.
    pub fn wake(&mut self, cx: &mut Cx<'_>, session: u64, wake: ReaderWake) {
        let pages = self.pages(cx);
        let Some(s) = self.session.as_mut().filter(|s| s.id == session) else { return };
        match wake {
            ReaderWake::Laid => s.laid_out(cx, &pages),
            ReaderWake::ReadRule(n) if n == s.read_timer => s.chapter_read(cx),
            ReaderWake::Flush(n) if n == s.flush_timer => {
                s.flush_pending = false;
                s.flush(cx);
            }
            ReaderWake::ReadRule(_) | ReaderWake::Flush(_) => {}
        }
    }

    /// The app left the foreground: the place is written and the read rule waits.
    pub fn background(&mut self, cx: &Cx<'_>) {
        if let Some(s) = self.session.as_mut() {
            s.read_timer += 1;
            s.flush(cx);
        }
    }

    /// Reading arrived from another device: move on when it read further.
    pub fn synced(&mut self, cx: &mut Cx<'_>) {
        let pages = self.pages(cx);
        if let Some(s) = self.session.as_mut() {
            s.moved_on(cx, &pages);
        }
    }
}

fn session_of(command: &ReaderCommand) -> Option<u64> {
    use ReaderCommand as C;
    match command {
        C::Surface { session, .. }
        | C::Shown { session, .. }
        | C::StripAt { session, .. }
        | C::NovelAt { session, .. }
        | C::NovelCounted { session, .. }
        | C::FixedRendered { session, .. }
        | C::NeedPage { session, .. }
        | C::Retry { session, .. }
        | C::OpenChapter { session, .. }
        | C::BackTo { session }
        | C::StartOver { session }
        | C::GoBack { session }
        | C::Close { session } => Some(*session),
        C::Open { .. }
        | C::SetSetting { .. }
        | C::UseDefault { .. }
        | C::LoadSettings { .. } => None,
    }
}

/// Which chapter to open and at which page, and whether that resumes a place.
fn start_of(
    cx: &Cx<'_>,
    work: &WorkId,
    chapters: &[ShelfChapter],
    chapter: Option<&ChapterId>,
    at: &OpenAt,
) -> Option<(usize, u32, bool)> {
    if chapters.is_empty() {
        return None;
    }
    let store = &cx.services.store;
    let kept = store.read_chapters(work.as_str()).unwrap_or_default();
    let marks = store.read_marks(work.as_str()).unwrap_or_default();
    let index_of = |id: &ChapterId| chapters.iter().position(|c| &c.id == id);
    let left_at = |id: &ChapterId| {
        kept.iter()
            .find(|k| k.chapter == id.as_str() && !k.read)
            .and_then(|k| k.last_page)
            .filter(|p| *p > 0)
    };
    Some(match at {
        OpenAt::Place { place } => (index_of(&place.chapter)?, place.page.unwrap_or(0), false),
        OpenAt::Page { page } => (chapter.and_then(index_of).unwrap_or(0), *page, false),
        OpenAt::Start => {
            let first_unread =
                chapters.iter().position(|c| !marks.is_read(c.id.as_str())).unwrap_or(0);
            (chapter.and_then(index_of).unwrap_or(first_unread), 0, false)
        }
        OpenAt::Resume => {
            let index = match chapter {
                Some(id) => index_of(id)?,
                None => {
                    let last = store
                        .read_work(work.as_str())
                        .ok()
                        .flatten()
                        .and_then(|w| w.last_chapter)
                        .and_then(|c| index_of(&ChapterId::new(c)));
                    match last {
                        // Finished: the next chapter, from its start.
                        Some(i) if marks.is_read(chapters[i].id.as_str()) => {
                            (i + 1).min(chapters.len() - 1)
                        }
                        Some(i) => i,
                        None => chapters
                            .iter()
                            .position(|c| !marks.is_read(c.id.as_str()))
                            .unwrap_or(0),
                    }
                }
            };
            match left_at(&chapters[index].id) {
                Some(page) => (index, page, true),
                None => (index, 0, false),
            }
        }
    })
}

impl Session {
    fn chapter(&self) -> &ShelfChapter {
        &self.chapters[self.at]
    }

    fn eink(&self) -> bool {
        self.surface.eink
    }

    fn page(&self) -> Option<u32> {
        self.place.as_ref().and_then(|p| p.page.or(p.strip.map(|s| s.image)))
    }

    /// The page a leaf starts on, and for a strip cut on e-ink its place in the strip.
    fn page_of_leaf(&self, leaf: u32) -> (Option<u32>, Option<StripPos>) {
        if let Some(cut) = self.cuts.get(leaf as usize) {
            return (Some(cut.image), Some(*cut));
        }
        let page = self.leaves.get(leaf as usize).and_then(|l| l.parts.first()).map(|p| p.page);
        (page, None)
    }

    fn leaf_of_page(&self, page: u32) -> u32 {
        if !self.cuts.is_empty() {
            return self.cuts.iter().position(|c| c.image >= page).unwrap_or(0) as u32;
        }
        self.leaves.iter().position(|l| l.parts.iter().any(|p| p.page >= page)).unwrap_or(0)
            as u32
    }

    fn plan(&self, cx: &Cx<'_>) -> Plan {
        let l = &self.layers;
        let panel = if self.surface.eink {
            match eink::pages_panel(cx.eink.as_ref()) {
                kaeri_pages::Panel::Lcd => kaeri_pages::Panel::Grey,
                panel => panel,
            }
        } else {
            kaeri_pages::Panel::Lcd
        };
        let colour_eink = panel.is_eink() && panel.shows_colour();
        let look = Look {
            colour: match l.word(Key::Colour).as_str() {
                "colour" => ColourChoice::Colour,
                "greys" => ColourChoice::Greys,
                _ if l.on(Key::GreyscalePages) => ColourChoice::Greys,
                _ => ColourChoice::Auto,
            },
            crop: l.on(Key::CropBorders),
            rainbow_filter: panel == kaeri_pages::Panel::Kaleido3 && l.on(Key::RainbowFilter),
            saturation: colour_eink.then(|| l.amount(Key::Saturation) as f32),
            contrast: if colour_eink { l.amount(Key::Contrast) as f32 } else { 1.0 },
            ..Look::default()
        };
        let layout = l.word(Key::PageLayout);
        let strip = self.hinted_strip || l.word(Key::Mode) == "strip";
        Plan {
            target: Target {
                panel,
                width: self.surface.width_px.max(1),
                height: self.surface.height_px.max(1),
            },
            look,
            two_pages: layout == "double" || (layout == "auto" && self.surface.two_pages),
            spreads: match l.word(Key::Spreads).as_str() {
                "rotate" => SpreadOnSingle::Rotate,
                "whole" => SpreadOnSingle::Whole,
                _ => SpreadOnSingle::Split,
            },
            mode: if strip { ReaderMode::Strip } else { ReaderMode::Paged },
            direction: if l.word(Key::Direction) == "ltr" {
                ReadDirection::LeftToRight
            } else {
                ReadDirection::RightToLeft
            },
            guess_mode: !self.hinted_strip && !l.series_has(Key::Mode),
            guess_direction: !l.series_has(Key::Direction),
        }
    }

    /// Ask the worker for the open chapter, starting at `page`.
    fn lay_out(&mut self, cx: &Cx<'_>, pages: &Pages, page: u32) {
        self.generation += 1;
        let chapter = self.chapter().clone();
        if chapter.is_text() {
            // Novels open in the novel reader, which serves the book itself.
            let failure = ReadingFailure::Unsupported { extension: "epub".into() };
            let _ = cx.tx.send(Event::Reading(ReadingEvent::Failed {
                about: ReadingAbout::Reader,
                failure,
            }));
            return;
        }
        self.start.page = Some(page);
        pages.open(OpenJob {
            session: self.id,
            generation: self.generation,
            chapter: chapter.id,
            path: chapter.path,
            plan: self.plan(cx),
            start_page: page,
            shared: self.laid.clone(),
        });
    }

    fn laid_out(&mut self, cx: &mut Cx<'_>, pages: &Pages) {
        let laid: Laid = match self.laid.lock() {
            Ok(laid) if laid.generation == self.generation => laid.clone(),
            _ => return,
        };
        if let Some(failure) = laid.failure {
            let _ = cx.tx.send(Event::Reading(ReadingEvent::Failed {
                about: ReadingAbout::Reader,
                failure,
            }));
            return;
        }
        self.mode = laid.mode.unwrap_or(ReaderMode::Paged);
        self.direction = laid.direction.unwrap_or(ReadDirection::RightToLeft);
        self.leaves = laid.leaves;
        self.cuts = laid.cuts;
        self.pages_total = laid.pages;
        let eink = self.eink();
        // A guess is kept for the series on this device, and said once.
        let now = cx.now();
        let store = &cx.services.store;
        let mut said = None;
        if laid.guessed_mode || self.hinted_strip {
            if store
                .set_series_guess(self.work.as_str(), Key::Mode.name(), "strip", now)
                .unwrap_or(false)
            {
                said = Some(ReadingMsg::ReaderDetectedStrip);
            }
        } else if laid.guessed_direction {
            let rtl = self.direction == ReadDirection::RightToLeft;
            if store
                .set_series_guess(
                    self.work.as_str(),
                    Key::Direction.name(),
                    if rtl { "rtl" } else { "ltr" },
                    now,
                )
                .unwrap_or(false)
            {
                said = Some(if rtl {
                    ReadingMsg::ReaderDetectedRightToLeft
                } else {
                    ReadingMsg::ReaderDetectedLeftToRight
                });
            }
        }
        if said.is_some() {
            self.layers = cx.layers(self.medium, Some(self.work.as_str()));
        }
        if !self.opened {
            self.opened = true;
            let marks = store.read_marks(self.work.as_str()).unwrap_or_default();
            let view = ReaderView {
                session: self.id,
                work: self.work.clone(),
                title: self.title.clone(),
                medium: self.medium,
                chapter: self.chapter().row(&marks),
                mode: self.mode,
                direction: self.direction,
                start: self.start.clone(),
                next: self.chapters.get(self.at + 1).map(|c| c.row(&marks)),
                previous: self
                    .at
                    .checked_sub(1)
                    .and_then(|i| self.chapters.get(i))
                    .map(|c| c.row(&marks)),
                settings: Some(cx.settings_view(self.medium, Some(&self.work))),
            };
            cx.send(ReaderEvent::Opened(Box::new(view)));
        }
        let mut leaves = self.leaves.clone();
        leaves.push(Leaf { index: leaves.len() as u32, parts: Vec::new(), end_page: true });
        cx.send(ReaderEvent::Leaves {
            session: self.id,
            chapter: self.chapter().id.clone(),
            leaves,
        });
        if let Some(msg) = said {
            cx.send(notice(msg, Some(ReaderAction::ChangeMode), RESUMED_MS, eink));
        }
        if let Some(page) = self.resumed.take() {
            let msg = ReadingMsg::ReaderResumed { page: i64::from(page) + 1 };
            cx.send(notice(msg, Some(ReaderAction::StartOver), RESUMED_MS, eink));
        }
        let start = self.leaf_of_page(self.start.page.unwrap_or(0));
        pages.want(self.id, self.generation, vec![start, start + 1]);
    }

    fn report(&mut self, cx: &mut Cx<'_>, pages: &Pages, command: ReaderCommand) {
        use ReaderCommand as C;
        // Any report stops the read rule's wait; the one that keeps it starts it again.
        self.read_timer += 1;
        match command {
            C::Shown { chapter, leaf, how, .. } if chapter == self.chapter().id => {
                self.shown(cx, pages, leaf, how)
            }
            C::StripAt { chapter, at, foot_visible, .. } if chapter == self.chapter().id => {
                let mut here = place(&self.work, &chapter, Some(at.image));
                here.strip = Some(at);
                self.place = Some(here);
                self.note_place(cx);
                if foot_visible && at.image + 1 >= self.pages_total {
                    self.wait_to_read(cx);
                }
            }
            C::Surface { surface, .. } => {
                self.surface = surface;
                let page = self.page().unwrap_or(0);
                self.lay_out(cx, pages, page);
            }
            C::NeedPage { page, .. } => pages.again(self.id, page),
            C::Retry { page: Some(page), .. } => pages.again(self.id, page),
            C::Retry { page: None, .. } => {
                let page = self.page().unwrap_or(0);
                self.lay_out(cx, pages, page);
            }
            C::OpenChapter { chapter, .. } => {
                let Some(index) = self.chapters.iter().position(|c| c.id == chapter) else {
                    return;
                };
                if index == self.at + 1 {
                    self.chapter_read(cx);
                }
                let store = &cx.services.store;
                let page = store
                    .read_chapters(self.work.as_str())
                    .unwrap_or_default()
                    .into_iter()
                    .find(|k| k.chapter == chapter.as_str() && !k.read)
                    .and_then(|k| k.last_page)
                    .unwrap_or(0);
                self.open_chapter(cx, pages, index, page);
            }
            C::BackTo { .. } => {
                if let Some(back) = self.back_to.take() {
                    self.go_to(cx, pages, back);
                }
            }
            C::StartOver { .. } => {
                let start = place(&self.work, &self.chapter().id, Some(0));
                self.go_to(cx, pages, start);
            }
            C::GoBack { .. } => {
                if let Some(back) = self.moved_back.take() {
                    self.go_to(cx, pages, back);
                }
            }
            C::NovelAt { .. } | C::NovelCounted { .. } | C::FixedRendered { .. } => {
                tracing::debug!("novel reports wait for the novel reader");
            }
            other => tracing::debug!(command = ?other, "a report for another chapter"),
        }
    }

    fn shown(&mut self, cx: &mut Cx<'_>, pages: &Pages, leaf: u32, how: TurnHow) {
        let story = self.leaves.len() as u32;
        if leaf >= story {
            // The end page, in the flow: read at once.
            self.chapter_read(cx);
            self.say_end_page(cx);
            return;
        }
        let (page, strip) = self.page_of_leaf(leaf);
        let mut here = place(&self.work, &self.chapter().id, page);
        here.strip = strip;
        if how == TurnHow::Jump
            && self.back_to.is_none()
            && let Some(before) = self.place.clone().filter(|p| p.page != here.page)
        {
            let page = i64::from(before.page.unwrap_or(0)) + 1;
            self.back_to = Some(before);
            cx.send(notice(
                ReadingMsg::ReaderBackToPage { page },
                Some(ReaderAction::BackTo),
                BACK_MS,
                self.eink(),
            ));
        }
        self.place = Some(here);
        self.note_place(cx);
        if leaf + 1 == story {
            self.say_end_page(cx);
            self.wait_to_read(cx);
        }
        let mut want = vec![leaf, leaf + 1, leaf + 2];
        if let Some(before) = leaf.checked_sub(1) {
            want.push(before);
        }
        pages.want(self.id, self.generation, want);
    }

    fn say_end_page(&mut self, cx: &Cx<'_>) {
        if std::mem::replace(&mut self.end_page_said, true) {
            return;
        }
        let marks = cx.services.store.read_marks(self.work.as_str()).unwrap_or_default();
        let next = self.chapters.get(self.at + 1).map(|c| c.row(&marks));
        let why_none = next.is_none().then_some(NoNext::CaughtUp);
        cx.send(ReaderEvent::EndPage { session: self.id, next, why_none });
    }

    fn wait_to_read(&mut self, cx: &Cx<'_>) {
        if !self.read_fired {
            after(cx.tx, READ_AFTER, self.id, ReaderWake::ReadRule(self.read_timer));
        }
    }

    /// The read rule fired: a finished read event, the trackers queued, `ChapterRead`.
    fn chapter_read(&mut self, cx: &Cx<'_>) {
        if std::mem::replace(&mut self.read_fired, true) {
            return;
        }
        let chapter = self.chapter().id.clone();
        let at = cx.now();
        let mut event = ReadEvent::new(self.work.as_str(), chapter.as_str(), at);
        event.completed = true;
        event.page = self.pages_total.checked_sub(1);
        event.of_pages = Some(self.pages_total).filter(|n| *n > 0);
        event.source_id = Some("local".into());
        let queued_for = queue_trackers(cx, &self.work, &chapter, at);
        if let Err(e) = cx.services.store.record_read(&event, queued_for) {
            tracing::warn!(error = %e, "a read chapter was not kept");
        }
        self.dirty = false;
        let _ = cx.tx.send(Event::ProgressQueued);
        cx.send(ReaderEvent::ChapterRead { work: self.work.clone(), chapter });
    }

    fn note_place(&mut self, cx: &Cx<'_>) {
        self.dirty = true;
        let waited = self.last_write.map_or(WRITE_EVERY, |t| t.elapsed());
        if waited >= WRITE_EVERY {
            self.flush(cx);
        } else if !self.flush_pending {
            self.flush_pending = true;
            self.flush_timer += 1;
            after(cx.tx, WRITE_EVERY - waited, self.id, ReaderWake::Flush(self.flush_timer));
        }
    }

    /// Write the place if it moved since the last write.
    fn flush(&mut self, cx: &Cx<'_>) {
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        let Some(here) = self.place.clone() else { return };
        self.last_write = Some(Instant::now());
        let mut event = ReadEvent::new(self.work.as_str(), here.chapter.as_str(), cx.now());
        event.page = here.page;
        event.of_pages = Some(self.pages_total).filter(|n| *n > 0);
        event.strip_image = here.strip.map(|s| s.image);
        event.strip_offset = here.strip.map(|s| f64::from(s.offset));
        event.source_id = Some("local".into());
        if let Err(e) = cx.services.store.record_read(&event, Vec::new()) {
            tracing::warn!(error = %e, "the reading place was not kept");
        }
    }

    fn open_chapter(&mut self, cx: &mut Cx<'_>, pages: &Pages, index: usize, page: u32) {
        self.flush(cx);
        self.at = index;
        self.opened = false;
        self.read_fired = false;
        self.end_page_said = false;
        self.back_to = None;
        self.place = None;
        self.start = place(&self.work, &self.chapters[index].id, Some(page));
        self.lay_out(cx, pages, page);
    }

    /// Show `to`: in the open chapter with `GoTo`, else by opening its chapter.
    fn go_to(&mut self, cx: &mut Cx<'_>, pages: &Pages, to: ReadPlace) {
        if to.chapter == self.chapter().id {
            cx.send(ReaderEvent::GoTo { session: self.id, place: to });
        } else if let Some(index) = self.chapters.iter().position(|c| c.id == to.chapter) {
            self.open_chapter(cx, pages, index, to.page.unwrap_or(0));
        }
    }

    /// A newer place from another device moves the reader only when it is further on.
    fn moved_on(&mut self, cx: &mut Cx<'_>, pages: &Pages) {
        let store = &cx.services.store;
        let Some(last) =
            store.read_work(self.work.as_str()).ok().flatten().and_then(|w| w.last_chapter)
        else {
            return;
        };
        let Some(theirs) = store
            .read_chapters(self.work.as_str())
            .ok()
            .and_then(|c| c.into_iter().find(|c| c.chapter == last))
        else {
            return;
        };
        let Some(device) = theirs.device.clone() else { return };
        if store.sync_device().ok().flatten().as_deref() == Some(device.as_str()) {
            return;
        }
        let Some(index) = self.chapters.iter().position(|c| c.id.as_str() == last) else {
            return;
        };
        let mine = self.page().unwrap_or(0);
        let page = theirs.last_page.unwrap_or(0);
        let further = index > self.at || (index == self.at && page > mine);
        if theirs.read || !further {
            return;
        }
        let name = store
            .sync_peers()
            .ok()
            .and_then(|peers| peers.into_iter().find(|p| p.0 == device).and_then(|p| p.1))
            .unwrap_or_else(|| device.clone());
        let back = self
            .place
            .clone()
            .unwrap_or_else(|| place(&self.work, &self.chapter().id, Some(mine)));
        let to = place(&self.work, &self.chapters[index].id, Some(page));
        let label = self.chapters[index].label.clone();
        self.moved_back = Some(back.clone());
        self.go_to(cx, pages, to.clone());
        cx.send(ReaderEvent::Moved { session: self.id, to, from_device: name.clone(), back });
        let msg =
            ReadingMsg::ReaderMoved { chapter: label, page: i64::from(page) + 1, device: name };
        cx.send(notice(msg, Some(ReaderAction::GoBack), MOVED_MS, self.eink()));
    }
}

/// Queue the chapter for the trackers signed in, for a work AniList lists: the highest whole
/// chapter, never lower than a tracker holds.
fn queue_trackers(cx: &Cx<'_>, work: &WorkId, chapter: &ChapterId, at: i64) -> Vec<String> {
    let Some(id) = work.as_str().strip_prefix("a:").and_then(|n| n.parse::<u32>().ok()) else {
        return Vec::new();
    };
    let Some(number) = chapter.key().and_then(|k| k.range()).map(|r| r.last().whole()) else {
        return Vec::new();
    };
    let op = TrackOp::SetReadProgress {
        anilist_id: AnilistId::new(id),
        chapter: number,
        volume: None,
    };
    crate::tracking::commands::signed_in_tracker_ids(&cx.services.sync)
        .into_iter()
        .filter(|tracker| cx.services.store.enqueue(tracker, &op, at).is_ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{io::Write as _, path::Path, sync::Arc};

    use kaeri_core::config::{Config, Paths};
    use kaeri_model::{playback::PlayerTheme, reader::PageEncoding};
    use kaeri_net::HttpClient;
    use kaeri_providers::ProviderRegistry;
    use kaeri_store::{
        Store,
        shelf::{LocalSeries, ShelfFile},
    };
    use tokio::sync::mpsc::UnboundedReceiver;

    use super::*;
    use crate::{host::QuietHost, outbox::Envelope};

    const WORK: &str = "a:4242";

    /// A comic archive of `pages` portrait pages, each a flat grey of its own.
    fn cbz(path: &Path, pages: u8) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for i in 0..pages {
            let page = image::GrayImage::from_pixel(300, 450, image::Luma([40 + i * 20]));
            let mut png = Vec::new();
            page.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .unwrap();
            zip.start_file(format!("{i:03}.png"), stored).unwrap();
            zip.write_all(&png).unwrap();
        }
        zip.finish().unwrap();
    }

    struct Harness {
        reader: Reader,
        config: ConfigState,
        services: Services,
        tx: Outbox,
        inbox: UnboundedReceiver<Envelope>,
        /// Reader events seen and not yet taken, in order.
        seen: std::collections::VecDeque<ReaderEvent>,
        /// Sessions whose leaves went out.
        laid: std::collections::HashSet<u64>,
        _dir: tempfile::TempDir,
    }

    fn theme() -> PlayerTheme {
        PlayerTheme {
            text: "#E4E0D6".into(),
            dim: "#A3A9C4".into(),
            obi: "#F2A64B".into(),
            state: "#7FC7D9".into(),
            alert: "#E06B54".into(),
            ground: "#161A2E".into(),
            font: "sans-serif".into(),
        }
    }

    /// A reader over a work of two three-page chapters in a folder.
    fn harness(store: Store) -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let books = dir.path().join("books");
        std::fs::create_dir_all(&books).unwrap();
        let series = LocalSeries {
            id: "s1".into(),
            folder: books.display().to_string(),
            name: "The Lantern Keeper".into(),
            medium: "manga".into(),
            work: Some(WORK.into()),
            state: "matched".into(),
        };
        store.put_local_series(&series).unwrap();
        for n in 1..=2 {
            let path = books.join(format!("Chapter {n}.cbz"));
            cbz(&path, 3);
            let file = ShelfFile {
                path: path.display().to_string(),
                chapter: None,
                label: format!("Chapter {n}"),
                volume: None,
                number_milli: Some(n * 1000),
                format: "cbz".into(),
                pages: Some(3),
                comicinfo: None,
            };
            store.put_local_file("s1", &file, 1, 1).unwrap();
        }
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let config = Config::default();
        let http = HttpClient::offline(&config.network).unwrap();
        let host = Arc::new(QuietHost::in_dir(&dir.path().join("tokens")));
        let registry = ProviderRegistry::new(Vec::new());
        let services =
            Services::build(&config, paths.clone(), http, store, registry, theme(), host)
                .unwrap();
        let config = ConfigState::new(config, None, &paths);
        let (tx, inbox) = Outbox::inbox();
        Harness {
            reader: Reader::default(),
            config,
            services,
            tx,
            inbox,
            seen: Default::default(),
            laid: Default::default(),
            _dir: dir,
        }
    }

    fn surface() -> ReaderSurface {
        ReaderSurface {
            width_px: 600,
            height_px: 900,
            scale: 2.0,
            eink: false,
            two_pages: false,
            dark: false,
        }
    }

    impl Harness {
        fn send(&mut self, command: ReaderCommand) {
            let mut cx = self.reader.cx(&mut self.config, &self.services, &self.tx);
            self.reader.command(&mut cx, command);
        }

        fn synced(&mut self) {
            let mut cx = self.reader.cx(&mut self.config, &self.services, &self.tx);
            self.reader.synced(&mut cx);
        }

        fn session(&self) -> u64 {
            self.reader.session.as_ref().unwrap().id
        }

        /// Carry out one envelope as the actor does, keeping a reader event.
        fn carry(&mut self, envelope: Envelope) {
            match envelope {
                Envelope::Internal(Internal::Reader { session, wake }) => {
                    let mut cx = self.reader.cx(&mut self.config, &self.services, &self.tx);
                    self.reader.wake(&mut cx, session, wake);
                }
                Envelope::Event(Event::Reader(event)) => {
                    match &event {
                        ReaderEvent::PageFailed { failure, .. } => {
                            panic!("a page failed: {failure:?}")
                        }
                        ReaderEvent::Leaves { session, .. } => {
                            self.laid.insert(*session);
                        }
                        ReaderEvent::PageReady { session, .. } => {
                            assert!(self.laid.contains(session), "a page before its leaves");
                        }
                        _ => {}
                    }
                    self.seen.push_back(event);
                }
                Envelope::Event(Event::Reading(ReadingEvent::Failed { failure, .. })) => {
                    panic!("the reader failed: {failure:?}");
                }
                _ => {}
            }
        }

        /// The first reader event `pick` takes, seen already or waited for; what it passes
        /// over stays for later.
        async fn until<T>(&mut self, mut pick: impl FnMut(&ReaderEvent) -> Option<T>) -> T {
            let mut looked = 0;
            loop {
                while looked < self.seen.len() {
                    if let Some(found) = pick(&self.seen[looked]) {
                        self.seen.remove(looked);
                        return found;
                    }
                    looked += 1;
                }
                let envelope = self.inbox.recv().await.expect("the inbox is open");
                self.carry(envelope);
            }
        }

        /// Everything waiting, carried out, and the reader events not yet taken.
        fn drain(&mut self) -> Vec<ReaderEvent> {
            while let Ok(envelope) = self.inbox.try_recv() {
                self.carry(envelope);
            }
            self.seen.drain(..).collect()
        }

        /// Open at `at` and wait for the view and its leaves.
        async fn open(&mut self, at: OpenAt) -> (ReaderView, Vec<Leaf>, Vec<ReaderEvent>) {
            let work = WorkId::new(WORK);
            self.send(ReaderCommand::Open { work, chapter: None, at, surface: surface() });
            let view = self
                .until(|e| match e {
                    ReaderEvent::Opened(view) => Some(view.as_ref().clone()),
                    _ => None,
                })
                .await;
            let leaves = self
                .until(|e| match e {
                    ReaderEvent::Leaves { leaves, .. } => Some(leaves.clone()),
                    _ => None,
                })
                .await;
            // The notices go out right behind the leaves.
            let notices = self.drain();
            self.seen.extend(notices.iter().cloned());
            (view, leaves, notices)
        }

        async fn shown(&mut self, leaf: u32, how: TurnHow) {
            let session = self.session();
            let chapter = self.reader.session.as_ref().unwrap().chapter().id.clone();
            self.send(ReaderCommand::Shown { session, chapter, leaf, how });
        }

        fn kept(&self, chapter: &str) -> Option<kaeri_store::reading::ReadChapter> {
            let store = &self.services.store;
            store.read_chapters(WORK).unwrap().into_iter().find(|c| c.chapter == chapter)
        }
    }

    fn notices(events: &[ReaderEvent]) -> Vec<ReadingMsg> {
        events
            .iter()
            .filter_map(|e| match e {
                ReaderEvent::Notice(n) => match &n.msg {
                    Msg::Reading(m) => Some(m.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    #[tokio::test(start_paused = true)]
    async fn a_chapter_opens_on_its_pages_and_counts_read_after_the_last_page_holds() {
        let mut h = harness(Store::open_in_memory().unwrap());
        let (view, leaves, _) = h.open(OpenAt::Resume).await;
        assert_eq!(view.chapter.id.as_str(), "1");
        assert_eq!(view.next.as_ref().map(|c| c.id.as_str()), Some("2"));
        assert!(view.settings.as_ref().is_some_and(|s| !s.rows.is_empty()), "the sheet's rows");
        assert_eq!(leaves.len(), 4, "three pages and the end page");
        assert!(leaves[3].end_page && leaves[..3].iter().all(|l| l.parts.len() == 1));

        // The first page lands first, as the original: the screen decodes it.
        let (page, file) = h
            .until(|e| match e {
                ReaderEvent::PageReady { page, file, .. } => Some((*page, file.clone())),
                _ => None,
            })
            .await;
        assert_eq!(page, 0);
        assert!(matches!(file.encoding, PageEncoding::Original { .. }), "{file:?}");
        assert!(Path::new(&file.path).exists());

        h.shown(0, TurnHow::Turn).await;
        assert_eq!(h.kept("1").and_then(|c| c.last_page), Some(0), "the first place at once");
        h.shown(2, TurnHow::Turn).await;
        let end =
            h.until(|e| matches!(e, ReaderEvent::EndPage { .. }).then(|| e.clone())).await;
        let ReaderEvent::EndPage { next, why_none, .. } = end else { unreachable!() };
        assert_eq!((next.map(|c| c.id.0), why_none), (Some("2".into()), None));
        assert!(!h.kept("1").unwrap().read, "not read before the last page holds");

        let before = Instant::now();
        let read = h
            .until(|e| match e {
                ReaderEvent::ChapterRead { chapter, .. } => Some(chapter.clone()),
                _ => None,
            })
            .await;
        assert_eq!(read.as_str(), "1");
        assert!(before.elapsed() >= READ_AFTER - Duration::from_millis(10));
        assert!(h.kept("1").unwrap().read);

        // Opened again, a finished chapter resumes at the next one, from its start.
        let session = h.session();
        h.send(ReaderCommand::Close { session });
        let (view, _, said) = h.open(OpenAt::Resume).await;
        assert_eq!((view.chapter.id.as_str(), view.start.page), ("2", Some(0)));
        assert!(
            notices(&said).is_empty() && notices(&h.drain()).is_empty(),
            "nothing to resume"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn turning_away_from_the_last_page_holds_the_read_rule_and_the_end_page_reads_at_once()
     {
        let mut h = harness(Store::open_in_memory().unwrap());
        h.open(OpenAt::Resume).await;
        h.shown(2, TurnHow::Turn).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        h.shown(1, TurnHow::Turn).await;
        tokio::time::sleep(READ_AFTER * 2).await;
        let said = h.drain();
        assert!(!said.iter().any(|e| matches!(e, ReaderEvent::ChapterRead { .. })), "{said:?}");
        assert!(!h.kept("1").unwrap().read);

        h.shown(3, TurnHow::Turn).await;
        let said = h.drain();
        assert!(said.iter().any(|e| matches!(e, ReaderEvent::ChapterRead { .. })), "at once");
        assert!(h.kept("1").unwrap().read);
    }

    #[tokio::test(start_paused = true)]
    async fn places_are_written_once_a_second_and_resume_says_where() {
        let mut h = harness(Store::open_in_memory().unwrap());
        h.open(OpenAt::Resume).await;
        h.shown(0, TurnHow::Turn).await;
        h.shown(1, TurnHow::Turn).await;
        assert_eq!(h.kept("1").and_then(|c| c.last_page), Some(0), "the second waits");
        tokio::time::sleep(WRITE_EVERY + Duration::from_millis(50)).await;
        h.drain();
        assert_eq!(h.kept("1").and_then(|c| c.last_page), Some(1));

        // Close writes what was waiting, without the second's wait.
        h.shown(2, TurnHow::Turn).await;
        h.shown(1, TurnHow::Turn).await;
        let session = h.session();
        h.send(ReaderCommand::Close { session });
        assert_eq!(h.kept("1").and_then(|c| c.last_page), Some(1));

        let (view, _, said) = h.open(OpenAt::Resume).await;
        assert_eq!((view.chapter.id.as_str(), view.start.page), ("1", Some(1)));
        assert_eq!(notices(&said), [ReadingMsg::ReaderResumed { page: 2 }]);
        let notice = said.iter().find_map(|e| match e {
            ReaderEvent::Notice(n) => Some(n),
            _ => None,
        });
        let notice = notice.unwrap();
        assert_eq!(
            (notice.action, notice.lasts_ms),
            (Some(ReaderAction::StartOver), RESUMED_MS)
        );

        h.send(ReaderCommand::StartOver { session: h.session() });
        let to = h
            .until(|e| match e {
                ReaderEvent::GoTo { place, .. } => Some(place.clone()),
                _ => None,
            })
            .await;
        assert_eq!(to.page, Some(0));
    }

    #[tokio::test(start_paused = true)]
    async fn a_jump_offers_the_page_it_left() {
        let mut h = harness(Store::open_in_memory().unwrap());
        h.open(OpenAt::Resume).await;
        h.shown(0, TurnHow::Turn).await;
        h.shown(1, TurnHow::Turn).await;
        h.shown(2, TurnHow::Jump).await;
        assert_eq!(notices(&h.drain()), [ReadingMsg::ReaderBackToPage { page: 2 }]);
        h.send(ReaderCommand::BackTo { session: h.session() });
        let to = h
            .until(|e| match e {
                ReaderEvent::GoTo { place, .. } => Some(place.clone()),
                _ => None,
            })
            .await;
        assert_eq!((to.chapter.as_str(), to.page), ("1", Some(1)));
    }

    #[tokio::test(start_paused = true)]
    async fn the_next_chapter_counts_this_one_read() {
        let mut h = harness(Store::open_in_memory().unwrap());
        h.open(OpenAt::Resume).await;
        h.shown(0, TurnHow::Turn).await;
        let session = h.session();
        h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
        let read = h
            .until(|e| match e {
                ReaderEvent::ChapterRead { chapter, .. } => Some(chapter.clone()),
                _ => None,
            })
            .await;
        assert_eq!(read.as_str(), "1");
        let view = h
            .until(|e| match e {
                ReaderEvent::Opened(view) => Some(view.chapter.id.clone()),
                _ => None,
            })
            .await;
        assert_eq!(view.as_str(), "2");
    }

    #[tokio::test(start_paused = true)]
    async fn a_device_that_read_further_moves_the_reader_and_one_behind_does_not() {
        let mine = Store::open_in_memory().unwrap();
        mine.sync_enable("phone").unwrap();
        let desk = Store::open_in_memory().unwrap();
        desk.set_sync_clock(|| 4_000_000_000_000);
        desk.sync_enable("desk").unwrap();
        mine.sync_device_add("desk", "Desk", "").unwrap();
        let mut h = harness(mine.clone());
        h.open(OpenAt::Resume).await;
        h.shown(1, TurnHow::Turn).await;

        let carry = |desk: &Store| {
            let ops = desk.ops_after(&mine.version_vector().unwrap(), usize::MAX).unwrap();
            mine.apply_ops(ops).unwrap();
        };
        // Behind: page 1 of chapter 1 is where this device already is.
        let mut behind = ReadEvent::new(WORK, "1", 1);
        behind.page = Some(0);
        desk.record_read(&behind, Vec::new()).unwrap();
        carry(&desk);
        h.synced();
        assert!(!h.drain().iter().any(|e| matches!(e, ReaderEvent::Moved { .. })));

        let mut ahead = ReadEvent::new(WORK, "2", 2);
        ahead.page = Some(2);
        desk.record_read(&ahead, Vec::new()).unwrap();
        carry(&desk);
        h.synced();
        let said = h.drain();
        let moved = said.iter().find_map(|e| match e {
            ReaderEvent::Moved { to, from_device, back, .. } => {
                Some((to.chapter.0.clone(), to.page, from_device.clone(), back.page))
            }
            _ => None,
        });
        assert_eq!(moved, Some(("2".into(), Some(2), "Desk".into(), Some(1))));
        assert_eq!(
            notices(&said),
            [ReadingMsg::ReaderMoved {
                chapter: "Chapter 2".into(),
                page: 3,
                device: "Desk".into()
            }]
        );
        let opened = h
            .until(|e| match e {
                ReaderEvent::Opened(view) => Some((view.chapter.id.clone(), view.start.page)),
                _ => None,
            })
            .await;
        assert_eq!((opened.0.as_str(), opened.1), ("2", Some(2)));

        h.send(ReaderCommand::GoBack { session: h.session() });
        let back = h
            .until(|e| match e {
                ReaderEvent::Opened(view) => Some((view.chapter.id.clone(), view.start.page)),
                _ => None,
            })
            .await;
        assert_eq!((back.0.as_str(), back.1), ("1", Some(1)));
    }

    #[tokio::test(start_paused = true)]
    async fn on_a_grey_e_ink_screen_pages_come_processed_and_notices_wait_for_the_turn() {
        let mut h = harness(Store::open_in_memory().unwrap());
        h.reader.facts = Some(kaeri_model::eink::ScreenFacts {
            manufacturer: "ONYX".into(),
            brand: "ONYX".into(),
            model: "Reader 7".into(),
            device: String::new(),
            product: String::new(),
            hardware: String::new(),
            sdk: 33,
            width_px: 600,
            height_px: 900,
            dpi: 300,
            refresh_hz: 60.0,
            eink_support: None,
            onyx_name: None,
            wide_colour_gamut: false,
            hooks: vec!["onyx".into()],
            page_keys: true,
            webview: Some(120),
        });
        let profile = h.reader.detect(h.config.effective()).unwrap();
        assert_eq!(profile.panel, kaeri_model::eink::Panel::Grey);
        let work = WorkId::new(WORK);
        let surface = ReaderSurface { eink: true, ..surface() };
        h.send(ReaderCommand::Open { work, chapter: None, at: OpenAt::Resume, surface });
        let file = h
            .until(|e| match e {
                ReaderEvent::PageReady { file, .. } => Some(file.clone()),
                _ => None,
            })
            .await;
        assert_eq!(
            file.encoding,
            PageEncoding::Raw { format: kaeri_model::reader::PixelFormat::Gray8, stride: 600 }
        );
        assert_eq!(
            std::fs::metadata(&file.path).unwrap().len(),
            u64::from(file.width * file.height)
        );
        h.shown(0, TurnHow::Turn).await;
        h.shown(2, TurnHow::Jump).await;
        let said = h.drain();
        let lasts = said.iter().find_map(|e| match e {
            ReaderEvent::Notice(n) => Some(n.lasts_ms),
            _ => None,
        });
        assert_eq!(lasts, Some(0));
    }

    #[tokio::test(start_paused = true)]
    async fn settings_load_with_their_rows_and_a_series_choice_lays_out_again() {
        let mut h = harness(Store::open_in_memory().unwrap());
        h.send(ReaderCommand::LoadSettings { medium: Medium::Manga, work: None });
        let rows = h
            .until(|e| match e {
                ReaderEvent::Settings(view) => Some(view.rows.clone()),
                _ => None,
            })
            .await;
        assert!(rows.iter().any(|r| r.key == Key::Direction));
        assert!(
            !rows.iter().any(|r| r.key == Key::RefreshText),
            "no e-ink rows on this screen"
        );

        h.open(OpenAt::Resume).await;
        let session = h.session();
        let work = WorkId::new(WORK);
        h.send(ReaderCommand::SetSetting {
            session: Some(session),
            medium: Medium::Manga,
            work: Some(work.clone()),
            key: Key::Mode,
            scope: kaeri_model::reader::SettingScope::SeriesHere,
            value: kaeri_model::reader::ReaderValue::Choice { name: "strip".into() },
        });
        let mode = h
            .until(|e| match e {
                ReaderEvent::Leaves { .. } => Some(()),
                _ => None,
            })
            .await;
        assert_eq!(mode, ());
        assert_eq!(h.reader.session.as_ref().unwrap().mode, ReaderMode::Strip);
    }
}
