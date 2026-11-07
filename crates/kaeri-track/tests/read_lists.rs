//! The MyAnimeList and Kitsu manga lists against their fakes: a pull that follows the paging, and
//! the pushes (change, create, volumes, read again). Nothing here reaches the real services.

use std::sync::Arc;

use kaeri_core::{
    ids::AnilistId,
    media::Medium,
    traits::{TrackOp, Tracker, WatchStatus},
};
use kaeri_testkit::readlists::{FakeReadLists, KITSU_TOKEN, KitsuEntry, MAL_TOKEN, MalEntry};
use kaeri_track::{KitsuTracker, MalTracker, TokenPair, kitsu, mal, read::ReadMapping};

/// AniList 7, 8 and 9 are MyAnimeList 70, 80, 90 and Kitsu 700, 800, 900.
struct Map;

impl Map {
    fn at(id: u32) -> Option<u32> {
        (7..=9).contains(&id).then_some(id)
    }
}

impl ReadMapping for Map {
    fn mal_id(&self, id: AnilistId) -> Option<u32> {
        Self::at(id.get()).map(|n| n * 10)
    }
    fn anilist_for_mal(&self, mal: u32) -> Option<AnilistId> {
        Self::at(mal / 10).map(AnilistId::new)
    }
    fn kitsu_id(&self, id: AnilistId) -> Option<u32> {
        Self::at(id.get()).map(|n| n * 100)
    }
    fn anilist_for_kitsu(&self, kitsu: u32) -> Option<AnilistId> {
        Self::at(kitsu / 100).map(AnilistId::new)
    }
    fn mangaupdates_id(&self, _: AnilistId) -> Option<u64> {
        None
    }
    fn anilist_for_mangaupdates(&self, _: u64) -> Option<AnilistId> {
        None
    }
}

struct Anime;
impl mal::IdMapping for Anime {
    fn mal_id(&self, _: AnilistId) -> Option<u32> {
        None
    }
    fn anilist_id(&self, _: u32) -> Option<AnilistId> {
        None
    }
}
impl kitsu::IdMapping for Anime {
    fn kitsu_id(&self, _: AnilistId) -> Option<u32> {
        None
    }
    fn anilist_id(&self, _: u32) -> Option<AnilistId> {
        None
    }
}

fn pair(token: &str) -> Option<TokenPair> {
    Some(TokenPair { access: token.into(), refresh: None, expires_at: None })
}

fn mal_tracker(server: &FakeReadLists, token: &str) -> MalTracker {
    MalTracker::new("client", reqwest::Client::new(), Arc::new(Anime), pair(token))
        .with_read_mapping(Arc::new(Map))
        .with_api(server.mal_api())
}

fn kitsu_tracker(server: &FakeReadLists, token: &str) -> KitsuTracker {
    KitsuTracker::new(reqwest::Client::new(), Arc::new(Anime), pair(token))
        .with_read_mapping(Arc::new(Map))
        .at(kitsu::Site::at(&server.kitsu_base()))
}

fn progress(id: u32, chapter: u32, volume: Option<u32>) -> TrackOp {
    TrackOp::SetReadProgress { anilist_id: AnilistId::new(id), chapter, volume }
}

fn status(id: u32, status: WatchStatus, rereading: bool) -> TrackOp {
    TrackOp::SetReadStatus { anilist_id: AnilistId::new(id), status, rereading, at: 1 }
}

#[tokio::test]
async fn the_mal_pull_follows_the_paging_and_reads_every_field() {
    let server = FakeReadLists::start().await;
    let mut novel = MalEntry::new(80, "completed", 40, 4);
    novel.media_type = "light_novel".into();
    server.add_mal(MalEntry::new(70, "reading", 12, 2));
    server.add_mal(novel);
    let mut again = MalEntry::new(90, "reading", 5, 1);
    again.rereading = true;
    again.score = 8;
    server.add_mal(again);
    server.add_mal(MalEntry::new(55, "reading", 1, 0));

    let got = mal_tracker(&server, MAL_TOKEN).pull_read_library().await.unwrap();
    assert_eq!(got.len(), 3, "an unmapped entry is skipped");
    let by = |id| got.iter().find(|e| e.anilist_id == AnilistId::new(id)).unwrap();
    assert_eq!((by(7).chapters, by(7).volumes), (12, Some(2)));
    assert_eq!(by(8).medium, Medium::Novel);
    assert_eq!(by(8).status, WatchStatus::Completed);
    assert!(by(9).rereading);
    assert_eq!(by(9).score, Some(8.0));
}

#[tokio::test]
async fn the_mal_push_writes_chapters_volumes_status_and_read_again() {
    let server = FakeReadLists::start().await;
    server.add_mal(MalEntry::new(70, "reading", 12, 2));
    let t = mal_tracker(&server, MAL_TOKEN);
    let ops = [
        progress(7, 15, Some(3)),
        status(7, WatchStatus::Current, true),
        progress(8, 4, None),
        progress(99, 1, None),
    ];
    let ignored = t.push_read(&ops).await.unwrap();
    assert_eq!(ignored, [AnilistId::new(99)]);
    let a = server.mal(70).unwrap();
    assert_eq!((a.chapters, a.volumes, a.rereading), (15, 3, true));
    let b = server.mal(80).unwrap();
    assert_eq!(b.chapters, 4);
}

#[tokio::test]
async fn a_rejected_mal_token_is_an_auth_error() {
    let server = FakeReadLists::start().await;
    let err = mal_tracker(&server, "stale").pull_read_library().await.unwrap_err();
    assert!(matches!(err, kaeri_core::Error::Auth(_)), "{err:?}");
}

#[tokio::test]
async fn the_kitsu_pull_follows_the_paging_and_has_no_volumes() {
    let server = FakeReadLists::start().await;
    server.add_kitsu(KitsuEntry::new(700, "current", 12));
    let mut novel = KitsuEntry::new(800, "completed", 40);
    novel.subtype = "novel".into();
    server.add_kitsu(novel);
    let mut again = KitsuEntry::new(900, "current", 5);
    again.reconsuming = true;
    again.rating_twenty = Some(16);
    server.add_kitsu(again);

    let got = kitsu_tracker(&server, KITSU_TOKEN).pull_read_library().await.unwrap();
    assert_eq!(got.len(), 3);
    let by = |id| got.iter().find(|e| e.anilist_id == AnilistId::new(id)).unwrap();
    assert_eq!((by(7).chapters, by(7).volumes), (12, None));
    assert_eq!(by(8).medium, Medium::Novel);
    assert!(by(9).rereading);
    assert_eq!(by(9).score, Some(8.0));
}

#[tokio::test]
async fn the_kitsu_push_changes_creates_and_never_writes_a_volume() {
    let server = FakeReadLists::start().await;
    server.add_kitsu(KitsuEntry::new(700, "current", 12));
    let t = kitsu_tracker(&server, KITSU_TOKEN);
    let ops = [
        progress(7, 15, Some(3)),
        status(7, WatchStatus::Current, true),
        progress(8, 4, Some(1)),
        progress(99, 1, None),
    ];
    let ignored = t.push_read(&ops).await.unwrap();
    assert_eq!(ignored, [AnilistId::new(99)]);
    let a = server.kitsu(700).unwrap();
    assert_eq!((a.progress, a.reconsuming), (15, true));
    let b = server.kitsu(800).unwrap();
    assert_eq!((b.progress, b.status.as_str()), (4, "current"));
    assert!(
        server.requests().iter().all(|(_, path)| !path.contains("volume")),
        "no volume field is ever asked for"
    );
}

#[tokio::test]
async fn a_rejected_kitsu_token_is_an_auth_error() {
    let server = FakeReadLists::start().await;
    let err = kitsu_tracker(&server, "stale").pull_read_library().await.unwrap_err();
    assert!(matches!(err, kaeri_core::Error::Auth(_)), "{err:?}");
}

#[tokio::test]
async fn removing_takes_a_title_off_the_mal_and_kitsu_lists_and_says_what_happened() {
    use kaeri_core::traits::ReadRemoval;
    let server = FakeReadLists::start().await;
    server.add_mal(MalEntry::new(70, "reading", 12, 2));
    server.add_kitsu(KitsuEntry::new(700, "current", 12));
    let (mal, kitsu) = (mal_tracker(&server, MAL_TOKEN), kitsu_tracker(&server, KITSU_TOKEN));
    let id = AnilistId::new(7);

    assert_eq!(mal.remove_read(id).await.unwrap(), ReadRemoval::Removed);
    assert_eq!(kitsu.remove_read(id).await.unwrap(), ReadRemoval::Removed);
    assert!(server.mal(70).is_none() && server.kitsu(700).is_none());
    // Asked again, the lists no longer hold it.
    assert_eq!(mal.remove_read(id).await.unwrap(), ReadRemoval::NotOnList);
    assert_eq!(kitsu.remove_read(id).await.unwrap(), ReadRemoval::NotOnList);
    // A title the mappings can't name is never guessed at.
    let unknown = AnilistId::new(99);
    assert_eq!(mal.remove_read(unknown).await.unwrap(), ReadRemoval::Unmatched);
    assert_eq!(kitsu.remove_read(unknown).await.unwrap(), ReadRemoval::Unmatched);

    let err = mal_tracker(&server, "stale").remove_read(AnilistId::new(8)).await.unwrap_err();
    assert!(matches!(err, kaeri_core::Error::Auth(_)), "{err:?}");
}
