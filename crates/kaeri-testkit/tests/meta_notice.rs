//! What the metadata ladder says about itself, the one-line notice the apps toast, through the
//! fake AniList: said when it is true, by what someone is looking at, and once.

use kaeri_core::ids::AnilistId;
use kaeri_meta::in_background;
use kaeri_testkit::{
    Failure, MetaKit, Operation,
    fixtures::{AIRING, FINISHED, MOVIE, SEQUEL},
};

const CACHED: &str = "no metadata source answered — showing what was last cached";
const BACK: &str = "anilist is back";
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
