//! Work that cannot be cancelled, run where it can be left behind.
//!
//! The PDF reader has no cancel hook, and a crafted file can keep it busy for many seconds. So a
//! PDF is opened on a worker thread with a time budget: when the budget runs out the caller gets
//! [`TimedOut`] and goes on, and the thread finishes in its own time and drops its result. Only a
//! few threads may be left running this way at once; past that, new work is refused rather than
//! letting a folder of hostile files pile up threads.

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

/// How many abandoned workers may still be running before new ones are refused.
const MAX_ABANDONED: usize = 2;

static ABANDONED: AtomicUsize = AtomicUsize::new(0);

/// How long a piece of work may take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget(Duration);

impl Budget {
    pub const fn of(duration: Duration) -> Self {
        Self(duration)
    }
}

/// The work did not finish in time, or was refused because too many earlier ones are still
/// running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the work took longer than its budget")]
pub struct TimedOut;

/// How many workers have been left running.
pub fn abandoned() -> usize {
    ABANDONED.load(Ordering::SeqCst)
}

/// Run `work` on its own thread and wait at most `budget` for it.
pub fn run<T, F>(budget: Budget, work: F) -> Result<T, TimedOut>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    if abandoned() >= MAX_ABANDONED {
        return Err(TimedOut);
    }
    let (tx, rx) = mpsc::channel();
    let gave_up = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = gave_up.clone();
    let spawned =
        std::thread::Builder::new().name("kaeri-books-worker".into()).spawn(move || {
            let result = work();
            // Count the thread out if the caller already counted it as abandoned.
            if tx.send(result).is_err() && flag.swap(false, Ordering::SeqCst) {
                ABANDONED.fetch_sub(1, Ordering::SeqCst);
            }
        });
    if spawned.is_err() {
        return Err(TimedOut);
    }
    match rx.recv_timeout(budget.0) {
        Ok(value) => Ok(value),
        Err(_) => {
            gave_up.store(true, Ordering::SeqCst);
            ABANDONED.fetch_add(1, Ordering::SeqCst);
            // The worker may have finished in the instant between the timeout and the flag.
            if rx.try_recv().is_ok() && gave_up.swap(false, Ordering::SeqCst) {
                ABANDONED.fetch_sub(1, Ordering::SeqCst);
            }
            Err(TimedOut)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_work_answers_and_slow_work_is_left_behind() {
        assert_eq!(run(Budget::of(Duration::from_secs(5)), || 7), Ok(7));
        let before = abandoned();
        let slow = run(Budget::of(Duration::from_millis(20)), || {
            std::thread::sleep(Duration::from_millis(300));
            1
        });
        assert_eq!(slow, Err(TimedOut));
        assert_eq!(abandoned(), before + 1);
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(abandoned(), before, "the thread counts itself out when it finishes");
    }
}
