//! The capabilities the host lends to a guest.
//!
//! Everything a plugin can do passes through here, which makes this the place where the sandbox
//! is real rather than declared. Three rules shape it:
//!
//! - **The guest never sees a socket.** [`Capabilities::fetch`] takes a request and returns
//!   bytes; the connection and the rate limiter both belong to the host.
//! - **Redirects are not followed.** A plugin allowed `example.com` must not be able to reach
//!   `evil.test` because `example.com` chose to `302` there. The redirect is handed back as a
//!   response, so the guest can decide — and the next hop is allowlisted again.
//! - **Every ceiling is counted host-side.** Fetch counts and body sizes are tracked here, not
//!   trusted to the guest.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use crate::sandbox::{self, Limits};

/// Why a host call failed, in the vocabulary the WIT world uses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostError {
    /// The URL was not permitted by the plugin's manifest.
    #[error("denied: {0}")]
    Denied(String),
    #[error("timed out")]
    Timeout,
    #[error("transport: {0}")]
    Transport(String),
}

/// One HTTP exchange, as a guest describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Headers a guest may not set.
///
/// Not paranoia about the guest so much as about *correctness*: `accept-encoding` has to agree
/// with the handshake the host negotiated, and `host`, `content-length`, `connection` and
/// `transfer-encoding` are the client's to compute. Everything else, including `user-agent` and
/// `cookie`, is the guest's to set.
const RESERVED_HEADERS: &[&str] =
    &["host", "content-length", "connection", "transfer-encoding", "accept-encoding"];

/// What the host lends to one plugin, for the duration of one call.
///
/// Constructed per call rather than per plugin, because the fetch counter has to reset — a
/// long-lived counter would let the twelfth search of a session fail for no reason the user
/// could understand.
pub struct Capabilities {
    plugin_id: String,
    allowed_hosts: Vec<String>,
    limits: Limits,
    fetches: Arc<AtomicU32>,
    /// What this call's fetches met, shared with whoever made the call.
    reach: Reach,
    /// `None` in tests that exercise policy without a network.
    http: Option<crate::HttpClient>,
    /// This plugin's section of the user's configuration, served one key at a time through
    /// `config-get`. Read-only pairs the host does not interpret — how a login-gated source
    /// receives an API key without the guest gaining any filesystem access.
    settings: std::collections::BTreeMap<String, String>,
}

/// Whether one call's fetches got through to a server. A guest whose every request failed on
/// the way out can only guess at why; the host knows it was the network.
#[derive(Debug, Clone, Default)]
pub struct Reach(Arc<Reached>);

#[derive(Debug, Default)]
struct Reached {
    answered: AtomicU32,
    unsent: AtomicU32,
}

impl Reach {
    fn note(&self, sent: &Result<Response, HostError>) {
        match sent {
            Ok(_) => self.0.answered.fetch_add(1, Ordering::Relaxed),
            Err(HostError::Transport(_)) => self.0.unsent.fetch_add(1, Ordering::Relaxed),
            Err(_) => return,
        };
    }

    /// Whether the call asked the network and no request reached a server: every fetch failed
    /// on the way, as it does with no connection.
    pub fn never_reached(&self) -> bool {
        self.0.unsent.load(Ordering::Relaxed) > 0
            && self.0.answered.load(Ordering::Relaxed) == 0
    }
}

impl Capabilities {
    pub fn new(
        plugin_id: impl Into<String>,
        allowed_hosts: Vec<String>,
        limits: Limits,
        http: Option<crate::HttpClient>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            allowed_hosts,
            limits,
            fetches: Arc::new(AtomicU32::new(0)),
            reach: Reach::default(),
            http,
            settings: Default::default(),
        }
    }

    /// Attach the user's settings for this plugin. Absent by default, because most calls —
    /// and every `describe` — must work without any.
    pub fn with_settings(
        mut self,
        settings: std::collections::BTreeMap<String, String>,
    ) -> Self {
        self.settings = settings;
        self
    }

    /// One value from this plugin's settings, for the lent `config-get`.
    pub fn setting(&self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn fetches_used(&self) -> u32 {
        self.fetches.load(Ordering::Relaxed)
    }

    /// What this call's fetches meet, readable after the call by whoever kept a copy.
    pub fn reach(&self) -> Reach {
        self.reach.clone()
    }

    /// Check a request against policy without performing it.
    ///
    /// Separated from [`Self::fetch`] so the rules are testable without a network, and so the
    /// order is unambiguous: budget, then scheme and host, then headers.
    pub fn authorise(&self, request: &Request) -> Result<(), HostError> {
        let used = self.fetches.load(Ordering::Relaxed);
        if used >= self.limits.max_fetches {
            return Err(HostError::Denied(format!(
                "fetch budget exhausted ({} per call)",
                self.limits.max_fetches
            )));
        }

        // GET and POST are what parsing needs. Anything else is a way to change state on a
        // remote site, which a provider plugin has no business doing.
        let method = request.method.to_ascii_uppercase();
        if !matches!(method.as_str(), "GET" | "POST") {
            return Err(HostError::Denied(format!("method {method} is not permitted")));
        }

        if !sandbox::is_allowed(&request.url, &self.allowed_hosts) {
            let host = sandbox::host_of(&request.url).unwrap_or_else(|| "?".into());
            return Err(HostError::Denied(format!(
                "{host:?} is not in {}'s allowed-hosts",
                self.plugin_id
            )));
        }

        if let Some(name) = request
            .headers
            .iter()
            .map(|(n, _)| n.to_ascii_lowercase())
            .find(|n| RESERVED_HEADERS.contains(&n.as_str()))
        {
            return Err(HostError::Denied(format!("header {name:?} is set by the host")));
        }

        Ok(())
    }

    /// QA ONLY, never merged: [`Self::fetch`] for a page image, which waits in the QA gate
    /// behind the plugin's own calls.
    pub(crate) async fn qa_fetch_page(&self, request: Request) -> Result<Response, HostError> {
        QA_PAGE.scope((), self.fetch(request)).await
    }

    /// Perform a request on the guest's behalf.
    pub async fn fetch(&self, request: Request) -> Result<Response, HostError> {
        self.authorise(&request)?;
        self.fetches.fetch_add(1, Ordering::Relaxed);

        let sent = match &self.http {
            Some(http) => self.send(http, request).await,
            None => Err(HostError::Transport("no http client in this host".into())),
        };
        self.reach.note(&sent);
        sent
    }

    /// Without `fetch` there is no client, so there is nothing to send with.
    #[cfg(not(feature = "fetch"))]
    async fn send(
        &self,
        http: &crate::HttpClient,
        _request: Request,
    ) -> Result<Response, HostError> {
        match *http {}
    }

    #[cfg(feature = "fetch")]
    async fn send(
        &self,
        http: &crate::HttpClient,
        request: Request,
    ) -> Result<Response, HostError> {
        tracing::debug!(plugin = %self.plugin_id, url = %request.url, "plugin fetch");
        // QA ONLY, never merged: at most one request per 4 s per plugin (KAERI_QA_GATE_MS),
        // plugin calls ahead of page images.
        qa_gate(&self.plugin_id, qa_gap(), QA_PAGE.try_with(|_| ()).is_ok()).await;
        // QA ONLY: a poor network for plugin fetches, from the file KAERI_QA_NET names:
        // "offline" fails at once, "hang" waits out the deadline, "delay <ms>" adds a wait,
        // "drop <percent>" fails that share of requests.
        if let Ok(path) = qa_path("KAERI_QA_NET", "qa-net") {
            let rule = std::fs::read_to_string(path).unwrap_or_default();
            let mut words = rule.split_whitespace();
            let arg = |w: Option<&str>| w.and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            match words.next() {
                Some("offline") => return Err(HostError::Transport("qa offline".into())),
                Some("hang") => {
                    tokio::time::sleep(self.limits.deadline).await;
                    return Err(HostError::Timeout);
                }
                Some("delay") => tokio::time::sleep(std::time::Duration::from_millis(arg(words.next()))).await,
                Some("drop") => {
                    let ns = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as u64)
                        .unwrap_or(0);
                    if ns % 100 < arg(words.next()) {
                        return Err(HostError::Transport("qa drop".into()));
                    }
                }
                _ => {}
            }
        }
        if let Ok(path) = qa_path("KAERI_QA_FETCH_LOG", "qa-fetch.log") {
            use std::io::Write as _;
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path)
            {
                let _ = writeln!(f, "{ms} {} {}", self.plugin_id, request.url.split('/').nth(2).unwrap_or("-"));
            }
        }

        // The same client the native sources use, so one place owns the connection settings.
        let mut builder = match request.method.to_ascii_uppercase().as_str() {
            "POST" => http.sources().post(&request.url),
            _ => http.sources().get(&request.url),
        };
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }

        let response = tokio::time::timeout(self.limits.deadline, builder.send())
            .await
            .map_err(|_| HostError::Timeout)?
            .map_err(|e| HostError::Transport(e.to_string()))?;

        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                Some((name.to_string(), value.to_str().ok()?.to_owned()))
            })
            .collect();

        let body = read_capped(response, self.limits.max_response_bytes).await?;
        Ok(Response { status, headers, body })
    }
}

/// QA ONLY, never merged: the env path, or on Android a file in the QA app's own folder.
#[cfg(feature = "fetch")]
fn qa_path(var: &str, file: &str) -> Result<String, std::env::VarError> {
    std::env::var(var).or_else(|e| {
        if cfg!(target_os = "android") {
            Ok(format!("/data/user/0/app.kaeri.app.qa/files/{file}"))
        } else {
            Err(e)
        }
    })
}

/// QA ONLY, never merged: the gap between two requests of one plugin, 4 s unless
/// KAERI_QA_GATE_MS sets it in milliseconds.
#[cfg(feature = "fetch")]
fn qa_gap() -> std::time::Duration {
    std::env::var("KAERI_QA_GATE_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(std::time::Duration::from_secs(4), std::time::Duration::from_millis)
}

tokio::task_local! {
    /// QA ONLY, never merged: set while a page image is fetched.
    static QA_PAGE: ();
}

/// QA ONLY, never merged: one plugin's turn: when its last request went out, and who waits.
#[cfg(feature = "fetch")]
#[derive(Default)]
struct QaTurn {
    queue: std::sync::Mutex<QaQueue>,
    moved: tokio::sync::Notify,
}

#[cfg(feature = "fetch")]
#[derive(Default)]
struct QaQueue {
    last: Option<tokio::time::Instant>,
    /// Waiters in turn order: plugin calls (`false`) ahead of page images, each in arrival order.
    waiting: std::collections::BTreeSet<(bool, u64)>,
    tickets: u64,
}

#[cfg(feature = "fetch")]
impl QaTurn {
    fn queue(&self) -> std::sync::MutexGuard<'_, QaQueue> {
        self.queue.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// QA ONLY, never merged: the turn for a plugin, the same one for every call with its id.
#[cfg(feature = "fetch")]
fn qa_turn(plugin: &str) -> std::sync::Arc<QaTurn> {
    static TURNS: std::sync::LazyLock<
        std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<QaTurn>>>,
    > = std::sync::LazyLock::new(Default::default);
    let mut turns = TURNS.lock().unwrap_or_else(|e| e.into_inner());
    turns.entry(plugin.to_owned()).or_default().clone()
}

/// QA ONLY, never merged: a place in the queue, given up when the waiter is dropped.
#[cfg(feature = "fetch")]
struct QaPlace<'a> {
    turn: &'a QaTurn,
    key: (bool, u64),
}

#[cfg(feature = "fetch")]
impl Drop for QaPlace<'_> {
    fn drop(&mut self) {
        self.turn.queue().waiting.remove(&self.key);
        self.turn.moved.notify_waiters();
    }
}

/// QA ONLY, never merged: waits for the first place in the plugin's queue and `gap` after its
/// last request that went out, then marks this one as sent. Plugin calls go ahead of page
/// images, so an open or a Try again is next whatever a strip has queued. Only a waiter that
/// gets through marks a send: one dropped or cancelled while it waits gives up its place and
/// the next waiter starts on time.
#[cfg(feature = "fetch")]
async fn qa_gate(plugin: &str, gap: std::time::Duration, page: bool) {
    let turn = qa_turn(plugin);
    let key = {
        let mut queue = turn.queue();
        queue.tickets += 1;
        let key = (page, queue.tickets);
        queue.waiting.insert(key);
        key
    };
    let _place = QaPlace { turn: &turn, key };
    loop {
        let moved = turn.moved.notified();
        tokio::pin!(moved);
        moved.as_mut().enable();
        let wake = {
            let mut queue = turn.queue();
            if queue.waiting.first() != Some(&key) {
                None
            } else {
                let now = tokio::time::Instant::now();
                match queue.last.map(|sent| sent + gap).filter(|at| *at > now) {
                    Some(at) => Some(at),
                    None => {
                        queue.last = Some(now);
                        drop(queue);
                        // Dropping the place takes it out of the queue and wakes the next.
                        return;
                    }
                }
            }
        };
        match wake {
            Some(at) => tokio::select! {
                () = tokio::time::sleep_until(at) => {}
                () = &mut moved => {}
            },
            None => moved.await,
        }
    }
}

#[cfg(all(test, feature = "fetch"))]
mod qa_gate_tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::time::Instant;

    const GAP: Duration = Duration::from_secs(4);
    /// How long the reader gives a page try, as kaeri-engine's `PAGE_WAIT`.
    const PAGE_WAIT: Duration = Duration::from_secs(15);

    fn ms(from: Instant, to: Instant) -> u64 {
        (to - from).as_millis() as u64
    }

    async fn call(plugin: &str) {
        super::qa_gate(plugin, GAP, false).await;
    }

    async fn page(plugin: &str) {
        super::qa_gate(plugin, GAP, true).await;
    }

    /// `n` waiters spawned now, each with `deadline`, recording when they went out.
    fn burst(
        plugin: &'static str,
        n: usize,
        is_page: bool,
        deadline: Duration,
        start: Instant,
        sent: &Arc<Mutex<Vec<u64>>>,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        (0..n)
            .map(|_| {
                let sent = sent.clone();
                tokio::spawn(async move {
                    let gate = super::qa_gate(plugin, GAP, is_page);
                    if tokio::time::timeout(deadline, gate).await.is_ok() {
                        sent.lock().unwrap().push(ms(start, Instant::now()));
                    }
                })
            })
            .collect()
    }

    #[tokio::test(start_paused = true)]
    async fn requests_of_one_plugin_go_out_a_gap_apart_and_other_plugins_wait_for_none() {
        let start = Instant::now();
        let mut at = Vec::new();
        for _ in 0..3 {
            call("qa-spacing-a").await;
            at.push(ms(start, Instant::now()));
        }
        assert_eq!(at, [0, 4000, 8000]);
        call("qa-spacing-b").await;
        assert_eq!(ms(start, Instant::now()), 8000, "another plugin waits for none");
        tokio::time::sleep(Duration::from_secs(10)).await;
        let later = Instant::now();
        page("qa-spacing-a").await;
        assert_eq!(Instant::now(), later, "a quiet plugin starts at once");
        super::qa_gate("qa-spacing-a", Duration::from_secs(8), false).await;
        assert_eq!(ms(later, Instant::now()), 8000, "the gap is the caller's");
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiter_dropped_by_its_deadline_does_not_delay_the_next_request() {
        let start = Instant::now();
        call("qa-dropped").await;
        // Its turn would come at 4 s, but its deadline drops it at 1 s.
        let dropped = tokio::time::timeout(Duration::from_secs(1), call("qa-dropped")).await;
        assert!(dropped.is_err());
        tokio::time::sleep_until(start + Duration::from_secs(5)).await;
        call("qa-dropped").await;
        assert_eq!(ms(start, Instant::now()), 5000, "goes at once, 4 s after the last sent");
        call("qa-dropped").await;
        assert_eq!(ms(start, Instant::now()), 9000);
    }

    #[tokio::test(start_paused = true)]
    async fn a_burst_of_25_goes_out_4_s_apart_and_dropped_ones_free_their_turns() {
        let start = Instant::now();
        let sent = Arc::new(Mutex::new(Vec::new()));
        for task in burst("qa-burst", 25, true, Duration::MAX, start, &sent) {
            task.await.unwrap();
        }
        let went = std::mem::take(&mut *sent.lock().unwrap());
        assert_eq!(went, (0..25).map(|i| i * 4000).collect::<Vec<u64>>());

        // A chapter's pages with a 30 s deadline each: eight go out, the rest give up, and a
        // retry after them goes out one gap after the last one sent, not minutes later.
        tokio::time::sleep(Duration::from_secs(10)).await;
        let start = Instant::now();
        for task in burst("qa-burst", 25, true, Duration::from_secs(30), start, &sent) {
            task.await.unwrap();
        }
        let went = std::mem::take(&mut *sent.lock().unwrap());
        assert_eq!(went, (0..8).map(|i| i * 4000).collect::<Vec<u64>>());
        assert_eq!(ms(start, Instant::now()), 30_000);
        page("qa-burst").await;
        assert_eq!(ms(start, Instant::now()), 32_000, "the retry waits only for the gap");
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiter_cancelled_mid_sleep_frees_its_turn() {
        let start = Instant::now();
        call("qa-cancel").await;
        let sleeper = tokio::spawn(async move {
            call("qa-cancel").await;
            ms(start, Instant::now())
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
        let behind = tokio::spawn(async move {
            call("qa-cancel").await;
            ms(start, Instant::now())
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
        // The first waiter is next and sleeps until 4 s; cancel it halfway.
        sleeper.abort();
        assert!(sleeper.await.unwrap_err().is_cancelled());
        assert_eq!(behind.await.unwrap(), 4000, "the one behind takes the freed turn on time");
        call("qa-cancel").await;
        assert_eq!(ms(start, Instant::now()), 8000);
    }

    #[tokio::test(start_paused = true)]
    async fn an_open_and_a_try_again_go_out_next_while_a_strip_waits() {
        // A strip queues 28 pages, each given two tries of PAGE_WAIT as the reader does.
        let start = Instant::now();
        let sent = Arc::new(Mutex::new(Vec::new()));
        let strip = burst("qa-strip", 28, true, PAGE_WAIT * 2, start, &sent);
        tokio::time::sleep(Duration::from_secs(1)).await;
        // Opening the next chapter is a plugin call: it goes at the next turn, 4 s, not
        // after the strip.
        tokio::time::timeout(PAGE_WAIT, call("qa-strip")).await.expect("the open went out");
        assert_eq!(ms(start, Instant::now()), 4000);
        tokio::time::sleep_until(start + Duration::from_secs(9)).await;
        tokio::time::timeout(PAGE_WAIT, call("qa-strip")).await.expect("a Try again went out");
        assert_eq!(ms(start, Instant::now()), 12_000);
        for task in strip {
            task.await.unwrap();
        }
        let went = std::mem::take(&mut *sent.lock().unwrap());
        assert_eq!(went, [0, 8000, 16_000, 20_000, 24_000, 28_000], "pages between them");
        // The pages that gave up held nothing: a page's Try again goes out at once.
        tokio::time::sleep_until(start + Duration::from_secs(40)).await;
        tokio::time::timeout(PAGE_WAIT, page("qa-strip")).await.expect("the page went out");
        assert_eq!(ms(start, Instant::now()), 40_000);
    }
}

#[cfg(feature = "fetch")]
/// A response's body, refused as soon as it is known to pass `ceiling` bytes: from its
/// Content-Length when it says one, else as it arrives. Read whole first, a body could fill the
/// host's memory with as much as the client's timeout let through, none of it counted against
/// the plugin's own memory ceiling.
async fn read_capped(
    response: reqwest::Response,
    ceiling: usize,
) -> Result<Vec<u8>, HostError> {
    use futures::StreamExt;

    let too_large = |size: u64| {
        HostError::Denied(format!(
            "response of {size} bytes exceeds the {ceiling} byte ceiling"
        ))
    };
    let declared = response.content_length();
    if let Some(declared) = declared
        && declared > ceiling as u64
    {
        return Err(too_large(declared));
    }
    let mut body = Vec::with_capacity(declared.map_or(0, |n| n as usize));
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|e| HostError::Transport(e.to_string()))?;
        if body.len() + chunk.len() > ceiling {
            return Err(too_large((body.len() + chunk.len()) as u64));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Capture groups for every match of `pattern` in `haystack`.
///
/// Group 0 is the whole match. A group that did not participate yields an empty string rather
/// than shifting the others along, so a guest can index by group number.
///
/// An invalid pattern returns no matches rather than an error: a guest cannot do anything useful
/// with a regex-compilation failure, and the WIT signature keeps it simple.
/// Matches one `regex-captures` call may return.
///
/// A parser pulling stream URLs out of a page wants tens. Anything past this is a guest using
/// the host's allocator rather than parsing, and the answer is no longer useful either way.
const MAX_REGEX_MATCHES: usize = 10_000;

pub fn regex_captures(pattern: &str, haystack: &str) -> Vec<Vec<String>> {
    // Bounded so a pathological pattern cannot spend the plugin's whole deadline compiling.
    let Ok(re) = regex::RegexBuilder::new(pattern).size_limit(1 << 20).build() else {
        tracing::debug!(pattern, "plugin supplied an invalid or oversized regex");
        return Vec::new();
    };
    // Bounded output as well as bounded compilation. The pattern, the haystack and therefore
    // the number of matches are all the guest's to choose, and every match allocates on the
    // *host* side — where neither containment mechanism reaches. The memory limiter governs
    // guest linear memory, not this; and epoch interruption cannot fire while the guest is not
    // executing, which it is not for the duration of a host call. `a?` over a full 64 MiB of
    // guest memory matched at every byte and cost gigabytes of host heap.
    let mut out = Vec::new();
    for caps in re.captures_iter(haystack) {
        if out.len() >= MAX_REGEX_MATCHES {
            tracing::warn!(
                pattern,
                limit = MAX_REGEX_MATCHES,
                "truncating a plugin's regex results"
            );
            break;
        }
        out.push(
            caps.iter()
                .map(|group| group.map(|m| m.as_str().to_owned()).unwrap_or_default())
                .collect(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body with no length that never ends is refused once it passes the ceiling, rather than
    /// read until the client's timeout.
    #[cfg(feature = "fetch")]
    #[tokio::test]
    async fn a_body_past_the_ceiling_is_refused_as_it_arrives() {
        let endless = futures::stream::repeat_with(|| Ok::<_, std::io::Error>(vec![0u8; 1024]));
        let response: reqwest::Response =
            http::Response::new(reqwest::Body::wrap_stream(endless)).into();
        let error = read_capped(response, 4 * 1024).await.unwrap_err();
        assert!(
            matches!(error, HostError::Denied(ref m) if m.contains("4096 byte")),
            "{error}"
        );

        let small: reqwest::Response =
            http::Response::new(reqwest::Body::from(vec![1u8; 10])).into();
        assert_eq!(read_capped(small, 4 * 1024).await.unwrap(), vec![1u8; 10]);
    }

    fn caps() -> Capabilities {
        Capabilities::new("example-plugin", vec!["example.com".into()], Limits::default(), None)
    }

    fn get(url: &str) -> Request {
        Request { method: "GET".into(), url: url.into(), headers: Vec::new(), body: None }
    }

    #[test]
    fn a_declared_host_is_authorised() {
        assert_eq!(caps().authorise(&get("https://example.com/api")), Ok(()));
    }

    #[test]
    fn an_undeclared_host_is_denied_by_name() {
        // The message names the plugin, because the user's question when this fires is "which
        // plugin is trying to reach that?".
        let error = caps().authorise(&get("https://evil.test/")).unwrap_err();
        assert!(
            matches!(error, HostError::Denied(ref m) if m.contains("example-plugin")),
            "{error}"
        );
        assert!(error.to_string().contains("evil.test"), "{error}");
    }

    #[test]
    fn only_get_and_post_are_permitted() {
        // A parser has no reason to change state on a remote site.
        for method in ["DELETE", "PUT", "PATCH", "CONNECT", "TRACE"] {
            let request = Request { method: method.into(), ..get("https://example.com/") };
            assert!(
                matches!(caps().authorise(&request), Err(HostError::Denied(_))),
                "{method} should be denied"
            );
        }
        for method in ["GET", "get", "POST", "post"] {
            let request = Request { method: method.into(), ..get("https://example.com/") };
            assert_eq!(caps().authorise(&request), Ok(()), "{method} should be allowed");
        }
    }

    #[test]
    fn a_guest_cannot_override_the_hosts_fingerprint_headers() {
        // A guest setting `accept-encoding` would make the headers disagree with the
        // handshake the host negotiated, and the response would fail to decode.
        for name in ["user-agent", "Accept-Encoding", "HOST", "content-length"] {
            let request = Request {
                headers: vec![(name.into(), "x".into())],
                ..get("https://example.com/")
            };
            let result = caps().authorise(&request);
            if name.eq_ignore_ascii_case("user-agent") {
                // Deliberately allowed: some sources key on a specific UA, and the host's
                // profile already sets a coherent one that the guest merely narrows.
                assert_eq!(result, Ok(()), "user-agent should be the guest's to set");
            } else {
                assert!(
                    matches!(result, Err(HostError::Denied(_))),
                    "{name} should be reserved"
                );
            }
        }
    }

    #[test]
    fn ordinary_headers_are_the_guests_to_set() {
        // Headers a source sets are passed through.
        let request = Request {
            headers: vec![
                ("referer".into(), "https://example.com/".into()),
                ("x-requested-with".into(), "XMLHttpRequest".into()),
            ],
            ..get("https://example.com/")
        };
        assert_eq!(caps().authorise(&request), Ok(()));
    }

    #[test]
    fn the_fetch_budget_is_counted_host_side() {
        // A guest cannot use the host's client as a request amplifier.
        let caps = Capabilities::new(
            "p",
            vec!["example.com".into()],
            Limits { max_fetches: 2, ..Limits::default() },
            None,
        );
        assert_eq!(caps.authorise(&get("https://example.com/")), Ok(()));
        caps.fetches.fetch_add(2, Ordering::Relaxed);
        let error = caps.authorise(&get("https://example.com/")).unwrap_err();
        assert!(error.to_string().contains("budget"), "{error}");
    }

    #[test]
    fn settings_are_absent_unless_attached() {
        // `describe` runs on capabilities with no settings, and that absence is deliberate:
        // a manifest must not depend on configuration it is about to be granted.
        let bare = caps();
        assert_eq!(bare.setting("api-key"), None);

        let configured = caps().with_settings(
            [("api-key".to_string(), "s3cret".to_string())].into_iter().collect(),
        );
        assert_eq!(configured.setting("api-key").as_deref(), Some("s3cret"));
        assert_eq!(configured.setting("unset"), None);
    }

    #[test]
    fn regex_returns_every_group_of_every_match() {
        let found = regex_captures(
            r#"file:\s*"([^"]+)".*?label:\s*"(\d+)p""#,
            r#"
            {file: "https://cdn.example.com/1080.m3u8", label: "1080p"},
            {file: "https://cdn.example.com/720.m3u8", label: "720p"}
        "#,
        );
        assert_eq!(found.len(), 2);
        assert_eq!(found[0][1], "https://cdn.example.com/1080.m3u8");
        assert_eq!(found[0][2], "1080");
        assert_eq!(found[1][2], "720");
    }

    #[test]
    fn a_pattern_that_matches_everywhere_does_not_allocate_without_bound() {
        // The pattern and the haystack are both the guest's, and every match allocates on the
        // *host* side — where neither containment mechanism reaches. The memory limiter
        // governs guest linear memory, and epoch interruption cannot fire while the guest is
        // not executing, which it is not for the whole of a host call. `a?` over a guest's
        // full 64 MiB matched at every byte and cost gigabytes of host heap.
        let haystack = "a".repeat(MAX_REGEX_MATCHES * 2);
        let found = regex_captures("a?", &haystack);
        assert_eq!(found.len(), MAX_REGEX_MATCHES, "output must be bounded");
    }

    #[test]
    fn a_non_participating_group_is_empty_rather_than_absent() {
        // So a guest can index by group number without counting which ones matched.
        let found = regex_captures(r"(a)|(b)", "a");
        assert_eq!(found[0], vec!["a", "a", ""]);
    }

    #[test]
    fn an_invalid_pattern_yields_no_matches_rather_than_an_error() {
        // A guest cannot do anything useful with a compilation failure.
        assert!(regex_captures(r"([unclosed", "anything").is_empty());
        assert!(regex_captures("", "").is_empty() || regex_captures("", "x").len() <= 2);
    }

    #[test]
    fn regex_has_no_catastrophic_backtracking_to_exploit() {
        // The `regex` crate is a finite automaton, so the classic nested-quantifier bomb is
        // linear rather than exponential. Worth asserting: a guest choosing the pattern makes
        // this a denial-of-service surface if the engine were a backtracker.
        let bomb = "(a+)+b";
        let haystack = "a".repeat(2_000);
        let started = std::time::Instant::now();
        let found = regex_captures(bomb, &haystack);
        assert!(found.is_empty());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "took {:?}",
            started.elapsed()
        );
    }
}
