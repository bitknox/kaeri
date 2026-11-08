//! The Komga client against answers written by hand from Komga's public API documentation,
//! served by a tiny in-process server: sign-in, the chapter list, and progress written back.

mod common;

use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
};

use common::serve;
use kaeri_bookserver::{Kind, Server, sign_in, source};
use kaeri_core::books::BookHints;

fn routes() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        (
            "/api/v1/users/me",
            "X-Auth-Token: tok-one\n\n{\"id\":\"u1\",\"email\":\"reader@books.example\",\"roles\":[\"USER\"]}",
        ),
        ("/actuator/info", r#"{"build":{"version":"1.11.0"}}"#),
        ("/api/v1/series", r#"{"content":[],"totalElements":3}"#),
        (
            "/api/v1/series/s1/books",
            r#"{"content":[{"id":"b1","name":"Lantern Keeper 001","sizeBytes":1000,"media":{"status":"READY","mediaType":"application/zip","pagesCount":3},"metadata":{"title":"","number":"1"}},{"id":"b2","name":"Lantern Keeper 002","sizeBytes":1000,"media":{"status":"READY","mediaType":"application/zip","pagesCount":2},"metadata":{"title":"","number":"2"}}],"totalElements":2}"#,
        ),
        ("/api/v1/books/b1/read-progress", ""),
        ("/api/v1/books/b2/read-progress", ""),
    ])
}

#[tokio::test]
async fn komga_signs_in_lists_and_writes_progress_back() {
    let (address, seen) = serve(routes()).await;
    let client = reqwest::Client::new();
    let done = sign_in(&client, &address, Some(Kind::Komga), "test", "test").await.unwrap();
    assert_eq!((done.kind, done.series), (Kind::Komga, Some(3)));
    assert_eq!(done.version, "1.11.0");

    let keep = tempfile::tempdir().unwrap();
    let source = source(
        &client,
        &Server {
            id: "k",
            name: "Komga",
            kind: Kind::Komga,
            address: &address,
            credential: &done.credential,
        },
        keep.path().to_path_buf(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let chapters = source.chapters("s1", &BookHints::default()).await.unwrap();
    let ids: Vec<&str> = chapters.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["b1", "b2"]);

    source.report_progress("s1", "b1", true).await.unwrap();
    source.report_progress("s1", "b2", false).await.unwrap();
    // Taking back what the server never had is not a failure; marking it read is.
    source.report_progress("s1", "b9", false).await.unwrap();
    assert!(source.report_progress("s1", "b9", true).await.is_err());

    let seen = seen.lock().unwrap();
    let read =
        seen.iter().find(|(m, p, _)| m == "PATCH" && p == "/api/v1/books/b1/read-progress");
    let read = read.expect("the read went to the book's progress");
    assert!(read.2.contains("\"completed\":true"), "{}", read.2);
    assert!(
        seen.iter().any(|(m, p, _)| m == "DELETE" && p == "/api/v1/books/b2/read-progress")
    );
    // After sign-in the token went as a header, never in a path.
    assert!(seen.iter().all(|(_, p, _)| !p.contains("tok-one")));
}

/// The next chapter's preview asks for a book's pages without its file: a comic names its
/// pages, and a PDF, which only its download would open, names none and is not downloaded.
#[tokio::test]
async fn komga_names_a_comic_s_pages_and_never_downloads_a_file_for_them() {
    let mut routes = routes();
    routes.extend([
        (
            "/api/v1/books/b1",
            r#"{"id":"b1","name":"Lantern Keeper 001","sizeBytes":1000,"media":{"status":"READY","mediaType":"application/zip","pagesCount":3},"metadata":{"title":"","number":"1"}}"#,
        ),
        (
            "/api/v1/books/b1/pages",
            r#"[{"number":1,"width":800,"height":1200},{"number":2},{"number":3}]"#,
        ),
        (
            "/api/v1/books/b4",
            r#"{"id":"b4","name":"Lantern Keeper 004","sizeBytes":1000,"media":{"status":"READY","mediaType":"application/pdf","pagesCount":12},"metadata":{"title":"","number":"4"}}"#,
        ),
    ]);
    let (address, seen) = serve(routes).await;
    let client = reqwest::Client::new();
    let done = sign_in(&client, &address, Some(Kind::Komga), "test", "test").await.unwrap();
    let keep = tempfile::tempdir().unwrap();
    let source = source(
        &client,
        &Server {
            id: "k",
            name: "Komga",
            kind: Kind::Komga,
            address: &address,
            credential: &done.credential,
        },
        keep.path().to_path_buf(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();

    let pages = source.chapter_pages("s1", "b1").await.unwrap().expect("a comic names pages");
    assert_eq!(pages.len(), 3);
    assert!(pages[0].url.ends_with("/api/v1/books/b1/pages/1"), "{}", pages[0].url);
    assert_eq!((pages[0].width, pages[0].height), (Some(800), Some(1200)));

    assert_eq!(source.chapter_pages("s1", "b4").await.unwrap(), None);
    let seen = seen.lock().unwrap();
    assert!(seen.iter().any(|(_, p, _)| p == "/api/v1/books/b4"));
    assert!(seen.iter().all(|(_, p, _)| !p.ends_with("/file")), "no file was downloaded");
}
