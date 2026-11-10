//! Reading on the trackers: what a series' Tracking section shows, the choice of which entry a
//! work is on a tracker, a conflict settled by hand, and the status and score changes that go to
//! the trackers' manga and novel lists.
//!
//! Local state stays the truth. A change is queued in the reading outbox of each signed-in
//! tracker that keeps such a list, and the drain sends it; see `kaeri_track::sync::drain_read`.
//! A chapter like 10.5 goes to a tracker as 10, and the series says so.

use std::sync::Arc;

use kaeri_core::{
    books::WorkId,
    ids::AnilistId,
    traits::{ReadRemoval, TrackOp, Tracker, WatchStatus},
};
use kaeri_model::{
    Msg, ReadingMsg,
    event::Event,
    reading::{
        LibrarySegmentRead, ListRemoval, ListRemovalOutcome, ReadingEvent, ReadingFailure,
        TrackerMapping, TrackerReadRow,
    },
};
use kaeri_store::Store;

use crate::{
    outbox::Outbox, services::Services, tracking::commands::signed_in_reading_tracker_ids,
};

/// The AniList id behind a work, for a work AniList lists. A folder's series have none, and no
/// tracker is asked about them.
fn anilist_of(work: &WorkId) -> Option<AnilistId> {
    work.as_str().strip_prefix("a:")?.parse().ok().map(AnilistId::new)
}

fn conflict_key(work: &str, tracker: &str) -> String {
    format!("read_conflict:{tracker}:{work}")
}

/// Remember that a pull found this work's status or score different from the tracker's.
pub(crate) fn note_conflict(store: &Store, id: AnilistId, tracker: &str) {
    let work = kaeri_track::sync::read_work(id);
    let _ = store.set_meta_i64(&conflict_key(&work, tracker), 1);
}

/// The note shown when the highest chapter read has a fraction: trackers count whole chapters.
fn split_note(store: &Store, work: &str) -> Option<Msg> {
    let highest = store
        .read_chapters(work)
        .ok()?
        .into_iter()
        .filter(|c| c.read)
        .filter_map(|c| c.chapter.trim().parse::<f64>().ok().map(|n| (n, c.chapter)))
        .max_by(|a, b| a.0.total_cmp(&b.0))?;
    if highest.0.fract() == 0.0 {
        return None;
    }
    Some(Msg::Reading(ReadingMsg::ReadingSplitChapter {
        chapter: highest.1.trim().to_owned(),
        whole: format!("{}", highest.0.floor()),
    }))
}

/// What the trackers hold for a work, from the chapter list in use: the whole chapters read, and
/// the highest volume whose every known chapter is read.
///
/// Where numbering restarts in each volume the chapters are counted in reading order, so a
/// series whose chapters begin again at 1 reports its true total. Otherwise the count is the
/// highest whole chapter number read, as before. `base` is what the store already knows (marks
/// and a watermark); the answer is never below it.
pub(crate) fn progress_in(
    list: &[crate::read_catalog::chapters::Kept],
    read: &crate::read_catalog::chapters::Read,
    base: (u32, Option<u32>),
) -> (u32, Option<u32>) {
    use std::collections::{HashMap, HashSet};
    // One row per chapter: several editions list the same one.
    let mut seen = HashSet::new();
    let rows: Vec<_> = list.iter().filter(|c| seen.insert(c.id.as_str())).collect();
    let volume_of = |c: &crate::read_catalog::chapters::Kept| {
        c.volume.as_deref().and_then(|v| v.trim().parse::<u32>().ok())
    };

    let mut by_number: HashMap<&str, HashSet<&str>> = HashMap::new();
    for c in &rows {
        if let (Some(number), Some(volume)) = (c.number.as_deref(), c.volume.as_deref()) {
            by_number.entry(number).or_default().insert(volume);
        }
    }
    let restarts = by_number.values().any(|volumes| volumes.len() > 1);

    let mut chapters = base.0;
    if restarts {
        // In reading order the chapters read up to the last one read, whole numbers only: a
        // half chapter between two others is not one more chapter to a tracker.
        let mut ordered: Vec<_> = rows.clone();
        ordered.sort_by_key(|c| (volume_of(c).unwrap_or(u32::MAX), c.range()));
        let whole = |c: &crate::read_catalog::chapters::Kept| {
            c.range().is_none_or(|r| !r.start.is_split())
        };
        let counted = ordered.iter().filter(|c| whole(c) && read.is_read(&c.id)).count();
        chapters = chapters.max(u32::try_from(counted).unwrap_or(u32::MAX));
    }

    // A volume counts when it has chapters and every one of them is read.
    let mut volumes: HashMap<u32, bool> = HashMap::new();
    for c in &rows {
        if let Some(v) = volume_of(c) {
            let all = volumes.entry(v).or_insert(true);
            *all = *all && read.is_read(&c.id);
        }
    }
    let finished = volumes.into_iter().filter(|(_, all)| *all).map(|(v, _)| v).max();
    (chapters, base.1.max(finished))
}

/// [`progress_in`] for a work, from the list its edition in use keeps. Without a list it is the
/// store's own figures.
pub(crate) fn progress_of(services: &Services, work: &WorkId) -> (u32, Option<u32>) {
    let base = services.store.read_progress(work.as_str()).unwrap_or((0, None));
    let Some((_, _, list)) =
        crate::read_catalog::chapters::of_work(&services.book_sources, &services.store, work)
    else {
        return base;
    };
    let read = crate::read_catalog::chapters::Read::of(&services.store, work.as_str());
    let found = progress_in(&list, &read, base);
    if let Err(e) = services.store.set_tracked_progress(work.as_str(), found.0, found.1) {
        tracing::warn!(error = %e, "the tracker progress was not kept");
    }
    found
}

/// Queue a status change for the trackers' reading lists. Read again is a status of its own
/// here and goes as Reading with `rereading` set, which each tracker spells its way.
pub(crate) fn queue_status(
    services: &Services,
    work: &WorkId,
    status: LibrarySegmentRead,
    at: i64,
    tx: &Outbox,
) {
    let Some(id) = anilist_of(work) else { return };
    let (status, rereading) = match status {
        LibrarySegmentRead::Reading => (WatchStatus::Current, false),
        LibrarySegmentRead::Rereading => (WatchStatus::Current, true),
        LibrarySegmentRead::Planning => (WatchStatus::Planning, false),
        LibrarySegmentRead::Completed => (WatchStatus::Completed, false),
        LibrarySegmentRead::Paused => (WatchStatus::Paused, false),
        LibrarySegmentRead::Dropped => (WatchStatus::Dropped, false),
        LibrarySegmentRead::YourFiles => return,
    };
    queue(services, &TrackOp::SetReadStatus { anilist_id: id, status, rereading, at }, at, tx);
}

/// Queue a score, out of ten, for the trackers' reading lists.
pub(crate) fn queue_score(
    services: &Services,
    work: &WorkId,
    score: Option<f32>,
    at: i64,
    tx: &Outbox,
) {
    let (Some(id), Some(score)) = (anilist_of(work), score.filter(|s| *s > 0.0)) else {
        return;
    };
    queue(services, &TrackOp::SetReadScore { anilist_id: id, score, at }, at, tx);
}

/// Queue `progress` for the trackers when it took the work's count down: an unread, when the
/// person chose that the latest change wins. It replaces any higher count still queued.
pub(crate) fn queue_lower(
    services: &Services,
    work: &WorkId,
    (chapter, volume): (u32, Option<u32>),
    at: i64,
    tx: &Outbox,
) {
    let Some(id) = anilist_of(work) else { return };
    let op = TrackOp::SetReadProgress { anilist_id: id, chapter, volume };
    let mut queued = false;
    for tracker in signed_in_reading_tracker_ids(&services.sync) {
        match services.store.requeue_read_progress(&tracker, &op, at) {
            Ok(_) => queued = true,
            Err(e) => tracing::warn!(error = %e, "a reading change was not queued"),
        }
    }
    if queued {
        for state in services.sync.state_after_read_enqueue() {
            let _ = tx.send(Event::Sync(Box::new(state)));
        }
    }
}

fn queue(services: &Services, op: &TrackOp, at: i64, tx: &Outbox) {
    let mut queued = false;
    for tracker in signed_in_reading_tracker_ids(&services.sync) {
        match services.store.enqueue(&tracker, op, at) {
            Ok(_) => queued = true,
            Err(e) => tracing::warn!(error = %e, "a reading change was not queued"),
        }
    }
    if queued {
        for state in services.sync.state_after_read_enqueue() {
            let _ = tx.send(Event::Sync(Box::new(state)));
        }
    }
}

/// Take a series off the lists: the person's here at once, then each signed-in reading
/// tracker's, answered with one result per tracker. The chapters read stay read.
pub(crate) fn remove_from_lists(services: &Services, work: WorkId, tx: &Outbox) {
    let trackers: Vec<Arc<dyn Tracker>> = services
        .sync
        .trackers
        .iter()
        .filter(|t| t.serves_reading() && t.is_authenticated())
        .cloned()
        .collect();
    if forget_here(&services.store, &work, &trackers, services.clock.now()) > 0 {
        for state in services.sync.state_after_read_enqueue() {
            let _ = tx.send(Event::Sync(Box::new(state)));
        }
    }
    let tx = tx.clone();
    tokio::spawn(async move {
        let results = match anilist_of(&work) {
            Some(id) => remove_there(&trackers, id).await,
            // A folder's series is on no tracker.
            None => Vec::new(),
        };
        let _ = tx.send(Event::Reading(ReadingEvent::RemovedFromLists { work, results }));
    });
}

/// Take a series off the lists here, and drop what still waited to go to a tracker about it, so
/// a queued status cannot put it back. Says how many queued changes went.
fn forget_here(store: &Store, work: &WorkId, trackers: &[Arc<dyn Tracker>], now: i64) -> usize {
    let name = work.as_str();
    if let Err(e) = store.set_read_status(name, None, now) {
        tracing::warn!(error = %e, "the series was not taken off the lists here");
    }
    for tracker in trackers {
        let _ = store.delete_meta(&conflict_key(name, tracker.id()));
    }
    let Some(id) = anilist_of(work) else { return 0 };
    store.forget_read_ops(id).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "queued reading changes were not dropped");
        0
    })
}

/// Ask every tracker at once to take the title off its list.
async fn remove_there(trackers: &[Arc<dyn Tracker>], id: AnilistId) -> Vec<ListRemoval> {
    let asks = trackers.iter().map(|tracker| async move {
        let answer = tracker.remove_read(id).await;
        removal(tracker.id(), answer)
    });
    futures::future::join_all(asks).await
}

/// One tracker's answer to a removal, with its words.
fn removal(tracker: &str, answer: Result<ReadRemoval, kaeri_core::Error>) -> ListRemoval {
    let name = crate::tracking::tracker_name(tracker);
    let r = Msg::Reading;
    let (outcome, line) = match answer {
        Ok(ReadRemoval::Removed) => (
            ListRemovalOutcome::Removed,
            vec![r(ReadingMsg::ReadingRemovedFrom { tracker: name })],
        ),
        Ok(ReadRemoval::NotOnList) => (
            ListRemovalOutcome::NotOnList,
            vec![r(ReadingMsg::ReadingRemovedNotThere { tracker: name })],
        ),
        Ok(ReadRemoval::Unmatched) => (
            ListRemovalOutcome::Unmatched,
            vec![r(ReadingMsg::ReadingRemovedUnmatched { tracker: name })],
        ),
        Err(e) => {
            tracing::info!(tracker, error = %e, "a tracker did not take the series off its list");
            let (failure, why) = match e {
                kaeri_core::Error::Auth(_) => (
                    ReadingFailure::SignInNeeded { what: name.clone() },
                    r(ReadingMsg::ReadingRemovedSignIn { tracker: name.clone() }),
                ),
                kaeri_core::Error::Network(_) => {
                    (ReadingFailure::Offline, ReadingFailure::Offline.msg())
                }
                other => {
                    let failure = ReadingFailure::Other { reason: other.to_string() };
                    let why = failure.msg();
                    (failure, why)
                }
            };
            (
                ListRemovalOutcome::Failed { failure },
                vec![r(ReadingMsg::ReadingRemovedFailed { tracker: name }), why],
            )
        }
    };
    ListRemoval { tracker: tracker.to_owned(), outcome, line }
}

/// Note the ids a book already carries, so the first sync has a MyAnimeList entry to write to.
/// AniList's own `idMal` is the first answer in the mapping order.
pub(crate) fn note_book(store: &Store, id: AnilistId, id_mal: Option<u32>, at: i64) {
    let Some(mal) = id_mal else { return };
    let work = kaeri_track::sync::read_work(id);
    if store.track_map_id(&work, "mal").ok().flatten().is_some() {
        return;
    }
    if let Err(e) = store.set_track_map(&work, "mal", Some(&mal.to_string()), "anilist", at) {
        tracing::warn!(error = %e, "the MyAnimeList id was not kept");
    }
}

/// How long a lookup that found nothing stands before it is asked again.
const RECHECK: i64 = 7 * 24 * 3600;

/// A match is acted on without asking at this confidence, and when it leads the next by
/// [`LEAD`]: the rule folders use.
const SURE: f64 = crate::local::SURE;
const LEAD: f64 = 0.05;

fn candidates_key(work: &str, tracker: &str) -> String {
    format!("read_candidates:{tracker}:{work}")
}

/// A MangaUpdates series offered for a work, as kept between the lookup and the person's answer.
#[derive(serde::Serialize, serde::Deserialize)]
struct Offer {
    id: String,
    title: String,
    year: Option<u16>,
    confidence: u8,
}

fn offers(store: &Store, work: &str, tracker: &str) -> Vec<kaeri_model::reading::MapCandidate> {
    store
        .get_meta_text(&candidates_key(work, tracker))
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<Vec<Offer>>(&json).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|o| kaeri_model::reading::MapCandidate {
            tracker_id: o.id,
            title: o.title,
            year: o.year,
            confidence: o.confidence,
        })
        .collect()
}

/// Whether a tracker still needs this work looked up: no answer yet, or an empty one that has
/// stood a while. A choice made by hand, or an id, is never asked again.
fn needs_lookup(maps: &[kaeri_store::reading::TrackMap], tracker: &str, now: i64) -> bool {
    match maps.iter().find(|m| m.tracker == tracker) {
        None => true,
        Some(m) if m.origin == "hand" || m.tracker_id.is_some() => false,
        Some(m) => now - m.checked_at > RECHECK,
    }
}

/// Match a work to the entries it is on at the signed-in trackers, in the background.
///
/// Nothing is asked unless a tracker that needs the answer is signed in, so someone who never
/// connects MangaUpdates, MyAnimeList or Kitsu makes no lookup. In order: MyAnimeList from
/// AniList's own `idMal`, else MangaBaka; Kitsu from its mappings, else MangaBaka; MangaUpdates
/// from MangaBaka, else a title search, where an unsure match is offered to the person.
pub(crate) fn resolve_for(services: &Services, work: &WorkId, tx: &Outbox) {
    let Some(id) = anilist_of(work).map(|id| id.get()) else { return };
    let now = kaeri_store::now();
    let maps = services.store.track_map(work.as_str()).unwrap_or_default();
    let wanted: Vec<String> = signed_in_reading_tracker_ids(&services.sync)
        .into_iter()
        .filter(|t| matches!(t.as_str(), "mal" | "kitsu" | "mangaupdates"))
        .filter(|t| needs_lookup(&maps, t, now))
        .collect();
    if wanted.is_empty() {
        return;
    }
    let Some(kept) = services.meta.kept_book(AnilistId::new(id)) else { return };
    let (store, lookups, tx, work) =
        (services.store.clone(), services.sync.lookups.clone(), tx.clone(), work.clone());
    let trackers = services.sync.trackers.clone();
    tokio::spawn(async move {
        let book = kept.book;
        let mut titles: Vec<String> =
            [&book.title.romaji, &book.title.english, &book.title.native]
                .into_iter()
                .flatten()
                .cloned()
                .chain(book.synonyms.iter().cloned())
                .collect();
        titles.dedup();
        let name = work.as_str();
        let keep = |tracker: &str, found: Option<String>, origin: &str| {
            if let Err(e) = store.set_track_map(name, tracker, found.as_deref(), origin, now) {
                tracing::warn!(error = %e, "a match was not kept");
            }
        };

        // MangaBaka, asked at most once and only if an earlier answer left a gap.
        let mut baka: Option<Option<kaeri_track::lookup::SourceIds>> = None;
        macro_rules! baka {
            () => {{
                if baka.is_none() {
                    baka = Some(match lookups.mangabaka(id).await {
                        Ok(found) => found,
                        Err(e) => {
                            tracing::info!(error = %e, "MangaBaka was not asked");
                            None
                        }
                    });
                }
                baka.flatten()
            }};
        }

        let mut mal = book.id_mal;
        if wanted.iter().any(|t| t == "mal") {
            match mal {
                Some(id) => keep("mal", Some(id.to_string()), "anilist"),
                None => {
                    mal = baka!().and_then(|ids| ids.my_anime_list);
                    keep("mal", mal.map(|m| m.to_string()), "mangabaka");
                }
            }
        }
        if wanted.iter().any(|t| t == "kitsu") {
            let mut found = lookups.kitsu_manga("anilist/manga", id).await.ok().flatten();
            let mut origin = "kitsu";
            if found.is_none()
                && let Some(mal) = mal
            {
                found = lookups.kitsu_manga("myanimelist/manga", mal).await.ok().flatten();
            }
            if found.is_none() {
                found = baka!().and_then(|ids| ids.kitsu);
                origin = "mangabaka";
            }
            keep("kitsu", found.map(|k| k.to_string()), origin);
        }
        if wanted.iter().any(|t| t == "mangaupdates") {
            let mut found = baka!().and_then(|ids| ids.manga_updates);
            if found.is_some() {
                keep("mangaupdates", found.map(|s| s.to_string()), "mangabaka");
            } else {
                found = None;
                let mut candidates = Vec::new();
                for title in titles.iter().take(2) {
                    match lookups.search_mangaupdates(title).await {
                        Ok(hits) if !hits.is_empty() => {
                            candidates = hits;
                            break;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::info!(error = %e, "MangaUpdates was not searched");
                            return;
                        }
                    }
                }
                let mut scored: Vec<_> = candidates
                    .iter()
                    .map(|c| {
                        let best = titles
                            .iter()
                            .map(|t| kaeri_meta::similarity(t, &c.title))
                            .fold(0.0_f64, f64::max);
                        (best, c)
                    })
                    .collect();
                scored.sort_by(|a, b| b.0.total_cmp(&a.0));
                if let Some((best, top)) = scored.first() {
                    let clear = scored.get(1).is_none_or(|next| best - next.0 >= LEAD);
                    if *best >= SURE && clear {
                        found = Some(top.series_id);
                    }
                }
                if found.is_none() && !scored.is_empty() {
                    let offered: Vec<Offer> = scored
                        .iter()
                        .take(5)
                        .map(|(score, c)| Offer {
                            id: c.series_id.to_string(),
                            title: c.title.clone(),
                            year: c.year,
                            confidence: (score * 100.0).round().clamp(0.0, 100.0) as u8,
                        })
                        .collect();
                    if let Ok(json) = serde_json::to_string(&offered) {
                        let _ =
                            store.set_meta_text(&candidates_key(name, "mangaupdates"), &json);
                    }
                }
                keep("mangaupdates", found.map(|s| s.to_string()), "search");
            }
        }
        let _ = store.wake_deferred();
        let progress = store.tracked_progress(name).unwrap_or((0, None));
        let rows = rows_for(&store, &trackers, &work, progress);
        let _ = tx.send(Event::Reading(ReadingEvent::Tracking { work: work.clone(), rows }));
    });
}

/// A series' rows on the trackers that keep a reading list.
pub(crate) fn load(services: &Services, work: WorkId, tx: &Outbox) {
    let rows =
        rows_for(&services.store, &services.sync.trackers, &work, progress_of(services, &work));
    let _ = tx.send(Event::Reading(ReadingEvent::Tracking { work, rows }));
}

/// The rows a series' Tracking section shows, for the trackers signed in to a reading list.
fn rows_for(
    store: &Store,
    trackers: &[std::sync::Arc<dyn kaeri_core::traits::Tracker>],
    work: &WorkId,
    (chapters, volume): (u32, Option<u32>),
) -> Vec<TrackerReadRow> {
    let name = work.as_str().to_owned();
    let (status, score) = store.read_status(&name).unwrap_or_default();
    let status = status
        .as_deref()
        .and_then(LibrarySegmentRead::parse)
        .filter(|s| *s != LibrarySegmentRead::YourFiles);
    let note = split_note(store, &name);
    let maps = store.track_map(&name).unwrap_or_default();
    trackers
        .iter()
        .filter(|t| t.serves_reading() && t.is_authenticated())
        .map(|tracker| {
            let id = tracker.id();
            let held = maps.iter().find(|m| m.tracker == id);
            let mapping = match held {
                Some(map) if map.tracker_id.is_some() => TrackerMapping::Mapped,
                Some(_) => match offers(store, &name, id) {
                    c if c.is_empty() => TrackerMapping::Unmapped,
                    candidates => TrackerMapping::Ask { candidates },
                },
                None => TrackerMapping::Looking,
            };
            let divergent =
                store.get_meta_i64(&conflict_key(&name, id)).ok().flatten().is_some();
            TrackerReadRow {
                tracker: id.to_owned(),
                chapters,
                // Kitsu has no volume field.
                volumes: if id == "kitsu" { None } else { volume },
                status,
                score: score.map(|s| s as f32),
                divergent,
                mapping,
                split_note: note.clone(),
            }
        })
        .collect()
}

/// Match a work to a tracker's entry by hand, or say it is not on that tracker. A hand choice is
/// never replaced by a lookup. What waited for the match is sent at the next drain.
pub(crate) fn choose(
    services: &Services,
    work: WorkId,
    tracker: &str,
    tracker_id: Option<String>,
    tx: &Outbox,
) {
    let now = kaeri_store::now();
    let id = tracker_id.as_deref().map(str::trim).filter(|id| !id.is_empty());
    let _ = services.store.clear_meta_prefix(&candidates_key(work.as_str(), tracker));
    if let Err(e) = services.store.set_track_map(work.as_str(), tracker, id, "hand", now) {
        tracing::warn!(error = %e, "the match was not kept");
    }
    if id.is_some() {
        let _ = services.store.wake_deferred();
    }
    load(services, work, tx);
}

/// Settle a status or score the tracker and this device disagree on. Keeping the local value
/// sends it; keeping the tracker's drops the flag, as the tracker already holds it.
pub(crate) fn resolve(
    services: &Services,
    work: WorkId,
    tracker: &str,
    keep_local: bool,
    tx: &Outbox,
) {
    let name = work.as_str();
    let _ = services.store.clear_meta_prefix(&conflict_key(name, tracker));
    if keep_local && let Some(id) = anilist_of(&work) {
        let now = kaeri_store::now();
        let (status, score) = services.store.read_status(name).unwrap_or_default();
        if let Some((status, rereading)) =
            status.as_deref().and_then(kaeri_track::sync::read_status_from_name)
        {
            let op = TrackOp::SetReadStatus { anilist_id: id, status, rereading, at: now };
            let _ = services.store.enqueue(tracker, &op, now);
        }
        if let Some(score) = score {
            let op = TrackOp::SetReadScore { anilist_id: id, score: score as f32, at: now };
            let _ = services.store.enqueue(tracker, &op, now);
        }
    }
    load(services, work, tx);
}

/// Sign in to MangaUpdates with the form's name and password.
pub(crate) fn connect_mangaupdates(
    services: &Services,
    username: String,
    password: String,
    tx: &Outbox,
) {
    let Services { sync, store, .. } = services;
    let (sync, tx, store) = (sync.clone(), tx.clone(), store.clone());
    tokio::spawn(async move {
        crate::tracking::connect_mangaupdates(sync.clone(), username, password, tx).await;
        crate::tracking::commands::announce_signed_in(&store, &sync);
    });
}

#[cfg(test)]
mod tests {
    use super::{Offer, candidates_key, needs_lookup, offers, progress_in};
    use kaeri_store::reading::TrackMap;

    fn map(tracker: &str, id: Option<&str>, origin: &str, at: i64) -> TrackMap {
        TrackMap {
            tracker: tracker.into(),
            tracker_id: id.map(Into::into),
            origin: origin.into(),
            checked_at: at,
        }
    }

    #[test]
    fn a_lookup_is_needed_only_for_a_missing_or_stale_empty_answer() {
        let now = 10_000_000;
        assert!(needs_lookup(&[], "kitsu", now));
        assert!(!needs_lookup(&[map("kitsu", Some("5"), "kitsu", 0)], "kitsu", now));
        assert!(!needs_lookup(&[map("kitsu", None, "hand", 0)], "kitsu", now));
        assert!(!needs_lookup(&[map("kitsu", None, "search", now - 60)], "kitsu", now));
        assert!(needs_lookup(&[map("kitsu", None, "search", 0)], "kitsu", now));
    }

    #[test]
    fn kept_offers_come_back_as_candidates() {
        let store = kaeri_store::Store::open_in_memory().expect("store");
        assert!(offers(&store, "a:7", "mangaupdates").is_empty());
        let kept = vec![Offer {
            id: "55".into(),
            title: "Made Up".into(),
            year: Some(2001),
            confidence: 74,
        }];
        let json = serde_json::to_string(&kept).unwrap();
        store.set_meta_text(&candidates_key("a:7", "mangaupdates"), &json).unwrap();
        let got = offers(&store, "a:7", "mangaupdates");
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].tracker_id.as_str(), got[0].confidence), ("55", 74));
    }

    use crate::read_catalog::chapters::{Kept, Read};

    fn chapter(id: &str, number: &str, volume: &str) -> Kept {
        Kept {
            id: id.into(),
            source_ref: id.into(),
            label: id.into(),
            number: Some(number.into()),
            volume: Some(volume.into()),
            title: None,
            version_label: None,
            language: None,
            published: None,
            pages: None,
            seen_at: None,
            part: None,
        }
    }

    fn marked(ids: &[&str]) -> Read {
        Read { marked: ids.iter().map(|s| (*s).to_owned()).collect(), ..Read::default() }
    }

    #[test]
    fn a_volume_counts_when_every_chapter_in_it_is_read() {
        let list: Vec<_> = [("1", "1"), ("2", "1"), ("3", "2"), ("4", "2")]
            .iter()
            .map(|(n, v)| chapter(n, n, v))
            .collect();
        assert_eq!(progress_in(&list, &marked(&["1", "2", "3"]), (3, None)), (3, Some(1)));
        assert_eq!(progress_in(&list, &marked(&["1", "2", "3", "4"]), (4, None)), (4, Some(2)));
        assert_eq!(progress_in(&list, &marked(&["2"]), (2, None)), (2, None));
    }

    #[test]
    fn a_volume_a_tracker_was_told_of_is_not_taken_back() {
        let list = vec![chapter("1", "1", "1"), chapter("2", "2", "1")];
        assert_eq!(progress_in(&list, &marked(&["1"]), (1, Some(3))), (1, Some(3)));
    }

    #[test]
    fn chapters_that_restart_in_each_volume_are_counted_in_reading_order() {
        let list = vec![
            chapter("v1/1", "1", "1"),
            chapter("v1/2", "2", "1"),
            chapter("v2/1", "1", "2"),
            chapter("v2/2", "2", "2"),
            chapter("v2/3", "3", "2"),
        ];
        let read = marked(&["v1/1", "v1/2", "v2/1"]);
        // Three chapters read, not the highest number, which is 2.
        assert_eq!(progress_in(&list, &read, (2, None)), (3, Some(1)));
    }

    #[test]
    fn a_half_chapter_in_a_restarting_series_is_not_another_chapter() {
        let list = vec![
            chapter("v1/1", "1", "1"),
            chapter("v1/1.5", "1.5", "1"),
            chapter("v2/1", "1", "2"),
        ];
        let read = marked(&["v1/1", "v1/1.5", "v2/1"]);
        assert_eq!(progress_in(&list, &read, (0, None)).0, 2);
    }

    mod removal {
        use std::sync::Arc;

        use kaeri_core::{
            books::WorkId,
            ids::AnilistId,
            traits::{ReadRemoval, TrackOp, Tracker, WatchStatus},
        };
        use kaeri_model::reading::{ListRemovalOutcome, ReadingFailure};
        use kaeri_store::Store;

        use super::super::{conflict_key, forget_here, remove_there};

        /// A tracker that answers a removal as told.
        struct Answers {
            id: &'static str,
            answer: fn() -> Result<ReadRemoval, kaeri_core::Error>,
        }

        #[async_trait::async_trait]
        impl Tracker for Answers {
            fn id(&self) -> &str {
                self.id
            }
            fn is_authenticated(&self) -> bool {
                true
            }
            fn serves_reading(&self) -> bool {
                true
            }
            async fn pull_library(
                &self,
            ) -> Result<Vec<kaeri_core::traits::TrackedEntry>, kaeri_core::Error> {
                Ok(Vec::new())
            }
            async fn push(&self, _: &[TrackOp]) -> Result<(), kaeri_core::Error> {
                Ok(())
            }
            async fn remove_read(
                &self,
                _: AnilistId,
            ) -> Result<ReadRemoval, kaeri_core::Error> {
                (self.answer)()
            }
        }

        fn tracker(
            id: &'static str,
            answer: fn() -> Result<ReadRemoval, kaeri_core::Error>,
        ) -> Arc<dyn Tracker> {
            Arc::new(Answers { id, answer })
        }

        #[tokio::test]
        async fn a_removal_clears_the_lists_here_and_says_what_each_tracker_did() {
            let store = Store::open_in_memory().unwrap();
            let id = AnilistId::new(900_501);
            let work = WorkId::new(format!("a:{}", id.get()));
            store.set_read_status(work.as_str(), Some("reading"), 1).unwrap();
            store.adopt_read(work.as_str(), "8", None, 1).unwrap();
            let status = TrackOp::SetReadStatus {
                anilist_id: id,
                status: WatchStatus::Current,
                rereading: false,
                at: 1,
            };
            store.enqueue("anilist", &status, 1).unwrap();
            store.set_meta_i64(&conflict_key(work.as_str(), "anilist"), 1).unwrap();
            let trackers = [
                tracker("anilist", || Ok(ReadRemoval::Removed)),
                tracker("kitsu", || Ok(ReadRemoval::NotOnList)),
                tracker("mal", || Ok(ReadRemoval::Unmatched)),
                tracker("mangaupdates", || Err(kaeri_core::Error::Auth("rejected".into()))),
                tracker("other", || Err(kaeri_core::Error::Network("timed out".into()))),
            ];

            assert_eq!(forget_here(&store, &work, &trackers, 2), 1, "the queued status goes");
            assert_eq!(store.read_status(work.as_str()).unwrap().0, None);
            assert_eq!(store.read_outbox_depth(None).unwrap(), 0);
            assert!(
                store.get_meta_i64(&conflict_key(work.as_str(), "anilist")).unwrap().is_none()
            );
            assert!(store.read_marks(work.as_str()).unwrap().is_read("8"), "reading stays");

            let results = remove_there(&trackers, id).await;
            let outcomes: Vec<_> = results.iter().map(|r| r.outcome.clone()).collect();
            assert_eq!(
                outcomes,
                [
                    ListRemovalOutcome::Removed,
                    ListRemovalOutcome::NotOnList,
                    ListRemovalOutcome::Unmatched,
                    ListRemovalOutcome::Failed {
                        failure: ReadingFailure::SignInNeeded { what: "MangaUpdates".into() }
                    },
                    ListRemovalOutcome::Failed { failure: ReadingFailure::Offline },
                ]
            );
            let words: Vec<Vec<&str>> = results
                .iter()
                .map(|r| r.line.iter().map(kaeri_model::Msg::id).collect())
                .collect();
            assert_eq!(
                words,
                [
                    vec!["reading-removed-from"],
                    vec!["reading-removed-not-there"],
                    vec!["reading-removed-unmatched"],
                    vec!["reading-removed-failed", "reading-removed-sign-in"],
                    vec!["reading-removed-failed", "reading-failed-offline"],
                ]
            );
        }

        #[test]
        fn a_folders_series_comes_off_the_lists_here_and_asks_no_tracker() {
            let store = Store::open_in_memory().unwrap();
            let work = WorkId::new("l:900502");
            store.set_read_status(work.as_str(), Some("paused"), 1).unwrap();
            assert_eq!(forget_here(&store, &work, &[], 2), 0);
            assert_eq!(store.read_status(work.as_str()).unwrap().0, None);
        }
    }
}
