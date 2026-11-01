//! See the player as Kaeri dresses it, with no sources configured.
//!
//! Plays one local video twice, as episodes 1 and 2 of a binge, through the real playback loop:
//! the bundled interface, the upscaling chain, the Up Next card, and the second episode taking
//! over the first one's window. Your config decides the player (binary, interface, upscaling);
//! history goes to an in-memory store and is never written.
//!
//! ```text
//! cargo run -p kaeri --example player_preview -- path/to/video.mp4 [--still URL]
//! ```
//!
//! Episode 1 starts 45 seconds before its end, so Up Next shows within seconds. `--still` is an
//! image URL for the Up Next card. Use CC-licensed media, such as Big Buck Bunny.

use std::time::Duration;

use kaeri_core::{
    config::{Config, Paths},
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind},
};
use kaeri_engine::playback::{
    self, NextEpisode, PlaySettings, PlaybackContext, slot::PlayerSlot,
};
use kaeri_model::event::Event;
use kaeri_net::HttpClient;
use kaeri_store::Store;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let video = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: player_preview VIDEO [--still URL]"))?;
    let mut still = None;
    while let Some(arg) = args.next() {
        if arg == "--still" {
            still = args.next();
        }
    }
    let video = std::fs::canonicalize(&video)?.to_string_lossy().into_owned();

    // Your config, but the player's bundle, shaders, socket and stills go in a scratch
    // directory removed when this ends, not into the real cache.
    let scratch = tempfile::tempdir()?;
    let paths = Paths { cache_dir: scratch.path().to_path_buf(), ..Paths::resolve()? };
    let config = Config::load(&paths)?;
    let mpv = kaeri_engine::playback::player::player(
        &config,
        &paths,
        &kaeri_tui::theme::player_theme(),
    );
    println!("player scratch dir: {}", mpv.scratch_dir().display());

    let store = Store::open_in_memory()?;
    let http = HttpClient::new(&config.network)?;
    let slot = PlayerSlot::default();
    let start = duration(&video).map(|d| (d - 45.0).max(0.0));

    let episodes = [
        (
            "1",
            Some(NextEpisode {
                number: "2".into(),
                title: Some("The Second Episode".into()),
                thumbnail: still,
            }),
        ),
        ("2", None),
    ];
    for (episode, next) in episodes {
        println!("── episode {episode}");
        let context = PlaybackContext {
            anilist_id: AnilistId::new(1),
            mal_id: None,
            episode: episode.into(),
            title: "Preview".into(),
            translation: Translation::Sub,
            resume_at: start,
            runtime_secs: None,
            speed: None,
            volume: None,
            next,
            episodes: Vec::new(),
            cover_url: None,
            banner_url: None,
            episode_total: None,
        };
        let settings = PlaySettings {
            threshold: config.playback.commit_threshold,
            auto_skip: false,
            auto_skip_recap: false,
            ask_was_opening: false,
            share_skip_times: Some(false),
            mark_chapters: true,
            skip_button: true,
            fullscreen: config.playback.fullscreen,
            subtitle_language: Some(config.playback.subtitle_language.clone()),
            auto_next: true,
            night_mode: config.playback.night_mode,
            remember_tracks: false,
            look: config.playback.upscaling.into(),
            shader_dir: kaeri_engine::playback::player::shader_dir(&config, &paths),
            tone_mapping: config.playback.tone_mapping.clone(),
            tracker_ids: Vec::new(),
            presence: Default::default(),
            reports: Default::default(),
            party: Default::default(),
            up_next_countdown: 10,
            mpv_options: Vec::new(),
            untracked: false,
        };
        let stream = Stream {
            provider_id: "preview".into(),
            ..Stream::new(video.clone(), StreamKind::Mp4)
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_keep, commands) = mpsc::unbounded_channel();
        let printer = tokio::spawn(async move {
            while let Some(update) = rx.recv().await {
                match update {
                    Event::Toast(t) => println!("  toast: {}", t.text),
                    Event::PlaybackEnded { watched, ending, .. } => {
                        println!("  ended (watched: {watched}, {ending:?})")
                    }
                    _ => {}
                }
            }
        });
        let play = playback::play(
            vec![stream],
            context,
            store.clone(),
            http.clone(),
            playback::Machine::desktop(
                mpv.clone(),
                slot.clone(),
                std::sync::Arc::new(kaeri_engine::host::DesktopHost::new(&config, &paths)),
            ),
            settings,
            tx.into(),
            commands,
        );
        tokio::time::timeout(Duration::from_secs(600), play).await?;
        let _ = printer.await;
    }
    Ok(())
}

/// The video's length, from ffprobe when it is installed.
fn duration(path: &str) -> Option<f64> {
    let output = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", path])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}
