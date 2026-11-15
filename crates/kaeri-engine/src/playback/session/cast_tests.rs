//! A playback cast to the fake receiver on loopback: the session drives the TV as it drives a
//! window, the next episode plays on the TV already showing it, and a stream the TV can't play
//! says why in the cast's own words.

use std::{sync::Arc, time::Duration};

use kaeri_cast::{
    CastDevice, CastFailure, DEFAULT_MEDIA_RECEIVER, PlainHttp, Timings,
    aac::FakeEncoder,
    fake::{
        FakeOptions, FakeReceiver,
        matroska::{Fixture, FixtureAudio, matroska},
    },
};
use kaeri_core::{
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind, Subtitle},
};
use kaeri_model::{Msg, command::PlayerCommand, event::Event, playback::Ending};
use kaeri_net::HttpClient;
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
    assert_eq!(rig.hub.left_on().as_deref(), Some("tv-1"), "noted for the next launch");

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
    let rig = Rig::with(true, FakeOptions { remembers: true, ..FakeOptions::default() }).await;
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
