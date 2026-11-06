//! A stand-in for AniList's GraphQL endpoint, on loopback.
//!
//! The documents in `kaeri_meta::anilist::query` carry no operation names, so a request
//! is told apart by the part of its document only that query has — its root field and the
//! argument that sets it apart — and answered from [`crate::fixtures`]. Answers are composed
//! per request rather than replayed whole: a search filters by its term, a page by its
//! `page` and `perPage`, a calendar window by its bounds. That is what lets one small
//! catalogue answer every screen without two screens contradicting each other.
//!
//! Beside it sits an empty Tenrai, under [`FakeAniList::tenrai_endpoint`]: every list it is
//! asked for is empty and every title is not found. It is there so the fallback can be switched
//! on, as the default config has it, without any request leaving the machine.
//!
//! Under [`FakeAniList::kitsu_base`] sits a fake Kitsu ([`kitsu`]), with one made-up account,
//! so a fixture build's Kitsu sign-in has somewhere to go that is not Kitsu.
//!
//! Asked to, it also serves the fixture titles' art ([`FakeAniList::serve_art`]), and points
//! every image URL it answers with at itself.
//!
//! HTTP is spoken by hand, as the loopback servers in the app's own tests do: one request per
//! connection, answered with `connection: close` so the client never reuses a socket this side
//! has finished with. That is all reqwest needs, and it keeps a server framework out of the
//! dependency tree.

use std::{
    collections::{HashMap, VecDeque},
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, MutexGuard},
};

use kaeri_core::ids::AnilistId;
use kaeri_meta::{AniList, Meta, Tenrai};
use kaeri_net::HttpClient;
use kaeri_store::Store;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, tcp::OwnedReadHalf},
    task::JoinHandle,
};

use crate::fixtures::{self, FIXTURE_NOW};

mod books;
pub mod kitsu;
mod threads;
pub use threads::comment;

/// A request body larger than this is not one the app sends, and is not read.
const MAX_BODY: usize = 1 << 20;

/// AniList's own cap on `perPage`.
const MAX_PER_PAGE: usize = 50;

/// Which of the app's queries a request was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    /// `query::media_pages` with one page: a text search, or a browse with no term.
    Search,
    /// `query::media_pages` with several pages: Search's rails, in one request.
    Browse,
    /// `query::FACETS`: the genres, tags and streaming sites the filters choose from.
    Facets,
    /// `query::by_id`: one title, with the detail-only fields.
    Detail,
    /// `query::by_ids`: several titles at once, for the Continue rail.
    Many,
    /// `query::by_ids_with_tables`: the covers a screen shows, with what their episode tables
    /// need, for the Mac's hover cards.
    Tables,
    /// `query::relations`: several titles' relations, two levels deep, for the season walk.
    Relations,
    /// `query::recommended_from`: what people who liked each title rated highest, by id.
    Recommended,
    /// `query::media_pages` with one page of a season and no term: Seasonal.
    Seasonal,
    /// `query::airing_schedule`: broadcasts in a window, for the Calendar.
    Schedule,
    /// `query::LAST_AIRED`.
    LastAired,
    /// `query::AIRED_SINCE`: what aired for the titles followed, for the new-episode check.
    AiredSince,
    /// `query::user_library`.
    Library,
    /// `query::VIEWER`.
    Viewer,
    /// `query::SAVE_PROGRESS`.
    SaveEntry,
    /// `query::DELETE_ENTRY`.
    DeleteEntry,
    /// `query::EPISODE_THREADS`: the forum threads that may be an episode's discussion.
    EpisodeThreads,
    /// `query::thread_comments`: one page of a thread's comments.
    ThreadComments,
    /// `query::save_thread_comment`: a comment posted, or edited.
    SaveThreadComment,
    /// `query::DELETE_THREAD_COMMENT`.
    DeleteThreadComment,
    /// `query::TOGGLE_COMMENT_LIKE`.
    ToggleCommentLike,
    /// `query::media_total`: how many titles a browse matches, one title of them asked for.
    Total,
    /// `query::trend_week`: the last week's highest daily trending rows, with their cards.
    TrendWeek,
    /// `query::trend_month_candidates`: four pages of the highest daily rows of a month.
    TrendMonthCandidates,
    /// `query::trend_month_series`: the month's candidates with each day's figure.
    TrendSeries,
    /// A request to the empty Tenrai.
    Tenrai,
    /// A request to the fake Kitsu: see [`kitsu`].
    Kitsu,
    /// Any request for AniList's `MANGA` type: a book page, books by id or a series page.
    Books,
    /// Anything else. Answered with an error, and recorded, so a test can catch a query the
    /// fake was never taught rather than reading its failure as the app's.
    Unknown,
}

impl Operation {
    /// Tell a GraphQL document apart by what only it contains.
    ///
    /// Mutations and the authenticated queries are checked first: their documents nest the
    /// media fields too, so the media checks would claim them. The single title comes before
    /// the schedules, because it asks for its own broadcasts beside the title.
    pub fn of(document: &str) -> Self {
        let document = document.split_whitespace().collect::<Vec<_>>().join(" ");
        let has = |needle: &str| document.contains(needle);
        if has("type: MANGA") {
            Self::Books
        } else if has("DeleteMediaListEntry") {
            Self::DeleteEntry
        } else if has("DeleteThreadComment") {
            Self::DeleteThreadComment
        } else if has("SaveThreadComment") {
            Self::SaveThreadComment
        } else if has("ToggleLikeV2") {
            Self::ToggleCommentLike
        } else if has("threadComments(") {
            Self::ThreadComments
        } else if has("threads(mediaCategoryId") {
            Self::EpisodeThreads
        } else if has("SaveMediaListEntry") {
            Self::SaveEntry
        } else if has("MediaListCollection") {
            Self::Library
        } else if has("Viewer") {
            Self::Viewer
        } else if has("Media(id:") {
            Self::Detail
        } else if has("airingSchedules(mediaId_in: $ids, airingAt_greater") {
            Self::AiredSince
        } else if has("airingSchedules(mediaId_in") {
            Self::LastAired
        } else if has("airingSchedules(") {
            Self::Schedule
        } else if has("pageInfo { total }") {
            Self::Total
        } else if has("mediaTrends(") && has("trending media {") {
            Self::TrendWeek
        } else if has("mediaTrends(") {
            Self::TrendMonthCandidates
        } else if has("trends(date_greater") {
            Self::TrendSeries
        } else if has("media(id_in:") && has("recommendations(") {
            Self::Recommended
        } else if has("media(id_in:") && has("relations {") && !has("prequels:") {
            Self::Relations
        } else if has("media(id_in:") && has("streamingEpisodes") {
            Self::Tables
        } else if has("media(id_in:") {
            Self::Many
        } else if has("GenreCollection") {
            Self::Facets
        } else if has("r1: Page(") {
            Self::Browse
        } else if has("r0: Page(") && has("season: $r0_season") && !has("search: $r0_search") {
            Self::Seasonal
        } else if has("r0: Page(") {
            Self::Search
        } else {
            Self::Unknown
        }
    }
}

/// One request the fake received.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub operation: Operation,
    /// `/graphql`, or the Tenrai path with its query string.
    pub path: String,
    /// The GraphQL document; empty for a Tenrai request.
    pub query: String,
    /// The GraphQL variables; `Null` for a Tenrai request.
    pub variables: Value,
    /// The `authorization` header, when one was sent.
    pub authorization: Option<String>,
    /// When it came.
    pub at: std::time::Instant,
    /// Whether the fake refused it for going over its limit: see
    /// [`FakeAniList::limit_per`].
    pub refused: bool,
}

/// A failure the fake answers with instead of the fixtures, for the failure paths.
///
/// Applies to GraphQL requests only: the empty Tenrai keeps answering, so a test of the
/// fallback ladder can fail AniList and watch the rungs below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// `429` with `retry-after` and `x-ratelimit-remaining: 0`, as AniList answers a spent
    /// budget. The client holds every later request for that many seconds, so `0` is the
    /// value for a test that means to go on asking.
    RateLimited { retry_after_secs: u64 },
    /// This HTTP status, with an `errors` body naming it.
    Status(u16),
    /// `200` with an `errors` array: GraphQL's own failure shape, and AniList's usual one.
    GraphQl(String),
    /// Read the request and hang up without a word: what the client reads as the network.
    Disconnect,
}

/// AniList's GraphQL endpoint on `127.0.0.1`, answering from the fixtures.
///
/// Stops accepting when dropped.
pub struct FakeAniList {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    accept: JoinHandle<()>,
}

struct State {
    requests: Vec<Request>,
    /// The most GraphQL requests a rolling window answers, and the window: see
    /// [`FakeAniList::limit_per`].
    limit: Option<(u32, std::time::Duration)>,
    /// When each GraphQL request the limit counted came, oldest first.
    counted: VecDeque<std::time::Instant>,
    next: VecDeque<Failure>,
    always: Option<Failure>,
    /// Failures for one operation each, until healed: see [`FakeAniList::fail_operation`].
    failing: HashMap<Operation, Failure>,
    now: i64,
    /// How far the clock has moved on past `now` without the broadcasts moving with it: see
    /// [`FakeAniList::advance`].
    ahead: i64,
    /// Titles answered in other forms than the fixtures', by id: see [`FakeAniList::set_title`].
    titles: HashMap<u64, Value>,
    /// The viewer's own title language, in place of the fixture's.
    viewer_language: Option<String>,
    /// The viewer's id, in place of the fixture's: see [`FakeAniList::answer_as_viewer`].
    viewer_id: Option<u64>,
    /// Titles beside the fixtures, by id, which a test made up: see [`FakeAniList::add_titles`].
    extra: HashMap<u64, Value>,
    /// Titles that are also in the search and browse pool: see [`FakeAniList::add_catalogue`].
    catalogue: Vec<AnilistId>,
    /// Broadcasts beside the fixtures' schedule: see [`FakeAniList::add_broadcasts`].
    broadcasts: Vec<Value>,
    /// Titles of the season the real clock is in, beside [`fixtures::SEASONAL`]: see
    /// [`FakeAniList::add_season`].
    season: Vec<AnilistId>,
    /// List entries beside the fixture library's: see [`FakeAniList::add_entries`].
    entries: Vec<Value>,
    /// How long each operation's answers are held back: see [`FakeAniList::answer_slowly`].
    slow: HashMap<Operation, std::time::Duration>,
    /// What the next answers say of the limit, as `(limit, remaining)`, with no reset: see
    /// [`FakeAniList::say_left`].
    said: VecDeque<(u32, u32)>,
    /// Where image URLs point once the fake serves art: its own `/art/`.
    art: Option<String>,
    /// How the art is served when it is served badly: see [`FakeAniList::trouble_art`].
    art_trouble: ArtTrouble,
    /// How often each image has been asked for, and in which order they first were.
    art_asked: HashMap<String, (usize, u32)>,
    /// The entries of the people the viewer follows, by title: see [`FakeAniList::set_following`].
    following: HashMap<u64, Vec<Value>>,
    /// Forum threads a test made up: see [`FakeAniList::add_thread`].
    threads: Vec<threads::FakeThread>,
    /// How many comments have been posted to them, for the new comments' ids.
    posted: u64,
    /// The books: see [`FakeAniList::add_books`].
    books: Vec<Value>,
}

/// Art served badly, as a busy CDN serves it.
#[derive(Debug, Clone, Copy, Default)]
struct ArtTrouble {
    /// How many times every second image is answered 503 before it is served.
    fail_first: u32,
    /// How long every image's answer is held back.
    delay: std::time::Duration,
}

impl FakeAniList {
    /// Bind a free port on loopback and start answering.
    pub async fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("the fake AniList binds a loopback port");
        let addr = listener.local_addr().expect("a bound listener has an address");
        let state = Arc::new(Mutex::new(State {
            requests: Vec::new(),
            limit: None,
            counted: VecDeque::new(),
            next: VecDeque::new(),
            always: None,
            now: FIXTURE_NOW,
            ahead: 0,
            titles: HashMap::new(),
            viewer_language: None,
            viewer_id: None,
            extra: HashMap::new(),
            catalogue: Vec::new(),
            broadcasts: Vec::new(),
            season: Vec::new(),
            entries: Vec::new(),
            slow: HashMap::new(),
            said: VecDeque::new(),
            failing: HashMap::new(),
            art: None,
            art_trouble: ArtTrouble::default(),
            art_asked: HashMap::new(),
            following: HashMap::new(),
            threads: Vec::new(),
            posted: 0,
            books: fixtures::books(),
        }));
        let accept = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                while let Ok((socket, _)) = listener.accept().await {
                    tokio::spawn(serve(socket, Arc::clone(&state)));
                }
            }
        });
        Self { addr, state, accept }
    }

    /// The GraphQL endpoint, for [`AniList::with_endpoint`].
    pub fn endpoint(&self) -> String {
        format!("http://{}/graphql", self.addr)
    }

    /// The empty Tenrai's base, for [`Tenrai::with_endpoint`].
    pub fn tenrai_endpoint(&self) -> String {
        format!("http://{}/tenrai/v1", self.addr)
    }

    /// An AniList client that talks to this server, with [`crate::TEST_RATE_LIMIT`] and no
    /// pacing of queued work, so a test's background requests go as soon as the limit allows.
    pub fn anilist(&self, http: &HttpClient) -> AniList {
        self.anilist_paced(http, kaeri_meta::Pacing::UNPACED)
    }

    /// [`Self::anilist`], with queued work paced by `pacing`, as the app paces it.
    pub fn anilist_paced(&self, http: &HttpClient, pacing: kaeri_meta::Pacing) -> AniList {
        AniList::new(http.clone(), crate::TEST_RATE_LIMIT)
            .with_endpoint(self.endpoint())
            .with_pacing(pacing)
    }

    /// Where the fake Kitsu answers, for [`kaeri_track`]'s `kitsu::Site::at`: its token
    /// endpoint and API under this base, as Kitsu lays them out.
    pub fn kitsu_base(&self) -> String {
        format!("http://{}/kitsu", self.addr)
    }

    /// A Tenrai client that talks to the empty one here.
    pub fn tenrai(&self, http: &HttpClient, store: &Store) -> Tenrai {
        Tenrai::new(http.clone(), store.clone()).with_endpoint(self.tenrai_endpoint())
    }

    /// A [`Meta`] over this server and `store`, with the fallback off.
    pub fn meta(&self, store: &Store) -> Meta {
        Meta::new(self.anilist(&crate::http()), None, store.clone())
    }

    /// A [`Meta`] over this server and `store`, with the fallback on and empty.
    pub fn meta_with_tenrai(&self, store: &Store) -> Meta {
        let http = crate::http();
        Meta::new(self.anilist(&http), Some(self.tenrai(&http, store)), store.clone())
    }

    /// Every request so far, in arrival order.
    pub fn requests(&self) -> Vec<Request> {
        lock(&self.state).requests.clone()
    }

    /// The requests for one operation, in arrival order.
    pub fn requests_for(&self, operation: Operation) -> Vec<Request> {
        lock(&self.state)
            .requests
            .iter()
            .filter(|r| r.operation == operation)
            .cloned()
            .collect()
    }

    pub fn clear_requests(&self) {
        lock(&self.state).requests.clear();
    }

    /// Answer the next GraphQL request with `failure`. Calls queue, one request each.
    pub fn fail_next(&self, failure: Failure) {
        lock(&self.state).next.push_back(failure);
    }

    /// Answer every GraphQL request with `failure` until [`Self::heal`]. Queued
    /// [`Self::fail_next`] failures still come first.
    pub fn fail_always(&self, failure: Failure) {
        lock(&self.state).always = Some(failure);
    }

    /// Answer every request for `operation` with `failure` until [`Self::heal`], and every
    /// other as ever: an AniList that fails one kind of query, such as Home's discovery while its
    /// queue's titles still load. [`Self::fail_next`] and [`Self::fail_always`] come first.
    pub fn fail_operation(&self, operation: Operation, failure: Failure) {
        lock(&self.state).failing.insert(operation, failure);
    }

    /// Drop every queued and standing failure.
    pub fn heal(&self) {
        let mut state = lock(&self.state);
        state.next.clear();
        state.always = None;
        state.failing.clear();
    }

    /// Serve the fixture titles' art ([`crate::art`]) under `/art/`, and point every image URL
    /// in an answer there instead of at [`crate::fixtures::ART_HOST`], which nothing resolves:
    /// for a front end that shows covers, banners and stills. Off until asked, so the image
    /// URLs a test sees are the fixtures' own.
    pub fn serve_art(&self) {
        lock(&self.state).art = Some(self.art_base());
    }

    /// Serve the art badly from now on, as a busy CDN would: every second image asked for (the
    /// first, the third, and so on, in the order they are first asked for) is answered
    /// `503 Service Unavailable` its first `fail_first` times, and every answer is held back by
    /// `delay`. For a front end's test that art on screen still arrives.
    pub fn trouble_art(&self, fail_first: u32, delay: std::time::Duration) {
        let mut state = lock(&self.state);
        state.art_trouble = ArtTrouble { fail_first, delay };
        state.art_asked.clear();
    }

    /// Where [`Self::serve_art`] serves the art from, standing in for
    /// [`crate::fixtures::ART_HOST`]: for a source's episode stills to point at too.
    pub fn art_base(&self) -> String {
        format!("http://{}/art/", self.addr)
    }

    /// Answer as if it were `now`, rather than [`FIXTURE_NOW`].
    ///
    /// Every broadcast moves by the same amount, so the schedule keeps its shape around the new
    /// clock: the next episode is still 2d 3h 15m away. For a test that runs against the real
    /// clock, which the calendar's window and the countdowns are taken from.
    pub fn set_now(&self, now: i64) {
        let mut state = lock(&self.state);
        state.now = now;
        state.ahead = 0;
    }

    /// Let `secs` pass: the clock moves on and the broadcasts stay where they are, so an
    /// episode due in an hour has aired once an hour has passed. For a test of what airs while
    /// the app is away, with the engine's own clock moved on as far.
    pub fn advance(&self, secs: i64) {
        lock(&self.state).ahead += secs;
    }

    /// Answer `id`'s title in these forms from now on, wherever the fake names it — a list, its
    /// page, a relation's card — for a test of a title that is missing one.
    pub fn set_title(
        &self,
        id: AnilistId,
        romaji: Option<&str>,
        english: Option<&str>,
        native: Option<&str>,
    ) {
        let title = json!({ "romaji": romaji, "english": english, "native": native });
        lock(&self.state).titles.insert(u64::from(id.get()), title);
    }

    /// The signed-in viewer reads AniList in `language` from now on: `ROMAJI`, `ENGLISH`,
    /// `NATIVE` or a stylised form, as AniList names them.
    pub fn set_viewer_title_language(&self, language: &str) {
        lock(&self.state).viewer_language = Some(language.to_owned());
    }

    /// The token is someone else's from now on: the viewer AniList answers with has `id`, as
    /// after a sign-in to another account elsewhere.
    pub fn answer_as_viewer(&self, id: u64) {
        lock(&self.state).viewer_id = Some(id);
    }

    /// The people the signed-in viewer follows who have `id` on a list, as `mediaList` entries in
    /// AniList's shape (`status`, `score`, `progress`, `private`, `user { name avatar { medium } }`),
    /// newest first. Answered only to a request that asks for them, with a token: see
    /// [`following_entry`].
    pub fn set_following(&self, id: AnilistId, entries: impl IntoIterator<Item = Value>) {
        lock(&self.state).following.insert(u64::from(id.get()), entries.into_iter().collect());
    }

    /// Answer at most `per_minute` GraphQL requests in any minute from now on, as AniList does,
    /// saying so in the `x-ratelimit-limit` and `x-ratelimit-remaining` headers, and refuse the
    /// rest with a `429` and `retry-after` until the oldest of the minute is a minute old.
    pub fn limit_per_minute(&self, per_minute: u32) {
        self.limit_per(per_minute, std::time::Duration::from_secs(60));
    }

    /// [`Self::limit_per_minute`] over a window of `window` in place of a minute, so a test of
    /// what a refusal sets off waits seconds rather than a minute for the window to pass.
    pub fn limit_per(&self, most: u32, window: std::time::Duration) {
        lock(&self.state).limit = Some((most.max(1), window));
    }

    /// Answer the next GraphQL requests as usual, each saying the next of `remaining` is left
    /// of `limit` a minute, with no reset, as AniList's answers do. Its nodes count apart, so
    /// one may say nothing is left while the window still has room: for a test of how the
    /// client reads what it is told.
    pub fn say_left(&self, limit: u32, remaining: impl IntoIterator<Item = u32>) {
        lock(&self.state).said.extend(remaining.into_iter().map(|left| (limit, left)));
    }

    /// Hold every answer to `operation` back by `wait` from now on, as a slow AniList would: for
    /// a test of what the app shows while it waits.
    pub fn answer_slowly(&self, operation: Operation, wait: std::time::Duration) {
        lock(&self.state).slow.insert(operation, wait);
    }

    /// Answer these titles too, each in the shape of a fixture under `fixtures/anilist/media/`,
    /// wherever a title page or the season walk asks after one: titles a test made up beside
    /// the fixtures, such as [`fixtures::franchise`]. No list, search or schedule has them.
    pub fn add_titles(&self, titles: impl IntoIterator<Item = Value>) {
        let mut state = lock(&self.state);
        for title in titles {
            let id = title["id"].as_u64().expect("a title has an id");
            state.extra.insert(id, title);
        }
    }

    /// Books beside [`fixtures::books`], each in that shape, or in place of one with its id:
    /// every book page, id list and series page has them.
    pub fn add_books(&self, books: impl IntoIterator<Item = Value>) {
        let mut state = lock(&self.state);
        for book in books {
            state.books.retain(|b| b["id"] != book["id"]);
            state.books.push(book);
        }
    }

    /// Broadcasts beside the fixtures' schedule, as `(title, episode, airing_at)` with the time
    /// at [`FIXTURE_NOW`]'s clock: they move with [`Self::set_now`] as the fixtures do. For a
    /// calendar with more than a page of rows in it, or the airing titles of a world added with
    /// [`Self::add_season`]. The title must be one the fake has: a fixture or an added title.
    pub fn add_broadcasts(&self, rows: impl IntoIterator<Item = (AnilistId, u32, i64)>) {
        let mut state = lock(&self.state);
        for (id, episode, airing_at) in rows {
            state.broadcasts.push(
                json!({ "mediaId": id.get(), "episode": episode, "airingAt": airing_at }),
            );
        }
    }

    /// [`Self::add_titles`], and the titles join the pool a search or a browse filters and
    /// sorts, beside the fixtures, so that a Search with no term finds them too. For a grid with
    /// more than the six titles in it, such as [`fixtures::catalogue_titles`].
    ///
    /// In a browse the new titles are mixed in with the fixtures, one after each, not put at the
    /// end, so a sort that ties (the fixtures carry no popularity) shows both kinds together. A
    /// season asked with no term still answers with [`fixtures::SEASONAL`] alone.
    pub fn add_catalogue(&self, titles: impl IntoIterator<Item = Value>) {
        let titles: Vec<Value> = titles.into_iter().collect();
        let ids: Vec<AnilistId> = titles
            .iter()
            .map(|t| AnilistId::new(t["id"].as_u64().expect("a title has an id") as u32))
            .collect();
        self.add_titles(titles);
        let mut state = lock(&self.state);
        for id in ids {
            if !state.catalogue.contains(&id) {
                state.catalogue.push(id);
            }
        }
    }

    /// [`Self::add_catalogue`], and the titles are this season's too: a browse of the season the
    /// real clock is in, with no term, answers with them beside [`fixtures::SEASONAL`], each
    /// filter and sort applied as AniList would. Each should say it is of that season, as
    /// [`crate::home_world::HomeWorld`]'s are, so a browse that names the season finds them
    /// either way.
    pub fn add_season(&self, titles: impl IntoIterator<Item = Value>) {
        let titles: Vec<Value> = titles.into_iter().collect();
        let ids: Vec<AnilistId> = titles
            .iter()
            .map(|t| AnilistId::new(t["id"].as_u64().expect("a title has an id") as u32))
            .collect();
        self.add_catalogue(titles);
        let mut state = lock(&self.state);
        for id in ids {
            if !state.season.contains(&id) {
                state.season.push(id);
            }
        }
    }

    /// Entries on the viewer's list beside the fixture library's, each in its shape (`id`,
    /// `mediaId`, `status`, `progress`, `score`, `updatedAt`), for titles the fake has. The
    /// library answers with them, and a browse's `onList` counts them.
    pub fn add_entries(&self, entries: impl IntoIterator<Item = Value>) {
        lock(&self.state).entries.extend(entries);
    }
}

impl Drop for FakeAniList {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

/// What AniList refuses a query for costing more than.
pub const COMPLEXITY_LIMIT: u64 = 500;

/// What `document` costs as AniList counts it, as far as the fake can tell: one for every field,
/// and a connection with a `perPage` argument (a literal or a variable of `variables`)
/// multiplies what is selected under it by that many. A root `Page` does not. An estimate of the rule AniList documents, not a replay of
/// it: the real service is the judge, and a live probe is the way to measure it.
pub fn complexity(document: &str, variables: &Value) -> u64 {
    let mut tokens: Vec<String> = Vec::new();
    let mut word = String::new();
    for c in document.chars() {
        if c.is_alphanumeric() || c == '_' || c == '$' {
            word.push(c);
        } else {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            if "{}():,[]!".contains(c) {
                tokens.push(c.to_string());
            }
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    let Some(start) = tokens.iter().position(|t| t == "{") else { return 0 };
    let mut at = start + 1;
    selection(&tokens, &mut at, variables)
}

/// The cost of the fields up to the `}` that closes the selection `at` is inside.
fn selection(tokens: &[String], at: &mut usize, variables: &Value) -> u64 {
    let mut total = 0;
    while *at < tokens.len() {
        match tokens[*at].as_str() {
            "}" => {
                *at += 1;
                return total;
            }
            "," | ":" => *at += 1,
            _ => {
                // A field, or an alias before the `:` that names it.
                if tokens.get(*at + 1).is_some_and(|t| t == ":") {
                    *at += 2;
                }
                // A `Page` is not a connection: its titles are counted once, however many it
                // holds. Fifty whole titles in one page answer, as do five pages of twenty,
                // while sixteen whole titles under one title's recommendations were refused.
                let page = tokens.get(*at).is_some_and(|t| t == "Page");
                *at += 1;
                let mut times = 1;
                if tokens.get(*at).is_some_and(|t| t == "(") {
                    let mut depth = 0;
                    while *at < tokens.len() {
                        match tokens[*at].as_str() {
                            "(" => depth += 1,
                            ")" => depth -= 1,
                            "perPage" => {
                                let value = tokens.get(*at + 2).map_or("", String::as_str);
                                times = value.parse().unwrap_or_else(|_| {
                                    variables[value.trim_start_matches('$')]
                                        .as_u64()
                                        .unwrap_or(1)
                                });
                            }
                            _ => {}
                        }
                        *at += 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                let mut children = 0;
                if tokens.get(*at).is_some_and(|t| t == "{") {
                    *at += 1;
                    children = selection(tokens, at, variables);
                }
                total += 1 + children * if page { 1 } else { times };
            }
        }
    }
    total
}

/// The roots of a document that asks several queries at once, as `query::together` writes one:
/// each root under an alias `q0`, `q1` and on, its variables named after it (`$q0_ids`). Each
/// comes back as its alias, a document of that root alone with its variables named as the query
/// names them, and those variables. `None` for any other document, the aliased pages of a search
/// included.
fn together(document: &str, variables: &Value) -> Option<Vec<(String, String, Value)>> {
    let open = document.find('{')?;
    let head = &document[..open];
    let declared: Vec<(String, String)> = match (head.find('('), head.rfind(')')) {
        (Some(from), Some(to)) if from < to => head[from + 1..to]
            .split(',')
            .filter_map(|d| {
                let (name, kind) = d.split_once(':')?;
                Some((name.trim().trim_start_matches('$').to_owned(), kind.trim().to_owned()))
            })
            .collect(),
        _ => Vec::new(),
    };
    let body: Vec<char> = document[open + 1..document.rfind('}')?].chars().collect();
    let mut parts = Vec::new();
    let mut at = 0;
    let skip = |at: &mut usize| {
        while body.get(*at).is_some_and(|c| c.is_whitespace() || *c == ',') {
            *at += 1;
        }
    };
    let word = |at: &mut usize| {
        let from = *at;
        while body.get(*at).is_some_and(|c| c.is_alphanumeric() || *c == '_') {
            *at += 1;
        }
        body[from..*at].iter().collect::<String>()
    };
    // Past the bracket that closes the one at `at`.
    let close = |at: &mut usize, open: char, shut: char| {
        let mut depth = 0;
        while let Some(c) = body.get(*at) {
            *at += 1;
            if *c == open {
                depth += 1;
            } else if *c == shut {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
        }
    };
    loop {
        skip(&mut at);
        if at >= body.len() {
            break;
        }
        let alias = word(&mut at);
        if !alias
            .strip_prefix('q')
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        {
            return None;
        }
        skip(&mut at);
        if body.get(at) != Some(&':') {
            return None;
        }
        at += 1;
        skip(&mut at);
        let from = at;
        word(&mut at);
        skip(&mut at);
        if body.get(at) == Some(&'(') {
            close(&mut at, '(', ')');
            skip(&mut at);
        }
        if body.get(at) != Some(&'{') {
            return None;
        }
        close(&mut at, '{', '}');
        let prefix = format!("{alias}_");
        let root: String =
            body[from..at].iter().collect::<String>().replace(&format!("${prefix}"), "$");
        let mut vars = serde_json::Map::new();
        let mut declarations = Vec::new();
        for (name, kind) in &declared {
            if let Some(own) = name.strip_prefix(&prefix) {
                declarations.push(format!("${own}: {kind}"));
                if let Some(value) = variables.get(name) {
                    vars.insert(own.to_owned(), value.clone());
                }
            }
        }
        let document = format!("query ({}) {{ {root} }}", declarations.join(", "));
        parts.push((alias, document, Value::Object(vars)));
    }
    (parts.len() > 1).then_some(parts)
}

/// The viewer as a test set it: its title language and its id, where one was given.
fn as_viewer(viewer: &mut Value, language: Option<&str>, id: Option<u64>) {
    if let Some(language) = language {
        viewer["options"] = json!({ "titleLanguage": language });
    }
    if let Some(id) = id {
        viewer["id"] = json!(id);
    }
}

/// The root field a document of one root answers under.
fn root_field(document: &str) -> String {
    let body = document.split_once('{').map_or("", |(_, b)| b).trim_start();
    body.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect()
}

/// A poisoned lock means a test already panicked; the state is still worth reading.
fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

/// What one connection asked for.
struct Head {
    method: String,
    path: String,
    length: usize,
    authorization: Option<String>,
}

enum Reply {
    Answer {
        status: u16,
        headers: Vec<(&'static str, String)>,
        body: Value,
    },
    /// An image, when the fake serves art.
    Image(&'static [u8]),
    HangUp,
}

impl Reply {
    fn ok(data: Value) -> Self {
        Self::status(200, json!({ "data": data }))
    }

    fn status(status: u16, body: Value) -> Self {
        Self::Answer { status, headers: Vec::new(), body }
    }

    /// GraphQL's error shape: an `errors` array, with `data` as the query left it.
    fn errors(status: u16, message: &str, data: Value) -> Self {
        Self::status(
            status,
            json!({ "errors": [{ "message": message, "status": status }], "data": data }),
        )
    }
}

async fn serve(socket: TcpStream, state: Arc<Mutex<State>>) {
    let (read, mut write) = socket.into_split();
    let mut reader = BufReader::new(read);
    let Some(head) = read_head(&mut reader).await else { return };
    let mut body = vec![0; head.length.min(MAX_BODY)];
    if reader.read_exact(&mut body).await.is_err() {
        return;
    }
    let reply = answer(&state, &head, &body);
    // Held back as a slow AniList would be, once the request is on the record.
    let payload: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let operation = Operation::of(payload["query"].as_str().unwrap_or_default());
    let slow = if head.path.starts_with("/art/") {
        Some(lock(&state).art_trouble.delay).filter(|wait| !wait.is_zero())
    } else {
        lock(&state).slow.get(&operation).copied()
    };
    if let Some(wait) = slow {
        tokio::time::sleep(wait).await;
    }
    let (status, headers, content_type, body) = match reply {
        Reply::Answer { status, headers, body } => {
            let mut body = body.to_string();
            let art = lock(&state).art.clone();
            if let Some(art) = art {
                body = body.replace(crate::fixtures::ART_HOST, &art);
            }
            (status, headers, "application/json", body.into_bytes())
        }
        Reply::Image(bytes) => (200, Vec::new(), "image/png", bytes.to_vec()),
        // A hang-up is the socket dropped here, unanswered.
        Reply::HangUp => return,
    };

    let mut out = format!(
        "HTTP/1.1 {status} {}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\
         connection: close\r\n",
        reason(status),
        body.len()
    );
    for (name, value) in headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str("\r\n");
    let mut out = out.into_bytes();
    out.extend_from_slice(&body);
    let _ = write.write_all(&out).await;
    let _ = write.shutdown().await;
}

async fn read_head(reader: &mut BufReader<OwnedReadHalf>) -> Option<Head> {
    let mut line = String::new();
    reader.read_line(&mut line).await.ok().filter(|n| *n > 0)?;
    let mut parts = line.split_whitespace();
    let mut head = Head {
        method: parts.next()?.to_owned(),
        path: parts.next()?.to_owned(),
        length: 0,
        authorization: None,
    };
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await.ok()? == 0 {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            return Some(head);
        }
        let Some((name, value)) = header.split_once(':') else { continue };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            head.length = value.parse().unwrap_or(0);
        } else if name.eq_ignore_ascii_case("authorization") {
            head.authorization = Some(value.to_owned());
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// Record the request, then answer it: with a failure if one is due, else from the fixtures.
/// Art, when the fake serves it, is answered without being recorded or failed, as it is not
/// AniList's, unless [`FakeAniList::trouble_art`] asked for it to be served badly.
fn answer(state: &Mutex<State>, head: &Head, body: &[u8]) -> Reply {
    if let Some(path) = head.path.strip_prefix("/art/") {
        let mut guard = lock(state);
        let state = &mut *guard;
        let serving = state.art.is_some();
        let first_asked = state.art_asked.len();
        let (order, times) = state.art_asked.entry(path.to_owned()).or_insert((first_asked, 0));
        *times += 1;
        if serving && *order % 2 == 0 && *times <= state.art_trouble.fail_first {
            return Reply::status(503, json!({ "message": "busy" }));
        }
        drop(guard);
        return match crate::art::png(path).filter(|_| serving && head.method == "GET") {
            Some(bytes) => Reply::Image(bytes),
            None => Reply::status(404, json!({ "message": "no such image" })),
        };
    }
    if let Some(rest) = head.path.strip_prefix("/kitsu") {
        lock(state).requests.push(Request {
            operation: Operation::Kitsu,
            path: head.path.clone(),
            query: String::new(),
            variables: Value::Null,
            authorization: head.authorization.clone(),
            at: std::time::Instant::now(),
            refused: false,
        });
        return kitsu::answer(&head.method, rest, head.authorization.as_deref(), body);
    }
    if let Some(rest) = head.path.strip_prefix("/tenrai/v1") {
        lock(state).requests.push(Request {
            operation: Operation::Tenrai,
            path: head.path.clone(),
            query: String::new(),
            variables: Value::Null,
            authorization: head.authorization.clone(),
            at: std::time::Instant::now(),
            refused: false,
        });
        return tenrai(rest);
    }

    let payload: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let query = payload["query"].as_str().unwrap_or_default().to_owned();
    let variables = payload.get("variables").cloned().unwrap_or(Value::Null);
    let operation = if head.method == "POST" && head.path == "/graphql" {
        Operation::of(&query)
    } else {
        Operation::Unknown
    };

    let asks_following = query.contains("isFollowing: true");
    // What the limit leaves of the minute, said on an answer it counted.
    let mut limit_headers: Option<(u32, usize, Option<u64>)> = None;
    // AniList refuses a query that costs more than 500. Only the queries the fake counts for
    // are held to it: see [`complexity`].
    if operation == Operation::Recommended && complexity(&query, &variables) > COMPLEXITY_LIMIT
    {
        return Reply::errors(400, "Max query complexity of 500 exceeded", Value::Null);
    }

    let (
        failure,
        (now, ahead),
        titles,
        (viewer_language, viewer_id),
        extra,
        pool,
        rows,
        following,
        season,
        listed,
    ) = {
        let mut state = lock(state);
        let at = std::time::Instant::now();
        let mut limited = None;
        if let Some((limit, minute)) = state.limit.filter(|_| head.path == "/graphql") {
            while state.counted.front().is_some_and(|first| at.duration_since(*first) >= minute)
            {
                state.counted.pop_front();
            }
            // The limit as a minute's worth, which is how AniList says it.
            let per_minute = (u64::from(limit) * 60 / minute.as_secs().max(1)) as u32;
            // How long until the oldest of the window leaves it, in whole seconds.
            let frees = |counted: &VecDeque<std::time::Instant>| {
                let oldest = counted.front().copied().unwrap_or(at);
                let wait =
                    minute.saturating_sub(at.duration_since(oldest)).as_secs_f64().ceil();
                (wait as u64).max(1)
            };
            if state.counted.len() >= limit as usize {
                limited =
                    Some(Failure::RateLimited { retry_after_secs: frees(&state.counted) });
                limit_headers = Some((per_minute, 0, None));
            } else {
                state.counted.push_back(at);
                let remaining = limit as usize - state.counted.len();
                // AniList's window is a minute, which a client waits out when nothing is
                // left. A shorter one says when it frees, or the client would wait the minute.
                let reset = (remaining == 0 && minute != std::time::Duration::from_secs(60))
                    .then(|| frees(&state.counted));
                limit_headers = Some((per_minute, remaining, reset));
            }
        }
        if head.path == "/graphql"
            && let Some((limit, left)) = state.said.pop_front()
        {
            limit_headers = Some((limit, left as usize, None));
        }
        state.requests.push(Request {
            operation,
            path: head.path.clone(),
            query: query.clone(),
            variables: variables.clone(),
            authorization: head.authorization.clone(),
            at,
            refused: limited.is_some(),
        });
        let failure = limited
            .or_else(|| state.next.pop_front())
            .or_else(|| state.always.clone())
            .or_else(|| state.failing.get(&operation).cloned());
        let clock = (state.now, state.ahead);
        let extra = state.extra.clone();
        let pool = state.catalogue.clone();
        let rows = state.broadcasts.clone();
        let following = state.following.clone();
        (
            failure,
            clock,
            state.titles.clone(),
            (state.viewer_language.clone(), state.viewer_id),
            extra,
            pool,
            rows,
            following,
            state.season.clone(),
            state.entries.clone(),
        )
    };
    let mut reply = match failure {
        Some(failure) => fail(failure),
        None => {
            let clock = Clock { shift: now - FIXTURE_NOW, now: now + ahead };
            let catalogue = Catalogue {
                extra: &extra,
                added: &pool,
                broadcasts: &rows,
                season: &season,
                entries: &listed,
            };
            let authorized = head.authorization.is_some();
            let writes = matches!(
                operation,
                Operation::SaveThreadComment
                    | Operation::DeleteThreadComment
                    | Operation::ToggleCommentLike
            );
            let thread = if writes && !authorized {
                Some(Reply::errors(401, "Invalid token", Value::Null))
            } else if operation == Operation::Books {
                let books = lock(state).books.clone();
                Some(books::respond(&books, &query, &variables))
            } else {
                threads::respond(&mut lock(state), operation, &variables, clock.now)
            };
            let mut reply = thread.unwrap_or_else(|| match together(&query, &variables) {
                // Each root of several in one request, answered as it would be alone.
                Some(parts) => {
                    let mut data = serde_json::Map::new();
                    for (alias, document, vars) in parts {
                        let operation = Operation::of(&document);
                        let reply = respond(
                            operation, &document, &vars, authorized, clock, catalogue, None,
                        );
                        match reply {
                            Reply::Answer { status: 200, mut body, .. }
                                if body.get("errors").is_none() =>
                            {
                                let root = root_field(&document);
                                let mut answer = body["data"][&root].take();
                                if root == "Viewer" {
                                    as_viewer(
                                        &mut answer,
                                        viewer_language.as_deref(),
                                        viewer_id,
                                    );
                                }
                                data.insert(alias, answer);
                            }
                            failed => return failed,
                        }
                    }
                    Reply::ok(Value::Object(data))
                }
                None => respond(
                    operation,
                    &query,
                    &variables,
                    authorized,
                    clock,
                    catalogue,
                    asks_following.then_some(&following),
                ),
            });
            if let Reply::Answer { body, .. } = &mut reply {
                retitle(body, &titles);
                if let Some(viewer) = body.pointer_mut("/data/Viewer").filter(|v| v.is_object())
                {
                    as_viewer(viewer, viewer_language.as_deref(), viewer_id);
                }
            }
            reply
        }
    };
    if let (Some((limit, remaining, reset)), Reply::Answer { headers, .. }) =
        (limit_headers, &mut reply)
    {
        headers.retain(|(name, _)| !name.starts_with("x-ratelimit-"));
        headers.push(("x-ratelimit-limit", limit.to_string()));
        headers.push(("x-ratelimit-remaining", remaining.to_string()));
        if let Some(reset) = reset {
            headers.push(("x-ratelimit-reset", reset.to_string()));
        }
    }
    reply
}

/// Give every title the test set another way its forms, wherever it is in `value`.
fn retitle(value: &mut Value, titles: &HashMap<u64, Value>) {
    if titles.is_empty() {
        return;
    }
    match value {
        Value::Object(fields) => {
            let id = fields.get("id").and_then(Value::as_u64);
            if let Some(title) = id.and_then(|id| titles.get(&id))
                && fields.contains_key("title")
            {
                fields.insert("title".into(), title.clone());
            }
            fields.values_mut().for_each(|field| retitle(field, titles));
        }
        Value::Array(items) => items.iter_mut().for_each(|item| retitle(item, titles)),
        _ => {}
    }
}

fn fail(failure: Failure) -> Reply {
    match failure {
        Failure::RateLimited { retry_after_secs } => Reply::Answer {
            status: 429,
            headers: vec![
                ("retry-after", retry_after_secs.to_string()),
                ("x-ratelimit-limit", "30".into()),
                ("x-ratelimit-remaining", "0".into()),
            ],
            body: json!({
                "errors": [{ "message": "Too Many Requests.", "status": 429 }],
                "data": null,
            }),
        },
        Failure::Status(status) => {
            Reply::errors(status, &format!("the fake AniList answered {status}"), Value::Null)
        }
        Failure::GraphQl(message) => Reply::errors(200, &message, Value::Null),
        Failure::Disconnect => Reply::HangUp,
    }
}

/// The empty Tenrai: no title is found, and every list is empty.
fn tenrai(path: &str) -> Reply {
    let path = path.split('?').next().unwrap_or_default();
    if path.starts_with("/anime/") && path.ends_with("/full") {
        return Reply::status(
            404,
            json!({ "status": 404, "message": "Resource does not exist" }),
        );
    }
    Reply::status(200, json!({ "data": [], "pagination": { "has_next_page": false } }))
}

/// Every title the fake can name by id: the fixtures, and those a test added.
#[derive(Clone, Copy)]
struct Catalogue<'a> {
    extra: &'a HashMap<u64, Value>,
    /// The added titles that a search and a browse also find: see
    /// [`FakeAniList::add_catalogue`].
    added: &'a [AnilistId],
    /// The broadcasts a test added to the schedule: see [`FakeAniList::add_broadcasts`].
    broadcasts: &'a [Value],
    /// The added titles of the real clock's season: see [`FakeAniList::add_season`].
    season: &'a [AnilistId],
    /// The list entries a test added: see [`FakeAniList::add_entries`].
    entries: &'a [Value],
}

impl<'a> Catalogue<'a> {
    /// `id` as the fake holds it, from the added titles or the fixtures, without copying it.
    fn wire(self, id: AnilistId) -> Option<&'a Value> {
        self.extra.get(&u64::from(id.get())).or_else(|| fixtures::wire(id))
    }

    /// `id` as a list query answers it: the detail-only fields left out.
    fn listed(self, id: AnilistId) -> Option<Value> {
        self.wire(id).map(fixtures::listed_form)
    }

    /// `id` as a page of search or browse results answers it: [`Self::listed`], with the
    /// services the page asks for and not their episode listings.
    fn listed_with_links(self, id: AnilistId) -> Option<Value> {
        let wire = self.wire(id)?;
        let mut media = fixtures::listed_form(wire);
        let links: Vec<Value> = wire["externalLinks"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|l| {
                json!({
                    "site": l["site"],
                    "url": l["url"],
                    "type": l["type"],
                    "isDisabled": l["isDisabled"].as_bool().unwrap_or(false),
                })
            })
            .collect();
        media["externalLinks"] = json!(links);
        Some(media)
    }

    /// Every title a browse draws from: the fixtures with the added titles mixed in, one after
    /// each, and any left over at the end.
    fn pool(self) -> Vec<AnilistId> {
        let mut pool = Vec::with_capacity(fixtures::ALL.len() + self.added.len());
        for n in 0..fixtures::ALL.len().max(self.added.len()) {
            pool.extend(fixtures::ALL.get(n).into_iter().chain(self.added.get(n)));
        }
        pool
    }

    /// The viewer's list entries: the fixture library's, then those a test added.
    fn entries(self) -> impl Iterator<Item = &'a Value> {
        fixtures::library_wire()["entries"].as_array().into_iter().flatten().chain(self.entries)
    }

    /// Every broadcast row: the fixtures' schedule, then those a test added.
    fn schedule(self) -> impl Iterator<Item = &'a Value> {
        fixtures::schedule_wire().iter().chain(self.broadcasts)
    }

    /// `id` as the single-title query answers it.
    fn get(self, id: u64) -> Option<Value> {
        let fixture = || fixtures::wire(AnilistId::new(id as u32)).cloned();
        self.extra.get(&id).cloned().or_else(fixture)
    }

    /// `id`'s own relations, as a related title carries them a level out: the kind, and the
    /// title by id and format. None for a title the fake does not have, as AniList would have
    /// some.
    fn neighbours(self, id: u64) -> Value {
        let edges: Vec<Value> = self
            .get(id)
            .and_then(|title| title["relations"]["edges"].as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .map(|edge| {
                json!({ "relationType": edge["relationType"],
                        "node": { "id": edge["node"]["id"], "format": edge["node"]["format"] } })
            })
            .collect();
        json!({ "edges": edges })
    }

    /// `media`'s relations with each related title's own, as the title page and the season walk
    /// ask for them.
    fn with_neighbours(self, mut media: Value) -> Value {
        if let Some(edges) = media.pointer_mut("/relations/edges").and_then(Value::as_array_mut)
        {
            for edge in edges {
                let id = edge["node"]["id"].as_u64().unwrap_or_default();
                edge["node"]["relations"] = self.neighbours(id);
            }
        }
        media
    }

    /// One title as the season walk is answered: its card, and its relations two levels deep.
    fn relations(self, id: u64) -> Option<Value> {
        let title = self.get(id)?;
        let mut card = json!({ "relations": title["relations"].clone() });
        for field in ["id", "title", "format", "season", "seasonYear"] {
            card[field] = title[field].clone();
        }
        card["coverImage"] = json!({ "large": title["coverImage"]["large"], "color": title["coverImage"]["color"] });
        Some(self.with_neighbours(card))
    }
}

fn respond(
    operation: Operation,
    document: &str,
    vars: &Value,
    authorized: bool,
    clock: Clock,
    catalogue: Catalogue<'_>,
    following: Option<&HashMap<u64, Vec<Value>>>,
) -> Reply {
    let needs_token = matches!(
        operation,
        Operation::Library | Operation::Viewer | Operation::SaveEntry | Operation::DeleteEntry
    ) || (operation == Operation::Detail && following.is_some());
    // The client never sends these without a token; a request that does is a bug worth seeing.
    if needs_token && !authorized {
        return Reply::errors(401, "Invalid token", Value::Null);
    }
    match operation {
        Operation::Search | Operation::Seasonal | Operation::Browse => {
            let mut data = serde_json::Map::new();
            for (alias, page) in pages(vars) {
                data.insert(
                    alias,
                    media_page(search(&page, catalogue), &page, clock, catalogue)["Page"]
                        .take(),
                );
            }
            Reply::ok(Value::Object(data))
        }
        Operation::Total => {
            let mut data = serde_json::Map::new();
            for (alias, page) in pages(vars) {
                let total = search(&page, catalogue).len();
                data.insert(alias, json!({ "pageInfo": { "total": total }, "media": [] }));
            }
            Reply::ok(Value::Object(data))
        }
        Operation::TrendWeek => {
            // The trend titles, most trending first, each on two days.
            let rows: Vec<Value> = trend_titles(catalogue)
                .into_iter()
                .enumerate()
                .flat_map(|(at, id)| {
                    let card = catalogue.listed_with_links(id).map(|m| clock.media(m));
                    [60, 40].map(|share| {
                        json!({
                            "mediaId": id.get(),
                            "trending": (TREND_TITLES - at) as u64 * share,
                            "media": card,
                        })
                    })
                })
                .collect();
            let mut rows = rows;
            rows.sort_by_key(|row| std::cmp::Reverse(row["trending"].as_u64()));
            Reply::ok(json!({ "Page": { "mediaTrends": rows } }))
        }
        Operation::TrendMonthCandidates => {
            // The same titles as the week, the later ones first: a month ranks them otherwise.
            let rows: Vec<Value> = trend_titles(catalogue)
                .into_iter()
                .rev()
                .map(|id| json!({ "mediaId": id.get(), "trending": 10 }))
                .collect();
            Reply::ok(json!({
                "p1": { "mediaTrends": rows },
                "p2": { "mediaTrends": [] },
                "p3": { "mediaTrends": [] },
                "p4": { "mediaTrends": [] },
            }))
        }
        Operation::TrendSeries => {
            let media: Vec<Value> = ids(&vars["ids"])
                .into_iter()
                .filter_map(|id| {
                    let at = trend_titles(catalogue).iter().position(|t| *t == id)?;
                    let mut card = clock.media(catalogue.listed_with_links(id)?);
                    // The later a title is in the catalogue, the more a month gives it.
                    let nodes: Vec<Value> = (0..3)
                        .map(|day| json!({ "date": day, "trending": (at as u64 + 1) * 10 }))
                        .collect();
                    card["trends"] = json!({ "nodes": nodes });
                    Some(card)
                })
                .collect();
            Reply::ok(json!({ "Page": { "media": media } }))
        }
        Operation::Facets => Reply::ok(facets()),
        Operation::Detail => {
            match vars["id"].as_u64().and_then(|id| catalogue.get(id)) {
                Some(media) => {
                    let mut data = detail(catalogue.with_neighbours(media), clock);
                    // Only to a document that asks, as AniList answers only what is asked.
                    if let Some(following) = following {
                        let entries = following
                            .get(&vars["id"].as_u64().unwrap_or_default())
                            .cloned()
                            .unwrap_or_default();
                        data["following"] = json!({ "mediaList": entries });
                    }
                    Reply::ok(data)
                }
                // What AniList answers for an id it has no title for: a 404 inside GraphQL's
                // error shape, which the client reads as an API failure rather than NotFound.
                None => Reply::errors(404, "Not Found.", json!({ "Media": null })),
            }
        }
        Operation::Many => {
            // The season headers ask each title's related titles too, each with its card.
            let headers = document.contains(kaeri_meta::anilist::query::HEADER_RELATIONS);
            let media: Vec<Value> = catalogue_order(&ids(&vars["ids"]), catalogue)
                .into_iter()
                .filter_map(|id| {
                    let mut listed = if fixtures::ALL.contains(&id) {
                        fixtures::listed_wire(id)
                    } else {
                        catalogue.listed(id)
                    }?;
                    if headers {
                        let edges: Vec<Value> = catalogue
                            .get(u64::from(id.get()))
                            .and_then(|title| title["relations"]["edges"].as_array().cloned())
                            .unwrap_or_default()
                            .into_iter()
                            .map(|mut edge| {
                                if let Some(node) = edge["node"].as_object_mut() {
                                    node.remove("relations");
                                }
                                edge
                            })
                            .collect();
                        listed["relations"] = json!({ "edges": edges });
                    }
                    Some(listed)
                })
                .map(|m| clock.media(m))
                .collect();
            Reply::ok(json!({ "Page": { "media": media } }))
        }
        Operation::Tables => {
            let media: Vec<Value> = catalogue_order(&ids(&vars["ids"]), catalogue)
                .into_iter()
                .filter_map(|id| Some((catalogue.listed(id)?, catalogue.wire(id)?)))
                .map(|(listed, wire)| {
                    let whole = clock.media(wire.clone());
                    let mut media = clock.media(listed);
                    // As AniList answers a title with none: an empty list, an empty connection.
                    media["streamingEpisodes"] =
                        whole.get("streamingEpisodes").cloned().unwrap_or_else(|| json!([]));
                    media["upcoming"] =
                        whole.get("upcoming").cloned().unwrap_or_else(|| json!({ "nodes": [] }));
                    // The connection takes no sort: oldest first, fifty at most.
                    let mut past = whole["aired"].as_array().cloned().unwrap_or_default();
                    past.sort_by_key(|row| row["episode"].as_u64());
                    past.truncate(50);
                    media["past"] = json!({ "nodes": past });
                    media
                })
                .collect();
            Reply::ok(json!({ "Page": { "media": media } }))
        }
        Operation::Recommended => {
            let per_page = vars["perPage"].as_u64().unwrap_or(MAX_PER_PAGE as u64) as usize;
            let document = document.split_whitespace().collect::<Vec<_>>().join(" ");
            // Asked for more than their ids, the titles come whole, as Home's "Because you
            // finished" once asked. As many as the connection's `perPage` says.
            let whole = !document.contains("mediaRecommendation { id }");
            let each = document
                .split_once("recommendations(perPage: ")
                .map(|(_, rest)| rest.split([',', ')']).next().unwrap_or_default())
                .and_then(|n| match n.strip_prefix('$') {
                    Some(name) => vars[name].as_u64(),
                    None => n.trim().parse().ok(),
                })
                .unwrap_or(MAX_PER_PAGE as u64) as usize;
            let media: Vec<Value> = ids(&vars["ids"])
                .into_iter()
                .take(per_page)
                .filter_map(|id| catalogue.wire(id))
                .map(|m| {
                    let ids = m["recommendations"]["nodes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|n| n["mediaRecommendation"]["id"].as_u64());
                    let nodes: Vec<Value> = if whole {
                        ids.filter_map(|id| catalogue.listed(AnilistId::new(id as u32)))
                            .take(each)
                            .map(|title| json!({ "mediaRecommendation": clock.media(title) }))
                            .collect()
                    } else {
                        ids.take(each).map(|id| json!({ "mediaRecommendation": { "id": id } })).collect()
                    };
                    json!({
                        "id": m["id"],
                        "title": m["title"],
                        "recommendations": { "nodes": nodes },
                    })
                })
                .collect();
            Reply::ok(json!({ "Page": { "media": media } }))
        }
        Operation::Relations => {
            let media: Vec<Value> = ids(&vars["ids"])
                .into_iter()
                .filter_map(|id| catalogue.relations(u64::from(id.get())))
                .collect();
            Reply::ok(json!({ "Page": { "media": media } }))
        }
        Operation::Schedule => Reply::ok(schedule(vars, clock, catalogue)),
        Operation::LastAired => Reply::ok(last_aired(vars, clock, catalogue)),
        Operation::AiredSince => Reply::ok(aired_since(vars, clock, catalogue)),
        Operation::Viewer => Reply::ok(json!({ "Viewer": fixtures::library_wire()["viewer"] })),
        Operation::Library => Reply::ok(library(clock, catalogue)),
        Operation::SaveEntry => Reply::ok(save_entry(vars, catalogue)),
        Operation::DeleteEntry => {
            let known = catalogue.entries().any(|e| e["id"] == vars["id"]);
            Reply::ok(json!({ "DeleteMediaListEntry": { "deleted": known } }))
        }
        // Answered from the threads a test made up, before this is reached.
        Operation::EpisodeThreads
        | Operation::ThreadComments
        | Operation::SaveThreadComment
        | Operation::DeleteThreadComment
        | Operation::ToggleCommentLike
        | Operation::Tenrai
        // Kitsu's requests are answered before the GraphQL ones are read.
        | Operation::Kitsu
        // Books are answered before the rest, from their own records.
        | Operation::Books
        | Operation::Unknown => {
            Reply::errors(400, "the fake AniList was never taught this query", Value::Null)
        }
    }
}

/// The fake's clock: how far the fixtures move, and the moment countdowns are counted from,
/// which is that far on and as far again as [`FakeAniList::advance`] has moved it.
#[derive(Clone, Copy)]
struct Clock {
    shift: i64,
    now: i64,
}

impl Clock {
    fn at(self, fixture_time: &Value) -> i64 {
        fixture_time.as_i64().unwrap_or_default() + self.shift
    }

    /// A title with its next broadcast moved, and its countdown counted from now — AniList
    /// computes `timeUntilAiring` per request, so a stored one would be wrong a second later.
    /// Its schedule moves with it, so the two never disagree.
    fn media(self, mut media: Value) -> Value {
        if let Some(next) = media.get_mut("nextAiringEpisode").and_then(Value::as_object_mut) {
            let at = self.at(&next["airingAt"]);
            next.insert("airingAt".into(), at.into());
            next.insert("timeUntilAiring".into(), (at - self.now).into());
        }
        if let Some(nodes) = media.pointer_mut("/upcoming/nodes").and_then(Value::as_array_mut)
        {
            self.broadcasts(nodes);
        }
        if let Some(aired) = media.get_mut("aired").and_then(Value::as_array_mut) {
            self.broadcasts(aired);
        }
        media
    }

    /// Each broadcast's `airingAt`, moved.
    fn broadcasts(self, rows: &mut [Value]) {
        for row in rows {
            let at = self.at(&row["airingAt"]);
            row["airingAt"] = at.into();
        }
    }
}

/// The titles of `asked` the fake has, in the catalogue's order rather than the order asked: the
/// client puts them back, and answering in the asked order would let a client that forgot pass
/// anyway. The titles a test added come after the fixtures, newest id first.
fn catalogue_order(asked: &[AnilistId], catalogue: Catalogue<'_>) -> Vec<AnilistId> {
    let mut added: Vec<AnilistId> = asked
        .iter()
        .copied()
        .filter(|id| !fixtures::ALL.contains(id))
        .filter(|id| catalogue.extra.contains_key(&u64::from(id.get())))
        .collect();
    added.sort_by_key(|id| std::cmp::Reverse(id.get()));
    fixtures::ALL.iter().copied().filter(|id| asked.contains(id)).chain(added).collect()
}

/// One entry of the people the viewer follows, as AniList answers it: for
/// [`FakeAniList::set_following`].
pub fn following_entry(
    name: &str,
    status: &str,
    score: Option<u32>,
    progress: u32,
    private: bool,
) -> Value {
    json!({
        "status": status,
        "score": score.unwrap_or(0),
        "progress": progress,
        "private": private,
        "user": { "name": name, "avatar": { "medium": format!("https://img.example/avatar/{name}.png") } },
    })
}

/// The single-title answer, as AniList shapes it: the title, and beside it the broadcasts
/// already out, newest first, under the alias `query::by_id` gives them. The fixture keeps them
/// on the title, as the app's `Media` does once it has read them.
fn detail(media: Value, clock: Clock) -> Value {
    let mut media = clock.media(media);
    let mut aired = media
        .as_object_mut()
        .and_then(|fields| fields.remove("aired"))
        .and_then(|aired| aired.as_array().cloned())
        .unwrap_or_default();
    aired.sort_by_key(|row| std::cmp::Reverse(row["episode"].as_u64()));
    json!({ "Media": media, "aired": { "airingSchedules": aired } })
}

/// Each page a `query::media_pages` request asks for, under its alias, with its variables
/// named as AniList's arguments are: `r0_genre_in` is `genre_in` of the page aliased `r0`.
fn pages(vars: &Value) -> Vec<(String, Value)> {
    let mut pages: Vec<(String, serde_json::Map<String, Value>)> = Vec::new();
    for (name, value) in vars.as_object().into_iter().flatten() {
        let Some((alias, argument)) = name.split_once('_') else { continue };
        let at = match pages.iter().position(|(a, _)| a == alias) {
            Some(at) => at,
            None => {
                pages.push((alias.to_owned(), serde_json::Map::new()));
                pages.len() - 1
            }
        };
        pages[at].1.insert(argument.to_owned(), value.clone());
    }
    pages.sort_by_key(|(alias, _)| alias[1..].parse::<u32>().unwrap_or(u32::MAX));
    pages.into_iter().map(|(alias, page)| (alias, Value::Object(page))).collect()
}

/// The titles a page finds, in the order asked for. A text search matches any title whose forms
/// or synonyms contain the term, any case; no term browses the whole catalogue. A season asked
/// with no term is answered with [`fixtures::SEASONAL`] when it is the one the real clock is in,
/// as the app asks for that one and a test should not break when the season turns, and with the
/// titles [`FakeAniList::add_season`] added beside them. Any other
/// season, and every other filter, is applied to the fixtures as AniList would apply it.
/// How many titles the fake's trending rows name.
const TREND_TITLES: usize = 6;

/// The titles the fake's trending rows are about: the first of the catalogue.
/// The titles a week or a month of trends ranks: the catalogue's first, starting halfway along,
/// so a week leads with other titles than today's live trending and the apps' shots of a period
/// switch show a change.
fn trend_titles(catalogue: Catalogue<'_>) -> Vec<AnilistId> {
    let mut titles: Vec<AnilistId> = catalogue.pool().into_iter().take(TREND_TITLES).collect();
    let half = titles.len() / 2;
    titles.rotate_left(half);
    titles
}

fn search(vars: &Value, catalogue: Catalogue<'_>) -> Vec<AnilistId> {
    let term = vars["search"].as_str().unwrap_or_default().trim().to_lowercase();
    let this_season = term.is_empty()
        && vars["season"].as_str() == Some(real_season().0)
        && vars["seasonYear"].as_u64() == Some(real_season().1);
    let pool: Vec<AnilistId> = if this_season {
        fixtures::SEASONAL.iter().chain(catalogue.season).copied().collect()
    } else {
        catalogue.pool()
    };
    let mut found: Vec<AnilistId> = pool
        .into_iter()
        .filter(|id| {
            let media = catalogue.wire(*id).expect("the pool lists titles the fake has");
            let names = ["romaji", "english", "native"]
                .iter()
                .filter_map(|form| media["title"][form].as_str())
                .chain(
                    media["synonyms"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str),
                );
            term.is_empty() || names.map(str::to_lowercase).any(|name| name.contains(&term))
        })
        .filter(|id| filtered(*id, vars, !this_season, catalogue))
        .collect();
    sort(&mut found, vars, catalogue);
    found
}

/// The season the real clock is in, in AniList's words, and its year.
fn real_season() -> (&'static str, u64) {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
        / 86_400;
    // Civil from days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let season = match month {
        1..=3 => "WINTER",
        4..=6 => "SPRING",
        7..=9 => "SUMMER",
        _ => "FALL",
    };
    (season, year as u64)
}

/// Whether a title passes every filter the page sets, as AniList reads them: `_in` is any of,
/// except genres and tags, which are every one of; `_not_in` is none of; `_greater` and
/// `_lesser` are exclusive. A field the fixture leaves out reads as AniList's most common
/// answer for it (made in Japan, licensed), or as nothing (no score, no popularity).
fn filtered(id: AnilistId, vars: &Value, with_season: bool, catalogue: Catalogue<'_>) -> bool {
    let Some(media) = catalogue.wire(id) else { return false };
    let list = |name: &str| vars[name].as_array().cloned().unwrap_or_default();
    let number = |name: &str| vars[name].as_i64();
    let genres = media["genres"].as_array().cloned().unwrap_or_default();
    let rank = number("minimumTagRank").unwrap_or(0);
    let tags: Vec<Value> = media["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["rank"].as_i64().unwrap_or(100) >= rank)
        .map(|t| t["name"].clone())
        .collect();
    let any_of = |name: &str, field: &Value| {
        let wanted = list(name);
        wanted.is_empty() || wanted.contains(field)
    };
    let date = |d: &Value| {
        d["year"].as_i64().unwrap_or(0) * 10_000
            + d["month"].as_i64().unwrap_or(0) * 100
            + d["day"].as_i64().unwrap_or(0)
    };
    let bounded = |field: Option<i64>, name: &str| {
        let greater = number(&format!("{name}_greater"));
        let lesser = number(&format!("{name}_lesser"));
        match field {
            Some(value) => {
                greater.is_none_or(|g| value > g) && lesser.is_none_or(|l| value < l)
            }
            None => greater.is_none() && lesser.is_none(),
        }
    };
    let country = media.get("countryOfOrigin").filter(|c| !c.is_null()).cloned();
    let country = country.unwrap_or_else(|| json!("JP"));
    let sites: Vec<Value> = media["externalLinks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|link| link["site"].clone())
        .collect();
    let on_list =
        catalogue.entries().any(|e| e["mediaId"].as_u64() == Some(u64::from(id.get())));

    let only = list("id_in");
    (only.is_empty() || only.iter().any(|i| i.as_u64() == Some(u64::from(id.get()))))
        && list("genre_in").iter().all(|g| genres.contains(g))
        && !list("genre_not_in").iter().any(|g| genres.contains(g))
        && list("tag_in").iter().all(|t| tags.contains(t))
        && !list("tag_not_in").iter().any(|t| tags.contains(t))
        && any_of("format_in", &media["format"])
        && any_of("status_in", &media["status"])
        && any_of("source_in", &media["source"])
        && any_of("countryOfOrigin_in", &country)
        && (vars["countryOfOrigin"].is_null() || vars["countryOfOrigin"] == country)
        && (!with_season
            || ((vars["season"].is_null() || vars["season"] == media["season"])
                && (vars["seasonYear"].is_null() || vars["seasonYear"] == media["seasonYear"])))
        && bounded(Some(date(&media["startDate"])), "startDate")
        && bounded(media["episodes"].as_i64(), "episodes")
        && bounded(media["duration"].as_i64(), "duration")
        && number("averageScore_greater")
            .is_none_or(|g| media["averageScore"].as_i64().is_some_and(|s| s > g))
        && number("popularity_greater")
            .is_none_or(|g| media["popularity"].as_i64().unwrap_or(0) > g)
        && (list("licensedBy_in").is_empty()
            || list("licensedBy_in").iter().any(|site| sites.contains(site)))
        && vars["isLicensed"]
            .as_bool()
            .is_none_or(|l| media["isLicensed"].as_bool().unwrap_or(true) == l)
        && vars["isAdult"]
            .as_bool()
            .is_none_or(|a| media["isAdult"].as_bool().unwrap_or(false) == a)
        && vars["onList"].as_bool().is_none_or(|wanted| on_list == wanted)
}

/// The page's sort, first name deciding, as AniList orders: relevance and `ID` keep the
/// catalogue's order, and a title with nothing to sort by goes last.
fn sort(found: &mut [AnilistId], vars: &Value, catalogue: Catalogue<'_>) {
    let first = match &vars["sort"] {
        Value::Array(sorts) => {
            sorts.first().and_then(Value::as_str).unwrap_or_default().to_owned()
        }
        sort => sort.as_str().unwrap_or_default().to_owned(),
    };
    let field = |id: &AnilistId, name: &str| {
        catalogue.wire(*id).and_then(|m| m[name].as_i64()).unwrap_or(i64::MIN)
    };
    let started = |id: &AnilistId| {
        catalogue.wire(*id).map_or(0, |m| {
            let d = &m["startDate"];
            d["year"].as_i64().unwrap_or(0) * 10_000
                + d["month"].as_i64().unwrap_or(0) * 100
                + d["day"].as_i64().unwrap_or(0)
        })
    };
    match first.as_str() {
        "SCORE_DESC" => found.sort_by_key(|id| std::cmp::Reverse(field(id, "averageScore"))),
        "POPULARITY_DESC" => found.sort_by_key(|id| std::cmp::Reverse(field(id, "popularity"))),
        "TRENDING_DESC" => found.sort_by_key(|id| std::cmp::Reverse(field(id, "trending"))),
        "FAVOURITES_DESC" => found.sort_by_key(|id| std::cmp::Reverse(field(id, "favourites"))),
        "START_DATE_DESC" => found.sort_by_key(|id| std::cmp::Reverse(started(id))),
        "START_DATE" => found.sort_by_key(started),
        "TITLE_ROMAJI" => found.sort_by_key(|id| {
            catalogue
                .wire(*id)
                .and_then(|m| m["title"]["romaji"].as_str().map(str::to_lowercase))
                .unwrap_or_default()
        }),
        _ => {}
    }
}

/// What the filters choose from: the genres the fixtures have and a few beside, made-up tags in
/// AniList's categories, one of them adult, and streaming sites, one of them disabled.
fn facets() -> Value {
    json!({
        "GenreCollection": ["Action", "Adventure", "Comedy", "Drama", "Fantasy", "Hentai",
                            "Mecha", "Mystery", "Romance", "Sci-Fi", "Slice of Life", "Sports"],
        "MediaTagCollection": [
            { "name": "Travel", "description": "Journeys from place to place.",
              "category": "Theme-Other", "isAdult": false, "isGeneralSpoiler": false },
            { "name": "Found Family", "description": "Strangers who become a family.",
              "category": "Theme-Drama", "isAdult": false, "isGeneralSpoiler": false },
            { "name": "Libraries", "description": "Set in or around a library.",
              "category": "Setting-Scene", "isAdult": false, "isGeneralSpoiler": false },
            { "name": "Weather Magic", "description": "Magic over wind and rain.",
              "category": "Theme-Fantasy", "isAdult": false, "isGeneralSpoiler": false },
            { "name": "Trains", "description": "Railways feature prominently.",
              "category": "Setting-Scene", "isAdult": false, "isGeneralSpoiler": false },
            { "name": "Gardening", "description": "Growing things.",
              "category": "Theme-Slice of Life", "isAdult": false, "isGeneralSpoiler": false },
            { "name": "The Last Lantern", "description": "A twist at the end.",
              "category": "Theme-Drama", "isAdult": false, "isGeneralSpoiler": true },
            { "name": "Nudity", "description": "Adult content.",
              "category": "Sexual Content", "isAdult": true, "isGeneralSpoiler": false },
        ],
        "ExternalLinkSourceCollection": [
            { "site": "Lantern Library", "isDisabled": false },
            { "site": "Hikari Library", "isDisabled": false },
            { "site": "Lantern Library", "isDisabled": false },
            { "site": "Old Stream", "isDisabled": true },
        ],
    })
}

/// `page` and `perPage` applied to a list, as AniList pages: one-based, at most fifty.
fn paged<T>(items: Vec<T>, vars: &Value) -> (Vec<T>, bool) {
    let page = vars["page"].as_u64().unwrap_or(1).max(1) as usize;
    let per_page = (vars["perPage"].as_u64().unwrap_or(MAX_PER_PAGE as u64) as usize)
        .clamp(1, MAX_PER_PAGE);
    let more = items.len() > page * per_page;
    (items.into_iter().skip((page - 1) * per_page).take(per_page).collect(), more)
}

fn media_page(
    found: Vec<AnilistId>,
    vars: &Value,
    clock: Clock,
    catalogue: Catalogue<'_>,
) -> Value {
    let (found, more) = paged(found, vars);
    let media: Vec<Value> = found
        .into_iter()
        .filter_map(|id| catalogue.listed_with_links(id))
        .map(|m| clock.media(m))
        .collect();
    json!({ "Page": { "pageInfo": { "hasNextPage": more }, "media": media } })
}

fn ids(value: &Value) -> Vec<AnilistId> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|id| AnilistId::new(id as u32))
        .collect()
}

/// Broadcasts strictly inside `from`..`to`, as `airingAt_greater` and `airingAt_lesser` have
/// it, in the order `sort` asks for.
fn schedule(vars: &Value, clock: Clock, catalogue: Catalogue<'_>) -> Value {
    let from = vars["from"].as_i64().unwrap_or(i64::MIN);
    let to = vars["to"].as_i64().unwrap_or(i64::MAX);
    let newest_first = sort_is(vars, "TIME_DESC");
    let mut rows: Vec<(i64, &Value)> = catalogue
        .schedule()
        .map(|row| (clock.at(&row["airingAt"]), row))
        .filter(|(at, _)| *at > from && *at < to)
        .collect();
    rows.sort_by_key(|(at, _)| if newest_first { -*at } else { *at });
    let (rows, more) = paged(rows, vars);
    let rows: Vec<Value> = rows
        .into_iter()
        .filter_map(|(at, row)| {
            let id = AnilistId::new(row["mediaId"].as_u64()? as u32);
            Some(json!({
                "episode": row["episode"],
                "airingAt": at,
                "media": clock.media(catalogue.listed(id)?),
            }))
        })
        .collect();
    json!({ "Page": { "pageInfo": { "hasNextPage": more }, "airingSchedules": rows } })
}

/// The newest broadcasts already aired for these titles, newest first, `perPage` of them.
fn last_aired(vars: &Value, clock: Clock, catalogue: Catalogue<'_>) -> Value {
    let asked = ids(&vars["ids"]);
    let mut rows: Vec<(i64, &Value)> = catalogue
        .schedule()
        .filter(|row| {
            row["mediaId"].as_u64().is_some_and(|id| asked.contains(&AnilistId::new(id as u32)))
        })
        .map(|row| (clock.at(&row["airingAt"]), row))
        .filter(|(at, _)| *at <= clock.now)
        .collect();
    rows.sort_by_key(|(at, _)| -*at);
    let (rows, _) = paged(rows, vars);
    let rows: Vec<Value> = rows
        .into_iter()
        .map(|(at, row)| {
            json!({ "mediaId": row["mediaId"], "episode": row["episode"], "airingAt": at })
        })
        .collect();
    json!({ "Page": { "airingSchedules": rows } })
}

/// What aired for these titles strictly inside `from`..`to`, oldest first, a page of them: each
/// with its title's forms and cover.
fn aired_since(vars: &Value, clock: Clock, catalogue: Catalogue<'_>) -> Value {
    let asked = ids(&vars["ids"]);
    let from = vars["from"].as_i64().unwrap_or(i64::MIN);
    let to = vars["to"].as_i64().unwrap_or(i64::MAX);
    let mut rows: Vec<(i64, &Value)> = catalogue
        .schedule()
        .filter(|row| {
            row["mediaId"].as_u64().is_some_and(|id| asked.contains(&AnilistId::new(id as u32)))
        })
        .map(|row| (clock.at(&row["airingAt"]), row))
        .filter(|(at, _)| *at > from && *at < to)
        .collect();
    rows.sort_by_key(|(at, _)| *at);
    let (rows, more) = paged(rows, vars);
    let rows: Vec<Value> = rows
        .into_iter()
        .filter_map(|(at, row)| {
            let id = AnilistId::new(row["mediaId"].as_u64()? as u32);
            let media = catalogue.wire(id)?;
            Some(json!({
                "episode": row["episode"],
                "airingAt": at,
                "media": {
                    "id": media["id"],
                    "title": media["title"],
                    "coverImage": media["coverImage"],
                    "bannerImage": media["bannerImage"],
                },
            }))
        })
        .collect();
    json!({ "Page": { "pageInfo": { "hasNextPage": more }, "airingSchedules": rows } })
}

fn sort_is(vars: &Value, wanted: &str) -> bool {
    match &vars["sort"] {
        Value::Array(sorts) => sorts.first().and_then(Value::as_str) == Some(wanted),
        sort => sort.as_str() == Some(wanted),
    }
}

/// The viewer's list, one list per status as AniList groups it, whatever `userId` asked for.
fn library(clock: Clock, catalogue: Catalogue<'_>) -> Value {
    let mut lists: Vec<(String, Vec<Value>)> = Vec::new();
    for entry in catalogue.entries() {
        let id = AnilistId::new(entry["mediaId"].as_u64().unwrap_or_default() as u32);
        let Some(media) = catalogue.listed(id) else { continue };
        let status = entry["status"].as_str().unwrap_or("CURRENT").to_owned();
        let row = json!({
            "id": entry["id"],
            "status": status,
            "progress": entry["progress"],
            "score": entry["score"],
            "updatedAt": entry["updatedAt"],
            "media": clock.media(media),
        });
        match lists.iter_mut().find(|(s, _)| *s == status) {
            Some((_, rows)) => rows.push(row),
            None => lists.push((status, vec![row])),
        }
    }
    let lists: Vec<Value> =
        lists.into_iter().map(|(_, rows)| json!({ "entries": rows })).collect();
    json!({ "MediaListCollection": { "lists": lists } })
}

/// The upsert's answer: the library entry's id, or for a title not on the list a stable
/// made-up one (its media id plus a million), with the fields as written.
///
/// Nothing is stored. The library answers the same afterwards, and a test asserts on what
/// was sent through [`FakeAniList::requests_for`].
fn save_entry(vars: &Value, catalogue: Catalogue<'_>) -> Value {
    let media_id = vars["mediaId"].as_u64().unwrap_or_default();
    let existing = catalogue.entries().find(|e| e["mediaId"].as_u64() == Some(media_id));
    let id = existing.map_or(json!(media_id + 1_000_000), |e| e["id"].clone());
    let field = |name: &str, otherwise: Value| {
        if vars[name].is_null() {
            existing.map_or(otherwise, |e| e[name].clone())
        } else {
            vars[name].clone()
        }
    };
    json!({ "SaveMediaListEntry": {
        "id": id,
        "progress": field("progress", json!(0)),
        "status": field("status", json!("CURRENT")),
    } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_meta::{BrowseFilter, anilist::query};

    fn search_document(term: &str, filter: &BrowseFilter) -> String {
        query::media_pages(&[filter.arguments(term, 1, 20)]).0
    }

    #[test]
    fn every_document_the_app_sends_is_one_the_fake_knows() {
        let season = BrowseFilter {
            season: Some("FALL".into()),
            year: Some(2026),
            ..BrowseFilter::default()
        };
        // The documents carry no operation names, so the fake tells them apart by content. A
        // change to a query that broke that would otherwise surface as a baffling test failure
        // somewhere far from here.
        for (document, operation) in [
            (search_document("kaze", &BrowseFilter::default()), Operation::Search),
            (search_document("", &BrowseFilter::default()), Operation::Search),
            (search_document("", &season), Operation::Seasonal),
            (search_document("kaze", &season), Operation::Search),
            (
                query::media_pages(&[
                    BrowseFilter::default().arguments("", 1, 20),
                    season.arguments("", 1, 20),
                ])
                .0,
                Operation::Browse,
            ),
            (query::FACETS.to_owned(), Operation::Facets),
            (
                query::media_total(&BrowseFilter::default().arguments("", 1, 1)).0,
                Operation::Total,
            ),
            (query::trend_week(), Operation::TrendWeek),
            (query::trend_month_candidates(), Operation::TrendMonthCandidates),
            (query::trend_month_series(), Operation::TrendSeries),
            (query::by_id(false), Operation::Detail),
            (query::by_id(true), Operation::Detail),
            (query::by_ids(), Operation::Many),
            (query::headers_by_ids(), Operation::Many),
            (query::by_ids_with_tables(), Operation::Tables),
            (query::relations(), Operation::Relations),
            (query::recommended_from(), Operation::Recommended),
            (query::airing_schedule(), Operation::Schedule),
            (query::LAST_AIRED.to_owned(), Operation::LastAired),
            (query::AIRED_SINCE.to_owned(), Operation::AiredSince),
            (query::user_library(), Operation::Library),
            (query::VIEWER.to_owned(), Operation::Viewer),
            (query::SAVE_PROGRESS.to_owned(), Operation::SaveEntry),
            (query::DELETE_ENTRY.to_owned(), Operation::DeleteEntry),
            (query::EPISODE_THREADS.to_owned(), Operation::EpisodeThreads),
            (query::thread_comments(), Operation::ThreadComments),
            (query::save_thread_comment(), Operation::SaveThreadComment),
            (query::DELETE_THREAD_COMMENT.to_owned(), Operation::DeleteThreadComment),
            (query::TOGGLE_COMMENT_LIKE.to_owned(), Operation::ToggleCommentLike),
        ] {
            assert_eq!(Operation::of(&document), operation, "{document}");
        }
        assert_eq!(Operation::of("query { Studio(id: 1) { name } }"), Operation::Unknown);
    }

    #[test]
    fn a_shifted_clock_moves_broadcasts_and_keeps_countdowns_honest() {
        let clock = Clock { shift: 1_000, now: FIXTURE_NOW + 1_000 };
        let moved = clock.media(fixtures::wire(fixtures::AIRING).unwrap().clone());
        let next = &moved["nextAiringEpisode"];
        assert_eq!(next["airingAt"], 1_792_163_730 + 1_000);
        assert_eq!(next["timeUntilAiring"], 184_500, "the same distance away");
    }
}
