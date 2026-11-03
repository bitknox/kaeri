//! Episode synopses for a whole title from one request: the optional fill for rows a source
//! listed without text.
//!
//! The service is a public mapping-keyed one at `api.ani.zip`. `GET /mappings?anilist_id=<id>`
//! answers with an `episodes` map keyed by episode number, each with an English `overview`
//! (from TVDB), sometimes a `summary`, a title, a still and an air date. It publishes no terms
//! and no rate limit, and sits behind a cache of about 15 minutes, so it is used lightly:
//!
//! - One request per title and listing, asked beside the source's listing, never before it.
//! - The answer is kept on the device in the store's key/value table, for [`AIRING_FOR`] while
//!   the show airs and [`SETTLED_FOR`] after, so a title opened again asks nothing.
//! - A `429`, a `5xx` or a dropped connection rests the service for a while ([`REST`], or the
//!   `Retry-After` it sends): asks during the rest are refused at once instead of waited for.
//! - Each request has a timeout, and nothing waits on it: the list is already said.
//!
//! The episodes are keyed by AniList's numbering, which is how the service numbers them. A
//! title whose source counts differently is put on by the caller through its numbering, and
//! left alone where the two cannot be lined up. Only the whole numbers are kept: specials and
//! other keys are dropped.
//!
//! It is off when `[meta] episode_text = false`.

use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use kaeri_core::ids::AnilistId;
use kaeri_net::HttpClient;
use kaeri_store::Store;

const API: &str = "https://api.ani.zip";

/// How long one request has, connect to last byte.
pub const TIMEOUT: Duration = Duration::from_secs(8);

/// How long an answer for an airing show keeps: a new episode's text turns up within hours.
pub const AIRING_FOR: i64 = 6 * 3_600;

/// How long an answer for a settled show keeps.
pub const SETTLED_FOR: i64 = 7 * 24 * 3_600;

/// How long a title the service does not know keeps.
pub const UNKNOWN_FOR: i64 = 24 * 3_600;

/// How long the service is left alone after a refusal or a failure with no word of its own.
pub const REST: Duration = Duration::from_secs(300);

/// The longest a rest lasts, whatever `Retry-After` says.
const REST_MAX: Duration = Duration::from_secs(3_600);

/// The most of an answer that is read.
const MAX_BODY: usize = 8 * 1024 * 1024;

/// The most of one synopsis that is kept.
const MAX_TEXT: usize = 4_000;

const KEY: &str = "episode_text:";

#[derive(Debug, Clone, thiserror::Error)]
pub enum EpisodeTextError {
    #[error("episode text: resting after a refusal")]
    Resting,
    #[error("episode text: {0}")]
    Failed(String),
}

/// What was kept of a title: the synopses by AniList's episode number, and until when.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Kept {
    pub until: i64,
    pub texts: BTreeMap<u32, String>,
}

impl Kept {
    /// Whether the answer is still fresh at `now`.
    pub fn fresh(&self, now: i64) -> bool {
        self.until > now
    }
}

/// The episode text client.
pub struct EpisodeText {
    http: HttpClient,
    store: Store,
    api: String,
    rest_until: Mutex<Option<Instant>>,
}

impl EpisodeText {
    pub fn new(http: HttpClient, store: Store) -> Self {
        Self { http, store, api: API.to_owned(), rest_until: Mutex::new(None) }
    }

    /// Ask `base` instead of the service: a stand-in on loopback, for tests and probes.
    #[doc(hidden)]
    pub fn with_endpoint(mut self, base: impl Into<String>) -> Self {
        self.api = base.into();
        self
    }

    /// What is kept of title `id`, of any age.
    pub fn kept(&self, id: AnilistId) -> Option<Kept> {
        let raw = self.store.get_meta_text(&format!("{KEY}{}", id.get())).ok().flatten()?;
        serde_json::from_str(&raw).ok()
    }

    /// Title `id`'s synopses by AniList's number: from what is kept while it is fresh at
    /// `now`, else from one request. `airing` decides how long a new answer keeps.
    pub async fn texts(
        &self,
        id: AnilistId,
        airing: bool,
        now: i64,
    ) -> Result<BTreeMap<u32, String>, EpisodeTextError> {
        if let Some(kept) = self.kept(id).filter(|kept| kept.fresh(now)) {
            return Ok(kept.texts);
        }
        let (texts, unknown) = self.fetch(id).await?;
        let keeps = if unknown {
            UNKNOWN_FOR
        } else if airing {
            AIRING_FOR
        } else {
            SETTLED_FOR
        };
        let kept = Kept { until: now + keeps, texts };
        if let Ok(raw) = serde_json::to_string(&kept)
            && let Err(error) = self.store.set_meta_text(&format!("{KEY}{}", id.get()), &raw)
        {
            tracing::warn!(%error, "could not keep episode text");
        }
        Ok(kept.texts)
    }

    fn resting(&self) -> bool {
        let mut rest = self.rest_until.lock().unwrap_or_else(|p| p.into_inner());
        match *rest {
            Some(until) if until > Instant::now() => true,
            Some(_) => {
                *rest = None;
                false
            }
            None => false,
        }
    }

    fn rest(&self, for_: Duration) {
        let until = Instant::now() + for_.min(REST_MAX);
        *self.rest_until.lock().unwrap_or_else(|p| p.into_inner()) = Some(until);
    }

    /// One request. With the texts, and whether the service does not know the title.
    async fn fetch(
        &self,
        id: AnilistId,
    ) -> Result<(BTreeMap<u32, String>, bool), EpisodeTextError> {
        if self.resting() {
            return Err(EpisodeTextError::Resting);
        }
        let response = self
            .http
            .plain()
            .get(format!("{}/mappings", self.api))
            .query(&[("anilist_id", id.get().to_string())])
            .timeout(TIMEOUT)
            .send()
            .await
            .map_err(|error| {
                self.rest(REST);
                EpisodeTextError::Failed(error.to_string())
            })?;
        let status = response.status();
        if status.as_u16() == 404 {
            return Ok((BTreeMap::new(), true));
        }
        if status.as_u16() == 429 || status.is_server_error() {
            let wait = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map_or(REST, Duration::from_secs);
            self.rest(wait);
            return Err(EpisodeTextError::Failed(format!("answered {status}")));
        }
        if !status.is_success() {
            return Err(EpisodeTextError::Failed(format!("answered {status}")));
        }
        if response.content_length().is_some_and(|n| n as usize > MAX_BODY) {
            return Err(EpisodeTextError::Failed("answer too large".into()));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| EpisodeTextError::Failed(error.to_string()))?;
        if bytes.len() > MAX_BODY {
            return Err(EpisodeTextError::Failed("answer too large".into()));
        }
        let body: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| EpisodeTextError::Failed(error.to_string()))?;
        Ok((texts_of(&body), false))
    }
}

/// The synopses in an answer by whole episode number: `overview`, else `summary`.
pub fn texts_of(body: &serde_json::Value) -> BTreeMap<u32, String> {
    let mut texts = BTreeMap::new();
    let Some(episodes) = body.get("episodes").and_then(|e| e.as_object()) else {
        return texts;
    };
    for (key, episode) in episodes {
        let Ok(number) = key.trim().parse::<u32>() else { continue };
        let text = ["overview", "summary"]
            .iter()
            .filter_map(|field| episode.get(*field).and_then(|v| v.as_str()))
            .map(str::trim)
            .find(|text| !text.is_empty());
        if let Some(text) = text {
            texts.insert(number, text.chars().take(MAX_TEXT).collect());
        }
    }
    texts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_leads_summary_follows_and_other_keys_are_dropped() {
        let body = serde_json::json!({ "episodes": {
            "1": { "overview": " A lantern. ", "summary": "Short." },
            "2": { "overview": "", "summary": "Only a summary." },
            "3": { "title": "No text" },
            "S1": { "overview": "A special." },
        }});
        let texts = texts_of(&body);
        assert_eq!(texts.get(&1).map(String::as_str), Some("A lantern."));
        assert_eq!(texts.get(&2).map(String::as_str), Some("Only a summary."));
        assert_eq!(texts.len(), 2);
    }
}
