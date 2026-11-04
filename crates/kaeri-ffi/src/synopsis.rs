//! What the episode list's layout asks of the model: whether a table has synopses, and how a
//! long series splits into ranges. See `kaeri_model::episodes`.

use kaeri_model::episodes::{self, EpisodeRange, EpisodeRow, EpisodeText};

/// Whether `rows` have synopses, expect some, or will not get any: what decides, at first
/// layout, whether rows are tall (room for text) or short.
#[uniffi::export]
pub fn episode_text(rows: Vec<EpisodeRow>) -> EpisodeText {
    episodes::episode_text(&rows)
}

/// The ranges of 100 a series of more than 100 episodes splits into; none for a shorter one.
/// `now` is Unix seconds.
#[uniffi::export]
pub fn episode_ranges(rows: Vec<EpisodeRow>, now: i64) -> Vec<EpisodeRange> {
    episodes::episode_ranges(&rows, now)
}

/// The index of the range to open first: the one holding the next episode.
#[uniffi::export]
pub fn episode_range_to_open(rows: Vec<EpisodeRow>) -> u32 {
    episodes::episode_range_to_open(&rows)
}

/// How many leading rows of `range` are watched and fold into one line; zero when the whole
/// range is watched.
#[uniffi::export]
pub fn episode_fold(rows: Vec<EpisodeRow>, range: EpisodeRange) -> u32 {
    episodes::episode_fold(&rows, &range)
}
