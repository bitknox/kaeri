//! Browsing extras: Trending over a period, Surprise me, and what they ask AniList.

use kaeri_core::ids::AnilistId;
use kaeri_engine::clock::Clock;
use kaeri_model::{
    browsing::{SurpriseOutcome, SurprisePool, TrendPeriod},
    command::Command,
    event::Event,
    search::SearchQuery,
};
use kaeri_testkit::{
    Operation,
    fixtures::{ALL, FINISHED, FIXTURE_NOW, MOVIE, SEQUEL},
};

use super::{content, ids};
use crate::harness::{Harness, Options, names};

/// The fake's week: the fixtures from halfway along, so it leads otherwise than today.
fn week() -> Vec<AnilistId> {
    let mut titles = ALL.to_vec();
    titles.rotate_left(ALL.len() / 2);
    titles
}

/// The app, its engine's clock and the fake's at `now`. The month's window is taken from the
/// engine's clock and the fake dates each day's figure by its own, so the two must agree: with
/// the engine on the system's clock, the fake's days fell out of the window a month after
/// [`FIXTURE_NOW`].
async fn at(now: i64) -> Harness {
    let clock = Clock::default();
    clock.advance(now - kaeri_store::now());
    Harness::with(Options { now, clock: Some(clock), ..Options::default() }).await
}

/// A moment in UTC.
fn utc(year: i16, month: i8, day: i8, hour: i8, minute: i8, second: i8) -> i64 {
    jiff::civil::date(year, month, day)
        .at(hour, minute, second, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .expect("a moment in UTC")
        .timestamp()
        .as_second()
}

/// The month's rail at `now`, after checking it took its two requests.
async fn month_at(now: i64) -> Vec<AnilistId> {
    let mut h = at(now).await;
    h.send(Command::LoadTrending(TrendPeriod::Month));
    let updates = h.settle().await;
    let (_, shown, _, failed) = trending(&updates);
    assert!(!failed, "the month failed at {now}");
    assert_eq!(h.server().requests_for(Operation::TrendMonthCandidates).len(), 1);
    assert_eq!(h.server().requests_for(Operation::TrendSeries).len(), 1);
    shown
}

fn trending(updates: &[Event]) -> (TrendPeriod, Vec<AnilistId>, Option<i64>, bool) {
    updates
        .iter()
        .find_map(|update| match update {
            Event::Trending { period, entries, updated_at, failed, .. } => {
                Some((*period, entries.iter().map(|e| e.id).collect(), *updated_at, *failed))
            }
            _ => None,
        })
        .expect("a trending rail was sent")
}

#[tokio::test]
async fn trending_over_a_week_is_summed_in_one_request_and_kept() {
    let mut h = Harness::new().await;
    h.send(Command::LoadTrending(TrendPeriod::Week));
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Trending"]);
    let (period, shown, updated_at, failed) = trending(&updates);
    assert_eq!((period, updated_at, failed), (TrendPeriod::Week, None, false));
    assert_eq!(shown, week(), "most trending first");
    assert_eq!(h.server().requests_for(Operation::TrendWeek).len(), 1);

    // Again: from what is kept, with its age, and nothing asked.
    h.send(Command::LoadTrending(TrendPeriod::Week));
    let again = h.settle().await;
    let (_, shown, updated_at, _) = trending(&again);
    assert_eq!(shown, week());
    assert!(updated_at.is_some(), "a kept answer says when it was made");
    assert_eq!(h.server().requests_for(Operation::TrendWeek).len(), 1);
}

#[tokio::test]
async fn trending_over_a_month_takes_two_requests_and_ranks_by_the_sum() {
    let mut reversed = week();
    reversed.reverse();
    let shown = month_at(FIXTURE_NOW).await;
    assert_eq!(shown, reversed, "the month's figures rank otherwise than the week's");
}

#[tokio::test]
async fn trending_over_a_month_ranks_the_same_whatever_the_date() {
    let mut reversed = week();
    reversed.reverse();
    let moments = [
        // Thirty days on from the fixtures, the moment the month's window first left their days.
        FIXTURE_NOW + 30 * 86_400,
        utc(2026, 11, 13, 12, 30, 0),
        // The ends of a short month, a leap day, a year and a clock change in Europe.
        utc(2027, 2, 28, 23, 59, 59),
        utc(2028, 2, 29, 0, 0, 0),
        utc(2026, 12, 31, 23, 59, 59),
        utc(2027, 3, 28, 1, 30, 0),
    ];
    for now in moments {
        assert_eq!(month_at(now).await, reversed, "the month ranks otherwise at {now}");
    }
}

#[tokio::test]
async fn a_trending_search_over_a_week_pages_the_periods_titles() {
    let mut h = Harness::new().await;
    h.send(Command::Search { query: SearchQuery::trending(TrendPeriod::Week) });
    let updates = h.settle().await;
    assert_eq!(ids(content(&updates)), week());
    assert_eq!(h.server().requests_for(Operation::TrendWeek).len(), 1);
    assert!(h.server().requests_for(Operation::Search).is_empty(), "no filter, no search");

    // With a filter the period's titles are held to it, in the period's order.
    let drama = SearchQuery {
        genres: vec!["Drama".into()],
        ..SearchQuery::trending(TrendPeriod::Week)
    };
    h.send(Command::Search { query: drama });
    let filtered = h.settle().await;
    assert_eq!(ids(content(&filtered)), [MOVIE, FINISHED, SEQUEL]);
}

#[tokio::test]
async fn surprise_me_on_the_filters_counts_then_picks_and_does_not_repeat() {
    let mut h = Harness::new().await;
    let query = SearchQuery { genres: vec!["Drama".into()], ..SearchQuery::default() };
    let pool = SurprisePool::Filters { query };
    let mut picked = Vec::new();
    for _ in 0..3 {
        h.send(Command::Surprise(pool.clone()));
        let updates = h.settle().await;
        let outcome = updates
            .iter()
            .find_map(|u| match u {
                Event::Surprise { outcome, .. } => Some(outcome.clone()),
                _ => None,
            })
            .expect("an outcome");
        let SurpriseOutcome::Picked { id, more } = outcome else { panic!("{outcome:?}") };
        assert!(more);
        assert!([FINISHED, SEQUEL, MOVIE].contains(&id));
        picked.push(id);
    }
    picked.sort();
    picked.dedup();
    assert_eq!(picked.len(), 3, "three picks, three titles");
    assert_eq!(h.server().requests_for(Operation::Total).len(), 1, "the count is kept");
}

#[tokio::test]
async fn surprise_me_on_filters_that_match_nothing_says_so() {
    let mut h = Harness::new().await;
    let query =
        SearchQuery { genres: vec!["Nothing Like It".into()], ..SearchQuery::default() };
    h.send(Command::Surprise(SurprisePool::Filters { query }));
    let updates = h.settle().await;
    assert!(
        updates
            .iter()
            .any(|u| matches!(u, Event::Surprise { outcome: SurpriseOutcome::Empty, .. }))
    );
    assert!(updates.iter().any(|u| matches!(u, Event::Toast(_))));
}
