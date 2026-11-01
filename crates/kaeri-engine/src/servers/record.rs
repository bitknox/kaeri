//! The saved servers, as the database keeps them.

use kaeri_core::ids::{AnidbId, AnilistId, ExternalIds, TmdbId, TvdbId};
use kaeri_jellyfin::{SeriesMatch, model::Session};
use kaeri_model::servers::{CheckedServer, LibraryChoice, ServerRow};
use kaeri_store::Store;
use serde::{Deserialize, Serialize};

const KEY: &str = "servers:list";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub name: String,
    pub address: String,
    pub version: String,
    pub user_id: String,
    pub user_name: String,
    #[serde(default)]
    pub libraries: Vec<LibraryRecord>,
    /// Whether the person has chosen libraries yet. Until then none is read.
    #[serde(default)]
    pub libraries_chosen: bool,
    #[serde(default = "yes")]
    pub report: bool,
    #[serde(default)]
    pub signed_out: bool,
    /// Signed out here, with Sign out, rather than by the server.
    #[serde(default)]
    pub signed_out_here: bool,
    #[serde(default)]
    pub series: u32,
    #[serde(default)]
    pub found: Vec<Found>,
    /// When the server last answered, in Unix seconds.
    #[serde(default)]
    pub last_seen: Option<i64>,
    /// The server did not answer the last time it was asked.
    #[serde(default)]
    pub not_answering: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    pub ticked: bool,
}

/// One series and the AniList entry it was matched to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Found {
    pub series: String,
    pub name: String,
    pub anilist: u32,
    #[serde(default)]
    pub anidb: Option<u32>,
    #[serde(default)]
    pub tvdb: Option<u32>,
    #[serde(default)]
    pub tmdb: Option<u32>,
    #[serde(default)]
    pub season: Option<u32>,
}

impl Record {
    pub fn new(checked: &CheckedServer, session: &Session) -> Self {
        Self {
            id: checked.id.clone(),
            name: checked.name.clone(),
            address: checked.address.clone(),
            version: checked.version.clone(),
            user_id: session.user_id.clone(),
            user_name: session.user_name.clone(),
            libraries: Vec::new(),
            libraries_chosen: false,
            report: true,
            signed_out: false,
            signed_out_here: false,
            series: 0,
            found: Vec::new(),
            last_seen: None,
            not_answering: false,
        }
    }

    /// Note what became of asking the server: it answered at `now`, or it did not answer.
    pub fn heard(&mut self, answered: bool, now: i64) {
        if answered {
            self.last_seen = Some(now);
        }
        self.not_answering = !answered;
    }

    pub fn matches(&self) -> Vec<SeriesMatch> {
        self.found
            .iter()
            .map(|f| SeriesMatch {
                series: f.series.clone(),
                name: f.name.clone(),
                anilist: AnilistId::new(f.anilist),
                ids: ExternalIds {
                    anilist: Some(AnilistId::new(f.anilist)),
                    mal: None,
                    anidb: f.anidb.map(AnidbId::new),
                    tvdb: f.tvdb.map(TvdbId::new),
                    tmdb: f.tmdb.map(TmdbId::new),
                },
                season: f.season,
            })
            .collect()
    }

    pub fn set_matches(&mut self, matches: Vec<SeriesMatch>, series: u32) {
        self.series = series;
        self.signed_out = false;
        self.signed_out_here = false;
        self.found = matches
            .into_iter()
            .map(|m| Found {
                series: m.series,
                name: m.name,
                anilist: m.anilist.get(),
                anidb: m.ids.anidb.map(|i| i.get()),
                tvdb: m.ids.tvdb.map(|i| i.get()),
                tmdb: m.ids.tmdb.map(|i| i.get()),
                season: m.season,
            })
            .collect();
    }

    pub fn row(&self) -> ServerRow {
        ServerRow {
            id: self.id.clone(),
            name: self.name.clone(),
            address: self.address.clone(),
            version: self.version.clone(),
            user: self.user_name.clone(),
            libraries: self
                .libraries
                .iter()
                .map(|l| LibraryChoice {
                    id: l.id.clone(),
                    name: l.name.clone(),
                    kind: l.kind.clone(),
                    ticked: l.ticked,
                })
                .collect(),
            series: self.series,
            matched: self.found.len() as u32,
            report: self.report,
            signed_out: self.signed_out,
            signed_out_here: self.signed_out_here,
            matching: None,
            not_answering: self.not_answering,
            last_seen: self.last_seen,
        }
    }
}

pub fn load(store: &Store) -> Vec<Record> {
    store
        .get_meta_text(KEY)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(store: &Store, records: &[Record]) {
    if let Ok(text) = serde_json::to_string(records) {
        let _ = store.set_meta_text(KEY, &text);
    }
}

/// Add a server, or replace the one with the same id and keep what was chosen on it.
pub fn upsert(store: &Store, mut record: Record) {
    let mut records = load(store);
    match records.iter_mut().find(|r| r.id == record.id) {
        Some(old) => {
            record.libraries = std::mem::take(&mut old.libraries);
            record.libraries_chosen = old.libraries_chosen;
            record.report = old.report;
            record.found = std::mem::take(&mut old.found);
            record.series = old.series;
            *old = record;
        }
        None => records.push(record),
    }
    save(store, &records);
}

pub fn update(store: &Store, id: &str, change: impl FnOnce(&mut Record)) {
    let mut records = load(store);
    if let Some(record) = records.iter_mut().find(|r| r.id == id) {
        change(record);
        save(store, &records);
    }
}

/// Take a server out of the list, and return it.
pub fn remove(store: &Store, id: &str) -> Option<Record> {
    let mut records = load(store);
    let at = records.iter().position(|r| r.id == id)?;
    let record = records.remove(at);
    save(store, &records);
    Some(record)
}

const HELD: &str = "servers:removed";
const PINS: &str = "servers:pins";

fn read<T: serde::de::DeserializeOwned + Default>(store: &Store, key: &str) -> T {
    store
        .get_meta_text(key)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write<T: Serialize>(store: &Store, key: &str, value: &T) {
    if let Ok(text) = serde_json::to_string(value) {
        let _ = store.set_meta_text(key, &text);
    }
}

/// Servers removed while their Undo notice shows. Their tokens are still in the secure store.
pub fn held(store: &Store) -> Vec<Record> {
    read(store, HELD)
}

pub fn hold(store: &Store, record: Record) {
    let mut held = held(store);
    held.retain(|r| r.id != record.id);
    held.push(record);
    write(store, HELD, &held);
}

/// Take a held server back out, for Undo or to forget it.
pub fn unhold(store: &Store, id: &str) -> Option<Record> {
    let mut held = held(store);
    let at = held.iter().position(|r| r.id == id)?;
    let record = held.remove(at);
    write(store, HELD, &held);
    Some(record)
}

/// The certificate fingerprints the person allowed, by the address they were allowed for.
pub fn pins(store: &Store) -> std::collections::BTreeMap<String, String> {
    read(store, PINS)
}

pub fn set_pin(store: &Store, address: &str, fingerprint: Option<String>) {
    let mut pins = pins(store);
    match fingerprint {
        Some(fingerprint) => pins.insert(address.to_owned(), fingerprint),
        None => pins.remove(address),
    };
    write(store, PINS, &pins);
}

/// Mika's Jellyfin as a scene shows it: signed in, the anime library read, three series and two
/// of them matched. Made-up; there is no server behind the address.
pub fn seed_fixture(store: &Store) {
    let checked = CheckedServer {
        id: "0f1e2d3c".into(),
        name: "Mika's Jellyfin".into(),
        version: "10.9.11".into(),
        address: "http://192.168.1.20:8096".into(),
        quick_connect: true,
    };
    let session =
        Session { token: String::new(), user_id: "u-mika".into(), user_name: "mika".into() };
    let mut record = Record::new(&checked, &session);
    record.libraries = vec![
        LibraryRecord {
            id: "lib-anime".into(),
            name: "Anime Shows".into(),
            kind: Some("tvshows".into()),
            ticked: true,
        },
        LibraryRecord {
            id: "lib-films".into(),
            name: "Films".into(),
            kind: Some("movies".into()),
            ticked: false,
        },
    ];
    record.libraries_chosen = true;
    record.series = 3;
    record.found = vec![
        Found {
            series: "s-1".into(),
            name: "Stardust Wayfarer".into(),
            anilist: 900_001,
            anidb: None,
            tvdb: None,
            tmdb: None,
            season: None,
        },
        Found {
            series: "s-2".into(),
            name: "Lantern Harbour".into(),
            anilist: 900_002,
            anidb: Some(424_242),
            tvdb: None,
            tmdb: None,
            season: None,
        },
    ];
    upsert(store, record);
}

#[cfg(feature = "testing")]
pub(crate) const FIXTURE_MATCHING: &str = "servers:fixture-matching";

/// For a scene, beside [`seed_fixture`]: Mika's Jellyfin part way through matching, "Matching ·
/// 12 of 84 series", a second server, Studio server, that signed this device out, and a third,
/// Attic server, that stopped answering two hours before `now`: "Not answering · last seen" and
/// that time. Made-up; there is no server behind any address. Only in a build with the
/// `testing` feature.
#[cfg(feature = "testing")]
pub fn seed_fixture_busy(store: &Store, now: i64) {
    let _ = store.set_meta_text(FIXTURE_MATCHING, "0f1e2d3c|12,84");
    let checked = CheckedServer {
        id: "5a6b7c8d".into(),
        name: "Studio server".into(),
        version: "10.10.3".into(),
        address: "https://studio.mika.example".into(),
        quick_connect: false,
    };
    let session =
        Session { token: String::new(), user_id: "u-studio".into(), user_name: "mika".into() };
    let mut record = Record::new(&checked, &session);
    record.libraries_chosen = true;
    record.signed_out = true;
    upsert(store, record);

    let checked = CheckedServer {
        id: "9e8f7a6b".into(),
        name: "Attic server".into(),
        version: "10.9.11".into(),
        address: "https://attic.mika.example".into(),
        quick_connect: false,
    };
    let session =
        Session { token: String::new(), user_id: "u-attic".into(), user_name: "mika".into() };
    let mut record = Record::new(&checked, &session);
    record.libraries_chosen = true;
    record.last_seen = Some(now - 2 * 3_600);
    record.not_answering = true;
    upsert(store, record);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_kept_before_last_seen_reads_back_as_never_seen() {
        let kept = r#"[{"id":"a1","name":"Mika's Jellyfin","address":"http://192.168.1.20:8096",
            "version":"10.9.11","user_id":"u","user_name":"mika"}]"#;
        let records: Vec<Record> = serde_json::from_str(kept).unwrap();
        assert_eq!(records[0].last_seen, None);
        assert!(!records[0].not_answering);
    }

    #[test]
    fn a_server_that_stops_answering_keeps_when_it_was_last_seen() {
        let checked = CheckedServer {
            id: "a1".into(),
            name: "Mika's Jellyfin".into(),
            version: "10.9.11".into(),
            address: "http://192.168.1.20:8096".into(),
            quick_connect: false,
        };
        let session =
            Session { token: String::new(), user_id: "u".into(), user_name: "mika".into() };
        let mut record = Record::new(&checked, &session);
        record.heard(true, 1_000);
        assert_eq!((record.row().not_answering, record.row().last_seen), (false, Some(1_000)));
        record.heard(false, 2_000);
        assert_eq!((record.row().not_answering, record.row().last_seen), (true, Some(1_000)));
        let read: Record =
            serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
        assert_eq!((read.not_answering, read.last_seen), (true, Some(1_000)));
        record.heard(true, 3_000);
        assert_eq!((record.not_answering, record.last_seen), (false, Some(3_000)));
    }
}
