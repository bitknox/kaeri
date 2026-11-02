//! Keep next: the next episodes of a show being watched, queued ahead of the viewer only as an
//! episode of that show starts or finishes (counted watched, played to its end, or marked
//! watched on its own), and not while downloads wait; and let go of a day after they are
//! watched. Nothing else queues them: not a launch, a change of setting, a mark of many episodes,
//! a list sync or the new-episode check.
//!
//! Off in every other test, as the rest of the engine's own work is; on here, with the device
//! taken to have a terabyte free. Nothing starts a torrent: the harness has no session, so what
//! is queued stays queued, which is all these read. Watching is dated by the wall clock, which
//! the engine reads it against, rather than by the fixtures' moment, which is in the past.
//! Unless a test signs in, there is no tracker account, so the shows followed are the ones
//! watched here lately.

use kaeri_core::{
    ids::AnilistId,
    settings::SettingValue,
    stream::{Stream, StreamKind},
};
use kaeri_model::{
    command::{AppState, Command},
    event::Event,
    playback::Ending,
    settings::SettingKey,
    show::ShowSettingKey,
};
use kaeri_providers::ProviderRegistry;
use kaeri_store::Store;
use kaeri_testkit::{
    fixtures::{AIRING, FINISHED},
    seed, sources,
};

use super::{WITH_ANILIST, toasts};
use crate::harness::{Harness, Options};

/// The fixture source, offering a torrent for every episode of [`FINISHED`] and [`AIRING`].
fn torrents() -> ProviderRegistry {
    let torrent = |n: u32| Stream {
        provider_id: sources::PROVIDER_ID.into(),
        download_source: Some(format!("magnet:?xt=urn:btih:{n:040x}&dn=fixture")),
        ..Stream::new("http://127.0.0.1:9/stream", StreamKind::TorrentHttp)
    };
    let provider = sources::provider()
        .with_streams_for(sources::key(FINISHED), vec![torrent(1)])
        .with_streams_for(sources::key(AIRING), vec![torrent(2)]);
    ProviderRegistry::new(vec![provider.arc()])
}

/// The app with keep next on, and the torrents above.
async fn keeping(options: Options) -> Harness {
    Harness::with(Options { keep_next: true, registry: Some(torrents()), ..options }).await
}

/// `config.toml` keeping `n` episodes.
fn keep(n: u32) -> Option<String> {
    Some(format!("[downloads]\nkeep_next = {n}\n"))
}

const HOUR: i64 = 3_600;

/// Episode `episode` of `id` starting, as the engine says of a play it has started.
fn starts(h: &mut Harness, id: AnilistId, episode: &str) {
    h.player_says(Event::PlaybackStaged {
        id,
        title: "Show".into(),
        episode: episode.into(),
        episode_title: None,
        speed: 1.0,
    });
}

/// Now, by the wall clock.
fn now() -> i64 {
    kaeri_store::now()
}

/// [`FINISHED`] being watched, four episodes in, the last ten minutes ago.
fn four_in(store: &Store) {
    store.replace_library_statuses("anilist", &[(FINISHED, "CURRENT")], now()).unwrap();
    seed::watched(store, FINISHED, 4, now() - 600);
}

/// `config.toml` signed in to AniList, keeping `n` episodes.
fn signed_in_keeping(n: u32) -> Option<String> {
    Some(format!("{WITH_ANILIST}\n[downloads]\nkeep_next = {n}\n"))
}

/// What the queue has of show `id`: each episode, with `ahead` beside one keep next queued.
fn queue(store: &Store, id: AnilistId) -> Vec<String> {
    let mut rows: Vec<_> =
        store.downloads().unwrap().into_iter().filter(|row| row.anilist_id == id).collect();
    rows.sort_by_key(|row| row.episode.parse::<u32>().unwrap_or(0));
    rows.iter()
        .map(|row| match row.ahead {
            true => format!("{} ahead", row.episode),
            false => row.episode.clone(),
        })
        .collect()
}

/// The last queue among `updates`, as the Downloads screen was told it: each episode, with
/// `ahead` beside one keep next queued.
fn said(updates: &[Event]) -> Vec<String> {
    let rows = updates
        .iter()
        .rev()
        .find_map(|update| match update {
            Event::Downloads(rows) => Some(rows),
            _ => None,
        })
        .expect("the queue was said");
    let mut rows: Vec<_> =
        rows.iter().map(|row| (row.episode.parse::<u32>().unwrap_or(0), row.ahead)).collect();
    rows.sort();
    rows.into_iter()
        .map(
            |(episode, ahead)| {
                if ahead { format!("{episode} ahead") } else { episode.to_string() }
            },
        )
        .collect()
}

#[tokio::test]
async fn nothing_is_queued_on_start() {
    let mut h =
        keeping(Options { config: keep(2), before_start: Some(four_in), ..Options::default() })
            .await;
    h.send(Command::LoadDownloads);
    let updates = h.settle().await;
    assert!(queue(h.store(), FINISHED).is_empty(), "{:?}", queue(h.store(), FINISHED));
    assert!(toasts(&updates).is_empty(), "quietly: {:?}", toasts(&updates));
}

#[tokio::test]
async fn an_episode_starting_tops_that_show_up_and_no_other() {
    let mut h = keeping(Options {
        config: keep(2),
        before_start: Some(|store| {
            four_in(store);
            seed::watched(store, AIRING, 2, now() - 600);
        }),
        ..Options::default()
    })
    .await;
    starts(&mut h, FINISHED, "5");
    let updates = h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);
    assert_eq!(said(&updates), ["5 ahead", "6 ahead"], "the Downloads screen says so");
    assert!(queue(h.store(), AIRING).is_empty(), "only the show that started");

    // Started again with nothing new to keep: the one row each.
    starts(&mut h, FINISHED, "5");
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);
}

#[tokio::test]
async fn changing_the_number_kept_or_marking_many_episodes_queues_nothing() {
    let mut h = keeping(Options::default()).await;
    four_in(h.store());
    h.send(Command::SetSetting {
        key: SettingKey::KeepNextEpisodes,
        value: SettingValue::Int(2),
    });
    h.send(Command::SetShowSetting {
        id: FINISHED,
        key: ShowSettingKey::KeepNext,
        value: Some(SettingValue::Int(3)),
    });
    // A season marked at once is not an episode finished.
    h.send(Command::SetWatched {
        id: FINISHED,
        episodes: vec!["5".into(), "6".into()],
        watched: true,
    });
    h.settle().await;
    assert!(queue(h.store(), FINISHED).is_empty(), "{:?}", queue(h.store(), FINISHED));

    // The show's own number, once one of its episodes starts.
    starts(&mut h, FINISHED, "7");
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["7 ahead", "8 ahead", "9 ahead"]);

    // Fewer kept: nothing deleted.
    h.send(Command::StepSetting { key: SettingKey::KeepNextEpisodes, delta: -1 });
    h.send(Command::ResetShowSettings { id: FINISHED });
    starts(&mut h, FINISHED, "7");
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["7 ahead", "8 ahead", "9 ahead"]);
}

#[tokio::test]
async fn one_episode_marked_watched_tops_that_show_up_once() {
    let mut h = keeping(Options {
        config: keep(2),
        before_start: Some(|store| {
            four_in(store);
            seed::watched(store, AIRING, 2, now() - 600);
        }),
        ..Options::default()
    })
    .await;
    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["5".into()], watched: true });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["6 ahead", "7 ahead"]);
    assert!(queue(h.store(), AIRING).is_empty(), "only the show marked");

    // Marked again with nothing new to keep: the one row each.
    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["5".into()], watched: true });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["6 ahead", "7 ahead"]);

    // Marked unwatched: nothing more queued, nothing deleted.
    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["5".into()], watched: false });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["6 ahead", "7 ahead"]);
}

#[tokio::test]
async fn an_episode_played_to_its_end_or_counted_watched_tops_the_show_up() {
    let mut h =
        keeping(Options { config: keep(2), before_start: Some(four_in), ..Options::default() })
            .await;
    h.send(Command::LoadDownloads);
    h.settle().await;

    // As the playback of 5 records it, then says it ended.
    seed::watched(h.store(), FINISHED, 5, now());
    let ended = Event::playback_ended(true, Ending::Complete { next: None });
    h.playback_says(1_000_001, FINISHED, "5", ended);
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["6 ahead", "7 ahead"]);

    // Counted watched partway through the credits of 6.
    seed::watched(h.store(), FINISHED, 6, now() + 60);
    h.playback_says(1_000_002, FINISHED, "6", Event::ProgressQueued);
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["6 ahead", "7 ahead", "8 ahead"]);
}

#[tokio::test]
async fn a_list_sync_or_the_new_episode_check_queues_nothing() {
    let mut h = keeping(Options {
        config: signed_in_keeping(4),
        signed_in: vec!["anilist"],
        ..Options::default()
    })
    .await;
    // The fake's list: three episodes into the airing show, of which five are out and the
    // sixth airs in two days; the finished one completed.
    h.send(Command::SyncNow);
    h.settle().await;
    h.send(Command::CheckNewEpisodes);
    h.settle().await;
    assert!(queue(h.store(), AIRING).is_empty(), "{:?}", queue(h.store(), AIRING));
    assert!(queue(h.store(), FINISHED).is_empty());

    // Once the viewer starts the airing show's next episode: only what is out.
    starts(&mut h, AIRING, "4");
    h.settle().await;
    assert_eq!(queue(h.store(), AIRING), ["4 ahead", "5 ahead"]);
}

#[tokio::test]
async fn nothing_is_kept_while_downloads_wait_on_cellular_or_off_screen() {
    let mut h = keeping(Options { config: keep(2), ..Options::default() }).await;
    h.send(Command::NetworkPath { cellular: true, constrained: false, offline: false });
    h.settle().await;
    four_in(h.store());
    starts(&mut h, FINISHED, "5");
    h.settle().await;
    assert!(queue(h.store(), FINISHED).is_empty(), "Wi-Fi only");

    // Back on Wi-Fi, but off screen: still waiting.
    h.send(Command::AppPhase { phase: AppState::Background { playing: false } });
    h.send(Command::NetworkPath { cellular: false, constrained: false, offline: false });
    h.settle().await;
    assert!(queue(h.store(), FINISHED).is_empty(), "off screen");

    // In front again: the show asked for while downloads waited, topped up.
    h.send(Command::AppPhase { phase: AppState::Foreground });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);
}

#[tokio::test]
async fn a_show_set_aside_on_the_list_keeps_its_downloads_and_is_topped_up_no_more() {
    let mut h = keeping(Options {
        config: signed_in_keeping(2),
        signed_in: vec!["anilist"],
        before_start: Some(four_in),
        ..Options::default()
    })
    .await;
    starts(&mut h, FINISHED, "5");
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);

    // Paused on the list, however lately it was watched here.
    h.store().replace_library_statuses("anilist", &[(FINISHED, "PAUSED")], now()).unwrap();
    seed::watched(h.store(), FINISHED, 5, now());
    starts(&mut h, FINISHED, "6");
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);
}

#[tokio::test]
async fn an_episode_asked_for_is_the_viewers_and_one_kept_ahead_becomes_theirs() {
    let mut h =
        keeping(Options { config: keep(2), before_start: Some(four_in), ..Options::default() })
            .await;
    starts(&mut h, FINISHED, "5");
    h.send(Command::DownloadEpisode { id: FINISHED, episode: "9".into() });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead", "9"]);

    h.send(Command::DownloadEpisode { id: FINISHED, episode: "5".into() });
    let updates = h.settle().await;
    assert_eq!(said(&updates), ["5", "6 ahead", "9"]);
}

#[tokio::test]
async fn an_episode_the_viewer_lets_go_of_is_not_queued_again_until_asked_for() {
    let mut h =
        keeping(Options { config: keep(2), before_start: Some(four_in), ..Options::default() })
            .await;
    starts(&mut h, FINISHED, "5");
    h.settle().await;
    let six = h.store().download_for(FINISHED, "6").unwrap().unwrap();
    h.send(Command::DownloadCancel { id: six.id });
    starts(&mut h, FINISHED, "5");
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead"]);

    h.send(Command::DownloadEpisode { id: FINISHED, episode: "6".into() });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6"]);
}

/// Episodes 1 and 3 of [`FINISHED`] downloaded ahead and 2 by hand; four watched, the first a
/// day and an hour ago and the last 22 hours ago, an hour apart.
fn watched_past_ahead(store: &Store) {
    seed::watched(store, FINISHED, 4, now() - 22 * HOUR);
    for (episode, ahead) in [("1", true), ("2", false), ("3", true)] {
        let row = if ahead {
            store.enqueue_download_ahead(FINISHED, episode, "Show", "magnet:?xt=urn:btih:1")
        } else {
            store.enqueue_download(FINISHED, episode, "Show", "magnet:?xt=urn:btih:1")
        };
        store.finish_download(row.unwrap().id, None).unwrap();
    }
}

#[tokio::test]
async fn an_episode_kept_ahead_goes_a_day_after_it_is_watched_and_one_asked_for_stays() {
    let mut h = keeping(Options {
        config: keep(2),
        before_start: Some(watched_past_ahead),
        ..Options::default()
    })
    .await;
    h.send(Command::LoadDownloads);
    let updates = h.settle().await;
    // One a day gone, two the viewer's, three an hour short of a day; nothing queued on start.
    assert_eq!(queue(h.store(), FINISHED), ["2", "3 ahead"]);
    assert_eq!(said(&updates), ["2", "3 ahead"]);
    assert!(
        !h.store().download_dismissed(FINISHED, "1").unwrap(),
        "not let go of by the viewer"
    );
}
