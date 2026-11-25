//! The room's conversation, kept in memory for the room and nowhere else: lines, reactions,
//! the room's own lines ("Mika paused at 12:04"), who is muted here, what this device may send,
//! and a send no relay echoed. Pure and clocked by the caller, in epoch milliseconds.
//!
//! Nothing here is written to disk, put in a bug report or logged.

use std::collections::{BTreeSet, VecDeque};

use kaeri_model::{
    party::{CONVERSATION_LINES, ECHO_WAIT_SECONDS, REACTION_MARKS},
    room::{
        CHAT_MAX_LENGTH, Reaction, RoomChatBody, RoomChatLine, RoomChatRefusal, RoomChatState,
        RoomConversation, RoomMessage, RoomReactionMark,
    },
};
use kaeri_room::SendLimits;

struct Pending {
    id: String,
    text: String,
    at: i64,
    reaction: bool,
}

pub(crate) struct Chat {
    limits: SendLimits,
    lines: VecDeque<(String, RoomChatLine)>,
    marks: VecDeque<(String, RoomReactionMark)>,
    muted: BTreeSet<String>,
    pending: Vec<Pending>,
    failed: Option<String>,
    next_id: u64,
    next_send: u64,
    told: (u32, u32),
    hidden: bool,
}

impl Chat {
    pub fn new(hidden: bool) -> Self {
        Self {
            limits: SendLimits::default(),
            lines: VecDeque::new(),
            marks: VecDeque::new(),
            muted: BTreeSet::new(),
            pending: Vec::new(),
            failed: None,
            next_id: 1,
            next_send: 1,
            told: (0, 0),
            hidden,
        }
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    pub fn new_episode(&mut self) {
        self.marks.clear();
    }

    fn secs(now: i64) -> f64 {
        now as f64 / 1000.0
    }

    fn push(&mut self, mid: &str, from: &str, you: bool, body: RoomChatBody) {
        let line = RoomChatLine {
            id: self.next_id,
            from: from.to_owned(),
            you,
            body,
            muted: self.muted.contains(mid),
        };
        self.next_id += 1;
        self.lines.push_back((mid.to_owned(), line));
        while self.lines.len() > CONVERSATION_LINES {
            self.lines.pop_front();
        }
    }

    /// A line the room says.
    pub fn room_line(&mut self, message: RoomMessage) {
        self.push("", "", false, RoomChatBody::Room { message });
    }

    /// An id and the text to send, or why not. `Ok(None)` for nothing to send.
    pub fn say(
        &mut self,
        text: &str,
        now: i64,
    ) -> Result<Option<(String, String)>, RoomChatRefusal> {
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let text: String = text.chars().take(CHAT_MAX_LENGTH as usize).collect();
        if text.is_empty() {
            return Ok(None);
        }
        self.limits.take_message(Self::secs(now)).map_err(refusal)?;
        self.failed = None;
        let id = format!("s{}", self.next_send);
        self.next_send += 1;
        self.pending.push(Pending {
            id: id.clone(),
            text: text.clone(),
            at: now,
            reaction: false,
        });
        Ok(Some((id, text)))
    }

    /// An id for a reaction at `at`, or why not.
    pub fn react(
        &mut self,
        reaction: Reaction,
        at: Option<f64>,
        now: i64,
    ) -> Result<(String, f64), RoomChatRefusal> {
        let Some(at) = at else { return Err(RoomChatRefusal::NotPlaying) };
        self.limits.take_reaction(Self::secs(now)).map_err(refusal)?;
        let id = format!("s{}", self.next_send);
        self.next_send += 1;
        self.pending.push(Pending {
            id: id.clone(),
            text: String::new(),
            at: now,
            reaction: true,
        });
        let _ = reaction;
        Ok((id, at))
    }

    /// This device's own send came back from a relay: show it.
    pub fn echoed(&mut self, id: &str, me: &str) {
        let Some(at) = self.pending.iter().position(|p| p.id == id) else { return };
        let pending = self.pending.remove(at);
        if !pending.reaction {
            self.push("", me, true, RoomChatBody::Said { text: pending.text });
        }
    }

    pub fn heard_said(&mut self, mid: &str, _id: &str, from: &str, text: &str, _muted: bool) {
        let text: String = text.chars().take(CHAT_MAX_LENGTH as usize).collect();
        self.push(mid, from, false, RoomChatBody::Said { text });
    }

    pub fn heard_reaction(
        &mut self,
        mid: &str,
        _id: &str,
        from: &str,
        reaction: Reaction,
        at: f64,
        muted: bool,
    ) {
        // The same reaction at the same second as the last line joins it.
        let second = at.floor();
        if let Some((_, line)) = self.lines.back_mut()
            && let RoomChatBody::Reacted { reaction: r, at: a, with } = &mut line.body
            && *r == reaction
            && a.floor() == second
            && line.from != from
        {
            with.push(from.to_owned());
        } else {
            self.push(
                mid,
                from,
                false,
                RoomChatBody::Reacted { reaction, at, with: Vec::new() },
            );
        }
        if !muted {
            self.marks.push_back((
                mid.to_owned(),
                RoomReactionMark { at, from: from.to_owned(), reaction },
            ));
            while self.marks.len() > REACTION_MARKS {
                self.marks.pop_front();
            }
        }
    }

    pub fn mute(&mut self, mid: &str, muted: bool) {
        if muted {
            self.muted.insert(mid.to_owned());
        } else {
            self.muted.remove(mid);
        }
        for (who, line) in &mut self.lines {
            if who == mid {
                line.muted = muted;
            }
        }
    }

    /// Lost sends and running waits. `true` when the conversation changed.
    pub fn tick(&mut self, now: i64) -> bool {
        let limit = i64::from(ECHO_WAIT_SECONDS) * 1000;
        let mut changed = false;
        self.pending.retain(|p| {
            let lost = now - p.at > limit;
            if lost && !p.reaction {
                self.failed = Some(p.text.clone());
                changed = true;
            }
            !lost
        });
        changed
    }

    pub fn conversation(&self) -> RoomConversation {
        RoomConversation {
            lines: self.lines.iter().map(|(_, l)| l.clone()).collect(),
            marks: self
                .marks
                .iter()
                .filter(|(mid, _)| !self.muted.contains(mid))
                .map(|(_, m)| m.clone())
                .collect(),
            failed: self.failed.clone(),
        }
    }

    pub fn state(&mut self, now: i64) -> RoomChatState {
        let now = Self::secs(now);
        let waits = (
            self.limits.message_wait(now).ceil() as u32,
            self.limits.reaction_wait(now).ceil() as u32,
        );
        self.told = waits;
        RoomChatState {
            max_length: CHAT_MAX_LENGTH,
            muted: self.muted.iter().cloned().collect(),
            wait_seconds: waits.0,
            react_wait_seconds: waits.1,
            hidden: self.hidden,
        }
    }
}

fn refusal(wait: f64) -> RoomChatRefusal {
    RoomChatRefusal::Wait { seconds: wait.ceil().max(1.0) as u32 }
}
