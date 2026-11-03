//! Found skip times: the opening and the ending from the audio, where nothing else has them.
//!
//! Each episode's first six minutes (the head) and last four (the tail) are fingerprinted with
//! chromaprint as the player's tap hands the audio over, ahead of playback. An opening is the
//! stretch of 30 to 180 s that this episode's head shares with an earlier episode's head; an
//! ending is 30 to 240 s shared between tails. The matcher follows Intro Skipper's: candidate
//! shifts from equal items, the longest run of close items per shift, then the edges trimmed and
//! placed with constants fitted on generated episodes, and pulled 1 s inward at each end so a
//! skip never takes story.
//!
//! Pure: the audio comes in as samples, the prints go out as items, and storing them and asking
//! for the audio are the engine's.

use rusty_chromaprint::{Configuration, Fingerprinter};

use crate::skip::{SkipInterval, SkipKind, SkipOrigin};

/// The rate the tap hands samples over at, mono.
pub const RATE: u32 = 11025;
/// How much of an episode's start is fingerprinted, in seconds: a cold open of up to four and a
/// half minutes and a 90 s opening.
pub const HEAD_SECS: f64 = 360.0;
/// How much of an episode's end is fingerprinted, in seconds: room for a scene and a preview
/// after the ending.
pub const TAIL_SECS: f64 = 240.0;

/// A matching item: the mean bit difference over [`SMOOTH`] items around it is at most this.
const MAX_BITS: f64 = 9.0;
const SMOOTH: usize = 9;
/// Run edges are trimmed to where five items in a row average this many bits or fewer.
const REFINE: f64 = 6.0;
/// Where a run's first item puts the start, in seconds after it.
const START_BIAS: f64 = 0.4;
/// Where a run's last item puts the end, as a share of an item's reach.
const END_SPAN: f64 = 0.62;
/// How far found edges are pulled inward, in seconds: a start too early or an end too late skips
/// story, and a second of opening left playing does not.
const MARGIN: f64 = 1.0;
/// Gaps between close items bridged within one run, in seconds.
const BRIDGE: f64 = 3.5;
/// At most this many candidate shifts are tried, each with its neighbours.
const SHIFTS: usize = 8;
/// A sample run continues the last one when it starts within this of where that one ended, in
/// seconds; anything further is a new run with its own fingerprinter.
const JOIN: f64 = 0.2;

fn config() -> Configuration {
    Configuration::preset_test2()
}

/// Seconds between one item and the next.
pub fn step() -> f64 {
    f64::from(config().item_duration_in_seconds())
}

/// Seconds of audio one item reaches past its own start: the frame and the classifiers' span.
fn span() -> f64 {
    config().delay() as f64 / f64::from(RATE)
}

/// Which end of an episode a print covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    Head,
    Tail,
}

impl Part {
    pub const ALL: [Self; 2] = [Self::Head, Self::Tail];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Tail => "tail",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "head" => Some(Self::Head),
            "tail" => Some(Self::Tail),
            _ => None,
        }
    }

    /// The kind this part finds.
    pub const fn kind(self) -> SkipKind {
        match self {
            Self::Head => SkipKind::Opening,
            Self::Tail => SkipKind::Ending,
        }
    }

    /// The part that finds `kind`, for the two kinds a part finds.
    pub const fn of(kind: SkipKind) -> Option<Self> {
        match kind {
            SkipKind::Opening => Some(Self::Head),
            SkipKind::Ending => Some(Self::Tail),
            SkipKind::Recap => None,
        }
    }

    /// The stretch of a file `length` seconds long this part covers. The tail needs the length.
    pub fn window(self, length: Option<f64>) -> Option<(f64, f64)> {
        match self {
            Self::Head => Some((0.0, length.map_or(HEAD_SECS, |l| l.min(HEAD_SECS)))),
            Self::Tail => length.map(|l| ((l - TAIL_SECS).max(0.0), l)),
        }
    }

    /// The tap's windows option for a file `length` seconds long, or the head alone before the
    /// length is known: `0-360` or `0-360,1180-1420`.
    pub fn tap_windows(length: Option<f64>) -> String {
        Self::ALL
            .iter()
            .filter_map(|part| part.window(length))
            .map(|(from, to)| format!("{}-{}", from.floor() as u64, to.ceil() as u64))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The shortest and longest stretch this part's kind may be, in seconds: AniSkip's own caps
    /// for a full episode. Under 30 s, a recap of the last episode's cold open matches too.
    const fn bounds(self) -> (f64, f64) {
        match self {
            Self::Head => (30.0, 180.0),
            Self::Tail => (30.0, 240.0),
        }
    }
}

/// One part's fingerprint: item `i` covers the audio from `start + i * step()`. An item of 0 is
/// audio not heard, as when playback jumped past it; a real item of 0 is stored as 1, a one-bit
/// difference the matcher does not notice.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Print {
    /// Seconds into the episode of item 0.
    pub start: f64,
    pub items: Vec<u32>,
}

impl Print {
    pub const fn new(start: f64) -> Self {
        Self { start, items: Vec::new() }
    }

    /// The print as stored: its items, little-endian.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.items.iter().flat_map(|item| item.to_le_bytes()).collect()
    }

    /// A stored print starting `start` seconds in.
    pub fn from_bytes(start: f64, bytes: &[u8]) -> Self {
        let items = bytes.as_chunks::<4>().0.iter().map(|b| u32::from_le_bytes(*b)).collect();
        Self { start, items }
    }

    /// Seconds of audio the print holds, gaps left out.
    pub fn heard(&self) -> f64 {
        self.items.iter().filter(|item| **item != 0).count() as f64 * step()
    }

    /// Items of a run that began `at` seconds in, from its `offset`th item on. Where the print
    /// already has an item, the first one heard stays.
    fn place(&mut self, at: f64, offset: usize, items: &[u32]) {
        let first = ((at - self.start) / step()).round();
        if first < 0.0 {
            // Before the part: only the items from the part's start on.
            let skip = (-first) as usize;
            if offset + items.len() <= skip {
                return;
            }
        }
        for (k, item) in items.iter().enumerate() {
            let index = first as i64 + (offset + k) as i64;
            let Ok(index) = usize::try_from(index) else { continue };
            if index >= self.items.len() {
                self.items.resize(index + 1, 0);
            }
            if self.items[index] == 0 {
                self.items[index] = (*item).max(1);
            }
        }
    }
}

/// One stretch of continuous audio, fingerprinted as it comes.
struct Run {
    part: Part,
    /// Seconds into the episode of its first sample.
    start: f64,
    /// Where the next sample continues it.
    next: f64,
    printer: Fingerprinter,
    /// Items already placed in the part's print.
    placed: usize,
}

/// One file's capture: the audio the tap hands over, placed into the head's and the tail's prints
/// by its timestamps. Not `Send`, as the fingerprinter is not: it lives on the thread it is
/// made on.
pub struct Capture {
    length: Option<f64>,
    head: Print,
    tail: Option<Print>,
    run: Option<Run>,
}

impl Default for Capture {
    fn default() -> Self {
        Self::new()
    }
}

impl Capture {
    pub const fn new() -> Self {
        Self { length: None, head: Print::new(0.0), tail: None, run: None }
    }

    /// The file's length is known, which places the tail.
    pub fn set_length(&mut self, length: f64) {
        if self.length.is_some() || length <= 0.0 {
            return;
        }
        self.length = Some(length);
        if let Some((start, _)) = Part::Tail.window(Some(length)) {
            self.tail = Some(Print::new(start));
        }
    }

    pub const fn length(&self) -> Option<f64> {
        self.length
    }

    /// The part a sample `pts` seconds in belongs to, if any.
    fn part_at(&self, pts: f64) -> Option<Part> {
        if let Some(tail) = &self.tail
            && pts >= tail.start - JOIN
        {
            return Some(Part::Tail);
        }
        (pts < HEAD_SECS).then_some(Part::Head)
    }

    fn print_mut(&mut self, part: Part) -> &mut Print {
        match part {
            Part::Head => &mut self.head,
            Part::Tail => self.tail.get_or_insert_with(|| Print::new(HEAD_SECS)),
        }
    }

    /// Samples at [`RATE`], mono, the first `pts` seconds into the episode.
    pub fn samples(&mut self, pts: f64, samples: &[i16]) {
        if samples.is_empty() {
            self.brk();
            return;
        }
        let Some(part) = self.part_at(pts) else {
            self.brk();
            return;
        };
        let continues = self
            .run
            .as_ref()
            .is_some_and(|run| run.part == part && (pts - run.next).abs() <= JOIN);
        if !continues {
            self.brk();
            let mut printer = Fingerprinter::new(&config());
            if printer.start(RATE, 1).is_err() {
                return;
            }
            self.run = Some(Run { part, start: pts, next: pts, printer, placed: 0 });
        }
        if let Some(run) = self.run.as_mut() {
            run.printer.consume(samples);
            run.next = pts + samples.len() as f64 / f64::from(RATE);
        }
        self.place(false);
    }

    /// The audio does not go on from here: a seek, dropped audio, the end of a window.
    pub fn brk(&mut self) {
        self.place(true);
        self.run = None;
    }

    /// The run's items so far into its print, and all of them when it closes.
    fn place(&mut self, closing: bool) {
        let Some(mut run) = self.run.take() else { return };
        if closing {
            run.printer.finish();
        }
        let items = run.printer.fingerprint();
        if items.len() > run.placed {
            let fresh = items[run.placed..].to_vec();
            let (start, placed) = (run.start, run.placed);
            self.print_mut(run.part).place(start, placed, &fresh);
            run.placed += fresh.len();
        }
        if !closing {
            self.run = Some(run);
        }
    }

    /// The print of `part` as it stands.
    pub fn print(&self, part: Part) -> Option<&Print> {
        match part {
            Part::Head => Some(&self.head),
            Part::Tail => self.tail.as_ref(),
        }
    }
}

/// A stretch this episode shares with another.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Found {
    pub kind: SkipKind,
    /// In this episode, in seconds, already pulled inward.
    pub start: f64,
    pub end: f64,
    /// Where the same stretch is in the other episode, pulled inward the same way.
    pub other_start: f64,
    pub other_end: f64,
}

impl Found {
    pub const fn interval(&self) -> SkipInterval {
        SkipInterval {
            kind: self.kind,
            start: self.start,
            end: self.end,
            origin: SkipOrigin::Found,
        }
    }
}

/// The stretch of `part` that `this` episode's print shares with `other`'s, if one fits the
/// part's kind. Until `settled`, `this` is still coming in, so a stretch that runs to its last
/// item may yet grow and is not reported.
pub fn find(part: Part, this: &Print, other: &Print, settled: bool) -> Option<Found> {
    let (min, max) = part.bounds();
    let run = longest_run(&this.items, &other.items, min, max)?;
    let step = step();
    let bridge = (BRIDGE / step).round() as usize;
    // Audio not heard next to an edge: the stretch may go on into it, so its edge is not known.
    // Within a bridge of an edge, as trimming may have moved the edge off the gap itself.
    let gap_before =
        |items: &[u32], at: usize| items[at.saturating_sub(bridge)..at].contains(&0);
    let gap_after = |items: &[u32], at: usize| {
        items
            .get(at + 1..(at + 1 + bridge).min(items.len()))
            .is_some_and(|near| near.contains(&0))
    };
    let (b0, b1) = ((run.a0 as i64 + run.shift) as usize, (run.a1 as i64 + run.shift) as usize);
    if gap_before(&this.items, run.a0)
        || gap_before(&other.items, b0)
        || gap_after(&this.items, run.a1)
        || gap_after(&other.items, b1)
    {
        return None;
    }
    if !settled && run.a1 + bridge + SMOOTH >= this.items.len() {
        return None;
    }
    let at = |print: &Print, first: usize, last: usize| {
        (
            print.start + first as f64 * step + START_BIAS + MARGIN,
            print.start + last as f64 * step + span() * END_SPAN - MARGIN,
        )
    };
    let (start, end) = at(this, run.a0, run.a1);
    let (other_start, other_end) = at(other, b0, b1);
    (end > start).then_some(Found { kind: part.kind(), start, end, other_start, other_end })
}

/// The longest run of close items `a` and `b` share, in `a`'s items, with `b` at `a + shift`.
#[derive(Debug, Clone, Copy)]
struct Shared {
    a0: usize,
    a1: usize,
    shift: i64,
}

fn longest_run(a: &[u32], b: &[u32], min: f64, max: f64) -> Option<Shared> {
    let step = step();
    let mut index: std::collections::HashMap<u32, Vec<usize>> =
        std::collections::HashMap::new();
    for (j, item) in b.iter().enumerate() {
        if *item != 0 {
            index.entry(*item).or_default().push(j);
        }
    }
    let mut votes: std::collections::HashMap<i64, u32> = std::collections::HashMap::new();
    for (i, item) in a.iter().enumerate() {
        if *item == 0 {
            continue;
        }
        if let Some(js) = index.get(item) {
            for &j in js {
                *votes.entry(j as i64 - i as i64).or_default() += 1;
            }
        }
    }
    let mut shifts: Vec<(u32, i64)> = votes.into_iter().map(|(s, c)| (c, s)).collect();
    shifts.sort_unstable_by(|x, y| y.cmp(x));
    let mut picked: Vec<i64> = Vec::new();
    for (count, shift) in shifts {
        if count < 2 || picked.len() >= SHIFTS {
            break;
        }
        if picked.iter().all(|p| (p - shift).abs() > 3) {
            picked.push(shift);
        }
    }

    let gap = (BRIDGE / step).round() as usize;
    let mut best: Option<(usize, Shared)> = None;
    for base in picked {
        // The neighbours too: a vote can land one item off.
        for shift in base - 1..=base + 1 {
            let lo = 0.max(-shift) as usize;
            let hi = (a.len() as i64).min(b.len() as i64 - shift).max(0) as usize;
            if hi <= lo {
                continue;
            }
            let bits: Vec<f64> = (lo..hi)
                .map(|i| {
                    let (x, y) = (a[i], b[(i as i64 + shift) as usize]);
                    // Audio not heard on either side never matches.
                    if x == 0 || y == 0 { 32.0 } else { f64::from((x ^ y).count_ones()) }
                })
                .collect();
            let half = SMOOTH / 2;
            let ok: Vec<bool> = (0..bits.len())
                .map(|k| {
                    let s = k.saturating_sub(half);
                    let e = (k + half + 1).min(bits.len());
                    bits[s..e].iter().sum::<f64>() / (e - s) as f64 <= MAX_BITS
                })
                .collect();
            let close = |i: usize| {
                let e = (i + 5).min(bits.len());
                bits[i..e].iter().sum::<f64>() / e.saturating_sub(i).max(1) as f64 <= REFINE
            };
            let mut k = 0;
            while k < ok.len() {
                if !ok[k] {
                    k += 1;
                    continue;
                }
                let start = k;
                let mut end = k;
                let mut j = k;
                while j < ok.len() {
                    if ok[j] {
                        end = j;
                        j += 1;
                    } else if j - end <= gap {
                        j += 1;
                    } else {
                        break;
                    }
                }
                let len = end + 1 - start;
                let secs = (end - start) as f64 * step + span();
                if secs >= min && secs <= max && best.as_ref().is_none_or(|(l, _)| len > *l) {
                    // Trim what a bridged gap or a chance match stretched: the run starts and
                    // ends where five items in a row are close on average.
                    let mut s0 = start;
                    while s0 + 5 < end && !close(s0) {
                        s0 += 1;
                    }
                    let mut e0 = end;
                    while e0 > s0 + 5 && !close(e0 - 4) {
                        e0 -= 1;
                    }
                    best = Some((len, Shared { a0: lo + s0, a1: lo + e0, shift }));
                }
                k = end + 1;
            }
        }
    }
    best.map(|(_, shared)| shared)
}

/// Generated audio for tests here and in the engine, not for anything that plays: episodes of
/// tones whose notes change four times a second, from a seed, so two stretches with the same
/// seed sound the same and any two others do not.
#[doc(hidden)]
pub mod fixtures {

    use super::RATE;

    /// `secs` seconds of the piece `seed` names, from `from` seconds into it.
    pub fn piece(seed: u64, from: f64, secs: f64) -> Vec<i16> {
        let n = (secs * f64::from(RATE)) as usize;
        let first = (from * f64::from(RATE)) as usize;
        let note_len = RATE as usize / 4;
        let mut out = Vec::with_capacity(n);
        let mut note = usize::MAX;
        let mut freqs = [0.0f64; 3];
        for k in first..first + n {
            if k / note_len != note {
                note = k / note_len;
                let mut x =
                    seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(note as u64);
                for f in &mut freqs {
                    x = x
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    *f = 110.0 * 2f64.powf(((x >> 33) % 48) as f64 / 12.0);
                }
            }
            let t = k as f64 / f64::from(RATE);
            let v: f64 = freqs.iter().map(|f| (std::f64::consts::TAU * f * t).sin()).sum();
            out.push((v / 3.0 * 9000.0) as i16);
        }
        out
    }

    /// An episode of `parts`, each a seed and its length in seconds.
    pub fn episode(parts: &[(u64, f64)]) -> Vec<i16> {
        parts.iter().flat_map(|(seed, secs)| piece(*seed, 0.0, *secs)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::episode;
    use super::*;

    const OPENING: u64 = 7;
    const ENDING: u64 = 8;

    /// The head of an episode fed through a capture in chunks the size a tap hands over.
    fn head(samples: &[i16], from: f64, upto: f64) -> Capture {
        let mut capture = Capture::new();
        feed(&mut capture, samples, from, upto);
        capture.brk();
        capture
    }

    fn feed(capture: &mut Capture, samples: &[i16], from: f64, upto: f64) {
        let rate = f64::from(RATE);
        let (a, b) = ((from * rate) as usize, ((upto * rate) as usize).min(samples.len()));
        for (k, chunk) in samples[a..b].chunks(4096).enumerate() {
            capture.samples(from + (k * 4096) as f64 / rate, chunk);
        }
    }

    fn near(found: f64, truth: f64, inward: bool) {
        // Inward by the margin, give or take the matcher's own error.
        let want = if inward { truth + MARGIN } else { truth - MARGIN };
        assert!((found - want).abs() <= 1.5, "found {found:.2}, want about {want:.2}");
    }

    #[test]
    fn finds_the_opening_two_heads_share_wherever_it_starts() {
        let one = episode(&[(101, 20.0), (OPENING, 90.0), (102, 250.0)]);
        let two = episode(&[(201, 65.0), (OPENING, 90.0), (202, 205.0)]);
        let (one, two) = (head(&one, 0.0, 360.0), head(&two, 0.0, 360.0));
        let found = find(
            Part::Head,
            two.print(Part::Head).unwrap(),
            one.print(Part::Head).unwrap(),
            true,
        )
        .expect("found");
        assert_eq!(found.kind, SkipKind::Opening);
        near(found.start, 65.0, true);
        near(found.end, 155.0, false);
        near(found.other_start, 20.0, true);
        near(found.other_end, 110.0, false);
        assert_eq!(found.interval().origin, SkipOrigin::Found);
    }

    #[test]
    fn nothing_shared_or_too_short_finds_nothing() {
        let one = head(&episode(&[(1, 30.0), (OPENING, 20.0), (2, 310.0)]), 0.0, 360.0);
        let two = head(&episode(&[(3, 50.0), (OPENING, 20.0), (4, 290.0)]), 0.0, 360.0);
        let three = head(&episode(&[(5, 360.0)]), 0.0, 360.0);
        let print = |c: &Capture| c.print(Part::Head).unwrap().clone();
        assert_eq!(find(Part::Head, &print(&two), &print(&one), true), None, "20 s is a recap");
        assert_eq!(find(Part::Head, &print(&three), &print(&one), true), None);
    }

    #[test]
    fn a_stretch_still_coming_in_waits_for_its_end() {
        let one = head(&episode(&[(11, 10.0), (OPENING, 90.0), (12, 260.0)]), 0.0, 360.0);
        let two = episode(&[(21, 30.0), (OPENING, 90.0), (22, 240.0)]);
        let mut capture = Capture::new();
        feed(&mut capture, &two, 0.0, 100.0);
        let other = one.print(Part::Head).unwrap();
        assert_eq!(find(Part::Head, capture.print(Part::Head).unwrap(), other, false), None);
        feed(&mut capture, &two, 100.0, 140.0);
        let found =
            find(Part::Head, capture.print(Part::Head).unwrap(), other, false).expect("found");
        near(found.start, 30.0, true);
        near(found.end, 120.0, false);
    }

    #[test]
    fn audio_joined_across_a_jump_lines_up_and_a_gap_at_an_edge_finds_nothing() {
        let one = head(&episode(&[(31, 40.0), (OPENING, 90.0), (32, 230.0)]), 0.0, 360.0);
        let two = episode(&[(41, 40.0), (OPENING, 90.0), (42, 230.0)]);
        let other = one.print(Part::Head).unwrap();
        // Heard out of order: from 200 s on first, then the start.
        let mut capture = Capture::new();
        feed(&mut capture, &two, 200.0, 360.0);
        capture.brk();
        feed(&mut capture, &two, 0.0, 200.0);
        capture.brk();
        let found = find(Part::Head, capture.print(Part::Head).unwrap(), other, true).unwrap();
        near(found.start, 40.0, true);
        near(found.end, 130.0, false);
        // Joined into the opening: where it starts was not heard.
        let mut capture = Capture::new();
        feed(&mut capture, &two, 70.0, 360.0);
        capture.brk();
        assert_eq!(find(Part::Head, capture.print(Part::Head).unwrap(), other, true), None);
    }

    #[test]
    fn finds_the_ending_two_tails_share() {
        // The last four minutes of two episodes of different lengths, the ending 30 s in.
        let tail =
            |pre: u64, after: f64| episode(&[(pre, 30.0), (ENDING, 90.0), (pre + 1, after)]);
        let capture = |samples: &[i16], length: f64| {
            let mut capture = Capture::new();
            capture.set_length(length);
            let rate = f64::from(RATE);
            for (k, chunk) in samples.chunks(4096).enumerate() {
                capture.samples(length - 240.0 + (k * 4096) as f64 / rate, chunk);
            }
            capture.brk();
            capture
        };
        let one = capture(&tail(51, 120.0), 1440.0);
        let two = capture(&tail(61, 120.0), 1425.0);
        let found = find(
            Part::Tail,
            two.print(Part::Tail).unwrap(),
            one.print(Part::Tail).unwrap(),
            true,
        )
        .expect("found");
        assert_eq!(found.kind, SkipKind::Ending);
        near(found.start, 1215.0, true);
        near(found.end, 1305.0, false);
        near(found.other_start, 1230.0, true);
    }

    #[test]
    fn a_print_survives_its_stored_bytes() {
        let print = Print { start: 1200.0, items: vec![0, 1, u32::MAX, 0x1234_5678] };
        assert_eq!(Print::from_bytes(1200.0, &print.to_bytes()), print);
    }

    #[test]
    fn the_tap_windows_cover_the_head_then_the_tail() {
        assert_eq!(Part::tap_windows(None), "0-360");
        assert_eq!(Part::tap_windows(Some(1420.0)), "0-360,1180-1420");
        assert_eq!(Part::tap_windows(Some(300.5)), "0-301,60-301");
        assert_eq!(Part::of(SkipKind::Recap), None);
    }
}
