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
        BookFile, BookHints, BookSourceKind, BookSourceManifest, ChapterContent, SeriesDetail,
        SeriesHit, SourceChapter, WorkKey, normalise_name,
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

/// The id the folders answer to in the registry.
pub const FOLDERS: &str = "folders";
const META_ORDER: &str = "reading.book_order";
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

impl FolderSource {
    pub fn new(store: Store) -> Self {
        Self {
            manifest: BookSourceManifest {
                id: FOLDERS.into(),
                name: "Folders".into(),
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
            .map(|s| SeriesHit::new(s.id, s.name))
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
    /// What reading's catalogue keeps between commands: see [`crate::read_catalog::Memory`].
    pub catalog: crate::read_catalog::Memory,
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
            catalog: crate::read_catalog::Memory::default(),
        };
        this.registry.put(Arc::new(FolderSource::new(this.store.clone())));
        this.restore();
        this
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

    fn put_server(&self, server: &BookServer) {
        let Some(kind) = Kind::parse(&server.kind) else { return };
        let Ok(credential) = self.tokens.get(&secret_key(&server.id)) else {
            let _ =
                self.store.put_book_server(&BookServer { signed_out: true, ..server.clone() });
            return;
        };
        let flag = self.flag(&server.id);
        flag.store(server.signed_out, Ordering::Relaxed);
        let built = kaeri_bookserver::source(
            &self.client,
            &Server {
                id: &server.id,
                name: &server.name,
                kind,
                address: &server.address,
                credential: &credential,
            },
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
                BookFolderRow {
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
                Some(BookServerRow {
                    kind: kind_of(Kind::parse(&s.kind)?),
                    off: self.registry.is_off(&s.id),
                    id: s.id,
                    name: s.name,
                    address: s.address,
                    series: 0,
                    report_progress: s.report_progress,
                    signed_out: s.signed_out,
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
    ) {
        let Some(path) = folder.path.clone().map(PathBuf::from) else {
            let _ = self.store.set_book_folder_available(&folder.id, false);
            return;
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
            }
            Ok(Err(e)) => {
                tracing::info!(error = %e, "a book folder could not be read");
                let _ = self.store.set_book_folder_available(&folder.id, false);
            }
            Err(e) => tracing::warn!(error = %e, "the folder scan stopped"),
        }
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
    match command {
        BookSourceCommand::Load => publish(),
        BookSourceCommand::AddFolder { folder } => {
            let Some(id) = this.add_folder(&folder, now) else { return publish() };
            publish();
            rescan(this, Some(id), now, publish);
        }
        BookSourceCommand::Scan => rescan(this, None, now, publish),
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
            tokio::spawn(async move {
                let event = match kaeri_bookserver::check(&this.client, &address).await {
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
            publish();
        }
        BookSourceCommand::SetReportProgress { id, on } => {
            if let Ok(Some(server)) = this.store.book_server(&id) {
                let _ =
                    this.store.put_book_server(&BookServer { report_progress: on, ..server });
            }
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
        BookSourceCommand::LoadForWork { work } => {
            let _ = tx.send(Event::BookSources(BookSourceEvent::ForWork {
                editions: this.editions(&work),
                work,
                asking: false,
            }));
        }
    }
}

fn rescan(
    this: BookSources,
    only: Option<String>,
    now: i64,
    publish: impl Fn() + Clone + Send + 'static,
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
            this.scan_folder(folder, now, tick).await;
            publish();
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
    /// The editions a work has: every source holding a series matched to it.
    pub(crate) fn editions(
        &self,
        work: &kaeri_core::books::WorkId,
    ) -> Vec<kaeri_model::reading::BookEditionRow> {
        let preferred = self.store.edition_pref(work.as_str()).ok().flatten();
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
            let manifest = self.registry.manifest(&source);
            let default = manifest
                .as_ref()
                .and_then(|m| m.media.iter().copied().find(|m| m.is_reading()))
                .unwrap_or(Medium::Manga);
            for hit in hits {
                let folder = (source == FOLDERS)
                    .then(|| local.iter().find(|s| s.id == hit.id))
                    .flatten();
                let medium =
                    folder.and_then(|s| Medium::parse(&s.medium)).or(medium).unwrap_or(default);
                let work = match self.store.book_match(&source, &hit.id) {
                    Ok(Some(work)) => work,
                    _ => {
                        let work = WorkKey::local(medium, &hit.title).to_string();
                        let _ = self
                            .store
                            .set_book_match(&source, &hit.id, &work, 50, false, false, now);
                        work
                    }
                };
                if !seen.insert(work.clone()) {
                    continue;
                }
                tiles.push(WorkTile {
                    work: WorkId::new(work),
                    title: hit.title,
                    medium,
                    cover: hit.cover,
                    format_label: None,
                    progress: None,
                    next: None,
                    new_chapter: false,
                    adult: false,
                    facts: Vec::new(),
                });
            }
        }
        tiles
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

    /// A work's chapters from its edition, grouped by volume, with the person's reading marked.
    pub async fn chapters(
        &self,
        work: &WorkId,
        edition: Option<&str>,
        now: i64,
    ) -> Result<(Option<BookEditionRow>, Vec<VolumeGroup>), ReadingFailure> {
        let (source, series) =
            self.edition_of(work, edition).ok_or(ReadingFailure::NotFound)?;
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
        let mut groups: Vec<VolumeGroup> = Vec::new();
        for chapter in &chapters {
            let id = chapter.key(restarts).id();
            let state = match read.get(id.as_str()) {
                Some(r) if r.read => ChapterReadState::Read,
                Some(r) => match r.last_page {
                    Some(page) => ChapterReadState::InProgress { page, of: r.of_pages },
                    None => ChapterReadState::Unread,
                },
                None => ChapterReadState::Unread,
            };
            let read_at = read.get(id.as_str()).and_then(|r| r.read_at);
            let row = ChapterRow {
                id,
                label: chapter.number_raw.clone(),
                state,
                number: chapter.number.map(|n| n.to_string()),
                title: chapter.title.clone(),
                volume: chapter.volume.clone(),
                published: chapter.published,
                pages: chapter.page_count,
                new_chapter: false,
                download: None,
                versions: 1,
                bookmarked: false,
                version_label: chapter.version_label.clone(),
                read_at,
            };
            match groups.iter_mut().find(|g| g.volume == chapter.volume) {
                Some(group) => group.chapters.push(row),
                None => groups.push(VolumeGroup {
                    volume: chapter.volume.clone(),
                    title: None,
                    chapters: vec![row],
                    all_read: false,
                }),
            }
        }
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
        let content = self
            .registry
            .content(&source, &series, &found.id, now)
            .await
            .map_err(|e| failure(&source, &e))?;
        Ok(Opened { source_id: source, series_ref: series, chapter: found, content })
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

    /// Tell the source a chapter was read or taken back, when the person allows it for that
    /// server. A failure is for the log: the mark itself is kept.
    pub fn report(&self, work: &WorkId, chapters: Vec<ChapterId>, read: bool, now: i64) {
        let this = self.clone();
        let work = work.clone();
        tokio::spawn(async move {
            let Some((source, series)) = this.edition_of(&work, None) else { return };
            let allowed = this
                .store
                .book_server(&source)
                .ok()
                .flatten()
                .is_some_and(|s| s.report_progress);
            if !allowed {
                return;
            }
            let Ok(listed) =
                this.registry.chapters(&source, &series, &BookHints::default(), now).await
            else {
                return;
            };
            let restarts = restarts_per_volume(&listed);
            for chapter in listed.iter().filter(|c| chapters.contains(&c.key(restarts).id())) {
                if let Err(e) = this
                    .registry
                    .report_progress(&source, &series, &chapter.id, read, now)
                    .await
                {
                    tracing::info!(error = %e, "a server was not told what was read");
                }
            }
        });
    }
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
