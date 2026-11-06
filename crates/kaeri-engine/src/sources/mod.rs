//! Assembling the provider chain.
//!
//! Lifted out of the binary so it can be exercised. This is the code that decides whether
//! torrent playback comes up at all — a proxy's check runs here, before any session is created — and
//! while it lived in `main.rs` the only way to find out what it had registered was to start the
//! whole interface and look. A probe can now ask it directly.

pub mod commands;
pub mod options;
pub mod torrent;

use kaeri_model::Msg;
use kaeri_model::StoreMsg;
use std::{sync::Arc, time::Duration};

use kaeri_core::config::{Config, Paths};
use kaeri_core::traits::ProviderKind;
use kaeri_net::HttpClient;
use kaeri_providers::{
    ProviderRegistry,
    proxy::{ProxyState, TorrentRoute},
    torrent::TorrentSession,
};

/// The largest torrent file read: one far past this is not a torrent file.
const TORRENT_FILE_LIMIT: u64 = 16 * 1024 * 1024;

/// What the person gave to open, as the magnet link torrent playback starts: a magnet link or an
/// info hash as it is, or else the path of a torrent file, read for what it says.
pub fn torrent_to_open(input: &str) -> std::result::Result<String, Msg> {
    use kaeri_providers::torrent::open;
    if let Some(link) = open::magnet_for(input) {
        return Ok(link);
    }
    let path = std::path::Path::new(input.trim());
    let small =
        std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= TORRENT_FILE_LIMIT);
    let bytes = small.then(|| std::fs::read(path).ok()).flatten();
    bytes
        .as_deref()
        .and_then(open::magnet_from_file)
        .ok_or(Msg::Store(StoreMsg::TorrentLinkUnusable))
}

/// Assemble the provider chain from config, waiting for torrent playback.
///
/// For the command-line paths, which need every source before they can do anything. The
/// interface uses [`build_registry_deferred`] and [`bring_up_torrent`] instead, because torrent
/// playback through a proxy waits on its check, and a proxy that is down takes as long as the
/// check's timeout to say so. Once up, the session is lent to every source in the chain that names
/// torrents, plugins that join later included.
///
/// Only sources that are actually configured are registered. A source listed in `order` but
/// unconfigured is skipped rather than added as a permanently-failing entry, which would
/// clutter the Providers screen with something the user never asked for.
///
/// The last element is the proxy's check when it is what held torrent playback back, for a
/// preview to show as the interface would.
pub async fn build_registry(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
    store: &kaeri_store::Store,
) -> (
    ProviderRegistry,
    Option<(TorrentRoute, Arc<TorrentSession>)>,
    Option<String>,
    Option<ProxyState>,
) {
    let registry = build_registry_deferred(config, http);
    if !torrent_wanted(config) {
        return (registry, None, None, None);
    }
    let started = std::time::Instant::now();
    let outcome = start_torrent_playback(config, paths, store).await;
    tracing::info!(elapsed = ?started.elapsed(), ok = outcome.is_ok(), "torrent playback");
    match outcome {
        Ok(up) => {
            lend_torrents(&registry, Some(&up.session), config);
            (registry, Some((up.route, up.session)), None, None)
        }
        // Playback that cannot start safely stays off, and the reason is surfaced, not
        // swallowed.
        Err((reason, held)) => {
            tracing::warn!(reason = %reason.english(), "torrent playback not started");
            (registry, None, Some(reason.english()), held)
        }
    }
}

/// The provider chain as config describes it. Torrent playback comes up beside it, once a
/// proxy's check has passed when one is set: see [`bring_up_torrent`].
pub fn build_registry_deferred(config: &Config, _http: &HttpClient) -> ProviderRegistry {
    let mut providers: Vec<std::sync::Arc<dyn kaeri_core::traits::Provider>> = Vec::new();
    for id in &config.providers.order {
        if config.providers.disabled.contains(id) {
            continue;
        }
        match id.as_str() {
            // Every `.wasm` in the plugin directory, registered at this point in the order. A
            // plugin is indistinguishable from a native source from here on: same ranking, same
            // health tracking, same failover.
            // Deliberately *not* loaded here — see `spawn_plugin_load`. Compiling a component
            // costs hundreds of milliseconds and nothing needs a plugin before the first frame.
            "plugins" => {}
            // Anything else may be a plugin's id, which ranks that plugin at this point in the
            // order — see `rank`. Plugins compile later, so a typo cannot be told from one yet.
            other => tracing::debug!(provider = other, "order entry left for a plugin"),
        }
    }
    // The demo is built in, and last: it is ranked after every source the order names.
    if config.sources.demo {
        providers.push(kaeri_providers::demo::DemoProvider::shared());
    }
    let registry = ProviderRegistry::new(providers);
    // A plugin switched off by name joins the chain all the same, and is not tried.
    registry.set_disabled(config.providers.disabled.iter().cloned());
    registry
}

/// Whether the configuration turns torrent playback on.
pub fn torrent_wanted(config: &Config) -> bool {
    config.providers.torrent.enabled
}

/// Lend `session` to every source in `registry` that names torrents, or take it back.
pub fn lend_torrents(
    registry: &ProviderRegistry,
    session: Option<&Arc<TorrentSession>>,
    config: &Config,
) {
    registry.torrents().set(session.cloned(), config.providers.torrent.trackers.clone());
}

/// Put a source that came up late, such as the demo added back, where `providers.order` puts it.
pub fn place(
    registry: &ProviderRegistry,
    provider: Arc<dyn kaeri_core::traits::Provider>,
    config: &Config,
) {
    let order = &config.providers.order;
    registry.insert_ranked(provider, |p| rank(p, order));
}

/// Put the whole chain in the order `providers.order` gives it, for a source that has just
/// joined it or an order that has just changed. See [`rank`].
pub fn arrange(registry: &ProviderRegistry, config: &Config) {
    let order = &config.providers.order;
    registry.rank_by(|p| rank(p, order));
    // Whatever the user just did to the sources, a source that went down earlier gets to
    // answer again, rather than staying out until the app is restarted.
    registry.retry_down();
}

/// Where a source stands in `order`: at its own entry when the order names it, at `"plugins"`
/// for a plugin it does not name, and after everything for a source it leaves out.
///
/// Then by id, so the plugins left at `"plugins"` run alphabetically. They used to run in the
/// order their files were read, which no one chose and a reinstall could change. Naming a plugin
/// only moves it: whether plugins run at all is still `"plugins"`'s to say, see
/// [`plugins_enabled`].
///
/// Every source is in that one list, first-party kinds included, and the user orders them all. A
/// kind's group entry (`"library"`, `"servers"`, `"catalogues"`, `"demo"`) places the sources of
/// that kind the order does not name by id, as `"plugins"` does for plugins. An older order that
/// knows none of this still tries Your media, servers and catalogues first, in that order, and the
/// demo last.
pub fn rank(
    provider: &dyn kaeri_core::traits::Provider,
    order: &[String],
) -> (i64, u8, String) {
    let manifest = provider.manifest();
    let at = |name: &str| order.iter().position(|id| id == name);
    let place = at(&manifest.id)
        .or_else(|| kaeri_model::sources::group_entry(manifest.kind.as_str()).and_then(at))
        .map(|place| place as i64)
        .unwrap_or_else(|| match manifest.kind {
            ProviderKind::Library | ProviderKind::Server | ProviderKind::Catalogue => -1,
            _ => order.len() as i64,
        });
    // Sources that share a place are told apart by kind, then by id.
    (place, manifest.kind.tier(), manifest.id.clone())
}

/// Load plugins in the background and add them to a live registry.
///
/// The whole reason this is not part of [`build_registry`]: compiling a WebAssembly component is
/// expensive and deterministic. Measured at 874 ms for the JavaScript reference plugin — which was
/// most of the ~976 ms the app spent assembling sources before it could draw anything, for a
/// capability most launches never reach for. wasmtime's compilation cache makes the *second* run
/// cheap; loading off the critical path makes the first one cheap too.
///
/// The chain is put in order once they have joined it, so where a plugin is tried is what
/// `providers.order` says, whenever it finished compiling. See [`rank`].
///
/// The chain says they are on their way until they have joined it, or failed to, so a play
/// asked for meanwhile can wait for them rather than look up the episode without them: see
/// [`ProviderRegistry::expect_arrivals`].
pub fn spawn_plugin_load(
    registry: ProviderRegistry,
    config: Config,
    http: HttpClient,
    paths: Paths,
    books: Option<crate::book_plugins::BookPlugins>,
) -> tokio::task::JoinHandle<Option<Msg>> {
    let shows = plugins_enabled(&config);
    // Said before the task is spawned, so a play sent the moment the engine is up sees it.
    let arriving = shows.then(|| Arrivals::expected(&registry));
    tokio::spawn(async move {
        if !shows && books.is_none() {
            return None;
        }
        let _arriving = arriving;
        let started = std::time::Instant::now();
        match load_plugins(&config, &http, &paths, books.is_some()).await {
            Ok(loaded) => {
                if let Some(holder) = &books {
                    let count = loaded.books.len();
                    holder.extend(loaded.books);
                    tracing::info!(count, "book plugins loaded");
                }
                if !shows {
                    return None;
                }
                let count = loaded.providers.len();
                registry.extend(loaded.providers);
                arrange(&registry, &config);
                tracing::info!(count, elapsed = ?started.elapsed(), "plugins registered");
                (!loaded.outdated.is_empty()).then(|| {
                    Msg::Store(StoreMsg::PluginsOutdated {
                        count: loaded.outdated.len() as i64,
                        names: loaded.outdated.join(", "),
                    })
                })
            }
            Err(reason) => {
                tracing::warn!(%reason, "plugin host not started");
                Some(Msg::Store(StoreMsg::PluginsLoadFailed { reason }))
            }
        }
    })
}

/// Sources on their way into a chain, said done when this is dropped: once they have joined it
/// and it is in order, or the load failed, or its task was cut short.
struct Arrivals(ProviderRegistry);

impl Arrivals {
    fn expected(registry: &ProviderRegistry) -> Self {
        registry.expect_arrivals();
        Self(registry.clone())
    }
}

impl Drop for Arrivals {
    fn drop(&mut self) {
        self.0.arrivals_done();
    }
}

/// How long a lookup waits for sources still on their way before it goes ahead without them.
///
/// Compiling a plugin takes under a second once its build is cached and a few the first time;
/// past this, something is wrong with the load, and an answer from the rest of the chain is
/// worth more than waiting on.
pub const ARRIVALS_WAIT: Duration = Duration::from_secs(30);

/// Wait for sources on their way into `registry`, for at most [`ARRIVALS_WAIT`], saying so with
/// `note` while it waits.
pub async fn arrived(registry: &ProviderRegistry, note: impl FnOnce()) {
    if !registry.is_arriving() {
        return;
    }
    note();
    if !registry.arrivals(ARRIVALS_WAIT).await {
        tracing::warn!(wait = ?ARRIVALS_WAIT, "sources still loading; going ahead without them");
    }
}

/// The limits `[providers.plugins]` sets.
pub fn plugin_limits(config: &Config) -> kaeri_plugin::Limits {
    kaeri_plugin::Limits {
        memory_bytes: config.providers.plugins.memory_mb.max(1).saturating_mul(1024 * 1024),
        deadline: Duration::from_secs(config.providers.plugins.deadline_secs.max(1)),
        ..Default::default()
    }
}

/// A plugin host as the app runs plugins: lent the HTTP client, given the user's settings, and
/// caching compiled components.
fn plugin_host(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
) -> std::result::Result<kaeri_plugin::PluginHost, String> {
    let cache = paths.cache_dir.join("wasm");
    Ok(kaeri_plugin::PluginHost::with_cache(
        plugin_limits(config),
        Some(http.clone()),
        Some(&cache),
    )
    .map_err(|e| e.to_string())?
    .with_plugin_settings(config.providers.plugins.settings.clone()))
}

/// Whether plugins are part of the chain at all.
pub fn plugins_enabled(config: &Config) -> bool {
    config.providers.order.iter().any(|id| id == "plugins")
        && !config.providers.disabled.iter().any(|id| id == "plugins")
}

/// Why a plugin file did not load.
pub enum PluginLoadError {
    /// Built for another major version of the plugin ABI; its author has to rebuild it.
    Outdated,
    /// Anything else, with the plugin host's own words.
    Failed(String),
}

/// One plugin file, loaded: a show source for the chain, or a book source.
pub enum LoadedPlugin {
    Shows(Arc<dyn kaeri_core::traits::Provider>),
    Books(Arc<kaeri_plugin::WasmBookSource>),
}

/// Load one plugin file, for a plugin installed while the app runs. A book source loads only
/// when `books` says reading is available; otherwise it is refused as a source of another kind.
pub async fn load_plugin(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
    path: &std::path::Path,
    books: bool,
) -> std::result::Result<LoadedPlugin, PluginLoadError> {
    let host = plugin_host(config, http, paths).map_err(PluginLoadError::Failed)?;
    let refused = |e| match e {
        kaeri_plugin::PluginError::OutdatedAbi { .. } => PluginLoadError::Outdated,
        e => PluginLoadError::Failed(e.to_string()),
    };
    if !books {
        let plugin = host.load(path).await.map_err(refused)?;
        return Ok(LoadedPlugin::Shows(Arc::new(kaeri_plugin::WasmProvider::new(plugin))));
    }
    Ok(match host.load_any(path).await.map_err(refused)? {
        kaeri_plugin::AnyPlugin::Shows(plugin) => {
            LoadedPlugin::Shows(Arc::new(kaeri_plugin::WasmProvider::new(plugin)))
        }
        kaeri_plugin::AnyPlugin::Books(plugin) => {
            LoadedPlugin::Books(Arc::new(kaeri_plugin::WasmBookSource::new(plugin)))
        }
    })
}

/// Every plugin in the plugin directory: the show sources, the book sources when `books` asks
/// for them, and the names of those built for an older interface.
struct LoadedPlugins {
    providers: Vec<Arc<dyn kaeri_core::traits::Provider>>,
    books: Vec<kaeri_plugin::WasmBookSource>,
    outdated: Vec<String>,
}

/// Load every plugin in the plugin directory.
async fn load_plugins(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
    books: bool,
) -> std::result::Result<LoadedPlugins, String> {
    let host = plugin_host(config, http, paths)?;

    let dir = paths.plugin_dir();
    let loaded = kaeri_plugin::load_plugins_reporting(&host, &dir, books).await;
    tracing::info!(
        dir = %dir.display(),
        count = loaded.providers.len(),
        books = loaded.books.len(),
        "plugins loaded"
    );

    Ok(LoadedPlugins {
        providers: loaded
            .providers
            .into_iter()
            .map(|p| Arc::new(p) as Arc<dyn kaeri_core::traits::Provider>)
            .collect(),
        books: loaded.books,
        outdated: loaded.outdated,
    })
}

/// Torrent playback, up: its route, and its session.
pub struct TorrentUp {
    pub route: TorrentRoute,
    pub session: Arc<TorrentSession>,
}

/// The route torrent playback takes, when the configuration can build one: a mistake a retry
/// cannot fix otherwise.
fn prepare_torrent(config: &Config, fixture: bool) -> std::result::Result<TorrentRoute, Msg> {
    route_for(config, fixture)
        .map_err(|reason| Msg::Store(StoreMsg::TorrentProxyUnusable { reason }))
}

/// The route `config` describes: directly, or through its proxy. In a fixture build, a proxy's
/// check passes without connecting anywhere.
pub fn route_for(config: &Config, fixture: bool) -> std::result::Result<TorrentRoute, String> {
    let proxy = config.providers.torrent.proxy.clone();
    if fixture { TorrentRoute::fixed(proxy) } else { TorrentRoute::new(proxy) }
}

/// The longest torrents wait between checks of a proxy that keeps failing.
const RETRY_CEILING: Duration = Duration::from_secs(60);

/// Check the route, then start the torrent session.
///
/// Through a proxy, the check comes before a session exists, so a proxy that is down is said
/// rather than found later as torrents that find no peers.
// The error is a whole message with its values, which is large; it is built once, on failure.
#[allow(clippy::result_large_err)]
async fn start_torrent_playback(
    config: &Config,
    paths: &Paths,
    store: &kaeri_store::Store,
) -> std::result::Result<TorrentUp, (Msg, Option<ProxyState>)> {
    let route = prepare_torrent(config, false).map_err(|reason| (reason, None))?;

    let state = route.verify().await;
    if !state.is_ready() {
        return Err((Msg::Store(StoreMsg::TorrentProxyWaiting), Some(state)));
    }

    sweep_before_session(config, paths, store).await;
    start_torrent_session(paths, route).await.map_err(|reason| (reason, None))
}

/// Bring torrent playback up for the interface, however long a proxy's check takes to pass.
///
/// The interface draws while this runs: a proxy that is down made the check wait out its whole
/// timeout before the first frame. And while the check fails it is made again, sooner at first
/// and then once a minute at most, or at once when Check now asks, so a proxy started after
/// launch brings torrents up without a restart. Once it passes, nothing more is checked. Each
/// answer reaches the interface as it arrives.
///
/// Errs only for what no amount of waiting fixes: a configuration that cannot work, or a
/// session that would not start.
pub async fn bring_up_torrent(
    config: Config,
    paths: Paths,
    store: kaeri_store::Store,
    tx: crate::outbox::Outbox,
    check: std::sync::Arc<tokio::sync::Notify>,
    fixture: bool,
) -> std::result::Result<TorrentUp, Msg> {
    use kaeri_model::event::Event;

    let route = prepare_torrent(&config, fixture)?;
    // Before the wait rather than after it: a cache left full by an earlier run should not stay
    // full for as long as the proxy is down.
    sweep_before_session(&config, &paths, &store).await;

    let mut said = false;
    let mut wait = Duration::from_secs(5);
    loop {
        let state = route.verify().await;
        if tx.send(Event::TorrentProxy(Some(proxy_status(&state)))).is_err() {
            return Err(Msg::Store(StoreMsg::TorrentInterfaceClosed));
        }
        if state.is_ready() {
            if said {
                tracing::info!("the torrent proxy check passed");
            }
            break;
        }
        if !said {
            said = true;
            tracing::warn!(
                reason = state.reason().unwrap_or("-"),
                "the torrent proxy check failed"
            );
            let _ = tx.send(Event::ProviderNote(Msg::Store(StoreMsg::TorrentProxyWaiting)));
        }
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            () = check.notified() => {}
        }
        wait = (wait * 2).min(RETRY_CEILING);
    }

    start_torrent_session(&paths, route).await
}

/// The check's answer as the interface is told it, without its reasons, which are the log's.
pub fn proxy_status(state: &ProxyState) -> kaeri_model::sources::ProxyStatus {
    use kaeri_model::sources::ProxyStatus;
    match state {
        ProxyState::Unchecked => ProxyStatus::Checking,
        ProxyState::Passed => ProxyStatus::Passed,
        ProxyState::Failed { .. } => ProxyStatus::Failed,
        ProxyState::Direct => ProxyStatus::Direct,
    }
}

/// Bring the stream cache within its limit before a session exists, so nothing a session is
/// about to write can be mistaken for a leftover. Nothing is live yet; downloads are kept by the
/// sweep itself.
async fn sweep_before_session(config: &Config, paths: &Paths, store: &kaeri_store::Store) {
    let dir = paths.cache_dir.join("torrents");
    let limit = config.providers.torrent.cache_limit_bytes();
    crate::background::prune::sweep_torrent_cache(dir, limit, store, Default::default()).await;
}

/// Start the torrent session on a route that is ready.
async fn start_torrent_session(
    paths: &Paths,
    route: TorrentRoute,
) -> std::result::Result<TorrentUp, Msg> {
    let session = Arc::new(
        kaeri_providers::torrent::TorrentSession::start(
            route.clone(),
            paths.cache_dir.join("torrents"),
            paths.cache_dir.join("dht.json"),
            random_token(),
        )
        .await
        .map_err(|e| Msg::Store(StoreMsg::TorrentSessionFailed { reason: e.to_string() }))?,
    );

    Ok(TorrentUp { route, session })
}

/// An unguessable path segment for the loopback stream server.
///
/// Not a security boundary — loopback binding is — but it stops an unrelated local process
/// from stumbling onto the stream via a predictable path.
fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(kaeri_store::now() as u64);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use kaeri_core::traits::ProviderKind;
    use kaeri_providers::mock::MockProvider;

    use super::*;

    fn order(entries: &[&str]) -> Config {
        let mut config = Config::default();
        config.providers.order = entries.iter().map(|e| (*e).to_owned()).collect();
        config
    }

    fn plugin(id: &str) -> Arc<dyn kaeri_core::traits::Provider> {
        MockProvider::new(id).kind(ProviderKind::Plugin).arc()
    }

    fn source(id: &str, kind: ProviderKind) -> Arc<dyn kaeri_core::traits::Provider> {
        MockProvider::new(id).kind(kind).arc()
    }

    #[test]
    fn a_named_plugin_ranks_at_its_own_entry_and_the_rest_at_plugins_by_id() {
        let config = order(&["beta", "extra-a", "plugins", "extra"]);
        let order = &config.providers.order;
        let ranked = |p: Arc<dyn kaeri_core::traits::Provider>| rank(p.as_ref(), order);
        assert_eq!(ranked(plugin("beta")), (0, 3, "beta".into()));
        assert_eq!(ranked(source("extra-a", ProviderKind::Native)), (1, 3, "extra-a".into()));
        assert_eq!(ranked(plugin("gamma")), (2, 3, "gamma".into()));
        assert_eq!(ranked(plugin("alpha")), (2, 3, "alpha".into()));
        assert_eq!(ranked(source("extra", ProviderKind::Native)), (3, 3, "extra".into()));
        // Left out of the order: last, as before.
        assert_eq!(ranked(source("other", ProviderKind::Native)), (4, 3, "other".into()));
    }

    #[test]
    fn only_a_plugin_falls_back_to_the_plugins_entry() {
        // A built-in source the order leaves out is not a plugin, whatever the order says
        // about plugins.
        let config = order(&["plugins"]);
        let order = &config.providers.order;
        assert_eq!(rank(source("extra", ProviderKind::Native).as_ref(), order).0, 1);
        assert_eq!(rank(plugin("alpha").as_ref(), order).0, 0);
    }

    #[test]
    fn an_order_that_predates_first_party_sources_still_puts_them_first_and_the_demo_last() {
        let registry = ProviderRegistry::new(vec![
            source("demo", ProviderKind::Demo),
            plugin("alpha"),
            source("catalogue", ProviderKind::Catalogue),
            source("server", ProviderKind::Server),
            source("folder", ProviderKind::Library),
            source("extra", ProviderKind::Native),
        ]);
        arrange(&registry, &order(&["extra", "plugins"]));
        assert_eq!(registry.ids(), ["folder", "server", "catalogue", "extra", "alpha", "demo"]);
    }

    #[test]
    fn the_user_can_put_any_source_anywhere_first_party_ones_included() {
        let registry = ProviderRegistry::new(vec![
            source("folder", ProviderKind::Library),
            source("server", ProviderKind::Server),
            plugin("alpha"),
            plugin("beta"),
            source("demo", ProviderKind::Demo),
        ]);
        arrange(&registry, &order(&["alpha", "servers", "beta", "library", "demo"]));
        assert_eq!(registry.ids(), ["alpha", "server", "beta", "folder", "demo"]);
        // Named by id, one sits apart from the rest of its kind.
        arrange(&registry, &order(&["server", "plugins", "library", "demo"]));
        assert_eq!(registry.ids(), ["server", "alpha", "beta", "folder", "demo"]);
    }

    #[test]
    fn a_new_install_starts_with_first_party_kinds_first() {
        let default = Config::default().providers.order;
        assert_eq!(default, kaeri_model::sources::default_source_order());
        let registry = ProviderRegistry::new(vec![
            plugin("alpha"),
            source("demo", ProviderKind::Demo),
            source("server", ProviderKind::Server),
        ]);
        arrange(&registry, &Config::default());
        assert_eq!(registry.ids(), ["server", "alpha", "demo"]);
    }

    #[test]
    fn a_source_that_went_down_is_tried_again_once_the_order_changes() {
        let registry = ProviderRegistry::new(vec![plugin("alpha"), plugin("beta")]);
        let failure = kaeri_core::error::ProviderError::Transport("timed out".into());
        for _ in 0..3 {
            registry.health().record_failure("alpha", &failure, 0);
        }
        assert_eq!(registry.usable_ids(), ["beta"], "alpha is down");
        arrange(&registry, &order(&["beta", "alpha"]));
        assert_eq!(
            registry.usable_ids(),
            ["beta", "alpha"],
            "the user's change is a new chance"
        );
    }

    #[test]
    fn arranging_puts_the_chain_in_the_order_the_config_gives() {
        // As plugins arrive: in the order their files were read, after the built-in sources.
        let registry = ProviderRegistry::new(vec![
            source("native", ProviderKind::Native),
            plugin("gamma"),
            plugin("beta"),
            plugin("alpha"),
        ]);
        arrange(&registry, &order(&["beta", "native", "plugins"]));
        assert_eq!(registry.ids(), ["beta", "native", "alpha", "gamma"]);

        // A late source still takes its place in it.
        let config = order(&["beta", "extra", "native", "plugins"]);
        arrange(&registry, &config);
        let order = &config.providers.order;
        registry.insert_ranked(source("extra", ProviderKind::Native), |p| rank(p, order));
        assert_eq!(registry.ids(), ["beta", "extra", "native", "alpha", "gamma"]);
    }

    #[test]
    fn a_torrent_to_open_is_a_link_a_hash_or_a_torrent_file() {
        let hash = "fd45c2c75aedb6a1916c201a03f9640e9ee006f6";
        assert_eq!(torrent_to_open(hash), Ok(format!("magnet:?xt=urn:btih:{hash}")));

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ep01.torrent");
        std::fs::write(
            &file,
            b"d4:infod6:lengthi10e4:name8:ep01.mkv12:piece lengthi16384e6:pieces20:aaaaaaaaaaaaaaaaaaaaee",
        )
        .unwrap();
        let opened = torrent_to_open(file.to_str().unwrap()).unwrap();
        assert_eq!(opened, format!("magnet:?xt=urn:btih:{hash}&dn=ep01.mkv"));

        let page = dir.path().join("page.html");
        std::fs::write(&page, "<html></html>").unwrap();
        for unusable in [page.to_str().unwrap(), "no such file", "https://index.example/1"] {
            assert_eq!(
                torrent_to_open(unusable),
                Err(Msg::Store(StoreMsg::TorrentLinkUnusable)),
                "{unusable}"
            );
        }
    }

    /// A preview shows the line and toast the interface would, so the proxy's check has to
    /// come back with what held torrent playback back.
    #[tokio::test]
    async fn a_proxy_that_holds_torrents_back_comes_back_with_its_check() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let mut config = Config::default();
        let torrent = &mut config.providers.torrent;
        torrent.enabled = true;
        // A proxy nothing listens on: the check fails at once and offline.
        torrent.proxy = Some("socks5://127.0.0.1:1".into());
        let http = HttpClient::new(&config.network).unwrap();
        let store = kaeri_store::Store::open_in_memory().unwrap();

        let (registry, route, note, held) =
            build_registry(&config, &http, &paths, &store).await;
        assert!(route.is_none() && registry.torrents().session().is_none());
        assert!(note.is_some_and(|n| n.starts_with("The proxy check failed.")));
        let held = held.expect("the check that held it back");
        assert!(!held.is_ready(), "{held:?}");
    }
}
