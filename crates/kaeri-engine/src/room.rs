//! The room: watching together with no host. One task beside the engine holds the room's link
//! (`kaeri_room::link`), its state (`kaeri_room::Room`), the seat playbacks take (the same
//! [`PartyHub`](crate::playback::seat::PartyHub) as before), and the conversation, and tells the
//! front end the room as it stands. Built with the `room` feature. See `docs/room-protocol.md`.

use std::{collections::HashMap, time::Duration};

use kaeri_core::ids::AnilistId;
use kaeri_meta::Meta;
use kaeri_model::{
    Msg,
    event::Event,
    room::{
        CARRY_ON_AFTER_MS, DEFAULT_RELAYS, MoveCandidate, OldInvite, PausedFor, Reaction,
        RelayHealth, RoomCarriedOn, RoomCommand, RoomConnection, RoomEpisode, RoomEpisodeInfo,
        RoomGlimpse, RoomLeftWhy, RoomMember, RoomMessage, RoomMessageKind, RoomMoveOffer,
        RoomMoved, RoomRelaySettings, RoomShare, RoomStartKind, RoomUpNext, RoomView,
        SPARE_RELAYS, WaitReason, base64url, unbase64url,
    },
};
use kaeri_room::{
    Happening, Reason, Room, Word,
    keys::{Identity, pk8},
    link::{self, Heard, Link, Out},
    wire::{self, Body, Cause, Ep, Message, Mid, MovePayload, MoveTo, StateCode, Status, Up},
};
use tokio::sync::mpsc;

use crate::{
    outbox::Outbox,
    playback::seat::{SeatCue, SeatMessage, SeatReport},
    room_chat::Chat,
    services::Services,
    state::EngineState,
};

/// How long a newcomer waits for a sync answer before it is the room.
const SYNC_WAIT: Duration = Duration::from_secs(3);
/// The heartbeat, and the faster one while this device is waited for.
const HEARTBEAT_MS: i64 = 15_000;
const HEARTBEAT_WAITED_MS: i64 = 5_000;
/// A Move offer lasts this long.
const OFFER_LIFE_MS: i64 = 600_000;
/// The app's version as a Hello says it.
const APP: &str = "0.6";

/// What the engine tells the room's task.
enum Control {
    Command(RoomCommand),
    /// The sources check for the room's episode found nothing here.
    NoSource,
    /// This device's own Up next point came: its player asked for the room's next episode.
    UpNextPoint,
}

/// The room's episode and the one after it, as the task last saw them.
type Episodes = std::sync::Arc<std::sync::Mutex<Option<(Ep, Option<Ep>)>>>;

/// The room going, as the engine steers it.
pub struct Running {
    control: mpsc::UnboundedSender<Control>,
    episodes: Episodes,
}

impl Running {
    fn going(&self) -> bool {
        !self.control.is_closed()
    }
}

fn going(engine: &EngineState) -> Option<&Running> {
    engine.room.as_ref().filter(|running| running.going())
}

/// The clock rooms run on: epoch milliseconds, or the stopped moment of a scene recording.
pub fn now_ms() -> i64 {
    if let Some(at) = crate::clock::stopped() {
        return at * 1000;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// A room command from the front end.
pub fn command(
    command: RoomCommand,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    match command {
        RoomCommand::Lobby { invite, name, pick, anyway } => {
            join(invite.as_deref(), name, pick, anyway, engine, services, tx);
        }
        RoomCommand::PeekInvite { invite } => peek(&invite, services, tx),
        RoomCommand::Rejoin { room_id } | RoomCommand::LookIn { room_id } => {
            // Recent rooms come with the store's table.
            let _ = tx.send(Event::RoomLookFailed {
                room_id,
                reason: Msg::verbatim("Recent rooms aren't kept yet."),
            });
        }
        RoomCommand::LoadRecent => {
            let _ = tx.send(Event::RoomRecent(Vec::new()));
        }
        RoomCommand::ForgetRoom { .. } => {}
        RoomCommand::LoadRelays => relays(engine, tx),
        RoomCommand::SetRelays { relays: list } => {
            engine.room_relays =
                list.into_iter().filter(|r| kaeri_model::room::valid_relay(r)).collect();
            relays(engine, tx);
        }
        RoomCommand::Leave => {
            if let Some(running) = going(engine) {
                let _ = running.control.send(Control::Command(RoomCommand::Leave));
            }
            engine.room = None;
        }
        other => {
            if let Some(running) = going(engine) {
                let _ = running.control.send(Control::Command(other));
            }
        }
    }
}

fn relays(engine: &EngineState, tx: &Outbox) {
    let _ = tx.send(Event::RoomRelays(RoomRelaySettings {
        defaults: DEFAULT_RELAYS.iter().map(|r| (*r).to_owned()).collect(),
        spares: SPARE_RELAYS.iter().map(|r| (*r).to_owned()).collect(),
        chosen: engine.room_relays.clone(),
    }));
}

/// The engine is going away, or another room takes this one's place.
pub fn leave(engine: &mut EngineState) {
    if let Some(running) = engine.room.take() {
        let _ = running.control.send(Control::Command(RoomCommand::Leave));
    }
}

/// A play while a room is going: the room's episode plays here (`false`), the room's next one is
/// this device's Up next point (the room's Up next moves everyone together, never one player
/// on its own), and any other becomes the room's episode. `false` with no room.
pub fn play_in_room(id: AnilistId, episode: &str, engine: &EngineState) -> bool {
    let Some(running) = going(engine) else { return false };
    let ep = Ep(id.get(), episode.to_owned());
    let (room, next) = running
        .episodes
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .unzip();
    // The room's own episode plays here: it's how the app answers RoomStarted.
    if room.as_ref() == Some(&ep) {
        return false;
    }
    let control = if next.flatten().as_ref() == Some(&ep) {
        Control::UpNextPoint
    } else {
        Control::Command(RoomCommand::ChangeEpisode { id, episode: episode.to_owned() })
    };
    running.control.send(control).is_ok()
}

/// A room is going on this device.
pub fn is_going(engine: &EngineState) -> bool {
    going(engine).is_some()
}

/// The room's episode has no source here: this device reads No source (the reason stays here),
/// unless the source picker is open for it, which it still reads as Choosing.
pub fn no_source(engine: &EngineState) {
    if let Some(running) = going(engine) {
        let _ = running.control.send(Control::NoSource);
    }
}

fn random_secret() -> [u8; 16] {
    rand::random()
}

fn random_hex(bytes: usize) -> String {
    (0..bytes).map(|_| format!("{:02x}", rand::random::<u8>())).collect()
}

/// Where a room is: its secret and relays, and what the invite said.
#[derive(Debug, Clone)]
struct Target {
    secret: [u8; 16],
    relays: Vec<String>,
    by: Option<String>,
    title: Option<String>,
}

fn target(invite: &str) -> Result<Target, RoomLeftWhy> {
    match RoomShare::parse(invite) {
        Ok(Some(share)) => {
            let relays = if share.relays.is_empty() {
                link::relays_for_new_room(&[], &DEFAULT_RELAYS)
            } else {
                share.relays
            };
            Ok(Target { secret: share.secret, relays, by: share.by, title: share.title })
        }
        Ok(None) => {
            Err(RoomLeftWhy::Failed { reason: Msg::verbatim("That isn't a room's invite.") })
        }
        Err(OldInvite::SyncplayLink | OldInvite::Code) => Err(RoomLeftWhy::OldInvite),
    }
}

/// How a room starts on this device.
#[derive(Debug, Clone)]
enum Start {
    /// Join and follow the room.
    Join { anyway: bool },
    /// A new room watching this episode.
    New(RoomEpisode),
    /// The first member of a moved room: same episode, from here, playing or not.
    Moved { ep: Ep, pos: f64, playing: bool },
}

fn join(
    invite: Option<&str>,
    name: Option<String>,
    pick: Option<RoomEpisode>,
    anyway: bool,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let (target, start) = match invite {
        Some(invite) => match target(invite) {
            Ok(target) => (target, Start::Join { anyway }),
            Err(why) => {
                let _ = tx.send(Event::RoomLeft { why });
                return;
            }
        },
        None => {
            let Some(pick) = pick.clone() else {
                let _ = tx.send(Event::RoomLeft {
                    why: RoomLeftWhy::Failed {
                        reason: Msg::verbatim("Choose an episode for the room first."),
                    },
                });
                return;
            };
            let relays = link::relays_for_new_room(&engine.room_relays, &DEFAULT_RELAYS);
            (
                Target { secret: random_secret(), relays, by: None, title: None },
                Start::New(pick),
            )
        }
    };
    #[cfg(feature = "party")]
    crate::party::leave(engine, tx);
    leave(engine);
    let name = name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| engine.config.effective().syncplay.name.clone());
    let name = if name.trim().is_empty() { "kaeri".to_owned() } else { name };
    let (control, controls) = mpsc::unbounded_channel();
    let seats = services.party.open();
    let env = Env {
        meta: services.meta.clone(),
        tx: tx.clone(),
        hidden: engine.config.effective().syncplay.hide_chat,
    };
    let episodes = Episodes::default();
    tokio::spawn(run(target, start, name, env, controls, seats, episodes.clone()));
    engine.room = Some(Running { control, episodes });
}

#[derive(Clone)]
struct Env {
    meta: Meta,
    tx: Outbox,
    hidden: bool,
}

/// Look into an invite's room without anyone seeing.
fn peek(invite: &str, services: &Services, tx: &Outbox) {
    let target = match target(invite) {
        Ok(target) => target,
        Err(why) => {
            let _ = tx.send(Event::RoomLeft { why });
            return;
        }
    };
    let tx = tx.clone();
    let meta = services.meta.clone();
    tokio::spawn(async move {
        let Ok(mut link) = link::open(target.secret, target.relays.clone()).await else {
            let _ = tx.send(Event::RoomLeft { why: RoomLeftWhy::Unreachable });
            return;
        };
        let now = now_ms();
        let me = random_hex(8);
        let session = link.session.pubkey();
        let mut room = Room::new(&me, "", &session, now);
        let nonce = random_hex(8);
        let body = Body {
            v: wire::VERSION,
            from: session.clone(),
            mid: me.clone(),
            ctr: 1,
            t: now,
            st: Status::default(),
            m: Message::SyncReq { nonce: nonce.clone(), peek: true },
        };
        let _ = link.out.send(Out::Body(body));
        let deadline = tokio::time::sleep(SYNC_WAIT);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                () = &mut deadline => break,
                heard = link.heard.recv() => match heard {
                    Some(Heard::Body(body)) if matches!(&body.m, Message::SyncAns { nonce: n, .. } if *n == nonce) => {
                        room.receive(*body, now_ms());
                        break;
                    }
                    Some(_) => {}
                    None => break,
                },
            }
        }
        let now = now_ms();
        let members: Vec<RoomMember> = room
            .members()
            .into_iter()
            .filter(|m| m.mid != me && m.left_at.is_none())
            .map(|m| member_of(&room, m, now, &HashMap::new(), false))
            .collect();
        let room_episode = match room.playhead() {
            Some(head) => Some(info(&meta, &head.ep, target.title.as_deref()).await),
            None => None,
        };
        let _ = tx.send(Event::RoomGlimpse(RoomGlimpse {
            room_id: None,
            by: target.by.clone(),
            full: members.len() as u32 >= kaeri_model::room::ROOM_CAP,
            members,
            room_episode,
            playing: room.playing(),
        }));
    });
}

async fn info(meta: &Meta, ep: &Ep, fallback: Option<&str>) -> RoomEpisodeInfo {
    let id = AnilistId::new(ep.0);
    let media = meta.media(id).await.ok();
    RoomEpisodeInfo {
        id,
        episode: ep.1.clone(),
        title: media
            .as_ref()
            .map(|m| m.title.display().to_owned())
            .or_else(|| fallback.map(str::to_owned))
            .unwrap_or_else(|| format!("Title {id}")),
        name: None,
        banner_url: media.as_ref().and_then(|m| m.banner_image.clone()),
        cover_url: media.as_ref().and_then(|m| m.cover_image.best().map(str::to_owned)),
    }
}

fn ep_of(e: &RoomEpisode) -> Ep {
    Ep(e.id.get(), e.episode.clone())
}

fn episode_of(ep: &Ep) -> RoomEpisode {
    RoomEpisode { id: AnilistId::new(ep.0), episode: ep.1.clone() }
}

/// The episode after `ep`, by number.
fn next_of(ep: &Ep) -> Option<Ep> {
    let n: u32 = ep.1.trim().parse().ok()?;
    Some(Ep(ep.0, (n + 1).to_string()))
}

fn word_state(word: Word) -> kaeri_model::room::MemberState {
    use kaeri_model::room::MemberState as S;
    match word {
        Word::Watching => S::Watching,
        Word::Ready => S::Ready,
        Word::Paused => S::Paused,
        Word::Loading => S::Loading,
        Word::Buffering => S::Buffering,
        Word::Behind => S::Behind,
        Word::OtherEpisode => S::OtherEpisode,
        Word::Choosing => S::Choosing,
        Word::NoSource => S::NoSource,
        Word::PlayerClosed => S::PlayerClosed,
        Word::Reconnecting => S::Reconnecting,
        Word::Left => S::Left,
    }
}

/// Local ms to epoch ms: the room's clock is the epoch already.
fn member_of(
    room: &Room,
    m: &kaeri_room::Member,
    now: i64,
    muted: &HashMap<Mid, bool>,
    waited: bool,
) -> RoomMember {
    let word = room.word(m, now);
    let behind = (word == Word::Behind)
        .then(|| {
            let pos = m.status.pos?;
            let room_pos = room.position(now)?;
            let moved = if m.status.play { (now - m.status_at) as f64 / 1000.0 } else { 0.0 };
            Some((room_pos - pos - moved).max(0.0) as f32)
        })
        .flatten();
    RoomMember {
        id: m.mid.clone(),
        name: m.name.clone(),
        you: m.mid == room.me(),
        state: word_state(word),
        since_ms: Some(m.since()),
        behind_s: behind,
        episode: (word == Word::OtherEpisode)
            .then(|| m.status.ep.as_ref().map(episode_of))
            .flatten(),
        paused_room: room.paused_room(m),
        waited,
        background: m.status.bg,
        muted: muted.get(&m.mid).copied().unwrap_or(false),
    }
}

fn reason(reason: Reason) -> WaitReason {
    match reason {
        Reason::Loading => WaitReason::Loading,
        Reason::Seeking => WaitReason::Seeking,
        Reason::Buffering => WaitReason::Buffering,
    }
}

/// A Move offer this device holds.
struct Offer {
    payload: MovePayload,
    expires: i64,
}

struct Task {
    env: Env,
    name: String,
    /// This device's identity key in this room: it gives `me`.
    identity: Identity,
    me: Mid,
    session: String,
    ctr: u64,
    target: Target,
    link: Link,
    room: Room,
    chat: Chat,
    muted: HashMap<Mid, bool>,
    /// Where cues go: the playback seated last.
    cues: Option<mpsc::UnboundedSender<SeatCue>>,
    /// What this device's player says it is doing.
    status: Status,
    status_at: i64,
    /// The room's episode, with its words.
    infos: HashMap<Ep, RoomEpisodeInfo>,
    pending_infos: Vec<tokio::sync::oneshot::Receiver<RoomEpisodeInfo>>,
    relays: Vec<RelayHealth>,
    connection: RoomConnection,
    last_sent: i64,
    /// What the player was last told, paused or not, and whether its samples caught up.
    pause: PlayerPause,
    /// The last wait said in the chat.
    said_wait: u64,
    told_view: Option<RoomView>,
    told_members: Vec<RoomMember>,
    offers: HashMap<String, Offer>,
    moved: Option<(Mid, Vec<Mid>)>,
    joined: bool,
    /// The room's episode and the next, for the engine's plays.
    episodes: Episodes,
}

impl Task {
    fn now(&self) -> i64 {
        now_ms()
    }

    fn say(&self, event: Event) {
        let _ = self.env.tx.send(event);
    }

    fn say_chat_state(&mut self) {
        let state = self.chat.state(now_ms());
        self.say(Event::RoomChatState(state));
    }

    fn name_of(&self, mid: &str) -> String {
        self.room.member(mid).map_or_else(|| "Someone".to_owned(), |m| m.name.clone())
    }

    fn who(&self, mid: &str) -> String {
        if mid == self.me { kaeri_i18n::t::room_you() } else { self.name_of(mid) }
    }

    /// [`Self::who`] inside a list of names, mid-sentence: "Ren and you".
    fn who_in_list(&self, mid: &str) -> String {
        if mid == self.me { kaeri_i18n::t::room_you_in_list() } else { self.name_of(mid) }
    }

    fn body(&mut self, m: Message) -> Body {
        let now = self.now();
        self.ctr += 1;
        self.last_sent = now;
        let mut st = self.status.clone();
        st.age = (now - self.status_at).max(0) as u32;
        st.dg = [self.room.seq(), self.room.relays_seq(), self.room.epoch().0];
        st.play = self.room.playing();
        st.pos = self.room.position(now).or(st.pos);
        Body {
            v: wire::VERSION,
            from: self.session.clone(),
            mid: self.me.clone(),
            ctr: self.ctr,
            t: self.link.corrected(now),
            st,
            m,
        }
    }

    fn send(&mut self, m: Message) {
        let body = self.body(m);
        let _ = self.link.out.send(Out::Body(body));
    }

    fn set_status(&mut self, change: impl FnOnce(&mut Status)) {
        let before = self.status.clone();
        change(&mut self.status);
        if self.status.s != before.s {
            self.status_at = self.now();
        }
        if self.status != before {
            let now = self.now();
            self.room.set_status(self.status.clone(), now);
            if self.joined {
                self.send(Message::Status);
            }
        }
    }

    fn write(&mut self, change: impl FnOnce(&mut Playhead), ep: Option<Ep>) {
        let now = self.now();
        let old = self.room.playhead().cloned();
        if let Some(head) = self.room.write(now, now, change, ep) {
            // Ready for a wait this device started, when its player has the episode open.
            if head.wseq == head.seq && self.status.s == StateCode::Watching {
                self.status.rdy = Some(head.wseq);
                self.room.set_status(self.status.clone(), now);
            }
            self.send(Message::Playhead(head.clone()));
            self.on_playhead(old, head);
        }
    }

    /// A playhead won: say it, and move the player.
    fn on_playhead(&mut self, old: Option<Playhead>, new: Playhead) {
        let by = self.who(&new.by);
        let at = new.pos;
        let kind = match new.cause {
            Cause::Pause => Some(RoomMessageKind::Paused { by, at }),
            Cause::Play => {
                old.as_ref().filter(|o| !o.play).map(|_| RoomMessageKind::Resumed { by })
            }
            Cause::Seek => Some(RoomMessageKind::Seeked {
                by,
                from: old.as_ref().map_or(0.0, |o| o.pos),
                to: new.pos,
            }),
            Cause::SkipOpening => Some(RoomMessageKind::SkippedOpening { by }),
            Cause::Episode if old.is_none() => {
                Some(RoomMessageKind::Started { by, episode: new.ep.1.clone() })
            }
            Cause::Episode | Cause::MoveOn => {
                Some(RoomMessageKind::EpisodeChanged { by, episode: new.ep.1.clone() })
            }
            Cause::CarryOn => {
                // Nobody carries on without themselves.
                let past = new
                    .carry
                    .iter()
                    .filter(|m| **m != new.by)
                    .map(|m| self.who_in_list(m))
                    .collect();
                Some(RoomMessageKind::CarriedOn { by, past })
            }
            Cause::UpNext => {
                new.next.as_ref().map(|n| RoomMessageKind::UpNext { episode: n.1.clone() })
            }
            Cause::UpNextCancel => Some(RoomMessageKind::UpNextCancelled { by }),
            Cause::Stall => None,
        };
        if let Some(kind) = kind {
            self.message(kind);
        }
        let new_episode = old.as_ref().is_none_or(|o| o.ep != new.ep);
        // A seek's wait: a player already on the episode is ready once it has moved there.
        // (The player's own Ready for the cue comes with the transport's seat changes.)
        let new_wait = old.as_ref().is_none_or(|o| o.wseq != new.wseq);
        if new_wait
            && !new_episode
            && self.status.s == StateCode::Watching
            && self.status.ep.as_ref() == Some(&new.ep)
            && self.status.rdy != Some(new.wseq)
        {
            let wseq = new.wseq;
            self.set_status(|s| s.rdy = Some(wseq));
        }
        if new_episode {
            self.look_up(new.ep.clone());
            if let Some(next) = &new.next {
                self.look_up(next.clone());
            }
            if old.is_some() || self.status.ep.as_ref() != Some(&new.ep) {
                let kind = if new.cause == Cause::MoveOn {
                    RoomStartKind::MoveOn
                } else {
                    RoomStartKind::Start
                };
                self.chat.new_episode();
                self.status.ue = false;
                self.say(Event::RoomStarted {
                    id: AnilistId::new(new.ep.0),
                    episode: new.ep.1.clone(),
                    kind,
                    at: None,
                });
            }
        } else if matches!(new.cause, Cause::Seek | Cause::SkipOpening | Cause::CarryOn) {
            self.cue(SeatCue::Seek(new.pos));
        }
        self.cue_play();
    }

    fn message(&mut self, kind: RoomMessageKind) {
        let message = RoomMessage { text: words(&kind), kind };
        self.cue(SeatCue::Notice(message.text.clone()));
        self.chat.room_line(message.clone());
        self.say(Event::RoomMessage(message));
        self.say(Event::RoomChat(self.chat.conversation()));
    }

    fn cue(&self, cue: SeatCue) {
        if let Some(cues) = &self.cues {
            let _ = cues.send(cue);
        }
    }

    /// Tell the player to play or pause with the room.
    fn cue_play(&mut self) {
        let paused = !self.room.playing();
        if self.pause.tell(paused, self.now()) {
            self.cue(SeatCue::Pause(paused));
        }
    }

    fn look_up(&mut self, ep: Ep) {
        if self.infos.contains_key(&ep) {
            return;
        }
        let fallback = self.target.title.clone();
        self.infos.insert(
            ep.clone(),
            RoomEpisodeInfo {
                id: AnilistId::new(ep.0),
                episode: ep.1.clone(),
                title: fallback.clone().unwrap_or_default(),
                name: None,
                banner_url: None,
                cover_url: None,
            },
        );
        let meta = self.env.meta.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = tx.send(info(&meta, &ep, fallback.as_deref()).await);
        });
        self.pending_infos.push(rx);
    }

    fn view(&self) -> RoomView {
        let now = self.now();
        let room = &self.room;
        let head = room.playhead();
        let waited = room.waited(now);
        let waiting: Vec<PausedFor> = waited
            .iter()
            .map(|w| PausedFor {
                id: w.mid.clone(),
                name: self.name_of(&w.mid),
                you: w.mid == self.me,
                reason: reason(w.reason),
                since_ms: w.since,
            })
            .collect();
        let oldest = waited.iter().map(|w| w.since).min();
        let present = room.present(now);
        let carried = room.carried_past(now);
        let up_next = head.and_then(|h| {
            let next = h.next.as_ref()?;
            let episode = self.infos.get(next).cloned()?;
            let waiting_for: Vec<String> =
                room.up_next_waiting_for(now).into_iter().map(|m| m.name.clone()).collect();
            match &h.up {
                Up::Counting { .. } => Some(RoomUpNext {
                    episode,
                    ends_at_ms: room.up_next_ends_at(now),
                    waiting_for: Vec::new(),
                    cancelled_by: None,
                }),
                Up::Cancelled => Some(RoomUpNext {
                    episode,
                    ends_at_ms: None,
                    waiting_for: Vec::new(),
                    cancelled_by: Some(self.who(&h.by)),
                }),
                Up::None if self.status.ue && !waiting_for.is_empty() => Some(RoomUpNext {
                    episode,
                    ends_at_ms: None,
                    waiting_for,
                    cancelled_by: None,
                }),
                Up::None => None,
            }
        });
        let share = RoomShare {
            secret: self.target.secret,
            relays: if self.target.relays.iter().map(String::as_str).eq(DEFAULT_RELAYS) {
                Vec::new()
            } else {
                self.target.relays.clone()
            },
            by: Some(self.name.clone()),
            title: head
                .and_then(|h| self.infos.get(&h.ep))
                .map(|i| i.title.clone())
                .filter(|t| !t.is_empty()),
            episode: head.map(|h| h.ep.1.clone()),
        };
        RoomView {
            room_id: base64url(&self.target.secret[..8]),
            name: self.name.clone(),
            connection: self.connection,
            episode: self.status.ep.as_ref().map(episode_of),
            room_episode: head.and_then(|h| self.infos.get(&h.ep).cloned()),
            next_episode: head
                .and_then(|h| h.next.as_ref())
                .and_then(|n| self.infos.get(n).cloned()),
            link: Some(share.https_link()),
            playing: room.playing(),
            before_play: room.before_play(),
            starts_at_ms: room.starts_at(),
            paused_by: head
                .filter(|h| !h.play && h.cause == Cause::Pause)
                .map(|h| self.name_of(&h.by)),
            paused_by_you: head
                .is_some_and(|h| !h.play && h.cause == Cause::Pause && h.by == self.me),
            position: room.position(now),
            position_at_ms: now,
            wait_shown_at_ms: oldest.map(|at| at + kaeri_room::room::WAIT_SHOWN_AFTER),
            carry_on_at_ms: oldest.map(|at| at + CARRY_ON_AFTER_MS),
            can_carry_on: oldest.is_some_and(|at| now - at >= CARRY_ON_AFTER_MS),
            can_dont_wait: waited.iter().any(|w| w.mid == self.me),
            waiting,
            carried_on: (!carried.is_empty()).then(|| {
                let by = head.map(|h| h.by.clone()).unwrap_or_default();
                RoomCarriedOn {
                    by: self.name_of(&by),
                    by_you: by == self.me,
                    past: carried.iter().map(|m| m.name.clone()).collect(),
                }
            }),
            up_next,
            sources: None,
            relays: self.relays.clone(),
            looking: !self.joined,
            move_candidates: present
                .iter()
                .filter(|m| m.mid != self.me)
                .map(|m| {
                    let muted = self.muted.get(&m.mid).copied().unwrap_or(false);
                    MoveCandidate {
                        id: m.mid.clone(),
                        name: m.name.clone(),
                        muted,
                        ticked: !muted,
                    }
                })
                .collect(),
            moved: self.moved.as_ref().map(|(by, went)| RoomMoved {
                by: self.name_of(by),
                went: went.iter().map(|m| self.name_of(m)).collect(),
            }),
            latency_ms: self.relays.iter().filter_map(|r| r.rtt_ms).min(),
        }
    }

    fn members(&self) -> Vec<RoomMember> {
        let now = self.now();
        let waited: Vec<Mid> = self.room.waited(now).into_iter().map(|w| w.mid).collect();
        self.room
            .members()
            .into_iter()
            .filter(|m| self.room.shown(m, now))
            .map(|m| member_of(&self.room, m, now, &self.muted, waited.contains(&m.mid)))
            .collect()
    }

    /// Tell the front end what changed.
    fn refresh(&mut self) {
        let now = self.room.playhead().map(|h| (h.ep.clone(), h.next.clone()));
        *self.episodes.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = now;
        let view = self.view();
        let mut compare = view.clone();
        compare.position_at_ms = 0;
        compare.position = compare.position.map(|p| p.floor());
        let told = self.told_view.as_ref().map(|v| {
            let mut v = v.clone();
            v.position_at_ms = 0;
            v.position = v.position.map(|p| p.floor());
            v
        });
        if told.as_ref() != Some(&compare)
            || told.as_ref().is_some_and(|t| t.playing != view.playing)
        {
            if self.told_view.is_none() {
                self.say(Event::RoomJoined(view.clone()));
            } else {
                self.say(Event::RoomState(view.clone()));
            }
            self.told_view = Some(view);
        }
        let members = self.members();
        if members != self.told_members {
            self.say(Event::RoomMembers(members.clone()));
            self.told_members = members;
        }
        self.cue_play();
    }

    fn tick(&mut self) {
        let now = self.now();
        self.room.settle(now);
        if self.joined {
            self.up_next(now);
        }
        // Waits worth saying in the chat: past 1 s.
        let waited = self.room.waited(now);
        let wseq = self.room.playhead().map_or(0, |h| h.wseq);
        if let Some(first) = waited.first()
            && now - first.since >= kaeri_room::room::WAIT_SHOWN_AFTER
            && self.said_wait != wseq
        {
            self.said_wait = wseq;
            let names = waited.iter().map(|w| self.who_in_list(&w.mid)).collect();
            self.message(RoomMessageKind::WaitingFor { names, reason: reason(first.reason) });
        }
        let period = if waited.iter().any(|w| w.mid == self.me) {
            HEARTBEAT_WAITED_MS
        } else {
            HEARTBEAT_MS
        };
        if self.joined && now - self.last_sent >= period {
            self.send(Message::Heartbeat);
        }
        if self.chat.tick(now) {
            self.say(Event::RoomChat(self.chat.conversation()));
        }
        self.offers.retain(|id, offer| {
            let keep = offer.expires > now;
            if !keep {
                let _ = self.env.tx.send(Event::RoomMoveOfferGone { id: id.clone() });
            }
            keep
        });
        self.refresh();
    }

    fn on_heard(&mut self, heard: Heard) {
        match heard {
            Heard::Body(body) => {
                if body.mid == self.me {
                    if let Message::Chat { id, .. } | Message::React { id, .. } = &body.m {
                        self.chat.echoed(id, &self.name.clone());
                        self.say(Event::RoomChat(self.chat.conversation()));
                    }
                    return;
                }
                let now = self.now();
                let mover = matches!(body.m, Message::Leave).then(|| body.mid.clone());
                for happening in self.room.receive(*body, now) {
                    self.on_happening(happening);
                }
                let _ = mover;
            }
            Heard::Up { relays_up } => {
                self.connection = if relays_up == 0 {
                    RoomConnection::Reconnecting { relays_up: 0 }
                } else if self.joined {
                    RoomConnection::Connected
                } else {
                    RoomConnection::Joining
                };
            }
            Heard::Relays(list) => {
                self.relays = list
                    .into_iter()
                    .map(|r| RelayHealth { url: r.url, healthy: r.healthy, rtt_ms: r.rtt_ms })
                    .collect();
            }
            Heard::Flushed => self.say(Event::RoomFlushed),
            Heard::Backdate { mid, ms } => self.room.backdate(&mid, ms),
            Heard::Pose { s, ms_ago, ep, behind, ue } => {
                let now = self.now();
                let head = self.room.playhead().cloned();
                let ep = ep.or_else(|| head.as_ref().map(|h| h.ep.clone()));
                let ready =
                    s == StateCode::Watching && head.as_ref().map(|h| &h.ep) == ep.as_ref();
                let pos = behind.and_then(|b| Some((self.room.position(now)? - b).max(0.0)));
                self.set_status(|st| {
                    st.s = s;
                    st.ep = ep;
                    st.rdy = if ready { head.as_ref().map(|h| h.wseq) } else { None };
                    st.rejoin = false;
                    st.dw = false;
                    st.ue = ue;
                    if pos.is_some() {
                        st.pos = pos;
                    }
                });
                self.status_at = now - ms_ago;
                self.room.set_status_since(self.status.clone(), now, ms_ago);
                if self.joined {
                    self.send(Message::Status);
                }
            }
        }
    }

    fn on_happening(&mut self, happening: Happening) {
        match happening {
            Happening::Joined { mid } => {
                if self.joined {
                    let name = self.name_of(&mid);
                    self.message(RoomMessageKind::Joined { name });
                }
            }
            Happening::Left { mid } => {
                let name = self.name_of(&mid);
                let went = self.moved.as_mut().is_some_and(|(by, went)| {
                    if *by == mid {
                        return false;
                    }
                    went.push(mid.clone());
                    true
                });
                if went {
                    self.message(RoomMessageKind::WentAlong { name });
                } else if self.moved.as_ref().is_some_and(|(by, _)| *by == mid) {
                    // Said as the move.
                } else {
                    self.message(RoomMessageKind::Left { name });
                }
            }
            Happening::Playhead { old, new } => {
                if new.cause == Cause::Play && old.as_ref().is_some_and(|o| o.seq == new.seq) {
                    let by = self.who(&new.by);
                    self.message(RoomMessageKind::AlreadyPaused { by });
                }
                self.on_playhead(old.map(|o| *o), *new);
            }
            Happening::Chat { mid, id, text } => {
                let from = self.name_of(&mid);
                let muted = self.muted.get(&mid).copied().unwrap_or(false);
                self.chat.heard_said(&mid, &id, &from, &text, muted);
                if !muted && !self.env.hidden {
                    self.cue(SeatCue::Chat {
                        who: from.clone(),
                        you: false,
                        text: text.clone(),
                        notice: format!("{from}: {text}"),
                    });
                }
                self.say(Event::RoomChat(self.chat.conversation()));
            }
            Happening::React { mid, id, reaction, at } => {
                let from = self.name_of(&mid);
                let muted = self.muted.get(&mid).copied().unwrap_or(false);
                let reaction = from_wire(reaction);
                self.chat.heard_reaction(&mid, &id, &from, reaction, at, muted);
                if !muted {
                    self.say(Event::RoomReaction(kaeri_model::room::RoomReaction {
                        from: from.clone(),
                        you: false,
                        reaction,
                        at,
                    }));
                    self.cue(SeatCue::Reaction { who: from, reaction });
                }
                self.say(Event::RoomChat(self.chat.conversation()));
            }
            Happening::SyncRequested { nonce, .. } => {
                let now = self.now();
                if self.joined && self.room.rank(&nonce, now) == 0 {
                    let answer = self.room.sync_answer(&nonce, now);
                    self.send(answer);
                }
            }
            Happening::Move { by, id, to } => {
                let now = self.now();
                self.moved = Some((by.clone(), Vec::new()));
                let name = self.name_of(&by);
                self.message(RoomMessageKind::MovedRoom { by: name.clone() });
                let from = self.room.member(&by).map(|m| m.session.clone()).unwrap_or_default();
                let mine = to.iter().find(|t| t.mid == self.me && t.pk8 == pk8(&self.session));
                if let Some(mine) = mine
                    && let Some(payload) =
                        self.link.session.open_from(&from, &mine.sealed).and_then(|plain| {
                            serde_json::from_slice::<MovePayload>(&plain).ok()
                        })
                {
                    let names = to
                        .iter()
                        .filter(|t| t.mid != self.me)
                        .map(|t| self.name_of(&t.mid))
                        .collect();
                    let title = self
                        .infos
                        .get(&payload.ep)
                        .map(|i| i.title.clone())
                        .unwrap_or_default();
                    let offer = RoomMoveOffer {
                        id: id.clone(),
                        by: name,
                        names,
                        title,
                        episode: payload.ep.1.clone(),
                        expires_at_ms: now + OFFER_LIFE_MS,
                    };
                    self.offers.insert(id, Offer { payload, expires: now + OFFER_LIFE_MS });
                    self.say(Event::RoomMoveOffer(offer));
                }
            }
            Happening::Relays { list } => {
                self.target.relays = list.clone();
                let _ = self.link.out.send(Out::Relays(list));
            }
            Happening::Rekey { by, seq, t, session, to } => {
                let me = pk8(&self.session);
                let Some(secret) = to
                    .iter()
                    .find(|(pk, _)| *pk == me)
                    .and_then(|(_, sealed)| self.link.session.open_from(&session, sealed))
                    .and_then(|plain| String::from_utf8(plain).ok())
                    .and_then(|text| unbase64url(&text))
                    .and_then(|bytes| <[u8; 16]>::try_from(bytes.as_slice()).ok())
                else {
                    // Not sealed to this session: this device was Reconnecting at the press.
                    return;
                };
                if self.room.take_epoch(seq, t, &session) {
                    let _ = self.link.out.send(Out::Rekey(secret));
                    self.target.secret = secret;
                    let by = self.who(&by);
                    self.message(RoomMessageKind::NewInvite { by });
                }
            }
        }
    }

    fn on_report(&mut self, report: SeatReport) {
        match report {
            SeatReport::Playing { id, episode, .. } => {
                let ep = Ep(id.get(), episode);
                let wseq = self.room.playhead().map(|h| h.wseq);
                let on_room = self.room.playhead().is_some_and(|h| h.ep == ep);
                self.set_status(|s| {
                    s.s = StateCode::Watching;
                    s.ep = Some(ep);
                    s.rdy = if on_room { wseq } else { None };
                    s.rejoin = false;
                    s.dw = false;
                });
                self.pause.forget();
                self.cue_play();
                if on_room
                    && self.room.playing()
                    && let Some(pos) = self.room.position(self.now())
                {
                    self.cue(SeatCue::Seek(pos));
                }
            }
            SeatReport::Sample { position, paused, .. } => {
                self.status.pos = Some(position);
                // The viewer pressed play or pause on the player; a sample from before the
                // player took the room's last cue is not a press.
                match self.pause.sample(paused, self.now()) {
                    Sampled::Same => {}
                    // The player never showed what it was told (its own file start unpaused it):
                    // tell it again, never take it as the viewer's.
                    Sampled::Again(paused) => self.cue(SeatCue::Pause(paused)),
                    Sampled::Pressed(true) => self.write(
                        |h| {
                            h.play = false;
                            h.cause = Cause::Pause;
                        },
                        None,
                    ),
                    Sampled::Pressed(false) => self.play(),
                }
            }
            SeatReport::Ended { finished, .. } => {
                if finished {
                    self.set_status(|s| s.ue = true);
                } else {
                    self.set_status(|s| s.s = StateCode::PlayerClosed);
                }
                self.cues = None;
            }
        }
    }

    /// Play: Play for everyone before play, Carry on during a wait, else play.
    fn play(&mut self) {
        let now = self.now();
        let waiting = !self.room.waited(now).is_empty();
        if self.room.playhead().is_some_and(|h| h.play) && waiting {
            self.carry_on();
        } else {
            self.write(
                |h| {
                    h.play = true;
                    h.cause = Cause::Play;
                },
                None,
            );
        }
    }

    /// The room's Up next, from this device: once everyone taking part has reached their own Up
    /// next point, the count starts from the room's place; when it runs out, the room moves on.
    /// Every device may write either; they agree on the tie.
    fn up_next(&mut self, now: i64) {
        let Some(head) = self.room.playhead().cloned() else { return };
        let Some(next) = head.next.clone() else { return };
        match head.up {
            Up::None => {
                let everyone = self.status.ue
                    && self.room.playing()
                    && self.room.up_next_waiting_for(now).is_empty();
                if everyone {
                    let from_pos = self.room.position(now).unwrap_or(head.pos);
                    self.write(
                        |h| {
                            h.cause = Cause::UpNext;
                            h.up = Up::Counting { from_pos };
                        },
                        None,
                    );
                }
            }
            Up::Counting { .. } => {
                if self.room.up_next_ends_at(now).is_some_and(|end| end <= now) {
                    let after = next_of(&next);
                    self.write(
                        |h| {
                            h.cause = Cause::MoveOn;
                            h.pos = 0.0;
                            h.play = true;
                            h.up = Up::None;
                            h.next = after;
                        },
                        Some(next),
                    );
                }
            }
            Up::Cancelled => {}
        }
    }

    /// Carry on without whoever the room waits for. This device never carries on past itself:
    /// when it is among them, that part is Don't wait for me.
    fn carry_on(&mut self) {
        let now = self.now();
        let waited: Vec<Mid> = self.room.waited(now).into_iter().map(|w| w.mid).collect();
        if waited.contains(&self.me) {
            self.set_status(|s| s.dw = true);
        }
        let others: Vec<Mid> = waited.into_iter().filter(|m| *m != self.me).collect();
        if others.is_empty() {
            return;
        }
        self.write(
            |h| {
                h.play = true;
                h.cause = Cause::CarryOn;
                h.carry.extend(others);
            },
            None,
        );
    }

    /// A command; `true` when the task is over.
    async fn on_command(&mut self, command: RoomCommand) -> bool {
        match command {
            RoomCommand::Leave => {
                self.send(Message::Leave);
                self.say(Event::RoomLeft { why: RoomLeftWhy::Left });
                return true;
            }
            RoomCommand::Play { play: true } => self.play(),
            RoomCommand::Play { play: false } => self.write(
                |h| {
                    h.play = false;
                    h.cause = Cause::Pause;
                },
                None,
            ),
            RoomCommand::CarryOn => self.carry_on(),
            RoomCommand::DontWait => self.set_status(|s| s.dw = true),
            RoomCommand::SkipOpening { to } => self.write(
                |h| {
                    h.pos = to;
                    h.cause = Cause::SkipOpening;
                },
                None,
            ),
            RoomCommand::CancelUpNext => self.write(
                |h| {
                    h.cause = Cause::UpNextCancel;
                    h.up = Up::Cancelled;
                },
                None,
            ),
            RoomCommand::PlayNextNow => {
                if let Some(next) = self.room.playhead().and_then(|h| h.next.clone()) {
                    let after = next_of(&next);
                    self.write(
                        |h| {
                            h.cause = Cause::MoveOn;
                            h.pos = 0.0;
                            h.play = true;
                            h.up = Up::None;
                            h.next = after;
                        },
                        Some(next),
                    );
                }
            }
            RoomCommand::ChangeEpisode { id, episode } => {
                let ep = Ep(id.get(), episode);
                let next = next_of(&ep);
                self.write(
                    |h| {
                        h.cause = Cause::Episode;
                        h.pos = 0.0;
                        h.next = next;
                        h.up = Up::None;
                    },
                    Some(ep),
                );
            }
            RoomCommand::ChoosingSource { open } => self.set_status(|s| {
                s.s = if open { StateCode::Choosing } else { StateCode::PlayerClosed };
            }),
            RoomCommand::ClosePlayer => {
                self.set_status(|s| s.s = StateCode::PlayerClosed);
                self.cues = None;
            }
            RoomCommand::OpenPlayer => {
                if let Some(head) = self.room.playhead().cloned() {
                    self.set_status(|s| {
                        s.s = StateCode::Loading;
                        s.ep = Some(head.ep.clone());
                        s.rejoin = true;
                    });
                    let at = self.room.position(self.now());
                    self.say(Event::RoomStarted {
                        id: AnilistId::new(head.ep.0),
                        episode: head.ep.1.clone(),
                        kind: RoomStartKind::Rejoin,
                        at,
                    });
                }
            }
            RoomCommand::NewInvite => {
                let secret = random_secret();
                let now = self.now();
                let plain = base64url(&secret);
                // The new secret, sealed to every member present now (their proved sessions).
                let to: Vec<(String, String)> = self
                    .room
                    .present(now)
                    .into_iter()
                    .filter(|m| m.mid != self.me && !m.session.is_empty())
                    .filter_map(|m| {
                        Some((
                            pk8(&m.session),
                            self.link.session.seal_to(&m.session, plain.as_bytes())?,
                        ))
                    })
                    .collect();
                let seq = self.room.epoch().0 + 1;
                let body = self.body(Message::Rekey { seq, to });
                self.room.take_epoch(seq, body.t, &self.session.clone());
                let _ = self.link.out.send(Out::Body(body));
                let _ = self.link.out.send(Out::Rekey(secret));
                self.target.secret = secret;
                self.message(RoomMessageKind::NewInvite { by: kaeri_i18n::t::room_you() });
            }
            RoomCommand::MoveRoom { take } => return self.move_room(&take).await,
            RoomCommand::AnswerMove { offer, go } => {
                let Some(offer_held) = self.offers.remove(&offer) else { return false };
                self.say(Event::RoomMoveOfferGone { id: offer });
                if go {
                    let Some(secret) = unbase64url(&offer_held.payload.secret)
                        .and_then(|s| <[u8; 16]>::try_from(s.as_slice()).ok())
                    else {
                        return false;
                    };
                    let target = Target {
                        secret,
                        relays: offer_held.payload.relays.clone(),
                        by: Some(offer_held.payload.by.clone()),
                        title: self.target.title.clone(),
                    };
                    return self.switch(target, Start::Join { anyway: true }).await;
                }
            }
            RoomCommand::EditRelays { relays } => {
                let list: Vec<String> =
                    relays.into_iter().filter(|r| kaeri_model::room::valid_relay(r)).collect();
                let seq = self.room.relays_seq() + 1;
                self.room.set_relays(list.clone(), seq);
                self.target.relays = list.clone();
                self.send(Message::Relays { seq, list: list.clone() });
                let _ = self.link.out.send(Out::Relays(list));
            }
            RoomCommand::Say { text } => {
                let now = self.now();
                match self.chat.say(&text, now) {
                    Ok(Some((id, text))) => {
                        self.send(Message::Chat { id, text });
                        self.say(Event::RoomChat(self.chat.conversation()));
                    }
                    Ok(None) => {}
                    Err(refusal) => self.say(Event::RoomChatRefused(refusal)),
                }
                self.say_chat_state();
            }
            RoomCommand::React { reaction } => {
                let now = self.now();
                let at = self.status.pos.filter(|_| self.status.s.player_open());
                match self.chat.react(reaction, at, now) {
                    Ok((id, at)) => {
                        self.send(Message::React { id, reaction: to_wire(reaction), at });
                        self.say(Event::RoomReaction(kaeri_model::room::RoomReaction {
                            from: self.name.clone(),
                            you: true,
                            reaction,
                            at,
                        }));
                    }
                    Err(refusal) => self.say(Event::RoomChatRefused(refusal)),
                }
                self.say_chat_state();
            }
            RoomCommand::Mute { id, muted } => {
                self.muted.insert(id.clone(), muted);
                self.chat.mute(&id, muted);
                self.say(Event::RoomChat(self.chat.conversation()));
                self.say_chat_state();
            }
            RoomCommand::HideChat { hidden } => {
                self.env.hidden = hidden;
                self.chat.set_hidden(hidden);
                if hidden {
                    self.cue(SeatCue::ChatCleared);
                }
                self.say_chat_state();
            }
            // Answered by the engine itself.
            RoomCommand::Lobby { .. }
            | RoomCommand::Rejoin { .. }
            | RoomCommand::LoadRelays
            | RoomCommand::SetRelays { .. }
            | RoomCommand::LoadRecent
            | RoomCommand::ForgetRoom { .. }
            | RoomCommand::LookIn { .. }
            | RoomCommand::PeekInvite { .. } => {}
        }
        false
    }

    /// Move to a new room with `take`: offers sealed to each, then this device goes first.
    async fn move_room(&mut self, take: &[String]) -> bool {
        let Some(head) = self.room.playhead().cloned() else { return false };
        let now = self.now();
        let secret = random_secret();
        let pos = self.room.position(now).unwrap_or(head.pos);
        let playing = self.room.playing();
        let payload = MovePayload {
            secret: base64url(&secret),
            relays: self.target.relays.clone(),
            by: self.name.clone(),
            ep: head.ep.clone(),
            now_pos: pos,
            playing,
        };
        let plain = serde_json::to_vec(&payload).unwrap_or_default();
        // Sealed to each ticked member's proved session key: nobody else can open it.
        let to: Vec<MoveTo> = take
            .iter()
            .filter_map(|mid| self.room.member(mid))
            .filter(|m| !m.session.is_empty() && m.mid != self.me)
            .filter_map(|m| {
                Some(MoveTo {
                    mid: m.mid.clone(),
                    pk8: pk8(&m.session),
                    sealed: self.link.session.seal_to(&m.session, &plain)?,
                })
            })
            .collect();
        if to.is_empty() {
            return false;
        }
        self.send(Message::Move { id: random_hex(8), to });
        let target = Target {
            secret,
            relays: self.target.relays.clone(),
            by: Some(self.name.clone()),
            title: self.target.title.clone(),
        };
        self.switch(target, Start::Moved { ep: head.ep, pos, playing }).await
    }

    /// Leave this room with an ordinary Leave and join `target`, keeping the player.
    async fn switch(&mut self, target: Target, start: Start) -> bool {
        self.send(Message::Leave);
        let link = match link::open(target.secret, target.relays.clone()).await {
            Ok(link) => link,
            Err(_) => {
                self.say(Event::RoomLeft { why: RoomLeftWhy::Unreachable });
                return true;
            }
        };
        self.say(Event::RoomLeft { why: RoomLeftWhy::Moved });
        let now = self.now();
        self.identity = Identity::new();
        self.me = self.identity.mid();
        self.session = link.session.pubkey();
        self.ctr = 0;
        self.link = link;
        self.target = target;
        self.room = Room::new(&self.me, &self.name, &self.session, now);
        self.room.set_status(self.status.clone(), now);
        self.chat = Chat::new(self.env.hidden);
        self.muted.clear();
        self.moved = None;
        self.told_view = None;
        self.told_members.clear();
        self.joined = false;
        self.offers.clear();
        self.begin(start).await
    }

    /// Find the room: ask, and wait for the answer or be the room. `true` when it's over.
    async fn begin(&mut self, start: Start) -> bool {
        let nonce = random_hex(8);
        if !matches!(start, Start::Join { .. }) {
            return self.first(start);
        }
        self.send(Message::SyncReq { nonce: nonce.clone(), peek: false });
        let deadline = tokio::time::sleep(SYNC_WAIT);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                () = &mut deadline => break,
                heard = self.link.heard.recv() => match heard {
                    Some(Heard::Body(body)) if matches!(&body.m, Message::SyncAns { nonce: n, .. } if *n == nonce) => {
                        let now = self.now();
                        self.room.receive(*body, now);
                        break;
                    }
                    Some(heard) => self.on_heard(heard),
                    None => {
                        self.say(Event::RoomLeft { why: RoomLeftWhy::Unreachable });
                        return true;
                    }
                },
            }
        }
        let Start::Join { anyway } = start else { unreachable!() };
        if !anyway && self.room.count() > kaeri_model::room::ROOM_CAP as usize {
            self.say(Event::RoomLeft { why: RoomLeftWhy::Full });
            return true;
        }
        let free = self.room.free_name(&self.name);
        if free != self.name {
            self.name = free.clone();
            self.room.rename_me(&free);
            self.message(RoomMessageKind::Renamed { name: free });
        }
        self.hello();
        if let Some(head) = self.room.playhead().cloned() {
            self.look_up(head.ep.clone());
            if let Some(next) = &head.next {
                self.look_up(next.clone());
            }
            let on_it =
                self.status.ep.as_ref() == Some(&head.ep) && self.status.s.player_open();
            if !on_it {
                let kind = if self.room.before_play() {
                    RoomStartKind::Start
                } else {
                    RoomStartKind::Rejoin
                };
                self.set_status(|s| {
                    s.s = StateCode::Loading;
                    s.ep = Some(head.ep.clone());
                    s.rejoin = kind == RoomStartKind::Rejoin;
                });
                let at = (kind == RoomStartKind::Rejoin)
                    .then(|| self.room.position(self.now()))
                    .flatten();
                self.say(Event::RoomStarted {
                    id: AnilistId::new(head.ep.0),
                    episode: head.ep.1.clone(),
                    kind,
                    at,
                });
            } else {
                let wseq = head.wseq;
                self.set_status(|s| s.rdy = Some(wseq));
            }
        }
        false
    }

    fn hello(&mut self) {
        self.joined = true;
        if !matches!(self.connection, RoomConnection::Reconnecting { .. }) {
            self.connection = RoomConnection::Connected;
        }
        let name = self.name.clone();
        let idpk = self.identity.pubkey();
        let idsig = self.identity.prove(&self.session, &self.link.room_id);
        self.room.set_my_proof(&idpk, &idsig);
        self.send(Message::Hello { name, app: APP.into(), caps: Vec::new(), idpk, idsig });
    }

    /// This device is the room's first member.
    fn first(&mut self, start: Start) -> bool {
        self.hello();
        match start {
            Start::New(pick) => {
                let ep = ep_of(&pick);
                let next = next_of(&ep);
                let on_it = self.status.ep.as_ref() == Some(&ep) && self.status.s.player_open();
                if on_it {
                    // Started from the player: it keeps playing, the player becomes the room.
                    let pos = self.status.pos.unwrap_or(0.0);
                    let playing = self.pause.told() != Some(true);
                    self.write(
                        |h| {
                            h.cause = Cause::Play;
                            h.play = playing;
                            h.pos = pos;
                            h.next = next;
                            h.wseq = 0;
                        },
                        Some(ep.clone()),
                    );
                    self.look_up(ep);
                } else {
                    self.write(
                        |h| {
                            h.cause = Cause::Episode;
                            h.play = false;
                            h.next = next;
                        },
                        Some(ep),
                    );
                }
            }
            Start::Moved { ep, pos, playing } => {
                let next = next_of(&ep);
                self.write(
                    |h| {
                        h.cause = Cause::Play;
                        h.play = playing;
                        h.pos = pos;
                        h.next = next;
                        h.wseq = 0;
                    },
                    Some(ep),
                );
            }
            Start::Join { .. } => {}
        }
        false
    }
}

use kaeri_room::wire::Playhead;

fn from_wire(reaction: wire::Reaction) -> Reaction {
    match reaction {
        wire::Reaction::Laugh => Reaction::Laugh,
        wire::Reaction::Gasp => Reaction::Gasp,
        wire::Reaction::Clap => Reaction::Clap,
        wire::Reaction::Cry => Reaction::Cry,
    }
}

fn to_wire(reaction: Reaction) -> wire::Reaction {
    match reaction {
        Reaction::Laugh => wire::Reaction::Laugh,
        Reaction::Gasp => wire::Reaction::Gasp,
        Reaction::Clap => wire::Reaction::Clap,
        Reaction::Cry => wire::Reaction::Cry,
    }
}

/// `m:ss`, or `h:mm:ss`.
fn clock(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 { seconds as u64 } else { 0 };
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// "Mika and Ren", "Mika, Ren and Sol", in the app's language.
fn and(names: &[String]) -> String {
    use kaeri_i18n::t;
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => {
            let mut head = rest[0].clone();
            for name in &rest[1..] {
                head = t::room_names_comma(&head, name);
            }
            t::room_names_and(&head, last)
        }
    }
}

/// The room's line in words, in the app's language.
pub(crate) fn words(kind: &RoomMessageKind) -> String {
    use RoomMessageKind as K;
    use kaeri_i18n::t;
    let you = t::room_you();
    match kind {
        K::Started { by, episode } if *by == you => t::room_chat_you_started(episode),
        K::Started { by, episode } => t::room_chat_started(by, episode),
        K::Joined { name } => t::room_chat_joined(name),
        K::Left { name } => t::room_chat_left(name),
        K::Paused { by, at } if *by == you => t::room_chat_you_paused(&clock(*at)),
        K::Paused { by, at } => t::room_chat_paused(by, &clock(*at)),
        K::Resumed { by } => t::room_chat_resumed(by),
        K::Seeked { by, from, to } => t::room_chat_jumped(by, &clock(*from), &clock(*to)),
        K::Corrected => t::room_chat_corrected(),
        K::EpisodeChanged { by, episode } => t::room_chat_episode_changed(by, episode),
        K::JoinedAt { name } => t::room_chat_caught_up(name),
        K::AlreadyPaused { by } => t::room_chat_already_paused(by),
        K::WaitingFor { names, reason } => {
            if names.len() == 1 && names[0] == t::room_you_in_list() {
                return t::room_chat_paused_for_you();
            }
            let names = and(names);
            match reason {
                WaitReason::Loading => t::room_chat_paused_for_loading(&names),
                WaitReason::Seeking => t::room_chat_paused_for_seeking(&names),
                WaitReason::Buffering => t::room_chat_paused_for_buffering(&names),
            }
        }
        K::CarriedOn { by, past } => t::room_chat_carried_on(by, &and(past)),
        K::SkippedOpening { by } => t::room_chat_skipped_opening(by),
        K::NewInvite { by } => t::room_chat_new_invite(by),
        K::UpNext { episode } => t::room_chat_up_next(episode),
        K::UpNextCancelled { by } => t::room_chat_up_next_cancelled(by),
        K::MovedRoom { by } => t::room_chat_moved(by),
        K::WentAlong { name } => t::room_chat_went(name),
        K::Renamed { name } => t::room_chat_renamed(name),
    }
}

async fn run(
    target: Target,
    start: Start,
    name: String,
    env: Env,
    mut controls: mpsc::UnboundedReceiver<Control>,
    mut seats: mpsc::UnboundedReceiver<SeatMessage>,
    episodes: Episodes,
) {
    let link = match link::open(target.secret, target.relays.clone()).await {
        Ok(link) => link,
        Err(_) => {
            let _ = env.tx.send(Event::RoomLeft { why: RoomLeftWhy::Unreachable });
            return;
        }
    };
    let now = now_ms();
    let identity = Identity::new();
    let me = identity.mid();
    let session = link.session.pubkey();
    let hidden = env.hidden;
    let mut task = Task {
        env,
        name: name.clone(),
        identity,
        room: Room::new(&me, &name, &session, now),
        me,
        session,
        ctr: 0,
        target,
        link,
        chat: Chat::new(hidden),
        muted: HashMap::new(),
        cues: None,
        status: Status::default(),
        status_at: now,
        infos: HashMap::new(),
        pending_infos: Vec::new(),
        relays: Vec::new(),
        connection: RoomConnection::Joining,
        last_sent: now,
        pause: PlayerPause::default(),
        said_wait: 0,
        told_view: None,
        told_members: Vec::new(),
        offers: HashMap::new(),
        moved: None,
        joined: false,
        episodes,
    };
    // A playback going takes its seat first: starting from the player keeps it.
    let settle = tokio::time::sleep(Duration::from_millis(300));
    tokio::pin!(settle);
    loop {
        tokio::select! {
            () = &mut settle => break,
            Some(message) = seats.recv() => task.on_seat(message),
        }
    }
    if task.begin(start).await {
        return;
    }
    task.refresh();
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            heard = task.link.heard.recv() => match heard {
                Some(heard) => {
                    task.on_heard(heard);
                    task.refresh();
                }
                None => {
                    task.connection = RoomConnection::Reconnecting { relays_up: 0 };
                    task.refresh();
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            },
            message = seats.recv() => match message {
                Some(message) => {
                    task.on_seat(message);
                    task.refresh();
                }
                // The hub was opened for another room, or the engine is gone.
                None => {
                    task.send(Message::Leave);
                    return;
                }
            },
            command = controls.recv() => match command {
                Some(Control::Command(command)) => {
                    if task.on_command(command).await {
                        return;
                    }
                    task.refresh();
                }
                Some(Control::UpNextPoint) => {
                    task.set_status(|s| s.ue = true);
                    task.refresh();
                }
                Some(Control::NoSource) => {
                    task.set_status(|s| {
                        if s.s != StateCode::Choosing {
                            s.s = StateCode::NoSource;
                            s.rdy = None;
                        }
                    });
                    task.refresh();
                }
                None => {
                    task.send(Message::Leave);
                    return;
                }
            },
            _ = tick.tick() => {
                task.take_infos();
                task.tick();
            }
        }
    }
}

impl Task {
    fn on_seat(&mut self, message: SeatMessage) {
        match message {
            SeatMessage::Seated(cues) => {
                self.cues = Some(cues);
                self.pause.forget();
            }
            SeatMessage::Report(report) => self.on_report(report),
        }
    }

    fn take_infos(&mut self) {
        let mut changed = false;
        self.pending_infos.retain_mut(|rx| match rx.try_recv() {
            Ok(info) => {
                let ep = Ep(info.id.get(), info.episode.clone());
                self.infos.insert(ep, info);
                changed = true;
                false
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => true,
            Err(_) => false,
        });
        if changed {
            self.refresh();
        }
    }
}

/// How long a pause or play cue is waited for in the player's samples. A sample taken before the
/// player took the cue still shows the old state; within this it is stale, not a press.
const EXPECT_PAUSE_MS: i64 = 2_000;

/// What the player was told, paused or not, and the cue its samples haven't caught up with yet.
/// Without the wait, a sample from before a cue reads as the viewer pressing the other way, the
/// room follows it, and room and player pause and resume each other for ever.
#[derive(Debug, Default)]
struct PlayerPause {
    told: Option<bool>,
    /// The state told, until when its samples are waited for.
    expect: Option<(bool, i64)>,
}

impl PlayerPause {
    /// The room wants `paused`: `true` when the player must be told.
    fn tell(&mut self, paused: bool, now: i64) -> bool {
        if self.told == Some(paused) {
            return false;
        }
        self.told = Some(paused);
        self.expect = Some((paused, now + EXPECT_PAUSE_MS));
        true
    }

    fn told(&self) -> Option<bool> {
        self.told
    }

    /// A new player, or a new file: nothing told yet.
    fn forget(&mut self) {
        *self = Self::default();
    }

    /// A sample from the player: the viewer's own press, the told state to send again, or
    /// nothing to do.
    fn sample(&mut self, paused: bool, now: i64) -> Sampled {
        let Some(told) = self.told else { return Sampled::Same };
        if let Some((want, until)) = self.expect {
            if paused == want {
                self.expect = None;
                return Sampled::Same;
            }
            if now < until {
                return Sampled::Same;
            }
            // It never showed the told state: something else moved it (a file's own start).
            // A press counts only once the player has done what it was told.
            self.expect = Some((want, now + EXPECT_PAUSE_MS));
            return Sampled::Again(want);
        }
        if paused == told {
            return Sampled::Same;
        }
        self.told = Some(paused);
        Sampled::Pressed(paused)
    }
}

/// What a player's sample means for the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sampled {
    Same,
    /// The viewer pressed pause (`true`) or play.
    Pressed(bool),
    /// Send the player its told state again.
    Again(bool),
}

#[cfg(test)]
mod pause_tests {
    use super::{PlayerPause, Sampled};

    /// The room and a player that applies cues a moment late, with samples every 250 ms: what
    /// the room writes as the viewer's presses over 5 s. The player's first samples after each
    /// cue still show the state before it.
    fn presses(guarded: bool) -> usize {
        presses_with(guarded, None, false)
    }

    /// [`presses`], with the room waiting (paused) from the start, and the player's own file
    /// start unpausing it at `file_start` ms.
    fn presses_with(guarded: bool, file_start: Option<i64>, waiting: bool) -> usize {
        let mut pause = PlayerPause::default();
        let mut room_paused = waiting;
        let mut player_paused = true;
        // Cues in flight, applied in order 600 ms after each was sent.
        let mut cues: std::collections::VecDeque<(bool, i64)> = Default::default();
        let mut presses = 0;
        let mut now = 0;
        while now < 5_000 {
            if pause.tell(room_paused, now) {
                cues.push_back((room_paused, now + 600));
            }
            while let Some((to, at)) = cues.front().copied()
                && now >= at
            {
                player_paused = to;
                cues.pop_front();
            }
            if file_start == Some(now) {
                // The file starts: the player unpauses by itself, and its seat reports Playing,
                // which tells the player the room's state again from here.
                player_paused = false;
                pause.forget();
            }
            let press = if guarded {
                match pause.sample(player_paused, now) {
                    Sampled::Pressed(p) => Some(p),
                    Sampled::Again(p) => {
                        cues.push_back((p, now + 600));
                        None
                    }
                    Sampled::Same => None,
                }
            } else {
                // The old rule: any sample that differs from what was told is a press.
                pause.told().is_some_and(|t| t != player_paused).then_some(player_paused)
            };
            if let Some(paused) = press {
                presses += 1;
                room_paused = paused;
            }
            now += 250;
        }
        presses
    }

    #[test]
    fn a_files_own_start_playing_over_a_wait_is_never_your_press() {
        assert!(presses_with(false, Some(1_000), true) > 0, "the old rule takes it as yours");
        assert_eq!(presses_with(true, Some(1_000), true), 0);
    }

    #[test]
    fn stale_samples_no_longer_pause_and_resume_the_room_for_ever() {
        assert!(presses(false) > 4, "the old rule loops: {}", presses(false));
        assert_eq!(presses(true), 0);
    }

    #[test]
    fn a_real_press_still_counts_once_the_player_has_done_what_it_was_told() {
        let mut pause = PlayerPause::default();
        assert!(pause.tell(false, 0));
        assert_eq!(pause.sample(true, 100), Sampled::Same, "stale");
        assert_eq!(pause.sample(false, 300), Sampled::Same, "caught up");
        assert_eq!(pause.sample(true, 400), Sampled::Pressed(true), "the viewer paused");
        assert!(!pause.tell(true, 450), "already paused there");
        // Nothing told yet: no press.
        pause.forget();
        assert_eq!(pause.sample(false, 4_000), Sampled::Same);
    }

    #[test]
    fn a_player_that_unpauses_at_its_files_start_is_told_again_not_taken_as_a_press() {
        let mut pause = PlayerPause::default();
        // The room waits: pause. The player's own file start plays over it and never pauses.
        assert!(pause.tell(true, 0));
        assert_eq!(pause.sample(false, 500), Sampled::Same);
        assert_eq!(pause.sample(false, 2_500), Sampled::Again(true));
        assert_eq!(pause.sample(false, 3_000), Sampled::Same, "waiting for the cue again");
        assert_eq!(pause.sample(true, 3_400), Sampled::Same, "it took it");
        // Now a play is the viewer's.
        assert_eq!(pause.sample(false, 6_000), Sampled::Pressed(false));
    }
}
