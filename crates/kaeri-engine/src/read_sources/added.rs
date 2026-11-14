//! What came in after a book server or a folder was added, and the servers found and not yet
//! added: see [`kaeri_model::came_in`].
//!
//! A server is signed in to first ([`BookSources::probe`]), which lists its libraries and counts
//! its series and holds the sign-in in memory. Adding it ([`BookSources::add_found`]) writes the
//! sign-in to the secure store and keeps the libraries picked in the store's meta, under
//! `reading.server.<id>`. A server that showed no series is looked at again every
//! [`RECHECK`], and on refresh.

use std::{collections::HashMap, time::Duration};

use kaeri_books::library::Scan;
use kaeri_bookserver::{KavitaLink, Kind, ServerError};
use kaeri_model::{
    book_sources::BookServerKind,
    came_in::{
        AddedServer, CameIn, FolderReport, FormatCount, NothingCameIn, ServerFound,
        ServerLibrary,
    },
};
use kaeri_store::book_sources::{BookServer, Folder};
use serde::{Deserialize, Serialize};

use super::{BookSources, kind_of, secret_key, short_id};

/// How long an empty server waits before Kaeri looks at it again.
pub const RECHECK: Duration = Duration::from_secs(30 * 60);
const META_SERVER: &str = "reading.server.";
const META_FOLDER: &str = "reading.folder_report.";

/// What the store keeps of a server beside its row: the libraries picked, and what it held
/// when last asked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ServerMeta {
    /// The library ids asked; empty asks every one.
    #[serde(default)]
    pub libraries: Vec<String>,
    /// How many libraries were added: the picked ones, or all of them.
    #[serde(default)]
    pub library_count: u32,
    #[serde(default)]
    pub series: Option<u32>,
    /// It showed no series when last asked.
    #[serde(default)]
    pub empty: bool,
    /// When it was last asked, as a Unix time.
    #[serde(default)]
    pub checked_at: Option<i64>,
    /// The last four characters of a Kavita key it signs in with.
    #[serde(default)]
    pub key_last4: Option<String>,
}

impl ServerMeta {
    /// When an empty server is looked at again.
    pub fn rechecks_at(&self) -> Option<i64> {
        self.empty.then(|| self.checked_at.unwrap_or(0) + RECHECK.as_secs() as i64)
    }
}

/// What a folder's last scan reported beside its series.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FolderMeta {
    #[serde(default)]
    pub files: u32,
    #[serde(default)]
    pub skipped: u32,
}

/// A server signed in to and not added yet. Its sign-in lives here, in memory, until then.
#[derive(Clone)]
pub(crate) struct Pending {
    pub kind: Kind,
    pub name: String,
    pub address: String,
    pub user: Option<String>,
    pub credential: String,
    pub series: Option<u32>,
    pub libraries: Vec<kaeri_bookserver::Library>,
    pub key_last4: Option<String>,
}

impl std::fmt::Debug for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pending")
            .field("kind", &self.kind)
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

/// What a server is called in Your sources: the kind, as its sign-in says.
fn host(address: &str) -> String {
    kaeri_bookserver::host_of(address)
}

/// Why a sign-in or an address check came to nothing, in the words the add sheet uses.
pub(crate) fn nothing_for(
    error: &ServerError,
    address: &str,
    kind: Kind,
    key: bool,
) -> NothingCameIn {
    let host = host(address);
    match error {
        ServerError::Unauthorized if key => NothingCameIn::WrongKey { host },
        ServerError::Unauthorized => NothingCameIn::WrongPassword { host, kind: kind_of(kind) },
        ServerError::NotABookServer => NothingCameIn::NotABookServer { host },
        _ => NothingCameIn::Unreachable { host },
    }
}

/// The id a server is kept under: the same on every device for the same address and account.
/// How many libraries a server's row names. An OPDS catalogue keeps none of its own but is one:
/// "OPDS · 38 series in 1 library".
pub(crate) fn row_libraries(kind: BookServerKind, count: u32) -> u32 {
    if kind == BookServerKind::Opds { count.max(1) } else { count }
}

pub(crate) fn server_id(address: &str, user: Option<&str>) -> String {
    format!("srv-{}", short_id(&format!("{address}\u{1f}{}", user.unwrap_or("key"))))
}

/// A folder's report, from its scan.
pub(crate) fn folder_report(folder: &Folder, scan: &Scan) -> FolderReport {
    let books = scan
        .series
        .iter()
        .map(|s| s.files.iter().filter(|f| f.broken.is_none()).count())
        .sum::<usize>();
    // Series whose books name them, rather than their folder.
    let matched = scan
        .series
        .iter()
        .filter(|s| {
            s.files
                .iter()
                .any(|f| f.own_series.as_deref().is_some_and(|o| !o.trim().is_empty()))
        })
        .count();
    let mut locked: Vec<FormatCount> = Vec::new();
    for file in scan.series.iter().flat_map(|s| &s.files).filter(|f| f.locked) {
        let format = file.format.to_ascii_uppercase();
        match locked.iter_mut().find(|f| f.format == format) {
            Some(f) => f.count += 1,
            None => locked.push(FormatCount { format, count: 1 }),
        }
    }
    let readable =
        scan.series.iter().filter(|s| s.files.iter().any(|f| f.broken.is_none())).count();
    FolderReport {
        id: folder.id.clone(),
        name: folder.name.clone().or_else(|| folder.path.clone()).unwrap_or_default(),
        path: folder.path.clone(),
        series: u32::try_from(readable).unwrap_or(u32::MAX),
        books: u32::try_from(books).unwrap_or(u32::MAX),
        files: scan.seen,
        matched: u32::try_from(matched).unwrap_or(u32::MAX),
        unsupported: scan
            .unsupported
            .iter()
            .map(|(format, count)| FormatCount { format: format.clone(), count: *count })
            .collect(),
        locked,
        listed: scan.unsupported_files.clone(),
        already_added: false,
    }
}

/// What came in from a folder: its report, or why nothing did.
pub(crate) fn folder_came_in(report: FolderReport) -> CameIn {
    if report.series > 0 {
        CameIn::Folder(report)
    } else if report.skipped() > 0 {
        CameIn::Empty(NothingCameIn::UnsupportedFiles { folder: report })
    } else {
        CameIn::Empty(NothingCameIn::EmptyFolder { folder: report })
    }
}

impl BookSources {
    fn pending(&self) -> std::sync::MutexGuard<'_, HashMap<String, Pending>> {
        self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn server_meta(&self, id: &str) -> ServerMeta {
        self.store
            .get_meta_text(&format!("{META_SERVER}{id}"))
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn keep_server_meta(&self, id: &str, meta: &ServerMeta) {
        let text = serde_json::to_string(meta).unwrap_or_default();
        if let Err(e) = self.store.set_meta_text(&format!("{META_SERVER}{id}"), &text) {
            tracing::warn!(error = %e, "what the server holds was not kept");
        }
    }

    pub(crate) fn forget_server_meta(&self, id: &str) {
        let _ = self.store.delete_meta(&format!("{META_SERVER}{id}"));
    }

    pub(crate) fn folder_meta(&self, id: &str) -> FolderMeta {
        self.store
            .get_meta_text(&format!("{META_FOLDER}{id}"))
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub(crate) fn keep_folder_meta(&self, report: &FolderReport) {
        let meta = FolderMeta { files: report.files, skipped: report.skipped() };
        let text = serde_json::to_string(&meta).unwrap_or_default();
        let _ = self.store.set_meta_text(&format!("{META_FOLDER}{}", report.id), &text);
    }

    /// Sign in to a server and say what it holds, holding the sign-in until it is added.
    /// `address` may be a Kavita OPDS link, whose key signs in on its own.
    pub async fn probe(
        &self,
        address: &str,
        user: Option<&str>,
        password: Option<&str>,
    ) -> CameIn {
        if let Some(link) = kaeri_bookserver::kavita_link(address) {
            return self.probe_key(&link).await;
        }
        let found = match kaeri_bookserver::check(&self.servers, address).await {
            Ok(found) => found,
            Err(e) => {
                return CameIn::Empty(nothing_for(&e, address, Kind::Opds, false));
            }
        };
        let user = user.unwrap_or_default();
        let done = match kaeri_bookserver::sign_in(
            &self.servers,
            &found.address,
            found.kind,
            user,
            password.unwrap_or_default(),
        )
        .await
        {
            Ok(done) => done,
            Err(e) => {
                let kind = found.kind.unwrap_or(Kind::Opds);
                return CameIn::Empty(nothing_for(&e, &found.address, kind, false));
            }
        };
        let user = (!user.is_empty()).then(|| user.to_owned());
        self.hold(found.address, user, done, None).await
    }

    async fn probe_key(&self, link: &KavitaLink) -> CameIn {
        match kaeri_bookserver::sign_in_with_key(&self.servers, link).await {
            Ok(done) => {
                let last4 = kaeri_bookserver::last_four(&link.key);
                self.hold(link.root.clone(), None, done, Some(last4)).await
            }
            Err(e) => CameIn::Empty(nothing_for(&e, &link.root, Kind::Kavita, true)),
        }
    }

    /// Hold a sign-in, list the server's libraries, and say what was found.
    async fn hold(
        &self,
        address: String,
        user: Option<String>,
        done: kaeri_bookserver::SignedIn,
        key_last4: Option<String>,
    ) -> CameIn {
        let libraries =
            kaeri_bookserver::libraries(&self.servers, &address, done.kind, &done.credential)
                .await
                .unwrap_or_else(|e| {
                    tracing::debug!(error = %e, "the server's libraries were not listed");
                    Vec::new()
                });
        let pending_id = server_id(&address, user.as_deref());
        let found = ServerFound {
            pending: pending_id.clone(),
            kind: kind_of(done.kind),
            name: done.name.clone(),
            host: host(&address),
            address: address.clone(),
            series: done.series,
            libraries: libraries
                .iter()
                .map(|l| ServerLibrary {
                    id: l.id.clone(),
                    name: l.name.clone(),
                    series: l.series,
                })
                .collect(),
            key_last4: key_last4.clone(),
        };
        self.pending().insert(
            pending_id,
            Pending {
                kind: done.kind,
                name: done.name,
                address,
                user,
                credential: done.credential,
                series: done.series,
                libraries,
                key_last4,
            },
        );
        if done.series == Some(0) {
            CameIn::Empty(NothingCameIn::EmptyCatalogue { found })
        } else {
            CameIn::Found(found)
        }
    }

    /// Add a server [`Self::probe`] found, asking only `libraries` (all of them when empty).
    pub fn add_found(
        &self,
        pending: &str,
        libraries: Vec<String>,
        report_progress: bool,
        now: i64,
    ) -> Option<CameIn> {
        let held = self.pending().remove(pending)?;
        // Only libraries the server listed; all of them when none or every one is picked.
        let known: Vec<&str> = held.libraries.iter().map(|l| l.id.as_str()).collect();
        let mut picked: Vec<String> =
            libraries.into_iter().filter(|l| known.contains(&l.as_str())).collect();
        if picked.len() == known.len() {
            picked.clear();
        }
        let series = if picked.is_empty() {
            held.series
        } else {
            held.libraries
                .iter()
                .filter(|l| picked.contains(&l.id))
                .map(|l| l.series)
                .sum::<Option<u32>>()
        };
        let count = if picked.is_empty() { known.len() } else { picked.len() };
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        let host = host(&held.address);
        Some(match self.add_server(held, picked, count, series, report_progress, now) {
            Ok(added) => CameIn::ServerAdded(added),
            Err(()) => CameIn::Empty(NothingCameIn::Unreachable { host }),
        })
    }

    /// Keep a server: its sign-in in the secure store, its row, and what it holds.
    fn add_server(
        &self,
        held: Pending,
        picked: Vec<String>,
        library_count: u32,
        series: Option<u32>,
        report_progress: bool,
        now: i64,
    ) -> Result<AddedServer, ()> {
        let id = server_id(&held.address, held.user.as_deref());
        if self.tokens.set(&secret_key(&id), &held.credential).is_err() {
            return Err(());
        }
        let row = BookServer {
            id: id.clone(),
            kind: held.kind.as_str().into(),
            name: held.name.clone(),
            address: held.address.clone(),
            user: held.user.clone(),
            report_progress,
            signed_out: false,
            last_seen: Some(now),
        };
        if self.store.put_book_server(&row).is_err() {
            let _ = self.tokens.clear(&secret_key(&id));
            return Err(());
        }
        let meta = ServerMeta {
            libraries: picked,
            library_count,
            series,
            empty: series == Some(0),
            checked_at: Some(now),
            key_last4: held.key_last4.clone(),
        };
        self.keep_server_meta(&id, &meta);
        self.put_server(&row);
        Ok(AddedServer {
            id,
            kind: kind_of(held.kind),
            name: held.name,
            host: host(&held.address),
            series,
            libraries: meta.library_count,
            empty: meta.empty,
        })
    }

    /// Look again at the servers that showed no series, those due by `now` or every one when
    /// `all`. Says which now hold some.
    pub async fn recheck(&self, now: i64, all: bool) -> Vec<AddedServer> {
        let mut filled = Vec::new();
        for server in self.store.book_servers().unwrap_or_default() {
            let mut meta = self.server_meta(&server.id);
            let due = meta.rechecks_at().is_some_and(|at| all || at <= now);
            if !meta.empty || !due || server.signed_out {
                continue;
            }
            let Some(kind) = Kind::parse(&server.kind) else { continue };
            let Ok(credential) = self.tokens.get(&secret_key(&server.id)) else { continue };
            let counted = kaeri_bookserver::count_series_kept(
                &self.servers,
                &server.address,
                kind,
                &credential,
                &self.gate(&server.id),
            )
            .await;
            meta.checked_at = Some(now);
            match counted {
                Ok(Some(n)) if n > 0 => {
                    meta.series = Some(n);
                    meta.empty = false;
                    filled.push(AddedServer {
                        id: server.id.clone(),
                        kind: kind_of(kind),
                        name: server.name.clone(),
                        host: host(&server.address),
                        series: Some(n),
                        libraries: meta.library_count,
                        empty: false,
                    });
                }
                Ok(_) => {}
                Err(e) => tracing::debug!(error = %e, "an empty server was not asked again"),
            }
            self.keep_server_meta(&server.id, &meta);
        }
        filled
    }

    /// Every server signed in to here, with its sign-in, for a paired device that asks.
    pub(crate) fn gifts(&self) -> Vec<ServerGift> {
        self.store
            .book_servers()
            .unwrap_or_default()
            .into_iter()
            .filter(|s| !s.signed_out)
            .filter_map(|s| {
                let credential = self.tokens.get(&secret_key(&s.id)).ok()?;
                let meta = self.server_meta(&s.id);
                Some(ServerGift {
                    kind: s.kind,
                    name: s.name,
                    address: s.address,
                    user: s.user,
                    credential,
                    libraries: meta.libraries,
                    library_count: meta.library_count,
                    series: meta.series,
                    report_progress: s.report_progress,
                    key_last4: meta.key_last4,
                })
            })
            .collect()
    }

    /// Whether a server with this id is one of this device's.
    pub(crate) fn has_server(&self, id: &str) -> bool {
        self.store.book_server(id).ok().flatten().is_some()
    }

    /// How many folders this device reads from, which stay on it.
    pub(crate) fn folder_count(&self) -> u32 {
        u32::try_from(self.store.book_folders().unwrap_or_default().len()).unwrap_or(u32::MAX)
    }

    /// Add a server a paired device handed over, asking the libraries it asks.
    pub(crate) fn add_gift(&self, gift: ServerGift, now: i64) -> Option<AddedServer> {
        let kind = Kind::parse(&gift.kind)?;
        let held = Pending {
            kind,
            name: gift.name,
            address: gift.address,
            user: gift.user,
            credential: gift.credential,
            series: gift.series,
            libraries: Vec::new(),
            key_last4: gift.key_last4,
        };
        self.add_server(
            held,
            gift.libraries,
            gift.library_count,
            gift.series,
            gift.report_progress,
            now,
        )
        .ok()
    }
}

/// A book server and its sign-in, as one device hands it to another over the paired link,
/// sealed for the asking device. No `Debug`: the credential is a secret.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ServerGift {
    pub kind: String,
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub user: Option<String>,
    pub credential: String,
    #[serde(default)]
    pub libraries: Vec<String>,
    #[serde(default)]
    pub library_count: u32,
    #[serde(default)]
    pub series: Option<u32>,
    #[serde(default = "yes")]
    pub report_progress: bool,
    #[serde(default)]
    pub key_last4: Option<String>,
}

const fn yes() -> bool {
    true
}

impl ServerGift {
    pub fn id(&self) -> String {
        server_id(&self.address, self.user.as_deref())
    }

    pub fn kind(&self) -> Option<BookServerKind> {
        Kind::parse(&self.kind).map(kind_of)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_server_is_looked_at_again_after_half_an_hour() {
        let meta = ServerMeta { empty: true, checked_at: Some(1_000), ..ServerMeta::default() };
        assert_eq!(meta.rechecks_at(), Some(1_000 + 1_800));
        let full =
            ServerMeta { series: Some(4), checked_at: Some(1_000), ..ServerMeta::default() };
        assert_eq!(full.rechecks_at(), None);
    }

    #[test]
    fn an_opds_catalogue_is_one_library() {
        assert_eq!(row_libraries(BookServerKind::Opds, 0), 1);
        assert_eq!(row_libraries(BookServerKind::Kavita, 0), 0);
        assert_eq!(row_libraries(BookServerKind::Kavita, 2), 2);
    }

    #[test]
    fn a_wrong_key_is_not_a_wrong_password() {
        let at = "https://books.home.example/";
        assert_eq!(
            nothing_for(&ServerError::Unauthorized, at, Kind::Kavita, true),
            NothingCameIn::WrongKey { host: "books.home.example".into() }
        );
        assert_eq!(
            nothing_for(&ServerError::Unauthorized, at, Kind::Komga, false),
            NothingCameIn::WrongPassword {
                host: "books.home.example".into(),
                kind: BookServerKind::Komga
            }
        );
        assert!(matches!(
            nothing_for(&ServerError::Unreachable("x".into()), at, Kind::Opds, false),
            NothingCameIn::Unreachable { .. }
        ));
    }

    #[test]
    fn a_folder_of_only_unsupported_books_says_so() {
        let folder = Folder {
            id: "f".into(),
            path: Some("/books".into()),
            tree_uri: None,
            bookmark: None,
            name: Some("Books".into()),
            added_at: 0,
            last_scan: None,
            available: true,
        };
        let scan = Scan {
            seen: 3,
            unsupported: vec![("MOBI".into(), 2), ("AZW3".into(), 1)],
            unsupported_files: vec!["/books/a.mobi".into()],
            ..Scan::default()
        };
        let report = folder_report(&folder, &scan);
        assert_eq!((report.files, report.skipped(), report.series), (3, 3, 0));
        assert!(matches!(
            folder_came_in(report.clone()),
            CameIn::Empty(NothingCameIn::UnsupportedFiles { .. })
        ));
        let empty = FolderReport { unsupported: Vec::new(), ..report };
        assert!(matches!(
            folder_came_in(empty),
            CameIn::Empty(NothingCameIn::EmptyFolder { .. })
        ));
    }
}
