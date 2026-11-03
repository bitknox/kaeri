//! A title's episodes, as its episode table and the player's Episodes menu list them.

/// One row of the timing-sheet episode table.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EpisodeRow {
    pub number: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title: Option<String>,
    /// Still frame for this episode, when one was published.
    ///
    /// Only ever drawn for the selected row: the table is dense, and one image that follows the
    /// cursor costs a single fetch per move rather than one per visible row.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub thumbnail: Option<String>,
    /// One-paragraph synopsis, when the source has one.
    ///
    /// Shown for the selected row only, under its still, for the same reason the still is:
    /// the table is a timing sheet, and a paragraph per row would drown it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub description: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub duration_secs: Option<u64>,
    /// How far through this episode the local history says we are.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub watched: f64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub completed: bool,
    /// Filler, where the source that listed the episode says so.
    ///
    /// `None` means unknown, which is the common case: most shows have no filler, and most sources
    /// say nothing either way. Rendered only when present, so such a show shows no column rather
    /// than a column of blanks.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub kind: Option<EpisodeKind>,
    /// Whether offering to skip this whole episode is reasonable.
    ///
    /// Only pure filler. `mixed` episodes contain filler *and* canon, so skipping one loses story —
    /// the distinction is the whole reason this is a separate field rather than `kind == "filler"`
    /// at every call site.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub skippable: bool,
    /// When the episode airs or aired, in Unix seconds, from AniList's schedule. Later than now
    /// means it has not aired yet. `None` when the schedule does not say, which is not the same
    /// as unaired: a long runner's oldest episodes, and titles AniList keeps no schedule for.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airs_at: Option<i64>,
    /// When the episode was to air before AniList's schedule moved it later, in Unix seconds:
    /// "Delayed (was 23:00)". Known only when this device saw the schedule before the move;
    /// `None` when it has not moved since it was first seen here.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub delayed_from: Option<i64>,
    /// Listed from AniList's own data, because no source is set up or none could find the
    /// show: it can be marked watched, but there is nothing to play or download it from, so
    /// a front end offers to add a source instead. A table is all of one kind or the other.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub metadata_only: bool,
    /// The name of the source that serves this episode, when it is not the one the table is
    /// from: the title's first source in the order had no such episode, and this one filled the
    /// gap. `None` for an episode from the table's own source, which is nearly all of them.
    /// A front end shows it quietly, where it already says an episode's source.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub source: Option<String>,
    /// AniList says the episode is out and no source set up lists it. It stays in the table so
    /// the season is whole and can be marked watched, and a play says no source has it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub unsourced: bool,
    /// The viewer has not reached this episode: it is past their progress (the tracker's, or the
    /// furthest episode they finished here) and not finished itself. What spoiler protection
    /// hides: the episode's summary, its still and any score for it. The setting, and a show's
    /// own, decide whether to hide; this says only which episodes are ahead. `false` for an
    /// episode with no number to compare.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub past_progress: bool,
    /// Spoiler protection, when it is on, hides this episode: it is not finished and comes after
    /// the title's next episode (the one Continue offers), which is shown, as is everything
    /// watched. This is the one rule every front end reads; none works it out for itself. A row
    /// not out yet can be ahead too: it has nothing to hide, and a front end draws it as the
    /// unaired row it is. Never set for an episode with no number to compare.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub spoiler_ahead: bool,
    /// This row has no synopsis yet, and another source's lookup is still running that may
    /// bring one: [`episode_text`] reads it as [`EpisodeText::Expected`], so a front end keeps
    /// room for the text where it will land. Never stored: a table read back from the cache
    /// has no lookup running.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub synopsis_pending: bool,
}

/// Whether a table has synopses to show, which decides at first layout how tall its rows are:
/// room for text only where text can land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EpisodeText {
    /// At least one row has a synopsis.
    Present,
    /// None yet, and a lookup that may bring some is still running.
    Expected,
    /// None, and none coming: every row is the short form.
    None,
}

/// What `rows` have of synopses: see [`EpisodeText`].
pub fn episode_text(rows: &[EpisodeRow]) -> EpisodeText {
    if rows.iter().any(|row| row.description.as_deref().is_some_and(|t| !t.trim().is_empty())) {
        EpisodeText::Present
    } else if rows.iter().any(|row| row.synopsis_pending) {
        EpisodeText::Expected
    } else {
        EpisodeText::None
    }
}

/// How many episodes one range of a long series holds.
pub const RANGE_SIZE: usize = 100;

/// One range of a long series' table: rows `start..end` of it (`end` exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EpisodeRange {
    pub start: u32,
    pub end: u32,
    /// Every row in the range is completed.
    pub watched: bool,
    /// Every row in the range is not out yet (a schedule date later than `now`).
    pub not_out: bool,
}

/// The ranges `rows` split into: none for a series of [`RANGE_SIZE`] episodes or fewer, else
/// consecutive ranges of 100 in table order. `now` is Unix seconds.
pub fn episode_ranges(rows: &[EpisodeRow], now: i64) -> Vec<EpisodeRange> {
    if rows.len() <= RANGE_SIZE {
        return Vec::new();
    }
    rows.chunks(RANGE_SIZE)
        .enumerate()
        .map(|(i, chunk)| EpisodeRange {
            start: (i * RANGE_SIZE) as u32,
            end: (i * RANGE_SIZE + chunk.len()) as u32,
            watched: chunk.iter().all(|row| row.completed),
            not_out: chunk.iter().all(|row| row.airs_at.is_some_and(|at| at > now)),
        })
        .collect()
}

/// The index of the range that opens first: the one holding the first episode not completed
/// (the next episode), else the last.
pub fn episode_range_to_open(rows: &[EpisodeRow]) -> u32 {
    let next =
        rows.iter().position(|row| !row.completed).unwrap_or(rows.len().saturating_sub(1));
    (next / RANGE_SIZE) as u32
}

/// How many of the leading rows of a range are watched, for the one line they fold into when
/// the range opens: the run of completed rows from `start`, never the whole range.
pub fn episode_fold(rows: &[EpisodeRow], range: &EpisodeRange) -> u32 {
    let slice = &rows[range.start as usize..range.end as usize];
    let run = slice.iter().take_while(|row| row.completed).count();
    if run == slice.len() { 0 } else { run as u32 }
}

/// How far a title's dub goes, as the source a dubbed play would use lists it: what
/// [`Event::TitleDub`] says, and what [`present::dub_line`] words for the title page.
///
/// No dub at all, and a source that could not say, are both no extent: `None` where it is
/// carried, and nothing on the page.
///
/// [`Event::TitleDub`]: crate::event::Event::TitleDub
/// [`present::dub_line`]: crate::present::dub_line
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct DubExtent {
    /// The last whole episode the source lists in the dub, in the numbering of the title's
    /// episode table: 6 for "Dub to episode 6".
    pub through: u32,
    /// The dub has every episode aired so far: all of a finished show, or an airing one's dub
    /// caught up with its broadcast. `false` when it stops short, and when how far the show
    /// has aired cannot be lined up with the source's numbers.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub complete: bool,
}

/// One episode's broadcast, from AniList's schedule: [`EpisodeRow::airs_at`] by number, and
/// what a play can be checked against before looking for an episode that is not out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EpisodeAiring {
    pub episode: u32,
    /// Unix seconds.
    pub airs_at: i64,
}

impl EpisodeAiring {
    /// Whether this broadcast is still to come at `now`.
    pub fn is_upcoming(&self, now: i64) -> bool {
        self.airs_at > now
    }
}

/// What an episode is to the story, by the filler index or the source's own claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EpisodeKind {
    /// Adapted from the source material.
    Canon,
    /// Canon with filler padding: labelled, never skipped, since skipping loses story.
    Mixed,
    /// Anime-original padding, the one kind worth offering to skip.
    Filler,
    /// Anime-original, but canon within the anime's own continuity.
    AnimeCanon,
}

impl EpisodeRow {
    /// Runtime as `mm:ss`, which is how a timing sheet reads.
    pub fn runtime(&self) -> String {
        match self.duration_secs {
            Some(secs) => format!("{}:{:02}", secs / 60, secs % 60),
            None => "--:--".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn text_is_present_expected_or_not_coming() {
        let mut table = rows(3);
        assert_eq!(episode_text(&table), EpisodeText::None);
        table[1].synopsis_pending = true;
        assert_eq!(episode_text(&table), EpisodeText::Expected);
        table[2].description = Some("A lamplighter walks on.".into());
        assert_eq!(episode_text(&table), EpisodeText::Present);
        table[2].description = Some("  ".into());
        assert_eq!(episode_text(&table), EpisodeText::Expected);
    }

    #[test]
    fn long_series_split_into_ranges_of_a_hundred() {
        assert!(episode_ranges(&rows(100), 0).is_empty());
        let mut table = rows(230);
        for row in table.iter_mut().take(120) {
            row.completed = true;
        }
        for row in table.iter_mut().skip(200) {
            row.airs_at = Some(10);
        }
        let ranges = episode_ranges(&table, 5);
        let spans: Vec<_> = ranges.iter().map(|r| (r.start, r.end)).collect();
        assert_eq!(spans, [(0, 100), (100, 200), (200, 230)]);
        assert_eq!(ranges.iter().map(|r| r.watched).collect::<Vec<_>>(), [true, false, false]);
        assert_eq!(ranges.iter().map(|r| r.not_out).collect::<Vec<_>>(), [false, false, true]);
        assert_eq!(episode_range_to_open(&table), 1);
        assert_eq!(episode_fold(&table, &ranges[1]), 20);
        assert_eq!(episode_fold(&table, &ranges[0]), 0, "a whole watched range folds nothing");
    }

    #[test]
    fn a_broadcast_is_upcoming_until_its_moment() {
        let airing = EpisodeAiring { episode: 6, airs_at: 1_000 };
        assert!(airing.is_upcoming(999));
        assert!(!airing.is_upcoming(1_000), "airing now is out");
        assert!(!airing.is_upcoming(5_000));
    }
}

/// Whether spoiler protection hides `row` when it is on ([`EpisodeRow::spoiler_ahead`]): the
/// episode is not finished and is numbered after `next`, the title's next episode. With 1 to 6
/// watched and 7 next, 8 and later are ahead, and 7 is not.
pub fn spoiler_ahead(row: &EpisodeRow, next: u32) -> bool {
    !row.completed && row.number.trim().parse::<u32>().is_ok_and(|n| n > next)
}
