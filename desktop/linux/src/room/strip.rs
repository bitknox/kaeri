//! The full screen strip: the faces and the room's line in an OSD capsule at the top trailing
//! corner, with Carry on when the wait reaches 20 s. It shows only while it matters: while
//! anyone isn't watching, and for 8 s after any change. Otherwise it leaves with the controls.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::glib;

use super::{
    column::Act,
    face::FaceView,
    model::RoomView,
    words::{self, LineButtons, Moves},
};

/// How long the strip stays after a change.
const AFTER_CHANGE: Duration = Duration::from_secs(8);
/// Faces before the "+N" face.
const FACES: usize = 4;

pub struct Strip {
    pub widget: gtk::Box,
    faces: gtk::Box,
    words: gtk::Label,
    carry: gtk::Button,
    fader: crate::motion::Fader,
    key: RefCell<String>,
    /// Shown for a change until this source fires.
    hold: RefCell<Option<glib::SourceId>>,
    holding: Cell<bool>,
    controls: Cell<bool>,
    must: Cell<bool>,
    on: Cell<bool>,
}

impl Strip {
    pub fn new(act: impl Fn(Act) + 'static) -> Rc<Self> {
        let faces = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let words = gtk::Label::new(None);
        words.add_css_class("room-strip-words");
        words.add_css_class("numeric");
        words.set_margin_start(4);
        words.set_margin_end(6);
        let carry = gtk::Button::new();
        carry.add_css_class("pill");
        carry.add_css_class("suggested-action");
        carry.add_css_class("room-strip-carry");
        carry.set_visible(false);
        carry.connect_clicked(move |_| act(Act::CarryOn));
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        widget.add_css_class("osd");
        widget.add_css_class("room-strip");
        widget.set_halign(gtk::Align::End);
        widget.set_valign(gtk::Align::Start);
        widget.set_margin_top(16);
        widget.set_margin_end(16);
        widget.append(&faces);
        widget.append(&words);
        widget.append(&carry);
        widget.set_visible(false);
        Rc::new(Self {
            widget,
            faces,
            words,
            carry,
            fader: crate::motion::Fader::always(),
            key: RefCell::default(),
            hold: RefCell::default(),
            holding: Cell::new(false),
            controls: Cell::new(false),
            must: Cell::new(false),
            on: Cell::new(false),
        })
    }

    /// Full screen on or off: off, the strip never shows.
    pub fn set_on(self: &Rc<Self>, on: bool) {
        self.on.set(on);
        self.refresh();
    }

    /// The player's controls came up or left.
    pub fn set_controls(self: &Rc<Self>, up: bool) {
        self.controls.set(up);
        // Under the header bar while it shows, at the corner when it leaves.
        self.widget.set_margin_top(if up { 60 } else { 16 });
        self.refresh();
    }

    pub fn show(self: &Rc<Self>, view: &RoomView, moves: &Moves, now_ms: i64) {
        let line = words::line(view, moves, now_ms);
        let members = words::ordered(view);
        while let Some(c) = self.faces.first_child() {
            self.faces.remove(&c);
        }
        let capped = members.len() > FACES;
        for m in members.iter().take(if capped { FACES - 1 } else { FACES }) {
            let face = FaceView::new(24);
            let look = words::face(view, m, now_ms);
            face.show(&m.name, &look);
            face.widget.set_can_target(true);
            face.widget.set_tooltip_text(Some(&format!("{}, {}", m.name, look.word))); // i18n-skip
            face.widget.update_property(&[gtk::accessible::Property::Label(&format!(
                "{}, {}",
                m.name, look.word
            ))]); // i18n-skip
            self.faces.append(&face.widget);
        }
        if capped {
            let more = gtk::Label::new(Some(&format!("+{}", members.len() - (FACES - 1)))); // i18n-skip
            more.add_css_class("room-more-face");
            more.add_css_class("caption");
            self.faces.append(&more);
        }
        // The line, with the seconds while it waits and nobody has carried on yet.
        let secs = view.waiting.first().map(|w| words::secs_since(w.since_ms, now_ms));
        let text = match (&line.buttons, secs) {
            (LineButtons::None, Some(s)) if line.holds => format!(
                "{}  <span weight=\"normal\" foreground=\"#A3A9C4\">· {s}\u{a0}s</span>",
                glib::markup_escape_text(&line.title)
            ), // i18n-skip
            _ => glib::markup_escape_text(&line.title).to_string(),
        };
        self.words.set_markup(&text);
        match &line.buttons {
            LineButtons::CarryOn(label) => {
                self.carry.set_label(label);
                self.carry.set_visible(true);
            }
            _ => self.carry.set_visible(false),
        }
        self.must.set(words::anyone_not_watching(view) || line.holds);
        // A change of state (not of seconds) holds the strip for 8 s.
        let key = format!(
            "{}|{}",
            line.title,
            members.iter().map(|m| format!("{:?}", m.state)).collect::<Vec<_>>().join(",")
        );
        if *self.key.borrow() != key {
            let first = self.key.borrow().is_empty();
            self.key.replace(key);
            if !first {
                self.hold_for_change();
            }
        }
        self.refresh();
    }

    fn hold_for_change(self: &Rc<Self>) {
        if let Some(old) = self.hold.borrow_mut().take() {
            old.remove();
        }
        self.holding.set(true);
        let me = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(AFTER_CHANGE, move || {
            if let Some(me) = me.upgrade() {
                me.hold.borrow_mut().take();
                me.holding.set(false);
                me.refresh();
            }
        });
        self.hold.replace(Some(id));
    }

    fn refresh(&self) {
        let show =
            self.on.get() && (self.must.get() || self.holding.get() || self.controls.get());
        self.fader.set(&self.widget, show);
    }
}
