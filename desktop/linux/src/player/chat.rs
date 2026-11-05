//! Watch together chat over the video: the column of the newest lines at the top leading corner
//! (three at most, then a fold line that opens the conversation), the moment a reaction gives
//! bottom trailing, the four reaction buttons in the top bar, and the field C opens above the
//! controls. The conversation itself is the engine's, kept for the party only; this is how the
//! player shows it. Hide chat takes all of it away, and a server with chat off takes the field
//! and the reactions.
//!
//! Chat never takes the amber: Send is tonal, the glyphs are silk and the ticks are haze.
//! Fades run on their own clock rather than libadwaita's, so with Reduce Animation the rise and
//! the closing up of the column go and the fades stay.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    party::{
        PartyChatBody, PartyChatLine, PartyChatRefusal, PartyCommand, PartyReaction,
        PartyReactionMark,
    },
};

use crate::{
    party::{chat as words, glyph},
    window::Ctx,
};

/// Lines on the video at most, before the fold.
const COLUMN: usize = 3;
/// How long a reaction's moment holds.
const MOMENT: Duration = Duration::from_millis(1500);
/// The rise of a line or a moment, on the standard spring, in px.
const RISE: f64 = 6.0;
/// Where the column starts, from the leading edge and from the top.
const LEAD: i32 = 16;
const TOP: i32 = 62;

/// Whether the system asks for less motion.
fn reduced() -> bool {
    !gtk::Settings::default().is_some_and(|s| s.is_gtk_enable_animations())
}

/// Fade `widget` to `to` over `ms` on the frame clock, whatever the system's animation setting
/// says, then run `done`. A widget that is not on screen ends at once.
pub fn fade(
    widget: &impl IsA<gtk::Widget>,
    to: f64,
    ms: u32,
    out: bool,
    done: impl FnOnce() + 'static,
) {
    let widget = widget.as_ref().clone();
    if !widget.is_mapped() {
        widget.set_opacity(to);
        done();
        return;
    }
    let from = widget.opacity();
    let began = Cell::new(None::<i64>);
    let done = Cell::new(Some(done));
    widget.add_tick_callback(move |widget, clock| {
        let now = clock.frame_time();
        let start = match began.get() {
            Some(start) => start,
            None => {
                began.set(Some(now));
                now
            }
        };
        let t = ((now - start) as f64 / (f64::from(ms) * 1000.0)).clamp(0.0, 1.0);
        // Fade-in eases out, fade-out eases in, as the motion tokens say.
        let eased = if out { t * t * t } else { 1.0 - (1.0 - t).powi(3) };
        widget.set_opacity(from + (to - from) * eased);
        if t >= 1.0 {
            if let Some(done) = done.take() {
                done();
            }
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
}

/// Raise `line` from `RISE` px low to its place on the standard spring, unless motion is
/// reduced.
fn rise(line: &impl IsA<gtk::Widget>) {
    if reduced() {
        return;
    }
    let widget = line.as_ref().clone();
    widget.set_margin_top(RISE as i32);
    let target = adw::CallbackAnimationTarget::new({
        let widget = widget.clone();
        move |value| widget.set_margin_top(value.round() as i32)
    });
    let animation = crate::motion::standard(&widget, RISE, 0.0, target);
    animation.connect_done({
        let widget = widget.clone();
        move |_| widget.set_margin_top(0)
    });
    crate::motion::once_mapped(&widget, move || animation.play());
}

/// A line waiting for its turn on the video.
struct Pending {
    who: String,
    you: bool,
    text: String,
}

/// A line on the video.
struct Shown {
    revealer: gtk::Revealer,
}

pub struct Chat {
    ctx: Rc<Ctx>,
    me: RefCell<std::rc::Weak<Chat>>,
    /// The column: top leading, over the video.
    pub column: gtk::Box,
    shown: RefCell<VecDeque<Shown>>,
    waiting: RefCell<VecDeque<Pending>>,
    fold: gtk::Button,
    fold_label: gtk::Label,
    /// Reaction moments, bottom trailing.
    pub moments: gtk::Box,
    /// The field C opens, bottom leading above the controls.
    pub field: gtk::Box,
    entry: gtk::Entry,
    send: gtk::Button,
    tail: gtk::Label,
    /// The four reaction buttons, in the top bar before the party button.
    pub buttons: Vec<gtk::Button>,
    /// The corner cards' box, which a moment lifts.
    cards: gtk::Box,
    /// The newest line seen, so a snapshot shows only what came after.
    seen: Cell<u64>,
    /// The people popover is open: the same lines are in it, so the column stays shut.
    popover_open: Cell<bool>,
    returned: RefCell<Option<String>>,
    open_conversation: RefCell<Option<Rc<dyn Fn()>>>,
    marks: RefCell<Rc<dyn Fn(Vec<PartyReactionMark>)>>,
    held: Cell<bool>,
    told_left: Cell<bool>,
}

impl Chat {
    pub fn new(ctx: &Rc<Ctx>, cards: &gtk::Box) -> Rc<Self> {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        column.set_halign(gtk::Align::Start);
        column.set_valign(gtk::Align::Start);
        column.set_margin_start(LEAD);
        column.set_margin_top(TOP);
        column.set_can_target(false);
        column.set_accessible_role(gtk::AccessibleRole::Presentation);

        let fold_label = gtk::Label::new(None);
        let fold = gtk::Button::builder().child(&fold_label).build();
        fold.add_css_class("chat-line");
        fold.add_css_class("chat-fold");
        fold.set_halign(gtk::Align::Start);
        fold.set_visible(false);
        column.append(&fold);

        let moments = gtk::Box::new(gtk::Orientation::Vertical, 6);
        moments.set_halign(gtk::Align::End);
        moments.set_valign(gtk::Align::End);
        moments.set_margin_end(12);
        moments.set_margin_bottom(64);
        moments.set_can_target(false);
        moments.set_accessible_role(gtk::AccessibleRole::Presentation);

        let entry = gtk::Entry::builder().placeholder_text(t::party_chat_field()).build();
        entry.set_size_request(400, 34);
        entry.update_property(&[gtk::accessible::Property::Label(&t::party_chat_field())]);
        let tail = gtk::Label::new(None);
        tail.add_css_class("chat-tail");
        tail.add_css_class("numeric");
        tail.set_halign(gtk::Align::End);
        tail.set_valign(gtk::Align::Center);
        tail.set_margin_end(10);
        tail.set_can_target(false);
        tail.set_visible(false);
        let over = gtk::Overlay::new();
        over.set_child(Some(&entry));
        over.add_overlay(&tail);
        let send = gtk::Button::with_label(&t::party_chat_send());
        send.set_sensitive(false);
        let field = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        field.add_css_class("linked");
        field.add_css_class("chat-field");
        field.set_halign(gtk::Align::Start);
        field.set_valign(gtk::Align::End);
        field.set_margin_start(16);
        field.set_margin_bottom(76);
        field.append(&over);
        field.append(&send);
        field.set_visible(false);

        let buttons: Vec<gtk::Button> = glyph::ALL
            .iter()
            .map(|&reaction| {
                let button = gtk::Button::builder()
                    .child(&glyph::icon(reaction, 16, glyph::WEIGHT))
                    .tooltip_text(words::label(reaction))
                    .valign(gtk::Align::Center)
                    .build();
                button.add_css_class("flat");
                button.add_css_class("image-button");
                button.update_property(&[gtk::accessible::Property::Label(&words::label(
                    reaction,
                ))]);
                button.set_visible(false);
                button.connect_clicked({
                    let ctx = Rc::clone(ctx);
                    move |_| ctx.hub.send(Command::Party(PartyCommand::React { reaction }))
                });
                button
            })
            .collect();

        let chat = Rc::new(Self {
            ctx: Rc::clone(ctx),
            me: RefCell::default(),
            column,
            shown: RefCell::default(),
            waiting: RefCell::default(),
            fold,
            fold_label,
            moments,
            field,
            entry: entry.clone(),
            send: send.clone(),
            tail,
            buttons,
            cards: cards.clone(),
            seen: Cell::new(0),
            popover_open: Cell::new(false),
            returned: RefCell::default(),
            open_conversation: RefCell::default(),
            marks: RefCell::new(Rc::new(|_| {})),
            held: Cell::new(false),
            told_left: Cell::new(false),
        });
        chat.me.replace(Rc::downgrade(&chat));
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let chat = Rc::downgrade(&chat);
            move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape
                    && let Some(chat) = chat.upgrade()
                {
                    chat.entry.set_text("");
                    chat.close_field();
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
        });
        entry.add_controller(keys);
        chat.fold.connect_clicked({
            let chat = Rc::downgrade(&chat);
            move |_| {
                if let Some(chat) = chat.upgrade()
                    && let Some(open) = chat.open_conversation.borrow().clone()
                {
                    open();
                }
            }
        });
        entry.connect_activate({
            let chat = Rc::downgrade(&chat);
            move |_| {
                if let Some(chat) = chat.upgrade() {
                    chat.submit();
                }
            }
        });
        send.connect_clicked({
            let chat = Rc::downgrade(&chat);
            move |_| {
                if let Some(chat) = chat.upgrade() {
                    chat.submit();
                }
            }
        });
        entry.connect_changed({
            let chat = Rc::downgrade(&chat);
            move |_| {
                if let Some(chat) = chat.upgrade() {
                    chat.typed();
                }
            }
        });
        ctx.prefs.watch_hide_chat({
            let chat = Rc::downgrade(&chat);
            move |_| {
                if let Some(chat) = chat.upgrade() {
                    chat.reflect();
                }
            }
        });
        ctx.on_size({
            let chat = Rc::downgrade(&chat);
            move |_| {
                if let Some(chat) = chat.upgrade() {
                    chat.reflect();
                }
            }
        });
        chat
    }

    /// What the window does for a click on the fold: opens the conversation.
    pub fn on_open_conversation(&self, open: impl Fn() + 'static) {
        self.open_conversation.replace(Some(Rc::new(open)));
    }

    /// Where the seek bar's ticks go: called with the reactions' marks whenever they change, and
    /// with none when chat is hidden.
    pub fn on_marks(&self, marks: impl Fn(Vec<PartyReactionMark>) + 'static) {
        self.marks.replace(Rc::new(marks));
    }

    /// The people popover opened or closed.
    pub fn popover(&self, open: bool) {
        self.popover_open.set(open);
        self.column.set_visible(!open);
        // The popover has its own field; what is typed here stays for the next C.
        if open {
            self.close_field();
        }
        if !open {
            crate::motion::fade_in(&self.column);
        }
    }

    fn hidden(&self) -> bool {
        self.ctx.prefs.hide_chat()
    }

    fn allowed(&self) -> bool {
        self.ctx.hub.known.borrow().party_chat_state.as_ref().is_none_or(|s| s.allowed)
    }

    fn in_party(&self) -> bool {
        self.ctx.hub.known.borrow().party.is_some()
    }

    /// The buttons, the field and everything on the video as the party, the server and Hide
    /// chat now allow.
    pub fn reflect(&self) {
        let hidden = self.hidden();
        let live = self.in_party() && !hidden && self.allowed();
        let roomy = self.ctx.size() != crate::window::Size::Narrow;
        for button in &self.buttons {
            button.set_visible(live && roomy);
        }
        if hidden || !self.in_party() {
            self.clear();
            self.close_field();
            (self.marks.borrow())(Vec::new());
        } else if !live {
            self.close_field();
        }
        if !hidden {
            let marks = self.ctx.hub.known.borrow().party_chat.marks.clone();
            (self.marks.borrow())(marks);
        }
    }

    /// Everything on the video goes at once.
    fn clear(&self) {
        while let Some(shown) = self.shown.borrow_mut().pop_front() {
            self.column.remove(&shown.revealer);
        }
        self.waiting.borrow_mut().clear();
        self.fold.set_visible(false);
        while let Some(moment) = self.moments.first_child() {
            self.moments.remove(&moment);
        }
        self.lift();
    }

    /// What the engine said.
    pub fn heard(&self, event: &Event, on_screen: bool) {
        match event {
            Event::PartyJoined(_) => {
                self.seen.set(0);
                self.reflect();
            }
            Event::PartyLeft { .. } => {
                self.seen.set(0);
                self.returned.replace(None);
                self.reflect();
            }
            Event::PartyChat(conversation) => {
                let (fresh, newest) = words::fresh(&conversation.lines, self.seen.get());
                self.seen.set(newest);
                let hidden = self.hidden();
                if !hidden && on_screen && !self.popover_open.get() {
                    for line in fresh {
                        self.arrive(line);
                    }
                }
                if !hidden {
                    (self.marks.borrow())(conversation.marks.clone());
                }
                // A send that never came back: the text is back in the field, once.
                match &conversation.failed {
                    Some(text) => {
                        if self.returned.borrow().as_deref() != Some(text.as_str()) {
                            if self.field.is_visible() && self.entry.text().is_empty() {
                                self.entry.set_text(text);
                                self.entry.set_position(-1);
                            }
                            self.returned.replace(Some(text.clone()));
                        }
                    }
                    None => {
                        self.returned.replace(None);
                    }
                }
            }
            Event::PartyChatState(state) => {
                self.reflect();
                self.entry.set_max_length(state.max_length.try_into().unwrap_or(150));
                self.cooling(state.react_wait_seconds > 0);
                self.typed();
            }
            Event::PartyReaction(reaction) => {
                if on_screen && !self.hidden() {
                    self.moment(reaction);
                }
            }
            Event::PartyChatRefused(PartyChatRefusal::Off) => {
                self.ctx.toast(&t::party_chat_off());
            }
            _ => {}
        }
    }

    /// The four buttons dim while the next reaction waits.
    fn cooling(&self, cooling: bool) {
        for button in &self.buttons {
            button.set_sensitive(!cooling);
            if cooling {
                button.add_css_class("cooling");
            } else {
                button.remove_css_class("cooling");
            }
        }
    }

    // The column.

    fn arrive(&self, line: &PartyChatLine) {
        let PartyChatBody::Said { text } = &line.body else { return };
        let who = words::who(line);
        let room = self.shown.borrow().len() < COLUMN && self.waiting.borrow().is_empty();
        let pending = Pending { who, you: line.you, text: text.clone() };
        if room {
            self.show(pending);
        } else {
            self.waiting.borrow_mut().push_back(pending);
            self.update_fold();
        }
    }

    fn update_fold(&self) {
        let count = self.waiting.borrow().len();
        self.fold.set_visible(count > 0);
        if count > 0 {
            self.fold_label.set_label(&t::party_chat_more(count as i64));
            self.fold.update_property(&[gtk::accessible::Property::Label(
                &t::party_chat_more(count as i64),
            )]);
        }
    }

    fn line_widget(who: &str, you: bool, text: &str) -> gtk::Box {
        let label = gtk::Label::new(None);
        label.set_markup(&words::said_markup(who, you, text));
        label.set_wrap(true);
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_lines(3);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_xalign(0.0);
        label.set_max_width_chars(46);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        line.add_css_class("chat-line");
        line.set_halign(gtk::Align::Start);
        line.append(&label);
        line.set_accessible_role(gtk::AccessibleRole::Presentation);
        line
    }

    fn show(&self, pending: Pending) {
        let line = Self::line_widget(&pending.who, pending.you, &pending.text);
        let revealer = gtk::Revealer::new();
        revealer.set_transition_type(gtk::RevealerTransitionType::None);
        revealer.set_reveal_child(true);
        revealer.set_child(Some(&line));
        revealer.set_halign(gtk::Align::Start);
        // Before the fold, which stays last.
        self.column.insert_child_after(
            &revealer,
            self.column.last_child().and_then(|l| l.prev_sibling()).as_ref(),
        );
        self.shown.borrow_mut().push_back(Shown { revealer: revealer.clone() });
        self.update_fold();
        revealer.set_opacity(0.0);
        rise(&line);
        crate::motion::once_mapped(&revealer, {
            let revealer = revealer.clone();
            move || fade(&revealer, 1.0, 200, false, || {})
        });
        let me = self.me_weak();
        let hold = words::hold(&pending.text);
        let target = revealer.downgrade();
        glib::timeout_add_local_once(hold, move || {
            if let (Some(chat), Some(revealer)) = (me.upgrade(), target.upgrade()) {
                Self::leave(&chat, &revealer);
            }
        });
    }

    /// A line has held its time: it fades out, the column closes up, and the next waiting line
    /// takes its place.
    fn leave(chat: &Self, revealer: &gtk::Revealer) {
        let me = chat.me_weak();
        let revealer_c = revealer.clone();
        fade(revealer, 0.0, 300, true, move || {
            let Some(chat) = me.upgrade() else { return };
            let finish = {
                let me = chat.me_weak();
                let revealer = revealer_c.clone();
                move || {
                    let Some(chat) = me.upgrade() else { return };
                    chat.shown.borrow_mut().retain(|s| s.revealer != revealer);
                    if revealer.parent().is_some() {
                        chat.column.remove(&revealer);
                    }
                    chat.promote();
                }
            };
            if reduced() {
                // No sliding: the others fade in at their new places as the gap closes.
                finish();
                for shown in chat.shown.borrow().iter() {
                    shown.revealer.set_opacity(0.4);
                    fade(&shown.revealer, 1.0, 200, false, || {});
                }
            } else {
                // The column closes up on the standard curve.
                revealer_c.set_transition_type(gtk::RevealerTransitionType::SlideUp);
                revealer_c.set_transition_duration(300);
                let finish = Cell::new(Some(finish));
                revealer_c.connect_child_revealed_notify(move |r| {
                    if !r.is_child_revealed()
                        && let Some(finish) = finish.take()
                    {
                        finish();
                    }
                });
                revealer_c.set_reveal_child(false);
            }
        });
    }

    fn promote(&self) {
        while self.shown.borrow().len() < COLUMN {
            let Some(next) = self.waiting.borrow_mut().pop_front() else { break };
            self.show(next);
        }
        self.update_fold();
    }

    // Moments.

    fn moment(&self, reaction: &PartyReaction) {
        let pill = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        pill.add_css_class("chat-moment");
        pill.set_halign(gtk::Align::End);
        pill.append(&glyph::icon(reaction.reaction, 20, glyph::WEIGHT));
        let who = if reaction.you { t::party_chat_you() } else { reaction.from.clone() };
        let name = gtk::Label::new(Some(&who));
        pill.append(&name);
        // Not in the accessibility tree: the conversation has it as text.
        pill.set_accessible_role(gtk::AccessibleRole::Presentation);
        pill.update_state(&[gtk::accessible::State::Hidden(true)]);
        pill.set_opacity(0.0);
        self.moments.append(&pill);
        self.lift();
        rise(&pill);
        crate::motion::once_mapped(&pill, {
            let pill = pill.clone();
            move || fade(&pill, 1.0, 200, false, || {})
        });
        let me = self.me_weak();
        let target = pill.downgrade();
        glib::timeout_add_local_once(MOMENT, move || {
            let (Some(chat), Some(pill)) = (me.upgrade(), target.upgrade()) else { return };
            let leaving = Rc::clone(&chat);
            let gone = pill.clone();
            fade(&pill, 0.0, 300, true, move || {
                if gone.parent().is_some() {
                    leaving.moments.remove(&gone);
                }
                leaving.lift();
            });
        });
    }

    /// A moment stacks above the corner cards: they rise by its height while it is up.
    fn lift(&self) {
        let mut count = 0;
        let mut at = self.moments.first_child();
        while let Some(widget) = at {
            count += 1;
            at = widget.next_sibling();
        }
        self.cards.set_margin_bottom(96 + if count > 0 { count * 42 } else { 0 });
    }

    // The field.

    /// C: the field opens above the controls and takes focus, or the room's server says it has
    /// no chat. Whether the key was taken.
    pub fn open_field(&self) -> bool {
        if !self.in_party() || self.hidden() {
            return false;
        }
        if !self.allowed() {
            self.ctx.toast(&t::party_chat_off());
            return true;
        }
        if !self.field.is_visible() {
            self.field.set_visible(true);
            self.field.set_opacity(0.0);
            rise(&self.field);
            let field = self.field.clone();
            crate::motion::once_mapped(&self.field, move || {
                fade(&field, 1.0, 200, false, || {})
            });
        }
        let state = self.ctx.hub.known.borrow().party_chat_state.clone();
        if let Some(state) = state {
            self.entry.set_max_length(state.max_length.try_into().unwrap_or(150));
        }
        self.entry.grab_focus();
        self.typed();
        true
    }

    /// Escape, or the controls going with nothing typed: the field fades away.
    pub fn close_field(&self) {
        if !self.field.is_visible() {
            return;
        }
        self.held.set(false);
        let field = self.field.clone();
        fade(&self.field, 0.0, 300, true, move || {
            field.set_visible(false);
            field.set_opacity(1.0);
        });
    }

    /// Whether a toast's text is a chat line the column already shows (or hides on purpose).
    pub fn is_echo(&self, text: &str) -> bool {
        let known = self.ctx.hub.known.borrow();
        known.party_chat.lines.iter().rev().take(4).any(|line| match &line.body {
            PartyChatBody::Said { text: said } => text.contains(said.as_str()),
            _ => false,
        })
    }

    /// Whether the field holds the controls up: it has focus, or has something typed in it.
    pub fn holds_controls(&self) -> bool {
        self.field.is_visible() && (self.entry.has_focus() || !self.entry.text().is_empty())
    }

    /// Type into the field, for a shot.
    #[cfg(feature = "fixtures")]
    pub fn shot_type(&self, text: &str) {
        self.open_field();
        self.entry.set_text(text);
        self.entry.set_position(-1);
    }

    /// Send what is typed, for a shot.
    #[cfg(feature = "fixtures")]
    pub fn shot_send(&self) {
        self.submit();
    }

    /// Open the people popover's chat, for a shot of the states it says instead of a list.
    #[cfg(feature = "fixtures")]
    pub fn shot_pop(&self) {
        if let Some(open) = self.open_conversation.borrow().clone() {
            open();
        }
    }

    fn submit(&self) {
        let text = self.entry.text().trim().to_owned();
        let wait =
            self.ctx.hub.known.borrow().party_chat_state.as_ref().map_or(0, |s| s.wait_seconds);
        if text.is_empty() || wait > 0 {
            return;
        }
        words::say(&self.ctx, &text);
        self.entry.set_text("");
    }

    fn typed(&self) {
        let state = self.ctx.hub.known.borrow().party_chat_state.clone();
        let typed = self.entry.text().trim().chars().count();
        let wait = state.as_ref().map_or(0, |s| s.wait_seconds);
        self.send.set_sensitive(typed > 0 && wait == 0);
        let max = state.as_ref().map_or(150, |s| s.max_length as usize);
        let left = max.saturating_sub(self.entry.text().chars().count());
        let text = if wait > 0 {
            Some(t::party_chat_wait(i64::from(wait)))
        } else if left <= words::COUNT_FROM && typed > 0 {
            Some(left.to_string())
        } else {
            None
        };
        self.tail.set_label(text.as_deref().unwrap_or(""));
        self.tail.set_visible(text.is_some());
        if text.is_some() {
            self.entry.add_css_class("counting");
        } else {
            self.entry.remove_css_class("counting");
        }
        let widget = self.entry.upcast_ref::<gtk::Widget>();
        if left <= words::COUNT_FROM && typed > 0 {
            if !self.told_left.replace(true) {
                widget.announce(
                    &t::party_chat_left(left as i64),
                    gtk::AccessibleAnnouncementPriority::Medium,
                );
            }
        } else {
            self.told_left.set(false);
        }
    }

    fn me_weak(&self) -> std::rc::Weak<Chat> {
        self.me.borrow().clone()
    }
}
