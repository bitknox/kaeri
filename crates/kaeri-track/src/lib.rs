//! Tracker sync.
//!
//! **Local is the source of truth; sync is a projection of it.** The app has to be fully usable
//! with no account and no network, so nothing about history depends on a tracker existing. What
//! goes outward is the small syncable part — progress, status, score — and it goes through a
//! durable queue rather than a live call, because progress recorded on a plane has to survive
//! the process exiting.
//!
//! The layout follows that split:
//!
//! - [`merge`] decides what to do when the two sides disagree. Pure, and the most heavily
//!   tested module here, because this is the code that can overwrite someone's list.
//! - [`sync`] drains the outbox and reconciles a pulled library. Boring under failure by
//!   design.
//! - [`auth`] gets a token. See its docs for why the plan's PKCE flow does not work against
//!   AniList.
//! - [`secret`] keeps the token in the OS keychain, or a `0600` file where there is none.
//! - [`anilist`] is the reference [`Tracker`](kaeri_core::traits::Tracker) implementation.
//!
//! Adding a second tracker needs an auth flow and a `Tracker` impl — the mapping layer already
//! carries `mal_id`, `kitsu_id` and the rest, so identity is a solved problem rather than a new
//! subsystem.

pub mod anilist;
pub mod auth;
pub mod device;
mod held;
pub mod kitsu;
pub mod lookup;
pub mod mal;
pub mod mangaupdates;
pub mod merge;
pub mod read;
pub mod secret;
pub mod simkl;
pub mod sync;
pub mod trakt;

pub use anilist::AniListTracker;
pub use auth::{AuthError, Flow};
pub use device::{DeviceCode, DeviceEndpoints, DeviceError, DeviceFlow};
pub use kitsu::KitsuTracker;
pub use mal::MalTracker;
pub use mangaupdates::MangaUpdatesTracker;
pub use merge::{Conflict, Field, LocalState, Merged};
pub use secret::{Storage, TokenPair, TokenStore};
pub use simkl::SimklTracker;
pub use sync::{DrainReport, PullReport, drain, pull, queue_progress};
pub use trakt::{SeasonMapping, TraktTracker};

/// What the project's requests call themselves, as `kaeri-net`'s plain client does.
pub(crate) const USER_AGENT: &str = concat!("kaeri/", env!("CARGO_PKG_VERSION"));

/// Seconds since the Unix epoch.
///
/// Duplicated rather than taken from the store, so this crate stays independent of it.
pub fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// A tracker's answer as JSON, or the error it amounts to.
///
/// A failed answer is the error [`answer_error`] makes of it. Reading a failure's body as if it
/// were a page would find no entries in it, and an outage would pass for an empty library. An
/// answer that says success and cannot be read is the tracker's trouble, not a refusal.
///
/// `tracker` is the id errors carry (`mal`), `name` what the person reads (`myanimelist`).
pub(crate) async fn page_json(
    response: reqwest::Response,
    tracker: &str,
    name: &str,
) -> Result<serde_json::Value, kaeri_core::Error> {
    if !response.status().is_success() {
        return Err(failed_answer(response, tracker, name).await);
    }
    response.json().await.map_err(|e| kaeri_core::Error::Unavailable {
        tracker: tracker.to_owned(),
        message: e.to_string(),
    })
}

/// The error a tracker's failed answer amounts to, read from its status, with the body for
/// the reason.
pub(crate) async fn failed_answer(
    response: reqwest::Response,
    tracker: &str,
    name: &str,
) -> kaeri_core::Error {
    let status = response.status();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let body = response.text().await.unwrap_or_default();
    answer_error(tracker, name, status, retry_after, &body)
}

/// What a tracker's failed answer means, by its status.
///
/// The outbox acts on the difference, so it is drawn with care. A 401 is an auth error, which
/// only signing in again fixes. A 429 is a pause the tracker asked for. A 408, a 5xx or a
/// maintenance page is the tracker's own trouble, to wait out. Only what is left, a 4xx about
/// the request itself (an entry gone, a value refused), is a refusal that the same request
/// would get again, and only a refusal ever brings a queued change nearer its end.
pub(crate) fn answer_error(
    tracker: &str,
    name: &str,
    status: reqwest::StatusCode,
    retry_after: Option<u64>,
    body: &str,
) -> kaeri_core::Error {
    let tracker = tracker.to_owned();
    let message = said(status, body);
    match status.as_u16() {
        401 => kaeri_core::Error::Auth(format!("{name} rejected the token")),
        429 => kaeri_core::Error::RateLimited { tracker, message, retry_in_secs: retry_after },
        408 | 500.. => kaeri_core::Error::Unavailable { tracker, message },
        _ => kaeri_core::Error::Tracker { tracker, message },
    }
}

/// A request that never got an answer: the network, not the tracker.
pub(crate) fn unreachable(name: &str, error: &reqwest::Error) -> kaeri_core::Error {
    kaeri_core::Error::Network(format!("{name}: {error}"))
}

/// What became of each title of a push that writes them one at a time, kept as it goes.
///
/// A refusal is the title's own, and the next title goes. A rejected credential or a pause
/// the tracker asked for stops the push, and every title not yet tried waits with it. The
/// tracker's own trouble is the title's alone the first time: a second in a row, with nothing
/// answered between, is the tracker being down, and the push stops there rather than send
/// every remaining title into it.
#[derive(Debug, Default)]
pub(crate) struct Titles {
    pushed: kaeri_core::traits::Pushed,
    /// Titles in a row the tracker failed on its side.
    unavailable: u32,
    stopped: Option<kaeri_core::traits::PushFailure>,
}

impl Titles {
    /// Whether the next title is to be tried. When not, [`Self::record`] files it with the
    /// failure that stopped the push.
    pub(crate) const fn go(&self) -> bool {
        self.stopped.is_none()
    }

    /// What became of `id`: its write's result, or, once stopped, the reason.
    pub(crate) fn record(
        &mut self,
        id: kaeri_core::ids::AnilistId,
        result: Result<(), kaeri_core::Error>,
    ) {
        self.settle(id, result.err().map(|error| kaeri_core::traits::PushFailure::of(&error)));
    }

    /// [`Self::record`] for an outcome already classified: `None` when it was applied.
    pub(crate) fn settle(
        &mut self,
        id: kaeri_core::ids::AnilistId,
        failure: Option<kaeri_core::traits::PushFailure>,
    ) {
        use kaeri_core::traits::PushFailure;
        if let Some(stopped) = &self.stopped {
            self.pushed.failed.push((id, stopped.clone()));
            return;
        }
        let Some(failure) = failure else {
            self.unavailable = 0;
            return;
        };
        match &failure {
            PushFailure::Refused(_) => self.unavailable = 0,
            PushFailure::Auth(_) | PushFailure::RateLimited { .. } => {
                self.stopped = Some(failure.clone());
            }
            PushFailure::Unavailable(_) => {
                self.unavailable += 1;
                if self.unavailable >= 2 {
                    self.stopped = Some(failure.clone());
                }
            }
        }
        self.pushed.failed.push((id, failure));
    }

    /// A title the tracker answered for without applying anything.
    pub(crate) fn ignore(&mut self, id: kaeri_core::ids::AnilistId) {
        self.pushed.ignored.push(id);
    }

    pub(crate) fn finish(self) -> kaeri_core::traits::Pushed {
        self.pushed
    }
}

/// What a tracker's refusal says, for a person: the status, and the first sentence of the reason
/// when the body is JSON that gives one. Never the body itself, which can be a whole HTML page.
pub(crate) fn said(status: reqwest::StatusCode, body: &str) -> String {
    match reason_in(body) {
        Some(reason) => format!("{status}: {reason}"),
        None => status.to_string(),
    }
}

/// The first sentence of the reason a JSON error body gives: OAuth's `error_description`, MAL's
/// `hint`, a `message`, the first of an `errors` list, or the bare `error` code last.
fn reason_in(body: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(body).ok()?;
    let first = &parsed["errors"][0];
    let reason = [
        &parsed["error_description"],
        &parsed["hint"],
        &parsed["message"],
        &first["detail"],
        &first["message"],
        &first["title"],
        &parsed["error"],
    ]
    .into_iter()
    .find_map(|said| said.as_str().map(str::trim).filter(|said| !said.is_empty()))?;
    // One sentence, and not a long one.
    let end = reason.find(['\n', '\r']).unwrap_or(reason.len());
    let line = &reason[..end];
    let sentence = line.find(". ").map_or(line, |at| &line[..=at]);
    Some(sentence.chars().take(160).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(status: u16, body: &'static str) -> reqwest::Response {
        http::Response::builder().status(status).body(body).unwrap().into()
    }

    #[tokio::test]
    async fn a_maintenance_page_is_an_error_not_an_empty_library() {
        let body = r#"{"error":"maintenance","message":"temporarily in maintenance mode"}"#;
        let err = page_json(answer(503, body), "mal", "myanimelist").await.unwrap_err();
        // The tracker's own trouble, never a refusal: a queued change waits it out.
        let kaeri_core::Error::Unavailable { tracker, message } = err else {
            panic!("expected the tracker to be unavailable, got {err:?}");
        };
        assert_eq!(tracker, "mal");
        assert!(message.starts_with("503"), "{message}");
        assert!(message.contains("maintenance"), "{message}");
    }

    #[tokio::test]
    async fn a_rate_limit_is_an_error_too() {
        let response: reqwest::Response = http::Response::builder()
            .status(429)
            .header("retry-after", "30")
            .body("slow down")
            .unwrap()
            .into();
        let err = page_json(response, "kitsu", "kitsu").await.unwrap_err();
        assert!(
            matches!(err, kaeri_core::Error::RateLimited { retry_in_secs: Some(30), .. }),
            "{err:?}"
        );
    }

    #[test]
    fn only_a_refusal_of_the_request_itself_is_a_refusal() {
        use kaeri_core::traits::PushFailure;
        let class = |status: u16| {
            let status = reqwest::StatusCode::from_u16(status).unwrap();
            PushFailure::of(&answer_error("mal", "myanimelist", status, None, ""))
        };
        for refused in [400, 403, 404, 409, 410, 422] {
            assert!(matches!(class(refused), PushFailure::Refused(_)), "{refused}");
        }
        for waits in [408, 500, 502, 503, 504, 522] {
            assert!(matches!(class(waits), PushFailure::Unavailable(_)), "{waits}");
        }
        assert!(matches!(class(429), PushFailure::RateLimited { .. }));
        assert!(matches!(class(401), PushFailure::Auth(_)));
    }

    #[tokio::test]
    async fn an_unreadable_success_is_the_trackers_trouble_not_a_refusal() {
        let err = page_json(answer(200, "<html>"), "kitsu", "kitsu").await.unwrap_err();
        assert!(matches!(err, kaeri_core::Error::Unavailable { .. }), "{err:?}");
    }

    #[test]
    fn a_title_loop_goes_past_a_refusal_and_stops_at_an_outage() {
        use kaeri_core::{ids::AnilistId, traits::PushFailure};
        let id = AnilistId::new;
        let refused =
            || Err(kaeri_core::Error::Tracker { tracker: "t".into(), message: "gone".into() });
        let down = || Err(kaeri_core::Error::Network("dns".into()));
        let mut titles = Titles::default();
        titles.record(id(1), refused());
        titles.record(id(2), Ok(()));
        titles.record(id(3), down());
        assert!(titles.go(), "one failure on the tracker's side may be the title's own");
        titles.record(id(4), Ok(()));
        titles.record(id(5), down());
        titles.record(id(6), down());
        assert!(!titles.go(), "two in a row is the tracker being down");
        titles.record(id(7), Ok(()));
        let pushed = titles.finish();
        assert!(matches!(pushed.failure_for(id(1)), Some(PushFailure::Refused(_))));
        assert_eq!(pushed.failure_for(id(2)), None);
        assert!(matches!(pushed.failure_for(id(3)), Some(PushFailure::Unavailable(_))));
        assert_eq!(pushed.failure_for(id(4)), None);
        assert!(matches!(pushed.failure_for(id(7)), Some(PushFailure::Unavailable(_))));

        let mut titles = Titles::default();
        titles.record(id(1), Err(kaeri_core::Error::Auth("expired".into())));
        assert!(!titles.go(), "a rejected credential stops the push");
        titles.record(id(2), Ok(()));
        assert!(matches!(titles.finish().failure_for(id(2)), Some(PushFailure::Auth(_))));
    }

    #[tokio::test]
    async fn a_rejected_token_is_an_auth_error() {
        let err = page_json(answer(401, "{}"), "mal", "myanimelist").await.unwrap_err();
        let kaeri_core::Error::Auth(message) = err else { panic!("{err:?}") };
        assert_eq!(message, "myanimelist rejected the token");
    }

    #[tokio::test]
    async fn a_good_answer_is_its_json() {
        let page = page_json(answer(200, r#"{"data":[]}"#), "simkl", "simkl").await.unwrap();
        assert!(page["data"].as_array().is_some_and(Vec::is_empty));
    }

    #[tokio::test]
    async fn a_long_error_body_is_cut_short() {
        let body: &'static str = Box::leak("x".repeat(5_000).into_boxed_str());
        let err = page_json(answer(500, body), "mal", "myanimelist").await.unwrap_err();
        let kaeri_core::Error::Unavailable { message, .. } = err else { unreachable!() };
        assert!(message.len() < 300, "{}", message.len());
    }

    #[test]
    fn a_refusal_is_its_status_and_one_sentence_of_why_never_a_page() {
        use reqwest::StatusCode;
        let page = "<!DOCTYPE html><html><body><h1>503 Service Unavailable</h1></body></html>";
        assert_eq!(said(StatusCode::SERVICE_UNAVAILABLE, page), "503 Service Unavailable");
        let oauth = r#"{"error":"invalid_grant","error_description":"The code has expired. Ask for a new one.\nTrace 42"}"#;
        assert_eq!(
            said(StatusCode::BAD_REQUEST, oauth),
            "400 Bad Request: The code has expired."
        );
        let mal =
            r#"{"error":"invalid_request","hint":"Cannot decrypt the authorization code"}"#;
        assert_eq!(
            said(StatusCode::BAD_REQUEST, mal),
            "400 Bad Request: Cannot decrypt the authorization code"
        );
        let listed = r#"{"errors":[{"title":"Forbidden","detail":"Not yours to change."}]}"#;
        assert_eq!(said(StatusCode::FORBIDDEN, listed), "403 Forbidden: Not yours to change.");
        assert_eq!(
            said(StatusCode::BAD_REQUEST, r#"{"error":"invalid_client"}"#),
            "400 Bad Request: invalid_client"
        );
        assert_eq!(said(StatusCode::INTERNAL_SERVER_ERROR, ""), "500 Internal Server Error");
    }
}
