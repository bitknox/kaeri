//! Search, Reading side: a field over AniList's covers, and with words, a row of covers from each
//! of the person's sources under them. Under the field the filters: Format, Origin, Status,
//! Genres and Started, each a menu button, then a rule and Your sources only, which shows once
//! there are words; the order is at the end. With nothing typed it browses AniList, and a line
//! says what it shows.
//!
//! With words, AniList's first page is a grid of posters. From your sources follows, a row per
//! source in Your sources order: placeholders while it is asked, its covers once it answers, or
//! why it didn't and Try again for that source alone. Sources that found nothing share one line.
//! More from AniList then brings AniList's next page under them, so the sources never sit behind
//! endless covers.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
    time::Duration,
};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    browsing::SurprisePool,
    event::Event,
    reading::{
        ReadFilterChoice, ReadFilters, ReadHomeRow, ReadRowFormat, ReadRowKind,
        ReadSearchQuery, ReadSourceGroup, ReadSourceKind, ReadSourceSearch, ReadSourceState,
        ReadingAbout, ReadingCommand, ReadingEvent, ReadingHomeView, WorkTile, read_filters,
    },
};

use super::{
    Dest, OnScreen,
    discovery::{self, Caption, Landing, Part},
    listen, scope, send,
};
use crate::{
    hub::Hub,
    motion,
    pages::search_filters::{
        chips_box, filter_row, focus_chip, mark_set, menu_button, set_label, show_chips,
        sort_button,
    },
    say,
    squeeze::Squeeze,
    states::{Shape, States},
    widgets,
    window::Ctx,
};

/// How many covers a row of the start shows, as the shows' rails do.
const START_SHOWN: usize = 12;
/// Placeholders in a source's row while it is asked.
const SOURCE_PLACEHOLDERS: usize = 5;
/// AniList's grid waits as two full rows of placeholders.
const ANILIST_PLACEHOLDER_ROWS: i32 = 2;
/// Over From your sources, after AniList's grid; with nothing above it, 4.
const ABOVE_SOURCES: i32 = 34;
/// Over a source's row, after the one before; the first, 12.
const ABOVE_SOURCE: i32 = 26;

/// The order's words, as the Shows side says them.
fn sort_name(key: &str) -> String {
    match key {
        "TRENDING_DESC" => t::read_sort_trending(),
        "SCORE_DESC" => t::read_sort_score(),
        "START_DATE_DESC" => t::read_sort_newest(),
        "TITLE_ROMAJI" => t::library_sort_title(),
        _ => t::sort_popularity(),
    }
}

/// The words for `keys` among `choices`.
fn labels(keys: &[String], choices: &[ReadFilterChoice]) -> Vec<String> {
    keys.iter()
        .filter_map(|k| choices.iter().find(|c| c.key == *k).map(|c| say::msg(&c.label)))
        .collect()
}

/// The formats' words, with a row's formats read as the one they make: manga with one-shots is
/// Manga.
fn format_labels(keys: &[String], choices: &[ReadFilterChoice]) -> Vec<String> {
    let mut keys = keys.to_vec();
    for format in ReadRowFormat::ALL {
        let group = format.search_formats();
        if group.len() > 1 && group.iter().all(|k| keys.contains(k)) {
            keys.retain(|k| !group[1..].contains(k));
        }
    }
    labels(&keys, choices)
}

/// When a series started, as the query keeps it: `None` for any time.
fn started_of(query: &ReadSearchQuery) -> Option<&str> {
    query.started.as_deref().filter(|s| !s.is_empty() && *s != "any")
}

/// What a browse shows: the filters set, or the order with none.
fn browse_line(query: &ReadSearchQuery, filters: &ReadFilters, total: Option<u32>) -> String {
    let mut parts = format_labels(&query.formats, &filters.formats);
    parts.extend(labels(&query.origins, &filters.origins));
    if let Some(status) = &query.status {
        parts.extend(labels(std::slice::from_ref(status), &filters.statuses));
    }
    if let Some(started) = started_of(query) {
        parts.extend(labels(&[started.to_owned()], &filters.started));
    }
    parts.extend(query.genres.iter().map(|g| say::genre(g)));
    // The order follows what is set: "Manga, Trending".
    parts.push(sort_name(
        query.sort.as_deref().or(filters.sorts.first().map(String::as_str)).unwrap_or(""),
    ));
    let what = discovery::join(parts, t::read_linux_browse_more);
    match total {
        Some(n) => t::read_linux_line_total(&what, &t::read_search_total(i64::from(n))),
        None => t::read_search_browse_line(&what),
    }
}

/// View all's line: the row's own words, then the count once AniList knows it.
fn titled_line(line: &str, total: Option<u32>) -> String {
    match total {
        Some(n) => t::read_linux_line_total(line, &t::read_search_total(i64::from(n))),
        None => line.to_owned(),
    }
}

/// The line an AniList result gets when sources have it: "In Home library", "In 2 of your
/// sources".
fn joined_lines(search: &ReadSourceSearch) -> HashMap<String, String> {
    search
        .joins
        .iter()
        .filter(|j| !j.source_names.is_empty())
        .map(|j| {
            let line = match j.source_names.as_slice() {
                [one] => t::read_search_in_source(one),
                names => t::read_search_in_sources(names.len() as i64),
            };
            (j.work.0.clone(), line)
        })
        .collect()
}

/// The names of the sources that found nothing, for the one line at the end.
fn nothing_names(groups: &[ReadSourceGroup]) -> Vec<String> {
    groups
        .iter()
        .filter(|g| g.state == ReadSourceState::Nothing)
        .map(|g| g.source_name.clone())
        .collect()
}

/// The one polite announcement once a search has settled: "4 from AniList, 2 from your
/// sources." with each source that didn't answer after it. `anilist` is `None` when AniList
/// wasn't asked, and holds `None` when it didn't answer.
fn settled_words(anilist: Option<Option<usize>>, groups: &[ReadSourceGroup]) -> String {
    let found: usize = groups
        .iter()
        .filter(|g| g.state == ReadSourceState::Hits)
        .map(|g| g.tiles.len() + g.joined as usize)
        .sum();
    let mut said = match anilist {
        Some(Some(n)) => t::read_linux_search_done(n as i64, found as i64),
        // AniList was asked and didn't answer: said first, as the failed sources are after.
        Some(None) => t::read_linux_said_then(
            &t::read_search_anilist_failed(),
            &t::read_linux_search_done_sources(found as i64),
        ),
        None => t::read_linux_search_done_sources(found as i64),
    };
    for group in groups {
        if matches!(
            group.state,
            ReadSourceState::Failed { .. } | ReadSourceState::TimedOut { .. }
        ) {
            said = t::read_linux_said_then(
                &said,
                &t::read_search_source_failed(&group.source_name),
            );
        }
    }
    said
}

/// The order with words and none chosen: AniList's match, as the engine asks with no sort.
const RELEVANCE: &str = "relevance"; // i18n-skip

/// What a chip takes off.
#[derive(Clone)]
enum ChipKey {
    Genre(String),
    NotGenre(String),
    Started,
    Origins,
    Formats,
    Status,
    Mine,
}

/// The filters and the order, and the query they make: the shows' row of capsules with their
/// chips under it, reading's filters in the shows' slots (Started in Year's, Origin in Season's,
/// Your sources only in Available's).
struct Filters {
    row: gtk::Box,
    chips: adw::WrapBox,
    genres: gtk::MenuButton,
    started: gtk::MenuButton,
    origin: gtk::MenuButton,
    format: gtk::MenuButton,
    status: gtk::MenuButton,
    mine: gtk::ToggleButton,
    sort: gtk::MenuButton,
    sort_label: gtk::Label,
    actions: gio::SimpleActionGroup,
    choices: ReadFilters,
    query: RefCell<ReadSearchQuery>,
    /// What each chip on show takes off, in their order.
    chip_keys: RefCell<Vec<ChipKey>>,
    /// Words are typed: Your sources only shows, and the order offers Relevance.
    words: Cell<bool>,
    changed: RefCell<Option<Rc<dyn Fn()>>>,
    /// The switch is being set to the query's state, not by the person.
    settling: Cell<bool>,
}

impl Filters {
    fn new() -> Rc<Self> {
        let genres = menu_button(&t::read_filter_genres());
        let started = menu_button(&t::read_search_started());
        let origin = menu_button(&t::read_filter_origin());
        let format = menu_button(&t::read_filter_format());
        let status = menu_button(&t::read_filter_status());
        let (sort, sort_label) = sort_button();
        // Your sources only, in Available's slot: a capsule of the row, raised while on.
        let mine = gtk::ToggleButton::with_label(&t::read_search_sources_only());
        mine.add_css_class("flat");
        mine.add_css_class("filter");
        mine.add_css_class("read-sources-only");
        mine.set_visible(false);
        mine.set_accessible_role(gtk::AccessibleRole::Switch);
        let capsules = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for button in [&genres, &started, &origin, &format, &status] {
            capsules.append(button);
        }
        capsules.append(&mine);
        let row = filter_row(&capsules, &sort);
        let chips = chips_box();
        let actions = gio::SimpleActionGroup::new();
        row.insert_action_group("rfilter", Some(&actions));
        chips.insert_action_group("rfilter", Some(&actions));
        let filters = Rc::new(Self {
            row,
            chips,
            genres,
            started,
            origin,
            format,
            status,
            mine,
            sort,
            sort_label,
            actions,
            choices: read_filters(),
            query: RefCell::default(),
            chip_keys: RefCell::default(),
            words: Cell::new(false),
            changed: RefCell::default(),
            settling: Cell::new(false),
        });
        filters.mine.connect_toggled({
            let weak = Rc::downgrade(&filters);
            move |toggle| {
                let Some(filters) = weak.upgrade() else { return };
                if filters.settling.get() {
                    return;
                }
                let on = toggle.is_active();
                filters.edit(|q| ReadSearchQuery { sources_only: on, ..q.clone() });
                if on {
                    // Said once: what the switch hides.
                    toggle.announce(
                        &t::read_linux_anilist_hidden(),
                        gtk::AccessibleAnnouncementPriority::Medium,
                    );
                }
            }
        });
        filters.wire();
        filters.refresh();
        filters
    }

    fn wire(self: &Rc<Self>) {
        let plain = |name: &str, f: fn(&ReadSearchQuery) -> ReadSearchQuery| {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(filters) = weak.upgrade() {
                    filters.edit(f);
                }
            });
            self.actions.add_action(&action);
        };
        plain("any-genre", |q| ReadSearchQuery {
            genres: Vec::new(),
            exclude_genres: Vec::new(),
            ..q.clone()
        });
        plain("any-origin", |q| ReadSearchQuery { origins: Vec::new(), ..q.clone() });
        plain("any-format", |q| ReadSearchQuery { formats: Vec::new(), ..q.clone() });
        plain("any-status", |q| ReadSearchQuery { status: None, ..q.clone() });
        // Every filter off, Your sources only with them; the order stays, as the shows' does.
        plain("clear", |q| ReadSearchQuery {
            sort: q.sort.clone(),
            ..ReadSearchQuery::default()
        });

        // Each chip takes its filter off: its index among those on show.
        let remove = gio::SimpleAction::new("remove", Some(glib::VariantTy::UINT32));
        let weak = Rc::downgrade(self);
        remove.connect_activate(move |_, target| {
            let (Some(filters), Some(at)) =
                (weak.upgrade(), target.and_then(u32::from_variant))
            else {
                return;
            };
            let Some(key) = filters.chip_keys.borrow().get(at as usize).cloned() else {
                return;
            };
            let had_focus = filters.chips.focus_child().is_some();
            filters.edit(|q| {
                let mut q = q.clone();
                match key {
                    ChipKey::Genre(name) => q.genres.retain(|g| *g != name),
                    ChipKey::NotGenre(name) => q.exclude_genres.retain(|g| *g != name),
                    ChipKey::Started => q.started = None,
                    ChipKey::Origins => q.origins.clear(),
                    ChipKey::Formats => q.formats.clear(),
                    ChipKey::Status => q.status = None,
                    ChipKey::Mine => q.sources_only = false,
                }
                q
            });
            if had_focus {
                focus_chip(&filters.chips, at as usize, &filters.genres);
            }
        });
        self.actions.add_action(&remove);
    }

    fn on_change(&self, f: impl Fn() + 'static) {
        self.changed.replace(Some(Rc::new(f)));
    }

    /// The query for `text`. Without words there are no sources to ask: AniList alone.
    fn query(&self, text: &str) -> ReadSearchQuery {
        let text = text.trim().to_owned();
        let sources_only = self.query.borrow().sources_only && !text.is_empty();
        ReadSearchQuery { text, sources_only, ..self.query.borrow().clone() }
    }

    fn any_set(&self) -> bool {
        let q = self.query.borrow();
        !(q.formats.is_empty()
            && q.origins.is_empty()
            && q.status.is_none()
            && started_of(&q).is_none()
            && q.genres.is_empty()
            && q.exclude_genres.is_empty())
    }

    /// Nothing typed, set or chosen: Search's start.
    fn at_start(&self, text: &str) -> bool {
        text.trim().is_empty() && !self.any_set() && self.query.borrow().sort.is_none()
    }

    /// Words were typed or cleared: Your sources only shows only with words, and the order offers
    /// Relevance.
    fn set_words(self: &Rc<Self>, words: bool) {
        if self.words.replace(words) != words {
            self.refresh();
        }
    }

    /// Every filter off, as Clear filters does.
    fn clear(&self) {
        self.actions.activate_action("clear", None);
    }

    /// One genre's series, as its filter alone, as a genre tile asks.
    fn only_genre(self: &Rc<Self>, genre: String) {
        self.edit(|_| ReadSearchQuery { genres: vec![genre], ..ReadSearchQuery::default() });
    }

    /// Take `query`'s filters and order as they are, as See all on a row hands them over.
    fn adopt(self: &Rc<Self>, query: &ReadSearchQuery) {
        self.query.replace(ReadSearchQuery {
            text: String::new(),
            sources_only: false,
            ..query.clone()
        });
        self.refresh();
    }

    fn edit(self: &Rc<Self>, f: impl FnOnce(&ReadSearchQuery) -> ReadSearchQuery) {
        let next = f(&self.query.borrow());
        self.query.replace(next);
        self.refresh();
        let changed = self.changed.borrow().clone();
        if let Some(changed) = changed {
            changed();
        }
    }

    /// A checked item's action: made the first time, its state kept after.
    fn check(
        self: &Rc<Self>,
        name: &str,
        on: bool,
        flip: impl Fn(&ReadSearchQuery) -> ReadSearchQuery + 'static,
    ) {
        if let Some(action) =
            self.actions.lookup_action(name).and_downcast::<gio::SimpleAction>()
        {
            action.set_state(&on.to_variant());
            return;
        }
        let action = gio::SimpleAction::new_stateful(name, None, &on.to_variant());
        let weak = Rc::downgrade(self);
        action.connect_activate(move |_, _| {
            if let Some(filters) = weak.upgrade() {
                filters.edit(&flip);
            }
        });
        self.actions.add_action(&action);
    }

    /// Format or Origin: any number of each, then Any once one is set, as the shows' Format.
    fn several(self: &Rc<Self>, origin: bool) {
        let query = self.query.borrow().clone();
        let (button, title, list, picked, prefix) = if origin {
            (
                &self.origin,
                t::read_filter_origin(),
                &self.choices.origins,
                &query.origins,
                "origin",
            )
        } else {
            (
                &self.format,
                t::read_filter_format(),
                &self.choices.formats,
                &query.formats,
                "format",
            )
        };
        let all = gio::Menu::new();
        for choice in list {
            let name = format!("{prefix}-{}", choice.key);
            let key = choice.key.clone();
            self.check(&name, picked.contains(&choice.key), move |q| {
                let mut q = q.clone();
                let list = if origin { &mut q.origins } else { &mut q.formats };
                if let Some(at) = list.iter().position(|k| *k == key) {
                    list.remove(at);
                } else {
                    list.push(key.clone());
                }
                q
            });
            all.append(Some(&say::msg(&choice.label)), Some(&format!("rfilter.{name}")));
        }
        let menu = gio::Menu::new();
        menu.append_section(None, &all);
        let words = if origin { labels(picked, list) } else { format_labels(picked, list) };
        if !words.is_empty() {
            let any = gio::Menu::new();
            let (said, action) = if origin {
                (t::read_filter_any(), "rfilter.any-origin")
            } else {
                (t::filter_any_format(), "rfilter.any-format")
            };
            any.append(Some(&said), Some(action));
            menu.append_section(None, &any);
        }
        button.set_menu_model(Some(&menu));
        let label = match words.len() {
            0 => title,
            n if n > 2 && !origin => t::filter_linux_formats_count(n as i64),
            _ => words.join(", "),
        };
        set_label(button, &label, !words.is_empty());
    }

    fn refresh(self: &Rc<Self>) {
        self.several(false);
        self.several(true);
        let query = self.query.borrow().clone();
        let choices = &self.choices;
        let words = self.words.get();

        // Status: one of them, then Any status once one is set, as the shows' Status.
        let statuses = gio::Menu::new();
        for choice in &choices.statuses {
            let name = format!("status-{}", choice.key);
            let key = choice.key.clone();
            self.check(&name, query.status.as_deref() == Some(choice.key.as_str()), move |q| {
                let on = q.status.as_deref() == Some(key.as_str());
                ReadSearchQuery { status: (!on).then(|| key.clone()), ..q.clone() }
            });
            statuses.append(Some(&say::msg(&choice.label)), Some(&format!("rfilter.{name}")));
        }
        let menu = gio::Menu::new();
        menu.append_section(None, &statuses);
        if query.status.is_some() {
            let any = gio::Menu::new();
            any.append(Some(&t::filter_any_status()), Some("rfilter.any-status"));
            menu.append_section(None, &any);
        }
        self.status.set_menu_model(Some(&menu));
        let status = query.status.as_ref().and_then(|s| {
            labels(std::slice::from_ref(s), &choices.statuses).into_iter().next()
        });
        set_label(
            &self.status,
            status.as_deref().unwrap_or(&t::read_filter_status()),
            status.is_some(),
        );

        // Genres, as the shows' menu: each a toggle, Left out in a submenu, Any genre once set.
        let wanted = gio::Menu::new();
        let left_out = gio::Menu::new();
        for (at, genre) in choices.genres.iter().enumerate() {
            let (want, leave) = (format!("genre-{at}"), format!("leave-{at}"));
            let named = genre.clone();
            self.check(&want, query.genres.contains(genre), move |q| {
                let mut q = q.clone();
                q.exclude_genres.retain(|g| *g != named);
                if let Some(at) = q.genres.iter().position(|g| *g == named) {
                    q.genres.remove(at);
                } else {
                    q.genres.push(named.clone());
                }
                q
            });
            let named = genre.clone();
            self.check(&leave, query.exclude_genres.contains(genre), move |q| {
                let mut q = q.clone();
                q.genres.retain(|g| *g != named);
                if let Some(at) = q.exclude_genres.iter().position(|g| *g == named) {
                    q.exclude_genres.remove(at);
                } else {
                    q.exclude_genres.push(named.clone());
                }
                q
            });
            wanted.append(Some(&say::genre(genre)), Some(&format!("rfilter.{want}")));
            left_out.append(Some(&say::genre(genre)), Some(&format!("rfilter.{leave}")));
        }
        let menu = gio::Menu::new();
        menu.append_section(None, &wanted);
        let more = gio::Menu::new();
        more.append_submenu(Some(&t::filter_left_out()), &left_out);
        menu.append_section(None, &more);
        let set = query.genres.len() + query.exclude_genres.len();
        if set > 0 {
            let any = gio::Menu::new();
            any.append(Some(&t::filter_any_genre()), Some("rfilter.any-genre"));
            menu.append_section(None, &any);
        }
        self.genres.set_menu_model(Some(&menu));
        let label = match (set, query.genres.first(), query.exclude_genres.first()) {
            (0, ..) => t::read_filter_genres(),
            (1, Some(genre), _) => say::genre(genre),
            (1, None, Some(genre)) => t::filter_linux_not(&say::genre(genre)),
            (n, ..) => t::filter_linux_genres_count(n as i64),
        };
        set_label(&self.genres, &label, set > 0);

        // Started: any time on its own, then the spans, as the shows' Year.
        let started = started_of(&query).map(str::to_owned);
        let any = gio::Menu::new();
        let spans = gio::Menu::new();
        for choice in &choices.started {
            let name = format!("started-{}", choice.key);
            let key = (choice.key != "any").then(|| choice.key.clone());
            let shown = key.is_none();
            self.check(&name, started == key, move |q| ReadSearchQuery {
                started: key.clone(),
                ..q.clone()
            });
            let menu = if shown { &any } else { &spans };
            menu.append(Some(&say::msg(&choice.label)), Some(&format!("rfilter.{name}")));
        }
        let menu = gio::Menu::new();
        menu.append_section(None, &any);
        menu.append_section(None, &spans);
        self.started.set_menu_model(Some(&menu));
        let started_words = started
            .as_ref()
            .and_then(|s| labels(std::slice::from_ref(s), &choices.started).into_iter().next());
        set_label(
            &self.started,
            started_words.as_deref().unwrap_or(&t::read_search_started()),
            started_words.is_some(),
        );

        // The order, as the shows' sort: with words and none chosen, Relevance, offered first.
        let current = match &query.sort {
            Some(sort) => sort.clone(),
            None if words => RELEVANCE.to_owned(),
            None => choices.sorts.first().cloned().unwrap_or_default(),
        };
        let sorts = gio::Menu::new();
        if words {
            self.check("sort-relevance", current == RELEVANCE, |q| ReadSearchQuery {
                sort: None,
                ..q.clone()
            });
            sorts.append(Some(&t::sort_relevance()), Some("rfilter.sort-relevance"));
        }
        for key in &choices.sorts {
            let name = format!("sort-{key}");
            let chosen = key.clone();
            self.check(&name, *key == current, move |q| ReadSearchQuery {
                sort: Some(chosen.clone()),
                ..q.clone()
            });
            sorts.append(Some(&sort_name(key)), Some(&format!("rfilter.{name}")));
        }
        let menu = gio::Menu::new();
        menu.append_section(Some(&t::filter_linux_sort_by()), &sorts);
        self.sort.set_menu_model(Some(&menu));
        let named =
            if current == RELEVANCE { t::sort_relevance() } else { sort_name(&current) };
        self.sort_label.set_label(&named);
        self.sort.set_tooltip_text(Some(&t::sort_label(&named)));
        mark_set(&self.sort, query.sort.is_some());

        // Your sources only: the other filters are AniList's, so they dim while it is on.
        let mine = query.sources_only && words;
        self.mine.set_visible(words);
        self.settling.set(true);
        self.mine.set_active(mine);
        self.settling.set(false);
        mark_set(&self.mine, mine);
        self.mine.update_state(&[gtk::accessible::State::Checked(if mine {
            gtk::AccessibleTristate::True
        } else {
            gtk::AccessibleTristate::False
        })]);
        for button in [&self.genres, &self.started, &self.origin, &self.format, &self.status] {
            dim(button, mine);
        }
        dim(&self.sort, mine);

        // What is set, as the shows' chips, in the row's order.
        let mut chips: Vec<(String, ChipKey)> = Vec::new();
        for genre in &query.genres {
            chips.push((say::genre(genre), ChipKey::Genre(genre.clone())));
        }
        for genre in &query.exclude_genres {
            chips.push((
                t::filter_linux_not(&say::genre(genre)),
                ChipKey::NotGenre(genre.clone()),
            ));
        }
        if let Some(words) = started_words {
            chips.push((words, ChipKey::Started));
        }
        let origins = labels(&query.origins, &choices.origins);
        if !origins.is_empty() {
            chips.push((origins.join(", "), ChipKey::Origins));
        }
        let formats = format_labels(&query.formats, &choices.formats);
        if !formats.is_empty() {
            chips.push((formats.join(", "), ChipKey::Formats));
        }
        if let Some(status) = status {
            chips.push((status, ChipKey::Status));
        }
        if mine {
            chips.push((t::read_search_sources_only(), ChipKey::Mine));
        }
        let words: Vec<String> = chips.iter().map(|(label, _)| label.clone()).collect();
        show_chips(&self.chips, &words, "rfilter.remove", "rfilter.clear");
        self.chip_keys.replace(chips.into_iter().map(|(_, key)| key).collect());
    }
}

/// A filter that is AniList's alone: dimmed to 0.38 and out of reach while Your sources only is
/// on.
fn dim(button: &impl IsA<gtk::Widget>, on: bool) {
    let button = button.as_ref();
    button.set_sensitive(!on);
    if on {
        button.add_css_class("read-dimmed");
    } else {
        button.remove_css_class("read-dimmed");
    }
}

/// Covers stand this far in from the page's margin and under what is over them, as the shows'
/// grid's padding and cells put theirs.
const COVERS_IN: i32 = 32;
const COVERS_DOWN: i32 = 16;

/// Covers, or their placeholders, set in as the shows' grid sets its covers.
fn inset<W: IsA<gtk::Widget>>(covers: W) -> W {
    covers.set_margin_start(COVERS_IN);
    covers.set_margin_end(COVERS_IN);
    covers.set_margin_top(COVERS_DOWN);
    covers
}

/// A grid of posters, as many columns as fit, 16 apart across and 24 down.
fn poster_grid() -> gtk::Box {
    let layout = crate::cards_layout::CardsLayout::apart(
        discovery::POSTER,
        discovery::GAP,
        discovery::GRID_ROW_GAP,
    );
    let grid = gtk::Box::new(gtk::Orientation::Vertical, 0);
    grid.set_layout_manager(Some(layout));
    grid.set_valign(gtk::Align::Start);
    grid.set_accessible_role(gtk::AccessibleRole::List);
    grid.update_property(&[gtk::accessible::Property::Label(&t::poster_grid_name())]);
    inset(grid)
}

fn add_posters(
    hub: &Hub,
    grid: &gtk::Box,
    tiles: &[WorkTile],
    caption: Caption,
    joined: &HashMap<String, String>,
) {
    for tile in tiles {
        let line = joined.get(&tile.work.0).map(String::as_str);
        grid.append(&discovery::item(&discovery::poster(hub, tile, caption, line, true)));
    }
}

/// The captions AniList's results take: Format · Origin for typed words; browsing one format, as
/// View all on a row does, the origin in its place (and the start month on a New row's list);
/// several formats, the format and status.
fn caption_for(query: &ReadSearchQuery) -> Caption {
    let one_format = ReadRowFormat::ALL.iter().any(|format| {
        let formats = format.search_formats();
        !query.formats.is_empty() && query.formats.iter().all(|f| formats.contains(f))
    });
    let started = query.started.as_deref().is_some_and(|s| !s.is_empty() && s != "any"); // i18n-skip
    if !query.text.is_empty() {
        Caption::FormatOrigin
    } else if one_format && started {
        Caption::OriginStarted
    } else if one_format {
        Caption::OriginStatus
    } else {
        Caption::FormatStatus
    }
}

/// One row of the start: a discovery row of reading's Home, as the shows' rails stand.
struct StartRow {
    id: String,
    place: gtk::Box,
    title: gtk::Label,
    side: gtk::Label,
    all: gtk::Button,
    landing: Rc<Landing>,
    /// See all's query and the line over its results.
    view_all: RefCell<Option<(ReadSearchQuery, Option<String>)>>,
    /// What it shows, to leave it be when a send changes nothing in it.
    signature: RefCell<String>,
}

/// One source's row in From your sources.
struct GroupSlot {
    id: String,
    part: Rc<Part>,
    name: gtk::Label,
    kind: gtk::Label,
    spinner: adw::Spinner,
    signature: RefCell<String>,
    /// Its failure is on show: asked again, placeholders take its place after 600 ms.
    failed: Cell<bool>,
}

struct Search {
    ctx: Weak<Ctx>,
    hub: Weak<Hub>,
    filters: Rc<Filters>,
    entry: gtk::SearchEntry,
    states: Rc<States>,
    page: adw::NavigationPage,
    veil: gtk::Overlay,
    /// AniList's part: its line, the other side's count, its note and its first page.
    line: gtk::Label,
    other: gtk::Box,
    other_words: gtk::Label,
    anilist: gtk::Box,
    anilist_place: Squeeze,
    anilist_landing: Rc<Landing>,
    grid: RefCell<gtk::Box>,
    /// From your sources.
    sources: gtk::Box,
    sources_head: gtk::Label,
    groups: gtk::Box,
    /// From your sources fading out, until it hides.
    sources_fade: RefCell<Option<adw::TimedAnimation>>,
    nothing_line: gtk::Label,
    slots: RefCell<Vec<Rc<GroupSlot>>>,
    /// AniList's later pages, under the sources, and More from AniList.
    later: gtk::Box,
    more_row: gtk::Box,
    more_shown: gtk::Label,
    spinner: adw::Spinner,
    /// Nothing found anywhere: the empty state shows in the page's place.
    nothing: Cell<bool>,
    /// The empty state, with Clear filters while filters are set.
    empty: adw::StatusPage,
    clear: gtk::Button,
    /// The start, before anything is typed, set or chosen, and the results.
    body: gtk::Stack,
    /// The start's rows, reading Home's discovery rows, by id.
    start_rows: gtk::Box,
    rows: RefCell<Vec<Rc<StartRow>>>,
    /// Home's rows have been asked for, or have come.
    home_asked: Cell<bool>,
    /// The start's rows wait for the place to show.
    shown: Rc<OnScreen>,

    /// The query asked and the last page of it in, from 0.
    at: RefCell<(ReadSearchQuery, u32)>,
    asked: Cell<bool>,
    tiles: RefCell<Vec<WorkTile>>,
    first_page: RefCell<Vec<WorkTile>>,
    has_more: Cell<bool>,
    asking_more: Cell<bool>,
    total: Cell<Option<u32>>,
    /// View all's line for the query it opened, kept while that query is on screen.
    titled: RefCell<Option<(ReadSearchQuery, String)>>,
    anilist_failed: Cell<bool>,
    anilist_in: Cell<bool>,
    source: RefCell<Option<ReadSourceSearch>>,
    joined: RefCell<HashMap<String, String>>,
    announced: Cell<bool>,
    /// Back to Read, while View all has Search over it.
    back: gtk::Button,
    /// Opened by View all: the first result takes focus once it lands.
    focus_first: Cell<bool>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    // AniList's part.
    // What the results are, in the shows' caption over the grid.
    let line = widgets::haze("", "caption");
    line.set_margin_bottom(12);
    line.set_visible(false);
    let other_words = widgets::haze("", "caption");
    let show_them = gtk::Button::with_label(&t::search_show_them());
    show_them.add_css_class("flat");
    let other = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    other.set_margin_bottom(12);
    other.set_visible(false);
    other.append(&other_words);
    other.append(&show_them);
    // The placeholders keep the line's room over them, so the covers land where they wait.
    let anilist_landing = Landing::new(|| {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.set_accessible_role(gtk::AccessibleRole::Presentation);
        let room = widgets::haze("", "caption");
        room.set_margin_bottom(12);
        room.set_accessible_role(gtk::AccessibleRole::Presentation);
        column.append(&room);
        column.append(&inset(discovery::grid_placeholders(ANILIST_PLACEHOLDER_ROWS)));
        column.upcast()
    });
    let anilist = gtk::Box::new(gtk::Orientation::Vertical, 0);
    anilist.append(&anilist_landing.widget);
    let anilist_place = Squeeze::new(&anilist, gtk::Orientation::Vertical, 0);

    // From your sources.
    let sources_head = gtk::Label::new(Some(&t::read_search_from_sources()));
    sources_head.add_css_class("title-4");
    sources_head.set_xalign(0.0);
    sources_head.set_accessible_role(gtk::AccessibleRole::Heading);
    sources_head.update_property(&[gtk::accessible::Property::Level(2)]);
    sources_head.set_margin_bottom(2);
    let groups = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let nothing_line = widgets::haze("", "read-search-nothing-line");
    nothing_line.set_visible(false);
    let sources = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sources.append(&sources_head);
    sources.append(&groups);
    sources.append(&nothing_line);
    sources.set_visible(false);

    // AniList's later pages and More from AniList.
    let later = poster_grid();
    later.set_margin_top(22);
    later.set_visible(false);
    let more_button = gtk::Button::with_label(&t::read_search_more_anilist());
    more_button.add_css_class("home-sec");
    let more_shown = widgets::haze("", "read-search-shown");
    more_shown.set_valign(gtk::Align::Center);
    more_shown.set_wrap(false);
    more_shown.set_visible(false);
    let more_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    more_row.set_margin_top(22);
    more_row.append(&more_button);
    more_row.append(&more_shown);
    more_row.set_visible(false);
    let spinner = adw::Spinner::new();
    spinner.set_halign(gtk::Align::Center);
    spinner.set_size_request(24, 24);
    spinner.set_margin_top(16);
    spinner.set_visible(false);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.add_css_class("read-search-body");
    body.append(&anilist_place);
    body.append(&sources);
    body.append(&later);
    body.append(&more_row);
    body.append(&spinner);
    let veil = gtk::Overlay::new();
    veil.set_child(Some(&body));
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&veil)
        .build();
    // The shows' empty state, with reading's words, and Clear filters as theirs.
    let clear = gtk::Button::with_label(&t::search_clear_filters());
    clear.add_css_class("pill");
    clear.set_visible(false);
    let empty =
        crate::states::plain(&t::read_search_nothing_browse(), &t::read_search_nothing_line());
    clear.set_halign(gtk::Align::Center);
    empty.set_child(Some(&clear));
    let states = States::new(&ctx.hub, &scroll, Shape::Grid, &empty, &t::read_search_failed());
    states.offline_words(&t::search_needs_connection(), &t::state_offline_detail());

    // The field in its bar under the header, where the shows' Search has it, so switching sides
    // moves neither.
    let (bar, entry, toggle) = crate::pages::search::field(&t::read_search_placeholder());
    bar.set_search_mode(true);
    // Typing anywhere on this side goes to its field, as on the shows' side. The capture only
    // acts while the bar is on screen, so the shows' field and the player keep their keys.
    bar.set_key_capture_widget(Some(&ctx.window));
    ctx.read_search_focus.replace(Some(Rc::new({
        let (bar, entry) = (bar.downgrade(), entry.downgrade());
        move || {
            if let (Some(bar), Some(entry)) = (bar.upgrade(), entry.upgrade()) {
                crate::pages::search::open_field(&bar, &entry);
            }
        }
    })));
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_start(&crate::window::tv_session_button(ctx));
    header.pack_end(&toggle);
    // Back to Read, while View all has Search pushed over it.
    let back = gtk::Button::from_icon_name("go-previous-symbolic");
    back.set_tooltip_text(Some(&t::action_back()));
    back.update_property(&[gtk::accessible::Property::Label(&t::action_back())]);
    back.set_action_name(Some("win.read-back-home"));
    back.set_visible(false);
    header.pack_start(&back);
    ctx.on_dest_changed({
        let (ctx, back) = (Rc::downgrade(ctx), back.downgrade());
        move |dest| {
            let (Some(ctx), Some(back)) = (ctx.upgrade(), back.upgrade()) else { return };
            if dest != Some(Dest::ReadSearch) {
                ctx.read_from_home.set(false);
            }
            back.set_visible(ctx.read_from_home.get());
        }
    });
    let filters = Filters::new();
    // Surprise me: a random book from the filters in effect, in the shows' place.
    let surprise =
        widgets::surprise_button(ctx, &t::surprise_me(), &t::surprise_hint_filters(), false, {
            let filters = Rc::downgrade(&filters);
            move || {
                let filters = filters.upgrade()?;
                Some(SurprisePool::Reading { query: filters.query("") })
            }
        });
    header.pack_end(&surprise);

    // The start: Home's discovery rows, then the genres, as the shows' start.
    let start_rows = gtk::Box::new(gtk::Orientation::Vertical, 18);
    // Until Home's rows come, two rows of placeholders, as the shows' rails wait.
    for _ in 0..2 {
        let waiting = inset(discovery::grid_placeholders(1));
        waiting.set_margin_start(18 + COVERS_IN);
        waiting.set_margin_end(18 + COVERS_IN);
        start_rows.append(&waiting);
    }
    let genres = crate::pages::search::genre_tiles(&filters.choices.genres, {
        let filters = Rc::downgrade(&filters);
        move |genre| {
            if let Some(filters) = filters.upgrade() {
                filters.only_genre(genre);
            }
        }
    });
    let start = gtk::Box::new(gtk::Orientation::Vertical, 18);
    start.set_margin_top(12);
    start.set_margin_bottom(24);
    start.append(&start_rows);
    start.append(&crate::pages::search::section(&t::search_genres(), &genres));
    let start = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&start)
        .build();
    let body_stack = gtk::Stack::new();
    body_stack.set_vexpand(true);
    body_stack.add_named(&start, Some("start"));
    body_stack.add_named(&states.widget, Some("results"));

    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.append(&bar);
    // Shows or Reading leads the content bar, where Library has it.
    let scope = scope(ctx, Dest::Search, true);
    content.append(&crate::pages::search_filters::with_scope(ctx, &filters.row, &scope));
    content.append(&filters.chips);
    content.append(&body_stack);
    let page = widgets::read_content_page(&t::nav_search(), &header, &content);

    let search = Rc::new(Search {
        ctx: Rc::downgrade(ctx),
        hub: Rc::downgrade(&ctx.hub),
        filters: Rc::clone(&filters),
        entry: entry.clone(),
        states: states.clone(),
        page: page.clone(),
        veil,
        line,
        other,
        other_words,
        anilist,
        anilist_place,
        anilist_landing,
        grid: RefCell::new(poster_grid()),
        sources,
        sources_head,
        sources_fade: RefCell::default(),
        groups,
        nothing_line,
        slots: RefCell::default(),
        later,
        more_row,
        more_shown,
        spinner,
        nothing: Cell::new(false),
        empty,
        clear: clear.clone(),
        body: body_stack,
        start_rows,
        rows: RefCell::default(),
        home_asked: Cell::new(false),
        shown: OnScreen::new(&page),
        at: RefCell::new((ReadSearchQuery::default(), 0)),
        titled: RefCell::default(),
        asked: Cell::new(false),
        tiles: RefCell::default(),
        first_page: RefCell::default(),
        has_more: Cell::new(false),
        asking_more: Cell::new(false),
        total: Cell::new(None),
        anilist_failed: Cell::new(false),
        anilist_in: Cell::new(false),
        source: RefCell::default(),
        joined: RefCell::default(),
        announced: Cell::new(false),
        back: back.clone(),
        focus_first: Cell::new(false),
    });

    filters.on_change({
        let search = Rc::downgrade(&search);
        move || {
            if let Some(search) = search.upgrade() {
                search.ask(false);
            }
        }
    });
    clear.connect_clicked({
        let filters = Rc::downgrade(&filters);
        move |_| {
            if let Some(filters) = filters.upgrade() {
                filters.clear();
            }
        }
    });
    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    entry.connect_search_changed({
        let (search, pending) = (Rc::downgrade(&search), Rc::clone(&pending));
        move |entry| {
            let Some(search) = search.upgrade() else { return };
            search.filters.set_words(!entry.text().trim().is_empty());
            if let Some(id) = pending.borrow_mut().take() {
                id.remove();
            }
            let (weak, slot) = (Rc::downgrade(&search), Rc::clone(&pending));
            let id = glib::timeout_add_local_once(Duration::from_millis(350), move || {
                slot.borrow_mut().take();
                if let Some(search) = weak.upgrade() {
                    search.ask(false);
                }
            });
            *pending.borrow_mut() = Some(id);
        }
    });
    // Browsing loads its next page near the end by itself; with words, More from AniList asks.
    scroll.vadjustment().connect_value_changed({
        let search = Rc::downgrade(&search);
        move |adj| {
            let Some(search) = search.upgrade() else { return };
            let near_end = adj.value() + adj.page_size() > adj.upper() - 600.0;
            let browsing = search.at.borrow().0.text.is_empty();
            if near_end && browsing {
                search.more();
            }
        }
    });
    more_button.connect_clicked({
        let search = Rc::downgrade(&search);
        move |_| {
            if let Some(search) = search.upgrade() {
                search.more();
            }
        }
    });
    // Show them: over to the Shows side.
    show_them.connect_clicked({
        let ctx = Rc::downgrade(ctx);
        move |_| {
            let Some(ctx) = ctx.upgrade() else { return };
            ctx.prefs.set_scope_reading(Dest::Search.name(), false);
            let _ = WidgetExt::activate_action(
                &ctx.window,
                "win.scope",
                Some(&Dest::Search.name().to_variant()),
            );
        }
    });
    states.on_retry({
        let search = Rc::downgrade(&search);
        move || {
            if let Some(search) = search.upgrade() {
                search.ask(true);
            }
        }
    });
    listen(ctx, &page, {
        let search = Rc::downgrade(&search);
        move |event| {
            if let Some(search) = search.upgrade() {
                search.heard(event);
            }
        }
    });
    // View all on a Read row: its filters and order, no words.
    ctx.read_view_all.replace(Some(Rc::new({
        let search = Rc::downgrade(&search);
        move |query, line: Option<String>| {
            if let Some(search) = search.upgrade() {
                search.see_all(&query, line);
                // Focus waits on Back until the list lands, then takes its first result.
                search.focus_first.set(true);
                let back = search.back.clone();
                glib::idle_add_local_once(move || {
                    if back.is_mapped() {
                        back.grab_focus();
                    }
                });
            }
        }
    })));
    // Shown with nothing asked yet: at its first showing, or when reading was turned on after
    // the window opened.
    page.connect_map({
        let search = Rc::downgrade(&search);
        move |_| {
            if let Some(search) = search.upgrade()
                && !search.asked.get()
                && search.hub.upgrade().is_some_and(|h| super::reads(&h))
            {
                search.ask(false);
            }
        }
    });
    if search.shown.now() && super::reads(&ctx.hub) {
        search.ask(false);
    }
    page.set_tag(Some("read-search"));
    widgets::keep(&page, search);
    page
}

impl Search {
    /// Ask for what the field and the filters say now: `force` asks again even when nothing
    /// changed (Try again, reading turned on, View all).
    fn ask(self: &Rc<Self>, force: bool) {
        let Some(hub) = self.hub.upgrade() else { return };
        let query = self.filters.query(&self.entry.text());
        let before = self.at.borrow().0.clone();
        if !force && self.asked.get() && query == before {
            return;
        }
        self.asked.set(true);
        self.focus_first.set(false);
        // Nothing typed, set or chosen: the start, and nothing is asked of AniList.
        if self.filters.at_start(&query.text) {
            *self.at.borrow_mut() = (query, 0);
            self.titled.replace(None);
            self.body.set_visible_child_name("start");
            self.ask_home(&hub);
            return;
        }
        self.body.set_visible_child_name("results");
        let words = !query.text.is_empty();
        let same_words = words && before.text == query.text && self.source.borrow().is_some();
        *self.at.borrow_mut() = (query.clone(), 0);
        self.tiles.borrow_mut().clear();
        self.has_more.set(false);
        self.asking_more.set(false);
        self.total.set(None);
        self.anilist_failed.set(false);
        self.anilist_in.set(false);
        self.announced.set(false);
        self.spinner.set_visible(false);
        self.more_row.set_visible(false);
        self.later.set_visible(false);
        super::clear(&self.later);
        self.nothing.set(false);
        // New words keep the sources' rows where they are until their new answers land, as
        // AniList's grid is kept; a browse has no sources' part and starts it again.
        if !same_words {
            self.source.replace(None);
            self.joined.borrow_mut().clear();
            if !words {
                for slot in self.slots.take() {
                    self.groups.remove(&slot.part.place);
                }
                if let Some(fade) = self.sources_fade.take() {
                    fade.pause();
                }
                self.sources.set_visible(false);
                self.sources.set_opacity(1.0);
                self.nothing_line.set_visible(false);
            }
        }
        self.show_anilist(!query.sources_only);
        if words {
            self.states.content();
            if !query.sources_only {
                self.anilist_landing.wait(false);
            }
        } else {
            self.states.loading(Some(t::activity_browse()));
        }
        send(&hub, ReadingCommand::Search { query });
    }

    /// See all on a row of the start, or View all on Read's: the row's filters and order with no
    /// words, under the row's own line.
    fn see_all(self: &Rc<Self>, query: &ReadSearchQuery, line: Option<String>) {
        self.filters.adopt(query);
        self.entry.set_text("");
        self.filters.set_words(false);
        self.titled.replace(None);
        self.ask(true);
        let asked = self.at.borrow().0.clone();
        self.titled.replace(line.map(|line| (asked, line)));
        self.set_line();
    }

    /// Home's rows for the start, once: Read asks for them too, and the start follows each send.
    fn ask_home(&self, hub: &Rc<Hub>) {
        if !self.home_asked.replace(true) {
            send(hub, ReadingCommand::LoadHome { refresh: false });
        }
    }

    /// AniList's next page: under the grid while browsing, under the sources with words.
    fn more(&self) {
        if !self.has_more.get() || self.asking_more.get() {
            return;
        }
        let Some(hub) = self.hub.upgrade() else { return };
        self.asking_more.set(true);
        self.spinner.set_visible(true);
        let (query, page) = self.at.borrow().clone();
        send(&hub, ReadingCommand::SearchMore { query, page: page + 1 });
    }

    /// AniList's part shows, or goes while Your sources only is on: it fades out over 200 ms as
    /// the sources rise on the standard spring, and comes back the same way. Under Reduce
    /// Animation the two states crossfade.
    fn show_anilist(&self, show: bool) {
        let place = &self.anilist_place;
        let shown = place.get_visible() && !place.is_closing();
        self.place_sources(!show);
        if shown == show {
            return;
        }
        let set = |show: bool| {
            if show {
                place.set_visible(true);
                place.open(|| {});
            } else {
                place.mark_closing();
                place.set_visible(false);
            }
            self.anilist.set_opacity(1.0);
        };
        if !self.page.is_mapped() {
            set(show);
            return;
        }
        if motion::reduced() {
            if let Some(body) = self.veil.child() {
                motion::ghost(&self.veil, &body, true, || set(show));
            } else {
                set(show);
            }
            return;
        }
        let animation = motion::timed(
            &self.anilist,
            if show { 0.0 } else { self.anilist.opacity() },
            if show { 1.0 } else { 0.0 },
            200,
            motion::opacity(&self.anilist),
        );
        animation.set_easing(if show {
            adw::Easing::EaseOutCubic
        } else {
            adw::Easing::EaseInCubic
        });
        if show {
            place.set_visible(true);
            place.set_closed();
            self.anilist.set_opacity(0.0);
            place.open(|| {});
        } else {
            let gone = place.clone();
            place.close(move || gone.set_visible(false));
        }
        animation.play();
    }

    /// The start's rows as Home's newest send has them: each keeps its place by id, its
    /// placeholders while it is on its way, and its covers crossfading in.
    fn draw_start(self: &Rc<Self>, view: &ReadingHomeView) {
        let Some(hub) = self.hub.upgrade() else { return };
        if self.rows.borrow().is_empty() {
            super::clear(&self.start_rows);
        }
        let mut old = self.rows.take();
        let mut live = Vec::new();
        // Catalogue-wide, as the shows' start: Because you finished stays on Home.
        for row in view.rows.iter().filter(|r| r.kind != ReadRowKind::Because) {
            let slot = match old.iter().position(|s| s.id == row.id) {
                Some(at) => old.remove(at),
                None => self.start_row(row),
            };
            fill_start_row(&hub, &slot, row);
            live.push(slot);
        }
        for slot in old {
            if slot.place.parent().is_some() {
                self.start_rows.remove(&slot.place);
            }
        }
        let mut previous: Option<gtk::Widget> = None;
        for slot in &live {
            let place = &slot.place;
            if place.parent().as_ref() == Some(self.start_rows.upcast_ref()) {
                self.start_rows.reorder_child_after(place, previous.as_ref());
            } else {
                self.start_rows.insert_child_after(place, previous.as_ref());
            }
            previous = Some(place.clone().upcast());
        }
        self.rows.replace(live);
    }

    /// A row of the start: its heading and side words, See all at the right, then its covers.
    fn start_row(self: &Rc<Self>, row: &ReadHomeRow) -> Rc<StartRow> {
        let title = widgets::heading("");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.update_property(&[gtk::accessible::Property::Level(2)]);
        let side = widgets::haze("", "caption");
        side.set_wrap(false);
        side.set_valign(gtk::Align::Center);
        side.set_visible(false);
        let face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        face.append(&gtk::Label::new(Some(&t::search_see_all())));
        face.append(&gtk::Image::from_icon_name("go-next-symbolic"));
        let all = gtk::Button::builder().child(&face).build();
        all.add_css_class("flat");
        all.set_visible(false);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.append(&title);
        top.append(&side);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        top.append(&spacer);
        top.append(&all);
        let landing = Landing::new(|| inset(discovery::grid_placeholders(1)));
        let place = gtk::Box::new(gtk::Orientation::Vertical, 8);
        place.set_margin_start(18);
        place.set_margin_end(18);
        place.append(&top);
        place.append(&landing.widget);
        let slot = Rc::new(StartRow {
            id: row.id.clone(),
            place,
            title,
            side,
            all: all.clone(),
            landing,
            view_all: RefCell::default(),
            signature: RefCell::default(),
        });
        all.connect_clicked({
            let (search, slot) = (Rc::downgrade(self), Rc::downgrade(&slot));
            move |_| {
                let (Some(search), Some(slot)) = (search.upgrade(), slot.upgrade()) else {
                    return;
                };
                let Some((query, line)) = slot.view_all.borrow().clone() else { return };
                search.see_all(&query, line);
            }
        });
        slot
    }

    /// From your sources sits 34 under AniList's grid, or at the top with nothing over it.
    fn place_sources(&self, alone: bool) {
        self.sources_head.set_margin_top(if alone { 4 } else { ABOVE_SOURCES });
    }

    fn heard(self: &Rc<Self>, event: &Event) {
        match event {
            Event::Reading(ReadingEvent::Search {
                query,
                page,
                tiles,
                has_more,
                other_scope,
                total,
            }) => {
                if *query != self.at.borrow().0 {
                    return;
                }
                self.results(query, *page, tiles, *has_more, *other_scope, *total);
            }
            Event::Reading(ReadingEvent::SourceSearch(search)) => {
                if search.query != self.at.borrow().0 {
                    return;
                }
                self.source.replace(Some((**search).clone()));
                self.draw_sources();
            }
            Event::Reading(ReadingEvent::Failed { about: ReadingAbout::Search, failure }) => {
                if self.asking_more.replace(false) {
                    self.spinner.set_visible(false);
                    return;
                }
                if self.at.borrow().0.text.is_empty() {
                    self.states.failed(&super::failure(failure));
                } else {
                    // The sources were still asked: AniList's part says it didn't answer.
                    self.anilist_failed.set(true);
                    self.anilist_in.set(true);
                    self.first_page.borrow_mut().clear();
                    self.draw_anilist();
                    self.draw_sources();
                }
            }
            Event::Reading(ReadingEvent::Home(view)) => {
                self.home_asked.set(true);
                let (this, view) = (Rc::downgrade(self), view.clone());
                self.shown.run("start", move || {
                    if let Some(this) = this.upgrade() {
                        this.draw_start(&view);
                    }
                });
            }
            Event::Medium(_)
                if self.shown.now() && self.hub.upgrade().is_some_and(|h| super::reads(&h)) =>
            {
                self.ask(true);
            }
            _ => {}
        }
    }

    fn results(
        self: &Rc<Self>,
        query: &ReadSearchQuery,
        page: u32,
        found: &[WorkTile],
        has_more: bool,
        other_scope: Option<u32>,
        total: Option<u32>,
    ) {
        let Some(hub) = self.hub.upgrade() else { return };
        let browsing = query.text.is_empty();
        if page == 0 {
            self.tiles.borrow_mut().clear();
            self.first_page.replace(found.to_vec());
        }
        self.at.borrow_mut().1 = page;
        self.tiles.borrow_mut().extend(found.iter().cloned());
        self.has_more.set(has_more);
        self.asking_more.set(false);
        self.spinner.set_visible(false);
        self.total.set(total.or((!has_more).then(|| self.tiles.borrow().len() as u32)));
        self.anilist_in.set(true);

        // While only the demo is on, a query lists its works in the demo's own rows.
        let only_demo = !browsing && hub.known.borrow().demo.only_books;
        if page == 0 {
            match other_scope.filter(|n| *n > 0 && !browsing) {
                Some(n) => {
                    self.other_words.set_label(&t::search_other_scope(
                        i64::from(n),
                        &t::library_scope_shows(),
                    ));
                    self.other.set_visible(true);
                }
                None => self.other.set_visible(false),
            }
            if only_demo {
                if let Some(ctx) = self.ctx.upgrade() {
                    let rows = super::demo::search_results(&ctx, &self.tiles.borrow());
                    self.anilist_landing.land(&rows);
                }
            } else {
                self.draw_anilist();
            }
        } else if browsing {
            add_posters(&hub, &self.grid.borrow(), found, self.caption(), &HashMap::new());
            self.set_line();
        } else {
            add_posters(&hub, &self.later, found, self.caption(), &self.joined.borrow());
            self.later.set_visible(true);
        }

        if browsing {
            if self.tiles.borrow().is_empty() {
                let set = self.filters.any_set();
                self.empty.set_title(&t::read_search_nothing_browse());
                self.empty.set_description(Some(&if set {
                    t::read_search_nothing_filters_line()
                } else {
                    t::read_search_nothing_line()
                }));
                self.clear.set_visible(set);
                self.states.empty();
            } else {
                self.states.content();
            }
        } else {
            self.states.content();
            self.draw_sources();
        }
        self.draw_more();
    }

    /// The captions of the query on screen.
    fn caption(&self) -> Caption {
        caption_for(&self.at.borrow().0)
    }

    /// The line over the grid: what a browse shows, or how many AniList found once it knows.
    fn set_line(&self) {
        let query = self.at.borrow().0.clone();
        let titled = self
            .titled
            .borrow()
            .as_ref()
            .filter(|(asked, _)| *asked == query)
            .map(|(_, line)| line.clone());
        let words = if let Some(line) = titled {
            Some(titled_line(&line, self.total.get()))
        } else if query.text.is_empty() {
            Some(browse_line(&query, &self.filters.choices, self.total.get()))
        } else {
            match self.total.get() {
                Some(n) if n > 0 => Some(t::read_search_total(i64::from(n))),
                Some(_) => None,
                // Answered with results and no exact count yet: the line without a number.
                None if !self.first_page.borrow().is_empty() => {
                    Some(t::read_search_total_unknown())
                }
                None => None,
            }
        };
        self.line.set_label(words.as_deref().unwrap_or_default());
        self.line.set_visible(words.is_some());
    }

    /// AniList's part: the line, the other side's count, a note when it found nothing or didn't
    /// answer, and its first page as a grid, with the sources' joins on the posters.
    fn draw_anilist(self: &Rc<Self>) {
        let Some(hub) = self.hub.upgrade() else { return };
        self.set_line();
        let part = gtk::Box::new(gtk::Orientation::Vertical, 0);
        for widget in [self.line.upcast_ref::<gtk::Widget>(), self.other.upcast_ref()] {
            if let Some(parent) = widget.parent().and_downcast::<gtk::Box>() {
                parent.remove(widget);
            }
            part.append(widget);
        }
        let failure = self.anilist_failed.get();
        let words = !self.at.borrow().0.text.is_empty();
        let tiles =
            if words { self.first_page.borrow().clone() } else { self.tiles.borrow().clone() };
        if failure {
            part.append(&note(
                &t::read_search_anilist_failed(),
                &t::read_search_anilist_failed_line(),
            ));
        } else if words && tiles.is_empty() {
            part.append(&note(&t::read_search_anilist_nothing(), ""));
        }
        let grid = poster_grid();
        add_posters(&hub, &grid, &tiles, self.caption(), &self.joined.borrow());
        grid.set_visible(!tiles.is_empty());
        part.append(&grid);
        self.grid.replace(grid.clone());
        self.anilist_landing.land(&part);
        // Opened by View all: the first result takes focus from Back, unless it has moved on.
        if self.focus_first.replace(false) && !tiles.is_empty() {
            let (back, first) = (self.back.clone(), grid.clone());
            motion::once_mapped(&grid, move || {
                if back.has_focus() || !back.is_mapped() {
                    first.child_focus(gtk::DirectionType::TabForward);
                }
            });
        }
    }

    /// More from AniList, with how many show, and of how many once AniList's count is exact.
    fn draw_more(&self) {
        let query = self.at.borrow().0.clone();
        let words = !query.text.is_empty();
        let show = words && !query.sources_only && self.anilist_in.get() && self.has_more.get();
        self.more_row.set_visible(show);
        let shown = self.tiles.borrow().len();
        // AniList's count is only ever exact: without it, how many show and nothing more.
        self.more_shown.set_label(&match self.total.get() {
            Some(total) => t::read_search_shown(shown as i64, i64::from(total)),
            None => t::read_search_shown_some(shown as i64),
        });
        self.more_shown.set_visible(show);
    }

    /// From your sources, as the newest send for the query has it.
    fn draw_sources(self: &Rc<Self>) {
        let Some(search) = self.source.borrow().clone() else { return };
        let Some(hub) = self.hub.upgrade() else { return };
        let query = self.at.borrow().0.clone();

        // Joins first: AniList's posters say which sources have them.
        let joined = joined_lines(&search);
        if *self.joined.borrow() != joined {
            self.joined.replace(joined);
            if self.anilist_in.get() && !self.anilist_failed.get() {
                self.draw_anilist();
            }
        }

        let mut old = self.slots.take();
        let mut live = Vec::new();
        for group in &search.groups {
            let slot = match old.iter().position(|s| s.id == group.source_id) {
                Some(at) => old.remove(at),
                None => self.new_group(group),
            };
            self.fill_group(&hub, &slot, group, &query);
            live.push(slot);
        }
        // Sources gone from the answer fade and close their gap.
        for slot in old {
            let (groups, place) = (self.groups.clone(), slot.part.place.clone());
            let search = Rc::downgrade(self);
            let gone = move || {
                if place.parent().is_some() {
                    groups.remove(&place);
                }
                if let Some(search) = search.upgrade() {
                    search.show_sources();
                }
            };
            if slot.part.is_shown() {
                slot.part.leave(gone);
            } else if !slot.part.place.is_closing() {
                gone();
            }
        }
        let mut previous: Option<gtk::Widget> = None;
        for slot in &live {
            let place = &slot.part.place;
            if place.parent().as_ref() == Some(self.groups.upcast_ref()) {
                self.groups.reorder_child_after(place, previous.as_ref());
            } else {
                self.groups.insert_child_after(place, previous.as_ref());
            }
            previous = Some(place.clone().upcast());
        }
        self.slots.replace(live);
        self.first_group_gap();

        let names = nothing_names(&search.groups);
        self.nothing_line.set_label(&t::read_search_source_nothing(&discovery::names(&names)));
        if names.is_empty() {
            self.nothing_line.set_visible(false);
        } else if !self.nothing_line.get_visible() {
            self.nothing_line.set_visible(true);
            motion::fade_in(&self.nothing_line);
        }
        self.show_sources();

        // Nothing anywhere: AniList found nothing (or wasn't asked) and every source nothing.
        let anilist_nothing = query.sources_only
            || (self.anilist_in.get()
                && !self.anilist_failed.get()
                && self.first_page.borrow().is_empty());
        let all_nothing = search.groups.iter().all(|g| g.state == ReadSourceState::Nothing);
        let nowhere = search.done && anilist_nothing && all_nothing;
        // Nothing anywhere takes the page's place as the shows' empty state, or gives it back.
        if nowhere {
            self.empty.set_title(&t::read_search_nothing(&query.text));
            let line = t::read_search_nothing_line();
            self.empty.set_description(Some(
                &if !query.sources_only && !search.groups.is_empty() {
                    t::read_linux_said_then(&line, &t::read_search_sources_nothing())
                } else {
                    line
                },
            ));
            self.clear.set_visible(self.filters.any_set());
            self.nothing.set(true);
            self.states.empty();
        } else if self.nothing.replace(false) {
            self.states.content();
        }
        if !nowhere && !query.sources_only && !self.anilist_place.is_closing() {
            self.anilist_place.set_visible(true);
        }
        self.draw_more();

        if search.done && !self.announced.replace(true) {
            let anilist = (!query.sources_only).then(|| {
                (!self.anilist_failed.get()).then(|| {
                    self.total.get().map_or(self.first_page.borrow().len(), |n| n as usize)
                })
            });
            let words = if nowhere {
                t::read_search_nothing(&query.text)
            } else {
                settled_words(anilist, &search.groups)
            };
            self.page
                .upcast_ref::<gtk::Widget>()
                .announce(&words, gtk::AccessibleAnnouncementPriority::Medium);
        }
    }

    /// The first source's row sits 12 under the section's heading; the rest 26 apart.
    fn first_group_gap(&self) {
        let mut first = true;
        for slot in self.slots.borrow().iter() {
            slot.part.inner.set_margin_top(if first { 12 } else { ABOVE_SOURCE });
            first &= !slot.part.place.get_visible();
        }
    }

    /// From your sources shows while a row or the line of sources that found nothing does,
    /// fading in when it comes and out when it goes; never over Nothing anywhere.
    fn show_sources(&self) {
        let rows = self.slots.borrow().iter().any(|s| s.part.place.get_visible());
        let show = rows || self.nothing_line.get_visible();
        // Fading out counts as gone, so a row that comes back meanwhile stops the fade.
        let leaving = self
            .sources_fade
            .borrow()
            .as_ref()
            .is_some_and(|fade| fade.state() == adw::AnimationState::Playing);
        let shown = self.sources.get_visible() && !leaving;
        if show && !shown {
            let from = self.sources.opacity();
            if let Some(fade) = self.sources_fade.take() {
                fade.pause();
            }
            self.sources.set_visible(true);
            if leaving && self.page.is_mapped() {
                // Back from part way out: it fades in from where it got to.
                let back = motion::timed(
                    &self.sources,
                    from,
                    1.0,
                    200,
                    motion::opacity(&self.sources),
                );
                back.set_easing(adw::Easing::EaseOutCubic);
                back.play();
            } else {
                // The heading fades in; the rows and the line under it fade on their own.
                self.sources.set_opacity(1.0);
                if self.page.is_mapped() {
                    motion::fade_in(&self.sources_head);
                }
            }
        } else if !show && shown {
            if self.page.is_mapped() {
                self.sources_fade.replace(Some(motion::fade_out(&self.sources)));
            } else {
                self.sources.set_visible(false);
            }
        }
    }

    fn new_group(self: &Rc<Self>, group: &ReadSourceGroup) -> Rc<GroupSlot> {
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.set_margin_bottom(10);
        let name = gtk::Label::new(Some(&group.source_name));
        name.add_css_class("read-source-name");
        name.set_xalign(0.0);
        // A long name ends in an ellipsis rather than widening the page; the kind after it
        // shrinks first.
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_width_chars(4);
        name.set_tooltip_text(Some(&group.source_name));
        name.set_accessible_role(gtk::AccessibleRole::Heading);
        name.update_property(&[gtk::accessible::Property::Level(3)]);
        head.append(&name);
        let kind = widgets::haze("", "read-source-kind");
        kind.set_wrap(false);
        kind.set_ellipsize(gtk::pango::EllipsizeMode::End);
        kind.set_width_chars(2);
        kind.set_accessible_role(gtk::AccessibleRole::Presentation);
        head.append(&kind);
        let spinner = adw::Spinner::new();
        spinner.set_size_request(12, 12);
        spinner.set_valign(gtk::Align::Center);
        spinner.set_visible(false);
        spinner
            .upcast_ref::<gtk::Widget>()
            .set_accessible_role(gtk::AccessibleRole::Presentation);
        head.append(&spinner);
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
        inner.append(&head);
        let landing = Landing::new(|| discovery::placeholders(SOURCE_PLACEHOLDERS));
        // A source's covers stand in from its name, as the shows' rails' covers do.
        landing.widget.set_margin_start(COVERS_IN);
        landing.widget.set_margin_end(COVERS_IN);
        inner.append(&landing.widget);
        let part = Part::new(inner, landing, {
            let search = Rc::downgrade(self);
            move || {
                if let Some(search) = search.upgrade() {
                    search.first_group_gap();
                    search.show_sources();
                }
            }
        });
        Rc::new(GroupSlot {
            id: group.source_id.clone(),
            part,
            name,
            kind,
            spinner,
            signature: RefCell::default(),
            failed: Cell::new(false),
        })
    }

    fn fill_group(
        self: &Rc<Self>,
        hub: &Rc<Hub>,
        slot: &Rc<GroupSlot>,
        group: &ReadSourceGroup,
        query: &ReadSearchQuery,
    ) {
        slot.name.set_label(&group.source_name);
        // Server, Folder or Plugin beside the name; the demo shows its name alone.
        let kind = group.kind.map(|kind| match kind {
            ReadSourceKind::Server => t::read_search_source_kind_server(),
            ReadSourceKind::Folder => t::read_search_source_kind_folder(),
            ReadSourceKind::Plugin => t::read_search_source_kind_plugin(),
        });
        slot.kind.set_label(kind.as_deref().unwrap_or_default());
        slot.kind.set_visible(kind.is_some());
        let heading = match &kind {
            Some(kind) => t::read_linux_source_with_kind(&group.source_name, kind),
            None => group.source_name.clone(),
        };
        let asking = group.state == ReadSourceState::Asking;
        slot.spinner.set_visible(asking);
        slot.part.landing.widget.update_state(&[gtk::accessible::State::Busy(asking)]);
        if asking {
            slot.name.update_property(&[gtk::accessible::Property::Label(
                &t::read_linux_source_searching(&heading),
            )]);
            slot.signature.replace(String::new());
            slot.part.landing.wait(!slot.failed.get());
            return;
        }
        slot.name.update_property(&[gtk::accessible::Property::Label(&heading)]);
        let signature = format!("{:?}{:?}", group.state, group.tiles);
        if *slot.signature.borrow() == signature {
            return;
        }
        slot.signature.replace(signature);
        match &group.state {
            ReadSourceState::Hits if !group.tiles.is_empty() => {
                slot.failed.set(false);
                let posters: Vec<gtk::Button> = group
                    .tiles
                    .iter()
                    .map(|tile| discovery::poster(hub, tile, Caption::Amount, None, false))
                    .collect();
                let strip = discovery::strip(&posters, &group.source_name);
                slot.part.landing.land(&strip);
                slot.part.reveal();
            }
            ReadSourceState::Failed { failure } => {
                slot.failed.set(true);
                let block = self.failure(group, &super::failure(failure), None, query);
                slot.part.landing.land(&block);
                slot.part.reveal();
            }
            ReadSourceState::TimedOut { seconds } => {
                slot.failed.set(true);
                let reason = t::read_search_source_timeout(i64::from(*seconds));
                let spoken = t::read_linux_source_timeout(i64::from(*seconds));
                let block = self.failure(group, &reason, Some(&spoken), query);
                slot.part.landing.land(&block);
                slot.part.reveal();
            }
            // Nothing, or every hit joined an AniList result: the row goes.
            _ => {
                slot.failed.set(false);
                let search = Rc::downgrade(self);
                slot.part.leave(move || {
                    if let Some(search) = search.upgrade() {
                        search.first_group_gap();
                        search.show_sources();
                    }
                });
            }
        }
    }

    /// A source that didn't answer: the glyph, the words, the reason and Try again for it alone.
    fn failure(
        &self,
        group: &ReadSourceGroup,
        reason: &str,
        spoken: Option<&str>,
        query: &ReadSearchQuery,
    ) -> gtk::Box {
        let block = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        block.add_css_class("read-source-failed");
        let glyph = gtk::Image::from_icon_name("dialog-warning-symbolic");
        glyph.set_pixel_size(17);
        glyph.add_css_class("read-fail-glyph");
        glyph.set_valign(gtk::Align::Start);
        glyph.set_accessible_role(gtk::AccessibleRole::Presentation);
        block.append(&glyph);
        let words = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let line = gtk::Label::new(Some(&t::read_search_source_failed(&group.source_name)));
        line.add_css_class("read-fail-line");
        line.set_xalign(0.0);
        line.set_wrap(true);
        words.append(&line);
        let why = widgets::haze(reason, "read-fail-reason");
        if let Some(spoken) = spoken {
            why.update_property(&[gtk::accessible::Property::Label(spoken)]);
        }
        why.set_margin_top(2);
        why.set_margin_bottom(10);
        words.append(&why);
        let again = gtk::Button::with_label(&t::home_try_again());
        again.add_css_class("home-sec");
        again.set_halign(gtk::Align::Start);
        // Several sources may fail: each Try again is named with its own.
        again.update_property(&[gtk::accessible::Property::Label(
            &t::read_linux_try_again_source(&group.source_name),
        )]);
        again.connect_clicked({
            let (hub, query, id) = (self.hub.clone(), query.clone(), group.source_id.clone());
            move |_| {
                if let Some(hub) = hub.upgrade() {
                    send(
                        &hub,
                        ReadingCommand::RetrySourceSearch {
                            query: query.clone(),
                            source_id: id.clone(),
                        },
                    );
                }
            }
        });
        words.append(&again);
        block.append(&words);
        block
    }
}

/// A row of the start as the engine sent it: the heading, See all, and the covers once in.
fn fill_start_row(hub: &Hub, slot: &StartRow, row: &ReadHomeRow) {
    let title = say::msg(&row.title);
    slot.title.set_label(&title);
    let side = row.side.as_ref().map(say::msg);
    slot.side.set_label(side.as_deref().unwrap_or_default());
    slot.side.set_visible(side.is_some());
    slot.view_all
        .replace(row.view_all.clone().map(|q| (q, row.view_all_title.as_ref().map(say::msg))));
    slot.all.set_visible(row.view_all.is_some());
    slot.all.update_property(&[gtk::accessible::Property::Label(&t::read_linux_see_all_row(
        &title,
    ))]);
    if row.pending {
        slot.title.update_property(&[gtk::accessible::Property::Label(
            &t::read_linux_row_loading(&title),
        )]);
        slot.place.set_visible(true);
        slot.landing.wait(true);
        return;
    }
    slot.title.update_property(&[gtk::accessible::Property::Label(&title)]);
    // A row with nothing to show stays out of the start.
    slot.place.set_visible(!row.tiles.is_empty());
    let signature = format!("{:?}", row.tiles);
    if *slot.signature.borrow() == signature && slot.landing.is_visible() {
        return;
    }
    slot.signature.replace(signature);
    let grid = poster_grid();
    let shown = &row.tiles[..row.tiles.len().min(START_SHOWN)];
    add_posters(hub, &grid, shown, Caption::row(row.kind), &HashMap::new());
    slot.landing.land(&grid);
}

/// A line over AniList's part: AniList found nothing, or didn't answer, with a line under it.
fn note(title: &str, line: &str) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 2);
    column.set_margin_bottom(12);
    let head = gtk::Label::new(Some(title));
    head.add_css_class("read-search-note");
    head.set_xalign(0.0);
    head.set_wrap(true);
    column.append(&head);
    if !line.is_empty() {
        column.append(&widgets::haze(line, "read-fail-reason"));
    }
    column
}

/// For a screenshot, as the shows' filters shot: a genre left out, two formats and a status,
/// browsing with nothing typed.
#[cfg(feature = "fixtures")]
pub fn shot_filters(shell: &Rc<crate::window::Shell>) {
    fn first<T: IsA<gtk::Widget>>(widget: &gtk::Widget) -> Option<T> {
        if let Some(found) = widget.downcast_ref::<T>()
            && widget.is_mapped()
        {
            return Some(found.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = first(&next) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    fn first_filter(widget: &gtk::Widget) -> Option<gtk::MenuButton> {
        if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
            && widget.is_mapped()
            && widget.has_css_class("filter")
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = first_filter(&next) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    let window = shell.ctx.window.clone();
    glib::timeout_add_local_once(Duration::from_millis(1500), move || {
        if let Some(entry) = first::<gtk::SearchEntry>(window.upcast_ref()) {
            entry.set_text("");
        }
        let Some(button) = first_filter(window.upcast_ref()) else { return };
        let left_out = read_filters().genres.iter().position(|g| g == "Romance").unwrap_or(0); // i18n-skip
        for action in [
            format!("rfilter.leave-{left_out}"),
            "rfilter.format-manga".into(),    // i18n-skip
            "rfilter.format-webtoon".into(),  // i18n-skip
            "rfilter.status-finished".into(), // i18n-skip
        ] {
            let _ = button.activate_action(&action, None);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_model::{
        Msg, ReadingMsg,
        reading::{ReadAnilistState, ReadSourceJoin, ReadingFailure},
    };

    fn group(name: &str, state: ReadSourceState, hits: usize, joined: u32) -> ReadSourceGroup {
        let tile = WorkTile {
            work: kaeri_core::books::WorkId(format!("s:{name}")),
            title: "Paper Lantern Records".into(),
            medium: kaeri_core::media::Medium::Manga,
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
        };
        ReadSourceGroup {
            source_id: name.to_lowercase(),
            source_name: name.into(),
            kind: None,
            state,
            tiles: vec![tile; hits],
            joined,
        }
    }

    #[test]
    fn results_are_captioned_for_how_they_were_asked() {
        let mut query = ReadSearchQuery { text: "lantern".into(), ..Default::default() };
        assert_eq!(caption_for(&query), Caption::FormatOrigin);
        query.text.clear();
        assert_eq!(caption_for(&query), Caption::FormatStatus);
        query.formats = ReadRowFormat::Manga.search_formats();
        assert_eq!(caption_for(&query), Caption::OriginStatus);
        query.started = Some("12-months".into());
        assert_eq!(caption_for(&query), Caption::OriginStarted);
        query.started = None;
        query.formats.push("webtoon".into());
        assert_eq!(caption_for(&query), Caption::FormatStatus);
    }

    #[test]
    fn the_sources_that_found_nothing_share_one_line() {
        let groups = [
            group("Home library", ReadSourceState::Hits, 2, 1),
            group("Comics folder", ReadSourceState::Nothing, 0, 0),
            group("Demo", ReadSourceState::Nothing, 0, 0),
        ];
        assert_eq!(discovery::names(&nothing_names(&groups)), "Comics folder and Demo");
    }

    #[test]
    fn a_settled_search_is_said_once_with_each_source_that_did_not_answer() {
        let groups = [
            group("Home library", ReadSourceState::Hits, 2, 1),
            group("Northwind", ReadSourceState::TimedOut { seconds: 15 }, 0, 0),
            group(
                "Eastport",
                ReadSourceState::Failed { failure: ReadingFailure::Offline },
                0,
                0,
            ),
        ];
        assert_eq!(
            settled_words(Some(Some(4)), &groups),
            "4 from AniList, 3 from your sources. Northwind didn't answer. Eastport didn't answer."
        );
        assert_eq!(settled_words(None, &groups[..1]), "3 from your sources.");
        assert_eq!(
            settled_words(Some(None), &groups[..1]),
            "AniList didn't answer. 3 from your sources."
        );
    }

    #[test]
    fn a_rows_formats_read_as_one() {
        let choice = |key: &str, label: ReadingMsg| ReadFilterChoice {
            key: key.into(),
            label: Msg::Reading(label),
        };
        let choices = [
            choice("manga", ReadingMsg::ReadingKindManga),
            choice("one-shot", ReadingMsg::ReadingKindOneShot),
        ];
        let manga = ReadRowFormat::Manga.search_formats();
        assert_eq!(format_labels(&manga, &choices), [say::msg(&choices[0].label)]);
        assert_eq!(
            format_labels(&["one-shot".into()], &choices),
            [say::msg(&choices[1].label)]
        );
    }

    #[test]
    fn a_result_in_one_source_names_it_and_in_more_counts_them() {
        let join = |work: &str, names: &[&str]| ReadSourceJoin {
            work: kaeri_core::books::WorkId(work.into()),
            source_ids: names.iter().map(|n| n.to_lowercase()).collect(),
            source_names: names.iter().map(|n| (*n).to_owned()).collect(),
        };
        let search = ReadSourceSearch {
            query: ReadSearchQuery::default(),
            anilist: ReadAnilistState::Answered,
            groups: Vec::new(),
            joins: vec![
                join("a:1", &["Home library"]),
                join("a:2", &["Home library", "Comics folder"]),
            ],
            done: true,
        };
        let lines = joined_lines(&search);
        assert_eq!(lines["a:1"], "In Home library");
        assert_eq!(lines["a:2"], "In 2 of your sources");
    }

    #[test]
    fn any_time_is_no_started_filter() {
        let mut query = ReadSearchQuery { started: Some("any".into()), ..Default::default() };
        assert_eq!(started_of(&query), None);
        query.started = Some("12-months".into());
        assert_eq!(started_of(&query), Some("12-months"));
    }
}
