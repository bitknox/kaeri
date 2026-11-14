//! Trakt.
//!
//! The awkward one, and worth being honest about why rather than pretending otherwise: **Trakt is
//! not an anime tracker.** It tracks television, and anime appears in it as ordinary shows keyed on
//! TVDB and TMDB ids. That has two consequences this module cannot engineer away:
//!
//! - **Identity comes from `tvdb_id`.** Measured against the real 22,374-title mapping table:
//!   20,625 titles carry a `mal_id` (which is what AniList, MAL and Simkl key on) and only **7,225
//!   carry a TVDB id**. Two thirds of the corpus cannot reach Trakt at all.
//! - **Episode numbering is per-season, and usually unknowable.** Trakt wants `S2E03`; Kaeri
//!   counts absolutely, because AniList splits cours that way. The
//!   datasets' `episode_offset` bridges it — but of the 4,196 TVDB-mapped entries that *share* a
//!   series with another entry, only 590 have one. So for **3,606 titles** there is no way to say
//!   which season an episode belongs to.
//!
//! That second number is why this module refuses rather than assuming season one. Writing absolute
//! episode numbers into a first season would mark unrelated episodes as watched on somebody's real
//! account, and silently: 26 episodes into a third season would land on the first season's finale.
//! A skipped push is recoverable and visible in the log; wrong data is neither.
//!
//! Net effect: Trakt works correctly for roughly **3,600 titles** — the single-season ones — and
//! declines the rest. It is worth having if you already live in Trakt. It is not a replacement for
//! AniList, MAL or Simkl, all three of which cover 92% of the same corpus with no ambiguity.
//!
//! A sign-in comes with a refresh token, and its access token runs out. It is renewed an hour
//! before then, and again when Trakt rejects it, kept in step with the other processes signed in
//! through the same store: see [`crate::held`]. Only a renewal Trakt refuses asks for a new
//! sign-in.

use std::sync::Arc;

use kaeri_core::{
    ids::AnilistId,
    traits::{TrackOp, TrackedEntry, Tracker, WatchStatus},
};

use crate::{
    device::{self, DeviceEndpoints},
    held::{Held, Renews},
    secret::{TokenPair, TokenStore},
};

const API: &str = "https://api.trakt.tv";
/// How long before its access token runs out a sign-in is renewed. An access token can last as
/// little as a day, which [`crate::held::MARGIN_SECS`] would renew on every call.
const RENEW_MARGIN_SECS: i64 = 3_600;
/// Trakt requires this header and rejects requests without it.
const API_VERSION: &str = "2";

/// What Trakt needs to identify an episode: a show, and a season-relative number.
pub trait SeasonMapping: Send + Sync {
    /// The TVDB id for a title, if the mapping layer knows one.
    fn tvdb_id(&self, anilist_id: AnilistId) -> Option<u32>;

    /// Turn an absolute episode number into `(season, episode)`, or `None` if it cannot be placed.
    ///
    /// **`None` is not an edge case here, it is half the corpus.** Measured against the real mapping
    /// table: 7,225 titles carry a TVDB id, 4,196 of them share that id with another AniList entry —
    /// because TVDB keeps a sequel in the same series where AniList splits it — and only 590 of those
    /// have the `episode_offset` needed to say which season they are. That leaves 3,606 titles where
    /// assuming season 1 would write absolute episode numbers into the first season: watching S3E02
    /// would mark something entirely different as seen.
    ///
    /// So an ambiguous title is refused rather than guessed. A skipped push is recoverable; wrong
    /// data written to somebody's account is not.
    fn season_episode(&self, anilist_id: AnilistId, absolute: u32) -> Option<(u32, u32)>;

    /// The reverse, for reading a library back.
    fn absolute_episode(&self, anilist_id: AnilistId, season: u32, episode: u32) -> u32;

    fn anilist_for_tvdb(&self, tvdb_id: u32) -> Option<AnilistId>;
}

pub struct TraktTracker {
    client_id: String,
    /// Sent with a renewal by an app Trakt issued one to.
    client_secret: Option<String>,
    http: reqwest::Client,
    mapping: Arc<dyn SeasonMapping>,
    /// The pair, renewed before it runs out and kept in step with other processes: see
    /// [`crate::held`].
    held: Held,
    /// Where a sign-in is renewed: [`device::TRAKT_RENEW`], except in tests.
    endpoints: DeviceEndpoints,
    /// [`API`], except in tests, which point it at a local server.
    api: String,
}

impl TraktTracker {
    pub fn new(
        client_id: impl Into<String>,
        http: reqwest::Client,
        mapping: Arc<dyn SeasonMapping>,
        token: Option<TokenPair>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: None,
            http,
            mapping,
            held: Held::new("trakt", "trakt", token).with_margin(RENEW_MARGIN_SECS),
            endpoints: device::TRAKT_RENEW,
            api: API.to_owned(),
        }
    }

    /// The app's secret, where Trakt issued one: its renewals send it.
    #[must_use]
    pub fn with_client_secret(mut self, secret: Option<String>) -> Self {
        self.client_secret = secret.filter(|s| !s.trim().is_empty());
        self
    }

    /// Kept in step with the other processes signed in through `store`. A renewal replaces the
    /// refresh token too, so without this the terminal and the app would end each other's.
    #[must_use]
    pub fn with_token_store(mut self, store: TokenStore) -> Self {
        self.held.share(store);
        self
    }

    /// A renewed token pair the caller should write back, if a renewal happened and no store
    /// was given, or the store did not take it.
    pub async fn take_renewed(&self) -> Option<TokenPair> {
        self.held.take_renewed().await
    }

    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        token: &str,
    ) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.api))
            .header("trakt-api-key", &self.client_id)
            .header("trakt-api-version", API_VERSION)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
    }
}

impl Renews for TraktTracker {
    async fn renew(&self, refresh: &str, now: i64) -> Result<TokenPair, kaeri_core::Error> {
        device::refresh_token(
            &self.http,
            &self.endpoints,
            &self.client_id,
            self.client_secret.as_deref(),
            refresh,
            now,
        )
        .await
        .map_err(|e| e.renewal("trakt"))
    }
}

#[async_trait::async_trait]
impl Tracker for TraktTracker {
    fn id(&self) -> &str {
        "trakt"
    }

    fn is_authenticated(&self) -> bool {
        self.held.is_held()
    }

    /// Progress needs a TVDB id and a season to put the episode in; both come from the
    /// datasets, so a later refresh can supply either. Status and score are never sent to
    /// Trakt, and saying they are addressable lets the push discard them as it always has.
    fn can_address(&self, op: &TrackOp) -> bool {
        match op {
            TrackOp::SetProgress { anilist_id, episode } => {
                self.mapping.tvdb_id(*anilist_id).is_some()
                    && self.mapping.season_episode(*anilist_id, *episode).is_some()
            }
            TrackOp::SetStatus { .. }
            | TrackOp::SetScore { .. }
            | TrackOp::SetReadProgress { .. }
            | TrackOp::SetReadStatus { .. }
            | TrackOp::SetReadScore { .. } => true,
        }
    }

    async fn accept_credentials(
        &self,
        access: &str,
        refresh: Option<&str>,
        expires_at: Option<i64>,
    ) {
        self.held
            .accept(TokenPair {
                access: access.to_owned(),
                refresh: refresh.map(str::to_owned),
                expires_at,
            })
            .await;
    }

    async fn forget_credentials(&self) {
        self.held.forget().await;
    }

    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, kaeri_core::Error> {
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        let response = self
            .held
            .send(self, &mut token, |t| {
                self.request(reqwest::Method::GET, "/sync/watched/shows", t)
            })
            .await?;

        let body = crate::page_json(response, "trakt", "trakt").await?;

        let mut entries = Vec::new();
        for item in body.as_array().into_iter().flatten() {
            let Some(tvdb_id) = item["show"]["ids"]["tvdb"].as_u64() else { continue };
            let Some(anilist_id) = self.mapping.anilist_for_tvdb(tvdb_id as u32) else {
                tracing::debug!(tvdb_id, "trakt show has no anilist mapping; skipped");
                continue;
            };

            // Trakt reports per-season episode lists. The highest watched episode across every
            // season, converted back to absolute numbering, is the progress figure — taking the
            // count instead would be wrong for a show with gaps.
            let mut progress = 0;
            for season in item["seasons"].as_array().into_iter().flatten() {
                let number = season["number"].as_u64().unwrap_or(1) as u32;
                for episode in season["episodes"].as_array().into_iter().flatten() {
                    let watched = episode["number"].as_u64().unwrap_or(0) as u32;
                    progress = progress
                        .max(self.mapping.absolute_episode(anilist_id, number, watched));
                }
            }

            entries.push(TrackedEntry {
                anilist_id,
                progress,
                // `/sync/watched` says what has been seen, not which list it is on. Reporting
                // `Current` is the honest reading; guessing `Completed` from an episode count would
                // need a total Trakt does not give here.
                status: WatchStatus::Current,
                score: None,
            });
        }
        tracing::info!(count = entries.len(), "trakt library pulled");
        Ok(entries)
    }

    async fn push(&self, ops: &[TrackOp]) -> Result<(), kaeri_core::Error> {
        if !self.is_authenticated() {
            return Err(kaeri_core::Error::Auth("signed out".into()));
        }

        let mut shows = Vec::new();
        for op in ops {
            // Progress only. Trakt's list semantics do not line up with a watch status — a show is
            // in your watchlist or it is not — and inventing a mapping would write something the
            // user did not ask for.
            let TrackOp::SetProgress { anilist_id, episode } = op else { continue };
            let Some(tvdb_id) = self.mapping.tvdb_id(*anilist_id) else {
                tracing::warn!(
                    anilist_id = anilist_id.get(),
                    "no tvdb id, so trakt cannot be told about this title"
                );
                continue;
            };
            let Some((season, number)) = self.mapping.season_episode(*anilist_id, *episode)
            else {
                // Refused, not guessed. This title shares a TVDB series with other seasons and the
                // datasets carry no offset, so there is no way to say which season the episode is
                // in — and assuming the first would mark an unrelated episode as watched.
                tracing::warn!(
                    anilist_id = anilist_id.get(),
                    "trakt: cannot place this episode in a season, so it was not pushed"
                );
                continue;
            };
            shows.push(serde_json::json!({
                "ids": { "tvdb": tvdb_id },
                "seasons": [{
                    "number": season,
                    // Every episode up to this point, like Simkl: Trakt records a history of
                    // episodes rather than a progress counter.
                    "episodes": (1..=number)
                        .map(|n| serde_json::json!({ "number": n }))
                        .collect::<Vec<_>>(),
                }],
            }));
        }

        if shows.is_empty() {
            return Ok(());
        }
        let body = serde_json::json!({ "shows": shows });
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        let response = self
            .held
            .send(self, &mut token, |t| {
                self.request(reqwest::Method::POST, "/sync/history", t).json(&body)
            })
            .await?;

        // One request for the whole batch, so a refusal is the batch's: the drain then asks
        // again title by title, and only the refused title's change fails.
        if !response.status().is_success() {
            return Err(crate::failed_answer(response, "trakt", "trakt").await);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed {
        tvdb: Option<u32>,
        /// Episodes in seasons before this one, as the datasets' `episode_offset` gives it.
        offset: u32,
    }

    impl SeasonMapping for Fixed {
        fn tvdb_id(&self, _: AnilistId) -> Option<u32> {
            self.tvdb
        }
        fn season_episode(&self, _: AnilistId, absolute: u32) -> Option<(u32, u32)> {
            if self.offset == 0 {
                Some((1, absolute))
            } else {
                Some((2, absolute.saturating_sub(self.offset)))
            }
        }
        fn absolute_episode(&self, _: AnilistId, season: u32, episode: u32) -> u32 {
            if season <= 1 { episode } else { episode + self.offset }
        }
        fn anilist_for_tvdb(&self, _: u32) -> Option<AnilistId> {
            Some(AnilistId::new(1))
        }
    }

    #[test]
    fn an_absolute_episode_becomes_season_relative() {
        // The problem this whole trait exists for: Kaeri counts absolutely because AniList does,
        // and Trakt wants S2E03. Sending 15 as season 1 episode 15 marks the wrong episode.
        let mapping = Fixed { tvdb: Some(1234), offset: 12 };
        assert_eq!(mapping.season_episode(AnilistId::new(1), 15), Some((2, 3)));
        assert_eq!(mapping.absolute_episode(AnilistId::new(1), 2, 3), 15);
    }

    #[test]
    fn with_no_offset_everything_is_season_one() {
        // The documented assumption. Stated in a test so it is a known limitation rather than a
        // surprise: a split-cour show with no dataset offset syncs to season 1.
        let mapping = Fixed { tvdb: Some(1), offset: 0 };
        assert_eq!(mapping.season_episode(AnilistId::new(1), 15), Some((1, 15)));
    }

    #[tokio::test]
    async fn a_title_with_no_tvdb_id_is_skipped_rather_than_guessed() {
        // TVDB coverage is much thinner than MAL's, so this is the common case rather than an edge
        // one — and inventing an id would write progress onto somebody else's show.
        let tracker = TraktTracker::new(
            "id",
            reqwest::Client::new(),
            Arc::new(Fixed { tvdb: None, offset: 0 }),
            Some(TokenPair { access: "t".into(), refresh: None, expires_at: None }),
        );
        // No network happens: with nothing mappable there is nothing to send.
        let result = tracker
            .push(&[TrackOp::SetProgress { anilist_id: AnilistId::new(1), episode: 3 }])
            .await;
        assert!(result.is_ok(), "an unmappable title is not an error, got {result:?}");
    }

    #[test]
    fn progress_with_no_tvdb_id_waits_for_the_mappings_but_a_status_does_not() {
        // A missing TVDB id is something a dataset refresh can fix, so the drain holds that
        // op. A status never goes to Trakt at all, so holding it would hold it forever.
        let tracker = TraktTracker::new(
            "id",
            reqwest::Client::new(),
            Arc::new(Fixed { tvdb: None, offset: 0 }),
            None,
        );
        let id = AnilistId::new(1);
        assert!(!tracker.can_address(&TrackOp::SetProgress { anilist_id: id, episode: 3 }));
        assert!(tracker.can_address(&TrackOp::SetStatus {
            anilist_id: id,
            status: WatchStatus::Completed,
            at: 0
        }));
    }

    use crate::simkl::tests::serve_with;

    /// A tracker signed in with `pair`, its API and token endpoint on the fake server at `base`.
    fn tracker_at(base: &str, pair: TokenPair) -> TraktTracker {
        let token_url: &'static str = Box::leak(format!("{base}/oauth/token").into_boxed_str());
        TraktTracker {
            endpoints: DeviceEndpoints { token_url, ..device::TRAKT_RENEW },
            api: base.to_owned(),
            ..TraktTracker::new(
                "id",
                reqwest::Client::new(),
                Arc::new(Fixed { tvdb: Some(1), offset: 0 }),
                Some(pair),
            )
        }
    }

    fn pair(access: &str, expires_at: Option<i64>) -> TokenPair {
        TokenPair { access: access.into(), refresh: Some("rt-1".into()), expires_at }
    }

    fn episode() -> [TrackOp; 1] {
        [TrackOp::SetProgress { anilist_id: AnilistId::new(1), episode: 2 }]
    }

    const RENEWED: &str = r#"{"access_token":"new","token_type":"bearer","expires_in":86400,
        "refresh_token":"rt-2","scope":"public","created_at":1}"#;

    #[tokio::test]
    async fn a_sign_in_that_ran_out_is_renewed_before_the_push_and_the_new_pair_kept() {
        let (base, server) = serve_with(vec![(200, RENEWED), (201, "{}")]).await;
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::with_backend(dir.path(), crate::secret::Backend::File);
        let ran_out = pair("old", Some(1));
        store
            .set_pair("trakt", &ran_out.access, ran_out.refresh.as_deref(), ran_out.expires_at)
            .unwrap();
        let tracker = tracker_at(&base, ran_out).with_token_store(store.clone());

        tracker.push(&episode()).await.unwrap();

        let requests = server.await.unwrap();
        assert!(requests[0].starts_with("POST /oauth/token"), "{}", requests[0]);
        assert!(requests[0].contains(r#""refresh_token":"rt-1""#), "{}", requests[0]);
        assert!(requests[1].starts_with("POST /sync/history"), "{}", requests[1]);
        assert!(requests[1].contains("authorization: Bearer new"), "{}", requests[1]);
        // Stored at once: the refresh token it replaced no longer works.
        let stored = store.reread_pair("trakt").unwrap();
        assert_eq!((stored.access.as_str(), stored.refresh.as_deref()), ("new", Some("rt-2")));
        assert!(tracker.take_renewed().await.is_none(), "stored when it was renewed");
    }

    #[tokio::test]
    async fn a_token_trakt_rejects_is_renewed_and_the_push_sent_once_more() {
        let (base, server) = serve_with(vec![(401, "{}"), (200, RENEWED), (201, "{}")]).await;
        let tracker = tracker_at(&base, pair("old", None));

        tracker.push(&episode()).await.unwrap();

        let requests = server.await.unwrap();
        assert!(requests[0].contains("authorization: Bearer old"), "{}", requests[0]);
        assert!(requests[1].starts_with("POST /oauth/token"), "{}", requests[1]);
        assert!(requests[2].contains("authorization: Bearer new"), "{}", requests[2]);
        let renewed = tracker.take_renewed().await.expect("the renewed pair, to store");
        assert_eq!(renewed.refresh.as_deref(), Some("rt-2"));
    }

    #[tokio::test]
    async fn only_a_renewal_trakt_refuses_asks_for_a_new_sign_in() {
        let refused =
            r#"{"error":"invalid_grant","error_description":"The refresh token is invalid."}"#;
        let (base, server) = serve_with(vec![(401, "{}"), (400, refused)]).await;
        let tracker = tracker_at(&base, pair("old", None));

        let error = tracker.pull_library().await.unwrap_err();
        assert!(matches!(error, kaeri_core::Error::Auth(_)), "got {error:?}");
        assert_eq!(server.await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_renewal_that_never_reached_trakt_is_tried_again_later() {
        // A port with nothing behind it: the renewal cannot be sent at all.
        let closed = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let base = format!("http://{}", closed.local_addr().unwrap());
        drop(closed);
        let tracker = tracker_at(&base, pair("old", Some(1)));

        let error = tracker.push(&episode()).await.unwrap_err();
        // The network, never a refusal: the outbox waits it out without counting it.
        assert!(matches!(error, kaeri_core::Error::Network(_)), "got {error:?}");
        assert!(tracker.is_authenticated(), "still signed in, to try again");
    }

    #[tokio::test]
    async fn a_signed_out_tracker_refuses_before_touching_the_network() {
        let tracker = TraktTracker::new(
            "id",
            reqwest::Client::new(),
            Arc::new(Fixed { tvdb: Some(1), offset: 0 }),
            None,
        );
        let error = tracker
            .push(&[TrackOp::SetProgress { anilist_id: AnilistId::new(1), episode: 1 }])
            .await
            .unwrap_err();
        assert!(matches!(error, kaeri_core::Error::Auth(_)), "got {error:?}");
    }
}
