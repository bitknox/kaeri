//! An ask for the list made while a background pull's request waits its turn in the background
//! queue, with the viewer's and the list's locks held.

use std::{sync::Arc, time::Duration};

use kaeri_core::config::NetworkConfig;
use kaeri_meta::Pacing;
use kaeri_net::HttpClient;
use kaeri_testkit::anilist::FakeAniList;
use kaeri_track::AniListTracker;

#[tokio::test]
async fn the_library_screen_raises_the_background_pull_it_waits_on() {
    let fake = FakeAniList::start().await;
    let http = HttpClient::new(&NetworkConfig::default()).unwrap();
    // Background work held back for a minute, as a long background queue holds a pull.
    let pacing = Pacing { launch_at_most: Duration::from_secs(60), ..Pacing::UNPACED };
    let client = fake.anilist_paced(&http, pacing).with_token(Some("token".into()));
    client.launching();
    let tracker = Arc::new(AniListTracker::new(client));

    let pull = tokio::spawn({
        let tracker = Arc::clone(&tracker);
        async move { kaeri_meta::in_background(tracker.library()).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!pull.is_finished(), "the pull waits in the background queue");

    let screen = tokio::time::timeout(Duration::from_secs(5), tracker.library()).await;
    screen.expect("the screen does not wait out the background queue").unwrap();
    let pulled = tokio::time::timeout(Duration::from_secs(5), pull).await;
    pulled.expect("the pull the screen waited on went with it").unwrap().unwrap();
}
