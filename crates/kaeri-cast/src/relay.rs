//! The relay a TV fetches from while a cast lasts: the stream proxy's and the torrent server's
//! loopback addresses, a file on this device, and what the remuxer and the subtitle converter
//! make, on the interface that faces the TV.
//!
//! What it guards with:
//! - It binds the one local address that routes to the TV, never `0.0.0.0`, on a fresh port.
//! - Every path carries a 128-bit token made for this cast alone; any other path is a `404`.
//! - It accepts connections from the TV's address alone and drops the rest unread.
//! - It fetches upstream only from the origins it was handed, and those a playlist it relayed
//!   names, so it is no open proxy.
//! - It closes when the [`Relay`] is dropped, which the cast does when it ends, and every
//!   connection open to it closes with it.
//!
//! What a receiver needs that mpv did not: the receiver is a web page, so HLS and WebVTT are
//! fetched by script and need CORS, `Access-Control-Allow-Origin` on every answer and an answer
//! to `OPTIONS`, and Chromium's Private Network Access may ask for
//! `Access-Control-Allow-Private-Network`.
//!
//! Bodies are streamed as they arrive. Playlists alone are read whole, to point their addresses
//! back here. A body's first bytes are read before its head is sent: a segment its host names as
//! an image or text goes to the TV under the media type its bytes say, since the receiver's
//! player believes the type and refuses the stream, and bytes that are no media a receiver
//! plays are noted (see [`SourceHealth::unreadable`]).

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use futures::StreamExt;
use kaeri_playback::hls::{PLAYLIST_TYPE, playlist_start};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, tcp::OwnedWriteHalf},
    task::{JoinHandle, JoinSet},
};

use crate::{
    fetch::Upstream,
    probe::{media_type, names_media},
};

/// A resource made on request: its content type and body, or `None` for a `404`.
pub type Made<'a> = Pin<Box<dyn Future<Output = Option<(String, Vec<u8>)>> + Send + 'a>>;

/// What the relay makes on request, under `/x/`: the remuxer's playlist and segments, a WebVTT
/// track.
pub trait Dynamic: Send + Sync {
    fn get<'a>(&'a self, path: &'a str) -> Made<'a>;
}

/// The largest playlist relayed.
const PLAYLIST_LIMIT: usize = 8 << 20;

/// How much of a body is read before its head is sent, to say what it is.
const SNIFFED: usize = 1024;

/// The fewest first bytes that can tell media from something else: one MPEG-TS packet.
const MIN_SNIFF: usize = 188;

struct Shared {
    token: String,
    base: String,
    allow: IpAddr,
    upstream: Arc<dyn Upstream>,
    /// The headers each upstream request carries: the stream's, which change with each file.
    headers: Mutex<Vec<(String, String)>>,
    /// The `scheme://authority` origins the relay may fetch from.
    origins: Mutex<HashSet<String>>,
    /// Single files shared under `/s/<id>/`.
    files: Mutex<HashMap<String, PathBuf>>,
    dynamic: Mutex<Option<Arc<dyn Dynamic>>>,
    /// Small bodies kept in memory under `/b/<id>/`: the capability clips.
    bytes: Mutex<HashMap<String, (&'static str, &'static [u8])>>,
    /// Requests that arrived, counted as they arrive: a TV that reads a header and refuses
    /// closes the connection before the body is done.
    requests: AtomicU64,
    /// Requests for the picture and sound (an init or media segment, a progressive range), as
    /// they arrive, playlists and subtitles aside.
    media_requests: AtomicU64,
    last_request: Mutex<Instant>,
    /// Requests waiting on the source now: the upstream's answer or its next bytes, or what the
    /// remuxer makes. A TV that waits on these has said nothing about what it plays.
    source_waits: AtomicU64,
    /// A wait on the source failed, or took [`Shared::slow_source`] or longer, since the reset.
    source_trouble: AtomicBool,
    /// When the last wait on the source ended, or the reset, whichever came later.
    source_settled: Mutex<Instant>,
    /// Since the reset, a body served as media started with bytes no receiver plays.
    unreadable: AtomicBool,
    slow_source: Mutex<Duration>,
}

/// A wait on the source for one of the TV's requests, counted while it lasts.
struct SourceWait<'a> {
    shared: &'a Shared,
    since: Instant,
}

impl<'a> SourceWait<'a> {
    fn begin(shared: &'a Shared) -> Self {
        shared.source_waits.fetch_add(1, Ordering::Relaxed);
        Self { shared, since: Instant::now() }
    }

    /// The source failed this request.
    fn failed(&self) {
        self.shared.source_trouble.store(true, Ordering::Relaxed);
    }
}

impl Drop for SourceWait<'_> {
    fn drop(&mut self) {
        if self.since.elapsed() >= *lock(&self.shared.slow_source) {
            self.shared.source_trouble.store(true, Ordering::Relaxed);
        }
        *lock(&self.shared.source_settled) = Instant::now();
        self.shared.source_waits.fetch_sub(1, Ordering::Relaxed);
    }
}

/// How the source behind the TV's requests has done since the last [`Relay::reset_stats`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceHealth {
    /// A request is waiting on the source now.
    pub busy: bool,
    /// A wait on the source failed or was slow.
    pub troubled: bool,
    /// Since the last wait on the source ended, or the reset.
    pub quiet: Duration,
    /// The TV was served media whose first bytes are no format a receiver plays, such as a
    /// segment with other bytes before its MPEG-TS. Its refusal is the stream's, not the
    /// format's.
    pub unreadable: bool,
}

/// A wait on the source this long is a slow source, unless the cast says otherwise.
const SLOW_SOURCE: Duration = Duration::from_secs(4);

/// A running relay. Dropping it closes it.
pub struct Relay {
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.close();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Relay {
    /// Listen on `bind` for `allow` alone, fetching upstream through `upstream`.
    pub async fn start(
        bind: Ipv4Addr,
        allow: IpAddr,
        upstream: Arc<dyn Upstream>,
    ) -> std::io::Result<Self> {
        if bind.is_unspecified() {
            return Err(std::io::Error::other("the relay never listens on every interface"));
        }
        let listener = TcpListener::bind(SocketAddr::from((bind, 0))).await?;
        let port = listener.local_addr()?.port();
        let token: String = (0..16).map(|_| format!("{:02x}", rand::random::<u8>())).collect();
        let shared = Arc::new(Shared {
            base: format!("http://{bind}:{port}/{token}"),
            token,
            allow,
            upstream,
            headers: Mutex::default(),
            origins: Mutex::default(),
            files: Mutex::default(),
            dynamic: Mutex::default(),
            bytes: Mutex::default(),
            requests: AtomicU64::new(0),
            media_requests: AtomicU64::new(0),
            last_request: Mutex::new(Instant::now()),
            source_waits: AtomicU64::new(0),
            source_trouble: AtomicBool::new(false),
            source_settled: Mutex::new(Instant::now()),
            unreadable: AtomicBool::new(false),
            slow_source: Mutex::new(SLOW_SOURCE),
        });
        let task = tokio::spawn(serve(listener, Arc::clone(&shared)));
        Ok(Self { shared, task })
    }

    /// Stop serving: the listener and every open connection close, and what was shared is
    /// forgotten. The cast calls it the moment it ends; dropping the relay does it too.
    pub fn close(&self) {
        self.task.abort();
        // A request already being answered holds `Shared` until its task is dropped; what it
        // could reach goes now.
        lock(&self.shared.dynamic).take();
        lock(&self.shared.files).clear();
        lock(&self.shared.bytes).clear();
        lock(&self.shared.origins).clear();
    }

    pub fn base(&self) -> &str {
        &self.shared.base
    }

    /// Share one file, with byte ranges, and return its URL.
    pub fn share_file(&self, path: &std::path::Path) -> String {
        let id = format!("{:08x}", rand::random::<u32>());
        let name =
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        lock(&self.shared.files).insert(id.clone(), path.to_owned());
        format!("{}/s/{id}/{}", self.shared.base, encode(&name))
    }

    /// Share `body`, kept in memory, as `name` of `content_type`, and return its URL.
    pub fn share_bytes(
        &self,
        name: &str,
        content_type: &'static str,
        body: &'static [u8],
    ) -> String {
        let id = format!("{:08x}", rand::random::<u32>());
        lock(&self.shared.bytes).insert(id.clone(), (content_type, body));
        format!("{}/b/{id}/{}", self.shared.base, encode(name))
    }

    /// The URL that fetches `upstream` with the stream's headers; its origin is allowed from now.
    pub fn upstream_url(&self, upstream: &str) -> String {
        if let Some(origin) = origin(upstream) {
            lock(&self.shared.origins).insert(origin);
        }
        format!("{}/u/{}", self.shared.base, encode(upstream))
    }

    /// `text`, a playlist fetched from `base`, with its addresses pointed back through the
    /// relay; their origins are allowed from now.
    pub fn relay_playlist(&self, text: &str, base: &str) -> String {
        rewrite_playlist(text, base, &|u| self.upstream_url(u))
    }

    /// The headers every upstream request carries from now on.
    pub fn set_headers(&self, headers: Vec<(String, String)>) {
        *lock(&self.shared.headers) = headers;
    }

    pub fn set_dynamic(&self, dynamic: Option<Arc<dyn Dynamic>>) {
        *lock(&self.shared.dynamic) = dynamic;
    }

    pub fn dynamic_url(&self, path: &str) -> String {
        format!("{}/x/{}", self.shared.base, path.trim_start_matches('/'))
    }

    /// Requests that arrived since the last [`Relay::reset_stats`].
    pub fn requests(&self) -> u64 {
        self.shared.requests.load(Ordering::Relaxed)
    }

    /// Whether the TV asked for the picture or sound since the last reset: an init segment, a
    /// media segment or a progressive range, counted when the request arrives.
    pub fn media_served(&self) -> bool {
        self.shared.media_requests.load(Ordering::Relaxed) > 0
    }

    pub fn reset_stats(&self) {
        self.shared.requests.store(0, Ordering::Relaxed);
        self.shared.media_requests.store(0, Ordering::Relaxed);
        self.shared.source_trouble.store(false, Ordering::Relaxed);
        self.shared.unreadable.store(false, Ordering::Relaxed);
        *lock(&self.shared.source_settled) = Instant::now();
    }

    /// How the source has done for the TV since the last reset: whether the TV is waiting on
    /// it, and whether it was slow or failed. What a TV does while it waits on a slow source
    /// says nothing about what it plays.
    pub fn source(&self) -> SourceHealth {
        let shared = &self.shared;
        SourceHealth {
            busy: shared.source_waits.load(Ordering::Relaxed) > 0,
            troubled: shared.source_trouble.load(Ordering::Relaxed),
            quiet: lock(&shared.source_settled).elapsed(),
            unreadable: shared.unreadable.load(Ordering::Relaxed),
        }
    }

    /// A wait on the source this long or longer counts as a slow source.
    pub fn set_slow_source(&self, slow: Duration) {
        *lock(&self.shared.slow_source) = slow;
    }

    /// How long since the TV last asked for anything.
    pub fn idle_for(&self) -> Duration {
        lock(&self.shared.last_request).elapsed()
    }
}

/// How long the relay waits after an accept failed before it tries again: at first, and at most
/// while the failures go on.
const ACCEPT_RETRY: Duration = Duration::from_millis(50);
const ACCEPT_RETRY_MAX: Duration = Duration::from_secs(1);

/// Take the TV's connections on `listener` and serve each one, until the relay closes.
async fn serve(listener: TcpListener, shared: Arc<Shared>) {
    // Every connection lives in this set, so aborting the accept loop drops them all: a TV's
    // kept-alive connection gets nothing once the cast has ended.
    let mut connections = JoinSet::new();
    // A failed accept leaves the listener as it was. The process may be out of file descriptors
    // for a moment (a torrent's peers take many), or a connection may have gone before it was
    // taken. Ending the loop there would close the listener and every connection open on it,
    // and the TV could fetch nothing more from this relay while the cast lasts. So the relay
    // waits and takes connections again, and only closing it ends the loop.
    let mut failures = 0_u32;
    loop {
        let (socket, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(e) => {
                failures = failures.saturating_add(1);
                // Said as the failures start, then less and less often while they go on.
                if failures.is_power_of_two() {
                    tracing::warn!(error = %e, failures, "the relay couldn't take a connection");
                }
                // The listener stays ready after a failure, so trying at once would spin.
                tokio::time::sleep(accept_retry(failures)).await;
                continue;
            }
        };
        if failures > 0 {
            tracing::info!(failures, "the relay takes connections again");
            failures = 0;
        }
        while connections.try_join_next().is_some() {}
        if peer.ip() != shared.allow {
            tracing::debug!(peer = %peer.ip(), "relay refused a connection");
            continue;
        }
        let shared = Arc::clone(&shared);
        connections.spawn(async move {
            if let Err(e) = connection(socket, &shared).await {
                tracing::debug!(error = %e, "relay connection ended");
            }
        });
    }
}

/// The wait after `failures` accepts in a row failed: [`ACCEPT_RETRY`], doubling with each
/// failure up to [`ACCEPT_RETRY_MAX`].
fn accept_retry(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(5);
    ACCEPT_RETRY.saturating_mul(1 << doublings).min(ACCEPT_RETRY_MAX)
}

/// `scheme://authority` of a URL.
fn origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    Some(format!("{}://{}", scheme.to_ascii_lowercase(), authority.to_ascii_lowercase()))
}

struct Request {
    method: String,
    target: String,
    range: Option<String>,
    private_network: bool,
    close: bool,
}

async fn read_request(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> std::io::Result<Option<Request>> {
    let mut line = String::new();
    if reader.read_line(&mut line).await? == 0 {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let mut request = Request {
        method: parts.next().unwrap_or_default().to_owned(),
        target: parts.next().unwrap_or_default().to_owned(),
        range: None,
        private_network: false,
        close: false,
    };
    let mut total = line.len();
    loop {
        let mut header = String::new();
        let n = reader.read_line(&mut header).await?;
        total += n;
        if total > 32 * 1024 {
            return Err(std::io::Error::other("headers too long"));
        }
        let header = header.trim_end();
        if n == 0 || header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else { continue };
        let value = value.trim().to_owned();
        match name.trim().to_ascii_lowercase().as_str() {
            "range" => request.range = Some(value),
            "access-control-request-private-network" => request.private_network = true,
            "connection" => request.close = value.eq_ignore_ascii_case("close"),
            _ => {}
        }
    }
    Ok(Some(request))
}

async fn connection(socket: TcpStream, shared: &Shared) -> std::io::Result<()> {
    socket.set_nodelay(true)?;
    let (read, mut write) = socket.into_split();
    let mut reader = BufReader::new(read);
    while let Some(request) =
        tokio::time::timeout(Duration::from_secs(60), read_request(&mut reader))
            .await
            .unwrap_or(Ok(None))?
    {
        *lock(&shared.last_request) = Instant::now();
        shared.requests.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(
            method = %request.method,
            path = request.target.split('?').next().unwrap_or_default(),
            range = request.range.as_deref().unwrap_or("all"),
            "relay asked"
        );
        let (status, keep) = answer(&request, shared, &mut write).await?;
        tracing::debug!(method = %request.method, status, "relayed");
        if request.close || !keep {
            break;
        }
    }
    Ok(())
}

fn cors(request: &Request) -> String {
    let mut out = String::from(
        "Access-Control-Allow-Origin: *\r\n\
         Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n",
    );
    if request.method == "OPTIONS" {
        out.push_str(
            "Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
             Access-Control-Allow-Headers: *\r\n\
             Access-Control-Max-Age: 600\r\n",
        );
        if request.private_network {
            out.push_str("Access-Control-Allow-Private-Network: true\r\n");
        }
    }
    out
}

async fn head(
    write: &mut OwnedWriteHalf,
    status: u16,
    extra: &str,
    length: Option<u64>,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        502 => "Bad Gateway",
        _ => "Relayed",
    };
    let length = match length {
        Some(length) => format!("Content-Length: {length}\r\n"),
        None => "Connection: close\r\n".to_owned(),
    };
    // The same paths carry each load's playlist and segments, so nothing may be kept.
    let head =
        format!("HTTP/1.1 {status} {reason}\r\nCache-Control: no-store\r\n{extra}{length}\r\n");
    write.write_all(head.as_bytes()).await
}

/// Answer one request: its status, and whether the connection may carry another.
async fn answer(
    request: &Request,
    shared: &Shared,
    write: &mut OwnedWriteHalf,
) -> std::io::Result<(u16, bool)> {
    let cors = cors(request);
    let Some(rest) = request
        .target
        .split('?')
        .next()
        .and_then(|t| t.strip_prefix('/'))
        .and_then(|t| t.strip_prefix(shared.token.as_str()))
        .and_then(|t| t.strip_prefix('/'))
    else {
        head(write, 404, "", Some(0)).await?;
        return Ok((404, true));
    };
    if request.method == "OPTIONS" {
        head(write, 204, &cors, Some(0)).await?;
        return Ok((204, true));
    }
    let head_only = request.method == "HEAD";
    if request.method != "GET" && !head_only {
        head(write, 405, &cors, Some(0)).await?;
        return Ok((405, true));
    }
    if let Some(kept) = rest.strip_prefix("b/") {
        let id = kept.split('/').next().unwrap_or_default();
        let found = lock(&shared.bytes).get(id).copied();
        if let Some((content_type, body)) = found {
            if !head_only {
                shared.media_requests.fetch_add(1, Ordering::Relaxed);
            }
            let extra = format!("{cors}Content-Type: {content_type}\r\n");
            head(write, 200, &extra, Some(body.len() as u64)).await?;
            if !head_only {
                write.write_all(body).await?;
            }
            return Ok((200, true));
        }
    } else if let Some(shared_file) = rest.strip_prefix("s/") {
        let id = shared_file.split('/').next().unwrap_or_default();
        let path = lock(&shared.files).get(id).cloned();
        if let Some(path) = path {
            return file(shared, &path, request, &cors, head_only, write).await;
        }
    } else if let Some(encoded) = rest.strip_prefix("u/") {
        return upstream(shared, &decode(encoded), request, &cors, head_only, write).await;
    } else if let Some(path) = rest.strip_prefix("x/") {
        let dynamic = lock(&shared.dynamic).clone();
        let made = match dynamic {
            Some(dynamic) => {
                let wait = SourceWait::begin(shared);
                let made = dynamic.get(path).await;
                if made.is_none() {
                    wait.failed();
                }
                made
            }
            None => None,
        };
        if let Some((content_type, body)) = made {
            if is_media(&content_type) && !head_only {
                shared.media_requests.fetch_add(1, Ordering::Relaxed);
            }
            let extra = format!("{cors}Content-Type: {content_type}\r\n");
            head(write, 200, &extra, Some(body.len() as u64)).await?;
            if !head_only {
                write.write_all(&body).await?;
            }
            return Ok((200, true));
        }
    }
    head(write, 404, &cors, Some(0)).await?;
    Ok((404, true))
}

/// Whether bytes of this type are the picture and sound, not a playlist or subtitles.
fn is_media(content_type: &str) -> bool {
    let kind = content_type.to_ascii_lowercase();
    !(kind.contains("mpegurl") || kind.contains("vtt") || kind.starts_with("text/"))
}

pub fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str() {
        "m3u8" => "application/vnd.apple.mpegurl",
        "ts" => "video/mp2t",
        "m4s" | "mp4" | "m4v" => "video/mp4",
        "m4a" => "audio/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "vtt" => "text/vtt",
        _ => "application/octet-stream",
    }
}

/// A single `bytes=a-b` range against a resource of `size` bytes.
fn parse_range(range: &str, size: u64) -> Option<(u64, u64)> {
    let spec = range.trim().strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let suffix: u64 = suffix.parse().ok()?;
            (size.saturating_sub(suffix), size.checked_sub(1)?)
        }
        (start, "") => (start.parse().ok()?, size.checked_sub(1)?),
        (start, end) => {
            (start.parse().ok()?, end.parse::<u64>().ok()?.min(size.checked_sub(1)?))
        }
    };
    (start <= end && start < size).then_some((start, end))
}

async fn file(
    shared: &Shared,
    path: &std::path::Path,
    request: &Request,
    cors: &str,
    head_only: bool,
    write: &mut OwnedWriteHalf,
) -> std::io::Result<(u16, bool)> {
    let Ok(mut file) = File::open(path).await else {
        head(write, 404, cors, Some(0)).await?;
        return Ok((404, true));
    };
    let size = file.metadata().await?.len();
    let kind = content_type(&path.to_string_lossy());
    let mut extra = format!("{cors}Content-Type: {kind}\r\nAccept-Ranges: bytes\r\n");
    let (status, start, len) = match request.range.as_deref().map(|r| parse_range(r, size)) {
        Some(Some((start, end))) => {
            extra.push_str(&format!("Content-Range: bytes {start}-{end}/{size}\r\n"));
            (206, start, end - start + 1)
        }
        Some(None) => {
            let extra = format!("{cors}Content-Range: bytes */{size}\r\n");
            head(write, 416, &extra, Some(0)).await?;
            return Ok((416, true));
        }
        None => (200, 0, size),
    };
    if is_media(kind) && !head_only {
        shared.media_requests.fetch_add(1, Ordering::Relaxed);
    }
    head(write, status, &extra, Some(len)).await?;
    if head_only {
        return Ok((status, true));
    }
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut body = file.take(len);
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        let n = body.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        write.write_all(&buffer[..n]).await?;
    }
    Ok((status, true))
}

async fn upstream(
    shared: &Shared,
    url: &str,
    request: &Request,
    cors: &str,
    head_only: bool,
    write: &mut OwnedWriteHalf,
) -> std::io::Result<(u16, bool)> {
    let allowed = origin(url).is_some_and(|o| lock(&shared.origins).contains(&o));
    if !allowed {
        head(write, 404, cors, Some(0)).await?;
        return Ok((404, true));
    }
    let mut headers = lock(&shared.headers).clone();
    if let Some(range) = &request.range {
        headers.push(("Range".into(), range.clone()));
    }
    let wait = SourceWait::begin(shared);
    let mut response = match shared.upstream.get(url, &headers).await {
        Ok(response) => response,
        Err(e) => {
            tracing::debug!(error = %e, "relay upstream failed");
            wait.failed();
            drop(wait);
            head(write, 502, cors, Some(0)).await?;
            return Ok((502, true));
        }
    };
    let status = response.status;
    if status >= 400 {
        wait.failed();
    }
    drop(wait);
    // Where the answer came from after any redirect: a playlist's relative addresses are
    // relative to that.
    let base = response.url.clone().unwrap_or_else(|| url.to_owned());
    let mut kind = response.header("content-type").map_or_else(
        || content_type(url.split('?').next().unwrap_or(url)).to_owned(),
        str::to_owned,
    );
    let content_range = response.header("content-range").map(str::to_owned);
    let length: Option<u64> =
        response.header("content-length").and_then(|l| l.trim().parse().ok());
    let ranges = response.header("accept-ranges").is_some() || status == 206;
    let from_start = status == 200
        || content_range
            .as_deref()
            .is_some_and(|r| r.trim_start_matches("bytes ").trim().starts_with("0-"));

    let mut playlist = kind.contains("mpegurl")
        || url.split('?').next().is_some_and(|p| p.to_ascii_lowercase().ends_with(".m3u8"));
    // The first bytes say what the body is: a tokenised playlist address may come as plain
    // bytes or text, and a segment named as an image. A HEAD is answered from them too, since
    // the receiver's player may choose a segment's type from a HEAD alone.
    let mut first = Vec::new();
    if !playlist
        && matches!(status, 200 | 206)
        && from_start
        && (!head_only || !names_media(&kind))
    {
        let wait = SourceWait::begin(shared);
        while first.len() < SNIFFED {
            match response.body.next().await {
                Some(Ok(chunk)) => first.extend_from_slice(&chunk),
                Some(Err(e)) => {
                    wait.failed();
                    return Err(e);
                }
                None => {
                    // Spent: a stream may not be asked again once it has ended.
                    response.body = Box::pin(futures::stream::empty());
                    break;
                }
            }
        }
        drop(wait);
        let text = String::from_utf8_lossy(&first[..first.len().min(64)]).into_owned();
        let text = text.trim_start_matches('\u{feff}').trim_start();
        // A playlist may carry leading bytes before its `#EXTM3U`.
        if !names_media(&kind) && playlist_start(&first).is_some() {
            playlist = true;
        } else if text.starts_with("WEBVTT") {
            kind = "text/vtt".into();
        } else {
            match media_type(&first) {
                Some(found) if !names_media(&kind) => {
                    tracing::debug!(named = %kind, found, "relaying a segment as what it is");
                    kind = found.to_owned();
                }
                Some(_) => {}
                None if is_media(&kind) && first.len() >= MIN_SNIFF => {
                    tracing::debug!(named = %kind, "relaying bytes no receiver plays");
                    shared.unreadable.store(true, Ordering::Relaxed);
                }
                None => {}
            }
        }
    }
    let mut extra = format!("{cors}Content-Type: {kind}\r\n");
    if let Some(content_range) = content_range {
        extra.push_str(&format!(
            "Content-Range: bytes {}\r\n",
            content_range.trim_start_matches("bytes ")
        ));
    }
    if ranges {
        extra.push_str("Accept-Ranges: bytes\r\n");
    }
    let sniffed = playlist && !first.is_empty();
    if playlist && matches!(status, 200 | 206) {
        // A playlist is rewritten whole, so it is fetched whole, whatever range was asked.
        let whole = status == 200
            || response.header("content-range").is_some_and(|r| {
                let r = r.trim_start_matches("bytes ").trim();
                let (span, total) = r.split_once('/').unwrap_or((r, ""));
                let (from, to) = span.split_once('-').unwrap_or(("", ""));
                from == "0" && to.parse::<u64>().ok().map(|t| t + 1) == total.parse().ok()
            });
        let wait = SourceWait::begin(shared);
        let body = if whole {
            response.bytes(PLAYLIST_LIMIT.saturating_sub(first.len())).await.map(|rest| {
                let mut body = std::mem::take(&mut first);
                body.extend(rest);
                body
            })
        } else {
            let headers = lock(&shared.headers).clone();
            match shared.upstream.get(url, &headers).await {
                Ok(again) if again.status == 200 => again.bytes(PLAYLIST_LIMIT).await,
                _ => Err(std::io::Error::other("the playlist couldn't be read whole")),
            }
        };
        let body = match body {
            Ok(body) => body,
            Err(e) => {
                wait.failed();
                drop(wait);
                tracing::debug!(error = %e, "relay upstream failed");
                head(write, 502, cors, Some(0)).await?;
                return Ok((502, true));
            }
        };
        drop(wait);
        let text = String::from_utf8_lossy(&body[playlist_start(&body).unwrap_or(0)..]);
        let origins = std::cell::RefCell::new(Vec::new());
        let rewritten = rewrite_playlist(&text, &base, &|u| {
            if let Some(o) = origin(u) {
                origins.borrow_mut().push(o);
            }
            format!("{}/u/{}", shared.base, encode(u))
        });
        lock(&shared.origins).extend(origins.into_inner());
        let kind = if sniffed { PLAYLIST_TYPE } else { kind.as_str() };
        let extra = format!("{cors}Content-Type: {kind}\r\n");
        head(write, 200, &extra, Some(rewritten.len() as u64)).await?;
        if !head_only {
            write.write_all(rewritten.as_bytes()).await?;
        }
        return Ok((200, true));
    }

    if (200..300).contains(&status) && is_media(&kind) && !head_only {
        shared.media_requests.fetch_add(1, Ordering::Relaxed);
    }
    head(write, status, &extra, length).await?;
    if head_only {
        return Ok((status, length.is_some()));
    }
    let mut body = response.body;
    let mut sent = 0u64;
    if !first.is_empty() {
        write.write_all(&first).await?;
        sent += first.len() as u64;
    }
    loop {
        let chunk = {
            let wait = SourceWait::begin(shared);
            let chunk = body.next().await;
            if matches!(chunk, Some(Err(_))) {
                wait.failed();
            }
            chunk
        };
        let Some(chunk) = chunk else { break };
        let chunk = chunk?;
        write.write_all(&chunk).await?;
        sent += chunk.len() as u64;
    }
    // A body shorter than it said can't leave the connection in step.
    Ok((status, length.is_some_and(|l| l == sent)))
}

/// Point every URI in an HLS playlist back through the relay, and declare HE-AAC audio as
/// AAC-LC (see [`plain_aac`]).
pub fn rewrite_playlist(text: &str, base: &str, proxy: &dyn Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#EXT-X-STREAM-INF:") {
            out.push_str(&plain_aac(trimmed));
            out.push('\n');
        } else if trimmed.is_empty() {
            out.push('\n');
        } else if !trimmed.starts_with('#') {
            out.push_str(&proxy(&resolve(base, trimmed)));
            out.push('\n');
        } else if let Some(start) = trimmed.find("URI=\"") {
            let value_at = start + 5;
            let end = trimmed[value_at..].find('"').map_or(trimmed.len(), |e| value_at + e);
            out.push_str(&trimmed[..value_at]);
            out.push_str(&proxy(&resolve(base, &trimmed[value_at..end])));
            out.push_str(&trimmed[end..]);
            out.push('\n');
        } else {
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    out
}

/// A variant's `CODECS` with HE-AAC (`mp4a.40.5`) and HE-AAC v2 (`mp4a.40.29`) named as AAC-LC
/// (`mp4a.40.2`). A Chromecast refuses some MPEG-TS streams whose master declares HE-AAC, with
/// `MEDIA_UNKNOWN`, and plays the same segments declared as AAC-LC: the decoder finds the SBR
/// and parametric stereo in the stream by itself, as browsers' do.
fn plain_aac(line: &str) -> String {
    let Some(start) = line.find("CODECS=\"").map(|i| i + 8) else { return line.to_owned() };
    let Some(end) = line[start..].find('"').map(|e| start + e) else { return line.to_owned() };
    let codecs: Vec<&str> = line[start..end]
        .split(',')
        .map(|codec| match codec.trim() {
            "mp4a.40.5" | "mp4a.40.29" => "mp4a.40.2",
            other => other,
        })
        .collect();
    format!("{}{}{}", &line[..start], codecs.join(","), &line[end..])
}

/// Resolve `reference` against `base`, enough for playlist URIs.
pub fn resolve(base: &str, reference: &str) -> String {
    if reference.contains("://") {
        return reference.to_owned();
    }
    let scheme_end = base.find("://").map_or(0, |i| i + 3);
    let authority_end = base[scheme_end..].find('/').map_or(base.len(), |i| scheme_end + i);
    if let Some(rooted) = reference.strip_prefix('/') {
        return format!("{}/{rooted}", &base[..authority_end]);
    }
    let path = base.split('?').next().unwrap_or(base);
    let dir_end = path.rfind('/').filter(|&i| i >= authority_end).map_or(path.len(), |i| i + 1);
    let mut joined = format!("{}{reference}", &path[..dir_end]);
    if dir_end == path.len() && !joined[..dir_end].ends_with('/') {
        joined.insert(dir_end, '/');
    }
    joined
}

fn encode(url: &str) -> String {
    let mut out = String::with_capacity(url.len() + 16);
    for byte in url.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn decode(encoded: &str) -> String {
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) =
                encoded.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::{PlainHttp, UpstreamResponse};

    #[test]
    fn ranges_read_as_http_says() {
        assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=90-200", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
    }

    #[test]
    fn playlist_uris_come_back_through_the_relay() {
        let text =
            "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:4,\nseg0.m4s\n/abs/seg1.m4s\n";
        let out = rewrite_playlist(text, "http://cdn.example/a/b.m3u8", &|u| format!("P[{u}]"));
        assert!(out.contains("#EXT-X-MAP:URI=\"P[http://cdn.example/a/init.mp4]\""), "{out}");
        assert!(out.contains("\nP[http://cdn.example/a/seg0.m4s]\n"));
        assert!(out.contains("\nP[http://cdn.example/abs/seg1.m4s]\n"));
    }

    #[test]
    fn a_master_declares_he_aac_as_aac_lc() {
        let text = "#EXTM3U\n\
            #EXT-X-STREAM-INF:BANDWIDTH=800000,CODECS=\"avc1.640033,mp4a.40.29\",RESOLUTION=1920x1080\n\
            hi.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=400000,CODECS=\"avc1.64001f, mp4a.40.5\"\n\
            mid.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=200000,CODECS=\"avc1.42e01e,mp4a.40.2\"\n\
            lo.m3u8\n\
            #EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=9000,CODECS=\"avc1.640033\",URI=\"i.m3u8\"\n";
        let out =
            rewrite_playlist(text, "http://cdn.example/a/master.m3u8", &|u| format!("P[{u}]"));
        assert!(
            out.contains("CODECS=\"avc1.640033,mp4a.40.2\",RESOLUTION=1920x1080\n"),
            "{out}"
        );
        assert!(out.contains("CODECS=\"avc1.64001f,mp4a.40.2\"\n"), "{out}");
        assert!(out.contains("CODECS=\"avc1.42e01e,mp4a.40.2\"\n"), "{out}");
        assert!(!out.contains("mp4a.40.29") && !out.contains("mp4a.40.5"), "{out}");
        assert!(out.contains("\nP[http://cdn.example/a/hi.m3u8]\n"), "{out}");
        assert!(out.contains("URI=\"P[http://cdn.example/a/i.m3u8]\""), "{out}");
        // A variant line without CODECS is left as it is.
        assert_eq!(plain_aac("#EXT-X-STREAM-INF:BANDWIDTH=1"), "#EXT-X-STREAM-INF:BANDWIDTH=1");
    }

    /// GET `url` as the receiver does, with its origin.
    async fn get(url: &str, range: Option<&str>) -> UpstreamResponse {
        let mut headers = vec![("Origin".to_owned(), crate::probe::RECEIVER_ORIGIN.to_owned())];
        if let Some(range) = range {
            headers.push(("Range".into(), range.into()));
        }
        PlainHttp.get(url, &headers).await.unwrap()
    }

    #[tokio::test]
    async fn the_relay_serves_its_tv_with_cors_and_only_what_it_was_handed() {
        let body: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let source = crate::fetch::tests::serve(body.clone(), true).await;
        let relay = Relay::start(
            Ipv4Addr::LOCALHOST,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            Arc::new(PlainHttp),
        )
        .await
        .unwrap();

        // Streamed through, chunked upstream and all, with CORS.
        let url = relay.upstream_url(&source);
        let response = get(&url, None).await;
        assert_eq!(response.status, 200);
        assert_eq!(response.header("access-control-allow-origin"), Some("*"));
        assert_eq!(response.header("cache-control"), Some("no-store"));
        assert_eq!(response.bytes(1 << 20).await.unwrap(), body);
        assert!(relay.media_served());

        let ranged = get(&url, Some("bytes=100-199")).await;
        assert_eq!(ranged.status, 206);
        assert_eq!(ranged.bytes(1 << 20).await.unwrap(), body[100..200]);

        // Somewhere it was never handed is not fetched.
        let elsewhere = format!("{}/u/{}", relay.base(), encode("http://elsewhere.example/x"));
        assert_eq!(get(&elsewhere, None).await.status, 404);
        // A guessed token is a 404, whatever the path.
        let wrong = url.replace(&relay.shared.token, &"0".repeat(32));
        assert_eq!(get(&wrong, None).await.status, 404);

        // A file here, by ranges, and the preflight a receiver may send.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("episode.mp4");
        std::fs::write(&path, &body).unwrap();
        let shared = relay.share_file(&path);
        let part = get(&shared, Some("bytes=0-")).await;
        assert_eq!((part.status, part.header("content-type")), (206, Some("video/mp4")));
        assert_eq!(part.bytes(1 << 20).await.unwrap().len(), body.len());

        relay.reset_stats();
        assert_eq!(relay.requests(), 0);
        assert!(!relay.media_served());

        // A TV that asks, reads the head and hangs up has still asked for media.
        let (authority, path) = shared.trim_start_matches("http://").split_once('/').unwrap();
        let mut socket = tokio::net::TcpStream::connect(authority).await.unwrap();
        socket
            .write_all(format!("GET /{path} HTTP/1.1\r\nHost: tv\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut start = [0u8; 12];
        socket.read_exact(&mut start).await.unwrap();
        drop(socket);
        assert!(start.starts_with(b"HTTP/1.1 200"));
        assert_eq!(relay.requests(), 1);
        assert!(relay.media_served());

        // Bytes kept in memory, as the capability clips are.
        relay.reset_stats();
        let kept = relay.share_bytes("clip.webm", "video/webm", b"not a real clip");
        let response = get(&kept, None).await;
        assert_eq!(response.header("content-type"), Some("video/webm"));
        assert_eq!(response.bytes(1024).await.unwrap(), b"not a real clip");
        assert!(relay.media_served());
    }

    #[tokio::test]
    async fn a_playlist_served_as_plain_bytes_is_still_rewritten() {
        // A tokenised playlist address with no `.m3u8`, answered without an HLS type.
        let playlist = "#EXTM3U\n#EXTINF:4.0,\nseg0.ts\n#EXT-X-ENDLIST\n";
        let source = crate::fetch::tests::serve(playlist.as_bytes().to_vec(), false).await;
        let relay = Relay::start(
            Ipv4Addr::LOCALHOST,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            Arc::new(PlainHttp),
        )
        .await
        .unwrap();
        let response =
            get(&relay.upstream_url(&format!("{source}/play?token=abc")), None).await;
        assert_eq!(response.status, 200);
        assert!(response.header("content-type").is_some_and(|t| t.contains("mpegurl")));
        let text = String::from_utf8(response.bytes(1 << 20).await.unwrap()).unwrap();
        assert!(text.contains(&format!("{}/u/", relay.base())), "{text}");
        assert!(!relay.media_served(), "a playlist is not the picture or the sound");

        // Bytes that aren't a playlist go through as they came.
        let body: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
        let source = crate::fetch::tests::serve(body.clone(), true).await;
        let response = get(&relay.upstream_url(&format!("{source}/v?token=abc")), None).await;
        assert_eq!(response.bytes(1 << 20).await.unwrap(), body);
        assert!(relay.media_served());
    }

    #[tokio::test]
    async fn a_playlist_with_leading_bytes_is_rewritten_from_its_tag() {
        // A playlist may carry leading bytes before `#EXTM3U`. The receiver gets it from the
        // tag on, labelled as a playlist, its URIs pointed back at the relay.
        let mut body = b"\x00\x01\x02\x03\x04\x05\x06\n".to_vec();
        body.extend_from_slice(b"#EXTM3U\n#EXTINF:4.0,\nseg0.bin\n#EXT-X-ENDLIST\n");
        let source = crate::fetch::tests::serve_typed(body, false, Some("image/jpeg")).await;
        let relay = Relay::start(
            Ipv4Addr::LOCALHOST,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            Arc::new(PlainHttp),
        )
        .await
        .unwrap();
        let response = get(&relay.upstream_url(&format!("{source}/list.bin")), None).await;
        assert_eq!(response.status, 200);
        assert_eq!(response.header("content-type"), Some(PLAYLIST_TYPE));
        let text = String::from_utf8(response.bytes(1 << 20).await.unwrap()).unwrap();
        assert!(text.starts_with("#EXTM3U\n"), "{text}");
        assert!(text.contains(&format!("{}/u/", relay.base())), "{text}");
    }

    /// HEAD `url`, as the receiver's player does to learn a segment's type.
    async fn head_of(url: &str) -> String {
        let (authority, path) = url.trim_start_matches("http://").split_once('/').unwrap();
        let mut socket = tokio::net::TcpStream::connect(authority).await.unwrap();
        socket
            .write_all(
                format!("HEAD /{path} HTTP/1.1\r\nHost: tv\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        let mut answer = String::new();
        socket.read_to_string(&mut answer).await.unwrap();
        answer.to_ascii_lowercase()
    }

    #[tokio::test]
    async fn a_segment_named_as_an_image_goes_to_the_tv_as_the_media_it_is() {
        let mut segment = vec![0xFFu8; 188 * 40];
        for packet in segment.chunks_mut(188) {
            packet[0] = 0x47;
        }
        let source =
            crate::fetch::tests::serve_typed(segment.clone(), true, Some("image/jpeg")).await;
        let relay = Relay::start(
            Ipv4Addr::LOCALHOST,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            Arc::new(PlainHttp),
        )
        .await
        .unwrap();
        let url = relay.upstream_url(&format!("{source}/seg-1.jpg"));

        let head = head_of(&url).await;
        assert!(head.contains("content-type: video/mp2t"), "{head}");
        assert!(!relay.media_served(), "a HEAD is not the picture");

        let response = get(&url, None).await;
        assert_eq!(response.header("content-type"), Some("video/mp2t"));
        assert_eq!(response.bytes(1 << 20).await.unwrap(), segment, "the bytes as they came");
        assert!(relay.media_served());
        assert!(!relay.source().unreadable);

        // A range from the start is named the same; one from the middle can't be read, and is
        // passed on as it was named.
        let start = get(&url, Some("bytes=0-")).await;
        assert_eq!((start.status, start.header("content-type")), (206, Some("video/mp2t")));
        let middle = get(&url, Some("bytes=1000-1999")).await;
        assert_eq!(middle.header("content-type"), Some("image/jpeg"));

        // Bytes that are no media a receiver plays are noted, so a refusal blames nothing.
        relay.reset_stats();
        let mut junk = b"\x89PNG\r\n\x1a\n".to_vec();
        junk.extend(&segment);
        let source =
            crate::fetch::tests::serve_typed(junk.clone(), false, Some("image/png")).await;
        let response = get(&relay.upstream_url(&format!("{source}/seg-2.png")), None).await;
        assert_eq!(response.header("content-type"), Some("image/png"));
        assert_eq!(response.bytes(1 << 20).await.unwrap(), junk);
        assert!(relay.media_served());
        assert!(relay.source().unreadable);
        relay.reset_stats();
        assert!(!relay.source().unreadable);
    }

    /// An upstream that sends every request on to `edge`, as a CDN's redirect does, and says
    /// where it landed.
    struct Redirecting {
        edge: String,
    }

    #[async_trait::async_trait]
    impl Upstream for Redirecting {
        async fn get(
            &self,
            url: &str,
            headers: &[(String, String)],
        ) -> std::io::Result<UpstreamResponse> {
            let landed = format!("{}/edge/index.m3u8", self.edge);
            let mut response = PlainHttp.get(&self.edge, headers).await?;
            response.url = Some(landed).filter(|landed| landed != url);
            Ok(response)
        }
    }

    #[tokio::test]
    async fn a_playlist_reached_through_a_redirect_resolves_against_where_it_landed() {
        let playlist = "#EXTM3U\n#EXTINF:4.0,\nseg0.ts\n#EXT-X-ENDLIST\n";
        let edge = crate::fetch::tests::serve(playlist.as_bytes().to_vec(), false).await;
        let relay = Relay::start(
            Ipv4Addr::LOCALHOST,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            Arc::new(Redirecting { edge: edge.clone() }),
        )
        .await
        .unwrap();
        let asked = relay.upstream_url("http://origin.example/show/index.m3u8");
        let text =
            String::from_utf8(get(&asked, None).await.bytes(1 << 20).await.unwrap()).unwrap();
        let segment = format!("{}/u/{}", relay.base(), encode(&format!("{edge}/edge/seg0.ts")));
        assert!(text.contains(&segment), "{text}");
        // And the edge's segments may be fetched from now on.
        let origins = lock(&relay.shared.origins).clone();
        assert!(origins.contains(&origin(&edge).unwrap()), "{origins:?}");
    }

    #[tokio::test]
    async fn a_connection_kept_alive_is_closed_when_the_cast_ends() {
        let relay = Relay::start(
            Ipv4Addr::LOCALHOST,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            Arc::new(PlainHttp),
        )
        .await
        .unwrap();
        let kept = relay.share_bytes("clip.webm", "video/webm", b"twelve bytes");
        let (authority, path) = kept.trim_start_matches("http://").split_once('/').unwrap();
        let ask = format!("GET /{path} HTTP/1.1\r\nHost: tv\r\n\r\n");
        let mut socket = tokio::net::TcpStream::connect(authority).await.unwrap();
        socket.write_all(ask.as_bytes()).await.unwrap();
        let mut answer = vec![0u8; 512];
        let n = socket.read(&mut answer).await.unwrap();
        assert!(answer[..n].starts_with(b"HTTP/1.1 200"));

        drop(relay);
        tokio::time::sleep(Duration::from_millis(50)).await;
        // The same connection, asked again: nothing comes back but the close.
        let _ = socket.write_all(ask.as_bytes()).await;
        let mut rest = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut rest))
            .await
            .expect("the connection closed");
        assert!(read.is_err() || rest.is_empty(), "{}", String::from_utf8_lossy(&rest));
    }

    /// The process out of file descriptors, which only a Unix limit brings about in a test.
    #[cfg(unix)]
    mod descriptors {
        use super::*;

        /// Set in the process of its own the test runs in.
        const CHILD: &str = "KAERI_RELAY_OUT_OF_DESCRIPTORS";

        /// Too many open files, the same number on Linux and macOS.
        const EMFILE: i32 = 24;

        const CLIP: &[u8] = b"not a real clip";

        /// The process runs out of file descriptors for a moment, as a torrent's peers can make
        /// it, just as the TV opens a connection: the relay can't take it then. Once descriptors
        /// are free again, that connection is answered, the one the TV kept open still is, and so
        /// is the next. On Apple systems the kernel closes a connection it had no descriptor
        /// for, so there the TV sees that one close and opens another.
        #[test]
        fn the_relay_takes_connections_again_after_running_out_of_descriptors() {
            if std::env::var_os(CHILD).is_some() {
                let runtime =
                    tokio::runtime::Builder::new_current_thread().enable_all().build();
                runtime.unwrap().block_on(run_out());
                return;
            }
            // A process of its own with a low limit, so running out touches no other test.
            let module = module_path!().split_once("::").unwrap().1;
            let test = format!(
                "{module}::the_relay_takes_connections_again_after_running_out_of_descriptors"
            );
            let run = std::process::Command::new("sh")
                .args(["-c", "ulimit -S -n 64 && exec \"$0\" \"$@\""])
                .arg(std::env::current_exe().unwrap())
                .args(["--exact", &test, "--nocapture", "--test-threads", "1"])
                .env(CHILD, "1")
                .output()
                .unwrap();
            let said = format!(
                "{}{}",
                String::from_utf8_lossy(&run.stdout),
                String::from_utf8_lossy(&run.stderr)
            );
            assert!(run.status.success() && said.contains("1 passed"), "{said}");
        }

        /// The test, in a process limited to 64 descriptors.
        async fn run_out() {
            let relay = Relay::start(
                Ipv4Addr::LOCALHOST,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                Arc::new(PlainHttp),
            )
            .await
            .unwrap();
            let url = relay.share_bytes("clip.webm", "video/webm", CLIP);
            let (authority, path) = url.trim_start_matches("http://").split_once('/').unwrap();
            let mut kept = tokio::net::TcpStream::connect(authority).await.unwrap();
            assert!(ask(&mut kept, path).await.starts_with("HTTP/1.1 200"));

            // Every descriptor taken, the last one by the connection the TV opens.
            let mut taken = Vec::new();
            loop {
                match std::fs::File::open("/dev/null") {
                    Ok(file) => taken.push(file),
                    Err(e) if e.raw_os_error() == Some(EMFILE) => break,
                    Err(e) => panic!("{e}"),
                }
                assert!(taken.len() < 1024, "the limit of 64 never came");
            }
            taken.pop();
            let opened = std::net::TcpStream::connect(authority).unwrap();
            opened.set_nonblocking(true).unwrap();
            let mut opened = tokio::net::TcpStream::from_std(opened).unwrap();
            // The relay tries to take it, and can't.
            tokio::time::sleep(Duration::from_millis(300)).await;
            drop(taken);

            let answer = ask(&mut opened, path).await;
            let closed_by_the_system = cfg!(target_vendor = "apple") && answer.is_empty();
            assert!(
                answer.starts_with("HTTP/1.1 200") || closed_by_the_system,
                "the one opened meanwhile: {answer:?}"
            );
            let answer = ask(&mut kept, path).await;
            assert!(answer.starts_with("HTTP/1.1 200"), "the one kept open: {answer:?}");
            assert_eq!(get(&url, None).await.bytes(1024).await.unwrap(), CLIP);
        }

        /// Ask for `path` on `socket` and read the answer through to the clip's bytes, or what
        /// came before the connection failed or 10 s went by.
        async fn ask(socket: &mut tokio::net::TcpStream, path: &str) -> String {
            let mut answer = Vec::new();
            let asking = async {
                let request = format!("GET /{path} HTTP/1.1\r\nHost: tv\r\n\r\n");
                socket.write_all(request.as_bytes()).await?;
                let mut chunk = [0u8; 512];
                while !answer.ends_with(CLIP) {
                    let n = socket.read(&mut chunk).await?;
                    if n == 0 {
                        break;
                    }
                    answer.extend_from_slice(&chunk[..n]);
                }
                std::io::Result::Ok(())
            };
            let _ = tokio::time::timeout(Duration::from_secs(10), asking).await;
            String::from_utf8_lossy(&answer).into_owned()
        }
    }

    #[tokio::test]
    async fn the_relay_drops_anyone_but_its_tv() {
        let relay = Relay::start(
            Ipv4Addr::LOCALHOST,
            "192.0.2.7".parse().unwrap(),
            Arc::new(PlainHttp),
        )
        .await
        .unwrap();
        let url = relay.dynamic_url("t/1.vtt");
        assert!(PlainHttp.get(&url, &[]).await.is_err(), "closed before a byte");
        assert!(
            Relay::start(
                Ipv4Addr::UNSPECIFIED,
                "192.0.2.7".parse().unwrap(),
                Arc::new(PlainHttp)
            )
            .await
            .is_err()
        );
    }
}
