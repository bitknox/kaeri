//! The still clock: the time a fixture scene's made-up waits run on in a screenshot, held at the
//! moment the shot names. A source the scene has answer 4.5 s in has answered in a shot held at
//! 5 s, at once, and has not in one held at 3 s, however slow the machine taking it, so every run
//! of the shot shows the same and none waits for the moment on the wall clock.
//!
//! The clock is off in every app, in every recording and in every test that does not turn it on:
//! [`wait`] is then the wall clock's sleep. A Fixtures build turns it on for a shot with
//! [`hold_from_start`] (`KAERI_STILL_AT` when a scene starts, or
//! `testing_still_clock_hold_seconds` in the bindings for a launch argument), an app's replay of
//! made-up events steps it with [`hold`] (`testing_still_clock_hold`), and the apps' own timing of
//! a shot, such as a starting screen's seconds, reads [`now`].
//!
//! Held, the clock moves by itself from the moment one wait ends to the moment the next one does,
//! resting [`SETTLE`] of real time on each so the work a wait let go has run before the next, and
//! stops at the hold. A wait that would end past the hold never ends until the hold moves on.
//!
//! When the clock goes on to the hold itself depends on how it was held. [`hold`] sends it there
//! once the waits due on the way have ended. [`hold_from_start`] holds it at nought until the
//! scene first waits, so what the scene asks for after starting up counts from nought, and goes
//! on to the hold only once the scene's waits due by then have ended and it has begun no other
//! for [`QUIET`].

use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::Duration,
};

use tokio::sync::watch;

/// How long a held clock rests on each moment a wait ends at, in real time, before moving on: the
/// work that wait let go runs first, and a wait it begins is counted from that moment.
pub const SETTLE: Duration = Duration::from_millis(25);

/// How long a fixture scene's client waits for an answer while the clock is on: long enough for
/// any shot to be taken first, as the scene's slow answers wait on the hold, not the wall clock,
/// and a wall-clock timeout would cut a held one off.
pub const CLIENT_WAIT: Duration = Duration::from_secs(600);

/// How long a scene held from its start ([`StillClock::held_at`]) must go without a new wait or a
/// wait ending before the clock goes on to the hold.
pub const QUIET: Duration = Duration::from_millis(150);

/// A still clock: nought when made, moved only by its holds. Cheap to clone; every clone is the
/// same clock.
#[derive(Clone)]
pub struct StillClock {
    inner: Arc<Inner>,
}

struct Inner {
    /// Now, in milliseconds since the clock was made.
    now: watch::Sender<u64>,
    state: Mutex<State>,
}

struct State {
    /// Where the clock stops, in milliseconds.
    hold: u64,
    /// The moments waits end at, with how many end at each.
    ends: BTreeMap<u64, usize>,
    /// Whether the clock may go on to the hold yet: once something has waited on it, or a
    /// [`StillClock::hold`] has sent it there.
    started: bool,
    /// When the clock last saw a wait begin or end, in real time: it goes on to the hold only
    /// [`QUIET`] after. `None` lets it go at once.
    busy: Option<std::time::Instant>,
}

impl StillClock {
    /// A clock at nought, held at `at` from the scene's start: it stays at nought until something
    /// first waits on it, and goes on to `at` once the waits due by then have ended and the scene
    /// has been [`QUIET`]. Moved by a thread of its own for as long as a clone of it lives.
    pub fn held_at(at: Duration) -> Self {
        let clock = Self {
            inner: Arc::new(Inner {
                now: watch::Sender::new(0),
                state: Mutex::new(State {
                    hold: millis(at),
                    ends: BTreeMap::new(),
                    started: false,
                    busy: None,
                }),
            }),
        };
        let weak: Weak<Inner> = Arc::downgrade(&clock.inner);
        let spawned =
            std::thread::Builder::new().name("kaeri-still-clock".into()).spawn(move || {
                while let Some(inner) = weak.upgrade() {
                    inner.step();
                    drop(inner);
                    std::thread::sleep(SETTLE);
                }
            });
        if let Err(e) = spawned {
            tracing::warn!("the still clock's thread did not start: {e}");
        }
        clock.inner.step();
        clock
    }

    /// Now on this clock: how far it has moved since it was made.
    pub fn now(&self) -> Duration {
        Duration::from_millis(*self.inner.now.borrow())
    }

    /// Where the clock stops.
    pub fn held(&self) -> Duration {
        Duration::from_millis(self.inner.lock().hold)
    }

    /// Move the hold on to `at` and send the clock there: it runs on through each wait that ends
    /// on the way, and stops at the hold, at once when nothing is due before it. A hold before the
    /// present one moves the clock to the present one; the clock never goes back.
    pub fn hold(&self, at: Duration) {
        {
            let mut state = self.inner.lock();
            state.hold = state.hold.max(millis(at));
            state.started = true;
            state.busy = None;
        }
        self.inner.step();
    }

    /// Move the hold on to `at`, as [`StillClock::held_at`] holds it: the clock goes there only
    /// once the scene has waited and gone quiet.
    pub fn hold_from_start(&self, at: Duration) {
        {
            let mut state = self.inner.lock();
            state.hold = state.hold.max(millis(at));
        }
        self.inner.step();
    }

    /// A wait of `length` on this clock, from now on it.
    pub async fn wait(&self, length: Duration) {
        let end = {
            let mut state = self.inner.lock();
            let end = self.inner.now.borrow().saturating_add(millis(length));
            if end <= *self.inner.now.borrow() {
                return;
            }
            *state.ends.entry(end).or_default() += 1;
            state.started = true;
            state.busy = Some(std::time::Instant::now());
            end
        };
        // A wait given up before it ends (a timeout's, a dropped request's) takes its end away,
        // so the clock does not stop at it.
        let _pending = Pending { inner: &self.inner, end };
        let mut now = self.inner.now.subscribe();
        // The clock outlives every wait on it, so the channel cannot close first.
        let _ = now.wait_for(|now| *now >= end).await;
    }
}

/// One wait's end on the clock, taken away when the wait is over or given up.
struct Pending<'a> {
    inner: &'a Inner,
    end: u64,
}

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        let mut state = self.inner.lock();
        if let Some(count) = state.ends.get_mut(&self.end) {
            *count -= 1;
            if *count == 0 {
                state.ends.remove(&self.end);
            }
        }
    }
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// One move: to the first moment a wait ends at, if it is no later than the hold, or else to
    /// the hold once the clock may go there.
    fn step(&self) {
        let mut state = self.lock();
        let now = *self.now.borrow();
        state.ends.retain(|end, _| *end > now);
        if let Some(end) = state.ends.keys().next().copied().filter(|end| *end <= state.hold) {
            state.busy = Some(std::time::Instant::now());
            self.now.send_replace(end);
            return;
        }
        let quiet = state.busy.is_none_or(|busy| busy.elapsed() >= QUIET);
        if state.started && quiet && state.hold > now {
            self.now.send_replace(state.hold);
        }
    }
}

fn millis(length: Duration) -> u64 {
    u64::try_from(length.as_millis()).unwrap_or(u64::MAX)
}

/// The process's still clock, once [`hold`] or [`hold_from_start`] has turned it on.
static CLOCK: OnceLock<StillClock> = OnceLock::new();

/// Turn the process's still clock on and send it to `at` ([`StillClock::hold`]): for an app's
/// replay of made-up events, stepping the clock to each one's moment. From then on every [`wait`]
/// runs on it. For a Fixtures build's shot or a test of one only; nothing turns it off again.
pub fn hold(at: Duration) {
    CLOCK.get_or_init(|| StillClock::held_at(Duration::ZERO)).hold(at);
}

/// Turn the process's still clock on held at `at` from the scene's start
/// ([`StillClock::held_at`]), or move its hold on to `at` that way: for a shot of what a scene
/// shows `at` in, its made-up waits counted from when it first waits. For a Fixtures build's shot
/// or a test of one only; nothing turns it off again.
pub fn hold_from_start(at: Duration) {
    CLOCK.get_or_init(|| StillClock::held_at(Duration::ZERO)).hold_from_start(at);
}

/// The process's still clock, if [`hold`] or [`hold_from_start`] has turned it on.
pub fn clock() -> Option<&'static StillClock> {
    CLOCK.get()
}

/// Now on the process's still clock, if [`hold`] has turned it on.
pub fn now() -> Option<Duration> {
    clock().map(StillClock::now)
}

/// A made-up wait of `length`: on the process's still clock once [`hold`] has turned it on, and
/// otherwise on the wall clock. The fixture scenes' slow answers wait with this.
pub async fn wait(length: Duration) {
    match clock() {
        Some(clock) => clock.wait(length).await,
        None if length.is_zero() => {}
        None => tokio::time::sleep(length).await,
    }
}

/// `work`, given up after `length` of [`wait`]: `None` when the wait ended first.
pub async fn timeout<F: Future>(length: Duration, work: F) -> Option<F::Output> {
    tokio::select! {
        done = work => Some(done),
        () = wait(length) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// The waits end in their order, each at its own moment, all at once in real time.
    #[tokio::test(flavor = "multi_thread")]
    async fn waits_up_to_the_hold_end_in_order_at_once() {
        let clock = StillClock::held_at(ms(5_000));
        let ended = Arc::new(Mutex::new(Vec::new()));
        let started = std::time::Instant::now();
        let waits: Vec<_> = [4_500, 450, 2_400, 1_300]
            .into_iter()
            .map(|length| {
                let (clock, ended) = (clock.clone(), Arc::clone(&ended));
                tokio::spawn(async move {
                    clock.wait(ms(length)).await;
                    ended.lock().unwrap().push((length, clock.now()));
                })
            })
            .collect();
        for wait in waits {
            wait.await.unwrap();
        }
        let ended = ended.lock().unwrap().clone();
        assert_eq!(
            ended,
            vec![(450, ms(450)), (1_300, ms(1_300)), (2_400, ms(2_400)), (4_500, ms(4_500)),]
        );
        assert!(started.elapsed() < Duration::from_secs(2), "took {:?}", started.elapsed());
        tokio::time::sleep(QUIET + SETTLE * 4).await;
        assert_eq!(clock.now(), ms(5_000), "the clock stops at the hold");
    }

    /// A wait past the hold does not end; moving the hold on ends it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_wait_past_the_hold_ends_once_the_hold_moves_on() {
        let clock = StillClock::held_at(ms(3_000));
        let wait = tokio::spawn({
            let clock = clock.clone();
            async move { clock.wait(ms(20_000)).await }
        });
        tokio::time::sleep(QUIET + SETTLE * 4).await;
        assert!(!wait.is_finished(), "a wait past the hold ended");
        assert_eq!(clock.now(), ms(3_000));
        clock.hold(ms(21_000));
        tokio::time::timeout(Duration::from_secs(2), wait).await.unwrap().unwrap();
        assert_eq!(clock.now(), ms(20_000));
        tokio::time::sleep(QUIET + SETTLE * 4).await;
        assert_eq!(clock.now(), ms(21_000));
    }

    /// A wait begun when another ends counts from that moment, as on the wall clock.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_wait_after_a_wait_counts_from_its_end() {
        let clock = StillClock::held_at(ms(10_000));
        let other = tokio::spawn({
            let clock = clock.clone();
            async move { clock.wait(ms(6_000)).await }
        });
        clock.wait(ms(1_000)).await;
        assert_eq!(clock.now(), ms(1_000));
        clock.wait(ms(2_000)).await;
        assert_eq!(clock.now(), ms(3_000), "the second wait counted from the first one's end");
        other.await.unwrap();
        assert_eq!(clock.now(), ms(6_000));
    }

    /// Held from the start, the clock stays at nought until the scene first waits, so its first
    /// asks count from nought however long it takes to start up.
    #[tokio::test(flavor = "multi_thread")]
    async fn held_from_the_start_it_waits_for_the_first_wait() {
        let clock = StillClock::held_at(ms(5_000));
        tokio::time::sleep(QUIET * 2).await;
        assert_eq!(clock.now(), Duration::ZERO, "it went on before the scene waited");
        clock.wait(ms(4_500)).await;
        assert_eq!(clock.now(), ms(4_500));
        tokio::time::sleep(QUIET + SETTLE * 4).await;
        assert_eq!(clock.now(), ms(5_000));
    }

    /// A wait given up before its end takes the end away: the clock goes on to the hold without
    /// stopping there.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_wait_given_up_leaves_no_stop_behind() {
        let clock = StillClock::held_at(Duration::ZERO);
        let gone = tokio::time::timeout(Duration::from_millis(50), clock.wait(ms(5_000))).await;
        assert!(gone.is_err(), "the wait ended at nought");
        assert!(clock.inner.lock().ends.is_empty(), "the given-up wait's end is still there");
        clock.hold(ms(10_000));
        assert_eq!(clock.now(), ms(10_000), "the clock stopped short of the hold");
    }

    /// The hold never goes back, and a wait of nought ends at once.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_hold_only_moves_on() {
        let clock = StillClock::held_at(ms(4_000));
        clock.hold(ms(1_000));
        assert_eq!(clock.held(), ms(4_000));
        assert_eq!(
            clock.now(),
            ms(4_000),
            "nothing waits, so the clock is at the hold at once"
        );
        clock.wait(Duration::ZERO).await;
        assert_eq!(clock.now(), ms(4_000));
    }

    /// Off, a made-up wait is the wall clock's.
    #[tokio::test(start_paused = true)]
    async fn off_the_waits_are_the_wall_clocks() {
        assert!(clock().is_none());
        let started = tokio::time::Instant::now();
        wait(ms(4_500)).await;
        assert_eq!(started.elapsed(), ms(4_500));
        assert_eq!(timeout(ms(100), std::future::pending::<()>()).await, None);
    }
}
