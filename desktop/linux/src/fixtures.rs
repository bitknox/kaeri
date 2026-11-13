//! The engine on the test kit's scene, for screenshots and for trying the app with nothing set
//! up: the fake AniList on loopback with the fixture titles, signed in so the Library fills, and
//! four titles under way, as the phones' Fixtures builds show them. Every episode streams the
//! clip a run hands in. Nothing leaves the machine: the client cannot resolve a host name.

use std::{path::Path, sync::Arc};

use kaeri_core::{
    config::NetworkConfig,
    media::{Episode, MediaFormat, SearchHit, Translation},
    stream::{Stream, StreamKind},
};
use kaeri_engine::{
    clock::Clock,
    engine::Engine,
    host::Host,
    options::{AppClients, Background, EngineOptions, Parts},
    plugin_stores, tracking,
};
use kaeri_meta::Meta;
use kaeri_net::HttpClient;
use kaeri_providers::{MockProvider, ProviderRegistry};
use kaeri_store::{DownloadState, Store, WatchEvent};
use kaeri_testkit::{
    Failure, FakeAniList, Operation,
    fixtures::{self, AIRING, AIRING_NO_MAL, FINISHED, ONA},
    home_world::{HomeClock, HomeWorld},
    seed, sources,
};
use kaeri_track::{
    AniListTracker,
    secret::{Backend, TokenStore},
};

use crate::engine::{Started, open, paths_under, theme};

/// Whether the app runs on the fixture scene, which has no network.
static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the app runs on the fixture scene.
pub fn active() -> bool {
    ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
}

/// The engine's runtime, for the scene's party server to run on.
static RUNTIME: std::sync::OnceLock<tokio::runtime::Handle> = std::sync::OnceLock::new();

/// Where the fake AniList serves its art, `http://…/art/`, for shots that draw covers of their own.
pub static ART_BASE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Keep `handle`, the engine's runtime, for [`FakeParty::start`].
pub fn keep_runtime(handle: tokio::runtime::Handle) {
    ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = RUNTIME.set(handle);
}

/// A Syncplay server on loopback with members the scene plays, so a party can be joined,
/// steered and left with nothing on the network.
pub struct FakeParty {
    server: kaeri_party::fake::FakeServer,
    members:
        std::cell::RefCell<std::collections::HashMap<String, kaeri_party::fake::FakeMember>>,
}

impl FakeParty {
    /// A server that takes every member, or refuses each with `refuse`.
    pub fn start(refuse: Option<&str>) -> Option<Self> {
        use kaeri_party::fake::{FakeOptions, FakeServer};
        let handle = RUNTIME.get()?;
        let options =
            FakeOptions { refuse: refuse.map(str::to_owned), ..FakeOptions::default() };
        let server = handle
            .block_on(handle.spawn(FakeServer::start(options)))
            .ok()?
            .inspect_err(
                |e| tracing::warn!(error = %e, "the scene's party server did not start"),
            )
            .ok()?;
        Some(Self { server, members: Default::default() })
    }

    /// Where to join it, as `host:port`.
    pub fn server(&self) -> String {
        self.server.address()
    }

    fn with(&self, act: impl FnOnce(&kaeri_party::fake::FakeServer)) {
        let Some(handle) = RUNTIME.get() else { return };
        let _entered = handle.enter();
        act(&self.server);
    }

    fn member(&self, name: &str, act: impl FnOnce(&kaeri_party::fake::FakeMember)) {
        let Some(handle) = RUNTIME.get() else { return };
        let _entered = handle.enter();
        if let Some(member) = self.members.borrow().get(name) {
            act(member);
        }
    }

    /// `name` comes into `room`.
    pub fn add(&self, room: &str, name: &str) {
        let mut member = None;
        self.with(|server| member = Some(server.member(room, name)));
        if let Some(member) = member {
            self.members.borrow_mut().insert(name.to_owned(), member);
        }
    }

    /// The member opens `file`, `duration` seconds long.
    pub fn opens(&self, name: &str, file: &str, duration: f64) {
        self.member(name, |m| m.open(file, duration));
    }

    /// The member sets the room's playlist: the current episode, then the next.
    pub fn playlist(&self, name: &str, files: &[&str]) {
        self.member(name, |m| {
            m.set_playlist(files);
            m.set_index(0);
        });
    }

    pub fn ready(&self, name: &str, ready: bool) {
        self.member(name, |m| m.ready(ready));
    }

    pub fn says(&self, name: &str, message: &str) {
        self.member(name, |m| m.chat(message));
    }

    /// The member reacts `at` seconds into the episode.
    pub fn reacts(&self, name: &str, reaction: kaeri_party::Reaction, at: f64) {
        self.member(name, |m| m.chat(&kaeri_party::reaction::encode(reaction, at)));
    }

    /// The server turns chat on or off, and says how long a line may be to members who join
    /// after this.
    pub fn set_chat(&self, on: bool, max_length: Option<u32>) {
        self.with(|server| server.set_chat(on, max_length));
    }

    /// The server takes chat and sends it to no one, so a sender's line never comes back.
    pub fn swallow_chat(&self, swallow: bool) {
        self.with(|server| server.swallow_chat(swallow));
    }

    /// The server goes away, as one lost does: every connection dropped and none taken.
    pub fn go_down(&self, down: bool) {
        self.with(|server| {
            server.set_down(down);
            if down {
                server.drop_connections();
            }
        });
    }
}

/// An address that takes a connection and never answers, for the form waiting on a room.
pub fn silent_server() -> Option<(std::net::TcpListener, String)> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
    let address = listener.local_addr().ok()?.to_string();
    Some((listener, address))
}

/// The environment variable that gives the scene Home's world: invented titles dated by the
/// clock, enough of them airing this season to fill every shelf. The same one the phones'
/// Fixtures builds read.
pub const HOME_WORLD_VAR: &str = "KAERI_FIXTURES_HOME_WORLD";

/// Set by the offline scene, so the network path says offline as the device would.
pub static OFFLINE_SCENE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Set by the reading scene, so the network path says online and the chapters queued there
/// download, from the scene's own sources. The other scenes leave the network unsaid: their
/// seeded queues, the downloads scene's rows in every state among them, hold still for shots.
pub static ONLINE_SCENE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// How long a slow scene holds each of Home's discovery answers: long enough for the shots to
/// catch the placeholders and the activity line after four seconds, inside the wait the slow
/// scene gives the client.
const SLOW_ANSWER: std::time::Duration = std::time::Duration::from_secs(10);
/// How long the client waits for an answer: twice the slow answer in the slow scene.
const fn client_wait(scene: HomeScene) -> u64 {
    if matches!(scene, HomeScene::Slow) { 20 } else { 5 }
}

/// What Home and the app around it start from: the board's states, as the phones' Fixtures scenes
/// name them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeScene {
    /// Signed in, four titles under way: case 1, Continue at 4.
    #[default]
    Watching,
    /// Signed out, nothing watched: case 3, Continue at 0.
    Fresh,
    /// Signed in, the one title finished: case 2, Continue at 0.
    Suggested,
    /// Signed out with one title under way: case 1, Continue at 1.
    OneLeft,
    /// Fifteen titles under way: twelve shown, View all and the count.
    ContinueFull,
    /// Signed in, nothing watched, and a source that has nothing: Add to Planning.
    NotFound,
    /// Watching with every discovery answer held past the activity line.
    Slow,
    /// Watching with every discovery request failing.
    Failed,
    /// Nothing watched and no network.
    Offline,
    /// Watching, with reading on and set to Both: a manga, a novel and a strip with a history.
    Reading,
    /// [`Self::Reading`] with keeping chapters ready on and the Downloads place filled.
    ReadingDownloads,
    /// [`Self::Reading`] with the manga a long series: 120 chapters listed, 1 to 59 read and
    /// chapter 60 at page 9 of 20.
    ReadingLong,
    /// Watching with no source installed.
    Sourceless,
}

impl HomeScene {
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "watching" => Self::Watching,
            "fresh" => Self::Fresh,
            "suggested" => Self::Suggested,
            "one-left" => Self::OneLeft,
            "continue-full" => Self::ContinueFull,
            "not-found" => Self::NotFound,
            "slow" => Self::Slow,
            "failed" => Self::Failed,
            "offline" => Self::Offline,
            "sourceless" => Self::Sourceless,
            "reading" => Self::Reading,
            "reading-downloads" => Self::ReadingDownloads,
            "reading-long" => Self::ReadingLong,
            _ => return None,
        })
    }

    /// Whether the scene has reading on, with its books and history.
    fn reading(self) -> bool {
        matches!(self, Self::Reading | Self::ReadingDownloads | Self::ReadingLong)
    }

    fn signed_in(self) -> bool {
        !matches!(self, Self::Fresh | Self::OneLeft | Self::Offline)
    }

    /// The world's own history: finished and caught-up shows, none of them in Continue.
    fn world_history(self) -> bool {
        matches!(
            self,
            Self::Watching | Self::Suggested | Self::Slow | Self::Failed | Self::Sourceless
        )
    }
}

/// What the scene starts with beyond its history.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    /// A file every episode streams, for the player.
    pub media: Option<String>,
    /// An episode downloading, so closing the window asks first.
    pub downloading: bool,
    /// The state Home starts in.
    pub home: HomeScene,
    /// A source ahead of the fixture one that cannot choose the finished title's match, so its
    /// page asks for the match: the match picker's scene.
    pub unsure: bool,
    /// A first launch: the demo is the only source, on, and nothing is watched. It reaches a
    /// server of the scene's that never answers, not Commons.
    pub demo: bool,
}

/// Whether the environment asks for Home's world.
fn world_asked() -> bool {
    std::env::var(HOME_WORLD_VAR).is_ok_and(|v| v == "1")
}

/// Start the engine on the scene, its files under `root`.
pub async fn start(root: &Path, scene: &Scene, host: Arc<dyn Host>) -> Result<Started, String> {
    let paths = paths_under(root);
    let signed_in = scene.home.signed_in();
    if signed_in {
        write_config(&paths.config_file, scene.demo, scene.home)?;
    }
    let (config, store, _) = open(&paths)?;
    let clock = Clock::default();
    let now = clock.now();
    let world = world_asked().then(|| HomeWorld::at(HomeClock::in_zone(now, &clock.zone())));
    if !scene.demo {
        seed_scene(&store, scene.home, now);
        if scene.home.reading() {
            crate::fixtures_reading::seed_books(
                &store,
                &paths.data_dir,
                scene.home == HomeScene::ReadingLong,
                now,
            );
        }
    }
    if let Some(world) = world.as_ref().filter(|_| scene.home.world_history()) {
        world.seed_history(&store, now);
        let mut statuses = fixtures::library_statuses();
        statuses.extend(world.library_statuses());
        store
            .replace_library_statuses("anilist", &statuses, now)
            .map_err(|e| format!("seeding the list's statuses: {e}"))?;
    }
    if scene.downloading && store.download_for(AIRING, "4").ok().flatten().is_none() {
        // A queue with a row in every state.
        seed::download(&store, FINISHED, "1", DownloadState::Done);
        seed::download(&store, FINISHED, "2", DownloadState::Done);
        seed::download(&store, AIRING, "4", DownloadState::Active);
        seed::download(&store, AIRING, "5", DownloadState::Queued);
        seed::download(&store, ONA, "3", DownloadState::Paused);
        seed::download(&store, AIRING_NO_MAL, "2", DownloadState::Failed);
    }
    let tokens = TokenStore::with_backend(paths.data_dir.join("tokens"), Backend::File);
    if signed_in {
        tokens.set("anilist", "fixture-anilist-token").map_err(|e| e.to_string())?;
    }

    let server = FakeAniList::start().await;
    server.set_now(now);
    server.add_titles(fixtures::long_franchise());
    server.serve_art();
    let _ = ART_BASE.set(server.art_base());
    if !scene.demo && scene.home.reading() {
        crate::fixtures_reading::keep_books(&store, &server.art_base(), now);
    }
    crate::hunt::seed(&store, now);
    // Home's world: this season's titles, the rest in the browse, and their broadcasts.
    if let Some(world) = &world {
        let (season, rest): (Vec<_>, Vec<_>) = world
            .titles
            .iter()
            .cloned()
            .partition(|t| world.this_season.iter().any(|id| t["id"] == id.get()));
        server.add_season(season);
        server.add_catalogue(rest);
        server.add_broadcasts(world.broadcasts.iter().copied());
        if scene.home.world_history() {
            server.add_entries(world.entries.iter().cloned());
        }
    }
    match scene.home {
        HomeScene::Slow => {
            for operation in DISCOVERY {
                server.answer_slowly(operation, SLOW_ANSWER);
            }
        }
        HomeScene::Failed => {
            for operation in DISCOVERY {
                server.fail_operation(operation, Failure::Status(500));
            }
        }
        HomeScene::Offline => {
            OFFLINE_SCENE.store(true, std::sync::atomic::Ordering::Relaxed);
            server.fail_always(Failure::Disconnect)
        }
        HomeScene::Reading | HomeScene::ReadingLong if !scene.demo => {
            ONLINE_SCENE.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        _ => {}
    }
    let http = HttpClient::offline(&NetworkConfig {
        timeout_secs: client_wait(scene.home),
        ..config.network.clone()
    })
    .map_err(|e| e.to_string())?;
    let meta = Meta::new(server.anilist(&http), None, store.clone());
    let tracked = tracking::native::with_app_clients(&config, &AppClients::default());
    let mut sync = tracking::Sync::with_tokens(&tracked, &store, &http, tokens.clone());
    if let Some(at) = sync.trackers.iter().position(|t| t.id() == "anilist") {
        let client = server.anilist(&http).with_token(tokens.get("anilist").ok());
        let tracker = Arc::new(AniListTracker::new(client));
        sync.trackers[at] = tracker.clone();
        sync.anilist = Some(tracker);
    }
    let stores =
        plugin_stores::Stores::with_tokens(&config, &paths, tokens).map_err(|m| m.english())?;

    let mut options = EngineOptions::new(paths, config.clone(), http, store, theme());
    options.saved = Some(config);
    options.hosted_player = true;
    options.reading = true;
    options.background = Background::none();
    let provided = world.as_ref().map(HomeWorld::provided).unwrap_or_default();
    let chain = match scene.home {
        _ if scene.demo => demo_registry(),
        HomeScene::Sourceless => ProviderRegistry::new(Vec::new()),
        HomeScene::NotFound => ProviderRegistry::new(vec![
            kaeri_providers::MockProvider::new(sources::PROVIDER_ID)
                .with_hits(Vec::new())
                .arc(),
        ]),
        _ => registry(&server.art_base(), scene.media.as_deref(), &provided, scene.unsure),
    };
    // A setting that changes which sources exist rebuilds the chain from the config, and the
    // scene's own source would be gone for every shot after it: it goes back in place.
    let (kept, providers) = (chain.clone(), chain.all());
    let rebuilt: Box<dyn Fn() + Send> = Box::new(move || kept.replace(providers.clone()));
    options.sources = Some(chain);
    options.book_source = Some(crate::hunt::source());
    if let Ok(free) = std::env::var("HUNT_FREE") {
        options.free_space = free.parse().ok();
    }
    options.parts =
        Some(Parts { meta, sync, stores, clock, fixture_route: true, windows: None });
    let (engine, events): (Engine, _) =
        Engine::start(options, host).await.map_err(|e| e.to_string())?;
    Ok(Started { engine, events, keep: Some(Box::new(server)), rebuilt: Some(rebuilt) })
}

/// What Home asks AniList for beyond its queue's titles and their broadcasts: the season's
/// lists, the genre shelf, recommendations and the season walk. The slow and failing scenes hold
/// these up.
const DISCOVERY: [Operation; 6] = [
    Operation::Browse,
    Operation::Search,
    Operation::Seasonal,
    Operation::Recommended,
    Operation::Relations,
    Operation::Schedule,
];

/// The fixture provider, its stills from the fake, every episode streaming `media` when given.
/// The demo alone, standing at an address that takes a connection and never answers: playing it
/// shows the starting screen and never reaches Commons.
fn demo_registry() -> ProviderRegistry {
    let base = silent_server().map_or_else(
        || "http://127.0.0.1:9".to_owned(),
        |(listener, address)| {
            // Kept for the run: a listener that is dropped refuses, and the screen would fail.
            std::mem::forget(listener);
            format!("http://{address}")
        },
    );
    let demo = kaeri_providers::demo::DemoProvider::with_endpoints(base.clone(), base);
    ProviderRegistry::new(vec![Arc::new(demo)])
}

/// The fixture provider, its stills from the fake, every episode streaming `media` when given.
/// `provided` is Home's world's titles with the episodes out of each, which it has too.
fn registry(
    art: &str,
    media: Option<&str>,
    provided: &[(kaeri_meta::Media, u32)],
    unsure: bool,
) -> ProviderRegistry {
    let mut provider =
        sources::with_long_franchise_and(sources::provider_with_art(art), provided);
    if let Some(media) = media {
        let stream = Stream {
            provider_id: sources::PROVIDER_ID.into(),
            ..Stream::new(media, StreamKind::Mp4)
        };
        let world = provided.iter().map(|p| p.0.id);
        for id in fixtures::ALL.into_iter().chain(world) {
            provider = provider.with_streams_for(sources::key(id), vec![stream.clone()]);
        }
    }
    if unsure {
        let clip = match media {
            Some(media) => Stream::new(media, StreamKind::Mp4),
            None => sources::episode_clip(),
        };
        return ProviderRegistry::new(vec![unsure_source(art, &clip).arc(), provider.arc()]);
    }
    ProviderRegistry::new(vec![provider.arc()])
}

/// The id of the source the unsure scene puts first.
pub const UNSURE_SOURCE: &str = "hotaru-open-archive";

/// The source the unsure scene puts first: an invented archive that lists four titles for
/// Hoshikuzu no Tabibito, two that could be it and two ruled out (one by its year, one by its
/// format), and the show itself for a search by its English title.
fn unsure_source(art: &str, clip: &Stream) -> MockProvider {
    let hit =
        |key: &str, title: &str, episodes: u32, year: u16, format: MediaFormat| SearchHit {
            episode_count: Some(episodes),
            year: Some(year),
            format: Some(format),
            ..SearchHit::new(format!("{UNSURE_SOURCE}:{key}"), title)
        };
    let show = format!("{UNSURE_SOURCE}:stardust-wayfarer");
    let episodes = sources::episodes(FINISHED)
        .into_iter()
        .map(|e| Episode {
            thumbnail: e.thumbnail.map(|t| t.replacen(fixtures::ART_HOST, art, 1)),
            ..e
        })
        .collect();
    let clip = Stream { provider_id: UNSURE_SOURCE.into(), ..clip.clone() };
    MockProvider::new(UNSURE_SOURCE)
        .named("Hotaru Open Archive")
        .with_hits_for(
            "Hoshikuzu no Tabibito",
            vec![
                hit("hoshi-tabi", "Hoshi Tabi Diaries", 12, 2024, MediaFormat::Tv),
                hit("tabibito-hoshikuzu", "Tabibito to Hoshikuzu", 13, 2025, MediaFormat::Tv),
                hit("hoshikuzu-2019", "Hoshikuzu no Tabibito", 10, 2019, MediaFormat::Tv),
                hit("hoshikuzu-ova", "Hoshikuzu no Tabibito: Yoru", 1, 2024, MediaFormat::Ova),
            ],
        )
        .with_hits_for(
            "Stardust Wayfarer",
            vec![
                hit("stardust-wayfarer", "Stardust Wayfarer", 12, 2024, MediaFormat::Tv),
                hit(
                    "stardust-specials",
                    "Stardust Wayfarer Specials",
                    2,
                    2024,
                    MediaFormat::Special,
                ),
            ],
        )
        .with_episodes_for(show.clone(), episodes)
        .with_streams_for(show, vec![clip])
}

/// AniList on, as a signed-in user has it, tokens in files, and skips offered rather than taken.
fn write_config(file: &Path, demo: bool, home: HomeScene) -> Result<(), String> {
    if file.exists() {
        return Ok(());
    }
    let dir = file.parent().ok_or("the config has no folder")?;
    std::fs::create_dir_all(dir)
        .and_then(|()| {
            std::fs::write(
                file,
                format!(
                    "[trackers]\nenabled = [\"anilist\"]\ntoken_storage = \"file\"\n\n\
                 [trackers.anilist]\nclient_id = \"fixture\"\nclient_secret = \"fixture\"\n\n\
                 [playback]\nskip_opening = false\nupscaling = \"off\"\n{}{}",
                    if demo { "\n[sources]\ndemo = true\n" } else { "" },
                    match home {
                        HomeScene::Reading | HomeScene::ReadingLong =>
                            "\n[reading]\nkaeri_is_for = \"both\"\n",
                        HomeScene::ReadingDownloads => {
                            "\n[reading]\nkaeri_is_for = \"both\"\nkeep_chapters_ready = true\n"
                        }
                        _ => "",
                    }
                ),
            )
        })
        .map_err(|e| format!("{}: {e}", file.display()))
}

/// The history `scene` describes, the newest watch an hour before `now`.
fn seed_scene(store: &Store, scene: HomeScene, now: i64) {
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;
    match scene {
        HomeScene::Fresh | HomeScene::NotFound | HomeScene::Offline => {}
        HomeScene::Suggested => {
            let all = fixtures::media(FINISHED).episodes.unwrap_or(1);
            seed::watched(store, FINISHED, all, now - HOUR);
        }
        HomeScene::OneLeft => seed::watched(store, AIRING_NO_MAL, 7, now - DAY),
        HomeScene::ContinueFull => {
            let long = fixtures::long_franchise_media();
            let shows = fixtures::ALL.into_iter().chain(long.iter().map(|m| m.id)).take(15);
            // Oldest first, an hour apart, each stopped five minutes into episode 1.
            for (n, id) in shows.collect::<Vec<_>>().into_iter().rev().enumerate() {
                seed::stopped_at(store, id, "1", 5.0 * 60.0, now - (15 - n as i64) * HOUR);
            }
        }
        HomeScene::Watching | HomeScene::Slow | HomeScene::Failed | HomeScene::Sourceless => {
            seed_history(store, now)
        }
        HomeScene::Reading | HomeScene::ReadingDownloads | HomeScene::ReadingLong => {
            seed_history(store, now);
            crate::fixtures_reading::seed_reading(
                store,
                scene == HomeScene::ReadingDownloads,
                scene == HomeScene::ReadingLong,
                now,
            );
        }
    }
}

/// The phones' Watching scene: four titles under way, the newest an hour before `now`.
fn seed_history(store: &Store, now: i64) {
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;
    if store.continue_list(1).is_ok_and(|rows| !rows.is_empty()) {
        return;
    }
    seed::watched(store, AIRING_NO_MAL, 7, now - 3 * DAY);
    seed::watched(store, AIRING, 3, now - 2 * DAY);
    seed::watched(store, ONA, 1, now - DAY - HOUR);
    // Switched to the dub after episode 1, so its Continue row is marked "Dub".
    let dubbed = WatchEvent {
        duration_secs: Some(fixtures::runtime_secs(ONA)),
        watched_secs: 13.0 * 60.0 + 30.0,
        provider_id: Some(sources::PROVIDER_ID.to_owned()),
        translation: Some(Translation::Dub),
        completed: false,
        at: now - DAY,
        ..WatchEvent::new(ONA, "2", 13.0 * 60.0 + 30.0)
    };
    store.record_event(&dubbed).expect("seeding a dubbed episode");
    seed::watched(store, FINISHED, 6, now - 2 * HOUR);
    seed::stopped_at(store, FINISHED, "7", 10.0 * 60.0 + 6.0, now - HOUR);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scene_leaves_four_titles_to_continue_with_the_newest_first() {
        let root = tempfile::tempdir().expect("a scratch directory");
        let (_config, store, _) = open(&paths_under(root.path())).expect("it opens");
        seed_history(&store, 1_800_000_000);
        seed_history(&store, 1_800_000_000);
        let ids: Vec<_> =
            store.continue_list(15).expect("the rail").iter().map(|p| p.anilist_id).collect();
        assert_eq!(ids, [FINISHED, ONA, AIRING, AIRING_NO_MAL], "seeded once, newest first");
        let dubbed: Vec<_> =
            ids.iter().map(|id| store.last_translation(*id).expect("a read")).collect();
        let sub = Some(Translation::Sub);
        assert_eq!(dubbed, [sub, Some(Translation::Dub), sub, sub], "one watched dubbed");
    }
}
