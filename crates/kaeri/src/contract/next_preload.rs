//! Preloading the next episode: made ready while an ending plays, only while Auto-play next is
//! on, and never at launch or offline; and the Settings row that lets a limited network have
//! its first seconds too, dimmed while Auto-play next is off.
//!
//! The source counts what it is asked, so "made ready" is the source asked for the next
//! episode, and "not made ready" is a source asked nothing.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use kaeri_core::settings::SettingValue;
use kaeri_model::{command::Command, event::Event, settings::SettingKey};
use kaeri_providers::ProviderRegistry;
use kaeri_testkit::fixtures::FINISHED;

use crate::harness::{Harness, Options, names};

/// The App over the fixture source, and the count of what the source was asked.
async fn with_a_counted_source(config: Option<&str>) -> (Harness, Arc<AtomicUsize>) {
    let provider = kaeri_testkit::sources::provider();
    let asked = provider.call_count();
    let registry = ProviderRegistry::new(vec![provider.arc()]);
    let options = Options {
        registry: Some(registry),
        config: config.map(Into::into),
        ..Options::default()
    };
    (Harness::with(options).await, asked)
}

/// The ending of episode 4 asks for episode 5 to be made ready.
async fn the_ending_asks(h: &mut Harness) {
    h.player_says(Event::PrefetchNext { id: FINISHED, episode: "5".into() });
    assert_eq!(names(&h.settle().await), ["PrefetchNext"]);
}

/// Whether the source is asked anything past `before` within a couple of seconds.
async fn asked_past(asked: &AtomicUsize, before: usize) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        if asked.load(Ordering::SeqCst) > before {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

#[tokio::test]
async fn nothing_is_made_ready_at_launch() {
    let (mut h, asked) = with_a_counted_source(None).await;
    h.settle().await;
    assert!(!asked_past(&asked, 0).await, "the source was asked at launch");
}

#[tokio::test]
async fn with_auto_play_next_off_the_ending_makes_nothing_ready() {
    let (mut h, asked) = with_a_counted_source(Some("[playback]\nauto_next = false\n")).await;
    h.settle().await;
    let before = asked.load(Ordering::SeqCst);
    the_ending_asks(&mut h).await;
    assert!(!asked_past(&asked, before).await, "nothing rolls on, so nothing is looked for");
}

#[tokio::test]
async fn offline_the_ending_makes_nothing_ready() {
    let (mut h, asked) = with_a_counted_source(None).await;
    h.send(Command::NetworkPath { cellular: false, constrained: false, offline: true });
    h.settle().await;
    let before = asked.load(Ordering::SeqCst);
    the_ending_asks(&mut h).await;
    assert!(!asked_past(&asked, before).await);
}

#[tokio::test]
async fn on_mobile_data_the_next_episode_is_still_looked_for() {
    // Light, with the setting off: the streams are found, whatever video waits.
    let (mut h, asked) = with_a_counted_source(None).await;
    h.send(Command::NetworkPath { cellular: true, constrained: false, offline: false });
    h.settle().await;
    let before = asked.load(Ordering::SeqCst);
    the_ending_asks(&mut h).await;
    assert!(asked_past(&asked, before).await, "the source was never asked");
}

/// The Preload row as the terminal shows it.
fn preload_row(h: &Harness) -> kaeri_tui::app::SettingLine {
    h.app()
        .setting_rows()
        .into_iter()
        .find(|row| row.key == SettingKey::PreloadMetered)
        .expect("the Preload row")
}

#[tokio::test]
async fn preload_on_metered_steps_like_the_other_switches_and_is_kept() {
    let mut h = Harness::new().await;
    h.settle().await;
    let row = preload_row(&h);
    assert_eq!((row.label.as_str(), row.value.as_str()), ("Preload on metered", "Off"));
    assert!(row.editable && !row.dimmed);

    h.send(Command::StepSetting { key: SettingKey::PreloadMetered, delta: 1 });
    h.settle().await;
    assert_eq!(preload_row(&h).value, "On");
    let file = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert!(file.contains("[playback]\npreload_metered = true\n"), "{file}");

    // The row sits right under Auto-play next.
    let rows = h.app().setting_rows();
    let at = |key| rows.iter().position(|row| row.key == key).unwrap();
    assert_eq!(at(SettingKey::PreloadMetered), at(SettingKey::AutoNext) + 1);
}

#[tokio::test]
async fn preload_on_metered_waits_on_auto_play_next() {
    let mut h = Harness::new().await;
    h.settle().await;
    h.send(Command::SetSetting { key: SettingKey::AutoNext, value: SettingValue::Bool(false) });
    h.settle().await;
    let row = preload_row(&h);
    assert!(row.dimmed && !row.editable, "dimmed and not changed while nothing rolls on");
    let note = row.note.unwrap();
    assert!(note.starts_with("Needs Auto-play next. "), "{note}");

    h.send(Command::SetSetting { key: SettingKey::AutoNext, value: SettingValue::Bool(true) });
    h.settle().await;
    let row = preload_row(&h);
    assert!(!row.dimmed && row.editable);
    assert!(!row.note.unwrap().starts_with("Needs"));
}

#[tokio::test]
async fn the_note_names_torrents_while_torrent_playback_is_on() {
    let mut off = Harness::new().await;
    off.settle().await;
    let note = preload_row(&off).note.unwrap();
    assert!(!note.contains("Torrents"), "{note}");

    let config = "[providers.torrent]\nenabled = true\n";
    let mut on =
        Harness::with(Options { config: Some(config.into()), ..Options::default() }).await;
    on.settle().await;
    let note = preload_row(&on).note.unwrap();
    assert!(note.ends_with(" Torrents load their first pieces the same way."), "{note}");
}
