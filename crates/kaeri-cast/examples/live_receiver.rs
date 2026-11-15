//! Cast local files to one real TV through Kaeri's receiver and print what happened: the
//! receiver that ran, the plan, what the receiver said on Kaeri's namespace, and the failure.
//! A live probe: it reaches a real TV. Run it under the HOME guard, against a TV you may use,
//! with synthetic media only.
//!
//! `cargo run -p kaeri-cast --example live_receiver -- <tv-ip> [receiver-id] <file.mkv>...`
//!
//! `--status` prints the app the TV runs; `--launch` launches the receiver alone, says how the
//! TV answered and closes it. Before each cast, a TV already running a Kaeri receiver or the
//! Default Media Receiver is left alone.
//!
//! A file may name a side-loaded subtitle after a comma: `episode.mkv,episode.ass`. The TV is
//! muted for the run and put back as it was.

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use kaeri_cast::{
    CastDevice, CastMemory, CastPlayer, CastSetup, CastState, PlainHttp, Timings,
    aac::NoEncoder,
    client::{DEFAULT_MEDIA_RECEIVER, Device, KAERI_RECEIVER, Metadata},
    formats::{CastSource, SourceKind},
    player::{CastLoad, CastSubtitle, UpNextCard},
};

#[tokio::main]
async fn main() {
    // `RUST_LOG=kaeri_cast=debug` shows the relay's requests and the player's steps, timed.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_target(false)
        .init();
    let mut args = std::env::args().skip(1);
    let ip: IpAddr = args.next().expect("the TV's address").parse().expect("an IP address");
    let mut rest: Vec<String> = args.collect();
    let receiver = rest
        .first()
        .filter(|a| a.len() == 8 && a.chars().all(|c| c.is_ascii_hexdigit()))
        .cloned();
    if receiver.is_some() {
        rest.remove(0);
    }
    let address = SocketAddr::new(ip, 8009);
    if rest.first().map(String::as_str) == Some("--status") {
        println!("running: {:?}", running(address).await);
        return;
    }
    if rest.first().map(String::as_str) == Some("--launch") {
        let id = receiver.unwrap_or_else(|| KAERI_RECEIVER.unwrap_or_default().to_owned());
        let (device, _incoming) =
            Device::connect(address, Duration::from_secs(15)).await.unwrap();
        match device.launch(&id).await {
            Ok(app) => {
                println!("launched {} session {}", app.app_id, app.session_id);
                tokio::time::sleep(Duration::from_secs(3)).await;
                let _ = device.stop(&app).await;
            }
            Err(e) => println!("launch of {id} failed: {e}"),
        }
        return;
    }
    for file in rest {
        if let Some(app) = running(address).await.filter(|app| BUSY.contains(&app.as_str())) {
            println!("\n=== {file}: the TV runs {app}; left alone");
            return;
        }
        let (media, subtitle) = match file.split_once(',') {
            Some((media, subtitle)) => (media.to_owned(), Some(PathBuf::from(subtitle))),
            None => (file.clone(), None),
        };
        cast_one(ip, receiver.clone(), &media, subtitle).await;
    }
}

/// Apps that mean another sender is casting.
const BUSY: [&str; 3] = ["A7A6DDE4", "BA36AC09", DEFAULT_MEDIA_RECEIVER];

/// The app the TV runs in the foreground, other than its idle screen.
async fn running(address: SocketAddr) -> Option<String> {
    let (device, _incoming) = Device::connect(address, Duration::from_secs(15)).await.ok()?;
    device.status().await.ok()?.app.map(|app| app.app_id)
}

async fn cast_one(
    ip: IpAddr,
    receiver: Option<String>,
    media: &str,
    subtitle: Option<PathBuf>,
) {
    println!("\n=== {media}");
    let setup = CastSetup {
        device: CastDevice {
            instance: "Chromecast-live".into(),
            id: "live-tv".into(),
            name: "TV".into(),
            model: "Chromecast".into(),
            address: SocketAddr::new(ip, 8009),
            video: true,
            group: false,
        },
        memory: Arc::new(CastMemory::memory()),
        upstream: Arc::new(PlainHttp),
        encoder: Arc::new(NoEncoder),
        timings: Timings::default(),
    };
    let (player, mut events) = match CastPlayer::connect(setup).await {
        Ok(connected) => connected,
        Err(e) => {
            println!("connect failed: {e}");
            return;
        }
    };
    tokio::spawn(async move { while events.recv().await.is_some() {} });
    if let Some(receiver) = receiver {
        player.use_receiver(Some(receiver));
    }
    let mut heard = player.heard();
    let started = Instant::now();
    tokio::spawn(async move {
        while let Ok(message) = heard.recv().await {
            println!("  [{:>5.1}s] heard {message:?}", started.elapsed().as_secs_f64());
        }
    });
    if let Err(e) = player.launch().await {
        println!("launch failed: {e}");
        return;
    }
    println!("  [{:>5.1}s] launched", started.elapsed().as_secs_f64());
    // Each change of the load's step and the TV's state, timed from the launch's start.
    let mut status = player.status();
    tokio::spawn(async move {
        let mut last = None;
        let mut moved = false;
        while status.changed().await.is_ok() {
            let now = status.borrow().clone();
            let seen = (now.step, now.state);
            if last != Some(seen) || (!moved && now.position > 0.0) {
                moved |= now.position > 0.0;
                println!(
                    "  [{:>5.1}s] step={:?} state={:?} position={:.2}",
                    started.elapsed().as_secs_f64(),
                    now.step,
                    now.state,
                    now.position
                );
                last = Some(seen);
            }
        }
    });
    let was_muted = player.current().muted.unwrap_or(false);
    let _ = player.set_muted(true).await;
    player.set_greeting(Some("en".into()), Some("computer".into()));

    let path = std::fs::canonicalize(media).expect("the file");
    let url = format!("file://{}", path.display());
    let subtitles: Vec<CastSubtitle> = subtitle
        .into_iter()
        .map(|path| CastSubtitle {
            path,
            title: Some("Test".into()),
            language: Some("en".into()),
        })
        .collect();
    let subtitle_on = (!subtitles.is_empty()).then_some(0);
    println!("  [{:>5.1}s] load", started.elapsed().as_secs_f64());
    player.load(CastLoad {
        source: CastSource { url: url.clone(), kind: SourceKind::File, headers: Vec::new() },
        url,
        headers: Vec::new(),
        start: None,
        metadata: Metadata {
            series: "A Made-up Show".into(),
            title: "Episode 1".into(),
            image: None,
        },
        subtitles,
        subtitle: subtitle_on,
        audio_language: None,
        audio_track: None,
        audio_memory: None,
    });

    // Up to 40 s for a moving position or a failure.
    let mut played = false;
    let mut first_moving = None;
    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let now = player.current();
        if now.failure.is_some() {
            break;
        }
        if now.state == CastState::Playing && now.position > 1.0 {
            played = true;
            first_moving.get_or_insert(started.elapsed());
            break;
        }
    }
    let now = player.current();
    println!(
        "receiver={:?} played={played} moving_after={first_moving:?} state={:?} position={:.1} duration={:?}",
        now.receiver, now.state, now.position, now.duration
    );
    println!("plan={:?}", now.plan);
    println!("failure={:?}", now.failure);
    println!("audio choices={} subtitles={:?}", now.audio.len(), now.subtitles);

    if played {
        let _ = player.set_paused(true).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!(
            "after pause: {:?} at {:.1}",
            player.current().state,
            player.current().position
        );
        let _ = player.seek(30.0, true).await;
        let _ = player.set_paused(false).await;
        tokio::time::sleep(Duration::from_secs(4)).await;
        println!(
            "after seek 30: {:?} at {:.1}",
            player.current().state,
            player.current().position
        );
        player.tell_skipped("opening");
        player.tell_up_next(Some(UpNextCard {
            number: Some("2".into()),
            title: Some("A made-up next episode".into()),
            still: None,
            seconds: 10,
        }));
        tokio::time::sleep(Duration::from_secs(4)).await;
        player.tell_up_next(None);
        let _ = player.set_subtitle(None).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!(
            "end: {:?} at {:.1} failure={:?}",
            player.current().state,
            player.current().position,
            player.current().failure
        );
    }
    if !was_muted {
        let _ = player.set_muted(false).await;
    }
    let _ = player.quit().await;
    tokio::time::sleep(Duration::from_millis(500)).await;
}
