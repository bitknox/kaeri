//! A cast against the fake receiver on loopback, from connect to the end of the file.

use std::{collections::HashMap, time::Duration};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

use super::*;
use crate::{
    aac::{FakeEncoder, NoEncoder},
    fake::{
        CustomReceiver, FakeOptions, FakeReceiver, Refusal, Refuse,
        matroska::{Fixture, FixtureAudio, matroska},
    },
    fetch::PlainHttp,
};

/// A loopback server answering each path with its body, ranges and all, without CORS: what the
/// stream proxy and the torrent server look like from here.
async fn serve(files: HashMap<String, Vec<u8>>) -> String {
    serve_slowly(files, HashMap::new()).await
}

/// [`serve`], with each path in `delays` sending its body only after its delay, its head at
/// once: a torrent's pieces not there yet, or a slow CDN.
async fn serve_slowly(
    files: HashMap<String, Vec<u8>>,
    delays: HashMap<String, Duration>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let files = files.clone();
            let delays = delays.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(socket);
                let mut line = String::new();
                if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                    return;
                }
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let mut range = None;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).await.unwrap_or(0) == 0
                        || header.trim().is_empty()
                    {
                        break;
                    }
                    if let Some(value) =
                        header.to_ascii_lowercase().strip_prefix("range: bytes=")
                    {
                        let (a, b) = value.trim().split_once('-').unwrap();
                        range = Some((a.parse::<usize>().unwrap(), b.parse::<usize>().ok()));
                    }
                }
                let mut socket = reader.into_inner();
                let Some(body) = files.get(&path) else {
                    let _ = socket
                        .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                        .await;
                    return;
                };
                let kind = if path.ends_with(".m3u8") {
                    "application/vnd.apple.mpegurl"
                } else if path.ends_with(".jpg") {
                    // A segment host that names its MPEG-TS as images.
                    "image/jpeg"
                } else {
                    "video/x-matroska"
                };
                let (status, part, extra) = match range {
                    Some((a, b)) => {
                        let b = b.unwrap_or(body.len() - 1).min(body.len() - 1);
                        (
                            "206 Partial Content",
                            &body[a..=b],
                            format!("Content-Range: bytes {a}-{b}/{}\r\n", body.len()),
                        )
                    }
                    None => ("200 OK", &body[..], String::new()),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nAccept-Ranges: bytes\r\n{extra}Content-Length: {}\r\n\r\n",
                    part.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                if let Some(delay) = delays.get(&path) {
                    tokio::time::sleep(*delay).await;
                }
                let _ = socket.write_all(part).await;
            });
        }
    });
    format!("http://{address}")
}

/// A cast player for the TV, launching the Default Media Receiver as a TV Kaeri's receiver isn't
/// published to does: the relay's conversions and clips. A test of Kaeri's receiver names it
/// with [`CastPlayer::use_receiver`] after this.
async fn connect_standard(
    setup: CastSetup,
) -> Result<(CastPlayer, mpsc::UnboundedReceiver<PlaybackEvent>), CastFailure> {
    let (player, events) = CastPlayer::connect(setup).await?;
    player.use_receiver(Some(DEFAULT_MEDIA_RECEIVER.into()));
    Ok((player, events))
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

fn setup(
    receiver: &FakeReceiver,
    memory: Arc<CastMemory>,
    encoder: Arc<dyn AacEncoder>,
) -> CastSetup {
    CastSetup {
        device: CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: receiver.address,
            video: true,
            group: false,
        },
        memory,
        upstream: Arc::new(PlainHttp),
        encoder,
        timings: timings(),
    }
}

fn torrent(url: String) -> CastLoad {
    CastLoad {
        source: CastSource { url: url.clone(), kind: SourceKind::Torrent, headers: Vec::new() },
        url,
        headers: Vec::new(),
        start: None,
        metadata: Metadata {
            series: "A Made-up Show".into(),
            title: "Episode 1".into(),
            image: None,
        },
        subtitles: Vec::new(),
        subtitle: None,
        audio_language: Some("ja".into()),
        audio_track: None,
        audio_memory: None,
    }
}

async fn next(
    rx: &mut mpsc::UnboundedReceiver<PlaybackEvent>,
    want: impl Fn(&PlaybackEvent) -> bool,
) -> PlaybackEvent {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let event = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .expect("the event in time")
            .expect("the player still there");
        if want(&event) {
            return event;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_torrents_matroska_is_remuxed_with_aac_and_driven_as_mpv_is() {
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions { speed: 8.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let encoder = FakeEncoder::default();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::clone(&memory), Arc::new(encoder)))
            .await
            .unwrap();
    assert!(receiver.heard("LAUNCH").is_empty(), "nothing starts on the TV before a load");

    let dir = tempfile::tempdir().unwrap();
    let subtitles = dir.path().join("episode.ass");
    std::fs::write(
        &subtitles,
        "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
         Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\n",
    )
    .unwrap();
    let mut load = torrent(format!("{base}/episode.mkv"));
    load.start = Some(4.0);
    load.subtitles = vec![CastSubtitle {
        path: subtitles,
        title: Some("English".into()),
        language: Some("en".into()),
    }];
    load.subtitle = Some(0);
    player.load(load);
    assert_eq!(next(&mut events, |e| *e == PlaybackEvent::Loaded).await, PlaybackEvent::Loaded);

    let sent = &receiver.heard("LOAD")[0];
    assert_eq!(sent["media"]["contentType"], "application/x-mpegurl");
    assert_eq!(sent["media"]["hlsSegmentFormat"], "fmp4");
    assert_eq!(sent["media"]["metadata"]["seriesTitle"], "A Made-up Show");
    assert_eq!(sent["media"]["tracks"][0]["trackContentType"], "text/vtt");
    assert_eq!(sent["activeTrackIds"], serde_json::json!([1]));
    assert_eq!(sent["currentTime"], 4.0);
    let fetched = receiver.fetched();
    assert!(fetched.iter().all(|f| f.cors && f.status == 200), "{fetched:?}");
    assert_eq!(fetched.len(), 3, "playlist, init, first segment");
    let plan = player.status().borrow().plan.clone().unwrap();
    assert_eq!(plan.audio, AudioPlan::ToAac { from: crate::formats::AudioCodec::Ac3 });
    assert!(memory.caps("tv-1").played("hls-fmp4"));

    player.toggle_pause().await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Paused(true)).await;
    player.set_paused(false).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Paused(false)).await;
    player.set_rate(1.5).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Speed(1.5)).await;
    player.seek(15.0, true).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Seeking(false)).await;
    // The position keeps moving between changes because it is asked for.
    let PlaybackEvent::Progress { position, .. } = next(
        &mut events,
        |e| matches!(e, PlaybackEvent::Progress { position, .. } if *position > 16.0),
    )
    .await
    else {
        unreachable!()
    };
    assert!(position <= 20.0);
    assert_eq!(
        next(&mut events, |e| matches!(e, PlaybackEvent::Ended { .. })).await,
        PlaybackEvent::Ended { complete: true }
    );
    player.quit().await.unwrap();
    assert!(player.has_quit() && !player.is_alive());
    assert!(
        receiver.heard("STOP").iter().any(|m| m["sessionId"].is_string()),
        "the app stopped"
    );
}

fn opus() -> FixtureAudio {
    let mut opus = FixtureAudio::aac();
    opus.codec = "A_OPUS";
    opus.private = b"OpusHead\x01\x02\x38\x01\x80\xbb\x00\x00\x00\x00\x00".to_vec();
    opus.frame_ms = 20;
    opus
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_codec_falls_back_to_aac_and_the_next_cast_starts_there() {
    let file = matroska(&Fixture { audio: Some(opus()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    // However the TV says no: idle with an error, an `ERROR` message, or a stall.
    for how in [Refusal::IdleError, Refusal::Error, Refusal::Stall] {
        let receiver = FakeReceiver::start(FakeOptions {
            speed: 4.0,
            refuse: Refuse {
                bytes: vec![b"Opus".to_vec()],
                how: Some(how),
                ..Refuse::default()
            },
            ..FakeOptions::default()
        })
        .await
        .unwrap();
        let memory = Arc::new(CastMemory::memory());
        let (player, mut events) = connect_standard(setup(
            &receiver,
            Arc::clone(&memory),
            Arc::new(FakeEncoder::default()),
        ))
        .await
        .unwrap();

        // Opus copied is refused, and the same load goes on with the Opus turned to AAC.
        player.load(torrent(format!("{base}/episode.mkv")));
        next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
        assert_eq!(receiver.heard("LOAD").len(), 2, "{how:?}");
        let plan = player.status().borrow().plan.clone().unwrap();
        assert_eq!(plan.audio, AudioPlan::ToAac { from: crate::formats::AudioCodec::Opus });
        assert!(memory.caps("tv-1").refused("opus/hls"), "{how:?}");
        assert!(!memory.caps("tv-1").refused("hls-fmp4"));

        // The next cast turns the Opus to AAC from the start.
        player.load(torrent(format!("{base}/episode.mkv")));
        next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
        assert_eq!(receiver.heard("LOAD").len(), 3);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_that_stops_hls_at_720_lines_plays_a_1080_line_file_passed_through() {
    let tall = Fixture {
        audio: Some(FixtureAudio::aac()),
        width: 1920,
        height: 1080,
        ..Fixture::default()
    };
    let short = Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() };
    let base = serve(HashMap::from([
        ("/1080.mkv".to_owned(), matroska(&tall)),
        ("/360.mkv".to_owned(), matroska(&short)),
    ]))
    .await;
    let receiver = FakeReceiver::start(FakeOptions {
        speed: 4.0,
        refuse: Refuse { hls_taller_than: Some(720), ..Refuse::default() },
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::clone(&memory), Arc::new(NoEncoder)))
            .await
            .unwrap();

    // The remux is refused for its height, and the file as it is plays.
    player.load(torrent(format!("{base}/1080.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let sent = receiver.heard("LOAD");
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1]["media"]["contentType"], "video/x-matroska");
    let caps = memory.caps("tv-1");
    assert!(caps.refused("h264/1080/hls"));
    assert!(!caps.refused("hls-fmp4"), "HLS itself still plays");
    assert!(caps.played("mkv"));

    // The next 1080-line file goes straight to the file; a smaller one is still remuxed.
    player.load(torrent(format!("{base}/1080.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(receiver.heard("LOAD").len(), 3);
    assert_eq!(receiver.heard("LOAD")[2]["media"]["contentType"], "video/x-matroska");
    player.load(torrent(format!("{base}/360.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(receiver.heard("LOAD")[3]["media"]["contentType"], "application/x-mpegurl");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_file_whose_picture_a_tv_may_lack_is_asked_about_with_a_clip_first() {
    // A file the remuxer can't read, so it goes as it is: HEVC with sound.
    let hevc = Fixture {
        video: "V_MPEGH/ISO/HEVC",
        cues: false,
        audio: Some(FixtureAudio::aac()),
        ..Fixture::default()
    };
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), matroska(&hevc))])).await;

    // A TV with no HEVC decoder plays such a file's sound alone and says nothing. The clip,
    // with no sound, is refused, and the file never reaches the TV.
    let hub = FakeReceiver::start(FakeOptions {
        speed: 4.0,
        refuse: Refuse {
            bytes: vec![b"hvc1".to_vec(), b"V_MPEGH".to_vec()],
            how: Some(Refusal::LoadFailed),
            sound_alone: true,
            ..Refuse::default()
        },
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        connect_standard(setup(&hub, Arc::clone(&memory), Arc::new(NoEncoder))).await.unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { .. } | PlaybackEvent::Loaded))
        .await;
    let sent = hub.heard("LOAD");
    assert_eq!(sent.len(), 1, "the clip alone");
    assert_eq!(sent[0]["media"]["contentType"], "video/mp4");
    assert_eq!(
        player.failure(),
        Some(CastFailure::Uncastable(Uncastable::VideoCodec("HEVC".into())))
    );
    assert!(memory.caps("tv-1").refused("hevc/720/progressive"));

    // A TV that plays the clip gets the file next, and is not asked again.
    let tv = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        connect_standard(setup(&tv, Arc::clone(&memory), Arc::new(NoEncoder))).await.unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let sent = tv.heard("LOAD");
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0]["media"]["contentType"], "video/mp4");
    assert_eq!(sent[1]["media"]["contentType"], "video/x-matroska");
    assert!(memory.caps("tv-1").played("hevc/720/progressive"));
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(tv.heard("LOAD").len(), 3, "no clip the second time");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_file_the_remuxer_cant_read_is_passed_through() {
    let fixture =
        Fixture { cues: false, audio: Some(FixtureAudio::aac()), ..Fixture::default() };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("episode.mkv");
    std::fs::write(&path, matroska(&fixture)).unwrap();
    let receiver = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::clone(&memory), Arc::new(NoEncoder)))
            .await
            .unwrap();
    let url = path.to_string_lossy().into_owned();
    let mut load = torrent(url.clone());
    load.source = CastSource { url, kind: SourceKind::File, headers: Vec::new() };
    player.load(load);
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let sent = receiver.heard("LOAD");
    assert_eq!(sent.len(), 1, "the remux plan never reached the TV");
    assert_eq!(sent[0]["media"]["contentType"], "video/x-matroska");
    assert!(memory.caps("tv-1").played("mkv"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hls_behind_the_stream_proxy_is_relayed_with_cors() {
    let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg0.ts\n#EXTINF:4.0,\nseg1.ts\n#EXT-X-ENDLIST\n";
    let mut segment = vec![0u8; 188 * 4];
    for packet in segment.chunks_mut(188) {
        packet[0] = 0x47;
    }
    let base = serve(HashMap::from([
        ("/a/index.m3u8".to_owned(), playlist.as_bytes().to_vec()),
        ("/a/seg0.ts".to_owned(), segment.clone()),
        ("/a/seg1.ts".to_owned(), segment),
    ]))
    .await;
    let receiver = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::clone(&memory), Arc::new(NoEncoder)))
            .await
            .unwrap();
    let url = format!("{base}/a/index.m3u8");
    let load = CastLoad {
        source: CastSource {
            url: url.clone(),
            kind: SourceKind::Hls,
            headers: vec![("Referer".into(), "https://player.example/".into())],
        },
        ..torrent(url)
    };
    player.load(load);
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let fetched = receiver.fetched();
    assert_eq!(fetched.len(), 2);
    assert!(fetched.iter().all(|f| f.cors && f.url.contains("/u/")), "{fetched:?}");
    assert_eq!(receiver.heard("LOAD")[0]["media"]["hlsVideoSegmentFormat"], "mpeg2_ts");
    assert_eq!(
        next(&mut events, |e| matches!(e, PlaybackEvent::Ended { .. })).await,
        PlaybackEvent::Ended { complete: true }
    );
    assert!(memory.caps("tv-1").played("hls-ts"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_first_segment_is_waited_for_and_teaches_nothing() {
    let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg0.ts\n#EXTINF:4.0,\nseg1.ts\n#EXT-X-ENDLIST\n";
    let mut segment = vec![0u8; 188 * 4];
    for packet in segment.chunks_mut(188) {
        packet[0] = 0x47;
    }
    // The first segment takes twice the movement window and more than the fetch window, as a
    // torrent that has only just started does.
    let base = serve_slowly(
        HashMap::from([
            ("/a/index.m3u8".to_owned(), playlist.as_bytes().to_vec()),
            ("/a/seg0.ts".to_owned(), segment.clone()),
            ("/a/seg1.ts".to_owned(), segment),
        ]),
        HashMap::from([("/a/seg0.ts".to_owned(), Duration::from_secs(3))]),
    )
    .await;
    let receiver = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let mut setup = setup(&receiver, Arc::clone(&memory), Arc::new(NoEncoder));
    setup.timings.slow_source = Duration::from_secs(1);
    let (player, mut events) = connect_standard(setup).await.unwrap();
    let url = format!("{base}/a/index.m3u8");
    let load = CastLoad {
        source: CastSource {
            url: url.clone(),
            kind: SourceKind::Hls,
            headers: vec![("Referer".into(), "https://player.example/".into())],
        },
        ..torrent(url)
    };
    player.load(load);
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(receiver.heard("LOAD").len(), 1, "the first plan played, once it could");
    let caps = memory.caps("tv-1");
    assert!(
        caps.formats.values().all(|answer| answer.played),
        "a slow source is not a refusal: {:?}",
        caps.formats
    );
    assert!(caps.played("hls-ts"));
}

/// An HLS stream of 1080-line H.264 whose two segments are `seg0.jpg` and `seg1.jpg` with
/// `segment`'s bytes, behind a referer, and the load for it.
async fn hls_with_jpg_segments(segment: Vec<u8>) -> CastLoad {
    let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=900000,RESOLUTION=1920x1080,CODECS=\"avc1.640028,mp4a.40.2\"\nv/index.m3u8\n";
    let media = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg0.jpg\n#EXTINF:4.0,\nseg1.jpg\n#EXT-X-ENDLIST\n";
    let base = serve(HashMap::from([
        ("/a/master.m3u8".to_owned(), master.as_bytes().to_vec()),
        ("/a/v/index.m3u8".to_owned(), media.as_bytes().to_vec()),
        ("/a/v/seg0.jpg".to_owned(), segment.clone()),
        ("/a/v/seg1.jpg".to_owned(), segment),
    ]))
    .await;
    let url = format!("{base}/a/master.m3u8");
    CastLoad {
        source: CastSource {
            url: url.clone(),
            kind: SourceKind::Hls,
            headers: vec![("Referer".into(), "https://player.example/".into())],
        },
        ..torrent(url)
    }
}

fn ts_segment() -> Vec<u8> {
    let mut segment = vec![0xFFu8; 188 * 8];
    for packet in segment.chunks_mut(188) {
        packet[0] = 0x47;
    }
    segment
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn segments_named_as_images_reach_the_tv_as_the_media_they_are() {
    let receiver = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::clone(&memory), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(hls_with_jpg_segments(ts_segment()).await);
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(receiver.heard("LOAD").len(), 1);
    let caps = memory.caps("tv-1");
    assert!(caps.played("h264/1080/hls"), "{:?}", caps.formats);
    assert!(caps.formats.values().all(|answer| answer.played), "{:?}", caps.formats);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refusal_after_bytes_no_receiver_plays_blames_no_codec() {
    // Each segment has an image's header before its MPEG-TS, which the TV can't read.
    let mut segment = b"\x89PNG\r\n\x1a\n".to_vec();
    segment.extend(ts_segment());
    let receiver = FakeReceiver::start(FakeOptions {
        refuse: Refuse {
            bytes: vec![b"\x89PNG".to_vec()],
            how: Some(Refusal::LoadFailed),
            ..Refuse::default()
        },
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::clone(&memory), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(hls_with_jpg_segments(segment).await);
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(player.failure(), Some(CastFailure::Refused), "not a codec the TV lacks");
    assert!(memory.caps("tv-1").formats.is_empty(), "{:?}", memory.caps("tv-1").formats);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_uncastable_stream_never_takes_the_tv_over() {
    let base =
        serve(HashMap::from([("/episode.mkv".to_owned(), matroska(&Fixture::default()))]))
            .await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(
        player.failure(),
        Some(CastFailure::Uncastable(Uncastable::AudioCodec("AC-3".into())))
    );
    assert!(receiver.heard("LAUNCH").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_that_never_fetches_ends_the_cast() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver =
        FakeReceiver::start(FakeOptions { fetches: false, ..FakeOptions::default() })
            .await
            .unwrap();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(player.failure(), Some(CastFailure::NeverFetches));
    assert_eq!(receiver.heard("LOAD").len(), 1, "no second plan: none would fare better");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_while_the_tv_starts_the_receiver_closes_it_once_up() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions {
        launch: Duration::from_millis(800),
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, _events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while receiver.heard("LAUNCH").is_empty() {
        assert!(tokio::time::Instant::now() < deadline, "the load launched the receiver");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // Stop on the card while the card still says "Starting".
    player.quit().await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !receiver.heard("STOP").iter().any(|m| m["sessionId"].is_string()) {
        assert!(tokio::time::Instant::now() < deadline, "the receiver was left up on the TV");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(receiver.heard("LOAD").is_empty(), "nothing was loaded after the stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_sender_taking_over_ends_the_cast() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    receiver.take_over();
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(player.failure(), Some(CastFailure::TakenOver));
    assert!(!player.is_alive());
    // The relay closed with the cast, though the player is still held.
    let served = receiver.fetched()[0].url.clone();
    assert!(PlainHttp.get(&served, &[]).await.is_err(), "the relay still answers");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_speed_asked_for_before_a_load_is_set_once_it_plays() {
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) = connect_standard(setup(
        &receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(FakeEncoder::default()),
    ))
    .await
    .unwrap();
    // The session sets the speed from its options before it loads the first file.
    player.set_rate(1.5).await.unwrap();
    assert!(receiver.heard("SET_PLAYBACK_RATE").is_empty());
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    next(&mut events, |e| *e == PlaybackEvent::Speed(1.5)).await;
    assert_eq!(receiver.heard("SET_PLAYBACK_RATE")[0]["playbackRate"], 1.5);
    assert_eq!(player.rate(), 1.5);
    player.quit().await.unwrap();
}

/// A cast of a short file to a fake that plays at four times real time, playing.
async fn playing(
    receiver: &FakeReceiver,
) -> (CastPlayer, mpsc::UnboundedReceiver<PlaybackEvent>) {
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let (player, mut events) = connect_standard(setup(
        receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(FakeEncoder::default()),
    ))
    .await
    .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    (player, events)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rate_of_0_while_the_tv_buffers_is_no_speed() {
    let receiver = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let (player, mut events) = playing(&receiver).await;
    next(&mut events, |e| *e == PlaybackEvent::Speed(1.0)).await;
    let no_speed = |e: &PlaybackEvent| {
        assert!(
            !matches!(e, PlaybackEvent::Speed(_)),
            "the TV waiting for data was taken for a speed: {e:?}"
        );
    };
    receiver.buffer(true);
    next(&mut events, |e| {
        no_speed(e);
        *e == PlaybackEvent::Buffering(true)
    })
    .await;
    // A step up from here is a step up from normal speed, never from 0.
    assert_eq!(player.rate(), 1.0);
    receiver.buffer(false);
    next(&mut events, |e| {
        no_speed(e);
        matches!(e, PlaybackEvent::Progress { position, .. } if *position >= 2.0)
    })
    .await;
    assert_eq!(player.rate(), 1.0);
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_speed_of_0_asks_the_tv_for_nothing() {
    let receiver = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let (player, mut events) = connect_standard(setup(
        &receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(FakeEncoder::default()),
    ))
    .await
    .unwrap();
    // A session carrying a speed of 0 into the cast.
    player.set_rate(f64::NAN).await.unwrap();
    player.set_rate(0.0).await.unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    next(
        &mut events,
        |e| matches!(e, PlaybackEvent::Progress { position, .. } if *position >= 2.0),
    )
    .await;
    assert!(
        receiver.heard("SET_PLAYBACK_RATE").is_empty(),
        "{:?}",
        receiver.heard("SET_PLAYBACK_RATE")
    );
    assert_eq!(player.rate(), 1.0);
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_late_subtitle_keeps_the_speed_asked_for() {
    let receiver = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let (player, mut events) = playing(&receiver).await;
    player.set_rate(1.5).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Speed(1.5)).await;

    // The subtitle loads the file again where it is, and a load starts at normal speed.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("late.srt");
    std::fs::write(&path, "1\n00:00:01,000 --> 00:00:02,000\nHello\n").unwrap();
    player.add_subtitle(&path, None, Some("en".into())).await.unwrap();
    assert_eq!(receiver.heard("LOAD").len(), 2);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while receiver.heard("SET_PLAYBACK_RATE").len() < 2 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the speed asked for was not set again after the reload"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(receiver.heard("SET_PLAYBACK_RATE")[1]["playbackRate"], 1.5);
    next(&mut events, |e| *e == PlaybackEvent::Speed(1.5)).await;
    assert_eq!(player.rate(), 1.5);
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_that_goes_silent_ends_the_cast_and_never_holds_a_control_up() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let mut setup = setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder));
    setup.timings.silence = Duration::from_secs(2);
    let (player, mut events) = connect_standard(setup).await.unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    // Long enough on a live TV that the heartbeat and polls keep the channel up.
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(player.is_alive(), "a TV that answers is kept");

    // Switched off at the wall: the socket stays open, and nothing comes back.
    receiver.go_silent();
    let began = tokio::time::Instant::now();
    for control in 0..3 {
        let asked = tokio::time::Instant::now();
        let sent = match control {
            0 => player.set_paused(true).await,
            1 => player.seek(10.0, true).await,
            _ => player.set_volume(50.0).await,
        };
        assert!(sent.is_ok() || !player.is_alive(), "{sent:?}");
        assert!(asked.elapsed() < Duration::from_secs(3), "a control waited on a silent TV");
    }
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert!(began.elapsed() < Duration::from_secs(8), "noticed in {:?}", began.elapsed());
    assert!(!player.is_alive());
    assert!(matches!(player.failure(), Some(CastFailure::Lost(_))), "{:?}", player.failure());
    assert_eq!(player.status().borrow().state, CastState::Ended);

    // Stop on the card doesn't wait out the silence either.
    let asked = tokio::time::Instant::now();
    let _ = player.quit().await;
    assert!(asked.elapsed() < Duration::from_secs(3));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_with_fixed_volume_only_mutes_and_a_long_pause_ends_the_cast() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver =
        FakeReceiver::start(FakeOptions { control: "fixed", ..FakeOptions::default() })
            .await
            .unwrap();
    let mut setup = setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder));
    setup.timings.idle_close = Duration::from_millis(800);
    let (player, mut events) = connect_standard(setup).await.unwrap();
    assert_eq!(player.volume_control(), VolumeControl::MuteOnly);
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;

    player.set_volume(30.0).await.unwrap();
    assert!(receiver.heard("SET_VOLUME").is_empty(), "the TV's own volume is in charge");
    player.set_muted(true).await.unwrap();
    assert_eq!(receiver.heard("SET_VOLUME")[0]["volume"]["muted"], true);

    player.set_paused(true).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Paused(true)).await;
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    // The media's own stop ends the file a moment before the app is closed.
    let closed = async {
        while !(receiver.heard("STOP").iter().any(|m| m["sessionId"].is_string())
            && !player.is_alive())
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(5), closed).await.expect("the app closed");
}

#[test]
fn subtitles_become_webvtt() {
    let srt =
        "1\n00:00:01,000 --> 00:00:02,500\nHello\n\n2\n00:00:03,000 --> 00:00:04,000\nAgain\n";
    let vtt = to_webvtt(srt, Some("srt"));
    assert!(vtt.starts_with("WEBVTT\n\n"));
    assert!(vtt.contains("00:00:01.000 --> 00:00:02.500\nHello"));
    assert!(!vtt.contains("\n1\n"));
    assert_eq!(to_webvtt("WEBVTT\n\nx", None), "WEBVTT\n\nx");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_audio_track_loads_the_file_again_where_it_was_without_a_new_start() {
    let dub = FixtureAudio { language: "eng", ..FixtureAudio::aac() };
    let file = Fixture {
        seconds: 60,
        audio: Some(FixtureAudio::aac()),
        more_audio: vec![dub],
        ..Fixture::default()
    };
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), matroska(&file))])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    let mut status = player.status();
    player.load(torrent(format!("{base}/episode.mkv")));
    assert!(status.borrow_and_update().step.is_some(), "a load not yet playing says so");
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let playing = player.status().borrow().clone();
    assert_eq!(playing.step, None);
    let languages: Vec<_> =
        playing.audio.iter().map(|a| (a.index, a.language.as_deref(), a.selected)).collect();
    assert_eq!(languages, [(0, Some("jpn"), true), (1, Some("eng"), false)]);

    // Played a little, the dub is picked.
    next(
        &mut events,
        |e| matches!(e, PlaybackEvent::Progress { position, .. } if *position >= 2.0),
    )
    .await;
    player.set_audio(1);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while receiver.heard("LOAD").len() < 2
        || !player.status().borrow().audio.iter().any(|a| a.index == 1 && a.selected)
        || player.status().borrow().step.is_some()
    {
        assert!(tokio::time::Instant::now() < deadline, "the dub loaded");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let loads = receiver.heard("LOAD");
    assert!(loads[1]["currentTime"].as_f64().unwrap_or(0.0) >= 2.0, "{:?}", loads[1]);
    // The session heard nothing start again: no second `Loaded` before the file plays on.
    next(&mut events, |e| match e {
        PlaybackEvent::Loaded => panic!("the reload was taken for a new file"),
        PlaybackEvent::Progress { position, .. } => *position >= 3.0,
        _ => false,
    })
    .await;
    // The track playing isn't loaded again.
    player.set_audio(1);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(receiver.heard("LOAD").len(), 2);
}

/// Cast a 60 s Matroska file with a dub to a fake, play a little, and pause it.
async fn paused_with_a_dub()
-> (CastPlayer, mpsc::UnboundedReceiver<PlaybackEvent>, FakeReceiver) {
    let dub = FixtureAudio { language: "eng", ..FixtureAudio::aac() };
    let file = Fixture {
        seconds: 60,
        audio: Some(FixtureAudio::aac()),
        more_audio: vec![dub],
        ..Fixture::default()
    };
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), matroska(&file))])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    next(
        &mut events,
        |e| matches!(e, PlaybackEvent::Progress { position, .. } if *position >= 2.0),
    )
    .await;
    player.set_paused(true).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Paused(true)).await;
    (player, events, receiver)
}

/// Whether the cast stays paused for a while, saying nothing of playing again.
async fn stays_paused(
    player: &CastPlayer,
    events: &mut mpsc::UnboundedReceiver<PlaybackEvent>,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, events.recv()).await {
        assert_ne!(event, PlaybackEvent::Paused(false), "the TV started playing by itself");
    }
    assert_eq!(player.status().borrow().state, CastState::Paused);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_audio_track_picked_while_paused_loads_paused() {
    let (player, mut events, receiver) = paused_with_a_dub().await;
    player.set_audio(1);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while receiver.heard("LOAD").len() < 2
        || !player.status().borrow().audio.iter().any(|a| a.index == 1 && a.selected)
        || player.status().borrow().step.is_some()
    {
        assert!(tokio::time::Instant::now() < deadline, "the dub loaded");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let loads = receiver.heard("LOAD");
    assert_eq!(loads[1]["autoplay"], false, "{:?}", loads[1]);
    assert!(loads[1]["currentTime"].as_f64().unwrap_or(0.0) >= 2.0, "{:?}", loads[1]);
    stays_paused(&player, &mut events).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_late_subtitle_while_paused_loads_paused() {
    let (player, mut events, receiver) = paused_with_a_dub().await;
    let dir = tempfile::tempdir().unwrap();
    let late = dir.path().join("late.vtt");
    std::fs::write(&late, "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello\n").unwrap();
    player.add_subtitle(&late, None, Some("en".into())).await.unwrap();
    let loads = receiver.heard("LOAD");
    assert_eq!(loads.len(), 2);
    assert_eq!(loads[1]["autoplay"], false, "{:?}", loads[1]);
    stays_paused(&player, &mut events).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_fixtures_test_pattern_plays_on_a_fake_that_plays_what_it_cant_fetch() {
    let receiver = FakeReceiver::start(FakeOptions {
        plays_unfetched: true,
        duration: 1440.0,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    let url = "av://lavfi:testsrc=duration=1440:size=320x240:rate=30".to_owned();
    let mut load = torrent(url.clone());
    load.source.kind = SourceKind::Progressive;
    player.load(load);
    assert_eq!(next(&mut events, |e| *e == PlaybackEvent::Loaded).await, PlaybackEvent::Loaded);
    assert_eq!(
        receiver.heard("LOAD")[0]["media"]["contentId"],
        url.as_str(),
        "direct, as it came"
    );
    assert_eq!(player.failure(), None);
}

#[test]
fn a_fixtures_advert_reads_as_a_tv_with_a_screen() {
    let address = "127.0.0.1:8009".parse().unwrap();
    let txt = crate::fake::advert_txt("fake-1", "Living room", "Chromecast");
    let device = CastDevice::from_txt("Chromecast-fake-1", address, &txt).unwrap();
    assert_eq!((device.name.as_str(), device.id.as_str()), ("Living room", "fake-1"));
    assert!(device.shown());
}

fn media_status(state: &str, position: f64) -> MediaStatus {
    MediaStatus {
        media_session_id: 1,
        state: state.into(),
        idle_reason: None,
        position,
        duration: None,
        rate: 1.0,
        active_tracks: Vec::new(),
        volume: None,
        muted: None,
        content: None,
        metadata: None,
        has_media: false,
    }
}

#[test]
fn what_was_asked_holds_until_a_status_agrees_or_a_while_has_passed() {
    let now = Instant::now();
    let mut want = Wanted {
        paused: Some((true, now)),
        position: Some((300.0, now)),
        level: Some((0.2, now)),
        ..Wanted::default()
    };
    // A poll answered before the pause and the seek landed.
    let mut before = media_status("PLAYING", 12.0);
    before.volume = Some(0.4);
    want.media(&before);
    assert!(want.paused.is_some() && want.position.is_some() && want.level.is_some());
    let mut after = media_status("PAUSED", 300.4);
    after.volume = Some(0.2);
    want.media(&after);
    assert!(want.paused.is_none() && want.position.is_none() && want.level.is_none());

    let long_ago = Instant::now().checked_sub(CONFIRM).unwrap();
    let mut want = Wanted { paused: Some((true, long_ago)), ..Wanted::default() };
    want.media(&media_status("PLAYING", 12.0));
    assert!(want.paused.is_none(), "a TV that kept playing wins in the end");

    let mut want = Wanted { paused: Some((false, now)), ..Wanted::default() };
    want.media(&media_status("IDLE", 0.0));
    assert!(want.paused.is_none(), "nothing to play or pause once the file ended");
}

/// Each status seen: its state, its duration, and whether it had the media's description.
type Seen = Arc<Mutex<Vec<(CastState, Option<f64>, bool)>>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn controls_show_at_once_and_the_duration_outlives_statuses_without_the_media() {
    let receiver =
        FakeReceiver::start(FakeOptions { duration: 600.0, ..FakeOptions::default() })
            .await
            .unwrap();
    let (player, mut events) = playing(&receiver).await;
    let seen: Seen = Arc::default();
    let watching = tokio::spawn({
        let (mut status, seen) = (player.status(), Arc::clone(&seen));
        async move {
            while status.changed().await.is_ok() {
                let now = status.borrow_and_update().clone();
                lock(&seen).push((now.state, now.duration, now.metadata.is_some()));
            }
        }
    });
    // The remuxed playlist gives the file's own length.
    let duration = player.current().duration;
    assert!(duration.is_some_and(|d| d > 0.0));

    let pause = player.set_paused(true);
    tokio::pin!(pause);
    // Polled once: the request is out and the TV hasn't answered.
    let _ = futures::poll!(&mut pause);
    assert_eq!(player.current().state, CastState::Paused, "paused on the tap");
    pause.await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Paused(true)).await;

    player.seek(120.0, true).await.unwrap();
    assert!((player.current().position - 120.0).abs() < 2.0);
    receiver.buffer(true);
    next(&mut events, |e| *e == PlaybackEvent::Buffering(true)).await;
    receiver.buffer(false);
    player.set_paused(false).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Paused(false)).await;

    let now = player.current();
    assert_eq!(now.duration, duration, "{now:?}");
    assert_eq!(now.metadata.map(|m| m.series).as_deref(), Some("A Made-up Show"));
    watching.abort();
    player.quit().await.unwrap();
    let seen = lock(&seen).clone();
    assert!(seen.len() >= 4, "{seen:?}");
    let playing = seen.iter().filter(|(state, ..)| {
        matches!(state, CastState::Playing | CastState::Paused | CastState::Buffering)
    });
    for (state, duration, metadata) in playing {
        assert!(duration.is_some() && *metadata, "{state:?} without the media: {seen:?}");
    }
}

/// A fake whose own receiver does `custom`, and a cast to it of a short file, playing.
async fn cast_with_own_receiver(
    custom: CustomReceiver,
) -> (FakeReceiver, CastPlayer, mpsc::UnboundedReceiver<PlaybackEvent>) {
    let receiver =
        FakeReceiver::start(FakeOptions { custom, ..FakeOptions::default() }).await.unwrap();
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let mut setup =
        setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default()));
    setup.timings.ready = Duration::from_millis(800);
    let (player, mut events) = connect_standard(setup).await.unwrap();
    player.use_receiver(Some("0A1B2C3D".into()));
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    (receiver, player, events)
}

fn launched(receiver: &FakeReceiver) -> Vec<String> {
    receiver.heard("LAUNCH").iter().map(|m| m["appId"].as_str().unwrap().to_owned()).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kaeris_own_receiver_plays_where_it_runs() {
    let (receiver, player, _events) = cast_with_own_receiver(CustomReceiver::Runs).await;
    assert_eq!(launched(&receiver), ["0A1B2C3D"]);
    assert_eq!(player.current().receiver.as_deref(), Some("0A1B2C3D"));
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_that_refuses_kaeris_receiver_plays_on_the_default_one() {
    let (receiver, player, _events) = cast_with_own_receiver(CustomReceiver::Unknown).await;
    assert_eq!(launched(&receiver), ["0A1B2C3D", DEFAULT_MEDIA_RECEIVER]);
    assert_eq!(player.current().receiver.as_deref(), Some(DEFAULT_MEDIA_RECEIVER));
    assert_eq!(player.failure(), None);
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kaeris_receiver_closing_at_its_first_load_hands_over_to_the_default_one() {
    let (receiver, player, _events) =
        cast_with_own_receiver(CustomReceiver::ClosesAtLoad).await;
    assert_eq!(launched(&receiver), ["0A1B2C3D", DEFAULT_MEDIA_RECEIVER]);
    assert_eq!(player.current().receiver.as_deref(), Some(DEFAULT_MEDIA_RECEIVER));
    assert_eq!(player.failure(), None);
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kaeris_receiver_that_never_says_ready_hands_over_to_the_default_one() {
    let (receiver, player, _events) = cast_with_own_receiver(CustomReceiver::Silent).await;
    assert_eq!(launched(&receiver), ["0A1B2C3D", DEFAULT_MEDIA_RECEIVER]);
    assert_eq!(player.current().receiver.as_deref(), Some(DEFAULT_MEDIA_RECEIVER));
    assert_eq!(player.failure(), None);
    assert!(receiver.heard("hello").len() == 1, "greeted once, never on the standard one");
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kaeris_receiver_is_greeted_and_its_messages_are_heard() {
    let (receiver, player, mut events) = cast_with_own_receiver(CustomReceiver::Runs).await;
    let hello = receiver.heard("hello");
    assert_eq!(hello.len(), 1);
    assert_eq!(hello[0]["version"], KAERI_PROTOCOL);
    let mut heard = player.heard();
    player.set_greeting(Some("sv".into()), Some("phone".into()));
    assert_eq!(receiver_heard_soon(&receiver, "hello", 2).await[1]["device"], "phone");
    player.tell_skipped("opening");
    assert_eq!(receiver_heard_soon(&receiver, "skipped", 1).await[0]["part"], "opening");

    receiver.say(serde_json::json!({"type": "subtitles-plain", "version": 1}));
    assert_eq!(heard.recv().await.unwrap(), FromReceiver::SubtitlesPlain);
    receiver.say(serde_json::json!({"type": "up-next", "version": 1, "action": "play"}));
    assert_eq!(heard.recv().await.unwrap(), FromReceiver::UpNext { play: true });
    receiver.say(serde_json::json!({"type": "something-new", "version": 2}));
    // A subtitle that didn't load leaves the episode playing.
    receiver.say(serde_json::json!({"type": "error", "part": "subtitles", "status": 404}));
    let FromReceiver::Error(error) = heard.recv().await.unwrap() else { panic!() };
    assert_eq!((error.part.as_str(), error.status), ("subtitles", Some(404)));
    assert_eq!(player.failure(), None);

    // A segment the source didn't give ends the episode with the one sentence for it.
    receiver.say(serde_json::json!({"type": "error", "part": "network", "status": 503}));
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(player.failure(), Some(CastFailure::Segment));
    assert!(player.is_alive(), "the receiver stays up for the next play");
    player.quit().await.unwrap();
}

/// A cast of `file` on Kaeri's receiver that refuses it, saying `says` as it does; the player
/// once the cast has ended, and the TV's memory.
async fn refused_on_own_receiver(
    file: Vec<u8>,
    says: Value,
) -> (FakeReceiver, CastPlayer, Arc<CastMemory>) {
    let receiver = FakeReceiver::start(FakeOptions {
        refuse: Refuse {
            content_types: vec!["video/x-matroska".into()],
            how: Some(Refusal::LoadFailed),
            says: Some(says),
            ..Refuse::default()
        },
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let memory = Arc::new(CastMemory::memory());
    let mut setup = setup(&receiver, Arc::clone(&memory), Arc::new(FakeEncoder::default()));
    setup.timings.ready = Duration::from_millis(800);
    let (player, _events) = connect_standard(setup).await.unwrap();
    player.use_receiver(Some("0A1B2C3D".into()));
    player.load(torrent(format!("{base}/episode.mkv")));
    for _ in 0..200 {
        if player.failure().is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    (receiver, player, memory)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kaeris_receiver_is_handed_the_file_as_it_is() {
    let file = matroska(&Fixture {
        audio: Some(opus()),
        subtitle: Some("S_TEXT/ASS"),
        attachments: vec![
            ("Sample Sans.ttf", "application/x-truetype-font", b"\0\x01\0\0glyphs".to_vec()),
            ("cover.jpg", "image/jpeg", b"\xff\xd8\xff".to_vec()),
        ],
        ..Fixture::default()
    });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let mut setup =
        setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default()));
    setup.timings.ready = Duration::from_millis(800);
    let (player, mut events) = connect_standard(setup).await.unwrap();
    player.use_receiver(Some("0A1B2C3D".into()));
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;

    // No remux, no AAC, no clip: the relay passes the Matroska through as it is.
    let loads = receiver.heard("LOAD");
    assert_eq!(loads.len(), 1);
    let media = &loads[0]["media"];
    assert_eq!(media["contentType"], "video/x-matroska");
    assert!(!media["contentId"].as_str().unwrap().contains("/x/r/"), "{media}");
    let kaeri = &media["customData"]["kaeri"];
    assert_eq!(kaeri["version"], KAERI_PROTOCOL);
    assert_eq!(kaeri["container"], "mkv");
    assert_eq!(kaeri["route"], "relay");
    assert_eq!(kaeri["url"], media["contentId"]);
    assert_eq!(kaeri["audio"]["codec"], "opus");
    assert_eq!(kaeri["video"]["codec"], "h264");
    // The file's font for its typeset track, and no cover art.
    let fonts = kaeri["fonts"].as_array().unwrap();
    assert_eq!(fonts.len(), 1, "{kaeri}");
    assert!(fonts[0].as_str().unwrap().ends_with("/x/f/1.ttf"), "{kaeri}");
    let plan = player.status().borrow().plan.clone().unwrap();
    assert_eq!(plan.route, CastRoute::Relayed { remux: false });
    assert_eq!(plan.audio, AudioPlan::Copy(crate::formats::AudioCodec::Opus));
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_picture_kaeris_receiver_cant_decode_ends_naming_its_codec() {
    let (receiver, player, _memory) = refused_on_own_receiver(
        matroska(&Fixture::default()),
        serde_json::json!({"type": "error", "version": 1, "part": "video", "codec": "hev1"}),
    )
    .await;
    assert_eq!(
        player.failure(),
        Some(CastFailure::Uncastable(Uncastable::VideoCodec("hev1".into())))
    );
    // No device converts video, and the standard receiver wouldn't play it either.
    assert_eq!(receiver.heard("LOAD").len(), 1);
    assert_eq!(launched(&receiver), ["0A1B2C3D"]);
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_audio_error_on_kaeris_receiver_never_blames_the_video() {
    let (_receiver, player, memory) = refused_on_own_receiver(
        matroska(&Fixture { audio: Some(opus()), ..Fixture::default() }),
        serde_json::json!({"type": "error", "version": 1, "part": "audio", "codec": "opus"}),
    )
    .await;
    assert_eq!(
        player.failure(),
        Some(CastFailure::Uncastable(Uncastable::AudioCodec("opus".into())))
    );
    // Kaeri's receiver said what failed: nothing is guessed, so nothing is marked.
    let caps = memory.caps("tv-1");
    assert!(caps.formats.values().all(|answer| answer.played), "{caps:?}");
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refusal_kaeris_receiver_gives_no_codec_for_says_the_plain_sentence() {
    let (_receiver, player, _memory) = refused_on_own_receiver(
        matroska(&Fixture::default()),
        serde_json::json!({"type": "error", "version": 1, "part": "manifest"}),
    )
    .await;
    assert_eq!(player.failure(), Some(CastFailure::Refused));
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failure_kaeris_receiver_cant_place_loads_the_same_plan_once_more() {
    let (receiver, player, _memory) = refused_on_own_receiver(
        matroska(&Fixture::default()),
        serde_json::json!({"type": "error", "version": 1, "part": "unknown"}),
    )
    .await;
    assert_eq!(player.failure(), Some(CastFailure::Refused));
    let loads = receiver.heard("LOAD");
    assert_eq!(loads.len(), 2);
    assert_eq!(loads[0]["media"]["contentId"], loads[1]["media"]["contentId"]);
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_connect_tries_the_original_again() {
    let file = matroska(&Fixture { audio: Some(opus()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let memory = Arc::new(CastMemory::memory());
    for session in 0..2 {
        let receiver = FakeReceiver::start(FakeOptions {
            speed: 4.0,
            refuse: Refuse {
                bytes: vec![b"Opus".to_vec()],
                how: Some(Refusal::IdleError),
                ..Refuse::default()
            },
            ..FakeOptions::default()
        })
        .await
        .unwrap();
        let (player, mut events) = connect_standard(setup(
            &receiver,
            Arc::clone(&memory),
            Arc::new(FakeEncoder::default()),
        ))
        .await
        .unwrap();
        player.load(torrent(format!("{base}/episode.mkv")));
        next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
        // Each session tries the Opus as it is first; the refusal of the last one is gone.
        assert_eq!(receiver.heard("LOAD").len(), 2, "session {session}");
        player.quit().await.unwrap();
    }
}

/// The messages of `kind` the fake heard, once there are `count`.
async fn receiver_heard_soon(receiver: &FakeReceiver, kind: &str, count: usize) -> Vec<Value> {
    for _ in 0..50 {
        let heard = receiver.heard(kind);
        if heard.len() >= count {
            return heard;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("never heard {count} {kind}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_session_left_running_is_joined_with_what_it_plays() {
    let receiver = FakeReceiver::start(FakeOptions {
        duration: 600.0,
        remembers: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, _events) = playing(&receiver).await;
    let duration = player.current().duration;
    let session = player.receiver().unwrap().session_id;
    // The app is closed with the cast running: the TV keeps playing.
    drop(player);
    let joined = CastPlayer::join(
        setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default())),
        &session,
    )
    .await
    .unwrap();
    let (player, _events) = joined.expect("the receiver still runs");
    let now = player.current();
    assert_eq!(now.state, CastState::Playing, "{now:?}");
    assert_eq!(now.duration, duration);
    assert_eq!(now.metadata.map(|m| m.title).as_deref(), Some("Episode 1"));
    player.set_paused(true).await.unwrap();
    assert_eq!(receiver.heard("PAUSE").len(), 1, "controls reach the joined receiver");
    player.quit().await.unwrap();

    let gone = CastPlayer::join(
        setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default())),
        &session,
    )
    .await
    .unwrap();
    assert!(gone.is_none(), "a session that ended isn't joined");
}

/// An HLS master with the original and a dub as audio renditions beside one picture, the
/// original the default, behind headers so the relay serves it.
async fn hls_with_a_dub() -> CastLoad {
    let master = "#EXTM3U\n\
        #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",LANGUAGE=\"ja\",NAME=\"Japanese\",DEFAULT=YES,AUTOSELECT=YES,URI=\"ja/index.m3u8\"\n\
        #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",LANGUAGE=\"en\",NAME=\"English\",DEFAULT=NO,AUTOSELECT=YES,URI=\"en/index.m3u8\"\n\
        #EXT-X-STREAM-INF:BANDWIDTH=900000,RESOLUTION=1280x720,CODECS=\"avc1.64001f,mp4a.40.2\",AUDIO=\"aud\"\n\
        v/index.m3u8\n";
    let media = |name: &str| {
        let mut text = String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n");
        for i in 0..15 {
            text.push_str(&format!("#EXTINF:4.0,\n{name}{i}.ts\n"));
        }
        text.push_str("#EXT-X-ENDLIST\n");
        text.into_bytes()
    };
    let mut files = HashMap::from([
        ("/a/master.m3u8".to_owned(), master.as_bytes().to_vec()),
        ("/a/v/index.m3u8".to_owned(), media("v")),
        ("/a/ja/index.m3u8".to_owned(), media("a")),
        ("/a/en/index.m3u8".to_owned(), media("a")),
    ]);
    for dir in ["v", "ja", "en"] {
        for i in 0..15 {
            let name = if dir == "v" { "v" } else { "a" };
            files.insert(format!("/a/{dir}/{name}{i}.ts"), ts_segment());
        }
    }
    let base = serve(files).await;
    let url = format!("{base}/a/master.m3u8");
    CastLoad {
        source: CastSource {
            url: url.clone(),
            kind: SourceKind::Hls,
            headers: vec![("Referer".into(), "https://player.example/".into())],
        },
        ..torrent(url)
    }
}

/// Which audio renditions the fake fetched, by their folder.
fn renditions_fetched(receiver: &FakeReceiver) -> Vec<&'static str> {
    let fetched = receiver.fetched();
    ["ja", "en"]
        .into_iter()
        .filter(|lang| {
            fetched.iter().any(|f| f.url.contains(&format!("%2F{lang}%2Findex.m3u8")))
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_tv_gets_the_remembered_dub_alone_and_the_card_switches_it() {
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    // A sub watcher's language, but the series remembers the dub: the dub plays, as in the app.
    let load = CastLoad {
        audio_language: Some("ja".into()),
        audio_memory: Some(TrackPrint { lang: Some("eng".into()), ..TrackPrint::default() }),
        ..hls_with_a_dub().await
    };
    player.load(load);
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let loads = receiver.heard("LOAD");
    let content = loads[0]["media"]["contentId"].as_str().unwrap().to_owned();
    assert!(content.ends_with("/x/h/master.m3u8"), "{content}");
    assert_eq!(renditions_fetched(&receiver), ["en"], "{:?}", receiver.fetched());
    let playing = player.status().borrow().clone();
    let audio: Vec<_> = playing
        .audio
        .iter()
        .map(|a| (a.index, a.language.as_deref(), a.title.as_deref(), a.selected))
        .collect();
    assert_eq!(audio, [(0, Some("ja"), None, false), (1, Some("en"), None, true)]);

    // The original, picked on the card, loads where the file was.
    next(
        &mut events,
        |e| matches!(e, PlaybackEvent::Progress { position, .. } if *position >= 2.0),
    )
    .await;
    let before = receiver.fetched().len();
    player.set_audio(0);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while receiver.heard("LOAD").len() < 2
        || !player.status().borrow().audio.iter().any(|a| a.index == 0 && a.selected)
        || player.status().borrow().step.is_some()
    {
        assert!(tokio::time::Instant::now() < deadline, "the original loaded");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let loads = receiver.heard("LOAD");
    assert!(loads[1]["currentTime"].as_f64().unwrap_or(0.0) >= 2.0, "{:?}", loads[1]);
    let after: Vec<_> = receiver.fetched()[before..].iter().map(|f| f.url.clone()).collect();
    assert!(after.iter().any(|u| u.contains("%2Fja%2Findex.m3u8")), "{after:?}");
    assert!(!after.iter().any(|u| u.contains("%2Fen%2Findex.m3u8")), "{after:?}");
}

#[test]
fn the_cast_picks_audio_as_the_app_does() {
    let track = |language: &str, name: &str, default| AudioTrack {
        language: Some(language.into()),
        name: Some(name.into()),
        default,
        ..AudioTrack::default()
    };
    let info = MediaInfo {
        audio: vec![
            track("ja", "Japanese", true),
            track("en", "English Commentary", false),
            track("en", "English", false),
        ],
        renditions: true,
        ..MediaInfo::default()
    };
    let load = |language: Option<&str>| CastLoad {
        audio_language: language.map(str::to_owned),
        ..torrent("https://cdn.example/a/master.m3u8".into())
    };
    // A dub watcher gets the programme in their language, not the commentary.
    assert_eq!(app_pick(&info, &load(Some("eng"))), Some(2));
    assert_eq!(app_pick(&info, &load(Some("ja"))), Some(0));
    // A language the stream lacks: its default.
    assert_eq!(app_pick(&info, &load(Some("fr"))), Some(0));
    // The card names a rendition only when the name says more than the language.
    assert_eq!(audio_title(&info.audio[0]), None);
    assert_eq!(audio_title(&info.audio[1]).as_deref(), Some("English Commentary"));
}
