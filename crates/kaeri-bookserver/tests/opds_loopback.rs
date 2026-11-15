//! Book servers on loopback.
//!
//! R7E-1: no server kind's sign-in, whatever header carries it, follows a 301, 302, 307 or 308
//! to another host, port or scheme, on a page, a file or any other request.
//!
//! R6E-1: a book server's sign-in goes only to its own origin (scheme, host and port), never
//! to another host its feed names, and never along a redirect that leaves the origin. Two fake
//! servers: the catalogue on one port, the pages and files it links to on another.
//!
//! R6E-4: a catalogue whose search link is an OpenSearch description is searched through the
//! template the description holds.
//!
//! R7E-12: a catalogue with neither a feed of all series nor a search, as a Kaeri feed before
//! its search, is searched through the series its sections list.

use std::sync::{Arc, Mutex, atomic::AtomicBool};

use kaeri_bookserver::{Auth, Http, Kind, Server, basic, source};
use kaeri_core::books::{BookHints, ChapterContent};
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
    api_key: Option<String>,
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
                    // Bytes that can't start a request, such as a TLS hello, are answered at
                    // once: waiting on them for a blank line would hold the client to its
                    // timeout, and the request would be seen only after it gave up.
                    let unreadable = n > 0 && !buf[0].is_ascii_uppercase();
                    if read == 0 || unreadable || buf[..n].windows(4).any(|w| w == b"\r\n\r\n")
                    {
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
                    api_key: header("x-api-key"),
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

fn redirect(status: &str, to: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .into_bytes()
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
    asked
        .iter()
        .filter(|a| a.authorization.is_some() || a.token.is_some() || a.api_key.is_some())
        .collect()
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_catalogue_searched_through_an_opensearch_description_finds_its_books() {
    let (listener, address) = listener().await;
    let root = r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
 <id>urn:x:root</id><title>Made-up Books</title><updated>2026-01-01T00:00:00Z</updated>
 <link rel="search" href="/opds/osd" type="application/opensearchdescription+xml"/>
 <entry><id>urn:x:lib</id><title>Library</title><updated>2026-01-01T00:00:00Z</updated>
  <link rel="subsection" href="/opds/lib" type="application/atom+xml;profile=opds-catalog;kind=navigation"/></entry>
</feed>"#;
    let osd = r#"<?xml version="1.0"?>
<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/">
 <ShortName>Made-up Books</ShortName>
 <Url type="application/atom+xml;profile=opds-catalog;kind=acquisition" template="/opds/search?q={searchTerms}"/>
</OpenSearchDescription>"#;
    let found = r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
 <id>urn:x:found</id><title>Search</title><updated>2026-01-01T00:00:00Z</updated>
 <entry><id>urn:x:b1</id><title>Lantern Test Ferry v01</title><updated>2026-01-02T00:00:00Z</updated>
  <category term="Lantern Test Ferry" label="Lantern Test Ferry"/>
  <link rel="http://opds-spec.org/acquisition" href="/files/b1.epub" type="application/epub+zip"/></entry>
 <entry><id>urn:x:b2</id><title>Lantern Test Ferry v02</title><updated>2026-01-03T00:00:00Z</updated>
  <category term="Lantern Test Ferry" label="Lantern Test Ferry"/>
  <link rel="http://opds-spec.org/acquisition" href="/files/b2.epub" type="application/epub+zip"/></entry>
</feed>"#;
    let seen = serve(
        listener,
        vec![
            ("/opds/".into(), ok(ATOM, root.as_bytes())),
            ("/opds/osd".into(), ok("application/opensearchdescription+xml", osd.as_bytes())),
            ("/opds/search".into(), ok(ATOM, found.as_bytes())),
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
            address: &format!("{address}/opds"),
            credential: &credential,
        },
        keep.path().to_owned(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let hints = BookHints::default();
    let hits = source.search("Lantern", &hints).await.expect("the search answers");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].title, "Lantern Test Ferry");
    let chapters = source.chapters(&hits[0].id, &hints).await.unwrap();
    assert_eq!(chapters.len(), 2);
    let asked: Vec<String> = seen.lock().unwrap().iter().map(|a| a.path.clone()).collect();
    assert!(asked.iter().any(|p| p == "/opds/search?q=Lantern"), "{asked:?}");
}

/// R7E-12. A catalogue with neither a feed of all series nor a search, whose sections are not
/// named in English, finds its series by name through its sections, whatever the case,
/// accents or letter width.
#[tokio::test]
async fn a_catalogue_without_a_search_is_searched_through_its_sections() {
    let (listener, address) = listener().await;
    let section = |id: &str, title: &str, href: &str, kind: &str| {
        format!(
            r#" <entry><id>{id}</id><title>{title}</title><updated>2026-01-01T00:00:00Z</updated>
  <link rel="subsection" href="{href}" type="application/atom+xml;profile=opds-catalog;kind={kind}"/></entry>
"#
        )
    };
    let feed = |id: &str, entries: &str| {
        format!(
            r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
 <id>{id}</id><title>Kaeri</title><updated>2026-01-01T00:00:00Z</updated>
{entries}</feed>"#
        )
    };
    let root = feed(
        "urn:x:root",
        &(section("urn:x:d", "Téléchargements", "/opds/downloads", "navigation")
            + &section("urn:x:f", "Dossiers", "/opds/folders", "navigation")),
    );
    let downloads =
        feed("urn:x:d", &section("urn:x:d1", "Café Lantern", "/opds/d/1", "acquisition"));
    let folders = feed(
        "urn:x:f",
        &(section("urn:x:f1", "Ferry Notes", "/opds/f/1", "acquisition")
            + &section("urn:x:f2", "灯台の本", "/opds/f/2", "acquisition")),
    );
    let books = feed(
        "urn:x:books",
        r#" <entry><id>urn:x:b1</id><title>Ferry Notes v01</title><updated>2026-01-02T00:00:00Z</updated>
  <link rel="http://opds-spec.org/acquisition" href="/files/b1.epub" type="application/epub+zip"/></entry>
"#,
    );
    serve(
        listener,
        vec![
            ("/opds".into(), ok(ATOM, root.as_bytes())),
            ("/opds/downloads".into(), ok(ATOM, downloads.as_bytes())),
            ("/opds/folders".into(), ok(ATOM, folders.as_bytes())),
            ("/opds/f/1".into(), ok(ATOM, books.as_bytes())),
        ],
    )
    .await;
    let keep = tempfile::tempdir().unwrap();
    let credential = format!("basic:{}", basic("reader", "made-up-pass"));
    let source = source(
        &client(),
        &Server {
            id: "a",
            name: "Kaeri",
            kind: Kind::Opds,
            address: &format!("{address}/opds"),
            credential: &credential,
        },
        keep.path().to_owned(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let hints = BookHints::default();
    for (term, want) in [
        ("ferry", "Ferry Notes"),
        ("ＦＥＲＲＹ", "Ferry Notes"),
        ("cafe", "Café Lantern"),
        ("灯台", "灯台の本"),
    ] {
        let hits = source.search(term, &hints).await.expect("the search answers");
        let titles: Vec<&str> = hits.iter().map(|h| h.title.as_str()).collect();
        assert_eq!(titles, [want], "{term}");
    }
    assert!(source.search("harbour", &hints).await.unwrap().is_empty());
    let hits = source.search("Ferry", &hints).await.unwrap();
    let chapters = source.chapters(&hits[0].id, &hints).await.unwrap();
    assert_eq!(chapters.len(), 1, "the hit opens to its books");
}

/// Every credentialed server kind, with what its sign-in was kept as and the secret that must
/// stay home.
fn kinds() -> Vec<(Kind, String, &'static str)> {
    vec![
        (Kind::Komga, "token:made-up-token".into(), "made-up-token"),
        (Kind::Komga, format!("basic:{}", basic("reader", "made-up-pass")), "Basic "),
        (
            Kind::Kavita,
            r#"kavita:{"token":"made-up-jwt","refreshToken":"r","apiKey":"k"}"#.into(),
            "made-up-jwt",
        ),
        (Kind::Opds, format!("basic:{}", basic("reader", "made-up-pass")), "Basic "),
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_server_kind_s_sign_in_follows_a_redirect_to_another_host_port_or_scheme() {
    let (a_listener, a) = listener().await;
    let (b_listener, b) = listener().await;
    let port_a = a.rsplit(':').next().unwrap().to_owned();
    let port_b = b.rsplit(':').next().unwrap().to_owned();
    // Another port, another host name for the other server, and https on this very host and
    // port, which the plain server here can only see as bytes it can't read.
    let targets = [
        ("port", format!("{b}/landing")),
        ("host", format!("http://localhost:{port_b}/landing")),
        ("scheme", format!("https://127.0.0.1:{port_a}/landing")),
    ];
    let statuses = [
        "301 Moved Permanently",
        "302 Found",
        "307 Temporary Redirect",
        "308 Permanent Redirect",
    ];
    let mut routes = Vec::new();
    for status in statuses {
        for (name, to) in &targets {
            let code = &status[..3];
            routes.push((format!("/p/{code}/{name}"), redirect(status, to)));
        }
    }
    let home = serve(a_listener, routes).await;
    let away =
        serve(b_listener, vec![("/landing".into(), ok("image/png", b"made-up page"))]).await;

    for (kind, credential, secret) in kinds() {
        let keep = tempfile::tempdir().unwrap();
        let source = source(
            &client(),
            &Server {
                id: "a",
                name: "Made-up Books",
                kind,
                address: &a,
                credential: &credential,
            },
            keep.path().to_owned(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        for status in statuses {
            for (name, _) in &targets {
                home.lock().unwrap().clear();
                away.lock().unwrap().clear();
                let code = &status[..3];
                // A page as a server's reference names it, with a sign-in header of its own
                // the server's fetch must not carry along either.
                let mut page = kaeri_core::books::PageRef::new(format!("{a}/p/{code}/{name}"));
                page.headers = vec![
                    ("X-Auth-Token".into(), "made-up-token".into()),
                    ("X-Api-Key".into(), "made-up-token".into()),
                ];
                let fetched = source.fetch_page(&page).await;
                let home = home.lock().unwrap().clone();
                let away = away.lock().unwrap().clone();
                let what = format!("{kind:?} {status} to another {name}");
                assert!(!home.is_empty() && signed(&home[..1]).len() == 1, "{what}: {home:?}");
                if *name == "scheme" {
                    // The https hop can't be answered here; whatever reached the server after
                    // the first request is no readable sign-in.
                    assert!(fetched.is_err(), "{what}");
                    assert!(signed(&home[1..]).is_empty(), "{what}: {home:?}");
                } else {
                    assert_eq!(fetched.unwrap(), b"made-up page", "{what}");
                    assert_eq!(away.len(), 1, "{what}: {away:?}");
                    assert!(signed(&away).is_empty(), "{what} carried {secret}: {away:?}");
                }
            }
        }
    }
}
