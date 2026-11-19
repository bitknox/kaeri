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
        gone: Duration::from_secs(1),
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
        card: None,
    }
}

async fn next(
    rx: &mut mpsc::UnboundedReceiver<PlaybackEvent>,
    want: impl Fn(&PlaybackEvent) -> bool,
) -> PlaybackEvent {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let mut seen = Vec::new();
    loop {
        let event = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("the event in time; heard only {seen:?}"))
            .expect("the player still there");
        if want(&event) {
            return event;
        }
        if !matches!(event, PlaybackEvent::Progress { .. }) {
            seen.push(event);
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
async fn another_sender_loading_on_the_same_receiver_session_takes_the_tv() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    receiver.load_elsewhere();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let event = tokio::time::timeout_at(deadline, events.recv())
            .await
            .expect("the cast ends")
            .expect("the events go on until it ends");
        assert_ne!(event, PlaybackEvent::Loaded, "their file is never taken for this one's");
        assert!(
            !matches!(event, PlaybackEvent::Ended { complete: true }),
            "their file never marks this episode watched"
        );
        if matches!(event, PlaybackEvent::Ended { .. }) {
            break;
        }
    }
    assert_eq!(player.failure(), Some(CastFailure::TakenOver));
    assert!(!player.is_alive());
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
    setup.timings.reconnect = Duration::from_secs(2);
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
        // While the connection is made again a control finds it closed, and says so at once.
        assert!(
            sent.is_ok() || matches!(sent, Err(CastError::Closed)) || !player.is_alive(),
            "{sent:?}"
        );
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

/// A player whose connection is taken for gone after a second of silence, and made again within
/// `reconnect`.
async fn playing_quick_to_notice(
    receiver: &FakeReceiver,
    reconnect: Duration,
) -> (CastPlayer, mpsc::UnboundedReceiver<PlaybackEvent>) {
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let mut setup =
        setup(receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default()));
    setup.timings.silence = Duration::from_secs(1);
    setup.timings.reconnect = reconnect;
    let (player, mut events) = connect_standard(setup).await.unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    (player, events)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_connection_that_drops_while_the_tv_plays_on_is_made_again() {
    let receiver = FakeReceiver::start(FakeOptions {
        duration: 600.0,
        remembers: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, mut events) = playing_quick_to_notice(&receiver, Duration::from_secs(6)).await;
    let connects = receiver.heard("CONNECT").len();

    // The phone's screen goes off and its Wi-Fi naps: the connection dies, the TV plays on.
    receiver.drop_connection();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    while receiver.heard("CONNECT").len() <= connects + 1 {
        assert!(tokio::time::Instant::now() < deadline, "connected again in time");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let at = player.position().unwrap_or_default();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(player.is_alive(), "the cast goes on");
    assert_eq!(player.failure(), None);
    assert_eq!(player.current().state, CastState::Playing, "{:?}", player.current());
    assert!(player.position().unwrap_or_default() > at, "the position moves on");
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event, PlaybackEvent::Ended { .. }), "never ended: {event:?}");
    }

    player.set_paused(true).await.unwrap();
    assert_eq!(receiver.heard("PAUSE").len(), 1, "controls reach the TV on the new connection");
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_connection_the_tv_closes_while_it_plays_on_is_made_again_and_joins_its_receiver() {
    let receiver = FakeReceiver::start(FakeOptions {
        duration: 600.0,
        remembers: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, mut events) = playing_quick_to_notice(&receiver, Duration::from_secs(6)).await;
    let connects = receiver.heard("CONNECT").len();
    let launches = receiver.heard("LAUNCH").len();
    let loads = receiver.heard("LOAD").len();

    // The TV closes the socket, with a FIN, and plays on.
    receiver.close_connection();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    while receiver.heard("CONNECT").len() <= connects + 1 {
        assert!(tokio::time::Instant::now() < deadline, "connected again in time");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(player.is_alive(), "the cast goes on");
    assert_eq!(player.failure(), None);
    assert_eq!(player.current().state, CastState::Playing, "{:?}", player.current());
    assert_eq!(
        receiver.heard("LAUNCH").len(),
        launches,
        "the receiver is joined, not launched"
    );
    assert_eq!(receiver.heard("LOAD").len(), loads, "nothing loads again");
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event, PlaybackEvent::Ended { .. }), "never ended: {event:?}");
    }
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_connection_made_again_to_a_tv_that_closed_the_app_ends_the_cast() {
    // A TV that forgets between connections: the app it ran is gone on the next one.
    let receiver =
        FakeReceiver::start(FakeOptions { duration: 600.0, ..FakeOptions::default() })
            .await
            .unwrap();
    let (player, mut events) = playing_quick_to_notice(&receiver, Duration::from_secs(6)).await;
    let began = tokio::time::Instant::now();
    receiver.drop_connection();
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert!(began.elapsed() < Duration::from_secs(5), "noticed in {:?}", began.elapsed());
    assert!(!player.is_alive());
    assert!(matches!(player.failure(), Some(CastFailure::Lost(_))), "{:?}", player.failure());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_that_never_answers_again_ends_the_cast_once_the_tries_run_out() {
    let receiver = FakeReceiver::start(FakeOptions {
        duration: 600.0,
        remembers: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, mut events) = playing_quick_to_notice(&receiver, Duration::from_secs(3)).await;
    let began = tokio::time::Instant::now();
    receiver.go_silent();
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(player.is_alive(), "still trying");
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    let took = began.elapsed();
    assert!(took < Duration::from_secs(7), "gave up in {took:?}");
    assert!(!player.is_alive());
    assert_eq!(player.status().borrow().state, CastState::Ended);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_with_fixed_volume_is_sent_no_volume_and_a_long_pause_ends_the_cast() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver =
        FakeReceiver::start(FakeOptions { control: "fixed", ..FakeOptions::default() })
            .await
            .unwrap();
    let mut setup = setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder));
    setup.timings.idle_close = Duration::from_millis(800);
    let (player, mut events) = connect_standard(setup).await.unwrap();
    assert_eq!(player.volume_control(), VolumeControl::Fixed);
    assert!(!player.can_mute());
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(player.current().volume, VolumeControl::Fixed);
    assert!(!player.current().can_mute, "the card offers no mute");

    // A fixed TV says it took a level or a mute, but its sound stays where the TV has it.
    player.set_volume(30.0).await.unwrap();
    player.set_muted(true).await.unwrap();
    assert!(receiver.heard("SET_VOLUME").is_empty(), "the TV's own volume is in charge");
    assert!(!player.muted());

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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_muted_at_the_receiver_stays_muted_through_media_statuses() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let level = player.current().level;
    player.set_muted(true).await.unwrap();
    // Past the time a control is shown unconfirmed, with media statuses polled all along: each
    // says its stream plays unmuted at full volume.
    let until = tokio::time::Instant::now() + CONFIRM + Duration::from_millis(500);
    let mut statuses = 0;
    while tokio::time::Instant::now() < until {
        if let Ok(Some(PlaybackEvent::Progress { .. })) =
            tokio::time::timeout_at(until, events.recv()).await
        {
            statuses += 1;
        }
    }
    assert!(statuses > 5, "media statuses came in: {statuses}");
    assert!(player.muted(), "the TV's mute holds");
    let status = player.current();
    assert_eq!(status.muted, Some(true));
    assert_eq!(status.level, level, "the level stays the receiver's");
    assert_eq!(status.volume, VolumeControl::Level);
    assert!(status.can_mute);
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_that_refuses_a_mute_or_a_level_is_no_longer_offered_it() {
    let receiver = FakeReceiver::start(FakeOptions {
        refuses_level: true,
        refuses_mute: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, _events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    assert_eq!(player.volume_control(), VolumeControl::Level);
    assert!(player.can_mute(), "an attenuation TV offers both until it refuses one");
    let mut status = player.status();

    assert!(player.set_muted(true).await.is_err());
    assert!(!player.can_mute());
    assert!(!player.muted(), "the refused mute isn't shown");
    assert!(!status.borrow_and_update().can_mute, "the card hears it at once");
    assert!(player.set_volume(30.0).await.is_err());
    assert_eq!(player.volume_control(), VolumeControl::Fixed);
    assert_eq!(status.borrow_and_update().volume, VolumeControl::Fixed);

    // Neither is sent again.
    player.set_muted(true).await.unwrap();
    player.set_volume(40.0).await.unwrap();
    assert_eq!(receiver.heard("SET_VOLUME").len(), 2);
    player.quit().await.unwrap();
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
        card: None,
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
    assert!(want.paused.is_none() && want.position.is_none());
    // The stream's volume in a media status is not the TV's: only the receiver confirms it.
    assert!(want.level.is_some(), "a media status never confirms the TV's volume");
    want.volume(Some(0.2), None);
    assert!(want.level.is_none());

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

    // Inside the 20 s file the remuxed playlist describes, with time to spare after it resumes:
    // past its end, the resume could hear the file finish before it plays.
    player.seek(8.0, true).await.unwrap();
    assert!((player.current().position - 8.0).abs() < 2.0);
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
    // The watch keeps only the newest status, so a busy machine sees fewer of them.
    let seen = lock(&seen).clone();
    assert!(!seen.is_empty(), "{seen:?}");
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
async fn kaeris_receiver_stopped_on_the_tv_mid_episode_ends_the_cast_and_launches_nothing() {
    let (receiver, player, mut events) = cast_with_own_receiver(CustomReceiver::Runs).await;
    // Its connection closes before the receiver's status says nothing runs.
    receiver.stop_app();
    // While the TV is heard out, the card no longer plays: it waits on the TV, its clock still.
    let waiting = async {
        loop {
            let status = player.current();
            if status.state != CastState::Playing {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    let status = tokio::time::timeout(Duration::from_millis(500), waiting)
        .await
        .expect("the card stops playing at once");
    assert_eq!(status.state, CastState::Buffering);
    assert!(status.waiting_since.is_some());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(player.current().position, status.position, "the clock stands still");
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    let ended = async {
        while player.is_alive() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(5), ended).await.expect("the cast ended");
    assert_eq!(
        player.failure(),
        Some(CastFailure::StoppedElsewhere),
        "not lost: the TV answers"
    );
    assert_eq!(player.current().state, CastState::Ended);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(launched(&receiver), ["0A1B2C3D"], "no other app is put on the TV");
    assert_eq!(receiver.heard("LOAD").len(), 1, "the episode isn't loaded again");
    let _ = player.quit().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kaeris_receiver_stopped_on_the_tv_at_a_load_ends_the_cast_and_launches_nothing() {
    let receiver = FakeReceiver::start(FakeOptions {
        custom: CustomReceiver::StoppedAtLoad,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let mut setup =
        setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default()));
    setup.timings.ready = Duration::from_millis(800);
    let (player, mut events) = connect_standard(setup).await.unwrap();
    player.use_receiver(Some("0A1B2C3D".into()));
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(player.failure(), Some(CastFailure::StoppedElsewhere));
    assert!(!player.is_alive());
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(launched(&receiver), ["0A1B2C3D"], "the Default Media Receiver isn't put on");
    assert_eq!(receiver.heard("LOAD").len(), 1, "the episode isn't loaded again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cast_stopped_before_the_tv_played_it_is_still_where_its_load_starts() {
    // The card carries this to another TV, where 0 s would start the episode over.
    let receiver = FakeReceiver::start(FakeOptions {
        custom: CustomReceiver::StoppedAtLoad,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let mut setup =
        setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default()));
    setup.timings.ready = Duration::from_millis(800);
    let (player, mut events) = connect_standard(setup).await.unwrap();
    player.use_receiver(Some("0A1B2C3D".into()));
    player.load(CastLoad { start: Some(600.0), ..torrent(format!("{base}/episode.mkv")) });
    assert_eq!(player.current().position, 600.0, "where the load starts while it loads");
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(player.current().state, CastState::Ended);
    assert_eq!(player.current().position, 600.0);
    assert_eq!(player.position(), Some(600.0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cast_stopped_on_the_tv_while_it_loads_says_no_progress_from_its_start() {
    // The receiver reports the file at 0 s before it seeks to the resume point; a stop then must
    // leave the session no position to save over where the episode was left.
    let receiver = FakeReceiver::start(FakeOptions {
        custom: CustomReceiver::StoppedWhileLoading,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let mut setup =
        setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default()));
    setup.timings.ready = Duration::from_millis(800);
    let (player, mut events) = connect_standard(setup).await.unwrap();
    player.use_receiver(Some("0A1B2C3D".into()));
    player.load(CastLoad { start: Some(600.0), ..torrent(format!("{base}/episode.mkv")) });
    let mut heard = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let event = tokio::time::timeout_at(deadline, events.recv())
            .await
            .unwrap_or_else(|_| panic!("the cast ended in time; heard {heard:?}"))
            .expect("the player still there");
        let ended = matches!(event, PlaybackEvent::Ended { complete: false });
        heard.push(event);
        if ended {
            break;
        }
    }
    assert!(
        !heard.iter().any(|e| matches!(e, PlaybackEvent::Progress { .. })),
        "no progress from a file that never played: {heard:?}"
    );
    assert!(!player.is_alive());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn buffering_after_the_file_played_still_says_where_it_is() {
    let (receiver, player, mut events) = cast_with_own_receiver(CustomReceiver::Runs).await;
    let PlaybackEvent::Progress { position, .. } =
        next(&mut events, |e| matches!(e, PlaybackEvent::Progress { .. })).await
    else {
        unreachable!()
    };
    receiver.buffer(true);
    // The buffering status comes with the place, as it did before it buffered.
    let mut said = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let event = tokio::time::timeout_at(deadline, events.recv())
            .await
            .expect("buffering in time")
            .expect("the player still there");
        match event {
            PlaybackEvent::Progress { position, .. } => said = Some(position),
            PlaybackEvent::Buffering(true) => break,
            _ => {}
        }
    }
    let said = said.expect("a place with the buffering status");
    assert!(said >= position, "buffering keeps the place: {said} < {position}");
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_sender_taking_over_kaeris_receiver_ends_the_cast_and_loads_nothing() {
    let (receiver, player, mut events) = cast_with_own_receiver(CustomReceiver::Runs).await;
    // The TV closes this app's connection and shows nothing running before the other app is up.
    receiver.take_over();
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    let ended = async {
        while player.is_alive() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(5), ended).await.expect("the cast ended");
    assert_eq!(player.failure(), Some(CastFailure::TakenOver));
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(launched(&receiver), ["0A1B2C3D"], "nothing is launched over the other app");
    assert_eq!(receiver.heard("LOAD").len(), 1, "nothing is loaded on the other app");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kaeris_receiver_stopped_on_the_tv_while_idle_ends_the_session_and_launches_nothing() {
    let receiver = FakeReceiver::start(FakeOptions {
        custom: CustomReceiver::Runs,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let mut setup =
        setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default()));
    setup.timings.ready = Duration::from_millis(800);
    let (player, _events) = connect_standard(setup).await.unwrap();
    player.use_receiver(Some("0A1B2C3D".into()));
    player.launch().await.unwrap();
    receiver.stop_app();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while player.is_alive() {
        assert!(tokio::time::Instant::now() < deadline, "the session ended");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(launched(&receiver), ["0A1B2C3D"], "no other app is put on the TV");
    assert_eq!(player.failure(), Some(CastFailure::StoppedElsewhere));
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

/// Wait for the player's status to read as `want` says, up to 15 seconds, and return it.
async fn status_until(player: &CastPlayer, want: impl Fn(&CastStatus) -> bool) -> CastStatus {
    let mut status = player.status();
    let waited = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let now = status.borrow_and_update().clone();
            if want(&now) {
                return now;
            }
            status.changed().await.expect("the player still there");
        }
    });
    waited.await.unwrap_or_else(|_| panic!("the status in time; last {:?}", player.current()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_cast_tries_kaeris_receiver_first_however_often_it_fell_back() {
    let receiver = FakeReceiver::start(FakeOptions {
        custom: CustomReceiver::Silent,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    // One memory for both casts, as the engine keeps one for every TV.
    let memory = Arc::new(CastMemory::memory());
    for cast in 1..=2 {
        let mut setup = setup(&receiver, Arc::clone(&memory), Arc::new(FakeEncoder::default()));
        setup.timings.ready = Duration::from_millis(800);
        let (player, mut events) = connect_standard(setup).await.unwrap();
        player.use_receiver(Some("0A1B2C3D".into()));
        assert!(
            !player.current().fallen_back,
            "cast {cast} hasn't fallen back before it tries"
        );
        player.load(torrent(format!("{base}/episode.mkv")));
        next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
        let status = player.current();
        assert!(status.fallen_back, "cast {cast} says it fell back");
        assert_eq!(status.receiver.as_deref(), Some(DEFAULT_MEDIA_RECEIVER));
        player.quit().await.unwrap();
    }
    assert_eq!(
        launched(&receiver),
        ["0A1B2C3D", DEFAULT_MEDIA_RECEIVER, "0A1B2C3D", DEFAULT_MEDIA_RECEIVER],
        "the second cast tries Kaeri's receiver first too"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kaeris_receiver_that_comes_up_never_reads_as_fallen_back() {
    let (_receiver, player, _events) = cast_with_own_receiver(CustomReceiver::Runs).await;
    assert!(!player.current().fallen_back);
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_dropped_connection_reads_as_reconnecting_until_it_joins_again() {
    let receiver = FakeReceiver::start(FakeOptions {
        duration: 600.0,
        remembers: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, _events) = playing_quick_to_notice(&receiver, Duration::from_secs(6)).await;
    assert!(!player.current().reconnecting);

    // The TV takes a moment to answer again, so the time between is there to be seen.
    receiver.answer_after(Duration::from_millis(500));
    receiver.drop_connection();
    let reconnecting = status_until(&player, |s| s.reconnecting).await;
    assert_eq!(reconnecting.state, CastState::Playing, "the TV plays on meanwhile");
    assert!(player.is_alive());
    let back = status_until(&player, |s| !s.reconnecting).await;
    assert_eq!(back.state, CastState::Playing, "back where it was: {back:?}");
    assert_eq!(back.failure, None);
    assert!(player.is_alive());
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reconnecting_ends_with_the_cast_when_the_tries_run_out() {
    let receiver = FakeReceiver::start(FakeOptions {
        duration: 600.0,
        remembers: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, _events) = playing_quick_to_notice(&receiver, Duration::from_secs(3)).await;
    let mut seen = player.status();
    receiver.go_silent();
    status_until(&player, |s| s.reconnecting).await;
    // From then on, every status reads as reconnecting until the one that ends the cast: it
    // never reads as playing on the way.
    seen.borrow_and_update();
    let ended = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            seen.changed().await.expect("the player still there");
            let now = seen.borrow_and_update().clone();
            if now.state == CastState::Ended {
                return now;
            }
            assert!(now.reconnecting, "read as {now:?} before the end");
        }
    })
    .await
    .expect("the cast ended in time");
    assert!(!ended.reconnecting);
    assert!(matches!(ended.failure, Some(CastFailure::Lost(_))), "{:?}", ended.failure);
    assert!(!player.is_alive());
}

/// A player on the TV with the engine's own silence and tries, playing a long file.
async fn playing_with_the_engines_timings(
    receiver: &FakeReceiver,
    change: impl FnOnce(&mut Timings),
) -> (CastPlayer, mpsc::UnboundedReceiver<PlaybackEvent>) {
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let mut setup =
        setup(receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default()));
    assert_eq!(setup.timings.silence, Timings::default().silence);
    assert_eq!(setup.timings.reconnect, Duration::from_secs(20));
    change(&mut setup.timings);
    let (player, mut events) = connect_standard(setup).await.unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    (player, events)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_short_drop_off_the_network_is_ridden_out_within_seconds_of_coming_back() {
    let receiver = FakeReceiver::start(FakeOptions {
        duration: 600.0,
        remembers: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, _events) = playing_with_the_engines_timings(&receiver, |_| {}).await;

    // The phone leaves the Wi-Fi for 3 s: the socket closes at once, and the tries made
    // meanwhile go nowhere and are never answered.
    receiver.unreachable_for(Duration::from_secs(3));
    let dropped = tokio::time::Instant::now();
    receiver.close_connection();
    status_until(&player, |s| s.reconnecting).await;
    let back = status_until(&player, |s| !s.reconnecting).await;
    let took = dropped.elapsed();
    assert!(took < Duration::from_secs(6), "joined again {took:?} after the drop");
    assert_eq!(back.failure, None, "{back:?}");
    assert!(player.is_alive());
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_change_of_network_while_reconnecting_tries_the_tv_again_at_once() {
    let receiver = FakeReceiver::start(FakeOptions {
        duration: 600.0,
        remembers: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    // Tries long enough that only the change of network brings the next one early.
    let (player, _events) = playing_with_the_engines_timings(&receiver, |timings| {
        timings.retry = Duration::from_secs(10);
    })
    .await;

    receiver.unreachable_for(Duration::from_secs(1));
    receiver.close_connection();
    status_until(&player, |s| s.reconnecting).await;
    tokio::time::sleep(Duration::from_millis(1300)).await;
    // Back on the TV's Wi-Fi, as the system says.
    let changed = tokio::time::Instant::now();
    player.network_changed();
    status_until(&player, |s| !s.reconnecting).await;
    let took = changed.elapsed();
    assert!(took < Duration::from_secs(2), "joined again {took:?} after the change");
    assert_eq!(player.failure(), None);
    assert!(player.is_alive());
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stop_on_the_tv_a_takeover_and_a_lost_tv_each_end_the_cast_their_own_way() {
    // Stopped on the TV: no app runs afterwards, and the TV still answers.
    let (receiver, player, _events) = cast_with_own_receiver(CustomReceiver::Runs).await;
    receiver.stop_app();
    let ended = status_until(&player, |s| s.state == CastState::Ended).await;
    assert_eq!(ended.failure, Some(CastFailure::StoppedElsewhere));

    // Another sender's app comes up where Kaeri's ran.
    let (receiver, player, _events) = cast_with_own_receiver(CustomReceiver::Runs).await;
    receiver.take_over();
    let ended = status_until(&player, |s| s.state == CastState::Ended).await;
    assert_eq!(ended.failure, Some(CastFailure::TakenOver));

    // The TV went away: nothing answers, then or on the tries to connect again.
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, _events) = playing_quick_to_notice(&receiver, Duration::from_secs(2)).await;
    receiver.go_silent();
    let ended = status_until(&player, |s| s.state == CastState::Ended).await;
    assert!(matches!(ended.failure, Some(CastFailure::Lost(_))), "{:?}", ended.failure);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stall_kaeris_receiver_reports_mid_episode_ends_the_cast_as_unplayable() {
    let (receiver, player, mut events) = cast_with_own_receiver(CustomReceiver::Runs).await;
    // The receiver waited at a playhead nothing would fill and gave up: it says so.
    receiver.say(serde_json::json!({
        "type": "error",
        "version": 1,
        "part": "unknown",
        "detail": "unpacking: stalled at 4.50 s, read to 33.00 s, video 0.00-33.00, audio 0.00-4.52",
    }));
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(player.failure(), Some(CastFailure::Refused));
    assert!(player.is_alive(), "the receiver stays up for the next play");
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_media_error_mid_episode_with_no_report_ends_the_cast_as_unplayable() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) =
        connect_standard(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(player.failure(), None);
    receiver.fail_media();
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(player.failure(), Some(CastFailure::Refused));
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
    // The file's own ASS track is the receiver's to read: listed with where it is in the file,
    // a stand-in WebVTT track with its id, and a line on the phone's list.
    let subtitles = kaeri["subtitles"].as_array().unwrap();
    assert_eq!(subtitles.len(), 1, "{kaeri}");
    assert_eq!(subtitles[0]["inside"], 0);
    assert_eq!(subtitles[0]["format"], "ass");
    assert_eq!(subtitles[0]["id"], 1);
    assert!(subtitles[0]["url"].is_null(), "{kaeri}");
    assert_eq!(kaeri["subtitle"], serde_json::Value::Null);
    assert_eq!(media["tracks"][0]["trackId"], 1, "{media}");
    let listed = player.status().borrow().subtitles.clone();
    assert_eq!(listed.iter().map(|t| t.id).collect::<Vec<_>>(), [1]);
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
        Some(CastFailure::Uncastable(Uncastable::VideoCodec("HEVC".into())))
    );
    // No device converts video, and the standard receiver wouldn't play it either.
    assert_eq!(receiver.heard("LOAD").len(), 1);
    assert_eq!(launched(&receiver), ["0A1B2C3D"]);
    player.quit().await.unwrap();
}

#[test]
fn the_files_own_subtitle_tracks_are_read_as_the_receiver_shows_them() {
    assert_eq!(super::inside_format("S_TEXT/ASS"), Some("ass"));
    assert_eq!(super::inside_format("S_TEXT/UTF8"), Some("srt"));
    assert_eq!(super::inside_format("S_HDMV/PGS"), Some("pgs"));
    assert_eq!(super::inside_format("S_VOBSUB"), Some("vobsub"));
    assert_eq!(super::inside_format("S_DVBSUB"), None);
}

#[test]
fn a_codec_kaeris_receiver_names_reads_as_the_notice_names_it() {
    let plan = CastPlan {
        route: CastRoute::Relayed { remux: false },
        video: crate::formats::VideoFormat::other_from_matroska("V_MPEG2"),
        height: Some(720),
        audio: AudioPlan::Copy(crate::formats::AudioCodec::Dts),
        delivery: Delivery::Mkv,
    };
    // The load's own word comes back as the part's name; a codec string's base as the common one.
    assert_eq!(super::named_codec("video:mpeg-2", Some(&plan), true), "MPEG-2");
    assert_eq!(super::named_codec("video:mpeg2", None, true), "MPEG-2");
    assert_eq!(super::named_codec("dts", Some(&plan), false), "DTS");
    assert_eq!(super::named_codec("hev1", Some(&plan), true), "HEVC");
    assert_eq!(super::named_codec("ec-3", None, false), "E-AC-3");
    assert_eq!(super::named_codec("video:vc1", None, true), "VC1");
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
        Some(CastFailure::Uncastable(Uncastable::AudioCodec("Opus".into())))
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

/// A second sender connects to a TV another one casts to, and launching the receiver joins the
/// one that runs: its controls drive what the other sender plays there.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_receiver_another_sender_plays_on_takes_this_ones_controls() {
    let receiver = FakeReceiver::start(FakeOptions {
        duration: 600.0,
        remembers: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (theirs, _events) = playing(&receiver).await;
    let session = theirs.receiver().unwrap().session_id;
    let launches = receiver.heard("LAUNCH").len();
    // The fake serves one sender at a time: the other one's connection goes as this one comes.
    drop(theirs);

    let (player, _events) = connect_standard(setup(
        &receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(FakeEncoder::default()),
    ))
    .await
    .unwrap();
    player.launch().await.unwrap();
    assert_eq!(player.receiver().map(|app| app.session_id), Some(session), "the same receiver");
    let mut status = player.status();
    let now = tokio::time::timeout(
        Duration::from_secs(5),
        status.wait_for(|s| s.state == CastState::Playing),
    )
    .await
    .expect("the receiver says what plays")
    .unwrap()
    .clone();
    assert_eq!(now.metadata.map(|m| m.title).as_deref(), Some("Episode 1"));
    assert_eq!(receiver.heard("LAUNCH").len(), launches + 1);
    assert_eq!(receiver.heard("LOAD").len(), 1, "nothing loaded again");

    player.set_paused(true).await.unwrap();
    receiver_heard_soon(&receiver, "PAUSE", 1).await;
    player.seek(300.0, true).await.unwrap();
    let seek = receiver_heard_soon(&receiver, "SEEK", 1).await;
    assert_eq!(seek[0]["currentTime"].as_f64(), Some(300.0));
    player.stop().await.unwrap();
    let stops = receiver.heard("STOP");
    assert!(stops.iter().any(|m| m["mediaSessionId"].is_i64()), "{stops:?}");
}

/// The sender that started what a joined receiver plays stops casting: the cast this sender
/// joined ends at once as stopped there, with no wait on the TV and nothing read as starting
/// between.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_joined_cast_ends_at_once_when_the_sender_that_started_it_stops_casting() {
    let receiver = FakeReceiver::start(FakeOptions {
        duration: 600.0,
        remembers: true,
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (theirs, _events) = playing(&receiver).await;
    drop(theirs);
    let (player, _events) = connect_standard(setup(
        &receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(FakeEncoder::default()),
    ))
    .await
    .unwrap();
    player.launch().await.unwrap();
    let mut status = player.status();
    tokio::time::timeout(
        Duration::from_secs(5),
        status.wait_for(|s| s.state == CastState::Playing),
    )
    .await
    .expect("the receiver says what plays")
    .unwrap();

    receiver.stop_elsewhere();
    let mut seen = Vec::new();
    let ended = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let now = status.borrow_and_update().clone();
            if now.state == CastState::Ended {
                return now;
            }
            seen.push((now.state, now.waiting_since.is_some()));
            status.changed().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| panic!("ended at once, not after {seen:?}"));
    assert_eq!(ended.failure, Some(CastFailure::StoppedElsewhere));
    assert!(!seen.iter().any(|(_, waiting)| *waiting), "never waits on the TV: {seen:?}");
}

fn card_of(episode: &str) -> JoinCard {
    JoinCard {
        id: 900_002,
        episode: episode.into(),
        title: "A Made-up Show".into(),
        episode_title: None,
    }
}

/// A load says its card beside the media, and the TV says it back with the media's status: on
/// the Default Media Receiver and on Kaeri's own.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_load_carries_its_card_and_the_tv_says_it_back() {
    for standard in [true, false] {
        let file = matroska(&Fixture::default());
        let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
        let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
        let setup =
            setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(FakeEncoder::default()));
        let (player, mut events) = if standard {
            connect_standard(setup).await.unwrap()
        } else {
            CastPlayer::connect(setup).await.unwrap()
        };
        let load =
            CastLoad { card: Some(card_of("1")), ..torrent(format!("{base}/episode.mkv")) };
        player.load(load);
        next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
        let loads = receiver.heard("LOAD");
        assert_eq!(
            loads[0]["media"]["customData"]["kaeriCard"],
            json!({"id": 900_002, "episode": "1", "title": "A Made-up Show"}),
            "standard: {standard}"
        );
        assert_eq!(
            loads[0]["media"]["customData"]["kaeri"].is_object(),
            !standard,
            "Kaeri's part only for Kaeri's receiver"
        );
        assert_eq!(player.current().card, Some(card_of("1")), "standard: {standard}");
        player.quit().await.unwrap();
    }
}

/// A phone joined to the cast another device started on a TV that keeps its receiver: the other
/// device's load of episode 1, with or without its card, and the phone's player that joined it.
async fn joined_to(receiver: &FakeReceiver, card: Option<JoinCard>) -> CastPlayer {
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let (theirs, mut events) = connect_standard(setup(
        receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(FakeEncoder::default()),
    ))
    .await
    .unwrap();
    theirs.load(CastLoad { card, ..torrent(format!("{base}/episode.mkv")) });
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    // The fake serves one sender at a time: the other one's connection goes as this one comes.
    drop(theirs);
    let (player, _events) = connect_standard(setup(
        receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(FakeEncoder::default()),
    ))
    .await
    .unwrap();
    player.launch().await.unwrap();
    status_until(&player, |s| s.state == CastState::Playing).await;
    player
}

fn tv_that_keeps_its_receiver() -> FakeOptions {
    FakeOptions { duration: 600.0, remembers: true, ..FakeOptions::default() }
}

/// The other Kaeri device casts its next episode to the TV: the phone that joined follows it,
/// with the new card, and its controls go to the new episode.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_joined_player_follows_the_next_episode_another_kaeri_device_casts() {
    let receiver = FakeReceiver::start(tv_that_keeps_its_receiver()).await.unwrap();
    let player = joined_to(&receiver, Some(card_of("1"))).await;
    assert_eq!(player.current().card, Some(card_of("1")));

    receiver.load_elsewhere_with(json!({"kaeriCard": card_of("2")}));
    let now = status_until(&player, |s| {
        s.card.as_ref().is_some_and(|c| c.episode == "2") && s.state == CastState::Playing
    })
    .await;
    assert_eq!(now.failure, None);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(player.is_alive(), "never taken over: {:?}", player.failure());

    player.set_paused(true).await.unwrap();
    let pauses = receiver_heard_soon(&receiver, "PAUSE", 1).await;
    assert_eq!(pauses[0]["mediaSessionId"], 2, "the controls go to the new episode");
}

/// Another app's load on the TV, or a Kaeri without the card, still takes it from a phone
/// that joined.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_joined_player_is_taken_over_by_a_load_without_a_card() {
    let receiver = FakeReceiver::start(tv_that_keeps_its_receiver()).await.unwrap();
    let player = joined_to(&receiver, Some(card_of("1"))).await;
    receiver.load_elsewhere();
    status_until(&player, |s| s.state == CastState::Ended).await;
    assert_eq!(player.failure(), Some(CastFailure::TakenOver));
}

/// The phone's own cast is taken over by another Kaeri device's load as before: only a phone
/// that joined follows.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cast_of_this_device_is_taken_over_by_another_kaeri_devices_load() {
    let receiver = FakeReceiver::start(tv_that_keeps_its_receiver()).await.unwrap();
    let (player, _events) = playing(&receiver).await;
    receiver.load_elsewhere_with(json!({"kaeriCard": card_of("2")}));
    status_until(&player, |s| s.state == CastState::Ended).await;
    assert_eq!(player.failure(), Some(CastFailure::TakenOver));
}

/// A cast loaded without a card, by another app or an older Kaeri: the phone that joins it
/// drives it and reads no card.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_joined_cast_without_a_card_has_none_and_takes_the_controls() {
    let receiver = FakeReceiver::start(tv_that_keeps_its_receiver()).await.unwrap();
    let player = joined_to(&receiver, None).await;
    assert_eq!(player.current().card, None);
    player.set_paused(true).await.unwrap();
    receiver_heard_soon(&receiver, "PAUSE", 1).await;
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
