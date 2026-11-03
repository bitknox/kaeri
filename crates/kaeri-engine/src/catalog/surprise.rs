//! Surprise me and Another: one title picked at random from the filters on screen or from the
//! Planning list.
//!
//! The Planning list asks nothing: it is on the device. The filters ask AniList how many titles
//! match (kept for ten minutes per filter), then for one page of them, picked at random among the
//! first [`TOTAL_CEILING`](kaeri_meta::anilist::trends::TOTAL_CEILING) of them. A pick skips what
//! is completed, dropped or dismissed with Not interested, what is adult unless adult titles are
//! shown, and the last [`REMEMBERED`] picks, which is what makes Another say something new. When
//! all that is left has just been picked, the memory is let go and the pick goes on.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use kaeri_core::ids::AnilistId;
use kaeri_meta::{AniListError, Media};
use kaeri_model::{
    Msg,
    browsing::{SurpriseOutcome, SurprisePool},
    event::Event,
    notice::Notice,
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
