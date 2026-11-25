//! The clock the starting screen times a start on. It is the wall clock, except in a Fixtures
//! build's still shot, where it is the engine's still clock ([`kaeri_providers::still`]) once a
//! shot has turned that on: the screen's seconds are then the moment the shot holds, however
//! slow the machine taking it. A build without the `fixtures` feature has only the wall clock.

use std::time::{Duration, Instant};

/// A moment on the starting screen's clock.
#[derive(Clone, Copy, Debug)]
pub struct Moment {
    wall: Instant,
    /// Now on the still clock at this moment, while that is on.
    #[cfg(feature = "fixtures")]
    still: Option<Duration>,
}

impl Moment {
    pub fn now() -> Self {
        Self {
            wall: Instant::now(),
            #[cfg(feature = "fixtures")]
            still: kaeri_providers::still::now(),
        }
    }

    /// How long since this moment: on the still clock when it was on then and still is, else on
    /// the wall clock.
    pub fn elapsed(&self) -> Duration {
        #[cfg(feature = "fixtures")]
        if let (Some(then), Some(now)) = (self.still, kaeri_providers::still::now()) {
            return now.saturating_sub(then);
        }
        self.wall.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the still clock off, as in every app and recording, a moment is the wall clock's.
    #[test]
    fn off_it_is_the_wall_clock() {
        let then = Moment::now();
        std::thread::sleep(Duration::from_millis(20));
        assert!(then.elapsed() >= Duration::from_millis(20));
    }
}
