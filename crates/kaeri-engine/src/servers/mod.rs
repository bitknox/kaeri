//! Media servers: the Jellyfin servers the person runs, as sources.
//!
//! What is kept: the list of servers (address, who is signed in, the libraries read, the matches
//! to AniList) as JSON in the database, and the access token in the secure store. The password is
//! never kept. A saved server joins the source chain as one source; its matches are what it
//! answers from, so nothing searches the server at play time.
//!
//! Each server's row also says when it last answered, kept with it, and whether it answered the
//! last time it was asked: loading the servers asks each signed-in one, as do its libraries and
//! its matching. A server that stops answering reads "Not answering · last seen 14:20".

mod matching;
mod record;
mod report;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use kaeri_core::traits::ProviderKind;
use kaeri_jellyfin::{
    Address, ClientInfo, Jellyfin, JellyfinError, JellyfinSource, discovery, model::Session,
    source_id, tls,
};
use kaeri_model::{
    event::Event,
    local::MatchProgress,
    servers::{
        CertificateCheck, CheckedServer, FoundServer, ServerCommand, ServerEvent,
        ServerFailure, ServerRow, ServerStep,
    },
};
use kaeri_net::HttpClient;
use kaeri_providers::ProviderRegistry;
use kaeri_store::Store;
use kaeri_track::TokenStore;
use tokio::task::AbortHandle;

use crate::{outbox::Outbox, services::Services};

#[cfg(feature = "testing")]
pub use record::seed_fixture_busy;
pub use record::{Record, seed_fixture};

/// The servers saved in `store`.
pub fn saved(store: &Store) -> Vec<Record> {
    record::load(store)
}
pub use report::{Report, Reports};

/// How long a removed server is held for Undo: the notice's 8 seconds, and a little more for a
/// tap at the last moment to arrive.
pub const UNDO: Duration = Duration::from_secs(10);

/// How long loading the servers waits for each to answer before it counts as not answering.
const PROBE: Duration = Duration::from_secs(8);

/// The handles the servers need, cheap to clone.
#[derive(Clone)]
pub struct Servers {
    pub(crate) tokens: TokenStore,
    store: Store,
    http: HttpClient,
    quick: Arc<Mutex<Option<AbortHandle>>>,
    /// How far each server's matching has gone, while it runs.
    progress: Arc<Mutex<HashMap<String, MatchProgress>>>,
    /// The timers that forget removed servers, so Undo can stop one.
    undo: Arc<Mutex<HashMap<String, AbortHandle>>>,
    /// Where the broadcast goes. The local network's, except in a test.
    target: std::net::SocketAddr,
}

impl Servers {
    pub fn new(tokens: TokenStore, store: Store, http: HttpClient) -> Self {
        let servers = Self {
            tokens,
            store,
            http,
            quick: Default::default(),
            progress: Default::default(),
            undo: Default::default(),
            target: discovery::broadcast_target(),
        };
        #[cfg(feature = "testing")]
        if let Some((id, numbers)) =
            servers.store.get_meta_text(record::FIXTURE_MATCHING).ok().flatten().and_then(
                |text| text.split_once('|').map(|(a, b)| (a.to_owned(), b.to_owned())),
            )
        {
            let numbers: Vec<u32> = numbers.split(',').filter_map(|n| n.parse().ok()).collect();
            if let [done, total] = numbers[..] {
                servers
                    .set_progress(&id, Some(MatchProgress { done, total, wait_seconds: None }));
            }
        }
        servers
    }

    /// The same, with the broadcast aimed at `target`.
    pub fn discovering_at(mut self, target: std::net::SocketAddr) -> Self {
        self.target = target;
        self
    }

    pub fn reports(&self) -> Reports {
        Reports::new(self.clone())
    }

    fn token_key(server: &str) -> String {
        source_id(server)
    }

    /// The device as Jellyfin's dashboard lists it. The id is made once and kept.
    fn client_info(&self) -> ClientInfo {
        const KEY: &str = "servers:device";
        let device_id = match self.store.get_meta_text(KEY).ok().flatten() {
            Some(id) => id,
            None => {
                let id = format!("{:032x}", rand::random::<u128>());
                let _ = self.store.set_meta_text(KEY, &id);
                id
            }
        };
        ClientInfo {
            device: format!("Kaeri on {}", std::env::consts::OS),
            device_id,
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }

    /// A client for `address`, trusting the certificate the person allowed for it, if any.
    fn client(&self, address: Address) -> Jellyfin {
        let pinned = record::pins(&self.store)
            .get(address.base())
            .and_then(|fingerprint| tls::pinned_client(fingerprint).ok());
        match pinned {
            Some(http) => Jellyfin::with_http(http, address, self.client_info()),
            None => Jellyfin::new(&self.http, address, self.client_info()),
        }
    }

    /// A saved server's client, signed in with its kept token.
    pub(crate) fn signed_in(&self, record: &Record) -> Option<Jellyfin> {
        let address = Address::parse(&record.address).ok()?;
        let token = self.tokens.get(&Self::token_key(&record.id)).ok()?;
        Some(self.client(address).with_session(Session {
            token,
            user_id: record.user_id.clone(),
            user_name: record.user_name.clone(),
        }))
    }

    pub(crate) fn records(&self) -> Vec<Record> {
        record::load(&self.store)
    }

    /// The saved servers as sources, in the chain. Called once at launch.
    pub fn join_chain(&self, registry: &ProviderRegistry, config: &kaeri_core::config::Config) {
        // A server removed just before the app last quit is forgotten now.
        for held in record::held(&self.store) {
            self.forget(&held.id);
        }
        for record in self.records() {
            self.put_source(&record, registry, &config.providers.order);
        }
    }

    fn put_source(&self, record: &Record, registry: &ProviderRegistry, order: &[String]) {
        let Some(client) = self.signed_in(record).or_else(|| self.scene_client(record)) else {
            return;
        };
        let source = JellyfinSource::new(&record.id, &record.name, client, record.matches());
        registry.put(Arc::new(source));
        registry.rank_by(|p| crate::sources::rank(p, order));
    }

    /// For a scene: a server it seeded has no kept token, but still joins the chain, so the
    /// Sources list carries its row as the real engine's does. A signed-out server stays out.
    #[cfg(feature = "testing")]
    fn scene_client(&self, record: &Record) -> Option<Jellyfin> {
        if record.signed_out {
            return None;
        }
        let address = Address::parse(&record.address).ok()?;
        let session = Session {
            token: String::new(),
            user_id: record.user_id.clone(),
            user_name: record.user_name.clone(),
        };
        Some(self.client(address).with_session(session))
    }

    #[cfg(not(feature = "testing"))]
    fn scene_client(&self, _record: &Record) -> Option<Jellyfin> {
        None
    }

    fn rows(&self) -> Vec<ServerRow> {
        let progress = self.progress.lock().unwrap_or_else(|e| e.into_inner()).clone();
        self.records()
            .iter()
            .map(|r| ServerRow { matching: progress.get(&r.id).cloned(), ..r.row() })
            .collect()
    }

    fn set_progress(&self, server: &str, progress: Option<MatchProgress>) {
        let mut map = self.progress.lock().unwrap_or_else(|e| e.into_inner());
        match progress {
            Some(p) => map.insert(server.to_owned(), p),
            None => map.remove(server),
        };
    }

    /// Forget a held server for good: its sign-in token, and the certificate allowed for its
    /// address when no other server uses it.
    fn forget(&self, server: &str) {
        let Some(held) = record::unhold(&self.store, server) else { return };
        let _ = self.tokens.clear(&Self::token_key(server));
        if !self.records().iter().any(|r| r.address == held.address) {
            record::set_pin(&self.store, &held.address, None);
        }
    }

    /// Note what became of asking `server`: it answered, or it did not. Any answer counts,
    /// a refusal too; a certificate this device no longer trusts says nothing either way.
    fn heard<T>(&self, server: &str, result: &Result<T, JellyfinError>) {
        let answered = match result {
            Ok(_) => true,
            Err(JellyfinError::Unreachable { .. }) => false,
            Err(JellyfinError::UntrustedCertificate { .. }) => return,
            Err(_) => true,
        };
        let now = crate::now_epoch();
        record::update(&self.store, server, |r| r.heard(answered, now));
    }

    /// Ask each signed-in server whether it answers, and say the rows again as each does.
    fn probe(&self, tx: &Outbox) {
        for record in self.records().into_iter().filter(|r| !r.signed_out) {
            // A scene's servers have no kept token, so they are never asked.
            let Some(client) = self.signed_in(&record) else { continue };
            let (servers, tx) = (self.clone(), tx.clone());
            tokio::spawn(async move {
                let result =
                    tokio::time::timeout(PROBE, client.public_info()).await.unwrap_or_else(
                        |_| Err(JellyfinError::Unreachable { address: record.address.clone() }),
                    );
                servers.heard(&record.id, &result);
                servers.publish(&tx);
            });
        }
    }

    fn publish(&self, tx: &Outbox) {
        let _ = tx.send(Event::Server(ServerEvent::Servers(self.rows())));
    }
}

fn failed(tx: &Outbox, step: ServerStep, error: &JellyfinError) {
    let _ = tx.send(Event::Server(ServerEvent::Failed { step, failure: failure(error) }));
}

/// [`failed`], and for a certificate this device does not trust, the fingerprint to compare.
async fn failed_at(tx: &Outbox, step: ServerStep, error: &JellyfinError, typed: &str) {
    failed(tx, step, error);
    let JellyfinError::UntrustedCertificate { host } = error else { return };
    let Ok(address) = Address::parse(typed) else { return };
    if let Ok(fingerprint) = tls::peer_fingerprint(&address).await {
        let _ = tx.send(Event::Server(ServerEvent::Certificate(CertificateCheck {
            host: host.clone(),
            address: address.base().to_owned(),
            groups: tls::groups(&fingerprint),
            fingerprint,
        })));
    }
}

/// A failure as the apps are told it.
pub fn failure(error: &JellyfinError) -> ServerFailure {
    match error {
        JellyfinError::Unreachable { address } => {
            ServerFailure::Unreachable { address: address.clone() }
        }
        JellyfinError::NotJellyfin => ServerFailure::NotJellyfin,
        JellyfinError::TooOld { version } => ServerFailure::TooOld { version: version.clone() },
        JellyfinError::BadCredentials => ServerFailure::Credentials,
        JellyfinError::QuickConnectOff => ServerFailure::QuickConnectOff,
        JellyfinError::QuickConnectExpired => ServerFailure::QuickConnectExpired,
        JellyfinError::PlainHttp => ServerFailure::PlainHttp,
        JellyfinError::UntrustedCertificate { host } => {
            ServerFailure::Certificate { host: host.clone() }
        }
        JellyfinError::SignedOut => ServerFailure::SignedOut,
        JellyfinError::BadAddress => ServerFailure::BadAddress,
        JellyfinError::Status { .. } | JellyfinError::Decode(_) => ServerFailure::Other,
    }
}

/// Run one command. Everything that waits on the network runs as a task.
pub fn command(command: ServerCommand, services: &Services, order: Vec<String>, tx: &Outbox) {
    let servers = services.servers.clone();
    let (registry, meta, store) =
        (services.registry.clone(), services.meta.clone(), services.store.clone());
    let tx = tx.clone();
    match command {
        ServerCommand::Load => {
            servers.publish(&tx);
            servers.probe(&tx);
        }
        ServerCommand::Discover => {
            tokio::spawn(async move {
                let found = tx.clone();
                discovery::discover(servers.target, discovery::WINDOW, move |server| {
                    let _ = found.send(Event::Server(ServerEvent::Found(FoundServer {
                        id: server.id,
                        name: server.name,
                        address: server.address,
                    })));
                })
                .await;
                let _ = tx.send(Event::Server(ServerEvent::DiscoveryDone));
            });
        }
        ServerCommand::Check { address } => {
            tokio::spawn(async move {
                match check(&servers, &address).await {
                    Ok((checked, _)) => {
                        let _ = tx.send(Event::Server(ServerEvent::Checked(checked)));
                    }
                    Err(e) => failed_at(&tx, ServerStep::Address, &e, &address).await,
                }
            });
        }
        ServerCommand::TrustCertificate { address, fingerprint } => {
            tokio::spawn(async move {
                let parsed = match Address::parse(&address) {
                    Ok(parsed) => parsed,
                    Err(e) => return failed(&tx, ServerStep::Address, &e),
                };
                record::set_pin(&store, parsed.base(), Some(tls::normal(&fingerprint)));
                match check(&servers, &address).await {
                    Ok((checked, _)) => {
                        let _ = tx.send(Event::Server(ServerEvent::Checked(checked)));
                    }
                    Err(e) => failed_at(&tx, ServerStep::Address, &e, &address).await,
                }
            });
        }
        ServerCommand::SignIn { address, user, password } => {
            tokio::spawn(async move {
                let result = async {
                    let (checked, client) = check(&servers, &address).await?;
                    let session = client.sign_in(&user, &password).await?;
                    Ok::<_, JellyfinError>((checked, session))
                }
                .await;
                match result {
                    Ok((checked, session)) => {
                        finish_sign_in(&servers, checked, session, &registry, &order, &tx)
                    }
                    Err(e) => failed_at(&tx, ServerStep::SignIn, &e, &address).await,
                }
            });
        }
        ServerCommand::StartQuickConnect { address } => {
            let again = servers.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = quick_connect(&servers, &address, &registry, &order, &tx).await
                {
                    failed_at(&tx, ServerStep::QuickConnect, &e, &address).await;
                }
            });
            let mut slot = again.quick.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(old) = slot.replace(handle.abort_handle()) {
                old.abort();
            }
        }
        ServerCommand::CancelQuickConnect => {
            let mut slot = servers.quick.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(old) = slot.take() {
                old.abort();
            }
        }
        ServerCommand::LoadLibraries { server } => {
            tokio::spawn(async move {
                let Some(record) = servers.records().into_iter().find(|r| r.id == server)
                else {
                    return;
                };
                let Some(client) = servers.signed_in(&record) else {
                    failed(&tx, ServerStep::Libraries, &JellyfinError::SignedOut);
                    return;
                };
                let choices = matching::choices(&client, &record).await;
                servers.heard(&server, &choices);
                match choices {
                    Ok(libraries) => {
                        let _ = tx
                            .send(Event::Server(ServerEvent::Libraries { server, libraries }));
                    }
                    Err(e) => failed(&tx, ServerStep::Libraries, &e),
                }
            });
        }
        ServerCommand::SetLibraries { server, libraries } => {
            tokio::spawn(async move {
                let Some(record) = servers.records().into_iter().find(|r| r.id == server)
                else {
                    return;
                };
                let Some(client) = servers.signed_in(&record) else {
                    failed(&tx, ServerStep::Libraries, &JellyfinError::SignedOut);
                    return;
                };
                let all = client.libraries().await;
                servers.heard(&server, &all);
                match all {
                    Ok(all) => record::update(&store, &server, |r| {
                        r.libraries = all
                            .into_iter()
                            .map(|l| record::LibraryRecord {
                                ticked: libraries.contains(&l.id),
                                id: l.id,
                                name: l.name,
                                kind: l.collection_type,
                            })
                            .collect();
                        r.libraries_chosen = true;
                    }),
                    Err(e) => return failed(&tx, ServerStep::Libraries, &e),
                }
                servers.publish(&tx);
                sync(servers, server, registry, meta, store, order, tx);
            });
        }
        ServerCommand::Refresh { server } => {
            sync(servers, server, registry, meta, store, order, tx)
        }
        ServerCommand::SetReport { server, report } => {
            record::update(&store, &server, |r| r.report = report);
            servers.publish(&tx);
        }
        ServerCommand::Remove { server } => {
            // Held, token and all, so Undo needs no password. Forgotten after the notice.
            if let Some(removed) = record::remove(&store, &server) {
                record::hold(&store, removed);
                let later = servers.clone();
                let id = server.clone();
                let timer = tokio::spawn(async move {
                    tokio::time::sleep(UNDO).await;
                    later.forget(&id);
                });
                let mut undo = servers.undo.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(earlier) = undo.insert(server.clone(), timer.abort_handle()) {
                    earlier.abort();
                }
            }
            registry.remove(&source_id(&server));
            servers.set_progress(&server, None);
            servers.publish(&tx);
        }
        ServerCommand::Restore { server } => {
            let timer = servers.undo.lock().unwrap_or_else(|e| e.into_inner()).remove(&server);
            if let Some(timer) = timer {
                timer.abort();
            }
            if let Some(held) = record::unhold(&store, &server) {
                record::upsert(&store, held.clone());
                servers.put_source(&held, &registry, &order);
            }
            servers.publish(&tx);
        }
        ServerCommand::SignOut { server } => {
            tokio::spawn(async move {
                let Some(record) = servers.records().into_iter().find(|r| r.id == server)
                else {
                    return;
                };
                if let Some(client) = servers.signed_in(&record) {
                    // Ends the token on the server too. Best effort: one that is switched off
                    // still has it deleted here.
                    let _ = client.sign_out().await;
                }
                let _ = servers.tokens.clear(&Servers::token_key(&server));
                record::update(&store, &server, |r| {
                    r.signed_out = true;
                    r.signed_out_here = true;
                });
                registry.remove(&source_id(&server));
                servers.publish(&tx);
            });
        }
    }
}

async fn check(
    servers: &Servers,
    typed: &str,
) -> Result<(CheckedServer, Jellyfin), JellyfinError> {
    let address = Address::parse(typed)?;
    let client = servers.client(address.clone());
    let info = client.public_info().await?;
    let quick_connect = client.quick_connect_enabled().await.unwrap_or(false);
    Ok((
        CheckedServer {
            id: info.id,
            name: info.server_name,
            version: info.version,
            address: address.base().to_owned(),
            quick_connect,
        },
        client,
    ))
}

/// Poll an approval: every two seconds, for as long as the server keeps the code.
async fn quick_connect(
    servers: &Servers,
    address: &str,
    registry: &ProviderRegistry,
    order: &[String],
    tx: &Outbox,
) -> Result<(), JellyfinError> {
    let (checked, client) = check(servers, address).await?;
    let started = client.quick_connect_start().await?;
    let _ =
        tx.send(Event::Server(ServerEvent::QuickConnectCode { code: started.code.clone() }));
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if client.quick_connect_approved(&started.secret).await? {
            break;
        }
    }
    let session = client.sign_in_with_quick_connect(&started.secret).await?;
    finish_sign_in(servers, checked, session, registry, order, tx);
    Ok(())
}

/// Keep the server and its token, put it in the chain, and say so.
fn finish_sign_in(
    servers: &Servers,
    checked: CheckedServer,
    session: Session,
    registry: &ProviderRegistry,
    order: &[String],
    tx: &Outbox,
) {
    if servers.tokens.set(&Servers::token_key(&checked.id), &session.token).is_err() {
        failed(tx, ServerStep::SignIn, &JellyfinError::SignedOut);
        return;
    }
    let mut record = Record::new(&checked, &session);
    record.heard(true, crate::now_epoch());
    record::upsert(&servers.store, record.clone());
    servers.put_source(&record, registry, order);
    let _ = tx.send(Event::Server(ServerEvent::SignedIn { server: checked.id }));
    servers.publish(tx);
}

/// Match the ticked libraries' series to AniList, keep the matches, and give the source them.
fn sync(
    servers: Servers,
    server: String,
    registry: ProviderRegistry,
    meta: kaeri_meta::Meta,
    store: Store,
    order: Vec<String>,
    tx: Outbox,
) {
    tokio::spawn(async move {
        let Some(record) = servers.records().into_iter().find(|r| r.id == server) else {
            return;
        };
        let Some(client) = servers.signed_in(&record) else {
            failed(&tx, ServerStep::Libraries, &JellyfinError::SignedOut);
            return;
        };
        let progress = {
            let (servers, tx, server) = (servers.clone(), tx.clone(), server.clone());
            move |p: MatchProgress| {
                servers.set_progress(&server, Some(p));
                servers.publish(&tx);
            }
        };
        let result = matching::match_all(&client, &record, &meta, &store, &progress).await;
        servers.set_progress(&server, None);
        servers.heard(&server, &result);
        match result {
            Ok((matches, series)) => {
                record::update(&store, &server, |r| r.set_matches(matches, series));
                if let Some(updated) = servers.records().into_iter().find(|r| r.id == server) {
                    registry.remove(&source_id(&server));
                    servers.put_source(&updated, &registry, &order);
                }
            }
            Err(e) => {
                if matches!(e, JellyfinError::SignedOut) {
                    record::update(&store, &server, |r| r.signed_out = true);
                }
                failed(&tx, ServerStep::Libraries, &e);
            }
        }
        servers.publish(&tx);
    });
}

/// Whether a source in the chain is a media server's.
pub fn is_server(provider: &dyn kaeri_core::traits::Provider) -> bool {
    provider.manifest().kind == ProviderKind::Server
}
