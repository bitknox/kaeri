//! The home-screen widgets' record: Continue's first titles and the week's broadcasts, for a
//! native app to write where its widgets read it. See `Command::WidgetSnapshot`.
//!
//! Made from what Continue and the calendar already compute: Continue's titles from history,
//! each one's progress and resume point as Continue counts them, and its title and art from the
//! metadata cache where the cache has it; and the broadcasts from the start of the device's day to
//! a week on, as the calendar fetches a window of them. A background task can ask for it, so it
//! asks AniList as little as it can.

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use kaeri_core::ids::AnilistId;
use kaeri_meta::{Media, Meta, anilist::AiringEntry};
use kaeri_model::{
    event::Event,
    widgets::{
        WIDGET_DATA_VERSION, WidgetAiring, WidgetContinue, WidgetData, WidgetNextAiring,
    },
};
use kaeri_store::Store;

use crate::{
    catalog::calendar::{CalendarMemory, Need},
    clock::Clock,
    data, new_episodes,
    outbox::Outbox,
    services::Services,
    state::EngineState,
};

/// How many titles to continue the widgets are given.
const CONTINUE_ROWS: u32 = 5;

/// The longest the record waits for the week's broadcasts, inside a background task's time. A
/// slower answer goes on behind it, and the record is said again when the week is whole.
const ASK_FOR: Duration = Duration::from_secs(15);

/// How many times a record that could not read the whole week asks again, and how long it
/// waits before each: AniList's own word while it is rate limiting, and at least this.
pub(crate) const FILL_IN_TRIES: u32 = 4;
pub(crate) const FILL_IN_AFTER: Duration = Duration::from_secs(30);

/// Make the record, off the actor, and say it as [`Event::WidgetSnapshot`]: once, with what
/// could not be fetched left out.
pub(crate) fn snapshot(engine: &EngineState, services: &Services, tx: &Outbox) {
    let tracker = services
        .sync
        .trackers
        .first()
        .filter(|tracker| tracker.is_authenticated())
        .map(|tracker| tracker.id().to_owned());
    let make = Make {
        store: services.store.clone(),
        meta: services.meta.clone(),
        tracker,
        clock: services.clock.clone(),
        schedule: engine.calendar.clone(),
    };
    let tx = tx.clone();
    tokio::spawn(async move {
        // Said at once with what is kept, and again as the week fills in: a week cut short by a
        // rate limit or a slow answer still shows what it had, and is made whole behind it.
        let mut tries = 0;
        loop {
            let (data, whole) = make.run().await;
            let _ = tx.send(Event::WidgetSnapshot(data));
            tries += 1;
            if whole || tries > FILL_IN_TRIES {
                break;
            }
            let wait = make.meta.rate_limit_wait().await.unwrap_or_default();
            tokio::time::sleep(wait.max(FILL_IN_AFTER)).await;
        }
    });
}

/// What making the record works with.
struct Make {
    store: Store,
    meta: Meta,
    /// The Library's tracker, when it is signed in.
    tracker: Option<String>,
    clock: Clock,
    schedule: CalendarMemory,
}

impl Make {
    /// The record, and whether the whole week was read for it.
    async fn run(&self) -> (WidgetData, bool) {
        let now = self.clock.now();
        let days = Days::of(now, &self.clock.zone());
        let (continue_watching, (broadcasts, whole)) =
            tokio::join!(self.continuing(now), self.broadcasts(&days));
        let followed: HashSet<AnilistId> =
            new_episodes::followed(&self.store, self.tracker.is_some(), now, true)
                .into_iter()
                .collect();
        let rows = self.airing(&broadcasts, &followed);
        let next_airing = self.next_airing(&rows, &followed, now);
        let (airing_today, next_days) =
            rows.into_iter().partition(|row| row.airs_at < days.tomorrow);
        let data = WidgetData {
            version: WIDGET_DATA_VERSION,
            generated_at: now,
            continue_watching,
            airing_today,
            signed_in: self.tracker.is_some(),
            tracker: self.tracker.clone(),
            next_airing,
            next_days,
        };
        (data, whole)
    }

    /// Continue's first titles, most recently watched first, as Continue counts where each
    /// stands. Titles and art from the cache, and only the titles it lacks asked for.
    async fn continuing(&self, now: i64) -> Vec<WidgetContinue> {
        let ids: Vec<AnilistId> = self
            .store
            .continue_list(CONTINUE_ROWS)
            .unwrap_or_default()
            .into_iter()
            .map(|progress| progress.anilist_id)
            .collect();
        let mut media: HashMap<AnilistId, Media> =
            ids.iter().filter_map(|id| Some((*id, self.cached(*id)?))).collect();
        let missing: Vec<AnilistId> =
            ids.iter().filter(|id| !media.contains_key(id)).copied().collect();
        if !missing.is_empty() {
            match tokio::time::timeout(ASK_FOR, self.meta.media_many(&missing)).await {
                Ok(Ok(found)) => media.extend(found.into_iter().map(|m| (m.id, m))),
                Ok(Err(error)) => tracing::warn!(%error, "the widgets' titles were not found"),
                Err(_) => tracing::warn!("gave up waiting for the widgets' titles"),
            }
        }
        ids.iter()
            .filter_map(|id| media.get(id))
            .map(|media| self.to_continue(media, now))
            .collect()
    }

    /// One title to continue, as Continue has it: the episode it starts, where that was left
    /// and how much of it is left, and when the next one airs.
    fn to_continue(&self, media: &Media, now: i64) -> WidgetContinue {
        let entry = data::entry_from(media, Some(&self.store));
        let episode = entry.progress.map_or(1, |progress| progress.next);
        let position = entry.resume.map(|resume| resume.position);
        let duration = self.length(media, episode);
        let left = duration.map(|duration| (duration - position.unwrap_or(0.0)).max(0.0));
        let next = media.next_airing_episode.filter(|next| next.airing_at > now);
        WidgetContinue {
            id: media.id,
            title: self.meta.titles().pick(&media.title).to_owned(),
            episode,
            watched: entry.progress.map_or(0, |progress| progress.done),
            episodes: entry.episodes,
            fraction: entry.resume.and_then(|resume| resume.fraction),
            cover_url: entry.cover_url,
            episode_title: media.episode_titles().remove(&episode),
            resume: position.is_some(),
            position_secs: position.map(|secs| secs.round() as u32),
            duration_secs: duration.map(|secs| secs.round() as u32),
            minutes_left: left.map(|secs| (secs / 60.0).ceil() as u32),
            next_airing: next.map(|next| next.airing_at),
            next_airing_episode: next.map(|next| next.episode),
            banner_url: media.banner_image.clone(),
            cover_color: media.cover_image.color.clone(),
            mark: kaeri_model::present::episode_mark(entry.watched_in).map(str::to_owned),
        }
    }

    /// How long `episode` of `media` is, in seconds: as its player last found it, or AniList's
    /// length for the title's episodes.
    fn length(&self, media: &Media, episode: u32) -> Option<f64> {
        let episode = episode.to_string();
        let played = self.store.events_for(media.id, 50).ok().and_then(|events| {
            events.into_iter().find(|e| e.episode == episode).and_then(|e| e.duration_secs)
        });
        played
            .filter(|secs| *secs > 0.0)
            .or(media.duration.map(|minutes| f64::from(minutes) * 60.0))
    }

    /// A title as the cache last had it, from any fetch that wrote it.
    fn cached(&self, id: AnilistId) -> Option<Media> {
        let (payload, _) = self.store.cached_media(id).ok().flatten()?;
        serde_json::from_str(&payload).ok()
    }

    /// The broadcasts from the start of today to a week on, aired or not, oldest first, and
    /// whether the whole week was read: the schedule the calendar, the notices and the digest
    /// share, so only the pages it does not hold are asked for. What it held before stands in
    /// for pages that cannot be had.
    async fn broadcasts(&self, days: &Days) -> (Vec<AiringEntry>, bool) {
        // `airingAt_greater` is strict, and a broadcast at midnight is the new day's.
        let need = Need {
            past_to: Some(days.today - 1),
            ahead_to: Some(days.week_end),
            ..Need::default()
        };
        let cover = self.schedule.cover(
            &self.meta,
            self.clock.now(),
            crate::now_epoch(),
            need,
            false,
            true,
        );
        match tokio::time::timeout(ASK_FOR, cover).await {
            Ok(covered) => {
                if let Some(error) = &covered.failure {
                    tracing::warn!(%error, "the week's broadcasts were not all found");
                }
                (
                    covered.window.between(days.today - 1, days.week_end),
                    covered.failure.is_none(),
                )
            }
            Err(_) => {
                tracing::warn!("gave up waiting for the week's broadcasts; using what is held");
                let held =
                    self.schedule.held().map(|w| w.between(days.today - 1, days.week_end));
                (held.unwrap_or_default(), false)
            }
        }
    }

    /// The broadcasts as the widgets show them, each marked by whether its show is followed and
    /// whether the episode has been watched here.
    fn airing(
        &self,
        broadcasts: &[AiringEntry],
        followed: &HashSet<AnilistId>,
    ) -> Vec<WidgetAiring> {
        let titles = self.meta.titles();
        let mut watched: HashMap<AnilistId, Vec<String>> = HashMap::new();
        broadcasts
            .iter()
            .map(|row| {
                let id = row.media.id;
                let done = watched
                    .entry(id)
                    .or_insert_with(|| self.store.completed_episodes(id).unwrap_or_default());
                WidgetAiring {
                    id,
                    title: titles.pick(&row.media.title).to_owned(),
                    episode: row.episode,
                    airs_at: row.airing_at,
                    on_list: followed.contains(&id),
                    cover_url: row.media.cover_image.best().map(str::to_owned),
                    watched: done.iter().any(|e| e.trim().parse::<u32>() == Ok(row.episode)),
                    translation: None,
                    banner_url: row.media.banner_image.clone(),
                    cover_color: row.media.cover_image.color.clone(),
                }
            })
            .collect()
    }

    /// The soonest broadcast still to come of a show followed: in the week's, or past them the
    /// soonest next episode the metadata cache has for one.
    fn next_airing(
        &self,
        rows: &[WidgetAiring],
        followed: &HashSet<AnilistId>,
        now: i64,
    ) -> Option<WidgetNextAiring> {
        let this_week = rows
            .iter()
            .filter(|row| row.on_list && row.airs_at > now)
            .min_by_key(|row| row.airs_at);
        if let Some(row) = this_week {
            return Some(WidgetNextAiring {
                id: row.id,
                title: row.title.clone(),
                episode: row.episode,
                airs_at: row.airs_at,
            });
        }
        followed
            .iter()
            .filter_map(|id| {
                let media = self.cached(*id)?;
                let next = media.next_airing_episode.filter(|next| next.airing_at > now)?;
                Some(WidgetNextAiring {
                    id: *id,
                    title: self.meta.titles().pick(&media.title).to_owned(),
                    episode: next.episode,
                    airs_at: next.airing_at,
                })
            })
            .min_by_key(|next| (next.airs_at, next.id.get()))
    }
}

/// The days the record is of, in `zone`, the clock's: the device's as the system has it, outside
/// a test. The start of today, of tomorrow, and of the day a week on. A day is 23 or 25 hours
/// where the clocks change, and 24 hours from `now` when the zone cannot be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Days {
    today: i64,
    tomorrow: i64,
    week_end: i64,
}

impl Days {
    fn of(now: i64, zone: &jiff::tz::TimeZone) -> Self {
        let start = |date: jiff::civil::Date| {
            date.to_zoned(zone.clone()).ok().map(|zoned| zoned.timestamp().as_second())
        };
        let bounds = jiff::Timestamp::from_second(now).ok().and_then(|at| {
            let today = at.to_zoned(zone.clone()).date();
            let mut week_end = today;
            for _ in 0..7 {
                week_end = week_end.tomorrow().ok()?;
            }
            Some(Self {
                today: start(today)?,
                tomorrow: start(today.tomorrow().ok()?)?,
                week_end: start(week_end)?,
            })
        });
        bounds.unwrap_or(Self { today: now, tomorrow: now + DAY, week_end: now + 7 * DAY })
    }
}

const DAY: i64 = 24 * 3_600;

/// The week's broadcasts as the record reads them, from the start of today to the start of the
/// day a week on, from the schedule everything shares: for the notifications' week
/// (`crate::upcoming`), so the two cost AniList one read. With the end of that week, and whether
/// the whole week was read.
pub(crate) async fn week(
    store: Store,
    meta: Meta,
    clock: Clock,
    schedule: CalendarMemory,
    now: i64,
) -> (Vec<AiringEntry>, i64, bool) {
    let days = Days::of(now, &clock.zone());
    let make = Make { store, meta, tracker: None, clock, schedule };
    let (rows, whole) = make.broadcasts(&days).await;
    (rows, days.week_end, whole)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_days_are_the_zones_from_midnight_to_midnight() {
        use jiff::tz::{TimeZone, offset};

        // 2026-10-14 12:00:30 UTC, the fixtures' moment: Wednesday midday in UTC, and Thursday
        // already fourteen hours east.
        let now = 1_791_979_230;
        let utc = Days::of(now, &TimeZone::UTC);
        assert_eq!(utc.today, now - (12 * 3_600 + 30), "{utc:?}");
        assert_eq!((utc.tomorrow - utc.today, utc.week_end - utc.today), (DAY, 7 * DAY));
        let east = Days::of(now, &TimeZone::fixed(offset(14)));
        assert_eq!(east.today, utc.tomorrow - 14 * 3_600, "{east:?}");

        // A week on, central Europe's clocks go back on the Sunday, so that week is an hour
        // longer.
        let europe = TimeZone::posix("CET-1CEST,M3.5.0,M10.5.0/3").expect("a POSIX zone");
        let days = Days::of(now + 7 * DAY, &europe);
        assert_eq!(days.tomorrow - days.today, DAY, "{days:?}");
        assert_eq!(days.week_end - days.today, 7 * DAY + 3_600, "{days:?}");
    }
}
