//! The starting screen's widgets: the art under its ground, the show, the loader line, the step
//! and detail lines, the buttons by time, the Details list and the failure. It draws the
//! [`Frame`] the model (`start_model.rs`) gives and says what the viewer pressed.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::{gdk, glib};
use kaeri_core::media::Translation;
use kaeri_i18n::t;

use super::start_model::{Action, Button, Frame, Look, Mark, Row};
use crate::motion;

/// Art brighter than this, by the mean relative luminance of its centre band, sits under the
/// deeper ground.
pub const BRIGHT: f64 = 0.15;

/// Text that changes in place: the new text fades in over the old on the standard token, where
/// the old one is, and the room is the larger of the two.
struct Crossfade {
    stack: gtk::Stack,
    pages: [Page; 2],
    on: Cell<usize>,
    key: Cell<&'static str>,
}

struct Page {
    row: gtk::Box,
    dot: gtk::Box,
    label: gtk::Label,
}

impl Page {
    fn new(css: &[&str]) -> Self {
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("start-dot");
        dot.set_visible(false);
        dot.set_valign(gtk::Align::Center);
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_hexpand(true);
        for class in css {
            label.add_css_class(class);
        }
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&dot);
        row.append(&label);
        Self { row, dot, label }
    }
}

impl Crossfade {
    fn new(css: &[&str]) -> Self {
        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(300);
        stack.set_hhomogeneous(false);
        stack.set_vhomogeneous(false);
        stack.set_halign(gtk::Align::Fill);
        let pages = [Page::new(css), Page::new(css)];
        stack.add_named(&pages[0].row, Some("a"));
        stack.add_named(&pages[1].row, Some("b"));
        Self { stack, pages, on: Cell::new(0), key: Cell::new("") }
    }

    /// Show `text`: crossfaded when it is another step, in place when it is the same one.
    fn set(&self, text: &str, key: &'static str, dot: bool) {
        let now = &self.pages[self.on.get()];
        if now.label.label() == text && now.dot.is_visible() == dot {
            return;
        }
        if self.key.get() == key && now.dot.is_visible() == dot {
            now.label.set_label(text);
            return;
        }
        let next = 1 - self.on.get();
        let page = &self.pages[next];
        page.label.set_label(text);
        page.dot.set_visible(dot);
        self.stack.set_visible_child_name(if next == 0 { "a" } else { "b" });
        self.on.set(next);
        self.key.set(key);
    }
}

/// The 2 pt loader line: silk at 14%, and an amber stretch 32% wide that moves across and back,
/// or, with animations off, rests at the start and breathes between 100% and 40%.
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
            let amber = (f64::from(amber.red()), f64::from(amber.green()), f64::from(amber.blue()));
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
            let reduced = reduced_motion();
            // The stretch: moving, or resting and breathing under reduced motion.
            let (x, alpha) = if reduced {
                (0.0, 0.7 + 0.3 * (drawn.get() * std::f64::consts::TAU / 2.0).cos())
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
        area.add_tick_callback(move |area, clock| {
            let Some(loader) = weak.upgrade() else { return glib::ControlFlow::Break };
            let now = clock.frame_time();
            let dt = last.replace(Some(now)).map_or(0.0, |was| (now - was) as f64 / 1e6);
            phase.set(phase.get() + dt);
            // The fill follows each percentage and the determinate look fades in over 0.2 s.
            let target = loader.percent.get().map_or(0.0, |p| f64::from(p) / 100.0);
            if reduced_motion() {
                loader.fill.set(target);
            } else {
                loader.fill.set(loader.fill.get() + (target - loader.fill.get()) * (dt * 8.0).min(1.0));
            }
            let want = if loader.percent.get().is_some() { 1.0 } else { 0.0 };
            let step = dt / 0.2;
            let mix = loader.mix.get();
            loader.mix.set(if want > mix { (mix + step).min(want) } else { (mix - step).max(want) });
            let fade_to = if loader.shown.get() { 1.0 } else { 0.0 };
            let fade = loader.fade.get();
            let rate = dt / 0.3;
            loader.fade.set(if fade_to > fade { (fade + rate).min(fade_to) } else { (fade - rate).max(fade_to) });
            area.queue_draw();
            glib::ControlFlow::Continue
        });
        loader
    }
}

/// Whether the system asks for animations to be off.
fn reduced_motion() -> bool {
    !gtk::Settings::default().is_some_and(|s| s.is_gtk_enable_animations())
}

/// One button of the waiting row: it fades in at the trailing end, and out when it stops
/// applying, and keeps its room either way.
struct Slot {
    button: gtk::Button,
    label: gtk::Label,
    chevron: Option<gtk::Image>,
    present: Cell<bool>,
    fade: RefCell<Option<adw::TimedAnimation>>,
}

impl Slot {
    fn new(action: Action) -> Self {
        let label = gtk::Label::new(None);
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        inner.append(&label);
        let chevron = (action == Action::Details).then(|| {
            let image = gtk::Image::from_icon_name("pan-down-symbolic");
            inner.append(&image);
            image
        });
        let button = gtk::Button::new();
        button.set_child(Some(&inner));
        button.set_valign(gtk::Align::Center);
        button.add_css_class("start-button");
        if matches!(action, Action::Cancel | Action::Details) {
            button.add_css_class("flat");
        }
        button.set_opacity(0.0);
        button.set_can_target(false);
        button.set_focusable(false);
        Self { button, label, chevron, present: Cell::new(false), fade: RefCell::default() }
    }

    fn show(&self, on: bool, immediately: bool) {
        if self.present.replace(on) == on {
            return;
        }
        self.button.set_can_target(on);
        self.button.set_focusable(on);
        if let Some(fade) = self.fade.take() {
            fade.skip();
        }
        let from = self.button.opacity();
        let (to, ms) = if on { (1.0, 200) } else { (0.0, 300) };
        if immediately {
            self.button.set_opacity(to);
            return;
        }
        let animation =
            adw::TimedAnimation::new(&self.button, from, to, ms, motion::opacity(&self.button));
        animation.set_easing(if on { adw::Easing::EaseOutCubic } else { adw::Easing::EaseInCubic });
        animation.play();
        self.fade.replace(Some(animation));
    }
}

/// A row of buttons that wraps at the largest text sizes.
fn wrap() -> adw::WrapBox {
    let row = adw::WrapBox::new();
    row.set_child_spacing(8);
    row.set_line_spacing(8);
    row.set_align(0.0);
    row.add_css_class("start-actions");
    row
}

/// What the view tells the player.
pub type OnAction = Rc<dyn Fn(Action)>;

type OnTranslation = Rc<dyn Fn(Translation)>;

pub struct StartView {
    pub root: gtk::Box,
    pub art: gtk::Picture,
    ground: gtk::Box,
    title: gtk::Label,
    episode: gtk::Label,
    credit: gtk::Label,
    loader: Rc<Loader>,
    step: Crossfade,
    detail: Crossfade,
    detail_on: Cell<bool>,
    switcher: gtk::Stack,
    slots: Vec<(Action, Slot)>,
    failing: adw::WrapBox,
    failing_row: RefCell<Vec<Action>>,
    reveal: gtk::Revealer,
    rows_box: gtk::Box,
    shown_rows: RefCell<Vec<Row>>,
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
        let art = gtk::Picture::builder().accessible_role(gtk::AccessibleRole::Presentation).build();
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
        title.set_accessible_role(gtk::AccessibleRole::Heading);
        let episode = gtk::Label::new(None);
        episode.add_css_class("haze");
        episode.add_css_class("start-episode");
        episode.set_xalign(0.0);
        episode.set_wrap(true);
        let credit = gtk::Label::new(Some(&t::player_demo_credit()));
        credit.add_css_class("haze");
        credit.add_css_class("caption");
        credit.set_wrap(true);
        credit.set_xalign(0.0);
        credit.set_visible(false);

        let loader = Loader::new();
        let step = Crossfade::new(&["start-step"]);
        let detail = Crossfade::new(&["haze", "start-detail"]);
        step.stack.set_margin_top(12);
        detail.stack.set_margin_top(3);
        detail.stack.set_opacity(0.0);
        detail.stack.set_height_request(20);

        // The waiting row keeps room for every button it will have; the failure row is built
        // for the failure; the two crossfade.
        let waiting = wrap();
        let slots: Vec<(Action, Slot)> = [
            Action::Cancel,
            Action::FromBeginning,
            Action::ChooseSource,
            Action::Details,
            Action::Skip,
            Action::ShowDownloads,
        ]
        .into_iter()
        .map(|a| (a, Slot::new(a)))
        .collect();
        for (_, slot) in &slots {
            waiting.append(&slot.button);
        }
        let failing = wrap();
        let switcher = gtk::Stack::new();
        switcher.set_transition_type(gtk::StackTransitionType::Crossfade);
        switcher.set_transition_duration(300);
        switcher.set_hhomogeneous(false);
        switcher.add_named(&waiting, Some("waiting"));
        switcher.add_named(&failing, Some("failing"));
        switcher.set_halign(gtk::Align::Start);

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
        let reveal = gtk::Revealer::new();
        reveal.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        reveal.set_transition_duration(300);
        reveal.set_child(Some(&list));

        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.add_css_class("start-column");
        column.append(&title);
        column.append(&episode);
        column.append(&credit);
        column.append(&loader.root);
        column.append(&step.stack);
        column.append(&detail.stack);
        column.append(&switcher);
        column.append(&reveal);
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(520);
        clamp.set_tightening_threshold(520);
        clamp.set_child(Some(&column));
        clamp.set_halign(gtk::Align::Center);
        clamp.set_valign(gtk::Align::Center);
        clamp.set_margin_start(24);
        clamp.set_margin_end(24);
        clamp.set_margin_top(48);
        clamp.set_margin_bottom(24);

        let stack = gtk::Overlay::new();
        stack.set_child(Some(&art));
        stack.add_overlay(&ground);
        stack.add_overlay(&clamp);
        stack.set_vexpand(true);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("player-starting");
        root.append(&stack);

        let view = Rc::new(Self {
            root,
            art,
            ground,
            title,
            episode,
            credit,
            loader,
            step,
            detail,
            detail_on: Cell::new(false),
            switcher,
            slots,
            failing,
            failing_row: RefCell::default(),
            reveal,
            rows_box,
            shown_rows: RefCell::default(),
            language,
            sub,
            dub,
            quiet: Cell::new(false),
            on_action: RefCell::default(),
            on_translation: RefCell::default(),
            first: Cell::new(true),
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
        for (toggle, translation) in [(&view.sub, Translation::Sub), (&view.dub, Translation::Dub)] {
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

    /// A new start: the show and episode, and the room made ready for the first lines.
    pub fn begin(&self, show: &str, episode_line: &str, demo: bool) {
        self.title.set_label(show);
        self.episode.set_label(episode_line);
        self.credit.set_visible(demo);
        self.first.set(true);
        self.detail.stack.set_opacity(0.0);
        self.detail_on.set(false);
        self.loader.shown.set(true);
        self.loader.fade.set(1.0);
        self.loader.percent.set(None);
        self.loader.mix.set(0.0);
        self.reveal.set_reveal_child(false);
        self.switcher.set_visible_child_name("waiting");
        for (_, slot) in &self.slots {
            slot.show(false, true);
        }
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

    /// Draw `frame`. `immediately` skips the fades, for a screenshot of a held state or the
    /// first frame of a screen that is itself fading in.
    pub fn show(self: &Rc<Self>, frame: &Frame, details_open: bool) {
        let failed = frame.failure.is_some();
        let (step, key) = match &frame.failure {
            Some((title, _)) => (title.as_str(), "failure"),
            None => (frame.step.as_str(), frame.key),
        };
        self.step.set(step, key, failed);
        let detail = match &frame.failure {
            Some((_, reason)) => reason.clone(),
            None => frame.detail(),
        };
        // The seconds change the line in place; the facts crossfade.
        let detail_key: &'static str = if failed { "failure" } else { leak_key(&frame.lead) };
        self.detail.set(&detail, detail_key, false);
        let wanted = !detail.is_empty();
        if wanted && !self.detail_on.replace(true) {
            motion::fade_in(&self.detail.stack);
        }
        self.loader.shown.set(!failed);
        self.loader.percent.set(frame.percent);
        self.live_update(&frame.step, &detail);

        if failed {
            self.build_failing(&frame.buttons);
            self.switcher.set_visible_child_name("failing");
        } else {
            self.switcher.set_visible_child_name("waiting");
            // Offline has its own two buttons, and the others have no room kept there.
            let offline = frame.buttons.iter().any(|b| b.action == Action::ShowDownloads);
            for (action, slot) in &self.slots {
                slot.button.set_visible(
                    (*action == Action::ShowDownloads) == offline
                        || matches!(action, Action::Cancel),
                );
                let button = frame.buttons.iter().find(|b| b.action == *action);
                if let Some(button) = button {
                    slot.label.set_label(&button.label);
                }
                slot.show(button.is_some(), self.first.get());
                if let (Some(chevron), true) = (&slot.chevron, true) {
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
        self.reveal.set_reveal_child(details_open && can_open);
        if *self.shown_rows.borrow() != frame.rows {
            self.fill_rows(&frame.rows);
            self.shown_rows.replace(frame.rows.clone());
        }
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

    fn build_failing(self: &Rc<Self>, buttons: &[Button]) {
        let actions: Vec<Action> = buttons.iter().map(|b| b.action).collect();
        if *self.failing_row.borrow() == actions {
            return;
        }
        while let Some(child) = self.failing.first_child() {
            self.failing.remove(&child);
        }
        for button in buttons {
            let b = gtk::Button::with_label(&button.label);
            b.add_css_class("start-button");
            match button.look {
                Look::Primary => b.add_css_class("suggested-action"),
                Look::Quiet => b.add_css_class("flat"),
                Look::Tonal => {}
            }
            let action = button.action;
            let weak = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                if let Some(view) = weak.upgrade() {
                    view.pressed(action);
                }
            });
            self.failing.append(&b);
        }
        self.failing_row.replace(actions);
    }

    fn fill_rows(self: &Rc<Self>, rows: &[Row]) {
        while let Some(child) = self.rows_box.first_child() {
            self.rows_box.remove(&child);
        }
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
            note.set_wrap(true);
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
            if row.skip {
                let skip = gtk::Button::with_label(&t::linux_start_row_skip());
                skip.add_css_class("flat");
                skip.add_css_class("start-row-skip");
                let weak = Rc::downgrade(self);
                skip.connect_clicked(move |_| {
                    if let Some(view) = weak.upgrade() {
                        view.pressed(Action::Skip);
                    }
                });
                line.append(&skip);
            }
            self.rows_box.append(&line);
        }
    }

    /// Tell a screen reader the moments the design names, and keep the lines readable as one.
    fn live_update(&self, step: &str, detail: &str) {
        let text = if detail.is_empty() {
            step.to_owned()
        } else {
            t::linux_start_status(step, detail)
        };
        self.step.stack.update_property(&[gtk::accessible::Property::Label(&text)]);
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

/// A key for the detail line's lead: the text itself, kept for the life of the program, so a
/// changed lead crossfades and an unchanged one (only the seconds moving) changes in place.
fn leak_key(lead: &str) -> &'static str {
    thread_local! {
        static KEYS: RefCell<std::collections::HashMap<String, &'static str>> = RefCell::default();
    }
    KEYS.with(|keys| {
        let mut keys = keys.borrow_mut();
        if let Some(key) = keys.get(lead) {
            return *key;
        }
        let key: &'static str = Box::leak(lead.to_owned().into_boxed_str());
        keys.insert(lead.to_owned(), key);
        key
    })
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
}
