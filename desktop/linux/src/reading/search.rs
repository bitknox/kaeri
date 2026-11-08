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
    event::Event,
    reading::{
        ReadFilterChoice, ReadFilters, ReadSearchQuery, ReadSourceGroup, ReadSourceSearch,
        ReadSourceState, ReadingAbout, ReadingCommand, ReadingEvent, WorkTile, read_filters,
    },
};

use super::{
    Dest,
    discovery::{self, Landing},
    listen, scope, send,
};
use crate::{
    hub::Hub,
    motion,
    pages::search_filters::{faded_scroll, menu_button, set_label},
    say,
    squeeze::Squeeze,
    states::{Shape, States},
    widgets,
    window::Ctx,
};

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
        _ => t::read_sort_popular(),
    }
}

/// The words for `keys` among `choices`.
fn labels(keys: &[String], choices: &[ReadFilterChoice]) -> Vec<String> {
    keys.iter()
        .filter_map(|k| choices.iter().find(|c| c.key == *k).map(|c| say::msg(&c.label)))
        .collect()
}

/// "Manga", or "Manga +1" when more than one is chosen.
fn many(words: &[String]) -> Option<String> {
    let first = words.first()?;
    Some(if words.len() == 1 {
        first.clone()
    } else {
        t::read_filter_many(first, words.len() as i64 - 1)
    })
}

/// When a series started, as the query keeps it: `None` for any time.
fn started_of(query: &ReadSearchQuery) -> Option<&str> {
    query.started.as_deref().filter(|s| !s.is_empty() && *s != "any")
}

/// What a browse shows: the filters set, or the order with none.
fn browse_line(query: &ReadSearchQuery, filters: &ReadFilters, total: Option<u32>) -> String {
    let mut parts = labels(&query.formats, &filters.formats);
    parts.extend(labels(&query.origins, &filters.origins));
    if let Some(status) = &query.status {
        parts.extend(labels(std::slice::from_ref(status), &filters.statuses));
    }
    if let Some(started) = started_of(query) {
        parts.extend(labels(&[started.to_owned()], &filters.started));
    }
    parts.extend(query.genres.iter().map(|g| say::genre(g)));
    let what = if parts.is_empty() {
        sort_name(
            query.sort.as_deref().or(filters.sorts.first().map(String::as_str)).unwrap_or(""),
        )
    } else {
        parts.join(", ") // i18n-skip
    };
    match total {
        Some(n) => format!("{what} \u{b7} {}", t::read_search_total(i64::from(n))), // i18n-skip
        None => t::read_search_browse_line(&what),
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
/// sources." with each source that didn't answer after it.
fn settled_words(anilist: Option<usize>, groups: &[ReadSourceGroup]) -> String {
    let found: usize = groups
        .iter()
        .filter(|g| g.state == ReadSourceState::Hits)
        .map(|g| g.tiles.len() + g.joined as usize)
        .sum();
    let mut said = match anilist {
        Some(n) => t::read_linux_search_done(n as i64, found as i64),
        None => t::read_linux_search_done_sources(found as i64),
    };
    for group in groups {
        if matches!(
            group.state,
            ReadSourceState::Failed { .. } | ReadSourceState::TimedOut { .. }
        ) {
            said = format!("{said} {}", t::read_search_source_failed(&group.source_name)); // i18n-skip
        }
    }
    said
}

/// The filters and the order, and the query they make.
struct Filters {
    row: gtk::Box,
    format: gtk::MenuButton,
    origin: gtk::MenuButton,
    status: gtk::MenuButton,
    genres: gtk::MenuButton,
    started: gtk::MenuButton,
    rule: gtk::Separator,
    mine: gtk::ToggleButton,
    mine_check: gtk::Image,
    sort: gtk::MenuButton,
    sort_label: gtk::Label,
    actions: gio::SimpleActionGroup,
    choices: ReadFilters,
    query: RefCell<ReadSearchQuery>,
    changed: RefCell<Option<Rc<dyn Fn()>>>,
    /// The switch is being set to the query's state, not by the person.
    settling: Cell<bool>,
}

impl Filters {
    fn new() -> Rc<Self> {
        let format = menu_button(&t::read_filter_format());
        let origin = menu_button(&t::read_filter_origin());
        let status = menu_button(&t::read_filter_status());
        let genres = menu_button(&t::read_filter_genres());
        let started = menu_button(&t::read_search_started());
        let sort_label = gtk::Label::new(None);
        let sort_line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        sort_line.append(&gtk::Image::from_icon_name("view-sort-descending-symbolic"));
        sort_line.append(&sort_label);
        let sort = gtk::MenuButton::builder().child(&sort_line).always_show_arrow(true).build();
        sort.add_css_class("flat");
        sort.add_css_class("filter");
        // Your sources only, after a rule: off it is outlined, on it is raised with a check.
        let rule = gtk::Separator::new(gtk::Orientation::Vertical);
        rule.add_css_class("read-filter-rule");
        rule.set_valign(gtk::Align::Center);
        rule.set_visible(false);
        let mine_check = gtk::Image::from_icon_name("object-select-symbolic");
        mine_check.set_pixel_size(12);
        mine_check.set_visible(false);
        let mine_words = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        mine_words.append(&mine_check);
        mine_words.append(&gtk::Label::new(Some(&t::read_search_sources_only())));
        let mine = gtk::ToggleButton::builder().child(&mine_words).build();
        mine.add_css_class("read-sources-only");
        mine.set_valign(gtk::Align::Center);
        mine.set_visible(false);
        mine.set_accessible_role(gtk::AccessibleRole::Switch);
        mine.update_property(&[gtk::accessible::Property::Label(
            &t::read_search_sources_only(),
        )]);
        let capsules = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for button in [&format, &origin, &status, &genres, &started] {
            capsules.append(button);
        }
        capsules.append(&rule);
        capsules.append(&mine);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_margin_start(12);
        row.set_margin_end(12);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.append(&faded_scroll(&capsules));
        row.append(&sort);
        let actions = gio::SimpleActionGroup::new();
        row.insert_action_group("rfilter", Some(&actions));
        let filters = Rc::new(Self {
            row,
            format,
            origin,
            status,
            genres,
            started,
            rule,
            mine,
            mine_check,
            sort,
            sort_label,
            actions,
            choices: read_filters(),
            query: RefCell::default(),
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
        filters.refresh();
        filters
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

    /// Words were typed or cleared: Your sources only shows only with words.
    fn set_words(&self, words: bool) {
        self.rule.set_visible(words);
        self.mine.set_visible(words);
    }

    /// Every filter off; the order and Your sources only stay.
    fn clear(self: &Rc<Self>) {
        self.edit(|q| ReadSearchQuery {
            sort: q.sort.clone(),
            sources_only: q.sources_only,
            ..ReadSearchQuery::default()
        });
    }

    /// Take `query`'s filters and order as they are, as View all on a Read row hands them over.
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

    /// Format or Origin: any number of each.
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
        let menu = gio::Menu::new();
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
            menu.append(Some(&say::msg(&choice.label)), Some(&format!("rfilter.{name}")));
        }
        button.set_menu_model(Some(&menu));
        let words = labels(picked, list);
        set_label(button, &many(&words).unwrap_or(title), !words.is_empty());
    }

    fn refresh(self: &Rc<Self>) {
        self.several(false);
        self.several(true);
        let query = self.query.borrow().clone();
        let choices = &self.choices;

        // Status: one, or any.
        let any = gio::Menu::new();
        self.check("status-any", query.status.is_none(), |q| ReadSearchQuery {
            status: None,
            ..q.clone()
        });
        any.append(Some(&t::read_filter_any()), Some("rfilter.status-any"));
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
        menu.append_section(None, &any);
        menu.append_section(None, &statuses);
        self.status.set_menu_model(Some(&menu));
        let status = query.status.as_ref().and_then(|s| {
            labels(std::slice::from_ref(s), &choices.statuses).into_iter().next()
        });
        set_label(
            &self.status,
            status.as_deref().unwrap_or(&t::read_filter_status()),
            status.is_some(),
        );

        // Genres: included, or left out.
        let included = gio::Menu::new();
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
            included.append(Some(&say::genre(genre)), Some(&format!("rfilter.{want}")));
            left_out.append(Some(&say::genre(genre)), Some(&format!("rfilter.{leave}")));
        }
        let menu = gio::Menu::new();
        menu.append_submenu(Some(&t::read_filter_included()), &included);
        menu.append_submenu(Some(&t::read_filter_left_out()), &left_out);
        self.genres.set_menu_model(Some(&menu));
        let set = query.genres.len() + query.exclude_genres.len();
        let label = match (set, query.genres.first(), query.exclude_genres.first()) {
            (0, ..) => t::read_filter_genres(),
            (1, Some(genre), _) => say::genre(genre),
            (1, None, Some(genre)) => t::read_filter_not_genre(&say::genre(genre)),
            (n, ..) => t::read_filter_genres_count(n as i64),
        };
        set_label(&self.genres, &label, set > 0);

        // Started: any time, or one of the spans.
        let started = started_of(&query).map(str::to_owned);
        let spans = gio::Menu::new();
        for choice in &choices.started {
            let name = format!("started-{}", choice.key);
            let key = (choice.key != "any").then(|| choice.key.clone());
            self.check(&name, started == key, move |q| ReadSearchQuery {
                started: key.clone(),
                ..q.clone()
            });
            spans.append(Some(&say::msg(&choice.label)), Some(&format!("rfilter.{name}")));
        }
        self.started.set_menu_model(Some(&spans));
        let words = started
            .as_ref()
            .and_then(|s| labels(std::slice::from_ref(s), &choices.started).into_iter().next());
        set_label(
            &self.started,
            words.as_deref().unwrap_or(&t::read_search_started()),
            words.is_some(),
        );

        // The order.
        let current =
            query.sort.clone().or_else(|| choices.sorts.first().cloned()).unwrap_or_default();
        let sorts = gio::Menu::new();
        for key in &choices.sorts {
            let name = format!("sort-{key}");
            let chosen = key.clone();
            self.check(&name, *key == current, move |q| ReadSearchQuery {
                sort: Some(chosen.clone()),
                ..q.clone()
            });
            sorts.append(Some(&sort_name(key)), Some(&format!("rfilter.{name}")));
        }
        self.sort.set_menu_model(Some(&sorts));
        self.sort_label.set_label(&sort_name(&current));

        // Your sources only: the other filters are AniList's, so they dim while it is on.
        let mine = query.sources_only;
        self.settling.set(true);
        self.mine.set_active(mine);
        self.settling.set(false);
        self.mine_check.set_visible(mine);
        self.mine.update_state(&[gtk::accessible::State::Checked(if mine {
            gtk::AccessibleTristate::True
        } else {
            gtk::AccessibleTristate::False
        })]);
        for button in [&self.format, &self.origin, &self.status, &self.genres, &self.started] {
            dim(button, mine);
        }
        dim(&self.sort, mine);
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
    grid
}

fn add_posters(
    hub: &Hub,
    grid: &gtk::Box,
    tiles: &[WorkTile],
    joined: &HashMap<String, String>,
) {
    for tile in tiles {
        let line = joined.get(&tile.work.0).map(String::as_str);
        grid.append(&discovery::item(&discovery::poster(hub, tile, line)));
    }
}

/// One source's row in From your sources.
struct GroupSlot {
    id: String,
    place: Squeeze,
    inner: gtk::Box,
    name: gtk::Label,
    kind: gtk::Label,
    spinner: adw::Spinner,
    landing: Rc<Landing>,
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
    nothing_line: gtk::Label,
    slots: RefCell<Vec<Rc<GroupSlot>>>,
    /// AniList's later pages, under the sources, and More from AniList.
    later: gtk::Box,
    more_row: gtk::Box,
    more_shown: gtk::Label,
    spinner: adw::Spinner,
    /// Nothing found anywhere.
    nothing: gtk::Box,
    nothing_title: gtk::Label,
    nothing_line_one: gtk::Label,
    nothing_line_two: gtk::Label,
    /// The empty state while browsing, with Clear filters.
    empty: adw::StatusPage,
    clear: gtk::Button,

    /// The query asked and the last page of it in, from 0.
    at: RefCell<(ReadSearchQuery, u32)>,
    asked: Cell<bool>,
    tiles: RefCell<Vec<WorkTile>>,
    first_page: RefCell<Vec<WorkTile>>,
    has_more: Cell<bool>,
    asking_more: Cell<bool>,
    total: Cell<Option<u32>>,
    anilist_failed: Cell<bool>,
    anilist_in: Cell<bool>,
    source: RefCell<Option<ReadSourceSearch>>,
    joined: RefCell<HashMap<String, String>>,
    announced: Cell<bool>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    // AniList's part.
    let line = widgets::haze("", "read-search-total");
    line.set_visible(false);
    let other_words = widgets::haze("", "caption");
    let show_them = gtk::Button::with_label(&t::search_show_them());
    show_them.add_css_class("flat");
    let other = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    other.set_margin_bottom(12);
    other.set_visible(false);
    other.append(&other_words);
    other.append(&show_them);
    let anilist_landing =
        Landing::new(|| discovery::grid_placeholders(ANILIST_PLACEHOLDER_ROWS));
    let anilist = gtk::Box::new(gtk::Orientation::Vertical, 0);
    anilist.append(&anilist_landing.widget);
    let anilist_place = Squeeze::new(&anilist, gtk::Orientation::Vertical, 0);

    // From your sources.
    let sources_head = gtk::Label::new(Some(&t::read_search_from_sources()));
    sources_head.add_css_class("read-search-section");
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

    // Nothing found anywhere.
    let nothing_title = gtk::Label::new(None);
    nothing_title.add_css_class("read-search-nothing-title");
    nothing_title.set_xalign(0.0);
    nothing_title.set_wrap(true);
    nothing_title.set_accessible_role(gtk::AccessibleRole::Heading);
    let nothing_line_one = widgets::haze("", "read-search-nothing-text");
    let nothing_line_two = widgets::haze("", "read-search-nothing-text");
    nothing_line_two.set_margin_top(6);
    let nothing = gtk::Box::new(gtk::Orientation::Vertical, 6);
    nothing.set_margin_top(6);
    nothing.append(&nothing_title);
    nothing.append(&nothing_line_one);
    nothing.append(&nothing_line_two);
    nothing.set_visible(false);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.add_css_class("read-search-body");
    body.append(&nothing);
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
    let clear = gtk::Button::with_label(&t::read_search_clear_filters());
    clear.add_css_class("pill");
    clear.set_visible(false);
    let empty =
        crate::states::plain(&t::read_search_nothing_browse(), &t::read_search_nothing_line());
    clear.set_halign(gtk::Align::Center);
    empty.set_child(Some(&clear));
    let states = States::new(&ctx.hub, &scroll, Shape::Grid, &empty, &t::read_search_failed());

    let entry = gtk::SearchEntry::builder()
        .placeholder_text(t::read_search_placeholder())
        .hexpand(true)
        .build();
    let clamp = adw::Clamp::builder().maximum_size(520).child(&entry).build();
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
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
    header.set_title_widget(Some(&clamp));
    let filters = Filters::new();
    // Shows or Reading leads the content bar, where Library has it.
    filters.row.prepend(&scope(ctx, Dest::Search, true));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&filters.row);
    states.widget.set_vexpand(true);
    content.append(&states.widget);
    let page = widgets::content_page(&t::nav_search(), &header, &content);

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
        groups,
        nothing_line,
        slots: RefCell::default(),
        later,
        more_row,
        more_shown,
        spinner,
        nothing,
        nothing_title,
        nothing_line_one,
        nothing_line_two,
        empty,
        clear: clear.clone(),
        at: RefCell::new((ReadSearchQuery::default(), 0)),
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
        move |query| {
            if let Some(search) = search.upgrade() {
                search.filters.adopt(&query);
                search.entry.set_text("");
                search.filters.set_words(false);
                search.ask(true);
            }
        }
    })));
    // Shown with nothing asked yet, as when reading was turned on after the window opened.
    page.connect_shown({
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
    if super::reads(&ctx.hub) {
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
        self.nothing.set_visible(false);
        // The same words with Your sources only flipped keep the sources' rows where they are
        // until their new answers land; other words start the section again.
        if !same_words {
            self.source.replace(None);
            self.joined.borrow_mut().clear();
            for slot in self.slots.take() {
                self.groups.remove(&slot.place);
            }
            self.sources.set_visible(false);
        }
        self.show_anilist(!query.sources_only);
        if words {
            self.states.content();
            if !query.sources_only {
                self.anilist_landing.wait(false);
            }
        } else {
            self.states.loading(None);
        }
        send(&hub, ReadingCommand::Search { query });
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
        let shown = place.is_visible() && !place.is_closing();
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
        let animation = adw::TimedAnimation::new(
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
            Event::Medium(_) if self.hub.upgrade().is_some_and(|h| super::reads(&h)) => {
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
                    self.anilist_landing.land(&rows, false);
                }
            } else {
                self.draw_anilist();
            }
        } else if browsing {
            add_posters(&hub, &self.grid.borrow(), found, &HashMap::new());
            self.set_line();
        } else {
            add_posters(&hub, &self.later, found, &self.joined.borrow());
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

    /// The line over the grid: what a browse shows, or how many AniList found once it knows.
    fn set_line(&self) {
        let query = self.at.borrow().0.clone();
        let words = if query.text.is_empty() {
            Some(browse_line(&query, &self.filters.choices, self.total.get()))
        } else {
            self.total.get().filter(|n| *n > 0).map(|n| t::read_search_total(i64::from(n)))
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
        add_posters(&hub, &grid, &tiles, &self.joined.borrow());
        grid.set_visible(!tiles.is_empty());
        part.append(&grid);
        self.grid.replace(grid);
        self.anilist_landing.land(&part, failure);
    }

    /// More from AniList, with how many show once AniList has said how many there are.
    fn draw_more(&self) {
        let query = self.at.borrow().0.clone();
        let words = !query.text.is_empty();
        let show = words && !query.sources_only && self.anilist_in.get() && self.has_more.get();
        self.more_row.set_visible(show);
        let shown = self.tiles.borrow().len();
        match self.total.get() {
            Some(total) if show => {
                self.more_shown
                    .set_label(&t::read_search_shown(shown as i64, i64::from(total)));
                self.more_shown.set_visible(true);
            }
            _ => self.more_shown.set_visible(false),
        }
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
        for slot in old {
            self.groups.remove(&slot.place);
        }
        let mut previous: Option<gtk::Widget> = None;
        for slot in &live {
            if slot.place.parent().as_ref() == Some(self.groups.upcast_ref()) {
                self.groups.reorder_child_after(&slot.place, previous.as_ref());
            } else {
                self.groups.insert_child_after(&slot.place, previous.as_ref());
            }
            previous = Some(slot.place.clone().upcast());
        }
        self.slots.replace(live);
        self.first_group_gap();

        let names = nothing_names(&search.groups);
        self.nothing_line.set_label(&t::read_search_source_nothing(&discovery::names(&names)));
        self.nothing_line.set_visible(!names.is_empty());
        let any_shown = self.slots.borrow().iter().any(|s| s.place.is_visible());
        self.sources.set_visible(any_shown || !names.is_empty());

        // Nothing anywhere: AniList found nothing (or wasn't asked) and every source nothing.
        let anilist_nothing = query.sources_only
            || (self.anilist_in.get()
                && !self.anilist_failed.get()
                && self.first_page.borrow().is_empty());
        let all_nothing = search.groups.iter().all(|g| g.state == ReadSourceState::Nothing);
        let nowhere = search.done && anilist_nothing && all_nothing;
        if nowhere {
            self.nothing_title.set_label(&t::read_search_nothing(&query.text));
            self.nothing_line_one.set_label(&t::read_search_nothing_line());
            self.nothing_line_two.set_label(&t::read_search_sources_nothing());
            self.nothing_line_two.set_visible(!query.sources_only && !search.groups.is_empty());
            self.nothing.set_visible(true);
            self.anilist_place.set_visible(false);
            self.sources.set_visible(false);
        } else {
            self.nothing.set_visible(false);
            if !query.sources_only && !self.anilist_place.is_closing() {
                self.anilist_place.set_visible(true);
            }
        }
        self.draw_more();

        if search.done && !self.announced.replace(true) {
            let anilist = (!query.sources_only && !self.anilist_failed.get()).then(|| {
                self.total.get().map_or(self.first_page.borrow().len(), |n| n as usize)
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
            slot.inner.set_margin_top(if first { 12 } else { ABOVE_SOURCE });
            first &= !slot.place.is_visible();
        }
    }

    fn new_group(self: &Rc<Self>, group: &ReadSourceGroup) -> Rc<GroupSlot> {
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.set_margin_bottom(10);
        let name = gtk::Label::new(Some(&group.source_name));
        name.add_css_class("read-source-name");
        name.set_xalign(0.0);
        name.set_accessible_role(gtk::AccessibleRole::Heading);
        name.update_property(&[gtk::accessible::Property::Level(3)]);
        head.append(&name);
        let kind = widgets::haze("", "read-source-kind");
        kind.set_wrap(false);
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
        inner.append(&landing.widget);
        let place = Squeeze::new(&inner, gtk::Orientation::Vertical, 0);
        place.set_visible(false);
        landing.on_visible({
            let (place, search) = (place.clone(), Rc::downgrade(self));
            move || {
                place.set_visible(true);
                if let Some(search) = search.upgrade() {
                    search.first_group_gap();
                    if !search.nothing.is_visible() {
                        search.sources.set_visible(true);
                    }
                }
            }
        });
        Rc::new(GroupSlot {
            id: group.source_id.clone(),
            place,
            inner,
            name,
            kind,
            spinner,
            landing,
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
        // The kind beside the name waits for the engine to send it with the group.
        slot.kind.set_visible(false);
        let heading = group.source_name.clone();
        let asking = group.state == ReadSourceState::Asking;
        slot.spinner.set_visible(asking);
        slot.landing.widget.update_state(&[gtk::accessible::State::Busy(asking)]);
        if asking {
            slot.name.update_property(&[gtk::accessible::Property::Label(&format!(
                "{heading}, {}", // i18n-skip
                t::read_search_asking(&group.source_name)
            ))]);
            slot.signature.replace(String::new());
            slot.landing.wait(!slot.failed.get());
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
                let posters: Vec<gtk::Button> =
                    group.tiles.iter().map(|tile| discovery::poster(hub, tile, None)).collect();
                let strip = discovery::strip(&posters, &group.source_name);
                slot.landing.land(&strip, false);
            }
            ReadSourceState::Failed { failure } => {
                slot.failed.set(true);
                let block = self.failure(group, &super::failure(failure), query);
                slot.landing.land(&block, true);
            }
            ReadSourceState::TimedOut { seconds } => {
                slot.failed.set(true);
                let reason = t::read_search_source_timeout(i64::from(*seconds));
                let block = self.failure(group, &reason, query);
                slot.landing.land(&block, true);
            }
            // Nothing, or every hit joined an AniList result: the row goes.
            _ => {
                slot.failed.set(false);
                slot.landing.stop();
                if slot.place.is_visible() && slot.place.is_mapped() && !slot.place.is_closing()
                {
                    let place = slot.place.clone();
                    motion::fade_away(&slot.inner, move || {
                        let gone = place.clone();
                        place.close(move || gone.set_visible(false));
                    });
                } else if !slot.place.is_closing() {
                    slot.place.set_visible(false);
                }
            }
        }
    }

    /// A source that didn't answer: the glyph, the words, the reason and Try again for it alone.
    fn failure(
        &self,
        group: &ReadSourceGroup,
        reason: &str,
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
        why.set_margin_top(2);
        why.set_margin_bottom(10);
        words.append(&why);
        let again = gtk::Button::with_label(&t::home_try_again());
        again.add_css_class("home-sec");
        again.set_halign(gtk::Align::Start);
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

/// For a screenshot: Format set to Manga, its menu open.
#[cfg(feature = "fixtures")]
pub fn shot_filters(shell: &Rc<crate::window::Shell>) {
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
        if let Some(button) = first_filter(window.upcast_ref()) {
            let _ = button.activate_action("rfilter.format-manga", None);
            glib::timeout_add_local_once(Duration::from_millis(600), move || button.popup());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_model::reading::{ReadAnilistState, ReadSourceJoin, ReadingFailure};

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
        };
        ReadSourceGroup {
            source_id: name.to_lowercase(),
            source_name: name.into(),
            state,
            tiles: vec![tile; hits],
            joined,
        }
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
            settled_words(Some(4), &groups),
            "4 from AniList, 3 from your sources. Northwind didn't answer. Eastport didn't answer."
        );
        assert_eq!(settled_words(None, &groups[..1]), "3 from your sources.");
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
