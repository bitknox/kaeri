//! The data a hover card or preview shows, for covers on screen: no request on a hover.

use kaeri_core::ids::AnilistId;
use kaeri_model::event::Event;

use crate::{data, dispatch::Lent};

/// The most titles one lookup carries.
const AT_MOST: usize = 50;

/// Say the cards of `ids`: first those the metadata cache holds from the lists that showed
/// them, then the rest from one batched lookup at low priority. A title nothing could describe
/// is left out, and the card keeps what the list row had.
pub async fn load_cards(lent: Lent, ids: Vec<AnilistId>) {
    let Lent { meta, store, tx, .. } = lent;
    let mut wanted: Vec<AnilistId> = Vec::new();
    for id in ids {
        if !wanted.contains(&id) && wanted.len() < AT_MOST {
            wanted.push(id);
        }
    }
    if wanted.is_empty() {
        return;
    }
    let kept = meta.cached(&wanted);
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
