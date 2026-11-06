//! The Kavita client against answers written by hand from Kavita's public API documentation,
//! served by a tiny in-process server.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

use kaeri_bookserver::{Kind, Server, sign_in, source};
use kaeri_core::books::{BookHints, ChapterContent};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

type Seen = Arc<Mutex<Vec<(String, String, String)>>>;

/// Serves canned JSON by path prefix; records (method, path, body).
async fn serve(routes: HashMap<&'static str, &'static str>) -> (String, Seen) {
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
                log.lock().unwrap().push((method.into(), path.into(), body));
                let answer = routes
                    .iter()
                    .filter(|(prefix, _)| path.starts_with(**prefix))
                    .max_by_key(|(prefix, _)| prefix.len())
                    .map(|(_, json)| *json);
                let response = match answer {
                    Some(json) => format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
                        json.len()
                    ),
                    None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
                };
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    (address, seen)
}

fn routes() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        (
            "/api/Account/login",
            r#"{"username":"test","token":"jwt.one","refreshToken":"refresh.one","apiKey":"key-one","email":"","roles":["Login"]}"#,
        ),
        ("/api/Server/server-info-slim", r#"{"kavitaVersion":"0.8.1.0","installId":"x"}"#),
        (
            "/api/Search/search",
            r#"{"series":[{"seriesId":7,"name":"Lantern Keeper","localizedName":"Lantern Keeper","libraryId":1,"format":1}],"collections":[],"readingLists":[],"persons":[],"genres":[],"tags":[],"files":[],"chapters":[]}"#,
        ),
        (
            "/api/Series/metadata",
            r#"{"summary":"A keeper of lanterns.","writers":[{"id":1,"name":"A. Example"}],"genres":[{"id":2,"title":"Fantasy"}]}"#,
        ),
        (
            "/api/Series/series-detail",
            r#"{"specials":[{"id":903,"range":"Extra","title":"","titleName":"Extra","pages":4,"isSpecial":true,"files":[{"format":1}]}],"chapters":[],"volumes":[{"id":11,"name":"1","chapters":[{"id":901,"range":"1-9","titleName":"","title":"","pages":3,"isSpecial":false,"releaseDate":"2025-02-03T00:00:00","files":[{"format":1}]}]},{"id":12,"name":"2","chapters":[{"id":902,"range":"0","titleName":"","title":"","pages":2,"isSpecial":false,"files":[{"format":1}]}]}],"storylineChapters":[]}"#,
        ),
        ("/api/Series/7", r#"{"id":7,"name":"Lantern Keeper","libraryId":1}"#),
        ("/api/Reader/chapter-info", r#"{"pages":3,"fileDimensions":[]}"#),
        ("/api/Reader/mark-multiple", "{}"),
        ("/api/Account", "{}"),
    ])
}

#[tokio::test]
async fn kavita_signs_in_lists_opens_and_writes_progress_back() {
    let (address, seen) = serve(routes()).await;
    let client = reqwest::Client::new();
    let done = sign_in(&client, &address, Some(Kind::Kavita), "test", "test").await.unwrap();
    assert_eq!((done.kind, done.series), (Kind::Kavita, 1));
    assert_eq!(done.version, "0.8.1.0");
    assert!(done.credential.starts_with("kavita:"));

    let source = source(
        &client,
        &Server {
            id: "k",
            name: "Kavita",
            kind: Kind::Kavita,
            address: &address,
            credential: &done.credential,
        },
        std::env::temp_dir().join("kaeri-kavita-test"),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let hints = BookHints::default();

    let hits = source.search("lantern", &hints).await.unwrap();
    assert_eq!((hits[0].id.as_str(), hits[0].title.as_str()), ("7", "Lantern Keeper"));
    assert!(hits[0].cover.as_deref().unwrap().contains("apiKey=key-one"));

    let detail = source.series("7", &hints).await.unwrap();
    assert_eq!(detail.description.as_deref(), Some("A keeper of lanterns."));
    assert_eq!(detail.authors, ["A. Example"]);
    assert_eq!(detail.genres, ["Fantasy"]);

    let chapters = source.chapters("7", &hints).await.unwrap();
    let ids: Vec<&str> = chapters.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["901", "902", "903"]);
    assert_eq!(chapters[0].page_count, Some(3));
    assert!(chapters[0].published.is_some());

    let ChapterContent::Images(pages) = source.content("7", "901").await.unwrap() else {
        panic!("images");
    };
    assert_eq!(pages.len(), 3);
    assert!(pages[2].url.contains("chapterId=901") && pages[2].url.contains("page=2"));

    source.report_progress("7", "901", true).await.unwrap();
    source.report_progress("7", "901", false).await.unwrap();
    let seen = seen.lock().unwrap();
    let read = seen.iter().find(|(_, p, _)| p == "/api/Reader/mark-multiple-read").unwrap();
    assert_eq!(read.0, "POST");
    assert!(read.2.contains("\"chapterIds\":[901]"), "{}", read.2);
    assert!(seen.iter().any(|(_, p, _)| p == "/api/Reader/mark-multiple-unread"));
    // Every call after sign-in carried the JWT; the secret never went in a path except the key.
    assert!(seen.iter().all(|(_, p, _)| !p.contains("jwt.one")));
}
