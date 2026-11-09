//! Reading's Home and Search, while this computer is set to reading alone.
//!
//! The terminal has no reader, so both are text: Home's shelves as headed lines, and Search as
//! AniList's results followed by the person's own sources, one group each. The engine decides what
//! is found and how each group stands; this keeps the newest answer for the query on screen and
//! lays it out as the lines the screen draws and the cursor walks.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use kaeri_core::books::WorkId;
use kaeri_i18n::t;
use kaeri_model::{
    book_sources::{BookSourceCommand, BookSourcesView},
    command::Command,
    reading::{
        LibrarySegmentRead, ReadAnilistState, ReadHomeEnd, ReadHomeRow, ReadRowKind,
        ReadSearchQuery, ReadSourceGroup, ReadSourceKind, ReadSourceSearch, ReadSourceState,
        ReadingAbout, ReadingCommand, ReadingEvent, ReadingFailure, ReadingHomeView, WorkTile,
        kind_words,
    },
};

use super::{App, Field, alert, info, say};
use crate::{
    keymap::Action,
    nav::{Focus, Section, StageView},
};

/// How many of a source's hits show before its Show more line.
pub const GROUP_HITS: usize = 5;

/// How many of a Home row's titles show as lines.
pub const SHELF_TILES: usize = 5;

/// How long a part still on its way is drawn as nothing before its placeholder shows.
pub const PLACEHOLDER_AFTER: Duration = Duration::from_millis(600);

/// How long a placeholder stays once drawn, so a part that lands just after it never flickers.
pub const PLACEHOLDER_HOLD: Duration = Duration::from_millis(300);

/// How long Home's rows can be on their way before a line says suggestions are still loading.
pub const LOADING_LINE_AFTER: Duration = Duration::from_secs(4);

/// One part of the screen asked for: when, and when its answer came. Each part keeps its own, so
/// asking one source again leaves every other part's placeholder where it was.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wait {
    asked: Option<Instant>,
    landed: Option<Instant>,
}

impl Wait {
    fn asked(at: Instant) -> Self {
        Self { asked: Some(at), landed: None }
    }

    /// The answer came. The first answer counts; later ones only change what is drawn.
    fn land(&mut self, at: Instant) {
        self.landed.get_or_insert(at);
    }

    /// Whether the placeholder shows now, for a part still on its way: from 600 ms after it was
    /// asked. A part with no time of asking shows it at once.
    pub fn waited(&self, now: Instant) -> bool {
        self.asked.is_none_or(|at| now.saturating_duration_since(at) >= PLACEHOLDER_AFTER)
    }

    /// Whether a part that has landed is still drawn as on its way: its placeholder showed, and
    /// has not yet been up for 0.3 s.
    pub fn held(&self, now: Instant) -> bool {
        match (self.asked, self.landed) {
            (Some(asked), Some(landed)) => {
                let shown = asked + PLACEHOLDER_AFTER;
                landed >= shown && now < shown + PLACEHOLDER_HOLD
            }
            _ => false,
        }
    }
}

/// What a source is, beside its name in Search. A built-in source shows its name alone.
pub fn kind_word(kind: ReadSourceKind) -> String {
    match kind {
        ReadSourceKind::Server => t::read_search_source_kind_server(),
        ReadSourceKind::Folder => t::read_search_source_kind_folder(),
        ReadSourceKind::Plugin => t::read_search_source_kind_plugin(),
    }
}

/// How AniList's part of the search on screen stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnilistState {
    #[default]
    Asking,
    Answered,
    Failed,
}

/// Reading's state in the terminal.
#[derive(Debug, Default)]
pub struct Reading {
    /// Home as the engine last sent it.
    pub home: Option<ReadingHomeView>,
    /// Home asked for, so its loading line waits before it shows and then holds.
    pub home_wait: Wait,
    /// Each of Home's rows that came still on its way, by row id.
    pub row_waits: HashMap<String, Wait>,
    /// The filters the next search carries, its words aside. Sticky across queries, as the
    /// shows' filters are.
    pub filter: ReadSearchQuery,
    /// The search the results on screen answer, once one has been asked.
    pub searched: Option<ReadSearchQuery>,
    /// When it was asked: the time every part of it starts from.
    pub asked: Option<Instant>,
    /// AniList's part, and each source's by its id, so placeholders and spinners wait before
    /// they show and hold once they do.
    pub anilist_wait: Wait,
    pub source_waits: HashMap<String, Wait>,
    /// The name of the Home row View all came from, with the search it asked, while that search
    /// is the one on screen.
    pub view_all: Option<(ReadSearchQuery, String)>,
    /// The lines the cursor walked when last looked at, so the cursor stays on its line when
    /// lines land above it.
    keys: Vec<String>,
    pub anilist: Vec<WorkTile>,
    pub anilist_state: AnilistState,
    /// The last page of AniList's results on screen, counted from 0.
    pub page: u32,
    pub more: bool,
    pub total: Option<u32>,
    pub loading_more: bool,
    /// The sources' part, the newest for the search on screen.
    pub sources: Option<ReadSourceSearch>,
    /// Groups opened past their first hits with Show more.
    pub expanded: HashSet<String>,
    /// How many sources are on, from the Sources view.
    on: usize,
    /// While Kaeri is for both: Search is turned to reading's.
    pub side: bool,
}

/// Which part of the screen a line belongs to: Tab moves between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// A Home shelf, numbered from the top.
    Shelf(usize),
    Anilist,
    Sources,
}

/// One line of reading's Home or Search, in the order drawn.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadLine {
    Gap,
    /// A Home shelf's ruled heading, with the line beside it.
    Shelf {
        title: String,
        side: Option<String>,
    },
    /// A line said quietly: offline, nothing found, the closing lines.
    Quiet(String),
    /// A line said in the text colour: what a search found nothing for.
    Said(String),
    /// Something that failed, with the key that asks again. A source's or AniList's is a line
    /// the cursor can land on; Home's is asked again with the key alone.
    Failed {
        reason: String,
        retry: Retry,
    },
    /// Two placeholder lines held for a shelf still on its way, drawn once it has waited.
    Placeholder {
        shown: bool,
    },
    /// A series: a Home tile, an AniList result or a source's hit.
    Work {
        work: WorkId,
        title: String,
        /// Its facts, or on Home what its line says.
        facts: String,
        /// On AniList's results: the sources that have it.
        joined: Option<String>,
        /// A new chapter, or another line worth the state role.
        fresh: bool,
        /// Whether `a` puts it on Planning: a discovery row's title or an AniList result.
        plannable: bool,
        part: Part,
    },
    /// View all under a Home shelf: Search with the shelf's filters, under the shelf's name.
    ViewAll {
        query: Box<ReadSearchQuery>,
        title: Option<String>,
        part: Part,
    },
    /// AniList's heading, with its count once it is known.
    AnilistHeading(String),
    /// AniList is being asked; the spinner shows once it has waited.
    AnilistAsking {
        shown: bool,
    },
    /// More from AniList, with how many results show.
    MoreAnilist {
        shown: String,
    },
    /// From your sources.
    SourcesHeading,
    /// A source's name and what it is.
    Group {
        name: String,
        kind: Option<String>,
    },
    /// A source still asked; the spinner shows once it has waited.
    Asking {
        name: String,
        shown: bool,
    },
    ShowMore {
        source_id: String,
        n: usize,
    },
}

/// What the key on a failure line asks again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retry {
    Home,
    Anilist,
    Source(String),
}

impl ReadLine {
    /// Whether the cursor can land on it.
    pub fn selectable(&self) -> bool {
        match self {
            Self::Work { .. }
            | Self::ViewAll { .. }
            | Self::MoreAnilist { .. }
            | Self::ShowMore { .. } => true,
            Self::Failed { retry, .. } => *retry != Retry::Home,
            _ => false,
        }
    }

    /// The part of the screen a selectable line is in.
    pub fn part(&self) -> Part {
        match self {
            Self::Work { part, .. } | Self::ViewAll { part, .. } => *part,
            Self::MoreAnilist { .. } | Self::Failed { retry: Retry::Anilist, .. } => {
                Part::Anilist
            }
            _ => Part::Sources,
        }
    }

    /// What the line is, whatever lands around it: the cursor follows this. `shelf` is the
    /// heading of the Home row it is under.
    fn key(&self, shelf: &str) -> Option<String> {
        let part = match self.part() {
            Part::Shelf(_) => shelf,
            Part::Anilist => "anilist", // i18n-skip
            Part::Sources => "sources", // i18n-skip
        };
        Some(match self {
            Self::Work { work, .. } => format!("{part}\u{1f}{}", work.as_str()), // i18n-skip
            Self::ViewAll { .. } => format!("{part}\u{1f}all"),                  // i18n-skip
            Self::MoreAnilist { .. } => "anilist\u{1f}more".into(),              // i18n-skip
            Self::Failed { retry: Retry::Anilist, .. } => "anilist\u{1f}failed".into(), // i18n-skip
            Self::Failed { retry: Retry::Source(id), .. } => format!("failed\u{1f}{id}"), // i18n-skip
            Self::ShowMore { source_id, .. } => format!("more\u{1f}{source_id}"), // i18n-skip
            _ => return None,
        })
    }
}

impl Reading {
    /// Take the Sources view: how many sources are on. The folders on this computer are one
    /// source, as the engine asks them.
    fn take_sources(&mut self, view: &BookSourcesView) {
        let servers = view.servers.iter().filter(|server| !server.off).count();
        let plugins = view.plugins.iter().filter(|plugin| !plugin.off).count();
        let folders = usize::from(view.folders.iter().any(|folder| !folder.removed));
        self.on = servers + plugins + folders;
    }

    /// Whether the search on screen hides AniList's results.
    pub fn sources_only(&self) -> bool {
        self.filter.sources_only
    }
}

/// Whether a reading search asks for nothing: no words, and no filter set.
pub fn read_query_blank(query: &ReadSearchQuery) -> bool {
    query.text.trim().is_empty()
        && query.formats.is_empty()
        && query.origins.is_empty()
        && query.status.is_none()
        && query.genres.is_empty()
        && query.exclude_genres.is_empty()
        && query.sort.is_none()
        && query.started.as_deref().is_none_or(|s| s == "any")
        && !query.sources_only
}

impl App {
    /// Whether `section` is reading's: Home while Kaeri is for reading alone, Read while it is for
    /// both, and Search in either once it is turned to reading.
    pub fn reads(&self, section: Section) -> bool {
        match section {
            Section::Home => self.reading_only(),
            Section::Read => self.both(),
            Section::Search => self.reading_only() || (self.both() && self.reading.side),
            _ => false,
        }
    }

    /// Whether reading's Home or Search is the view on top.
    pub fn reading_here(&self) -> bool {
        matches!(self.nav.current(), StageView::Section(section) if self.reads(*section))
    }

    /// Whether the shows' Home or Search is on top while Kaeri is for both, so the status line
    /// says whose they are and which key turns to reading's.
    pub fn shows_side_here(&self) -> bool {
        self.both()
            && !self.nav.has_overlay()
            && matches!(
                self.nav.current(),
                StageView::Section(section @ (Section::Home | Section::Search))
                    if !self.reads(*section)
            )
    }

    /// Whether the section on screen is one reading draws, under whatever is pushed over it.
    pub fn reading_section(&self) -> bool {
        self.reads(self.nav.section())
    }

    /// Whether reading's section on screen is its Home: Home, or Read beside the shows'.
    fn read_home_here(&self) -> bool {
        self.nav.section() != Section::Search
    }

    /// The search the field and the reading filters make.
    pub fn read_query(&self) -> ReadSearchQuery {
        ReadSearchQuery {
            text: self.text(Field::Search).trim().to_owned(),
            ..self.reading.filter.clone()
        }
    }

    /// Work to fill reading's Home or Search.
    pub(super) fn reload_reading(&mut self) -> Option<Command> {
        self.commands.push(Command::BookSources(BookSourceCommand::Load));
        if self.read_home_here() {
            self.ask_home();
            return Some(Command::Reading(ReadingCommand::LoadHome { refresh: false }));
        }
        if read_query_blank(&self.read_query()) {
            self.reading.searched = None;
            self.reading.sources = None;
            return None;
        }
        Some(self.run_read_search())
    }

    /// Home asked for. While one is on screen it stays, so nothing waits on its loading line.
    fn ask_home(&mut self) {
        let now = self.clock.now_instant();
        let reading = &mut self.reading;
        reading.home_wait = Wait::asked(now);
        if reading.home.is_some() {
            reading.home_wait.land(now);
        }
        reading.row_waits.clear();
    }

    /// Ask the search the field and the filters make, from the top of its results.
    pub(super) fn run_read_search(&mut self) -> Command {
        let query = self.read_query();
        let now = self.clock.now_instant();
        let reading = &mut self.reading;
        reading.searched = Some(query.clone());
        reading.asked = Some(now);
        reading.anilist_wait = Wait::asked(now);
        reading.source_waits.clear();
        if reading.view_all.as_ref().is_some_and(|(asked, _)| *asked != query) {
            reading.view_all = None;
        }
        reading.anilist.clear();
        reading.anilist_state = AnilistState::Asking;
        reading.page = 0;
        reading.more = false;
        reading.total = None;
        reading.loading_more = false;
        reading.sources = None;
        reading.expanded.clear();
        reading.keys.clear();
        self.cursors.search = Default::default();
        Command::Reading(ReadingCommand::Search { query })
    }

    /// Whether Search has lines for ↓ to move into from its field.
    pub fn search_has_results(&self) -> bool {
        if self.reading_section() {
            return !self.read_items().is_empty();
        }
        !self.content.is_empty()
    }

    /// Whether Enter in Search's field opens the highlighted line rather than asking: the lines
    /// on screen answer the query as typed.
    pub fn read_opens_on_enter(&self) -> bool {
        self.reading.searched.as_ref() == Some(&self.read_query())
            && !self.read_items().is_empty()
    }

    /// Enter in Search's field: the search, or the highlighted line once the results answer it.
    pub(super) fn submit_read_search(&mut self) -> Option<Command> {
        if self.read_opens_on_enter() {
            return self.open_read_line();
        }
        if read_query_blank(&self.read_query()) {
            return None;
        }
        Some(self.run_read_search())
    }

    /// The engine's word on reading.
    pub(super) fn apply_reading(&mut self, event: ReadingEvent) {
        let now = self.clock.now_instant();
        match event {
            ReadingEvent::Home(view) => {
                let reading = &mut self.reading;
                reading.home_wait.land(now);
                let asked = reading.home_wait.asked;
                for row in &view.rows {
                    if row.tiles.is_empty() && row.pending {
                        reading
                            .row_waits
                            .entry(row.id.clone())
                            .or_insert(Wait { asked, landed: None });
                    } else if let Some(wait) = reading.row_waits.get_mut(&row.id) {
                        wait.land(now);
                    }
                }
                reading.home = Some(*view);
            }
            ReadingEvent::Search { query, page, tiles, has_more, total, .. } => {
                if self.reading.searched.as_ref() != Some(&query) {
                    return;
                }
                let reading = &mut self.reading;
                if page == 0 {
                    reading.anilist = tiles;
                } else {
                    let known: HashSet<WorkId> =
                        reading.anilist.iter().map(|tile| tile.work.clone()).collect();
                    reading
                        .anilist
                        .extend(tiles.into_iter().filter(|t| !known.contains(&t.work)));
                }
                reading.anilist_state = AnilistState::Answered;
                reading.anilist_wait.land(now);
                reading.page = reading.page.max(page);
                reading.more = has_more;
                reading.total = total;
                reading.loading_more = false;
            }
            ReadingEvent::SourceSearch(sources) => {
                if self.reading.searched.as_ref() != Some(&sources.query) {
                    return;
                }
                let reading = &mut self.reading;
                if sources.anilist == ReadAnilistState::Failed {
                    reading.anilist_state = AnilistState::Failed;
                    reading.anilist_wait.land(now);
                }
                // A group seen asking waits from when it was asked; one that answered in the
                // same word as it first showed was never drawn waiting, so holds nothing.
                for group in &sources.groups {
                    let asking = group.state == ReadSourceState::Asking;
                    match reading.source_waits.get_mut(&group.source_id) {
                        Some(wait) if !asking => wait.land(now),
                        Some(_) => {}
                        None => {
                            let asked = reading.asked;
                            let landed = if asking { None } else { asked };
                            reading
                                .source_waits
                                .insert(group.source_id.clone(), Wait { asked, landed });
                        }
                    }
                }
                reading.sources = Some(*sources);
            }
            ReadingEvent::Failed { about: ReadingAbout::Search, failure } => {
                let reading = &mut self.reading;
                if reading.loading_more {
                    reading.loading_more = false;
                    self.push_toast(alert(say(&failure.msg())));
                } else if reading.searched.is_some()
                    && reading.anilist_state == AnilistState::Asking
                {
                    reading.anilist_state = AnilistState::Failed;
                    reading.anilist_wait.land(now);
                }
            }
            // Opening a series is the one other thing asked from here.
            ReadingEvent::Failed { about: ReadingAbout::Series, failure }
                if self.reading_only() || self.both() =>
            {
                self.push_toast(alert(say(&failure.msg())));
            }
            _ => {}
        }
        self.settle_reading_cursor();
    }

    /// The Sources view, for how many sources there are.
    pub(super) fn apply_book_sources(&mut self, view: &BookSourcesView) {
        self.reading.take_sources(view);
    }

    /// Keep the cursor on the line it was on when lines land above or below it, at the same
    /// height in the window, once the person is among the lines. A line that has gone, or a
    /// cursor still behind Search's field, stays where it was, within the lines there are.
    /// Called when an answer lands and on the idle tick, as a held placeholder gives way.
    pub(super) fn settle_reading_cursor(&mut self) {
        if !self.reading_section() {
            return;
        }
        let keys = self.read_keys();
        if keys == self.reading.keys {
            return;
        }
        let old = std::mem::replace(&mut self.reading.keys, keys);
        let section = self.nav.section();
        let selected = self.cursors.section(section).selected;
        let among = match section {
            Section::Search => self.search_in_results,
            _ => self.nav.focus() == Focus::Stage,
        };
        let found = old
            .get(selected)
            .filter(|_| among)
            .and_then(|key| self.reading.keys.iter().position(|k| k == key));
        let len = self.reading.keys.len();
        let cursor = self.cursors.section_mut(section);
        match found {
            Some(at) => {
                let above = cursor.selected.saturating_sub(cursor.offset);
                cursor.selected = at;
                cursor.offset = at.saturating_sub(above);
            }
            None => {
                cursor.selected = cursor.selected.min(len.saturating_sub(1));
                cursor.offset = cursor.offset.min(cursor.selected);
            }
        }
    }

    /// What each line the cursor walks is, in order.
    fn read_keys(&self) -> Vec<String> {
        let mut shelf = String::new();
        self.read_lines()
            .iter()
            .filter_map(|line| {
                if let ReadLine::Shelf { title, .. } = line {
                    shelf.clone_from(title);
                }
                if line.selectable() { line.key(&shelf) } else { None }
            })
            .collect()
    }

    /// Reading's lines for the section on screen.
    pub fn read_lines(&self) -> Vec<ReadLine> {
        if self.read_home_here() { self.read_home_lines() } else { self.read_search_lines() }
    }

    /// The lines the cursor walks, in order.
    pub fn read_items(&self) -> Vec<ReadLine> {
        self.read_lines().into_iter().filter(ReadLine::selectable).collect()
    }

    /// The line under the cursor.
    pub fn read_selected(&self) -> Option<ReadLine> {
        self.read_items().into_iter().nth(self.section_cursor().selected)
    }

    /// Home's lines: a notice, the person's own shelves, the discovery rows and the end.
    fn read_home_lines(&self) -> Vec<ReadLine> {
        let mut lines = Vec::new();
        let Some(view) = &self.reading.home else { return lines };
        let now = self.clock.now_instant();
        if view.offline {
            let time = view
                .offline_since
                .and_then(|at| self.clock.local(at))
                .map(|at| at.format("%H:%M").to_string());
            lines.push(ReadLine::Quiet(match time {
                Some(time) => t::home_offline(&time),
                None => t::home_offline_empty_title(),
            }));
            lines.push(ReadLine::Gap);
        } else if view.failed {
            lines.push(ReadLine::Failed { reason: t::home_failed(), retry: Retry::Home });
            lines.push(ReadLine::Gap);
        }
        let mut shelf = 0;
        let own = [
            (t::home_continue(), &view.continue_reading),
            (t::home_this_week(), &view.this_week),
            (t::home_up_next(), &view.up_next),
        ];
        for (title, tiles) in own {
            if tiles.is_empty() {
                continue;
            }
            if !lines.is_empty() && lines.last() != Some(&ReadLine::Gap) {
                lines.push(ReadLine::Gap);
            }
            lines.push(ReadLine::Shelf { title, side: None });
            lines.extend(
                tiles.iter().take(SHELF_TILES).map(|tile| home_tile(tile, shelf, None, false)),
            );
            shelf += 1;
        }
        let mut loading_said = false;
        for row in &view.rows {
            let wait = self.reading.row_waits.get(&row.id);
            let on_way = row.tiles.is_empty() && row.pending;
            let held = wait.is_some_and(|wait| wait.held(now));
            if row.tiles.is_empty() && !on_way {
                continue;
            }
            if !lines.is_empty() && lines.last() != Some(&ReadLine::Gap) {
                lines.push(ReadLine::Gap);
            }
            lines.push(ReadLine::Shelf {
                title: say(&row.title),
                side: row.side.as_ref().map(say),
            });
            if on_way || held {
                let shown = held || wait.unwrap_or(&self.reading.home_wait).waited(now);
                lines.push(ReadLine::Placeholder { shown });
                // Past 4 s, the first row still on its way says what it waits for.
                if shown && !loading_said && self.home_waited(now, LOADING_LINE_AFTER) {
                    lines.push(ReadLine::Quiet(t::home_loading()));
                    loading_said = true;
                }
                shelf += 1;
                continue;
            }
            lines.extend(
                row.tiles
                    .iter()
                    .take(SHELF_TILES)
                    .map(|tile| home_tile(tile, shelf, Some(row), !tile.demo)),
            );
            if let Some(query) = &row.view_all {
                lines.push(ReadLine::ViewAll {
                    query: Box::new(query.clone()),
                    title: row.view_all_title.as_ref().map(say),
                    part: Part::Shelf(shelf),
                });
            }
            shelf += 1;
        }
        if view.offline && view.rows.iter().all(|row| row.tiles.is_empty()) {
            lines.push(ReadLine::Gap);
            lines.push(ReadLine::Quiet(t::read_home_offline_rows()));
        }
        if view.end == Some(ReadHomeEnd::NoSources) || view.no_sources {
            lines.push(ReadLine::Gap);
            lines.push(ReadLine::Quiet(t::tui_read_sources_in_apps()));
        }
        lines
    }

    /// Whether Home was asked for at least `long` ago.
    fn home_waited(&self, now: Instant, long: Duration) -> bool {
        self.reading.home_wait.asked.is_some_and(|at| now.saturating_duration_since(at) >= long)
    }

    /// Search's lines: AniList's results, then From your sources.
    fn read_search_lines(&self) -> Vec<ReadLine> {
        let mut lines = Vec::new();
        let reading = &self.reading;
        let Some(query) = &reading.searched else { return lines };
        let now = self.clock.now_instant();
        let sources = reading.sources.as_ref();
        let joins: HashMap<&WorkId, &kaeri_model::reading::ReadSourceJoin> = sources
            .map(|s| s.joins.iter().map(|join| (&join.work, join)).collect())
            .unwrap_or_default();
        let search_wait = Wait { asked: reading.asked, landed: None };
        let wait_of = |group: &ReadSourceGroup| {
            reading.source_waits.get(&group.source_id).copied().unwrap_or(search_wait)
        };
        // A part that answered while its placeholder was up keeps it 0.3 s more.
        let on_way = |group: &ReadSourceGroup| {
            group.state == ReadSourceState::Asking || wait_of(group).held(now)
        };
        let anilist_on_way =
            reading.anilist_state == AnilistState::Asking || reading.anilist_wait.held(now);

        let groups_found = sources.is_some_and(|s| {
            s.groups.iter().any(|g| {
                on_way(g)
                    || !g.tiles.is_empty()
                    || matches!(
                        g.state,
                        ReadSourceState::Failed { .. } | ReadSourceState::TimedOut { .. }
                    )
            })
        });
        let anilist_empty = query.sources_only
            || (!anilist_on_way
                && reading.anilist_state == AnilistState::Answered
                && reading.anilist.is_empty());
        let settled_empty = anilist_empty && sources.is_some_and(|s| s.done) && !groups_found;
        if settled_empty && !query.text.is_empty() {
            if query.sources_only {
                lines.push(ReadLine::Said(t::tui_read_sources_nothing()));
                lines.push(ReadLine::Quiet(t::read_search_nothing_line()));
            } else {
                lines.push(ReadLine::Said(t::read_search_nothing(&query.text)));
                lines.push(ReadLine::Quiet(t::read_search_nothing_line()));
                lines.push(ReadLine::Gap);
                lines.push(ReadLine::Quiet(t::read_search_sources_nothing()));
            }
            return lines;
        }

        if !query.sources_only {
            match reading.anilist_state {
                _ if anilist_on_way => {
                    let held = reading.anilist_wait.held(now);
                    lines.push(ReadLine::AnilistHeading(self.anilist_heading(None)));
                    lines.push(ReadLine::AnilistAsking {
                        shown: held || reading.anilist_wait.waited(now),
                    });
                }
                AnilistState::Failed => {
                    lines.push(ReadLine::Failed {
                        reason: t::read_search_anilist_failed(),
                        retry: Retry::Anilist,
                    });
                    lines.push(ReadLine::Quiet(t::read_search_anilist_failed_line()));
                }
                // Nothing to count: the one line says it.
                _ if reading.anilist.is_empty() => {
                    lines.push(ReadLine::Quiet(t::read_search_anilist_nothing()));
                }
                _ => {
                    lines.push(ReadLine::AnilistHeading(self.anilist_heading(reading.total)));
                    for tile in &reading.anilist {
                        let joined = joins.get(&tile.work).and_then(|join| {
                            match join.source_names.len() {
                                0 => None,
                                1 => Some(t::read_search_in_source(&join.source_names[0])),
                                n => Some(t::read_search_in_sources(n as i64)),
                            }
                        });
                        lines.push(ReadLine::Work {
                            work: tile.work.clone(),
                            title: tile.title.clone(),
                            facts: anilist_facts(tile),
                            joined,
                            fresh: false,
                            plannable: !tile.demo,
                            part: Part::Anilist,
                        });
                    }
                    if reading.more {
                        let shown = reading.anilist.len() as i64;
                        let shown = match reading.total {
                            Some(total) => t::read_search_shown(shown, i64::from(total)),
                            None => t::read_search_shown_some(shown),
                        };
                        lines.push(ReadLine::MoreAnilist { shown });
                    }
                }
            }
        }

        let Some(sources) = sources else {
            return lines;
        };
        let shown: Vec<_> = sources
            .groups
            .iter()
            .filter(|g| {
                on_way(g)
                    || match g.state {
                        ReadSourceState::Nothing => false,
                        ReadSourceState::Hits => !g.tiles.is_empty(),
                        _ => true,
                    }
            })
            .collect();
        if !shown.is_empty() {
            if !lines.is_empty() {
                lines.push(ReadLine::Gap);
            }
            lines.push(ReadLine::SourcesHeading);
        }
        for group in shown {
            lines.push(ReadLine::Group {
                name: group.source_name.clone(),
                kind: group.kind.map(kind_word),
            });
            if on_way(group) {
                let wait = wait_of(group);
                lines.push(ReadLine::Asking {
                    name: group.source_name.clone(),
                    shown: wait.held(now) || wait.waited(now),
                });
                continue;
            }
            match &group.state {
                ReadSourceState::Failed { failure } => lines.push(ReadLine::Failed {
                    reason: failed_reason(&group.source_name, Some(failure), None),
                    retry: Retry::Source(group.source_id.clone()),
                }),
                ReadSourceState::TimedOut { seconds } => lines.push(ReadLine::Failed {
                    reason: failed_reason(&group.source_name, None, Some(*seconds)),
                    retry: Retry::Source(group.source_id.clone()),
                }),
                _ => {
                    let open = reading.expanded.contains(&group.source_id);
                    let take = if open { group.tiles.len() } else { GROUP_HITS };
                    for tile in group.tiles.iter().take(take) {
                        lines.push(ReadLine::Work {
                            work: tile.work.clone(),
                            title: tile.title.clone(),
                            facts: hit_facts(tile),
                            joined: None,
                            fresh: false,
                            plannable: false,
                            part: Part::Sources,
                        });
                    }
                    if !open && group.tiles.len() > GROUP_HITS {
                        lines.push(ReadLine::ShowMore {
                            source_id: group.source_id.clone(),
                            n: group.tiles.len() - GROUP_HITS,
                        });
                    }
                }
            }
        }
        let nothing: Vec<&str> = sources
            .groups
            .iter()
            .filter(|g| {
                !on_way(g)
                    && (g.state == ReadSourceState::Nothing
                        || (g.state == ReadSourceState::Hits
                            && g.tiles.is_empty()
                            && g.joined == 0))
            })
            .map(|g| g.source_name.as_str())
            .collect();
        if !nothing.is_empty() {
            lines.push(ReadLine::Gap);
            lines.push(ReadLine::Quiet(t::read_search_source_nothing(&names_joined(&nothing))));
        }
        lines
    }

    /// AniList's heading: how many series it found, without a number until the engine knows it,
    /// after the name of the Home row View all came from.
    fn anilist_heading(&self, total: Option<u32>) -> String {
        let count = match total {
            Some(total) => t::read_search_total(i64::from(total)),
            None => t::read_search_total_unknown(),
        };
        let reading = &self.reading;
        match reading
            .view_all
            .as_ref()
            .filter(|(asked, _)| reading.searched.as_ref() == Some(asked))
        {
            Some((_, title)) => format!("{title} · {count}"), // i18n-skip
            None => count,
        }
    }

    /// Whether reading's Home has waited long enough for its loading line, and holds it 0.3 s
    /// once drawn.
    pub fn read_home_loading(&self) -> Option<bool> {
        let now = self.clock.now_instant();
        let wait = &self.reading.home_wait;
        if self.reading.home.is_none() {
            return Some(wait.waited(now));
        }
        wait.held(now).then_some(true)
    }

    /// A key on reading's Home or Search, ahead of the shows' meaning of it. `None` lets it
    /// through.
    pub(super) fn reading_key(&mut self, action: Action) -> Option<Option<Command>> {
        if !self.reading_here() {
            return None;
        }
        self.settle_reading_cursor();
        let on_stage = self.nav.focus() == Focus::Stage;
        let search = self.nav.section() == Section::Search;
        match action {
            Action::Open if on_stage => Some(self.open_read_line()),
            Action::ShowSources if search => Some(self.toggle_sources_only()),
            // Tab moves between the parts here, and with one part does nothing: the rail is a
            // key away, and folding it would be something the status line never offered.
            Action::ToggleRail if on_stage => {
                self.next_read_part();
                Some(None)
            }
            Action::Refresh => Some(self.refresh_reading()),
            Action::Filter if search => {
                self.open_search_filters();
                Some(None)
            }
            Action::Surprise => {
                self.works_elsewhere(action);
                Some(None)
            }
            Action::SetListStatus if on_stage => Some(self.plan_read_line()),
            _ if action.acts_on_a_title() => {
                self.works_elsewhere(action);
                Some(None)
            }
            _ => None,
        }
    }

    /// Enter on a reading line.
    pub(super) fn open_read_line(&mut self) -> Option<Command> {
        let Some(line) = self.read_selected() else {
            self.push_toast(info(t::lists_nothing_selected()));
            return None;
        };
        match line {
            // The terminal has no reader: the series opens in the apps. Asking for it still
            // tells the engine whether a source has it, which Home says next time.
            ReadLine::Work { work, .. } => {
                self.push_toast(info(t::tui_read_opens_in_apps()));
                Some(Command::Reading(ReadingCommand::LoadSeries { work }))
            }
            ReadLine::ViewAll { query, title, .. } => {
                let query = *query;
                self.set_text(Field::Search, query.text.clone());
                self.reading.filter = ReadSearchQuery { text: String::new(), ..query.clone() };
                self.reading.view_all = title.map(|title| (query, title));
                self.reading.side = true;
                self.nav.go_to(Section::Search);
                self.nav.focus_stage();
                self.search_in_results = true;
                self.detail = None;
                Some(self.run_read_search())
            }
            ReadLine::MoreAnilist { .. } => self.more_anilist(),
            ReadLine::ShowMore { source_id, .. } => {
                self.reading.expanded.insert(source_id);
                None
            }
            ReadLine::Failed { retry, .. } => self.retry(retry),
            _ => None,
        }
    }

    /// `a`: the title under the cursor onto Planning, on a discovery row or AniList's results.
    fn plan_read_line(&mut self) -> Option<Command> {
        match self.read_selected() {
            Some(ReadLine::Work { work, plannable: true, .. }) => {
                self.push_toast(info(t::library_added_planning()));
                Some(Command::Reading(ReadingCommand::SetStatus {
                    work,
                    status: LibrarySegmentRead::Planning,
                }))
            }
            _ => {
                self.works_elsewhere(Action::SetListStatus);
                None
            }
        }
    }

    /// `W`, while Kaeri is for both: from the shows' Home to Read and back, and Search turned
    /// between the shows' and reading's.
    pub(super) fn switch_medium(&mut self) -> Option<Command> {
        let section = match self.nav.current() {
            StageView::Section(section) if self.both() => *section,
            _ => {
                self.works_elsewhere(Action::SwitchMedium);
                return None;
            }
        };
        match section {
            Section::Home => self.go_to_section(Section::Read),
            Section::Read => self.go_to_section(Section::Home),
            Section::Search => {
                self.reading.side = !self.reading.side;
                self.reading.keys.clear();
                *self.cursors.section_mut(section) = Default::default();
                self.reload()
            }
            _ => {
                self.works_elsewhere(Action::SwitchMedium);
                None
            }
        }
    }

    /// The next page of AniList's results.
    fn more_anilist(&mut self) -> Option<Command> {
        let reading = &mut self.reading;
        if reading.loading_more || !reading.more {
            return None;
        }
        let query = reading.searched.clone()?;
        reading.loading_more = true;
        Some(Command::Reading(ReadingCommand::SearchMore { query, page: reading.page + 1 }))
    }

    /// Ask again what a failure line names: Home, the whole search for AniList, or one source,
    /// whose group alone goes back to asking.
    fn retry(&mut self, retry: Retry) -> Option<Command> {
        match retry {
            Retry::Home => {
                self.ask_home();
                self.commands.push(Command::BookSources(BookSourceCommand::Load));
                Some(Command::Reading(ReadingCommand::LoadHome { refresh: true }))
            }
            Retry::Anilist => {
                let in_results = self.search_in_results;
                let task = self.run_read_search();
                self.search_in_results = in_results;
                Some(task)
            }
            Retry::Source(source_id) => {
                let query = self.reading.searched.clone()?;
                let now = self.clock.now_instant();
                self.reading.source_waits.insert(source_id.clone(), Wait::asked(now));
                Some(Command::Reading(ReadingCommand::RetrySourceSearch { query, source_id }))
            }
        }
    }

    /// `r`: Home loaded again; on Search, the failure under the cursor, else the first on the
    /// screen, else everything again.
    fn refresh_reading(&mut self) -> Option<Command> {
        if self.read_home_here() {
            return self.retry(Retry::Home);
        }
        if let Some(ReadLine::Failed { retry, .. }) = self.read_selected() {
            return self.retry(retry);
        }
        let failed = self.read_lines().into_iter().find_map(|line| match line {
            ReadLine::Failed { retry, .. } => Some(retry),
            _ => None,
        });
        if let Some(retry) = failed {
            return self.retry(retry);
        }
        if read_query_blank(&self.read_query()) {
            return None;
        }
        Some(self.run_read_search())
    }

    /// `s` on Search: Your sources only, on or off, and the search asked again with it. With no
    /// words there is nothing for the sources to find, so the line says to type a title.
    pub(super) fn toggle_sources_only(&mut self) -> Option<Command> {
        if self.text(Field::Search).trim().is_empty() {
            self.push_toast(info(t::tui_read_sources_only_words()));
            return None;
        }
        let on = !self.reading.filter.sources_only;
        self.reading.filter.sources_only = on;
        self.push_toast(info(if on {
            t::tui_read_sources_only_on()
        } else {
            t::tui_read_sources_only_off()
        }));
        let in_results = self.search_in_results;
        let task = self.run_read_search();
        self.search_in_results = in_results;
        Some(task)
    }

    /// Tab: the first line of the next part, round to the first. `false` with one part only.
    fn next_read_part(&mut self) -> bool {
        let items = self.read_items();
        let mut starts: Vec<usize> = Vec::new();
        let mut last: Option<Part> = None;
        for (n, item) in items.iter().enumerate() {
            let part = item.part();
            if last != Some(part) {
                starts.push(n);
                last = Some(part);
            }
        }
        if starts.len() < 2 {
            return false;
        }
        let section = self.nav.section();
        let selected = self.cursors.section(section).selected;
        let next = starts.iter().copied().find(|start| *start > selected).unwrap_or(starts[0]);
        let cursor = self.cursors.section_mut(section);
        cursor.selected = next;
        cursor.offset = next;
        if section == Section::Search {
            self.search_in_results = true;
        }
        true
    }

    /// How the status line names reading: the sources a search asks.
    pub fn read_status(&self, wide: bool) -> String {
        if wide {
            t::tui_read_status(self.reading.on as i64)
        } else {
            t::tui_read_status_short()
        }
    }

    /// How many sources are on, for the header while Kaeri is for reading.
    pub fn read_sources_on(&self) -> usize {
        self.reading.on
    }
}

/// A Home title as a line: Not found in your sources when a source was asked and had nothing,
/// else what the engine says is next, else its caption. `plannable` for a discovery row's, which
/// `a` puts on Planning.
fn home_tile(
    tile: &WorkTile,
    shelf: usize,
    row: Option<&ReadHomeRow>,
    plannable: bool,
) -> ReadLine {
    let facts = if tile.not_in_sources {
        t::home_not_found()
    } else if let Some(next) = &tile.next {
        say(next)
    } else {
        home_caption(tile, row)
    };
    ReadLine::Work {
        work: tile.work.clone(),
        title: tile.title.clone(),
        facts,
        joined: None,
        fresh: tile.new_chapter && !tile.not_in_sources,
        plannable,
        part: Part::Shelf(shelf),
    }
}

/// A discovery title's caption. A row of one format leaves the format out: origin and status, or
/// on a New row origin and the month it started ("Japan · since March"). A mixed row leads with
/// the format, as Search does.
fn home_caption(tile: &WorkTile, row: Option<&ReadHomeRow>) -> String {
    let Some(row) = row.filter(|row| row.format.is_some()) else {
        return fact_line(tile);
    };
    let status = tile.facts.get(1);
    let second = match row.kind {
        ReadRowKind::New => tile.started.as_ref().or(status),
        _ => status,
    };
    let caption = facts_joined(tile.origin.iter().chain(second).map(say));
    if caption.is_empty() { fact_line(tile) } else { caption }
}

/// A tile's facts as one line.
fn fact_line(tile: &WorkTile) -> String {
    facts_joined(tile.facts.iter().map(say))
}

/// An AniList result's facts: its format, then where it comes from when AniList says
/// ("Light novel · Japan").
fn anilist_facts(tile: &WorkTile) -> String {
    facts_joined(tile.facts.first().into_iter().chain(&tile.origin).map(say))
}

/// A source's hit: its format in the words AniList's lines use, then how much of it the source
/// holds ("Light novel · 2 volumes", "Manga · 1 file · CBZ") when the source says.
fn hit_facts(tile: &WorkTile) -> String {
    let format = tile
        .facts
        .first()
        .cloned()
        .unwrap_or_else(|| kind_words(tile.format_label.as_deref(), tile.medium, None));
    facts_joined(std::iter::once(&format).chain(&tile.amount).map(say))
}

fn facts_joined(facts: impl Iterator<Item = String>) -> String {
    facts.collect::<Vec<_>>().join(" · ") // i18n-skip
}

/// Names as a sentence lists them: "A", "A and B", "A, B and C".
fn names_joined(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => t::tui_read_names_and(&rest.join(", "), last), // i18n-skip
    }
}

/// Why a source has no results: that it did not answer, and the reason or how long it was given.
/// Offline, a source says it needs a connection: the engine's offline words are the reader's.
fn failed_reason(name: &str, failure: Option<&ReadingFailure>, seconds: Option<u32>) -> String {
    let reason = match (failure, seconds) {
        (_, Some(seconds)) => t::read_search_source_timeout(i64::from(seconds)),
        (Some(ReadingFailure::Offline), None) => return t::tui_read_source_offline(name),
        // These already name the source and say what went wrong with it.
        (
            Some(
                failure @ (ReadingFailure::SourceDown { .. }
                | ReadingFailure::Blocked { .. }
                | ReadingFailure::Parse { .. }),
            ),
            None,
        ) => return say(&failure.msg()),
        (Some(failure), None) => say(&failure.msg()),
        (None, None) => return t::read_search_source_failed(name),
    };
    format!("{} {}", t::read_search_source_failed(name), reason) // i18n-skip
}

/// How many rows reading's filters overlay has.
pub const READ_FILTER_ROWS: usize = 7;

/// The reading filters overlay's row labels, in order.
pub fn read_filter_labels() -> [String; READ_FILTER_ROWS] {
    [
        t::read_filter_format(),
        t::read_filter_origin(),
        t::read_filter_status(),
        t::read_filter_genres(),
        t::read_search_started(),
        t::lists_filter_sort(),
        t::read_search_sources_only(),
    ]
}

/// A sort as the shows' search words it.
fn sort_word(sort: &str) -> Option<String> {
    Some(match sort {
        "POPULARITY_DESC" => t::read_sort_popular(),
        "TRENDING_DESC" => t::read_sort_trending(),
        "SCORE_DESC" => t::read_sort_score(),
        "START_DATE_DESC" => t::read_sort_newest(),
        "TITLE_ROMAJI" => t::library_sort_title(),
        _ => return None,
    })
}

/// A choice's words, by its key.
fn choice_word(
    choices: &[kaeri_model::reading::ReadFilterChoice],
    key: &str,
) -> Option<String> {
    choices.iter().find(|c| c.key == key).map(|c| say(&c.label))
}

/// What a filter row shows: its value, or Any when unset.
pub fn read_filter_value(filter: &ReadSearchQuery, row: usize) -> String {
    let choices = kaeri_model::reading::read_filters();
    let value = match row {
        0 => filter.formats.first().and_then(|key| choice_word(&choices.formats, key)),
        1 => filter.origins.first().and_then(|key| choice_word(&choices.origins, key)),
        2 => filter.status.as_deref().and_then(|key| choice_word(&choices.statuses, key)),
        3 => filter.genres.first().cloned(),
        4 => filter
            .started
            .as_deref()
            .filter(|key| *key != "any")
            .and_then(|key| choice_word(&choices.started, key)),
        5 => filter.sort.as_deref().and_then(sort_word),
        _ => {
            return if filter.sources_only { t::value_on() } else { t::value_off() };
        }
    };
    value.unwrap_or_else(t::read_filter_any)
}

/// The filters set, as the line under Search's field lists them: empty when none is. Your
/// sources only has its own place on that line.
pub fn read_filter_summary(filter: &ReadSearchQuery) -> String {
    (0..READ_FILTER_ROWS - 1)
        .filter(|row| match row {
            0 => !filter.formats.is_empty(),
            1 => !filter.origins.is_empty(),
            2 => filter.status.is_some(),
            3 => !filter.genres.is_empty(),
            4 => filter.started.as_deref().is_some_and(|s| s != "any"),
            _ => filter.sort.is_some(),
        })
        .map(|row| read_filter_value(filter, row))
        .collect::<Vec<_>>()
        .join(" · ") // i18n-skip
}

/// Step a reading filter row: forward walks the choices and comes back round to Any.
pub fn cycle_read_filter(filter: &mut ReadSearchQuery, row: usize, forward: bool) {
    fn step(current: Option<&str>, keys: &[String], forward: bool) -> Option<String> {
        let at = current.and_then(|c| keys.iter().position(|k| k == c));
        let next = match (at, forward) {
            (None, true) => Some(0),
            (None, false) => keys.len().checked_sub(1),
            (Some(i), true) => (i + 1 < keys.len()).then_some(i + 1),
            (Some(0), false) => None,
            (Some(i), false) => Some(i - 1),
        };
        next.map(|i| keys[i].clone())
    }
    let choices = kaeri_model::reading::read_filters();
    let keys = |list: &[kaeri_model::reading::ReadFilterChoice]| -> Vec<String> {
        list.iter().map(|c| c.key.clone()).filter(|k| k != "any").collect()
    };
    match row {
        0 => {
            let next = step(
                filter.formats.first().map(String::as_str),
                &keys(&choices.formats),
                forward,
            );
            filter.formats = next.into_iter().collect();
        }
        1 => {
            let next = step(
                filter.origins.first().map(String::as_str),
                &keys(&choices.origins),
                forward,
            );
            filter.origins = next.into_iter().collect();
        }
        2 => filter.status = step(filter.status.as_deref(), &keys(&choices.statuses), forward),
        3 => {
            let next =
                step(filter.genres.first().map(String::as_str), &choices.genres, forward);
            filter.genres = next.into_iter().collect();
        }
        4 => filter.started = step(filter.started.as_deref(), &keys(&choices.started), forward),
        5 => filter.sort = step(filter.sort.as_deref(), &choices.sorts, forward),
        _ => filter.sources_only = !filter.sources_only,
    }
}

#[cfg(test)]
mod tests {
    use kaeri_core::{books::WorkId, media::Medium};
    use kaeri_model::{
        Msg, ReadingMsg,
        event::Event,
        medium::{AppEdition, KaeriIsFor, MediumView},
        reading::{
            ReadAnilistState, ReadHomeRow, ReadRowKind, ReadSourceGroup, ReadSourceJoin,
            ReadSourceSearch, ReadSourceState, ReadingCommand, ReadingEvent, WorkTile,
            kind_words,
        },
    };

    use super::*;
    use crate::app::fixtures::*;

    fn tile(id: &str, title: &str) -> WorkTile {
        WorkTile {
            work: WorkId::new(id),
            title: title.into(),
            medium: Medium::Manga,
            cover: None,
            format_label: None,
            progress: None,
            next: None,
            new_chapter: false,
            adult: false,
            facts: Vec::new(),
            demo: false,
            not_in_sources: false,
            origin: None,
            amount: None,
            started: None,
        }
    }

    fn group(id: &str, state: ReadSourceState, tiles: Vec<WorkTile>) -> ReadSourceGroup {
        ReadSourceGroup {
            source_id: id.into(),
            source_name: id.into(),
            kind: None,
            state,
            tiles,
            joined: 0,
        }
    }

    /// Reading alone, on Search, "lantern" asked and nothing back yet, on a clock the test moves.
    fn asked() -> App {
        let mut a = app();
        a.clock = crate::clock::Clock::fixed(1_793_000_000, crate::clock::Zone::utc());
        a.apply(Event::Medium(MediumView {
            answer: KaeriIsFor::Read,
            edition: AppEdition::Full,
            fixed: false,
            reading_available: true,
            ask: None,
            account: None,
        }));
        a.go_to_section(Section::Search);
        type_text(&mut a, "lantern");
        let task = a.handle(Action::Open, 10);
        let query = a.read_query();
        assert_eq!(task, Some(Command::Reading(ReadingCommand::Search { query })));
        a.take_commands();
        a
    }

    /// AniList's first page of results for the search on screen.
    fn anilist(a: &mut App, tiles: Vec<WorkTile>, has_more: bool, total: Option<u32>) {
        a.apply(Event::Reading(ReadingEvent::Search {
            query: a.read_query(),
            page: 0,
            tiles,
            has_more,
            other_scope: None,
            total,
        }));
    }

    /// Reading alone, on Search, "lantern" asked: two AniList results, one source with a hit and
    /// one that gave no answer. The sources' part comes first, as the engine sends it.
    fn searched() -> App {
        let mut a = asked();
        let query = a.read_query();
        a.apply(Event::Reading(ReadingEvent::SourceSearch(Box::new(ReadSourceSearch {
            query,
            anilist: ReadAnilistState::Answered,
            groups: vec![
                group(
                    "shelf",
                    ReadSourceState::Hits,
                    vec![tile("s:1", "Paper Lantern Recipes")],
                ),
                group("northwind", ReadSourceState::TimedOut { seconds: 15 }, Vec::new()),
            ],
            joins: Vec::new(),
            done: true,
        }))));
        anilist(
            &mut a,
            vec![tile("a:1", "Lantern Street Notes"), tile("a:2", "Lanterns Upriver")],
            false,
            Some(2),
        );
        a
    }

    fn work_facts(a: &App, title: &str) -> String {
        a.read_lines()
            .into_iter()
            .find_map(|line| match line {
                ReadLine::Work { title: t, facts, .. } if t == title => Some(facts),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no line for {title}"))
    }

    #[test]
    fn an_anilist_result_reads_format_then_origin() {
        let mut a = asked();
        let novel = WorkTile {
            facts: vec![
                kind_words(Some("novel"), Medium::Novel, Some("JP")),
                Msg::Reading(ReadingMsg::ReadingStatusReleasing),
            ],
            origin: Some(Msg::Reading(ReadingMsg::ReadingOriginJapan)),
            ..tile("a:1", "Lantern Street Notes")
        };
        let unknown = WorkTile {
            facts: vec![kind_words(Some("manga"), Medium::Manga, None)],
            ..tile("a:2", "Lanterns Upriver")
        };
        anilist(&mut a, vec![novel, unknown], false, Some(2));
        assert_eq!(work_facts(&a, "Lantern Street Notes"), "Light novel · Japan");
        assert_eq!(work_facts(&a, "Lanterns Upriver"), "Manga", "no origin part when unknown");
    }

    #[test]
    fn a_source_hit_says_how_much_the_source_holds_when_it_says() {
        let mut a = asked();
        let query = a.read_query();
        let volumes = WorkTile {
            amount: Some(Msg::Reading(ReadingMsg::ReadingVolumes { count: 2 })),
            ..tile("s:1", "Paper Lantern Recipes")
        };
        let file = WorkTile {
            amount: Some(Msg::Reading(ReadingMsg::ReadHitFilesFormat {
                count: 1,
                format: "CBZ".into(),
            })),
            ..tile("s:2", "Lantern Festival Sketchbook")
        };
        let silent = tile("s:3", "Lantern Lane Almanac");
        a.apply(Event::Reading(ReadingEvent::SourceSearch(Box::new(ReadSourceSearch {
            query,
            anilist: ReadAnilistState::Answered,
            groups: vec![group("shelf", ReadSourceState::Hits, vec![volumes, file, silent])],
            joins: Vec::new(),
            done: true,
        }))));
        assert_eq!(work_facts(&a, "Paper Lantern Recipes"), "Manga · 2 volumes");
        assert_eq!(work_facts(&a, "Lantern Festival Sketchbook"), "Manga · 1 file · CBZ");
        assert_eq!(work_facts(&a, "Lantern Lane Almanac"), "Manga", "the format alone");
    }

    #[test]
    fn a_group_shows_the_engines_kind_and_a_built_in_source_its_name_alone() {
        let mut a = asked();
        let query = a.read_query();
        let hits = || vec![tile("s:1", "Paper Lantern Recipes")];
        let kinds = [
            (Some(ReadSourceKind::Server), Some("Server")),
            (Some(ReadSourceKind::Folder), Some("Folder")),
            (Some(ReadSourceKind::Plugin), Some("Plugin")),
            (None, None),
        ];
        a.apply(Event::Reading(ReadingEvent::SourceSearch(Box::new(ReadSourceSearch {
            query,
            anilist: ReadAnilistState::Answered,
            groups: kinds
                .iter()
                .enumerate()
                .map(|(n, (kind, _))| ReadSourceGroup {
                    kind: *kind,
                    ..group(&format!("source-{n}"), ReadSourceState::Hits, hits())
                })
                .collect(),
            joins: Vec::new(),
            done: true,
        }))));
        let drawn: Vec<Option<String>> = a
            .read_lines()
            .into_iter()
            .filter_map(|line| match line {
                ReadLine::Group { kind, .. } => Some(kind),
                _ => None,
            })
            .collect();
        let want: Vec<Option<String>> =
            kinds.iter().map(|(_, word)| word.map(str::to_owned)).collect();
        assert_eq!(drawn, want);
    }

    #[test]
    fn an_unknown_count_draws_no_number() {
        let mut a = asked();
        let tiles = (0..30).map(|n| tile(&format!("a:{n}"), &format!("Lantern {n}"))).collect();
        anilist(&mut a, tiles, true, None);
        let lines = a.read_lines();
        assert_eq!(lines[0], ReadLine::AnilistHeading("Series from AniList".into()));
        assert!(lines.contains(&ReadLine::MoreAnilist { shown: "30 shown".into() }));
        let digits = |text: &str| text.chars().any(|c| c.is_ascii_digit());
        assert!(
            !lines.iter().any(|line| matches!(line, ReadLine::AnilistHeading(h) if digits(h)))
        );
    }

    #[test]
    fn a_known_count_is_in_the_heading() {
        let a = searched();
        assert_eq!(a.read_lines()[0], ReadLine::AnilistHeading("2 series from AniList".into()));
        assert!(
            !a.read_lines().iter().any(|line| matches!(line, ReadLine::MoreAnilist { .. }))
        );
    }

    #[test]
    fn a_joined_hit_shows_once_and_only_as_its_result() {
        let mut a = asked();
        let query = a.read_query();
        // The joins arrive first: the joined hit has already left its group.
        a.apply(Event::Reading(ReadingEvent::SourceSearch(Box::new(ReadSourceSearch {
            query,
            anilist: ReadAnilistState::Answered,
            groups: vec![ReadSourceGroup {
                joined: 1,
                ..group("shelf", ReadSourceState::Hits, Vec::new())
            }],
            joins: vec![ReadSourceJoin {
                work: WorkId::new("a:1"),
                source_ids: vec!["shelf".into()],
                source_names: vec!["Shelf".into()],
            }],
            done: true,
        }))));
        let works = |a: &App| {
            a.read_lines()
                .into_iter()
                .filter_map(|line| match line {
                    ReadLine::Work { work, joined, .. } => Some((work, joined)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert!(works(&a).is_empty(), "nothing drawn before AniList's results");
        assert!(a.read_lines().contains(&ReadLine::AnilistAsking { shown: false }));
        anilist(&mut a, vec![tile("a:1", "Lantern Street Notes")], false, Some(1));
        assert_eq!(works(&a), vec![(WorkId::new("a:1"), Some("In Shelf".into()))]);
    }

    #[test]
    fn search_lays_out_anilist_then_each_source() {
        let a = searched();
        let items = a.read_items();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].part(), Part::Anilist);
        assert!(
            matches!(&items[2], ReadLine::Work { part: Part::Sources, title, .. } if title == "Paper Lantern Recipes")
        );
        assert!(
            matches!(&items[3], ReadLine::Failed { retry: Retry::Source(id), .. } if id == "northwind")
        );
    }

    fn medium(a: &mut App, answer: KaeriIsFor) {
        a.apply(Event::Medium(MediumView {
            answer,
            edition: AppEdition::Full,
            fixed: false,
            reading_available: true,
            ask: None,
            account: None,
        }));
    }

    /// Reading's Home: Continue with one title, then a Trending row with one.
    fn home_view() -> ReadingHomeView {
        ReadingHomeView {
            continue_reading: vec![tile("a:5", "Saltwind Diary")],
            rows: vec![ReadHomeRow {
                id: "trending-manga".into(),
                kind: ReadRowKind::Trending,
                format: None,
                title: Msg::Reading(ReadingMsg::ReadHomeTrendingManga),
                side: None,
                tiles: vec![tile("a:6", "Iron Kite Brigade")],
                view_all: None,
                view_all_title: None,
                pending: false,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn both_puts_read_beside_home_and_w_moves_between_them() {
        let mut a = app();
        medium(&mut a, KaeriIsFor::Both);
        assert_eq!(
            a.rail_sections()[..5],
            [
                Section::Home,
                Section::Calendar,
                Section::Seasonal,
                Section::Read,
                Section::Search
            ]
        );
        a.go_to_section(Section::Home);
        a.take_commands();
        assert!(!a.reading_here() && a.shows_side_here());
        let task = a.handle(Action::SwitchMedium, 10);
        assert_eq!(task, Some(Command::Reading(ReadingCommand::LoadHome { refresh: false })));
        assert_eq!(a.nav.section(), Section::Read);
        assert!(a.reading_here() && !a.shows_side_here());
        let task = a.handle(Action::SwitchMedium, 10);
        assert_eq!(task, Some(Command::LoadHome { refresh: false }));
        assert_eq!(a.nav.section(), Section::Home);
        assert!(!a.reading_here());
    }

    #[test]
    fn both_turns_search_between_the_shows_and_reading() {
        let mut a = app();
        medium(&mut a, KaeriIsFor::Both);
        a.go_to_section(Section::Read);
        a.handle(Action::FocusSearch, 10);
        assert!(a.reading_here(), "Search opened from Read is reading's");
        // Out of the field, where W is a letter.
        a.nav.focus_rail();
        a.handle(Action::SwitchMedium, 10);
        assert!(!a.reading_here() && a.shows_side_here());
        a.go_to_section(Section::Home);
        a.handle(Action::FocusSearch, 10);
        assert!(!a.reading_here(), "Search opened from Home is the shows'");
    }

    #[test]
    fn reading_alone_leaves_the_shows_only_sections_out_of_the_rail() {
        let mut a = app();
        a.go_to_section(Section::Calendar);
        medium(&mut a, KaeriIsFor::Read);
        assert_eq!(a.nav.section(), Section::Home, "Calendar gave way to Home");
        let rail = a.rail_sections();
        for gone in [Section::Calendar, Section::Seasonal, Section::Together, Section::Read] {
            assert!(!rail.contains(&gone), "{gone:?}");
        }
        assert_eq!(a.handle(Action::JumpSection(2), 10), None);
        assert_eq!(a.nav.section(), Section::Home, "Calendar's key goes nowhere");
        a.nav.focus_rail();
        a.handle(Action::Down, 10);
        assert_eq!(a.nav.section(), Section::Search, "the rail walks past what it left out");
    }

    #[test]
    fn the_switch_says_where_it_works_when_kaeri_is_for_one_thing() {
        let mut a = app();
        medium(&mut a, KaeriIsFor::Watch);
        a.go_to_section(Section::Home);
        a.take_commands();
        assert_eq!(a.handle(Action::SwitchMedium, 10), None);
        assert!(!a.toasts.is_empty());
        assert!(!a.reading_here());
    }

    #[test]
    fn leaving_both_gives_home_back_to_the_shows() {
        let mut a = app();
        medium(&mut a, KaeriIsFor::Both);
        a.go_to_section(Section::Home);
        a.handle(Action::SwitchMedium, 10);
        assert!(a.reading_here());
        medium(&mut a, KaeriIsFor::Watch);
        assert!(!a.reading_here());
        medium(&mut a, KaeriIsFor::Both);
        assert!(!a.reading_here(), "Both starts on the shows' again");
    }

    #[test]
    fn a_puts_an_anilist_result_on_planning_but_not_a_source_hit() {
        let mut a = searched();
        a.handle(Action::Down, 10);
        let task = a.handle(Action::SetListStatus, 10);
        assert_eq!(
            task,
            Some(Command::Reading(ReadingCommand::SetStatus {
                work: WorkId::new("a:1"),
                status: LibrarySegmentRead::Planning,
            }))
        );
        // The source's hit, after AniList's two results.
        a.handle(Action::Down, 10);
        a.handle(Action::Down, 10);
        assert!(matches!(a.read_selected(), Some(ReadLine::Work { part: Part::Sources, .. })));
        a.toasts.clear();
        assert_eq!(a.handle(Action::SetListStatus, 10), None);
        assert!(!a.toasts.is_empty(), "says where it works");
    }

    #[test]
    fn a_puts_a_discovery_row_title_on_planning_but_not_a_continue_one() {
        let mut a = app();
        medium(&mut a, KaeriIsFor::Read);
        a.go_to_section(Section::Home);
        a.apply(Event::Reading(ReadingEvent::Home(Box::new(home_view()))));
        a.take_commands();
        a.nav.focus_stage();
        assert_eq!(a.handle(Action::SetListStatus, 10), None, "Continue's title");
        a.handle(Action::Down, 10);
        assert_eq!(
            a.handle(Action::SetListStatus, 10),
            Some(Command::Reading(ReadingCommand::SetStatus {
                work: WorkId::new("a:6"),
                status: LibrarySegmentRead::Planning,
            }))
        );
    }

    /// The sources' part as the engine sends it, for the search on screen.
    fn sources(a: &mut App, anilist: ReadAnilistState, groups: Vec<ReadSourceGroup>) {
        let query = a.read_query();
        a.apply(Event::Reading(ReadingEvent::SourceSearch(Box::new(ReadSourceSearch {
            query,
            anilist,
            groups,
            joins: Vec::new(),
            done: true,
        }))));
    }

    fn wait(a: &mut App, ms: u64) {
        a.clock.advance(Duration::from_millis(ms));
    }

    #[test]
    fn a_part_that_lands_before_its_placeholder_shows_at_once() {
        let mut a = asked();
        wait(&mut a, 500);
        assert!(a.read_lines().contains(&ReadLine::AnilistAsking { shown: false }));
        anilist(&mut a, vec![tile("a:1", "Lantern Street Notes")], false, Some(1));
        assert!(!a.read_lines().iter().any(|l| matches!(l, ReadLine::AnilistAsking { .. })));
    }

    #[test]
    fn a_placeholder_once_drawn_holds_for_a_third_of_a_second() {
        let mut a = asked();
        wait(&mut a, 650);
        assert!(a.read_lines().contains(&ReadLine::AnilistAsking { shown: true }));
        wait(&mut a, 50);
        anilist(&mut a, vec![tile("a:1", "Lantern Street Notes")], false, Some(1));
        assert!(
            a.read_lines().contains(&ReadLine::AnilistAsking { shown: true }),
            "still held at 700 ms"
        );
        wait(&mut a, 200);
        assert!(!a.read_lines().iter().any(|l| matches!(l, ReadLine::AnilistAsking { .. })));
        assert_eq!(work_facts(&a, "Lantern Street Notes"), "");
    }

    #[test]
    fn asking_one_source_again_leaves_the_other_parts_waiting_as_they_were() {
        let mut a = asked();
        sources(
            &mut a,
            ReadAnilistState::Asking,
            vec![
                group("northwind", ReadSourceState::TimedOut { seconds: 15 }, Vec::new()),
                group("shelf", ReadSourceState::Asking, Vec::new()),
            ],
        );
        wait(&mut a, 700);
        let spinners = |a: &App| {
            a.read_lines()
                .into_iter()
                .filter(|l| {
                    matches!(
                        l,
                        ReadLine::AnilistAsking { shown: true }
                            | ReadLine::Asking { shown: true, .. }
                    )
                })
                .count()
        };
        assert_eq!(spinners(&a), 2);
        a.nav.focus_stage();
        a.search_in_results = true;
        let task = a.handle(Action::Refresh, 10);
        assert!(matches!(
            task,
            Some(Command::Reading(ReadingCommand::RetrySourceSearch { .. }))
        ));
        sources(
            &mut a,
            ReadAnilistState::Asking,
            vec![
                group("northwind", ReadSourceState::Asking, Vec::new()),
                group("shelf", ReadSourceState::Asking, Vec::new()),
            ],
        );
        assert_eq!(spinners(&a), 2, "AniList and the shelf keep theirs; Northwind waits anew");
    }

    #[test]
    fn the_cursor_stays_on_its_line_when_results_land_above_it() {
        let mut a = asked();
        sources(
            &mut a,
            ReadAnilistState::Asking,
            vec![group(
                "shelf",
                ReadSourceState::Hits,
                vec![tile("s:1", "Paper Lantern Recipes")],
            )],
        );
        a.handle(Action::Down, 10);
        assert!(
            matches!(a.read_selected(), Some(ReadLine::Work { title, .. }) if title == "Paper Lantern Recipes")
        );
        anilist(
            &mut a,
            vec![tile("a:1", "Lantern Street Notes"), tile("a:2", "Lanterns Upriver")],
            false,
            Some(2),
        );
        assert!(
            matches!(a.read_selected(), Some(ReadLine::Work { title, .. }) if title == "Paper Lantern Recipes"),
            "{:?}",
            a.read_selected()
        );
    }

    #[test]
    fn anilists_failure_is_a_line_r_and_enter_ask_again() {
        let mut a = asked();
        sources(
            &mut a,
            ReadAnilistState::Failed,
            vec![group("northwind", ReadSourceState::TimedOut { seconds: 15 }, Vec::new())],
        );
        let items = a.read_items();
        assert!(matches!(items[0], ReadLine::Failed { retry: Retry::Anilist, .. }));
        a.nav.focus_stage();
        a.search_in_results = true;
        // AniList's is under the cursor, and first on the screen.
        let task = a.handle(Action::Refresh, 10);
        assert!(matches!(task, Some(Command::Reading(ReadingCommand::Search { .. }))));
        assert!(a.search_in_results);
        sources(
            &mut a,
            ReadAnilistState::Failed,
            vec![group("northwind", ReadSourceState::TimedOut { seconds: 15 }, Vec::new())],
        );
        // On Northwind's line, `r` asks Northwind.
        a.handle(Action::Down, 10);
        let task = a.handle(Action::Refresh, 10);
        assert!(matches!(
            task,
            Some(Command::Reading(ReadingCommand::RetrySourceSearch { ref source_id, .. }))
                if source_id == "northwind"
        ));
        a.handle(Action::Top, 10);
        let task = a.handle(Action::Open, 10);
        assert!(matches!(task, Some(Command::Reading(ReadingCommand::Search { .. }))));
    }

    #[test]
    fn an_offline_source_says_it_needs_a_connection() {
        let mut a = asked();
        sources(
            &mut a,
            ReadAnilistState::Answered,
            vec![group(
                "northwind",
                ReadSourceState::Failed { failure: ReadingFailure::Offline },
                Vec::new(),
            )],
        );
        let reason = a.read_lines().into_iter().find_map(|l| match l {
            ReadLine::Failed { reason, retry: Retry::Source(_) } => Some(reason),
            _ => None,
        });
        assert_eq!(reason.as_deref(), Some("northwind needs a connection."));
    }

    #[test]
    fn your_sources_only_with_nothing_found_says_so_and_what_to_try() {
        let mut a = asked();
        a.reading.filter.sources_only = true;
        a.handle(Action::Open, 10);
        sources(
            &mut a,
            ReadAnilistState::Skipped,
            vec![
                group("shelf", ReadSourceState::Nothing, Vec::new()),
                group("northwind", ReadSourceState::Nothing, Vec::new()),
            ],
        );
        let lines = a.read_lines();
        assert_eq!(lines[0], ReadLine::Said("Nothing found in your sources".into()));
        assert_eq!(lines.len(), 2, "{lines:?}");
    }

    #[test]
    fn nothing_found_names_the_sources_as_a_sentence_does() {
        assert_eq!(names_joined(&["Shelf"]), "Shelf");
        assert_eq!(names_joined(&["Shelf", "Northwind"]), "Shelf and Northwind");
        assert_eq!(names_joined(&["A", "B", "C"]), "A, B and C");
    }

    #[test]
    fn s_with_no_words_says_to_type_a_title_and_changes_nothing() {
        let mut a = app();
        medium(&mut a, KaeriIsFor::Read);
        a.go_to_section(Section::Search);
        a.take_commands();
        a.nav.focus_rail();
        a.toasts.clear();
        assert_eq!(a.handle(Action::ShowSources, 10), None);
        assert!(!a.reading.sources_only());
        assert_eq!(a.toasts.len(), 1);
    }

    #[test]
    fn tab_with_one_part_keeps_the_rail_as_it_was() {
        let mut a = asked();
        sources(
            &mut a,
            ReadAnilistState::Answered,
            vec![group(
                "shelf",
                ReadSourceState::Hits,
                vec![tile("s:1", "Paper Lantern Recipes")],
            )],
        );
        anilist(&mut a, Vec::new(), false, Some(0));
        a.handle(Action::Down, 10);
        let width = a.nav.rail_width();
        assert_eq!(a.handle(Action::ToggleRail, 10), None);
        assert_eq!(a.nav.rail_width(), width);
    }

    #[test]
    fn view_all_names_its_row_over_anilists_results() {
        let mut a = app();
        medium(&mut a, KaeriIsFor::Read);
        a.go_to_section(Section::Home);
        let mut view = home_view();
        view.rows[0].view_all = Some(ReadSearchQuery::default());
        view.rows[0].view_all_title = Some(Msg::Reading(ReadingMsg::ReadViewAllTrendingManga));
        a.apply(Event::Reading(ReadingEvent::Home(Box::new(view))));
        a.nav.focus_stage();
        a.handle(Action::Bottom, 10);
        a.handle(Action::Open, 10);
        assert_eq!(a.nav.section(), Section::Search);
        let lines = a.read_lines();
        let ReadLine::AnilistHeading(heading) = &lines[0] else { panic!("{lines:?}") };
        assert!(heading.ends_with(" · Series from AniList"), "{heading}");
        anilist(&mut a, vec![tile("a:1", "Lantern Street Notes")], false, Some(1));
        let lines = a.read_lines();
        let ReadLine::AnilistHeading(heading) = &lines[0] else { panic!("{lines:?}") };
        assert!(heading.ends_with(" · 1 series from AniList"), "{heading}");
    }

    #[test]
    fn a_format_rows_caption_is_origin_then_status_or_when_it_started() {
        let japan = Some(Msg::Reading(ReadingMsg::ReadingOriginJapan));
        let titled = WorkTile {
            facts: vec![
                kind_words(Some("manga"), Medium::Manga, Some("JP")),
                Msg::Reading(ReadingMsg::ReadingStatusReleasing),
            ],
            origin: japan,
            ..tile("a:1", "Lantern Street Notes")
        };
        let row = |kind| ReadHomeRow {
            id: "row".into(),
            kind,
            format: Some(kaeri_model::reading::ReadRowFormat::Manga),
            title: Msg::Reading(ReadingMsg::ReadHomeTrendingManga),
            side: None,
            tiles: Vec::new(),
            view_all: None,
            view_all_title: None,
            pending: false,
        };
        assert_eq!(
            home_caption(&titled, Some(&row(ReadRowKind::Trending))),
            "Japan · Releasing"
        );
        assert_eq!(home_caption(&titled, None), "Manga · Releasing", "a mixed row's");
    }

    #[test]
    fn a_stale_answer_is_dropped() {
        let mut a = searched();
        let mut other = a.read_query();
        other.text = "kite".into();
        a.apply(Event::Reading(ReadingEvent::Search {
            query: other,
            page: 0,
            tiles: Vec::new(),
            has_more: false,
            other_scope: None,
            total: Some(0),
        }));
        assert_eq!(a.reading.anilist.len(), 2);
    }

    #[test]
    fn tab_moves_between_anilist_and_the_sources() {
        let mut a = searched();
        a.handle(Action::Down, 10);
        assert!(a.search_in_results);
        a.handle(Action::ToggleRail, 10);
        assert_eq!(a.cursor(), 2);
        a.handle(Action::ToggleRail, 10);
        assert_eq!(a.cursor(), 0);
    }

    #[test]
    fn s_turns_your_sources_only_on_and_asks_again() {
        let mut a = searched();
        a.handle(Action::Down, 10);
        let task = a.handle(Action::ShowSources, 10);
        assert!(a.reading.sources_only());
        let Some(Command::Reading(ReadingCommand::Search { query })) = task else {
            panic!("asked again: {task:?}");
        };
        assert!(query.sources_only);
        assert!(a.search_in_results, "the keyboard stays in the results");
        a.handle(Action::ShowSources, 10);
        assert!(!a.reading.sources_only());
    }

    #[test]
    fn r_asks_the_source_that_gave_no_answer_again() {
        let mut a = searched();
        a.handle(Action::Down, 10);
        let task = a.handle(Action::Refresh, 10);
        assert!(matches!(
            task,
            Some(Command::Reading(ReadingCommand::RetrySourceSearch { ref source_id, .. }))
                if source_id == "northwind"
        ));
        // Enter on the failed line asks it too.
        a.handle(Action::Bottom, 10);
        let task = a.handle(Action::Open, 10);
        assert!(matches!(
            task,
            Some(Command::Reading(ReadingCommand::RetrySourceSearch { .. }))
        ));
    }

    #[test]
    fn enter_on_a_result_asks_for_the_series_and_says_where_it_reads() {
        let mut a = searched();
        // The results answer the query as typed, so Enter in the field opens the first.
        let task = a.handle(Action::Open, 10);
        assert_eq!(
            task,
            Some(Command::Reading(ReadingCommand::LoadSeries { work: WorkId::new("a:1") }))
        );
        assert!(!a.toasts.is_empty());
    }

    #[test]
    fn title_keys_say_they_work_elsewhere_rather_than_acting_on_a_hidden_show() {
        let mut a = searched();
        a.handle(Action::Down, 10);
        assert_eq!(a.handle(Action::ShowEpisodes, 10), None);
        assert!(a.selected_entry().is_none());
    }
}
