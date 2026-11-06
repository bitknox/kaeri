//! Devices in one sync group, end to end through the engine as the apps run it: what one does
//! reaches the others through the shared folder, a change made on two settles the same way, and
//! a device removed from the group stops receiving. One test, ignored, runs it all on a real
//! WebDAV server. The road's own rules are tested in kaeri-sync and the merge rules in
//! kaeri-store; this is the engine between them.
#![cfg(feature = "testing")]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use kaeri_core::settings::SettingValue;
use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiStart};
use kaeri_model::{
    catalog::LibrarySegment,
    command::Command,
    event::Event,
    group::{SyncFolderKind, SyncFolderSpec, SyncProblem, SyncStatus},
    settings::SettingKey,
    stores::Secret,
};
use kaeri_testkit::fixtures::{AIRING, FINISHED};

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

struct Device {
    engine: Arc<FfiEngine>,
    events: mpsc::Receiver<Event>,
    root: tempfile::TempDir,
}

impl Device {
    fn start() -> Self {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = |name: &str| root.path().join(name).display().to_string();
        let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
        let engine = FfiEngine::start_testing(dirs, Arc::new(Quiet), FfiStart::default())
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
        Self { engine, events, root }
    }

    fn send(&self, command: Command) {
        self.engine.send(command);
    }

    fn status_where(&mut self, what: &str, ok: impl Fn(&SyncStatus) -> bool) -> SyncStatus {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let Ok(event) = self.events.recv_timeout(left) else { panic!("no {what} in time") };
            if let Event::SyncStatus(status) = event
                && ok(&status)
            {
                return status;
            }
        }
    }

    /// A pass through the folder, waited out. What was said before it is let go first, so an
    /// older status does not stand in for the pass's own.
    fn pass(&mut self) -> SyncStatus {
        while self.events.try_recv().is_ok() {}
        self.send(Command::SyncFolderNow);
        self.status_where("a pass", |s| !s.syncing && s.last_synced.is_some())
    }

    /// This device's id in the group.
    fn id(&mut self) -> String {
        self.pass().devices.into_iter().find(|d| d.this_device).expect("it lists itself").id
    }

    /// Mark `episodes` of `id` watched, and wait until the device kept them.
    fn watch(&self, id: kaeri_core::ids::AnilistId, episodes: &[&str]) {
        self.send(Command::SetWatched {
            id,
            episodes: episodes.iter().map(|e| (*e).to_owned()).collect(),
            watched: true,
        });
        let deadline = Instant::now() + PATIENCE;
        while !episodes.iter().all(|e| self.watched(id).iter().any(|w| w == e)) {
            assert!(
                Instant::now() < deadline,
                "the device never kept its watch of {episodes:?}"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// The device's own database, as the engine keeps it.
    fn store(&self) -> kaeri_store::Store {
        kaeri_store::Store::open(self.root.path().join("data/kaeri.db")).expect("the database")
    }

    fn watched(&self, id: kaeri_core::ids::AnilistId) -> Vec<String> {
        let mut episodes: Vec<String> = self
            .store()
            .events_for(id, 1_000)
            .unwrap()
            .into_iter()
            .map(|e| e.episode)
            .collect();
        episodes.sort();
        episodes.dedup();
        episodes
    }

    fn config(&self) -> String {
        std::fs::read_to_string(self.root.path().join("config/config.toml")).unwrap_or_default()
    }

    /// Wait, passing through the folder now and then, until `done` holds on this device.
    fn until_synced(&mut self, what: &str, done: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + PATIENCE;
        while !done(self) {
            assert!(
                Instant::now() < deadline,
                "{what} never reached this device: watched {:?}, status {:?}, skip_filler {:?}",
                self.watched(FINISHED),
                self.store().synced_status(FINISHED.get()),
                self.config().lines().find(|l| l.contains("skip_filler")),
            );
            self.pass();
            thread::sleep(Duration::from_millis(100));
        }
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

/// A device that sets sync up in `folder`, and the recovery key it is shown.
fn set_up(name: &str, folder: SyncFolderSpec) -> (Device, String) {
    let mut device = Device::start();
    device.send(Command::StartSync { device_name: name.into(), folder });
    let key = {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match device.events.recv_timeout(left).expect("the recovery key") {
                Event::SyncRecoveryKey(key) => break key,
                Event::SyncProblem(p) => panic!("{name} could not set sync up: {p:?}"),
                _ => {}
            }
        }
    };
    device.status_where("sync on", |s| s.enabled && s.last_synced.is_some());
    (device, key)
}

/// A device that joins the group in `folder` with its recovery key, once it lists `devices`.
fn join(name: &str, folder: SyncFolderSpec, key: &str, devices: usize) -> Device {
    let mut device = Device::start();
    device.send(Command::JoinSync {
        device_name: name.into(),
        folder,
        recovery_key: Secret(key.to_owned()),
    });
    device.status_where("the group's devices", |s| s.enabled && s.devices.len() == devices);
    device
}

/// Two devices in a group through a shared folder: the first sets it up, the second joins with
/// the recovery key.
fn group(shared: &std::path::Path) -> (Device, Device) {
    let (mut desk, key) = set_up("Desk", folder(shared));
    let laptop = join("Laptop", folder(shared), &key, 2);
    desk.pass();
    (desk, laptop)
}

/// Watched episodes, a list status and a shared setting set on one device are on the other
/// after a pass on each; a setting kept to the device stays where it was set.
#[test]
fn what_one_device_does_reaches_the_other() {
    let shared = tempfile::tempdir().expect("a shared folder");
    let (mut desk, mut laptop) = group(shared.path());

    desk.send(Command::SetWatched {
        id: FINISHED,
        episodes: vec!["1".into(), "2".into(), "3".into()],
        watched: true,
    });
    desk.send(Command::SetStatus { id: FINISHED, status: LibrarySegment::Watching });
    desk.send(Command::SetSetting {
        key: SettingKey::SkipFiller,
        value: SettingValue::Bool(true),
    });
    let deadline = Instant::now() + PATIENCE;
    while desk.watched(FINISHED).len() < 3
        || desk.store().synced_status(FINISHED.get()).unwrap().is_none()
    {
        assert!(Instant::now() < deadline, "the desk never kept its own changes");
        thread::sleep(Duration::from_millis(50));
    }
    desk.pass();

    laptop.until_synced("the desk's changes", |d| {
        d.watched(FINISHED) == ["1", "2", "3"]
            && d.store().synced_status(FINISHED.get()).unwrap().as_deref() == Some("CURRENT")
            && d.config().contains("skip_filler = true")
    });

    // An episode unwatched on the laptop is unwatched on the desk.
    laptop.send(Command::SetWatched {
        id: FINISHED,
        episodes: vec!["3".into()],
        watched: false,
    });
    let deadline = Instant::now() + PATIENCE;
    while laptop.watched(FINISHED).len() != 2 {
        assert!(Instant::now() < deadline, "the laptop never kept its unwatch");
        thread::sleep(Duration::from_millis(50));
    }
    laptop.pass();
    desk.until_synced("the laptop's unwatch", |d| d.watched(FINISHED) == ["1", "2"]);
}

/// Both devices change the same title's status before either passes: both end on the later
/// change, whichever device passes first.
#[test]
fn a_change_made_on_both_settles_the_same_on_both() {
    let shared = tempfile::tempdir().expect("a shared folder");
    let (mut desk, mut laptop) = group(shared.path());

    desk.send(Command::SetStatus { id: AIRING, status: LibrarySegment::Paused });
    thread::sleep(Duration::from_millis(300));
    laptop.send(Command::SetStatus { id: AIRING, status: LibrarySegment::Dropped });
    let deadline = Instant::now() + PATIENCE;
    while desk.store().synced_status(AIRING.get()).unwrap().is_none()
        || laptop.store().synced_status(AIRING.get()).unwrap().is_none()
    {
        assert!(Instant::now() < deadline, "the statuses were never kept");
        thread::sleep(Duration::from_millis(50));
    }
    // The laptop's later change passes last, and still both end on it; then the other order.
    laptop.pass();
    desk.pass();
    laptop.pass();
    let on = |d: &Device| d.store().synced_status(AIRING.get()).unwrap();
    assert_eq!(on(&desk).as_deref(), Some("DROPPED"));
    assert_eq!(on(&laptop).as_deref(), Some("DROPPED"));
}

/// A device removed from the group is listed by nobody, what the group does after no longer
/// reaches it, and it says it was removed.
#[test]
fn a_removed_device_stops_receiving() {
    let shared = tempfile::tempdir().expect("a shared folder");
    let (mut desk, mut laptop) = group(shared.path());
    let laptop_id = laptop
        .pass()
        .devices
        .into_iter()
        .find(|d| d.this_device)
        .expect("the laptop lists itself")
        .id;

    desk.send(Command::RemoveSyncDevice { device: laptop_id.clone() });
    let after =
        desk.status_where("the laptop gone", |s| s.devices.iter().all(|d| d.id != laptop_id));
    assert_eq!(after.devices.len(), 1);

    desk.send(Command::SetWatched { id: FINISHED, episodes: vec!["5".into()], watched: true });
    thread::sleep(Duration::from_millis(500));
    desk.pass();
    let seen = laptop.pass();
    assert!(
        seen.error.as_deref().is_some_and(|e| e.contains("removed from the group")),
        "the laptop is told it was removed: {seen:?}"
    );
    thread::sleep(Duration::from_millis(300));
    laptop.pass();
    assert!(
        !laptop.watched(FINISHED).contains(&"5".to_owned()),
        "a removed device still received the group's changes: {seen:?}"
    );
}

/// Pass through the folder on each device in turn until `done` holds on all of them.
fn rounds(devices: &mut [&mut Device], what: &str, done: impl Fn(&Device) -> bool) {
    let deadline = Instant::now() + PATIENCE * 2;
    loop {
        for d in devices.iter_mut() {
            d.pass();
        }
        if devices.iter().all(|d| done(d)) {
            return;
        }
        assert!(Instant::now() < deadline, "{what} never settled on every device");
        thread::sleep(Duration::from_millis(100));
    }
}

fn removed(status: &SyncStatus) -> bool {
    status.error.as_deref().is_some_and(|e| e.contains("removed from the group"))
}

/// Ask `device` for a new recovery key, and the key it says.
fn new_recovery_key(device: &mut Device) -> String {
    while device.events.try_recv().is_ok() {}
    device.send(Command::NewRecoveryKey);
    let deadline = Instant::now() + PATIENCE;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if let Event::SyncRecoveryKey(key) =
            device.events.recv_timeout(left).expect("a new key")
        {
            return key;
        }
    }
}

/// A new device that tries to join with `key`: the devices it lists, or the problem it is told.
fn try_join(folder: SyncFolderSpec, key: &str) -> Result<usize, SyncProblem> {
    let device = Device::start();
    device.send(Command::JoinSync {
        device_name: "Spare".into(),
        folder,
        recovery_key: Secret(key.to_owned()),
    });
    let deadline = Instant::now() + PATIENCE;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match device.events.recv_timeout(left).expect("the join's outcome") {
            Event::SyncProblem(p) => return Err(p),
            Event::SyncStatus(s) if s.enabled && s.last_synced.is_some() => {
                return Ok(s.devices.len());
            }
            _ => {}
        }
    }
}

/// A device still holding the recovery key from before another device made a new one removes a
/// device: the new key keeps opening the folder and the old one does not, the device that held
/// the old key no longer offers it, and the removed device is told.
#[test]
fn a_removal_never_brings_back_a_replaced_recovery_key() {
    let shared = tempfile::tempdir().expect("a shared folder");
    let (mut desk, old) = set_up("Desk", folder(shared.path()));
    let mut laptop = join("Laptop", folder(shared.path()), &old, 2);
    let mut tv = join("Television", folder(shared.path()), &old, 3);
    let tv_id = tv.id();
    rounds(&mut [&mut desk, &mut laptop, &mut tv], "the three devices", |_| true);
    let fresh = new_recovery_key(&mut desk);
    assert_ne!(fresh, old);

    laptop.send(Command::RemoveSyncDevice { device: tv_id.clone() });
    let after = laptop.status_where("the TV gone", |s| s.devices.iter().all(|d| d.id != tv_id));
    assert!(!after.recovery_key, "the laptop still offers a key that opens nothing: {after:?}");
    assert!(matches!(
        try_join(folder(shared.path()), &old),
        Err(SyncProblem::WrongRecoveryKey)
    ));
    assert!(try_join(folder(shared.path()), &fresh).is_ok());

    rounds(&mut [&mut desk, &mut laptop], "the removal", |d| {
        d.store().sync_peers().unwrap().iter().any(|p| p.0 == tv_id && p.3)
    });
    let deadline = Instant::now() + PATIENCE;
    while !removed(&tv.pass()) {
        assert!(Instant::now() < deadline, "the TV is never told it was removed");
    }
}

/// The WebDAV server a person's own sync folder would be on, from `KAERI_TEST_WEBDAV` (its
/// address), `KAERI_TEST_WEBDAV_USER` and `KAERI_TEST_WEBDAV_PASSWORD`.
fn webdav() -> SyncFolderSpec {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
    SyncFolderSpec {
        kind: SyncFolderKind::WebDav,
        address: Some(var("KAERI_TEST_WEBDAV")),
        user: Some(var("KAERI_TEST_WEBDAV_USER")),
        password: Some(Secret(var("KAERI_TEST_WEBDAV_PASSWORD"))),
    }
}

/// Three devices, then others, in one group on a real WebDAV server: watches, a list status and
/// a shared setting reach every device, changes made on two settle the same everywhere, and a
/// device removed, online or while offline, is told so on every pass while the others carry on
/// with nothing lost. A removal by a device holding a replaced recovery key leaves the new key
/// the one that opens the folder.
///
/// It needs a WebDAV server with an empty folder at the address, which rclone gives:
///
/// ```text
/// rclone serve webdav "$(mktemp -d)" --addr 127.0.0.1:18080 --user ann --pass dav-test-pw
/// KAERI_TEST_WEBDAV=http://127.0.0.1:18080/ KAERI_TEST_WEBDAV_USER=ann \
///   KAERI_TEST_WEBDAV_PASSWORD=dav-test-pw \
///   cargo test -p kaeri-ffi --features testing --test sync_two -- --ignored webdav_server
/// ```
///
/// To run it again on the same server, make a new folder (`curl -u ann:dav-test-pw -X MKCOL
/// http://127.0.0.1:18080/again/`) and add it to `KAERI_TEST_WEBDAV`.
#[test]
#[ignore = "needs a WebDAV server: see the doc comment"]
fn removing_devices_on_a_webdav_server() {
    let (mut a, old) = set_up("Desk", webdav());
    let mut b = join("Laptop", webdav(), &old, 2);
    let mut c = join("Tablet", webdav(), &old, 3);
    let b_id = b.id();
    rounds(&mut [&mut a, &mut b, &mut c], "the three devices", |d| {
        d.store().sync_peers().unwrap().iter().filter(|p| !p.3).count() == 3
    });

    a.watch(FINISHED, &["1"]);
    b.watch(FINISHED, &["2"]);
    c.watch(FINISHED, &["3"]);
    rounds(&mut [&mut a, &mut b, &mut c], "each device's watch", |d| {
        d.watched(FINISHED) == ["1", "2", "3"]
    });

    a.send(Command::SetStatus { id: FINISHED, status: LibrarySegment::Watching });
    c.send(Command::SetSetting {
        key: SettingKey::SkipFiller,
        value: SettingValue::Bool(true),
    });
    thread::sleep(Duration::from_millis(500));
    rounds(&mut [&mut a, &mut b, &mut c], "a status and a setting", |d| {
        d.store().synced_status(FINISHED.get()).unwrap().as_deref() == Some("CURRENT")
            && d.config().contains("skip_filler = true")
    });

    // The same status and the same setting changed on two devices before either passes: the
    // later change wins everywhere.
    a.send(Command::SetStatus { id: AIRING, status: LibrarySegment::Paused });
    b.send(Command::SetSetting {
        key: SettingKey::SkipFiller,
        value: SettingValue::Bool(true),
    });
    thread::sleep(Duration::from_millis(400));
    b.send(Command::SetStatus { id: AIRING, status: LibrarySegment::Dropped });
    a.send(Command::SetSetting {
        key: SettingKey::SkipFiller,
        value: SettingValue::Bool(false),
    });
    thread::sleep(Duration::from_millis(500));
    rounds(&mut [&mut b, &mut a, &mut c], "the changes made on two devices", |d| {
        d.store().synced_status(AIRING.get()).unwrap().as_deref() == Some("DROPPED")
            && !d.config().contains("skip_filler = true")
    });

    // A removes B, which is online.
    a.send(Command::RemoveSyncDevice { device: b_id.clone() });
    let after = a.status_where("B gone", |s| s.devices.iter().all(|d| d.id != b_id));
    assert_eq!(after.devices.len(), 2, "{after:?}");
    let seen = b.pass();
    assert!(removed(&seen), "B is told it was removed: {seen:?}");
    a.watch(FINISHED, &["4"]);
    c.watch(FINISHED, &["5"]);
    b.watch(FINISHED, &["6"]);
    rounds(&mut [&mut a, &mut c], "A and C after B went", |d| {
        d.watched(FINISHED) == ["1", "2", "3", "4", "5"]
    });
    for _ in 0..3 {
        let seen = b.pass();
        assert!(removed(&seen), "B is told it was removed on every pass: {seen:?}");
    }
    assert_eq!(b.watched(FINISHED), ["1", "2", "3", "6"], "B received after its removal");
    for d in [&mut a, &mut c] {
        let s = d.pass();
        assert_eq!((s.error.as_deref(), s.devices.len()), (None, 2), "{s:?}");
        assert_eq!(d.watched(FINISHED), ["1", "2", "3", "4", "5"]);
        assert_eq!(
            d.store().synced_status(FINISHED.get()).unwrap().as_deref(),
            Some("CURRENT")
        );
        assert_eq!(d.store().synced_status(AIRING.get()).unwrap().as_deref(), Some("DROPPED"));
        assert!(!d.config().contains("skip_filler = true"));
    }

    // A makes a new recovery key, D joins with it, and C, which still holds the old one, removes
    // D while D is offline with a watch of its own.
    let fresh = new_recovery_key(&mut a);
    let mut d = join("Television", webdav(), &fresh, 3);
    let d_id = d.id();
    rounds(&mut [&mut a, &mut c, &mut d], "D in the group", |dev| {
        dev.watched(FINISHED) == ["1", "2", "3", "4", "5"]
            && dev.store().sync_peers().unwrap().iter().any(|p| p.0 == d_id && !p.3)
    });
    d.watch(FINISHED, &["7"]);
    c.send(Command::RemoveSyncDevice { device: d_id.clone() });
    c.status_where("D gone", |s| s.devices.iter().all(|x| x.id != d_id));
    c.watch(FINISHED, &["8"]);
    rounds(&mut [&mut c, &mut a], "A and C after D went", |dev| {
        dev.watched(FINISHED) == ["1", "2", "3", "4", "5", "8"]
            && dev.store().sync_peers().unwrap().iter().any(|p| p.0 == d_id && p.3)
    });
    // D comes back.
    let deadline = Instant::now() + PATIENCE;
    while !removed(&d.pass()) {
        assert!(Instant::now() < deadline, "D is never told it was removed");
    }
    assert!(removed(&d.pass()), "D is told on the next pass too");
    for dev in [&mut a, &mut c] {
        let s = dev.pass();
        assert_eq!((s.error.as_deref(), s.devices.len()), (None, 2), "{s:?}");
        assert!(!dev.watched(FINISHED).contains(&"7".to_owned()), "D's offline watch arrived");
    }
    assert!(matches!(try_join(webdav(), &old), Err(SyncProblem::WrongRecoveryKey)));
    assert_eq!(try_join(webdav(), &fresh), Ok(3));
}
