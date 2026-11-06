//! The engine as a front end holds it: started, sent commands, read for what happened, and shut
//! down, with one task behind it that owns everything else.
//!
//! [`Engine::start`] stands the engine up and spawns its actor: one task that owns the engine
//! side's state and every handle the work shares, and reads, in one place, the commands
//! [`Engine::send`] hands it, the inbox the work answers on, and torrent playback reporting in.
//! The front end reads what happened from the [`EventStream`], in the order the actor applied
//! it. Nothing else of the engine is the front end's to call, and nothing of the front end's
//! reaches the engine except as a command.
//!
//! Shaped for a front end on the other side of an FFI. Events are pulled, in batches, rather
//! than pushed through a callback; the one callback interface is the [`Host`]; and [`send`]
//! only puts the command on a channel, so a UI thread that is not async can call it.
//!
//! [`send`]: Engine::send

use kaeri_model::Msg;
use kaeri_model::StoreMsg;
use std::{
    collections::VecDeque,
    panic::{self, AssertUnwindSafe},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::FutureExt;
use kaeri_model::{
    command::{Command, PlayerCommand},
    event::Event,
    jobs::JobId,
    notice::{Notice, NoticeHint},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

#[cfg(feature = "desktop")]
use crate::playback::player::{launch_notice, player};
use crate::playback::{
    autonext::Follow,
    link::hosted::HostedPlayers,
    start::{opener_for, play_following},
};
use crate::{
    background::{datasets::keep_datasets_current, digest, prune},
    data,
    dispatch::{dispatch, handle_internal, receive},
    error::EngineError,
    host::Host,
    jobs::JobGuard,
    options::EngineOptions,
    outbox::{Envelope, Internal, Outbox, Played, WeakOutbox},
    playback::{autonext, prefetch::prefetch},
    plugin_stores,
    services::Services,
    settings,
    sources::{self, TorrentUp},
    state::EngineState,
    tracking, updates,
};

/// How long the exit waits for a playback to record where it was left.
const EXIT_FLUSH: Duration = Duration::from_secs(2);

/// How long the exit routes the commands still queued behind it, once the last position is
/// saved: enough for a setting changed just before quitting, and no more, however many wait.
const EXIT_DRAIN: Duration = Duration::from_millis(250);

/// The longest [`Engine::shutdown`] waits for the actor to wind down: the flush, the queue and
/// the torrents, with a step under way to finish first. Past it the app quits regardless.
const EXIT_WAIT: Duration = Duration::from_secs(5);

/// How long the exit gives the torrent session to stand down. librqbit's stop ends in a fixed
/// one-second wait after the work that matters, so this is what a quit with torrents costs.
const TORRENT_CLOSE: Duration = Duration::from_secs(1);

/// The engine, as a front end holds it: where its commands go.
///
/// Dropping it, or [`Self::shutdown`], ends the actor. What the actor had started runs on until
/// the runtime it runs on ends, as it always has when the app quits.
pub struct Engine {
    commands: mpsc::UnboundedSender<Message>,
    /// Where a shutdown is asked for: apart from the commands, so it is read ahead of every
    /// command still queued, however many a host has sent.
    stops: mpsc::UnboundedSender<oneshot::Sender<()>>,
    /// The players a front end hosts, which what they report goes straight to, as mpv's
    /// reports are read off its socket: not behind the commands waiting, nor held up by an
    /// engine winding down, which waits on the player for the episode's last position.
    hosted: Arc<HostedPlayers>,
    /// The novels open in a web view, read by its scheme handler without the actor.
    novels: Arc<crate::reading::novel::NovelBooks>,
    /// Jobs numbered so far, here as well as in the actor so [`Self::send`] can say at once
    /// which number the actor will announce. The actor numbers the jobs it starts on its own
    /// from the same count: see [`Jobs`].
    jobs: Jobs,
    /// Commands sent so far, which an idle engine's word is read against: see `Traced::Idle`.
    #[cfg(feature = "testing")]
    sent: AtomicU64,
    /// Casting's devices and the cast running now, which the TV picker and the cast's card read.
    #[cfg(feature = "cast")]
    casts: Arc<crate::cast::CastHub>,
    /// What the app tells sync about this device beside the commands: see
    /// [`crate::syncing::ThisDevice`].
    this_device: crate::syncing::ThisDevice,
}

/// [`Engine::send`] for the remote, held apart from the engine so the remote's server can keep
/// one. A remote sends no player reports, which only the hosted player makes, and is not counted
/// in `Engine::sent`: a test that reads `Traced::Idle` sends through the engine itself.
#[cfg(feature = "remote")]
#[derive(Clone)]
pub struct RemoteSender {
    commands: mpsc::UnboundedSender<Message>,
    jobs: Jobs,
}

/// A sender whose commands a test reads back, with no engine behind it.
#[cfg(all(test, feature = "cast"))]
impl RemoteSender {
    #[allow(
        irrefutable_let_patterns,
        reason = "Message has more variants with the testing feature"
    )]
    pub(crate) fn recording() -> (Self, impl FnMut() -> Vec<Command>) {
        let (commands, mut rx) = mpsc::unbounded_channel();
        let read = move || {
            let mut sent = Vec::new();
            while let Ok(message) = rx.try_recv() {
                if let Message::Command { command, .. } = message {
                    sent.push(command);
                }
            }
            sent
        };
        (Self { commands, jobs: Jobs::default() }, read)
    }
}

#[cfg(feature = "remote")]
impl RemoteSender {
    pub fn send(&self, command: Command) -> Option<JobId> {
        let job = command.activity().map(|activity| (self.jobs.next(), activity));
        let id = job.as_ref().map(|(id, _)| *id);
        self.commands.send(Message::Command { job, command }).ok()?;
        id
    }
}

/// What the front end hands the actor.
// Commands carry a search's whole query, and pass through here once each: not worth a box.
#[allow(clippy::large_enum_variant)]
enum Message {
    /// A command, with the job [`Engine::send`] numbered for it, and its activity.
    Command { job: Option<(JobId, String)>, command: Command },
    /// An event, as the work would send it: see [`Engine::inject`].
    #[cfg(feature = "testing")]
    Inject(Event),
    /// An event, as a playback would send it: see [`Engine::inject_from`].
    #[cfg(feature = "testing")]
    InjectFrom(Played, Event),
    /// The engine's record of the titles, as it stands: see [`Engine::title_cache`].
    #[cfg(feature = "testing")]
    TitleCache(oneshot::Sender<crate::cache::TitleCache>),
    /// Whether a playback is under way, as the exit would see it: see [`Engine::under_way`].
    #[cfg(feature = "testing")]
    UnderWay(oneshot::Sender<bool>),
    /// A step that panics, as a bug in one would: see [`Engine::panic_once`].
    #[cfg(feature = "testing")]
    Panic,
}

/// The count jobs are numbered by, from one: shared by [`Engine::send`], which numbers the
/// jobs of the commands sent, and the actor, which numbers the jobs it starts on its own.
#[derive(Clone, Default)]
struct Jobs(Arc<AtomicU64>);

impl Jobs {
    fn next(&self) -> JobId {
        JobId::new(self.0.fetch_add(1, Ordering::Relaxed) + 1)
    }
}

impl Engine {
    /// Stand the engine up, and start its actor.
    ///
    /// Does what the app's launch did on the engine side, in the same order: the sweeps of old
    /// files, the source chain and its plugins loading behind it, the handles the work shares,
    /// the update check and the airing digest, the plugin load's outcome said when it comes, the
    /// joined stores asked for their lists, the trackers' sync loops, the mapping datasets, torrent
    /// playback brought up, and the check of the player.
    ///
    /// What was true at launch is queued at the head of the [`EventStream`], for the front end
    /// to apply before its first frame: the settings, the joined stores, the chain's rows when
    /// it was given whole, the trackers' states, and what the check of the player found. That check runs
    /// mpv, so it is not waited for here: the front end can send its first commands while it
    /// runs, as the app did, and [`EventStream::launch`] waits for it. Everything else the
    /// engine says follows, in the order the actor applied it.
    ///
    /// Spawns on the tokio runtime it is called from, which has to outlive the engine.
    pub async fn start(
        options: EngineOptions,
        host: Arc<dyn Host>,
    ) -> Result<(Self, EventStream), EngineError> {
        let EngineOptions {
            paths,
            config,
            saved,
            http,
            store,
            player_theme,
            hosted_player,
            audio_tap,
            every_play_a_party,
            app_clients,
            device_label,
            handheld,
            phone,
            device_kind,
            language,
            sources: given,
            background,
            set_aside,
            reading,
            edition,
            #[cfg(feature = "testing")]
            parts,
            #[cfg(feature = "testing")]
            cloud_test,
            #[cfg(feature = "testing")]
            free_space,
            #[cfg(feature = "testing")]
            book_source,
        } = options;
        // Read before anything is written: no config file is a new install.
        let new_install = !paths.config_file.exists();
        if background.prune {
            // Old logs, stale covers and the player's leftovers, off the critical path.
            {
                let paths = paths.clone();
                tokio::task::spawn_blocking(move || prune::sweep_at_launch(&paths));
            }
            // With torrents off nothing else would ever prune what an earlier run streamed. With
            // them on, the session prunes on its way up, before it writes anything.
            if !config.providers.torrent.enabled {
                let (dir, limit) = (
                    paths.cache_dir.join("torrents"),
                    config.providers.torrent.cache_limit_bytes(),
                );
                let store = store.clone();
                tokio::spawn(async move {
                    prune::sweep_torrent_cache(dir, limit, &store, Default::default()).await;
                });
            }
        }

        let own_chain = given.is_none();
        // Book source plugins, loaded by the same background load while reading is available.
        let book_plugins = crate::book_plugins::BookPlugins::default();
        let (registry, plugins_loading) = match given {
            Some(registry) => (registry, None),
            None => {
                // Without torrent playback, which comes up behind the interface, once a proxy's
                // check passes when there is one: see `TorrentTasks::bring_up`.
                let registry = sources::build_registry_deferred(&config, &http);
                tracing::info!(providers = ?registry.ids(), "provider registry ready");
                // Plugins join the chain when they have finished compiling. Nothing waits on
                // this: the first frame is worth more than a source nobody has asked for yet.
                let loading = sources::spawn_plugin_load(
                    registry.clone(),
                    config.clone(),
                    http.clone(),
                    paths.clone(),
                    reading.then(|| book_plugins.clone()),
                );
                (registry, Some(loading))
            }
        };
        // A chain given whole too: a plugin switched off by name is in it, and not tried.
        registry.set_disabled(config.providers.disabled.iter().cloned());

        // The trackers as an app's own sign-in clients turn them on; the file keeps none of it.
        let tracked = crate::tracking::native::with_app_clients(&config, &app_clients);
        // What every task is lent, built around that chain, or from a test's parts.
        #[cfg(feature = "testing")]
        let trace = parts.is_some();
        #[cfg(feature = "testing")]
        let services = match parts {
            Some(parts) => {
                let (power, hosted) = crate::services::players(&paths);
                #[cfg(feature = "cast")]
                let casts = Arc::new(crate::cast::CastHub::in_memory(http.clone()));
                let servers = crate::servers::Servers::new(
                    kaeri_track::TokenStore::over(crate::host::file_secrets(&paths)),
                    store.clone(),
                    http.clone(),
                );
                let local = crate::local::Local::new(store.clone());
                let book_sources = crate::read_sources::BookSources::new(
                    store.clone(),
                    kaeri_track::TokenStore::over(crate::host::file_secrets(&paths)),
                    http.plain().clone(),
                    &paths,
                );
                Services {
                    book_sources,
                    tokens: kaeri_track::TokenStore::over(crate::host::file_secrets(&paths)),
                    syncing: Default::default(),
                    servers,
                    local,
                    power,
                    hosted,
                    hosted_player,
                    every_play_a_party,
                    paths,
                    http,
                    store,
                    meta: parts.meta,
                    registry,
                    sync: parts.sync,
                    stores: parts.stores,
                    slot: Default::default(),
                    offline: Default::default(),
                    connection: Arc::new(tokio::sync::watch::channel(false).0),
                    prefetch: Default::default(),
                    artwork: Default::default(),
                    player_theme,
                    app_clients: Default::default(),
                    host,
                    clock: parts.clock,
                    fixture_route: parts.fixture_route,
                    party: Default::default(),
                    skip_find: Default::default(),
                    book_plugins: Default::default(),
                    novels: Default::default(),
                    windows: parts.windows,
                    #[cfg(feature = "cast")]
                    casts,
                }
            }
            None => Services::build(&tracked, paths, http, store, registry, player_theme, host)
                .map_err(|reason| EngineError::PluginStores { reason })?,
        };
        #[cfg(not(feature = "testing"))]
        let services =
            Services::build(&tracked, paths, http, store, registry, player_theme, host)
                .map_err(|reason| EngineError::PluginStores { reason })?;
        // The player a play opens, mpv's process or the one the front end hosts, and the app's
        // own sign-in clients, where signing in will read them (E6).
        // And what a store joined with an invite calls this device unless told otherwise.
        let stores = services.stores.labelled(&device_label).reading(reading);
        #[cfg(feature = "testing")]
        let room_for_prints = free_space;
        #[cfg(not(feature = "testing"))]
        let room_for_prints = None;
        let skip_find = Arc::new(crate::skip_find::SkipFind::new(
            audio_tap && hosted_player,
            services.paths.data_dir.clone(),
            room_for_prints,
        ));
        let services = Services {
            hosted_player,
            every_play_a_party,
            app_clients,
            stores,
            skip_find,
            book_plugins,
            ..services
        };
        #[cfg(feature = "testing")]
        if let Some(source) = book_source {
            services.book_sources.registry.put(source);
        }
        #[cfg(feature = "testing")]
        if let Some(test) = cloud_test {
            services.syncing.cloud.use_test(test);
        }
        services.servers.join_chain(&services.registry, &config);
        services.local.put_source(&services.registry, &config.providers.order);
        let hosted = Arc::clone(&services.hosted);
        let novels = Arc::clone(&services.novels);
        #[cfg(feature = "cast")]
        let casts = Arc::clone(&services.casts);

        // Where torrent playback reports in once it is up, tagged with the bring-up it answers.
        let (arrivals_tx, arrivals) = mpsc::unbounded_channel();
        // The engine side's own state, on the config the app was launched with.
        let mut state = EngineState::new(config, saved, &services.paths, arrivals_tx);
        if let Some(overlay) = crate::profile::overlay_of(&services.store, &services.paths) {
            state.config.use_overlay(Some(overlay), &services.paths);
        }
        state.config.set_sync(services.store.clone());
        crate::tracking::commands::announce_signed_in(&services.store, &services.sync);
        state.config.set_handheld(handheld);
        state.config.set_phone(phone);
        let account = {
            use kaeri_store::sync::kinds::MEDIUM_ACCOUNT;
            let store = &services.store;
            store.sync_regs(MEDIUM_ACCOUNT).ok().and_then(|regs| {
                let (key, value) = regs.into_iter().next()?;
                Some((value, store.sync_reg_writer(MEDIUM_ACCOUNT, &key).ok().flatten()))
            })
        };
        state.reading = crate::reading::ReadingState::at_launch(
            reading,
            edition,
            state.config.effective().reading.kaeri_is_for.as_deref(),
            account.as_ref().map(|(answer, writer)| (answer.as_str(), writer.clone())),
            new_install,
        );
        crate::crash::arm_from(&services.paths.data_dir, state.config.effective());
        services.syncing.set_language(language.clone());
        #[cfg(feature = "cast")]
        {
            casts.set_language(language);
            casts.set_show_tvs(state.config.effective().cast.show_tvs);
        }
        #[cfg(not(feature = "cast"))]
        let _ = language;
        // With the counts of shows that set each setting for themselves, which the rows say.
        crate::show_settings::count(&mut state, &services.store);
        #[cfg(feature = "testing")]
        let room = free_space;
        #[cfg(not(feature = "testing"))]
        let room = None;
        state.ahead = crate::downloads::ahead::Ahead::new(background.keep_next, room);
        let mut launch = VecDeque::new();
        // The settings first: everything after is drawn under the config they say.
        state.config.set_skip_find_paused(services.skip_find.paused(state.config.effective()));
        launch.push_back(Event::Settings(state.config.view()));
        // Whether trailers may start, on that config, before the front end says anything.
        launch.push_back(crate::autoplay::at_launch(&mut state));
        // A database found damaged was moved aside and this one is new: why the history is empty.
        if let Some(aside) = set_aside {
            let path = aside.display().to_string();
            launch.push_back(Event::Toast(Notice::alert(Msg::NoticeDatabaseSetAside { path })));
        }
        // Joined stores as they were left, so Providers can count them on the first frame.
        let joined = match services.stores.rows() {
            Ok(rows) => {
                let joined = !rows.is_empty();
                launch.push_back(Event::Stores(rows));
                joined
            }
            Err(e) => {
                launch.push_back(Event::Toast(Notice::alert(e)));
                false
            }
        };
        // A chain given whole is final, so its rows are what launch says of it.
        if !own_chain {
            let rows = data::provider_rows(&services.registry, &services.stores);
            launch.push_back(Event::Providers(rows));
        }
        launch.push_back(Event::Demo(crate::demo::state(&state, &services)));

        // Background work reports through this, each piece holding its own sender, and the
        // engine side's messages to itself ride it too, each in its place among the events.
        let mut inbox = Inbox::default();

        if background.update_check {
            // The daily update check: one cached request, one quiet toast when a release exists.
            updates::spawn_check(
                state.config.effective().updates.check,
                state.config.effective().updates.beta,
                !services.hosted_player,
                &services.paths.cache_dir,
                &services.http,
                &inbox.sender(),
            );
        }
        if background.digest {
            // The airing digest: what aired for shows you watch since the last launch. Same
            // philosophy as the update check — one line, only when there is something to say.
            digest::spawn(
                state.config.effective().notifications.airing_digest,
                state.config.effective().notifications.desktop,
                services.store.clone(),
                services.meta.clone(),
                state.calendar.clone(),
                services.clock.clone(),
                services.host.clone(),
                inbox.sender(),
            );
        }
        if let Some(loading) = plugins_loading {
            say_plugin_load(loading, inbox.sender());
        }
        if background.store_refresh && joined {
            refresh_joined(services.stores.clone(), inbox.sender());
        }
        // Keep next lets go of what was watched, from the inbox rather than now, and queues
        // nothing: it tops a show up only as one of its episodes starts or finishes.
        if background.keep_next {
            let _ = inbox.sender().internal(Internal::LetGo);
        }

        crate::syncing::keep_syncing(&services, &inbox.sender());
        for tracker in services.sync.initial_states() {
            launch.push_back(Event::Sync(Box::new(tracker)));
        }
        // Home's first screen goes first: AniList's background work waits for it.
        services.meta.anilist().launching();
        // How titles are named: the setting, and under `anilist` the signed-in viewer's own
        // choice, which comes with the viewer lookup the library pull makes anyway.
        services.meta.titles().choose(state.config.effective().meta.title_language);
        if services.meta.titles().choice() == kaeri_core::config::TitleChoice::Anilist {
            // As the viewer read AniList when last asked, from the first frame, until the
            // viewer's answer says again.
            let kept = services.sync.anilist.as_ref().and_then(|a| a.kept_title_language());
            services.meta.titles().viewer_reads(kept);
            crate::title_language::follow_viewer(&services, &inbox.sender());
        }
        if background.sync_loops {
            tracking::spawn_loops(
                services.sync.clone(),
                services.meta.anilist().clone(),
                inbox.sender(),
            );
            // What waits for AniSkip from the last run, as the tracker queues are sent.
            if state.config.effective().playback.share_skip_times == Some(true) {
                crate::aniskip::Sharing::shared()
                    .kick(services.http.clone(), services.store.clone());
            }
        }
        if background.datasets {
            // Keep the mapping datasets current without ever blocking startup.
            tokio::spawn(keep_datasets_current(
                services.store.clone(),
                services.http.clone(),
                inbox.sender(),
            ));
        }

        // The torrent stack's live parts, held so a settings change can tear them down and
        // stand them back up without a restart. They are replaced together: the download
        // manager and seeding hold a session alive, and leaving one behind would mean traffic
        // outliving the source it belongs to. A chain
        // given whole has torrent playback in it already, or none.
        if own_chain {
            let tx = inbox.sender();
            let EngineState { config, torrent_tasks, arrivals, .. } = &mut state;
            torrent_tasks.bring_up(config.effective(), &services, &tx, arrivals);
        }

        // The player is checked last, as launch always has, and not waited for. Only the
        // terminal app plays in mpv; an app hosting its own player has nothing to install.
        #[cfg(feature = "desktop")]
        let checking = (background.player_check && !services.hosted_player).then(|| {
            let config = state.config.effective();
            let mpv = player(config, &services.paths, &services.player_theme);
            let binary = config.playback.mpv_binary.clone();
            tokio::spawn(async move { launch_notice(&mpv, &binary).await.map(Event::Toast) })
        });
        // Without the desktop's player there is none to check.
        #[cfg(not(feature = "desktop"))]
        let checking = None;

        let (commands_tx, commands) = mpsc::unbounded_channel();
        let (stops_tx, stops) = mpsc::unbounded_channel();
        let (out, live) = mpsc::unbounded_channel();
        let jobs = Jobs::default();
        // Commands sync's handle sends itself, while the engine runs: not one that keeps it up.
        let weak = commands_tx.downgrade();
        let this_device = crate::syncing::ThisDevice::of(&services, move |command| {
            if let Some(commands) = weak.upgrade() {
                let _ = commands.send(Message::Command { job: None, command });
            }
        });
        if let Some(kind) = device_kind {
            this_device.set_kind(kind);
        }
        let actor = Actor {
            state,
            services,
            inbox,
            commands,
            stops,
            arrivals,
            out,
            jobs: jobs.clone(),
            #[cfg(feature = "testing")]
            routed: 0,
            #[cfg(feature = "testing")]
            trace,
            #[cfg(feature = "testing")]
            injected: Default::default(),
        };
        tokio::spawn(actor.run());

        let engine = Self {
            commands: commands_tx,
            stops: stops_tx,
            jobs,
            hosted,
            novels,
            #[cfg(feature = "cast")]
            casts,
            this_device,
            #[cfg(feature = "testing")]
            sent: AtomicU64::new(0),
        };
        Ok((engine, EventStream { launch, checking, live }))
    }

    /// A web view's request for a file of the open novel: the scheme handler's answer, read on
    /// the caller's thread. `None` for a URL that is no book's.
    pub fn book_resource(&self, url: &str) -> Option<kaeri_model::reader::BookResource> {
        self.novels.resource(url)
    }

    /// The open novels, for a scheme handler that keeps them apart from the engine.
    pub fn novels(&self) -> Arc<crate::reading::novel::NovelBooks> {
        Arc::clone(&self.novels)
    }

    /// Hand the engine a command, and say which job it starts, for work someone waits on.
    ///
    /// Only puts it on the actor's channel, so any thread may call this, async or not. The job
    /// is numbered here, once each, from one, and the actor announces it with
    /// [`Event::JobStarted`] before the command is routed, so the announcement comes ahead of
    /// everything the work says. Commands are routed one at a time in the order they are sent,
    /// and whatever one of them does before the next is routed, such as a setting written, is
    /// done for the next to see.
    ///
    /// `None` too once the engine has stopped, when nothing would announce the job.
    pub fn send(&self, command: Command) -> Option<JobId> {
        if let Command::PlayerReport { playback, report } = command {
            self.hosted.report(playback, report);
            return None;
        }
        let job = command.activity().map(|activity| (self.jobs.next(), activity));
        let id = job.as_ref().map(|(id, _)| *id);
        #[cfg(feature = "testing")]
        self.sent.fetch_add(1, Ordering::Relaxed);
        self.commands.send(Message::Command { job, command }).ok()?;
        id
    }

    /// What a TV's remote sends its commands through: routed as [`Self::send`] routes them, in
    /// the same order as the front end's own, and numbered from the same count. See
    /// [`crate::remote`].
    /// Casting's devices, what the engine learned of each TV, and the cast running now.
    #[cfg(feature = "cast")]
    pub fn casts(&self) -> Arc<crate::cast::CastHub> {
        Arc::clone(&self.casts)
    }

    /// What sync needs to hear about this device from the app rather than by command: its kind,
    /// its remote's key, the network's DNS-SD, and whether a phone may drive this TV.
    pub fn this_device(&self) -> crate::syncing::ThisDevice {
        self.this_device.clone()
    }

    #[cfg(feature = "remote")]
    pub fn remote_sender(&self) -> RemoteSender {
        RemoteSender { commands: self.commands.clone(), jobs: self.jobs.clone() }
    }

    /// Stop as the app quits, ahead of the commands still queued.
    ///
    /// A playback under way is stopped, the way `x` stops it, and given a moment to record
    /// where it was left: it writes its last history row before it reports its ending, which
    /// is what this waits for. The tracker writes a row every ten seconds of playback, so
    /// quitting without this would lose up to that much of the resume point. Under way is the
    /// playback Now Playing shows, started and not yet ended, as the engine keeps track of it:
    /// one still finding its source has nothing to save, and its wait would only delay the
    /// exit.
    ///
    /// Then the torrent session, if there is one, is stood down in order: each torrent paused
    /// so the next launch knows how far it got, then its tasks and connections stopped. That
    /// is bounded at a second, and with torrents off there is nothing to wait for.
    ///
    /// It is read ahead of the commands still queued, and the last position is saved before
    /// any of them is routed: they get a moment, [`EXIT_DRAIN`], and the rest are let go. The
    /// whole of it is held to [`EXIT_WAIT`].
    pub async fn shutdown(self) {
        self.stopping().await;
    }

    /// [`Self::shutdown`], asked for now and waited on in what it returns, with the engine kept:
    /// a front end whose player reports through [`Self::send`] keeps sending while the last
    /// position is saved, and that save waits on the player's report.
    pub fn stopping(&self) -> impl Future<Output = ()> + Send + 'static {
        let (done, stopped) = oneshot::channel();
        let asked = self.stops.send(done).is_ok();
        async move {
            if asked && tokio::time::timeout(EXIT_WAIT, stopped).await.is_err() {
                tracing::warn!("the engine did not wind down in time; quitting regardless");
            }
        }
    }

    /// Commands sent so far: what [`Traced::Idle`] is read against.
    #[cfg(feature = "testing")]
    pub fn sent(&self) -> u64 {
        self.sent.load(Ordering::Relaxed)
    }

    /// Hand the engine an event as its work would send it, after everything sent before: what a
    /// playback says, for a test that has no player. Counted as a command, so the engine falls
    /// idle only once it has done its part with the event and passed it on.
    #[cfg(feature = "testing")]
    pub fn inject(&self, event: Event) {
        self.sent.fetch_add(1, Ordering::Relaxed);
        let _ = self.commands.send(Message::Inject(event));
    }

    /// [`Self::inject`], as a playback of `episode` of `id` would send it, named with that
    /// playback: `playback` numbers it, from a million up so that no playback the engine
    /// starts is mistaken for it. The first event under a number starts it.
    #[cfg(feature = "testing")]
    pub fn inject_from(
        &self,
        playback: u64,
        id: kaeri_core::ids::AnilistId,
        episode: &str,
        event: Event,
    ) {
        self.sent.fetch_add(1, Ordering::Relaxed);
        let played = Played {
            serial: kaeri_model::playback::PlaybackId::new(playback),
            id,
            episode: episode.to_owned(),
        };
        let _ = self.commands.send(Message::InjectFrom(played, event));
    }

    /// The engine's record of the titles, as it stands when the engine reads the request:
    /// after every command sent before it is routed. For a test that has settled, everything
    /// they led to is in it.
    #[cfg(feature = "testing")]
    pub async fn title_cache(&self) -> crate::cache::TitleCache {
        let (tx, rx) = oneshot::channel();
        let _ = self.commands.send(Message::TitleCache(tx));
        rx.await.expect("the engine answers while it runs")
    }

    /// Whether a playback is under way, as [`Self::shutdown`] would see it now: the one Now
    /// Playing shows, started and not ended. Read once every command sent before it is routed.
    #[cfg(feature = "testing")]
    pub async fn under_way(&self) -> bool {
        let (tx, rx) = oneshot::channel();
        let _ = self.commands.send(Message::UnderWay(tx));
        rx.await.expect("the engine answers while it runs")
    }

    /// Have the engine's loop run a step that panics, in its place among the commands.
    #[cfg(feature = "testing")]
    pub fn panic_once(&self) {
        let _ = self.commands.send(Message::Panic);
    }
}

/// Say whatever the background plugin load concluded, once. A plugin that failed to load must
/// not be silent: the Providers screen is where a missing source gets explained.
fn say_plugin_load(loading: JoinHandle<Option<Msg>>, tx: Outbox) {
    tokio::spawn(async move {
        if let Ok(outcome) = loading.await {
            match outcome {
                Some(note) => {
                    // Plugins skipped for an old ABI are said, but the rest did arrive.
                    let arrived =
                        !matches!(note, Msg::Store(StoreMsg::PluginsLoadFailed { .. }));
                    let _ = tx.send(Event::ProviderNote(note));
                    if arrived {
                        let _ = tx.internal(Internal::SourcesReordered);
                    }
                }
                // Ranked again by the order as it stands now, since the user may have moved a
                // source while these compiled, and the Providers screen refreshed so late
                // arrivals actually appear on it.
                None => {
                    let _ = tx.internal(Internal::SourcesReordered);
                }
            }
        }
    });
}

/// Ask each joined store for its current list, once, in the background. Updates are said once
/// at launch, the way a new release is, and never installed without being asked. Asked of the
/// rows launch said, which the front end was given whole.
fn refresh_joined(stores: plugin_stores::Stores, tx: Outbox) {
    tokio::spawn(async move {
        if let Ok(rows) = stores.refresh().await {
            let _ = tx.send(Event::Stores(rows));
        }
        let updates = stores.pending_updates();
        // Where to install them is the front end's to say: the terminal's Providers, Enter.
        if !updates.is_empty() {
            let notice = Notice::info(Msg::Store(StoreMsg::PluginUpdatesReady {
                count: updates.len() as i64,
                names: updates.join(", "),
            }));
            let _ = tx.send(Event::Toast(notice.with_hint(NoticeHint::OpenStores)));
        }
    });
}

/// What happened, as a front end reads it: what launch found, then every event in the order
/// the engine applied it.
///
/// Pulled rather than pushed, so a front end reads it when it is ready to apply what it reads,
/// on its own thread: [`Self::next_events`] waits for a batch, [`Self::try_next_events`] takes
/// what is there, and [`Self::recv`] takes one at a time, for a loop that selects over it.
pub struct EventStream {
    /// What launch found, not yet taken.
    launch: VecDeque<Event>,
    /// The check of the player, the last of what launch finds, until it is taken.
    checking: Option<JoinHandle<Option<Event>>>,
    /// Everything since, from the actor.
    live: mpsc::UnboundedReceiver<Out>,
}

/// What the actor puts on the stream.
// Events carry a search's whole query, and pass through here once each: not worth a box.
#[allow(clippy::large_enum_variant)]
enum Out {
    /// For the front end.
    Event(Event),
    /// A message the engine side left itself was handled here, after what it said: see
    /// [`Traced::Handled`].
    #[cfg(feature = "testing")]
    Handled(Internal),
    /// Nothing left to do: see [`Traced::Idle`].
    #[cfg(feature = "testing")]
    Idle(u64),
}

impl Out {
    /// The event, for a front end: what only a test wants to hear is passed over.
    fn event(self) -> Option<Event> {
        match self {
            Self::Event(event) => Some(event),
            #[cfg(feature = "testing")]
            Self::Handled(_) | Self::Idle(_) => None,
        }
    }
}

impl EventStream {
    /// What was true at launch, whole: the joined stores, the chain's rows when it was given
    /// whole, the trackers' states, and what the check of the player found, which this waits
    /// for. For the front end to apply before its first frame, so the first frame shows the
    /// app as it was launched. Nothing said since is in it; empty once taken.
    pub async fn launch(&mut self) -> Vec<Event> {
        if let Some(checking) = self.checking.take()
            && let Ok(Some(notice)) = checking.await
        {
            self.launch.push_back(notice);
        }
        self.launch.drain(..).collect()
    }

    /// The next event, waiting for one: launch's first, then everything since. `None` once the
    /// engine has stopped.
    ///
    /// Cancel-safe, for a loop that selects over it beside its own events: an event is taken
    /// only when it is returned.
    pub async fn recv(&mut self) -> Option<Event> {
        if let Some(event) = self.launch.pop_front() {
            return Some(event);
        }
        if let Some(checking) = &mut self.checking {
            let found = checking.await;
            self.checking = None;
            if let Ok(Some(event)) = found {
                return Some(event);
            }
        }
        while let Some(out) = self.live.recv().await {
            if let Some(event) = out.event() {
                return Some(event);
            }
        }
        None
    }

    /// The events there are, waiting for at least one: a batch for a front end to apply in
    /// one go. Empty once the engine has stopped.
    pub async fn next_events(&mut self) -> Vec<Event> {
        let Some(first) = self.recv().await else { return Vec::new() };
        let mut events = vec![first];
        events.extend(self.try_next_events());
        events
    }

    /// The events there are, without waiting: what launch found, as far as it is known, and
    /// then everything since. Nothing since launch comes before the check of the player has
    /// been taken, so the order holds however this is called.
    pub fn try_next_events(&mut self) -> Vec<Event> {
        let mut events: Vec<Event> = self.launch.drain(..).collect();
        if let Some(checking) = &mut self.checking {
            if !checking.is_finished() {
                return events;
            }
            if let Some(Ok(Some(event))) = futures::FutureExt::now_or_never(checking) {
                events.push(event);
            }
            self.checking = None;
        }
        while let Ok(out) = self.live.try_recv() {
            events.extend(out.event());
        }
        events
    }

    /// The next thing the engine says, with the two things only a test wants to hear: each
    /// message the engine side left itself, in the place it was handled, and the moment it had
    /// nothing left to do. Only an engine started with [`crate::options::Parts`] says them.
    #[cfg(feature = "testing")]
    pub async fn next_traced(&mut self) -> Option<Traced> {
        if let Some(event) = self.launch.pop_front() {
            return Some(Traced::Event(event));
        }
        Some(match self.live.recv().await? {
            Out::Event(event) => Traced::Event(event),
            Out::Handled(message) => Traced::Handled(message),
            Out::Idle(routed) => Traced::Idle { routed },
        })
    }
}

/// What an engine started from a test's parts says: its events, and two things beside them.
#[cfg(feature = "testing")]
// Events carry a search's whole query, and pass through here once each: not worth a box.
#[allow(clippy::large_enum_variant)]
pub enum Traced {
    /// An event, as any front end reads it.
    Event(Event),
    /// A message the engine side left itself, handled after everything it said was passed on,
    /// and before anything after it.
    Handled(Internal),
    /// The engine had nothing left to do once it had routed this many commands: none queued,
    /// nothing in its inbox, and no work holding a way to answer. So everything those commands
    /// led to has been said, ahead of this; once `routed` is every command sent, there is no
    /// more to wait for. Said each time the engine falls idle.
    Idle { routed: u64 },
}

/// The engine's own task: the state, the handles, and everything it reads.
struct Actor {
    state: EngineState,
    services: Services,
    inbox: Inbox,
    commands: mpsc::UnboundedReceiver<Message>,
    /// Shutdowns asked for: see [`Engine::stopping`].
    stops: mpsc::UnboundedReceiver<oneshot::Sender<()>>,
    arrivals: mpsc::UnboundedReceiver<(u64, Result<TorrentUp, Msg>)>,
    out: mpsc::UnboundedSender<Out>,
    /// The count the jobs it starts on its own are numbered by: see [`Jobs`]. Only an ending
    /// that rolls on leads it to start any.
    jobs: Jobs,
    /// Commands routed so far.
    #[cfg(feature = "testing")]
    routed: u64,
    /// Whether to say what only a test wants to hear: see [`Traced`].
    #[cfg(feature = "testing")]
    trace: bool,
    /// The playbacks a test has spoken for, by number: see [`Engine::inject_from`].
    #[cfg(feature = "testing")]
    injected: std::collections::HashSet<kaeri_model::playback::PlaybackId>,
}

impl Actor {
    /// Read and act until the front end lets go of the engine or shuts it down.
    ///
    /// Commands first, then the inbox, then torrent playback: a command is routed as soon as
    /// it is sent, as the loop dispatched it in the same pass that produced it, and what a
    /// bring-up says on its way up is in the inbox before its arrival is.
    async fn run(mut self) {
        loop {
            // Each pass follows one thing done, so this is said once each time the engine
            // falls idle.
            #[cfg(feature = "testing")]
            if self.trace && self.idle() {
                let _ = self.out.send(Out::Idle(self.routed));
            }
            tokio::select! {
                biased;
                // Ahead of the commands, however many are queued: the app is quitting.
                Some(done) = self.stops.recv() => {
                    self.stop().await;
                    let _ = done.send(());
                    return;
                }
                message = self.commands.recv() => match message {
                    Some(Message::Command { job, command }) => {
                        let routed = panic::catch_unwind(AssertUnwindSafe(|| {
                            self.route(job, command)
                        }));
                        self.carry_on("a command", routed);
                    }
                    #[cfg(feature = "testing")]
                    Some(Message::Inject(event)) => {
                        let _ = self.inbox.sender().send(event);
                        self.routed += 1;
                    }
                    #[cfg(feature = "testing")]
                    Some(Message::InjectFrom(played, event)) => {
                        if self.injected.insert(played.serial) {
                            self.started(&played);
                        }
                        let _ = self.inbox.sender().for_playback(played).send(event);
                        self.routed += 1;
                    }
                    #[cfg(feature = "testing")]
                    Some(Message::TitleCache(answer)) => {
                        let _ = answer.send(self.state.cache.clone());
                    }
                    #[cfg(feature = "testing")]
                    Some(Message::UnderWay(answer)) => {
                        let _ = answer.send(self.under_way());
                    }
                    #[cfg(feature = "testing")]
                    Some(Message::Panic) => {
                        let step = panic::catch_unwind(|| panic!("a step that panics"));
                        self.carry_on("a test's step", step);
                    }
                    // Every handle on the engine has gone.
                    None => return,
                },
                envelope = self.inbox.recv(), if !self.inbox.is_empty() => {
                    if let Some(envelope) = envelope {
                        let acted = AssertUnwindSafe(self.act_on(envelope)).catch_unwind().await;
                        self.carry_on("the inbox", acted);
                    }
                }
                Some((generation, outcome)) = self.arrivals.recv() => {
                    let arrived = panic::catch_unwind(AssertUnwindSafe(|| {
                        self.arrived(generation, outcome)
                    }));
                    self.carry_on("a torrent source", arrived);
                }
            }
        }
    }

    /// After one step of the loop: a step that panicked is logged and said, and the engine
    /// carries on with the next, rather than every screen waiting on an engine that has gone.
    /// What the step left half done stays so; the alert says what to do if that shows.
    fn carry_on(&self, step: &str, outcome: std::thread::Result<()>) {
        let Err(panic) = outcome else { return };
        let said = panic
            .downcast_ref::<&str>()
            .map(|said| (*said).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        tracing::error!(step, panic = %said, "an engine step panicked; carrying on");
        self.pass_on(Event::Toast(Notice::alert(Msg::NoticeEngineFault)));
    }

    /// No command queued, nothing in the inbox, no work holding a way to answer, and no torrent
    /// source reporting in.
    #[cfg(feature = "testing")]
    fn idle(&self) -> bool {
        self.inbox.is_empty() && self.commands.is_empty() && self.arrivals.is_empty()
    }

    /// Route a command, its job announced first: everything the command does before it returns
    /// is done before the next is routed.
    ///
    /// While offline, what it loads is noted; once a command brings the network back, what
    /// was noted is asked for again, each as its own job: see [`crate::connectivity`].
    fn route(&mut self, job: Option<(JobId, String)>, command: Command) {
        let tx = self.inbox.sender();
        if self.state.network.offline {
            self.state.missed.note(&command);
        }
        let job = job.map(|(id, activity)| JobGuard::announce(id, activity, &tx));
        dispatch(command, job, &mut self.state, &self.services, &tx);
        for again in self.state.missed.take_due() {
            let job = again
                .activity()
                .map(|activity| JobGuard::announce(self.jobs.next(), activity, &tx));
            dispatch(again, job, &mut self.state, &self.services, &tx);
        }
        self.refresh_up_next();
        self.refresh_home();
        #[cfg(feature = "testing")]
        {
            self.routed += 1;
        }
    }

    /// "Up next for you" sent again when the Continue queue is not what it was when it was last
    /// sent, once it has been asked for: its room depends on the rows above it. Cut to the room
    /// by the load, so a front end never trims it.
    fn refresh_up_next(&mut self) {
        let Some(sent) = self.state.up_next_queue.as_ref() else {
            return;
        };
        if *sent == crate::catalog::up_next::queue_mark(&self.services.store) {
            return;
        }
        let tx = self.inbox.sender();
        dispatch(Command::LoadUpNext, None, &mut self.state, &self.services, &tx);
    }

    /// Home sent again when Continue is not where it stood when Home was last loaded, once Home
    /// has been asked for: an episode watched, a show started or finished, a row removed. The
    /// load sends it whole, with the spotlight and every shelf as they now stand.
    fn refresh_home(&mut self) {
        let Some(sent) = self.state.home_mark else {
            return;
        };
        if sent == crate::catalog::home::queue_mark(&self.services.store) {
            return;
        }
        let tx = self.inbox.sender();
        dispatch(
            Command::LoadHome { refresh: false },
            None,
            &mut self.state,
            &self.services,
            &tx,
        );
    }

    /// One item of the inbox, in its place.
    async fn act_on(&mut self, envelope: Envelope) {
        match envelope {
            Envelope::Event(event) => self.forward(event, None),
            Envelope::Played(played, event) => self.forward(event, Some(played)),
            // What the engine side left itself to do, after everything sent before it and
            // before anything sent after.
            Envelope::Internal(message) => {
                let tx = self.inbox.sender();
                let said = handle_internal(message, &mut self.state, &self.services, &tx).await;
                said.into_iter().for_each(|said| self.pass_on(said));
                #[cfg(feature = "testing")]
                if self.trace {
                    let _ = self.out.send(Out::Handled(message));
                }
            }
        }
    }

    /// An event on its way to the front end, from the playback `played` when one said it: the
    /// engine side's part in it, and what it has to say ahead of it.
    ///
    /// A title the event moves on — a tick of the playhead, an ending, a window handed over —
    /// is said whole just before it, so that whatever the event brings up, the screen a
    /// finished episode drops back to among them, shows the title as it now stands.
    fn forward(&mut self, mut event: Event, played: Option<Played>) {
        // Named as the titles are shown now, before the record takes the names in.
        crate::title_language::retitle(&mut event, self.services.meta.titles().language());
        // A show whose episode starts, or is finished: keep next tops that show up.
        let kept = match &event {
            Event::PlaybackStaged { id, .. } | Event::CastStaged { id, .. } => Some(*id),
            _ => {
                played.as_ref().filter(|_| crate::downloads::ahead::after(&event)).map(|p| p.id)
            }
        };
        // Where a viewer is in a show: what the Continue queue is made of.
        let queue_moves = matches!(
            event,
            Event::Progress { .. } | Event::ProgressQueued | Event::PlaybackEnded { .. }
        );
        // A table no source could list: its show is not one to download ahead.
        self.state.ahead.observe(&event);
        // A list that failed while offline: loaded again once back.
        if self.state.network.offline {
            self.state.missed.observe(&event);
        }
        let Self { services, inbox, .. } = self;
        let said = receive(&event, services, || inbox.sender());
        said.into_iter().for_each(|said| self.pass_on(said));
        // What it says of a title, taken into the record, and what that moved.
        let moved = self.state.cache.observe(&event, played.as_ref());
        if let Some(progress) = moved.and_then(|id| self.state.cache.progress(id)) {
            self.pass_on(progress);
        }
        // What the player changed of the config, kept and saved, with what the save has to say
        // coming after the event, in its place in the inbox.
        let Self { services, inbox, state, .. } = self;
        settings::remember(&event, state, services, || inbox.sender());
        // What it leads to on its own, started now and said right after it, so nothing the
        // front end reads comes between an ending and the episode it rolls on to.
        let watched = played.clone().filter(|_| matches!(event, Event::ProgressQueued));
        let after = self.follow(&mut event, played.as_ref());
        self.pass_on(event);
        after.into_iter().for_each(|said| self.pass_on(said));
        // An episode counted as watched: its discussion opens from now on, and is looked up.
        if let Some(played) = watched.as_ref() {
            let tx = self.inbox.sender();
            crate::discussion::watched(
                played.id,
                &played.episode,
                &self.state,
                &self.services,
                &tx,
            );
        }
        if queue_moves {
            self.refresh_up_next();
            self.refresh_home();
        }
        // An episode started or finished: the episodes after where the viewer is, kept.
        if let Some(id) = kept {
            let tx = self.inbox.sender();
            crate::downloads::ahead::top_up(id, &self.state, &self.services, &tx);
        }
    }

    /// Start what a playback's event leads to — the next episode an ending rolls on to, an
    /// episode picked in the player, the episode restarted in the other translation, the next
    /// episode's streams found ahead — and return what to say of it after the event: see
    /// [`autonext`]. An ending that rolls on is said to.
    ///
    /// A play started here is a job like one the front end asks for, numbered from the same
    /// count, and staged like one: it is announced, then said as `PlaybackStaged`.
    fn follow(&mut self, event: &mut Event, played: Option<&Played>) -> Vec<Event> {
        let finished = self.state.staging.observe(event, played);
        // The room starts or moves on: this device opens the episode from its own sources.
        #[cfg(feature = "party")]
        if let Event::PartyStarted { id, episode, .. } = event {
            let tx = self.inbox.sender();
            crate::playback::start::play_party(
                *id,
                episode.clone(),
                None,
                &mut self.state,
                &self.services,
                &tx,
            );
        }
        if let Event::PrefetchNext { id, episode } = event {
            prefetch(*id, episode.clone(), &self.state, &self.services);
            return Vec::new();
        }
        // As the show that ended has it: a show can skip filler, or not, for itself.
        let effective = self.state.config.effective();
        let skip_filler = match &finished {
            Some(staged) => {
                let store = &self.services.store;
                crate::show_settings::for_show(effective, store, staged.id).playback.skip_filler
            }
            None => effective.playback.skip_filler,
        };
        let (mut said, follow) = autonext::follow(
            event,
            finished,
            self.state.staging.front(),
            &self.state.cache,
            skip_filler,
        );
        let Some(follow) = follow else { return said };
        said.extend(self.roll_on(follow, event));
        said
    }

    /// Play what `event` led to, as `follow` says, and return what it was staged as. An ending
    /// that rolls on is said to.
    ///
    /// Where this build has no player to open, nothing rolls on: what the rules would follow
    /// with is let go, and nothing is said of it.
    fn roll_on(&mut self, follow: Follow, event: &mut Event) -> Option<Event> {
        let opener = opener_for(&self.state, &self.services)?;
        if let Event::PlaybackEnded { rolling_on, .. } = event {
            *rolling_on = true;
        }
        let tx = self.inbox.sender();
        let play = Command::play(follow.id, follow.episode.clone());
        let job =
            play.activity().map(|activity| JobGuard::announce(self.jobs.next(), activity, &tx));
        Some(play_following(follow, opener, job, &mut self.state, &self.services, &tx))
    }

    /// Whether a playback is under way: the one Now Playing shows has started playing, and
    /// has not ended. One still finding its stream has nothing to record.
    fn under_way(&self) -> bool {
        let EngineState { staging, cache, .. } = &self.state;
        staging.front().is_some_and(|front| cache.under_way(front.serial))
    }

    /// A playback a test speaks for has started, as a play the front end asked for starts
    /// one: the record takes what it says from now, and Now Playing shows it.
    #[cfg(feature = "testing")]
    fn started(&mut self, played: &Played) {
        use crate::{cache::PlayKind, playback::autonext::Staged};
        let EngineState { cache, staging, .. } = &mut self.state;
        cache.started(played);
        let title = cache.name_for(played.id, PlayKind::Staged);
        cache.announce(played.id, title.clone());
        staging.staged(Staged {
            serial: played.serial,
            id: played.id,
            episode: played.episode.clone(),
            title,
            episode_title: cache.episode_title(played.id, &played.episode),
        });
    }

    /// Torrent playback is up, or will not come up; only the latest bring-up counts.
    fn arrived(&mut self, generation: u64, outcome: Result<TorrentUp, Msg>) {
        let tx = self.inbox.sender();
        let crate::phases::Gate { hold, refusal } = crate::phases::gate_now(&mut self.state);
        let EngineState { torrent_tasks, torrent_route, shared_config, .. } = &mut self.state;
        torrent_tasks.arrived(
            generation,
            outcome,
            torrent_route,
            &self.services,
            shared_config,
            &tx,
        );
        // Up while the app is away, on cellular or in Low Data Mode: what waits, waits from the
        // start, and what may not start does not.
        if let Some((_, session)) = torrent_route.clone() {
            session.refuse_on_this_network(refusal);
            if hold != kaeri_providers::torrent::session::Hold::NOTHING {
                tokio::spawn(async move {
                    let _ = session.set_hold(hold).await;
                });
            }
        }
    }

    fn pass_on(&self, mut event: Event) {
        crate::title_language::retitle(&mut event, self.services.meta.titles().language());
        let _ = self.out.send(Out::Event(event));
    }

    /// Everything the app does on its way out: the playback's last record, then the commands
    /// queued before the quit for [`EXIT_DRAIN`], then the torrents.
    async fn stop(&mut self) {
        let playing = self.under_way();
        self.flush_playback(playing).await;
        self.drain_commands();
        self.close_torrents().await;
    }

    /// Route what is queued, in order, until the queue is empty or [`EXIT_DRAIN`] is up. A
    /// setting changed as the app quit is written; a flood of commands is let go.
    fn drain_commands(&mut self) {
        let deadline = std::time::Instant::now() + EXIT_DRAIN;
        while let Ok(message) = self.commands.try_recv() {
            // A command is all there is outside a testing build.
            #[cfg_attr(not(feature = "testing"), allow(irrefutable_let_patterns))]
            if let Message::Command { job, command } = message {
                let routed = panic::catch_unwind(AssertUnwindSafe(|| self.route(job, command)));
                self.carry_on("a command", routed);
            }
            if std::time::Instant::now() >= deadline {
                tracing::warn!("commands still queued as the app quit were let go");
                return;
            }
        }
    }

    /// Stand the torrent session down within [`TORRENT_CLOSE`], then let its route go.
    async fn close_torrents(&mut self) {
        let closing = self.state.torrent_tasks.close();
        if tokio::time::timeout(TORRENT_CLOSE, closing).await.is_err() {
            tracing::debug!("the torrent session was still winding down as the app quit");
        }
        self.state.torrent_route = None;
    }

    /// Stop what is playing, if it is under way, and wait briefly for it to record where it
    /// ended. What the inbox holds meanwhile is let go, unread, as the app quits, but for what
    /// a hosted player is told: its quit is what makes it report the end this waits for.
    async fn flush_playback(&mut self, playing: bool) {
        let Some(controls) = self.state.player_tx.as_ref().filter(|_| playing) else { return };
        if controls.send(PlayerCommand::Stop).is_err() {
            return;
        }
        let ended = async {
            while let Some(envelope) = self.inbox.recv().await {
                let (Envelope::Event(event) | Envelope::Played(_, event)) = envelope else {
                    continue;
                };
                match event {
                    Event::PlaybackEnded { .. } => break,
                    Event::PlayerOps { .. } | Event::PlayerUi { .. } => {
                        let _ = self.out.send(Out::Event(event));
                    }
                    _ => {}
                }
            }
        };
        if tokio::time::timeout(EXIT_FLUSH, ended).await.is_err() {
            tracing::warn!("the player did not stop in time; its last position may be lost");
        }
    }
}

/// The engine side's inbox: the channels the work answers on, read in the order they opened.
///
/// The actor holds no sender of its own between one thing and the next, only a weak one, and
/// lends a strong one to each piece of work. So a channel closes exactly when the last work
/// holding it ends, and an inbox with every channel closed and read is an engine with nothing
/// outstanding: what [`Traced::Idle`] says, with no waiting for a quiet moment. Work started
/// after a channel closed is lent a new one, read after it: everything on the old one was sent
/// before the new one existed, so the order holds.
#[derive(Default)]
struct Inbox {
    open: VecDeque<mpsc::UnboundedReceiver<Envelope>>,
    newest: Option<WeakOutbox>,
}

impl Inbox {
    /// A way to answer: onto the newest channel while it is open, or onto a new one.
    fn sender(&mut self) -> Outbox {
        if let Some(tx) = self.newest.as_ref().and_then(WeakOutbox::upgrade) {
            return tx;
        }
        let (tx, rx) = Outbox::inbox();
        self.newest = Some(tx.downgrade());
        self.open.push_back(rx);
        tx
    }

    /// Every channel closed and read: no work holds a way to answer, and nothing is unread.
    fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// The next item, from the oldest channel still open or unread. `None` once there is none.
    ///
    /// Cancel-safe: a channel is let go only once it is closed and read.
    async fn recv(&mut self) -> Option<Envelope> {
        while let Some(oldest) = self.open.front_mut() {
            match oldest.recv().await {
                Some(envelope) => return Some(envelope),
                None => {
                    self.open.pop_front();
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_core::config::{Config, Paths};
    #[cfg(feature = "party")]
    use kaeri_model::party::PartyCommand;
    use kaeri_model::playback::PlayerTheme;
    use kaeri_model::{Cause, Msg};
    use kaeri_net::HttpClient;
    use kaeri_providers::ProviderRegistry;

    use crate::host::{QuietHost, SecretError, SecretStore, Storage};

    /// An engine on the defaults, in `dir`, with no sources, doing nothing it is not asked to,
    /// and keeping its tokens in files there rather than in the keychain.
    async fn quiet_engine(dir: &std::path::Path) -> (Engine, EventStream) {
        engine_on(dir, Arc::new(QuietHost::in_dir(&dir.join("tokens")))).await
    }

    /// [`quiet_engine`], on `host`.
    async fn engine_on(dir: &std::path::Path, host: Arc<dyn Host>) -> (Engine, EventStream) {
        engine_with(dir, host, |_| {}).await
    }

    /// [`engine_on`], with its options changed as `change` says.
    async fn engine_with(
        dir: &std::path::Path,
        host: Arc<dyn Host>,
        change: impl FnOnce(&mut EngineOptions),
    ) -> (Engine, EventStream) {
        let paths = Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
        };
        let config = Config::default();
        let http = HttpClient::offline(&config.network).unwrap();
        let store = kaeri_store::Store::open_in_memory().unwrap();
        let theme = PlayerTheme {
            text: "#E4E0D6".into(),
            dim: "#A3A9C4".into(),
            obi: "#F2A64B".into(),
            state: "#7FC7D9".into(),
            alert: "#E06B54".into(),
            ground: "#161A2E".into(),
            font: "sans-serif".into(),
        };
        let mut options = EngineOptions::new(paths, config, http, store, theme);
        options.sources = Some(ProviderRegistry::new(Vec::new()));
        options.background = crate::options::Background::none();
        change(&mut options);
        Engine::start(options, host).await.unwrap()
    }

    /// The trackers' states launch says, by tracker.
    async fn launch_states(events: &mut EventStream) -> Vec<kaeri_model::sync::SyncState> {
        let launch = events.launch().await.into_iter();
        launch
            .filter_map(
                |event| if let Event::Sync(state) = event { Some(*state) } else { None },
            )
            .collect()
    }

    /// A host with a store of its own, as an app offers a phone's keychain.
    struct Keeping(Arc<Kept>);

    impl Host for Keeping {
        fn open_url(&self, _: &str) -> Result<(), String> {
            Err("a test opens nothing".into())
        }

        fn notify(&self, _: &str, _: &str) {}

        fn secrets(&self) -> Option<Arc<dyn SecretStore>> {
            Some(self.0.clone())
        }
    }

    /// Values kept in memory, said to be in a keychain.
    #[derive(Default)]
    struct Kept(std::sync::Mutex<std::collections::HashMap<String, String>>);

    impl SecretStore for Kept {
        fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
            Ok(self.0.lock().unwrap().get(id).cloned())
        }

        fn set(&self, id: &str, value: &str) -> Result<Storage, SecretError> {
            self.0.lock().unwrap().insert(id.to_owned(), value.to_owned());
            Ok(Storage::Keychain)
        }

        fn delete(&self, id: &str) -> Result<(), SecretError> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }

        fn storage_for(&self, _: &str) -> Storage {
            Storage::Keychain
        }
    }

    #[tokio::test]
    async fn an_apps_own_mal_client_signs_in_through_its_sheet_with_pkce_and_a_state() {
        use kaeri_model::{notice::NoticeKind, stores::Secret};
        let dir = tempfile::tempdir().unwrap();
        let kept = Arc::new(Kept::default());
        let clients = crate::options::AppClients {
            mal: Some("a-mal-client".into()),
            ..crate::options::AppClients::default()
        };
        let host = Arc::new(Keeping(kept.clone()));
        let (engine, mut events) =
            engine_with(dir.path(), host, |options| options.app_clients = clients).await;
        let launch = launch_states(&mut events).await;
        assert!(launch.iter().any(|state| state.tracker == "mal"), "turned on for the app");

        // Each a job: what it says comes before it is done.
        let said = |seen: &[Event]| {
            let toast = seen.iter().find_map(|e| match e {
                Event::Toast(notice) => Some(notice.clone()),
                _ => None,
            });
            toast.expect("a notice")
        };
        engine.send(Command::Connect { tracker: "mal".into() });
        let seen = until(&mut events, |e| matches!(e, Event::JobDone(_))).await;
        let Some(Event::Authorize { tracker, url, callback_scheme }) =
            seen.iter().find(|e| matches!(e, Event::Authorize { .. }))
        else {
            panic!("the sign-in sheet: {seen:?}")
        };
        assert_eq!((tracker.as_str(), callback_scheme.as_str()), ("mal", "kaeri"));
        assert!(url.starts_with("https://myanimelist.net/v1/oauth2/authorize?"), "{url}");
        assert!(url.contains("client_id=a-mal-client"), "{url}");
        assert!(url.contains("code_challenge_method=plain"), "{url}");
        assert!(url.contains("redirect_uri=kaeri%3A%2F%2Fauth%2Fmal"), "{url}");
        let state = url.split("&state=").nth(1).expect("a state");

        // A code with no state is not this sign-in's, as nothing without one is.
        let bare = Secret("kaeri://auth/mal?code=a-code".into());
        engine.send(Command::FinishSignIn { tracker: "mal".into(), callback: bare });
        let refused = said(&until(&mut events, |e| matches!(e, Event::JobDone(_))).await);
        assert_eq!(
            refused.msg,
            Some(Msg::NoticeSignInWrongRequest { tracker: "MyAnimeList".into() })
        );

        // The code comes back with the state; its exchange, which asks MyAnimeList itself,
        // cannot leave this test, and nothing is kept.
        let callback = Secret(format!("kaeri://auth/mal?code=a-code&state={state}"));
        engine.send(Command::FinishSignIn { tracker: "mal".into(), callback });
        let failed = said(&until(&mut events, |e| matches!(e, Event::JobDone(_))).await);
        assert_eq!(failed.kind, NoticeKind::Alert);
        assert!(
            matches!(&failed.msg, Some(Msg::NoticeTokenExchangeFailed { tracker, .. }) if tracker == "MyAnimeList"),
            "{:?}",
            failed.msg
        );
        assert_eq!(kept.get("mal").unwrap(), None);
    }

    #[tokio::test]
    async fn an_anilist_client_registered_on_an_app_link_asks_the_sheet_to_watch_for_https() {
        let dir = tempfile::tempdir().unwrap();
        let clients = crate::options::AppClients {
            anilist: Some("4242".into()),
            anilist_redirect: Some("https://kaeri.app/auth/anilist".into()),
            ..crate::options::AppClients::default()
        };
        let host = Arc::new(Keeping(Arc::new(Kept::default())));
        let (engine, mut events) =
            engine_with(dir.path(), host, |options| options.app_clients = clients).await;
        launch_states(&mut events).await;
        engine.send(Command::Connect { tracker: "anilist".into() });
        let seen = until(&mut events, |e| matches!(e, Event::JobDone(_))).await;
        let Some(Event::Authorize { tracker, url, callback_scheme }) =
            seen.iter().find(|e| matches!(e, Event::Authorize { .. }))
        else {
            panic!("the sign-in sheet: {seen:?}")
        };
        assert_eq!((tracker.as_str(), callback_scheme.as_str()), ("anilist", "https"));
        assert!(url.contains("client_id=4242&response_type=token"), "{url}");
    }

    #[tokio::test]
    async fn the_trackers_read_their_tokens_from_the_hosts_store() {
        let dir = tempfile::tempdir().unwrap();
        let kept = Arc::new(Kept::default());
        kept.set("kitsu", "a-kitsu-token").unwrap();
        let (_engine, mut events) = engine_on(dir.path(), Arc::new(Keeping(kept))).await;
        let [kitsu] = &launch_states(&mut events).await[..] else { panic!("kitsu alone") };
        assert!(kitsu.connected, "the host's token was not read");
        assert_eq!(kitsu.storage.as_deref(), Some("OS keychain"));
        assert!(!kitsu.storage_degraded);
        assert!(
            !dir.path().join("data").join("tokens").exists(),
            "a file was written beside it"
        );
    }

    #[tokio::test]
    async fn a_host_with_no_store_has_its_tokens_kept_in_files_under_the_data_directory() {
        // What a host that offers nothing gets, and says it gets.
        struct Bare;
        impl Host for Bare {
            fn open_url(&self, _: &str) -> Result<(), String> {
                Err("a test opens nothing".into())
            }
            fn notify(&self, _: &str, _: &str) {}
        }
        let dir = tempfile::tempdir().unwrap();
        let tokens = dir.path().join("data").join("tokens");
        std::fs::create_dir_all(&tokens).unwrap();
        std::fs::write(tokens.join("kitsu.token"), "a-kitsu-token").unwrap();

        let (_engine, mut events) = engine_on(dir.path(), Arc::new(Bare)).await;
        let [kitsu] = &launch_states(&mut events).await[..] else { panic!("kitsu alone") };
        assert!(kitsu.connected, "the file's token was not read");
        assert_eq!(kitsu.storage.as_deref(), Some("0600 file"));
        assert!(kitsu.storage_degraded, "a file is less than a keychain, and says so");
    }

    #[tokio::test]
    async fn launch_says_the_settings_autoplay_the_stores_the_chain_and_the_trackers_in_order()
    {
        let dir = tempfile::tempdir().unwrap();
        let (_engine, mut events) = quiet_engine(dir.path()).await;
        let launch: Vec<&str> = events
            .launch()
            .await
            .iter()
            .map(|event| match event {
                Event::Settings(_) => "settings",
                Event::Autoplay(view) if view.trailers => "autoplay",
                Event::Stores(_) => "stores",
                Event::Providers(_) => "providers",
                Event::Demo(_) => "demo",
                Event::Sync(_) => "sync",
                _ => "other",
            })
            .collect();
        // Kitsu is the one tracker the defaults build.
        // Trailers may start until the front end says otherwise: Always is the default.
        assert_eq!(launch, ["settings", "autoplay", "stores", "providers", "demo", "sync"]);
        assert!(events.launch().await.is_empty(), "taken once");
    }

    #[tokio::test]
    async fn a_job_is_numbered_at_once_and_announced_before_its_work_says_anything() {
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;

        let score = Command::SetScore { id: kaeri_core::ids::AnilistId::new(1), score: 7.0 };
        assert_eq!(engine.send(score), None, "nothing to wait on, so no job");
        let first = engine.send(Command::LoadDownloads);
        let second = engine.send(Command::CheckProviders).expect("a job");
        assert_eq!(first, None, "the downloads are read at once");
        assert_eq!(second, JobId::new(1), "numbered from one");

        let mut seen = Vec::new();
        while !matches!(seen.last(), Some(Event::JobDone(_))) {
            seen.extend(events.next_events().await);
        }
        let Some(Event::JobStarted { job, activity }) =
            seen.iter().find(|e| matches!(e, Event::JobStarted { .. }))
        else {
            panic!("announced")
        };
        assert_eq!((*job, activity.as_str()), (second, "checking providers"));
        let started = seen.iter().position(|e| matches!(e, Event::JobStarted { .. }));
        let answered = seen.iter().position(|e| matches!(e, Event::Providers(_)));
        assert!(started < answered, "announced before the answer");
        // The downloads, sent first, are answered first.
        assert!(matches!(seen[0], Event::Downloads(_)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_thread_that_is_not_async_can_send() {
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        let engine = Arc::new(engine);
        let sender = Arc::clone(&engine);
        let job =
            std::thread::spawn(move || sender.send(Command::CheckProviders)).join().unwrap();
        assert_eq!(job, Some(JobId::new(1)));
        let mut done = false;
        while !done {
            done = events.next_events().await.iter().any(|e| matches!(e, Event::JobDone(_)));
        }
    }

    /// Events until one `done` says is the last, or a second of nothing.
    async fn until(events: &mut EventStream, done: impl Fn(&Event) -> bool) -> Vec<Event> {
        let mut seen = Vec::new();
        while !seen.last().is_some_and(&done) {
            match tokio::time::timeout(Duration::from_secs(1), events.next_events()).await {
                Ok(batch) if !batch.is_empty() => seen.extend(batch),
                _ => panic!("nothing more, having seen {seen:?}"),
            }
        }
        seen
    }

    #[cfg(feature = "testing")]
    #[tokio::test]
    async fn a_step_that_panics_is_said_and_the_engine_carries_on() {
        use kaeri_model::notice::NoticeKind;
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        engine.panic_once();
        let seen = until(&mut events, |e| matches!(e, Event::Toast(_))).await;
        let [Event::Toast(fault)] = &seen[..] else { panic!("the alert alone: {seen:?}") };
        assert_eq!((fault.kind, fault.message()), (NoticeKind::Alert, Msg::NoticeEngineFault));
        // The next command is routed as ever.
        engine.send(Command::LoadDownloads);
        until(&mut events, |e| matches!(e, Event::Downloads(_))).await;
    }

    #[tokio::test]
    async fn a_step_asked_for_is_put_in_effect_written_and_said_whole() {
        use kaeri_model::settings::SettingKey;
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;

        engine.send(Command::StepSetting { key: SettingKey::AutoNext, delta: 1 });
        let seen = until(&mut events, |e| matches!(e, Event::Toast(_))).await;
        let [Event::Settings(view), Event::Toast(saved)] = &seen[..] else {
            panic!("the settings, then the notice: {seen:?}")
        };
        let row = view.rows.iter().find(|row| row.key == SettingKey::AutoNext).unwrap();
        assert_eq!(row.value, "off");
        assert_eq!(view.refused, None);
        assert_eq!(saved.msg, Some(Msg::NoticeSaved { label: "auto-play next".into() }));
        let file = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert_eq!(file, "[playback]\nauto_next = false\n");
    }

    #[tokio::test]
    async fn a_refused_write_is_said_with_its_setting_put_back() {
        use kaeri_model::{notice::NoticeKind, settings::SettingKey};
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;

        // Marked watched at 900%: refused by the config's own rules.
        let value = kaeri_core::settings::SettingValue::Float(9.0);
        engine.send(Command::SetSetting { key: SettingKey::CommitThreshold, value });
        let seen = until(&mut events, |e| matches!(e, Event::Toast(_))).await;
        let [Event::Settings(view), Event::Toast(refusal)] = &seen[..] else {
            panic!("the settings, then the notice: {seen:?}")
        };
        assert_eq!(view.refused, Some(SettingKey::CommitThreshold));
        let row = view.rows.iter().find(|row| row.key == SettingKey::CommitThreshold).unwrap();
        assert_ne!(row.value, "900%", "put back to what the file holds");
        assert_eq!(refusal.kind, NoticeKind::Alert);
        assert!(
            matches!(&refusal.msg, Some(Msg::NoticeNotSaved { reason }) if reason.contains("commit_threshold")),
            "{:?}",
            refusal.msg
        );
        assert!(!dir.path().join("config.toml").exists(), "nothing written");
    }

    #[tokio::test]
    async fn sub_or_dub_for_the_session_is_put_in_effect_and_not_written() {
        use kaeri_core::media::Translation;
        use kaeri_model::settings::SettingKey;
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;

        engine.send(Command::SetTranslation { translation: Translation::Dub });
        let seen = until(&mut events, |e| matches!(e, Event::Settings(_))).await;
        let [Event::Settings(view)] = &seen[..] else { panic!("the settings alone: {seen:?}") };
        assert_eq!(view.translation, Translation::Dub);
        let row = view.rows.iter().find(|row| row.key == SettingKey::Translation).unwrap();
        assert_eq!(row.value, "sub · dub this session");
        assert!(!dir.path().join("config.toml").exists(), "the file is the saved choice's");
    }

    #[tokio::test]
    async fn a_show_sets_a_setting_for_itself_and_the_settings_count_it() {
        use kaeri_core::settings::SettingValue;
        use kaeri_model::{
            settings::{SettingKey, SettingOverride},
            show::ShowSettingKey,
        };
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        let launch = events.launch().await;
        let Some(Event::Settings(view)) = launch.first() else { panic!("{launch:?}") };
        assert!(view.overridden.is_empty(), "no show sets anything yet");
        let id = kaeri_core::ids::AnilistId::new(900_001);

        let key = ShowSettingKey::SkipOpening;
        engine.send(Command::SetShowSetting {
            id,
            key,
            value: Some(SettingValue::Bool(false)),
        });
        let seen = until(&mut events, |e| matches!(e, Event::Settings(_))).await;
        let [Event::ShowSettings(sheet), Event::Settings(view)] = &seen[..] else {
            panic!("the sheet, then the settings: {seen:?}")
        };
        assert_eq!(sheet.id, id);
        let row = sheet.rows.iter().find(|row| row.key == key).unwrap();
        assert_eq!(row.own, Some(SettingValue::Bool(false)));
        assert_eq!((row.value.as_str(), row.default.as_str()), ("off", "on"));
        assert_eq!(
            view.overridden,
            [SettingOverride { key: SettingKey::SkipOpening, shows: 1 }]
        );
        assert!(!dir.path().join("config.toml").exists(), "Settings' own value is untouched");

        // Asked for again, as a sheet opening does.
        engine.send(Command::LoadShowSettings { id });
        let seen = until(&mut events, |e| matches!(e, Event::ShowSettings(_))).await;
        let [Event::ShowSettings(again)] = &seen[..] else {
            panic!("the sheet alone: {seen:?}")
        };
        assert_eq!(again, sheet);

        engine.send(Command::SetShowSetting { id, key, value: None });
        let seen = until(&mut events, |e| matches!(e, Event::Settings(_))).await;
        let [Event::ShowSettings(sheet), Event::Settings(view)] = &seen[..] else {
            panic!("the sheet, then the settings: {seen:?}")
        };
        assert!(sheet.rows.iter().all(|row| row.own.is_none()), "following Settings again");
        assert!(view.overridden.is_empty());
    }

    #[tokio::test]
    async fn a_value_a_show_setting_does_not_take_is_refused_in_words() {
        use kaeri_core::settings::SettingValue;
        use kaeri_model::{notice::NoticeKind, show::ShowSettingKey};
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        let id = kaeri_core::ids::AnilistId::new(900_001);

        let value = Some(SettingValue::Str("hd".into()));
        engine.send(Command::SetShowSetting { id, key: ShowSettingKey::Quality, value });
        let seen = until(&mut events, |e| matches!(e, Event::Toast(_))).await;
        let [Event::Toast(refusal)] = &seen[..] else { panic!("the refusal alone: {seen:?}") };
        assert_eq!(refusal.kind, NoticeKind::Alert);
        assert_eq!(
            refusal.msg,
            Some(Msg::NoticeShowSettingRefused {
                value: "\u{201c}hd\u{201d}".into(),
                setting: "quality".into(),
            })
        );
    }

    #[tokio::test]
    async fn a_reset_puts_the_show_back_on_settings_with_the_source_it_kept() {
        use kaeri_core::settings::SettingValue;
        use kaeri_model::{settings::SettingKey, show::ShowSettingKey};
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        let id = kaeri_core::ids::AnilistId::new(900_001);

        let dub = Some(SettingValue::Str("dub".into()));
        engine.send(Command::SetShowSetting {
            id,
            key: ShowSettingKey::Translation,
            value: dub,
        });
        until(&mut events, |e| matches!(e, Event::Settings(_))).await;
        engine.send(Command::SetProviderPreference { id, provider: Some("yamabiko".into()) });
        let seen = until(&mut events, |e| matches!(e, Event::ShowSettings(_))).await;
        let [Event::Settings(view), Event::Toast(_), Event::ShowSettings(pinned)] = &seen[..]
        else {
            panic!("the settings, counted again, the notice, then the show's sheet: {seen:?}")
        };
        assert_eq!(
            pinned.source.as_deref(),
            Some("yamabiko"),
            "the sheet says the pin at once"
        );
        let counted: Vec<_> = view.overridden.iter().map(|o| (o.key, o.shows)).collect();
        assert_eq!(counted, [(SettingKey::SourceOrder, 1), (SettingKey::Translation, 1)]);
        engine.send(Command::LoadShowSettings { id });
        let seen = until(&mut events, |e| matches!(e, Event::ShowSettings(_))).await;
        let [Event::ShowSettings(sheet)] = &seen[..] else { panic!("{seen:?}") };
        assert_eq!(sheet.source.as_deref(), Some("yamabiko"));

        engine.send(Command::ResetShowSettings { id });
        let seen = until(&mut events, |e| matches!(e, Event::Settings(_))).await;
        let [Event::ShowSettings(sheet), Event::Settings(view)] = &seen[..] else {
            panic!("the sheet, then the settings: {seen:?}")
        };
        assert!(sheet.rows.iter().all(|row| row.own.is_none()));
        assert_eq!(sheet.source, None, "back to trying the sources in order");
        assert!(sheet.remembered.is_empty());
        assert!(view.overridden.is_empty());
    }

    #[tokio::test]
    async fn a_proxy_is_written_as_one_change_and_its_password_kept_apart() {
        use kaeri_model::{
            proxy::{PROXY_SECRET, ProxyView, TorrentProxy},
            stores::Secret,
        };
        let dir = tempfile::tempdir().unwrap();
        let kept = Arc::new(Kept::default());
        let (engine, mut events) = engine_on(dir.path(), Arc::new(Keeping(kept.clone()))).await;
        events.launch().await;

        let proxy = TorrentProxy {
            host: "192.0.2.10".into(),
            port: 1080,
            user: Some("mika".into()),
            password: Some(Secret("lantern".into())),
        };
        engine.send(Command::SetTorrentProxy { proxy: Some(proxy) });
        let seen = until(&mut events, |e| matches!(e, Event::Toast(_))).await;
        let [Event::Settings(view), Event::Toast(saved)] = &seen[..] else {
            panic!("the settings, then the notice: {seen:?}")
        };
        assert_eq!(view.refused, None);
        assert_eq!(saved.msg, Some(Msg::NoticeProxySaved));
        let user = Some("mika".to_owned());
        let shown = ProxyView { host: "192.0.2.10".into(), port: 1080, user };
        assert_eq!(view.torrent.proxy, Some(shown));
        let file = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(file.contains("proxy = \"socks5://mika@192.0.2.10:1080\""), "{file}");
        assert!(!file.contains("lantern"), "the password stays out of the file:\n{file}");
        assert_eq!(kept.get(PROXY_SECRET).unwrap().as_deref(), Some("lantern"));

        // No proxy: torrents connect directly, and the password is not kept for nothing.
        engine.send(Command::SetTorrentProxy { proxy: None });
        let seen = until(&mut events, |e| matches!(e, Event::Toast(_))).await;
        let [Event::Settings(view), Event::Toast(removed)] = &seen[..] else {
            panic!("the settings, then the notice: {seen:?}")
        };
        assert_eq!(removed.msg, Some(Msg::NoticeProxyRemoved));
        assert_eq!(view.torrent.proxy, None);
        let file = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(!file.contains("proxy"), "{file}");
        assert_eq!(kept.get(PROXY_SECRET).unwrap(), None);
    }

    #[tokio::test]
    async fn a_proxy_with_no_address_is_refused_and_nothing_written() {
        use kaeri_model::{notice::NoticeKind, proxy::TorrentProxy, settings::SettingKey};
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;

        let nowhere = TorrentProxy { host: " ".into(), port: 1080, user: None, password: None };
        engine.send(Command::SetTorrentProxy { proxy: Some(nowhere) });
        let seen = until(&mut events, |e| matches!(e, Event::Toast(_))).await;
        let [Event::Settings(view), Event::Toast(refusal)] = &seen[..] else {
            panic!("the settings, then the notice: {seen:?}")
        };
        assert_eq!(view.refused, Some(SettingKey::TorrentProxy));
        assert_eq!(view.torrent.proxy, None, "as the file has it");
        assert_eq!(refusal.kind, NoticeKind::Alert);
        assert_eq!(
            refusal.msg,
            Some(Msg::NoticeNotSavedBecause { cause: Cause::ProxyNeedsAddress })
        );
        assert!(!dir.path().join("config.toml").exists(), "nothing written");
    }

    #[cfg(not(feature = "desktop"))]
    #[tokio::test]
    async fn without_a_player_a_play_is_refused_in_words_and_nothing_is_staged() {
        use kaeri_model::{command::PlayerCommand, notice::NoticeKind};
        let dir = tempfile::tempdir().unwrap();
        // A front end that hosts no player, with no mpv to fall back on.
        let host = Arc::new(QuietHost::in_dir(&dir.path().join("tokens")));
        let (engine, mut events) =
            engine_with(dir.path(), host, |options| options.hosted_player = false).await;
        events.launch().await;
        let id = kaeri_core::ids::AnilistId::new(1);

        let job = engine.send(Command::play(id, "1")).expect("a job");
        let seen = until(&mut events, |e| matches!(e, Event::JobDone(_))).await;
        let [
            Event::JobStarted { job: started, .. },
            Event::Toast(refusal),
            Event::JobDone(done),
        ] = &seen[..]
        else {
            panic!("the job, the refusal and the job's end: {seen:?}")
        };
        assert_eq!((*started, *done), (job, job));
        assert_eq!(refusal.kind, NoticeKind::Alert);
        assert_eq!(refusal.text, Msg::NoticeNoPlayer.english());

        // Steering nothing says nothing; a download played from disk is refused alike.
        engine.send(Command::Player(PlayerCommand::Stop));
        let path = dir.path().join("Hoshikuzu no Tabibito 01.mkv");
        let title = "Hoshikuzu no Tabibito".into();
        let path = path.display().to_string();
        engine.send(Command::PlayLocal { id, episode: "1".into(), title, path });
        let seen = until(&mut events, |e| matches!(e, Event::Toast(_))).await;
        assert!(
            matches!(&seen[..], [Event::Toast(n)] if n.text == Msg::NoticeNoPlayer.english()),
            "{seen:?}"
        );

        // A release picked by hand the same, and a watch party where this build has none:
        // refused before anything is staged.
        let episode = || "1".to_owned();
        let mut plays = vec![(
            Command::PlaySource {
                id,
                episode: episode(),
                provider_id: "yamabiko".into(),
                source_id: "a".into(),
            },
            Msg::NoticeNoPlayer.english(),
        )];
        if cfg!(not(feature = "party")) {
            plays.push((
                Command::play_party(id, episode()),
                Msg::Store(StoreMsg::NoticePartyNotBuilt).english(),
            ));
        }
        for (play, refusal) in plays {
            engine.send(play);
            let seen = until(&mut events, |e| matches!(e, Event::JobDone(_))).await;
            assert!(
                seen.iter().any(|e| matches!(e, Event::Toast(n) if n.text == refusal)),
                "{seen:?}"
            );
            assert!(
                !seen.iter().any(|e| matches!(e, Event::PlaybackStaged { .. })),
                "{seen:?}"
            );
        }
    }

    /// [`until`], for an event anywhere in what comes, not only last in its batch: what a
    /// party says comes in bursts. The wait is long because Windows takes about two seconds to
    /// refuse a connection to a closed local port.
    #[cfg(any(feature = "party", feature = "cast"))]
    async fn seen_until(events: &mut EventStream, done: impl Fn(&Event) -> bool) -> Vec<Event> {
        let mut seen = Vec::new();
        while !seen.iter().any(&done) {
            match tokio::time::timeout(Duration::from_secs(10), events.next_events()).await {
                Ok(batch) if !batch.is_empty() => seen.extend(batch),
                _ => panic!("nothing more, having seen {seen:?}"),
            }
        }
        seen
    }

    /// A party joined, steered and left through the engine, on a server of the test's own.
    #[cfg(feature = "party")]
    #[tokio::test]
    async fn a_watch_party_is_joined_steered_and_left() {
        use kaeri_model::party::{PartyMessageKind, SUGGESTED_SERVER};
        use kaeri_party::fake::{FakeOptions, FakeServer};

        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        let server = FakeServer::start(FakeOptions::default()).await.unwrap();

        // The form: the public server suggested, nothing used before, and a room to meet in.
        engine.send(Command::LoadPartyForm);
        let seen = seen_until(&mut events, |e| matches!(e, Event::PartyForm(_))).await;
        let Some(Event::PartyForm(form)) =
            seen.iter().find(|e| matches!(e, Event::PartyForm(_)))
        else {
            unreachable!()
        };
        assert_eq!(form.suggested_server, SUGGESTED_SERVER);
        assert!(form.recent_servers.is_empty());
        assert_eq!(
            kaeri_model::party::clean_party_code(&form.room).as_deref(),
            Some(form.room.as_str()),
            "a code: {}",
            form.room
        );
        // Each form a room of its own, past guessing.
        let first = form.room.clone();
        engine.send(Command::LoadPartyForm);
        let seen = seen_until(&mut events, |e| matches!(e, Event::PartyForm(_))).await;
        let again = seen.iter().find_map(|e| match e {
            Event::PartyForm(form) => Some(form.room.clone()),
            _ => None,
        });
        assert_ne!(again, Some(first));

        // Joined: the room said, and its server kept first among the ones used.
        let room = "amber-anchor";
        let join = || Command::PartyJoin {
            server: server.address(),
            room: room.into(),
            name: Some("Ada".into()),
        };
        engine.send(join());
        let seen = seen_until(&mut events, |e| matches!(e, Event::PartyJoined(_))).await;
        let Some(Event::PartyJoined(view)) =
            seen.iter().find(|e| matches!(e, Event::PartyJoined(_)))
        else {
            unreachable!()
        };
        assert_eq!((view.room.as_str(), view.name.as_str()), (room, "Ada"));
        assert_eq!(view.server, server.address());
        // The room's members listed, before anyone else comes in, and the server saved.
        let mut seen = seen;
        while !(seen.iter().any(|e| matches!(e, Event::Settings(_)))
            && seen.iter().any(|e| matches!(e, Event::PartyMembers(_))))
        {
            seen.extend(seen_until(&mut events, |_| true).await);
        }
        let saved = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(saved.contains(&server.address()), "{saved}");
        // The same room again is the party already going.
        engine.send(join());

        // Someone else comes in, and the members are said with them.
        let mika = server.member(room, "Mika");
        let seen = seen_until(&mut events, |e| {
            matches!(e, Event::PartyMessage(m) if matches!(&m.kind, PartyMessageKind::Joined { name } if name == "Mika"))
        })
        .await;
        assert!(!seen.iter().any(|e| matches!(e, Event::PartyJoined(_))), "{seen:?}");
        let members = seen_until(
            &mut events,
            |e| matches!(e, Event::PartyMembers(members) if members.len() == 2),
        )
        .await;
        let Some(Event::PartyMembers(members)) =
            members.iter().rev().find(|e| matches!(e, Event::PartyMembers(m) if m.len() == 2))
        else {
            unreachable!()
        };
        assert!(members.iter().any(|m| m.name == "Ada" && m.you));

        // Ready, as the room sees it.
        engine.send(Command::PartyReady { ready: true });
        let seen =
            seen_until(&mut events, |e| matches!(e, Event::PartyState(view) if view.ready))
                .await;
        assert!(!seen.is_empty());
        let ready =
            || server.room(room).and_then(|r| r.members.into_iter().find(|m| m.name == "Ada"));
        for _ in 0..50 {
            if ready().is_some_and(|ada| ada.ready == Some(true)) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(ready().and_then(|ada| ada.ready), Some(true));

        // Another room: the first is left without a word, and the new one says itself.
        engine.send(Command::PartyJoin {
            server: server.address(),
            room: "birch-bell".into(),
            name: Some("Ada".into()),
        });
        let seen = seen_until(
            &mut events,
            |e| matches!(e, Event::PartyJoined(view) if view.room == "birch-bell"),
        )
        .await;
        assert!(!seen.iter().any(|e| matches!(e, Event::PartyLeft { .. })), "{seen:?}");
        for _ in 0..50 {
            if ready().is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(ready(), None, "out of the first room");

        // Left: said once, and a party command after it is told there is none.
        drop(mika);
        engine.send(Command::PartyLeave);
        let seen = seen_until(&mut events, |e| matches!(e, Event::PartyLeft { .. })).await;
        assert!(
            seen.iter().any(|e| matches!(e, Event::PartyLeft { reason: None })),
            "{seen:?}"
        );
        engine.send(Command::PartyReady { ready: false });
        seen_until(&mut events, |e| matches!(e, Event::PartyLeft { reason: None })).await;

        // A server written wrong is refused with how to write it.
        engine.send(Command::PartyJoin {
            server: "syncplay.example:port".into(),
            room: room.into(),
            name: None,
        });
        let seen = seen_until(&mut events, |e| matches!(e, Event::PartyLeft { .. })).await;
        let Some(reason) = seen.iter().find_map(|e| match e {
            Event::PartyLeft { reason } => reason.clone(),
            _ => None,
        }) else {
            panic!("{seen:?}")
        };
        assert!(reason.english().contains("host and a port"), "{reason}");
    }

    /// A lobby joined by link or code reads what the room watches from its playlist, counts
    /// down when everyone is ready, and starts the room.
    #[cfg(feature = "party")]
    #[tokio::test]
    async fn a_lobby_follows_the_rooms_playlist_and_starts_when_all_are_ready() {
        use kaeri_model::party::{PartyCommand, PartyFound, PartyStartKind};
        use kaeri_party::fake::{FakeOptions, FakeServer};

        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        let server = FakeServer::start(FakeOptions::default()).await.unwrap();
        let room = "kite-7qm4-xr9d";
        let mika = server.member(room, "Mika");
        mika.set_playlist(&[
            "Natsukaze Relay, episode 9 [anilist:104321:9]",
            "Natsukaze Relay, episode 10 [anilist:104321:10]",
        ]);
        mika.set_index(0);

        engine.send(Command::Party(PartyCommand::Lobby {
            server: server.address(),
            room: room.into(),
            name: Some("Ada".into()),
            pick: None,
        }));
        let seen = seen_until(&mut events, |e| {
            matches!(e, Event::PartyState(view) if view.room_episode.is_some() && view.next_episode.is_some())
        })
        .await;
        let Some(Event::PartyState(view)) = seen
            .iter()
            .rev()
            .find(|e| matches!(e, Event::PartyState(view) if view.room_episode.is_some()))
        else {
            panic!("{seen:?}")
        };
        let found = view.room_episode.as_ref().unwrap();
        assert_eq!(found.id.map(kaeri_core::ids::AnilistId::get), Some(104_321));
        assert_eq!((found.episode.as_str(), found.found), ("9", PartyFound::Tag));
        assert_eq!(view.next_episode.as_ref().map(|n| n.episode.as_str()), Some("10"));
        assert_eq!(view.starter.as_deref(), Some("Mika"));
        assert!(!view.can_start_anyway, "Mika set the episode, not this device");
        assert_eq!(view.code.as_deref(), None, "a code is on the suggested server only");
        assert!(
            view.link.as_deref().is_some_and(|l| l.starts_with("https://kaeri.app/party#"))
        );

        // Both ready: the count runs, then the room starts.
        mika.ready(true);
        engine.send(Command::PartyReady { ready: true });
        let seen = seen_until(&mut events, |e| matches!(e, Event::PartyStarted { .. })).await;
        assert!(
            seen.iter().any(|e| matches!(e, Event::PartyState(v) if v.starting_in == Some(3))),
            "the count ran: {seen:?}"
        );
        assert!(seen.iter().any(|e| matches!(
            e,
            Event::PartyStarted { id, episode, kind: PartyStartKind::Start }
                if id.get() == 104_321 && episode == "9"
        )));
    }

    /// A room someone else picked and started, joined from its lobby without saying ready:
    /// this device opens the room's episode and joins in.
    #[cfg(feature = "party")]
    #[tokio::test]
    async fn a_room_someone_else_plays_opens_the_player_without_waiting_for_ready() {
        use kaeri_model::party::{PartyCommand, PartyStartKind};
        use kaeri_party::fake::{FakeOptions, FakeServer};

        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        let server = FakeServer::start(FakeOptions::default()).await.unwrap();
        let room = "lark-3vd8-pq2m";
        let mika = server.member(room, "Mika");
        mika.set_playlist(&["Natsukaze Relay, episode 9 [anilist:104321:9]"]);
        mika.set_index(0);
        mika.open("Natsukaze Relay, episode 9 [anilist:104321:9]", 1440.0);

        engine.send(Command::Party(PartyCommand::Lobby {
            server: server.address(),
            room: room.into(),
            name: Some("Ada".into()),
            pick: None,
        }));
        seen_until(&mut events, |e| {
            matches!(e, Event::PartyState(view) if view.room_episode.as_ref().is_some_and(|r| r.id.is_some()))
        })
        .await;
        // Paused, the lobby waits.
        let seen =
            seen_until(&mut events, |e| matches!(e, Event::PartyMembers(m) if m.len() == 2))
                .await;
        assert!(!seen.iter().any(|e| matches!(e, Event::PartyStarted { .. })), "{seen:?}");

        mika.play();
        let seen = seen_until(&mut events, |e| matches!(e, Event::PartyStarted { .. })).await;
        assert!(
            seen.iter().any(|e| matches!(
                e,
                Event::PartyStarted { id, episode, kind: PartyStartKind::Rejoin }
                    if id.get() == 104_321 && episode == "9"
            )),
            "{seen:?}"
        );
    }

    /// A play anywhere in the app while in a party picks that episode for the room, rather
    /// than playing it alone; the room's own episode is not picked again.
    #[cfg(feature = "party")]
    #[tokio::test]
    async fn a_play_while_in_a_party_picks_the_episode_for_the_room() {
        use kaeri_core::ids::AnilistId;
        use kaeri_party::fake::{FakeOptions, FakeServer};

        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        let server = FakeServer::start(FakeOptions::default()).await.unwrap();
        let room = "wren-5kt2-hb7c";
        let _mika = server.member(room, "Mika");
        engine.send(Command::PartyJoin {
            server: server.address(),
            room: room.into(),
            name: Some("Ada".into()),
        });
        seen_until(&mut events, |e| matches!(e, Event::PartyJoined(_))).await;

        engine.send(Command::play(AnilistId::new(104_321), "9"));
        let picked = |e: &Event| {
            matches!(e, Event::PartyState(view) if view.room_episode.as_ref().is_some_and(|r| {
                r.id.map(AnilistId::get) == Some(104_321) && r.episode == "9"
            }))
        };
        let seen = seen_until(&mut events, picked).await;
        assert!(!seen.iter().any(|e| matches!(e, Event::PlaybackStaged { .. })), "{seen:?}");
        for _ in 0..50 {
            if server.room(room).is_some_and(|r| !r.playlist.is_empty()) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let kept = server.room(room).expect("the room exists");
        assert!(kept.playlist[0].ends_with("[anilist:104321:9]"), "{:?}", kept.playlist);

        // The same episode again changes nothing in the room, and still plays nothing alone.
        engine.send(Command::play(AnilistId::new(104_321), "9"));
        let seen = seen_until(&mut events, |e| matches!(e, Event::JobDone(_))).await;
        assert!(!seen.iter().any(|e| matches!(e, Event::PlaybackStaged { .. })), "{seen:?}");
        assert_eq!(server.room(room).unwrap().playlist, kept.playlist);

        // Out of the party, a play is this device's own again.
        engine.send(Command::PartyLeave);
        seen_until(&mut events, |e| matches!(e, Event::PartyLeft { .. })).await;
        engine.send(Command::play(AnilistId::new(104_321), "9"));
        seen_until(&mut events, |e| matches!(e, Event::PlaybackStaged { .. })).await;
    }

    /// A device that starts a party writes the playlist the others read: this episode and the
    /// next, in the name every Kaeri player reports.
    #[cfg(feature = "party")]
    #[tokio::test]
    async fn starting_a_party_writes_the_episode_and_the_next_to_the_room() {
        use kaeri_core::ids::AnilistId;
        use kaeri_model::party::{PartyCommand, PartyEpisode};
        use kaeri_party::fake::{FakeOptions, FakeServer};

        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        let server = FakeServer::start(FakeOptions::default()).await.unwrap();
        let room = "kite-7qm4-xr9d";

        engine.send(Command::Party(PartyCommand::Lobby {
            server: server.address(),
            room: room.into(),
            name: Some("Ada".into()),
            pick: Some(PartyEpisode { id: AnilistId::new(104_321), episode: "9".into() }),
        }));
        let seen = seen_until(
            &mut events,
            |e| matches!(e, Event::PartyState(view) if view.can_start_anyway),
        )
        .await;
        let Some(Event::PartyState(view)) =
            seen.iter().rev().find(|e| matches!(e, Event::PartyState(v) if v.can_start_anyway))
        else {
            panic!("{seen:?}")
        };
        assert_eq!(view.starter.as_deref(), Some("Ada"));
        for _ in 0..50 {
            if server.room(room).is_some_and(|r| !r.playlist.is_empty()) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let kept = server.room(room).expect("the room exists");
        assert_eq!(kept.index, Some(0));
        assert_eq!(kept.playlist.len(), 2, "{:?}", kept.playlist);
        assert!(kept.playlist[0].ends_with("[anilist:104321:9]"), "{:?}", kept.playlist);
        assert!(kept.playlist[1].ends_with("[anilist:104321:10]"), "{:?}", kept.playlist);
    }

    /// Looking in joins for a moment, says who is there and what the room watches, and leaves.
    #[cfg(feature = "party")]
    #[tokio::test]
    async fn looking_in_reports_the_room_and_leaves_it() {
        use kaeri_party::fake::{FakeOptions, FakeServer};

        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        let server = FakeServer::start(FakeOptions::default()).await.unwrap();
        let room = "kite-7qm4-xr9d";
        let mika = server.member(room, "Mika");
        mika.set_playlist(&["Natsukaze Relay, episode 9 [anilist:104321:9]"]);
        mika.set_index(0);

        engine.send(Command::Party(PartyCommand::LookIn {
            server: server.address(),
            room: room.into(),
            name: Some("Ada".into()),
        }));
        let seen = seen_until(&mut events, |e| {
            matches!(e, Event::PartyGlimpse(_) | Event::PartyLookFailed { .. })
        })
        .await;
        let Some(Event::PartyGlimpse(glimpse)) =
            seen.iter().find(|e| matches!(e, Event::PartyGlimpse(_)))
        else {
            panic!("{seen:?}")
        };
        assert_eq!(
            glimpse.members.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            ["Mika"]
        );
        assert_eq!(glimpse.room_episode.as_ref().map(|r| r.episode.as_str()), Some("9"));
        for _ in 0..50 {
            if server.room(room).is_some_and(|r| r.members.len() == 1) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(server.room(room).map(|r| r.members.len()), Some(1), "only Mika is left");

        // A server that is not there is a failed look, said in words.
        engine.send(Command::Party(PartyCommand::LookIn {
            server: "127.0.0.1:9".into(),
            room: room.into(),
            name: None,
        }));
        let seen =
            seen_until(&mut events, |e| matches!(e, Event::PartyLookFailed { .. })).await;
        assert!(seen.iter().any(|e| matches!(e, Event::PartyLookFailed { .. })), "{seen:?}");
    }

    /// A party asked of a play with no server chosen: refused, naming the public one and who
    /// runs it, with nothing staged.
    #[cfg(feature = "party")]
    #[tokio::test]
    async fn a_party_play_with_no_server_names_the_suggestion() {
        use kaeri_model::party::{SUGGESTED_SERVER, SUGGESTED_SERVER_RUN_BY};

        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        engine.send(Command::play_party(kaeri_core::ids::AnilistId::new(7), "1"));
        let seen = seen_until(&mut events, |e| matches!(e, Event::JobDone(_))).await;
        let reason = seen.iter().find_map(|e| match e {
            Event::PartyLeft { reason } => reason.clone(),
            _ => None,
        });
        let reason = reason.unwrap_or_else(|| panic!("{seen:?}"));
        assert!(
            reason.english().contains(SUGGESTED_SERVER)
                && reason.english().contains(SUGGESTED_SERVER_RUN_BY)
        );
        assert!(!seen.iter().any(|e| matches!(e, Event::PlaybackStaged { .. })), "{seen:?}");
    }

    /// `syncplay.enabled` sends every play to the party on the terminal, which asks for that,
    /// and does nothing in an app, which starts parties when asked.
    #[cfg(feature = "party")]
    #[tokio::test]
    async fn every_play_joins_the_party_on_the_terminal_alone() {
        use kaeri_model::party::SUGGESTED_SERVER;

        for terminal in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let host = Arc::new(QuietHost::in_dir(&dir.path().join("tokens")));
            let (engine, mut events) = engine_with(dir.path(), host, |options| {
                options.config.syncplay.enabled = true;
                options.every_play_a_party = terminal;
            })
            .await;
            events.launch().await;
            engine.send(Command::play(kaeri_core::ids::AnilistId::new(7), "1"));
            let seen = seen_until(&mut events, |e| matches!(e, Event::JobDone(_))).await;
            // No server is set, so the terminal's play is told to choose one; the app's plays
            // with no word of a party.
            let asked = seen.iter().any(|e| {
                matches!(e, Event::PartyLeft { reason: Some(reason) } if reason.english().contains(SUGGESTED_SERVER))
            });
            assert_eq!(asked, terminal, "{seen:?}");
            engine.shutdown().await;
        }
    }

    /// A play while casting goes to the TV, the next episode's included: the cast's card is
    /// told of it, and the player here is told nothing that would bring it up.
    #[cfg(feature = "cast")]
    #[tokio::test]
    async fn a_play_while_casting_is_staged_for_the_tv_and_never_for_the_player_here() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(QuietHost::in_dir(&dir.path().join("tokens")));
        let (engine, mut events) =
            engine_with(dir.path(), host, |options| options.hosted_player = true).await;
        events.launch().await;
        let hub = engine.casts();
        hub.found(kaeri_cast::CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: "127.0.0.2:8009".parse().unwrap(),
            video: true,
            group: false,
        });
        let id = kaeri_core::ids::AnilistId::new(1);
        let here =
            |seen: &Vec<Event>| seen.iter().any(|e| matches!(e, Event::PlaybackStaged { .. }));
        let cast = |seen: &Vec<Event>, episode: &str| {
            seen.iter()
                .any(|e| matches!(e, Event::CastStaged { episode: on, .. } if on == episode))
        };
        let ended = |e: &Event| matches!(e, Event::PlaybackEnded { .. });

        engine.send(Command::CastTo {
            device: "tv-1".into(),
            id,
            episode: "1".into(),
            position: None,
        });
        let seen = seen_until(&mut events, ended).await;
        assert!(!here(&seen) && cast(&seen, "1"), "{seen:?}");

        // The next episode, or one picked, while the cast goes on: on the TV too.
        assert!(hub.casting());
        engine.send(Command::play(id, "2"));
        let seen = seen_until(&mut events, ended).await;
        assert!(!here(&seen) && cast(&seen, "2"), "{seen:?}");

        // Casting stopped, a play opens here again.
        engine.send(Command::StopCasting);
        engine.send(Command::play(id, "3"));
        let seen = seen_until(&mut events, |e| matches!(e, Event::PlaybackStaged { .. })).await;
        assert!(!seen.iter().any(|e| matches!(e, Event::CastStaged { .. })), "{seen:?}");
        engine.shutdown().await;
    }

    #[tokio::test]
    async fn quitting_with_nothing_playing_does_not_wait() {
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        // No playback was ever started: there is nothing to stop.
        tokio::time::timeout(Duration::from_millis(500), engine.shutdown())
            .await
            .expect("no wait for a player that is not there");
        assert!(events.recv().await.is_none(), "the engine has stopped");
    }

    /// The cloud drives' sign-in, on a fake service on loopback.
    #[cfg(feature = "testing")]
    mod cloud {
        use std::sync::Mutex;

        use kaeri_model::{
            group::{
                CloudUnavailable, SyncCloudWay, SyncCommand, SyncFolderKind, SyncFolderSpec,
                SyncProblem, SyncSignInStep, SyncStatus,
            },
            stores::Secret,
        };
        use kaeri_sync::google::{DeviceStep as Step, FakeGoogle};

        use super::*;
        use crate::{cloud::CloudTest, options::CloudClient};

        /// A host that says what it was asked to open and, when it `browses`, goes there like a
        /// person who says yes at once.
        struct Browsing {
            kept: Arc<Kept>,
            opened: Arc<Mutex<Vec<String>>>,
            browses: bool,
        }

        impl Host for Browsing {
            fn open_url(&self, url: &str) -> Result<(), String> {
                self.opened.lock().unwrap().push(url.to_owned());
                if self.browses {
                    let url = url.to_owned();
                    tokio::spawn(async move {
                        let _ = reqwest::Client::new().get(url).send().await;
                    });
                }
                Ok(())
            }

            fn notify(&self, _: &str, _: &str) {}

            fn secrets(&self) -> Option<Arc<dyn SecretStore>> {
                Some(self.kept.clone())
            }
        }

        struct Rig {
            engine: Engine,
            watch: Watch,
            fake: FakeGoogle,
            kept: Arc<Kept>,
            opened: Arc<Mutex<Vec<String>>>,
            dir: tempfile::TempDir,
        }

        async fn rig(kind: CloudClient, browses: bool, ids: bool) -> Rig {
            let dir = tempfile::tempdir().unwrap();
            let fake = FakeGoogle::start().await;
            let kept = Arc::new(Kept::default());
            let opened = Arc::new(Mutex::new(Vec::new()));
            let host =
                Arc::new(Browsing { kept: kept.clone(), opened: opened.clone(), browses });
            let ep = fake.endpoints();
            let test = CloudTest {
                google: ep.clone(),
                dropbox: ep.clone(),
                dropbox_content: ep.api_base.clone(),
                onedrive: ep,
                poll_every: Some(Duration::from_millis(20)),
            };
            let clients = if ids {
                crate::options::AppClients {
                    google: Some("123.apps.example".into()),
                    google_secret: Some("shh-secret".into()),
                    dropbox: Some("a-dropbox-key".into()),
                    onedrive: Some("an-onedrive-id".into()),
                    cloud_kind: Some(kind),
                    ..Default::default()
                }
            } else {
                crate::options::AppClients {
                    cloud_kind: Some(kind),
                    no_built_in_clients: true,
                    ..Default::default()
                }
            };
            let (engine, mut events) = engine_with(dir.path(), host, |options| {
                options.app_clients = clients;
                options.cloud_test = Some(test);
            })
            .await;
            events.launch().await;
            Rig { engine, watch: Watch { events, held: Vec::new() }, fake, kept, opened, dir }
        }

        /// The engine's events, held so that what one wait read past is there for the next.
        struct Watch {
            events: EventStream,
            held: Vec<Event>,
        }

        impl Watch {
            /// Everything up to the first event `done` accepts, which is the last of them.
            async fn until(&mut self, done: impl Fn(&Event) -> bool) -> Vec<Event> {
                loop {
                    if let Some(at) = self.held.iter().position(&done) {
                        return self.held.drain(..=at).collect();
                    }
                    match tokio::time::timeout(
                        Duration::from_secs(20),
                        self.events.next_events(),
                    )
                    .await
                    {
                        Ok(batch) if !batch.is_empty() => self.held.extend(batch),
                        _ => panic!("nothing more, having seen {:?}", self.held),
                    }
                }
            }
        }

        fn google() -> SyncFolderKind {
            SyncFolderKind::GoogleDrive
        }

        fn folder(kind: SyncFolderKind) -> SyncFolderSpec {
            SyncFolderSpec { kind, address: None, user: None, password: None }
        }

        fn ended(kind: SyncFolderKind) -> impl Fn(&Event) -> bool {
            move |e| matches!(e, Event::SyncSignIn { service, step: None } if *service == kind)
        }

        fn status_of(seen: &[Event]) -> SyncStatus {
            seen.iter()
                .rev()
                .find_map(|e| if let Event::SyncStatus(s) = e { Some(s.clone()) } else { None })
                .expect("a status")
        }

        fn problems(seen: &[Event]) -> Vec<SyncProblem> {
            seen.iter()
                .filter_map(
                    |e| if let Event::SyncProblem(p) = e { Some(p.clone()) } else { None },
                )
                .collect()
        }

        fn has_token(rig: &Rig, id: &str) -> bool {
            rig.kept.get(id).unwrap().is_some_and(|t| !t.is_empty())
        }

        #[tokio::test]
        async fn a_desktop_signs_in_to_google_through_the_browser_and_sets_the_folder_up() {
            let mut rig = rig(CloudClient::Desktop, true, true).await;
            rig.engine.send(Command::StartSync {
                device_name: "Desk".into(),
                folder: folder(google()),
            });
            let seen = rig.watch.until(ended(google())).await;
            let steps: Vec<_> = seen
                .iter()
                .filter_map(|e| match e {
                    Event::SyncSignIn { step: Some(step), .. } => Some(step.clone()),
                    _ => None,
                })
                .collect();
            // The browser step carries the page, for "Open the page again".
            let url = rig.opened.lock().unwrap()[0].clone();
            // Connecting once the browser comes back, before the code is exchanged.
            assert_eq!(
                steps,
                [
                    SyncSignInStep::Browser { url: url.clone() },
                    SyncSignInStep::Connecting,
                    SyncSignInStep::Finishing
                ]
            );
            assert!(seen.iter().any(|e| matches!(e, Event::SyncRecoveryKey(_))), "{seen:?}");
            assert!(!seen.iter().any(|e| matches!(e, Event::SyncProblem(_))), "{seen:?}");
            // The browser was sent to Google's page for the app's client, with PKCE and a
            // loopback redirect, and the desktop client's secret went in the exchange only.
            assert!(url.contains("client_id=123.apps.example"), "{url}");
            assert!(url.contains("code_challenge_method=S256"), "{url}");
            assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A"), "{url}");
            assert!(!url.contains("shh-secret"));
            let exchange = &rig.fake.token_requests()[0];
            assert_eq!(exchange.get("client_secret").map(String::as_str), Some("shh-secret"));
            // The refresh token is in the token store, and in no file of ours.
            assert!(has_token(&rig, "cloud-google"));
            let token = rig.kept.get("cloud-google").unwrap().unwrap();
            let config =
                std::fs::read_to_string(rig.dir.path().join("config.toml")).unwrap_or_default();
            assert!(!config.contains(&token));

            rig.engine.send(Command::LoadSync);
            let seen = rig.watch.until(|e| matches!(e, Event::SyncStatus(_))).await;
            let status = status_of(&seen);
            assert_eq!(status.folder.as_ref().map(|f| f.kind.clone()), Some(google()));
            let row = status.clouds.iter().find(|c| c.service == google()).unwrap();
            assert!(row.signed_in);
            assert_eq!(row.way, SyncCloudWay::Browser);
            // The account Drive names, on the picker's row and the folder's.
            assert_eq!(row.account.as_deref(), Some("kim@mail.example"));
            let folder = status.folder.clone().unwrap();
            assert_eq!(folder.account.as_deref(), Some("kim@mail.example"));
            assert_eq!(status.group_folder, Some(google()));
            assert!(status.recovery_key);
            assert!(status.recovery_key_made.is_some_and(|at| at > 0));

            // Signing out forgets the account with the sign-in.
            rig.engine.send(Command::SyncSignOut { service: google() });
            let seen = rig.watch.until(|e| matches!(e, Event::SyncStatus(_))).await;
            let row = status_of(&seen).clouds.into_iter().find(|c| c.service == google());
            assert_eq!(row.unwrap().account, None);
        }

        #[tokio::test]
        async fn removing_a_cloud_folder_signs_out_of_its_service() {
            let mut rig = rig(CloudClient::Desktop, true, true).await;
            rig.engine.send(Command::StartSync {
                device_name: "Desk".into(),
                folder: folder(google()),
            });
            rig.watch.until(ended(google())).await;
            assert!(has_token(&rig, "cloud-google"));
            rig.engine.send(Command::SyncGroup(SyncCommand::ChangeSyncFolder { folder: None }));
            let seen = rig.watch.until(|e| matches!(e, Event::SyncStatus(_))).await;
            let status = status_of(&seen);
            assert!(status.enabled && status.folder.is_none(), "{status:?}");
            assert!(!has_token(&rig, "cloud-google"));
            let row = status.clouds.into_iter().find(|c| c.service == google()).unwrap();
            assert!(!row.signed_in && row.account.is_none());
        }

        #[tokio::test]
        async fn a_first_set_up_that_fails_leaves_no_folder_and_no_sign_in() {
            let mut rig = rig(CloudClient::Desktop, true, true).await;
            rig.fake.fail_drive(1000, 500, None);
            rig.engine.send(Command::StartSync {
                device_name: "Desk".into(),
                folder: folder(google()),
            });
            let seen = rig.watch.until(ended(google())).await;
            assert!(!problems(&seen).is_empty(), "{seen:?}");
            assert!(!has_token(&rig, "cloud-google"));
            rig.engine.send(Command::LoadSync);
            let seen = rig.watch.until(|e| matches!(e, Event::SyncStatus(_))).await;
            let status = status_of(&seen);
            assert!(status.folder.is_none(), "{status:?}");
            assert!(status.clouds.iter().all(|c| !c.signed_in), "{status:?}");
        }

        #[tokio::test]
        async fn a_tv_never_sets_a_groups_first_folder_up() {
            let mut rig = rig(CloudClient::Tv, false, true).await;
            rig.fake.script_device(&[Step::Done]);
            rig.engine.send(Command::StartSync {
                device_name: "Living room TV".into(),
                folder: folder(google()),
            });
            let seen = rig.watch.until(ended(google())).await;
            assert!(problems(&seen).contains(&SyncProblem::NoGroup), "{seen:?}");
            assert!(!seen.iter().any(|e| matches!(e, Event::SyncRecoveryKey(_))), "{seen:?}");
            // Signed in all the same: the folder can be joined once a phone has set it up.
            assert!(has_token(&rig, "cloud-google"));
            rig.engine.send(Command::LoadSync);
            let seen = rig.watch.until(|e| matches!(e, Event::SyncStatus(_))).await;
            assert!(!status_of(&seen).enabled);
        }

        #[tokio::test]
        async fn a_revoked_sign_in_is_a_problem_to_sign_in_again_from_and_leaving_forgets_it() {
            let mut rig = rig(CloudClient::Desktop, true, true).await;
            rig.engine.send(Command::StartSync {
                device_name: "Desk".into(),
                folder: folder(google()),
            });
            rig.watch.until(ended(google())).await;

            rig.fake.revoke_refresh_tokens();
            rig.fake.expire_access_tokens();
            rig.engine.send(Command::SyncFolderNow);
            let seen = rig
                .watch
                .until(
                    |e| matches!(e, Event::SyncStatus(s) if !s.syncing && s.problem.is_some()),
                )
                .await;
            let needed = SyncProblem::SignInNeeded { service: google() };
            assert!(problems(&seen).contains(&needed), "{seen:?}");
            assert_eq!(status_of(&seen).problem, Some(needed));

            // "Sign in again": a new sign-in, and the road works without a restart.
            rig.engine.send(Command::SyncSignIn { service: google() });
            let seen = rig.watch.until(|e| {
                matches!(e, Event::SyncStatus(s) if !s.syncing && s.problem.is_none() && s.last_synced.is_some())
            })
            .await;
            assert!(!seen.iter().any(|e| matches!(e, Event::SyncProblem(_))), "{seen:?}");

            // Signing out says the same problem; leaving forgets every sign-in.
            rig.engine.send(Command::SyncSignOut { service: google() });
            rig.watch.until(|e| matches!(e, Event::SyncStatus(s) if s.problem.is_some())).await;
            assert!(!has_token(&rig, "cloud-google"));
            rig.engine.send(Command::SyncSignIn { service: google() });
            rig.watch.until(|e| {
                matches!(e, Event::SyncStatus(s) if s.problem.is_none() && s.last_synced.is_some())
            })
            .await;
            assert!(has_token(&rig, "cloud-google"));
            rig.engine.send(Command::LeaveSync);
            rig.watch.until(|e| matches!(e, Event::SyncStatus(s) if !s.enabled)).await;
            assert!(!has_token(&rig, "cloud-google"), "the sign-in goes with the group");
        }

        #[tokio::test]
        async fn a_tv_shows_a_code_and_polls_until_it_is_approved() {
            let mut rig = rig(CloudClient::Tv, false, true).await;
            rig.fake.script_device(&[Step::Pending, Step::SlowDown, Step::Done]);
            rig.engine.send(Command::SyncSignIn { service: google() });
            let seen = rig.watch.until(ended(google())).await;
            let Some(SyncSignInStep::Code {
                user_code,
                verification_url,
                expires_at,
                interval,
            }) = seen.iter().find_map(|e| match e {
                Event::SyncSignIn {
                    step: Some(step @ SyncSignInStep::Code { .. }), ..
                } => Some(step.clone()),
                _ => None,
            })
            else {
                panic!("a code: {seen:?}")
            };
            assert_eq!(user_code, "ABCD-EFGH");
            assert_eq!(verification_url, "https://www.google.example/device");
            assert_eq!(interval, 5);
            assert!(expires_at > kaeri_store::now());
            assert!(has_token(&rig, "cloud-google"));
            assert!(rig.opened.lock().unwrap().is_empty(), "no browser on a TV");
        }

        #[tokio::test]
        async fn a_device_code_that_is_declined_or_expires_is_a_typed_problem() {
            let mut rig = rig(CloudClient::Tv, false, true).await;
            for (step, want) in [
                (Step::Denied, SyncProblem::SignInDenied { service: google() }),
                (Step::Expired, SyncProblem::SignInExpired { service: google() }),
            ] {
                rig.fake.script_device(&[step]);
                rig.engine.send(Command::SyncSignIn { service: google() });
                let seen = rig.watch.until(ended(google())).await;
                assert!(problems(&seen).contains(&want), "{seen:?}");
                assert!(!has_token(&rig, "cloud-google"));
            }
        }

        #[tokio::test]
        async fn cancelling_stops_the_code_screen_and_the_sheet_quietly() {
            let mut rig = rig(CloudClient::Tv, false, true).await;
            rig.engine.send(Command::SyncSignIn { service: google() });
            rig.watch
                .until(|e| {
                    matches!(
                        e,
                        Event::SyncSignIn { step: Some(SyncSignInStep::Code { .. }), .. }
                    )
                })
                .await;
            rig.engine.send(Command::CancelSyncSignIn);
            let seen = rig.watch.until(ended(google())).await;
            assert!(!seen.iter().any(|e| matches!(e, Event::SyncProblem(_))), "{seen:?}");
            assert!(!has_token(&rig, "cloud-google"));

            let mut rig = self::rig(CloudClient::Android, false, true).await;
            rig.engine.send(Command::SyncSignIn { service: SyncFolderKind::Dropbox });
            rig.watch
                .until(|e| {
                    matches!(
                        e,
                        Event::SyncSignIn { step: Some(SyncSignInStep::Sheet { .. }), .. }
                    )
                })
                .await;
            rig.engine.send(Command::CancelSyncSignIn);
            let seen = rig.watch.until(ended(SyncFolderKind::Dropbox)).await;
            assert!(!seen.iter().any(|e| matches!(e, Event::SyncProblem(_))), "{seen:?}");
        }

        #[tokio::test]
        async fn a_phone_signs_in_through_the_sheet_and_only_this_requests_state_counts() {
            let mut rig = rig(CloudClient::Ios, false, true).await;
            rig.engine.send(Command::SyncSignIn { service: SyncFolderKind::OneDrive });
            let seen = rig
                .watch
                .until(|e| {
                    matches!(
                        e,
                        Event::SyncSignIn { step: Some(SyncSignInStep::Sheet { .. }), .. }
                    )
                })
                .await;
            let Some(Event::SyncSignIn {
                step: Some(SyncSignInStep::Sheet { url, callback_scheme }),
                ..
            }) = seen.last()
            else {
                panic!("the sheet")
            };
            assert_eq!(callback_scheme, "kaeri");
            assert!(url.contains("redirect_uri=kaeri%3A%2F%2Fauth%2Fonedrive"), "{url}");

            // A redirect for a page this engine never opened is refused, and said.
            let wrong = Secret("kaeri://auth/onedrive?code=fake-code&state=other".into());
            rig.engine.send(Command::FinishSyncSignIn {
                service: SyncFolderKind::OneDrive,
                callback: wrong,
            });
            let seen = rig.watch.until(ended(SyncFolderKind::OneDrive)).await;
            let denied = SyncProblem::SignInDenied { service: SyncFolderKind::OneDrive };
            assert!(problems(&seen).contains(&denied), "{seen:?}");
            assert!(!has_token(&rig, "cloud-onedrive"));

            rig.engine.send(Command::SyncSignIn { service: SyncFolderKind::OneDrive });
            let state = query(&sheet_url(&mut rig).await, "state");
            let callback =
                Secret(format!("kaeri://auth/onedrive?code=fake-code&state={state}"));
            rig.engine.send(Command::FinishSyncSignIn {
                service: SyncFolderKind::OneDrive,
                callback,
            });
            rig.watch.until(ended(SyncFolderKind::OneDrive)).await;
            assert!(has_token(&rig, "cloud-onedrive"));
            // The Microsoft client is public: no secret went with it.
            assert!(rig.fake.token_requests().iter().all(|r| !r.contains_key("client_secret")));
        }

        /// The next sheet step's page.
        async fn sheet_url(rig: &mut Rig) -> String {
            let seen = rig
                .watch
                .until(|e| {
                    matches!(
                        e,
                        Event::SyncSignIn { step: Some(SyncSignInStep::Sheet { .. }), .. }
                    )
                })
                .await;
            match seen.last() {
                Some(Event::SyncSignIn {
                    step: Some(SyncSignInStep::Sheet { url, .. }),
                    ..
                }) => url.clone(),
                _ => panic!("the sheet"),
            }
        }

        /// A query value of `url`, as sent (the states and challenges need no decoding).
        fn query(url: &str, name: &str) -> String {
            let query = url.split_once('?').map_or("", |(_, q)| q);
            query
                .split('&')
                .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
                .unwrap_or_else(|| panic!("no {name} in {url}"))
                .to_owned()
        }

        // The Mac's Google sign-in that never finished: a second sign-in began over the first
        // while its page was still open (a second click on the picker's row, the row showing no
        // wait). The first page's redirect then reached the second sign-in, which refused it as
        // another request's and waited ten minutes with no page open, while the first sign-in's
        // end closed the picker: nothing on screen, and nothing said.
        #[tokio::test]
        async fn a_second_sign_in_over_an_open_page_takes_that_pages_redirect() {
            let mut rig = rig(CloudClient::Ios, false, true).await;
            for _ in 0..2 {
                rig.engine.send(Command::StartSync {
                    device_name: "Mac".into(),
                    folder: folder(google()),
                });
            }
            let first = sheet_url(&mut rig).await;
            let second = sheet_url(&mut rig).await;
            assert_ne!(query(&first, "state"), query(&second, "state"));
            // The first sign-in's end does not close the second's sheet.
            tokio::time::sleep(Duration::from_millis(200)).await;
            rig.engine.send(Command::LoadSync);
            let seen = rig.watch.until(|e| matches!(e, Event::SyncStatus(_))).await;
            assert!(!seen.iter().any(ended(google())), "{seen:?}");

            // The person finishes the first page.
            let callback = Secret(format!(
                "example.apps.123:/oauth2redirect?state={}&code=fake-code&scope=x",
                query(&first, "state")
            ));
            rig.engine.send(Command::FinishSyncSignIn { service: google(), callback });
            let seen = rig.watch.until(ended(google())).await;
            assert!(problems(&seen).is_empty(), "{seen:?}");
            assert!(seen.iter().any(|e| matches!(e, Event::SyncRecoveryKey(_))), "{seen:?}");
            assert!(has_token(&rig, "cloud-google"));
            // Exchanged with the first page's own verifier, as its code was asked for.
            let sent = rig.fake.token_requests();
            let verifier =
                sent.iter().find_map(|r| r.get("code_verifier")).expect("an exchange");
            assert_eq!(
                kaeri_sync::google::challenge_for(verifier),
                query(&first, "code_challenge")
            );
        }

        #[tokio::test]
        async fn a_refused_late_or_declined_redirect_is_said() {
            let mut rig = rig(CloudClient::Ios, false, true).await;
            let denied = SyncProblem::SignInDenied { service: google() };

            // Declined on the service's page.
            rig.engine.send(Command::SyncSignIn { service: google() });
            let state = query(&sheet_url(&mut rig).await, "state");
            let callback = Secret(format!(
                "example.apps.123:/oauth2redirect?error=access_denied&state={state}"
            ));
            rig.engine.send(Command::FinishSyncSignIn { service: google(), callback });
            let seen = rig.watch.until(ended(google())).await;
            assert!(problems(&seen).contains(&denied), "{seen:?}");

            // A code the service does not take.
            rig.engine.send(Command::SyncSignIn { service: google() });
            let state = query(&sheet_url(&mut rig).await, "state");
            let callback =
                Secret(format!("example.apps.123:/oauth2redirect?state={state}&code=stale"));
            rig.engine.send(Command::FinishSyncSignIn { service: google(), callback });
            let seen = rig.watch.until(ended(google())).await;
            assert!(problems(&seen).contains(&denied), "{seen:?}");

            // A page that comes back with no sign-in waiting.
            let callback = Secret(format!(
                "example.apps.123:/oauth2redirect?state={state}&code=fake-code"
            ));
            rig.engine.send(Command::FinishSyncSignIn { service: google(), callback });
            let seen = rig.watch.until(|e| matches!(e, Event::SyncProblem(_))).await;
            let expired = SyncProblem::SignInExpired { service: google() };
            assert_eq!(problems(&seen), [expired]);
            assert!(!has_token(&rig, "cloud-google"));
        }

        #[tokio::test]
        async fn a_google_sheet_on_ios_comes_back_on_the_reversed_client_id() {
            let mut rig = rig(CloudClient::Ios, false, true).await;
            rig.engine.send(Command::SyncSignIn { service: google() });
            let seen = rig
                .watch
                .until(|e| {
                    matches!(
                        e,
                        Event::SyncSignIn { step: Some(SyncSignInStep::Sheet { .. }), .. }
                    )
                })
                .await;
            let Some(Event::SyncSignIn {
                step: Some(SyncSignInStep::Sheet { callback_scheme, url }),
                ..
            }) = seen.last()
            else {
                panic!("the sheet")
            };
            assert_eq!(callback_scheme, "example.apps.123");
            assert!(url.contains("redirect_uri=example.apps.123%3A%2Foauth2redirect"), "{url}");
            // No secret on iOS.
            assert!(!url.contains("shh-secret"));
        }

        #[tokio::test]
        async fn a_service_with_no_client_says_why_and_does_not_start() {
            let mut rig = rig(CloudClient::Ios, false, false).await;
            rig.engine.send(Command::LoadSync);
            let seen = rig.watch.until(|e| matches!(e, Event::SyncStatus(_))).await;
            let unavailable =
                SyncCloudWay::Unavailable { reason: CloudUnavailable::NoClientId };
            let clouds = status_of(&seen).clouds;
            assert_eq!(clouds.len(), 3);
            assert!(clouds.iter().all(|c| c.way == unavailable && !c.signed_in), "{clouds:?}");

            rig.engine.send(Command::StartSync {
                device_name: "Phone".into(),
                folder: folder(google()),
            });
            let seen = rig.watch.until(ended(google())).await;
            let want = SyncProblem::CloudUnavailable {
                service: google(),
                reason: CloudUnavailable::NoClientId,
            };
            assert!(problems(&seen).contains(&want), "{seen:?}");
            assert!(rig.opened.lock().unwrap().is_empty());
        }

        #[tokio::test]
        async fn an_app_with_no_clients_of_its_own_signs_in_with_kaeris() {
            for (kind, way) in [
                (CloudClient::Ios, SyncCloudWay::Sheet),
                (CloudClient::Android, SyncCloudWay::Sheet),
                (CloudClient::Desktop, SyncCloudWay::Browser),
                (CloudClient::Tv, SyncCloudWay::DeviceCode),
            ] {
                let dir = tempfile::tempdir().unwrap();
                let clients =
                    crate::options::AppClients { cloud_kind: Some(kind), ..Default::default() };
                let host = Arc::new(QuietHost::in_dir(&dir.path().join("tokens")));
                let (engine, mut events) =
                    engine_with(dir.path(), host, |o| o.app_clients = clients).await;
                events.launch().await;
                let mut watch = Watch { events, held: Vec::new() };
                engine.send(Command::LoadSync);
                let seen = watch.until(|e| matches!(e, Event::SyncStatus(_))).await;
                for cloud in status_of(&seen).clouds {
                    let want = if kind == CloudClient::Tv
                        && cloud.service == SyncFolderKind::Dropbox
                    {
                        SyncCloudWay::Unavailable { reason: CloudUnavailable::NeedsPhone }
                    } else if cloud.service == SyncFolderKind::GoogleDrive
                        && !crate::cloud::has_own_google(kind)
                    {
                        SyncCloudWay::Unavailable { reason: CloudUnavailable::NoClientId }
                    } else {
                        way.clone()
                    };
                    assert_eq!(cloud.way, want, "{kind:?} {:?}", cloud.service);
                }
            }
        }

        #[tokio::test]
        async fn a_tv_cannot_sign_in_to_dropbox_and_a_desktop_needs_its_secret() {
            let mut rig = rig(CloudClient::Tv, false, true).await;
            rig.engine.send(Command::LoadSync);
            let seen = rig.watch.until(|e| matches!(e, Event::SyncStatus(_))).await;
            let clouds = status_of(&seen).clouds;
            let way = |kind: SyncFolderKind| {
                clouds.iter().find(|c| c.service == kind).unwrap().way.clone()
            };
            assert_eq!(
                way(SyncFolderKind::Dropbox),
                SyncCloudWay::Unavailable { reason: CloudUnavailable::NeedsPhone }
            );
            assert_eq!(way(google()), SyncCloudWay::DeviceCode);
            assert_eq!(way(SyncFolderKind::OneDrive), SyncCloudWay::DeviceCode);

            let dir = tempfile::tempdir().unwrap();
            let clients = crate::options::AppClients {
                google: Some("123.apps.example".into()),
                cloud_kind: Some(CloudClient::Desktop),
                ..Default::default()
            };
            let host = Arc::new(QuietHost::in_dir(&dir.path().join("tokens")));
            let (engine, mut events) =
                engine_with(dir.path(), host, |o| o.app_clients = clients).await;
            events.launch().await;
            let mut watch = Watch { events, held: Vec::new() };
            engine.send(Command::LoadSync);
            let seen = watch.until(|e| matches!(e, Event::SyncStatus(_))).await;
            let row = status_of(&seen).clouds.into_iter().find(|c| c.service == google());
            assert_eq!(
                row.unwrap().way,
                SyncCloudWay::Unavailable { reason: CloudUnavailable::NoClientId },
                "Google's desktop client is no use without its secret"
            );
        }
    }

    /// Chapter downloads through the commands the apps send, on a fake book source.
    #[cfg(feature = "testing")]
    mod book_downloads {
        use std::sync::atomic::{AtomicBool, Ordering};

        use async_trait::async_trait;
        use kaeri_core::{
            ProviderError,
            books::{
                BookHints, BookSourceKind, BookSourceManifest, ChapterContent, ChapterId,
                PageRef, SeriesDetail, SeriesHit, SourceChapter, WorkId,
            },
            media::Medium,
            traits::{BookSource, PageError},
        };
        use kaeri_model::reading::{
            BookDownloadsView, ChapterDownloadState, DownloadPick, ReadingCommand, ReadingEvent,
        };

        use super::*;

        const WORK: &str = "a:900601";

        /// A source whose second chapter waits for the test before its pages arrive.
        struct Gated {
            manifest: BookSourceManifest,
            published: i64,
            held: AtomicBool,
        }

        #[async_trait]
        impl BookSource for Gated {
            fn manifest(&self) -> &BookSourceManifest {
                &self.manifest
            }

            async fn search(
                &self,
                _: &str,
                _: &BookHints,
            ) -> Result<Vec<SeriesHit>, ProviderError> {
                Ok(Vec::new())
            }

            async fn series(
                &self,
                id: &str,
                _: &BookHints,
            ) -> Result<SeriesDetail, ProviderError> {
                Ok(SeriesDetail::new(id, "The Lantern Ferry"))
            }

            async fn chapters(
                &self,
                _: &str,
                _: &BookHints,
            ) -> Result<Vec<SourceChapter>, ProviderError> {
                Ok((1..=5)
                    .map(|n| {
                        let mut chapter = SourceChapter::new(n.to_string(), n.to_string());
                        chapter.published = Some(self.published);
                        chapter
                    })
                    .collect())
            }

            async fn content(
                &self,
                _: &str,
                chapter: &str,
            ) -> Result<ChapterContent, ProviderError> {
                Ok(ChapterContent::Images(
                    (1..=2)
                        .map(|n| PageRef::new(format!("https://pages.example/{chapter}/{n}")))
                        .collect(),
                ))
            }

            async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
                while self.held.load(Ordering::SeqCst) && page.url.contains("/2/") {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
                bytes.extend_from_slice(&[0; 64]);
                Ok(bytes)
            }
        }

        /// The downloads the engine says once `done` accepts them, asked for again until then.
        async fn downloads_until(
            engine: &Engine,
            events: &mut EventStream,
            done: impl Fn(&BookDownloadsView) -> bool,
        ) -> BookDownloadsView {
            let mut last = None;
            for _ in 0..500 {
                engine.send(Command::Reading(ReadingCommand::LoadDownloads));
                let batch = events.next_events().await;
                for event in batch {
                    if let Event::Reading(ReadingEvent::Downloads(view)) = event {
                        if done(&view) {
                            return view;
                        }
                        last = Some(view);
                    }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            panic!("the downloads never came to it, the last being {last:?}");
        }

        fn states(view: &BookDownloadsView) -> Vec<(String, ChapterDownloadState)> {
            let mut rows: Vec<_> = view
                .rows
                .iter()
                .map(|r| (r.chapter.id.as_str().to_owned(), r.state.clone()))
                .collect();
            rows.sort_by(|a, b| a.0.cmp(&b.0));
            rows
        }

        fn done_ids(view: &BookDownloadsView) -> Vec<String> {
            states(view)
                .into_iter()
                .filter(|(_, s)| *s == ChapterDownloadState::Done)
                .map(|(id, _)| id)
                .collect()
        }

        #[tokio::test]
        async fn download_cancel_delete_and_download_new_run_through_the_engine() {
            let dir = tempfile::tempdir().unwrap();
            let host = Arc::new(QuietHost::in_dir(&dir.path().join("tokens")));
            let source = Arc::new(Gated {
                manifest: BookSourceManifest {
                    id: "shelf".into(),
                    name: "Shelf".into(),
                    version: "1".into(),
                    kind: BookSourceKind::Native,
                    media: vec![Medium::Manga],
                    languages: vec!["en".into()],
                    allowed_hosts: Vec::new(),
                    capabilities: Vec::new(),
                },
                published: kaeri_store::now() - 3_600,
                held: AtomicBool::new(true),
            });
            let held = Arc::clone(&source);
            let (engine, mut events) = engine_with(dir.path(), host, |o| {
                let now = kaeri_store::now();
                o.reading = true;
                o.store.set_book_match("shelf", "s-1", WORK, 100, true, false, 1).unwrap();
                o.store
                    .seed_book_work(WORK, "manga", None, None, "The Lantern Ferry", 1)
                    .unwrap();
                o.store.set_read_status(WORK, Some("reading"), now).unwrap();
                o.book_source = Some(source);
            })
            .await;
            events.launch().await;
            let work = WorkId::new(WORK);
            let ids =
                |list: &[&str]| list.iter().map(|c| ChapterId::new(*c)).collect::<Vec<_>>();

            // The list is kept, which is what Updates reads.
            engine.send(Command::Reading(ReadingCommand::LoadChapters {
                work: work.clone(),
                edition: None,
            }));
            seen_until(&mut events, |e| {
                matches!(e, Event::Reading(ReadingEvent::Chapters { .. }))
            })
            .await;
            let none = downloads_until(&engine, &mut events, |_| true).await;
            assert!(none.rows.is_empty());

            // Three chapters asked for: the first lands, the second waits on its pages, the
            // third is queued behind it.
            engine.send(Command::Reading(ReadingCommand::Download {
                work: work.clone(),
                pick: DownloadPick::Chapters { ids: ids(&["1", "2", "3"]) },
            }));
            let view = downloads_until(&engine, &mut events, |v| {
                let s = states(v);
                s.len() == 3
                    && s[0].1 == ChapterDownloadState::Done
                    && matches!(s[1].1, ChapterDownloadState::Downloading { .. })
                    && s[2].1 == ChapterDownloadState::Queued
            })
            .await;
            assert_eq!(done_ids(&view), ["1"]);

            // Cancelling the queued one leaves the finished and the fetching.
            engine.send(Command::Reading(ReadingCommand::DownloadCancel {
                work: work.clone(),
                chapters: ids(&["3"]),
            }));
            let view = downloads_until(&engine, &mut events, |v| v.rows.len() == 2).await;
            assert_eq!(states(&view)[0].0, "1");

            // The second arrives; deleting the first removes its file and row.
            held.held.store(false, Ordering::SeqCst);
            let view =
                downloads_until(&engine, &mut events, |v| done_ids(v) == ["1", "2"]).await;
            assert_eq!(view.meter.chapters, 2);
            engine.send(Command::Reading(ReadingCommand::DownloadDelete {
                work: work.clone(),
                chapters: ids(&["1"]),
            }));
            let view = downloads_until(&engine, &mut events, |v| v.rows.len() == 1).await;
            assert_eq!(done_ids(&view), ["2"]);

            // What came out and is unread, queued in one go.
            engine.send(Command::Reading(ReadingCommand::DownloadNew));
            let view = downloads_until(&engine, &mut events, |v| done_ids(v).len() == 5).await;
            assert_eq!(view.meter.chapters, 5);
            engine.shutdown().await;
        }

        /// A made-up comic archive of `pages` generated pages.
        fn comic_file(path: &std::path::Path, pages: u32) {
            use kaeri_books::write::CbzWriter;
            let mut w = CbzWriter::new(std::fs::File::create(path).unwrap(), None).unwrap();
            for n in 0..pages {
                let image = image::RgbImage::from_fn(200, 300, |x, y| {
                    image::Rgb([(x ^ y) as u8, n as u8, (x * 3 + y) as u8])
                });
                let mut out = std::io::Cursor::new(Vec::new());
                image.write_to(&mut out, image::ImageFormat::Png).unwrap();
                w.add_page(&out.into_inner()).unwrap();
            }
            w.finish().unwrap();
        }

        async fn store_until(what: &str, done: impl Fn() -> bool) {
            for _ in 0..500 {
                if done() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            panic!("{what} never happened");
        }

        #[tokio::test]
        async fn send_to_e_reader_runs_through_the_engine() {
            let dir = tempfile::tempdir().unwrap();
            let host = Arc::new(QuietHost::in_dir(&dir.path().join("tokens")));
            let mut kept = None;
            let (engine, mut events) = engine_with(dir.path(), host, |o| {
                let now = kaeri_store::now();
                o.reading = true;
                o.store.set_book_match("shelf", "s-1", WORK, 100, true, false, 1).unwrap();
                o.store
                    .seed_book_work(WORK, "manga", None, None, "The Lantern Ferry", 1)
                    .unwrap();
                o.store.set_read_status(WORK, Some("reading"), now).unwrap();
                for (chapter, pages) in [("1", 2), ("2", 2), ("3", 30)] {
                    let path = dir.path().join(format!("{chapter}.cbz"));
                    comic_file(&path, pages);
                    let ask = kaeri_store::book_downloads::BookDownloadAsk {
                        work: WORK,
                        chapter,
                        source_id: "shelf",
                        version_label: None,
                        keep_ready: false,
                    };
                    o.store.queue_book_download(&ask, now).unwrap();
                    let id = o.store.book_downloads(Some(WORK)).unwrap().last().unwrap().id;
                    o.store
                        .book_download_done(
                            id,
                            &path.display().to_string(),
                            Some(pages),
                            10,
                            now,
                        )
                        .unwrap();
                }
                kept = Some(o.store.clone());
            })
            .await;
            let store = kept.unwrap();
            events.launch().await;
            let work = WorkId::new(WORK);
            let ids =
                |list: &[&str]| list.iter().map(|c| ChapterId::new(*c)).collect::<Vec<_>>();

            // The targets are engine data.
            engine.send(Command::Reading(ReadingCommand::LoadExportTargets));
            let seen = seen_until(&mut events, |e| {
                matches!(e, Event::Reading(ReadingEvent::ExportTargets(_)))
            })
            .await;
            let targets = seen
                .iter()
                .find_map(|e| match e {
                    Event::Reading(ReadingEvent::ExportTargets(t)) => Some(t.clone()),
                    _ => None,
                })
                .unwrap();
            assert!(targets.iter().any(|t| t.id == "kobo" && t.width_px == 1264));

            // A send is a job: progress, then the files.
            engine.send(Command::Reading(ReadingCommand::Prepare {
                work: work.clone(),
                pick: DownloadPick::Chapters { ids: ids(&["1", "2"]) },
                target: "kobo".into(),
                format: kaeri_model::reading::ExportFormat::Cbz,
            }));
            let seen = seen_until(&mut events, |e| {
                matches!(e, Event::Reading(ReadingEvent::ExportReady { .. }))
            })
            .await;
            assert!(seen.iter().any(|e| matches!(e, Event::JobStarted { .. })));
            assert!(seen.iter().any(|e| matches!(
                e,
                Event::Reading(ReadingEvent::ExportProgress { total: 4, .. })
            )));
            let files = seen
                .iter()
                .find_map(|e| match e {
                    Event::Reading(ReadingEvent::ExportReady { files, .. }) => {
                        Some(files.clone())
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(files.len(), 1);
            assert!(
                files[0].name.ends_with(".cbz")
                    && std::path::Path::new(&files[0].path).exists()
            );

            // A send that is cancelled leaves no files and never says it is ready.
            engine.send(Command::Reading(ReadingCommand::Prepare {
                work: work.clone(),
                pick: DownloadPick::Chapters { ids: ids(&["3"]) },
                target: "kobo".into(),
                format: kaeri_model::reading::ExportFormat::Cbz,
            }));
            let mut all = seen_until(&mut events, |e| {
                matches!(e, Event::Reading(ReadingEvent::ExportProgress { .. }))
            })
            .await;
            let job = all
                .iter()
                .find_map(|e| match e {
                    Event::Reading(ReadingEvent::ExportProgress { job, .. }) => Some(*job),
                    _ => None,
                })
                .unwrap();
            engine.send(Command::Reading(ReadingCommand::CancelPrepare { job }));
            all.extend(
                seen_until(&mut events, |e| matches!(e, Event::JobDone(j) if *j == job)).await,
            );
            assert!(!all.iter().any(|e| matches!(
                e,
                Event::Reading(ReadingEvent::ExportReady { .. } | ReadingEvent::Failed { .. })
            )));
            assert!(!dir.path().join("data/exports").join(job.get().to_string()).exists());

            // After sharing, the answer to where you stopped becomes a ReadAdopt.
            engine.send(Command::Reading(ReadingCommand::Shared {
                work: work.clone(),
                chapters: ids(&["1", "2"]),
                target: "kobo".into(),
            }));
            store_until("the send being kept", || store.pending_sent(WORK).unwrap().is_some())
                .await;
            engine.send(Command::Reading(ReadingCommand::StoppedAt {
                work: work.clone(),
                chapter: Some(ChapterId::new("2")),
                page: None,
            }));
            store_until("the answer being kept", || {
                store.read_marks(WORK).unwrap().is_read("2")
            })
            .await;
            assert!(store.read_marks(WORK).unwrap().is_read("1"));
            assert!(!store.read_marks(WORK).unwrap().is_read("3"));
            assert!(store.pending_sent(WORK).unwrap().is_none());
            engine.shutdown().await;
        }

        /// A source with real page images, whose second page of every chapter waits for the
        /// test while `held`.
        struct Pages {
            manifest: BookSourceManifest,
            held: AtomicBool,
        }

        #[async_trait]
        impl BookSource for Pages {
            fn manifest(&self) -> &BookSourceManifest {
                &self.manifest
            }

            async fn search(
                &self,
                _: &str,
                _: &BookHints,
            ) -> Result<Vec<SeriesHit>, ProviderError> {
                Ok(Vec::new())
            }

            async fn series(
                &self,
                id: &str,
                _: &BookHints,
            ) -> Result<SeriesDetail, ProviderError> {
                Ok(SeriesDetail::new(id, "The Lantern Ferry"))
            }

            async fn chapters(
                &self,
                _: &str,
                _: &BookHints,
            ) -> Result<Vec<SourceChapter>, ProviderError> {
                Ok((1..=3)
                    .map(|n| {
                        let mut chapter = SourceChapter::new(n.to_string(), n.to_string());
                        chapter.published = Some(kaeri_store::now() - 3_600);
                        chapter
                    })
                    .collect())
            }

            async fn content(
                &self,
                _: &str,
                chapter: &str,
            ) -> Result<ChapterContent, ProviderError> {
                Ok(ChapterContent::Images(
                    (1..=2)
                        .map(|n| PageRef::new(format!("https://pages.example/{chapter}/{n}")))
                        .collect(),
                ))
            }

            async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
                while self.held.load(Ordering::SeqCst) && page.url.ends_with("/2") {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                let image = image::RgbImage::from_fn(200, 300, |x, y| {
                    image::Rgb([(x ^ y) as u8, 7, (x * 3 + y) as u8])
                });
                let mut out = std::io::Cursor::new(Vec::new());
                image.write_to(&mut out, image::ImageFormat::Png).unwrap();
                Ok(out.into_inner())
            }
        }

        fn pages_source(held: bool) -> Arc<Pages> {
            Arc::new(Pages {
                manifest: BookSourceManifest {
                    id: "shelf".into(),
                    name: "Shelf".into(),
                    version: "1".into(),
                    kind: BookSourceKind::Native,
                    media: vec![Medium::Manga],
                    languages: vec!["en".into()],
                    allowed_hosts: Vec::new(),
                    capabilities: Vec::new(),
                },
                held: AtomicBool::new(held),
            })
        }

        /// Every folder under the exports and the downloads that a send or a download left.
        fn leftovers(dir: &std::path::Path) -> Vec<String> {
            let mut found = Vec::new();
            for sub in ["data/exports", "data/book-downloads"] {
                let Ok(entries) = std::fs::read_dir(dir.join(sub)) else { continue };
                for entry in entries.flatten() {
                    found.push(format!("{sub}/{}", entry.file_name().to_string_lossy()));
                }
            }
            found
        }

        #[tokio::test]
        async fn send_fetches_what_is_not_downloaded_and_stops_when_cancelled() {
            let dir = tempfile::tempdir().unwrap();
            let host = Arc::new(QuietHost::in_dir(&dir.path().join("tokens")));
            let source = pages_source(false);
            let held = Arc::clone(&source);
            let mut kept = None;
            let (engine, mut events) = engine_with(dir.path(), host, |o| {
                let now = kaeri_store::now();
                o.reading = true;
                o.store.set_book_match("shelf", "s-1", WORK, 100, true, false, 1).unwrap();
                o.store
                    .seed_book_work(WORK, "manga", None, None, "The Lantern Ferry", 1)
                    .unwrap();
                o.store.set_read_status(WORK, Some("reading"), now).unwrap();
                // Chapter 1 is one the person downloaded.
                let path = dir.path().join("downloaded-1.cbz");
                comic_file(&path, 2);
                let ask = kaeri_store::book_downloads::BookDownloadAsk {
                    work: WORK,
                    chapter: "1",
                    source_id: "shelf",
                    version_label: None,
                    keep_ready: false,
                };
                o.store.queue_book_download(&ask, now).unwrap();
                let id = o.store.book_downloads(Some(WORK)).unwrap()[0].id;
                o.store
                    .book_download_done(id, &path.display().to_string(), Some(2), 10, now)
                    .unwrap();
                o.book_source = Some(source);
                kept = Some(o.store.clone());
            })
            .await;
            let store = kept.unwrap();
            events.launch().await;
            let work = WorkId::new(WORK);
            let ids =
                |list: &[&str]| list.iter().map(|c| ChapterId::new(*c)).collect::<Vec<_>>();
            let prepare = |list: &[&str]| {
                Command::Reading(ReadingCommand::Prepare {
                    work: work.clone(),
                    pick: DownloadPick::Chapters { ids: ids(list) },
                    target: "kobo".into(),
                    format: kaeri_model::reading::ExportFormat::Cbz,
                })
            };

            // One downloaded chapter and one not: both are sent, the second fetched for the
            // send alone.
            engine.send(prepare(&["1", "2"]));
            let seen = seen_until(&mut events, |e| {
                matches!(
                    e,
                    Event::Reading(
                        ReadingEvent::ExportReady { .. } | ReadingEvent::Failed { .. }
                    )
                )
            })
            .await;
            let files = seen
                .iter()
                .find_map(|e| match e {
                    Event::Reading(ReadingEvent::ExportReady { files, .. }) => {
                        Some(files.clone())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("the send failed: {seen:?}"));
            assert_eq!(files.len(), 1);
            let book = kaeri_books::open(
                std::path::Path::new(&files[0].path),
                &kaeri_books::Limits::default(),
            )
            .unwrap();
            let kaeri_books::Book::Comic(comic) = book else { panic!("not a comic") };
            assert_eq!(comic.pages().len(), 4);
            // Progress covered the fetching and the preparing on one scale.
            assert!(seen.iter().any(|e| matches!(
                e,
                Event::Reading(ReadingEvent::ExportProgress { total: 200, .. })
            )));
            // The fetched chapter is not a download, and no file of it is left.
            let rows = store.book_downloads(Some(WORK)).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].chapter, "1");
            let left = leftovers(dir.path());
            assert!(
                left.iter().all(|l| !l.ends_with("-fetch") && !l.contains("book-downloads")),
                "{left:?}"
            );

            // Cancelled while the second chapter's pages are coming: nothing is sent, nothing
            // is kept, and the fetch stops.
            held.held.store(true, Ordering::SeqCst);
            engine.send(prepare(&["1", "2", "3"]));
            let mut all = seen_until(&mut events, |e| {
                matches!(e, Event::Reading(ReadingEvent::ExportProgress { .. }))
            })
            .await;
            let job = all
                .iter()
                .find_map(|e| match e {
                    Event::Reading(ReadingEvent::ExportProgress { job, .. }) => Some(*job),
                    _ => None,
                })
                .unwrap();
            engine.send(Command::Reading(ReadingCommand::CancelPrepare { job }));
            held.held.store(false, Ordering::SeqCst);
            all.extend(
                seen_until(&mut events, |e| matches!(e, Event::JobDone(j) if *j == job)).await,
            );
            assert!(!all.iter().any(|e| matches!(
                e,
                Event::Reading(ReadingEvent::ExportReady { .. } | ReadingEvent::Failed { .. })
            )));
            let rows = store.book_downloads(Some(WORK)).unwrap();
            assert_eq!(rows.len(), 1, "a send adds no downloads");
            let left = leftovers(dir.path());
            assert!(
                left.iter().all(|l| !l.ends_with("-fetch") && !l.contains("book-downloads")),
                "{left:?}"
            );
            engine.shutdown().await;
        }

        #[cfg(feature = "opds-feed")]
        #[tokio::test]
        async fn the_feed_password_is_kept_in_the_secure_store_not_the_config() {
            use kaeri_core::settings::SettingValue;
            use kaeri_model::settings::SettingKey;
            use kaeri_track::{TokenStore, secret::Backend};
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            const PASSWORD: &str = "made-up-feed-password";
            let dir = tempfile::tempdir().unwrap();
            // The file storage `KAERI_TOKEN_STORAGE=file` picks: a 0600 file under tokens/.
            let host = Arc::new(QuietHost::in_dir(&dir.path().join("tokens")));
            let (engine, mut events) =
                engine_with(dir.path(), host, |o| o.reading = true).await;
            events.launch().await;
            engine.send(Command::SetSetting {
                key: SettingKey::OpdsPassword,
                value: SettingValue::Str(PASSWORD.into()),
            });
            engine.send(Command::SetSetting {
                key: SettingKey::OpdsFeed,
                value: SettingValue::Bool(true),
            });
            let seen = seen_until(
                &mut events,
                |e| matches!(e, Event::Reading(ReadingEvent::Feed(v)) if v.on),
            )
            .await;
            let view = seen
                .iter()
                .find_map(|e| match e {
                    Event::Reading(ReadingEvent::Feed(v)) if v.on => Some(v.clone()),
                    _ => None,
                })
                .unwrap();
            assert!(view.password_set);

            // The store has it; the config file says only that one is set.
            let tokens = TokenStore::with_backend(dir.path().join("tokens"), Backend::File);
            assert_eq!(tokens.get("opds-feed").unwrap(), PASSWORD);
            let config = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
            assert!(!config.contains(PASSWORD), "{config}");
            assert!(config.contains("password_set = true"), "{config}");

            // And the feed signs in with it.
            for (password, status) in [(PASSWORD, "200"), ("wrong", "401")] {
                use base64::Engine as _;
                let auth = base64::engine::general_purpose::STANDARD
                    .encode(format!("reader:{password}"));
                let mut stream =
                    tokio::net::TcpStream::connect(("127.0.0.1", view.port)).await.unwrap();
                let request = format!(
                    "GET /opds HTTP/1.1\r\nHost: feed.example\r\nAuthorization: Basic {auth}\r\nConnection: close\r\n\r\n"
                );
                stream.write_all(request.as_bytes()).await.unwrap();
                let mut reply = String::new();
                stream.read_to_string(&mut reply).await.unwrap();
                assert!(reply.starts_with(&format!("HTTP/1.1 {status}")), "{reply}");
            }

            // Clearing it forgets it and stops the feed.
            engine.send(Command::SetSetting {
                key: SettingKey::OpdsPassword,
                value: SettingValue::Str(String::new()),
            });
            seen_until(&mut events, |e| {
                matches!(e, Event::Reading(ReadingEvent::Feed(v)) if !v.on && !v.password_set)
            })
            .await;
            // A store of its own, since this one remembers what it read.
            let fresh = TokenStore::with_backend(dir.path().join("tokens"), Backend::File);
            assert!(fresh.get("opds-feed").is_err());
            engine.shutdown().await;
        }
    }
}
