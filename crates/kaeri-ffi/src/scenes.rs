//! The engine started on a scene: the test kit's fixtures with a history already in the
//! database, and the fake AniList answering at the real time, for the Apple app's Fixtures build
//! to show its screens filled.

use std::sync::Arc;

use kaeri_core::media::{Episode, MediaFormat, SearchHit, Translation};
use kaeri_core::stream::{Stream, StreamKind};
use kaeri_engine::clock::Clock;
use kaeri_meta::Media;
use kaeri_model::Msg;
use kaeri_providers::{MockProvider, ProviderRegistry};
use kaeri_store::Store;
use kaeri_testkit::{
    Failure, FakeAniList, Operation,
    fixtures::{self, AIRING, AIRING_NO_MAL, FINISHED, ONA},
    home_world::{HomeClock, HomeWorld},
    seed, sources,
};
use kaeri_track::secret::{Backend, TokenStore};

use crate::{
    engine::{FfiEngine, Opened, RemoteHome, runtime},
    host::{Adapter, FfiHost},
    start::{FfiDirs, FfiError, FfiStart, SecretsMode},
    testing,
};

/// What a scene puts in the database before the engine starts on it.
///
/// Every scene's fake AniList also has the test kit's long franchise
/// ([`fixtures::LONG_FRANCHISE_SEASONS`], twelve seasons of Cloudcutter Airship) and the short one
/// ([`fixtures::FRANCHISE_SEASONS`], five seasons of Lantern Wood), in no list, search or
/// schedule: their pages open by id.
///
/// With the environment variable [`HOME_WORLD_VAR`] set to `1` when a scene starts, the scene
/// has Home's world too ([`HomeWorld`], ids 950001 to 950036): invented titles dated by the
/// scene's clock, fourteen of them airing this season, so This season, Airing today, Search,
/// Seasonal and the Calendar have a season's worth whatever the date. The signed-in scenes with
/// a history ([`Self::Watching`], its kin and [`Self::Suggested`]) also have the world's
/// history: four shows finished here and Completed on the list, The Forest Courier last, three
/// caught up and Watching, and two on Planning. None of it is in Continue, so Home's shelves
/// fill as the design board draws them and Continue is as it was. It is for Home's shots: the
/// caught-up shows and what airs today change the new-episode notices and the widgets' record,
/// which the apps' tests read from the scenes as they are without it.
///
/// With the environment variable `KAERI_FIXTURES_CATALOGUE` set to `1` when a scene starts,
/// eight more invented titles ([`fixtures::catalogue_titles`], ids 900101 to 900108) join the
/// six in Search and browse, for a grid of fourteen: no scene's own data changes, and with the
/// variable unset or anything else the six-title world is exactly as it was.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiScene {
    /// Nothing: a first launch, as [`FfiEngine::start_testing`] starts.
    Fresh,
    /// Signed in to the fake AniList, whose library then fills the Library, and four titles
    /// under way, newest first, as Continue shows them:
    ///
    /// - Hoshikuzu no Tabibito, episodes 1 to 6 watched and episode 7 left at 10:06 of 24:00;
    /// - Hagane no Niwashi, episode 1 watched and episode 2 left at 13:30 of 18:00;
    /// - Kumo no Ue no Toshokan, episodes 1 to 3 watched, so episode 4 is next;
    /// - Natsukaze Relay, episodes 1 to 7 watched, and episode 8 out since 19 hours ago.
    Watching,
    /// [`Self::Watching`] with no source set up: every title's episodes come from AniList's own
    /// data, marked so, and a page offers to add a source in place of Play.
    Sourceless,
    /// A scene that plays: Hoshikuzu no Tabibito's episodes 1 to 6 watched, so Continue starts
    /// episode 7, and every episode of every title streams `media`, a local file or URL the
    /// app's own player can open (the test kit's own clip is a lavfi source, which the app's
    /// libmpv has no device for). Skipping the opening is off, so the skip is offered rather
    /// than taken, unless the config says otherwise. An address ending `.m3u8` is a web stream:
    /// HLS behind a referer lock, as provider CDNs have them, which goes through the engine's
    /// stream proxy with its header ([`SCENE_REFERER`]).
    Playing { media: String },
    /// [`Self::Playing`], with skip times the viewer marked on episode 7, `length_secs` long as
    /// `media` is: a recap from 0:00 to 0:08 and an opening from 0:10 to 0:40. The seek line
    /// marks them as yours and Skip recap, then Skip opening, are offered as each comes.
    SkipMarked { media: String, length_secs: u32 },
    /// [`Self::Watching`], with every episode streaming `media` as [`Self::Playing`]'s do: a TV
    /// on a fixture resumes episode 7 at once from a file it can seek, where the test kit's clip
    /// is a test pattern mpv must play through to the resume point.
    WatchingMedia { media: String },
    /// [`Self::Watching`] with a second source ahead of the fixture one, Hotaru Open Archive,
    /// which lists four titles for Hoshikuzu no Tabibito and is sure of none: two could be it
    /// and two are ruled out, one by its year and one by its format. So its page asks for the
    /// match. A search there for "Stardust Wayfarer" finds the show itself, whose episodes play,
    /// and its specials, ruled out. Every other title it has nothing for, so the fixture source
    /// answers for them as in [`Self::Watching`].
    Unsure,
    /// Home's second spotlight case: signed in, every episode of Hoshikuzu no Tabibito watched
    /// and nothing under way, so Continue is empty and the spotlight is from Up next for you.
    /// With the Home world on, that is The Ice Knight Season 2, a new season airing now of a
    /// show finished here.
    Suggested,
    /// Signed out, with one show in Continue: Natsukaze Relay's episodes 1 to 7 watched and
    /// episode 8 out, the spotlight's first case. Once episode 8 is watched or marked so, the
    /// show is caught up and the spotlight turns to the popular pick: the change from the first
    /// case to the third, with Continue at one and then at none.
    OneLeft,
    /// [`Self::Watching`] for Search's extras. Cards say what is known of their dubs: Hoshikuzu no
    /// Tabibito "Dub 8", Natsukaze Relay "Dub" with no count, and the rest nothing. A source that
    /// can search for dubs, Tsuki Dub Catalogue, lists Hoshikuzu no Tabibito in the dub with eight
    /// episodes, so a search for it asks, finds that and says "Dub 8". Surprise me has the
    /// fixtures to pick from, and the long franchise's table is long enough for ranges.
    Browsing,
    /// Signed in, with fifteen shows under way, each stopped part way into episode 1, so
    /// Continue shows twelve with View all and its count.
    ContinueFull,
    /// Signed in with nothing watched, and the one source has none of the titles: every title
    /// Home asks after is not found in your sources, and its main action is Add to Planning.
    NotFound,
    /// [`Self::Watching`] with AniList slow for everything but the queue's own titles: each of
    /// Home's discovery requests is answered after [`SLOW_ANSWER`], so the local sections show
    /// at once, the placeholders come in and the activity line after them.
    Slow,
    /// [`Self::Watching`] with AniList failing everything but the queue's own titles and their
    /// broadcasts: Continue shows, the discovery shelves are left out and one notice says so.
    Failed,
    /// [`Self::Sourceless`] with the viewer's licensed services chosen: Lantern Library, which
    /// AniList lists for Hoshikuzu no Tabibito. Its page is the licensed-service case where the
    /// title does not play here and the main action is Watch on Lantern Library, and the Licensed
    /// services page has one service ticked and the rest to choose.
    Licensed,
    /// [`Self::Watching`] with one Jellyfin server saved, Mika's Jellyfin: two libraries, the
    /// anime one read, three series and two of them matched. The Media servers page has its row,
    /// and the server is a source in the chain.
    Servers,
    /// [`Self::Watching`] with one folder added, Media: matched series, ones that need a look
    /// (Hoshikuzu Tabibito S2 1080p, 12 files, a 71% best guess), ones not matched (Hoshikage
    /// Home Movies, 8 files), and a file with no episode number waiting for Set episode
    /// (extras/Making of.mkv). The Your media page has its three lists.
    Library,
    /// [`Self::Library`] and [`Self::Servers`] together, mid-work: the folder's row says
    /// "Matching · 31 of 58 series · Waiting for AniList · 12 s", Mika's Jellyfin says "Matching ·
    /// 12 of 84 series", a second server, Studio server, has signed this device out, with Sign in
    /// again, and a third, Attic server, says "Not answering · last seen" two hours before the
    /// scene's now. Check this server's data is [`testing_certificate_check`].
    SourcesBusy,
    /// [`Self::Library`] and [`Self::Servers`] together, with Lantern Library chosen as a licensed
    /// service, for Search's Available filter. A result's line ends "In your library" for
    /// Stardust Wayfarer (the folder comes before the server), "On Mika's Jellyfin" for The
    /// Library Above the Clouds and "On Lantern Library" for Stardust Wayfarer Season 2. Plays here
    /// keeps to the first two, and On your services to the titles Lantern Library carries.
    Available,
    /// [`Self::Watching`] with three watch party rooms kept, as Together shows them: Natsukaze
    /// Relay's with two friends an hour ago, Stardust Wayfarer's with one a day ago, and
    /// Cloudcutter Airship's alone a week ago. The party lobby states are reached by joining the
    /// fake party server ([`FfiEngine::start_fake_party`]) and steering its members: Natsukaze
    /// Relay's episode 9 has not aired (Out on a day), Stardust Wayfarer's episodes play, and
    /// [`Self::Sourceless`] and [`Self::NotFound`] show the other two sources lines.
    Together,
    /// Nothing watched and offline from the start: the fake AniList hangs up on every request
    /// and the engine is told the path is offline, whatever the host says, so Home has nothing
    /// saved to show. [`FfiEngine::testing_set_offline`] does the same to any other scene once
    /// Home has loaded, for the offline line over what it last loaded.
    Offline,
    /// [`Self::Watching`], and the viewer has just opened a magnet link of their own,
    /// [`FIXTURE_OPENED_LINK`]: once the engine has started it answers with
    /// `Event::LinkOpened`, its guess Hoshikuzu no Tabibito, episode 3, for the confirm step
    /// ("Is this…?" with Change and Play without tracking). Torrent playback is off, as it is in
    /// every scene, so the step offers to turn it on in place of Play.
    LinkOpened,
    /// A first launch with the demo as the only source: Home's spotlight offers the demo, and
    /// Sources lists it with About the demo.
    DemoOnly,
    /// [`Self::DemoOnly`] after part 1 was played: the spotlight is gone, Continue holds the
    /// demo, and Home ends as a user with only the demo would.
    DemoPlayed,
    /// The fixture source beside the demo, as after adding a source with the demo still on.
    DemoAndStore,
    /// No source at all and the demo taken out, so Sources offers Add the demo.
    DemoRemoved,
    /// [`Self::Watching`] with a styled look: larger serif yellow subtitles on a plate, styled
    /// subtitles restyled.
    SubtitleStyled,
    /// A first launch with crash reports off, as they are until turned on.
    CrashOff,
    /// Crash reports on, with one saved report in the list.
    CrashSaved,
    /// Crash reports on, turned on a day ago. The app offers its system's own records from the
    /// week before; the scene has none of its own.
    CrashHeld,
    /// [`Self::Fresh`] with a plugin store, Hilltop Plugins, served on loopback and joined as its
    /// owner, for the owner screens' invite links:
    ///
    /// - Book club, a link that works five times for 30 days, used by Alex and Kim;
    /// - Sam, a link that works once for seven days, not used yet;
    /// - in the ended group, Rin, used up, and Film night, cancelled after Jo joined with it.
    ///
    /// The members are the owner, Alex, Kim, Rin and Jo.
    StoreOwner,
    /// [`Self::Fresh`] with the same store, not joined: [`FfiEngine::testing_store_invite`] is
    /// the Book club link, used once so far, for the join sheet with its name field.
    StoreInvited,
    /// [`Self::Watching`] with sync set up: this device is Kim's iPhone, the group holds the
    /// Living room TV as well, heard from two minutes ago, the folder is `Shared/Kaeri`, and one
    /// note says a watch of Hoshikuzu no Tabibito episode 3 was kept although the iPhone marked
    /// it unwatched. No folder is behind it; the engine runs no pass.
    SyncOn,
    /// [`Self::Watching`] with sync on nearby only: this device is Kim's iPhone, paired with the
    /// Living room TV (signed in to AniList), heard from five minutes ago. No cloud folder, so no
    /// recovery key.
    SyncNearby,
    /// [`Self::Watching`] syncing through Google Drive, signed in as kim@mail.example: this
    /// device is Kim's iPhone, the group holds Kim's MacBook Air, and the recovery key was made
    /// three days ago and can be shown again.
    SyncCloud,
    /// [`Self::Watching`] on a TV (start it with `FfiPlatform::AndroidTv`): this device is the
    /// Living room TV, paired with Kim's iPhone, which syncs through Dropbox and holds the
    /// sign-in the TV can use. The TV has no folder yet.
    SyncTvPhoneSignIn,
    /// [`Self::Watching`] with three profiles: Alex, who is in use and whose data this is,
    /// Kim, and Sam, a kids profile. Kim's and Sam's start empty.
    Profiles,
}

/// The magnet link [`FfiScene::LinkOpened`] opens: a made-up info hash whose name is
/// `Hoshikuzu no Tabibito - 03 [1080p].mkv`. No swarm has it.
pub const FIXTURE_OPENED_LINK: &str = "magnet:?xt=urn:btih:5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e\
&dn=Hoshikuzu%20no%20Tabibito%20-%2003%20%5B1080p%5D.mkv";

/// [`FIXTURE_OPENED_LINK`], for a Fixtures build to open again, as its Open link or file item
/// does with a link pasted.
///
/// Only in a build with the `testing` feature, which an app never turns on.
#[uniffi::export]
pub fn fixture_opened_link() -> String {
    FIXTURE_OPENED_LINK.to_owned()
}

/// The environment variable that puts Home's world in a scene: see [`FfiScene`].
pub const HOME_WORLD_VAR: &str = "KAERI_FIXTURES_HOME_WORLD";

/// Whether the environment asks for Home's world.
fn home_world_asked() -> bool {
    std::env::var(HOME_WORLD_VAR).is_ok_and(|v| v == "1")
}

/// What Check this server shows for a made-up server at home whose certificate this device
/// does not trust: the host, the address to pass back with Allow this certificate, and the
/// fingerprint in groups of four. For the apps' fixture shots of that step, which no scene can
/// reach without a server on the network. Only in a build with the `testing` feature.
#[uniffi::export]
pub fn testing_certificate_check() -> kaeri_model::servers::CertificateCheck {
    // Made up, in the shape the engine sends: SHA-256 as upper-case hex.
    let fingerprint = "3F9A61C4E07B2D58A1F6093CB7E4D2096A5C18F3B04E7D92C6A13F58E9B0274D";
    kaeri_model::servers::CertificateCheck {
        host: "jellyfin.mika.example".into(),
        address: "https://jellyfin.mika.example".into(),
        groups: fingerprint
            .as_bytes()
            .chunks(4)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect(),
        fingerprint: fingerprint.into(),
    }
}

/// How long [`FfiScene::Slow`]'s fake AniList holds each of Home's discovery answers: past the
/// four seconds after which Home's activity line shows, within the five the engine's client
/// waits for an answer.
pub const SLOW_ANSWER: std::time::Duration = std::time::Duration::from_millis(4_500);

/// What Home asks AniList for beyond its queue's titles and their broadcasts: the season's
/// lists, the genre shelf, recommendations and the season walk. [`FfiScene::Slow`] and
/// [`FfiScene::Failed`] hold these up.
const DISCOVERY: [Operation; 6] = [
    Operation::Browse,
    Operation::Search,
    Operation::Seasonal,
    Operation::Recommended,
    Operation::Relations,
    Operation::Schedule,
];

impl FfiScene {
    /// Whether the scene has the Home world's history and list entries beside its own.
    fn has_world_history(&self) -> bool {
        matches!(
            self,
            Self::Watching
                | Self::Browsing
                | Self::Servers
                | Self::Library
                | Self::SourcesBusy
                | Self::Available
                | Self::WatchingMedia { .. }
                | Self::Sourceless
                | Self::SubtitleStyled
                | Self::Suggested
                | Self::Slow
                | Self::Failed
                | Self::LinkOpened
                | Self::SyncOn
                | Self::SyncNearby
                | Self::SyncCloud
                | Self::SyncTvPhoneSignIn
                | Self::Profiles
        )
    }

    /// Whether the scene starts signed in to the fake AniList.
    fn signed_in(&self) -> bool {
        matches!(
            self,
            Self::Watching
                | Self::Browsing
                | Self::Servers
                | Self::Library
                | Self::SourcesBusy
                | Self::Available
                | Self::WatchingMedia { .. }
                | Self::Sourceless
                | Self::SubtitleStyled
                | Self::Unsure
                | Self::LinkOpened
                | Self::Suggested
                | Self::ContinueFull
                | Self::NotFound
                | Self::Slow
                | Self::Failed
                | Self::SyncOn
                | Self::SyncNearby
                | Self::SyncCloud
                | Self::SyncTvPhoneSignIn
                | Self::Profiles
        )
    }

    /// How sync stands in the scene, for those that have it on.
    fn sync(&self) -> Option<kaeri_engine::syncing::FixtureSync> {
        use kaeri_engine::syncing::FixtureSync;
        Some(match self {
            Self::SyncOn => FixtureSync::Folder,
            Self::SyncNearby => FixtureSync::Nearby,
            Self::SyncCloud => FixtureSync::Cloud,
            Self::SyncTvPhoneSignIn => FixtureSync::TvPhoneSignIn,
            _ => return None,
        })
    }
}

#[uniffi::export]
impl FfiEngine {
    /// [`FfiEngine::start_testing`] on `scene`, with the fake AniList answering at the time
    /// the engine starts rather than the fixtures' own, so that what aired and what airs next
    /// is counted from now, as the engine counts it. `start` is taken as `start_testing` takes
    /// it: tokens are files under `dirs.data` whatever its `secrets` says.
    ///
    /// Only in a build with the `testing` feature, which an app never turns on.
    #[uniffi::constructor(default(start))]
    pub fn start_scene(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        scene: FfiScene,
        start: FfiStart,
    ) -> Result<Arc<Self>, FfiError> {
        let world = home_world_asked();
        Self::start_scene_from(dirs, host, scene, &start, Clock::default(), world)
    }
}

impl FfiEngine {
    /// [`FfiEngine::start_scene`] on `clock` rather than the system's own: the scene seeded, and
    /// the fake AniList answering, as of the moment it reads, and today and the week read in its
    /// zone. For a Rust test that reads days in a zone of its own (`Clock::in_zone`), so that its
    /// today and its week come out the same on any date, at any hour and in any zone it runs in;
    /// `testing_advance_clock` moves the clock on. Not in the bindings.
    pub fn start_scene_on(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        scene: FfiScene,
        clock: Clock,
    ) -> Result<Arc<Self>, FfiError> {
        let world = home_world_asked();
        Self::start_scene_from(dirs, host, scene, &FfiStart::default(), clock, world)
    }

    /// [`FfiEngine::start_scene_on`] with Home's world in the scene, whatever the environment
    /// says: see [`FfiScene`]. For Home's own scene tests. Not in the bindings.
    pub fn start_home_scene_on(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        scene: FfiScene,
        clock: Clock,
    ) -> Result<Arc<Self>, FfiError> {
        Self::start_scene_from(dirs, host, scene, &FfiStart::default(), clock, true)
    }

    fn start_scene_from(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        scene: FfiScene,
        start: &FfiStart,
        clock: Clock,
        home_world: bool,
    ) -> Result<Arc<Self>, FfiError> {
        if scene.signed_in() {
            sign_in_config(&dirs)?;
        }
        if matches!(scene, FfiScene::Playing { .. } | FfiScene::SkipMarked { .. }) {
            offer_skips(&dirs)?;
        }
        let now = clock.now();
        scene_config(&dirs, &scene, now)?;
        if scene == FfiScene::Profiles {
            seed_profiles(&dirs, now)?;
        }
        let opened = Opened::open_as(&dirs, start.profile.as_deref())?;
        seed_crashes(&opened.paths.data_dir, &scene, now);
        let world = home_world.then(|| HomeWorld::at(HomeClock::in_zone(now, &clock.zone())));
        seed_scene(&opened.store, &scene, now);
        if let Some(sync) = scene.sync() {
            let tokens =
                TokenStore::with_backend(opened.paths.data_dir.join("tokens"), Backend::File);
            for (id, value) in kaeri_engine::syncing::seed_fixture_as(&opened.store, now, sync)
            {
                tokens.set(id, &value).map_err(|e| {
                    FfiError::engine(Msg::StartFailed { reason: e.to_string() })
                })?;
            }
        }
        if let Some(world) = world.as_ref().filter(|_| scene.has_world_history()) {
            world.seed_history(&opened.store, now);
            library_statuses(&opened.store, world, now);
        }
        if scene.signed_in() {
            // Where the testing engine keeps tokens: files under the data directory.
            TokenStore::with_backend(opened.paths.data_dir.join("tokens"), Backend::File)
                .set("anilist", "fixture-anilist-token")
                .map_err(|e| FfiError::engine(Msg::StartFailed { reason: e.to_string() }))?;
        }
        let host = Adapter::new(host, SecretsMode::Host, &opened.config, &opened.paths);
        let home = RemoteHome::of(&host, &opened.paths);
        let runtime = runtime()?;
        let started = runtime.block_on(async {
            let server = FakeAniList::start().await;
            server.set_now(now);
            // The long franchise beside the six, for a season picker of twelve: a page opens on
            // it by id, as no list has it.
            server.add_titles(fixtures::long_franchise());
            // And the short one, five seasons with a film, an OVA and a web series beside them,
            // for a season row that fits across the page with Related beside it.
            server.add_titles(fixtures::franchise());
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
                if scene.has_world_history() {
                    server.add_entries(world.entries.iter().cloned());
                }
            }
            // Eight more titles in the browse, for a grid of fourteen, only when asked for.
            if std::env::var("KAERI_FIXTURES_CATALOGUE").is_ok_and(|v| v == "1") {
                server.add_catalogue(fixtures::catalogue_titles());
            }
            match scene {
                FfiScene::Slow => {
                    for operation in DISCOVERY {
                        server.answer_slowly(operation, SLOW_ANSWER);
                    }
                }
                FfiScene::Failed => {
                    for operation in DISCOVERY {
                        server.fail_operation(operation, Failure::Status(500));
                    }
                }
                _ => {}
            }
            let provided = world.as_ref().map(HomeWorld::provided).unwrap_or_default();
            let sources = Some(scene_sources(&scene, &server.art_base(), &provided));
            let store_invite = match scene {
                FfiScene::StoreOwner | FfiScene::StoreInvited => Some(
                    stores::serve(&opened, scene == FfiScene::StoreOwner)
                        .await
                        .map_err(|e| FfiError::engine(Msg::StartFailed { reason: e }))?,
                ),
                _ => None,
            };
            let testing = testing::Testing::new(server, clock);
            if let Some(link) = store_invite {
                let _ = testing.store_invite.set(link);
            }
            let options = testing::options(opened, &testing, start, sources)?;
            Ok::<_, FfiError>((Self::launch(options, host).await?, testing))
        });
        let engine = match started {
            Ok((started, testing)) => {
                Self::hold(runtime, Ok(started), Some(Box::new(testing)), home)
            }
            Err(e) => Self::hold(runtime, Err(e), None, home),
        }?;
        if scene == FfiScene::Offline {
            engine.testing_set_offline(true);
        }
        if scene == FfiScene::LinkOpened {
            engine.send(kaeri_model::command::Command::Opened(
                kaeri_model::opened::OpenedCommand::OpenLink {
                    input: FIXTURE_OPENED_LINK.to_owned(),
                },
            ));
        }
        Ok(engine)
    }
}

/// Downloads for the Downloads screen to show, seeded into the database at `dirs` before a scene
/// starts on it: episode 6 of the finished fixture downloaded, its file `clip` (a real file on
/// the device, which plays offline), episode 7 of it 40% in, and episode 3 of the airing one
/// waiting. The engine's downloads are torrents, which no fixture has, so this is how a build
/// shows them without one.
///
/// Only in a build with the `testing` feature, which an app never turns on.
#[uniffi::export]
pub fn seed_fixture_downloads(dirs: FfiDirs, clip: String) -> Result<(), FfiError> {
    let opened = Opened::open(&dirs)?;
    let size = std::fs::metadata(&clip)
        .map_err(|e| FfiError::dirs(Msg::verbatim(format!("{clip}: {e}"))))?
        .len();
    seed_downloads(&opened.store, &clip, size)
        .map_err(|e| FfiError::store(Msg::verbatim(format!("seeding the downloads: {e}"))))
}

/// The three rows [`seed_fixture_downloads`] seeds, the finished one at `clip`, `size` bytes.
fn seed_downloads(store: &Store, clip: &str, size: u64) -> kaeri_store::Result<()> {
    use kaeri_store::DownloadState;
    let done = seed::download(store, FINISHED, "6", DownloadState::Done);
    store.update_download_progress(done.id, size, size, Some(clip))?;
    store.finish_download(done.id, Some(clip))?;
    seed::download(store, FINISHED, "7", DownloadState::Active);
    seed::download(store, AIRING, "3", DownloadState::Queued);
    Ok(())
}

/// The scene's sources: the test kit's fixture provider, its stills under `art`, with the long
/// franchise's titles beside the six, which the scene's fake AniList has too, and `provided`,
/// Home's world's titles with the episodes out of each, when the scene has it. Every episode
/// streams the test kit's episode-long clip, or `media` in a scene that plays. None at all for
/// [`FfiScene::Sourceless`].
fn scene_sources(scene: &FfiScene, art: &str, provided: &[(Media, u32)]) -> ProviderRegistry {
    use kaeri_providers::demo::DemoProvider;
    let demo = DemoProvider::shared();
    match scene {
        // The demo alone: what a first launch with nothing else has.
        FfiScene::DemoOnly
        | FfiScene::DemoPlayed
        | FfiScene::CrashOff
        | FfiScene::CrashSaved
        | FfiScene::CrashHeld => return ProviderRegistry::new(vec![demo]),
        FfiScene::DemoRemoved => return ProviderRegistry::new(Vec::new()),
        // The fixture source, and the demo last.
        FfiScene::Fresh | FfiScene::DemoAndStore => {
            let registry = fixture_sources(scene, art, provided);
            registry.extend(vec![demo]);
            return registry;
        }
        _ => {}
    }
    fixture_sources(scene, art, provided)
}

/// [`scene_sources`] without the demo.
fn fixture_sources(scene: &FfiScene, art: &str, provided: &[(Media, u32)]) -> ProviderRegistry {
    if matches!(scene, FfiScene::Sourceless | FfiScene::Licensed) {
        return ProviderRegistry::new(Vec::new());
    }
    if *scene == FfiScene::NotFound {
        // A source that answers every search with nothing: a miss for every title.
        return ProviderRegistry::new(vec![
            MockProvider::new(sources::PROVIDER_ID).with_hits(Vec::new()).arc(),
        ]);
    }
    let franchise =
        || sources::with_long_franchise_and(sources::provider_with_art(art), provided);
    let (FfiScene::Playing { media }
    | FfiScene::SkipMarked { media, .. }
    | FfiScene::WatchingMedia { media }) = scene
    else {
        let clip = sources::episode_clip();
        let mut provider = franchise();
        for id in fixtures::ALL {
            provider = provider.with_streams_for(sources::key(id), vec![clip.clone()]);
        }
        let others = fixtures::long_franchise_media()
            .into_iter()
            .chain(provided.iter().map(|p| p.0.clone()));
        for media in others {
            provider = provider.with_streams_for(sources::key(media.id), vec![clip.clone()]);
        }
        if *scene == FfiScene::Unsure {
            return ProviderRegistry::new(vec![
                unsure_source(art, &clip).arc(),
                provider.arc(),
            ]);
        }
        return ProviderRegistry::new(vec![provider.arc()]);
    };
    let web = media.starts_with("http")
        && media.split('?').next().is_some_and(|path| path.ends_with(".m3u8"));
    let stream = Stream {
        provider_id: sources::PROVIDER_ID.into(),
        headers: if web { vec![("Referer".into(), SCENE_REFERER.into())] } else { Vec::new() },
        ..Stream::new(media, if web { StreamKind::Hls } else { StreamKind::Mp4 })
    };
    let mut provider = franchise();
    for id in fixtures::ALL {
        provider = provider.with_streams_for(sources::key(id), vec![stream.clone()]);
    }
    ProviderRegistry::new(vec![provider.arc()])
}

/// The source [`FfiScene::Unsure`] puts first: an invented archive that lists four titles for
/// Hoshikuzu no Tabibito, none close enough to choose, and the show itself for a search by its
/// English title. The show's episodes are the fixture's, streaming `clip`.
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

/// The id of [`FfiScene::Unsure`]'s first source.
pub const UNSURE_SOURCE: &str = "hotaru-open-archive";

/// The referer a Playing scene's web stream is locked to: an invented source's.
pub const SCENE_REFERER: &str = "https://lantern-archive.example/";

/// A config with AniList enabled, as a signed-in user's has it: the fixtures' client, and tokens
/// in files. Left alone when there is a config already.
fn sign_in_config(dirs: &FfiDirs) -> Result<(), FfiError> {
    let file = std::path::Path::new(&dirs.config).join("config.toml");
    if file.exists() {
        return Ok(());
    }
    let written = std::fs::create_dir_all(&dirs.config).and_then(|()| {
        std::fs::write(
            &file,
            "[trackers]\nenabled = [\"anilist\"]\ntoken_storage = \"file\"\n\n\
             [trackers.anilist]\nclient_id = \"fixture\"\nclient_secret = \"fixture\"\n",
        )
    });
    written.map_err(|e| FfiError::config(Msg::verbatim(format!("{}: {e}", file.display()))))
}

/// The config a scene starts from, written where there is none yet. A first launch has none, and
/// reads as having the demo on; the scenes that say otherwise write it.
fn scene_config(dirs: &FfiDirs, scene: &FfiScene, now: i64) -> Result<(), FfiError> {
    let file = std::path::Path::new(&dirs.config).join("config.toml");
    let text = match scene {
        FfiScene::DemoAndStore => "[sources]\ndemo = true\n".to_owned(),
        FfiScene::DemoRemoved => "[sources]\ndemo = false\n".to_owned(),
        FfiScene::SubtitleStyled => "[subtitles]\nsize = 130\nfont = \"serif\"\n\
             colour = \"yellow\"\nedge = \"outline\"\nbackground = \"each-line\"\n\
             opacity = 75\nraise = 8\nstyled = \"restyle\"\n"
            .to_owned(),
        FfiScene::CrashSaved => crash_config(now),
        FfiScene::CrashHeld => crash_config(now - 24 * 3_600),
        _ => return Ok(()),
    };
    // A signed-in scene's file is there already, with tables of its own: add this scene's after them.
    let text = match std::fs::read_to_string(&file) {
        Ok(existing) if existing.contains(text.lines().next().unwrap_or("[")) => return Ok(()),
        Ok(existing) => format!("{existing}\n{text}"),
        Err(_) => text,
    };
    std::fs::create_dir_all(&dirs.config).and_then(|()| std::fs::write(&file, text)).map_err(
        |e| FfiError::dirs(Msg::verbatim(format!("{} cannot be written: {e}", file.display()))),
    )
}

/// Saving on since `since` (Unix seconds), with the demo on.
fn crash_config(since: i64) -> String {
    format!(
        "[sources]\ndemo = true\n\n[crash_reports]\nsave = true\nask = true\nenabled_at = {}\n",
        since * 1_000
    )
}

/// The one saved report of [`FfiScene::CrashSaved`], in the crash store of the scene's data.
fn seed_crashes(data_dir: &std::path::Path, scene: &FfiScene, now: i64) {
    use kaeri_model::crash::{CrashFrame, CrashPlace, CrashRecord};
    if *scene != FfiScene::CrashSaved {
        return;
    }
    let record = CrashRecord {
        id: "scene-1".into(),
        at_ms: (now - 2 * 3_600) * 1_000,
        uptime_secs: 754,
        platform: "Fixtures".into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        place: CrashPlace::Player,
        detail: "playing".into(),
        reason: "index out of bounds".into(),
        frames: vec![CrashFrame {
            module: "kaeri".into(),
            symbol: Some("kaeri_engine::play::start".into()),
            offset: 0,
        }],
        demo_on: true,
        ..CrashRecord::default()
    };
    kaeri_engine::crash::CrashStore::in_data(data_dir).save(&record, true, now * 1_000);
}

/// A config that offers skips rather than taking them, written where there is none yet.
fn offer_skips(dirs: &FfiDirs) -> Result<(), FfiError> {
    let file = std::path::Path::new(&dirs.config).join("config.toml");
    if file.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&dirs.config)
        .and_then(|()| std::fs::write(&file, "[playback]\nskip_opening = false\n"))
        .map_err(|e| {
            FfiError::dirs(Msg::verbatim(format!("{} cannot be written: {e}", file.display())))
        })
}

/// The fake library's statuses as the last sync left them: the fixture library's and the Home
/// world's. A scene starts with them, so what Home leaves out or suggests from the list is
/// there from the first load rather than after the engine's first sync.
fn library_statuses(store: &Store, world: &HomeWorld, now: i64) {
    let mut statuses = fixtures::library_statuses();
    statuses.extend(world.library_statuses());
    store
        .replace_library_statuses("anilist", &statuses, now)
        .expect("seeding the list's statuses");
}

/// The history `scene` describes, the newest watch an hour before `now`.
fn seed_scene(store: &Store, scene: &FfiScene, now: i64) {
    if matches!(scene, FfiScene::Library | FfiScene::SourcesBusy | FfiScene::Available) {
        kaeri_engine::local::seed_fixture(store);
    }
    if matches!(scene, FfiScene::Servers | FfiScene::SourcesBusy | FfiScene::Available) {
        kaeri_engine::servers::seed_fixture(store);
    }
    if *scene == FfiScene::SourcesBusy {
        kaeri_engine::local::seed_fixture_matching(store);
        kaeri_engine::servers::seed_fixture_busy(store, now);
    }
    if matches!(scene, FfiScene::Licensed | FfiScene::Available) {
        kaeri_engine::licensed::set_services(vec!["Lantern Library".into()], store);
    }
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;
    match scene {
        FfiScene::Fresh
        | FfiScene::NotFound
        | FfiScene::Offline
        | FfiScene::DemoOnly
        | FfiScene::DemoAndStore
        | FfiScene::DemoRemoved
        | FfiScene::CrashOff
        | FfiScene::CrashSaved
        | FfiScene::CrashHeld
        | FfiScene::StoreOwner
        | FfiScene::StoreInvited => {}
        FfiScene::DemoPlayed => {
            seed::watched(store, kaeri_core::demo::DEMO_ANILIST_ID, 1, now - HOUR);
        }
        FfiScene::Playing { .. } => seed::watched(store, FINISHED, 6, now - HOUR),
        FfiScene::SkipMarked { length_secs, .. } => {
            seed::watched(store, FINISHED, 6, now - HOUR);
            seed::skip_marks(store, FINISHED, "7", f64::from(*length_secs), now - HOUR);
        }
        FfiScene::Suggested => {
            let all = fixtures::media(FINISHED).episodes.unwrap_or(1);
            seed::watched(store, FINISHED, all, now - HOUR);
        }
        FfiScene::OneLeft => seed::watched(store, AIRING_NO_MAL, 7, now - DAY),
        FfiScene::ContinueFull => {
            let long = fixtures::long_franchise_media();
            let shows = fixtures::ALL.into_iter().chain(long.iter().map(|m| m.id)).take(15);
            // Oldest first, an hour apart, each stopped five minutes into episode 1.
            for (n, id) in shows.collect::<Vec<_>>().into_iter().rev().enumerate() {
                let at = now - (15 - n as i64) * HOUR;
                seed::stopped_at(store, id, "1", 5.0 * 60.0, at);
            }
        }
        FfiScene::Watching
        | FfiScene::Servers
        | FfiScene::Library
        | FfiScene::SourcesBusy
        | FfiScene::Available
        | FfiScene::Together
        | FfiScene::WatchingMedia { .. }
        | FfiScene::Sourceless
        | FfiScene::Licensed
        | FfiScene::SubtitleStyled
        | FfiScene::Unsure
        | FfiScene::Slow
        | FfiScene::Failed
        | FfiScene::SyncOn
        | FfiScene::SyncNearby
        | FfiScene::SyncCloud
        | FfiScene::SyncTvPhoneSignIn
        | FfiScene::Profiles
        | FfiScene::Browsing
        | FfiScene::LinkOpened => {
            // Oldest first, since each watch moves its title to the top.
            seed::watched(store, AIRING_NO_MAL, 7, now - 3 * DAY);
            seed::watched(store, AIRING, 3, now - 2 * DAY);
            seed::watched(store, ONA, 1, now - DAY - HOUR);
            // Switched to the dub after episode 1, so its row is marked "Dub".
            seed::stopped_in(store, ONA, "2", 13.0 * 60.0 + 30.0, Translation::Dub, now - DAY);
            seed::watched(store, FINISHED, 6, now - 2 * HOUR);
            seed::stopped_at(store, FINISHED, "7", 10.0 * 60.0 + 6.0, now - HOUR);
            if *scene == FfiScene::Together {
                seed_rooms(store, now);
            }
            if *scene == FfiScene::Browsing {
                seed_dubs(store, now);
            }
        }
    }
}

/// The profiles [`FfiScene::Profiles`] keeps, Alex last used.
fn seed_profiles(dirs: &FfiDirs, now: i64) -> Result<(), FfiError> {
    let data = std::path::Path::new(&dirs.data);
    let failed = |e: kaeri_store::StoreError| {
        FfiError::store(Msg::StartFailedDatabase {
            path: dirs.data.clone(),
            reason: e.to_string(),
        })
    };
    kaeri_store::profile::add(data, "Kim", false, "Alex", now - 20).map_err(failed)?;
    kaeri_store::profile::add(data, "Sam", true, "Alex", now - 10).map_err(failed)?;
    kaeri_store::profile::touch(data, "p1", now).map_err(failed)
}

/// The dubs [`FfiScene::Browsing`]'s cards say, as title pages once found them.
fn seed_dubs(store: &Store, now: i64) {
    let row = |through: Option<u32>| kaeri_store::DubRow {
        has_dub: true,
        through,
        complete: false,
        aired: None,
        said_at: now,
        until: now + 24 * 3_600,
    };
    store.put_dub(FINISHED, "franchise", &row(Some(8))).expect("seeding a dub");
    store.put_dub(AIRING_NO_MAL, "franchise", &row(None)).expect("seeding a dub");
}

/// The three rooms [`FfiScene::Together`] keeps.
fn seed_rooms(store: &Store, now: i64) {
    use kaeri_store::PartyRoom;
    const HOUR: i64 = 3_600;
    let rooms = [
        (AIRING_NO_MAL, "kite-7qm4-xr9d", "8", &["Mika", "Ren"][..], now - HOUR),
        (FINISHED, "ember-2hv8-kd4p", "3", &["Mika"][..], now - 24 * HOUR),
        (ONA, "ember-9wz3-tn6c", "1", &[][..], now - 7 * 24 * HOUR),
    ];
    for (id, room, episode, names, last_at) in rooms {
        let media = fixtures::media(id);
        store
            .remember_party_room(&PartyRoom {
                server: "syncplay.pl:8995".into(),
                room: room.into(),
                anilist_id: id,
                title: media.title.display().to_owned(),
                episode: episode.into(),
                names: names.iter().map(|n| (*n).to_owned()).collect(),
                cover_url: None,
                last_at,
            })
            .expect("seeding a party room");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Home's world at the tests' moment, read in UTC.
    fn world() -> HomeWorld {
        HomeWorld::at(HomeClock::in_zone(1_800_000_000, &jiff::tz::TimeZone::UTC))
    }

    #[test]
    fn the_worlds_history_leaves_continue_as_it_was_and_the_list_in_the_store() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        seed_scene(&store, &FfiScene::Watching, 1_800_000_000);
        let world = world();
        world.seed_history(&store, 1_800_000_000);
        library_statuses(&store, &world, 1_800_000_000);
        let continuing = store.continue_list(15).expect("the rail");
        let ids: Vec<_> = continuing.iter().map(|p| p.anilist_id).collect();
        assert_eq!(ids, [FINISHED, ONA, AIRING, AIRING_NO_MAL]);
        assert_eq!(store.watched_titles(50).expect("the history").len(), 4 + 7);
        let completed = store.library_with_status("COMPLETED").expect("the list");
        assert_eq!(completed.len(), 1 + 4, "{completed:?}");
        assert_eq!(store.library_with_status("PLANNING").expect("the list").len(), 1 + 2);
    }

    #[test]
    fn watching_leaves_four_titles_to_continue_with_the_newest_first() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        seed_scene(&store, &FfiScene::Watching, 1_800_000_000);

        let continuing = store.continue_list(15).expect("the rail");
        let ids: Vec<_> = continuing.iter().map(|p| p.anilist_id).collect();
        assert_eq!(ids, [FINISHED, ONA, AIRING, AIRING_NO_MAL]);
        assert_eq!(store.resume_position(FINISHED, "7").expect("a read"), Some(606.0));
        assert_eq!(store.resume_position(ONA, "2").expect("a read"), Some(810.0));
        // One watched dubbed, the rest subbed.
        let dubbed =
            continuing.iter().map(|p| store.last_translation(p.anilist_id).expect("a read"));
        let sub = Some(Translation::Sub);
        assert_eq!(dubbed.collect::<Vec<_>>(), [sub, Some(Translation::Dub), sub, sub]);
    }

    #[test]
    fn sourceless_is_watching_with_no_source_at_all() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        seed_scene(&store, &FfiScene::Sourceless, 1_800_000_000);
        assert_eq!(store.continue_list(15).expect("the rail").len(), 4);
        let none = scene_sources(
            &FfiScene::Sourceless,
            "http://127.0.0.1:9/art/",
            &world().provided(),
        );
        assert!(none.is_empty());
    }

    #[test]
    fn library_has_three_groups_in_three_states() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        seed_scene(&store, &FfiScene::Library, 1_800_000_000);
        let groups = kaeri_engine::local::saved(&store);
        assert_eq!(groups.len(), 5);
        let waiting: Vec<_> = groups
            .iter()
            .flat_map(|g| g.files.iter().filter(|f| g.waits_for_episode(f)))
            .map(|f| f.path.as_str())
            .collect();
        assert_eq!(waiting, ["/Volumes/Media/Stardust Wayfarer/extras/Making of.mkv"]);
    }

    #[test]
    fn sources_busy_adds_a_signed_out_and_a_silent_server_to_the_library_and_servers() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        seed_scene(&store, &FfiScene::SourcesBusy, 1_800_000_000);
        let saved = kaeri_engine::servers::saved(&store);
        assert_eq!(saved.len(), 3);
        assert!(saved[1].signed_out);
        assert!(saved[2].not_answering && !saved[2].signed_out);
        assert_eq!(saved[2].last_seen, Some(1_800_000_000 - 2 * 3_600));
        assert_eq!(kaeri_engine::local::saved(&store).len(), 5);
    }

    #[test]
    fn available_has_the_library_a_server_and_a_service() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        seed_scene(&store, &FfiScene::Available, 1_800_000_000);
        assert_eq!(kaeri_engine::local::saved(&store).len(), 5);
        assert_eq!(kaeri_engine::servers::saved(&store).len(), 1);
        assert_eq!(kaeri_engine::licensed::settings(&store).services, ["Lantern Library"]);
    }

    #[test]
    fn the_certificate_check_is_sixteen_groups_of_four() {
        let check = testing_certificate_check();
        assert_eq!(check.groups.len(), 16);
        assert_eq!(check.groups.concat(), check.fingerprint);
    }

    #[test]
    fn servers_has_one_saved_server_with_two_matches() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        seed_scene(&store, &FfiScene::Servers, 1_800_000_000);
        let saved = kaeri_engine::servers::saved(&store);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].matches().len(), 2);
    }

    #[test]
    fn licensed_is_sourceless_with_a_service_chosen() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        seed_scene(&store, &FfiScene::Licensed, 1_800_000_000);
        assert_eq!(store.continue_list(15).expect("the rail").len(), 4);
        assert_eq!(kaeri_engine::licensed::settings(&store).services, ["Lantern Library"]);
        let none =
            scene_sources(&FfiScene::Licensed, "http://127.0.0.1:9/art/", &world().provided());
        assert!(none.is_empty());
    }

    #[test]
    fn unsure_asks_about_the_finished_title_and_finds_it_by_its_english_one() {
        use kaeri_core::Provider;
        use kaeri_meta::title::{MatchTarget, Rejection, rank};
        let target = MatchTarget {
            titles: vec![
                "Hoshikuzu no Tabibito".into(),
                "Stardust Wayfarer".into(),
                "Hoshitabi".into(),
            ],
            episode_count: Some(12),
            year: Some(2024),
            format: Some(MediaFormat::Tv),
            hints: Default::default(),
        };
        let source = unsure_source("http://127.0.0.1:9/art/", &sources::episode_clip());
        let runtime =
            tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let search = |query: &str| {
            runtime
                .block_on(source.search(query, Translation::Sub, &Default::default()))
                .unwrap()
        };

        let ladder = search("Hoshikuzu no Tabibito");
        let ranked = rank(&target, &ladder);
        let open: Vec<_> = ranked.iter().filter(|s| s.rejected.is_none()).collect();
        assert_eq!(open.len(), 2, "{ranked:?}");
        assert!(open.iter().all(|s| !s.is_confident()), "{ranked:?}");
        let mut out: Vec<_> = ranked.iter().filter_map(|s| s.rejected).collect();
        out.sort_by_key(|r| format!("{r:?}"));
        assert_eq!(out, [Rejection::Format, Rejection::Year]);

        let asked = search("  stardust WAYFARER ");
        let ranked = rank(&target, &asked);
        assert!(ranked[0].is_confident(), "{ranked:?}");
        assert_eq!(ranked[0].hit.title, "Stardust Wayfarer");
        assert!(search("Kumo no Ue no Toshokan").is_empty());

        let registry =
            scene_sources(&FfiScene::Unsure, "http://127.0.0.1:9/art/", &world().provided());
        assert_eq!(registry.ids(), [UNSURE_SOURCE, sources::PROVIDER_ID]);
    }

    #[test]
    fn browsing_says_the_dubs_it_was_seeded_with_on_the_cards() {
        let store = Store::open_in_memory().expect("a database");
        seed_scene(&store, &FfiScene::Browsing, 1_800_000_000);
        let line = |id| {
            kaeri_engine::data::entry_from(&fixtures::media(id), Some(&store))
                .count_line
                .map(|line| line.long)
                .unwrap_or_default()
        };
        assert!(line(FINISHED).ends_with("Dub 8"), "{}", line(FINISHED));
        assert!(line(AIRING_NO_MAL).ends_with("Dub"), "{}", line(AIRING_NO_MAL));
        assert!(!line(ONA).contains("Dub"));
    }

    #[test]
    fn fresh_leaves_the_database_empty() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        seed_scene(&store, &FfiScene::Fresh, 1_800_000_000);
        assert!(store.continue_list(15).expect("the rail").is_empty());
    }

    #[test]
    fn playing_leaves_episode_seven_to_start_and_streams_the_media_for_every_episode() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        let scene = FfiScene::Playing { media: "/tmp/episode.mkv".into() };
        seed_scene(&store, &scene, 1_800_000_000);
        let continuing = store.continue_list(15).expect("the rail");
        assert_eq!(continuing.iter().map(|p| p.anilist_id).collect::<Vec<_>>(), [FINISHED]);
        assert_eq!(store.resume_position(FINISHED, "7").expect("a read"), None);
        let playing = scene_sources(&scene, "http://127.0.0.1:9/art/", &world().provided());
        assert_eq!(playing.ids(), [sources::PROVIDER_ID]);
        // The same, with the viewer's own skip times on episode 7.
        let marked = FfiScene::SkipMarked { media: "/tmp/episode.mkv".into(), length_secs: 90 };
        let store = Store::open(dir.path().join("marked.db")).expect("a database");
        seed_scene(&store, &marked, 1_800_000_000);
        let marks = store.skip_marks(FINISHED, "7").expect("a read");
        assert_eq!(
            marks
                .iter()
                .map(|m| (m.kind.as_str(), m.start_ms, m.length_ms))
                .collect::<Vec<_>>(),
            [("op", 10_000, 90_000), ("recap", 0, 90_000)]
        );
        // Watching on a file of its own: Watching's history, and the file for every episode.
        let file = FfiScene::WatchingMedia { media: "/tmp/episode.mp4".into() };
        let store = Store::open(dir.path().join("watching.db")).expect("a database");
        seed_scene(&store, &file, 1_800_000_000);
        assert_eq!(store.resume_position(FINISHED, "7").expect("a read"), Some(606.0));
        assert_eq!(
            scene_sources(&file, "http://127.0.0.1:9/art/", &world().provided()).ids(),
            [sources::PROVIDER_ID]
        );
        // Every scene has the fixture provider, with the long franchise beside the six.
        let watching =
            scene_sources(&FfiScene::Watching, "http://127.0.0.1:9/art/", &world().provided());
        assert_eq!(watching.ids(), [sources::PROVIDER_ID]);
    }

    #[test]
    fn the_fixture_downloads_are_one_finished_at_the_clip_one_running_and_one_waiting() {
        use kaeri_store::DownloadState;
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("kaeri.db")).expect("a database");
        seed_downloads(&store, "/sdcard/clip.mkv", 2_048).expect("seeded");
        let rows = store.downloads().expect("the queue");
        let states: Vec<_> = rows.iter().map(|row| (row.episode.as_str(), row.state)).collect();
        assert!(states.contains(&("6", DownloadState::Done)), "{states:?}");
        assert!(states.contains(&("7", DownloadState::Active)), "{states:?}");
        assert!(states.contains(&("3", DownloadState::Queued)), "{states:?}");
        let done = rows.iter().find(|row| row.episode == "6").expect("the finished one");
        assert_eq!(done.path.as_deref(), Some("/sdcard/clip.mkv"));
        assert_eq!((done.downloaded, done.total), (2_048, 2_048));
    }

    #[test]
    fn playing_offers_skips_in_a_config_of_its_own_and_keeps_one_already_there() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let at = |name: &str| dir.path().join(name).display().to_string();
        let dirs = FfiDirs { config: at("config"), data: at("data"), cache: at("cache") };
        offer_skips(&dirs).expect("written");
        let written = std::fs::read_to_string(dir.path().join("config/config.toml")).unwrap();
        assert!(written.contains("skip_opening = false"), "{written}");
        std::fs::write(dir.path().join("config/config.toml"), "# mine\n").unwrap();
        offer_skips(&dirs).expect("left alone");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config/config.toml")).unwrap(),
            "# mine\n"
        );
    }
}

/// The plugin store scenes' store: Hilltop Plugins on loopback, with its invite links.
mod stores {
    use std::sync::Arc;

    use kaeri_engine::plugin_stores::Stores;
    use kaeri_model::stores::{InviteLasts, InviteUses};
    use kaeri_plugin_store::{
        Invite, StoreClient,
        protocol::{self, InviteRequest, Role},
        server::{self, Server, Storage},
    };
    use kaeri_track::secret::{Backend, TokenStore};

    use crate::engine::Opened;

    const NAME: &str = "Hilltop Plugins";

    /// Serve the store from under the scene's data directory, its invite links made and used as
    /// [`FfiScene::StoreOwner`](super::FfiScene::StoreOwner) lists them, and join it as its
    /// owner when `joined`. Answers the Book club link. The server runs on the engine's runtime
    /// for as long as the engine does.
    pub(super) async fn serve(opened: &Opened, joined: bool) -> Result<String, String> {
        let (listener, addr) = server::bind("127.0.0.1:0".parse().map_err(|e| format!("{e}"))?)
            .await
            .map_err(|e| e.to_string())?;
        let base = format!("http://{addr}");
        // A scene starts afresh each time, on a new port: a store or a record of joining one
        // left by the last run would point at an address nothing answers.
        let dir = opened.paths.data_dir.join("fixture-store");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(kaeri_engine::plugin_stores::state_path(&opened.paths));
        let store = server::open(&Storage::Dir(dir), Some(&base), Some(NAME)).await?;
        let owner_invite = store
            .fresh_owner_invite(&base, server::now())
            .await
            .map_err(|e| e.to_string())?
            .ok_or("the scene's store already has an owner")?;
        let server = Arc::new(Server::new(store, Some(base.clone()), None, None, false));
        tokio::spawn(server::serve(listener, server, None, std::future::pending()));

        // The owner: this device when the scene joins, someone else's when it does not.
        let client = StoreClient::offline(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        let owner = if joined {
            let tokens =
                TokenStore::with_backend(opened.paths.data_dir.join("tokens"), Backend::File);
            let stores =
                Stores::with_offline_tokens(&opened.config, &opened.paths, tokens.clone())
                    .map_err(|e| e.english())?;
            let joined =
                stores.join(&owner_invite.link(), None).await.map_err(|e| e.english())?;
            tokens.get(&joined.token_key()).map_err(|e| e.to_string())?
        } else {
            client.redeem(&owner_invite, "Computer").await.map_err(|e| e.to_string())?.token
        };

        let make = |name: &'static str, uses: InviteUses, lasts: InviteLasts| {
            let request = InviteRequest::Member {
                name: name.into(),
                role: Role::Member,
                uses: match uses {
                    InviteUses::Once => protocol::InviteUses::Once,
                    InviteUses::Five => protocol::InviteUses::Five,
                    InviteUses::Any => protocol::InviteUses::Any,
                },
                lasts: match lasts {
                    InviteLasts::Day => protocol::InviteLasts::Day,
                    InviteLasts::Week => protocol::InviteLasts::Week,
                    InviteLasts::Month => protocol::InviteLasts::Month,
                    InviteLasts::Forever => protocol::InviteLasts::Forever,
                },
            };
            let (client, base, owner) = (client.clone(), base.clone(), owner.clone());
            async move {
                let made = client
                    .create_invite(&base, &owner, &request)
                    .await
                    .map_err(|e| e.to_string())?;
                let link = Invite::parse(&made.link).map_err(|e| e.to_string())?;
                Ok::<_, String>((made.id, link))
            }
        };
        let join = |invite: Invite, name: Option<&'static str>| {
            let client = client.clone();
            async move {
                client
                    .redeem_as(&invite, "Phone", name)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        };

        let (_, book_club) = make("Book club", InviteUses::Five, InviteLasts::Month).await?;
        join(book_club.clone(), Some("Alex")).await?;
        if joined {
            join(book_club.clone(), Some("Kim")).await?;
            make("Sam", InviteUses::Once, InviteLasts::Week).await?;
            let (_, rin) = make("Rin", InviteUses::Once, InviteLasts::Week).await?;
            join(rin, None).await?;
            let (film, film_night) =
                make("Film night", InviteUses::Five, InviteLasts::Week).await?;
            join(film_night, Some("Jo")).await?;
            client.cancel_invite(&base, &owner, &film).await.map_err(|e| e.to_string())?;
        }
        Ok(book_club.link())
    }
}
