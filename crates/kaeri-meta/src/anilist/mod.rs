//! AniList GraphQL client.
//!
//! The stable spine. Everything else in Kaeri can fail and the app degrades; if this
//! layer is unavailable there is nothing to browse, so it is the one place where rate
//! limiting and caching are treated as correctness rather than optimisation.
//!
//! The observed budget is **30 requests/minute**, shared across search, seasonal, calendar
//! and library. That is small enough that a careless screen refresh can exhaust it, which
//! is why queries are wide (one round trip per screen) and responses are cached.

pub mod books;
pub mod model;
pub mod query;
pub mod readlist;
pub mod threads;
pub mod trends;

use std::sync::Arc;

use kaeri_core::ids::AnilistId;
use kaeri_net::{
    HttpClient, RateLimiter,
    ratelimit::{Pacing, Priority, parse_rate_headers, parse_rate_limit},
};
use serde::de::DeserializeOwned;

pub use model::{
    Broadcast, DescriptionSpan, ExternalLink, FuzzyDate, LastAired, Media, Ranking, Relation,
    SourceMaterial, StreamingEpisode, Tag, Title, TitleLanguage, Viewer, spoiler_runs,
};
pub use threads::{Liked, Thread, ThreadComment};

const ENDPOINT: &str = "https://graphql.anilist.co";

#[derive(Debug, Clone, thiserror::Error)]
pub enum AniListError {
    #[error("network: {0}")]
    Network(String),

    /// AniList returns `200` with an `errors` array rather than an HTTP error status, so
    /// this is the *normal* failure shape and has to be checked explicitly.
    #[error("anilist: {0}")]
    Api(String),

    #[error("unexpected response shape: {0}")]
    Decode(String),

    /// AniList refused the request for going over the limit, and kept refusing after waiting
    /// as its headers said. Never a reason to stop asking AniList: the caller keeps what it
    /// has, and asks again after `retry_in_secs`.
    #[error("rate limited, retry in {retry_in_secs}s")]
    RateLimited { retry_in_secs: u64 },

    #[error("not found")]
    NotFound,

    #[error("authentication required")]
    Unauthenticated,

    /// AniList's own trouble on a list write: a `5xx`, whatever its body says. Only list writes
    /// answer with this, so the tracker outbox can wait it out rather than count it as a
    /// refusal of the change. Every other ask reports a `5xx` as `Api` or `Decode`, as before.
    #[error("anilist: HTTP {status}: {message}")]
    Server { status: u16, message: String },
}

pub type Result<T> = std::result::Result<T, AniListError>;

/// Which season a browse query is for, and the one a title premiered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Fall,
}

impl Season {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Winter => "WINTER",
            Self::Spring => "SPRING",
            Self::Summer => "SUMMER",
            Self::Fall => "FALL",
        }
    }

    /// The season AniList calls `name`.
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Winter, Self::Spring, Self::Summer, Self::Fall]
            .into_iter()
            .find(|season| season.as_str() == name)
    }

    /// The season containing a given month.
    pub const fn of_month(month: u8) -> Self {
        match month {
            1..=3 => Self::Winter,
            4..=6 => Self::Spring,
            7..=9 => Self::Summer,
            _ => Self::Fall,
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Winter => Self::Spring,
            Self::Spring => Self::Summer,
            Self::Summer => Self::Fall,
            Self::Fall => Self::Winter,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Winter => Self::Fall,
            Self::Spring => Self::Winter,
            Self::Summer => Self::Spring,
            Self::Fall => Self::Summer,
        }
    }
}

/// Server-side browse filters: every argument of `Page.media` a search can set, in AniList's
/// own words and values.
///
/// Every field here is applied by AniList rather than locally, which is what makes a wide
/// filter surface affordable against a 30/minute budget. Bounds are exclusive, as AniList's
/// `_greater` and `_lesser` arguments are. An unset field is left out of the request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BrowseFilter {
    /// AniList's sort names, the first deciding. Empty is relevance with a term and
    /// popularity without. Ties are broken by id either way, so pages do not shuffle.
    pub sort: Vec<String>,
    /// Every one of these.
    pub genres: Vec<String>,
    pub genres_excluded: Vec<String>,
    /// Every one of these.
    pub tags: Vec<String>,
    pub tags_excluded: Vec<String>,
    pub min_tag_rank: Option<u32>,
    /// Any of these (`TV`, `TV_SHORT`, …).
    pub formats: Vec<String>,
    /// Any of these (`RELEASING`, …).
    pub statuses: Vec<String>,
    /// `WINTER`…
    pub season: Option<String>,
    /// The year of [`Self::season`], or on its own, the year a title aired in.
    pub year: Option<u32>,
    /// Start dates as AniList's `YYYYMMDD` integers, exclusive.
    pub started_after: Option<u32>,
    pub started_before: Option<u32>,
    pub episodes_greater: Option<u32>,
    pub episodes_lesser: Option<u32>,
    /// Minutes an episode.
    pub duration_greater: Option<u32>,
    pub duration_lesser: Option<u32>,
    /// Average score, 0 to 100, above this.
    pub min_score: Option<u32>,
    /// People with the title on a list, above this.
    pub min_popularity: Option<u32>,
    /// Any of these (`MANGA`, `LIGHT_NOVEL`, …).
    pub sources: Vec<String>,
    /// Any of these ISO codes.
    pub countries: Vec<String>,
    /// Streaming officially on any of these sites.
    pub licensed_by: Vec<String>,
    pub licensed: Option<bool>,
    /// On the signed-in viewer's lists, or not; needs the token.
    pub on_list: Option<bool>,
    /// Adult or not; `None` asks for both.
    pub adult: Option<bool>,
    /// Only these titles: Search's Available filter at Plays here, which keeps to the titles the
    /// viewer's own sources have.
    pub ids: Vec<u32>,
}

impl BrowseFilter {
    pub fn is_empty(&self) -> bool {
        *self == Self { adult: self.adult, ..Self::default() }
    }

    /// The arguments of one page of this browse for `term`, as [`query::media_pages`] takes
    /// them: only those set.
    pub fn arguments(
        &self,
        term: &str,
        page: u32,
        per_page: u32,
    ) -> Vec<(query::Argument, serde_json::Value)> {
        use serde_json::json;
        let term = term.trim();
        let mut out: Vec<(query::Argument, serde_json::Value)> = vec![
            (("page", "Int"), json!(page.max(1))),
            (("perPage", "Int"), json!(per_page.clamp(1, 50))),
        ];
        let mut sort = self.sort.clone();
        if sort.is_empty() {
            // `SEARCH_MATCH` means nothing without a term to match.
            sort.push(if term.is_empty() { "POPULARITY_DESC" } else { "SEARCH_MATCH" }.into());
        }
        if !sort.iter().any(|s| s == "ID" || s == "ID_DESC") {
            sort.push("ID".into());
        }
        out.push((("sort", "[MediaSort]"), json!(sort)));
        if !term.is_empty() {
            out.push((("search", "String"), json!(term)));
        }
        let lists: [(query::Argument, &Vec<String>); 9] = [
            (("genre_in", "[String]"), &self.genres),
            (("genre_not_in", "[String]"), &self.genres_excluded),
            (("tag_in", "[String]"), &self.tags),
            (("tag_not_in", "[String]"), &self.tags_excluded),
            (("format_in", "[MediaFormat]"), &self.formats),
            (("status_in", "[MediaStatus]"), &self.statuses),
            (("source_in", "[MediaSource]"), &self.sources),
            (("licensedBy_in", "[String]"), &self.licensed_by),
            (("countryOfOrigin_in", "[CountryCode]"), &self.countries),
        ];
        for (argument, values) in lists {
            if values.is_empty() {
                continue;
            }
            // One country goes as the plain argument, which every schema has had.
            if argument.0 == "countryOfOrigin_in" && values.len() == 1 {
                out.push((("countryOfOrigin", "CountryCode"), json!(values[0])));
            } else {
                out.push((argument, json!(values)));
            }
        }
        if !self.tags.is_empty() || !self.tags_excluded.is_empty() {
            out.push((("minimumTagRank", "Int"), json!(self.min_tag_rank.unwrap_or(60))));
        }
        if let Some(season) = &self.season {
            out.push((("season", "MediaSeason"), json!(season)));
        }
        let numbers: [(query::Argument, Option<u32>); 9] = [
            (("seasonYear", "Int"), self.year),
            (("startDate_greater", "FuzzyDateInt"), self.started_after),
            (("startDate_lesser", "FuzzyDateInt"), self.started_before),
            (("episodes_greater", "Int"), self.episodes_greater),
            (("episodes_lesser", "Int"), self.episodes_lesser),
            (("duration_greater", "Int"), self.duration_greater),
            (("duration_lesser", "Int"), self.duration_lesser),
            (("averageScore_greater", "Int"), self.min_score),
            (("popularity_greater", "Int"), self.min_popularity),
        ];
        for (argument, value) in numbers {
            if let Some(value) = value {
                out.push((argument, json!(value)));
            }
        }
        if !self.ids.is_empty() {
            out.push((("id_in", "[Int]"), json!(self.ids)));
        }
        let flags: [(query::Argument, Option<bool>); 3] = [
            (("isLicensed", "Boolean"), self.licensed),
            (("onList", "Boolean"), self.on_list),
            (("isAdult", "Boolean"), self.adult),
        ];
        for (argument, value) in flags {
            if let Some(value) = value {
                out.push((argument, json!(value)));
            }
        }
        out
    }

    /// A filter with every argument set, for the tests that hold the documents to GraphQL.
    #[cfg(test)]
    pub(crate) fn every_argument() -> Self {
        Self {
            sort: vec!["SCORE_DESC".into()],
            genres: vec!["Mecha".into()],
            genres_excluded: vec!["Romance".into()],
            tags: vec!["Time Travel".into()],
            tags_excluded: vec!["Isekai".into()],
            min_tag_rank: Some(80),
            formats: vec!["TV".into(), "MOVIE".into()],
            statuses: vec!["FINISHED".into()],
            season: Some("FALL".into()),
            year: Some(2024),
            started_after: Some(2009_9999),
            started_before: Some(2020_0000),
            episodes_greater: Some(0),
            episodes_lesser: Some(13),
            duration_greater: Some(15),
            duration_lesser: Some(61),
            min_score: Some(69),
            min_popularity: Some(1000),
            sources: vec!["MANGA".into()],
            countries: vec!["JP".into(), "KR".into()],
            licensed_by: vec!["Hikari Library".into()],
            licensed: Some(true),
            on_list: Some(false),
            adult: Some(false),
            ids: vec![900_001, 900_002],
        }
    }
}

/// What the filters choose from, as AniList lists it.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Facets {
    pub genres: Vec<String>,
    pub tags: Vec<FacetTag>,
    /// Streaming sites, each once, in AniList's order.
    pub streaming: Vec<String>,
}

/// One of AniList's tags.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FacetTag {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// "Theme-Action", "Cast-Main Cast", "Setting-Scene", "Demographic".
    #[serde(default)]
    pub category: String,
    #[serde(default, rename = "isAdult")]
    pub adult: bool,
    #[serde(default, rename = "isGeneralSpoiler")]
    pub spoiler: bool,
}

tokio::task_local! {
    /// The class the caller's AniList requests go in: see [`Lane`].
    static LANE: Lane;
}

/// The class a piece of work's AniList requests go in, shared by everything the work asks, and
/// raised when someone more urgent comes to wait on it. Work outside any lane is a screen's.
///
/// A lane is what lets a background ask be joined: a title's page asked for in the background
/// that its page then opens on is raised to the screen's class while it waits, rather than
/// keeping the page in the queue.
#[derive(Clone, Debug)]
pub struct Lane(Arc<std::sync::atomic::AtomicU8>);

impl Lane {
    pub fn new(priority: Priority) -> Self {
        Self(Arc::new(std::sync::atomic::AtomicU8::new(rank(priority))))
    }

    /// The caller's own lane, or a new one in the caller's class.
    pub fn current() -> Self {
        LANE.try_with(Clone::clone).unwrap_or_else(|_| Self::new(Priority::Screen))
    }

    pub fn priority(&self) -> Priority {
        match self.0.load(std::sync::atomic::Ordering::SeqCst) {
            0 => Priority::Screen,
            1 => Priority::OnScreen,
            _ => Priority::Background,
        }
    }

    /// Whether `other` is this lane, rather than one in the same class.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Raise the lane to `priority`, if that is more urgent than it is.
    pub fn raise(&self, priority: Priority) {
        self.0.fetch_min(rank(priority), std::sync::atomic::Ordering::SeqCst);
    }

    /// Run `work` in this lane.
    pub async fn run<F: Future>(&self, work: F) -> F::Output {
        LANE.scope(self.clone(), work).await
    }
}

const fn rank(priority: Priority) -> u8 {
    match priority {
        Priority::Screen => 0,
        Priority::OnScreen => 1,
        Priority::Background => 2,
    }
}

/// AniList requests made inside `work` are background work: queued behind what is on screen,
/// paced, and kept out of the share left for screens (see [`kaeri_net::ratelimit::Pacing`]). A
/// sync, a relations walk or the new-episode check never makes a search wait.
///
/// Inside work for the page on screen ([`on_screen`]) this changes nothing: that caller knows
/// the work has someone looking at it.
pub async fn in_background<F: Future>(work: F) -> F::Output {
    match LANE.try_with(Clone::clone) {
        Ok(lane) if lane.priority() != Priority::Screen => lane.run(work).await,
        _ => Lane::new(Priority::Background).run(work).await,
    }
}

/// AniList requests made inside `work` serve the page on screen, which shows without them and
/// fills in as they land: the page's franchise walk, its seasons' headers, the open Calendar's
/// walk. They go right after any screen request waiting, without the background's spacing,
/// while more than the screens' share is left.
pub async fn on_screen<F: Future>(work: F) -> F::Output {
    match LANE.try_with(Clone::clone) {
        Ok(lane) if lane.priority() == Priority::OnScreen => lane.run(work).await,
        _ => Lane::new(Priority::OnScreen).run(work).await,
    }
}

/// The class the caller's requests go in.
pub fn priority() -> Priority {
    LANE.try_with(Lane::priority).unwrap_or(Priority::Screen)
}

/// Whether the caller is doing work nobody is looking at: see [`in_background`].
pub fn is_background() -> bool {
    priority() == Priority::Background
}

/// The most titles [`AniList::media_many`] asks after in one request: four pages of fifty.
/// Each page's titles are counted once against AniList's complexity limit, so four stay well
/// inside it (`kaeri-testkit` holds the document to that), and no franchise has more seasons.
pub const MANY_PER_REQUEST: usize = 4 * 50;

/// What AniList allows a minute when it is not degraded: the most the bucket follows the
/// server's `X-RateLimit-Limit` up to.
pub const MAX_PER_MINUTE: u32 = 90;

/// How many times a request refused for going over the limit waits as the headers say and goes
/// again, before it says so.
const RATE_LIMIT_RETRIES: u32 = 3;

/// One upcoming broadcast, for the Calendar screen.
#[derive(Debug, Clone)]
pub struct AiringEntry {
    pub episode: u32,
    pub airing_at: i64,
    pub media: Media,
}

/// A page of results.
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub has_next: bool,
}

/// One page of broadcasts to ask for: see [`AniList::airing_pages`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiringPage {
    pub from: i64,
    pub to: i64,
    pub page: u32,
    pub per_page: u32,
    /// Newest first, for the past: see [`query::airing_schedule`].
    pub newest_first: bool,
}

/// What [`AniList::suggestions`] found.
#[derive(Debug, Clone, Default)]
pub struct Suggestions {
    /// The titles asked for, in the order asked.
    pub titles: Vec<Media>,
    /// Each title recommendations were asked from, with its name and the ids recommended off
    /// it, in the order asked.
    pub from: Vec<(AnilistId, String, Vec<AnilistId>)>,
    /// The ids rated highest off the `because` title, best first, when one was asked about.
    pub because: Option<Vec<AnilistId>>,
}

/// One row of the authenticated user's list.
///
/// Carries the full `Media` as well as the list state, so a library pull populates the Library
/// screen without a second round of lookups against a 30-per-minute budget.
#[derive(Debug, Clone)]
pub struct LibraryEntry {
    /// The `MediaList` entry id — distinct from the media id, and what deletion needs.
    pub entry_id: Option<u32>,
    pub media: Media,
    /// AniList's own status string: `CURRENT`, `PLANNING`, `COMPLETED`, …
    pub status: String,
    pub progress: u32,
    /// On the 10-point decimal scale. `0` means unscored on AniList, mapped to `None` here so
    /// "unscored" and "scored zero" stay distinguishable.
    pub score: Option<f32>,
    /// When AniList last changed this entry — the timestamp last-write-wins needs.
    pub updated_at: i64,
}

/// The AniList client.
#[derive(Clone)]
pub struct AniList {
    http: HttpClient,
    limiter: Arc<RateLimiter>,
    /// Behind a lock so a sign-in *during* a session takes effect without a restart.
    ///
    /// It was a plain `Option<String>` captured at construction, which is why signing in used to
    /// end with "restart to start syncing": the token reached the keychain and nothing that could
    /// use it ever saw it. A shared cell is the whole difference.
    token: Arc<std::sync::RwLock<Option<String>>>,
    /// Where GraphQL requests go: [`ENDPOINT`], unless a test pointed it elsewhere.
    endpoint: Arc<str>,
}

impl AniList {
    pub fn new(http: HttpClient, requests_per_minute: u32) -> Self {
        Self {
            http,
            limiter: Arc::new(RateLimiter::per_minute_up_to(
                requests_per_minute,
                MAX_PER_MINUTE,
            )),
            token: Arc::new(std::sync::RwLock::new(None)),
            endpoint: Arc::from(ENDPOINT),
        }
    }

    /// Send every request to `endpoint` instead of AniList: a stand-in server on loopback.
    ///
    /// For tests and probes only, which is why it is hidden from the docs and has no config
    /// key. A configurable endpoint would be one more way for a user's metadata to come from
    /// somewhere that is not AniList, and nothing but a test has a reason to want that. The
    /// request is otherwise unchanged, so it is the production path that gets exercised: the
    /// same rate limiter, and the same plain client, which speaks `http://` as readily as
    /// `https://`.
    #[doc(hidden)]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Arc::from(endpoint.into());
        self
    }

    /// Attach an OAuth token for library and mutation access.
    pub fn with_token(self, token: Option<String>) -> Self {
        self.set_token(token);
        self
    }

    /// Swap the token in, for a sign-in that happens while the app is running.
    pub fn set_token(&self, token: Option<String>) {
        *self.token.write().unwrap_or_else(|e| e.into_inner()) = token;
    }

    pub fn is_authenticated(&self) -> bool {
        self.token().is_some()
    }

    /// A snapshot of the token. Cloned out rather than held: the lock must not be alive across an
    /// `await`, and this is read on every request.
    fn token(&self) -> Option<String> {
        self.token.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Pace background work as `pacing` says, on every client that shares this one's budget.
    pub fn with_pacing(self, pacing: Pacing) -> Self {
        self.limiter.set_pacing(pacing);
        self
    }

    /// The app is launching: background work waits for the launch's screens. See
    /// [`RateLimiter::launching`].
    pub fn launching(&self) {
        self.limiter.launching();
    }

    /// The launch's screens were answered: background work begins shortly. See
    /// [`RateLimiter::launched`].
    pub fn launched(&self) {
        self.limiter.launched();
    }

    /// Whether the launch holds background work back now.
    pub fn in_launch(&self) -> bool {
        self.limiter.in_launch()
    }

    /// Wait until the launch no longer holds background work back.
    pub async fn launch_over(&self) {
        self.limiter.launch_over().await;
    }

    /// How long the next request would spend waiting on the rate limit.
    ///
    /// Surfaced so a slow screen can say *why* it is slow. AniList's measured budget is 30 requests
    /// a minute shared across every screen, so a burst of navigation genuinely can drain it — and
    /// "loading" forever with no reason given is indistinguishable from a hang.
    pub async fn rate_limit_wait(&self) -> Option<std::time::Duration> {
        self.limiter.would_wait().await
    }

    /// Execute a GraphQL operation.
    ///
    /// Uses the plain client, which follows redirects.
    async fn execute<T: DeserializeOwned>(
        &self,
        document: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        self.execute_as(document, variables, false).await
    }

    /// [`Self::execute`] for a list write, which answers a `5xx` as [`AniListError::Server`].
    async fn execute_write<T: DeserializeOwned>(
        &self,
        document: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        self.execute_as(document, variables, true).await
    }

    async fn execute_as<T: DeserializeOwned>(
        &self,
        document: &str,
        variables: serde_json::Value,
        write: bool,
    ) -> Result<T> {
        let mut refused = 0;
        let lane = LANE.try_with(Clone::clone).ok();
        loop {
            self.limiter
                .acquire_by(|| lane.as_ref().map_or(Priority::Screen, Lane::priority))
                .await;

            let mut request = self
                .http
                .plain()
                .post(&*self.endpoint)
                .header("content-type", "application/json")
                .header("accept", "application/json");
            if let Some(token) = &self.token() {
                request = request.header("authorization", format!("Bearer {token}"));
            }

            let response = request
                .json(&serde_json::json!({ "query": document, "variables": variables }))
                .send()
                .await
                .map_err(|e| AniListError::Network(e.to_string()))?;

            let status = response.status();

            // Feed the server's own accounting back into the limiter before anything else —
            // it is authoritative, and our local bucket is only an estimate.
            let headers = response.headers().clone();
            let get = |name: &str| -> Option<String> {
                headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)
            };
            let (remaining, reset) = parse_rate_headers(get, now_epoch());
            if let Some(limit) = parse_rate_limit(get) {
                self.limiter.observe_limit(limit).await;
            }
            self.limiter.observe(remaining, reset).await;

            if status.as_u16() == 429 {
                // A wait and another go, never a failure: the limiter holds every request until
                // the time the headers give, and the one that earned the refusal goes again.
                let wait = reset.unwrap_or(std::time::Duration::from_secs(60));
                self.limiter.back_off(wait).await;
                refused += 1;
                if refused > RATE_LIMIT_RETRIES {
                    return Err(AniListError::RateLimited { retry_in_secs: wait.as_secs() });
                }
                tracing::info!(
                    ?wait,
                    "rate limited; waiting as AniList says, then asking again"
                );
                continue;
            }

            if status.as_u16() == 401 {
                return Err(AniListError::Unauthenticated);
            }

            if write && status.is_server_error() {
                let body: serde_json::Value = response.json().await.unwrap_or_default();
                let message = body["errors"][0]["message"].as_str().unwrap_or_default();
                return Err(AniListError::Server {
                    status: status.as_u16(),
                    message: message.chars().take(160).collect(),
                });
            }

            let body: serde_json::Value =
                response.json().await.map_err(|e| AniListError::Decode(e.to_string()))?;

            // GraphQL reports failure inside a 200, so this check is the real error path.
            if let Some(errors) = body.get("errors").and_then(|e| e.as_array())
                && !errors.is_empty()
            {
                let message = errors
                    .iter()
                    .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(AniListError::Api(if message.is_empty() {
                    format!("request failed with HTTP {}", status.as_u16())
                } else {
                    message
                }));
            }

            let data = body
                .get("data")
                .ok_or_else(|| AniListError::Decode("response had no data field".into()))?;

            return serde_json::from_value(data.clone())
                .map_err(|e| AniListError::Decode(e.to_string()));
        }
    }

    /// Full-text search, with nothing else set.
    pub async fn search(&self, term: &str, page: u32, per_page: u32) -> Result<Page<Media>> {
        self.browse(term, &BrowseFilter::default(), page, per_page).await
    }

    /// One page of a search for `term` with `filter`, or a browse when `term` is empty.
    pub async fn browse(
        &self,
        term: &str,
        filter: &BrowseFilter,
        page: u32,
        per_page: u32,
    ) -> Result<Page<Media>> {
        let mut pages = self.browse_many(&[(term, filter)], page, per_page).await?;
        Ok(pages.pop().unwrap_or(Page { items: Vec::new(), has_next: false }))
    }

    /// Page `page` of several browses in one request, in the order asked: Search's rails.
    pub async fn browse_many(
        &self,
        browses: &[(&str, &BrowseFilter)],
        page: u32,
        per_page: u32,
    ) -> Result<Vec<Page<Media>>> {
        let browses: Vec<_> =
            browses.iter().map(|(term, filter)| (*term, *filter, page, per_page)).collect();
        self.browse_pages(&browses).await
    }

    /// Several browses in one request, each its own page of its own size, in the order asked:
    /// Home's shelves, which ask further down two of the rails than the others.
    pub async fn browse_pages(
        &self,
        browses: &[(&str, &BrowseFilter, u32, u32)],
    ) -> Result<Vec<Page<Media>>> {
        if browses.is_empty() {
            return Ok(Vec::new());
        }
        let arguments: Vec<_> = browses
            .iter()
            .map(|(term, filter, page, per_page)| filter.arguments(term, *page, *per_page))
            .collect();
        let (document, variables) = query::media_pages(&arguments);
        let data: serde_json::Value = self.execute(&document, variables).await?;
        (0..browses.len()).map(|i| decode_media_page(&data[format!("r{i}")])).collect()
    }

    /// The genres, tags and streaming sites the filters choose from.
    pub async fn facets(&self) -> Result<Facets> {
        let data: serde_json::Value =
            self.execute(query::FACETS, serde_json::json!({})).await?;
        let genres = data["GenreCollection"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| g.as_str().map(str::to_owned))
            .collect();
        let tags = data["MediaTagCollection"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| serde_json::from_value::<FacetTag>(t.clone()).ok())
            .collect();
        let mut streaming: Vec<String> = Vec::new();
        for site in data["ExternalLinkSourceCollection"].as_array().into_iter().flatten() {
            if site["isDisabled"].as_bool() == Some(true) {
                continue;
            }
            if let Some(name) = site["site"].as_str()
                && !streaming.iter().any(|s| s == name)
            {
                streaming.push(name.to_owned());
            }
        }
        Ok(Facets { genres, tags, streaming })
    }

    /// How many requests could go now without waiting on the limit.
    pub async fn budget_left(&self) -> f64 {
        self.limiter.available().await
    }

    /// Several titles in one request, for the `CONTINUE` rail.
    ///
    /// Returned in *the order asked for*, not AniList's. The rail is ordered by when you last
    /// watched each title, and that ordering is the whole point of it — letting the API's own
    /// order through would shuffle it into something meaningless.
    ///
    /// Past fifty, AniList's page, the titles go as pages of fifty under aliases in one request
    /// (see [`query::together`]), [`MANY_PER_REQUEST`] to a request.
    pub async fn media_many(&self, ids: &[AnilistId]) -> Result<Vec<Media>> {
        self.many_of(ids, query::by_ids).await
    }

    /// [`Self::media_many`] with each title's related titles named (see
    /// [`query::HEADER_RELATIONS`]): the season headers a title page asks ahead.
    pub async fn media_headers(&self, ids: &[AnilistId]) -> Result<Vec<Media>> {
        self.many_of(ids, query::headers_by_ids).await
    }

    /// `ids` asked with `document`, a page of fifty at most to a part, in the order asked.
    async fn many_of(&self, ids: &[AnilistId], document: fn() -> String) -> Result<Vec<Media>> {
        let many_part = |chunk: &[AnilistId]| many_part_of(chunk, document());
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        if let [chunk] = ids.chunks(50).collect::<Vec<_>>()[..] {
            let (document, variables) = many_part(chunk);
            let data: serde_json::Value = self.execute(&document, variables).await?;
            return decode_many(&data, chunk);
        }
        let mut found = Vec::with_capacity(ids.len());
        for request in ids.chunks(MANY_PER_REQUEST) {
            let parts: Vec<_> = request.chunks(50).map(many_part).collect();
            let answers = self.execute_together(&parts).await?;
            for (chunk, answer) in request.chunks(50).zip(answers) {
                found.extend(decode_many(&answer, chunk)?);
            }
        }
        Ok(found)
    }

    /// [`Self::media_many`] with what each title's episode table needs ([`query::TABLE_FIELDS`]),
    /// in the order asked. Each title says whether it came whole: `false` when its past
    /// broadcasts may run past the fifty this query brings, so only [`Self::media`] has the
    /// newest of them.
    pub async fn media_tables(&self, ids: &[AnilistId]) -> Result<Vec<(Media, bool)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
        let data: serde_json::Value = self
            .execute(
                &query::by_ids_with_tables(),
                serde_json::json!({ "ids": raw, "perPage": ids.len().min(50) }),
            )
            .await?;
        let mut found: Vec<(Media, bool)> = data["Page"]["media"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|wire| tabled(wire.clone()))
            .collect();
        let mut ordered = Vec::with_capacity(found.len());
        for id in ids {
            if let Some(index) = found.iter().position(|(m, _)| m.id == *id) {
                ordered.push(found.remove(index));
            }
        }
        Ok(ordered)
    }

    /// What people who liked each of `ids` (at most [`query::RECOMMENDED_FROM`]) rated highest:
    /// each title with its name and the ids recommended off it, best rated first. Titles
    /// AniList has nothing recommended for come back with none. See [`query::recommended_from`].
    pub async fn recommended_from(
        &self,
        ids: &[AnilistId],
    ) -> Result<Vec<(AnilistId, String, Vec<AnilistId>)>> {
        let ids = &ids[..ids.len().min(query::RECOMMENDED_FROM)];
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let (document, variables) = recommended_from_part(ids);
        let data: serde_json::Value = self.execute(&document, variables).await?;
        Ok(decode_recommended_from(&data, ids))
    }

    /// What one step of Up next asks, in one request (see [`query::together`]): the cards of
    /// `titles` (at most fifty), what people who liked each of `from` rated highest (as
    /// [`Self::recommended_from`]), and with `because` the ids of the `each` titles rated
    /// highest off that one (as [`query::recommended_ids`]). A part with nothing to ask is
    /// left out and answered empty.
    pub async fn suggestions(
        &self,
        titles: &[AnilistId],
        from: &[AnilistId],
        because: Option<(AnilistId, u32)>,
    ) -> Result<Suggestions> {
        let titles = &titles[..titles.len().min(50)];
        let from = &from[..from.len().min(query::RECOMMENDED_FROM)];
        let mut parts = Vec::new();
        if !titles.is_empty() {
            parts.push(many_part(titles));
        }
        if !from.is_empty() {
            parts.push(recommended_from_part(from));
        }
        if let Some((id, each)) = because {
            parts.push((
                query::recommended_ids(),
                serde_json::json!({ "ids": [id.get()], "perPage": 1, "each": each }),
            ));
        }
        let mut answers = self.execute_together(&parts).await?.into_iter();
        let mut next = || answers.next().unwrap_or(serde_json::Value::Null);
        let titles = if titles.is_empty() { Vec::new() } else { decode_many(&next(), titles)? };
        let from =
            if from.is_empty() { Vec::new() } else { decode_recommended_from(&next(), from) };
        let because = because.map(|_| {
            next()["Page"]["media"][0]["recommendations"]["nodes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|n| n["mediaRecommendation"]["id"].as_u64())
                .map(|n| AnilistId::new(n as u32))
                .collect()
        });
        Ok(Suggestions { titles, from, because })
    }

    /// Run `parts`, each a query of one root field, as one request (see [`query::together`]),
    /// and give each its answer as it would have come alone: its root field under its own name.
    /// One part goes as it is.
    async fn execute_together(
        &self,
        parts: &[(String, serde_json::Value)],
    ) -> Result<Vec<serde_json::Value>> {
        match parts {
            [] => Ok(Vec::new()),
            [(document, variables)] => {
                Ok(vec![self.execute(document, variables.clone()).await?])
            }
            _ => {
                let (document, variables) = query::together(parts);
                let data: serde_json::Value = self.execute(&document, variables).await?;
                Ok(parts
                    .iter()
                    .enumerate()
                    .map(|(i, (document, _))| {
                        let mut answer = serde_json::Map::new();
                        answer.insert(
                            query::root_of(document).to_owned(),
                            data[format!("q{i}")].clone(),
                        );
                        serde_json::Value::Object(answer)
                    })
                    .collect())
            }
        }
    }

    pub async fn media(&self, id: AnilistId) -> Result<Media> {
        // The people the viewer follows are asked for in the same request, and only when someone
        // is signed in: the filter needs an authenticated viewer.
        let data: serde_json::Value = self
            .execute(
                &query::by_id(self.is_authenticated()),
                serde_json::json!({ "id": id.get() }),
            )
            .await?;
        decode_detail(&data)
    }

    /// The cards of at most fifty titles, each with its relations two levels deep: one step of
    /// the season walk. See [`query::relations`].
    ///
    /// A title AniList has no anime for is not in the answer; one it answers in a shape this
    /// build cannot read is left out too, so the walk takes both for a link that leads nowhere.
    pub async fn relations(&self, ids: &[AnilistId]) -> Result<Vec<model::RelationNode>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = &ids[..ids.len().min(50)];
        let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
        let data: serde_json::Value = self
            .execute(
                &query::relations(),
                serde_json::json!({ "ids": raw, "perPage": ids.len() }),
            )
            .await?;
        Ok(data["Page"]["media"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|node| serde_json::from_value(node.clone()).ok())
            .collect())
    }

    /// Upcoming broadcasts in a time window — the Calendar screen.
    pub async fn airing_between(
        &self,
        from: i64,
        to: i64,
        page: u32,
        per_page: u32,
    ) -> Result<Page<AiringEntry>> {
        self.airing_between_sorted(from, to, page, per_page, false).await
    }

    /// The same window, with the sort direction chosen by the caller.
    ///
    /// `newest_first` matters for the past half of the calendar: see [`query::airing_schedule`].
    pub async fn airing_between_sorted(
        &self,
        from: i64,
        to: i64,
        page: u32,
        per_page: u32,
        newest_first: bool,
    ) -> Result<Page<AiringEntry>> {
        let mut pages =
            self.airing_pages(&[AiringPage { from, to, page, per_page, newest_first }]).await?;
        Ok(pages.pop().unwrap_or(Page { items: Vec::new(), has_next: false }))
    }

    /// Several pages of broadcasts, each as [`Self::airing_between_sorted`] asks for one, in
    /// one request and in the order asked (see [`query::together`]).
    pub async fn airing_pages(&self, pages: &[AiringPage]) -> Result<Vec<Page<AiringEntry>>> {
        let parts: Vec<_> = pages
            .iter()
            .map(|p| {
                let sort = if p.newest_first { "TIME_DESC" } else { "TIME" };
                (
                    query::airing_schedule(),
                    serde_json::json!({
                        "from": p.from, "to": p.to, "page": p.page.max(1),
                        "perPage": p.per_page, "sort": [sort]
                    }),
                )
            })
            .collect();
        let answers = self.execute_together(&parts).await?;
        Ok(answers.iter().map(|data| airing_page(&data["Page"])).collect())
    }

    /// What aired for `ids` after `from` and before `to`, oldest first: a page of at most
    /// fifty broadcasts, and whether there are more after it. Each carries its title's forms,
    /// cover and banner, and nothing else of the title.
    ///
    /// One request for every title, however many: see [`query::AIRED_SINCE`].
    pub async fn aired_since(
        &self,
        ids: &[AnilistId],
        from: i64,
        to: i64,
    ) -> Result<Page<AiringEntry>> {
        if ids.is_empty() {
            return Ok(Page { items: Vec::new(), has_next: false });
        }
        let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
        let data: serde_json::Value = self
            .execute(
                query::AIRED_SINCE,
                serde_json::json!({ "ids": raw, "from": from, "to": to, "perPage": 50 }),
            )
            .await?;
        Ok(airing_page(&data["Page"]))
    }

    /// When each of these titles last aired an episode.
    ///
    /// Batched deliberately — see [`query::LAST_AIRED`] for why this cannot be folded into the
    /// list query and why the weekly-cadence shortcut does not work. Callers should pass only
    /// the titles that are actually releasing; a finished show has nothing to report and would
    /// spend request budget crowding out one that does.
    pub async fn last_aired(&self, ids: &[AnilistId]) -> Result<Vec<LastAired>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
        // Sorted newest-first across every id, so a few rows per title is ample to find each
        // one's latest. Asking for one row per title would return only the newest overall.
        let per_page = (ids.len() * 2).min(50);
        let data: serde_json::Value = self
            .execute(query::LAST_AIRED, serde_json::json!({ "ids": raw, "perPage": per_page }))
            .await?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let mut out: Vec<LastAired> = data["Page"]["airingSchedules"]
            .as_array()
            .map(|arr| {
                arr.iter().filter_map(|r| serde_json::from_value(r.clone()).ok()).collect()
            })
            .unwrap_or_default();
        // The `notYetAired` filter is AniList's own bookkeeping and it lags around a
        // broadcast; a row whose air time is still ahead is not an answer to "what last
        // aired", and dropped *before* the dedup so it cannot shadow the row that is.
        out.retain(|r| r.airing_at <= now);
        // Keep the newest row per title and drop the rest.
        out.sort_by(|a, b| {
            a.media_id.get().cmp(&b.media_id.get()).then(b.airing_at.cmp(&a.airing_at))
        });
        out.dedup_by_key(|r| r.media_id);
        Ok(out)
    }

    /// The authenticated user's whole anime list.
    ///
    /// One request for every status at once — AniList returns the lists nested under
    /// `MediaListCollection`, and paginating per status would cost five round trips against a
    /// 30-per-minute budget.
    pub async fn library(&self, user_id: u32) -> Result<Vec<LibraryEntry>> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: serde_json::Value = self
            .execute(&query::user_library(), serde_json::json!({ "userId": user_id }))
            .await?;
        decode_library(&data)
    }

    /// The signed-in viewer and the list of `user_id`, in one request: for a viewer whose id
    /// is known from before, so the list need not wait for the viewer's answer. The list is the
    /// viewer's only when the viewer's id is `user_id`, which the caller checks.
    pub async fn viewer_and_library(
        &self,
        user_id: u32,
    ) -> Result<(Viewer, Vec<LibraryEntry>)> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let parts = [
            (query::VIEWER.to_owned(), serde_json::json!({})),
            (query::user_library(), serde_json::json!({ "userId": user_id })),
        ];
        let answers = self.execute_together(&parts).await?;
        let viewer = decode_viewer(&answers[0]["Viewer"])?;
        Ok((viewer, decode_library(&answers[1])?))
    }

    /// Write progress, status and/or score for one title.
    ///
    /// Idempotent by construction: `SaveMediaListEntry` is an upsert keyed on `mediaId`, so
    /// replaying a queued op after an ambiguous failure is safe. That is what lets the outbox
    /// retry without bookkeeping.
    ///
    /// `None` fields are omitted rather than sent as null, because sending null would *clear*
    /// the value — pushing progress would wipe the user's score.
    /// Remove a title from the user's list entirely.
    ///
    /// Takes the **MediaList entry** id from [`Self::save_entry`], not a media id — they are
    /// different numbers, and confusing them would delete an unrelated entry.
    pub async fn delete_entry(&self, entry_id: u32) -> Result<bool> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: serde_json::Value =
            self.execute(query::DELETE_ENTRY, serde_json::json!({ "id": entry_id })).await?;
        Ok(data["DeleteMediaListEntry"]["deleted"].as_bool().unwrap_or(false))
    }

    pub async fn save_entry(
        &self,
        id: AnilistId,
        progress: Option<u32>,
        status: Option<&str>,
        score: Option<f32>,
    ) -> Result<Option<u32>> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }

        let mut variables = serde_json::Map::new();
        variables.insert("mediaId".into(), id.get().into());
        if let Some(progress) = progress {
            variables.insert("progress".into(), progress.into());
        }
        if let Some(status) = status {
            variables.insert("status".into(), status.into());
        }
        if let Some(score) = score {
            // `scoreRaw` is always out of 100, whatever scale the user's profile displays.
            // The plain `score` argument is read in that profile scale, so sending our
            // 10-point value through it would write 8/100 for a 100-point user.
            variables.insert("scoreRaw".into(), ((score * 10.0).round() as u32).into());
        }

        let data: serde_json::Value =
            self.execute_write(query::SAVE_PROGRESS, serde_json::Value::Object(variables)).await?;
        // The entry id, so a caller that needs to remove the entry again can. `None` is not a
        // failure — the mutation succeeded either way.
        Ok(data["SaveMediaListEntry"]["id"].as_u64().map(|id| id as u32))
    }

    /// The authenticated user's id, for library queries.
    pub async fn viewer_id(&self) -> Result<u32> {
        self.viewer().await.map(|viewer| viewer.id)
    }

    /// The authenticated user: their id, name and the title language they read AniList in.
    pub async fn viewer(&self) -> Result<Viewer> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: serde_json::Value =
            self.execute(query::VIEWER, serde_json::json!({})).await?;
        decode_viewer(&data["Viewer"])
    }
}

/// [`query::by_ids`] for `ids`, at most fifty: the document and its variables.
fn many_part(ids: &[AnilistId]) -> (String, serde_json::Value) {
    many_part_of(ids, query::by_ids())
}

/// `document`, [`query::by_ids`] or its like, for `ids`, at most fifty.
fn many_part_of(ids: &[AnilistId], document: String) -> (String, serde_json::Value) {
    let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
    (document, serde_json::json!({ "ids": raw, "perPage": ids.len().min(50) }))
}

/// [`query::by_ids`]'s answer, in the order of `ids`.
fn decode_many(data: &serde_json::Value, ids: &[AnilistId]) -> Result<Vec<Media>> {
    let page: Page<Media> = decode_media_page(&data["Page"])?;
    let mut found = page.items;
    let mut ordered = Vec::with_capacity(found.len());
    for id in ids {
        if let Some(index) = found.iter().position(|m| m.id == *id) {
            ordered.push(found.remove(index));
        }
    }
    Ok(ordered)
}

/// [`query::recommended_from`] for `ids`: the document and its variables.
fn recommended_from_part(ids: &[AnilistId]) -> (String, serde_json::Value) {
    let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
    (query::recommended_from(), serde_json::json!({ "ids": raw, "perPage": ids.len() }))
}

/// [`query::recommended_from`]'s answer, in the order of `ids`.
fn decode_recommended_from(
    data: &serde_json::Value,
    ids: &[AnilistId],
) -> Vec<(AnilistId, String, Vec<AnilistId>)> {
    let id_of = |v: &serde_json::Value| v.as_u64().map(|n| AnilistId::new(n as u32));
    let mut found: Vec<(AnilistId, String, Vec<AnilistId>)> = data["Page"]["media"]
        .as_array()
        .map(|media| {
            media
                .iter()
                .filter_map(|m| {
                    let name = ["english", "romaji", "native"]
                        .iter()
                        .find_map(|k| m["title"][*k].as_str())
                        .unwrap_or_default()
                        .to_owned();
                    let recommended = m["recommendations"]["nodes"]
                        .as_array()
                        .map(|nodes| {
                            nodes
                                .iter()
                                .filter_map(|n| id_of(&n["mediaRecommendation"]["id"]))
                                .collect()
                        })
                        .unwrap_or_default();
                    Some((id_of(&m["id"])?, name, recommended))
                })
                .collect()
        })
        .unwrap_or_default();
    // In the order asked, as `media_many` does: the caller asked most recent first.
    found.sort_by_key(|(id, ..)| ids.iter().position(|asked| asked == id));
    found
}

/// The single-title answer: the title, with the broadcasts already out that were asked for
/// beside it put on it. Those are best-effort, so a shape this build does not expect leaves the
/// title without them rather than failing it.
fn decode_detail(data: &serde_json::Value) -> Result<Media> {
    let node = data.get("Media").filter(|m| !m.is_null()).ok_or(AniListError::NotFound)?;
    let mut media: Media = serde_json::from_value(node.clone())
        .map_err(|e| AniListError::Decode(e.to_string()))?;
    media.aired = data["aired"]["airingSchedules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| serde_json::from_value(row.clone()).ok())
        .collect();
    // Best-effort too, and nothing private: AniList withholds those, and this keeps one out if it
    // does not.
    media.following = data["following"]["mediaList"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| serde_json::from_value::<model::Follower>(row.clone()).ok())
        .filter(|entry| !entry.private && !entry.user.name.is_empty())
        .take(query::FOLLOWING_LIMIT as usize)
        .collect();
    Ok(media)
}

/// The `Viewer` node: an id it cannot do without, a name and a language it can.
fn decode_viewer(node: &serde_json::Value) -> Result<Viewer> {
    let id = node["id"]
        .as_u64()
        .map(|v| v as u32)
        .ok_or_else(|| AniListError::Decode("viewer response had no id".into()))?;
    Ok(Viewer {
        id,
        name: node["name"].as_str().map(str::to_owned),
        title_language: node["options"]["titleLanguage"]
            .as_str()
            .and_then(TitleLanguage::from_wire),
    })
}

/// A `MediaListCollection` answer, every list's rows in one.
fn decode_library(data: &serde_json::Value) -> Result<Vec<LibraryEntry>> {
    let lists = data["MediaListCollection"]["lists"]
        .as_array()
        .ok_or_else(|| AniListError::Decode("library had no lists".into()))?;
    let mut out = Vec::new();
    for list in lists {
        for entry in list["entries"].as_array().into_iter().flatten() {
            // One malformed entry must not blank a whole library. Someone with 800 titles
            // and one broken row should still see 799.
            match decode_library_entry(entry) {
                Some(decoded) => out.push(decoded),
                None => tracing::warn!("skipping undecodable library entry"),
            }
        }
    }
    Ok(out)
}

/// Decode one `MediaList` row.
///
/// Returns `None` rather than an error: the caller skips the row and keeps the rest of the
/// library, which matters when the alternative is showing an empty list.
fn decode_library_entry(entry: &serde_json::Value) -> Option<LibraryEntry> {
    let media = serde_json::from_value::<Media>(entry.get("media")?.clone()).ok()?;
    Some(LibraryEntry {
        entry_id: entry["id"].as_u64().map(|id| id as u32),
        media,
        status: entry["status"].as_str().unwrap_or("CURRENT").to_owned(),
        progress: entry["progress"].as_u64().unwrap_or(0) as u32,
        // AniList sends 0 for unscored. Keeping that as `Some(0.0)` would look like someone
        // deliberately rated a show zero, and would then be pushed back as such.
        score: entry["score"].as_f64().map(|s| s as f32).filter(|s| *s > 0.0),
        updated_at: entry["updatedAt"].as_i64().unwrap_or(0),
    })
}

/// One title of [`AniList::media_tables`]: its past broadcasts moved where [`Media`] keeps them,
/// newest first as [`AniList::media`] gives them, and whether they are all there. `None` for an
/// entry that does not read, which is passed over as a page's are.
fn tabled(mut wire: serde_json::Value) -> Option<(Media, bool)> {
    let past = wire.as_object_mut().and_then(|fields| fields.remove("past"));
    let mut media: Media = match serde_json::from_value(wire) {
        Ok(media) => media,
        Err(e) => {
            tracing::warn!(error = %e, "skipping undecodable media entry");
            return None;
        }
    };
    let mut aired: Vec<Broadcast> = past
        .and_then(|past| serde_json::from_value::<model::BroadcastConnection>(past).ok())
        .map(|past| past.nodes)
        .unwrap_or_default();
    let whole = aired.len() < query::TABLE_PAST_AT_MOST;
    aired.sort_by_key(|b| std::cmp::Reverse(b.episode));
    media.aired = aired;
    Some((media, whole))
}

fn decode_media_page(page: &serde_json::Value) -> Result<Page<Media>> {
    let items = page["media"]
        .as_array()
        .map(|arr| {
            // Skip individually-undecodable entries rather than failing the page: one
            // malformed title should not blank an entire screen.
            arr.iter()
                .filter_map(|m| match serde_json::from_value::<Media>(m.clone()) {
                    Ok(media) => Some(media),
                    Err(e) => {
                        tracing::warn!(error = %e, "skipping undecodable media entry");
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Page { items, has_next: has_next_page(page) })
}

/// A page of broadcasts, each with its title as far as the query asked for it. A row that does
/// not read is passed over rather than failing the page.
fn airing_page(page: &serde_json::Value) -> Page<AiringEntry> {
    let items = page["airingSchedules"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|entry| {
                    Some(AiringEntry {
                        episode: entry.get("episode")?.as_u64()? as u32,
                        airing_at: entry.get("airingAt")?.as_i64()?,
                        media: serde_json::from_value(entry.get("media")?.clone()).ok()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Page { items, has_next: has_next_page(page) }
}

fn has_next_page(page: &serde_json::Value) -> bool {
    page["pageInfo"]["hasNextPage"].as_bool().unwrap_or(false)
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_whose_past_may_run_on_is_not_whole() {
        let wire = |aired: u32| {
            let nodes: Vec<serde_json::Value> = (1..=aired)
                .map(|n| serde_json::json!({ "episode": n, "airingAt": 1_000 * i64::from(n) }))
                .collect();
            serde_json::json!({ "id": 7, "past": { "nodes": nodes } })
        };
        let (media, whole) = tabled(wire(12)).unwrap();
        assert!(whole);
        assert_eq!(media.aired.first().map(|b| b.episode), Some(12), "newest first");
        assert_eq!(media.aired.len(), 12);
        let (_, whole) = tabled(wire(50)).unwrap();
        assert!(!whole, "fifty may be the first fifty of more");
    }

    #[test]
    fn seasons_cycle_in_both_directions() {
        assert_eq!(Season::Winter.next(), Season::Spring);
        assert_eq!(Season::Fall.next(), Season::Winter, "must wrap");
        assert_eq!(Season::Winter.previous(), Season::Fall, "must wrap");
        for s in [Season::Winter, Season::Spring, Season::Summer, Season::Fall] {
            assert_eq!(s.next().previous(), s);
        }
    }

    #[test]
    fn months_map_to_the_right_season() {
        assert_eq!(Season::of_month(1), Season::Winter);
        assert_eq!(Season::of_month(4), Season::Spring);
        assert_eq!(Season::of_month(7), Season::Summer);
        assert_eq!(Season::of_month(10), Season::Fall);
        assert_eq!(Season::of_month(12), Season::Fall);
    }

    #[test]
    fn a_media_page_decodes_and_reports_pagination() {
        let payload = serde_json::json!({
            "pageInfo": {"hasNextPage": true},
            "media": [{"id": 154587, "title": {"romaji": "Hoshikuzu no Tabibito"}}]
        });
        let page = decode_media_page(&payload).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page.has_next);
    }

    #[test]
    fn one_malformed_entry_does_not_blank_the_page() {
        // A single bad title should cost that title, not the whole screen.
        let payload = serde_json::json!({
            "pageInfo": {"hasNextPage": false},
            "media": [
                {"id": 1, "title": {"romaji": "Good"}},
                {"title": {"romaji": "Missing its id"}},
                {"id": 3, "title": {"romaji": "Also good"}}
            ]
        });
        let page = decode_media_page(&payload).unwrap();
        assert_eq!(page.items.len(), 2);
        assert!(!page.has_next);
    }

    #[test]
    fn an_empty_page_is_not_an_error() {
        // A search with no matches is a valid answer, not a failure.
        let page = decode_media_page(&serde_json::json!({"media": []})).unwrap();
        assert!(page.items.is_empty());
        assert!(!page.has_next);
    }

    #[test]
    fn missing_page_info_defaults_to_no_further_pages() {
        assert!(!has_next_page(&serde_json::json!({})));
    }

    #[test]
    fn the_broadcasts_asked_for_beside_a_title_are_put_on_it() {
        let data = serde_json::json!({
            "Media": {"id": 1, "title": {"romaji": "One"}},
            "aired": {"airingSchedules": [
                {"episode": 9, "airingAt": 900},
                {"episode": "not a number"},
                {"episode": 8, "airingAt": 800}
            ]}
        });
        let media = decode_detail(&data).unwrap();
        let aired: Vec<_> = media.aired.iter().map(|b| (b.episode, b.airing_at)).collect();
        assert_eq!(aired, [(9, 900), (8, 800)], "a malformed row is passed over");

        // A title answered without them is a title all the same.
        let bare = decode_detail(&serde_json::json!({"Media": {"id": 1}})).unwrap();
        assert!(bare.aired.is_empty());
        assert!(matches!(
            decode_detail(&serde_json::json!({"Media": null})),
            Err(AniListError::NotFound)
        ));
    }

    #[test]
    fn the_viewer_carries_the_title_language_they_read_in() {
        let viewer = decode_viewer(&serde_json::json!({
            "id": 5150, "name": "fixture-viewer", "options": {"titleLanguage": "NATIVE_STYLISED"}
        }))
        .unwrap();
        assert_eq!(viewer.id, 5150);
        assert_eq!(viewer.name.as_deref(), Some("fixture-viewer"));
        assert_eq!(viewer.title_language, Some(TitleLanguage::Native));

        // Options AniList left out, or a language this build does not know, are no language.
        let bare = decode_viewer(&serde_json::json!({"id": 1})).unwrap();
        assert_eq!(bare.title_language, None);
        assert!(decode_viewer(&serde_json::json!({"name": "no id"})).is_err());
    }

    #[test]
    fn an_unauthenticated_client_refuses_viewer_queries_without_a_round_trip() {
        // Spending one of only 30 requests per minute to be told we are not logged in
        // would be a waste; we already know.
        let client = AniList::new(
            HttpClient::new(&kaeri_core::config::NetworkConfig::default()).unwrap(),
            30,
        );
        assert!(!client.is_authenticated());
        let err = futures_lite_block(client.viewer_id());
        assert!(matches!(err, Err(AniListError::Unauthenticated)));
    }

    #[test]
    fn requests_go_to_anilist_unless_a_test_says_otherwise() {
        let http = HttpClient::new(&kaeri_core::config::NetworkConfig::default()).unwrap();
        let client = AniList::new(http, 30);
        assert_eq!(&*client.endpoint, ENDPOINT);
        let pointed = client.with_endpoint("http://127.0.0.1:9/graphql");
        assert_eq!(&*pointed.endpoint, "http://127.0.0.1:9/graphql");
    }

    /// Minimal blocking executor so this test needs no async runtime feature.
    fn futures_lite_block<T>(fut: impl Future<Output = T>) -> T {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(fut)
    }
}
