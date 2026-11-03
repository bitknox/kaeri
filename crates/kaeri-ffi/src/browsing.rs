//! What Search's extras and the episode list's Go to episode ask of the model: counts on cards,
//! the trending periods, and typing an episode number. See `kaeri_model::browsing`.
//!
//! Surprise me, Another, a period's rail and the dub ask are commands and events, not calls: see
//! `Command::Surprise`, `Command::LoadTrending`, `Event::Surprise`, `Event::Trending`,
//! `Event::DubsKnown` and `Event::SearchDubs`.

use kaeri_model::{
    browsing::{self, CountLine, DubKnown, GoTo, TrendPeriod},
    catalog::Entry,
    episodes::EpisodeRow,
};

/// The count line of a card, from its entry and the dub it is known to have: "12 episodes ·
/// Dub 8". For a card that learned of a dub after it was made: [`Event::DubsKnown`] brings the
/// line itself, and this says it again for an entry the app changed, such as one whose list
/// status moved. `None` when there is nothing to say.
///
/// [`Event::DubsKnown`]: kaeri_model::event::Event::DubsKnown
#[uniffi::export]
pub fn entry_count_line(entry: Entry, dub: Option<DubKnown>) -> Option<CountLine> {
    let mut entry = entry;
    entry.dub = dub;
    browsing::count_line(&entry)
}

/// The periods of Trending in the order of the menu: today, this week, this month.
#[uniffi::export]
pub fn trend_periods() -> Vec<TrendPeriod> {
    TrendPeriod::ALL.to_vec()
}

/// "Trending this week": a period's heading.
#[uniffi::export]
pub fn trend_period_label(period: TrendPeriod) -> String {
    period.label()
}

/// "This week": a period's choice in the menu.
#[uniffi::export]
pub fn trend_period_choice(period: TrendPeriod) -> String {
    period.choice()
}

/// How long a kept answer for `period` serves, in seconds.
#[uniffi::export]
pub fn trend_period_kept_for(period: TrendPeriod) -> i64 {
    period.kept_for()
}

/// What typing `typed` into Go to episode means for the table `rows` as of `now` (Unix seconds).
#[uniffi::export]
pub fn go_to_episode(rows: Vec<EpisodeRow>, typed: String, now: i64) -> GoTo {
    browsing::go_to_episode(&rows, &typed, now)
}

/// What Go to episode says of `result`: "No episode 537. This show has 412." or "Episode 410
/// isn't out yet. The latest is 402."; `None` for a row to go to and for nothing typed.
#[uniffi::export]
pub fn go_to_message(result: GoTo) -> Option<String> {
    browsing::go_to_message(&result)
}

/// The field's placeholder: "1 to 412".
#[uniffi::export]
pub fn go_to_hint(rows: Vec<EpisodeRow>) -> String {
    browsing::go_to_hint(&rows)
}

/// The field's accessibility label: "Episode, 1 to 412".
#[uniffi::export]
pub fn go_to_label(rows: Vec<EpisodeRow>) -> String {
    browsing::go_to_label(&rows)
}

/// The range of a long series' table that holds row `index`; `None` for a table of one range.
#[uniffi::export]
pub fn episode_range_of_row(rows: Vec<EpisodeRow>, index: u32) -> Option<u32> {
    browsing::episode_range_of_row(&rows, index)
}

/// The range that holds the episode named `number`, where the player's Episodes panel opens.
#[uniffi::export]
pub fn episode_range_of(rows: Vec<EpisodeRow>, number: String) -> Option<u32> {
    browsing::episode_range_of(&rows, &number)
}

/// The range after stepping `by` ranges from `current` of `count`, held to the first and the
/// last: Previous and Next range.
#[uniffi::export]
pub fn episode_range_step(count: u32, current: u32, by: i32) -> u32 {
    browsing::episode_range_step(count, current, by)
}
