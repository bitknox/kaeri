//! The words for an aired episode that no source lists yet: the title page's status line under
//! its buttons, the episode row's line, and what Source for this title says each source has.
//! The engine says which state it is in (`kaeri_model::missing`); this writes the sentences.
//! Pure, so it is tested without a display.

use kaeri_i18n::t;
use kaeri_model::{
    episodes::EpisodeRow,
    missing::{
        self as model, AiredAgo, CheckOutcome, CoverageLine, Lacking, MissingEpisode,
        MissingPhase,
    },
    sources::SourceOption,
};

/// "25 min ago" or "2 h ago" within a day of `aired_at`; `None` from a day on, where the date
/// says it.
pub fn ago(aired_at: i64, now: i64) -> Option<String> {
    match model::aired_ago(aired_at, now) {
        AiredAgo::Minutes { minutes } => {
            Some(t::time_ago(&t::time_minutes(i64::from(minutes))))
        }
        AiredAgo::Hours { hours } => Some(t::time_ago(&t::time_hours(i64::from(hours)))),
        AiredAgo::OnDate => None,
    }
}

/// The status as the page shows it: the first sentence (in silk) and the rest (in haze). A
/// status with no head is all haze: an episode the schedule does not date keeps today's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub head: Option<String>,
    pub tail: String,
}

impl Status {
    /// The whole line, as a screen reader reads it.
    pub fn text(&self) -> String {
        match &self.head {
            Some(head) => format!("{head} {}", self.tail), // i18n-skip
            None => self.tail.clone(),
        }
    }

    /// Where the head ends in [`Self::text`], in bytes.
    pub fn head_len(&self) -> usize {
        self.head.as_ref().map_or(0, String::len)
    }
}

/// The status of `missing` at `now`: when it aired, then what the sources said. `answer` is
/// what the last check the viewer asked for came to; quiet rechecks that come back empty change
/// nothing. `date` writes a day as the page's rows do.
pub fn status(
    missing: &MissingEpisode,
    answer: Option<CheckOutcome>,
    offline: bool,
    now: i64,
    date: impl Fn(i64) -> String,
) -> Status {
    let episode = i64::from(missing.episode);
    let head =
        missing.aired_at.filter(|_| missing.phase != MissingPhase::Undated).map(
            |at| match ago(at, now) {
                Some(ago) => t::episode_missing_aired(episode, &ago),
                None => t::episode_missing_aired_on(episode, &date(at)),
            },
        );
    Status { head, tail: tail(missing, answer, offline) }
}

/// The sentences after the head. Inside the quiet window Kaeri says it looks again, or, offline,
/// that it looks once back online; past it, nothing rechecks and offline keeps "still".
fn tail(missing: &MissingEpisode, answer: Option<CheckOutcome>, offline: bool) -> String {
    let offline = offline || answer == Some(CheckOutcome::Offline);
    match missing.phase {
        MissingPhase::LongAgo => t::episode_missing_none(),
        MissingPhase::Undated => t::episode_unsourced(),
        MissingPhase::After => match answer {
            Some(CheckOutcome::NoAnswer) if !offline => t::episode_missing_no_answer(),
            _ => t::episode_missing_still(),
        },
        MissingPhase::Quiet if offline => t::episode_missing_offline(),
        MissingPhase::Quiet => {
            let said = match answer {
                Some(CheckOutcome::Missing) => t::episode_missing_still(),
                Some(CheckOutcome::NoAnswer) => t::episode_missing_no_answer(),
                _ => t::episode_missing_not_yet(),
            };
            let looks = t::episode_missing_looks_again(i64::from(missing.every_minutes));
            format!("{said} {looks}") // i18n-skip
        }
    }
}

/// Check again's tooltip and accessible description: Kaeri also looks by itself, said only while
/// it does.
pub fn check_help(missing: &MissingEpisode) -> Option<String> {
    (missing.phase == MissingPhase::Quiet).then(|| {
        t::episode_check_again_help(
            i64::from(missing.every_minutes),
            i64::from(missing.for_hours),
        )
    })
}

/// What a check the viewer asked for came to, said to a screen reader once. Found says the
/// episode is in the sources now; offline says the device is.
pub fn answer_said(episode: u32, outcome: CheckOutcome) -> String {
    match outcome {
        CheckOutcome::Found => t::episode_found_announcement(i64::from(episode)),
        CheckOutcome::Missing => t::episode_missing_still(),
        CheckOutcome::NoAnswer => t::episode_missing_no_answer(),
        CheckOutcome::Offline => t::state_offline_line(),
    }
}

/// The line of an episode row no source lists: when it aired and that the sources don't have
/// it yet, for a week after it aired; then the date and today's words. With no airing time, or
/// one still ahead, today's words alone.
pub fn row_line(row: &EpisodeRow, now: i64, date: impl Fn(i64) -> String) -> String {
    let Some(at) = row.airs_at.filter(|at| *at <= now) else { return t::episode_unsourced() };
    let when = match ago(at, now) {
        Some(ago) => t::episode_row_aired_ago(&ago),
        None => t::media_aired_on(&date(at)),
    };
    let state = if model::unsourced_yet(Some(at), now) {
        t::episode_unsourced_yet()
    } else {
        t::episode_unsourced()
    };
    t::episode_facts_join(&when, &state)
}

/// What a source has of the title, under its name in Source for this title: "Episodes 1 to 5.
/// Not 6 yet". `None` for a source with a note, which says that instead.
pub fn coverage(option: &SourceOption, now: i64) -> Option<String> {
    let lacking = |lacking: Lacking| match lacking {
        Lacking::One { episode } => episode.to_string(),
        Lacking::Pair { first, second } => {
            t::source_picker_missing_pair(i64::from(first), i64::from(second))
        }
        Lacking::Range { first, last } => {
            t::source_picker_missing_range(i64::from(first), i64::from(last))
        }
    };
    Some(match model::coverage_of(option, now)? {
        CoverageLine::Episodes { first, last, count } => {
            t::source_picker_episodes(i64::from(count), i64::from(first), i64::from(last))
        }
        CoverageLine::NotYet { first, last, lacking: l } => {
            t::source_picker_episodes_not_yet(i64::from(first), i64::from(last), &lacking(l))
        }
        CoverageLine::Not { first, last, lacking: l } => {
            t::source_picker_episodes_not(i64::from(first), i64::from(last), &lacking(l))
        }
        CoverageLine::Gap { covered, of, lacking: l } => {
            t::source_picker_coverage_missing(i64::from(covered), i64::from(of), &lacking(l))
        }
        CoverageLine::Count { covered, of } => {
            t::source_picker_coverage(i64::from(covered), i64::from(of))
        }
    })
}

#[cfg(test)]
mod tests {
    use kaeri_model::missing::{LONG_AGO_SECS, RECHECK_FOR_SECS};

    use super::*;

    const NOW: i64 = 1_800_000_000;
    const DAY: i64 = 24 * 3_600;

    fn date(_: i64) -> String {
        "7 Nov 2026".into()
    }

    fn missing(ago: Option<i64>) -> MissingEpisode {
        let aired_at = ago.map(|ago| NOW - ago);
        MissingEpisode {
            episode: 6,
            aired_at,
            phase: model::missing_phase(aired_at, NOW),
            previous: Some(5),
            every_minutes: 15,
            for_hours: 6,
        }
    }

    fn said(m: &MissingEpisode, answer: Option<CheckOutcome>, offline: bool) -> String {
        status(m, answer, offline, NOW, date).text()
    }

    #[test]
    fn just_aired_says_when_and_that_kaeri_looks_again() {
        let m = missing(Some(25 * 60));
        assert_eq!(
            said(&m, None, false),
            "Episode 6 aired 25 min ago. Your sources don't have it yet. Kaeri looks again \
             every 15 minutes."
        );
        let s = status(&m, None, false, NOW, date);
        assert_eq!(&s.text()[..s.head_len()], "Episode 6 aired 25 min ago.");
        assert!(check_help(&m).is_some_and(|h| h.contains("15 minutes for 6 hours")));
    }

    #[test]
    fn an_asked_check_turns_yet_into_still_or_no_answer() {
        let m = missing(Some(40 * 60));
        assert!(said(&m, Some(CheckOutcome::Missing), false).contains("still don't have it"));
        assert!(said(&m, Some(CheckOutcome::Missing), false).ends_with("every 15 minutes."));
        assert!(said(&m, Some(CheckOutcome::NoAnswer), false).contains("didn't answer"));
    }

    #[test]
    fn hours_then_a_date_then_other_words() {
        let hours = missing(Some(2 * 3_600 + 60));
        assert!(said(&hours, None, false).starts_with("Episode 6 aired 2 h ago."));
        let after = missing(Some(RECHECK_FOR_SECS + 60));
        assert_eq!(
            said(&after, None, false),
            "Episode 6 aired 6 h ago. Your sources still don't have it."
        );
        assert!(check_help(&after).is_none(), "the hint goes with the quiet window");
        let dated = missing(Some(2 * DAY));
        assert_eq!(
            said(&dated, None, false),
            "Episode 6 aired on 7 Nov 2026. Your sources still don't have it."
        );
        let long = missing(Some(LONG_AGO_SECS + DAY));
        assert_eq!(
            said(&long, None, false),
            "Episode 6 aired on 7 Nov 2026. None of your sources has it."
        );
    }

    #[test]
    fn offline_promises_a_look_only_inside_the_quiet_window() {
        let quiet = missing(Some(25 * 60));
        assert_eq!(
            said(&quiet, None, true),
            "Episode 6 aired 25 min ago. Kaeri looks for it when you're back online."
        );
        assert_eq!(said(&quiet, Some(CheckOutcome::Offline), false), said(&quiet, None, true));
        let after = missing(Some(RECHECK_FOR_SECS + 60));
        assert!(
            said(&after, Some(CheckOutcome::NoAnswer), true).ends_with("still don't have it.")
        );
    }

    #[test]
    fn an_undated_episode_keeps_todays_words() {
        let undated = missing(None);
        let s = status(&undated, None, false, NOW, date);
        assert_eq!(s.head, None);
        assert_eq!(s.text(), t::episode_unsourced());
    }

    #[test]
    fn the_row_says_not_in_your_sources_yet_for_a_week() {
        let row = |ago: Option<i64>| EpisodeRow {
            number: "6".into(),
            title: None,
            thumbnail: None,
            description: None,
            duration_secs: None,
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            airs_at: ago.map(|ago| NOW - ago),
            delayed_from: None,
            metadata_only: false,
            followed: Vec::new(),
            source: None,
            unsourced: true,
            past_progress: false,
            spoiler_ahead: false,
            synopsis_pending: false,
        };
        assert_eq!(
            row_line(&row(Some(25 * 60)), NOW, date),
            "Aired 25 min ago · Not in your sources yet"
        );
        assert_eq!(
            row_line(&row(Some(2 * DAY)), NOW, date),
            "Aired 7 Nov 2026 · Not in your sources yet"
        );
        assert_eq!(
            row_line(&row(Some(LONG_AGO_SECS)), NOW, date),
            "Aired 7 Nov 2026 · No source has this episode"
        );
        assert_eq!(row_line(&row(None), NOW, date), t::episode_unsourced());
    }

    #[test]
    fn a_source_says_which_episodes_it_lacks() {
        let option = |first, last, lacking: Vec<u32>, covered| SourceOption {
            first: Some(first),
            last: Some(last),
            lacking,
            lacking_since: Some(NOW - 25 * 60),
            covered,
            of: 6,
            provider_id: "a".into(),
            name: "Source a".into(),
            pinned: false,
            note: None,
            releases: Vec::new(),
            pinned_release: None,
        };
        assert_eq!(
            coverage(&option(1, 5, vec![6], 5), NOW).unwrap(),
            "Episodes 1 to 5. Not 6 yet"
        );
        assert_eq!(
            coverage(&option(1, 4, vec![5, 6], 4), NOW).unwrap(),
            "Episodes 1 to 4. Not 5 or 6 yet"
        );
        assert_eq!(coverage(&option(1, 6, vec![], 6), NOW).unwrap(), "Episodes 1 to 6");
        let mut noted = option(1, 6, vec![], 6);
        noted.note = Some(kaeri_model::sources::SourceNote::Resting);
        assert_eq!(coverage(&noted, NOW), None);
    }
}
