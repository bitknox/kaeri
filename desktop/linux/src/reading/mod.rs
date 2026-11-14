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

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

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

/// Settings, Reading, Reader defaults: Skip credit pages, on by default, a manga setting that
/// syncs. The switch follows the engine's settings for manga, and a flip sets them.
pub fn reader_defaults_group(ctx: &Rc<Ctx>) -> adw::PreferencesGroup {
    use kaeri_model::reader::{ReaderCommand, ReaderEvent, ReaderSettingKey, ReaderValue};
    let group = adw::PreferencesGroup::builder().title(t::settings_reader_defaults()).build();
    let row = adw::SwitchRow::builder()
        .title(glib::markup_escape_text(&t::settings_skip_credits()))
        .subtitle(glib::markup_escape_text(&t::settings_skip_credits_note()))
        .active(true)
        .build();
    group.add(&row);
    let key = ReaderSettingKey::SkipCreditPages;
    // The engine's word is what the switch shows; a flip it sends back is not sent again.
    let answering = Rc::new(Cell::new(false));
    listen(ctx, &row, {
        let (row, answering) = (row.downgrade(), Rc::clone(&answering));
        move |event| {
            if let kaeri_model::event::Event::Reader(ReaderEvent::Settings(view)) = event
                && view.medium == kaeri_core::media::Medium::Manga
                && view.work.is_none()
                && let Some(found) = view.rows.iter().find(|r| r.key == key)
                && let ReaderValue::Toggle { on } = found.value
                && let Some(row) = row.upgrade()
                && row.is_active() != on
            {
                answering.set(true);
                row.set_active(on);
                answering.set(false);
            }
        }
    });
    row.connect_active_notify({
        let hub = Rc::downgrade(&ctx.hub);
        move |row| {
            if answering.get() {
                return;
            }
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::Reader(ReaderCommand::SetSetting {
                    session: None,
                    medium: kaeri_core::media::Medium::Manga,
                    work: None,
                    key,
                    scope: key.home_scope(),
                    value: ReaderValue::Toggle { on: row.is_active() },
                }));
            }
        }
    });
    ctx.hub.send(Command::Reader(ReaderCommand::LoadSettings {
        medium: kaeri_core::media::Medium::Manga,
        work: None,
    }));
    group
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

/// A chapter row's line in the engine's words, with a group's language named in the app's
/// language from its tag ("from Group B, Spanish"), not the engine's English name.
pub fn row_line(chapter: &kaeri_model::reading::ChapterRow) -> String {
    use kaeri_model::ReadingMsg as R;
    let named = |tag: Option<&str>, english: &str| {
        tag.map_or_else(
            || english.to_owned(),
            |tag| say::language(tag.split(['-', '_']).next().unwrap_or(tag)),
        )
    };
    let line: Vec<Msg> = chapter
        .line
        .iter()
        .map(|msg| match msg {
            Msg::Reading(R::ReadingRowFromGroupIn { group, language }) => {
                Msg::Reading(R::ReadingRowFromGroupIn {
                    group: group.clone(),
                    language: named(chapter.group_language.as_deref(), language),
                })
            }
            Msg::Reading(R::ReadingRowPartFromGroupIn { part, group, language }) => {
                let tag = chapter
                    .part_groups
                    .iter()
                    .find(|p| i64::from(p.part) == *part)
                    .and_then(|p| p.language.as_deref());
                Msg::Reading(R::ReadingRowPartFromGroupIn {
                    part: *part,
                    group: group.clone(),
                    language: named(tag, language),
                })
            }
            other => other.clone(),
        })
        .collect();
    // The engine's words start in lower case, "from Group B, Spanish"; the row's line starts
    // with a capital.
    say::capitalized(&dotted(&line))
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
    /// The list on screen, and how many of its cards are built.
    tiles: RefCell<Vec<WorkTile>>,
    built: Cell<usize>,
    /// Whether a look at the view is already waiting for the main loop.
    asked: Cell<bool>,
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
        Self {
            widget,
            hub: Rc::clone(hub),
            tiles: RefCell::default(),
            built: Cell::new(0),
            asked: Cell::new(false),
        }
    }

    /// Builds more cards as `scroll`, which holds the grid, nears the last one built: a long
    /// list opens with its first cards and adds the rest as they come near, not all at once.
    /// The keyboard reaches them too, as the view follows the focus.
    pub fn follow(self: &Rc<Self>, scroll: &gtk::ScrolledWindow) {
        let ask = {
            let grid = Rc::downgrade(self);
            move |adjustment: &gtk::Adjustment| {
                let Some(grid) = grid.upgrade() else { return };
                if grid.asked.replace(true) {
                    return;
                }
                // After the layout that moved the view, never inside it.
                let (grid, adjustment) = (Rc::downgrade(&grid), adjustment.clone());
                glib::idle_add_local_once(move || {
                    let Some(grid) = grid.upgrade() else { return };
                    grid.asked.set(false);
                    grid.build(Some(View {
                        top: adjustment.value(),
                        height: adjustment.page_size(),
                        whole: adjustment.upper(),
                    }));
                });
            }
        };
        let adjustment = scroll.vadjustment();
        adjustment.connect_value_changed(ask.clone());
        adjustment.connect_changed(ask);
    }

    pub fn set(&self, tiles: &[WorkTile]) {
        while let Some(child) = self.widget.first_child() {
            self.widget.remove(&child);
        }
        *self.tiles.borrow_mut() = tiles.to_vec();
        self.built.set(0);
        self.build(None);
    }

    /// Builds the cards the view at `view` wants next, as `cards_to_build` counts them, as many as
    /// `per_turn` allows.
    fn build(&self, view: Option<View>) {
        let tiles = self.tiles.borrow();
        let from = self.built.get();
        let more = per_turn(from, cards_to_build(from, tiles.len(), view));
        for tile in &tiles[from..from + more] {
            let card = work_card(&self.hub, tile, COVER_WIDTH);
            card.set_accessible_role(gtk::AccessibleRole::ListItem);
            self.widget.append(&card);
        }
        self.built.set(from + more);
    }
}

/// How many of the `wanted` cards one turn of the main loop builds, with `built` built: the first
/// screens at once, then a few at a time. The cards a turn adds are laid out in the next frame,
/// so a few keep that frame short; the view asks again after it while it still wants more.
fn per_turn(built: usize, wanted: usize) -> usize {
    const STEP: usize = 8;
    if built == 0 { wanted } else { wanted.min(STEP) }
}

/// Where a scrolled view stands: its top, its height and the height of what it holds.
#[derive(Clone, Copy)]
struct View {
    top: f64,
    height: f64,
    whole: f64,
}

/// How many cards a grid builds next, with `built` of `total` built and the view at `view`: a
/// first batch, then another whenever the view comes within two of its heights of the end.
fn cards_to_build(built: usize, total: usize, view: Option<View>) -> usize {
    const BATCH: usize = 48;
    let wanted = match view {
        _ if built == 0 => BATCH,
        Some(v) if v.height > 0.0 && v.top + v.height * 3.0 >= v.whole => BATCH,
        _ => 0,
    };
    wanted.min(total.saturating_sub(built))
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
            // A bare number is a chapter, as its row and the reader call it.
            Some((chapter.clone(), t::action_continue_chapter_plain(label)))
        }
        SeriesAction::ReadPart { chapter, named: Some(named), .. } => {
            Some((chapter.clone(), t::action_read_named(named)))
        }
        SeriesAction::ReadPart { chapter, label, named: None } if !numbered(label) => {
            Some((chapter.clone(), t::action_read_named(label)))
        }
        SeriesAction::ReadPart { chapter, label, named: None } => {
            Some((chapter.clone(), t::action_read_chapter(label)))
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

#[cfg(test)]
mod build_tests {
    use super::{View, cards_to_build, per_turn};

    /// The first screens come at once; later cards a few per turn, so no frame lays out many.
    #[test]
    fn later_cards_come_a_few_per_turn() {
        assert_eq!(per_turn(0, 48), 48);
        let later = per_turn(48, 48);
        assert!(later > 0 && later <= 8, "{later} cards in one turn");
        assert_eq!(per_turn(48, 3), 3);
        assert_eq!(per_turn(48, 0), 0);
    }

    /// A long list opens with a few screens of cards, not all of them, and adds a batch as the
    /// view comes near the end of what is built, never past the list.
    #[test]
    fn a_long_list_builds_its_cards_as_the_view_comes_near_them() {
        let view = |top: f64| Some(View { top, height: 600.0, whole: 6000.0 });
        let first = cards_to_build(0, 2000, None);
        assert!(first > 0 && first < 100, "{first} cards first");
        assert_eq!(cards_to_build(0, 12, None), 12);
        assert_eq!(cards_to_build(first, 2000, view(0.0)), 0);
        assert!(cards_to_build(first, 2000, view(4500.0)) > 0);
        assert_eq!(cards_to_build(1990, 2000, view(5400.0)), 10);
        assert_eq!(cards_to_build(2000, 2000, view(5400.0)), 0);
        // A view not laid out yet asks for nothing.
        let unlaid = Some(View { top: 0.0, height: 0.0, whole: 0.0 });
        assert_eq!(cards_to_build(first, 2000, unlaid), 0);
    }
}
