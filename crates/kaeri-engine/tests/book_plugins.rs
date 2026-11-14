//! Book source plugins load beside show sources, and only when reading is available.
//!
//! Skips itself when the reference plugins are not built, like the conformance suite:
//!
//! ```sh
//! cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-rust/Cargo.toml
//! cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-books/Cargo.toml
//! ```

use std::path::{Path, PathBuf};

use kaeri_core::{
    config::{Config, Paths},
    media::Medium,
    traits::BookSource,
};
use kaeri_engine::{book_plugins::BookPlugins, sources};
use kaeri_net::HttpClient;
use kaeri_providers::ProviderRegistry;

fn built(plugin: &str, file: &str) -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../plugins/{plugin}/target/wasm32-wasip2/release/{file}"));
    if path.exists() {
        Some(path)
    } else if std::env::var_os("KAERI_REQUIRE_PLUGINS").is_some() {
        panic!("{} is not built, and KAERI_REQUIRE_PLUGINS is set.", path.display());
    } else {
        eprintln!("skipping: {} is not built.", path.display());
        None
    }
}

/// A machine whose plugin directory holds the show and the book reference plugins.
fn machine(root: &Path) -> Option<Paths> {
    let shows = built("example-rust", "kaeri_example_plugin.wasm")?;
    let books = built("example-books", "kaeri_example_books_plugin.wasm")?;
    let paths = Paths {
        config_file: root.join("config").join("config.toml"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
    };
    let dir = paths.plugin_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(shows, dir.join("example-rust.wasm")).unwrap();
    std::fs::copy(books, dir.join("example-books.wasm")).unwrap();
    Some(paths)
}

async fn load(paths: &Paths, books: Option<BookPlugins>) -> ProviderRegistry {
    let config = Config::default();
    let http = HttpClient::offline(&config.network).unwrap();
    let registry = ProviderRegistry::new(Vec::new());
    let note =
        sources::spawn_plugin_load(registry.clone(), config, http, paths.clone(), books).await;
    assert!(note.unwrap().is_none(), "nothing to say about these plugins");
    registry
}

#[tokio::test]
async fn without_reading_a_book_source_in_the_directory_is_left_alone() {
    let root = tempfile::tempdir().unwrap();
    let Some(paths) = machine(root.path()) else { return };
    let registry = load(&paths, None).await;
    assert!(registry.get("example-rust").is_some());
    assert!(registry.get("example-books").is_none(), "never in the show chain");
}

#[tokio::test]
async fn with_reading_book_sources_load_beside_the_chain() {
    let root = tempfile::tempdir().unwrap();
    let Some(paths) = machine(root.path()) else { return };
    let books = BookPlugins::default();
    let registry = load(&paths, Some(books.clone())).await;
    assert!(registry.get("example-rust").is_some());
    assert!(registry.get("example-books").is_none(), "never in the show chain");
    assert_eq!(books.ids(), ["example-books"]);

    let source = books.get("example-books").unwrap();
    assert_eq!(source.manifest().media, [Medium::Manga, Medium::Novel]);
    let rows = books.rows(|_| None);
    assert_eq!(rows[0].name, "Example (books)");
    assert_eq!(rows[0].languages, ["en"]);
    // A real call through the sandbox, which needs no network for this series.
    let chapters =
        source.chapters("example:lantern-cartographer", &Default::default()).await.unwrap();
    assert_eq!(chapters.len(), 5);
}
