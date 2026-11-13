//! Book sources: components built for `wit/kaeri-books.wit`.
//!
//! The same host as show sources, for a second world. A book plugin gets the same sandbox: the
//! same empty WASI floor, the same memory ceiling and epoch deadline, a fresh store per call, and
//! the same five lent functions with the same allowlist behind `fetch`. Only the exported
//! interface differs, so this module is the bindings for it, the host side of its own copy of
//! `host`, and a loaded plugin's calls.
//!
//! A page image is fetched by the host, never by the guest: [`LoadedBookPlugin::fetch_page`]
//! does that with the plugin's own `allowed-hosts`, outside any guest call. A plugin built for the
//! optional `book-plugin-with-pages` world may make pages itself as well. The host tells it from
//! its exports at load ([`LoadedBookPlugin::makes_pages`]), asks it with
//! [`LoadedBookPlugin::page_bytes`], a guest call like any other, and checks the bytes it hands
//! back before keeping them: [`check_page`]. A plugin built for `book-plugin` is never asked.

use std::path::Path;

use crate::{
    engine::{PluginError, PluginHost, PluginKind, PluginState, new_store},
    host::{self, Capabilities},
    sandbox::Limits,
};

wasmtime::component::bindgen!({
    path: "../../wit/kaeri-books.wit",
    world: "book-plugin",
    // As for show sources: async on both sides, and host functions may trap with an error
    // rather than panic. See `engine.rs`.
    imports: { default: async | trappable },
    exports: { default: async },
});

pub use self::exports::kaeri::books::source::{
    Chapter, Content, Manifest, Page, SeriesDetail, SeriesHit, SourceError, TitleHints,
};
use self::kaeri::books::host::{HostError as GuestHostError, HttpRequest, HttpResponse};

/// The optional world: `book-plugin` and the `pages` export, for a plugin that makes some pages
/// itself. Its `host` is the one above, so one linker serves both worlds.
mod with_pages {
    wasmtime::component::bindgen!({
        path: "../../wit/kaeri-books.wit",
        world: "book-plugin-with-pages",
        imports: { default: async | trappable },
        exports: { default: async },
        with: { "kaeri:books/host": super::kaeri::books::host },
    });
}

/// The export a plugin built for `book-plugin-with-pages` has and one built for `book-plugin`
/// does not.
const PAGES_EXPORT: &str = "kaeri:books/pages@";

/// The major version of the book source ABI this host speaks: the major of the WIT package in
/// `wit/kaeri-books.wit`. It moves apart from [`crate::engine::ABI_MAJOR`].
pub const BOOKS_ABI_MAJOR: u64 = 1;

/// The largest page image a plugin may hand over from `page-bytes`. Twice the ceiling on one
/// fetched response, for a page a plugin assembled and encoded itself; a quarter of the guest's
/// default memory, which has to hold the page while it is made.
pub const MAX_PAGE_BYTES: usize = 16 * 1024 * 1024;

/// The most pixels a plugin's page may claim in its header: the reader's own decode cap, so a
/// page is refused here rather than after it is kept.
pub const MAX_PAGE_PIXELS: u64 = 40_000_000;

/// Whether bytes a plugin made are a page the host will keep: under [`MAX_PAGE_BYTES`], an image
/// format the reader knows by its first bytes (JPEG, PNG, WebP, GIF or AVIF), and a header that
/// claims at most [`MAX_PAGE_PIXELS`]. Only the header is read, so a page that would decompress
/// to gigabytes is refused before anything is decoded. The reason is for the log and the
/// source's health, as a `parse` error.
pub fn check_page(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_PAGE_BYTES {
        return Err(format!(
            "the page is {} bytes, over the {MAX_PAGE_BYTES} byte limit",
            bytes.len()
        ));
    }
    measure(bytes)
}

#[cfg(feature = "fetch")]
fn measure(bytes: &[u8]) -> Result<(), String> {
    if kaeri_pages::SourceFormat::sniff(bytes).is_none() {
        return Err("the page is not an image Kaeri shows".into());
    }
    let info = kaeri_pages::inspect(bytes).map_err(|e| format!("the page has no size: {e}"))?;
    let pixels = u64::from(info.width) * u64::from(info.height);
    if info.width == 0 || info.height == 0 {
        return Err("the page has no size".into());
    }
    if pixels > MAX_PAGE_PIXELS {
        return Err(format!(
            "the page is {}x{}, over the {MAX_PAGE_PIXELS} pixel limit",
            info.width, info.height
        ));
    }
    Ok(())
}

/// A host built without `fetch` reads no pages: it has nothing to show them with.
#[cfg(not(feature = "fetch"))]
fn measure(_: &[u8]) -> Result<(), String> {
    Err("this build of Kaeri reads no pages".into())
}

/// Ids a book plugin may not take: Kaeri's own book sources, and the demo.
const RESERVED: &[&str] = &["folders", "servers"];

fn refuse_reserved(id: &str) -> Result<(), String> {
    let trimmed = id.trim().to_ascii_lowercase();
    if kaeri_core::demo::reserved(id) {
        Err(format!("the id {id:?} is reserved for the built-in demo source"))
    } else if RESERVED.contains(&trimmed.as_str()) {
        Err(format!("the id {id:?} is the name of one of Kaeri's own book sources"))
    } else {
        Ok(())
    }
}

/// Link this world's `host` into `linker`.
pub(crate) fn add_to_linker(
    linker: &mut wasmtime::component::Linker<PluginState>,
) -> wasmtime::Result<()> {
    BookPlugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(linker, |state| state)
}

/// One authorised fetch, in this world's error vocabulary.
async fn one_fetch(
    capabilities: &Capabilities,
    request: HttpRequest,
) -> Result<HttpResponse, GuestHostError> {
    let outbound = host::Request {
        method: request.method,
        url: request.url,
        headers: request.headers,
        body: request.body,
    };
    match capabilities.fetch(outbound).await {
        Ok(response) => Ok(HttpResponse {
            status: response.status,
            headers: response.headers,
            body: response.body,
        }),
        Err(host::HostError::Denied(m)) => Err(GuestHostError::Denied(m)),
        Err(host::HostError::Timeout) => Err(GuestHostError::Timeout),
        Err(host::HostError::Transport(m)) => Err(GuestHostError::Transport(m)),
    }
}

// The book world's copy of `host`: the same five functions as `kaeri:provider/host`, served the
// same way.
impl kaeri::books::host::Host for PluginState {
    async fn fetch(
        &mut self,
        request: HttpRequest,
    ) -> wasmtime::Result<Result<HttpResponse, GuestHostError>> {
        Ok(one_fetch(&self.capabilities, request).await)
    }

    async fn fetch_many(
        &mut self,
        requests: Vec<HttpRequest>,
    ) -> wasmtime::Result<Vec<Result<HttpResponse, GuestHostError>>> {
        Ok(futures::future::join_all(
            requests.into_iter().map(|request| one_fetch(&self.capabilities, request)),
        )
        .await)
    }

    async fn log(&mut self, level: String, msg: String) -> wasmtime::Result<()> {
        let plugin = self.capabilities.plugin_id();
        match level.to_ascii_lowercase().as_str() {
            "error" => tracing::error!(plugin, "{msg}"),
            "warn" => tracing::warn!(plugin, "{msg}"),
            "debug" | "trace" => tracing::debug!(plugin, "{msg}"),
            _ => tracing::info!(plugin, "{msg}"),
        }
        Ok(())
    }

    async fn regex_captures(
        &mut self,
        pattern: String,
        haystack: String,
    ) -> wasmtime::Result<Vec<Vec<String>>> {
        Ok(host::regex_captures(&pattern, &haystack))
    }

    async fn config_get(&mut self, key: String) -> wasmtime::Result<Option<String>> {
        Ok(self.capabilities.setting(&key))
    }
}

/// A compiled book plugin, ready to call. Every call gets a fresh store, as for show sources.
pub struct LoadedBookPlugin {
    engine: wasmtime::Engine,
    linker: wasmtime::component::Linker<PluginState>,
    component: wasmtime::component::Component,
    manifest: Manifest,
    limits: Limits,
    http: Option<crate::HttpClient>,
    settings: std::collections::BTreeMap<String, String>,
    /// Built for `book-plugin-with-pages`.
    makes_pages: bool,
}

impl std::fmt::Debug for LoadedBookPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedBookPlugin")
            .field("id", &self.manifest.id)
            .field("version", &self.manifest.version)
            .field("allowed_hosts", &self.manifest.allowed_hosts)
            .field("limits", &self.limits)
            .field("makes_pages", &self.makes_pages)
            .finish_non_exhaustive()
    }
}

impl LoadedBookPlugin {
    /// Link a compiled book component, and ask it to describe itself with no capabilities.
    pub(crate) async fn finish(
        host: &PluginHost,
        path: &Path,
        component: wasmtime::component::Component,
    ) -> Result<Self, PluginError> {
        let display = path.display().to_string();
        let linker = host.linker(&display, PluginKind::Books)?;
        let makes_pages = component
            .component_type()
            .exports(host.engine())
            .any(|(name, _)| name.starts_with(PAGES_EXPORT));
        let plugin = Self {
            engine: host.engine().clone(),
            linker,
            component,
            manifest: Manifest {
                id: String::new(),
                display_name: String::new(),
                version: String::new(),
                allowed_hosts: Vec::new(),
                media: Vec::new(),
                languages: Vec::new(),
                capabilities: Vec::new(),
            },
            limits: host.limits(),
            http: host.http(),
            settings: Default::default(),
            makes_pages,
        };
        // As for show sources: describing itself is the one call made with no capabilities.
        let manifest = plugin
            .describe_with(Capabilities::new(display.clone(), Vec::new(), plugin.limits, None))
            .await
            .map_err(|e| PluginError::Load { path: display.clone(), message: e.to_string() })?;
        host.check_id(path, &manifest.id, refuse_reserved)?;
        tracing::info!(
            id = %manifest.id,
            version = %manifest.version,
            hosts = ?manifest.allowed_hosts,
            media = ?manifest.media,
            makes_pages,
            "book plugin loaded"
        );
        let settings = host.settings_for(path, &manifest.id);
        Ok(Self { manifest, settings, ..plugin })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Whether the plugin was built for `book-plugin-with-pages`, and so may make pages itself.
    pub fn makes_pages(&self) -> bool {
        self.makes_pages
    }

    /// The capabilities one call is lent: made fresh per call, so its fetch count starts at nought.
    pub(crate) fn capabilities(&self) -> Capabilities {
        Capabilities::new(
            self.manifest.id.clone(),
            self.manifest.allowed_hosts.clone(),
            self.limits,
            self.http.clone(),
        )
        .with_settings(self.settings.clone())
    }

    fn deadline_error(&self, error: &wasmtime::Error) -> PluginError {
        PluginError::from_call(&self.manifest.id, self.limits.deadline, error)
    }

    async fn instantiate(
        &self,
        store: &mut wasmtime::Store<PluginState>,
    ) -> Result<BookPlugin, PluginError> {
        BookPlugin::instantiate_async(store, &self.component, &self.linker)
            .await
            .map_err(|e| self.deadline_error(&e))
    }

    async fn describe_with(&self, capabilities: Capabilities) -> Result<Manifest, PluginError> {
        let mut store = new_store(&self.engine, self.limits, capabilities);
        let instance = self.instantiate(&mut store).await?;
        instance
            .kaeri_books_source()
            .call_describe(&mut store)
            .await
            .map_err(|e| PluginError::from_call("plugin", self.limits.deadline, &e))
    }

    /// Re-read the manifest, with no capabilities, as at load time.
    pub async fn describe(&self) -> Result<Manifest, PluginError> {
        self.describe_with(Capabilities::new(
            self.manifest.id.clone(),
            Vec::new(),
            self.limits,
            None,
        ))
        .await
    }

    pub async fn search(
        &self,
        query: &str,
        hints: &TitleHints,
    ) -> Result<Result<Vec<SeriesHit>, SourceError>, PluginError> {
        self.search_with(self.capabilities(), query, hints).await
    }

    /// [`Self::search`] under `capabilities`, kept by the caller to read the call's reach.
    pub(crate) async fn search_with(
        &self,
        capabilities: Capabilities,
        query: &str,
        hints: &TitleHints,
    ) -> Result<Result<Vec<SeriesHit>, SourceError>, PluginError> {
        let mut store = new_store(&self.engine, self.limits, capabilities);
        let instance = self.instantiate(&mut store).await?;
        instance
            .kaeri_books_source()
            .call_search(&mut store, query, hints)
            .await
            .map_err(|e| self.deadline_error(&e))
    }

    pub async fn series(
        &self,
        id: &str,
        hints: &TitleHints,
    ) -> Result<Result<SeriesDetail, SourceError>, PluginError> {
        self.series_with(self.capabilities(), id, hints).await
    }

    /// [`Self::series`] under `capabilities`, kept by the caller to read the call's reach.
    pub(crate) async fn series_with(
        &self,
        capabilities: Capabilities,
        id: &str,
        hints: &TitleHints,
    ) -> Result<Result<SeriesDetail, SourceError>, PluginError> {
        let mut store = new_store(&self.engine, self.limits, capabilities);
        let instance = self.instantiate(&mut store).await?;
        instance
            .kaeri_books_source()
            .call_series(&mut store, id, hints)
            .await
            .map_err(|e| self.deadline_error(&e))
    }

    pub async fn list_chapters(
        &self,
        id: &str,
        hints: &TitleHints,
    ) -> Result<Result<Vec<Chapter>, SourceError>, PluginError> {
        self.list_chapters_with(self.capabilities(), id, hints).await
    }

    /// [`Self::list_chapters`] under `capabilities`, kept by the caller to read the call's reach.
    pub(crate) async fn list_chapters_with(
        &self,
        capabilities: Capabilities,
        id: &str,
        hints: &TitleHints,
    ) -> Result<Result<Vec<Chapter>, SourceError>, PluginError> {
        let mut store = new_store(&self.engine, self.limits, capabilities);
        let instance = self.instantiate(&mut store).await?;
        instance
            .kaeri_books_source()
            .call_list_chapters(&mut store, id, hints)
            .await
            .map_err(|e| self.deadline_error(&e))
    }

    pub async fn chapter_content(
        &self,
        series: &str,
        chapter: &str,
    ) -> Result<Result<Content, SourceError>, PluginError> {
        self.chapter_content_with(self.capabilities(), series, chapter).await
    }

    /// [`Self::chapter_content`] under `capabilities`, kept by the caller to read the call's reach.
    pub(crate) async fn chapter_content_with(
        &self,
        capabilities: Capabilities,
        series: &str,
        chapter: &str,
    ) -> Result<Result<Content, SourceError>, PluginError> {
        let mut store = new_store(&self.engine, self.limits, capabilities);
        let instance = self.instantiate(&mut store).await?;
        instance
            .kaeri_books_source()
            .call_chapter_content(&mut store, series, chapter)
            .await
            .map_err(|e| self.deadline_error(&e))
    }

    /// The bytes of the page at `url` of `chapter`, if the plugin makes that page itself; `None`
    /// when the host is to fetch it. Asked only of a plugin that [makes pages](Self::makes_pages),
    /// under the same limits as every other call, and checked by [`check_page`]: a page that fails
    /// the check is the plugin's `parse` error. A plugin built for `book-plugin` answers `None`
    /// without a call.
    pub async fn page_bytes(
        &self,
        series: &str,
        chapter: &str,
        url: &str,
    ) -> Result<Result<Option<Vec<u8>>, SourceError>, PluginError> {
        self.page_bytes_with(self.capabilities(), series, chapter, url).await
    }

    /// [`Self::page_bytes`] under `capabilities`, kept by the caller to read the call's reach.
    pub(crate) async fn page_bytes_with(
        &self,
        capabilities: Capabilities,
        series: &str,
        chapter: &str,
        url: &str,
    ) -> Result<Result<Option<Vec<u8>>, SourceError>, PluginError> {
        if !self.makes_pages {
            return Ok(Ok(None));
        }
        let mut store = new_store(&self.engine, self.limits, capabilities);
        let instance = with_pages::BookPluginWithPages::instantiate_async(
            &mut store,
            &self.component,
            &self.linker,
        )
        .await
        .map_err(|e| self.deadline_error(&e))?;
        let made = instance
            .kaeri_books_pages()
            .call_page_bytes(&mut store, series, chapter, url)
            .await
            .map_err(|e| self.deadline_error(&e))?;
        // The store, and the guest memory the page was made in, go before the check runs.
        drop(store);
        Ok(match made {
            Ok(Some(bytes)) => match check_page(&bytes) {
                Ok(()) => Ok(Some(bytes)),
                Err(reason) => {
                    tracing::warn!(plugin = %self.manifest.id, %reason, "a plugin's page was refused");
                    Err(SourceError::Parse(reason))
                }
            },
            Ok(None) => Ok(None),
            Err(error) => Err(same_error(error)),
        })
    }

    /// Fetch one page image a chapter named, as the host does for every page: a GET through the
    /// plugin's own `allowed-hosts`, with the headers the page carries, under the same size
    /// ceiling as any fetch. Outside any guest call, so the plugin never sees the bytes.
    pub async fn fetch_page(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<host::Response, host::HostError> {
        let capabilities = Capabilities::new(
            self.manifest.id.clone(),
            self.manifest.allowed_hosts.clone(),
            self.limits,
            self.http.clone(),
        );
        capabilities
            .qa_fetch_page(host::Request {
                method: "GET".into(),
                url: url.to_owned(),
                headers: headers.to_vec(),
                body: None,
            })
            .await
    }
}

/// The `pages` world's copy of `source-error`, as the `source` one.
fn same_error(error: with_pages::exports::kaeri::books::source::SourceError) -> SourceError {
    use with_pages::exports::kaeri::books::source::SourceError as Pages;
    match error {
        Pages::NotFound => SourceError::NotFound,
        Pages::Blocked(reason) => SourceError::Blocked(reason),
        Pages::Parse(reason) => SourceError::Parse(reason),
        Pages::Other(reason) => SourceError::Other(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_book_plugin_may_not_take_a_built_in_source_s_id() {
        for id in ["folders", "Servers", " demo "] {
            assert!(refuse_reserved(id).is_err(), "{id}");
        }
        assert!(refuse_reserved("example-books").is_ok());
    }
}
