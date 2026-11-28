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
    /// The faces shown, kept from tick to tick so a tooltip or focus on one stays.
    shown: RefCell<Vec<FaceView>>,
    more: gtk::Label,
    words: gtk::Label,
    /// "· 6 s" while the room waits, in the quieter colour.
    secs: gtk::Label,
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
        let secs = gtk::Label::new(None);
        secs.add_css_class("room-strip-words");
        secs.add_css_class("room-strip-secs");
        secs.add_css_class("numeric");
        secs.set_margin_end(6);
        secs.set_visible(false);
        let more = gtk::Label::new(None);
        more.add_css_class("room-more-face");
        more.add_css_class("caption");
        more.set_visible(false);
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
        widget.append(&more);
        widget.append(&words);
        widget.append(&secs);
        widget.append(&carry);
        widget.set_visible(false);
        Rc::new(Self {
            widget,
            faces,
            shown: RefCell::default(),
            more,
            words,
            secs,
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

    /// Out of the room: no hold left running, and the next room starts afresh.
    pub fn reset(&self) {
        if let Some(old) = self.hold.borrow_mut().take() {
            old.remove();
        }
        self.holding.set(false);
        self.key.replace(String::new());
        self.must.set(false);
        self.on.set(false);
        self.refresh();
    }

    pub fn show(self: &Rc<Self>, view: &RoomView, moves: &Moves, now_ms: i64) {
        let line = words::line(view, moves, now_ms);
        let members = words::ordered(view);
        let capped = members.len() > FACES;
        let count = members.len().min(if capped { FACES - 1 } else { FACES });
        // Only a change in how many faces show builds them again; a tick redraws them.
        if self.shown.borrow().len() != count {
            while let Some(c) = self.faces.first_child() {
                self.faces.remove(&c);
            }
            let made: Vec<FaceView> = (0..count)
                .map(|_| {
                    let face = FaceView::new(24);
                    face.widget.set_can_target(true);
                    face.widget.set_accessible_role(gtk::AccessibleRole::Img);
                    self.faces.append(&face.widget);
                    face
                })
                .collect();
            self.shown.replace(made);
        }
        for (face, m) in self.shown.borrow().iter().zip(&members) {
            let look = words::face(view, m, now_ms);
            face.show(&m.name, &look);
            let label = kaeri_i18n::t::room_face_label(&m.name, &look.word);
            if face.widget.tooltip_text().as_deref() != Some(label.as_str()) {
                face.widget.set_tooltip_text(Some(&label));
                face.widget.update_property(&[gtk::accessible::Property::Label(&label)]);
            }
        }
        self.more.set_visible(capped);
        if capped {
            let rest = (members.len() - (FACES - 1)) as i64;
            self.more.set_label(&kaeri_i18n::t::room_more_faces(rest));
            self.more.update_property(&[gtk::accessible::Property::Label(
                &kaeri_i18n::t::room_more_faces_label(rest),
            )]);
        }
        // The line, with the seconds while it waits and nobody has carried on yet.
        let secs = view.waiting.first().map(|w| words::secs_since(w.since_ms, now_ms));
        let (title, tail) = match (&line.buttons, secs) {
            (LineButtons::None, Some(s)) if line.holds => split_seconds(
                &line.title,
                &kaeri_i18n::t::room_line_with_seconds(&line.title, s),
            ),
            _ => (line.title.clone(), String::new()),
        };
        self.words.set_label(&title);
        self.secs.set_label(&tail);
        self.secs.set_visible(!tail.is_empty());
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

    /// Whether the strip is on: full screen in a room.
    #[cfg(test)]
    pub fn is_on(&self) -> bool {
        self.on.get()
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

/// The line and its seconds, apart: the seconds show in the quieter colour. A language that
/// puts the seconds first keeps the whole line in one.
fn split_seconds(title: &str, with: &str) -> (String, String) {
    match with.strip_prefix(title) {
        Some(tail) if !tail.trim().is_empty() => {
            (title.to_owned(), tail.trim_start().to_owned())
        }
        _ => (with.to_owned(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::split_seconds;

    #[test]
    fn the_seconds_come_apart_from_the_line() {
        assert_eq!(
            split_seconds("Waiting for Ren", "Waiting for Ren · 6 s"),
            ("Waiting for Ren".to_owned(), "· 6 s".to_owned())
        );
        assert_eq!(
            split_seconds("Wait", "6 s · Wait"),
            ("6 s · Wait".to_owned(), String::new())
        );
    }
}
