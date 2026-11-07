//! Optional profiles: a personal database per profile beside the device's.
//!
//! A one-person install has no `profiles.json`, one connection and none of this running. Adding a
//! second profile creates the file, a database under `profiles/<id>/`, and from then on the store
//! holds two connections: the device's (sources, downloads, caches) and the personal one (history,
//! choices, the tracker outbox, the sync log). The first profile's personal database is
//! `kaeri.db` itself, the same file the device connection has open, so nothing of today's install
//! moves.
//!
//! A SQLite authorizer on each connection refuses the other side's tables, so a method that takes
//! the wrong connection fails loudly in a test instead of showing one person's data to another.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::{
    Connection,
    hooks::{AuthAction, Authorization},
};
use serde::{Deserialize, Serialize};

use crate::{Result, Store, StoreError, schema};

/// Tables that belong to one person. Read and written through the personal connection.
pub const PERSONAL_TABLES: &[&str] = &[
    "watch_event",
    "watch_progress",
    "sync_outbox",
    "app_state",
    "title_index",
    "mapping_override",
    "provider_preference",
    "track_memory",
    "show_setting",
    "library_status",
    "episode_notice",
    "sync_meta",
    "sync_op",
    "sync_reg",
    "sync_tomb",
    "sync_unwatch",
    "sync_mark",
    "sync_peer",
    "sync_remote_download",
    "sync_note",
    "read_event",
    "read_adopt",
    "read_reset",
    "read_chapter",
    "read_work",
    "read_series_setting",
    "read_status",
    "read_bookmark",
    "read_sent",
    "read_outbox",
    "read_track_map",
    "read_notice",
    "read_mark",
    "read_unread",
    "read_note",
    "book_source_match",
    "book_edition_pref",
];

/// Tables that belong to the device and are shared by every profile.
pub const DEVICE_TABLES: &[&str] = &[
    "media_cache",
    "mapping",
    "episode_relation",
    "dataset_state",
    "mapping_resolution",
    "download",
    "download_dismissed",
    "server_health",
    "airing_time",
    "skip_marks",
    "skip_votes",
    "aniskip_outbox",
    "party_room",
    "dub_extent",
    "skip_prints",
    "skip_found",
    "skip_found_show",
    "book_work",
    "book_rhythm",
    "book_chapter_cache",
    "book_series_hint",
    "book_folder",
    "book_local_series",
    "book_local_file",
    "book_server",
    "book_download",
    "book_progress_outbox",
    "book_progress_state",
];

const FILE: &str = "profiles.json";

/// One profile, as `profiles.json` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub initials: String,
    #[serde(default)]
    pub kids: bool,
    /// Seconds since the epoch the profile was last switched to.
    #[serde(default)]
    pub last_used: i64,
}

/// `profiles.json`: the profiles in the order they were added, and which one is today's database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileList {
    /// The profile whose personal data is `kaeri.db` itself.
    pub primary: String,
    pub profiles: Vec<Profile>,
}

/// What deleting a profile did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deleted {
    /// The profile moved into the first profile's place, when the first was deleted.
    pub promoted: Option<String>,
    /// How many profiles are left; with one, the install is a single-user one again.
    pub remaining: usize,
}

/// The initials shown on a profile's circle: the first letters of up to two words.
pub fn initials_of(name: &str) -> String {
    let mut out: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    if out.is_empty() {
        out.push('?');
    }
    out
}

impl ProfileList {
    /// The list, or `None` for the one-profile install that has no file.
    pub fn load(data_dir: &Path) -> Result<Option<Self>> {
        let path = data_dir.join(FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(StoreError::DataDir { path: path.display().to_string(), source });
            }
        };
        let list: Self = serde_json::from_str(&text)
            .map_err(|e| StoreError::UnsupportedFormat(format!("{FILE}: {e}")))?;
        if list.profiles.iter().all(|p| p.id != list.primary) {
            return Err(StoreError::UnsupportedFormat(format!(
                "{FILE} names no first profile"
            )));
        }
        Ok(Some(list))
    }

    fn save(&self, data_dir: &Path) -> Result<()> {
        let path = data_dir.join(FILE);
        let json = serde_json::to_string_pretty(self)
            .map_err(|source| StoreError::Encode { what: "profiles", source })?;
        let tmp = data_dir.join(format!("{FILE}.tmp"));
        std::fs::write(&tmp, json)
            .and_then(|()| std::fs::rename(&tmp, &path))
            .map_err(|source| StoreError::DataDir { path: path.display().to_string(), source })
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// The directory a profile's own files are in (its database, `profile.toml`).
    pub fn dir(data_dir: &Path, id: &str) -> PathBuf {
        data_dir.join("profiles").join(id)
    }

    /// Where a profile's personal tables are: `kaeri.db` for the first, else under its directory.
    pub fn database(&self, data_dir: &Path, id: &str) -> PathBuf {
        if id == self.primary {
            data_dir.join("kaeri.db")
        } else {
            Self::dir(data_dir, id).join("kaeri.db")
        }
    }
}

/// Add a profile. The first call also creates `profiles.json`, with today's install named
/// `current_name` as the first profile; nothing else on disk moves. The new profile's database
/// starts empty.
pub fn add(
    data_dir: &Path,
    name: &str,
    kids: bool,
    current_name: &str,
    now: i64,
) -> Result<Profile> {
    let mut list = ProfileList::load(data_dir)?.unwrap_or_else(|| ProfileList {
        primary: "p1".into(),
        profiles: vec![Profile {
            id: "p1".into(),
            name: current_name.trim().to_owned(),
            initials: initials_of(current_name),
            kids: false,
            last_used: now,
        }],
    });
    let next = list
        .profiles
        .iter()
        .filter_map(|p| p.id.strip_prefix('p').and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0)
        + 1;
    let profile = Profile {
        id: format!("p{next}"),
        name: name.trim().to_owned(),
        initials: initials_of(name),
        kids,
        last_used: now,
    };
    let dir = ProfileList::dir(data_dir, &profile.id);
    std::fs::create_dir_all(&dir)
        .map_err(|source| StoreError::DataDir { path: dir.display().to_string(), source })?;
    schema::migrate(&Connection::open(dir.join("kaeri.db"))?)?;
    list.profiles.push(profile.clone());
    list.save(data_dir)?;
    Ok(profile)
}

/// Mark `id` as the profile used last, which is the one the next launch enters.
pub fn touch(data_dir: &Path, id: &str, now: i64) -> Result<()> {
    let mut list = ProfileList::load(data_dir)?.ok_or_else(|| missing(id))?;
    let profile = list.profiles.iter_mut().find(|p| p.id == id).ok_or_else(|| missing(id))?;
    profile.last_used = now;
    list.save(data_dir)
}

/// Rename a profile or change its kids switch.
pub fn edit(data_dir: &Path, id: &str, name: &str, kids: bool) -> Result<()> {
    let mut list = ProfileList::load(data_dir)?.ok_or_else(|| missing(id))?;
    let profile = list.profiles.iter_mut().find(|p| p.id == id).ok_or_else(|| missing(id))?;
    profile.name = name.trim().to_owned();
    profile.initials = initials_of(name);
    profile.kids = kids;
    list.save(data_dir)
}

fn missing(id: &str) -> StoreError {
    StoreError::Missing(format!("profile {id}"))
}

/// Refuse the tables of the other side on `conn`.
fn guard(conn: &Connection, denied: &'static [&'static str]) -> rusqlite::Result<()> {
    conn.authorizer(Some(move |ctx: rusqlite::hooks::AuthContext<'_>| {
        let table = match ctx.action {
            AuthAction::Read { table_name, .. }
            | AuthAction::Update { table_name, .. }
            | AuthAction::Insert { table_name }
            | AuthAction::Delete { table_name } => table_name,
            _ => return Authorization::Allow,
        };
        if ctx.database_name.is_none_or(|d| d == "main") && denied.contains(&table) {
            Authorization::Deny
        } else {
            Authorization::Allow
        }
    }))?;
    conn.flush_prepared_statement_cache();
    Ok(())
}

fn unguard(conn: &Connection) -> rusqlite::Result<()> {
    conn.authorizer(None::<fn(rusqlite::hooks::AuthContext<'_>) -> Authorization>)?;
    conn.flush_prepared_statement_cache();
    Ok(())
}

fn open_personal(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    schema::migrate(&conn)?;
    guard(&conn, DEVICE_TABLES)?;
    Ok(conn)
}

impl Store {
    /// Whether this store holds a second connection, which is when it has profiles.
    pub fn has_profiles(&self) -> bool {
        !Arc::ptr_eq(&self.conn, &self.personal_conn())
    }

    /// The profile in use, once profiles are on.
    pub fn active_profile(&self) -> Option<String> {
        self.active.lock().ok().and_then(|a| a.clone())
    }

    /// Make `id` the profile in use: open its personal database, install the guards, and swap it
    /// in for every clone of this store. The first call on an install that has just added a
    /// second profile is what gives the store its second connection.
    pub fn use_profile(&self, data_dir: &Path, id: &str, now: i64) -> Result<()> {
        let mut list = ProfileList::load(data_dir)?.ok_or_else(|| missing(id))?;
        if list.get(id).is_none() {
            return Err(missing(id));
        }
        let personal = open_personal(&list.database(data_dir, id))?;
        {
            let device = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
            guard(&device, PERSONAL_TABLES)?;
        }
        *self.personal.write().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Arc::new(Mutex::new(personal));
        *self.active.lock().map_err(|_| StoreError::Poisoned)? = Some(id.to_owned());
        // Mappings refreshed while another profile was in use are news to this one's outbox.
        self.wake_deferred()?;
        if let Some(p) = list.profiles.iter_mut().find(|p| p.id == id) {
            p.last_used = now;
        }
        list.save(data_dir)
    }

    /// Back to the one-connection store of a single-user install.
    fn drop_profiles(&self) -> Result<()> {
        {
            let device = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
            unguard(&device)?;
        }
        *self.personal.write().unwrap_or_else(std::sync::PoisonError::into_inner) =
            self.conn.clone();
        *self.active.lock().map_err(|_| StoreError::Poisoned)? = None;
        Ok(())
    }

    /// Delete a profile and its directory. The profile in use cannot be deleted: switch first.
    ///
    /// Deleting the first profile while others remain moves the profile in use into its place:
    /// its personal tables are copied into `kaeri.db` in one transaction. With one profile left
    /// `profiles.json` goes, and the install is again exactly a single-user one.
    pub fn delete_profile(&self, data_dir: &Path, id: &str) -> Result<Deleted> {
        let mut list = ProfileList::load(data_dir)?.ok_or_else(|| missing(id))?;
        if list.get(id).is_none() {
            return Err(missing(id));
        }
        let Some(active) = self.active_profile() else {
            return Err(StoreError::Profile("no profile is in use".into()));
        };
        if active == id {
            return Err(StoreError::Profile("the profile in use cannot be deleted".into()));
        }
        let mut promoted = None;
        if id == list.primary {
            let from = ProfileList::dir(data_dir, &active).join("kaeri.db");
            promote(&data_dir.join("kaeri.db"), &from)?;
            // The promoted profile's connection points at the directory about to go.
            list.primary.clone_from(&active);
            let personal = open_personal(&data_dir.join("kaeri.db"))?;
            *self.personal.write().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Arc::new(Mutex::new(personal));
            let _ = std::fs::remove_dir_all(ProfileList::dir(data_dir, &active));
            promoted = Some(active);
        } else {
            let _ = std::fs::remove_dir_all(ProfileList::dir(data_dir, id));
        }
        list.profiles.retain(|p| p.id != id);
        let remaining = list.profiles.len();
        if remaining <= 1 {
            let _ = std::fs::remove_file(data_dir.join(FILE));
            let _ = std::fs::remove_dir(data_dir.join("profiles"));
            self.drop_profiles()?;
        } else {
            list.save(data_dir)?;
        }
        Ok(Deleted { promoted, remaining })
    }
}

/// Copy the personal tables of the database at `from` over those of the one at `main`, in one
/// transaction.
fn promote(main: &Path, from: &Path) -> Result<()> {
    let mut conn = Connection::open(main)?;
    schema::migrate(&conn)?;
    conn.execute("ATTACH DATABASE ?1 AS promoted", [from.to_string_lossy().as_ref()])?;
    let copied = (|| -> rusqlite::Result<()> {
        let tx = conn.transaction()?;
        for table in PERSONAL_TABLES {
            tx.execute(&format!("DELETE FROM main.{table}"), [])?;
            tx.execute(
                &format!("INSERT INTO main.{table} SELECT * FROM promoted.{table}"),
                [],
            )?;
        }
        tx.execute("DELETE FROM main.sqlite_sequence", [])?;
        tx.execute(
            "INSERT INTO main.sqlite_sequence SELECT * FROM promoted.sqlite_sequence",
            [],
        )?;
        tx.commit()
    })();
    let _ = conn.execute("DETACH DATABASE promoted", []);
    Ok(copied?)
}

#[cfg(test)]
pub(crate) fn guard_for_test(
    device: &Connection,
    personal: &Connection,
) -> rusqlite::Result<()> {
    guard(device, PERSONAL_TABLES)?;
    guard(personal, DEVICE_TABLES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WatchEvent, now};
    use kaeri_core::ids::AnilistId;

    fn watch(id: u32, episode: &str) -> WatchEvent {
        WatchEvent {
            anilist_id: AnilistId::new(id),
            episode: episode.into(),
            position_secs: 300.0,
            duration_secs: Some(1400.0),
            watched_secs: 300.0,
            provider_id: None,
            translation: None,
            completed: false,
            at: 1_000,
        }
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, base, out);
                } else {
                    let name = p.strip_prefix(base).unwrap().display().to_string();
                    if !name.contains("-wal") && !name.contains("-shm") {
                        out.push(name);
                    }
                }
            }
        }
        walk(dir, dir, &mut out);
        out.sort();
        out
    }

    #[test]
    fn every_table_belongs_to_the_device_or_a_person() {
        let store = Store::open_in_memory().unwrap();
        let tables: Vec<String> = store
            .with_conn(|c| {
                let mut stmt = c.prepare(
                    "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                )?;
                Ok(stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
            })
            .unwrap();
        for table in tables {
            let personal = PERSONAL_TABLES.contains(&table.as_str());
            let device = DEVICE_TABLES.contains(&table.as_str());
            assert!(
                personal != device || table == "schema_migration",
                "{table} must be listed as exactly one of personal or device"
            );
        }
    }

    #[test]
    fn a_single_user_install_has_no_profile_files_and_one_connection() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("kaeri.db")).unwrap();
        store.record_event(&watch(1, "1")).unwrap();
        assert!(!store.has_profiles());
        assert_eq!(store.active_profile(), None);
        assert_eq!(ProfileList::load(dir.path()).unwrap(), None);
        assert!(!dir.path().join("profiles").exists());
    }

    #[test]
    fn a_second_profile_has_its_own_history_and_the_first_keeps_today_s() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("kaeri.db")).unwrap();
        store.record_event(&watch(1, "1")).unwrap();
        store.enqueue_download(AnilistId::new(1), "1", "Show", "magnet:?xt=urn:x").unwrap();

        let aki = add(dir.path(), "Aki", false, "Mika", now()).unwrap();
        assert_eq!(aki.initials, "A");
        // Adding alone changes nothing about the running store.
        assert!(!store.has_profiles());
        store.use_profile(dir.path(), &aki.id, now()).unwrap();
        assert!(store.has_profiles());
        assert_eq!(store.active_profile().as_deref(), Some("p2"));

        assert!(store.progress(AnilistId::new(1)).unwrap().is_none(), "Aki starts empty");
        assert_eq!(store.downloads().unwrap().len(), 1, "downloads are shared");
        store.record_event(&watch(2, "4")).unwrap();

        store.use_profile(dir.path(), "p1", now()).unwrap();
        assert!(store.progress(AnilistId::new(1)).unwrap().is_some());
        assert!(store.progress(AnilistId::new(2)).unwrap().is_none(), "Mika never sees Aki's");

        // A clone taken before the switch follows it.
        let clone = store.clone();
        store.use_profile(dir.path(), "p2", now()).unwrap();
        assert!(clone.progress(AnilistId::new(2)).unwrap().is_some());
    }

    #[test]
    fn the_wrong_connection_is_refused_in_both_directions() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("kaeri.db")).unwrap();
        let aki = add(dir.path(), "Aki", false, "Mika", now()).unwrap();
        store.use_profile(dir.path(), &aki.id, now()).unwrap();
        let on_device =
            store.with_conn(|c| Ok(c.prepare("SELECT 1 FROM watch_event").map(|_| ())?));
        assert!(on_device.is_err());
        let on_person =
            store.with_personal(|c| Ok(c.prepare("SELECT 1 FROM download").map(|_| ())?));
        assert!(on_person.is_err());
    }

    #[test]
    fn deleting_back_to_one_leaves_todays_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("kaeri.db")).unwrap();
        store.record_event(&watch(1, "1")).unwrap();
        let before = files(dir.path());

        let aki = add(dir.path(), "Aki", false, "Mika", now()).unwrap();
        store.use_profile(dir.path(), &aki.id, now()).unwrap();
        store.record_event(&watch(2, "1")).unwrap();
        store.use_profile(dir.path(), "p1", now()).unwrap();

        let out = store.delete_profile(dir.path(), &aki.id).unwrap();
        assert_eq!(out, Deleted { promoted: None, remaining: 1 });
        assert_eq!(files(dir.path()), before, "the same files in the same places");
        assert!(!store.has_profiles());
        assert!(store.progress(AnilistId::new(1)).unwrap().is_some());
    }

    #[test]
    fn deleting_the_first_profile_moves_the_other_into_its_place() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("kaeri.db")).unwrap();
        store.record_event(&watch(1, "1")).unwrap();
        let before = files(dir.path());
        let aki = add(dir.path(), "Aki", false, "Mika", now()).unwrap();
        store.use_profile(dir.path(), &aki.id, now()).unwrap();
        store.record_event(&watch(2, "9")).unwrap();

        // The profile in use cannot go; the first can, with Aki in use.
        assert!(store.delete_profile(dir.path(), &aki.id).is_err());
        let out = store.delete_profile(dir.path(), "p1").unwrap();
        assert_eq!(out, Deleted { promoted: Some("p2".into()), remaining: 1 });
        assert_eq!(files(dir.path()), before);
        assert!(!store.has_profiles());
        assert!(store.progress(AnilistId::new(1)).unwrap().is_none(), "Mika's history is gone");
        let aki_progress = store.progress(AnilistId::new(2)).unwrap().expect("Aki's moved in");
        assert_eq!(aki_progress.last_episode, "9");

        // A reopened store reads the same.
        drop(store);
        let again = Store::open(dir.path().join("kaeri.db")).unwrap();
        assert!(again.progress(AnilistId::new(2)).unwrap().is_some());
    }

    #[test]
    fn with_three_profiles_deleting_the_first_keeps_the_picker() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("kaeri.db")).unwrap();
        let aki = add(dir.path(), "Aki", false, "Mika", now()).unwrap();
        let _jun = add(dir.path(), "Jun", true, "Mika", now()).unwrap();
        store.use_profile(dir.path(), &aki.id, now()).unwrap();
        let out = store.delete_profile(dir.path(), "p1").unwrap();
        assert_eq!(out.remaining, 2);
        let list = ProfileList::load(dir.path()).unwrap().unwrap();
        assert_eq!(list.primary, "p2");
        assert!(store.has_profiles());
        assert!(list.get("p3").unwrap().kids);
    }

    #[test]
    fn initials_come_from_the_first_two_words() {
        assert_eq!(initials_of("study mac"), "SM");
        assert_eq!(initials_of("Aki"), "A");
        assert_eq!(initials_of("  "), "?");
    }
}
