//! The time the new-episode check, the widgets' record and the notifications' week go by, and
//! the zone they read days in.
//!
//! The system's clock in the system's zone. With the `testing` feature a test can move it on
//! (`Clock::advance`), to see what a check finds once an episode has aired without waiting for
//! it, and read its days in a zone of its own (`Clock::in_zone`), so that what is today and what
//! airs this week do not depend on where, or at what hour, the test runs. A clone cannot be
//! stopped at a moment of its own: what it dates is compared with what the store and the metadata
//! cache date by the system's clock, which the rest of the engine reads directly
//! ([`crate::now_epoch`]). The whole engine can be, by [`stop_at`], for recording the apps'
//! scenes: then every read of now, this clock's and [`crate::now_epoch`]'s, is that moment, and
//! two recordings come out the same.

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use jiff::tz::TimeZone;

/// Stop the whole engine's now at `at`, in seconds since the Unix epoch, for the rest of the
/// process: for recording the apps' scenes, which must come out the same on every run. Nought
/// starts it again. The store's clock and the metadata cache's ages stop with it
/// ([`kaeri_store::stop_clock_at`]).
#[cfg(feature = "testing")]
pub fn stop_at(at: i64) {
    kaeri_store::stop_clock_at(at);
}

/// The moment [`stop_at`] stopped now at, if it did.
pub(crate) fn stopped() -> Option<i64> {
    kaeri_store::stopped()
}

/// Now, as the work that is dated by it reads it, and the zone its days are in. Cheap to clone;
/// every clone moves together.
#[derive(Debug, Clone, Default)]
pub struct Clock {
    /// How far ahead of the system's clock this one runs: always nought outside a test.
    ahead: Arc<AtomicI64>,
    /// The zone a test reads days in, in place of the system's.
    zone: Option<TimeZone>,
}

impl Clock {
    /// Now, in seconds since the Unix epoch.
    pub fn now(&self) -> i64 {
        crate::now_epoch().saturating_add(self.ahead.load(Ordering::Relaxed))
    }

    /// The zone days are read in: the system's, as the device has it, or a test's.
    pub fn zone(&self) -> TimeZone {
        self.zone.clone().unwrap_or_else(TimeZone::system)
    }

    /// The system's clock, reading days in `zone` rather than the system's zone.
    #[cfg(any(test, feature = "testing"))]
    pub fn in_zone(zone: TimeZone) -> Self {
        Self { ahead: Arc::default(), zone: Some(zone) }
    }

    /// Move this clock, and every clone of it, `secs` further on.
    #[cfg(any(test, feature = "testing"))]
    pub fn advance(&self, secs: i64) {
        self.ahead.fetch_add(secs, Ordering::Relaxed);
    }
}
