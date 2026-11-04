//! Plugin stores, as the app uses them.
//!
//! `kaeri-plugin-store` speaks the protocol and checks everything that comes back. This is
//! the layer that knows about this machine: where the state file and plugin directory are, which
//! credential store tokens go in, and how the store screens want their rows.

pub mod tasks;

use kaeri_model::StoreMsg;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use kaeri_core::config::{Config, Paths};
use kaeri_model::{
    Msg,
    sources::{ProviderOrigin, ProviderUpdate},
    stores::{
        self, AnonymousDeviceRow, InviteEndReason, InviteLasts, InviteLinkView, InviteUseRow,
        InviteUses, InvitedStore, LocalPluginRow, OpenJoinView, Secret, StoreInviteRow,
        StoreMemberRow, StorePluginRow, StorePluginStatus, StoreRow, StoreState,
    },
};
use kaeri_plugin_store::{
    ClientError, Inspector, Invite, Joined, LocalState, PluginInfo, PluginStatus, StoreClient,
    crypto::{self, PublicKey},
    install,
    local::{self, plugin_file},
    manifest,
    protocol::{
        self as wire, EndReason, Index, IndexEntry, InviteKind, InviteRequest, NO_END_DATE,
        OpenState, PendingInvite, RESERVED_IDS, Role, host_set,
    },
};
use kaeri_track::secret::TokenStore;

pub use kaeri_store_server::WasmInspector;

/// What the store screens and tasks share.
#[derive(Clone)]
pub struct Stores {
    client: StoreClient,
    state_path: PathBuf,
    plugin_dir: PathBuf,
    tokens: TokenStore,
    inspector: Arc<tokio::sync::OnceCell<Arc<WasmInspector>>>,
    config: Config,
    paths: Paths,
    /// Held across every read-modify-write of the state file, so two tasks finishing together
    /// cannot each save a copy that is missing the other's change.
    lock: Arc<tokio::sync::Mutex<()>>,
    /// Why the last refresh of each store failed. Kept only in memory: "offline" is a moment's
    /// state, and the next refresh decides it again.
    offline: Arc<Mutex<HashMap<String, String>>>,
    /// What an invite join calls this device unless the person joining names it otherwise: the
    /// kind of device the app runs on, as [`Self::labelled`] sets it.
    device_label: Arc<str>,
}

/// A link the store would not join with, and how to reach whoever runs it: the store's contact,
/// for the line under the reason ("Contact: …"), when it gave one.
#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    /// Boxed, since a `Result` carries its error by value.
    pub msg: Box<Msg>,
    pub contact: Option<String>,
    /// The store could not be reached: trying again later can work.
    pub offline: bool,
}

impl From<Msg> for Refused {
    fn from(msg: Msg) -> Self {
        Self { msg: Box::new(msg), contact: None, offline: false }
    }
}

/// Why this device could not make an invite for another device of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceInviteRefusal {
    /// The store could not be reached.
    Offline,
    /// The store turned the request down.
    Refused,
    /// This device may not make one: it is not a member of the store at that address, joined it
    /// with the open link, or the store no longer accepts its token.
    NotAllowed,
}

/// A store's members and invite links, for its owner.
#[derive(Debug, Clone, PartialEq)]
pub struct MembersView {
    pub members: Vec<StoreMemberRow>,
    /// The invite links that still work.
    pub invites: Vec<StoreInviteRow>,
    /// Those that ended in the last 30 days, the latest first.
    pub ended: Vec<StoreInviteRow>,
    /// The store's server offers the invite choices.
    pub invite_choices: bool,
    /// How to reach whoever runs the store, when its operator gave one.
    pub contact: Option<String>,
}

/// The state file, next to the database.
pub fn state_path(paths: &Paths) -> PathBuf {
    paths.data_dir.join("plugin-stores.json")
}

impl Stores {
    /// Stores with tokens kept in `tokens`: the engine's, wherever its host keeps them, or a
    /// test's, out of the real keychain.
    pub fn with_tokens(
        config: &Config,
        paths: &Paths,
        tokens: TokenStore,
    ) -> Result<Self, Msg> {
        let timeout = Duration::from_secs(config.network.timeout_secs.max(1));
        Ok(Self {
            client: StoreClient::new(timeout).map_err(foreign)?,
            state_path: state_path(paths),
            plugin_dir: paths.plugin_dir(),
            tokens,
            inspector: Arc::new(tokio::sync::OnceCell::new()),
            config: config.clone(),
            paths: paths.clone(),
            lock: Arc::new(tokio::sync::Mutex::new(())),
            offline: Arc::new(Mutex::new(HashMap::new())),
            device_label: stores::COMPUTER.into(),
        })
    }

    /// These stores with a client that resolves no name, for fixtures and tests: a store joined
    /// or refreshed by name fails to resolve instead of reaching the internet.
    pub fn with_offline_tokens(
        config: &Config,
        paths: &Paths,
        tokens: TokenStore,
    ) -> Result<Self, Msg> {
        let timeout = Duration::from_secs(config.network.timeout_secs.max(1));
        Ok(Self {
            client: StoreClient::offline(timeout).map_err(foreign)?,
            ..Self::with_tokens(config, paths, tokens)?
        })
    }

    /// These stores, calling this device `kind` when it joins with an invite: "iPad", say. The
    /// terminal app's are [`stores::COMPUTER`].
    #[must_use]
    pub fn labelled(self, kind: &str) -> Self {
        Self { device_label: stores::device_label(Some(kind), stores::COMPUTER).into(), ..self }
    }

    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// The WebAssembly host, built the first time a plugin has to be read. Nothing pays for an
    /// engine until someone installs or publishes.
    async fn inspector(&self) -> Result<Arc<WasmInspector>, Msg> {
        self.inspector
            .get_or_try_init(|| async {
                WasmInspector::new(
                    crate::sources::plugin_limits(&self.config),
                    Some(&self.paths.cache_dir.join("wasm")),
                )
                .map(Arc::new)
            })
            .await
            .cloned()
            .map_err(Msg::verbatim)
    }

    fn load(&self) -> Result<LocalState, Msg> {
        LocalState::load(&self.state_path).map_err(foreign)
    }

    /// Change the state file under the lock.
    async fn update<T>(&self, change: impl FnOnce(&mut LocalState) -> T) -> Result<T, Msg> {
        let _held = self.lock.lock().await;
        let mut state = self.load()?;
        let out = change(&mut state);
        state.save(&self.state_path).map_err(foreign)?;
        Ok(out)
    }

    /// A joined store's address and name, for the sync log: never its token.
    pub fn address_of(&self, id: &str) -> Option<(String, String)> {
        self.joined(id).ok().map(|joined| (joined.base, joined.name))
    }

    fn joined(&self, id: &str) -> Result<Joined, Msg> {
        self.load()?.store(id).cloned().ok_or(Msg::Store(StoreMsg::StoreNotJoined))
    }

    fn token(&self, joined: &Joined) -> Result<String, Msg> {
        self.tokens
            .get(&joined.token_key())
            .map_err(|_| Msg::Store(StoreMsg::StoreTokenMissing { store: joined.name.clone() }))
    }

    fn joined_with_token(&self, id: &str) -> Result<(Joined, String), Msg> {
        let joined = self.joined(id)?;
        let token = self.token(&joined)?;
        Ok((joined, token))
    }

    /// Rows for the store screens, from what is on disk. No network.
    pub fn rows(&self) -> Result<Vec<StoreRow>, Msg> {
        let state = self.load()?;
        let offline = self.offline.lock().unwrap_or_else(|e| e.into_inner()).clone();
        Ok(rows(&state, &self.plugin_dir, &offline))
    }

    /// Where each plugin a store installed came from, and the newer version that store offers
    /// when it offers one, by plugin id: what the Providers rows say of a plugin. From what is
    /// on disk, with no network; nothing when the state file cannot be read.
    pub fn origins(&self) -> HashMap<String, (ProviderOrigin, Option<ProviderUpdate>)> {
        let Ok(state) = self.load() else { return HashMap::new() };
        origins(&state, &self.plugin_dir)
    }

    /// Fetch every joined store's plugin list at once, then return the rows.
    ///
    /// A store that cannot be reached keeps the last list it sent and is marked offline; one that
    /// refuses the token is marked as having removed this device.
    pub async fn refresh(&self) -> Result<Vec<StoreRow>, Msg> {
        let state = self.load()?;
        // Nothing joined, nothing to ask — and no state file written just for looking.
        if state.stores.is_empty() {
            return self.rows();
        }
        // Stores that refused this device are asked again too. A refusal is usually final, but
        // one bad answer from a proxy in front of a store should not be.
        let fetches = state.stores.iter().map(|joined| {
            let joined = joined.clone();
            async move {
                let outcome = match self.token(&joined) {
                    Ok(token) => self.fetch_index(&joined, &token).await,
                    Err(e) => Err(Fetch::Failed(e.english())),
                };
                (joined.id, outcome)
            }
        });
        let outcomes = futures::future::join_all(fetches).await;
        let now = now();
        self.update(|state| {
            let mut offline = self.offline.lock().unwrap_or_else(|e| e.into_inner());
            for (id, outcome) in outcomes {
                match outcome {
                    Ok(index) => {
                        state.accept_index(&id, index, now);
                        offline.remove(&id);
                    }
                    Err(Fetch::Revoked) => {
                        state.mark_revoked(&id);
                        offline.remove(&id);
                    }
                    Err(Fetch::Failed(reason)) => {
                        offline.insert(id, reason);
                    }
                }
            }
        })
        .await?;
        self.rows()
    }

    async fn fetch_index(&self, joined: &Joined, token: &str) -> Result<Index, Fetch> {
        let key = joined
            .public_key()
            .ok_or_else(|| Fetch::Failed(Msg::Store(StoreMsg::StoreKeyDamaged).english()))?;
        match self.client.index(&joined.base, token, &key, joined.sequence).await {
            Ok(index) => Ok(index),
            Err(ClientError::Unauthorized) => Err(Fetch::Revoked),
            Err(e) => Err(Fetch::Failed(e.to_string())),
        }
    }

    /// Fetch one store's list, record it, and return it.
    async fn refresh_one(&self, joined: &Joined, token: &str) -> Result<Index, Msg> {
        let outcome = self.fetch_index(joined, token).await;
        let offline = |reason: Option<String>| {
            let mut offline = self.offline.lock().unwrap_or_else(|e| e.into_inner());
            match reason {
                Some(reason) => offline.insert(joined.id.clone(), reason),
                None => offline.remove(&joined.id),
            };
        };
        match outcome {
            Ok(index) => {
                offline(None);
                let accepted = index.clone();
                self.update(|state| state.accept_index(&joined.id, accepted, now())).await?;
                Ok(index)
            }
            Err(Fetch::Revoked) => {
                offline(None);
                self.update(|state| state.mark_revoked(&joined.id)).await?;
                Err(Msg::Store(StoreMsg::StoreDeviceRefused { store: joined.name.clone() }))
            }
            Err(Fetch::Failed(reason)) => {
                offline(Some(reason.clone()));
                Err(Msg::Store(StoreMsg::StoreUnreachable {
                    store: joined.name.clone(),
                    reason,
                }))
            }
        }
    }

    /// Refuse a link to a store already joined at a different address.
    ///
    /// A store is known by its key, and a key is public: anyone can put a joined store's key in a
    /// link to their own server. Joining "again" through such a link would re-point the store's
    /// record at that server and hand it the token being replaced.
    fn same_address(&self, invite: &Invite) -> Result<Option<Joined>, Msg> {
        let previous = self.load()?.store(&invite.key.id()).cloned();
        if let Some(previous) = &previous
            && previous.base != invite.base
        {
            return Err(Msg::Store(StoreMsg::StoreMoved {
                store: previous.name.clone(),
                current: previous.base.clone(),
                target: invite.base.clone(),
            }));
        }
        Ok(previous)
    }

    /// Refuse an open link to a store this machine already joined by name.
    ///
    /// Joining again through the link would swap the member this machine joined as, owner
    /// rights and all, for one with no name, and revoke the token that held them. A store joined
    /// with the link already, or one that removed this device, is joined again as usual.
    fn not_unnamed(invite: &Invite, previous: Option<&Joined>) -> Result<(), Msg> {
        match previous {
            Some(previous) if invite.open && !previous.anonymous && !previous.revoked => {
                Err(Msg::Store(StoreMsg::StoreJoinWouldUnname {
                    store: previous.name.clone(),
                    member: previous.member.name.clone(),
                }))
            }
            _ => Ok(()),
        }
    }

    /// What a pasted link leads to, checked against the key it carries. Spends nothing: an
    /// invite stays unused and an open link joins no one.
    pub async fn preview(&self, link: &str) -> Result<InvitedStore, Msg> {
        self.preview_checked(link).await.map_err(|refused| *refused.msg)
    }

    /// [`Self::preview`], with the store's contact when it refused the link.
    pub async fn preview_checked(&self, link: &str) -> Result<InvitedStore, Refused> {
        let invite = Invite::parse(link).map_err(foreign)?;
        let previous = self.same_address(&invite)?;
        Self::not_unnamed(&invite, previous.as_ref())?;
        let info = self.client.preview(&invite).await.map_err(foreign)?;
        // Who runs it and what it offers, signed by the key the link carries: nothing, from a
        // store that predates the preview. A link that would not join is said now, not after
        // the viewer has said yes to it.
        let previewed =
            self.client.preview_invite(&invite).await.map_err(|e| refused(e, &info.name))?;
        let (index, facts) =
            previewed.map_or((None, None), |(index, facts)| (Some(index), facts));
        let state = self.load()?;
        let id = invite.key.id();
        Ok(InvitedStore {
            link: Secret(invite.link()),
            name: info.name,
            url: invite.base.clone(),
            host: kaeri_model::stores::host(&invite.base),
            fingerprint: invite.key.fingerprint(),
            already_joined: previous.is_some(),
            owners: index.as_ref().map(|index| index.owners.clone()).unwrap_or_default(),
            plugins: index
                .as_ref()
                .map(|index| plugin_rows(&state, &id, index, &self.plugin_dir))
                .unwrap_or_default(),
            open: invite.open || index.as_ref().is_some_and(|index| index.open),
            anonymous: invite.open,
            // An open link sends no name, so there is none to show or change.
            device_label: if invite.open {
                String::new()
            } else {
                self.device_label.to_string()
            },
            names_joiner: !invite.open && facts.is_some_and(|facts| facts.names_joiner()),
        })
    }

    /// Spend an invite, or join with an open link, keep the token, and fetch the store's list.
    ///
    /// An invite sends `label` as this device's name in the store, or the kind of device this is
    /// when it is `None` or blank: never the machine's own name, which is often its owner's.
    pub async fn join(&self, link: &str, label: Option<&str>) -> Result<Joined, Msg> {
        self.join_as(link, label, None).await.map_err(|refused| *refused.msg)
    }

    /// [`Self::join`], with the joiner's own name for an invite that works more than once, and
    /// the store's contact when it refused the link.
    pub async fn join_as(
        &self,
        link: &str,
        label: Option<&str>,
        name: Option<&str>,
    ) -> Result<Joined, Refused> {
        let invite = Invite::parse(link).map_err(foreign)?;
        let previous = self.same_address(&invite)?;
        Self::not_unnamed(&invite, previous.as_ref())?;
        // Checked again right before the invite is spent, not only when it was previewed: the
        // answer is what decides who receives the code.
        let info = self.client.preview(&invite).await.map_err(unreached)?;
        // An open join is sent no label: the store keeps nothing about who joined.
        let label = if invite.open {
            String::new()
        } else {
            stores::device_label(label, &self.device_label)
        };
        // Only a name with something in it is sent: an empty one names them after the invite.
        let name = name.and_then(wire::clean_name);
        let redeemed = self
            .client
            .redeem_as(&invite, &label, name.as_deref())
            .await
            .map_err(|e| refused(e, &info.name))?;

        // Nothing is kept until the server shows it holds the key the link named. Serving a
        // public key from /v1/store proves nothing; only the store itself can sign a list with
        // it. The floor is the highest list this machine has seen from the store, joined or not.
        let id = invite.key.id();
        let floor = {
            let state = self.load()?;
            let remembered = state.sequences.get(&id).copied().unwrap_or(0);
            previous.as_ref().map_or(0, |p| p.sequence).max(remembered)
        };
        let first = self.client.index(&invite.base, &redeemed.token, &invite.key, floor).await;
        if let Err(
            e @ (ClientError::BadSignature
            | ClientError::KeyMismatch
            | ClientError::Rollback { .. }
            | ClientError::Protocol(_)
            | ClientError::Malformed(_)),
        ) = &first
        {
            let _ = self.client.leave(&invite.base, &redeemed.token).await;
            return Err(Msg::Store(StoreMsg::StoreNotTheInvited {
                url: invite.base.clone(),
                reason: e.to_string(),
            })
            .into());
        }

        let old_token = previous.as_ref().and_then(|p| self.tokens.get(&p.token_key()).ok());
        // The token before the record: a record with no token would be a store that looks joined
        // and cannot be used.
        self.tokens
            .set(&local::token_key(&id), &redeemed.token)
            .map_err(|e| Msg::Store(StoreMsg::StoreTokenNotKept { reason: e.to_string() }))?;
        let joined = Joined {
            id: id.clone(),
            base: invite.base.clone(),
            name: info.name,
            key: invite.key.encode(),
            member: redeemed.member,
            joined_at: now(),
            sequence: 0,
            index: None,
            checked_at: None,
            revoked: false,
            anonymous: invite.open,
        };
        let now = now();
        self.update(|state| {
            state.upsert(joined.clone());
            match first {
                Ok(index) => state.accept_index(&id, index, now),
                Err(ClientError::Unauthorized) => state.mark_revoked(&id),
                Err(_) => {}
            }
        })
        .await?;

        // Joined again from this machine: the token it replaces is still live at the store, so it
        // is revoked there — at the address it was issued for, and nowhere else.
        if let (Some(previous), Some(old)) = (&previous, old_token)
            && old != redeemed.token
        {
            let _ = self.client.leave(&previous.base, &old).await;
        }
        Ok(self.joined(&id)?)
    }

    /// Forget a store and revoke this device's token there.
    pub async fn leave(&self, id: &str) -> Result<String, Msg> {
        let joined = self.joined(id)?;
        // Best effort: a store that has gone away must not stop anyone from leaving it.
        if let Ok(token) = self.token(&joined)
            && let Err(e) = self.client.leave(&joined.base, &token).await
        {
            tracing::info!(store = %joined.name, error = %e, "could not revoke the token on leaving");
        }
        let _ = self.tokens.clear(&joined.token_key());
        self.update(|state| state.remove_store(id)).await?;
        self.offline.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
        Ok(joined.name)
    }

    /// Download, verify and install a store's plugin. Returns its entry and where it was written.
    pub async fn install(
        &self,
        store: &str,
        plugin: &str,
    ) -> Result<(IndexEntry, PathBuf), Msg> {
        // The chain swaps a source by id, so a plugin named after a built-in source would replace
        // it — torrent playback and its proxy included.
        if RESERVED_IDS.contains(&plugin) {
            return Err(Msg::Store(StoreMsg::PluginReservedName { plugin: plugin.to_owned() }));
        }
        let (joined, token) = self.joined_with_token(store)?;
        let shown = joined.index.as_ref().and_then(|i| i.plugin(plugin)).cloned();
        let index = self.refresh_one(&joined, &token).await?;
        let entry = index.plugin(plugin).cloned().ok_or_else(|| {
            Msg::Store(StoreMsg::StorePluginWithdrawn {
                store: joined.name.clone(),
                plugin: plugin.to_owned(),
            })
        })?;

        // The user agreed to what the screen showed. If the store changed the plugin in between,
        // that agreement does not cover it: say so, and let the refreshed screen show the change.
        if let Some(shown) = shown
            && (shown.version != entry.version
                || host_set(&shown.allowed_hosts) != host_set(&entry.allowed_hosts))
        {
            return Err(Msg::Store(StoreMsg::StorePluginChanged {
                store: joined.name.clone(),
                plugin: plugin.to_owned(),
            }));
        }
        let state = self.load()?;
        if let PluginStatus::OtherStore(other) = state.status(store, &entry, &self.plugin_dir) {
            let other = state.store(&other).map_or(other.clone(), |s| s.name.clone());
            return Err(Msg::Store(StoreMsg::PluginFromOtherStore {
                plugin: plugin.to_owned(),
                store: other,
            }));
        }
        // A copy added by hand under another file name, declaring the same id. Installing beside
        // it would leave two plugins answering to one name.
        let inspector = self.inspector().await?;
        let target = plugin_file(&self.plugin_dir, plugin);
        if let Some((other, _)) = self
            .local_files(inspector.as_ref())
            .await
            .into_iter()
            .find(|(path, info)| info.id == entry.id && *path != target)
        {
            return Err(Msg::Store(StoreMsg::PluginDuplicateFile {
                path: other.display().to_string(),
                plugin: plugin.to_owned(),
            }));
        }

        let bytes = self
            .client
            .download(&joined.base, &token, &entry)
            .await
            .map_err(|e| install_failed(plugin, e))?;
        let path = install::install(&bytes, &entry, inspector.as_ref(), &self.plugin_dir)
            .await
            .map_err(|e| install_failed(plugin, e))?;
        self.update(|state| state.record_install(store, &entry)).await?;
        Ok((entry, path))
    }

    /// Delete an installed plugin's file and forget where it came from.
    pub async fn remove(&self, plugin: &str) -> Result<(), Msg> {
        install::uninstall(&self.plugin_dir, plugin).await.map_err(foreign)?;
        self.update(|state| state.forget_install(plugin)).await?;
        Ok(())
    }

    /// An invite link: for someone new when `name` is given, for another device of yours when not.
    pub async fn invite(
        &self,
        store: &str,
        name: Option<String>,
    ) -> Result<InviteLinkView, Msg> {
        self.invite_with(store, name, InviteUses::Once, InviteLasts::Week).await
    }

    /// [`Self::invite`], working as many times and for as long as the owner picked. Another
    /// device's link works once for seven days whatever is asked.
    pub async fn invite_with(
        &self,
        store: &str,
        name: Option<String>,
        uses: InviteUses,
        lasts: InviteLasts,
    ) -> Result<InviteLinkView, Msg> {
        let (joined, token) = self.joined_with_token(store)?;
        if joined.anonymous && name.is_none() {
            return Err(Msg::Store(StoreMsg::StoreInviteNeedsLink {
                store: joined.name.clone(),
            }));
        }
        let request = match &name {
            Some(name) => InviteRequest::Member {
                name: name.clone(),
                role: Role::Member,
                uses: match uses {
                    InviteUses::Once => wire::InviteUses::Once,
                    InviteUses::Five => wire::InviteUses::Five,
                    InviteUses::Any => wire::InviteUses::Any,
                },
                lasts: match lasts {
                    InviteLasts::Day => wire::InviteLasts::Day,
                    InviteLasts::Week => wire::InviteLasts::Week,
                    InviteLasts::Month => wire::InviteLasts::Month,
                    InviteLasts::Forever => wire::InviteLasts::Forever,
                },
            },
            None => InviteRequest::Device,
        };
        let created =
            self.client.create_invite(&joined.base, &token, &request).await.map_err(foreign)?;
        // A link carrying a different key would send whoever uses it to a different store.
        let link = Invite::parse(&created.link)
            .map_err(|e| Msg::Store(StoreMsg::StoreLinkMalformed { reason: e.to_string() }))?;
        if Some(link.key) != joined.public_key() {
            return Err(Msg::Store(StoreMsg::StoreLinkWrongKey));
        }
        Ok(InviteLinkView {
            store: store.to_owned(),
            // Rebuilt from its parsed parts rather than shown as the store sent it, so what goes
            // on screen and to the clipboard is exactly a link and nothing else.
            link: Secret(link.link()),
            for_whom: name,
            expires_at: created.expires_at,
            max: created.max_uses,
            until: until(created.expires_at),
        })
    }

    /// A link that adds another device to this device's membership of the store at `address`,
    /// for a paired device of the same person (sync's Join the plugin store). It works once, for
    /// seven days, as any link for another device does. Answers the store's name with it.
    pub async fn device_invite_for(
        &self,
        address: &str,
    ) -> Result<(String, Secret), DeviceInviteRefusal> {
        let state = self.load().map_err(|_| DeviceInviteRefusal::NotAllowed)?;
        let joined = state
            .stores
            .iter()
            .find(|joined| joined.base == address && !joined.revoked)
            .cloned()
            .ok_or(DeviceInviteRefusal::NotAllowed)?;
        // A device that joined with the open link has no name to make an invite for.
        if joined.anonymous {
            return Err(DeviceInviteRefusal::NotAllowed);
        }
        let token = self.token(&joined).map_err(|_| DeviceInviteRefusal::NotAllowed)?;
        let created = self
            .client
            .create_invite(&joined.base, &token, &InviteRequest::Device)
            .await
            .map_err(|e| match e {
                ClientError::Network(_) => DeviceInviteRefusal::Offline,
                ClientError::Unauthorized => DeviceInviteRefusal::NotAllowed,
                ClientError::Server { status: 403, .. } => DeviceInviteRefusal::NotAllowed,
                _ => DeviceInviteRefusal::Refused,
            })?;
        // A link carrying a different key would send whoever uses it to a different store.
        let link = Invite::parse(&created.link).map_err(|_| DeviceInviteRefusal::Refused)?;
        if Some(link.key) != joined.public_key() || link.base != joined.base {
            return Err(DeviceInviteRefusal::Refused);
        }
        Ok((joined.name, Secret(link.link())))
    }

    /// A one-time link that signs a browser in to the store's admin page.
    pub async fn admin_link(&self, store: &str) -> Result<String, Msg> {
        let (joined, token) = self.joined_with_token(store)?;
        let link =
            self.client.admin_link(&joined.base, &token).await.map_err(|e| match e.code() {
                Some("not_found") => {
                    Msg::Store(StoreMsg::StoreAdminMissing { store: joined.name.clone() })
                }
                _ => foreign(e),
            })?;
        // Opened in a browser with owner rights, so it has to be this store's own page, on this
        // store's own address, and nothing a store could slip in instead.
        let expected = format!("{}/admin/#login=", joined.base);
        let code = link.link.strip_prefix(&expected).filter(|code| crypto::is_code(code));
        match code {
            Some(code) => Ok(format!("{expected}{code}")),
            None => {
                Err(Msg::Store(StoreMsg::StoreAdminLinkForeign { store: joined.name.clone() }))
            }
        }
    }

    pub async fn members(
        &self,
        store: &str,
    ) -> Result<(Vec<StoreMemberRow>, Vec<StoreInviteRow>), Msg> {
        let view = self.members_view(store).await?;
        Ok((view.members, view.invites))
    }

    /// A store's members, its invite links that work and those that ended, and whether its
    /// server offers the invite choices.
    pub async fn members_view(&self, store: &str) -> Result<MembersView, Msg> {
        let (joined, token) = self.joined_with_token(store)?;
        let (list, info) = tokio::join!(
            self.client.members(&joined.base, &token),
            self.client.info(&joined.base)
        );
        let list = list.map_err(foreign)?;
        // A store that does not answer about itself is taken for one from before the choices:
        // the sheet then says its invites work once, which is what such a store makes.
        let (invite_choices, contact) = info.map_or((false, None), |info| {
            let contact = info.contact.map(|c| c.trim().to_owned()).filter(|c| !c.is_empty());
            (info.invite_choices, contact)
        });
        let members = list
            .members
            .into_iter()
            .map(|m| StoreMemberRow {
                you: m.id == joined.member.id,
                id: m.id,
                name: m.name,
                owner: m.role == Role::Owner,
                devices: m.devices,
                last_seen: m.last_seen,
            })
            .collect();
        Ok(MembersView {
            members,
            invites: list.invites.into_iter().map(invite_row).collect(),
            ended: list.ended_invites.into_iter().map(invite_row).collect(),
            invite_choices,
            contact,
        })
    }

    /// Who can join a store, its open link, and the devices that joined with it. `None` from a
    /// store whose server has no open joining, which is invite-only and says nothing more.
    pub async fn open_join(&self, store: &str) -> Result<Option<OpenJoinView>, Msg> {
        let (joined, token) = self.joined_with_token(store)?;
        match self.client.open_state(&joined.base, &token).await {
            Ok(state) => Ok(Some(self.open_view(&joined, state)?)),
            Err(ClientError::NotOpen) => Ok(None),
            Err(e) => Err(foreign(e)),
        }
    }

    /// Let anyone with the store's link join, or only people its owners invite. The store's list
    /// is fetched again after, so its row says which.
    pub async fn set_open(&self, store: &str, open: bool) -> Result<OpenJoinView, Msg> {
        let (joined, token) = self.joined_with_token(store)?;
        let state =
            self.client.set_open(&joined.base, &token, open).await.map_err(|e| match e {
                ClientError::NotOpen => Msg::Store(StoreMsg::StoreOpenJoinUnsupported {
                    store: joined.name.clone(),
                }),
                e => foreign(e),
            })?;
        let view = self.open_view(&joined, state)?;
        let _ = self.refresh_one(&joined, &token).await;
        Ok(view)
    }

    /// Remove one device that joined with the store's link, or all of them.
    pub async fn remove_anonymous(
        &self,
        store: &str,
        device: Option<&str>,
    ) -> Result<OpenJoinView, Msg> {
        let (joined, token) = self.joined_with_token(store)?;
        let state = match device {
            Some(device) => {
                self.client
                    .revoke_device(&joined.base, &token, device)
                    .await
                    .map_err(foreign)?;
                self.client.open_state(&joined.base, &token).await
            }
            None => self.client.remove_anonymous(&joined.base, &token).await,
        }
        .map_err(foreign)?;
        self.open_view(&joined, state)
    }

    /// What the store says of who can join, for its owner's screens. The link has to be this
    /// store's own, with its key, at its address: it goes to the clipboard and from there to
    /// whoever the owner sends it to.
    fn open_view(&self, joined: &Joined, state: OpenState) -> Result<OpenJoinView, Msg> {
        let link = match state.link.as_deref() {
            Some(link) => {
                let link = Invite::parse(link).map_err(|e| {
                    Msg::Store(StoreMsg::StoreLinkMalformed { reason: e.to_string() })
                })?;
                if Some(link.key) != joined.public_key() || !link.open {
                    return Err(Msg::Store(StoreMsg::StoreLinkWrongKey));
                }
                Some(Secret(link.link()))
            }
            None => None,
        };
        Ok(OpenJoinView {
            open: state.open && link.is_some(),
            link,
            devices: state.devices,
            recent: state
                .device_list
                .into_iter()
                .map(|device| AnonymousDeviceRow {
                    id: device.id,
                    joined_at: device.created_at,
                    last_seen: device.last_seen,
                })
                .collect(),
        })
    }

    pub async fn remove_member(&self, store: &str, member: &str) -> Result<(), Msg> {
        let (joined, token) = self.joined_with_token(store)?;
        self.client.remove_member(&joined.base, &token, member).await.map_err(foreign)
    }

    pub async fn cancel_invite(&self, store: &str, invite: &str) -> Result<(), Msg> {
        let (joined, token) = self.joined_with_token(store)?;
        self.client.cancel_invite(&joined.base, &token, invite).await.map_err(foreign)
    }

    /// Every plugin in the plugin directory, read for what it is, against what `store` offers.
    pub async fn local_plugins(&self, store: &str) -> Result<Vec<LocalPluginRow>, Msg> {
        let joined = self.joined(store)?;
        let inspector = self.inspector().await?;
        let mut rows: Vec<LocalPluginRow> = self
            .local_files(inspector.as_ref())
            .await
            .into_iter()
            .map(|(_, info)| LocalPluginRow {
                published: joined
                    .index
                    .as_ref()
                    .and_then(|i| i.plugin(&info.id))
                    .map(|e| e.version.clone()),
                id: info.id,
                name: info.name,
                version: info.version,
                hosts: info.allowed_hosts,
            })
            .collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        rows.dedup_by(|a, b| a.id == b.id);
        Ok(rows)
    }

    /// The plugin files in the plugin directory and what each declares. Unloadable ones are left
    /// out: they could not be published anyway.
    async fn local_files(&self, inspector: &dyn Inspector) -> Vec<(PathBuf, PluginInfo)> {
        let Ok(mut entries) = tokio::fs::read_dir(&self.plugin_dir).await else {
            return Vec::new();
        };
        let mut found = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("wasm")) {
                continue;
            }
            let Ok(bytes) = tokio::fs::read(&path).await else { continue };
            match inspector.inspect(&bytes).await {
                Ok(info) => found.push((path, info)),
                Err(e) => tracing::info!(file = %path.display(), error = %e, "not publishable"),
            }
        }
        found
    }

    /// Publish a local plugin to a store.
    pub async fn publish(&self, store: &str, plugin: &str) -> Result<IndexEntry, Msg> {
        let (joined, token) = self.joined_with_token(store)?;
        let inspector = self.inspector().await?;
        // Named after its id first, which is where the store would install it; any other file
        // that declares the id after that.
        let named = plugin_file(&self.plugin_dir, plugin);
        let path = if tokio::fs::try_exists(&named).await.unwrap_or(false) {
            named
        } else {
            self.local_files(inspector.as_ref())
                .await
                .into_iter()
                .find(|(_, info)| info.id == plugin)
                .map(|(path, _)| path)
                .ok_or_else(|| {
                    Msg::Store(StoreMsg::PluginMissingLocally { plugin: plugin.to_owned() })
                })?
        };
        let bytes =
            tokio::fs::read(&path).await.map_err(|e| publish_failed(plugin, foreign(e)))?;
        // Stamped on the way out, so a store that cannot run plugins — a Worker, a serverless
        // function — can still read what this one is.
        let info = inspector
            .inspect(&bytes)
            .await
            .map_err(|e| publish_failed(plugin, Msg::verbatim(e)))?;
        let stamped =
            manifest::stamp(&bytes, &info).map_err(|e| publish_failed(plugin, foreign(e)))?;
        let entry = self
            .client
            .publish(&joined.base, &token, stamped.clone())
            .await
            .map_err(|e| publish_failed(plugin, foreign(e)))?;

        // The owner's own copy is the published one, so the store page should say installed
        // rather than "added by hand". The stamped bytes are what was published, so they replace
        // the file: the same plugin, now carrying its manifest.
        if path == plugin_file(&self.plugin_dir, &entry.id)
            && crypto::sha256_hex(&stamped) == entry.sha256
            && tokio::fs::write(&path, &stamped).await.is_ok()
        {
            self.update(|state| state.record_install(store, &entry)).await?;
        }
        let _ = self.refresh_one(&joined, &token).await;
        Ok(entry)
    }

    pub async fn unpublish(&self, store: &str, plugin: &str) -> Result<(), Msg> {
        let (joined, token) = self.joined_with_token(store)?;
        self.client.unpublish(&joined.base, &token, plugin).await.map_err(foreign)?;
        let _ = self.refresh_one(&joined, &token).await;
        Ok(())
    }

    /// Plugins with an update waiting, by name, for the launch notice.
    pub fn pending_updates(&self) -> Vec<String> {
        self.load()
            .map(|state| {
                state
                    .updates(&self.plugin_dir)
                    .into_iter()
                    .map(|(_, entry)| entry.name)
                    .collect()
            })
            .unwrap_or_default()
    }
}

enum Fetch {
    Revoked,
    Failed(String),
}

/// An error from outside this layer, as the person reads it: the words are its own.
fn foreign(error: impl std::fmt::Display) -> Msg {
    Msg::verbatim(error.to_string())
}

/// A plugin that could not be downloaded or installed, and why.
fn install_failed(plugin: &str, error: impl std::fmt::Display) -> Msg {
    Msg::Store(StoreMsg::PluginInstallFailed {
        plugin: plugin.to_owned(),
        reason: error.to_string(),
    })
}

/// A plugin that could not be published, and why. A reason that is a message of ours is said
/// as it reads.
fn publish_failed(plugin: &str, reason: Msg) -> Msg {
    Msg::Store(StoreMsg::PluginPublishFailed {
        plugin: plugin.to_owned(),
        reason: reason.english(),
    })
}

/// A refused preview or join, in the words the link calls for.
/// An invite link as the owner screens list it.
fn invite_row(invite: PendingInvite) -> StoreInviteRow {
    StoreInviteRow {
        device: invite.kind == InviteKind::Device,
        owner: invite.role == Role::Owner,
        used: invite.uses,
        max: invite.max_uses,
        until: until(invite.expires_at),
        ended: invite.ended.map(|ended| stores::InviteEnded {
            reason: match ended.reason {
                EndReason::UsedUp => InviteEndReason::UsedUp,
                EndReason::RanOut => InviteEndReason::RanOut,
                EndReason::Cancelled => InviteEndReason::Cancelled,
            },
            at: ended.at,
        }),
        uses: invite
            .joined
            .into_iter()
            .map(|used| InviteUseRow {
                member: used.member,
                name: used.name,
                at: used.used_at,
                removed: used.removed,
            })
            .collect(),
        id: invite.id,
        name: invite.name,
        expires_at: invite.expires_at,
    }
}

/// When an invite stops working, or `None` for one with no end date.
fn until(expires_at: i64) -> Option<i64> {
    (expires_at < NO_END_DATE).then_some(expires_at)
}

/// The most owners an error names before it stops: "Ask Alex, Sam or Kim for a new one".
const OWNERS_NAMED: usize = 3;

/// A refusal of a link, in the person's words. An invite that stopped working says why and who
/// to ask, from what the store said: its owners, or for a store that names none, the owners of
/// `store`.
fn refused(error: ClientError, store: &str) -> Refused {
    let ClientError::InviteEnded { reason, at, contact, owners, .. } = error else {
        let offline = matches!(error, ClientError::Network(_));
        return Refused { offline, ..Refused::from(unusable(error)) };
    };
    let named: Vec<&str> = owners
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .take(OWNERS_NAMED)
        .collect();
    let owners = match named.as_slice() {
        [] => format!("the owners of {store}"),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    };
    let msg = match reason {
        EndReason::UsedUp => Msg::Store(StoreMsg::StoreInviteUsedUp { owners }),
        EndReason::RanOut => Msg::Store(StoreMsg::StoreInviteEnded {
            owners,
            when: ended_on(at.unwrap_or_else(now)),
        }),
        EndReason::Cancelled => Msg::Store(StoreMsg::StoreInviteWasCancelled { owners }),
    };
    let contact = contact.map(|c| c.trim().to_owned()).filter(|c| !c.is_empty());
    Refused { msg: Box::new(msg), contact, offline: false }
}

/// A store that did not answer as the person reads it, marked offline when it was the network.
fn unreached(error: ClientError) -> Refused {
    let offline = matches!(error, ClientError::Network(_));
    Refused { offline, ..Refused::from(foreign(error)) }
}

/// The day an invite ended, where this device is: "on 7 November 2026".
fn ended_on(at: i64) -> String {
    jiff::Timestamp::from_second(at)
        .map(|at| {
            at.to_zoned(jiff::tz::TimeZone::system()).strftime("on %-d %B %Y").to_string()
        })
        .unwrap_or_default()
}

fn unusable(error: ClientError) -> Msg {
    match error.code() {
        Some("invite_unusable") => Msg::Store(StoreMsg::StoreInviteUnusable),
        Some("open_unusable") => Msg::Store(StoreMsg::StoreOpenLinkUnusable),
        _ => foreign(error),
    }
}

/// Flatten the state file into the screens' rows.
pub fn rows(
    state: &LocalState,
    plugin_dir: &Path,
    offline: &HashMap<String, String>,
) -> Vec<StoreRow> {
    state
        .stores
        .iter()
        .map(|joined| StoreRow {
            id: joined.id.clone(),
            name: joined.name.clone(),
            url: joined.base.clone(),
            host: kaeri_model::stores::host(&joined.base),
            fingerprint: PublicKey::parse(&joined.key)
                .map(|k| k.fingerprint())
                .unwrap_or_else(|_| "unknown".into()),
            member: joined.member.name.clone(),
            owner: joined.is_owner(),
            state: if joined.revoked {
                StoreState::Revoked
            } else if let Some(reason) = offline.get(&joined.id) {
                StoreState::Offline(reason.clone())
            } else {
                StoreState::Ready
            },
            checked_at: joined.checked_at,
            plugins: joined
                .index
                .as_ref()
                .map(|index| plugin_rows(state, &joined.id, index, plugin_dir))
                .unwrap_or_default(),
            owners: joined.index.as_ref().map(|index| index.owners.clone()).unwrap_or_default(),
            open: joined.index.as_ref().is_some_and(|index| index.open),
            anonymous: joined.anonymous,
        })
        .collect()
}

/// Store `store`'s list as its screens show it, each plugin with where it stands here.
fn plugin_rows(
    state: &LocalState,
    store: &str,
    index: &Index,
    plugin_dir: &Path,
) -> Vec<StorePluginRow> {
    index
        .plugins
        .iter()
        .map(|entry| StorePluginRow {
            id: entry.id.clone(),
            name: entry.name.clone(),
            version: entry.version.clone(),
            hosts: entry.allowed_hosts.clone(),
            status: match state.status(store, entry, plugin_dir) {
                PluginStatus::Available => StorePluginStatus::Available,
                PluginStatus::Installed => StorePluginStatus::Installed,
                PluginStatus::Update { from, new_hosts } => {
                    StorePluginStatus::Update { from, new_hosts }
                }
                PluginStatus::AddedByHand => StorePluginStatus::AddedByHand,
                PluginStatus::OtherStore(other) => StorePluginStatus::OtherStore(
                    state.store(&other).map_or(other, |s| s.name.clone()),
                ),
            },
        })
        .collect()
}

/// [`Stores::origins`], from the state file read.
fn origins(
    state: &LocalState,
    plugin_dir: &Path,
) -> HashMap<String, (ProviderOrigin, Option<ProviderUpdate>)> {
    state
        .installed
        .iter()
        .map(|(id, installed)| {
            // A store left since keeps no name here; the plugin keeps working all the same.
            let joined = state.store(&installed.store);
            let origin = ProviderOrigin::Store {
                id: installed.store.clone(),
                name: joined.map(|store| store.name.clone()),
            };
            let entry = joined.and_then(|store| store.index.as_ref()?.plugin(id));
            let update = entry.and_then(|entry| {
                match state.status(&installed.store, entry, plugin_dir) {
                    PluginStatus::Update { new_hosts, .. } => {
                        Some(ProviderUpdate { version: entry.version.clone(), new_hosts })
                    }
                    _ => None,
                }
            });
            (id.clone(), (origin, update))
        })
        .collect()
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_plugin_store::{
        local::Installed,
        protocol::{Member, PROTOCOL},
    };

    fn entry(id: &str, version: &str) -> IndexEntry {
        IndexEntry {
            id: id.into(),
            name: format!("{id} plugin"),
            version: version.into(),
            allowed_hosts: vec!["api.example".into()],
            translations: Vec::new(),
            sha256: String::new(),
            size: 0,
            published_at: 0,
        }
    }

    #[test]
    fn rows_carry_each_plugins_standing_and_the_stores_condition() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(plugin_file(dir.path(), "old"), b"x").unwrap();
        let key = crypto::SigningKey::generate().unwrap().public();
        let mut state = LocalState::default();
        state.upsert(Joined {
            id: "abc".into(),
            base: "https://plugins.example".into(),
            name: "Ours".into(),
            key: key.encode(),
            member: Member { id: "m".into(), name: "Alex".into(), role: Role::Owner },
            joined_at: 0,
            sequence: 1,
            index: Some(Index {
                protocol: PROTOCOL,
                store: "Ours".into(),
                key: key.encode(),
                sequence: 1,
                issued_at: 0,
                plugins: vec![entry("new", "1.0.0"), entry("old", "2.0.0")],
                owners: vec!["Alex".into(), "Sam".into()],
                open: true,
                contact: None,
            }),
            checked_at: Some(1_760_000_000),
            revoked: false,
            anonymous: false,
        });
        state.installed.insert(
            "old".into(),
            Installed {
                store: "abc".into(),
                version: "1.0.0".into(),
                sha256: String::new(),
                allowed_hosts: Vec::new(),
            },
        );

        let offline = HashMap::from([("abc".to_owned(), "timed out".to_owned())]);
        let rows = rows(&state, dir.path(), &offline);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert!(row.owner);
        assert_eq!(row.fingerprint, key.fingerprint());
        assert_eq!(row.state, StoreState::Offline("timed out".into()));
        assert_eq!(row.plugins[0].status, StorePluginStatus::Available);
        assert_eq!(
            row.plugins[1].status,
            StorePluginStatus::Update {
                from: "1.0.0".into(),
                new_hosts: vec!["api.example".into()]
            }
        );
        assert_eq!(row.updates(), 1);
        assert_eq!(row.host, "plugins.example");
        assert_eq!(row.checked_at, Some(1_760_000_000));
        assert_eq!(row.owners, ["Alex", "Sam"], "who runs it, from its signed list");
        assert!(row.open, "anyone can join it, from its signed list");
        assert!(!row.anonymous);

        // What the Providers rows say of the plugin installed from it, and of its update.
        let store = ProviderOrigin::Store { id: "abc".into(), name: Some("Ours".into()) };
        let update =
            ProviderUpdate { version: "2.0.0".into(), new_hosts: vec!["api.example".into()] };
        let found = origins(&state, dir.path());
        assert_eq!(found.get("old"), Some(&(store, Some(update))));
        assert!(!found.contains_key("new"), "offered, not installed");
        // A store left: the plugin stays, nameless and with no update to offer.
        state.remove_store("abc");
        let left = ProviderOrigin::Store { id: "abc".into(), name: None };
        assert_eq!(origins(&state, dir.path()).get("old"), Some(&(left, None)));
    }
}
