//! What a server holds for an account: its libraries and their series, a Kavita OPDS link's key
//! sign-in, and a source that asks only the libraries chosen. Answers are written by hand from
//! each server's public API documentation and served by a tiny in-process server.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

use kaeri_bookserver::{
    Kind, Server, ServerError, count_series, kavita_link, libraries, sign_in, sign_in_with_key,
    source_with,
};
use kaeri_core::books::BookHints;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

type Seen = Arc<Mutex<Vec<(String, String, String)>>>;

/// One canned answer: a status, header lines and a body.
#[derive(Clone)]
struct Answer {
    status: u16,
    headers: &'static str,
    body: &'static str,
}

fn ok(body: &'static str) -> Answer {
    Answer { status: 200, headers: "", body }
}

/// Serves canned answers by the longest matching path prefix, and records each request as
/// (method, path, body). A route keyed `METHOD path` answers only that method.
async fn serve(routes: HashMap<&'static str, Answer>) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let log = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let routes = routes.clone();
            let log = log.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let mut first = text.lines().next().unwrap_or_default().split(' ');
                let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
                let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
                log.lock().unwrap().push((method.into(), path.into(), body.clone()));
                let answer = routes
                    .iter()
                    .filter(|(key, _)| match key.split_once(' ') {
                        Some((m, prefix)) => m == method && path.starts_with(prefix),
                        None => path.starts_with(**key),
                    })
                    .max_by_key(|(key, _)| key.len())
                    .map(|(_, answer)| answer.clone());
                let response = match answer {
                    Some(Answer { status, headers, body }) => {
                        let headers: String =
                            headers.lines().map(|h| format!("{h}\r\n")).collect();
                        format!(
                            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                    }
                    None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .into(),
                };
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    (address, seen)
}

const KEY: &str = "0f1e2d3c-4b5a-6978-8a9b-acbdcedf1234";

fn kavita() -> HashMap<&'static str, Answer> {
    HashMap::from([
        (
            "POST /api/Plugin/authenticate",
            ok(
                r#"{"username":"kim","token":"jwt.key","refreshToken":"refresh.key","apiKey":"0f1e2d3c-4b5a-6978-8a9b-acbdcedf1234"}"#,
            ),
        ),
        ("/api/Server/server-info-slim", ok(r#"{"kavitaVersion":"0.8.4.0"}"#)),
        (
            "/api/Series/all-v2",
            Answer {
                status: 200,
                headers: "Pagination: {\"currentPage\":1,\"itemsPerPage\":1,\"totalItems\":212,\"totalPages\":212}",
                body: "[]",
            },
        ),
        (
            "/api/Library/libraries",
            ok(r#"[{"id":1,"name":"Manga","type":0},{"id":2,"name":"Comics","type":1}]"#),
        ),
        (
            "/api/Search/search",
            ok(
                r#"{"series":[{"seriesId":7,"name":"Lantern Keeper","libraryId":1},{"seriesId":8,"name":"Paper Harbour","libraryId":2}]}"#,
            ),
        ),
    ])
}

#[tokio::test]
async fn a_kavita_opds_link_signs_in_with_its_key_and_lists_the_libraries() {
    let (address, seen) = serve(kavita()).await;
    let link = kavita_link(&format!("{address}/api/opds/{KEY}")).expect("a Kavita link");
    assert_eq!(link.root, address);
    let client = reqwest::Client::new();
    let done = sign_in_with_key(&client, &link).await.expect("the key signs in");
    assert_eq!(done.kind, Kind::Kavita);
    assert_eq!(done.series, Some(212));
    assert!(done.credential.contains("jwt.key"));
    assert!(done.credential.contains(KEY), "the key is kept with the sign-in");
    {
        let seen = seen.lock().unwrap();
        let auth =
            seen.iter().find(|(_, p, _)| p.starts_with("/api/Plugin/authenticate")).unwrap();
        assert_eq!(auth.0, "POST");
        assert!(auth.1.contains(&format!("apiKey={KEY}")));
    }

    let found = libraries(&client, &address, Kind::Kavita, &done.credential).await.unwrap();
    let names: Vec<_> = found.iter().map(|l| (l.id.as_str(), l.name.as_str())).collect();
    assert_eq!(names, [("1", "Manga"), ("2", "Comics")]);
    assert!(found.iter().all(|l| l.series == Some(212)));
    // Each library was counted with a filter on it.
    let seen = seen.lock().unwrap();
    assert!(seen.iter().any(|(_, p, body)| p.starts_with("/api/Series/all-v2")
        && body.contains("\"field\":19")
        && body.contains("\"value\":\"2\"")));
}

#[tokio::test]
async fn a_kavita_key_it_does_not_know_is_refused_not_unreachable() {
    let mut routes = kavita();
    routes
        .insert("POST /api/Plugin/authenticate", Answer { status: 401, headers: "", body: "" });
    let (address, _) = serve(routes).await;
    let link = kavita_link(&format!("{address}/api/opds/{KEY}")).unwrap();
    let error = sign_in_with_key(&reqwest::Client::new(), &link).await.unwrap_err();
    assert_eq!(error, ServerError::Unauthorized);
}

#[tokio::test]
async fn a_kavita_source_asks_only_the_libraries_chosen() {
    let (address, _) = serve(kavita()).await;
    let client = reqwest::Client::new();
    let link = kavita_link(&format!("{address}/api/opds/{KEY}")).unwrap();
    let done = sign_in_with_key(&client, &link).await.unwrap();
    let server = Server {
        id: "srv",
        name: "Kavita",
        kind: Kind::Kavita,
        address: &address,
        credential: &done.credential,
    };
    let dir = tempfile::tempdir().unwrap();
    let all = source_with(
        &client,
        &server,
        &[],
        dir.path().to_owned(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(all.search("", &BookHints::default()).await.unwrap().len(), 2);
    let comics = source_with(
        &client,
        &server,
        &["2".to_owned()],
        dir.path().to_owned(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let hits = comics.search("", &BookHints::default()).await.unwrap();
    assert_eq!(hits.iter().map(|h| h.title.as_str()).collect::<Vec<_>>(), ["Paper Harbour"]);
}

fn komga(total: &'static str) -> HashMap<&'static str, Answer> {
    HashMap::from([
        ("/api/v1/users/me", ok(r#"{"id":"u1","email":"kim@home.example","roles":["USER"]}"#)),
        ("/actuator/info", ok(r#"{"build":{"version":"1.11.0"}}"#)),
        ("/api/v1/series", ok(total)),
        (
            "/api/v1/libraries",
            ok(r#"[{"id":"0A","name":"Manga"},{"id":"0B","name":"Comics"}]"#),
        ),
    ])
}

#[tokio::test]
async fn komga_lists_its_libraries_with_their_series_and_counts_again() {
    let (address, seen) = serve(komga(r#"{"content":[],"totalElements":168}"#)).await;
    let client = reqwest::Client::new();
    let done = sign_in(&client, &address, Some(Kind::Komga), "kim", "pw").await.unwrap();
    assert_eq!(done.series, Some(168));
    let found = libraries(&client, &address, Kind::Komga, &done.credential).await.unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[1].name, "Comics");
    assert_eq!(found[1].series, Some(168));
    assert!(
        seen.lock()
            .unwrap()
            .iter()
            .any(|(_, p, _)| p.starts_with("/api/v1/series?size=1&library_id=0B"))
    );
    let again = count_series(&client, &address, Kind::Komga, &done.credential).await.unwrap();
    assert_eq!(again, Some(168));
    // An OPDS catalogue has no libraries to tell apart.
    assert!(libraries(&client, &address, Kind::Opds, "basic:eA==").await.unwrap().is_empty());
}

#[tokio::test]
async fn an_empty_komga_counts_none() {
    let (address, _) = serve(komga(r#"{"content":[],"totalElements":0}"#)).await;
    let client = reqwest::Client::new();
    let done = sign_in(&client, &address, Some(Kind::Komga), "kim", "pw").await.unwrap();
    assert_eq!(done.series, Some(0));
}
