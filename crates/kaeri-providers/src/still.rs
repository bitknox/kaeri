//! The still clock: the time a fixture scene's made-up waits run on in a screenshot, held at the
//! moment the shot names. A source the scene has answer 4.5 s in has answered in a shot held at
//! 5 s, at once, and has not in one held at 3 s, however slow the machine taking it, so every run
//! of the shot shows the same and none waits for the moment on the wall clock.
//!
//! The clock is off in every app, in every recording and in every test that does not turn it on:
//! [`wait`] is then the wall clock's sleep. A Fixtures build turns it on for a shot with
//! [`hold`] (`testing_still_clock_hold` in the bindings, or `KAERI_STILL_AT` when a scene
//! starts), and the apps' own timing of a shot, such as a starting screen's seconds, reads
//! [`now`].
//!
//! Held, the clock moves by itself from the moment one wait ends to the moment the next one does,
//! resting [`SETTLE`] of real time on each so the work a wait let go has run before the next, and
//! stops at the hold. A wait that would end past the hold never ends until the hold moves on.

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
}

impl StillClock {
    /// A clock at nought, held at `at`, moved by a thread of its own for as long as a clone of it
    /// lives.
    pub fn held_at(at: Duration) -> Self {
        let clock = Self {
            inner: Arc::new(Inner {
                now: watch::Sender::new(0),
                state: Mutex::new(State { hold: millis(at), ends: BTreeMap::new() }),
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

    /// Move the hold on to `at`: the clock runs on to it, through each wait that ends on the way,
    /// and stops there. A hold before the present one changes nothing; the clock never goes back.
    pub fn hold(&self, at: Duration) {
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
            end
        };
        let mut now = self.inner.now.subscribe();
        // The clock outlives every wait on it, so the channel cannot close first.
        let _ = now.wait_for(|now| *now >= end).await;
    }
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// One move: to the first moment a wait ends at, if it is no later than the hold, or else to
    /// the hold.
    fn step(&self) {
        let mut state = self.lock();
        let now = *self.now.borrow();
        state.ends.retain(|end, _| *end > now);
        let next = state.ends.keys().next().copied().filter(|end| *end <= state.hold);
        let to = next.unwrap_or(state.hold);
        if to > now {
            self.now.send_replace(to);
        }
    }
}

fn millis(length: Duration) -> u64 {
    u64::try_from(length.as_millis()).unwrap_or(u64::MAX)
}

/// The process's still clock, once [`hold`] has turned it on.
static CLOCK: OnceLock<StillClock> = OnceLock::new();

/// Turn the process's still clock on, held at `at`, or move its hold on to `at`: from then on
/// every [`wait`] runs on it. For a Fixtures build's shot or a test of one only; nothing turns it
/// off again.
pub fn hold(at: Duration) {
    CLOCK.get_or_init(|| StillClock::held_at(Duration::ZERO)).hold(at);
}

/// The process's still clock, if [`hold`] has turned it on.
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
        tokio::time::sleep(SETTLE * 4).await;
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
        tokio::time::sleep(SETTLE * 8).await;
        assert!(!wait.is_finished(), "a wait past the hold ended");
        assert_eq!(clock.now(), ms(3_000));
        clock.hold(ms(21_000));
        tokio::time::timeout(Duration::from_secs(2), wait).await.unwrap().unwrap();
        assert_eq!(clock.now(), ms(20_000));
        tokio::time::sleep(SETTLE * 4).await;
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
