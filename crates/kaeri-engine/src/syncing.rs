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
        SyncProblem, SyncStatus, SyncStoreOffer,
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

const KEYRING: &str = "sync-keyring";
const SEED: &str = "sync-seed";
const RECOVERY: &str = "sync-recovery";
const PASSWORD: &str = "sync-password";
const FOLDER: &str = "engine.folder";
const NAME: &str = "engine.name";
const LAST: &str = "engine.last";
const ERROR: &str = "engine.error";
/// A problem a person can fix, kept until a pass works: `signin:<kind>`.
const PROBLEM: &str = "engine.problem";
const SIGN_IN: &str = "signin:";

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

fn kind_name(kind: &SyncFolderKind) -> &'static str {
    match kind {
        SyncFolderKind::Directory => "dir",
        SyncFolderKind::WebDav => "webdav",
        SyncFolderKind::Nextcloud => "nextcloud",
        SyncFolderKind::GoogleDrive => "google",
        SyncFolderKind::Dropbox => "dropbox",
        SyncFolderKind::OneDrive => "onedrive",
    }
}

fn kind_of(name: &str) -> Option<SyncFolderKind> {
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

fn save_folder(services: &Ctx, spec: &SyncFolderSpec) {
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
    let json = services.store.sync_meta_get(FOLDER).ok().flatten()?;
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
async fn target_for(
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
                    let start = nextcloud::start(&address)
                        .await
                        .map_err(|e| SyncProblem::Other(e.to_string()))?;
                    services.host.open_url(&start.login_url).map_err(SyncProblem::Other)?;
                    match nextcloud::wait(&start, Duration::from_secs(2), LOGIN_WAIT)
                        .await
                        .map_err(|e| SyncProblem::Other(e.to_string()))?
                    {
                        nextcloud::Poll::Done { login_name, app_password, .. } => {
                            (address.clone(), login_name, app_password.expose().to_owned())
                        }
                        nextcloud::Poll::Pending => return Err(SyncProblem::Unreachable.into()),
                    }
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

fn device_key(services: &Ctx) -> DeviceKey {
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

fn keep_ring(services: &Ctx, ring: &Keyring) {
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
    let devices = store
        .sync_peers()
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, _, _, removed)| !removed)
        .map(|(id, name, _, _)| SyncDeviceView {
            this_device: id == me,
            last_seen: seen.get(&id).copied().filter(|at| *at > 0),
            id,
            name: name.unwrap_or_default(),
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
    SyncStatus {
        enabled: true,
        device_name: Some(device_name(services)).filter(|n| !n.is_empty()),
        folder: saved_folder(services)
            .map(|spec| SyncFolderView { label: label_of(&spec), kind: spec.kind }),
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

fn say_status(services: &Ctx, tx: &Outbox) {
    let _ = tx.send(Event::SyncStatus(status(services)));
}

/// Set a group up in an empty folder.
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
    let id = keys::new_device_id();
    let key = device_key(services);
    match FolderRoad::create(target, "", services.store.clone(), &id, key.clone()).await {
        Ok((road, recovery)) => {
            if let Err(e) = services.store.sync_enable(&id) {
                return fail(services, tx, SyncProblem::Other(e.to_string()));
            }
            announce_self(services, &id, &device_name, &key);
            let _ = services.store.sync_meta_set(NAME, &device_name);
            save_folder(services, &folder);
            keep_ring(services, &road.keyring());
            let _ = services.sync.tokens.set(RECOVERY, &recovery.to_text());
            tracking::commands::announce_signed_in(&services.store, &services.sync);
            services.syncing.set_road(Some(Arc::new(road)));
            let _ = tx.send(Event::SyncRecoveryKey(recovery.to_text()));
            pass(services, tx).await;
        }
        Err(e) => fail(services, tx, problem_in(Some(&folder.kind), &e)),
    }
}

/// Join the group a folder holds, with its recovery key.
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
    let id = keys::new_device_id();
    let key = device_key(services);
    match FolderRoad::join(target, "", services.store.clone(), &id, key.clone(), &recovery_key)
        .await
    {
        Ok(road) => {
            if let Err(e) = services.store.sync_enable(&id) {
                return fail(services, tx, SyncProblem::Other(e.to_string()));
            }
            announce_self(services, &id, &device_name, &key);
            let _ = services.store.sync_meta_set(NAME, &device_name);
            save_folder(services, &folder);
            keep_ring(services, &road.keyring());
            let _ = services.sync.tokens.set(RECOVERY, &recovery_key.to_text());
            tracking::commands::announce_signed_in(&services.store, &services.sync);
            services.syncing.set_road(Some(Arc::new(road)));
            pass(services, tx).await;
        }
        Err(e) => fail(services, tx, problem_in(Some(&folder.kind), &e)),
    }
}

fn fail(services: &Ctx, tx: &Outbox, p: SyncProblem) {
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
    let ring = services
        .sync
        .tokens
        .get(KEYRING)
        .ok()
        .and_then(|h| hex::decode(h).ok())
        .and_then(|b| Keyring::from_bytes(&b).ok())
        .ok_or_else(no_group)?;
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

/// Sync as a fixture build shows it: set up on a folder, with a second device that was heard
/// from a moment ago and one note about a watch that was kept. No folder is behind it, so a
/// build with fixtures never runs a pass.
pub fn seed_fixture(store: &kaeri_store::Store, now: i64) {
    let _ = store.sync_enable("fixture-me");
    let _ = store.sync_device_add("fixture-me", "Kim's iPhone", "00");
    let _ = store.sync_device_add("fixture-tv", "Living room TV", "00");
    let _ = store.sync_touch_peer("fixture-tv", now - 120);
    let _ = store.sync_meta_set(NAME, "Kim's iPhone");
    let _ = store.sync_meta_set(LAST, &(now - 120).to_string());
    if let Ok(json) = serde_json::to_string(&Saved {
        kind: "dir".into(),
        address: Some("Shared/Kaeri".into()),
        user: None,
    }) {
        let _ = store.sync_meta_set(FOLDER, &json);
    }
    let _ = store.sync_fixture_note(1_000_001, "3", "fixture-me", "fixture-tv", now - 3_600);
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
        pubkey: device_key(services).public_hex(),
    })
}

/// Turn sync on for a device that pairs before it has any folder: a group of its own, which the
/// other device's history then joins.
fn enable_for_pairing(services: &Ctx, device_name: &str) -> Result<(), SyncProblem> {
    if services.store.sync_device().ok().flatten().is_some() {
        return Ok(());
    }
    let id = keys::new_device_id();
    services.store.sync_enable(&id).map_err(|e| SyncProblem::Other(e.to_string()))?;
    announce_self(services, &id, device_name, &device_key(services));
    let _ = services.store.sync_meta_set(NAME, device_name);
    tracking::commands::announce_signed_in(&services.store, &services.sync);
    Ok(())
}

/// What a session with another device brought, carried out as a pass through the folder's is.
fn carry_out_peer(services: &Ctx, tx: &Outbox, report: &PeerReport) {
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
                        Some(Ok(report)) => carry_out_peer(&ctx, &tx, &report),
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
            carry_out_peer(services, tx, &report);
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
            carry_out_peer(services, tx, &report);
            say_status(services, tx);
        }
        Err(e) => fail(services, tx, problem(&e)),
    }
}
