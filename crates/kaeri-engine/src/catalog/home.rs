//! Home: the spotlight, Continue, the week and discovery, composed here and drawn by every app.
//!
//! What is local goes out at once: Continue from the watch history and the metadata cache, the
//! spotlight when something is waiting in it, and every part kept from the last load. Then each
//! part AniList answers for (the season's rails, the suggestions, the week, today) lands on its
//! own, and Home goes out again, whole, with that part in its place. Nothing a later part brings
//! moves what is above it, as the order is fixed here.
//!
//! What AniList said is kept as ids for half an hour under `home:*` (the season's rails under
//! Search's own key, as they are the same request), so a return to Home within it asks nothing.
//! Offline, the kept parts are shown whatever their age, with when they were fetched.
//!
//! Dismissals are Up next for you's own (`up-next:dismissed`), and every shelf and the spotlight
//! honour them, so Not interested anywhere takes the title off all of Home, from what is kept,
//! with no new request.

use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use kaeri_core::ids::AnilistId;
use kaeri_meta::{Media, Meta};
use kaeri_model::{
    Msg, Span,
    catalog::{AiringStatus, Entry, Season},
    event::Event,
    home::{
        CONTINUE_SHOWN, HiddenTitle, HomeAction, HomeEnd, HomeItem, HomeLink, HomePart,
        HomeSection, HomeView, Playable, SHELF_SHOWN, SUGGESTIONS_SHOWN, SectionKind,
        Spotlight, SpotlightCase, WEEK_DAYS, home_checked_action,
    },
    present::SeasonYear,
    present::{Casing, QueueItem},
    search::{SearchQuery, SearchSort},
    up_next::{UpNextAction, UpNextReason},
};
use kaeri_store::Store;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    catalog::{
        browse,
        filter::{self, Viewer},
        up_next::{self, Pick},
    },
    data,
    dispatch::Lent,
    outbox::Outbox,
};

/// How long what AniList said is served before it is asked again.
pub const KEPT_FOR: i64 = 30 * 60;
const SEASON_KEY: &str = "home:season";
const PERSONAL_KEY: &str = "home:personal";
const WEEK_KEY: &str = "home:week";
const POPULAR_WEEK_KEY: &str = "home:week-popular";
/// When the last load had every part answer: what an offline Home says it is from.
const DAY: i64 = 86_400;
/// The Continue rows looked at for something waiting. The heading counts the waiting ones.
const CONTINUE_LOOKED_AT: u32 = 50;
/// How long a Continue title's cached record serves before Home asks for it again: its airing
/// is what tells a caught-up show from one with an episode waiting.
const CONTINUE_FRESH: i64 = 30 * 60;
/// How many suggestions are found, beyond what is shown, so a dismissal or a title started
/// since leaves the shelf full without asking again.
const SUGGESTIONS_FOUND: usize = 24;
/// The most recommendations asked for "Because you finished", before the shelf's filters: the
/// most whole titles one request can carry inside AniList's query complexity of 500.
const BECAUSE_ASKED: u32 = 9;
/// The score, out of 100, a finished title needs before "Because you finished" is about it.
const BECAUSE_SCORE: u32 = 70;
/// The finished titles a genre needs before it has a taste rail.
const GENRE_AT_LEAST: usize = 3;
/// How long before the next season starts its shelf shows.
const STARTING_BEFORE: i64 = 14 * DAY;
/// A show from the history counts as followed for This week while it airs, or for this long
/// after its last broadcast.
const FOLLOWED_FOR: i64 = 21 * DAY;
/// The titles of the season before, asked for when nothing of this one has aired yet.
const PREVIOUS_ASKED: u32 = 20;
/// The deepest page of a shelf's order asked, fifty titles each, the first with the rails, so a
/// viewer who follows most of the season costs a request or two more, never the whole season.
const DEEPER_PAGES: u32 = 3;
/// What a shelf is filled to before it stops asking further down its order: a full shelf with
/// room left for the titles the shelves above it take.
const DEEPER_WANTED: usize = SHELF_SHOWN + SHELF_SHOWN / 2;
/// The pages of the airing schedule the popular week reads, fifty broadcasts each: a week of
/// every broadcast AniList lists, in the order they air.
const POPULAR_WEEK_PAGES: u32 = 6;
/// The broadcasts of a day the popular week keeps, so a dismissal leaves the day full.
const POPULAR_WEEK_KEPT: usize = 12;
/// The broadcasts of a day the popular week shows: premieres first, then the most popular.
const POPULAR_WEEK_SHOWN: usize = 3;

/// What a load of Home is told of the viewer and the moment.
#[derive(Clone)]
pub struct HomeAsk {
    /// A tracker is signed in, so its list counts.
    pub listed: bool,
    /// Adult titles may be suggested.
    pub adult: bool,
    pub now: i64,
    pub zone: jiff::tz::TimeZone,
    /// No network path: show what is kept, and ask nothing.
    pub offline: bool,
    /// Ask again even what was kept less than half an hour ago.
    pub refresh: bool,
    /// A source is installed and on, so something on Home can play.
    pub sources: bool,
    /// Where the demo stands: the spotlight's case 0 and the demo-only closing line.
    pub demo: kaeri_model::sources::DemoState,
}

/// What the engine remembers of Home between sends. Cheap to clone; every clone is the same.
#[derive(Clone, Default)]
pub struct HomeMemory(Arc<Mutex<Memory>>);

#[derive(Default)]
struct Memory {
    /// Home has been asked for, so a change to what it shows sends it again.
    asked: bool,
    /// Settings, Hidden titles has been asked for, so a dismissal sends it again.
    hidden_asked: bool,
    /// The parts on their way.
    loading: HashSet<HomePart>,
    /// What the season part on its way was asked from, while one is (see [`season_inputs`]),
    /// and its end: true once it has been sent.
    season_running: Option<(SeasonInputs, tokio::sync::watch::Receiver<bool>)>,
    /// Continue's records are being asked for again, so what is waiting in it is not known yet.
    continuing: bool,
    /// The last Home sent, with `failed` off.
    last: Option<HomeView>,
}

impl HomeMemory {
    fn lock(&self) -> std::sync::MutexGuard<'_, Memory> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Whether Home has been asked for.
    pub fn asked(&self) -> bool {
        self.lock().asked
    }

    /// Whether Settings, Hidden titles has been asked for.
    pub fn hidden_asked(&self) -> bool {
        self.lock().hidden_asked
    }

    pub fn ask_hidden(&self) {
        self.lock().hidden_asked = true;
    }

    /// Whether `id` is on the Home last sent.
    pub fn shows(&self, id: AnilistId) -> bool {
        self.lock().last.as_ref().is_some_and(|home| home.ids().any(|shown| shown == id))
    }
}

/// The season's rails, as kept: this season's most popular, the next season's, the season
/// before when nothing of this one has aired, and the taste rail.
#[derive(Serialize, Deserialize, Clone, Default)]
struct SeasonPart {
    at: i64,
    season: String,
    year: u16,
    adult: bool,
    this: Vec<u32>,
    next: Vec<u32>,
    #[serde(default)]
    previous: Vec<u32>,
    #[serde(default)]
    genre: Option<Rail>,
    /// What is trending now, from any season: Search's first rail.
    #[serde(default)]
    trending: Vec<u32>,
}

/// A rail named after what it is from: a genre, or a finished title.
#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
struct Rail {
    name: String,
    ids: Vec<u32>,
}

/// What the suggestions were found from, and what they are.
#[derive(Serialize, Deserialize, Clone, Default)]
struct PersonalPart {
    at: i64,
    from: u64,
    picks: Vec<Pick>,
    #[serde(default)]
    because: Option<Rail>,
    /// The titles found finished, for the rules every shelf keeps.
    #[serde(default)]
    finished: Vec<u32>,
    /// A lookup failed, so this is what the others found: shown, and asked for again at once
    /// rather than served for half an hour.
    #[serde(default)]
    incomplete: bool,
}

/// One broadcast.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
struct Broadcast {
    id: u32,
    episode: u32,
    at: i64,
}

/// This week's broadcasts of the shows followed, or with none, the most popular of every show.
#[derive(Serialize, Deserialize, Clone, Default)]
struct SchedulePart {
    at: i64,
    from: u64,
    broadcasts: Vec<Broadcast>,
    /// The titles of broadcasts the metadata cache did not have, as the schedule sent them.
    #[serde(default)]
    media: Vec<Media>,
}

fn kept<T: DeserializeOwned>(store: &Store, key: &str) -> Option<T> {
    store.get_meta_text(key).ok().flatten().and_then(|json| serde_json::from_str(&json).ok())
}

fn keep<T: Serialize>(store: &Store, key: &str, value: &T) {
    if let Ok(json) = serde_json::to_string(value)
        && let Err(e) = store.set_meta_text(key, &json)
    {
        tracing::warn!(error = %e, key, "could not keep part of Home");
    }
}

/// The day `now` is in, in `zone`: its midnight, and the date.
fn today(now: i64, zone: &jiff::tz::TimeZone) -> Option<(i64, jiff::civil::Date)> {
    let zoned = jiff::Timestamp::from_second(now).ok()?.to_zoned(zone.clone());
    let start = zoned.start_of_day().ok()?;
    Some((start.timestamp().as_second(), zoned.date()))
}

/// The season the viewer's own date is in, and its year: it changes on 1 January, 1 April,
/// 1 July and 1 October where they are.
pub fn season_on(now: i64, zone: &jiff::tz::TimeZone) -> (Season, u16) {
    let date = today(now, zone).map(|(_, d)| d);
    let (year, month) = date.map_or((1970, 1), |d| (d.year(), d.month()));
    (season_of_month(month), year.max(0) as u16)
}

fn season_of_month(month: i8) -> Season {
    match month {
        1..=3 => Season::Winter,
        4..=6 => Season::Spring,
        7..=9 => Season::Summer,
        _ => Season::Fall,
    }
}

fn next_season(season: Season, year: u16) -> (Season, u16) {
    match season {
        Season::Winter => (Season::Spring, year),
        Season::Spring => (Season::Summer, year),
        Season::Summer => (Season::Fall, year),
        Season::Fall => (Season::Winter, year + 1),
    }
}

fn previous_season(season: Season, year: u16) -> (Season, u16) {
    match season {
        Season::Winter => (Season::Fall, year.saturating_sub(1)),
        Season::Spring => (Season::Winter, year),
        Season::Summer => (Season::Spring, year),
        Season::Fall => (Season::Summer, year),
    }
}

/// When the season after the one `now` is in starts, in Unix seconds, at midnight where the
/// viewer is.
fn next_season_starts(now: i64, zone: &jiff::tz::TimeZone) -> Option<i64> {
    let (_, date) = today(now, zone)?;
    let month = (date.month() - 1) / 3 * 3 + 4;
    let (year, month) = if month > 12 { (date.year() + 1, 1) } else { (date.year(), month) };
    let start = jiff::civil::date(year, month, 1).to_zoned(zone.clone()).ok()?;
    Some(start.timestamp().as_second())
}

fn season_wire(season: Season) -> &'static str {
    kaeri_model::present::season_wire(season)
}

fn starting_title(season: Season) -> Msg {
    match season {
        Season::Winter => Msg::HomeStartingWinter,
        Season::Spring => Msg::HomeStartingSpring,
        Season::Summer => Msg::HomeStartingSummer,
        Season::Fall => Msg::HomeStartingAutumn,
    }
}

/// Where the Continue rows and the history stand, to tell when Home has to go out again: an
/// episode watched, a show started or finished, a row removed.
pub fn queue_mark(store: &Store) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for p in store.continue_list(CONTINUE_LOOKED_AT).unwrap_or_default() {
        (p.anilist_id, &p.last_episode, p.episodes_done, p.updated_at).hash(&mut hasher);
        p.last_position.to_bits().hash(&mut hasher);
    }
    for p in store.watched_titles(CONTINUE_LOOKED_AT).unwrap_or_default() {
        (p.anilist_id, p.episodes_done).hash(&mut hasher);
    }
    hasher.finish()
}

/// The genre most common among the titles finished, when at least [`GENRE_AT_LEAST`] of them
/// share it, from what is cached. Ties go to the genre of the most recently finished title.
fn favourite_genre(
    meta: &Meta,
    store: &Store,
    finished: &HashSet<AnilistId>,
) -> Option<String> {
    let mut order: Vec<AnilistId> =
        store.watched_titles(200).unwrap_or_default().iter().map(|p| p.anilist_id).collect();
    let mut rest: Vec<AnilistId> =
        finished.iter().copied().filter(|id| !order.contains(id)).collect();
    rest.sort_unstable_by_key(|id| id.get());
    order.extend(rest);
    let ids: Vec<AnilistId> = order.into_iter().filter(|id| finished.contains(id)).collect();
    let mut counts: Vec<(String, usize)> = Vec::new();
    for media in meta.cached(&ids) {
        for genre in &media.genres {
            match counts.iter_mut().find(|(g, _)| g == genre) {
                Some((_, n)) => *n += 1,
                None => counts.push((genre.clone(), 1)),
            }
        }
    }
    // The first to reach the highest count wins: the order is the most recently finished first.
    let best = counts.iter().map(|(_, n)| *n).max()?;
    (best >= GENRE_AT_LEAST)
        .then(|| counts.into_iter().find(|(_, n)| *n == best).map(|(g, _)| g))
        .flatten()
}

fn genre_query(genre: &str) -> SearchQuery {
    SearchQuery {
        genres: vec![genre.to_owned()],
        sort: Some(SearchSort::Popularity),
        ..SearchQuery::default()
    }
}

/// The ids of the Continue rows, most recent first.
fn continue_ids(store: &Store) -> Vec<AnilistId> {
    store
        .continue_list(CONTINUE_LOOKED_AT)
        .unwrap_or_default()
        .iter()
        .map(|p| p.anilist_id)
        .collect()
}

/// The viewer, as every shelf's rules read them, with what the suggestions found finished.
fn viewer<'a>(meta: &Meta, store: &'a Store, ask: &HomeAsk) -> Viewer<'a> {
    let mut finished = filter::finished_here(meta, store, ask.listed);
    if let Some(part) = kept::<PersonalPart>(store, PERSONAL_KEY) {
        finished.extend(part.finished.iter().map(|id| AnilistId::new(*id)));
    }
    Viewer::new(
        store,
        ask.listed,
        ask.adult,
        continue_ids(store),
        finished,
        filter::dismissed(store),
    )
}

/// Whether anything of the title is out: AniList says it airs or has aired, or schedules an
/// episode after the first.
fn aired(entry: &Entry) -> bool {
    matches!(
        entry.status,
        Some(AiringStatus::Releasing | AiringStatus::Finished | AiringStatus::Hiatus)
    ) || entry.next_episode.is_some_and(|n| n > 1)
}

fn entries(meta: &Meta, store: &Store, ids: &[u32]) -> Vec<Entry> {
    let ids: Vec<AnilistId> = ids.iter().map(|id| AnilistId::new(*id)).collect();
    meta.cached(&ids).iter().map(|m| data::entry_from(m, Some(store))).collect()
}

/// What the sources check last found for `id`, while it is a day old or less.
fn playable(store: &Store, id: AnilistId, now: i64) -> Playable {
    match kept::<crate::catalog::home::Probed>(store, &probe_key(id)) {
        Some(probed) if now - probed.at < DAY => {
            if probed.found {
                Playable::Found
            } else {
                Playable::NotFound
            }
        }
        _ => Playable::Unknown,
    }
}

/// What the sources check found, as kept.
#[derive(Serialize, Deserialize)]
pub(crate) struct Probed {
    pub at: i64,
    pub found: bool,
}

pub(crate) fn probe_key(id: AnilistId) -> String {
    format!("probe:{}", id.get())
}

/// The main action for a title that has not been started: play episode 1 when it is out and a
/// source may have it, and otherwise Add to Planning, or nothing when it is on Planning already.
fn first_action(out: bool, playable: Playable, planned: bool) -> HomeAction {
    match (out && playable != Playable::NotFound, planned) {
        (true, _) => HomeAction::Play,
        (false, false) => HomeAction::AddToPlanning,
        (false, true) => HomeAction::Nothing,
    }
}

fn poster(entry: Entry, viewer: &Viewer<'_>, line: Option<Msg>, now: i64) -> HomeItem {
    let playable = playable(viewer_store(viewer), entry.id, now);
    let planned = viewer.status_of(entry.id).as_deref() == Some("PLANNING");
    let out = entry.status != Some(AiringStatus::NotYetReleased);
    let starts = (!out).then_some(entry.start_date).flatten();
    HomeItem {
        action: first_action(out, playable, planned),
        line: if starts.is_some() { None } else { line },
        episode: out.then_some(1),
        resume: None,
        progress: None,
        new: false,
        airs_at: None,
        day: None,
        reason: None,
        starts,
        playable,
        because: None,
        entry,
    }
}

/// The newest episode out, under a poster of a title still airing.
fn newest_out(entry: &Entry) -> Option<Msg> {
    let airing = matches!(entry.status, Some(AiringStatus::Releasing | AiringStatus::Hiatus));
    let (episode, _) = entry.latest_aired().filter(|_| airing)?;
    Some(Msg::HomeEpisodeOutShort { episode: i64::from(episode) })
}

fn viewer_store<'a>(viewer: &Viewer<'a>) -> &'a Store {
    viewer.store()
}

/// A Continue card, when the show has something waiting: an episode left part-way, or one out
/// past the last watched.
fn continue_card(entry: &Entry, store: &Store, now: i64, today: i64) -> Option<HomeItem> {
    let row = QueueItem::of(entry, Casing::Sentence);
    // Watched to the end is finished, not waiting, though the row would offer a rewatch.
    let past_the_end = row.resume.is_none()
        && entry.episodes.is_some_and(|total| entry.progress.is_some_and(|p| p.done >= total));
    if !row.playable || past_the_end {
        return None;
    }
    let episode = i64::from(row.episode);
    let line = match (row.resume, row.fraction, row.aired_ago) {
        (Some(resume), Some(fraction), _) if fraction > 0.0 => {
            let left = resume.position / fraction - resume.position;
            let minutes = ((left / 60.0).round() as i64).max(1);
            Some(Msg::HomeTimeLeft { episode, minutes })
        }
        // Part way through an episode whose length is not known: the action says Resume.
        (Some(_), _, _) => None,
        (None, _, Some(ago)) if now - ago >= today => {
            Some(Msg::HomeEpisodeOutToday { episode })
        }
        (None, _, _) if newest_waiting(entry, row.episode) => {
            Some(Msg::HomeEpisodeOut { episode })
        }
        (None, _, _) => Some(Msg::HomeEpisodeNext { episode }),
    };
    let playable = playable(store, entry.id, now);
    let action = if row.resume.is_some() { HomeAction::Resume } else { HomeAction::Play };
    Some(HomeItem {
        entry: entry.clone(),
        // A show in Continue is the viewer's already: a miss offers its page, never Planning.
        action: home_checked_action(action, playable, true),
        line,
        episode: Some(row.episode),
        resume: row.resume,
        progress: row.fraction,
        new: row.aired_ago.is_some(),
        airs_at: None,
        day: None,
        reason: None,
        starts: None,
        playable,
        because: None,
    })
}

/// Whether `episode` is what a show still airing put out last, so its line may say it is out.
/// For a finished show, or one with more out since, the episode is only next.
fn newest_waiting(entry: &Entry, episode: u32) -> bool {
    let airing = matches!(entry.status, Some(AiringStatus::Releasing | AiringStatus::Hiatus));
    airing && entry.latest_aired().is_some_and(|(latest, _)| latest == episode)
}

/// The shows with something waiting, most recent first, as many as are cached.
fn continue_cards(meta: &Meta, store: &Store, now: i64, today: i64) -> Vec<HomeItem> {
    let ids = continue_ids(store);
    meta.cached(&ids)
        .iter()
        .map(|m| data::entry_from(m, Some(store)))
        .filter_map(|entry| continue_card(&entry, store, now, today))
        .collect()
}

/// The Up next for you items, in Up next's own order, before the shelf is cut: the suggestions
/// kept that are still allowed.
fn suggestions(
    meta: &Meta,
    viewer: &Viewer<'_>,
    part: &PersonalPart,
    now: i64,
) -> Vec<(HomeItem, Entry)> {
    let store = viewer_store(viewer);
    let picks: Vec<&Pick> = part.picks.iter().collect();
    let ids: Vec<u32> = picks.iter().map(|p| p.id).collect();
    let found: HashMap<AnilistId, Entry> =
        entries(meta, store, &ids).into_iter().map(|e| (e.id, e)).collect();
    let mut out = Vec::new();
    for pick in picks {
        let Some(entry) = found.get(&AnilistId::new(pick.id)) else { continue };
        if !viewer.eligible(entry) {
            continue;
        }
        let mut pick = pick.clone();
        pick.planned = viewer.status_of(entry.id).as_deref() == Some("PLANNING");
        let item = up_next::item(&pick, entry.clone());
        let playable = playable(store, entry.id, now);
        let out_now = item.action == UpNextAction::PlayFirstEpisode;
        // The short reason; the full one is the app's, from `because`.
        let line = match item.reason {
            UpNextReason::NewSeason => Some(Msg::HomeReasonNewSeason),
            UpNextReason::Planning => Some(Msg::HomeReasonPlanning),
            UpNextReason::Recommended => Some(Msg::HomeReasonRecommended),
        };
        let home = HomeItem {
            entry: item.entry.clone(),
            action: first_action(out_now, playable, pick.planned),
            line: if item.starts.is_some() { None } else { line },
            episode: out_now.then_some(1),
            resume: None,
            progress: None,
            new: false,
            airs_at: None,
            day: None,
            reason: Some(item.reason),
            starts: item.starts,
            playable,
            because: item.because.clone(),
        };
        out.push((home, item.entry));
    }
    out
}

/// The label over a suggested spotlight: why Up next for you has it.
fn suggested_label(
    meta: &Meta,
    viewer: &Viewer<'_>,
    item: &HomeItem,
    pick: Option<&Pick>,
) -> Msg {
    match item.reason {
        Some(UpNextReason::NewSeason) => {
            // The season before, as the viewer finished it.
            let finished: Vec<AnilistId> = item
                .entry
                .prequels
                .iter()
                .map(|p| p.id)
                .filter(|id| viewer.finished.contains(id))
                .collect();
            match meta.cached(&finished).first() {
                Some(media) => Msg::HomeSpotlightNewSeason {
                    title: meta.titles().pick(&media.title).to_owned(),
                },
                None => Msg::HomeReasonNewSeason,
            }
        }
        Some(UpNextReason::Planning) => Msg::HomeSpotlightPlanning,
        Some(UpNextReason::Recommended) => match pick.and_then(|p| p.because.clone()) {
            Some(title) => Msg::HomeSpotlightBecause { title },
            None => Msg::HomeSpotlightRecommended,
        },
        _ => Msg::HomeSpotlightPopular,
    }
}

/// The popular pick: the first title of `ids` (a season's most popular first) the viewer may be
/// offered that has an episode out.
fn popular(meta: &Meta, viewer: &Viewer<'_>, ids: &[u32]) -> Option<Entry> {
    entries(meta, viewer_store(viewer), ids)
        .into_iter()
        .find(|entry| aired(entry) && viewer.eligible(entry))
}

/// The season's part as kept, when it is this season's under these settings.
/// What a run of the season part is asked from: the season, its year, whether adult titles
/// count, and the genre finished most.
type SeasonInputs = (String, u16, bool, Option<String>);

/// [`SeasonInputs`] now.
fn season_inputs(meta: &Meta, store: &Store, ask: &HomeAsk) -> SeasonInputs {
    let (season, year) = season_on(ask.now, &ask.zone);
    let genre = favourite_genre(meta, store, &viewer(meta, store, ask).finished);
    (season_wire(season).to_owned(), year, ask.adult, genre)
}

fn season_part(store: &Store, ask: &HomeAsk) -> Option<SeasonPart> {
    let (season, year) = season_on(ask.now, &ask.zone);
    kept::<SeasonPart>(store, SEASON_KEY).filter(|part| {
        part.season == season_wire(season) && part.year == year && part.adult == ask.adult
    })
}

/// Nothing of the viewer's own for Home to go on: no title on a signed-in tracker's list and
/// nothing watched but the demo. This week then holds the popular broadcasts and the premieres
/// of every show, and Trending now follows it, after This season.
fn fresh(store: &Store, ask: &HomeAsk) -> bool {
    let own = |id: &AnilistId| *id != kaeri_core::demo::DEMO_ANILIST_ID;
    let listed = ask.listed
        && store.library_statuses().is_ok_and(|rows| rows.iter().any(|(id, _)| own(id)));
    !listed && !week_candidates(store, ask).iter().any(own)
}

fn day_mark(ask: &HomeAsk) -> i64 {
    today(ask.now, &ask.zone).map_or(0, |(start, _)| start)
}

/// Home as it stands: what is local, and every part kept. `loading` is what is on its way.
fn compose(
    meta: &Meta,
    store: &Store,
    ask: &HomeAsk,
    loading: &HashSet<HomePart>,
    continuing: bool,
) -> HomeView {
    let now = ask.now;
    let viewer = viewer(meta, store, ask);
    let mut sections: Vec<HomeSection> = Vec::new();
    let mut pending: Vec<HomePart> = Vec::new();

    // Continue, and the spotlight when something in it is waiting.
    let cards = continue_cards(meta, store, now, day_mark(ask));
    let mut spotlight = cards.first().map(|card| Spotlight {
        case: SpotlightCase::Continuing,
        label: Msg::HomeSpotlightContinuing,
        entry: card.entry.clone(),
        episode: card.episode.unwrap_or(1),
        action: card.action,
        reason: None,
        resume: card.resume,
        starts: None,
        playable: card.playable,
        // The minutes left, that the newest episode is out whenever it came out, or the
        // episode next.
        line: match &card.line {
            Some(Msg::HomeEpisodeOutToday { episode }) => {
                Some(Msg::HomeEpisodeOut { episode: *episode })
            }
            line => line.clone(),
        },
        because: None,
    });
    if !cards.is_empty() {
        sections.push(HomeSection {
            kind: SectionKind::Continue,
            title: Msg::HomeContinue,
            total: cards.len() as u32,
            items: cards.into_iter().take(CONTINUE_SHOWN).collect(),
            view_all: Some(HomeLink::Watching),
        });
    }

    let personal = kept::<PersonalPart>(store, PERSONAL_KEY);
    let season = season_part(store, ask);
    let fresh = fresh(store, ask);
    let personal_waits = personal.is_none() && loading.contains(&HomePart::Suggestions);
    let season_waits = season.is_none() && loading.contains(&HomePart::Season);
    if personal_waits {
        pending.push(HomePart::Suggestions);
    }
    if season_waits {
        pending.push(HomePart::Season);
    }

    let suggested =
        personal.as_ref().map(|p| suggestions(meta, &viewer, p, now)).unwrap_or_default();
    // A title shows on one discovery shelf only: the first that has it.
    let mut seen: HashSet<AnilistId> = HashSet::new();
    let shelf = |items: Vec<HomeItem>, most: usize, seen: &mut HashSet<AnilistId>| {
        let items: Vec<HomeItem> =
            items.into_iter().filter(|item| seen.insert(item.entry.id)).take(most).collect();
        items
    };

    // The next season, in the two weeks before it starts.
    let (this_season, this_year) = season_on(now, &ask.zone);
    let (next, next_year) = next_season(this_season, this_year);
    let starting =
        next_season_starts(now, &ask.zone).is_some_and(|at| at - now <= STARTING_BEFORE);
    if starting && let Some(part) = &season {
        let in_next = |entry: &Entry| {
            let by_season = entry.season == Some(next) && entry.year == Some(next_year);
            let by_date = entry.start_date.is_some_and(|d| {
                d.year == next_year && d.month.is_some_and(|m| season_of_month(m as i8) == next)
            });
            by_season || by_date
        };
        let mut items: Vec<HomeItem> = suggested
            .iter()
            .filter(|(item, entry)| {
                matches!(item.reason, Some(UpNextReason::NewSeason | UpNextReason::Planning))
                    && in_next(entry)
            })
            .map(|(item, _)| item.clone())
            .collect();
        items.extend(
            entries(meta, store, &part.next)
                .into_iter()
                .filter(|e| viewer.eligible(e))
                .map(|e| poster(e, &viewer, None, now)),
        );
        let items = shelf(items, SHELF_SHOWN, &mut seen);
        if !items.is_empty() {
            sections.push(HomeSection {
                kind: SectionKind::Starting,
                title: starting_title(next),
                total: items.len() as u32,
                items,
                view_all: Some(HomeLink::Browse {
                    query: SearchQuery::season(next, next_year),
                }),
            });
        }
    }

    // This week, of the shows followed. With none, the popular week comes after This season.
    if !fresh {
        match kept::<SchedulePart>(store, WEEK_KEY) {
            Some(part) => {
                let items = broadcast_cards(meta, store, &part, ask, true);
                if !items.is_empty() {
                    sections.push(HomeSection {
                        kind: SectionKind::ThisWeek,
                        title: Msg::HomeThisWeek,
                        total: items.len() as u32,
                        items,
                        view_all: Some(HomeLink::CalendarView),
                    });
                }
            }
            None if loading.contains(&HomePart::Week) => pending.push(HomePart::Week),
            None => {}
        }
    }

    // Up next for you.
    let up_next = shelf(
        suggested.iter().map(|(item, _)| item.clone()).collect(),
        SUGGESTIONS_SHOWN,
        &mut seen,
    );
    if !up_next.is_empty() {
        sections.push(HomeSection {
            kind: SectionKind::UpNext,
            title: Msg::HomeUpNext,
            total: up_next.len() as u32,
            items: up_next,
            view_all: None,
        });
    }

    // This season.
    if let Some(part) = &season {
        let items = entries(meta, store, &part.this)
            .into_iter()
            .filter(|e| viewer.eligible(e))
            .map(|e| {
                let line = newest_out(&e);
                poster(e, &viewer, line, now)
            })
            .collect();
        let items = shelf(items, SHELF_SHOWN, &mut seen);
        if !items.is_empty() {
            sections.push(HomeSection {
                kind: SectionKind::ThisSeason,
                title: Msg::HomeThisSeason,
                total: items.len() as u32,
                items,
                view_all: Some(HomeLink::SeasonalView),
            });
        }
    }

    // Because you finished.
    if let Some(rail) = personal.as_ref().and_then(|p| p.because.as_ref()) {
        let items = entries(meta, store, &rail.ids)
            .into_iter()
            .filter(|e| viewer.eligible(e))
            .map(|e| poster(e, &viewer, None, now))
            .collect();
        let items = shelf(items, SUGGESTIONS_SHOWN, &mut seen);
        if !items.is_empty() {
            sections.push(HomeSection {
                kind: SectionKind::Because,
                title: Msg::HomeBecause { title: rail.name.clone() },
                total: items.len() as u32,
                items,
                view_all: None,
            });
        }
    }

    // More of the genre finished most.
    if let Some(rail) = season.as_ref().and_then(|p| p.genre.as_ref()) {
        let items = entries(meta, store, &rail.ids)
            .into_iter()
            .filter(|e| viewer.eligible(e))
            .map(|e| poster(e, &viewer, None, now))
            .collect();
        let items = shelf(items, SHELF_SHOWN, &mut seen);
        if !items.is_empty() {
            sections.push(HomeSection {
                kind: SectionKind::MoreGenre,
                title: Msg::HomeMoreGenre { genre: rail.name.to_lowercase() },
                total: items.len() as u32,
                items,
                view_all: Some(HomeLink::Browse { query: genre_query(&rail.name) }),
            });
        }
    }

    // With nothing of the viewer's own: the week's popular broadcasts and premieres, then what
    // is trending, the titles shown above left out of it.
    if fresh {
        match kept::<SchedulePart>(store, POPULAR_WEEK_KEY)
            .filter(|p| p.from == day_mark(ask) as u64)
        {
            Some(part) => {
                let items = popular_week(meta, store, &part, ask, &viewer);
                if !items.is_empty() {
                    sections.push(HomeSection {
                        kind: SectionKind::ThisWeek,
                        title: Msg::HomeThisWeek,
                        total: items.len() as u32,
                        items,
                        view_all: Some(HomeLink::CalendarView),
                    });
                }
            }
            None if loading.contains(&HomePart::Week) => pending.push(HomePart::Week),
            None => {}
        }
        if let Some(part) = &season {
            let items = entries(meta, store, &part.trending)
                .into_iter()
                .filter(|e| viewer.eligible(e))
                .map(|e| {
                    let line = newest_out(&e);
                    poster(e, &viewer, line, now)
                })
                .collect();
            let items = shelf(items, SHELF_SHOWN, &mut seen);
            if !items.is_empty() {
                sections.push(HomeSection {
                    kind: SectionKind::Trending,
                    title: Msg::HomeTrending,
                    total: items.len() as u32,
                    items,
                    view_all: Some(HomeLink::Browse {
                        query: SearchQuery::sorted(SearchSort::Trending),
                    }),
                });
            }
        }
    }

    // The spotlight, when nothing waits in Continue: the first suggestion, then this season's
    // popular pick. Neither is decided while what decides it is still on its way, so it never
    // shows one and then the other.
    if spotlight.is_none() && !personal_waits && !continuing {
        spotlight = suggested.first().map(|(item, entry)| {
            let pick = personal
                .as_ref()
                .and_then(|p| p.picks.iter().find(|pick| pick.id == entry.id.get()));
            Spotlight {
                case: SpotlightCase::Suggested,
                label: suggested_label(meta, &viewer, item, pick),
                entry: item.entry.clone(),
                episode: 1,
                action: item.action,
                reason: item.reason,
                resume: None,
                starts: item.starts,
                playable: item.playable,
                line: None,
                because: item.because.clone(),
            }
        });
        if spotlight.is_none()
            && !season_waits
            && let Some(part) = &season
        {
            let entry = popular(meta, &viewer, &part.this)
                .or_else(|| popular(meta, &viewer, &part.previous));
            spotlight = entry.map(|entry| {
                let item = poster(entry, &viewer, None, now);
                Spotlight {
                    case: SpotlightCase::Popular,
                    label: Msg::HomeSpotlightPopular,
                    episode: 1,
                    action: item.action,
                    reason: None,
                    resume: None,
                    starts: item.starts,
                    playable: item.playable,
                    line: None,
                    because: None,
                    entry: item.entry,
                }
            });
        }
    }

    // The demo, ahead of the other cases, until it is first played or a store source is in.
    if ask.demo.on && ask.demo.only && !ask.demo.played {
        let media = kaeri_meta::demo::media();
        spotlight = Some(Spotlight {
            case: SpotlightCase::Demo,
            label: Msg::HomeSpotlightDemo,
            entry: data::entry_from(&media, Some(store)),
            episode: 1,
            action: HomeAction::Play,
            reason: None,
            resume: None,
            starts: None,
            playable: Playable::Found,
            line: None,
            because: None,
        });
    }

    // Up next for you leaves out the suggestion the spotlight shows.
    if let Some(spot) = spotlight.as_ref().filter(|s| s.case == SpotlightCase::Suggested) {
        if let Some(section) = sections.iter_mut().find(|s| s.kind == SectionKind::UpNext) {
            section.items.retain(|item| item.entry.id != spot.entry.id);
            section.total = section.items.len() as u32;
        }
        sections.retain(|s| !s.items.is_empty());
    }

    // The end waits for Continue's own titles too, so a front end that reads it as settled has
    // the whole queue.
    let end = (pending.is_empty() && !continuing).then_some(if !ask.sources {
        HomeEnd::NoSources
    } else if ask.listed {
        HomeEnd::Everything
    } else {
        HomeEnd::SignIn
    });
    let kept_any = personal.is_some()
        || season.is_some()
        || kept::<SchedulePart>(store, WEEK_KEY).is_some()
        || kept::<SchedulePart>(store, POPULAR_WEEK_KEY).is_some();
    // The oldest part shown, so the line never makes Home sound fresher than it is.
    let offline_since = (ask.offline && kept_any).then(|| {
        [
            personal.as_ref().map(|p| p.at),
            season.as_ref().map(|p| p.at),
            kept::<SchedulePart>(store, WEEK_KEY).map(|p| p.at),
            kept::<SchedulePart>(store, POPULAR_WEEK_KEY).map(|p| p.at),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(now)
    });
    // The season This season holds, for the apps to word beside its heading.
    let this_season = sections.iter().any(|s| s.kind == SectionKind::ThisSeason).then(|| {
        let (season, year) = season_on(ask.now, &ask.zone);
        SeasonYear { season, year: i32::from(year) }
    });
    HomeView {
        spotlight,
        sections,
        pending,
        end,
        offline: ask.offline,
        offline_since,
        failed: false,
        season: this_season,
    }
}

/// This week's cards, each broadcast in the viewer's day order, or today's, most popular
/// first as they were kept.
fn broadcast_cards(
    meta: &Meta,
    store: &Store,
    part: &SchedulePart,
    ask: &HomeAsk,
    week: bool,
) -> Vec<HomeItem> {
    let Some((start, date)) = today(ask.now, &ask.zone) else { return Vec::new() };
    let end = start + i64::from(WEEK_DAYS) * DAY;
    let ids: Vec<AnilistId> = part.broadcasts.iter().map(|b| AnilistId::new(b.id)).collect();
    let mut known: HashMap<AnilistId, Media> =
        part.media.iter().map(|m| (m.id, m.clone())).collect();
    for media in meta.cached(&ids) {
        known.insert(media.id, media);
    }
    let mut rows: Vec<Broadcast> = part
        .broadcasts
        .iter()
        .copied()
        .filter(|b| !week || (b.at >= start && b.at < end))
        .collect();
    if week {
        rows.sort_by_key(|b| (b.at, b.id));
    }
    rows.into_iter()
        .filter_map(|b| {
            let media = known.get(&AnilistId::new(b.id))?;
            let entry = data::entry_from(media, Some(store));
            let out = b.at <= ask.now;
            let day = jiff::Timestamp::from_second(b.at)
                .ok()
                .map(|t| t.to_zoned(ask.zone.clone()).date())
                .and_then(|d| date.until(d).ok())
                .map(|span| span.get_days().max(0) as u32);
            Some(HomeItem {
                action: if out { HomeAction::Play } else { HomeAction::Nothing },
                line: out.then(|| Msg::HomeOutAgo { ago: Span::countdown(ask.now - b.at) }),
                episode: Some(b.episode),
                resume: None,
                progress: None,
                new: out,
                airs_at: Some(b.at),
                day: if week { day } else { None },
                reason: None,
                starts: None,
                playable: playable(store, entry.id, ask.now),
                because: None,
                entry,
            })
        })
        .collect()
}

/// The popular week as shown: each day's premieres first, then its most popular broadcasts, that
/// may be offered to the viewer, [`POPULAR_WEEK_SHOWN`] a day, in the order they air.
fn popular_week(
    meta: &Meta,
    store: &Store,
    part: &SchedulePart,
    ask: &HomeAsk,
    viewer: &Viewer<'_>,
) -> Vec<HomeItem> {
    let ids: Vec<AnilistId> = part.broadcasts.iter().map(|b| AnilistId::new(b.id)).collect();
    let mut popularity: HashMap<AnilistId, u32> =
        part.media.iter().map(|m| (m.id, m.popularity.unwrap_or(0))).collect();
    for media in meta.cached(&ids) {
        popularity.insert(media.id, media.popularity.unwrap_or(0));
    }
    let mut days: HashMap<u32, Vec<HomeItem>> = HashMap::new();
    for item in broadcast_cards(meta, store, part, ask, true) {
        if viewer.eligible(&item.entry) {
            days.entry(item.day.unwrap_or(0)).or_default().push(item);
        }
    }
    let mut items: Vec<HomeItem> = days
        .into_values()
        .flat_map(|mut day| {
            day.sort_by_key(|item| {
                let rank = popularity.get(&item.entry.id).copied().unwrap_or(0);
                (item.episode != Some(1), std::cmp::Reverse(rank))
            });
            day.truncate(POPULAR_WEEK_SHOWN);
            day
        })
        .collect();
    items.sort_by_key(|item| (item.airs_at, item.entry.id.get()));
    items
}

/// Compose Home and send it, unless it is what was sent last. `failed` marks this send as the
/// one after a part failed.
fn send(
    meta: &Meta,
    store: &Store,
    tx: &Outbox,
    ask: &HomeAsk,
    memory: &HomeMemory,
    failed: bool,
) {
    let mut mem = memory.lock();
    let home = compose(meta, store, ask, &mem.loading, mem.continuing);
    if !failed && mem.last.as_ref() == Some(&home) {
        return;
    }
    mem.last = Some(home.clone());
    let _ = tx.send(Event::Home(Box::new(HomeView { failed, ..home })));
}

/// Send Home again from what is kept, asking nothing: after a dismissal, a restore, or an
/// answer of the sources check. Only once Home has been asked for.
pub fn changed(meta: &Meta, store: &Store, tx: &Outbox, ask: &HomeAsk, memory: &HomeMemory) {
    if memory.asked() {
        send(meta, store, tx, ask, memory, false);
    }
}

/// Which parts are due: those not kept, kept from before the half hour, or kept from what has
/// changed since (the history, the lists, the followed shows, the day, the season).
fn due(meta: &Meta, store: &Store, ask: &HomeAsk) -> HashSet<HomePart> {
    let mut due = HashSet::new();
    let fresh = |at: i64| !ask.refresh && ask.now - at < KEPT_FOR && ask.now >= at;
    let finished = viewer(meta, store, ask).finished;
    let genre = favourite_genre(meta, store, &finished);
    if !season_part(store, ask)
        .is_some_and(|p| fresh(p.at) && p.genre.as_ref().map(|r| &r.name) == genre.as_ref())
    {
        due.insert(HomePart::Season);
    }
    let from = personal_signature(store, ask);
    if !kept::<PersonalPart>(store, PERSONAL_KEY)
        .is_some_and(|p| fresh(p.at) && p.from == from && !p.incomplete)
    {
        due.insert(HomePart::Suggestions);
    }
    if self::fresh(store, ask) {
        let day = day_mark(ask) as u64;
        let kept = kept::<SchedulePart>(store, POPULAR_WEEK_KEY);
        if !kept.is_some_and(|p| fresh(p.at) && p.from == day) {
            due.insert(HomePart::Week);
        }
    } else {
        let from = week_signature(&week_candidates(store, ask), ask);
        if !kept::<SchedulePart>(store, WEEK_KEY).is_some_and(|p| fresh(p.at) && p.from == from)
        {
            due.insert(HomePart::Week);
        }
    }
    due
}

/// Whether part `part` has an answer kept, which Home shows while it is asked again.
fn has_kept(store: &Store, ask: &HomeAsk, part: HomePart) -> bool {
    match part {
        HomePart::Season => season_part(store, ask).is_some(),
        HomePart::Suggestions => kept::<PersonalPart>(store, PERSONAL_KEY).is_some(),
        HomePart::Week if fresh(store, ask) => {
            kept::<SchedulePart>(store, POPULAR_WEEK_KEY).is_some()
        }
        HomePart::Week => kept::<SchedulePart>(store, WEEK_KEY).is_some(),
        HomePart::Today => false,
    }
}

/// Home: at once from what is local and kept, then again as each part due lands.
///
/// A part with nothing kept is asked at once, for the screen. A part kept from before, which
/// Home shows meanwhile, is asked again in the background queue and replaced in place when it
/// lands, unless the viewer asked for a refresh: the launch asks AniList only for what it has
/// no answer to at all. Continue's records likewise: those missing are asked at once, and those
/// kept but old are asked again behind. Once the screen's parts are in, the launch's background
/// work may begin.
pub async fn load(lent: Lent, memory: HomeMemory, ask: HomeAsk) {
    let Lent { meta, store, tx, .. } = lent;
    let mut due = if ask.offline { HashSet::new() } else { due(&meta, &store, &ask) };
    let (missing, old) =
        if ask.offline { (Vec::new(), Vec::new()) } else { stale_continue(&store) };
    let season = due.contains(&HomePart::Season).then(|| season_inputs(&meta, &store, &ask));
    // A season part already on its way from the same inputs brings what this load would ask
    // for, and its end sends Home: this load waits for that end rather than asking twice.
    let mut season_waits = None;
    let mut season_ends = None;
    {
        let mut mem = memory.lock();
        if let Some(inputs) = &season {
            match &mem.season_running {
                Some((running, end)) if running == inputs => {
                    season_waits = Some(end.clone());
                    due.remove(&HomePart::Season);
                }
                _ => {
                    let (end, ended) = tokio::sync::watch::channel(false);
                    mem.season_running = Some((inputs.clone(), ended));
                    season_ends = Some(end);
                }
            }
        }
        mem.asked = true;
        mem.loading.extend(due.iter().copied());
        // Asked for, so sent, even when it is what was sent last: the view may be new.
        mem.last = None;
        mem.continuing |= !missing.is_empty();
    }
    send(&meta, &store, &tx, &ask, &memory, false);
    if ask.offline {
        meta.anilist().launched();
        return;
    }
    let (queued, asked): (HashSet<HomePart>, HashSet<HomePart>) =
        due.into_iter().partition(|part| !ask.refresh && has_kept(&store, &ask, *part));
    let (old, missing) =
        if ask.refresh { (Vec::new(), [missing, old].concat()) } else { (old, missing) };
    let parts = |due: HashSet<HomePart>, continue_ids: Vec<AnilistId>| Parts {
        meta: meta.clone(),
        store: store.clone(),
        tx: tx.clone(),
        ask: ask.clone(),
        memory: memory.clone(),
        season: due.contains(&HomePart::Season).then(|| season.clone()).flatten(),
        due,
        continue_ids,
        continuing: false,
        season_waits: None,
        season_ends: None,
    };
    let mut screen = parts(asked, missing);
    screen.continuing = !screen.continue_ids.is_empty();
    let mut behind = parts(queued, old);
    // The wait for a season part on its way goes with the screen's: it is one.
    screen.season_waits = season_waits;
    if behind.due.contains(&HomePart::Season) {
        behind.season_ends = season_ends;
    } else {
        screen.season_ends = season_ends;
    }
    if !behind.is_empty() {
        tokio::spawn(kaeri_meta::in_background(behind.run()));
    }
    screen.run().await;
    // The screen's parts are in: the launch's background work may begin.
    meta.anilist().launched();
}

/// The parts one pass of a Home load asks for, at the screen's priority or in the background.
struct Parts {
    meta: Meta,
    store: Store,
    tx: Outbox,
    ask: HomeAsk,
    memory: HomeMemory,
    due: HashSet<HomePart>,
    /// Continue's records to ask again.
    continue_ids: Vec<AnilistId>,
    /// Home waits for these, as "not known yet", until they are in.
    continuing: bool,
    /// What the season part is asked from, while this pass asks for it.
    season: Option<SeasonInputs>,
    /// The end of a season part another load is asking for, which this one waits for.
    season_waits: Option<tokio::sync::watch::Receiver<bool>>,
    /// Said when this pass's season part is sent.
    season_ends: Option<tokio::sync::watch::Sender<bool>>,
}

impl Parts {
    fn is_empty(&self) -> bool {
        self.due.is_empty() && self.continue_ids.is_empty() && self.season_waits.is_none()
    }

    /// Ask for every part, each sent as it lands.
    async fn run(self) {
        let Self {
            meta,
            store,
            tx,
            ask,
            memory,
            due,
            continue_ids,
            continuing,
            season,
            season_waits,
            season_ends,
        } = self;
        let finish = |part: Option<HomePart>, outcome: Result<(), String>| {
            if let Err(e) = &outcome {
                tracing::warn!(error = %e, ?part, "part of Home failed");
            }
            match part {
                Some(part) => {
                    memory.lock().loading.remove(&part);
                }
                None if continuing => memory.lock().continuing = false,
                None => {}
            }
            send(&meta, &store, &tx, &ask, &memory, outcome.is_err());
            outcome.is_ok()
        };
        // A part that fails, or comes back short of a lookup, is not given up on: what is kept
        // is on screen already, and the part is asked for again after AniList's own wait, a
        // few times. A short part is sent as it is, and again when it is whole.
        let partial = || send(&meta, &store, &tx, &ask, &memory, false);
        let wanted = |part| due.contains(&part);
        tokio::join!(
            async {
                if continue_ids.is_empty() {
                    return true;
                }
                let run =
                    || async { refresh_continue(&meta, &continue_ids).await.map(|()| true) };
                finish(None, retried(&meta, run, partial).await)
            },
            async {
                if let Some(mut end) = season_waits {
                    // Sent by the load that asked; a load that went away ends the wait too.
                    let _ = end.wait_for(|sent| *sent).await;
                    return true;
                }
                if !wanted(HomePart::Season) {
                    return true;
                }
                let deeper = std::sync::Mutex::new(None);
                let run = || async {
                    let shelves = load_season(&meta, &store, &ask).await?;
                    *deeper.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = shelves;
                    Ok(true)
                };
                let outcome = retried(&meta, run, partial).await;
                {
                    let mut mem = memory.lock();
                    if mem.season_running.as_ref().map(|(inputs, _)| inputs) == season.as_ref()
                    {
                        mem.season_running = None;
                    }
                }
                let whole = finish(Some(HomePart::Season), outcome);
                if let Some(end) = season_ends {
                    let _ = end.send(true);
                }
                // Further down the shelves' order, behind whatever is on screen: the shelves
                // are shown from their first page meanwhile, and filled out in place.
                let deeper =
                    deeper.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(deeper) = deeper {
                    let (meta, store, tx, ask, memory) =
                        (meta.clone(), store.clone(), tx.clone(), ask.clone(), memory.clone());
                    tokio::spawn(kaeri_meta::in_background(async move {
                        if deepen(&meta, &store, &ask, deeper).await {
                            send(&meta, &store, &tx, &ask, &memory, false);
                        }
                    }));
                }
                whole
            },
            async {
                if !wanted(HomePart::Suggestions) {
                    return true;
                }
                let run = || load_personal(&meta, &store, &ask);
                finish(Some(HomePart::Suggestions), retried(&meta, run, partial).await)
            },
            async {
                if !wanted(HomePart::Week) {
                    return true;
                }
                let run = || async {
                    if fresh(&store, &ask) {
                        load_popular_week(&meta, &store, &ask).await.map(|()| true)
                    } else {
                        load_week(&meta, &store, &ask).await.map(|()| true)
                    }
                };
                finish(Some(HomePart::Week), retried(&meta, run, partial).await)
            },
        );
    }
}

/// Run a part until it is whole: `run` says whether it was, a failure or a short answer is run
/// again after the wait, and `partial` says what a short answer found. After the last go, a
/// short answer stands and a failure is the part's.
async fn retried<F, Fut>(meta: &Meta, mut run: F, partial: impl Fn()) -> Result<(), String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<bool, String>>,
{
    let mut tries = 0;
    loop {
        // The first go is the screen's; the goes after a wait are background work, behind
        // whatever is on screen by then.
        let outcome =
            if tries == 0 { run().await } else { kaeri_meta::in_background(run()).await };
        match outcome {
            Ok(true) => return Ok(()),
            Ok(false) if tries >= crate::catalog::RETRIES => return Ok(()),
            Ok(false) => partial(),
            Err(e) if tries >= crate::catalog::RETRIES => return Err(e),
            Err(e) => tracing::warn!(error = %e, "part of Home failed; asking again"),
        }
        tries += 1;
        tokio::time::sleep(crate::catalog::retry_wait(meta).await).await;
    }
}

/// Continue's titles whose cached record is missing or old: their airing is what says an
/// episode is waiting.
/// Split into those with no record kept, which Home asks for at once, and those kept but old,
/// which it asks again behind.
fn stale_continue(store: &Store) -> (Vec<AnilistId>, Vec<AnilistId>) {
    stale_among(store, continue_ids(store))
        .into_iter()
        .partition(|id| store.cached_media(*id).ok().flatten().is_none())
}

/// The titles among `ids` whose cached record is missing or older than [`CONTINUE_FRESH`].
pub(crate) fn stale_among(store: &Store, ids: Vec<AnilistId>) -> Vec<AnilistId> {
    let wall = kaeri_store::now();
    ids.into_iter()
        .filter(|id| {
            store
                .cached_media(*id)
                .ok()
                .flatten()
                .is_none_or(|(_, at)| wall - at >= CONTINUE_FRESH)
        })
        .collect()
}

/// Continue's stale records, asked again in one request.
async fn refresh_continue(meta: &Meta, stale: &[AnilistId]) -> Result<(), String> {
    meta.media_many(stale).await.map(|_| ()).map_err(|e| e.to_string())
}

/// The season's rails: Search's, in one request with the taste rail beside them, and the
/// season before when nothing of this one has aired. This season and what is trending are asked
/// fifty deep, AniList's page, so their shelves fill for a viewer who follows much of the
/// season. When they still fall short, what to ask further down their order comes back, for
/// [`deepen`] to ask behind the screen.
async fn load_season(
    meta: &Meta,
    store: &Store,
    ask: &HomeAsk,
) -> Result<Option<Deeper>, String> {
    let (season, year) = season_on(ask.now, &ask.zone);
    let finished = viewer(meta, store, ask).finished;
    let genre = favourite_genre(meta, store, &finished);
    let extra: Vec<SearchQuery> = genre.iter().map(|g| genre_query(g)).collect();
    let (rails, extra) =
        browse::fetch_rails(meta, store, ask.adult, ask.now, (season, year), &extra).await?;
    let ids = |media: Option<&Vec<Media>>| -> Vec<u32> {
        media.map(|m| m.iter().map(|m| m.id.get()).collect()).unwrap_or_default()
    };
    let mut part = SeasonPart {
        at: ask.now,
        season: season_wire(season).to_owned(),
        year,
        adult: ask.adult,
        this: ids(rails.get(1)),
        next: ids(rails.get(2)),
        previous: Vec::new(),
        genre: genre.map(|name| Rail { name, ids: ids(extra.first()) }),
        trending: ids(rails.first()),
    };
    // Until a title of the new season has an episode out, the pick stays with the season
    // before, which this asks for once.
    keep(store, SEASON_KEY, &part);
    if popular(meta, &viewer(meta, store, ask), &part.this).is_none() {
        let (before, before_year) = previous_season(season, year);
        let query = SearchQuery::season(before, before_year);
        let filter = crate::catalog::search::browse_filter(&query, ask.adult);
        let found =
            meta.find("", &filter, 1, PREVIOUS_ASKED).await.map_err(|e| e.to_string())?;
        part.previous = found.page.items.iter().map(|m| m.id.get()).collect();
        keep(store, SEASON_KEY, &part);
    }
    // A first page short of AniList's whole page was the whole order already.
    let queries = browse::rails(season, year);
    let full =
        |at: usize| rails.get(at).is_some_and(|r| r.len() >= browse::SHELF_PAGE as usize);
    let shelves: Vec<(Shelf, SearchQuery)> = [(Shelf::This, 1), (Shelf::Trending, 0)]
        .into_iter()
        .filter(|(_, at)| full(*at))
        .filter_map(|(shelf, at)| Some((shelf, queries.get(at)?.2.clone())))
        .collect();
    Ok((!shelves.is_empty()).then_some(Deeper { at: part.at, shelves }))
}

/// One of the shelves [`deepen`] fills out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shelf {
    This,
    Trending,
}

/// What [`deepen`] asks further down: the shelves of the season part kept at `at`.
struct Deeper {
    at: i64,
    shelves: Vec<(Shelf, SearchQuery)>,
}

/// Asks further down the shelves' order, every shelf's next page in one request, until each
/// holds [`DEEPER_WANTED`] titles Home may show, its order runs out, or [`DEEPER_PAGES`] pages
/// were asked in all. Updates the season part kept, if it is still the one asked from, and says
/// whether it did. A page that fails ends it with what was found.
async fn deepen(meta: &Meta, store: &Store, ask: &HomeAsk, deeper: Deeper) -> bool {
    let Some(mut part) = season_part(store, ask).filter(|p| p.at == deeper.at) else {
        return false;
    };
    let who = viewer(meta, store, ask);
    let shown =
        |ids: &[u32]| entries(meta, store, ids).iter().filter(|e| who.eligible(e)).count();
    let filters: Vec<(Shelf, kaeri_meta::BrowseFilter)> = deeper
        .shelves
        .iter()
        .map(|(shelf, query)| (*shelf, crate::catalog::search::browse_filter(query, ask.adult)))
        .collect();
    let mut open: Vec<Shelf> = filters.iter().map(|(shelf, _)| *shelf).collect();
    let mut changed = false;
    for page in 2..=DEEPER_PAGES {
        open.retain(|shelf| {
            let ids = if *shelf == Shelf::This { &part.this } else { &part.trending };
            shown(ids) < DEEPER_WANTED
        });
        if open.is_empty() {
            break;
        }
        let asked: Vec<(&str, &kaeri_meta::BrowseFilter, u32, u32)> = filters
            .iter()
            .filter(|(shelf, _)| open.contains(shelf))
            .map(|(_, filter)| ("", filter, page, browse::SHELF_PAGE))
            .collect();
        let found = match meta.browse_pages(&asked).await {
            Ok(found) => found,
            Err(e) => {
                tracing::info!(error = %e, page, "the shelves could not ask further down");
                break;
            }
        };
        let mut more = Vec::new();
        for (shelf, found) in open.iter().zip(found) {
            let ids = if *shelf == Shelf::This { &mut part.this } else { &mut part.trending };
            for media in &found.items {
                if !ids.contains(&media.id.get()) {
                    ids.push(media.id.get());
                    changed = true;
                }
            }
            if found.has_next {
                more.push(*shelf);
            }
        }
        open = more;
    }
    // Kept only over the part it was asked from: a newer load's answer stands.
    if changed && season_part(store, ask).is_some_and(|p| p.at == deeper.at) {
        keep(store, SEASON_KEY, &part);
        return true;
    }
    false
}

/// What the suggestions depend on, for telling kept ones from stale: the history, the lists,
/// Continue and the settings. Not the dismissals, which are applied as Home is composed.
fn personal_signature(store: &Store, ask: &HomeAsk) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ask.listed.hash(&mut hasher);
    ask.adult.hash(&mut hasher);
    continue_ids(store).hash(&mut hasher);
    for p in store.watched_titles(60).unwrap_or_default() {
        (p.anilist_id, p.episodes_done).hash(&mut hasher);
    }
    if ask.listed {
        for status in ["COMPLETED", "PLANNING"] {
            store.library_with_status(status).unwrap_or_default().hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Up next for you, as Home has it: the suggestions with nothing airing today in their place,
/// and "Because you finished" from the most recent title finished that rated well.
async fn load_personal(meta: &Meta, store: &Store, ask: &HomeAsk) -> Result<bool, String> {
    let from = personal_signature(store, ask);
    let queue: Vec<AnilistId> =
        store.continue_list(15).unwrap_or_default().iter().map(|p| p.anilist_id).collect();
    let anything = store.watched_titles(1).is_ok_and(|w| !w.is_empty())
        || (ask.listed
            && ["COMPLETED", "PLANNING"]
                .iter()
                .any(|s| store.library_with_status(s).is_ok_and(|ids| !ids.is_empty())));
    if !anything {
        keep_personal(
            store,
            ask,
            &PersonalPart { at: ask.now, from, ..PersonalPart::default() },
        );
        return Ok(true);
    }
    let up_ask = up_next::Ask {
        listed: ask.listed,
        adult: ask.adult,
        now: ask.now,
        zone: ask.zone.clone(),
    };
    // Every dismissal is left in, and taken out as Home is composed, so an Undo needs no request.
    // "Because you finished" is asked with the suggestions, in their requests.
    let because = up_next::Because { score: BECAUSE_SCORE, each: BECAUSE_ASKED };
    let found =
        up_next::find(meta, store, &queue, &[], &up_ask, SUGGESTIONS_FOUND, Some(because))
            .await?;
    let mut complete = found.complete;
    let because = match found.because {
        Some((source, Ok(media))) => Some(Rail {
            name: source.title.clone(),
            ids: media.iter().map(|m| m.id.get()).collect(),
        }),
        Some((_, Err(e))) => {
            complete = false;
            tracing::warn!(error = %e, "Because you finished failed");
            None
        }
        None => None,
    };
    let part = PersonalPart {
        at: ask.now,
        from,
        picks: found.picks.into_iter().map(|(pick, _)| pick).collect(),
        because,
        finished: found.finished_ids.iter().map(|id| id.get()).collect(),
        incomplete: !complete,
    };
    // Not kept for a newer run's answer, this one is not asked again: the newer one stands.
    Ok(!keep_personal(store, ask, &part) || complete)
}

/// Keep `part` unless the history or the lists changed while it was found and a run since has
/// kept what they are now: two runs a launch makes, before the pull and after it, can share
/// their last request and land together, and the later inputs' answer is the one Home shows.
/// Whether `part` was kept.
fn keep_personal(store: &Store, ask: &HomeAsk, part: &PersonalPart) -> bool {
    static KEEPING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = KEEPING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = personal_signature(store, ask);
    if part.from != now
        && kept::<PersonalPart>(store, PERSONAL_KEY).is_some_and(|k| k.from == now)
    {
        return false;
    }
    keep(store, PERSONAL_KEY, part);
    true
}

/// The shows that may be followed: Watching on the tracker's list, and the history. What This
/// week is kept for: a change to them asks for the week again.
fn week_candidates(store: &Store, ask: &HomeAsk) -> Vec<AnilistId> {
    let mut ids: Vec<AnilistId> = if ask.listed {
        store.library_with_status("CURRENT").unwrap_or_default()
    } else {
        Vec::new()
    };
    let history = store.watched_titles(CONTINUE_LOOKED_AT).unwrap_or_default();
    for id in history.iter().map(|p| p.anilist_id).chain(continue_ids(store)) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// The shows followed: Watching on the tracker's list, and the shows of the history that air
/// or aired an episode in the last three weeks. A show of the history whose record is not
/// cached is asked about too, as the window leaves out one with nothing in it.
fn followed(
    meta: &Meta,
    store: &Store,
    ask: &HomeAsk,
    candidates: &[AnilistId],
) -> Vec<AnilistId> {
    let listed: HashSet<AnilistId> = if ask.listed {
        store.library_with_status("CURRENT").unwrap_or_default().into_iter().collect()
    } else {
        HashSet::new()
    };
    let cached: HashMap<AnilistId, Media> =
        meta.cached(candidates).into_iter().map(|m| (m.id, m)).collect();
    let cutoff = ask.now - FOLLOWED_FOR;
    candidates
        .iter()
        .copied()
        .filter(|id| {
            if listed.contains(id) {
                return true;
            }
            let Some(media) = cached.get(id) else { return true };
            let entry = data::entry_from(media, None);
            let airs =
                matches!(entry.status, Some(AiringStatus::Releasing | AiringStatus::Hiatus))
                    || media.next_airing_episode.is_some();
            let ended_lately = entry.end_date.is_some_and(|d| {
                let (month, day) = (d.month.unwrap_or(12), d.day.unwrap_or(28));
                jiff::civil::date(d.year as i16, month as i8, day as i8)
                    .to_zoned(ask.zone.clone())
                    .is_ok_and(|z| z.timestamp().as_second() >= cutoff)
            });
            airs || ended_lately
        })
        .collect()
}

fn week_signature(followed: &[AnilistId], ask: &HomeAsk) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut ids: Vec<u32> = followed.iter().map(|id| id.get()).collect();
    ids.sort_unstable();
    ids.hash(&mut hasher);
    day_mark(ask).hash(&mut hasher);
    hasher.finish()
}

fn schedule_part(
    page: &[kaeri_meta::anilist::AiringEntry],
    meta: &Meta,
    at: i64,
    from: u64,
) -> SchedulePart {
    let ids: Vec<AnilistId> = page.iter().map(|a| a.media.id).collect();
    let cached: HashSet<AnilistId> = meta.cached(&ids).iter().map(|m| m.id).collect();
    let mut media: Vec<Media> = Vec::new();
    for airing in page {
        if !cached.contains(&airing.media.id) && !media.iter().any(|m| m.id == airing.media.id)
        {
            media.push(airing.media.clone());
        }
    }
    SchedulePart {
        at,
        from,
        broadcasts: page
            .iter()
            .map(|a| Broadcast { id: a.media.id.get(), episode: a.episode, at: a.airing_at })
            .collect(),
        media,
    }
}

/// This week's broadcasts of the shows followed, from the start of today: one request.
async fn load_week(meta: &Meta, store: &Store, ask: &HomeAsk) -> Result<(), String> {
    let candidates = week_candidates(store, ask);
    let from = week_signature(&candidates, ask);
    let followed = followed(meta, store, ask, &candidates);
    let start = day_mark(ask);
    let page = if followed.is_empty() {
        Vec::new()
    } else {
        meta.aired_since(&followed, start - 1, start + i64::from(WEEK_DAYS) * DAY)
            .await
            .map_err(|e| e.to_string())?
            .items
    };
    keep(store, WEEK_KEY, &schedule_part(&page, meta, ask.now, from));
    Ok(())
}

/// The week's broadcasts of every show, from the start of today, for a viewer with nothing of
/// their own to follow: up to [`POPULAR_WEEK_PAGES`] requests. Each day keeps its premieres and
/// its most popular broadcasts, one a show, [`POPULAR_WEEK_KEPT`] at most.
async fn load_popular_week(meta: &Meta, store: &Store, ask: &HomeAsk) -> Result<(), String> {
    let start = day_mark(ask);
    let end = start + i64::from(WEEK_DAYS) * DAY;
    let mut rows = Vec::new();
    for page in 1..=POPULAR_WEEK_PAGES {
        let found =
            meta.airing_between(start - 1, end, page, 50).await.map_err(|e| e.to_string())?;
        rows.extend(found.items);
        if !found.has_next {
            break;
        }
    }
    // What the viewer may not be shown is left out before each day keeps its best, so a day
    // never runs short for titles the strip would drop.
    let who = viewer(meta, store, ask);
    let mut days: HashMap<u32, Vec<kaeri_meta::anilist::AiringEntry>> = HashMap::new();
    for row in rows {
        let Some(day) = day_of(row.airing_at, ask) else { continue };
        if !who.eligible(&data::entry_from(&row.media, Some(store))) {
            continue;
        }
        let shows = days.entry(day).or_default();
        if !shows.iter().any(|kept| kept.media.id == row.media.id) {
            shows.push(row);
        }
    }
    let mut kept: Vec<_> = days
        .into_values()
        .flat_map(|mut shows| {
            shows.sort_by_key(|a| (a.episode != 1, std::cmp::Reverse(a.media.popularity)));
            shows.truncate(POPULAR_WEEK_KEPT);
            shows
        })
        .collect();
    kept.sort_by_key(|a| (a.airing_at, a.media.id.get()));
    keep(store, POPULAR_WEEK_KEY, &schedule_part(&kept, meta, ask.now, start as u64));
    Ok(())
}

/// The day of the week strip `at` falls on, today being 0, in the viewer's zone.
fn day_of(at: i64, ask: &HomeAsk) -> Option<u32> {
    let (_, today) = today(ask.now, &ask.zone)?;
    let date = jiff::Timestamp::from_second(at).ok()?.to_zoned(ask.zone.clone()).date();
    let days = today.until(date).ok()?.get_days();
    (0..WEEK_DAYS as i32).contains(&days).then_some(days as u32)
}

/// The titles hidden with Not interested that are still hidden, the most recent first: for
/// Settings, Hidden titles.
pub async fn hidden_titles(meta: &Meta, store: &Store, tx: &Outbox) {
    let dismissed = filter::dismissed(store);
    let ids: Vec<AnilistId> = dismissed.iter().rev().map(|d| AnilistId::new(d.id)).collect();
    let mut known: HashMap<AnilistId, Media> =
        meta.cached(&ids).into_iter().map(|m| (m.id, m)).collect();
    let missing: Vec<AnilistId> =
        ids.iter().copied().filter(|id| !known.contains_key(id)).collect();
    if !missing.is_empty() {
        match meta.media_many(&missing).await {
            Ok(found) => known.extend(found.into_iter().map(|m| (m.id, m))),
            Err(e) => tracing::warn!(error = %e, "hidden titles could not be looked up"),
        }
    }
    let viewer = Viewer::new(store, false, true, [], HashSet::new(), dismissed.clone());
    let rows = dismissed
        .iter()
        .rev()
        .filter_map(|d| {
            let media = known.get(&AnilistId::new(d.id))?;
            let entry = data::entry_from(media, Some(store));
            viewer.hidden(&entry).then_some(HiddenTitle {
                until_airs: d.waiting,
                hidden_at: d.at,
                entry,
            })
        })
        .collect();
    let _ = tx.send(Event::HiddenTitles(rows));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_found_from_older_lists_do_not_replace_newer_ones() {
        let store = Store::open_in_memory().unwrap();
        let ask = HomeAsk {
            listed: true,
            adult: false,
            now: 1_000,
            zone: jiff::tz::TimeZone::UTC,
            offline: false,
            refresh: false,
            sources: true,
            demo: Default::default(),
        };
        let part = |from: u64, id: u32| PersonalPart {
            at: ask.now,
            from,
            picks: Vec::new(),
            because: Some(Rail { name: String::new(), ids: vec![id] }),
            finished: Vec::new(),
            incomplete: false,
        };
        let kept_rail = || kept::<PersonalPart>(&store, PERSONAL_KEY).and_then(|p| p.because);
        // Before the pull: found from an empty list, and kept while the lists stay so.
        let before = personal_signature(&store, &ask);
        assert!(keep_personal(&store, &ask, &part(before, 1)));
        // The pull lands; the run after it keeps what it found from the list.
        store
            .replace_library_statuses("anilist", &[(AnilistId::new(7), "COMPLETED")], 1_000)
            .unwrap();
        let after = personal_signature(&store, &ask);
        assert_ne!(before, after);
        assert!(keep_personal(&store, &ask, &part(after, 2)));
        // The run from before the pull lands last, and the newer answer stays.
        assert!(!keep_personal(&store, &ask, &part(before, 1)));
        assert_eq!(kept_rail().map(|r| r.ids), Some(vec![2]));
    }

    #[test]
    fn because_you_finished_asks_within_anilists_complexity() {
        // Its ids ride with Up next's other lookups at that step, at the most each asks.
        let most = kaeri_meta::anilist::query::RECOMMENDED_FROM;
        let ids: Vec<u32> = (1..=50).collect();
        let parts = [
            (
                kaeri_meta::anilist::query::by_ids(),
                serde_json::json!({ "ids": ids, "perPage": 50 }),
            ),
            (
                kaeri_meta::anilist::query::recommended_from(),
                serde_json::json!({ "ids": (0..most).collect::<Vec<_>>(), "perPage": most }),
            ),
            (
                kaeri_meta::anilist::query::recommended_ids(),
                serde_json::json!({ "ids": [1], "perPage": 1, "each": BECAUSE_ASKED }),
            ),
        ];
        let (document, variables) = kaeri_meta::anilist::query::together(&parts);
        let cost = kaeri_testkit::complexity(&document, &variables);
        assert!(cost <= kaeri_testkit::COMPLEXITY_LIMIT / 2, "{cost}");
    }
}
