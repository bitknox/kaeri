//! The engine as the bindings drive it: started for a test, sent commands from threads that are
//! not the engine's, and read by polling `next_events` outside its runtime, the way a foreign
//! executor polls it.
#![cfg(feature = "testing")]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use kaeri_core::ids::AnilistId;
use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiStart};
use kaeri_model::{
    catalog::{ContentFor, ContentList, Entry},
    command::Command,
    event::Event,
    opened::OpenedCommand,
    search::SearchQuery,
};

/// How long a test waits for what it is waiting on before failing. Everything is on loopback,
/// so a wait this long is a bug.
const PATIENCE: Duration = Duration::from_secs(30);

/// A host that opens nothing, shows nothing and keeps no secrets.
struct Quiet;

impl FfiHost for Quiet {
    fn open_url(&self, _url: String) -> bool {
        false
    }

    fn notify(&self, _title: String, _body: String) {}

    fn secret_get(&self, _id: String) -> Option<String> {
        None
    }

    fn secret_set(&self, _id: String, _value: String) -> bool {
        false
    }

    fn secret_delete(&self, _id: String) -> bool {
        true
    }
}

/// An engine started for a test in a temporary directory, which it is kept beside.
fn started() -> (Arc<FfiEngine>, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_testing(dirs, Arc::new(Quiet), FfiStart::default())
        .expect("the engine starts");
    (engine, root)
}

/// Every event until one `until` accepts, that one included.
///
/// Polled on a plain thread with an executor that is not tokio's, as the bindings poll it from
/// Swift or Kotlin.
fn read_until(
    engine: &Arc<FfiEngine>,
    until: impl Fn(&Event) -> bool + Send + 'static,
) -> Vec<Event> {
    let engine = Arc::clone(engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut read = Vec::new();
        let found = loop {
            let batch = futures::executor::block_on(engine.next_events());
            // Empty once the engine has stopped.
            if batch.is_empty() {
                break false;
            }
            let found = batch.iter().any(&until);
            read.extend(batch);
            if found {
                break true;
            }
        };
        let _ = tx.send((read, found));
    });
    let (read, found) = rx.recv_timeout(PATIENCE).expect("the event in time");
    assert!(found, "the engine stopped first, having said: {read:?}");
    read
}

/// The titles of the first list for a search for `query` among `events`.
fn search_titles<'a>(events: &'a [Event], query: &str) -> Option<&'a [Entry]> {
    events.iter().find_map(|event| match event {
        Event::Content {
            view: ContentFor::Search { query: q },
            content: ContentList::Entries(entries),
        } if q.text == query => Some(entries.as_slice()),
        _ => None,
    })
}

/// Whether `event` ends `job`.
fn ends(job: kaeri_model::jobs::JobId) -> impl Fn(&Event) -> bool + Send + 'static {
    move |event| matches!(event, Event::JobDone(done) if *done == job)
}

#[test]
fn a_search_comes_back_with_the_fixture_titles() {
    let (engine, _root) = started();
    let search = Command::Search { query: SearchQuery::text("hoshikuzu") };
    let job = engine.send(search).expect("a search is work someone waits on");

    // The job ends after everything its work said, the list among it.
    let read = read_until(&engine, ends(job));
    let titles: Vec<_> = search_titles(&read, "hoshikuzu")
        .expect("the search's titles")
        .iter()
        .map(|entry| (entry.id.get(), entry.title.as_str(), entry.secondary.as_deref()))
        .collect();
    assert_eq!(
        titles,
        [
            (900_001, "Stardust Wayfarer", Some("Hoshikuzu no Tabibito")),
            (900_004, "Stardust Wayfarer Season 2", Some("Hoshikuzu no Tabibito 2nd Season")),
        ]
    );
    // What launch said came first, the settings ahead of everything.
    assert!(matches!(read.first(), Some(Event::Settings(_))), "{read:?}");
    engine.shutdown();
}

#[test]
fn commands_can_be_sent_from_any_thread() {
    let (engine, _root) = started();
    let sender = Arc::clone(&engine);
    let job = thread::spawn(move || sender.send(Command::LoadContinue))
        .join()
        .expect("the sending thread")
        .expect("loading Continue is a job");

    let read = read_until(&engine, ends(job));
    // Nothing has been watched in a new database.
    assert!(
        read.iter().any(|event| matches!(
            event,
            Event::Content { view: ContentFor::Continue, content: ContentList::Empty }
        )),
        "{read:?}"
    );
    engine.shutdown();
}

#[test]
fn a_title_comes_back_whole_with_the_source_pinned_for_it_ahead() {
    let (engine, _root) = started();
    let job = engine
        .send(Command::LoadDetail(AnilistId::new(900_001)))
        .expect("loading a title is a job");

    let read = read_until(&engine, ends(job));
    let asked: Vec<_> = read
        .iter()
        .skip_while(
            |event| !matches!(event, Event::JobStarted { job: started, .. } if *started == job),
        )
        .collect();
    // Nothing pinned in a new database, said ahead of the page so it never shows the wrong one.
    assert!(matches!(asked[1], Event::ProviderPreference(None)), "{asked:?}");
    let Event::Detail(entry) = asked[2] else { panic!("the page: {asked:?}") };
    assert_eq!((entry.id.get(), entry.title.as_str()), (900_001, "Stardust Wayfarer"));
    assert!(!entry.related.is_empty(), "the detail-only fields came with it");
    engine.shutdown();
}

#[test]
fn trailers_are_held_by_what_the_device_and_the_network_say() {
    use kaeri_model::trailer::{AutoplayHold, AutoplayView};
    let (engine, _root) = started();
    let saying = |expected: AutoplayView| move |event: &Event| matches!(event, Event::Autoplay(view) if *view == expected);
    let free = AutoplayView { trailers: true, held_by: None };
    let held = |hold| AutoplayView { trailers: false, held_by: Some(hold) };

    // Launch says trailers may start: Always is the default, and nothing is known to hold them.
    read_until(&engine, saying(free));

    let device = |screen_reader| Command::DeviceState {
        reduce_motion: false,
        autoplay_previews: true,
        low_power: false,
        thermal_serious: false,
        screen_reader,
        low_memory: false,
    };
    engine.send(device(true));
    read_until(&engine, saying(held(AutoplayHold::ScreenReader)));
    engine.send(device(false));
    read_until(&engine, saying(free));
    engine.send(Command::NetworkPath { cellular: false, constrained: true, offline: false });
    read_until(&engine, saying(held(AutoplayHold::Network)));
    engine.shutdown();
}

/// The path as a platform monitor says it.
fn path(offline: bool) -> Command {
    Command::NetworkPath { cellular: false, constrained: false, offline }
}

#[test]
fn being_offline_is_said_once_and_coming_back_loads_again_what_was_asked_meanwhile() {
    let (engine, _root) = started();
    let connectivity = |event: &Event| matches!(event, Event::Connectivity { .. });
    let season = |event: &Event| matches!(event, Event::JobStarted { activity, .. } if activity == "loading this season");

    // The path goes: said once, however often the monitor repeats it.
    engine.send(path(true));
    engine.send(path(true));
    let read = read_until(&engine, connectivity);
    // The trailers held while offline come in the same batch, after it.
    assert!(
        read.iter().any(|event| matches!(event, Event::Connectivity { offline: true })),
        "{read:?}"
    );

    // Nothing is refused while offline: the screen asks, and is answered as it can be.
    let job = engine.send(Command::LoadSeasonal).expect("loading the season is a job");
    let read = read_until(&engine, ends(job));
    assert!(!read.iter().any(connectivity), "said once: {read:?}");

    // Back: said, then the season asked for again, once, as a job of its own.
    engine.send(path(false));
    let read = read_until(&engine, |event| {
        matches!(event, Event::Content { view: ContentFor::Seasonal, .. })
    });
    let back = read
        .iter()
        .position(|event| matches!(event, Event::Connectivity { offline: false }))
        .unwrap_or_else(|| panic!("back online said: {read:?}"));
    let again = read.iter().position(season);
    assert!(again.is_some_and(|again| again > back), "{read:?}");

    // Online already: nothing said, and nothing loaded again.
    engine.send(path(false));
    let job = engine.send(Command::LoadDetail(AnilistId::new(900_001))).expect("a job");
    let read = read_until(&engine, ends(job));
    assert!(!read.iter().any(|event| connectivity(event) || season(event)), "{read:?}");
    engine.shutdown();
}

#[test]
fn the_fake_anilist_says_what_it_was_asked() {
    let (engine, _root) = started();
    let before = engine.testing_anilist_requests().len();
    let job = engine
        .send(Command::LoadDetail(AnilistId::new(900_001)))
        .expect("loading a title is a job");
    read_until(&engine, ends(job));
    // One title is one request, by its operation's name. The page's season headers are asked
    // for apart from its job, in the background, so they may or may not have gone by now.
    let asked = engine.testing_anilist_requests()[before..].to_vec();
    assert_eq!(asked[0], "Detail");
    assert!(asked[1..].iter().all(|name| name == "Many"), "{asked:?}");
    engine.shutdown();
}

#[test]
fn shutdown_returns_and_ends_the_stream() {
    let (engine, _root) = started();
    let stopping = Arc::clone(&engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        stopping.shutdown();
        let _ = tx.send(());
    });
    rx.recv_timeout(PATIENCE).expect("shutdown returns");

    // Launch's events are still there to read, and then nothing more comes.
    let mut batches = 0;
    while !futures::executor::block_on(engine.next_events()).is_empty() {
        batches += 1;
        assert!(batches < 100, "the stream ends once the engine has stopped");
    }
    assert_eq!(engine.send(Command::LoadContinue), None);
    // A second shutdown has nothing left to stop.
    engine.shutdown();
}

#[test]
fn a_fixture_cover_arrives_as_an_image() {
    let (engine, _root) = started();
    let search = Command::Search { query: SearchQuery::text("hoshikuzu") };
    let job = engine.send(search).expect("a search is work someone waits on");
    let read = read_until(&engine, ends(job));
    let cover = search_titles(&read, "hoshikuzu").expect("the search's titles")[0]
        .cover_url
        .clone()
        .expect("a fixture title has a cover");

    // The fake serves the fixtures' art, on loopback, so the app shows real images.
    assert!(cover.starts_with("http://127.0.0.1:"), "{cover}");
    assert_eq!(engine.send(Command::FetchArtwork { url: cover.clone() }), None);
    let read = read_until(&engine, |event| {
        matches!(event, Event::ArtworkReady { .. } | Event::ImageFailed { .. })
    });
    let Some(Event::ArtworkReady { url, bytes }) = read.last() else {
        panic!("the cover arrives: {read:?}");
    };
    assert_eq!(url, &cover);
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "a PNG");
}

#[test]
fn the_watching_scene_is_signed_in_and_its_library_is_the_fakes() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_scene(
        dirs,
        Arc::new(Quiet),
        kaeri_ffi::FfiScene::Watching,
        kaeri_ffi::FfiStart::default(),
    )
    .expect("the engine starts");

    let job = engine
        .send(Command::LoadLibrary(kaeri_model::catalog::LibrarySegment::Watching))
        .expect("a library load is work someone waits on");
    let read = read_until(&engine, ends(job));
    let watching = read.iter().find_map(|event| match event {
        Event::Content {
            view: ContentFor::Library(_),
            content: ContentList::Entries(rows),
        } => Some(rows.iter().map(|row| row.id.get()).collect::<Vec<_>>()),
        _ => None,
    });
    assert_eq!(watching, Some(vec![900_002]), "{read:?}");
    let signed_in = read.iter().any(|event| {
        matches!(event, Event::Sync(state) if state.tracker == "anilist" && state.connected)
    });
    assert!(signed_in, "AniList says it is signed in: {read:?}");
}

/// The confirm step for a link the viewer opened: the scene opens its fixture link, and the
/// engine guesses the title and the episode from its name, with Change's titles and whether
/// torrent playback is on. A link that is not one is refused, and a bare hash has no guess.
#[test]
fn an_opened_link_is_guessed_from_its_name_for_the_viewer_to_confirm() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_scene(
        dirs,
        Arc::new(Quiet),
        kaeri_ffi::FfiScene::LinkOpened,
        kaeri_ffi::FfiStart::default(),
    )
    .expect("the engine starts");

    let read = read_until(&engine, |event| matches!(event, Event::LinkOpened(_)));
    let opened = read
        .iter()
        .find_map(|event| match event {
            Event::LinkOpened(opened) => Some(opened.clone()),
            _ => None,
        })
        .expect("the link is read");
    assert_eq!(opened.link, kaeri_ffi::fixture_opened_link());
    assert_eq!(opened.name.as_deref(), Some("Hoshikuzu no Tabibito - 03 [1080p].mkv"));
    let guess = opened.guess.as_ref().expect("a guess");
    assert_eq!(guess.title.id, AnilistId::new(900_001), "the first season, not the sequel");
    assert_eq!(guess.episode.as_deref(), Some("3"));
    assert_eq!(opened.candidates.first().map(|e| e.id), Some(guess.title.id));
    assert!(
        opened.candidates.iter().any(|e| e.id == AnilistId::new(900_004)),
        "Change lists it"
    );
    assert!(!opened.playback_on, "torrent playback is off in the scenes");

    // Change: other words find other titles.
    let job = engine
        .send(Command::Opened(OpenedCommand::Find {
            link: opened.link.clone(),
            query: "Kumo no Ue".into(),
        }))
        .expect("a search is work someone waits on");
    let read = read_until(&engine, ends(job));
    let found = read.iter().find_map(|event| match event {
        Event::OpenedChoices { candidates, failed: None, .. } => {
            Some(candidates.iter().map(|e| e.id).collect::<Vec<_>>())
        }
        _ => None,
    });
    let airing = AnilistId::new(900_002);
    assert!(found.as_ref().is_some_and(|ids| ids.contains(&airing)), "{found:?}");

    // Play is refused while torrent playback is off, and records nothing.
    engine.send(Command::Opened(OpenedCommand::Play {
        link: opened.link.clone(),
        id: guess.title.id,
        episode: "3".into(),
        title: guess.title.title.clone(),
    }));
    let read = read_until(&engine, |event| matches!(event, Event::Toast(_)));
    assert!(
        read.iter().any(|event| matches!(event, Event::Toast(notice)
            if notice.msg == Some(kaeri_model::Msg::Store(kaeri_model::StoreMsg::TorrentLinkPlaybackOff)))),
        "{read:?}"
    );

    // A bare hash has no name to guess from; a page is not a link.
    let hash = "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e";
    engine.send(Command::Opened(OpenedCommand::OpenLink { input: hash.into() }));
    let read = read_until(&engine, |event| matches!(event, Event::LinkOpened(_)));
    let bare = read.iter().find_map(|event| match event {
        Event::LinkOpened(opened) => Some(opened.clone()),
        _ => None,
    });
    assert!(
        bare.is_some_and(|b| b.name.is_none() && b.guess.is_none() && b.candidates.is_empty())
    );
    engine.send(Command::Opened(OpenedCommand::OpenLink {
        input: "https://index.example/1".into(),
    }));
    let read = read_until(&engine, |event| matches!(event, Event::Toast(_)));
    assert!(
        read.iter().any(|event| matches!(event, Event::Toast(notice)
            if notice.msg == Some(kaeri_model::Msg::Store(kaeri_model::StoreMsg::TorrentLinkUnusable)))),
        "{read:?}"
    );
}

fn store_scene(scene: kaeri_ffi::FfiScene) -> (Arc<FfiEngine>, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_scene(dirs, Arc::new(Quiet), scene, FfiStart::default())
        .expect("the engine starts");
    (engine, root)
}

#[test]
fn the_store_owner_scene_lists_invites_in_use_and_ended() {
    let (engine, _root) = store_scene(kaeri_ffi::FfiScene::StoreOwner);
    let job = engine.send(Command::LoadStores).expect("loading stores is waited on");
    let read = read_until(&engine, ends(job));
    let store = read
        .iter()
        .find_map(|event| match event {
            Event::Stores(rows) => rows.first().map(|row| row.id.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("the scene's store is joined: {read:?}"));

    let job = engine.send(Command::LoadMembers { store }).expect("members are waited on");
    let read = read_until(&engine, ends(job));
    let Some(Event::StoreMembers { invites, ended, invite_choices, .. }) =
        read.iter().find(|event| matches!(event, Event::StoreMembers { .. }))
    else {
        panic!("the members arrive: {read:?}");
    };
    assert!(invite_choices);
    let book_club = invites.iter().find(|row| row.name == "Book club").expect("Book club");
    assert_eq!((book_club.used, book_club.max), (2, 5));
    assert!(invites.iter().any(|row| row.name == "Sam" && row.used == 0));
    let mut ended_names = ended.iter().map(|row| row.name.as_str()).collect::<Vec<_>>();
    ended_names.sort_unstable();
    // The owner's own invite, used when the scene joined, ended with its one use.
    assert_eq!(ended_names, ["Film night", "Owner", "Rin"]);
    assert!(ended.iter().all(|row| row.ended.is_some()));
}

#[test]
fn the_store_invited_scene_hands_over_a_link_that_asks_for_a_name() {
    let (engine, _root) = store_scene(kaeri_ffi::FfiScene::StoreInvited);
    let link = engine.testing_store_invite().expect("the scene's invite link");
    let job = engine
        .send(Command::PreviewStore { link: kaeri_model::stores::Secret(link) })
        .expect("a preview");
    let read = read_until(&engine, ends(job));
    let preview = read.iter().find_map(|event| match event {
        Event::StorePreview(kaeri_model::stores::InvitePreview::Store(store)) => Some(store),
        _ => None,
    });
    let store = preview.unwrap_or_else(|| panic!("the store's preview: {read:?}"));
    assert_eq!(store.name, "Hilltop Plugins");
    assert!(store.names_joiner, "the link works five times, so it asks for a name");
}
