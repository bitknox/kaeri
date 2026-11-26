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
//! The room listens to the engine's `Room*` events ([`Room::heard`]) and answers its buttons with
//! `Command::Room`; what only the player can do (play a source, open Sources) goes to the player.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_model::{
    command::Command,
    event::Event,
    room::{RoomCommand, RoomGlimpse, RoomLeftWhy, RoomMember, RoomMoveOffer},
};

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
#[cfg(feature = "fixtures")]
pub mod scenes;
pub mod strip;
pub mod words;

use banner::{Banner, Panel};
use column::{Act, Column};
use model::{ConvLine, RoomView};
use strip::Strip;
use words::Moves;

/// The column's contents start fading in this long after the column starts to come in.
const CONTENT_DELAY: Duration = Duration::from_millis(120);

/// Where the room's commands go.
type Sender = Rc<dyn Fn(Command)>;

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
    /// The engine's room, its members, an invite's look before joining, and a move offer.
    engine_view: RefCell<Option<kaeri_model::room::RoomView>>,
    members: RefCell<Vec<RoomMember>>,
    glimpse: RefCell<Option<(String, RoomGlimpse)>>,
    offer: RefCell<Option<RoomMoveOffer>>,
    /// Your sources for the room's episode, for the Choose a source banner.
    sources: RefCell<Vec<(String, String, bool)>>,
    /// How many sources looked for the episode, for No source.
    source_count: Cell<usize>,
    send: RefCell<Option<Sender>>,
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
            engine_view: RefCell::default(),
            members: RefCell::default(),
            glimpse: RefCell::default(),
            offer: RefCell::default(),
            sources: RefCell::default(),
            source_count: Cell::new(0),
            send: RefCell::default(),
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

    /// What the player does with the buttons only it can answer: a source to play, Sources,
    /// Search.
    pub fn on_act(&self, f: impl Fn(Act) + 'static) {
        self.act.replace(Some(Rc::new(f)));
    }

    /// Where the room's commands go: the engine.
    pub fn set_sender(&self, send: impl Fn(Command) + 'static) {
        self.send.replace(Some(Rc::new(send)));
    }

    fn send(&self, command: RoomCommand) {
        let f = self.send.borrow().clone();
        if let Some(f) = f {
            f(Command::Room(command));
        }
    }

    /// Your sources for the room's episode, as the Choose a source banner lists them, and how
    /// many sources there are.
    pub fn set_sources(&self, sources: Vec<(String, String, bool)>, count: usize) {
        self.sources.replace(sources);
        self.source_count.set(count);
    }

    fn handle(self: &Rc<Self>, a: Act) {
        match &a {
            Act::Leave => {
                let me = Rc::downgrade(self);
                dialogs::leave(&self.widget, false, move |answer| {
                    if answer == dialogs::LeaveAnswer::Leave
                        && let Some(me) = me.upgrade()
                    {
                        me.send(RoomCommand::Leave);
                    }
                });
                return;
            }
            Act::MoveRoom => {
                self.ask_move();
                return;
            }
            Act::Move(take) => self.send(RoomCommand::MoveRoom { take: take.clone() }),
            Act::CopyLink => {
                if let Some(link) = self.view.borrow().as_ref().and_then(|v| v.link.clone()) {
                    self.widget.display().clipboard().set_text(&link);
                }
            }
            Act::CarryOn => self.send(RoomCommand::CarryOn),
            Act::DontWait => self.send(RoomCommand::DontWait),
            Act::PlayForEveryone => self.send(RoomCommand::Play { play: true }),
            Act::NewInvite => self.send(RoomCommand::NewInvite),
            Act::SaveRelays(relays) => {
                self.send(RoomCommand::EditRelays { relays: relays.clone() });
            }
            Act::Say(text) => self.send(RoomCommand::Say { text: text.clone() }),
            Act::Join => {
                let glimpse = self.glimpse.borrow().clone();
                if let Some((invite, glimpse)) = glimpse {
                    self.send(RoomCommand::Lobby {
                        invite: Some(invite),
                        name: None,
                        pick: None,
                        anyway: glimpse.full,
                    });
                }
            }
            Act::NotNow => {
                if self.glimpse.borrow_mut().take().is_some() {
                    if self.engine_view.borrow().is_none() {
                        self.clear();
                    } else {
                        self.redraw();
                    }
                    return;
                }
                self.send(RoomCommand::ChoosingSource { open: false });
            }
            Act::GoAlong | Act::Stay => {
                let offer = self.offer.borrow_mut().take();
                if let Some(offer) = offer {
                    self.send(RoomCommand::AnswerMove { offer: offer.id, go: a == Act::GoAlong });
                }
                self.redraw();
            }
            Act::PlayInRoom(_) | Act::OpenSources | Act::SearchTitles => {}
        }
        self.pass(a);
    }

    /// What the engine says about the room.
    pub fn heard(self: &Rc<Self>, event: &Event) {
        match event {
            Event::RoomJoined(view) | Event::RoomState(view) => {
                if matches!(event, Event::RoomJoined(_)) {
                    self.glimpse.replace(None);
                    self.members.replace(Vec::new());
                }
                self.engine_view.replace(Some(view.clone()));
                self.redraw();
            }
            Event::RoomMembers(members) => {
                self.members.replace(members.clone());
                self.redraw();
            }
            Event::RoomChat(conversation) => {
                self.show_conversation(&model::ConvLine::from_engine(conversation));
            }
            Event::RoomGlimpse(glimpse) => {
                let mut held = self.glimpse.borrow_mut();
                // Only the invite this device opened shows as one.
                if let Some((_, g)) = held.as_mut() {
                    *g = glimpse.clone();
                    drop(held);
                    self.redraw();
                }
            }
            Event::RoomMoveOffer(offer) => {
                self.offer.replace(Some(offer.clone()));
                self.redraw();
            }
            Event::RoomMoveOfferGone { id } => {
                let gone = self.offer.borrow().as_ref().is_some_and(|o| &o.id == id);
                if gone {
                    self.offer.replace(None);
                    self.redraw();
                }
            }
            Event::RoomLeft { why } => {
                self.engine_view.replace(None);
                self.members.replace(Vec::new());
                self.offer.replace(None);
                if !matches!(why, RoomLeftWhy::Left | RoomLeftWhy::Moved) {
                    self.glimpse.replace(None);
                }
                if self.glimpse.borrow().is_some() {
                    self.redraw();
                } else {
                    self.clear();
                }
            }
            _ => {}
        }
    }

    /// An invite opened here: the room shows as it is, with Join the room and Not now.
    pub fn peek(&self, invite: &str) {
        self.glimpse.replace(Some((
            invite.to_owned(),
            RoomGlimpse {
                room_id: None,
                by: None,
                members: Vec::new(),
                room_episode: None,
                playing: false,
                full: false,
            },
        )));
        self.send(RoomCommand::PeekInvite { invite: invite.to_owned() });
    }

    /// Draw the room again from what the engine said.
    fn redraw(self: &Rc<Self>) {
        let glimpse = self.glimpse.borrow().clone();
        let view = match (&glimpse, self.engine_view.borrow().as_ref()) {
            (Some((_, g)), _) if g.room_episode.is_some() || !g.members.is_empty() => {
                RoomView::from_glimpse(g)
            }
            (_, Some(v)) => RoomView::from_engine(v, &self.members.borrow()),
            _ => return,
        };
        let mut moves = Moves::default();
        if let Some(offer) = self.offer.borrow().as_ref() {
            moves.offer_by = Some(offer.by.clone());
            moves.offer_others = offer.names.clone();
        }
        if let Some(moved) = self.engine_view.borrow().as_ref().and_then(|v| v.moved.clone()) {
            moves.moved_by = Some(moved.by);
            moves.went = moved.went;
        }
        self.show(view, moves);
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
            view.position_at(self.now()),
            move |take| {
                if let Some(me) = me.upgrade() {
                    me.handle(Act::Move(take));
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
        let panel = Panel::for_view(&view, &self.sources.borrow(), self.source_count.get(), now);
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
        self.banner.show(&RoomView::default(), None);
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
                || v.starts_at_ms.is_some()
                || v.playing
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
                    let panel = Panel::for_view(
                        &view,
                        &me.sources.borrow(),
                        me.source_count.get(),
                        now,
                    );
                    me.banner.show(&view, panel.as_ref());
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
