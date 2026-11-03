//! A party's conversation, kept in memory for the party and nowhere else: the lines that came
//! back from the server, the reactions among them, who is muted on this device, what this device
//! may send, and a send that never came back. Pure and clocked by the caller (seconds on any
//! steady clock), so the party's task only wires it to the room and the front end.
//!
//! Nothing here is written to disk, put in a bug report or logged: a member who joins later sees
//! nothing said before, and leaving the party drops it all.

use std::collections::{BTreeSet, VecDeque};

use kaeri_model::party::{
    CONVERSATION_LINES, ECHO_WAIT_SECONDS, PartyChatBody, PartyChatLine, PartyChatRefusal,
    PartyChatState, PartyConversation, PartyReaction, PartyReactionMark, REACTION_MARKS,
};
use kaeri_party::{
    Reaction, SendLimits,
    notice::clean_to,
    reaction::{self},
    wire::DEFAULT_CHAT_LENGTH,
};

/// What a line from the room is, for the party's task to pass on.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Heard {
    /// Nothing to show on the video: from a muted member, or empty.
    Quiet,
    /// Something typed, to show as a notice.
    Typed { from: String, you: bool, text: String },
    /// A reaction, for the moment on the video.
    Reacted(PartyReaction),
}

/// A line sent and not yet back.
#[derive(Debug, Clone)]
struct Pending {
    text: String,
    at: f64,
    /// A reaction, which is dropped without a word when it is lost.
    reaction: bool,
}

pub(crate) struct Chat {
    me: String,
    allowed: bool,
    max_length: usize,
    limits: SendLimits,
    lines: VecDeque<PartyChatLine>,
    marks: VecDeque<PartyReactionMark>,
    muted: BTreeSet<String>,
    pending: Vec<Pending>,
    failed: Option<String>,
    next_id: u64,
    /// The waits last told to the front end, so a change is told once.
    told: (u32, u32),
    /// The terminal's Hide chat: nothing reaches the player while it is on.
    hidden: bool,
}

impl Chat {
    pub fn new(me: &str) -> Self {
        Self {
            me: me.to_owned(),
            allowed: true,
            max_length: DEFAULT_CHAT_LENGTH,
            limits: SendLimits::default(),
            lines: VecDeque::new(),
            marks: VecDeque::new(),
            muted: BTreeSet::new(),
            pending: Vec::new(),
            failed: None,
            next_id: 1,
            told: (0, 0),
            hidden: false,
        }
    }

    pub fn set_me(&mut self, name: &str) {
        name.clone_into(&mut self.me);
    }

    /// What the server takes, from its welcome.
    pub fn set_features(&mut self, allowed: bool, max_length: usize) {
        self.allowed = allowed;
        self.max_length = max_length;
    }

    /// The episode changed: its reactions are not the next one's.
    pub fn new_episode(&mut self) {
        self.marks.clear();
    }

    /// Whether a tick is wanted: a send is waited for, or a wait is running down.
    pub fn busy(&self) -> bool {
        !self.pending.is_empty() || self.told != (0, 0)
    }

    /// The line to send for `text`, or why not. `Ok(None)` for text with nothing in it.
    pub fn say(&mut self, text: &str, now: f64) -> Result<Option<String>, PartyChatRefusal> {
        if !self.allowed {
            return Err(PartyChatRefusal::Off);
        }
        let text = clean_to(text, self.max_length);
        // A cut at the limit ends in the ellipsis `clean_to` adds: it is not part of what was
        // written, and would push the line past what the server takes.
        let text = fit(&text, self.max_length);
        if text.is_empty() {
            return Ok(None);
        }
        self.limits.take_message(now).map_err(refusal)?;
        self.failed = None;
        self.pending.push(Pending { text: text.clone(), at: now, reaction: false });
        Ok(Some(text))
    }

    /// The line for a reaction at `at` seconds into the episode, or why not.
    pub fn react(
        &mut self,
        reaction: Reaction,
        at: Option<f64>,
        now: f64,
    ) -> Result<String, PartyChatRefusal> {
        if !self.allowed {
            return Err(PartyChatRefusal::Off);
        }
        let Some(at) = at else { return Err(PartyChatRefusal::NotPlaying) };
        self.limits.take_reaction(now).map_err(refusal)?;
        let line = reaction::encode(reaction, at);
        self.pending.push(Pending { text: line.clone(), at: now, reaction: true });
        Ok(line)
    }

    /// A line came from the room: this device's own coming back settles its send.
    pub fn hear(&mut self, from: &str, text: &str) -> Heard {
        let text = text.trim();
        if text.is_empty() {
            return Heard::Quiet;
        }
        let you = from == self.me;
        if you && let Some(at) = self.pending.iter().position(|p| p.text == text) {
            self.pending.remove(at);
        }
        let muted = self.muted.contains(from);
        let id = self.next_id;
        self.next_id += 1;
        let (body, heard) = match reaction::decode(text) {
            Some((kind, at)) => {
                // Kept for a muted member too, so Unmute brings their ticks back.
                self.marks.push_back(PartyReactionMark {
                    at,
                    from: from.to_owned(),
                    reaction: model(kind),
                });
                while self.marks.len() > REACTION_MARKS {
                    self.marks.pop_front();
                }
                let moment =
                    PartyReaction { from: from.to_owned(), you, reaction: model(kind), at };
                (
                    PartyChatBody::Reacted { reaction: model(kind), at, with: Vec::new() },
                    Heard::Reacted(moment),
                )
            }
            None => (
                PartyChatBody::Said { text: text.to_owned() },
                Heard::Typed { from: from.to_owned(), you, text: text.to_owned() },
            ),
        };
        self.lines.push_back(PartyChatLine {
            id,
            from: from.to_owned(),
            you,
            body,
            muted: false,
        });
        while self.lines.len() > CONVERSATION_LINES {
            self.lines.pop_front();
        }
        if muted { Heard::Quiet } else { heard }
    }

    /// Mute or unmute a member for the party. Their lines stay in the buffer.
    pub fn mute(&mut self, name: &str, muted: bool) {
        if muted {
            self.muted.insert(name.to_owned());
        } else {
            self.muted.remove(name);
        }
    }

    /// Move time on: sends with no echo after [`ECHO_WAIT_SECONDS`] are lost. Returns whether the
    /// conversation changed (a text send failed).
    pub fn tick(&mut self, now: f64) -> bool {
        let mut changed = false;
        let mut kept = Vec::with_capacity(self.pending.len());
        for send in std::mem::take(&mut self.pending) {
            if now - send.at >= f64::from(ECHO_WAIT_SECONDS) || send.at > now {
                if !send.reaction {
                    self.failed = Some(send.text);
                    changed = true;
                }
            } else {
                kept.push(send);
            }
        }
        self.pending = kept;
        changed
    }

    /// The state for the front end, and whether it differs from the one last told.
    pub fn state(&mut self, now: f64) -> PartyChatState {
        PartyChatState {
            allowed: self.allowed,
            max_length: u32::try_from(self.max_length).unwrap_or(u32::MAX),
            muted: self.muted.iter().cloned().collect(),
            wait_seconds: whole(self.limits.message_wait(now)),
            react_wait_seconds: whole(self.limits.reaction_wait(now)),
            hidden: self.hidden,
        }
    }

    /// The terminal's Hide chat, from the config and as it is turned over.
    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    pub fn hidden(&self) -> bool {
        self.hidden
    }

    /// The most characters in one message: the server's, or 150.
    pub fn max_length(&self) -> usize {
        self.max_length
    }

    /// The waits last told to the front end.
    pub fn told_waits(&self) -> (u32, u32) {
        self.told
    }

    /// Record the waits of a state as told.
    pub fn told(&mut self, state: &PartyChatState) {
        self.told = (state.wait_seconds, state.react_wait_seconds);
    }

    /// The conversation as the front end shows it: reactions made at the same second by
    /// different members merge into one line.
    pub fn conversation(&self) -> PartyConversation {
        let mut lines: Vec<PartyChatLine> = Vec::with_capacity(self.lines.len());
        for line in &self.lines {
            let mut line = line.clone();
            line.muted = self.muted.contains(&line.from);
            if let (
                PartyChatBody::Reacted { reaction, at, .. },
                Some(PartyChatLine {
                    body: PartyChatBody::Reacted { reaction: before, at: when, with },
                    from: first,
                    you: first_you,
                    muted: first_muted,
                    ..
                }),
            ) = (&line.body, lines.last_mut())
                && !line.muted
                && !*first_muted
                && !line.you
                && !*first_you
                && reaction == before
                && (at - *when).abs() < 0.5
                && *first != line.from
                && !with.contains(&line.from)
            {
                with.push(line.from);
                continue;
            }
            lines.push(line);
        }
        PartyConversation {
            lines,
            marks: self
                .marks
                .iter()
                .filter(|m| !self.muted.contains(&m.from))
                .cloned()
                .collect(),
            failed: self.failed.clone(),
        }
    }
}

fn refusal(wait: f64) -> PartyChatRefusal {
    PartyChatRefusal::Wait { seconds: whole(wait).max(1) }
}

/// Seconds rounded up to a whole number.
fn whole(seconds: f64) -> u32 {
    if seconds > 0.0 { seconds.ceil() as u32 } else { 0 }
}

/// `text` within `limit` characters, without the ellipsis `clean_to` ends a cut one with.
fn fit(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    text.chars().take(limit).collect::<String>().trim_end().to_owned()
}

pub(crate) fn model(reaction: Reaction) -> kaeri_model::party::Reaction {
    use kaeri_model::party::Reaction as M;
    match reaction {
        Reaction::Laugh => M::Laugh,
        Reaction::Gasp => M::Gasp,
        Reaction::Clap => M::Clap,
        Reaction::Cry => M::Cry,
    }
}

pub(crate) fn party(reaction: kaeri_model::party::Reaction) -> Reaction {
    use kaeri_model::party::Reaction as M;
    match reaction {
        M::Laugh => Reaction::Laugh,
        M::Gasp => Reaction::Gasp,
        M::Clap => Reaction::Clap,
        M::Cry => Reaction::Cry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(chat: &Chat) -> Vec<String> {
        chat.conversation()
            .lines
            .into_iter()
            .map(|l| match l.body {
                PartyChatBody::Said { text } => format!("{}: {text}", l.from),
                PartyChatBody::Reacted { reaction, at, with } => {
                    format!("{} {:?} {at} {with:?}", l.from, reaction)
                }
            })
            .collect()
    }

    #[test]
    fn a_line_shows_when_it_comes_back_and_not_before() {
        let mut chat = Chat::new("Noa");
        let line = chat.say("  back in 2  ", 0.0).unwrap().unwrap();
        assert_eq!(line, "back in 2");
        assert!(chat.conversation().lines.is_empty());
        assert_eq!(
            chat.hear("Noa", "back in 2"),
            Heard::Typed { from: "Noa".into(), you: true, text: "back in 2".into() }
        );
        let conversation = chat.conversation();
        assert_eq!(conversation.lines.len(), 1);
        assert!(conversation.lines[0].you);
        // Echoed, so it is not lost later.
        assert!(!chat.tick(60.0));
        assert_eq!(chat.conversation().failed, None);
    }

    #[test]
    fn no_echo_in_five_seconds_is_a_failed_send_with_the_text_back() {
        let mut chat = Chat::new("Noa");
        chat.say("back in 2", 0.0).unwrap();
        assert!(!chat.tick(4.9));
        assert!(chat.tick(5.0));
        assert_eq!(chat.conversation().failed.as_deref(), Some("back in 2"));
        assert!(!chat.busy());
        // The next send clears it.
        chat.say("again", 6.0).unwrap();
        assert_eq!(chat.conversation().failed, None);
    }

    #[test]
    fn a_lost_reaction_is_dropped_without_a_failure() {
        let mut chat = Chat::new("Noa");
        chat.react(Reaction::Clap, Some(10.0), 0.0).unwrap();
        assert!(!chat.tick(9.0));
        assert_eq!(chat.conversation().failed, None);
    }

    #[test]
    fn a_line_is_cut_to_what_the_server_takes_and_cleaned() {
        let mut chat = Chat::new("Noa");
        chat.set_features(true, 10);
        let line = chat.say("one\ntwo   three four five", 0.0).unwrap().unwrap();
        assert_eq!(line, "one two th");
        assert_eq!(chat.say("  \n ", 1.0), Ok(None));
    }

    #[test]
    fn chat_off_refuses_every_send() {
        let mut chat = Chat::new("Noa");
        chat.set_features(false, 150);
        assert_eq!(chat.say("hi", 0.0), Err(PartyChatRefusal::Off));
        assert_eq!(chat.react(Reaction::Laugh, Some(1.0), 0.0), Err(PartyChatRefusal::Off));
        assert!(!chat.state(0.0).allowed);
    }

    #[test]
    fn the_fourth_message_in_five_seconds_waits_and_the_state_says_how_long() {
        let mut chat = Chat::new("Noa");
        for (i, t) in [0.0, 0.5, 1.0].into_iter().enumerate() {
            chat.say(&format!("m{i}"), t).unwrap();
        }
        assert_eq!(chat.say("m3", 2.0), Err(PartyChatRefusal::Wait { seconds: 3 }));
        assert_eq!(chat.state(2.0).wait_seconds, 3);
        assert_eq!(chat.state(4.5).wait_seconds, 1);
        assert_eq!(chat.state(5.0).wait_seconds, 0);
    }

    #[test]
    fn reactions_need_a_playing_episode_and_a_gap() {
        let mut chat = Chat::new("Noa");
        assert_eq!(chat.react(Reaction::Gasp, None, 0.0), Err(PartyChatRefusal::NotPlaying));
        assert_eq!(chat.react(Reaction::Gasp, Some(724.0), 0.0).unwrap(), "*gasps* 12:04");
        assert_eq!(
            chat.react(Reaction::Gasp, Some(725.0), 1.0),
            Err(PartyChatRefusal::Wait { seconds: 1 })
        );
        assert_eq!(chat.state(1.0).react_wait_seconds, 1);
    }

    #[test]
    fn a_reaction_is_a_moment_and_a_mark_not_a_typed_line() {
        let mut chat = Chat::new("Noa");
        let heard = chat.hear("Mika", "*laughs* 12:04");
        assert_eq!(
            heard,
            Heard::Reacted(PartyReaction {
                from: "Mika".into(),
                you: false,
                reaction: kaeri_model::party::Reaction::Laugh,
                at: 724.0,
            })
        );
        let conversation = chat.conversation();
        assert_eq!(conversation.marks.len(), 1);
        assert_eq!(conversation.marks[0].at, 724.0);
        chat.new_episode();
        assert!(chat.conversation().marks.is_empty());
        assert_eq!(chat.conversation().lines.len(), 1);
    }

    #[test]
    fn reactions_at_the_same_moment_become_one_line() {
        let mut chat = Chat::new("Noa");
        chat.hear("Ren", "*laughs* 12:04");
        chat.hear("Mika", "*laughs* 12:04");
        chat.hear("Sol", "*laughs* 12:05");
        chat.hear("Ren", "*laughs* 12:04");
        let lines = said(&chat);
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(lines[0].contains("Ren") && lines[0].contains("Mika"), "{lines:?}");
    }

    #[test]
    fn mute_hides_a_member_from_the_video_the_marks_and_the_list_but_keeps_the_lines() {
        let mut chat = Chat::new("Noa");
        chat.hear("Ren", "so good");
        chat.hear("Ren", "*claps* 3:24");
        chat.mute("Ren", true);
        assert_eq!(chat.hear("Ren", "still here"), Heard::Quiet);
        assert_eq!(chat.hear("Ren", "*gasps* 4:00"), Heard::Quiet);
        let conversation = chat.conversation();
        assert!(conversation.lines.iter().all(|l| l.muted));
        assert!(conversation.marks.is_empty());
        assert_eq!(chat.state(0.0).muted, vec!["Ren".to_owned()]);
        chat.mute("Ren", false);
        let conversation = chat.conversation();
        assert!(conversation.lines.iter().all(|l| !l.muted));
        assert_eq!(conversation.marks.len(), 2);
    }

    #[test]
    fn hide_chat_keeps_the_conversation_and_the_state_says_it_is_on() {
        let mut chat = Chat::new("Noa");
        chat.hear("Ren", "so good");
        chat.set_hidden(true);
        assert!(chat.hidden() && chat.state(0.0).hidden);
        // Hearing still keeps the list, for the people popover and the TUI's pane.
        chat.hear("Ren", "still here");
        assert_eq!(said(&chat), vec!["Ren: so good", "Ren: still here"]);
        chat.set_hidden(false);
        assert!(!chat.state(0.0).hidden);
    }

    #[test]
    fn the_conversation_keeps_two_hundred_lines_oldest_dropped_first() {
        let mut chat = Chat::new("Noa");
        for i in 0..250 {
            chat.hear("Mika", &format!("line {i}"));
        }
        let conversation = chat.conversation();
        assert_eq!(conversation.lines.len(), 200);
        assert_eq!(conversation.lines[0].id, 51);
        assert_eq!(conversation.lines[199].id, 250);
    }

    #[test]
    fn a_typed_look_alike_is_a_reaction_and_a_half_typed_one_is_text() {
        let mut chat = Chat::new("Noa");
        assert!(matches!(chat.hear("Mika", "*claps* 1:00"), Heard::Reacted(_)));
        assert!(matches!(chat.hear("Mika", "*claps*"), Heard::Typed { .. }));
    }

    #[test]
    fn a_line_from_another_device_of_this_member_is_still_theirs() {
        let mut chat = Chat::new("Noa");
        chat.hear("Noa", "from the other room");
        assert!(chat.conversation().lines[0].you);
    }

    #[test]
    fn nothing_is_busy_when_nothing_waits() {
        let mut chat = Chat::new("Noa");
        assert!(!chat.busy());
        chat.say("hi", 0.0).unwrap();
        assert!(chat.busy());
    }
}
