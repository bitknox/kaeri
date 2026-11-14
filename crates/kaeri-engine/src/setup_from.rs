//! "Already use Kaeri on another device?": a new device paired with one already set up asks it
//! for its setup over the paired link, sealed for the asking device as every handoff is (see
//! [`crate::handoff`]).
//!
//! The setup is an invite to every plugin store the other device is a member of (each device
//! still gets its own, see [`crate::store_join`]), each book server with its sign-in, its trackers'
//! sign-ins, and its answer to Watch, read, or both. Folders stay on the device they are on, and
//! are only counted. A sign-in that comes is held in memory until the person picks the servers
//! to add, and is then kept in this device's secure store, never in the config or a log.

use kaeri_model::{
    came_in::CameIn,
    event::Event,
    group::{FirstRunPage, JoinedSetup, SyncBookServerOffer},
    medium::KaeriIsFor,
};
use kaeri_sync::lan::Paired;
use serde::{Deserialize, Serialize};

use crate::{
    outbox::Outbox,
    read_sources::{BookSources, added::ServerGift},
    store_join,
    syncing::{Ctx, SyncHub},
    tracking,
};

/// The ask for the other device's setup.
pub(crate) const SETUP: &str = "setup";

/// Whether a handoff ask is this module's.
pub(crate) fn is_setup_ask(id: &str) -> bool {
    id == SETUP
}

/// The answer to [`SETUP`]. Each field defaults, for an answer from a Kaeri before one of them.
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct SetupGift {
    /// `watch`, `read` or `both`.
    #[serde(default)]
    medium: Option<String>,
    #[serde(default)]
    servers: Vec<ServerGift>,
    /// The trackers it is signed in to, by id. Their sign-ins come as tracker answers.
    #[serde(default)]
    signed_in: Vec<String>,
    #[serde(default)]
    folders: u32,
}

/// A book server a paired device handed over and this device has not added yet.
pub struct HeldServer {
    from: String,
    gift: ServerGift,
}

impl std::fmt::Debug for HeldServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the sign-in.
        f.debug_struct("HeldServer")
            .field("from", &self.from)
            .field("address", &self.gift.address)
            .finish_non_exhaustive()
    }
}

/// The answering side: what this device is set up with.
pub(crate) fn answer(
    syncing: &SyncHub,
    sources: &BookSources,
    sync: &tracking::Sync,
) -> SetupGift {
    SetupGift {
        medium: syncing.medium().map(|m| m.as_str().to_owned()),
        servers: sources.gifts(),
        signed_in: sync
            .trackers
            .iter()
            .filter(|t| t.is_authenticated())
            .map(|t| t.id().to_owned())
            .collect(),
        folders: sources.folder_count(),
    }
}

fn held(syncing: &SyncHub) -> std::sync::MutexGuard<'_, Vec<HeldServer>> {
    syncing.book_servers.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Forget every server held, as when this device leaves the group.
pub fn forget_all(ctx: &Ctx) {
    held(&ctx.syncing).clear();
}

/// The servers held, for `SyncStatus::book_servers_to_add`.
pub(crate) fn offers(ctx: &Ctx) -> Vec<SyncBookServerOffer> {
    held(&ctx.syncing).iter().filter_map(|h| offer(ctx, h)).collect()
}

fn offer(ctx: &Ctx, held: &HeldServer) -> Option<SyncBookServerOffer> {
    let gift = &held.gift;
    let id = gift.id();
    Some(SyncBookServerOffer {
        from: held.from.clone(),
        kind: gift.kind()?,
        name: gift.name.clone(),
        host: kaeri_bookserver::host_of(&gift.address),
        address: gift.address.clone(),
        user: gift.user.clone(),
        key_last4: gift.key_last4.clone(),
        libraries: gift.libraries.clone(),
        added: ctx.book_sources.has_server(&id),
        id,
    })
}

/// The asking side: hold the servers that came, and say what the other device brought and which
/// first-run pages that answers. `copied` are the trackers whose sign-ins were taken in this
/// session.
pub(crate) fn take(ctx: &Ctx, tx: &Outbox, from: &Paired, gift: Option<SetupGift>) {
    let gift = gift.unwrap_or_default();
    {
        let mut held = held(&ctx.syncing);
        held.retain(|h| h.from != from.device);
        for server in gift.servers {
            if server.kind().is_none() {
                continue;
            }
            held.retain(|h| h.gift.id() != server.id());
            held.push(HeldServer { from: from.device.clone(), gift: server });
        }
    }
    let joined = store_join::joined(ctx);
    let stores: Vec<_> = store_join::offers(ctx, &joined)
        .into_iter()
        .filter(|o| o.invite_from.as_deref() == Some(from.device.as_str()))
        .collect();
    let servers: Vec<_> = held(&ctx.syncing)
        .iter()
        .filter(|h| h.from == from.device)
        .filter_map(|h| offer(ctx, h))
        .collect();
    let medium = gift.medium.as_deref().and_then(KaeriIsFor::parse);
    let signed_in_here =
        |id: &str| ctx.sync.trackers.iter().any(|t| t.id() == id && t.is_authenticated());
    let lists =
        !gift.signed_in.is_empty() && gift.signed_in.iter().all(|id| signed_in_here(id));
    let setup = JoinedSetup {
        device: from.device.clone(),
        device_name: from.name.clone(),
        answered: answered(medium.is_some(), lists, !stores.is_empty() || !servers.is_empty()),
        medium,
        stores,
        servers,
        signed_in: gift.signed_in,
        folders: gift.folders,
    };
    let _ = tx.send(Event::SetupFrom(setup));
}

/// The first-run pages a setup answers.
fn answered(medium: bool, lists: bool, sources: bool) -> Vec<FirstRunPage> {
    [
        (medium, FirstRunPage::Medium),
        (lists, FirstRunPage::Lists),
        (sources, FirstRunPage::Sources),
    ]
    .into_iter()
    .filter_map(|(yes, page)| yes.then_some(page))
    .collect()
}

/// `SyncCommand::AskSetupFrom`: ask `device` (or, with `None`, the next device to pair) for its
/// setup, its plugin stores and its trackers' sign-ins; with `address`, open a session with it
/// now.
pub async fn ask(ctx: &Ctx, tx: &Outbox, device: Option<String>, address: Option<String>) {
    crate::handoff::ask_setup(&ctx.store, device.as_deref());
    match (device, address) {
        (Some(device), Some(address)) => {
            crate::syncing::sync_with(ctx, tx, &device, &address).await;
        }
        _ => crate::syncing::say_status(ctx, tx),
    }
}

/// `SyncCommand::AddBookServersFrom`: add the servers picked (every one held from `device` when
/// none is named), each said with `CameIn::ServerAdded`. Their sign-ins go to this device's
/// secure store.
pub fn add_from(ctx: &Ctx, tx: &Outbox, device: &str, servers: &[String], now: i64) {
    let picked: Vec<HeldServer> = {
        let mut held = held(&ctx.syncing);
        let (picked, kept) = std::mem::take(&mut *held).into_iter().partition(|h| {
            h.from == device && (servers.is_empty() || servers.contains(&h.gift.id()))
        });
        *held = kept;
        picked
    };
    let mut added_any = false;
    for server in picked {
        if ctx.book_sources.has_server(&server.gift.id()) {
            continue;
        }
        match ctx.book_sources.add_gift(server.gift, now) {
            Some(added) => {
                added_any = true;
                let _ = tx.send(Event::CameIn(CameIn::ServerAdded(added)));
            }
            None => tracing::warn!("a book server from a paired device could not be added"),
        }
    }
    if added_any {
        let _ = tx.internal(crate::outbox::Internal::BookSourcesChanged);
    }
    crate::syncing::say_status(ctx, tx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setup_answers_only_the_pages_it_brought() {
        assert_eq!(answered(true, false, true), [FirstRunPage::Medium, FirstRunPage::Sources]);
        assert!(answered(false, false, false).is_empty());
    }

    #[test]
    fn an_answer_from_an_older_kaeri_reads_as_an_empty_setup() {
        let gift: SetupGift = serde_json::from_str("{}").unwrap();
        assert!(gift.medium.is_none() && gift.servers.is_empty() && gift.folders == 0);
    }

    #[test]
    fn a_held_server_never_shows_its_sign_in() {
        let held = HeldServer {
            from: "desk".into(),
            gift: serde_json::from_str(
                r#"{"kind":"komga","name":"Komga","address":"https://books.home.example","credential":"basic:c2VjcmV0"}"#,
            )
            .unwrap(),
        };
        let shown = format!("{held:?}");
        assert!(!shown.contains("c2VjcmV0"), "{shown}");
        assert!(shown.contains("books.home.example"));
    }
}
