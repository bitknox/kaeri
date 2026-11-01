//! The data a hover card or preview shows, for covers on screen: no request on a hover.

use kaeri_core::ids::AnilistId;
use kaeri_model::event::Event;

use crate::{data, dispatch::Lent};

/// The most titles one lookup carries.
const AT_MOST: usize = 50;

/// Say the cards of `ids`: first those the metadata cache holds, fresh, from the lists that
/// showed them, then the rest from one batched lookup at low priority. A title nothing could describe
/// is left out, and the card keeps what the list row had.
pub async fn load_cards(lent: Lent, ids: Vec<AnilistId>) {
    let Lent { meta, store, tx, .. } = lent;
    let wanted = at_most(ids);
    if wanted.is_empty() {
        return;
    }
    // Only records as fresh as Home keeps its own: the cache outlives launches, and an older
    // record would put last season's airing line over the row the list just brought. The rest
    // are asked for, and until they come the card keeps what the list row had.
    let missing = crate::catalog::home::stale_among(&store, wanted.clone());
    let fresh: Vec<AnilistId> =
        wanted.iter().copied().filter(|id| !missing.contains(id)).collect();
    let kept = meta.cached(&fresh);
    if !kept.is_empty() {
        let entries = kept.iter().map(|media| data::entry_from(media, Some(&store))).collect();
        let _ = tx.send(Event::Cards(entries));
    }
    let missing: Vec<AnilistId> =
        wanted.iter().copied().filter(|id| !kept.iter().any(|m| m.id == *id)).collect();
    if missing.is_empty() {
        return;
    }
    match kaeri_meta::in_background(meta.media_many(&missing)).await {
        Ok(found) if !found.is_empty() => {
            let entries =
                found.iter().map(|media| data::entry_from(media, Some(&store))).collect();
            let _ = tx.send(Event::Cards(entries));
        }
        Ok(_) => {}
        Err(e) => tracing::debug!(error = %e, "the hover cards could not be filled"),
    }
}

/// Say the cards of the covers a screen shows, with what each one's episode table needs, from
/// one batched lookup at low priority: see `Meta::media_tables`. The tables are then built from
/// it when a hover asks, with no request of their own. Titles it brought already are said
/// again from memory.
pub async fn prepare_cards(lent: Lent, ids: Vec<AnilistId>) {
    let Lent { meta, store, tx, .. } = lent;
    let wanted = at_most(ids);
    if wanted.is_empty() {
        return;
    }
    let found = kaeri_meta::in_background(meta.media_tables(&wanted)).await;
    if !found.is_empty() {
        let entries = found.iter().map(|media| data::entry_from(media, Some(&store))).collect();
        let _ = tx.send(Event::Cards(entries));
    }
}

/// `ids` once each, the first [`AT_MOST`] of them.
fn at_most(ids: Vec<AnilistId>) -> Vec<AnilistId> {
    let mut wanted: Vec<AnilistId> = Vec::new();
    for id in ids {
        if !wanted.contains(&id) && wanted.len() < AT_MOST {
            wanted.push(id);
        }
    }
    wanted
}
