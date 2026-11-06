//! Two devices in one sync group, end to end through the engine as the apps run it: what one
//! does reaches the other through the shared folder, both settle a change made on each the same
//! way, and a device removed from the group stops receiving. The road's own rules are tested in
//! kaeri-sync and the merge rules in kaeri-store; this is the engine between them.
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
    group::{SyncFolderKind, SyncFolderSpec, SyncStatus},
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

    /// A pass through the folder, waited out.
    fn pass(&mut self) -> SyncStatus {
        self.send(Command::SyncFolderNow);
        self.status_where("a pass", |s| !s.syncing && s.last_synced.is_some())
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

/// Two devices in a group through a shared folder: the first sets it up, the second joins with
/// the recovery key.
fn group(shared: &std::path::Path) -> (Device, Device) {
    let mut desk = Device::start();
    desk.send(Command::StartSync { device_name: "Desk".into(), folder: folder(shared) });
    let key = {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if let Event::SyncRecoveryKey(key) =
                desk.events.recv_timeout(left).expect("the recovery key")
            {
                break key;
            }
        }
    };
    desk.status_where("sync on", |s| s.enabled && s.last_synced.is_some());
    let mut laptop = Device::start();
    laptop.send(Command::JoinSync {
        device_name: "Laptop".into(),
        folder: folder(shared),
        recovery_key: Secret(key),
    });
    laptop.status_where("both devices", |s| s.enabled && s.devices.len() == 2);
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
