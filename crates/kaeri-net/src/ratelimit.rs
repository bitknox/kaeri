//! Token-bucket rate limiting.
//!
//! AniList advertises its limit in response headers (`x-ratelimit-limit: 30`,
//! `x-ratelimit-remaining`, `x-ratelimit-reset`), and the spine of the whole app sits
//! behind it — search, seasonal, calendar, library. Getting throttled there does not
//! degrade one provider, it degrades everything.
//!
//! So the limiter is *reactive* as well as proactive: it paces requests to the configured
//! budget, and when the server reports its own view it defers to that. The server is the
//! authority on its own limit; our local count is only an estimate.
//!
//! Requests come in three classes ([`Priority`]). A screen's go first, as the bucket allows.
//! Work for the page on screen goes next, while the server's count stays above a reserve kept
//! for screens. Background work is a queue: it waits for the launch's screens, for a quiet
//! moment after the last screen request, and then goes one at a time at a share of the limit,
//! never into the reserve. See [`Pacing`].

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::time::Instant;

/// How urgent a request is. A screen's request goes before the others: work for the page on
/// screen waits while a screen's is waiting, and background work waits for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Something on screen is waiting for it.
    Screen,
    /// For the page on screen, which shows without it and fills in when it lands: its
    /// franchise's seasons, their headers, the open Calendar's walk. Goes as soon as no screen
    /// request waits, without the background's spacing and quiet, while more than the reserve
    /// is left.
    OnScreen,
    /// Nobody is looking at it: syncs, widgets, digests, stills, prefetch. Queued, paced and
    /// kept out of the reserve: see [`Pacing`].
    Background,
}

/// How background work is paced, against the live limit `L` the server reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pacing {
    /// Background work goes at `L / background_share` a minute: one every
    /// `60 / (L / background_share)` seconds. Zero leaves it unpaced.
    pub background_share: u32,
    /// What background work and work for the page on screen never take: `L / reserve_share` of
    /// what the server says is left. Zero keeps no reserve.
    pub reserve_share: u32,
    /// How long background work waits after any screen request.
    pub quiet: Duration,
    /// How long background work waits after the launch's screens are answered.
    pub launch_after: Duration,
    /// The longest the launch holds background work back, whatever the screens do.
    pub launch_at_most: Duration,
}

impl Pacing {
    /// The app's: a sixth of the limit for background work, a third kept for screens, ten
    /// seconds of quiet after a screen request, and the launch's screens first.
    pub const APP: Self = Self {
        background_share: 6,
        reserve_share: 3,
        quiet: Duration::from_secs(10),
        launch_after: Duration::from_secs(5),
        launch_at_most: Duration::from_secs(15),
    };

    /// Background work as soon as the bucket allows, with no reserve and no launch: for tests
    /// that are about something else.
    pub const UNPACED: Self = Self {
        background_share: 0,
        reserve_share: 0,
        quiet: Duration::ZERO,
        launch_after: Duration::ZERO,
        launch_at_most: Duration::ZERO,
    };
}

impl Default for Pacing {
    fn default() -> Self {
        Self::APP
    }
}

/// How long a server's window lasts: AniList counts a fixed minute from the first request.
const WINDOW: Duration = Duration::from_secs(60);
/// What a hold adds past the end of the window, for the request that began it being counted a
/// moment after it was sent.
const WINDOW_SLACK: Duration = Duration::from_secs(1);
/// How often work that is not a screen's looks again while it waits: its class can be raised,
/// and the launch can end, while it sleeps.
const POLL: Duration = Duration::from_millis(250);
/// How long work that lets a screen go first waits before it looks again.
const YIELD: Duration = Duration::from_millis(50);

/// A refilling token bucket.
#[derive(Debug)]
pub struct RateLimiter {
    state: Mutex<State>,
    /// The most a server's own limit may resize the bucket to, per minute.
    ceiling: u32,
    /// Screen requests waiting for a token, which the others let go first.
    screens_waiting: AtomicUsize,
    /// Requests for the page on screen waiting, which background work lets go first.
    on_screen_waiting: AtomicUsize,
}

#[derive(Debug)]
struct State {
    tokens: f64,
    last_refill: Instant,
    /// Set when the server tells us to stop until a given moment.
    hold_until: Option<Instant>,
    /// Tokens the bucket holds at most.
    capacity: f64,
    /// Tokens added per second.
    refill_rate: f64,
    pacing: Pacing,
    /// What the server said is left in its window, the lowest it said, less what went since.
    server_left: Option<f64>,
    /// When the window [`Self::server_left`] counts in ends, as far as it is known.
    server_left_until: Option<Instant>,
    /// When the server's window began: the first answer after a hold, or the first that says
    /// the limit less one is left.
    window_start: Option<Instant>,
    /// Answers in a row, within a window, that said nothing is left and gave no reset.
    zeros: u32,
    /// The next answer begins a window: a hold has just been set.
    after_hold: bool,
    /// When the last screen request went.
    last_screen: Option<Instant>,
    /// When the last background request went.
    last_background: Option<Instant>,
    /// Background work waits until then: the launch's screens go first.
    launch_until: Option<Instant>,
    /// Background requests waiting, by ticket, in the order they asked.
    queue: VecDeque<u64>,
    next_ticket: u64,
}

impl State {
    /// Forget what the server said of a window that has ended.
    fn expire(&mut self, now: Instant) {
        if self.server_left_until.is_some_and(|until| until <= now) {
            self.server_left = None;
            self.server_left_until = None;
            self.window_start = None;
            self.zeros = 0;
        }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }

    /// What may go now, as far as both the bucket and the server's word know.
    fn left(&self) -> f64 {
        self.server_left.map_or(self.tokens, |server| self.tokens.min(server))
    }

    /// The tokens kept for screens.
    fn reserve(&self) -> f64 {
        if self.pacing.reserve_share == 0 {
            return 0.0;
        }
        (self.capacity / f64::from(self.pacing.reserve_share))
            .floor()
            .clamp(0.0, (self.capacity - 1.0).max(0.0))
    }

    /// How long until more than the reserve is left, at most [`POLL`].
    fn reserve_wait(&self, now: Instant) -> Duration {
        let reserve = self.reserve();
        let server_short = self.server_left.is_some_and(|server| server < 1.0 + reserve);
        let wait = if server_short {
            self.server_left_until.map_or(POLL, |until| until.saturating_duration_since(now))
        } else {
            Duration::from_secs_f64(((1.0 + reserve - self.tokens) / self.refill_rate).max(0.0))
        };
        wait.clamp(Duration::from_millis(1), POLL)
    }

    /// The gap kept between background requests: `60 / (L / share)` seconds.
    fn spacing(&self) -> Duration {
        if self.pacing.background_share == 0 {
            return Duration::ZERO;
        }
        let per_minute = (self.capacity / f64::from(self.pacing.background_share)).max(1.0);
        Duration::from_secs_f64(60.0 / per_minute)
    }

    fn take(&mut self, now: Instant, priority: Priority) {
        self.tokens -= 1.0;
        if let Some(server) = self.server_left.as_mut() {
            *server = (*server - 1.0).max(0.0);
        }
        match priority {
            Priority::Screen => self.last_screen = Some(now),
            Priority::OnScreen => {}
            Priority::Background => self.last_background = Some(now),
        }
    }

    /// Hold every request for `hold`, and take the next answer as a window's first.
    /// Hold every request for `hold`. The server's window ends with it, so what it said was
    /// left goes too, and the first answer after the hold begins the next window.
    fn hold_for(&mut self, now: Instant, hold: Duration) {
        self.hold_until = Some(hold_from(now, hold));
        self.after_hold = true;
        self.zeros = 0;
        self.server_left = None;
        self.server_left_until = None;
        self.window_start = None;
    }
}

/// A request of one class in the queue, counted until it has its token or is dropped.
struct Waiting<'a>(&'a AtomicUsize);

impl<'a> Waiting<'a> {
    fn new(counter: &'a AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A background request's place in the queue, given up when it is dropped.
struct Ticket<'a> {
    state: &'a Mutex<State>,
    number: u64,
}

impl Drop for Ticket<'_> {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.queue.retain(|n| *n != self.number);
    }
}

impl RateLimiter {
    /// Build a limiter for `per_minute` requests.
    pub fn per_minute(per_minute: u32) -> Self {
        let capacity = f64::from(per_minute.max(1));
        Self::build(capacity, capacity / 60.0, per_minute.max(1))
    }

    /// Build a limiter for `per_minute` requests that the server's own word, through
    /// [`Self::observe_limit`], may raise to `ceiling` a minute and lower to one.
    pub fn per_minute_up_to(per_minute: u32, ceiling: u32) -> Self {
        let capacity = f64::from(per_minute.max(1));
        Self::build(capacity, capacity / 60.0, ceiling.max(per_minute).max(1))
    }

    fn build(capacity: f64, refill_rate: f64, ceiling: u32) -> Self {
        Self {
            state: Mutex::new(State {
                tokens: capacity,
                last_refill: Instant::now(),
                hold_until: None,
                capacity,
                refill_rate,
                pacing: Pacing::APP,
                server_left: None,
                server_left_until: None,
                window_start: None,
                zeros: 0,
                after_hold: false,
                last_screen: None,
                last_background: None,
                launch_until: None,
                queue: VecDeque::new(),
                next_ticket: 0,
            }),
            ceiling,
            screens_waiting: AtomicUsize::new(0),
            on_screen_waiting: AtomicUsize::new(0),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Build a limiter for `per_minute` requests that may never burst above `burst`.
    ///
    /// A plain [`Self::per_minute`] bucket is as deep as the whole minute's budget, so a
    /// cold limiter will happily fire the entire allowance at once. That is fine against a
    /// service which only counts per minute, and wrong against one that also caps requests
    /// per *second* — the burst is spent in the first instant and every request after it is
    /// refused. Capacity is the burst ceiling; the refill rate stays the sustained budget.
    pub fn per_minute_burst(per_minute: u32, burst: u32) -> Self {
        let refill_rate = f64::from(per_minute.max(1)) / 60.0;
        Self::build(f64::from(burst.max(1)), refill_rate, per_minute.max(1))
    }

    /// Pace background work as `pacing` says, from now on.
    pub fn set_pacing(&self, pacing: Pacing) {
        self.lock().pacing = pacing;
    }

    /// The app is launching: background work waits for its screens, [`Pacing::launch_at_most`]
    /// at most, until [`Self::launched`] says they were answered.
    pub fn launching(&self) {
        let mut state = self.lock();
        let at_most = state.pacing.launch_at_most;
        state.launch_until = (!at_most.is_zero()).then(|| Instant::now() + at_most);
    }

    /// The launch's screens were answered: background work begins [`Pacing::launch_after`]
    /// from now, or sooner if the launch's longest wait ends first.
    pub fn launched(&self) {
        let mut state = self.lock();
        let after = Instant::now() + state.pacing.launch_after;
        if let Some(until) = state.launch_until.as_mut() {
            *until = (*until).min(after);
        }
    }

    /// Whether the launch holds background work back now.
    pub fn in_launch(&self) -> bool {
        self.lock().launch_until.is_some_and(|until| until > Instant::now())
    }

    /// Wait until the launch no longer holds background work back.
    pub async fn launch_over(&self) {
        loop {
            let until = self.lock().launch_until;
            match until {
                Some(until) if until > Instant::now() => tokio::time::sleep_until(until).await,
                _ => return,
            }
        }
    }

    /// Wait until a request may proceed.
    pub async fn acquire(&self) {
        self.acquire_with(Priority::Screen).await;
    }

    /// Wait until a request of `priority` may proceed.
    pub async fn acquire_with(&self, priority: Priority) {
        self.acquire_by(|| priority).await;
    }

    /// Wait until a request may proceed, in the class `priority` says, which is asked again
    /// each time the request looks: a background request a screen comes to wait on is raised
    /// while it waits.
    pub async fn acquire_by(&self, priority: impl Fn() -> Priority) {
        let mut screen = None;
        let mut on_screen = None;
        let mut ticket: Option<Ticket<'_>> = None;
        loop {
            let class = priority();
            match class {
                Priority::Screen => {
                    screen.get_or_insert_with(|| Waiting::new(&self.screens_waiting));
                    on_screen = None;
                }
                Priority::OnScreen => {
                    on_screen.get_or_insert_with(|| Waiting::new(&self.on_screen_waiting));
                    screen = None;
                }
                Priority::Background => {}
            }
            if class != Priority::Background {
                ticket = None;
            }
            let wait = {
                let mut state = self.lock();
                let now = Instant::now();
                state.expire(now);
                if class == Priority::Background && ticket.is_none() {
                    let number = state.next_ticket;
                    state.next_ticket += 1;
                    state.queue.push_back(number);
                    ticket = Some(Ticket { state: &self.state, number });
                }
                match state.hold_until {
                    // A server-imposed hold outranks our own accounting.
                    Some(until) if until > now => Some(until - now),
                    _ => {
                        state.hold_until = None;
                        state.refill(now);
                        self.wait_for(&mut state, class, now, ticket.as_ref().map(|t| t.number))
                    }
                }
            };
            match wait {
                Some(d) if class == Priority::Screen => tokio::time::sleep(d).await,
                Some(d) => tokio::time::sleep(d.min(POLL)).await,
                None => {
                    // Taken: out of the queue as the ticket goes.
                    drop(ticket);
                    return;
                }
            }
        }
    }

    /// How long a request of `class` waits before it looks again, or `None` once it has its
    /// token. Outside a hold, with the bucket refilled.
    fn wait_for(
        &self,
        state: &mut State,
        class: Priority,
        now: Instant,
        ticket: Option<u64>,
    ) -> Option<Duration> {
        let screens = self.screens_waiting.load(Ordering::SeqCst) > 0;
        match class {
            Priority::Screen => {
                if state.tokens >= 1.0 {
                    state.take(now, class);
                    None
                } else {
                    Some(Duration::from_secs_f64((1.0 - state.tokens) / state.refill_rate))
                }
            }
            Priority::OnScreen => {
                if screens {
                    Some(YIELD)
                } else if state.left() >= 1.0 + state.reserve() {
                    state.take(now, class);
                    None
                } else {
                    Some(state.reserve_wait(now))
                }
            }
            Priority::Background => {
                let ready = |at: Option<Instant>, gap: Duration| {
                    at.and_then(|at| at.checked_add(gap)).filter(|ready| *ready > now)
                };
                if screens || self.on_screen_waiting.load(Ordering::SeqCst) > 0 {
                    return Some(YIELD);
                }
                if let Some(until) = state.launch_until.filter(|until| *until > now) {
                    return Some(until - now);
                }
                if state.queue.front().copied() != ticket {
                    return Some(POLL);
                }
                if let Some(ready) = ready(state.last_screen, state.pacing.quiet) {
                    return Some(ready - now);
                }
                if let Some(ready) = ready(state.last_background, state.spacing()) {
                    return Some(ready - now);
                }
                if state.left() < 1.0 + state.reserve() {
                    return Some(state.reserve_wait(now));
                }
                state.take(now, class);
                None
            }
        }
    }

    /// Follow the limit the server reports (`X-RateLimit-Limit`), up to this limiter's ceiling:
    /// AniList runs degraded at 30 a minute and normally allows 90, and the bucket should be as
    /// large as the server's word, whichever it is today. A bigger bucket refills faster but
    /// does not start full: the tokens already counted stay.
    pub async fn observe_limit(&self, limit: u32) {
        let per_minute = f64::from(limit.clamp(1, self.ceiling));
        let mut state = self.lock();
        if (state.capacity - per_minute).abs() < f64::EPSILON {
            return;
        }
        tracing::debug!(per_minute, "following the server's rate limit");
        state.capacity = per_minute;
        state.refill_rate = per_minute / 60.0;
        state.tokens = state.tokens.min(per_minute);
    }

    /// How long the next screen request would block, without consuming a token.
    ///
    /// For telling the user *why* something is slow. A budget of thirty requests a minute is small
    /// enough that a burst of navigation can drain it, and an indefinite loading indicator with no
    /// explanation is the worst possible way to communicate "waiting for a rate limit".
    pub async fn would_wait(&self) -> Option<Duration> {
        let state = self.lock();
        let now = Instant::now();
        if let Some(until) = state.hold_until
            && until > now
        {
            return Some(until - now);
        }
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        let tokens = (state.tokens + elapsed * state.refill_rate).min(state.capacity);
        if tokens >= 1.0 {
            return None;
        }
        Some(Duration::from_secs_f64((1.0 - tokens) / state.refill_rate))
    }

    /// How many requests could go now without waiting, as far as the bucket knows: none while
    /// the server holds us.
    pub async fn available(&self) -> f64 {
        let state = self.lock();
        let now = Instant::now();
        if state.hold_until.is_some_and(|until| until > now) {
            return 0.0;
        }
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        (state.tokens + elapsed * state.refill_rate).min(state.capacity)
    }

    /// Adopt the server's own view of the limit, from an answer's headers.
    ///
    /// `remaining` is what the server says is left in its window, and never revises the bucket
    /// upwards. When it says nothing is left, with a `reset`, every request waits until then.
    /// Without one, a single answer is not believed, since AniList's nodes disagree by a few: it
    /// takes two in a window, and the hold then lasts until the window ends, a minute from its
    /// first request, or a minute when it began is not known.
    pub async fn observe(&self, remaining: Option<u32>, reset_in: Option<Duration>) {
        let Some(remaining) = remaining else { return };
        let mut state = self.lock();
        let now = Instant::now();
        state.expire(now);
        state.refill(now);
        let limit = state.capacity.round() as u32;
        let begins = state.after_hold
            || (state.window_start.is_none() && remaining.saturating_add(1) >= limit);
        if begins {
            state.after_hold = false;
            state.window_start = Some(now);
            state.server_left = Some(f64::from(remaining));
            state.server_left_until = Some(now + WINDOW);
            state.zeros = 0;
        } else {
            let lowest =
                state.server_left.map_or(f64::from(remaining), |s| s.min(remaining.into()));
            state.server_left = Some(lowest);
            if state.server_left_until.is_none() {
                state.server_left_until = Some(now + WINDOW);
            }
        }
        // Never revise upwards: the server may be counting requests we do not know about, so
        // its number is a ceiling, not a refill.
        state.tokens = state.tokens.min(f64::from(remaining));
        if remaining > 0 {
            return;
        }
        if let Some(reset) = reset_in {
            let hold = reset.min(MAX_HOLD);
            state.hold_for(now, hold);
            tracing::warn!(?hold, "rate limit exhausted, holding as the server says");
            return;
        }
        state.zeros += 1;
        if state.zeros < 2 {
            tracing::info!(
                "the server says nothing is left; pacing, not holding, until it says so again"
            );
            return;
        }
        let hold = state
            .window_start
            .map(|start| (start + WINDOW + WINDOW_SLACK).saturating_duration_since(now))
            .filter(|hold| !hold.is_zero())
            .unwrap_or(WINDOW)
            .max(MIN_HOLD);
        state.hold_for(now, hold);
        tracing::warn!(?hold, "rate limit exhausted, holding until the window ends");
    }

    /// Explicitly back off, for a `429` or `Retry-After`.
    pub async fn back_off(&self, duration: Duration) {
        let mut state = self.lock();
        state.tokens = 0.0;
        state.hold_for(Instant::now(), duration);
    }
}

/// The longest a server's word holds requests back: an hour, as `x-ratelimit-reset` is read.
/// A `Retry-After` of a day, as a block page may send, would otherwise stall every request for
/// the day, and one near `u64::MAX` would overflow the clock.
pub const MAX_HOLD: Duration = Duration::from_secs(3_600);

/// `now` plus `hold`, held to [`MAX_HOLD`].
fn hold_from(now: Instant, hold: Duration) -> Instant {
    now.checked_add(hold.min(MAX_HOLD)).unwrap_or(now)
}

/// Pull rate-limit signals out of response headers.
///
/// Returns `(remaining, reset_in)`. Handles both the absolute-epoch and
/// seconds-from-now forms of `x-ratelimit-reset`, because services disagree about which
/// they mean and guessing wrong turns a 1-second wait into a 56-year one. When both a reset
/// and a `Retry-After` are sent the larger counts: a reset in epoch seconds against a clock
/// that is a little fast reads as "now", and a wait is never read as less than a second.
pub fn parse_rate_headers(
    get: impl Fn(&str) -> Option<String>,
    now_epoch: i64,
) -> (Option<u32>, Option<Duration>) {
    let remaining = get("x-ratelimit-remaining").and_then(|v| v.trim().parse::<u32>().ok());

    let reset =
        get("x-ratelimit-reset").and_then(|v| v.trim().parse::<i64>().ok()).and_then(|reset| {
            // Values far in the future are epoch timestamps; small ones are durations. An epoch
            // reset already past says nothing about the wait, only that the clocks disagree, so
            // it is not read as one: a refusal then waits as `Retry-After` or the minute says.
            let secs = if reset > now_epoch.saturating_sub(86_400) && reset > 100_000 {
                (reset > now_epoch).then(|| reset - now_epoch)?
            } else {
                reset.max(0)
            };
            Some(Duration::from_secs(secs as u64))
        });
    let retry_after =
        get("retry-after").and_then(|v| v.trim().parse::<u64>().ok()).map(Duration::from_secs);
    let wait = match (reset, retry_after) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };

    (remaining, wait.map(|wait| wait.clamp(MIN_HOLD, MAX_HOLD)))
}

/// The shortest wait the server's word is read as: a reset that parses as zero seconds would
/// retry at once, earn another refusal and a longer timeout.
pub const MIN_HOLD: Duration = Duration::from_secs(1);

/// The limit the server reports for a minute (`x-ratelimit-limit`), if it does.
pub fn parse_rate_limit(get: impl Fn(&str) -> Option<String>) -> Option<u32> {
    get("x-ratelimit-limit").and_then(|v| v.trim().parse::<u32>().ok()).filter(|n| *n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let owned: Vec<(String, String)> =
            pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        move |name| {
            owned.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn requests_within_budget_do_not_block() {
        let limiter = RateLimiter::per_minute(30);
        let start = Instant::now();
        for _ in 0..30 {
            limiter.acquire().await;
        }
        assert_eq!(start.elapsed(), Duration::ZERO, "full bucket should be free");
    }

    #[tokio::test(start_paused = true)]
    async fn a_burst_ceiling_is_not_the_whole_minute() {
        // 120/min with a ceiling of 4: the fifth request in one instant has to wait,
        // where a plain per_minute(120) would have let 120 through.
        let limiter = RateLimiter::per_minute_burst(120, 4);
        for _ in 0..4 {
            limiter.acquire().await;
        }
        let start = Instant::now();
        limiter.acquire().await;
        // 120/min refills two tokens a second, so the next one is half a second out.
        assert!(
            start.elapsed() >= Duration::from_millis(400),
            "burst ceiling should pace the next request, waited {:?}",
            start.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_burst_ceiling_still_sustains_the_full_rate() {
        let limiter = RateLimiter::per_minute_burst(120, 4);
        let start = Instant::now();
        for _ in 0..120 {
            limiter.acquire().await;
        }
        // A minute's worth of requests takes about a minute, not longer: the ceiling
        // shapes the bursts, it does not cut the budget.
        assert!(
            start.elapsed() <= Duration::from_secs(60),
            "sustained rate should still be 120/min, took {:?}",
            start.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn exceeding_the_budget_paces_rather_than_failing() {
        let limiter = RateLimiter::per_minute(30);
        for _ in 0..30 {
            limiter.acquire().await;
        }
        let start = Instant::now();
        limiter.acquire().await;
        // 30/min means one token every 2s.
        assert!(
            start.elapsed() >= Duration::from_secs(2),
            "should have waited for a refill, waited {:?}",
            start.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn the_server_view_can_only_tighten_the_budget() {
        let limiter = RateLimiter::per_minute(30);
        // Server says only 1 request left despite our bucket being full.
        limiter.observe(Some(1), None).await;
        limiter.acquire().await;

        let start = Instant::now();
        limiter.acquire().await;
        assert!(
            start.elapsed() >= Duration::from_secs(2),
            "server's lower count must be respected over our own optimism"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn exhaustion_holds_until_the_reset_moment() {
        let limiter = RateLimiter::per_minute(30);
        limiter.observe(Some(0), Some(Duration::from_secs(45))).await;
        let start = Instant::now();
        limiter.acquire().await;
        assert!(start.elapsed() >= Duration::from_secs(45));
    }

    #[tokio::test(start_paused = true)]
    async fn back_off_blocks_for_the_requested_duration() {
        let limiter = RateLimiter::per_minute(60);
        limiter.back_off(Duration::from_secs(10)).await;
        let start = Instant::now();
        limiter.acquire().await;
        assert!(start.elapsed() >= Duration::from_secs(10));
    }

    #[test]
    fn reset_header_as_seconds_from_now_is_read_literally() {
        let (remaining, reset) = parse_rate_headers(
            headers(&[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "12")]),
            1_785_000_000,
        );
        assert_eq!(remaining, Some(0));
        assert_eq!(reset, Some(Duration::from_secs(12)));
    }

    #[test]
    fn reset_header_as_an_epoch_timestamp_is_converted_to_a_delay() {
        // The failure this guards against: treating an epoch value as a duration would
        // schedule a wait decades long and hang the app.
        let now = 1_785_000_000_i64;
        let (_, reset) =
            parse_rate_headers(headers(&[("x-ratelimit-reset", "1785000030")]), now);
        assert_eq!(reset, Some(Duration::from_secs(30)));
    }

    #[test]
    fn a_retry_after_is_held_to_an_hour_too() {
        for said in ["86400", "18446744073709551615"] {
            let (_, reset) =
                parse_rate_headers(headers(&[("retry-after", said)]), 1_792_000_000);
            assert_eq!(reset, Some(MAX_HOLD), "retry-after: {said}");
        }
    }

    /// A hold handed in directly, as a client that reads its own Retry-After hands it in, is
    /// held to the hour as well, and one too large for the clock is no panic.
    #[tokio::test(start_paused = true)]
    async fn no_hold_outlasts_an_hour_however_long_it_is_asked_for() {
        let limiter = RateLimiter::per_minute(90);
        limiter.back_off(Duration::MAX).await;
        assert!(limiter.would_wait().await.is_some_and(|wait| wait <= MAX_HOLD));
        limiter.observe(Some(0), Some(Duration::from_secs(86_400))).await;
        assert!(limiter.would_wait().await.is_some_and(|wait| wait <= MAX_HOLD));
    }

    #[test]
    fn reset_delays_are_capped_to_an_hour() {
        let (_, reset) = parse_rate_headers(headers(&[("x-ratelimit-reset", "99999999")]), 0);
        assert_eq!(reset, Some(Duration::from_secs(3_600)));
    }

    #[test]
    fn retry_after_is_used_when_no_reset_header_exists() {
        let (_, reset) = parse_rate_headers(headers(&[("retry-after", "5")]), 0);
        assert_eq!(reset, Some(Duration::from_secs(5)));
    }

    #[test]
    fn missing_headers_yield_no_signal() {
        let (remaining, reset) = parse_rate_headers(headers(&[]), 0);
        assert_eq!(remaining, None);
        assert_eq!(reset, None);
    }

    #[test]
    fn anilist_headers_parse_as_observed_in_the_wild() {
        // Exactly what graphql.anilist.co returned during planning.
        let (remaining, _) = parse_rate_headers(
            headers(&[("x-ratelimit-limit", "30"), ("x-ratelimit-remaining", "29")]),
            1_785_000_000,
        );
        assert_eq!(remaining, Some(29));
    }

    #[test]
    fn a_reset_never_reads_as_zero_seconds() {
        let now = 1_785_000_000;
        let (_, wait) = parse_rate_headers(headers(&[("x-ratelimit-reset", "0")]), now);
        assert_eq!(wait, Some(MIN_HOLD), "a literal zero waits a second");
        // An epoch reset against a clock that runs ahead is no wait at all: the caller's own
        // default (a minute, for a refusal) stands, not a retry at once.
        let (_, wait) =
            parse_rate_headers(headers(&[("x-ratelimit-reset", "1784999990")]), now);
        assert_eq!(wait, None);
        let (_, wait) = parse_rate_headers(
            headers(&[("x-ratelimit-reset", "1784999990"), ("retry-after", "60")]),
            now,
        );
        assert_eq!(wait, Some(Duration::from_secs(60)));
    }

    #[test]
    fn the_longer_of_the_reset_and_retry_after_counts() {
        let now = 1_785_000_000;
        let (_, wait) = parse_rate_headers(
            headers(&[("x-ratelimit-reset", "1784999999"), ("retry-after", "42")]),
            now,
        );
        assert_eq!(wait, Some(Duration::from_secs(42)), "a skewed reset does not hide the 42");
        let (_, wait) = parse_rate_headers(
            headers(&[("x-ratelimit-reset", "1785000050"), ("retry-after", "20")]),
            now,
        );
        assert_eq!(wait, Some(Duration::from_secs(50)));
    }

    #[test]
    fn the_limit_header_is_read() {
        assert_eq!(parse_rate_limit(headers(&[("x-ratelimit-limit", "90")])), Some(90));
        assert_eq!(parse_rate_limit(headers(&[("x-ratelimit-limit", "0")])), None);
        assert_eq!(parse_rate_limit(headers(&[])), None);
    }

    #[tokio::test(start_paused = true)]
    async fn the_bucket_follows_the_servers_limit_up_to_its_ceiling() {
        let limiter = RateLimiter::per_minute_up_to(30, 90);
        limiter.observe_limit(90).await;
        // Drain what was counted, then a minute refills 90, not 30.
        for _ in 0..30 {
            limiter.acquire().await;
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
        assert!((limiter.available().await - 90.0).abs() < 0.5);
        // Past the ceiling is held to it, and the degraded 30 shrinks the bucket again.
        limiter.observe_limit(500).await;
        assert!(limiter.available().await <= 90.0);
        limiter.observe_limit(30).await;
        assert!(limiter.available().await <= 30.0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_screen_goes_before_background_work_waiting_for_the_same_token() {
        let limiter = std::sync::Arc::new(RateLimiter::per_minute(60));
        for _ in 0..60 {
            limiter.acquire().await;
        }
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let low = {
            let (limiter, order) = (limiter.clone(), order.clone());
            tokio::spawn(async move {
                limiter.acquire_with(Priority::Background).await;
                order.lock().unwrap().push("low");
            })
        };
        tokio::time::sleep(Duration::from_millis(10)).await;
        let screen = {
            let (limiter, order) = (limiter.clone(), order.clone());
            tokio::spawn(async move {
                limiter.acquire().await;
                order.lock().unwrap().push("screen");
            })
        };
        let _ = tokio::join!(low, screen);
        assert_eq!(*order.lock().unwrap(), ["screen", "low"]);
    }

    /// A limiter as AniList's degraded minute has it: thirty, said by the server.
    async fn degraded() -> RateLimiter {
        let limiter = RateLimiter::per_minute_up_to(90, 90);
        limiter.observe_limit(30).await;
        limiter
    }

    #[tokio::test(start_paused = true)]
    async fn background_work_goes_at_a_sixth_of_the_limit_after_a_quiet_moment() {
        let limiter = degraded().await;
        limiter.acquire().await;
        let start = Instant::now();
        let mut went = Vec::new();
        for _ in 0..4 {
            limiter.acquire_with(Priority::Background).await;
            went.push(start.elapsed().as_secs());
        }
        // Ten seconds of quiet after the screen's request, then one every twelve: 30 / 6 is five
        // a minute.
        assert_eq!(went, [10, 22, 34, 46]);
        // At ninety a minute, one every four.
        limiter.observe_limit(90).await;
        limiter.acquire_with(Priority::Background).await;
        let start = Instant::now();
        limiter.acquire_with(Priority::Background).await;
        assert_eq!(start.elapsed().as_secs(), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn background_work_never_takes_the_third_kept_for_screens() {
        let limiter = degraded().await;
        limiter.set_pacing(Pacing { quiet: Duration::ZERO, ..Pacing::APP });
        // The server says twelve are left in a window just begun: background work may take two,
        // and leaves ten, a third of thirty, for screens.
        limiter.observe(Some(29), None).await;
        limiter.observe(Some(12), None).await;
        let start = Instant::now();
        limiter.acquire_with(Priority::Background).await;
        limiter.acquire_with(Priority::Background).await;
        assert_eq!(start.elapsed().as_secs(), 12);
        let third = tokio::time::timeout(
            Duration::from_secs(30),
            limiter.acquire_with(Priority::Background),
        )
        .await;
        assert!(third.is_err(), "the reserve holds until the window ends");
        // Screens still have their ten.
        let start = Instant::now();
        for _ in 0..10 {
            limiter.acquire().await;
        }
        assert!(start.elapsed() < Duration::from_secs(1), "{:?}", start.elapsed());
        // The window ends a minute after it began, and background work goes again.
        let start = Instant::now();
        limiter.acquire_with(Priority::Background).await;
        assert!(start.elapsed() <= Duration::from_secs(30), "{:?}", start.elapsed());
    }

    #[tokio::test(start_paused = true)]
    async fn the_launch_holds_background_work_until_its_screens_are_answered() {
        let limiter = std::sync::Arc::new(degraded().await);
        limiter.set_pacing(Pacing { quiet: Duration::ZERO, ..Pacing::APP });
        limiter.launching();
        let start = Instant::now();
        let waiting = {
            let limiter = limiter.clone();
            tokio::spawn(async move {
                limiter.acquire_with(Priority::Background).await;
                start.elapsed().as_secs()
            })
        };
        tokio::time::sleep(Duration::from_secs(3)).await;
        limiter.launched();
        // Five seconds after the screens.
        assert_eq!(waiting.await.unwrap(), 8);
        // A launch whose screens never say so ends after fifteen.
        limiter.launching();
        let start = Instant::now();
        limiter.launch_over().await;
        assert_eq!(start.elapsed().as_secs(), 15);
    }

    #[tokio::test(start_paused = true)]
    async fn work_for_the_page_on_screen_goes_after_screens_and_before_the_queue() {
        let limiter = std::sync::Arc::new(degraded().await);
        // Spent: every request waits for a token, two seconds apart.
        for _ in 0..30 {
            limiter.acquire().await;
        }
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let go = |priority: Priority, name: &'static str| {
            let (limiter, order) = (limiter.clone(), order.clone());
            tokio::spawn(async move {
                limiter.acquire_with(priority).await;
                order.lock().unwrap().push(name);
            })
        };
        let background = go(Priority::Background, "background");
        tokio::time::sleep(Duration::from_millis(10)).await;
        let on_screen = go(Priority::OnScreen, "on screen");
        tokio::time::sleep(Duration::from_millis(10)).await;
        let screen = go(Priority::Screen, "screen");
        let _ = tokio::join!(background, on_screen, screen);
        assert_eq!(*order.lock().unwrap(), ["screen", "on screen", "background"]);
    }

    #[tokio::test(start_paused = true)]
    async fn work_for_the_page_on_screen_skips_the_quiet_and_the_spacing() {
        let limiter = degraded().await;
        limiter.acquire().await;
        let start = Instant::now();
        for _ in 0..3 {
            limiter.acquire_with(Priority::OnScreen).await;
        }
        assert!(start.elapsed() < Duration::from_millis(5), "{:?}", start.elapsed());
    }

    #[tokio::test(start_paused = true)]
    async fn a_raised_request_leaves_the_queue() {
        let limiter = std::sync::Arc::new(degraded().await);
        limiter.acquire().await;
        let class = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let start = Instant::now();
        let waiting = {
            let (limiter, class) = (limiter.clone(), class.clone());
            tokio::spawn(async move {
                limiter
                    .acquire_by(|| {
                        if class.load(Ordering::SeqCst) {
                            Priority::OnScreen
                        } else {
                            Priority::Background
                        }
                    })
                    .await;
                start.elapsed()
            })
        };
        tokio::time::sleep(Duration::from_secs(2)).await;
        class.store(true, Ordering::SeqCst);
        let waited = waiting.await.unwrap();
        assert!(waited < Duration::from_secs(3), "went as it was raised, not after the quiet");
    }

    #[tokio::test(start_paused = true)]
    async fn background_requests_go_in_the_order_they_asked() {
        let limiter = std::sync::Arc::new(degraded().await);
        limiter.set_pacing(Pacing { quiet: Duration::ZERO, ..Pacing::APP });
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut tasks = Vec::new();
        for n in 0..4 {
            let (limiter, order) = (limiter.clone(), order.clone());
            tasks.push(tokio::spawn(async move {
                limiter.acquire_with(Priority::Background).await;
                order.lock().unwrap().push(n);
            }));
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(*order.lock().unwrap(), [0, 1, 2, 3]);
    }

    #[tokio::test(start_paused = true)]
    async fn nothing_left_said_once_paces_and_said_twice_holds_until_the_window_ends() {
        let limiter = degraded().await;
        // The window's first answer, then twenty seconds on, one node says nothing is left.
        limiter.observe(Some(29), None).await;
        tokio::time::sleep(Duration::from_secs(20)).await;
        limiter.observe(Some(0), None).await;
        let wait = limiter.would_wait().await.expect("the bucket is spent");
        assert!(wait <= Duration::from_secs(2), "once is paced, not held: {wait:?}");
        // Said again: held until the window's start plus 61 seconds, 41 from now.
        tokio::time::sleep(Duration::from_secs(1)).await;
        limiter.observe(Some(0), None).await;
        let wait = limiter.would_wait().await.expect("held");
        assert_eq!(wait.as_secs(), 40, "{wait:?}");
        let start = Instant::now();
        limiter.acquire().await;
        assert_eq!(start.elapsed().as_secs(), 40);
    }

    #[tokio::test(start_paused = true)]
    async fn with_no_window_known_nothing_left_twice_holds_a_minute() {
        let limiter = degraded().await;
        limiter.observe(Some(5), None).await;
        limiter.observe(Some(0), None).await;
        limiter.observe(Some(0), None).await;
        let wait = limiter.would_wait().await.expect("held");
        assert_eq!(wait, Duration::from_secs(60));
    }

    #[tokio::test(start_paused = true)]
    async fn a_refusal_holds_as_long_as_it_says_and_begins_the_next_window() {
        let limiter = degraded().await;
        limiter.observe(Some(29), None).await;
        // A 429 with a retry-after of 7: held 7, however much the window had left.
        limiter.back_off(Duration::from_secs(7)).await;
        assert_eq!(limiter.would_wait().await, Some(Duration::from_secs(7)));
        tokio::time::sleep(Duration::from_secs(7)).await;
        // The first answer after it begins a window: nothing left twice holds until its end.
        limiter.observe(Some(25), None).await;
        tokio::time::sleep(Duration::from_secs(30)).await;
        limiter.observe(Some(0), None).await;
        limiter.observe(Some(0), None).await;
        assert_eq!(limiter.would_wait().await, Some(Duration::from_secs(31)));
    }
}
