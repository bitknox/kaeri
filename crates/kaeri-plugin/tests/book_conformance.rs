//! The book source conformance suite: the second plugin world, `kaeri:books`, against real
//! compiled components.
//!
//! | Component | Job |
//! |---|---|
//! | `example-books` | The reference book source: a manga and a novel, on `books.example`. Built for `book-plugin`, so the host fetches every page. |
//! | `test-hostile-books` | Attacks the host: spins, allocates, reaches for undeclared hosts, hands over pages no reader should take. Built for `book-plugin-with-pages`. |
//!
//! It also answers a question about the ABI itself: whether a record in `source` can grow a
//! field without making a different world. It cannot. See [`a_record_that_grows_a_field_is_a_different_world`].
//!
//! As in `conformance.rs`, a component that is not built is skipped, and with
//! `KAERI_REQUIRE_PLUGINS` set a missing one fails the test.
//!
//! ```sh
//! cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-books/Cargo.toml
//! cargo build --release --target wasm32-wasip2 --manifest-path plugins/test-hostile-books/Cargo.toml
//! cargo test -p kaeri-plugin --test book_conformance
//! ```

use std::{path::PathBuf, time::Duration};

use kaeri_plugin::{
    AnyPlugin, PluginError, PluginHost, PluginKind, WasmBookSource,
    books::{Content, MAX_PAGE_BYTES, SourceError, TitleHints},
    sandbox::Limits,
};

fn plugins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

fn built(path: PathBuf, how: &str) -> Option<PathBuf> {
    if path.exists() {
        Some(path)
    } else if std::env::var_os("KAERI_REQUIRE_PLUGINS").is_some() {
        panic!("{} is not built, and KAERI_REQUIRE_PLUGINS is set.\n  {how}", path.display());
    } else {
        eprintln!("skipping: {} is not built.\n  {how}", path.display());
        None
    }
}

fn example() -> Option<PathBuf> {
    built(
        plugins_dir()
            .join("example-books/target/wasm32-wasip2/release/kaeri_example_books_plugin.wasm"),
        "cargo build --release --target wasm32-wasip2 \
         --manifest-path plugins/example-books/Cargo.toml",
    )
}

fn hostile() -> Option<PathBuf> {
    built(
        plugins_dir().join(
            "test-hostile-books/target/wasm32-wasip2/release/kaeri_hostile_books_plugin.wasm",
        ),
        "cargo build --release --target wasm32-wasip2 \
         --manifest-path plugins/test-hostile-books/Cargo.toml",
    )
}

fn show_source() -> Option<PathBuf> {
    built(
        plugins_dir()
            .join("example-rust/target/wasm32-wasip2/release/kaeri_example_plugin.wasm"),
        "cargo build --release --target wasm32-wasip2 \
         --manifest-path plugins/example-rust/Cargo.toml",
    )
}

fn host() -> PluginHost {
    PluginHost::new(Limits::default(), None).unwrap()
}

fn nothing() -> TitleHints {
    TitleHints {
        anilist_id: None,
        mal_id: None,
        romaji: None,
        english: None,
        native: None,
        synonyms: Vec::new(),
        medium: None,
    }
}

const LANTERN: &str = "example:lantern-cartographer";
const SALTGLASS: &str = "example:saltglass-letters";

// ── the kind comes from the plugin ───────────────────────────────────────────────────────────

#[tokio::test]
async fn a_book_component_loads_as_a_book_source_and_describes_itself() {
    let Some(path) = example() else { return };
    let plugin = host().load_book(&path).await.expect("load");
    let manifest = plugin.manifest();
    assert_eq!(manifest.id, "example-books");
    assert_eq!(manifest.display_name, "Example (books)");
    assert!(!manifest.version.is_empty());
    assert_eq!(manifest.allowed_hosts, ["books.example", "pages.example"]);
    assert_eq!(manifest.media, ["manga", "novel"]);
    assert_eq!(manifest.languages, ["en"]);
    // Again, with no capabilities, as at load time.
    assert_eq!(plugin.describe().await.expect("describe").id, "example-books");

    let any = host().load_any(&path).await.expect("load any");
    assert_eq!(any.kind(), PluginKind::Books);
    assert_eq!(any.id(), "example-books");
}

#[tokio::test]
async fn each_loader_refuses_the_other_kind_before_running_it() {
    let (Some(books), Some(shows)) = (example(), show_source()) else { return };
    match host().load(&books).await {
        Err(PluginError::OtherKind { kind, wanted, .. }) => {
            assert_eq!((kind, wanted), (PluginKind::Books, PluginKind::Shows));
        }
        other => panic!("expected OtherKind, got {:?}", other.map(|_| ())),
    }
    match host().load_book(&shows).await {
        Err(PluginError::OtherKind { kind, wanted, .. }) => {
            assert_eq!((kind, wanted), (PluginKind::Shows, PluginKind::Books));
        }
        other => panic!("expected OtherKind, got {:?}", other.map(|_| ())),
    }
    assert!(matches!(host().load_any(&shows).await, Ok(AnyPlugin::Shows(_))));
}

#[tokio::test]
async fn a_directory_of_both_kinds_loads_books_only_when_asked() {
    let (Some(books), Some(shows), Some(bad)) = (example(), show_source(), hostile()) else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(&shows, dir.path().join("a-shows.wasm")).unwrap();
    std::fs::copy(&books, dir.path().join("b-books.wasm")).unwrap();
    std::fs::copy(&bad, dir.path().join("c-hostile-books.wasm")).unwrap();

    // Reading off: book sources are skipped quietly, neither loaded nor called outdated.
    let off = kaeri_plugin::load_providers_reporting(&host(), dir.path()).await;
    assert_eq!(off.providers.len(), 1);
    assert!(off.books.is_empty() && off.outdated.is_empty());

    // Reading on: both kinds, each in its own list.
    let on = kaeri_plugin::load_plugins_reporting(&host(), dir.path(), true).await;
    assert_eq!(on.providers.len(), 1);
    let ids: Vec<_> = on
        .books
        .iter()
        .map(|b| kaeri_core::traits::BookSource::manifest(b).id.clone())
        .collect();
    assert_eq!(ids, ["example-books", "test-hostile-books"]);
    assert!(on.outdated.is_empty());
}

#[tokio::test]
async fn a_book_component_for_another_major_is_named_for_what_it_is() {
    use wasm_encoder::{
        Component, ComponentImportSection, ComponentTypeRef, ComponentTypeSection, InstanceType,
    };
    let component = |imports: &[&str]| {
        let mut types = ComponentTypeSection::new();
        types.instance(&InstanceType::new());
        let mut section = ComponentImportSection::new();
        for import in imports {
            section.import(*import, ComponentTypeRef::Instance(0));
        }
        let mut component = Component::new();
        component.section(&types);
        component.section(&section);
        component.finish()
    };
    let dir = tempfile::tempdir().unwrap();
    let load = |bytes: Vec<u8>| {
        let path = dir.path().join("plugin.wasm");
        std::fs::write(&path, bytes).unwrap();
        async move { host().load_any(&path).await.map(|_| ()) }
    };

    match load(component(&["kaeri:books/host@2.0.0"])).await {
        Err(PluginError::NeedsNewerKaeri { world, .. }) => {
            assert_eq!(world, "kaeri:books@2.0.0")
        }
        other => panic!("expected NeedsNewerKaeri, got {other:?}"),
    }
    // No book source was built for a major before 1.
    match load(component(&["kaeri:books/host@0.2.0"])).await {
        Err(PluginError::Load { message, .. }) => {
            assert!(message.contains("kaeri:books@1"), "{message}")
        }
        other => panic!("expected a load error, got {other:?}"),
    }
    // A plugin is one kind or the other.
    match load(component(&["kaeri:books/host@1.0.0", "kaeri:provider/host@3.0.0"])).await {
        Err(PluginError::Load { message, .. }) => {
            assert!(message.contains("both"), "{message}")
        }
        other => panic!("expected a load error, got {other:?}"),
    }
}

// ── records cross the ABI ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_series_and_its_chapters_cross_with_every_field() {
    let Some(path) = example() else { return };
    let plugin = host().load_book(&path).await.expect("load");

    let series = plugin.series(LANTERN, &nothing()).await.expect("no trap").expect("series");
    assert_eq!(series.title, "The Lantern Cartographer");
    assert_eq!(series.reading_hint.as_deref(), Some("rtl"));
    assert_eq!(series.status.as_deref(), Some("releasing"));
    assert_eq!(series.genres, ["Adventure", "Mystery"]);

    let chapters =
        plugin.list_chapters(LANTERN, &nothing()).await.expect("no trap").expect("chapters");
    let labels: Vec<_> = chapters.iter().map(|c| c.number.as_str()).collect();
    assert_eq!(labels, ["1", "2", "2", "10.5", "Extra"]);
    assert_eq!(chapters[1].version_label.as_deref(), Some("Group A"));
    assert_eq!(chapters[3].volume.as_deref(), Some("2"));
    assert_eq!(chapters[3].page_count, Some(8));
    assert_eq!(chapters[4].volume, None);
    assert_eq!(chapters[0].published.as_deref(), Some("2026-01-02"));
}

#[tokio::test]
async fn chapter_content_is_pages_with_their_headers_or_text() {
    let Some(path) = example() else { return };
    let plugin = host().load_book(&path).await.expect("load");

    let pages = plugin.chapter_content(LANTERN, "lantern-1").await.expect("no trap");
    let Ok(Content::Images(pages)) = pages else { panic!("expected pages, got {pages:?}") };
    assert_eq!(pages.len(), 3);
    assert_eq!(pages[0].url, "https://pages.example/lantern/lantern-1/001.jpg");
    assert_eq!(pages[0].width, Some(1_200));
    assert!(pages[0].headers.iter().any(|(name, _)| name == "referer"));

    let text = plugin.chapter_content(SALTGLASS, "saltglass-2").await.expect("no trap");
    let Ok(Content::Text(text)) = text else { panic!("expected text, got {text:?}") };
    assert!(text.starts_with("<h2>Low Tide</h2>"), "{text}");
}

#[tokio::test]
async fn hints_cross_and_a_search_without_a_client_fails_cleanly() {
    let Some(path) = example() else { return };
    let plugin = host().load_book(&path).await.expect("load");

    // A catalogue keyed by AniList id answers from the hint, with no network.
    let hints = TitleHints { anilist_id: Some(990_001), ..nothing() };
    let hits = plugin.search("anything", &hints).await.expect("no trap").expect("hits");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, LANTERN);
    assert_eq!(hits[0].year, Some(2026));

    // Without one it fetches, and with no client lent the fetch fails as a transport error.
    let result = plugin.search("lantern", &nothing()).await.expect("no trap");
    assert!(matches!(result, Err(SourceError::Blocked(_))), "got {result:?}");
}

#[tokio::test]
async fn not_found_survives_the_abi() {
    let Some(path) = example() else { return };
    let plugin = host().load_book(&path).await.expect("load");
    let result = plugin.list_chapters("not-an-example-id", &nothing()).await.expect("no trap");
    assert!(matches!(result, Err(SourceError::NotFound)), "got {result:?}");
}

#[tokio::test]
async fn a_book_plugin_is_an_ordinary_book_source() {
    use kaeri_core::{
        books::{BookHints, BookSourceKind, ChapterContent, ReadingHint},
        media::Medium,
        traits::BookSource,
    };
    let Some(path) = example() else { return };
    let source = WasmBookSource::new(host().load_book(&path).await.expect("load"));

    let manifest = source.manifest();
    assert_eq!(manifest.kind, BookSourceKind::Plugin);
    assert_eq!(manifest.media, [Medium::Manga, Medium::Novel]);
    assert_eq!(manifest.name, "Example (books)");

    let detail = source.series(LANTERN, &BookHints::default()).await.expect("series");
    assert_eq!(detail.reading_hint, Some(ReadingHint::RightToLeft));

    let chapters = source.chapters(LANTERN, &BookHints::default()).await.expect("chapters");
    let numbers: Vec<_> =
        chapters.iter().map(|c| c.number.map(|n| n.to_string())).collect::<Vec<_>>();
    assert_eq!(
        numbers,
        [Some("1".into()), Some("2".into()), Some("2".into()), Some("10.5".into()), None]
    );
    assert_eq!(chapters[4].number_raw, "Extra");
    assert_eq!(chapters[0].published, Some(1_767_312_000));
    // Read marks key on the work and the number, not on the version.
    assert_eq!(chapters[1].key(false), chapters[2].key(false));

    let content = source.content(LANTERN, "lantern-extra").await.expect("content");
    let ChapterContent::Images(pages) = content else { panic!("expected pages") };
    assert_eq!(pages.len(), 4);
    // A plugin built for `book-plugin` makes no pages: the host fetches every one.
    assert!(pages.iter().all(|p| p.made.is_none()), "{pages:?}");

    let missing = source.chapters("nope", &BookHints::default()).await.unwrap_err();
    assert!(!missing.should_failover());
    assert!(source.health_check().await.is_ok());

    // A page on a declared host goes to the client; with none lent, it fails as transport.
    let page = source.fetch_page(&pages[0]).await.unwrap_err();
    assert!(!matches!(page, kaeri_core::error::ProviderError::Blocked(_)), "{page:?}");
}

#[tokio::test]
async fn a_plugin_built_for_book_plugin_is_never_asked_for_pages() {
    let Some(path) = example() else { return };
    let plugin = host().load_book(&path).await.expect("load");
    assert!(!plugin.makes_pages());
    let url = "https://pages.example/lantern/lantern-1/001.jpg";
    let page = plugin.page_bytes(LANTERN, "lantern-1", url).await.expect("no call to trap");
    assert!(matches!(page, Ok(None)), "got {page:?}");
}

#[tokio::test]
async fn a_plugin_with_pages_makes_the_pages_it_names_and_leaves_the_rest() {
    use kaeri_core::{books::ChapterContent, traits::BookSource};
    let Some(path) = hostile() else { return };
    let plugin = host().load_book(&path).await.expect("load");
    assert!(plugin.makes_pages());

    // A page it makes comes back whole, as a real PNG that passes the host's check.
    let page = plugin.page_bytes("x", "1", &made("honest")).await.expect("no trap");
    let page = page.expect("a page").expect("made by the plugin");
    assert!(page.starts_with(b"\x89PNG"), "{:?}", &page[..8.min(page.len())]);
    // A page it leaves to the host is `None`.
    let left = plugin.page_bytes("x", "1", "https://allowed.example/page-2.jpg").await;
    assert!(matches!(left, Ok(Ok(None))), "got {left:?}");

    // As a book source, each page carries its chapter, and the source's own page fetch asks
    // the plugin first.
    let source = WasmBookSource::new(plugin);
    let ChapterContent::Images(pages) = source.content("x", "1").await.expect("content") else {
        panic!("expected pages")
    };
    let chapter = pages[0].made.as_ref().expect("a page the plugin may make");
    assert_eq!((chapter.series.as_str(), chapter.chapter.as_str()), ("x", "1"));
    let honest = &hostile_pages(&source).await["honest"];
    let bytes = BookSource::fetch_page(&source, honest).await.expect("the plugin's page");
    assert_eq!(bytes, page);
}

#[tokio::test]
async fn settings_reach_a_book_plugin_named_after_its_id() {
    let Some(built) = example() else { return };
    let dir = tempfile::tempdir().unwrap();
    let host = || {
        host().with_plugin_settings(
            [(
                "example-books".to_string(),
                [("group".to_string(), "Group B".to_string())].into_iter().collect(),
            )]
            .into_iter()
            .collect(),
        )
    };
    let versions = |plugin: kaeri_plugin::LoadedBookPlugin| async move {
        plugin
            .list_chapters(LANTERN, &nothing())
            .await
            .expect("no trap")
            .expect("chapters")
            .into_iter()
            .filter_map(|c| c.version_label)
            .collect::<Vec<_>>()
    };

    let named = dir.path().join("example-books.wasm");
    std::fs::copy(&built, &named).unwrap();
    assert_eq!(versions(host().load_book(&named).await.unwrap()).await, ["Group B"]);

    let impostor = dir.path().join("weather.wasm");
    std::fs::copy(&built, &impostor).unwrap();
    assert_eq!(
        versions(host().load_book(&impostor).await.unwrap()).await,
        ["Group A", "Group B"],
        "settings must not follow an id a file merely claims"
    );
}

// ── the adversary ────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_spinning_book_plugin_is_stopped_by_the_deadline_and_can_be_called_again() {
    let Some(path) = hostile() else { return };
    let host = PluginHost::new(
        Limits { deadline: Duration::from_millis(500), ..Limits::default() },
        None,
    )
    .unwrap();
    let plugin = host.load_book(&path).await.expect("load");

    let started = std::time::Instant::now();
    let error = plugin.search("anything", &nothing()).await.expect_err("stopped");
    assert!(matches!(error, PluginError::Deadline { .. }), "got {error:?}");
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(plugin.describe().await.expect("describe again").id, "test-hostile-books");
}

#[tokio::test]
async fn a_hoarding_book_plugin_is_stopped_by_the_memory_ceiling() {
    let Some(path) = hostile() else { return };
    let host = PluginHost::new(
        Limits {
            memory_bytes: 24 * 1024 * 1024,
            deadline: Duration::from_secs(5),
            ..Limits::default()
        },
        None,
    )
    .unwrap();
    let plugin = host.load_book(&path).await.expect("load");
    match plugin.series("x", &nothing()).await {
        Err(PluginError::Trap { .. } | PluginError::Deadline { .. }) => {}
        Ok(Err(SourceError::Other(message))) if message.contains("ESCAPED") => {
            panic!("the memory ceiling did not hold: {message}")
        }
        other => panic!("expected the guest to be stopped, got {other:?}"),
    }
}

#[tokio::test]
async fn a_book_plugin_reaching_an_undeclared_host_is_denied() {
    let Some(path) = hostile() else { return };
    let plugin = host().load_book(&path).await.expect("load");
    assert_eq!(plugin.manifest().allowed_hosts, ["allowed.example"]);

    let result = plugin.list_chapters("x", &nothing()).await.expect("no trap");
    let Err(SourceError::Other(message)) = result else {
        panic!("expected the guest to report the denial, got {result:?}");
    };
    assert!(message.starts_with("denied:"), "{message}");
    assert!(message.contains("exfiltrate.example"), "{message}");
}

#[tokio::test]
async fn pages_on_an_undeclared_host_are_never_fetched() {
    use kaeri_core::{books::ChapterContent, error::ProviderError, traits::BookSource};
    let Some(path) = hostile() else { return };
    let source = WasmBookSource::new(host().load_book(&path).await.expect("load"));

    // The setting it was never granted reads as none, or this would be an ESCAPED error.
    let content = source.content("x", "1").await.expect("content");
    let ChapterContent::Images(pages) = content else { panic!("expected pages") };

    // The page host is checked against the plugin's own list, as any fetch is.
    let denied = source.fetch_page(&pages[0]).await.unwrap_err();
    let ProviderError::Blocked(reason) = denied else {
        panic!("expected a denial: {denied:?}")
    };
    assert!(reason.contains("exfiltrate.example"), "{reason}");
    // A declared host with a header the host sets itself is refused too.
    let denied = source.fetch_page(&pages[1]).await.unwrap_err();
    let ProviderError::Blocked(reason) = denied else {
        panic!("expected a denial: {denied:?}")
    };
    assert!(reason.contains("host"), "{reason}");
}

/// The address of the hostile plugin's page `name`, one it makes itself.
fn made(name: &str) -> String {
    format!("https://allowed.example/made/{name}.png")
}

/// The hostile plugin's pages that it makes itself, as a book source sees them, by name.
async fn hostile_pages(
    source: &WasmBookSource,
) -> std::collections::HashMap<String, kaeri_core::books::PageRef> {
    use kaeri_core::{books::ChapterContent, traits::BookSource};
    let ChapterContent::Images(pages) = source.content("x", "1").await.expect("content") else {
        panic!("expected pages")
    };
    pages
        .into_iter()
        .filter_map(|p| {
            let name =
                p.url.strip_prefix("https://allowed.example/made/")?.strip_suffix(".png")?;
            Some((name.to_owned(), p))
        })
        .collect()
}

#[tokio::test]
async fn a_page_over_the_size_ceiling_is_refused_as_a_parse_error() {
    use kaeri_core::{error::ProviderError, traits::BookSource};
    let Some(path) = hostile() else { return };
    let plugin = host().load_book(&path).await.expect("load");
    let result = plugin.page_bytes("x", "1", &made("too-large")).await.expect("no trap");
    let Err(SourceError::Parse(reason)) = result else {
        panic!("expected the page to be refused, got {:?}", result.map(|b| b.map(|b| b.len())))
    };
    assert!(reason.contains(&format!("{} bytes", MAX_PAGE_BYTES + 1)), "{reason}");

    // Through the book source it is the plugin's parse error, which fails over and counts
    // against its health, never a status that would ask for fresh addresses.
    let source = WasmBookSource::new(plugin);
    let page = &hostile_pages(&source).await["too-large"];
    let error = BookSource::fetch_page(&source, page).await.unwrap_err();
    assert!(!error.expired() && !error.transient(), "{error:?}");
    let error = source.fetch_page(page).await.unwrap_err();
    assert!(matches!(error, ProviderError::Parse(_)) && error.should_failover(), "{error:?}");
}

#[tokio::test]
async fn a_page_that_is_no_image_is_refused_by_its_first_bytes() {
    let Some(path) = hostile() else { return };
    let plugin = host().load_book(&path).await.expect("load");
    let result = plugin.page_bytes("x", "1", &made("not-an-image")).await.expect("no trap");
    let Err(SourceError::Parse(reason)) = result else {
        panic!("expected the page to be refused, got {result:?}")
    };
    assert!(reason.contains("not an image"), "{reason}");
}

#[tokio::test]
async fn a_page_whose_header_claims_too_many_pixels_is_refused_before_decoding() {
    let Some(path) = hostile() else { return };
    let plugin = host().load_book(&path).await.expect("load");
    let result = plugin.page_bytes("x", "1", &made("too-many-pixels")).await.expect("no trap");
    let Err(SourceError::Parse(reason)) = result else {
        panic!("expected the page to be refused, got {result:?}")
    };
    assert!(reason.contains("20000x20000"), "{reason}");
    assert!(reason.contains("pixel limit"), "{reason}");
}

#[tokio::test]
async fn a_page_that_never_comes_is_stopped_by_the_deadline() {
    use kaeri_core::traits::BookSource;
    let Some(path) = hostile() else { return };
    let host = PluginHost::new(
        Limits { deadline: Duration::from_millis(500), ..Limits::default() },
        None,
    )
    .unwrap();
    let plugin = host.load_book(&path).await.expect("load");
    let started = std::time::Instant::now();
    let error = plugin.page_bytes("x", "1", &made("spin")).await.expect_err("stopped");
    assert!(matches!(error, PluginError::Deadline { .. }), "got {error:?}");
    assert!(started.elapsed() < Duration::from_secs(10));

    // A stopped page fails the page, not the reader: the plugin's parse error, not transient.
    let source = WasmBookSource::new(plugin);
    let page = &hostile_pages(&source).await["spin"];
    let error = BookSource::fetch_page(&source, page).await.unwrap_err();
    assert!(!error.transient(), "a stopped plugin is not asked again at once: {error:?}");
}

#[tokio::test]
async fn a_page_that_hoards_memory_is_stopped_by_the_memory_ceiling() {
    let Some(path) = hostile() else { return };
    let host = PluginHost::new(
        Limits {
            memory_bytes: 24 * 1024 * 1024,
            deadline: Duration::from_secs(5),
            ..Limits::default()
        },
        None,
    )
    .unwrap();
    let plugin = host.load_book(&path).await.expect("load");
    match plugin.page_bytes("x", "1", &made("hoard")).await {
        Err(PluginError::Trap { .. } | PluginError::Deadline { .. }) => {}
        Ok(Err(SourceError::Other(message))) if message.contains("ESCAPED") => {
            panic!("the memory ceiling did not hold: {message}")
        }
        other => panic!(
            "expected the guest to be stopped, got {:?}",
            other.map(|r| r.map(|b| b.map(|b| b.len())))
        ),
    }
}

#[tokio::test]
async fn open_sets_a_host_does_not_know_are_ignored_rather_than_refused() {
    use kaeri_core::{media::Medium, traits::BookSource};
    let Some(path) = hostile() else { return };
    let source = WasmBookSource::new(host().load_book(&path).await.expect("load"));
    assert_eq!(source.manifest().media, [Medium::Manga]);
    assert_eq!(source.manifest().capabilities, ["a-capability-no-host-knows"]);
    assert!(!source.manifest().has("torrents"));
}

// ── what changes the world ───────────────────────────────────────────────────────────────────
//
// The anime WIT says records prefer `option` fields over new shapes. That keeps a contract
// readable, but does it let a record grow without a new major? The component model checks an
// export's types when a component is instantiated, and these tests ask it directly: the example
// book plugin, built against `kaeri:books@1.0.0`, loaded by a host whose copy of the WIT differs
// from the plugin's in one place.
//
// Each host world here is the real one cut down to the functions it checks. A host may look up
// fewer exports than a component has, so the control, cut down the same way with nothing
// changed, has to load and run: that is what shows the one change is what fails the others.

mod worlds {
    /// The test host's side of `host`: it lends nothing.
    macro_rules! lend_nothing {
        ($world:ident) => {
            use self::$world::kaeri::books::host::{HostError, HttpRequest, HttpResponse};

            impl $world::kaeri::books::host::Host for State {
                async fn fetch(
                    &mut self,
                    _: HttpRequest,
                ) -> wasmtime::Result<Result<HttpResponse, HostError>> {
                    Ok(Err(HostError::Transport("none".into())))
                }
                async fn fetch_many(
                    &mut self,
                    requests: Vec<HttpRequest>,
                ) -> wasmtime::Result<Vec<Result<HttpResponse, HostError>>> {
                    Ok(requests
                        .iter()
                        .map(|_| Err(HostError::Transport("none".into())))
                        .collect())
                }
                async fn log(&mut self, _: String, _: String) -> wasmtime::Result<()> {
                    Ok(())
                }
                async fn regex_captures(
                    &mut self,
                    _: String,
                    _: String,
                ) -> wasmtime::Result<Vec<Vec<String>>> {
                    Ok(Vec::new())
                }
                async fn config_get(&mut self, _: String) -> wasmtime::Result<Option<String>> {
                    Ok(None)
                }
            }
        };
    }

    pub struct State {
        wasi: wasmtime_wasi::WasiCtx,
        table: wasmtime::component::ResourceTable,
    }

    impl State {
        pub fn new() -> Self {
            Self {
                wasi: wasmtime_wasi::WasiCtxBuilder::new().build(),
                table: wasmtime::component::ResourceTable::new(),
            }
        }
    }

    impl wasmtime_wasi::WasiView for State {
        fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
            wasmtime_wasi::WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
        }
    }

    pub mod unchanged {
        wasmtime::component::bindgen!({
            path: "tests/worlds/unchanged.wit",
            world: "book-plugin",
            imports: { default: async | trappable },
            exports: { default: async },
        });
    }
    lend_nothing!(unchanged);

    pub mod grown_record {
        wasmtime::component::bindgen!({
            path: "tests/worlds/grown-record.wit",
            world: "book-plugin",
            imports: { default: async | trappable },
            exports: { default: async },
        });
    }
    mod lend_grown_record {
        use super::{State, grown_record};
        lend_nothing!(grown_record);
    }

    pub mod grown_variant {
        wasmtime::component::bindgen!({
            path: "tests/worlds/grown-variant.wit",
            world: "book-plugin",
            imports: { default: async | trappable },
            exports: { default: async },
        });
    }
    mod lend_grown_variant {
        use super::{State, grown_variant};
        lend_nothing!(grown_variant);
    }
}

/// The example book plugin, compiled, with a linker for one of the worlds above.
fn harness(
    path: &std::path::Path,
) -> (wasmtime::Engine, wasmtime::component::Component, wasmtime::Store<worlds::State>) {
    let mut config = wasmtime::Config::new();
    config.wasm_component_model(true);
    let engine = wasmtime::Engine::new(&config).unwrap();
    let component = wasmtime::component::Component::from_file(&engine, path).unwrap();
    let store = wasmtime::Store::new(&engine, worlds::State::new());
    (engine, component, store)
}

#[tokio::test]
async fn the_control_world_loads_the_plugin_and_reads_its_chapters() {
    let Some(path) = example() else { return };
    let (engine, component, mut store) = harness(&path);
    let mut linker = wasmtime::component::Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker).unwrap();
    worlds::unchanged::BookPlugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(
        &mut linker,
        |state| state,
    )
    .unwrap();
    let plugin =
        worlds::unchanged::BookPlugin::instantiate_async(&mut store, &component, &linker)
            .await
            .expect("a host that checks fewer exports than a plugin has still loads it");
    let hints = worlds::unchanged::exports::kaeri::books::source::TitleHints {
        anilist_id: None,
        mal_id: None,
        romaji: None,
        english: None,
        native: None,
        synonyms: Vec::new(),
        medium: None,
    };
    let chapters = plugin
        .kaeri_books_source()
        .call_list_chapters(&mut store, LANTERN, &hints)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(chapters.len(), 5);
}

#[tokio::test]
async fn a_record_that_grows_a_field_is_a_different_world() {
    // A host whose `chapter` has one more `option` field than the plugin's cannot load the
    // plugin at all: the export's type is checked field by field when it is instantiated. So
    // growing a record in `source` strands every plugin built before it, and is a major.
    let Some(path) = example() else { return };
    let (engine, component, mut store) = harness(&path);
    let mut linker = wasmtime::component::Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker).unwrap();
    worlds::grown_record::BookPlugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(
        &mut linker,
        |state| state,
    )
    .unwrap();
    let error =
        worlds::grown_record::BookPlugin::instantiate_async(&mut store, &component, &linker)
            .await
            .err()
            .expect("a grown record must not match the plugin's");
    // The whole chain, with its causes: "type mismatch with results", then the field count.
    let message = format!("{error:?}");
    assert!(message.contains("expected record of 9 fields, found 8 fields"), "{message}");
}

#[tokio::test]
async fn a_variant_that_grows_a_case_is_a_different_world() {
    let Some(path) = example() else { return };
    let (engine, component, mut store) = harness(&path);
    let mut linker = wasmtime::component::Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker).unwrap();
    worlds::grown_variant::BookPlugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(
        &mut linker,
        |state| state,
    )
    .unwrap();
    let result =
        worlds::grown_variant::BookPlugin::instantiate_async(&mut store, &component, &linker)
            .await;
    let message = format!("{:?}", result.err().expect("a grown variant must not match"));
    assert!(message.contains("expected variant of 3 cases, found 2 cases"), "{message}");
}
