//! Where books come from: the registry of book sources, the folders and servers that feed it,
//! and the commands of the Sources screen.
//!
//! Folders are scanned into the store and read back through [`FolderSource`]; servers are
//! [`kaeri_bookserver`] clients with their sign-in in the secure store under
//! `book-server/<id>`; plugins come from [`crate::book_plugins::BookPlugins`]. All of them sit in
//! one [`BookRegistry`], which owns the person's order, the off switches, health and failover.
//!
//! The order and the off switches are kept in the store's meta, and told to sync as registers
//! (`book_source_order`, `book_source_off`) so another device's change arrives as an effect that
//! [`BookSources::apply_effect`] folds in.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use async_trait::async_trait;
use kaeri_books::{
    Limits,
    library::{self, Nothing, ScannedSeries},
};
use kaeri_bookserver::{Kind, Server};
use kaeri_core::{
    books::{
        BookFile, BookHints, BookSourceKind, BookSourceManifest, ChapterContent, HitAmount,
        ReadingHint, SeriesDetail, SeriesHit, SourceChapter, WorkKey, normalise_name,
    },
    error::ProviderError,
    media::Medium,
    traits::BookSource,
};
use kaeri_model::{
    book_sources::{
        BookFolderRow, BookFolderScan, BookGroupRow, BookServerFailure, BookServerKind,
        BookServerRow, BookSourceCommand, BookSourceEvent, BookSourcesView, FolderRef,
    },
    event::Event,
    servers::{ServerFailure, ServerStep},
};
use kaeri_providers::BookRegistry;
use kaeri_store::{
    Store,
    book_sources::{BookServer, Folder, LocalFile, LocalSeries},
    sync::kinds,
};
use kaeri_track::TokenStore;

use crate::{book_plugins::BookPlugins, outbox::Outbox};

pub(crate) mod added;
pub use added::RECHECK;

/// The id the folders answer to in the registry.
pub const FOLDERS: &str = "folders";
const META_ORDER: &str = "reading.book_order";
/// How long a source's word on how a series reads is kept before it is asked again.
const HINT_KEPT: i64 = 7 * 24 * 3600;
/// When a source could not say, how long until it is asked again.
const HINT_RETRY: i64 = 3600;
/// How long the source has to say how a series reads, asked in the background.
const HINT_WAIT: std::time::Duration = std::time::Duration::from_secs(20);
/// How long the reader's open waits for that answer, when none is kept and one is coming.
const HINT_OPEN_WAIT: std::time::Duration = std::time::Duration::from_millis(1500);

/// A source being asked how a series reads: it ends when the answer is kept or given up on.
pub(crate) type HintLookup = tokio::sync::watch::Receiver<()>;

/// Wait for a lookup to end, for no longer than the reader's open waits.
pub(crate) async fn settle_hint(mut lookup: HintLookup) {
    let _ = tokio::time::timeout(HINT_OPEN_WAIT, lookup.changed()).await;
}
const META_OFF: &str = "reading.book_off";
/// How long a removed folder can be taken back.
const UNDO_SECONDS: u64 = 8;

fn secret_key(server: &str) -> String {
    format!("book-server/{server}")
}

/// A short stable id for text: the hex of the hash a local work key uses.
fn short_id(text: &str) -> String {
    WorkKey::local(Medium::Manga, text).to_string().chars().skip(2).take(12).collect()
}

fn kind_of(kind: Kind) -> BookServerKind {
    match kind {
        Kind::Komga => BookServerKind::Komga,
        Kind::Kavita => BookServerKind::Kavita,
        Kind::Opds => BookServerKind::Opds,
    }
}

// ---------------------------------------------------------------------------------------------
// The folders as a source

/// The scanned folders, as one book source. Series are the scan's groups, chapters its files.
pub struct FolderSource {
    manifest: BookSourceManifest,
    store: Store,
}

/// The folders' name as a source, in the language `tag` names: "Folders". The name says what
/// they are, so no kind is drawn beside it.
pub fn folders_name(tag: &str) -> String {
    kaeri_model::Msg::from(kaeri_model::ReadingMsg::ReadSourceFolders).text_in(tag)
}

impl FolderSource {
    /// Named in English until [`BookSources::set_language`] names it in the app's language.
    pub fn new(store: Store) -> Self {
        Self::named(store, folders_name("en"))
    }

    fn named(store: Store, name: String) -> Self {
        Self {
            manifest: BookSourceManifest {
                id: FOLDERS.into(),
                name,
                version: String::new(),
                kind: BookSourceKind::Folder,
                media: vec![Medium::Manga, Medium::Novel],
                languages: Vec::new(),
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            },
            store,
        }
    }
}

/// `10500` as `10.5`.
fn milli_label(milli: i64) -> String {
    let (whole, part) = (milli / 1000, milli % 1000);
    if part == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{part:03}").trim_end_matches('0').to_owned()
    }
}

fn other(error: kaeri_store::StoreError) -> ProviderError {
    ProviderError::Other(error.to_string())
}

/// A folder series' files, as its hit's line says them: "1 file · CBZ", "3 files".
pub(crate) fn files_amount(
    files: &[kaeri_store::book_sources::LocalFile],
) -> Option<HitAmount> {
    let count = u32::try_from(files.len()).ok().filter(|n| *n > 0)?;
    let first = files[0].format.to_uppercase();
    let one = files.iter().all(|f| f.format.eq_ignore_ascii_case(&first));
    Some(HitAmount::Files { count, format: (one && !first.is_empty()).then_some(first) })
}

#[async_trait]
impl BookSource for FolderSource {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _hints: &BookHints,
    ) -> Result<Vec<SeriesHit>, ProviderError> {
        let wanted = normalise_name(query);
        Ok(self
            .store
            .local_book_series()
            .map_err(other)?
            .into_iter()
            .filter(|s| wanted.is_empty() || normalise_name(&s.name).contains(&wanted))
            .map(|s| {
                let files = self.store.local_book_files(&s.id).unwrap_or_default();
                let amount = files_amount(&files);
                SeriesHit { amount, ..SeriesHit::new(s.id, s.name) }
            })
            .collect())
    }

    async fn series(
        &self,
        id: &str,
        _hints: &BookHints,
    ) -> Result<SeriesDetail, ProviderError> {
        let series = self
            .store
            .local_book_series()
            .map_err(other)?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or(ProviderError::NotFound)?;
        Ok(SeriesDetail::new(series.id, series.name))
    }

    async fn chapters(
        &self,
        id: &str,
        _hints: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        let files = self.store.local_book_files(id).map_err(other)?;
        if files.is_empty() {
            return Err(ProviderError::NotFound);
        }
        Ok(files
            .into_iter()
            .map(|f| {
                let number = f.number_milli.map(milli_label);
                let mut chapter = SourceChapter::of(
                    &f.path,
                    f.volume.as_deref(),
                    number.as_deref(),
                    Some(&f.label),
                    &f.label,
                );
                chapter.page_count = f.pages;
                chapter
            })
            .collect())
    }

    async fn content(
        &self,
        _series: &str,
        chapter: &str,
    ) -> Result<ChapterContent, ProviderError> {
        let path = PathBuf::from(chapter);
        if !path.exists() {
            return Err(ProviderError::NotFound);
        }
        Ok(ChapterContent::Book(BookFile { path, temporary: false }))
    }
}

// ---------------------------------------------------------------------------------------------
// The holder

/// Everything the Sources screen and the reading commands share. Cheap to clone.
#[derive(Clone)]
pub struct BookSources {
    pub registry: BookRegistry,
    store: Store,
    tokens: TokenStore,
    client: reqwest::Client,
    keep: PathBuf,
    flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    scanning: Arc<Mutex<HashMap<String, Arc<AtomicU32>>>>,
    removed: Arc<RwLock<HashSet<String>>>,
    undo: Arc<Mutex<HashMap<String, tokio::task::AbortHandle>>>,
    /// Servers signed in to and not added yet, with their sign-ins: in memory only.
    pending: Arc<Mutex<HashMap<String, added::Pending>>>,
    /// The reading demo, in the registry while the demo is on.
    pub demo: kaeri_providers::reading_demo::ReadingDemo,
    /// The series whose sources are being asked how they read, by source and series.
    hint_lookups: Arc<Mutex<HashMap<(String, String), HintLookup>>>,
    /// What reading's catalogue keeps between commands: see [`crate::read_catalog::Memory`].
    pub catalog: crate::read_catalog::Memory,
    /// The chapters downloaded for reading and the queue that fetches them.
    pub downloads: crate::downloads::books::BookDownloads,
    /// The sends to e-readers under way.
    pub exports: crate::export::Exports,
    /// The OPDS feed, on the desktop builds.
    #[cfg(feature = "opds-feed")]
    pub feed: crate::export::feed::Feed,
    /// Reading progress waiting for book servers: see [`crate::write_back`].
    pub write_back: crate::write_back::WriteBack,
}

impl std::fmt::Debug for BookSources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.registry.ids()).finish()
    }
}

impl BookSources {
    pub fn new(
        store: Store,
        tokens: TokenStore,
        client: reqwest::Client,
        paths: &kaeri_core::config::Paths,
    ) -> Self {
        let this = Self {
            registry: BookRegistry::new(),
            store,
            tokens,
            client,
            keep: paths.cache_dir.join("books"),
            flags: Arc::default(),
            scanning: Arc::default(),
            removed: Arc::default(),
            undo: Arc::default(),
            pending: Arc::default(),
            hint_lookups: Arc::default(),
            demo: kaeri_providers::reading_demo::ReadingDemo::new(Some(
                paths.cache_dir.join("reading-demo"),
            )),
            catalog: crate::read_catalog::Memory::default(),
            downloads: crate::downloads::books::BookDownloads::new(
                paths.data_dir.join("book-downloads"),
            ),
            exports: crate::export::Exports::new(paths.data_dir.join("exports")),
            #[cfg(feature = "opds-feed")]
            feed: crate::export::feed::Feed::default(),
            write_back: crate::write_back::WriteBack::default(),
        };
        this.registry.put(Arc::new(FolderSource::new(this.store.clone())));
        this.restore();
        this
    }

    pub(crate) fn store(&self) -> &Store {
        &self.store
    }

    /// The order and the off switches as kept, and every server signed in to.
    fn restore(&self) {
        self.apply_kept();
        for server in self.store.book_servers().unwrap_or_default() {
            self.put_server(&server);
        }
    }

    fn kept_list(&self, key: &str) -> Vec<String> {
        self.store
            .get_meta_text(key)
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn apply_kept(&self) {
        self.registry.set_order(self.kept_list(META_ORDER));
        self.registry.set_off(self.kept_list(META_OFF));
    }

    fn keep_list(&self, key: &str, list: &[String]) {
        let text = serde_json::to_string(list).unwrap_or_default();
        if let Err(e) = self.store.set_meta_text(key, &text) {
            tracing::warn!(error = %e, "could not keep the book source list");
        }
    }

    /// Put plugins that have loaded into the registry, and take out the ones that were removed.
    pub fn adopt_plugins(&self, plugins: &BookPlugins) {
        let here: HashSet<String> = plugins.ids().into_iter().collect();
        for id in &here {
            if let Some(plugin) = plugins.get(id) {
                let same = self
                    .registry
                    .get(id)
                    .is_some_and(|s| std::ptr::addr_eq(Arc::as_ptr(&s), Arc::as_ptr(&plugin)));
                if !same {
                    self.registry.put(plugin);
                }
            }
        }
        for source in self.registry.all() {
            let manifest = source.manifest();
            if manifest.kind == BookSourceKind::Plugin && !here.contains(&manifest.id) {
                self.registry.remove(&manifest.id);
            }
        }
    }

    /// A plugin was removed while the app runs: it stops being asked at once.
    pub fn unload(&self, plugin: &str) {
        self.registry.remove(plugin);
    }

    fn flag(&self, id: &str) -> Arc<AtomicBool> {
        self.flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(id.to_owned())
            .or_default()
            .clone()
    }

    /// How `server` refuses a sign-in it no longer takes: a password for a Basic sign-in, a key
    /// or token otherwise.
    pub(crate) fn refusal_of(
        &self,
        server: &str,
    ) -> kaeri_model::book_sources::WriteBackProblem {
        use kaeri_model::book_sources::WriteBackProblem as P;
        match self.tokens.get(&secret_key(server)) {
            Ok(credential) if credential.starts_with("basic:") => P::RefusedPassword,
            _ => P::RefusedKey,
        }
    }

    /// Ask `server` once more after it refused the sign-in.
    pub(crate) fn forget_refusal(&self, server: &str) {
        self.flag(server).store(false, Ordering::Relaxed);
    }

    fn put_server(&self, server: &BookServer) {
        let Some(kind) = Kind::parse(&server.kind) else { return };
        let Ok(credential) = self.tokens.get(&secret_key(&server.id)) else {
            let _ =
                self.store.put_book_server(&BookServer { signed_out: true, ..server.clone() });
            return;
        };
        let flag = self.flag(&server.id);
        flag.store(server.signed_out, Ordering::Relaxed);
        let libraries = self.server_meta(&server.id).libraries;
        let built = kaeri_bookserver::source_with(
            &self.client,
            &Server {
                id: &server.id,
                name: &server.name,
                kind,
                address: &server.address,
                credential: &credential,
            },
            &libraries,
            self.keep.clone(),
            flag,
        );
        if let Some(source) = built {
            self.registry.put(source);
        }
    }

    /// A change another device made to the order or the off switches.
    pub fn apply_effect(&self, kind: &str, key: &str, value: Option<&str>) {
        match kind {
            kinds::BOOK_SOURCE_ORDER => {
                let order: Vec<String> =
                    value.and_then(|v| serde_json::from_str(v).ok()).unwrap_or_default();
                self.keep_list(META_ORDER, &order);
            }
            kinds::BOOK_SOURCE_OFF => {
                let mut off = self.kept_list(META_OFF);
                off.retain(|id| id != key);
                if value.is_some() {
                    off.push(key.to_owned());
                }
                self.keep_list(META_OFF, &off);
            }
            _ => return,
        }
        self.apply_kept();
    }

    fn set_order(&self, ids: Vec<String>) {
        self.keep_list(META_ORDER, &ids);
        let text = serde_json::to_string(&ids).unwrap_or_default();
        if let Err(e) =
            self.store.sync_register(kinds::BOOK_SOURCE_ORDER, "", Some(&text), None)
        {
            tracing::warn!(error = %e, "the book source order was not told to sync");
        }
        self.registry.set_order(ids);
    }

    fn set_off(&self, id: &str, off: bool) {
        let mut list = self.kept_list(META_OFF);
        list.retain(|i| i != id);
        if off {
            list.push(id.to_owned());
        }
        self.keep_list(META_OFF, &list);
        let value = off.then_some("1");
        if let Err(e) = self.store.sync_register(kinds::BOOK_SOURCE_OFF, id, value, None) {
            tracing::warn!(error = %e, "the book source switch was not told to sync");
        }
        self.registry.set_off(list);
    }

    // -----------------------------------------------------------------------------------------
    // The view

    pub fn view(
        &self,
        plugins: &BookPlugins,
        plugin_store: impl Fn(&str) -> Option<String>,
    ) -> BookSourcesView {
        self.adopt_plugins(plugins);
        let removed = self.removed.read().unwrap_or_else(|e| e.into_inner()).clone();
        let scanning = self.scanning.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let series = self.store.local_book_series().unwrap_or_default();
        let folders = self
            .store
            .book_folders()
            .unwrap_or_default()
            .into_iter()
            .map(|f| {
                let count = u32::try_from(series.iter().filter(|s| s.folder == f.id).count())
                    .unwrap_or(u32::MAX);
                let scan = match scanning.get(&f.id) {
                    Some(found) => {
                        BookFolderScan::Scanning { found: found.load(Ordering::Relaxed) }
                    }
                    None if !f.available => BookFolderScan::Missing,
                    None => BookFolderScan::Done,
                };
                let report = self.folder_meta(&f.id);
                BookFolderRow {
                    files: report.files,
                    skipped: report.skipped,
                    name: f.name.clone().or_else(|| f.path.clone()).unwrap_or_default(),
                    path: f.path.clone(),
                    removed: removed.contains(&f.id),
                    id: f.id,
                    scan,
                    series: count,
                }
            })
            .collect();
        let groups = series
            .iter()
            .filter(|s| !removed.contains(&s.folder))
            .map(|s| BookGroupRow {
                group: s.id.clone(),
                name: s.name.clone(),
                medium: Medium::parse(&s.medium).unwrap_or(Medium::Manga),
                work: s.work.clone().map(kaeri_core::books::WorkId::new),
                matched_title: None,
                files: u32::try_from(self.store.local_book_files(&s.id).map_or(0, |f| f.len()))
                    .unwrap_or(u32::MAX),
                kept: s.state == "kept",
            })
            .collect();
        let servers = self
            .store
            .book_servers()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|s| {
                let meta = self.server_meta(&s.id);
                let (pending, sending, saved_at, problem, problem_at) =
                    self.write_back_row(&s.id, s.report_progress);
                let kind = kind_of(Kind::parse(&s.kind)?);
                Some(BookServerRow {
                    kind,
                    off: self.registry.is_off(&s.id),
                    id: s.id,
                    name: s.name,
                    address: s.address,
                    series: meta.series.unwrap_or(0),
                    report_progress: s.report_progress,
                    signed_out: s.signed_out,
                    libraries: added::row_libraries(kind, meta.library_count),
                    empty: meta.empty,
                    rechecks_at: meta.rechecks_at(),
                    key_last4: meta.key_last4,
                    progress_pending: pending,
                    progress_sending: sending,
                    progress_saved_at: saved_at,
                    progress_problem: problem,
                    progress_problem_at: problem_at,
                })
            })
            .collect();
        let mut plugin_rows = plugins.rows(plugin_store);
        for row in &mut plugin_rows {
            row.off = self.registry.is_off(&row.id);
        }
        BookSourcesView {
            folders,
            groups,
            servers,
            plugins: plugin_rows,
            order: self.registry.ids(),
        }
    }

    // -----------------------------------------------------------------------------------------
    // Folders

    fn add_folder(&self, folder: &FolderRef, now: i64) -> Option<String> {
        let basis = folder.path.as_deref().or(folder.tree_uri.as_deref())?;
        let id = short_id(basis);
        let name = folder.name.clone().or_else(|| {
            folder.path.as_deref().and_then(|p| {
                std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned())
            })
        });
        let usable = folder.path.is_some();
        let row = Folder {
            id: id.clone(),
            path: folder.path.clone(),
            tree_uri: folder.tree_uri.clone(),
            bookmark: folder.bookmark.clone(),
            name,
            added_at: now,
            last_scan: None,
            available: usable,
        };
        self.store.put_book_folder(&row).ok()?;
        self.removed.write().unwrap_or_else(|e| e.into_inner()).remove(&id);
        Some(id)
    }

    /// Scan one folder on a blocking thread, telling `progress` what it has found.
    pub async fn scan_folder(
        &self,
        folder: Folder,
        now: i64,
        progress: impl Fn() + Send + 'static,
    ) -> Option<kaeri_model::came_in::FolderReport> {
        let Some(path) = folder.path.clone().map(PathBuf::from) else {
            let _ = self.store.set_book_folder_available(&folder.id, false);
            return None;
        };
        let found = Arc::new(AtomicU32::new(0));
        self.scanning
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(folder.id.clone(), found.clone());
        let counter = found.clone();
        let scanned = tokio::task::spawn_blocking(move || {
            library::scan(&path, &Nothing, &Limits::default(), &mut |n| {
                counter.store(n, Ordering::Relaxed);
                progress();
            })
        })
        .await;
        self.scanning.lock().unwrap_or_else(|e| e.into_inner()).remove(&folder.id);
        match scanned {
            Ok(Ok(scan)) => {
                let report = added::folder_report(&folder, &scan);
                self.keep_folder_meta(&report);
                let rows: Vec<_> = scan.series.iter().map(|s| to_rows(&folder.id, s)).collect();
                if let Err(e) = self.store.replace_book_scan(&folder.id, &rows, now) {
                    tracing::warn!(error = %e, "the scan was not kept");
                }
                // Each series is findable by its work, so chapters and editions can be asked.
                for (series, _) in &rows {
                    if let Some(work) = &series.work {
                        let keep = matches!(
                            self.store.book_match_state(FOLDERS, &series.id),
                            Ok(Some((_, true)))
                        );
                        if !keep {
                            let _ = self.store.set_book_match(
                                FOLDERS,
                                &series.id,
                                work,
                                series.confidence,
                                false,
                                false,
                                now,
                            );
                        }
                    }
                }
                Some(report)
            }
            Ok(Err(e)) => {
                tracing::info!(error = %e, "a book folder could not be read");
                let _ = self.store.set_book_folder_available(&folder.id, false);
                None
            }
            Err(e) => {
                tracing::warn!(error = %e, "the folder scan stopped");
                None
            }
        }
    }

    /// Put the reading demo in the registry, as the demo is switched on, with each of its series
    /// matched to the work it is read under, so a demo work opens before anything is searched.
    pub fn put_demo(&self, now: i64) {
        self.registry.put(Arc::new(self.demo.clone()));
        self.match_demo(now);
    }

    /// Match the demo's series to their works, unless they already are: kept in the store, so
    /// the works keep their progress across launches, and never shared with other devices,
    /// which match them the same way.
    fn match_demo(&self, now: i64) {
        use kaeri_core::{demo::DEMO_SOURCE_ID, reading_demo::WORKS};
        for work in &WORKS {
            let id = work.work_id();
            if matches!(self.store.book_match(DEMO_SOURCE_ID, work.id), Ok(Some(w)) if w == id.as_str())
            {
                continue;
            }
            if let Err(e) = self.store.set_book_match(
                DEMO_SOURCE_ID,
                work.id,
                id.as_str(),
                100,
                true,
                false,
                now,
            ) {
                tracing::warn!(error = %e, "a demo work was not matched");
            }
        }
    }

    /// The demo's works a reading search for `query` finds while the demo is on, as tiles
    /// marked as the demo's: none with the demo off, for an empty query, or for filters the
    /// demo has nothing for (genres, origins, a status).
    pub fn demo_tiles(&self, query: &kaeri_model::reading::ReadSearchQuery) -> Vec<WorkTile> {
        if self.registry.get(kaeri_core::demo::DEMO_SOURCE_ID).is_none()
            || !query.genres.is_empty()
            || !query.origins.is_empty()
            || query.status.is_some()
        {
            return Vec::new();
        }
        kaeri_core::reading_demo::WORKS
            .iter()
            .filter(|w| w.answers_to(&query.text))
            .filter(|w| query.medium.is_none_or(|m| m == w.medium()))
            .filter(|w| {
                query.formats.is_empty() || query.formats.iter().any(|f| f == w.format())
            })
            .map(demo_tile)
            .collect()
    }

    /// Take the reading demo out, and the pages it kept.
    pub fn remove_demo(&self) {
        self.registry.remove(kaeri_core::demo::DEMO_SOURCE_ID);
        self.demo.clear();
    }

    /// Whether a book source other than the demo is here: a folder, a server or a plugin.
    pub fn has_own(&self) -> bool {
        let folders = self.store.book_folders().map(|f| !f.is_empty()).unwrap_or(false);
        folders
            || self
                .registry
                .ids()
                .iter()
                .any(|id| id != FOLDERS && id != kaeri_core::demo::DEMO_SOURCE_ID)
    }
}

/// A work of the reading demo as a tile: its own medium and cover, marked as the demo's.
pub(crate) fn demo_tile(work: &kaeri_core::reading_demo::DemoWork) -> WorkTile {
    let medium = work.medium();
    WorkTile {
        work: work.work_id(),
        title: work.series.to_owned(),
        medium,
        cover: work.cover().map(str::to_owned),
        format_label: Some(work.format().to_owned()),
        progress: None,
        next: None,
        new_chapter: false,
        adult: false,
        facts: vec![kaeri_model::reading::kind_words(Some(work.format()), medium, None)],
        demo: true,
        not_in_sources: false,
        origin: None,
        amount: None,
        started: None,
    }
}

/// A scanned series as the store keeps it.
fn to_rows(folder: &str, series: &ScannedSeries) -> (LocalSeries, Vec<LocalFile>) {
    let medium = if series.novel { Medium::Novel } else { Medium::Manga };
    let id = format!("{folder}:{}", series.key);
    let work = WorkKey::local(medium, &series.name).to_string();
    let local = LocalSeries {
        id: id.clone(),
        folder: folder.to_owned(),
        name: series.name.clone(),
        medium: medium.as_str().to_owned(),
        work: Some(work),
        state: "unmatched".into(),
        confidence: 0,
        reason: None,
    };
    let files = series
        .files
        .iter()
        .map(|f| LocalFile {
            path: f.path.clone(),
            series: id.clone(),
            volume: f.volume.clone(),
            label: f.label.clone(),
            number_milli: f.number_milli,
            format: f.format.clone(),
            pages: f.pages,
            size: i64::try_from(f.size).unwrap_or(i64::MAX),
            mtime: f.mtime,
            comicinfo: f.info.as_ref().and_then(|i| serde_json::to_string(i).ok()),
        })
        .collect();
    (local, files)
}

// ---------------------------------------------------------------------------------------------
// Commands

fn server_failure(error: &kaeri_bookserver::ServerError, address: &str) -> BookServerFailure {
    use kaeri_bookserver::ServerError as E;
    match error {
        E::NotABookServer => BookServerFailure::NotABookServer,
        E::Unreachable(_) => BookServerFailure::Server {
            failure: ServerFailure::Unreachable { address: address.to_owned() },
        },
        E::Unauthorized => BookServerFailure::Server { failure: ServerFailure::Credentials },
        _ => BookServerFailure::Server { failure: ServerFailure::Other },
    }
}

/// Run one book source command. The slow ones run as tasks and answer when they finish.
pub fn command(
    sources: &BookSources,
    plugins: &BookPlugins,
    plugin_store: impl Fn(&str) -> Option<String> + Clone + Send + Sync + 'static,
    command: BookSourceCommand,
    now: i64,
    tx: &Outbox,
) {
    let tx = tx.clone();
    let this = sources.clone();
    let plugins = plugins.clone();
    let publish = {
        let (this, plugins, tx, plugin_store) =
            (this.clone(), plugins.clone(), tx.clone(), plugin_store.clone());
        move || {
            let view = this.view(&plugins, &plugin_store);
            let _ = tx.send(Event::BookSources(BookSourceEvent::View(view)));
        }
    };
    this.write_back.set_publisher(Arc::new(publish.clone()));
    match command {
        BookSourceCommand::Load => publish(),
        BookSourceCommand::AddFolder { folder } => {
            let Some(id) = this.add_folder(&folder, now) else { return publish() };
            publish();
            rescan(this, Some(id), now, publish, Some(tx));
        }
        BookSourceCommand::Scan => {
            rescan(this.clone(), None, now, publish.clone(), None);
            recheck(this, now, true, publish, tx);
        }
        BookSourceCommand::Recheck => recheck(this, now, true, publish, tx),
        BookSourceCommand::Probe { address, user, password } => {
            tokio::spawn(async move {
                let came = this
                    .probe(&address, user.as_deref(), password.as_ref().map(|p| p.0.as_str()))
                    .await;
                let _ = tx.send(Event::CameIn(came));
            });
        }
        BookSourceCommand::AddServer { pending, libraries, report_progress } => {
            match this.add_found(&pending, libraries, report_progress, now) {
                Some(came) => {
                    let _ = tx.send(Event::CameIn(came));
                }
                None => tracing::debug!("a server was added that is no longer held"),
            }
            publish();
        }
        BookSourceCommand::PrefetchDemo => {
            if this.registry.get(kaeri_core::demo::DEMO_SOURCE_ID).is_some() {
                let demo = this.demo.clone();
                tokio::spawn(async move {
                    let ready = demo.prefetch().await;
                    tracing::debug!(ready, "demo pages fetched ahead");
                });
            }
        }
        BookSourceCommand::RemoveFolder { id } => {
            this.removed.write().unwrap_or_else(|e| e.into_inner()).insert(id.clone());
            publish();
            let (keeper, forget) = (this.clone(), id.clone());
            let handle = tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(UNDO_SECONDS)).await;
                let gone =
                    keeper.removed.write().unwrap_or_else(|e| e.into_inner()).remove(&forget);
                if gone {
                    let _ = keeper.store.remove_book_folder(&forget);
                }
                keeper.undo.lock().unwrap_or_else(|e| e.into_inner()).remove(&forget);
                publish();
            });
            this.undo
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(id, handle.abort_handle());
        }
        BookSourceCommand::RestoreFolder { id } => {
            if let Some(timer) = this.undo.lock().unwrap_or_else(|e| e.into_inner()).remove(&id)
            {
                timer.abort();
            }
            this.removed.write().unwrap_or_else(|e| e.into_inner()).remove(&id);
            publish();
        }
        BookSourceCommand::FindMatch { group, query } => {
            // Matching needs the manga catalogue, which is not on this device yet.
            let _ = tx.send(Event::BookSources(BookSourceEvent::Matches {
                group,
                query,
                choices: Vec::new(),
                offline: true,
            }));
        }
        BookSourceCommand::ChooseMatch { group, anilist } => {
            let work = format!("a:{anilist}");
            let _ =
                this.store.set_local_series_match(&group, Some(&work), "matched", 100, None);
            let _ = this.store.set_book_match(FOLDERS, &group, &work, 100, true, false, now);
            publish();
        }
        BookSourceCommand::Keep { group } => {
            let series = this.store.local_book_series().unwrap_or_default();
            if let Some(s) = series.iter().find(|s| s.id == group) {
                let _ = this.store.set_local_series_match(
                    &group,
                    s.work.as_deref(),
                    "kept",
                    s.confidence,
                    None,
                );
            }
            publish();
        }
        BookSourceCommand::CheckServer { address } => {
            // A Kavita OPDS link is checked at its root: its key stays here until `Probe`.
            let link = kaeri_bookserver::kavita_link(&address);
            let address = link.as_ref().map_or(address, |l| l.root.clone());
            tokio::spawn(async move {
                let checked =
                    kaeri_bookserver::check(&this.client, &address).await.map(|mut c| {
                        if link.is_some() {
                            c.kind = Some(Kind::Kavita);
                        }
                        c
                    });
                let event = match checked {
                    Ok(found) => BookSourceEvent::ServerChecked {
                        // A server that wants a sign-in before it says is taken as a catalogue:
                        // signing in finds out which it is.
                        kind: kind_of(found.kind.unwrap_or(Kind::Opds)),
                        name: kaeri_bookserver::host_of(&found.address),
                        version: String::new(),
                        address: found.address,
                    },
                    Err(error) => BookSourceEvent::ServerFailed {
                        step: ServerStep::Address,
                        failure: server_failure(&error, &address),
                    },
                };
                let _ = tx.send(Event::BookSources(event));
            });
        }
        BookSourceCommand::SignIn { address, user, password } => {
            tokio::spawn(async move {
                let event = sign_in(&this, &address, &user, &password.0, now).await;
                let _ = tx.send(Event::BookSources(event));
                publish();
            });
        }
        BookSourceCommand::RemoveServer { id } => {
            this.registry.remove(&id);
            let _ = this.tokens.clear(&secret_key(&id));
            let _ = this.store.remove_book_server(&id);
            this.forget_server_meta(&id);
            publish();
        }
        BookSourceCommand::SetReportProgress { id, on } => {
            if let Ok(Some(server)) = this.store.book_server(&id) {
                let _ =
                    this.store.put_book_server(&BookServer { report_progress: on, ..server });
            }
            if !on {
                this.drop_progress(&id);
            }
            publish();
        }
        BookSourceCommand::RetryProgress { id } => {
            this.retry_progress(&id, now);
            publish();
        }
        BookSourceCommand::SetOrder { ids } => {
            this.set_order(ids);
            publish();
        }
        BookSourceCommand::SetOff { id, off } => {
            this.set_off(&id, off);
            publish();
        }
        // The catalogue answers these: see `read_catalog::editions`.
        BookSourceCommand::LoadForWork { .. }
        | BookSourceCommand::SearchSourceForWork { .. }
        | BookSourceCommand::ChooseSourceSeries { .. } => {}
    }
}

/// Scan the folders, or only `only`. With `report`, what came in from each folder is said.
fn rescan(
    this: BookSources,
    only: Option<String>,
    now: i64,
    publish: impl Fn() + Clone + Send + 'static,
    report: Option<Outbox>,
) {
    tokio::spawn(async move {
        let folders: Vec<_> = this
            .store
            .book_folders()
            .unwrap_or_default()
            .into_iter()
            .filter(|f| only.as_ref().is_none_or(|id| *id == f.id))
            .collect();
        for folder in folders {
            let tick = publish.clone();
            let found = this.scan_folder(folder, now, tick).await;
            if let (Some(tx), Some(found)) = (&report, found) {
                let _ = tx.send(Event::CameIn(added::folder_came_in(found)));
            }
            publish();
        }
    });
}

/// Look again at the servers that showed no series: those due, or every one with `all`. A
/// server that now holds some is said, as it would have been when added.
fn recheck(
    this: BookSources,
    now: i64,
    all: bool,
    publish: impl Fn() + Send + 'static,
    tx: Outbox,
) {
    tokio::spawn(async move {
        let filled = this.recheck(now, all).await;
        for server in &filled {
            if let Some(stored) = this.store.book_server(&server.id).ok().flatten() {
                // Asked again with what it now holds.
                this.put_server(&stored);
            }
        }
        for server in filled {
            let _ = tx.send(Event::CameIn(kaeri_model::came_in::CameIn::ServerAdded(server)));
        }
        publish();
    });
}

/// Every [`RECHECK`] once launch is over, look again at the servers that showed no series.
/// Runs in AniList's paced background queue, after Home's first screen.
pub fn spawn_rechecks(
    this: BookSources,
    anilist: kaeri_meta::AniList,
    tx: Outbox,
    clock: impl Fn() -> i64 + Send + 'static,
) {
    tokio::spawn(async move {
        anilist.launch_over().await;
        let mut every = tokio::time::interval(RECHECK);
        every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            every.tick().await;
            let now = clock();
            let filled = kaeri_meta::in_background(this.recheck(now, false)).await;
            for server in &filled {
                if let Some(stored) = this.store.book_server(&server.id).ok().flatten() {
                    this.put_server(&stored);
                }
            }
            for server in filled {
                let _ =
                    tx.send(Event::CameIn(kaeri_model::came_in::CameIn::ServerAdded(server)));
            }
        }
    });
}

async fn sign_in(
    this: &BookSources,
    address: &str,
    user: &str,
    password: &str,
    now: i64,
) -> BookSourceEvent {
    let failed = |step, error: &kaeri_bookserver::ServerError| BookSourceEvent::ServerFailed {
        step,
        failure: server_failure(error, address),
    };
    let found = match kaeri_bookserver::check(&this.client, address).await {
        Ok(found) => found,
        Err(e) => return failed(ServerStep::Address, &e),
    };
    let done = match kaeri_bookserver::sign_in(
        &this.client,
        &found.address,
        found.kind,
        user,
        password,
    )
    .await
    {
        Ok(done) => done,
        Err(e) => return failed(ServerStep::SignIn, &e),
    };
    let id = format!("srv-{}", short_id(&format!("{}\u{1f}{user}", found.address)));
    if this.tokens.set(&secret_key(&id), &done.credential).is_err() {
        return BookSourceEvent::ServerFailed {
            step: ServerStep::SignIn,
            failure: BookServerFailure::Server { failure: ServerFailure::Other },
        };
    }
    let row = BookServer {
        id: id.clone(),
        kind: done.kind.as_str().into(),
        name: done.name.clone(),
        address: found.address,
        user: Some(user.to_owned()),
        report_progress: true,
        signed_out: false,
        last_seen: Some(now),
    };
    if this.store.put_book_server(&row).is_err() {
        return BookSourceEvent::ServerFailed {
            step: ServerStep::SignIn,
            failure: BookServerFailure::Server { failure: ServerFailure::Other },
        };
    }
    this.put_server(&row);
    BookSourceEvent::ServerAdded { id, kind: kind_of(done.kind), series: done.series }
}

impl BookSources {
    /// Name what the engine names in the language the app speaks (a tag from
    /// `kaeri_i18n::languages()`): the folders as a source, and the feed's sections.
    pub fn set_language(&self, tag: &str) {
        self.registry.put(Arc::new(FolderSource::named(self.store.clone(), folders_name(tag))));
        #[cfg(feature = "opds-feed")]
        self.feed.use_language(tag);
    }

    /// A source's name as its manifest gives it, or its id when it is no longer loaded.
    pub fn source_name(&self, id: &str) -> String {
        self.registry.manifest(id).map_or_else(|| id.to_owned(), |m| m.name)
    }

    /// The editions a work has: every source holding a series matched to it.
    pub(crate) fn editions(
        &self,
        work: &kaeri_core::books::WorkId,
    ) -> Vec<kaeri_model::reading::BookEditionRow> {
        let preferred = self.store.edition_pref(work.as_str()).ok().flatten();
        let in_use = self.edition_of(work, None).map(|(source, _)| source);
        let refs = self.store.book_refs_for_work(work.as_str()).unwrap_or_default();
        let mut seen = HashSet::new();
        refs.into_iter()
            .filter(|(source, _)| seen.insert(source.clone()))
            .filter_map(|(source, series)| {
                let manifest = self.registry.manifest(&source)?;
                // What the source listed last time, from the chapter cache.
                let kept =
                    crate::read_catalog::chapters::summary(&self.store, &source, &series);
                Some(kaeri_model::reading::BookEditionRow {
                    preferred: preferred.as_ref().is_some_and(|p| p.source_id == source),
                    in_use: in_use.as_deref() == Some(source.as_str()),
                    source_id: source,
                    source_name: manifest.name,
                    language: kept.language,
                    version_label: kept.version_label,
                    chapters: kept.chapters,
                    latest: kept.latest,
                })
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Browsing and opening

use kaeri_core::books::{ChapterId, PageRef, WorkId};
use kaeri_model::reading::{
    BookEditionRow, ChapterReadState, ChapterRow, ReadingFailure, VolumeGroup, WorkTile,
};

/// A chapter opened: what it holds and where it came from.
#[derive(Debug)]
pub struct Opened {
    pub source_id: String,
    pub series_ref: String,
    pub chapter: SourceChapter,
    pub content: ChapterContent,
}

pub(crate) fn failure(source: &str, error: &ProviderError) -> ReadingFailure {
    match error {
        ProviderError::NotFound => ReadingFailure::NotFound,
        ProviderError::Unavailable(_) => {
            ReadingFailure::SignInNeeded { what: source.to_owned() }
        }
        ProviderError::Transport(_) => ReadingFailure::Offline,
        ProviderError::Blocked(reason) => {
            ReadingFailure::Blocked { source: source.to_owned(), reason: reason.clone() }
        }
        _ => ReadingFailure::SourceDown { source: source.to_owned() },
    }
}

/// Whether one chapter number appears in two volumes: numbering that starts again per volume.
pub fn restarts_per_volume(chapters: &[SourceChapter]) -> bool {
    let mut seen: HashMap<kaeri_core::books::ChapterRange, HashSet<&str>> = HashMap::new();
    for c in chapters {
        if let (Some(number), Some(volume)) = (c.number, c.volume.as_deref()) {
            seen.entry(number).or_default().insert(volume);
        }
    }
    seen.values().any(|volumes| volumes.len() > 1)
}

impl BookSources {
    /// Search every source and give the works found as tiles. A series no match has been made
    /// for gets a work of its own on this device, so it can be opened.
    pub async fn search(&self, text: &str, medium: Option<Medium>, now: i64) -> Vec<WorkTile> {
        let found = self.registry.search(text, &BookHints::default(), medium, now).await;
        let local = self.store.local_book_series().unwrap_or_default();
        let mut seen = HashSet::new();
        let mut tiles = Vec::new();
        for (source, hits) in found.found {
            for hit in hits {
                if let Some(tile) = self.hit_tile(&source, hit, medium, &local, now)
                    && seen.insert(tile.work.0.clone())
                {
                    tiles.push(tile);
                }
            }
        }
        tiles
    }

    /// What a source's hit is in the medium its source or folder says, else `medium`, else the
    /// first reading medium the source carries.
    pub(crate) fn hit_medium(
        &self,
        source: &str,
        hit: &SeriesHit,
        medium: Option<Medium>,
        local: &[kaeri_store::book_sources::LocalSeries],
    ) -> Medium {
        let default = self
            .registry
            .manifest(source)
            .as_ref()
            .and_then(|m| m.media.iter().copied().find(|m| m.is_reading()))
            .unwrap_or(Medium::Manga);
        let folder =
            (source == FOLDERS).then(|| local.iter().find(|s| s.id == hit.id)).flatten();
        folder.and_then(|s| Medium::parse(&s.medium)).or(medium).unwrap_or(default)
    }

    /// A source's hit as a tile: the work it is matched to, or a work of its own on this device
    /// when no match was made, kept so it can be opened. The demo's works are tiles of their
    /// own, in their medium and with their cover.
    pub(crate) fn hit_tile(
        &self,
        source: &str,
        hit: SeriesHit,
        medium: Option<Medium>,
        local: &[kaeri_store::book_sources::LocalSeries],
        now: i64,
    ) -> Option<WorkTile> {
        let amount = hit.amount.as_ref().map(crate::read_catalog::tiles::amount);
        if source == kaeri_core::demo::DEMO_SOURCE_ID {
            let mut tile = kaeri_core::reading_demo::work(&hit.id).map(demo_tile)?;
            tile.amount = amount;
            return Some(tile);
        }
        let origin = crate::read_catalog::tiles::origin(hit.origin.clone());
        let medium = self.hit_medium(source, &hit, medium, local);
        let work = match self.store.book_match(source, &hit.id) {
            Ok(Some(work)) => work,
            _ => {
                let work = WorkKey::local(medium, &hit.title).to_string();
                let _ =
                    self.store.set_book_match(source, &hit.id, &work, 50, false, false, now);
                work
            }
        };
        Some(WorkTile {
            work: WorkId::new(work),
            title: hit.title,
            medium,
            cover: hit.cover,
            format_label: hit.format.map(|f| f.as_str().to_owned()),
            progress: None,
            next: None,
            new_chapter: false,
            adult: false,
            facts: Vec::new(),
            demo: false,
            not_in_sources: false,
            origin,
            amount,
            started: None,
        })
    }

    /// Which source and series reference a work is read from: the chosen edition, the person's
    /// preference, or the first source in their order that holds it.
    pub(crate) fn edition_of(
        &self,
        work: &WorkId,
        edition: Option<&str>,
    ) -> Option<(String, String)> {
        let refs = self.store.book_refs_for_work(work.as_str()).unwrap_or_default();
        let wanted = edition.map(str::to_owned).or_else(|| {
            self.store.edition_pref(work.as_str()).ok().flatten().map(|p| p.source_id)
        });
        if let Some(wanted) = wanted
            && let Some(found) = refs.iter().find(|(source, _)| *source == wanted)
        {
            return Some(found.clone());
        }
        let order = self.registry.ids();
        order.into_iter().find_map(|id| refs.iter().find(|(source, _)| *source == id).cloned())
    }

    /// How the source a work is read from says its pages read, as last kept.
    pub(crate) fn hint_of(&self, work: &WorkId) -> Option<ReadingHint> {
        let (source, series) = self.edition_of(work, None)?;
        let (hint, _) = self.store.book_series_hint(&source, &series).ok().flatten()?;
        hint.as_deref().and_then(ReadingHint::parse)
    }

    /// A lookup the reader's open may wait for: the source of the work's edition is being asked
    /// how it reads, and nothing it said is kept.
    pub(crate) fn hint_lookup(&self, work: &WorkId) -> Option<HintLookup> {
        let (source, series) = self.edition_of(work, None)?;
        let lookup =
            self.hint_lookups.lock().ok()?.get(&(source.clone(), series.clone())).cloned()?;
        let kept = self.store.book_series_hint(&source, &series).ok().flatten();
        kept.is_none().then_some(lookup)
    }

    /// Ask the source how a series reads in the background, unless it was asked lately or is
    /// being asked now.
    fn ask_hint(&self, source: &str, series: &str, now: i64) {
        let kept = self.store.book_series_hint(source, series).ok().flatten();
        if kept.as_ref().is_some_and(|(_, at)| now - at < HINT_KEPT) {
            return;
        }
        let key = (source.to_owned(), series.to_owned());
        let (ended, lookup) = tokio::sync::watch::channel(());
        {
            let Ok(mut lookups) = self.hint_lookups.lock() else { return };
            if lookups.contains_key(&key) {
                return;
            }
            lookups.insert(key.clone(), lookup);
        }
        let sources = self.clone();
        tokio::spawn(async move {
            sources.learn_hint(&key.0, &key.1, kept, now).await;
            if let Ok(mut lookups) = sources.hint_lookups.lock() {
                lookups.remove(&key);
            }
            // Its waiters hear the lookup end as the sender goes.
            drop(ended);
        });
    }

    /// Ask the source how a series reads and keep the answer.
    async fn learn_hint(
        &self,
        source: &str,
        series: &str,
        kept: Option<(Option<String>, i64)>,
        now: i64,
    ) {
        let hints = BookHints::default();
        let asked = self.registry.series(source, series, &hints, now);
        let (hint, at) = match tokio::time::timeout(HINT_WAIT, asked).await {
            Ok(Ok(detail)) => (detail.reading_hint.map(ReadingHint::as_str), now),
            // Asked again within the hour, keeping what it said before.
            outcome => {
                if let Ok(Err(error)) = outcome {
                    tracing::debug!(source, %error, "a series' reading hint was not read");
                }
                let before = kept.as_ref().and_then(|(hint, _)| hint.as_deref());
                let hint = before.and_then(ReadingHint::parse).map(ReadingHint::as_str);
                (hint, now - HINT_KEPT + HINT_RETRY)
            }
        };
        if let Err(error) = self.store.put_book_series_hint(source, series, hint, at) {
            tracing::warn!(%error, "a series' reading hint was not kept");
        }
    }

    /// A work's chapters from its edition, grouped by volume, with the person's reading marked.
    pub async fn chapters(
        &self,
        work: &WorkId,
        edition: Option<&str>,
        now: i64,
    ) -> Result<(Option<BookEditionRow>, Vec<VolumeGroup>), ReadingFailure> {
        let (source, series) =
            self.edition_of(work, edition).ok_or(ReadingFailure::NotFound)?;
        // The source is asked how the series reads beside its chapters, and never held up for.
        self.ask_hint(&source, &series, now);
        let chapters = self
            .registry
            .chapters(&source, &series, &BookHints::default(), now)
            .await
            .map_err(|e| failure(&source, &e))?;
        crate::read_catalog::chapters::keep(&self.store, &source, &series, &chapters, now);
        let restarts = restarts_per_volume(&chapters);
        let read: HashMap<String, kaeri_store::reading::ReadChapter> = self
            .store
            .read_chapters(work.as_str())
            .unwrap_or_default()
            .into_iter()
            .map(|c| (c.chapter.clone(), c))
            .collect();
        let downloads = self.downloads.states(&self.store, work.as_str());
        let bookmarks: HashSet<String> =
            self.store.bookmarks(work.as_str()).unwrap_or_default().into_iter().collect();
        let mut groups: Vec<VolumeGroup> = Vec::new();
        let mut listed: HashSet<ChapterId> = HashSet::new();
        let mut group_of: HashMap<Option<String>, usize> = HashMap::new();
        for chapter in &chapters {
            let id = chapter.key(restarts).id();
            // Two versions of one chapter, as two groups' releases of it, are one row: the first
            // listed, which is the one that opens. Its row counts the others as versions.
            if !listed.insert(id.clone()) {
                continue;
            }
            let state = match read.get(id.as_str()) {
                Some(r) if r.read => ChapterReadState::Read,
                Some(r) => match r.last_page {
                    Some(page) => ChapterReadState::InProgress {
                        page: kaeri_model::reading::shown_page(page),
                        of: r.of_pages,
                    },
                    None => ChapterReadState::Unread,
                },
                None => ChapterReadState::Unread,
            };
            let read_at = read.get(id.as_str()).and_then(|r| r.read_at);
            let download = downloads.get(id.as_str()).cloned();
            let bookmarked = bookmarks.contains(id.as_str());
            let number = chapter.number.map(|n| n.to_string());
            let title = crate::read_catalog::chapters::own_title(
                chapter.title.as_deref(),
                &chapter.number_raw,
                number.as_deref(),
            )
            .map(str::to_owned);
            let row = ChapterRow {
                id,
                label: chapter.number_raw.clone(),
                state,
                number,
                title,
                volume: chapter.volume.clone(),
                published: chapter.published,
                pages: chapter.page_count,
                new_chapter: false,
                download,
                versions: 1,
                bookmarked,
                version_label: chapter.version_label.clone(),
                read_at,
                line: Vec::new(),
                gone_from_source: false,
            };
            let at = *group_of.entry(chapter.volume.clone()).or_insert_with(|| {
                groups.push(VolumeGroup {
                    volume: chapter.volume.clone(),
                    title: None,
                    chapters: Vec::new(),
                    all_read: false,
                });
                groups.len() - 1
            });
            groups[at].chapters.push(row);
        }
        keep_dropped(&self.store, work, &read, &bookmarks, &mut groups);
        for group in &mut groups {
            group.all_read =
                group.chapters.iter().all(|c| matches!(c.state, ChapterReadState::Read));
        }
        let editions = self.editions(work);
        let row = editions.into_iter().find(|e| e.source_id == source).map(|mut e| {
            e.chapters = u32::try_from(chapters.len()).unwrap_or(u32::MAX);
            e
        });
        Ok((row, groups))
    }

    /// Open a chapter: its content from the edition, with the source named so the reader can
    /// ask again for page addresses that expire.
    pub async fn open(
        &self,
        work: &WorkId,
        chapter: &ChapterId,
        edition: Option<&str>,
        now: i64,
    ) -> Result<Opened, ReadingFailure> {
        let (source, series, found) = self.find_chapter(work, chapter, edition, now).await?;
        let content = self
            .registry
            .content(&source, &series, &found.id, now)
            .await
            .map_err(|e| failure(&source, &e))?;
        Ok(Opened { source_id: source, series_ref: series, chapter: found, content })
    }

    /// The source, its series and its own chapter for `chapter` of `work`, in the edition.
    async fn find_chapter(
        &self,
        work: &WorkId,
        chapter: &ChapterId,
        edition: Option<&str>,
        now: i64,
    ) -> Result<(String, String, SourceChapter), ReadingFailure> {
        let (source, series) =
            self.edition_of(work, edition).ok_or(ReadingFailure::NotFound)?;
        let chapters = self
            .registry
            .chapters(&source, &series, &BookHints::default(), now)
            .await
            .map_err(|e| failure(&source, &e))?;
        let restarts = restarts_per_volume(&chapters);
        let found = chapters
            .into_iter()
            .find(|c| c.key(restarts).id() == *chapter)
            .ok_or(ReadingFailure::NotFound)?;
        Ok((source, series, found))
    }

    /// The first page of a chapter of pictures, and nothing more of it: the reader's preview of
    /// the next chapter. `None` for a chapter of text, or one only a whole file's download
    /// would open. The page is fetched as the reader fetches it, with fresh addresses asked
    /// for once on a 403 or 410.
    pub async fn first_page(
        &self,
        work: &WorkId,
        chapter: &ChapterId,
        now: i64,
    ) -> Result<Option<Vec<u8>>, ReadingFailure> {
        let (source, series, found) = self.find_chapter(work, chapter, None, now).await?;
        let pages = self
            .registry
            .chapter_pages(&source, &series, &found.id, now)
            .await
            .map_err(|e| failure(&source, &e))?;
        let Some(pages) = pages.filter(|p| !p.is_empty()) else { return Ok(None) };
        let mut opened = Opened {
            source_id: source,
            series_ref: series,
            chapter: found,
            content: ChapterContent::Images(pages),
        };
        self.page(&mut opened, 0, now).await.map(Some)
    }

    /// Fetch page `index` of an opened chapter's images, asking the source again once for
    /// addresses that have expired.
    pub async fn page(
        &self,
        opened: &mut Opened,
        index: usize,
        now: i64,
    ) -> Result<Vec<u8>, ReadingFailure> {
        let ChapterContent::Images(pages) = &mut opened.content else {
            return Err(ReadingFailure::NotFound);
        };
        let at = kaeri_providers::ChapterAt {
            source: &opened.source_id,
            series: &opened.series_ref,
            chapter: &opened.chapter.id,
        };
        self.registry
            .fetch_page(&at, pages, index, now, |page: PageRef| {
                let client = self.client.clone();
                async move { plain_fetch(&client, &page).await }
            })
            .await
            .map_err(|e| failure(&opened.source_id, &e))
    }
}

/// Chapters downloaded here that the source no longer lists stay on the series page, marked as
/// gone from the source, in their place among the others: the file is still on the device and
/// still opens.
fn keep_dropped(
    store: &Store,
    work: &WorkId,
    read: &HashMap<String, kaeri_store::reading::ReadChapter>,
    bookmarks: &HashSet<String>,
    groups: &mut Vec<VolumeGroup>,
) {
    let listed: HashSet<String> = groups
        .iter()
        .flat_map(|g| g.chapters.iter().map(|c| c.id.as_str().to_owned()))
        .collect();
    let downloads = store.book_downloads(Some(work.as_str())).unwrap_or_default();
    for d in downloads.iter().filter(|d| {
        d.state == kaeri_store::book_downloads::BookDownloadState::Done
            && d.path.as_deref().is_some_and(|p| std::path::Path::new(p).exists())
            && !listed.contains(&d.chapter)
    }) {
        let was = read.get(&d.chapter);
        let mut row = crate::downloads::books::chapter_row(
            d,
            was.is_some_and(|r| r.read),
            bookmarks.contains(&d.chapter),
        );
        if let Some(r) = was {
            row.read_at = r.read_at;
            if !r.read
                && let Some(page) = r.last_page
            {
                row.state = ChapterReadState::InProgress {
                    page: kaeri_model::reading::shown_page(page),
                    of: r.of_pages,
                };
            }
        }
        row.gone_from_source = true;
        let group = match groups.iter().position(|g| g.volume == row.volume) {
            Some(at) => &mut groups[at],
            None => {
                groups.push(VolumeGroup {
                    volume: row.volume.clone(),
                    title: None,
                    chapters: Vec::new(),
                    all_read: false,
                });
                groups.last_mut().expect("just pushed")
            }
        };
        let at = place_of(&group.chapters, &row);
        group.chapters.insert(at, row);
    }
}

/// Where a row goes among others listed in number order, rising or falling; the end when its
/// number is not known.
fn place_of(rows: &[ChapterRow], row: &ChapterRow) -> usize {
    let range = |r: &ChapterRow| r.id.key().and_then(|k| k.range());
    let Some(mine) = range(row) else { return rows.len() };
    let falling = match (rows.first().and_then(range), rows.last().and_then(range)) {
        (Some(first), Some(last)) => first > last,
        _ => false,
    };
    rows.iter()
        .position(|r| {
            range(r).is_some_and(|theirs| if falling { theirs < mine } else { theirs > mine })
        })
        .unwrap_or(rows.len())
}

/// A page fetched with the engine's own client, for a source that does not fetch its own.
async fn plain_fetch(
    client: &reqwest::Client,
    page: &PageRef,
) -> Result<Vec<u8>, kaeri_core::traits::PageError> {
    use kaeri_core::traits::PageError;
    let mut request = client.get(&page.url).timeout(std::time::Duration::from_secs(30));
    for (name, value) in &page.headers {
        request = request.header(name, value);
    }
    let response = request.send().await.map_err(|_| {
        PageError::Failed(ProviderError::Transport("the request failed".into()))
    })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(PageError::Status(status));
    }
    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|_| PageError::Failed(ProviderError::Transport("the request failed".into())))
}
