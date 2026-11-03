//! What the starting screen says and offers, from the engine's steps and the clock: no widgets,
//! so every rule of the design is tested here. The view (`start_view.rs`) draws a [`Frame`].
//!
//! Times are `Duration`s since the screen opened, passed in by the caller.

use std::time::Duration;

use kaeri_core::{ids::AnilistId, media::Translation};
use kaeri_i18n::t;
use kaeri_model::{
    Msg,
    party::NoSourceWhy,
    starting::{SourceMiss, StartFailure, StartStep},
};

use crate::say;

/// The shortest a step line's text stays before a newer one takes its place.
pub const HOLD: Duration = Duration::from_millis(600);
/// When the detail line appears, and Play from the beginning.
pub const DETAIL_AT: Duration = Duration::from_secs(1);
/// When the seconds join the detail line, and Choose source and Details join the row.
pub const TOOLS_AT: Duration = Duration::from_secs(3);
/// How long one source or stream is tried before Skip is offered for it.
pub const SKIP_AFTER: Duration = Duration::from_secs(8);
/// When the step line says plainly that it is still waiting.
pub const SLOW_AT: Duration = Duration::from_secs(20);

const JOIN: &str = " · "; // i18n-skip

/// What a button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Cancel,
    FromBeginning,
    ChooseSource,
    Details,
    Skip,
    ShowDownloads,
    ChooseAnother,
    TryAgain,
    FixMatch,
    AddStore,
    Close,
}

/// How a button is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Flat: Cancel, Details, Close.
    Quiet,
    /// The tonal pill.
    Tonal,
    /// The amber of a failure's primary.
    Primary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub action: Action,
    pub label: String,
    pub look: Look,
}

/// The mark in front of a Details row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Done,
    Empty,
    Now,
    Rest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub mark: Mark,
    pub name: String,
    pub note: String,
    pub time: String,
    /// Whether the row carries a Skip.
    pub skip: bool,
}

/// What the screen shows now.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// The step line, and which step it says: a different key crossfades, the same key (a
    /// percentage) changes in place.
    pub step: String,
    pub key: &'static str,
    /// The detail line's facts and, from 3 s, the seconds; empty before 1 s.
    pub lead: String,
    pub seconds: Option<i64>,
    /// The buttons now, in arrival order.
    pub buttons: Vec<Button>,
    /// The failure's title and reason, when the start has failed.
    pub failure: Option<(String, String)>,
    /// A buffering percentage, when mpv gives one.
    pub percent: Option<u32>,
    pub rows: Vec<Row>,
    /// The translation the Details switch shows, when it is known.
    pub translation: Option<Translation>,
    pub details_open: bool,
}

impl Frame {
    /// The detail line as one string.
    pub fn detail(&self) -> String {
        match (self.lead.is_empty(), self.seconds) {
            (true, None) => String::new(),
            (true, Some(s)) => t::linux_start_seconds(s),
            (false, None) => self.lead.clone(),
            (false, Some(s)) => format!("{}{JOIN}{}", self.lead, t::linux_start_seconds(s)),
        }
    }
}

#[derive(Debug, Clone)]
struct Asking {
    name: String,
    place: u32,
    total: u32,
    pinned: bool,
}

#[derive(Debug, Clone)]
struct Seen {
    name: String,
    miss: Option<SourceMiss>,
    took: Option<Duration>,
}

/// The step line's text, held.
#[derive(Debug, Clone)]
struct Held {
    text: String,
    key: &'static str,
    since: Duration,
}

/// One start, from the play being staged to its first frame or its end.
pub struct Start {
    pub id: AnilistId,
    pub episode: String,
    resume: Option<f64>,
    from_beginning: bool,
    shown: Held,
    waiting: Option<(String, &'static str)>,
    /// Since when the rung (the source being asked, the stream being opened) has been tried.
    rung_at: Duration,
    looked_up_at: Option<Duration>,
    asking: Option<Asking>,
    /// What came of the sources asked, in order, and the ones resting.
    seen: Vec<Seen>,
    resting: Vec<String>,
    last_miss: Option<(String, SourceMiss)>,
    peers: Option<u32>,
    rate: Option<u64>,
    finding_peers: Option<Option<String>>,
    found: Option<(Option<String>, Translation)>,
    stream: Option<(u32, u32)>,
    no_video: bool,
    percent: Option<u32>,
    offline: bool,
    failure: Option<(StartFailure, bool)>,
    /// The step line to come back to when the device is back online.
    before_offline: Option<Held>,
    pub details_open: bool,
    announced_slow: bool,
    /// Every miss was "no such title", for the failure's primary.
    all_no_match: bool,
}

impl Start {
    pub fn new(id: AnilistId, episode: &str, resume: Option<f64>) -> Self {
        Self {
            id,
            episode: episode.to_owned(),
            resume,
            from_beginning: false,
            shown: Held {
                text: t::start_looking_up_title(),
                key: "lookup",
                since: Duration::ZERO,
            },
            waiting: None,
            rung_at: Duration::ZERO,
            looked_up_at: None,
            asking: None,
            seen: Vec::new(),
            resting: Vec::new(),
            last_miss: None,
            peers: None,
            rate: None,
            finding_peers: None,
            found: None,
            stream: None,
            no_video: false,
            percent: None,
            offline: false,
            failure: None,
            before_offline: None,
            details_open: false,
            announced_slow: false,
            all_no_match: true,
        }
    }

    pub fn number(&self) -> i64 {
        crate::party::number(&self.episode)
    }

    /// The viewer asked to begin at 0: the Resuming fact and its button go.
    pub fn begin_from_start(&mut self) {
        self.from_beginning = true;
    }

    pub fn resuming(&self) -> Option<f64> {
        self.resume.filter(|_| !self.from_beginning)
    }

    /// Say `text` on the step line at `at`: at once when the one showing has had its time,
    /// else it waits, and a newer one during the wait replaces it.
    fn say(&mut self, text: String, key: &'static str, at: Duration) {
        if self.shown.text == text && self.shown.key == key {
            self.waiting = None;
            return;
        }
        let held = at.saturating_sub(self.shown.since);
        // A percentage changes in place, with no hold.
        if held >= HOLD || key == self.shown.key {
            self.shown = Held { text, key, since: if key == self.shown.key { self.shown.since } else { at } };
            self.waiting = None;
        } else {
            self.waiting = Some((text, key));
        }
    }

    /// The clock moved: a waiting text whose turn has come takes its place. True when the
    /// step line changed.
    pub fn tick(&mut self, at: Duration) -> bool {
        if let Some((text, key)) = self.waiting.clone()
            && at.saturating_sub(self.shown.since) >= HOLD
        {
            self.shown = Held { text, key, since: at };
            self.waiting = None;
            return true;
        }
        false
    }

    /// A step the engine said, with the line it brings.
    pub fn step(&mut self, step: &StartStep, line: Option<&Msg>, at: Duration) {
        if self.failure.is_some() {
            return;
        }
        let text = line.map(say::msg);
        match step {
            StartStep::LookingUpTitle => {}
            StartStep::GettingSourcesReady => {
                self.looked_up_at.get_or_insert(at);
            }
            StartStep::AskingSource { name, place, total, pinned } => {
                self.looked_up_at.get_or_insert(at);
                self.close_asking(at, None);
                self.asking = Some(Asking {
                    name: name.clone(),
                    place: *place,
                    total: *total,
                    pinned: *pinned,
                });
                self.rung_at = at;
                self.finding_peers = None;
            }
            StartStep::SourceMissed { name, miss } => {
                self.close_asking(at, Some(*miss));
                if *miss != SourceMiss::NoMatch {
                    self.all_no_match = false;
                }
                self.last_miss = Some((name.clone(), *miss));
            }
            StartStep::SourcesResting { names } => self.resting = names.clone(),
            StartStep::FindingPeers { name } => {
                self.finding_peers = Some(name.clone());
            }
            StartStep::Found { source, translation, .. } => {
                self.close_asking(at, None);
                self.found = Some((source.clone(), *translation));
                self.last_miss = None;
                self.finding_peers = None;
                self.rung_at = at;
            }
            StartStep::FetchingSubtitles => {}
            StartStep::OpeningStream { attempt, total } => {
                self.stream = Some((*attempt, *total));
                self.rung_at = at;
            }
            StartStep::TryingAnotherStream { next, total } => {
                self.no_video = true;
                self.stream = Some((*next, *total));
                self.rung_at = at;
                self.percent = None;
            }
            StartStep::Swarm { peers, bytes_per_second } => {
                self.peers = Some(*peers);
                self.rate = Some(*bytes_per_second);
            }
            StartStep::Speed { bytes_per_second } => self.rate = Some(*bytes_per_second),
            StartStep::Buffering { percent } => self.percent = Some(*percent),
        }
        if self.offline {
            return;
        }
        let (text, key) = match step {
            StartStep::Buffering { percent } => (
                t::player_start_buffering_percent(&say::percent(f64::from(*percent))),
                "buffering",
            ),
            StartStep::LookingUpTitle => (text.unwrap_or_default(), "lookup"),
            StartStep::GettingSourcesReady => (text.unwrap_or_default(), "sources"),
            StartStep::AskingSource { .. } => (text.unwrap_or_default(), "asking"),
            StartStep::FindingPeers { .. } => (text.unwrap_or_default(), "peers"),
            StartStep::Found { .. } => (text.unwrap_or_default(), "opening"),
            StartStep::TryingAnotherStream { .. } => (text.unwrap_or_default(), "another"),
            _ => return,
        };
        if !text.is_empty() {
            self.say(text, key, at);
        }
    }

    fn close_asking(&mut self, at: Duration, miss: Option<SourceMiss>) {
        if let Some(asking) = self.asking.take() {
            self.seen.push(Seen {
                name: asking.name,
                miss,
                took: Some(at.saturating_sub(self.rung_at)),
            });
        } else if let (Some(miss), Some(last)) = (miss, self.seen.last_mut())
            && last.miss.is_none()
        {
            last.miss = Some(miss);
        }
    }

    /// The device went offline or came back.
    pub fn connectivity(&mut self, offline: bool, at: Duration) {
        if offline && !self.offline {
            self.before_offline = Some(self.shown.clone());
            self.offline = true;
            self.waiting = None;
            self.shown = Held { text: t::player_start_offline(), key: "offline", since: at };
        } else if !offline && self.offline {
            self.offline = false;
            if let Some(mut held) = self.before_offline.take() {
                held.since = at;
                self.shown = held;
            }
        }
    }

    /// The start ended without playing.
    pub fn fail(&mut self, failure: StartFailure) {
        self.failure = Some((failure, true));
        self.waiting = None;
    }

    /// The step line now, when one is wanted a moment later than it was told.
    fn step_text(&self, at: Duration) -> (String, &'static str) {
        if self.offline {
            return (self.shown.text.clone(), self.shown.key);
        }
        if at >= SLOW_AT
            && self.shown.key == "asking"
            && let Some(asking) = &self.asking
        {
            return (t::player_start_still_waiting(&asking.name), "slow");
        }
        (self.shown.text.clone(), self.shown.key)
    }

    /// The lead of the detail line: the facts of the step, before the seconds.
    fn lead(&self) -> String {
        if self.offline {
            return t::player_start_offline_detail(&self.episode);
        }
        let mut facts: Vec<String> = Vec::new();
        let place = |asking: &Asking| {
            (asking.total > 1)
                .then(|| t::player_start_place(i64::from(asking.place), i64::from(asking.total)))
        };
        match self.shown.key {
            "asking" | "slow" => {
                if let Some((name, miss)) = &self.last_miss {
                    facts.push(miss_text(name, *miss, &self.episode));
                } else if self.asking.as_ref().is_some_and(|a| a.pinned) {
                    facts.push(t::player_start_pinned());
                }
                if let Some(asking) = &self.asking {
                    if self.last_miss.is_some() && asking.pinned {
                        // The miss leads; the pin is said only on its own.
                    }
                    facts.extend(place(asking));
                }
            }
            "peers" => {
                if let Some(peers) = self.peers {
                    facts.push(t::player_start_peers(i64::from(peers)));
                } else if let Some(Some(name)) = &self.finding_peers {
                    facts.push(t::player_start_from(name));
                }
            }
            "buffering" => {
                if let Some(peers) = self.peers {
                    facts.push(t::player_start_peers(i64::from(peers)));
                }
                if let Some(rate) = self.rate.filter(|r| *r > 0) {
                    facts.push(t::player_start_speed(&say::bytes(rate)));
                }
            }
            "another" => {
                facts.push(t::player_start_no_video());
                if let Some((next, total)) = self.stream {
                    facts.push(t::player_start_stream_place(i64::from(next), i64::from(total)));
                }
            }
            "opening" => {
                if let Some((Some(name), _)) = &self.found {
                    facts.push(t::player_start_from(name));
                }
                if let Some(peers) = self.peers {
                    facts.push(t::player_start_peers(i64::from(peers)));
                }
                if let Some(rate) = self.rate.filter(|r| *r > 0) {
                    facts.push(t::player_start_speed(&say::bytes(rate)));
                }
            }
            _ => {}
        }
        if facts.is_empty()
            && let Some(position) = self.resuming()
        {
            facts.push(t::player_start_resuming(&say::clock(position)));
        }
        facts.join(JOIN)
    }

    /// Whether another source or stream is left to skip to, and what Skip would say.
    fn skippable(&self) -> Option<Button> {
        let skip = |label: String| Button { action: Action::Skip, label, look: Look::Tonal };
        match self.shown.key {
            "asking" | "slow" => {
                let asking = self.asking.as_ref()?;
                (asking.place < asking.total)
                    .then(|| skip(t::player_start_skip_source(&asking.name)))
            }
            "opening" | "another" | "buffering" => {
                let (attempt, total) = self.stream?;
                (attempt < total).then(|| skip(t::player_start_skip_stream()))
            }
            _ => None,
        }
    }

    /// The buttons at `at`, in arrival order.
    fn buttons(&self, at: Duration) -> Vec<Button> {
        let button = |action, label, look| Button { action, label, look };
        if let Some((failure, _)) = &self.failure {
            let again = button(Action::TryAgain, t::action_try_again(), Look::Tonal);
            let details = button(Action::Details, t::player_start_details(), Look::Quiet);
            let close = button(Action::Close, t::action_close(), Look::Quiet);
            return match failure.why {
                NoSourceWhy::NotAired => vec![button(Action::Close, t::action_close(), Look::Tonal)],
                NoSourceWhy::NoSources => vec![
                    button(Action::AddStore, t::stores_add_title(), Look::Primary),
                    again,
                    close,
                ],
                NoSourceWhy::NotFound if self.all_no_match && !self.seen.is_empty() => vec![
                    button(Action::FixMatch, t::player_start_fix_match(), Look::Primary),
                    button(Action::ChooseAnother, t::player_choose_another_source(), Look::Tonal),
                    details,
                    close,
                ],
                NoSourceWhy::NotFound => vec![
                    button(Action::ChooseAnother, t::player_choose_another_source(), Look::Primary),
                    again,
                    details,
                    close,
                ],
            };
        }
        let mut row = vec![button(Action::Cancel, t::action_cancel(), Look::Quiet)];
        if self.offline {
            row.push(button(Action::ShowDownloads, t::state_offline_show_downloads(), Look::Tonal));
            return row;
        }
        if at >= DETAIL_AT && self.resuming().is_some() {
            row.push(button(
                Action::FromBeginning,
                t::player_start_from_beginning(),
                Look::Tonal,
            ));
        }
        if at >= TOOLS_AT {
            row.push(button(Action::ChooseSource, t::player_choose_source(), Look::Tonal));
            row.push(button(Action::Details, t::player_start_details(), Look::Quiet));
        }
        if at.saturating_sub(self.rung_at) >= SKIP_AFTER
            && let Some(skip) = self.skippable()
        {
            row.push(skip);
        }
        row
    }

    fn rows(&self, at: Duration) -> Vec<Row> {
        let mut rows = Vec::new();
        if let Some(done) = self.looked_up_at {
            rows.push(Row {
                mark: Mark::Done,
                name: t::linux_start_row_looked_up(),
                note: String::new(),
                time: seconds_text(done),
                skip: false,
            });
        }
        for seen in &self.seen {
            rows.push(Row {
                mark: if seen.miss.is_some() { Mark::Empty } else { Mark::Done },
                name: seen.name.clone(),
                note: seen.miss.map(|m| miss_note(m, &self.episode)).unwrap_or_default(),
                time: seen.took.map(seconds_text).unwrap_or_default(),
                skip: false,
            });
        }
        if let Some(asking) = &self.asking
            && self.failure.is_none()
        {
            rows.push(Row {
                mark: Mark::Now,
                name: asking.name.clone(),
                note: t::linux_start_row_asking(),
                time: seconds_text(at.saturating_sub(self.rung_at)),
                skip: asking.place < asking.total && at.saturating_sub(self.rung_at) >= SKIP_AFTER,
            });
        }
        for name in &self.resting {
            rows.push(Row {
                mark: Mark::Rest,
                name: name.clone(),
                note: t::player_start_resting(),
                time: String::new(),
                skip: false,
            });
        }
        rows
    }

    /// Everything the screen shows at `at`.
    pub fn frame(&self, at: Duration) -> Frame {
        let (step, key) = self.step_text(at);
        let failure = self.failure.as_ref().map(|(failure, _)| {
            (
                t::player_failed_title(self.number()),
                say::msg(&failure.text),
            )
        });
        let lead = if failure.is_some() || at < DETAIL_AT && !self.offline {
            String::new()
        } else {
            self.lead()
        };
        let seconds = (at >= TOOLS_AT && failure.is_none() && !self.offline)
            .then_some(at.as_secs() as i64);
        Frame {
            step,
            key,
            lead,
            seconds,
            buttons: self.buttons(at),
            failure,
            percent: self.percent.filter(|_| self.shown.key == "buffering"),
            rows: self.rows(at),
            translation: self.found.as_ref().map(|(_, translation)| *translation),
            details_open: self.details_open,
        }
    }

    /// The moment, once, to tell a screen reader the start is slow.
    pub fn slow_announcement(&mut self, at: Duration) -> Option<String> {
        if self.announced_slow || at < SKIP_AFTER || self.failure.is_some() || self.offline {
            return None;
        }
        self.announced_slow = true;
        Some(t::linux_start_still_starting())
    }
}

fn seconds_text(took: Duration) -> String {
    t::linux_start_seconds(took.as_secs_f64().round() as i64)
}

fn miss_text(name: &str, miss: SourceMiss, episode: &str) -> String {
    match miss {
        SourceMiss::NoMatch => t::player_start_miss_no_match(name),
        SourceMiss::NoEpisode => t::player_start_miss_no_episode(name, episode),
        SourceMiss::Failed => t::player_start_miss_failed(name),
        SourceMiss::TimedOut => t::player_start_miss_timed_out(name),
        SourceMiss::Skipped => t::player_start_miss_skipped(name),
    }
}

/// A Details row's note for a source that missed: the sentence without the name in front.
fn miss_note(miss: SourceMiss, episode: &str) -> String {
    match miss {
        SourceMiss::NoMatch => t::linux_start_row_no_match(),
        SourceMiss::NoEpisode => t::linux_start_row_no_episode(episode),
        SourceMiss::Failed => t::linux_start_row_failed(),
        SourceMiss::TimedOut => t::linux_start_row_timed_out(),
        SourceMiss::Skipped => t::linux_start_row_skipped(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_model::starting::StartStep as S;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn start() -> Start {
        Start::new(AnilistId::new(900_001), "7", Some(606.0))
    }

    fn asking(name: &str, place: u32, total: u32) -> S {
        S::AskingSource { name: name.into(), place, total, pinned: false }
    }

    #[test]
    fn a_newer_step_waits_out_the_hold_and_only_the_latest_shows() {
        let mut s = start();
        s.step(&asking("Alpha", 1, 3), Some(&Msg::StartAskingSource { name: "Alpha".into() }), ms(100));
        // Inside the first text's hold.
        assert_eq!(s.frame(ms(100)).key, "lookup");
        s.step(&S::Found { count: 1, source: None, quality: None, translation: Translation::Sub }, Some(&Msg::StartOpeningStream), ms(300));
        assert_eq!(s.frame(ms(300)).key, "lookup");
        assert!(!s.tick(ms(500)));
        assert!(s.tick(ms(600)));
        let frame = s.frame(ms(600));
        assert_eq!(frame.key, "opening", "the asking step was passed over");
    }

    #[test]
    fn a_step_after_the_hold_shows_at_once() {
        let mut s = start();
        s.step(&asking("Alpha", 1, 3), Some(&Msg::StartAskingSource { name: "Alpha".into() }), ms(700));
        assert_eq!(s.frame(ms(700)).key, "asking");
        assert!(s.frame(ms(700)).step.contains("Alpha"));
    }

    #[test]
    fn the_tools_come_by_time_in_arrival_order() {
        let mut s = start();
        s.step(&asking("Alpha", 1, 3), Some(&Msg::StartAskingSource { name: "Alpha".into() }), ms(700));
        let labels = |s: &Start, at| -> Vec<Action> {
            s.frame(at).buttons.iter().map(|b| b.action).collect()
        };
        assert_eq!(labels(&s, ms(900)), [Action::Cancel]);
        assert_eq!(labels(&s, ms(1_000)), [Action::Cancel, Action::FromBeginning]);
        assert_eq!(
            labels(&s, ms(3_000)),
            [Action::Cancel, Action::FromBeginning, Action::ChooseSource, Action::Details]
        );
        assert_eq!(labels(&s, ms(8_800)).last(), Some(&Action::Skip));
    }

    #[test]
    fn skip_needs_eight_seconds_on_one_source_and_another_to_go() {
        let mut s = start();
        s.begin_from_start();
        s.step(&asking("Alpha", 1, 3), Some(&Msg::StartAskingSource { name: "Alpha".into() }), ms(700));
        assert!(!s.frame(ms(8_600)).buttons.iter().any(|b| b.action == Action::Skip));
        let f = s.frame(ms(8_700));
        let skip = f.buttons.iter().find(|b| b.action == Action::Skip).expect("skip");
        assert!(skip.label.contains("Alpha"));
        // The last source has nothing to skip to.
        s.step(&asking("Harbor", 3, 3), Some(&Msg::StartAskingSource { name: "Harbor".into() }), ms(9_000));
        assert!(!s.frame(ms(20_000)).buttons.iter().any(|b| b.action == Action::Skip));
    }

    #[test]
    fn detail_comes_at_one_second_and_the_seconds_at_three() {
        let mut s = start();
        s.step(&asking("Alpha", 1, 3), Some(&Msg::StartAskingSource { name: "Alpha".into() }), ms(700));
        assert_eq!(s.frame(ms(900)).detail(), "");
        let one = s.frame(ms(1_000)).detail();
        assert!(one.contains("1 of 3"), "{one}");
        assert!(!one.ends_with(" s"), "{one}");
        assert!(s.frame(ms(10_000)).detail().ends_with("10 s"));
    }

    #[test]
    fn resuming_leads_the_detail_only_when_nothing_else_does() {
        let mut s = Start::new(AnilistId::new(1), "7", Some(606.0));
        s.step(&S::LookingUpTitle, Some(&Msg::StartLookingUpTitle), ms(0));
        assert_eq!(s.frame(ms(1_500)).detail(), "Resuming at 10:06");
        s.begin_from_start();
        assert_eq!(s.frame(ms(1_500)).detail(), "");
        assert!(!s.frame(ms(1_500)).buttons.iter().any(|b| b.action == Action::FromBeginning));
    }

    #[test]
    fn slow_wording_from_twenty_seconds_and_no_false_failure() {
        let mut s = start();
        s.step(&asking("Brook", 2, 3), Some(&Msg::StartAskingSource { name: "Brook".into() }), ms(1_500));
        let f = s.frame(ms(24_000));
        assert_eq!(f.step, "Still waiting for Brook");
        assert!(f.failure.is_none());
    }

    #[test]
    fn only_a_failure_event_makes_a_failure() {
        let mut s = start();
        s.fail(StartFailure {
            why: NoSourceWhy::NotAired,
            text: Msg::NoticeNotAired { episode: 7, when: kaeri_model::Cause::AirsTomorrow { time: "18:30".into() } },
        });
        let f = s.frame(ms(700));
        assert_eq!(f.failure.as_ref().map(|f| f.0.as_str()), Some("Couldn't play episode 7"));
        assert_eq!(f.buttons.len(), 1);
        assert_eq!(f.buttons[0].action, Action::Close);
    }

    #[test]
    fn offline_waits_with_show_downloads_and_comes_back() {
        let mut s = start();
        s.connectivity(true, ms(0));
        let f = s.frame(ms(1_000));
        assert_eq!(f.step, "Waiting for a connection");
        assert!(f.detail().contains("You're offline"));
        let actions: Vec<_> = f.buttons.iter().map(|b| b.action).collect();
        assert_eq!(actions, [Action::Cancel, Action::ShowDownloads]);
        s.connectivity(false, ms(5_000));
        assert_eq!(s.frame(ms(5_000)).key, "lookup");
    }

    #[test]
    fn buffering_percent_is_a_number_in_the_step_and_changes_in_place() {
        let mut s = start();
        s.step(&S::Buffering { percent: 10 }, Some(&Msg::StartBuffering), ms(5_000));
        assert_eq!(s.frame(ms(5_000)).percent, Some(10));
        s.step(&S::Buffering { percent: 25 }, Some(&Msg::StartBuffering), ms(5_100));
        let f = s.frame(ms(5_100));
        assert_eq!(f.percent, Some(25));
        assert!(f.step.contains("25"), "{}", f.step);
    }

    #[test]
    fn a_direct_start_has_no_percent() {
        let mut s = start();
        s.step(&S::Speed { bytes_per_second: 1_800_000 }, None, ms(3_400));
        assert_eq!(s.frame(ms(3_400)).percent, None);
    }

    #[test]
    fn the_slow_announcement_comes_once() {
        let mut s = start();
        assert!(s.slow_announcement(ms(7_000)).is_none());
        assert!(s.slow_announcement(ms(8_000)).is_some());
        assert!(s.slow_announcement(ms(9_000)).is_none());
    }
}
