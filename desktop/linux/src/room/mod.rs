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
pub mod upnext;
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
    pub up_next: upnext::UpNext,
    /// The header bar's toggle that shows the column.
    pub toggle: gtk::ToggleButton,
    view: RefCell<Option<RoomView>>,
    moves: RefCell<Moves>,
    /// The engine's room, its members, an invite's look before joining, and a move offer.
    engine_view: RefCell<Option<kaeri_model::room::RoomView>>,
    members: RefCell<Vec<RoomMember>>,
    glimpse: RefCell<Option<(String, RoomGlimpse)>>,
    /// Whether the invite's look has come back; until then there is nothing to show of it.
    glimpse_heard: Cell<bool>,
    /// Join the room was pressed on the invite shown, and the room isn't joined yet.
    joining: Cell<bool>,
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
    /// Bumped by every change of full screen, the morph and leaving: a timer or animation
    /// started for an earlier one finds it changed and does nothing.
    turn: Cell<u64>,
    /// The column's fade for full screen, stopped by the next turn.
    column_fade: RefCell<Option<adw::SpringAnimation>>,
    /// Hide chat in rooms: no conversation, no field and no reaction ticks.
    chat_hidden: Cell<bool>,
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
        let up_next = upnext::UpNext::new();
        stage.add_overlay(&up_next.widget);
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
            up_next,
            toggle,
            view: RefCell::default(),
            moves: RefCell::default(),
            engine_view: RefCell::default(),
            members: RefCell::default(),
            glimpse: RefCell::default(),
            glimpse_heard: Cell::new(false),
            joining: Cell::new(false),
            offer: RefCell::default(),
            sources: RefCell::default(),
            source_count: Cell::new(0),
            send: RefCell::default(),
            clock: RefCell::new(Rc::new(wall_ms)),
            tick: RefCell::default(),
            fullscreen: Cell::new(false),
            turn: Cell::new(0),
            column_fade: RefCell::default(),
            chat_hidden: Cell::new(false),
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
        // Your name's "you" and your chat lines carry the haze in markup: drawn again when
        // High Contrast changes.
        let weak = Rc::downgrade(&room);
        crate::style::on_contrast_change(&room.widget, move || {
            if let Some(room) = weak.upgrade() {
                room.column.redraw_conversation();
                room.redraw();
            }
        });
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
                    self.joining.set(true);
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
                    self.after_invite();
                    return;
                }
                self.send(RoomCommand::ChoosingSource { open: false });
            }
            Act::GoAlong | Act::Stay => {
                let offer = self.offer.borrow_mut().take();
                if let Some(offer) = offer {
                    self.send(RoomCommand::AnswerMove {
                        offer: offer.id,
                        go: a == Act::GoAlong,
                    });
                }
                self.redraw();
            }
            Act::PlayInRoom(_)
            | Act::OpenSources
            | Act::SearchTitles
            | Act::InviteGone
            | Act::Notice(_) => {}
        }
        self.pass(a);
    }

    /// What the engine says about the room.
    pub fn heard(self: &Rc<Self>, event: &Event) {
        match event {
            Event::RoomJoined(view) | Event::RoomState(view) => {
                if matches!(event, Event::RoomJoined(_)) {
                    self.joining.set(false);
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
                // Only the look this device waits on shows: a slow answer for another invite,
                // or a look into a recent room (the place's), never stands in for it.
                if let Some((_, g)) = held
                    .as_mut()
                    .filter(|(invite, _)| glimpse.invite.as_deref() == Some(invite.as_str()))
                {
                    *g = glimpse.clone();
                    drop(held);
                    self.glimpse_heard.set(true);
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
            Event::RoomLookFailed { invite: Some(invite), reason, why, .. } => {
                let waited =
                    self.glimpse.borrow().as_ref().is_some_and(|(held, _)| held == invite);
                if !waited {
                    return;
                }
                // The look found nothing: the invite goes, and the room you are in stays.
                self.glimpse.replace(None);
                if matches!(why, None | Some(RoomLeftWhy::Unreachable)) {
                    self.ask_again(invite.clone());
                } else {
                    self.pass(Act::Notice(crate::say::msg(reason)));
                    self.after_invite();
                }
            }
            Event::RoomLeft { why } => {
                // Joining the invite shown here failed (no room was joined yet): its relays
                // didn't answer, or the player says why.
                if self.engine_view.borrow().is_none() && self.joining.take() {
                    let invite = self.glimpse.take().map(|(invite, _)| invite);
                    self.glimpse_heard.set(false);
                    match (why, invite) {
                        (RoomLeftWhy::Unreachable, Some(invite)) => self.ask_again(invite),
                        _ => self.after_invite(),
                    }
                    return;
                }
                // Otherwise it is the room you are in that went; an invite you look at stays.
                self.engine_view.replace(None);
                self.members.replace(Vec::new());
                self.offer.replace(None);
                if self.glimpse.borrow().is_some() && self.glimpse_heard.get() {
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
        self.glimpse_heard.set(false);
        self.joining.set(false);
        self.glimpse.replace(Some((
            invite.to_owned(),
            RoomGlimpse {
                room_id: None,
                by: None,
                members: Vec::new(),
                room_episode: None,
                playing: false,
                full: false,
                invite: Some(invite.to_owned()),
            },
        )));
        self.send(RoomCommand::PeekInvite { invite: invite.to_owned() });
    }

    /// An invite whose relays didn't answer: an alert over where you were. Try again looks
    /// again; Close lets the invite go.
    fn ask_again(self: &Rc<Self>, invite: String) {
        let me = Rc::downgrade(self);
        dialogs::unreachable(&self.widget, move |again| {
            let Some(me) = me.upgrade() else { return };
            if again {
                me.peek(&invite);
            } else {
                me.after_invite();
            }
        });
    }

    /// An invite shown here went (Not now, or Back before joining): the room you are in shows
    /// again, or, in none, the player has nothing to show for it.
    fn after_invite(self: &Rc<Self>) {
        self.glimpse_heard.set(false);
        self.joining.set(false);
        if self.engine_view.borrow().is_none() {
            self.clear();
            self.pass(Act::InviteGone);
        } else {
            self.redraw();
        }
    }

    /// Back before joining an invite: it goes, as Not now.
    pub fn drop_invite(self: &Rc<Self>) {
        if self.glimpse.borrow_mut().take().is_some() {
            self.glimpse_heard.set(false);
            self.joining.set(false);
            if self.engine_view.borrow().is_none() {
                self.clear();
            } else {
                self.redraw();
            }
        }
    }

    /// Whether an invite opened here is showing, or being looked at, over whatever else.
    pub fn looking(&self) -> bool {
        self.glimpse.borrow().is_some()
    }

    /// Whether this device is in a room the engine joined, rather than looking at an invite.
    pub fn joined(&self) -> bool {
        self.engine_view.borrow().is_some()
    }

    /// Draw the room again from what the engine said.
    fn redraw(self: &Rc<Self>) {
        let glimpse = self.glimpse.borrow().clone();
        let view = match (&glimpse, self.engine_view.borrow().as_ref()) {
            // Once the look came back, even an empty room shows, with Join the room.
            (Some((_, g)), _) if self.glimpse_heard.get() => RoomView::from_glimpse(g),
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
        let panel =
            Panel::for_view(&view, &self.sources.borrow(), self.source_count.get(), now);
        self.banner.show(&view, panel.as_ref());
        self.up_next.show(Some(&view), now);
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
        self.next_turn();
        self.view.replace(None);
        self.column.widget.set_opacity(1.0);
        self.strip.reset();
        self.banner.show(&RoomView::default(), None);
        self.up_next.show(None, 0);
        self.column.show_conversation(&[]);
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
                    me.up_next.show(Some(&view), now);
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
            let me = Rc::downgrade(self);
            let turn = self.next_turn();
            glib::timeout_add_local_once(CONTENT_DELAY, move || {
                // A later turn (full screen, or the room gone) owns the column's opacity now.
                if me.upgrade().is_some_and(|me| me.turn.get() == turn) {
                    crate::motion::fade_in(&content);
                }
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
        let turn = self.next_turn();
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
            let me = Rc::downgrade(self);
            anim.connect_done(move |_| {
                if let Some(me) = me.upgrade().filter(|me| me.turn.get() == turn) {
                    me.split.set_show_sidebar(false);
                }
            });
            anim.play();
            self.column_fade.replace(Some(anim));
            let me = Rc::downgrade(self);
            glib::timeout_add_local_once(Duration::from_millis(500), move || {
                if let Some(me) = me.upgrade().filter(|me| me.turn.get() == turn) {
                    me.strip.set_on(true);
                }
            });
        } else {
            self.strip.set_on(false);
            column.set_opacity(1.0);
            if show {
                column.set_opacity(0.0);
                self.split.set_show_sidebar(true);
                let anim =
                    crate::motion::standard(&column, 0.0, 1.0, crate::motion::opacity(&column));
                anim.play();
                self.column_fade.replace(Some(anim));
            }
        }
    }

    /// A new turn of full screen or the morph: what an earlier one left running stands down.
    fn next_turn(&self) -> u64 {
        let turn = self.turn.get() + 1;
        self.turn.set(turn);
        if let Some(fade) = self.column_fade.take() {
            fade.skip();
        }
        turn
    }

    /// Hide chat in rooms, from the setting or the room's chat state.
    pub fn set_chat_hidden(&self, hidden: bool) {
        self.chat_hidden.set(hidden);
        self.column.set_chat_hidden(hidden);
    }

    pub fn chat_hidden(&self) -> bool {
        self.chat_hidden.get()
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

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use kaeri_model::{
        Msg,
        event::Event,
        room::{RoomGlimpse, RoomLeftWhy, RoomView as EngineView},
    };

    use gtk::prelude::*;

    use super::{Room, column::Act};
    use crate::gtk_test::{pump, run};

    /// A room in a window of its own, with the acts it passes to the player.
    fn room() -> (Rc<Room>, Rc<RefCell<Vec<Act>>>, gtk::Window) {
        let stage = gtk::Overlay::new();
        let room = Room::new(&gtk::Box::new(gtk::Orientation::Vertical, 0), &stage);
        let window = gtk::Window::new();
        window.set_child(Some(&room.widget));
        let passed: Rc<RefCell<Vec<Act>>> = Rc::default();
        room.on_act({
            let passed = Rc::clone(&passed);
            move |a| passed.borrow_mut().push(a)
        });
        (room, passed, window)
    }

    fn empty_look() -> RoomGlimpse {
        RoomGlimpse {
            room_id: None,
            by: None,
            members: Vec::new(),
            room_episode: None,
            playing: false,
            full: false,
            invite: Some(INVITE.into()),
        }
    }

    const INVITE: &str = "kaeri://room#k";

    fn joined() -> EngineView {
        EngineView { room_id: "r1".into(), ..EngineView::default() }
    }

    #[test]
    fn an_empty_room_s_invite_still_shows() {
        run(|| {
            let (room, _, _window) = room();
            room.peek(INVITE);
            assert!(!room.in_room(), "nothing shows before the look comes back");
            room.heard(&Event::RoomGlimpse(empty_look()));
            assert!(room.in_room(), "the empty room shows, with Join the room");
        });
    }

    #[test]
    fn not_now_on_an_invite_alone_lets_the_player_go() {
        run(|| {
            let (room, passed, _window) = room();
            room.peek(INVITE);
            room.heard(&Event::RoomGlimpse(empty_look()));
            room.handle(Act::NotNow);
            assert!(!room.in_room());
            assert_eq!(*passed.borrow(), vec![Act::InviteGone]);
        });
    }

    #[test]
    fn back_on_an_invite_leaves_no_room_behind() {
        run(|| {
            let (room, _, _window) = room();
            room.peek(INVITE);
            room.heard(&Event::RoomGlimpse(empty_look()));
            assert!(!room.joined());
            room.drop_invite();
            assert!(!room.in_room());
        });
    }

    fn look_failed(invite: &str) -> Event {
        Event::RoomLookFailed {
            room_id: String::new(),
            reason: Msg::RoomLookUnreachable,
            invite: Some(invite.into()),
            why: Some(RoomLeftWhy::Unreachable),
        }
    }

    #[test]
    fn a_failed_look_at_an_invite_keeps_the_room_you_are_in() {
        run(|| {
            let (room, passed, _window) = room();
            room.heard(&Event::RoomJoined(joined()));
            room.peek(INVITE);
            room.heard(&look_failed(INVITE));
            assert!(room.joined() && room.in_room());
            // The invite went: its glimpse, coming late, shows nothing.
            room.heard(&Event::RoomGlimpse(empty_look()));
            assert!(room.glimpse.borrow().is_none());
            assert!(passed.borrow().is_empty());
        });
    }

    #[test]
    fn only_the_look_waited_on_answers() {
        run(|| {
            let (room, _, _window) = room();
            room.peek("kaeri://room#first");
            room.peek(INVITE);
            // The first invite's answers come late: neither stands in for this one.
            let mut first = empty_look();
            first.invite = Some("kaeri://room#first".into());
            room.heard(&Event::RoomGlimpse(first));
            room.heard(&look_failed("kaeri://room#first"));
            assert!(!room.in_room() && room.glimpse.borrow().is_some());
            // A recent room's look is the place's.
            let mut recent = empty_look();
            recent.invite = None;
            recent.room_id = Some("r9".into());
            room.heard(&Event::RoomGlimpse(recent));
            assert!(!room.in_room());
            room.heard(&Event::RoomGlimpse(empty_look()));
            assert!(room.in_room());
        });
    }

    #[test]
    fn leaving_your_room_keeps_the_invite_you_look_at() {
        run(|| {
            let (room, _, _window) = room();
            room.heard(&Event::RoomJoined(joined()));
            room.peek(INVITE);
            room.heard(&Event::RoomGlimpse(empty_look()));
            room.heard(&Event::RoomLeft { why: RoomLeftWhy::Unreachable });
            assert!(!room.joined());
            assert!(room.in_room(), "the invite still shows, with Join the room");
        });
    }

    #[test]
    fn a_join_that_fails_lets_the_invite_go() {
        run(|| {
            let (room, passed, _window) = room();
            room.peek(INVITE);
            room.heard(&Event::RoomGlimpse(empty_look()));
            room.handle(Act::Join);
            room.heard(&Event::RoomLeft { why: RoomLeftWhy::Full });
            assert!(!room.in_room() && room.glimpse.borrow().is_none());
            assert_eq!(*passed.borrow(), vec![Act::Join, Act::InviteGone]);
        });
    }

    #[test]
    fn back_on_an_invite_over_your_room_shows_your_room_again() {
        run(|| {
            let (room, _, _window) = room();
            room.heard(&Event::RoomJoined(joined()));
            room.peek(INVITE);
            room.heard(&Event::RoomGlimpse(empty_look()));
            assert!(room.looking() && room.joined());
            room.drop_invite();
            assert!(!room.looking() && room.joined() && room.in_room());
        });
    }

    #[test]
    fn full_screen_right_after_the_room_comes_in_keeps_the_column_faded() {
        run(|| {
            let (room, _, _window) = room();
            room.heard(&Event::RoomJoined(joined()));
            room.set_fullscreen(true);
            pump(800);
            assert!(room.column.widget.opacity() < 0.5, "the morph's late step stood down");
        });
    }

    #[test]
    fn leaving_full_screen_at_once_leaves_no_strip_behind() {
        run(|| {
            let (room, _, _window) = room();
            room.heard(&Event::RoomJoined(joined()));
            room.set_fullscreen(true);
            room.set_fullscreen(false);
            pump(800);
            assert!(!room.strip.is_on());
            room.set_fullscreen(true);
            room.clear();
            pump(800);
            assert!(!room.strip.is_on());
            assert!(!room.split.shows_sidebar());
        });
    }
}
