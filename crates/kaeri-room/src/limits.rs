//! How much this member says: at most three messages in five seconds, then Send waits, and one
//! reaction per two seconds. Servers have no flood control of their own that Kaeri knows of, so
//! the client keeps to it. Pure and clocked by the caller, in seconds on any steady clock.

use std::collections::VecDeque;

/// Messages allowed within [`WINDOW`].
pub const MESSAGES: usize = 3;
/// The span the message limit is counted over, in seconds.
pub const WINDOW: f64 = 5.0;
/// The least time between two reactions, in seconds.
pub const REACTION_GAP: f64 = 2.0;

/// What this member has sent lately. A reaction is a chat line too, so it counts as a message
/// and has its own gap on top.
#[derive(Debug, Clone, Default)]
pub struct SendLimits {
    sent: VecDeque<f64>,
    last_reaction: Option<f64>,
}

impl SendLimits {
    fn prune(&mut self, now: f64) {
        while self.sent.front().is_some_and(|at| now - *at >= WINDOW || *at > now) {
            self.sent.pop_front();
        }
        if self.last_reaction.is_some_and(|at| at > now) {
            self.last_reaction = None;
        }
    }

    /// Seconds until a message may be sent; 0 when it may be now.
    pub fn message_wait(&mut self, now: f64) -> f64 {
        self.prune(now);
        if self.sent.len() < MESSAGES {
            return 0.0;
        }
        let oldest_that_counts = self.sent[self.sent.len() - MESSAGES];
        (oldest_that_counts + WINDOW - now).max(0.0)
    }

    /// Seconds until a reaction may be sent; 0 when it may be now.
    pub fn reaction_wait(&mut self, now: f64) -> f64 {
        let messages = self.message_wait(now);
        let gap = self.last_reaction.map_or(0.0, |at| (at + REACTION_GAP - now).max(0.0));
        gap.max(messages)
    }

    /// Take a message if it may go now: `Ok`, or `Err` with the seconds to wait.
    pub fn take_message(&mut self, now: f64) -> Result<(), f64> {
        match self.message_wait(now) {
            wait if wait > 0.0 => Err(wait),
            _ => {
                self.sent.push_back(now);
                Ok(())
            }
        }
    }

    /// Take a reaction if it may go now: `Ok`, or `Err` with the seconds to wait.
    pub fn take_reaction(&mut self, now: f64) -> Result<(), f64> {
        match self.reaction_wait(now) {
            wait if wait > 0.0 => Err(wait),
            _ => {
                self.sent.push_back(now);
                self.last_reaction = Some(now);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_messages_in_five_seconds_then_send_waits() {
        let mut limits = SendLimits::default();
        assert_eq!(limits.take_message(0.0), Ok(()));
        assert_eq!(limits.take_message(1.0), Ok(()));
        assert_eq!(limits.take_message(2.0), Ok(()));
        assert_eq!(limits.take_message(2.5), Err(2.5));
        assert!(limits.take_message(4.9).is_err_and(|wait| (wait - 0.1).abs() < 1e-9));
        // The first one is out of the window at 5.0.
        assert_eq!(limits.take_message(5.0), Ok(()));
        assert!(limits.take_message(5.1).is_err_and(|wait| (wait - 0.9).abs() < 1e-9));
    }

    #[test]
    fn a_refused_message_does_not_count() {
        let mut limits = SendLimits::default();
        for t in [0.0, 0.1, 0.2] {
            limits.take_message(t).unwrap();
        }
        for _ in 0..10 {
            assert!(limits.take_message(1.0).is_err());
        }
        assert_eq!(limits.take_message(5.0), Ok(()));
    }

    #[test]
    fn one_reaction_per_two_seconds_and_they_count_as_messages() {
        let mut limits = SendLimits::default();
        assert_eq!(limits.take_reaction(0.0), Ok(()));
        assert_eq!(limits.take_reaction(1.5), Err(0.5));
        assert_eq!(limits.take_reaction(2.0), Ok(()));
        assert_eq!(limits.take_reaction(4.0), Ok(()));
        // Three lines in the window: the next waits for the first to leave it.
        assert_eq!(limits.take_message(4.5), Err(0.5));
        assert_eq!(limits.reaction_wait(4.5), 1.5);
    }

    #[test]
    fn a_clock_that_goes_back_forgets_what_it_cannot_place() {
        let mut limits = SendLimits::default();
        limits.take_reaction(100.0).unwrap();
        assert_eq!(limits.take_reaction(1.0), Ok(()));
    }
}
