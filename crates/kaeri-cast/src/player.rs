//! A TV as the player a playback runs in: what the engine's `CastLink` wraps.
//!
//! The session drives every player in mpv's vocabulary. [`CastPlayer`] takes those calls and
//! turns them into Cast commands, and turns what the receiver reports into the same
//! [`PlaybackEvent`]s mpv's reports become, so resume, skips, Up Next and the history the session
//! writes work unchanged.
//!
//! A load is tried plan by plan, best first ([`crate::formats::cast_plans`]):
//! - A plan is refused when the receiver answers `LOAD_FAILED`, `LOAD_CANCELLED` or
//!   `INVALID_REQUEST`, sends `ERROR` or goes idle with `ERROR`. It stalls when its position
//!   hasn't moved [`Timings::movement`] after the load.
//! - A refusal counts against a format part only when the TV asked the relay for media, counted
//!   as the request arrives: a TV that never asked for the picture said nothing about its codec.
//! - Nothing counts while the TV waits on the source behind the relay, or after that source was
//!   slow or failed: a torrent that has only just started is not a codec the TV lacks. Nor does
//!   it count after the relay served media whose first bytes are no format a receiver plays. A stall
//!   with the source served in good time blames a copied audio codec alone, never the picture
//!   or the container.
//! - Before a file whose picture this TV hasn't played, a capability clip is loaded
//!   ([`crate::clips`]): a TV with no decoder for a file's picture can play its sound alone and
//!   report nothing wrong.
//! - A relayed plan whose bytes nobody fetched within [`Timings::fetch`] ends the cast: the TV
//!   can't reach this device (a firewall, a guest network), and no other plan would fare better.
//! - Until a plan plays, the session hears nothing; then `Loaded`, as from mpv.
//!
//! The receiver app is launched at the first load that has a plan, so a stream that can't be
//! cast never takes the TV over, unless a cast session launched it first ([`CastPlayer::launch`]).
//! It is Kaeri's own receiver when one is named, and the Default Media Receiver when there is
//! none, when Kaeri's doesn't launch, or when it closes before it played anything. Every cast
//! tries Kaeri's receiver first: a failure on one cast is never held against the next, and
//! [`CastStatus::fallen_back`] says when this one fell back. The media status is asked for every
//! [`Timings::poll`] for the whole cast, since the Default Media Receiver reports on changes
//! only. After [`Timings::idle_close`] paused, the cast ends and the TV goes back to what it
//! showed.

use std::{
    collections::HashMap,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use kaeri_playback::{
    PlaybackEvent,
    tracks::{Track, TrackKind, TrackPrint, choose_audio, language_matches},
};
use serde_json::{Value, json};
use tokio::{
    sync::{Mutex as AsyncMutex, broadcast, mpsc, watch},
    task::JoinHandle,
};

use crate::{
    aac::AacEncoder,
    channel::{CastError, Incoming, NS_CONNECTION, NS_MEDIA, NS_RECEIVER},
    client::{
        App, DEFAULT_MEDIA_RECEIVER, Device, KAERI_PROTOCOL, KAERI_RECEIVER, Load, Media,
        MediaStatus, Metadata, NS_KAERI, ReceiverStatus, TextTrack, VolumeControl,
    },
    clips::{Clip, clip},
    device::{CastDevice, local_address_towards, same_network},
    fetch::{RangeFile, Upstream},
    formats::{
        AudioCodec, AudioPlan, AudioTrack, CastPlan, CastRoute, CastSource, Delivery,
        MediaInfo, SourceKind, Uncastable, cast_plans, pass_through_plans,
    },
    master,
    memory::CastMemory,
    mkv, probe,
    relay::{Dynamic, Made, Relay, rewrite_playlist},
    remux::{AudioMode, RemuxOptions, Remuxer, Source},
    vtt,
};

/// How long the cast waits on each thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timings {
    /// Between status polls.
    pub poll: Duration,
    /// From an accepted load to a position that moves, or the plan is refused.
    pub movement: Duration,
    /// From the load to the TV's first fetch from the relay, or it never fetches.
    pub fetch: Duration,
    /// Paused this long, the cast ends.
    pub idle_close: Duration,
    /// A wait on the source this long, for a request of the TV's, is a slow source: what the
    /// TV does meanwhile says nothing about the format.
    pub slow_source: Duration,
    /// How long a plan may wait on a slow source before it is given up, unlearnt.
    pub patience: Duration,
    /// The TV said nothing this long, heartbeat and status polls unanswered: it is gone.
    pub silence: Duration,
    /// How long Kaeri's receiver has to say `ready` once launched, before the Default Media
    /// Receiver takes its place.
    pub ready: Duration,
    /// How long a dropped connection is tried again before the cast counts as lost: the
    /// receiver keeps playing meanwhile, and a new connection joins its session.
    pub reconnect: Duration,
    /// How long one try at connecting again may take, from the TCP connection to the TV's first
    /// status: a TV on the same network answers in well under a second, and a try that waits on
    /// a network the phone has left gives way to the next.
    pub retry: Duration,
    /// How long the receiver's statuses are heard after the receiver app left the TV before it
    /// counts as stopped there: another sender's app comes up a moment after this one closes,
    /// and a TV that went away is left to [`Timings::reconnect`].
    pub gone: Duration,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            poll: Duration::from_secs(1),
            movement: Duration::from_secs(8),
            fetch: Duration::from_secs(15),
            idle_close: Duration::from_secs(10 * 60),
            slow_source: Duration::from_secs(4),
            patience: Duration::from_secs(60),
            silence: crate::channel::SILENCE,
            ready: Duration::from_secs(8),
            reconnect: Duration::from_secs(20),
            retry: Duration::from_secs(2),
            gone: Duration::from_secs(4),
        }
    }
}

/// Everything a cast to one TV is set up with.
#[derive(Clone)]
pub struct CastSetup {
    pub device: CastDevice,
    pub memory: Arc<CastMemory>,
    pub upstream: Arc<dyn Upstream>,
    pub encoder: Arc<dyn AacEncoder>,
    pub timings: Timings,
}

/// A subtitle file on this device, side-loaded as a WebVTT track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastSubtitle {
    pub path: PathBuf,
    pub title: Option<String>,
    pub language: Option<String>,
}

/// One file to cast.
#[derive(Debug, Clone, PartialEq)]
pub struct CastLoad {
    /// The stream as its source gave it.
    pub source: CastSource,
    /// What a local player would open: the stream proxy's or the torrent server's loopback address,
    /// or the file's path.
    pub url: String,
    /// The headers `url` wants.
    pub headers: Vec<(String, String)>,
    pub start: Option<f64>,
    pub metadata: Metadata,
    pub subtitles: Vec<CastSubtitle>,
    /// Which of `subtitles` is on.
    pub subtitle: Option<usize>,
    /// The language the audio track is chosen by.
    pub audio_language: Option<String>,
    /// The audio track the viewer picked, by its place among the file's, over the language.
    pub audio_track: Option<usize>,
    /// The audio track the series remembers, found again by what it is, as the app's player
    /// finds it: ahead of the language.
    pub audio_memory: Option<TrackPrint>,
}

/// Why a cast didn't play, or stopped.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CastFailure {
    #[error(transparent)]
    Uncastable(#[from] Uncastable),
    /// The TV never fetched from the relay.
    #[error("the TV never fetched the stream")]
    NeverFetches,
    /// Every plan was refused.
    #[error("the TV couldn't play this stream")]
    Refused,
    /// Another device started something on the TV.
    #[error("another device took over the TV")]
    TakenOver,
    /// This device isn't on the TV's network, or reaches it through a VPN.
    #[error("this device isn't on the TV's network")]
    NotSameNetwork,
    /// The TV stopped answering.
    #[error("the TV went away: {0}")]
    Lost(String),
    /// The receiver app was stopped from outside while the TV still answered: its own remote,
    /// another phone's cast controls or the system's. Nothing runs on the TV afterwards.
    #[error("the cast was stopped on the TV")]
    StoppedElsewhere,
    /// Kaeri's receiver couldn't get part of the file from the source mid-play.
    #[error("the source didn't answer for part of the episode")]
    Segment,
    /// Kaeri's receiver lost the relay on this device mid-play.
    #[error("the TV lost the stream from this device")]
    LostStream,
}

/// Where a cast is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CastState {
    #[default]
    Connecting,
    Loading,
    Playing,
    Paused,
    Buffering,
    Idle,
    Ended,
}

/// What a cast that hasn't started playing is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastStep {
    /// This device reads the stream and makes it ready for the TV: the probe, the remux.
    Preparing,
    /// The TV opens its player or loads the stream.
    Waiting,
}

/// An audio track of the file casting, as the card lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastAudio {
    /// Its place among the file's audio tracks: what [`CastPlayer::set_audio`] takes.
    pub index: usize,
    pub language: Option<String>,
    /// Its own name, when it says more than its language: "Commentary", "English 5.1".
    pub title: Option<String>,
    pub codec: Option<String>,
    pub channels: u16,
    pub selected: bool,
}

/// A text track sent with the load, as the card lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastText {
    /// The track's id on the TV, from one: what [`CastPlayer::set_subtitle`] takes.
    pub id: u32,
    pub language: Option<String>,
    pub title: Option<String>,
    pub selected: bool,
}

/// What the card shows of a cast, as it changes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CastStatus {
    pub state: CastState,
    pub position: f64,
    pub duration: Option<f64>,
    /// The receiver's volume, 0 to 1.
    pub level: Option<f64>,
    pub muted: Option<bool>,
    pub volume: VolumeControl,
    /// The plan playing, once one does.
    pub plan: Option<CastPlan>,
    pub failure: Option<CastFailure>,
    /// What a load not yet playing is doing.
    pub step: Option<CastStep>,
    /// When the load not yet playing was asked for.
    pub since: Option<Instant>,
    /// The file's audio tracks the TV can be given, when there is a choice: a file the relay
    /// remuxes, with more than one.
    pub audio: Vec<CastAudio>,
    /// The text tracks sent with the load.
    pub subtitles: Vec<CastText>,
    /// What the receiver shows of the media: the show, the episode line and the art.
    pub metadata: Option<Metadata>,
    /// The receiver app's ID while it runs for this cast: Kaeri's own or the Default Media
    /// Receiver.
    pub receiver: Option<String>,
    /// Since when the TV has waited for the stream, while it waits: a buffering spell in a file
    /// that plays, never a start or a reload.
    pub waiting_since: Option<Instant>,
    /// Kaeri's receiver didn't launch, never said it was ready, or closed at a load on this cast,
    /// so the Default Media Receiver plays it. Set once it falls back, for the rest of the cast.
    pub fallen_back: bool,
    /// The connection to the TV closed and is being made again, for up to
    /// [`Timings::reconnect`]: `state` is where the TV was. Cleared when it joins again, or when
    /// the tries run out and the cast ends.
    pub reconnecting: bool,
}

/// How long a control's effect is shown before the TV has said it. A receiver answers within a
/// second; a status that still disagrees after this is the TV's own word.
const CONFIRM: Duration = Duration::from_secs(3);

/// What the viewer asked of the TV that its status hasn't said yet. The status shows it at once,
/// so a control answers on the tap; the receiver status that agrees confirms it, and one that
/// still disagrees after [`CONFIRM`] wins.
#[derive(Debug, Clone, Copy, Default)]
struct Wanted {
    paused: Option<(bool, Instant)>,
    position: Option<(f64, Instant)>,
    level: Option<(f64, Instant)>,
    muted: Option<(bool, Instant)>,
}

impl Wanted {
    /// Check what is wanted against a media status.
    fn media(&mut self, status: &MediaStatus) {
        let stale = |at: &Instant| at.elapsed() >= CONFIRM;
        if status.idle_reason.is_some() || status.state == "IDLE" {
            self.paused = None;
            self.position = None;
        }
        if let Some((paused, at)) = self.paused
            && ((status.state == "PAUSED") == paused || stale(&at))
        {
            self.paused = None;
        }
        if let Some((target, at)) = self.position {
            // Played on since, at up to double speed, and a receiver lands near a keyframe.
            let near = 2.0 + at.elapsed().as_secs_f64() * 2.0;
            if (status.position - target).abs() <= near || stale(&at) {
                self.position = None;
            }
        }
    }

    /// Check the volume wanted against what the receiver said of it.
    fn volume(&mut self, level: Option<f64>, muted: Option<bool>) {
        let stale = |at: &Instant| at.elapsed() >= CONFIRM;
        if let Some((want, at)) = self.level
            && (level.is_some_and(|l| (l - want).abs() < 0.005) || stale(&at))
        {
            self.level = None;
        }
        if let Some((want, at)) = self.muted
            && (muted == Some(want) || stale(&at))
        {
            self.muted = None;
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What the relay makes on request for one cast: the remuxed stream, the text tracks, and an
/// HLS master with the chosen audio alone.
#[derive(Default)]
struct Resources {
    remuxer: Mutex<Option<Arc<Remuxer>>>,
    texts: Mutex<HashMap<String, Arc<Vec<u8>>>>,
    /// Subtitle files as they are, for Kaeri's receiver, with their media types.
    originals: Mutex<HashMap<String, Served>>,
    master: Mutex<Option<Arc<Vec<u8>>>>,
}

/// A file the relay serves as it is: its content type and bytes.
type Served = (&'static str, Arc<Vec<u8>>);

/// Where the relay serves [`Resources::master`].
const MASTER: &str = "h/master.m3u8";

impl Dynamic for Resources {
    fn get<'a>(&'a self, path: &'a str) -> Made<'a> {
        Box::pin(async move {
            if let Some(text) = lock(&self.texts).get(path).cloned() {
                return Some(("text/vtt".into(), text.to_vec()));
            }
            if let Some((kind, bytes)) = lock(&self.originals).get(path).cloned() {
                return Some((kind.into(), bytes.to_vec()));
            }
            if path == MASTER {
                let master = lock(&self.master).clone()?;
                return Some(("application/vnd.apple.mpegurl".into(), master.to_vec()));
            }
            let remuxer = lock(&self.remuxer).clone()?;
            match path {
                "r/index.m3u8" => Some((
                    "application/vnd.apple.mpegurl".into(),
                    remuxer.playlist("seg", "init.mp4").into_bytes(),
                )),
                "r/init.mp4" => remuxer.init().ok().map(|b| ("video/mp4".into(), b)),
                _ => {
                    let index =
                        path.strip_prefix("r/seg")?.strip_suffix(".m4s")?.parse().ok()?;
                    let made =
                        tokio::task::spawn_blocking(move || remuxer.segment(index)).await;
                    match made {
                        Ok(Ok(segment)) => Some(("video/mp4".into(), segment)),
                        Ok(Err(e)) => {
                            tracing::warn!(error = %e, index, "a segment couldn't be made");
                            None
                        }
                        Err(_) => None,
                    }
                }
            }
        })
    }
}

#[derive(Default)]
struct State {
    app: Option<App>,
    /// A plan is being tried: the session hears nothing yet.
    trying: bool,
    /// A reload of our own is out (a late subtitle): its new media session is taken quietly.
    reloading: bool,
    /// The media session the session heard `Loaded` for.
    loaded: Option<i64>,
    /// The file loaded has played on the TV: its load was confirmed, or the receiver said
    /// `PLAYING` or `PAUSED` for it. Before that, a position is where the receiver began, often
    /// 0 s before it seeks, and never where the episode got to.
    played: bool,
    last: Option<MediaStatus>,
    paused: Option<bool>,
    paused_since: Option<Instant>,
    buffering: bool,
    rate: Option<f64>,
    /// The speed the viewer asked for, kept for the next load: a `LOAD` starts at normal speed.
    asked_rate: Option<f64>,
    level: Option<f64>,
    muted: Option<bool>,
    control: VolumeControl,
    ended: bool,
    /// The connection to the TV dropped and is being made again: the session waits on it.
    reconnecting: bool,
    closed: bool,
    quit: bool,
    failure: Option<CastFailure>,
    /// The receiver sent `ERROR` while a plan was tried: the Nest Hub's word for audio it can't
    /// decode, after it accepted the load.
    errored: bool,
    /// What Kaeri's receiver said failed while a plan was tried.
    reported: Option<ReceiverError>,
    /// The load in force, with its tracks, for a reload when a subtitle comes later.
    load: Option<Load>,
    /// A reload of the same file asked for while paused: it loads paused, so the TV doesn't
    /// start playing by itself.
    hold_paused: bool,
    plan: Option<CastPlan>,
    /// What the session asked to cast, with the subtitles that came later: what a change of
    /// audio loads again.
    asked: Option<CastLoad>,
    step: Option<CastStep>,
    since: Option<Instant>,
    audio: Vec<CastAudio>,
    subtitles: Vec<CastText>,
    /// The text track on, from one.
    subtitle: Option<u32>,
    /// What the viewer asked that the TV hasn't confirmed yet.
    want: Wanted,
    /// The receiver app to launch: Kaeri's own when one is named.
    receiver: Option<String>,
    /// Kaeri's receiver didn't launch or closed before it played: the Default Media Receiver
    /// plays this cast.
    fallen_back: bool,
    /// Kaeri's receiver closed while a plan was tried: the plan is tried again on the Default
    /// Media Receiver.
    relaunch: bool,
    /// Since when the TV has said it waits for data.
    waiting_since: Option<Instant>,
    /// The receiver app left the TV and why isn't known yet.
    gone: Option<Gone>,
    /// The language and device word Kaeri's receiver is greeted with.
    language: Option<String>,
    device_word: Option<String>,
}

/// The receiver app left the TV: its connection closed, or the receiver's status stopped
/// listing it. The receiver's statuses over the next [`Timings::gone`] say why.
#[derive(Debug, Clone, Copy)]
struct Gone {
    since: Instant,
    /// Whether the latest receiver status since lists the app still: `None` while none came.
    listed: Option<bool>,
}

impl State {
    /// Whether the TV is paused, or asked to pause and not yet heard from.
    fn paused_now(&self) -> bool {
        self.want.paused.map(|(p, _)| p).or(self.paused).unwrap_or(false)
    }

    fn status(&self) -> CastStatus {
        let state = if self.closed || self.quit {
            CastState::Ended
        } else if self.trying {
            CastState::Loading
        } else if self.ended {
            CastState::Idle
        } else {
            match self.last.as_ref().map(|s| s.state.as_str()) {
                Some("PLAYING") => CastState::Playing,
                Some("PAUSED") => CastState::Paused,
                Some("BUFFERING" | "LOADING") => CastState::Buffering,
                Some(_) => CastState::Idle,
                None if self.app.is_some() => CastState::Loading,
                None => CastState::Connecting,
            }
        };
        // The receiver app left and the TV hasn't said why: nothing plays meanwhile, and the
        // card waits on the TV with its clock still.
        let state = match state {
            CastState::Playing | CastState::Paused | CastState::Buffering
                if self.gone.is_some() =>
            {
                CastState::Buffering
            }
            state => state,
        };
        // What the viewer asked shows until the TV says otherwise.
        let state = match (state, self.want.paused) {
            (
                CastState::Playing | CastState::Paused | CastState::Buffering,
                Some((true, _)),
            ) => CastState::Paused,
            (CastState::Paused, Some((false, _))) => CastState::Playing,
            (state, _) => state,
        };
        let position = self.want.position.map(|(p, _)| p);
        CastStatus {
            state,
            position: position.or(self.last.as_ref().map(|s| s.position)).unwrap_or(0.0),
            duration: self.last.as_ref().and_then(|s| s.duration),
            level: self.want.level.map(|(l, _)| l).or(self.level),
            muted: self.want.muted.map(|(m, _)| m).or(self.muted),
            volume: self.control,
            plan: self.plan.clone(),
            failure: self.failure.clone(),
            step: self.step.filter(|_| self.trying),
            since: self.since.filter(|_| self.trying),
            audio: self.audio.clone(),
            subtitles: self
                .subtitles
                .iter()
                .map(|t| CastText { selected: Some(t.id) == self.subtitle, ..t.clone() })
                .collect(),
            metadata: self.last.as_ref().and_then(|s| s.metadata.clone()),
            receiver: self.app.as_ref().map(|a| a.app_id.clone()),
            waiting_since: self
                .gone
                .map(|gone| gone.since)
                .or(self.waiting_since)
                .filter(|_| !self.trying && !self.ended),
            fallen_back: self.fallen_back,
            reconnecting: self.reconnecting && !self.closed && !self.quit,
        }
    }
}

struct Inner {
    setup: CastSetup,
    /// The connection to the TV: replaced when it dropped and was made again.
    device: Mutex<Device>,
    relay: Relay,
    resources: Arc<Resources>,
    /// The media session in force. Read without waiting, so a control never queues behind a
    /// launch.
    media: Mutex<Option<Media>>,
    /// Held while the receiver app launches, so two loads launch it once.
    launching: AsyncMutex<()>,
    state: Mutex<State>,
    events: Mutex<Option<mpsc::UnboundedSender<PlaybackEvent>>>,
    status: watch::Sender<CastStatus>,
    /// What Kaeri's receiver said in its `ready`, once it has, for the app running now.
    ready: watch::Sender<Option<Value>>,
    /// What Kaeri's receiver says on its own namespace, for the engine.
    heard: broadcast::Sender<FromReceiver>,
    /// The load being tried, aborted by a stop or the next load.
    attempt: Mutex<Option<JoinHandle<()>>>,
    /// Counts the device's changes of network: a try at connecting again starts over on one.
    network: watch::Sender<u64>,
}

impl Inner {
    fn say(&self, event: PlaybackEvent) {
        if let Some(events) = lock(&self.events).as_ref() {
            let _ = events.send(event);
        }
    }

    fn publish(&self) {
        let status = lock(&self.state).status();
        self.publish_status(status);
    }

    /// Publish the status of a state the caller already holds locked.
    fn publish_locked(&self, state: &State) {
        self.publish_status(state.status());
    }

    fn publish_status(&self, status: CastStatus) {
        self.status.send_if_modified(|current| {
            let changed = *current != status;
            *current = status;
            changed
        });
    }

    /// Note what the viewer asked of the TV, shown until the TV confirms it.
    fn want(&self, change: impl FnOnce(&mut Wanted)) {
        change(&mut lock(&self.state).want);
        self.publish();
    }

    /// Say an event from inside the state lock. The status goes out first, so whoever reacts to
    /// the event reads a status that already includes what caused it.
    fn say_locked(&self, state: &State, event: PlaybackEvent) {
        self.publish_locked(state);
        self.say(event);
    }

    /// The running receiver app, launched when there is none yet.
    ///
    /// The launch runs in a task of its own, which a Stop doesn't abort: a receiver that comes
    /// up after the viewer stopped is closed again, so it isn't left on the TV with no sender.
    async fn app(self: &Arc<Self>) -> Result<Media, CastError> {
        let _launching = self.launching.lock().await;
        if let Some(media) = self.media() {
            return Ok(media);
        }
        let inner = Arc::clone(self);
        let launch = tokio::spawn(async move {
            // Kaeri's receiver first on every cast: one that failed on the last says nothing
            // of this one.
            let receiver = {
                let state = lock(&inner.state);
                state.receiver.clone().filter(|_| !state.fallen_back)
            };
            let app = match receiver {
                Some(receiver) if receiver != DEFAULT_MEDIA_RECEIVER => {
                    inner.ready.send_replace(None);
                    match inner.device().launch(&receiver).await {
                        // The receiver answers `hello` with `ready`, and also says it to each
                        // sender that connects, so neither order of the two loses it.
                        Ok(app) => match inner.hello(&app).wait_ready(&app).await {
                            Some(ready) => {
                                tracing::info!(%ready, "Kaeri's receiver is ready");
                                app
                            }
                            None => {
                                tracing::info!(
                                    "Kaeri's receiver never said it was ready; the Default Media Receiver plays"
                                );
                                lock(&inner.state).fallen_back = true;
                                inner.device().launch(DEFAULT_MEDIA_RECEIVER).await?
                            }
                        },
                        Err(e) => {
                            tracing::info!(error = %e, "Kaeri's receiver didn't launch; the Default Media Receiver plays");
                            lock(&inner.state).fallen_back = true;
                            inner.device().launch(DEFAULT_MEDIA_RECEIVER).await?
                        }
                    }
                }
                _ => inner.device().launch(DEFAULT_MEDIA_RECEIVER).await?,
            };
            {
                // Under the state's lock, so a quit either sees this media or is seen here.
                let mut state = lock(&inner.state);
                if !state.quit {
                    state.app = Some(app.clone());
                    state.gone = None;
                    let fresh = Media::new(inner.device().channel.clone(), app);
                    *lock(&inner.media) = Some(fresh.clone());
                    return Ok(fresh);
                }
            }
            tracing::info!("the receiver came up after the stop; closing it");
            let _ = tokio::time::timeout(CONTROL_WAIT, inner.device().stop(&app)).await;
            Err(CastError::Closed)
        });
        launch.await.unwrap_or(Err(CastError::Closed))
    }

    /// Kaeri's receiver runs now.
    fn runs_own(&self) -> bool {
        let state = lock(&self.state);
        !state.fallen_back
            && state.app.as_ref().is_some_and(|app| app.app_id != DEFAULT_MEDIA_RECEIVER)
    }

    /// Whether the next load goes to Kaeri's receiver. When it is the receiver to launch and
    /// nothing runs yet, it is launched now, so the plans are made for the receiver that plays.
    async fn own_receiver(self: &Arc<Self>) -> bool {
        if self.runs_own() {
            return true;
        }
        let expected = {
            let state = lock(&self.state);
            state.app.is_none()
                && !state.fallen_back
                && state.receiver.as_deref().is_some_and(|r| r != DEFAULT_MEDIA_RECEIVER)
        };
        if !expected {
            return false;
        }
        match self.app().await {
            Ok(_) => self.runs_own(),
            Err(e) => {
                tracing::info!(error = %e, "no receiver yet; planning for the Default Media Receiver");
                false
            }
        }
    }

    /// The media session in force, for a command.
    fn media(&self) -> Option<Media> {
        lock(&self.media).clone()
    }

    /// The connection to the TV in force.
    fn device(&self) -> Device {
        lock(&self.device).clone()
    }

    /// Wait for Kaeri's receiver `app` to say `ready`, up to [`Timings::ready`]. `None`: the
    /// page never loaded, or isn't Kaeri's.
    async fn wait_ready(&self, app: &App) -> Option<Value> {
        let mut ready = self.ready.subscribe();
        let waited = tokio::time::timeout(
            self.setup.timings.ready,
            ready.wait_for(|ready| ready.is_some()),
        )
        .await;
        let said = waited.ok()?.ok()?.clone();
        tracing::debug!(app = %app.app_id, "ready heard");
        said
    }

    /// Greet Kaeri's receiver: the language for its strings and the word for this device.
    fn hello(&self, app: &App) -> &Self {
        let (language, device) = {
            let state = lock(&self.state);
            (state.language.clone(), state.device_word.clone())
        };
        let hello = json!({"type": "hello", "version": KAERI_PROTOCOL,
                           "language": language, "device": device});
        if let Err(e) = self.device().channel.send(&app.transport_id, NS_KAERI, &hello) {
            tracing::debug!(error = %e, "couldn't greet Kaeri's receiver");
        }
        self
    }

    /// Say `message` to Kaeri's receiver, when it runs. Nothing on the Default Media Receiver.
    fn tell(&self, mut message: Value) {
        let app = lock(&self.state).app.clone();
        let Some(app) = app.filter(|a| a.app_id != DEFAULT_MEDIA_RECEIVER) else { return };
        message["version"] = json!(KAERI_PROTOCOL);
        if let Err(e) = self.device().channel.send(&app.transport_id, NS_KAERI, &message) {
            tracing::debug!(error = %e, "couldn't tell Kaeri's receiver");
        }
    }
}

/// What Kaeri's receiver says that the engine acts on.
#[derive(Debug, Clone, PartialEq)]
pub enum FromReceiver {
    /// Typeset subtitles fell behind; the receiver shows them as plain text from now.
    SubtitlesPlain,
    /// Up Next's Play now (`true`) or Cancel, pressed on the TV's remote.
    UpNext { play: bool },
    /// The receiver waited this many seconds with the relay unreachable.
    SenderGone { waited: f64 },
    /// Something it fetched or played failed.
    Error(ReceiverError),
}

/// Up Next as Kaeri's receiver draws it over the episode.
#[derive(Debug, Clone, PartialEq)]
pub struct UpNextCard {
    pub number: Option<String>,
    pub title: Option<String>,
    /// The next episode's still, a URL the TV can fetch.
    pub still: Option<String>,
    /// Seconds until it plays by itself.
    pub seconds: u32,
}

/// Kaeri's receiver's report of a failure.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReceiverError {
    /// `video`, `audio`, `subtitles`, `network`, `manifest`, or `unknown` for a media error
    /// neither the player nor the TV placed.
    pub part: String,
    /// The player's own words for it.
    pub detail: Option<String>,
    /// The HTTP status of the fetch, 0 for no answer at all.
    pub status: Option<u16>,
    /// The Cast framework's detailed error code.
    pub detailed: Option<i64>,
    /// The Shaka player's error code.
    pub shaka: Option<i64>,
    pub codec: Option<String>,
    pub position: Option<f64>,
    /// What was being fetched: `playlist`, `init`, `segment`, `subtitle` or `font`.
    pub fetched: Option<String>,
    /// Nothing answered the fetch.
    pub lost: bool,
}

impl ReceiverError {
    fn read(payload: &Value) -> Self {
        // The names of the first protocol drafts, read as the parts they were.
        let part = match payload["part"].as_str().unwrap_or("video") {
            "subtitle" | "font" => "subtitles",
            "segment" => "network",
            "playlist" | "init" => "manifest",
            "media" => "video",
            part => part,
        };
        Self {
            part: part.to_owned(),
            detail: payload["detail"].as_str().map(str::to_owned),
            status: payload["status"].as_u64().and_then(|s| u16::try_from(s).ok()),
            detailed: payload["detailed"].as_i64(),
            shaka: payload["shaka"].as_i64(),
            codec: payload["codec"].as_str().map(str::to_owned),
            position: payload["position"].as_f64(),
            fetched: payload["fetched"].as_str().map(str::to_owned),
            lost: payload["lost"].as_bool().unwrap_or(false),
        }
    }
}

/// A TV playing what the session tells it.
pub struct CastPlayer {
    inner: Arc<Inner>,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for CastPlayer {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
        if let Some(attempt) = lock(&self.inner.attempt).take() {
            attempt.abort();
        }
    }
}

impl CastPlayer {
    /// Connect to the TV, read its volume and firmware, and open the relay it will fetch from,
    /// on the interface facing it and for it alone. Nothing starts on the TV yet.
    pub async fn connect(
        setup: CastSetup,
    ) -> Result<(Self, mpsc::UnboundedReceiver<PlaybackEvent>), CastFailure> {
        let address = setup.device.address;
        let IpAddr::V4(tv) = address.ip() else {
            return Err(CastFailure::NotSameNetwork);
        };
        // Loopback is the fake receiver in tests, and nothing else ever has that address.
        let bind = if tv.is_loopback() {
            tv
        } else {
            let local = local_address_towards(tv).map_err(|_| CastFailure::NotSameNetwork)?;
            if !same_network(local, tv) {
                return Err(CastFailure::NotSameNetwork);
            }
            local
        };
        let lost = |e: CastError| CastFailure::Lost(e.to_string());
        let relay = Relay::start(bind, address.ip(), Arc::clone(&setup.upstream))
            .await
            .map_err(|e| CastFailure::Lost(e.to_string()))?;
        relay.set_slow_source(setup.timings.slow_source);
        let resources = Arc::new(Resources::default());
        relay.set_dynamic(Some(Arc::clone(&resources) as Arc<dyn Dynamic>));
        let (device, incoming) =
            Device::connect(address, setup.timings.silence).await.map_err(lost)?;
        let receiver = device.status().await.map_err(lost)?;
        let firmware =
            if tv.is_loopback() { None } else { firmware(setup.upstream.as_ref(), tv).await };
        // A session tries every stream as it is: nothing refused in the last one counts.
        setup.memory.new_session(&setup.device.id);
        if let Err(e) = setup.memory.seen(
            &setup.device.id,
            &setup.device.name,
            &setup.device.model,
            firmware.as_deref(),
            receiver.volume,
        ) {
            tracing::warn!(error = %e, "couldn't note the TV");
        }

        let state = State {
            control: receiver.volume,
            level: receiver.level,
            muted: receiver.muted,
            receiver: KAERI_RECEIVER.map(str::to_owned),
            ..State::default()
        };
        let (events, rx) = mpsc::unbounded_channel();
        let (status, _) = watch::channel(state.status());
        let inner = Arc::new(Inner {
            setup,
            device: Mutex::new(device),
            relay,
            resources,
            media: Mutex::new(None),
            launching: AsyncMutex::new(()),
            state: Mutex::new(state),
            events: Mutex::new(Some(events)),
            status,
            ready: watch::channel(None).0,
            heard: broadcast::channel(16).0,
            attempt: Mutex::new(None),
            network: watch::channel(0).0,
        });
        let pump = tokio::spawn(pump(incoming, Arc::downgrade(&inner)));
        let poll = tokio::spawn(poll(Arc::downgrade(&inner)));
        Ok((Self { inner, tasks: vec![pump, poll] }, rx))
    }

    /// Launch `receiver` in place of Kaeri's own, from the next launch: an app ID from the
    /// config. `None` keeps Kaeri's own, or the Default Media Receiver while there is none.
    pub fn use_receiver(&self, receiver: Option<String>) {
        let receiver = receiver.filter(|r| !r.trim().is_empty());
        lock(&self.inner.state).receiver = receiver.or(KAERI_RECEIVER.map(str::to_owned));
    }

    /// The language Kaeri's receiver shows its strings in and the word it calls this device
    /// by (`iphone`, `phone`, `mac`, ...). A receiver already running hears it at once.
    pub fn set_greeting(&self, language: Option<String>, device: Option<String>) {
        {
            let mut state = lock(&self.inner.state);
            state.language = language.clone();
            state.device_word = device.clone();
        }
        self.inner.tell(json!({"type": "hello", "language": language, "device": device}));
    }

    /// What Kaeri's receiver says from now on: plain subtitles, Up Next's buttons, errors.
    pub fn heard(&self) -> broadcast::Receiver<FromReceiver> {
        self.inner.heard.subscribe()
    }

    /// This device is about to stop answering (an iPhone leaving the foreground): Kaeri's
    /// receiver shows that it waits for it.
    pub fn tell_away(&self) {
        let device = lock(&self.inner.state).device_word.clone();
        self.inner.tell(json!({"type": "sender-away", "device": device}));
    }

    /// The engine skipped the opening or the ending (`part`): Kaeri's receiver says so on the
    /// TV for a moment.
    pub fn tell_skipped(&self, part: &str) {
        self.inner.tell(json!({"type": "skipped", "part": part}));
    }

    /// Up Next counting on this device: its episode for the receiver's own card, `None` to
    /// hide it.
    pub fn tell_up_next(&self, next: Option<UpNextCard>) {
        let message = match next {
            Some(next) => json!({"type": "up-next", "number": next.number, "title": next.title,
                                 "still": next.still, "seconds": next.seconds}),
            None => json!({"type": "up-next"}),
        };
        self.inner.tell(message);
    }

    /// Start the receiver app now, before anything plays: what connecting to a TV does in a
    /// cast session. The TV shows the receiver's idle screen until a file loads.
    pub async fn launch(&self) -> Result<(), CastFailure> {
        let launched = self.inner.app().await;
        self.inner.publish();
        launched.map(drop).map_err(|e| CastFailure::Lost(e.to_string()))
    }

    /// Join the receiver app `session` names, when it still runs on the TV: what a cast session
    /// left running when this device last held it. `None` when it has gone.
    pub async fn join(
        setup: CastSetup,
        session: &str,
    ) -> Result<Option<(Self, mpsc::UnboundedReceiver<PlaybackEvent>)>, CastFailure> {
        let (player, events) = Self::connect(setup).await?;
        let lost = |e: CastError| CastFailure::Lost(e.to_string());
        let receiver = player.inner.device().status().await.map_err(lost)?;
        let Some(app) = receiver.app.filter(|app| app.session_id == session) else {
            player.inner.relay.close();
            return Ok(None);
        };
        player.inner.device().channel.connect_to(&app.transport_id).map_err(lost)?;
        let mut media = Media::new(player.inner.device().channel.clone(), app.clone());
        {
            let mut state = lock(&player.inner.state);
            state.app = Some(app);
            state.gone = None;
        }
        // Its media session, when something plays: the answer is the first status. It is taken
        // here, as the receive task may read the same message only after this returns.
        match media.status().await {
            Ok(Some(status)) => {
                let mut state = lock(&player.inner.state);
                state.last = Some(status.clone());
                apply(&player.inner, &mut state, &status);
            }
            Ok(None) => {}
            Err(e) => tracing::debug!(error = %e, "the joined receiver didn't say what plays"),
        }
        *lock(&player.inner.media) = Some(media);
        player.inner.publish();
        Ok(Some((player, events)))
    }

    /// The receiver app running for this cast.
    pub fn receiver(&self) -> Option<App> {
        lock(&self.inner.state).app.clone()
    }

    /// A fresh stream of the session's events for the next playback, ending the one before: a
    /// cast session plays one playback after another on the same receiver.
    pub fn events(&self) -> mpsc::UnboundedReceiver<PlaybackEvent> {
        let (events, rx) = mpsc::unbounded_channel();
        *lock(&self.inner.events) = Some(events);
        rx
    }

    /// End the event stream: the playback reading it is over, and the receiver stays.
    pub fn detach(&self) {
        lock(&self.inner.events).take();
    }

    /// The device moved to another network, or back to one: a try at connecting again that
    /// waits on the network it left gives way to a new try at once, so a phone back on the
    /// TV's Wi-Fi joins again as it comes back.
    pub fn network_changed(&self) {
        self.inner.network.send_modify(|changes| *changes += 1);
    }

    /// The card's view of the cast, as it changes.
    pub fn status(&self) -> watch::Receiver<CastStatus> {
        self.inner.status.subscribe()
    }

    pub fn device(&self) -> &CastDevice {
        &self.inner.setup.device
    }

    /// The card's view of the cast as it stands.
    pub fn current(&self) -> CastStatus {
        self.inner.status.borrow().clone()
    }

    /// Where the TV is in the file, or where it was asked to seek to and hasn't said yet.
    pub fn position(&self) -> Option<f64> {
        let state = lock(&self.inner.state);
        state.want.position.map(|(p, _)| p).or(state.last.as_ref().map(|s| s.position))
    }

    /// Say `event` to the session as if the TV had: what the sender's card asks of the session
    /// that only a player would, such as playing the next episode when Up Next's count ends.
    pub fn say(&self, event: PlaybackEvent) {
        self.inner.say(event);
    }

    /// Cast `load` in place of whatever plays. Returns at once: what happened comes back as
    /// events, `Loaded` once a plan plays, or `Ended` unplayed with [`CastPlayer::failure`] set.
    pub fn load(&self, load: CastLoad) {
        self.start(load, false);
    }

    /// Load the file casting again with the audio track at `index` among its tracks, from where
    /// it is: a TV takes its tracks with the load. The session hears no new `Loaded`, as for a
    /// late subtitle. Nothing when no file is casting or the track is the one playing.
    pub fn set_audio(&self, index: usize) {
        let load = {
            let state = lock(&self.inner.state);
            if state.trying || !state.audio.iter().any(|a| a.index == index && !a.selected) {
                return;
            }
            let Some(mut load) = state.asked.clone() else { return };
            load.audio_track = Some(index);
            load.start = state.last.as_ref().map(|s| s.position).or(load.start);
            load.subtitle =
                state.subtitle.and_then(|id| usize::try_from(id).ok()?.checked_sub(1));
            load
        };
        tracing::info!(index, "loading the cast again for another audio track");
        self.start(load, true);
    }

    fn start(&self, load: CastLoad, quiet: bool) {
        begin(&self.inner, load, quiet);
    }
}

/// Try `load` on the TV, ending any load still tried: `quiet` loads the same file again where it
/// was, for another track or another receiver, with no new start for the session.
fn begin(owner: &Arc<Inner>, load: CastLoad, quiet: bool) {
    {
        let mut state = lock(&owner.state);
        state.trying = true;
        state.reloading = false;
        state.hold_paused = quiet && state.paused == Some(true);
        state.ended = false;
        state.failure = None;
        state.loaded = None;
        // The same file again keeps its word; a new one has not played yet.
        state.played &= quiet;
        state.step = Some(CastStep::Preparing);
        state.since = Some(Instant::now());
        state.asked = Some(load.clone());
    }
    owner.publish();
    let inner = Arc::clone(owner);
    let task = tokio::spawn(async move {
        let outcome = attempt(&inner, load).await;
        let speed = {
            let mut state = lock(&inner.state);
            state.trying = false;
            state.step = None;
            state.since = None;
            match outcome {
                Ok(plan) => {
                    tracing::info!(route = ?plan.route, delivery = ?plan.delivery, "cast playing");
                    state.plan = Some(plan);
                    // The same file where it was, for the session: no new `Loaded`.
                    state.reloading = quiet;
                    state.played = true;
                    // The status that confirmed it is the session's first.
                    if let Some(last) = state.last.clone() {
                        apply(&inner, &mut state, &last);
                    }
                }
                Err(failure) => {
                    tracing::warn!(%failure, "nothing cast");
                    state.failure = Some(failure);
                    state.ended = true;
                    inner.say_locked(&state, PlaybackEvent::Ended { complete: false });
                }
            }
            // A load starts at normal speed: the one asked for is set once it plays.
            let playing = state.plan.is_some() && !state.ended;
            state.asked_rate.filter(|r| playing && (r - 1.0).abs() > f64::EPSILON)
        };
        inner.publish();
        if let (Some(rate), Some(media)) = (speed, inner.media()) {
            let _ = answered(async move { media.set_rate(rate).await.map(|_| ()) }).await;
        }
    });
    if let Some(previous) = lock(&owner.attempt).replace(task) {
        previous.abort();
    }
}

impl CastPlayer {
    /// Side-load a subtitle file that came after the load, by a reload where it is: tracks are
    /// fixed at `LOAD`.
    pub async fn add_subtitle(
        &self,
        path: &Path,
        title: Option<String>,
        language: Option<String>,
    ) -> Result<(), CastError> {
        let track = text_track(
            &self.inner,
            &CastSubtitle { path: path.to_owned(), title, language },
            None,
        )?;
        let reload = {
            let mut state = lock(&self.inner.state);
            let position = state.last.as_ref().map(|s| s.position);
            let paused = state.paused == Some(true);
            if let Some(asked) = state.asked.as_mut() {
                asked.subtitles.push(CastSubtitle {
                    path: path.to_owned(),
                    title: track_title(&track),
                    language: track_language(&track),
                });
            }
            let reload = state.load.as_mut().map(|load| {
                let mut track = track.clone();
                track.id = u32::try_from(load.tracks.len() + 1).unwrap_or(u32::MAX);
                load.tracks.push(track);
                load.start = position.or(load.start);
                // Paused stays paused: the reload doesn't start the TV by itself.
                load.autoplay = !paused;
                load.clone()
            });
            if let Some(load) = &reload
                && let Some(added) = load.tracks.last()
            {
                state.subtitles.push(CastText {
                    id: added.id,
                    language: track_language(added),
                    title: track_title(added),
                    selected: false,
                });
            }
            if reload.is_some() && !state.trying {
                state.reloading = true;
            }
            reload
        };
        match reload {
            Some(load) if !lock(&self.inner.state).trying => {
                let mut media = self.inner.app().await?;
                let inner = Arc::downgrade(&self.inner);
                answered(async move {
                    media.load(&load).await?;
                    // A load starts at normal speed: the one asked for is set again.
                    let rate = inner.upgrade().and_then(|inner| {
                        *lock(&inner.media) = Some(media.clone());
                        lock(&inner.state).asked_rate.filter(|r| (r - 1.0).abs() > f64::EPSILON)
                    });
                    if let Some(rate) = rate {
                        media.set_rate(rate).await?;
                    }
                    Ok(())
                })
                .await
            }
            _ => Ok(()),
        }
    }

    /// Pause or play. The status says so at once, and the TV's next status confirms it.
    pub async fn set_paused(&self, paused: bool) -> Result<(), CastError> {
        let Some(media) = self.inner.media().filter(|m| m.media_session_id.is_some()) else {
            return Ok(());
        };
        self.inner.want(|want| want.paused = Some((paused, Instant::now())));
        let result = answered(async move {
            if paused { media.pause().await } else { media.play().await }.map(|_| ())
        })
        .await;
        if result.is_err() {
            self.inner.want(|want| want.paused = None);
        }
        result
    }

    pub async fn toggle_pause(&self) -> Result<(), CastError> {
        let paused = lock(&self.inner.state).paused_now();
        self.set_paused(!paused).await
    }

    /// Seek to `seconds`, or by them when not `absolute`.
    pub async fn seek(&self, seconds: f64, absolute: bool) -> Result<(), CastError> {
        let Some(media) = self.inner.media() else { return Ok(()) };
        let target = if absolute { seconds } else { self.position().unwrap_or(0.0) + seconds };
        let target = target.max(0.0);
        if media.media_session_id.is_some() {
            self.inner.want(|want| want.position = Some((target, Instant::now())));
        }
        self.inner.say(PlaybackEvent::Seeking(true));
        let inner = Arc::downgrade(&self.inner);
        let result = answered(async move {
            let result = media.seek(target).await;
            if let Some(inner) = inner.upgrade() {
                inner.say(PlaybackEvent::Seeking(false));
            }
            result.map(|_| ())
        })
        .await;
        if result.is_err() {
            self.inner.want(|want| want.position = None);
        }
        result
    }

    pub async fn set_rate(&self, rate: f64) -> Result<(), CastError> {
        // 0 or less is no speed to play at, and clamped it would play at half speed.
        if !is_rate(rate) {
            tracing::debug!(rate, "no speed to cast at");
            return Ok(());
        }
        let rate = rate.clamp(0.5, 2.0);
        lock(&self.inner.state).asked_rate = Some(rate);
        // Before a load there is nothing to set: the load applies it once it plays.
        let Some(media) = self.inner.media() else { return Ok(()) };
        answered(async move { media.set_rate(rate).await.map(|_| ()) }).await
    }

    pub fn rate(&self) -> f64 {
        lock(&self.inner.state).rate.unwrap_or(1.0)
    }

    /// The receiver's volume, 0 to 100. Nothing on a TV whose volume is its own.
    pub async fn set_volume(&self, level: f64) -> Result<(), CastError> {
        if lock(&self.inner.state).control == VolumeControl::MuteOnly {
            return Ok(());
        }
        let device = self.inner.device();
        let level = (level / 100.0).clamp(0.0, 1.0);
        self.inner.want(|want| want.level = Some((level, Instant::now())));
        let result = answered(async move { device.set_volume(Some(level), None).await }).await;
        if result.is_err() {
            self.inner.want(|want| want.level = None);
        }
        result
    }

    pub fn volume(&self) -> f64 {
        let state = lock(&self.inner.state);
        state.want.level.map(|(l, _)| l).or(state.level).map_or(100.0, |l| (l * 100.0).round())
    }

    pub fn volume_control(&self) -> VolumeControl {
        lock(&self.inner.state).control
    }

    pub async fn set_muted(&self, muted: bool) -> Result<(), CastError> {
        let device = self.inner.device();
        self.inner.want(|want| want.muted = Some((muted, Instant::now())));
        let result = answered(async move { device.set_volume(None, Some(muted)).await }).await;
        if result.is_err() {
            self.inner.want(|want| want.muted = None);
        }
        result
    }

    pub fn muted(&self) -> bool {
        let state = lock(&self.inner.state);
        state.want.muted.map(|(m, _)| m).or(state.muted).unwrap_or(false)
    }

    /// Show the text track `id` (1-based, as loaded), or none.
    pub async fn set_subtitle(&self, id: Option<u32>) -> Result<(), CastError> {
        {
            let mut state = lock(&self.inner.state);
            state.subtitle = id.filter(|id| state.subtitles.iter().any(|t| t.id == *id));
        }
        self.inner.publish();
        let Some(media) = self.inner.media() else { return Ok(()) };
        let ids: Vec<u32> = id.into_iter().collect();
        answered(async move { media.active_tracks(&ids).await.map(|_| ()) }).await
    }

    /// Stop the file, keeping the receiver for the next.
    pub async fn stop(&self) -> Result<(), CastError> {
        if let Some(attempt) = lock(&self.inner.attempt).take() {
            attempt.abort();
            // A load cut short isn't loading any more.
            {
                let mut state = lock(&self.inner.state);
                state.trying = false;
                state.step = None;
                state.since = None;
            }
            self.inner.publish();
        }
        let Some(media) = self.inner.media() else { return Ok(()) };
        if media.media_session_id.is_none() {
            return Ok(());
        }
        answered(async move { media.stop().await.map(|_| ()) }).await
    }

    /// Stop playing and close the receiver app, so the TV goes back to what it showed. The
    /// event stream ends with it.
    pub async fn quit(&self) -> Result<(), CastError> {
        if let Some(attempt) = lock(&self.inner.attempt).take() {
            attempt.abort();
        }
        lock(&self.inner.state).quit = true;
        let result = close(&self.inner).await;
        self.inner.relay.close();
        self.inner.publish();
        lock(&self.inner.events).take();
        result
    }

    /// Wait while the connection to the TV is made again, and say whether the cast goes on: a
    /// control sent meanwhile would find the connection closed.
    pub async fn settled(&self) -> bool {
        let timings = &self.inner.setup.timings;
        let deadline = Instant::now() + timings.reconnect + timings.silence;
        while lock(&self.inner.state).reconnecting && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        self.is_alive()
    }

    pub fn is_alive(&self) -> bool {
        let state = lock(&self.inner.state);
        (state.reconnecting || self.inner.device().channel.is_open())
            && !state.closed
            && !state.quit
    }

    pub fn has_quit(&self) -> bool {
        lock(&self.inner.state).quit
    }

    /// Why nothing played, or why the cast stopped.
    pub fn failure(&self) -> Option<CastFailure> {
        lock(&self.inner.state).failure.clone()
    }
}

/// How long a control waits for the TV's answer before it lets the session go on. mpv answers
/// in milliseconds; a TV that takes longer is answered off the session's loop, and one that has
/// gone is noticed by its channel's silence.
const CONTROL_WAIT: Duration = Duration::from_secs(2);

/// Run a command to the TV, waiting for its answer up to [`CONTROL_WAIT`]. Past that it
/// finishes by itself, and what it says is only logged.
async fn answered(
    command: impl std::future::Future<Output = Result<(), CastError>> + Send + 'static,
) -> Result<(), CastError> {
    let task = tokio::spawn(async move {
        let result = command.await;
        if let Err(e) = &result {
            tracing::debug!(error = %e, "the TV didn't take a command");
        }
        result
    });
    match tokio::time::timeout(CONTROL_WAIT, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Ok(()),
        Err(_) => {
            tracing::debug!("the TV is slow to answer; going on without it");
            Ok(())
        }
    }
}

/// Stop the media and the app, when one runs.
async fn close(inner: &Inner) -> Result<(), CastError> {
    let media = lock(&inner.media).take();
    let Some(media) = media else { return Ok(()) };
    lock(&inner.state).app = None;
    let device = inner.device();
    answered(async move {
        if media.media_session_id.is_some() {
            let _ = media.stop().await;
        }
        device.stop(&media.app).await
    })
    .await
}

/// `cast_build_revision` from the TV's local setup API, when it answers within two seconds.
async fn firmware(upstream: &dyn Upstream, tv: std::net::Ipv4Addr) -> Option<String> {
    let url = format!("http://{tv}:8008/setup/eureka_info?params=build_info");
    let fetch = async {
        let response = upstream.get(&url, &[]).await.ok()?;
        let body = response.bytes(256 * 1024).await.ok()?;
        let info: Value = serde_json::from_slice(&body).ok()?;
        info["build_info"]["cast_build_revision"]
            .as_str()
            .or_else(|| info["cast_build_revision"].as_str())
            .map(str::to_owned)
    };
    tokio::time::timeout(Duration::from_secs(2), fetch).await.ok().flatten()
}

/// What `load` holds, read as cheaply as its kind allows.
async fn media_info(setup: &CastSetup, load: &CastLoad) -> (MediaInfo, probe::CorsProbe) {
    let upstream = setup.upstream.as_ref();
    let mut probe = probe::CorsProbe::default();
    let mut info = match load.source.kind {
        SourceKind::Hls => {
            // The source itself, as the TV would ask it.
            let (info, found) =
                probe::hls(upstream, &load.source.url, &load.source.headers).await;
            probe = found;
            info
        }
        SourceKind::File => {
            let path = PathBuf::from(load.url.strip_prefix("file://").unwrap_or(&load.url));
            tokio::task::spawn_blocking(move || {
                let mut file = RangeFile::local(&path).ok()?;
                let mut head = vec![0u8; 64];
                let n = std::io::Read::read(&mut file, &mut head).ok()?;
                let container = probe::sniff(&head[..n]);
                if container == Some(Delivery::Mkv) {
                    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(0)).ok()?;
                    return probe::matroska(&mut file).ok();
                }
                let mut info = MediaInfo { container, ..MediaInfo::default() };
                if container == Some(Delivery::Mp4) {
                    let mut buffer = Vec::new();
                    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(0)).ok()?;
                    std::io::Read::read_to_end(
                        &mut std::io::Read::take(&mut file, 4 << 20),
                        &mut buffer,
                    )
                    .ok()?;
                    probe::read_sample_entries(&buffer, &mut info);
                }
                Some(info)
            })
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
        }
        SourceKind::Progressive | SourceKind::Torrent => {
            let info = probe::progressive(upstream, &load.url, &load.headers).await;
            if info.container == Some(Delivery::Mkv) {
                probe::matroska_remote(
                    Arc::clone(&setup.upstream),
                    &load.url,
                    load.headers.clone(),
                )
                .await
                .unwrap_or(info)
            } else {
                info
            }
        }
        SourceKind::DeepLink => MediaInfo::default(),
    };
    if let Some(index) = load.audio_track.filter(|i| *i < info.audio.len()) {
        info.audio_choice = index;
    } else if let Some(index) = app_pick(&info, load) {
        info.audio_choice = index;
    }
    (info, probe)
}

/// The audio track of `info` the app's player would pick: the series' remembered track, then
/// the programme in the language asked for. `None` leaves the stream's own choice.
fn app_pick(info: &MediaInfo, load: &CastLoad) -> Option<usize> {
    if info.audio.len() < 2 {
        return None;
    }
    let tracks: Vec<Track> = info
        .audio
        .iter()
        .zip(0i64..)
        .map(|(audio, id)| Track {
            id,
            kind: TrackKind::Audio,
            lang: audio.language.clone(),
            title: audio.name.clone(),
            default: audio.default,
            forced: false,
            hearing_impaired: false,
            visual_impaired: false,
            external: false,
            external_filename: None,
            codec: None,
            selected: false,
            main_selection: None,
        })
        .collect();
    let refs: Vec<&Track> = tracks.iter().collect();
    let picked =
        choose_audio(&refs, load.audio_language.as_deref(), load.audio_memory.as_ref())?;
    usize::try_from(picked.id).ok()
}

/// A rendition's name for the card, when it says more than its language: "English" beside
/// English says nothing.
fn audio_title(track: &AudioTrack) -> Option<String> {
    let name = track.name.clone().filter(|n| !n.trim().is_empty())?;
    let Some(language) = track.language.as_deref() else { return Some(name) };
    let alone = Track {
        id: 0,
        kind: TrackKind::Audio,
        lang: None,
        title: Some(name.clone()),
        default: false,
        forced: false,
        hearing_impaired: false,
        visual_impaired: false,
        external: false,
        external_filename: None,
        codec: None,
        selected: false,
        main_selection: None,
    };
    let only_language = name.split_whitespace().count() == 1
        && alone.language().is_some_and(|l| language_matches(&l, language));
    (!only_language).then_some(name)
}

/// A subtitle file as a WebVTT track the relay serves.
fn text_track(
    inner: &Inner,
    subtitle: &CastSubtitle,
    id: Option<u32>,
) -> Result<TextTrack, CastError> {
    let text = std::fs::read(&subtitle.path)?;
    let text = String::from_utf8_lossy(&text);
    let extension = subtitle.path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    let webvtt = to_webvtt(&text, extension.as_deref());
    let mut texts = lock(&inner.resources.texts);
    let id = id.unwrap_or_else(|| u32::try_from(texts.len() + 1).unwrap_or(u32::MAX));
    let name = format!("t/{}-{id}.vtt", texts.len() + 1);
    texts.insert(name.clone(), Arc::new(webvtt.into_bytes()));
    Ok(TextTrack {
        id,
        url: inner.relay.dynamic_url(&name),
        name: subtitle.title.clone().unwrap_or_else(|| {
            subtitle.language.clone().unwrap_or_else(|| format!("Subtitles {id}"))
        }),
        language: subtitle.language.clone().unwrap_or_else(|| "und".into()),
    })
}

/// The empty WebVTT stand-in for a subtitle track inside the file, which Kaeri's receiver
/// draws itself: the TV's list carries its id, language and name.
fn inside_track(inner: &Inner, original: &Original) -> TextTrack {
    let name = format!("t/inside-{}.vtt", original.id);
    lock(&inner.resources.texts).insert(name.clone(), Arc::new(b"WEBVTT\n\n".to_vec()));
    TextTrack {
        id: original.id,
        url: inner.relay.dynamic_url(&name),
        name: original.title.clone().unwrap_or_else(|| {
            original.language.clone().unwrap_or_else(|| format!("Subtitles {}", original.id))
        }),
        language: original.language.clone().unwrap_or_else(|| "und".into()),
    }
}

/// A subtitle file's text as WebVTT: ASS converted with its typesetting dropped, SRT given a
/// header and full stops, WebVTT as it is.
pub fn to_webvtt(text: &str, extension: Option<&str>) -> String {
    let text = text.trim_start_matches('\u{feff}');
    let ass = matches!(extension, Some("ass" | "ssa"))
        || text.contains("[Script Info]")
        || text.contains("[Events]");
    if ass {
        return vtt::convert(text).0;
    }
    if text.trim_start().starts_with("WEBVTT") {
        return text.to_owned();
    }
    srt_to_vtt(text)
}

/// SRT to WebVTT: a header, and full stops for the decimal commas.
fn srt_to_vtt(srt: &str) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for line in srt.lines() {
        if line.contains("-->") {
            out.push_str(&line.replace(',', "."));
        } else if !line.trim().is_empty() && line.trim().chars().all(|c| c.is_ascii_digit()) {
            continue;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// What a plan's try came to.
enum Tried {
    Played,
    /// Not played, and what that teaches.
    Refused(Blame),
}

/// What a plan the TV didn't play is held against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Blame {
    /// Nothing: the TV never asked for media, or waited on a slow or failing source.
    Nothing,
    /// The format: the receiver refused it in its own words.
    Format,
    /// A copied audio codec at most: the plan stalled with every byte served.
    Audio,
}

/// Try `load`'s plans in order until one plays.
async fn attempt(inner: &Arc<Inner>, load: CastLoad) -> Result<CastPlan, CastFailure> {
    let setup = &inner.setup;
    let (info, cors) = media_info(setup, &load).await;
    // Kaeri's receiver is handed the stream as it is; the Default Media Receiver gets the
    // relay's conversions, and the capability clips.
    let own = inner.own_receiver().await;
    let plans = if own {
        pass_through_plans(&load.source, &info, &cors)?
    } else {
        let caps = setup.memory.caps(&setup.device.id);
        cast_plans(&load.source, &info, &caps, &cors, setup.encoder.as_ref())?
    };
    tracing::info!(plans = plans.len(), video = ?info.video, own, "casting");
    let audio = audio_choices(setup, &load, &info, &cors, own);

    // The text tracks, once for every plan.
    lock(&inner.resources.texts).clear();
    lock(&inner.resources.originals).clear();
    let mut originals = if own { original_subtitles(inner, &load) } else { Vec::new() };
    let mut tracks = Vec::new();
    for (index, subtitle) in load.subtitles.iter().enumerate() {
        let id = u32::try_from(index + 1).unwrap_or(u32::MAX);
        match text_track(inner, subtitle, Some(id)) {
            Ok(track) => tracks.push(track),
            Err(e) => tracing::warn!(error = %e, "a subtitle couldn't be read"),
        }
    }
    let fonts = if own {
        let typeset_file = originals.iter().any(|o| o.format == "ass");
        let first_id = u32::try_from(load.subtitles.len() + 1).unwrap_or(u32::MAX);
        let (inside, fonts) = file_extras(inner, &load, &info, typeset_file, first_id).await;
        // The file's own tracks list beside the side-loaded ones; their WebVTT tracks are
        // empty stand-ins whose ids the receiver shows them by.
        tracks.extend(inside.iter().map(|o| inside_track(inner, o)));
        originals.extend(inside);
        fonts
    } else {
        Vec::new()
    };
    let active: Vec<u32> = load
        .subtitle
        .and_then(|i| u32::try_from(i + 1).ok())
        .filter(|id| tracks.iter().any(|t| t.id == *id))
        .into_iter()
        .collect();
    {
        let mut state = lock(&inner.state);
        state.audio = audio;
        state.subtitles = tracks
            .iter()
            .map(|t| CastText {
                id: t.id,
                language: track_language(t),
                title: track_title(t),
                selected: false,
            })
            .collect();
        state.subtitle = active.first().copied();
    }
    inner.publish();

    // A clip's refusal, when it is what ended the last plan that could be made.
    let mut why = None;
    // What Kaeri's receiver said failed: one more plan is tried after it, then it is said.
    let mut reported: Option<ReceiverError> = None;
    // A failure Kaeri's receiver couldn't place loads the same plan once more.
    let mut same_again = false;
    let mut plans = std::collections::VecDeque::from(plans);
    while let Some(plan) = plans.pop_front() {
        // What an earlier plan or clip of this session taught is heeded at once.
        let caps = setup.memory.caps(&setup.device.id);
        if !own && plan.parts().iter().any(|part| caps.refused(part)) {
            continue;
        }
        step(inner, CastStep::Waiting);
        if !own
            && plan.wants_clip()
            && !caps.played(&plan.video_part())
            && let Some(clip) = clip(&plan.video, plan.height)
            && !ask_with_clip(inner, &plan, clip, &load).await?
        {
            why = Some(Uncastable::VideoCodec(plan.video.to_string()));
            continue;
        }
        step(inner, CastStep::Preparing);
        let Some((url, content_type)) = stage(inner, &load, &info, &plan, own).await else {
            continue;
        };
        step(inner, CastStep::Waiting);
        let kaeri = own.then(|| {
            let state = lock(&inner.state);
            kaeri_load(
                &url,
                &plan,
                &info,
                &originals,
                load.subtitle.and_then(|i| originals.iter().find(|o| o.id as usize == i + 1)),
                &fonts,
                (state.language.as_deref(), state.device_word.as_deref()),
            )
        });
        let cast = Load {
            url,
            content_type,
            hls_segment_format: match plan.delivery {
                Delivery::HlsFmp4 => Some("fmp4".into()),
                Delivery::HlsTs => Some("ts".into()),
                _ => None,
            },
            metadata: load.metadata.clone(),
            start: load.start,
            tracks: tracks.clone(),
            active_tracks: active.clone(),
            style: None,
            autoplay: !lock(&inner.state).hold_paused,
            kaeri,
        };
        let tried = try_plan(inner, plan.relayed(), cast, own).await?;
        if own && !inner.runs_own() {
            // Kaeri's receiver gave way to the Default Media Receiver: its own plans.
            tracing::info!("planning again for the Default Media Receiver");
            return Box::pin(attempt(inner, load)).await;
        }
        match tried {
            Tried::Played => {
                if let Err(e) = setup.memory.record(&setup.device.id, &plan, true) {
                    tracing::warn!(error = %e, "couldn't note what the TV played");
                }
                return Ok(plan);
            }
            Tried::Refused(_) if own => {
                let said = what_the_receiver_said(inner).await;
                tracing::info!(route = ?plan.route, ?said, "Kaeri's receiver didn't play a plan");
                // No device converts video: a picture the TV can't decode ends it at once.
                if let Some(said) = said.as_ref().filter(|said| said.part == "video") {
                    return Err(said_failure(Some(said), Some(&plan)));
                }
                if said.as_ref().is_some_and(|said| said.part == "unknown") {
                    if same_again {
                        reported = said;
                        break;
                    }
                    same_again = true;
                    plans.push_front(plan.clone());
                    continue;
                }
                if reported.is_some() {
                    break;
                }
                reported = said;
            }
            Tried::Refused(blame) => {
                why = None;
                tracing::info!(route = ?plan.route, delivery = ?plan.delivery, ?blame, "the TV didn't play a plan");
                let noted = match blame {
                    Blame::Nothing => Ok(()),
                    Blame::Format => setup.memory.record(&setup.device.id, &plan, false),
                    Blame::Audio => {
                        setup.memory.record_stall(&setup.device.id, &plan).map(drop)
                    }
                };
                if let Err(e) = noted {
                    tracing::warn!(error = %e, "couldn't note what the TV refused");
                }
            }
        }
    }
    if own {
        return Err(said_failure(reported.as_ref(), None));
    }
    // When what this load taught leaves nothing, the reason is the part the TV refused.
    let caps = setup.memory.caps(&setup.device.id);
    cast_plans(&load.source, &info, &caps, &cors, setup.encoder.as_ref())?;
    Err(why.map_or(CastFailure::Refused, CastFailure::Uncastable))
}

/// What Kaeri's receiver reported of the load it refused. It says it before `LOAD_FAILED`, but
/// the two are read on different paths, so the refusal can be heard first: a moment is given.
async fn what_the_receiver_said(inner: &Inner) -> Option<ReceiverError> {
    for _ in 0..10 {
        if let Some(said) = lock(&inner.state).reported.take() {
            return Some(said);
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    None
}

/// Why a cast on Kaeri's receiver didn't play: the codec it named, or the plain sentence.
fn said_failure(said: Option<&ReceiverError>, plan: Option<&CastPlan>) -> CastFailure {
    let Some(said) = said else { return CastFailure::Refused };
    match (said.part.as_str(), said.codec.as_deref()) {
        ("video", Some(codec)) => Uncastable::VideoCodec(named_codec(codec, plan, true)).into(),
        ("audio", Some(codec)) => {
            Uncastable::AudioCodec(named_codec(codec, plan, false)).into()
        }
        _ => CastFailure::Refused,
    }
}

/// The name a notice shows for a codec Kaeri's receiver named. The receiver gives the base of
/// the TV's codec string (`hev1`, `ec-3`) or echoes the load's own word (`video:mpeg2`), so the
/// loaded part keeps its own name and a codec string's base gets the common one.
fn named_codec(said: &str, plan: Option<&CastPlan>, video: bool) -> String {
    match plan {
        Some(plan) if video && said == plan.video.part() => return plan.video.to_string(),
        Some(CastPlan { audio: AudioPlan::Copy(codec), .. })
            if !video && said == codec.part() =>
        {
            return codec.to_string();
        }
        _ => {}
    }
    let base = said.strip_prefix(if video { "video:" } else { "audio:" }).unwrap_or(said);
    let name = match base.to_ascii_lowercase().as_str() {
        "avc1" | "avc3" | "h264" => "H.264",
        "hev1" | "hvc1" | "hevc" => "HEVC",
        "vp09" | "vp9" => "VP9",
        "vp08" | "vp8" => "VP8",
        "mp2v" | "mpeg2" | "mpeg-2" => "MPEG-2",
        "av01" | "av1" => "AV1",
        "mp4a" | "aac" => "AAC",
        "ac-3" | "ac3" => "AC-3",
        "ec-3" | "eac3" => "E-AC-3",
        "dts" | "dtsc" | "dtse" | "dtsh" | "dtsl" | "dtsx" => "DTS",
        "mlpa" | "truehd" => "TrueHD",
        "opus" => "Opus",
        "flac" => "FLAC",
        _ => return base.to_ascii_uppercase(),
    };
    name.to_owned()
}

/// A subtitle as it is, for Kaeri's receiver to draw: a side-loaded file served by the relay
/// with its format, or one of the file's own tracks, which the receiver reads itself.
#[derive(Debug, Clone)]
struct Original {
    id: u32,
    /// Null for a track inside the file.
    url: Option<String>,
    format: &'static str,
    language: Option<String>,
    title: Option<String>,
    /// The index among the file's own subtitle tracks, for a track inside it.
    inside: Option<u32>,
}

/// Every subtitle of `load` as it is, in the order of `load.subtitles`. A file that can't be
/// read is left out of the receiver's list; its WebVTT track still stands.
fn original_subtitles(inner: &Inner, load: &CastLoad) -> Vec<Original> {
    let mut originals = Vec::new();
    for (index, subtitle) in load.subtitles.iter().enumerate() {
        let id = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let extension = subtitle
            .path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let (format, kind) = match extension.as_str() {
            "ass" | "ssa" => ("ass", "text/x-ssa"),
            "srt" => ("srt", "application/x-subrip"),
            "vtt" => ("vtt", "text/vtt"),
            "sup" => ("pgs", "application/octet-stream"),
            "sub" | "idx" => ("vobsub", "application/octet-stream"),
            _ => ("srt", "text/plain"),
        };
        let bytes = match std::fs::read(&subtitle.path) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(error = %e, "a subtitle couldn't be read");
                continue;
            }
        };
        let name = format!("o/{id}.{extension}");
        lock(&inner.resources.originals).insert(name.clone(), (kind, Arc::new(bytes)));
        originals.push(Original {
            id,
            url: Some(inner.relay.dynamic_url(&name)),
            format,
            language: subtitle.language.clone(),
            title: subtitle.title.clone(),
            inside: None,
        });
    }
    originals
}

/// What a Matroska file carries for Kaeri's receiver beyond its picture and sound: its own
/// subtitle tracks, numbered from `first_id`, and the attached fonts the relay serves for them
/// to typeset with. The fonts are read only when something typeset can show: a side-loaded ASS
/// file or an ASS track.
async fn file_extras(
    inner: &Inner,
    load: &CastLoad,
    info: &MediaInfo,
    typeset_file: bool,
    first_id: u32,
) -> (Vec<Original>, Vec<String>) {
    if info.container != Some(Delivery::Mkv) {
        return (Vec::new(), Vec::new());
    }
    let (upstream, url, headers) =
        (Arc::clone(&inner.setup.upstream), load.url.clone(), load.headers.clone());
    let local = load.source.kind == SourceKind::File;
    let runtime = tokio::runtime::Handle::current();
    let read = tokio::task::spawn_blocking(move || -> std::io::Result<_> {
        let mut file = if local {
            RangeFile::local(Path::new(url.strip_prefix("file://").unwrap_or(&url)))?
        } else {
            RangeFile::remote(upstream, &url, headers, runtime)?
        };
        let header = mkv::header(&mut file)?;
        let subtitles: Vec<mkv::TrackInfo> =
            header.tracks.into_iter().filter(|t| t.kind == 17).collect();
        let typeset =
            typeset_file || subtitles.iter().any(|t| inside_format(&t.codec) == Some("ass"));
        let fonts = match header.attachments {
            // A font that can't be read leaves the tracks standing, drawn in the fallback font.
            Some(at) if typeset => mkv::fonts(&mut file, at).unwrap_or_else(|e| {
                tracing::info!(error = %e, "the file's fonts couldn't be read");
                Vec::new()
            }),
            _ => Vec::new(),
        };
        Ok((subtitles, fonts))
    })
    .await;
    let (subtitles, fonts) = match read {
        Ok(Ok(read)) => read,
        Ok(Err(e)) => {
            tracing::info!(error = %e, "the file's subtitles and fonts couldn't be read");
            return (Vec::new(), Vec::new());
        }
        Err(_) => return (Vec::new(), Vec::new()),
    };
    let mut inside = Vec::new();
    let mut id = first_id;
    for (index, track) in subtitles.iter().enumerate() {
        let Some(format) = inside_format(&track.codec) else { continue };
        inside.push(Original {
            id,
            url: None,
            format,
            language: Some(track.language.clone()).filter(|l| !l.is_empty() && l != "und"),
            title: track.name.clone().filter(|name| !name.is_empty()),
            inside: u32::try_from(index).ok(),
        });
        id = id.saturating_add(1);
    }
    let mut urls = Vec::new();
    for (number, font) in fonts.into_iter().enumerate() {
        let Some((extension, kind)) = font.font() else { continue };
        let name = format!("f/{}.{extension}", number + 1);
        lock(&inner.resources.originals).insert(name.clone(), (kind, Arc::new(font.data)));
        urls.push(inner.relay.dynamic_url(&name));
    }
    (inside, urls)
}

/// The format Kaeri's receiver reads a Matroska subtitle track as, from its codec id; `None`
/// for one it doesn't show.
fn inside_format(codec: &str) -> Option<&'static str> {
    Some(match codec {
        "S_TEXT/ASS" | "S_TEXT/SSA" | "S_ASS" | "S_SSA" => "ass",
        "S_TEXT/UTF8" | "S_TEXT/ASCII" => "srt",
        "S_TEXT/WEBVTT" | "D_WEBVTT/SUBTITLES" => "vtt",
        "S_HDMV/PGS" => "pgs",
        "S_VOBSUB" => "vobsub",
        _ => return None,
    })
}

/// What Kaeri's receiver reads beside the media: where the stream comes from, its container,
/// the audio track to play, the subtitles as they are with the file's fonts, and the sender's
/// language and device word for its first screen.
fn kaeri_load(
    url: &str,
    plan: &CastPlan,
    info: &MediaInfo,
    originals: &[Original],
    active: Option<&Original>,
    fonts: &[String],
    (language, device): (Option<&str>, Option<&str>),
) -> Value {
    let container = match plan.delivery {
        Delivery::HlsTs | Delivery::HlsFmp4 => "hls",
        Delivery::Mp4 => "mp4",
        Delivery::Mkv => "mkv",
        Delivery::WebM => "webm",
    };
    let audio = info.chosen_audio().map(|track| {
        json!({
            "index": info.audio_choice,
            "language": track.language,
            "codec": track.codec.as_ref().map(AudioCodec::part),
        })
    });
    json!({
        "version": KAERI_PROTOCOL,
        "url": url,
        "route": if plan.relayed() { "relay" } else { "direct" },
        "container": container,
        "segments": match plan.delivery {
            Delivery::HlsFmp4 => Some("fmp4"),
            Delivery::HlsTs => Some("ts"),
            _ => None,
        },
        "video": { "codec": plan.video.part(), "height": plan.height },
        "audio": audio,
        "subtitles": originals.iter().map(|o| {
            let mut entry = json!({
                "id": o.id,
                "url": o.url,
                "format": o.format,
                "language": o.language,
                "title": o.title,
            });
            if let Some(inside) = o.inside {
                entry["inside"] = inside.into();
            }
            entry
        }).collect::<Vec<_>>(),
        "subtitle": active.map(|o| o.id),
        "fonts": fonts,
        "language": language,
        "device": device,
    })
}

/// Say what a load not yet playing is doing.
fn step(inner: &Inner, step: CastStep) {
    let changed = {
        let mut state = lock(&inner.state);
        let changed = state.step != Some(step);
        state.step = Some(step);
        changed
    };
    if changed {
        inner.publish();
    }
}

/// The audio tracks of `info` the card offers, each one the TV could be given, when there is
/// more than one: a file the relay remuxes, or an HLS master's renditions, which the TV is
/// handed one at a time.
fn audio_choices(
    setup: &CastSetup,
    load: &CastLoad,
    info: &MediaInfo,
    cors: &probe::CorsProbe,
    own: bool,
) -> Vec<CastAudio> {
    let remuxed = info.container == Some(Delivery::Mkv);
    if !(own || remuxed || info.renditions) || info.audio.len() < 2 {
        return Vec::new();
    }
    let caps = setup.memory.caps(&setup.device.id);
    info.audio
        .iter()
        .enumerate()
        // Kaeri's receiver plays every track, decoding what the TV lacks itself.
        .filter(|(index, _)| {
            if own {
                return true;
            }
            let info = MediaInfo { audio_choice: *index, ..info.clone() };
            cast_plans(&load.source, &info, &caps, cors, setup.encoder.as_ref()).is_ok_and(
                |plans| {
                    !remuxed
                        || plans.iter().any(|p| p.route == CastRoute::Relayed { remux: true })
                },
            )
        })
        .map(|(index, track)| CastAudio {
            index,
            language: track.language.clone(),
            title: audio_title(track),
            codec: track.codec.as_ref().map(ToString::to_string),
            channels: track.channels,
            selected: index == info.audio_choice,
        })
        .collect()
}

/// A text track's language as the file named it, `None` for "und".
fn track_language(track: &TextTrack) -> Option<String> {
    Some(track.language.clone()).filter(|l| !l.is_empty() && l != "und")
}

/// A text track's own name, when it is more than its language or a number.
fn track_title(track: &TextTrack) -> Option<String> {
    Some(track.name.clone()).filter(|name| {
        !name.is_empty()
            && Some(name) != track_language(track).as_ref()
            && !name.starts_with("Subtitles ")
    })
}

/// Ask the TV whether it decodes `plan`'s picture with `clip`, before a file whose sound would
/// play without it. A refusal is noted for this TV and the plan skipped; a clip that plays is
/// noted too. A TV that never asked for the clip said nothing, and the plan is tried.
async fn ask_with_clip(
    inner: &Arc<Inner>,
    plan: &CastPlan,
    clip: Clip,
    load: &CastLoad,
) -> Result<bool, CastFailure> {
    let setup = &inner.setup;
    let url = inner.relay.share_bytes(clip.name, clip.content_type, clip.bytes);
    let cast = Load {
        url,
        content_type: clip.content_type.to_owned(),
        hls_segment_format: None,
        metadata: load.metadata.clone(),
        start: None,
        tracks: Vec::new(),
        active_tracks: Vec::new(),
        style: None,
        autoplay: true,
        kaeri: None,
    };
    let part = plan.video_part();
    // A clip has no sound, so only the receiver's own refusal says it lacks the decoder.
    let played = match try_plan(inner, true, cast, false).await? {
        Tried::Played => true,
        Tried::Refused(Blame::Format) => false,
        Tried::Refused(Blame::Nothing | Blame::Audio) => return Ok(true),
    };
    tracing::info!(clip = clip.name, part, played, "asked the TV with a clip");
    if let Err(e) = setup.memory.note(&setup.device.id, &part, played) {
        tracing::warn!(error = %e, "couldn't note the clip's answer");
    }
    Ok(played)
}

/// Make `plan`'s address ready: the relay's route to the source, or the remuxer's playlist.
/// `None` when the plan can't be made, as for a file the remuxer can't read.
async fn stage(
    inner: &Inner,
    load: &CastLoad,
    info: &MediaInfo,
    plan: &CastPlan,
    own: bool,
) -> Option<(String, String)> {
    let relay = &inner.relay;
    *lock(&inner.resources.remuxer) = None;
    *lock(&inner.resources.master) = None;
    let content_type = plan.delivery.content_type().to_owned();
    match &plan.route {
        CastRoute::Direct { url } => {
            // Kaeri's receiver switches renditions itself; it is handed the whole master.
            if !own
                && let Some(master) = chosen_master(inner, info, plan, url, &[], false).await
            {
                return Some((master, content_type));
            }
            Some((url.clone(), content_type))
        }
        CastRoute::Relayed { remux: true } => {
            let options = RemuxOptions {
                audio_track: info.audio_choice,
                audio: match &plan.audio {
                    AudioPlan::ToAac { .. } => {
                        AudioMode::ToAac(Arc::clone(&inner.setup.encoder))
                    }
                    _ => AudioMode::Copy,
                },
            };
            let (upstream, url, headers) =
                (Arc::clone(&inner.setup.upstream), load.url.clone(), load.headers.clone());
            let local = load.source.kind == SourceKind::File;
            let runtime = tokio::runtime::Handle::current();
            let opened = tokio::task::spawn_blocking(move || {
                let source: Box<dyn Source> = if local {
                    let path = url.strip_prefix("file://").unwrap_or(&url);
                    Box::new(RangeFile::local(Path::new(path))?)
                } else {
                    Box::new(RangeFile::remote(upstream, &url, headers, runtime)?)
                };
                Remuxer::open(source, options).map_err(|e| std::io::Error::other(e.to_string()))
            })
            .await
            .ok()?;
            match opened {
                Ok(remuxer) => {
                    *lock(&inner.resources.remuxer) = Some(Arc::new(remuxer));
                    Some((relay.dynamic_url("r/index.m3u8"), content_type))
                }
                Err(e) => {
                    tracing::info!(error = %e, "the remuxer can't read this file; passing it through");
                    None
                }
            }
        }
        CastRoute::Relayed { remux: false } => {
            if load.source.kind == SourceKind::File {
                let path = load.url.strip_prefix("file://").unwrap_or(&load.url);
                return Some((relay.share_file(Path::new(path)), content_type));
            }
            relay.set_headers(load.headers.clone());
            if !own
                && let Some(master) =
                    chosen_master(inner, info, plan, &load.url, &load.headers, true).await
            {
                return Some((master, content_type));
            }
            Some((relay.upstream_url(&load.url), content_type))
        }
    }
}

/// For an HLS master with audio renditions, the master with the chosen one alone, served by
/// the relay: its addresses point back through the relay when `relayed`, and otherwise straight
/// at the source, so the TV still fetches the stream itself. `None` when there is no choice to
/// make or the master can't be read: then the TV is handed the address as it is.
async fn chosen_master(
    inner: &Inner,
    info: &MediaInfo,
    plan: &CastPlan,
    url: &str,
    headers: &[(String, String)],
    relayed: bool,
) -> Option<String> {
    if !info.renditions || !plan.delivery.is_hls() {
        return None;
    }
    let chosen = info.chosen_audio()?;
    let response = inner.setup.upstream.get(url, headers).await.ok()?;
    if !(200..300).contains(&response.status) {
        return None;
    }
    let landed = response.url.clone().unwrap_or_else(|| url.to_owned());
    let body = response.bytes(4 << 20).await.ok()?;
    let text = String::from_utf8_lossy(&body);
    let Some(kept) =
        master::keep_audio(&text, chosen.language.as_deref(), chosen.name.as_deref())
    else {
        tracing::info!("the master's audio couldn't be chosen; handing it over as it is");
        return None;
    };
    let rewritten = if relayed {
        inner.relay.relay_playlist(&kept, &landed)
    } else {
        rewrite_playlist(&kept, &landed, &str::to_owned)
    };
    tracing::info!(
        language = chosen.language.as_deref().unwrap_or("none"),
        relayed,
        "handing the TV the master with one audio rendition"
    );
    *lock(&inner.resources.master) = Some(Arc::new(rewritten.into_bytes()));
    Some(inner.relay.dynamic_url(MASTER))
}

/// Whether what the TV did says anything about the format: it was served media, in good time,
/// and that media started as a format it plays.
fn fair(inner: &Inner) -> bool {
    let source = inner.relay.source();
    inner.relay.media_served() && !source.busy && !source.troubled && !source.unreadable
}

/// What a plan the TV didn't play teaches, by how it went.
fn refused(inner: &Inner) -> Tried {
    // A TV that gave up while it waited on a slow source said nothing about the format.
    Tried::Refused(if fair(inner) { Blame::Format } else { Blame::Nothing })
}

/// What a plan that never moved teaches: a stall says less than a refusal (see
/// [`CastCaps::record_stall`]), and one behind a slow source says nothing.
///
/// [`CastCaps::record_stall`]: crate::memory::CastCaps::record_stall
fn stalled(inner: &Inner) -> Tried {
    Tried::Refused(if fair(inner) { Blame::Audio } else { Blame::Nothing })
}

/// Whether a plan that hasn't moved `waited` after its load has had its chance: the TV is
/// waiting on nothing of ours, and when the source was slow, it has had `movement` since the
/// source caught up.
fn had_its_chance(inner: &Inner, waited: Duration) -> bool {
    let movement = inner.setup.timings.movement;
    let source = inner.relay.source();
    waited >= movement && !source.busy && (!source.troubled || source.quiet >= movement)
}

/// Load one plan, or a capability clip, and watch whether it plays. `relayed` when the TV
/// reads it from the relay.
///
/// Only the receiver's own word (`LOAD_FAILED`, `ERROR`, idle with an error) is a refusal of
/// the format. A plan that doesn't move is a stall, and while the TV waits on the source behind
/// the relay (a torrent that has only just started, a slow CDN) it is not even that: the plan
/// is given until [`Timings::patience`].
async fn try_plan(
    inner: &Arc<Inner>,
    relayed: bool,
    cast: Load,
    own: bool,
) -> Result<Tried, CastFailure> {
    loop {
        let tried = try_plan_once(inner, relayed, cast.clone()).await;
        // Kaeri's receiver closed under the load: the same plan on the Default Media Receiver,
        // or, for a plan made for Kaeri's receiver, new plans.
        let again = std::mem::take(&mut lock(&inner.state).relaunch);
        if !again {
            return tried;
        }
        if own {
            return Ok(Tried::Refused(Blame::Nothing));
        }
        tracing::info!("trying the plan again on the Default Media Receiver");
    }
}

/// One try of [`try_plan`] on the receiver app running, or launched for it.
async fn try_plan_once(
    inner: &Arc<Inner>,
    relayed: bool,
    cast: Load,
) -> Result<Tried, CastFailure> {
    const TICK: Duration = Duration::from_millis(100);
    let timings = inner.setup.timings;
    inner.relay.reset_stats();
    let lost = |e: CastError| CastFailure::Lost(e.to_string());
    let mut media = inner.app().await.map_err(lost)?;
    {
        let mut state = lock(&inner.state);
        state.last = None;
        state.errored = false;
        state.reported = None;
        state.load = Some(cast.clone());
    }
    let started = Instant::now();
    let never_fetched = || relayed && inner.relay.requests() == 0;
    let closed = || {
        let state = lock(&inner.state);
        if state.relaunch {
            return Some(CastFailure::Lost("Kaeri's receiver closed".into()));
        }
        state
            .closed
            .then(|| state.failure.clone().unwrap_or(CastFailure::Lost("closed".into())))
    };

    // Until the TV answers the load.
    let answer = {
        let loading = media.load(&cast);
        tokio::pin!(loading);
        loop {
            tokio::select! {
                answer = &mut loading => break answer,
                () = tokio::time::sleep(TICK.min(timings.poll)) => {}
            }
            if let Some(failure) = closed() {
                return Err(failure);
            }
            if lock(&inner.state).errored {
                return Ok(refused(inner));
            }
            let waited = started.elapsed();
            if never_fetched() {
                if waited >= timings.fetch {
                    return Err(CastFailure::NeverFetches);
                }
                continue;
            }
            if waited >= timings.fetch && had_its_chance(inner, waited) {
                return Ok(stalled(inner));
            }
            if waited >= timings.patience {
                return Ok(Tried::Refused(Blame::Nothing));
            }
        }
    };
    let first = match answer {
        Ok(status) => status,
        Err(CastError::Refused(_)) => return Ok(refused(inner)),
        Err(CastError::Timeout(_)) if never_fetched() => return Err(CastFailure::NeverFetches),
        Err(CastError::Timeout(_)) => return Ok(stalled(inner)),
        Err(e) => return Err(lost(e)),
    };

    // Until its position moves.
    *lock(&inner.media) = Some(media.clone());
    let session = first.media_session_id;
    let from = first.position;
    let answered = Instant::now();
    loop {
        tokio::time::sleep(TICK.min(timings.poll)).await;
        if let Some(failure) = closed() {
            return Err(failure);
        }
        {
            let state = lock(&inner.state);
            if state.errored {
                return Ok(refused(inner));
            }
            if let Some(last) = state.last.as_ref().filter(|s| s.media_session_id == session) {
                match last.idle_reason.as_deref() {
                    // A short file, a capability clip above all, can end between two polls.
                    Some("FINISHED") => return Ok(Tried::Played),
                    Some(_) => return Ok(refused(inner)),
                    None if last.position > from + 0.2 => return Ok(Tried::Played),
                    // Loaded paused, as asked: the TV took the file and waits.
                    None if !cast.autoplay && last.state == "PAUSED" => {
                        return Ok(Tried::Played);
                    }
                    None => {}
                }
            }
        }
        if never_fetched() && started.elapsed() >= timings.fetch {
            return Err(CastFailure::NeverFetches);
        }
        let waited = answered.elapsed();
        if had_its_chance(inner, waited) {
            return Ok(stalled(inner));
        }
        if waited >= timings.patience {
            return Ok(Tried::Refused(Blame::Nothing));
        }
    }
}

/// Whether `rate` is a speed to play at: a receiver's rate of 0 while it buffers is not.
fn is_rate(rate: f64) -> bool {
    rate.is_finite() && rate > 0.0
}

/// Apply a status the session may hear: `Loaded` for a new media session, then what changed.
fn apply(inner: &Inner, state: &mut State, status: &MediaStatus) {
    let new_session = state.loaded != Some(status.media_session_id);
    if let Some(reason) = status.idle_reason.as_deref() {
        if new_session || state.ended || state.reloading {
            return;
        }
        state.ended = true;
        match reason {
            "FINISHED" => inner.say_locked(state, PlaybackEvent::Ended { complete: true }),
            "INTERRUPTED" => {
                // Something else was loaded in its place.
                state.ended = false;
                inner.say_locked(state, PlaybackEvent::Replaced);
            }
            _ => inner.say_locked(state, PlaybackEvent::Ended { complete: false }),
        }
        return;
    }
    if status.state == "IDLE" {
        return;
    }
    if new_session && state.loaded.is_some() && !state.reloading {
        // Another sender loaded its own file on the same receiver session: what plays now isn't
        // this cast's, and the TV is theirs.
        end_cast(inner, state, CastFailure::TakenOver);
        return;
    }
    if new_session {
        state.loaded = Some(status.media_session_id);
        state.ended = false;
        if state.reloading {
            // Our own reload, for a late subtitle: the same file, where it was.
            state.reloading = false;
        } else {
            state.paused = None;
            state.rate = None;
            inner.say_locked(state, PlaybackEvent::Loaded);
        }
    }
    // Buffering after it played keeps its place; a file still loading says where it began, not
    // where it got to, and is no progress: a stop then would save 0 s over the resume point.
    state.played |= matches!(status.state.as_str(), "PLAYING" | "PAUSED");
    if state.played {
        inner.say_locked(
            &*state,
            PlaybackEvent::Progress { position: status.position, duration: status.duration },
        );
    }
    let paused = status.state == "PAUSED";
    if state.paused != Some(paused) {
        state.paused = Some(paused);
        state.paused_since = paused.then(Instant::now);
        inner.say_locked(state, PlaybackEvent::Paused(paused));
    }
    let buffering = matches!(status.state.as_str(), "BUFFERING" | "LOADING");
    if state.buffering != buffering {
        state.buffering = buffering;
        inner.say_locked(state, PlaybackEvent::Buffering(buffering));
    }
    // A receiver whose player waits for data reports a rate of 0 (Shaka holds the video
    // element there while it buffers): the speed it plays at is unchanged.
    if is_rate(status.rate) && state.rate != Some(status.rate) {
        state.rate = Some(status.rate);
        inner.say_locked(state, PlaybackEvent::Speed(status.rate));
    }
}

/// Note the receiver's volume, and say it when it moved.
fn volume(inner: &Inner, state: &mut State, level: Option<f64>, muted: Option<bool>) {
    if let Some(level) = level
        && state.level != Some(level)
    {
        state.level = Some(level);
        inner.say_locked(state, PlaybackEvent::Volume((level * 100.0).round()));
    }
    if let Some(muted) = muted
        && state.muted != Some(muted)
    {
        state.muted = Some(muted);
    }
}

/// Read what the receiver says into the session's events. A connection that drops while the
/// receiver plays on is made again, within [`Timings::reconnect`], and the session goes on.
async fn pump(mut incoming: mpsc::UnboundedReceiver<Incoming>, inner: std::sync::Weak<Inner>) {
    loop {
        while let Some(message) = incoming.recv().await {
            let Some(inner) = inner.upgrade() else { return };
            receive(&inner, &message);
            inner.publish();
        }
        let Some(inner) = inner.upgrade() else { return };
        let failure = match reconnect(&inner).await {
            Ok(next) => {
                incoming = next;
                inner.publish();
                continue;
            }
            Err(failure) => failure,
        };
        // The TV went away, or what ran on it did.
        let mut state = lock(&inner.state);
        state.reconnecting = false;
        if !state.closed && !state.quit {
            tracing::info!(%failure, "cast ended");
            inner.relay.close();
            state.closed = true;
            state.failure.get_or_insert(failure);
            if !state.ended {
                state.ended = true;
                inner.say_locked(&state, PlaybackEvent::Ended { complete: false });
            }
        }
        drop(state);
        inner.publish();
        return;
    }
}

/// Why one try at connecting again didn't take.
enum Rejoin {
    /// The TV didn't answer: the next try may.
    Again(CastError),
    /// The TV answered, without the app this session ran: there is nothing to go back to.
    Over(CastFailure),
}

/// Connect to the TV again after the connection dropped, and join the receiver app this session
/// ran if it still runs: a phone's Wi-Fi napping with the screen off drops it while the TV
/// plays on. Tries until [`Timings::reconnect`] runs out; the stream returned is the new
/// connection's.
async fn reconnect(inner: &Inner) -> Result<mpsc::UnboundedReceiver<Incoming>, CastFailure> {
    let dropped = || CastFailure::Lost("the connection closed".into());
    let budget = inner.setup.timings.reconnect;
    {
        let mut state = lock(&inner.state);
        if state.closed || state.quit || budget.is_zero() {
            return Err(dropped());
        }
        state.reconnecting = true;
    }
    // The apps say so from the moment the connection closed.
    inner.publish();
    tracing::info!("the connection to the TV dropped; connecting again");
    let deadline = Instant::now() + budget;
    let mut network = inner.network.subscribe();
    network.borrow_and_update();
    let mut wait = Duration::from_millis(250);
    let result = loop {
        if lock(&inner.state).quit {
            break Err(dropped());
        }
        let left = deadline.saturating_duration_since(Instant::now());
        // Short tries, one after another: a try made while the phone was off the TV's network
        // waits on nothing, and the next one, once it is back, answers at once.
        let tried = tokio::time::timeout(left.min(inner.setup.timings.retry), rejoin(inner));
        let moved = tokio::select! {
            tried = tried => {
                match tried {
                    Ok(Ok(incoming)) => break Ok(incoming),
                    Ok(Err(Rejoin::Over(failure))) => break Err(failure),
                    Ok(Err(Rejoin::Again(e))) => {
                        tracing::debug!(error = %e, "the TV didn't answer yet");
                    }
                    Err(_) => tracing::debug!("the TV didn't answer in time"),
                }
                false
            }
            // The network changed under the try: the next one goes out on the new one.
            Ok(()) = network.changed() => true,
        };
        if moved {
            tracing::debug!("the network changed; trying the TV again at once");
            wait = Duration::from_millis(250);
            if Instant::now() >= deadline {
                break Err(dropped());
            }
            continue;
        }
        if Instant::now() + wait >= deadline {
            break Err(dropped());
        }
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            Ok(()) = network.changed() => {
                tracing::debug!("the network changed; trying the TV again at once");
            }
        }
        wait = (wait * 2).min(Duration::from_secs(1));
    };
    // A connection made again clears it here; tries that ran out clear it as the cast ends, so
    // the status never reads as playing between the two.
    if result.is_ok() {
        lock(&inner.state).reconnecting = false;
    }
    match &result {
        Ok(_) => tracing::info!("connected to the TV again"),
        Err(failure) => tracing::info!(%failure, "couldn't connect to the TV again"),
    }
    result
}

/// One try at [`reconnect`].
async fn rejoin(inner: &Inner) -> Result<mpsc::UnboundedReceiver<Incoming>, Rejoin> {
    let timings = &inner.setup.timings;
    let (device, incoming) =
        Device::connect_within(inner.setup.device.address, timings.silence, timings.retry)
            .await
            .map_err(Rejoin::Again)?;
    let receiver = device.status().await.map_err(Rejoin::Again)?;
    let ours = lock(&inner.state).app.clone();
    if let Some(ours) = &ours {
        match receiver.app {
            // It runs on: whatever closed went with the old connection.
            Some(app) if app.session_id == ours.session_id => lock(&inner.state).gone = None,
            Some(_) => return Err(Rejoin::Over(CastFailure::TakenOver)),
            None => {
                return Err(Rejoin::Over(CastFailure::Lost("the TV closed the app".into())));
            }
        }
        device.channel.connect_to(&ours.transport_id).map_err(Rejoin::Again)?;
    }
    if let Some(media) = lock(&inner.media).as_mut() {
        media.channel = device.channel.clone();
    }
    *lock(&inner.device) = device;
    // Kaeri's receiver shows the sender back, and the first poll says where the TV is.
    if let Some(app) = ours.filter(|a| a.app_id != DEFAULT_MEDIA_RECEIVER) {
        inner.hello(&app);
    }
    if let Some(media) = inner.media()
        && let Err(e) = media.poll()
    {
        tracing::debug!(error = %e, "couldn't ask the TV what plays");
    }
    Ok(incoming)
}

fn receive(inner: &Arc<Inner>, message: &Incoming) {
    let mut state = lock(&inner.state);
    if message.namespace == NS_RECEIVER && message.payload["type"] == "RECEIVER_STATUS" {
        let Some(receiver) = ReceiverStatus::read(&message.payload) else { return };
        // The TV's volume and mute are the receiver's alone: a media status carries the
        // stream's, which a TV muted at the receiver never shows.
        state.control = receiver.volume;
        state.want.volume(receiver.level, receiver.muted);
        volume(inner, &mut state, receiver.level, receiver.muted);
        let Some(ours) = state.app.as_ref().map(|a| a.session_id.clone()) else { return };
        if state.quit || state.closed {
            return;
        }
        let still_ours =
            message.payload["status"]["applications"].as_array().is_some_and(|apps| {
                apps.iter().any(|a| a["sessionId"].as_str() == Some(ours.as_str()))
            });
        if still_ours {
            if let Some(gone) = state.gone.as_mut() {
                gone.listed = Some(true);
            }
        } else if receiver.app.is_some() {
            // Another app runs where this one did: another sender took the TV.
            end_cast(inner, &mut state, CastFailure::TakenOver);
        } else if let Some(gone) = state.gone.as_mut() {
            gone.listed = Some(false);
        } else {
            // Nothing runs: stopped on the TV, or about to make way for another sender's app.
            app_gone(inner, &mut state, Some(false));
        }
        return;
    }
    let ours = state.app.as_ref().is_some_and(|a| a.transport_id == message.source);
    if message.namespace == NS_CONNECTION && message.payload["type"] == "CLOSE" && ours {
        // The receiver's status says why: it may come after the close, or a moment later.
        if !state.quit && !state.closed && state.gone.is_none() {
            app_gone(inner, &mut state, None);
        }
        return;
    }
    if message.namespace == NS_KAERI {
        from_kaeris_receiver(inner, &mut state, message, ours);
        return;
    }
    if message.namespace != NS_MEDIA {
        return;
    }
    if message.payload["type"] == "ERROR" {
        tracing::info!(payload = %message.payload, "the receiver reported an error");
        if state.trying {
            state.errored = true;
        }
        return;
    }
    let Some(mut status) = MediaStatus::first(&message.payload) else { return };
    // A status after a pause, a seek or a buffering spell leaves the media's description out:
    // the duration and title stand, so the card never reads as a file of no length.
    if let Some(last) = &state.last {
        status.fill_from(last);
    }
    state.want.media(&status);
    let waits = status.state == "BUFFERING" || status.state == "LOADING";
    state.waiting_since = waits.then(|| state.waiting_since.unwrap_or_else(Instant::now));
    state.last = Some(status.clone());
    if !state.trying {
        apply(inner, &mut state, &status);
    }
}

/// A message of Kaeri's receiver on its own namespace. `ready` comes while it launches, before
/// it is the app in force; the rest only from the app in force.
fn from_kaeris_receiver(inner: &Inner, state: &mut State, message: &Incoming, ours: bool) {
    let payload = &message.payload;
    let kind = payload["type"].as_str().unwrap_or_default();
    if kind == "ready" {
        inner.ready.send_replace(Some(payload.clone()));
        return;
    }
    if !ours {
        return;
    }
    let heard = match kind {
        "subtitles-plain" => FromReceiver::SubtitlesPlain,
        "up-next" => FromReceiver::UpNext { play: payload["action"] == "play" },
        "sender-gone" => {
            FromReceiver::SenderGone { waited: payload["waited"].as_f64().unwrap_or(0.0) }
        }
        "error" => {
            let error = ReceiverError::read(payload);
            tracing::info!(?error, "Kaeri's receiver reported an error");
            match error.part.as_str() {
                // The episode plays on with plain subtitles, or none.
                "subtitles" => {}
                _ if state.trying => {
                    state.errored = true;
                    state.reported = Some(error.clone());
                }
                part if !state.ended && state.plan.is_some() => {
                    let relayed = state.plan.as_ref().is_some_and(CastPlan::relayed);
                    let failure = match part {
                        "video" | "audio" => said_failure(Some(&error), state.plan.as_ref()),
                        "network" | "manifest"
                            if relayed && (error.lost || error.status == Some(0)) =>
                        {
                            CastFailure::LostStream
                        }
                        "network" | "manifest" => CastFailure::Segment,
                        _ => CastFailure::Refused,
                    };
                    tracing::warn!(%failure, "the cast stopped mid-episode");
                    state.failure = Some(failure);
                    state.ended = true;
                    inner.say_locked(state, PlaybackEvent::Ended { complete: false });
                    inner.publish_locked(state);
                }
                _ => {}
            }
            FromReceiver::Error(error)
        }
        _ => return,
    };
    let _ = inner.heard.send(heard);
}

/// The receiver app left the TV: hear the receiver's statuses for [`Timings::gone`], then say
/// why with [`gone_why`]. Another app showing up ends the cast as taken over at once.
fn app_gone(inner: &Arc<Inner>, state: &mut State, listed: Option<bool>) {
    let since = Instant::now();
    state.gone = Some(Gone { since, listed });
    tracing::info!("the receiver app left the TV; hearing why");
    let device = inner.device();
    let wait = inner.setup.timings.gone;
    let inner = Arc::downgrade(inner);
    tokio::spawn(async move {
        // The answer is heard as any receiver status is.
        if let Ok(Err(e)) = tokio::time::timeout(wait, device.status()).await {
            tracing::debug!(error = %e, "couldn't ask the TV what runs");
        }
        drop(device);
        tokio::time::sleep_until((since + wait).into()).await;
        let Some(inner) = inner.upgrade() else { return };
        let mut state = lock(&inner.state);
        // A connection made again joins the app or ends the cast by itself.
        if state.gone.is_none_or(|gone| gone.since != since) || state.reconnecting {
            return;
        }
        gone_why(&inner, &mut state);
        drop(state);
        inner.publish();
    });
}

/// Why the receiver app left, by what the TV said since. Nothing running is a stop on the TV
/// (its remote, another phone's Cast card, the system's cast controls) and ends the cast as
/// stopped there. The app still listed, or no word at all, is an app that failed: the Default
/// Media Receiver takes over a load in flight, and anything else ends the cast as lost.
fn gone_why(inner: &Inner, state: &mut State) {
    let Some(gone) = state.gone.take() else { return };
    if state.closed || state.quit {
        return;
    }
    if gone.listed == Some(false) {
        end_cast(inner, state, CastFailure::StoppedElsewhere);
    } else if !falls_back(inner, state) {
        end_cast(inner, state, CastFailure::Lost("the TV closed the app".into()));
    }
}

/// Kaeri's receiver failed while a file was loading on it, so the Default Media Receiver takes
/// its place and the load is tried again there. `false` for any other app, and for Kaeri's
/// gone while idle or playing: it said it was ready and played, so the cast ends rather than
/// putting another app on the TV.
fn falls_back(inner: &Inner, state: &mut State) -> bool {
    let custom = state.app.as_ref().is_some_and(|app| app.app_id != DEFAULT_MEDIA_RECEIVER);
    if !custom || state.fallen_back || !state.trying {
        return false;
    }
    tracing::warn!("Kaeri's receiver failed at a load; the Default Media Receiver takes over");
    state.app = None;
    state.gone = None;
    state.fallen_back = true;
    state.relaunch = true;
    *lock(&inner.media) = None;
    true
}

/// The receiver app is gone, or someone else's: the cast is over.
fn end_cast(inner: &Inner, state: &mut State, failure: CastFailure) {
    if state.closed {
        return;
    }
    tracing::info!(%failure, "cast ended");
    inner.relay.close();
    state.closed = true;
    state.app = None;
    state.failure = Some(failure);
    if !state.ended {
        state.ended = true;
        inner.say_locked(state, PlaybackEvent::Ended { complete: false });
    }
}

/// Ask for the media status every [`Timings::poll`], and end a cast paused too long.
async fn poll(inner: std::sync::Weak<Inner>) {
    let period = inner.upgrade().map_or(Duration::from_secs(1), |i| i.setup.timings.poll);
    let mut tick = tokio::time::interval(period);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        let Some(inner) = inner.upgrade() else { return };
        let (done, idle) = {
            let state = lock(&inner.state);
            let idle = state
                .paused_since
                .filter(|_| state.paused == Some(true) && !state.trying)
                .is_some_and(|since| since.elapsed() >= inner.setup.timings.idle_close);
            (state.closed || state.quit, idle)
        };
        if done {
            return;
        }
        if idle {
            tracing::info!("paused too long; ending the cast");
            let _ = close(&inner).await;
            inner.relay.close();
            let mut state = lock(&inner.state);
            state.closed = true;
            if !state.ended {
                state.ended = true;
                inner.say_locked(&state, PlaybackEvent::Ended { complete: false });
            }
            drop(state);
            inner.publish();
            return;
        }
        // A poll that can't go out waits for the connection made again, or the end above.
        if let Some(media) = inner.media()
            && let Err(e) = media.poll()
        {
            tracing::trace!(error = %e, "couldn't ask the TV what plays");
        }
    }
}

#[cfg(test)]
#[path = "player_tests.rs"]
mod tests;
