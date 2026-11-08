//! Reading's discovery parts, shared by Read and Search: a poster (the cover at 128 by 192 with
//! corners of 12, its title and one line under it), its placeholder at the same size, a sideways
//! row of posters that Tab leaves in one step, and the slot that holds placeholders while its
//! part is on its way and crossfades into what lands.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    Msg, ReadingMsg,
    reading::{ReadRowKind, WorkTile},
};

use crate::{hub::Hub, motion, say, squeeze::Squeeze, states, widgets};

/// A poster's cover: 128 by 192.
pub const POSTER: i32 = 128;
pub const POSTER_HEIGHT: i32 = 192;
/// Between posters in a row, and between columns of a grid.
pub const GAP: i32 = 16;
/// Between rows of a grid.
pub const GRID_ROW_GAP: i32 = 24;
/// Placeholders a row holds while it waits.
const PLACEHOLDERS: usize = 8;
/// The most posters a grid row holds, in the widest window.
const GRID_PLACEHOLDERS_ACROSS: i32 = 20;

/// What the line under a poster's title is made of, where it shows. A line never holds more than
/// two facts; the screen reader label says them all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caption {
    /// The facts as the engine gives them, on the person's own sections: "Manga · Releasing".
    Facts,
    /// Trending rows and a browse of one format: "Korea · Releasing".
    OriginStatus,
    /// New rows: "Japan · since March", or the status until the start month is known.
    OriginStarted,
    /// Finished rows, whose heading already says finished: "Japan · 112 chapters".
    OriginCount,
    /// Rows of mixed formats and a browse of several: "Manga · Finished".
    FormatStatus,
    /// An AniList result for typed words: "Light novel · Japan".
    FormatOrigin,
    /// A source's hit: how much of it the source holds, "2 volumes", or nothing.
    Amount,
}

impl Caption {
    /// The caption a Home row of `kind` takes.
    pub fn row(kind: ReadRowKind) -> Self {
        match kind {
            ReadRowKind::Trending => Self::OriginStatus,
            ReadRowKind::New => Self::OriginStarted,
            ReadRowKind::Finished => Self::OriginCount,
            ReadRowKind::Because | ReadRowKind::MoreGenre => Self::FormatStatus,
        }
    }
}

/// A tile's facts by what they say: its format, its status and how long it is.
#[derive(Default)]
struct Facts {
    format: Option<String>,
    status: Option<String>,
    count: Option<String>,
    origin: Option<String>,
    started: Option<String>,
}

fn facts_of(tile: &WorkTile) -> Facts {
    let mut facts = Facts {
        origin: tile.origin.as_ref().map(say::msg).filter(|w| !w.is_empty()),
        started: started_of(tile),
        ..Facts::default()
    };
    for (at, fact) in tile.facts.iter().enumerate() {
        let words = say::msg(fact);
        if words.is_empty() {
            continue;
        }
        let place = match fact {
            Msg::Reading(
                ReadingMsg::ReadingStatusReleasing
                | ReadingMsg::ReadingStatusFinished
                | ReadingMsg::ReadingStatusHiatus
                | ReadingMsg::ReadingStatusCancelled
                | ReadingMsg::ReadingStatusNotYet,
            ) => &mut facts.status,
            Msg::Reading(
                ReadingMsg::ReadingChapters { .. } | ReadingMsg::ReadingVolumes { .. },
            ) => &mut facts.count,
            // The engine puts the format first.
            _ if at == 0 => &mut facts.format,
            _ => continue,
        };
        place.get_or_insert(words);
    }
    facts
}

/// When a New row's series started, once the engine says.
fn started_of(_tile: &WorkTile) -> Option<String> {
    None
}

/// The parts of the line under a poster's title: Not found in your sources once a row's series
/// was opened and no source had it, else what comes next, else `caption`'s words.
fn parts(tile: &WorkTile, caption: Caption) -> Vec<String> {
    if tile.not_in_sources {
        return vec![t::home_not_found()];
    }
    if let Some(next) = &tile.next {
        return vec![say::msg(next)];
    }
    let Facts { format, status, count, origin, started } = facts_of(tile);
    let pair = |first: Option<String>, second: Option<String>| {
        first.into_iter().chain(second).collect::<Vec<_>>()
    };
    match caption {
        Caption::Facts => tile.facts.iter().map(say::msg).filter(|w| !w.is_empty()).collect(),
        Caption::OriginStatus => pair(origin.or(format), status),
        Caption::OriginStarted => pair(origin.or(format), started.or(status)),
        Caption::OriginCount => pair(origin.or(format), count.or(status)),
        Caption::FormatStatus => pair(format, status),
        Caption::FormatOrigin => pair(format, origin),
        Caption::Amount => tile.amount.iter().map(say::msg).filter(|w| !w.is_empty()).collect(),
    }
}

/// The line under a poster's title.
pub fn line_of(tile: &WorkTile, caption: Caption) -> String {
    parts(tile, caption).join(" \u{b7} ") // i18n-skip
}

/// Everything a screen reader hears about a poster under its title, whatever its line shows:
/// "Manga, Japan, Releasing, 112 chapters".
fn spoken_parts(tile: &WorkTile, caption: Caption) -> Vec<String> {
    if tile.not_in_sources || tile.next.is_some() || caption == Caption::Amount {
        return parts(tile, caption);
    }
    let Facts { format, status, count, origin, started } = facts_of(tile);
    let started = if caption == Caption::OriginStarted { started } else { None };
    [format, origin, status, started, count].into_iter().flatten().collect()
}

/// What a screen reader says for a poster: "Title. Manga, Japan, Releasing." and, after a miss,
/// "Title. Not found in your sources." A joined result adds where it is: "In Home library."
pub fn spoken(tile: &WorkTile, caption: Caption, joined: Option<&str>) -> String {
    let line = spoken_parts(tile, caption).join(", "); // i18n-skip
    let mut said = tile.title.clone();
    for part in [Some(line.as_str()), joined].into_iter().flatten().filter(|p| !p.is_empty()) {
        said = format!("{said}. {part}"); // i18n-skip
    }
    if tile.new_chapter {
        said = format!("{said}. {}", t::chapter_state_new()); // i18n-skip
    }
    format!("{said}.") // i18n-skip
}

/// `names` as one list: "Comics folder, Northwind and Demo".
pub fn names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => t::linux_reader_names_and(&rest.join(", "), last), // i18n-skip
    }
}

/// A cover with no art: a raised plate with the title set in it and a book at its foot. The plate
/// and its words are hidden from screen readers, which hear the caption once.
fn no_cover(title: &str) -> gtk::Box {
    let plate = gtk::Box::new(gtk::Orientation::Vertical, 0);
    plate.add_css_class("read-no-cover");
    plate.set_size_request(POSTER, POSTER_HEIGHT);
    plate.set_overflow(gtk::Overflow::Hidden);
    plate.set_accessible_role(gtk::AccessibleRole::Presentation);
    let words = widgets::show_title(title, "read-no-cover-title");
    words.set_lines(5);
    words.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.set_width_chars(1);
    words.set_max_width_chars(1);
    words.set_valign(gtk::Align::Start);
    words.set_vexpand(true);
    words.set_accessible_role(gtk::AccessibleRole::Presentation);
    plate.append(&words);
    let book = gtk::Image::from_icon_name("kaeri-book-symbolic");
    book.set_pixel_size(16);
    book.add_css_class("haze");
    book.set_halign(gtk::Align::Start);
    book.set_accessible_role(gtk::AccessibleRole::Presentation);
    plate.append(&book);
    plate
}

/// A poster's cover: the art at 128 by 192, or the plate with the title when there is none.
fn cover(hub: &Hub, tile: &WorkTile) -> gtk::Widget {
    match tile.cover.as_deref().filter(|u| !u.is_empty()) {
        Some(url) => {
            let (frame, _) = widgets::art(hub, Some(url), None, "read-poster");
            let fit = crate::fit::Fit::new(&frame, 2.0 / 3.0, POSTER, true, 0);
            fit.set_halign(gtk::Align::Start);
            fit.set_valign(gtk::Align::Start);
            fit.set_accessible_role(gtk::AccessibleRole::Presentation);
            fit.upcast()
        }
        None => no_cover(&tile.title).upcast(),
    }
}

/// One series as a poster: the cover, the track when it is being read, the title on one line
/// and the line under it, with `joined` in silk under that when one of the person's sources has
/// it. Opens the series; right-click opens its menu. A discovery poster (`plan`) offers Add to
/// Planning there and on Ctrl+D; the person's own series and a source's hits offer Details only.
pub fn poster(
    hub: &Hub,
    tile: &WorkTile,
    caption: Caption,
    joined: Option<&str>,
    plan: bool,
) -> gtk::Button {
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner.set_size_request(POSTER, -1);
    inner.append(&cover(hub, tile));
    let reading = tile.progress.is_some();
    if let Some(progress) = tile.progress {
        let bar = super::track(progress, POSTER);
        bar.set_margin_top(6);
        inner.append(&bar);
    }
    let title = widgets::show_title(&tile.title, "home-poster-title");
    title.set_wrap(false);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_width_chars(1);
    title.set_max_width_chars(1);
    title.set_margin_top(if reading { 6 } else { 8 });
    title.set_tooltip_text(Some(&tile.title));
    inner.append(&title);

    let sub = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    if tile.new_chapter {
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("home-new");
        dot.set_valign(gtk::Align::Center);
        sub.append(&dot);
    }
    let text = if tile.new_chapter && tile.next.is_none() {
        t::chapter_state_new()
    } else {
        line_of(tile, caption)
    };
    let line = gtk::Label::new(Some(&text));
    line.add_css_class("haze");
    line.add_css_class("home-sub");
    line.set_xalign(0.0);
    line.set_width_chars(1);
    line.set_max_width_chars(1);
    line.set_hexpand(true);
    // Its one line is held even when empty, so the row's captions line up. A miss may take two.
    line.set_size_request(-1, 16);
    if tile.not_in_sources {
        line.set_wrap(true);
        line.set_lines(2);
    } else {
        line.set_single_line_mode(true);
    }
    line.set_ellipsize(gtk::pango::EllipsizeMode::End);
    sub.append(&line);
    inner.append(&sub);

    if let Some(joined) = joined {
        let label = gtk::Label::new(Some(joined));
        label.add_css_class("read-joined");
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_width_chars(1);
        label.set_max_width_chars(1);
        label.set_margin_top(2);
        inner.append(&label);
    }

    let button = gtk::Button::new();
    button.set_child(Some(&inner));
    button.add_css_class("flat");
    button.add_css_class("home-card");
    button.set_halign(gtk::Align::Start);
    button.set_valign(gtk::Align::Start);
    button.set_hexpand(false);
    button.set_action_target_value(Some(&tile.work.0.to_variant()));
    button.set_action_name(Some("win.open-series"));
    button.update_property(&[gtk::accessible::Property::Label(&spoken(tile, caption, joined))]);
    if !plan {
        // The person's own series and a source's hits: Details only, never Add to Planning.
        let menu = gtk::gio::Menu::new();
        menu.append_item(&widgets::menu_item(&t::home_details(), "win.open-series", &tile.work.0));
        widgets::context_menu(&button, &menu);
        return button;
    }
    button.update_property(&[gtk::accessible::Property::KeyShortcuts(PLAN_KEYS_SPOKEN)]);
    super::work_menu(&button, &tile.work, Some((t::library_add_planning(), PLAN.to_owned())));
    // Add to Planning without the menu: GTK has no custom screen reader actions, so the poster
    // names its key for them.
    let keys = gtk::ShortcutController::new();
    let shortcut = gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string(PLAN_KEYS),
        Some(gtk::NamedAction::new(PLAN)),
    );
    shortcut.set_arguments(Some(&tile.work.0.to_variant()));
    keys.add_shortcut(shortcut);
    button.add_controller(keys);
    button
}

/// Add to Planning, on a work.
const PLAN: &str = "win.read-plan"; // i18n-skip
/// Its key on a focused poster, as GTK parses it and as ARIA writes it.
pub const PLAN_KEYS: &str = "<Control>d"; // i18n-skip
const PLAN_KEYS_SPOKEN: &str = "Control+D"; // i18n-skip

/// A poster as an item of a list, for the row or the grid that holds it.
pub fn item(poster: &impl IsA<gtk::Widget>) -> gtk::Box {
    let item = gtk::Box::new(gtk::Orientation::Vertical, 0);
    item.set_accessible_role(gtk::AccessibleRole::ListItem);
    item.set_halign(gtk::Align::Start);
    item.set_valign(gtk::Align::Start);
    item.append(poster);
    item
}

/// A poster's placeholder at its final size: the plate where the cover lands and two bars for its
/// words.
pub fn placeholder() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.set_size_request(POSTER, -1);
    card.append(&states::plate(POSTER, POSTER_HEIGHT, "read-poster-plate"));
    let title = states::plate(POSTER * 8 / 10, 12, "bar");
    title.set_margin_top(8);
    card.append(&title);
    let line = states::plate(POSTER / 2, 10, "bar");
    line.set_margin_top(8);
    // The caption's two lines take 34 px; the bars, 30.
    line.set_margin_bottom(4);
    card.append(&line);
    card
}

/// `n` placeholders in a row, clipped where the row ends: they ask for no width of their own.
pub fn placeholders(n: usize) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
    for _ in 0..n {
        row.append(&placeholder());
    }
    let clip = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(&row)
        .build();
    clip.set_can_target(false);
    clip.set_can_focus(false);
    clip.set_accessible_role(gtk::AccessibleRole::Presentation);
    clip.upcast()
}

/// Placeholders for a grid of posters: `rows` full rows at any width, 16 apart across and 24 down
/// as the posters land.
pub fn grid_placeholders(rows: i32) -> gtk::Widget {
    let layout = crate::cards_layout::CardsLayout::full_rows(POSTER, GAP, GRID_ROW_GAP, rows);
    let grid = gtk::Box::new(gtk::Orientation::Vertical, 0);
    grid.set_layout_manager(Some(layout));
    grid.set_valign(gtk::Align::Start);
    // As many as the widest window fits across; the layout shows whole rows of them.
    for _ in 0..GRID_PLACEHOLDERS_ACROSS * rows {
        grid.append(&placeholder());
    }
    grid.set_can_target(false);
    grid.set_can_focus(false);
    grid.set_accessible_role(gtk::AccessibleRole::Presentation);
    grid.upcast()
}

/// The placeholders of a Home row.
pub fn row_placeholders() -> gtk::Widget {
    placeholders(PLACEHOLDERS)
}

/// A paging button over a row's covers, centred on them. Hidden from Tab and screen readers: the
/// arrow keys and the scroll bar do the same work.
fn arrow(icon: &str, tip: &str, side: gtk::Align) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.add_css_class("circular");
    button.add_css_class("home-arrow");
    button.set_halign(side);
    button.set_valign(gtk::Align::Start);
    button.set_margin_top(POSTER_HEIGHT / 2 - 18);
    button.set_size_request(36, 36);
    button.set_focusable(false);
    button.set_can_focus(false);
    button.set_tooltip_text(Some(tip));
    button.set_visible(false);
    button.set_accessible_role(gtk::AccessibleRole::Presentation);
    button
}

/// Page a row by the whole posters that fit, landing on a poster's edge, on the standard token.
fn page_by(adj: &gtk::Adjustment, host: &gtk::ScrolledWindow, direction: i32) {
    let pitch = f64::from(POSTER + GAP);
    let fit = ((adj.page_size() + f64::from(GAP)) / pitch).floor().max(1.0);
    let at = (adj.value() / pitch).round();
    let end = (adj.upper() - adj.page_size()).max(adj.lower());
    let to = ((at + f64::from(direction) * fit) * pitch).clamp(adj.lower(), end);
    let target = adw::CallbackAnimationTarget::new({
        let adj = adj.clone();
        move |v| adj.set_value(v)
    });
    motion::standard(host, adj.value(), to, target).play();
}

/// A row of posters that scrolls sideways, named `name` as a list for screen readers. Arrows
/// move through it and keep the focused poster in view; Tab and Shift+Tab leave it in one step.
/// The paging buttons fade in while the pointer is over it.
pub fn strip(posters: &[gtk::Button], name: &str) -> gtk::Widget {
    let cards = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
    cards.set_accessible_role(gtk::AccessibleRole::List);
    cards.update_property(&[gtk::accessible::Property::Label(name)]);
    for poster in posters {
        cards.append(&item(poster));
    }
    let viewport = gtk::Viewport::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    viewport.set_child(Some(&cards));
    viewport.set_scroll_to_focus(true);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(&viewport)
        .build();
    let layers = gtk::Overlay::new();
    layers.set_child(Some(&scroll));
    let prev = arrow("go-previous-symbolic", &t::home_previous(), gtk::Align::Start);
    let next = arrow("go-next-symbolic", &t::home_next(), gtk::Align::End);
    layers.add_overlay(&prev);
    layers.add_overlay(&next);

    let hadj = scroll.hadjustment();
    let over = Rc::new(Cell::new(false));
    let fade = Rc::new((motion::Fader::default(), motion::Fader::default()));
    let refresh = {
        let (hadj, prev, next, over, fade) =
            (hadj.clone(), prev.clone(), next.clone(), Rc::clone(&over), Rc::clone(&fade));
        Rc::new(move || {
            let back = hadj.value() > 1.0;
            let forward = hadj.value() + hadj.page_size() < hadj.upper() - 1.0;
            fade.0.set(&prev, over.get() && back);
            fade.1.set(&next, over.get() && forward);
        })
    };
    hadj.connect_changed({
        let refresh = Rc::clone(&refresh);
        move |_| refresh()
    });
    hadj.connect_value_changed({
        let refresh = Rc::clone(&refresh);
        move |_| refresh()
    });
    let hover = gtk::EventControllerMotion::new();
    hover.connect_enter({
        let (over, refresh) = (Rc::clone(&over), Rc::clone(&refresh));
        move |_, _, _| {
            over.set(true);
            refresh();
        }
    });
    hover.connect_leave({
        let (over, refresh) = (Rc::clone(&over), Rc::clone(&refresh));
        move |_| {
            over.set(false);
            refresh();
        }
    });
    layers.add_controller(hover);
    prev.connect_clicked({
        let (hadj, scroll) = (hadj.clone(), scroll.clone());
        move |_| page_by(&hadj, &scroll, -1)
    });
    next.connect_clicked({
        let (hadj, scroll) = (hadj.clone(), scroll.clone());
        move |_| page_by(&hadj, &scroll, 1)
    });

    // Tab leaves the row: the focus goes to the row's last poster (its first, going back) with
    // nothing scrolled, and GTK moves it on from there to whatever follows the row.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let (cards, viewport) = (cards.clone(), viewport.clone());
        move |_, key, _, state| {
            use gtk::gdk::Key;
            if !matches!(key, Key::Tab | Key::ISO_Left_Tab | Key::KP_Tab) {
                return glib::Propagation::Proceed;
            }
            let back =
                key == Key::ISO_Left_Tab || state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
            let edge = if back { cards.first_child() } else { cards.last_child() };
            if let Some(poster) = edge.and_then(|item| item.first_child()) {
                viewport.set_scroll_to_focus(false);
                poster.grab_focus();
                let viewport = viewport.clone();
                glib::idle_add_local_once(move || viewport.set_scroll_to_focus(true));
            }
            glib::Propagation::Proceed
        }
    });
    cards.add_controller(keys);
    widgets::keep(&layers, (fade, refresh, over));
    layers.upcast()
}

/// A place whose part is on its way: nothing for 600 ms, then placeholders at final size, held at
/// least 0.3 s, then what landed fades in over them in 200 ms and they fade out under it in
/// 300 ms. The place keeps its height through the crossfade, then eases to what landed on the
/// standard spring, so a short answer or a failure never drops in one cut. Under Reduce
/// Animation the crossfade stays and the height changes at once.
pub struct Landing {
    /// The place, holding its height while what it shows changes.
    pub widget: Squeeze,
    overlay: gtk::Overlay,
    make: Box<dyn Fn() -> gtk::Widget>,
    /// The placeholders on show, and since when.
    shown_placeholders: RefCell<Option<(gtk::Widget, Instant)>>,
    content: RefCell<Option<gtk::Widget>>,
    timer: RefCell<Option<glib::SourceId>>,
    /// What lands once the placeholders have been held long enough.
    held: RefCell<Option<glib::SourceId>>,
    /// Told each time the place has something to show after showing nothing.
    on_visible: RefCell<Option<Box<dyn Fn()>>>,
    visible: Cell<bool>,
}

impl Landing {
    pub fn new(make: impl Fn() -> gtk::Widget + 'static) -> Rc<Self> {
        let overlay = gtk::Overlay::new();
        overlay.set_valign(gtk::Align::Start);
        let widget = Squeeze::new(&overlay, gtk::Orientation::Vertical, 0);
        widget.set_valign(gtk::Align::Start);
        Rc::new(Self {
            widget,
            overlay,
            make: Box::new(make),
            shown_placeholders: RefCell::default(),
            content: RefCell::default(),
            timer: RefCell::default(),
            held: RefCell::default(),
            on_visible: RefCell::default(),
            visible: Cell::new(false),
        })
    }

    /// Run `f` when the place has something to show after showing nothing.
    pub fn on_visible(&self, f: impl Fn() + 'static) {
        self.on_visible.replace(Some(Box::new(f)));
    }

    /// Whether anything shows yet.
    pub fn is_visible(&self) -> bool {
        self.visible.get()
    }

    fn seen(&self) {
        if !self.visible.replace(true)
            && let Some(f) = self.on_visible.borrow().as_ref()
        {
            f();
        }
    }

    /// Back to showing nothing, once the place has closed: the next part to land is told to
    /// [`Self::on_visible`] again.
    pub fn reset(&self) {
        self.stop();
        self.visible.set(false);
        self.content.take();
        self.shown_placeholders.take();
        self.overlay.set_child(None::<&gtk::Widget>);
    }

    /// `change` swaps what shows, crossfaded in place, and the height eases to the new one.
    fn swap(&self, change: impl FnOnce()) {
        if self.overlay.is_mapped() {
            self.widget.hold();
            motion::ghost(&self.overlay, &self.overlay, true, change);
            self.widget.release();
        } else {
            change();
        }
    }

    /// The part is on its way. With `keep`, what shows stays until the answer lands.
    pub fn wait(self: &Rc<Self>, keep: bool) {
        if let Some(id) = self.held.take() {
            id.remove();
        }
        if self.timer.borrow().is_some() || self.shown_placeholders.borrow().is_some() {
            return;
        }
        if keep && self.content.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(states::SILENT, move || {
            if let Some(landing) = weak.upgrade() {
                landing.timer.take();
                landing.show_placeholders();
            }
        });
        self.timer.replace(Some(id));
    }

    fn show_placeholders(&self) {
        let placeholders = (self.make)();
        match self.content.take() {
            Some(_) => self.swap(|| self.overlay.set_child(Some(&placeholders))),
            None => {
                // In a place already on screen they fade in; a part's place fades as it shows.
                let fade = self.overlay.is_mapped();
                self.overlay.set_child(Some(&placeholders));
                if fade {
                    motion::fade_in(&placeholders);
                }
            }
        }
        self.shown_placeholders.replace(Some((placeholders, Instant::now())));
        self.seen();
    }

    /// Nothing is on its way any more, as when the row goes: what was due is not shown.
    pub fn stop(&self) {
        if let Some(id) = self.timer.take() {
            id.remove();
        }
        if let Some(id) = self.held.take() {
            id.remove();
        }
    }

    /// What the part brought: posters or a failure.
    pub fn land(self: &Rc<Self>, content: &impl IsA<gtk::Widget>) {
        let content = content.as_ref().clone();
        if let Some(id) = self.timer.take() {
            id.remove();
        }
        if let Some(id) = self.held.take() {
            id.remove();
        }
        let since = self.shown_placeholders.borrow().as_ref().map(|(_, at)| *at);
        if let Some(since) = since {
            let left = states::AT_LEAST.saturating_sub(since.elapsed());
            if left.is_zero() {
                self.crossfade(content);
            } else {
                let weak = Rc::downgrade(self);
                let id = glib::timeout_add_local_once(left, move || {
                    if let Some(landing) = weak.upgrade() {
                        landing.held.take();
                        landing.crossfade(content);
                    }
                });
                self.held.replace(Some(id));
            }
            return;
        }
        if self.content.borrow().is_some() {
            // An answer over an answer: crossfaded in place.
            self.swap(|| self.overlay.set_child(Some(&content)));
        } else {
            let fade = self.overlay.is_mapped();
            self.overlay.set_child(Some(&content));
            if fade {
                motion::fade_in(&content);
            }
        }
        self.content.replace(Some(content));
        self.seen();
    }

    /// Fade `content` in over the placeholders, then them out under it, then ease the height.
    fn crossfade(self: &Rc<Self>, content: gtk::Widget) {
        let Some((placeholders, _)) = self.shown_placeholders.take() else {
            self.overlay.set_child(Some(&content));
            self.content.replace(Some(content));
            return;
        };
        self.content.replace(Some(content.clone()));
        if !self.overlay.is_mapped() {
            self.overlay.set_child(Some(&content));
            return;
        }
        self.widget.hold();
        content.set_valign(gtk::Align::Start);
        content.set_opacity(0.0);
        self.overlay.add_overlay(&content);
        self.overlay.set_measure_overlay(&content, true);
        let fade_in = timed(&content, 0.0, 1.0, 200, adw::Easing::EaseOutCubic);
        let weak = Rc::downgrade(self);
        fade_in.connect_done(move |_| {
            let Some(landing) = weak.upgrade() else { return };
            let fade_out = timed(&placeholders, 1.0, 0.0, 300, adw::Easing::EaseInCubic);
            let (weak, content) = (Rc::downgrade(&landing), content.clone());
            fade_out.connect_done(move |_| {
                let Some(landing) = weak.upgrade() else { return };
                landing.settle(&content);
                landing.widget.release();
            });
            fade_out.play();
        });
        fade_in.play();
    }

    /// The content takes the place's child, out of the overlay.
    /// A newer answer that landed meanwhile has the place already: this one only leaves.
    fn settle(&self, content: &gtk::Widget) {
        if content.parent().as_ref() == Some(self.overlay.upcast_ref())
            && self.overlay.child().as_ref() != Some(content)
        {
            self.overlay.remove_overlay(content);
        }
        content.set_opacity(1.0);
        if self.content.borrow().as_ref() == Some(content) {
            self.overlay.set_child(Some(content));
        }
    }
}

/// A part's place in a column: it fades in when something shows there after nothing did, and
/// opens its room on the standard spring when it comes back or would push what shows after it;
/// it leaves by fading, then closing its gap.
pub struct Part {
    pub place: Squeeze,
    pub inner: gtk::Box,
    pub landing: Rc<Landing>,
    fade: RefCell<Option<adw::TimedAnimation>>,
}

impl Part {
    /// `inner` holds the part's heading and `landing`; `shown` runs each time it shows.
    pub fn new(inner: gtk::Box, landing: Rc<Landing>, shown: impl Fn() + 'static) -> Rc<Self> {
        let place = Squeeze::new(&inner, gtk::Orientation::Vertical, 0);
        place.set_visible(false);
        let part = Rc::new(Self {
            place,
            inner,
            landing: Rc::clone(&landing),
            fade: RefCell::default(),
        });
        landing.on_visible({
            let part = Rc::downgrade(&part);
            move || {
                if let Some(part) = part.upgrade() {
                    part.reveal();
                    shown();
                }
            }
        });
        part
    }

    /// Whether it shows and is not on its way out.
    pub fn is_shown(&self) -> bool {
        self.place.is_visible() && !self.place.is_closing()
    }

    /// Whether something on screen comes after it, in its column or the one holding that.
    fn pushes(&self) -> bool {
        let after = |mut next: Option<gtk::Widget>| {
            while let Some(widget) = next {
                if widget.is_visible() && widget.is_mapped() && widget.height() > 0 {
                    return true;
                }
                next = widget.next_sibling();
            }
            false
        };
        after(self.place.next_sibling())
            || self.place.parent().is_some_and(|column| after(column.next_sibling()))
    }

    fn fade_to(&self, to: f64, done: impl Fn() + 'static) {
        if let Some(old) = self.fade.take() {
            old.pause();
        }
        let from = self.inner.opacity();
        let ms = if to > from { 200 } else { 300 };
        let animation =
            adw::TimedAnimation::new(&self.inner, from, to, ms, motion::opacity(&self.inner));
        animation.set_easing(if to > from {
            adw::Easing::EaseOutCubic
        } else {
            adw::Easing::EaseInCubic
        });
        animation.connect_done(move |_| done());
        animation.play();
        self.fade.replace(Some(animation));
    }

    /// Show the part: it fades in from wherever it was, opening on the spring when it was
    /// closing or closed, or when it would push what shows after it. One not placed yet starts
    /// once it is on screen.
    pub fn reveal(self: &Rc<Self>) {
        if self.is_shown() {
            return;
        }
        let hidden = !self.place.is_visible();
        let closing = self.place.is_closing();
        if hidden {
            self.inner.set_opacity(0.0);
        }
        let open = closing || (hidden && self.pushes());
        if hidden && open && !closing {
            self.place.set_closed();
        }
        self.place.set_visible(true);
        let part = Rc::downgrade(self);
        motion::once_mapped(&self.place, move || {
            let Some(part) = part.upgrade() else { return };
            if open || part.place.is_closing() {
                part.place.open(|| {});
            }
            part.fade_to(1.0, || {});
        });
    }

    /// The part goes: it fades out, its gap closes, and its landing starts over. Off screen it
    /// goes at once.
    pub fn leave(self: &Rc<Self>, done: impl Fn() + 'static) {
        self.landing.stop();
        if self.place.is_closing() || !self.place.is_visible() {
            return;
        }
        let part = Rc::downgrade(self);
        let gone = move || {
            if let Some(part) = part.upgrade() {
                part.place.set_visible(false);
                part.inner.set_opacity(1.0);
                part.landing.reset();
                done();
            }
        };
        if !self.place.is_mapped() {
            self.place.mark_closing();
            self.place.set_closed();
            gone();
            return;
        }
        self.place.mark_closing();
        let part = Rc::downgrade(self);
        let gone = Rc::new(gone);
        self.fade_to(0.0, move || {
            let (Some(part), gone) = (part.upgrade(), Rc::clone(&gone)) else { return };
            part.place.close(move || gone());
        });
    }
}

/// A timed fade on `widget`'s opacity that plays under Reduce Animation too, where the crossfade
/// is what reduced motion keeps.
fn timed(
    widget: &impl IsA<gtk::Widget>,
    from: f64,
    to: f64,
    ms: u32,
    easing: adw::Easing,
) -> adw::TimedAnimation {
    widget.as_ref().set_opacity(from);
    let animation = adw::TimedAnimation::new(widget, from, to, ms, motion::opacity(widget));
    animation.set_easing(easing);
    animation.set_follow_enable_animations_setting(false);
    animation
}

#[cfg(test)]
mod tests {
    use kaeri_model::{Msg, ReadingMsg};

    use super::*;

    #[test]
    fn names_are_one_list() {
        assert_eq!(names(&[]), "");
        assert_eq!(names(&["Demo".into()]), "Demo");
        assert_eq!(
            names(&["Comics folder".into(), "Northwind".into(), "Demo".into()]),
            "Comics folder, Northwind and Demo"
        );
    }

    fn tile() -> WorkTile {
        WorkTile {
            work: kaeri_core::books::WorkId("a:1".into()),
            title: "Lantern Street Notes".into(),
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
        }
    }

    #[test]
    fn a_miss_is_said_in_place_of_the_facts() {
        let mut tile = tile();
        tile.not_in_sources = true;
        assert_eq!(line_of(&tile, Caption::Facts), t::home_not_found());
        assert_eq!(
            spoken(&tile, Caption::Facts, None),
            format!("Lantern Street Notes. {}.", t::home_not_found())
        );
    }

    #[test]
    fn a_joined_result_says_where_it_is() {
        let tile = tile();
        assert_eq!(
            spoken(&tile, Caption::Facts, Some("In Home library")),
            "Lantern Street Notes. In Home library."
        );
    }

    fn reading(msg: ReadingMsg) -> Msg {
        Msg::Reading(msg)
    }

    #[test]
    fn each_place_says_its_own_caption() {
        let mut tile = tile();
        tile.facts = vec![
            reading(ReadingMsg::ReadingKindLightNovel),
            reading(ReadingMsg::ReadingStatusReleasing),
        ];
        tile.origin = Some(reading(ReadingMsg::ReadingOriginJapan));
        assert_eq!(line_of(&tile, Caption::Facts), "Light novel \u{b7} Releasing");
        assert_eq!(line_of(&tile, Caption::OriginStatus), "Japan \u{b7} Releasing");
        assert_eq!(line_of(&tile, Caption::FormatOrigin), "Light novel \u{b7} Japan");
        assert_eq!(line_of(&tile, Caption::Amount), "");
        assert_eq!(
            spoken(&tile, Caption::FormatOrigin, None),
            "Lantern Street Notes. Light novel, Japan, Releasing."
        );
        tile.origin = None;
        assert_eq!(line_of(&tile, Caption::OriginStatus), "Light novel \u{b7} Releasing");
        assert_eq!(line_of(&tile, Caption::FormatOrigin), "Light novel");
    }

    #[test]
    fn a_row_caption_holds_two_facts_and_the_label_all() {
        let mut tile = tile();
        tile.facts = vec![
            reading(ReadingMsg::ReadingKindManga),
            reading(ReadingMsg::ReadingStatusFinished),
            reading(ReadingMsg::ReadingChapters { count: 112 }),
        ];
        tile.origin = Some(reading(ReadingMsg::ReadingOriginJapan));
        let finished = Caption::row(ReadRowKind::Finished);
        let line = line_of(&tile, finished);
        assert_eq!(line.split(" \u{b7} ").count(), 2);
        assert!(line.starts_with("Japan \u{b7} ") && line.contains("112"), "{line}");
        assert_eq!(
            line_of(&tile, Caption::row(ReadRowKind::Trending)),
            "Japan \u{b7} Finished"
        );
        assert_eq!(line_of(&tile, Caption::row(ReadRowKind::Because)), "Manga \u{b7} Finished");
        let said = spoken(&tile, finished, None);
        assert!(said.starts_with("Lantern Street Notes. Manga, Japan, Finished, "), "{said}");
        // A light novel with no count keeps its status on a Finished row.
        tile.facts.truncate(2);
        assert_eq!(line_of(&tile, finished), "Japan \u{b7} Finished");
    }
}
