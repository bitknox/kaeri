//! A Cast receiver on loopback, for tests: the platform receiver, the Default Media Receiver and
//! its media session, with playback simulated at a chosen speed.
//!
//! It speaks the same TLS and framing as a TV, with a self-signed test certificate (the files
//! beside this one, made for this alone). A `LOAD` fetches what it is given as the receiver does,
//! with the receiver's `Origin`: for HLS the playlist, the init segment and the first segment,
//! each needing CORS; otherwise the first bytes. What it fetched decides what it refuses, as
//! [`Refuse`] says. Then the position advances at `speed` seconds a second until the duration,
//! where it goes idle with `FINISHED`. Like the real receiver it reports on changes only, so a
//! sender that doesn't poll sees the position stand still.
//!
//! What it does not do: decode anything or answer `deviceauth`.

pub mod matroska;

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        ServerConfig,
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    },
};

use crate::{
    channel::{NS_CONNECTION, NS_HEARTBEAT, NS_MEDIA, NS_RECEIVER},
    client::{DEFAULT_MEDIA_RECEIVER, NS_KAERI},
    fetch::{PlainHttp, Upstream},
    probe::RECEIVER_ORIGIN,
    proto::{CastMessage, Payload},
    relay::resolve,
};

const CERT: &[u8] = include_bytes!("cert.der");
const KEY: &[u8] = include_bytes!("key.der");

/// How long a TV shows nothing running between one sender's app closing and another's coming
/// up, when the other sender takes over.
const TAKEOVER_GAP: Duration = Duration::from_millis(300);

/// How the fake turns down what it can't play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// `LOAD_FAILED`.
    LoadFailed,
    /// Accepts the load, and the position never moves.
    Stall,
    /// Accepts the load, then goes idle with `ERROR`.
    IdleError,
    /// Accepts the load, then sends an `ERROR` message, as the Nest Hub does for FLAC.
    Error,
}

/// What the fake refuses: a load whose fetched bytes contain one of `bytes` (a sample entry's
/// type, `hvc1` or `Opus`), or whose content type is one of `content_types`.
#[derive(Debug, Clone, Default)]
pub struct Refuse {
    pub bytes: Vec<Vec<u8>>,
    pub content_types: Vec<String>,
    pub how: Option<Refusal>,
    /// A picture it refuses beside sound it plays goes unremarked: the sound plays, the position
    /// moves, and nothing is reported, as the Nest Hub does with HEVC in a file.
    pub sound_alone: bool,
    /// HLS whose picture is taller than this is `LOAD_FAILED`: the Nest Hub stops at 720 lines.
    pub hls_taller_than: Option<u32>,
    /// What Kaeri's receiver says on its namespace as it refuses: `error {part, codec}`.
    pub says: Option<Value>,
}

/// What the fake does when asked to launch an app other than the Default Media Receiver: Kaeri's
/// own receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CustomReceiver {
    /// Launches it, and it plays as the Default Media Receiver does: a TV it is published to.
    #[default]
    Runs,
    /// Answers `LAUNCH_ERROR`, as a TV does for an app ID that isn't published to it.
    Unknown,
    /// Launches it, and its connection closes at its first `LOAD` while the TV still lists it,
    /// as a receiver page that fails does.
    ClosesAtLoad,
    /// Launches it, and it is stopped on the TV at its first `LOAD`, from the TV's remote or
    /// another phone's Cast card: its connection closes and no app runs afterwards.
    StoppedAtLoad,
    /// Launches it, and at its first `LOAD` it reports the file loading at 0 s, before it seeks
    /// to where the load starts, and is then stopped on the TV.
    StoppedWhileLoading,
    /// Launches it, and it never says `ready` nor answers a `LOAD`: a receiver page that
    /// didn't load.
    Silent,
}

/// How the fake answers a command (pause, seek, a buffering spell) with its status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Echo {
    /// Without the `media` object, as real receivers do.
    #[default]
    Partial,
    /// With the `media` object every time.
    Full,
    /// Without the `media` object, and only after this long: a TV slow to answer.
    Late(Duration),
}

/// How the fake is set up.
#[derive(Debug, Clone)]
pub struct FakeOptions {
    /// Seconds of playback a second.
    pub speed: f64,
    /// Every file's length unless its HLS playlist says.
    pub duration: f64,
    pub refuse: Refuse,
    /// The volume's `controlType`: `attenuation` or `fixed`.
    pub control: &'static str,
    /// Whether it fetches at all: a TV behind a firewall doesn't, and a `LOAD` then hangs.
    pub fetches: bool,
    /// How long the receiver app takes to come up after `LAUNCH`: 3 to 6.5 s on a real TV.
    pub launch: Duration,
    /// Plays a stream it can't fetch as though it had read it, for `duration`: for an app's
    /// fixtures, whose episodes are test patterns only mpv draws (`av://lavfi:…`).
    pub plays_unfetched: bool,
    /// What an app other than the Default Media Receiver does.
    pub custom: CustomReceiver,
    /// Keeps the receiver app, what it plays and the volume from one sender connection to the
    /// next, as a TV does: for a sender that comes back to a cast it left running.
    pub remembers: bool,
    /// How statuses after commands come.
    pub echo: Echo,
}

/// What a TV that [`FakeOptions::remembers`] keeps between sender connections.
#[derive(Debug, Clone)]
struct Kept {
    transport: String,
    app: Option<(String, String)>,
    playing: Option<Playing>,
    next_session: i64,
    level: f64,
    muted: bool,
}

impl Default for FakeOptions {
    fn default() -> Self {
        Self {
            speed: 1.0,
            duration: 20.0,
            refuse: Refuse::default(),
            control: "attenuation",
            fetches: true,
            launch: Duration::ZERO,
            plays_unfetched: false,
            custom: CustomReceiver::default(),
            remembers: false,
            echo: Echo::default(),
        }
    }
}

/// The `_googlecast._tcp` TXT record a fixture finds a fake under, as a TV with Chromecast built
/// in sends it: its id, its friendly name, its model and the video-out capability bit.
pub fn advert_txt(id: &str, name: &str, model: &str) -> Vec<(String, String)> {
    [("id", id), ("fn", name), ("md", model), ("ca", "4101")]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// One fetch the fake made: the URL, its status, the body's length, and whether the answer
/// carried CORS.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub url: String,
    pub status: u16,
    pub bytes: usize,
    pub cors: bool,
}

/// A running fake receiver.
pub struct FakeReceiver {
    pub address: SocketAddr,
    /// Every message the receiver was sent, as JSON with its namespace.
    pub heard: Arc<Mutex<Vec<(String, Value)>>>,
    pub fetched: Arc<Mutex<Vec<Fetched>>>,
    control: mpsc::UnboundedSender<Control>,
    task: JoinHandle<()>,
}

enum Control {
    TakeOver,
    Silence,
    /// This connection only: the TV goes on and answers the next.
    Drop,
    /// This connection is closed, and the TV goes on and answers the next.
    Close,
    /// The receiver app is stopped on the TV.
    StopApp,
    /// Another sender loads its own file on the same receiver session.
    LoadElsewhere,
    Buffering(bool),
    Say(Value),
    /// Each connection from now waits this long before it is answered.
    AnswerAfter(Duration),
}

impl Drop for FakeReceiver {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Debug, Clone)]
struct Playing {
    session: i64,
    url: String,
    duration: f64,
    position: f64,
    state: &'static str,
    rate: f64,
    tracks: Vec<u64>,
    since: Instant,
    stalled: bool,
    /// Waiting for data, as [`FakeReceiver::buffer`] says.
    buffering: bool,
    /// The LOAD's metadata, said back with the media as a receiver does.
    metadata: Value,
}

impl Playing {
    fn advance(&mut self, speed: f64) {
        if self.state == "PLAYING" && !self.stalled && !self.buffering {
            self.position += self.since.elapsed().as_secs_f64() * self.rate * speed;
        }
        self.since = Instant::now();
    }

    /// The status as a receiver sends it, with the media's description when `describe`: a
    /// receiver includes it in the answer to a `LOAD` or a `GET_STATUS`, and leaves it out of the
    /// statuses that follow a pause, a seek or a change in buffering.
    fn status(&self, idle_reason: Option<&str>, volume: &Value) -> Value {
        self.status_of(idle_reason, volume, true)
    }

    fn status_of(&self, idle_reason: Option<&str>, volume: &Value, describe: bool) -> Value {
        let playing = if self.buffering { "BUFFERING" } else { self.state };
        let mut status = json!({
            "mediaSessionId": self.session,
            // Shaka holds the video element's rate at 0 while it buffers, and the receiver
            // reports the element's rate.
            "playbackRate": if self.buffering { 0.0 } else { self.rate },
            "playerState": if idle_reason.is_some() { "IDLE" } else { playing },
            "currentTime": self.position.min(self.duration),
            "supportedMediaCommands": 12303,
            "volume": volume,
            "activeTrackIds": self.tracks,
            "media": {"contentId": self.url, "duration": self.duration, "streamType": "BUFFERED",
                      "metadata": self.metadata},
        });
        if let Some(reason) = idle_reason {
            status["idleReason"] = json!(reason);
        }
        if !describe && let Some(status) = status.as_object_mut() {
            status.remove("media");
        }
        status
    }
}

impl FakeReceiver {
    pub async fn start(options: FakeOptions) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
        let config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(std::io::Error::other)?
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(CERT.to_vec())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(KEY.to_vec())),
            )
            .map_err(std::io::Error::other)?;
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let heard: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
        let fetched: Arc<Mutex<Vec<Fetched>>> = Arc::default();
        let (control, mut controls) = mpsc::unbounded_channel();
        let kept: Arc<Mutex<Option<Kept>>> = Arc::default();
        let task = {
            let (heard, fetched) = (Arc::clone(&heard), Arc::clone(&fetched));
            tokio::spawn(async move {
                // One sender at a time, as the tests have it; the next connection takes over.
                let mut current: Option<JoinHandle<()>> = None;
                let mut forward: Option<mpsc::UnboundedSender<Control>> = None;
                // Gone silent, the TV answers no connection again: the socket is held unread.
                let mut gone = false;
                let mut held = Vec::new();
                let mut answer_after = Duration::ZERO;
                loop {
                    tokio::select! {
                        accepted = listener.accept() => {
                            let Ok((socket, _)) = accepted else { return };
                            if gone {
                                held.push(socket);
                                continue;
                            }
                            let acceptor = acceptor.clone();
                            let (heard, fetched, options, kept) = (
                                Arc::clone(&heard),
                                Arc::clone(&fetched),
                                options.clone(),
                                Arc::clone(&kept),
                            );
                            let (tx, rx) = mpsc::unbounded_channel();
                            forward = Some(tx);
                            if let Some(previous) = current.take() {
                                previous.abort();
                            }
                            current = Some(tokio::spawn(async move {
                                tokio::time::sleep(answer_after).await;
                                if let Ok(tls) = acceptor.accept(socket).await {
                                    let _ = serve(tls, heard, fetched, options, rx, kept).await;
                                }
                            }));
                        }
                        Some(control) = controls.recv() => {
                            if let Control::AnswerAfter(after) = control {
                                answer_after = after;
                                continue;
                            }
                            gone |= matches!(control, Control::Silence);
                            if let Some(forward) = &forward {
                                let _ = forward.send(control);
                            }
                        }
                    }
                }
            })
        };
        Ok(Self { address, heard, fetched, control, task })
    }

    /// The messages heard so far of `kind`, in any namespace.
    pub fn heard(&self, kind: &str) -> Vec<Value> {
        lock(&self.heard)
            .iter()
            .filter(|(_, v)| v["type"] == kind)
            .map(|(_, v)| v.clone())
            .collect()
    }

    pub fn fetched(&self) -> Vec<Fetched> {
        lock(&self.fetched).clone()
    }

    /// Another sender launches its own app, as when someone casts from their phone. As a TV
    /// does, the app running closes first, with nothing running for a moment, before the other
    /// app is up.
    pub fn take_over(&self) {
        let _ = self.control.send(Control::TakeOver);
    }

    /// Another sender loads its own file on the receiver session this one plays on.
    pub fn load_elsewhere(&self) {
        let _ = self.control.send(Control::LoadElsewhere);
    }

    /// Say nothing more and read nothing more, with the connection left open: a TV switched
    /// off at the wall, or gone from the Wi-Fi, which sends no FIN. It answers no connection again.
    pub fn go_silent(&self) {
        let _ = self.control.send(Control::Silence);
    }

    /// The connection this sender holds goes dead, unanswered and with no FIN, while the TV plays
    /// on and answers the next one: a phone whose Wi-Fi naps with the screen off.
    pub fn drop_connection(&self) {
        let _ = self.control.send(Control::Drop);
    }

    /// Answer each connection from now only after `delay`: a TV slow to take a sender back, so
    /// the time between a dropped connection and the next one can be seen.
    pub fn answer_after(&self, delay: Duration) {
        let _ = self.control.send(Control::AnswerAfter(delay));
    }

    /// The receiver app is stopped on the TV, with its remote or from another phone's Cast card:
    /// no app runs afterwards.
    pub fn stop_app(&self) {
        let _ = self.control.send(Control::StopApp);
    }

    /// The TV closes the connection this sender holds, with a FIN, and plays on and answers the
    /// next one: a TV that drops its senders when its Wi-Fi roams.
    pub fn close_connection(&self) {
        let _ = self.control.send(Control::Close);
    }

    /// Wait for data, or stop waiting, and say so as the receiver's Shaka player does:
    /// `BUFFERING` with a `playbackRate` of 0, then the rate it plays at again.
    pub fn buffer(&self, waiting: bool) {
        let _ = self.control.send(Control::Buffering(waiting));
    }

    /// Say `message` on Kaeri's namespace from the receiver app, as Kaeri's receiver does.
    pub fn say(&self, message: Value) {
        let _ = self.control.send(Control::Say(message));
    }
}

#[allow(clippy::too_many_lines)]
async fn serve(
    tls: tokio_rustls::server::TlsStream<TcpStream>,
    heard: Arc<Mutex<Vec<(String, Value)>>>,
    fetched: Arc<Mutex<Vec<Fetched>>>,
    options: FakeOptions,
    mut controls: mpsc::UnboundedReceiver<Control>,
    kept: Arc<Mutex<Option<Kept>>>,
) -> std::io::Result<()> {
    let (mut read, mut write) = tokio::io::split(tls);
    let (tx, mut rx) = mpsc::unbounded_channel::<CastMessage>();
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if write.write_all(&message.frame()).await.is_err() {
                break;
            }
        }
    });
    let mut transport = "web-1".to_owned();
    let mut app: Option<(String, String)> = None;
    let mut playing: Option<Playing> = None;
    let mut next_session = 1;
    let mut level = 0.4;
    let mut muted = false;
    let mut ticker = tokio::time::interval(Duration::from_millis(50));
    let send = |source: &str, destination: &str, namespace: &str, payload: Value| {
        let _ = tx.send(CastMessage::text(source, destination, namespace, payload.to_string()));
    };
    let volume = |level: f64, muted: bool| json!({"level": level, "muted": muted, "controlType": options.control, "stepInterval": 0.05});
    // A media status carries the stream's own volume, never the TV's: a TV muted at the receiver
    // still says its stream plays unmuted.
    let stream = json!({"level": 1.0, "muted": false});
    let receiver_status = |app: &Option<(String, String)>, request: u64, level, muted| {
        let applications: Vec<Value> = app
            .iter()
            .map(|(session, id)| {
                json!({"appId": id, "displayName": "Receiver", "sessionId": session,
                       "transportId": if id == DEFAULT_MEDIA_RECEIVER { "web-1" } else { "web-9" },
                       "isIdleScreen": false, "namespaces": [{"name": NS_MEDIA}]})
            })
            .collect();
        json!({"type": "RECEIVER_STATUS", "requestId": request, "status": {
            "applications": applications, "volume": volume(level, muted)}})
    };
    if options.remembers
        && let Some(was) = lock(&kept).clone()
    {
        (transport, app, playing, next_session, level, muted) =
            (was.transport, was.app, was.playing, was.next_session, was.level, was.muted);
    }
    let mut sender = String::from("*");
    loop {
        // Kept before each wait: a connection ends only there, when the next one takes over.
        if options.remembers {
            *lock(&kept) = Some(Kept {
                transport: transport.clone(),
                app: app.clone(),
                playing: playing.clone(),
                next_session,
                level,
                muted,
            });
        }
        tokio::select! {
            _ = ticker.tick() => {
                if let Some(p) = playing.as_mut() {
                    p.advance(options.speed);
                    if p.position >= p.duration {
                        let done = p.status(Some("FINISHED"), &stream);
                        playing = None;
                        send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0, "status": [done]}));
                    }
                }
            }
            Some(control) = controls.recv() => match control {
                Control::TakeOver => {
                    if app.take().is_some() {
                        send(&transport, &sender, NS_CONNECTION, json!({"type": "CLOSE"}));
                        send("receiver-0", "*", NS_RECEIVER, receiver_status(&app, 0, level, muted));
                        tokio::time::sleep(TAKEOVER_GAP).await;
                    }
                    app = Some(("someone-else".into(), "E0000000".into()));
                    playing = None;
                    send("receiver-0", "*", NS_RECEIVER, receiver_status(&app, 0, level, muted));
                }
                Control::Silence | Control::Drop => {
                    // Holding the socket, the writer and all, and never touching it again.
                    let _held = (&read, &tx);
                    std::future::pending::<()>().await;
                }
                // Kept above, so the next connection finds the TV as it was.
                Control::Close => return Ok(()),
                Control::StopApp => {
                    // The app's connection closes first, then the status says nothing runs.
                    if app.take().is_some() {
                        send(&transport, &sender, NS_CONNECTION, json!({"type": "CLOSE"}));
                    }
                    playing = None;
                    send("receiver-0", "*", NS_RECEIVER, receiver_status(&app, 0, level, muted));
                }
                Control::LoadElsewhere => {
                    if let Some(old) = playing.take() {
                        send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0,
                            "status": [old.status(Some("INTERRUPTED"), &stream)]}));
                        let theirs = Playing {
                            session: next_session,
                            url: "http://elsewhere.example/theirs.mp4".into(),
                            position: 0.0,
                            since: Instant::now(),
                            ..old
                        };
                        next_session += 1;
                        send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0,
                            "status": [theirs.status(None, &stream)]}));
                        playing = Some(theirs);
                    }
                }
                Control::Buffering(waiting) => {
                    if let Some(p) = playing.as_mut() {
                        p.advance(options.speed);
                        p.buffering = waiting;
                        send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0,
                            "status": [p.status_of(None, &stream, options.echo == Echo::Full)]}));
                    }
                }
                Control::Say(message) => send(&transport, "*", NS_KAERI, message),
                // Taken by the listener, never sent to a connection.
                Control::AnswerAfter(_) => {}
            },
            frame = read_frame(&mut read) => {
                let Some(message) = frame? else { return Ok(()) };
                let Payload::Text(text) = &message.payload else { continue };
                let Ok(payload) = serde_json::from_str::<Value>(text) else { continue };
                sender.clone_from(&message.source);
                lock(&heard).push((message.namespace.clone(), payload.clone()));
                let request = payload["requestId"].as_u64().unwrap_or(0);
                let kind = payload["type"].as_str().unwrap_or_default().to_owned();
                match (message.namespace.as_str(), kind.as_str()) {
                    (NS_HEARTBEAT, "PING") => send(&message.destination, &sender, NS_HEARTBEAT, json!({"type": "PONG"})),
                    (NS_RECEIVER, "GET_STATUS") => {
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request, level, muted));
                    }
                    (NS_RECEIVER, "SET_VOLUME") => {
                        if options.control != "fixed" && let Some(l) = payload["volume"]["level"].as_f64() {
                            level = l;
                        }
                        if let Some(m) = payload["volume"]["muted"].as_bool() {
                            muted = m;
                        }
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request, level, muted));
                    }
                    (NS_RECEIVER, "LAUNCH") => {
                        tokio::time::sleep(options.launch).await;
                        let id = payload["appId"].as_str().unwrap_or(DEFAULT_MEDIA_RECEIVER).to_owned();
                        if id != DEFAULT_MEDIA_RECEIVER && options.custom == CustomReceiver::Unknown {
                            send("receiver-0", &sender, NS_RECEIVER, json!({
                                "type": "LAUNCH_ERROR", "requestId": request, "reason": "NOT_FOUND"}));
                            continue;
                        }
                        transport = if id == DEFAULT_MEDIA_RECEIVER { "web-1" } else { "web-9" }.to_owned();
                        playing = None;
                        app = Some((format!("session-{request}"), id));
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request, level, muted));
                    }
                    (NS_KAERI, "hello") => {
                        let custom = app.as_ref().is_some_and(|(_, id)| id != DEFAULT_MEDIA_RECEIVER);
                        if custom && options.custom != CustomReceiver::Silent {
                            send(&transport, &sender, NS_KAERI, json!({"type": "ready", "version": 1,
                                "build": "fake", "caps": {"typeset": true, "audioSwitch": true}}));
                        }
                    }
                    (NS_RECEIVER, "STOP") => {
                        app = None;
                        playing = None;
                        send(&transport, &sender, NS_CONNECTION, json!({"type": "CLOSE"}));
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request, level, muted));
                    }
                    (NS_MEDIA, "LOAD") => {
                        let custom = app.as_ref().is_some_and(|(_, id)| id != DEFAULT_MEDIA_RECEIVER);
                        if custom && options.custom == CustomReceiver::Silent {
                            continue;
                        }
                        if custom && options.custom == CustomReceiver::ClosesAtLoad {
                            // The receiver page failed: its connection closes, the load goes
                            // unanswered, and the TV lists the app still.
                            playing = None;
                            send(&transport, &sender, NS_CONNECTION, json!({"type": "CLOSE"}));
                            continue;
                        }
                        if custom && options.custom == CustomReceiver::StoppedAtLoad {
                            app = None;
                            playing = None;
                            send(&transport, &sender, NS_CONNECTION, json!({"type": "CLOSE"}));
                            send("receiver-0", "*", NS_RECEIVER, receiver_status(&app, 0, level, muted));
                            continue;
                        }
                        if custom && options.custom == CustomReceiver::StoppedWhileLoading {
                            let session = next_session;
                            next_session += 1;
                            send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0,
                                "status": [{"mediaSessionId": session, "playbackRate": 1.0, "playerState": "LOADING",
                                    "currentTime": 0.0, "supportedMediaCommands": 12303, "volume": stream,
                                    "media": {"contentId": payload["media"]["contentId"], "duration": options.duration,
                                        "streamType": "BUFFERED"}}]}));
                            tokio::time::sleep(Duration::from_millis(200)).await;
                            app = None;
                            playing = None;
                            send(&transport, &sender, NS_CONNECTION, json!({"type": "CLOSE"}));
                            send("receiver-0", "*", NS_RECEIVER, receiver_status(&app, 0, level, muted));
                            continue;
                        }
                        if !options.fetches {
                            // A TV that can't reach the sender: the load hangs.
                            continue;
                        }
                        let url = payload["media"]["contentId"].as_str().unwrap_or_default().to_owned();
                        let content_type = payload["media"]["contentType"].as_str().unwrap_or_default().to_owned();
                        let hls = content_type.contains("mpegurl");
                        let result = fetch_like_a_receiver(&url, hls, &fetched).await;
                        let refused = match &result {
                            Ok((_, bytes)) => {
                                let mut info = crate::formats::MediaInfo::default();
                                crate::probe::read_sample_entries(bytes, &mut info);
                                let tall = hls && options.refuse.hls_taller_than.zip(info.height).is_some_and(|(limit, h)| h > limit);
                                let matched = options.refuse.content_types.contains(&content_type)
                                    || options.refuse.bytes.iter().any(|needle| contains(bytes, needle));
                                let sound = SOUND.iter().any(|needle| contains(bytes, needle));
                                if tall {
                                    Some(Refusal::LoadFailed)
                                } else {
                                    options.refuse.how.filter(|_| matched && !(options.refuse.sound_alone && sound && !hls))
                                }
                            }
                            Err(_) if options.plays_unfetched => options
                                .refuse
                                .how
                                .filter(|_| options.refuse.content_types.contains(&content_type)),
                            Err(_) => Some(Refusal::LoadFailed),
                        };
                        if let Some(said) = options.refuse.says.clone().filter(|_| custom && refused.is_some()) {
                            send(&transport, "*", NS_KAERI, said);
                        }
                        if refused == Some(Refusal::LoadFailed) {
                            send(&transport, &sender, NS_MEDIA, json!({
                                "type": "LOAD_FAILED", "requestId": request, "detailedErrorCode": 104}));
                            continue;
                        }
                        let duration = result.ok().and_then(|(d, _)| d).unwrap_or(options.duration);
                        let session = next_session;
                        next_session += 1;
                        if let Some(old) = playing.take() {
                            send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0,
                                "status": [old.status(Some("INTERRUPTED"), &stream)]}));
                        }
                        let p = Playing {
                            session,
                            url,
                            duration,
                            position: payload["currentTime"].as_f64().unwrap_or(0.0),
                            state: if payload["autoplay"] == false { "PAUSED" } else { "PLAYING" },
                            rate: 1.0,
                            tracks: payload["activeTrackIds"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default(),
                            since: Instant::now(),
                            stalled: refused == Some(Refusal::Stall),
                            buffering: false,
                            metadata: payload["media"]["metadata"].clone(),
                        };
                        send(&transport, &sender, NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": request, "status": [p.status(None, &stream)]}));
                        if refused == Some(Refusal::Error) {
                            send(&transport, "*", NS_MEDIA, json!({"type": "ERROR", "requestId": 0, "detailedErrorCode": 100}));
                            continue;
                        }
                        if refused == Some(Refusal::IdleError) {
                            send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0,
                                "status": [p.status(Some("ERROR"), &stream)]}));
                        } else {
                            playing = Some(p);
                        }
                    }
                    (NS_MEDIA, kind) => {
                        let Some(p) = playing.as_mut() else {
                            send(&transport, &sender, NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": request, "status": []}));
                            continue;
                        };
                        p.advance(options.speed);
                        let mut idle = None;
                        match kind {
                            "PLAY" => p.state = "PLAYING",
                            "PAUSE" => p.state = "PAUSED",
                            "SEEK" => p.position = payload["currentTime"].as_f64().unwrap_or(p.position),
                            "SET_PLAYBACK_RATE" => p.rate = payload["playbackRate"].as_f64().unwrap_or(1.0),
                            "EDIT_TRACKS_INFO" => p.tracks = payload["activeTrackIds"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default(),
                            "STOP" => idle = Some("CANCELLED"),
                            _ => {}
                        }
                        let status = p.status_of(idle, &stream, kind == "GET_STATUS" || options.echo == Echo::Full);
                        if idle.is_some() {
                            playing = None;
                        }
                        if let Echo::Late(late) = options.echo {
                            tokio::time::sleep(late).await;
                        }
                        send(&transport, &sender, NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": request, "status": [status]}));
                    }
                    _ => {}
                }
            }
        }
    }
}

/// What says a stream has sound: an MP4 audio sample entry, or a Matroska audio codec id.
const SOUND: [&[u8]; 6] = [b"mp4a", b"Opus", b"fLaC", b"A_AAC", b"A_OPUS", b"A_FLAC"];

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

async fn read_frame<R: AsyncReadExt + Unpin>(
    read: &mut R,
) -> std::io::Result<Option<CastMessage>> {
    let mut len = [0u8; 4];
    if read.read_exact(&mut len).await.is_err() {
        return Ok(None);
    }
    let mut body = vec![0u8; u32::from_be_bytes(len) as usize];
    read.read_exact(&mut body).await?;
    CastMessage::decode(&body).map(Some).map_err(std::io::Error::other)
}

/// GET `url` as the receiver does; for HLS the playlist, its init segment and its first
/// segment, each refused without CORS. The duration is the playlist's, when there is one, and
/// the bytes are everything fetched, for the refusal rules.
async fn fetch_like_a_receiver(
    url: &str,
    hls: bool,
    fetched: &Mutex<Vec<Fetched>>,
) -> Result<(Option<f64>, Vec<u8>), u16> {
    let get = |url: String| async move {
        let headers = [
            ("Origin".to_owned(), RECEIVER_ORIGIN.to_owned()),
            ("Range".to_owned(), "bytes=0-".to_owned()),
        ];
        let response = PlainHttp.get(&url, &headers).await.map_err(|_| 502u16)?;
        let (status, cors) =
            (response.status, response.header("access-control-allow-origin").is_some());
        let kind = response.header("content-type").map(str::to_owned);
        let body = response.prefix(4 << 20).await.map_err(|_| 502u16)?;
        Ok::<_, u16>((url, status, cors, kind, body))
    };
    let mut all = Vec::new();
    // A segment's type is believed, as the receiver's player believes it: one named as an
    // image is refused, whatever its bytes are.
    let mut note =
        |(url, status, cors, kind, body): (String, u16, bool, Option<String>, Vec<u8>),
         segment: bool| {
            lock(fetched).push(Fetched { url, status, bytes: body.len(), cors });
            if !(200..300).contains(&status) || (hls && !cors) {
                return Err(status);
            }
            if segment && kind.as_deref().is_some_and(|kind| !crate::probe::read_as_media(kind))
            {
                return Err(415);
            }
            all.extend_from_slice(&body);
            Ok(body)
        };
    let body = note(get(url.to_owned()).await?, false)?;
    if !hls {
        return Ok((None, all));
    }
    let mut url = url.to_owned();
    let mut text = String::from_utf8_lossy(&body).into_owned();
    // The audio rendition its player plays: the default one, as a receiver with nothing else
    // to go by picks.
    if let Some(audio) = text.lines().find_map(|l| {
        let at = l.find("URI=\"")? + 5;
        (l.starts_with("#EXT-X-MEDIA:")
            && l.contains("TYPE=AUDIO")
            && l.contains("DEFAULT=YES"))
        .then(|| l[at..].split('"').next().map(str::to_owned))?
    }) {
        note(get(resolve(&url, &audio)).await?, false)?;
    }
    if let Some(variant) = text
        .lines()
        .skip_while(|l| !l.starts_with("#EXT-X-STREAM-INF"))
        .find(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        url = resolve(&url, variant.trim());
        let body = note(get(url.clone()).await?, false)?;
        text = String::from_utf8_lossy(&body).into_owned();
    }
    let duration: f64 = text
        .lines()
        .filter_map(|l| l.strip_prefix("#EXTINF:"))
        .filter_map(|l| l.split(',').next()?.parse::<f64>().ok())
        .sum();
    if let Some(map) = text.lines().find_map(|l| {
        let at = l.find("URI=\"")? + 5;
        l.starts_with("#EXT-X-MAP").then(|| l[at..].split('"').next().map(str::to_owned))?
    }) {
        note(get(resolve(&url, &map)).await?, true)?;
    }
    if let Some(first) = text.lines().find(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        note(get(resolve(&url, first.trim())).await?, true)?;
    }
    Ok(((duration > 0.0).then_some(duration), all))
}
