//! What the player's Episodes and Sources panel draws from besides the engine's menus: a switch
//! to another source as the engine says it goes, what a revealed episode row offers, and the
//! words its rows say. Pure, so it is tested without a display. After the Apple and Android
//! panels' own models.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use kaeri_core::{media::Translation, traits::SourceCandidate};
use kaeri_i18n::t;
use kaeri_model::{
    Msg,
    episodes::{EpisodeKind, EpisodeRow},
    playback::SwitchTo,
    starting::{ListingMiss, ListingState, SourceHold, SourceListing},
};

/// The panel's two tabs. It opens on Episodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelTab {
    #[default]
    Episodes,
    Sources,
}

impl PanelTab {
    /// The toggle's name in the tab switcher.
    pub fn name(self) -> &'static str {
        match self {
            Self::Episodes => "episodes",
            Self::Sources => "sources",
        }
    }

    pub fn from_name(name: &str) -> Self {
        if name == "sources" { Self::Sources } else { Self::Episodes }
    }
}

/// What a switch of the playing episode goes to, by the ids the panel's rows carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchTarget {
    /// The source, as `SourceCandidate::provider_id` names it; empty for a stream of the source
    /// playing.
    pub provider_id: String,
    /// The name the source goes by.
    pub name: String,
    /// The release, by its `SourceCandidate::id`; `None` for a stream.
    pub release: Option<String>,
    /// The stream, by its `MenuSource::id`; `None` for a release.
    pub stream: Option<String>,
}

impl SwitchTarget {
    pub fn release(candidate: &SourceCandidate) -> Self {
        Self {
            provider_id: candidate.provider_id.clone(),
            name: source_name(candidate),
            release: Some(candidate.id.clone()),
            stream: None,
        }
    }

    pub fn stream(id: &str, name: &str) -> Self {
        Self {
            provider_id: String::new(),
            name: name.to_owned(),
            release: None,
            stream: Some(id.to_owned()),
        }
    }

    /// The same row: a stream by its id, a release by its source and id.
    pub fn same(&self, other: &Self) -> bool {
        match &self.stream {
            Some(stream) => other.stream.as_ref() == Some(stream),
            None => {
                other.stream.is_none()
                    && self.release == other.release
                    && self.provider_id == other.provider_id
            }
        }
    }

    /// The key of the row this target is on, as the panel keeps its rows.
    pub fn key(&self) -> String {
        match (&self.stream, &self.release) {
            (Some(stream), _) => format!("stream/{stream}"), // i18n-skip
            (None, Some(release)) => format!("release/{}/{release}", self.provider_id), // i18n-skip
            (None, None) => format!("source/{}", self.provider_id), // i18n-skip
        }
    }
}

impl From<&SwitchTo> for SwitchTarget {
    fn from(to: &SwitchTo) -> Self {
        Self {
            provider_id: to.provider_id.clone(),
            name: to.name.clone(),
            release: to.release.clone(),
            stream: to.stream.clone(),
        }
    }
}

/// The name a release's source goes by, or its id while the engine has not named it.
pub fn source_name(candidate: &SourceCandidate) -> String {
    if candidate.provider_name.is_empty() {
        candidate.provider_id.clone()
    } else {
        candidate.provider_name.clone()
    }
}

/// How far a switch picked in the panel has got.
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    /// Sent, and the engine has said nothing yet.
    Asked,
    /// The release is being found; the episode plays on.
    Finding,
    /// Found and loading at the playhead; the picture is held.
    Loading,
    /// It did not start, and the episode plays on from what played before: the engine's line.
    Failed(Msg),
}

/// Waiting is silent this long: most switches land inside it.
pub const SILENT: Duration = Duration::from_millis(600);
/// "Starting · 3 s" from here.
pub const COUNTING: Duration = Duration::from_secs(3);

/// A switch picked in the panel, from the click until it lands or fails. The row it was picked
/// on shows it: nothing for 600 ms, then a spinner, then "Starting · 3 s"; a failure stays on
/// the row until the next pick, with Try again.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelSwitch {
    pub target: SwitchTarget,
    pub phase: Phase,
    /// When it was picked: what the row's wait counts from.
    pub since: Instant,
    /// The release a switch was picked from, for Try again.
    pub candidate: Option<SourceCandidate>,
}

impl PanelSwitch {
    pub fn asked(
        target: SwitchTarget,
        candidate: Option<SourceCandidate>,
        now: Instant,
    ) -> Self {
        Self { target, phase: Phase::Asked, since: now, candidate }
    }

    /// Whether `target` is the row this switch was picked on.
    pub fn on(&self, target: &SwitchTarget) -> bool {
        self.target.same(target)
    }

    pub fn failed(&self) -> Option<&Msg> {
        match &self.phase {
            Phase::Failed(text) => Some(text),
            _ => None,
        }
    }

    /// Whether the spinner shows at `now`: past the silent 600 ms, and not failed.
    pub fn spinning(&self, now: Instant) -> bool {
        self.failed().is_none() && now.saturating_duration_since(self.since) >= SILENT
    }

    /// The whole seconds waited at `now`, once "Starting · 3 s" takes the detail line's place.
    pub fn counting(&self, now: Instant) -> Option<u64> {
        let waited = now.saturating_duration_since(self.since);
        (self.failed().is_none() && waited >= COUNTING).then_some(waited.as_secs())
    }

    /// The switch moves on to `phase`. A switch the panel did not pick (a stream from the More
    /// menu) becomes a switch of its own.
    pub fn track(
        known: Option<Self>,
        target: SwitchTarget,
        phase: Phase,
        now: Instant,
    ) -> Self {
        match known {
            Some(known) if known.on(&target) => Self { phase, ..known },
            _ => Self { target, phase, since: now, candidate: None },
        }
    }
}

/// A switch to the other version, subtitled or dubbed, from the pick until it lands, or its
/// failure until the next pick. The picked half of the switch shows it: nothing for 600 ms, then
/// a spinner, then "Finding episode 7 dubbed · 3 s" under the switch.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionSwitch {
    pub to: Translation,
    pub phase: Phase,
    /// When it was picked: what the wait counts from.
    pub since: Instant,
}

impl VersionSwitch {
    pub fn asked(to: Translation, now: Instant) -> Self {
        Self { to, phase: Phase::Asked, since: now }
    }

    pub fn failed(&self) -> Option<&Msg> {
        match &self.phase {
            Phase::Failed(text) => Some(text),
            _ => None,
        }
    }

    /// Whether the switch's spinner shows at `now`: past the silent 600 ms, and not failed.
    pub fn spinning(&self, now: Instant) -> bool {
        self.failed().is_none() && now.saturating_duration_since(self.since) >= SILENT
    }

    /// "Finding episode 7 dubbed · 3 s" at `now`, from 3 s.
    pub fn note(&self, episode: &str, now: Instant) -> Option<String> {
        let waited = now.saturating_duration_since(self.since);
        if self.failed().is_some() || waited < COUNTING {
            return None;
        }
        let seconds = waited.as_secs() as i64;
        Some(match self.to {
            Translation::Dub => t::player_finding_version_seconds(episode, seconds),
            Translation::Sub => t::player_finding_version_subbed_seconds(episode, seconds),
        })
    }

    /// The switch moves on to `phase`; one picked elsewhere (a remote) is tracked from now.
    pub fn track(known: Option<Self>, to: Translation, phase: Phase, now: Instant) -> Self {
        match known {
            Some(known) if known.to == to => Self { phase, ..known },
            _ => Self { to, phase, since: now },
        }
    }
}

/// Whether the release playing still plays once the engine says a switch landed on `to`: a
/// release names itself, and a stream of the release's own source keeps it. The other version,
/// or a stream of another source the engine moved to, lets it go.
pub fn keeps_release(to: &SwitchTo, playing: Option<&SourceCandidate>) -> bool {
    to.translation.is_none()
        && to.stream.is_some()
        && playing.is_some_and(|release| release.provider_id == to.provider_id)
}

/// A revealed row's buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeAction {
    Play,
    Resume,
    /// Another episode, from its beginning.
    FromBeginning,
    /// The episode playing, back to its start.
    FromBeginningPlaying,
    MarkWatched,
}

impl EpisodeAction {
    pub fn title(self) -> String {
        match self {
            Self::Play => t::action_play(),
            Self::Resume => t::action_resume(),
            Self::FromBeginning | Self::FromBeginningPlaying => {
                t::episode_play_from_beginning()
            }
            Self::MarkWatched => t::episode_mark_watched(),
        }
    }

    /// The amber one, which Return and a double-click take.
    pub fn primary(self) -> bool {
        matches!(self, Self::Play | Self::Resume)
    }

    /// Whether it starts something, and so closes the panel.
    pub fn plays(self) -> bool {
        !matches!(self, Self::MarkWatched)
    }
}

/// What a revealed row can do, in order, as the board has it.
pub fn actions(
    row: &EpisodeRow,
    current: bool,
    metadata_only: bool,
    not_out: bool,
) -> Vec<EpisodeAction> {
    use EpisodeAction::*;
    if metadata_only || (row.unsourced && !not_out) {
        return if row.completed { vec![] } else { vec![MarkWatched] };
    }
    if not_out {
        return vec![];
    }
    if current {
        vec![FromBeginningPlaying]
    } else if row.watched > 0.0 && !row.completed {
        vec![Resume, FromBeginning]
    } else if row.completed {
        vec![Play]
    } else {
        vec![Play, MarkWatched]
    }
}

/// Whether the episode has not aired yet at `now`, in Unix seconds.
pub fn not_out(row: &EpisodeRow, now: i64) -> bool {
    row.airs_at.is_some_and(|at| at > now)
}

/// "E7" before a titled row's title, or the number as it is when it is not whole.
pub fn number(number: &str) -> String {
    number.parse::<i64>().map_or_else(|_| number.to_owned(), t::episode_number_short)
}

/// The row's title: its own, or "Episode 11" while it has none.
pub fn title(row: &EpisodeRow) -> String {
    match row.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(title) => title.to_owned(),
        None => {
            row.number.parse::<i64>().map_or_else(|_| row.number.clone(), t::episode_number)
        }
    }
}

/// Whole minutes, never under one.
fn minutes(seconds: f64) -> i64 {
    (seconds / 60.0).round().max(1.0) as i64
}

/// What the panel knows of the playhead, for the playing row.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Playhead {
    pub position: f64,
    pub duration: Option<f64>,
}

/// How the detail line writes a time: when an episode airs, and the unsourced line's date.
pub struct Clock<'a> {
    pub now: i64,
    pub airs: &'a dyn Fn(i64) -> String,
    pub date: &'a dyn Fn(i64) -> String,
}

/// The row's one detail line: "Playing · 11 min left" on the playing row, else the kind, then
/// when it airs, unsourced, watched, the time left or the length, and the source that fills it
/// in.
pub fn detail(
    row: &EpisodeRow,
    current: bool,
    metadata_only: bool,
    playhead: Playhead,
    clock: &Clock,
) -> String {
    if current {
        let length = playhead.duration.or(row.duration_secs.map(|s| s as f64)).unwrap_or(0.0);
        return t::episode_row_playing(&t::time_minutes_left(minutes(
            length - playhead.position,
        )));
    }
    let mut parts = Vec::new();
    match row.kind {
        Some(EpisodeKind::Filler) => parts.push(t::episode_filler()),
        Some(EpisodeKind::Mixed) => parts.push(t::episode_row_mixed()),
        _ => {}
    }
    if let Some(at) = row.airs_at.filter(|at| *at > clock.now) {
        parts.push(t::episode_row_airs(&(clock.airs)(at)));
        if let Some(was) = row.delayed_from {
            parts.push(t::episode_row_delayed(&(clock.airs)(was)));
        }
        return parts.join(" · "); // i18n-skip
    }
    if row.unsourced && !metadata_only {
        parts.push(crate::missing::row_line(row, clock.now, clock.date));
        return parts.join(" · "); // i18n-skip
    }
    let seconds = row.duration_secs.map(|s| s as f64);
    if row.completed {
        parts.push(t::episode_watched());
    } else if let Some(seconds) = seconds.filter(|_| row.watched > 0.0 && !metadata_only) {
        parts.push(t::time_minutes_left(minutes(seconds * (1.0 - row.watched))));
    } else if let Some(seconds) = seconds.filter(|s| *s > 0.0) {
        parts.push(t::episode_row_length(minutes(seconds)));
    }
    if !metadata_only && let Some(source) = &row.source {
        parts.push(t::episode_row_from(source));
    }
    parts.join(" · ") // i18n-skip
}

/// The silk line on the still: the playhead on the playing row, the history's on a started one.
pub fn progress(
    row: &EpisodeRow,
    current: bool,
    metadata_only: bool,
    playhead: Playhead,
) -> Option<f64> {
    if metadata_only {
        return None;
    }
    if current {
        let length = playhead.duration.or(row.duration_secs.map(|s| s as f64))?;
        return (length > 0.0).then(|| (playhead.position / length).clamp(0.0, 1.0));
    }
    (!row.completed && row.watched > 0.0).then(|| row.watched.clamp(0.0, 1.0))
}

/// "1080p · 1.4 GiB · night-index · Dubbed · Automatic picks this", from the release's own
/// fields only.
pub fn release_detail(candidate: &SourceCandidate) -> String {
    let mut parts = Vec::new();
    if let Some(quality) = candidate.quality {
        parts.push(format!("{quality}p")); // i18n-skip
    }
    if let Some(size) = candidate.size.as_deref().filter(|s| !s.is_empty()) {
        parts.push(size.to_owned());
    }
    if let Some(label) = candidate.label.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        parts.push(label.to_owned());
    }
    if candidate.dubbed {
        parts.push(t::audio_dubbed());
    } else if candidate.dual_audio {
        parts.push(t::player_panel_release_dual_audio());
    }
    if candidate.auto_pick {
        parts.push(t::player_release_automatic());
    }
    parts.join(" · ") // i18n-skip
}

/// The calendar day `days` after the day of `now`, both as (year, day of year) pairs: how many
/// days on `at` is from `now`, for the air line's today, tomorrow and weekday.
pub fn days_between(now: (i32, i32), at: (i32, i32), days_in_now_year: i32) -> i32 {
    if now.0 == at.0 { at.1 - now.1 } else { days_in_now_year - now.1 + at.1 }
}

/// The Sources tab while its sources answer: when it asked, the last rows the engine said, and
/// when each row first said Asking, which its seconds count from. Drawn from the last rows,
/// never merged.
#[derive(Debug, Clone)]
pub struct Listing {
    pub asked: Instant,
    pub rows: Option<Vec<SourceListing>>,
    asking_since: HashMap<String, Instant>,
}

/// What a source's detail line says while the tab loads, apart from its words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingLine {
    /// Held empty, one line high: the silent 600 ms.
    Held,
    /// "Asking".
    Asking,
    /// "Asking · 4 s", from 3 s.
    Counting(u64),
    /// What the source answered: its coverage line, the miss, or Switched off.
    Answered,
}

impl Listing {
    pub fn asked(now: Instant) -> Self {
        Self { asked: now, rows: None, asking_since: HashMap::new() }
    }

    /// The engine said every row again.
    pub fn took(&mut self, rows: Vec<SourceListing>, now: Instant) {
        for row in &rows {
            if row.state == ListingState::Asking {
                self.asking_since.entry(row.provider_id.clone()).or_insert(now);
            }
        }
        self.rows = Some(rows);
    }

    /// Whether a source is still asked, or the engine has not said the rows yet.
    pub fn busy(&self) -> bool {
        self.rows
            .as_ref()
            .is_none_or(|rows| rows.iter().any(|r| r.state == ListingState::Asking))
    }

    /// The line of the source `provider_id` in `state` at `now`, and whether its spinner shows.
    pub fn line(
        &self,
        provider_id: &str,
        state: Option<&ListingState>,
        now: Instant,
    ) -> (ListingLine, bool) {
        match state {
            None => (ListingLine::Held, false),
            Some(ListingState::Asking) => {
                if now.saturating_duration_since(self.asked) < SILENT {
                    return (ListingLine::Held, false);
                }
                let since = self.asking_since.get(provider_id).copied().unwrap_or(self.asked);
                let waited = now.saturating_duration_since(since);
                if waited >= COUNTING {
                    (ListingLine::Counting(waited.as_secs()), true)
                } else {
                    (ListingLine::Asking, true)
                }
            }
            Some(_) => (ListingLine::Answered, false),
        }
    }

    /// Move every time back by `by`, as if the tab had asked that much earlier.
    #[cfg(test)]
    pub fn age(&mut self, by: Duration) {
        self.asked -= by;
        for since in self.asking_since.values_mut() {
            *since -= by;
        }
    }
}

/// The words of a source's answer: `coverage`, its line from the title's sources, when it
/// listed releases, else the one release's own; the starting screen's row words for a miss;
/// Switched off.
pub fn listing_words(state: &ListingState, coverage: Option<String>, episode: &str) -> String {
    match state {
        ListingState::Asking => String::new(),
        ListingState::Listed { releases } => coverage
            .or_else(|| match releases.as_slice() {
                [one] => Some(release_detail(one)),
                _ => None,
            })
            .unwrap_or_default(),
        ListingState::Missed { miss } => miss_words(*miss, episode),
        ListingState::Off => t::source_note_off(),
    }
}

/// The starting screen's row words for a source that listed nothing.
pub fn miss_words(miss: ListingMiss, episode: &str) -> String {
    match miss {
        ListingMiss::NoMatch => t::player_start_row_no_match(),
        ListingMiss::Ambiguous => t::source_note_ambiguous(),
        ListingMiss::NoEpisode => t::player_start_row_no_episode(episode),
        ListingMiss::Failed => t::player_start_row_failed(),
        ListingMiss::TimedOut => t::player_start_row_timed_out(),
        ListingMiss::HeldBack { why } => match why {
            SourceHold::TorrentsOff => t::player_start_row_held_torrents_off(),
            SourceHold::TorrentsStarting => t::player_start_row_held_torrents_starting(),
            SourceHold::TorrentsFailed => t::player_start_row_held_torrents_failed(),
            SourceHold::ProxyCheck => t::player_start_row_held_proxy(),
            SourceHold::Cellular => t::player_start_row_held_cellular(),
            SourceHold::Other => t::player_start_row_held_other(),
        },
    }
}

/// The releases a source listed, once it has.
pub fn listed(state: Option<&ListingState>) -> &[SourceCandidate] {
    match state {
        Some(ListingState::Listed { releases }) => releases,
        _ => &[],
    }
}

/// Whether the row's title is in haze: the source answered with nothing to pick, or is off.
pub fn listing_dim(state: Option<&ListingState>) -> bool {
    matches!(state, Some(ListingState::Missed { .. } | ListingState::Off))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(number: &str) -> EpisodeRow {
        EpisodeRow {
            number: number.into(),
            title: Some("The lamp seller".into()),
            duration_secs: Some(1420),
            ..EpisodeRow::default()
        }
    }

    fn clock(now: i64) -> (i64, impl Fn(i64) -> String, impl Fn(i64) -> String) {
        (now, |at: i64| format!("at {at}"), |at: i64| format!("day {at}"))
    }

    fn say(
        row: &EpisodeRow,
        current: bool,
        meta: bool,
        playhead: Playhead,
        now: i64,
    ) -> String {
        let (now, airs, date) = clock(now);
        detail(row, current, meta, playhead, &Clock { now, airs: &airs, date: &date })
    }

    fn candidate(id: &str) -> SourceCandidate {
        SourceCandidate {
            id: id.into(),
            provider_id: "index".into(),
            provider_name: "Example index".into(),
            title: "The Lamp Road 07 1080p".into(),
            quality: Some(1080),
            label: Some("night-index".into()),
            size: Some("1.4 GiB".into()),
            dual_audio: false,
            dubbed: false,
            auto_pick: false,
        }
    }

    #[test]
    fn the_other_version_is_silent_then_spins_then_counts() {
        let start = Instant::now();
        let version = VersionSwitch::asked(Translation::Dub, start);
        assert!(!version.spinning(start + Duration::from_millis(599)));
        assert!(version.spinning(start + SILENT));
        assert_eq!(version.note("7", start + Duration::from_millis(2999)), None);
        assert_eq!(
            version.note("7", start + Duration::from_millis(4200)),
            Some(t::player_finding_version_seconds("7", 4))
        );
        let sub = VersionSwitch::asked(Translation::Sub, start);
        assert_eq!(
            sub.note("7", start + COUNTING),
            Some(t::player_finding_version_subbed_seconds("7", 3))
        );
    }

    #[test]
    fn the_other_version_failed_neither_spins_nor_counts_and_keeps_its_wait() {
        let start = Instant::now();
        let asked = VersionSwitch::asked(Translation::Dub, start);
        let later = start + Duration::from_secs(5);
        let finding =
            VersionSwitch::track(Some(asked), Translation::Dub, Phase::Finding, later);
        assert_eq!(finding.since, start, "the wait counts from the pick");
        let failed = VersionSwitch::track(
            Some(finding),
            Translation::Dub,
            Phase::Failed(Msg::PlayerPanelDubFailed { episode: "7".into() }),
            later,
        );
        assert!(!failed.spinning(later));
        assert_eq!(failed.note("7", later), None);
        let other = VersionSwitch::track(Some(failed), Translation::Sub, Phase::Finding, later);
        assert_eq!(other.since, later, "one picked elsewhere counts from now");
    }

    #[test]
    fn a_landing_elsewhere_lets_the_release_go() {
        let release = candidate("r1");
        let stream = |provider: &str| SwitchTo {
            provider_id: provider.into(),
            name: "Example index".into(),
            stream: Some("2".into()),
            ..SwitchTo::default()
        };
        assert!(keeps_release(&stream("index"), Some(&release)), "another stream of it");
        assert!(!keeps_release(&stream("harbor"), Some(&release)), "another source took over");
        let version = SwitchTo { translation: Some(Translation::Dub), ..stream("index") };
        assert!(!keeps_release(&version, Some(&release)), "the other version");
        let source = SwitchTo { stream: None, ..stream("index") };
        assert!(!keeps_release(&source, Some(&release)), "the source's own pick");
    }

    #[test]
    fn a_switch_is_silent_then_spins_then_counts() {
        let start = Instant::now();
        let target = SwitchTarget::release(&candidate("r1"));
        let switch = PanelSwitch::asked(target, None, start);
        assert!(!switch.spinning(start + Duration::from_millis(599)));
        assert!(switch.spinning(start + SILENT));
        assert_eq!(switch.counting(start + Duration::from_millis(2999)), None);
        assert_eq!(switch.counting(start + Duration::from_millis(4200)), Some(4));
    }

    #[test]
    fn a_failed_switch_neither_spins_nor_counts() {
        let start = Instant::now();
        let target = SwitchTarget::release(&candidate("r1"));
        let switch = PanelSwitch::track(
            Some(PanelSwitch::asked(target.clone(), Some(candidate("r1")), start)),
            target,
            Phase::Failed(Msg::PlayerPanelSwitchFailed {
                source: "Example index".into(),
                current: "Example source".into(),
            }),
            start + Duration::from_secs(5),
        );
        assert!(!switch.spinning(start + Duration::from_secs(6)));
        assert_eq!(switch.counting(start + Duration::from_secs(6)), None);
        // The pick it came from stays, for Try again, and so does its clock.
        assert_eq!(switch.candidate, Some(candidate("r1")));
        assert_eq!(switch.since, start);
    }

    #[test]
    fn a_switch_picked_elsewhere_gets_a_row_of_its_own() {
        let start = Instant::now();
        let picked = PanelSwitch::asked(SwitchTarget::release(&candidate("r1")), None, start);
        let later = start + Duration::from_secs(1);
        let other = SwitchTarget::stream("s3", "Example source");
        let switch = PanelSwitch::track(Some(picked), other.clone(), Phase::Finding, later);
        assert_eq!(switch.target, other);
        assert_eq!(switch.since, later);
    }

    #[test]
    fn targets_match_a_stream_by_id_and_a_release_by_source_and_id() {
        let release = SwitchTarget::release(&candidate("r1"));
        let mut elsewhere = release.clone();
        elsewhere.provider_id = "home".into();
        assert!(release.same(&release.clone()));
        assert!(!release.same(&elsewhere));
        let stream = SwitchTarget::stream("s2", "Example source");
        let mut renamed = stream.clone();
        renamed.name = "Other name".into();
        assert!(stream.same(&renamed));
        assert!(!stream.same(&release));
        assert_eq!(release.key(), "release/index/r1");
        assert_eq!(stream.key(), "stream/s2");
    }

    #[test]
    fn a_revealed_row_offers_what_the_board_says() {
        use EpisodeAction::*;
        let fresh = row("8");
        assert_eq!(actions(&fresh, false, false, false), vec![Play, MarkWatched]);
        assert_eq!(actions(&fresh, true, false, false), vec![FromBeginningPlaying]);
        let started = EpisodeRow { watched: 0.42, ..row("3") };
        assert_eq!(actions(&started, false, false, false), vec![Resume, FromBeginning]);
        let done = EpisodeRow { completed: true, watched: 1.0, ..row("2") };
        assert_eq!(actions(&done, false, false, false), vec![Play]);
        assert_eq!(actions(&fresh, false, false, true), vec![]);
        let unsourced = EpisodeRow { unsourced: true, ..row("9") };
        assert_eq!(actions(&unsourced, false, false, false), vec![MarkWatched]);
        assert_eq!(actions(&fresh, false, true, false), vec![MarkWatched]);
        assert_eq!(actions(&done, false, true, false), vec![]);
        assert!(Play.primary() && Resume.primary() && !MarkWatched.primary());
    }

    #[test]
    fn the_detail_line_says_the_row_as_it_is() {
        let playhead = Playhead { position: 724.0, duration: Some(1420.0) };
        assert_eq!(say(&row("7"), true, false, playhead, 0), "Playing · 12 min left");
        assert_eq!(say(&row("8"), false, false, playhead, 0), "24 min");
        let started = EpisodeRow { watched: 0.42, ..row("3") };
        assert_eq!(say(&started, false, false, playhead, 0), "14 min left");
        let done = EpisodeRow { completed: true, kind: Some(EpisodeKind::Filler), ..row("5") };
        assert_eq!(say(&done, false, false, playhead, 0), "Filler · Watched");
        let filled = EpisodeRow { source: Some("Home server".into()), ..row("9") };
        assert_eq!(say(&filled, false, false, playhead, 0), "24 min · From Home server");
        assert_eq!(say(&filled, false, true, playhead, 0), "24 min");
        let coming = EpisodeRow {
            airs_at: Some(500),
            delayed_from: Some(400),
            duration_secs: None,
            kind: Some(EpisodeKind::Mixed),
            ..row("11")
        };
        assert_eq!(
            say(&coming, false, false, playhead, 100),
            "Mixed · Airs at 500 · Delayed, was at 400"
        );
    }

    #[test]
    fn the_still_shows_the_playhead_or_the_history() {
        let playhead = Playhead { position: 710.0, duration: Some(1420.0) };
        assert_eq!(progress(&row("7"), true, false, playhead), Some(0.5));
        let started = EpisodeRow { watched: 0.42, ..row("3") };
        assert_eq!(progress(&started, false, false, playhead), Some(0.42));
        assert_eq!(progress(&row("8"), false, false, playhead), None);
        assert_eq!(progress(&started, false, true, playhead), None);
    }

    #[test]
    fn a_release_says_its_own_fields_only() {
        let mut release = candidate("r1");
        assert_eq!(release_detail(&release), "1080p · 1.4 GiB · night-index");
        release.dual_audio = true;
        release.auto_pick = true;
        release.label = None;
        assert_eq!(
            release_detail(&release),
            "1080p · 1.4 GiB · Subbed and dubbed · Automatic picks this"
        );
        release.dubbed = true;
        assert_eq!(release_detail(&release), "1080p · 1.4 GiB · Dubbed · Automatic picks this");
    }

    #[test]
    fn titles_and_numbers_fall_back_to_the_number() {
        assert_eq!(number("7"), "E7");
        assert_eq!(number("7.5"), "7.5");
        let untitled = EpisodeRow { title: None, ..row("11") };
        assert_eq!(title(&untitled), "Episode 11");
        assert_eq!(title(&row("7")), "The lamp seller");
    }

    fn listing(id: &str, state: ListingState) -> SourceListing {
        SourceListing { provider_id: id.into(), name: format!("{id} name"), state }
    }

    #[test]
    fn a_source_asked_is_silent_then_asks_then_counts_from_its_first_asking() {
        let start = Instant::now();
        let mut loading = Listing::asked(start);
        assert!(loading.busy(), "nothing said yet");
        assert_eq!(loading.line("home", None, start), (ListingLine::Held, false));
        loading.took(vec![listing("home", ListingState::Asking)], start);
        let asking = Some(&ListingState::Asking);
        let at = |ms: u64| start + Duration::from_millis(ms);
        assert_eq!(loading.line("home", asking, at(599)), (ListingLine::Held, false));
        assert_eq!(loading.line("home", asking, at(600)), (ListingLine::Asking, true));
        assert_eq!(loading.line("home", asking, at(2999)), (ListingLine::Asking, true));
        assert_eq!(loading.line("home", asking, at(4200)), (ListingLine::Counting(4), true));
        // A row said again later keeps the time it first said Asking.
        loading.took(vec![listing("home", ListingState::Asking)], at(2000));
        assert_eq!(loading.line("home", asking, at(3100)), (ListingLine::Counting(3), true));
        let off = Some(&ListingState::Off);
        assert_eq!(loading.line("home", off, at(100)), (ListingLine::Answered, false));
        loading.took(vec![listing("home", ListingState::Off)], at(3200));
        assert!(!loading.busy());
    }

    #[test]
    fn a_source_answers_in_the_starting_screens_words() {
        let one = ListingState::Listed { releases: vec![candidate("r1")] };
        assert_eq!(
            listing_words(&one, None, "7"),
            "1080p · 1.4 GiB · night-index",
            "one release, before the title's sources are known"
        );
        assert_eq!(listing_words(&one, Some("Episodes 1 to 9".into()), "7"), "Episodes 1 to 9");
        let missed = |miss| ListingState::Missed { miss };
        assert_eq!(listing_words(&missed(ListingMiss::Failed), None, "7"), "Didn't answer");
        assert_eq!(listing_words(&missed(ListingMiss::TimedOut), None, "7"), "Took too long");
        assert_eq!(
            listing_words(&missed(ListingMiss::Ambiguous), None, "7"),
            "Couldn't match this title"
        );
        assert_eq!(
            listing_words(&missed(ListingMiss::NoEpisode), None, "7"),
            t::player_start_row_no_episode("7")
        );
        assert_eq!(
            listing_words(
                &missed(ListingMiss::HeldBack { why: SourceHold::Cellular }),
                None,
                "7"
            ),
            "Not asked · Torrents are off on cellular"
        );
        assert_eq!(listing_words(&ListingState::Off, None, "7"), "Switched off");
        assert!(listing_dim(Some(&ListingState::Off)));
        assert!(listing_dim(Some(&missed(ListingMiss::Failed))));
        assert!(!listing_dim(Some(&one)) && !listing_dim(Some(&ListingState::Asking)));
        assert_eq!(listed(Some(&one)).len(), 1);
    }

    #[test]
    fn days_between_crosses_the_new_year() {
        assert_eq!(days_between((2026, 300), (2026, 302), 365), 2);
        assert_eq!(days_between((2026, 364), (2027, 1), 365), 2);
    }
}
