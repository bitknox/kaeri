//! What a front end lists and opens: the CONTINUE rail and this season, the calendar, search,
//! and a title's page.

pub mod browse;
pub mod calendar;
pub mod cards;
pub mod detail;
pub mod dub_search;
pub mod filter;
pub mod home;
pub mod lists;
pub mod probe;
pub mod recents;
pub mod search;
pub mod surprise;
pub mod trending;
pub mod up_next;

use std::time::Duration;

/// How many more goes a lookup that failed gets, each after AniList's own wait.
pub const RETRIES: u32 = 3;

/// The least a lookup waits before it goes again: short in a test, which has no AniList to wait
/// for.
#[cfg(not(feature = "testing"))]
const RETRY_AFTER: Duration = Duration::from_secs(30);
#[cfg(feature = "testing")]
const RETRY_AFTER: Duration = Duration::from_millis(150);

/// How long to wait before a lookup that failed goes again: as long as the rate limit says,
/// and at least [`RETRY_AFTER`].
pub async fn retry_wait(meta: &kaeri_meta::Meta) -> Duration {
    meta.rate_limit_wait().await.unwrap_or_default().max(RETRY_AFTER)
}
