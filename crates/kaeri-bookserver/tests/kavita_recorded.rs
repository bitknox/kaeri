//! The Kavita client against answers written by hand from Kavita's public API documentation,
//! served by a tiny in-process server.

mod common;

use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
};

use kaeri_bookserver::{Kind, Server, sign_in, source};
use kaeri_core::books::{BookHints, ChapterContent};

use common::serve;

fn routes() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        (
            "/api/Account/login",
            r#"{"username":"test","token":"jwt.one","refreshToken":"refresh.one","apiKey":"key-one","email":"","roles":["Login"]}"#,
        ),
        ("/api/Server/server-info-slim", r#"{"kavitaVersion":"0.8.1.0","installId":"x"}"#),
        (
            "/api/Series/all-v2",
            "Pagination: {\"currentPage\":1,\"itemsPerPage\":1,\"totalItems\":212,\"totalPages\":212}\n\n[{\"id\":7,\"name\":\"Lantern Keeper\"}]",
        ),
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
            r#"{"specials":[{"id":903,"range":"Extra","title":"","titleName":"Extra","pages":4,"isSpecial":true,"files":[{"format":1}]}],"chapters":[],"volumes":[{"id":11,"name":"1","chapters":[{"id":901,"range":"1-9","titleName":"","title":"","pages":3,"isSpecial":false,"releaseDate":"2025-02-03T00:00:00","files":[{"format":1}]}]},{"id":12,"name":"2","chapters":[{"id":902,"range":"0","titleName":"","title":"","pages":2,"isSpecial":false,"releaseDate":"0001-01-01T00:00:00","files":[{"format":1}]}]}],"storylineChapters":[]}"#,
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
    // The count is the series list's total, not what a search returns.
    assert_eq!((done.kind, done.series), (Kind::Kavita, Some(212)));
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
    // Kavita keeps no direction per series, and its library's type is not taken for one.
    assert_eq!(detail.reading_hint, None);

    let chapters = source.chapters("7", &hints).await.unwrap();
    let ids: Vec<&str> = chapters.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["901", "902", "903"]);
    assert_eq!(chapters[0].page_count, Some(3));
    assert!(chapters[0].published.is_some());
    // Kavita keeps a release date as midnight with no offset: that day, wherever the person is.
    assert_eq!(chapters[0].published_day.map(|d| d.to_string()).as_deref(), Some("2025-02-03"));
    // Kavita marks a chapter with no date by the least date it holds: no date.
    assert_eq!((chapters[1].published, chapters[1].published_day), (None, None));

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
