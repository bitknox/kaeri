//! R6E-1: a book server's sign-in goes only to its own origin (scheme, host and port), never
//! to another host its feed names, and never along a redirect that leaves the origin. Two fake
//! servers on loopback: the catalogue on one port, the pages and files it links to on another.

use std::sync::{Arc, Mutex, atomic::AtomicBool};

use kaeri_bookserver::{Auth, Http, Kind, Server, basic, source};
use kaeri_core::{
    books::{BookHints, ChapterContent},
    traits::BookSource,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// What a server saw: the path and the sign-in headers of each request.
#[derive(Debug, Clone)]
struct Asked {
    path: String,
    authorization: Option<String>,
    token: Option<String>,
}

type Seen = Arc<Mutex<Vec<Asked>>>;

/// A server's routes: a path prefix and the whole answer, status line and headers included.
/// The longest prefix wins; anything else is a 404.
async fn serve(listener: TcpListener, routes: Vec<(String, Vec<u8>)>) -> Seen {
    let seen: Seen = Arc::default();
    let log = seen.clone();
    let routes = Arc::new(routes);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let routes = routes.clone();
            let log = log.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                let mut n = 0;
                while n < buf.len() {
                    let read = stream.read(&mut buf[n..]).await.unwrap_or(0);
                    n += read;
                    if read == 0 || buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = text.split(' ').nth(1).unwrap_or_default().to_owned();
                let header = |name: &str| {
                    text.lines().find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_owned())
                    })
                };
                log.lock().unwrap().push(Asked {
                    path: path.clone(),
                    authorization: header("authorization"),
                    token: header("x-auth-token"),
                });
                let answer = routes
                    .iter()
                    .filter(|(prefix, _)| path.starts_with(prefix.as_str()))
                    .max_by_key(|(prefix, _)| prefix.len())
                    .map_or_else(
                        || {
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                                .to_vec()
                        },
                        |(_, answer)| answer.clone(),
                    );
                let _ = stream.write_all(&answer).await;
            });
        }
    });
    seen
}

fn ok(content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut answer = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    answer.extend_from_slice(body);
    answer
}

fn moved(to: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .into_bytes()
}

const ATOM: &str = "application/atom+xml;profile=opds-catalog";

fn root_feed() -> String {
    r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
 <id>urn:x:root</id><title>Made-up Books</title><updated>2026-01-01T00:00:00Z</updated>
 <entry><id>urn:x:series</id><title>All series</title><updated>2026-01-01T00:00:00Z</updated>
  <link rel="subsection" href="/opds/series" type="application/atom+xml;profile=opds-catalog;kind=navigation"/></entry>
</feed>"#
        .to_owned()
}

fn series_feed() -> String {
    r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
 <id>urn:x:all</id><title>All series</title><updated>2026-01-01T00:00:00Z</updated>
 <entry><id>urn:x:s1</id><title>Lantern Test Ferry</title><updated>2026-01-01T00:00:00Z</updated>
  <link rel="subsection" href="/opds/series/1" type="application/atom+xml;profile=opds-catalog;kind=acquisition"/></entry>
</feed>"#
        .to_owned()
}

/// A book read page by page from `elsewhere`, and a book downloaded whole from there.
fn books_feed(elsewhere: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:pse="http://vaemendis.net/opds-pse/ns">
 <id>urn:x:s1</id><title>Lantern Test Ferry</title><updated>2026-01-01T00:00:00Z</updated>
 <entry><id>urn:x:b1</id><title>Lantern Test Ferry v01</title><updated>2026-01-02T00:00:00Z</updated>
  <link rel="http://opds-spec.org/acquisition" href="{elsewhere}/files/b1.cbz" type="application/vnd.comicbook+zip"/>
  <link rel="http://vaemendis.net/opds-pse/stream" href="{elsewhere}/pages/b1/{{pageNumber}}" type="image/png" pse:count="2"/></entry>
 <entry><id>urn:x:b2</id><title>Lantern Test Ferry v02</title><updated>2026-01-03T00:00:00Z</updated>
  <link rel="http://opds-spec.org/acquisition" href="{elsewhere}/files/b2.epub" type="application/epub+zip"/></entry>
</feed>"#
    )
}

async fn listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    (listener, address)
}

/// The engine's sources client follows no redirects; the server's [`Http`] follows them.
fn client() -> reqwest::Client {
    reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap()
}

fn signed(asked: &[Asked]) -> Vec<&Asked> {
    asked.iter().filter(|a| a.authorization.is_some() || a.token.is_some()).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_catalogue_s_pages_and_files_on_another_host_are_asked_without_the_sign_in() {
    let (a_listener, a) = listener().await;
    let (b_listener, b) = listener().await;
    let catalogue = serve(
        a_listener,
        vec![
            ("/opds/".into(), ok(ATOM, root_feed().as_bytes())),
            ("/opds/series".into(), ok(ATOM, series_feed().as_bytes())),
            ("/opds/series/1".into(), ok(ATOM, books_feed(&b).as_bytes())),
        ],
    )
    .await;
    let elsewhere = serve(
        b_listener,
        vec![
            ("/pages/".into(), ok("image/png", b"made-up page")),
            ("/files/".into(), ok("application/epub+zip", b"made-up book")),
        ],
    )
    .await;

    let keep = tempfile::tempdir().unwrap();
    let credential = format!("basic:{}", basic("reader", "made-up-pass"));
    let source = source(
        &client(),
        &Server {
            id: "a",
            name: "Made-up Books",
            kind: Kind::Opds,
            address: &format!("{a}/opds"),
            credential: &credential,
        },
        keep.path().to_owned(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let hints = BookHints::default();
    let hits = source.search("", &hints).await.unwrap();
    let series = &hits[0].id;
    let pages = source.chapter_pages(series, "urn:x:b1").await.unwrap().unwrap();
    assert_eq!(pages.len(), 2);
    for page in &pages {
        assert!(page.url.starts_with(&b), "{}", page.url);
        assert!(page.headers.is_empty(), "a page elsewhere carries {:?}", page.headers);
        assert_eq!(source.fetch_page(page).await.unwrap(), b"made-up page");
    }
    let ChapterContent::Book(file) = source.content(series, "urn:x:b2").await.unwrap() else {
        panic!("an EPUB is downloaded whole");
    };
    assert_eq!(std::fs::read(&file.path).unwrap(), b"made-up book");

    let elsewhere = elsewhere.lock().unwrap().clone();
    assert_eq!(elsewhere.len(), 3, "{elsewhere:?}");
    assert!(signed(&elsewhere).is_empty(), "the sign-in went to another host: {elsewhere:?}");
    // The catalogue itself was asked signed in, every time.
    let catalogue = catalogue.lock().unwrap().clone();
    assert!(!catalogue.is_empty());
    assert_eq!(signed(&catalogue).len(), catalogue.len(), "{catalogue:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_redirect_to_another_origin_drops_the_sign_in_and_one_back_home_keeps_it() {
    let (a_listener, a) = listener().await;
    let (b_listener, b) = listener().await;
    let home = serve(
        a_listener,
        vec![
            ("/away".into(), moved(&format!("{b}/landing"))),
            ("/around".into(), moved("/settled")),
            ("/settled".into(), ok("text/plain", b"home")),
        ],
    )
    .await;
    let away = serve(
        b_listener,
        vec![
            ("/landing".into(), ok("text/plain", b"elsewhere")),
            ("/back".into(), moved(&format!("{a}/settled"))),
        ],
    )
    .await;

    for auth in [
        Auth::Basic(basic("reader", "made-up-pass")),
        Auth::Bearer("made-up-token".into()),
        Auth::Header("X-Auth-Token", "made-up-token".into()),
    ] {
        home.lock().unwrap().clear();
        away.lock().unwrap().clear();
        let http = Http::new(&client(), &a, auth.clone());
        assert_eq!(http.text("away").await.unwrap(), "elsewhere");
        assert_eq!(http.text("around").await.unwrap(), "home");
        assert_eq!(http.text(&format!("{b}/back")).await.unwrap(), "home");

        let home = home.lock().unwrap().clone();
        let away = away.lock().unwrap().clone();
        assert_eq!(away.len(), 2, "{away:?}");
        assert!(signed(&away).is_empty(), "{auth:?} left the origin: {away:?}");
        // /away, /around, /settled, and /settled again after the hop back from the other host.
        assert_eq!(home.len(), 4, "{home:?}");
        assert_eq!(signed(&home).len(), 4, "{auth:?} was not kept at home: {home:?}");
    }
}
