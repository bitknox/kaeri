//! Finding which entry a work is on a tracker, when AniList's own `idMal` does not say.
//!
//! Three public, unauthenticated lookups, asked from the person's device for one work at a time
//! and only while a tracker that needs the answer is signed in:
//!
//! - MangaBaka's `/v1/source/anilist/{id}` gives the MyAnimeList, Kitsu and MangaUpdates ids
//!   for an AniList id. Its data is CC BY-NC-SA 4.0 and its outside ids are third-party data:
//!   nothing here is shipped or cached beyond the person's own store, and the About page
//!   credits MangaBaka. MangaBaka writes a MangaUpdates id in base 36.
//! - Kitsu's `/mappings` names a Kitsu manga from an `anilist/manga` or `myanimelist/manga` id.
//! - MangaUpdates' `POST /series/search` gives candidates by title.
//!
//! Each service is spaced by its own [`Pacer`] and answers are decoded strictly: a field the
//! service stops sending is an error, not a guess.

use std::time::Duration;

use serde::Deserialize;

use crate::mangaupdates::Pacer;

pub const MANGABAKA: &str = "https://api.mangabaka.org";
pub const KITSU: &str = "https://kitsu.app/api/edge";
pub const MANGAUPDATES: &str = "https://api.mangaupdates.com/v1";

const AGENT: &str = concat!("Kaeri/", env!("CARGO_PKG_VERSION"), " (+https://kaeri.app)");

/// The ids MangaBaka knows for one work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceIds {
    pub my_anime_list: Option<u32>,
    pub kitsu: Option<u32>,
    pub manga_updates: Option<u64>,
}

/// A MangaUpdates series that may be the work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub series_id: u64,
    pub title: String,
    pub year: Option<u16>,
    pub novel: bool,
}

#[derive(Deserialize)]
struct BakaReply {
    data: BakaData,
}

#[derive(Deserialize)]
struct BakaData {
    series: Vec<BakaSeries>,
}

#[derive(Deserialize)]
struct BakaSeries {
    state: String,
    source: BakaSources,
}

#[derive(Deserialize)]
struct BakaSources {
    #[serde(default)]
    my_anime_list: Option<BakaRef<u32>>,
    #[serde(default)]
    kitsu: Option<BakaRef<u32>>,
    #[serde(default)]
    manga_updates: Option<BakaRef<String>>,
}

#[derive(Deserialize)]
struct BakaRef<T> {
    id: T,
}

/// MangaBaka's reply as ids. A merged or removed series is not one to trust.
pub fn decode_mangabaka(body: &str) -> Result<Option<SourceIds>, serde_json::Error> {
    let reply: BakaReply = serde_json::from_str(body)?;
    let Some(series) = reply.data.series.into_iter().find(|s| s.state == "active") else {
        return Ok(None);
    };
    Ok(Some(SourceIds {
        my_anime_list: series.source.my_anime_list.map(|r| r.id),
        kitsu: series.source.kitsu.map(|r| r.id),
        manga_updates: series
            .source
            .manga_updates
            .and_then(|r| u64::from_str_radix(r.id.trim(), 36).ok()),
    }))
}

#[derive(Deserialize)]
struct KitsuMappings {
    data: Vec<KitsuMapping>,
}

#[derive(Deserialize)]
struct KitsuMapping {
    relationships: KitsuRelationships,
}

#[derive(Deserialize)]
struct KitsuRelationships {
    item: KitsuItem,
}

#[derive(Deserialize)]
struct KitsuItem {
    #[serde(default)]
    data: Option<KitsuTarget>,
}

#[derive(Deserialize)]
struct KitsuTarget {
    #[serde(rename = "type")]
    kind: String,
    id: String,
}

/// The Kitsu manga a mapping lookup named.
pub fn decode_kitsu_mapping(body: &str) -> Result<Option<u32>, serde_json::Error> {
    let reply: KitsuMappings = serde_json::from_str(body)?;
    Ok(reply
        .data
        .into_iter()
        .filter_map(|m| m.relationships.item.data)
        .find(|t| t.kind == "manga")
        .and_then(|t| t.id.parse().ok()))
}

#[derive(Deserialize)]
struct SearchReply {
    results: Vec<SearchHit>,
}

#[derive(Deserialize)]
struct SearchHit {
    record: SearchRecord,
}

#[derive(Deserialize)]
struct SearchRecord {
    series_id: u64,
    title: String,
    #[serde(rename = "type")]
    kind: String,
    year: String,
}

/// MangaUpdates' search hits as candidates. Doujinshi and art books are not what a work is.
pub fn decode_search(body: &str) -> Result<Vec<Candidate>, serde_json::Error> {
    let reply: SearchReply = serde_json::from_str(body)?;
    Ok(reply
        .results
        .into_iter()
        .map(|h| h.record)
        .filter(|r| !matches!(r.kind.as_str(), "Doujinshi" | "Artbook"))
        .map(|r| Candidate {
            series_id: r.series_id,
            title: r.title,
            year: r.year.trim().parse().ok(),
            novel: r.kind == "Novel",
        })
        .collect())
}

/// The lookups, against the services or against stand-ins.
pub struct Lookups {
    http: reqwest::Client,
    baka: String,
    kitsu: String,
    updates: String,
    baka_pace: Pacer,
    kitsu_pace: Pacer,
    updates_pace: Pacer,
}

impl Lookups {
    pub fn new(http: reqwest::Client) -> Self {
        let gap = Duration::from_secs(1);
        Self {
            http,
            baka: MANGABAKA.into(),
            kitsu: KITSU.into(),
            updates: MANGAUPDATES.into(),
            baka_pace: Pacer::new(gap, gap),
            kitsu_pace: Pacer::new(gap, gap),
            updates_pace: Pacer::new(gap, gap),
        }
    }

    /// Ask stand-ins instead of the services, without waiting between asks: for a test.
    #[must_use]
    pub fn at(mut self, baka: &str, kitsu: &str, updates: &str) -> Self {
        self.baka = baka.trim_end_matches('/').into();
        self.kitsu = kitsu.trim_end_matches('/').into();
        self.updates = updates.trim_end_matches('/').into();
        let none = Duration::ZERO;
        self.baka_pace = Pacer::new(none, none);
        self.kitsu_pace = Pacer::new(none, none);
        self.updates_pace = Pacer::new(none, none);
        self
    }

    fn failed(service: &str, message: String) -> kaeri_core::Error {
        kaeri_core::Error::Tracker { tracker: service.to_owned(), message }
    }

    /// MangaBaka's ids for an AniList manga. `None` when it does not know the work.
    pub async fn mangabaka(
        &self,
        anilist_id: u32,
    ) -> Result<Option<SourceIds>, kaeri_core::Error> {
        self.baka_pace.space(false).await;
        let response = self
            .http
            .get(format!("{}/v1/source/anilist/{anilist_id}", self.baka))
            .header("User-Agent", AGENT)
            .send()
            .await
            .map_err(|e| Self::failed("mangabaka", e.to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(Self::failed("mangabaka", crate::said(status, &body)));
        }
        decode_mangabaka(&body).map_err(|e| Self::failed("mangabaka", e.to_string()))
    }

    /// The Kitsu manga Kitsu itself maps to `site` (`anilist/manga` or `myanimelist/manga`)
    /// entry `external_id`.
    pub async fn kitsu_manga(
        &self,
        site: &str,
        external_id: u32,
    ) -> Result<Option<u32>, kaeri_core::Error> {
        self.kitsu_pace.space(false).await;
        let response = self
            .http
            .get(format!("{}/mappings", self.kitsu))
            .query(&[
                ("filter[externalSite]", site),
                ("filter[externalId]", &external_id.to_string()),
                ("include", "item"),
                ("fields[manga]", "canonicalTitle"),
            ])
            .header("User-Agent", AGENT)
            .header("Accept", "application/vnd.api+json")
            .send()
            .await
            .map_err(|e| Self::failed("kitsu", e.to_string()))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(Self::failed("kitsu", crate::said(status, &body)));
        }
        decode_kitsu_mapping(&body).map_err(|e| Self::failed("kitsu", e.to_string()))
    }

    /// MangaUpdates' series matching `title`, best first.
    pub async fn search_mangaupdates(
        &self,
        title: &str,
    ) -> Result<Vec<Candidate>, kaeri_core::Error> {
        self.updates_pace.space(false).await;
        let response = self
            .http
            .post(format!("{}/series/search", self.updates))
            .header("User-Agent", AGENT)
            .json(&serde_json::json!({ "search": title, "perpage": 8, "page": 1 }))
            .send()
            .await
            .map_err(|e| Self::failed("mangaupdates", e.to_string()))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(Self::failed("mangaupdates", crate::said(status, &body)));
        }
        let mut found =
            decode_search(&body).map_err(|e| Self::failed("mangaupdates", e.to_string()))?;
        found.truncate(8);
        Ok(found)
    }
}
