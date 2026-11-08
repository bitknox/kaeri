//! Reading's Home and Search, while this computer is set to reading alone.
//!
//! The terminal has no reader, so both are text: Home's shelves as headed lines, and Search as
//! AniList's results followed by the person's own sources, one group each. The engine decides what
//! is found and how each group stands; this keeps the newest answer for the query on screen and
//! lays it out as the lines the screen draws and the cursor walks.

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use kaeri_core::books::WorkId;
use kaeri_i18n::t;
use kaeri_model::{
    book_sources::{BookSourceCommand, BookSourcesView},
    command::Command,
    reading::{
        ReadAnilistState, ReadHomeEnd, ReadSearchQuery, ReadSourceKind, ReadSourceSearch,
        ReadSourceState, ReadingAbout, ReadingCommand, ReadingEvent, ReadingFailure,
        ReadingHomeView, WorkTile,
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
pub const PLACEHOLDER_AFTER: std::time::Duration = std::time::Duration::from_millis(600);

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
    /// When Home was asked for, so its loading line waits before it shows.
    pub home_asked: Option<Instant>,
    /// The filters the next search carries, its words aside. Sticky across queries, as the
    /// shows' filters are.
    pub filter: ReadSearchQuery,
    /// The search the results on screen answer, once one has been asked.
    pub searched: Option<ReadSearchQuery>,
    /// When it was asked, so placeholders and spinners wait before they show.
    pub asked: Option<Instant>,
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
    /// A part that failed to load, said in the alert role.
    Alert(String),
    /// Two placeholder lines held for a shelf still on its way.
    Placeholder,
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
        part: Part,
    },
    /// View all under a Home shelf: Search with the shelf's filters.
    ViewAll {
        query: Box<ReadSearchQuery>,
        part: Part,
    },
    /// AniList's heading, with its count once it is known.
    AnilistHeading(String),
    /// AniList is being asked.
    AnilistAsking,
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
    /// A source still asked.
    Asking {
        name: String,
    },
    /// A source that failed or gave no answer in time, with the key that asks it again.
    SourceFailed {
        source_id: String,
        reason: String,
    },
    ShowMore {
        source_id: String,
        n: usize,
    },
}

impl ReadLine {
    /// Whether the cursor can land on it.
    pub fn selectable(&self) -> bool {
        matches!(
            self,
            Self::Work { .. }
                | Self::ViewAll { .. }
                | Self::MoreAnilist { .. }
                | Self::SourceFailed { .. }
                | Self::ShowMore { .. }
        )
    }

    /// The part of the screen a selectable line is in.
    pub fn part(&self) -> Part {
        match self {
            Self::Work { part, .. } | Self::ViewAll { part, .. } => *part,
            Self::MoreAnilist { .. } => Part::Anilist,
            _ => Part::Sources,
        }
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

    /// How many sources a search asks: the groups of the one on screen, else the sources on.
    pub fn source_count(&self) -> usize {
        self.sources.as_ref().map_or(self.on, |sources| sources.groups.len())
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
    /// Whether reading's Home or Search is the view on top.
    pub fn reading_here(&self) -> bool {
        self.reading_only()
            && matches!(
                self.nav.current(),
                StageView::Section(Section::Home) | StageView::Section(Section::Search)
            )
    }

    /// Whether the section on screen is one reading draws, under whatever is pushed over it.
    pub(super) fn reading_section(&self) -> bool {
        self.reading_only() && matches!(self.nav.section(), Section::Home | Section::Search)
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
        match self.nav.section() {
            Section::Home => {
                self.reading.home_asked = Some(self.clock.now_instant());
                Some(Command::Reading(ReadingCommand::LoadHome { refresh: false }))
            }
            _ if read_query_blank(&self.read_query()) => {
                self.reading.searched = None;
                self.reading.sources = None;
                None
            }
            _ => Some(self.run_read_search()),
        }
    }

    /// Ask the search the field and the filters make, from the top of its results.
    pub(super) fn run_read_search(&mut self) -> Command {
        let query = self.read_query();
        let reading = &mut self.reading;
        reading.searched = Some(query.clone());
        reading.asked = Some(self.clock.now_instant());
        reading.anilist.clear();
        reading.anilist_state = AnilistState::Asking;
        reading.page = 0;
        reading.more = false;
        reading.total = None;
        reading.loading_more = false;
        reading.sources = None;
        reading.expanded.clear();
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
        match event {
            ReadingEvent::Home(view) => {
                self.reading.home = Some(*view);
                self.fit_reading_cursor();
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
                reading.page = reading.page.max(page);
                reading.more = has_more;
                reading.total = total;
                reading.loading_more = false;
                self.fit_reading_cursor();
            }
            ReadingEvent::SourceSearch(sources) => {
                if self.reading.searched.as_ref() != Some(&sources.query) {
                    return;
                }
                if sources.anilist == ReadAnilistState::Failed {
                    self.reading.anilist_state = AnilistState::Failed;
                }
                self.reading.sources = Some(*sources);
                self.fit_reading_cursor();
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
                }
            }
            // Opening a series is the one other thing asked from here.
            ReadingEvent::Failed { about: ReadingAbout::Series, failure }
                if self.reading_only() =>
            {
                self.push_toast(alert(say(&failure.msg())));
            }
            _ => {}
        }
    }

    /// The Sources view, for how many sources there are.
    pub(super) fn apply_book_sources(&mut self, view: &BookSourcesView) {
        self.reading.take_sources(view);
    }

    /// Keep the cursor on a line that exists, after the lines changed under it.
    fn fit_reading_cursor(&mut self) {
        if !self.reading_section() {
            return;
        }
        let len = self.read_items().len();
        let cursor = self.cursors.section_mut(self.nav.section());
        cursor.selected = cursor.selected.min(len.saturating_sub(1));
        cursor.offset = cursor.offset.min(cursor.selected);
    }

    /// Reading's lines for the section on screen.
    pub fn read_lines(&self) -> Vec<ReadLine> {
        match self.nav.section() {
            Section::Home => self.read_home_lines(),
            _ => self.read_search_lines(),
        }
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
        if view.offline {
            let time = view
                .offline_since
                .and_then(|at| self.clock.local(at))
                .map(|at| at.format("%H:%M").to_string());
            if let Some(time) = time {
                lines.push(ReadLine::Quiet(t::home_offline(&time)));
                lines.push(ReadLine::Gap);
            }
        } else if view.failed {
            lines.push(ReadLine::Alert(t::home_failed()));
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
            lines.extend(tiles.iter().take(SHELF_TILES).map(|tile| home_tile(tile, shelf)));
            shelf += 1;
        }
        for row in &view.rows {
            if row.tiles.is_empty() && !row.pending {
                continue;
            }
            if !lines.is_empty() && lines.last() != Some(&ReadLine::Gap) {
                lines.push(ReadLine::Gap);
            }
            lines.push(ReadLine::Shelf {
                title: say(&row.title),
                side: row.side.as_ref().map(say),
            });
            if row.tiles.is_empty() {
                lines.push(ReadLine::Placeholder);
                shelf += 1;
                continue;
            }
            lines.extend(row.tiles.iter().take(SHELF_TILES).map(|tile| home_tile(tile, shelf)));
            if let Some(query) = &row.view_all {
                lines.push(ReadLine::ViewAll {
                    query: Box::new(query.clone()),
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
            lines.push(ReadLine::Quiet(t::home_read_no_sources()));
        }
        lines
    }

    /// Search's lines: AniList's results, then From your sources.
    fn read_search_lines(&self) -> Vec<ReadLine> {
        let mut lines = Vec::new();
        let reading = &self.reading;
        let Some(query) = &reading.searched else { return lines };
        let sources = reading.sources.as_ref();
        let joins: HashMap<&WorkId, &kaeri_model::reading::ReadSourceJoin> = sources
            .map(|s| s.joins.iter().map(|join| (&join.work, join)).collect())
            .unwrap_or_default();

        let groups_found = sources.is_some_and(|s| {
            s.groups.iter().any(|g| {
                !g.tiles.is_empty()
                    || matches!(
                        g.state,
                        ReadSourceState::Asking
                            | ReadSourceState::Failed { .. }
                            | ReadSourceState::TimedOut { .. }
                    )
            })
        });
        let settled_empty = reading.anilist_state == AnilistState::Answered
            && reading.anilist.is_empty()
            && sources.is_some_and(|s| s.done)
            && !groups_found;
        if settled_empty && !query.sources_only && !query.text.is_empty() {
            lines.push(ReadLine::Quiet(t::read_search_nothing(&query.text)));
            lines.push(ReadLine::Quiet(t::read_search_nothing_line()));
            lines.push(ReadLine::Gap);
            lines.push(ReadLine::Quiet(t::read_search_sources_nothing()));
            return lines;
        }

        if !query.sources_only {
            match reading.anilist_state {
                AnilistState::Asking => {
                    lines.push(ReadLine::AnilistHeading(t::read_search_total_unknown()));
                    lines.push(ReadLine::AnilistAsking);
                }
                AnilistState::Failed => {
                    lines.push(ReadLine::Alert(t::read_search_anilist_failed()));
                    lines.push(ReadLine::Quiet(t::read_search_anilist_failed_line()));
                }
                AnilistState::Answered if reading.anilist.is_empty() => {
                    lines.push(ReadLine::AnilistHeading(anilist_heading(reading.total)));
                    lines.push(ReadLine::Quiet(t::read_search_anilist_nothing()));
                }
                AnilistState::Answered => {
                    lines.push(ReadLine::AnilistHeading(anilist_heading(reading.total)));
                    for tile in &reading.anilist {
                        let joined = joins.get(&tile.work).and_then(|join| {
                            match join.source_names.len() {
                                0 => None,
                                1 => Some(t::tui_read_in_source(&join.source_names[0])),
                                n => Some(t::tui_read_in_sources(n as i64)),
                            }
                        });
                        lines.push(ReadLine::Work {
                            work: tile.work.clone(),
                            title: tile.title.clone(),
                            facts: anilist_facts(tile),
                            joined,
                            fresh: false,
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
            .filter(|g| match g.state {
                ReadSourceState::Nothing => false,
                ReadSourceState::Hits => !g.tiles.is_empty(),
                _ => true,
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
            match &group.state {
                ReadSourceState::Asking => {
                    lines.push(ReadLine::Asking { name: group.source_name.clone() })
                }
                ReadSourceState::Failed { failure } => lines.push(ReadLine::SourceFailed {
                    source_id: group.source_id.clone(),
                    reason: failed_reason(&group.source_name, Some(failure), None),
                }),
                ReadSourceState::TimedOut { seconds } => lines.push(ReadLine::SourceFailed {
                    source_id: group.source_id.clone(),
                    reason: failed_reason(&group.source_name, None, Some(*seconds)),
                }),
                ReadSourceState::Hits | ReadSourceState::Nothing => {
                    let open = reading.expanded.contains(&group.source_id);
                    let take = if open { group.tiles.len() } else { GROUP_HITS };
                    for tile in group.tiles.iter().take(take) {
                        lines.push(ReadLine::Work {
                            work: tile.work.clone(),
                            title: tile.title.clone(),
                            facts: hit_facts(tile),
                            joined: None,
                            fresh: false,
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
                g.state == ReadSourceState::Nothing
                    || (g.state == ReadSourceState::Hits && g.tiles.is_empty() && g.joined == 0)
            })
            .map(|g| g.source_name.as_str())
            .collect();
        if !nothing.is_empty() {
            lines.push(ReadLine::Gap);
            lines.push(ReadLine::Quiet(t::read_search_source_nothing(&nothing.join(", "))));
        }
        lines
    }

    /// Whether the line under the cursor is in a part still on its way for long enough that its
    /// placeholder shows.
    pub fn read_waited(&self) -> bool {
        let asked = match self.nav.section() {
            Section::Home => self.reading.home_asked,
            _ => self.reading.asked,
        };
        asked.is_none_or(|at| {
            self.clock.now_instant().saturating_duration_since(at) >= PLACEHOLDER_AFTER
        })
    }

    /// A key on reading's Home or Search, ahead of the shows' meaning of it. `None` lets it
    /// through.
    pub(super) fn reading_key(&mut self, action: Action) -> Option<Option<Command>> {
        if !self.reading_here() {
            return None;
        }
        let on_stage = self.nav.focus() == Focus::Stage;
        let search = self.nav.section() == Section::Search;
        match action {
            Action::Open if on_stage => Some(self.open_read_line()),
            Action::ShowSources if search => Some(self.toggle_sources_only()),
            Action::ToggleRail if on_stage && self.next_read_part() => Some(None),
            Action::Refresh => Some(self.refresh_reading()),
            Action::Filter if search => {
                self.open_search_filters();
                Some(None)
            }
            Action::Surprise => {
                self.works_elsewhere(action);
                Some(None)
            }
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
                self.push_toast(info(t::tui_medium_read_line()));
                Some(Command::Reading(ReadingCommand::LoadSeries { work }))
            }
            ReadLine::ViewAll { query, .. } => {
                let query = *query;
                self.set_text(Field::Search, query.text.clone());
                self.reading.filter = ReadSearchQuery { text: String::new(), ..query };
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
            ReadLine::SourceFailed { source_id, .. } => self.retry_source(source_id),
            _ => None,
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

    /// Ask one source again; only its group goes back to asking.
    fn retry_source(&mut self, source_id: String) -> Option<Command> {
        let query = self.reading.searched.clone()?;
        self.reading.asked = Some(self.clock.now_instant());
        Some(Command::Reading(ReadingCommand::RetrySourceSearch { query, source_id }))
    }

    /// `r`: the failed source under the cursor, else the first that failed, else everything
    /// again.
    fn refresh_reading(&mut self) -> Option<Command> {
        if self.nav.section() == Section::Home {
            self.reading.home_asked = Some(self.clock.now_instant());
            self.commands.push(Command::BookSources(BookSourceCommand::Load));
            return Some(Command::Reading(ReadingCommand::LoadHome { refresh: true }));
        }
        if let Some(ReadLine::SourceFailed { source_id, .. }) = self.read_selected() {
            return self.retry_source(source_id);
        }
        let failed = self.read_lines().into_iter().find_map(|line| match line {
            ReadLine::SourceFailed { source_id, .. } => Some(source_id),
            _ => None,
        });
        if let Some(source_id) = failed {
            return self.retry_source(source_id);
        }
        if read_query_blank(&self.read_query()) {
            return None;
        }
        Some(self.run_read_search())
    }

    /// `s` on Search: Your sources only, on or off, and the search asked again with it.
    pub(super) fn toggle_sources_only(&mut self) -> Option<Command> {
        let on = !self.reading.filter.sources_only;
        self.reading.filter.sources_only = on;
        self.push_toast(info(if on {
            t::tui_read_sources_only_on()
        } else {
            t::tui_read_sources_only_off()
        }));
        if read_query_blank(&self.read_query()) || self.text(Field::Search).trim().is_empty() {
            self.reading.searched = None;
            self.reading.sources = None;
            return None;
        }
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
            t::tui_read_status(self.reading.source_count() as i64)
        } else {
            t::tui_read_status_short()
        }
    }
}

/// A Home tile as a line: Not found in your sources when a source was asked and had nothing,
/// else what the engine says is next, else its facts.
fn home_tile(tile: &WorkTile, shelf: usize) -> ReadLine {
    let facts = if tile.not_in_sources {
        t::home_not_found()
    } else if let Some(next) = &tile.next {
        say(next)
    } else {
        fact_line(tile)
    };
    ReadLine::Work {
        work: tile.work.clone(),
        title: tile.title.clone(),
        facts,
        joined: None,
        fresh: tile.new_chapter && !tile.not_in_sources,
        part: Part::Shelf(shelf),
    }
}

/// AniList's heading: its count once the engine knows it exactly, else no number at all.
fn anilist_heading(total: Option<u32>) -> String {
    match total {
        Some(total) => t::tui_read_anilist_count(i64::from(total)),
        None => t::read_search_total_unknown(),
    }
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

/// A source's hit: what the engine says of it, then how much of it the source holds ("2
/// volumes", "1 file · CBZ"), when the source says.
fn hit_facts(tile: &WorkTile) -> String {
    facts_joined(tile.facts.iter().chain(&tile.amount).map(say))
}

fn facts_joined(facts: impl Iterator<Item = String>) -> String {
    facts.collect::<Vec<_>>().join(" · ") // i18n-skip
}

/// Why a source has no results: that it did not answer, and the reason or how long it was given.
fn failed_reason(name: &str, failure: Option<&ReadingFailure>, seconds: Option<u32>) -> String {
    let reason = match (failure, seconds) {
        (_, Some(seconds)) => t::read_search_source_timeout(i64::from(seconds)),
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
            ReadAnilistState, ReadSourceGroup, ReadSourceJoin, ReadSourceSearch,
            ReadSourceState, ReadingCommand, ReadingEvent, WorkTile, kind_words,
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

    /// Reading alone, on Search, "lantern" asked and nothing back yet.
    fn asked() -> App {
        let mut a = app();
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
        assert_eq!(work_facts(&a, "Paper Lantern Recipes"), "2 volumes");
        assert_eq!(work_facts(&a, "Lantern Festival Sketchbook"), "1 file · CBZ");
        assert_eq!(work_facts(&a, "Lantern Lane Almanac"), "");
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
        assert_eq!(a.read_lines()[0], ReadLine::AnilistHeading("AniList · 2 series".into()));
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
        assert!(a.read_lines().contains(&ReadLine::AnilistAsking));
        anilist(&mut a, vec![tile("a:1", "Lantern Street Notes")], false, Some(1));
        assert_eq!(works(&a), vec![(WorkId::new("a:1"), Some("in Shelf".into()))]);
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
            matches!(&items[3], ReadLine::SourceFailed { source_id, .. } if source_id == "northwind")
        );
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
