//! Search: one query for typing, filtering and browsing, answered a page at a time.
//!
//! Every search goes to AniList as one request with every filter applied there, so a wide
//! filter surface costs nothing extra against the 30-a-minute budget. What keeps live search
//! inside that budget is here: a search waits until the changes stop, a newer one replaces
//! one still waiting (which spends nothing, as the limiter only takes a token when a request
//! goes), answers are kept for a while, and when the budget runs low a change of text waits
//! for Return.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kaeri_core::{ids::AnilistId, traits::ProviderKind};
use kaeri_providers::ProviderRegistry;

use kaeri_meta::{BrowseFilter, Found, Media, anilist::Page};
use kaeri_model::{
    catalog::{
        AiringStatus, AvailableAt, ContentFor, ContentList, Entry, Season, SourceMaterial,
    },
    event::Event,
    search::{Available, MediaFormat, OnList, SearchQuery, SearchSort},
};
use tokio::task::AbortHandle;

use crate::{
    catalog::lists::{follow_aired, publish_list},
    data,
    dispatch::{Lent, lend, spawn_abortable},
    jobs::JobGuard,
    now_epoch,
    outbox::Outbox,
    services::Services,
    state::EngineState,
};

/// Titles a page of results holds.
pub const PAGE: u32 = 30;
/// Answers kept, by query and page.
const KEPT: usize = 64;
/// How long a kept answer serves.
const KEPT_FOR: i64 = 10 * 60;
/// How long live search waits after the text changed, and after a filter did.
const AFTER_TYPING: Duration = Duration::from_millis(400);
const AFTER_FILTERING: Duration = Duration::from_millis(250);
/// Below this many requests left in the budget, a change of text waits for Return.
const LIVE_FLOOR: f64 = 6.0;

/// What search keeps between commands: the answers, the search waiting or running, and
/// whether live search is waiting for Return. Shared with the tasks it starts.
#[derive(Clone, Default)]
pub struct Searches {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    kept: Vec<Kept>,
    running: Option<AbortHandle>,
    last_live: Option<SearchQuery>,
    paused: bool,
}

struct Kept {
    key: Key,
    at: i64,
    found: Found,
}

#[derive(Clone, PartialEq, Eq)]
struct Key {
    query: SearchQuery,
    adult: bool,
    page: u32,
}

impl Searches {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Stop the search waiting or running, and keep `next` in its place.
    fn replace(&self, next: Option<AbortHandle>) {
        if let Some(previous) = std::mem::replace(&mut self.lock().running, next) {
            previous.abort();
        }
    }

    fn recall(&self, key: &Key, now: i64) -> Option<Found> {
        let mut inner = self.lock();
        inner.kept.retain(|kept| now - kept.at < KEPT_FOR);
        let at = inner.kept.iter().position(|kept| kept.key == *key)?;
        // Most recently used last, so the first is the one to let go.
        let kept = inner.kept.remove(at);
        let found = kept.found.clone();
        inner.kept.push(kept);
        Some(found)
    }

    fn keep(&self, key: Key, found: &Found, now: i64) {
        let mut inner = self.lock();
        inner.kept.retain(|kept| kept.key != key);
        if inner.kept.len() >= KEPT {
            inner.kept.remove(0);
        }
        inner.kept.push(Kept { key, at: now, found: found.clone() });
    }

    /// Say whether live search waits for Return, when that changed.
    fn pause(&self, paused: bool, tx: &Outbox) {
        let changed = std::mem::replace(&mut self.lock().paused, paused) != paused;
        if changed {
            let _ = tx.send(Event::LiveSearch { paused });
        }
    }
}

/// The query as it is searched for and kept: its text without the spaces around it.
fn normal(query: &SearchQuery) -> SearchQuery {
    SearchQuery { text: query.trimmed_text().to_owned(), ..query.clone() }
}

/// Search now, replacing any search waiting or running, and keep it among the recent ones.
pub fn submit(
    query: SearchQuery,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let adult = engine.config.effective().meta.adult;
    let searches = engine.search.clone();
    let query = normal(&query);
    if !query.is_blank() {
        crate::catalog::recents::remember(query.clone(), services, tx);
    }
    searches.lock().last_live = Some(query.clone());
    let running = searches.clone();
    let handle = spawn_abortable(job, services, tx, move |lent| async move {
        first_page(lent, &running, query, adult).await;
    });
    searches.replace(Some(handle));
}

/// Search as the query changes, once the changes stop.
pub fn live(query: SearchQuery, engine: &mut EngineState, services: &Services, tx: &Outbox) {
    let adult = engine.config.effective().meta.adult;
    let searches = engine.search.clone();
    let query = normal(&query);
    let text_changed = {
        let mut inner = searches.lock();
        let changed = inner.last_live.as_ref().is_none_or(|last| last.text != query.text);
        inner.last_live = Some(query.clone());
        changed
    };
    // Nothing to search for: Search's start shows, or the text is too short to mean much yet.
    if query.is_blank() || query.waits_for_words() {
        searches.replace(None);
        return;
    }
    let kept = searches.recall(&Key { query: query.clone(), adult, page: 1 }, now_epoch());
    let lent = lend(services, tx);
    let running = searches.clone();
    let task = tokio::spawn(async move {
        if kept.is_none() {
            tokio::time::sleep(if text_changed { AFTER_TYPING } else { AFTER_FILTERING }).await;
            if text_changed && lent.meta.anilist().budget_left().await < LIVE_FLOOR {
                running.pause(true, &lent.tx);
                return;
            }
        }
        running.pause(false, &lent.tx);
        first_page(lent, &running, query, adult).await;
    });
    searches.replace(Some(task.abort_handle()));
}

/// The first page of `query`: whether there is more, any caption, then the titles.
async fn first_page(lent: Lent, searches: &Searches, query: SearchQuery, adult: bool) {
    let view = ContentFor::Search { query: query.clone() };
    let query_text = query.text.clone();
    let query_text = query_text.as_str();
    match find(&lent, searches, &query, adult, 1).await {
        Ok(found) => {
            let _ = lent.tx.send(Event::SearchPaging { query, has_next: found.page.has_next });
            // Sent every time, empty when nothing was left out, so a caption from an earlier
            // answer to the same search cannot outlive it.
            let _ = lent.tx.send(Event::Caption { view: view.clone(), text: caption(&found) });
            let mut entries = entries(&lent, &found.page);
            // The demo's title leads the results when the query is one it answers to.
            if lent.registry.get(kaeri_core::demo::DEMO_SOURCE_ID).is_some()
                && kaeri_providers::demo::answers_to(query_text)
            {
                let demo = data::entry_from(&kaeri_meta::demo::media(), Some(&lent.store));
                entries.insert(0, demo);
            }
            publish_list(&lent.meta, &lent.tx, view, entries, now_epoch()).await;
        }
        Err(e) => {
            let _ = lent.tx.send(Event::Content { view, content: ContentList::Failed(e) });
        }
    }
}

/// A further page of the results on screen.
pub async fn more(lent: Lent, searches: Searches, query: SearchQuery, page: u32, adult: bool) {
    let query = normal(&query);
    let page = page.max(2);
    match find(&lent, &searches, &query, adult, page).await {
        Ok(found) => {
            let entries = entries(&lent, &found.page);
            let has_next = found.page.has_next;
            let _ = lent.tx.send(Event::SearchMore {
                query,
                page,
                entries: entries.clone(),
                has_next,
            });
            follow_aired(&lent.meta, &lent.tx, &entries, now_epoch()).await;
        }
        Err(e) => {
            // Empty, with more to come: the next scroll asks for the same page again.
            tracing::warn!(error = %e, page, "a further page of search results failed");
            let _ = lent.tx.send(Event::SearchMore {
                query,
                page,
                entries: Vec::new(),
                has_next: true,
            });
        }
    }
}

async fn find(
    lent: &Lent,
    searches: &Searches,
    query: &SearchQuery,
    adult: bool,
    page: u32,
) -> Result<Found, String> {
    let key = Key { query: normal(query), adult, page };
    // Available reads the viewer's sources and services, which change under a kept answer.
    let keeps = query.available.is_none();
    if keeps && let Some(found) = searches.recall(&key, now_epoch()) {
        return Ok(found);
    }
    let mut filter = browse_filter(query, adult);
    let own = match query.available {
        Some(Available::PlaysHere) => {
            let own = own_titles(&lent.registry);
            let mut ids: Vec<u32> = own.keys().map(|id| id.get()).collect();
            if ids.is_empty() {
                return Ok(nothing());
            }
            ids.sort_unstable();
            filter.ids = ids;
            Some(own)
        }
        Some(Available::OnYourServices) => {
            let services = crate::licensed::settings(&lent.store).services;
            let uses = |s: &String| services.iter().any(|u| u.eq_ignore_ascii_case(s));
            let wanted: Vec<String> = if filter.licensed_by.is_empty() {
                services.clone()
            } else {
                filter.licensed_by.iter().filter(|s| uses(s)).cloned().collect()
            };
            if wanted.is_empty() {
                return Ok(nothing());
            }
            filter.licensed_by = wanted;
            None
        }
        None => None,
    };
    let mut found = lent
        .meta
        .find(query.trimmed_text(), &filter, page, PAGE)
        .await
        .map_err(|e| e.to_string())?;
    // The fallback cannot ask for titles by id, so it is kept to them here.
    if let Some(own) = own
        && !found.unapplied.is_empty()
    {
        found.page.items.retain(|m| own.contains_key(&m.id));
    }
    // An answer from the fallback is not kept: AniList may be back by the next search.
    if keeps && found.unapplied.is_empty() {
        searches.keep(key, &found, now_epoch());
    }
    Ok(found)
}

fn entries(lent: &Lent, page: &Page<Media>) -> Vec<Entry> {
    let own = own_titles(&lent.registry);
    let services = crate::licensed::settings(&lent.store);
    page.items
        .iter()
        .map(|m| {
            let mut entry = data::entry_from(m, Some(&lent.store));
            entry.available_at = own.get(&m.id).cloned().or_else(|| {
                let service = entry.available_on.iter().find(|s| services.uses(s))?;
                Some(AvailableAt::OnService { service: service.clone() })
            });
            entry
        })
        .collect()
}

/// Where each title the viewer's own sources have plays, by AniList id: the first such source
/// in source order. Plugins and web sources never count.
fn own_titles(registry: &ProviderRegistry) -> HashMap<AnilistId, AvailableAt> {
    let mut out = HashMap::new();
    for provider in registry.all() {
        let manifest = provider.manifest();
        if registry.is_disabled(&manifest.id) {
            continue;
        }
        let at = match manifest.kind {
            ProviderKind::Library => AvailableAt::InLibrary,
            ProviderKind::Server => {
                AvailableAt::OnServer { server: manifest.display_name.clone() }
            }
            ProviderKind::Catalogue | ProviderKind::Demo => AvailableAt::PlaysHere,
            _ => continue,
        };
        for id in provider.titles() {
            out.entry(id).or_insert_with(|| at.clone());
        }
    }
    out
}

/// No results, without asking AniList: Available set where nothing can match it.
fn nothing() -> Found {
    Found { page: Page { items: Vec::new(), has_next: false }, unapplied: Vec::new() }
}

/// What the fallback could not apply, said once.
fn caption(found: &Found) -> String {
    let words = &found.unapplied;
    let list = match words.as_slice() {
        [] => return String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    let verb = if words.len() == 1 && !words[0].ends_with('s') { "was" } else { "were" };
    format!("AniList is unavailable, so {list} {verb} not applied.")
}

/// A query as AniList's arguments: bounds made exclusive, the adult setting applied.
pub fn browse_filter(query: &SearchQuery, adult: bool) -> BrowseFilter {
    let years = query.years.ordered();
    let (year, started_after, started_before) = match years.single() {
        Some(year) => (Some(year), None, None),
        None => (
            None,
            years.min.map(|y| (y - 1) * 10_000 + 9_999),
            years.max.map(|y| (y + 1) * 10_000),
        ),
    };
    let episodes = query.episodes.ordered();
    let minutes = query.minutes.ordered();
    let above = |at_least: Option<u32>| at_least.filter(|n| *n > 0).map(|n| n - 1);
    let below = |at_most: Option<u32>| at_most.map(|n| n + 1);
    let with_tags = !query.tags.is_empty() || !query.tags_excluded.is_empty();
    BrowseFilter {
        sort: vec![sort_name(query.sort_in_effect(), query.is_browse()).to_owned()],
        genres: query.genres.clone(),
        genres_excluded: query.genres_excluded.clone(),
        tags: query.tags.clone(),
        tags_excluded: query.tags_excluded.clone(),
        min_tag_rank: with_tags.then(|| u32::from(query.tag_rank_in_effect())),
        formats: query.formats.iter().map(|f| format_name(*f).to_owned()).collect(),
        statuses: query.statuses.iter().map(|s| status_name(*s).to_owned()).collect(),
        season: query.season.map(|s| season_name(s).to_owned()),
        year,
        started_after,
        started_before,
        episodes_greater: above(episodes.min),
        episodes_lesser: below(episodes.max),
        duration_greater: above(minutes.min),
        duration_lesser: below(minutes.max),
        min_score: above(query.min_score.map(u32::from)),
        min_popularity: above(query.min_popularity),
        sources: query.sources.iter().map(|s| source_name(*s).to_owned()).collect(),
        countries: query.countries.clone(),
        licensed_by: query.streaming_on.clone(),
        licensed: query.licensed_only.then_some(true),
        on_list: query.on_list.map(|on| on == OnList::Only),
        adult: (!adult).then_some(false),
        ids: Vec::new(),
    }
}

fn sort_name(sort: SearchSort, browse: bool) -> &'static str {
    match sort {
        SearchSort::Relevance if browse => "POPULARITY_DESC",
        SearchSort::Relevance => "SEARCH_MATCH",
        SearchSort::Trending => "TRENDING_DESC",
        SearchSort::Popularity => "POPULARITY_DESC",
        SearchSort::Score => "SCORE_DESC",
        SearchSort::Newest => "START_DATE_DESC",
        SearchSort::Oldest => "START_DATE",
        SearchSort::Title => "TITLE_ROMAJI",
        SearchSort::Favourites => "FAVOURITES_DESC",
    }
}

fn format_name(format: MediaFormat) -> &'static str {
    match format {
        MediaFormat::Tv => "TV",
        MediaFormat::TvShort => "TV_SHORT",
        MediaFormat::Movie => "MOVIE",
        MediaFormat::Special => "SPECIAL",
        MediaFormat::Ova => "OVA",
        MediaFormat::Ona => "ONA",
        MediaFormat::Music => "MUSIC",
    }
}

fn status_name(status: AiringStatus) -> &'static str {
    match status {
        AiringStatus::Releasing => "RELEASING",
        AiringStatus::Finished => "FINISHED",
        AiringStatus::NotYetReleased => "NOT_YET_RELEASED",
        AiringStatus::Cancelled => "CANCELLED",
        AiringStatus::Hiatus => "HIATUS",
    }
}

fn season_name(season: Season) -> &'static str {
    match season {
        Season::Winter => "WINTER",
        Season::Spring => "SPRING",
        Season::Summer => "SUMMER",
        Season::Fall => "FALL",
    }
}

fn source_name(source: SourceMaterial) -> &'static str {
    match source {
        SourceMaterial::Original => "ORIGINAL",
        SourceMaterial::Manga => "MANGA",
        SourceMaterial::LightNovel => "LIGHT_NOVEL",
        SourceMaterial::VisualNovel => "VISUAL_NOVEL",
        SourceMaterial::VideoGame => "VIDEO_GAME",
        SourceMaterial::Novel => "NOVEL",
        SourceMaterial::Doujinshi => "DOUJINSHI",
        SourceMaterial::Anime => "ANIME",
        SourceMaterial::WebNovel => "WEB_NOVEL",
        SourceMaterial::LiveAction => "LIVE_ACTION",
        SourceMaterial::Game => "GAME",
        SourceMaterial::Comic => "COMIC",
        SourceMaterial::MultimediaProject => "MULTIMEDIA_PROJECT",
        SourceMaterial::PictureBook => "PICTURE_BOOK",
        SourceMaterial::Other => "OTHER",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_model::search::Bounds;

    #[test]
    fn a_query_becomes_anilists_arguments_with_its_bounds_made_exclusive() {
        let query = SearchQuery {
            years: Bounds { min: Some(2010), max: Some(2015) },
            episodes: Bounds { min: None, max: Some(12) },
            minutes: Bounds { min: Some(60), max: None },
            min_score: Some(70),
            min_popularity: Some(1_000),
            formats: vec![MediaFormat::TvShort],
            tags: vec!["Time Travel".into()],
            on_list: Some(OnList::Hide),
            ..SearchQuery::default()
        };
        let filter = browse_filter(&query, false);
        assert_eq!(filter.started_after, Some(2009_9999));
        assert_eq!(filter.started_before, Some(2016_0000));
        assert_eq!((filter.episodes_greater, filter.episodes_lesser), (None, Some(13)));
        assert_eq!((filter.duration_greater, filter.duration_lesser), (Some(59), None));
        assert_eq!(filter.min_score, Some(69));
        assert_eq!(filter.min_popularity, Some(999));
        assert_eq!(filter.formats, ["TV_SHORT"]);
        assert_eq!(filter.min_tag_rank, Some(60));
        assert_eq!(filter.on_list, Some(false));
        assert_eq!(filter.adult, Some(false), "adult titles are hidden unless asked for");
        assert_eq!(filter.sort, ["POPULARITY_DESC"], "a browse with filters by popularity");
        assert_eq!(browse_filter(&query, true).adult, None);

        let one_year = SearchQuery {
            years: Bounds { min: Some(2024), max: Some(2024) },
            ..SearchQuery::text("kaze")
        };
        let filter = browse_filter(&one_year, false);
        assert_eq!((filter.year, filter.started_after), (Some(2024), None));
        assert_eq!(filter.sort, ["SEARCH_MATCH"]);
    }

    #[test]
    fn the_fallbacks_caption_names_what_it_left_out() {
        let found =
            |unapplied| Found { page: Page { items: Vec::new(), has_next: false }, unapplied };
        assert_eq!(caption(&found(vec![])), "");
        assert_eq!(
            caption(&found(vec!["tags"])),
            "AniList is unavailable, so tags were not applied."
        );
        assert_eq!(
            caption(&found(vec!["tags", "episode count", "source"])),
            "AniList is unavailable, so tags, episode count and source were not applied."
        );
        assert_eq!(
            caption(&found(vec!["episode length"])),
            "AniList is unavailable, so episode length was not applied."
        );
    }
}
