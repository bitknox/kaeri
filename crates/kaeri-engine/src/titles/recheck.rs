//! Looking again for an episode that aired and no source lists yet.
//!
//! The person can ask with Check again ([`MissingCommand::CheckAgain`]), which lists the title
//! afresh, sends its table as any load does, then says what came of it. And Kaeri looks by
//! itself, quietly: every [`RECHECK_EVERY_SECS`] from the first [`RECHECK_EVERY_SECS`] after an
//! episode of a show being watched airs, for [`RECHECK_FOR_SECS`]. Those checks wait for
//! Home's first screen, run one at a time in AniList's paced background queue, pause while
//! offline and run once on coming back. They stop the moment a source lists the episode. One
//! that comes back empty changes nothing on screen; one that finds it brings an open page's
//! table up to date, and a closed page is right the next time it opens.
//!
//! [`MissingCommand::CheckAgain`]: kaeri_model::missing::MissingCommand::CheckAgain

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use kaeri_core::ids::AnilistId;
use kaeri_model::{
    event::Event,
    missing::{CheckOutcome, EpisodeCheck, RECHECK_EVERY_SECS, RECHECK_FOR_SECS},
};

use super::{
    episodes::{Listing, list_episodes},
    refresh::Ticket,
};
use crate::{
    dispatch,
    outbox::{Internal, Outbox},
    services::Services,
    state::EngineState,
};

/// How often the quiet rechecks look at what is due.
pub const TICK: Duration = Duration::from_secs(60);

/// How long past the end of the quiet window a due check still runs: the ticks are a minute
/// apart, so the last one may land a little after.
const SLACK: i64 = 2 * 60;

/// What the quiet rechecks keep between ticks.
#[derive(Debug, Default)]
pub struct Rechecks {
    /// When each episode was last looked for, by title and episode.
    looked: HashMap<(AnilistId, u32), i64>,
    /// Episodes a check found: never looked for again.
    found: Arc<Mutex<HashSet<(AnilistId, u32)>>>,
    /// A batch of checks is running: the next tick waits for it.
    running: Arc<AtomicBool>,
    /// A tick came while offline, so coming back runs one round.
    missed: bool,
    /// The broadcasts of the shows being watched, and when they were read.
    schedule: Option<(i64, Vec<Aired>)>,
}

/// One broadcast: title, episode, and when it aired or airs, in Unix seconds.
type Aired = (AnilistId, u32, i64);

/// What `listing` says of episode `episode`.
pub(crate) fn outcome(listing: &Listing, episode: u32) -> CheckOutcome {
    match listing {
        Listing::Rows(rows) => {
            let listed = rows.iter().any(|row| {
                row.number.trim().parse::<u32>().ok() == Some(episode) && !row.unsourced
            });
            if listed { CheckOutcome::Found } else { CheckOutcome::Missing }
        }
        Listing::Unanswered => CheckOutcome::NoAnswer,
        Listing::Unsure => CheckOutcome::Missing,
    }
}

/// Check again, as the person asked: list title `id` afresh, which sends its table, then say
/// what came of episode `episode`. Offline, nothing is asked and the answer comes at once.
pub fn check_again(
    id: AnilistId,
    episode: u32,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let said = move |outcome| {
        Event::EpisodeChecked(EpisodeCheck { id, episode, outcome, asked: true })
    };
    if engine.network.offline {
        let _ = tx.send(said(CheckOutcome::Offline));
        return;
    }
    engine.listed.note(id);
    let ticket = engine.listed.ticket(id);
    let table = engine.dubs.table(id);
    let naming = engine.names.naming(id, false, &services.clock);
    let found = Arc::clone(&engine.rechecks.found);
    dispatch::spawn_abortable(None, services, tx, move |lent| async move {
        let tx = lent.tx.clone();
        let outcome = outcome(&list_episodes(lent, id, naming, ticket, false).await, episode);
        drop(table);
        if outcome == CheckOutcome::Found {
            found.lock().unwrap_or_else(|e| e.into_inner()).insert((id, episode));
        }
        let _ = tx.send(said(outcome));
    });
}

/// Start the quiet rechecks' clock: once Home's first screen is in, a tick every [`TICK`].
pub fn spawn_ticks(anilist: kaeri_meta::AniList, tx: Outbox) {
    tokio::spawn(async move {
        anilist.launch_over().await;
        let mut every = tokio::time::interval(TICK);
        every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            every.tick().await;
            if tx.internal(Internal::Recheck).is_err() {
                return;
            }
        }
    });
}

/// The device is online again: a tick missed while offline runs its round now.
pub fn reconnected(engine: &mut EngineState, services: &Services, tx: &Outbox) {
    if std::mem::take(&mut engine.rechecks.missed) {
        round(true, engine, services, tx);
    }
}

/// A tick of the quiet rechecks: run what is due.
pub fn tick(engine: &mut EngineState, services: &Services, tx: &Outbox) {
    round(false, engine, services, tx);
}

/// A quiet recheck found episode `episode` of title `id`. An open page lists it again, so its
/// table has the episode, then hears what was found.
pub fn found(
    id: AnilistId,
    episode: u32,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    if !engine.listed.has(id) {
        return;
    }
    let ticket = engine.listed.ticket(id);
    let table = engine.dubs.table(id);
    let naming = engine.names.naming(id, engine.network.offline, &services.clock);
    dispatch::spawn_abortable(None, services, tx, move |lent| async move {
        let tx = lent.tx.clone();
        let listing = kaeri_meta::in_background(list_episodes(lent, id, naming, ticket, false));
        let outcome = outcome(&listing.await, episode);
        drop(table);
        if outcome == CheckOutcome::Found {
            let check = EpisodeCheck { id, episode, outcome, asked: false };
            let _ = tx.send(Event::EpisodeChecked(check));
        }
    });
}

fn round(back: bool, engine: &mut EngineState, services: &Services, tx: &Outbox) {
    if engine.network.offline {
        engine.rechecks.missed = true;
        return;
    }
    // Someone who only reads follows no shows.
    if !engine.reading.answer().watches() {
        return;
    }
    let rechecks = &mut engine.rechecks;
    if rechecks.running.load(Ordering::SeqCst) {
        return;
    }
    let now = services.clock.now();
    if rechecks.schedule.as_ref().is_none_or(|(read, _)| now - read >= RECHECK_EVERY_SECS) {
        rechecks.schedule = Some((now, schedule(services, now)));
    }
    let aired =
        rechecks.schedule.as_ref().map(|(_, aired)| aired.as_slice()).unwrap_or_default();
    let found = rechecks.found.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let due = due(aired, &rechecks.looked, &found, now, back);
    if due.is_empty() {
        return;
    }
    for key in &due {
        rechecks.looked.insert(*key, now);
    }
    // What was looked for long ago is forgotten, so the map stays small.
    rechecks.looked.retain(|_, at| now - *at <= RECHECK_FOR_SECS + SLACK);
    rechecks.running.store(true, Ordering::SeqCst);
    let running = Arc::clone(&rechecks.running);
    let found = Arc::clone(&rechecks.found);
    let naming = engine.names.unlooked(&services.clock);
    dispatch::spawn_abortable(None, services, tx, move |lent| async move {
        let dispatch::Lent { meta, store, registry, stores, tx } = lent;
        for (id, episode) in due {
            let lent = dispatch::Lent {
                meta: meta.clone(),
                store: store.clone(),
                registry: registry.clone(),
                stores: stores.clone(),
                tx: tx.clone(),
            };
            let listing = list_episodes(lent, id, naming.clone(), Ticket::muted(id), false);
            let outcome = outcome(&kaeri_meta::in_background(listing).await, episode);
            tracing::debug!(id = id.get(), episode, ?outcome, "quiet recheck");
            if outcome == CheckOutcome::Found {
                found.lock().unwrap_or_else(|e| e.into_inner()).insert((id, episode));
                let _ = tx.internal(Internal::RecheckFound { id, episode });
            }
        }
        running.store(false, Ordering::SeqCst);
    });
}

/// The broadcasts of the shows being watched that could be due in the next while: aired within
/// the quiet window, or airing before the schedule is read again. From the stored metadata
/// alone, so AniList is not asked.
fn schedule(services: &Services, now: i64) -> Vec<Aired> {
    let store = &services.store;
    let signed_in = crate::new_episodes::list_signed_in(services);
    let mut aired = Vec::new();
    for id in crate::new_episodes::followed(store, signed_in, now, false) {
        let Some(media) =
            store.cached_media(id).ok().flatten().and_then(|(payload, _)| {
                serde_json::from_str::<kaeri_meta::Media>(&payload).ok()
            })
        else {
            continue;
        };
        for (episode, at) in media.air_times() {
            if at >= now - RECHECK_FOR_SECS - SLACK && at <= now + RECHECK_EVERY_SECS {
                aired.push((id, episode, at));
            }
        }
    }
    aired
}

/// Which of `aired` to look for at `now`: those from [`RECHECK_EVERY_SECS`] to
/// [`RECHECK_FOR_SECS`] after they aired, not found, and not looked for in the last
/// [`RECHECK_EVERY_SECS`]. Coming `back` online, every one in the window once.
fn due(
    aired: &[Aired],
    looked: &HashMap<(AnilistId, u32), i64>,
    found: &HashSet<(AnilistId, u32)>,
    now: i64,
    back: bool,
) -> Vec<(AnilistId, u32)> {
    aired
        .iter()
        .filter(|(_, _, at)| {
            now >= at + RECHECK_EVERY_SECS && now <= at + RECHECK_FOR_SECS + SLACK
        })
        .map(|(id, episode, _)| (*id, *episode))
        .filter(|key| !found.contains(key))
        .filter(|key| match looked.get(key) {
            None => true,
            Some(_) if back => true,
            Some(at) => now - at >= RECHECK_EVERY_SECS,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_model::episodes::EpisodeRow;

    const NOW: i64 = 1_800_000_000;
    const MIN: i64 = 60;

    fn id() -> AnilistId {
        AnilistId::new(7)
    }

    #[test]
    fn the_first_look_is_fifteen_minutes_after_airing() {
        let aired = [(id(), 6, NOW - 14 * MIN)];
        assert!(due(&aired, &HashMap::new(), &HashSet::new(), NOW, false).is_empty());
        let aired = [(id(), 6, NOW - 15 * MIN)];
        assert_eq!(due(&aired, &HashMap::new(), &HashSet::new(), NOW, false), [(id(), 6)]);
    }

    #[test]
    fn then_every_fifteen_minutes_for_six_hours_and_no_longer() {
        let aired = [(id(), 6, NOW - 2 * 3_600)];
        let mut looked = HashMap::from([((id(), 6), NOW - 14 * MIN)]);
        assert!(due(&aired, &looked, &HashSet::new(), NOW, false).is_empty());
        looked.insert((id(), 6), NOW - 15 * MIN);
        assert_eq!(due(&aired, &looked, &HashSet::new(), NOW, false).len(), 1);
        let late = [(id(), 6, NOW - RECHECK_FOR_SECS - SLACK - 1)];
        assert!(due(&late, &HashMap::new(), &HashSet::new(), NOW, false).is_empty());
    }

    #[test]
    fn a_found_episode_is_never_looked_for_again() {
        let aired = [(id(), 6, NOW - 30 * MIN)];
        let found = HashSet::from([(id(), 6)]);
        assert!(due(&aired, &HashMap::new(), &found, NOW, false).is_empty());
        assert!(due(&aired, &HashMap::new(), &found, NOW, true).is_empty());
    }

    #[test]
    fn coming_back_online_looks_once_whatever_the_last_look() {
        let aired = [(id(), 6, NOW - 40 * MIN)];
        let looked = HashMap::from([((id(), 6), NOW - 5 * MIN)]);
        assert!(due(&aired, &looked, &HashSet::new(), NOW, false).is_empty());
        assert_eq!(due(&aired, &looked, &HashSet::new(), NOW, true), [(id(), 6)]);
    }

    fn row(number: &str, unsourced: bool) -> EpisodeRow {
        EpisodeRow {
            number: number.into(),
            title: None,
            thumbnail: None,
            description: None,
            duration_secs: None,
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            airs_at: None,
            delayed_from: None,
            metadata_only: false,
            source: None,
            unsourced,
            past_progress: false,
            spoiler_ahead: false,
            synopsis_pending: false,
        }
    }

    #[test]
    fn a_listing_says_found_missing_or_no_answer() {
        let listed = Listing::Rows(vec![row("5", false), row("6", false)]);
        assert_eq!(outcome(&listed, 6), CheckOutcome::Found);
        let short = Listing::Rows(vec![row("5", false), row("6", true)]);
        assert_eq!(outcome(&short, 6), CheckOutcome::Missing);
        assert_eq!(outcome(&Listing::Unanswered, 6), CheckOutcome::NoAnswer);
        assert_eq!(outcome(&Listing::Unsure, 6), CheckOutcome::Missing);
    }
}
