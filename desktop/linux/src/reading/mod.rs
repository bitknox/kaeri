//! Reading: its places (Read, Updates, Library, Search, Downloads, Your sources), a series' page
//! and the reader. The engine decides what is shown and what it means; this draws it, and
//! reports what was painted. Books are made of the same parts the shows are: covers, shelves,
//! boxed lists and the states with their waiting rule.

pub mod chapters;
pub mod downloads;
pub mod export;
//pub mod feed;
pub mod home;
pub mod library;
pub mod medium;
pub mod novel;
pub mod reader;
pub mod search;
pub mod series;
pub mod series_extra;
pub mod sources;
pub mod updates;

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_core::books::{ChapterId, WorkId};
use kaeri_i18n::t;
use kaeri_model::{
    Msg,
    command::Command,
    medium::KaeriIsFor,
    reading::{ReadingCommand, ReadingFailure, SeriesAction, WorkTile},
};

use crate::{
    fit::Fit,
    hub::Hub,
    motion,
    pages::Dest,
    say,
    states::{Shape, States},
    widgets,
    window::Ctx,
};

/// Width of a cover on a shelf and in a grid, at 2:3.
pub const COVER_WIDTH: i32 = 148;

/// Send a reading command.
pub fn send(hub: &Hub, command: ReadingCommand) {
    hub.send(Command::Reading(command));
}

/// Whether reading shows anywhere on this device.
pub fn reads(hub: &Hub) -> bool {
    hub.known.borrow().medium.as_ref().is_some_and(|m| m.reads())
}

/// Whether this build has reading at all.
pub fn available(hub: &Hub) -> bool {
    hub.known.borrow().medium.as_ref().is_some_and(|m| m.reading_available)
}

/// What Kaeri is for here: watching until the engine says otherwise.
pub fn answer(hub: &Hub) -> KaeriIsFor {
    hub.known.borrow().medium.as_ref().map_or(KaeriIsFor::Watch, |m| {
        if m.reading_available { m.answer } else { KaeriIsFor::Watch }
    })
}

/// Engine messages joined with a middle dot, in the language in use.
pub fn dotted(parts: &[Msg]) -> String {
    parts.iter().map(say::msg).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" \u{b7} ")
}

/// What failed, in plain words.
pub fn failure(failure: &ReadingFailure) -> String {
    say::msg(&failure.msg())
}

/// A work's cover at 2:3, `width` wide, named for screen readers by its title.
pub fn cover(hub: &Hub, url: Option<&str>, title: &str, width: i32) -> Fit {
    let (frame, _) = widgets::art(hub, url, None, "cover");
    if url.is_none_or(str::is_empty) {
        // No art yet: an outlined place, so a row on the raised colour still shows one.
        frame.add_css_class("cover-empty");
    }
    let fit = Fit::new(&frame, 2.0 / 3.0, width, true, 0);
    fit.set_halign(gtk::Align::Start);
    fit.set_valign(gtk::Align::Start);
    fit.update_property(&[gtk::accessible::Property::Label(title)]);
    fit
}

/// A thin track under a cover: how far into the series someone is.
pub fn track(fraction: f32, width: i32) -> gtk::ProgressBar {
    let bar = gtk::ProgressBar::new();
    bar.add_css_class("read-track");
    bar.set_fraction(f64::from(fraction.clamp(0.0, 1.0)));
    bar.set_size_request(width, -1);
    bar.set_halign(gtk::Align::Start);
    widgets::name_progress(&bar);
    bar
}

/// The small water dot that marks a new chapter, with its words beside it.
pub fn new_words() -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("home-new");
    dot.set_valign(gtk::Align::Center);
    line.append(&dot);
    let words = widgets::haze(&t::chapter_state_new(), "caption");
    line.append(&words);
    line
}

/// Open `work`'s page when `widget` is clicked or activated.
pub fn opens_series(widget: &impl IsA<gtk::Widget>, work: &WorkId) {
    let click = gtk::GestureClick::new();
    click.set_button(gtk::gdk::BUTTON_PRIMARY);
    click.connect_released({
        let work = work.0.clone();
        move |gesture, _, _, _| {
            if let Some(widget) = gesture.widget() {
                let _ = widget.activate_action("win.open-series", Some(&work.to_variant()));
            }
        }
    });
    widget.as_ref().add_controller(click);
    widget.as_ref().set_cursor_from_name(Some("pointer"));
    widget.as_ref().set_focusable(true);
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed({
        let work = work.0.clone();
        move |controller, key, _, _| {
            if matches!(
                key,
                gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter | gtk::gdk::Key::space
            ) && let Some(widget) = controller.widget()
            {
                let _ = widget.activate_action("win.open-series", Some(&work.to_variant()));
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    });
    widget.as_ref().add_controller(keys);
}

/// A right-click menu for a work: its page, and what the place adds.
pub fn work_menu(
    widget: &impl IsA<gtk::Widget>,
    work: &WorkId,
    extra: Option<(String, String)>,
) {
    let menu = gio::Menu::new();
    menu.append_item(&widgets::menu_item(
        &t::chapter_updates_open_series_menu(),
        "win.open-series",
        &work.0,
    ));
    if let Some((label, action)) = extra {
        menu.append_item(&widgets::menu_item(&label, &action, &work.0));
    }
    widgets::context_menu(widget, &menu);
}

/// One work on a shelf or in a grid: the cover, its title, and what the engine says under it.
pub fn work_card(hub: &Hub, tile: &WorkTile, width: i32) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.set_size_request(width.min(120), -1);
    column.set_halign(gtk::Align::Start);
    column.add_css_class("read-card");
    column.append(&cover(hub, tile.cover.as_deref(), &tile.title, width));
    if let Some(progress) = tile.progress {
        column.append(&track(progress, width));
    }
    let title = widgets::show_title(&tile.title, "row");
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_tooltip_text(Some(&tile.title));
    title.set_valign(gtk::Align::Start);
    title.set_max_width_chars(width / 8);
    column.append(&title);
    let mut said = vec![tile.title.clone()];
    if tile.new_chapter {
        column.append(&new_words());
        said.push(t::chapter_state_new());
    }
    // What comes next, or with nothing next the facts on one line. The format label is a key,
    // for filters, not words.
    let line = match &tile.next {
        Some(next) => say::msg(next),
        None => dotted(&tile.facts),
    };
    for line in [line].into_iter().filter(|l| !l.is_empty()) {
        let label = widgets::haze(&line, "caption");
        label.set_wrap(false);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_max_width_chars(width / 8);
        column.append(&label);
        said.push(line);
    }
    column.update_property(&[gtk::accessible::Property::Label(&said.join(", "))]);
    opens_series(&column, &tile.work);
    work_menu(&column, &tile.work, None);
    column
}

/// A work in a grid, kept in the grid's model.
type Item = glib::BoxedAnyObject;

/// A grid of covers, as many columns as fit, one work to a cell.
pub struct WorkGrid {
    pub widget: gtk::GridView,
    store: gio::ListStore,
}

impl WorkGrid {
    pub fn new(hub: &Rc<Hub>) -> Self {
        let store = gio::ListStore::new::<Item>();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_bind({
            let hub = Rc::clone(hub);
            move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let Some(object) = item.item().and_downcast::<Item>() else { return };
                let tile = object.borrow::<WorkTile>();
                item.set_child(Some(&work_card(&hub, &tile, COVER_WIDTH)));
            }
        });
        factory.connect_unbind(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                item.set_child(gtk::Widget::NONE);
            }
        });
        let selection = gtk::NoSelection::new(Some(store.clone()));
        let widget = gtk::GridView::new(Some(selection), Some(factory));
        widget.set_max_columns(12);
        widget.set_min_columns(2);
        widget.add_css_class("posters");
        widget.update_property(&[gtk::accessible::Property::Label(&t::poster_grid_name())]);
        widget.set_vexpand(true);
        Self { widget, store }
    }

    pub fn set(&self, tiles: &[WorkTile]) {
        let items: Vec<Item> = tiles.iter().cloned().map(Item::new).collect();
        self.store.splice(0, self.store.n_items(), &items);
    }
}

/// A heading with an aside and, at the end, a silk link to another place.
pub fn heading_row(title: &str, aside: Option<&str>, link: Option<(&str, Dest)>) -> gtk::Box {
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let label = gtk::Label::new(Some(title));
    label.add_css_class("home-heading");
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_accessible_role(gtk::AccessibleRole::Heading);
    head.append(&label);
    if let Some(aside) = aside {
        let aside = gtk::Label::new(Some(aside));
        aside.add_css_class("haze");
        aside.add_css_class("home-aside");
        head.append(&aside);
    }
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    head.append(&spacer);
    if let Some((text, dest)) = link {
        let words = gtk::Label::new(Some(text));
        let chevron = gtk::Image::from_icon_name("go-next-symbolic");
        chevron.set_pixel_size(12);
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        inner.append(&words);
        inner.append(&chevron);
        let button = gtk::Button::new();
        button.set_child(Some(&inner));
        button.add_css_class("flat");
        button.add_css_class("home-all");
        button.set_valign(gtk::Align::Center);
        button.set_action_target_value(Some(&dest.name().to_variant()));
        button.set_action_name(Some("win.go"));
        head.append(&button);
    }
    head
}

/// A row of covers that scrolls sideways, with paging buttons that show while the pointer is
/// over it.
pub fn shelf(cards: &[gtk::Widget], gap: i32) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, gap);
    for card in cards {
        row.append(card);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .child(&row)
        .build();
    scroll.set_propagate_natural_height(true);
    let layers = gtk::Overlay::new();
    layers.set_child(Some(&scroll));
    let arrows = [
        ("go-previous-symbolic", t::home_previous(), gtk::Align::Start, -1),
        ("go-next-symbolic", t::home_next(), gtk::Align::End, 1),
    ];
    let buttons: Vec<gtk::Button> = arrows
        .into_iter()
        .map(|(icon, tip, side, by)| {
            let button = gtk::Button::from_icon_name(icon);
            button.add_css_class("circular");
            button.add_css_class("home-arrow");
            button.set_halign(side);
            button.set_valign(gtk::Align::Start);
            button.set_margin_top(COVER_WIDTH * 3 / 4 - 18);
            button.set_focusable(false);
            button.set_tooltip_text(Some(&tip));
            button.set_visible(false);
            button.update_property(&[gtk::accessible::Property::Label(&tip)]);
            button.connect_clicked({
                let scroll = scroll.clone();
                move |_| page_by(&scroll, by, f64::from(COVER_WIDTH + gap))
            });
            layers.add_overlay(&button);
            button
        })
        .collect();
    let over = Rc::new(Cell::new(false));
    let refresh = {
        let hadj = scroll.hadjustment();
        let (back, forward) = (buttons[0].clone(), buttons[1].clone());
        let over = Rc::clone(&over);
        Rc::new(move || {
            back.set_visible(over.get() && hadj.value() > 1.0);
            forward.set_visible(
                over.get() && hadj.value() + hadj.page_size() < hadj.upper() - 1.0,
            );
        })
    };
    let hadj = scroll.hadjustment();
    hadj.connect_changed({
        let refresh = Rc::clone(&refresh);
        move |_| refresh()
    });
    hadj.connect_value_changed({
        let refresh = Rc::clone(&refresh);
        move |_| refresh()
    });
    let motion_ctl = gtk::EventControllerMotion::new();
    motion_ctl.connect_enter({
        let (over, refresh) = (Rc::clone(&over), Rc::clone(&refresh));
        move |_, _, _| {
            over.set(true);
            refresh();
        }
    });
    motion_ctl.connect_leave({
        let (over, refresh) = (Rc::clone(&over), Rc::clone(&refresh));
        move |_| {
            over.set(false);
            refresh();
        }
    });
    layers.add_controller(motion_ctl);
    layers.upcast()
}

/// Page a shelf by the whole cards that fit, on the standard token.
fn page_by(host: &gtk::ScrolledWindow, direction: i32, pitch: f64) {
    let adj = host.hadjustment();
    let fit = ((adj.page_size() + pitch - f64::from(COVER_WIDTH)) / pitch).floor().max(1.0);
    let at = (adj.value() / pitch).round();
    let end = (adj.upper() - adj.page_size()).max(adj.lower());
    let to = ((at + f64::from(direction) * fit) * pitch).clamp(adj.lower(), end);
    let target = adw::CallbackAnimationTarget::new({
        let adj = adj.clone();
        move |v| adj.set_value(v)
    });
    motion::standard(host, adj.value(), to, target).play();
}

/// A day as the updates and the week say it: Today, Yesterday, or the day's name and date.
pub fn day_label(day: i64) -> String {
    let now = glib::DateTime::now_local().ok();
    let then = glib::DateTime::from_unix_local(day).ok();
    if let (Some(now), Some(then)) = (now, then) {
        let today = now.day_of_year() + now.year() * 400;
        let that = then.day_of_year() + then.year() * 400;
        if that == today {
            return t::chapter_updates_today();
        }
        if that + 1 == today {
            return t::chapter_updates_yesterday();
        }
        if let Ok(text) = then.format("%A %-d %B") {
            return text.to_string();
        }
    }
    String::new()
}

/// The unix time `seconds` ago, in the words Updates uses.
pub fn ago(since: i64) -> String {
    let now = glib::real_time() / 1_000_000;
    say::ago((now - since).max(0))
}

/// Which of Shows and Reading a screen shared by both is on, kept for this device. Shown in the
/// header only with Both; with one medium the screen is that medium's and there is no control.
/// `reading` is the side the page is on, which its choice shows.
pub fn scope(ctx: &Rc<Ctx>, screen: Dest, reading: bool) -> gtk::Widget {
    let toggles = adw::ToggleGroup::new();
    toggles.add(adw::Toggle::builder().name("shows").label(t::library_scope_shows()).build());
    toggles
        .add(adw::Toggle::builder().name("reading").label(t::library_scope_reading()).build());
    toggles.add_css_class("round");
    toggles.set_valign(gtk::Align::Center);
    toggles.set_active_name(Some(if reading { "reading" } else { "shows" }));
    toggles.set_tooltip_text(Some(&t::library_scope_label()));
    toggles.set_visible(answer(&ctx.hub) == KaeriIsFor::Both);
    let settling = Cell::new(false);
    toggles.connect_active_name_notify({
        let ctx = Rc::downgrade(ctx);
        move |toggles| {
            let Some(ctx) = ctx.upgrade() else { return };
            if settling.get() {
                return;
            }
            let chosen = toggles.active_name().as_deref() == Some("reading");
            ctx.prefs.set_scope_reading(screen.name(), chosen);
            let _ = WidgetExt::activate_action(
                &ctx.window,
                "win.scope",
                Some(&screen.name().to_variant()),
            );
            // This page stays the side it is on, for when the choice comes back to it.
            settling.set(true);
            toggles.set_active_name(Some(if reading { "reading" } else { "shows" }));
            settling.set(false);
        }
    });
    let id = ctx.hub.listen({
        let toggles = toggles.downgrade();
        let hub = Rc::downgrade(&ctx.hub);
        move |event| {
            if matches!(event, kaeri_model::event::Event::Medium(_))
                && let (Some(toggles), Some(hub)) = (toggles.upgrade(), hub.upgrade())
            {
                toggles.set_visible(answer(&hub) == KaeriIsFor::Both);
            }
        }
    });
    toggles.connect_destroy({
        let hub = Rc::downgrade(&ctx.hub);
        move |_| {
            if let Some(hub) = hub.upgrade() {
                hub.forget(id);
            }
        }
    });
    toggles.upcast()
}

/// A page's navigation tag for a series, so one is never pushed over itself.
pub fn series_tag(work: &WorkId) -> String {
    format!("series-{}", work.0)
}

/// A place's page: its header bar, the states and the column its content is drawn into.
pub struct Place {
    pub page: adw::NavigationPage,
    pub header: adw::HeaderBar,
    pub states: Rc<States>,
    pub column: gtk::Box,
}

/// A destination of reading with its header (the sidebar button, the TV session button), the
/// states with the waiting rule, and a column to draw into.
pub fn place(
    ctx: &Rc<Ctx>,
    title: &str,
    shape: Shape,
    empty: &impl IsA<gtk::Widget>,
    failed: &str,
    wide: bool,
) -> Place {
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_start(&crate::window::tv_session_button(ctx));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
    let scroll = if wide {
        column.set_margin_top(12);
        column.set_margin_bottom(24);
        column.set_margin_start(24);
        column.set_margin_end(24);
        gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&column)
            .build()
    } else {
        widgets::column_page(&column, 820)
    };
    let states = States::new(&ctx.hub, &scroll, shape, empty, failed);
    let page = widgets::content_page(title, &header, &states.widget);
    Place { page, header, states, column }
}

/// Remove everything from a box.
pub fn clear(column: &gtk::Box) {
    while let Some(child) = column.first_child() {
        column.remove(&child);
    }
}

/// Listen to the engine until `widget` is gone.
pub fn listen(
    ctx: &Rc<Ctx>,
    widget: &impl IsA<gtk::Widget>,
    f: impl Fn(&kaeri_model::event::Event) + 'static,
) {
    let id = ctx.hub.listen(f);
    let hub = Rc::downgrade(&ctx.hub);
    widget.as_ref().connect_destroy(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(id);
        }
    });
}

/// The chapter and the words a series' main action opens, when it opens one.
pub fn action_chapter(action: &SeriesAction) -> Option<(ChapterId, String)> {
    match action {
        SeriesAction::Read { chapter, label } => {
            Some((chapter.clone(), t::action_read_chapter(label)))
        }
        SeriesAction::Continue { chapter, label, page } => {
            Some((chapter.clone(), t::action_continue_chapter(label, i64::from(*page))))
        }
        SeriesAction::ContinuePart { chapter, label, .. } => {
            Some((chapter.clone(), t::action_continue_part_plain(label)))
        }
        SeriesAction::ReadPart { chapter, label } => {
            Some((chapter.clone(), t::action_read_part(label)))
        }
        SeriesAction::ReadAgain | SeriesAction::NoSource => None,
    }
}

/// Open the reader on a chapter of a work: `win.read` takes the work and the chapter.
pub fn read_target(work: &WorkId, chapter: Option<&ChapterId>) -> glib::Variant {
    (work.0.clone(), chapter.map(|c| c.0.clone()).unwrap_or_default()).to_variant()
}

/// A button that opens a chapter in the reader.
pub fn read_button(label: &str, work: &WorkId, chapter: &ChapterId) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("pill");
    button.add_css_class("suggested-action");
    button.set_action_target_value(Some(&read_target(work, Some(chapter))));
    button.set_action_name(Some("win.read"));
    button
}

/// A reading destination's title in the sidebar's words.
pub fn title_of(dest: Dest) -> String {
    dest.title()
}

/// Whether a home has nothing to draw.
pub fn is_blank(view: &kaeri_model::reading::ReadingHomeView) -> bool {
    view.spotlight.is_none()
        && view.continue_reading.is_empty()
        && view.this_week.is_empty()
        && view.up_next.is_empty()
        && view.releasing.is_empty()
        && view.because.is_empty()
        && view.genre_shelf.is_empty()
}
