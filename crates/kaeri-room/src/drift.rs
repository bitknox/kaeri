//! Keeping one player with the room: what the player did that the room has to hear (a seek of
//! the viewer's own), what to ask of the player to stay with the room's position, and whether
//! this device is Behind. Pure and clocked by the caller, in milliseconds, like [`crate::room`].
//!
//! The thresholds are the ones the Syncplay client used, kept: rewind past 4 s ahead,
//! fast-forward past 5 s behind (landing a quarter second on), 0.95× speed past 1.5 s ahead,
//! back to normal within 0.1 s, and a 2 s settle after any seek before correcting again.
//! Behind: more than 3 s behind the room for 4 s; it ends within 1 s of the room for 2 s.

/// A position that moved this much more than playing explains was moved on purpose, seconds.
pub const SEEK_THRESHOLD: f64 = 1.0;
/// A player within this of a place it was told, or of the room's, landed there, seconds.
pub const LANDED: f64 = SEEK_THRESHOLD + 0.5;
/// Ahead of the room by more than this, the player is moved back.
pub const REWIND_THRESHOLD: f64 = 4.0;
/// Behind by more than this, the player is moved on.
pub const FASTFORWARD_THRESHOLD: f64 = 5.0;
/// How far past the room a player that fell behind is moved.
pub const FASTFORWARD_EXTRA: f64 = 0.25;
/// Ahead by more than this, the player slows down until it is back.
pub const SLOWDOWN_THRESHOLD: f64 = 1.5;
/// Within this of the room, a slowed player goes back to normal speed.
pub const SLOWDOWN_RESET: f64 = 0.1;
/// How much slower a player that ran ahead plays.
pub const SLOWDOWN_RATE: f64 = 0.95;
/// No correction this long after a seek, ms.
pub const SETTLE_MS: i64 = 2_000;
/// A seek this side asked for is looked for in the samples this long, ms.
pub const EXPECT_SEEK_MS: i64 = 5_000;
/// How many of the room's last places a player may still report as its own, late: a slow load
/// applies the room's cues long after they were sent, and a player whose frame froze can say an
/// old place again.
pub const TOLD_KEPT: usize = 4;
/// Behind past this many seconds...
pub const BEHIND_SECONDS: f64 = 3.0;
/// ...for this long, ms.
pub const BEHIND_FOR_MS: i64 = 4_000;
/// Behind ends within this many seconds of the room...
pub const CAUGHT_UP_SECONDS: f64 = 1.0;
/// ...for this long, ms.
pub const CAUGHT_UP_FOR_MS: i64 = 2_000;

/// What the drift asks for after a sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    /// The viewer moved the player to here: a seek for the room.
    ViewerSeek(f64),
    /// Move the player to here, to stay with the room.
    Seek(f64),
    /// Play at this speed.
    Speed(f64),
    /// This device is Behind now (`true`), or caught up.
    Behind(bool),
}

#[derive(Debug, Clone, Copy)]
struct Last {
    pos: f64,
    paused: bool,
    speed: f64,
    at: i64,
}

/// One player kept with one room.
#[derive(Debug, Default)]
pub struct Drift {
    last: Option<Last>,
    /// A place the room told the player to go to, until when, and where the player was then.
    expect_seek: Option<(f64, i64, Option<f64>)>,
    /// The room's last places for this file, newest last: a player reporting one of them is
    /// late or stale, never the viewer.
    told: Vec<f64>,
    settle_until: i64,
    slowed: bool,
    behind: bool,
    behind_since: Option<i64>,
    near_since: Option<i64>,
}

impl Drift {
    /// A new file, or a new room: start over.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn behind(&self) -> bool {
        self.behind
    }

    /// The room told the player to seek to `to`: the jump that follows is the room's.
    pub fn cued_seek(&mut self, to: f64, now: i64) {
        self.placed(to);
        let from = self.last.as_ref().map(|l| l.pos);
        self.expect_seek = Some((to, now + EXPECT_SEEK_MS, from));
        self.settle_until = now + SETTLE_MS;
    }

    /// The room was at `pos` for this file: its playhead landed there, or the player was opened
    /// there.
    pub fn placed(&mut self, pos: f64) {
        self.told.retain(|&p| (p - pos).abs() > LANDED);
        self.told.push(pos);
        if self.told.len() > TOLD_KEPT {
            self.told.remove(0);
        }
    }

    /// A sample from the player, with the room's position and whether the room plays now.
    pub fn sample(
        &mut self,
        pos: f64,
        paused: bool,
        speed: f64,
        now: i64,
        room: Option<(f64, bool)>,
    ) -> Vec<Act> {
        let mut out = Vec::new();
        let last = self.last.replace(Last { pos, paused, speed, at: now });
        if let Some((_, until, _)) = self.expect_seek
            && now >= until
        {
            self.expect_seek = None;
        }
        // A jump playing doesn't explain: the room's own seek, or the viewer's.
        if let Some(last) = last {
            let played =
                if last.paused { 0.0 } else { (now - last.at) as f64 / 1000.0 * last.speed };
            let jumped = (pos - (last.pos + played)).abs() > SEEK_THRESHOLD;
            if jumped {
                self.settle_until = now + SETTLE_MS;
                // Never the viewer's: a place the room told the player to go to, or a stale
                // report of where it was before (both looked for the whole window, as a player
                // can report late and out of order), or the room's own place, where a seek would
                // move nothing (a late report of a cue, or the player opening at the room's place).
                let told = self.expect_seek.is_some_and(|(to, _, from)| {
                    (pos - to).abs() <= LANDED
                        || from.is_some_and(|f| (pos - f).abs() <= LANDED)
                }) || self.told.iter().any(|&p| (pos - p).abs() <= LANDED);
                let rooms = room.is_some_and(|(room_pos, _)| (pos - room_pos).abs() <= LANDED);
                if !told && !rooms {
                    out.push(Act::ViewerSeek(pos));
                    return out;
                }
            }
        }
        let Some((room_pos, room_playing)) = room else { return out };
        let ahead = pos - room_pos;
        // Behind, judged whatever the speed, while the room plays.
        if room_playing && !paused {
            if ahead < -BEHIND_SECONDS {
                self.near_since = None;
                let since = *self.behind_since.get_or_insert(now);
                if !self.behind && now - since >= BEHIND_FOR_MS {
                    self.behind = true;
                    out.push(Act::Behind(true));
                }
            } else {
                self.behind_since = None;
                if ahead.abs() <= CAUGHT_UP_SECONDS {
                    let since = *self.near_since.get_or_insert(now);
                    if self.behind && now - since >= CAUGHT_UP_FOR_MS {
                        self.behind = false;
                        out.push(Act::Behind(false));
                    }
                } else {
                    self.near_since = None;
                }
            }
        }
        if !room_playing || paused || now < self.settle_until || self.expect_seek.is_some() {
            return out;
        }
        if ahead > REWIND_THRESHOLD {
            self.cued_seek(room_pos, now);
            out.push(Act::Seek(room_pos));
        } else if ahead < -FASTFORWARD_THRESHOLD {
            let to = room_pos + FASTFORWARD_EXTRA;
            self.cued_seek(to, now);
            out.push(Act::Seek(to));
        } else if ahead > SLOWDOWN_THRESHOLD && !self.slowed {
            self.slowed = true;
            out.push(Act::Speed(SLOWDOWN_RATE));
        } else if self.slowed && ahead.abs() <= SLOWDOWN_RESET {
            self.slowed = false;
            out.push(Act::Speed(1.0));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Samples once a second from `from` ms, the player at `pos(t)` and the room at `room(t)`.
    fn run(
        drift: &mut Drift,
        from: i64,
        to: i64,
        pos: impl Fn(i64) -> f64,
        room: impl Fn(i64) -> f64,
    ) -> Vec<(i64, Act)> {
        let mut out = Vec::new();
        let mut t = from;
        while t <= to {
            for act in drift.sample(pos(t), false, 1.0, t, Some((room(t), true))) {
                out.push((t, act));
            }
            t += 1_000;
        }
        out
    }

    #[test]
    fn a_player_with_the_room_is_left_alone() {
        let mut drift = Drift::default();
        let acts = run(
            &mut drift,
            0,
            20_000,
            |t| 10.0 + t as f64 / 1000.0,
            |t| 10.2 + t as f64 / 1000.0,
        );
        assert!(acts.is_empty(), "{acts:?}");
    }

    #[test]
    fn the_viewers_own_seek_is_heard_and_the_rooms_is_not() {
        let mut drift = Drift::default();
        assert!(drift.sample(10.0, false, 1.0, 0, None).is_empty());
        assert_eq!(drift.sample(60.0, false, 1.0, 1_000, None), vec![Act::ViewerSeek(60.0)]);
        // The room's seek, cued here, isn't the viewer's.
        drift.cued_seek(120.0, 2_000);
        assert!(drift.sample(120.2, false, 1.0, 2_500, None).is_empty());
        // Paused, the position doesn't move: no seek.
        assert!(drift.sample(120.2, true, 1.0, 3_500, None).is_empty());
        assert!(drift.sample(120.2, true, 1.0, 6_500, None).is_empty());
    }

    #[test]
    fn a_told_place_reported_late_or_twice_is_never_the_viewers_seek() {
        let mut drift = Drift::default();
        let paused_at = |pos| Some((pos, false));
        // Paused for Ren at 0:12; the room seeks to 0:35. The player reports a stale 0:12 between
        // its reports of 0:35, the last of them long after the cue.
        assert!(drift.sample(12.0, true, 1.0, 0, paused_at(12.0)).is_empty());
        drift.cued_seek(35.0, 100);
        for (pos, at) in [(12.0, 600), (35.0, 1_500), (12.0, 2_000), (35.0, 3_000)] {
            assert!(
                drift.sample(pos, true, 1.0, at, paused_at(35.0)).is_empty(),
                "{pos} at {at}"
            );
        }
        // Past the window, going back to 0:12 is the viewer's own seek, and the room's place
        // after it is not.
        assert_eq!(
            drift.sample(12.0, true, 1.0, 9_000, paused_at(35.0)),
            vec![Act::ViewerSeek(12.0)]
        );
        assert!(drift.sample(35.0, true, 1.0, 9_500, paused_at(35.0)).is_empty());
        // A player opening at the room's place (0:00, then 0:12.3) moved nothing.
        let mut drift = Drift::default();
        assert!(drift.sample(0.0, true, 1.0, 0, paused_at(12.3)).is_empty());
        assert!(drift.sample(12.3, true, 1.0, 400, paused_at(12.3)).is_empty());
        // The viewer's own seek away from the room's place still counts.
        assert_eq!(
            drift.sample(40.0, true, 1.0, 900, paused_at(12.3)),
            vec![Act::ViewerSeek(40.0)]
        );
    }

    #[test]
    fn the_rooms_old_places_reported_after_the_window_are_never_the_viewers_seek() {
        // After play: the room was written at 0:12 and plays on. The player opens slowly, past
        // the cue's window: its file start, then the place it was opened at, then a frozen frame
        // saying 0:12 again.
        let mut drift = Drift::default();
        let playing_from_12 = |at: i64| Some((12.0 + at as f64 / 1000.0, true));
        drift.placed(12.0);
        drift.cued_seek(12.1, 100);
        for (pos, at) in [(0.0, 6_500), (12.0, 6_700), (19.0, 7_000), (12.0, 7_500)] {
            let acts = drift.sample(pos, false, 1.0, at, playing_from_12(at));
            assert!(
                !acts.iter().any(|a| matches!(a, Act::ViewerSeek(_))),
                "{pos} at {at}: {acts:?}"
            );
        }
        // Waiting: paused at 0:12, Mika sought to 0:35. Long after, the player says 0:12 again,
        // then 0:35 where it is.
        let mut drift = Drift::default();
        let paused_at = |pos| Some((pos, false));
        drift.placed(12.0);
        drift.placed(35.0);
        drift.cued_seek(35.0, 0);
        for (pos, at) in [(35.0, 1_000), (35.0, 7_000), (12.0, 8_000), (35.0, 8_500)] {
            assert!(
                drift.sample(pos, true, 1.0, at, paused_at(35.0)).is_empty(),
                "{pos} at {at}"
            );
        }
        // The viewer's own seek somewhere new still counts.
        assert_eq!(
            drift.sample(50.0, true, 1.0, 9_000, paused_at(35.0)),
            vec![Act::ViewerSeek(50.0)]
        );
    }

    #[test]
    fn only_the_last_few_places_are_kept() {
        let mut drift = Drift::default();
        for pos in [10.0, 20.0, 30.0, 40.0, 50.0, 20.0] {
            drift.placed(pos);
        }
        assert_eq!(drift.told, vec![30.0, 40.0, 50.0, 20.0]);
    }

    #[test]
    fn far_behind_moves_on_and_reads_behind_until_caught_up() {
        let mut drift = Drift::default();
        // 6 s behind from the start: moved on once (after the settle), and Behind after 4 s.
        let acts =
            run(&mut drift, 0, 5_000, |t| t as f64 / 1000.0, |t| 6.0 + t as f64 / 1000.0);
        assert!(acts.contains(&(0, Act::Seek(6.25))), "{acts:?}");
        let mut drift = Drift::default();
        // 3.5 s behind: no seek (under 5 s), but Behind after 4 s.
        let acts =
            run(&mut drift, 0, 6_000, |t| t as f64 / 1000.0, |t| 3.5 + t as f64 / 1000.0);
        assert_eq!(acts, vec![(4_000, Act::Behind(true))]);
        // Caught up within 1 s for 2 s: Behind ends.
        let acts = run(
            &mut drift,
            7_000,
            11_000,
            |t| 0.5 + t as f64 / 1000.0,
            |t| 1.0 + t as f64 / 1000.0,
        );
        assert_eq!(acts, vec![(9_000, Act::Behind(false))]);
    }

    #[test]
    fn ahead_slows_down_and_far_ahead_moves_back() {
        let mut drift = Drift::default();
        let acts =
            run(&mut drift, 0, 3_000, |t| 2.0 + t as f64 / 1000.0, |t| t as f64 / 1000.0);
        assert_eq!(acts.first(), Some(&(0, Act::Speed(SLOWDOWN_RATE))));
        let mut drift = Drift::default();
        let acts =
            run(&mut drift, 0, 3_000, |t| 4.5 + t as f64 / 1000.0, |t| t as f64 / 1000.0);
        assert_eq!(acts.first(), Some(&(0, Act::Seek(0.0))));
    }

    #[test]
    fn nothing_is_corrected_while_the_room_waits_or_the_player_is_paused() {
        let mut drift = Drift::default();
        assert!(drift.sample(10.0, false, 1.0, 0, Some((30.0, false))).is_empty());
        assert!(drift.sample(10.0, true, 1.0, 1_000, Some((30.0, true))).is_empty());
    }
}
