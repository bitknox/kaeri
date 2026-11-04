//! Cross-device sync and profiles through the engine's commands: two apps over one shared
//! folder, a recovery key that does not fit, and the profile list.
#![cfg(feature = "testing")]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiPlatform, FfiScene, FfiStart};
use kaeri_model::{
    command::Command,
    event::Event,
    group::{
        CloudUnavailable, SyncCloudWay, SyncCommand, SyncCopy, SyncFolderKind, SyncFolderSpec,
        SyncProblem, SyncSignInStep, SyncStatus,
    },
    profiles::ProfilesView,
    stores::Secret,
};

const PATIENCE: Duration = Duration::from_secs(30);

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

struct App {
    engine: Arc<FfiEngine>,
    events: mpsc::Receiver<Event>,
    _root: tempfile::TempDir,
}

impl App {
    fn start() -> Self {
        Self::start_in(None, FfiStart::default())
    }

    fn start_in(scene: Option<FfiScene>, start: FfiStart) -> Self {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = |name: &str| root.path().join(name).display().to_string();
        let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
        let engine = match scene {
            Some(scene) => FfiEngine::start_scene(dirs, Arc::new(Quiet), scene, start),
            None => FfiEngine::start_testing(dirs, Arc::new(Quiet), start),
        }
        .expect("the engine starts");
        let (tx, events) = mpsc::channel();
        let reader = Arc::clone(&engine);
        thread::spawn(move || {
            loop {
                let batch = futures::executor::block_on(reader.next_events());
                if batch.is_empty() {
                    return;
                }
                for event in batch {
                    if tx.send(event).is_err() {
                        return;
                    }
                }
            }
        });
        Self { engine, events, _root: root }
    }

    fn until<T>(&mut self, what: &str, mut want: impl FnMut(&Event) -> Option<T>) -> T {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let Ok(event) = self.events.recv_timeout(left) else { panic!("no {what} in time") };
            if let Some(found) = want(&event) {
                return found;
            }
        }
    }

    fn status_where(&mut self, what: &str, ok: impl Fn(&SyncStatus) -> bool) -> SyncStatus {
        self.until(what, |event| match event {
            Event::SyncStatus(status) if ok(status) => Some(status.clone()),
            _ => None,
        })
    }

    fn profiles_where(
        &mut self,
        what: &str,
        ok: impl Fn(&ProfilesView) -> bool,
    ) -> ProfilesView {
        self.until(what, |event| match event {
            Event::Profiles(view) if ok(view) => Some(view.clone()),
            _ => None,
        })
    }
}

fn folder(path: &std::path::Path) -> SyncFolderSpec {
    SyncFolderSpec {
        kind: SyncFolderKind::Directory,
        address: Some(path.display().to_string()),
        user: None,
        password: None,
    }
}

#[test]
fn two_devices_share_a_group_through_a_folder() {
    let shared = tempfile::tempdir().expect("a shared folder");
    let mut living_room = App::start();
    living_room.engine.send(Command::StartSync {
        device_name: "Living room".into(),
        folder: folder(shared.path()),
    });
    let key = living_room.until("the recovery key", |event| match event {
        Event::SyncRecoveryKey(key) => Some(key.clone()),
        _ => None,
    });
    assert_eq!(key.replace('-', "").len(), 24, "{key}");
    let first = living_room.status_where("sync on", |s| s.enabled && s.last_synced.is_some());
    assert_eq!(first.devices.len(), 1);
    assert_eq!(first.device_name.as_deref(), Some("Living room"));

    let mut phone = App::start();
    phone.engine.send(Command::JoinSync {
        device_name: "Phone".into(),
        folder: folder(shared.path()),
        recovery_key: Secret(key),
    });
    let joined = phone.status_where("both devices", |s| s.enabled && s.devices.len() == 2);
    assert!(joined.devices.iter().any(|d| d.name == "Living room" && !d.this_device));
    assert!(joined.devices.iter().any(|d| d.name == "Phone" && d.this_device));

    living_room.engine.send(Command::SyncFolderNow);
    let seen = living_room.status_where("both devices", |s| s.devices.len() == 2);
    assert!(seen.devices.iter().any(|d| d.name == "Phone"));

    // The recovery key, said again for the sheet behind the lock.
    living_room.engine.send(Command::ShowRecoveryKey);
    living_room
        .until("the key again", |e| matches!(e, Event::SyncRecoveryKey(_)).then_some(()));

    // Leaving keeps the data and ends the sync.
    phone.engine.send(Command::LeaveSync);
    let left = phone.status_where("sync off", |s| !s.enabled);
    assert!(left.devices.is_empty());
}

#[test]
fn a_wrong_recovery_key_does_not_join() {
    let shared = tempfile::tempdir().expect("a shared folder");
    let mut first = App::start();
    first
        .engine
        .send(Command::StartSync { device_name: "One".into(), folder: folder(shared.path()) });
    first.until("the recovery key", |e| matches!(e, Event::SyncRecoveryKey(_)).then_some(()));

    let mut other = App::start();
    other.engine.send(Command::JoinSync {
        device_name: "Other".into(),
        folder: folder(shared.path()),
        recovery_key: Secret("AAAA-AAAA-AAAA-AAAA-AAAA-AAAA".into()),
    });
    let problem = other.until("a problem", |event| match event {
        Event::SyncProblem(problem) => Some(problem.clone()),
        _ => None,
    });
    assert_eq!(problem, SyncProblem::WrongRecoveryKey);
    assert!(!other.status_where("status", |_| true).enabled);
}

#[test]
fn a_single_user_has_no_profiles_and_adding_one_makes_two() {
    let mut app = App::start();
    app.engine.send(Command::LoadProfiles);
    let none = app.profiles_where("the profiles", |_| true);
    assert!(none.profiles.is_empty());

    app.engine.send(Command::AddProfile {
        name: "Kim".into(),
        kids: false,
        first_name: Some("Alex".into()),
    });
    let two = app.profiles_where("two profiles", |v| v.profiles.len() == 2);
    assert_eq!(two.profiles[0].name, "Alex");
    assert!(two.profiles[0].active, "today's data stays the profile in use");
    assert_eq!(two.profiles[1].initials, "K");

    let kim = two.profiles[1].id.clone();
    app.engine.send(Command::EditProfile { id: kim.clone(), name: "Kim R".into(), kids: true });
    let edited = app.profiles_where("edited", |v| v.profiles.iter().any(|p| p.name == "Kim R"));
    assert!(edited.profiles[1].kids);

    // Switching asks the app to start the engine again on the other profile.
    app.engine.send(Command::SwitchProfile { id: kim.clone() });
    let id = app.until("the switch", |e| match e {
        Event::ProfileSwitch { id } => Some(id.clone()),
        _ => None,
    });
    assert_eq!(id, kim);

    // Deleting back to one profile makes it a single user's install again.
    app.engine.send(Command::DeleteProfile { id: kim });
    let one = app.profiles_where("none", |v| v.profiles.is_empty());
    assert!(one.profiles.is_empty());
}

#[test]
fn the_sync_scene_shows_a_second_device_and_a_note() {
    let mut app = App::start_in(Some(FfiScene::SyncOn), FfiStart::default());
    app.engine.send(Command::LoadSync);
    let status = app.status_where("the status", |_| true);
    assert!(status.enabled && !status.syncing && status.error.is_none());
    assert_eq!(status.folder.as_ref().map(|f| f.label.as_str()), Some("Shared/Kaeri"));
    let names: Vec<_> =
        status.devices.iter().map(|d| (d.name.as_str(), d.this_device)).collect();
    assert_eq!(names, [("Kim's iPhone", true), ("Living room TV", false)]);
    assert_eq!(status.notes.len(), 1);
    let note = status.notes[0].id;
    app.engine.send(Command::DismissSyncNote { id: note });
    app.status_where("the note gone", |s| s.notes.is_empty());
}

#[test]
fn the_profiles_scene_starts_in_the_first_profile_and_can_start_in_another() {
    let mut app = App::start_in(Some(FfiScene::Profiles), FfiStart::default());
    app.engine.send(Command::LoadProfiles);
    let view = app.profiles_where("three profiles", |v| v.profiles.len() == 3);
    let names: Vec<_> = view.profiles.iter().map(|p| (p.name.as_str(), p.active)).collect();
    assert_eq!(names, [("Alex", true), ("Kim", false), ("Sam", false)]);
    assert!(view.profiles[2].kids);

    let mut kim = App::start_in(
        Some(FfiScene::Profiles),
        FfiStart { profile: Some("p2".into()), ..FfiStart::default() },
    );
    kim.engine.send(Command::LoadProfiles);
    let view = kim.profiles_where("three profiles", |v| v.profiles.len() == 3);
    assert!(view.profiles[1].active && !view.profiles[0].active);
}

#[test]
fn two_devices_pair_with_a_code_and_a_wrong_code_does_not() {
    let mut living_room = App::start();
    living_room.engine.send(Command::PairInvite { device_name: "Living room".into() });
    let (code, port) = living_room.until("the code", |event| match event {
        Event::SyncInvite { code, port, seconds } => {
            assert_eq!(*seconds, 120);
            Some((code.clone(), *port))
        }
        _ => None,
    });
    assert_eq!(code.len(), 6);

    let mut phone = App::start();
    let address = format!("127.0.0.1:{port}");
    let wrong = if code == "000000" { "111111" } else { "000000" };
    phone.engine.send(Command::PairJoin {
        device_name: "Phone".into(),
        address: address.clone(),
        code: wrong.into(),
    });
    let problem = phone.until("a problem", |event| match event {
        Event::SyncProblem(problem) => Some(problem.clone()),
        _ => None,
    });
    assert!(matches!(problem, SyncProblem::Other(_)), "{problem:?}");

    phone.engine.send(Command::PairJoin { device_name: "Phone".into(), address, code });
    let status = phone.status_where("both devices", |s| s.devices.len() == 2);
    assert!(status.devices.iter().any(|d| d.name == "Living room"));
    let status = living_room.status_where("both devices", |s| s.devices.len() == 2);
    assert!(status.devices.iter().any(|d| d.name == "Phone"));
}

/// Where the fixtures' fake sign-in page sends the browser: its `Location`, read without following
/// it, as the app reads it from the sheet.
fn fake_sign_in_redirect(url: &str) -> String {
    use std::io::{Read, Write};
    let rest = url.strip_prefix("http://").expect("the fake service is on loopback");
    let (host, path) = rest.split_once('/').expect("a path");
    let mut socket = std::net::TcpStream::connect(host).expect("the fake service");
    write!(socket, "GET /{path} HTTP/1.1\r\nhost: {host}\r\nconnection: close\r\n\r\n")
        .expect("a request");
    let mut reply = String::new();
    socket.read_to_string(&mut reply).expect("a reply");
    reply
        .lines()
        .find_map(|l| l.strip_prefix("Location: ").or_else(|| l.strip_prefix("location: ")))
        .expect("a redirect")
        .to_owned()
}

#[test]
fn a_fixtures_phone_walks_through_the_cloud_sign_in_without_an_account() {
    let start = FfiStart { platform: Some(FfiPlatform::Ios), ..FfiStart::default() };
    let mut app = App::start_in(None, start);
    app.engine.send(Command::LoadSync);
    let status = app.status_where("the drives", |s| !s.clouds.is_empty());
    assert!(status.clouds.iter().all(|c| c.way == SyncCloudWay::Sheet), "{:?}", status.clouds);

    app.engine.send(Command::StartSync {
        device_name: "Phone".into(),
        folder: SyncFolderSpec {
            kind: SyncFolderKind::GoogleDrive,
            address: None,
            user: None,
            password: None,
        },
    });
    let (url, scheme) = app.until("the sheet", |event| match event {
        Event::SyncSignIn {
            step: Some(SyncSignInStep::Sheet { url, callback_scheme }),
            ..
        } => Some((url.clone(), callback_scheme.clone())),
        _ => None,
    });
    assert_eq!(scheme, "client.fixture.app");
    let callback = fake_sign_in_redirect(&url);
    assert!(callback.starts_with("client.fixture.app:/oauth2redirect?"), "{callback}");
    app.engine.send(Command::FinishSyncSignIn {
        service: SyncFolderKind::GoogleDrive,
        callback: Secret(callback),
    });
    app.until("the folder", |event| match event {
        Event::SyncSignIn { step: Some(SyncSignInStep::Finishing), .. } => Some(()),
        _ => None,
    });
    app.until("the recovery key", |event| match event {
        Event::SyncRecoveryKey(_) => Some(()),
        _ => None,
    });
    let status = app.status_where("the drive in use", |s| s.enabled && s.last_synced.is_some());
    assert_eq!(status.folder.map(|f| f.kind), Some(SyncFolderKind::GoogleDrive));
    assert!(
        status.clouds.iter().any(|c| c.service == SyncFolderKind::GoogleDrive && c.signed_in)
    );
}

#[test]
fn a_fixtures_tv_shows_a_code_and_cancel_ends_it_and_dropbox_needs_a_phone() {
    let start = FfiStart { platform: Some(FfiPlatform::AndroidTv), ..FfiStart::default() };
    let mut app = App::start_in(None, start);
    app.engine.send(Command::LoadSync);
    let status = app.status_where("the drives", |s| !s.clouds.is_empty());
    let way = |kind: SyncFolderKind| {
        status.clouds.iter().find(|c| c.service == kind).expect("a row").way.clone()
    };
    assert_eq!(way(SyncFolderKind::GoogleDrive), SyncCloudWay::DeviceCode);
    assert_eq!(way(SyncFolderKind::OneDrive), SyncCloudWay::DeviceCode);
    assert_eq!(
        way(SyncFolderKind::Dropbox),
        SyncCloudWay::Unavailable { reason: CloudUnavailable::NeedsPhone }
    );

    app.engine.send(Command::SyncSignIn { service: SyncFolderKind::OneDrive });
    let code = app.until("the code", |event| match event {
        Event::SyncSignIn { step: Some(SyncSignInStep::Code { user_code, .. }), .. } => {
            Some(user_code.clone())
        }
        _ => None,
    });
    assert_eq!(code, "ABCD-EFGH");
    app.engine.send(Command::CancelSyncSignIn);
    app.until("the end", |event| match event {
        Event::SyncSignIn { step: None, .. } => Some(()),
        _ => None,
    });
}

/// Pair `asker`, which shows the code, with `holder`, which enters it. The port `asker` listens
/// on comes back, for sessions after. `asker`'s events are left for the caller to read.
fn pair(asker: &mut App, asker_name: &str, holder: &mut App, holder_name: &str) -> u16 {
    asker.engine.send(Command::PairInvite { device_name: asker_name.into() });
    let (code, port) = asker.until("the code", |event| match event {
        Event::SyncInvite { code, port, .. } => Some((code.clone(), *port)),
        _ => None,
    });
    holder.engine.send(Command::PairJoin {
        device_name: holder_name.into(),
        address: format!("127.0.0.1:{port}"),
        code,
    });
    holder.status_where("both devices", |s| s.devices.len() == 2);
    port
}

fn other_device(status: &SyncStatus) -> String {
    status.devices.iter().find(|d| !d.this_device).expect("another device").id.clone()
}

fn copied(app: &mut App) -> (String, Vec<String>, Vec<SyncFolderKind>) {
    app.until("the sign-ins", |event| match event {
        Event::SyncSignInsCopied { device, trackers, clouds, .. } => {
            Some((device.clone(), trackers.clone(), clouds.clone()))
        }
        _ => None,
    })
}

#[test]
fn a_group_paired_nearby_gains_a_folder_that_its_other_device_opens_without_the_key() {
    let shared = tempfile::tempdir().expect("a shared folder");
    let mut tv = App::start();
    let mut phone = App::start();
    // Copy sign-ins, asked before the code is shown, is answered in the pairing itself.
    tv.engine.send(Command::SyncGroup(SyncCommand::CopySignIns {
        device: None,
        copy: vec![SyncCopy::Trackers],
        address: None,
    }));
    pair(&mut tv, "Living room TV", &mut phone, "Kim's iPhone");
    let (from, trackers, _) = copied(&mut tv);
    assert!(trackers.is_empty(), "the phone is signed in to no tracker: {trackers:?}");
    let paired = tv.status_where("both devices", |s| s.devices.len() == 2);
    assert_eq!(from, other_device(&paired));
    assert!(paired.devices.iter().all(|d| d.asked.is_empty()), "answered, so nothing waits");

    // The phone adds the group's first folder: the same group, with a recovery key now.
    phone.engine.send(Command::StartSync {
        device_name: "Kim's iPhone".into(),
        folder: folder(shared.path()),
    });
    phone.until("the recovery key", |e| matches!(e, Event::SyncRecoveryKey(_)).then_some(()));
    let on = phone.status_where("the folder", |s| s.last_synced.is_some());
    assert_eq!(on.devices.len(), 2, "the device keeps its id: {:?}", on.devices);
    assert!(on.recovery_key && on.recovery_key_made.is_some());
    assert_eq!(on.group_folder, Some(SyncFolderKind::Directory));

    // The TV was wrapped for, so it opens the folder with its own key and no recovery key.
    tv.engine.send(Command::StartSync {
        device_name: "Living room TV".into(),
        folder: folder(shared.path()),
    });
    let joined = tv.until("the folder", |event| match event {
        Event::SyncRecoveryKey(_) => {
            panic!("only the device that made the folder says the key")
        }
        Event::SyncProblem(p) => panic!("{p:?}"),
        Event::SyncStatus(s) if s.last_synced.is_some() => Some(s.clone()),
        _ => None,
    });
    assert_eq!(joined.folder.map(|f| f.kind), Some(SyncFolderKind::Directory));
    assert_eq!(joined.devices.len(), 2);
    assert!(!joined.recovery_key, "the TV never holds the recovery key");

    // A device outside the group finds a group it has no key for.
    let mut stranger = App::start();
    stranger.engine.send(Command::StartSync {
        device_name: "Study PC".into(),
        folder: folder(shared.path()),
    });
    let problem = stranger.until("a problem", |event| match event {
        Event::SyncProblem(problem) => Some(problem.clone()),
        _ => None,
    });
    assert_eq!(problem, SyncProblem::GroupExists);
}

#[test]
fn a_device_moves_to_another_folder_drops_it_and_starts_again_after_leaving() {
    let (first, second, third) = (
        tempfile::tempdir().expect("a folder"),
        tempfile::tempdir().expect("a folder"),
        tempfile::tempdir().expect("a folder"),
    );
    let mut app = App::start();
    app.engine
        .send(Command::StartSync { device_name: "Desk".into(), folder: folder(first.path()) });
    app.until("the recovery key", |e| matches!(e, Event::SyncRecoveryKey(_)).then_some(()));
    let before = app.status_where("the folder", |s| s.last_synced.is_some());
    let me = before.devices[0].id.clone();

    // Moving keeps the group and its recovery key: no new key is said.
    app.engine.send(Command::SyncGroup(SyncCommand::ChangeSyncFolder {
        folder: Some(folder(second.path())),
    }));
    let moved = app.until("the new folder", |event| match event {
        Event::SyncRecoveryKey(_) => panic!("the recovery key stays"),
        Event::SyncStatus(s)
            if s.last_synced.is_some()
                && s.folder.as_ref().is_some_and(|f| {
                    f.label.contains(&*second.path().display().to_string())
                }) =>
        {
            Some(s.clone())
        }
        _ => None,
    });
    assert!(moved.recovery_key);
    assert!(second.path().join("group.bin").exists());

    // Dropping the folder keeps the group, nearby only.
    app.engine.send(Command::SyncGroup(SyncCommand::ChangeSyncFolder { folder: None }));
    let dropped = app.status_where("no folder", |s| s.folder.is_none());
    assert!(dropped.enabled && !dropped.recovery_key);
    assert_eq!(dropped.devices[0].id, me);

    // Leaving, then starting again in another folder, writes under the same id.
    app.engine.send(Command::LeaveSync);
    app.status_where("sync off", |s| !s.enabled);
    app.engine
        .send(Command::StartSync { device_name: "Desk".into(), folder: folder(third.path()) });
    app.until("the recovery key", |e| matches!(e, Event::SyncRecoveryKey(_)).then_some(()));
    let again = app.status_where("on again", |s| s.enabled && s.last_synced.is_some());
    assert_eq!(again.devices.iter().find(|d| d.this_device).map(|d| d.id.clone()), Some(me));
}

#[test]
fn a_tv_uses_a_paired_phones_dropbox_sign_in_and_takes_the_groups_folder() {
    let ios = FfiStart { platform: Some(FfiPlatform::Ios), ..FfiStart::default() };
    let tv_start = FfiStart { platform: Some(FfiPlatform::AndroidTv), ..FfiStart::default() };
    let mut phone = App::start_in(None, ios);
    let mut tv = App::start_in(None, tv_start);
    let port = pair(&mut tv, "Living room TV", &mut phone, "Kim's iPhone");

    // The phone sets the group's folder up in Dropbox, through the fixtures' fake sheet.
    phone.engine.send(Command::StartSync {
        device_name: "Kim's iPhone".into(),
        folder: SyncFolderSpec {
            kind: SyncFolderKind::Dropbox,
            address: None,
            user: None,
            password: None,
        },
    });
    let url = phone.until("the sheet", |event| match event {
        Event::SyncSignIn { step: Some(SyncSignInStep::Sheet { url, .. }), .. } => {
            Some(url.clone())
        }
        _ => None,
    });
    phone.engine.send(Command::FinishSyncSignIn {
        service: SyncFolderKind::Dropbox,
        callback: Secret(fake_sign_in_redirect(&url)),
    });
    let on = phone.status_where("the folder", |s| s.last_synced.is_some());
    let tv_id = other_device(&on);

    // The TV asks; the phone's next session with it carries the answer.
    let tv_status = tv.status_where("both devices", |s| s.devices.len() == 2);
    let phone_id = other_device(&tv_status);
    tv.engine.send(Command::SyncGroup(SyncCommand::CopySignIns {
        device: Some(phone_id.clone()),
        copy: vec![SyncCopy::Cloud { service: SyncFolderKind::Dropbox }],
        address: None,
    }));
    let asked = tv.status_where("asked", |s| s.devices.iter().any(|d| !d.asked.is_empty()));
    assert!(
        asked
            .devices
            .iter()
            .any(|d| d.asked == [SyncCopy::Cloud { service: SyncFolderKind::Dropbox }])
    );
    phone
        .engine
        .send(Command::SyncWithDevice { device: tv_id, address: format!("127.0.0.1:{port}") });
    let (from, _, clouds) = copied(&mut tv);
    assert_eq!(from, phone_id);
    assert_eq!(clouds, [SyncFolderKind::Dropbox]);
    let taken = tv.status_where("the folder", |s| {
        s.folder.is_some() && s.devices.iter().any(|d| d.folder.is_some())
    });
    assert_eq!(taken.folder.map(|f| f.kind), Some(SyncFolderKind::Dropbox));
    assert!(taken.clouds.iter().any(|c| c.service == SyncFolderKind::Dropbox && c.signed_in));
    assert!(!taken.recovery_key);
    let held = taken.devices.iter().find(|d| d.id == phone_id).expect("the phone");
    assert_eq!(held.folder, Some(SyncFolderKind::Dropbox));
    assert_eq!(held.sign_ins, [SyncFolderKind::Dropbox]);
    assert!(held.asked.is_empty());
}

#[test]
fn the_sync_scenes_show_nearby_cloud_and_a_tv_with_a_phones_sign_in() {
    let mut nearby = App::start_in(Some(FfiScene::SyncNearby), FfiStart::default());
    nearby.engine.send(Command::LoadSync);
    let status = nearby.status_where("the status", |_| true);
    assert!(status.enabled && status.folder.is_none() && !status.recovery_key);
    let tv = status.devices.iter().find(|d| !d.this_device).expect("the TV");
    assert_eq!(
        (tv.name.as_str(), tv.trackers.as_slice()),
        ("Living room TV", &["anilist".to_owned()][..])
    );

    let mut cloud = App::start_in(Some(FfiScene::SyncCloud), FfiStart::default());
    cloud.engine.send(Command::LoadSync);
    let status = cloud.status_where("the status", |_| true);
    let folder = status.folder.expect("a folder");
    assert_eq!(folder.kind, SyncFolderKind::GoogleDrive);
    assert_eq!(folder.account.as_deref(), Some("kim@mail.example"));
    assert!(status.recovery_key && status.recovery_key_made.is_some());
    let mut names: Vec<_> = status.devices.iter().map(|d| d.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Kim's MacBook Air", "Kim's iPhone"]);
    let drive = status.clouds.iter().find(|c| c.service == SyncFolderKind::GoogleDrive);
    assert!(drive.is_some_and(|c| c.signed_in && c.account.is_some()));
    cloud.engine.send(Command::ShowRecoveryKey);
    cloud.until("the key", |e| matches!(e, Event::SyncRecoveryKey(_)).then_some(()));

    let start = FfiStart { platform: Some(FfiPlatform::AndroidTv), ..FfiStart::default() };
    let mut tv = App::start_in(Some(FfiScene::SyncTvPhoneSignIn), start);
    tv.engine.send(Command::LoadSync);
    let status = tv.status_where("the status", |_| true);
    assert!(status.folder.is_none());
    assert_eq!(status.group_folder, Some(SyncFolderKind::Dropbox));
    let phone = status.devices.iter().find(|d| !d.this_device).expect("the phone");
    assert_eq!(phone.name, "Kim's iPhone");
    assert_eq!(phone.sign_ins, [SyncFolderKind::Dropbox]);
    let dropbox = status.clouds.iter().find(|c| c.service == SyncFolderKind::Dropbox);
    assert_eq!(
        dropbox.map(|c| c.way.clone()),
        Some(SyncCloudWay::Unavailable { reason: CloudUnavailable::NeedsPhone })
    );
}
