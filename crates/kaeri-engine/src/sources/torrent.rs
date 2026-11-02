//! The torrent stack as the settings stand it up and down: the tasks that run beside a torrent
//! session once it is up, the session lent to the sources that name torrents, and
//! the source chain rebuilt when a setting changes which sources there are.

use kaeri_model::Msg;
use kaeri_model::StoreMsg;
use std::{sync::Arc, time::Duration};

use kaeri_core::config::Config;
use kaeri_model::{event::Event, notice::Notice, sources::ProxyStatus};
use kaeri_providers::proxy::TorrentRoute;
use kaeri_store::Store;
use tokio::sync::{Notify, mpsc};

use crate::{
    SharedConfig, background::prune, data, downloads, outbox::Outbox, services::Services,
    sources,
};

/// How long a bring-up waits for the session it replaces to stand down.
const TEARDOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// Everything torrent playback runs in the background: the download manager, seeding and the
/// stream cache's upkeep.
///
/// Grouped so they can be stopped and started as one. Playback the user has just switched
/// off must stop *seeding*, not merely stop being offered — and that means the tasks
/// holding its session have to end with it.
#[derive(Default)]
pub struct TorrentTasks {
    /// Bringing the session up, waiting for a proxy's check to pass when there is one.
    bring_up: Option<tokio::task::JoinHandle<()>>,
    /// Which bring-up the current one is. A result from an earlier one — the settings changed
    /// while it waited — describes playback that has since been switched off or replaced.
    generation: u64,
    downloads: Option<tokio::task::JoinHandle<()>>,
    seeding: Option<tokio::task::JoinHandle<()>>,
    /// Keeps the stream cache within its limit while the session writes to it.
    cache: Option<tokio::task::JoinHandle<()>>,
    session: Option<Arc<kaeri_providers::torrent::TorrentSession>>,
    /// The session a settings change let go of, still standing down: the next bring-up waits
    /// for it, so the old traffic stops before new traffic starts, without the actor waiting.
    teardown: Option<tokio::task::JoinHandle<()>>,
    /// Brings a failing proxy's next check forward, for Sources' Check now: a bring-up waiting on
    /// it checks at once.
    check: Arc<Notify>,
}

impl TorrentTasks {
    fn start(
        route: &Option<(TorrentRoute, Arc<kaeri_providers::torrent::TorrentSession>)>,
        store: &Store,
        config: &SharedConfig,
        tx: &Outbox,
        check: &Arc<Notify>,
    ) -> Self {
        let Some((route, session)) = route.clone() else {
            // No session means no downloads, which is correct rather than a limitation:
            // every downloadable source here is a torrent.
            return Self { check: Arc::clone(check), ..Self::default() };
        };

        let downloads =
            downloads::spawn(store.clone(), session.clone(), Arc::clone(config), tx.clone());
        downloads::publish_now(store, tx);

        let _ = tx.send(Event::TorrentProxy(Some(sources::proxy_status(&route.state()))));
        let seeding =
            tokio::spawn(seeding_watch(session.clone(), Arc::clone(config), tx.clone()));
        let cache = tokio::spawn(prune::torrent_cache_watch(
            session.clone(),
            store.clone(),
            Arc::clone(config),
        ));
        Self {
            bring_up: None,
            generation: 0,
            downloads: Some(downloads),
            seeding: Some(seeding),
            cache: Some(cache),
            session: Some(session),
            teardown: None,
            check: Arc::clone(check),
        }
    }

    /// Start bringing torrent playback up, when the configuration asks for it.
    ///
    /// In the background: a proxy's check is a connection to a proxy that may be down, and the
    /// first frame must not wait on it. The interface hears what the check found meanwhile, and
    /// the source joins the chain once the session is up: see [`Self::arrived`].
    pub(crate) fn bring_up(
        &mut self,
        config: &Config,
        services: &Services,
        tx: &Outbox,
        arrivals: &mpsc::UnboundedSender<(u64, std::result::Result<sources::TorrentUp, Msg>)>,
    ) {
        self.generation += 1;
        if !sources::torrent_wanted(config) {
            return;
        }
        let starting = if config.providers.torrent.proxy.is_some() {
            ProxyStatus::Checking
        } else {
            ProxyStatus::Direct
        };
        let _ = tx.send(Event::TorrentProxy(Some(starting)));
        let generation = self.generation;
        let check = Arc::clone(&self.check);
        let fixture = services.fixture_route;
        let teardown = self.teardown.take();
        let (config, paths, store, tx, arrivals) = (
            // With the proxy's password, which only this copy holds.
            crate::proxy::with_proxy_password(config, services),
            services.paths.clone(),
            services.store.clone(),
            tx.clone(),
            arrivals.clone(),
        );
        self.bring_up = Some(tokio::spawn(async move {
            // The session this one replaces stands down first, within reason: one that takes
            // longer has paused what it could, and is left to finish on its own.
            if let Some(teardown) = teardown
                && tokio::time::timeout(TEARDOWN_WAIT, teardown).await.is_err()
            {
                tracing::warn!("the previous torrent session was still standing down");
            }
            // A fixture build says what the check finds and starts no session: a session reaches
            // the DHT and the trackers, which no fixture may.
            if fixture {
                check_alone(&config, &tx, true);
                return;
            }
            let outcome =
                sources::bring_up_torrent(config, paths, store, tx, check, fixture).await;
            let _ = arrivals.send((generation, outcome));
        }));
    }

    /// A bring-up finished: lend the session to the sources that name torrents and start what
    /// runs beside it, or say why it will not come up.
    pub(crate) fn arrived(
        &mut self,
        generation: u64,
        outcome: std::result::Result<sources::TorrentUp, Msg>,
        torrent_route: &mut Option<(
            TorrentRoute,
            Arc<kaeri_providers::torrent::TorrentSession>,
        )>,
        services: &Services,
        config: &SharedConfig,
        tx: &Outbox,
    ) {
        if generation != self.generation {
            // Superseded. Its session holds no torrents yet, so letting go of it is enough.
            return;
        }
        self.bring_up = None;
        let up = match outcome {
            Ok(up) => up,
            Err(reason) => {
                tracing::warn!(reason = %reason.english(), "torrent playback not started");
                let _ = tx.send(Event::ProviderNote(reason));
                return;
            }
        };
        let registry = &services.registry;
        let snapshot =
            config.read().map(|c| c.clone()).unwrap_or_else(|e| e.into_inner().clone());
        sources::lend_torrents(registry, Some(&up.session), &snapshot);
        // The sources that name torrents are usable now: what is on screen was listed without.
        let _ = tx.internal(crate::outbox::Internal::SourcesReordered);
        *torrent_route = Some((up.route, up.session));
        let check = Arc::clone(&self.check);
        *self = Self {
            generation,
            ..Self::start(torrent_route, &services.store, config, tx, &check)
        };
        let _ = tx.send(Event::Providers(data::provider_rows(registry, &services.stores)));
        tracing::info!(providers = ?registry.ids(), "torrent playback up");
    }

    /// Check the proxy again now, as Sources' Check now asks: `Checking`, then what it found, on
    /// `Event::TorrentProxy`; `None` there when torrents are off.
    ///
    /// A bring-up waiting on a failed check checks at once, and starts the session once it
    /// passes. With none, as when the session is up or could not start, the proxy is checked on
    /// its own and nothing else follows from it.
    pub(crate) fn check_now(&self, config: &Config, services: &Services, tx: &Outbox) {
        if self.bring_forward(config, tx) {
            check_alone(
                &crate::proxy::with_proxy_password(config, services),
                tx,
                services.fixture_route,
            );
        }
    }

    /// Check now, as far as it is the tasks': say torrents are off, or that it is checking and
    /// have a bring-up waiting on it check now. Whether nothing waits, so the proxy is checked on
    /// its own.
    fn bring_forward(&self, config: &Config, tx: &Outbox) -> bool {
        if !sources::torrent_wanted(config) {
            let _ = tx.send(Event::TorrentProxy(None));
            return false;
        }
        let _ = tx.send(Event::TorrentProxy(Some(ProxyStatus::Checking)));
        // A bring-up that has already given up, its outcome not yet taken in, waits on nothing.
        if self.bring_up.as_ref().is_some_and(|task| !task.is_finished()) {
            self.check.notify_one();
            return false;
        }
        true
    }

    /// Stop everything and close the session before letting go of it, as a settings change
    /// asks.
    ///
    /// Closing is the whole point: dropping the handles would leave librqbit seeding whatever
    /// it holds until the process exits, which for playback the user just turned off is
    /// precisely the traffic they meant to stop. The close pauses each torrent in turn, which
    /// on a slow disk takes a while, so it runs in a task of its own rather than on the actor:
    /// the next bring-up waits for it. The watchers go first, so none of them reacts to the
    /// session going.
    fn stop(&mut self) {
        // A bring-up still waiting on a proxy has no session to close, only a wait to end.
        if let Some(waiting) = self.bring_up.take() {
            waiting.abort();
        }
        for task in [self.downloads.take(), self.seeding.take(), self.cache.take()]
            .into_iter()
            .flatten()
        {
            task.abort();
        }
        if let Some(session) = self.session.take() {
            let earlier = self.teardown.take();
            self.teardown = Some(tokio::spawn(async move {
                if let Some(earlier) = earlier {
                    let _ = earlier.await;
                }
                session.close().await;
                tracing::info!("torrent session closed for a settings change");
            }));
        }
    }

    /// Stand everything down as the app quits: the watchers first, so none of them reacts to
    /// the session going, then the session, closed in order so it keeps how far each torrent
    /// got, and waited for here rather than in the background as [`Self::stop`] leaves it.
    pub(crate) async fn close(&mut self) {
        if let Some(waiting) = self.bring_up.take() {
            waiting.abort();
        }
        for task in [self.downloads.take(), self.seeding.take(), self.cache.take()]
            .into_iter()
            .flatten()
        {
            task.abort();
        }
        if let Some(session) = self.session.take() {
            session.close().await;
        }
    }
}

/// Check the proxy `config` names on its own, with no session or bring-up to act on what it
/// finds, and say what it found.
fn check_alone(config: &Config, tx: &Outbox, fixture: bool) {
    let route = match sources::route_for(config, fixture) {
        Ok(route) => route,
        Err(reason) => {
            let _ = tx.send(Event::TorrentProxy(Some(ProxyStatus::Failed)));
            let _ = tx.send(Event::Toast(Notice::alert(Msg::Store(
                StoreMsg::TorrentProxyUnusable { reason },
            ))));
            return;
        }
    };
    let tx = tx.clone();
    tokio::spawn(async move {
        let state = route.verify().await;
        let _ = tx.send(Event::TorrentProxy(Some(sources::proxy_status(&state))));
    });
}

/// Keep the session's seeding in line with the settings, retire watched episodes that have
/// given back enough, and tell the Downloads screen what is still seeding.
///
/// Reads the shared config each tick rather than being told about changes: a setting saved on
/// the Settings screen reaches the session within one tick, upload cap included, with no
/// rebuild of the source (which would tear down the episode playing).
async fn seeding_watch(
    session: Arc<kaeri_providers::torrent::TorrentSession>,
    config: SharedConfig,
    tx: Outbox,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(10));
    let mut shown: Option<Vec<kaeri_model::downloads::SeedRow>> = None;
    loop {
        ticker.tick().await;
        let goal =
            config.read().map(|c| c.providers.torrent.seeding.clone()).unwrap_or_default();
        session.configure_seeding(&goal).await;
        let rows: Vec<kaeri_model::downloads::SeedRow> = session
            .reap_seeding()
            .await
            .into_iter()
            .map(|seed| kaeri_model::downloads::SeedRow {
                name: seed.name,
                uploaded_bytes: seed.uploaded_bytes,
                ratio: seed.ratio,
                goal: goal.ratio,
                left_secs: seed.left.as_secs(),
            })
            .collect();
        // Only when it changed: an idle screen should not redraw every ten seconds for nothing.
        if shown.as_ref() != Some(&rows) {
            if tx.send(Event::Seeding(rows.clone())).is_err() {
                break;
            }
            shown = Some(rows);
        }
    }
}

/// Rebuild the source chain from the configuration as it now stands.
///
/// Called when a setting redefines what sources exist. The chain is swapped *in place*
/// so every holder of a registry clone sees the change, and the plugins that compiled in
/// the background are carried across — they are not described by config and a rebuild
/// that forgot them would silently drop sources the user installed.
pub fn reload_sources(
    services: &Services,
    torrent_tasks: &mut TorrentTasks,
    torrent_route: &mut Option<(TorrentRoute, Arc<kaeri_providers::torrent::TorrentSession>)>,
    config: &SharedConfig,
    tx: &Outbox,
    arrivals: &mpsc::UnboundedSender<(u64, std::result::Result<sources::TorrentUp, Msg>)>,
) {
    // Old traffic stops before new traffic starts. The other order would leave two
    // sessions briefly alive, and one of them is the one being switched off. The stop goes on
    // in the background, and the bring-up below waits for it.
    torrent_tasks.stop();
    *torrent_route = None;
    let registry = &services.registry;
    // The sources that name torrents wait for the next session, not the one going.
    sources::lend_torrents(registry, None, &Config::default());

    let plugins = registry.of_kind(kaeri_core::traits::ProviderKind::Plugin);
    let snapshot = config.read().map(|c| c.clone()).unwrap_or_else(|e| e.into_inner().clone());
    let rebuilt = sources::build_registry_deferred(&snapshot, &services.http);
    let mut providers = rebuilt.all();
    providers.extend(plugins);
    registry.replace(providers);
    registry.set_disabled(snapshot.providers.disabled.iter().cloned());
    registry.retry_down();
    // The plugins were carried across behind the rebuilt sources; a plugin the order names
    // ahead of one of those goes back in front of it.
    sources::arrange(registry, &snapshot);

    torrent_tasks.bring_up(&snapshot, services, tx, arrivals);

    if !sources::torrent_wanted(&snapshot) {
        // No session: say so, and clear a line that now describes nothing.
        let _ = tx.send(Event::TorrentProxy(None));
    }
    let note = if !sources::torrent_wanted(&snapshot) {
        Some(Msg::Store(StoreMsg::SourcesReloadedTorrentsOff))
    } else if snapshot.providers.torrent.proxy.is_some() {
        Some(Msg::Store(StoreMsg::SourcesReloadedTorrentsChecking))
    } else {
        None
    };
    if let Some(note) = note {
        let _ = tx.send(Event::ProviderNote(note));
    }
    let _ = tx.send(Event::Providers(data::provider_rows(registry, &services.stores)));
    tracing::info!(providers = ?registry.ids(), "source chain reloaded");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Torrents on, through `proxy` when there is one.
    fn torrents(proxy: Option<&str>) -> Config {
        let mut config = Config::default();
        let torrent = &mut config.providers.torrent;
        torrent.enabled = true;
        torrent.proxy = proxy.map(Into::into);
        config
    }

    fn said(rx: &mut mpsc::UnboundedReceiver<Event>) -> Vec<Event> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    /// What `Event::TorrentProxy`s said, in order; anything else fails.
    fn lines(said: Vec<Event>) -> Vec<Option<ProxyStatus>> {
        said.into_iter()
            .map(|event| match event {
                Event::TorrentProxy(status) => status,
                other => panic!("not the proxy's line: {other:?}"),
            })
            .collect()
    }

    #[tokio::test]
    async fn check_now_wakes_a_waiting_bring_up_or_checks_alone() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx: Outbox = tx.into();
        let mut tasks = TorrentTasks::default();

        // Off: there is nothing to check.
        assert!(!tasks.bring_forward(&Config::default(), &tx));
        assert_eq!(lines(said(&mut rx)), [None]);

        // On with no proxy, and nothing waits: checked on its own, and direct.
        let direct = torrents(None);
        assert!(tasks.bring_forward(&direct, &tx));
        assert_eq!(lines(said(&mut rx)), [Some(ProxyStatus::Checking)]);
        check_alone(&direct, &tx, false);
        let answer = rx.recv().await.into_iter().collect();
        assert_eq!(lines(answer), [Some(ProxyStatus::Direct)]);

        // A fixture build's proxy passes without connecting anywhere.
        check_alone(&torrents(Some("socks5://127.0.0.1:1")), &tx, true);
        let answer = rx.recv().await.into_iter().collect();
        assert_eq!(lines(answer), [Some(ProxyStatus::Passed)]);

        // A bring-up waiting on the proxy checks now, and says what it found itself.
        let config = torrents(Some("socks5://127.0.0.1:1"));
        let check = Arc::clone(&tasks.check);
        let waiting = tokio::spawn(async move { check.notified().await });
        tasks.bring_up = Some(waiting);
        tokio::task::yield_now().await;
        assert!(!tasks.bring_forward(&config, &tx));
        assert_eq!(lines(said(&mut rx)), [Some(ProxyStatus::Checking)]);
        let woken =
            tokio::time::timeout(Duration::from_secs(5), tasks.bring_up.take().unwrap());
        woken.await.expect("woken").unwrap();

        // One that has given up waits on nothing.
        tasks.bring_up = Some(tokio::spawn(async {}));
        tokio::task::yield_now().await;
        while !tasks.bring_up.as_ref().unwrap().is_finished() {
            tokio::task::yield_now().await;
        }
        assert!(tasks.bring_forward(&config, &tx), "checked on its own");
    }

    #[tokio::test]
    async fn a_proxy_that_cannot_be_used_is_said_and_not_checked() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx: Outbox = tx.into();
        check_alone(&torrents(Some("socks5://127.0.0.1")), &tx, false);
        let mut said = said(&mut rx);
        let Some(Event::Toast(notice)) = said.pop() else { panic!("{said:?}") };
        assert_eq!(lines(said), [Some(ProxyStatus::Failed)]);
        assert!(notice.text.starts_with("The proxy setting can't be used."), "{notice:?}");
    }
}
