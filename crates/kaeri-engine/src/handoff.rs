//! Sign-ins one paired device asks another for, carried by the paired link (see
//! [`kaeri_sync::handoff`]): the trackers it is not signed in to (Copy sign-ins), Dropbox's
//! sign-in for a TV, which has no way to sign in to Dropbox itself, and the Nextcloud or WebDAV
//! login of the folder the other device syncs through.
//!
//! What a device asks for is kept until a session with that device answers it, whichever side
//! opens the session. An answer is sealed for the asking device's own key, so only it can open
//! it. A device that receives the login of the folder the group uses, and has no folder yet,
//! takes that folder, with the group's keys that came with it.

use kaeri_model::{
    event::Event,
    group::{SyncCopy, SyncFolderKind, SyncFolderSpec, SyncSignInStep},
    stores::Secret,
};
use kaeri_store::Store;
use kaeri_sync::{
    handoff::{Handoff, Received},
    keys::Keyring,
    lan::Paired,
    road::FolderRoad,
};
use serde::{Deserialize, Serialize};

use crate::{
    cloud::{self, Ended, Service},
    outbox::Outbox,
    syncing::{self, Ctx, KEYRING, PASSWORD, kind_name, kind_of},
    tracking,
};

/// What this device asks of a device, kept per device: `handoff.<device>`.
const WANTS: &str = "handoff.";
/// What it asks of the next device it pairs with.
const NEXT: &str = "next";
const TRACKERS: &str = "trackers";
const TRACKER: &str = "tracker:";
const CLOUD: &str = "cloud:";

fn wants(store: &Store, device: &str) -> Vec<String> {
    store
        .sync_meta_get(&format!("{WANTS}{device}"))
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn set_wants(store: &Store, device: &str, wants: &[String]) {
    let key = format!("{WANTS}{device}");
    let _ = if wants.is_empty() {
        store.sync_meta_remove(&key)
    } else {
        match serde_json::to_string(wants) {
            Ok(json) => store.sync_meta_set(&key, &json),
            Err(_) => return,
        }
    };
}

/// Forget everything asked, as when this device leaves the group.
pub fn forget_all(store: &Store) {
    let _ = store.sync_meta_clear(WANTS);
}

/// The id a copy is asked by, or `None` for a sign-in no other device can give: Google Drive's
/// and OneDrive's belong to the kind of device that got them, and a folder on a disk has none.
fn copy_id(copy: &SyncCopy) -> Option<String> {
    match copy {
        SyncCopy::Trackers => Some(TRACKERS.into()),
        SyncCopy::Cloud { service } => match service {
            SyncFolderKind::Dropbox | SyncFolderKind::Nextcloud | SyncFolderKind::WebDav => {
                Some(format!("{CLOUD}{}", kind_name(service)))
            }
            _ => None,
        },
    }
}

fn copy_of(id: &str) -> Option<SyncCopy> {
    if id == TRACKERS {
        return Some(SyncCopy::Trackers);
    }
    kind_of(id.strip_prefix(CLOUD)?).map(|service| SyncCopy::Cloud { service })
}

/// What this device has asked `device` for and not yet received.
pub fn asked_of(store: &Store, device: &str) -> Vec<SyncCopy> {
    wants(store, device).iter().filter_map(|id| copy_of(id)).collect()
}

/// A tracker's sign-in, as given.
#[derive(Serialize, Deserialize)]
struct TrackerGift {
    id: String,
    access: String,
    refresh: Option<String>,
    expires_at: Option<i64>,
}

/// A folder's sign-in, as given. `ring` (the group keyring, hex) comes with the login of the
/// folder the giving device syncs through, so the asking device can take that folder.
#[derive(Serialize, Deserialize)]
struct CloudGift {
    kind: String,
    secret: String,
    address: Option<String>,
    user: Option<String>,
    account: Option<String>,
    ring: Option<String>,
}

/// An answer: what was given, and the folder sign-ins asked for that the giver lacks.
#[derive(Serialize, Deserialize, Default)]
struct Bundle {
    trackers: Vec<TrackerGift>,
    clouds: Vec<CloudGift>,
    lacking: Vec<String>,
}

/// The engine's side of the paired link's handoff.
pub struct EngineHandoff {
    store: Store,
    sync: tracking::Sync,
}

impl EngineHandoff {
    pub fn of(ctx: &Ctx) -> Self {
        Self { store: ctx.store.clone(), sync: ctx.sync.clone() }
    }

    fn in_group(&self, device: &str) -> bool {
        self.store
            .sync_peers()
            .unwrap_or_default()
            .iter()
            .any(|(id, _, _, removed)| id == device && !removed)
    }

    fn gift_for(&self, kind: &SyncFolderKind) -> Option<CloudGift> {
        let folder = syncing::saved_folder_in(&self.store);
        let mine = folder.as_ref().is_some_and(|f| f.kind == *kind);
        let ring = || mine.then(|| self.sync.tokens.get(KEYRING).ok()).flatten();
        match kind {
            SyncFolderKind::Dropbox => {
                let secret = self
                    .sync
                    .tokens
                    .get(Service::Dropbox.token_id())
                    .ok()
                    .filter(|t| !t.trim().is_empty())?;
                Some(CloudGift {
                    kind: kind_name(kind).into(),
                    secret,
                    address: None,
                    user: None,
                    account: cloud::account(&self.store, Service::Dropbox),
                    ring: ring(),
                })
            }
            SyncFolderKind::Nextcloud | SyncFolderKind::WebDav if mine => {
                let folder = folder?;
                let secret = self.sync.tokens.get(PASSWORD).ok().filter(|p| !p.is_empty())?;
                Some(CloudGift {
                    kind: kind_name(kind).into(),
                    secret,
                    address: folder.address,
                    user: folder.user,
                    account: None,
                    ring: ring(),
                })
            }
            _ => None,
        }
    }
}

impl Handoff for EngineHandoff {
    fn asks(&self, peer: &Paired, just_paired: bool) -> Vec<String> {
        let mut wanted = wants(&self.store, &peer.device);
        if just_paired {
            // What was asked of the next device to pair is this one's to answer.
            for id in wants(&self.store, NEXT) {
                if !wanted.contains(&id) {
                    wanted.push(id);
                }
            }
            set_wants(&self.store, &peer.device, &wanted);
            set_wants(&self.store, NEXT, &[]);
        }
        let mut asks = wanted.clone();
        if wanted.iter().any(|id| id == TRACKERS) {
            asks.extend(
                self.sync
                    .trackers
                    .iter()
                    .filter(|t| !t.is_authenticated())
                    .map(|t| format!("{TRACKER}{}", t.id())),
            );
        }
        asks
    }

    fn answer(&self, peer: &Paired, asks: &[String]) -> Option<Vec<u8>> {
        // Only a device still in the group is given anything.
        if !self.in_group(&peer.device) {
            return None;
        }
        let mut bundle = Bundle::default();
        for ask in asks {
            if let Some(id) = ask.strip_prefix(TRACKER) {
                let signed_in =
                    self.sync.trackers.iter().any(|t| t.id() == id && t.is_authenticated());
                if let (true, Ok(pair)) = (signed_in, self.sync.tokens.get_pair(id))
                    && !pair.access.is_empty()
                {
                    bundle.trackers.push(TrackerGift {
                        id: id.to_owned(),
                        access: pair.access,
                        refresh: pair.refresh,
                        expires_at: pair.expires_at,
                    });
                }
            } else if let Some(kind) = ask.strip_prefix(CLOUD).and_then(kind_of) {
                match self.gift_for(&kind) {
                    Some(gift) => bundle.clouds.push(gift),
                    None => bundle.lacking.push(kind_name(&kind).to_owned()),
                }
            }
        }
        serde_json::to_vec(&bundle).ok()
    }
}

/// Ask `device` (or, with `None`, the next device to pair) for `copy`, replacing what was asked
/// before; with `address`, open a session with it now.
pub async fn copy_sign_ins(
    ctx: &Ctx,
    tx: &Outbox,
    device: Option<String>,
    copy: &[SyncCopy],
    address: Option<String>,
) {
    let ids: Vec<String> = copy.iter().filter_map(copy_id).collect();
    set_wants(&ctx.store, device.as_deref().unwrap_or(NEXT), &ids);
    match (device, address) {
        (Some(device), Some(address)) if !ids.is_empty() => {
            syncing::sync_with(ctx, tx, &device, &address).await;
        }
        _ => syncing::say_status(ctx, tx),
    }
}

/// Take what a device answered: trackers signed in at once, a Dropbox sign-in kept, and the
/// folder the group uses taken when this device has none.
pub async fn take(ctx: &Ctx, tx: &Outbox, received: &Received) {
    let from = received.from.device.clone();
    set_wants(&ctx.store, &from, &[]);
    let Ok(bundle) = serde_json::from_slice::<Bundle>(&received.plain) else {
        tracing::warn!("a paired device's answer could not be read");
        return;
    };
    let mut trackers = Vec::new();
    for gift in bundle.trackers {
        let asked = received.asked.iter().any(|a| a.strip_prefix(TRACKER) == Some(&gift.id));
        if !asked {
            continue;
        }
        let kept = ctx.sync.tokens.set_pair(
            &gift.id,
            &gift.access,
            gift.refresh.as_deref(),
            gift.expires_at,
        );
        if let Err(e) = kept {
            tracing::warn!(tracker = %gift.id, error = %e, "a copied sign-in could not be kept");
            continue;
        }
        let pair = kaeri_track::TokenPair {
            access: gift.access,
            refresh: gift.refresh,
            expires_at: gift.expires_at,
        };
        tracking::activate(&ctx.sync, &gift.id, &pair, tx).await;
        trackers.push(gift.id);
    }
    if !trackers.is_empty() {
        tracking::commands::announce_signed_in(&ctx.store, &ctx.sync);
    }
    let mut clouds = Vec::new();
    for gift in bundle.clouds {
        let Some(kind) = kind_of(&gift.kind) else { continue };
        if !received.asked.iter().any(|a| a.strip_prefix(CLOUD) == Some(gift.kind.as_str())) {
            continue;
        }
        if kind == SyncFolderKind::Dropbox {
            if ctx.sync.tokens.set(Service::Dropbox.token_id(), &gift.secret).is_err() {
                continue;
            }
            cloud::keep_account(&ctx.store, Service::Dropbox, gift.account.as_deref());
        }
        if syncing::saved_folder_in(&ctx.store).is_none()
            && let Some(ring) = gift.ring.as_deref()
        {
            take_folder(ctx, tx, &kind, &gift, ring).await;
        }
        clouds.push(kind);
    }
    syncing::announce_cloud(ctx);
    let lacking = bundle.lacking.iter().filter_map(|k| kind_of(k)).collect();
    let _ = tx.send(Event::SyncSignInsCopied { device: from, trackers, clouds, lacking });
}

/// Sync through the folder a paired device gave the login of, with the group keys it gave.
async fn take_folder(
    ctx: &Ctx,
    tx: &Outbox,
    kind: &SyncFolderKind,
    gift: &CloudGift,
    ring: &str,
) {
    let Some(ring) = hex::decode(ring).ok().and_then(|b| Keyring::from_bytes(&b).ok()) else {
        return;
    };
    let Some(id) = ctx.store.sync_device().ok().flatten() else { return };
    let mut spec = SyncFolderSpec {
        kind: kind.clone(),
        address: gift.address.clone(),
        user: gift.user.clone(),
        password: None,
    };
    match kind {
        SyncFolderKind::WebDav => spec.password = Some(Secret(gift.secret.clone())),
        // A Nextcloud login is kept as its app password, which the target then uses.
        SyncFolderKind::Nextcloud => {
            let _ = ctx.sync.tokens.set(PASSWORD, &gift.secret);
        }
        _ => {}
    }
    cloud::say_for(tx, kind, Some(SyncSignInStep::Finishing));
    let target = match syncing::target_for(ctx, None, &mut spec).await {
        Ok(target) => target,
        Err(Ended::Problem(p)) => {
            cloud::ended(tx, kind);
            return syncing::fail(ctx, tx, p);
        }
        Err(Ended::Cancelled) => return cloud::ended(tx, kind),
    };
    let road =
        FolderRoad::open(target, "", ctx.store.clone(), &id, syncing::device_key(ctx), ring);
    syncing::take_folder(ctx, &spec, road);
    cloud::ended(tx, kind);
    let (ctx, tx) = (ctx.clone(), tx.clone());
    // The first pass runs on its own, so the session that brought the login ends now.
    tokio::spawn(async move { syncing::pass(&ctx, &tx).await });
}
