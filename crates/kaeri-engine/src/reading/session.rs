//! The open reader session: where the person is, when a chapter counts as read, resume and
//! Back to page, and moves from other devices.
//!
//! The app reports what it painted; the session decides what that means. A place is written at
//! most once a second per chapter, and always on Close and when the app leaves the
//! foreground. The read rule waits 2 s on the last leaf (or a strip's foot) and fires at once
//! on the end page or when the next chapter opens. Times come from the engine's clock; the
//! timers run on the runtime and come back as [`Internal::Reader`] wakes.

mod novel;
pub(crate) use novel::SOURCE;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use kaeri_core::{
    books::{ChapterId, WorkId},
    config::{CountsAsRead, SendProgressAfter},
    ids::AnilistId,
    media::Medium,
    traits::TrackOp,
};
use kaeri_model::{
    Msg, ReadingMsg,
    eink::{EinkProfile, ScreenFacts},
    event::Event,
    reader::{
        Leaf, NoNext, NovelPos, OpenAt, ReadDirection, ReadPlace, ReaderAction, ReaderCommand,
        ReaderEvent, ReaderMode, ReaderNotice, ReaderSettingKey as Key, ReaderSettingsView,
        ReaderSurface, ReaderView, ReaderWait, StripPos, TurnHow,
    },
    reading::{ReadingAbout, ReadingEvent, ReadingFailure},
};
use kaeri_pages::{ColourChoice, Look, Target, layout::SpreadOnSingle};
use kaeri_store::{reading::ReadEvent, shelf::BookWorkKind};
use tokio::time::Instant;

use super::{
    eink,
    pages::{Laid, OpenJob, Pages, PeekJob, Plan, Shared},
    settings::{self, Layers, Screen, SourceWord},
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
/// The person's own jump of more than this many pages, or screens of a novel, offers Back to
/// page.
const BACK_PAST: u32 = 2;

/// What the reader needs from the rest of the engine for one command.
pub struct Cx<'a> {
    pub config: &'a mut ConfigState,
    pub services: &'a Services,
    pub tx: &'a Outbox,
    pub screen: Screen,
    pub eink: Option<EinkProfile>,
    /// The system web view's major version, where the app said it (Android).
    pub webview: Option<u32>,
}

impl Cx<'_> {
    fn send(&self, event: ReaderEvent) {
        let _ = self.tx.send(Event::Reader(event));
    }

    fn now(&self) -> i64 {
        self.services.clock.now()
    }

    fn layers(&self, medium: Medium, work: Option<&str>) -> Layers {
        let source = work.map(|w| self.source_word(&WorkId::new(w)));
        self.layers_with(medium, work, source)
    }

    /// The layers with the source's word as given: a session's, read once when it opened, so a
    /// hint that comes later changes nothing until the series is opened again.
    fn layers_with(
        &self,
        medium: Medium,
        work: Option<&str>,
        source: Option<SourceWord>,
    ) -> Layers {
        let layers = Layers::read(
            &self.services.store,
            &self.config.effective().reading,
            medium,
            work,
            self.screen,
        );
        match source {
            Some(source) => layers.with_source(source),
            None => layers,
        }
    }

    /// What the series' source and the catalogue say of how it reads.
    fn source_word(&self, work: &WorkId) -> SourceWord {
        SourceWord::of(self.services, work)
    }

    pub fn settings_view(&self, medium: Medium, work: Option<&WorkId>) -> ReaderSettingsView {
        self.settings_view_with(medium, work, None)
    }

    /// The sheet as the open session reads it: with its source's word, when it is the work's.
    fn settings_view_with(
        &self,
        medium: Medium,
        work: Option<&WorkId>,
        session: Option<SourceWord>,
    ) -> ReaderSettingsView {
        let id = work.map(WorkId::as_str);
        let layers = match session {
            Some(source) => self.layers_with(medium, id, Some(source)),
            None => self.layers(medium, id),
        };
        let tap_zones = (medium != Medium::Anime).then(|| layers.tap_zones());
        ReaderSettingsView { medium, work: work.cloned(), rows: layers.rows(), tap_zones }
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
    /// An open waiting for a source to list the work's chapters.
    listing: Option<Listing>,
    /// The novels' cache was emptied of an earlier run's files.
    novels_cleared: bool,
}

/// An open whose chapters come from a source.
#[derive(Debug)]
struct Listing {
    id: u64,
    work: WorkId,
    chapter: Option<ChapterId>,
    at: OpenAt,
    surface: ReaderSurface,
    slot: novel::Listed,
    /// What the wait was for.
    then: Then,
}

/// What an open waiting off the actor does once its wait ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Then {
    /// Open from the chapters in the slot.
    Open,
    /// Book plugins were loading: look for the work's chapters again, now they are in.
    Look,
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
    /// The first layout has asked whether another device read further.
    moved_checked: bool,
    start: ReadPlace,
    /// Where the person is, from the last report.
    place: Option<ReadPlace>,
    /// Show "Resumed at page N" once the leaves are out.
    resumed: Option<u32>,
    /// How many pages the chapter had where its kept place was read, until it is laid out: a
    /// place read in another source's split of the chapter is moved to the same point in this.
    kept_of: Option<u32>,
    /// The catalogue says no more chapters are coming.
    ended: bool,
    /// The next chapter's first page was asked of the worker.
    peeked: bool,
    /// The next chapter from a source, being fetched while this one is read.
    ahead: Option<novel::Ahead>,
    /// The next chapter's first page for the end page, followed from the download queue's fetch
    /// of that chapter.
    first_page: Option<novel::FirstPage>,
    /// The place before the person's own jump, while Back to page is offered.
    back_to: Option<ReadPlace>,
    /// The place before another device moved the reader, for Go back.
    moved_back: Option<ReadPlace>,
    read_fired: bool,
    /// A chapter read that the trackers hear of when the reader closes, when progress is sent
    /// then.
    unsent: Option<(ChapterId, i64)>,
    end_page_said: bool,
    read_timer: u64,
    flush_timer: u64,
    flush_pending: bool,
    dirty: bool,
    last_write: Option<Instant>,
    /// The chapter and page of the place last written.
    written: Option<(ChapterId, Option<u32>)>,
    /// The place last written counted past credit pages learnt while it was open, and that count.
    written_past_credits: Option<(ReadPlace, u32)>,
    /// The novel open in the web view.
    novel: Option<novel::Open>,
    /// Where the next novel chapter opens.
    restore: Option<NovelPos>,
    /// What a chapter opened off the actor gave.
    prepared: novel::Slot,
    /// The source's chapter opening off the actor, until what it gave is laid out.
    source_open: Option<novel::SourceOpen>,
    /// The open chapter's pages from a source, landing as they are fetched.
    feed: Option<Arc<crate::reading::feed::Feed>>,
    /// "Resumed at page N" waits for a novel's first page.
    novel_resumed: bool,
    /// A work of the reading demo, whose credit the reader shows.
    demo: Option<kaeri_model::sources::DemoWorkInfo>,
    /// What the source and the catalogue said of how the series reads, when it opened.
    source: SourceWord,
    /// Said on close, for the pages still coming from a source into the session's folder.
    gone: crate::reading::feed::Gone,
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
    // Holds the sender it was lent: the engine keeps none between one piece of work and the
    // next, so a weak one would be closed before the timer ran.
    let tx = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(wait).await;
        let _ = tx.internal(Internal::Reader { session, wake });
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
        let screen = Screen::new(self.eink.as_ref(), config.handheld(), config.phone());
        let webview = self.facts.as_ref().and_then(|f| f.webview);
        Cx { config, services, tx, screen, eink: self.eink.clone(), webview }
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
                Pages::start(dir, cx.services.host.page_decoder())
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
                let session = self.source_of(work.as_ref());
                cx.send(ReaderEvent::Settings(cx.settings_view_with(
                    medium,
                    work.as_ref(),
                    session,
                )));
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
            ReaderCommand::TapGuideSeen { guide } => {
                let paths = cx.services.paths.clone();
                if let Err(e) = settings::guide_seen(cx.config, &paths, &guide) {
                    tracing::warn!(error = %e, "the tap zone guide was not kept as seen");
                }
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
                // Sessions count from 1: 0 is a close from an app that has none yet, which
                // drops the open still waiting and leaves an open session alone.
                if session == 0 {
                    self.listing = None;
                } else {
                    let open = self.session.as_ref().is_some_and(|s| s.id == session);
                    let pending = self.listing.as_ref().is_some_and(|l| l.id == session);
                    if open || pending {
                        self.close(cx);
                    }
                }
                cx.send(ReaderEvent::Closed { session });
            }
            other => {
                let pages = self.pages(cx);
                let Some(s) =
                    self.session.as_mut().filter(|s| Some(s.id) == session_of(&other))
                else {
                    // Reports for a closed session are dropped. A command the app waits on
                    // hears that the session is closed, so it never waits for good.
                    if let Some(session) = session_of(&other).filter(|_| waits_on(&other)) {
                        cx.send(ReaderEvent::Closed { session });
                    }
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
        let source = self.source_of(work.as_ref());
        cx.send(ReaderEvent::Settings(cx.settings_view_with(medium, work.as_ref(), source)));
        let pages = self.pages(cx);
        if let Some(s) = self
            .session
            .as_mut()
            .filter(|s| Some(s.id) == session || Some(&s.work) == work.as_ref())
        {
            s.layers = cx.layers_with(s.medium, Some(s.work.as_str()), Some(s.source));
            if s.novel.is_some() {
                s.restyle(cx);
            } else if settings::relays(key) {
                let page = s.page_now();
                s.lay_out(cx, &pages, page);
            }
        }
    }

    fn close(&mut self, cx: &Cx<'_>) {
        // An open still waiting for its source goes too: nothing opens when it answers.
        self.listing = None;
        if let Some(mut s) = self.session.take() {
            s.read_timer += 1;
            s.flush_leaving(cx);
            // Progress held for the close goes now: the furthest chapter read this time.
            if let Some((chapter, at)) = s.unsent.take()
                && !queue_trackers(cx, &s.work, &chapter, at).is_empty()
            {
                let _ = cx.tx.send(Event::ProgressQueued);
            }
            if let Some(pages) = &self.pages {
                pages.close(s.id);
            }
            cx.services.novels.remove(s.id);
            let _ = std::fs::remove_dir_all(
                novel::cache_dir(&cx.services.paths.cache_dir).join(format!("{}-fixed", s.id)),
            );
            // Its fetched pages go, and any still on their way go as they land.
            s.gone.set();
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
        if !std::mem::replace(&mut self.novels_cleared, true) {
            // An earlier run's files: session numbers start again each run.
            let _ = std::fs::remove_dir_all(novel::cache_dir(&cx.services.paths.cache_dir));
        }
        self.next += 1;
        let id = self.next;
        // Without the book plugins, the list would be the shelf alone and the chapter asked for
        // missed: ones still loading that may hold the work are waited for.
        cx.services.book_sources.take_in_plugins(&cx.services.book_plugins);
        if let Some(slot) = novel::plugins_coming(cx, id, &work) {
            self.listing =
                Some(Listing { id, work, chapter, at, surface, slot, then: Then::Look });
            cx.send(ReaderEvent::Opening { chapter: None, wait: None });
            return;
        }
        self.look(cx, id, work, chapter, at, surface);
    }

    /// The chapters of `work` as the shelf and its source have them, and the open from them:
    /// at once, or once the source has listed them.
    fn look(
        &mut self,
        cx: &mut Cx<'_>,
        id: u64,
        work: WorkId,
        chapter: Option<ChapterId>,
        at: OpenAt,
        surface: ReaderSurface,
    ) {
        cx.services.book_sources.take_in_plugins(&cx.services.book_plugins);
        let mut chapters = shelf_of(cx, &work);
        // A work from a source reads the source's whole list, the series page's, with what is
        // on the shelf read from its file. The list as kept serves when there is one to read;
        // with nothing on the shelf, the source is asked off the actor. A shelf with no list
        // kept beside it opens on its own, never waiting on the source.
        if novel::sourced(cx, &work) {
            let kept = novel::kept_list(cx, &work, &chapters);
            // A chapter asked for that the list as kept does not name yet (a notification of
            // one just out) is looked for in the source's list as it is now.
            let missing = |list: &[ShelfChapter]| {
                chapter.as_ref().is_some_and(|c| list.iter().all(|l| l.id != *c))
            };
            // Whether the source is asked for the list, and asked again past the kept one.
            let ask = match kept {
                Some(listed) if !missing(&listed) => {
                    chapters = listed;
                    None
                }
                Some(_) => Some(true),
                None if chapters.is_empty() || missing(&chapters) => Some(false),
                None => None,
            };
            if let Some(again) = ask {
                // The source is known: it is asked for the list, and the line names it.
                let source = cx.services.book_sources.edition_of(&work, None);
                let source = source.map(|(id, _)| cx.services.book_sources.source_name(&id));
                let slot = novel::list(cx, id, &work, chapters, again);
                self.listing =
                    Some(Listing { id, work, chapter, at, surface, slot, then: Then::Open });
                cx.send(ReaderEvent::Opening {
                    chapter: None,
                    wait: source.map(|source| ReaderWait::ChapterList { source }),
                });
                return;
            }
        }
        // No word kept yet on how the series reads, and a source being asked: the open waits
        // a moment for its answer.
        if !chapters.is_empty()
            && let Some(slot) = novel::after_hint(cx, id, &work, &chapters)
        {
            self.listing =
                Some(Listing { id, work, chapter, at, surface, slot, then: Then::Open });
            return;
        }
        self.open_with(cx, id, work, chapters, chapter, at, surface);
    }

    /// The open session's word from its source, when it reads `work`.
    fn source_of(&self, work: Option<&WorkId>) -> Option<SourceWord> {
        self.session.as_ref().filter(|s| Some(&s.work) == work).map(|s| s.source)
    }

    /// A source listed the chapters an open waited for.
    fn listed(&mut self, cx: &mut Cx<'_>, session: u64) {
        let Some(listing) = self.listing.take_if(|l| l.id == session) else { return };
        if listing.then == Then::Look {
            let Listing { id, work, chapter, at, surface, .. } = listing;
            self.look(cx, id, work, chapter, at, surface);
            return;
        }
        let listed = listing.slot.lock().ok().and_then(|mut s| s.take());
        match listed {
            Some(Ok(chapters)) if !chapters.is_empty() => {
                let Listing { id, work, chapter, at, surface, .. } = listing;
                self.open_with(cx, id, work, chapters, chapter, at, surface);
            }
            Some(Err(failure)) => {
                let _ = cx.tx.send(Event::Reading(ReadingEvent::Failed {
                    about: ReadingAbout::Reader,
                    failure,
                }));
            }
            _ => {
                let _ = cx.tx.send(Event::Reading(ReadingEvent::Failed {
                    about: ReadingAbout::Reader,
                    failure: ReadingFailure::NotFound,
                }));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn open_with(
        &mut self,
        cx: &mut Cx<'_>,
        id: u64,
        work: WorkId,
        chapters: Vec<ShelfChapter>,
        chapter: Option<ChapterId>,
        at: OpenAt,
        surface: ReaderSurface,
    ) {
        let store = &cx.services.store;
        let form = crate::data::title_form(cx.services.meta.titles());
        let kind = store.book_work_kind(work.as_str()).ok().flatten();
        // A demo work says its own medium: opened from Home or the first run before its page was
        // ever loaded, the catalogue has not fetched it, and its chapters come from the demo.
        let demo_work = kaeri_core::reading_demo::WORKS.iter().find(|w| w.work_id() == work);
        let medium = match (kind.as_ref().map(|k| k.medium.as_str()), chapters.first()) {
            _ if demo_work.is_some() => demo_work.map_or(Medium::Manga, |w| w.medium()),
            (Some("novel"), _) => Medium::Novel,
            (None, Some(c)) if c.is_text() => Medium::Novel,
            (Some(other), _) if other != "manga" && other != "comic" => {
                Medium::parse(other).unwrap_or(Medium::Manga)
            }
            _ => Medium::Manga,
        };
        let Some(Start { index, page, strip, resumed, of }) =
            start_of(cx, &work, &chapters, chapter.as_ref(), &at)
        else {
            let failure = ReadingFailure::NotFound;
            let _ = cx.tx.send(Event::Reading(ReadingEvent::Failed {
                about: ReadingAbout::Reader,
                failure,
            }));
            return;
        };
        let marks = crate::read_catalog::chapters::read_marks(store, work.as_str());
        let row = chapters[index].row(&marks);
        // A chapter not on this device is asked of its source, which the line names.
        let wait = (chapters[index].format == novel::SOURCE)
            .then(|| cx.services.book_sources.edition_of(&work, None))
            .flatten()
            .map(|(source, _)| ReaderWait::ChapterPages {
                chapter: row.number.clone().unwrap_or_else(|| row.label.clone()),
                source: cx.services.book_sources.source_name(&source),
            });
        cx.send(ReaderEvent::Opening { chapter: Some(row), wait });
        let source = cx.source_word(&work);
        let layers = cx.layers_with(medium, Some(work.as_str()), Some(source));
        // A novel opens at its kept text position, or the one asked for.
        let restore = match &at {
            OpenAt::Place { place } if place.chapter == chapters[index].id => {
                place.novel.clone()
            }
            OpenAt::Resume => novel::kept_novel(cx, &work, &chapters[index].id),
            _ => None,
        };
        let resumed = resumed || (matches!(at, OpenAt::Resume) && restore.is_some());
        let start = match &at {
            OpenAt::Place { place } if place.chapter == chapters[index].id => place.clone(),
            // A kept place in a long strip comes back to the same spot in its image.
            _ => ReadPlace { strip, ..place(&work, &chapters[index].id, Some(page)) },
        };
        let ended = kind.as_ref().is_some_and(BookWorkKind::ended);
        let demo = cx
            .services
            .book_sources
            .edition_of(&work, None)
            .filter(|(source, _)| source == kaeri_core::demo::DEMO_SOURCE_ID)
            .and_then(|(_, series)| kaeri_core::reading_demo::work(&series))
            .map(kaeri_model::sources::DemoWorkInfo::of);
        let mut session = Session {
            demo,
            source,
            id,
            work,
            medium,
            // The demo's works are never fetched from the catalogue: their series names them.
            title: kind
                .and_then(|k| k.title_in(form))
                .or_else(|| demo_work.map(|w| w.series.to_owned()))
                .unwrap_or_default(),
            ended,
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
            moved_checked: false,
            start,
            place: None,
            resumed: resumed.then_some(page),
            kept_of: of,
            peeked: false,
            ahead: None,
            first_page: None,
            back_to: None,
            moved_back: None,
            read_fired: false,
            unsent: None,
            end_page_said: false,
            read_timer: 0,
            flush_timer: 0,
            flush_pending: false,
            dirty: false,
            last_write: None,
            written: None,
            written_past_credits: None,
            novel: None,
            restore,
            prepared: Arc::default(),
            source_open: None,
            feed: None,
            novel_resumed: false,
            gone: crate::reading::feed::Gone::new(novel::streamed_dir(
                &cx.services.paths.cache_dir,
                id,
            )),
        };
        let pages = self.pages(cx);
        session.lay_out(cx, &pages, page);
        self.session = Some(session);
    }

    /// A timer ran out or the worker laid a chapter out.
    pub fn wake(&mut self, cx: &mut Cx<'_>, session: u64, wake: ReaderWake) {
        if wake == ReaderWake::Listed {
            self.listed(cx, session);
            return;
        }
        let pages = self.pages(cx);
        let Some(s) = self.session.as_mut().filter(|s| s.id == session) else { return };
        match wake {
            ReaderWake::Laid => s.laid_out(cx, &pages),
            // Every page of a source's chapter landed or failed and they lay out otherwise than
            // guessed: laid out again at the same page, as a change of screen is.
            ReaderWake::Relay(n) if n == s.generation && s.novel.is_none() => {
                let page = s.page_now();
                s.lay_out(cx, &pages, page);
            }
            ReaderWake::Relay(_) => {}
            ReaderWake::Prepared(n) => s.prepared_now(cx, &pages, n),
            ReaderWake::Listed => {}
            ReaderWake::ReadRule(n) if n == s.read_timer => s.chapter_read(cx, Reached::Stayed),
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
            s.flush_leaving(cx);
        }
    }

    /// The device went off line or came back: an end page already said follows the connection,
    /// since what it leads into, and why it has nothing, depend on it.
    pub fn network_moved(&mut self, cx: &mut Cx<'_>, was_offline: bool) {
        let pages = self.pages(cx);
        if let Some(s) = self.session.as_mut() {
            s.network_moved(cx, &pages, was_offline);
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
        | C::NotACreditPage { session, .. }
        | C::Close { session } => Some(*session),
        C::Open { .. }
        | C::SetSetting { .. }
        | C::UseDefault { .. }
        | C::TapGuideSeen { .. }
        | C::LoadSettings { .. } => None,
    }
}

/// Whether the app waits on an answer to `command`: a chapter or a place it asked to open, or
/// a chapter to lay out again.
fn waits_on(command: &ReaderCommand) -> bool {
    use ReaderCommand as C;
    matches!(
        command,
        C::OpenChapter { .. }
            | C::BackTo { .. }
            | C::GoBack { .. }
            | C::StartOver { .. }
            | C::Retry { .. }
    )
}

/// What is on the shelf of `work`, its downloads named as the work's list names them.
fn shelf_of(cx: &Cx<'_>, work: &WorkId) -> Vec<ShelfChapter> {
    let mut chapters = shelf::chapters(&cx.services.store, work.as_str());
    if let Some((_, _, list)) = crate::read_catalog::chapters::of_work(
        &cx.services.book_sources,
        &cx.services.store,
        work,
    ) {
        shelf::name_downloads(&mut chapters, &list);
    }
    chapters
}

/// Whether the download queue is fetching `chapter`, or will once its turn comes: queued, or
/// unread and about to be kept ready as the chapter before it opens.
fn downloading(cx: &Cx<'_>, work: &WorkId, chapter: &ChapterId) -> bool {
    use kaeri_store::book_downloads::BookDownloadState as State;
    let setup = cx.services.book_sources.downloads.setup();
    match cx.services.store.book_download(work.as_str(), chapter.as_str()) {
        Ok(Some(row)) => match row.state {
            State::Downloading => true,
            State::Queued => !setup.hold,
            _ => false,
        },
        Ok(None) if setup.keep_ready && !setup.hold => {
            !crate::read_catalog::chapters::read_marks(&cx.services.store, work.as_str())
                .is_read(chapter.as_str())
        }
        _ => false,
    }
}

/// Whether `chapter` opens with no network: downloaded, or a file on this device.
fn on_device(chapter: &ShelfChapter) -> bool {
    chapter.format != novel::SOURCE && !chapter.path.as_os_str().is_empty()
}

/// A chapter's row for the end page, which says when a downloaded one is here.
fn end_row(
    chapter: &ShelfChapter,
    marks: &kaeri_store::reading::ReadMarks,
) -> kaeri_model::reading::ChapterRow {
    let mut row = chapter.row(marks);
    if chapter.source.is_some() && on_device(chapter) {
        row.download = Some(kaeri_model::reading::ChapterDownloadState::Done);
    }
    row
}

/// Where a chapter opens.
struct Start {
    /// The chapter's index in the shelf.
    index: usize,
    page: u32,
    /// The kept place within a long strip's image, when the open resumes one.
    strip: Option<StripPos>,
    /// Whether that resumes a place.
    resumed: bool,
    /// The pages the chapter had where that place was kept.
    of: Option<u32>,
}

/// Which chapter to open and at which page, and whether that resumes a place.
fn start_of(
    cx: &Cx<'_>,
    work: &WorkId,
    chapters: &[ShelfChapter],
    chapter: Option<&ChapterId>,
    at: &OpenAt,
) -> Option<Start> {
    if chapters.is_empty() {
        return None;
    }
    let store = &cx.services.store;
    // Parts read under their own numbers count for their whole chapter.
    let folds = crate::read_catalog::chapters::Folds::of_work(store, work.as_str());
    let marks = folds.marks(store.read_marks(work.as_str()).unwrap_or_default());
    let kept = folds.places(store.read_chapters(work.as_str()).unwrap_or_default(), &marks);
    let index_of = |id: &ChapterId| chapters.iter().position(|c| &c.id == id);
    // The kept place of an unfinished chapter: its page, and in a long strip how far into that
    // page's image. A strip read partway down its first image is a place too.
    let left_at = |id: &ChapterId| {
        let k = kept.iter().find(|k| k.chapter == id.as_str() && !k.read)?;
        let strip = k
            .strip_image
            .zip(k.strip_offset)
            .map(|(image, offset)| StripPos { image, offset: (offset as f32).clamp(0.0, 1.0) });
        let page = k.last_page.or(strip.map(|s| s.image)).unwrap_or(0);
        let strip = strip.filter(|s| s.image == page);
        (page > 0 || strip.is_some_and(|s| s.offset > 0.0)).then_some((page, strip, k.of_pages))
    };
    let at_page = |index, page| Start { index, page, strip: None, resumed: false, of: None };
    // A chapter asked for and not in the list opens nothing: never another chapter in its place.
    let asked = || match chapter {
        Some(id) => index_of(id).map(Some),
        None => Some(None),
    };
    Some(match at {
        OpenAt::Place { place } => at_page(index_of(&place.chapter)?, place.page.unwrap_or(0)),
        OpenAt::Page { page } => at_page(asked()?.unwrap_or(0), *page),
        OpenAt::Start => {
            let first_unread =
                chapters.iter().position(|c| !marks.is_read(c.id.as_str())).unwrap_or(0);
            at_page(asked()?.unwrap_or(first_unread), 0)
        }
        OpenAt::Resume => {
            let index = match asked()? {
                Some(index) => index,
                None => {
                    let last = store
                        .read_work(work.as_str())
                        .ok()
                        .flatten()
                        .and_then(|w| w.last_chapter)
                        .map(|c| folds.whole_of(&c).unwrap_or(c))
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
                Some((page, strip, of)) => Start { index, page, strip, resumed: true, of },
                None => at_page(index, 0),
            }
        }
    })
}

impl Session {
    /// What `Opened` says of the open chapter.
    fn view(&self, cx: &Cx<'_>) -> ReaderView {
        let marks =
            crate::read_catalog::chapters::read_marks(&cx.services.store, self.work.as_str());
        ReaderView {
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
            settings: Some(cx.settings_view_with(
                self.medium,
                Some(&self.work),
                Some(self.source),
            )),
            demo_credit: self.demo.clone(),
            progress_host: cx.services.book_sources.progress_host(&self.work),
        }
    }

    fn chapter(&self) -> &ShelfChapter {
        &self.chapters[self.at]
    }

    fn eink(&self) -> bool {
        self.surface.eink
    }

    fn page(&self) -> Option<u32> {
        self.place.as_ref().and_then(|p| p.page.or(p.strip.map(|s| s.image)))
    }

    /// The page a layout again keeps the reader at: the one shown, or until one is, the one the
    /// chapter opens at. A turned screen or a retry before the first leaf showed keeps the start.
    fn page_now(&self) -> u32 {
        self.page().or(self.start.page).unwrap_or(0)
    }

    /// The page a leaf starts on, and for a strip cut on e-ink its place in the strip.
    fn page_of_leaf(&self, leaf: u32) -> (Option<u32>, Option<StripPos>) {
        if let Some(cut) = self.cuts.get(leaf as usize) {
            return (Some(cut.image), Some(*cut));
        }
        let page = self.leaves.get(leaf as usize).and_then(|l| l.parts.first()).map(|p| p.page);
        (page, None)
    }

    /// The e-ink leaf of a strip cut that holds `at`: the last cut starting at or above it.
    fn leaf_of_strip(&self, at: StripPos) -> u32 {
        self.cuts
            .iter()
            .rposition(|c| c.image < at.image || (c.image == at.image && c.offset <= at.offset))
            .unwrap_or(0) as u32
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
        // The layers give the mode and direction in order: the person's own choice for the
        // series, its source's word (a webtoon opens as the Webtoons default), the reader's kept
        // guess, then the medium. Where the source said, the book's shape is not guessed from.
        let strip = l.word(Key::Mode) == "strip";
        let vertical = l.word(Key::Mode) == "vertical" && !self.surface.eink;
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
            mode: if strip {
                ReaderMode::Strip
            } else if vertical {
                ReaderMode::Vertical
            } else {
                ReaderMode::Paged
            },
            direction: if l.word(Key::Direction) == "ltr" {
                ReadDirection::LeftToRight
            } else {
                ReadDirection::RightToLeft
            },
            guess_mode: !l.series_has(Key::Mode) && !l.sourced(Key::Mode),
            guess_direction: !l.series_has(Key::Direction) && !l.sourced(Key::Direction),
            fit_width: l.word(Key::Fit) == "width",
        }
    }

    /// Move a kept place read in a split of the chapter `kept_of` pages long to the same point
    /// of this one, `total` pages long once the credit pages `skipped` are left out. Once, as the
    /// chapter is first laid out.
    fn rescale_kept(&mut self, total: u32, skipped: &[u32]) {
        if let Some(of) = self.kept_of.take()
            && let Some((page, strip)) = kept_past_credits(
                self.start.page.unwrap_or(0),
                self.start.strip,
                of,
                (total, skipped),
            )
        {
            self.start.page = Some(page);
            self.start.strip = strip;
            if self.resumed.is_some() {
                self.resumed = Some(page);
            }
        }
    }

    /// A start past the chapter's last page, as a place kept from a longer copy of it, is its
    /// last page: the reader opens there and `Opened` says so.
    fn clamp_start(&mut self, total: u32) {
        let Some(last) = total.checked_sub(1) else { return };
        if self.start.page.is_some_and(|p| p > last) {
            self.start.page = Some(last);
            self.start.strip = None;
            self.resumed = self.resumed.map(|p| p.min(last));
        }
    }

    /// Ask the worker for the open chapter, starting at `page`.
    fn lay_out(&mut self, cx: &Cx<'_>, pages: &Pages, page: u32) {
        self.generation += 1;
        self.follow_download(cx, self.at);
        let chapter = self.chapter().clone();
        self.start.page = Some(page);
        // The place within a strip holds only while the chapter opens at its image.
        self.start.strip = self.start.strip.filter(|s| s.image == page);
        // An open of another chapter, or of this one before it was downloaded, stops.
        if self
            .source_open
            .as_ref()
            .is_some_and(|o| o.chapter != chapter.id || chapter.format != novel::SOURCE)
        {
            self.source_open = None;
        }
        // Another chapter's pages that had not all landed are fetched again when it opens, and
        // so is one with credit pages or parts, which its folder alone does not tell.
        let left = self.feed.take_if(|f| f.chapter != chapter.id);
        // The chapter left stops fetching, so its pages never hold up the one opening.
        if let Some(left) = &left {
            left.leave();
        }
        if let Some(left) = left
            && (!left.complete() || !left.credits.is_empty() || left.parts.is_some())
            && let Some(c) = self.chapters.iter_mut().find(|c| c.id == left.chapter)
        {
            c.format = novel::SOURCE.into();
            c.path = PathBuf::new();
        }
        if novel::prepares(&chapter) {
            // Opened off the actor: a novel goes to the web view, pictures to the worker.
            self.prepare(cx, pages, &chapter);
            return;
        }
        pages.open(OpenJob {
            session: self.id,
            generation: self.generation,
            chapter: chapter.id,
            path: chapter.path,
            plan: self.plan(cx),
            start_page: page,
            shared: self.laid.clone(),
            out: cx.tx.clone(),
            feed: self.feed.clone(),
            // A feed left its credit pages out as it came.
            credits: self.credit_check(cx).filter(|_| self.feed.is_none()),
        });
    }

    /// What a source's chapter learns of the series' credit pages as it is fetched, and whether
    /// it leaves them out.
    fn learning(&self, cx: &Cx<'_>) -> super::credits::Learning {
        super::credits::Learning {
            store: cx.services.store.clone(),
            work: self.work.as_str().to_owned(),
            skip: self.medium == Medium::Manga && self.layers.on(Key::SkipCreditPages),
        }
    }

    /// The credit pages are looked for in a manga's chapters, unless the person turned that off.
    fn credit_check(&self, cx: &Cx<'_>) -> Option<super::credits::CreditCheck> {
        (self.medium == Medium::Manga && self.layers.on(Key::SkipCreditPages)).then(|| {
            super::credits::CreditCheck {
                store: cx.services.store.clone(),
                work: self.work.as_str().to_owned(),
                chapter: self.chapter().id.as_str().to_owned(),
                now: cx.now(),
            }
        })
    }

    /// The open chapter's parts: from its feed, or as its download kept them.
    fn parts_now(
        &self,
        cx: &Cx<'_>,
        credits: &[kaeri_model::reader::CreditPage],
    ) -> Option<super::feed::FeedParts> {
        let chapter = self.chapter();
        if let Some(feed) = &self.feed
            && feed.chapter == chapter.id
        {
            return feed.parts.clone();
        }
        let store = &cx.services.store;
        let (work, id) = (self.work.as_str(), chapter.id.as_str());
        let download = store.book_download(work, id).ok()??;
        // Only the downloaded file itself was cut into these parts.
        if download.path.as_deref().map(Path::new) != Some(chapter.path.as_path()) {
            return None;
        }
        let skipped: Vec<u32> = credits.iter().map(|c| c.page).collect();
        crate::downloads::books::kept_parts(store, work, id).map(|p| p.counted(&skipped))
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
        // A place kept in a split of the chapter with other pages, as another source's, goes to
        // the same point in this one.
        let skipped: Vec<u32> = laid.credits.iter().map(|c| c.page).collect();
        self.rescale_kept(laid.pages, &skipped);
        self.clamp_start(laid.pages);
        let eink = self.eink();
        // A guess is kept for the series on this device, and said once.
        let now = cx.now();
        let store = &cx.services.store;
        let mut said = None;
        // A webtoon's mode is the Webtoons default, read at each open, so it is no guess.
        if laid.guessed_mode {
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
            self.layers =
                cx.layers_with(self.medium, Some(self.work.as_str()), Some(self.source));
        }
        if !self.opened {
            self.opened = true;
            cx.send(ReaderEvent::Opened(Box::new(self.view(cx))));
            self.downloads_follow(cx, false);
        }
        let mut leaves = self.leaves.clone();
        leaves.push(Leaf { index: leaves.len() as u32, parts: Vec::new(), end_page: true });
        cx.send(ReaderEvent::Leaves {
            session: self.id,
            chapter: self.chapter().id.clone(),
            leaves,
        });
        if !laid.credits.is_empty() {
            let count = laid.credits.len() as i64;
            cx.send(ReaderEvent::CreditPages {
                session: self.id,
                chapter: self.chapter().id.clone(),
                pages: laid.credits.clone(),
                msg: Msg::Reading(ReadingMsg::ReaderCreditPagesSkipped { count }),
            });
        }
        if let Some(parts) = self.parts_now(cx, &laid.credits) {
            cx.send(ReaderEvent::ChapterParts {
                session: self.id,
                chapter: self.chapter().id.clone(),
                parts: parts.parts,
                total: parts.total,
                missing: parts.missing,
            });
        }
        if let Some(msg) = said {
            cx.send(notice(msg, Some(ReaderAction::ChangeMode), RESUMED_MS, eink));
        }
        if let Some(page) = self.resumed.take()
            && let Some(msg) = resumed_msg(self.mode, page, self.start.strip)
        {
            cx.send(notice(msg, Some(ReaderAction::StartOver), RESUMED_MS, eink));
        }
        let start = match self.start.strip {
            Some(at) if !self.cuts.is_empty() => self.leaf_of_strip(at),
            _ => self.leaf_of_page(self.start.page.unwrap_or(0)),
        };
        pages.want(self.id, self.generation, vec![start, start + 1], cx.tx);
        // Another device that read further while this one was away moves the reader, as it
        // would had it read while the reader was open.
        if !std::mem::replace(&mut self.moved_checked, true) {
            self.moved_on(cx, pages);
        }
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
                // Scrolling on to another image is a turn.
                if self.page() != Some(at.image) {
                    self.drop_back(cx);
                }
                let mut here = place(&self.work, &chapter, Some(at.image));
                here.strip = Some(at);
                self.place = Some(here);
                self.note_place(cx);
                if foot_visible && at.image + 1 >= self.pages_total {
                    self.wait_to_read(cx);
                }
                // The images around the place, as a turn wants the leaves around it: one
                // behind and three ahead. Images made already are not made again.
                let leaf = self.leaf_of_page(at.image);
                let mut want = vec![leaf, leaf + 1, leaf + 2, leaf + 3];
                if let Some(before) = leaf.checked_sub(1) {
                    want.push(before);
                }
                pages.want(self.id, self.generation, want, cx.tx);
                // Near the foot, the divider names the next chapter and shows its first page;
                // from half way, a chapter from a source is fetched ahead.
                if at.image + 3 >= self.pages_total {
                    self.say_end_page(cx);
                    self.peek(cx, pages);
                } else if (at.image + 1) * 2 >= self.pages_total {
                    self.read_ahead(cx, pages);
                }
            }
            C::Surface { surface, .. } if self.novel.is_some() => {
                self.surface = surface;
                self.restyle(cx);
            }
            C::Surface { surface, .. } => {
                self.surface = surface;
                let page = self.page_now();
                self.lay_out(cx, pages, page);
            }
            C::NeedPage { page, .. } => pages.again(self.id, page, cx.tx),
            C::Retry { page: Some(page), .. } => pages.again(self.id, page, cx.tx),
            C::Retry { page: None, .. } => {
                let page = self.page_now();
                self.lay_out(cx, pages, page);
            }
            C::OpenChapter { chapter, .. } => {
                let Some(index) = self.index_of(cx, &chapter) else {
                    self.not_found(cx);
                    return;
                };
                // Skipping the extras moves on as Continue does; they stay unread. So does the
                // end page's next chapter offline, the next one on this device.
                let offline = cx.services.offline.load(std::sync::atomic::Ordering::Relaxed);
                if index == self.at + 1
                    || Some(index) == self.skip_from(self.at + 1, false)
                    || Some(index) == self.end_next(offline)
                {
                    self.chapter_read(cx, Reached::MovedOn);
                }
                // A strip flows into the next chapter in the same scroll, under its marker:
                // it starts at its first image, never at a place kept from before.
                let flowing = self.mode == ReaderMode::Strip && index == self.at + 1;
                let store = &cx.services.store;
                let page = crate::read_catalog::chapters::read_state(store, self.work.as_str())
                    .1
                    .into_iter()
                    .find(|k| k.chapter == chapter.as_str() && !k.read && !flowing)
                    .and_then(|k| k.last_page)
                    .unwrap_or(0);
                let novel = novel::kept_novel(cx, &self.work, &chapter);
                self.open_chapter(cx, pages, index, page, novel);
            }
            C::BackTo { .. } => {
                if let Some(back) = self.back_to.clone() {
                    self.drop_back(cx);
                    self.go_to(cx, pages, back);
                } else {
                    // An offer already withdrawn: the app hears again that it is gone.
                    cx.send(ReaderEvent::BackToPage { session: self.id, page: None });
                }
            }
            C::StartOver { .. } => {
                let mut start = place(&self.work, &self.chapter().id, Some(0));
                start.novel = self.novel_start();
                self.go_to(cx, pages, start);
            }
            C::NotACreditPage { fingerprint, .. } => {
                let store = &cx.services.store;
                if let Err(e) =
                    store.keep_credit_page(self.work.as_str(), fingerprint, cx.now())
                {
                    tracing::warn!(error = %e, "a page kept from the credit pages was not kept");
                    return;
                }
                if self.novel.is_none() {
                    // A source's chapter is fetched again, the page kept this time.
                    if let Some(feed) = self.feed.take()
                        && let Some(c) = self.chapters.iter_mut().find(|c| c.id == feed.chapter)
                    {
                        c.format = novel::SOURCE.into();
                        c.path = PathBuf::new();
                    }
                    let page = self.page_now();
                    self.lay_out(cx, pages, page);
                }
            }
            C::GoBack { .. } => {
                if let Some(back) = self.moved_back.take() {
                    self.go_to(cx, pages, back);
                }
            }
            C::NovelAt { .. } | C::NovelCounted { .. } | C::FixedRendered { .. } => {
                self.novel_report(cx, command)
            }
            other => tracing::debug!(command = ?other, "a report for another chapter"),
        }
    }

    fn shown(&mut self, cx: &mut Cx<'_>, pages: &Pages, leaf: u32, how: TurnHow) {
        if !matches!(how, TurnHow::Jump | TurnHow::Placed) {
            self.drop_back(cx);
        }
        let story = self.leaves.len() as u32;
        if leaf >= story {
            // The end page, in the flow: read at once.
            self.chapter_read(cx, Reached::MovedOn);
            self.say_end_page(cx);
            return;
        }
        let (page, strip) = self.page_of_leaf(leaf);
        let mut here = place(&self.work, &self.chapter().id, page);
        here.strip = strip;
        if how == TurnHow::Jump {
            self.jumped(cx, &here);
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
        pages.want(self.id, self.generation, want, cx.tx);
        // Near the end, the next chapter's first page is made for the end page; from half way,
        // a chapter from a source is fetched ahead.
        if leaf + 3 >= story {
            self.peek(cx, pages);
        } else if (leaf + 1) * 2 >= story {
            self.read_ahead(cx, pages);
        }
    }

    /// Fetch the next chapter ahead when it comes from a source, once a chapter: from half way
    /// through the open one, so it opens at once from the end page or the strip's foot.
    fn read_ahead(&mut self, cx: &Cx<'_>, pages: &Pages) {
        let index = self.at + 1;
        self.follow_download(cx, index);
        let Some(next) = self.chapters.get(index).filter(|c| c.format == novel::SOURCE) else {
            return;
        };
        if self.ahead.as_ref().is_some_and(|a| a.chapter == next.id) {
            return;
        }
        // The download queue is fetching it, as one kept ready: it opens from that file, and
        // the source is not asked for it twice.
        if downloading(cx, &self.work, &next.id) {
            return;
        }
        let gone = self.gone.clone();
        let learning = self.learning(cx);
        let ahead = novel::fetch_ahead(
            cx,
            pages,
            (self.id, learning),
            &self.work,
            (index, next),
            &gone,
        );
        self.ahead = Some(ahead);
    }

    /// A chapter as the downloads have it now, not as they had it when the session opened: one
    /// downloaded since reads from its file, and one whose file was deleted from its source.
    fn follow_download(&mut self, cx: &Cx<'_>, index: usize) {
        let work = self.work.clone();
        let Some(chapter) = self.chapters.get_mut(index) else { return };
        if chapter.format == novel::SOURCE {
            let Ok(Some(row)) =
                cx.services.store.book_download(work.as_str(), chapter.id.as_str())
            else {
                return;
            };
            let Some(path) = row.path.filter(|p| std::path::Path::new(p).exists()) else {
                return;
            };
            if row.state != kaeri_store::book_downloads::BookDownloadState::Done {
                return;
            }
            let path = PathBuf::from(path);
            chapter.format = path
                .extension()
                .and_then(|e| e.to_str())
                .map_or_else(|| "folder".to_owned(), str::to_ascii_lowercase);
            chapter.path = path;
            chapter.pages = row.pages_total.or(chapter.pages);
            chapter.source = Some(row.source_id);
        } else if chapter.source.is_some()
            && !chapter.path.as_os_str().is_empty()
            && !chapter.path.exists()
            && novel::sourced(cx, &work)
        {
            chapter.format = novel::SOURCE.into();
            chapter.path = PathBuf::new();
            chapter.source = None;
        }
    }

    /// The person jumped to `here`: more than [`BACK_PAST`] pages from the place before, Back to
    /// page offers that place. A second jump keeps the first place.
    pub(super) fn jumped(&mut self, cx: &Cx<'_>, here: &ReadPlace) {
        if self.back_to.is_some() {
            return;
        }
        let Some(before) = self.place.clone() else { return };
        let (Some(from), Some(to)) = (before.page, here.page) else { return };
        if from.abs_diff(to) <= BACK_PAST {
            return;
        }
        cx.send(ReaderEvent::BackToPage { session: self.id, page: Some(from + 1) });
        self.back_to = Some(before);
    }

    /// A turn, a used return or another device's place: Back to page goes.
    pub(super) fn drop_back(&mut self, cx: &Cx<'_>) {
        if self.back_to.take().is_some() {
            cx.send(ReaderEvent::BackToPage { session: self.id, page: None });
        }
    }

    /// Ask the worker for the next chapter's first page, once a chapter. A downloaded chapter's
    /// is made from its file. A chapter from a source is fetched ahead, and that page is made as
    /// it lands; one the download queue has is followed, and its first page made from the
    /// download as it lands.
    fn peek(&mut self, cx: &Cx<'_>, pages: &Pages) {
        if std::mem::replace(&mut self.peeked, true) {
            return;
        }
        let offline = cx.services.offline.load(std::sync::atomic::Ordering::Relaxed);
        let Some(index) = self.end_next(offline) else { return };
        self.follow_download(cx, index);
        let Some(next) = self.chapters.get(index) else { return };
        if next.format == novel::SOURCE {
            let fetching = self.ahead.as_ref().is_some_and(|a| a.chapter == next.id);
            if !fetching && downloading(cx, &self.work, &next.id) {
                let first = novel::follow_first_page(
                    cx,
                    pages,
                    self.id,
                    &self.work,
                    (index, next),
                    &self.gone,
                );
                self.first_page = Some(first);
            } else {
                // Its first page comes with the chapter fetched ahead.
                self.read_ahead(cx, pages);
            }
            return;
        }
        if novel::prepares(next) {
            return;
        }
        if next.path.as_os_str().is_empty() {
            return;
        }
        pages.peek(PeekJob {
            session: self.id,
            chapter: next.id.clone(),
            path: next.path.clone(),
            out: cx.tx.clone(),
        });
    }

    fn say_end_page(&mut self, cx: &Cx<'_>) {
        if std::mem::replace(&mut self.end_page_said, true) {
            return;
        }
        let marks =
            crate::read_catalog::chapters::read_marks(&cx.services.store, self.work.as_str());
        let offline = cx.services.offline.load(std::sync::atomic::Ordering::Relaxed);
        let index = self.end_next(offline);
        let following = index.map(|i| &self.chapters[i]);
        let next = following.map(|c| end_row(c, &marks));
        let next_source = following
            .and_then(|c| c.source.as_deref())
            .map(|id| cx.services.book_sources.source_name(id));
        let why_none = next.is_none().then(|| self.why_no_next(cx));
        let skip = index.and_then(|i| self.skip_from(i, offline).map(|s| (i, s)));
        let skip_to = skip.map(|(_, s)| end_row(&self.chapters[s], &marks));
        let extras = skip.map_or(0, |(i, s)| s - i) as u32;
        cx.send(ReaderEvent::EndPage {
            session: self.id,
            next,
            why_none,
            next_source,
            skip_to,
            extras,
        });
    }

    /// Said again with the connection as it is now, once the end page was said with it the
    /// other way. Its next chapter's first page is asked for again when that chapter changed.
    fn network_moved(&mut self, cx: &Cx<'_>, pages: &Pages, was_offline: bool) {
        let offline = cx.services.offline.load(std::sync::atomic::Ordering::Relaxed);
        if !self.end_page_said || offline == was_offline {
            return;
        }
        let before = self.end_next(was_offline);
        self.end_page_said = false;
        self.say_end_page(cx);
        if self.peeked && before != self.end_next(offline) {
            self.peeked = false;
            self.peek(cx, pages);
        }
    }

    /// The chapter the end page leads into: the one after the open chapter, and offline the
    /// next one on this device, since one only a source has would not open.
    fn end_next(&self, offline: bool) -> Option<usize> {
        (self.at + 1..self.chapters.len()).find(|&i| !offline || on_device(&self.chapters[i]))
    }

    /// Where the end page's skip goes from its next chapter, `next`: past the extras from
    /// there, to the next whole chapter. Offline, only to one on this device.
    fn skip_from(&self, next: usize, offline: bool) -> Option<usize> {
        let list: Vec<(ChapterId, Option<String>)> =
            self.chapters.iter().map(|c| (c.id.clone(), c.title.clone())).collect();
        crate::read_catalog::extras::skip_to(&list, next)
            .filter(|&i| !offline || on_device(&self.chapters[i]))
    }

    /// Why the end page has no next chapter: the series ended with this one, which offers
    /// Read again; the device is offline; or the person is caught up.
    fn why_no_next(&self, cx: &Cx<'_>) -> NoNext {
        let store = &cx.services.store;
        let later = crate::read_catalog::chapters::of_work(
            &cx.services.book_sources,
            store,
            &self.work,
        )
        .is_some_and(|(_, _, list)| {
            let at = list.iter().position(|c| c.id == self.chapter().id.as_str());
            at.is_some_and(|at| at + 1 < list.len())
        });
        if self.ended && !later {
            NoNext::Finished
        } else if cx.services.offline.load(std::sync::atomic::Ordering::Relaxed) {
            NoNext::Offline
        } else {
            NoNext::CaughtUp
        }
    }

    fn wait_to_read(&mut self, cx: &Cx<'_>) {
        if !self.read_fired && counts(cx, Reached::Stayed) {
            after(cx.tx, READ_AFTER, self.id, ReaderWake::ReadRule(self.read_timer));
        }
    }

    /// The read rule fired: a finished read event, the trackers queued, `ChapterRead`. Nothing,
    /// when the person's Counts as read takes no account of how the last page was `reached`.
    pub(super) fn chapter_read(&mut self, cx: &Cx<'_>, reached: Reached) {
        if self.read_fired || !counts(cx, reached) {
            return;
        }
        self.read_fired = true;
        let chapter = self.chapter().id.clone();
        let at = cx.now();
        let mut event = ReadEvent::new(self.work.as_str(), chapter.as_str(), at);
        event.completed = true;
        event.page = self.pages_total.checked_sub(1);
        event.of_pages = Some(self.pages_total).filter(|n| *n > 0);
        event.source_id = Some("local".into());
        let send_now = matches!(
            cx.config.effective().reading.send_progress_after,
            SendProgressAfter::Chapter
        );
        // The read is kept before the trackers hear of it, so what they are told counts it:
        // the chapter that finishes a volume names that volume.
        let told = match trackers_of(cx, &self.work) {
            Some((_, trackers)) if send_now => trackers,
            _ => Vec::new(),
        };
        if let Err(e) = cx.services.store.record_read(&event, told) {
            tracing::warn!(error = %e, "a read chapter was not kept");
        }
        let queued_for = if send_now {
            queue_trackers(cx, &self.work, &chapter, at)
        } else {
            self.unsent = Some((chapter.clone(), at));
            Vec::new()
        };
        let trackers = queued_for.iter().map(|id| crate::tracking::tracker_name(id)).collect();
        // A book server the person lets Kaeri write to hears of it now, as a mark from the
        // series page does: it is their own library, not a tracker.
        cx.services.book_sources.report_read(&self.work, &chapter, at, cx.tx);
        self.dirty = false;
        let _ = cx.tx.send(Event::ProgressQueued);
        cx.send(ReaderEvent::ChapterRead { work: self.work.clone(), chapter, trackers });
        self.downloads_follow(cx, true);
    }

    /// The chapter was opened, or counts as read: downloads keep the next ones ready and let go
    /// of what is read.
    pub(super) fn downloads_follow(&self, cx: &Cx<'_>, finished: bool) {
        let lent = crate::downloads::books::Lent::of(cx.services, cx.tx);
        let id = self.chapter().id.clone();
        cx.services.book_sources.downloads.read_moved(&lent, &self.work, &id, finished);
    }

    fn note_place(&mut self, cx: &Cx<'_>) {
        self.dirty = true;
        // A strip scrolled within the image last written is written once it leaves: on a new
        // image or chapter, the close, or the app leaving the foreground.
        if let Some(here) = &self.place
            && here.strip.is_some()
            && self.written.as_ref().is_some_and(|(c, p)| *c == here.chapter && *p == here.page)
        {
            return;
        }
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
        self.written = Some((here.chapter.clone(), here.page));
        self.write_place(cx, &here, self.pages_total);
    }

    /// Write the place as the chapter is left. When credit pages of the chapter were learnt while
    /// it was open, the place is written as the chapter counts its pages once they are left out,
    /// so the Continue row and the next open agree.
    fn flush_leaving(&mut self, cx: &Cx<'_>) {
        let Some(past) = self.place_past_new_credits(cx) else {
            self.flush(cx);
            return;
        };
        if !self.dirty && self.written_past_credits.as_ref() == Some(&past) {
            return;
        }
        self.dirty = false;
        self.last_write = Some(Instant::now());
        self.written = self.place.as_ref().map(|p| (p.chapter.clone(), p.page));
        self.write_place(cx, &past.0, past.1);
        self.written_past_credits = Some(past);
    }

    /// The place and the chapter's count once the credit pages learnt since it was laid out
    /// leave it, as from the next chapter fetched ahead. The page read stays the same: a place
    /// on a page now left out goes to the one before it. `None` when nothing was learnt.
    fn place_past_new_credits(&self, cx: &Cx<'_>) -> Option<(ReadPlace, u32)> {
        let here = self.place.clone().filter(|p| p.novel.is_none() && !self.read_fired)?;
        let feed = self.feed.as_ref().filter(|f| f.chapter == here.chapter)?;
        let page = here.page?;
        let left: Vec<u32> = feed.credits.iter().map(|c| c.page).collect();
        let total = u32::try_from(feed.count() + left.len()).ok()?;
        let found = self.learning(cx).find(here.chapter.as_str(), total);
        let new: Vec<u32> =
            found.iter().map(|p| p.page).filter(|p| !left.contains(p)).collect();
        if new.is_empty() {
            return None;
        }
        // Pages by where they are in the source's chapter, as counted now and as they will be.
        let order: Vec<u32> = (0..total).filter(|p| !left.contains(p)).collect();
        let kept: Vec<u32> = order.iter().copied().filter(|p| !new.contains(p)).collect();
        let moved = moved_to(&order, &kept, page)?;
        let mut place = here;
        place.page = Some(moved);
        place.strip = place.strip.map(|s| StripPos { image: moved, ..s });
        Some((place, u32::try_from(kept.len()).ok()?))
    }

    fn write_place(&self, cx: &Cx<'_>, here: &ReadPlace, of: u32) {
        let mut event = ReadEvent::new(self.work.as_str(), here.chapter.as_str(), cx.now());
        event.page = here.page;
        event.of_pages = Some(of).filter(|n| *n > 0);
        event.strip_image = here.strip.map(|s| s.image);
        event.strip_offset = here.strip.map(|s| f64::from(s.offset));
        event.novel_pos = here.novel.as_ref().map(NovelPos::encode);
        event.source_id = Some("local".into());
        if let Err(e) = cx.services.store.record_read(&event, Vec::new()) {
            tracing::warn!(error = %e, "the reading place was not kept");
        }
    }

    fn open_chapter(
        &mut self,
        cx: &mut Cx<'_>,
        pages: &Pages,
        index: usize,
        page: u32,
        novel: Option<NovelPos>,
    ) {
        self.flush_leaving(cx);
        if self.novel.take().is_some() {
            cx.services.novels.remove(self.id);
        }
        self.restore = novel;
        self.novel_resumed = false;
        self.at = index;
        self.opened = false;
        self.read_fired = false;
        self.end_page_said = false;
        self.peeked = false;
        self.back_to = None;
        self.place = None;
        self.kept_of = None;
        // A chapter fetched ahead and not the one opening stops fetching.
        self.ahead = self.ahead.take().filter(|a| a.chapter == self.chapters[index].id);
        self.first_page = None;
        self.start = place(&self.work, &self.chapters[index].id, Some(page));
        self.lay_out(cx, pages, page);
    }

    /// Show `to`: in the open chapter with `GoTo`, else by opening its chapter.
    fn go_to(&mut self, cx: &mut Cx<'_>, pages: &Pages, to: ReadPlace) {
        if to.chapter == self.chapter().id {
            cx.send(ReaderEvent::GoTo { session: self.id, place: to });
        } else if let Some(index) = self.index_of(cx, &to.chapter) {
            self.open_chapter(cx, pages, index, to.page.unwrap_or(0), to.novel.clone());
        } else {
            self.not_found(cx);
        }
    }

    /// Where `chapter` is in the session's list. One the list does not name is looked for in
    /// the work's chapters as this device has them now: a list loaded since the session
    /// opened, such as the series page's fresh one, can name a chapter the session's own does
    /// not. That list becomes the session's when it names both `chapter` and the open one.
    fn index_of(&mut self, cx: &Cx<'_>, chapter: &ChapterId) -> Option<usize> {
        if let Some(index) = self.chapters.iter().position(|c| c.id == *chapter) {
            return Some(index);
        }
        let mut now = shelf_of(cx, &self.work);
        if novel::sourced(cx, &self.work)
            && let Some(listed) = novel::kept_list(cx, &self.work, &now)
        {
            now = listed;
        }
        let index = now.iter().position(|c| c.id == *chapter)?;
        let at = now.iter().position(|c| c.id == self.chapter().id)?;
        self.chapters = now;
        self.at = at;
        Some(index)
    }

    /// A chapter asked for is in no list this device has: the reader says so rather than
    /// waiting on it.
    fn not_found(&self, cx: &Cx<'_>) {
        let _ = cx.tx.send(Event::Reading(ReadingEvent::Failed {
            about: ReadingAbout::Reader,
            failure: ReadingFailure::NotFound,
        }));
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
        // This device's own id stays its own with sync off: places it kept while sync was on
        // carry it.
        if store.sync_device_kept().ok().flatten().as_deref() == Some(device.as_str()) {
            return;
        }
        let Some(index) = self.chapters.iter().position(|c| c.id.as_str() == last) else {
            return;
        };
        let mine = self.page().unwrap_or(0);
        let page = theirs.last_page.unwrap_or(0);
        let their_pos = theirs.novel_pos.as_deref().and_then(NovelPos::decode);
        let my_pos = self.place.as_ref().and_then(|p| p.novel.clone());
        let further_here = match (&their_pos, &my_pos) {
            // Novels compare by the text position: page numbers differ between screens.
            (Some(theirs), Some(mine)) => theirs > mine,
            (Some(_), None) if self.novel.is_some() => true,
            _ => page > mine,
        };
        let further = index > self.at || (index == self.at && further_here);
        if theirs.read || !further {
            return;
        }
        // A device whose name is not known (removed from sync, or this one before a reinstall)
        // is never named by its id.
        let name = store
            .sync_peers()
            .ok()
            .and_then(|peers| peers.into_iter().find(|p| p.0 == device).and_then(|p| p.1))
            .filter(|name| !name.trim().is_empty());
        let back = self
            .place
            .clone()
            .unwrap_or_else(|| place(&self.work, &self.chapter().id, Some(mine)));
        let mut to = place(&self.work, &self.chapters[index].id, Some(page));
        to.novel = their_pos;
        let label = self.chapters[index].label.clone();
        self.moved_back = Some(back.clone());
        self.drop_back(cx);
        self.go_to(cx, pages, to.clone());
        let from_device = name.clone().unwrap_or_default();
        cx.send(ReaderEvent::Moved { session: self.id, to, from_device, back });
        let msg = moved_msg(self.mode, &label, i64::from(page) + 1, name);
        cx.send(notice(msg, Some(ReaderAction::GoBack), MOVED_MS, self.eink()));
    }
}

/// "Resumed at page 8", or at an image in a strip. Nothing at the chapter's very start, where
/// Start over would have nothing to do.
/// A place at `page` (and in a strip, how far into its image) of a chapter `of` pages long, at
/// the same point of the chapter in `total` pages. `None` when the two are the same length, or
/// either is unknown.
pub(super) fn rescaled(
    page: u32,
    strip: Option<StripPos>,
    of: u32,
    total: u32,
) -> Option<(u32, Option<StripPos>)> {
    if of == 0 || total == 0 || of == total {
        return None;
    }
    let into = strip.filter(|s| s.image == page).map_or(0.0, |s| f64::from(s.offset));
    let at = (f64::from(page) + into) / f64::from(of) * f64::from(total);
    let image = (at.floor() as u32).min(total - 1);
    let offset = (at - f64::from(image)).clamp(0.0, 1.0) as f32;
    Some((image, strip.map(|_| StripPos { image, offset })))
}

/// As [`rescaled`], for a chapter that now leaves out the credit pages `skipped` (by where they
/// are among every page the source has). A place counted with every page moves past the credit
/// pages before it, so the page read stays the same; a place counted otherwise is scaled.
pub(super) fn kept_past_credits(
    page: u32,
    strip: Option<StripPos>,
    of: u32,
    (total, skipped): (u32, &[u32]),
) -> Option<(u32, Option<StripPos>)> {
    if skipped.is_empty() || of as usize != total as usize + skipped.len() {
        return rescaled(page, strip, of, total);
    }
    let every: Vec<u32> = (0..of).collect();
    let kept: Vec<u32> = every.iter().copied().filter(|p| !skipped.contains(p)).collect();
    let image = moved_to(&every, &kept, page)?;
    Some((image, strip.filter(|s| s.image == page).map(|s| StripPos { image, ..s })))
}

/// Page `page` of a chapter whose pages are `before` (each by where it is among every page the
/// source has), in the same chapter with the pages `after`: the same page, or on one left out,
/// the one before it.
fn moved_to(before: &[u32], after: &[u32], page: u32) -> Option<u32> {
    let at = *before.get(page as usize)?;
    let moved = after.iter().rposition(|p| *p <= at).unwrap_or(0);
    u32::try_from(moved).ok()
}

fn resumed_msg(mode: ReaderMode, page: u32, strip: Option<StripPos>) -> Option<ReadingMsg> {
    let into = strip.filter(|s| s.image == page).map_or(0.0, |s| s.offset);
    if page == 0 && into < 0.05 {
        return None;
    }
    Some(if mode == ReaderMode::Strip {
        ReadingMsg::ReaderResumedImage { image: i64::from(page) + 1 }
    } else {
        ReadingMsg::ReaderResumed { page: i64::from(page) + 1 }
    })
}

/// The Moved notice for a chapter's label. A label that is only a number, with or without a
/// chapter word (`14`, `14.5`, `Chapter 14`), reads "chapter 14"; any other label is shown whole.
/// The Moved notice for a move to `page` of the chapter `label`: in images in a strip, as every
/// other line there counts.
fn moved_msg(mode: ReaderMode, label: &str, page: i64, device: Option<String>) -> ReadingMsg {
    let lower = label.trim().to_lowercase();
    let rest = ["chapter", "chap.", "chap", "ch.", "ch"]
        .iter()
        .find_map(|w| lower.strip_prefix(w))
        .map_or(lower.as_str(), |r| r.trim_start_matches([' ', '.', ':', '#']));
    let number = kaeri_core::books::ChapterNumber::parse(rest).map(|n| n.to_string());
    let strip = mode == ReaderMode::Strip;
    let (image, label) = (page, label.to_owned());
    match (number, device, strip) {
        (Some(chapter), Some(device), false) => {
            ReadingMsg::ReaderMovedNumber { chapter, page, device }
        }
        (Some(chapter), Some(device), true) => {
            ReadingMsg::ReaderMovedNumberImage { chapter, image, device }
        }
        (Some(chapter), None, false) => {
            ReadingMsg::ReaderMovedNumberElsewhere { chapter, page }
        }
        (Some(chapter), None, true) => {
            ReadingMsg::ReaderMovedNumberElsewhereImage { chapter, image }
        }
        (None, Some(device), false) => ReadingMsg::ReaderMoved { chapter: label, page, device },
        (None, Some(device), true) => {
            ReadingMsg::ReaderMovedImage { chapter: label, image, device }
        }
        (None, None, false) => ReadingMsg::ReaderMovedElsewhere { chapter: label, page },
        (None, None, true) => ReadingMsg::ReaderMovedElsewhereImage { chapter: label, image },
    }
}

/// How the reader came to the end of a chapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Reached {
    /// The last page stayed on screen for [`READ_AFTER`].
    Stayed,
    /// The person moved past the last page: to the end page, or to the next chapter.
    MovedOn,
}

/// Whether the chapter counts as read, by the person's Counts as read, when its last page was
/// `reached` so. Their own marks count under every rule; the reader's never under `marked`.
pub(super) fn counts(cx: &Cx<'_>, reached: Reached) -> bool {
    match cx.config.effective().reading.counts_as_read {
        CountsAsRead::LastPage => true,
        CountsAsRead::MovingOn => reached == Reached::MovedOn,
        CountsAsRead::Marked => false,
    }
}

/// The work's AniList id and the signed-in reading trackers a read of it goes to. `None` for a
/// work AniList does not know, and for the demo: it is read to try the reader, never told to a
/// tracker, even matched to a work.
fn trackers_of(cx: &Cx<'_>, work: &WorkId) -> Option<(AnilistId, Vec<String>)> {
    let demo = cx
        .services
        .book_sources
        .edition_of(work, None)
        .is_some_and(|(source, _)| source == kaeri_core::demo::DEMO_SOURCE_ID);
    if demo {
        return None;
    }
    let id = work.as_str().strip_prefix("a:").and_then(|n| n.parse::<u32>().ok())?;
    let trackers = crate::tracking::commands::signed_in_reading_tracker_ids(&cx.services.sync);
    Some((AnilistId::new(id), trackers))
}

/// Queue the chapter for the trackers signed in, for a work AniList lists: the highest whole
/// chapter, never lower than a tracker holds, with the volume the progress kept names. The
/// read of `chapter` is kept first. Says which trackers it went to.
fn queue_trackers(cx: &Cx<'_>, work: &WorkId, chapter: &ChapterId, at: i64) -> Vec<String> {
    let Some((id, trackers)) = trackers_of(cx, work) else { return Vec::new() };
    let number = chapter.key().and_then(|k| k.range()).map(|r| r.last().whole());
    // The highest whole chapter read, and the volume a watermark named: half chapters go as
    // the chapter below them, and a tracker never holds less than it was told.
    let (read, volume) = crate::read_tracking::progress_of(cx.services, work);
    // A chapter with no number, as a whole volume read as one book, tells what the progress
    // kept names: the volumes finished and the chapters counted. With neither there is nothing.
    if number.is_none() && read == 0 && volume.is_none() {
        return Vec::new();
    }
    let chapter = number.unwrap_or(0).max(read);
    let op = TrackOp::SetReadProgress { anilist_id: id, chapter, volume };
    trackers
        .into_iter()
        .filter(|tracker| cx.services.store.enqueue(tracker, &op, at).is_ok())
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn moved_notice_names_the_chapter_once() {
        let moved = |label: &str| moved_msg(ReaderMode::Paged, label, 6, Some("Desk".into()));
        let number = |n: &str| ReadingMsg::ReaderMovedNumber {
            chapter: n.into(),
            page: 6,
            device: "Desk".into(),
        };
        assert_eq!(moved("Chapter 14"), number("14"));
        assert_eq!(moved("14.5"), number("14.5"));
        assert_eq!(moved("Ch. 3"), number("3"));
        assert_eq!(
            moved("The Long Night"),
            ReadingMsg::ReaderMoved {
                chapter: "The Long Night".into(),
                page: 6,
                device: "Desk".into()
            }
        );
        assert!(matches!(moved("Chapter 14: The Night"), ReadingMsg::ReaderMoved { .. }));
    }

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

    mod answers;
    mod back_to;
    mod cross_parts;
    mod eink_strip;
    mod extras;
    mod groups;
    mod hints;
    mod leaving;
    mod novels;
    mod peek;
    mod sources;
    mod trackers;

    const WORK: &str = "a:4242";

    /// A comic archive of `pages` portrait pages, each a flat grey of its own.
    fn cbz(path: &Path, pages: u8) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for i in 0..pages {
            let page =
                image::GrayImage::from_pixel(300, 450, image::Luma([40 + (i % 10) * 20]));
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
        /// Whether a failed page or open is expected, and kept instead of a panic.
        failures: bool,
        /// The opens that failed, when `failures` is on.
        failed: Vec<ReadingFailure>,
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
        harness_of(store, "manga", two_chapters)
    }

    /// Two chapters of three pages in a folder.
    fn two_chapters(books: &Path) -> Vec<ShelfFile> {
        {
            (1..=2)
                .map(|n| {
                    let path = books.join(format!("Chapter {n}.cbz"));
                    cbz(&path, 3);
                    ShelfFile {
                        path: path.display().to_string(),
                        chapter: None,
                        label: format!("Chapter {n}"),
                        volume: None,
                        number_milli: Some(n * 1000),
                        format: "cbz".into(),
                        pages: Some(3),
                        comicinfo: None,
                        source: None,
                    }
                })
                .collect()
        }
    }

    /// A reader over a work of `medium` whose folder `files` fills.
    fn harness_of(
        store: Store,
        medium: &str,
        files: impl FnOnce(&Path) -> Vec<ShelfFile>,
    ) -> Harness {
        harness_on(store, medium, files, None)
    }

    /// As [`harness_of`], on a host that has `decoder` for the formats the engine cannot read.
    fn harness_on(
        store: Store,
        medium: &str,
        files: impl FnOnce(&Path) -> Vec<ShelfFile>,
        decoder: Option<Arc<dyn crate::host::PageDecoder>>,
    ) -> Harness {
        harness_signed(store, medium, files, decoder, false)
    }

    /// As [`harness_on`], signed in to AniList when `signed_in`, so a read chapter queues
    /// progress for it. Nothing is sent: the client is offline.
    fn harness_signed(
        store: Store,
        medium: &str,
        files: impl FnOnce(&Path) -> Vec<ShelfFile>,
        decoder: Option<Arc<dyn crate::host::PageDecoder>>,
        signed_in: bool,
    ) -> Harness {
        let dir = tempfile::tempdir().unwrap();
        if signed_in {
            let tokens = kaeri_track::TokenStore::with_backend(
                dir.path().join("tokens"),
                kaeri_track::secret::Backend::File,
            );
            tokens.set("anilist", "token").unwrap();
        }
        let books = dir.path().join("books");
        std::fs::create_dir_all(&books).unwrap();
        let series = LocalSeries {
            id: "s1".into(),
            folder: books.display().to_string(),
            name: "The Lantern Keeper".into(),
            medium: medium.into(),
            work: Some(WORK.into()),
            state: "matched".into(),
        };
        store.put_local_series(&series).unwrap();
        for file in files(&books) {
            store.put_local_file("s1", &file, 1, 1).unwrap();
        }
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let mut config = Config::default();
        if signed_in {
            config.trackers.enabled.push("anilist".into());
        }
        let http = HttpClient::offline(&config.network).unwrap();
        let mut host = QuietHost::in_dir(&dir.path().join("tokens"));
        if let Some(decoder) = decoder {
            host = host.decoding(decoder);
        }
        let host = Arc::new(host);
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
            failures: false,
            failed: Vec::new(),
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

        /// The device goes off line or comes back, as `NetworkPath` tells the reader.
        fn network(&mut self, offline: bool) {
            let was = self.services.offline.swap(offline, std::sync::atomic::Ordering::Relaxed);
            let mut cx = self.reader.cx(&mut self.config, &self.services, &self.tx);
            self.reader.network_moved(&mut cx, was);
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
                        ReaderEvent::PageFailed { failure, .. } if !self.failures => {
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
                    assert!(self.failures, "the reader failed: {failure:?}");
                    self.failed.push(failure);
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
            self.open_work(WorkId::new(WORK), at).await
        }

        /// As [`Self::open`], for another work than [`WORK`].
        async fn open_work(
            &mut self,
            work: WorkId,
            at: OpenAt,
        ) -> (ReaderView, Vec<Leaf>, Vec<ReaderEvent>) {
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

    /// What Back to page offered and took away, in order: `Some(page)` offered, `None` gone.
    fn offers(events: &[ReaderEvent]) -> Vec<Option<u32>> {
        events
            .iter()
            .filter_map(|e| match e {
                ReaderEvent::BackToPage { page, .. } => Some(*page),
                _ => None,
            })
            .collect()
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

    #[tokio::test]
    async fn a_screen_turned_before_the_first_leaf_showed_keeps_the_start_page() {
        let mut h = harness(Store::open_in_memory().unwrap());
        h.open(OpenAt::Page { page: 2 }).await;
        // The phone turns while the reader opens: the new shape comes before any leaf showed.
        let session = h.session();
        let surface = ReaderSurface { width_px: 900, height_px: 600, ..surface() };
        h.send(ReaderCommand::Surface { session, surface });
        assert_eq!(h.reader.session.as_ref().unwrap().start.page, Some(2));
    }

    /// The engine lends each piece of work a sender and holds none between them, so the page
    /// worker must keep the one it was lent while it has pages to make.
    #[tokio::test]
    async fn the_page_worker_answers_when_nothing_else_holds_the_outbox() {
        use std::time::Duration;
        use tokio::time::timeout;

        let mut h = harness(Store::open_in_memory().unwrap());
        let (tx, later) = Outbox::inbox();
        let first = std::mem::replace(&mut h.inbox, later);
        h.send(ReaderCommand::Open {
            work: WorkId::new(WORK),
            chapter: None,
            at: OpenAt::Resume,
            surface: surface(),
        });
        // The command's sender is gone, as when the actor finishes handling it; only the
        // worker still holds the first channel.
        drop(std::mem::replace(&mut h.tx, tx));
        let mut first = first;
        while let Some(envelope) = timeout(Duration::from_secs(20), first.recv())
            .await
            .expect("the first channel closes")
        {
            h.carry(envelope);
        }
        timeout(
            Duration::from_secs(20),
            h.until(|e| matches!(e, ReaderEvent::PageReady { .. }).then_some(())),
        )
        .await
        .expect("a page was made");
        // With the harness's sender let go too, the channel closes once the worker is done.
        let (spare, _unused) = Outbox::inbox();
        drop(std::mem::replace(&mut h.tx, spare));
        while let Some(envelope) = timeout(Duration::from_secs(20), h.inbox.recv())
            .await
            .expect("the engine goes idle")
        {
            h.carry(envelope);
        }
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

    #[tokio::test]
    async fn progress_set_to_go_on_close_waits_for_the_reader_to_close() {
        let store = Store::open_in_memory().unwrap();
        let mut h = harness_signed(store, "manga", two_chapters, None, true);
        h.config.effective_mut().reading.send_progress_after = SendProgressAfter::Close;
        assert_eq!(
            crate::tracking::commands::signed_in_reading_tracker_ids(&h.services.sync),
            ["anilist"]
        );
        let queued = |h: &Harness| h.services.store.read_outbox("anilist").unwrap();
        h.open(OpenAt::Resume).await;
        h.shown(2, TurnHow::Turn).await;
        h.until(|e| matches!(e, ReaderEvent::ChapterRead { .. }).then_some(())).await;
        assert!(h.kept("1").unwrap().read);
        assert!(queued(&h).is_empty(), "read here at once, told to the trackers on close");

        let session = h.session();
        h.send(ReaderCommand::Close { session });
        let ops = queued(&h);
        assert!(
            matches!(ops.as_slice(), [TrackOp::SetReadProgress { chapter: 1, .. }]),
            "{ops:?}"
        );

        // The default tells them as each chapter is read.
        h.config.effective_mut().reading.send_progress_after = SendProgressAfter::Chapter;
        h.open(OpenAt::Resume).await;
        h.shown(2, TurnHow::Turn).await;
        h.until(|e| matches!(e, ReaderEvent::ChapterRead { .. }).then_some(())).await;
        let ops = queued(&h);
        assert!(
            ops.iter().any(|op| matches!(op, TrackOp::SetReadProgress { chapter: 2, .. })),
            "chapter 2 queued before the close: {ops:?}"
        );
    }

    #[tokio::test]
    async fn a_webtoon_opens_as_the_webtoons_default_and_its_last_chapter_offers_read_again() {
        let store = Store::open_in_memory().unwrap();
        store
            .seed_book_work(WORK, "manga", Some("webtoon"), Some("JP"), "The Lantern Keeper", 1)
            .unwrap();
        let mut h = harness(store);
        let (view, _, said) = h.open(OpenAt::Resume).await;
        assert_eq!(view.mode, ReaderMode::Strip, "Long strip is the default");
        assert!(notices(&said).is_empty(), "the person's default, nothing detected");
        let session = h.session();
        h.send(ReaderCommand::Close { session });

        // Paged for webtoons, and the series opens paged: the default is read at each open.
        h.config.effective_mut().reading.webtoon.mode = kaeri_core::config::WebtoonMode::Paged;
        let (view, _, _) = h.open(OpenAt::Resume).await;
        assert_eq!(view.mode, ReaderMode::Paged);
        let session = h.session();
        h.send(ReaderCommand::Close { session });

        // A series' own mode wins over the default.
        h.services.store.set_series_setting(WORK, "mode", Some("strip"), true, 2).unwrap();
        let (view, _, _) = h.open(OpenAt::Resume).await;
        assert_eq!(view.mode, ReaderMode::Strip);
        h.services.store.set_series_setting(WORK, "mode", None, true, 3).unwrap();

        // Still releasing, the last chapter's end page says caught up; finished, it offers
        // Read again.
        let end_of_last = async |h: &mut Harness| {
            let session = h.session();
            h.send(ReaderCommand::Close { session });
            let work = WorkId::new(WORK);
            let chapter = Some(ChapterId::new("2"));
            let at = OpenAt::Resume;
            h.send(ReaderCommand::Open { work, chapter, at, surface: surface() });
            h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;
            let session = h.session();
            let chapter = ChapterId::new("2");
            h.send(ReaderCommand::Shown { session, chapter, leaf: 2, how: TurnHow::Turn });
            h.until(|e| match e {
                ReaderEvent::EndPage { next: None, why_none, .. } => Some(*why_none),
                _ => None,
            })
            .await
        };
        assert_eq!(end_of_last(&mut h).await, Some(NoNext::CaughtUp));
        h.services
            .store
            .put_book_work(&kaeri_store::book_catalog::BookWork {
                work: WORK.into(),
                medium: "manga".into(),
                format: Some("webtoon".into()),
                titles: "[\"The Lantern Keeper\"]".into(),
                status: Some("FINISHED".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(end_of_last(&mut h).await, Some(NoNext::Finished));
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
    async fn moving_on_counts_a_chapter_read_only_past_its_last_page() {
        let mut h = harness(Store::open_in_memory().unwrap());
        h.config.effective_mut().reading.counts_as_read = CountsAsRead::MovingOn;
        let read_now = |said: &[ReaderEvent]| {
            said.iter().find_map(|e| match e {
                ReaderEvent::ChapterRead { chapter, .. } => Some(chapter.as_str().to_owned()),
                _ => None,
            })
        };

        // Opening the next chapter counts the one before it.
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
        h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;

        // Holding the last page is not enough; turning past it to the end page is.
        h.shown(2, TurnHow::Turn).await;
        tokio::time::sleep(READ_AFTER * 2).await;
        assert_eq!(read_now(&h.drain()), None);
        assert!(h.kept("2").is_none_or(|c| !c.read), "the last page held is not enough");
        h.shown(3, TurnHow::Turn).await;
        assert_eq!(read_now(&h.drain()).as_deref(), Some("2"), "at once");
        assert!(h.kept("2").unwrap().read);
    }

    #[tokio::test(start_paused = true)]
    async fn marked_leaves_reading_to_the_persons_marks_and_continue_and_trackers_follow() {
        let store = Store::open_in_memory().unwrap();
        let mut h = harness_signed(store, "manga", two_chapters, None, true);
        h.config.effective_mut().reading.counts_as_read = CountsAsRead::Marked;
        let queued = |h: &Harness| h.services.store.read_outbox("anilist").unwrap();
        let any_read = |said: &[ReaderEvent]| {
            said.iter().any(|e| matches!(e, ReaderEvent::ChapterRead { .. }))
        };

        // Neither the last page held nor the end page reads the chapter.
        h.open(OpenAt::Resume).await;
        h.shown(2, TurnHow::Turn).await;
        tokio::time::sleep(READ_AFTER * 2).await;
        h.shown(3, TurnHow::Turn).await;
        tokio::time::sleep(READ_AFTER * 2).await;
        assert!(!any_read(&h.drain()));
        assert!(h.kept("1").is_none_or(|c| !c.read), "the reader marks nothing");
        assert!(queued(&h).is_empty(), "nor tells the trackers");

        // Continue follows the marks: opened again, reading goes on in the unmarked chapter.
        let session = h.session();
        h.send(ReaderCommand::Close { session });
        let (view, _, _) = h.open(OpenAt::Resume).await;
        assert_eq!(view.chapter.id.as_str(), "1");

        // Nor does opening the next chapter.
        let session = h.session();
        h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
        h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;
        assert!(!any_read(&h.drain()));
        assert!(h.kept("1").is_none_or(|c| !c.read));
        let session = h.session();
        h.send(ReaderCommand::Close { session });

        // The person's own mark counts, and what the trackers hold follows it: the next sync
        // sends the work's tracked progress.
        let tracked = |h: &Harness| h.services.store.tracked_progress(WORK).unwrap().0;
        assert_eq!(tracked(&h), 0);
        super::super::mark_read(
            &h.services,
            WorkId::new(WORK),
            vec![ChapterId::new("1")],
            true,
            &h.tx,
        );
        assert!(h.kept("1").unwrap().read);
        assert_eq!(tracked(&h), 1);
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

    /// A strip wants the images around its place as it scrolls, one behind and three ahead,
    /// and an image made already is not made again.
    #[tokio::test(start_paused = true)]
    async fn a_strip_wants_the_images_around_its_place() {
        let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |books| {
            let path = books.join("Chapter 1.cbz");
            cbz(&path, 8);
            vec![ShelfFile {
                path: path.display().to_string(),
                chapter: None,
                label: "Chapter 1".into(),
                volume: None,
                number_milli: Some(1000),
                format: "cbz".into(),
                pages: Some(8),
                comicinfo: None,
                source: None,
            }]
        });
        h.services.store.set_series_setting(WORK, "mode", Some("strip"), true, 2).unwrap();
        let (view, leaves, _) = h.open(OpenAt::Resume).await;
        assert_eq!(
            (view.mode, leaves.len()),
            (ReaderMode::Strip, 9),
            "eight images and the end"
        );
        let mut made = std::collections::BTreeSet::new();
        let ready = async |h: &mut Harness| {
            h.until(|e| match e {
                ReaderEvent::PageReady { page, .. } => Some(*page),
                _ => None,
            })
            .await
        };
        while made.len() < 2 {
            made.insert(ready(&mut h).await);
        }
        assert_eq!(made, [0, 1].into(), "the opening makes the first two");

        let session = h.session();
        let chapter = ChapterId::new("1");
        let at = |image| StripPos { image, offset: 0.5 };
        h.send(ReaderCommand::StripAt {
            session,
            chapter: chapter.clone(),
            at: at(4),
            foot_visible: false,
        });
        while made.len() < 7 {
            assert!(made.insert(ready(&mut h).await), "an image made twice");
        }
        assert_eq!(made, [0, 1, 3, 4, 5, 6, 7].into());

        // The same place again and a scroll back to the top: only image 3's neighbour 2 is new.
        h.send(ReaderCommand::StripAt {
            session,
            chapter: chapter.clone(),
            at: at(4),
            foot_visible: false,
        });
        h.send(ReaderCommand::StripAt { session, chapter, at: at(0), foot_visible: false });
        assert_eq!(ready(&mut h).await, 2);
        let again =
            h.drain().into_iter().filter(|e| matches!(e, ReaderEvent::PageReady { .. }));
        assert_eq!(again.count(), 0, "nothing made again");
    }

    /// A long strip opens again where it was left: at the image and how far into it, in the
    /// same run and after a restart on the same store.
    #[tokio::test(start_paused = true)]
    async fn a_strip_resumes_inside_its_image_after_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("data/kaeri.db");
        let mut h = harness(Store::open(&db).unwrap());
        h.services.store.set_series_setting(WORK, "mode", Some("strip"), true, 2).unwrap();
        let (view, _, _) = h.open(OpenAt::Resume).await;
        assert_eq!((view.mode, view.start.strip), (ReaderMode::Strip, None));
        let strip_at = |h: &mut Harness, at: StripPos| {
            let session = h.session();
            let chapter = ChapterId::new("1");
            h.send(ReaderCommand::StripAt { session, chapter, at, foot_visible: false });
            h.send(ReaderCommand::Close { session });
        };
        let at = StripPos { image: 1, offset: 0.4 };
        strip_at(&mut h, at);
        let kept = h.kept("1").unwrap();
        assert_eq!((kept.last_page, kept.strip_image), (Some(1), Some(1)));

        let (view, _, said) = h.open(OpenAt::Resume).await;
        assert_eq!((view.start.page, view.start.strip), (Some(1), Some(at)));
        assert_eq!(notices(&said), [ReadingMsg::ReaderResumedImage { image: 2 }]);
        let session = h.session();
        h.send(ReaderCommand::Close { session });
        // The book stays where the first run laid it; only the app is started again.
        let _books = h._dir;
        drop((h.reader, h.services, h.tx, h.inbox));

        // Started again on the same store.
        let mut h = harness(Store::open(&db).unwrap());
        let (view, _, said) = h.open(OpenAt::Resume).await;
        assert_eq!(view.mode, ReaderMode::Strip);
        assert_eq!((view.start.page, view.start.strip), (Some(1), Some(at)));
        assert_eq!(notices(&said), [ReadingMsg::ReaderResumedImage { image: 2 }]);

        // Partway down the first image is a place too.
        let top = StripPos { image: 0, offset: 0.25 };
        strip_at(&mut h, top);
        let (view, _, _) = h.open(OpenAt::Resume).await;
        assert_eq!((view.start.page, view.start.strip), (Some(0), Some(top)));
        let session = h.session();
        h.send(ReaderCommand::Close { session });

        // From the start, nothing of the strip's place.
        let (view, _, _) = h.open(OpenAt::Start).await;
        assert_eq!((view.start.page, view.start.strip), (Some(0), None));
    }

    #[tokio::test(start_paused = true)]
    async fn a_chapter_kept_at_its_start_opens_without_a_resumed_notice() {
        let mut h = harness(Store::open_in_memory().unwrap());
        h.open(OpenAt::Resume).await;
        h.shown(1, TurnHow::Turn).await;
        h.shown(0, TurnHow::Turn).await;
        let session = h.session();
        h.send(ReaderCommand::Close { session });
        assert_eq!(h.kept("1").and_then(|c| c.last_page), Some(0));

        let (view, _, said) = h.open(OpenAt::Resume).await;
        assert_eq!(view.start.page, Some(0));
        assert!(notices(&said).is_empty(), "no Resumed at page 1: {said:?}");
    }

    /// A strip runs on into the next chapter under its marker, so that chapter opens at its
    /// first image even where a place in it was kept; Continue on a paged chapter goes back to
    /// the kept page.
    #[tokio::test(start_paused = true)]
    async fn a_strip_flows_into_the_next_chapter_at_its_first_image() {
        let store = Store::open_in_memory().unwrap();
        let mut h = harness(store);
        // Chapter 2 was left at its second image.
        h.services.store.set_series_setting(WORK, "mode", Some("strip"), true, 2).unwrap();
        let work = WorkId::new(WORK);
        let at = OpenAt::Resume;
        h.send(ReaderCommand::Open {
            work,
            chapter: Some(ChapterId::new("2")),
            at,
            surface: surface(),
        });
        h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;
        let session = h.session();
        let chapter = ChapterId::new("2");
        let at = StripPos { image: 1, offset: 0.5 };
        h.send(ReaderCommand::StripAt { session, chapter, at, foot_visible: false });
        h.send(ReaderCommand::Close { session });
        assert_eq!(h.kept("2").and_then(|c| c.last_page), Some(1));

        let work = WorkId::new(WORK);
        let chapter = Some(ChapterId::new("1"));
        h.send(ReaderCommand::Open { work, chapter, at: OpenAt::Start, surface: surface() });
        h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;
        h.drain();
        let session = h.session();
        let foot = StripPos { image: 2, offset: 1.0 };
        let chapter = ChapterId::new("1");
        h.send(ReaderCommand::StripAt { session, chapter, at: foot, foot_visible: true });
        h.send(ReaderCommand::OpenChapter { session, chapter: ChapterId::new("2") });
        let view = h
            .until(|e| match e {
                ReaderEvent::Opened(view) => Some(view.clone()),
                _ => None,
            })
            .await;
        assert_eq!(view.chapter.id.as_str(), "2");
        assert_eq!((view.start.page, view.start.strip), (Some(0), None));
        let said = h.drain();
        assert!(notices(&said).is_empty(), "flowing on says nothing: {said:?}");
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

    /// A device sync no longer knows the name of is "another device", never its id.
    #[tokio::test(start_paused = true)]
    async fn a_device_with_no_name_is_another_device_in_the_moved_notice() {
        let mine = Store::open_in_memory().unwrap();
        mine.sync_enable("phone").unwrap();
        let gone = Store::open_in_memory().unwrap();
        gone.set_sync_clock(|| 4_000_000_000_000);
        gone.sync_enable("gone").unwrap();
        let mut h = harness(mine.clone());
        h.open(OpenAt::Resume).await;
        h.shown(1, TurnHow::Turn).await;
        let mut ahead = ReadEvent::new(WORK, "2", 2);
        ahead.page = Some(2);
        gone.record_read(&ahead, Vec::new()).unwrap();
        let ops = gone.ops_after(&mine.version_vector().unwrap(), usize::MAX).unwrap();
        mine.apply_ops(ops).unwrap();
        h.synced();
        h.until(|e| matches!(e, ReaderEvent::Opened(_)).then_some(())).await;
        let said = h.drain();
        let from = said.iter().find_map(|e| match e {
            ReaderEvent::Moved { from_device, .. } => Some(from_device.clone()),
            _ => None,
        });
        assert_eq!(from.as_deref(), Some(""));
        assert_eq!(
            notices(&said),
            [ReadingMsg::ReaderMovedNumberElsewhere { chapter: "2".into(), page: 3 }]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_device_that_read_further_while_this_one_was_away_moves_the_reader_on_open() {
        let mine = Store::open_in_memory().unwrap();
        mine.sync_enable("phone").unwrap();
        let desk = Store::open_in_memory().unwrap();
        desk.set_sync_clock(|| 4_000_000_000_000);
        desk.sync_enable("desk").unwrap();
        mine.sync_device_add("desk", "Desk", "").unwrap();
        let mut here = ReadEvent::new(WORK, "1", 1);
        here.page = Some(1);
        mine.record_read(&here, Vec::new()).unwrap();
        let mut ahead = ReadEvent::new(WORK, "2", 2);
        ahead.page = Some(2);
        desk.record_read(&ahead, Vec::new()).unwrap();
        let ops = desk.ops_after(&mine.version_vector().unwrap(), usize::MAX).unwrap();
        mine.apply_ops(ops).unwrap();
        let mut h = harness(mine);
        h.send(ReaderCommand::Open {
            work: WorkId::new(WORK),
            chapter: Some(ChapterId::new("1")),
            at: OpenAt::Resume,
            surface: surface(),
        });
        let said = h
            .until(|e| match e {
                ReaderEvent::Moved { to, from_device, .. } => {
                    Some((to.chapter.0.clone(), from_device.clone()))
                }
                _ => None,
            })
            .await;
        assert_eq!(said, ("2".into(), "Desk".into()));
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
        // The move opens chapter 2 on the page worker, which may finish before the events are
        // looked at, so the open is waited for first and what came before it is looked at after.
        let opened = h
            .until(|e| match e {
                ReaderEvent::Opened(view) => Some((view.chapter.id.clone(), view.start.page)),
                _ => None,
            })
            .await;
        assert_eq!((opened.0.as_str(), opened.1), ("2", Some(2)));
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
            [ReadingMsg::ReaderMovedNumber {
                chapter: "2".into(),
                page: 3,
                device: "Desk".into()
            }]
        );

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
        assert_eq!(profile.panel, kaeri_model::eink::EinkPanel::Grey);
        let mut kept = ReadEvent::new(WORK, "1", 1);
        kept.page = Some(2);
        h.services.store.record_read(&kept, Vec::new()).unwrap();
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
        let said = h.drain();
        let lasts = said.iter().find_map(|e| match e {
            ReaderEvent::Notice(n) => Some(n.lasts_ms),
            _ => None,
        });
        assert_eq!(lasts, Some(0));
    }

    /// A made-up AVIF file: the header a decoder reads the size from and nothing the engine can
    /// decode, for a host decoder to stand in for.
    fn avif(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\0\0\0\x1cftypavif\0\0\0\0mif1avif".to_vec();
        bytes.extend_from_slice(&20u32.to_be_bytes());
        bytes.extend_from_slice(b"ispe\0\0\0\0");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    /// A one-page archive whose page is an AVIF file.
    fn avif_book(books: &Path) -> Vec<ShelfFile> {
        let path = books.join("Chapter 1.cbz");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("000.avif", stored).unwrap();
        zip.write_all(&avif(300, 450)).unwrap();
        zip.finish().unwrap();
        vec![ShelfFile {
            path: path.display().to_string(),
            chapter: None,
            label: "Chapter 1".into(),
            volume: None,
            number_milli: Some(1000),
            format: "cbz".into(),
            pages: Some(1),
            comicinfo: None,
            source: None,
        }]
    }

    /// A host decoder that answers `image` after `delay`, and keeps what it was asked.
    struct FakeDecoder {
        image: Option<crate::host::DecodedImage>,
        delay: std::time::Duration,
        asked: std::sync::Mutex<Vec<(String, u32, u32, std::thread::ThreadId)>>,
    }

    impl FakeDecoder {
        fn new(
            image: Option<crate::host::DecodedImage>,
            delay: std::time::Duration,
        ) -> Arc<Self> {
            Arc::new(Self { image, delay, asked: Default::default() })
        }

        /// A grey ramp, 300 by 450 pixels.
        fn ramp() -> crate::host::DecodedImage {
            let rgba = (0..300 * 450)
                .flat_map(|i| {
                    let v = (i % 300 * 255 / 299) as u8;
                    [v, v, v, 255]
                })
                .collect();
            crate::host::DecodedImage { width: 300, height: 450, rgba }
        }
    }

    impl crate::host::PageDecoder for FakeDecoder {
        fn decode(
            &self,
            bytes: Vec<u8>,
            format: String,
            max_width: u32,
            max_height: u32,
        ) -> Option<crate::host::DecodedImage> {
            assert!(bytes.starts_with(b"\0\0\0\x1cftyp"));
            self.asked.lock().unwrap().push((
                format,
                max_width,
                max_height,
                std::thread::current().id(),
            ));
            std::thread::sleep(self.delay);
            self.image.clone()
        }
    }

    /// A reader on an Onyx-like grey e-ink screen, over an AVIF book, on `decoder`.
    fn avif_eink(decoder: Option<Arc<dyn crate::host::PageDecoder>>) -> Harness {
        let mut h = harness_on(Store::open_in_memory().unwrap(), "manga", avif_book, decoder);
        h.failures = true;
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
        assert_eq!(profile.panel, kaeri_model::eink::EinkPanel::Grey);
        let work = WorkId::new(WORK);
        let surface = ReaderSurface { eink: true, ..surface() };
        h.send(ReaderCommand::Open { work, chapter: None, at: OpenAt::Resume, surface });
        h
    }

    #[tokio::test]
    async fn an_avif_page_on_e_ink_is_decoded_by_the_host_and_comes_dithered() {
        let decoder = FakeDecoder::new(Some(FakeDecoder::ramp()), Default::default());
        let mut h = avif_eink(Some(decoder.clone()));
        let file = h
            .until(|e| match e {
                ReaderEvent::PageReady { file, .. } => Some(file.clone()),
                _ => None,
            })
            .await;
        assert_eq!(
            file.encoding,
            PageEncoding::Raw {
                format: kaeri_model::reader::PixelFormat::Gray8,
                stride: file.width
            }
        );
        let bytes = std::fs::read(&file.path).unwrap();
        assert_eq!(bytes.len() as u32, file.width * file.height);
        // The ramp runs from black to white, so the dither used the whole range.
        assert!(bytes.iter().any(|&v| v < 40) && bytes.iter().any(|&v| v > 215));
        let asked = decoder.asked.lock().unwrap();
        // The page worker asked, once, for the page's own size: the screen shows it larger.
        assert_eq!(asked.len(), 1);
        assert_eq!((asked[0].0.as_str(), asked[0].1, asked[0].2), ("image/avif", 300, 450));
        assert_ne!(asked[0].3, std::thread::current().id());
    }

    #[tokio::test]
    async fn an_avif_page_on_e_ink_fails_as_unsupported_without_a_host_decoder() {
        let mut h = avif_eink(None);
        let failure = h
            .until(|e| match e {
                ReaderEvent::PageFailed { failure, .. } => Some(failure.clone()),
                _ => None,
            })
            .await;
        assert_eq!(failure, ReadingFailure::Unsupported { extension: "avif".into() });
    }

    #[tokio::test]
    async fn an_avif_page_the_host_cannot_read_fails_as_unsupported() {
        let decoder = FakeDecoder::new(None, Default::default());
        let mut h = avif_eink(Some(decoder));
        let failure = h
            .until(|e| match e {
                ReaderEvent::PageFailed { failure, .. } => Some(failure.clone()),
                _ => None,
            })
            .await;
        assert_eq!(failure, ReadingFailure::Unsupported { extension: "avif".into() });
    }

    #[tokio::test]
    async fn a_host_decoder_over_its_budget_fails_the_page() {
        let slow =
            crate::reading::pages::HOST_DECODE_BUDGET + std::time::Duration::from_millis(600);
        let decoder = FakeDecoder::new(Some(FakeDecoder::ramp()), slow);
        let mut h = avif_eink(Some(decoder));
        let started = std::time::Instant::now();
        let failure = h
            .until(|e| match e {
                ReaderEvent::PageFailed { failure, .. } => Some(failure.clone()),
                _ => None,
            })
            .await;
        assert!(matches!(failure, ReadingFailure::Unreadable { .. }), "{failure:?}");
        assert!(started.elapsed() < slow, "the page waited for the decoder");
    }

    /// A PDF of `pages` vector pages, each a red square on white.
    fn vector_pdf(pages: usize) -> Vec<u8> {
        let content = b"1 0 0 rg 20 20 120 120 re f";
        let mut objects = vec![
            "<</Type/Catalog/Pages 2 0 R>>".to_owned(),
            format!(
                "<</Type/Pages/Kids[{}]/Count {pages}>>",
                (0..pages).map(|i| format!("{} 0 R", 3 + i * 2)).collect::<Vec<_>>().join(" ")
            ),
        ];
        for i in 0..pages {
            objects.push(format!(
                "<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 300]/Contents {} 0 R>>",
                4 + i * 2
            ));
            objects.push(format!(
                "<</Length {}>>\nstream\n{}\nendstream",
                content.len(),
                String::from_utf8_lossy(content)
            ));
        }
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, object) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend(format!("{} 0 obj\n{object}\nendobj\n", i + 1).bytes());
        }
        let xref = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
        for offset in offsets {
            out.extend(format!("{offset:010} 00000 n \n").bytes());
        }
        out.extend(
            format!(
                "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .bytes(),
        );
        out
    }

    #[tokio::test(start_paused = true)]
    async fn a_pdf_chapter_opens_with_its_pages_made_off_the_worker() {
        let store = Store::open_in_memory().unwrap();
        let mut h = harness(store.clone());
        let path = h._dir.path().join("books").join("Chapter 3.pdf");
        std::fs::write(&path, vector_pdf(2)).unwrap();
        let file = ShelfFile {
            path: path.display().to_string(),
            chapter: None,
            label: "Chapter 3".into(),
            volume: None,
            number_milli: Some(3000),
            format: "pdf".into(),
            pages: None,
            comicinfo: None,
            source: None,
        };
        store.put_local_file("s1", &file, 1, 1).unwrap();
        for _ in 0..2 {
            let work = WorkId::new(WORK);
            let chapter = Some(ChapterId::new("3"));
            h.send(ReaderCommand::Open {
                work,
                chapter,
                at: OpenAt::Start,
                surface: surface(),
            });
            let leaves = h
                .until(|e| match e {
                    ReaderEvent::Leaves { leaves, .. } => Some(leaves.len()),
                    _ => None,
                })
                .await;
            assert_eq!(leaves, 3, "two pages and the end page");
            let file = h
                .until(|e| match e {
                    ReaderEvent::PageReady { page: 0, file, .. } => Some(file.clone()),
                    _ => None,
                })
                .await;
            assert!(
                matches!(&file.encoding, PageEncoding::Original { mime } if mime == "image/png")
            );
            // Rendered at no more than twice the screen, whole: 600 wide by 900 high.
            assert_eq!((file.width, file.height), (1200, 1800));
            let session = h.session();
            h.send(ReaderCommand::Close { session });
        }
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
    /// The settings carry the tap zones, and a guide kept as seen stays seen for that kind of
    /// reading only, in this device's config.
    #[tokio::test(start_paused = true)]
    async fn settings_carry_the_tap_zones_and_a_seen_guide_is_kept() {
        use kaeri_model::tap_zones::{TapReading, ZoneLayout};
        let mut h = harness(Store::open_in_memory().unwrap());
        macro_rules! load {
            ($h:expr) => {{
                $h.send(ReaderCommand::LoadSettings { medium: Medium::Manga, work: None });
                $h.until(|e| match e {
                    ReaderEvent::Settings(view) => Some(view.tap_zones.clone()),
                    _ => None,
                })
            }};
        }
        let zones = load!(h).await.expect("manga has tap zones");
        assert_eq!((zones.paged, zones.scrolling), (ZoneLayout::Sides, ZoneLayout::Off));
        let reading = TapReading::new(ReaderMode::Paged, ReadDirection::RightToLeft);
        assert!(zones.guide_due(&reading), "never seen on this device");

        let guide = zones.map(&reading, None).guide;
        h.send(ReaderCommand::TapGuideSeen { guide: guide.clone() });
        let zones = load!(h).await.unwrap();
        assert_eq!(zones.guide_seen_paged, guide);
        assert!(zones.guide_seen_scrolling.is_empty());
        assert!(!zones.guide_due(&reading));
        assert_eq!(
            h.config.saved().reading.taps.get("guide_paged").and_then(|v| v.as_str()),
            Some(guide.as_str())
        );
        // A value that names no kind of reading is dropped.
        h.send(ReaderCommand::TapGuideSeen { guide: "sideways".into() });
        assert_eq!(load!(h).await.unwrap().guide_seen_paged, guide);
    }

    /// A strip hinted by the work's origin gives way to the mode the person picks, at once and
    /// not only after the reader is opened again.
    #[tokio::test(start_paused = true)]
    async fn a_mode_picked_wins_over_the_origin_hint_at_once() {
        let store = Store::open_in_memory().unwrap();
        store
            .seed_book_work(WORK, "manga", Some("MANGA"), Some("KR"), "The Lantern Keeper", 1)
            .unwrap();
        let mut h = harness(store);
        let (view, _, _) = h.open(OpenAt::Resume).await;
        assert_eq!(view.mode, ReaderMode::Strip, "the origin hints a strip");
        let session = h.session();
        h.send(ReaderCommand::SetSetting {
            session: Some(session),
            medium: Medium::Manga,
            work: Some(WorkId::new(WORK)),
            key: Key::Mode,
            scope: kaeri_model::reader::SettingScope::SeriesHere,
            value: kaeri_model::reader::ReaderValue::Choice { name: "paged".into() },
        });
        h.until(|e| matches!(e, ReaderEvent::Leaves { .. }).then_some(())).await;
        assert_eq!(h.reader.session.as_ref().unwrap().mode, ReaderMode::Paged);
    }

    /// A chapter read from a file on the shelf has no other source to choose, so the failed
    /// chapter offers Try again.
    #[test]
    fn a_file_on_the_shelf_has_no_other_source() {
        let h = harness(Store::open_in_memory().unwrap());
        let chapters = crate::reading::shelf::chapters(&h.services.store, WORK);
        let work = WorkId::new(WORK);
        assert!(crate::reading::other_sources(&h.services, &work, &chapters[0].id).is_empty());
    }

    /// Each part says its shape before its file is made, so a strip can hold its place.
    #[tokio::test(start_paused = true)]
    async fn leaves_carry_each_page_s_shape() {
        let mut h = harness(Store::open_in_memory().unwrap());
        let (_, leaves, _) = h.open(OpenAt::Resume).await;
        let ratio = leaves[0].parts[0].ratio.expect("a ratio");
        assert!((ratio - 300.0 / 450.0).abs() < 0.001, "{ratio}");
    }

    /// Near a chapter's end the next chapter's first page is made, once, for the end page.
    #[tokio::test(start_paused = true)]
    async fn the_next_chapter_s_first_page_is_made_near_the_end() {
        let mut h = harness(Store::open_in_memory().unwrap());
        h.open(OpenAt::Resume).await;
        h.shown(1, TurnHow::Turn).await;
        let (chapter, file) = h
            .until(|e| match e {
                ReaderEvent::NextPreview { chapter, file, .. } => {
                    Some((chapter.clone(), file.clone()))
                }
                _ => None,
            })
            .await;
        let opened = h.reader.session.as_ref().unwrap().chapter().id.clone();
        assert_ne!(chapter, opened, "the preview is of the next chapter");
        assert_eq!((file.width, file.height), (300, 450));
        assert!(Path::new(&file.path).exists());
        h.shown(2, TurnHow::Turn).await;
        let again = h.drain().iter().any(|e| matches!(e, ReaderEvent::NextPreview { .. }));
        assert!(!again, "made once per chapter");
    }
}
