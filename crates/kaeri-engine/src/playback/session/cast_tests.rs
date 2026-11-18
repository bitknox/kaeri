//! A playback cast to the fake receiver on loopback: the session drives the TV as it drives a
//! window, the next episode plays on the TV already showing it, and a stream the TV can't play
//! says why in the cast's own words.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use kaeri_cast::{
    CastDevice, CastFailure, CastPlayer, CastSetup, DEFAULT_MEDIA_RECEIVER, PlainHttp, Timings,
    aac::FakeEncoder,
    fake::{
        CustomReceiver, FakeOptions, FakeReceiver,
        matroska::{Fixture, FixtureAudio, matroska},
    },
};
use kaeri_core::{
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind, Subtitle},
};
use kaeri_model::{
    Msg,
    command::{Command, PlayerCommand},
    event::Event,
    playback::Ending,
};
use kaeri_net::HttpClient;
use kaeri_remote::command::RemoteCommand;
use kaeri_store::Store;
use tokio::sync::mpsc;

use super::{Machine, play, scripted::settings};
use crate::{
    cast::{CastHub, SessionState, SessionView},
    host::Host,
    playback::{
        EpisodeEntry, NextEpisode, PlaybackContext,
        link::{PlayerTarget, cast::CastOpener},
        power::ReportedPower,
        slot::PlayerSlot,
    },
};

struct NoHost;

impl Host for NoHost {
    fn open_url(&self, _: &str) -> Result<(), String> {
        Ok(())
    }

    fn notify(&self, _: &str, _: &str) {}
}

fn timings() -> Timings {
    Timings {
        poll: Duration::from_millis(100),
        movement: Duration::from_millis(1500),
        fetch: Duration::from_secs(2),
        idle_close: Duration::from_secs(600),
        ..Timings::default()
    }
}

fn context(episode: &str, next: Option<&str>) -> PlaybackContext {
    PlaybackContext {
        anilist_id: AnilistId::new(900_002),
        mal_id: None,
        episode: episode.into(),
        title: "Stardust Wayfarer".into(),
        translation: Translation::Sub,
        resume_at: None,
        runtime_secs: None,
        speed: None,
        volume: None,
        next: next.map(|n| NextEpisode { number: n.into(), title: None, thumbnail: None }),
        episodes: vec![EpisodeEntry {
            number: "2".into(),
            title: Some("Low Tide".into()),
            watched: false,
            filler: false,
        }],
        cover_url: None,
        banner_url: Some("https://art.example/banner.jpg".into()),
        episode_total: None,
    }
}

/// A TV on loopback and what casts to it.
struct Rig {
    receiver: FakeReceiver,
    hub: Arc<CastHub>,
    opener: Arc<CastOpener>,
    slot: PlayerSlot,
    store: Store,
    dir: tempfile::TempDir,
}

impl Rig {
    async fn new(encoder: bool) -> Self {
        Self::with(encoder, FakeOptions { speed: 8.0, ..FakeOptions::default() }).await
    }

    /// A TV cast to through the Default Media Receiver, as `cast.receiver` can name it: the
    /// relay's conversions and the encoder are what it plays with.
    async fn standard(encoder: bool) -> Self {
        Self::on(encoder, FakeOptions { speed: 8.0, ..FakeOptions::default() }, true, timings())
            .await
    }

    async fn with(encoder: bool, options: FakeOptions) -> Self {
        Self::on(encoder, options, false, timings()).await
    }

    async fn on(encoder: bool, options: FakeOptions, standard: bool, timings: Timings) -> Self {
        let receiver = FakeReceiver::start(options).await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let hub =
            Arc::new(CastHub::with(Some(dir.path().to_owned()), Arc::new(PlainHttp), timings));
        if standard {
            hub.set_receiver(Some(DEFAULT_MEDIA_RECEIVER.into()));
        }
        if encoder {
            hub.set_encoder(Arc::new(FakeEncoder::default()));
        }
        let device = CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: receiver.address,
            video: true,
            group: false,
        };
        hub.begin(device.clone());
        let opener = Arc::new(CastOpener::new(Arc::clone(&hub), device, dir.path().to_owned()));
        Self {
            receiver,
            hub,
            opener,
            slot: PlayerSlot::default(),
            store: Store::open_in_memory().unwrap(),
            dir,
        }
    }

    fn device(&self) -> CastDevice {
        CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: self.receiver.address,
            video: true,
            group: false,
        }
    }

    /// An episode on disk, as a download is played: `name` in the rig's directory.
    fn file(&self, name: &str, fixture: &Fixture) -> Stream {
        let path = self.dir.path().join(name);
        std::fs::write(&path, matroska(fixture)).unwrap();
        Stream::new(path.to_string_lossy(), StreamKind::Mp4)
    }

    fn subtitles(&self) -> Subtitle {
        let path = self.dir.path().join("episode.en.ass");
        std::fs::write(
            &path,
            "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, \
             Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\n",
        )
        .unwrap();
        Subtitle {
            language: "en".into(),
            url: path.to_string_lossy().into_owned(),
            hard: false,
            format: None,
        }
    }

    async fn play(&self, streams: Vec<Stream>, context: PlaybackContext) -> Vec<Event> {
        self.run(streams, context, false).await
    }

    /// [`Self::play`], with the engine's events reaching the hub as they happen, as the
    /// engine's own loop hands them on: the card knows the episode it casts.
    async fn play_seen(&self, streams: Vec<Stream>, context: PlaybackContext) -> Vec<Event> {
        self.run(streams, context, true).await
    }

    async fn run(
        &self,
        streams: Vec<Stream>,
        context: PlaybackContext,
        seen: bool,
    ) -> Vec<Event> {
        let http = HttpClient::offline(&kaeri_core::config::NetworkConfig::default()).unwrap();
        let machine = Machine {
            opener: self.opener.clone(),
            slot: self.slot.clone(),
            host: Arc::new(NoHost),
            power: Arc::new(ReportedPower::default()),
            offline: Default::default(),
            sources: None,
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_controls, commands) = mpsc::unbounded_channel::<PlayerCommand>();
        let (seeing, mut rx) = if seen {
            let hub = Arc::clone(&self.hub);
            let seeing = tokio::spawn(async move {
                let mut events = Vec::new();
                while let Some(event) = rx.recv().await {
                    hub.observe(&event);
                    events.push(event);
                }
                events
            });
            (Some(seeing), mpsc::unbounded_channel().1)
        } else {
            (None, rx)
        };
        let mut settings = settings();
        settings.subtitle_language = Some("en".into());
        let playing = play(
            streams,
            context,
            self.store.clone(),
            http,
            machine,
            settings,
            tx.into(),
            commands,
        );
        tokio::time::timeout(Duration::from_secs(30), playing)
            .await
            .expect("the playback ends");
        if let Some(seeing) = seeing {
            return seeing.await.unwrap();
        }
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }
}

fn ending(events: &[Event]) -> Option<Ending> {
    events.iter().rev().find_map(|event| match event {
        Event::PlaybackEnded { ending, .. } => Some(ending.clone()),
        _ => None,
    })
}

fn toasts(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Toast(notice) => Some(notice.text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_episode_casts_with_its_subtitles_and_the_next_plays_on_the_same_tv() {
    let rig = Rig::new(true).await;
    let first =
        Stream { subtitles: vec![rig.subtitles()], ..rig.file("1.mkv", &Fixture::default()) };

    let events = rig.play(vec![first], context("1", Some("2"))).await;
    assert_eq!(
        ending(&events),
        Some(Ending::Complete { next: Some("2".into()) }),
        "{events:?}"
    );
    let loaded = rig.receiver.heard("LOAD");
    let media = &loaded[0]["media"];
    assert_eq!(media["metadata"]["seriesTitle"], "Stardust Wayfarer");
    assert_eq!(media["metadata"]["title"], "Episode 1");
    assert_eq!(media["metadata"]["images"][0]["url"], "https://art.example/banner.jpg");
    assert_eq!(media["tracks"][0]["trackContentType"], "text/vtt");
    assert_eq!(
        loaded[0]["activeTrackIds"],
        serde_json::json!([1]),
        "subtitles on for a sub watcher"
    );
    assert!(rig.hub.view().status.and_then(|s| s.plan).is_some(), "the card knows the plan");

    // The TV waits for the next episode, and auto-next plays it there.
    let second = rig.file("2.mkv", &Fixture::default());
    let events = rig.play(vec![second], context("2", None)).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }), "{events:?}");
    assert_eq!(rig.receiver.heard("LAUNCH").len(), 1, "the receiver kept running");
    let loaded = rig.receiver.heard("LOAD");
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[1]["media"]["metadata"]["title"], "Episode 2 · Low Tide");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_kept_for_the_next_episode_is_let_go_by_a_play_on_this_device() {
    let rig = Rig::new(true).await;
    let first = rig.file("1.mkv", &Fixture::default());
    let events = rig.play(vec![first], context("1", Some("2"))).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: Some("2".into()) }));

    assert!(rig.slot.take(&PlayerTarget::Local).is_none(), "a window here is not the TV");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !rig.receiver.heard("STOP").iter().any(|m| m["sessionId"].is_string()) {
        assert!(tokio::time::Instant::now() < deadline, "the TV went back to what it showed");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(rig.slot.take(&PlayerTarget::Cast("tv-1".into())).is_none(), "nothing kept");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn audio_the_tv_cant_play_says_so_and_never_takes_the_tv_over() {
    let rig = Rig::standard(false).await;
    let stream = rig.file("1.mkv", &Fixture::default());
    let events = rig.play(vec![stream], context("1", None)).await;
    assert_eq!(ending(&events), Some(Ending::Failed));
    let said = Msg::NoticeCastAudioCodec { name: "Living room".into(), codec: "AC-3".into() }
        .english();
    assert!(toasts(&events).contains(&said), "{:?}", toasts(&events));
    assert!(rig.receiver.heard("LAUNCH").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kaeris_receiver_plays_the_audio_the_standard_one_cant() {
    let rig = Rig::new(false).await;
    let stream = rig.file("1.mkv", &Fixture::default());
    let events = rig.play(vec![stream], context("1", None)).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }), "{events:?}");
    let said = Msg::NoticeCastAudioCodec { name: "Living room".into(), codec: "AC-3".into() }
        .english();
    assert!(!toasts(&events).contains(&said), "{:?}", toasts(&events));
    // Handed over as it is: no AAC on the way.
    let loaded = rig.receiver.heard("LOAD");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0]["media"]["customData"]["kaeri"]["audio"]["codec"], "ac3");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stream_the_tv_cant_play_fails_over_to_one_it_can() {
    let rig = Rig::standard(false).await;
    let ac3 = rig.file("1-ac3.mkv", &Fixture::default());
    let aac = rig
        .file("1-aac.mkv", &Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let events = rig.play(vec![ac3, aac], context("1", None)).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }), "{events:?}");
    assert_eq!(rig.receiver.heard("LOAD").len(), 1, "only the stream it can play was loaded");
}

/// The hub's session once `want` says it is there.
async fn session_until(hub: &CastHub, want: impl Fn(&SessionView) -> bool) -> SessionView {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let view = hub.session();
        if want(&view) {
            return view;
        }
        assert!(tokio::time::Instant::now() < deadline, "the session as it stood: {view:?}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn receiver_stops(receiver: &FakeReceiver) -> usize {
    receiver.heard("STOP").iter().filter(|m| m["sessionId"].is_string()).count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_held_tv_plays_one_play_after_another_and_stays_until_let_go() {
    let rig = Rig::new(true).await;
    rig.hub.connect(rig.device());
    assert_eq!(rig.hub.session().state, SessionState::Connecting);
    let view = session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    assert_eq!(view.device.map(|d| d.name).as_deref(), Some("Living room"));
    assert!(view.own_receiver, "Kaeri's receiver runs where it is published");
    assert_eq!(rig.receiver.heard("LAUNCH").len(), 1, "connecting launches the receiver");
    assert!(rig.receiver.heard("LOAD").is_empty());

    let first = rig.file("1.mkv", &Fixture::default());
    let events = rig.play(vec![first], context("1", None)).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }), "{events:?}");
    // The playback is over and the TV stays held, its receiver up.
    let view = session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    assert!(view.item.is_none(), "{view:?}");
    assert_eq!(receiver_stops(&rig.receiver), 0);

    let second = rig.file("2.mkv", &Fixture::default());
    let events = rig.play(vec![second], context("2", None)).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }), "{events:?}");
    assert_eq!(rig.receiver.heard("LAUNCH").len(), 1, "one receiver for the whole session");
    assert_eq!(rig.receiver.heard("LOAD").len(), 2);
    assert_eq!(rig.hub.noted_on().as_deref(), Some("tv-1"), "noted for the next launch");

    assert!(rig.hub.disconnect());
    assert_eq!(rig.hub.session().state, SessionState::Idle);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while receiver_stops(&rig.receiver) == 0 {
        assert!(tokio::time::Instant::now() < deadline, "the receiver closed");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(rig.hub.left_on(), None, "nothing to join after a disconnect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_taken_over_ends_the_session_and_says_so() {
    let rig = Rig::new(true).await;
    rig.hub.connect(rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    rig.receiver.take_over();
    let view = session_until(&rig.hub, |v| v.state == SessionState::Error).await;
    let said = crate::playback::link::cast::notice(&CastFailure::TakenOver, "Living room");
    assert_eq!(view.notice, Some(said));
    assert!(rig.hub.session_device().is_none(), "plays open here again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_session_left_running_is_joined_again_once_its_tv_is_found() {
    // The fake TV serves one sender at a time: the first run's connection, closed as the next
    // one comes, is taken for lost rather than made again, as an ended app's would be.
    let options = FakeOptions { remembers: true, ..FakeOptions::default() };
    let rig = Rig::on(true, options, false, lost_at_once()).await;
    rig.hub.connect(rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Connected).await;

    // The app comes back with the receiver still up: a new hub over the same files.
    let later = Arc::new(CastHub::with(
        Some(rig.dir.path().to_owned()),
        Arc::new(PlainHttp),
        timings(),
    ));
    assert_eq!(later.left_on().as_deref(), Some("tv-1"));
    let mut elsewhere = rig.device();
    elsewhere.id = "tv-2".into();
    later.rejoin(&elsewhere);
    assert_eq!(later.session().state, SessionState::Idle, "only the TV it was left on");
    later.rejoin(&rig.device());
    let view = session_until(&later, |v| v.state == SessionState::Connected).await;
    assert!(view.own_receiver);
    assert_eq!(rig.receiver.heard("LAUNCH").len(), 1, "joined, not launched again");

    assert!(later.disconnect());
    let again = Arc::new(CastHub::with(
        Some(rig.dir.path().to_owned()),
        Arc::new(PlainHttp),
        timings(),
    ));
    assert_eq!(again.left_on(), None);
}

/// The record of the cast left running, as the engine keeps it on disk.
fn record(rig: &Rig) -> Option<serde_json::Value> {
    let bytes = std::fs::read(rig.dir.path().join("cast-session.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_record(rig: &Rig, record: &serde_json::Value) {
    std::fs::write(rig.dir.path().join("cast-session.json"), record.to_string()).unwrap();
}

/// Wait for the record on disk to read as `want` says.
async fn record_until(
    rig: &Rig,
    want: impl Fn(Option<&serde_json::Value>) -> bool,
) -> Option<serde_json::Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let now = record(rig);
        if want(now.as_ref()) {
            return now;
        }
        assert!(tokio::time::Instant::now() < deadline, "the record as it stood: {now:?}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The record names episode 1 of the rig's title.
fn records_episode_one(record: Option<&serde_json::Value>) -> bool {
    record.is_some_and(|r| r["card"] == serde_json::json!([900_002, "1"]))
}

/// A new run of the app over the rig's files: what a launch after the app was ended has.
fn relaunched(rig: &Rig) -> Arc<CastHub> {
    Arc::new(CastHub::with(
        Some(rig.dir.path().to_owned()),
        Arc::new(PlainHttp),
        lost_at_once(),
    ))
}

/// Every state `hub`'s session reads as from now on.
fn states_of(hub: &Arc<CastHub>) -> Arc<std::sync::Mutex<Vec<SessionState>>> {
    let states = Arc::new(std::sync::Mutex::new(vec![hub.session().state]));
    let (hub, seen) = (Arc::downgrade(hub), Arc::clone(&states));
    let mut changes = hub.upgrade().unwrap().changes();
    tokio::spawn(async move {
        while changes.changed().await.is_ok() {
            let Some(hub) = hub.upgrade() else { return };
            seen.lock().unwrap().push(hub.session().state);
        }
    });
    states
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .try_into()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cast_left_running_is_joined_quietly_at_the_next_launch() {
    let (rig, playing) = playing_on_a_tv_that_plays_on().await;
    // The record follows the cast: the TV at its address, its receiver session and the episode.
    let noted = record_until(&rig, records_episode_one).await.unwrap();
    assert_eq!(noted["tv"]["address"], rig.receiver.address.to_string());
    assert!(noted["until"].as_i64().unwrap() > unix_now() + 600, "{noted}");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // The app is ended mid-episode and opened again, with no picker and no browse.
    let later = relaunched(&rig);
    let states = states_of(&later);
    assert!(later.resume_left(), "a record that counts is looked for");
    assert_eq!(later.session().state, SessionState::Idle, "nothing shows while it looks");
    let view =
        session_until(&later, |v| v.state == SessionState::Playing && v.item.is_some()).await;
    assert_eq!(view.device.map(|d| d.name).as_deref(), Some("Living room"));
    let item = view.item.unwrap();
    assert_eq!(item.id, Some(AnilistId::new(900_002)));
    assert_eq!(item.episode.as_deref(), Some("1"));
    assert!(item.position > 0.0, "at the TV's position: {item:?}");
    assert!(
        !states.lock().unwrap().contains(&SessionState::Connecting),
        "never read as connecting: {:?}",
        states.lock().unwrap()
    );
    assert_eq!(rig.receiver.heard("LAUNCH").len(), 1, "joined, not launched again");
    assert_eq!(rig.receiver.heard("LOAD").len(), 1, "the episode never started over");
    playing.abort();

    // Stopping the cast drops the record.
    assert!(later.disconnect());
    record_until(&rig, |r| r.is_none()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_playing_something_else_at_launch_drops_the_record_and_is_left_alone() {
    // Another Kaeri device's episode on the same receiver session, then another app.
    for other_app in [false, true] {
        let (rig, playing) = playing_on_a_tv_that_plays_on().await;
        let noted = record_until(&rig, records_episode_one).await.unwrap();
        if other_app {
            rig.receiver.take_over();
        } else {
            rig.receiver.load_elsewhere_with(serde_json::json!({
                "kaeriCard": {"id": 900_002, "episode": "2", "title": "Stardust Wayfarer"},
            }));
        }
        session_until(&rig.hub, |v| v.state == SessionState::Error).await;
        let _ = playing.await;
        // The app was ended before it heard: its record still names episode 1.
        write_record(&rig, &noted);
        let stops = rig.receiver.heard("STOP").len();

        let later = relaunched(&rig);
        assert!(later.resume_left());
        record_until(&rig, |r| r.is_none()).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let view = later.session();
        assert_eq!(view.state, SessionState::Idle, "other app {other_app}: {view:?}");
        assert_eq!(view.notice, None);
        assert_eq!(later.left_on(), None);
        assert_eq!(rig.receiver.heard("STOP").len(), stops, "what plays there is left alone");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stale_record_is_dropped_at_launch_without_reaching_the_tv() {
    let (rig, playing) = playing_on_a_tv_that_plays_on().await;
    let mut noted = record_until(&rig, records_episode_one).await.unwrap();
    playing.abort();
    let connects = rig.receiver.heard("CONNECT").len();

    // With TVs hidden in Settings, nothing is tried and the record goes.
    write_record(&rig, &noted);
    let hidden = relaunched(&rig);
    hidden.set_show_tvs(false);
    assert!(!hidden.resume_left());
    assert_eq!(record(&rig), None);

    noted["until"] = serde_json::json!(unix_now() - 60);
    write_record(&rig, &noted);
    let later = relaunched(&rig);
    assert!(!later.resume_left(), "a stale record is not looked for");
    assert_eq!(record(&rig), None, "and it goes");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(rig.receiver.heard("CONNECT").len(), connects, "the TV was never reached");
    assert_eq!(later.session().state, SessionState::Idle);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_launch_without_a_record_looks_for_no_tv() {
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let hub =
        Arc::new(CastHub::with(Some(dir.path().to_owned()), Arc::new(PlainHttp), timings()));
    assert!(!hub.resume_left(), "nothing to look for");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(receiver.heard.lock().unwrap().is_empty(), "the TV was never reached");
    assert_eq!(hub.session().state, SessionState::Idle);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_out_of_reach_at_launch_drops_the_record_quietly() {
    // A record naming a TV whose address answers nothing any more.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let record = serde_json::json!({
        "device": "tv-1",
        "session": "session-1",
        "tv": {
            "instance": "Chromecast-test", "id": "tv-1", "name": "Living room",
            "model": "Chromecast", "address": closed.to_string(), "video": true, "group": false,
        },
        "card": [900_002, "1"],
        "until": unix_now() + 600,
    });
    let path = dir.path().join("cast-session.json");
    std::fs::write(&path, record.to_string()).unwrap();
    let hub =
        Arc::new(CastHub::with(Some(dir.path().to_owned()), Arc::new(PlainHttp), timings()));
    assert!(hub.resume_within(Duration::from_secs(1)));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while path.exists() {
        assert!(tokio::time::Instant::now() < deadline, "the record was dropped");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let view = hub.session();
    assert_eq!(view.state, SessionState::Idle);
    assert_eq!(view.notice, None);
}

/// Wait for `want` of the TV a session was left on, as the hub's note says.
async fn left_until(hub: &CastHub, want: Option<&str>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while hub.noted_on().as_deref() != want {
        assert!(tokio::time::Instant::now() < deadline, "left on {:?}", hub.noted_on());
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cast_begun_from_play_on_tv_is_joined_again_after_the_system_ends_the_app() {
    let options = FakeOptions { remembers: true, ..FakeOptions::default() };
    let rig = Rig::on(true, options, false, lost_at_once()).await;
    // Play on TV opens its own player, as a playback link does, with no session behind it.
    let (player, _events) = CastPlayer::connect(CastSetup {
        device: rig.device(),
        memory: rig.hub.memory(),
        upstream: rig.hub.upstream(),
        encoder: rig.hub.encoder(),
        timings: rig.hub.timings(),
    })
    .await
    .unwrap();
    let player = Arc::new(player);
    rig.hub.attach(&player);
    assert_eq!(rig.hub.left_on(), None, "nothing runs on the TV yet");
    player.launch().await.unwrap();
    left_until(&rig.hub, Some("tv-1")).await;

    // The system ends the app with the receiver up: a new run over the same files joins it.
    let later = Arc::new(CastHub::with(
        Some(rig.dir.path().to_owned()),
        Arc::new(PlainHttp),
        timings(),
    ));
    assert_eq!(later.left_on().as_deref(), Some("tv-1"));
    later.rejoin(&rig.device());
    session_until(&later, |v| v.state == SessionState::Connected).await;
    assert_eq!(rig.receiver.heard("LAUNCH").len(), 1, "joined, not launched again");
    assert!(later.disconnect());

    // A cast the viewer stops leaves nothing to join.
    let again =
        Rig::with(true, FakeOptions { remembers: true, ..FakeOptions::default() }).await;
    let (player, _events) = CastPlayer::connect(CastSetup {
        device: again.device(),
        memory: again.hub.memory(),
        upstream: again.hub.upstream(),
        encoder: again.hub.encoder(),
        timings: again.hub.timings(),
    })
    .await
    .unwrap();
    let player = Arc::new(player);
    again.hub.attach(&player);
    player.launch().await.unwrap();
    left_until(&again.hub, Some("tv-1")).await;
    player.quit().await.unwrap();
    left_until(&again.hub, None).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn finding_the_tv_again_while_a_play_on_tv_cast_plays_keeps_its_card() {
    let options = FakeOptions { remembers: true, ..FakeOptions::default() };
    let rig = Rig::on(true, options, false, lost_at_once()).await;
    let (player, _events) = CastPlayer::connect(CastSetup {
        device: rig.device(),
        memory: rig.hub.memory(),
        upstream: rig.hub.upstream(),
        encoder: rig.hub.encoder(),
        timings: rig.hub.timings(),
    })
    .await
    .unwrap();
    let player = Arc::new(player);
    rig.hub.attach(&player);
    player.launch().await.unwrap();
    left_until(&rig.hub, Some("tv-1")).await;

    // The header's TV picker browses again and finds the TV the cast plays on.
    rig.hub.rejoin(&rig.device());
    assert!(rig.hub.attached(&player), "the card still follows the cast");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(rig.hub.attached(&player), "the card still follows the cast");
    assert_eq!(rig.hub.session().state, SessionState::Idle, "no session joined beside it");
    assert_eq!(rig.receiver.heard("LAUNCH").len(), 1);
    assert!(player.is_alive());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_stopped_on_the_tv_and_picked_again_is_never_joined_beside_the_new_cast() {
    let options = FakeOptions { duration: 600.0, ..FakeOptions::default() };
    let rig = Arc::new(Rig::with(true, options).await);
    rig.hub.connect(rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    left_until(&rig.hub, Some("tv-1")).await;
    // The TV stops the cast: nothing is left to join.
    rig.receiver.stop_app();
    session_until(&rig.hub, |v| v.state != SessionState::Connected).await;
    left_until(&rig.hub, None).await;

    // Choose another TV: the play goes to the TV on its own, and the picker's browse finds the
    // TV while it starts.
    rig.hub.begin(rig.device());
    let file = rig.file("1.mkv", &Fixture::default());
    let playing = {
        let rig = Arc::clone(&rig);
        tokio::spawn(async move { rig.play(vec![file], context("1", None)).await })
    };
    left_until(&rig.hub, Some("tv-1")).await;
    let player = rig.hub.player().expect("the play's player");
    let launches = rig.receiver.heard("LAUNCH").len();
    rig.hub.rejoin(&rig.device());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(rig.hub.attached(&player), "the card still follows the play");
    assert_eq!(rig.hub.session().state, SessionState::Idle, "no session joined beside it");
    assert_eq!(rig.receiver.heard("LAUNCH").len(), launches);
    playing.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_picked_again_after_its_cast_stopped_is_not_left_running_while_it_plays() {
    let options = FakeOptions { duration: 600.0, ..FakeOptions::default() };
    let rig = Arc::new(Rig::with(true, options).await);
    rig.hub.connect(rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    rig.receiver.stop_app();
    session_until(&rig.hub, |v| v.state != SessionState::Connected).await;
    left_until(&rig.hub, None).await;

    // Choose another TV: the play goes to the TV on its own, with no session behind it.
    rig.hub.begin(rig.device());
    let file = rig.file("1.mkv", &Fixture::default());
    let playing = {
        let rig = Arc::clone(&rig);
        tokio::spawn(async move { rig.play(vec![file], context("1", None)).await })
    };
    left_until(&rig.hub, Some("tv-1")).await;
    let player = rig.hub.player().expect("the play's player");
    assert!(player.is_alive());
    // The apps read a TV left running with no session as one joined again: Reconnecting.
    assert_eq!(rig.hub.session().state, SessionState::Idle);
    assert_eq!(rig.hub.left_on(), None, "the TV plays the new cast, nothing to join");

    // The play ends: its note is let go, and nothing is left to join.
    player.quit().await.unwrap();
    left_until(&rig.hub, None).await;
    assert_eq!(rig.hub.left_on(), None);
    playing.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_found_while_a_cast_connects_to_it_is_left_to_that_cast() {
    let options = FakeOptions { remembers: true, ..FakeOptions::default() };
    let rig = Rig::on(true, options, false, lost_at_once()).await;
    rig.hub.connect(rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Connected).await;

    // A later run with the receiver still up casts to that TV, and finds it while connecting.
    let later = Arc::new(CastHub::with(
        Some(rig.dir.path().to_owned()),
        Arc::new(PlainHttp),
        timings(),
    ));
    assert_eq!(later.left_on().as_deref(), Some("tv-1"));
    later.begin(rig.device());
    let opening = later.opening();
    later.rejoin(&rig.device());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(later.session().state, SessionState::Idle, "nothing joined while it connects");
    drop(opening);
    later.rejoin(&rig.device());
    session_until(&later, |v| v.state == SessionState::Connected).await;
    assert!(later.disconnect());
}

/// A TV whose silence is noticed after a second, and connected to again for six.
fn quick_to_notice() -> Timings {
    Timings { silence: Duration::from_secs(1), reconnect: Duration::from_secs(6), ..timings() }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_held_tv_whose_connection_drops_is_connected_to_again_and_stays_held() {
    let options = FakeOptions { remembers: true, ..FakeOptions::default() };
    let rig = Rig::on(true, options, false, quick_to_notice()).await;
    rig.hub.connect(rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    let connects = rig.receiver.heard("CONNECT").len();

    // The phone is locked and its Wi-Fi naps: the connection dies, the receiver stays up.
    rig.receiver.drop_connection();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    while rig.receiver.heard("CONNECT").len() <= connects + 1 {
        assert!(tokio::time::Instant::now() < deadline, "connected again in time");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let view = rig.hub.session();
    assert_eq!(view.state, SessionState::Connected, "{view:?}");
    assert_eq!(view.notice, None);
    assert_eq!(rig.receiver.heard("LAUNCH").len(), 1, "joined, not launched again");
    assert!(rig.hub.disconnect());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_idle_held_tv_that_closed_the_receiver_meanwhile_is_let_go_quietly() {
    let rig = Rig::on(true, FakeOptions::default(), false, quick_to_notice()).await;
    rig.hub.connect(rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    // A TV that forgets between connections: the receiver is gone on the next one.
    rig.receiver.drop_connection();
    let view = session_until(&rig.hub, |v| v.state == SessionState::Idle).await;
    assert_eq!(view.notice, None);
    assert_eq!(rig.hub.left_on(), None, "nothing left to join");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_held_tv_lost_mid_episode_says_so_and_is_looked_for_once_found_again() {
    let options = FakeOptions { duration: 600.0, ..FakeOptions::default() };
    let rig = Arc::new(Rig::on(true, options, false, quick_to_notice()).await);
    rig.hub.connect(rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    let file = rig.file("1.mkv", &Fixture::default());
    let playing = {
        let rig = Arc::clone(&rig);
        tokio::spawn(async move { rig.play(vec![file], context("1", None)).await })
    };
    session_until(&rig.hub, |v| {
        v.state == SessionState::Playing && v.item.as_ref().is_some_and(|i| !i.loading)
    })
    .await;

    // The connection drops and the TV that answers next has closed the receiver.
    rig.receiver.drop_connection();
    let view = session_until(&rig.hub, |v| v.state == SessionState::Error).await;
    assert_eq!(view.notice, Some(Msg::NoticeCastLost { name: "Living room".into() }));
    assert!(rig.hub.session_device().is_none(), "plays open here again");
    let events = playing.await.unwrap();
    assert_ne!(ending(&events), Some(Ending::Complete { next: None }), "{events:?}");
    assert_eq!(rig.hub.left_on().as_deref(), Some("tv-1"), "it may still play there");

    // Found again: the session ended lost is looked for, and let go as its receiver is gone.
    rig.hub.rejoin(&rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Idle).await;
    assert_eq!(rig.hub.left_on(), None);
}

/// A TV whose dropped connection is taken for lost at once, with no tries to connect again: the
/// receiver plays on while this device has let it go.
fn lost_at_once() -> Timings {
    Timings { silence: Duration::from_secs(1), reconnect: Duration::ZERO, ..timings() }
}

/// A rig on a TV that keeps its receiver playing across connections, with episode 1 playing on
/// it, and the playback that drives it.
async fn playing_on_a_tv_that_plays_on() -> (Arc<Rig>, tokio::task::JoinHandle<Vec<Event>>) {
    playing_on(FakeOptions { remembers: true, duration: 600.0, ..FakeOptions::default() }).await
}

/// [`playing_on_a_tv_that_plays_on`], on a TV set up with `options`.
async fn playing_on(options: FakeOptions) -> (Arc<Rig>, tokio::task::JoinHandle<Vec<Event>>) {
    playing_with(options, lost_at_once()).await
}

/// [`playing_on`], with the hub's `timings`.
async fn playing_with(
    options: FakeOptions,
    timings: Timings,
) -> (Arc<Rig>, tokio::task::JoinHandle<Vec<Event>>) {
    let rig = Arc::new(Rig::on(true, options, false, timings).await);
    rig.hub.found(rig.device());
    rig.hub.connect(rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    let file = rig.file("1.mkv", &Fixture::default());
    // The engine stages the cast before its playback starts, as a play sent to a TV does.
    rig.hub.observe(&Event::CastStaged {
        id: AnilistId::new(900_002),
        title: "Stardust Wayfarer".into(),
        episode: "1".into(),
        episode_title: None,
        speed: 1.0,
    });
    let playing = {
        let rig = Arc::clone(&rig);
        tokio::spawn(async move { rig.play_seen(vec![file], context("1", None)).await })
    };
    session_until(&rig.hub, |v| {
        v.state == SessionState::Playing && v.item.as_ref().is_some_and(|i| !i.loading)
    })
    .await;
    (rig, playing)
}

/// A cast keeps the episode's place in the watch history as it plays, as a window does, so an
/// app the system ends mid-cast still resumes near where the TV was.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cast_saves_its_place_while_it_plays() {
    // The TV plays the file for ten minutes, ten seconds a second.
    let options = FakeOptions { duration: 600.0, speed: 10.0, ..FakeOptions::default() };
    let rig = Arc::new(Rig::on(true, options, false, timings()).await);
    let file = rig.file("1.mkv", &Fixture::default());
    let playing = {
        let rig = Arc::clone(&rig);
        tokio::spawn(async move { rig.play_seen(vec![file], context("1", None)).await })
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let saved = loop {
        let saved = rig.store.resume_position(AnilistId::new(900_002), "1").unwrap();
        if saved.is_some_and(|at| at >= 30.0) {
            break saved;
        }
        assert!(tokio::time::Instant::now() < deadline, "a place saved mid-cast: {saved:?}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    assert!(!playing.is_finished(), "still casting when {saved:?} was saved");
    playing.abort();
}

/// A TvSender on the rig's hub, and the commands it sends the engine.
fn sender_on(rig: &Rig) -> (crate::remote::sender::TvSender, impl FnMut() -> Vec<Command>) {
    sender_for(&rig.hub)
}

/// A TvSender on `hub`, and the commands it sends the engine.
fn sender_for(
    hub: &Arc<CastHub>,
) -> (crate::remote::sender::TvSender, impl FnMut() -> Vec<Command> + use<>) {
    let sender = crate::remote::sender::TvSender::new(
        kaeri_remote::identity::Identity::generate().unwrap(),
        kaeri_remote::client::Hello { name: "Phone".into(), app: "kaeri-test/1".into() },
        kaeri_remote::known::KnownTvs::memory(),
    );
    let (engine, sent) = crate::engine::RemoteSender::recording();
    sender.attach_casting(Arc::clone(hub), engine);
    (sender, sent)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_picked_again_after_a_lost_connection_joins_the_receiver_still_playing() {
    let (rig, playing) = playing_on_a_tv_that_plays_on().await;
    rig.receiver.drop_connection();
    let view = session_until(&rig.hub, |v| v.state == SessionState::Error).await;
    assert_eq!(view.notice, Some(Msg::NoticeCastLost { name: "Living room".into() }));
    playing.await.unwrap();
    let launches = rig.receiver.heard("LAUNCH").len();
    let loads = rig.receiver.heard("LOAD").len();

    // Picked again from the cast button: the receiver still plays the episode, and is joined.
    rig.hub.connect(rig.device());
    let view = session_until(&rig.hub, |v| v.state == SessionState::Playing).await;
    assert!(view.item.is_some(), "{view:?}");
    assert!(view.own_receiver);
    assert_eq!(rig.receiver.heard("LAUNCH").len(), launches, "joined, never launched again");
    assert_eq!(rig.receiver.heard("LOAD").len(), loads, "nothing loaded again");
    assert!(rig.hub.disconnect());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn play_here_after_a_lost_connection_plays_the_episode_on_here() {
    let (rig, playing) = playing_on_a_tv_that_plays_on().await;
    let (sender, mut sent) = sender_on(&rig);
    tokio::time::sleep(Duration::from_millis(500)).await;
    rig.receiver.drop_connection();
    session_until(&rig.hub, |v| v.state == SessionState::Error).await;
    playing.await.unwrap();
    sent();

    let from = sender.cast_play_here().expect("the episode comes back to play here");
    assert_eq!(from.episode, "1");
    assert!(from.position >= 0.0);
    assert!(sent().iter().any(|c| matches!(c, Command::StopCasting)), "the TV is let go");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn back_after_the_session_ended_away_the_receiver_is_joined_and_plays_on() {
    let (rig, playing) = playing_on_a_tv_that_plays_on().await;
    let (sender, mut sent) = sender_on(&rig);
    // The app leaves the screen: the TV pauses for it.
    sender.cast_away(true);
    session_until(&rig.hub, |v| v.item.as_ref().is_some_and(|i| i.paused)).await;
    // Away long enough that the connection dies and the session ends.
    rig.receiver.drop_connection();
    session_until(&rig.hub, |v| v.state == SessionState::Error).await;
    playing.await.unwrap();
    sent();
    let launches = rig.receiver.heard("LAUNCH").len();
    let loads = rig.receiver.heard("LOAD").len();

    // Back: the receiver still holds the episode where it paused, and plays on.
    sender.cast_away(false);
    let view = session_until(&rig.hub, |v| {
        v.state == SessionState::Playing && v.item.as_ref().is_some_and(|i| !i.paused)
    })
    .await;
    assert!(view.item.is_some());
    assert_eq!(rig.receiver.heard("LAUNCH").len(), launches, "joined, never launched again");
    assert_eq!(rig.receiver.heard("LOAD").len(), loads, "never loaded again");
    assert!(
        !sent().iter().any(|c| matches!(c, Command::CastTo { .. })),
        "the episode isn't cast again"
    );
    assert!(rig.hub.disconnect());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn each_fall_back_to_the_tvs_own_player_is_said_and_the_next_connect_tries_kaeris_first()
{
    let options = FakeOptions { custom: CustomReceiver::Silent, ..FakeOptions::default() };
    let rig = Rig::on(
        true,
        options,
        false,
        Timings { ready: Duration::from_millis(800), ..timings() },
    )
    .await;
    let fallback = Msg::NoticeCastFallback { name: "Living room".into() };
    for connect in 1..=2 {
        rig.hub.connect(rig.device());
        let view = session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
        assert!(!view.own_receiver, "connect {connect}: {view:?}");
        assert_eq!(view.note.as_ref(), Some(&fallback), "connect {connect} says it");
        assert!(rig.hub.disconnect());
        assert_eq!(rig.hub.session().note, None);
    }
    let launched: Vec<_> = rig
        .receiver
        .heard("LAUNCH")
        .iter()
        .map(|m| m["appId"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(launched.len(), 4, "{launched:?}");
    assert_ne!(launched[2], DEFAULT_MEDIA_RECEIVER, "the second connect tries Kaeri's first");
    assert_eq!(launched[3], DEFAULT_MEDIA_RECEIVER);
}

/// How many times the cast's note came to read `note`, from nothing or another note, as an
/// app's watch reads the hub: each is one notice. A watch can miss a moment between two
/// changes, so this never counts more than were said, and may count fewer.
fn notes_said(
    hub: &Arc<CastHub>,
    note: Msg,
) -> (Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let said = Arc::new(AtomicUsize::new(0));
    let mut changes = hub.changes();
    let (hub, count) = (Arc::clone(hub), Arc::clone(&said));
    let watch = tokio::spawn(async move {
        let mut was = None;
        loop {
            let now = hub.view().note;
            if now.as_ref() == Some(&note) && was.as_ref() != Some(&note) {
                count.fetch_add(1, Ordering::SeqCst);
            }
            was = now;
            if changes.changed().await.is_err() {
                return;
            }
        }
    });
    (said, watch)
}

/// Play on TV to a TV that can't run Kaeri's receiver, as the apps start it: the TV is
/// connected to, and the episode plays there once it is up. Every cast that falls back says so
/// once, the first of an app run and each one after it, and the episode's start keeps the note.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn each_play_on_tv_that_falls_back_says_so_once() {
    let options =
        FakeOptions { speed: 8.0, custom: CustomReceiver::Unknown, ..FakeOptions::default() };
    let rig = Rig::with(true, options).await;
    let fallback = Msg::NoticeCastFallback { name: "Living room".into() };
    let (said, watch) = notes_said(&rig.hub, fallback.clone());
    for cast in 1..=3 {
        rig.hub.connect(rig.device());
        if cast < 3 {
            // Play on TV: the play's cast begins as the TV connects.
            rig.hub.begin(rig.device());
            rig.hub.asked(AnilistId::new(900_002), "1".into());
        }
        let file = rig.file(&format!("{cast}.mkv"), &Fixture::default());
        let events = rig.play(vec![file], context("1", None)).await;
        assert_eq!(ending(&events), Some(Ending::Complete { next: None }), "cast {cast}");
        assert_eq!(rig.hub.view().note.as_ref(), Some(&fallback), "cast {cast} keeps it");
        assert!(said.load(Ordering::SeqCst) <= cast, "cast {cast} says it once");
        assert!(rig.hub.disconnect());
    }

    // A TV connected to with nothing playing says it as it connects, and the play that
    // follows on it doesn't say it again.
    rig.hub.connect(rig.device());
    let view = session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    assert_eq!(view.note.as_ref(), Some(&fallback));
    let file = rig.file("4.mkv", &Fixture::default());
    let before = said.load(Ordering::SeqCst);
    let events = rig.play(vec![file], context("1", None)).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }));
    assert_eq!(rig.hub.view().note.as_ref(), Some(&fallback), "kept, not said again");
    assert_eq!(said.load(Ordering::SeqCst), before, "said once for the held TV");
    assert!(rig.hub.disconnect());
    watch.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_held_tv_reads_as_reconnecting_while_its_connection_is_made_again() {
    let options = FakeOptions { remembers: true, ..FakeOptions::default() };
    let timings = Timings {
        silence: Duration::from_secs(1),
        reconnect: Duration::from_secs(6),
        ..timings()
    };
    let rig = Rig::on(true, options, false, timings).await;
    rig.hub.connect(rig.device());
    let view = session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    assert!(!view.reconnecting);
    // Every change is read, as an app's watch reads them, and the TV takes a moment to answer
    // again, so the time between is there to be seen.
    let mut changes = rig.hub.changes();
    rig.receiver.answer_after(Duration::from_millis(500));
    rig.receiver.drop_connection();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let view = loop {
        let view = rig.hub.session();
        if view.reconnecting {
            break view;
        }
        tokio::time::timeout_at(deadline, changes.changed())
            .await
            .unwrap_or_else(|_| panic!("reconnecting in time: {view:?}"))
            .unwrap();
    };
    assert_eq!(view.state, SessionState::Connected, "held as it was meanwhile");
    let view = session_until(&rig.hub, |v| !v.reconnecting).await;
    assert_eq!(view.state, SessionState::Connected, "joined again: {view:?}");
    assert!(rig.hub.disconnect());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_held_tv_stopped_on_the_tv_with_nothing_playing_lets_go_quietly() {
    let rig = Rig::new(true).await;
    rig.hub.connect(rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Connected).await;
    rig.receiver.stop_app();
    let view = session_until(&rig.hub, |v| v.state != SessionState::Connected).await;
    assert_eq!(view.state, SessionState::Idle, "nothing to play here: {view:?}");
    assert_eq!(view.notice, None);
}

/// A phone joins a cast another device started: the TV plays what that device cast, and this
/// phone's controls drive it on the TV.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cast_another_device_started_takes_the_controls_of_a_phone_that_joins_it() {
    let (rig, playing) = playing_on_a_tv_that_plays_on().await;
    // Another phone, which never cast there: nothing of its own was left on the TV.
    let dir = tempfile::tempdir().unwrap();
    let phone =
        Arc::new(CastHub::with(Some(dir.path().to_owned()), Arc::new(PlainHttp), timings()));
    let (sender, _sent) = sender_for(&phone);
    let launches = rig.receiver.heard("LAUNCH").len();
    phone.connect(rig.device());
    let view = session_until(&phone, |v| {
        v.state == SessionState::Playing && v.item.as_ref().is_some_and(|i| !i.paused)
    })
    .await;
    // The other device's cast says what it plays, so its card opens here.
    let item = view.item.unwrap();
    assert_eq!(item.id, Some(AnilistId::new(900_002)), "the other device's show");
    assert_eq!(item.episode.as_deref(), Some("1"));
    assert_eq!(item.title, "Stardust Wayfarer");
    assert_eq!(rig.receiver.heard("LAUNCH").len(), launches + 1);
    assert_eq!(rig.receiver.heard("LOAD").len(), 1, "nothing loaded again");
    // The first phone's connection went as this one came.
    playing.await.unwrap();
    let session = sender.cast_session().expect("a remote for the joined cast");
    let now = session.card().card.now.expect("the card of what the TV plays");
    assert_eq!((now.id, now.episode.as_str()), (AnilistId::new(900_002), "1"));
    assert_eq!(now.title, "Stardust Wayfarer");
    assert_eq!(now.episode_line, "Episode 1");

    // The remote's controls drive the TV, as the mini player's do.
    let pauses = rig.receiver.heard("PAUSE").len();
    session.command(RemoteCommand::TogglePause);
    session_until(&phone, |v| v.item.as_ref().is_some_and(|i| i.paused)).await;
    assert_eq!(rig.receiver.heard("PAUSE").len(), pauses + 1, "the TV paused");
    assert!(session.card().card.now.is_some_and(|now| now.paused));
    sender.cast_command(RemoteCommand::TogglePause);
    session_until(&phone, |v| v.item.as_ref().is_some_and(|i| !i.paused)).await;
    sender.cast_command(RemoteCommand::SeekTo { position: 300.0 });
    session_until(&phone, |v| v.item.as_ref().is_some_and(|i| i.position >= 299.0)).await;
    let media_stops = |receiver: &FakeReceiver| {
        receiver.heard("STOP").iter().filter(|m| m["mediaSessionId"].is_i64()).count()
    };
    sender.cast_command(RemoteCommand::Stop);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while media_stops(&rig.receiver) == 0 {
        assert!(tokio::time::Instant::now() < deadline, "the TV stopped the episode");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(phone.disconnect());
}

/// A phone joined to another device's cast, the cast as the other device leaves it.
async fn joined_from_another_phone(
    rig: &Rig,
) -> (Arc<CastHub>, crate::remote::sender::TvSender, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let phone =
        Arc::new(CastHub::with(Some(dir.path().to_owned()), Arc::new(PlainHttp), timings()));
    let (sender, _sent) = sender_for(&phone);
    phone.connect(rig.device());
    session_until(&phone, |v| v.state == SessionState::Playing && v.item.is_some()).await;
    (phone, sender, dir)
}

/// The device that started the cast casts its next episode: the phone that joined follows it,
/// its card on the new episode and its controls on what plays now.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_joined_phone_follows_the_next_episode_the_other_device_casts() {
    let (rig, playing) = playing_on_a_tv_that_plays_on().await;
    let (phone, sender, _dir) = joined_from_another_phone(&rig).await;
    playing.await.unwrap();

    rig.receiver.load_elsewhere_with(serde_json::json!({
        "kaeriCard": {"id": 900_002, "episode": "2", "title": "Stardust Wayfarer",
            "episodeTitle": "Low Tide"},
    }));
    let view = session_until(&phone, |v| {
        v.item.as_ref().is_some_and(|i| i.episode.as_deref() == Some("2"))
    })
    .await;
    assert_eq!(view.state, SessionState::Playing, "followed, not taken over");
    assert_eq!(view.notice, None);
    let item = view.item.unwrap();
    assert_eq!(item.id, Some(AnilistId::new(900_002)));
    assert_eq!(item.episode_title.as_deref(), Some("Low Tide"));
    let session = sender.cast_session().expect("the remote stays open");
    let now = session.card().card.now.expect("the new episode's card");
    assert_eq!(now.episode, "2");
    assert_eq!(now.episode_line, "Episode 2 · Low Tide");

    session.command(RemoteCommand::TogglePause);
    session_until(&phone, |v| v.item.as_ref().is_some_and(|i| i.paused)).await;
    let pause = rig.receiver.heard("PAUSE").pop().expect("a pause");
    assert_eq!(pause["mediaSessionId"], 2, "the new episode's media session");
    assert!(phone.disconnect());
}

/// Another app casts over what the joined phone followed: the TV is theirs, and the phone
/// says so.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_apps_cast_over_a_joined_phone_is_a_takeover() {
    let (rig, playing) = playing_on_a_tv_that_plays_on().await;
    let (phone, _sender, _dir) = joined_from_another_phone(&rig).await;
    playing.await.unwrap();

    rig.receiver.load_elsewhere();
    let view = session_until(&phone, |v| v.state == SessionState::Error).await;
    let said = crate::playback::link::cast::notice(&CastFailure::TakenOver, "Living room");
    assert_eq!(view.notice, Some(said));
    assert!(phone.session_device().is_none(), "plays open here again");
}

/// A cast that doesn't say what it plays, another app's or an older Kaeri's: the mini player's
/// controls work, and there is no card to open.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_joined_cast_that_does_not_say_what_it_plays_has_controls_and_no_card() {
    let (rig, playing) = playing_on_a_tv_that_plays_on().await;
    let (phone, _first, _dir) = joined_from_another_phone(&rig).await;
    playing.await.unwrap();
    // An older Kaeri's next episode, with no card: a takeover, so join it afresh.
    rig.receiver.load_elsewhere();
    session_until(&phone, |v| v.state == SessionState::Error).await;
    drop(phone);
    let dir = tempfile::tempdir().unwrap();
    let phone =
        Arc::new(CastHub::with(Some(dir.path().to_owned()), Arc::new(PlainHttp), timings()));
    let (sender, _sent) = sender_for(&phone);
    phone.connect(rig.device());
    let view = session_until(&phone, |v| {
        v.state == SessionState::Playing && v.item.as_ref().is_some_and(|i| !i.paused)
    })
    .await;
    let item = view.item.unwrap();
    assert_eq!((item.id, item.episode), (None, None), "nothing to open a card by");
    let session = sender.cast_session().expect("a cast to control");
    assert_eq!(session.card().card.now, None, "no card");

    let pauses = rig.receiver.heard("PAUSE").len();
    sender.cast_command(RemoteCommand::TogglePause);
    session_until(&phone, |v| v.item.as_ref().is_some_and(|i| i.paused)).await;
    assert_eq!(rig.receiver.heard("PAUSE").len(), pauses + 1, "the TV paused");
    assert!(phone.disconnect());
}

/// Choose another TV after the lost notice: the receiver played on without this device, and the
/// episode carries on from where the TV is, not from where this device last saw it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_tv_picked_again_after_a_lost_connection_carries_on_from_where_it_is() {
    // Ten seconds a second, so the TV is well ahead by the time it is picked again.
    let options =
        FakeOptions { remembers: true, duration: 600.0, speed: 10.0, ..FakeOptions::default() };
    let (rig, playing) = playing_on(options).await;
    let (sender, mut sent) = sender_on(&rig);
    let view =
        session_until(&rig.hub, |v| v.item.as_ref().is_some_and(|i| i.position >= 10.0)).await;
    let seen = view.item.unwrap().position;
    rig.receiver.drop_connection();
    let view = session_until(&rig.hub, |v| v.state == SessionState::Error).await;
    assert_eq!(view.notice, Some(Msg::NoticeCastLost { name: "Living room".into() }));
    playing.await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    sent();
    let launches = rig.receiver.heard("LAUNCH").len();

    // The card's Choose another TV, with the episode where the card last had it.
    sender.cast("cast:tv-1", AnilistId::new(900_002), "1".into(), Some(seen)).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let position = loop {
        if let Some(position) = sent().into_iter().find_map(|c| match c {
            Command::CastTo { position, .. } => Some(position),
            _ => None,
        }) {
            break position;
        }
        assert!(tokio::time::Instant::now() < deadline, "the episode is cast again");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let position = position.expect("a position to play from");
    assert!(position > seen + 5.0, "from the TV's {position} s, not this device's {seen} s");
    assert_eq!(rig.receiver.heard("LAUNCH").len(), launches, "joined, never launched again");
    assert_eq!(rig.hub.session().state, SessionState::Playing);

    // Asked once: a later cast of it goes from where it is asked to.
    assert!(rig.hub.disconnect());
}

/// A TV playing ten seconds a second, out of this device's reach for longer than it tries to
/// connect again: the cast reads Reconnecting all that time, then lost. The browse finds the TV
/// while it is still out of reach, so the session it was left running can't be joined yet.
/// Returns the playback's last place on it, once the TV can be reached again.
async fn lost_out_of_reach(
    options: FakeOptions,
) -> (Arc<Rig>, crate::remote::sender::TvSender, impl FnMut() -> Vec<Command>, f64) {
    let timings = Timings {
        silence: Duration::from_secs(1),
        reconnect: Duration::from_secs(3),
        retry: Duration::from_millis(500),
        ..timings()
    };
    let (rig, playing) = playing_with(options, timings).await;
    let (sender, mut sent) = sender_for(&rig.hub);
    let view =
        session_until(&rig.hub, |v| v.item.as_ref().is_some_and(|i| i.position >= 10.0)).await;
    let seen = view.item.unwrap().position;
    let away = Duration::from_secs(5);
    let back = tokio::time::Instant::now() + away;
    rig.receiver.unreachable_for(away);
    rig.receiver.drop_connection();
    let view = session_until(&rig.hub, |v| v.state == SessionState::Error).await;
    assert_eq!(view.notice, Some(Msg::NoticeCastLost { name: "Living room".into() }));
    playing.await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while rig.hub.noted_playing().is_none() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        rig.hub.noted_playing(),
        Some((AnilistId::new(900_002), "1".into())),
        "noted as left playing it"
    );

    // Choose another TV opens the picker, and the browse finds the TV still out of reach.
    rig.hub.rejoin(&rig.device());
    session_until(&rig.hub, |v| v.state == SessionState::Idle).await;
    tokio::time::sleep_until(back + Duration::from_millis(300)).await;
    sent();
    (rig, sender, sent, seen)
}

/// The episode the card sends with Choose another TV, once the cast picks it up again.
async fn cast_again_from(
    sender: &crate::remote::sender::TvSender,
    sent: &mut impl FnMut() -> Vec<Command>,
    position: f64,
) -> f64 {
    sender.cast("cast:tv-1", AnilistId::new(900_002), "1".into(), Some(position)).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(position) = sent().into_iter().find_map(|c| match c {
            Command::CastTo { position, .. } => Some(position),
            _ => None,
        }) {
            return position.expect("a position to play from");
        }
        assert!(tokio::time::Instant::now() < deadline, "the episode is cast again");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Choose another TV after a long loss, with the browse having found the TV while it was still
/// out of reach: the receiver played on, and is joined, not loaded again from where this device
/// last saw it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_tv_picked_after_a_long_loss_is_joined_where_it_played_on_to() {
    let options =
        FakeOptions { remembers: true, duration: 600.0, speed: 10.0, ..FakeOptions::default() };
    let (rig, sender, mut sent, seen) = lost_out_of_reach(options).await;
    let launches = rig.receiver.heard("LAUNCH").len();

    let position = cast_again_from(&sender, &mut sent, seen).await;
    assert!(position > seen + 30.0, "from the TV's {position} s, not this device's {seen} s");
    assert_eq!(rig.receiver.heard("LAUNCH").len(), launches, "joined, never launched again");
    assert!(rig.hub.disconnect());
}

/// Another phone stopped the receiver once the TV was back: nothing is left to join, and the
/// episode loads from where the engine last knew the TV to be, not from an older place the app
/// saved.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_tv_picked_after_a_long_loss_with_nothing_to_join_loads_where_it_was_last_known() {
    let options =
        FakeOptions { remembers: true, duration: 600.0, speed: 10.0, ..FakeOptions::default() };
    let (rig, sender, mut sent, seen) = lost_out_of_reach(options).await;
    let (other, _events) = CastPlayer::connect(rig.hub.setup(rig.device())).await.unwrap();
    other.launch().await.unwrap();
    other.quit().await.unwrap();

    let position = cast_again_from(&sender, &mut sent, 2.0).await;
    assert!(position >= seen, "from {position} s, where the TV was last known, {seen} s");
    assert!(position < seen + 30.0, "not run on past what was known: {position} s");
    assert!(rig.hub.disconnect());
}

/// The episode a TV was left playing is carried on there only for that episode.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_episode_cast_to_a_tv_left_playing_goes_from_its_own_position() {
    let (rig, playing) = playing_on_a_tv_that_plays_on().await;
    let (sender, mut sent) = sender_on(&rig);
    rig.receiver.drop_connection();
    session_until(&rig.hub, |v| v.state == SessionState::Error).await;
    playing.await.unwrap();
    sent();

    sender.cast("cast:tv-1", AnilistId::new(900_002), "2".into(), Some(42.0)).unwrap();
    assert_eq!(
        sent(),
        vec![Command::CastTo {
            device: "tv-1".into(),
            id: AnilistId::new(900_002),
            episode: "2".into(),
            position: Some(42.0),
        }]
    );
}
