//! The party's conversation, the window's side: the words every place says for it (a reaction
//! line, a tick's names, how long a line holds), and the pane that shows it with its field,
//! in the people popover and in the lobby. The conversation itself is the engine's, in memory
//! for the party only, and arrives whole after every change.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    party::{
        PartyChatBody, PartyChatLine, PartyChatState, PartyCommand, PartyReactionMark, Reaction,
    },
};

use crate::window::Ctx;

/// The newest line's own words with its reaction: "Ren and Mika laughed".
pub fn reacted(names: &str, reaction: Reaction) -> String {
    match reaction {
        Reaction::Laugh => t::party_reacted_laugh(names),
        Reaction::Gasp => t::party_reacted_gasp(names),
        Reaction::Clap => t::party_reacted_clap(names),
        Reaction::Cry => t::party_reacted_cry(names),
    }
}

/// The button's name for a reaction: "Laugh".
pub fn label(reaction: Reaction) -> String {
    match reaction {
        Reaction::Laugh => t::party_react_laugh(),
        Reaction::Gasp => t::party_react_gasp(),
        Reaction::Clap => t::party_react_clap(),
        Reaction::Cry => t::party_react_cry(),
    }
}

/// A moment in the episode: `m:ss`, or `h:mm:ss` from the first hour.
pub fn time(at: f64) -> String {
    let total = at.max(0.0).round() as i64;
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Members named together: "Ren", "Ren and Mika", "Ren, Mika and Sol", "Ren, Mika and 2 others".
pub fn names(first: &str, with: &[String]) -> String {
    match with {
        [] => first.to_owned(),
        [second] => t::party_names_two(first, second),
        [second, third] => t::party_names_three(first, second, third),
        [second, rest @ ..] => t::party_names_others(first, second, rest.len() as i64),
    }
}

/// Who a line is from, as it reads: "You" for this device's own.
pub fn who(line: &PartyChatLine) -> String {
    if line.you { t::party_chat_you() } else { line.from.clone() }
}

/// A reaction line as the conversation says it: "Ren and Mika laughed at 12:04".
pub fn reaction_line(line: &PartyChatLine) -> Option<String> {
    let PartyChatBody::Reacted { reaction, at, with } = &line.body else { return None };
    Some(t::party_reacted_at(&reacted(&names(&who(line), with), *reaction), &time(*at)))
}

/// How long a line holds on the video: 4 s, and 1 s for every 10 words past 12, as notices do.
pub fn hold(text: &str) -> Duration {
    let words = text.split_whitespace().count();
    Duration::from_secs_f64(4.0 + words.saturating_sub(12) as f64 / 10.0)
}

/// One tick on the seek bar: where it is and who reacted there.
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    pub at: f64,
    pub names: Vec<String>,
}

impl Tick {
    /// The seek bar's label near it: "12:04 · Ren and Mika".
    pub fn label(&self) -> String {
        let (first, rest) =
            self.names.split_first().map_or(("", &[][..]), |(f, r)| (f.as_str(), r));
        t::party_tick_label(&time(self.at), &names(first, rest))
    }
}

/// The reactions' marks as ticks on a line `width` px wide for an episode `duration` long:
/// marks within `near` px of the tick before them share it.
pub fn ticks(marks: &[PartyReactionMark], duration: f64, width: f64, near: f64) -> Vec<Tick> {
    if duration <= 0.0 || width <= 0.0 {
        return Vec::new();
    }
    let mut sorted: Vec<&PartyReactionMark> = marks.iter().collect();
    sorted.sort_by(|a, b| a.at.total_cmp(&b.at));
    let mut out: Vec<Tick> = Vec::new();
    for mark in sorted {
        let joins =
            out.last().is_some_and(|last| (mark.at - last.at) / duration * width <= near);
        if joins {
            let last = out.last_mut().expect("checked");
            if !last.names.contains(&mark.from) {
                last.names.push(mark.from.clone());
            }
        } else {
            out.push(Tick { at: mark.at, names: vec![mark.from.clone()] });
        }
    }
    out
}

/// The lines a conversation lists: a muted member's run of lines becomes one line saying so.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Said { who: String, you: bool, text: String },
    Reacted(String),
    Muted(String),
}

pub fn entries(lines: &[PartyChatLine]) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for line in lines {
        if line.muted {
            let again = matches!(out.last(), Some(Entry::Muted(name)) if name == &line.from);
            if !again {
                out.push(Entry::Muted(line.from.clone()));
            }
            continue;
        }
        out.push(match &line.body {
            PartyChatBody::Said { text } => {
                Entry::Said { who: who(line), you: line.you, text: text.clone() }
            }
            PartyChatBody::Reacted { .. } => {
                Entry::Reacted(reaction_line(line).unwrap_or_default())
            }
        });
    }
    out
}

/// The newest lines typed by others that are new since `seen`, and the newest id.
pub fn fresh(lines: &[PartyChatLine], seen: u64) -> (Vec<&PartyChatLine>, u64) {
    let newest = lines.iter().map(|l| l.id).max().unwrap_or(seen).max(seen);
    let new = lines
        .iter()
        .filter(|l| l.id > seen && !l.muted && matches!(l.body, PartyChatBody::Said { .. }))
        .collect();
    (new, newest)
}

/// Say `text` to the room.
pub fn say(ctx: &Ctx, text: &str) {
    ctx.hub.send(Command::Party(PartyCommand::Say { text: text.to_owned() }));
}

/// Where a pane is: the people popover, which is narrow and scrolls, or the lobby, which has
/// the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Popover,
    Lobby,
}

/// The conversation with its field: the lines newest last, "Couldn't send" with Try again after
/// a line that never came back, the field linked with Send, and the three other things the
/// place can say instead (Hide chat is on, the server has no chat, nothing yet).
pub struct Pane {
    pub root: gtk::Box,
    ctx: Rc<Ctx>,
    place: Place,
    list: gtk::Box,
    scroll: gtk::ScrolledWindow,
    field: gtk::Box,
    entry: gtk::Entry,
    send: gtk::Button,
    tail: gtk::Label,
    off: gtk::Label,
    hidden: gtk::Box,
    privacy: gtk::Label,
    /// The lost text already put back in the field, so a later snapshot does not put it back
    /// again after it was edited or sent.
    returned: RefCell<Option<String>>,
    told_left: Cell<bool>,
    told_wait: Cell<u32>,
}

/// Characters left at which the count shows.
pub const COUNT_FROM: usize = 20;

impl Pane {
    pub fn new(ctx: &Rc<Ctx>, place: Place) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
        list.update_property(&[gtk::accessible::Property::Label(&t::party_chat_title())]);
        let scroll = gtk::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .build();
        match place {
            Place::Popover => scroll.set_max_content_height(200),
            Place::Lobby => scroll.set_max_content_height(360),
        }

        let entry =
            gtk::Entry::builder().placeholder_text(t::party_chat_field()).hexpand(true).build();
        entry.add_css_class("chat-entry");
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
        over.set_hexpand(true);
        let send = gtk::Button::with_label(&t::party_chat_send());
        send.set_sensitive(false);
        let field = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        field.add_css_class("linked");
        field.append(&over);
        field.append(&send);

        let off = crate::widgets::haze(&t::party_chat_off(), "caption");
        off.set_xalign(0.0);
        off.set_wrap(true);
        off.set_visible(false);

        let hidden = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let hidden_title = gtk::Label::new(Some(&t::party_chat_hidden()));
        hidden_title.set_xalign(0.0);
        let hidden_line = crate::widgets::haze(&t::party_chat_hidden_line(), "caption");
        hidden_line.set_xalign(0.0);
        hidden_line.set_wrap(true);
        let show = gtk::Button::with_label(&t::party_chat_show());
        show.set_halign(gtk::Align::Start);
        show.set_margin_top(4);
        hidden.append(&hidden_title);
        hidden.append(&hidden_line);
        hidden.append(&show);
        hidden.set_visible(false);

        let privacy = crate::widgets::haze(&t::party_chat_privacy(), "caption");
        privacy.set_xalign(0.0);
        privacy.set_wrap(true);
        privacy.set_visible(place == Place::Lobby);

        root.append(&scroll);
        root.append(&field);
        root.append(&off);
        root.append(&hidden);
        root.append(&privacy);

        let pane = Rc::new(Self {
            root,
            ctx: Rc::clone(ctx),
            place,
            list,
            scroll,
            field,
            entry: entry.clone(),
            send: send.clone(),
            tail,
            off,
            hidden,
            privacy,
            returned: RefCell::default(),
            told_left: Cell::new(false),
            told_wait: Cell::new(0),
        });
        show.connect_clicked({
            let ctx = Rc::clone(ctx);
            move |_| ctx.prefs.set_hide_chat(false)
        });
        entry.connect_activate({
            let pane = Rc::downgrade(&pane);
            move |_| {
                if let Some(pane) = pane.upgrade() {
                    pane.submit();
                }
            }
        });
        send.connect_clicked({
            let pane = Rc::downgrade(&pane);
            move |_| {
                if let Some(pane) = pane.upgrade() {
                    pane.submit();
                }
            }
        });
        entry.connect_changed({
            let pane = Rc::downgrade(&pane);
            move |_| {
                if let Some(pane) = pane.upgrade() {
                    pane.typed();
                }
            }
        });
        ctx.prefs.watch_hide_chat({
            let pane = Rc::downgrade(&pane);
            move |_| {
                if let Some(pane) = pane.upgrade() {
                    pane.refresh();
                }
            }
        });
        ctx.hub.listen({
            let pane = Rc::downgrade(&pane);
            move |event| {
                let Some(pane) = pane.upgrade() else { return };
                if matches!(
                    event,
                    Event::PartyChat(_) | Event::PartyChatState(_) | Event::PartyLeft { .. }
                ) {
                    pane.refresh();
                }
            }
        });
        pane.refresh();
        pane
    }

    /// What Send is waiting for, from the engine: whole seconds, 0 when it may send.
    fn wait(&self) -> u32 {
        self.ctx.hub.known.borrow().party_chat_state.as_ref().map_or(0, |s| s.wait_seconds)
    }

    fn submit(&self) {
        let text = self.entry.text().trim().to_owned();
        if text.is_empty() || self.wait() > 0 {
            return;
        }
        say(&self.ctx, &text);
        self.entry.set_text("");
    }

    /// The field changed: Send and the count at its end.
    fn typed(&self) {
        let state = self.ctx.hub.known.borrow().party_chat_state.clone();
        self.show_tail(state.as_ref());
    }

    fn show_tail(&self, state: Option<&PartyChatState>) {
        let typed = self.entry.text().trim().chars().count();
        let wait = state.map_or(0, |s| s.wait_seconds);
        self.send.set_sensitive(typed > 0 && wait == 0);
        let max = state.map_or(150, |s| s.max_length as usize);
        let left = max.saturating_sub(self.entry.text().chars().count());
        let text = if wait > 0 {
            Some(t::party_chat_wait(i64::from(wait)))
        } else if left <= COUNT_FROM && typed > 0 {
            Some(left.to_string())
        } else {
            None
        };
        match &text {
            Some(text) => self.tail.set_label(text),
            None => self.tail.set_label(""),
        }
        self.tail.set_visible(text.is_some());
        if text.is_some() {
            self.entry.add_css_class("counting");
        } else {
            self.entry.remove_css_class("counting");
        }
        // Said once, to a screen reader: the count at 20 left, and a wait when it starts.
        let widget = self.entry.upcast_ref::<gtk::Widget>();
        if left <= COUNT_FROM && typed > 0 {
            if !self.told_left.replace(true) {
                widget.announce(
                    &t::party_chat_left(left as i64),
                    gtk::AccessibleAnnouncementPriority::Medium,
                );
            }
        } else {
            self.told_left.set(false);
        }
        if wait > 0 {
            if self.told_wait.replace(wait) == 0 {
                widget.announce(
                    &t::party_chat_wait_spoken(i64::from(wait)),
                    gtk::AccessibleAnnouncementPriority::Medium,
                );
            }
        } else {
            self.told_wait.set(0);
        }
    }

    /// Show the conversation and what the room allows, as the engine last said.
    pub fn refresh(&self) {
        let (conversation, state) = {
            let known = self.ctx.hub.known.borrow();
            (known.party_chat.clone(), known.party_chat_state.clone())
        };
        let hide = self.ctx.prefs.hide_chat();
        let allowed = state.as_ref().is_none_or(|s| s.allowed);
        self.hidden.set_visible(hide);
        self.off.set_visible(!hide && !allowed);
        let live = !hide && allowed;
        self.scroll.set_visible(live);
        self.field.set_visible(live);
        self.privacy.set_visible(self.place == Place::Lobby && !hide);
        if !live {
            return;
        }
        if let Some(state) = &state {
            self.entry.set_max_length(state.max_length.try_into().unwrap_or(150));
        }
        self.rebuild(&conversation, state.as_ref());
        // A send that never came back: its text is back in the field, once.
        match &conversation.failed {
            Some(text) => {
                if self.returned.borrow().as_deref() != Some(text.as_str()) {
                    if self.entry.text().is_empty() {
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
        self.show_tail(state.as_ref());
    }

    fn rebuild(
        &self,
        conversation: &kaeri_model::party::PartyConversation,
        state: Option<&PartyChatState>,
    ) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        let muted: Vec<String> = state.map(|s| s.muted.clone()).unwrap_or_default();
        for entry in entries(&conversation.lines) {
            self.list.append(&match entry {
                Entry::Said { who, you, text } => said_row(&who, you, &text),
                Entry::Reacted(line) => quiet_row(&line),
                Entry::Muted(name) => {
                    let ctx = Rc::clone(&self.ctx);
                    let name_for = name.clone();
                    let _ = &muted;
                    muted_row(&name, move || {
                        ctx.hub.send(Command::Party(PartyCommand::Mute {
                            name: name_for.clone(),
                            muted: false,
                        }));
                    })
                }
            });
        }
        if let Some(text) = &conversation.failed {
            let ctx = Rc::clone(&self.ctx);
            let text = text.clone();
            self.list.append(&failed_row(move || say(&ctx, &text)));
        }
        let adjustment = self.scroll.vadjustment();
        let scroll = self.scroll.clone();
        gtk::glib::idle_add_local_once(move || {
            let adjustment = scroll.vadjustment();
            adjustment.set_value(adjustment.upper() - adjustment.page_size());
        });
        let _ = adjustment;
    }
}

/// A typed line: the name in bold, "You" in haze, then the words, wrapped.
fn said_row(who: &str, you: bool, text: &str) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = gtk::Label::new(Some(who));
    name.add_css_class("heading");
    if you {
        name.add_css_class("haze");
    }
    name.set_valign(gtk::Align::Start);
    let words = gtk::Label::new(Some(text));
    words.set_wrap(true);
    words.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    words.set_xalign(0.0);
    words.set_hexpand(true);
    words.set_max_width_chars(34);
    row.append(&name);
    row.append(&words);
    row.set_accessible_role(gtk::AccessibleRole::Group);
    row.update_property(&[gtk::accessible::Property::Label(&t::party_chat_announce(
        who, text,
    ))]);
    row.upcast()
}

/// A line in haze: a reaction, "Ren and Mika laughed at 12:04".
fn quiet_row(text: &str) -> gtk::Widget {
    let label = crate::widgets::haze(text, "caption");
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.upcast()
}

/// "Ren is muted · Unmute".
fn muted_row(name: &str, unmute: impl Fn() + 'static) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let line = crate::widgets::haze(&t::party_muted_line(name), "caption");
    line.set_xalign(0.0);
    let button = gtk::Button::with_label(&t::party_unmute_short());
    button.add_css_class("flat");
    button.add_css_class("chat-link");
    button.set_tooltip_text(Some(&t::party_unmute(name)));
    button.update_property(&[gtk::accessible::Property::Label(&t::party_unmute(name))]);
    button.connect_clicked(move |_| unmute());
    row.append(&line);
    row.append(&crate::widgets::haze("\u{b7}", "caption")); // i18n-skip
    row.append(&button);
    row.upcast()
}

/// "Couldn't send · Try again".
fn failed_row(retry: impl Fn() + 'static) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let line = crate::widgets::haze(&t::party_chat_not_sent(), "caption");
    line.set_xalign(0.0);
    let button = gtk::Button::with_label(&t::party_chat_retry());
    button.add_css_class("flat");
    button.add_css_class("chat-link");
    button.connect_clicked(move |_| retry());
    row.append(&line);
    row.append(&crate::widgets::haze("\u{b7}", "caption")); // i18n-skip
    row.append(&button);
    row.set_accessible_role(gtk::AccessibleRole::Group);
    row.update_property(&[gtk::accessible::Property::Label(&t::party_chat_not_sent())]);
    row.upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: u64, from: &str, body: PartyChatBody, muted: bool) -> PartyChatLine {
        PartyChatLine { id, from: from.into(), you: false, body, muted }
    }

    fn said(id: u64, from: &str, text: &str) -> PartyChatLine {
        line(id, from, PartyChatBody::Said { text: text.into() }, false)
    }

    #[test]
    fn a_moment_reads_as_minutes_and_seconds_and_hours_from_the_first() {
        assert_eq!(time(724.0), "12:04");
        assert_eq!(time(5.0), "0:05");
        assert_eq!(time(3725.0), "1:02:05");
    }

    #[test]
    fn members_are_named_together_up_to_three_and_then_counted() {
        let with = |n: &[&str]| n.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(names("Ren", &[]), "Ren");
        assert_eq!(names("Ren", &with(&["Mika"])), "Ren and Mika");
        assert_eq!(names("Ren", &with(&["Mika", "Sol"])), "Ren, Mika and Sol");
        assert_eq!(names("Ren", &with(&["Mika", "Sol", "Ada"])), "Ren, Mika and 2 others");
    }

    #[test]
    fn a_reaction_line_names_who_and_when() {
        let reacted = line(
            3,
            "Ren",
            PartyChatBody::Reacted {
                reaction: Reaction::Laugh,
                at: 724.0,
                with: vec!["Mika".into()],
            },
            false,
        );
        assert_eq!(reaction_line(&reacted).as_deref(), Some("Ren and Mika laughed at 12:04"));
        assert_eq!(reaction_line(&said(1, "Ren", "hi")), None);
    }

    #[test]
    fn a_line_holds_four_seconds_and_one_more_for_each_ten_words_past_twelve() {
        assert_eq!(hold("this part again"), Duration::from_secs(4));
        let long = "one two three four five six seven eight nine ten eleven twelve thirteen";
        assert_eq!(hold(long), Duration::from_secs_f64(4.1));
        let longer = vec!["word"; 32].join(" ");
        assert_eq!(hold(&longer), Duration::from_secs(6));
    }

    #[test]
    fn marks_within_four_pixels_share_one_tick() {
        let mark = |at: f64, from: &str| PartyReactionMark {
            at,
            from: from.into(),
            reaction: Reaction::Laugh,
        };
        // 1440 s over 960 px: 1.5 s a pixel.
        let marks =
            [mark(724.0, "Ren"), mark(204.0, "Sol"), mark(726.0, "Mika"), mark(468.0, "Ren")];
        let found = ticks(&marks, 1440.0, 960.0, 4.0);
        assert_eq!(found.len(), 3);
        assert_eq!(found[2].names, ["Ren", "Mika"]);
        assert_eq!(found[2].label(), "12:04 \u{b7} Ren and Mika");
        let apart = ticks(&marks, 1440.0, 960.0, 0.5);
        assert_eq!(apart.len(), 4);
    }

    #[test]
    fn a_muted_members_lines_are_one_line_saying_so() {
        let lines = [
            said(1, "Mika", "ok starting"),
            line(2, "Ren", PartyChatBody::Said { text: "hidden".into() }, true),
            line(3, "Ren", PartyChatBody::Said { text: "also hidden".into() }, true),
            said(4, "Sol", "wait"),
        ];
        let listed = entries(&lines);
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[1], Entry::Muted("Ren".into()));
    }

    #[test]
    fn only_new_typed_lines_from_others_are_fresh_and_a_snapshot_keeps_the_count() {
        let lines = [said(1, "Mika", "a"), said(2, "Ren", "b"), said(3, "Sol", "c")];
        let (new, newest) = fresh(&lines, 1);
        assert_eq!(new.iter().map(|l| l.id).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(newest, 3);
        let (none, same) = fresh(&lines, 3);
        assert!(none.is_empty());
        assert_eq!(same, 3);
    }
}
