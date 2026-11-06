//! Reading on the trackers: what a series' Tracking section shows, the choice of which entry a
//! work is on a tracker, a conflict settled by hand, and the status and score changes that go to
//! the trackers' manga and novel lists.
//!
//! Local state stays the truth. A change is queued in the reading outbox of each signed-in
//! tracker that keeps such a list, and the drain sends it; see `kaeri_track::sync::drain_read`.
//! A chapter like 10.5 goes to a tracker as 10, and the series says so.

use kaeri_core::{
    books::WorkId,
    ids::AnilistId,
    traits::{TrackOp, WatchStatus},
};
use kaeri_model::{
    Msg, ReadingMsg,
    event::Event,
    reading::{LibrarySegmentRead, ReadingEvent, TrackerMapping, TrackerReadRow},
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

/// A series' rows on the trackers that keep a reading list.
pub(crate) fn load(services: &Services, work: WorkId, tx: &Outbox) {
    let store = &services.store;
    let name = work.as_str().to_owned();
    let (chapters, volume) = store.read_progress(&name).unwrap_or((0, None));
    let (status, score) = store.read_status(&name).unwrap_or_default();
    let status = status
        .as_deref()
        .and_then(LibrarySegmentRead::parse)
        .filter(|s| *s != LibrarySegmentRead::YourFiles);
    let note = split_note(store, &name);
    let signed_in = signed_in_reading_tracker_ids(&services.sync);
    let maps = store.track_map(&name).unwrap_or_default();
    let rows = services
        .sync
        .trackers
        .iter()
        .filter(|t| t.serves_reading() && signed_in.iter().any(|id| id == t.id()))
        .map(|tracker| {
            let id = tracker.id();
            let held = maps.iter().find(|m| m.tracker == id);
            let mapping = match held {
                Some(map) if map.tracker_id.is_some() => TrackerMapping::Mapped,
                Some(_) => TrackerMapping::Unmapped,
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
        .collect();
    let _ = tx.send(Event::Reading(ReadingEvent::Tracking { work, rows }));
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
