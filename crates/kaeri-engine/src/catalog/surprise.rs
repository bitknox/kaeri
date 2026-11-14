//! Surprise me and Another: one title picked at random from the filters on screen or from the
//! Planning list, or one book from reading's filters.
//!
//! The Planning list asks nothing: it is on the device. The filters ask AniList how many titles
//! match (kept for ten minutes per filter), then for one page of them, picked at random among the
//! first [`TOTAL_CEILING`](kaeri_meta::anilist::trends::TOTAL_CEILING) of them. A pick skips what
//! is completed, dropped or dismissed with Not interested, what is adult unless adult titles are
//! shown, and the last [`REMEMBERED`] picks, which is what makes Another say something new. When
//! all that is left has just been picked, the memory is let go and the pick goes on.
//!
//! Reading's filters ask AniList's books, as Search's start rows do. Books have no count to ask
//! first: a pick tries a random page among the first [`READING_PAGES`], and a page past the end
//! says where the matches stop, which is kept as the count is.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use kaeri_core::ids::AnilistId;
use kaeri_meta::{
    AniListError, Media,
    anilist::books::{Book, BookAsk},
};
use kaeri_model::{
    Msg,
    browsing::{SurpriseOutcome, SurprisePool},
    event::Event,
    notice::Notice,
    reading::ReadSearchQuery,
    search::SearchQuery,
};
use kaeri_store::Store;

use crate::{
    catalog::{filter::dismissed, search::browse_filter},
    dispatch::Lent,
};

/// How many of the last picks are not picked again.
pub const REMEMBERED: usize = 20;
/// Titles of a page of the filters' matches.
const PER_PAGE: u32 = 50;
/// How long a count of matches serves.
const TOTAL_FOR: i64 = 10 * 60;
/// How many pages are tried before the filters count as having nothing to pick.
const ATTEMPTS: usize = 3;
/// Pages of reading's matches a pick may land on: the first thousand books in the order asked,
/// popularity unless Search chose another.
const READING_PAGES: usize = 20;
/// Pages a reading pick asks for at most, a page past the end among them.
const READING_ATTEMPTS: usize = 5;

/// What picks keep between commands: the last ones, and the counts of matches asked.
#[derive(Clone, Default)]
pub struct Picks {
    inner: Arc<Mutex<Memory>>,
}

#[derive(Default)]
struct Memory {
    recent: VecDeque<AnilistId>,
    totals: HashMap<String, (u32, i64)>,
}

impl Picks {
    fn lock(&self) -> std::sync::MutexGuard<'_, Memory> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn recently(&self, id: AnilistId) -> bool {
        self.lock().recent.contains(&id)
    }

    fn remember(&self, id: AnilistId) {
        let mut memory = self.lock();
        memory.recent.retain(|seen| *seen != id);
        memory.recent.push_back(id);
        while memory.recent.len() > REMEMBERED {
            memory.recent.pop_front();
        }
    }

    fn forget_all(&self) {
        self.lock().recent.clear();
    }

    fn total(&self, key: &str, now: i64) -> Option<u32> {
        let (total, at) = *self.lock().totals.get(key)?;
        (now - at < TOTAL_FOR).then_some(total)
    }

    fn keep_total(&self, key: String, total: u32, now: i64) {
        let mut memory = self.lock();
        memory.totals.retain(|_, (_, at)| now - *at < TOTAL_FOR);
        memory.totals.insert(key, (total, now));
    }
}

/// How the pick chooses among `len` things: an index below `len`. Random, except in a test.
pub type Chooser<'a> = &'a (dyn Fn(usize) -> usize + Send + Sync);

/// A random index below `len`.
pub fn random(len: usize) -> usize {
    use rand::RngExt;
    rand::rng().random_range(0..len.max(1))
}

/// [`Command::Surprise`]: pick from `pool`, and say what came of it.
///
/// [`Command::Surprise`]: kaeri_model::command::Command::Surprise
pub async fn pick(
    lent: Lent,
    picks: Picks,
    pool: SurprisePool,
    adult: bool,
    offline: bool,
    now: i64,
    choose: Chooser<'_>,
) {
    let (outcome, notice) = match &pool {
        SurprisePool::Planning => planning(&lent.store, &lent.meta, &picks, adult, choose),
        SurprisePool::Filters { query } => {
            filters(&lent, &picks, query, adult, offline, now, choose).await
        }
        SurprisePool::Reading { query } => {
            reading(&lent.meta, &picks, query, adult, offline, now, choose).await
        }
    };
    if let Some(msg) = notice {
        let _ = lent.tx.send(Event::Toast(Notice::info(msg)));
    }
    let _ = lent.tx.send(Event::Surprise { pool, outcome });
}

/// Whether `id` may be picked: not finished with, and not dismissed.
fn open(store: &Store, id: AnilistId, dismissed: &[crate::catalog::filter::Dismissed]) -> bool {
    if dismissed.iter().any(|d| d.id == id.get()) {
        return false;
    }
    !matches!(store.library_status(id).ok().flatten().as_deref(), Some("COMPLETED" | "DROPPED"))
}

fn planning(
    store: &Store,
    meta: &kaeri_meta::Meta,
    picks: &Picks,
    adult: bool,
    choose: Chooser<'_>,
) -> (SurpriseOutcome, Option<Msg>) {
    let dismissed = dismissed(store);
    let eligible: Vec<AnilistId> = store
        .library_with_status("PLANNING")
        .unwrap_or_default()
        .into_iter()
        .filter(|id| open(store, *id, &dismissed))
        .filter(|id| {
            // Adult titles are known by the cards kept; one never seen is not hidden.
            adult || meta.cached(&[*id]).first().is_none_or(|m| m.is_adult != Some(true))
        })
        .collect();
    if eligible.is_empty() {
        return (SurpriseOutcome::Empty, Some(Msg::NoticeSurpriseEmptyPlanning));
    }
    let mut fresh: Vec<AnilistId> =
        eligible.iter().copied().filter(|id| !picks.recently(*id)).collect();
    if fresh.is_empty() {
        picks.forget_all();
        fresh = eligible.clone();
    }
    let id = fresh[choose(fresh.len()).min(fresh.len() - 1)];
    picks.remember(id);
    (SurpriseOutcome::Picked { id, more: eligible.len() > 1 }, None)
}

async fn filters(
    lent: &Lent,
    picks: &Picks,
    query: &SearchQuery,
    adult: bool,
    offline: bool,
    now: i64,
    choose: Chooser<'_>,
) -> (SurpriseOutcome, Option<Msg>) {
    if offline {
        return (SurpriseOutcome::Offline, Some(Msg::NoticeSurpriseOffline));
    }
    let empty = || (SurpriseOutcome::Empty, Some(Msg::NoticeSurpriseEmptyFilters));
    let failed = || (SurpriseOutcome::Failed, Some(Msg::NoticeSurpriseFailed));
    // The text, the sort and the period do not narrow what a pick is from.
    let mut shaped = query.clone();
    shaped.text.clear();
    let filter = browse_filter(&shaped, adult);
    let key = format!("{filter:?}");
    let total = match picks.total(&key, now) {
        Some(total) => total,
        None => match lent.meta.browse_total(&filter).await {
            Ok(Some(total)) => {
                picks.keep_total(key, total, now);
                total
            }
            Ok(None) => return failed(),
            Err(e) => {
                tracing::warn!(error = %e, "could not count the titles to pick from");
                return match e {
                    AniListError::Network(_) | AniListError::RateLimited { .. } => failed(),
                    _ => failed(),
                };
            }
        },
    };
    if total == 0 {
        return empty();
    }
    let pages = total.div_ceil(PER_PAGE).max(1) as usize;
    let dismissed = dismissed(&lent.store);
    let mut tried = Vec::new();
    for _ in 0..ATTEMPTS.min(pages) {
        // A page not tried yet, when there are others.
        let mut page = choose(pages).min(pages - 1);
        while tried.contains(&page) && tried.len() < pages {
            page = (page + 1) % pages;
        }
        tried.push(page);
        let found = match lent.meta.find("", &filter, page as u32 + 1, PER_PAGE).await {
            Ok(found) => found,
            Err(e) => {
                tracing::warn!(error = %e, "could not fetch titles to pick from");
                return failed();
            }
        };
        let open: Vec<&Media> = found
            .page
            .items
            .iter()
            .filter(|m| open(&lent.store, m.id, &dismissed))
            .filter(|m| adult || m.is_adult != Some(true))
            .collect();
        let mut fresh: Vec<&Media> =
            open.iter().copied().filter(|m| !picks.recently(m.id)).collect();
        if fresh.is_empty()
            && !open.is_empty()
            && (total as usize) <= REMEMBERED + PER_PAGE as usize
        {
            // Everything there is has just been picked: begin again.
            picks.forget_all();
            fresh = open.clone();
        }
        if fresh.is_empty() {
            continue;
        }
        let id = fresh[choose(fresh.len()).min(fresh.len() - 1)].id;
        picks.remember(id);
        return (SurpriseOutcome::Picked { id, more: total > 1 }, None);
    }
    empty()
}

async fn reading(
    meta: &kaeri_meta::Meta,
    picks: &Picks,
    query: &ReadSearchQuery,
    adult: bool,
    offline: bool,
    now: i64,
    choose: Chooser<'_>,
) -> (SurpriseOutcome, Option<Msg>) {
    if offline {
        return (SurpriseOutcome::Offline, Some(Msg::NoticeSurpriseOffline));
    }
    let empty = || (SurpriseOutcome::Empty, Some(Msg::NoticeSurpriseEmptyFilters));
    let failed = || (SurpriseOutcome::Failed, Some(Msg::NoticeSurpriseFailed));
    // The words do not narrow what a pick is from, as on the shows' side.
    let mut shaped = query.clone();
    shaped.text.clear();
    let filter = crate::read_catalog::search::filter_of(&shaped, adult, now);
    let key = format!("read:{filter:?}");
    let mut pages = picks.total(&key, now).map_or(READING_PAGES, |n| n as usize).max(1);
    let mut tried: Vec<usize> = Vec::new();
    for attempt in 0..READING_ATTEMPTS {
        // A page not tried yet, when there are others. The last try is the first page, which
        // has the matches if there are any.
        let mut page = choose(pages).min(pages - 1);
        if attempt + 1 == READING_ATTEMPTS && !tried.contains(&0) {
            page = 0;
        }
        while tried.contains(&page) && tried.iter().filter(|p| **p < pages).count() < pages {
            page = (page + 1) % pages;
        }
        if tried.contains(&page) {
            break;
        }
        tried.push(page);
        let ask = BookAsk::browse("", &filter, page as u32 + 1, PER_PAGE);
        let found = match meta.books(&[ask]).await.map(|pages| pages.into_iter().next()) {
            Ok(Some(found)) => found,
            Ok(None) => return failed(),
            Err(e) => {
                tracing::warn!(error = %e, "could not fetch books to pick from");
                return failed();
            }
        };
        if found.items.is_empty() {
            if page == 0 {
                return empty();
            }
            // The matches end before this page: the pick is among the pages before it.
            pages = page;
            picks.keep_total(key.clone(), pages as u32, now);
            continue;
        }
        if !found.has_next && page + 1 < pages {
            pages = page + 1;
            picks.keep_total(key.clone(), pages as u32, now);
        }
        let open: Vec<&Book> = found.items.iter().filter(|b| adult || !b.adult()).collect();
        let mut fresh: Vec<&Book> =
            open.iter().copied().filter(|b| !picks.recently(b.id)).collect();
        if fresh.is_empty() && !open.is_empty() && pages == 1 {
            // Everything there is has just been picked: begin again.
            picks.forget_all();
            fresh = open.clone();
        }
        if fresh.is_empty() {
            continue;
        }
        let id = fresh[choose(fresh.len()).min(fresh.len() - 1)].id;
        picks.remember(id);
        return (SurpriseOutcome::Picked { id, more: pages > 1 || open.len() > 1 }, None);
    }
    empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planned(store: &Store, ids: &[u32]) {
        let rows: Vec<(AnilistId, &str)> =
            ids.iter().map(|id| (AnilistId::new(*id), "PLANNING")).collect();
        store.replace_library_statuses("anilist", &rows, 0).unwrap();
    }

    fn meta(store: &Store) -> kaeri_meta::Meta {
        let http =
            kaeri_net::HttpClient::offline(&kaeri_core::config::NetworkConfig::default())
                .unwrap();
        kaeri_meta::Meta::new(kaeri_meta::AniList::new(http, 30), None, store.clone())
    }

    #[test]
    fn planning_picks_from_the_list_and_never_the_last_picks_until_it_must() {
        let store = Store::open_in_memory().unwrap();
        planned(&store, &[1, 2, 3]);
        let meta = meta(&store);
        let picks = Picks::default();
        let first = |len: usize| len - len; // always the first of what is left
        let mut seen = Vec::new();
        for _ in 0..3 {
            let (outcome, notice) = planning(&store, &meta, &picks, false, &first);
            assert!(notice.is_none());
            let SurpriseOutcome::Picked { id, more } = outcome else { panic!("{outcome:?}") };
            assert!(more);
            assert!(!seen.contains(&id), "a pick is not the one before");
            seen.push(id);
        }
        // All three are in the memory: it is let go and the pick goes on.
        let (outcome, _) = planning(&store, &meta, &picks, false, &first);
        assert!(matches!(outcome, SurpriseOutcome::Picked { .. }));
    }

    #[test]
    fn a_planning_list_of_one_has_no_another_and_an_empty_one_says_so() {
        let store = Store::open_in_memory().unwrap();
        let meta = meta(&store);
        let picks = Picks::default();
        let (outcome, notice) = planning(&store, &meta, &picks, false, &random);
        assert_eq!(outcome, SurpriseOutcome::Empty);
        assert_eq!(notice, Some(Msg::NoticeSurpriseEmptyPlanning));
        planned(&store, &[7]);
        let (outcome, _) = planning(&store, &meta, &picks, false, &random);
        assert_eq!(outcome, SurpriseOutcome::Picked { id: AnilistId::new(7), more: false });
    }

    fn book(id: u32, popularity: u32, genre: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "title": { "romaji": format!("Lantern Row {id}"), "english": null },
            "format": "MANGA",
            "status": "FINISHED",
            "startDate": { "year": 2015, "month": 1, "day": 1 },
            "tags": [],
            "isAdult": false,
            "popularity": popularity,
            "genres": [genre],
            "synonyms": [],
            "coverImage": { "large": format!("https://covers.example/{id}.png") },
        })
    }

    #[tokio::test]
    async fn reading_picks_a_book_from_the_filters_and_learns_where_they_end() {
        use kaeri_testkit::anilist::{FakeAniList, Operation};
        let fake = FakeAniList::start().await;
        fake.add_books([book(920_001, 30, "Drama"), book(920_002, 20, "Drama")]);
        fake.add_books([book(920_003, 10, "Drama"), book(920_004, 40, "Comedy")]);
        let store = Store::open_in_memory().unwrap();
        let meta = fake.meta(&store);
        let picks = Picks::default();
        let query = ReadSearchQuery {
            text: "words do not narrow it".into(),
            genres: vec!["Drama".into()],
            ..ReadSearchQuery::default()
        };
        // Always the last page and the last book: the pages past the end come first.
        let last = |len: usize| len - 1;
        let mut seen = Vec::new();
        for _ in 0..3 {
            let (outcome, notice) =
                reading(&meta, &picks, &query, false, false, 0, &last).await;
            assert!(notice.is_none());
            let SurpriseOutcome::Picked { id, more } = outcome else { panic!("{outcome:?}") };
            assert!(more);
            assert!(!seen.contains(&id), "a pick is not the one before");
            seen.push(id);
        }
        seen.sort();
        let drama: Vec<AnilistId> = [920_001, 920_002, 920_003].map(AnilistId::new).into();
        assert_eq!(seen, drama);
        // The tries past the end, then the first page; once the end is known, one page a pick.
        assert_eq!(fake.requests_for(Operation::Books).len(), READING_ATTEMPTS + 2);
        // All three were picked: the memory lets go and the pick goes on.
        let (outcome, _) = reading(&meta, &picks, &query, false, false, 0, &last).await;
        assert!(matches!(outcome, SurpriseOutcome::Picked { .. }));
    }

    #[tokio::test]
    async fn reading_filters_with_nothing_or_no_connection_say_so() {
        let fake = kaeri_testkit::anilist::FakeAniList::start().await;
        fake.add_books([book(920_011, 10, "Drama")]);
        let store = Store::open_in_memory().unwrap();
        let meta = fake.meta(&store);
        let picks = Picks::default();
        let query = ReadSearchQuery {
            genres: vec!["Nothing Like It".into()],
            ..ReadSearchQuery::default()
        };
        let (outcome, notice) = reading(&meta, &picks, &query, false, false, 0, &random).await;
        assert_eq!(outcome, SurpriseOutcome::Empty);
        assert_eq!(notice, Some(Msg::NoticeSurpriseEmptyFilters));
        let (outcome, notice) = reading(&meta, &picks, &query, false, true, 0, &random).await;
        assert_eq!(outcome, SurpriseOutcome::Offline);
        assert_eq!(notice, Some(Msg::NoticeSurpriseOffline));
    }

    #[test]
    fn the_memory_holds_twenty_picks() {
        let picks = Picks::default();
        for id in 1..=25 {
            picks.remember(AnilistId::new(id));
        }
        assert!(!picks.recently(AnilistId::new(5)));
        assert!(picks.recently(AnilistId::new(6)));
        assert!(picks.recently(AnilistId::new(25)));
    }
}
