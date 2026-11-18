//! What the metadata ladder says about itself, the one-line notice the apps toast, through the
//! fake AniList: said when it is true, by what someone is looking at, and once.

use kaeri_core::ids::AnilistId;
use kaeri_meta::{AniList, BrowseFilter, Meta, Tenrai, in_background};
use kaeri_store::Store;
use kaeri_testkit::{
    Failure, FakeAniList, MetaKit, Operation, TEST_RATE_LIMIT,
    fixtures::{AIRING, FINISHED, MOVIE, SEQUEL},
    seed,
};

const CACHED: &str = "no metadata source answered — showing what was last cached";
const BACK: &str = "anilist is back";
const UNREACHABLE: &str = "anilist is unreachable — fresh data via tenrai for now";
const IDS: [AnilistId; 3] = [FINISHED, SEQUEL, AIRING];

/// A ladder whose titles an earlier answer cached, so the cache rung has something to show.
async fn cached() -> MetaKit {
    let kit = MetaKit::start().await;
    let found = kit.meta.media_many(&IDS).await.expect("the fake answers");
    assert_eq!(found.len(), IDS.len());
    assert_eq!(kit.meta.take_notice(), None, "a fresh answer from a fresh start says nothing");
    kit
}

/// What the engine would toast now.
fn said(kit: &MetaKit) -> Vec<String> {
    kit.meta.take_notice().into_iter().collect()
}

/// The launch queue's batch refused while the screen's asks are answered: what Android showed as
/// "No metadata service answered" over fresh data, and again after each "AniList is back".
#[tokio::test]
async fn a_refused_background_batch_says_nothing_while_the_screen_is_answered() {
    for refusal in [
        Failure::GraphQl("Max query complexity exceeded".into()),
        Failure::RateLimited { retry_after_secs: 0 },
    ] {
        let kit = cached().await;
        kit.server.fail_operation(Operation::Many, refusal.clone());

        let mut toasts = Vec::new();
        let kept = in_background(kit.meta.media_many(&IDS)).await;
        assert_eq!(
            kept.map(|m| m.len()).ok(),
            Some(IDS.len()),
            "{refusal:?}: the cache stands in"
        );
        toasts.extend(said(&kit));
        kit.meta.media(MOVIE).await.expect("the page is answered");
        toasts.extend(said(&kit));
        // A rate limit is waited out three times before it gives up, so it goes round once.
        if matches!(refusal, Failure::GraphQl(_)) {
            let _ = in_background(kit.meta.media_many(&IDS)).await;
            toasts.extend(said(&kit));
        }
        assert_eq!(toasts, Vec::<String>::new(), "{refusal:?}: AniList answered every request");
    }
}

/// A rate limit or GraphQL's refusal on a screen's own ask is an answer too: the cache stands in,
/// nothing claims AniList is down, and AniList is never skipped for it.
#[tokio::test]
async fn a_refusal_on_screen_falls_to_the_cache_without_saying_anilist_is_down() {
    // A rate limit is waited out three times before it gives up, so it goes round once; the
    // breaker's own tests hold it to never tripping.
    for (refusal, rounds) in [
        (Failure::GraphQl("Max query complexity exceeded".into()), 4),
        (Failure::RateLimited { retry_after_secs: 0 }, 1),
    ] {
        let kit = cached().await;
        kit.server.fail_operation(Operation::Many, refusal.clone());
        for _ in 0..rounds {
            let kept = kit.meta.media_many(&IDS).await;
            assert_eq!(kept.map(|m| m.len()).ok(), Some(IDS.len()), "{refusal:?}");
            assert_eq!(said(&kit), Vec::<String>::new(), "{refusal:?}");
        }
        kit.server.heal();
        kit.server.clear_requests();
        kit.meta.media_many(&IDS).await.expect("answered again");
        assert_eq!(
            kit.server.requests_for(Operation::Many).len(),
            1,
            "{refusal:?}: still asked"
        );
        assert_eq!(said(&kit), Vec::<String>::new(), "{refusal:?}: nothing to come back from");
    }
}

/// A real outage on a screen is said once, and so is the recovery, from whichever lane AniList
/// answers first.
#[tokio::test]
async fn an_outage_on_screen_is_said_once_and_so_is_the_recovery() {
    for failure in [Failure::Status(500), Failure::Status(503), Failure::Disconnect] {
        let kit = cached().await;
        kit.server.fail_operation(Operation::Many, failure.clone());
        let mut toasts = Vec::new();
        for _ in 0..2 {
            let kept = kit.meta.media_many(&IDS).await;
            assert_eq!(kept.map(|m| m.len()).ok(), Some(IDS.len()), "{failure:?}");
            toasts.extend(said(&kit));
        }
        assert_eq!(toasts, [CACHED], "{failure:?}");

        kit.server.heal();
        in_background(kit.meta.media(MOVIE)).await.expect("AniList answers again");
        assert_eq!(said(&kit), [BACK], "{failure:?}");
        kit.meta.media_many(&IDS).await.expect("answered");
        assert_eq!(said(&kit), Vec::<String>::new(), "{failure:?}: said once");
    }
}

/// Background work that meets an outage says nothing until AniList has failed often enough to be
/// skipped: then it is a state, not one request, and the screens are on the cache too.
#[tokio::test]
async fn a_background_outage_is_said_once_anilist_is_skipped() {
    let kit = cached().await;
    kit.server.fail_operation(Operation::Many, Failure::Status(503));
    let mut toasts = Vec::new();
    for _ in 0..3 {
        let kept = in_background(kit.meta.media_many(&IDS)).await;
        assert_eq!(kept.map(|m| m.len()).ok(), Some(IDS.len()));
        toasts.push(said(&kit));
    }
    assert_eq!(toasts, [vec![], vec![], vec![CACHED.to_owned()]]);
    assert_eq!(kit.meta.health()[0].state, "backing off");
}

/// An address nothing listens on, so a connection there is refused.
fn refusing() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let address = listener.local_addr().expect("its address");
    drop(listener);
    address.to_string()
}

/// A ladder over `store` asking AniList at `anilist` and Tenrai at `tenrai`, through one
/// client, as the app asks both, with every fixture title mapped so Tenrai is asked for each.
fn ladder(store: &Store, anilist: String, tenrai: Option<String>) -> Meta {
    seed::mappings(store);
    let http = kaeri_testkit::http();
    Meta::new(
        AniList::new(http.clone(), TEST_RATE_LIMIT).with_endpoint(anilist),
        tenrai.map(|at| Tenrai::new(http, store.clone()).with_endpoint(at)),
        store.clone(),
    )
}

/// Each title once from `meta`, kept, and what it said meanwhile.
async fn ask_each(meta: &Meta) -> Vec<String> {
    let mut toasts = Vec::new();
    for id in IDS {
        let kept = meta.media(id).await;
        assert_eq!(kept.map(|m| m.id).ok(), Some(id), "the cache stands in");
        toasts.extend(meta.take_notice());
    }
    toasts
}

/// A device that lost its path fails every host together: AniList and a second host both
/// refusing is the connection, not AniList, and the app's offline state is what says it. The
/// breaker still rests AniList for its cooldown.
#[tokio::test]
async fn every_host_refusing_together_blames_no_source() {
    let kit = cached().await;
    let meta = ladder(
        &kit.store,
        format!("http://{}/graphql", refusing()),
        Some(format!("http://{}/tenrai/v1", refusing())),
    );
    let mut toasts = Vec::new();
    for _ in 0..3 {
        toasts.extend(ask_each(&meta).await);
    }
    assert_eq!(toasts, Vec::<String>::new());
    assert_eq!(meta.health()[0].state, "backing off");
}

/// AniList refusing while another host answers is AniList's outage, said as ever.
#[tokio::test]
async fn anilist_refusing_while_another_host_answers_is_said() {
    let kit = cached().await;
    let meta = ladder(
        &kit.store,
        format!("http://{}/graphql", refusing()),
        Some(kit.server.tenrai_endpoint()),
    );
    let found = meta.find("wayfarer", &BrowseFilter::default(), 1, 20).await;
    assert!(found.is_ok(), "Tenrai answers: {found:?}");
    assert_eq!(said_by(&meta), [UNREACHABLE]);

    // Tenrai knows none of the titles, but its answer still says the device is online.
    let meta = ladder(
        &kit.store,
        format!("http://{}/graphql", refusing()),
        Some(kit.server.tenrai_endpoint()),
    );
    assert_eq!(ask_each(&meta).await, [CACHED]);
}

/// The platform saying there is no path is enough, with no other host asked.
#[tokio::test]
async fn no_path_from_the_platform_blames_no_source() {
    let kit = cached().await;
    let meta = ladder(&kit.store, format!("http://{}/graphql", refusing()), None);
    meta.path(true);
    assert_eq!(ask_each(&meta).await, Vec::<String>::new());

    // With the path back and no other host asked, AniList refusing is AniList's, as before.
    meta.path(false);
    let meta = ladder(&kit.store, format!("http://{}/graphql", refusing()), None);
    assert_eq!(ask_each(&meta).await, [CACHED]);
}

/// The path comes back while AniList's breaker still rests it: the other host answers first, and
/// AniList, skipped, is not said to be down for the outage that was the device's. The asks
/// between, which only AniList heard, are weighed as the one before them was.
#[tokio::test]
async fn a_skip_after_the_path_came_back_blames_no_source() {
    let kit = cached().await;
    let other = FakeAniList::start().await;
    kit.server.fail_always(Failure::Disconnect);
    other.fail_operation(Operation::Tenrai, Failure::Disconnect);
    let meta = ladder(&kit.store, kit.server.endpoint(), Some(other.tenrai_endpoint()));
    let kept = meta.media(FINISHED).await;
    assert_eq!(kept.map(|m| m.id).ok(), Some(FINISHED));
    let mut toasts: Vec<String> = meta.take_notice().into_iter().collect();
    // Every title is kept, so these ask AniList alone, until it is skipped.
    for _ in 0..3 {
        let kept = meta.media_many(&IDS).await;
        assert_eq!(kept.map(|m| m.len()).ok(), Some(IDS.len()));
        toasts.extend(meta.take_notice());
    }
    assert_eq!(meta.health()[0].state, "backing off");

    other.heal();
    let found = meta.find("wayfarer", &BrowseFilter::default(), 1, 20).await;
    assert!(found.is_ok(), "Tenrai answers again: {found:?}");
    toasts.extend(meta.take_notice());
    assert_eq!(toasts, Vec::<String>::new());
}

fn said_by(meta: &Meta) -> Vec<String> {
    meta.take_notice().into_iter().collect()
}
