//! "Up next for you": what to suggest under a short Continue, from what the viewer did here and
//! their tracker list, and only then from outside.
//!
//! In this order: new seasons (sequels of what they finished, not started), Planning, AniList's
//! recommendations from what they finished. What airs today is Home's own section. Never a title whose prequel is unfinished, never one already in the queue or started,
//! never one dismissed with Not interested. At most [`SUGGESTIONS_SHOWN`] are sent, whatever the
//! length of the queue.
//!
//! Asked of AniList by id, in batches, and kept as ids for half an hour like Search's rails.

use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
};

use kaeri_core::ids::AnilistId;
use kaeri_meta::{Media, Meta};
use kaeri_model::{
    catalog::{AiringStatus, Entry, LibrarySegment},
    event::Event,
    home::SUGGESTIONS_SHOWN,
    up_next::{UpNextAction, UpNextItem, UpNextReason},
};
use kaeri_store::Store;
use serde::{Deserialize, Serialize};

use crate::{
    catalog::filter::{DISMISSED_KEY, Dismissed, Viewer, dismissed},
    data,
    dispatch::Lent,
};

/// How long what was found is served before it is asked again.
const KEPT_FOR: i64 = 30 * 60;
const KEPT_KEY: &str = "up-next:kept";
/// Finished titles looked up for their sequels: the most recent watched here, and the most
/// recent on the list as Completed. Together one request of at most fifty.
const WATCHED_PROBED: u32 = 25;
const COMPLETED_PROBED: usize = 25;
/// Sequels and Planning titles looked up together, one request of at most fifty.
const SEQUELS_LOOKED_UP: usize = 30;
const PLANNING_LOOKED_UP: usize = 20;
/// The most titles one lookup by id carries: AniList's page of media.
const MEDIA_PER_REQUEST: usize = 50;
/// Recommended titles looked up, enough to fill the room after the ones that are filtered out.
const RECOMMENDED_LOOKED_UP: usize = 18;

/// What was found, as kept: the ids and why, and what they were found from.
#[derive(Serialize, Deserialize)]
struct Kept {
    at: i64,
    from: u64,
    picks: Vec<Pick>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Pick {
    pub id: u32,
    pub reason: Why,
    pub because: Option<String>,
    /// The title's first episode is not out yet, from the schedule that suggested it.
    pub unaired: bool,
    /// The viewer's list has it as Planning.
    #[serde(default)]
    pub planned: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Why {
    NewSeason,
    Planning,
    Recommended,
}

impl From<Why> for UpNextReason {
    fn from(why: Why) -> Self {
        match why {
            Why::NewSeason => Self::NewSeason,
            Why::Planning => Self::Planning,
            Why::Recommended => Self::Recommended,
        }
    }
}

/// What the load is told about the viewer and the moment.
pub struct Ask {
    /// A tracker is signed in, so its list counts.
    pub listed: bool,
    /// Adult titles may be suggested.
    pub adult: bool,
    pub now: i64,
    pub zone: jiff::tz::TimeZone,
}

/// Send "Up next for you": at once from what was kept when it is fresh, and otherwise after the
/// lookups. Nothing is sent when they fail; a second rail is not worth a toast, and what is on
/// screen stays.
pub async fn load_up_next(lent: Lent, ask: Ask) {
    // A lookup that failed is asked for again after AniList's own wait, a few times, and the
    // shelf is sent again as each answer fills it in.
    for attempt in 0..=crate::catalog::RETRIES {
        let done = if attempt == 0 {
            up_next_once(&lent, &ask).await
        } else {
            // Asked again behind whatever is on screen by then.
            tokio::time::sleep(crate::catalog::retry_wait(&lent.meta).await).await;
            kaeri_meta::in_background(up_next_once(&lent, &ask)).await
        };
        if done {
            return;
        }
    }
}

/// One go at Up next. Whether it needs no further go: it was sent whole, or could not be found.
async fn up_next_once(lent: &Lent, ask: &Ask) -> bool {
    let Lent { meta, store, tx, .. } = lent;
    let queue = queue(store);
    let room = SUGGESTIONS_SHOWN;
    // Nothing watched yet has nothing to suggest from.
    if store.stats().is_ok_and(|s| s.titles == 0) {
        let _ = tx.send(Event::UpNext(Vec::new()));
        return true;
    }
    let dismissed = dismissed(store);
    let from = signature(store, &queue, &dismissed, ask, room);
    if let Some(kept) = kept(store)
        && kept.from == from
        && ask.now - kept.at < KEPT_FOR
        && let Some(items) = items_from_cache(meta, store, &kept.picks)
    {
        let _ = tx.send(Event::UpNext(items));
        return true;
    }
    let (picks, complete) = match find(meta, store, &queue, &dismissed, ask, room, None).await {
        Ok(found) => (found.picks, found.complete),
        Err(e) => {
            tracing::warn!(error = %e, "Up next could not be found");
            return false;
        }
    };
    let cut: Vec<Pick> = picks.iter().take(room).map(|(pick, _)| pick.clone()).collect();
    let items = picks.into_iter().take(room).map(|(pick, entry)| item(&pick, entry)).collect();
    let kept = Kept { at: ask.now, from, picks: cut };
    // A lookup that failed leaves the others' picks on screen but not kept: the next ask finds
    // the rest instead of serving a thin shelf for half an hour.
    if complete
        && let Ok(json) = serde_json::to_string(&kept)
        && let Err(e) = store.set_meta_text(KEPT_KEY, &json)
    {
        tracing::warn!(error = %e, "could not keep Up next");
    }
    let _ = tx.send(Event::UpNext(items));
    complete
}

/// Not interested in `id`: hidden until it starts airing, and for good if it already has.
pub async fn dismiss(lent: Lent, id: AnilistId) {
    let Lent { meta, store, tx, .. } = lent;
    let known = meta.cached(&[id]);
    let media = match known.into_iter().next() {
        Some(media) => Some(media),
        None => meta.media_many(&[id]).await.ok().and_then(|found| found.into_iter().next()),
    };
    // Not knowing its status is hidden for good: showing it again would be the worse mistake.
    let waiting = media.is_some_and(|m| {
        data::entry_from(&m, None).status == Some(AiringStatus::NotYetReleased)
    });
    let mut all = dismissed(&store);
    all.retain(|d| d.id != id.get());
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    all.push(Dismissed { id: id.get(), waiting, at });
    match serde_json::to_string(&all) {
        Ok(json) => {
            if let Err(e) = store.set_meta_text(DISMISSED_KEY, &json) {
                tracing::warn!(error = %e, "could not keep that Up next was dismissed");
                let _ = tx.send(Event::Toast(kaeri_model::notice::Notice::alert(
                    kaeri_model::Msg::NoticeContinueHideFailed { reason: e.to_string() },
                )));
            }
        }
        Err(e) => tracing::warn!(error = %e, "could not write the dismissals"),
    }
}

/// Undo of [`dismiss`]: `id` may be suggested again, and the suggestions are sent anew.
pub async fn restore(lent: Lent, id: AnilistId, ask: Ask) {
    let mut all = dismissed(&lent.store);
    all.retain(|d| d.id != id.get());
    match serde_json::to_string(&all) {
        Ok(json) => {
            if let Err(e) = lent.store.set_meta_text(DISMISSED_KEY, &json) {
                tracing::warn!(error = %e, "could not undo the dismissal");
            }
        }
        Err(e) => tracing::warn!(error = %e, "could not write the dismissals"),
    }
    load_up_next(lent, ask).await;
}

/// Which titles are on Continue, to tell when the queue has changed: a show finished, one
/// started, a row removed. The order they are in does not count.
pub fn queue_mark(store: &Store) -> Vec<u32> {
    let mut ids: Vec<u32> = queue(store).iter().map(|id| id.get()).collect();
    ids.sort_unstable();
    ids
}

/// The ids on Continue, the queue the suggestions sit under.
fn queue(store: &Store) -> Vec<AnilistId> {
    store.continue_list(15).unwrap_or_default().iter().map(|p| p.anilist_id).collect()
}

fn kept(store: &Store) -> Option<Kept> {
    store
        .get_meta_text(KEPT_KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
}

/// What the answer depends on, for telling a kept one from a stale one: the queue, the history,
/// the list, the dismissals and the settings.
fn signature(
    store: &Store,
    queue: &[AnilistId],
    dismissed: &[Dismissed],
    ask: &Ask,
    room: usize,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    room.hash(&mut hasher);
    ask.listed.hash(&mut hasher);
    ask.adult.hash(&mut hasher);
    queue.hash(&mut hasher);
    for d in dismissed {
        (d.id, d.waiting).hash(&mut hasher);
    }
    for p in store.watched_titles(60).unwrap_or_default() {
        (p.anilist_id, p.episodes_done, p.updated_at).hash(&mut hasher);
    }
    if ask.listed {
        for status in ["COMPLETED", "PLANNING"] {
            store.library_with_status(status).unwrap_or_default().hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn items_from_cache(meta: &Meta, store: &Store, picks: &[Pick]) -> Option<Vec<UpNextItem>> {
    let ids: Vec<AnilistId> = picks.iter().map(|p| AnilistId::new(p.id)).collect();
    let media = meta.cached(&ids);
    if media.len() != picks.len() {
        return None;
    }
    Some(
        picks
            .iter()
            .zip(media)
            .map(|(pick, media)| item(pick, data::entry_from(&media, Some(store))))
            .collect(),
    )
}

pub(crate) fn item(pick: &Pick, entry: Entry) -> UpNextItem {
    let mut entry = entry;
    let planned = pick.planned;
    if planned {
        entry.list_status = Some(LibrarySegment::Planning);
    }
    let unaired = entry.status == Some(AiringStatus::NotYetReleased) || pick.unaired;
    let action = match (unaired, planned) {
        (false, _) => UpNextAction::PlayFirstEpisode,
        (true, false) => UpNextAction::AddToPlanning,
        (true, true) => UpNextAction::Nothing,
    };
    let starts = if unaired { entry.start_date } else { None };
    UpNextItem {
        reason: pick.reason.into(),
        because: pick.because.clone(),
        starts,
        action,
        entry,
    }
}

/// What [`find`] found: the suggestions, and what they were found from.
pub(crate) struct Found {
    /// In order, and at most as many as the room after the groups that cannot fill it.
    pub picks: Vec<(Pick, Entry)>,
    /// Every title finished, whichever way, including those on the list not looked up.
    pub finished_ids: HashSet<AnilistId>,
    /// Every lookup answered. When one failed, the picks are those the others found: shown, but
    /// not kept as the answer, and asked for again.
    pub complete: bool,
    /// With [`Because`] asked: the most recent title finished that rated well enough, and the
    /// titles rated highest off it, best first, or why they could not be found.
    pub because: Option<(Entry, Result<Vec<Media>, String>)>,
}

/// Home's "Because you finished", found with the suggestions: off the most recent title finished
/// scored at least `score`, the `each` titles rated highest.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Because {
    pub score: u32,
    pub each: u32,
}

/// The suggestions, in order, and as many as the room, with the groups that cannot fill it
/// followed by the ones that can.
pub(crate) async fn find(
    meta: &Meta,
    store: &Store,
    queue: &[AnilistId],
    dismissed: &[Dismissed],
    ask: &Ask,
    room: usize,
    because: Option<Because>,
) -> Result<Found, String> {
    let queue: HashSet<AnilistId> = queue.iter().copied().collect();
    let status_of = |id: AnilistId| -> Option<String> {
        ask.listed.then(|| store.library_status(id).ok().flatten()).flatten()
    };
    let started = |id: AnilistId| -> bool {
        store.progress(id).ok().flatten().is_some()
            || status_of(id).is_some_and(|s| s != "PLANNING")
    };

    // What was finished: every episode watched here, or Completed on the list.
    let watched = store.watched_titles(WATCHED_PROBED).unwrap_or_default();
    let completed: Vec<AnilistId> = if ask.listed {
        store.library_with_status("COMPLETED").unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut probe: Vec<AnilistId> = watched.iter().map(|p| p.anilist_id).collect();
    for id in completed.iter().take(COMPLETED_PROBED) {
        if !probe.contains(id) {
            probe.push(*id);
        }
    }
    // The Planning titles ride along with the probe, as far as one request of fifty holds them:
    // they are known already, and asking for them apart is a request for nothing new.
    let mut planning_ids: Vec<AnilistId> = if ask.listed {
        store.library_with_status("PLANNING").unwrap_or_default()
    } else {
        Vec::new()
    };
    planning_ids.retain(|id| !queue.contains(id) && !started(*id));
    planning_ids.truncate(PLANNING_LOOKED_UP);
    let probe_len = probe.len();
    let with_probe = MEDIA_PER_REQUEST.saturating_sub(probe_len);
    for id in planning_ids.iter().take(with_probe) {
        if !probe.contains(id) {
            probe.push(*id);
        }
    }
    let probed = meta.media_many(&probe).await.map_err(|e| e.to_string())?;
    let done: HashMap<AnilistId, u32> =
        watched.iter().map(|p| (p.anilist_id, p.episodes_done)).collect();
    let mut finished: HashSet<AnilistId> = completed.iter().copied().collect();
    let mut finished_entries: Vec<Entry> = Vec::new();
    for media in &probed {
        let entry = data::entry_from(media, Some(store));
        let complete = entry
            .episodes
            .filter(|n| *n > 0)
            .is_some_and(|total| done.get(&entry.id).copied().unwrap_or(0) >= total);
        if complete || completed.contains(&entry.id) {
            finished.insert(entry.id);
            finished_entries.push(entry);
        }
    }
    // Nothing above can have a sequel left over a title it names itself, so probe order is the
    // order of recency: watched here first, then the list.
    let order = |id: &AnilistId| probe.iter().position(|p| p == id).unwrap_or(usize::MAX);
    finished_entries.sort_by_key(|e| order(&e.id));

    let viewer = Viewer::new(
        store,
        ask.listed,
        ask.adult,
        queue.iter().copied(),
        finished.clone(),
        dismissed.to_vec(),
    );
    let suggestible = |entry: &Entry| -> bool { viewer.eligible(entry) };

    // New seasons and Planning, in one lookup.
    let mut sequel_ids: Vec<AnilistId> = Vec::new();
    for entry in &finished_entries {
        for id in &entry.sequels {
            if !sequel_ids.contains(id) && !queue.contains(id) && !started(*id) {
                sequel_ids.push(*id);
            }
        }
    }
    sequel_ids.truncate(SEQUELS_LOOKED_UP);
    planning_ids.retain(|id| !sequel_ids.contains(id));
    // Only the titles the probe did not already carry are asked for.
    let wanted: Vec<AnilistId> = sequel_ids
        .iter()
        .chain(&planning_ids)
        .filter(|id| !probed.iter().any(|m| m.id == **id))
        .copied()
        .collect();
    // What is recommended off the titles finished most recently, and with `because` off the
    // one Home names, go in the same request when they are wanted whatever the lookup finds:
    // the new seasons and Planning can make no more picks than they have titles.
    let sources: Vec<AnilistId> = finished_entries
        .iter()
        .take(kaeri_meta::anilist::query::RECOMMENDED_FROM)
        .map(|e| e.id)
        .collect();
    let because_from: Option<Entry> = because.and_then(|because| {
        finished_entries.iter().find(|e| e.score.is_some_and(|s| s >= because.score)).cloned()
    });
    let recommending = sequel_ids.len() + planning_ids.len() < room && !sources.is_empty();
    let asked = meta
        .suggestions(
            &wanted,
            if recommending { &sources } else { &[] },
            because_from.as_ref().zip(because).map(|(e, b)| (e.id, b.each)),
        )
        .await;
    let found = asked.titles.map_err(|e| e.to_string())?;
    let entries: HashMap<AnilistId, Entry> =
        probed.iter().chain(&found).map(|m| (m.id, data::entry_from(m, Some(store)))).collect();

    let mut picks: Vec<(Pick, Entry)> = Vec::new();
    let group = |ids: &[AnilistId], why: Why, picks: &mut Vec<(Pick, Entry)>| {
        let mut rows: Vec<&Entry> = ids
            .iter()
            .filter_map(|id| entries.get(id))
            .filter(|entry| suggestible(entry))
            // A title that is announced without a date says nothing yet; on Planning it is the
            // viewer's own, and is shown last.
            .filter(|entry| why == Why::Planning || rank(entry) < 3)
            .collect();
        rows.sort_by_key(|entry| rank(entry));
        for entry in rows {
            picks.push((
                Pick {
                    id: entry.id.get(),
                    reason: why,
                    because: None,
                    unaired: false,
                    planned: status_of(entry.id).as_deref() == Some("PLANNING"),
                },
                entry.clone(),
            ));
        }
    };
    group(&sequel_ids, Why::NewSeason, &mut picks);
    group(&planning_ids, Why::Planning, &mut picks);

    // What people who liked what was finished rated highest.
    let mut complete = true;
    let mut candidates: Vec<(AnilistId, String)> = Vec::new();
    if picks.len() < room && !finished_entries.is_empty() {
        let from = match asked.from {
            Some(from) => from,
            None => meta.recommended_from(&sources).await,
        };
        match from {
            Ok(from) => {
                let longest = from.iter().map(|(_, _, recs)| recs.len()).max().unwrap_or(0);
                // One from each title in turn, so the most recent does not take them all.
                for n in 0..longest {
                    for (_, name, recs) in &from {
                        let Some(id) = recs.get(n) else { continue };
                        let known = queue.contains(id)
                            || started(*id)
                            || finished.contains(id)
                            || status_of(*id).is_some()
                            || picks.iter().any(|(p, _)| p.id == id.get())
                            || candidates.iter().any(|(c, _)| c == id);
                        if !known {
                            candidates.push((*id, name.clone()));
                        }
                    }
                }
                candidates.truncate(RECOMMENDED_LOOKED_UP);
            }
            Err(e) => {
                complete = false;
                tracing::warn!(error = %e, "recommendations failed");
            }
        }
    }
    // The recommended titles and Home's "Because you finished" are looked up together.
    let because_ids: Option<Result<Vec<AnilistId>, String>> =
        asked.because.map(|ids| ids.map_err(|e| e.to_string()));
    let mut ids: Vec<AnilistId> = candidates.iter().map(|(id, _)| *id).collect();
    for id in because_ids.iter().flatten().flatten() {
        if !ids.contains(id) {
            ids.push(*id);
        }
    }
    let looked_up = if ids.is_empty() {
        Ok(Vec::new())
    } else {
        meta.media_many(&ids).await.map_err(|e| e.to_string())
    };
    match &looked_up {
        Ok(media) => {
            for media in media.iter().filter(|m| candidates.iter().any(|(id, _)| *id == m.id)) {
                let entry = data::entry_from(media, Some(store));
                if !suggestible(&entry) {
                    continue;
                }
                let because = candidates
                    .iter()
                    .find(|(id, _)| *id == entry.id)
                    .map(|(_, name)| name.clone());
                picks.push((
                    Pick {
                        id: entry.id.get(),
                        reason: Why::Recommended,
                        because,
                        unaired: false,
                        planned: false,
                    },
                    entry,
                ));
            }
        }
        Err(e) if !candidates.is_empty() => {
            complete = false;
            tracing::warn!(error = %e, "recommended titles failed");
        }
        Err(_) => {}
    }
    let because = because_from.zip(because_ids).map(|(source, ids)| {
        let titles = ids.and_then(|ids| {
            let media = looked_up.clone()?;
            Ok(ids
                .iter()
                .filter_map(|id| media.iter().find(|m| m.id == *id).cloned())
                .collect())
        });
        (source, titles)
    });

    Ok(Found { picks, finished_ids: finished, complete, because })
}

/// Airing first, then announced with a date, then finished, then announced without one.
fn rank(entry: &Entry) -> u8 {
    match entry.status {
        Some(AiringStatus::Releasing | AiringStatus::Hiatus) => 0,
        Some(AiringStatus::NotYetReleased) if entry.start_date.is_some() => 1,
        Some(AiringStatus::NotYetReleased) => 3,
        _ => 2,
    }
}
