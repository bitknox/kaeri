//! Search's filters up front, as the Mac has them: genres (wanted, or left out), year, season,
//! format and status, each a menu button, and the sort at the end. Below them, what is set, each
//! taken off with a click, and Clear All. Shown before typing too, so the titles can be browsed
//! without a word typed. The query is the engine's [`SearchQuery`]; the chips' words are its own.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    browsing::TrendPeriod,
    catalog::Season,
    search::{
        ActiveFilter, Available, FilterKey, MediaFormat, Pick, SEARCH_STATUSES, SearchQuery,
        SearchSort,
    },
};

use crate::{
    say,
    window::{Ctx, Size},
};

/// AniList's genres, until the engine says which it offers.
const GENRES: [&str; 16] = [
    "Action",        // i18n-skip
    "Adventure",     // i18n-skip
    "Comedy",        // i18n-skip
    "Drama",         // i18n-skip
    "Fantasy",       // i18n-skip
    "Horror",        // i18n-skip
    "Mecha",         // i18n-skip
    "Music",         // i18n-skip
    "Mystery",       // i18n-skip
    "Psychological", // i18n-skip
    "Romance",       // i18n-skip
    "Sci-Fi",        // i18n-skip
    "Slice of Life", // i18n-skip
    "Sports",        // i18n-skip
    "Supernatural",  // i18n-skip
    "Thriller",      // i18n-skip
];

const SEASONS: [Season; 4] = [Season::Winter, Season::Spring, Season::Summer, Season::Fall];

/// The oldest year the menu offers.
const FIRST_YEAR: u32 = 1970;

/// What the filters call when one changes.
type Changed = Box<dyn Fn()>;

pub struct Filters {
    /// The menu buttons, in a row that scrolls sideways when the window is narrow.
    pub row: gtk::Box,
    /// What is set, as chips.
    pub chips: adw::WrapBox,
    genres: gtk::MenuButton,
    year: gtk::MenuButton,
    season: gtk::MenuButton,
    available: gtk::MenuButton,
    format: gtk::MenuButton,
    status: gtk::MenuButton,
    sort: gtk::MenuButton,
    sort_label: gtk::Label,
    actions: gio::SimpleActionGroup,
    /// Everything but the text: the filters and the sort.
    query: RefCell<SearchQuery>,
    /// Whether text is typed, for the sorts on offer: relevance only with text.
    with_text: std::cell::Cell<bool>,
    names: RefCell<Vec<String>>,
    changed: RefCell<Option<Changed>>,
}

/// A menu button of the row: flat until it holds a choice.
pub fn menu_button(label: &str) -> gtk::MenuButton {
    let button = gtk::MenuButton::builder().label(label).always_show_arrow(true).build();
    button.add_css_class("flat");
    button.add_css_class("filter");
    button
}

impl Filters {
    pub fn new() -> Rc<Self> {
        let genres = menu_button(&t::filter_genres());
        let year = menu_button(&t::filter_year());
        let season = menu_button(&t::filter_season());
        let available = menu_button(&t::filter_linux_available());
        let format = menu_button(&t::filter_format());
        let status = menu_button(&t::filter_status());
        let (sort, sort_label) = sort_button();
        let capsules = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for button in [&genres, &year, &season, &available, &format, &status] {
            capsules.append(button);
        }
        let row = filter_row(&capsules, &sort);
        let chips = chips_box();

        let actions = gio::SimpleActionGroup::new();
        row.insert_action_group("filter", Some(&actions));
        chips.insert_action_group("filter", Some(&actions));

        let filters = Rc::new(Self {
            row,
            chips,
            genres,
            year,
            season,
            available,
            format,
            status,
            sort,
            sort_label,
            actions,
            query: RefCell::new(SearchQuery::default()),
            with_text: std::cell::Cell::new(false),
            names: RefCell::new(GENRES.iter().map(|g| (*g).to_owned()).collect()),
            changed: RefCell::default(),
        });
        filters.wire();
        filters.refresh();
        filters
    }

    /// Run `f` whenever the viewer changes a filter or the sort.
    pub fn on_change(&self, f: impl Fn() + 'static) {
        self.changed.replace(Some(Box::new(f)));
    }

    /// Every filter off, as Clear All does.
    pub fn clear(&self) {
        self.actions.activate_action("clear", None);
    }

    /// The query with `text`, and every filter and the sort as chosen.
    pub fn query(&self, text: &str) -> SearchQuery {
        SearchQuery { text: text.trim().to_owned(), ..self.query.borrow().clone() }
    }

    /// Text typed or cleared: the sort on offer follows, relevance only with text.
    pub fn typed(self: &Rc<Self>, with_text: bool) {
        if self.with_text.replace(with_text) != with_text {
            self.refresh();
        }
    }

    /// The genres the engine says AniList offers.
    pub fn set_genres(self: &Rc<Self>, genres: &[String]) {
        if genres.is_empty() || *self.names.borrow() == genres {
            return;
        }
        self.names.replace(genres.to_vec());
        self.refresh();
    }

    /// The genres on offer, for Search's start.
    pub fn genres(&self) -> Vec<String> {
        self.names.borrow().clone()
    }

    /// Set the filters and the sort to `query`'s, as a genre tile or a screenshot does, and
    /// search.
    pub fn set(self: &Rc<Self>, query: &SearchQuery) {
        self.query.replace(SearchQuery { text: String::new(), ..query.clone() });
        self.refresh();
        self.tell();
    }

    fn edit(self: &Rc<Self>, f: impl FnOnce(&SearchQuery) -> SearchQuery) {
        let next = f(&self.query.borrow());
        self.query.replace(next);
        self.refresh();
        self.tell();
    }

    fn tell(&self) {
        if let Some(changed) = self.changed.borrow().as_ref() {
            changed();
        }
    }

    fn wire(self: &Rc<Self>) {
        let plain = |name: &str, f: fn(&SearchQuery) -> SearchQuery| {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(filters) = weak.upgrade() {
                    filters.edit(f);
                }
            });
            self.actions.add_action(&action);
        };
        plain("any-genre", |q| SearchQuery {
            genres: vec![],
            genres_excluded: vec![],
            ..q.clone()
        });
        plain("any-format", |q| SearchQuery { formats: vec![], ..q.clone() });
        plain("any-status", |q| SearchQuery { statuses: vec![], ..q.clone() });
        plain("clear", SearchQuery::cleared);

        // The one-of-many choices: a string target each, "" for any.
        let choice = |name: &str, f: fn(&SearchQuery, &str) -> SearchQuery| {
            let action = gio::SimpleAction::new_stateful(
                name,
                Some(glib::VariantTy::STRING),
                &"".to_variant(),
            );
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, target| {
                let (Some(filters), Some(value)) =
                    (weak.upgrade(), target.and_then(String::from_variant))
                else {
                    return;
                };
                filters.edit(|q| f(q, &value));
            });
            self.actions.add_action(&action);
        };
        choice("year", |q, value| q.in_year(value.parse().ok()));
        choice("season", |q, value| SearchQuery {
            season: SEASONS.into_iter().find(|s| season_name(*s) == value),
            ..q.clone()
        });
        choice("available", |q, value| SearchQuery {
            available: match value {
                "here" => Some(Available::PlaysHere),
                "services" => Some(Available::OnYourServices),
                _ => None,
            },
            ..q.clone()
        });
        choice("sort", |q, value| {
            // "Trending:week": a sort, and for Trending the period it covers.
            let (name, period) = value.split_once(':').unwrap_or((value, ""));
            let sort = SearchSort::ALL.into_iter().find(|s| s.label() == name);
            let period = TrendPeriod::ALL
                .into_iter()
                .find(|p| period_key(*p) == period)
                .filter(|p| sort == Some(SearchSort::Trending) && *p != TrendPeriod::Today);
            SearchQuery { sort, period, ..q.clone() }
        });

        // Each chip takes its filter off: its index among what is set.
        let remove = gio::SimpleAction::new("remove", Some(glib::VariantTy::UINT32));
        let weak = Rc::downgrade(self);
        remove.connect_activate(move |_, target| {
            let (Some(filters), Some(at)) =
                (weak.upgrade(), target.and_then(u32::from_variant))
            else {
                return;
            };
            let key = filters.query.borrow().active().get(at as usize).map(|f| f.key.clone());
            if let Some(key) = key {
                // A chip removed from the keyboard hands focus on: to the chip that takes its
                // place, else the one before, else the filter row.
                let had_focus = filters.chips.focus_child().is_some();
                filters.edit(|q| q.without(&key));
                if had_focus {
                    focus_chip(&filters.chips, at as usize, &filters.genres);
                }
            }
        });
        self.actions.add_action(&remove);
    }

    /// A check item's action: on while `on` says so, and `flip` when chosen.
    fn check(
        self: &Rc<Self>,
        name: &str,
        on: bool,
        flip: impl Fn(&SearchQuery) -> SearchQuery + 'static,
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

    fn set_state(&self, name: &str, value: &str) {
        if let Some(action) =
            self.actions.lookup_action(name).and_downcast::<gio::SimpleAction>()
        {
            action.set_state(&value.to_variant());
        }
    }

    /// The buttons' words, their menus and the chips, as the query stands.
    fn refresh(self: &Rc<Self>) {
        let query = self.query.borrow().clone();
        self.show_genres(&query);
        self.show_year(&query);
        self.show_season(&query);
        self.show_available(&query);
        self.show_formats(&query);
        self.show_statuses(&query);
        self.show_sort(&query);
        self.show_chips(&query.active());
    }

    fn show_genres(self: &Rc<Self>, query: &SearchQuery) {
        let names = self.names.borrow().clone();
        let wanted = gio::Menu::new();
        let left_out = gio::Menu::new();
        for (at, name) in names.iter().enumerate() {
            let pick = query.genre_pick(name);
            let (want, leave) = (format!("genre-{at}"), format!("leave-{at}"));
            let named = name.clone();
            self.check(&want, pick == Pick::Include, move |q| {
                let on = q.genre_pick(&named) == Pick::Include;
                q.picking_genre(&named, if on { Pick::Off } else { Pick::Include })
            });
            let named = name.clone();
            self.check(&leave, pick == Pick::Exclude, move |q| {
                let on = q.genre_pick(&named) == Pick::Exclude;
                q.picking_genre(&named, if on { Pick::Off } else { Pick::Exclude })
            });
            wanted.append(Some(&say::genre(name)), Some(&format!("filter.{want}")));
            left_out.append(Some(&say::genre(name)), Some(&format!("filter.{leave}")));
        }
        let menu = gio::Menu::new();
        menu.append_section(None, &wanted);
        let more = gio::Menu::new();
        more.append_submenu(Some(&t::filter_left_out()), &left_out);
        menu.append_section(None, &more);
        let set = query.genres.len() + query.genres_excluded.len();
        if set > 0 {
            let any = gio::Menu::new();
            any.append(Some(&t::filter_any_genre()), Some("filter.any-genre"));
            menu.append_section(None, &any);
        }
        self.genres.set_menu_model(Some(&menu));
        let label = match set {
            0 => t::filter_genres(),
            1 => query
                .active()
                .into_iter()
                .next()
                .map(|f| chip_label(query, &f))
                .unwrap_or_default(),
            n => t::filter_linux_genres_count(n as i64),
        };
        set_label(&self.genres, &label, set > 0);
    }

    fn show_year(&self, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let any = gio::Menu::new();
        any.append(Some(&t::filter_any_year()), Some("filter.year::"));
        menu.append_section(None, &any);
        let years = gio::Menu::new();
        for year in (FIRST_YEAR..=this_year() + 1).rev() {
            years.append(Some(&year.to_string()), Some(&format!("filter.year::{year}")));
        }
        menu.append_section(None, &years);
        self.year.set_menu_model(Some(&menu));
        let single = query.years.single();
        self.set_state("year", &single.map(|y| y.to_string()).unwrap_or_default());
        let label = query.active().into_iter().find(|f| f.key == FilterKey::Years);
        let set = !query.years.is_open();
        set_label(&self.year, &label.map(|f| f.label).unwrap_or_else(t::filter_year), set);
    }

    fn show_available(&self, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let any = gio::Menu::new();
        any.append(Some(&t::filter_linux_available_any()), Some("filter.available::"));
        menu.append_section(None, &any);
        let places = gio::Menu::new();
        places.append(
            Some(&available_name(Available::PlaysHere)),
            Some("filter.available::here"),
        );
        places.append(
            Some(&available_name(Available::OnYourServices)),
            Some("filter.available::services"),
        );
        menu.append_section(None, &places);
        self.available.set_menu_model(Some(&menu));
        let value = match query.available {
            Some(Available::PlaysHere) => "here",
            Some(Available::OnYourServices) => "services",
            None => "",
        };
        self.set_state("available", value);
        let label =
            query.available.map(available_name).unwrap_or_else(t::filter_linux_available);
        set_label(&self.available, &label, query.available.is_some());
    }

    fn show_season(&self, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let any = gio::Menu::new();
        any.append(Some(&t::filter_any_season()), Some("filter.season::"));
        menu.append_section(None, &any);
        let seasons = gio::Menu::new();
        for season in SEASONS {
            seasons.append(
                Some(&say::season_name(season)),
                Some(&format!("filter.season::{}", season_name(season))),
            );
        }
        menu.append_section(None, &seasons);
        self.season.set_menu_model(Some(&menu));
        self.set_state("season", query.season.map(season_name).unwrap_or_default());
        // A season is a season of one year.
        let open = query.years.single().is_some() || query.season.is_some();
        self.season.set_sensitive(open);
        self.season.set_tooltip_text((!open).then(t::filter_season_pick_year).as_deref());
        let label = query.season.map(say::season_name).unwrap_or_else(t::filter_season);
        set_label(&self.season, &label, query.season.is_some());
    }

    fn show_formats(self: &Rc<Self>, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let all = gio::Menu::new();
        for (at, format) in MediaFormat::ALL.into_iter().enumerate() {
            let name = format!("format-{at}");
            self.check(&name, query.formats.contains(&format), move |q| {
                q.toggling_format(format)
            });
            all.append(Some(&format_name(format)), Some(&format!("filter.{name}")));
        }
        menu.append_section(None, &all);
        let count = query.formats.len();
        if count > 0 {
            let any = gio::Menu::new();
            any.append(Some(&t::filter_any_format()), Some("filter.any-format"));
            menu.append_section(None, &any);
        }
        self.format.set_menu_model(Some(&menu));
        let label = match count {
            0 => t::filter_format(),
            1 | 2 => formats_line(&query.formats),
            n => t::filter_linux_formats_count(n as i64),
        };
        set_label(&self.format, &label, count > 0);
    }

    fn show_statuses(self: &Rc<Self>, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let all = gio::Menu::new();
        for (at, status) in SEARCH_STATUSES.into_iter().enumerate() {
            let name = format!("status-{at}");
            self.check(&name, query.statuses.contains(&status), move |q: &SearchQuery| {
                q.toggling_status(status)
            });
            all.append(Some(&say::airing_status(status)), Some(&format!("filter.{name}")));
        }
        menu.append_section(None, &all);
        let count = query.statuses.len();
        if count > 0 {
            let any = gio::Menu::new();
            any.append(Some(&t::filter_any_status()), Some("filter.any-status"));
            menu.append_section(None, &any);
        }
        self.status.set_menu_model(Some(&menu));
        let label = match count {
            0 => t::filter_status(),
            1 => say::airing_status(query.statuses[0]),
            n => t::filter_linux_statuses_count(n as i64),
        };
        set_label(&self.status, &label, count > 0);
    }

    fn show_sort(&self, query: &SearchQuery) {
        let with_text = self.with_text.get();
        let effect = SearchQuery {
            text: if with_text { "x".into() } else { String::new() },
            ..query.clone()
        }
        .sort_in_effect();
        let menu = gio::Menu::new();
        let sorts = gio::Menu::new();
        for sort in SearchSort::ALL {
            if sort == SearchSort::Relevance && !with_text {
                continue;
            }
            // Trending is its three periods, which a rail's See all can show.
            if sort == SearchSort::Trending {
                for period in TrendPeriod::ALL {
                    sorts.append(
                        Some(&period.label()),
                        Some(&format!("filter.sort::{}:{}", sort.label(), period_key(period))),
                    );
                }
                continue;
            }
            sorts.append(
                Some(&sort_name(sort)),
                Some(&format!("filter.sort::{}", sort.label())),
            );
        }
        menu.append_section(Some(&t::filter_linux_sort_by()), &sorts);
        self.sort.set_menu_model(Some(&menu));
        let named = if effect == SearchSort::Trending {
            let period = query.period.unwrap_or(TrendPeriod::Today);
            self.set_state("sort", &format!("{}:{}", effect.label(), period_key(period)));
            period.label()
        } else {
            self.set_state("sort", effect.label());
            sort_name(effect)
        };
        self.sort_label.set_label(&named);
        self.sort.set_tooltip_text(Some(&t::sort_label(&named)));
        mark_set(&self.sort, query.sort.is_some());
    }

    fn show_chips(&self, active: &[ActiveFilter]) {
        let query = self.query.borrow().clone();
        let labels: Vec<String> = active.iter().map(|f| chip_label(&query, f)).collect();
        show_chips(&self.chips, &labels, "filter.remove", "filter.clear");
    }
}

/// A button's words, and whether it holds a choice: flat until it does.
pub fn set_label(button: &gtk::MenuButton, label: &str, set: bool) {
    button.set_label(label);
    mark_set(button, set);
}

/// A capsule of the row, flat until it holds a choice.
pub fn mark_set(widget: &impl IsA<gtk::Widget>, set: bool) {
    if set {
        widget.remove_css_class("flat");
    } else {
        widget.add_css_class("flat");
    }
}

/// The sort at the row's end: its glyph, the order's name and the arrow. Returns the button and
/// the label that names the order.
pub fn sort_button() -> (gtk::MenuButton, gtk::Label) {
    let label = gtk::Label::new(None);
    // Whole where there is room; a narrow window at a large text size shortens it rather than
    // pushing the row past the edge.
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    line.append(&gtk::Image::from_icon_name("view-sort-descending-symbolic"));
    line.append(&label);
    let sort = gtk::MenuButton::builder().child(&line).always_show_arrow(true).build();
    sort.add_css_class("flat");
    sort.add_css_class("filter");
    (sort, label)
}

/// The row under the field: `capsules` scrolling sideways, then the sort at its end.
pub fn filter_row(capsules: &gtk::Box, sort: &gtk::MenuButton) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_margin_start(12);
    row.set_margin_end(12);
    row.append(&faded_scroll(capsules));
    row.append(sort);
    row
}

/// The row with Shows or Reading at its start, where Library has it. A narrow window can't fit
/// the two with the sort beside the filters, so there it sits on a line of its own over them.
pub fn with_scope(ctx: &Rc<Ctx>, row: &gtk::Box, scope: &gtk::Widget) -> gtk::Box {
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    line.set_margin_start(12);
    line.set_margin_end(12);
    holder.append(&line);
    holder.append(row);
    row.prepend(scope);
    ctx.on_size({
        let (line, row, scope) = (line.downgrade(), row.downgrade(), scope.downgrade());
        move |size| {
            if let (Some(line), Some(row), Some(scope)) =
                (line.upgrade(), row.upgrade(), scope.upgrade())
            {
                lay_scope(&line, &row, &scope, size);
            }
        }
    });
    holder
}

/// Shows or Reading in `row`, or on `line` over it in a narrow window.
fn lay_scope(line: &gtk::Box, row: &gtk::Box, scope: &gtk::Widget, size: Size) {
    let narrow = size == Size::Narrow;
    let on_line = scope.parent().as_ref() == Some(line.upcast_ref());
    if narrow && !on_line {
        row.remove(scope);
        line.append(scope);
        // Under it, the row's own gap; hidden with it when only one medium is on.
        scope.set_margin_bottom(6);
    } else if !narrow && on_line {
        line.remove(scope);
        row.prepend(scope);
        scope.set_margin_bottom(0);
    }
}

/// Where the set filters stand as chips, under the row.
pub fn chips_box() -> adw::WrapBox {
    let chips = adw::WrapBox::builder()
        .child_spacing(6)
        .line_spacing(6)
        .halign(gtk::Align::Start)
        .build();
    chips.set_margin_start(18);
    chips.set_margin_end(18);
    chips.set_visible(false);
    chips
}

/// One set filter as a chip: its words and a cross, taken off by `action` with its index `at`.
pub fn chip(label: &str, action: &str, at: u32) -> gtk::Button {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let words = gtk::Label::new(Some(label));
    words.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.set_max_width_chars(28);
    line.append(&words);
    line.append(&gtk::Image::from_icon_name("window-close-symbolic"));
    let chip = gtk::Button::builder().child(&line).build();
    chip.add_css_class("pill");
    chip.add_css_class("filter-chip");
    chip.set_halign(gtk::Align::Start);
    chip.set_tooltip_text(Some(&t::filter_remove(label)));
    chip.update_property(&[gtk::accessible::Property::Label(&t::filter_remove(label))]);
    chip.set_action_target_value(Some(&at.to_variant()));
    chip.set_action_name(Some(action));
    chip
}

/// The chips for `labels`, then Clear all when more than one is set.
pub fn show_chips(chips: &adw::WrapBox, labels: &[String], remove: &str, clear: &str) {
    while let Some(child) = chips.first_child() {
        chips.remove(&child);
    }
    for (at, label) in labels.iter().enumerate() {
        chips.append(&chip(label, remove, at as u32));
    }
    if labels.len() > 1 {
        let all = gtk::Button::with_label(&t::filter_linux_clear_all());
        all.add_css_class("flat");
        all.set_halign(gtk::Align::Start);
        all.set_action_name(Some(clear));
        chips.append(&all);
    }
    chips.set_visible(!labels.is_empty());
}

/// A chip removed from the keyboard hands focus on: to the chip that takes its place, else the
/// one before, else `first`.
pub fn focus_chip(chips: &adw::WrapBox, at: usize, first: &impl IsA<gtk::Widget>) {
    let mut all = Vec::new();
    let mut child = chips.first_child();
    while let Some(c) = child {
        child = c.next_sibling();
        all.push(c);
    }
    match all.get(at).or_else(|| at.checked_sub(1).and_then(|i| all.get(i))) {
        Some(chip) => {
            chip.grab_focus();
        }
        None => {
            first.as_ref().grab_focus();
        }
    }
}

/// A format as the catalogue says it.
fn format_name(format: MediaFormat) -> String {
    match format {
        MediaFormat::Tv => t::format_tv(),
        MediaFormat::TvShort => t::format_tv_short(),
        MediaFormat::Movie => t::format_movie(),
        MediaFormat::Special => t::format_special(),
        MediaFormat::Ova => t::format_ova(),
        MediaFormat::Ona => t::format_ona(),
        MediaFormat::Music => t::format_music(),
    }
}

fn formats_line(formats: &[MediaFormat]) -> String {
    formats.iter().map(|f| format_name(*f)).collect::<Vec<_>>().join(", ")
}

/// The name a period has in a sort's action target.
fn period_key(period: TrendPeriod) -> &'static str {
    match period {
        TrendPeriod::Today => "today",
        TrendPeriod::Week => "week",
        TrendPeriod::Month => "month",
    }
}

/// A sort as the catalogue says it.
fn sort_name(sort: SearchSort) -> String {
    match sort {
        SearchSort::Relevance => t::sort_relevance(),
        SearchSort::Trending => t::filter_linux_sort_trending(),
        SearchSort::Popularity => t::sort_popularity(),
        SearchSort::Score => t::sort_score(),
        SearchSort::Newest => t::filter_linux_sort_newest(),
        SearchSort::Oldest => t::filter_linux_sort_oldest(),
        SearchSort::Title => t::library_sort_title(),
        SearchSort::Favourites => t::filter_linux_sort_favourites(),
    }
}

/// What a chip says, in the language in use where the catalogue has the words; the engine's own
/// wording for the filters it words alone.
fn chip_label(query: &SearchQuery, filter: &ActiveFilter) -> String {
    match &filter.key {
        FilterKey::Genre { name } => say::genre(name),
        FilterKey::NotGenre { name } => t::filter_linux_not(&say::genre(name)),
        FilterKey::Formats => formats_line(&query.formats),
        FilterKey::Statuses => {
            query.statuses.iter().map(|s| say::airing_status(*s)).collect::<Vec<_>>().join(", ")
        }
        FilterKey::Available => query.available.map(available_name).unwrap_or_default(),
        FilterKey::Premiered => query.season.map(say::season_name).unwrap_or_default(),
        _ => filter.label.clone(),
    }
}

fn available_name(available: Available) -> String {
    match available {
        Available::PlaysHere => t::filter_linux_available_here(),
        Available::OnYourServices => t::filter_linux_available_services(),
    }
}

/// A season's name as an action's target.
fn season_name(season: Season) -> &'static str {
    match season {
        Season::Winter => "winter",
        Season::Spring => "spring",
        Season::Summer => "summer",
        Season::Fall => "fall",
    }
}

fn this_year() -> u32 {
    glib::DateTime::now_local()
        .map(|now| now.year())
        .ok()
        .and_then(|y| u32::try_from(y).ok())
        .unwrap_or(2026)
}

/// The menu buttons in a row that scrolls sideways, as the Mac draws it: the edge faded where
/// more of it goes on, with an arrow there that scrolls to it, so a narrow window shows there is
/// more to reach.
pub fn faded_scroll(capsules: &gtk::Box) -> gtk::Overlay {
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .propagate_natural_width(true)
        .min_content_width(120)
        .hexpand(true)
        .child(capsules)
        .build();
    let adjustment = scroll.hadjustment();
    let edge = |side: &str, align: gtk::Align, icon: &str, tip: &str, by: f64| {
        let fade =
            gtk::Box::builder().halign(align).width_request(48).can_target(false).build();
        fade.add_css_class("edge-fade");
        fade.add_css_class(side);
        let arrow = gtk::Button::from_icon_name(icon);
        arrow.add_css_class("circular");
        arrow.add_css_class("edge-arrow");
        arrow.set_halign(align);
        arrow.set_valign(gtk::Align::Center);
        arrow.set_tooltip_text(Some(tip));
        // The row's buttons take the keyboard on their own: the arrow is for the pointer.
        arrow.set_focusable(false);
        let adjustment = adjustment.downgrade();
        arrow.connect_clicked(move |_| {
            let Some(adjustment) = adjustment.upgrade() else { return };
            let to = adjustment.value() + by * adjustment.page_size() * 0.7;
            adjustment.set_value(to.clamp(0.0, adjustment.upper() - adjustment.page_size()));
        });
        (fade, arrow)
    };
    let (start_fade, start_arrow) = edge(
        "start",
        gtk::Align::Start,
        "go-previous-symbolic",
        &t::filter_linux_earlier(),
        -1.0,
    );
    let (end_fade, end_arrow) =
        edge("end", gtk::Align::End, "go-next-symbolic", &t::filter_linux_more(), 1.0);
    let overlay = gtk::Overlay::builder().child(&scroll).hexpand(true).build();
    for widget in [&start_fade, &end_fade] {
        overlay.add_overlay(widget);
    }
    overlay.add_overlay(&start_arrow);
    overlay.add_overlay(&end_arrow);
    // The adjustment's own handlers run this, and each arrow's click holds the adjustment, so
    // both hold the other side weakly: a strong hold both ways would keep the arrows, their fades
    // and the adjustment alive after the row is dropped.
    let edges: [glib::WeakRef<gtk::Widget>; 4] = [
        start_fade.upcast_ref::<gtk::Widget>().downgrade(),
        start_arrow.upcast_ref::<gtk::Widget>().downgrade(),
        end_fade.upcast_ref::<gtk::Widget>().downgrade(),
        end_arrow.upcast_ref::<gtk::Widget>().downgrade(),
    ];
    let follow = move |a: &gtk::Adjustment| {
        let (value, room) = (a.value(), a.upper() - a.page_size());
        let (before, after) = (value > 1.0, room - value > 1.0);
        for (edge, shown) in edges.iter().zip([before, before, after, after]) {
            if let Some(edge) = edge.upgrade() {
                edge.set_visible(shown);
            }
        }
    };
    follow(&adjustment);
    adjustment.connect_value_changed(follow.clone());
    adjustment.connect_changed(follow);
    overlay
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Search's row in the smallest window, 350 px inside: the medium goes over the filters and
    /// the sort, at twice the text size too, and back into the row in a wider one.
    #[test]
    fn the_row_fits_a_narrow_window_at_any_text_size() {
        crate::gtk_test::run(|| {
            use crate::gtk_test::pump;
            let toggles = adw::ToggleGroup::new();
            for (name, label) in
                [("shows", t::library_scope_shows()), ("reading", t::library_scope_reading())]
            {
                toggles.add(adw::Toggle::builder().name(name).label(label).build());
            }
            toggles.add_css_class("round");
            let scope: gtk::Widget = toggles.upcast();
            let capsules = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            for words in ["Genres", "Status", "Started", "Format"] {
                capsules.append(&gtk::MenuButton::builder().label(words).build());
            }
            let (sort, label) = sort_button();
            label.set_label(&t::search_sort_favourites());
            let row = filter_row(&capsules, &sort);
            let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            holder.append(&line);
            holder.append(&row);
            row.prepend(&scope);
            let window = gtk::Window::builder().default_width(1280).default_height(800).build();
            window.set_child(Some(&holder));
            window.present();
            pump(100);
            assert_eq!(scope.parent().as_ref(), Some(row.upcast_ref::<gtk::Widget>()));

            lay_scope(&line, &row, &scope, Size::Narrow);
            pump(100);
            let min_width = || holder.measure(gtk::Orientation::Horizontal, -1).0;
            let normal = min_width();
            let settings = gtk::Settings::default().expect("a display");
            let dpi = settings.gtk_xft_dpi();
            settings.set_gtk_xft_dpi(2 * 96 * 1024);
            pump(150);
            let large = min_width();
            let parts = (
                line.measure(gtk::Orientation::Horizontal, -1).0,
                row.measure(gtk::Orientation::Horizontal, -1).0,
                sort.measure(gtk::Orientation::Horizontal, -1).0,
            );
            settings.set_gtk_xft_dpi(dpi);
            assert_eq!(scope.parent().as_ref(), Some(line.upcast_ref::<gtk::Widget>()));
            lay_scope(&line, &row, &scope, Size::Medium);
            pump(50);
            assert_eq!(row.first_child().as_ref(), Some(&scope));
            window.destroy();

            assert!(normal <= 350, "the row needs {normal} px at the normal text size");
            assert!(large <= 350, "the row needs {large} px at twice the size: {parts:?}");
        });
    }

    #[test]
    fn a_season_name_reads_back_as_its_season() {
        for season in SEASONS {
            assert_eq!(
                SEASONS.into_iter().find(|s| season_name(*s) == season_name(season)),
                Some(season)
            );
        }
    }

    /// The row's arrows and its adjustment hold each other: a dropped row must free them all.
    #[test]
    fn a_dropped_filter_row_frees_its_arrows_and_adjustment() {
        crate::gtk_test::run(|| {
            let capsules = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            for name in ["Format", "Status", "Genres", "Year", "Season"] {
                capsules.append(&menu_button(name));
            }
            let row = faded_scroll(&capsules);
            let window = gtk::Window::builder().default_width(200).default_height(80).build();
            window.set_child(Some(&row));
            window.present();
            crate::gtk_test::pump(200);
            let mut held: Vec<(&str, glib::WeakRef<glib::Object>)> =
                vec![("row", row.upcast_ref::<glib::Object>().downgrade())];
            let mut child = row.first_child();
            while let Some(widget) = child {
                if let Some(scroll) = widget.downcast_ref::<gtk::ScrolledWindow>() {
                    held.push((
                        "adjustment",
                        scroll.hadjustment().upcast::<glib::Object>().downgrade(),
                    ));
                } else if widget.is::<gtk::Button>() {
                    held.push(("arrow", widget.upcast_ref::<glib::Object>().downgrade()));
                }
                child = widget.next_sibling();
            }
            assert_eq!(held.len(), 4, "the row, its adjustment and two arrows");
            window.set_child(gtk::Widget::NONE);
            drop((row, capsules));
            window.destroy();
            drop(window);
            // GTK lets go of a widget it was laying out on a later turn of the loop.
            let kept = || -> Vec<&str> {
                held.iter()
                    .filter(|(_, weak)| weak.upgrade().is_some())
                    .map(|(name, _)| *name)
                    .collect()
            };
            for _ in 0..40 {
                if kept().is_empty() {
                    break;
                }
                crate::gtk_test::pump(25);
            }
            let kept = kept();
            assert!(kept.is_empty(), "kept after the row was dropped: {kept:?}");
        });
    }

    #[test]
    fn the_genres_known_before_the_engine_says_are_unique() {
        let mut names = GENRES.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), GENRES.len());
    }
}
