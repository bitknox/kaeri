//! Casting to TVs with Chromecast built in: the devices the TV picker's browse found, what the
//! engine has learned of each TV, and the cast running now.
//!
//! A cast is a playback like any other, run by the engine's session here: the streams are found
//! on this device, history and skipping and auto-next work as they do in a window, and the
//! player the session drives is the TV, through [`CastLink`]. [`CastHub`] is what the parts
//! share: the sender's picker reads its devices, [`Command::CastTo`] opens a cast through it,
//! and the card reads the cast's status and the engine's events off it.
//!
//! A cast session ([`CastHub::connect`]) holds a TV the way the big apps do: connecting
//! launches the receiver and holds it, every play goes to the TV while it lasts, a playback that
//! ends leaves the receiver up for the next one, and only a disconnect, the TV or the network
//! ends it. [`CastHub::session`] is what the apps draw of it. A session this device left running
//! is joined again when the browse finds its TV ([`CastHub::rejoin`]).
//!
//! [`CastLink`]: crate::playback::link::cast::CastLink
//! [`Command::CastTo`]: kaeri_model::command::Command::CastTo

use std::{
    collections::BTreeMap,
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock, PoisonError, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use futures::StreamExt;
pub use kaeri_cast as protocol;
use kaeri_cast::{
    AacEncoder, CastDevice, CastFailure, CastMemory, CastPlayer, CastSetup, CastState,
    CastStatus, DEFAULT_MEDIA_RECEIVER, FromReceiver, NoEncoder, PlainHttp, Timings,
    UpNextCard, Upstream, UpstreamResponse, VolumeControl, formats::is_loopback_url,
};
use kaeri_core::ids::AnilistId;
use kaeri_model::{event::Event, playback::SkipKind};
use kaeri_net::HttpClient;
use kaeri_playback::{PlaybackEvent, ScriptEvent};
use kaeri_remote::state::{RemoteState, StateFolder};
use tokio::sync::{broadcast, watch};

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the parts of casting share. One per engine.
pub struct CastHub {
    /// Where `cast-tvs.json` is kept; `None` keeps it in memory, for tests.
    dir: Option<PathBuf>,
    memory: OnceLock<Arc<CastMemory>>,
    upstream: Arc<dyn Upstream>,
    encoder: Mutex<Arc<dyn AacEncoder>>,
    timings: Timings,
    /// What the browse found, by TXT `id`.
    devices: Mutex<BTreeMap<String, CastDevice>>,
    cast: Mutex<Current>,
    /// Counts up whenever the devices or the cast read differently.
    changes: watch::Sender<u64>,
    /// Whether the TV picker lists TVs with Chromecast built in: Settings' row.
    show_tvs: AtomicBool,
    /// The language the front end speaks, for the TV's episode lines.
    language: Mutex<Option<String>>,
    /// What Kaeri's receiver calls this device: `iphone`, `phone`, `mac`, ...
    device_word: Mutex<Option<String>>,
    /// The player whose receiver's messages are followed.
    listening: Mutex<Weak<CastPlayer>>,
    /// What Kaeri's receiver says, for the sender's parts that act on it.
    heard: broadcast::Sender<FromReceiver>,
    /// The cast session: the TV this device holds.
    session: Mutex<Session>,
    /// The receiver app the config names in place of Kaeri's own.
    receiver: Mutex<Option<String>>,
    /// The session left running, to join again: in `cast-session.json` beside `cast-tvs.json`.
    left: Mutex<Option<Option<Left>>>,
    /// Counts the cast links opened on a held player: only the latest stops its media.
    links: std::sync::atomic::AtomicU64,
    /// The player whose fall back to the TV's own player was last said, so it is said once.
    told_fallback: Mutex<Weak<CastPlayer>>,
    /// The runtime the hub was made on, for work a platform thread starts (a browse's find).
    runtime: Option<tokio::runtime::Handle>,
    /// Casts connecting to a TV outside the session, until their player is attached.
    opening: Arc<std::sync::atomic::AtomicUsize>,
}

/// A cast connecting to a TV outside the session: the hub joins nothing meanwhile.
pub(crate) struct Opening(Arc<std::sync::atomic::AtomicUsize>);

impl Drop for Opening {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// The TV this device holds, from connect until disconnect, or until the TV ends it.
#[derive(Default)]
struct Session {
    device: Option<CastDevice>,
    player: Option<Arc<CastPlayer>>,
    connecting: bool,
    /// Why the session ended, when the TV or the network ended it.
    failure: Option<CastFailure>,
    /// Counts connects and disconnects, so a connect that lands after a newer one is dropped.
    generation: u64,
    /// The link that plays on the held player now.
    link: u64,
    /// What played when the TV or the network ended the session, for Play here.
    ended: Option<SessionItem>,
}

/// A session this device left running on a TV: its TXT `id` and the receiver app's session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Left {
    device: String,
    session: String,
}

const LEFT_FILE: &str = "cast-session.json";

/// What an attached cast last noted of its receiver.
#[derive(Default)]
struct Noted {
    /// The receiver session noted as left running.
    session: Option<String>,
    /// Whether something played or loaded at the last status before it ended.
    busy: bool,
}

/// Where a cast session is, as the apps' cast button and its sheet draw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionState {
    /// No TV is held.
    #[default]
    Idle,
    /// Connecting to the TV and launching the receiver.
    Connecting,
    /// The receiver is up and nothing plays.
    Connected,
    /// Something plays on the TV, or is starting, paused or buffering.
    Playing,
    /// The TV or the network ended the session: `failure` says why.
    Error,
}

/// What the cast button shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CastButton {
    /// No TV found, or TVs are off in Settings: no button.
    #[default]
    Hidden,
    /// TVs are found and none is held: the outline button opens the chooser.
    Available,
    Connecting,
    /// A TV is held, playing or not.
    Connected,
}

/// What plays on the TV, or comes next.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionItem {
    /// The title, when Kaeri plays it; `None` for what the receiver plays from elsewhere.
    pub id: Option<AnilistId>,
    /// The show.
    pub title: String,
    pub episode: Option<String>,
    pub episode_title: Option<String>,
    /// The art the TV shows, by URL.
    pub image: Option<String>,
    pub position: f64,
    pub duration: Option<f64>,
    pub paused: bool,
    /// The TV waits for the stream in the middle of the file: whole seconds since it began,
    /// `Some(0)` at once. Never set while it starts or reloads, which `loading` says.
    pub waiting: Option<u32>,
    /// The TV is still opening it, or this device is still finding its streams, or the file
    /// loads again for another audio track.
    pub loading: bool,
}

/// The cast session as the apps draw it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionView {
    pub state: SessionState,
    pub button: CastButton,
    /// Counts up with every change to the cast: a control holds what it shows until a view
    /// with a higher `rev` arrives.
    pub rev: u64,
    pub device: Option<CastDevice>,
    pub item: Option<SessionItem>,
    /// What plays next: Up Next's episode, once the playback has said it.
    pub queue: Vec<SessionItem>,
    pub failure: Option<CastFailure>,
    /// Why the session ended, in the engine's words, naming the TV.
    pub notice: Option<kaeri_model::Msg>,
    /// A note while a TV is held, for a toast: shown when it first reads differently. The TV
    /// shows plain subtitles, or plays this cast in its own player as Kaeri's receiver didn't
    /// start.
    pub note: Option<kaeri_model::Msg>,
    /// The receiver's volume, 0 to 1.
    pub level: Option<f64>,
    pub muted: Option<bool>,
    pub volume: VolumeControl,
    /// Kaeri's own receiver runs, rather than the Default Media Receiver.
    pub own_receiver: bool,
    /// The connection to the TV closed and is being made again: `state` stays what it was until
    /// it joins again, or the tries run out and the session ends.
    pub reconnecting: bool,
}

/// How long Up Next counts down without an ending to start it, when the setting says none:
/// the hosted players' own.
const UP_NEXT_COUNTDOWN: u32 = 10;
/// Without an ending, a file longer than two minutes shows Up Next this long before its end.
const UP_NEXT_WITHOUT_ENDING: f64 = 30.0;
/// An automatic skip's Undo stays for this much playback.
const UNDO_PLAYBACK: f64 = 6.0;
/// And for this long at most, however little played: a paused cast lets it go too.
const UNDO_LONGEST: Duration = Duration::from_secs(15);

/// What plays next, as the session said it, and how far the card's countdown has gone.
#[derive(Debug, Clone, PartialEq)]
pub struct UpNext {
    pub number: String,
    pub title: Option<String>,
    /// The episode's still, by URL.
    pub still: Option<String>,
    /// The episode rolls on by itself when the count ends.
    pub auto: bool,
    /// Where the card shows: the ending's start, when known.
    at: Option<f64>,
    countdown: u32,
    /// The viewer said no, for this episode.
    cancelled: bool,
    /// Play next was asked for, by the count or the viewer.
    played: bool,
    /// Where the count runs from since a seek landed in it.
    restarted_at: Option<f64>,
}

impl UpNext {
    /// Where the card shows and its count starts: the ending, or near the end of the file.
    fn shows_at(&self, duration: Option<f64>) -> Option<f64> {
        if let Some(at) = self.at {
            return Some(at);
        }
        let duration = duration.filter(|d| *d > 0.0)?;
        Some(if duration > 120.0 {
            duration - UP_NEXT_WITHOUT_ENDING
        } else {
            (duration - f64::from(self.countdown())).max(0.0)
        })
    }

    fn countdown(&self) -> u32 {
        if self.countdown > 0 { self.countdown } else { UP_NEXT_COUNTDOWN }
    }

    /// Whether the card is up at `position`.
    pub fn shown(&self, position: f64, duration: Option<f64>) -> bool {
        !self.cancelled
            && !self.played
            && self.shows_at(duration).is_some_and(|at| position >= at)
    }

    /// Seconds left on the count at `position`, while it runs: never with no countdown, where
    /// the card waits for the viewer.
    pub fn remaining(&self, position: f64, duration: Option<f64>) -> Option<f64> {
        if !self.auto || self.cancelled || self.countdown == 0 {
            return None;
        }
        let shows = self.shows_at(duration)?;
        let start = self.restarted_at.map_or(shows, |r| r.max(shows));
        (position >= start).then(|| (f64::from(self.countdown) - (position - start)).max(0.0))
    }
}

/// The note that an automatic skip can be undone.
#[derive(Debug, Clone, PartialEq)]
pub struct Undo {
    pub kind: SkipKind,
    /// Where Undo seeks back to.
    pub to: f64,
    /// Where playback was when the note was first seen with a position after the skip.
    landed: Option<f64>,
    since: Instant,
}

impl Undo {
    /// When the skip was made.
    pub fn since(&self) -> Instant {
        self.since
    }
}

/// The cast running now, and what the card reads of it.
#[derive(Default)]
struct Current {
    /// The TV the viewer cast to, from the moment they asked.
    device: Option<CastDevice>,
    player: Weak<CastPlayer>,
    status: Option<CastStatus>,
    folder: StateFolder,
    state: Option<RemoteState>,
    /// The viewer stopped casting, or the cast ended for good.
    over: bool,
    /// When the episode now starting was asked for: the tap, or the end of the one before.
    /// `None` once it plays.
    starting: Option<Instant>,
    /// A load began since `starting`, so a status that plays is this episode's.
    loading: bool,
    next: Option<UpNext>,
    /// The skip on offer: what it skips and where to.
    skip: Option<(SkipKind, f64)>,
    undo: Option<Undo>,
    /// The title and episode the cast was last asked to play, for Try again.
    asked: Option<(AnilistId, String)>,
    /// A note while it plays, said once: the TV shows plain subtitles, or plays this cast in its
    /// own player.
    note: Option<kaeri_model::Msg>,
    /// Kaeri's receiver was told to show Up Next's card.
    told_next: bool,
}

/// The cast as the card reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CastView {
    pub device: Option<CastDevice>,
    /// A player is connected to the TV.
    pub connected: bool,
    pub status: Option<CastStatus>,
    /// What the engine's events say of the playback.
    pub state: Option<RemoteState>,
    pub over: bool,
    /// When the episode now starting was asked for, until it plays.
    pub starting: Option<Instant>,
    pub next: Option<UpNext>,
    pub skip: Option<(SkipKind, f64)>,
    pub undo: Option<Undo>,
    /// The title and episode the cast was last asked to play.
    pub asked: Option<(AnilistId, String)>,
    /// A note while it plays, said once.
    pub note: Option<kaeri_model::Msg>,
}

impl CastHub {
    /// The hub for an engine whose data directory holds the remote's files in `dir`, fetching
    /// what a TV can't with `http`.
    pub fn new(dir: PathBuf, http: HttpClient) -> Self {
        Self::with(Some(dir), Arc::new(HttpUpstream(http)), Timings::default())
    }

    /// A hub that keeps what it learns of TVs in memory alone: what a test's engine has.
    pub fn in_memory(http: HttpClient) -> Self {
        Self::with(None, Arc::new(HttpUpstream(http)), Timings::default())
    }

    /// A hub with its parts given: what tests stand casting up with.
    pub fn with(dir: Option<PathBuf>, upstream: Arc<dyn Upstream>, timings: Timings) -> Self {
        let (changes, _) = watch::channel(0);
        Self {
            dir,
            memory: OnceLock::new(),
            upstream,
            encoder: Mutex::new(Arc::new(NoEncoder)),
            timings,
            devices: Mutex::new(BTreeMap::new()),
            cast: Mutex::new(Current::default()),
            changes,
            show_tvs: AtomicBool::new(true),
            language: Mutex::new(None),
            device_word: Mutex::new(None),
            listening: Mutex::new(Weak::new()),
            heard: broadcast::channel(16).0,
            session: Mutex::new(Session::default()),
            receiver: Mutex::new(None),
            left: Mutex::new(None),
            links: std::sync::atomic::AtomicU64::new(0),
            told_fallback: Mutex::new(Weak::new()),
            runtime: tokio::runtime::Handle::try_current().ok(),
            opening: Arc::default(),
        }
    }

    /// Run `task` on the runtime this is called on, or the hub's own from a platform thread.
    pub(crate) fn spawn(&self, task: impl std::future::Future<Output = ()> + Send + 'static) {
        match tokio::runtime::Handle::try_current().ok().or_else(|| self.runtime.clone()) {
            Some(runtime) => drop(runtime.spawn(task)),
            None => tracing::warn!("no runtime to run casting's work on"),
        }
    }

    /// The receiver app the config names in place of Kaeri's own: `cast.receiver`.
    pub fn set_receiver(&self, receiver: Option<String>) {
        *lock(&self.receiver) = receiver.filter(|r| !r.trim().is_empty());
    }

    /// The receiver app to launch in place of Kaeri's own: `KAERI_CAST_RECEIVER_APP` in a
    /// build that honours it ([`dev_receiver`]), else the config's `cast.receiver`.
    pub fn receiver(&self) -> Option<String> {
        dev_receiver().or_else(|| lock(&self.receiver).clone())
    }

    /// What a cast to `device` is set up with.
    pub fn setup(&self, device: CastDevice) -> CastSetup {
        CastSetup {
            device,
            memory: self.memory(),
            upstream: self.upstream(),
            encoder: self.encoder(),
            timings: self.timings,
        }
    }

    /// Connect to `device` and hold it: the receiver launches, and every play goes to the TV
    /// until [`Self::disconnect`]. A receiver this device left running there and that still
    /// runs is joined instead, as it plays, never launched again. A session held on another TV
    /// is let go first; one on this TV already is kept. Returns at once: [`Self::session`]
    /// follows it.
    pub fn connect(self: &Arc<Self>, device: CastDevice) {
        self.connect_then(device, |_, _| {});
    }

    /// [`Self::connect`], then `then` with the player held and whether it joined the receiver
    /// left running rather than launching one. Not called when the connect fails or a newer
    /// one came first.
    pub(crate) fn connect_then(
        self: &Arc<Self>,
        device: CastDevice,
        then: impl FnOnce(&Arc<CastPlayer>, bool) + Send + 'static,
    ) {
        let (generation, old) = {
            let mut session = lock(&self.session);
            let same = session.device.as_ref().is_some_and(|d| d.id == device.id);
            let alive = session.player.as_ref().is_some_and(|p| p.is_alive());
            if same && (session.connecting || alive) {
                return;
            }
            let generation = session.generation + 1;
            let old = std::mem::take(&mut *session).player;
            *session = Session {
                device: Some(device.clone()),
                connecting: true,
                generation,
                ..Session::default()
            };
            (generation, old)
        };
        if let Some(old) = old {
            self.spawn(async move {
                let _ = old.quit().await;
            });
        }
        // The card is for this TV from now, with nothing starting on it yet.
        *lock(&self.cast) = Current { device: Some(device.clone()), ..Current::default() };
        self.bump();
        let left = self.left().filter(|left| left.device == device.id);
        let hub = Arc::clone(self);
        self.spawn(async move {
            if let Some(left) = left {
                match CastPlayer::join(hub.setup(device.clone()), &left.session).await {
                    Ok(Some((player, _events))) => {
                        player.use_receiver(hub.receiver());
                        tracing::info!("joined the receiver this device left running");
                        let player = Arc::new(player);
                        if hub.connected(generation, Ok(Arc::clone(&player))) {
                            then(&player, true);
                        }
                        return;
                    }
                    // The receiver closed meanwhile: a new one launches.
                    Ok(None) => {}
                    Err(failure) => {
                        hub.connected(generation, Err(failure));
                        return;
                    }
                }
            }
            let connected = async {
                let (player, _events) = CastPlayer::connect(hub.setup(device)).await?;
                player.use_receiver(hub.receiver());
                player.launch().await?;
                Ok::<_, CastFailure>(Arc::new(player))
            }
            .await;
            if let Ok(player) = &connected {
                if hub.connected(generation, Ok(Arc::clone(player))) {
                    then(player, false);
                }
            } else {
                hub.connected(generation, connected);
            }
        });
    }

    /// A connect finished: the session holds the player, or says why it couldn't. Whether it
    /// holds it.
    fn connected(
        self: &Arc<Self>,
        generation: u64,
        connected: Result<Arc<CastPlayer>, CastFailure>,
    ) -> bool {
        {
            let mut session = lock(&self.session);
            if session.generation != generation {
                // A disconnect or another connect came first.
                drop(session);
                if let Ok(player) = connected {
                    self.spawn(async move {
                        let _ = player.quit().await;
                    });
                }
                return false;
            }
            session.connecting = false;
            match &connected {
                Ok(player) => session.player = Some(Arc::clone(player)),
                Err(failure) => session.failure = Some(failure.clone()),
            }
        }
        match connected {
            Ok(player) => {
                tracing::info!(device = %player.device().name, "cast session connected");
                self.attach(&player);
                self.follow(&player);
                self.leave(&player);
            }
            Err(failure) => {
                tracing::warn!(%failure, "couldn't connect to the TV");
                self.failed(failure);
                self.bump();
                return false;
            }
        }
        self.bump();
        true
    }

    /// Watch the held player for the TV ending the session by itself.
    fn follow(self: &Arc<Self>, player: &Arc<CastPlayer>) {
        let mut status = player.status();
        let hub = Arc::downgrade(self);
        let held = Arc::downgrade(player);
        tokio::spawn(async move {
            // Whether something played when it ended: a receiver that closes itself while idle
            // ends the session quietly.
            let mut busy = false;
            // The episode last seen playing, for Play here once the playback has let it go.
            let mut seen: Option<SessionItem> = None;
            loop {
                let now = status.borrow_and_update().clone();
                if now.state == CastState::Ended {
                    if let (Some(hub), Some(player)) = (hub.upgrade(), held.upgrade()) {
                        let lost = matches!(now.failure, Some(CastFailure::Lost(_)));
                        // Kaeri's receiver closes itself after a while with nothing playing,
                        // which reads as a stop on the TV: idle, either ends quietly.
                        let quiet =
                            lost || matches!(now.failure, Some(CastFailure::StoppedElsewhere));
                        let failure = now.failure.filter(|_| busy || !quiet);
                        hub.session_ended(&player, failure, lost, busy, seen);
                    }
                    return;
                }
                if let Some(hub) = hub.upgrade()
                    && let Some(item) =
                        Self::item(&hub.view(), Some(&now)).filter(|item| item.id.is_some())
                {
                    seen = Some(item);
                }
                busy = now.step.is_some()
                    || matches!(
                        now.state,
                        CastState::Playing | CastState::Paused | CastState::Buffering
                    );
                if status.changed().await.is_err() {
                    return;
                }
            }
        });
    }

    /// The TV ended the session `player` held: closed the receiver, went away, or someone else
    /// took it over. A connection `lost` mid-episode, after the player's own tries to connect
    /// again, may have left the receiver playing, so the TV found again joins it; an idle one is
    /// let go.
    fn session_ended(
        &self,
        player: &Arc<CastPlayer>,
        failure: Option<CastFailure>,
        lost: bool,
        busy: bool,
        seen: Option<SessionItem>,
    ) {
        let item = Self::item(&self.view(), Some(&player.current()))
            .filter(|item| item.id.is_some())
            .or(seen);
        {
            let mut session = lock(&self.session);
            if !session.player.as_ref().is_some_and(|p| Arc::ptr_eq(p, player)) {
                return;
            }
            tracing::info!(?failure, "the TV ended the cast session");
            session.player = None;
            session.ended = item.filter(|_| failure.is_some());
            if failure.is_none() {
                session.device = None;
            }
            session.failure = failure;
        }
        if !lost || !busy {
            self.forget_left();
        }
        self.end();
    }

    /// Let the held TV go: the receiver closes and the TV goes back to what it showed. Whether
    /// a session was held or connecting.
    pub fn disconnect(&self) -> bool {
        let (player, held) = {
            let mut session = lock(&self.session);
            let held = session.device.is_some();
            let generation = session.generation + 1;
            let player = std::mem::take(&mut *session).player;
            session.generation = generation;
            (player, held)
        };
        if let Some(player) = player {
            self.spawn(async move {
                if let Err(e) = player.quit().await {
                    tracing::debug!(error = %e, "the TV didn't take the disconnect");
                }
            });
        }
        if held {
            self.forget_left();
            self.bump();
        }
        held
    }

    /// The held player, when it is for the TV with this TXT `id` and still up.
    pub fn held(&self, device: &str) -> Option<Arc<CastPlayer>> {
        let session = lock(&self.session);
        session.player.clone().filter(|p| p.device().id == device && p.is_alive())
    }

    /// The TV a session holds or is connecting to, which every play goes to.
    pub fn session_device(&self) -> Option<CastDevice> {
        let session = lock(&self.session);
        session.device.clone().filter(|_| {
            session.connecting || session.player.as_ref().is_some_and(|p| p.is_alive())
        })
    }

    /// Whether `player` is the session's.
    pub(crate) fn holds(&self, player: &Arc<CastPlayer>) -> bool {
        lock(&self.session).player.as_ref().is_some_and(|p| Arc::ptr_eq(p, player))
    }

    /// A cast connects to a TV outside the session, until the guard drops once its player is
    /// attached or the connect failed.
    pub(crate) fn opening(&self) -> Opening {
        self.opening.fetch_add(1, Ordering::Relaxed);
        Opening(Arc::clone(&self.opening))
    }

    /// A link opens on the held player: it is the one that plays there now.
    pub(crate) fn claim(&self) -> u64 {
        let link = self.links.fetch_add(1, Ordering::Relaxed) + 1;
        lock(&self.session).link = link;
        link
    }

    /// Whether the link `link` is the one that plays on the held player.
    pub(crate) fn owns(&self, link: u64) -> bool {
        lock(&self.session).link == link
    }

    /// A playback on the held TV ended: the receiver stays up, and nothing is starting.
    pub(crate) fn played_out(&self) {
        {
            let mut cast = lock(&self.cast);
            cast.starting = None;
            cast.loading = false;
            cast.next = None;
            cast.skip = None;
            cast.undo = None;
        }
        self.bump();
    }

    /// Wait for a connect to finish, up to `wait`: a play asked for while the TV connects goes
    /// to it once it is up.
    pub async fn connecting(&self, wait: Duration) {
        let deadline = tokio::time::Instant::now() + wait;
        let mut changes = self.changes();
        while lock(&self.session).connecting {
            if tokio::time::timeout_at(deadline, changes.changed()).await.is_err() {
                return;
            }
        }
    }

    /// The cast session as the apps draw it.
    pub fn session(&self) -> SessionView {
        let (device, player, connecting, failure) = {
            let session = lock(&self.session);
            (
                session.device.clone(),
                session.player.clone(),
                session.connecting,
                session.failure.clone(),
            )
        };
        let view = self.view();
        let status = player.as_ref().map(|p| p.current());
        let item = Self::item(&view, status.as_ref());
        let state = if device.is_none() {
            SessionState::Idle
        } else if connecting {
            SessionState::Connecting
        } else if player.as_ref().is_some_and(|p| p.is_alive()) {
            if item.is_some() || view.starting.is_some() {
                SessionState::Playing
            } else {
                SessionState::Connected
            }
        } else if failure.is_some() {
            SessionState::Error
        } else {
            SessionState::Idle
        };
        let queue = view
            .next
            .iter()
            .filter(|_| state == SessionState::Playing)
            .map(|next| SessionItem {
                id: item.as_ref().and_then(|i| i.id),
                title: item.as_ref().map(|i| i.title.clone()).unwrap_or_default(),
                episode: Some(next.number.clone()),
                episode_title: next.title.clone(),
                image: next.still.clone(),
                ..SessionItem::default()
            })
            .collect();
        let button = match state {
            SessionState::Connecting => CastButton::Connecting,
            SessionState::Connected | SessionState::Playing => CastButton::Connected,
            SessionState::Idle | SessionState::Error
                if self.shows_tvs() && !lock(&self.devices).is_empty() =>
            {
                CastButton::Available
            }
            SessionState::Idle | SessionState::Error => CastButton::Hidden,
        };
        SessionView {
            state,
            button,
            rev: self.rev(),
            device: device.clone().filter(|_| state != SessionState::Idle),
            item: item.filter(|_| state == SessionState::Playing),
            queue,
            notice: failure.as_ref().filter(|_| state == SessionState::Error).and_then(|f| {
                let name = device.as_ref()?.name.clone();
                Some(crate::playback::link::cast::notice(f, &name))
            }),
            note: view
                .note
                .filter(|_| matches!(state, SessionState::Playing | SessionState::Connected)),
            failure: failure.filter(|_| state == SessionState::Error),
            level: status.as_ref().and_then(|s| s.level),
            muted: status.as_ref().and_then(|s| s.muted),
            volume: status.as_ref().map_or(VolumeControl::Level, |s| s.volume),
            own_receiver: status
                .as_ref()
                .and_then(|s| s.receiver.as_deref())
                .is_some_and(|r| r != DEFAULT_MEDIA_RECEIVER),
            reconnecting: status.as_ref().is_some_and(|s| s.reconnecting)
                && matches!(state, SessionState::Playing | SessionState::Connected),
        }
    }

    /// What plays on the TV: what this device started, or what a joined receiver plays.
    fn item(view: &CastView, status: Option<&CastStatus>) -> Option<SessionItem> {
        let now = view.state.as_ref().and_then(|s| s.now_playing.clone());
        match (&now, status) {
            (Some(now), status) => Some(SessionItem {
                id: Some(now.id),
                title: now.title.clone(),
                episode: Some(now.episode.clone()),
                episode_title: now.episode_title.clone(),
                image: status.and_then(|s| s.metadata.as_ref()?.image.clone()),
                position: status.map_or(now.position, |s| s.position),
                duration: status.and_then(|s| s.duration).or(now.duration),
                paused: status.map_or(now.paused, |s| s.state == CastState::Paused),
                waiting: status.and_then(waited),
                loading: now.loading
                    || status.is_none_or(|s| {
                        s.step.is_some() || s.plan.is_none() && s.metadata.is_none()
                    }),
            }),
            // What the receiver plays that this device didn't start: a session joined again.
            (None, Some(status))
                if matches!(
                    status.state,
                    CastState::Playing | CastState::Paused | CastState::Buffering
                ) =>
            {
                status.metadata.as_ref().map(|metadata| SessionItem {
                    id: None,
                    title: metadata.series.clone(),
                    episode: None,
                    episode_title: Some(metadata.title.clone()).filter(|t| !t.is_empty()),
                    image: metadata.image.clone(),
                    position: status.position,
                    duration: status.duration,
                    paused: status.state == CastState::Paused,
                    waiting: waited(status),
                    loading: false,
                })
            }
            _ => None,
        }
    }

    /// What played when the TV or the network ended the session: Play here plays it on.
    pub fn ended_item(&self) -> Option<SessionItem> {
        lock(&self.session).ended.clone()
    }

    /// Note the receiver `player` launched, so a later run joins it again.
    fn leave(&self, player: &CastPlayer) {
        let Some(app) = player.receiver() else { return };
        self.write_left(Some(Left {
            device: player.device().id.clone(),
            session: app.session_id,
        }));
    }

    fn forget_left(&self) {
        self.write_left(None);
    }

    fn write_left(&self, left: Option<Left>) {
        if let Some(dir) = &self.dir {
            let path = dir.join(LEFT_FILE);
            let written = match &left {
                Some(left) => serde_json::to_vec(left)
                    .map_err(io::Error::other)
                    .and_then(|bytes| std::fs::write(&path, bytes)),
                None => match std::fs::remove_file(&path) {
                    Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                    other => other,
                },
            };
            if let Err(e) = written {
                tracing::debug!(error = %e, "couldn't note the cast session");
            }
        }
        *lock(&self.left) = Some(left);
    }

    /// The session left running, read the first time it is needed.
    fn left(&self) -> Option<Left> {
        let mut left = lock(&self.left);
        left.get_or_insert_with(|| {
            let path = self.dir.as_ref()?.join(LEFT_FILE);
            serde_json::from_slice(&std::fs::read(path).ok()?).ok()
        })
        .clone()
    }

    /// The TXT `id` of the TV a session was left running on, for the app to browse for at
    /// launch: once found, the session is joined again. Nothing while a cast connects or plays:
    /// the note is then that cast's own, kept for a later run, and nothing waits to be joined.
    pub fn left_on(&self) -> Option<String> {
        self.left().filter(|_| !self.casting_now()).map(|left| left.device)
    }

    /// The TXT `id` the note of a session left running names, whatever casts now.
    #[cfg(test)]
    pub(crate) fn noted_on(&self) -> Option<String> {
        self.left().map(|left| left.device)
    }

    /// A cast connects outside the session, or the card follows a player that is up.
    fn casting_now(&self) -> bool {
        self.opening.load(Ordering::Relaxed) > 0
            || self.player().is_some_and(|player| player.is_alive())
    }

    /// Join the session left running on `device`, when its receiver still runs and no session
    /// is held, one that ended lost included: the browse found that TV again. Nothing otherwise,
    /// and nothing while a cast connects or plays: one begun from Play on TV or a play sent to a
    /// TV holds no session, and joining beside it would start its card over and hold the TV twice.
    pub fn rejoin(self: &Arc<Self>, device: &CastDevice) {
        let Some(left) = self.left().filter(|left| left.device == device.id) else { return };
        if self.casting_now() {
            return;
        }
        let generation = {
            let mut session = lock(&self.session);
            if session.connecting || session.player.is_some() {
                return;
            }
            let generation = session.generation + 1;
            *session = Session {
                device: Some(device.clone()),
                connecting: true,
                generation,
                ..Session::default()
            };
            generation
        };
        *lock(&self.cast) = Current { device: Some(device.clone()), ..Current::default() };
        self.bump();
        let hub = Arc::clone(self);
        let device = device.clone();
        self.spawn(async move {
            let joined = CastPlayer::join(hub.setup(device), &left.session).await;
            match joined {
                Ok(Some((player, _events))) => {
                    player.use_receiver(hub.receiver());
                    tracing::info!("joined the cast session left running");
                    hub.connected(generation, Ok(Arc::new(player)));
                }
                // The receiver closed meanwhile, or the TV can't be reached: nothing to join,
                // and nothing to say.
                Ok(None) | Err(_) => {
                    hub.forget_left();
                    let ours = {
                        let mut session = lock(&hub.session);
                        let ours = session.generation == generation;
                        if ours {
                            *session = Session { generation, ..Session::default() };
                        }
                        ours
                    };
                    if ours {
                        *lock(&hub.cast) = Current::default();
                        hub.bump();
                    }
                }
            }
        });
    }

    /// Whether the TV picker lists TVs with Chromecast built in, from Settings.
    pub fn set_show_tvs(&self, show: bool) {
        if self.show_tvs.swap(show, Ordering::Relaxed) != show {
            if !show {
                lock(&self.devices).clear();
            }
            self.bump();
        }
    }

    pub fn shows_tvs(&self) -> bool {
        self.show_tvs.load(Ordering::Relaxed)
    }

    /// The language the front end speaks, as a tag, for what the TV shows.
    pub fn set_language(&self, language: Option<String>) {
        *lock(&self.language) = language;
        if let Some(player) = self.player() {
            player.set_greeting(self.language(), self.device_word());
        }
    }

    pub fn language(&self) -> Option<String> {
        lock(&self.language).clone()
    }

    /// What Kaeri's receiver calls this device in its own strings ("Waiting for the iPhone"):
    /// `iphone`, `ipad`, `phone`, `tablet`, `mac` or `computer`.
    pub fn set_device_word(&self, word: Option<String>) {
        *lock(&self.device_word) = word;
        if let Some(player) = self.player() {
            player.set_greeting(self.language(), self.device_word());
        }
    }

    pub fn device_word(&self) -> Option<String> {
        lock(&self.device_word).clone()
    }

    /// What Kaeri's receiver says from now on, past the hub's own handling.
    pub fn heard(&self) -> broadcast::Receiver<FromReceiver> {
        self.heard.subscribe()
    }

    /// Follow what `player`'s receiver says, once per player.
    fn listen(self: &Arc<Self>, player: &Arc<CastPlayer>) {
        {
            let mut listening = lock(&self.listening);
            if Weak::ptr_eq(&listening, &Arc::downgrade(player)) {
                return;
            }
            *listening = Arc::downgrade(player);
        }
        player.set_greeting(self.language(), self.device_word());
        let mut heard = player.heard();
        let hub = Arc::downgrade(self);
        let tv = player.device().clone();
        tokio::spawn(async move {
            loop {
                let said = match heard.recv().await {
                    Ok(said) => said,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return,
                };
                let Some(hub) = hub.upgrade() else { return };
                if said == FromReceiver::SubtitlesPlain {
                    match hub.memory().told_subtitles_plain(&tv.id) {
                        Ok(true) => {
                            lock(&hub.cast).note =
                                Some(kaeri_model::Msg::NoticeCastSubtitlesPlain {
                                    name: tv.name.clone(),
                                });
                            hub.bump();
                        }
                        Ok(false) => {}
                        Err(e) => tracing::warn!(error = %e, "couldn't note the TV"),
                    }
                }
                let _ = hub.heard.send(said);
            }
        });
    }

    /// What the engine has learned of each TV, read the first time it is needed.
    pub fn memory(&self) -> Arc<CastMemory> {
        Arc::clone(self.memory.get_or_init(|| {
            let Some(dir) = &self.dir else { return Arc::new(CastMemory::memory()) };
            if let Err(e) = std::fs::create_dir_all(dir) {
                tracing::warn!(error = %e, "couldn't make the remote's directory");
            }
            Arc::new(CastMemory::open(dir).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "couldn't read what was learned of TVs");
                CastMemory::memory()
            }))
        }))
    }

    pub fn upstream(&self) -> Arc<dyn Upstream> {
        Arc::clone(&self.upstream)
    }

    pub fn timings(&self) -> Timings {
        self.timings
    }

    /// The front end's AAC encoder, which turns AC-3, E-AC-3 and other audio a TV can't play
    /// into AAC. Until one is set, such audio is uncastable.
    pub fn set_encoder(&self, encoder: Arc<dyn AacEncoder>) {
        *lock(&self.encoder) = encoder;
    }

    pub fn encoder(&self) -> Arc<dyn AacEncoder> {
        Arc::clone(&lock(&self.encoder))
    }

    /// A device the browse found, from its TXT record. Speaker groups and receivers without a
    /// screen are left out.
    pub fn found(&self, device: CastDevice) {
        if !device.shown() || !self.shows_tvs() {
            return;
        }
        lock(&self.devices).insert(device.id.clone(), device);
        self.bump();
    }

    /// The browse lost the device with this service instance name.
    pub fn lost(&self, instance: &str) {
        let removed = {
            let mut devices = lock(&self.devices);
            let before = devices.len();
            devices.retain(|_, d| d.instance != instance);
            before != devices.len()
        };
        if removed {
            self.bump();
        }
    }

    /// Forget what the browse found, as the picker closes.
    pub fn forget_found(&self) {
        let had = !std::mem::take(&mut *lock(&self.devices)).is_empty();
        if had {
            self.bump();
        }
    }

    /// The devices found, by name.
    pub fn devices(&self) -> Vec<CastDevice> {
        let mut devices: Vec<_> = lock(&self.devices).values().cloned().collect();
        devices.sort_by(|a, b| a.name.cmp(&b.name));
        devices
    }

    /// The device found with this TXT `id`, or the one casting now.
    pub fn device(&self, id: &str) -> Option<CastDevice> {
        lock(&self.devices)
            .get(id)
            .cloned()
            .or_else(|| lock(&self.cast).device.clone().filter(|d| d.id == id))
    }

    /// The viewer cast to `device`: the card is for it from now.
    pub fn begin(&self, device: CastDevice) {
        *lock(&self.cast) = Current {
            device: Some(device),
            starting: Some(Instant::now()),
            ..Current::default()
        };
        self.bump();
    }

    /// The episode the cast was asked to play, for Try again to ask for it once more.
    pub fn asked(&self, id: AnilistId, episode: String) {
        lock(&self.cast).asked = Some((id, episode));
    }

    /// The viewer stopped casting, or it ended for good.
    pub fn end(&self) {
        {
            let mut cast = lock(&self.cast);
            cast.over = true;
            cast.player = Weak::new();
        }
        self.bump();
    }

    /// The cast `player` closed, when the card is still for it: casting ends, so the card isn't
    /// left starting a file that never comes. A cast begun since, on this TV or another, goes on.
    pub(crate) fn end_for(&self, player: &Arc<CastPlayer>) {
        let current = Weak::ptr_eq(&lock(&self.cast).player, &Arc::downgrade(player));
        if current {
            self.end();
        }
    }

    /// The cast couldn't reach the TV: the card says why, and casting ends, so the next play
    /// opens on this device.
    pub(crate) fn failed(&self, failure: CastFailure) {
        {
            let mut cast = lock(&self.cast);
            let mut status = cast.status.clone().unwrap_or_default();
            status.state = CastState::Ended;
            status.failure = Some(failure);
            cast.status = Some(status);
            cast.over = true;
            cast.player = Weak::new();
        }
        self.bump();
    }

    /// Whether a cast was asked for and has not ended, or a session holds a TV.
    pub fn casting(&self) -> bool {
        let cast = lock(&self.cast);
        (cast.device.is_some() && !cast.over) || {
            drop(cast);
            self.session_device().is_some()
        }
    }

    /// Whether the card follows `player`.
    pub(crate) fn attached(&self, player: &Arc<CastPlayer>) -> bool {
        Weak::ptr_eq(&lock(&self.cast).player, &Arc::downgrade(player))
    }

    /// The player a cast opened, which the card's volume and mute go to.
    pub(crate) fn attach(self: &Arc<Self>, player: &Arc<CastPlayer>) {
        self.listen(player);
        let mut status = player.status();
        lock(&self.cast).player = Arc::downgrade(player);
        let mut noted = Noted::default();
        let now = status.borrow_and_update().clone();
        self.note_left(player, &now, &mut noted);
        self.note_fallback(player, &now);
        self.set_status(now);
        let hub = Arc::downgrade(self);
        let held = Arc::downgrade(player);
        tokio::spawn(async move {
            while status.changed().await.is_ok() {
                let (Some(hub), Some(player)) = (hub.upgrade(), held.upgrade()) else { return };
                let now = status.borrow_and_update().clone();
                hub.note_left(&player, &now, &mut noted);
                hub.note_fallback(&player, &now);
                hub.set_status(now);
            }
        });
    }

    /// Kaeri's receiver didn't start on `player`'s cast, so the TV plays it in its own player:
    /// the note says so once for each time it happens, however often the card is opened again.
    fn note_fallback(&self, player: &Arc<CastPlayer>, status: &CastStatus) {
        if !status.fallen_back {
            return;
        }
        {
            let mut told = lock(&self.told_fallback);
            if Weak::ptr_eq(&told, &Arc::downgrade(player)) {
                return;
            }
            *told = Arc::downgrade(player);
        }
        tracing::info!("the TV plays this cast in its own player");
        lock(&self.cast).note =
            Some(kaeri_model::Msg::NoticeCastFallback { name: player.device().name.clone() });
    }

    /// Keep the note of the receiver a cast plays on, a cast begun from Play on TV included, so a
    /// run the system ended joins it again once the TV is found: noted as the receiver comes up,
    /// let go as the cast ends, unless the connection was lost mid-episode.
    fn note_left(&self, player: &CastPlayer, status: &CastStatus, noted: &mut Noted) {
        if status.state == CastState::Ended {
            let lost = matches!(status.failure, Some(CastFailure::Lost(_)));
            if let Some(session) = noted.session.take()
                && !(lost && noted.busy)
                && self.left().is_some_and(|left| left.session == session)
            {
                self.forget_left();
            }
            return;
        }
        noted.busy = status.step.is_some()
            || matches!(
                status.state,
                CastState::Playing | CastState::Paused | CastState::Buffering
            );
        if let Some(app) = player.receiver()
            && noted.session.as_deref() != Some(app.session_id.as_str())
        {
            self.leave(player);
            noted.session = Some(app.session_id);
        }
    }

    pub(crate) fn set_status(&self, status: CastStatus) {
        let play_next = {
            let mut cast = lock(&self.cast);
            if status.step.is_some() {
                cast.loading = true;
            }
            let playing = matches!(
                status.state,
                CastState::Playing | CastState::Paused | CastState::Buffering
            );
            if cast.loading && playing && status.plan.is_some() && status.step.is_none() {
                cast.starting = None;
                cast.loading = false;
            }
            let (position, duration) = (status.position, status.duration);
            if let Some(undo) = cast.undo.as_mut().filter(|_| playing) {
                let landed = *undo.landed.get_or_insert(position);
                if position - landed >= UNDO_PLAYBACK
                    || position < landed - 1.0
                    || undo.since.elapsed() >= UNDO_LONGEST
                {
                    cast.undo = None;
                }
            }
            // The count ran out: the TV plays the next episode, as a hosted player asks.
            let ran_out = status.state == CastState::Playing
                && cast.next.as_ref().is_some_and(|next| {
                    !next.played && next.remaining(position, duration).is_some_and(|r| r <= 0.0)
                });
            if ran_out && let Some(next) = cast.next.as_mut() {
                next.played = true;
            }
            // Kaeri's receiver draws Up Next's card on the TV too, while this one shows it.
            let shown = cast.next.as_ref().filter(|next| next.shown(position, duration));
            let tell = (shown.is_some() != cast.told_next).then(|| {
                shown.map(|next| UpNextCard {
                    number: Some(next.number.clone()),
                    title: next.title.clone(),
                    still: next.still.clone(),
                    seconds: next.remaining(position, duration).map_or(0, |r| r.ceil() as u32),
                })
            });
            if tell.is_some() {
                cast.told_next = !cast.told_next;
            }
            cast.status = Some(status);
            let player = cast.player.upgrade();
            if let (Some(tell), Some(player)) = (tell, player.as_ref()) {
                player.tell_up_next(tell);
            }
            ran_out.then_some(player).flatten()
        };
        if let Some(player) = play_next {
            tracing::info!("Up Next's count ended on the cast; playing the next episode");
            player.say(PlaybackEvent::Script(ScriptEvent::PlayNext));
        }
        self.bump();
    }

    /// What the session shows a player: the cast keeps what the sender's card draws, Up Next,
    /// the skip on offer and the Undo note, and that the next episode is starting.
    pub(crate) fn overlay(&self, overlay: &crate::playback::link::Overlay) {
        use crate::playback::{link::Overlay, offered};
        use kaeri_model::playback::SkipKind;
        let mut skipped = None;
        {
            let mut cast = lock(&self.cast);
            match overlay {
                Overlay::Next { card: Some(card), still, countdown } => {
                    // The same episode keeps what the viewer did to its card.
                    let kept = cast.next.take().filter(|next| next.number == card.number);
                    cast.next = Some(UpNext {
                        number: card.number.clone(),
                        title: card.title.clone(),
                        still: still.clone(),
                        auto: card.auto,
                        at: card.at,
                        countdown: *countdown,
                        cancelled: kept.as_ref().is_some_and(|k| k.cancelled),
                        played: kept.as_ref().is_some_and(|k| k.played),
                        restarted_at: kept.and_then(|k| k.restarted_at),
                    });
                }
                Overlay::Next { card: None, .. } => cast.next = None,
                Overlay::SkipOffer { kind, to } => cast.skip = Some((offered(*kind), *to)),
                Overlay::SkipCleared => cast.skip = None,
                Overlay::Skipped { kind, from, .. } => {
                    // Kaeri's receiver shows the skip on the TV too.
                    skipped = match offered(*kind) {
                        SkipKind::Opening => Some("opening"),
                        SkipKind::Ending => Some("ending"),
                        SkipKind::Recap => None,
                    };
                    cast.undo = Some(Undo {
                        kind: offered(*kind),
                        to: *from,
                        landed: None,
                        since: Instant::now(),
                    });
                }
                // A file is starting on the TV: the first, or the next after one ended.
                Overlay::Starting { id, episode, .. } => {
                    cast.asked = Some((*id, episode.clone()));
                    if cast.starting.is_none() {
                        cast.starting = Some(Instant::now());
                        cast.loading = false;
                    }
                    cast.skip = None;
                    cast.undo = None;
                    cast.note = None;
                }
                _ => return,
            }
        }
        if let Some(part) = skipped
            && let Some(player) = self.player()
        {
            player.tell_skipped(part);
        }
        self.bump();
    }

    /// The TV was asked to seek to `target`: an Up Next count it lands in runs again from its
    /// full length there.
    pub(crate) fn seeked(&self, target: f64) {
        {
            let mut cast = lock(&self.cast);
            let duration = cast.status.as_ref().and_then(|s| s.duration);
            let Some(next) = cast.next.as_mut().filter(|n| !n.played) else { return };
            next.restarted_at =
                next.shows_at(duration).filter(|start| target >= *start).map(|_| target);
        }
        self.bump();
    }

    /// The viewer said no to the next episode, or asked for it now: the card comes down.
    pub fn next_answered(&self, play: bool) {
        {
            let mut cast = lock(&self.cast);
            let Some(next) = cast.next.as_mut() else { return };
            if play {
                next.played = true;
            } else {
                next.cancelled = true;
            }
        }
        self.bump();
    }

    /// The Undo note was taken.
    pub fn undo_taken(&self) -> Option<Undo> {
        let undo = lock(&self.cast).undo.take();
        if undo.is_some() {
            self.bump();
        }
        undo
    }

    /// The player casting now, while it is.
    pub fn player(&self) -> Option<Arc<CastPlayer>> {
        lock(&self.cast).player.upgrade()
    }

    /// The device moved to another network, or back to one: a connection to the TV being made
    /// again tries at once on the new one.
    pub fn network_changed(&self) {
        let held = lock(&self.session).player.clone();
        let casting = self.player();
        if let Some(player) = &held {
            player.network_changed();
        }
        if let Some(player) =
            casting.filter(|p| !held.as_ref().is_some_and(|h| Arc::ptr_eq(h, p)))
        {
            player.network_changed();
        }
    }

    /// An engine event on its way to the front end, for the card's title, episode and skips.
    pub fn observe(&self, event: &Event) {
        let changed = {
            let mut cast = lock(&self.cast);
            if cast.device.is_none() || !cast.folder.event(event) {
                false
            } else {
                cast.state = Some(cast.folder.publish());
                true
            }
        };
        if changed {
            self.bump();
        }
    }

    /// The cast as the card reads it now.
    pub fn view(&self) -> CastView {
        let cast = lock(&self.cast);
        let player = cast.player.upgrade();
        CastView {
            device: cast.device.clone(),
            connected: player.as_ref().is_some_and(|p| p.is_alive()),
            status: cast.status.clone(),
            state: cast.state.clone(),
            over: cast.over,
            starting: cast.starting,
            next: cast.next.clone(),
            skip: cast.skip,
            undo: cast.undo.clone(),
            asked: cast.asked.clone(),
            note: cast.note.clone(),
        }
    }

    /// Counts up whenever the devices or the cast read differently.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    /// How many times the cast has changed: a card's `rev`.
    pub fn rev(&self) -> u64 {
        *self.changes.borrow()
    }

    fn bump(&self) {
        self.changes.send_modify(|n| *n += 1);
    }
}

/// Whole seconds the TV has waited for the stream, while it waits.
pub(crate) fn waited(status: &CastStatus) -> Option<u32> {
    let since = status.waiting_since.filter(|_| status.state == CastState::Buffering)?;
    Some(u32::try_from(since.elapsed().as_secs()).unwrap_or(u32::MAX))
}

/// What the relay fetches upstream with: the sources client for the source's own hosts, with
/// the headers they want, and plain HTTP for the stream proxy and the torrent server on loopback.
struct HttpUpstream(HttpClient);

#[async_trait]
impl Upstream for HttpUpstream {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> io::Result<UpstreamResponse> {
        if is_loopback_url(url) {
            return PlainHttp.get(url, headers).await;
        }
        let mut request = self.0.sources().get(url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let response = request.send().await.map_err(io::Error::other)?;
        let status = response.status().as_u16();
        // Where the answer came from, when that is not where it was asked for.
        let landed = Some(response.url().to_string()).filter(|landed| landed != url);
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value.to_str().ok().map(|value| (name.as_str().to_owned(), value.to_owned()))
            })
            .collect();
        let body = response
            .bytes_stream()
            .map(|chunk| chunk.map(|bytes| bytes.to_vec()).map_err(io::Error::other));
        Ok(UpstreamResponse { status, headers, body: Box::pin(body), url: landed })
    }
}

/// The environment variable that names a development receiver app (the "Kaeri test" receiver on
/// the tunnel) in place of Kaeri's own.
pub const DEV_RECEIVER_ENV: &str = "KAERI_CAST_RECEIVER_APP";

/// `KAERI_CAST_RECEIVER_APP`, in the builds that honour it: the engine's tests and the Fixtures
/// and QA apps (`testing`), and the terminal (`dev-receiver`). A release app build never reads
/// it, so nothing on a viewer's machine can point their TV at another page.
#[cfg(any(feature = "testing", feature = "dev-receiver"))]
pub fn dev_receiver() -> Option<String> {
    dev_receiver_from(std::env::var(DEV_RECEIVER_ENV).ok())
}

/// Release app builds: never.
#[cfg(not(any(feature = "testing", feature = "dev-receiver")))]
pub fn dev_receiver() -> Option<String> {
    None
}

/// A Cast app ID as the variable gives it: eight hex digits, upper case. Anything else is
/// ignored, with a warning, so a typo falls back to Kaeri's own receiver.
#[cfg_attr(not(any(test, feature = "testing", feature = "dev-receiver")), allow(dead_code))]
fn dev_receiver_from(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_ascii_uppercase();
    if value.is_empty() {
        return None;
    }
    if value.len() == 8 && value.chars().all(|c| c.is_ascii_hexdigit()) {
        tracing::info!(receiver = %value, "launching the receiver {DEV_RECEIVER_ENV} names");
        Some(value)
    } else {
        tracing::warn!("{DEV_RECEIVER_ENV} is no Cast app ID; Kaeri's own receiver launches");
        None
    }
}

#[cfg(test)]
mod dev_receiver_tests {
    use super::dev_receiver_from;

    #[test]
    fn takes_an_app_id_and_ignores_anything_else() {
        assert_eq!(dev_receiver_from(Some(" 0a1b2c3d ".into())).as_deref(), Some("0A1B2C3D"));
        assert_eq!(dev_receiver_from(None), None);
        assert_eq!(dev_receiver_from(Some(String::new())), None);
        assert_eq!(dev_receiver_from(Some("https://receiver.example/".into())), None);
        assert_eq!(dev_receiver_from(Some("0A1B2C3".into())), None);
    }
}
