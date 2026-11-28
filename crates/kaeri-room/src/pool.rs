//! The room's relays, all read and written: one connection each, dedupe by event id, the send
//! caps, health, the echo round trip, the clock check, the background flush and key epochs. The
//! engine sees a [`Link`]: bodies out, bodies in (the first copy of each, its own echo included).
//!
//! - **Dedupe:** the last 4096 event ids. The first copy is used, so delays come from the fastest
//!   relay.
//! - **Caps:** 20 events a minute and 6 in 10 s per relay. Over it, heartbeats are dropped first
//!   and anything else waits. `rate-limited:` stops heartbeats to that relay for 60 s and halves
//!   its cap for 10 minutes; changes still go to it, chat doesn't. A refused event is never sent
//!   again to the same relay.
//! - **Health:** the socket is open and our own events come back. Unhealthy after an event not
//!   echoed within 20 s (the timer runs only while one is awaited), a socket that failed 3 times
//!   running, three of our events refused running (but for `rate-limited:` and `duplicate:`;
//!   until it takes one again), or `blocked:`/`restricted:`. An `invalid:` about time runs the
//!   clock check again.
//! - **Subscription:** `{"kinds":[23517], "#z":[tags], "since": now − 30}`, today's tag and the
//!   other day's near midnight, and the previous epoch's for two minutes after a rekey. A relay
//!   swap asks only the new relay: a kept one asked again sends the last 30 s over again.
//! - **Closing:** what was said last goes out, and each relay answers or echoes it (2 s at
//!   most) before its socket closes.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use nostr::event::Event;
use serde_json::json;
use tokio::sync::mpsc;

use crate::{
    clock,
    envelope::{self, Gate, KIND},
    keys::{RoomKeys, Session},
    link::{Heard, Link, Out, RelayState, Unreachable},
    relay::{self, FromRelay, RelayHandle, ToRelay},
    wire::{Body, Message},
};

/// Event ids remembered for dedupe.
pub const DEDUPE: usize = 4096;
/// Per relay: events a minute, and a burst in 10 s.
pub const CAP_MINUTE: usize = 20;
pub const CAP_BURST: usize = 6;
/// Our own event not back within this: the relay is unhealthy.
pub const ECHO_LIMIT_MS: i64 = 20_000;
/// A socket failing this many times running is unhealthy.
pub const FAILURES_UNHEALTHY: u32 = 3;
/// A relay refusing this many of our events running, for any reason but `rate-limited:` or
/// `duplicate:`, is unhealthy until it takes one again: a web of trust, PoW or payment the room's
/// fresh session keys never meet.
pub const REFUSED_UNHEALTHY: u32 = 3;
/// After `rate-limited:`: no heartbeats for this long, half the cap for the longer.
pub const RATE_QUIET_MS: i64 = 60_000;
pub const RATE_HALVED_MS: i64 = 600_000;
/// The old epoch is still heard this long after a rekey.
pub const OLD_EPOCH_MS: i64 = 120_000;
/// The background flush waits for one relay's OK at most this long.
pub const FLUSH_LIMIT_MS: i64 = 2_000;
/// A join waits this long for any relay.
pub const JOIN_WAIT: Duration = Duration::from_secs(8);
/// The subscription asks for what the relay still holds from this far back, seconds.
pub const SINCE_S: i64 = 30;
/// What was said while no relay was up waits this long for one, ms.
pub const UNSENT_FOR_MS: i64 = 30_000;
/// And this many at most, the oldest giving way.
pub const UNSENT_CAP: usize = 32;

/// Events that reached no relay: none was up. They go to the first that comes up, unless they
/// are too old by then (a relay takes events only near its clock, and the room has moved on).
struct Unsent<T = Event>(VecDeque<(T, i64)>);

impl<T> Default for Unsent<T> {
    fn default() -> Self {
        Self(VecDeque::new())
    }
}

impl<T> Unsent<T> {
    fn push(&mut self, event: T, now: i64) {
        if self.0.len() == UNSENT_CAP {
            self.0.pop_front();
        }
        self.0.push_back((event, now));
    }

    /// Everything still fresh at `now`, oldest first; the queue is empty after.
    fn take(&mut self, now: i64) -> Vec<T> {
        self.0.drain(..).filter(|(_, at)| now - at < UNSENT_FOR_MS).map(|(e, _)| e).collect()
    }
}

/// The local clock, epoch ms. Tests can swap it.
pub type Now = Arc<dyn Fn() -> i64 + Send + Sync>;

fn system_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

struct Slot {
    /// This connection's own number: reports carry it, so a replaced relay's late ones are
    /// never taken for its successor's in the same place.
    id: usize,
    url: String,
    handle: RelayHandle,
    open: bool,
    failures: u32,
    /// Our events refused here running (see [`REFUSED_UNHEALTHY`]).
    refused: u32,
    unusable: bool,
    /// Our events awaiting their echo here: id → sent at, and the event, to send again should
    /// the socket go down first.
    awaiting: HashMap<String, (i64, Event)>,
    rtt: Option<u32>,
    /// Send times in the last minute.
    sent: VecDeque<i64>,
    quiet_until: i64,
    halved_until: i64,
    /// Waiting for room under the cap, with when each was said. Kept over a reconnect (a
    /// refresh drops none of it), and let go when older than [`UNSENT_FOR_MS`].
    queue: VecDeque<(Event, i64)>,
}

impl Slot {
    fn healthy(&self, now: i64) -> bool {
        self.open
            && !self.unusable
            && self.failures < FAILURES_UNHEALTHY
            && self.refused < REFUSED_UNHEALTHY
            && !self.awaiting.values().any(|(at, _)| now - at > ECHO_LIMIT_MS)
    }

    fn room(&mut self, now: i64) -> bool {
        while self.sent.front().is_some_and(|t| now - t >= 60_000) {
            self.sent.pop_front();
        }
        let halve = now < self.halved_until;
        let (minute, burst) =
            if halve { (CAP_MINUTE / 2, CAP_BURST / 2) } else { (CAP_MINUTE, CAP_BURST) };
        let recent = self.sent.iter().filter(|t| now - **t < 10_000).count();
        self.sent.len() < minute && recent < burst
    }

    fn put(&mut self, event: Event, now: i64) {
        self.sent.push_back(now);
        self.awaiting.insert(event.id.to_hex(), (now, event.clone()));
        self.handle.send(ToRelay::Send(Box::new(event)));
    }
}

struct Pool {
    now: Now,
    unsent: Unsent<Event>,
    offset: Arc<AtomicI64>,
    keys: RoomKeys,
    old: Option<(RoomKeys, i64)>,
    session: Session,
    gate: Gate,
    slots: Vec<Slot>,
    /// The next connection's number.
    next_id: usize,
    reports: mpsc::UnboundedSender<(usize, FromRelay)>,
    heard: mpsc::UnboundedSender<Heard>,
    seen: HashSet<String>,
    seen_order: VecDeque<String>,
    flushing: Option<(HashSet<String>, i64)>,
    tags: Vec<String>,
    told_up: Option<u32>,
    told_relays: Vec<RelayState>,
    http: reqwest::Client,
    clock_tx: mpsc::UnboundedSender<Vec<i64>>,
}

/// Options for [`open_with`]. Tests swap the clock.
#[derive(Clone)]
pub struct Options {
    pub now: Now,
    /// Ask each relay's NIP-11 for its clock.
    pub check_clock: bool,
    /// Tried when none of the room's relays answers.
    pub spares: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self { now: Arc::new(system_now), check_clock: true, spares: Vec::new() }
    }
}

/// Connect to the room with `secret` on `relays`: answered once any relay is up, or
/// [`Unreachable`] after 8 s with none (and 8 s more on the spares).
pub async fn open(secret: [u8; 16], relays: Vec<String>) -> Result<Link, Unreachable> {
    open_with(secret, relays, Options::default()).await
}

pub async fn open_with(
    secret: [u8; 16],
    relays: Vec<String>,
    options: Options,
) -> Result<Link, Unreachable> {
    let session = Session::new();
    let (out_tx, out_rx) = mpsc::unbounded_channel();
    let (heard_tx, heard_rx) = mpsc::unbounded_channel();
    let (reports_tx, reports_rx) = mpsc::unbounded_channel();
    let (clock_tx, clock_rx) = mpsc::unbounded_channel();
    let mut pool = Pool::new(secret, session.clone(), &options, reports_tx, heard_tx, clock_tx);
    let offset = Arc::clone(&pool.offset);
    pool.set_relays(&relays);
    if options.check_clock {
        pool.check_clock();
    }
    // Wait for the first relay, then hand the link over.
    let mut reports_rx = reports_rx;
    let mut used = relays.clone();
    let first = wait_up(&mut pool, &mut reports_rx).await;
    if !first {
        let spares: Vec<String> =
            options.spares.iter().filter(|s| !relays.contains(s)).take(3).cloned().collect();
        if spares.is_empty() {
            return Err(Unreachable::NoRelay);
        }
        pool.set_relays(&spares);
        used = spares;
        if !wait_up(&mut pool, &mut reports_rx).await {
            return Err(Unreachable::NoRelay);
        }
    }
    let room_id = pool.keys.room_id();
    tokio::spawn(run(pool, out_rx, reports_rx, clock_rx));
    Ok(Link {
        out: out_tx,
        heard: heard_rx,
        relays: used,
        session: Arc::new(session),
        room_id,
        offset,
    })
}

async fn wait_up(
    pool: &mut Pool,
    reports: &mut mpsc::UnboundedReceiver<(usize, FromRelay)>,
) -> bool {
    let deadline = tokio::time::sleep(JOIN_WAIT);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            () = &mut deadline => return false,
            report = reports.recv() => match report {
                Some((id, report)) => {
                    let up = matches!(report, FromRelay::Up);
                    if pool.on_report(id, report) && up {
                        return true;
                    }
                }
                None => return false,
            },
        }
    }
}

async fn run(
    mut pool: Pool,
    mut out: mpsc::UnboundedReceiver<Out>,
    mut reports: mpsc::UnboundedReceiver<(usize, FromRelay)>,
    mut clock: mpsc::UnboundedReceiver<Vec<i64>>,
) {
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            o = out.recv() => match o {
                Some(o) => pool.on_out(o),
                None => break,
            },
            Some((id, report)) = reports.recv() => {
                pool.on_report(id, report);
            }
            Some(samples) = clock.recv() => {
                if let Some(off) = clock::offset(&samples) {
                    tracing::info!(offset_ms = off, me = %pool.session.pk8(), "room: clock corrected from the relays");
                    pool.offset.store(off, Ordering::Relaxed);
                    pool.subscribe();
                }
            }
            _ = tick.tick() => pool.on_tick(),
        }
        if pool.heard.is_closed() {
            break;
        }
    }
    // The room is closing: what it said last (a Leave, usually) is still read, then what waits
    // under the cap goes now.
    while let Ok(o) = out.try_recv() {
        pool.on_out(o);
    }
    let now = pool.local();
    for slot in &mut pool.slots {
        while let Some((event, _)) = slot.queue.pop_front() {
            slot.put(event, now);
        }
    }
    // Each relay answers it or echoes it before its socket closes, for up to the flush's limit:
    // a public relay takes an event a moment after it comes, and never one whose connection is
    // gone by then.
    let mut pending: HashSet<(usize, String)> = pool
        .slots
        .iter()
        .filter(|s| s.open && !s.unusable)
        .flat_map(|s| s.awaiting.keys().map(|id| (s.id, id.clone())))
        .collect();
    let limit = tokio::time::sleep(Duration::from_millis(FLUSH_LIMIT_MS as u64));
    tokio::pin!(limit);
    while !pending.is_empty() {
        tokio::select! {
            () = &mut limit => break,
            report = reports.recv() => {
                let Some((slot, report)) = report else { break };
                match &report {
                    FromRelay::Ok { id, .. } => { pending.remove(&(slot, id.clone())); }
                    FromRelay::Event(event) => { pending.remove(&(slot, event.id.to_hex())); }
                    FromRelay::Down { .. } | FromRelay::Unusable(_) => {
                        pending.retain(|(s, _)| *s != slot);
                    }
                    FromRelay::Up => {}
                }
                pool.on_report(slot, report);
            }
        }
    }
    // Dropping the slots stops their connections.
    pool.slots.clear();
}

impl Pool {
    fn new(
        secret: [u8; 16],
        session: Session,
        options: &Options,
        reports: mpsc::UnboundedSender<(usize, FromRelay)>,
        heard: mpsc::UnboundedSender<Heard>,
        clock_tx: mpsc::UnboundedSender<Vec<i64>>,
    ) -> Self {
        let keys = RoomKeys::new(secret);
        Self {
            now: Arc::clone(&options.now),
            offset: Arc::new(AtomicI64::new(0)),
            gate: Gate::new(keys.room_id()),
            keys,
            old: None,
            session,
            slots: Vec::new(),
            next_id: 0,
            reports,
            heard,
            seen: HashSet::new(),
            seen_order: VecDeque::new(),
            flushing: None,
            unsent: Unsent::default(),
            tags: Vec::new(),
            told_up: None,
            told_relays: Vec::new(),
            http: reqwest::Client::builder().use_rustls_tls().build().unwrap_or_default(),
            clock_tx,
        }
    }

    fn local(&self) -> i64 {
        (self.now)()
    }

    /// The corrected clock.
    fn corrected(&self) -> i64 {
        self.local() + self.offset.load(Ordering::Relaxed)
    }

    fn set_relays(&mut self, urls: &[String]) {
        // A relay keeps its connection in its place; a new one in a place gets a new number.
        let mut old: Vec<Option<Slot>> =
            std::mem::take(&mut self.slots).into_iter().map(Some).collect();
        let mut slots = Vec::with_capacity(urls.len());
        let mut fresh = HashSet::new();
        for (i, url) in urls.iter().enumerate() {
            let kept = old.get_mut(i).and_then(|s| s.take_if(|s| s.url == *url));
            slots.push(kept.unwrap_or_else(|| {
                let slot = self.slot(url);
                fresh.insert(slot.id);
                slot
            }));
        }
        self.slots = slots;
        // Only the new relays: a kept one asked again sends everything of the last 30 s again.
        self.subscribe_where(|slot| fresh.contains(&slot.id));
        self.tell_relays();
    }

    fn slot(&mut self, url: &str) -> Slot {
        let id = self.next_id;
        self.next_id += 1;
        Slot {
            id,
            url: url.to_owned(),
            handle: relay::spawn(
                id,
                url.to_owned(),
                self.session.clone(),
                self.reports.clone(),
            ),
            open: false,
            failures: 0,
            refused: 0,
            unusable: false,
            awaiting: HashMap::new(),
            rtt: None,
            sent: VecDeque::new(),
            quiet_until: 0,
            halved_until: 0,
            queue: VecDeque::new(),
        }
    }

    fn check_clock(&self) {
        let http = self.http.clone();
        let urls: Vec<String> = self.slots.iter().map(|s| s.url.clone()).collect();
        let now = Arc::clone(&self.now);
        let tx = self.clock_tx.clone();
        tokio::spawn(async move {
            let asks = urls.iter().map(|u| {
                let now = Arc::clone(&now);
                let http = http.clone();
                async move { clock::sample(&http, u, move || now()).await }
            });
            let samples: Vec<i64> =
                futures::future::join_all(asks).await.into_iter().flatten().collect();
            let _ = tx.send(samples);
        });
    }

    fn tags_now(&self) -> Vec<String> {
        let now = self.corrected();
        let mut tags = self.keys.tags_at(now);
        if let Some((old, until)) = &self.old
            && now < *until
        {
            tags.extend(old.tags_at(now));
        }
        tags
    }

    fn subscribe(&mut self) {
        self.subscribe_where(|_| true);
    }

    /// Subscribe the relays `which` picks with today's filter. A new tag list goes to every relay
    /// all the same.
    fn subscribe_where(&mut self, which: impl Fn(&Slot) -> bool) {
        let tags = self.tags_now();
        let changed = tags != self.tags;
        self.tags.clone_from(&tags);
        let since = self.corrected() / 1000 - SINCE_S;
        let filter = json!({"kinds": [KIND], "#z": tags, "since": since.max(0)});
        for slot in self.slots.iter().filter(|s| changed || which(s)) {
            slot.handle.send(ToRelay::Subscribe(filter.clone()));
        }
    }

    fn say(&self, heard: Heard) {
        let _ = self.heard.send(heard);
    }

    fn tell_relays(&mut self) {
        let now = self.local();
        let list: Vec<RelayState> = self
            .slots
            .iter()
            .map(|s| RelayState {
                url: s.url.clone(),
                healthy: s.healthy(now),
                rtt_ms: s.rtt.filter(|_| s.open),
            })
            .collect();
        if list != self.told_relays {
            self.told_relays.clone_from(&list);
            self.say(Heard::Relays(list));
        }
        let up = self.slots.iter().filter(|s| s.open && !s.unusable).count() as u32;
        if self.told_up != Some(up) {
            self.told_up = Some(up);
            self.say(Heard::Up { relays_up: up });
        }
    }

    fn send(&mut self, body: &Body) -> Vec<String> {
        let now = self.local();
        let event = envelope::build(&self.keys, &self.session, body, self.corrected());
        let heartbeat = matches!(body.m, Message::Heartbeat);
        let chat = matches!(body.m, Message::Chat { .. } | Message::React { .. });
        // Said again once a relay is up (Status and SyncReq), or worth nothing late: not kept.
        let keep =
            !matches!(body.m, Message::Heartbeat | Message::Status | Message::SyncReq { .. });
        if keep && !self.slots.iter().any(|s| s.open && !s.unusable) {
            tracing::debug!(target: relay::WIRE, me = %self.session.pk8(), id = %event.id.to_hex(), "kept: no relay up");
            self.unsent.push(event.clone(), now);
        }
        for slot in &mut self.slots {
            if !slot.open || slot.unusable {
                continue;
            }
            let quiet = now < slot.quiet_until;
            if (heartbeat || chat) && quiet {
                continue;
            }
            if slot.room(now) {
                slot.put(event.clone(), now);
            } else if !heartbeat {
                tracing::debug!(target: relay::WIRE, relay = %slot.url, me = %self.session.pk8(), id = %event.id.to_hex(), "held: over the cap");
                slot.queue.push_back((event.clone(), now));
            }
        }
        vec![event.id.to_hex()]
    }

    fn on_out(&mut self, out: Out) {
        match out {
            Out::Body(body) => {
                self.send(&body);
            }
            Out::Flush(body) => {
                let ids = self.send(&body);
                let until = self.local() + FLUSH_LIMIT_MS;
                self.flushing = Some((ids.into_iter().collect(), until));
            }
            Out::Rekey(secret) => {
                let old = std::mem::replace(&mut self.keys, RoomKeys::new(secret));
                self.old = Some((old, self.corrected() + OLD_EPOCH_MS));
                self.gate.set_room_id(self.keys.room_id());
                self.subscribe();
            }
            Out::Refresh => {
                for slot in &self.slots {
                    slot.handle.send(ToRelay::Refresh);
                }
            }
            Out::Relays(list) => self.set_relays(&list),
        }
    }

    fn remember(&mut self, id: &str) -> bool {
        if !self.seen.insert(id.to_owned()) {
            return false;
        }
        self.seen_order.push_back(id.to_owned());
        if self.seen_order.len() > DEDUPE
            && let Some(old) = self.seen_order.pop_front()
        {
            self.seen.remove(&old);
        }
        true
    }

    /// A relay connection's report; `false` when that connection was replaced already.
    fn on_report(&mut self, id: usize, report: FromRelay) -> bool {
        let now = self.local();
        let Some(slot) = self.slots.iter_mut().find(|s| s.id == id) else { return false };
        if matches!(report, FromRelay::Up | FromRelay::Down { .. }) {
            tracing::debug!(target: relay::WIRE, relay = %slot.url, me = %self.session.pk8(), report = ?report, "report");
        }
        match report {
            FromRelay::Up => {
                slot.open = true;
                slot.failures = 0;
                slot.awaiting.clear();
                for event in self.unsent.take(now) {
                    if slot.room(now) {
                        slot.put(event, now);
                    } else {
                        slot.queue.push_back((event, now));
                    }
                }
            }
            FromRelay::Down { failures } => {
                slot.open = false;
                slot.failures = failures;
                // What wasn't echoed may never have left (a refresh, a write that failed): it
                // goes again once the relay is back, ahead of what waits under the cap. A copy
                // that did arrive is heard once all the same: every event is deduped by its id.
                let mut again: Vec<(Event, i64)> = slot
                    .awaiting
                    .drain()
                    .filter(|(_, (at, _))| now - at < UNSENT_FOR_MS)
                    .map(|(_, (at, event))| (event, at))
                    .collect();
                again.sort_by_key(|(_, at)| *at);
                for item in again.into_iter().rev() {
                    slot.queue.push_front(item);
                }
            }
            FromRelay::Unusable(why) => {
                tracing::info!(relay = %slot.url, %why, "room: relay unusable for this room");
                slot.unusable = true;
                slot.open = false;
            }
            FromRelay::Ok { id, accepted, message } => {
                if accepted {
                    slot.refused = 0;
                    if self.flushing.as_ref().is_some_and(|(ids, _)| ids.contains(&id)) {
                        self.flushing = None;
                        self.say(Heard::Flushed);
                    }
                } else {
                    slot.awaiting.remove(&id);
                    if !message.starts_with("rate-limited:")
                        && !message.starts_with("duplicate:")
                    {
                        slot.refused += 1;
                    }
                    if message.starts_with("rate-limited:") {
                        slot.quiet_until = now + RATE_QUIET_MS;
                        slot.halved_until = now + RATE_HALVED_MS;
                    } else if message.starts_with("blocked:")
                        || message.starts_with("restricted:")
                    {
                        slot.unusable = true;
                    } else if message.starts_with("invalid:")
                        && ["time", "created_at", "old", "future"]
                            .iter()
                            .any(|w| message.contains(w))
                    {
                        self.check_clock();
                    }
                }
            }
            FromRelay::Event(event) => {
                let id = event.id.to_hex();
                if let Some((sent, _)) = slot.awaiting.remove(&id) {
                    slot.rtt = Some((now - sent).max(0) as u32);
                }
                if self.remember(&id) {
                    self.take(&event);
                }
            }
        }
        self.tell_relays();
        true
    }

    fn take(&mut self, event: &Event) {
        let now = self.corrected();
        let keys: Vec<&RoomKeys> = std::iter::once(&self.keys)
            .chain(self.old.as_ref().filter(|(_, until)| now < *until).map(|(k, _)| k))
            .collect();
        let mut body = match envelope::open(&keys, event, now) {
            Ok(body) => body,
            Err(why) => {
                tracing::debug!(?why, me = %self.session.pk8(), id = %event.id.to_hex(), "room: event dropped");
                return;
            }
        };
        if body.from != self.session.pubkey()
            && let Err(why) = self.gate.admit(&mut body, now)
        {
            tracing::debug!(?why, me = %self.session.pk8(), id = %event.id.to_hex(), "room: event dropped");
            return;
        }
        self.say(Heard::Body(Box::new(body)));
    }

    fn on_tick(&mut self) {
        let now = self.local();
        for slot in &mut self.slots {
            slot.queue.retain(|(_, at)| now - at < UNSENT_FOR_MS);
            while slot.open && !slot.queue.is_empty() && slot.room(now) {
                let (event, _) = slot.queue.pop_front().expect("not empty");
                slot.put(event, now);
            }
        }
        if let Some((_, until)) = &self.flushing
            && now >= *until
        {
            self.flushing = None;
            self.say(Heard::Flushed);
        }
        if self.old.as_ref().is_some_and(|(_, until)| self.corrected() >= *until) {
            self.old = None;
        }
        if self.tags_now() != self.tags {
            self.subscribe();
        }
        self.tell_relays();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_replaced_relays_late_reports_never_reach_the_one_in_its_place() {
        // Neon: reports named a relay by its place, so the old relay 0's last "unusable" marked
        // the spare that took its place.
        let (reports, _reports_rx) = mpsc::unbounded_channel();
        let (heard, _heard_rx) = mpsc::unbounded_channel();
        let (clock, _clock_rx) = mpsc::unbounded_channel();
        let options = Options { check_clock: false, ..Options::default() };
        let mut pool = Pool::new([5; 16], Session::new(), &options, reports, heard, clock);
        pool.set_relays(&["ws://127.0.0.1:9/old".into()]);
        let old = pool.slots[0].id;
        pool.set_relays(&["ws://127.0.0.1:9/spare".into()]);
        assert!(!pool.on_report(old, FromRelay::Unusable("restricted: no".into())));
        assert!(!pool.on_report(old, FromRelay::Up));
        assert!(!pool.slots[0].unusable && !pool.slots[0].open);
        let spare = pool.slots[0].id;
        assert!(pool.on_report(spare, FromRelay::Up));
        assert!(pool.slots[0].open);
    }

    #[test]
    fn what_waits_for_a_relay_is_kept_for_a_while_and_only_so_much() {
        let mut unsent = Unsent::default();
        for n in 0..UNSENT_CAP + 2 {
            unsent.push(n, n as i64 * 1_000);
        }
        // The oldest two gave way; of the rest, those older than the limit are let go.
        let now = 3_000 + UNSENT_FOR_MS;
        assert_eq!(unsent.take(now), (4..UNSENT_CAP + 2).collect::<Vec<_>>());
        assert!(unsent.take(now).is_empty(), "sent once");
    }
}
