//! The room: watching together with the video, the people and the chat in one place (room
//! board, Linux frame). In the player, the room is a column beside the video in an
//! `AdwOverlaySplitView`, toggled from the player's header bar; full screen hides the column
//! and shows the strip; the video area carries the room's banner before play and while your own
//! player has nothing to show.
//!
//! Starting from the player morphs the player into the room: the window grows by the column's
//! width when the screen has room for it (otherwise the video gives way), the column slides in
//! beside the video, which doesn't move, and its contents fade in from 120 ms. With Reduce
//! Animation the room crossfades in over the player instead, so the picture never goes empty.
//!
//! Until the engine's room FFI lands, the room draws the stand-in records in [`model`].

// The stand-ins carry what the engine will send; the rest is wired with its FFI.
#![allow(dead_code)]

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::glib;

pub mod banner;
pub mod column;
pub mod copy;
pub mod dialogs;
pub mod face;
#[cfg(any(test, feature = "fixtures"))]
pub mod fixture;
pub mod model;
pub mod place;
pub mod popovers;
pub mod strip;
pub mod words;

use banner::{Banner, Panel};
use column::{Act, Column};
use model::{ConvLine, RoomView};
use strip::Strip;
use words::Moves;

/// The column's contents start fading in this long after the column starts to come in.
const CONTENT_DELAY: Duration = Duration::from_millis(120);

/// The room around the player's page.
pub struct Room {
    /// The player's widget in the window: the veil (for Reduce Animation's crossfade) over the
    /// split view of the page and the column.
    pub widget: gtk::Overlay,
    pub split: adw::OverlaySplitView,
    pub column: Rc<Column>,
    pub strip: Rc<Strip>,
    pub banner: Banner,
    /// The header bar's toggle that shows the column.
    pub toggle: gtk::ToggleButton,
    view: RefCell<Option<RoomView>>,
    moves: RefCell<Moves>,
    sources: RefCell<Vec<(String, String, bool)>>,
    /// This device's clock, in Unix epoch ms. Shots hold it still.
    clock: RefCell<Rc<dyn Fn() -> i64>>,
    tick: RefCell<Option<glib::SourceId>>,
    fullscreen: Cell<bool>,
    act: RefCell<Option<column::ActFn>>,
    /// The player's own watch-together button, away while in a room.
    aside: RefCell<Vec<gtk::Widget>>,
}

fn wall_ms() -> i64 {
    glib::real_time() / 1000
}

impl Room {
    /// The room around `page`, with the strip and the banner laid on `stage`.
    pub fn new(page: &impl IsA<gtk::Widget>, stage: &gtk::Overlay) -> Rc<Self> {
        let column = Column::new();
        let split = adw::OverlaySplitView::builder()
            .sidebar_position(gtk::PackType::End)
            .min_sidebar_width(f64::from(column::WIDTH))
            .max_sidebar_width(f64::from(column::WIDTH))
            .sidebar_width_unit(adw::LengthUnit::Px)
            .show_sidebar(false)
            .enable_show_gesture(false)
            .enable_hide_gesture(false)
            .build();
        split.add_css_class("room-split");
        split.set_content(Some(page));
        split.set_sidebar(Some(&column.widget));
        let widget = gtk::Overlay::new();
        widget.set_child(Some(&split));

        let toggle = gtk::ToggleButton::builder()
            .icon_name("sidebar-show-right-symbolic")
            .tooltip_text(copy::room_show_room())
            .valign(gtk::Align::Center)
            .visible(false)
            .build();
        toggle.add_css_class("flat");
        toggle.update_property(&[gtk::accessible::Property::Label(&copy::room_show_room())]);

        let act_cell: column::ActCell = Rc::default();
        let forward = {
            let act_cell = Rc::clone(&act_cell);
            move |a: Act| {
                let f = act_cell.borrow().clone();
                if let Some(f) = f {
                    f(a);
                }
            }
        };
        let forward = Rc::new(forward);
        let strip = Strip::new({
            let f = Rc::clone(&forward);
            move |a| f(a)
        });
        let banner = Banner::new({
            let f = Rc::clone(&forward);
            move |a| f(a)
        });
        stage.add_overlay(&banner.widget);
        stage.add_overlay(&strip.widget);
        column.on_act({
            let f = Rc::clone(&forward);
            move |a| f(a)
        });

        let room = Rc::new(Self {
            widget,
            split,
            column,
            strip,
            banner,
            toggle,
            view: RefCell::default(),
            moves: RefCell::default(),
            sources: RefCell::new(vec![
                ("Local files".into(), "1080p · 1.2 GB".into(), true), // i18n-skip
                ("night-index".into(), "720p · 640 MB".into(), false), // i18n-skip
            ]),
            clock: RefCell::new(Rc::new(wall_ms)),
            tick: RefCell::default(),
            fullscreen: Cell::new(false),
            act: RefCell::default(),
            aside: RefCell::default(),
        });
        // The toggle shows and hides the column; the column's own state follows back.
        room.toggle.connect_toggled({
            let room = Rc::downgrade(&room);
            move |t| {
                if let Some(room) = room.upgrade()
                    && room.split.shows_sidebar() != t.is_active()
                    && !room.fullscreen.get()
                {
                    room.split.set_show_sidebar(t.is_active());
                }
            }
        });
        // Acts the room answers itself; the rest go to the player.
        {
            let room_weak = Rc::downgrade(&room);
            act_cell.replace(Some(Rc::new(move |a: Act| {
                let Some(room) = room_weak.upgrade() else { return };
                room.handle(a);
            })));
        }
        room
    }

    /// `widget` goes while in a room: the toggle takes its place.
    pub fn set_aside(&self, widget: &impl IsA<gtk::Widget>) {
        self.aside.borrow_mut().push(widget.as_ref().clone());
    }

    /// What the player does with the room's buttons.
    pub fn on_act(&self, f: impl Fn(Act) + 'static) {
        self.act.replace(Some(Rc::new(f)));
    }

    fn handle(self: &Rc<Self>, a: Act) {
        match &a {
            Act::Leave => {
                let me = Rc::downgrade(self);
                dialogs::leave(&self.widget, false, move |answer| {
                    if answer == dialogs::LeaveAnswer::Leave
                        && let Some(me) = me.upgrade()
                    {
                        me.pass(Act::Leave);
                    }
                });
                return;
            }
            Act::MoveRoom => {
                self.ask_move();
                return;
            }
            Act::CopyLink => {
                if let Some(link) = self.view.borrow().as_ref().and_then(|v| v.link.clone()) {
                    self.widget.display().clipboard().set_text(&link);
                }
            }
            _ => {}
        }
        self.pass(a);
    }

    fn pass(&self, a: Act) {
        let f = self.act.borrow().clone();
        if let Some(f) = f {
            f(a);
        }
    }

    /// Move to a new room: the dialog that ticks who comes along.
    pub fn ask_move(self: &Rc<Self>) -> Option<adw::AlertDialog> {
        let view = self.view.borrow().clone()?;
        let me = Rc::downgrade(self);
        Some(dialogs::move_room(
            &self.widget,
            &view.name,
            &view.move_candidates,
            view.position,
            move |take| {
                if let Some(me) = me.upgrade() {
                    me.pass(Act::MoveRoom);
                    let _ = take;
                }
            },
        ))
    }

    /// Ask before closing the player: Leave, Keep the room or Cancel.
    pub fn ask_close(
        &self,
        answer: impl Fn(dialogs::LeaveAnswer) + 'static,
    ) -> adw::AlertDialog {
        dialogs::leave(&self.widget, true, answer)
    }

    /// Hold the room's clock at `now_ms`, for shots.
    pub fn hold_clock(&self, now_ms: i64) {
        self.clock.replace(Rc::new(move || now_ms));
    }

    fn now(&self) -> i64 {
        (self.clock.borrow())()
    }

    pub fn in_room(&self) -> bool {
        self.view.borrow().is_some()
    }

    /// The room as it stands. The first view of a room brings the column in, as the morph.
    pub fn show(self: &Rc<Self>, view: RoomView, moves: Moves) {
        let first = self.view.borrow().is_none();
        let now = self.now();
        self.column.show(&view, &moves, now);
        self.strip.show(&view, &moves, now);
        let panel = Panel::for_view(&view, &self.sources.borrow());
        self.banner.show(&view, panel.as_ref());
        self.toggle.set_visible(true);
        for w in self.aside.borrow().iter() {
            w.set_visible(false);
        }
        self.view.replace(Some(view));
        self.moves.replace(moves);
        if first {
            self.morph_in();
        }
        self.keep_ticking();
    }

    pub fn show_conversation(&self, lines: &[ConvLine]) {
        self.column.show_conversation(lines);
    }

    /// Out of the room: the column goes and the player is the player again.
    pub fn clear(&self) {
        self.view.replace(None);
        self.split.set_show_sidebar(false);
        self.toggle.set_visible(false);
        self.toggle.set_active(false);
        for w in self.aside.borrow().iter() {
            w.set_visible(true);
        }
        self.strip.set_on(false);
        if let Some(t) = self.tick.borrow_mut().take() {
            t.remove();
        }
    }

    /// The one-second tick that counts the seconds, while anything counts.
    fn keep_ticking(self: &Rc<Self>) {
        let counts = self.view.borrow().as_ref().is_some_and(|v| {
            !v.waiting.is_empty()
                || v.up_next.is_some()
                || v.members.iter().any(|m| m.since_ms.is_some())
        });
        let mut tick = self.tick.borrow_mut();
        if counts && tick.is_none() {
            let me = Rc::downgrade(self);
            *tick = Some(glib::timeout_add_local(Duration::from_secs(1), move || {
                let Some(me) = me.upgrade() else { return glib::ControlFlow::Break };
                let (view, moves) = (me.view.borrow().clone(), me.moves.borrow().clone());
                if let Some(view) = view {
                    let now = me.now();
                    me.column.show(&view, &moves, now);
                    me.strip.show(&view, &moves, now);
                }
                glib::ControlFlow::Continue
            }));
        } else if !counts && let Some(t) = tick.take() {
            t.remove();
        }
    }

    /// The player becomes the room: the window grows by the column's width when the screen
    /// has room, and the column comes in beside the video.
    fn morph_in(self: &Rc<Self>) {
        if self.fullscreen.get() {
            return;
        }
        let window = self.widget.root().and_downcast::<gtk::Window>();
        let grow = || {
            let Some(window) = &window else { return };
            if window.is_maximized() || window.is_fullscreen() {
                return;
            }
            let width = window.width();
            let fits = window
                .surface()
                .and_then(|s| gtk::prelude::WidgetExt::display(window).monitor_at_surface(&s))
                .is_some_and(|m| m.geometry().width() >= width + column::WIDTH);
            if fits {
                window.set_default_size(width + column::WIDTH, window.height());
            }
        };
        if crate::motion::reduced() {
            // The room crossfades in over the player; nothing slides or grows on screen.
            let split = self.split.clone();
            crate::motion::ghost(&self.widget, &self.split, true, move || {
                grow();
                split.set_show_sidebar(true);
            });
        } else {
            grow();
            let content = self.column.widget.clone();
            content.set_opacity(0.0);
            self.split.set_show_sidebar(true);
            glib::timeout_add_local_once(CONTENT_DELAY, move || {
                crate::motion::fade_in(&content);
            });
        }
        self.toggle.set_active(true);
    }

    /// Full screen on or off: the column leaves (24 px and a fade on standard) and the strip
    /// comes in; leaving reverses it. With Reduce Animation the two crossfade.
    pub fn set_fullscreen(self: &Rc<Self>, on: bool) {
        if self.fullscreen.replace(on) == on || !self.in_room() {
            self.strip.set_on(on && self.in_room());
            return;
        }
        let show = !on && self.toggle.is_active();
        if crate::motion::reduced() {
            let (split, strip) = (self.split.clone(), Rc::clone(&self.strip));
            crate::motion::ghost(&self.widget, &self.split, true, move || {
                split.set_show_sidebar(show);
                strip.set_on(on);
            });
            return;
        }
        let column = self.column.widget.clone();
        if on {
            let anim =
                crate::motion::standard(&column, 1.0, 0.0, crate::motion::opacity(&column));
            let split = self.split.clone();
            anim.connect_done(move |_| split.set_show_sidebar(false));
            anim.play();
            let strip = Rc::clone(&self.strip);
            glib::timeout_add_local_once(Duration::from_millis(500), move || {
                strip.set_on(true)
            });
        } else {
            self.strip.set_on(false);
            if show {
                column.set_opacity(0.0);
                self.split.set_show_sidebar(true);
                let anim =
                    crate::motion::standard(&column, 0.0, 1.0, crate::motion::opacity(&column));
                anim.play();
            }
        }
    }

    /// The player's controls came up or left: the strip follows when nothing holds it.
    pub fn set_controls(self: &Rc<Self>, up: bool) {
        self.strip.set_controls(up);
    }

    /// For shots: the popovers and dialogs by name.
    pub fn shot_open(self: &Rc<Self>, which: &str) {
        match which {
            "invite" | "details" | "edit-relays" => self.column.shot_popover(which),
            "move" => {
                self.ask_move();
            }
            "leave" => {
                dialogs::leave(&self.widget, false, |_| {});
            }
            "close" => {
                dialogs::leave(&self.widget, true, |_| {});
            }
            _ => {}
        }
    }
}
