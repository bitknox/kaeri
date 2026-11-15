//! Episode synopses, titles and stills for a whole title from one request: the optional fill for
//! rows a source listed without them.
//!
//! The service is a public mapping-keyed one at `api.ani.zip`. `GET /mappings?anilist_id=<id>`
//! answers with an `episodes` map keyed by episode number, each with an English `overview`
//! (from TVDB), sometimes a `summary`, a title by language, a still and an air date. It
//! publishes no terms and no rate limit, and sits behind a cache of about 15 minutes, so it is
//! used lightly:
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

/// The most of one episode title that is kept.
const MAX_TITLE: usize = 300;

/// The longest still address that is kept.
const MAX_URL: usize = 2_048;

/// What a kept answer was read for. One kept before titles and stills were read has none, so
/// it is asked again when online, and its synopses still serve until then.
const VERSION: u8 = 2;

const KEY: &str = "episode_text:";

#[derive(Debug, Clone, thiserror::Error)]
pub enum EpisodeTextError {
    #[error("episode text: resting after a refusal")]
    Resting,
    #[error("episode text: {0}")]
    Failed(String),
}

/// What the service says of a title's episodes, each by AniList's episode number.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fill {
    /// Synopses.
    pub texts: BTreeMap<u32, String>,
    /// Titles, in English, else in romaji.
    pub titles: BTreeMap<u32, String>,
    /// Stills, as `https` addresses.
    pub stills: BTreeMap<u32, String>,
}

/// What was kept of a title, and until when.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Kept {
    pub until: i64,
    texts: BTreeMap<u32, String>,
    #[serde(default)]
    titles: BTreeMap<u32, String>,
    #[serde(default)]
    stills: BTreeMap<u32, String>,
    #[serde(default)]
    version: u8,
}

impl Kept {
    fn new(until: i64, fill: Fill) -> Self {
        let Fill { texts, titles, stills } = fill;
        Self { until, texts, titles, stills, version: VERSION }
    }

    /// Whether the answer is still fresh at `now`, and was read for all [`Fill`] holds.
    pub fn fresh(&self, now: i64) -> bool {
        self.version >= VERSION && self.until > now
    }

    /// What was kept, of any age.
    pub fn fill(&self) -> Fill {
        Fill {
            texts: self.texts.clone(),
            titles: self.titles.clone(),
            stills: self.stills.clone(),
        }
    }
}

/// The episode fill's client: synopses, titles and stills by AniList id.
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

    /// What the service says of title `id`'s episodes: from what is kept while it is fresh at
    /// `now`, else from one request. `airing` decides how long a new answer keeps.
    pub async fn fill(
        &self,
        id: AnilistId,
        airing: bool,
        now: i64,
    ) -> Result<Fill, EpisodeTextError> {
        if let Some(kept) = self.kept(id).filter(|kept| kept.fresh(now)) {
            return Ok(kept.fill());
        }
        let (fill, unknown) = self.fetch(id).await?;
        let keeps = if unknown {
            UNKNOWN_FOR
        } else if airing {
            AIRING_FOR
        } else {
            SETTLED_FOR
        };
        let kept = Kept::new(now + keeps, fill);
        if let Ok(raw) = serde_json::to_string(&kept)
            && let Err(error) = self.store.set_meta_text(&format!("{KEY}{}", id.get()), &raw)
        {
            tracing::warn!(%error, "could not keep episode text");
        }
        Ok(kept.fill())
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

    /// One request. With what it says, and whether the service does not know the title.
    async fn fetch(&self, id: AnilistId) -> Result<(Fill, bool), EpisodeTextError> {
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
            return Ok((Fill::default(), true));
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
        Ok((fill_in(&body), false))
    }
}

/// What an answer says by whole episode number: the synopsis from `overview`, else `summary`;
/// the title in English, else in romaji; and the still from `image` when it is an `https`
/// address.
pub fn fill_in(body: &serde_json::Value) -> Fill {
    let mut fill = Fill::default();
    let Some(episodes) = body.get("episodes").and_then(|e| e.as_object()) else {
        return fill;
    };
    for (key, episode) in episodes {
        let Ok(number) = key.trim().parse::<u32>() else { continue };
        if number == 0 {
            continue;
        }
        let text = ["overview", "summary"]
            .iter()
            .filter_map(|field| episode.get(*field).and_then(|v| v.as_str()))
            .map(str::trim)
            .find(|text| !text.is_empty());
        if let Some(text) = text {
            fill.texts.insert(number, text.chars().take(MAX_TEXT).collect());
        }
        let title = ["en", "x-jat"]
            .iter()
            .filter_map(|language| episode.get("title")?.get(*language)?.as_str())
            .map(str::trim)
            .find(|title| !title.is_empty());
        if let Some(title) = title {
            fill.titles.insert(number, title.chars().take(MAX_TITLE).collect());
        }
        let still = episode
            .get("image")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|url| url.starts_with("https://") && url.len() <= MAX_URL)
            .filter(|url| !url.chars().any(char::is_whitespace));
        if let Some(still) = still {
            fill.stills.insert(number, still.to_owned());
        }
    }
    fill
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
        let texts = fill_in(&body).texts;
        assert_eq!(texts.get(&1).map(String::as_str), Some("A lantern."));
        assert_eq!(texts.get(&2).map(String::as_str), Some("Only a summary."));
        assert_eq!(texts.len(), 2);
    }

    #[test]
    fn titles_are_english_then_romaji_and_stills_only_https() {
        let body = serde_json::json!({ "episodes": {
            "1": {
                "title": { "ja": "灯", "en": " The Lantern ", "x-jat": "Tomoshibi" },
                "image": "https://img.episodes.example/1.jpg",
            },
            "2": { "title": { "en": "", "x-jat": "Kaze no Michi" }, "image": "http://img.episodes.example/2.jpg" },
            "3": { "title": { "ja": "星" }, "image": "https://img.episodes.example/a b.jpg" },
            "0": { "title": { "en": "A Prologue" }, "image": "https://img.episodes.example/0.jpg" },
        }});
        let found = fill_in(&body);
        assert_eq!(found.titles.get(&1).map(String::as_str), Some("The Lantern"));
        assert_eq!(found.titles.get(&2).map(String::as_str), Some("Kaze no Michi"));
        assert_eq!(found.titles.len(), 2);
        assert_eq!(
            found.stills.get(&1).map(String::as_str),
            Some("https://img.episodes.example/1.jpg")
        );
        assert_eq!(found.stills.len(), 1);
    }

    #[test]
    fn an_answer_kept_before_titles_and_stills_were_read_is_stale_but_still_read() {
        let old: Kept =
            serde_json::from_str(r#"{"until":100,"texts":{"1":"A lantern."}}"#).unwrap();
        assert_eq!(old.fill().texts.get(&1).map(String::as_str), Some("A lantern."));
        assert!(old.fill().titles.is_empty());
        assert!(!old.fresh(50));

        let new = Kept::new(100, Fill::default());
        let again: Kept = serde_json::from_str(&serde_json::to_string(&new).unwrap()).unwrap();
        assert!(again.fresh(50));
        assert!(!again.fresh(100));
    }
}
