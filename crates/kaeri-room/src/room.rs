//! The room as one device sees it: its members and their statuses, the playhead register, the
//! waits, and the timeouts. Pure and clocked by the caller, in milliseconds on this device's clock,
//! so every rule here is a table test.
//!
//! Waits aren't messages: every device works them out from the same statuses (`docs/room-protocol.md`,
//! "Waits"). The room plays when its intent is play and nobody it waits for is still loading.

use std::collections::BTreeMap;

use crate::wire::{
    Body, Cause, Ep, Message, Mid, Playhead, RosterEntry, StateCode, Status, Up,
};

/// Silence before someone reads Reconnecting, ms (2.5 heartbeats and 2.5 s).
pub const RECONNECTING_AFTER: i64 = 40_000;
/// The same for a device whose heartbeat is 30 s (in the background).
pub const RECONNECTING_AFTER_SLOW: i64 = 75_000;
/// Silence before someone reads Left, ms.
pub const LEFT_AFTER: i64 = 180_000;
/// The same when their last status said the app was in the background.
pub const LEFT_AFTER_BACKGROUND: i64 = 600_000;
/// How long a Left row stays before it goes.
pub const LEFT_SHOWN: i64 = 4_000;
/// A wait shows once it passes this.
pub const WAIT_SHOWN_AFTER: i64 = 1_000;
/// Carry on comes forward at this.
pub const CARRY_ON_AFTER: i64 = 20_000;
/// The count before the room's first play of an episode, once everyone is ready.
pub const START_COUNT: i64 = 3_000;
/// Up next counts this long, in room seconds.
pub const UP_NEXT_SECONDS: f64 = 10.0;
/// A seq this far above the current one is dropped.
pub const SEQ_JUMP_LIMIT: u64 = 1000;
/// Mid-episode stalls wait only in rooms this size or smaller.
pub const STALL_WAIT_LIMIT: usize = 5;

/// One person in the room, this device included.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub mid: Mid,
    pub name: String,
    /// Their session key, hex.
    pub session: String,
    pub status: Status,
    /// When their status arrived here.
    pub status_at: i64,
    /// When anything last arrived from them.
    pub heard_at: i64,
    pub joined_at: i64,
    /// Sent Leave, or silent past the Left timeout, at this moment.
    pub left_at: Option<i64>,
    /// Their identity key and its proof for `session`, passed on in sync answers.
    pub idpk: String,
    pub idsig: String,
}

impl Member {
    /// When their current state began, on this device's clock.
    pub fn since(&self) -> i64 {
        self.status_at - i64::from(self.status.age)
    }
}

/// Why the room waits for someone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Loading,
    Seeking,
    Buffering,
}

/// A member the room waits for.
#[derive(Debug, Clone, PartialEq)]
pub struct Waited {
    pub mid: Mid,
    pub reason: Reason,
    pub since: i64,
}

/// What a person's row says, before the app's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Word {
    Watching,
    Ready,
    Paused,
    Loading,
    Buffering,
    Behind,
    OtherEpisode,
    Choosing,
    NoSource,
    PlayerClosed,
    Reconnecting,
    Left,
}

/// Something that happened, for the engine to say.
#[derive(Debug, Clone, PartialEq)]
pub enum Happening {
    Joined {
        mid: Mid,
    },
    Left {
        mid: Mid,
    },
    Playhead {
        old: Option<Box<Playhead>>,
        new: Box<Playhead>,
    },
    Chat {
        mid: Mid,
        id: String,
        text: String,
    },
    React {
        mid: Mid,
        id: String,
        reaction: crate::wire::Reaction,
        at: f64,
    },
    SyncRequested {
        from: Mid,
        nonce: String,
        peek: bool,
    },
    Move {
        by: Mid,
        id: String,
        to: Vec<crate::wire::MoveTo>,
    },
    Relays {
        list: Vec<String>,
    },
    /// A rekey: `to` holds the new secret sealed to each member present at the press.
    Rekey {
        by: Mid,
        seq: u64,
        t: i64,
        session: String,
        to: Vec<(String, String)>,
    },
}

/// The room clock: running from `from` seconds at `start`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Run {
    start: i64,
    from: f64,
}

#[derive(Debug, Clone)]
pub struct Room {
    me: Mid,
    members: BTreeMap<Mid, Member>,
    playhead: Option<Playhead>,
    /// When the playhead took effect here.
    effect_at: i64,
    run: Option<Run>,
    /// Since when nothing holds a play intent; the start of the count before the first play.
    clear_at: Option<i64>,
    /// The room hasn't played this episode yet.
    before_play: bool,
    /// The cause and writer of the write that started the current wait.
    wait_cause: Option<(Cause, Mid)>,
    relays: Vec<String>,
    relays_seq: u64,
    /// The key epoch register: the winning rekey's `(seq, t, session)`.
    epoch: (u64, i64, String),
    /// Per room relay, since when more than half the present members have reported it unhealthy.
    relay_bad_since: Vec<Option<i64>>,
    /// Since when this device has had fewer than two healthy room relays.
    few_relays_since: Option<i64>,
}

/// A relay reported unhealthy by a majority this long is swapped out, ms.
pub const RELAY_SWAP_AFTER: i64 = 60_000;
/// The most relays a room uses after minority adds.
pub const RELAYS_MOST: usize = 4;

impl Room {
    /// A room with only this device in it.
    pub fn new(me: &str, name: &str, session: &str, now: i64) -> Self {
        let mut members = BTreeMap::new();
        members.insert(
            me.to_owned(),
            Member {
                mid: me.to_owned(),
                name: name.to_owned(),
                session: session.to_owned(),
                status: Status::default(),
                status_at: now,
                heard_at: now,
                joined_at: now,
                left_at: None,
                idpk: String::new(),
                idsig: String::new(),
            },
        );
        Self {
            me: me.to_owned(),
            members,
            playhead: None,
            effect_at: now,
            run: None,
            clear_at: None,
            before_play: false,
            wait_cause: None,
            relays: Vec::new(),
            relays_seq: 0,
            epoch: (0, 0, String::new()),
            relay_bad_since: Vec::new(),
            few_relays_since: None,
        }
    }

    /// The room's relays as they should be after the health reports, when they should change:
    /// a relay more than half the present members (this device included) have reported
    /// unhealthy for 60 s is replaced by the first spare not in the list. A device with fewer
    /// than two healthy room relays for 60 s may add the first spare, up to four, removing
    /// nothing. Members who haven't reported health yet (`ok == 0`) aren't counted. `None`:
    /// leave the list.
    pub fn relay_change(&mut self, now: i64, spares: &[String]) -> Option<Vec<String>> {
        let list = self.relays.clone();
        if list.is_empty() {
            return None;
        }
        self.relay_bad_since.resize(list.len(), None);
        let reports: Vec<u8> =
            self.present(now).into_iter().map(|m| m.status.ok).filter(|ok| *ok != 0).collect();
        let spare = spares.iter().find(|s| !list.contains(s)).cloned();
        for (i, since) in self.relay_bad_since.iter_mut().enumerate().take(8) {
            let bad = reports.iter().filter(|ok| *ok & (1 << i) == 0).count();
            if !reports.is_empty() && bad * 2 > reports.len() {
                let since = *since.get_or_insert(now);
                if now - since >= RELAY_SWAP_AFTER
                    && let Some(spare) = spare.clone()
                {
                    let mut swapped = list.clone();
                    swapped[i] = spare;
                    self.relay_bad_since.clear();
                    return Some(swapped);
                }
            } else {
                *since = None;
            }
        }
        let mine = self.my_status().ok;
        let healthy = (0..list.len().min(8)).filter(|i| mine & (1 << i) != 0).count();
        if mine != 0 && healthy < 2 && list.len() < RELAYS_MOST {
            let since = *self.few_relays_since.get_or_insert(now);
            if now - since >= RELAY_SWAP_AFTER
                && let Some(spare) = spare
            {
                self.few_relays_since = None;
                let mut added = list;
                added.push(spare);
                return Some(added);
            }
        } else {
            self.few_relays_since = None;
        }
        None
    }

    /// The key epoch: the winning rekey's `(seq, t, session)`.
    pub fn epoch(&self) -> &(u64, i64, String) {
        &self.epoch
    }

    /// A rekey `(seq, t, session)`, this device's or someone's: `true` when it wins the epoch
    /// register (a higher seq, then the lower `t`, then the lower key), and is now the epoch.
    pub fn take_epoch(&mut self, seq: u64, t: i64, session: &str) -> bool {
        use std::cmp::Reverse;
        let (cur_seq, cur_t, cur_pk) = &self.epoch;
        let wins = (seq, Reverse(t), Reverse(session))
            > (*cur_seq, Reverse(*cur_t), Reverse(cur_pk.as_str()))
            && seq <= cur_seq + SEQ_JUMP_LIMIT;
        if wins {
            self.epoch = (seq, t, session.to_owned());
        }
        wins
    }

    pub fn me(&self) -> &str {
        &self.me
    }

    pub fn member(&self, mid: &str) -> Option<&Member> {
        self.members.get(mid)
    }

    /// Everyone, in the order they joined, this device first.
    pub fn members(&self) -> Vec<&Member> {
        let mut list: Vec<&Member> = self.members.values().collect();
        list.sort_by_key(|m| (m.mid != self.me, m.joined_at, m.mid.clone()));
        list
    }

    pub fn playhead(&self) -> Option<&Playhead> {
        self.playhead.as_ref()
    }

    pub fn before_play(&self) -> bool {
        self.before_play
    }

    pub fn relays(&self) -> &[String] {
        &self.relays
    }

    /// The highest seq seen.
    pub fn seq(&self) -> u64 {
        self.playhead.as_ref().map_or(0, |p| p.seq)
    }

    /// A name nobody present has: `want`, or `want 2`, the lowest free number.
    pub fn free_name(&self, want: &str) -> String {
        let taken = |name: &str| {
            self.members
                .values()
                .any(|m| m.mid != self.me && m.left_at.is_none() && m.name == name)
        };
        if !taken(want) {
            return want.to_owned();
        }
        (2..).map(|n| format!("{want} {n}")).find(|n| !taken(n)).unwrap_or_default()
    }

    /// This device's identity proof, for the sync answers it gives.
    pub fn set_my_proof(&mut self, idpk: &str, idsig: &str) {
        if let Some(me) = self.members.get_mut(&self.me) {
            me.idpk = idpk.to_owned();
            me.idsig = idsig.to_owned();
        }
    }

    pub fn rename_me(&mut self, name: &str) {
        if let Some(me) = self.members.get_mut(&self.me) {
            me.name = name.to_owned();
        }
    }

    /// Members present now: not Left and not Reconnecting.
    pub fn present(&self, now: i64) -> Vec<&Member> {
        self.members().into_iter().filter(|m| self.is_present(m, now)).collect()
    }

    fn is_present(&self, m: &Member, now: i64) -> bool {
        m.left_at.is_none() && (m.mid == self.me || !self.silent(m, now))
    }

    /// No word from them long enough to read Reconnecting.
    fn silent(&self, m: &Member, now: i64) -> bool {
        let limit =
            if m.status.hb >= 30 { RECONNECTING_AFTER_SLOW } else { RECONNECTING_AFTER };
        now - m.heard_at > limit
    }

    /// This device's own status changed.
    pub fn set_status(&mut self, status: Status, now: i64) {
        if let Some(me) = self.members.get_mut(&self.me) {
            me.status = status;
            me.status_at = now;
            me.heard_at = now;
        }
        self.settle(now);
    }

    /// This device's own status, begun `ms_ago`.
    pub fn set_status_since(&mut self, status: Status, now: i64, ms_ago: i64) {
        self.set_status(status, now);
        if let Some(me) = self.members.get_mut(&self.me) {
            me.status_at = now - ms_ago;
        }
        self.settle(now);
    }

    pub fn my_status(&self) -> &Status {
        &self.members[&self.me].status
    }

    /// Whether a member takes part in waits.
    fn taking_part(&self, m: &Member, now: i64) -> bool {
        let Some(head) = &self.playhead else { return false };
        self.is_present(m, now)
            && m.status.s.player_open()
            && m.status.ep.as_ref() == Some(&head.ep)
            && !m.status.rejoin
            && !m.status.dw
    }

    /// Who the room waits for now, the first to start first.
    pub fn waited(&self, now: i64) -> Vec<Waited> {
        let Some(head) = &self.playhead else { return Vec::new() };
        // A room nobody asked to play waits for no one: before Play it is Not started yet, and
        // a paused room is someone's pause. The wait starts with Play.
        if head.wseq == 0 || !head.play {
            return Vec::new();
        }
        let stall_by = match &self.wait_cause {
            Some((Cause::Stall, by)) => Some(by.clone()),
            _ => None,
        };
        let seeking = matches!(self.wait_cause, Some((Cause::Seek | Cause::SkipOpening, _)));
        let mut out: Vec<Waited> = self
            .members()
            .into_iter()
            .filter(|m| self.taking_part(m, now))
            .filter(|m| m.status.rdy != Some(head.wseq) && !head.carry.contains(&m.mid))
            .filter(|m| stall_by.as_ref().is_none_or(|by| *by == m.mid))
            .map(|m| {
                // The member's own state first, as its row says it: someone opening the
                // episode after a seek is loading, not seeking.
                let reason = match m.status.s {
                    StateCode::Buffering => Reason::Buffering,
                    StateCode::Loading => Reason::Loading,
                    _ if seeking => Reason::Seeking,
                    _ => Reason::Loading,
                };
                let since =
                    if m.status.s == StateCode::Loading || m.status.s == StateCode::Buffering {
                        m.since()
                    } else {
                        self.effect_at
                    };
                Waited { mid: m.mid.clone(), reason, since }
            })
            .collect();
        out.sort_by_key(|w| w.since);
        out
    }

    /// The room plays now.
    pub fn playing(&self) -> bool {
        self.run.is_some()
    }

    /// Before play, the moment play starts once everyone is ready: "Starting in 3".
    pub fn starts_at(&self) -> Option<i64> {
        (self.before_play && self.run.is_none())
            .then_some(self.clear_at?)
            .map(|at| at + START_COUNT)
    }

    /// The room's position at `now`, in seconds.
    pub fn position(&self, now: i64) -> Option<f64> {
        let head = self.playhead.as_ref()?;
        Some(match self.run {
            Some(run) => run.from + (now - run.start).max(0) as f64 / 1000.0,
            None => head.pos,
        })
    }

    /// Work out whether the room plays, after any change and on every tick.
    pub fn settle(&mut self, now: i64) {
        self.expire(now);
        let Some(head) = &self.playhead else { return };
        let clear = head.play && self.waited(now).is_empty();
        if !clear {
            if self.run.is_some() {
                let pos = self.position(now);
                if let (Some(pos), Some(head)) = (pos, self.playhead.as_mut()) {
                    head.pos = pos;
                }
            }
            self.run = None;
            self.clear_at = None;
            return;
        }
        let clear_at = *self.clear_at.get_or_insert(now);
        if self.run.is_none() {
            let start = if self.before_play { clear_at + START_COUNT } else { clear_at };
            if now >= start {
                let from = head.pos;
                self.run = Some(Run { start: now, from });
                self.before_play = false;
            }
        }
    }

    /// Mark the silent as Left.
    fn expire(&mut self, now: i64) {
        for m in self.members.values_mut() {
            if m.mid == self.me || m.left_at.is_some() {
                continue;
            }
            let limit = if m.status.bg { LEFT_AFTER_BACKGROUND } else { LEFT_AFTER };
            if now - m.heard_at > limit {
                m.left_at = Some(m.heard_at + limit);
            }
        }
    }

    /// Members who went Left more than [`LEFT_SHOWN`] ago, to drop from the rows.
    pub fn shown(&self, m: &Member, now: i64) -> bool {
        m.left_at.is_none_or(|at| now - at < LEFT_SHOWN)
    }

    /// What `m`'s row says.
    pub fn word(&self, m: &Member, now: i64) -> Word {
        if m.left_at.is_some() {
            return Word::Left;
        }
        if m.mid != self.me && self.silent(m, now) {
            return Word::Reconnecting;
        }
        let head = self.playhead.as_ref();
        match m.status.s {
            StateCode::Loading => Word::Loading,
            StateCode::Buffering => Word::Buffering,
            StateCode::Choosing => Word::Choosing,
            StateCode::NoSource => Word::NoSource,
            StateCode::PlayerClosed => Word::PlayerClosed,
            StateCode::Watching | StateCode::Behind => {
                if head.is_some_and(|h| m.status.ep.as_ref() != Some(&h.ep)) {
                    Word::OtherEpisode
                } else if m.status.s == StateCode::Behind {
                    Word::Behind
                } else if self.before_play {
                    Word::Ready
                } else if head.is_some_and(|h| !h.play) {
                    Word::Paused
                } else {
                    Word::Watching
                }
            }
        }
    }

    /// The member paused the room: Paused it.
    pub fn paused_room(&self, m: &Member) -> bool {
        self.playhead
            .as_ref()
            .is_some_and(|h| !h.play && h.cause == Cause::Pause && h.by == m.mid)
    }

    /// Those carried on past who are still loading.
    pub fn carried_past(&self, now: i64) -> Vec<&Member> {
        let Some(head) = &self.playhead else { return Vec::new() };
        if head.cause != Cause::CarryOn {
            return Vec::new();
        }
        head.carry
            .iter()
            .filter_map(|mid| self.members.get(mid))
            .filter(|m| {
                self.is_present(m, now)
                    && matches!(m.status.s, StateCode::Loading | StateCode::Buffering)
            })
            .collect()
    }

    /// Who hasn't reached their own Up next point, while some have.
    pub fn up_next_waiting_for(&self, now: i64) -> Vec<&Member> {
        let taking: Vec<&Member> =
            self.members().into_iter().filter(|m| self.taking_part(m, now)).collect();
        if !taking.iter().any(|m| m.status.ue) {
            return Vec::new();
        }
        taking.into_iter().filter(|m| !m.status.ue).collect()
    }

    /// When Up next's count ends, while it counts and the room plays.
    pub fn up_next_ends_at(&self, now: i64) -> Option<i64> {
        let head = self.playhead.as_ref()?;
        let Up::Counting { from_pos } = head.up else { return None };
        let pos = self.position(now)?;
        let left = (from_pos + UP_NEXT_SECONDS - pos).max(0.0);
        Some(now + (left * 1000.0) as i64)
    }

    /// How many are in the room for the cap: everyone not Left.
    pub fn count(&self) -> usize {
        self.members.values().filter(|m| m.left_at.is_none()).count()
    }

    /// A new playhead from this device, applied here and returned to send. `t` is the corrected
    /// clock.
    pub fn write(
        &mut self,
        now: i64,
        t: i64,
        change: impl FnOnce(&mut Playhead),
        ep: Option<Ep>,
    ) -> Option<Playhead> {
        let session = self.members[&self.me].session.clone();
        let pos = self.position(now).unwrap_or(0.0);
        let mut head = match (&self.playhead, ep) {
            (Some(head), None) => head.clone(),
            (old, Some(ep)) => Playhead {
                seq: 0,
                t,
                pk: session.clone(),
                ep,
                next: None,
                pos: 0.0,
                play: old.as_ref().is_some_and(|h| h.play),
                cause: Cause::Episode,
                by: self.me.clone(),
                wseq: 0,
                carry: Vec::new(),
                up: Up::None,
            },
            (None, None) => return None,
        };
        head.pos = pos;
        head.seq = self.seq() + 1;
        head.t = t;
        head.pk = session;
        head.by = self.me.clone();
        change(&mut head);
        if head.cause.starts_wait() {
            head.wseq = head.seq;
            head.carry.clear();
        }
        self.apply(head.clone(), now);
        Some(head)
    }

    /// Apply a playhead from anyone. Returns the one it replaced, when it won.
    pub fn apply(&mut self, head: Playhead, now: i64) -> Option<Option<Playhead>> {
        if let Some(old) = &self.playhead
            && (head.seq > old.seq + SEQ_JUMP_LIMIT || !head.beats(old))
        {
            return None;
        }
        let pos_now = self.position(now);
        let old = self.playhead.replace(head.clone());
        self.effect_at = now;
        if head.cause.starts_wait() && old.as_ref().is_none_or(|o| o.wseq != head.wseq) {
            self.wait_cause = Some((head.cause, head.by.clone()));
        }
        if head.cause == Cause::Episode
            && old.as_ref().is_none_or(|o| o.ep != head.ep || o.wseq != head.wseq)
        {
            self.before_play = true;
        }
        if head.cause == Cause::CarryOn {
            // Start without them: no count.
            self.before_play = false;
        }
        let steady = matches!(head.cause, Cause::UpNext | Cause::UpNextCancel)
            && old.as_ref().is_some_and(|o| o.ep == head.ep && o.play == head.play);
        if steady {
            // The count changes nothing the room does: its clock runs on.
            if let (Some(pos), Some(h)) = (pos_now, self.playhead.as_mut()) {
                h.pos = pos;
            }
            if let Some(run) = self.run.as_mut() {
                run.start = now;
                run.from = pos_now.unwrap_or(run.from);
            }
        } else {
            self.run = None;
            self.clear_at = None;
        }
        self.settle(now);
        Some(old)
    }

    /// An event from someone else (or this device's own echo, which changes nothing).
    pub fn receive(&mut self, body: Body, now: i64) -> Vec<Happening> {
        let mut out = Vec::new();
        if body.mid == self.me {
            return out;
        }
        if let Message::SyncAns {
            playhead,
            roster,
            now_pos,
            playing_now,
            started,
            relays,
            relays_seq,
            ..
        } = &body.m
        {
            for entry in roster {
                self.take_roster(entry, now, &mut out);
            }
            if let Some(head) = playhead.clone() {
                let won = self.apply(head, now);
                if let (Some(old), Some(new)) = (won, self.playhead.clone()) {
                    out.push(Happening::Playhead {
                        old: old.map(Box::new),
                        new: Box::new(new),
                    });
                }
                if *started {
                    self.before_play = false;
                }
                if let Some(pos) = now_pos {
                    if let Some(h) = self.playhead.as_mut() {
                        h.pos = *pos;
                    }
                    if *playing_now {
                        self.before_play = false;
                        self.clear_at = Some(now);
                        self.run = Some(Run { start: now, from: *pos });
                    }
                }
            }
            if *relays_seq > self.relays_seq {
                self.relays_seq = *relays_seq;
                self.relays.clone_from(relays);
            }
        }
        let known = self.members.contains_key(&body.mid);
        match &body.m {
            Message::Hello { name, idpk, idsig, .. } => {
                let back = self.members.get(&body.mid).is_some_and(|m| m.left_at.is_some());
                let member = self.members.entry(body.mid.clone()).or_insert_with(|| Member {
                    mid: body.mid.clone(),
                    name: name.clone(),
                    session: body.from.clone(),
                    status: body.st.clone(),
                    status_at: now,
                    heard_at: now,
                    joined_at: now,
                    left_at: None,
                    idpk: String::new(),
                    idsig: String::new(),
                });
                member.name.clone_from(name);
                member.session.clone_from(&body.from);
                member.idpk.clone_from(idpk);
                member.idsig.clone_from(idsig);
                member.left_at = None;
                if !known || back {
                    out.push(Happening::Joined { mid: body.mid.clone() });
                }
            }
            _ if !known => {
                // Not bound yet: a SyncReq from a newcomer is still answered.
                if let Message::SyncReq { nonce, peek } = &body.m {
                    out.push(Happening::SyncRequested {
                        from: body.mid.clone(),
                        nonce: nonce.clone(),
                        peek: *peek,
                    });
                }
                return out;
            }
            _ => {}
        }
        let Some(member) = self.members.get_mut(&body.mid) else { return out };
        member.heard_at = now;
        if member.status != body.st {
            member.status = body.st.clone();
            member.status_at = now;
        }
        if member.left_at.is_some() && !matches!(body.m, Message::Leave) {
            member.left_at = None;
            out.push(Happening::Joined { mid: body.mid.clone() });
        }
        match body.m {
            Message::Leave => {
                if let Some(m) = self.members.get_mut(&body.mid) {
                    m.left_at = Some(now);
                }
                out.push(Happening::Left { mid: body.mid });
            }
            Message::Playhead(mut head) => {
                head.pk = body.from;
                if let Some(old) = self.apply(head, now) {
                    let new = self.playhead.clone().expect("just applied");
                    out.push(Happening::Playhead {
                        old: old.map(Box::new),
                        new: Box::new(new),
                    });
                }
            }
            Message::Chat { id, text } => out.push(Happening::Chat { mid: body.mid, id, text }),
            Message::React { id, reaction, at } => {
                out.push(Happening::React { mid: body.mid, id, reaction, at });
            }
            Message::SyncReq { nonce, peek } => {
                out.push(Happening::SyncRequested { from: body.mid, nonce, peek });
            }
            Message::Move { id, to } => out.push(Happening::Move { by: body.mid, id, to }),
            Message::Rekey { seq, to } => out.push(Happening::Rekey {
                by: body.mid,
                seq,
                t: body.t,
                session: body.from,
                to,
            }),
            Message::Relays { seq, list } if seq > self.relays_seq => {
                self.relays_seq = seq;
                self.relays.clone_from(&list);
                out.push(Happening::Relays { list });
            }
            _ => {}
        }
        self.settle(now);
        out
    }

    fn take_roster(&mut self, entry: &RosterEntry, now: i64, out: &mut Vec<Happening>) {
        if entry.mid == self.me || entry.gone {
            return;
        }
        let joined = now - entry.joined_ms_ago as i64;
        let known = self.members.contains_key(&entry.mid);
        let member = self.members.entry(entry.mid.clone()).or_insert_with(|| Member {
            mid: entry.mid.clone(),
            name: entry.name.clone(),
            session: entry.pk.clone(),
            status: entry.st.clone(),
            status_at: now,
            heard_at: now,
            joined_at: joined,
            left_at: None,
            idpk: entry.idpk.clone(),
            idsig: entry.idsig.clone(),
        });
        if !entry.pk.is_empty() {
            member.session.clone_from(&entry.pk);
            member.idpk.clone_from(&entry.idpk);
            member.idsig.clone_from(&entry.idsig);
        }
        member.status = entry.st.clone();
        member.status_at = now;
        member.heard_at = now;
        if !known {
            out.push(Happening::Joined { mid: entry.mid.clone() });
        }
    }

    /// The answer to a sync request, from this device.
    pub fn sync_answer(&self, nonce: &str, now: i64) -> Message {
        let roster = self
            .members()
            .into_iter()
            .filter(|m| m.left_at.is_none())
            .map(|m| RosterEntry {
                mid: m.mid.clone(),
                name: m.name.clone(),
                pk: m.session.clone(),
                idpk: m.idpk.clone(),
                idsig: m.idsig.clone(),
                st: Status { age: (now - m.since()).max(0) as u32, ..m.status.clone() },
                joined_ms_ago: (now - m.joined_at).max(0) as u64,
                gone: false,
            })
            .collect();
        Message::SyncAns {
            nonce: nonce.to_owned(),
            playhead: self.playhead.clone(),
            relays: self.relays.clone(),
            relays_seq: self.relays_seq,
            epoch: self.epoch.0,
            now_pos: self.position(now),
            playing_now: self.playing(),
            started: !self.before_play,
            roster,
        }
    }

    /// Where this device falls among those who answer a sync request: it answers after
    /// `rank × 400 ms`, unless an answer came first.
    pub fn rank(&self, nonce: &str, now: i64) -> usize {
        let mut order: Vec<(u64, &str)> = self
            .present(now)
            .into_iter()
            .filter(|m| !m.status.bg)
            .map(|m| (crate::mix(nonce, &m.mid), m.mid.as_str()))
            .collect();
        order.sort();
        order.iter().position(|(_, mid)| *mid == self.me).unwrap_or(order.len())
    }

    /// Whether a stall here may start a wait: rooms of 5 or fewer.
    pub fn stall_may_wait(&self, now: i64) -> bool {
        self.present(now).len() <= STALL_WAIT_LIMIT
    }

    /// Treat `mid` as heard from `ms` earlier (the fake room's Reconnecting, for scenes).
    pub fn backdate(&mut self, mid: &str, ms: i64) {
        if let Some(m) = self.members.get_mut(mid) {
            m.heard_at -= ms;
        }
    }

    pub fn set_relays(&mut self, list: Vec<String>, seq: u64) {
        self.relays = list;
        self.relays_seq = seq;
    }

    pub fn relays_seq(&self) -> u64 {
        self.relays_seq
    }
}

#[cfg(test)]
mod tests;
