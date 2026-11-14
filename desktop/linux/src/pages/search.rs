//! Search: a sidebar destination and the header bar's search button. Typing anywhere in the
//! window opens the search bar and searches. The filters stand under the bar from the start, so
//! the titles can be browsed by genre, year or format without a word typed. Before anything is
//! typed or chosen, recent searches, This season and the genres; then the poster grid, and the
//! Settings rows whose names match under "Settings".

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use kaeri_model::{
    browsing::SurprisePool,
    catalog::{ContentFor, ContentList, Entry},
    command::Command,
    event::Event,
    search::SearchQuery,
};

use kaeri_i18n::t;

use super::{search_filters::Filters, trending::Rail};

use crate::{
    catalog,
    states::{Shape, States},
    widgets::{self, PosterGrid},
    window::Ctx,
};

/// How many recent searches the page keeps.
const RECENT: usize = 6;

/// The Search destination, and the window's hold on its search bar.
pub struct SearchPage {
    pub page: adw::NavigationPage,
    inner: Rc<Inner>,
}

struct Inner {
    ctx: Rc<Ctx>,
    bar: gtk::SearchBar,
    entry: gtk::SearchEntry,
    body: gtk::Stack,
    /// The results' loading, failed and offline states, under the filters.
    states: Rc<States>,
    grid: PosterGrid,
    /// The results on screen, every page so far.
    entries: RefCell<Vec<Entry>>,
    /// The page to ask for as the end comes near, while there is one, and whether it is asked.
    next_page: Cell<Option<u32>>,
    asking_more: Cell<bool>,
    /// Under the results while the next page is on its way.
    more_spinner: adw::Spinner,
    caption: gtk::Label,
    /// Surprise me, there while no words are typed.
    surprise: gtk::Button,
    /// The Trending rail, for a shot to drive.
    #[cfg(feature = "fixtures")]
    trending: Rail,
    /// Under the field while the demo is the only source: other titles wait for a store.
    demo_line: gtk::Label,
    /// Whether searching as you type waits for Return, to stay within AniList's limits.
    paused: Cell<bool>,
    season: PosterGrid,
    settings: gtk::ListBox,
    settings_group: gtk::Box,
    recent: gtk::FlowBox,
    recent_group: gtk::Box,
    empty: adw::StatusPage,
    filters: Rc<Filters>,
    asked: RefCell<Option<SearchQuery>>,
    searches: RefCell<Vec<String>>,
    capture: Cell<bool>,
}

pub fn build(ctx: &Rc<Ctx>) -> SearchPage {
    let (bar, entry, toggle) = field(&t::search_linux_placeholder());
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_start(&crate::window::tv_session_button(ctx));
    header.pack_end(&toggle);

    let filters = Filters::new();
    // Surprise me: a random title from the filters in effect, while no words are typed.
    let surprise =
        widgets::surprise_button(ctx, &t::surprise_me(), &t::surprise_hint_filters(), false, {
            let filters = Rc::downgrade(&filters);
            move || {
                let filters = filters.upgrade()?;
                Some(SurprisePool::Filters { query: filters.query("") })
            }
        });
    header.pack_end(&surprise);

    // Before typing.
    let recent = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(6)
        .column_spacing(6)
        .row_spacing(6)
        .build();
    let recent_group = section(&t::search_recent(), &recent);
    recent_group.set_visible(false);
    let clear = gtk::Button::with_label(&t::action_clear());
    clear.add_css_class("flat");
    clear.set_halign(gtk::Align::End);
    clear.set_tooltip_text(Some(&t::search_linux_clear_recent()));
    if let Some(heading) = recent_group.first_child() {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        recent_group.remove(&heading);
        heading.set_hexpand(true);
        line.append(&heading);
        line.append(&clear);
        recent_group.prepend(&line);
    }
    let season = PosterGrid::new(&ctx.hub);
    season.widget.set_vexpand(false);
    let genres = genre_tiles(&filters.genres(), {
        let filters = Rc::downgrade(&filters);
        move |genre| {
            if let Some(filters) = filters.upgrade() {
                // A genre's titles, as its filter alone.
                filters.set(&SearchQuery { genres: vec![genre], ..SearchQuery::default() });
            }
        }
    });
    let before = gtk::Box::new(gtk::Orientation::Vertical, 18);
    before.set_margin_top(12);
    before.set_margin_bottom(24);
    before.append(&recent_group);
    let trending = Rail::new(ctx);
    trending.widget.set_margin_start(18);
    trending.widget.set_margin_end(18);
    before.append(&trending.widget);
    before.append(&section(&t::continue_this_season(), &season.widget));
    before.append(&section(&t::search_genres(), &genres));
    let before = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&before)
        .build();

    // Results.
    let grid = PosterGrid::new(&ctx.hub);
    grid.widget.set_vexpand(false);
    // At the margins This Season's covers have, so a cover is one width in both.
    grid.widget.set_margin_start(18);
    grid.widget.set_margin_end(18);
    let settings = gtk::ListBox::new();
    settings.add_css_class("boxed-list");
    settings.set_selection_mode(gtk::SelectionMode::None);
    let settings_group = section(&t::nav_settings(), &settings);
    let results = gtk::Box::new(gtk::Orientation::Vertical, 12);
    results.set_margin_bottom(24);
    // What the filters left out, said just above the titles.
    let caption = widgets::haze("", "caption");
    caption.set_xalign(0.0);
    caption.set_margin_start(18);
    caption.set_margin_end(18);
    caption.set_visible(false);
    results.append(&caption);
    results.append(&grid.widget);
    let more_spinner = adw::Spinner::new();
    more_spinner.set_size_request(24, 24);
    more_spinner.set_halign(gtk::Align::Center);
    more_spinner.set_visible(false);
    more_spinner
        .upcast_ref::<gtk::Widget>()
        .update_property(&[gtk::accessible::Property::Label(&t::search_loading_more())]);
    results.append(&more_spinner);
    results.append(&settings_group);
    let results = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&results)
        .build();

    let empty = crate::states::plain(&t::search_linux_nothing_found(), "");
    // With filters set, the way out, as the Mac offers it.
    let clear_filters = gtk::Button::with_label(&t::search_clear_filters());
    clear_filters.add_css_class("pill");
    clear_filters.set_halign(gtk::Align::Center);
    clear_filters.set_visible(false);
    clear_filters.connect_clicked({
        let filters = Rc::downgrade(&filters);
        move |_| {
            if let Some(filters) = filters.upgrade() {
                filters.clear();
            }
        }
    });
    empty.set_child(Some(&clear_filters));
    let states = States::new(&ctx.hub, &results, Shape::Grid, &empty, &t::search_failed());
    states.offline_words(&t::search_needs_connection(), &t::state_offline_detail());
    let body = gtk::Stack::new();
    body.set_vexpand(true);
    body.add_named(&before, Some("before"));
    body.add_named(&states.widget, Some("results"));

    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.append(&bar);
    // Shows or Reading leads the content bar, where Library has it.
    let scope = crate::reading::scope(ctx, super::Dest::Search, false);
    column.append(&super::search_filters::with_scope(ctx, &filters.row, &scope));
    column.append(&filters.chips);
    let demo_line = widgets::haze(&t::search_demo_only(), "caption");
    demo_line.set_margin_start(18);
    demo_line.set_margin_end(18);
    demo_line.set_visible({
        let known = ctx.hub.known.borrow();
        known.demo.on && known.demo.only
    });
    column.append(&demo_line);
    column.append(&body);

    let inner = Rc::new(Inner {
        #[cfg(feature = "fixtures")]
        trending: trending.clone(),
        ctx: Rc::clone(ctx),
        bar: bar.clone(),
        entry: entry.clone(),
        body,
        states,
        grid,
        entries: RefCell::default(),
        next_page: Cell::new(None),
        asking_more: Cell::new(false),
        more_spinner,
        caption,
        surprise,
        demo_line,
        paused: Cell::new(false),
        season,
        settings,
        settings_group,
        recent,
        recent_group,
        empty,
        filters: Rc::clone(&filters),
        asked: RefCell::default(),
        searches: RefCell::new(ctx.prefs.recent_searches()),
        capture: Cell::new(false),
    });

    inner.show_recent();
    // See all browses the period shown, sorted by Trending.
    trending.on_see_all({
        let filters = Rc::downgrade(&filters);
        move |period| {
            if let Some(filters) = filters.upgrade() {
                filters.set(&SearchQuery::trending(period));
            }
        }
    });
    clear.connect_clicked({
        let inner = Rc::downgrade(&inner);
        move |_| {
            if let Some(inner) = inner.upgrade() {
                inner.forget_searches();
            }
        }
    });
    entry.connect_search_changed({
        let inner = Rc::downgrade(&inner);
        move |_| {
            if let Some(inner) = inner.upgrade() {
                inner.search(true);
            }
        }
    });
    entry.connect_activate({
        let inner = Rc::downgrade(&inner);
        move |entry| {
            if let Some(inner) = inner.upgrade() {
                inner.remember(entry.text().trim());
                inner.search(false);
            }
        }
    });
    inner.filters.on_change({
        let inner = Rc::downgrade(&inner);
        move || {
            if let Some(inner) = inner.upgrade() {
                inner.search(true);
            }
        }
    });
    inner.states.on_retry({
        let inner = Rc::downgrade(&inner);
        move || {
            if let Some(inner) = inner.upgrade() {
                inner.asked.replace(None);
                inner.search(false);
            }
        }
    });
    // The next page as the end comes within a screen.
    results.vadjustment().connect_value_changed({
        let inner = Rc::downgrade(&inner);
        move |adjustment| {
            if let Some(inner) = inner.upgrade()
                && adjustment.upper() - adjustment.value() - adjustment.page_size()
                    < adjustment.page_size()
            {
                inner.more();
            }
        }
    });
    inner.settings.connect_row_activated(|list, row| {
        let pane = row.widget_name();
        let _ = list.activate_action("win.settings-pane", Some(&pane.to_variant()));
    });
    ctx.hub.listen({
        let inner = Rc::downgrade(&inner);
        move |event| {
            if let Some(inner) = inner.upgrade() {
                inner.heard(event);
            }
        }
    });
    // Typing elsewhere opens Search before it searches.
    bar.connect_search_mode_enabled_notify({
        let inner = Rc::downgrade(&inner);
        move |bar| {
            let Some(inner) = inner.upgrade() else { return };
            if bar.is_search_mode() && inner.capture.get() && !inner.on_screen() {
                let _ = WidgetExt::activate_action(
                    &inner.ctx.window,
                    "win.go",
                    Some(&"search".to_variant()),
                );
            }
        }
    });
    inner.capture_keys(true);
    ctx.hub.send(Command::LoadSearchFacets);

    let page = widgets::content_page(&t::nav_search(), &header, &column);
    SearchPage { page, inner }
}

impl SearchPage {
    /// The search bar open with the caret in it, as Search in the sidebar and Ctrl+F leave it.
    pub fn focus(&self) {
        open_field(&self.inner.bar, &self.inner.entry);
    }

    /// Typing anywhere in the window opens the search: on while the library shows, off while the
    /// player, which has keys of its own, does.
    pub fn capture_keys(&self, on: bool) {
        self.inner.capture_keys(on);
    }

    /// Browse by `query`'s filters with nothing typed, as a screenshot asks.
    #[cfg(feature = "fixtures")]
    pub fn browse(&self, query: &SearchQuery) {
        self.focus();
        self.inner.entry.set_text("");
        self.inner.filters.set(query);
    }

    /// The Trending rail, as a screenshot drives its period.
    #[cfg(feature = "fixtures")]
    pub fn trending(&self) -> &Rail {
        &self.inner.trending
    }

    /// The query on screen, and the next page asked for, as a screenshot of paging asks.
    #[cfg(feature = "fixtures")]
    pub fn asked(&self) -> Option<SearchQuery> {
        self.inner.asked.borrow().clone()
    }

    /// Ask for the next page, as reaching the end does.
    #[cfg(feature = "fixtures")]
    pub fn more(&self) {
        self.inner.more();
    }

    /// Search for `query`, as a screenshot or a deep link asks.
    pub fn search_for(&self, query: &str) {
        self.focus();
        self.inner.entry.set_text(query);
        self.inner.search(false);
    }
}

impl Inner {
    fn capture_keys(&self, on: bool) {
        self.capture.set(on);
        let window: gtk::Widget = self.ctx.window.clone().upcast();
        self.bar.set_key_capture_widget(if on { Some(&window) } else { None });
    }

    fn on_screen(&self) -> bool {
        self.bar.is_mapped()
    }

    /// What is typed and chosen, the text trimmed as the engine tags its answer.
    fn query(&self) -> SearchQuery {
        self.filters.query(&self.entry.text())
    }

    /// Search for what is typed and chosen: `live` as it changes, which the engine waits on
    /// until the changes stop, or at once on Return.
    fn search(&self, live: bool) {
        let query = self.query();
        self.filters.typed(!query.trimmed_text().is_empty());
        self.surprise.set_visible(query.trimmed_text().is_empty());
        if query.is_blank() {
            self.asked.replace(None);
            self.body.set_visible_child_name("before");
            return;
        }
        self.match_settings(&query.text);
        if live && query.waits_for_words() {
            return;
        }
        if self.asked.borrow().as_ref() == Some(&query) && (live || !self.paused.get()) {
            return;
        }
        self.asked.replace(Some(query.clone()));
        self.caption.set_visible(false);
        self.next_page.set(None);
        self.more_waits(false);
        self.body.set_visible_child_name("results");
        // The results on screen stay for 600 ms, then placeholders stand in their place.
        self.states.loading(Some(searching(&query)));
        self.ctx.hub.send(if live && !self.paused.get() {
            Command::SearchLive { query }
        } else {
            Command::Search { query }
        });
    }

    /// The next page of the results, when there is one and it is not already asked for.
    fn more(&self) {
        let Some(page) = self.next_page.get() else { return };
        let Some(query) = self.asked.borrow().clone() else { return };
        if self.asking_more.get() || !self.states.has_answer() {
            return;
        }
        self.more_waits(true);
        self.ctx.hub.send(Command::SearchMore { query, page });
    }

    fn more_waits(&self, on: bool) {
        self.asking_more.set(on);
        self.more_spinner.set_visible(on);
    }

    fn heard(&self, event: &Event) {
        match event {
            // Titles are named another way now: what was found, searched for again.
            Event::TitlesChanged(_) => {
                if let Some(query) = self.asked.borrow().clone() {
                    self.ctx.hub.send(Command::Search { query });
                }
            }
            Event::Demo(state) => self.demo_line.set_visible(state.on && state.only),
            Event::Facets(facets) => self.filters.set_genres(&facets.genres),
            Event::Content { view: ContentFor::Seasonal, content } => {
                let entries = content.entries();
                self.season.set(&entries[..entries.len().min(12)]);
            }
            Event::LiveSearch { paused } => {
                self.paused.set(*paused);
                // A change of text that was not searched: Return searches it.
                if *paused && !self.states.has_answer() && self.asked.borrow().is_some() {
                    self.asked.replace(None);
                    self.empty.set_title(&t::search_linux_press_enter());
                    if let Some(clear) = self.empty.child() {
                        clear.set_visible(false);
                    }
                    self.empty.set_description(Some(&t::search_linux_paused(
                        crate::say::tracker_name("anilist"),
                    )));
                    self.states.empty();
                }
            }
            Event::SearchPaging { query, has_next } => {
                if self.asked.borrow().as_ref() == Some(query) {
                    self.next_page.set(has_next.then_some(2));
                }
            }
            Event::SearchMore { query, page, entries, has_next } => {
                if self.asked.borrow().as_ref() != Some(query) {
                    return;
                }
                self.more_waits(false);
                if entries.is_empty() {
                    // That page failed: asked for again on the next scroll.
                    return;
                }
                let mut all = self.entries.borrow_mut();
                let new: Vec<Entry> = entries
                    .iter()
                    .filter(|e| !all.iter().any(|a| a.id == e.id))
                    .cloned()
                    .collect();
                all.extend(new);
                self.grid.set(&all);
                self.next_page.set(has_next.then_some(page + 1));
            }
            Event::Caption { view: ContentFor::Search { query }, text } => {
                if self.asked.borrow().as_ref() == Some(query) {
                    self.caption.set_label(text);
                    self.caption.set_visible(!text.is_empty());
                }
            }
            Event::Content { view: ContentFor::Search { query }, content } => {
                if self.asked.borrow().as_ref() != Some(query) {
                    return;
                }
                match content {
                    ContentList::Loading => {
                        self.states.loading(Some(searching(query)));
                    }
                    ContentList::Entries(entries) if !entries.is_empty() => {
                        self.entries.replace(entries.clone());
                        self.grid.set(entries);
                        self.grid.widget.set_visible(true);
                        self.states.content();
                    }
                    ContentList::Entries(_) | ContentList::Empty | ContentList::Failed(_) => {
                        self.entries.borrow_mut().clear();
                        self.grid.set(&[]);
                        self.grid.widget.set_visible(false);
                        self.next_page.set(None);
                        if self.settings_group.is_visible() {
                            self.states.content();
                        } else if let ContentList::Failed(why) = content {
                            self.states.failed(why);
                        } else {
                            self.empty.set_title(&nothing_found(query));
                            if let Some(clear) = self.empty.child() {
                                clear.set_visible(query.has_filters());
                            }
                            self.empty.set_description(Some(&if query.has_filters() {
                                t::search_try_fewer_filters()
                            } else {
                                t::search_nothing_found_detail()
                            }));
                            self.states.empty();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The Settings rows whose names match, under "Settings" at the end.
    fn match_settings(&self, query: &str) {
        while let Some(row) = self.settings.first_child() {
            self.settings.remove(&row);
        }
        let query = query.to_lowercase();
        let mut found = 0;
        if query.chars().count() >= 3 {
            let known = self.ctx.hub.known.borrow();
            let rows = known.settings.iter().flat_map(|v| v.rows.iter().chain(&v.native_rows));
            for row in rows {
                let Some(pane) = super::settings::pane_of(row.key) else { continue };
                let label = catalog::label(row.key);
                if !label.to_lowercase().contains(&query) {
                    continue;
                }
                let line = adw::ActionRow::builder()
                    .title(label)
                    .subtitle(format!(
                        "{} · {}",
                        super::settings::pane_title(pane),
                        catalog::display(row)
                    ))
                    .activatable(true)
                    .build();
                line.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                line.set_widget_name(pane);
                self.settings.append(&line);
                found += 1;
            }
        }
        self.settings_group.set_visible(found > 0);
    }

    fn remember(&self, query: &str) {
        if query.is_empty() {
            return;
        }
        {
            let mut searches = self.searches.borrow_mut();
            searches.retain(|s| s != query);
            searches.insert(0, query.to_owned());
            searches.truncate(RECENT);
            self.ctx.prefs.set_recent_searches(&searches);
        }
        self.show_recent();
    }

    fn forget_searches(&self) {
        self.searches.borrow_mut().clear();
        self.ctx.prefs.set_recent_searches(&[]);
        self.show_recent();
    }

    fn show_recent(&self) {
        while let Some(child) = self.recent.first_child() {
            self.recent.remove(&child);
        }
        let searches = self.searches.borrow();
        for search in searches.iter() {
            let chip = gtk::Button::with_label(search);
            chip.add_css_class("pill");
            chip.add_css_class("small");
            let entry = self.entry.clone();
            chip.connect_clicked(move |chip| entry.set_text(&chip.label().unwrap_or_default()));
            self.recent.append(&chip);
        }
        self.recent_group.set_visible(!searches.is_empty());
    }
}

/// A search field open with the caret in it, as Search in the sidebar leaves it on either side.
pub fn open_field(bar: &gtk::SearchBar, entry: &gtk::SearchEntry) {
    bar.set_search_mode(true);
    entry.grab_focus();
}

/// The search field in its bar under the header, open, and the header's button that opens and
/// closes it.
pub fn field(placeholder: &str) -> (gtk::SearchBar, gtk::SearchEntry, gtk::ToggleButton) {
    let entry = gtk::SearchEntry::builder().placeholder_text(placeholder).hexpand(true).build();
    // Screen readers name the field by its prompt, which a placeholder alone does not give it.
    entry.update_property(&[gtk::accessible::Property::Label(placeholder)]);
    let clamp = adw::Clamp::builder().maximum_size(480).child(&entry).build();
    let bar = gtk::SearchBar::builder().child(&clamp).show_close_button(false).build();
    bar.connect_entry(&entry);
    let toggle = gtk::ToggleButton::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text(t::nav_search())
        .build();
    toggle
        .bind_property("active", &bar, "search-mode-enabled")
        .sync_create()
        .bidirectional()
        .build();
    (bar, entry, toggle)
}

/// The start's genre tiles: `pick` gets the genre's own name when one is clicked.
pub fn genre_tiles(genres: &[String], pick: impl Fn(String) + 'static) -> gtk::FlowBox {
    let tiles = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(8)
        .column_spacing(8)
        .row_spacing(8)
        .homogeneous(true)
        .build();
    let pick = Rc::new(pick);
    for genre in genres {
        // A longer translation is cut at the end, so two tiles still fit a narrow window.
        let name = gtk::Label::new(Some(&crate::say::genre(genre)));
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let tile = gtk::Button::builder().child(&name).build();
        tile.add_css_class("genre-tile");
        tile.set_widget_name(genre);
        let pick = Rc::clone(&pick);
        tile.connect_clicked(move |tile| pick(tile.widget_name().to_string()));
        tiles.append(&tile);
    }
    tiles
}

/// The faint line while a search runs.
fn searching(query: &SearchQuery) -> String {
    if query.is_browse() {
        t::activity_browse()
    } else {
        t::search_searching(query.trimmed_text())
    }
}

/// What a search that found nothing says, from its text and filters.
fn nothing_found(query: &SearchQuery) -> String {
    match (query.is_browse(), query.has_filters()) {
        (true, true) => t::search_nothing_with_filters(),
        (true, false) => t::search_linux_nothing_found(),
        (false, true) => t::search_nothing_found_with_filters(query.trimmed_text()),
        (false, false) => t::search_nothing_found(query.trimmed_text()),
    }
}

/// A heading over a block of the page, at the page's margins.
pub fn section(title: &str, child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_margin_start(18);
    column.set_margin_end(18);
    column.append(&widgets::heading(title));
    column.append(child);
    column
}

#[cfg(test)]
mod field_tests {
    use adw::prelude::*;

    use super::{field, open_field};
    use crate::gtk_test::{pump, run};

    /// R7L-4. Search on either side opens its field with the caret in it, from the Search toggle
    /// too, where Return would otherwise hide the bar.
    #[test]
    fn opening_a_search_field_puts_the_caret_in_it() {
        run(|| {
            let (bar, entry, toggle) = field("Made-up placeholder");
            let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
            page.append(&toggle);
            page.append(&bar);
            let window = gtk::Window::new();
            window.set_child(Some(&page));
            window.present();
            bar.set_search_mode(false);
            toggle.grab_focus();
            pump(100);
            open_field(&bar, &entry);
            pump(100);
            assert!(bar.is_search_mode());
            assert!(toggle.is_active(), "the toggle follows the bar");
            let focus =
                gtk::prelude::GtkWindowExt::focus(&window).expect("something has focus");
            assert!(
                focus == *entry.upcast_ref::<gtk::Widget>() || focus.is_ancestor(&entry),
                "the caret is in the field, not on {}",
                focus.type_().name()
            );
            window.destroy();
        });
    }

    /// The field has a name for screen readers, its prompt, not only a placeholder.
    #[test]
    fn the_search_field_is_named() {
        run(|| {
            let (_, entry, _) = field("Made-up placeholder");
            assert!(gtk::test_accessible_has_property(&entry, gtk::AccessibleProperty::Label));
        });
    }
}
