//! The data a hover card or preview shows, for covers on screen: no request on a hover.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kaeri_core::ids::AnilistId;
use kaeri_meta::{Lane, Priority};
use kaeri_model::event::Event;

use crate::{data, dispatch::Lent};

/// The most titles one lookup carries.
const AT_MOST: usize = 50;

/// Say the cards of `ids`: first those the metadata cache holds, fresh, from the lists that
/// showed them, then the rest from one batched lookup as work for the page on screen: someone
/// is looking at the card, which shows without it and fills in when it lands. A title nothing
/// could describe is left out, and the card keeps what the list row had.
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
    match kaeri_meta::on_screen(meta.media_many(&missing)).await {
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

/// The hover-card work under way, by title: a cover's card, the tables of a screen's covers,
/// and a card's stills. A cover hovered again while its first ask is still out joins that ask
/// and starts nothing, so its job ends at once and the activity list holds one row for it.
#[derive(Debug, Clone, Default)]
pub struct HoverAsks {
    /// [`load_cards`] under way.
    pub cards: UnderWay,
    /// [`prepare_cards`] under way.
    pub tables: UnderWay,
    /// The quiet episode listings for a card's stills under way.
    pub stills: UnderWay,
}

/// Titles some work is asking after, each with the lane that work's AniList requests go in.
#[derive(Debug, Clone, Default)]
pub struct UnderWay(Arc<Mutex<HashMap<AnilistId, Lane>>>);

impl UnderWay {
    /// Whether title `id` is being asked after. When it is, its ask is raised to `priority`,
    /// for a joiner more urgent than the work that asked.
    pub fn join(&self, id: AnilistId, priority: Priority) -> bool {
        let asks = self.0.lock().unwrap_or_else(|e| e.into_inner());
        asks.get(&id).inspect(|lane| lane.raise(priority)).is_some()
    }

    /// Take the titles of `ids` nothing here is asking after, for work in a new lane of
    /// `priority`, and raise the asks of the others to it. The titles are let go when the
    /// claim is dropped, however the work ends.
    pub fn claim(&self, ids: impl IntoIterator<Item = AnilistId>, priority: Priority) -> Claim {
        let lane = Lane::new(priority);
        let mut asks = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let mut taken = Vec::new();
        for id in ids {
            match asks.get(&id) {
                Some(under_way) => under_way.raise(priority),
                None => {
                    asks.insert(id, lane.clone());
                    taken.push(id);
                }
            }
        }
        Claim { under_way: self.clone(), ids: taken, lane }
    }
}

/// Titles taken by one piece of work: see [`UnderWay::claim`].
#[derive(Debug)]
pub struct Claim {
    under_way: UnderWay,
    ids: Vec<AnilistId>,
    lane: Lane,
}

impl Claim {
    /// The titles this work asks after: none when others were asking after all of them.
    pub fn ids(&self) -> &[AnilistId] {
        &self.ids
    }

    /// The lane the work's AniList requests go in, which a joiner raises.
    pub fn lane(&self) -> Lane {
        self.lane.clone()
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        let mut asks = self.under_way.0.lock().unwrap_or_else(|e| e.into_inner());
        for id in &self.ids {
            if asks.get(id).is_some_and(|lane| lane.same(&self.lane)) {
                asks.remove(id);
            }
        }
    }
}

/// `ids` once each, the first [`AT_MOST`] of them.
pub(crate) fn at_most(ids: Vec<AnilistId>) -> Vec<AnilistId> {
    let mut wanted: Vec<AnilistId> = Vec::new();
    for id in ids {
        if !wanted.contains(&id) && wanted.len() < AT_MOST {
            wanted.push(id);
        }
    }
    wanted
}
