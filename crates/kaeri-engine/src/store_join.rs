//! Plugin stores joined through a paired device: the device that is a member makes an invite for
//! another device of its own (`InviteRequest::Device`, which any named member may make) and hands
//! it over the paired link, sealed for the asking device as every handoff is (see
//! [`crate::handoff`]). The asking device spends it and gets its own token. A token never travels;
//! the invite does, only inside the link, and only to a device in the group.
//!
//! Two asks ride the handoff:
//!
//! - [`EVERY`]: an invite to every store the other device is a member of. First launch asks the
//!   next device to pair for these. The invites are held in memory, not spent, until the person
//!   picks which stores to join on the success sheet.
//! - [`ONE`] and an address: an invite to that store, spent as soon as it comes.

use std::time::Duration;

use kaeri_model::{
    event::Event,
    group::{SyncStoreOffer, SyncStoreProblem},
    stores::Secret,
};
use kaeri_plugin_store::Invite;
use kaeri_store::sync::kinds;
use serde::{Deserialize, Serialize};

use crate::{
    handoff,
    outbox::Outbox,
    plugin_stores::{DeviceInviteRefusal, Stores},
    syncing::{self, Ctx},
};

/// The ask for an invite to every store the other device is a member of; held when it comes.
pub(crate) const EVERY: &str = "stores";
/// The ask for an invite to the store at the address that follows; spent when it comes.
pub(crate) const ONE: &str = "store:";

/// How long the answering device spends making invites: well inside the time the asking side
/// waits for the answer ([`kaeri_sync::lan::ANSWER`]). Stores not done by then are offline.
const MAKING: Duration = Duration::from_secs(6);

/// Whether a handoff ask is one of this module's.
pub(crate) fn is_store_ask(id: &str) -> bool {
    id == EVERY || id.starts_with(ONE)
}

fn ask_for(address: &str) -> String {
    format!("{ONE}{address}")
}

/// An invite made for the asking device. No `Debug`: the link is a secret.
#[derive(Serialize, Deserialize)]
pub(crate) struct StoreGift {
    address: String,
    name: String,
    link: String,
}

/// Why no invite to a store asked for came.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Why {
    Offline,
    Refused,
    NotAllowed,
}

impl From<DeviceInviteRefusal> for Why {
    fn from(refusal: DeviceInviteRefusal) -> Self {
        match refusal {
            DeviceInviteRefusal::Offline => Self::Offline,
            DeviceInviteRefusal::Refused => Self::Refused,
            DeviceInviteRefusal::NotAllowed => Self::NotAllowed,
        }
    }
}

impl From<Why> for SyncStoreProblem {
    fn from(why: Why) -> Self {
        match why {
            Why::Offline => Self::Offline,
            Why::Refused => Self::Refused,
            Why::NotAllowed => Self::NotAllowed,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct StoreRefusal {
    address: String,
    why: Why,
}

/// An invite handed over and not yet spent, kept in memory only.
#[derive(Clone)]
pub struct Held {
    from: String,
    address: String,
    name: String,
    link: Secret,
}

/// The answering side: an invite for each store asked for, or why not.
pub(crate) async fn answer(
    stores: &Stores,
    asks: &[String],
) -> (Vec<StoreGift>, Vec<StoreRefusal>) {
    let mut wanted: Vec<String> =
        asks.iter().filter_map(|a| a.strip_prefix(ONE)).map(str::to_owned).collect();
    if asks.iter().any(|a| a == EVERY) {
        for row in stores.rows().unwrap_or_default() {
            if !row.anonymous && !wanted.contains(&row.url) {
                wanted.push(row.url);
            }
        }
    }
    let (mut gifts, mut refusals) = (Vec::new(), Vec::new());
    let until = tokio::time::Instant::now() + MAKING;
    for address in wanted {
        match tokio::time::timeout_at(until, stores.device_invite_for(&address)).await {
            Ok(Ok((name, link))) => gifts.push(StoreGift { address, name, link: link.0 }),
            Ok(Err(refusal)) => refusals.push(StoreRefusal { address, why: refusal.into() }),
            Err(_) => refusals.push(StoreRefusal { address, why: Why::Offline }),
        }
    }
    (gifts, refusals)
}

/// The asking side: hold what came for what was asked, and spend the invites to the stores the
/// person picked. A store picked that came with no invite and no reason was asked of a Kaeri
/// that makes none.
pub(crate) async fn take(
    ctx: &Ctx,
    tx: &Outbox,
    from: &str,
    asked: &[String],
    gifts: Vec<StoreGift>,
    refusals: Vec<StoreRefusal>,
) {
    let every = asked.iter().any(|a| a == EVERY);
    let picked: Vec<&str> = asked.iter().filter_map(|a| a.strip_prefix(ONE)).collect();
    for gift in gifts {
        if !every && !picked.contains(&gift.address.as_str()) {
            continue;
        }
        // Only a link to the store it says it is for.
        let fits = Invite::parse(&gift.link)
            .is_ok_and(|invite| invite.base == gift.address && !invite.open);
        if !fits {
            tracing::warn!(store = %gift.address, "a paired device sent an invite to another store");
            continue;
        }
        hold(
            ctx,
            Held {
                from: from.to_owned(),
                address: gift.address,
                name: gift.name,
                link: Secret(gift.link),
            },
        );
    }
    for address in picked {
        match unhold(ctx, from, address) {
            Some(held) => spend(ctx, tx, held).await,
            None => {
                let why = refusals
                    .iter()
                    .find(|r| r.address == address)
                    .map_or(Why::NotAllowed, |r| r.why);
                failed(ctx, tx, from, address, why.into());
            }
        }
    }
}

fn held(ctx: &Ctx) -> std::sync::MutexGuard<'_, Vec<Held>> {
    ctx.syncing.store_invites.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn hold(ctx: &Ctx, invite: Held) {
    let mut held = held(ctx);
    held.retain(|h| h.address != invite.address);
    held.push(invite);
}

fn unhold(ctx: &Ctx, from: &str, address: &str) -> Option<Held> {
    let mut held = held(ctx);
    let at = held.iter().position(|h| h.from == from && h.address == address)?;
    Some(held.remove(at))
}

/// Forget every invite held, as when this device leaves the group.
pub fn forget_all(ctx: &Ctx) {
    held(ctx).clear();
}

/// The addresses of the stores this device is a member of.
fn joined(ctx: &Ctx) -> Vec<String> {
    ctx.stores.rows().unwrap_or_default().into_iter().map(|row| row.url).collect()
}

/// What a store is called, by the group's log or, failing that, its host.
fn name_of(ctx: &Ctx, address: &str) -> String {
    ctx.store
        .sync_regs(kinds::STORE)
        .unwrap_or_default()
        .into_iter()
        .find(|(a, _)| a == address)
        .map(|(_, name)| name)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| kaeri_model::stores::host(address))
}

fn failed(ctx: &Ctx, tx: &Outbox, from: &str, address: &str, problem: SyncStoreProblem) {
    let _ = tx.send(Event::SyncStoreJoinFailed {
        device: from.to_owned(),
        address: address.to_owned(),
        name: name_of(ctx, address),
        problem,
    });
}

/// Spend a held invite. One that could not reach the store is held again, for another try.
async fn spend(ctx: &Ctx, tx: &Outbox, invite: Held) {
    if joined(ctx).contains(&invite.address) {
        return;
    }
    match ctx.stores.join_as(&invite.link.0, None, None).await {
        Ok(store) => {
            // The address and name travel to the person's other devices; the token never does.
            let _ = ctx.store.sync_register(kinds::STORE, &store.base, Some(&store.name), None);
            if let Ok(rows) = ctx.stores.rows() {
                let _ = tx.send(Event::Stores(rows));
            }
            let _ = tx.send(Event::SyncStoreJoined {
                device: invite.from,
                store: store.id,
                address: store.base,
                name: store.name,
            });
        }
        Err(refused) => {
            let problem = if refused.offline {
                SyncStoreProblem::Offline
            } else {
                SyncStoreProblem::Refused
            };
            let _ = tx.send(Event::SyncStoreJoinFailed {
                device: invite.from.clone(),
                address: invite.address.clone(),
                name: invite.name.clone(),
                problem,
            });
            if refused.offline {
                hold(ctx, invite);
            }
        }
    }
}

/// `SyncCommand::JoinStoresFrom`.
pub async fn join_from(
    ctx: &Ctx,
    tx: &Outbox,
    device: Option<String>,
    stores: Vec<String>,
    address: Option<String>,
) {
    let mine = joined(ctx);
    let Some(device) = device else {
        // Asked in the pairing itself, of whichever device pairs next.
        let asks = if stores.is_empty() {
            vec![EVERY.to_owned()]
        } else {
            stores.iter().filter(|a| !mine.contains(a)).map(|a| ask_for(a)).collect()
        };
        handoff::replace_store_asks(&ctx.store, None, &asks);
        return syncing::say_status(ctx, tx);
    };
    let targets: Vec<String> = if stores.is_empty() {
        offers(ctx, &mine)
            .into_iter()
            .filter(|offer| offer.invite_from.as_deref().is_none_or(|from| from == device))
            .map(|offer| offer.address)
            .collect()
    } else {
        stores.into_iter().filter(|a| !mine.contains(a)).collect()
    };
    let mut ask = Vec::new();
    for target in targets {
        match unhold(ctx, &device, &target) {
            Some(invite) => spend(ctx, tx, invite).await,
            None => ask.push(target),
        }
    }
    if ask.is_empty() {
        return syncing::say_status(ctx, tx);
    }
    let asks: Vec<String> = ask.iter().map(|a| ask_for(a)).collect();
    handoff::add_store_asks(&ctx.store, &device, &asks);
    match address {
        Some(address) => {
            if !syncing::sync_with(ctx, tx, &device, &address).await {
                // Nobody to make the invites: said now, and not left waiting for a session.
                handoff::drop_store_asks(&ctx.store, &device, &asks);
                for target in ask {
                    failed(ctx, tx, &device, &target, SyncStoreProblem::Offline);
                }
                syncing::say_status(ctx, tx);
            }
        }
        None => syncing::say_status(ctx, tx),
    }
}

/// The stores another device joined that this one has not, for `SyncStatus::stores_to_join`,
/// each marked with the device whose invite to it is held. A store whose invite came without
/// the log naming it yet is listed too.
pub(crate) fn offers(ctx: &Ctx, joined: &[String]) -> Vec<SyncStoreOffer> {
    let mut offers: Vec<SyncStoreOffer> = crate::sync::stores_to_join(&ctx.store, joined)
        .unwrap_or_default()
        .into_iter()
        .map(|(address, name)| SyncStoreOffer { address, name, invite_from: None })
        .collect();
    for invite in held(ctx).iter().filter(|h| !joined.contains(&h.address)) {
        match offers.iter_mut().find(|o| o.address == invite.address) {
            Some(offer) => offer.invite_from = Some(invite.from.clone()),
            None => offers.push(SyncStoreOffer {
                address: invite.address.clone(),
                name: invite.name.clone(),
                invite_from: Some(invite.from.clone()),
            }),
        }
    }
    offers
}

/// Tell the group about the stores this device joined before sync was on: their addresses and
/// names, never a token.
pub(crate) fn announce_joined(ctx: &Ctx) {
    let known: Vec<String> = ctx
        .store
        .sync_regs(kinds::STORE)
        .unwrap_or_default()
        .into_iter()
        .map(|(address, _)| address)
        .collect();
    for row in ctx.stores.rows().unwrap_or_default() {
        if !known.contains(&row.url) {
            let _ = ctx.store.sync_register(kinds::STORE, &row.url, Some(&row.name), None);
        }
    }
}
