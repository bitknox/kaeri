//! The book registry, folders and servers through the commands of the Sources screen.
//!
//! The order and off switch test makes its own data. The folder and server tests use the test
//! library outside the repo and skip themselves without it:
//!
//! ```sh
//! KAERI_TEST_LIBRARY=~/kaeri-test-library/books KAERI_TEST_SERVER=127.0.0.1:25600 \
//!   cargo test -p kaeri-engine --test read_sources
//! ```
//! with `python3 -I server.py --port 25600` running in the library's folder.

use std::time::Duration;

use kaeri_core::{books::WorkId, config::Paths, media::Medium};
use kaeri_engine::{book_plugins::BookPlugins, read_sources::BookSources};
use kaeri_model::{
    book_sources::{BookSourceCommand, BookSourceEvent, BookSourcesView, FolderRef},
    event::Event,
    stores::Secret,
};
use kaeri_store::{Store, sync::kinds};
use kaeri_track::TokenStore;
use tokio::sync::mpsc;

fn sources(dir: &std::path::Path) -> (BookSources, Store) {
    let store = Store::open_in_memory().unwrap();
    let paths = Paths {
        config_file: dir.join("config.toml"),
        data_dir: dir.join("data"),
        cache_dir: dir.join("cache"),
    };
    let tokens = TokenStore::new(dir.join("tokens"));
    (BookSources::new(store.clone(), tokens, reqwest::Client::new(), &paths), store)
}

fn run(sources: &BookSources, command: BookSourceCommand) -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    kaeri_engine::read_sources::command(
        sources,
        &BookPlugins::default(),
        |_| None,
        command,
        1_000,
        &tx.into(),
    );
    rx
}

async fn next_view(rx: &mut mpsc::UnboundedReceiver<Event>) -> BookSourcesView {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(20), rx.recv())
            .await
            .expect("an event")
            .expect("open");
        if let Event::BookSources(BookSourceEvent::View(view)) = event {
            return view;
        }
    }
}

#[tokio::test]
async fn the_order_and_the_off_switches_are_kept_and_follow_another_device() {
    let dir = tempfile::tempdir().unwrap();
    let (sources, store) = sources(dir.path());
    let mut rx = run(&sources, BookSourceCommand::SetOff { id: "folders".into(), off: true });
    assert!(sources.registry.is_off("folders"));
    drop(next_view(&mut rx).await);
    // A fresh holder over the same store reads both back.
    let tokens = TokenStore::new(dir.path().join("tokens"));
    let paths = Paths {
        config_file: dir.path().join("c.toml"),
        data_dir: dir.path().join("d"),
        cache_dir: dir.path().join("cache"),
    };
    let again = BookSources::new(store, tokens, reqwest::Client::new(), &paths);
    assert!(again.registry.is_off("folders"));
    // Another device switches it on and sets an order.
    again.apply_effect(kinds::BOOK_SOURCE_OFF, "folders", None);
    assert!(!again.registry.is_off("folders"));
    again.apply_effect(kinds::BOOK_SOURCE_ORDER, "", Some(r#"["b","folders","a"]"#));
    again.apply_effect(kinds::BOOK_SOURCE_OFF, "b", Some("1"));
    assert!(again.registry.is_off("b"));
}

#[tokio::test]
async fn a_folder_is_scanned_listed_searched_and_its_chapters_opened() {
    let Some(library) = std::env::var_os("KAERI_TEST_LIBRARY") else { return };
    let dir = tempfile::tempdir().unwrap();
    let (sources, _) = sources(dir.path());
    let mut rx = run(
        &sources,
        BookSourceCommand::AddFolder {
            folder: FolderRef {
                path: Some(library.to_string_lossy().into_owned()),
                ..FolderRef::default()
            },
        },
    );
    // Views arrive while it scans; the last one has the series.
    let mut view = next_view(&mut rx).await;
    for _ in 0..200 {
        if !view.groups.is_empty()
            && view
                .folders
                .iter()
                .all(|f| matches!(f.scan, kaeri_model::book_sources::BookFolderScan::Done))
        {
            break;
        }
        view = next_view(&mut rx).await;
    }
    assert!(view.groups.len() >= 8, "{} series", view.groups.len());
    assert!(view.groups.iter().any(|g| g.name == "Moonlit Harbor"));
    assert_eq!(view.order.first().map(String::as_str), Some("folders"));

    let tiles = sources.search("moonlit", Some(Medium::Manga), 1_000).await;
    assert_eq!(tiles.len(), 1, "{tiles:?}");
    let work: WorkId = tiles[0].work.clone();
    let (edition, volumes) = sources.chapters(&work, None, 1_000).await.unwrap();
    assert_eq!(edition.unwrap().source_id, "folders");
    let chapters: Vec<_> = volumes.iter().flat_map(|v| &v.chapters).collect();
    assert_eq!(chapters.len(), 4);
    let opened = sources.open(&work, &chapters[0].id, None, 1_000).await.unwrap();
    assert!(matches!(opened.content, kaeri_core::books::ChapterContent::Book(_)));
}

#[tokio::test]
async fn a_server_is_checked_signed_in_to_searched_and_told_what_was_read() {
    let Some(address) = std::env::var("KAERI_TEST_SERVER").ok().filter(|a| !a.is_empty())
    else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let (sources, store) = sources(dir.path());
    let mut rx = run(&sources, BookSourceCommand::CheckServer { address: address.clone() });
    let checked = loop {
        match rx.recv().await.unwrap() {
            Event::BookSources(event @ BookSourceEvent::ServerChecked { .. }) => break event,
            Event::BookSources(BookSourceEvent::ServerFailed { failure, .. }) => {
                panic!("{failure:?}")
            }
            _ => {}
        }
    };
    let BookSourceEvent::ServerChecked { address: found, .. } = checked else { unreachable!() };
    let mut rx = run(
        &sources,
        BookSourceCommand::SignIn {
            address: found,
            user: "test".into(),
            password: Secret("test".into()),
        },
    );
    let id = loop {
        match rx.recv().await.unwrap() {
            Event::BookSources(BookSourceEvent::ServerAdded { id, series, .. }) => {
                assert!(series > 0);
                break id;
            }
            Event::BookSources(BookSourceEvent::ServerFailed { failure, .. }) => {
                panic!("{failure:?}")
            }
            _ => {}
        }
    };
    assert!(sources.registry.get(&id).is_some());
    assert_eq!(store.book_servers().unwrap().len(), 1);

    let tiles = sources.search("", None, 1_000).await;
    assert!(!tiles.is_empty());
    let work = tiles[0].work.clone();
    let (_, volumes) = sources.chapters(&work, None, 1_000).await.unwrap();
    let chapter = volumes[0].chapters[0].id.clone();
    let mut opened = sources.open(&work, &chapter, None, 1_000).await.unwrap();
    if let kaeri_core::books::ChapterContent::Images(_) = &opened.content {
        assert!(!sources.page(&mut opened, 0, 1_000).await.unwrap().is_empty());
    }
    sources.report(&work, vec![chapter], true, 1_000);
    tokio::time::sleep(Duration::from_millis(500)).await;

    let _ = run(&sources, BookSourceCommand::RemoveServer { id: id.clone() });
    assert!(sources.registry.get(&id).is_none());
    assert!(store.book_servers().unwrap().is_empty());
}
