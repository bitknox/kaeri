//! Two engines on one database, as when the Mac app and the terminal share a data folder: each
//! test runs a second process of this test binary against the same file. A write that meets the
//! other process's lock waits for it rather than fail, so no change is lost.

use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};

use kaeri_store::{Store, reading::ReadEvent};

const WORK: &str = "a:990001";
const MARKS: u32 = 40;

/// This test binary again, running only `test` with `role` set to the database's path.
fn second_engine(test: &str, role: &str, db: &Path) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads", "1"])
        .env(role, db)
        .spawn()
        .unwrap()
}

fn wait_for(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(start.elapsed() < Duration::from_secs(30), "the second engine never got going");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn mark(store: &Store, chapter: &str) -> kaeri_store::Result<()> {
    let mut event = ReadEvent::new(WORK, chapter, 1_000);
    event.completed = true;
    store.record_read(&event, Vec::new())
}

const HOLDER: &str = "KAERI_TWO_ENGINES_HOLDER";

/// The other engine holds the write lock for a second, as a long write does. A reading mark made
/// meanwhile waits for it and is kept.
#[test]
fn a_mark_made_while_the_other_engine_writes_waits_and_is_kept() {
    if let Some(db) = std::env::var_os(HOLDER) {
        let db = PathBuf::from(db);
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        std::fs::write(db.with_extension("held"), b"").unwrap();
        std::thread::sleep(Duration::from_millis(1_500));
        conn.execute_batch("COMMIT").unwrap();
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("kaeri.db");
    let store = Store::open(&db).unwrap();
    let mut other = second_engine(
        "a_mark_made_while_the_other_engine_writes_waits_and_is_kept",
        HOLDER,
        &db,
    );
    wait_for(&db.with_extension("held"));

    let asked = Instant::now();
    mark(&store, "1").expect("the mark waits for the lock rather than fail");
    assert!(
        asked.elapsed() >= Duration::from_millis(500),
        "it did wait: {:?}",
        asked.elapsed()
    );
    assert!(other.wait().unwrap().success());
    assert!(store.read_marks(WORK).unwrap().is_read("1"));
}

const MARKER: &str = "KAERI_TWO_ENGINES_MARKER";

/// Both engines mark chapters in turn, as fast as they can: every mark from both is kept.
#[test]
fn two_engines_marking_at_once_lose_no_mark() {
    if let Some(db) = std::env::var_os(MARKER) {
        let db = PathBuf::from(db);
        let store = Store::open(&db).unwrap();
        std::fs::write(db.with_extension("ready"), b"").unwrap();
        for n in 0..MARKS {
            mark(&store, &format!("other-{n}")).unwrap();
        }
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("kaeri.db");
    let store = Store::open(&db).unwrap();
    let mut other = second_engine("two_engines_marking_at_once_lose_no_mark", MARKER, &db);
    wait_for(&db.with_extension("ready"));
    for n in 0..MARKS {
        mark(&store, &format!("here-{n}")).unwrap();
    }
    assert!(other.wait().unwrap().success(), "every mark the other engine made was kept");

    let marks = store.read_marks(WORK).unwrap();
    for n in 0..MARKS {
        assert!(marks.is_read(&format!("here-{n}")), "here-{n}");
        assert!(marks.is_read(&format!("other-{n}")), "other-{n}");
    }
}
