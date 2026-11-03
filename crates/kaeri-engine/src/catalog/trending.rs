//! Trending over a period: Search's Trending rail and the period menu.
//!
//! Today is AniList's live trending, the rail's own query. A week or a month is summed from
//! AniList's daily figures ([`kaeri_meta::Meta::trending_over`]): one request for the week, two
//! for the month. The answer is kept as ids in the store, with the time, so it shows at once
//! after a restart: six hours for the week, a day for the month. One that has gone old shows
//! first, with its age, and is asked again behind the screen. Nothing here runs at launch: it
//! answers [`Command::LoadTrending`], and the Trending sort with a period in Search.
//!
//! [`Command::LoadTrending`]: kaeri_model::command::Command::LoadTrending

use kaeri_core::ids::AnilistId;
use kaeri_meta::{AniListError, Media, Meta};
use kaeri_model::{
    browsing::TrendPeriod,
    catalog::Entry,
    event::Event,
    search::{SearchQuery, SearchSort},
};
use kaeri_store::Store;
use serde::{Deserialize, Serialize};

use crate::{catalog::browse::RAIL, catalog::search::browse_filter, data, dispatch::Lent};

/// Where the period the viewer chose is remembered on this device.
const CHOICE_KEY: &str = "trending:period";

/// What a period came to, as kept: when, for which adult setting, and the titles in order.
#[derive(Serialize, Deserialize)]
struct Kept {
    at: i64,
    adult: bool,
    ids: Vec<u32>,
}

fn key(period: TrendPeriod) -> &'static str {
    match period {
        TrendPeriod::Today => "trending:today",
        TrendPeriod::Week => "trending:week",
        TrendPeriod::Month => "trending:month",
    }
}

/// The period the viewer chose last, Today when none.
pub fn remembered(store: &Store) -> TrendPeriod {
    match store.get_meta_text(CHOICE_KEY).ok().flatten().as_deref() {
        Some("week") => TrendPeriod::Week,
        Some("month") => TrendPeriod::Month,
        _ => TrendPeriod::Today,
    }
}

fn choose(store: &Store, period: TrendPeriod) {
    let name = match period {
        TrendPeriod::Today => "today",
        TrendPeriod::Week => "week",
        TrendPeriod::Month => "month",
    };
    if let Err(e) = store.set_meta_text(CHOICE_KEY, name) {
        tracing::warn!(error = %e, "could not keep the trending period");
    }
}

/// What is kept for `period` under the adult setting `adult`, as the cards the metadata cache
/// still holds, and when it was made.
fn kept(
    meta: &Meta,
    store: &Store,
    period: TrendPeriod,
    adult: bool,
) -> Option<(Vec<Media>, i64)> {
    let kept: Kept =
        serde_json::from_str(&store.get_meta_text(key(period)).ok().flatten()?).ok()?;
    if kept.adult != adult {
        return None;
    }
    let ids: Vec<AnilistId> = kept.ids.iter().map(|id| AnilistId::new(*id)).collect();
    let media = meta.cached(&ids);
    // A cache that lost most of its cards is no answer: ask again.
    (media.len() * 2 >= ids.len() && !media.is_empty()).then_some((media, kept.at))
}

/// Ask AniList for `period` and keep what it says. Not for Today, which is the rail's own query.
async fn fetch(
    meta: &Meta,
    store: &Store,
    period: TrendPeriod,
    adult: bool,
    now: i64,
) -> Result<Vec<Media>, AniListError> {
    let mut media = meta.trending_over(i64::from(period.days()), now).await?;
    if !adult {
        media.retain(|m| m.is_adult != Some(true));
    }
    let kept = Kept { at: now, adult, ids: media.iter().map(|m| m.id.get()).collect() };
    if let Ok(json) = serde_json::to_string(&kept)
        && let Err(e) = store.set_meta_text(key(period), &json)
    {
        tracing::warn!(error = %e, "could not keep the trending period");
    }
    Ok(media)
}

/// The titles trending over a week or a month, most first: what is kept while it serves,
/// otherwise asked for, and what is kept however old when AniList cannot say. For a search sorted
/// by Trending with a period. `Err` carries how long AniList asked to wait, when it did.
pub(crate) async fn ranked(
    meta: &Meta,
    store: &Store,
    period: TrendPeriod,
    adult: bool,
    now: i64,
) -> Result<Vec<Media>, Option<u64>> {
    let held = kept(meta, store, period, adult);
    if let Some((media, at)) = &held
        && now - at < period.kept_for()
    {
        return Ok(media.clone());
    }
    match fetch(meta, store, period, adult, now).await {
        Ok(media) => Ok(media),
        Err(e) => match held {
            Some((media, _)) => Ok(media),
            None => Err(match e {
                AniListError::RateLimited { retry_in_secs } => Some(retry_in_secs),
                _ => None,
            }),
        },
    }
}

fn entries(media: &[Media], store: &Store) -> Vec<Entry> {
    media.iter().map(|m| data::entry_from(m, Some(store))).collect()
}

/// [`Command::LoadTrending`]: remember the choice, and answer with the rail for `period`.
///
/// [`Command::LoadTrending`]: kaeri_model::command::Command::LoadTrending
pub async fn load(lent: Lent, period: TrendPeriod, adult: bool, now: i64, offline: bool) {
    let Lent { meta, store, tx, .. } = lent;
    choose(&store, period);
    answer(&meta, &store, &tx, period, adult, now, offline).await;
}

/// The rail for the period chosen before, for Search's start: what is kept, from the cache
/// alone, and behind the screen asked again once old. Nothing for Today, which the start's own
/// rail is.
pub async fn load_remembered(lent: Lent, adult: bool, now: i64, offline: bool) {
    let Lent { meta, store, tx, .. } = lent;
    let period = remembered(&store);
    if period == TrendPeriod::Today {
        return;
    }
    answer(&meta, &store, &tx, period, adult, now, offline).await;
}

async fn answer(
    meta: &Meta,
    store: &Store,
    tx: &crate::outbox::Outbox,
    period: TrendPeriod,
    adult: bool,
    now: i64,
    offline: bool,
) {
    if period == TrendPeriod::Today {
        let filter = browse_filter(&SearchQuery::sorted(SearchSort::Trending), adult);
        let event = match meta.find("", &filter, 1, RAIL).await {
            Ok(found) => Event::Trending {
                period,
                entries: entries(&found.page.items, store),
                updated_at: None,
                failed: false,
                waiting: None,
            },
            Err(e) => {
                tracing::warn!(error = %e, "trending today failed");
                failed(period, None)
            }
        };
        let _ = tx.send(event);
        return;
    }
    let held = kept(meta, store, period, adult);
    let fresh = held.as_ref().is_some_and(|(_, at)| now - at < period.kept_for());
    if let Some((media, at)) = &held {
        let _ = tx.send(Event::Trending {
            period,
            entries: entries(media, store),
            updated_at: Some(*at),
            failed: false,
            waiting: None,
        });
        if fresh {
            return;
        }
    }
    if offline {
        if held.is_none() {
            let _ = tx.send(failed(period, None));
        }
        return;
    }
    // Old and shown: asked again behind the screen. Nothing shown: the viewer waits for it.
    let asked = fetch(meta, store, period, adult, now);
    let asked =
        if held.is_some() { kaeri_meta::in_background(asked).await } else { asked.await };
    match asked {
        Ok(media) => {
            let _ = tx.send(Event::Trending {
                period,
                entries: entries(&media, store),
                updated_at: None,
                failed: false,
                waiting: None,
            });
        }
        Err(e) if held.is_none() => {
            tracing::warn!(error = %e, ?period, "trending failed");
            let waiting = match e {
                AniListError::RateLimited { retry_in_secs } => {
                    Some(u32::try_from(retry_in_secs).unwrap_or(u32::MAX))
                }
                _ => None,
            };
            let _ = tx.send(failed(period, waiting));
        }
        Err(e) => tracing::info!(error = %e, ?period, "trending could not be refreshed"),
    }
}

fn failed(period: TrendPeriod, waiting: Option<u32>) -> Event {
    Event::Trending { period, entries: Vec::new(), updated_at: None, failed: true, waiting }
}
