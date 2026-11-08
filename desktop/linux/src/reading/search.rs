//! Search, Reading side: a field over a grid of covers, more on request. Under the field the
//! filters, as the Shows side has them: Format, Origin, Status and Genres, each a menu button,
//! with the order at the end. With nothing typed it browses, and a line says what it shows.

use std::{cell::RefCell, rc::Rc, time::Duration};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    reading::{
        ReadFilterChoice, ReadFilters, ReadSearchQuery, ReadingCommand, ReadingEvent,
        read_filters,
    },
};

use super::{Dest, WorkGrid, listen, scope, send};
use crate::{
    pages::search_filters::{faded_scroll, menu_button, set_label},
    say,
    states::{Shape, States},
    widgets,
    window::Ctx,
};

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

/// What a browse shows: the filters set, or the order with none.
fn browse_line(query: &ReadSearchQuery, filters: &ReadFilters, total: Option<u32>) -> String {
    let mut parts = labels(&query.formats, &filters.formats);
    parts.extend(labels(&query.origins, &filters.origins));
    if let Some(status) = &query.status {
        parts.extend(labels(std::slice::from_ref(status), &filters.statuses));
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

/// The filters and the order, and the query they make.
struct Filters {
    row: gtk::Box,
    format: gtk::MenuButton,
    origin: gtk::MenuButton,
    status: gtk::MenuButton,
    genres: gtk::MenuButton,
    sort: gtk::MenuButton,
    sort_label: gtk::Label,
    actions: gio::SimpleActionGroup,
    choices: ReadFilters,
    query: RefCell<ReadSearchQuery>,
    changed: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Filters {
    fn new() -> Rc<Self> {
        let format = menu_button(&t::read_filter_format());
        let origin = menu_button(&t::read_filter_origin());
        let status = menu_button(&t::read_filter_status());
        let genres = menu_button(&t::read_filter_genres());
        let sort_label = gtk::Label::new(None);
        let sort_line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        sort_line.append(&gtk::Image::from_icon_name("view-sort-descending-symbolic"));
        sort_line.append(&sort_label);
        let sort = gtk::MenuButton::builder().child(&sort_line).always_show_arrow(true).build();
        sort.add_css_class("flat");
        sort.add_css_class("filter");
        let capsules = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for button in [&format, &origin, &status, &genres] {
            capsules.append(button);
        }
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
            sort,
            sort_label,
            actions,
            choices: read_filters(),
            query: RefCell::default(),
            changed: RefCell::default(),
        });
        filters.refresh();
        filters
    }

    fn on_change(&self, f: impl Fn() + 'static) {
        self.changed.replace(Some(Rc::new(f)));
    }

    fn query(&self, text: &str) -> ReadSearchQuery {
        ReadSearchQuery { text: text.trim().to_owned(), ..self.query.borrow().clone() }
    }

    fn any_set(&self) -> bool {
        let q = self.query.borrow();
        !(q.formats.is_empty()
            && q.origins.is_empty()
            && q.status.is_none()
            && q.genres.is_empty()
            && q.exclude_genres.is_empty())
    }

    /// Every filter off; the order stays.
    fn clear(self: &Rc<Self>) {
        self.edit(|q| ReadSearchQuery { sort: q.sort.clone(), ..ReadSearchQuery::default() });
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
    }
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let grid = Rc::new(WorkGrid::new(&ctx.hub));
    // A query lists rows; browsing without one is the grid of covers.
    let rows = widgets::boxed_list();
    rows.add_css_class("read-search-rows");
    rows.set_margin_start(24);
    rows.set_margin_end(24);
    rows.set_margin_top(8);
    rows.set_margin_bottom(24);
    rows.set_visible(false);
    // The next page loads near the end by itself, with a spinner while it comes.
    let more = adw::Spinner::new();
    more.set_halign(gtk::Align::Center);
    more.set_size_request(24, 24);
    more.set_margin_top(8);
    more.set_margin_bottom(16);
    more.set_visible(false);
    let has_more = Rc::new(std::cell::Cell::new(false));
    let asking_more = Rc::new(std::cell::Cell::new(false));
    // Over the grid: what a browse shows, or how many the Shows side has for the words.
    let line = widgets::haze("", "caption");
    line.set_halign(gtk::Align::Start);
    line.set_margin_start(18);
    line.set_margin_top(6);
    line.set_visible(false);
    let other_words = widgets::haze("", "caption");
    let show_them = gtk::Button::with_label(&t::search_show_them());
    show_them.add_css_class("flat");
    let other = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    other.set_margin_start(18);
    other.set_margin_top(6);
    other.set_visible(false);
    other.append(&other_words);
    other.append(&show_them);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&line);
    body.append(&other);
    body.append(&grid.widget);
    body.append(&rows);
    // While only the demo is on, its works as rows instead of the grid.
    let demo_rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    demo_rows.set_visible(false);
    body.append(&demo_rows);
    body.append(&more);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&body)
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
    header.set_title_widget(Some(&clamp));
    let filters = Filters::new();
    // Shows or Reading leads the content bar, where Library has it.
    filters.row.prepend(&scope(ctx, Dest::Search, true));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&filters.row);
    states.widget.set_vexpand(true);
    content.append(&states.widget);
    let page = widgets::content_page(&t::nav_search(), &header, &content);

    let at = Rc::new(RefCell::new((ReadSearchQuery::default(), 1_u32)));
    let ask: Rc<dyn Fn()> = {
        let hub = Rc::downgrade(&ctx.hub);
        let (states, at, filters, entry) =
            (states.clone(), Rc::clone(&at), Rc::clone(&filters), entry.downgrade());
        Rc::new(move || {
            let (Some(hub), Some(entry)) = (hub.upgrade(), entry.upgrade()) else { return };
            let query = filters.query(&entry.text());
            *at.borrow_mut() = (query.clone(), 1);
            states.loading(None);
            send(&hub, ReadingCommand::Search { query });
        })
    };
    filters.on_change({
        let ask = Rc::clone(&ask);
        move || ask()
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
        let (ask, pending) = (Rc::clone(&ask), Rc::clone(&pending));
        move |_| {
            if let Some(id) = pending.borrow_mut().take() {
                id.remove();
            }
            let (ask, slot) = (Rc::clone(&ask), Rc::clone(&pending));
            let id = glib::timeout_add_local_once(Duration::from_millis(350), move || {
                slot.borrow_mut().take();
                ask();
            });
            *pending.borrow_mut() = Some(id);
        }
    });
    scroll.vadjustment().connect_value_changed({
        let (hub, at, has_more, asking_more, more) = (
            Rc::downgrade(&ctx.hub),
            Rc::clone(&at),
            Rc::clone(&has_more),
            Rc::clone(&asking_more),
            more.clone(),
        );
        move |adj| {
            let near_end = adj.value() + adj.page_size() > adj.upper() - 600.0;
            if !near_end || !has_more.get() || asking_more.get() {
                return;
            }
            if let Some(hub) = hub.upgrade() {
                asking_more.set(true);
                more.set_visible(true);
                let (query, page) = at.borrow().clone();
                send(&hub, ReadingCommand::SearchMore { query, page: page + 1 });
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
        let ask = Rc::clone(&ask);
        move || ask()
    });
    let tiles = Rc::new(RefCell::new(Vec::new()));
    listen(ctx, &page, {
        let (rows, has_more_now, asking_more) =
            (rows.clone(), Rc::clone(&has_more), Rc::clone(&asking_more));
        let (states, grid, more, at, tiles, filters) = (
            states.clone(),
            Rc::clone(&grid),
            more.clone(),
            Rc::clone(&at),
            Rc::clone(&tiles),
            Rc::clone(&filters),
        );
        let (line, other, other_words, empty, clear) =
            (line.clone(), other.clone(), other_words.clone(), empty.clone(), clear.clone());
        let (ask, hub) = (Rc::clone(&ask), Rc::downgrade(&ctx.hub));
        let (demo_rows, ctx_weak) = (demo_rows.clone(), Rc::downgrade(ctx));
        move |event| match event {
            Event::Reading(ReadingEvent::Search {
                query,
                page,
                tiles: found,
                has_more,
                other_scope,
                total,
            }) => {
                if *query != at.borrow().0 {
                    return;
                }
                if *page <= 1 {
                    tiles.borrow_mut().clear();
                }
                at.borrow_mut().1 = *page;
                tiles.borrow_mut().extend(found.iter().cloned());
                let browsing = query.text.is_empty();
                // While only the demo is on, a query lists its works in the demo's own rows.
                let only_demo = !browsing
                    && hub.upgrade().is_some_and(|h| h.known.borrow().demo.only_books);
                grid.widget.set_visible(browsing);
                rows.set_visible(!browsing && !only_demo);
                demo_rows.set_visible(only_demo);
                super::clear(&demo_rows);
                if only_demo {
                    if let Some(ctx) = ctx_weak.upgrade() {
                        demo_rows.append(&super::demo::search_results(&ctx, &tiles.borrow()));
                    }
                } else if browsing {
                    grid.set(&tiles.borrow());
                } else {
                    if *page <= 1 {
                        while let Some(row) = rows.first_child() {
                            rows.remove(&row);
                        }
                    }
                    if let Some(hub) = hub.upgrade() {
                        for tile in found {
                            rows.append(&super::work_row(&hub, tile));
                        }
                    }
                }
                has_more_now.set(*has_more);
                asking_more.set(false);
                more.set_visible(false);
                // Browsing says what it shows; a query says how many it found once it knows.
                let count = total.or((!*has_more).then(|| tiles.borrow().len() as u32));
                if browsing {
                    line.set_label(&browse_line(query, &filters.choices, *total));
                    line.set_visible(true);
                } else {
                    let words = count.map(|n| t::read_search_total(i64::from(n)));
                    line.set_label(words.as_deref().unwrap_or(""));
                    line.set_visible(words.is_some());
                }
                match other_scope.filter(|n| *n > 0 && !browsing) {
                    Some(n) => {
                        other_words.set_label(&t::search_other_scope(
                            i64::from(n),
                            &t::library_scope_shows(),
                        ));
                        other.set_visible(true);
                    }
                    None => other.set_visible(false),
                }
                if tiles.borrow().is_empty() && !only_demo {
                    let set = filters.any_set();
                    empty.set_title(&if browsing {
                        t::read_search_nothing_browse()
                    } else {
                        t::read_search_nothing(&query.text)
                    });
                    empty.set_description(Some(&if set {
                        t::read_search_nothing_filters_line()
                    } else {
                        t::read_search_nothing_line()
                    }));
                    clear.set_visible(set);
                    states.empty();
                } else {
                    states.content();
                }
            }
            Event::Reading(ReadingEvent::Failed {
                about: kaeri_model::reading::ReadingAbout::Search,
                failure,
            }) => {
                states.failed(&super::failure(failure));
            }
            Event::Medium(_) if hub.upgrade().is_some_and(|h| super::reads(&h)) => ask(),
            _ => {}
        }
    });
    // Shown with nothing asked yet, as when reading was turned on after the window opened.
    page.connect_shown({
        let (ask, tiles, hub) = (Rc::clone(&ask), Rc::clone(&tiles), Rc::downgrade(&ctx.hub));
        move |_| {
            if tiles.borrow().is_empty() && hub.upgrade().is_some_and(|h| super::reads(&h)) {
                ask();
            }
        }
    });
    if super::reads(&ctx.hub) {
        ask();
    }
    page.set_tag(Some("read-search"));
    page
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
