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
        RoomMoved, RoomRecent, RoomRelaySettings, RoomShare, RoomStartKind, RoomUpNext,
        RoomView, SPARE_RELAYS, WaitReason, base64url, unbase64url,
    },
};
use kaeri_room::{
    Happening, Reason, Room, Word,
    keys::{Identity, RoomKeys, pk8},
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
/// After a new invite's key, Hello again this long after the first: members still moving to
/// the new key missed the first, and hold an older proof to answer newcomers with.
const REHELLO_MS: i64 = 2_000;
const HEARTBEAT_WAITED_MS: i64 = 5_000;
/// A pause taken as the viewer's this soon before the app goes to the background was the
/// system's, and is withdrawn.
const SYSTEM_PAUSE_MS: i64 = 300;
/// A stall this long reads Buffering, and may hold the room.
const STALL_GRACE_MS: i64 = 1_500;
/// A member who stalled twice in this long doesn't hold the room again.
const STALL_REPEAT_MS: i64 = 600_000;
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
    /// The app went to the background or came back.
    Phase(kaeri_model::command::AppState),
    /// The network path changed: open the sockets again.
    Network,
}

/// The room's episode and the one after it, as the task last saw them.
type Episodes = std::sync::Arc<std::sync::Mutex<Option<(Ep, Option<Ep>)>>>;

/// The room going, as the engine steers it.
pub struct Running {
    control: mpsc::UnboundedSender<Control>,
    episodes: Episodes,
    /// A scene holds this device as Choosing (testing builds only).
    held: Held,
}

/// Whether a scene holds this device as Choosing, as the engine reads it.
type Held = std::sync::Arc<std::sync::atomic::AtomicBool>;

/// Whether a scene holds this device as Choosing: the app's real play finding no source is not
/// said over the picker the scene shows (testing builds only; never in a product build).
pub fn held_choosing(engine: &EngineState) -> bool {
    going(engine).is_some_and(|r| r.held.load(std::sync::atomic::Ordering::Relaxed))
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
        // A look never leaves the room this device is in: it fails as a look.
        RoomCommand::PeekInvite { invite } => match target(&invite) {
            Ok(target) => peek(target, Look::Invite(invite), spares(engine), services, tx),
            Err(why) => look_failed(Look::Invite(invite), String::new(), why, tx),
        },
        RoomCommand::Rejoin { room_id } => match recent(&room_id, services) {
            Some((target, name)) => {
                begin(target, Start::Join { anyway: false }, Some(name), engine, services, tx);
            }
            None => gone(room_id, tx),
        },
        RoomCommand::LookIn { room_id } => match recent(&room_id, services) {
            Some((target, _)) => {
                peek(target, Look::Recent(room_id), spares(engine), services, tx)
            }
            None => gone(room_id, tx),
        },
        RoomCommand::LoadRecent => {
            let rooms = services.store.recent_rooms().unwrap_or_default();
            let rooms = rooms
                .into_iter()
                .map(|r| RoomRecent {
                    room_id: r.room_id,
                    id: r.anilist_id,
                    title: r.title,
                    episode: r.episode,
                    names: r.names,
                    last_at: r.last_at,
                    cover_url: r.cover_url,
                })
                .collect();
            let _ = tx.send(Event::RoomRecent(rooms));
        }
        RoomCommand::ForgetRoom { room_id } => {
            if let Err(e) = services.store.forget_room(&room_id) {
                tracing::warn!(error = %e, "room: couldn't forget a recent room");
            }
        }
        RoomCommand::LoadRelays => relays(engine, tx),
        RoomCommand::SetSpareRelays { relays: list } => {
            let list = list.into_iter().filter(|r| kaeri_model::room::valid_relay(r)).collect();
            crate::settings::save_room_spares(list, engine, services, tx);
            // A room going swaps in the new spares from now on.
            if let Some(running) = going(engine) {
                let relays = spares(engine);
                let _ = running
                    .control
                    .send(Control::Command(RoomCommand::SetSpareRelays { relays }));
            }
            relays(engine, tx);
        }
        RoomCommand::SetRelays { relays: list } => {
            let list = list.into_iter().filter(|r| kaeri_model::room::valid_relay(r)).collect();
            crate::settings::save_room_relays(list, engine, services, tx);
            relays(engine, tx);
        }
        RoomCommand::HideChat { hidden } => {
            // Kept for every room on this device, and told to the one going.
            crate::settings::save_hide_chat(hidden, engine, services, tx);
            if let Some(running) = going(engine) {
                let _ =
                    running.control.send(Control::Command(RoomCommand::HideChat { hidden }));
            }
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
        defaults: shown(&DEFAULT_RELAYS),
        spares: spares(engine),
        default_spares: shown(&SPARE_RELAYS),
        chosen: engine.config.effective().room.relays.clone(),
    }));
}

/// A built-in relay list as Settings shows it. Testing builds (the apps' Fixtures, shots) show
/// made-up hosts, never real ones.
fn shown(list: &[&str; 3]) -> Vec<String> {
    #[cfg(feature = "testing")]
    {
        let base = if list == &SPARE_RELAYS { 4 } else { 1 };
        (base..base + 3).map(|n| format!("wss://relay-{n}.example")).collect()
    }
    #[cfg(not(feature = "testing"))]
    list.iter().map(|r| (*r).to_owned()).collect()
}

/// The spares in use: Settings' own, or the built-in ones.
fn spares(engine: &EngineState) -> Vec<String> {
    engine.config.effective().room.spare_relays.clone().unwrap_or_else(|| shown(&SPARE_RELAYS))
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

/// The app went to the background or came back: the room says so in this device's status.
pub fn app_phase(phase: kaeri_model::command::AppState, engine: &EngineState) {
    if let Some(running) = going(engine) {
        let _ = running.control.send(Control::Phase(phase));
    }
}

/// The network path changed: the room's sockets open again at once.
pub fn network_changed(engine: &EngineState) {
    if let Some(running) = going(engine) {
        let _ = running.control.send(Control::Network);
    }
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

/// Where a player that just opened on the room's episode is moved: the room's place, playing or
/// not, as a paused or waiting room is somewhere in the episode too. Before play the room is at
/// the start, where the player already is.
fn seat_at(playing: bool, position: Option<f64>) -> Option<f64> {
    position.filter(|&pos| playing || pos > 0.5)
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
        Ok(None) => Err(RoomLeftWhy::Failed { reason: Msg::RoomLookNotAnInvite }),
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

/// A recent room's way back in: its target and the name used there.
fn recent(room_id: &str, services: &Services) -> Option<(Target, String)> {
    let room = services.store.recent_room(room_id).ok()??;
    let secret =
        unbase64url(&room.secret).and_then(|s| <[u8; 16]>::try_from(s.as_slice()).ok())?;
    let relays = if room.relays.is_empty() {
        link::relays_for_new_room(&[], &DEFAULT_RELAYS)
    } else {
        room.relays
    };
    Some((Target { secret, relays, by: None, title: Some(room.title) }, room.name))
}

fn gone(room_id: String, tx: &Outbox) {
    let _ = tx.send(Event::RoomLookFailed {
        room_id,
        reason: Msg::RoomLookNotKept,
        invite: None,
        why: None,
    });
}

/// What a look is into: an invite before joining it, or a recent room by its id.
enum Look {
    Invite(String),
    Recent(String),
}

/// A look into a room found nothing.
fn look_failed(look: Look, room_id: String, why: RoomLeftWhy, tx: &Outbox) {
    let reason = match &why {
        RoomLeftWhy::OldInvite => Msg::RoomLookOldInvite,
        RoomLeftWhy::Failed { reason } => reason.clone(),
        _ => Msg::RoomLookUnreachable,
    };
    let (room_id, invite) = match look {
        Look::Invite(invite) => (room_id, Some(invite)),
        Look::Recent(id) => (id, None),
    };
    let _ = tx.send(Event::RoomLookFailed { room_id, reason, invite, why: Some(why) });
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
                    why: RoomLeftWhy::Failed { reason: Msg::RoomNeedsEpisode },
                });
                return;
            };
            let relays = link::relays_for_new_room(
                &engine.config.effective().room.relays,
                &DEFAULT_RELAYS,
            );
            (
                Target { secret: random_secret(), relays, by: None, title: None },
                Start::New(pick),
            )
        }
    };
    begin(target, start, name, engine, services, tx);
}

/// Start the room's task for `target`, leaving any room before it.
fn begin(
    target: Target,
    start: Start,
    name: Option<String>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    leave(engine);
    let name = name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| engine.config.effective().room.name.clone());
    let name = if name.trim().is_empty() { "kaeri".to_owned() } else { name };
    let (control, controls) = mpsc::unbounded_channel();
    let seats = services.party.open();
    let held = Held::default();
    let env = Env {
        meta: services.meta.clone(),
        store: services.store.clone(),
        tx: tx.clone(),
        hidden: engine.config.effective().room.hide_chat,
        spares: spares(engine),
        held: held.clone(),
    };
    let episodes = Episodes::default();
    tokio::spawn(run(target, start, name, env, controls, seats, episodes.clone()));
    engine.room = Some(Running { control, episodes, held });
}

#[derive(Clone)]
struct Env {
    meta: Meta,
    store: kaeri_store::Store,
    tx: Outbox,
    hidden: bool,
    /// The relays this room swaps in or opens on when the room's own are down: Settings'.
    spares: Vec<String>,
    /// Whether a scene holds this device as Choosing, for the engine to read.
    #[cfg_attr(not(feature = "testing"), allow(dead_code))]
    held: Held,
}

/// Look into a room without anyone seeing: an invite's, or a recent room's (`room_id`).
fn peek(target: Target, look: Look, spares: Vec<String>, services: &Services, tx: &Outbox) {
    let tx = tx.clone();
    let meta = services.meta.clone();
    let (room_id, invite) = match &look {
        Look::Recent(id) => (Some(id.clone()), None),
        Look::Invite(invite) => (None, Some(invite.clone())),
    };
    tokio::spawn(async move {
        let Ok(mut link) = link::open(target.secret, target.relays.clone(), spares).await
        else {
            let id = RoomKeys::new(target.secret).room_id_hex();
            look_failed(look, id, RoomLeftWhy::Unreachable, &tx);
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
            .map(|m| member_of(&room, m, now, &HashMap::new(), None))
            .collect();
        let room_episode = match room.playhead() {
            Some(head) => Some(info(&meta, &head.ep, target.title.as_deref()).await),
            None => None,
        };
        let _ = tx.send(Event::RoomGlimpse(RoomGlimpse {
            room_id,
            by: target.by.clone(),
            full: members.len() as u32 >= kaeri_model::room::ROOM_CAP,
            members,
            room_episode,
            playing: room.playing(),
            invite,
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

/// The position a Status says: the player's own while it is open (so the others can see how far
/// behind it is), else where the room is.
fn wire_pos(st: &Status, room: Option<f64>) -> Option<f64> {
    st.pos.filter(|_| st.s.player_open()).or(room)
}

/// Local ms to epoch ms: the room's clock is the epoch already.
fn member_of(
    room: &Room,
    m: &kaeri_room::Member,
    now: i64,
    muted: &HashMap<Mid, bool>,
    waited: Option<i64>,
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
        // A waited member's row counts from the wait's start, as the room's line does.
        since_ms: Some(waited.unwrap_or_else(|| m.since())),
        behind_s: behind,
        episode: (word == Word::OtherEpisode)
            .then(|| m.status.ep.as_ref().map(episode_of))
            .flatten(),
        paused_room: room.paused_room(m),
        waited: waited.is_some(),
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
    /// This player kept with the room's position.
    drift: kaeri_room::drift::Drift,
    /// Since when this player has been held waiting for data mid-episode.
    stall_since: Option<i64>,
    /// None of this device's sources has the room's episode: closing the empty player or the
    /// source picker leaves it reading No source, not Player closed.
    no_source: bool,
    /// Where the app was last told to open the player (RoomStarted's `at`): a player reporting
    /// it late is never the viewer's seek.
    opened_at: Option<f64>,
    /// Where the room was just before this device's own write, for its Seeked line.
    seek_from: Option<f64>,
    /// Whether the player last said it was paused: a room started from the player starts
    /// playing or paused as it is.
    player_paused: Option<bool>,
    /// When the player said its pause changed with no cue behind it: a press is the viewer's
    /// only then.
    pressed_at: Option<i64>,
    /// A scene holds this device's posed state against its own player's reports.
    #[cfg(feature = "testing")]
    held: bool,
    /// How far behind the held pose says you are, kept as posed while the room plays on.
    #[cfg(feature = "testing")]
    held_behind: Option<f64>,
    /// When this device wrote a stall wait: not again for a member who stalled twice in 10
    /// minutes.
    stall_writes: Vec<i64>,
    /// When the player's last pause was taken as the viewer's: one just before going to the
    /// background was the system's.
    last_pause_at: Option<i64>,
    /// The status before going to the background, to come back to.
    before_bg: Option<Status>,
    /// The last wait said in the chat.
    said_wait: u64,
    told_view: Option<RoomView>,
    told_members: Vec<RoomMember>,
    offers: HashMap<String, Offer>,
    moved: Option<(Mid, Vec<Mid>)>,
    joined: bool,
    /// When to say Hello again after a new key.
    rehello_at: Option<i64>,
    /// The room's episode and the next, for the engine's plays.
    episodes: Episodes,
    /// What recent rooms last kept of this room: its id, episode, title and who was there.
    remembered: Option<(String, Ep, String, Vec<String>)>,
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
        st.pos = wire_pos(&st, self.room.position(now));
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
        // A scene's held pose stands against this device's own player (testing builds only).
        #[cfg(feature = "testing")]
        if self.held {
            return;
        }
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
        self.seek_from = self.room.position(now);
        if let Some(head) = self.room.write(now, now, change, ep) {
            // Ready for a seek's wait the viewer made here: the player is already there.
            if head.wseq == head.seq
                && head.cause == Cause::Seek
                && self.status.s == StateCode::Watching
            {
                self.status.rdy = Some(head.wseq);
                self.room.set_status(self.status.clone(), now);
            }
            self.send(Message::Playhead(head.clone()));
            self.on_playhead(old, head);
        } else {
            self.seek_from = None;
        }
    }

    /// A playhead won: say it, and move the player.
    fn on_playhead(&mut self, old: Option<Playhead>, new: Playhead) {
        // The room's place for this file: a player reporting it later is late, not the viewer.
        if self.status.ep.as_ref() == Some(&new.ep) {
            self.drift.placed(new.pos);
        }
        let seek_from = self.seek_from.take();
        let by = self.who(&new.by);
        let at = new.pos;
        let kind = match new.cause {
            Cause::Pause => Some(RoomMessageKind::Paused { by, at }),
            // Play before the room ever played starts the count, not a resume.
            Cause::Play if self.room.before_play() => None,
            Cause::Play => {
                old.as_ref().filter(|o| !o.play).map(|_| RoomMessageKind::Resumed { by })
            }
            Cause::Seek => {
                // From where the room was, not where its last playhead was written; a seek that
                // moved nothing is not one.
                let from = seek_from.or(old.as_ref().map(|o| o.pos)).unwrap_or(0.0);
                ((new.pos - from).abs() > kaeri_room::drift::LANDED)
                    .then_some(RoomMessageKind::Seeked { by, from, to: new.pos })
            }
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
        let new_wait = old.as_ref().is_none_or(|o| o.wseq != new.wseq);
        let seek_wait = new_wait
            && !new_episode
            && matches!(new.cause, Cause::Seek | Cause::SkipOpening)
            && !(new.by == self.me && new.cause == Cause::Seek);
        if seek_wait
            && self.status.s.player_open()
            && self.status.ep.as_ref() == Some(&new.ep)
            && self.cues.is_none()
        {
            // No player seated here: nothing to wait for.
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
                self.no_source = false;
                self.say(Event::RoomStarted {
                    id: AnilistId::new(new.ep.0),
                    episode: new.ep.1.clone(),
                    kind,
                    at: None,
                });
            }
        } else if seek_wait {
            // The player moves there and answers Ready at its first frame.
            let now = self.now();
            self.drift.cued_seek(new.pos, now);
            self.cue(SeatCue::Seek { to: new.pos, cue: Some(new.wseq) });
        } else if new.cause == Cause::CarryOn {
            let now = self.now();
            self.drift.cued_seek(new.pos, now);
            self.cue(SeatCue::Seek { to: new.pos, cue: None });
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

    /// The player closed without finishing: Player closed, unless this device has no source
    /// for the episode or its source picker is open, which the closed player doesn't change.
    fn close_player(&mut self) {
        self.set_status(|s| {
            if !matches!(s.s, StateCode::NoSource | StateCode::Choosing) {
                s.s = StateCode::PlayerClosed;
            }
        });
    }

    /// Tell the player to play or pause with the room. Before the room has a playhead (a
    /// player handing over to a room it starts) there is nothing to follow yet: the player
    /// stays as it is, and the room starts from it.
    fn cue_play(&mut self) {
        if self.room.playhead().is_none() {
            return;
        }
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
            room_id: RoomKeys::new(self.target.secret).room_id_hex(),
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
        let waited = self.room.waited(now);
        self.room
            .members()
            .into_iter()
            .filter(|m| self.room.shown(m, now))
            .map(|m| {
                let since = waited.iter().find(|w| w.mid == m.mid).map(|w| w.since);
                let mut row = member_of(&self.room, m, now, &self.muted, since);
                if row.you && row.behind_s.is_some() {
                    row.behind_s = self.own_behind(now);
                }
                row
            })
            .collect()
    }

    /// How far your own player is behind the room: from its last sample, or as a held scene
    /// posed it. The room's own record of you keeps no player clock to count from.
    fn own_behind(&self, now: i64) -> Option<f32> {
        #[cfg(feature = "testing")]
        if let (true, Some(b)) = (self.held, self.held_behind) {
            return Some(b as f32);
        }
        let pos = self.status.pos?;
        Some((self.room.position(now)? - pos).max(0.0) as f32)
    }

    /// Tell the front end what changed.
    /// Keep this room in recent rooms, with its episode and who is there, as they change.
    fn remember(&mut self) {
        if !self.joined {
            return;
        }
        let Some(head) = self.room.playhead() else { return };
        let ep = head.ep.clone();
        let info = self.infos.get(&ep);
        let title = info.map(|i| i.title.clone()).unwrap_or_default();
        if title.is_empty() {
            return;
        }
        let keys = RoomKeys::new(self.target.secret);
        let room_id = keys.room_id_hex();
        let mut names: Vec<String> = self
            .room
            .members()
            .into_iter()
            .filter(|m| m.mid != self.me && m.left_at.is_none())
            .map(|m| m.name.clone())
            .collect();
        names.sort();
        // Who was there stays once they've gone: names only grow while in the room.
        if let Some((id, _, _, before)) = &self.remembered
            && *id == room_id
        {
            for name in before {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
            names.sort();
        }
        let key = (room_id.clone(), ep.clone(), title.clone(), names.clone());
        if self.remembered.as_ref() == Some(&key) {
            return;
        }
        let relays = if self.target.relays.iter().map(String::as_str).eq(DEFAULT_RELAYS) {
            Vec::new()
        } else {
            self.target.relays.clone()
        };
        let row = kaeri_store::RecentRoom {
            room_id,
            secret: base64url(&self.target.secret),
            relays,
            identity: self.identity.secret_hex(),
            name: self.name.clone(),
            anilist_id: AnilistId::new(ep.0),
            title,
            episode: ep.1.clone(),
            names,
            cover_url: info.and_then(|i| i.cover_url.clone()),
            last_at: self.now() / 1000,
        };
        match self.env.store.remember_room(&row) {
            Ok(()) => self.remembered = Some(key),
            Err(e) => {
                tracing::warn!(error = %e, "room: couldn't keep the room in recent rooms")
            }
        }
    }

    fn refresh(&mut self) {
        self.remember();
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
            self.stall(now);
            self.relays_check(now);
            self.answer_due();
            if self.rehello_at.is_some_and(|at| now >= at) {
                self.rehello_at = None;
                self.hello();
            }
        }
        // Waits worth saying in the chat: past 1 s.
        let waited = self.room.waited(now);
        let wseq = self.room.playhead().map_or(0, |h| h.wseq);
        // Before play the header says it ("Starting when Ren is ready"): nothing was paused,
        // so the chat has no wait line.
        if let Some(first) = waited.first()
            && !self.room.before_play()
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
                // Back on a relay after none: say where this device is and ask the room again.
                // Nothing relies on relays keeping events.
                let was_down = matches!(self.connection, RoomConnection::Reconnecting { .. });
                if was_down && relays_up > 0 && self.joined {
                    self.send(Message::Status);
                    self.send(Message::SyncReq { nonce: random_hex(8), peek: false });
                }
                self.connection = if relays_up == 0 {
                    RoomConnection::Reconnecting { relays_up: 0 }
                } else if self.joined {
                    RoomConnection::Connected
                } else {
                    RoomConnection::Joining
                };
            }
            Heard::Relays(list) => {
                // A bit per room relay that works here, and the round trip, in every status.
                let ok = list
                    .iter()
                    .take(8)
                    .enumerate()
                    .filter(|(_, r)| r.healthy)
                    .fold(0u8, |bits, (i, _)| bits | (1 << i));
                let rtt = list.iter().filter_map(|r| r.rtt_ms).min().unwrap_or(0);
                self.status.ok = ok;
                self.status.rtt = rtt.min(u32::from(u16::MAX)) as u16;
                self.relays = list
                    .into_iter()
                    .map(|r| RelayHealth { url: r.url, healthy: r.healthy, rtt_ms: r.rtt_ms })
                    .collect();
            }
            Heard::Flushed => self.say(Event::RoomFlushed),
            Heard::Backdate { mid, ms } => self.room.backdate(&mid, ms),
            Heard::Pose { s, ms_ago, ep, behind, ue, hold } => {
                #[cfg(feature = "testing")]
                {
                    self.held = false;
                    self.held_behind = None;
                }
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
                #[cfg(feature = "testing")]
                {
                    self.held = hold;
                    let choosing = hold && s == StateCode::Choosing;
                    self.env.held.store(choosing, std::sync::atomic::Ordering::Relaxed);
                    self.held_behind = behind.filter(|_| hold);
                }
                #[cfg(not(feature = "testing"))]
                let _ = hold;
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
                if self.joined {
                    let now = self.now();
                    self.room.will_answer(&nonce, now);
                    self.answer_due();
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
                    self.reprove(secret);
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
                self.drift.reset();
                if on_room {
                    // The places this player may still report from before it opened.
                    let placed = self.room.playhead().map(|h| h.pos).into_iter();
                    for pos in placed.chain(self.opened_at) {
                        self.drift.placed(pos);
                    }
                }
                self.stall_since = None;
                self.no_source = false;
                self.cue_play();
                if on_room
                    && let Some(pos) =
                        seat_at(self.room.playing(), self.room.position(self.now()))
                {
                    let now = self.now();
                    self.drift.cued_seek(pos, now);
                    self.cue(SeatCue::Seek { to: pos, cue: None });
                }
            }
            SeatReport::Ready { cue } => {
                let head = self.room.playhead().cloned();
                if let (Some(cue), Some(head)) = (cue, head)
                    && cue == head.wseq
                    && self.status.ep.as_ref() == Some(&head.ep)
                {
                    self.set_status(|s| s.rdy = Some(cue));
                }
            }
            SeatReport::Stalled(true) => {
                if self.stall_since.is_none() && self.status.s.player_open() {
                    self.stall_since = Some(self.now());
                }
            }
            SeatReport::Stalled(false) => {
                self.stall_since = None;
                if self.status.s == StateCode::Buffering {
                    // Playing again where it stalled: ready for the room's wait.
                    let wseq = self.room.playhead().map(|h| h.wseq);
                    self.set_status(|s| {
                        s.s = StateCode::Watching;
                        s.rdy = wseq;
                    });
                }
            }
            SeatReport::Sample { position, paused, speed, seeked, pressed } => {
                if seeked {
                    self.drift.viewer_seeked(self.now());
                }
                if pressed {
                    self.pressed_at = Some(self.now());
                }
                #[cfg(feature = "testing")]
                let held = self.held;
                #[cfg(not(feature = "testing"))]
                let held = false;
                if !held {
                    self.status.pos = Some(position);
                }
                self.player_paused = Some(paused);
                // Away, a pause is the system's (a call, the lock screen): never the room's.
                if self.status.bg {
                    return;
                }
                if self.status.rejoin
                    && self
                        .room
                        .position(self.now())
                        .is_some_and(|p| (p - position).abs() <= 1.0)
                {
                    self.set_status(|s| s.rejoin = false);
                }
                self.follow(position, paused, speed);
                // The viewer pressed play or pause on the player; a sample from before the
                // player took the room's last cue is not a press.
                let sampled = self.pause.sample(paused, self.now());
                let now = self.now();
                let by_viewer = matches!(sampled, Sampled::Pressed(_))
                    && self.pressed_at.take().is_some_and(|at| now - at <= PRESSED_FOR_MS);
                match sampled {
                    Sampled::Same => {}
                    // The player never showed what it was told (its own file start unpaused it):
                    // tell it again, never take it as the viewer's.
                    Sampled::Again(paused) => self.cue(SeatCue::Pause(paused)),
                    // Not the viewer's (the player's pause never changed by itself): a stale
                    // or bursty sample. The player is told the room's state again.
                    Sampled::Pressed(_) if !by_viewer => {
                        self.pause.forget();
                        self.cue_play();
                    }
                    Sampled::Pressed(true) => {
                        self.last_pause_at = Some(self.now());
                        self.write(
                            |h| {
                                h.play = false;
                                h.cause = Cause::Pause;
                            },
                            None,
                        );
                    }
                    Sampled::Pressed(false) => self.play(),
                }
            }
            SeatReport::Ended { finished, .. } => {
                if finished {
                    self.set_status(|s| s.ue = true);
                } else {
                    self.close_player();
                }
                self.cues = None;
            }
        }
    }

    /// Swap or add a relay when the health reports call for it (see `Room::relay_change`).
    fn relays_check(&mut self, now: i64) {
        let spares = self.env.spares.clone();
        let Some(list) = self.room.relay_change(now, &spares) else { return };
        let seq = self.room.relays_seq() + 1;
        tracing::info!(relays = list.len(), "room: the room's relays change");
        self.room.set_relays(list.clone(), seq);
        self.target.relays = list.clone();
        self.send(Message::Relays { seq, list: list.clone() });
        let _ = self.link.out.send(Out::Relays(list));
    }

    /// The app's phase. In the background without playing, this device reads Player closed and
    /// says so once (the app's background task waits for [`Event::RoomFlushed`]); playing on in
    /// the background (PiP, AirPlay, audio) its status stays real. Either way heartbeats slow to
    /// 30 s. Back in the foreground: fresh sockets, the room asked again, and the player comes
    /// back in at the room's place without being waited for.
    fn phase(&mut self, phase: kaeri_model::command::AppState) {
        use kaeri_model::command::AppState;
        let now = self.now();
        match phase {
            AppState::Background { playing } => {
                if self.status.bg {
                    return;
                }
                // A pause the app made for going away just before this never reaches the room.
                if self.last_pause_at.is_some_and(|at| now - at <= SYSTEM_PAUSE_MS) {
                    self.last_pause_at = None;
                    self.write(
                        |h| {
                            h.play = true;
                            h.cause = Cause::Play;
                        },
                        None,
                    );
                }
                self.before_bg = Some(self.status.clone());
                self.status.bg = true;
                self.status.hb = 30;
                if !playing {
                    self.status.s = StateCode::PlayerClosed;
                    self.status_at = now;
                }
                self.room.set_status(self.status.clone(), now);
                if self.joined {
                    let body = self.body(Message::Status);
                    let _ = self.link.out.send(Out::Flush(body));
                } else {
                    self.say(Event::RoomFlushed);
                }
            }
            AppState::Foreground | AppState::BackgroundDownloads => {
                if !self.status.bg {
                    return;
                }
                let before = self.before_bg.take().unwrap_or_else(|| self.status.clone());
                self.status =
                    Status { bg: false, hb: 15, rejoin: before.s.player_open(), ..before };
                self.status_at = now;
                self.room.set_status(self.status.clone(), now);
                let _ = self.link.out.send(Out::Refresh);
                if self.joined {
                    self.send(Message::Status);
                    self.send(Message::SyncReq { nonce: random_hex(8), peek: false });
                }
                self.pause.forget();
                self.drift.reset();
                self.cue_play();
            }
        }
    }

    /// Keep the player with the room: the viewer's own seek is the room's, the player is moved
    /// or slowed to stay with it, and Behind is said as it comes and goes.
    fn follow(&mut self, position: f64, paused: bool, speed: f64) {
        let now = self.now();
        let on_room = self.room.playhead().is_some_and(|h| {
            self.status.ep.as_ref() == Some(&h.ep) && self.status.s.player_open()
        });
        if !on_room {
            return;
        }
        let room = self.room.position(now).map(|p| (p, self.room.playing()));
        for act in self.drift.sample(position, paused, speed, now, room) {
            match act {
                kaeri_room::drift::Act::ViewerSeek(to) => self.write(
                    |h| {
                        h.pos = to;
                        h.cause = Cause::Seek;
                    },
                    None,
                ),
                kaeri_room::drift::Act::Seek(to) => self.cue(SeatCue::Seek { to, cue: None }),
                kaeri_room::drift::Act::Speed(rate) => self.cue(SeatCue::Speed(rate)),
                kaeri_room::drift::Act::Behind(behind) => {
                    if matches!(self.status.s, StateCode::Watching | StateCode::Behind) {
                        self.set_status(|s| {
                            s.s = if behind { StateCode::Behind } else { StateCode::Watching };
                        });
                    }
                }
            }
        }
    }

    /// A stall past 1.5 s reads Buffering, and in a room of 5 or fewer holds the room for this
    /// player alone, unless this member stalled twice in the last 10 minutes.
    fn stall(&mut self, now: i64) {
        let Some(since) = self.stall_since else { return };
        if now - since < STALL_GRACE_MS || self.status.s == StateCode::Buffering {
            return;
        }
        self.set_status(|s| s.s = StateCode::Buffering);
        self.status_at = since;
        self.stall_writes.retain(|at| now - at < STALL_REPEAT_MS);
        if self.room.playing() && self.room.stall_may_wait(now) && self.stall_writes.len() < 2 {
            self.stall_writes.push(now);
            let pos = self.status.pos;
            self.write(
                |h| {
                    h.cause = Cause::Stall;
                    if let Some(pos) = pos {
                        h.pos = pos;
                    }
                },
                None,
            );
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
        // A held pose stands against the player opening and closing, never against what you
        // choose yourself (testing builds only).
        #[cfg(feature = "testing")]
        // An app closes its picker or opens its player by itself as it plays; Choosing a source,
        // Play, Carry on and the like are what you choose.
        if matches!(
            command,
            RoomCommand::Play { .. }
                | RoomCommand::ChangeEpisode { .. }
                | RoomCommand::CarryOn
                | RoomCommand::DontWait
                | RoomCommand::SkipOpening { .. }
                | RoomCommand::CancelUpNext
                | RoomCommand::PlayNextNow
                | RoomCommand::ChoosingSource { open: true }
                | RoomCommand::Leave
        ) {
            self.held = false;
            self.held_behind = None;
            self.env.held.store(false, std::sync::atomic::Ordering::Relaxed);
        }
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
            RoomCommand::ChangeEpisode { id, episode }
                if self
                    .room
                    .playhead()
                    .is_some_and(|h| h.ep == Ep(id.get(), episode.clone())) =>
            {
                return Box::pin(self.on_command(RoomCommand::OpenPlayer)).await;
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
            RoomCommand::ChoosingSource { open: true } => {
                self.no_source |= self.status.s == StateCode::NoSource;
                self.set_status(|s| s.s = StateCode::Choosing)
            }
            RoomCommand::ChoosingSource { open: false } => {
                let no_source = self.no_source;
                self.set_status(|s| {
                    if s.s == StateCode::Choosing {
                        s.s = if no_source {
                            StateCode::NoSource
                        } else {
                            StateCode::PlayerClosed
                        };
                    }
                });
            }
            RoomCommand::ClosePlayer => {
                self.close_player();
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
                    self.opened_at = at;
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
                self.reprove(secret);
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
            // Settings' spares changed: the engine sends the list in use.
            RoomCommand::SetSpareRelays { relays } => self.env.spares = relays,
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
        let spares = self.env.spares.clone();
        let link = match link::open(target.secret, target.relays.clone(), spares).await {
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
        self.room.set_relays(self.target.relays.clone(), 0);
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
                self.opened_at = at;
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

    /// A new invite's key: this device proves itself for the new room id and says Hello again,
    /// so the others answer a newcomer by the new link with proofs it can check.
    fn reprove(&mut self, secret: [u8; 16]) {
        self.target.secret = secret;
        self.link.room_id = RoomKeys::new(secret).room_id();
        if self.joined {
            self.hello();
            self.rehello_at = Some(self.now() + REHELLO_MS);
        }
    }

    /// Answer the sync requests whose turn has come here.
    fn answer_due(&mut self) {
        let now = self.now();
        for nonce in self.room.answers_due(now) {
            let answer = self.room.sync_answer(&nonce, now);
            self.send(answer);
        }
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
                    let playing = self.player_paused != Some(true);
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
    let link = match link::open(target.secret, target.relays.clone(), env.spares.clone()).await
    {
        Ok(link) => link,
        Err(_) => {
            let _ = env.tx.send(Event::RoomLeft { why: RoomLeftWhy::Unreachable });
            return;
        }
    };
    let now = now_ms();
    // The same identity key as the last time in this room: a rejoin is the same person.
    let room_id = RoomKeys::new(target.secret).room_id_hex();
    let identity = env
        .store
        .recent_room(&room_id)
        .ok()
        .flatten()
        .and_then(|r| Identity::from_secret_hex(&r.identity))
        .unwrap_or_default();
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
        drift: kaeri_room::drift::Drift::default(),
        stall_since: None,
        no_source: false,
        player_paused: None,
        pressed_at: None,
        #[cfg(feature = "testing")]
        held: false,
        #[cfg(feature = "testing")]
        held_behind: None,
        opened_at: None,
        seek_from: None,
        stall_writes: Vec::new(),
        last_pause_at: None,
        before_bg: None,
        said_wait: 0,
        told_view: None,
        told_members: Vec::new(),
        offers: HashMap::new(),
        moved: None,
        joined: false,
        rehello_at: None,
        episodes,
        remembered: None,
    };
    task.room.set_relays(task.target.relays.clone(), 0);
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
                Some(Control::Phase(phase)) => {
                    task.phase(phase);
                    task.refresh();
                }
                Some(Control::Network) => {
                    let _ = task.link.out.send(Out::Refresh);
                }
                Some(Control::UpNextPoint) => {
                    task.set_status(|s| s.ue = true);
                    task.refresh();
                }
                Some(Control::NoSource) => {
                    task.no_source = true;
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
    /// When the player last showed the told state: reports it made before that can still
    /// arrive just after.
    confirmed_at: Option<i64>,
}

/// A sample's press is the viewer's only this soon after the player said its pause changed by
/// itself, ms.
const PRESSED_FOR_MS: i64 = 3_000;

/// After the player shows the told state, a contrary sample this soon is an older report
/// arriving late, not a press.
const SETTLE_MS: i64 = 1_000;

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

    #[cfg(test)]
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
                self.confirmed_at = Some(now);
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
        if self.confirmed_at.is_some_and(|at| now - at < SETTLE_MS) {
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
    fn a_player_opening_in_a_paused_or_waiting_room_goes_to_the_rooms_place() {
        assert_eq!(super::seat_at(true, Some(12.0)), Some(12.0));
        assert_eq!(super::seat_at(false, Some(12.0)), Some(12.0), "paused at 0:12");
        assert_eq!(super::seat_at(false, Some(35.0)), Some(35.0), "waiting after a seek");
        assert_eq!(super::seat_at(false, Some(0.0)), None, "before play: already there");
        assert_eq!(super::seat_at(true, None), None);
    }

    #[test]
    fn a_late_report_after_the_player_took_the_pause_is_not_a_press() {
        // The room waits: pause. The player takes it late, after its file loaded, and an older
        // position report saying it plays arrives just after.
        let mut pause = PlayerPause::default();
        assert!(pause.tell(true, 0));
        assert_eq!(pause.sample(false, 900), Sampled::Same, "before it took the cue");
        assert_eq!(pause.sample(true, 1_500), Sampled::Same, "it took it");
        assert_eq!(pause.sample(false, 1_600), Sampled::Same, "an older report, late");
        assert_eq!(pause.sample(true, 1_900), Sampled::Same);
        // A real play, later, is the viewer's.
        assert_eq!(pause.sample(false, 4_000), Sampled::Pressed(false));
    }

    #[test]
    fn the_others_see_how_far_behind_your_player_is() {
        // Neon: your Status said the room's clock as your place, so the others read Behind 0 s.
        use kaeri_room::wire::{Body, Ep, Message, StateCode, Status, VERSION};
        use std::collections::HashMap;
        let ep = Ep(4242, "9".into());
        let mut mika = kaeri_room::room::Room::new("m", "Mika", "pm", 0);
        mika.write(
            0,
            0,
            |h| {
                h.play = true;
                h.cause = kaeri_room::wire::Cause::Episode;
            },
            Some(ep.clone()),
        );
        let wseq = mika.playhead().unwrap().wseq;
        mika.set_status(
            Status {
                s: StateCode::Watching,
                ep: Some(ep.clone()),
                rdy: Some(wseq),
                ..Status::default()
            },
            0,
        );
        mika.settle(3_000);
        // Ada's player is 6 s behind the room at 20 s.
        let now = 23_000;
        let room = mika.position(now);
        assert_eq!(room, Some(20.0));
        let mut st = Status {
            s: StateCode::Behind,
            ep: Some(ep),
            pos: Some(14.0),
            play: true,
            ..Status::default()
        };
        st.pos = super::wire_pos(&st, room);
        let hello = Message::Hello {
            name: "Ada".into(),
            app: "0.6".into(),
            caps: vec![],
            idpk: String::new(),
            idsig: String::new(),
        };
        let body = |m| Body {
            v: VERSION,
            from: "pa".into(),
            mid: "a".into(),
            ctr: 1,
            t: 0,
            st: st.clone(),
            m,
        };
        mika.receive(body(hello), now);
        mika.receive(body(Message::Status), now);
        let ada = mika.members().into_iter().find(|m| m.mid == "a").unwrap();
        let row = super::member_of(&mika, ada, now, &HashMap::new(), None);
        assert_eq!(row.behind_s, Some(6.0));
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
        assert_eq!(pause.sample(true, 1_400), Sampled::Pressed(true), "the viewer paused");
        assert!(!pause.tell(true, 1_450), "already paused there");
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
