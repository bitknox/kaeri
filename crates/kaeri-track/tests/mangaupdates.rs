//! The MangaUpdates tracker against the fake: sign-in, the list pull, the push (add, update,
//! move, rating), the update delay and a rejected session. Nothing here reaches the real service.

use std::{sync::Arc, time::Duration};

use kaeri_core::{
    ids::AnilistId,
    media::Medium,
    traits::{TrackOp, Tracker, WatchStatus},
};
use kaeri_testkit::mangaupdates::{self as fake, FakeMangaUpdates, Item};
use kaeri_track::{MangaUpdatesTracker, mangaupdates, read::ReadMapping, secret::TokenPair};

/// Two made-up works: AniList 7 is series 9_000_000_001, AniList 8 is series 9_000_000_002.
struct Map;

impl ReadMapping for Map {
    fn mal_id(&self, _: AnilistId) -> Option<u32> {
        None
    }
    fn anilist_for_mal(&self, _: u32) -> Option<AnilistId> {
        None
    }
    fn kitsu_id(&self, _: AnilistId) -> Option<u32> {
        None
    }
    fn anilist_for_kitsu(&self, _: u32) -> Option<AnilistId> {
        None
    }
    fn mangaupdates_id(&self, id: AnilistId) -> Option<u64> {
        match id.get() {
            7 => Some(9_000_000_001),
            8 => Some(9_000_000_002),
            _ => None,
        }
    }
    fn anilist_for_mangaupdates(&self, series: u64) -> Option<AnilistId> {
        match series {
            9_000_000_001 => Some(AnilistId::new(7)),
            9_000_000_002 => Some(AnilistId::new(8)),
            _ => None,
        }
    }
}

const A: u64 = 9_000_000_001;
const B: u64 = 9_000_000_002;

fn pair(token: &str) -> TokenPair {
    TokenPair { access: token.into(), refresh: None, expires_at: None }
}

fn tracker(server: &FakeMangaUpdates, token: &str) -> MangaUpdatesTracker {
    MangaUpdatesTracker::new(reqwest::Client::new(), Arc::new(Map), Some(pair(token)))
        .with_api(server.base())
        .with_gaps(Duration::ZERO, Duration::ZERO)
}

fn item(series: u64, list_id: u64, chapter: u32, volume: u32) -> Item {
    Item {
        series,
        title: format!("Lantern Road {series}"),
        novel: false,
        list_id,
        chapter,
        volume,
        rating: None,
    }
}

fn progress(id: u32, chapter: u32, volume: Option<u32>) -> TrackOp {
    TrackOp::SetReadProgress { anilist_id: AnilistId::new(id), chapter, volume }
}

#[tokio::test]
async fn a_password_buys_a_session_token_with_nothing_to_renew_it() {
    let server = FakeMangaUpdates::start().await;
    let http = reqwest::Client::new();
    let got =
        mangaupdates::login(&http, &server.base(), fake::USER, fake::PASSWORD).await.unwrap();
    assert_eq!(got.access, fake::TOKEN);
    assert_eq!((got.refresh, got.expires_at), (None, None));
    let wrong = mangaupdates::login(&http, &server.base(), fake::USER, "nope").await;
    assert!(wrong.is_err());
}

#[tokio::test]
async fn the_pull_reads_every_list_and_skips_series_nobody_mapped() {
    let server = FakeMangaUpdates::start().await;
    server.add(item(A, fake::READING, 12, 2));
    server.add(item(B, fake::COMPLETE, 40, 4));
    server.add(item(9_000_000_099, fake::HOLD, 1, 0));
    server.add(item(9_000_000_098, fake::CUSTOM, 3, 0));
    let entries = tracker(&server, fake::TOKEN).pull_read_library().await.unwrap();
    assert_eq!(entries.len(), 2);
    let a = entries.iter().find(|e| e.anilist_id == AnilistId::new(7)).unwrap();
    assert_eq!((a.chapters, a.volumes, a.status), (12, Some(2), WatchStatus::Current));
    assert_eq!(a.medium, Medium::Manga);
    let b = entries.iter().find(|e| e.anilist_id == AnilistId::new(8)).unwrap();
    assert_eq!(b.status, WatchStatus::Completed);
}

#[tokio::test]
async fn a_new_series_is_added_then_its_numbers_set_in_array_calls() {
    let server = FakeMangaUpdates::start().await;
    let t = tracker(&server, fake::TOKEN);
    let ops = [progress(7, 5, Some(1)), progress(8, 2, None)];
    let ignored = t.push_read(&ops).await.unwrap().ignored;
    assert!(ignored.is_empty());
    assert_eq!(server.writes(), ["/v1/lists/series", "/v1/lists/series/update"]);
    let a = server.item(A).unwrap();
    assert_eq!((a.list_id, a.chapter, a.volume), (fake::READING, 5, 1));
    assert_eq!(server.item(B).unwrap().chapter, 2);
}

#[tokio::test]
async fn progress_never_goes_down_and_an_unchanged_series_is_not_written() {
    let server = FakeMangaUpdates::start().await;
    server.add(item(A, fake::READING, 20, 3));
    let t = tracker(&server, fake::TOKEN);
    t.push_read(&[progress(7, 4, Some(1))]).await.unwrap();
    assert!(server.writes().is_empty(), "nothing to raise");
    let a = server.item(A).unwrap();
    assert_eq!((a.chapter, a.volume), (20, 3));
    t.push_read(&[progress(7, 21, None)]).await.unwrap();
    assert_eq!(server.item(A).unwrap().chapter, 21);
    assert_eq!(server.item(A).unwrap().volume, 3);
}

#[tokio::test]
async fn a_status_moves_the_series_between_lists_but_never_off_a_custom_one() {
    let server = FakeMangaUpdates::start().await;
    server.add(item(A, fake::READING, 10, 1));
    server.add(item(B, fake::CUSTOM, 6, 0));
    let t = tracker(&server, fake::TOKEN);
    let status = |id, status| TrackOp::SetReadStatus {
        anilist_id: AnilistId::new(id),
        status,
        rereading: false,
        at: 1,
    };
    t.push_read(&[status(7, WatchStatus::Completed), status(8, WatchStatus::Completed)])
        .await
        .unwrap();
    assert_eq!(server.item(A).unwrap().list_id, fake::COMPLETE);
    assert_eq!(server.item(B).unwrap().list_id, fake::CUSTOM);
}

#[tokio::test]
async fn a_rating_is_set_on_the_series() {
    let server = FakeMangaUpdates::start().await;
    server.add(item(A, fake::READING, 1, 0));
    let t = tracker(&server, fake::TOKEN);
    t.push_read(&[TrackOp::SetReadScore { anilist_id: AnilistId::new(7), score: 8.5, at: 1 }])
        .await
        .unwrap();
    assert_eq!(server.item(A).unwrap().rating, Some(8.5));
}

#[tokio::test]
async fn the_five_second_delay_is_waited_out_and_the_write_sent_again() {
    let server = FakeMangaUpdates::start().await;
    server.add(item(A, fake::READING, 1, 0));
    server.delay_writes(2);
    let t = tracker(&server, fake::TOKEN);
    t.push_read(&[progress(7, 9, None)]).await.unwrap();
    assert_eq!(server.turned_away(), 2);
    assert_eq!(server.item(A).unwrap().chapter, 9);
}

#[tokio::test]
async fn a_delay_that_does_not_lift_is_left_to_the_outbox() {
    let server = FakeMangaUpdates::start().await;
    server.add(item(A, fake::READING, 1, 0));
    server.delay_writes(50);
    let t = tracker(&server, fake::TOKEN);
    // A pause, never a refusal: the outbox waits it out without counting it.
    let error = t.push_read(&[progress(7, 9, None)]).await.unwrap_err();
    assert!(matches!(error, kaeri_core::Error::RateLimited { .. }), "{error:?}");
    assert_eq!(server.item(A).unwrap().chapter, 1);
}

#[tokio::test]
async fn a_rejected_session_is_an_auth_error() {
    let server = FakeMangaUpdates::start().await;
    let t = tracker(&server, "stale");
    let err = t.pull_read_library().await.unwrap_err();
    assert!(matches!(err, kaeri_core::Error::Auth(_)), "{err:?}");
}

#[tokio::test]
async fn a_work_without_a_series_id_is_held_back_and_a_pushed_one_is_ignored() {
    let server = FakeMangaUpdates::start().await;
    let t = tracker(&server, fake::TOKEN);
    assert!(!t.can_address(&progress(99, 1, None)));
    assert!(t.can_address(&progress(7, 1, None)));
    let ignored = t.push_read(&[progress(99, 1, None)]).await.unwrap().ignored;
    assert_eq!(ignored, [AnilistId::new(99)]);
}

#[tokio::test]
async fn the_series_a_pull_read_is_not_asked_for_again() {
    let server = FakeMangaUpdates::start().await;
    server.add(item(A, fake::READING, 10, 0));
    let t = tracker(&server, fake::TOKEN);
    t.pull_read_library().await.unwrap();
    let before = server.requests().len();
    t.push_read(&[progress(7, 10, None)]).await.unwrap();
    let singles = server.requests()[before..]
        .iter()
        .filter(|r| r.path.starts_with("/v1/lists/series/"))
        .count();
    assert_eq!(singles, 0);
}

#[tokio::test]
async fn removing_takes_a_series_off_its_list() {
    use kaeri_core::traits::ReadRemoval;
    let server = FakeMangaUpdates::start().await;
    server.add(item(A, fake::READING, 10, 0));
    let t = tracker(&server, fake::TOKEN);
    // A pull first, so the kept answer is the one a removal must not trust.
    t.pull_read_library().await.unwrap();
    assert_eq!(t.remove_read(AnilistId::new(7)).await.unwrap(), ReadRemoval::Removed);
    assert!(server.item(A).is_none());
    assert_eq!(t.remove_read(AnilistId::new(7)).await.unwrap(), ReadRemoval::NotOnList);
    assert_eq!(t.remove_read(AnilistId::new(8)).await.unwrap(), ReadRemoval::NotOnList);
    assert_eq!(t.remove_read(AnilistId::new(99)).await.unwrap(), ReadRemoval::Unmatched);
}
