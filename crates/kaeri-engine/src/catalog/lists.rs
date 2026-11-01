//! The lists: what you were watching, from local history, and this season's most popular, each
//! followed by when its releasing titles last aired.

use kaeri_core::ids::AnilistId;
use kaeri_meta::Meta;
use kaeri_model::{
    Msg,
    catalog::{Aired, ContentFor, ContentList, TitleAired},
    event::Event,
    notice::Notice,
};
use kaeri_store::Store;

use crate::{data, dispatch::Lent, now_epoch, outbox::Outbox, services::Services};

/// What you were watching, as the CONTINUE rail lists it, or the empty that says nothing has
/// been watched yet.
pub async fn load_continue(lent: Lent) {
    let Lent { meta, store, tx, .. } = lent;
    let update = match continue_entries(&meta, &store).await {
        // Nothing in the history at all is a different empty from a history with
        // nothing left to continue — everything hidden with `x` — and only the first
        // is "nothing watched yet". `Empty` says it is the first.
        Ok(entries) if entries.is_empty() && store.stats().is_ok_and(|s| s.titles == 0) => {
            Event::Content { view: ContentFor::Continue, content: ContentList::Empty }
        }
        Ok(entries) => {
            publish_list(&meta, &tx, ContentFor::Continue, entries, now_epoch()).await;
            return;
        }
        Err(e) => {
            Event::Content { view: ContentFor::Continue, content: ContentList::Failed(e) }
        }
    };
    let _ = tx.send(update);
}

/// This season's most popular titles, captioned with which season it is: Search's preset for
/// the season, which the screen can open in Search to filter further.
pub async fn load_seasonal(lent: Lent, adult: bool) {
    const PAGE: u32 = 40;
    let (season, year) = crate::catalog::browse::this_season(now_epoch());
    let query = kaeri_model::search::SearchQuery::season(season, year);
    let filter = crate::catalog::search::browse_filter(&query, adult);
    let Lent { meta, store, tx, .. } = lent;
    let update = match meta.find("", &filter, 1, PAGE).await {
        Ok(found) => {
            // Which season this is, which the screen never said, and that one
            // page is not all of it when it is not.
            let season =
                format!("{} {year}", kaeri_model::search::season_label(season).to_lowercase());
            let text = if found.page.has_next {
                format!("{season} · the {PAGE} most popular")
            } else {
                season
            };
            let _ = tx.send(Event::Caption { view: ContentFor::Seasonal, text });
            let entries =
                found.page.items.iter().map(|m| data::entry_from(m, Some(&store))).collect();
            publish_list(&meta, &tx, ContentFor::Seasonal, entries, now_epoch()).await;
            return;
        }
        Err(e) => Event::Content {
            view: ContentFor::Seasonal,
            content: ContentList::Failed(e.to_string()),
        },
    };
    let _ = tx.send(update);
}

/// Take a title off the CONTINUE rail for good.
pub fn hide_from_continue(id: AnilistId, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    // The reducer has already dropped the row from the screen; failing to persist
    // that would resurrect it on the next reload, so the failure is named.
    if let Err(e) = store.hide_from_continue(id) {
        let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeContinueHideFailed {
            reason: e.to_string(),
        })));
    }
}

/// What you were watching, most recent first.
///
/// Trending and continuing are different questions, and this screen used to answer the wrong one:
/// it fetched the current season, which made the section labelled CONTINUE a discovery screen and
/// left an episode abandoned halfway through — the single most likely reason to open the app —
/// with nowhere to appear. Discovery has three sections of its own; this one is personal.
///
/// Built from local history, so it works with no account at all.
///
/// **Ordering is the feature.** `continue_list` orders by `updated_at DESC` and
/// [`AniList::media_many`] preserves the order it was asked in — which it has to, because AniList
/// returns `id_in` results in its own order and letting that through would shuffle the rail into
/// something arbitrary. Together they are what put the thing you last watched at the top.
///
/// An empty result is a real answer here, not a failure to paper over: the screen says so and
/// points at where to find something, rather than silently showing a different kind of list.
async fn continue_entries(
    meta: &Meta,
    store: &Store,
) -> std::result::Result<Vec<kaeri_model::catalog::Entry>, String> {
    let continuing = store.continue_list(CONTINUE_ROWS).unwrap_or_default();
    let ids: Vec<kaeri_core::ids::AnilistId> =
        continuing.iter().map(|p| p.anilist_id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    // The records asked for again are those older than Home keeps them: the rail and Home's
    // Continue read the same cache, and one request answers both.
    let stale = crate::catalog::home::stale_among(store, ids.clone());
    let held = meta.cached(&ids);
    let mut fetched = if stale.is_empty() {
        Vec::new()
    } else {
        match meta.media_many(&stale).await {
            Ok(found) => found,
            // What is kept stands in for the records that could not be asked again: the rail
            // is the history's, and an older airing line beats no rail.
            Err(e) if !held.is_empty() => {
                tracing::warn!(error = %e, "Continue's records were not asked again; kept ones shown");
                Vec::new()
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let media: Vec<_> = ids
        .iter()
        .filter_map(|id| {
            fetched
                .iter()
                .position(|m| m.id == *id)
                .map(|at| fetched.swap_remove(at))
                .or_else(|| held.iter().find(|m| m.id == *id).cloned())
        })
        .collect();
    Ok(media.iter().map(|m| data::entry_from(m, Some(store))).collect())
}

/// Titles kept in the CONTINUE rail. Long enough to cover what you are actually mid-way through,
/// short enough that it stays a shortlist rather than a second library.
const CONTINUE_ROWS: u32 = 15;

/// Publish a list, then follow up with the broadcast dates it needs a second request for.
///
/// Two sends rather than one: the last-aired query depends on which titles came back, so it
/// cannot be folded into the list query. The list renders as soon as it arrives and gains the
/// broadcast line a moment later, rather than the whole screen waiting on an annotation.
pub async fn publish_list(
    meta: &Meta,
    tx: &Outbox,
    view: ContentFor,
    entries: Vec<kaeri_model::catalog::Entry>,
    now: i64,
) {
    let _ = tx.send(Event::Content { view, content: ContentList::Entries(entries.clone()) });
    follow_aired(meta, tx, &entries, now).await;
}

/// When the releasing titles among `entries` last aired, as one more request, for a list
/// already on screen.
///
/// An answer is kept in memory until the title's next broadcast, so a list shown again asks
/// nothing for it. A request that fails, over the limit or not, is not given up on: what is
/// kept is said at once, and the rest is asked for again after AniList's own wait, a few times,
/// behind the list.
pub async fn follow_aired(
    meta: &Meta,
    tx: &Outbox,
    entries: &[kaeri_model::catalog::Entry],
    now: i64,
) {
    // Releasing titles only. A finished show has no broadcast to report, and including it would
    // spend rows of a capped response on titles that cannot use them.
    let titles: Vec<(kaeri_core::ids::AnilistId, i64)> = entries
        .iter()
        .filter_map(|e| e.airing_in.map(|airing_in| (e.id, now.saturating_add(airing_in))))
        .collect();
    if titles.is_empty() {
        return;
    }
    if say_aired(meta, tx, &titles, now).await {
        return;
    }
    let (meta, tx) = (meta.clone(), tx.clone());
    tokio::spawn(async move {
        for _ in 0..AIRED_RETRIES {
            let wait = meta.rate_limit_wait().await.unwrap_or_default();
            tokio::time::sleep(wait.max(AIRED_RETRY_AFTER)).await;
            if say_aired(&meta, &tx, &titles, now_epoch()).await {
                return;
            }
        }
    });
}

/// How many more times a last-aired lookup that failed is asked for, and the least it waits.
const AIRED_RETRIES: u32 = 3;
const AIRED_RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// Say when `titles` last aired, kept answers and new, and whether nothing was left unanswered.
async fn say_aired(
    meta: &Meta,
    tx: &Outbox,
    titles: &[(kaeri_core::ids::AnilistId, i64)],
    now: i64,
) -> bool {
    let (rows, failure) = meta.last_aired_kept(titles, now).await;
    let aired: Vec<TitleAired> = rows
        .into_iter()
        // Aired rows only. AniList has answered this query with rows whose air time is still
        // ahead, and naming next week's episode as "out just now" is exactly the lie the
        // broadcast line exists not to tell.
        .filter(|r| r.airing_at <= now)
        .map(|r| TitleAired {
            id: r.media_id,
            aired: Aired { episode: r.episode, ago: now.saturating_sub(r.airing_at) },
        })
        .collect();
    if !aired.is_empty() || failure.is_none() {
        let _ = tx.send(Event::LastAired(aired));
    }
    // Not worth a toast: the list is already on screen and complete, and this only adds a line
    // to it. Logged so the Logs overlay can still explain a missing line.
    if let Some(e) = &failure {
        tracing::warn!(%e, "last-aired lookup failed; asking again shortly");
    }
    failure.is_none()
}

/// The season containing today, derived from the epoch without a calendar dependency.
pub fn current_season() -> (kaeri_meta::anilist::Season, u16) {
    season_at(now_epoch())
}

/// The season containing `now`.
pub fn season_at(now: i64) -> (kaeri_meta::anilist::Season, u16) {
    use kaeri_meta::anilist::Season;
    let days = now / 86_400;
    // Civil-from-days, the standard algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (Season::of_month(month as u8), year as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_season_is_plausible() {
        let (_, year) = current_season();
        assert!((2024..2100).contains(&year), "derived year {year} is not plausible");
    }

    #[test]
    fn season_derivation_matches_known_dates() {
        use kaeri_meta::anilist::Season;
        // Verified against the civil-from-days algorithm.
        let cases = [
            (1_704_067_200_i64, Season::Winter, 2024u16), // 2024-01-01
            (1_719_792_000, Season::Summer, 2024),        // 2024-07-01
            (1_727_740_800, Season::Fall, 2024),          // 2024-10-01
        ];
        for (epoch, season, year) in cases {
            let days = epoch / 86_400;
            let z = days + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z.rem_euclid(146_097);
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let derived_year = if month <= 2 { y + 1 } else { y };
            assert_eq!(Season::of_month(month as u8), season, "for epoch {epoch}");
            assert_eq!(derived_year as u16, year, "for epoch {epoch}");
        }
    }
}
