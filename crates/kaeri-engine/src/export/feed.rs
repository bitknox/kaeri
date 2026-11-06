//! The OPDS feed: this computer's downloads and folders served as an OPDS 1.2 catalogue, with
//! the page-streaming extension, to e-readers on the home network.
//!
//! Off until turned on, never without a password (any user name, the password in HTTP Basic),
//! and only while Kaeri runs: the listener is a task of the engine, gone with it. What it serves
//! is only what the library lists, found by id in the database, never by a path from the
//! request. The documents are written by [`kaeri_books::opds`] and read back by
//! `kaeri-bookserver` in the tests. The HTTP is the little this needs: GET and HEAD, one request
//! per connection.

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use kaeri_books::{
    Book, Limits,
    opds::{
        Entry, Feed as Document, FeedKind, Link, REL_ACQUISITION, REL_IMAGE, REL_SUBSECTION,
        REL_THUMBNAIL,
    },
};
use kaeri_core::config::FeedConfig;
use kaeri_model::{
    event::Event,
    reading::{FeedView, ReadingEvent},
};
use kaeri_store::{Store, book_downloads::BookDownloadState};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

use crate::outbox::Outbox;

/// How many ports past the wanted one are tried when it is taken.
const PORT_TRIES: u16 = 10;
const READ_LIMIT: usize = 16 * 1024;
const READ_TIME: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Running {
    task: Option<JoinHandle<()>>,
    port: u16,
    password: String,
    wanted: Option<u16>,
}

/// The feed the engine holds. Cheap to clone.
#[derive(Clone, Default)]
pub struct Feed(Arc<Mutex<Running>>);

impl std::fmt::Debug for Feed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Feed").field("port", &self.port()).finish_non_exhaustive()
    }
}

/// The address of this computer on the local network, found without sending anything.
fn local_address() -> Option<IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then_some(ip)
}

impl Feed {
    fn state(&self) -> std::sync::MutexGuard<'_, Running> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The port it listens on, when it does.
    pub fn port(&self) -> Option<u16> {
        let state = self.state();
        state.task.as_ref().map(|_| state.port)
    }

    /// What the settings pane shows.
    pub fn view(&self, config: &FeedConfig, store: &Store) -> FeedView {
        let port = self.port();
        let addresses = match (port, local_address()) {
            (Some(port), Some(ip)) => vec![format!("http://{ip}:{port}/opds")],
            (Some(port), None) => vec![format!("http://localhost:{port}/opds")],
            _ => Vec::new(),
        };
        FeedView {
            on: port.is_some(),
            addresses,
            port: port.unwrap_or(0),
            password_set: !config.password.is_empty(),
            serving: served(store),
        }
    }

    /// Starts, moves or stops the listener to match the config, and says how it stands.
    pub fn apply(&self, config: &FeedConfig, store: Store, tx: &Outbox) {
        let want = config.on && !config.password.is_empty();
        let wanted = config.port.filter(|p| *p != 0);
        {
            let mut state = self.state();
            let same = state.task.is_some()
                && want
                && state.password == config.password
                && state.wanted == wanted;
            if same {
                return;
            }
            if let Some(task) = state.task.take() {
                task.abort();
            }
            if want {
                match bind(wanted) {
                    Some(listener) => {
                        state.port = listener.local_addr().map_or(0, |a| a.port());
                        state.password = config.password.clone();
                        state.wanted = wanted;
                        let password = config.password.clone();
                        let store = store.clone();
                        state.task = Some(tokio::spawn(serve(listener, password, store)));
                    }
                    None => tracing::warn!("the feed found no port to listen on"),
                }
            }
        }
        let _ = tx.send(Event::Reading(ReadingEvent::Feed(self.view(config, &store))));
    }
}

/// Binds the wanted port or the next free one; any free port when none is wanted.
fn bind(wanted: Option<u16>) -> Option<TcpListener> {
    let ports: Vec<u16> = match wanted {
        Some(p) => (p..=p.saturating_add(PORT_TRIES)).collect(),
        None => vec![0],
    };
    for port in ports {
        let Ok(listener) = std::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port)))
        else {
            continue;
        };
        if listener.set_nonblocking(true).is_ok()
            && let Ok(listener) = TcpListener::from_std(listener)
        {
            return Some(listener);
        }
    }
    None
}

async fn serve(listener: TcpListener, password: String, store: Store) {
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let (password, store) = (password.clone(), store.clone());
        tokio::spawn(async move {
            let _ = tokio::time::timeout(
                Duration::from_secs(300),
                handle(stream, &password, store),
            )
            .await;
        });
    }
}

/// The books the feed lists.
fn served(store: &Store) -> u32 {
    let downloads = store
        .book_downloads(None)
        .map(|rows| rows.iter().filter(|d| d.state == BookDownloadState::Done).count())
        .unwrap_or(0);
    let folders: usize = store
        .local_book_series()
        .unwrap_or_default()
        .iter()
        .map(|s| store.local_book_files(&s.id).map(|f| f.len()).unwrap_or(0))
        .sum();
    u32::try_from(downloads + folders).unwrap_or(u32::MAX)
}

// ---------------------------------------------------------------------------------------------
// HTTP

struct Request {
    head: bool,
    path: String,
    authorization: Option<String>,
}

async fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > READ_LIMIT {
            return None;
        }
        let n = tokio::time::timeout(READ_TIME, stream.read(&mut chunk)).await.ok()?.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut lines = text.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?;
    let target = first.next()?;
    if method != "GET" && method != "HEAD" {
        return None;
    }
    let authorization = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.trim().to_owned());
    let path = target.split(['?', '#']).next().unwrap_or("").to_owned();
    Some(Request { head: method == "HEAD", path, authorization })
}

/// Whether the request carries the password, in Basic authorization with any user name.
fn authorised(request: &Request, password: &str) -> bool {
    let Some(value) = request.authorization.as_deref() else { return false };
    let Some(encoded) = value.strip_prefix("Basic ").or_else(|| value.strip_prefix("basic "))
    else {
        return false;
    };
    let Ok(decoded) = STANDARD.decode(encoded.trim()) else { return false };
    let Ok(decoded) = String::from_utf8(decoded) else { return false };
    let given = decoded.split_once(':').map_or("", |(_, p)| p);
    let (a, b) = (given.as_bytes(), password.as_bytes());
    // The whole length is compared whatever differs, so the time says nothing.
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
    }
    diff == 0
}

enum Body {
    Bytes(Vec<u8>),
    File(PathBuf),
}

struct Response {
    status: &'static str,
    content_type: String,
    body: Body,
    challenge: bool,
}

impl Response {
    fn text(status: &'static str, text: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8".into(),
            body: Body::Bytes(text.as_bytes().to_vec()),
            challenge: false,
        }
    }

    fn document(feed: &Document) -> Self {
        Self {
            status: "200 OK",
            content_type: format!("{}; charset=utf-8", feed.media_type()),
            body: Body::Bytes(feed.to_xml().into_bytes()),
            challenge: false,
        }
    }
}

async fn handle(mut stream: TcpStream, password: &str, store: Store) {
    let Some(request) = read_request(&mut stream).await else {
        let _ =
            send(&mut stream, Response::text("400 Bad Request", "Bad request"), false).await;
        return;
    };
    let response = if authorised(&request, password) {
        route(&request.path, store).await
    } else {
        Response {
            challenge: true,
            ..Response::text("401 Unauthorized", "Sign in with the feed password.")
        }
    };
    let _ = send(&mut stream, response, request.head).await;
}

async fn send(stream: &mut TcpStream, response: Response, head: bool) -> std::io::Result<()> {
    let (length, file) = match &response.body {
        Body::Bytes(b) => (b.len() as u64, None),
        Body::File(path) => match tokio::fs::File::open(path).await {
            Ok(f) => (f.metadata().await?.len(), Some(f)),
            Err(_) => {
                let body = b"Not found";
                let h = format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(h.as_bytes()).await?;
                if !head {
                    stream.write_all(body).await?;
                }
                return stream.shutdown().await;
            }
        },
    };
    let mut header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {length}\r\nConnection: close\r\nCache-Control: no-store\r\n",
        response.status, response.content_type
    );
    if response.challenge {
        header.push_str("WWW-Authenticate: Basic realm=\"Kaeri\"\r\n");
    }
    header.push_str("\r\n");
    stream.write_all(header.as_bytes()).await?;
    if !head {
        match (response.body, file) {
            (Body::Bytes(b), _) => stream.write_all(&b).await?,
            (Body::File(_), Some(mut f)) => {
                tokio::io::copy(&mut f, stream).await?;
            }
            _ => {}
        }
    }
    stream.shutdown().await
}

// ---------------------------------------------------------------------------------------------
// The catalogue

fn decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = segment.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn stamp(secs: i64) -> String {
    jiff::Timestamp::from_second(secs).map_or_else(
        |_| "1970-01-01T00:00:00Z".into(),
        |t| t.strftime("%Y-%m-%dT%H:%M:%SZ").to_string(),
    )
}

fn media_type(format: &str) -> &'static str {
    match format {
        "cbz" => "application/vnd.comicbook+zip",
        "cbr" => "application/vnd.comicbook-rar",
        "cb7" => "application/x-cb7",
        "epub" => "application/epub+zip",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn image_type(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xFF, 0xD8, ..] => "image/jpeg",
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'G', b'I', b'F', ..] => "image/gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        _ => "application/octet-stream",
    }
}

/// A book the feed can serve.
struct Shelved {
    title: String,
    path: PathBuf,
    format: String,
    pages: Option<u32>,
    updated: i64,
    /// The segments after `/opds/book/` that name it.
    key: String,
}

fn title_of(store: &Store, work: &str) -> String {
    store
        .book_work_kind(work)
        .ok()
        .flatten()
        .and_then(|k| k.title)
        .unwrap_or_else(|| work.to_owned())
}

fn download_books(store: &Store, work: &str) -> Vec<Shelved> {
    let title = title_of(store, work);
    let mut rows: Vec<_> = store
        .book_downloads(Some(work))
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.state == BookDownloadState::Done && d.path.is_some())
        .collect();
    rows.sort_by(|a, b| kaeri_books::natural::compare(&a.chapter, &b.chapter));
    rows.into_iter()
        .map(|d| {
            let path = PathBuf::from(d.path.unwrap_or_default());
            let format = path
                .extension()
                .and_then(|e| e.to_str())
                .map_or_else(String::new, str::to_ascii_lowercase);
            Shelved {
                title: format!("{title} {}", d.chapter),
                path,
                format,
                pages: d.pages_total,
                updated: d.updated_at,
                key: format!("d/{}", d.id),
            }
        })
        .collect()
}

fn folder_books(store: &Store, series: &str) -> Vec<Shelved> {
    let name = store
        .local_book_series()
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.id == series)
        .map_or_else(|| series.to_owned(), |s| s.name);
    let mut files = store.local_book_files(series).unwrap_or_default();
    files.sort_by(|a, b| kaeri_books::natural::compare(&a.path, &b.path));
    files
        .into_iter()
        .enumerate()
        .map(|(n, f)| Shelved {
            title: format!("{name} {}", f.label),
            path: PathBuf::from(&f.path),
            format: f.format.clone(),
            pages: f.pages,
            updated: f.mtime,
            key: format!("f/{}/{n}", encode(series)),
        })
        .collect()
}

/// A book by the segments of its address.
fn find_book(store: &Store, kind: &str, first: &str, second: Option<&str>) -> Option<Shelved> {
    match kind {
        "d" => {
            let id: i64 = first.parse().ok()?;
            let row = store.book_downloads(None).ok()?.into_iter().find(|d| d.id == id)?;
            download_books(store, &row.work).into_iter().find(|b| b.key == format!("d/{id}"))
        }
        "f" => {
            let series = decode(first);
            let n: usize = second?.parse().ok()?;
            folder_books(store, &series).into_iter().nth(n)
        }
        _ => None,
    }
}

fn base_feed(id: &str, title: &str, kind: FeedKind, selfref: &str) -> Document {
    let mut feed = Document::new(id, title, &stamp(0), kind);
    feed.links.push(Link::feed("self", selfref, kind));
    feed.links.push(Link::feed("start", "/opds", FeedKind::Navigation));
    feed
}

fn section(
    id: &str,
    title: &str,
    href: &str,
    kind: FeedKind,
    summary: Option<String>,
) -> Entry {
    Entry {
        id: id.into(),
        title: title.into(),
        updated: stamp(0),
        summary,
        links: vec![Link::feed(REL_SUBSECTION, href, kind)],
        ..Entry::default()
    }
}

fn book_entry(book: &Shelved) -> Entry {
    let base = format!("/opds/book/{}", book.key);
    let mut links = Vec::new();
    if book.format != "folder" {
        links.push(Link::new(
            REL_ACQUISITION,
            &format!("{base}/file"),
            media_type(&book.format),
        ));
    }
    let pages = matches!(book.format.as_str(), "cbz" | "cbr" | "cb7" | "folder");
    if pages {
        links.push(Link::new(REL_IMAGE, &format!("{base}/page/0"), "image/jpeg"));
        links.push(Link::new(REL_THUMBNAIL, &format!("{base}/page/0"), "image/jpeg"));
        if let Some(count) = book.pages.filter(|n| *n > 0) {
            links.push(Link::page_stream(
                &format!("{base}/page/{{pageNumber}}"),
                "image/jpeg",
                count,
                None,
            ));
        }
    }
    Entry {
        id: format!("urn:kaeri:book:{}", book.key.replace('/', ":")),
        title: book.title.clone(),
        updated: stamp(book.updated),
        links,
        ..Entry::default()
    }
}

async fn route(path: &str, store: Store) -> Response {
    let segments: Vec<String> =
        path.trim_matches('/').split('/').filter(|s| !s.is_empty()).map(decode).collect();
    let parts: Vec<&str> = segments.iter().map(String::as_str).collect();
    match parts.as_slice() {
        ["opds"] => Response::document(&root(&store)),
        ["opds", "downloads"] => Response::document(&downloads(&store)),
        ["opds", "downloads", work] => Response::document(&acquisition(
            &format!("urn:kaeri:downloads:{work}"),
            &title_of(&store, work),
            &format!("/opds/downloads/{}", encode(work)),
            &download_books(&store, work),
        )),
        ["opds", "folders"] => Response::document(&folders(&store)),
        ["opds", "folders", series] => Response::document(&acquisition(
            &format!("urn:kaeri:folders:{series}"),
            series,
            &format!("/opds/folders/{}", encode(series)),
            &folder_books(&store, series),
        )),
        ["opds", "book", kind, first, rest @ ..] => {
            let (second, tail) = if *kind == "f" {
                (rest.first().copied(), rest.get(1..).unwrap_or(&[]))
            } else {
                (None, rest)
            };
            let Some(book) = find_book(&store, kind, first, second) else {
                return Response::text("404 Not Found", "Not found");
            };
            match tail {
                ["file"] if book.format != "folder" => Response {
                    status: "200 OK",
                    content_type: media_type(&book.format).into(),
                    body: Body::File(book.path),
                    challenge: false,
                },
                ["page", n] => match n.parse::<usize>() {
                    Ok(n) => page(book, n).await,
                    Err(_) => Response::text("404 Not Found", "Not found"),
                },
                _ => Response::text("404 Not Found", "Not found"),
            }
        }
        _ => Response::text("404 Not Found", "Not found"),
    }
}

fn root(store: &Store) -> Document {
    let mut feed = base_feed("urn:kaeri:opds", "Kaeri", FeedKind::Navigation, "/opds");
    let down = store.book_download_works().unwrap_or_default().len();
    let folders = store.local_book_series().unwrap_or_default().len();
    feed.entries.push(section(
        "urn:kaeri:downloads",
        "Downloads",
        "/opds/downloads",
        FeedKind::Navigation,
        Some(down.to_string()),
    ));
    feed.entries.push(section(
        "urn:kaeri:folders",
        "Folders",
        "/opds/folders",
        FeedKind::Navigation,
        Some(folders.to_string()),
    ));
    feed
}

fn downloads(store: &Store) -> Document {
    let mut feed =
        base_feed("urn:kaeri:downloads", "Downloads", FeedKind::Navigation, "/opds/downloads");
    let mut works = store.book_download_works().unwrap_or_default();
    works.sort();
    for work in works {
        if download_books(store, &work).is_empty() {
            continue;
        }
        feed.entries.push(section(
            &format!("urn:kaeri:downloads:{work}"),
            &title_of(store, &work),
            &format!("/opds/downloads/{}", encode(&work)),
            FeedKind::Acquisition,
            None,
        ));
    }
    feed
}

fn folders(store: &Store) -> Document {
    let mut feed =
        base_feed("urn:kaeri:folders", "Folders", FeedKind::Navigation, "/opds/folders");
    for series in store.local_book_series().unwrap_or_default() {
        feed.entries.push(section(
            &format!("urn:kaeri:folders:{}", series.id),
            &series.name,
            &format!("/opds/folders/{}", encode(&series.id)),
            FeedKind::Acquisition,
            None,
        ));
    }
    feed
}

fn acquisition(id: &str, title: &str, href: &str, books: &[Shelved]) -> Document {
    let mut feed = base_feed(id, title, FeedKind::Acquisition, href);
    feed.entries = books.iter().map(book_entry).collect();
    feed
}

/// One page image of a book, for page streaming.
async fn page(book: Shelved, index: usize) -> Response {
    let read = tokio::task::spawn_blocking(move || {
        let Book::Comic(mut comic) = kaeri_books::open(&book.path, &Limits::default()).ok()?
        else {
            return None;
        };
        comic.read_page(index).ok()
    })
    .await
    .ok()
    .flatten();
    match read {
        Some(bytes) => Response {
            status: "200 OK",
            content_type: image_type(&bytes).into(),
            body: Body::Bytes(bytes),
            challenge: false,
        },
        None => Response::text("404 Not Found", "Not found"),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use kaeri_books::{opds::REL_PAGE_STREAM, write::CbzWriter};
    use kaeri_store::book_downloads::BookDownloadAsk;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    const PASSWORD: &str = "made-up-password";

    struct Reply {
        status: u16,
        headers: String,
        body: Vec<u8>,
    }

    async fn get(port: u16, path: &str, password: Option<&str>, method: &str) -> Reply {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let auth = password
            .map(|p| {
                format!("Authorization: Basic {}\r\n", STANDARD.encode(format!("reader:{p}")))
            })
            .unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: feed.example\r\n{auth}Connection: close\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.unwrap();
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&raw[..split]).into_owned();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        Reply { status, headers: head, body: raw[split + 4..].to_vec() }
    }

    fn page(seed: u8) -> Vec<u8> {
        let image =
            image::RgbImage::from_fn(30, 40, |x, y| image::Rgb([seed, x as u8, y as u8]));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    struct Shelf {
        _dir: tempfile::TempDir,
        feed: Feed,
        port: u16,
        _events: tokio::sync::mpsc::UnboundedReceiver<Event>,
    }

    fn shelf() -> Shelf {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("one.cbz");
        let mut w = CbzWriter::new(std::fs::File::create(&path).unwrap(), None).unwrap();
        for n in 0..3 {
            w.add_page(&page(n)).unwrap();
        }
        w.finish().unwrap();
        let store = Store::open_in_memory().unwrap();
        let ask = BookDownloadAsk {
            work: "w-1",
            chapter: "1",
            source_id: "shelf.example",
            version_label: None,
            keep_ready: false,
        };
        store.queue_book_download(&ask, 10).unwrap();
        let id = store.book_downloads(Some("w-1")).unwrap()[0].id;
        store.book_download_done(id, &path.display().to_string(), Some(3), 100, 20).unwrap();
        let (tx, events) = tokio::sync::mpsc::unbounded_channel();
        let feed = Feed::default();
        let config = FeedConfig { on: true, port: None, password: PASSWORD.into() };
        feed.apply(&config, store, &Outbox::from(tx));
        let port = feed.port().expect("the feed listens");
        Shelf { _dir: dir, feed, port, _events: events }
    }

    #[tokio::test]
    async fn the_feed_asks_for_the_password() {
        let s = shelf();
        let none = get(s.port, "/opds", None, "GET").await;
        assert_eq!(none.status, 401);
        assert!(none.headers.contains("WWW-Authenticate: Basic"));
        let wrong = get(s.port, "/opds", Some("not-it"), "GET").await;
        assert_eq!(wrong.status, 401);
        let right = get(s.port, "/opds", Some(PASSWORD), "GET").await;
        assert_eq!(right.status, 200);
    }

    #[tokio::test]
    async fn without_a_password_or_when_off_nothing_listens() {
        let store = Store::open_in_memory().unwrap();
        let (tx, _events) = tokio::sync::mpsc::unbounded_channel();
        let feed = Feed::default();
        let outbox = Outbox::from(tx);
        feed.apply(
            &FeedConfig { on: true, port: None, password: String::new() },
            store.clone(),
            &outbox,
        );
        assert_eq!(feed.port(), None);
        feed.apply(
            &FeedConfig { on: true, port: None, password: PASSWORD.into() },
            store.clone(),
            &outbox,
        );
        assert!(feed.port().is_some());
        feed.apply(
            &FeedConfig { on: false, port: None, password: PASSWORD.into() },
            store,
            &outbox,
        );
        assert_eq!(feed.port(), None);
    }

    #[tokio::test]
    async fn the_documents_parse_with_the_opds_reader() {
        let s = shelf();
        let root = get(s.port, "/opds", Some(PASSWORD), "GET").await;
        let text = String::from_utf8(root.body).unwrap();
        let parsed = kaeri_bookserver::opds::parse(&text, "http://feed.example/opds").unwrap();
        assert!(parsed.entries.len() >= 2, "{} entries", parsed.entries.len());

        let list = get(s.port, "/opds/downloads/w-1", Some(PASSWORD), "GET").await;
        assert!(list.headers.contains("application/atom+xml"));
        let text = String::from_utf8(list.body).unwrap();
        let parsed = kaeri_bookserver::opds::parse(&text, "http://feed.example/opds").unwrap();
        assert_eq!(parsed.entries.len(), 1);
        let entry = &parsed.entries[0];
        assert!(entry.links.iter().any(|l| l.rel == REL_ACQUISITION));
        let stream = entry.links.iter().find(|l| l.rel == REL_PAGE_STREAM).unwrap();
        assert_eq!(stream.pse_count, Some(3));
        assert!(stream.href.contains("{pageNumber}"));
    }

    #[tokio::test]
    async fn the_page_stream_link_serves_the_pages() {
        let s = shelf();
        let list = get(s.port, "/opds/downloads/w-1", Some(PASSWORD), "GET").await;
        let text = String::from_utf8(list.body).unwrap();
        let parsed = kaeri_bookserver::opds::parse(&text, "http://feed.example/opds").unwrap();
        let entry = &parsed.entries[0];
        let stream = entry.links.iter().find(|l| l.rel == REL_PAGE_STREAM).unwrap();
        let path =
            stream.href.trim_start_matches("http://feed.example").replace("{pageNumber}", "1");
        let reply = get(s.port, &path, Some(PASSWORD), "GET").await;
        assert_eq!(reply.status, 200);
        assert!(reply.headers.contains("image/"));
        assert!(!reply.body.is_empty());
        assert!(image::load_from_memory(&reply.body).is_ok());
        let missing = path.replace("/page/1", "/page/9");
        assert_eq!(get(s.port, &missing, Some(PASSWORD), "GET").await.status, 404);
        // The file itself downloads whole.
        let file = entry.links.iter().find(|l| l.rel == REL_ACQUISITION).unwrap();
        let path = file.href.trim_start_matches("http://feed.example");
        let reply = get(s.port, path, Some(PASSWORD), "GET").await;
        assert_eq!(reply.status, 200);
        assert_eq!(&reply.body[..2], b"PK");
        let head = get(s.port, path, Some(PASSWORD), "HEAD").await;
        assert!(head.body.is_empty());
    }

    #[tokio::test]
    async fn a_path_from_the_request_reaches_no_file() {
        let s = shelf();
        for path in ["/opds/book/d/999/file", "/opds/book/d/../../etc/passwd", "/etc/passwd"] {
            assert_eq!(get(s.port, path, Some(PASSWORD), "GET").await.status, 404, "{path}");
        }
        drop(s.feed);
    }
}
