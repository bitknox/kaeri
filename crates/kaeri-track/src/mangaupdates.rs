//! MangaUpdates: the manga and novel list tracker that has no anime side.
//!
//! The differences from the other trackers decide the shape of this one:
//!
//! | | AniList, MyAnimeList, Kitsu | MangaUpdates |
//! |---|---|---|
//! | Sign-in | browser sheet or credentials sheet | a form: user name and password, once |
//! | Token | refreshes, or runs for a year | a session token with no refresh and no stated life |
//! | Statuses | one list with a status field | one list per status, and people add their own |
//! | Ids | AniList's, or a small number | its own 64-bit series id |
//! | Pace | a budget per minute | no number published, but list writes five seconds apart |
//!
//! The session token is kept like the others (a [`TokenPair`] with nothing to renew it), and a
//! token the service rejects is an auth error: the person signs in again on the form.
//!
//! MangaUpdates' acceptable use asks for credit, spacing and caching. Requests are spaced by a
//! [`Pacer`], list writes five seconds apart as its own `412 Five second update delay` asks, a
//! 412 is waited out and retried, and what was read is kept for a short while instead of asked
//! for again.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use kaeri_core::{
    ids::AnilistId,
    media::Medium,
    traits::{ReadRemoval, TrackOp, TrackedReadEntry, Tracker, WatchStatus},
};

use crate::{
    held::{Held, Renews},
    read::ReadMapping,
    secret::{TokenPair, TokenStore},
};

pub const API: &str = "https://api.mangaupdates.com/v1";

/// The gap between any two requests.
const READ_GAP: Duration = Duration::from_secs(1);

/// The gap between two list writes: MangaUpdates answers 412 to anything closer.
const WRITE_GAP: Duration = Duration::from_secs(5);

/// How many times a 412 is waited out before the batch is left to the outbox's backoff.
const DELAY_RETRIES: u32 = 3;

/// How long an entry read from the list serves later asks for it.
const ENTRY_REUSE: Duration = Duration::from_secs(300);

/// Rows a list search is asked for at a time, and how many pages are followed at most.
const PER_PAGE: u32 = 100;
const MAX_PAGES: u32 = 100;

/// Keeps requests apart: the gap between any two, and the larger one between list writes.
#[derive(Debug)]
pub struct Pacer {
    read_gap: Duration,
    write_gap: Duration,
    last: tokio::sync::Mutex<(Option<Instant>, Option<Instant>)>,
}

impl Pacer {
    pub fn new(read_gap: Duration, write_gap: Duration) -> Self {
        Self { read_gap, write_gap, last: tokio::sync::Mutex::new((None, None)) }
    }

    /// Wait until a request of this kind may go, and count it as made. Held across the wait,
    /// so requests made at once leave one after another.
    pub async fn space(&self, write: bool) {
        let mut last = self.last.lock().await;
        let mut ready = Instant::now();
        if let Some(any) = last.0 {
            ready = ready.max(any + self.read_gap);
        }
        if write && let Some(written) = last.1 {
            ready = ready.max(written + self.write_gap);
        }
        let wait = ready.saturating_duration_since(Instant::now());
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        let now = Instant::now();
        last.0 = Some(now);
        if write {
            last.1 = Some(now);
        }
    }
}

/// MangaUpdates' list types, and what each is for Kaeri.
const LIST_TYPES: [(&str, WatchStatus, u64); 5] = [
    ("read", WatchStatus::Current, 0),
    ("wish", WatchStatus::Planning, 1),
    ("complete", WatchStatus::Completed, 2),
    ("unfinished", WatchStatus::Dropped, 3),
    ("hold", WatchStatus::Paused, 4),
];

/// A status as a list type. A reread is on the reading list: MangaUpdates has no such state.
pub const fn list_type(status: WatchStatus) -> &'static str {
    match status {
        WatchStatus::Current | WatchStatus::Repeating => "read",
        WatchStatus::Planning => "wish",
        WatchStatus::Completed => "complete",
        WatchStatus::Dropped => "unfinished",
        WatchStatus::Paused => "hold",
    }
}

/// One of the person's lists.
#[derive(Debug, Clone, PartialEq, Eq)]
struct List {
    id: u64,
    /// `read`, `wish`, `complete`, `unfinished` or `hold`, or empty for a custom list.
    kind: String,
    custom: bool,
}

impl List {
    /// Kaeri's status for a series on this list. A custom list reads as Current.
    fn status(&self) -> WatchStatus {
        if self.custom {
            return WatchStatus::Current;
        }
        LIST_TYPES
            .iter()
            .find(|(kind, ..)| *kind == self.kind)
            .map_or(WatchStatus::Current, |(_, status, _)| *status)
    }
}

/// A series on a list.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    list_id: u64,
    chapter: u32,
    volume: u32,
}

/// Sign in with a user name and password.
///
/// The password's only journey: it goes to the login call and nowhere else, and the caller drops
/// it when this returns. What comes back is a session token, kept as the other trackers' are.
pub async fn login(
    http: &reqwest::Client,
    api: &str,
    username: &str,
    password: &str,
) -> Result<TokenPair, crate::auth::AuthError> {
    use crate::auth::AuthError;
    let response = http
        .put(format!("{}/account/login", api.trim_end_matches('/')))
        .header("User-Agent", crate::USER_AGENT)
        .json(&serde_json::json!({ "username": username.trim(), "password": password }))
        .send()
        .await
        .map_err(|e| AuthError::Exchange(e.to_string()))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    match parsed["context"]["session_token"].as_str().filter(|t| !t.is_empty()) {
        Some(token) if status.is_success() => {
            Ok(TokenPair { access: token.to_owned(), refresh: None, expires_at: None })
        }
        _ => Err(AuthError::Exchange(crate::said(status, &body))),
    }
}

/// End the session at MangaUpdates. Best effort: the token is forgotten here either way.
pub async fn logout(http: &reqwest::Client, api: &str, token: &str) {
    let _ = http
        .post(format!("{}/account/logout", api.trim_end_matches('/')))
        .header("User-Agent", crate::USER_AGENT)
        .bearer_auth(token)
        .send()
        .await;
}

/// The MangaUpdates tracker.
pub struct MangaUpdatesTracker {
    http: reqwest::Client,
    api: String,
    mapping: Arc<dyn ReadMapping>,
    held: Held,
    pacer: Pacer,
    /// The person's lists, asked for once a run.
    lists: tokio::sync::Mutex<Option<Vec<List>>>,
    /// What the list holds for a series, as last read or written, and when.
    entries: tokio::sync::Mutex<HashMap<u64, (Instant, Option<Entry>)>>,
}

impl MangaUpdatesTracker {
    pub fn new(
        http: reqwest::Client,
        mapping: Arc<dyn ReadMapping>,
        tokens: Option<TokenPair>,
    ) -> Self {
        Self {
            http,
            api: API.to_owned(),
            mapping,
            held: Held::new("mangaupdates", "MangaUpdates", tokens),
            pacer: Pacer::new(READ_GAP, WRITE_GAP),
            lists: tokio::sync::Mutex::new(None),
            entries: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Ask a stand-in at `api` instead of MangaUpdates: for a test, which never reaches it.
    #[must_use]
    pub fn with_api(mut self, api: impl Into<String>) -> Self {
        self.api = api.into().trim_end_matches('/').to_owned();
        self
    }

    /// Space requests by these gaps instead of MangaUpdates' own: for a test.
    #[must_use]
    pub fn with_gaps(mut self, read_gap: Duration, write_gap: Duration) -> Self {
        self.pacer = Pacer::new(read_gap, write_gap);
        self
    }

    /// Kept in step with the other processes signed in through `store`.
    #[must_use]
    pub fn with_token_store(mut self, store: TokenStore) -> Self {
        self.held.share(store);
        self
    }

    /// Where this tracker reaches MangaUpdates, which a sign-in goes to as well.
    pub fn api(&self) -> &str {
        &self.api
    }

    /// A pair the caller should write back, when the store did not take it. None is ever
    /// renewed here, so this stays empty; it is the same call the other trackers answer.
    pub async fn take_renewed(&self) -> Option<TokenPair> {
        self.held.take_renewed().await
    }

    /// Send a request, spaced, and with a 412 waited out and sent again.
    async fn send(
        &self,
        token: &mut String,
        write: bool,
        build: impl Fn(&str) -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, kaeri_core::Error> {
        let mut waited = 0;
        loop {
            self.pacer.space(write).await;
            let response = self
                .held
                .send(self, token, |t| build(t).header("User-Agent", crate::USER_AGENT))
                .await?;
            if response.status() == reqwest::StatusCode::PRECONDITION_FAILED
                && waited < DELAY_RETRIES
            {
                waited += 1;
                continue;
            }
            return Ok(response);
        }
    }

    async fn lists(&self, token: &mut String) -> Result<Vec<List>, kaeri_core::Error> {
        let mut kept = self.lists.lock().await;
        if let Some(lists) = kept.as_ref() {
            return Ok(lists.clone());
        }
        let url = format!("{}/lists", self.api);
        let response = self.send(token, false, |t| self.http.get(&url).bearer_auth(t)).await?;
        let body = crate::page_json(response, "mangaupdates", "MangaUpdates").await?;
        let mut lists: Vec<List> = body
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|list| {
                Some(List {
                    id: list["list_id"].as_u64()?,
                    kind: list["type"].as_str().unwrap_or_default().to_owned(),
                    custom: list["custom"].as_bool().unwrap_or(false),
                })
            })
            .collect();
        if lists.is_empty() {
            // A person's lists always include the five; the ids are the documented ones.
            lists = LIST_TYPES
                .iter()
                .map(|(kind, _, id)| List { id: *id, kind: (*kind).to_owned(), custom: false })
                .collect();
        }
        *kept = Some(lists.clone());
        Ok(lists)
    }

    async fn remember(&self, series: u64, entry: Option<Entry>) {
        self.entries.lock().await.insert(series, (Instant::now(), entry));
    }

    /// What the list holds for a series: kept for a while, else asked for.
    async fn entry_of(
        &self,
        token: &mut String,
        series: u64,
    ) -> Result<Option<Entry>, kaeri_core::Error> {
        if let Some((at, entry)) = self.entries.lock().await.get(&series)
            && at.elapsed() < ENTRY_REUSE
        {
            return Ok(entry.clone());
        }
        let url = format!("{}/lists/series/{series}", self.api);
        let response = self.send(token, false, |t| self.http.get(&url).bearer_auth(t)).await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            self.remember(series, None).await;
            return Ok(None);
        }
        let body = crate::page_json(response, "mangaupdates", "MangaUpdates").await?;
        let entry = entry_from(&body);
        self.remember(series, entry.clone()).await;
        Ok(entry)
    }

    /// A list write, spaced, that must be answered with success.
    async fn write(
        &self,
        token: &mut String,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<(), kaeri_core::Error> {
        let url = format!("{}{path}", self.api);
        let response =
            self.send(token, true, |t| self.http.post(&url).bearer_auth(t).json(body)).await?;
        crate::page_json(response, "mangaupdates", "MangaUpdates").await.map(|_| ())
    }
}

/// An entry from a list answer: the record, or the answer when it is the record.
fn entry_from(body: &serde_json::Value) -> Option<Entry> {
    let record = if body["record"].is_object() { &body["record"] } else { body };
    Some(Entry {
        list_id: record["list_id"].as_u64()?,
        chapter: record["status"]["chapter"].as_u64().unwrap_or(0) as u32,
        volume: record["status"]["volume"].as_u64().unwrap_or(0) as u32,
    })
}

impl Renews for MangaUpdatesTracker {
    async fn renew(&self, _refresh: &str, _now: i64) -> Result<TokenPair, String> {
        // A session has no refresh token, so this is only reached by a pair that has one stored
        // by hand; the person signs in on the form instead.
        Err("MangaUpdates sessions are not renewed".into())
    }
}

/// One work's changes in a batch.
#[derive(Debug, Default, Clone, PartialEq)]
struct Change {
    chapter: Option<u32>,
    volume: Option<u32>,
    status: Option<WatchStatus>,
    score: Option<f32>,
}

fn changes(ops: &[TrackOp]) -> Vec<(AnilistId, Change)> {
    let mut out: Vec<(AnilistId, Change)> = Vec::new();
    for op in ops.iter().filter(|op| op.is_reading()) {
        let id = op.anilist_id();
        let at = match out.iter().position(|(existing, _)| *existing == id) {
            Some(at) => at,
            None => {
                out.push((id, Change::default()));
                out.len() - 1
            }
        };
        let change = &mut out[at].1;
        match op {
            TrackOp::SetReadProgress { chapter, volume, .. } => {
                change.chapter = change.chapter.max(Some(*chapter));
                change.volume = change.volume.max(*volume);
            }
            TrackOp::SetReadStatus { status, .. } => change.status = Some(*status),
            TrackOp::SetReadScore { score, .. } => change.score = Some(*score),
            _ => {}
        }
    }
    out
}

#[async_trait::async_trait]
impl Tracker for MangaUpdatesTracker {
    fn id(&self) -> &str {
        "mangaupdates"
    }

    fn is_authenticated(&self) -> bool {
        self.held.is_held()
    }

    fn serves_reading(&self) -> bool {
        true
    }

    fn serves_watching(&self) -> bool {
        false
    }

    fn can_address(&self, op: &TrackOp) -> bool {
        op.is_reading() && self.mapping.mangaupdates_id(op.anilist_id()).is_some()
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
        // The lists and entries belonged to whoever was signed in before.
        *self.lists.lock().await = None;
        self.entries.lock().await.clear();
    }

    async fn forget_credentials(&self) {
        self.held.forget().await;
        *self.lists.lock().await = None;
        self.entries.lock().await.clear();
    }

    async fn pull_library(
        &self,
    ) -> Result<Vec<kaeri_core::traits::TrackedEntry>, kaeri_core::Error> {
        // No anime lists here.
        Ok(Vec::new())
    }

    async fn push(&self, _ops: &[TrackOp]) -> Result<(), kaeri_core::Error> {
        Ok(())
    }

    async fn pull_read_library(&self) -> Result<Vec<TrackedReadEntry>, kaeri_core::Error> {
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        let lists = self.lists(&mut token).await?;
        let mut out = Vec::new();
        for list in &lists {
            let mut page = 1_u32;
            loop {
                let url = format!("{}/lists/{}/search", self.api, list.id);
                let body = serde_json::json!({ "page": page, "perpage": PER_PAGE });
                let response = self
                    .send(&mut token, false, |t| {
                        self.http.post(&url).bearer_auth(t).json(&body)
                    })
                    .await?;
                let answer = crate::page_json(response, "mangaupdates", "MangaUpdates").await?;
                let results = answer["results"].as_array().cloned().unwrap_or_default();
                for row in &results {
                    let record = &row["record"];
                    let Some(series) = record["series"]["id"].as_u64() else { continue };
                    let entry = Entry {
                        list_id: list.id,
                        chapter: record["status"]["chapter"].as_u64().unwrap_or(0) as u32,
                        volume: record["status"]["volume"].as_u64().unwrap_or(0) as u32,
                    };
                    self.remember(series, Some(entry.clone())).await;
                    let Some(anilist_id) = self.mapping.anilist_for_mangaupdates(series) else {
                        continue;
                    };
                    let novel = record["series"]["type"].as_str() == Some("Novel")
                        || row["metadata"]["series"]["type"].as_str() == Some("Novel");
                    out.push(TrackedReadEntry {
                        anilist_id,
                        medium: if novel { Medium::Novel } else { Medium::Manga },
                        chapters: entry.chapter,
                        volumes: Some(entry.volume),
                        status: list.status(),
                        rereading: false,
                        score: row["metadata"]["user_list"]["rating"]
                            .as_f64()
                            .map(|r| r as f32)
                            .filter(|r| *r > 0.0),
                    });
                }
                let total = answer["total_hits"].as_u64().unwrap_or(0);
                if results.is_empty()
                    || u64::from(page) * u64::from(PER_PAGE) >= total
                    || page >= MAX_PAGES
                {
                    break;
                }
                page += 1;
            }
        }
        tracing::info!(count = out.len(), "mangaupdates lists pulled");
        Ok(out)
    }

    async fn remove_read(
        &self,
        anilist_id: AnilistId,
    ) -> Result<ReadRemoval, kaeri_core::Error> {
        let Some(series) = self.mapping.mangaupdates_id(anilist_id) else {
            return Ok(ReadRemoval::Unmatched);
        };
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        // Asked afresh: a kept answer may be from before the series went on a list.
        self.entries.lock().await.remove(&series);
        if self.entry_of(&mut token, series).await?.is_none() {
            return Ok(ReadRemoval::NotOnList);
        }
        self.write(&mut token, "/lists/series/delete", &serde_json::json!([series])).await?;
        self.remember(series, None).await;
        Ok(ReadRemoval::Removed)
    }

    async fn push_read(&self, ops: &[TrackOp]) -> Result<Vec<AnilistId>, kaeri_core::Error> {
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        let lists = self.lists(&mut token).await?;
        let list_for = |status: WatchStatus| {
            let kind = list_type(status);
            lists.iter().find(|l| !l.custom && l.kind == kind).map(|l| l.id)
        };
        let reading = list_for(WatchStatus::Current);

        let mut ignored = Vec::new();
        let mut additions = Vec::new();
        let mut updates = Vec::new();
        let mut ratings = Vec::new();
        let mut written: Vec<(u64, Entry)> = Vec::new();
        for (anilist_id, change) in changes(ops) {
            let Some(series) = self.mapping.mangaupdates_id(anilist_id) else {
                ignored.push(anilist_id);
                continue;
            };
            let current = self.entry_of(&mut token, series).await?;
            let custom = current
                .as_ref()
                .is_some_and(|e| lists.iter().any(|l| l.id == e.list_id && l.custom));
            // Where the series goes: a status moves it, except off a custom list, which a
            // status push never does; a new one starts on the reading list.
            let target = match (&current, change.status) {
                (Some(entry), _) if custom => Some(entry.list_id),
                (Some(entry), None) => Some(entry.list_id),
                (_, Some(status)) => list_for(status).or(current.as_ref().map(|e| e.list_id)),
                (None, None) => reading,
            };
            let Some(list_id) = target else {
                ignored.push(anilist_id);
                continue;
            };
            let held = current.clone().unwrap_or(Entry { list_id, chapter: 0, volume: 0 });
            // Never lower than the list holds.
            let chapter = change.chapter.unwrap_or(0).max(held.chapter);
            let volume = change.volume.unwrap_or(0).max(held.volume);
            if current.is_none() {
                additions.push(
                    serde_json::json!({ "series": { "id": series }, "list_id": list_id }),
                );
            }
            let moved = current.as_ref().is_some_and(|e| e.list_id != list_id);
            let progressed = chapter != held.chapter || volume != held.volume;
            if current.is_none() && (chapter > 0 || volume > 0) || moved || progressed {
                // Added first, and the numbers set after: a series just added shows chapter 1.
                updates.push(serde_json::json!({
                    "series": { "id": series },
                    "list_id": list_id,
                    "status": { "chapter": chapter, "volume": volume },
                }));
            }
            if let Some(score) = change.score {
                ratings.push((series, score));
            }
            written.push((series, Entry { list_id, chapter, volume }));
        }

        if !additions.is_empty() {
            self.write(&mut token, "/lists/series", &serde_json::Value::Array(additions))
                .await?;
        }
        if !updates.is_empty() {
            self.write(&mut token, "/lists/series/update", &serde_json::Value::Array(updates))
                .await?;
        }
        for (series, score) in ratings {
            let url = format!("{}/series/{series}/rating", self.api);
            let body = serde_json::json!({ "rating": (score * 10.0).round() / 10.0 });
            let response = self
                .send(&mut token, false, |t| self.http.put(&url).bearer_auth(t).json(&body))
                .await?;
            crate::page_json(response, "mangaupdates", "MangaUpdates").await?;
        }
        for (series, entry) in written {
            self.remember(series, Some(entry)).await;
        }
        Ok(ignored)
    }
}
