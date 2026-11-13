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
        let plugin_id = plugin_id.into();
        // QA ONLY, never merged: the QA gate learns which hosts are this source's.
        #[cfg(feature = "fetch")]
        if http.is_some() {
            qa_register(&plugin_id, &allowed_hosts);
        }
        Self {
            plugin_id,
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
        // QA ONLY, never merged: at most one request per 4 s per source (KAERI_QA_GATE_MS),
        // plugin calls ahead of page images. The turn is held until the request has finished,
        // so whatever holds it below (the net rule's delay, a slow connect) moves the next
        // request's turn with it.
        let page = QA_PAGE.try_with(|_| ()).is_ok();
        let qa_turn = qa_gate(Some(&self.plugin_id), &request.url, qa_gap(), page).await;
        // QA ONLY (r8 Linux harness): a refused source stops for the rest of the session.
        if std::fs::metadata(format!("{}.stop", std::env::var("KAERI_QA_FETCH_LOG").unwrap_or_default())).is_ok() {
            return Err(HostError::Transport("qa: stopped".into()));
        }
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
                Some("delay") => {
                    tokio::time::sleep(std::time::Duration::from_millis(arg(words.next())))
                        .await
                }
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
        // QA ONLY: logged as it goes out, after any rule held it.
        qa_log(&self.plugin_id, &request.url);

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

        let sent = tokio::time::timeout(self.limits.deadline, builder.send()).await;
        // QA ONLY: the request has finished; the source's next turn counts from now.
        drop(qa_turn);
        let response = sent
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
        // QA ONLY (r8 Linux harness): log the answer, and stop the plugin after a refusal.
        {
            let head = String::from_utf8_lossy(&body[..body.len().min(4096)]).to_lowercase();
            let refused = status == 429
                || ((status == 403 || status == 503)
                    && (head.contains("cf-chl") || head.contains("just a moment")));
            if refused {
                let _ = std::fs::write(format!("{}.stop", std::env::var("KAERI_QA_FETCH_LOG").unwrap_or_default()), "refused");
            }
            if let Ok(path) = std::env::var("KAERI_QA_FETCH_LOG") {
                use std::io::Write as _;
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = writeln!(f, "{ms} {} ANSWER {status} {}{}", self.plugin_id, body.len(), if refused { " REFUSED" } else { "" });
                }
            }
        }
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

/// QA ONLY, never merged: the gap between two requests of one source, 4 s unless
/// KAERI_QA_GATE_MS sets it in milliseconds, plus 50 ms so the fetch log's wall clock never
/// reads a pair under the gap.
#[cfg(feature = "fetch")]
fn qa_gap() -> std::time::Duration {
    std::env::var("KAERI_QA_GATE_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(std::time::Duration::from_secs(4), std::time::Duration::from_millis)
        + std::time::Duration::from_millis(50)
}

tokio::task_local! {
    /// QA ONLY, never merged: set while a page image is fetched.
    static QA_PAGE: ();
}

/// QA ONLY, never merged: the gate's state for the whole process. Each source (a plugin id)
/// has a turn. A request takes the turn of its own plugin and of every plugin whose
/// allowed-hosts cover its host, so a source's API host, its CDN and a host two plugins share
/// all pace together, and Kaeri's own requests to a source's host (artwork) wait too.
#[cfg(feature = "fetch")]
#[derive(Default)]
struct QaGate {
    turns: std::collections::HashMap<String, QaTurn>,
    /// Each plugin's allowed-hosts, as loaded.
    hosts: std::collections::HashMap<String, Vec<String>>,
    /// Waiters in turn order with the turns each needs: plugin calls (`false`) ahead of page
    /// images, each in arrival order.
    waiting: std::collections::BTreeMap<(bool, u64), Vec<String>>,
    tickets: u64,
}

/// QA ONLY, never merged: one source's turn.
#[cfg(feature = "fetch")]
#[derive(Default)]
struct QaTurn {
    /// A request of this source is on its way and has not finished.
    out: bool,
    /// When the last request finished, which is after it went out.
    last: Option<tokio::time::Instant>,
    /// The last turn went to a plugin call ahead of a page image that was waiting longer.
    /// The page images then go in arrival order with the calls, so a loop of plugin calls
    /// (a series page asked again and again) cannot starve a download's pages: they get at
    /// least every other turn.
    jumped: bool,
}

#[cfg(feature = "fetch")]
fn qa_state() -> (std::sync::MutexGuard<'static, QaGate>, &'static tokio::sync::Notify) {
    static GATE: std::sync::LazyLock<(std::sync::Mutex<QaGate>, tokio::sync::Notify)> =
        std::sync::LazyLock::new(Default::default);
    (GATE.0.lock().unwrap_or_else(|e| e.into_inner()), &GATE.1)
}

/// QA ONLY, never merged: note a plugin's allowed-hosts, so a request to one of them from
/// anywhere waits for that plugin's turn.
#[cfg(feature = "fetch")]
pub(crate) fn qa_register(plugin: &str, allowed_hosts: &[String]) {
    if plugin.is_empty() || allowed_hosts.is_empty() {
        return;
    }
    let (mut gate, _) = qa_state();
    if gate.hosts.get(plugin).is_none_or(|known| known != allowed_hosts) {
        gate.hosts.insert(plugin.to_owned(), allowed_hosts.to_vec());
    }
}

/// QA ONLY, never merged: where a waiter stands in the queues it shares with others. Plugin
/// calls go first unless one of the waiter's turns last went to a call that jumped a page.
#[cfg(feature = "fetch")]
fn qa_rank(gate: &QaGate, (page, ticket): (bool, u64), keys: &[String]) -> (bool, u64) {
    let jumped = keys.iter().any(|k| gate.turns.get(k).is_some_and(|t| t.jumped));
    (page && !jumped, ticket)
}

/// QA ONLY, never merged: the turns a request to `url` takes, sorted.
#[cfg(feature = "fetch")]
fn qa_keys(gate: &QaGate, plugin: Option<&str>, url: &str) -> Vec<String> {
    let mut keys: Vec<String> = plugin.into_iter().map(str::to_owned).collect();
    keys.extend(
        gate.hosts
            .iter()
            .filter(|(_, hosts)| sandbox::is_allowed(url, hosts))
            .map(|(id, _)| id.clone()),
    );
    keys.sort();
    keys.dedup();
    keys
}

/// QA ONLY, never merged: a place in the queue, given up when the waiter is dropped.
#[cfg(feature = "fetch")]
struct QaPlace {
    key: (bool, u64),
    armed: bool,
}

#[cfg(feature = "fetch")]
impl Drop for QaPlace {
    fn drop(&mut self) {
        if self.armed {
            let (mut gate, moved) = qa_state();
            gate.waiting.remove(&self.key);
            drop(gate);
            moved.notify_waiters();
        }
    }
}

/// QA ONLY, never merged: a request that holds its source's turn. Hold it until the request
/// has finished; dropping it, however the request ended, records that instant as the source's
/// last send, so the next request goes out a gap after this one really went out, whatever
/// held it after the gate (a QA net rule's delay, a slow connect).
#[cfg(feature = "fetch")]
#[must_use = "the turn is the source's until this is dropped"]
pub struct QaSending {
    keys: Vec<String>,
}

#[cfg(feature = "fetch")]
impl Drop for QaSending {
    fn drop(&mut self) {
        let (mut gate, moved) = qa_state();
        let now = tokio::time::Instant::now();
        for key in &self.keys {
            let turn = gate.turns.entry(key.clone()).or_default();
            turn.out = false;
            turn.last = Some(now);
        }
        drop(gate);
        moved.notify_waiters();
    }
}

/// QA ONLY, never merged: waits until this request's turns are free, first in their queues
/// and `gap` past their last finished request, then holds them until the returned
/// [`QaSending`] is dropped. `None` when the request is no source's (no plugin, and no
/// plugin's allowed-hosts cover its host). Plugin calls go ahead of page images, so an open
/// or a Try again is next whatever a strip has queued. A waiter dropped or cancelled while it
/// waits gives up its place and records nothing.
#[cfg(feature = "fetch")]
pub async fn qa_gate(
    plugin: Option<&str>,
    url: &str,
    gap: std::time::Duration,
    page: bool,
) -> Option<QaSending> {
    let (key, keys) = {
        let (mut gate, _) = qa_state();
        let keys = qa_keys(&gate, plugin, url);
        if keys.is_empty() {
            return None;
        }
        gate.tickets += 1;
        let key = (page, gate.tickets);
        gate.waiting.insert(key, keys.clone());
        (key, keys)
    };
    let mut place = QaPlace { key, armed: true };
    let moved = qa_state().1;
    loop {
        let notified = moved.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let wake = {
            let (mut gate, _) = qa_state();
            let mine = qa_rank(&gate, key, &keys);
            let ahead = gate.waiting.iter().any(|(theirs, their_keys)| {
                *theirs != key
                    && their_keys.iter().any(|k| keys.contains(k))
                    && qa_rank(&gate, *theirs, their_keys) < mine
            });
            let busy = keys.iter().any(|k| gate.turns.get(k).is_some_and(|t| t.out));
            if ahead || busy {
                None
            } else {
                let now = tokio::time::Instant::now();
                let ready = keys
                    .iter()
                    .filter_map(|k| gate.turns.get(k).and_then(|t| t.last))
                    .map(|last| last + gap)
                    .max()
                    .filter(|at| *at > now);
                match ready {
                    Some(at) => Some(at),
                    None => {
                        // A call that goes while an older page image waits on one of its
                        // turns has jumped it; a page image that goes clears the mark.
                        let jumped = !page
                            && gate.waiting.iter().any(|((their_page, ticket), their_keys)| {
                                *their_page
                                    && *ticket < key.1
                                    && their_keys.iter().any(|k| keys.contains(k))
                            });
                        for k in &keys {
                            let turn = gate.turns.entry(k.clone()).or_default();
                            turn.out = true;
                            turn.jumped = jumped;
                        }
                        gate.waiting.remove(&key);
                        place.armed = false;
                        return Some(QaSending { keys });
                    }
                }
            }
        };
        match wake {
            Some(at) => tokio::select! {
                () = tokio::time::sleep_until(at) => {}
                () = &mut notified => {}
            },
            None => notified.await,
        }
    }
}

/// QA ONLY, never merged: [`qa_gate`] for a request Kaeri makes itself (artwork), with the
/// gap from KAERI_QA_GATE_MS.
#[cfg(feature = "fetch")]
pub async fn qa_gate_url(url: &str) -> Option<QaSending> {
    qa_gate(None, url, qa_gap(), true).await
}

/// QA ONLY, never merged: a line in the QA fetch log (KAERI_QA_FETCH_LOG), written just
/// before a request goes out: epoch ms, who asked, the host.
#[cfg(feature = "fetch")]
pub fn qa_log(who: &str, url: &str) {
    use std::io::Write as _;
    let Ok(path) = qa_path("KAERI_QA_FETCH_LOG", "qa-fetch.log") else { return };
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{ms} {who} {}", url.split('/').nth(2).unwrap_or("-"));
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

    fn url(plugin: &str) -> String {
        format!("https://{plugin}.example/x")
    }

    /// A plugin call that finishes the moment it goes out.
    async fn call(plugin: &str) {
        drop(super::qa_gate(Some(plugin), &url(plugin), GAP, false).await);
    }

    async fn page(plugin: &str) {
        drop(super::qa_gate(Some(plugin), &url(plugin), GAP, true).await);
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
                    let u = url(plugin);
                    let gate = super::qa_gate(Some(plugin), &u, GAP, is_page);
                    if let Ok(turn) = tokio::time::timeout(deadline, gate).await {
                        sent.lock().unwrap().push(ms(start, Instant::now()));
                        drop(turn);
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
        let u = url("qa-spacing-a");
        drop(super::qa_gate(Some("qa-spacing-a"), &u, Duration::from_secs(8), false).await);
        assert_eq!(ms(later, Instant::now()), 8000, "the gap is the caller's");
    }

    #[tokio::test(start_paused = true)]
    async fn a_request_held_after_the_gate_moves_the_next_turn_to_when_it_finished() {
        // A QA net rule "delay 9000" holds the first request after it passed the gate; it goes
        // out at 9 s. The next must go 4 s after that, not 4 s after the gate let the first by.
        let start = Instant::now();
        let held = super::qa_gate(Some("qa-held"), "https://api.qa-held.example/a", GAP, false)
            .await
            .expect("a plugin's request takes its turn");
        let next = tokio::spawn(async move {
            let turn =
                super::qa_gate(Some("qa-held"), "https://cdn.qa-held.example/b", GAP, true)
                    .await;
            let at = ms(start, Instant::now());
            drop(turn);
            at
        });
        tokio::time::sleep(Duration::from_secs(9)).await;
        drop(held);
        assert_eq!(next.await.unwrap(), 13_000, "the source's CDN host shares its turn");
    }

    #[tokio::test(start_paused = true)]
    async fn a_host_in_a_plugins_allowed_hosts_waits_for_that_plugin_from_anywhere() {
        super::qa_register("qa-owner", &["cdn.qa-owner.example".into()]);
        let start = Instant::now();
        call("qa-owner").await;
        // Artwork from the source's CDN, asked by Kaeri itself, with no plugin.
        let art =
            super::qa_gate(None, "https://img.cdn.qa-owner.example/c.jpg", GAP, true).await;
        assert!(art.is_some(), "a source's host is gated");
        assert_eq!(ms(start, Instant::now()), 4000);
        drop(art);
        // Another plugin on the same backend waits for both turns.
        let other =
            super::qa_gate(Some("qa-sibling"), "https://cdn.qa-owner.example/d", GAP, false)
                .await;
        assert_eq!(ms(start, Instant::now()), 8000);
        drop(other);
        assert!(
            super::qa_gate(None, "https://elsewhere.example/e", GAP, true).await.is_none(),
            "Kaeri's requests to hosts no source declared are not gated"
        );
        assert_eq!(ms(start, Instant::now()), 8000);
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
    async fn a_request_cancelled_while_out_still_counts_as_sent() {
        let start = Instant::now();
        let out = tokio::spawn(async move {
            let _turn = super::qa_gate(Some("qa-out"), &url("qa-out"), GAP, false).await;
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        tokio::time::sleep(Duration::from_secs(2)).await;
        out.abort();
        assert!(out.await.unwrap_err().is_cancelled());
        call("qa-out").await;
        assert_eq!(ms(start, Instant::now()), 6000, "4 s after the cancelled one ended");
    }

    #[tokio::test(start_paused = true)]
    async fn a_loop_of_plugin_calls_leaves_a_page_image_every_other_turn() {
        // A series page asked again and again, back to back, while a download's page waits.
        let start = Instant::now();
        let calls = tokio::spawn(async {
            loop {
                call("qa-fair").await;
            }
        });
        tokio::time::sleep(Duration::from_millis(500)).await;
        let first = tokio::time::timeout(PAGE_WAIT, page("qa-fair")).await;
        assert!(first.is_ok(), "the page went out within its wait");
        // Calls at 0 and 4 s were queued first; the call at 8 s jumps the page; the page next.
        assert_eq!(ms(start, Instant::now()), 12_000);
        tokio::time::timeout(PAGE_WAIT, page("qa-fair")).await.expect("the next page went out");
        // Asked at 12 s: behind the call queued at 8 s (16 s) and one that jumps it (20 s).
        assert_eq!(ms(start, Instant::now()), 24_000, "never more than one jump in a row");
        calls.abort();
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
