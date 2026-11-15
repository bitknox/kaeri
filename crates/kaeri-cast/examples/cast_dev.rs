//! A test sender for Kaeri's Cast receiver: casts one file or URL to a TV through the same
//! `CastPlayer` the engine drives for the apps (relay, plans, the LOAD with Kaeri's custom data
//! and the receiver's namespace), and prints what the receiver says.
//!
//! ```text
//! KAERI_CAST_RECEIVER_APP=<dev app ID> cargo run -p kaeri-cast --example cast_dev -- <file or URL>
//!     [--tv "Chromecast TV"] [--ip 192.168.0.19] [--sub <file>] [--volume 0.05]
//! KAERI_CAST_RECEIVER_APP=<dev app ID> cargo run -p kaeri-cast --example cast_dev -- --matrix
//!     [tools/cast-dev/media/matrix.tsv] [--seconds 8] [--out <table.md>]
//!     [--media-base http://127.0.0.1:8787/media/]
//! ```
//!
//! Keys while one file plays: space pause/play, left and right seek 10 s, `a` next audio track,
//! `s` next subtitle track, `k` tell the receiver an opening was skipped, `q` stop and quit.
//!
//! Safety: it casts to one TV only. The TV at `--ip` must answer to the name `--tv` (default
//! "Chromecast TV"); a Nest device, a speaker group or anything named "Stue" is refused. The
//! TV's volume is set low (`--volume`, or muted where the level is fixed) and put back at the
//! end. Nothing is written outside the media folder and `--out`: what the TV refused is kept in
//! memory only. Run it under the HOME guard all the same.

use std::{
    io::Write,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use kaeri_cast::{
    CastDevice, CastLoad, CastMemory, CastPlayer, CastRoute, CastSetup, CastSource, CastStatus,
    CastSubtitle, Metadata, NoEncoder, PlainHttp, SourceKind, Timings, Upstream, VolumeControl,
    player::FromReceiver,
};
use kaeri_playback::PlaybackEvent;
use tokio::sync::{broadcast, mpsc};

const ENV: &str = "KAERI_CAST_RECEIVER_APP";

macro_rules! say {
    ($($arg:tt)*) => {{
        let mut out = std::io::stdout().lock();
        let _ = write!(out, "{}\r\n", format_args!($($arg)*));
        let _ = out.flush();
    }};
}

#[derive(Debug)]
struct Args {
    target: Option<String>,
    tv: String,
    ip: Ipv4Addr,
    app: String,
    sub: Option<PathBuf>,
    volume: f64,
    matrix: Option<PathBuf>,
    seconds: u64,
    out: Option<PathBuf>,
    media_base: String,
    /// Run the matrix on whatever receiver launched: a check of the runner itself.
    any_receiver: bool,
}

fn args() -> Result<Args, String> {
    let mut args = Args {
        target: None,
        tv: "Chromecast TV".into(),
        ip: Ipv4Addr::new(192, 168, 0, 19),
        app: std::env::var(ENV).unwrap_or_default(),
        sub: None,
        volume: 0.05,
        matrix: None,
        seconds: 8,
        out: None,
        media_base: "http://127.0.0.1:8787/media/".into(),
        any_receiver: false,
    };
    let mut it = std::env::args().skip(1).peekable();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--tv" => args.tv = value("--tv")?,
            "--ip" => args.ip = value("--ip")?.parse().map_err(|e| format!("--ip: {e}"))?,
            "--app" => args.app = value("--app")?,
            "--sub" => args.sub = Some(value("--sub")?.into()),
            "--volume" => {
                args.volume =
                    value("--volume")?.parse().map_err(|e| format!("--volume: {e}"))?;
            }
            "--seconds" => {
                args.seconds =
                    value("--seconds")?.parse().map_err(|e| format!("--seconds: {e}"))?;
            }
            "--out" => args.out = Some(value("--out")?.into()),
            "--media-base" => args.media_base = value("--media-base")?,
            "--any-receiver" => args.any_receiver = true,
            "--matrix" => {
                let next = it.peek().filter(|n| !n.starts_with("--")).cloned();
                if next.is_some() {
                    it.next();
                }
                args.matrix = Some(
                    next.map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("tools/cast-dev/media/matrix.tsv")),
                );
            }
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with("--") => return Err(format!("unknown flag {other}")),
            other => args.target = Some(other.to_owned()),
        }
    }
    let app = args.app.trim().to_ascii_uppercase();
    if app.len() != 8 || !app.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("set {ENV} (or --app) to the dev receiver's app ID"));
    }
    args.app = app;
    if !args.media_base.ends_with('/') {
        args.media_base.push('/');
    }
    if !(0.0..=0.3).contains(&args.volume) {
        return Err("--volume stays between 0 and 0.3".into());
    }
    if args.matrix.is_none() && args.target.is_none() {
        return Err("name a file or URL, or --matrix".into());
    }
    Ok(args)
}

/// The TV at `ip`, when it answers to `name` and is no device this tool must leave alone.
async fn the_tv(
    upstream: &dyn Upstream,
    ip: Ipv4Addr,
    name: &str,
) -> Result<CastDevice, String> {
    let refused = |n: &str| {
        let lower = n.to_lowercase();
        lower.contains("nest") || lower.trim() == "stue" || lower.contains("group")
    };
    if refused(name) {
        return Err(format!("{name:?} is a device this tool never casts to"));
    }
    let url = format!("http://{ip}:8008/setup/eureka_info?params=name,device_info");
    let info = async {
        let response = upstream.get(&url, &[]).await.ok()?;
        let body = response.bytes(256 * 1024).await.ok()?;
        serde_json::from_slice::<serde_json::Value>(&body).ok()
    };
    let info = tokio::time::timeout(Duration::from_secs(4), info)
        .await
        .ok()
        .flatten()
        .ok_or(format!("no Cast device answered at {ip}"))?;
    let found = info["name"].as_str().unwrap_or_default().to_owned();
    let model = info["device_info"]["model_name"].as_str().unwrap_or("Chromecast").to_owned();
    if refused(&found) || refused(&model) {
        return Err(format!("{ip} is {found:?} ({model}), a device this tool never casts to"));
    }
    if found != name {
        return Err(format!("{ip} answers to {found:?}, not {name:?}"));
    }
    Ok(CastDevice {
        instance: found.clone(),
        id: format!("cast-dev-{ip}"),
        name: found,
        model,
        address: SocketAddr::new(IpAddr::V4(ip), 8009),
        video: true,
        group: false,
    })
}

fn load_of(target: &str, sub: Option<&Path>) -> CastLoad {
    let remote = target.contains("://");
    let kind = match () {
        _ if remote && (target.contains(".m3u8") || target.contains(".mpd")) => SourceKind::Hls,
        _ if remote => SourceKind::Progressive,
        _ => SourceKind::File,
    };
    let url = if remote {
        target.to_owned()
    } else {
        std::fs::canonicalize(target)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| target.to_owned())
    };
    let name = Path::new(target).file_name().map(|n| n.to_string_lossy().into_owned());
    let subtitles: Vec<CastSubtitle> = sub
        .map(|path| CastSubtitle {
            path: std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned()),
            title: Some("Test".into()),
            language: Some("en".into()),
        })
        .into_iter()
        .collect();
    CastLoad {
        source: CastSource { url: url.clone(), kind, headers: Vec::new() },
        url,
        headers: Vec::new(),
        start: None,
        metadata: Metadata {
            series: "Cast test".into(),
            title: name.unwrap_or_else(|| "Test clip".into()),
            image: None,
        },
        subtitle: (!subtitles.is_empty()).then_some(0),
        subtitles,
        audio_language: None,
        audio_track: None,
        audio_memory: None,
    }
}

fn heard_line(heard: &FromReceiver) -> String {
    match heard {
        FromReceiver::SubtitlesPlain => "subtitles-plain".into(),
        FromReceiver::UpNext { play } => {
            format!("up-next {}", if *play { "play" } else { "cancel" })
        }
        FromReceiver::SenderGone { waited } => format!("sender-gone waited={waited:.0}s"),
        FromReceiver::Error(e) => {
            let mut line = format!("error part={}", e.part);
            if let Some(codec) = &e.codec {
                line += &format!(" codec={codec}");
            }
            if let Some(fetched) = &e.fetched {
                line += &format!(" fetched={fetched}");
            }
            if let Some(status) = e.status {
                line += &format!(" status={status}");
            }
            if e.lost {
                line += " lost";
            }
            if let Some(detailed) = e.detailed {
                line += &format!(" caf={detailed}");
            }
            if let Some(shaka) = e.shaka {
                line += &format!(" shaka={shaka}");
            }
            if let Some(detail) = &e.detail {
                line += &format!(" detail={detail:?}");
            }
            line
        }
    }
}

fn plan_line(status: &CastStatus) -> String {
    let Some(plan) = &status.plan else { return "-".into() };
    let route = match &plan.route {
        CastRoute::Direct { .. } => "direct",
        CastRoute::Relayed { remux: true } => "relay+remux",
        CastRoute::Relayed { remux: false } => "relay",
    };
    format!("{route} {:?} {:?} {:?}", plan.delivery, plan.video, plan.audio)
}

fn status_line(s: &CastStatus) -> String {
    let receiver = s.receiver.as_deref().unwrap_or("-");
    let mut line = format!(
        "status {:?} {:.1}/{} receiver={receiver}",
        s.state,
        s.position,
        s.duration.map_or("?".into(), |d| format!("{d:.1}")),
    );
    if let Some(step) = s.step {
        line += &format!(" step={step:?}");
    }
    if let Some(failure) = &s.failure {
        line += &format!(" failure=\"{failure}\"");
    }
    line
}

/// The volume as it was, put back by [`restore`].
#[derive(Clone, Copy)]
struct Volume {
    level: f64,
    muted: bool,
    fixed: bool,
}

async fn quiet(player: &CastPlayer, level: f64) -> Volume {
    let was = Volume {
        level: player.volume() / 100.0,
        muted: player.muted(),
        fixed: player.volume_control() == VolumeControl::MuteOnly,
    };
    let done = if was.fixed {
        player.set_muted(true).await
    } else {
        player.set_volume(level * 100.0).await
    };
    match done {
        Ok(()) if was.fixed => say!("volume is fixed on this TV: muted for the test"),
        Ok(()) => say!("volume {:.2} -> {level:.2} for the test", was.level),
        Err(e) => say!("couldn't lower the volume: {e}"),
    }
    was
}

async fn restore(player: &CastPlayer, was: Volume) {
    let done = if was.fixed {
        player.set_muted(was.muted).await
    } else {
        player.set_volume(was.level * 100.0).await
    };
    match done {
        Ok(()) if was.fixed => say!("volume is fixed on this TV: mute put back"),
        Ok(()) => say!("volume put back to {:.2}", was.level),
        Err(e) => say!("couldn't put the volume back ({:.2}): {e}", was.level),
    }
}

async fn interactive(player: &CastPlayer, load: CastLoad) {
    let mut events = player.events();
    let mut heard = player.heard();
    let mut status = player.status();
    let mut keys = EventStream::new();
    let raw = crossterm::terminal::enable_raw_mode().is_ok();
    say!(
        "keys: space pause/play, left/right seek 10 s, a audio, s subtitles, k skipped, q quit"
    );
    player.load(load);
    let mut last = String::new();
    let mut last_position = -10.0;
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(PlaybackEvent::Loaded) => say!("loaded: {}", plan_line(&player.current())),
                Some(PlaybackEvent::Ended { complete }) => {
                    say!("ended complete={complete} failure={:?}", player.failure());
                }
                Some(PlaybackEvent::Buffering(b)) => say!("buffering {b}"),
                Some(_) => {}
                None => { say!("the cast ended"); break; }
            },
            message = heard.recv() => match message {
                Ok(message) => say!("receiver: {}", heard_line(&message)),
                Err(broadcast::error::RecvError::Lagged(n)) => say!("missed {n} receiver messages"),
                Err(_) => break,
            },
            changed = status.changed() => {
                if changed.is_err() { break; }
                let s = status.borrow_and_update().clone();
                let line = status_line(&s);
                let head = line.split(' ').take(2).collect::<String>() + &line[line.find(" receiver").unwrap_or(0)..];
                if head != last || (s.position - last_position).abs() >= 5.0 {
                    say!("{line}");
                    last = head;
                    last_position = s.position;
                }
            }
            key = keys.next() => {
                let Some(Ok(Event::Key(KeyEvent { code, modifiers, kind: KeyEventKind::Press, .. }))) = key else {
                    if key.is_none() { break; }
                    continue;
                };
                let result = match code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char(' ') => player.toggle_pause().await,
                    KeyCode::Left => player.seek(-10.0, false).await,
                    KeyCode::Right => player.seek(10.0, false).await,
                    KeyCode::Char('a') => {
                        let tracks = player.current().audio;
                        match tracks.iter().position(|t| t.selected) {
                            _ if tracks.len() < 2 => say!("one audio track only"),
                            Some(i) => {
                                let next = &tracks[(i + 1) % tracks.len()];
                                say!("audio -> {} {:?}", next.index, next.language);
                                player.set_audio(next.index);
                            }
                            None => player.set_audio(tracks[0].index),
                        }
                        Ok(())
                    }
                    KeyCode::Char('s') => {
                        let texts = player.current().subtitles;
                        let on = texts.iter().position(|t| t.selected);
                        let next = match on {
                            None => texts.first().map(|t| t.id),
                            Some(i) => texts.get(i + 1).map(|t| t.id),
                        };
                        say!("subtitle -> {next:?} (of {})", texts.len());
                        player.set_subtitle(next).await
                    }
                    KeyCode::Char('k') => {
                        player.tell_skipped("opening");
                        say!("sent skipped opening");
                        Ok(())
                    }
                    _ => Ok(()),
                };
                if let Err(e) = result {
                    say!("command failed: {e}");
                }
            }
        }
    }
    if raw {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

struct Entry {
    id: String,
    how: String,
    path: String,
    sidecar: Option<String>,
    what: String,
}

fn read_matrix(path: &Path) -> Result<Vec<Entry>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("{}: {e} (run tools/cast-dev/media.sh)", path.display()))?;
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            (f.len() >= 5).then(|| Entry {
                id: f[0].into(),
                how: f[1].into(),
                path: f[2].into(),
                sidecar: (f[3] != "-").then(|| f[3].into()),
                what: f[4].into(),
            })
        })
        .collect())
}

struct Row {
    id: String,
    what: String,
    plays: &'static str,
    part: String,
    messages: String,
    plan: String,
}

async fn play_one(player: &CastPlayer, load: CastLoad, seconds: u64) -> (Row, CastStatus) {
    let mut events = player.events();
    let mut heard = player.heard();
    player.load(load);
    let started = Instant::now();
    let mut messages: Vec<String> = Vec::new();
    let mut loaded_at: Option<(Instant, f64)> = None;
    let mut ended = false;
    let deadline = Duration::from_secs(45);
    loop {
        let left = match loaded_at {
            Some((at, _)) => Duration::from_secs(seconds).saturating_sub(at.elapsed()),
            None => deadline.saturating_sub(started.elapsed()),
        };
        if left.is_zero() || ended {
            break;
        }
        tokio::select! {
            event = events.recv() => match event {
                Some(PlaybackEvent::Loaded) => {
                    say!("  loaded after {:.1}s: {}", started.elapsed().as_secs_f64(), plan_line(&player.current()));
                    loaded_at = Some((Instant::now(), player.current().position));
                }
                Some(PlaybackEvent::Ended { .. }) | None => ended = true,
                Some(_) => {}
            },
            message = heard.recv() => if let Ok(message) = message {
                let line = heard_line(&message);
                say!("  receiver: {line}");
                messages.push(line);
            },
            () = tokio::time::sleep(left) => {}
        }
    }
    // Late messages of the receiver about what just played.
    let grace = Instant::now();
    while grace.elapsed() < Duration::from_millis(700) {
        match tokio::time::timeout(Duration::from_millis(700), heard.recv()).await {
            Ok(Ok(message)) => {
                let line = heard_line(&message);
                say!("  receiver: {line}");
                messages.push(line);
            }
            _ => break,
        }
    }
    let status = player.current();
    let moved = loaded_at.map(|(_, from)| status.position - from).unwrap_or(0.0);
    let errors: Vec<&String> = messages.iter().filter(|m| m.starts_with("error")).collect();
    let stopped = errors.iter().any(|m| !m.starts_with("error part=subtitles"));
    let plays = match () {
        _ if loaded_at.is_none() => "no",
        _ if stopped || status.failure.is_some() => "no",
        _ if moved < (seconds as f64) * 0.6 => "stalls",
        _ if !errors.is_empty() => "yes, subs failed",
        _ => "yes",
    };
    let part = errors
        .first()
        .and_then(|m| m.split_whitespace().find_map(|w| w.strip_prefix("part=")))
        .map(str::to_owned)
        .or_else(|| status.failure.as_ref().map(|f| format!("({f})")))
        .unwrap_or_else(|| "-".into());
    let row = Row {
        id: String::new(),
        what: String::new(),
        plays,
        part,
        messages: if messages.is_empty() { "-".into() } else { messages.join("; ") },
        plan: plan_line(&status),
    };
    (row, status)
}

async fn matrix(player: &CastPlayer, args: &Args, tsv: &Path) -> Result<String, String> {
    let entries = read_matrix(tsv)?;
    let dir = tsv.parent().unwrap_or(Path::new("."));
    let mut rows = Vec::new();
    for (n, entry) in entries.iter().enumerate() {
        say!("[{}/{}] {} ({})", n + 1, entries.len(), entry.id, entry.what);
        let target = if entry.how == "url" {
            format!("{}{}", args.media_base, entry.path)
        } else {
            dir.join(&entry.path).to_string_lossy().into_owned()
        };
        let sidecar = entry.sidecar.as_ref().map(|s| dir.join(s));
        let (mut row, _) =
            play_one(player, load_of(&target, sidecar.as_deref()), args.seconds).await;
        say!("  -> {} part={} plan={}", row.plays, row.part, row.plan);
        row.id = entry.id.clone();
        row.what = entry.what.clone();
        rows.push(row);
        if let Err(e) = player.stop().await {
            say!("  stop failed: {e}");
        }
        if !player.is_alive() {
            return Err("the cast ended: the TV went away".into());
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
    }
    let mut table = String::from(
        "| Clip | What | Plays | Part failed | Receiver said | Plan |\n| --- | --- | --- | --- | --- | --- |\n",
    );
    for r in &rows {
        let cell = |s: &str| s.replace('|', "/");
        table += &format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            r.id,
            cell(&r.what),
            r.plays,
            cell(&r.part),
            cell(&r.messages),
            cell(&r.plan)
        );
    }
    let plays = rows.iter().filter(|r| r.plays.starts_with("yes")).count();
    table += &format!("\n{plays} of {} play for {} s.\n", rows.len(), args.seconds);
    Ok(table)
}

#[tokio::main]
async fn main() {
    let args = match args() {
        Ok(args) => args,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("cast_dev: {e}");
            }
            eprintln!(
                "usage: {ENV}=<app ID> cast_dev <file or URL> [--sub <file>] | --matrix [<matrix.tsv>] [--out <file>]"
            );
            std::process::exit(2);
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kaeri_cast=info".into()),
        )
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .init();

    let upstream: Arc<dyn Upstream> = Arc::new(PlainHttp);
    let device = match the_tv(upstream.as_ref(), args.ip, &args.tv).await {
        Ok(device) => device,
        Err(e) => {
            eprintln!("cast_dev: {e}");
            std::process::exit(1);
        }
    };
    say!(
        "TV: {} ({}) at {}; receiver app {}",
        device.name,
        device.model,
        device.address,
        args.app
    );
    let setup = CastSetup {
        device,
        memory: Arc::new(CastMemory::memory()),
        upstream,
        encoder: Arc::new(NoEncoder),
        timings: Timings::default(),
    };
    let (player, _events): (CastPlayer, mpsc::UnboundedReceiver<PlaybackEvent>) =
        match CastPlayer::connect(setup).await {
            Ok(connected) => connected,
            Err(e) => {
                eprintln!("cast_dev: couldn't connect: {e}");
                std::process::exit(1);
            }
        };
    player.use_receiver(Some(args.app.clone()));
    player.set_greeting(Some("en".into()), Some("mac".into()));
    let was = quiet(&player, args.volume).await;
    if let Err(e) = player.launch().await {
        say!("the receiver didn't launch: {e}");
    }
    let app = player.receiver().map(|a| a.app_id).unwrap_or_default();
    say!(
        "running receiver: {app}{}",
        if app == args.app { "" } else { " (not the dev receiver)" }
    );

    let mut code = 0;
    if args.matrix.is_some() && app != args.app && !args.any_receiver {
        // A table of the Default Media Receiver would read as the dev receiver's.
        say!(
            "matrix not run: {} isn't running (an unpublished app launches only on a test device, after a restart)",
            args.app
        );
        code = 1;
    } else if let Some(tsv) = &args.matrix {
        match matrix(&player, &args, tsv).await {
            Ok(table) => {
                let header = format!(
                    "# Cast receiver matrix\n\nTV: {} at {}. Receiver app {app}. Each clip plays {} s.\n\n",
                    args.tv, args.ip, args.seconds
                );
                let text = header + &table;
                match &args.out {
                    Some(out) => match std::fs::write(out, &text) {
                        Ok(()) => say!("table written to {}", out.display()),
                        Err(e) => {
                            say!("couldn't write {}: {e}", out.display());
                            code = 1;
                        }
                    },
                    None => say!("{text}"),
                }
            }
            Err(e) => {
                say!("matrix stopped: {e}");
                code = 1;
            }
        }
    } else if let Some(target) = &args.target {
        interactive(&player, load_of(target, args.sub.as_deref())).await;
    }
    let _ = player.stop().await;
    restore(&player, was).await;
    let _ = player.quit().await;
    std::process::exit(code);
}
