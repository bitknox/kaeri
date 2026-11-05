//! What browsing adds to cards and lists: the count line under a title, the dub a source said a
//! title has, Trending's periods, Surprise me, and Go to episode.
//!
//! Every rule here is the engine's, so each app says and does the same: the long and short count
//! forms and what a screen reader says, which picks are
//! offered, and what a number typed into an episode list means.

use kaeri_core::ids::AnilistId;
use kaeri_i18n::t;

use crate::{
    catalog::{AiringStatus, Entry, FuzzyDate, LibrarySegment},
    episodes::{DubExtent, EpisodeRow, RANGE_SIZE},
    present,
    search::SearchQuery,
};

/// A dub a source said a title has: how far it goes, when it counted. What a card's dub part
/// ("Dub 8", or "Dub" with no count) is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct DubKnown {
    /// The last whole episode the dub has, in the numbering of the title's episode table. `None`
    /// for a source that listed a dub and did not say how far it goes.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub through: Option<u32>,
    /// The dub has every episode aired.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub complete: bool,
}

impl From<DubExtent> for DubKnown {
    fn from(extent: DubExtent) -> Self {
        Self { through: Some(extent.through), complete: extent.complete }
    }
}

/// The count line of a card or a list row, worded by the engine from what is known of the title:
/// its episodes, how many are out, where the viewer is, and the dub a source said it has.
///
/// `long` is for a cell with room; `short` for one under about 120 pt, and for any cell where the
/// long form does not fit, so the dub part is never cut off. `spoken` is what a screen reader
/// reads after the title. `dub` is the dub part alone ("Dub 8"), for an app that fades it in
/// where it arrives later than the count; `long` and `short` already end with it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CountLine {
    pub long: String,
    pub short: String,
    pub spoken: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub dub: Option<String>,
}

const JOIN: &str = " \u{b7} ";
const SPOKEN_JOIN: &str = ", ";

fn month_short(month: u8) -> Option<String> {
    Some(match month {
        1 => t::date_month_jan_short(),
        2 => t::date_month_feb_short(),
        3 => t::date_month_mar_short(),
        4 => t::date_month_apr_short(),
        5 => t::date_month_may_short(),
        6 => t::date_month_jun_short(),
        7 => t::date_month_jul_short(),
        8 => t::date_month_aug_short(),
        9 => t::date_month_sep_short(),
        10 => t::date_month_oct_short(),
        11 => t::date_month_nov_short(),
        12 => t::date_month_dec_short(),
        _ => return None,
    })
}

fn month_long(month: u8) -> Option<String> {
    Some(match month {
        1 => t::date_month_jan_long(),
        2 => t::date_month_feb_long(),
        3 => t::date_month_mar_long(),
        4 => t::date_month_apr_long(),
        5 => t::date_month_may_long(),
        6 => t::date_month_jun_long(),
        7 => t::date_month_jul_long(),
        8 => t::date_month_aug_long(),
        9 => t::date_month_sep_long(),
        10 => t::date_month_oct_long(),
        11 => t::date_month_nov_long(),
        12 => t::date_month_dec_long(),
        _ => return None,
    })
}

/// The long, short and spoken forms of the part before the dub.
struct Base {
    long: String,
    short: String,
    spoken: String,
}

impl Base {
    fn same(text: String, spoken: String) -> Self {
        Self { long: text.clone(), short: text, spoken }
    }
}

fn base(entry: &Entry) -> Option<Base> {
    let total = entry.episodes.filter(|n| *n > 0);
    let movie = entry.format.as_deref() == Some("MOVIE");
    if movie && let Some(length) = present::film_length(entry) {
        return Some(Base::same(length.clone(), length));
    }
    if entry.status == Some(AiringStatus::NotYetReleased) {
        return Some(not_out(entry.start_date));
    }
    // Where the viewer is, on the list segments that mean a show in progress.
    if matches!(
        entry.list_status,
        Some(LibrarySegment::Watching | LibrarySegment::Paused | LibrarySegment::Dropped)
    ) && let (Some(progress), Some(total)) = (entry.progress, total)
        && progress.next >= 1
    {
        let (episode, total) = (i64::from(progress.next), i64::from(total));
        return Some(Base::same(
            t::count_next_of(episode, total),
            t::count_spoken_next_of(episode, total),
        ));
    }
    // Still airing: how many are out, of the total when AniList has one.
    if let Some(next) = entry.next_episode {
        let out = next.saturating_sub(1);
        if out == 0 {
            return Some(not_out(entry.start_date));
        }
        return Some(match total.filter(|total| *total >= out) {
            Some(total) => Base {
                long: t::count_out_of(out.into(), total.into()),
                short: t::count_out_of_short(out.into(), total.into()),
                spoken: t::count_spoken_out_of(out.into(), total.into()),
            },
            None => Base {
                long: t::count_out(out.into()),
                short: t::count_out_short(out.into()),
                spoken: t::count_spoken_out(out.into()),
            },
        });
    }
    let total = total?;
    Some(Base {
        long: t::count_episodes(total.into()),
        short: t::count_episodes_short(total.into()),
        spoken: t::count_spoken_episodes(total.into()),
    })
}

fn not_out(start: Option<FuzzyDate>) -> Base {
    if let Some(FuzzyDate { month: Some(month), day: Some(day), .. }) = start
        && let (Some(short), Some(long)) = (month_short(month), month_long(month))
    {
        return Base {
            long: t::count_starts(&format!("{day} {short}")),
            short: t::count_starts(&format!("{day} {short}")),
            spoken: t::count_spoken_starts(&format!("{day} {long}")),
        };
    }
    Base::same(t::count_not_out(), t::count_not_out())
}

/// The count line of `entry`, from what it carries: `None` when nothing is known to say, which
/// is a title AniList gives no episode count and no source has said a dub of.
///
/// A movie gives its length, and a dub is a yes or nothing. A card never says "No dub": a source
/// that did not say is not a no.
pub fn count_line(entry: &Entry) -> Option<CountLine> {
    let movie = entry.format.as_deref() == Some("MOVIE");
    let dub = entry.dub.map(|dub| match dub.through {
        Some(through) if through > 0 && !movie => {
            (t::count_dub_through(through.into()), t::count_spoken_dub_through(through.into()))
        }
        _ => (t::count_dub(), t::count_spoken_dub()),
    });
    let base = base(entry);
    match (base, dub) {
        (None, None) => None,
        (Some(base), None) => Some(CountLine {
            long: base.long,
            short: base.short,
            spoken: base.spoken,
            dub: None,
        }),
        (None, Some((dub, spoken))) => {
            Some(CountLine { long: dub.clone(), short: dub.clone(), spoken, dub: Some(dub) })
        }
        (Some(base), Some((dub, spoken))) => Some(CountLine {
            long: format!("{}{JOIN}{dub}", base.long),
            short: format!("{}{JOIN}{dub}", base.short),
            spoken: format!("{}{SPOKEN_JOIN}{spoken}", base.spoken),
            dub: Some(dub),
        }),
    }
}

/// The period Trending covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TrendPeriod {
    /// AniList's live trending: the default, and what Trending now has always been.
    Today,
    /// The last seven days, summed from AniList's daily figures.
    Week,
    /// The last thirty days.
    Month,
}

impl TrendPeriod {
    pub const ALL: [Self; 3] = [Self::Today, Self::Week, Self::Month];

    /// "Trending today", "Trending this week": the heading and the sort's name.
    pub fn label(self) -> String {
        match self {
            Self::Today => t::trending_today(),
            Self::Week => t::trending_week(),
            Self::Month => t::trending_month(),
        }
    }

    /// "Today", "This week", "This month": a choice of the period menu.
    pub fn choice(self) -> String {
        match self {
            Self::Today => t::trending_choice_today(),
            Self::Week => t::trending_choice_week(),
            Self::Month => t::trending_choice_month(),
        }
    }

    /// The days it looks back over.
    pub const fn days(self) -> u32 {
        match self {
            Self::Today => 1,
            Self::Week => 7,
            Self::Month => 30,
        }
    }

    /// How long a kept answer for it serves, in seconds: today an hour, a week six, a month a
    /// day.
    pub const fn kept_for(self) -> i64 {
        match self {
            Self::Today => 3_600,
            Self::Week => 6 * 3_600,
            Self::Month => 24 * 3_600,
        }
    }

    /// The period after this one, for a key that cycles: today, week, month, today.
    pub const fn next(self) -> Self {
        match self {
            Self::Today => Self::Week,
            Self::Week => Self::Month,
            Self::Month => Self::Today,
        }
    }
}

/// Where Surprise me picks from: the screen it is on.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
// One query rides in one variant, as it does in `Command::Search`: not worth a box across FFI.
#[allow(clippy::large_enum_variant)]
pub enum SurprisePool {
    /// Search's browse: every title the filters in effect match, which AniList is asked for.
    Filters { query: SearchQuery },
    /// The Planning list on this device. Asks nothing, and works offline.
    Planning,
}

/// What a Surprise me or Another asked for came to.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SurpriseOutcome {
    /// Open this title's page. `more` is whether the pool has another title to pick, which
    /// Another needs: with a pool of one the page has no Another.
    Picked { id: AnilistId, more: bool },
    /// Nothing to pick from. A notice says which, and the button stops spinning.
    Empty,
    /// Search picks need AniList and the device is offline. A notice says so.
    Offline,
    /// AniList could not be asked. A notice says so.
    Failed,
}

/// What a number typed into an episode list means: see [`go_to_episode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum GoTo {
    /// Nothing typed yet: Go waits for a number.
    Empty,
    /// The episode is row `index` of the table, in `range` (an index into
    /// [`episode_ranges`](crate::episodes::episode_ranges)) when the show has ranges. Switch to
    /// that range if it is not the one open, and put the row at the top with the selection fill.
    Row { index: u32, range: Option<u32> },
    /// The table has no such episode; `last` is how many episodes it has.
    NoSuch { number: u32, last: u32 },
    /// The table lists it with no date, beyond the latest episode out.
    NotOut { number: u32, latest: u32 },
}

/// The number a row stands for, when it is one.
fn row_number(row: &EpisodeRow) -> Option<f64> {
    row.number.trim().parse::<f64>().ok()
}

/// What typing `typed` into Go to episode (or digits into the list) means for the table `rows`
/// as of `now`.
///
/// An episode listed with a date is a row to go to, even before it airs. One listed with no date
/// that comes after the latest episode out is "not out yet": the latest dated episode that is
/// out says how far it goes. A table with no dates at all has every episode out.
pub fn go_to_episode(rows: &[EpisodeRow], typed: &str, now: i64) -> GoTo {
    let typed = typed.trim();
    if typed.is_empty() {
        return GoTo::Empty;
    }
    let Ok(number) = typed.parse::<u32>() else {
        // Not a whole number: a row can still be "12.5", which is typed in full.
        return match typed.parse::<f64>() {
            Ok(wanted) if wanted.is_finite() && wanted > 0.0 => {
                match rows.iter().position(|row| row_number(row) == Some(wanted)) {
                    Some(index) => row_at(rows, index),
                    None => GoTo::NoSuch { number: wanted.floor() as u32, last: last_of(rows) },
                }
            }
            _ => GoTo::Empty,
        };
    };
    let Some(index) = rows.iter().position(|row| row_number(row) == Some(f64::from(number)))
    else {
        return GoTo::NoSuch { number, last: last_of(rows) };
    };
    let row = &rows[index];
    let dated_ahead = row.airs_at.is_some_and(|at| at > now);
    if !dated_ahead && row.airs_at.is_none() {
        // No date: beyond the latest dated episode that is out, it has not come out.
        let latest = rows
            .iter()
            .filter(|r| r.airs_at.is_some_and(|at| at <= now))
            .filter_map(row_number)
            .fold(None::<f64>, |best, n| Some(best.map_or(n, |b| b.max(n))));
        if let Some(latest) = latest
            && f64::from(number) > latest
        {
            return GoTo::NotOut { number, latest: latest.floor() as u32 };
        }
    }
    row_at(rows, index)
}

fn row_at(rows: &[EpisodeRow], index: usize) -> GoTo {
    let range = (rows.len() > RANGE_SIZE).then_some((index / RANGE_SIZE) as u32);
    GoTo::Row { index: index as u32, range }
}

/// How many episodes the table has, in the numbers it lists: its highest whole number, else
/// its rows.
fn last_of(rows: &[EpisodeRow]) -> u32 {
    rows.iter()
        .filter_map(row_number)
        .fold(None::<f64>, |best, n| Some(best.map_or(n, |b| b.max(n))))
        .map_or(rows.len() as u32, |n| n.floor() as u32)
}

/// What Go to episode says of `result`, in the catalogue's words; `None` for a row to go to and
/// for nothing typed.
pub fn go_to_message(result: &GoTo) -> Option<String> {
    match *result {
        GoTo::NoSuch { number, last } => {
            Some(t::episode_go_to_none(number.into(), last.into()))
        }
        GoTo::NotOut { number, latest } => {
            Some(t::episode_go_to_not_out(number.into(), latest.into()))
        }
        GoTo::Row { .. } | GoTo::Empty => None,
    }
}

/// The field's placeholder: "1 to 412".
pub fn go_to_hint(rows: &[EpisodeRow]) -> String {
    let first = rows
        .iter()
        .filter_map(row_number)
        .fold(None::<f64>, |best, n| Some(best.map_or(n, |b| b.min(n))));
    t::episode_go_to_hint(first.map_or(1, |n| n.floor() as i64), last_of(rows).into())
}

/// The field's accessibility label: "Episode, 1 to 412".
pub fn go_to_label(rows: &[EpisodeRow]) -> String {
    let first = rows
        .iter()
        .filter_map(row_number)
        .fold(None::<f64>, |best, n| Some(best.map_or(n, |b| b.min(n))));
    t::episode_go_to_label(first.map_or(1, |n| n.floor() as i64), last_of(rows).into())
}

/// The range of a long series' table that holds row `index`; `None` for a table of one range.
pub fn episode_range_of_row(rows: &[EpisodeRow], index: u32) -> Option<u32> {
    (rows.len() > RANGE_SIZE && (index as usize) < rows.len())
        .then(|| index / RANGE_SIZE as u32)
}

/// The range that holds the episode named `number` (a row's own number, "402"): where the
/// player's Episodes panel opens, on the playing episode's range. `None` for a table of one
/// range, or an episode it does not list.
pub fn episode_range_of(rows: &[EpisodeRow], number: &str) -> Option<u32> {
    let index = rows.iter().position(|row| row.number == number)?;
    episode_range_of_row(rows, index as u32)
}

/// The range after stepping `by` ranges from `current` of `count`, held to the first and the
/// last: Previous and Next range. No wrap, so a key at the end does nothing.
pub fn episode_range_step(count: u32, current: u32, by: i32) -> u32 {
    if count == 0 {
        return 0;
    }
    let stepped = i64::from(current) + i64::from(by);
    stepped.clamp(0, i64::from(count - 1)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{catalog::Entry, progress::TitleProgress};

    fn entry() -> Entry {
        Entry::new(AnilistId::new(900_301), "Paper Crane Courier")
    }

    fn rows(count: usize) -> Vec<EpisodeRow> {
        (1..=count)
            .map(|n| EpisodeRow {
                number: n.to_string(),
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
                unsourced: false,
                past_progress: false,
                spoiler_ahead: false,
                synopsis_pending: false,
            })
            .collect()
    }

    #[test]
    fn a_finished_show_says_its_episodes_and_a_dub_only_when_a_source_said_one() {
        let mut e = Entry { episodes: Some(12), ..entry() };
        let line = count_line(&e).unwrap();
        assert_eq!((line.long.as_str(), line.short.as_str()), ("12 episodes", "12 ep"));
        assert_eq!(line.spoken, "12 episodes");
        assert_eq!(line.dub, None, "a card never says No dub");

        e.dub = Some(DubKnown { through: Some(8), complete: false });
        let line = count_line(&e).unwrap();
        assert_eq!(line.long, "12 episodes \u{b7} Dub 8");
        assert_eq!(line.short, "12 ep \u{b7} Dub 8");
        assert_eq!(line.spoken, "12 episodes, dubbed to episode 8");
        assert_eq!(line.dub.as_deref(), Some("Dub 8"));

        e.dub = Some(DubKnown { through: None, complete: false });
        let line = count_line(&e).unwrap();
        assert_eq!(line.long, "12 episodes \u{b7} Dub");
        assert_eq!(line.spoken, "12 episodes, has a dub");
    }

    #[test]
    fn an_airing_show_says_how_many_are_out() {
        let e = Entry {
            episodes: Some(24),
            next_episode: Some(6),
            dub: Some(DubKnown { through: Some(3), complete: false }),
            ..entry()
        };
        let line = count_line(&e).unwrap();
        assert_eq!(line.long, "5 of 24 out \u{b7} Dub 3");
        assert_eq!(line.short, "5 of 24 \u{b7} Dub 3");
        assert_eq!(line.spoken, "5 of 24 episodes out, dubbed to episode 3");

        let unknown = Entry { episodes: None, next_episode: Some(6), ..entry() };
        let line = count_line(&unknown).unwrap();
        assert_eq!((line.long.as_str(), line.short.as_str()), ("5 episodes out", "5 out"));
    }

    #[test]
    fn a_show_not_out_yet_says_when_it_starts_or_that_it_has_not() {
        let dated = Entry {
            status: Some(AiringStatus::NotYetReleased),
            start_date: Some(FuzzyDate { year: 2027, month: Some(1), day: Some(9) }),
            ..entry()
        };
        let line = count_line(&dated).unwrap();
        assert_eq!(line.long, "Starts 9 Jan");
        assert_eq!(line.spoken, "Starts 9 January");
        let undated = Entry {
            start_date: Some(FuzzyDate { year: 2027, month: Some(1), day: None }),
            ..dated
        };
        assert_eq!(count_line(&undated).unwrap().long, "Not out yet");
    }

    #[test]
    fn a_film_gives_its_length_and_a_dub_is_a_yes() {
        let film = Entry {
            format: Some("MOVIE".into()),
            episodes: Some(1),
            film_length: Some(crate::catalog::RunLength { hours: 1, minutes: 52 }),
            dub: Some(DubKnown { through: Some(1), complete: true }),
            ..entry()
        };
        let line = count_line(&film).unwrap();
        assert_eq!(line.long, "1 h 52 min \u{b7} Dub");
    }

    #[test]
    fn a_library_row_in_progress_says_where_the_viewer_is() {
        let watching = Entry {
            episodes: Some(12),
            list_status: Some(LibrarySegment::Watching),
            progress: Some(TitleProgress { done: 6, next: 7 }),
            dub: Some(DubKnown { through: Some(8), complete: false }),
            ..entry()
        };
        let line = count_line(&watching).unwrap();
        assert_eq!(line.long, "Ep 7 of 12 \u{b7} Dub 8");
        assert_eq!(line.spoken, "Episode 7 of 12, dubbed to episode 8");
        let done = Entry { list_status: Some(LibrarySegment::Completed), ..watching };
        assert_eq!(count_line(&done).unwrap().long, "12 episodes \u{b7} Dub 8");
    }

    #[test]
    fn nothing_known_says_nothing() {
        assert_eq!(count_line(&entry()), None);
        let only_dub =
            Entry { dub: Some(DubKnown { through: None, complete: false }), ..entry() };
        assert_eq!(count_line(&only_dub).unwrap().long, "Dub");
    }

    #[test]
    fn go_to_episode_finds_the_row_and_its_range() {
        let table = rows(412);
        assert_eq!(go_to_episode(&table, "", 0), GoTo::Empty);
        assert_eq!(go_to_episode(&table, " 537 ", 0), GoTo::NoSuch { number: 537, last: 412 });
        assert_eq!(
            go_to_message(&GoTo::NoSuch { number: 537, last: 412 }).as_deref(),
            Some("No episode 537. This show has 412.")
        );
        assert_eq!(go_to_episode(&table, "250", 0), GoTo::Row { index: 249, range: Some(2) });
        assert_eq!(go_to_episode(&rows(12), "5", 0), GoTo::Row { index: 4, range: None });
        assert_eq!(go_to_episode(&table, "0", 0), GoTo::NoSuch { number: 0, last: 412 });
        assert_eq!(go_to_hint(&table), "1 to 412");
        assert_eq!(go_to_label(&table), "Episode, 1 to 412");
    }

    #[test]
    fn go_to_an_episode_not_out_goes_when_it_has_a_date_and_refuses_when_it_has_none() {
        let now = 1_000;
        let mut table = rows(412);
        for row in table.iter_mut().take(402) {
            row.airs_at = Some(now - 10);
        }
        // 403 is announced with a date; 404 and later have none.
        table[402].airs_at = Some(now + 500);
        assert_eq!(go_to_episode(&table, "403", now), GoTo::Row { index: 402, range: Some(4) });
        assert_eq!(
            go_to_episode(&table, "410", now),
            GoTo::NotOut { number: 410, latest: 402 }
        );
        assert_eq!(
            go_to_message(&GoTo::NotOut { number: 410, latest: 402 }).as_deref(),
            Some("Episode 410 isn't out yet. The latest is 402.")
        );
        // A table with no dates at all has every episode out.
        assert_eq!(
            go_to_episode(&rows(412), "410", now),
            GoTo::Row { index: 409, range: Some(4) }
        );
    }

    #[test]
    fn the_player_panel_opens_on_the_playing_episodes_range() {
        let table = rows(412);
        assert_eq!(episode_range_of(&table, "402"), Some(4));
        assert_eq!(episode_range_of(&table, "7"), Some(0));
        assert_eq!(episode_range_of(&table, "999"), None);
        assert_eq!(episode_range_of(&rows(12), "7"), None, "a short show has no ranges");
        assert_eq!(episode_range_step(5, 4, 1), 4);
        assert_eq!(episode_range_step(5, 0, -1), 0);
        assert_eq!(episode_range_step(5, 1, 1), 2);
    }

    #[test]
    fn periods_cycle_and_keep_for_their_own_time() {
        assert_eq!(TrendPeriod::Today.next(), TrendPeriod::Week);
        assert_eq!(TrendPeriod::Month.next(), TrendPeriod::Today);
        assert_eq!(TrendPeriod::Week.kept_for(), 6 * 3_600);
        assert_eq!(TrendPeriod::Week.label(), "Trending this week");
    }
}
