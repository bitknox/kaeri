//! What the episode list's rows decide about synopses, apart from any widget: how tall a row is,
//! which rows spoiler protection holds back and where its line goes, what a row's facts say, and
//! how a long series splits into ranges with its watched episodes folded. The title page draws
//! what these say.

use kaeri_i18n::t;
use kaeri_model::episodes::{EpisodeKind, EpisodeRange, EpisodeRow, EpisodeText};

/// The still beside the text on a wide row, and under the title on a narrow one.
pub const STILL_WIDE: i32 = 192;
pub const STILL_NARROW: i32 = 144;

/// A row's fixed height: 128 beside the text, and on a narrow window 164 with the text under the
/// still or 100 without. Computed from the size, never from the text.
pub const ROW_WIDE: i32 = 128;
pub const ROW_NARROW_TALL: i32 = 164;
pub const ROW_NARROW_SHORT: i32 = 100;

/// The room three lines of body text take.
pub const TEXT_HEIGHT: i32 = 60;

/// How many leading watched rows are worth a line of their own.
const FOLD_LEAST: u32 = 2;

/// The row's height in a layout of `narrow` width. A wide row keeps its height whether or not it
/// has text: the text sits beside the still.
pub fn row_height(narrow: bool, tall: bool) -> i32 {
    match (narrow, tall) {
        (false, _) => ROW_WIDE,
        (true, true) => ROW_NARROW_TALL,
        (true, false) => ROW_NARROW_SHORT,
    }
}

/// Whether a row keeps room for text: text is there or on its way, and spoiler protection is not
/// holding this row back.
pub fn has_room(text: EpisodeText, protected: bool) -> bool {
    text != EpisodeText::None && !protected
}

/// The row's synopsis, when it has one.
pub fn text_of(row: &EpisodeRow) -> Option<&str> {
    row.description.as_deref().map(str::trim).filter(|text| !text.is_empty())
}

/// Whether spoiler protection holds this row back: it is on, the engine says the row is ahead
/// (`spoiler_ahead`), and the viewer has not asked to see it.
pub fn protected(row: &EpisodeRow, on: bool, shown: bool) -> bool {
    protectable(row, on) && !shown
}

/// Whether protection could hold this row back: it is ahead, whether or not the viewer has asked
/// to see it. Such a row keeps a blurred copy of its still to fade away.
pub fn protectable(row: &EpisodeRow, on: bool) -> bool {
    on && row.spoiler_ahead
}

/// Where spoiler protection starts in `rows`: the index of the first row ahead, and the number of
/// the last episode before it, which the line names.
pub fn protection_start(rows: &[EpisodeRow], on: bool) -> Option<(usize, i64)> {
    let at = rows.iter().position(|row| protectable(row, on))?;
    let before = at
        .checked_sub(1)
        .and_then(|i| number(&rows[i].number))
        .or_else(|| number(&rows[at].number).map(|n| n - 1))
        .unwrap_or(0);
    Some((at, before.max(0)))
}

/// An episode number as a whole number, a half episode taking the one before it.
pub fn number(text: &str) -> Option<i64> {
    text.parse::<i64>().ok().or_else(|| text.parse::<f64>().ok().map(|n| n.floor() as i64))
}

/// A row's facts line: its length (or what is left of it) and when it aired, a filler episode
/// saying so, an unreleased one saying when it airs. `date` words a Unix time; `now` is Unix
/// seconds. With `short`, the narrow form: the episode's number first ("E8"), and the air date on
/// a line of its own.
pub fn facts(
    row: &EpisodeRow,
    now: i64,
    date: impl Fn(i64) -> String,
    short: Option<i64>,
) -> String {
    let aired = row.airs_at.filter(|at| *at <= now);
    let minutes = |secs: f64| (secs / 60.0).round().max(1.0) as i64;
    let (head, when) = if row.unsourced {
        (t::episode_unsourced(), None)
    } else if let Some(secs) = row.duration_secs {
        // The episode playing next keeps its air date beside the time left.
        let head = if row.watched > 0.0 && !row.completed {
            t::episode_facts_left(minutes(secs as f64 * (1.0 - row.watched)))
        } else {
            t::episode_facts_length(minutes(secs as f64))
        };
        (head, aired.map(|at| t::episode_aired(&date(at))))
    } else if let Some(at) = row.airs_at.filter(|at| *at > now) {
        (t::episode_row_airs(&date(at)), None)
    } else if row.metadata_only {
        (t::episode_row_not_out(), None)
    } else if let Some(at) = aired {
        (date(at), None)
    } else {
        (String::new(), None)
    };
    let filler = (row.kind == Some(EpisodeKind::Filler)).then(t::episode_filler);
    let join = |first: String, second: String| match (first.is_empty(), second.is_empty()) {
        (true, _) => second,
        (_, true) => first,
        _ => t::episode_facts_join(&first, &second),
    };
    match short {
        None => {
            let line = join(head, when.unwrap_or_default());
            match filler {
                Some(filler) if !line.is_empty() => join(line, filler),
                _ => line,
            }
        }
        Some(number) => {
            let mut first = join(t::episode_short(number), head);
            if let Some(filler) = filler {
                first = join(first, filler);
            }
            match when {
                Some(when) => format!("{first}\n{when}"), // i18n-skip
                None => first,
            }
        }
    }
}

/// The range menu's items for `ranges`: the label and the state each says.
pub struct RangeItem {
    pub label: String,
    /// "Watched" or "Not out yet", else nothing.
    pub state: Option<String>,
}

/// One item per range, labelled by the episode numbers the range holds.
pub fn range_items(rows: &[EpisodeRow], ranges: &[EpisodeRange]) -> Vec<RangeItem> {
    ranges
        .iter()
        .map(|range| {
            let first = &rows[range.start as usize];
            let last = &rows[range.end as usize - 1];
            let from = number(&first.number).unwrap_or(i64::from(range.start) + 1);
            let to = number(&last.number).unwrap_or(i64::from(range.end));
            RangeItem {
                label: t::episode_range(from, to),
                state: if range.watched {
                    Some(t::episode_watched())
                } else if range.not_out {
                    Some(t::episode_row_not_out())
                } else {
                    None
                },
            }
        })
        .collect()
}

/// What one range shows: its rows `start..end`, with the first `fold` of them standing as one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub start: usize,
    pub end: usize,
    pub fold: usize,
}

/// The plan for `range` of `rows`, or for the whole table when it has no ranges. `fold` is the
/// engine's count of leading watched rows; a run shorter than two stays as rows, and nothing
/// folds once the viewer opened the line.
pub fn plan(
    rows: &[EpisodeRow],
    range: Option<&EpisodeRange>,
    fold: u32,
    opened: bool,
) -> Plan {
    let (start, end) = range.map_or((0, rows.len()), |r| (r.start as usize, r.end as usize));
    let fold = if opened || fold < FOLD_LEAST { 0 } else { fold as usize };
    Plan { start, end, fold }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(n: u32) -> EpisodeRow {
        EpisodeRow {
            number: n.to_string(),
            title: None,
            thumbnail: None,
            description: None,
            duration_secs: Some(24 * 60),
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
        }
    }

    fn table(count: u32, next: u32) -> Vec<EpisodeRow> {
        (1..=count)
            .map(|n| {
                let mut r = row(n);
                r.completed = n < next;
                r.past_progress = n > next;
                r.spoiler_ahead = n > next;
                r
            })
            .collect()
    }

    #[test]
    fn wide_rows_keep_their_height_and_narrow_ones_follow_the_text() {
        assert_eq!(row_height(false, true), 128);
        assert_eq!(row_height(false, false), 128);
        assert_eq!(row_height(true, true), 164);
        assert_eq!(row_height(true, false), 100);
    }

    #[test]
    fn room_is_kept_only_where_text_can_land() {
        assert!(has_room(EpisodeText::Present, false));
        assert!(has_room(EpisodeText::Expected, false));
        assert!(!has_room(EpisodeText::None, false));
        assert!(!has_room(EpisodeText::Present, true));
    }

    #[test]
    fn a_row_is_held_back_only_while_protection_is_on_and_unrevealed() {
        let rows = table(4, 2);
        assert!(!protected(&rows[1], true, false));
        assert!(protected(&rows[2], true, false));
        assert!(!protected(&rows[2], false, false));
        assert!(!protected(&rows[2], true, true));
    }

    #[test]
    fn the_line_goes_before_the_first_row_ahead_and_names_the_episode_before_it() {
        let rows = table(5, 2);
        assert_eq!(protection_start(&rows, true), Some((2, 2)));
        assert_eq!(protection_start(&rows, false), None);
        assert_eq!(protection_start(&table(3, 9), true), None);
        let mut first = table(3, 0);
        first.iter_mut().for_each(|r| r.spoiler_ahead = true);
        assert_eq!(protection_start(&first, true), Some((0, 0)));
    }

    #[test]
    fn facts_say_the_length_the_date_and_what_is_left() {
        let date = |_| "21 May 2014".to_owned();
        let mut r = row(1);
        assert_eq!(facts(&r, 100, date, None), "24 min");
        r.airs_at = Some(50);
        assert_eq!(facts(&r, 100, date, None), "24 min · Aired 21 May 2014");
        r.watched = 0.5;
        assert_eq!(facts(&r, 100, date, None), "12 min left · Aired 21 May 2014");
        r.watched = 0.0;
        r.kind = Some(EpisodeKind::Filler);
        assert_eq!(facts(&r, 100, date, None), "24 min · Aired 21 May 2014 · Filler");
        let mut later = row(2);
        later.duration_secs = None;
        later.airs_at = Some(500);
        assert_eq!(facts(&later, 100, date, None), "Airs 21 May 2014");
    }

    #[test]
    fn narrow_facts_lead_with_the_number_and_put_the_date_on_its_own_line() {
        let date = |_| "21 May 2014".to_owned();
        let mut r = row(8);
        r.airs_at = Some(50);
        assert_eq!(facts(&r, 100, date, Some(8)), "E8 · 24 min\nAired 21 May 2014");
        r.watched = 0.5;
        assert_eq!(facts(&r, 100, date, Some(8)), "E8 · 12 min left\nAired 21 May 2014");
        r.watched = 0.0;
        r.kind = Some(EpisodeKind::Filler);
        assert_eq!(facts(&r, 100, date, Some(8)), "E8 · 24 min · Filler\nAired 21 May 2014");
        r.airs_at = None;
        assert_eq!(facts(&r, 100, date, Some(8)), "E8 · 24 min · Filler");
    }

    #[test]
    fn range_items_are_worded_by_the_numbers_they_hold_and_say_what_is_done() {
        let mut rows = table(112, 101);
        rows.iter_mut().skip(100).for_each(|r| r.airs_at = Some(900));
        let ranges = kaeri_model::episodes::episode_ranges(&rows, 100);
        let items = range_items(&rows, &ranges);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "Episodes 1 to 100");
        assert_eq!(items[0].state.as_deref(), Some("Watched"));
        assert_eq!(items[1].label, "Episodes 101 to 112");
        assert_eq!(items[1].state.as_deref(), Some("Not out yet"));
    }

    #[test]
    fn a_run_of_one_watched_row_is_not_folded_and_an_opened_line_stays_open() {
        let rows = table(10, 4);
        assert_eq!(plan(&rows, None, 3, false), Plan { start: 0, end: 10, fold: 3 });
        assert_eq!(plan(&rows, None, 1, false).fold, 0);
        assert_eq!(plan(&rows, None, 3, true).fold, 0);
        let long = table(250, 230);
        let ranges = kaeri_model::episodes::episode_ranges(&long, 0);
        let at = kaeri_model::episodes::episode_range_to_open(&long) as usize;
        let fold = kaeri_model::episodes::episode_fold(&long, &ranges[at]);
        assert_eq!(
            plan(&long, Some(&ranges[at]), fold, false),
            Plan { start: 200, end: 250, fold: 29 }
        );
    }
}
