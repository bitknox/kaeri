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
pub(crate) mod pictures;
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
/// How long a series' list the source gave stays the reader's list, in seconds: an open soon
/// after the series page reads the list the page asked for.
pub(crate) const LIST_FRESH: i64 = 30 * 60;
/// How long a kept list may name the chapter an open asks the source for, in seconds. A
/// chapter it names that the source no longer finds there is looked up again.
const REF_FRESH: i64 = 24 * 3600;
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

/// When a chapter list came back empty, and the [`BookSources::source_mark`] of the source
/// that said so.
type EmptyList = (std::time::Instant, Option<u64>);

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
    /// The series whose chapter list came back empty lately, by source and series, with the
    /// [`BookSources::source_mark`] of the source that said so: an empty answer is not kept, so
    /// this is what keeps a work from being read from it meanwhile.
    empty: Arc<Mutex<HashMap<(String, String), EmptyList>>>,
    /// The language the app speaks, as [`BookSources::set_language`] last gave it: the language
    /// a work's chapters are counted in when its editions are compared, and the group a series
    /// is read from is one in it when the person picked none.
    language: Arc<RwLock<Option<String>>>,
    /// Chapter lists are grouped by volume: the Show volumes setting, as
    /// [`BookSources::set_show_volumes`] last gave it.
    show_volumes: Arc<std::sync::atomic::AtomicBool>,
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
    /// The book plugins as last taken in, asked whether a load is still under way.
    plugins: Arc<RwLock<Option<BookPlugins>>>,
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
            empty: Arc::default(),
            language: Arc::default(),
            show_volumes: Arc::default(),
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
            plugins: Arc::default(),
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
        let here = self.take_in_plugins(plugins);
        for source in self.registry.all() {
            let manifest = source.manifest();
            if manifest.kind == BookSourceKind::Plugin && !here.contains(&manifest.id) {
                self.registry.remove(&manifest.id);
            }
        }
    }

    /// Put plugins that have loaded into the registry, leaving the rest as it is; the ids of
    /// the plugins loaded.
    pub(crate) fn take_in_plugins(&self, plugins: &BookPlugins) -> HashSet<String> {
        if let Ok(mut seen) = self.plugins.write() {
            seen.get_or_insert_with(|| plugins.clone());
        }
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
        here
    }

    /// [`Self::take_in_plugins`], once plugins still loading are in, when `source` is not in yet
    /// (any source with `None`): a download or list asked for at launch is not answered "not
    /// found" for want of its plugin. Waits no longer than the series page does.
    pub(crate) async fn plugins_in(&self, plugins: &BookPlugins, source: Option<&str>) {
        self.take_in_plugins(plugins);
        if plugins.is_arriving() && source.is_none_or(|s| self.registry.get(s).is_none()) {
            plugins.arrived(crate::read_catalog::editions::PLUGINS_WAIT).await;
            self.take_in_plugins(plugins);
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
            .filter(|w| query.formats.is_empty() || query.format_keys().contains(&w.format()))
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
            rescan(this, Some(id), now, publish, tx, true);
        }
        BookSourceCommand::Scan => {
            rescan(this.clone(), None, now, publish.clone(), tx.clone(), false);
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
                    changed(&tx);
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
            let tx = tx.clone();
            let handle = tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(UNDO_SECONDS)).await;
                let gone =
                    keeper.removed.write().unwrap_or_else(|e| e.into_inner()).remove(&forget);
                if gone {
                    let _ = keeper.store.remove_book_folder(&forget);
                    changed(&tx);
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
            changed(&tx);
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
                let added = matches!(event, BookSourceEvent::ServerAdded { .. });
                let _ = tx.send(Event::BookSources(event));
                if added {
                    changed(&tx);
                }
                publish();
            });
        }
        BookSourceCommand::RemoveServer { id } => {
            this.registry.remove(&id);
            let _ = this.tokens.clear(&secret_key(&id));
            let _ = this.store.remove_book_server(&id);
            this.forget_server_meta(&id);
            publish();
            changed(&tx);
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
            changed(&tx);
        }
        BookSourceCommand::SetOff { id, off } => {
            this.set_off(&id, off);
            publish();
            changed(&tx);
        }
        // The catalogue answers these: see `read_catalog::editions`.
        BookSourceCommand::LoadForWork { .. }
        | BookSourceCommand::SearchSourceForWork { .. }
        | BookSourceCommand::ChooseSourceSeries { .. } => {}
    }
}

/// The book sources changed: what reading shows is brought up to date.
fn changed(tx: &Outbox) {
    let _ = tx.internal(crate::outbox::Internal::BookSourcesChanged);
}

/// Scan the folders, or only `only`. With `report`, what came in from each folder is said.
/// Reading is brought up to date once they are scanned.
fn rescan(
    this: BookSources,
    only: Option<String>,
    now: i64,
    publish: impl Fn() + Clone + Send + 'static,
    tx: Outbox,
    report: bool,
) {
    tokio::spawn(async move {
        let folders: Vec<_> = this
            .store
            .book_folders()
            .unwrap_or_default()
            .into_iter()
            .filter(|f| only.as_ref().is_none_or(|id| *id == f.id))
            .collect();
        let scanned = !folders.is_empty();
        for folder in folders {
            let tick = publish.clone();
            let found = this.scan_folder(folder, now, tick).await;
            if let (true, Some(found)) = (report, found) {
                let _ = tx.send(Event::CameIn(added::folder_came_in(found)));
            }
            publish();
        }
        if scanned {
            changed(&tx);
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
        if !filled.is_empty() {
            changed(&tx);
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
            if !filled.is_empty() {
                changed(&tx);
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
        if let Ok(mut language) = self.language.write() {
            *language = Some(tag.to_owned());
        }
        self.registry.put(Arc::new(FolderSource::named(self.store.clone(), folders_name(tag))));
        #[cfg(feature = "opds-feed")]
        self.feed.use_language(tag);
    }

    /// Group chapter lists by volume or not, from here on: see [`crate::read_catalog::volumes`].
    pub fn set_show_volumes(&self, show: bool) {
        self.show_volumes.store(show, std::sync::atomic::Ordering::Relaxed);
    }

    /// Whether chapter lists are grouped by volume.
    pub fn show_volumes(&self) -> bool {
        self.show_volumes.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The language the app speaks, once set.
    pub fn language(&self) -> Option<String> {
        self.language.read().ok().and_then(|l| l.clone())
    }

    /// A source's name as its manifest gives it, or its id when it is no longer loaded.
    pub fn source_name(&self, id: &str) -> String {
        self.registry.manifest(id).map_or_else(|| id.to_owned(), |m| m.name)
    }

    /// A mark of source `id` as it stands: its version and which join of it is in the chain.
    /// What the source said is good only while the mark holds: an update, or a server put again
    /// with what it now holds, makes another. `None` when it is not in the chain.
    pub(crate) fn source_mark(&self, id: &str) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        let manifest = self.registry.manifest(id)?;
        let joined = self.registry.joined(id)?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (id, manifest.version.as_str(), joined).hash(&mut hasher);
        Some(hasher.finish())
    }

    /// A mark of the sources searched by name, the ones on beside the folders, each at its
    /// [`Self::source_mark`], in no order: a work none of them had may be had once it changes.
    /// Moving a source leaves it as it was, since a miss is every source saying no.
    pub(crate) fn sources_mark(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut marks: Vec<u64> = self
            .registry
            .ids()
            .into_iter()
            .filter(|id| id != FOLDERS && !self.registry.is_off(id))
            .filter_map(|id| self.source_mark(&id))
            .collect();
        marks.sort_unstable();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        marks.hash(&mut hasher);
        hasher.finish()
    }

    /// The editions a work has: every source holding a series matched to it.
    pub(crate) fn editions(
        &self,
        work: &kaeri_core::books::WorkId,
    ) -> Vec<kaeri_model::reading::BookEditionRow> {
        let preferred = self.store.edition_pref(work.as_str()).ok().flatten();
        let in_use = self.edition_of(work, None);
        let mut refs = self.store.book_refs_for_work(work.as_str()).unwrap_or_default();
        // A source holding two series for the work shows the one read from.
        if let Some(used) = &in_use {
            refs.retain(|r| r.0 != used.0 || r == used);
        }
        let in_use = in_use.map(|(source, _)| source);
        let mut seen = HashSet::new();
        let language = self.language();
        refs.into_iter()
            .filter(|(source, _)| seen.insert(source.clone()))
            .filter_map(|(source, series)| {
                let manifest = self.registry.manifest(&source)?;
                // What the source listed last time, from the chapter cache.
                let listing = self.listing_of(&source, &series);
                let chosen = crate::read_catalog::chapters::chosen_group(
                    &self.store,
                    work.as_str(),
                    &source,
                );
                let kept = crate::read_catalog::chapters::summary(
                    &self.store,
                    &source,
                    &series,
                    chosen.as_deref(),
                    language.as_deref(),
                );
                let groups = kept
                    .groups
                    .iter()
                    .map(|g| kaeri_model::reading::GroupChoice {
                        in_use: kept.version_label.as_deref() == Some(g.label.as_str()),
                        label: g.label.clone(),
                        chapters: g.chapters,
                        language: g.language.clone(),
                    })
                    .collect();
                Some(kaeri_model::reading::BookEditionRow {
                    listing,
                    preferred: preferred.as_ref().is_some_and(|p| p.source_id == source),
                    in_use: in_use.as_deref() == Some(source.as_str()),
                    source_id: source,
                    source_name: manifest.name,
                    language: kept.language,
                    version_label: kept.version_label,
                    chapters: kept.chapters,
                    latest: kept.latest,
                    groups,
                    group_chosen: kept.chosen,
                })
            })
            .collect()
    }
}

/// How well each held series fits a work, to pick its edition by: a tier (0 has about all its
/// chapters or nothing is known of a server's or folder's books, 1 not known yet, 2 short of
/// them, 3 missing the first chapters, 4 a one-shot, none or far more), then how far its count
/// is from the work's. Chapters are counted in `language` when any series has some in it; the
/// work's count is `total`, else the most any series has.
fn editions_fit(
    sources: &BookSources,
    held: &[(String, String)],
    total: Option<u32>,
    language: Option<&str>,
) -> Vec<(u8, u32)> {
    use crate::read_catalog::{chapters, editions::counts_chapters, fit::Facts};
    let primary = |tag: &str| tag.split(['-', '_']).next().unwrap_or(tag).to_lowercase();
    let lists: Vec<Option<Vec<chapters::Kept>>> = held
        .iter()
        .map(|(source, series)| {
            counts_chapters(sources, source)
                .then(|| chapters::kept(&sources.store, source, series).map(|(list, _)| list))
                .flatten()
        })
        .collect();
    let count = |list: &[chapters::Kept], language: Option<&str>| {
        let mut facts = Facts::default();
        facts.count_labels(
            list.iter()
                .filter(|c| match (language, c.language.as_deref()) {
                    (Some(want), Some(theirs)) => primary(theirs) == primary(want),
                    _ => true,
                })
                .map(|c| (c.number.clone(), c.label.as_str(), c.title.as_deref())),
        );
        facts
    };
    let in_language = language.filter(|want| {
        lists
            .iter()
            .flatten()
            .flatten()
            .any(|c| c.language.as_deref().is_some_and(|l| primary(l) == primary(want)))
    });
    let counted: Vec<Option<Facts>> =
        lists.iter().map(|list| list.as_deref().map(|l| count(l, in_language))).collect();
    let most = counted.iter().flatten().filter_map(|f| f.chapters).max();
    let target = total.or(most);
    counted
        .into_iter()
        .zip(held)
        .map(|(facts, (source, _))| {
            let Some(facts) = facts else {
                return if counts_chapters(sources, source) {
                    (1, u32::MAX)
                } else {
                    (0, u32::MAX)
                };
            };
            let n = facts.chapters.unwrap_or(0);
            let one_shot = facts.named_one_shot || (n <= 1 && target.is_some_and(|t| t >= 2));
            // Nothing to read in the person's language, or only a stray chapter.
            if n == 0 || one_shot {
                return (4, u32::MAX);
            }
            let Some(target) = target else { return (0, 0) };
            // How far it reaches: the last chapter's number when that is lower than the count,
            // since extras, parts and side chapters add to a count and never to the numbers.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = facts.last.map_or(n, |last| n.min(last.max(0.0).floor() as u32).max(1));
            let slack = (target / 5).max(10);
            let near = (target / 20).max(3);
            if n > target + slack {
                (4, target.abs_diff(n))
            } else if !facts.has_the_start() {
                // A series that begins at chapter 47 cannot be read from the start.
                (3, target.saturating_sub(n))
            } else if n + near >= target {
                (0, 0)
            } else {
                (2, target - n)
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Browsing and opening

use kaeri_core::books::{ChapterId, PageRef, WorkId};
use kaeri_model::reading::{
    BookEditionRow, ChapterReadState, ChapterRow, EditionListing, ReadingFailure, VolumeGroup,
    WorkTile,
};

use crate::read_catalog::chapters::Asked;

/// A chapter opened: what it holds and where it came from.
#[derive(Debug, Clone)]
pub struct Opened {
    pub source_id: String,
    pub series_ref: String,
    /// The chapter, or its first part.
    pub chapter: SourceChapter,
    /// The pages of every part in reading order, or the chapter's text or file.
    pub content: ChapterContent,
    /// A chapter the source lists in parts: each part with its own page addresses, which
    /// [`BookSources::page`] asks for again by part when they expire. Empty for a chapter
    /// listed whole.
    pub parts: Vec<OpenedPart>,
    /// The group it is read in, as the source names it.
    pub group: Option<String>,
    /// For a chapter in parts: the parts in all, and those no group lists.
    pub total: u32,
    pub missing: Vec<u32>,
}

/// One part of a chapter opened in parts.
#[derive(Debug, Clone)]
pub struct OpenedPart {
    pub chapter: SourceChapter,
    pub pages: Vec<PageRef>,
    /// Its number, from 1.
    pub number: u32,
}

/// A chapter's entries in a source's list, in reading order, as [`BookSources::open`] reads
/// them.
#[derive(Debug)]
struct Found {
    entries: Vec<SourceChapter>,
    /// For a chapter in parts: each entry's part number, the parts in all and those missing.
    numbers: Vec<u32>,
    total: u32,
    missing: Vec<u32>,
}

impl Opened {
    /// A copy of a chapter of pictures, to fetch some of its pages alongside others. `None`
    /// for a chapter of text or a file.
    pub(crate) fn pictures_copy(&self) -> Option<Self> {
        let ChapterContent::Images(pages) = &self.content else { return None };
        Some(Self {
            source_id: self.source_id.clone(),
            series_ref: self.series_ref.clone(),
            chapter: self.chapter.clone(),
            content: ChapterContent::Images(pages.clone()),
            parts: self.parts.clone(),
            group: self.group.clone(),
            total: self.total,
            missing: self.missing.clone(),
        })
    }

    /// Where each part starts in the pages and how many pages it has: one span for a chapter
    /// listed whole.
    pub fn spans(&self) -> Vec<(u32, u32)> {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        if self.parts.is_empty() {
            return match &self.content {
                ChapterContent::Images(pages) => vec![(0, count(pages.len()))],
                _ => Vec::new(),
            };
        }
        let mut at = 0;
        self.parts
            .iter()
            .map(|p| {
                let span = (at, count(p.pages.len()));
                at += span.1;
                span
            })
            .collect()
    }
}

/// A chapter row from the source's entries for it: one, or its parts in order.
fn source_row(
    parts: &[&SourceChapter],
    id: ChapterId,
    state: ChapterReadState,
    read_at: Option<i64>,
    download: Option<kaeri_model::reading::ChapterDownloadState>,
    bookmarked: bool,
    (all, arranged, app): (
        &crate::read_catalog::arrange::Arranged,
        &crate::read_catalog::arrange::Row,
        Option<&str>,
    ),
) -> ChapterRow {
    let chapter = parts[0];
    let split = crate::read_catalog::arrange::Arranged::in_parts(arranged);
    let label = if split {
        crate::read_catalog::arrange::merged_label(chapter, &id)
    } else {
        chapter.number_raw.clone()
    };
    let number = id
        .key()
        .and_then(|k| k.range())
        .map(|r| r.to_string())
        .or_else(|| chapter.number.map(|n| n.to_string()));
    // A part's title often only says which part it is; the chapter's is the first part's,
    // without that.
    let merged = split.then(|| crate::read_catalog::arrange::merged_title(chapter)).flatten();
    let title = crate::read_catalog::chapters::own_title(
        if split { merged.as_deref() } else { chapter.title.as_deref() },
        &label,
        number.as_deref(),
    )
    .map(str::to_owned);
    let pages = parts.iter().map(|p| p.page_count).sum::<Option<u32>>();
    let mut row = ChapterRow {
        id,
        label,
        state,
        number,
        title,
        volume: chapter.volume.clone(),
        published: parts.iter().filter_map(|p| p.published).max(),
        pages,
        new_chapter: false,
        download,
        versions: 1,
        bookmarked,
        version_label: chapter.version_label.clone(),
        read_at,
        line: Vec::new(),
        gone_from_source: false,
        parts: 1,
        missing_parts: Vec::new(),
        part_groups: Vec::new(),
        other_group: false,
        group_language: None,
        other_source: None,
    };
    all.fill(arranged, app, &mut row);
    row
}

/// Why asking a source failed. `source` is the name the person knows it by, as the words
/// show it: [`BookSources::failure`] looks it up from an id.
pub(crate) fn failure(source: &str, error: &ProviderError) -> ReadingFailure {
    match error {
        ProviderError::NotFound => ReadingFailure::NotFound,
        ProviderError::Unavailable(_) => {
            ReadingFailure::SignInNeeded { what: source.to_owned() }
        }
        // A source not reached is not answering: the device is offline only when the network
        // says so, which [`BookSources::failure`] knows.
        ProviderError::Transport(_) => ReadingFailure::SourceDown { source: source.to_owned() },
        ProviderError::Blocked(reason) => {
            ReadingFailure::Blocked { source: source.to_owned(), reason: reason.clone() }
        }
        _ => ReadingFailure::SourceDown { source: source.to_owned() },
    }
}

/// Put a source's list in reading order, the one order the series page lists and the reader
/// reads: many sources list their newest chapter first. Numbered chapters go by number (by
/// volume first where numbers start again each volume), so a decimal or a run of chapters
/// takes its place between the others. A chapter with no number, such as an extra, stays
/// where the source put it: right behind the chapter it came out after. Versions of one
/// chapter keep the source's order, so the same one opens.
pub fn oldest_first(chapters: &mut Vec<SourceChapter>) {
    type Key = (i64, kaeri_core::books::ChapterRange);
    let restarts = restarts_per_volume(chapters);
    let key = |c: &SourceChapter| -> Option<Key> {
        let volume = if restarts { volume_order(c.volume.as_deref()) } else { 0 };
        c.number.map(|n| (volume, n))
    };
    let first = chapters.iter().find_map(key);
    let last = chapters.iter().rev().find_map(key);
    let falling = matches!((first, last), (Some(f), Some(l)) if f > l);
    // Each numbered chapter with the chapters of no number that came out after it, and its
    // place in the source's list.
    let mut runs: Vec<(Option<Key>, usize, Vec<SourceChapter>)> = Vec::new();
    let mut listed: Vec<(usize, SourceChapter)> = chapters.drain(..).enumerate().collect();
    if falling {
        listed.reverse();
    }
    for (at, chapter) in listed {
        match (key(&chapter), runs.last_mut()) {
            (None, Some(run)) => run.2.push(chapter),
            (found, _) => runs.push((found, at, vec![chapter])),
        }
    }
    runs.sort_by_key(|(key, at, _)| (*key, *at));
    chapters.extend(runs.into_iter().flat_map(|(_, _, run)| run));
}

/// A volume's place for [`oldest_first`]: its number, with no volume after every numbered one.
fn volume_order(volume: Option<&str>) -> i64 {
    volume
        .map(|v| v.trim_start_matches(|c: char| !c.is_ascii_digit()))
        .and_then(kaeri_core::books::ChapterNumber::parse)
        .map_or(i64::MAX, kaeri_core::books::ChapterNumber::milli)
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
        self.search_in(text, medium, None, now).await
    }

    /// [`Self::search`] asking only the sources in `scope`; `None` asks every source.
    pub async fn search_in(
        &self,
        text: &str,
        medium: Option<Medium>,
        scope: Option<&[String]>,
        now: i64,
    ) -> Vec<WorkTile> {
        let hints = BookHints::default();
        let found = self.registry.search_in(text, &hints, medium, scope, now).await;
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

    /// Why asking `source` failed, naming it as the person knows it. A source switched off
    /// says so, rather than the sign-in its refusal would otherwise read as.
    pub(crate) fn failure(&self, source: &str, error: &ProviderError) -> ReadingFailure {
        let name = self.source_name(source);
        if matches!(error, ProviderError::Unavailable(_)) && self.registry.is_off(source) {
            return ReadingFailure::SourceOff { source: name };
        }
        self.offline_or(failure(&name, error))
    }

    /// A source that could not be reached while the network says there is none is offline,
    /// whatever words the source chose for it.
    pub(crate) fn offline_or(&self, failure: ReadingFailure) -> ReadingFailure {
        match failure {
            ReadingFailure::SourceDown { .. } if self.write_back.is_offline() => {
                ReadingFailure::Offline
            }
            said => said,
        }
    }

    /// Keep where an edition's chapter-list ask stands; `None` once a list is kept.
    pub(crate) fn note_listing(
        &self,
        source: &str,
        series: &str,
        asked: Option<Asked>,
        now: i64,
    ) {
        let mark = self.source_mark(source);
        let mut kept = self.catalog.listings.lock().unwrap_or_else(|e| e.into_inner());
        let key = (source.to_owned(), series.to_owned());
        match asked {
            Some(asked) => kept.insert(key, (now, asked, mark)),
            None => kept.remove(&key),
        };
    }

    /// The last ask of an edition's chapter list that left no list kept, with when, unless the
    /// source joined anew since: what an update says is asked again.
    fn last_listing(&self, source: &str, series: &str) -> Option<(i64, Asked)> {
        let mark = self.source_mark(source);
        self.catalog
            .listings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(source.to_owned(), series.to_owned()))
            .filter(|(_, _, asked_of)| *asked_of == mark)
            .map(|(at, asked, _)| (*at, asked.clone()))
    }

    /// Where an edition's chapter list stands, for Read from's line.
    pub(crate) fn listing_of(&self, source: &str, series: &str) -> Option<EditionListing> {
        let asked = self.last_listing(source, series).map(|(_, asked)| asked);
        match asked {
            Some(Asked::Asking) => Some(EditionListing::Asking),
            Some(Asked::Empty) => Some(EditionListing::Empty),
            Some(Asked::Failed(failure)) => Some(EditionListing::Failed { failure }),
            None => crate::read_catalog::chapters::kept(&self.store, source, series)
                .filter(|(list, _)| !list.is_empty())
                .map(|_| EditionListing::Listed),
        }
    }

    /// When an edition's list was last asked for, if that ask left no list kept.
    pub(crate) fn listing_asked_at(&self, source: &str, series: &str) -> Option<i64> {
        self.last_listing(source, series).map(|(at, _)| at)
    }

    /// Which source and series reference a work is read from: the chosen edition or the
    /// person's preference, then a series the person confirmed, then the series whose chapters
    /// in the person's language come nearest the work's, then the person's order of sources. A
    /// one-shot, or a source with a fraction of the chapters, comes after one that has them.
    pub(crate) fn edition_of(
        &self,
        work: &WorkId,
        edition: Option<&str>,
    ) -> Option<(String, String)> {
        let refs = self.store.book_refs_for_work(work.as_str()).unwrap_or_default();
        let pref = self.store.edition_pref(work.as_str()).ok().flatten();
        let wanted =
            edition.map(str::to_owned).or_else(|| pref.as_ref().map(|p| p.source_id.clone()));
        let order = self.registry.ids();
        let mut held = refs;
        match wanted {
            // The person's pick waits while its source is not installed or is switched off:
            // the work reads from another edition until the source is back, and the pick is
            // kept for then. While plugins are loading it holds, as its plugin may be on the
            // way. An edition asked for by name is read from as it is.
            Some(wanted)
                if (edition.is_some() || self.may_be_available(&wanted))
                    && held.iter().any(|(source, _)| *source == wanted) =>
            {
                held.retain(|(source, _)| *source == wanted);
            }
            // Never a source switched off, nor one whose chapter list came back empty: with
            // nothing else held, nothing is read from rather than a page with no chapters.
            _ => held.retain(|(source, series)| {
                order.contains(source)
                    && !self.registry.is_off(source)
                    && !self.came_back_empty(source, series)
            }),
        }
        if held.len() <= 1 {
            return held.pop();
        }
        let language = pref
            .and_then(|p| p.language)
            .or_else(|| self.language.read().ok().and_then(|l| l.clone()));
        let total =
            self.store.book_work(work.as_str()).ok().flatten().and_then(|w| w.chapters_total);
        let fits = editions_fit(self, &held, total, language.as_deref());
        let position =
            |source: &str| order.iter().position(|id| id == source).unwrap_or(usize::MAX);
        held.into_iter()
            .zip(fits)
            .min_by_key(|((source, series), fit)| {
                let confirmed =
                    matches!(self.store.book_match_state(source, series), Ok(Some((_, true))));
                (!confirmed, fit.0, fit.1, position(source))
            })
            .map(|(held, _)| held)
    }

    /// Whether `source` can be asked: installed, and not switched off.
    pub(crate) fn is_available(&self, source: &str) -> bool {
        self.registry.get(source).is_some() && !self.registry.is_off(source)
    }

    /// Whether `source` may yet be asked: available, or not switched off while plugins are
    /// still loading, when it may be one of them.
    fn may_be_available(&self, source: &str) -> bool {
        let arriving = self
            .plugins
            .read()
            .ok()
            .and_then(|p| p.as_ref().map(BookPlugins::is_arriving))
            .unwrap_or(false);
        self.is_available(source) || (arriving && !self.registry.is_off(source))
    }

    /// While plugins are still loading: the edition a work is held in from a source not in
    /// yet, the person's preference first. Not one switched off.
    pub(crate) fn edition_not_in_yet(&self, work: &WorkId) -> Option<(String, String)> {
        let order = self.registry.ids();
        let mut held = self.store.book_refs_for_work(work.as_str()).unwrap_or_default();
        held.retain(|(source, _)| !order.contains(source) && !self.registry.is_off(source));
        let pref = self.store.edition_pref(work.as_str()).ok().flatten();
        let first = pref
            .and_then(|p| held.iter().position(|(source, _)| *source == p.source_id))
            .unwrap_or(0);
        (first < held.len()).then(|| held.swap_remove(first))
    }

    /// Note what a source's chapter list came back as: empty, or with chapters again.
    fn note_list(&self, source: &str, series: &str, chapters: &[SourceChapter]) {
        let mark = self.source_mark(source);
        let mut empty = self.empty.lock().unwrap_or_else(|e| e.into_inner());
        let key = (source.to_owned(), series.to_owned());
        if chapters.is_empty() {
            empty.insert(key, (std::time::Instant::now(), mark));
        } else {
            empty.remove(&key);
        }
    }

    /// The name of a switched-off source that holds `work`, in the person's order, for a work no
    /// source that is on holds: its page says the source is switched off, not that none has it.
    pub(crate) fn switched_off_holder(&self, work: &WorkId) -> Option<String> {
        let refs = self.store.book_refs_for_work(work.as_str()).unwrap_or_default();
        self.registry
            .ids()
            .into_iter()
            .filter(|id| self.registry.is_off(id))
            .find(|id| refs.iter().any(|(source, _)| source == id))
            .map(|id| self.source_name(&id))
    }

    /// The series' chapter list came back empty within the last quarter hour from the source
    /// as it stands, or was kept empty before.
    fn came_back_empty(&self, source: &str, series: &str) -> bool {
        const EMPTY_FOR: std::time::Duration = std::time::Duration::from_secs(15 * 60);
        let mark = self.source_mark(source);
        let noted = self.empty.lock().unwrap_or_else(|e| e.into_inner());
        noted
            .get(&(source.to_owned(), series.to_owned()))
            .is_some_and(|(at, said_by)| at.elapsed() < EMPTY_FOR && *said_by == mark)
            || crate::read_catalog::chapters::kept(&self.store, source, series)
                .is_some_and(|(list, _)| list.is_empty())
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

    /// The chapter lists of a work's series that are not kept yet, fetched and kept, so the
    /// edition read from is picked by what each source has. Only when nobody chose one, and
    /// each source is asked once at the same time as the others.
    async fn know_editions(&self, work: &WorkId, now: i64) {
        let chosen = self.store.edition_pref(work.as_str()).ok().flatten().is_some();
        let refs = self.store.book_refs_for_work(work.as_str()).unwrap_or_default();
        let order = self.registry.ids();
        if chosen || refs.len() < 2 {
            return;
        }
        let unknown: Vec<(String, String)> = refs
            .into_iter()
            .filter(|(source, series)| {
                order.contains(source)
                    && !self.registry.is_off(source)
                    && crate::read_catalog::editions::counts_chapters(self, source)
                    && crate::read_catalog::chapters::kept(&self.store, source, series)
                        .is_none()
            })
            .collect();
        let fetches = unknown.iter().map(|(source, series)| async move {
            if let Ok(list) =
                self.registry.chapters(source, series, &BookHints::default(), now).await
            {
                self.note_list(source, series, &list);
                crate::read_catalog::chapters::keep(&self.store, source, series, &list, now);
            }
        });
        futures::future::join_all(fetches).await;
    }

    /// A work's chapters from its edition, grouped by volume, with the person's reading marked.
    pub async fn chapters(
        &self,
        work: &WorkId,
        edition: Option<&str>,
        now: i64,
    ) -> Result<(Option<BookEditionRow>, Vec<VolumeGroup>), ReadingFailure> {
        if edition.is_none() {
            self.know_editions(work, now).await;
        }
        let (source, series) =
            self.edition_of(work, edition).ok_or(ReadingFailure::NotFound)?;
        // The source is asked how the series reads beside its chapters, and never held up for.
        self.ask_hint(&source, &series, now);
        let chapters = self.fetch_list(&source, &series, now).await?;
        let groups = self.groups_of(work, &source, &chapters);
        // The list just kept, counted as every screen counts it: one per chapter, its parts and
        // its groups' releases as one.
        let row = self.editions(work).into_iter().find(|e| e.source_id == source);
        Ok((row, groups))
    }

    /// The chapters the reader reads a work from, grouped and ordered as the series page lists
    /// them: the list the series page or an earlier open asked the source for, while it is
    /// [`LIST_FRESH`], so an open asks the source only for the chapter's content; otherwise
    /// the list asked again.
    pub(crate) async fn reading_chapters(
        &self,
        work: &WorkId,
        now: i64,
    ) -> Result<Vec<VolumeGroup>, ReadingFailure> {
        match self.kept_chapters(work, now, LIST_FRESH) {
            Some(groups) => Ok(groups),
            None => self.chapters(work, None, now).await.map(|(_, groups)| groups),
        }
    }

    /// As [`Self::reading_chapters`], from the list as kept alone, when it was asked within
    /// `max_age` seconds; `None` when there is none.
    pub(crate) fn kept_chapters(
        &self,
        work: &WorkId,
        now: i64,
        max_age: i64,
    ) -> Option<Vec<VolumeGroup>> {
        let (source, series) = self.edition_of(work, None)?;
        let chapters = self.kept_source_list(&source, &series, now, max_age)?;
        Some(self.groups_of(work, &source, &chapters))
    }

    /// As [`Self::chapters`], from the list as kept alone when it was asked within `max_age`
    /// seconds; `None` when there is none.
    pub(crate) fn kept_listing(
        &self,
        work: &WorkId,
        now: i64,
        max_age: i64,
    ) -> Option<(Option<BookEditionRow>, Vec<VolumeGroup>)> {
        let (source, series) = self.edition_of(work, None)?;
        let chapters = self.kept_source_list(&source, &series, now, max_age)?;
        let groups = self.groups_of(work, &source, &chapters);
        let row =
            self.editions(work).into_iter().find(|e| e.source_id == source).map(|mut e| {
                e.chapters = u32::try_from(chapters.len()).unwrap_or(u32::MAX);
                e
            });
        Some((row, groups))
    }

    /// Ask the source for a series' whole list, put it in reading order and keep it.
    async fn fetch_list(
        &self,
        source: &str,
        series: &str,
        now: i64,
    ) -> Result<Vec<SourceChapter>, ReadingFailure> {
        self.note_listing(source, series, Some(Asked::Asking), now);
        let asked = self.registry.chapters(source, series, &BookHints::default(), now).await;
        let mut chapters = match asked {
            Ok(chapters) => chapters,
            Err(e) => {
                let failure = self.failure(source, &e);
                self.note_listing(source, series, Some(Asked::Failed(failure.clone())), now);
                return Err(failure);
            }
        };
        let empty = chapters.is_empty().then_some(Asked::Empty);
        self.note_listing(source, series, empty, now);
        self.note_list(source, series, &chapters);
        oldest_first(&mut chapters);
        crate::read_catalog::chapters::keep(&self.store, source, series, &chapters, now);
        Ok(chapters)
    }

    /// A series' list as last kept, in reading order, when the source was asked for it within
    /// `max_age` seconds.
    fn kept_source_list(
        &self,
        source: &str,
        series: &str,
        now: i64,
        max_age: i64,
    ) -> Option<Vec<SourceChapter>> {
        let (json, at) = self.store.book_chapters(source, series).ok()??;
        if now.saturating_sub(at) > max_age {
            return None;
        }
        let kept: Vec<crate::read_catalog::chapters::Kept> =
            serde_json::from_str(&json).ok()?;
        let mut chapters: Vec<SourceChapter> =
            kept.iter().map(crate::read_catalog::chapters::Kept::source_chapter).collect();
        if chapters.is_empty() {
            return None;
        }
        oldest_first(&mut chapters);
        Some(chapters)
    }

    /// A work's chapters in `source`'s list grouped by volume, with the person's reading
    /// marked: one row per chapter, its parts as one, from the preferred group.
    fn groups_of(
        &self,
        work: &WorkId,
        source: &str,
        chapters: &[SourceChapter],
    ) -> Vec<VolumeGroup> {
        let chosen =
            crate::read_catalog::chapters::chosen_group(&self.store, work.as_str(), source);
        let language = self.language();
        let numbering =
            crate::read_catalog::chapters::numbering(&self.store, work.as_str(), None);
        let arranged = crate::read_catalog::arrange::arrange_in(
            chapters,
            chosen.as_deref(),
            language.as_deref(),
            &numbering,
        );
        // Read as Mark read up to here and sync count it, under a watermark, as well as one by
        // one, and parts read under their own numbers as their whole chapter.
        let (marks, places) =
            crate::read_catalog::chapters::read_state(&self.store, work.as_str());
        let read: HashMap<String, kaeri_store::reading::ReadChapter> =
            places.into_iter().map(|c| (c.chapter.clone(), c)).collect();
        let downloads = self.downloads.states(&self.store, work.as_str());
        let bookmarks: HashSet<String> =
            self.store.bookmarks(work.as_str()).unwrap_or_default().into_iter().collect();
        let mut rows: Vec<ChapterRow> = Vec::with_capacity(arranged.rows.len());
        // One row per chapter: its parts read as one, from the preferred group, or another
        // group where that one lacks it. Its row counts the other groups as versions.
        for arranged_row in &arranged.rows {
            let parts: Vec<&SourceChapter> =
                arranged_row.parts.iter().map(|&i| &chapters[i]).collect();
            let id = arranged_row.id.clone();
            let state = state_of(&marks, read.get(id.as_str()), id.as_str());
            let read_at = read.get(id.as_str()).and_then(|r| r.read_at);
            let download = downloads.get(id.as_str()).cloned();
            let bookmarked = bookmarks.contains(id.as_str());
            let row = source_row(
                &parts,
                id,
                state,
                read_at,
                download,
                bookmarked,
                (&arranged, arranged_row, language.as_deref()),
            );
            rows.push(row);
        }
        // Another source's chapters that what follows a chapter here stands for stay out, and
        // still say which volume the rows beside them are in.
        let mut held = Vec::new();
        keep_dropped(
            &self.store,
            (work, source),
            &read,
            &marks,
            &bookmarks,
            &mut rows,
            &mut held,
        );
        self.fill_from_others(work, source, (&read, &marks), &numbering, &mut rows, &mut held);
        tidy_filled(&mut rows);
        let known = crate::read_catalog::volumes::Known::of_work(&self.store, work.as_str());
        crate::read_catalog::volumes::group_beside(rows, held, &known, self.show_volumes())
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
        let (source, series, found, kept) =
            self.find_chapter(work, chapter, edition, now, true).await?;
        match self.open_found(&source, &series, found, now).await {
            Ok(opened) => Ok(opened),
            // The chapter as a kept list named it may have moved: the list is asked for again,
            // once, unless the source could not be reached at all.
            Err(e) if kept && !matches!(e, ProviderError::Transport(_)) => {
                tracing::debug!(error = %e, "a kept chapter did not open; listing again");
                let (source, series, found, _) =
                    self.find_chapter(work, chapter, edition, now, false).await?;
                self.open_found(&source, &series, found, now)
                    .await
                    .map_err(|e| self.failure(&source, &e))
            }
            Err(e) => Err(self.failure(&source, &e)),
        }
    }

    /// Open a chapter's entries in a source's list: its parts read on from the first with no
    /// break. A chapter whose first part is a whole file opens as that file.
    async fn open_found(
        &self,
        source: &str,
        series: &str,
        found: Found,
        now: i64,
    ) -> Result<Opened, ProviderError> {
        let Found { entries: mut found, numbers, total, missing } = found;
        // The group the chapter is read in: its first part's, as a part from another group is
        // named on its own.
        let group = found[0].version_label.clone();
        let first = found.remove(0);
        let mut numbers = numbers.into_iter();
        let first_number = numbers.next().unwrap_or(1);
        let content = self.registry.content(source, series, &first.id, now).await?;
        let mut opened = Opened {
            source_id: source.to_owned(),
            series_ref: series.to_owned(),
            chapter: first,
            content,
            parts: Vec::new(),
            group,
            total,
            missing,
        };
        if found.is_empty() {
            return Ok(opened);
        }
        let mut parts = Vec::new();
        let mut text = String::new();
        for (part, number) in found.into_iter().zip(numbers.chain(std::iter::repeat(0))) {
            let more = self.registry.content(source, series, &part.id, now).await?;
            match more {
                ChapterContent::Images(pages) => {
                    parts.push(OpenedPart { chapter: part, pages, number })
                }
                ChapterContent::Text(xhtml) => text.push_str(&xhtml),
                ChapterContent::Book(_) => {}
            }
        }
        match &mut opened.content {
            ChapterContent::Images(pages) => {
                let first = OpenedPart {
                    chapter: opened.chapter.clone(),
                    pages: pages.clone(),
                    number: first_number,
                };
                parts.insert(0, first);
                *pages = parts.iter().flat_map(|p| p.pages.iter().cloned()).collect();
                opened.parts = parts;
            }
            ChapterContent::Text(xhtml) => xhtml.push_str(&text),
            ChapterContent::Book(_) => {}
        }
        Ok(opened)
    }

    /// The source, its series and its own entries for `chapter` of `work`, in the edition, and
    /// whether a kept list named it: the chapter as the list shows it, from the preferred
    /// group, with its parts in order. With `kept`, a list the source gave within
    /// [`REF_FRESH`] that names the chapter is enough; otherwise, or when it does not, the
    /// source is asked. A chapter the edition lacks, asked for without naming an edition, is
    /// looked for in the person's other sources whose kept lists have it, as the series page
    /// lists it.
    async fn find_chapter(
        &self,
        work: &WorkId,
        chapter: &ChapterId,
        edition: Option<&str>,
        now: i64,
        kept: bool,
    ) -> Result<(String, String, Found, bool), ReadingFailure> {
        let (source, series) =
            self.edition_of(work, edition).ok_or(ReadingFailure::NotFound)?;
        let language = self.language();
        let numbering =
            crate::read_catalog::chapters::numbering(&self.store, work.as_str(), None);
        let find = |source: &str, chapters: Vec<SourceChapter>| {
            let chosen =
                crate::read_catalog::chapters::chosen_group(&self.store, work.as_str(), source);
            let arranged = crate::read_catalog::arrange::arrange_in(
                &chapters,
                chosen.as_deref(),
                language.as_deref(),
                &numbering,
            );
            let row = arranged.row(chapter)?;
            Some(Found {
                entries: row.parts.iter().map(|&i| chapters[i].clone()).collect(),
                numbers: row.numbers.clone(),
                total: row.total,
                missing: row.missing.clone(),
            })
        };
        if kept
            && let Some(found) = self
                .kept_source_list(&source, &series, now, REF_FRESH)
                .and_then(|list| find(&source, list))
        {
            return Ok((source, series, found, true));
        }
        let chapters = self.fetch_list(&source, &series, now).await?;
        if let Some(found) = find(&source, chapters) {
            return Ok((source, series, found, false));
        }
        if edition.is_some() {
            return Err(ReadingFailure::NotFound);
        }
        for (other, other_series) in
            self.store.book_refs_for_work(work.as_str()).unwrap_or_default()
        {
            if other == source || !self.is_available(&other) {
                continue;
            }
            let Some((list, _)) =
                crate::read_catalog::chapters::kept(&self.store, &other, &other_series)
            else {
                continue;
            };
            if !list.iter().any(|k| k.id == chapter.as_str()) {
                continue;
            }
            let Ok(chapters) = self.fetch_list(&other, &other_series, now).await else {
                continue;
            };
            if let Some(found) = find(&other, chapters) {
                return Ok((other, other_series, found, false));
            }
        }
        Err(ReadingFailure::NotFound)
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
        let Some(mut opened) = self.pictures(work, chapter, now).await? else {
            return Ok(None);
        };
        self.page(&mut opened, 0, now).await.map(Some)
    }

    /// A chapter's page images as its source names them, every part's in reading order,
    /// without downloading a whole book file: `None` for a chapter of text or one kept as a
    /// file.
    pub async fn pictures(
        &self,
        work: &WorkId,
        chapter: &ChapterId,
        now: i64,
    ) -> Result<Option<Opened>, ReadingFailure> {
        let (source, series, found, _) =
            self.find_chapter(work, chapter, None, now, true).await?;
        let Found { entries, numbers, total, missing } = found;
        let group = entries.first().and_then(|c| c.version_label.clone());
        let mut parts = Vec::new();
        let mut numbers = numbers.into_iter().chain(std::iter::repeat(0));
        for (at, entry) in entries.into_iter().enumerate() {
            let number = numbers.next().unwrap_or(0);
            let pages = self
                .registry
                .chapter_pages(&source, &series, &entry.id, now)
                .await
                .map_err(|e| self.failure(&source, &e))?;
            match pages.filter(|p| !p.is_empty()) {
                Some(pages) => parts.push(OpenedPart { chapter: entry, pages, number }),
                // A chapter whose first part is not pictures is left to its open.
                None if at == 0 => return Ok(None),
                None => {}
            }
        }
        let first = parts[0].chapter.clone();
        let pages: Vec<PageRef> = parts.iter().flat_map(|p| p.pages.iter().cloned()).collect();
        // A chapter listed whole reads its pages as the chapter's own.
        if parts.len() == 1 {
            parts.clear();
        }
        Ok(Some(Opened {
            source_id: source,
            series_ref: series,
            chapter: first,
            content: ChapterContent::Images(pages),
            parts,
            group,
            total,
            missing,
        }))
    }

    /// Fetch page `index` of an opened chapter's images, asking the source again once for
    /// addresses that have expired.
    pub async fn page(
        &self,
        opened: &mut Opened,
        index: usize,
        now: i64,
    ) -> Result<Vec<u8>, ReadingFailure> {
        // A part's page is fetched as that part's, so expired addresses are asked for again
        // of the part they belong to.
        let (chapter, pages, index) = if opened.parts.is_empty() {
            let ChapterContent::Images(pages) = &mut opened.content else {
                return Err(ReadingFailure::NotFound);
            };
            (&opened.chapter.id, pages, index)
        } else {
            let mut index = index;
            let part = opened
                .parts
                .iter_mut()
                .find(|p| {
                    let here = index < p.pages.len();
                    if !here {
                        index -= p.pages.len();
                    }
                    here
                })
                .ok_or(ReadingFailure::NotFound)?;
            (&part.chapter.id, &mut part.pages, index)
        };
        let at = kaeri_providers::ChapterAt {
            source: &opened.source_id,
            series: &opened.series_ref,
            chapter,
        };
        self.registry
            .fetch_page(&at, pages, index, now, |page: PageRef| {
                let client = self.client.clone();
                async move { plain_fetch(&client, &page).await }
            })
            .await
            .map_err(|e| self.failure(&opened.source_id, &e))
    }
}

impl BookSources {
    /// Chapters the edition does not list that the person's other sources for the work do, as
    /// last asked: listed in their place, marked with the source they come from, so the list
    /// never hides a chapter another source has. Opening one reads it from that source. What
    /// follows a chapter here stands for what follows it there, as
    /// [`crate::read_catalog::extras`] says: those rows go to `held` instead.
    fn fill_from_others(
        &self,
        work: &WorkId,
        source: &str,
        (read, marks): (
            &HashMap<String, kaeri_store::reading::ReadChapter>,
            &kaeri_store::reading::ReadMarks,
        ),
        numbering: &crate::read_catalog::arrange::Numbering,
        rows: &mut Vec<ChapterRow>,
        held: &mut Vec<ChapterRow>,
    ) {
        use crate::read_catalog::extras;
        let mut listed: HashSet<String> =
            rows.iter().map(|c| c.id.as_str().to_owned()).collect();
        let mut here = extras::Following::of(rows.iter().map(|c| (&c.id, c.title.as_deref())));
        let refs = self.store.book_refs_for_work(work.as_str()).unwrap_or_default();
        // A source not installed or switched off can't open what it listed, so its rows stay
        // out.
        let others = refs.into_iter().filter(|(s, _)| s != source && self.is_available(s));
        for (other, series) in others {
            let Some((list, _)) =
                crate::read_catalog::chapters::kept(&self.store, &other, &series)
            else {
                continue;
            };
            let entries: Vec<SourceChapter> = list.iter().map(|k| k.source_chapter()).collect();
            let chosen =
                crate::read_catalog::chapters::chosen_group(&self.store, work.as_str(), &other);
            let language = self.language();
            // Compared after parts merge, with the work's numbering: "7.1" and "7.2" there are
            // the 7 listed here, never a chapter of their own beside it.
            let arranged = crate::read_catalog::arrange::arrange_in(
                &entries,
                chosen.as_deref(),
                language.as_deref(),
                numbering,
            );
            let name = self.source_name(&other);
            let after = extras::follows(arranged.rows.iter().map(|r| &r.id));
            let titles = extras::titles_chapters(
                arranged.rows.iter().map(|r| (&r.id, entries[r.parts[0]].title.as_deref())),
            );
            for (row, after) in arranged.rows.iter().zip(after) {
                if listed.contains(row.id.as_str()) {
                    continue;
                }
                // The kept list's ids are the ones its source lists; arranged again they match.
                let parts: Vec<&SourceChapter> =
                    row.parts.iter().map(|&i| &entries[i]).collect();
                let was = read.get(row.id.as_str());
                let state = state_of(marks, was, row.id.as_str());
                let mut chapter_row = source_row(
                    &parts,
                    row.id.clone(),
                    state,
                    was.and_then(|r| r.read_at),
                    None,
                    false,
                    (&arranged, row, language.as_deref()),
                );
                chapter_row.other_group = false;
                chapter_row.group_language = None;
                chapter_row.other_source = Some(name.clone());
                let title = chapter_row.title.as_deref();
                if let Some(after) = after {
                    if here.stands_for(&after, title, titles) {
                        held.push(chapter_row);
                        continue;
                    }
                    here.add(after, title);
                }
                listed.insert(row.id.as_str().to_owned());
                let at = place_of(rows, &chapter_row);
                rows.insert(at, chapter_row);
            }
        }
    }
}

/// A chapter row's read state from the marks and its kept place.
fn state_of(
    marks: &kaeri_store::reading::ReadMarks,
    place: Option<&kaeri_store::reading::ReadChapter>,
    id: &str,
) -> ChapterReadState {
    match place {
        _ if marks.is_read(id) => ChapterReadState::Read,
        Some(r) if r.read => ChapterReadState::Read,
        Some(r) => match r.last_page {
            Some(page) => ChapterReadState::InProgress {
                page: kaeri_model::reading::shown_page(page),
                of: r.of_pages,
            },
            None => ChapterReadState::Unread,
        },
        None => ChapterReadState::Unread,
    }
}

/// Chapters downloaded here that the source no longer lists stay on the series page, marked as
/// gone from the source, in their place among the others: the file is still on the device and
/// still opens. One downloaded from another source that what follows a chapter here stands for,
/// as [`crate::read_catalog::extras`] says, goes to `held` instead.
fn keep_dropped(
    store: &Store,
    (work, source): (&WorkId, &str),
    read: &HashMap<String, kaeri_store::reading::ReadChapter>,
    marks: &kaeri_store::reading::ReadMarks,
    bookmarks: &HashSet<String>,
    rows: &mut Vec<ChapterRow>,
    held: &mut Vec<ChapterRow>,
) {
    use crate::read_catalog::extras;
    let listed: HashSet<String> = rows.iter().map(|c| c.id.as_str().to_owned()).collect();
    let here = extras::Following::of(rows.iter().map(|c| (&c.id, c.title.as_deref())));
    // A download of a part now read as its whole chapter ("7.1" before "7.2" came) is that
    // chapter's: the list has it already.
    let folds = crate::read_catalog::chapters::Folds::of_work(store, work.as_str());
    let downloads = store.book_downloads(Some(work.as_str())).unwrap_or_default();
    for d in downloads.iter().filter(|d| {
        d.state == kaeri_store::book_downloads::BookDownloadState::Done
            && d.path.as_deref().is_some_and(|p| std::path::Path::new(p).exists())
            && !listed.contains(&d.chapter)
            && !folds.is_part(&d.chapter)
    }) {
        let was = read.get(&d.chapter);
        let done = marks.is_read(&d.chapter);
        let mut row =
            crate::downloads::books::chapter_row(d, done, bookmarks.contains(&d.chapter));
        if let Some(r) = was {
            row.read_at = r.read_at;
            if !done && let Some(page) = r.last_page {
                row.state = ChapterReadState::InProgress {
                    page: kaeri_model::reading::shown_page(page),
                    of: r.of_pages,
                };
            }
        }
        row.gone_from_source = true;
        let after = extras::follows([&row.id]).pop().flatten();
        if d.source_id != source && after.is_some_and(|a| here.stands_for(&a, None, false)) {
            held.push(row);
            continue;
        }
        let at = place_of(rows, &row);
        rows.insert(at, row);
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

/// The rows filled in from other sources, made to read as the list in use: a numbered row is
/// labelled by its number, not the other source's own words for it ("Ferry 21.5" reads
/// "21.5"), and a one-shot the list in use has without a number is not listed a second time
/// from another source. Only another source's row named a one-shot with no number or number 0
/// goes; a numbered chapter that happens to carry the name stays.
fn tidy_filled(rows: &mut Vec<ChapterRow>) {
    let one_shot_here =
        rows.iter().any(|r| r.other_source.is_none() && r.number.is_none() && one_shot(r));
    if one_shot_here {
        rows.retain(|r| {
            r.other_source.is_none()
                || !one_shot(r)
                || r.number
                    .as_deref()
                    .is_some_and(|n| !n.parse::<f64>().is_ok_and(|n| n == 0.0))
        });
    }
    for row in rows.iter_mut().filter(|r| r.other_source.is_some()) {
        if let Some(number) = &row.number
            && row.label.chars().any(char::is_alphabetic)
        {
            row.label = number.clone();
        }
    }
}

/// Whether a row's label or title names it a one-shot, written in any of the usual ways.
fn one_shot(row: &ChapterRow) -> bool {
    [Some(row.label.as_str()), row.title.as_deref()].into_iter().flatten().any(|text| {
        let folded: String =
            text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
        folded == "oneshot"
    })
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

#[cfg(test)]
mod filled_tests {
    use kaeri_model::reading::ChapterReadState;

    use super::*;

    fn row(id: &str, label: &str, title: Option<&str>, from: Option<&str>) -> ChapterRow {
        let number = ChapterId::new(id).key().and_then(|k| k.range()).map(|r| r.to_string());
        ChapterRow {
            id: ChapterId::new(id),
            label: label.into(),
            state: ChapterReadState::Unread,
            number,
            title: title.map(str::to_owned),
            volume: None,
            published: None,
            pages: None,
            new_chapter: false,
            download: None,
            versions: 1,
            bookmarked: false,
            version_label: None,
            read_at: None,
            line: Vec::new(),
            gone_from_source: false,
            parts: 1,
            missing_parts: Vec::new(),
            part_groups: Vec::new(),
            other_group: false,
            group_language: None,
            other_source: from.map(str::to_owned),
        }
    }

    fn labels(rows: &[ChapterRow]) -> Vec<(&str, &str)> {
        rows.iter().map(|r| (r.id.as_str(), r.label.as_str())).collect()
    }

    /// A row from another source reads by its number, as the list's own rows do; its title
    /// and the list's own words stay as they are.
    #[test]
    fn a_row_from_another_source_is_labelled_by_its_number() {
        let mut rows = vec![
            row("21", "21", None, None),
            row("21.5", "Ferry 21.5", None, Some("Other shelf")),
            row("22", "Ch. 22", Some("The Pier"), None),
            row("x/-/extra", "Extra", None, Some("Other shelf")),
        ];
        tidy_filled(&mut rows);
        assert_eq!(
            labels(&rows),
            [("21", "21"), ("21.5", "21.5"), ("22", "Ch. 22"), ("x/-/extra", "Extra")]
        );
    }

    /// A one-shot listed here without a number is the other source's chapter 0 named a
    /// one-shot: it shows once. A numbered chapter of that name stays, and without a one-shot
    /// here, the other source's stays.
    #[test]
    fn a_one_shot_listed_here_is_not_listed_again_from_another_source() {
        let filled = |rows: &mut Vec<ChapterRow>| {
            rows.push(row("0", "0", Some("One shot"), Some("Other shelf")));
            rows.push(row("1", "1", Some("A Quiet Harbour"), Some("Other shelf")));
            rows.push(row("7", "7", Some("Oneshot"), Some("Other shelf")));
            tidy_filled(rows);
            rows.iter().map(|r| r.id.as_str().to_owned()).collect::<Vec<_>>()
        };
        let mut here = vec![row("x/-/oneshot", "Oneshot", None, None)];
        assert_eq!(filled(&mut here), ["x/-/oneshot", "1", "7"]);
        let mut none = vec![row("x/-/prologue", "Prologue", None, None)];
        assert_eq!(filled(&mut none), ["x/-/prologue", "0", "1", "7"]);
    }
}
