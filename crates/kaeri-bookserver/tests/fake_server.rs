//! The clients against the fake server in `~/kaeri-test-library/server.py`.
//!
//! Run the server (`python3 -I server.py --port 25600`, user and password `test`), then
//! `KAERI_TEST_SERVER=127.0.0.1:25600 cargo test -p kaeri-bookserver --test fake_server`.
//! Without the variable every test returns at once.

use std::sync::{Arc, atomic::AtomicBool};

use kaeri_bookserver::{Kind, Server, check, sign_in, source};
use kaeri_core::{
    books::{BookHints, ChapterContent},
    traits::BookSource,
};

fn address() -> Option<String> {
    std::env::var("KAERI_TEST_SERVER").ok().filter(|a| !a.is_empty())
}

async fn signed(typed: &str, kind: Option<Kind>) -> Arc<dyn BookSource> {
    let client = reqwest::Client::new();
    let found = check(&client, typed).await.expect("the address answers");
    let kind = kind.or(found.kind);
    let done = sign_in(&client, &found.address, kind, "test", "test").await.expect("sign in");
    assert!(done.series > 0, "the server lists series");
    let dir = std::env::temp_dir().join("kaeri-bookserver-test");
    source(
        &client,
        &Server {
            id: "test",
            name: "Test",
            kind: done.kind,
            address: &found.address,
            credential: &done.credential,
        },
        dir,
        Arc::new(AtomicBool::new(false)),
    )
    .expect("a source")
}

#[tokio::test]
async fn a_wrong_password_is_unauthorised_not_unreachable() {
    let Some(address) = address() else { return };
    let client = reqwest::Client::new();
    let found = check(&client, &address).await.unwrap();
    let error = sign_in(&client, &found.address, found.kind, "test", "nope").await.unwrap_err();
    assert!(error.needs_sign_in(), "{error:?}");
}

#[tokio::test]
async fn the_rest_api_lists_reads_and_writes_progress_back() {
    let Some(address) = address() else { return };
    let source = signed(&address, Some(Kind::Komga)).await;
    let hits = source.search("", &BookHints::default()).await.unwrap();
    assert!(!hits.is_empty());
    let hit = &hits[0];
    let detail = source.series(&hit.id, &BookHints::default()).await.unwrap();
    assert!(!detail.title.is_empty());
    let chapters = source.chapters(&hit.id, &BookHints::default()).await.unwrap();
    assert!(!chapters.is_empty());
    let first = &chapters[0];
    match source.content(&hit.id, &first.id).await.unwrap() {
        ChapterContent::Images(pages) => {
            assert!(!pages.is_empty());
            let bytes = source.fetch_page(&pages[0]).await.unwrap();
            assert!(!bytes.is_empty());
        }
        ChapterContent::Book(file) => {
            assert!(file.path.exists());
            let _ = std::fs::remove_file(&file.path);
        }
        ChapterContent::Text(text) => assert!(!text.is_empty()),
    }
    source.report_progress(&hit.id, &first.id, true).await.unwrap();
    source.report_progress(&hit.id, &first.id, false).await.unwrap();
    // Taking back what was never read is not an error.
    source.report_progress(&hit.id, &first.id, false).await.unwrap();
}

#[tokio::test]
async fn the_catalogue_lists_series_and_streams_pages() {
    let Some(address) = address() else { return };
    let source = signed(&format!("http://{address}/opds/v1.2/catalog"), Some(Kind::Opds)).await;
    let hits = source.search("", &BookHints::default()).await.unwrap();
    assert!(!hits.is_empty());
    let filtered =
        source.search(&hits[0].title.to_lowercase()[..3], &BookHints::default()).await.unwrap();
    assert!(!filtered.is_empty());
    let mut streamed = false;
    for hit in &hits {
        let chapters = source.chapters(&hit.id, &BookHints::default()).await.unwrap();
        let Some(first) = chapters.first() else { continue };
        match source.content(&hit.id, &first.id).await.unwrap() {
            ChapterContent::Images(pages) => {
                let bytes = source.fetch_page(&pages[0]).await.unwrap();
                assert!(!bytes.is_empty());
                streamed = true;
            }
            ChapterContent::Book(file) => {
                assert!(file.path.exists());
                let _ = std::fs::remove_file(&file.path);
            }
            ChapterContent::Text(_) => {}
        }
    }
    assert!(streamed, "at least one book streams its pages");
}

#[tokio::test]
async fn a_root_that_wants_a_sign_in_still_signs_in_and_is_found_to_be_komga() {
    let Some(address) = address() else { return };
    let client = reqwest::Client::new();
    let found = check(&client, &address).await.unwrap();
    let done = sign_in(&client, &found.address, found.kind, "test", "test").await.unwrap();
    assert_eq!(done.kind, Kind::Komga);
}
