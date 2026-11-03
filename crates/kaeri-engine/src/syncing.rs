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
    folder::{DirTarget, FolderTarget},
    keys::{self, DeviceKey, Keyring, RecoveryKey},
    lan::{self, Identity, Listener, Me},
    nextcloud,
    peer::PeerReport,
    road::{FolderReport, FolderRoad},
    webdav::{Password, WebDavTarget},
};

use crate::{
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

/// The parts of the services a pass needs, owned, so a pass can run on a task of its own.
#[derive(Clone)]
pub struct Ctx {
    pub store: kaeri_store::Store,
    pub sync: tracking::Sync,
    pub host: Arc<dyn Host>,
    pub stores: Stores,
    pub syncing: Arc<SyncHub>,
}

impl Ctx {
    pub fn of(services: &Services) -> Self {
        Self {
            store: services.store.clone(),
            sync: services.sync.clone(),
            host: services.host.clone(),
            stores: services.stores.clone(),
            syncing: services.syncing.clone(),
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
async fn target_for(
    services: &Ctx,
    spec: &mut SyncFolderSpec,
) -> Result<Arc<dyn FolderTarget>, SyncProblem> {
    let address = spec.address.clone().unwrap_or_default();
    match spec.kind {
        SyncFolderKind::Directory => {
            if address.is_empty() {
                return Err(SyncProblem::Other("no folder was given".into()));
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
                        nextcloud::Poll::Pending => return Err(SyncProblem::Unreachable),
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
            Err(SyncProblem::Other("this build has no sign-in for that service".into()))
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
    let Ok(Some(me)) = store.sync_device() else { return SyncStatus::default() };
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
    }
}

fn say_status(services: &Ctx, tx: &Outbox) {
    let _ = tx.send(Event::SyncStatus(status(services)));
}

/// Set a group up in an empty folder.
pub async fn start(
    services: &Ctx,
    tx: &Outbox,
    device_name: String,
    mut folder: SyncFolderSpec,
) {
    let target = match target_for(services, &mut folder).await {
        Ok(target) => target,
        Err(p) => return fail(services, tx, p),
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
        Err(e) => fail(services, tx, problem(&e)),
    }
}

/// Join the group a folder holds, with its recovery key.
pub async fn join(
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
    let target = match target_for(services, &mut folder).await {
        Ok(target) => target,
        Err(p) => return fail(services, tx, p),
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
        Err(e) => fail(services, tx, problem(&e)),
    }
}

fn fail(services: &Ctx, tx: &Outbox, p: SyncProblem) {
    let _ = tx.send(Event::SyncProblem(p));
    say_status(services, tx);
}

/// The road to the folder, opened from what was kept when it is not up.
async fn road_of(services: &Ctx) -> Option<Arc<FolderRoad>> {
    if let Some(road) = services.syncing.road() {
        return Some(road);
    }
    let id = services.store.sync_device().ok().flatten()?;
    let mut folder = saved_folder(services)?;
    let ring = services
        .sync
        .tokens
        .get(KEYRING)
        .ok()
        .and_then(|h| hex::decode(h).ok())
        .and_then(|b| Keyring::from_bytes(&b).ok())?;
    let target = target_for(services, &mut folder).await.ok()?;
    let road = Arc::new(FolderRoad::open(
        target,
        "",
        services.store.clone(),
        &id,
        device_key(services),
        ring,
    ));
    services.syncing.set_road(Some(road.clone()));
    Some(road)
}

/// One pass through the folder: what others wrote is read and applied, this device's own is
/// written. Said as the status before and after. At most one runs at a time.
pub async fn pass(services: &Ctx, tx: &Outbox) {
    if services.syncing.running.swap(true, Ordering::AcqRel) {
        return;
    }
    say_status(services, tx);
    let outcome = match road_of(services).await {
        Some(road) => road.sync().await.map(|report| (road, report)),
        None => Err(SyncError::NoGroup),
    };
    services.syncing.running.store(false, Ordering::Release);
    match outcome {
        Ok((road, report)) => {
            keep_ring(services, &road.keyring());
            let _ = services.store.sync_meta_set(LAST, &kaeri_store::now().to_string());
            let _ = services.store.sync_meta_set(ERROR, "");
            carry_out(services, tx, report);
        }
        Err(e) => {
            let _ = services.store.sync_meta_set(ERROR, &e.to_string());
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
    for key in [FOLDER, NAME, LAST, ERROR] {
        let _ = services.store.sync_meta_set(key, "");
    }
    say_status(services, tx);
}

/// Remove another device and rotate the group key so it reads nothing written from now on.
pub async fn remove_device(services: &Ctx, tx: &Outbox, device: &str) {
    if let Err(e) = services.store.sync_device_remove(device) {
        return fail(services, tx, SyncProblem::Other(e.to_string()));
    }
    if let (Some(road), Some(recovery)) = (road_of(services).await, recovery_key(services)) {
        match road.rotate_group_key(&recovery).await {
            Ok(ring) => keep_ring(services, &ring),
            Err(e) => return fail(services, tx, problem(&e)),
        }
    }
    pass(services, tx).await;
}

fn recovery_key(services: &Ctx) -> Option<RecoveryKey> {
    services.sync.tokens.get(RECOVERY).ok().and_then(|t| RecoveryKey::parse(&t).ok())
}

/// Make a new recovery key; the old one stops working at once.
pub async fn new_recovery_key(services: &Ctx, tx: &Outbox) {
    let Some(road) = road_of(services).await else {
        return fail(services, tx, SyncProblem::NoGroup);
    };
    match road.new_recovery_key().await {
        Ok(recovery) => {
            let _ = services.sync.tokens.set(RECOVERY, &recovery.to_text());
            let _ = tx.send(Event::SyncRecoveryKey(recovery.to_text()));
        }
        Err(e) => fail(services, tx, problem(&e)),
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
