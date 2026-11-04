//! The starting screen's widgets: the art under its ground, the show, the loader line, the step
//! and detail lines, the buttons by time, the Details list and the failure. It draws the
//! [`Frame`] the model (`start_model.rs`) gives and says what the viewer pressed.
//!
//! Nothing in the column moves once a start is on screen: the column's top is set when the start
//! begins (centred on the column as it is then, with the room of every button a start can show
//! kept from 0 s), lines are one line each, buttons keep their room when they come and go, and
//! Details and a longer failure grow downward only. Every fade runs on the frame clock, so the
//! crossfades stay crossfades with animations off.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

use adw::prelude::*;
use gtk::{gdk, glib};
use kaeri_core::media::Translation;
use kaeri_i18n::t;

use super::start_model::{Action, Button, Frame, HOLD, Look, Mark, Row};
use crate::motion;

/// Art brighter than this, by the mean relative luminance of its centre band, sits under the
/// deeper ground.
pub const BRIGHT: f64 = 0.15;

/// The column's widest, and the room kept around it.
const COLUMN: i32 = 520;
const SIDE: i32 = 24;
const TOP: i32 = 48;
const BOTTOM: i32 = 24;

/// The standard motion token, and a fade in.
const STANDARD: f64 = 300.0;
const FADE_IN: f64 = 200.0;

/// Opacity moved on the frame clock, ease-out towards 1 and ease-in towards 0. It runs whether
/// or not animations are on: the design keeps its fades under reduced motion.
#[derive(Clone)]
struct Fader {
    widget: gtk::Widget,
    run: Rc<RefCell<Option<gtk::TickCallbackId>>>,
}

impl Fader {
    fn new(widget: &impl IsA<gtk::Widget>) -> Self {
        Self { widget: widget.clone().upcast(), run: Rc::default() }
    }

    fn stop(&self) {
        if let Some(id) = self.run.borrow_mut().take() {
            id.remove();
        }
    }

    /// Set at once.
    fn set(&self, value: f64) {
        self.stop();
        self.widget.set_opacity(value);
    }

    /// Fade to `target` over `ms`, then call `done`.
    fn to_then(&self, target: f64, ms: f64, done: Option<Box<dyn Fn()>>) {
        self.stop();
        let from = self.widget.opacity();
        if (from - target).abs() < 0.001 || !self.widget.is_mapped() {
            self.widget.set_opacity(target);
            if let Some(done) = done {
                done();
            }
            return;
        }
        let began = Cell::new(None::<i64>);
        let run = Rc::clone(&self.run);
        let id = self.widget.add_tick_callback(move |widget, clock| {
            let now = clock.frame_time();
            let start = *began.get().get_or_insert(now);
            began.set(Some(start));
            let p = ((now - start) as f64 / (ms * 1000.0)).clamp(0.0, 1.0);
            let eased = if target > from { 1.0 - (1.0 - p).powi(3) } else { p.powi(3) };
            widget.set_opacity(from + (target - from) * eased);
            if p >= 1.0 {
                run.borrow_mut().take();
                if let Some(done) = &done {
                    done();
                }
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        self.run.replace(Some(id));
    }

    fn to(&self, target: f64, ms: f64) {
        self.to_then(target, ms, None);
    }
}

/// What decides whether a line crossfades: its text without the numbers, so a count that ticks
/// changes in place and anything else crossfades.
fn fade_key(text: &str) -> String {
    text.chars().filter(|c| !c.is_numeric()).collect()
}

/// Text that changes in place: the new text fades in over the old, where the old one is, on the
/// standard token. A change waits out 0.6 s after the one before, and the newest wins.
struct Crossfade {
    root: gtk::Overlay,
    pages: [Page; 2],
    on: Cell<usize>,
    text: RefCell<String>,
    key: RefCell<String>,
    swapped: Cell<Option<Instant>>,
}

struct Page {
    row: gtk::Box,
    dot: gtk::Box,
    label: gtk::Label,
    fade: Fader,
}

impl Page {
    fn new(css: &[&str]) -> Self {
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("start-dot");
        dot.set_visible(false);
        dot.set_valign(gtk::Align::Center);
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_single_line_mode(true);
        for class in css {
            label.add_css_class(class);
        }
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_valign(gtk::Align::Start);
        row.append(&dot);
        row.append(&label);
        let fade = Fader::new(&row);
        Self { row, dot, label, fade }
    }

    /// One line, cut at the end; a failure's second line may take two.
    fn lines(&self, two: bool) {
        self.label.set_single_line_mode(!two);
        self.label.set_wrap(two);
        self.label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        self.label.set_lines(if two { 2 } else { -1 });
    }
}

impl Crossfade {
    fn new(css: &[&str]) -> Self {
        let pages = [Page::new(css), Page::new(css)];
        let root = gtk::Overlay::new();
        root.set_child(Some(&pages[0].row));
        root.add_overlay(&pages[1].row);
        root.set_measure_overlay(&pages[1].row, true);
        pages[1].fade.set(0.0);
        Self {
            root,
            pages,
            on: Cell::new(0),
            text: RefCell::default(),
            key: RefCell::default(),
            swapped: Cell::new(None),
        }
    }

    /// The line from the start, with no fade.
    fn reset(&self, text: &str) {
        let on = self.on.get();
        for (i, page) in self.pages.iter().enumerate() {
            page.dot.set_visible(false);
            page.lines(false);
            page.label.set_label(if i == on { text } else { "" });
            page.fade.set(if i == on { 1.0 } else { 0.0 });
        }
        self.text.replace(text.to_owned());
        self.key.replace(fade_key(text));
        self.swapped.set(None);
    }

    /// Show `text`, crossfaded when it is another line, in place when only its numbers moved.
    /// `dot` marks a failure's title, and `two` lets a failure's reason take two lines.
    fn set(&self, text: &str, dot: bool, two: bool) {
        if *self.text.borrow() == text {
            return;
        }
        let key = format!("{dot}{two}{}", fade_key(text));
        let now = &self.pages[self.on.get()];
        if *self.key.borrow() == key || self.text.borrow().is_empty() {
            now.label.set_label(text);
            now.dot.set_visible(dot);
            now.lines(two);
            self.text.replace(text.to_owned());
            self.key.replace(key);
            return;
        }
        // The one before has its time first; the caller asks again on the next tick.
        if self.swapped.get().is_some_and(|at| at.elapsed() < HOLD) {
            return;
        }
        let next = 1 - self.on.get();
        let page = &self.pages[next];
        page.label.set_label(text);
        page.dot.set_visible(dot);
        page.lines(two);
        page.fade.to(1.0, STANDARD);
        now.fade.to(0.0, STANDARD);
        self.on.set(next);
        self.text.replace(text.to_owned());
        self.key.replace(key);
        self.swapped.set(Some(Instant::now()));
    }
}

/// The 2 pt loader line: silk at 14%, and an amber stretch 32% wide that moves across and back,
/// or, with animations off, rests at the start and breathes: 2 s from 100% to 40%, 2 s back.
struct Loader {
    root: gtk::Overlay,
    /// Never drawn: its colour is the amber, which the drawing reads from it.
    probe: gtk::Box,
    /// What the line has to say now: a percentage in 0..=100, or none.
    percent: Cell<Option<u32>>,
    /// How far the fill has come, 0..=1, and how far the determinate look has crossfaded in.
    fill: Cell<f64>,
    mix: Cell<f64>,
    /// Whether the line is on show: it goes, faded, when the start fails.
    shown: Cell<bool>,
    fade: Cell<f64>,
}

/// The breathing's opacity `seconds` into it: 100% at 0, 40% at 2 s, 100% again at 4 s.
pub fn breath(seconds: f64) -> f64 {
    0.7 + 0.3 * (seconds * std::f64::consts::TAU / 4.0).cos()
}

impl Loader {
    fn new() -> Rc<Self> {
        let area = gtk::DrawingArea::new();
        area.set_content_height(2);
        area.set_height_request(2);
        area.set_hexpand(true);
        area.set_valign(gtk::Align::Center);
        area.add_css_class("start-loader");
        area.set_accessible_role(gtk::AccessibleRole::Presentation);
        let probe = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        probe.add_css_class("start-amber");
        probe.set_can_target(false);
        probe.set_size_request(0, 0);
        let root = gtk::Overlay::new();
        root.add_css_class("start-line");
        root.set_child(Some(&area));
        root.add_overlay(&probe);
        let loader = Rc::new(Self {
            root,
            probe,
            percent: Cell::new(None),
            fill: Cell::new(0.0),
            mix: Cell::new(0.0),
            shown: Cell::new(true),
            fade: Cell::new(1.0),
        });
        let phase = Rc::new(Cell::new(0.0f64));
        let weak = Rc::downgrade(&loader);
        let drawn = Rc::clone(&phase);
        area.set_draw_func(move |area, cr, w, h| {
            let Some(loader) = weak.upgrade() else { return };
            let (w, h) = (f64::from(w), f64::from(h));
            let silk = area.color();
            let amber = loader.probe.color();
            let amber =
                (f64::from(amber.red()), f64::from(amber.green()), f64::from(amber.blue()));
            let fade = loader.fade.get();
            cr.set_source_rgba(
                f64::from(silk.red()),
                f64::from(silk.green()),
                f64::from(silk.blue()),
                0.14 * fade,
            );
            cr.rectangle(0.0, 0.0, w, h);
            let _ = cr.fill();
            let mix = loader.mix.get();
            // The stretch: moving, or resting and breathing under reduced motion.
            let (x, alpha) = if reduced_motion() {
                (0.0, breath(drawn.get()))
            } else {
                let t = (drawn.get() / 1.4) % 2.0;
                let t = if t > 1.0 { 2.0 - t } else { t };
                let eased = t * t * (3.0 - 2.0 * t);
                (eased * (1.0 - 0.32) * w, 1.0)
            };
            cr.set_source_rgba(amber.0, amber.1, amber.2, alpha * (1.0 - mix) * fade);
            cr.rectangle(x, 0.0, 0.32 * w, h);
            let _ = cr.fill();
            if mix > 0.0 {
                cr.set_source_rgba(amber.0, amber.1, amber.2, mix * fade);
                cr.rectangle(0.0, 0.0, loader.fill.get() * w, h);
                let _ = cr.fill();
            }
        });
        let weak = Rc::downgrade(&loader);
        let last = Cell::new(None::<i64>);
        area.add_tick_callback(move |area, _| {
            let Some(loader) = weak.upgrade() else { return glib::ControlFlow::Break };
            // The wall clock, not the frame clock's smoothed time, which runs slow when frames
            // come late: the loop and the breathing keep their seconds however the frames land.
            let now = glib::monotonic_time();
            let dt = last.replace(Some(now)).map_or(0.0, |was| (now - was) as f64 / 1e6);
            phase.set(phase.get() + dt);
            // The fill follows each percentage and the determinate look fades in over 0.2 s.
            let target = loader.percent.get().map_or(0.0, |p| f64::from(p) / 100.0);
            if reduced_motion() {
                loader.fill.set(target);
            } else {
                loader.fill.set(
                    loader.fill.get() + (target - loader.fill.get()) * (dt * 8.0).min(1.0),
                );
            }
            let want = if loader.percent.get().is_some() { 1.0 } else { 0.0 };
            let step = dt / 0.2;
            let mix = loader.mix.get();
            loader.mix.set(if want > mix {
                (mix + step).min(want)
            } else {
                (mix - step).max(want)
            });
            let fade_to = if loader.shown.get() { 1.0 } else { 0.0 };
            let fade = loader.fade.get();
            let rate = dt / 0.3;
            loader.fade.set(if fade_to > fade {
                (fade + rate).min(fade_to)
            } else {
                (fade - rate).max(fade_to)
            });
            // Once its fade away is done, the line has nothing to draw until it shows again.
            if fade > 0.0 || loader.fade.get() > 0.0 {
                area.queue_draw();
            }
            glib::ControlFlow::Continue
        });
        loader
    }
}

/// Whether the system asks for animations to be off.
fn reduced_motion() -> bool {
    !gtk::Settings::default().is_some_and(|s| s.is_gtk_enable_animations())
}

/// The longest a Skip label runs before it is cut, in characters, and the name the kept room
/// is measured with.
const SKIP_CHARS: i32 = 18;
fn skip_room() -> String {
    t::player_start_skip_source(&"m".repeat(12))
}

/// A button of the starting screen, drawn as its look says.
fn start_button(
    action: Action,
    label: &str,
    look: Look,
) -> (gtk::Button, gtk::Label, Option<gtk::Image>) {
    let text = gtk::Label::new(Some(label));
    if action == Action::Skip {
        text.set_max_width_chars(SKIP_CHARS);
        text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    }
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    inner.append(&text);
    let chevron = (action == Action::Details).then(|| {
        let image = gtk::Image::from_icon_name("pan-down-symbolic");
        inner.append(&image);
        image
    });
    let button = gtk::Button::new();
    button.set_child(Some(&inner));
    button.set_valign(gtk::Align::Start);
    button.add_css_class("start-button");
    match look {
        Look::Primary => button.add_css_class("suggested-action"),
        Look::Quiet => button.add_css_class("flat"),
        Look::Tonal => {}
    }
    (button, text, chevron)
}

/// One button of the waiting row: it fades in where its room is, and out when it stops
/// applying, and keeps its room either way.
struct Slot {
    button: gtk::Button,
    label: gtk::Label,
    chevron: Option<gtk::Image>,
    present: Cell<bool>,
    fade: Fader,
}

impl Slot {
    fn new(action: Action, label: &str, look: Look) -> Self {
        let (button, label, chevron) = start_button(action, label, look);
        let fade = Fader::new(&button);
        fade.set(0.0);
        button.set_can_target(false);
        button.set_focusable(false);
        Self { button, label, chevron, present: Cell::new(false), fade }
    }

    fn show(&self, on: bool, immediately: bool) {
        if self.present.replace(on) == on {
            return;
        }
        self.button.set_can_target(on);
        self.button.set_focusable(on);
        let to = if on { 1.0 } else { 0.0 };
        if immediately {
            self.fade.set(to);
        } else {
            self.fade.to(to, if on { FADE_IN } else { STANDARD });
        }
    }
}

/// A row of buttons that wraps at the largest text sizes.
fn wrap() -> adw::WrapBox {
    let row = adw::WrapBox::new();
    row.set_child_spacing(8);
    row.set_line_spacing(8);
    row.set_align(0.0);
    row.set_valign(gtk::Align::Start);
    row
}

/// Which row of buttons is on show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rows {
    Waiting,
    Offline,
    Failing,
}

/// What the view tells the player.
pub type OnAction = Rc<dyn Fn(Action)>;

type OnTranslation = Rc<dyn Fn(Translation)>;

const WAITING: [Action; 5] = [
    Action::Cancel,
    Action::FromBeginning,
    Action::ChooseSource,
    Action::Details,
    Action::Skip,
];

pub struct StartView {
    pub root: gtk::Box,
    pub art: gtk::Picture,
    ground: gtk::Box,
    column: gtk::Box,
    /// Where the column's top is, for the size it was placed at.
    anchor: Cell<Option<(i32, i32, i32)>>,
    title: gtk::Label,
    episode: gtk::Label,
    credit: gtk::Label,
    loader: Rc<Loader>,
    step: Crossfade,
    detail: Crossfade,
    detail_fade: Fader,
    detail_on: Cell<bool>,
    /// The row's kept room: every button a start can show, never drawn.
    room: adw::WrapBox,
    waiting: adw::WrapBox,
    offline: adw::WrapBox,
    failing: adw::WrapBox,
    row_fades: [Fader; 3],
    rows_on: Cell<Rows>,
    slots: Vec<(Action, Slot)>,
    failing_row: RefCell<Vec<Action>>,
    details: gtk::ScrolledWindow,
    details_fade: Fader,
    details_open: Cell<bool>,
    rows_box: gtk::Box,
    shown_rows: RefCell<Vec<Row>>,
    row_times: RefCell<Vec<gtk::Label>>,
    language: gtk::Box,
    sub: gtk::ToggleButton,
    dub: gtk::ToggleButton,
    quiet: Cell<bool>,
    on_action: RefCell<Option<OnAction>>,
    on_translation: RefCell<Option<OnTranslation>>,
    first: Cell<bool>,
}

impl StartView {
    pub fn new() -> Rc<Self> {
        let art =
            gtk::Picture::builder().accessible_role(gtk::AccessibleRole::Presentation).build();
        art.set_content_fit(gtk::ContentFit::Cover);
        art.set_can_shrink(true);
        art.set_opacity(0.0);
        let ground = gtk::Box::new(gtk::Orientation::Vertical, 0);
        ground.add_css_class("start-ground");

        let title = gtk::Label::new(None);
        title.add_css_class("show-title");
        title.add_css_class("start-title");
        title.set_xalign(0.0);
        title.set_wrap(true);
        title.set_lines(2);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_accessible_role(gtk::AccessibleRole::Heading);
        let episode = gtk::Label::new(None);
        episode.add_css_class("haze");
        episode.add_css_class("start-episode");
        episode.set_xalign(0.0);
        episode.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let credit = gtk::Label::new(Some(&t::player_demo_credit()));
        credit.add_css_class("haze");
        credit.add_css_class("caption");
        credit.set_wrap(true);
        credit.set_xalign(0.0);
        credit.set_visible(false);

        let loader = Loader::new();
        let step = Crossfade::new(&["start-step"]);
        let detail = Crossfade::new(&["haze", "start-detail"]);
        step.root.set_margin_top(12);
        detail.root.set_margin_top(3);
        let detail_fade = Fader::new(&detail.root);

        // The row: its kept room under the three rows that crossfade on it.
        let room = wrap();
        room.set_can_target(false);
        room.set_can_focus(false);
        room.set_opacity(0.0);
        room.upcast_ref::<gtk::Widget>().update_state(&[gtk::accessible::State::Hidden(true)]);
        let waiting = wrap();
        let slots: Vec<(Action, Slot)> = WAITING
            .into_iter()
            .map(|action| {
                let (label, look) = match action {
                    Action::Cancel => (t::action_cancel(), Look::Quiet),
                    Action::FromBeginning => (t::player_start_from_beginning(), Look::Tonal),
                    Action::ChooseSource => (t::player_choose_source(), Look::Tonal),
                    Action::Details => (t::player_start_details(), Look::Quiet),
                    _ => (skip_room(), Look::Tonal),
                };
                (action, Slot::new(action, &label, look))
            })
            .collect();
        for (_, slot) in &slots {
            waiting.append(&slot.button);
        }
        let offline = wrap();
        let failing = wrap();
        let row_box = gtk::Overlay::new();
        row_box.add_css_class("start-actions");
        row_box.set_child(Some(&room));
        for row in [&waiting, &offline, &failing] {
            row_box.add_overlay(row);
            row_box.set_measure_overlay(row, true);
        }
        let row_fades = [Fader::new(&waiting), Fader::new(&offline), Fader::new(&failing)];
        for fade in &row_fades[1..] {
            fade.set(0.0);
        }

        let rows_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        rows_box.add_css_class("start-rows");
        let sub = gtk::ToggleButton::with_label(&t::audio_subbed());
        let dub = gtk::ToggleButton::with_label(&t::audio_dubbed());
        dub.set_group(Some(&sub));
        let toggles = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        toggles.add_css_class("linked");
        toggles.append(&sub);
        toggles.append(&dub);
        let language_label = gtk::Label::new(Some(&t::player_start_translation()));
        language_label.add_css_class("haze");
        language_label.set_xalign(0.0);
        language_label.set_hexpand(true);
        let language = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        language.add_css_class("start-language");
        language.append(&language_label);
        language.append(&toggles);
        language.set_visible(false);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
        list.append(&rows_box);
        list.append(&language);
        let details = gtk::ScrolledWindow::new();
        details.set_hscrollbar_policy(gtk::PolicyType::Never);
        details.set_propagate_natural_height(true);
        details.set_child(Some(&list));
        details.set_visible(false);
        details.set_vexpand(false);
        let details_fade = Fader::new(&details);

        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.add_css_class("start-column");
        column.set_valign(gtk::Align::Start);
        column.append(&title);
        column.append(&episode);
        column.append(&credit);
        column.append(&loader.root);
        column.append(&step.root);
        column.append(&detail.root);
        column.append(&row_box);
        column.append(&details);

        let stack = gtk::Overlay::new();
        stack.set_child(Some(&art));
        stack.add_overlay(&ground);
        stack.add_overlay(&column);
        stack.set_vexpand(true);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("player-starting");
        root.append(&stack);

        let view = Rc::new(Self {
            root,
            art,
            ground,
            column,
            anchor: Cell::new(None),
            title,
            episode,
            credit,
            loader,
            step,
            detail,
            detail_fade,
            detail_on: Cell::new(false),
            room,
            waiting,
            offline,
            failing,
            row_fades,
            rows_on: Cell::new(Rows::Waiting),
            slots,
            failing_row: RefCell::default(),
            details,
            details_fade,
            details_open: Cell::new(false),
            rows_box,
            shown_rows: RefCell::default(),
            row_times: RefCell::default(),
            language,
            sub,
            dub,
            quiet: Cell::new(false),
            on_action: RefCell::default(),
            on_translation: RefCell::default(),
            first: Cell::new(true),
        });
        let weak = Rc::downgrade(&view);
        stack.connect_get_child_position(move |overlay, widget| {
            let view = weak.upgrade()?;
            (widget == view.column.upcast_ref::<gtk::Widget>()).then(|| view.place(overlay))
        });
        for (action, slot) in &view.slots {
            let weak = Rc::downgrade(&view);
            let action = *action;
            slot.button.connect_clicked(move |_| {
                if let Some(view) = weak.upgrade() {
                    view.pressed(action);
                }
            });
        }
        for (toggle, translation) in
            [(&view.sub, Translation::Sub), (&view.dub, Translation::Dub)]
        {
            let weak = Rc::downgrade(&view);
            toggle.connect_toggled(move |button| {
                let Some(view) = weak.upgrade() else { return };
                if button.is_active() && !view.quiet.get() {
                    let on = view.on_translation.borrow().clone();
                    if let Some(on) = on {
                        on(translation);
                    }
                }
            });
        }
        // The ground is set once the art has loaded, before it fades in.
        let weak = Rc::downgrade(&view);
        view.art.connect_paintable_notify(move |art| {
            let Some(view) = weak.upgrade() else { return };
            let Some(texture) = art.paintable().and_then(|p| p.downcast::<gdk::Texture>().ok())
            else {
                return;
            };
            view.set_ground(luminance(&texture) > BRIGHT);
            motion::fade_in(&view.art);
        });
        view
    }

    /// Where the column goes: `COLUMN` wide or the window less its sides, centred, and its top
    /// where the start first placed it for this size. It grows downward from there.
    fn place(&self, overlay: &gtk::Overlay) -> gdk::Rectangle {
        let (w, h) = (overlay.width(), overlay.height());
        let width = COLUMN.min(w - 2 * SIDE).max(1);
        let top = match self.anchor.get() {
            Some((aw, ah, top)) if (aw, ah) == (w, h) => top,
            _ => {
                let (_, natural, _, _) = self.column.measure(gtk::Orientation::Vertical, width);
                let open = if self.details.is_visible() {
                    self.details.measure(gtk::Orientation::Vertical, width).1
                } else {
                    0
                };
                let top = ((h - (natural - open)) / 2).max(TOP);
                if w > 0 && h > 0 {
                    self.anchor.set(Some((w, h, top)));
                }
                top
            }
        };
        let (least, _, _, _) = self.column.measure(gtk::Orientation::Vertical, width);
        gdk::Rectangle::new((w - width) / 2, top, width, (h - top - BOTTOM).max(least))
    }

    pub fn connect_action(&self, on: OnAction) {
        self.on_action.replace(Some(on));
    }

    pub fn connect_translation(&self, on: Rc<dyn Fn(Translation)>) {
        self.on_translation.replace(Some(on));
    }

    fn pressed(&self, action: Action) {
        let on = self.on_action.borrow().clone();
        if let Some(on) = on {
            on(action);
        }
    }

    /// The ground over the art: 55%, or 80% over bright art.
    pub fn set_ground(&self, bright: bool) {
        if bright {
            self.ground.add_css_class("bright");
        } else {
            self.ground.remove_css_class("bright");
        }
    }

    /// A new start: the show and episode, and the room made ready for every line and button it
    /// can show. `resumes` keeps the room of Play from the beginning.
    pub fn begin(&self, show: &str, episode_line: &str, demo: bool, resumes: bool) {
        self.title.set_label(show);
        self.episode.set_label(episode_line);
        self.credit.set_visible(demo);
        self.first.set(true);
        self.detail_fade.set(0.0);
        self.detail_on.set(false);
        self.step.reset("");
        self.detail.reset("");
        self.loader.shown.set(true);
        self.loader.fade.set(1.0);
        self.loader.percent.set(None);
        self.loader.mix.set(0.0);
        self.details_open.set(false);
        self.details_fade.set(0.0);
        self.details.set_visible(false);
        self.show_rows(Rows::Waiting, true);
        for (action, slot) in &self.slots {
            slot.show(false, true);
            if *action == Action::FromBeginning {
                slot.button.set_visible(resumes);
            }
        }
        // The kept room: the waiting row as wide as it can get.
        while let Some(child) = self.room.first_child() {
            self.room.remove(&child);
        }
        for (action, slot) in &self.slots {
            if slot.button.is_visible() {
                let label = if *action == Action::Skip {
                    skip_room()
                } else {
                    slot.label.label().into()
                };
                let look = match action {
                    Action::Cancel | Action::Details => Look::Quiet,
                    _ => Look::Tonal,
                };
                self.room.append(&start_button(*action, &label, look).0);
            }
        }
        self.failing_row.borrow_mut().clear();
        self.anchor.set(None);
        self.column.queue_resize();
        self.set_art_pending();
    }

    pub fn set_episode_line(&self, line: &str) {
        self.episode.set_label(line);
    }

    pub fn set_show(&self, show: &str) {
        self.title.set_label(show);
    }

    /// The art is on its way: shown when it arrives, with the ground its brightness asks for.
    pub fn set_art_pending(&self) {
        self.art.set_paintable(gtk::gdk::Paintable::NONE);
        self.art.set_opacity(0.0);
        self.set_ground(false);
    }

    /// The row on show: the others fade away and stop taking clicks and focus.
    fn show_rows(&self, which: Rows, immediately: bool) {
        if self.rows_on.replace(which) == which && !immediately {
            return;
        }
        let rows = [
            (Rows::Waiting, &self.waiting),
            (Rows::Offline, &self.offline),
            (Rows::Failing, &self.failing),
        ];
        for ((kind, row), fade) in rows.into_iter().zip(&self.row_fades) {
            let on = kind == which;
            row.set_can_target(on);
            row.set_can_focus(on);
            let to = if on { 1.0 } else { 0.0 };
            if immediately {
                fade.set(to);
            } else {
                fade.to(to, STANDARD);
            }
        }
    }

    /// Draw `frame`.
    pub fn show(self: &Rc<Self>, frame: &Frame, details_open: bool) {
        let failed = frame.failure.is_some();
        let step = match &frame.failure {
            Some((title, _)) => title.as_str(),
            None => frame.step.as_str(),
        };
        if self.first.get() {
            self.step.reset(step);
        } else {
            self.step.set(step, failed, false);
        }
        let detail = match &frame.failure {
            Some((_, reason)) => reason.clone(),
            None => frame.detail(),
        };
        self.detail.set(&detail, false, failed);
        if !detail.is_empty() && !self.detail_on.replace(true) {
            self.detail_fade.to(1.0, FADE_IN);
        }
        self.loader.shown.set(!failed);
        self.loader.percent.set(frame.percent);
        self.live_update(step, &detail);

        let immediately = self.first.get();
        if failed {
            self.build_failing(&frame.buttons);
            self.show_rows(Rows::Failing, immediately);
        } else if frame.buttons.iter().any(|b| b.action == Action::ShowDownloads) {
            self.build_offline(&frame.buttons);
            self.show_rows(Rows::Offline, immediately);
        } else {
            self.show_rows(Rows::Waiting, immediately);
            for (action, slot) in &self.slots {
                let button = frame.buttons.iter().find(|b| b.action == *action);
                if let Some(button) = button
                    && slot.label.label() != button.label
                {
                    slot.label.set_label(&button.label);
                }
                let on = button.is_some_and(|b| b.shown);
                if !on && slot.present.get() && slot.button.has_focus() {
                    // Focus moves on to the next button on show before this one's slot hides.
                    let next = frame.buttons.iter().filter(|b| b.shown).find_map(|b| {
                        let after = self.slots.iter().position(|(a, _)| a == action)?;
                        let (_, s) =
                            self.slots.iter().skip(after + 1).find(|(a, _)| *a == b.action)?;
                        Some(s.button.clone())
                    });
                    let next = next.or_else(|| {
                        self.slots
                            .iter()
                            .find(|(a, _)| *a == Action::Cancel)
                            .map(|(_, s)| s.button.clone())
                    });
                    if let Some(next) = next {
                        next.grab_focus();
                    }
                }
                slot.show(on, immediately);
                if let Some(chevron) = &slot.chevron {
                    chevron.set_icon_name(Some(if details_open {
                        "pan-up-symbolic"
                    } else {
                        "pan-down-symbolic"
                    }));
                }
            }
        }
        self.first.set(false);

        // Details: rows when there are any, and the translation switch once it is known.
        let can_open = frame.buttons.iter().any(|b| b.action == Action::Details);
        self.open_details(details_open && can_open);
        self.fill_rows(&frame.rows);
        match frame.translation {
            Some(translation) => {
                self.quiet.set(true);
                self.language.set_visible(true);
                self.sub.set_active(translation == Translation::Sub);
                self.dub.set_active(translation == Translation::Dub);
                self.quiet.set(false);
            }
            None => self.language.set_visible(false),
        }
    }

    /// Details opens under the row and closes again, fading; the column above stays put.
    fn open_details(&self, open: bool) {
        if self.details_open.replace(open) == open {
            return;
        }
        if open {
            self.details.set_visible(true);
            self.details_fade.to(1.0, STANDARD);
        } else {
            let details = self.details.clone();
            self.details_fade.to_then(
                0.0,
                STANDARD,
                Some(Box::new(move || details.set_visible(false))),
            );
        }
    }

    fn build_offline(self: &Rc<Self>, buttons: &[Button]) {
        if self.offline.first_child().is_some() {
            return;
        }
        for button in buttons {
            self.offline.append(&self.action_button(button));
        }
    }

    fn build_failing(self: &Rc<Self>, buttons: &[Button]) {
        let actions: Vec<Action> = buttons.iter().map(|b| b.action).collect();
        if *self.failing_row.borrow() == actions {
            return;
        }
        while let Some(child) = self.failing.first_child() {
            self.failing.remove(&child);
        }
        for button in buttons {
            self.failing.append(&self.action_button(button));
        }
        self.failing_row.replace(actions);
    }

    fn action_button(self: &Rc<Self>, button: &Button) -> gtk::Button {
        let (b, _, _) = start_button(button.action, &button.label, button.look);
        let action = button.action;
        let weak = Rc::downgrade(self);
        b.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.pressed(action);
            }
        });
        b
    }

    /// The Details rows: built again when what they say changes, their times updated in place.
    fn fill_rows(self: &Rc<Self>, rows: &[Row]) {
        let same = {
            let shown = self.shown_rows.borrow();
            shown.len() == rows.len()
                && shown.iter().zip(rows).all(|(a, b)| {
                    (a.mark, &a.name, &a.note, &a.skip) == (b.mark, &b.name, &b.note, &b.skip)
                })
        };
        if same {
            for (label, row) in self.row_times.borrow().iter().zip(rows) {
                if label.label() != row.time {
                    label.set_label(&row.time);
                }
            }
            self.shown_rows.replace(rows.to_vec());
            return;
        }
        while let Some(child) = self.rows_box.first_child() {
            self.rows_box.remove(&child);
        }
        // What was there before: a row or a row's Skip that is new fades in, the rest stays.
        let before: Vec<(String, bool)> = self
            .shown_rows
            .borrow()
            .iter()
            .map(|r| (r.name.clone(), r.skip.is_some()))
            .collect();
        let mut arriving = Vec::new();
        let mut times = Vec::new();
        for row in rows {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            line.add_css_class("start-row");
            let icon = gtk::Image::from_icon_name(match row.mark {
                Mark::Done => "object-select-symbolic",
                Mark::Empty | Mark::Rest => "list-remove-symbolic",
                Mark::Now => "media-record-symbolic",
            });
            icon.set_pixel_size(12);
            icon.add_css_class("haze");
            let name = gtk::Label::new(Some(&row.name));
            name.set_xalign(0.0);
            if row.mark != Mark::Now {
                name.add_css_class("haze");
            }
            let note = gtk::Label::new(None);
            note.add_css_class("haze");
            note.set_xalign(0.0);
            note.set_hexpand(true);
            note.set_ellipsize(gtk::pango::EllipsizeMode::End);
            if !row.note.is_empty() {
                note.set_label(&format!("· {}", row.note)); // i18n-skip
            }
            let time = gtk::Label::new(Some(&row.time));
            time.add_css_class("haze");
            time.add_css_class("numeric");
            line.append(&icon);
            line.append(&name);
            line.append(&note);
            line.append(&time);
            if let Some(label) = &row.skip {
                let skip = gtk::Button::with_label(label);
                skip.add_css_class("flat");
                skip.add_css_class("start-row-skip");
                let weak = Rc::downgrade(self);
                skip.connect_clicked(move |_| {
                    if let Some(view) = weak.upgrade() {
                        view.pressed(Action::Skip);
                    }
                });
                line.append(&skip);
                if before.iter().any(|(name, skip)| *name == row.name && !skip) {
                    arriving.push(skip.upcast::<gtk::Widget>());
                }
            }
            if !before.iter().any(|(name, _)| *name == row.name) {
                arriving.push(line.clone().upcast());
            }
            self.rows_box.append(&line);
            times.push(time);
        }
        for widget in arriving {
            let fade = Fader::new(&widget);
            fade.set(0.0);
            fade.to(1.0, FADE_IN);
        }
        self.row_times.replace(times);
        self.shown_rows.replace(rows.to_vec());
    }

    /// Keep the lines readable as one element; the moments the design names are announced by
    /// the player.
    fn live_update(&self, step: &str, detail: &str) {
        let text = if detail.is_empty() {
            step.to_owned()
        } else {
            t::player_start_status(step, detail)
        };
        self.step.root.update_property(&[gtk::accessible::Property::Label(&text)]);
    }

    pub fn focus_primary(&self) {
        let mut child = self.failing.first_child();
        while let Some(widget) = child {
            if widget.has_css_class("suggested-action") {
                widget.grab_focus();
                return;
            }
            child = widget.next_sibling();
        }
        if let Some(first) = self.failing.first_child() {
            first.grab_focus();
        }
    }
}

/// The mean relative luminance of the art's centre band: the middle 60% of the width and half
/// of the height, sampled from about 64 columns. Linear sRGB, as the board measured it.
pub fn luminance(texture: &gdk::Texture) -> f64 {
    let (w, h) = (texture.width() as usize, texture.height() as usize);
    if w == 0 || h == 0 {
        return 0.0;
    }
    let mut downloader = gdk::TextureDownloader::new(texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    let data: &[u8] = &bytes;
    let step = (w / 64).max(1);
    let (x0, x1) = (w * 2 / 10, w * 8 / 10);
    let (y0, y1) = (h / 4, h * 3 / 4);
    let linear = |v: u8| {
        let c = f64::from(v) / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let (mut sum, mut n) = (0.0, 0u32);
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            let i = y * stride + x * 4;
            if let Some(px) = data.get(i..i + 3) {
                sum += 0.2126 * linear(px[0]) + 0.7152 * linear(px[1]) + 0.0722 * linear(px[2]);
                n += 1;
            }
            x += step;
        }
        y += step;
    }
    if n == 0 { 0.0 } else { sum / f64::from(n) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bright_field_is_over_the_threshold_and_a_dark_one_is_not() {
        let texture = |value: u8| {
            let pixels = vec![value; 64 * 64 * 4];
            gdk::MemoryTexture::new(
                64,
                64,
                gdk::MemoryFormat::R8g8b8a8,
                &glib::Bytes::from_owned(pixels),
                64 * 4,
            )
            .upcast::<gdk::Texture>()
        };
        assert!(luminance(&texture(240)) > BRIGHT);
        assert!(luminance(&texture(20)) < BRIGHT);
    }

    #[test]
    fn the_breathing_runs_four_seconds_from_full_to_forty_percent_and_back() {
        assert!((breath(0.0) - 1.0).abs() < 1e-9);
        assert!((breath(2.0) - 0.4).abs() < 1e-9);
        assert!((breath(4.0) - 1.0).abs() < 1e-9);
        assert!(breath(1.0) > 0.4 && breath(1.0) < 1.0);
    }

    #[test]
    fn numbers_change_in_place_and_words_crossfade() {
        assert_eq!(fade_key("Buffering · 25%"), fade_key("Buffering · 40%"));
        assert_eq!(fade_key("1 of 3 sources · 4 s"), fade_key("1 of 3 sources · 5 s"));
        assert_ne!(fade_key("Asking Alpha"), fade_key("Asking Brook"));
    }
}
