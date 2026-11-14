//! The real AniList, MyAnimeList and Kitsu trackers draining both outboxes against their fakes,
//! when the service refuses one title, goes down or rejects the token. Nothing here reaches the
//! real services.

use std::sync::Arc;

use kaeri_core::{
    config::NetworkConfig,
    ids::AnilistId,
    traits::{TrackOp, Tracker, WatchStatus},
};
use kaeri_net::HttpClient;
use kaeri_store::Store;
use kaeri_testkit::{
    anilist::{Failure, FakeAniList, Operation},
    readlists::{FakeReadLists, KITSU_TOKEN, MAL_TOKEN},
};
use kaeri_track::{
    AniListTracker, KitsuTracker, MalTracker, TokenPair, kitsu, mal,
    read::ReadMapping,
    sync::{self, DrainReport},
};

/// AniList 6 to 9 are MyAnimeList 60 to 90 and Kitsu 600 to 900, as anime and as manga. Title 6
/// is the one a test has refused.
struct Map;

const REFUSED: u32 = 6;

fn at(id: AnilistId) -> Option<u32> {
    (6..=9).contains(&id.get()).then_some(id.get())
}

impl ReadMapping for Map {
    fn mal_id(&self, id: AnilistId) -> Option<u32> {
        at(id).map(|n| n * 10)
    }
    fn anilist_for_mal(&self, mal: u32) -> Option<AnilistId> {
        at(AnilistId::new(mal / 10)).map(AnilistId::new)
    }
    fn kitsu_id(&self, id: AnilistId) -> Option<u32> {
        at(id).map(|n| n * 100)
    }
    fn anilist_for_kitsu(&self, kitsu: u32) -> Option<AnilistId> {
        at(AnilistId::new(kitsu / 100)).map(AnilistId::new)
    }
    fn mangaupdates_id(&self, _: AnilistId) -> Option<u64> {
        None
    }
    fn anilist_for_mangaupdates(&self, _: u64) -> Option<AnilistId> {
        None
    }
}

impl mal::IdMapping for Map {
    fn mal_id(&self, id: AnilistId) -> Option<u32> {
        at(id).map(|n| n * 10)
    }
    fn anilist_id(&self, mal: u32) -> Option<AnilistId> {
        at(AnilistId::new(mal / 10)).map(AnilistId::new)
    }
}

impl kitsu::IdMapping for Map {
    fn kitsu_id(&self, id: AnilistId) -> Option<u32> {
        at(id).map(|n| n * 100)
    }
    fn anilist_id(&self, kitsu: u32) -> Option<AnilistId> {
        at(AnilistId::new(kitsu / 100)).map(AnilistId::new)
    }
}

fn pair(token: &str) -> Option<TokenPair> {
    Some(TokenPair { access: token.into(), refresh: None, expires_at: None })
}

fn mal_tracker(server: &FakeReadLists, token: &str) -> MalTracker {
    MalTracker::new("client", reqwest::Client::new(), Arc::new(Map), pair(token))
        .with_read_mapping(Arc::new(Map))
        .with_api(server.mal_api())
}

fn kitsu_tracker(server: &FakeReadLists, token: &str) -> KitsuTracker {
    KitsuTracker::new(reqwest::Client::new(), Arc::new(Map), pair(token))
        .with_read_mapping(Arc::new(Map))
        .at(kitsu::Site::at(&server.kitsu_base()))
}

fn anilist_tracker(fake: &FakeAniList) -> AniListTracker {
    let http = HttpClient::new(&NetworkConfig::default()).unwrap();
    AniListTracker::new(fake.anilist(&http).with_token(Some("token".into())))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Media {
    Watching,
    Reading,
}

impl Media {
    fn progress(self, id: u32, n: u32) -> TrackOp {
        let anilist_id = AnilistId::new(id);
        match self {
            Self::Watching => TrackOp::SetProgress { anilist_id, episode: n },
            Self::Reading => TrackOp::SetReadProgress { anilist_id, chapter: n, volume: None },
        }
    }

    fn completed(self, id: u32) -> TrackOp {
        let anilist_id = AnilistId::new(id);
        let status = WatchStatus::Completed;
        match self {
            Self::Watching => TrackOp::SetStatus { anilist_id, status, at: 1_000 },
            Self::Reading => {
                TrackOp::SetReadStatus { anilist_id, status, rereading: false, at: 1_000 }
            }
        }
    }

    /// Three changes to three titles the services take.
    fn good(self) -> [TrackOp; 3] {
        [self.progress(7, 5), self.completed(8), self.progress(9, 2)]
    }

    async fn drain(self, store: &Store, tracker: &dyn Tracker, now: i64) -> DrainReport {
        match self {
            Self::Watching => sync::drain(store, tracker, now).await.unwrap(),
            Self::Reading => sync::drain_read(store, tracker, now).await.unwrap(),
        }
    }

    fn attempts(self, store: &Store, tracker: &str) -> Vec<u32> {
        let all = match self {
            Self::Watching => store.claim_ready(tracker, i64::MAX / 2, 100),
            Self::Reading => store.claim_ready_read(tracker, i64::MAX / 2, 100),
        };
        all.unwrap().iter().map(|entry| entry.attempts).collect()
    }
}

const BOTH: [Media; 2] = [Media::Watching, Media::Reading];

/// The refused title queued first and the good ones behind it.
fn queue(store: &Store, tracker: &str, media: Media) {
    store.enqueue(tracker, &media.progress(REFUSED, 3), 900).unwrap();
    for op in media.good() {
        store.enqueue(tracker, &op, 1_000).unwrap();
    }
}

/// One drain with the refused title queued first: the good ones go, the refused one alone is
/// counted, and the tracker is not held.
async fn refused_one(media: Media, store: &Store, tracker: &dyn Tracker) {
    let case = format!("{} {media:?}", tracker.id());
    let report = media.drain(store, tracker, 1_000).await;
    assert_eq!((report.sent, report.remaining), (3, 1), "{case}: {report:?}");
    assert_eq!(media.attempts(store, tracker.id()), [1], "{case}: the refusal is counted");
    assert!(!report.needs_reauth, "{case}");
    // A change made a moment later goes on the next pass: a refusal holds no one else.
    store.enqueue(tracker.id(), &media.progress(8, 9), 1_010).unwrap();
    let next = media.drain(store, tracker, 1_020).await;
    assert_eq!(next.sent, 1, "{case}: {next:?}");
}

/// The service down for a day, a drain each minute: nothing is counted, and everything goes
/// once it is back.
async fn down_then_back(
    media: Media,
    store: &Store,
    tracker: &dyn Tracker,
    up: impl Fn(bool),
) -> u32 {
    let case = format!("{} {media:?}", tracker.id());
    for op in media.good() {
        store.enqueue(tracker.id(), &op, 1_000).unwrap();
    }
    up(false);
    let mut now = 1_000;
    while now < 1_000 + 86_400 {
        let report = media.drain(store, tracker, now).await;
        assert_eq!((report.sent, report.dropped), (0, 0), "{case}");
        assert!(!report.needs_reauth, "{case}");
        now += 60;
    }
    assert_eq!(media.attempts(store, tracker.id()), [0, 0, 0], "{case}: nothing was counted");
    up(true);
    let mut sent = 0;
    while now < 1_000 + 86_400 + 3_600 {
        sent += media.drain(store, tracker, now).await.sent;
        now += 60;
    }
    assert_eq!(sent, 3, "{case}: every change goes once the service is back");
    sent
}

#[tokio::test]
async fn anilist_sends_the_rest_when_it_refuses_one_title() {
    for media in BOTH {
        let fake = FakeAniList::start().await;
        fake.refuse_saves_for(REFUSED.into());
        let store = Store::open_in_memory().unwrap();
        queue(&store, "anilist", media);
        refused_one(media, &store, &anilist_tracker(&fake)).await;
    }
}

#[tokio::test]
async fn anilist_down_or_unreachable_loses_nothing() {
    for failure in [Failure::Status(503), Failure::Disconnect] {
        for media in BOTH {
            let fake = FakeAniList::start().await;
            let store = Store::open_in_memory().unwrap();
            let tracker = anilist_tracker(&fake);
            let up =
                |up: bool| if up { fake.heal() } else { fake.fail_always(failure.clone()) };
            down_then_back(media, &store, &tracker, up).await;
            // Two failures in a row end a pass, and the passes back off to one an hour: a day
            // down is a few dozen saves, not three a minute.
            let saves = fake.requests_for(Operation::SaveEntry).len();
            assert!(saves < 100, "{media:?}: {saves} saves");
        }
    }
}

#[tokio::test]
async fn anilist_rejecting_the_token_holds_everything() {
    for media in BOTH {
        let fake = FakeAniList::start().await;
        fake.fail_always(Failure::Status(401));
        let store = Store::open_in_memory().unwrap();
        queue(&store, "anilist", media);
        let tracker = anilist_tracker(&fake);
        let report = media.drain(&store, &tracker, 1_000).await;
        assert!(report.needs_reauth, "{media:?}");
        assert_eq!(fake.requests_for(Operation::SaveEntry).len(), 1, "{media:?}: one ask");
        assert_eq!(media.attempts(&store, "anilist"), [0, 0, 0, 0], "{media:?}");

        fake.heal();
        sync::clear_reauth_hold(&store, "anilist").unwrap();
        fake.refuse_saves_for(REFUSED.into());
        let back = media.drain(&store, &tracker, 2_000).await;
        assert_eq!(back.sent, 3, "{media:?}: a sign-in sends what was held");
    }
}

#[tokio::test]
async fn mal_sends_the_rest_when_it_refuses_one_title() {
    for media in BOTH {
        let server = FakeReadLists::start().await;
        server.refuse(REFUSED * 10);
        let store = Store::open_in_memory().unwrap();
        queue(&store, "mal", media);
        refused_one(media, &store, &mal_tracker(&server, MAL_TOKEN)).await;
    }
}

#[tokio::test]
async fn kitsu_sends_the_rest_when_it_refuses_one_title() {
    for media in BOTH {
        let server = FakeReadLists::start().await;
        server.refuse(REFUSED * 100);
        let store = Store::open_in_memory().unwrap();
        queue(&store, "kitsu", media);
        refused_one(media, &store, &kitsu_tracker(&server, KITSU_TOKEN)).await;
    }
}

#[tokio::test]
async fn mal_and_kitsu_down_lose_nothing() {
    for media in BOTH {
        let server = FakeReadLists::start().await;
        let store = Store::open_in_memory().unwrap();
        let up = |up: bool| server.set_down(!up);
        down_then_back(media, &store, &mal_tracker(&server, MAL_TOKEN), up).await;
        down_then_back(media, &store, &kitsu_tracker(&server, KITSU_TOKEN), up).await;
        if media == Media::Watching {
            let services: Vec<String> =
                server.anime_writes().into_iter().map(|(service, _)| service).collect();
            assert_eq!(services, ["mal", "mal", "mal", "kitsu", "kitsu", "kitsu"]);
        }
    }
}

#[tokio::test]
async fn mal_and_kitsu_rejecting_the_token_hold_everything() {
    for media in BOTH {
        let server = FakeReadLists::start().await;
        for tracker in [
            &mal_tracker(&server, "stale") as &dyn Tracker,
            &kitsu_tracker(&server, "stale") as &dyn Tracker,
        ] {
            let store = Store::open_in_memory().unwrap();
            queue(&store, tracker.id(), media);
            let report = media.drain(&store, tracker, 1_000).await;
            assert!(report.needs_reauth, "{} {media:?}", tracker.id());
            assert_eq!(media.attempts(&store, tracker.id()), [0, 0, 0, 0], "{}", tracker.id());
        }
    }
}
