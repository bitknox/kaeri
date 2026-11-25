//! An in-process room with scripted members, for tests and the apps' Fixtures builds: every state
//! on the board can be staged with no network. Testing builds only, never an app's release.
//!
//! Each scripted member runs the same [`Room`] as a real device and answers as one: it joins
//! with a Hello, answers sync requests, writes playheads, and gets ready for a wait on its own
//! unless it was told to load. A device joins the fake through [`crate::link::open`] with any of
//! [`FakeRoom::relays`]; any secret on those relays reaches a room, so New invite and Move to a new
//! room work too.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex, PoisonError, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::sync::mpsc;

use crate::{
    link::{Heard, Link, Out, RelayState},
    room::Room,
    wire::{
        Body, Cause, Ep, Message, Mid, MovePayload, MoveTo, Playhead, Reaction, StateCode,
        Status, Up, VERSION,
    },
};

/// The clock the fake and the device share, in epoch milliseconds.
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Net {
    /// Every fake relay url.
    relays: Vec<String>,
    /// Rooms by secret.
    buses: HashMap<[u8; 16], Arc<Bus>>,
    /// The relays the latest fake started, for new rooms.
    latest: Option<Vec<String>>,
    clock: Option<Clock>,
    /// Relays that answer nothing at all.
    dead: Vec<String>,
}

static NET: Mutex<Option<Net>> = Mutex::new(None);

fn net<T>(f: impl FnOnce(&mut Net) -> T) -> T {
    let mut net = NET.lock().unwrap_or_else(PoisonError::into_inner);
    let net = net.get_or_insert_with(|| Net {
        relays: Vec::new(),
        buses: HashMap::new(),
        latest: None,
        clock: None,
        dead: Vec::new(),
    });
    f(net)
}

/// The latest fake's relays, which new rooms use in place of the settings'.
pub(crate) fn relays() -> Option<Vec<String>> {
    net(|net| net.latest.clone())
}

/// A link to the room with `secret`, when `relays` names a fake relay.
pub(crate) fn open(
    secret: [u8; 16],
    relays: &[String],
) -> Option<Result<Link, crate::link::Unreachable>> {
    let bus = net(|net| {
        if !relays.iter().any(|r| net.relays.contains(r)) {
            return None;
        }
        if relays.iter().all(|r| net.dead.contains(r)) {
            return Some(None);
        }
        let clock = net.clock.clone()?;
        Some(Some(net.buses.entry(secret).or_insert_with(|| Bus::new(clock)).clone()))
    })?;
    Some(match bus {
        Some(bus) => Ok(bus.subscribe(relays.to_vec())),
        None => Err(crate::link::Unreachable::NoRelay),
    })
}

/// A random-enough secret for a fake room: no crypto needed here.
fn fresh_secret() -> [u8; 16] {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&crate::mix(&n.to_string(), &t.to_string()).to_le_bytes());
    out[8..].copy_from_slice(&crate::mix(&t.to_string(), "kaeri-fake").to_le_bytes());
    out
}

/// A member the fake plays.
struct Scripted {
    mid: Mid,
    session: String,
    room: Room,
    status: Status,
    status_at: i64,
    ctr: u64,
    /// Gets ready for a wait by itself.
    auto_ready: bool,
    /// Dropped: sends nothing.
    silent: bool,
}

struct Device {
    tx: mpsc::UnboundedSender<Heard>,
    mid: Option<Mid>,
    session: Option<String>,
}

#[derive(Default)]
struct BusState {
    devices: BTreeMap<u64, Device>,
    members: BTreeMap<String, Scripted>,
    swallow_chat: bool,
    relays: Vec<RelayState>,
    /// The room the last move went to, for a scripted member to go along.
    moved_to: Option<Arc<Bus>>,
}

struct Bus {
    state: Mutex<BusState>,
    clock: Clock,
}

impl Bus {
    fn new(clock: Clock) -> Arc<Self> {
        let bus = Arc::new(Self { state: Mutex::new(BusState::default()), clock });
        let weak = Arc::downgrade(&bus);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(heartbeats(weak));
        }
        bus
    }

    fn now(&self) -> i64 {
        (self.clock)()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BusState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn subscribe(self: &Arc<Self>, relays: Vec<String>) -> Link {
        let (tx, heard) = mpsc::unbounded_channel();
        let (out, mut rx) = mpsc::unbounded_channel::<Out>();
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        {
            let mut state = self.lock();
            if state.relays.is_empty() {
                state.relays = relays
                    .iter()
                    .map(|url| RelayState { url: url.clone(), healthy: true, rtt_ms: Some(40) })
                    .collect();
            }
            let _ = tx.send(Heard::Relays(state.relays.clone()));
            let up = state.relays.iter().filter(|r| r.healthy).count() as u32;
            let _ = tx.send(Heard::Up { relays_up: up });
            state.devices.insert(id, Device { tx, mid: None, session: None });
        }
        let bus = Arc::clone(self);
        let spawned = tokio::runtime::Handle::try_current().map(|handle| {
            handle.spawn(async move {
                while let Some(out) = rx.recv().await {
                    match out {
                        Out::Body(body) => bus.take_from_device(id, body),
                        Out::Flush(body) => {
                            bus.take_from_device(id, body);
                            if let Some(device) = bus.lock().devices.get(&id) {
                                let _ = device.tx.send(Heard::Flushed);
                            }
                        }
                        Out::Rekey(secret) => {
                            net(|net| net.buses.insert(secret, Arc::clone(&bus)));
                        }
                        Out::Refresh | Out::Relays(_) => {}
                    }
                }
                bus.lock().devices.remove(&id);
            })
        });
        drop(spawned);
        Link { out, heard, relays }
    }

    fn take_from_device(&self, id: u64, body: Body) {
        {
            let mut state = self.lock();
            if let Some(device) = state.devices.get_mut(&id) {
                device.mid = Some(body.mid.clone());
                device.session = Some(body.from.clone());
            }
            if state.swallow_chat && matches!(body.m, Message::Chat { .. }) {
                return;
            }
            if matches!(body.m, Message::Leave)
                && let Some(device) = state.devices.get_mut(&id)
            {
                device.mid = None;
            }
        }
        self.deliver(body);
    }

    /// Hand `body` to every device (its sender's echo included) and every scripted member, and
    /// whatever the members answer after it.
    fn deliver(&self, body: Body) {
        let mut queue = vec![body];
        while let Some(body) = queue.pop() {
            let now = self.now();
            let mut state = self.lock();
            for device in state.devices.values() {
                let _ = device.tx.send(Heard::Body(Box::new(body.clone())));
            }
            let mut answered_sync = false;
            let names: Vec<String> = state.members.keys().cloned().collect();
            for name in names {
                let member = state.members.get_mut(&name).expect("listed");
                if member.mid == body.mid || member.silent {
                    continue;
                }
                let happened = member.room.receive(body.clone(), now);
                for happening in happened {
                    match happening {
                        crate::Happening::SyncRequested { nonce, .. } if !answered_sync => {
                            answered_sync = true;
                            let answer = member.room.sync_answer(&nonce, now);
                            queue.push(member.body(answer, now));
                        }
                        crate::Happening::Playhead { old, new } => {
                            let (old, new) = (old.as_deref(), *new);
                            let new_wait = old.is_none_or(|o| o.wseq != new.wseq);
                            if new_wait && member.auto_ready && member.status.s.player_open() {
                                member.status.ep = Some(new.ep.clone());
                                member.status.rdy = Some(new.wseq);
                                member.status.s = StateCode::Watching;
                                member.status.play = new.play;
                                member.touch(now);
                                queue.push(member.body(Message::Status, now));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn device_mid(&self, name_or_you: &str) -> Option<(Mid, String)> {
        let state = self.lock();
        if name_or_you == YOU {
            return state
                .devices
                .values()
                .find_map(|d| Some((d.mid.clone()?, d.session.clone().unwrap_or_default())));
        }
        state.members.get(name_or_you).map(|m| (m.mid.clone(), m.session.clone()))
    }
}

/// Every 10 s each scripted member that isn't dropped says it's there.
async fn heartbeats(bus: Weak<Bus>) {
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let Some(bus) = bus.upgrade() else { return };
        let now = bus.now();
        let bodies: Vec<Body> = {
            let mut state = bus.lock();
            state
                .members
                .values_mut()
                .filter(|m| !m.silent)
                .map(|m| m.body(Message::Heartbeat, now))
                .collect()
        };
        for body in bodies {
            bus.deliver(body);
        }
    }
}

impl Scripted {
    fn body(&mut self, m: Message, now: i64) -> Body {
        self.ctr += 1;
        let mut st = self.status.clone();
        st.age = (now - self.status_at).max(0) as u32;
        st.dg = [self.room.seq(), self.room.relays_seq(), 0];
        Body {
            v: VERSION,
            from: self.session.clone(),
            mid: self.mid.clone(),
            ctr: self.ctr,
            t: now,
            st,
            m,
        }
    }

    fn touch(&mut self, now: i64) {
        self.status_at = now;
        self.room.set_status(self.status.clone(), now);
    }

    /// Ready for the room's current wait, watching its episode.
    fn watch(&mut self, now: i64) {
        let head = self.room.playhead().cloned();
        self.status.s = StateCode::Watching;
        self.status.ep = head.as_ref().map(|h| h.ep.clone());
        self.status.rdy = head.as_ref().map(|h| h.wseq);
        self.status.play = head.as_ref().is_some_and(|h| h.play);
        self.status.pos = self.room.position(now);
        self.status.rejoin = false;
        self.status.dw = false;
        self.status.bg = false;
        self.status.hb = 15;
        self.touch(now);
    }
}

/// The name the device goes by in [`FakeRoom`]'s member lists: "you".
pub const YOU: &str = "you";

/// A room with scripted members. Cheap to hold; dropping it leaves the room as it is.
pub struct FakeRoom {
    secret: [u8; 16],
    bus: Arc<Bus>,
    relays: Vec<String>,
}

impl FakeRoom {
    /// A fresh room, empty, on three fake relays. New rooms this process starts go to the same
    /// fake relays from now on. Call inside a tokio runtime: the members' heartbeats run on it.
    pub fn start(clock: Clock) -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let relays: Vec<String> = ["one", "two", "three"]
            .iter()
            .map(|r| format!("ws://localhost/fake-{n}-relay-{r}.example"))
            .collect();
        let secret = fresh_secret();
        let bus = Bus::new(clock.clone());
        net(|net| {
            net.relays.extend(relays.iter().cloned());
            net.latest = Some(relays.clone());
            net.clock = Some(clock);
            net.buses.insert(secret, Arc::clone(&bus));
        });
        Self { secret, bus, relays }
    }

    pub fn secret(&self) -> [u8; 16] {
        self.secret
    }

    pub fn relays(&self) -> Vec<String> {
        self.relays.clone()
    }

    fn now(&self) -> i64 {
        self.bus.now()
    }

    /// Act as `name`, then send what it says.
    fn act(&self, name: &str, act: impl FnOnce(&mut Scripted, i64) -> Option<Message>) {
        let now = self.now();
        let body = {
            let mut state = self.bus.lock();
            let Some(member) = state.members.get_mut(name) else { return };
            let message = act(member, now);
            message.map(|m| member.body(m, now))
        };
        if let Some(body) = body {
            self.bus.deliver(body);
        }
    }

    /// `name` comes into the room, watching what it watches.
    pub fn add_member(&self, name: &str) {
        let now = self.now();
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let mid = format!("{:016x}", crate::mix(name, &n.to_string()));
        let session = format!("{:064x}", u128::from(crate::mix(&mid, "session")));
        let mut room = Room::new(&mid, name, &session, now);
        // It knows the room as the others do.
        let answer = {
            let state = self.bus.lock();
            state.members.values().next().map(|m| m.room.sync_answer("fake", now))
        };
        if let Some(answer) = answer {
            let body = Body {
                v: VERSION,
                from: String::new(),
                mid: "fake-sync".into(),
                ctr: 0,
                t: now,
                st: Status::default(),
                m: answer,
            };
            room.receive(body, now);
        }
        let mut member = Scripted {
            mid,
            session,
            room,
            status: Status::default(),
            status_at: now,
            ctr: 0,
            auto_ready: true,
            silent: false,
        };
        member.watch(now);
        let hello = member.body(
            Message::Hello {
                name: name.to_owned(),
                app: "0.6".into(),
                caps: vec![],
                idpk: String::new(),
                idsig: String::new(),
            },
            now,
        );
        self.bus.lock().members.insert(name.to_owned(), member);
        self.bus.deliver(hello);
    }

    /// `name` starts the room on `ep`, playing or waiting for Play for everyone, with `next` as
    /// Up next. Add the member first.
    pub fn member_starts(&self, name: &str, ep: Ep, next: Option<Ep>, play: bool) {
        self.act(name, |m, now| {
            let head = m.room.write(
                now,
                now,
                |h| {
                    h.play = play;
                    h.next = next;
                    h.cause = Cause::Episode;
                },
                Some(ep),
            )?;
            m.watch(now);
            Some(Message::Playhead(head))
        });
        // Its own status, ready for the wait it started.
        self.act(name, |_, _| Some(Message::Status));
    }

    fn write(&self, name: &str, change: impl FnOnce(&mut Playhead)) {
        self.act(name, |m, now| m.room.write(now, now, change, None).map(Message::Playhead));
    }

    pub fn member_pauses(&self, name: &str) {
        self.write(name, |h| {
            h.play = false;
            h.cause = Cause::Pause;
        });
    }

    pub fn member_plays(&self, name: &str) {
        self.write(name, |h| {
            h.play = true;
            h.cause = Cause::Play;
        });
    }

    /// A seek to `to` seconds: the room waits for everyone to catch up.
    pub fn member_seeks(&self, name: &str, to: f64) {
        self.write(name, |h| {
            h.pos = to;
            h.cause = Cause::Seek;
        });
        self.act(name, |m, now| {
            m.watch(now);
            Some(Message::Status)
        });
    }

    pub fn member_skips_opening(&self, name: &str, to: f64) {
        self.write(name, |h| {
            h.pos = to;
            h.cause = Cause::SkipOpening;
        });
        self.act(name, |m, now| {
            m.watch(now);
            Some(Message::Status)
        });
    }

    /// Carry on without whoever the room waits for.
    pub fn member_carries_on(&self, name: &str) {
        self.act(name, |m, now| {
            let waited: Vec<Mid> = m.room.waited(now).into_iter().map(|w| w.mid).collect();
            let head = m.room.write(
                now,
                now,
                |h| {
                    h.play = true;
                    h.cause = Cause::CarryOn;
                    h.carry.extend(waited);
                },
                None,
            )?;
            Some(Message::Playhead(head))
        });
    }

    /// The room's episode changes to `ep`.
    pub fn member_changes_episode(&self, name: &str, ep: Ep, next: Option<Ep>) {
        self.act(name, |m, now| {
            let head = m.room.write(
                now,
                now,
                |h| {
                    h.next = next;
                    h.cause = Cause::Episode;
                    h.pos = 0.0;
                },
                Some(ep),
            )?;
            m.watch(now);
            Some(Message::Playhead(head))
        });
        self.act(name, |_, _| Some(Message::Status));
    }

    /// `name`'s own Up next point came; once everyone's has, the count starts.
    pub fn member_reaches_up_next(&self, name: &str) {
        self.act(name, |m, now| {
            m.status.ue = true;
            m.touch(now);
            Some(Message::Status)
        });
    }

    /// Up next counts from the room's place, for everyone.
    pub fn member_counts_up_next(&self, name: &str) {
        self.act(name, |m, now| {
            let from_pos = m.room.position(now).unwrap_or(0.0);
            let head = m.room.write(
                now,
                now,
                |h| {
                    h.cause = Cause::UpNext;
                    h.up = Up::Counting { from_pos };
                },
                None,
            )?;
            Some(Message::Playhead(head))
        });
    }

    pub fn member_cancels_up_next(&self, name: &str) {
        self.write(name, |h| {
            h.cause = Cause::UpNextCancel;
            h.up = Up::Cancelled;
        });
    }

    fn set(&self, name: &str, change: impl FnOnce(&mut Status)) {
        self.act(name, |m, now| {
            change(&mut m.status);
            m.touch(now);
            Some(Message::Status)
        });
    }

    /// `name` has been opening the episode for `seconds`: the room waits for it until
    /// [`FakeRoom::member_ready`].
    pub fn member_loads(&self, name: &str, seconds: f64) {
        self.act(name, |m, now| {
            m.auto_ready = false;
            let ep = m.room.playhead().map(|h| h.ep.clone());
            m.status.s = StateCode::Loading;
            m.status.ep = ep;
            m.status.rdy = None;
            m.touch(now);
            m.status_at = now - (seconds * 1000.0) as i64;
            Some(Message::Status)
        });
    }

    /// `name`'s player is ready: it watches with the room.
    pub fn member_ready(&self, name: &str) {
        self.act(name, |m, now| {
            m.auto_ready = true;
            m.watch(now);
            Some(Message::Status)
        });
    }

    /// `name` has stalled for `seconds` mid-episode. In a room of 5 or fewer it holds the room.
    pub fn member_stalls(&self, name: &str, seconds: f64) {
        self.act(name, |m, now| {
            m.auto_ready = false;
            m.status.s = StateCode::Buffering;
            m.touch(now);
            m.status_at = now - (seconds * 1000.0) as i64;
            Some(Message::Status)
        });
        let small = self
            .bus
            .lock()
            .members
            .get(name)
            .is_some_and(|m| m.room.stall_may_wait(self.now()));
        if small {
            self.write(name, |h| h.cause = Cause::Stall);
        }
    }

    /// `name` is `seconds` behind the room.
    pub fn member_behind(&self, name: &str, seconds: f64) {
        self.act(name, |m, now| {
            m.watch(now);
            m.status.s = StateCode::Behind;
            m.status.pos = m.room.position(now).map(|p| (p - seconds).max(0.0));
            m.touch(now);
            Some(Message::Status)
        });
    }

    /// `name` is watching with the room again.
    pub fn member_watches(&self, name: &str) {
        self.member_ready(name);
    }

    /// `name` has another episode open.
    pub fn member_on_episode(&self, name: &str, ep: Ep) {
        self.set(name, |s| {
            s.s = StateCode::Watching;
            s.ep = Some(ep);
        });
    }

    pub fn member_chooses_source(&self, name: &str) {
        self.set(name, |s| s.s = StateCode::Choosing);
    }

    pub fn member_has_no_source(&self, name: &str) {
        self.set(name, |s| s.s = StateCode::NoSource);
    }

    pub fn member_closes_player(&self, name: &str) {
        self.set(name, |s| s.s = StateCode::PlayerClosed);
    }

    /// `name`'s app went to the background: Player closed, heartbeats every 30 s.
    pub fn member_backgrounds(&self, name: &str) {
        self.set(name, |s| {
            s.s = StateCode::PlayerClosed;
            s.bg = true;
            s.hb = 30;
        });
    }

    /// `name`'s device stops answering. Devices read Reconnecting at once (the fake moves their
    /// last word 41 s back), and Left after 3 minutes.
    pub fn member_drops(&self, name: &str) {
        let mut state = self.bus.lock();
        let Some(m) = state.members.get_mut(name) else { return };
        m.silent = true;
        let mid = m.mid.clone();
        for device in state.devices.values() {
            let _ = device.tx.send(Heard::Backdate { mid: mid.clone(), ms: 41_000 });
        }
    }

    /// `name` comes back after a drop.
    pub fn member_returns(&self, name: &str) {
        if let Some(m) = self.bus.lock().members.get_mut(name) {
            m.silent = false;
        }
        self.act(name, |_, _| Some(Message::Status));
    }

    pub fn member_says(&self, name: &str, text: &str) {
        let id = format!("c{}", NEXT.fetch_add(1, Ordering::Relaxed));
        self.act(name, |_, _| Some(Message::Chat { id, text: text.to_owned() }));
    }

    pub fn member_reacts(&self, name: &str, reaction: Reaction, at: f64) {
        let id = format!("r{}", NEXT.fetch_add(1, Ordering::Relaxed));
        self.act(name, |_, _| Some(Message::React { id, reaction, at }));
    }

    pub fn member_leaves(&self, name: &str) {
        self.act(name, |_, _| Some(Message::Leave));
        self.bus.lock().members.remove(name);
    }

    /// `name` makes a new invite: the room's key rotates and everyone present moves with it.
    pub fn member_new_invite(&self, name: &str) {
        let secret = fresh_secret();
        net(|net| net.buses.insert(secret, Arc::clone(&self.bus)));
        self.act(name, |_, now| Some(Message::Rekey { seq: now as u64, to: Vec::new() }));
    }

    /// `name` moves the room to a new one, taking `take` (member names, or [`YOU`] for the
    /// device). Returns the offer's id.
    pub fn member_moves(&self, name: &str, take: &[&str]) -> String {
        let id = format!("m{}", NEXT.fetch_add(1, Ordering::Relaxed));
        let secret = fresh_secret();
        let to: Vec<MoveTo> = take
            .iter()
            .filter_map(|who| self.bus.device_mid(who))
            .map(|(mid, session)| {
                let payload = {
                    let state = self.bus.lock();
                    let now = self.now();
                    state.members.get(name).and_then(|m| {
                        let head = m.room.playhead()?;
                        Some(MovePayload {
                            secret: base64url(&secret),
                            relays: self.relays.clone(),
                            by: name.to_owned(),
                            ep: head.ep.clone(),
                            now_pos: m.room.position(now).unwrap_or(0.0),
                            playing: m.room.playing(),
                        })
                    })
                };
                MoveTo {
                    mid,
                    pk8: session.chars().take(16).collect(),
                    // Sealed pairwise in the transport; the fake sends it open.
                    sealed: payload
                        .map(|p| serde_json::to_string(&p).unwrap_or_default())
                        .unwrap_or_default(),
                }
            })
            .collect();
        let to_send = to.clone();
        let (ep, playing) = {
            let state = self.bus.lock();
            let m = state.members.get(name);
            (
                m.and_then(|m| m.room.playhead().map(|h| (h.ep.clone(), h.next.clone()))),
                m.is_some_and(|m| m.room.playing()),
            )
        };
        self.act(name, |_, _| Some(Message::Move { id: id.clone(), to: to_send }));
        // The mover goes first: an ordinary Leave here, and the new room's first member there.
        self.member_leaves(name);
        let clock = self.bus.clock.clone();
        let new_bus = Bus::new(clock);
        net(|net| net.buses.insert(secret, Arc::clone(&new_bus)));
        let new = FakeRoom { secret, bus: Arc::clone(&new_bus), relays: self.relays.clone() };
        new.add_member(name);
        if let Some((ep, next)) = ep {
            new.member_starts(name, ep, next, playing);
        }
        self.bus.lock().moved_to = Some(new_bus);
        id
    }

    /// `name`, ticked in a move, goes along (`go`) and leaves this room, or stays.
    pub fn member_answers_move(&self, name: &str, go: bool) {
        if go {
            self.member_leaves(name);
            let moved = self.bus.lock().moved_to.clone();
            if let Some(bus) = moved {
                let new = FakeRoom { secret: [0; 16], bus, relays: self.relays.clone() };
                new.add_member(name);
            }
        }
    }

    /// Relay `i` stops answering here, or answers again.
    pub fn relay_down(&self, i: usize, down: bool) {
        let mut state = self.bus.lock();
        if let Some(relay) = state.relays.get_mut(i) {
            relay.healthy = !down;
            relay.rtt_ms = if down { None } else { Some(40) };
        }
        let relays = state.relays.clone();
        let up = relays.iter().filter(|r| r.healthy).count() as u32;
        for device in state.devices.values() {
            let _ = device.tx.send(Heard::Relays(relays.clone()));
            let _ = device.tx.send(Heard::Up { relays_up: up });
        }
    }

    /// Relay `i` refuses this device's events with `reason` (`rate-limited:`, `blocked:`).
    pub fn relay_refuses(&self, i: usize, reason: &str) {
        let _ = reason;
        self.relay_down(i, true);
    }

    /// Every relay down: the device reads Reconnecting.
    pub fn all_relays_down(&self, down: bool) {
        for i in 0..3 {
            self.relay_down(i, down);
        }
    }

    /// None of this room's relays answers a device that joins from now on: "Couldn't reach this
    /// room's relays".
    pub fn unreachable(&self, dead: bool) {
        net(|net| {
            net.dead.retain(|r| !self.relays.contains(r));
            if dead {
                net.dead.extend(self.relays.iter().cloned());
            }
        });
    }

    /// The device's own player is in `s`, begun `seconds` ago, on `ep` (the room's when `None`),
    /// `behind` seconds behind: for scenes of the device's own states.
    pub fn pose_you(
        &self,
        s: StateCode,
        seconds: f64,
        ep: Option<Ep>,
        behind: Option<f64>,
        ue: bool,
    ) {
        let state = self.bus.lock();
        for device in state.devices.values() {
            let _ = device.tx.send(Heard::Pose {
                s,
                ms_ago: (seconds * 1000.0) as i64,
                ep: ep.clone(),
                behind,
                ue,
            });
        }
    }

    /// The room takes chat and sends it to no one: a send is never echoed.
    pub fn swallow_chat(&self, swallow: bool) {
        self.bus.lock().swallow_chat = swallow;
    }

    /// The device's member id, once it said Hello.
    pub fn you(&self) -> Option<Mid> {
        self.bus.device_mid(YOU).map(|(mid, _)| mid)
    }

    /// The scripted members' names.
    pub fn names(&self) -> Vec<String> {
        self.bus.lock().members.keys().cloned().collect()
    }

    /// Where the room is, as the first scripted member sees it.
    pub fn position(&self) -> Option<f64> {
        let now = self.now();
        self.bus.lock().members.values().next().and_then(|m| m.room.position(now))
    }

    /// The room plays, as the first scripted member sees it.
    pub fn playing(&self) -> bool {
        self.bus.lock().members.values().next().is_some_and(|m| m.room.playing())
    }

    /// Settle every member's room on the clock (the count before play, Up next).
    pub fn tick(&self) {
        let now = self.now();
        for m in self.bus.lock().members.values_mut() {
            m.room.settle(now);
        }
    }
}

fn base64url(bytes: &[u8]) -> String {
    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(char::from(B64[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    out
}

/// A Move payload as the fake sends it: open JSON.
pub fn open_move(sealed: &str) -> Option<MovePayload> {
    serde_json::from_str(sealed).ok()
}
