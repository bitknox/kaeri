//! How the app's AniList client paces what it asks, against the fake: queued work spaced and
//! kept off the share left for screens, a click going out at once while the queue drains, the
//! page on screen's work ahead of the queue, and how long the client holds when the server says
//! nothing is left.

use kaeri_meta::{AniList, Pacing, in_background, on_screen};
use kaeri_testkit::{Failure, FakeAniList, Operation};

use std::time::{Duration, Instant};

/// The app's pacing with no quiet after a screen request.
const NO_QUIET: Pacing = Pacing { quiet: Duration::ZERO, ..Pacing::APP };

async fn client(pacing: Pacing) -> (FakeAniList, AniList) {
    let server = FakeAniList::start().await;
    let anilist = server.anilist_paced(&kaeri_testkit::http(), pacing);
    (server, anilist)
}

/// The words of every search the fake was asked, in the order they came.
fn asked(server: &FakeAniList) -> Vec<String> {
    server
        .requests_for(Operation::Search)
        .iter()
        .filter_map(|r| {
            r.variables.as_object()?.iter().find_map(|(name, value)| {
                name.ends_with("search").then(|| value.as_str().map(str::to_owned)).flatten()
            })
        })
        .collect()
}

async fn search(anilist: &AniList, term: &str) {
    anilist.search(term, 1, 5).await.expect("the fake answers");
}

#[tokio::test]
async fn queued_work_goes_at_its_share_of_the_limit_and_screens_go_at_once() {
    // Six thousand a minute shared sixty ways: one queued request every 0.6 s.
    let (_server, anilist) = client(Pacing { background_share: 60, ..NO_QUIET }).await;
    let started = Instant::now();
    for term in ["a", "b", "c"] {
        in_background(search(&anilist, term)).await;
    }
    assert!(started.elapsed() >= Duration::from_millis(1_200), "{:?}", started.elapsed());

    let started = Instant::now();
    for term in ["d", "e", "f"] {
        search(&anilist, term).await;
    }
    assert!(started.elapsed() < Duration::from_millis(600), "{:?}", started.elapsed());
}

#[tokio::test]
async fn a_click_goes_out_at_once_while_the_queue_drains() {
    let (server, anilist) = client(Pacing { background_share: 60, ..NO_QUIET }).await;
    let queue = tokio::spawn({
        let anilist = anilist.clone();
        async move {
            for term in ["q1", "q2", "q3", "q4"] {
                in_background(search(&anilist, term)).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(900)).await;
    let started = Instant::now();
    search(&anilist, "click").await;
    assert!(started.elapsed() < Duration::from_millis(300), "{:?}", started.elapsed());
    queue.await.unwrap();

    let order = asked(&server);
    let click = order.iter().position(|t| t == "click").unwrap();
    assert!((1..4).contains(&click), "the click went between queued requests: {order:?}");
}

#[tokio::test]
async fn queued_work_and_the_page_on_screen_leave_the_last_third_to_screens() {
    let (server, anilist) = client(NO_QUIET).await;
    // Ninety a minute, twenty of them left: less than the thirty kept for screens.
    server.say_left(90, [20]);
    search(&anilist, "first").await;

    let wait = Duration::from_millis(800);
    let queued = tokio::time::timeout(wait, in_background(search(&anilist, "queued"))).await;
    assert!(queued.is_err(), "queued work waits for the window");
    let page = tokio::time::timeout(wait, on_screen(search(&anilist, "page"))).await;
    assert!(page.is_err(), "so does the page on screen's");
    let started = Instant::now();
    search(&anilist, "click").await;
    assert!(
        started.elapsed() < Duration::from_millis(300),
        "a screen goes: {:?}",
        started.elapsed()
    );
    assert_eq!(asked(&server), ["first", "click"]);
}

#[tokio::test]
async fn the_page_on_screen_skips_the_quiet_that_holds_the_queue() {
    let (server, anilist) =
        client(Pacing { quiet: Duration::from_secs(1), ..Pacing::APP }).await;
    search(&anilist, "screen").await;
    let queued = tokio::spawn({
        let anilist = anilist.clone();
        async move { in_background(search(&anilist, "queued")).await }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let started = Instant::now();
    on_screen(search(&anilist, "page")).await;
    assert!(started.elapsed() < Duration::from_millis(300), "{:?}", started.elapsed());
    search(&anilist, "click").await;
    queued.await.unwrap();
    assert_eq!(asked(&server), ["screen", "page", "click", "queued"]);
}

#[tokio::test]
async fn nothing_left_said_once_paces_and_said_twice_holds_until_the_window_ends() {
    let (server, anilist) = client(NO_QUIET).await;
    // The window begins with a full count; one node then says nothing is left.
    server.say_left(90, [89, 0]);
    search(&anilist, "a").await;
    search(&anilist, "b").await;
    let wait = anilist.rate_limit_wait().await.unwrap_or_default();
    assert!(wait < Duration::from_secs(2), "said once, the client paces: {wait:?}");

    // Said again in the same window: held until it ends, a minute and a second from its start.
    server.say_left(90, [0]);
    search(&anilist, "c").await;
    let wait = anilist.rate_limit_wait().await.unwrap_or_default();
    assert!(
        (Duration::from_secs(55)..=Duration::from_secs(61)).contains(&wait),
        "held to the window's end: {wait:?}"
    );
}

#[tokio::test]
async fn with_no_window_known_nothing_left_twice_holds_a_minute() {
    let (server, anilist) = client(NO_QUIET).await;
    server.say_left(90, [50, 0, 0]);
    for term in ["a", "b", "c"] {
        search(&anilist, term).await;
    }
    let wait = anilist.rate_limit_wait().await.unwrap_or_default();
    assert!(
        (Duration::from_secs(58)..=Duration::from_secs(60)).contains(&wait),
        "a minute: {wait:?}"
    );
}

#[tokio::test]
async fn a_refusal_waits_as_it_says_and_begins_the_next_window() {
    let (server, anilist) = client(NO_QUIET).await;
    server.fail_next(Failure::RateLimited { retry_after_secs: 1 });
    let started = Instant::now();
    search(&anilist, "a").await;
    assert!(started.elapsed() >= Duration::from_secs(1), "{:?}", started.elapsed());
    // The first zero of the new window paces, as any first zero does.
    server.say_left(90, [0]);
    search(&anilist, "b").await;
    let wait = anilist.rate_limit_wait().await.unwrap_or_default();
    assert!(wait < Duration::from_secs(5), "{wait:?}");
}
