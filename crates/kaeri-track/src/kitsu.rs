//! Kitsu: password-grant auth, refresh, and the [`Tracker`] implementation.
//!
//! Kitsu is the one tracker with **no app registration at all**: its token endpoint
//! accepts the OAuth password grant with no client id and no secret. That removes the
//! usual setup step, and adds the one obligation the other trackers never had — the
//! user's actual password passes through `--login` once. It is exchanged immediately
//! and never stored; what lands in the keychain is the same access/refresh pair every
//! other tracker keeps.
//!
//! | | MAL | Kitsu |
//! |---|---|---|
//! | Sign-in | browser redirect + PKCE | username and password, once |
//! | Token life | ~31 days, refreshed | ~30 days, refreshed |
//! | Identity | `mal_id` via the mapping | `kitsu_id` via the same mapping |
//! | Wire shape | plain JSON | JSON:API — ids are strings, writes name their type |
//!
//! The JSON:API shape drives the push design. Kitsu has no upsert: creating an entry
//! that exists is a validation error, updating one that does not is a 404. So every
//! push first asks which of the two it is doing — one extra request per *title*, on a
//! queue that batches per title, against an API without the 30/minute budget AniList
//! has.

use kaeri_core::{
    ids::AnilistId,
    traits::{ReadRemoval, TrackOp, TrackedEntry, TrackedReadEntry, Tracker, WatchStatus},
};

use crate::{
    held::{Held, Renews},
    secret::{TokenPair, TokenStore},
};

const TOKEN: &str = "https://kitsu.io/api/oauth/token";
const API: &str = "https://kitsu.io/api/edge";

/// Where Kitsu answers: its token endpoint and its API. Kitsu's own, or a stand-in for a build
/// that must not reach it, such as a fixture build's fake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    token: String,
    api: String,
}

impl Default for Site {
    fn default() -> Self {
        Self { token: TOKEN.to_owned(), api: API.to_owned() }
    }
}

impl Site {
    /// A stand-in at `base`, laid out as Kitsu is: `{base}/api/oauth/token` and `{base}/api/edge`.
    pub fn at(base: &str) -> Self {
        let base = base.trim_end_matches('/');
        Self { token: format!("{base}/api/oauth/token"), api: format!("{base}/api/edge") }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn api(&self) -> &str {
        &self.api
    }
}
const JSON_API: &str = "application/vnd.api+json";

/// Library pages to follow before deciding the server is not counting down.
/// Five hundred entries a page, so this is a library nobody has.
const MAX_LIBRARY_PAGES: u32 = 40;

/// Exchange a username and password for a token pair.
///
/// The password's only journey: it goes to Kitsu's token endpoint and nowhere else,
/// and the caller should drop it the moment this returns.
pub async fn login(
    http: &reqwest::Client,
    site: &Site,
    username: &str,
    password: &str,
    now: i64,
) -> Result<TokenPair, crate::auth::AuthError> {
    let form =
        [("grant_type", "password"), ("username", username.trim()), ("password", password)];
    post_token(http, site, &form, now).await
}

/// Renew an access token.
pub async fn refresh(
    http: &reqwest::Client,
    site: &Site,
    refresh_token: &str,
    now: i64,
) -> Result<TokenPair, crate::auth::AuthError> {
    let form = [("grant_type", "refresh_token"), ("refresh_token", refresh_token.trim())];
    post_token(http, site, &form, now).await
}

async fn post_token(
    http: &reqwest::Client,
    site: &Site,
    form: &[(&str, &str)],
    now: i64,
) -> Result<TokenPair, crate::auth::AuthError> {
    use crate::auth::AuthError;

    let response = http
        .post(site.token())
        .form(form)
        .send()
        .await
        .map_err(|e| AuthError::Exchange(e.to_string()))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();

    if let Some(access) = parsed["access_token"].as_str().filter(|t| !t.is_empty()) {
        return Ok(TokenPair {
            access: access.to_owned(),
            refresh: parsed["refresh_token"].as_str().map(str::to_owned),
            expires_at: parsed["expires_in"].as_i64().map(|secs| now + secs),
        });
    }

    // OAuth's own code goes with the reason, so a front end can tell credentials Kitsu refused
    // (`invalid_grant`) from Kitsu failing, whatever words the description uses.
    let said = crate::said(status, &body);
    Err(AuthError::Exchange(match parsed["error"].as_str() {
        Some(code) if !code.is_empty() && !said.contains(code) => format!("{said} ({code})"),
        _ => said,
    }))
}

/// Kitsu's `status` values.
pub const fn status_to_kitsu(status: WatchStatus) -> &'static str {
    match status {
        // Kitsu, like MAL, models rewatching as a flag rather than a status.
        WatchStatus::Current | WatchStatus::Repeating => "current",
        WatchStatus::Planning => "planned",
        WatchStatus::Completed => "completed",
        WatchStatus::Paused => "on_hold",
        WatchStatus::Dropped => "dropped",
    }
}

pub fn status_from_kitsu(status: &str) -> WatchStatus {
    match status {
        "planned" => WatchStatus::Planning,
        "completed" => WatchStatus::Completed,
        "on_hold" => WatchStatus::Paused,
        "dropped" => WatchStatus::Dropped,
        _ => WatchStatus::Current,
    }
}

/// Our 10-point score as Kitsu's `ratingTwenty`, clamped into its 2–20 band.
///
/// The floor matters: a 1/10 maps to 2, because Kitsu reserves 0 and 1 for "unrated"
/// and rejects them on write.
pub fn rating_twenty(score: f32) -> u32 {
    ((score * 2.0).round() as u32).clamp(2, 20)
}

/// Translates between AniList ids and Kitsu ids.
///
/// Its own trait rather than a reuse of [`crate::mal::IdMapping`], because a mapping
/// that answers for MAL and not Kitsu is common in the datasets and the two must be
/// able to disagree.
pub trait IdMapping: Send + Sync {
    fn kitsu_id(&self, anilist_id: AnilistId) -> Option<u32>;
    fn anilist_id(&self, kitsu_id: u32) -> Option<AnilistId>;
}

/// The Kitsu tracker.
pub struct KitsuTracker {
    http: reqwest::Client,
    site: Site,
    mapping: std::sync::Arc<dyn IdMapping>,
    /// Manga ids, when reading is kept: without them the manga list is not.
    read_mapping: Option<std::sync::Arc<dyn crate::read::ReadMapping>>,
    /// The pair, renewed before it runs out and kept in step with other processes: see
    /// [`crate::held`].
    held: Held,
    /// The signed-in user's id, fetched once per process. JSON:API ids are strings.
    user_id: tokio::sync::RwLock<Option<String>>,
}

impl Renews for KitsuTracker {
    async fn renew(
        &self,
        refresh_token: &str,
        now: i64,
    ) -> Result<TokenPair, kaeri_core::Error> {
        refresh(&self.http, &self.site, refresh_token, now)
            .await
            .map_err(|e| kaeri_core::Error::Auth(e.to_string()))
    }
}

impl KitsuTracker {
    pub fn new(
        http: reqwest::Client,
        mapping: std::sync::Arc<dyn IdMapping>,
        tokens: Option<TokenPair>,
    ) -> Self {
        Self {
            http,
            site: Site::default(),
            mapping,
            read_mapping: None,
            held: Held::new("kitsu", "kitsu", tokens),
            user_id: tokio::sync::RwLock::new(None),
        }
    }

    /// Keep the manga and novel list too, with the ids `mapping` knows.
    #[must_use]
    pub fn with_read_mapping(
        mut self,
        mapping: std::sync::Arc<dyn crate::read::ReadMapping>,
    ) -> Self {
        self.read_mapping = Some(mapping);
        self
    }

    /// On `site` rather than Kitsu itself.
    #[must_use]
    pub fn at(mut self, site: Site) -> Self {
        self.site = site;
        self
    }

    /// Where this tracker reaches Kitsu, which a sign-in goes to as well.
    pub fn site(&self) -> &Site {
        &self.site
    }

    /// Kept in step with the other processes signed in through `store`, as MAL's is.
    #[must_use]
    pub fn with_token_store(mut self, store: TokenStore) -> Self {
        self.held.share(store);
        self
    }

    /// A renewed token pair the caller should write back, if a refresh happened and no store
    /// was given, or the store did not take it.
    pub async fn take_renewed(&self) -> Option<TokenPair> {
        self.held.take_renewed().await
    }

    /// The signed-in user's id, fetched once and kept.
    async fn user_id(&self, token: &mut String) -> Result<String, kaeri_core::Error> {
        if let Some(id) = self.user_id.read().await.clone() {
            return Ok(id);
        }
        let url = format!("{}/users?filter[self]=true", self.site.api());
        let payload = self
            .held
            .send(self, token, |t| self.request(reqwest::Method::GET, &url, t))
            .await?;
        if payload.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(kaeri_core::Error::Auth("kitsu rejected the token".into()));
        }
        let body: serde_json::Value =
            payload.json().await.map_err(|e| tracker_error(e.to_string()))?;
        let Some(id) = body["data"][0]["id"].as_str().map(str::to_owned) else {
            return Err(tracker_error("kitsu returned no user for this token".into()));
        };
        *self.user_id.write().await = Some(id.clone());
        Ok(id)
    }

    fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        token: &str,
    ) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .bearer_auth(token)
            .header("Accept", JSON_API)
            .header("Content-Type", JSON_API)
    }

    /// The existing library entry for one `kind` (`anime` or `manga`), if the user has one.
    /// Kitsu's anime and manga ids are separate number spaces, so the kind is part of the ask.
    async fn entry_id_of(
        &self,
        token: &mut String,
        user_id: &str,
        kind: &str,
        kitsu_id: u32,
    ) -> Result<Option<String>, kaeri_core::Error> {
        let url = format!(
            "{}/library-entries?filter[userId]={user_id}&filter[{kind}Id]={kitsu_id}\
             &fields[libraryEntries]=status&page[limit]=1",
            self.site.api()
        );
        let body: serde_json::Value = self
            .held
            .send(self, token, |t| self.request(reqwest::Method::GET, &url, t))
            .await?
            .json()
            .await
            .map_err(|e| tracker_error(e.to_string()))?;
        Ok(body["data"][0]["id"].as_str().map(str::to_owned))
    }

    /// Create or change one library entry of `kind`, the write an anime or a manga push is.
    async fn write_entry(
        &self,
        token: &mut String,
        user_id: &str,
        kind: &str,
        kitsu_id: u32,
        attributes: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), kaeri_core::Error> {
        // Kitsu has no upsert, so ask which write this is. An entry appearing between the
        // check and the POST fails as a validation error and retries through the outbox, by
        // which time this check answers PATCH.
        let existing = self.entry_id_of(token, user_id, kind, kitsu_id).await?;
        let response = match &existing {
            Some(entry_id) => {
                let body = serde_json::json!({ "data": {
                    "id": entry_id,
                    "type": "libraryEntries",
                    "attributes": attributes,
                }});
                let url = format!("{}/library-entries/{entry_id}", self.site.api());
                self.held
                    .send(self, token, |t| {
                        self.request(reqwest::Method::PATCH, &url, t).json(&body)
                    })
                    .await
            }
            None => {
                // Creation demands a status: Kitsu answers 422 "status can't be blank" without
                // one, and a bare progress push for a never-listed title would sit in the
                // outbox retrying forever. Current is the only honest default for an entry
                // that exists because of a watch or a read; an op that carries its own wins.
                let mut attributes = attributes.clone();
                attributes
                    .entry("status")
                    .or_insert_with(|| status_to_kitsu(WatchStatus::Current).into());
                let body = serde_json::json!({ "data": {
                    "type": "libraryEntries",
                    "attributes": attributes,
                    "relationships": {
                        kind: { "data": { "type": kind, "id": kitsu_id.to_string() } },
                        "user": { "data": { "type": "users", "id": user_id } },
                    },
                }});
                let url = format!("{}/library-entries", self.site.api());
                self.held
                    .send(self, token, |t| {
                        self.request(reqwest::Method::POST, &url, t).json(&body)
                    })
                    .await
            }
        }?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(kaeri_core::Error::Auth("kitsu rejected the token".into()));
        }
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(tracker_error(format!(
                "kitsu answered {}",
                crate::said(status, &body)
            )));
        }
        Ok(())
    }
}

/// Coalesce a batch per title, like every other tracker: three ops for one show
/// become one write. Progress keeps the highest episode, because it is monotonic.
fn coalesce(ops: &[TrackOp]) -> Vec<(AnilistId, serde_json::Map<String, serde_json::Value>)> {
    let mut batched: Vec<(AnilistId, serde_json::Map<String, serde_json::Value>)> = Vec::new();
    for op in ops {
        // Reading waits in its own outbox and is never in a watching batch.
        if op.is_reading() {
            continue;
        }
        let id = op.anilist_id();
        let slot = match batched.iter_mut().find(|(existing, _)| *existing == id) {
            Some((_, attributes)) => attributes,
            None => {
                batched.push((id, serde_json::Map::new()));
                &mut batched.last_mut().expect("just pushed").1
            }
        };
        match op {
            TrackOp::SetProgress { episode, .. } => {
                let highest =
                    slot.get("progress").and_then(serde_json::Value::as_u64).unwrap_or(0);
                slot.insert("progress".into(), u64::from(*episode).max(highest).into());
            }
            TrackOp::SetStatus { status, .. } => {
                slot.insert("status".into(), status_to_kitsu(*status).into());
            }
            TrackOp::SetScore { score, .. } => {
                slot.insert("ratingTwenty".into(), rating_twenty(*score).into());
            }
            TrackOp::SetReadProgress { .. }
            | TrackOp::SetReadStatus { .. }
            | TrackOp::SetReadScore { .. } => {}
        }
    }
    batched
}

/// A batch of reading ops as one write per work. Chapters only: Kitsu's `volumesOwned` counts
/// owned copies, so a volume is never sent. A reread is the `reconsuming` flag.
fn coalesce_read(
    ops: &[TrackOp],
) -> Vec<(AnilistId, serde_json::Map<String, serde_json::Value>)> {
    let mut batched: Vec<(AnilistId, serde_json::Map<String, serde_json::Value>)> = Vec::new();
    for op in ops.iter().filter(|op| op.is_reading()) {
        let id = op.anilist_id();
        let at = match batched.iter().position(|(existing, _)| *existing == id) {
            Some(at) => at,
            None => {
                batched.push((id, serde_json::Map::new()));
                batched.len() - 1
            }
        };
        let slot = &mut batched[at].1;
        match op {
            TrackOp::SetReadProgress { chapter, .. } => {
                let held =
                    slot.get("progress").and_then(serde_json::Value::as_u64).unwrap_or(0);
                slot.insert("progress".into(), u64::from(*chapter).max(held).into());
            }
            TrackOp::SetReadStatus { status, rereading, .. } => {
                slot.insert("status".into(), status_to_kitsu(*status).into());
                slot.insert("reconsuming".into(), (*rereading).into());
            }
            TrackOp::SetReadScore { score, .. } => {
                slot.insert("ratingTwenty".into(), rating_twenty(*score).into());
            }
            _ => {}
        }
    }
    batched
}

#[async_trait::async_trait]
impl Tracker for KitsuTracker {
    fn id(&self) -> &str {
        "kitsu"
    }

    fn is_authenticated(&self) -> bool {
        self.held.is_held()
    }

    fn can_address(&self, op: &TrackOp) -> bool {
        if op.is_reading() {
            return self
                .read_mapping
                .as_ref()
                .is_some_and(|m| m.kitsu_id(op.anilist_id()).is_some());
        }
        self.mapping.kitsu_id(op.anilist_id()).is_some()
    }

    fn serves_reading(&self) -> bool {
        self.read_mapping.is_some()
    }

    async fn pull_read_library(&self) -> Result<Vec<TrackedReadEntry>, kaeri_core::Error> {
        let Some(mapping) = self.read_mapping.clone() else { return Ok(Vec::new()) };
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        let user_id = self.user_id(&mut token).await?;
        let mut entries = Vec::new();
        // `include=manga` carries the relationship's id, and `subtype` tells a novel.
        let mut next = Some(format!(
            "{}/library-entries?filter[userId]={user_id}&filter[kind]=manga\
             &fields[libraryEntries]=progress,status,ratingTwenty,reconsuming,manga\
             &include=manga&fields[manga]=subtype&page[limit]=500",
            self.site.api()
        ));
        let mut pages = 0_u32;
        while let Some(url) = next.take() {
            pages += 1;
            if pages > MAX_LIBRARY_PAGES {
                tracing::warn!(pages, "kitsu pagination did not terminate; stopping");
                break;
            }
            if !url.starts_with(self.site.api()) {
                tracing::warn!(url = %url, "kitsu paged us off its own api; stopping");
                break;
            }
            let response = self
                .held
                .send(self, &mut token, |t| self.request(reqwest::Method::GET, &url, t))
                .await?;
            let payload = crate::page_json(response, "kitsu", "kitsu").await?;
            let subtype_of = |id: &str| {
                payload["included"].as_array().into_iter().flatten().find_map(|item| {
                    (item["type"] == "manga" && item["id"] == id)
                        .then(|| item["attributes"]["subtype"].as_str().map(str::to_owned))
                        .flatten()
                })
            };
            for node in payload["data"].as_array().into_iter().flatten() {
                let Some(manga_id) = node["relationships"]["manga"]["data"]["id"].as_str()
                else {
                    continue;
                };
                let Some(anilist_id) =
                    manga_id.parse::<u32>().ok().and_then(|id| mapping.anilist_for_kitsu(id))
                else {
                    continue;
                };
                let attributes = &node["attributes"];
                let rereading = attributes["reconsuming"].as_bool().unwrap_or(false);
                entries.push(TrackedReadEntry {
                    anilist_id,
                    medium: if subtype_of(manga_id).as_deref() == Some("novel") {
                        kaeri_core::media::Medium::Novel
                    } else {
                        kaeri_core::media::Medium::Manga
                    },
                    chapters: attributes["progress"].as_u64().unwrap_or(0) as u32,
                    // Kitsu has no volumes-read field; `volumesOwned` counts copies.
                    volumes: None,
                    status: status_from_kitsu(
                        attributes["status"].as_str().unwrap_or("current"),
                    ),
                    rereading,
                    score: attributes["ratingTwenty"]
                        .as_f64()
                        .filter(|r| *r > 0.0)
                        .map(|r| (r / 2.0) as f32),
                });
            }
            next = payload["links"]["next"].as_str().map(str::to_owned);
        }
        tracing::info!(count = entries.len(), "kitsu manga list pulled");
        Ok(entries)
    }

    async fn push_read(&self, ops: &[TrackOp]) -> Result<Vec<AnilistId>, kaeri_core::Error> {
        let Some(mapping) = self.read_mapping.clone() else { return Ok(Vec::new()) };
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        let user_id = self.user_id(&mut token).await?;
        let mut ignored = Vec::new();
        for (anilist_id, attributes) in coalesce_read(ops) {
            if attributes.is_empty() {
                continue;
            }
            let Some(kitsu_id) = mapping.kitsu_id(anilist_id) else {
                tracing::warn!(
                    anilist_id = anilist_id.get(),
                    "no kitsu manga id for this work"
                );
                ignored.push(anilist_id);
                continue;
            };
            self.write_entry(&mut token, &user_id, "manga", kitsu_id, &attributes).await?;
        }
        Ok(ignored)
    }

    async fn remove_read(
        &self,
        anilist_id: AnilistId,
    ) -> Result<ReadRemoval, kaeri_core::Error> {
        let Some(kitsu_id) = self.read_mapping.as_ref().and_then(|m| m.kitsu_id(anilist_id))
        else {
            return Ok(ReadRemoval::Unmatched);
        };
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        let user_id = self.user_id(&mut token).await?;
        let Some(entry_id) = self.entry_id_of(&mut token, &user_id, "manga", kitsu_id).await?
        else {
            return Ok(ReadRemoval::NotOnList);
        };
        let url = format!("{}/library-entries/{entry_id}", self.site.api());
        let response = self
            .held
            .send(self, &mut token, |t| self.request(reqwest::Method::DELETE, &url, t))
            .await?;
        match response.status() {
            reqwest::StatusCode::UNAUTHORIZED => {
                Err(kaeri_core::Error::Auth("kitsu rejected the token".into()))
            }
            reqwest::StatusCode::NOT_FOUND => Ok(ReadRemoval::NotOnList),
            status if status.is_success() => Ok(ReadRemoval::Removed),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(tracker_error(format!("kitsu answered {}", crate::said(status, &body))))
            }
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
        // The user id belongs to the account, not the process.
        *self.user_id.write().await = None;
    }

    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, kaeri_core::Error> {
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        let user_id = self.user_id(&mut token).await?;

        let mut entries = Vec::new();
        // `include=anime` is what makes the relationship carry its id: JSON:API sends
        // resource linkage only for included relationships.
        let mut next = Some(format!(
            "{}/library-entries?filter[userId]={user_id}&filter[kind]=anime\
             &fields[libraryEntries]=progress,status,ratingTwenty,anime\
             &include=anime&fields[anime]=id&page[limit]=500",
            self.site.api()
        ));

        let mut pages = 0_u32;
        while let Some(url) = next.take() {
            // The same two guards as MAL, for the same reasons: a `next` pointing off
            // the API would hand that host the bearer token, and one pointing at
            // itself is a loop that grows `entries` until the process dies.
            pages += 1;
            if pages > MAX_LIBRARY_PAGES {
                tracing::warn!(pages, "kitsu pagination did not terminate; stopping");
                break;
            }
            if !url.starts_with(self.site.api()) {
                tracing::warn!(url = %url, "kitsu paged us off its own api; stopping");
                break;
            }

            let response = self
                .held
                .send(self, &mut token, |t| self.request(reqwest::Method::GET, &url, t))
                .await?;
            let payload = crate::page_json(response, "kitsu", "kitsu").await?;

            for node in payload["data"].as_array().into_iter().flatten() {
                let Some(kitsu_id) = node["relationships"]["anime"]["data"]["id"]
                    .as_str()
                    .and_then(|id| id.parse::<u32>().ok())
                else {
                    continue;
                };
                let Some(anilist_id) = self.mapping.anilist_id(kitsu_id) else {
                    tracing::debug!(kitsu_id, "no anilist id mapped; skipping");
                    continue;
                };
                let attributes = &node["attributes"];
                entries.push(TrackedEntry {
                    anilist_id,
                    progress: attributes["progress"].as_u64().unwrap_or(0) as u32,
                    status: status_from_kitsu(
                        attributes["status"].as_str().unwrap_or("current"),
                    ),
                    // `ratingTwenty` is 2–20 or null; halved onto the shared scale.
                    score: attributes["ratingTwenty"]
                        .as_f64()
                        .filter(|r| *r > 0.0)
                        .map(|r| (r / 2.0) as f32),
                });
            }

            next = payload["links"]["next"].as_str().map(str::to_owned);
        }

        tracing::info!(count = entries.len(), "kitsu library pulled");
        Ok(entries)
    }

    async fn push(&self, ops: &[TrackOp]) -> Result<(), kaeri_core::Error> {
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        let user_id = self.user_id(&mut token).await?;

        for (anilist_id, attributes) in coalesce(ops) {
            if attributes.is_empty() {
                continue;
            }
            // The drain holds an unmapped title's ops until the mappings change (see
            // `can_address`). One that arrives anyway is skipped and said out loud, never
            // raised: failing the batch would hold every other title's progress behind it.
            let Some(kitsu_id) = self.mapping.kitsu_id(anilist_id) else {
                tracing::warn!(
                    anilist_id = anilist_id.get(),
                    "no kitsu id, so kitsu cannot be told about this title"
                );
                continue;
            };

            self.write_entry(&mut token, &user_id, "anime", kitsu_id, &attributes).await?;
        }
        Ok(())
    }
}

fn tracker_error(message: String) -> kaeri_core::Error {
    kaeri_core::Error::Tracker { tracker: "kitsu".into(), message }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSHIKUZU: AnilistId = AnilistId::new(154_587);

    #[test]
    fn every_status_survives_a_round_trip() {
        // These strings go into someone's list; a mismatch quietly marks a show dropped.
        for status in [
            WatchStatus::Current,
            WatchStatus::Planning,
            WatchStatus::Completed,
            WatchStatus::Paused,
            WatchStatus::Dropped,
        ] {
            assert_eq!(status_from_kitsu(status_to_kitsu(status)), status);
        }
        // Lossy on purpose, like MAL: rewatching is a flag on Kitsu, not a status.
        assert_eq!(status_to_kitsu(WatchStatus::Repeating), "current");
    }

    #[test]
    fn scores_land_inside_kitsu_s_two_to_twenty_band() {
        assert_eq!(rating_twenty(10.0), 20);
        assert_eq!(rating_twenty(8.5), 17);
        // The floor: Kitsu reserves 0 and 1 for "unrated" and rejects them on write.
        assert_eq!(rating_twenty(1.0), 2);
        assert_eq!(rating_twenty(0.0), 2);
    }

    #[test]
    fn a_title_s_ops_coalesce_into_one_write() {
        let batched = coalesce(&[
            TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 3 },
            TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 7 },
            TrackOp::SetStatus { anilist_id: HOSHIKUZU, status: WatchStatus::Current, at: 1 },
            TrackOp::SetScore { anilist_id: HOSHIKUZU, score: 9.0, at: 2 },
        ]);
        assert_eq!(batched.len(), 1);
        let (id, attributes) = &batched[0];
        assert_eq!(*id, HOSHIKUZU);
        // Progress is monotonic: the higher episode wins whatever order ops arrive in.
        assert_eq!(attributes["progress"], 7);
        assert_eq!(attributes["status"], "current");
        assert_eq!(attributes["ratingTwenty"], 18);
    }

    #[test]
    fn two_titles_stay_two_writes() {
        let other = AnilistId::new(1);
        let batched = coalesce(&[
            TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 3 },
            TrackOp::SetProgress { anilist_id: other, episode: 1 },
        ]);
        assert_eq!(batched.len(), 2);
    }

    #[test]
    fn reading_ops_make_chapter_writes_and_never_a_volume() {
        let batched = coalesce_read(&[
            TrackOp::SetReadProgress { anilist_id: HOSHIKUZU, chapter: 4, volume: Some(2) },
            TrackOp::SetReadProgress { anilist_id: HOSHIKUZU, chapter: 9, volume: None },
            TrackOp::SetReadStatus {
                anilist_id: HOSHIKUZU,
                status: WatchStatus::Current,
                rereading: true,
                at: 1,
            },
            TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 50 },
        ]);
        assert_eq!(batched.len(), 1);
        let attributes = &batched[0].1;
        assert_eq!(attributes["progress"], 9);
        assert_eq!(attributes["reconsuming"], true);
        assert!(
            attributes.get("volumesOwned").is_none() && attributes.get("volumes").is_none()
        );
        assert!(
            coalesce(&[TrackOp::SetReadProgress {
                anilist_id: HOSHIKUZU,
                chapter: 1,
                volume: None
            }])
            .is_empty()
        );
    }
}
