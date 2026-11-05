//! A WebDAV server as a folder target: `PROPFIND` to list, `PUT` to a temporary name and `MOVE`
//! into place so a reader never sees half a file and a taken name is refused.

use std::{fmt, time::Duration};

use async_trait::async_trait;
use reqwest::{Method, StatusCode, Url};
use ring::rand::{SecureRandom, SystemRandom};

use crate::folder::{Entry, FolderError, FolderTarget, check_path};

/// Names a writer has not moved into place yet; a listing leaves them out.
const TMP_PREFIX: &str = ".kaeri-tmp-";

/// A password that never prints.
#[derive(Clone)]
pub struct Password(String);

impl Password {
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(..)")
    }
}

#[derive(Clone)]
pub struct WebDavTarget {
    client: reqwest::Client,
    /// The server's folder the sync root lives in, as a URL.
    base: Url,
    user: String,
    password: Password,
    root: Vec<String>,
    /// The longest `Retry-After` one retry will wait for.
    max_retry_wait: Duration,
}

impl fmt::Debug for WebDavTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebDavTarget")
            .field("base", &self.base.as_str())
            .field("user", &self.user)
            .finish()
    }
}

struct Reply {
    status: StatusCode,
    body: Vec<u8>,
}

impl WebDavTarget {
    /// `base` is the server address including any path (`https://dav.example/files/ann`), `root`
    /// the folder under it that holds the log (`kaeri-sync`, or `a/b`).
    pub fn new(
        base: &str,
        user: &str,
        password: Password,
        root: &str,
    ) -> Result<Self, FolderError> {
        let mut base =
            Url::parse(base).map_err(|e| FolderError::BadPath(format!("{base}: {e}")))?;
        if !matches!(base.scheme(), "http" | "https") || base.host_str().is_none() {
            return Err(FolderError::BadPath(base.to_string()));
        }
        base.set_query(None);
        base.set_fragment(None);
        let client = reqwest::Client::builder()
            .user_agent("Kaeri")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(0)
            .build()
            .map_err(|e| FolderError::Io(e.to_string()))?;
        Ok(Self {
            client,
            base,
            user: user.to_owned(),
            password,
            root: root.split('/').filter(|s| !s.is_empty()).map(str::to_owned).collect(),
            max_retry_wait: Duration::from_secs(30),
        })
    }

    /// Cap the wait a `Retry-After` can cause. Tests set it to zero.
    #[must_use]
    pub fn with_max_retry_wait(mut self, wait: Duration) -> Self {
        self.max_retry_wait = wait;
        self
    }

    /// Ask the server about the root: for the Connect button, before anything is saved. A root
    /// that is not there yet is fine (the first write makes it); a server that is not there is not.
    pub async fn check(&self) -> Result<(), FolderError> {
        match self.propfind(&self.url_for(&self.root, false), "0").await {
            Ok(_) => Ok(()),
            Err(FolderError::NotFound(_)) => {
                self.propfind(&self.url_for(&[], false), "0").await.map(|_| ())
            }
            Err(e) => Err(e),
        }
    }

    fn url_for(&self, dirs: &[String], trailing: bool) -> Url {
        let mut url = self.base.clone();
        if let Ok(mut segs) = url.path_segments_mut() {
            segs.pop_if_empty();
            segs.extend(dirs);
            if trailing {
                segs.push("");
            }
        }
        url
    }

    fn url(&self, path: &str, trailing: bool) -> Url {
        let all: Vec<String> = self
            .root
            .iter()
            .cloned()
            .chain(path.split('/').filter(|s| !s.is_empty()).map(str::to_owned))
            .collect();
        self.url_for(&all, trailing)
    }

    async fn send(
        &self,
        method: &str,
        url: &Url,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<Reply, FolderError> {
        let mut retried = false;
        loop {
            let method = Method::from_bytes(method.as_bytes())
                .map_err(|e| FolderError::Io(e.to_string()))?;
            let mut req = self
                .client
                .request(method, url.clone())
                .basic_auth(&self.user, Some(self.password.expose()));
            for (k, v) in headers {
                req = req.header(*k, *v);
            }
            if let Some(body) = body {
                req = req.body(body.to_vec());
            }
            let resp = req
                .send()
                .await
                .map_err(|e| FolderError::Unreachable(e.without_url().to_string()))?;
            let status = resp.status();
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            let body = resp
                .bytes()
                .await
                .map_err(|e| FolderError::Unreachable(e.without_url().to_string()))?;
            if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
                if !retried {
                    retried = true;
                    tokio::time::sleep(
                        retry_after.unwrap_or(Duration::from_secs(1)).min(self.max_retry_wait),
                    )
                    .await;
                    continue;
                }
                return Err(FolderError::Server(status.as_u16().to_string()));
            }
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Err(FolderError::Unauthorized);
            }
            return Ok(Reply { status, body: body.to_vec() });
        }
    }

    async fn propfind(&self, url: &Url, depth: &str) -> Result<Vec<Entry>, FolderError> {
        const BODY: &[u8] = br#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/><d:getcontentlength/></d:prop></d:propfind>"#;
        let reply = self
            .send(
                "PROPFIND",
                url,
                &[("Depth", depth), ("Content-Type", "application/xml")],
                Some(BODY),
            )
            .await?;
        match reply.status.as_u16() {
            207 | 200 => parse_multistatus(&reply.body, url),
            404 => Err(FolderError::NotFound(url.path().to_owned())),
            other => Err(FolderError::Server(other.to_string())),
        }
    }

    /// Make each directory on the way down, the root first. One that is there already answers 405.
    async fn make_dirs(&self, dirs: &[String]) -> Result<(), FolderError> {
        for i in 1..=dirs.len() {
            let url = self.url_for(&dirs[..i], true);
            let reply = self.send("MKCOL", &url, &[], None).await?;
            match reply.status.as_u16() {
                200 | 201 | 204 | 301 | 405 => {}
                other => return Err(FolderError::Server(format!("{other} making a folder"))),
            }
        }
        Ok(())
    }

    fn parents_of(&self, path: &str) -> Vec<String> {
        let mut dirs = self.root.clone();
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        dirs.extend(parts[..parts.len().saturating_sub(1)].iter().map(|s| (*s).to_owned()));
        dirs
    }

    async fn put(&self, url: &Url, bytes: &[u8], dirs: &[String]) -> Result<(), FolderError> {
        let mut made = false;
        loop {
            let reply = self
                .send("PUT", url, &[("Content-Type", "application/octet-stream")], Some(bytes))
                .await?;
            match reply.status.as_u16() {
                200 | 201 | 204 => return Ok(()),
                // The folder it goes in is not there yet.
                404 | 409 if !made => {
                    made = true;
                    self.make_dirs(dirs).await?;
                }
                other => return Err(FolderError::Server(format!("{other} on write"))),
            }
        }
    }
}

fn tmp_name(name: &str) -> String {
    let mut random = [0u8; 6];
    let _ = SystemRandom::new().fill(&mut random);
    format!("{TMP_PREFIX}{name}-{}", hex::encode(random))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(v)) => {
                out.push(v);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_multistatus(xml: &[u8], requested: &Url) -> Result<Vec<Entry>, FolderError> {
    let text =
        std::str::from_utf8(xml).map_err(|e| FolderError::Server(format!("not text: {e}")))?;
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| FolderError::Server(format!("not a listing: {e}")))?;
    let own = percent_decode(requested.path()).trim_end_matches('/').to_owned();
    let mut out = Vec::new();
    for response in doc.descendants().filter(|n| n.tag_name().name() == "response") {
        let child_text = |tag: &str| {
            response
                .descendants()
                .find(|n| n.tag_name().name() == tag)
                .and_then(|n| n.text())
                .map(|t| t.trim().to_owned())
        };
        let Some(href) = child_text("href") else { continue };
        let path = match Url::parse(&href) {
            Ok(u) => u.path().to_owned(),
            Err(_) => href.split(['?', '#']).next().unwrap_or("").to_owned(),
        };
        let path = percent_decode(&path);
        let path = path.trim_end_matches('/');
        if path == own {
            continue;
        }
        let Some(name) = path.rsplit('/').next().filter(|n| !n.is_empty()) else { continue };
        if name.starts_with(TMP_PREFIX) {
            continue;
        }
        let is_dir = response.descendants().any(|n| n.tag_name().name() == "collection");
        let size = child_text("getcontentlength").and_then(|s| s.parse().ok()).unwrap_or(0);
        out.push(Entry { name: name.to_owned(), is_dir, size });
    }
    out.sort();
    Ok(out)
}

#[async_trait]
impl FolderTarget for WebDavTarget {
    async fn list(&self, dir: &str) -> Result<Vec<Entry>, FolderError> {
        check_path(dir)?;
        match self.propfind(&self.url(dir, true), "1").await {
            Err(FolderError::NotFound(_)) => Ok(Vec::new()),
            other => other,
        }
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, FolderError> {
        check_path(path)?;
        let reply = self.send("GET", &self.url(path, false), &[], None).await?;
        match reply.status.as_u16() {
            200 => Ok(reply.body),
            404 => Err(FolderError::NotFound(path.to_owned())),
            other => Err(FolderError::Server(other.to_string())),
        }
    }

    async fn write_new(&self, path: &str, bytes: &[u8]) -> Result<(), FolderError> {
        check_path(path)?;
        let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
        let tmp_path =
            if dir.is_empty() { tmp_name(name) } else { format!("{dir}/{}", tmp_name(name)) };
        let (tmp, dest) = (self.url(&tmp_path, false), self.url(path, false));
        self.put(&tmp, bytes, &self.parents_of(path)).await?;
        let moved = self
            .send("MOVE", &tmp, &[("Destination", dest.as_str()), ("Overwrite", "F")], None)
            .await;
        match moved {
            Ok(reply) if matches!(reply.status.as_u16(), 200 | 201 | 204) => Ok(()),
            Ok(reply) => {
                let _ = self.send("DELETE", &tmp, &[], None).await;
                match reply.status.as_u16() {
                    412 => Err(FolderError::Exists(path.to_owned())),
                    other => Err(FolderError::Server(format!("{other} on move"))),
                }
            }
            Err(e) => {
                let _ = self.send("DELETE", &tmp, &[], None).await;
                Err(e)
            }
        }
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FolderError> {
        check_path(from)?;
        check_path(to)?;
        let (src, dest) = (self.url(from, false), self.url(to, false));
        let mut made = false;
        loop {
            let reply = self
                .send("MOVE", &src, &[("Destination", dest.as_str()), ("Overwrite", "T")], None)
                .await?;
            match reply.status.as_u16() {
                200 | 201 | 204 => return Ok(()),
                409 if !made => {
                    made = true;
                    self.make_dirs(&self.parents_of(to)).await?;
                }
                404 => return Err(FolderError::NotFound(from.to_owned())),
                other => return Err(FolderError::Server(format!("{other} on move"))),
            }
        }
    }

    async fn remove(&self, path: &str) -> Result<(), FolderError> {
        check_path(path)?;
        let reply = self.send("DELETE", &self.url(path, false), &[], None).await?;
        match reply.status.as_u16() {
            200 | 202 | 204 | 404 => Ok(()),
            other => Err(FolderError::Server(format!("{other} on delete"))),
        }
    }

    fn describe(&self) -> String {
        self.base.host_str().unwrap_or("webdav").to_owned()
    }
}

#[cfg(any(test, feature = "testing"))]
pub use fake::FakeDav;

/// A WebDAV server, and a Nextcloud login, on the loopback interface, for tests and Fixtures builds.
#[cfg(any(test, feature = "testing"))]
mod fake {
    use std::{
        collections::{BTreeMap, BTreeSet, VecDeque},
        fmt::Write as _,
        net::SocketAddr,
        sync::{Arc, Mutex, MutexGuard, PoisonError},
    };

    use base64::{Engine, engine::general_purpose::STANDARD};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };

    #[derive(Default)]
    struct State {
        files: BTreeMap<String, Vec<u8>>,
        dirs: BTreeSet<String>,
        creds: Option<(String, String)>,
        fail: VecDeque<u16>,
        offline: bool,
        log: Vec<String>,
        /// Login Flow v2: set when the person has "approved" in the browser.
        approved: Option<(String, String)>,
        login_user_agent: Option<String>,
    }

    /// Clones share the one server. Dropping the last clone stops it.
    #[derive(Clone)]
    pub struct FakeDav {
        state: Arc<Mutex<State>>,
        addr: SocketAddr,
        _task: Arc<Task>,
    }

    struct Task(tokio::task::JoinHandle<()>);
    impl Drop for Task {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    fn lock(s: &Mutex<State>) -> MutexGuard<'_, State> {
        s.lock().unwrap_or_else(PoisonError::into_inner)
    }

    impl FakeDav {
        pub async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
            let addr = listener.local_addr().expect("addr");
            let state = Arc::new(Mutex::new(State::default()));
            let shared = Arc::clone(&state);
            let task = tokio::spawn(async move {
                loop {
                    let Ok((stream, _)) = listener.accept().await else { break };
                    let state = Arc::clone(&shared);
                    tokio::spawn(async move {
                        let _ = serve(stream, state, addr).await;
                    });
                }
            });
            Self { state, addr, _task: Arc::new(Task(task)) }
        }

        /// The server's address, `http://127.0.0.1:port`.
        pub fn url(&self) -> String {
            format!("http://{}", self.addr)
        }

        /// Refuse any request that does not carry this Basic sign-in.
        pub fn require_credentials(&self, user: &str, password: &str) {
            lock(&self.state).creds = Some((user.to_owned(), password.to_owned()));
        }

        /// Answer the next `n` requests with `status` (429 and 5xx carry `Retry-After: 0`).
        pub fn fail_next(&self, n: usize, status: u16) {
            lock(&self.state).fail.extend(std::iter::repeat_n(status, n));
        }

        /// Drop every connection with no answer, as an unreachable server does, and stop again.
        pub fn set_offline(&self, offline: bool) {
            lock(&self.state).offline = offline;
        }

        /// Every request seen, as `METHOD /decoded/path`.
        pub fn requests(&self) -> Vec<String> {
            lock(&self.state).log.clone()
        }

        /// Every file held, by path.
        pub fn files(&self) -> Vec<String> {
            lock(&self.state).files.keys().cloned().collect()
        }

        /// The person approves the Login Flow v2 sign-in in their browser: the next poll hands out
        /// `login_name` and `app_password`, and the server then requires them.
        pub fn approve_login(&self, login_name: &str, app_password: &str) {
            let mut s = lock(&self.state);
            s.approved = Some((login_name.to_owned(), app_password.to_owned()));
            s.creds = Some((login_name.to_owned(), app_password.to_owned()));
            for dir in ["remote.php", "remote.php/dav", "remote.php/dav/files"] {
                s.dirs.insert(dir.to_owned());
            }
            s.dirs.insert(format!("remote.php/dav/files/{login_name}"));
        }

        /// The `User-Agent` the login start came with.
        pub fn login_user_agent(&self) -> Option<String> {
            lock(&self.state).login_user_agent.clone()
        }
    }

    struct Req {
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl Req {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        }
    }

    async fn read_req(stream: &mut TcpStream) -> Option<Req> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        let head_end = loop {
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i;
            }
            let n = stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                return None;
            }
            buf.extend_from_slice(&chunk[..n]);
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
        let mut lines = head.split("\r\n");
        let mut first = lines.next()?.split(' ');
        let method = first.next()?.to_owned();
        let path = first.next()?.to_owned();
        let headers: Vec<(String, String)> = lines
            .filter_map(|l| {
                l.split_once(':').map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
            })
            .collect();
        let len = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.parse::<usize>().ok())
            .unwrap_or(0);
        let mut body = buf[head_end + 4..].to_vec();
        while body.len() < len {
            let n = stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                return None;
            }
            body.extend_from_slice(&chunk[..n]);
        }
        body.truncate(len);
        Some(Req { method, path, headers, body })
    }

    struct Resp {
        status: u16,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
    }

    fn resp(status: u16) -> Resp {
        Resp { status, headers: Vec::new(), body: Vec::new() }
    }

    async fn serve(
        mut stream: TcpStream,
        state: Arc<Mutex<State>>,
        addr: SocketAddr,
    ) -> std::io::Result<()> {
        if lock(&state).offline {
            return Ok(());
        }
        let Some(req) = read_req(&mut stream).await else { return Ok(()) };
        let out = handle(&req, &state, addr);
        let mut head = format!(
            "HTTP/1.1 {} Status\r\nConnection: close\r\nContent-Length: {}\r\n",
            out.status,
            out.body.len()
        );
        for (k, v) in &out.headers {
            let _ = write!(head, "{k}: {v}\r\n");
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes()).await?;
        stream.write_all(&out.body).await?;
        stream.shutdown().await
    }

    fn parent_of(path: &str) -> &str {
        path.rsplit_once('/').map_or("", |(p, _)| p)
    }

    fn is_dir(s: &State, p: &str) -> bool {
        p.is_empty() || s.dirs.contains(p)
    }

    fn json(body: String) -> Resp {
        let mut r = resp(200);
        r.headers.push(("Content-Type", "application/json".into()));
        r.body = body.into_bytes();
        r
    }

    fn handle(req: &Req, state: &Mutex<State>, addr: SocketAddr) -> Resp {
        let raw_path = req.path.split('?').next().unwrap_or("");
        let path = super::percent_decode(raw_path).trim_matches('/').to_owned();
        let mut s = lock(state);
        s.log.push(format!("{} /{path}", req.method));
        if let Some(status) = s.fail.pop_front() {
            let mut r = resp(status);
            if status == 429 || status >= 500 {
                r.headers.push(("Retry-After", "0".into()));
            }
            return r;
        }
        // Nextcloud Login Flow v2 needs no sign-in.
        if path == "index.php/login/v2" && req.method == "POST" {
            s.login_user_agent = req.header("user-agent").map(str::to_owned);
            return json(format!(
                r#"{{"poll":{{"token":"poll-token","endpoint":"http://{addr}/index.php/login/v2/poll"}},"login":"http://{addr}/index.php/login/v2/flow/poll-token"}}"#
            ));
        }
        if path == "index.php/login/v2/poll" && req.method == "POST" {
            let body = String::from_utf8_lossy(&req.body);
            let Some((name, pass)) =
                s.approved.clone().filter(|_| body.contains("token=poll-token"))
            else {
                return resp(404);
            };
            return json(format!(
                r#"{{"server":"http://{addr}","loginName":"{name}","appPassword":"{pass}"}}"#
            ));
        }
        if let Some((user, pass)) = &s.creds {
            let want = format!("Basic {}", STANDARD.encode(format!("{user}:{pass}")));
            if req.header("authorization") != Some(want.as_str()) {
                let mut r = resp(401);
                r.headers.push(("WWW-Authenticate", "Basic realm=\"fake\"".into()));
                return r;
            }
        }
        match req.method.as_str() {
            "MKCOL" => {
                if is_dir(&s, &path) || s.files.contains_key(&path) {
                    resp(405)
                } else if !is_dir(&s, parent_of(&path)) {
                    resp(409)
                } else {
                    s.dirs.insert(path);
                    resp(201)
                }
            }
            "PUT" => {
                if is_dir(&s, &path) {
                    resp(405)
                } else if !is_dir(&s, parent_of(&path)) {
                    resp(409)
                } else {
                    let existed = s.files.insert(path, req.body.clone()).is_some();
                    resp(if existed { 204 } else { 201 })
                }
            }
            "GET" => match s.files.get(&path) {
                Some(bytes) => {
                    let mut r = resp(200);
                    r.body = bytes.clone();
                    r
                }
                None => resp(404),
            },
            "DELETE" => {
                let prefix = format!("{path}/");
                let had = s.files.remove(&path).is_some() | s.dirs.remove(&path);
                s.files.retain(|k, _| !k.starts_with(&prefix));
                s.dirs.retain(|k| !k.starts_with(&prefix));
                resp(if had { 204 } else { 404 })
            }
            "MOVE" => {
                let Some(dest) = req.header("destination").map(|d| {
                    let p = match d.split_once("://") {
                        Some((_, rest)) => {
                            rest.find('/').map_or_else(String::new, |i| rest[i..].to_owned())
                        }
                        None => d.to_owned(),
                    };
                    super::percent_decode(&p).trim_matches('/').to_owned()
                }) else {
                    return resp(400);
                };
                let Some(bytes) = s.files.get(&path).cloned() else { return resp(404) };
                if !is_dir(&s, parent_of(&dest)) {
                    return resp(409);
                }
                let taken = s.files.contains_key(&dest) || s.dirs.contains(&dest);
                if taken && req.header("overwrite") == Some("F") {
                    return resp(412);
                }
                s.files.remove(&path);
                s.files.insert(dest, bytes);
                resp(if taken { 204 } else { 201 })
            }
            "PROPFIND" => propfind(&s, &path, req.header("depth") == Some("1")),
            _ => resp(405),
        }
    }

    fn encode_href(path: &str) -> String {
        let mut out = String::new();
        for b in path.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~') {
                out.push(b as char);
            } else {
                let _ = write!(out, "%{b:02X}");
            }
        }
        out
    }

    fn entry(href: &str, dir: bool, size: usize) -> String {
        let enc = encode_href(href);
        if dir {
            format!(
                "<d:response><d:href>{enc}/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
            )
        } else {
            format!(
                "<d:response><d:href>{enc}</d:href><d:propstat><d:prop><d:resourcetype/><d:getcontentlength>{size}</d:getcontentlength></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
            )
        }
    }

    fn propfind(s: &State, path: &str, depth1: bool) -> Resp {
        let is_file = s.files.contains_key(path);
        if !is_file && !is_dir(s, path) {
            return resp(404);
        }
        let mut xml = String::from(r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:">"#);
        let size = s.files.get(path).map_or(0, Vec::len);
        xml.push_str(&entry(&format!("/{path}"), !is_file, size));
        if depth1 && !is_file {
            let prefix = if path.is_empty() { String::new() } else { format!("{path}/") };
            for (k, v) in &s.files {
                if k.strip_prefix(&prefix).is_some_and(|rest| !rest.contains('/')) {
                    xml.push_str(&entry(&format!("/{k}"), false, v.len()));
                }
            }
            for d in &s.dirs {
                if d.strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.contains('/') && !rest.is_empty())
                {
                    xml.push_str(&entry(&format!("/{d}"), true, 0));
                }
            }
        }
        xml.push_str("</d:multistatus>");
        let mut r = resp(207);
        r.headers.push(("Content-Type", "application/xml".into()));
        r.body = xml.into_bytes();
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(dav: &FakeDav) -> WebDavTarget {
        WebDavTarget::new(&dav.url(), "ann", Password::new("pw"), "kaeri-sync")
            .unwrap()
            .with_max_retry_wait(Duration::ZERO)
    }

    #[tokio::test]
    async fn keeps_the_folder_contract() {
        let dav = FakeDav::start().await;
        let t = target(&dav);
        assert!(t.list("").await.unwrap().is_empty());
        t.write_new("a/b/one.seg", b"1").await.unwrap();
        t.write_new("a/two.seg", b"22").await.unwrap();
        assert!(matches!(t.write_new("a/two.seg", b"x").await, Err(FolderError::Exists(_))));
        let root = t.list("").await.unwrap();
        assert_eq!(root.len(), 1);
        assert!(root[0].is_dir);
        let a = t.list("a").await.unwrap();
        assert_eq!(
            a.iter().map(|e| (e.name.as_str(), e.is_dir)).collect::<Vec<_>>(),
            [("b", true), ("two.seg", false)]
        );
        assert_eq!(a[1].size, 2);
        assert_eq!(t.read("a/two.seg").await.unwrap(), b"22");
        t.rename("a/two.seg", "a/three.seg").await.unwrap();
        assert!(matches!(t.read("a/two.seg").await, Err(FolderError::NotFound(_))));
        t.remove("a/three.seg").await.unwrap();
        t.remove("a/three.seg").await.unwrap();
        assert!(matches!(t.read("../x").await, Err(FolderError::BadPath(_))));
    }

    #[tokio::test]
    async fn second_write_to_a_name_is_refused_and_keeps_the_first() {
        let dav = FakeDav::start().await;
        let t = target(&dav);
        t.write_new("d/x.seg", b"first").await.unwrap();
        assert!(matches!(t.write_new("d/x.seg", b"second").await, Err(FolderError::Exists(_))));
        assert_eq!(t.read("d/x.seg").await.unwrap(), b"first");
        // The refused write left no temporary file behind.
        assert_eq!(dav.files(), ["kaeri-sync/d/x.seg"]);
    }

    #[tokio::test]
    async fn a_write_goes_through_a_temporary_name_that_is_never_listed() {
        let dav = FakeDav::start().await;
        let t = target(&dav);
        t.write_new("x.seg", b"data").await.unwrap();
        let log = dav.requests();
        assert!(
            log.iter().any(|l| l.starts_with("PUT /kaeri-sync/.kaeri-tmp-x.seg-")),
            "{log:?}"
        );
        assert!(log.iter().any(|l| l.starts_with("MOVE /kaeri-sync/.kaeri-tmp-x.seg-")));
        // A half-written file from another device's crashed write is not listed either.
        let tmp = t.url(".kaeri-tmp-y.seg-abc", false);
        t.send("PUT", &tmp, &[], Some(b"half")).await.unwrap();
        let names: Vec<String> =
            t.list("").await.unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["x.seg"]);
    }

    #[tokio::test]
    async fn bad_credentials_are_unauthorized() {
        let dav = FakeDav::start().await;
        dav.require_credentials("ann", "right");
        let t =
            WebDavTarget::new(&dav.url(), "ann", Password::new("wrong"), "kaeri-sync").unwrap();
        assert!(matches!(t.list("").await, Err(FolderError::Unauthorized)));
        assert!(matches!(t.write_new("a.seg", b"1").await, Err(FolderError::Unauthorized)));
        assert!(matches!(t.check().await, Err(FolderError::Unauthorized)));
        let ok =
            WebDavTarget::new(&dav.url(), "ann", Password::new("right"), "kaeri-sync").unwrap();
        ok.check().await.unwrap();
        ok.write_new("a.seg", b"1").await.unwrap();
    }

    #[tokio::test]
    async fn forbidden_is_unauthorized_too() {
        let dav = FakeDav::start().await;
        dav.fail_next(1, 403);
        assert!(matches!(target(&dav).read("a").await, Err(FolderError::Unauthorized)));
    }

    #[tokio::test]
    async fn one_429_is_retried() {
        let dav = FakeDav::start().await;
        let t = target(&dav);
        t.write_new("a.seg", b"1").await.unwrap();
        dav.fail_next(1, 429);
        assert_eq!(t.read("a.seg").await.unwrap(), b"1");
    }

    #[tokio::test]
    async fn a_second_failure_in_a_row_is_a_server_error() {
        let dav = FakeDav::start().await;
        let t = target(&dav);
        dav.fail_next(2, 500);
        assert!(matches!(t.read("a.seg").await, Err(FolderError::Server(_))));
        dav.fail_next(2, 429);
        assert!(matches!(t.list("").await, Err(FolderError::Server(_))));
    }

    #[tokio::test]
    async fn offline_is_unreachable_and_recovers() {
        let dav = FakeDav::start().await;
        let t = target(&dav);
        t.write_new("a.seg", b"1").await.unwrap();
        dav.set_offline(true);
        assert!(matches!(t.read("a.seg").await, Err(FolderError::Unreachable(_))));
        assert!(matches!(t.check().await, Err(FolderError::Unreachable(_))));
        dav.set_offline(false);
        assert_eq!(t.read("a.seg").await.unwrap(), b"1");
    }

    #[tokio::test]
    async fn nothing_listening_is_unreachable() {
        let dav = FakeDav::start().await;
        let url = dav.url();
        drop(dav);
        tokio::time::sleep(Duration::from_millis(50)).await;
        let t = WebDavTarget::new(&url, "a", Password::new("b"), "x").unwrap();
        assert!(matches!(t.check().await, Err(FolderError::Unreachable(_))));
    }

    #[tokio::test]
    async fn spaces_and_unicode_in_paths_round_trip() {
        let dav = FakeDav::start().await;
        let t = WebDavTarget::new(&dav.url(), "ann", Password::new("pw"), "my sync/kaeri ä")
            .unwrap();
        t.write_new("dev ice/ö 1 #%.seg", b"hej").await.unwrap();
        assert_eq!(t.read("dev ice/ö 1 #%.seg").await.unwrap(), b"hej");
        let listed = t.list("dev ice").await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "ö 1 #%.seg");
        assert_eq!(dav.files(), ["my sync/kaeri ä/dev ice/ö 1 #%.seg"]);
    }

    #[tokio::test]
    async fn a_one_mebibyte_body_round_trips() {
        let dav = FakeDav::start().await;
        let t = target(&dav);
        let body: Vec<u8> = (0..1024 * 1024).map(|i| (i % 251) as u8).collect();
        t.write_new("big.seg", &body).await.unwrap();
        assert_eq!(t.read("big.seg").await.unwrap(), body);
        assert_eq!(t.list("").await.unwrap()[0].size, 1024 * 1024);
    }

    #[tokio::test]
    async fn nested_parents_are_made_one_by_one() {
        let dav = FakeDav::start().await;
        let t = WebDavTarget::new(&dav.url(), "ann", Password::new("pw"), "a/b").unwrap();
        t.write_new("c/d/e/f.seg", b"1").await.unwrap();
        let mkcols: Vec<String> =
            dav.requests().into_iter().filter(|l| l.starts_with("MKCOL")).collect();
        assert_eq!(
            mkcols,
            ["MKCOL /a", "MKCOL /a/b", "MKCOL /a/b/c", "MKCOL /a/b/c/d", "MKCOL /a/b/c/d/e"]
        );
        // A second write finds them all and makes none.
        t.write_new("c/d/e/g.seg", b"2").await.unwrap();
        assert_eq!(dav.requests().iter().filter(|l| l.starts_with("MKCOL")).count(), 5);
    }

    #[tokio::test]
    async fn rename_replaces_and_makes_parents() {
        let dav = FakeDav::start().await;
        let t = target(&dav);
        t.write_new("a.seg", b"old").await.unwrap();
        t.write_new("b.seg", b"new").await.unwrap();
        t.rename("b.seg", "a.seg").await.unwrap();
        assert_eq!(t.read("a.seg").await.unwrap(), b"new");
        t.rename("a.seg", "sub/dir/c.seg").await.unwrap();
        assert_eq!(t.read("sub/dir/c.seg").await.unwrap(), b"new");
        assert!(matches!(t.rename("nope", "x").await, Err(FolderError::NotFound(_))));
    }

    #[tokio::test]
    async fn check_accepts_a_missing_root_and_describe_is_the_host() {
        let dav = FakeDav::start().await;
        let t = target(&dav);
        t.check().await.unwrap();
        assert_eq!(t.describe(), "127.0.0.1");
    }

    #[tokio::test]
    async fn a_base_url_with_a_path_is_kept() {
        let dav = FakeDav::start().await;
        let mk = |u: String| WebDavTarget::new(&u, "ann", Password::new("pw"), "ks").unwrap();
        let t = mk(format!("{}/dav/files/ann/", dav.url()));
        // The fake has no /dav folder yet: the write makes the root and nothing above it.
        assert!(t.write_new("x.seg", b"1").await.is_err());
        assert!(dav.requests().iter().any(|l| l.starts_with("PUT /dav/files/ann/ks/")));
    }

    #[test]
    fn the_password_does_not_print() {
        let t = WebDavTarget::new("http://dav.example", "ann", Password::new("hunter2"), "r")
            .unwrap();
        let shown = format!("{t:?} {:?}", Password::new("hunter2"));
        assert!(!shown.contains("hunter2"));
    }

    #[test]
    fn a_bad_url_is_refused() {
        assert!(WebDavTarget::new("not a url", "a", Password::new("b"), "r").is_err());
        assert!(WebDavTarget::new("ftp://dav.example", "a", Password::new("b"), "r").is_err());
    }
}
