//! The folder road of cross-device sync in the running engine: setting a group up, joining one,
//! a pass through the folder, and the status the Sync section shows.
//!
//! What is kept, and where: the folder and the device's name in the personal tables' `sync_meta`,
//! so each profile has its own group; the group keyring, the device's key seed, the recovery key
//! and a WebDAV password in the profile's token store, so they live wherever the host keeps
//! tokens and never in a file of ours.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use kaeri_model::{
    event::Event,
    group::{
        SyncDeviceView, SyncFolderKind, SyncFolderSpec, SyncFolderView, SyncNoteView,
        SyncProblem, SyncSignInStep, SyncStatus, SyncStoreOffer,
    },
};
use kaeri_store::sync::Effect;
use kaeri_sync::{
    SyncError,
    folder::{DirTarget, FolderError, FolderTarget},
    keys::{self, DeviceKey, Keyring, RecoveryKey},
    lan::{self, Identity, Listener, Me},
    nextcloud,
    peer::PeerReport,
    road::{FolderReport, FolderRoad},
    webdav::{Password, WebDavTarget},
};

use crate::{
    cloud::{self, Ended, Service},
    host::Host,
    outbox::{Internal, Outbox},
    plugin_stores::Stores,
    services::Services,
    tracking,
};

/// The folder under a WebDAV or Nextcloud address that holds the group.
const ROOT: &str = "kaeri-sync";
/// How long the person has to approve a Nextcloud sign-in.
const LOGIN_WAIT: Duration = Duration::from_secs(300);
/// Between passes while the engine runs.
pub const EVERY: Duration = Duration::from_secs(300);

pub(crate) const KEYRING: &str = "sync-keyring";
const SEED: &str = "sync-seed";
const RECOVERY: &str = "sync-recovery";
pub(crate) const PASSWORD: &str = "sync-password";
const FOLDER: &str = "engine.folder";
const NAME: &str = "engine.name";
const LAST: &str = "engine.last";
const ERROR: &str = "engine.error";
/// A problem a person can fix, kept until a pass works: `signin:<kind>`.
const PROBLEM: &str = "engine.problem";
const SIGN_IN: &str = "signin:";
/// When the recovery key this device holds was made, in Unix seconds.
const RECOVERY_MADE: &str = "engine.recovery_made";
/// What the folder road keeps of the folder it reads (cursors, numbers published), which means
/// nothing in another folder.
const ROAD_META: &str = "folder.";

/// The parts of the services a pass needs, owned, so a pass can run on a task of its own.
#[derive(Clone)]
pub struct Ctx {
    pub store: kaeri_store::Store,
    pub sync: tracking::Sync,
    pub host: Arc<dyn Host>,
    pub stores: Stores,
    pub syncing: Arc<SyncHub>,
    /// The sign-in clients this build ships with, for the cloud drives.
    pub clients: crate::options::AppClients,
    /// A fixture build: cloud sign-in goes to a fake service and the folder is kept in memory.
    pub fixture: bool,
}

impl Ctx {
    pub fn of(services: &Services) -> Self {
        Self {
            store: services.store.clone(),
            sync: services.sync.clone(),
            host: services.host.clone(),
            stores: services.stores.clone(),
            syncing: services.syncing.clone(),
            clients: services.app_clients.clone(),
            fixture: services.fixture_route,
        }
    }
}

/// What the engine keeps of sync between commands.
#[derive(Default)]
pub struct SyncHub {
    road: Mutex<Option<Arc<FolderRoad>>>,
    running: AtomicBool,
    effects: Mutex<Vec<Effect>>,
    listener: Mutex<Option<Arc<tokio::sync::Mutex<Listener>>>>,
    /// The cloud sign-in under way, and what a test or a fixture build points it at.
    pub cloud: crate::cloud::State,
    /// The language the front end speaks, as a tag, for the page a desktop sign-in ends on.
    language: Mutex<Option<String>>,
}

impl std::fmt::Debug for SyncHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncHub").finish_non_exhaustive()
    }
}

impl SyncHub {
    /// What a pass brought for the config, once.
    pub fn take_effects(&self) -> Vec<Effect> {
        std::mem::take(
            &mut *self.effects.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    pub fn set_language(&self, language: Option<String>) {
        *self.language.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = language;
    }

    /// The front end's language, English when it said none.
    pub fn language(&self) -> String {
        self.language
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| "en".into())
    }

    fn road(&self) -> Option<Arc<FolderRoad>> {
        self.road.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn set_road(&self, road: Option<Arc<FolderRoad>>) {
        *self.road.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = road;
    }
}

fn problem(e: &SyncError) -> SyncProblem {
    problem_in(None, e)
}

/// `e` as a problem in the folder this device syncs with.
fn problem_at(services: &Ctx, e: &SyncError) -> SyncProblem {
    problem_in(saved_folder(services).map(|f| f.kind).as_ref(), e)
}

/// `e` as a problem, in the cloud drive `kind` names when it is one: a refused sign-in is fixed
/// by signing in again.
fn problem_in(kind: Option<&SyncFolderKind>, e: &SyncError) -> SyncProblem {
    if let (SyncError::Folder(FolderError::Unauthorized), Some(kind)) = (e, kind)
        && Service::of(kind).is_some()
    {
        return SyncProblem::SignInNeeded { service: kind.clone() };
    }
    match e {
        SyncError::WrongRecoveryKey | SyncError::BadRecoveryKey => {
            SyncProblem::WrongRecoveryKey
        }
        SyncError::NoGroup => SyncProblem::NoGroup,
        SyncError::GroupExists => SyncProblem::GroupExists,
        SyncError::Folder(_) | SyncError::Network(_) => SyncProblem::Unreachable,
        other => SyncProblem::Other(other.to_string()),
    }
}

/// A problem in words, for the status's error line.
fn describe(p: &SyncProblem) -> String {
    match p {
        SyncProblem::SignInNeeded { .. } => "the sign-in is no longer accepted".into(),
        SyncProblem::Unreachable => "the folder could not be reached".into(),
        SyncProblem::NoGroup => "the folder holds no sync group".into(),
        SyncProblem::WrongRecoveryKey => "that recovery key does not open this folder".into(),
        SyncProblem::GroupExists => "the folder already holds a sync group".into(),
        SyncProblem::Other(text) => text.clone(),
        _ => "sync could not carry on".into(),
    }
}

pub(crate) fn kind_name(kind: &SyncFolderKind) -> &'static str {
    match kind {
        SyncFolderKind::Directory => "dir",
        SyncFolderKind::WebDav => "webdav",
        SyncFolderKind::Nextcloud => "nextcloud",
        SyncFolderKind::GoogleDrive => "google",
        SyncFolderKind::Dropbox => "dropbox",
        SyncFolderKind::OneDrive => "onedrive",
    }
}

pub(crate) fn kind_of(name: &str) -> Option<SyncFolderKind> {
    Some(match name {
        "dir" => SyncFolderKind::Directory,
        "webdav" => SyncFolderKind::WebDav,
        "nextcloud" => SyncFolderKind::Nextcloud,
        "google" => SyncFolderKind::GoogleDrive,
        "dropbox" => SyncFolderKind::Dropbox,
        "onedrive" => SyncFolderKind::OneDrive,
        _ => return None,
    })
}

/// A folder as kept: no secret in it.
#[derive(serde::Serialize, serde::Deserialize)]
struct Saved {
    kind: String,
    address: Option<String>,
    user: Option<String>,
}

pub(crate) fn save_folder(services: &Ctx, spec: &SyncFolderSpec) {
    let saved = Saved {
        kind: kind_name(&spec.kind).to_owned(),
        address: spec.address.clone(),
        user: spec.user.clone(),
    };
    if let Ok(json) = serde_json::to_string(&saved) {
        let _ = services.store.sync_meta_set(FOLDER, &json);
    }
}

fn saved_folder(services: &Ctx) -> Option<SyncFolderSpec> {
    saved_folder_in(&services.store)
}

/// The folder this device syncs through, as kept: no secret in it.
pub(crate) fn saved_folder_in(store: &kaeri_store::Store) -> Option<SyncFolderSpec> {
    let json = store.sync_meta_get(FOLDER).ok().flatten()?;
    let saved: Saved = serde_json::from_str(&json).ok()?;
    Some(SyncFolderSpec {
        kind: kind_of(&saved.kind)?,
        address: saved.address,
        user: saved.user,
        password: None,
    })
}

fn label_of(spec: &SyncFolderSpec) -> String {
    let address = spec.address.clone().unwrap_or_default();
    match spec.kind {
        SyncFolderKind::Directory => address,
        SyncFolderKind::WebDav | SyncFolderKind::Nextcloud => {
            address.split("://").last().unwrap_or(&address).trim_end_matches('/').to_owned()
        }
        SyncFolderKind::GoogleDrive => "Google Drive".into(),
        SyncFolderKind::Dropbox => "Dropbox".into(),
        SyncFolderKind::OneDrive => "OneDrive".into(),
    }
}

/// The target a folder spec names. A Nextcloud sign-in opens the server's login page and waits
/// for the person to approve; what it hands back is kept as a WebDAV login.
///
/// A cloud drive signs in first when this device holds no sign-in for it and `tx` is given, which
/// is how the person is asked; with no `tx` it is a [`SyncProblem::SignInNeeded`].
pub(crate) async fn target_for(
    services: &Ctx,
    tx: Option<&Outbox>,
    spec: &mut SyncFolderSpec,
) -> Result<Arc<dyn FolderTarget>, Ended> {
    let address = spec.address.clone().unwrap_or_default();
    match spec.kind {
        SyncFolderKind::Directory => {
            if address.is_empty() {
                return Err(SyncProblem::Other("no folder was given".into()).into());
            }
            Ok(Arc::new(DirTarget::new(address)))
        }
        SyncFolderKind::WebDav => {
            let password = match &spec.password {
                Some(secret) => secret.0.clone(),
                None => services.sync.tokens.get(PASSWORD).unwrap_or_default(),
            };
            let target = WebDavTarget::new(
                &address,
                spec.user.as_deref().unwrap_or_default(),
                Password::new(password.clone()),
                ROOT,
            )
            .map_err(|e| SyncProblem::Other(e.to_string()))?;
            target.check().await.map_err(|_| SyncProblem::Unreachable)?;
            let _ = services.sync.tokens.set(PASSWORD, &password);
            Ok(Arc::new(target))
        }
        SyncFolderKind::Nextcloud => {
            let (server, login, password) = match (&spec.user, &spec.password) {
                // Signed in before: the app password is kept.
                (Some(login), None) => (
                    address.clone(),
                    login.clone(),
                    services.sync.tokens.get(PASSWORD).unwrap_or_default(),
                ),
                _ => {
                    let (login_name, app_password) =
                        nextcloud_login(services, tx, &address).await?;
                    (address.clone(), login_name, app_password)
                }
            };
            let target =
                nextcloud::webdav_for(&server, &login, Password::new(password.clone()), ROOT)
                    .map_err(|e| SyncProblem::Other(e.to_string()))?;
            let _ = services.sync.tokens.set(PASSWORD, &password);
            spec.user = Some(login);
            Ok(Arc::new(target))
        }
        SyncFolderKind::GoogleDrive | SyncFolderKind::Dropbox | SyncFolderKind::OneDrive => {
            let service = Service::of(&spec.kind).expect("a cloud drive");
            cloud::target(services, service, tx).await
        }
    }
}

/// Sign in to a Nextcloud server through its login page: opened in the browser (said as
/// [`SyncSignInStep::Browser`]), or on a TV shown for a phone to open
/// ([`SyncSignInStep::Page`]). Cancelled as a cloud sign-in is.
async fn nextcloud_login(
    services: &Ctx,
    tx: Option<&Outbox>,
    address: &str,
) -> Result<(String, String), Ended> {
    let kind = SyncFolderKind::Nextcloud;
    // A pass in the background never asks anyone to sign in.
    let Some(tx) = tx else {
        return Err(SyncProblem::Unreachable.into());
    };
    let start =
        nextcloud::start(address).await.map_err(|e| SyncProblem::Other(e.to_string()))?;
    let url = start.login_url.clone();
    if cloud::on_tv(services) {
        cloud::say_for(tx, &kind, Some(SyncSignInStep::Page { url }));
    } else {
        services.host.open_url(&url).map_err(SyncProblem::Other)?;
        cloud::say_for(tx, &kind, Some(SyncSignInStep::Browser { url }));
    }
    let run = services.syncing.cloud.begin(kind.clone(), false);
    let waited = tokio::select! {
        () = run.cancel.notified() => Err(Ended::Cancelled),
        polled = nextcloud::wait(&start, Duration::from_secs(2), LOGIN_WAIT) => Ok(polled),
    };
    services.syncing.cloud.finish(run.id);
    match waited?.map_err(|e| SyncProblem::Other(e.to_string()))? {
        nextcloud::Poll::Done { login_name, app_password, .. } => {
            cloud::say_for(tx, &kind, Some(SyncSignInStep::Finishing));
            Ok((login_name, app_password.expose().to_owned()))
        }
        nextcloud::Poll::Pending => Err(SyncProblem::SignInExpired { service: kind }.into()),
    }
}

pub(crate) fn device_key(services: &Ctx) -> DeviceKey {
    let seed = match services.sync.tokens.get(SEED).ok().and_then(|h| hex::decode(h).ok()) {
        Some(seed) if seed.len() == 32 => seed,
        _ => {
            let seed = keys::new_device_seed().to_vec();
            let _ = services.sync.tokens.set(SEED, &hex::encode(&seed));
            seed
        }
    };
    DeviceKey::from_identity(&seed)
}

pub(crate) fn keep_ring(services: &Ctx, ring: &Keyring) {
    let _ = services.sync.tokens.set(KEYRING, &hex::encode(ring.to_bytes()));
}

fn device_name(services: &Ctx) -> String {
    services.store.sync_meta_get(NAME).ok().flatten().unwrap_or_default()
}

/// Announce this device in the group's log, once enabled and joined.
fn announce_self(services: &Ctx, id: &str, name: &str, key: &DeviceKey) {
    if let Err(e) = services.store.sync_device_add(id, name, &key.public_hex()) {
        tracing::warn!(error = %e, "could not add this device to the sync log");
    }
}

/// How sync stands, from the log and what is kept beside it.
pub fn status(services: &Ctx) -> SyncStatus {
    let store = &services.store;
    let clouds = cloud::views(services);
    let Ok(Some(me)) = store.sync_device() else {
        return SyncStatus { clouds, ..SyncStatus::default() };
    };
    let seen: std::collections::HashMap<String, i64> = store
        .sync_lead_candidates()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|c| c.last_seen.map(|at| (c.device, at)))
        .collect();
    let trackers: std::collections::HashMap<String, Vec<String>> = store
        .sync_lead_candidates()
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.device, c.trackers))
        .collect();
    let devices: Vec<SyncDeviceView> = store
        .sync_peers()
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, _, _, removed)| !removed)
        .map(|(id, name, _, _)| {
            let cloud = device_cloud(store, &id);
            SyncDeviceView {
                this_device: id == me,
                last_seen: seen.get(&id).copied().filter(|at| *at > 0),
                trackers: trackers.get(&id).cloned().unwrap_or_default(),
                sign_ins: cloud.signins.iter().filter_map(|k| kind_of(k)).collect(),
                folder: cloud.folder.as_deref().and_then(kind_of),
                asked: crate::handoff::asked_of(store, &id),
                id,
                name: name.unwrap_or_default(),
            }
        })
        .collect();
    let notes = store
        .sync_notes(0)
        .unwrap_or_default()
        .into_iter()
        .map(|n| SyncNoteView {
            id: n.id,
            anilist_id: n.anilist_id,
            episode: n.episode,
            watch_device: n.watch_device,
            unwatch_device: n.unwatch_device,
            at: n.at,
        })
        .collect();
    let joined: Vec<String> =
        services.stores.rows().unwrap_or_default().into_iter().map(|row| row.url).collect();
    let offers = crate::sync::stores_to_join(&services.store, &joined)
        .unwrap_or_default()
        .into_iter()
        .map(|(address, name)| SyncStoreOffer { address, name })
        .collect();
    let folder = saved_folder(services);
    // This device's folder, else the one most of the others say they use.
    let group_folder = folder.as_ref().map(|f| f.kind.clone()).or_else(|| {
        let mut counts: Vec<(SyncFolderKind, usize)> = Vec::new();
        for kind in devices.iter().filter(|d| !d.this_device).filter_map(|d| d.folder.clone()) {
            match counts.iter_mut().find(|(k, _)| *k == kind) {
                Some((_, n)) => *n += 1,
                None => counts.push((kind, 1)),
            }
        }
        counts.into_iter().max_by_key(|(_, n)| *n).map(|(kind, _)| kind)
    });
    let recovery_key = folder.is_some() && recovery_key(services).is_some();
    SyncStatus {
        enabled: true,
        device_name: Some(device_name(services)).filter(|n| !n.is_empty()),
        folder: folder.map(|spec| SyncFolderView {
            label: label_of(&spec),
            account: account_of(store, &spec),
            kind: spec.kind,
        }),
        group_folder,
        recovery_key,
        recovery_key_made: recovery_key
            .then(|| store.sync_meta_get(RECOVERY_MADE).ok().flatten())
            .flatten()
            .and_then(|v| v.parse().ok()),
        devices,
        last_synced: store.sync_meta_get(LAST).ok().flatten().and_then(|v| v.parse().ok()),
        syncing: services.syncing.running.load(Ordering::Relaxed),
        error: store.sync_meta_get(ERROR).ok().flatten().filter(|e| !e.is_empty()),
        notes,
        stores_to_join: offers,
        clouds,
        problem: saved_problem(services),
    }
}

fn saved_problem(services: &Ctx) -> Option<SyncProblem> {
    let kept = services.store.sync_meta_get(PROBLEM).ok().flatten()?;
    let kind = kind_of(kept.strip_prefix(SIGN_IN)?)?;
    Some(SyncProblem::SignInNeeded { service: kind })
}

pub(crate) fn say_status(services: &Ctx, tx: &Outbox) {
    let _ = tx.send(Event::SyncStatus(status(services)));
}

/// Set sync up with a folder: a new group in an empty folder, or, on a device already in a group
/// (paired nearby), the group's first folder. A folder that already holds this device's group,
/// with keys for this device, is taken as it is.
pub async fn start(services: &Ctx, tx: &Outbox, device_name: String, folder: SyncFolderSpec) {
    let kind = folder.kind.clone();
    start_in(services, tx, device_name, folder).await;
    cloud::ended(tx, &kind);
}

async fn start_in(
    services: &Ctx,
    tx: &Outbox,
    device_name: String,
    mut folder: SyncFolderSpec,
) {
    let target = match target_for(services, Some(tx), &mut folder).await {
        Ok(target) => target,
        Err(Ended::Cancelled) => return say_status(services, tx),
        Err(Ended::Problem(p)) => return fail(services, tx, p),
    };
    let enabled = services.store.sync_device().ok().flatten().is_some();
    let id = device_id(services);
    match settle(services, tx, target, &folder, &id).await {
        Ok(road) => {
            if !enabled {
                if let Err(e) = services.store.sync_enable(&id) {
                    return fail(services, tx, SyncProblem::Other(e.to_string()));
                }
                announce_self(services, &id, &device_name, &device_key(services));
                let _ = services.store.sync_meta_set(NAME, &device_name);
            }
            take_folder(services, &folder, road);
            tracking::commands::announce_signed_in(&services.store, &services.sync);
            pass(services, tx).await;
        }
        Err(p) => fail(services, tx, p),
    }
}

/// The id this device writes under: the one it had, even with sync turned off since, so the log
/// carries on; else a new one.
fn device_id(services: &Ctx) -> String {
    services.store.sync_device_kept().ok().flatten().unwrap_or_else(keys::new_device_id)
}

/// The road to a folder this device is about to sync through. A folder holding the group with
/// keys for this device is opened with them. An empty one gets the group: its keyring and
/// recovery key when this device holds them, else new ones, and a new recovery key is said once.
/// A TV never sets a group's folder up: it only joins one. A folder of another group is
/// [`SyncProblem::GroupExists`].
async fn settle(
    services: &Ctx,
    tx: &Outbox,
    target: Arc<dyn FolderTarget>,
    folder: &SyncFolderSpec,
    id: &str,
) -> Result<FolderRoad, SyncProblem> {
    let key = device_key(services);
    let store = services.store.clone();
    match FolderRoad::open_as_member(target.clone(), "", store.clone(), id, key.clone()).await {
        Ok(road) => return Ok(road),
        Err(SyncError::NoGroup) => {}
        Err(e) => return Err(problem_in(Some(&folder.kind), &e)),
    }
    if cloud::on_tv(services) {
        return Err(SyncProblem::NoGroup);
    }
    let ring = kept_ring(services).unwrap_or_else(Keyring::generate);
    let (recovery, fresh) = match recovery_key(services) {
        Some(key) => (key, false),
        None => (RecoveryKey::generate(), true),
    };
    let road = FolderRoad::create_with(target, "", store, id, key, ring, &recovery)
        .await
        .map_err(|e| problem_in(Some(&folder.kind), &e))?;
    let _ = services.sync.tokens.set(RECOVERY, &recovery.to_text());
    if fresh {
        let _ = services.store.sync_meta_set(RECOVERY_MADE, &kaeri_store::now().to_string());
        let _ = tx.send(Event::SyncRecoveryKey(recovery.to_text()));
    }
    Ok(road)
}

/// Sync through `folder` from now on, on `road`: what was read of another folder is forgotten.
pub(crate) fn take_folder(services: &Ctx, folder: &SyncFolderSpec, road: FolderRoad) {
    let _ = services.store.sync_meta_clear(ROAD_META);
    save_folder(services, folder);
    keep_ring(services, &road.keyring());
    for key in [LAST, ERROR, PROBLEM] {
        let _ = services.store.sync_meta_set(key, "");
    }
    services.syncing.set_road(Some(Arc::new(road)));
    announce_cloud(services);
}

fn kept_ring(services: &Ctx) -> Option<Keyring> {
    services
        .sync
        .tokens
        .get(KEYRING)
        .ok()
        .and_then(|h| hex::decode(h).ok())
        .and_then(|b| Keyring::from_bytes(&b).ok())
}

/// Join the group a folder holds, with its recovery key. A device paired nearby keeps its id and
/// its log, which the folder's group then gets.
pub async fn join(
    services: &Ctx,
    tx: &Outbox,
    device_name: String,
    folder: SyncFolderSpec,
    recovery: &str,
) {
    let kind = folder.kind.clone();
    join_in(services, tx, device_name, folder, recovery).await;
    cloud::ended(tx, &kind);
}

async fn join_in(
    services: &Ctx,
    tx: &Outbox,
    device_name: String,
    mut folder: SyncFolderSpec,
    recovery: &str,
) {
    let recovery_key = match RecoveryKey::parse(recovery) {
        Ok(key) => key,
        Err(e) => return fail(services, tx, problem(&e)),
    };
    let target = match target_for(services, Some(tx), &mut folder).await {
        Ok(target) => target,
        Err(Ended::Cancelled) => return say_status(services, tx),
        Err(Ended::Problem(p)) => return fail(services, tx, p),
    };
    let enabled = services.store.sync_device().ok().flatten().is_some();
    let id = device_id(services);
    let key = device_key(services);
    match FolderRoad::join(target, "", services.store.clone(), &id, key.clone(), &recovery_key)
        .await
    {
        Ok(road) => {
            if !enabled {
                if let Err(e) = services.store.sync_enable(&id) {
                    return fail(services, tx, SyncProblem::Other(e.to_string()));
                }
                announce_self(services, &id, &device_name, &key);
                let _ = services.store.sync_meta_set(NAME, &device_name);
            }
            let _ = services.sync.tokens.set(RECOVERY, &recovery_key.to_text());
            // Made before this device knew it: the date is not known.
            let _ = services.store.sync_meta_remove(RECOVERY_MADE);
            take_folder(services, &folder, road);
            tracking::commands::announce_signed_in(&services.store, &services.sync);
            pass(services, tx).await;
        }
        Err(e) => fail(services, tx, problem_in(Some(&folder.kind), &e)),
    }
}

/// Move this device to another folder, keeping the group, or stop using one with `None` and go
/// on syncing nearby. The old folder is left as it is; the other devices move when they choose
/// the new one, which already holds keys for them.
pub async fn change_folder(services: &Ctx, tx: &Outbox, folder: Option<SyncFolderSpec>) {
    let Some(id) = services.store.sync_device().ok().flatten() else {
        return fail(services, tx, SyncProblem::NoGroup);
    };
    let Some(mut folder) = folder else {
        services.syncing.set_road(None);
        let _ = services.store.sync_meta_clear(ROAD_META);
        for key in [FOLDER, LAST, ERROR, PROBLEM] {
            let _ = services.store.sync_meta_set(key, "");
        }
        let _ = services.sync.tokens.clear(PASSWORD);
        announce_cloud(services);
        return say_status(services, tx);
    };
    let kind = folder.kind.clone();
    let target = match target_for(services, Some(tx), &mut folder).await {
        Ok(target) => target,
        Err(Ended::Cancelled) => {
            cloud::ended(tx, &kind);
            return say_status(services, tx);
        }
        Err(Ended::Problem(p)) => {
            cloud::ended(tx, &kind);
            return fail(services, tx, p);
        }
    };
    match settle(services, tx, target, &folder, &id).await {
        Ok(road) => {
            take_folder(services, &folder, road);
            cloud::ended(tx, &kind);
            pass(services, tx).await;
        }
        Err(p) => {
            cloud::ended(tx, &kind);
            fail(services, tx, p);
        }
    }
}

/// What a device keeps for the cloud, as its `device_cloud` register says.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct DeviceCloud {
    /// Its folder's kind, by [`kind_name`].
    #[serde(default)]
    pub folder: Option<String>,
    /// The sign-ins it can give another device: Dropbox's, and its Nextcloud or WebDAV login.
    #[serde(default)]
    pub signins: Vec<String>,
}

pub(crate) fn device_cloud(store: &kaeri_store::Store, device: &str) -> DeviceCloud {
    store
        .sync_reg_value(kaeri_store::sync::kinds::DEVICE_CLOUD, device)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// The sign-ins this device can give another, by [`kind_name`].
pub(crate) fn giveable(services: &Ctx) -> Vec<String> {
    let mut out = Vec::new();
    if cloud::signed_in(services, Service::Dropbox) {
        out.push(kind_name(&SyncFolderKind::Dropbox).to_owned());
    }
    if let Some(folder) = saved_folder(services)
        && matches!(folder.kind, SyncFolderKind::Nextcloud | SyncFolderKind::WebDav)
        && services.sync.tokens.get(PASSWORD).is_ok_and(|p| !p.is_empty())
    {
        out.push(kind_name(&folder.kind).to_owned());
    }
    out
}

/// Tell the group which folder this device syncs through and what it can give, when it changed.
pub(crate) fn announce_cloud(services: &Ctx) {
    let mine = DeviceCloud {
        folder: saved_folder(services).map(|f| kind_name(&f.kind).to_owned()),
        signins: giveable(services),
    };
    if let Ok(json) = serde_json::to_string(&mine)
        && let Err(e) = services.store.sync_set_my_cloud(&json)
    {
        tracing::warn!(error = %e, "could not tell the group about this device's folder");
    }
}

/// The account a folder is signed in to: the cloud drive's, or the Nextcloud or WebDAV login.
fn account_of(store: &kaeri_store::Store, spec: &SyncFolderSpec) -> Option<String> {
    match Service::of(&spec.kind) {
        Some(service) => cloud::account(store, service),
        None => spec.user.clone().filter(|u| !u.is_empty()),
    }
}

pub(crate) fn fail(services: &Ctx, tx: &Outbox, p: SyncProblem) {
    let _ = tx.send(Event::SyncProblem(p));
    say_status(services, tx);
}

/// The road to the folder, opened from what was kept when it is not up.
async fn road_of(services: &Ctx) -> Result<Arc<FolderRoad>, SyncProblem> {
    if let Some(road) = services.syncing.road() {
        return Ok(road);
    }
    let no_group = || SyncProblem::NoGroup;
    let id = services.store.sync_device().ok().flatten().ok_or_else(no_group)?;
    let mut folder = saved_folder(services).ok_or_else(no_group)?;
    let ring = kept_ring(services).ok_or_else(no_group)?;
    // No `tx`: a pass in the background never asks anyone to sign in.
    let target = match target_for(services, None, &mut folder).await {
        Ok(target) => target,
        Err(Ended::Problem(p)) => return Err(p),
        Err(Ended::Cancelled) => return Err(no_group()),
    };
    let road = Arc::new(FolderRoad::open(
        target,
        "",
        services.store.clone(),
        &id,
        device_key(services),
        ring,
    ));
    services.syncing.set_road(Some(road.clone()));
    Ok(road)
}

/// One pass through the folder: what others wrote is read and applied, this device's own is
/// written. Said as the status before and after. At most one runs at a time.
pub async fn pass(services: &Ctx, tx: &Outbox) {
    if services.syncing.running.swap(true, Ordering::AcqRel) {
        return;
    }
    say_status(services, tx);
    let outcome = match road_of(services).await {
        Ok(road) => road.sync().await.map(|report| (road, report)).map_err(|e| {
            let kind = saved_folder(services).map(|f| f.kind);
            (problem_in(kind.as_ref(), &e), e.to_string())
        }),
        Err(p) => Err((p.clone(), describe(&p))),
    };
    services.syncing.running.store(false, Ordering::Release);
    match outcome {
        Ok((road, report)) => {
            keep_ring(services, &road.keyring());
            let _ = services.store.sync_meta_set(LAST, &kaeri_store::now().to_string());
            let _ = services.store.sync_meta_set(ERROR, "");
            let _ = services.store.sync_meta_set(PROBLEM, "");
            announce_cloud(services);
            carry_out(services, tx, report);
        }
        Err((p, text)) => {
            let _ = services.store.sync_meta_set(ERROR, &text);
            // A problem a person can fix is kept, and said once as it starts.
            if let SyncProblem::SignInNeeded { service } = &p {
                let was = services.store.sync_meta_get(PROBLEM).ok().flatten();
                let now = format!("{SIGN_IN}{}", kind_name(service));
                if was.as_deref() != Some(now.as_str()) {
                    let _ = services.store.sync_meta_set(PROBLEM, &now);
                    let _ = tx.send(Event::SyncProblem(p));
                }
            }
        }
    }
    say_status(services, tx);
}

fn carry_out(services: &Ctx, tx: &Outbox, report: FolderReport) {
    let trackers = tracking::commands::signed_in_tracker_ids(&services.sync);
    crate::sync::fill_tracker_gaps(&report.watches, &trackers, &services.store);
    if !report.effects.is_empty() {
        services
            .syncing
            .effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend(report.effects);
        let _ = tx.internal(Internal::SyncEffects);
    }
    if report.ops_applied > 0 {
        let _ = tx.send(Event::ProgressQueued);
    }
}

/// Leave the group: this device keeps its data and stops writing ops.
pub fn leave(services: &Ctx, tx: &Outbox) {
    services.syncing.set_road(None);
    if let Err(e) = services.store.sync_disable() {
        tracing::warn!(error = %e, "could not turn sync off");
    }
    for id in [KEYRING, RECOVERY, PASSWORD] {
        let _ = services.sync.tokens.clear(id);
    }
    // The cloud drives' sign-ins go with the group.
    cloud::forget_all(services);
    for key in [FOLDER, NAME, LAST, ERROR, PROBLEM] {
        let _ = services.store.sync_meta_set(key, "");
    }
    let _ = services.store.sync_meta_remove(RECOVERY_MADE);
    let _ = services.store.sync_meta_clear(ROAD_META);
    crate::handoff::forget_all(&services.store);
    say_status(services, tx);
}

/// Sign in to a cloud drive and keep the sign-in: "Sign in again" after a refused one, or another
/// account. When sync uses that drive, the road is opened afresh and a pass runs.
pub async fn sign_in_again(services: &Ctx, tx: &Outbox, kind: SyncFolderKind) {
    let Some(service) = Service::of(&kind) else {
        return fail(services, tx, SyncProblem::Other("that is not a cloud drive".into()));
    };
    let outcome = cloud::sign_in(services, tx, service).await;
    sign_in_done(services, tx, &kind, outcome).await;
    cloud::ended(tx, &kind);
}

/// What follows a sign-in on its own: a pass when sync uses the drive, the problem if it failed.
async fn sign_in_done(
    services: &Ctx,
    tx: &Outbox,
    kind: &SyncFolderKind,
    outcome: Result<(), Ended>,
) {
    match outcome {
        Ok(()) => {
            announce_cloud(services);
            let using = saved_folder(services).is_some_and(|f| &f.kind == kind);
            if using {
                services.syncing.set_road(None);
                let _ = services.store.sync_meta_set(PROBLEM, "");
                pass(services, tx).await;
            } else {
                say_status(services, tx);
            }
        }
        Err(Ended::Cancelled) => say_status(services, tx),
        Err(Ended::Problem(p)) => fail(services, tx, p),
    }
}

/// Forget the sign-in to a cloud drive. When sync uses it, the road stops until the person signs
/// in again.
pub fn sign_out(services: &Ctx, tx: &Outbox, kind: &SyncFolderKind) {
    let Some(service) = Service::of(kind) else { return };
    cloud::forget(services, service);
    announce_cloud(services);
    if saved_folder(services).is_some_and(|f| &f.kind == kind) {
        services.syncing.set_road(None);
        let _ = services.store.sync_meta_set(PROBLEM, &format!("{SIGN_IN}{}", kind_name(kind)));
        let _ =
            tx.send(Event::SyncProblem(SyncProblem::SignInNeeded { service: kind.clone() }));
    }
    say_status(services, tx);
}

/// Remove another device and rotate the group key so it reads nothing written from now on.
pub async fn remove_device(services: &Ctx, tx: &Outbox, device: &str) {
    if let Err(e) = services.store.sync_device_remove(device) {
        return fail(services, tx, SyncProblem::Other(e.to_string()));
    }
    if let (Ok(road), Some(recovery)) = (road_of(services).await, recovery_key(services)) {
        match road.rotate_group_key(&recovery).await {
            Ok(ring) => keep_ring(services, &ring),
            Err(e) => return fail(services, tx, problem_at(services, &e)),
        }
    }
    pass(services, tx).await;
}

fn recovery_key(services: &Ctx) -> Option<RecoveryKey> {
    services.sync.tokens.get(RECOVERY).ok().and_then(|t| RecoveryKey::parse(&t).ok())
}

/// Make a new recovery key; the old one stops working at once.
pub async fn new_recovery_key(services: &Ctx, tx: &Outbox) {
    let road = match road_of(services).await {
        Ok(road) => road,
        Err(p) => return fail(services, tx, p),
    };
    match road.new_recovery_key().await {
        Ok(recovery) => {
            let _ = services.sync.tokens.set(RECOVERY, &recovery.to_text());
            let _ =
                services.store.sync_meta_set(RECOVERY_MADE, &kaeri_store::now().to_string());
            let _ = tx.send(Event::SyncRecoveryKey(recovery.to_text()));
        }
        Err(e) => fail(services, tx, problem_at(services, &e)),
    }
}

/// Say the recovery key again, for the sheet behind the device's lock.
pub fn reveal_recovery_key(services: &Ctx, tx: &Outbox) {
    if let Some(key) = recovery_key(services) {
        let _ = tx.send(Event::SyncRecoveryKey(key.to_text()));
    }
}

/// Run passes while the engine runs, when sync is set up.
pub fn keep_syncing(services: &Services, tx: &Outbox) {
    if services.fixture_route {
        return;
    }
    if services.store.sync_device().ok().flatten().is_none() {
        return;
    }
    let (services, tx) = (Ctx::of(services), tx.clone());
    tokio::spawn(async move {
        loop {
            pass(&services, &tx).await;
            tokio::time::sleep(EVERY).await;
        }
    });
}

/// How sync stands in a fixture build's scene. No folder is behind any of them, so a build with
/// fixtures never runs a pass by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureSync {
    /// Set up on a folder, with a second device that was heard from a moment ago and one note
    /// about a watch that was kept.
    Folder,
    /// Paired nearby with the Living room TV, with no cloud folder yet.
    Nearby,
    /// Syncing through Google Drive, signed in as kim@mail.example, with Kim's MacBook Air and
    /// a recovery key made three days ago.
    Cloud,
    /// This device is the Living room TV, paired with Kim's iPhone, which syncs through Dropbox
    /// and holds the sign-in the TV can use. The TV has no folder yet.
    TvPhoneSignIn,
}

/// Sync as a fixture build shows it on a folder: see [`FixtureSync::Folder`].
pub fn seed_fixture(store: &kaeri_store::Store, now: i64) {
    let _ = seed_fixture_as(store, now, FixtureSync::Folder);
}

/// Seed `kind` into `store`. What comes back is the tokens the scene holds, `(id, value)`, for
/// the token store the fixture engine reads.
pub fn seed_fixture_as(
    store: &kaeri_store::Store,
    now: i64,
    kind: FixtureSync,
) -> Vec<(&'static str, String)> {
    let saved = |kind: &str, address: Option<&str>| {
        if let Ok(json) = serde_json::to_string(&Saved {
            kind: kind.into(),
            address: address.map(str::to_owned),
            user: None,
        }) {
            let _ = store.sync_meta_set(FOLDER, &json);
        }
    };
    // Another device's register, as its own ops would have brought it.
    let cloud_of = |device: &str, folder: Option<&str>, signins: &[&str]| {
        let json = serde_json::to_string(&DeviceCloud {
            folder: folder.map(str::to_owned),
            signins: signins.iter().map(|s| (*s).to_owned()).collect(),
        })
        .unwrap_or_default();
        let _ = store.sync_register(
            kaeri_store::sync::kinds::DEVICE_CLOUD,
            device,
            Some(&json),
            None,
        );
    };
    let trackers_of = |device: &str, trackers: &[&str]| {
        let _ = store.sync_fixture_trackers(device, trackers);
    };
    let me = "fixture-me";
    let _ = store.sync_enable(me);
    let mut tokens = Vec::new();
    match kind {
        FixtureSync::Folder => {
            let _ = store.sync_device_add(me, "Kim's iPhone", "00");
            let _ = store.sync_device_add("fixture-tv", "Living room TV", "00");
            let _ = store.sync_touch_peer("fixture-tv", now - 120);
            let _ = store.sync_meta_set(NAME, "Kim's iPhone");
            let _ = store.sync_meta_set(LAST, &(now - 120).to_string());
            saved("dir", Some("Shared/Kaeri"));
            let _ = store.sync_fixture_note(1_000_001, "3", me, "fixture-tv", now - 3_600);
        }
        FixtureSync::Nearby => {
            let _ = store.sync_device_add(me, "Kim's iPhone", "00");
            let _ = store.sync_device_add("fixture-tv", "Living room TV", "00");
            let _ = store.sync_touch_peer("fixture-tv", now - 300);
            trackers_of("fixture-tv", &["anilist"]);
            let _ = store.sync_meta_set(NAME, "Kim's iPhone");
        }
        FixtureSync::Cloud => {
            let _ = store.sync_device_add(me, "Kim's iPhone", "00");
            let _ = store.sync_device_add("fixture-mac", "Kim's MacBook Air", "00");
            let _ = store.sync_touch_peer("fixture-mac", now - 600);
            cloud_of("fixture-mac", Some("google"), &[]);
            let _ = store.sync_meta_set(NAME, "Kim's iPhone");
            let _ = store.sync_meta_set(LAST, &(now - 60).to_string());
            saved("google", None);
            cloud_of(me, Some("google"), &[]);
            cloud::keep_account(store, Service::Google, Some("kim@mail.example"));
            let _ = store.sync_meta_set(RECOVERY_MADE, &(now - 3 * 86_400).to_string());
            tokens.push((Service::Google.token_id(), "fixture-google-refresh".to_owned()));
            tokens.push((RECOVERY, RecoveryKey::generate().to_text()));
            tokens.push((KEYRING, hex::encode(Keyring::generate().to_bytes())));
        }
        FixtureSync::TvPhoneSignIn => {
            let _ = store.sync_device_add(me, "Living room TV", "00");
            let _ = store.sync_device_add("fixture-phone", "Kim's iPhone", "00");
            let _ = store.sync_touch_peer("fixture-phone", now - 120);
            cloud_of("fixture-phone", Some("dropbox"), &["dropbox"]);
            trackers_of("fixture-phone", &["anilist"]);
            let _ = store.sync_meta_set(NAME, "Living room TV");
        }
    }
    tokens
}

const LAN_KEY: &str = "sync-lan-key";

fn lan_me(services: &Ctx) -> Option<Me> {
    let device = services.store.sync_device().ok().flatten()?;
    let identity = match services
        .sync
        .tokens
        .get(LAN_KEY)
        .ok()
        .and_then(|h| hex::decode(h).ok())
        .and_then(|b| Identity::from_pkcs8(&b).ok())
    {
        Some(identity) => identity,
        None => {
            let identity = Identity::generate().ok()?;
            let _ = services.sync.tokens.set(LAN_KEY, &hex::encode(identity.to_pkcs8()));
            identity
        }
    };
    Some(Me {
        identity,
        device,
        name: device_name(services),
        key: device_key(services),
        handoff: Some(Arc::new(crate::handoff::EngineHandoff::of(services))),
    })
}

/// Turn sync on for a device that pairs before it has any folder: a group of its own, which the
/// other device's history then joins.
fn enable_for_pairing(services: &Ctx, device_name: &str) -> Result<(), SyncProblem> {
    if services.store.sync_device().ok().flatten().is_some() {
        return Ok(());
    }
    let id = device_id(services);
    services.store.sync_enable(&id).map_err(|e| SyncProblem::Other(e.to_string()))?;
    announce_self(services, &id, device_name, &device_key(services));
    let _ = services.store.sync_meta_set(NAME, device_name);
    tracking::commands::announce_signed_in(&services.store, &services.sync);
    announce_cloud(services);
    Ok(())
}

/// What a session with another device brought, carried out as a pass through the folder's is.
async fn carry_out_peer(services: &Ctx, tx: &Outbox, report: &PeerReport) {
    if let Some(received) = &report.handoff {
        crate::handoff::take(services, tx, received).await;
    }
    carry_out(
        services,
        tx,
        FolderReport {
            ops_applied: report.applied.ops,
            effects: report.applied.effects.clone(),
            watches: report.applied.watches.clone(),
            ..FolderReport::default()
        },
    );
}

/// Listen for other devices and show a code for one to pair with.
pub async fn invite(services: &Ctx, tx: &Outbox, device_name: String) {
    if let Err(p) = enable_for_pairing(services, &device_name) {
        return fail(services, tx, p);
    }
    let Some(me) = lan_me(services) else {
        return fail(services, tx, SyncProblem::Other("no key for this device".into()));
    };
    let held = services
        .syncing
        .listener
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let listener = match held {
        Some(listener) => listener,
        None => {
            let bound =
                Listener::bind(me, services.store.clone(), ([0, 0, 0, 0], 0).into()).await;
            let listener = match bound {
                Ok(listener) => Arc::new(tokio::sync::Mutex::new(listener)),
                Err(e) => return fail(services, tx, problem(&e)),
            };
            *services
                .syncing
                .listener
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(listener.clone());
            // Sessions end here, whoever opened them: carried out, and the status said again.
            let (watcher, tx, ctx) = (listener.clone(), tx.clone(), services.clone());
            tokio::spawn(async move {
                loop {
                    let session = {
                        let mut guard = watcher.lock().await;
                        guard.next_session().await
                    };
                    match session {
                        Some(Ok(report)) => carry_out_peer(&ctx, &tx, &report).await,
                        Some(Err(_)) => {}
                        None => return,
                    }
                    say_status(&ctx, &tx);
                }
            });
            listener
        }
    };
    let (code, port) = {
        let guard = listener.lock().await;
        (guard.show_code(), guard.port())
    };
    let _ = tx.send(Event::SyncInvite { code, port, seconds: lan::CODE_LIFETIME });
    say_status(services, tx);
}

/// Stop listening: the code is gone and nobody can pair.
pub fn stop_inviting(services: &Ctx) {
    *services.syncing.listener.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

fn address_of(text: &str) -> Result<std::net::SocketAddr, SyncProblem> {
    text.trim().parse().map_err(|_| SyncProblem::Other(format!("not an address: {text}")))
}

/// Pair with the device showing `code` at `address`, then swap histories.
pub async fn pair(services: &Ctx, tx: &Outbox, device_name: String, address: &str, code: &str) {
    if let Err(p) = enable_for_pairing(services, &device_name) {
        return fail(services, tx, p);
    }
    let (Some(me), Ok(addr)) = (lan_me(services), address_of(address)) else {
        return fail(services, tx, SyncProblem::Other("could not reach that device".into()));
    };
    match lan::dial(&me, &services.store, addr, Some(code), None).await {
        Ok(report) => {
            carry_out_peer(services, tx, &report).await;
            say_status(services, tx);
        }
        Err(e) => fail(services, tx, problem(&e)),
    }
}

/// Swap histories with a device already paired.
pub async fn sync_with(services: &Ctx, tx: &Outbox, device: &str, address: &str) {
    let (Some(me), Ok(addr)) = (lan_me(services), address_of(address)) else {
        return fail(services, tx, SyncProblem::Other("could not reach that device".into()));
    };
    let pinned = lan::fingerprint_of(&services.store, device);
    if pinned.is_none() {
        return fail(services, tx, SyncProblem::Other("that device is not paired".into()));
    }
    match lan::dial(&me, &services.store, addr, None, pinned).await {
        Ok(report) => {
            carry_out_peer(services, tx, &report).await;
            say_status(services, tx);
        }
        Err(e) => fail(services, tx, problem(&e)),
    }
}
