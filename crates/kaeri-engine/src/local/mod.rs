//! Your media: folders on this computer, read for video files and matched to AniList.
//!
//! What is kept: the folders and the groups found in them, as JSON in the database. Nothing is
//! written to the folders. A group is the files of one series. Matching goes folder name first
//! through AniList's search at the same confidence the sources use; a match below
//! [`SURE`] waits in Needs a look until the viewer confirms it, and only a sure or confirmed
//! match plays or reaches a tracker. Files the viewer keeps unmatched become a title with its own
//! metadata, which no tracker ever sees.

mod scan;
pub mod source;

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use kaeri_core::ids::AnilistId;
use kaeri_meta::Meta;
use kaeri_model::{
    event::Event,
    local::{
        FolderRow, FolderScan, GroupState, LocalCommand, LocalEvent, LocalGroup, LocalView,
        MatchChoice, MatchProgress, NotMatchedReason, UnnumberedFile,
    },
};
use kaeri_providers::ProviderRegistry;
use kaeri_store::Store;
use serde::{Deserialize, Serialize};

use crate::{outbox::Outbox, services::Services};

const KEY: &str = "local:state";

/// The confidence at which a match is acted on without asking.
pub const SURE: f64 = 0.85;

/// The most name lookups one scan makes, because AniList allows 30 requests a minute and the
/// rest of the app shares them. Groups left over are matched on the next scan.
const LOOKUPS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Standing {
    Matched,
    NeedsLook,
    NotMatched,
    Kept,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub episode: Option<u32>,
    /// The season, when the viewer set it with Set episode. 0 holds specials.
    #[serde(default)]
    pub season: Option<u32>,
    /// Set by the viewer with Set episode: a rescan keeps it.
    #[serde(default)]
    pub set: bool,
}

impl FileEntry {
    pub fn found(path: String, episode: Option<u32>) -> Self {
        Self { path, episode, season: None, set: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub folder: String,
    pub name: String,
    pub files: Vec<FileEntry>,
    pub standing: Standing,
    /// Said by the viewer: a rescan never changes it.
    #[serde(default)]
    pub decided: bool,
    pub anilist: Option<AnilistId>,
    pub matched_title: Option<String>,
    #[serde(default)]
    pub offline: bool,
    /// How sure the best guess is, 0 to 1.
    #[serde(default)]
    pub confidence: Option<f64>,
}

impl Group {
    /// Whether `file` waits for Set episode: its name has no number and its group's other files
    /// do, so its place would be a guess. A kept group numbers its files in order, and a group
    /// with no numbers at all is numbered in order too, as a film or a batch is.
    pub fn waits_for_episode(&self, file: &FileEntry) -> bool {
        self.standing != Standing::Kept
            && file.episode.is_none()
            && self.files.iter().any(|f| f.episode.is_some())
    }
}

/// A folder taken out and held for Undo.
struct Removed {
    /// Which removal this is, so a timer forgets only its own.
    stamp: u64,
    at: usize,
    folder: String,
    unavailable: bool,
    groups: Vec<Group>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    folders: Vec<String>,
    /// Folders that could not be read at the last scan.
    #[serde(default)]
    unavailable: Vec<String>,
    groups: Vec<Group>,
    /// Own-metadata ids handed out so far.
    #[serde(default)]
    own: u32,
}

/// The handles Your media needs, cheap to clone.
#[derive(Clone)]
pub struct Local {
    store: Store,
    scanning: Arc<Mutex<bool>>,
    /// Each folder's row line while a scan runs.
    progress: Arc<Mutex<HashMap<String, FolderScan>>>,
    /// Folders removed while their Undo notice shows.
    removed: Arc<Mutex<Vec<Removed>>>,
}

/// How long a removed folder is held for Undo: the notice's 8 seconds and a little more.
pub const UNDO: std::time::Duration = crate::servers::UNDO;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// How many seconds AniList's limit would hold a lookup back now, when it is a second or more.
pub(crate) async fn wait_seconds(meta: &Meta) -> Option<u32> {
    let wait = meta.rate_limit_wait().await?;
    (wait.as_secs_f64() >= 1.0).then(|| wait.as_secs_f64().ceil() as u32)
}

impl Local {
    pub fn new(store: Store) -> Self {
        let local = Self {
            store,
            scanning: Default::default(),
            progress: Default::default(),
            removed: Default::default(),
        };
        #[cfg(feature = "testing")]
        if let Some(scan) =
            local.store.get_meta_text(FIXTURE_SCAN).ok().flatten().and_then(|text| {
                text.split_once('|').map(|(f, n)| (f.to_owned(), n.to_owned()))
            })
        {
            // A scene's row line, mid-scan: "Matching · 31 of 58 series · Waiting for AniList".
            let numbers: Vec<u32> = scan.1.split(',').filter_map(|n| n.parse().ok()).collect();
            if let [done, total, wait] = numbers[..] {
                let progress =
                    MatchProgress { done, total, wait_seconds: (wait > 0).then_some(wait) };
                local.set_progress(&scan.0, Some(FolderScan::Matching { progress }));
            }
        }
        local
    }

    fn set_progress(&self, folder: &str, scan: Option<FolderScan>) {
        let mut map = lock(&self.progress);
        match scan {
            Some(scan) => map.insert(folder.to_owned(), scan),
            None => map.remove(folder),
        };
    }

    fn state(&self) -> State {
        self.store
            .get_meta_text(KEY)
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save(&self, state: &State) {
        if let Ok(text) = serde_json::to_string(state) {
            let _ = self.store.set_meta_text(KEY, &text);
        }
    }

    fn change(&self, change: impl FnOnce(&mut State)) {
        let mut state = self.state();
        change(&mut state);
        self.save(&state);
    }

    pub fn groups(&self) -> Vec<Group> {
        self.state().groups
    }

    fn view(&self) -> LocalView {
        let state = self.state();
        let scanning = *lock(&self.scanning);
        let progress = lock(&self.progress).clone();
        LocalView {
            folders: state
                .folders
                .iter()
                .map(|path| {
                    let groups: Vec<&Group> =
                        state.groups.iter().filter(|g| &g.folder == path).collect();
                    let playing: Vec<&&Group> = groups
                        .iter()
                        .filter(|g| matches!(g.standing, Standing::Matched | Standing::Kept))
                        .collect();
                    FolderRow {
                        path: path.clone(),
                        files: groups.iter().map(|g| g.files.len() as u32).sum(),
                        available: !state.unavailable.contains(path),
                        scan: progress.get(path).cloned(),
                        series: playing.len() as u32,
                        episodes: playing
                            .iter()
                            .map(|g| g.files.iter().filter(|f| !g.waits_for_episode(f)).count())
                            .sum::<usize>() as u32,
                        needs_look: groups
                            .iter()
                            .filter(|g| g.standing == Standing::NeedsLook)
                            .count() as u32,
                    }
                })
                .collect(),
            groups: state.groups.iter().map(row).collect(),
            scanning,
            unnumbered: state.groups.iter().flat_map(unnumbered).collect(),
        }
    }

    fn publish(&self, tx: &Outbox) {
        let _ = tx.send(Event::Local(LocalEvent::View(self.view())));
    }

    /// Put the source in the chain, where `order` places it. Replaces the one there.
    pub fn put_source(&self, registry: &ProviderRegistry, order: &[String]) {
        registry.remove(source::ID);
        if self.state().groups.is_empty() {
            return;
        }
        registry.put(Arc::new(source::LocalSource::new(self.groups())));
        registry.rank_by(|p| crate::sources::rank(p, order));
    }
}

/// Your media as a scene shows it: one folder, a matched series, one that needs a look and one
/// not matched. Made-up; no file exists behind the paths.
pub fn seed_fixture(store: &Store) {
    let group = |name: &str, standing, anilist: Option<u32>, title: Option<&str>| Group {
        id: format!("/Volumes/Media|{}", name.to_lowercase()),
        folder: "/Volumes/Media".into(),
        name: name.into(),
        files: (1..=3)
            .map(|n| {
                FileEntry::found(format!("/Volumes/Media/{name}/{name} - 0{n}.mkv"), Some(n))
            })
            .collect(),
        standing,
        decided: false,
        anilist: anilist.map(AnilistId::new),
        matched_title: title.map(str::to_owned),
        offline: false,
        confidence: (standing == Standing::NeedsLook).then_some(0.71),
    };
    let mut wayfarer =
        group("Stardust Wayfarer", Standing::Matched, Some(900_001), Some("Stardust Wayfarer"));
    // A file with no number beside numbered ones waits for Set episode.
    wayfarer.files.push(FileEntry::found(
        "/Volumes/Media/Stardust Wayfarer/extras/Making of.mkv".into(),
        None,
    ));
    let mut tabibito = group(
        "Hoshikuzu Tabibito S2 1080p",
        Standing::NeedsLook,
        Some(900_003),
        Some("Hoshikuzu no Tabibito Season 2"),
    );
    tabibito.files = (1..=12)
        .map(|n| {
            FileEntry::found(
                format!("/Volumes/Media/Hoshikuzu Tabibito S2 1080p/Hoshikuzu Tabibito S2 - {n:02}.mkv"),
                Some(n),
            )
        })
        .collect();
    let mut home = group("Hoshikage Home Movies", Standing::NotMatched, None, None);
    home.files = (1..=8)
        .map(|n| {
            FileEntry::found(format!("/Volumes/Media/Hoshikage Home Movies/Reel {n}.mp4"), None)
        })
        .collect();
    let local = Local::new(store.clone());
    local.save(&State {
        folders: vec!["/Volumes/Media".into()],
        unavailable: Vec::new(),
        groups: vec![
            wayfarer,
            group(
                "Lantern Harbour",
                Standing::NeedsLook,
                Some(900_002),
                Some("Lantern Harbour Season 2"),
            ),
            tabibito,
            group("Quiet Orchard Days", Standing::NotMatched, None, None),
            home,
        ],
        own: 0,
    });
}

#[cfg(feature = "testing")]
const FIXTURE_SCAN: &str = "local:fixture-scan";

/// For a scene: the folder seeded by [`seed_fixture`] shows as part way through matching, with
/// AniList holding the next lookup back. Only in a build with the `testing` feature.
#[cfg(feature = "testing")]
pub fn seed_fixture_matching(store: &Store) {
    let _ = store.set_meta_text(FIXTURE_SCAN, "/Volumes/Media|31,58,12");
}

/// The folders and groups saved in `store`.
pub fn saved(store: &Store) -> Vec<Group> {
    Local::new(store.clone()).groups()
}

fn row(group: &Group) -> LocalGroup {
    LocalGroup {
        id: group.id.clone(),
        name: group.name.clone(),
        files: group.files.len() as u32,
        state: match group.standing {
            Standing::Matched => GroupState::Matched,
            Standing::NeedsLook => GroupState::NeedsLook,
            Standing::NotMatched => GroupState::NotMatched,
            Standing::Kept => GroupState::Kept,
        },
        anilist: group.anilist,
        matched_title: group.matched_title.clone(),
        reason: (group.standing == Standing::NotMatched).then_some(if group.offline {
            NotMatchedReason::Offline
        } else {
            NotMatchedReason::NoResult
        }),
        folder: group.folder.clone(),
        confidence: (group.standing == Standing::NeedsLook)
            .then_some(group.confidence)
            .flatten()
            .map(|c| (c * 100.0).round().clamp(0.0, 100.0) as u8),
    }
}

/// The files of `group` that wait for Set episode, as the folder page lists them.
fn unnumbered(group: &Group) -> Vec<UnnumberedFile> {
    group
        .files
        .iter()
        .filter(|f| group.waits_for_episode(f))
        .map(|f| {
            let path = Path::new(&f.path);
            let rel = path.strip_prefix(&group.folder).unwrap_or(path);
            let dirs: Vec<String> = rel
                .parent()
                .into_iter()
                .flat_map(|p| p.components())
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            // The first folder is the group's own; a loose file has none.
            let place = dirs.get(1..).map(|d| d.join("/")).unwrap_or_default();
            UnnumberedFile {
                group: group.id.clone(),
                folder: group.folder.clone(),
                path: f.path.clone(),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                place,
            }
        })
        .collect()
}

/// Run one command. A scan runs as a task.
pub fn command(command: LocalCommand, services: &Services, order: Vec<String>, tx: &Outbox) {
    let local = services.local.clone();
    let (registry, meta, tx) = (services.registry.clone(), services.meta.clone(), tx.clone());
    match command {
        LocalCommand::Load => local.publish(&tx),
        LocalCommand::AddFolder { path } => {
            local.change(|s| {
                if !s.folders.contains(&path) {
                    s.folders.push(path);
                }
            });
            scan_all(local, registry, meta, order, tx);
        }
        LocalCommand::RemoveFolder { path } => {
            let mut held = None;
            local.change(|s| {
                let Some(at) = s.folders.iter().position(|f| f == &path) else { return };
                s.folders.remove(at);
                let unavailable = s.unavailable.contains(&path);
                s.unavailable.retain(|f| f != &path);
                let (gone, kept) =
                    std::mem::take(&mut s.groups).into_iter().partition(|g| g.folder == path);
                s.groups = kept;
                held = Some(Removed {
                    stamp: 0,
                    at,
                    folder: path.clone(),
                    unavailable,
                    groups: gone,
                });
            });
            if let Some(mut held) = held {
                static STAMPS: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let stamp = STAMPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                held.stamp = stamp;
                let mut removed = lock(&local.removed);
                removed.retain(|r| r.folder != path);
                removed.push(held);
                let later = local.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(UNDO).await;
                    lock(&later.removed).retain(|r| r.stamp != stamp);
                });
            }
            local.put_source(&registry, &order);
            local.publish(&tx);
        }
        LocalCommand::RestoreFolder { path } => {
            let held = {
                let mut removed = lock(&local.removed);
                removed.iter().position(|r| r.folder == path).map(|at| removed.remove(at))
            };
            match held {
                Some(held) => {
                    local.change(|s| {
                        if s.folders.contains(&held.folder) {
                            return;
                        }
                        s.folders.insert(held.at.min(s.folders.len()), held.folder.clone());
                        if held.unavailable {
                            s.unavailable.push(held.folder.clone());
                        }
                        s.groups.extend(held.groups);
                    });
                    local.put_source(&registry, &order);
                    local.publish(&tx);
                }
                // Past the notice: add it again, which scans it.
                None => crate::local::command(
                    LocalCommand::AddFolder { path },
                    services,
                    order,
                    &tx,
                ),
            }
        }
        LocalCommand::SetEpisode { group, path, season, episode } => {
            local.change(|s| {
                let Some(g) = s.groups.iter_mut().find(|g| g.id == group) else { return };
                if let Some(f) = g.files.iter_mut().find(|f| f.path == path) {
                    f.season = Some(season);
                    f.episode = Some(episode);
                    f.set = true;
                }
                g.files.sort_by(|a, b| (a.episode, &a.path).cmp(&(b.episode, &b.path)));
            });
            local.put_source(&registry, &order);
            local.publish(&tx);
        }
        LocalCommand::Scan => scan_all(local, registry, meta, order, tx),
        LocalCommand::FindMatch { group, query } => {
            let Some(found) = local.state().groups.into_iter().find(|g| g.id == group) else {
                return;
            };
            tokio::spawn(async move {
                let term =
                    if query.trim().is_empty() { found.name.as_str() } else { query.trim() };
                let numbered = found.files.iter().filter(|f| f.episode.is_some()).count();
                let files = u32::try_from(numbered).unwrap_or(u32::MAX);
                let (choices, offline) = match crate::titlematch::scored(&meta, term, None, 10)
                    .await
                {
                    Ok(scored) => (
                        scored.into_iter().map(|(m, score)| choice(&m, score, files)).collect(),
                        false,
                    ),
                    Err(()) => (Vec::new(), true),
                };
                let _ = tx.send(Event::Local(LocalEvent::Matches {
                    group,
                    query,
                    choices,
                    offline,
                }));
            });
        }
        LocalCommand::Choose { group, anilist } => {
            tokio::spawn(async move {
                let title =
                    meta.media(anilist).await.ok().map(|m| m.title.display().to_owned());
                local.change(|s| {
                    if let Some(g) = s.groups.iter_mut().find(|g| g.id == group) {
                        g.standing = Standing::Matched;
                        g.decided = true;
                        g.anilist = Some(anilist);
                        g.matched_title = title;
                        g.offline = false;
                    }
                });
                local.put_source(&registry, &order);
                local.publish(&tx);
            });
        }
        LocalCommand::Keep { group } => {
            local.change(|s| {
                let next = s.own;
                if let Some(g) = s.groups.iter_mut().find(|g| g.id == group) {
                    let reuse = g.anilist.filter(|id| id.is_own());
                    let id = reuse.unwrap_or_else(|| AnilistId::own(next));
                    g.standing = Standing::Kept;
                    g.decided = true;
                    g.anilist = Some(id);
                    g.matched_title = Some(g.name.clone());
                    g.offline = false;
                    if reuse.is_none() {
                        s.own += 1;
                    }
                }
            });
            local.put_source(&registry, &order);
            local.publish(&tx);
        }
    }
}

/// Read every folder, then match what is new. The view is sent when the files are found and
/// again when the matching is done.
/// A title Fix this match offers. Its episode count fits when it is the number of numbered files
/// in the group, or AniList doesn't say yet.
fn choice(media: &kaeri_meta::anilist::Media, score: f64, files: u32) -> MatchChoice {
    MatchChoice {
        anilist: media.id,
        title: media.title.display().to_owned(),
        format: media.format.and_then(crate::data::format_label),
        year: media.season_year,
        episodes: media.episodes,
        confidence: (score.clamp(0.0, 1.0) * 100.0).round() as u8,
        fits: media.episodes.is_none_or(|n| n == files),
    }
}

fn scan_all(
    local: Local,
    registry: ProviderRegistry,
    meta: Meta,
    order: Vec<String>,
    tx: Outbox,
) {
    {
        let mut scanning = local.scanning.lock().unwrap_or_else(|e| e.into_inner());
        if *scanning {
            return;
        }
        *scanning = true;
    }
    tokio::spawn(async move {
        let folders = local.state().folders;
        for folder in &folders {
            local.set_progress(folder, Some(FolderScan::Scanning { files: 0 }));
        }
        local.publish(&tx);
        let mut unavailable = Vec::new();
        let mut found: Vec<(String, Vec<scan::Series>)> = Vec::new();
        for folder in folders {
            let path = folder.clone();
            let (counting, out) = (local.clone(), tx.clone());
            let read = tokio::task::spawn_blocking(move || {
                scan::scan_counting(Path::new(&path), &mut |files| {
                    counting.set_progress(&path, Some(FolderScan::Scanning { files }));
                    counting.publish(&out);
                })
            })
            .await;
            match read {
                Ok(Ok(series)) => {
                    let files = series.iter().map(|s| s.files.len() as u32).sum();
                    local.set_progress(&folder, Some(FolderScan::Scanning { files }));
                    found.push((folder, series));
                }
                _ => {
                    local.set_progress(&folder, None);
                    unavailable.push(folder);
                }
            }
        }
        // Keep what the viewer decided and what was matched; groups from a folder that could
        // not be read stay as they were.
        local.change(|s| {
            let old = std::mem::take(&mut s.groups);
            s.unavailable = unavailable.clone();
            s.groups =
                old.iter().filter(|g| unavailable.contains(&g.folder)).cloned().collect();
            for (folder, series) in &found {
                for series in series {
                    let id = format!("{folder}|{}", series.name.to_lowercase());
                    let known = old.iter().find(|g| g.id == id);
                    let files: Vec<FileEntry> = series
                        .files
                        .iter()
                        .map(|f| {
                            let path = f.path.to_string_lossy().into_owned();
                            // What the viewer set with Set episode stays.
                            known
                                .and_then(|g| g.files.iter().find(|k| k.set && k.path == path))
                                .cloned()
                                .unwrap_or_else(|| FileEntry::found(path, f.episode))
                        })
                        .collect();
                    let group = match known {
                        Some(known) => Group { files, ..known.clone() },
                        None => Group {
                            id,
                            folder: folder.clone(),
                            name: series.name.clone(),
                            files,
                            standing: Standing::NotMatched,
                            decided: false,
                            anilist: None,
                            matched_title: None,
                            offline: false,
                            confidence: None,
                        },
                    };
                    s.groups.push(group);
                }
            }
        });
        local.put_source(&registry, &order);
        local.publish(&tx);

        // The groups this scan looks up, and how many of each folder's, for the row lines.
        let wanted: Vec<Group> = local
            .groups()
            .into_iter()
            .filter(|g| g.standing == Standing::NotMatched && !g.decided)
            .take(LOOKUPS)
            .collect();
        let mut tally: HashMap<String, (u32, u32)> = HashMap::new();
        for group in &wanted {
            tally.entry(group.folder.clone()).or_default().1 += 1;
        }
        let matching = |done: u32, total: u32, wait_seconds: Option<u32>| {
            FolderScan::Matching { progress: MatchProgress { done, total, wait_seconds } }
        };
        for (folder, _) in &found {
            let total = tally.get(folder).map_or(0, |t| t.1);
            local.set_progress(folder, (total > 0).then(|| matching(0, total, None)));
        }
        local.publish(&tx);
        for group in wanted {
            let (done, total) = tally.get(&group.folder).copied().unwrap_or_default();
            if let Some(seconds) = wait_seconds(&meta).await {
                local.set_progress(&group.folder, Some(matching(done, total, Some(seconds))));
                local.publish(&tx);
            }
            let result = crate::titlematch::candidates(&meta, &group.name, None).await;
            local.change(|s| {
                let Some(g) = s.groups.iter_mut().find(|g| g.id == group.id) else { return };
                match &result {
                    Err(()) => g.offline = true,
                    Ok(found) => {
                        g.offline = false;
                        if let Some(best) = found.first() {
                            let clear =
                                found.get(1).is_none_or(|next| best.score - next.score >= 0.05);
                            g.standing = if best.score >= SURE && clear {
                                Standing::Matched
                            } else {
                                Standing::NeedsLook
                            };
                            g.anilist = Some(best.id);
                            g.matched_title = Some(best.title.clone());
                            g.confidence = Some(best.score);
                        }
                    }
                }
            });
            let entry = tally.entry(group.folder.clone()).or_default();
            entry.0 += 1;
            let (done, total) = *entry;
            local.set_progress(
                &group.folder,
                (done < total).then(|| matching(done, total, None)),
            );
            local.publish(&tx);
        }
        lock(&local.progress).clear();
        *lock(&local.scanning) = false;
        local.put_source(&registry, &order);
        local.publish(&tx);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_that_waits_has_no_reason_but_one_not_matched_does() {
        let mut group = Group {
            id: "a".into(),
            folder: "f".into(),
            name: "n".into(),
            files: Vec::new(),
            standing: Standing::NeedsLook,
            decided: false,
            anilist: None,
            matched_title: None,
            offline: false,
            confidence: Some(0.714),
        };
        assert_eq!(row(&group).reason, None);
        assert_eq!(row(&group).confidence, Some(71));
        group.standing = Standing::NotMatched;
        assert_eq!(row(&group).reason, Some(NotMatchedReason::NoResult));
        group.offline = true;
        assert_eq!(row(&group).reason, Some(NotMatchedReason::Offline));
    }
}
