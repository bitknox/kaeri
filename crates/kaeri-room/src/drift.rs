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
    expect_seek: Option<(f64, i64)>,
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
        self.expect_seek = Some((to, now + EXPECT_SEEK_MS));
        self.settle_until = now + SETTLE_MS;
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
        if let Some((_, until)) = self.expect_seek
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
                match self.expect_seek {
                    Some((to, _)) if (pos - to).abs() <= SEEK_THRESHOLD + 0.5 => {
                        self.expect_seek = None;
                    }
                    _ => {
                        out.push(Act::ViewerSeek(pos));
                        return out;
                    }
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
