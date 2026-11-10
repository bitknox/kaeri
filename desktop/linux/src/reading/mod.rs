//! Reading: its places (Read, Updates, Library, Search, Downloads, Your sources), a series' page
//! and the reader. The engine decides what is shown and what it means; this draws it, and
//! reports what was painted. Books are made of the same parts the shows are: covers, shelves,
//! boxed lists and the states with their waiting rule.

pub mod came_in;
pub mod chapters;
pub mod demo;
pub mod discovery;
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
pub mod series_chapters;
pub mod series_extra;
pub mod server_progress;
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
    // The card is the cover's width whatever its words: titles wrap to two lines and end in an
    // ellipsis, and the track keeps its height with no progress so the rows line up.
    column.set_size_request(width, -1);
    column.set_hexpand(false);
    column.set_halign(gtk::Align::Start);
    column.add_css_class("read-card");
    column.append(&cover(hub, tile.cover.as_deref(), &tile.title, width));
    let bar = track(tile.progress.unwrap_or(0.0), width);
    if tile.progress.is_none() {
        bar.set_opacity(0.0);
        bar.set_accessible_role(gtk::AccessibleRole::Presentation);
    }
    column.append(&bar);
    let title = widgets::show_title(&tile.title, "row");
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_tooltip_text(Some(&tile.title));
    title.set_valign(gtk::Align::Start);
    title.set_width_chars(1);
    title.set_max_width_chars(1);
    // A wrapping label still asks for its whole line on a shelf's row, so a clamp holds it to
    // the cover's width.
    let clamp = adw::Clamp::builder()
        .maximum_size(width)
        .tightening_threshold(width)
        .child(&title)
        .build();
    clamp.set_halign(gtk::Align::Start);
    column.append(&clamp);
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
        label.set_width_chars(1);
        label.set_max_width_chars(1);
        column.append(&label);
        said.push(line);
    }
    column.update_property(&[gtk::accessible::Property::Label(&said.join(", "))]);
    opens_series(&column, &tile.work);
    work_menu(&column, &tile.work, None);
    column
}

/// The gap between cards in a grid, both ways, as the board draws it.
const GRID_GAP: i32 = 16;

/// A grid of covers, as many columns as fit, left-aligned with 16 px between them: the cards
/// keep their width and the room left over stays at the end of the row.
pub struct WorkGrid {
    pub widget: gtk::Box,
    hub: Rc<Hub>,
}

impl WorkGrid {
    pub fn new(hub: &Rc<Hub>) -> Self {
        let layout = crate::cards_layout::CardsLayout::new(COVER_WIDTH + 4, GRID_GAP);
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.set_layout_manager(Some(layout));
        widget.add_css_class("read-grid");
        widget.set_valign(gtk::Align::Start);
        widget.set_accessible_role(gtk::AccessibleRole::List);
        widget.update_property(&[gtk::accessible::Property::Label(&t::poster_grid_name())]);
        Self { widget, hub: Rc::clone(hub) }
    }

    pub fn set(&self, tiles: &[WorkTile]) {
        while let Some(child) = self.widget.first_child() {
            self.widget.remove(&child);
        }
        for tile in tiles {
            let card = work_card(&self.hub, tile, COVER_WIDTH);
            card.set_accessible_role(gtk::AccessibleRole::ListItem);
            self.widget.append(&card);
        }
    }
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
    let page = widgets::read_content_page(title, &header, &states.widget);
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
        // "Read chapter 12", but a chapter with no number goes by its own title: "Read Oneshot".
        SeriesAction::Read { chapter, label } if !numbered(label) => {
            Some((chapter.clone(), t::action_read_named(label)))
        }
        SeriesAction::Read { chapter, label } => {
            Some((chapter.clone(), t::action_read_chapter(label)))
        }
        SeriesAction::Continue { chapter, label, page } if !numbered(label) => {
            Some((chapter.clone(), t::action_continue_named_page(label, i64::from(*page))))
        }
        SeriesAction::Continue { chapter, label, page } => {
            Some((chapter.clone(), t::action_continue_chapter(label, i64::from(*page))))
        }
        // A part goes by the source's own name for it where it has one, as its row does.
        SeriesAction::ContinuePart { chapter, named: Some(named), .. } => {
            Some((chapter.clone(), t::action_continue_named_plain(named)))
        }
        SeriesAction::ContinuePart { chapter, label, named: None, .. } if !numbered(label) => {
            Some((chapter.clone(), t::action_continue_named_plain(label)))
        }
        SeriesAction::ContinuePart { chapter, label, named: None, .. } => {
            Some((chapter.clone(), t::action_continue_part_plain(label)))
        }
        SeriesAction::ReadPart { chapter, named: Some(named), .. } => {
            Some((chapter.clone(), t::action_read_named(named)))
        }
        SeriesAction::ReadPart { chapter, label, named: None } if !numbered(label) => {
            Some((chapter.clone(), t::action_read_named(label)))
        }
        SeriesAction::ReadPart { chapter, label, named: None } => {
            Some((chapter.clone(), t::action_read_part(label)))
        }
        SeriesAction::ReadAgain | SeriesAction::NoSource => None,
    }
}

/// The label is a chapter's number, "12", "10.5" or "12-13", which the words put "chapter" or
/// "part" before; anything else is the chapter's own title.
fn numbered(label: &str) -> bool {
    label.starts_with(|c: char| c.is_ascii_digit())
        && label.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-')
}

/// Open the reader on a chapter of a work: `win.read` takes the work and the chapter.
pub fn read_target(work: &WorkId, chapter: Option<&ChapterId>) -> glib::Variant {
    (work.0.clone(), chapter.map(|c| c.0.clone()).unwrap_or_default()).to_variant()
}

/// A button that opens a chapter in the reader.
pub fn read_button(label: &str, work: &WorkId, chapter: &ChapterId) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    // "Continue chapter 13 · page 10" in large text is wider than a narrow window: it wraps.
    if let Some(words) = button.child().and_downcast::<gtk::Label>() {
        words.set_wrap(true);
        words.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        words.set_justify(gtk::Justification::Center);
    }
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
        && view.rows.is_empty()
}

#[cfg(test)]
mod label_tests {
    use super::numbered;

    /// The series button puts "chapter" before a number, never before a chapter's own title.
    #[test]
    fn only_a_number_takes_the_word_chapter() {
        for label in ["12", "10.5", "12-13"] {
            assert!(numbered(label), "{label}");
        }
        for label in ["Oneshot", "The Lantern Festival", "1st Anniversary Extra", ""] {
            assert!(!numbered(label), "{label}");
        }
    }
}
