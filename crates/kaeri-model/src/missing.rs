//! An episode that has aired and that none of the viewer's sources lists yet.
//!
//! The title page names it, says when it aired and offers Check again, and Kaeri looks again by
//! itself every [`RECHECK_EVERY_SECS`] for [`RECHECK_FOR_SECS`] after it airs. A week on
//! ([`LONG_AGO_SECS`]) waiting is no longer the likely reason, so the words change and the
//! looking stops. The source picker says what each source has of the title, in ranges.
//!
//! Everything here is worked out from the episode table and the clock, so every front end
//! changes its words at the same moment. The front ends write the sentences; this says which.

use kaeri_core::ids::AnilistId;

use crate::{episodes::EpisodeRow, sources::SourceOption};

/// How often Kaeri looks again by itself for an episode that aired and no source lists.
pub const RECHECK_EVERY_SECS: i64 = 15 * 60;

/// How long after an episode airs Kaeri keeps looking by itself.
pub const RECHECK_FOR_SECS: i64 = 6 * 3_600;

/// How long after an episode airs the page stops saying the sources don't have it *yet*.
pub const LONG_AGO_SECS: i64 = 7 * 24 * 3_600;

const HOUR: i64 = 3_600;
const DAY: i64 = 24 * HOUR;

/// Where a missing episode stands, by the time since it aired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum MissingPhase {
    /// Within [`RECHECK_FOR_SECS`] of airing: Kaeri looks again every 15 minutes, and says so.
    Quiet,
    /// Past the quiet window and within [`LONG_AGO_SECS`]: Check again only.
    After,
    /// [`LONG_AGO_SECS`] or more after airing: other words, no Check again and no recheck.
    LongAgo,
    /// The schedule does not say when it aired: the page keeps the plain words, with Check
    /// again.
    Undated,
}

/// The episode a title's page would play next, when it has aired and no source lists it, and
/// no later episode has a source either: what the page says in the place of Play.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MissingEpisode {
    pub episode: u32,
    /// When it aired, in Unix seconds. `None` when the schedule does not say.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub aired_at: Option<i64>,
    pub phase: MissingPhase,
    /// The episode before it, when a source has it: what the page offers to play meanwhile.
    /// `None` for a first episode, or one whose predecessor no source lists either.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub previous: Option<u32>,
    /// How often Kaeri looks again, in minutes: the 15 of "every 15 minutes".
    pub every_minutes: u32,
    /// How long Kaeri keeps looking after an episode airs, in hours.
    pub for_hours: u32,
}

impl MissingEpisode {
    /// Whether the page offers Check again: always, until [`MissingPhase::LongAgo`].
    pub fn checkable(&self) -> bool {
        self.phase != MissingPhase::LongAgo
    }
}

/// What a check of a missing episode came to: the answer to
/// [`MissingCommand::CheckAgain`], and what a quiet recheck that found it says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CheckOutcome {
    /// A source lists the episode now. The title's table has come ahead of this.
    Found,
    /// The sources answered, and none lists it. Some may have failed: those say why in the
    /// source picker.
    Missing,
    /// No source answered.
    NoAnswer,
    /// The device is offline, so nothing was asked.
    Offline,
}

/// A check of title `id`'s episode `episode`, and what it came to.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EpisodeCheck {
    pub id: AnilistId,
    pub episode: u32,
    pub outcome: CheckOutcome,
    /// The person asked for it with Check again. A quiet recheck says only what it found, and
    /// only for a title on screen.
    pub asked: bool,
}

/// What a front end asks of a missing episode.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum MissingCommand {
    /// Ask the sources again for title `id`'s episode `episode`, now. The title's table comes
    /// back as on any load, then [`EpisodeCheck`] with `asked` set. Offline it asks nothing and
    /// answers [`CheckOutcome::Offline`] at once.
    CheckAgain { id: AnilistId, episode: u32 },
}

/// Where `aired_at` stands at `now`: see [`MissingPhase`].
pub fn missing_phase(aired_at: Option<i64>, now: i64) -> MissingPhase {
    let Some(aired_at) = aired_at else { return MissingPhase::Undated };
    let since = now - aired_at;
    if since >= LONG_AGO_SECS {
        MissingPhase::LongAgo
    } else if since > RECHECK_FOR_SECS {
        MissingPhase::After
    } else {
        MissingPhase::Quiet
    }
}

/// The missing episode a title's page shows, when it has one: the next episode, `next`, aired
/// and no source lists it, and no later episode has a source. A part-watched episode
/// (`resuming`) is resumed instead, and a table with a sourced later episode plays that one, as
/// it does today. `now` is Unix seconds.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub fn missing_episode(
    rows: Vec<EpisodeRow>,
    next: u32,
    resuming: bool,
    now: i64,
) -> Option<MissingEpisode> {
    missing_in(&rows, next, resuming, now)
}

/// [`missing_episode`], over a borrowed table.
pub fn missing_in(
    rows: &[EpisodeRow],
    next: u32,
    resuming: bool,
    now: i64,
) -> Option<MissingEpisode> {
    if resuming {
        return None;
    }
    let numbered =
        || rows.iter().filter_map(|r| Some((r.number.trim().parse::<u32>().ok()?, r)));
    let (_, row) = numbered().find(|(n, _)| *n == next)?;
    if !row.unsourced || row.airs_at.is_some_and(|at| at > now) {
        return None;
    }
    let sourced = |r: &EpisodeRow| !r.unsourced && !r.metadata_only;
    if numbered().any(|(n, r)| n > next && sourced(r) && r.airs_at.is_none_or(|at| at <= now)) {
        return None;
    }
    let previous = next
        .checked_sub(1)
        .filter(|p| *p >= 1)
        .filter(|p| numbered().any(|(n, r)| n == *p && sourced(r)));
    Some(MissingEpisode {
        episode: next,
        aired_at: row.airs_at,
        phase: missing_phase(row.airs_at, now),
        previous,
        every_minutes: (RECHECK_EVERY_SECS / 60) as u32,
        for_hours: (RECHECK_FOR_SECS / HOUR) as u32,
    })
}

/// How long ago an episode aired, as a status or a row says it: minutes for the first hour,
/// whole hours for the first day, then the date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum AiredAgo {
    /// "25 min ago". Never under one.
    Minutes { minutes: u32 },
    /// "2 h ago".
    Hours { hours: u32 },
    /// A day or more: the front end writes the date ("on 7 Nov").
    OnDate,
}

/// How long ago `aired_at` was at `now`: see [`AiredAgo`]. Updated on the minute, with no
/// motion, and never announced.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub fn aired_ago(aired_at: i64, now: i64) -> AiredAgo {
    let since = (now - aired_at).max(0);
    if since < HOUR {
        AiredAgo::Minutes { minutes: (since / 60).max(1) as u32 }
    } else if since < DAY {
        AiredAgo::Hours { hours: (since / HOUR) as u32 }
    } else {
        AiredAgo::OnDate
    }
}

/// What an unsourced episode's row says after its date: "Not in your sources yet" for the first
/// week after it aired, and "No source has this episode" from then on, or when the schedule
/// does not date it.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub fn unsourced_yet(aired_at: Option<i64>, now: i64) -> bool {
    aired_at.is_some_and(|at| at <= now && now - at < LONG_AGO_SECS)
}

/// The episodes a source lacks, in one of the shapes a line can say: "6", "5 or 6", "4 to 6".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Lacking {
    One { episode: u32 },
    Pair { first: u32, second: u32 },
    Range { first: u32, last: u32 },
}

impl Lacking {
    /// `episodes`, sorted, as one shape, when they make one: one, two, or three or more in a row.
    fn of(episodes: &[u32]) -> Option<Self> {
        match episodes {
            [] => None,
            [one] => Some(Self::One { episode: *one }),
            [first, second] => Some(Self::Pair { first: *first, second: *second }),
            [first, .., last] if (last - first) as usize + 1 == episodes.len() => {
                Some(Self::Range { first: *first, last: *last })
            }
            _ => None,
        }
    }
}

/// What a source's line in the source picker says it has of the title: see [`coverage_line`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CoverageLine {
    /// Every episode out: "Episodes 1 to 6", or "Episode 1" when `count` is one.
    Episodes { first: u32, last: u32, count: u32 },
    /// The newest ones missing, the first of them aired within a week: "Episodes 1 to 5. Not 6
    /// yet".
    NotYet { first: u32, last: u32, lacking: Lacking },
    /// The same, a week or more on: "Episodes 1 to 5. Not 6".
    Not { first: u32, last: u32, lacking: Lacking },
    /// A gap elsewhere: "5 of 6 episodes. Not 3".
    Gap { covered: u32, of: u32, lacking: Lacking },
    /// Any other shape, as the count it always was: "5 of 6 episodes".
    Count { covered: u32, of: u32 },
}

/// What `option`'s line says it has at `now`, or `None` for a source with a note (off,
/// resting, failed and the like), whose line says that instead.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub fn coverage_line(option: SourceOption, now: i64) -> Option<CoverageLine> {
    coverage_of(&option, now)
}

/// [`coverage_line`], over a borrowed option.
pub fn coverage_of(option: &SourceOption, now: i64) -> Option<CoverageLine> {
    if option.note.is_some() {
        return None;
    }
    let count = CoverageLine::Count { covered: option.covered, of: option.of };
    let (Some(first), Some(last)) = (option.first, option.last) else { return Some(count) };
    if option.of == 0 || option.covered == 0 {
        return Some(count);
    }
    let mut lacking = option.lacking.clone();
    lacking.sort_unstable();
    lacking.dedup();
    if lacking.is_empty() {
        return Some(CoverageLine::Episodes { first, last, count: option.covered });
    }
    let Some(shape) = Lacking::of(&lacking) else { return Some(count) };
    let tail = lacking[0] == last + 1 && lacking.windows(2).all(|w| w[1] == w[0] + 1);
    Some(if !tail {
        CoverageLine::Gap { covered: option.covered, of: option.of, lacking: shape }
    } else if unsourced_yet(option.lacking_since, now) {
        CoverageLine::NotYet { first, last, lacking: shape }
    } else {
        CoverageLine::Not { first, last, lacking: shape }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    fn row(n: u32, unsourced: bool, airs_at: Option<i64>) -> EpisodeRow {
        EpisodeRow {
            number: n.to_string(),
            title: None,
            thumbnail: None,
            description: None,
            duration_secs: None,
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            airs_at,
            delayed_from: None,
            metadata_only: false,
            source: None,
            unsourced,
            past_progress: false,
            spoiler_ahead: false,
            synopsis_pending: false,
        }
    }

    /// Episodes 1 to 5 listed, 6 aired `ago` seconds before [`NOW`] and listed by none, 7 a
    /// week on.
    fn season(ago: i64) -> Vec<EpisodeRow> {
        let mut rows: Vec<_> =
            (1..=5).map(|n| row(n, false, Some(NOW - DAY * 7 * (6 - n as i64)))).collect();
        rows.push(row(6, true, Some(NOW - ago)));
        rows.push(row(7, false, Some(NOW - ago + 7 * DAY)));
        rows
    }

    #[test]
    fn a_just_aired_episode_no_source_lists_is_missing_with_the_one_before_it() {
        let missing = missing_in(&season(25 * 60), 6, false, NOW).unwrap();
        assert_eq!(missing.episode, 6);
        assert_eq!(missing.phase, MissingPhase::Quiet);
        assert_eq!(missing.previous, Some(5));
        assert_eq!((missing.every_minutes, missing.for_hours), (15, 6));
        assert!(missing.checkable());
    }

    #[test]
    fn the_phase_moves_at_six_hours_and_at_a_week() {
        assert_eq!(missing_phase(Some(NOW - RECHECK_FOR_SECS), NOW), MissingPhase::Quiet);
        assert_eq!(missing_phase(Some(NOW - RECHECK_FOR_SECS - 1), NOW), MissingPhase::After);
        assert_eq!(missing_phase(Some(NOW - LONG_AGO_SECS + 1), NOW), MissingPhase::After);
        assert_eq!(missing_phase(Some(NOW - LONG_AGO_SECS), NOW), MissingPhase::LongAgo);
        assert_eq!(missing_phase(None, NOW), MissingPhase::Undated);
        let long = missing_in(&season(LONG_AGO_SECS + DAY), 6, false, NOW).unwrap();
        assert!(!long.checkable());
    }

    #[test]
    fn nothing_is_missing_while_resuming_or_once_a_source_lists_it() {
        assert!(missing_in(&season(60), 6, true, NOW).is_none(), "Resume wins");
        let mut rows = season(60);
        rows[5].unsourced = false;
        assert!(missing_in(&rows, 6, false, NOW).is_none());
        assert!(missing_in(&rows, 9, false, NOW).is_none(), "no such row");
    }

    #[test]
    fn a_later_episode_with_a_source_plays_as_today() {
        let mut rows = season(60);
        rows[6].airs_at = Some(NOW - 60);
        assert!(missing_in(&rows, 6, false, NOW).is_none());
    }

    #[test]
    fn a_first_episode_has_nothing_before_it() {
        let rows = vec![row(1, true, Some(NOW - 600))];
        assert_eq!(missing_in(&rows, 1, false, NOW).unwrap().previous, None);
    }

    #[test]
    fn aired_ago_is_minutes_then_hours_then_a_date() {
        assert_eq!(aired_ago(NOW - 10, NOW), AiredAgo::Minutes { minutes: 1 });
        assert_eq!(aired_ago(NOW - 25 * 60, NOW), AiredAgo::Minutes { minutes: 25 });
        assert_eq!(aired_ago(NOW - HOUR, NOW), AiredAgo::Hours { hours: 1 });
        assert_eq!(aired_ago(NOW - DAY + 1, NOW), AiredAgo::Hours { hours: 23 });
        assert_eq!(aired_ago(NOW - DAY, NOW), AiredAgo::OnDate);
    }

    #[test]
    fn the_row_says_yet_for_a_week() {
        assert!(unsourced_yet(Some(NOW - 6 * DAY), NOW));
        assert!(!unsourced_yet(Some(NOW - 7 * DAY), NOW));
        assert!(!unsourced_yet(None, NOW));
    }

    fn option(first: u32, last: u32, lacking: &[u32], of: u32) -> SourceOption {
        SourceOption {
            provider_id: "tsubame".into(),
            name: "tsubame".into(),
            covered: of - lacking.len() as u32,
            of,
            pinned: false,
            note: None,
            releases: Vec::new(),
            pinned_release: None,
            first: Some(first),
            last: Some(last),
            lacking: lacking.to_vec(),
            lacking_since: Some(NOW - 25 * 60),
        }
    }

    #[test]
    fn a_source_with_every_episode_says_its_range() {
        assert_eq!(
            coverage_of(&option(1, 6, &[], 6), NOW),
            Some(CoverageLine::Episodes { first: 1, last: 6, count: 6 })
        );
    }

    #[test]
    fn a_source_short_of_the_newest_says_which_it_lacks_yet() {
        assert_eq!(
            coverage_of(&option(1, 5, &[6], 6), NOW),
            Some(CoverageLine::NotYet {
                first: 1,
                last: 5,
                lacking: Lacking::One { episode: 6 }
            })
        );
        assert_eq!(
            coverage_of(&option(1, 4, &[6, 5], 6), NOW),
            Some(CoverageLine::NotYet {
                first: 1,
                last: 4,
                lacking: Lacking::Pair { first: 5, second: 6 }
            })
        );
        let mut old = option(1, 3, &[4, 5, 6], 6);
        old.lacking_since = Some(NOW - 8 * DAY);
        assert_eq!(
            coverage_of(&old, NOW),
            Some(CoverageLine::Not {
                first: 1,
                last: 3,
                lacking: Lacking::Range { first: 4, last: 6 }
            })
        );
    }

    #[test]
    fn a_gap_in_the_middle_is_counted_and_named() {
        assert_eq!(
            coverage_of(&option(1, 6, &[3], 6), NOW),
            Some(CoverageLine::Gap { covered: 5, of: 6, lacking: Lacking::One { episode: 3 } })
        );
        assert_eq!(
            coverage_of(&option(1, 9, &[2, 5, 7], 9), NOW),
            Some(CoverageLine::Count { covered: 6, of: 9 }),
            "three apart is no shape a line can say"
        );
    }

    #[test]
    fn a_source_with_a_note_or_no_range_keeps_its_words() {
        let mut noted = option(1, 5, &[6], 6);
        noted.note = Some(crate::sources::SourceNote::Resting);
        assert_eq!(coverage_of(&noted, NOW), None);
        let mut bare = option(1, 5, &[6], 6);
        bare.first = None;
        assert_eq!(coverage_of(&bare, NOW), Some(CoverageLine::Count { covered: 5, of: 6 }));
    }
}
