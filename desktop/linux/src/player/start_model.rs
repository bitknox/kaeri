//! What the starting screen says and offers, from the engine's steps and the clock: no widgets,
//! so every rule of the design is tested here. The view (`start_view.rs`) draws a [`Frame`].
//!
//! Times are `Duration`s since the screen opened, passed in by the caller.

use std::time::Duration;

use kaeri_core::{ids::AnilistId, media::Translation};
use kaeri_i18n::t;
use kaeri_model::{
    Cause, Msg,
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
/// How long one source or stream is tried before Skip is offered for it, and how long a start
/// waits before a screen reader hears that it is still starting.
pub const SKIP_AFTER: Duration = Duration::from_secs(8);
/// When the step line says plainly that it is still waiting.
pub const SLOW_AT: Duration = Duration::from_secs(20);

const JOIN: &str = " · "; // i18n-skip

/// What a button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// What the row's Skip says, when it carries one.
    pub skip: Option<String>,
}

/// Where a start is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Lookup,
    Sources,
    Asking,
    Peers,
    Opening,
    Another,
    Buffering,
    /// Offline: the engine waits for a connection before it asks any source.
    Waiting,
}

/// What the screen shows now.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// The step line, and the phase it belongs to.
    pub step: String,
    pub phase: Phase,
    /// The detail line's facts and, from 3 s, the seconds; empty before 1 s.
    pub lead: String,
    pub seconds: Option<i64>,
    /// The buttons now, in arrival order.
    pub buttons: Vec<Button>,
    /// The failure's title and its second line (the reason, then who was asked), when the
    /// start has failed.
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
            (true, Some(s)) => t::player_start_seconds(s),
            (false, None) => self.lead.clone(),
            (false, Some(s)) => format!("{}{JOIN}{}", self.lead, t::player_start_seconds(s)),
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
    phase: Phase,
    since: Duration,
}

/// One start, from the play being staged to its first frame or its end.
pub struct Start {
    pub id: AnilistId,
    pub episode: String,
    resume: Option<f64>,
    from_beginning: bool,
    shown: Held,
    waiting: Option<(String, Phase)>,
    /// Since when the start has been under way: the screen opening, or the end of a wait for a
    /// connection. The slow wording and the slow announcement count from here.
    active_since: Duration,
    /// Since when the rung (the source being asked, the stream being opened) has been tried.
    rung_at: Duration,
    looked_up_at: Option<Duration>,
    asking: Option<Asking>,
    /// What came of the sources asked, in order, and the ones resting.
    seen: Vec<Seen>,
    resting: Vec<String>,
    last_miss: Option<(String, SourceMiss)>,
    peers: Option<u32>,
    finding_peers: Option<Option<String>>,
    found: Option<(Option<String>, Translation)>,
    stream: Option<(u32, u32)>,
    no_video: bool,
    percent: Option<u32>,
    rate: Option<u64>,
    failure: Option<StartFailure>,
    pub details_open: bool,
    announced_slow: bool,
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
                phase: Phase::Lookup,
                since: Duration::ZERO,
            },
            waiting: None,
            active_since: Duration::ZERO,
            rung_at: Duration::ZERO,
            looked_up_at: None,
            asking: None,
            seen: Vec::new(),
            resting: Vec::new(),
            last_miss: None,
            peers: None,
            finding_peers: None,
            found: None,
            stream: None,
            no_video: false,
            percent: None,
            rate: None,
            failure: None,
            details_open: false,
            announced_slow: false,
        }
    }

    /// Whether the start resumes where the episode was left: the slot for Play from the
    /// beginning is kept from the start only then.
    pub fn resumes(&self) -> bool {
        self.resume.is_some()
    }

    /// The viewer asked to begin at 0: the detail line says so and the button is spent.
    pub fn begin_from_start(&mut self) {
        self.from_beginning = true;
    }

    /// Whether the start is waiting for a connection.
    pub fn offline(&self) -> bool {
        self.shown.phase == Phase::Waiting
    }

    /// Say `text` on the step line at `at`: at once when the one showing has had its time,
    /// else it waits, and a newer one during the wait replaces it.
    fn say(&mut self, text: String, phase: Phase, at: Duration) {
        if self.shown.text == text {
            self.shown.phase = phase;
            self.waiting = None;
            return;
        }
        let held = at.saturating_sub(self.shown.since);
        // A percentage changes in place, with no hold.
        let in_place = phase == Phase::Buffering && self.shown.phase == Phase::Buffering;
        if held >= HOLD || in_place {
            let since = if in_place { self.shown.since } else { at };
            self.shown = Held { text, phase, since };
            self.waiting = None;
        } else {
            self.waiting = Some((text, phase));
        }
    }

    /// The clock moved: a waiting text whose turn has come takes its place. True when the
    /// step line changed.
    pub fn tick(&mut self, at: Duration) -> bool {
        if let Some((text, phase)) = self.waiting.clone()
            && at.saturating_sub(self.shown.since) >= HOLD
        {
            self.shown = Held { text, phase, since: at };
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
        if *step == StartStep::WaitingForConnection {
            // Said once per wait; the engine asks no source until it is back.
            self.waiting = None;
            self.shown =
                Held { text: t::player_start_offline(), phase: Phase::Waiting, since: at };
            return;
        }
        if self.offline() {
            // Back online: the start is under way from now.
            self.active_since = at;
            self.rung_at = at;
        }
        let text = line.map(say::msg);
        match step {
            StartStep::LookingUpTitle | StartStep::FetchingSubtitles => {}
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
            StartStep::WaitingForConnection => {}
        }
        let (text, phase) = match step {
            StartStep::Buffering { percent } => (
                t::player_start_buffering_percent(&say::percent(f64::from(*percent))),
                Phase::Buffering,
            ),
            // A direct stream has data coming: it buffers, with no number to give.
            StartStep::Speed { .. } if self.shown.phase == Phase::Opening => {
                (say::msg(&Msg::StartBuffering), Phase::Buffering)
            }
            StartStep::LookingUpTitle => (text.unwrap_or_default(), Phase::Lookup),
            StartStep::GettingSourcesReady => (text.unwrap_or_default(), Phase::Sources),
            StartStep::AskingSource { .. } => (text.unwrap_or_default(), Phase::Asking),
            StartStep::FindingPeers { .. } => (text.unwrap_or_default(), Phase::Peers),
            StartStep::Found { .. } => (text.unwrap_or_default(), Phase::Opening),
            StartStep::TryingAnotherStream { .. } => (text.unwrap_or_default(), Phase::Another),
            _ => return,
        };
        if !text.is_empty() {
            self.say(text, phase, at);
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

    /// The start ended without playing.
    pub fn fail(&mut self, failure: StartFailure) {
        self.failure = Some(failure);
        self.waiting = None;
    }

    /// The source the wait is on, once one has been asked.
    fn waiting_for(&self) -> Option<&str> {
        let asked = self.asking.as_ref().map(|a| a.name.as_str());
        let from = self.found.as_ref().and_then(|(source, _)| source.as_deref());
        match self.shown.phase {
            Phase::Lookup | Phase::Sources | Phase::Waiting => None,
            Phase::Asking => asked,
            Phase::Peers => self.finding_peers.as_ref().and_then(Option::as_deref).or(asked),
            Phase::Opening | Phase::Another | Phase::Buffering => from.or(asked),
        }
    }

    /// The step line at `at`: the engine's, and in any phase past 20 s, said plainly.
    fn step_text(&self, at: Duration) -> String {
        if at.saturating_sub(self.active_since) >= SLOW_AT
            && let Some(name) = self.waiting_for()
        {
            return t::player_start_still_waiting(name);
        }
        self.shown.text.clone()
    }

    /// The lead of the detail line: where the episode starts, then the facts of the step.
    fn lead(&self) -> String {
        if self.offline() {
            return t::player_start_offline_detail(&self.episode);
        }
        let mut facts: Vec<String> = Vec::new();
        if self.from_beginning {
            facts.push(t::player_start_beginning());
        } else if let Some(position) = self.resume {
            facts.push(t::player_start_resuming(&say::clock(position)));
        }
        match self.shown.phase {
            Phase::Lookup | Phase::Sources | Phase::Waiting => {}
            Phase::Asking => {
                if let Some(asking) = &self.asking {
                    if asking.pinned {
                        facts.push(t::player_start_pinned());
                    }
                    if let Some((name, miss)) = &self.last_miss {
                        facts.push(miss_text(name, *miss, &self.episode));
                    }
                    if asking.total > 1 {
                        facts.push(t::player_start_place(
                            i64::from(asking.place),
                            i64::from(asking.total),
                        ));
                    }
                }
            }
            Phase::Peers => {
                if let Some(Some(name)) = &self.finding_peers {
                    facts.push(t::player_start_from(name));
                }
            }
            Phase::Opening | Phase::Another | Phase::Buffering => {
                if self.no_video {
                    facts.push(t::player_start_no_video());
                }
                if let Some(peers) = self.peers {
                    facts.push(t::player_start_peers(i64::from(peers)));
                    if let Some(rate) = self.rate.filter(|r| *r > 0) {
                        facts.push(t::player_start_speed(&speed(rate)));
                    }
                } else if let Some((Some(name), _)) = &self.found
                    && !self.no_video
                {
                    facts.push(t::player_start_from(name));
                }
                if self.no_video
                    && let Some((next, total)) = self.stream.filter(|(_, total)| *total > 1)
                {
                    facts.push(t::player_start_stream_place(i64::from(next), i64::from(total)));
                }
            }
        }
        facts.join(JOIN)
    }

    /// What Skip would say now, when there is another source or stream to skip to.
    fn skip_label(&self, at: Duration) -> Option<String> {
        if at.saturating_sub(self.rung_at) < SKIP_AFTER {
            return None;
        }
        let asking = self.asking.as_ref();
        let source = || {
            asking.filter(|a| a.place < a.total).map(|a| t::player_start_skip_source(&a.name))
        };
        match self.shown.phase {
            Phase::Lookup | Phase::Sources | Phase::Waiting => None,
            Phase::Asking | Phase::Peers => source(),
            Phase::Opening | Phase::Another | Phase::Buffering => match self.stream {
                Some((attempt, total)) if attempt < total => {
                    Some(t::player_start_skip_stream())
                }
                _ => source(),
            },
        }
    }

    /// The buttons at `at`, in arrival order.
    fn buttons(&self, at: Duration) -> Vec<Button> {
        let button = |action, label, look| Button { action, label, look };
        if let Some(failure) = &self.failure {
            return failure_buttons(failure);
        }
        let mut row = vec![button(Action::Cancel, t::action_cancel(), Look::Quiet)];
        if self.offline() {
            row.push(button(
                Action::ShowDownloads,
                t::state_offline_show_downloads(),
                Look::Tonal,
            ));
            return row;
        }
        if at >= DETAIL_AT && self.resume.is_some() && !self.from_beginning {
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
        if let Some(label) = self.skip_label(at) {
            row.push(button(Action::Skip, label, Look::Tonal));
        }
        row
    }

    fn rows(&self, at: Duration) -> Vec<Row> {
        let mut rows = Vec::new();
        if let Some(done) = self.looked_up_at {
            rows.push(Row {
                mark: Mark::Done,
                name: t::player_start_looked_up(),
                note: String::new(),
                time: seconds_text(done),
                skip: None,
            });
        }
        for seen in &self.seen {
            rows.push(Row {
                mark: if seen.miss.is_some() { Mark::Empty } else { Mark::Done },
                name: seen.name.clone(),
                note: seen.miss.map(|m| miss_note(m, &self.episode)).unwrap_or_default(),
                time: seen.took.map(seconds_text).unwrap_or_default(),
                skip: None,
            });
        }
        if let Some(asking) = &self.asking
            && self.failure.is_none()
        {
            let skip = (asking.place < asking.total
                && at.saturating_sub(self.rung_at) >= SKIP_AFTER)
                .then(|| t::player_start_skip_source(&asking.name));
            rows.push(Row {
                mark: Mark::Now,
                name: asking.name.clone(),
                note: t::player_start_row_asking(),
                time: seconds_text(at.saturating_sub(self.rung_at)),
                skip,
            });
        }
        for name in &self.resting {
            rows.push(Row {
                mark: Mark::Rest,
                name: name.clone(),
                note: t::player_start_resting(),
                time: String::new(),
                skip: None,
            });
        }
        rows
    }

    /// Everything the screen shows at `at`.
    pub fn frame(&self, at: Duration) -> Frame {
        let failure = self.failure.as_ref().map(|failure| {
            let reason = failure_reason(&failure.text);
            let second = match names(&failure.asked) {
                Some(names) => format!("{reason} {}", t::player_start_asked(&names)),
                None => reason,
            };
            (t::player_failed_title_text(&self.episode), second)
        });
        let offline = self.offline();
        let lead = if failure.is_some() || at < DETAIL_AT && !offline {
            String::new()
        } else {
            self.lead()
        };
        let seconds =
            (at >= TOOLS_AT && failure.is_none() && !offline).then_some(at.as_secs() as i64);
        Frame {
            step: self.step_text(at),
            phase: self.shown.phase,
            lead,
            seconds,
            buttons: self.buttons(at),
            failure,
            percent: self.percent.filter(|_| self.shown.phase == Phase::Buffering),
            rows: self.rows(at),
            translation: self.found.as_ref().map(|(_, translation)| *translation),
            details_open: self.details_open,
        }
    }

    /// The moment, once, to tell a screen reader the start is slow.
    pub fn slow_announcement(&mut self, at: Duration) -> Option<String> {
        if self.announced_slow
            || at.saturating_sub(self.active_since) < SKIP_AFTER
            || self.failure.is_some()
            || self.offline()
        {
            return None;
        }
        self.announced_slow = true;
        Some(t::player_start_still_starting())
    }
}

/// The buttons of a failure: the primary first, by what went wrong.
fn failure_buttons(failure: &StartFailure) -> Vec<Button> {
    let button = |action, label, look| Button { action, label, look };
    let again = |look| button(Action::TryAgain, t::action_try_again(), look);
    let another = |look| button(Action::ChooseAnother, t::player_choose_another_source(), look);
    let details = button(Action::Details, t::player_start_details(), Look::Quiet);
    let close = button(Action::Close, t::action_close(), Look::Quiet);
    match failure.why {
        NoSourceWhy::NotAired => vec![button(Action::Close, t::action_close(), Look::Tonal)],
        NoSourceWhy::NoSources => {
            vec![button(Action::AddStore, t::stores_add_title(), Look::Primary), close]
        }
        NoSourceWhy::NoConnection => vec![
            again(Look::Primary),
            button(Action::ShowDownloads, t::state_offline_show_downloads(), Look::Tonal),
            close,
        ],
        NoSourceWhy::NotFound => match &failure.text {
            Msg::NoticeReason { reason: Cause::TitleNoMatch { .. } } => vec![
                button(Action::FixMatch, t::player_start_fix_match(), Look::Primary),
                another(Look::Tonal),
                details,
                close,
            ],
            Msg::NoticeReason { reason: Cause::SourcesTimedOut } => {
                vec![again(Look::Primary), another(Look::Tonal), details, close]
            }
            _ => vec![another(Look::Primary), again(Look::Tonal), details, close],
        },
    }
}

/// The failure's reason in plain words: the cause's first sentence, without what each source
/// answered (the Details rows have that) and without the hint after it (the buttons give it).
/// Built from the cause, not cut out of its text.
pub fn failure_reason(text: &Msg) -> String {
    let bare = |reason: Cause| say::msg(&Msg::NoticeReason { reason });
    let full = match text {
        Msg::NoticeReason { reason: Cause::NoSourceHasEpisode { episode, .. } } => {
            bare(Cause::NoSourceHasEpisode { episode: episode.clone(), detail: String::new() })
        }
        Msg::NoticeReason { reason: Cause::TitleNoMatch { .. } } => {
            bare(Cause::TitleNoMatch { detail: String::new() })
        }
        Msg::NoticeReason { reason: Cause::ReleaseFailed { .. } } => {
            bare(Cause::ReleaseFailed { detail: String::new() })
        }
        other => say::msg(other),
    };
    first_sentence(&full)
}

/// The first sentence of `text`, ending in its stop: a full stop counts when a space or the end
/// follows it, so "7.5" is not cut.
fn first_sentence(text: &str) -> String {
    let mut end = text.len();
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let wide = matches!(c, '\u{3002}' | '\u{FF01}' | '\u{FF1F}');
        let stop = matches!(c, '.' | '!' | '?')
            && chars.peek().is_none_or(|(_, next)| next.is_whitespace());
        if wide || stop {
            end = i + c.len_utf8();
            break;
        }
    }
    let mut sentence =
        text[..end].trim().trim_end_matches([':', '\u{FF1A}']).trim_end().to_owned();
    if !sentence.ends_with(['.', '!', '?', '\u{3002}', '\u{FF01}', '\u{FF1F}']) {
        sentence.push('.');
    }
    sentence
}

/// Names as a sentence lists them: "Alpha", "Alpha and Brook", "Alpha, Brook and Harbor".
pub fn names(list: &[String]) -> Option<String> {
    match list {
        [] => None,
        [one] => Some(one.clone()),
        [head @ .., last] => {
            let first =
                head[1..].iter().fold(head[0].clone(), |list, next| t::names_list(&list, next));
            Some(t::names_pair(&first, last))
        }
    }
}

/// A rate as the detail line gives it, with one decimal from megabytes up: "3.4 MB".
pub fn speed(bytes_per_second: u64) -> String {
    let value = bytes_per_second as f64;
    if value >= 1e9 {
        t::size_gigabyte(&format!("{:.1}", value / 1e9))
    } else if value >= 1e6 {
        t::size_megabyte(&format!("{:.1}", value / 1e6))
    } else if value >= 1e3 {
        t::size_kilobyte(&format!("{:.0}", value / 1e3))
    } else {
        t::size_byte(&format!("{value:.0}"))
    }
}

fn seconds_text(took: Duration) -> String {
    t::player_start_seconds(took.as_secs_f64().round() as i64)
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
        SourceMiss::NoMatch => t::player_start_row_no_match(),
        SourceMiss::NoEpisode => t::player_start_row_no_episode(episode),
        SourceMiss::Failed => t::player_start_row_failed(),
        SourceMiss::TimedOut => t::player_start_row_timed_out(),
        SourceMiss::Skipped => t::player_start_row_skipped(),
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

    fn ask(s: &mut Start, name: &str, place: u32, total: u32, at: Duration) {
        s.step(
            &asking(name, place, total),
            Some(&Msg::StartAskingSource { name: name.into() }),
            at,
        );
    }

    fn actions(f: &Frame) -> Vec<Action> {
        f.buttons.iter().map(|b| b.action).collect()
    }

    #[test]
    fn a_newer_step_waits_out_the_hold_and_only_the_latest_shows() {
        let mut s = start();
        ask(&mut s, "Alpha", 1, 3, ms(100));
        // Inside the first text's hold.
        assert_eq!(s.frame(ms(100)).phase, Phase::Lookup);
        s.step(
            &S::Found { count: 1, source: None, quality: None, translation: Translation::Sub },
            Some(&Msg::StartOpeningStream),
            ms(300),
        );
        assert_eq!(s.frame(ms(300)).phase, Phase::Lookup);
        assert!(!s.tick(ms(500)));
        assert!(s.tick(ms(600)));
        assert_eq!(s.frame(ms(600)).phase, Phase::Opening, "the asking step was passed over");
    }

    #[test]
    fn a_step_after_the_hold_shows_at_once() {
        let mut s = start();
        ask(&mut s, "Alpha", 1, 3, ms(700));
        let f = s.frame(ms(700));
        assert_eq!(f.phase, Phase::Asking);
        assert!(f.step.contains("Alpha"));
    }

    #[test]
    fn the_tools_come_by_time_in_arrival_order() {
        let mut s = start();
        ask(&mut s, "Alpha", 1, 3, ms(700));
        assert_eq!(actions(&s.frame(ms(900))), [Action::Cancel]);
        assert_eq!(actions(&s.frame(ms(1_000))), [Action::Cancel, Action::FromBeginning]);
        assert_eq!(
            actions(&s.frame(ms(3_000))),
            [Action::Cancel, Action::FromBeginning, Action::ChooseSource, Action::Details]
        );
        assert_eq!(actions(&s.frame(ms(8_800))).last(), Some(&Action::Skip));
    }

    #[test]
    fn skip_needs_eight_seconds_on_one_source_and_another_to_go() {
        let mut s = start();
        ask(&mut s, "Alpha", 1, 3, ms(700));
        assert!(!actions(&s.frame(ms(8_600))).contains(&Action::Skip));
        let f = s.frame(ms(8_700));
        let skip = f.buttons.iter().find(|b| b.action == Action::Skip).expect("skip");
        assert!(skip.label.contains("Alpha"));
        // The last source has nothing to skip to.
        ask(&mut s, "Harbor", 3, 3, ms(9_000));
        assert!(!actions(&s.frame(ms(20_000))).contains(&Action::Skip));
    }

    #[test]
    fn detail_comes_at_one_second_and_the_seconds_at_three() {
        let mut s = start();
        ask(&mut s, "Alpha", 1, 3, ms(700));
        assert_eq!(s.frame(ms(900)).detail(), "");
        let one = s.frame(ms(1_000)).detail();
        assert_eq!(one, "Resuming at 10:06 · 1 of 3 sources");
        assert!(s.frame(ms(10_000)).detail().ends_with("10 s"));
    }

    #[test]
    fn the_resume_fact_stays_and_flips_when_playing_from_the_beginning() {
        let mut s = start();
        ask(&mut s, "Alpha", 1, 3, ms(700));
        s.step(
            &S::SourceMissed { name: "Alpha".into(), miss: SourceMiss::NoEpisode },
            None,
            ms(1_500),
        );
        ask(&mut s, "Brook", 2, 3, ms(1_500));
        s.tick(ms(1_500));
        assert_eq!(
            s.frame(ms(2_000)).detail(),
            "Resuming at 10:06 · Alpha doesn't have episode 7 · 2 of 3 sources"
        );
        s.begin_from_start();
        let f = s.frame(ms(2_000));
        assert!(f.detail().starts_with("Starting from the beginning · "), "{}", f.detail());
        assert!(!actions(&f).contains(&Action::FromBeginning));
    }

    #[test]
    fn no_resume_no_play_from_the_beginning() {
        let mut s = Start::new(AnilistId::new(1), "7", None);
        ask(&mut s, "Alpha", 1, 1, ms(700));
        assert!(!s.resumes());
        assert_eq!(actions(&s.frame(ms(1_500))), [Action::Cancel]);
        assert_eq!(s.frame(ms(1_500)).detail(), "");
    }

    #[test]
    fn slow_wording_in_every_phase_from_twenty_seconds_and_no_false_failure() {
        let mut s = start();
        ask(&mut s, "Brook", 2, 3, ms(1_500));
        let f = s.frame(ms(24_000));
        assert_eq!(f.step, "Still waiting for Brook");
        assert!(f.failure.is_none());

        let mut s = start();
        ask(&mut s, "Harbor", 1, 2, ms(700));
        s.step(
            &S::FindingPeers { name: Some("Harbor".into()) },
            Some(&Msg::StartFindingPeers),
            ms(900),
        );
        s.tick(ms(1_300));
        assert_eq!(s.frame(ms(19_000)).phase, Phase::Peers);
        assert_eq!(s.frame(ms(21_000)).step, "Still waiting for Harbor");

        let mut s = start();
        ask(&mut s, "Brook", 1, 3, ms(700));
        s.step(
            &S::Found {
                count: 3,
                source: Some("Brook".into()),
                quality: None,
                translation: Translation::Sub,
            },
            Some(&Msg::StartOpeningStream),
            ms(2_300),
        );
        assert_eq!(s.frame(ms(21_000)).step, "Still waiting for Brook");
    }

    #[test]
    fn a_failure_says_why_in_plain_words_and_who_was_asked() {
        let mut s = start();
        s.fail(StartFailure {
            why: NoSourceWhy::NotFound,
            text: Msg::NoticeReason {
                reason: Cause::NoSourceHasEpisode {
                    episode: "7".into(),
                    detail: "alpha: not found, brook: not found".into(),
                },
            },
            asked: vec!["Alpha".into(), "Brook".into()],
        });
        let (title, reason) = s.frame(ms(3_000)).failure.expect("failure");
        assert_eq!(title, "Couldn't play episode 7");
        assert_eq!(reason, "No source has episode 7. Alpha and Brook were asked.");
        assert_eq!(
            actions(&s.frame(ms(3_000))),
            [Action::ChooseAnother, Action::TryAgain, Action::Details, Action::Close]
        );
    }

    #[test]
    fn a_failure_on_a_half_episode_names_it() {
        let mut s = Start::new(AnilistId::new(1), "7.5", None);
        s.fail(StartFailure {
            why: NoSourceWhy::NoSources,
            text: Msg::NoticeReason { reason: Cause::NoSources },
            asked: Vec::new(),
        });
        let f = s.frame(ms(500));
        let (title, reason) = f.failure.clone().expect("failure");
        assert_eq!(title, "Couldn't play episode 7.5");
        assert!(!reason.contains("Add one"), "{reason}");
        assert_eq!(actions(&f), [Action::AddStore, Action::Close]);
    }

    #[test]
    fn a_title_with_no_match_leads_with_fix_match() {
        let mut s = start();
        s.fail(StartFailure {
            why: NoSourceWhy::NotFound,
            text: Msg::NoticeReason {
                reason: Cause::TitleNoMatch { detail: "alpha: no match".into() },
            },
            asked: vec!["Alpha".into()],
        });
        let f = s.frame(ms(2_000));
        assert_eq!(actions(&f)[0], Action::FixMatch);
        let reason = f.failure.expect("failure").1;
        assert!(!reason.contains("alpha: no match"), "{reason}");
        assert!(reason.ends_with("Alpha were asked."), "{reason}");
    }

    #[test]
    fn only_a_failure_event_makes_a_failure() {
        let mut s = start();
        assert!(s.frame(ms(60_000)).failure.is_none());
        s.fail(StartFailure {
            why: NoSourceWhy::NotAired,
            text: Msg::NoticeNotAired {
                episode: 7,
                when: Cause::AirsTomorrow { time: "18:30".into() },
            },
            asked: Vec::new(),
        });
        let f = s.frame(ms(700));
        assert_eq!(
            f.failure.as_ref().map(|f| f.1.as_str()),
            Some("Episode 7 airs tomorrow at 18:30.")
        );
        assert_eq!(actions(&f), [Action::Close]);
    }

    #[test]
    fn offline_waits_with_show_downloads_and_starts_on_the_next_step() {
        let mut s = start();
        s.step(&S::WaitingForConnection, None, ms(0));
        let f = s.frame(ms(1_000));
        assert_eq!(f.step, "Waiting for a connection");
        assert!(f.detail().contains("You're offline"));
        assert_eq!(actions(&f), [Action::Cancel, Action::ShowDownloads]);
        // Long offline waits are not slow starts.
        assert!(s.slow_announcement(ms(9_000)).is_none());
        ask(&mut s, "Alpha", 1, 2, ms(8_700));
        let f = s.frame(ms(8_700));
        assert_eq!(f.step, "Asking Alpha");
        assert_eq!(actions(&f)[..2], [Action::Cancel, Action::FromBeginning]);
        // The slow wording counts from the end of the wait.
        assert_eq!(s.frame(ms(26_000)).step, "Asking Alpha");
        assert_eq!(s.frame(ms(29_000)).step, "Still waiting for Alpha");
    }

    #[test]
    fn offline_too_long_fails_with_try_again_first() {
        let mut s = start();
        s.step(&S::WaitingForConnection, None, ms(0));
        s.fail(StartFailure {
            why: NoSourceWhy::NoConnection,
            text: Msg::NoticeReason { reason: Cause::NoConnection },
            asked: Vec::new(),
        });
        let f = s.frame(ms(120_000));
        assert_eq!(f.failure.expect("failure").1, "No connection.");
        assert_eq!(
            actions(&s.frame(ms(120_000))),
            [Action::TryAgain, Action::ShowDownloads, Action::Close]
        );
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
    fn a_direct_stream_buffers_with_no_percent() {
        let mut s = start();
        ask(&mut s, "Brook", 1, 3, ms(700));
        s.step(
            &S::Found {
                count: 3,
                source: Some("Brook".into()),
                quality: None,
                translation: Translation::Sub,
            },
            Some(&Msg::StartOpeningStream),
            ms(2_300),
        );
        s.step(&S::OpeningStream { attempt: 1, total: 3 }, None, ms(2_350));
        s.step(&S::Speed { bytes_per_second: 1_800_000 }, None, ms(3_400));
        let f = s.frame(ms(3_400));
        assert_eq!(f.phase, Phase::Buffering);
        assert_eq!(f.step, "Buffering");
        assert_eq!(f.percent, None);
        assert_eq!(f.detail(), "Resuming at 10:06 · From Brook · 3 s");
    }

    #[test]
    fn speed_has_one_decimal() {
        let mut s = start();
        s.step(&S::Swarm { peers: 12, bytes_per_second: 3_400_000 }, None, ms(6_000));
        s.step(&S::Buffering { percent: 40 }, Some(&Msg::StartBuffering), ms(8_000));
        assert_eq!(
            s.frame(ms(15_000)).detail(),
            "Resuming at 10:06 · 12 peers · 3.4 MB/s · 15 s"
        );
        assert_eq!(speed(14_000_000), "14.0 MB");
        assert_eq!(speed(800_000), "800 kB");
    }

    #[test]
    fn names_are_listed_as_a_sentence() {
        let names =
            |list: &[&str]| names(&list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
        assert_eq!(names(&[]), None);
        assert_eq!(names(&["Alpha"]).as_deref(), Some("Alpha"));
        assert_eq!(names(&["Alpha", "Brook"]).as_deref(), Some("Alpha and Brook"));
        assert_eq!(
            names(&["Alpha", "Brook", "Harbor"]).as_deref(),
            Some("Alpha, Brook and Harbor")
        );
    }

    #[test]
    fn the_slow_announcement_comes_once() {
        let mut s = start();
        assert!(s.slow_announcement(ms(7_000)).is_none());
        assert!(s.slow_announcement(ms(8_000)).is_some());
        assert!(s.slow_announcement(ms(9_000)).is_none());
    }
}
