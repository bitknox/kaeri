//! Marking an opening, ending or recap by hand: the steps of the marking bar, the checks a
//! marked stretch must pass, and when to ask "Was that the opening?".
//!
//! Pure, as the tracker is: the session feeds it the playhead and what the viewer pressed, and
//! reads back what to show. The limits are AniSkip's own, from its auto-vote: a time it would
//! vote down on arrival is kept on this device instead.

use crate::skip::{SkipInterval, SkipKind};

/// The shortest stretch that can be saved, in seconds. AniSkip votes anything shorter down.
pub const SHORTEST: f64 = 5.0;

/// How close a mark must be to AniSkip's time, at both edges, to count as the same, in seconds.
pub const SAME_WITHIN: f64 = 2.0;

/// How far before and after each edge "Watch the edges" plays, in seconds.
pub const PREVIEW_AROUND: f64 = 3.0;

/// The longest stretch of `kind` AniSkip takes in an episode `episode_length` seconds long, as
/// its auto-vote has it: by whether the episode is short (under 2 min), half (under 12 min),
/// full (under 35 min) or a film.
pub fn longest(kind: SkipKind, episode_length: f64) -> f64 {
    let class = if episode_length < 120.0 {
        0
    } else if episode_length < 720.0 {
        1
    } else if episode_length < 2100.0 {
        2
    } else {
        3
    };
    let table: [f64; 4] = match kind {
        SkipKind::Recap => [40.0, 180.0, 360.0, 720.0],
        SkipKind::Opening | SkipKind::Ending => [40.0, 120.0, 180.0, 480.0],
    };
    table[class]
}

/// Whether a stretch can be saved, and where it goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Check {
    /// It can be saved, and shared.
    Fine,
    /// Under [`SHORTEST`]: Save is off.
    TooShort,
    /// Longer than AniSkip takes in an episode this long: saved on this device only. `most` is
    /// AniSkip's longest, in seconds.
    TooLong { most: f64 },
}

/// Check a stretch from `start` to `end` of `kind` in an episode `episode_length` long.
pub fn check(kind: SkipKind, start: f64, end: f64, episode_length: f64) -> Check {
    let length = end - start;
    if length < SHORTEST {
        return Check::TooShort;
    }
    let most = longest(kind, episode_length);
    if length > most { Check::TooLong { most } } else { Check::Fine }
}

/// Whether `a` and `b` are the same stretch: both edges within [`SAME_WITHIN`].
pub fn same(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() <= SAME_WITHIN && (a.1 - b.1).abs() <= SAME_WITHIN
}

/// The kind a mark started at `position` of a file `duration` long is likely to be: an opening
/// in the first third, an ending in the last, and otherwise an opening.
pub fn guess_kind(position: f64, duration: Option<f64>) -> SkipKind {
    match duration {
        Some(d) if d > 0.0 && position > d * 2.0 / 3.0 => SkipKind::Ending,
        _ => SkipKind::Opening,
    }
}

/// Which step of marking the viewer is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Play to where it starts, then mark it.
    Start,
    /// Play to where it ends, then mark it.
    End,
    /// The two edges, to nudge, watch and save.
    Review,
}

/// One edge of the stretch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
}

/// The marking bar, from opening it to Save or closing it.
#[derive(Debug, Clone, PartialEq)]
pub struct Marking {
    pub kind: SkipKind,
    pub step: Step,
    pub start: Option<f64>,
    pub end: Option<f64>,
    /// Whether this edits a mark of the kind the viewer made before.
    pub editing: bool,
    /// Where "Watch the edges" is, while it plays.
    preview: Option<Preview>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Preview {
    /// Playing up to [`PREVIEW_AROUND`] past the start, then on to before the end.
    AroundStart,
    /// Playing up to [`PREVIEW_AROUND`] past the end, then back to where it was.
    AroundEnd,
}

impl Marking {
    /// Open the bar on the Start step, for `kind`.
    pub const fn begin(kind: SkipKind) -> Self {
        Self { kind, step: Step::Start, start: None, end: None, editing: false, preview: None }
    }

    /// Open the bar on the Review step, with `stretch`'s edges: to edit a mark the viewer made,
    /// or to keep a stretch they skipped by hand.
    pub fn review(stretch: &SkipInterval, editing: bool) -> Self {
        Self {
            kind: stretch.kind,
            step: Step::Review,
            start: Some(stretch.start),
            end: Some(stretch.end),
            editing,
            preview: None,
        }
    }

    /// Mark the edge the step names at `position`. The End step moves to the Review, with the
    /// edges swapped if the end came before the start. Nothing on the Review.
    pub fn mark(&mut self, position: f64) {
        match self.step {
            Step::Start => {
                self.start = Some(position.max(0.0));
                self.step = Step::End;
            }
            Step::End => {
                let start = self.start.unwrap_or(0.0);
                let (start, end) =
                    if position < start { (position, start) } else { (start, position) };
                self.start = Some(start.max(0.0));
                self.end = Some(end);
                self.step = Step::Review;
            }
            Step::Review => {}
        }
    }

    /// Move `edge` by `by` seconds, within the file. Only on the Review, where both are marked.
    /// The edges swap if they cross.
    pub fn nudge(&mut self, edge: Edge, by: f64, duration: Option<f64>) {
        if self.step != Step::Review {
            return;
        }
        let (Some(start), Some(end)) = (self.start, self.end) else { return };
        let limit = duration.unwrap_or(f64::MAX);
        let moved = |at: f64| (at + by).clamp(0.0, limit);
        let (start, end) = match edge {
            Edge::Start => (moved(start), end),
            Edge::End => (start, moved(end)),
        };
        let (start, end) = if end < start { (end, start) } else { (start, end) };
        self.start = Some(start);
        self.end = Some(end);
        self.preview = None;
    }

    /// Step back one step: the Review to the End, the End to the Start. `false` from the Start,
    /// which closes the bar. An edit's review closes at once, as there is no step it came from.
    pub fn back(&mut self) -> bool {
        self.preview = None;
        match self.step {
            Step::Review if self.editing => false,
            Step::Review => {
                self.end = None;
                self.step = Step::End;
                true
            }
            Step::End => {
                self.start = None;
                self.step = Step::Start;
                true
            }
            Step::Start => false,
        }
    }

    /// Change the kind being marked; the edges stay.
    pub fn set_kind(&mut self, kind: SkipKind) {
        self.kind = kind;
    }

    /// The stretch as marked, once both edges are.
    pub fn stretch(&self) -> Option<(f64, f64)> {
        Some((self.start?, self.end?))
    }

    /// Its length in seconds, `0` until both edges are marked.
    pub fn length(&self) -> f64 {
        self.stretch().map_or(0.0, |(start, end)| (end - start).max(0.0))
    }

    /// The check, on the Review; [`Check::TooShort`] before it.
    pub fn check(&self, episode_length: f64) -> Check {
        match self.stretch() {
            Some((start, end)) if self.step == Step::Review => {
                check(self.kind, start, end, episode_length)
            }
            _ => Check::TooShort,
        }
    }

    /// Start "Watch the edges": where to seek to first. `None` off the Review.
    pub fn preview(&mut self) -> Option<f64> {
        let (start, _) = self.stretch().filter(|_| self.step == Step::Review)?;
        self.preview = Some(Preview::AroundStart);
        Some((start - PREVIEW_AROUND).max(0.0))
    }

    /// Whether the edges are being watched.
    pub const fn previewing(&self) -> bool {
        self.preview.is_some()
    }

    /// The playhead moved to `position` while the edges are watched: where to seek to next, if
    /// anywhere. Past the start, on to before the end; past the end, the watch is over.
    pub fn previewed(&mut self, position: f64) -> Option<f64> {
        let (start, end) = self.stretch()?;
        match self.preview? {
            Preview::AroundStart if position >= start + PREVIEW_AROUND => {
                self.preview = Some(Preview::AroundEnd);
                let to = (end - PREVIEW_AROUND).max(start + PREVIEW_AROUND);
                (to > position + 0.5).then_some(to)
            }
            Preview::AroundEnd if position >= end + PREVIEW_AROUND => {
                self.preview = None;
                None
            }
            _ => None,
        }
    }
}

/// How long "Was that the opening?" waits after the last seek before asking, in seconds: a held
/// key's seeks are one jump.
pub const JUMP_SETTLES: f64 = 2.0;

/// The jumps forward that look like skipping an opening by hand: of 60 to 150 seconds, landing
/// from within the first six minutes.
const OPENING_JUMP: (f64, f64) = (60.0, 150.0);
const OPENING_BEFORE: f64 = 360.0;

/// Watches the viewer's own seeks for one that skipped an opening by hand, to ask once an
/// episode whether that was it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JumpWatch {
    /// The jump under way: from where, to where, and when its last seek came.
    jump: Option<(f64, f64, f64)>,
    asked: bool,
}

impl JumpWatch {
    /// The viewer sought from `from` to `to` at `now`, in seconds of any clock. A seek soon
    /// after another carries the jump on.
    pub fn sought(&mut self, from: f64, to: f64, now: f64) {
        self.jump = match self.jump {
            Some((origin, _, at)) if now - at <= JUMP_SETTLES => Some((origin, to, now)),
            _ => Some((from, to, now)),
        };
    }

    /// At `now`, whether to ask about the stretch the jump skipped: once it has settled, when it
    /// looks like an opening, and only once per episode. `known` is whether an opening is
    /// known already, which leaves nothing to ask.
    pub fn settled(&mut self, now: f64, known: bool) -> Option<(f64, f64)> {
        let (from, to, at) = self.jump?;
        if now - at < JUMP_SETTLES {
            return None;
        }
        self.jump = None;
        let length = to - from;
        let looks =
            (OPENING_JUMP.0..=OPENING_JUMP.1).contains(&length) && from < OPENING_BEFORE;
        if self.asked || known || !looks {
            return None;
        }
        self.asked = true;
        Some((from, to))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skip::SkipOrigin;

    #[test]
    fn aniskip_takes_longer_stretches_in_longer_episodes() {
        assert_eq!(longest(SkipKind::Opening, 90.0), 40.0);
        assert_eq!(longest(SkipKind::Opening, 600.0), 120.0);
        assert_eq!(longest(SkipKind::Opening, 1440.0), 180.0);
        assert_eq!(longest(SkipKind::Ending, 5400.0), 480.0);
        assert_eq!(longest(SkipKind::Recap, 1440.0), 360.0);
        assert_eq!(longest(SkipKind::Recap, 5400.0), 720.0);
    }

    #[test]
    fn a_stretch_is_checked_against_aniskip_limits() {
        assert_eq!(check(SkipKind::Opening, 10.0, 14.0, 1440.0), Check::TooShort);
        assert_eq!(check(SkipKind::Opening, 10.0, 100.0, 1440.0), Check::Fine);
        assert_eq!(
            check(SkipKind::Opening, 10.0, 200.0, 1440.0),
            Check::TooLong { most: 180.0 }
        );
        assert_eq!(check(SkipKind::Recap, 0.0, 200.0, 1440.0), Check::Fine);
    }

    #[test]
    fn two_presses_mark_a_stretch_and_the_edges_swap_when_crossed() {
        let mut marking = Marking::begin(SkipKind::Opening);
        marking.mark(100.0);
        assert_eq!((marking.step, marking.start), (Step::End, Some(100.0)));
        marking.mark(10.0);
        assert_eq!(marking.step, Step::Review);
        assert_eq!(marking.stretch(), Some((10.0, 100.0)));
        assert_eq!(marking.length(), 90.0);
        marking.mark(50.0);
        assert_eq!(marking.stretch(), Some((10.0, 100.0)), "nothing more to mark");
    }

    #[test]
    fn edges_are_nudged_within_the_file() {
        let mut marking = Marking::begin(SkipKind::Ending);
        marking.nudge(Edge::Start, 1.0, None);
        assert_eq!(marking.start, None, "nothing to nudge before the review");
        marking.mark(1300.0);
        marking.mark(1390.0);
        marking.nudge(Edge::End, 5.0, Some(1392.0));
        assert_eq!(marking.end, Some(1392.0));
        marking.nudge(Edge::Start, -1.0, Some(1392.0));
        assert_eq!(marking.stretch(), Some((1299.0, 1392.0)));
        marking.nudge(Edge::Start, 100.0, Some(1392.0));
        assert_eq!(marking.stretch(), Some((1392.0, 1392.0)), "held at the end");
    }

    #[test]
    fn back_steps_one_step_then_closes() {
        let mut marking = Marking::begin(SkipKind::Recap);
        marking.mark(0.0);
        marking.mark(30.0);
        assert!(marking.back());
        assert_eq!((marking.step, marking.end), (Step::End, None));
        assert!(marking.back());
        assert_eq!((marking.step, marking.start), (Step::Start, None));
        assert!(!marking.back());

        let mine = SkipInterval {
            kind: SkipKind::Opening,
            start: 10.0,
            end: 100.0,
            origin: SkipOrigin::Yours,
        };
        let mut editing = Marking::review(&mine, true);
        assert!(!editing.back(), "an edit has no step before its review");
    }

    #[test]
    fn the_check_waits_for_the_review() {
        let mut marking = Marking::begin(SkipKind::Opening);
        assert_eq!(marking.check(1440.0), Check::TooShort);
        marking.mark(10.0);
        marking.mark(100.0);
        assert_eq!(marking.check(1440.0), Check::Fine);
    }

    #[test]
    fn watching_the_edges_plays_around_each_and_stops() {
        let mut marking = Marking::begin(SkipKind::Opening);
        marking.mark(10.0);
        marking.mark(100.0);
        assert_eq!(marking.preview(), Some(7.0));
        assert!(marking.previewing());
        assert_eq!(marking.previewed(9.0), None);
        assert_eq!(marking.previewed(13.0), Some(97.0));
        assert_eq!(marking.previewed(98.0), None);
        assert_eq!(marking.previewed(103.0), None);
        assert!(!marking.previewing());
    }

    #[test]
    fn the_kind_is_guessed_from_where_the_playhead_is() {
        assert_eq!(guess_kind(30.0, Some(1440.0)), SkipKind::Opening);
        assert_eq!(guess_kind(1300.0, Some(1440.0)), SkipKind::Ending);
        assert_eq!(guess_kind(1300.0, None), SkipKind::Opening);
    }

    #[test]
    fn a_jump_of_an_openings_length_near_the_start_is_asked_about_once() {
        let mut watch = JumpWatch::default();
        // Held right arrow: nine seeks of 10 s, each soon after the last, are one jump.
        for n in 0..9 {
            let from = 30.0 + f64::from(n) * 10.0;
            watch.sought(from, from + 10.0, f64::from(n) * 0.3);
        }
        assert_eq!(watch.settled(3.0, false), None, "still settling");
        assert_eq!(watch.settled(5.0, false), Some((30.0, 120.0)));
        watch.sought(200.0, 290.0, 10.0);
        assert_eq!(watch.settled(20.0, false), None, "once an episode");
    }

    #[test]
    fn other_jumps_are_not_asked_about() {
        let mut short = JumpWatch::default();
        short.sought(30.0, 60.0, 0.0);
        assert_eq!(short.settled(5.0, false), None, "too short");
        let mut late = JumpWatch::default();
        late.sought(400.0, 490.0, 0.0);
        assert_eq!(late.settled(5.0, false), None, "past the first six minutes");
        let mut known = JumpWatch::default();
        known.sought(30.0, 120.0, 0.0);
        assert_eq!(known.settled(5.0, true), None, "an opening is known");
        let mut back = JumpWatch::default();
        back.sought(120.0, 30.0, 0.0);
        assert_eq!(back.settled(5.0, false), None, "backwards");
    }
}
