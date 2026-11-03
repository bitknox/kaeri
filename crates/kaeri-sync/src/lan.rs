//! The peer road between devices on one network: mutual TLS 1.3 on the remote's raw-key stack
//! under the `kaeri-sync/1` protocol name, pairing with a short code (SPAKE2, bound to the
//! connection and to both keys), and then [`crate::peer::exchange`] over the session.
//!
//! A device that has paired is known by its key and may dial again without a code. A device that
//! has not may only pair, with the code that is showing: three tries, two minutes, then the code
//! is gone.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use kaeri_remote::{
    identity::Fingerprint,
    pairing::{Binding, MAX_TRIES, PairingCode, PhonePairing, TvPairing},
    tls::{self, EXPORTER_LABEL, SYNC_ALPN},
};
pub use kaeri_remote::{identity::Identity, pairing::CODE_LIFETIME};
use kaeri_store::Store;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::{
    error::{Result, SyncError},
    peer::{self, PeerReport},
};

const TRUST: &str = "lan.trust";
const ANSWER: Duration = Duration::from_secs(10);
const MAX_FRAME: usize = 64 * 1024;

/// What this device is, to the devices it pairs with.
#[derive(Clone)]
pub struct Me {
    pub identity: Identity,
    pub device: String,
    pub name: String,
    /// The public half of the device's group key, hex, which the others wrap the group key for.
    pub pubkey: String,
}

/// A paired device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paired {
    pub device: String,
    pub name: String,
    pub pubkey: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum Msg {
    /// The dialer's first word: a pairing start when it has a code, else nothing.
    Hello { pair: Option<Vec<u8>> },
    /// The listener knows the dialer's key: straight to the exchange.
    Ready,
    /// The listener's side of the pairing: its message and its proof.
    PairReply { message: Vec<u8>, mac: Vec<u8> },
    /// The dialer's proof, and who it is.
    Proof { mac: Vec<u8>, who: Paired },
    /// The listener accepted the proof, and says who it is.
    Paired { who: Paired },
    /// No: the code is not showing, ran out, or was wrong too often.
    Refused,
}

async fn write<W: AsyncWrite + Unpin>(w: &mut W, msg: &Msg) -> Result<()> {
    let body = serde_json::to_vec(msg).map_err(|_| SyncError::Damaged("encoding"))?;
    w.write_all(&(body.len() as u32).to_be_bytes()).await.map_err(io)?;
    w.write_all(&body).await.map_err(io)?;
    w.flush().await.map_err(io)
}

async fn read<R: AsyncRead + Unpin, T: DeserializeOwned>(r: &mut R) -> Result<T> {
    let mut len = [0u8; 4];
    tokio::time::timeout(ANSWER, r.read_exact(&mut len))
        .await
        .map_err(|_| SyncError::Network("no answer".into()))?
        .map_err(io)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(SyncError::Damaged("oversized frame"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await.map_err(io)?;
    serde_json::from_slice(&body).map_err(|_| SyncError::Damaged("frame"))
}

fn io(e: std::io::Error) -> SyncError {
    SyncError::Network(e.to_string())
}

fn pairing(e: impl std::fmt::Display) -> SyncError {
    SyncError::Pairing(e.to_string())
}

/// The devices paired, by key fingerprint.
pub fn trusted(store: &Store) -> Result<Vec<(String, Paired)>> {
    let json = store.sync_meta_get(TRUST)?.unwrap_or_default();
    Ok(serde_json::from_str(&json).unwrap_or_default())
}

fn trust(store: &Store, fingerprint: Fingerprint, who: Paired) -> Result<()> {
    let mut all = trusted(store)?;
    let key = fingerprint.to_hex();
    all.retain(|(k, _)| *k != key);
    all.push((key, who.clone()));
    store.sync_meta_set(
        TRUST,
        &serde_json::to_string(&all).map_err(|_| SyncError::Damaged("trust"))?,
    )?;
    // In the group's log too, so the folder road wraps the group key for it and the Devices list
    // names it.
    store.sync_device_add(&who.device, &who.name, &who.pubkey)?;
    Ok(())
}

/// Forget a device's key: it can no longer open a session without pairing again.
pub fn forget(store: &Store, device: &str) -> Result<()> {
    let mut all = trusted(store)?;
    all.retain(|(_, p)| p.device != device);
    store.sync_meta_set(
        TRUST,
        &serde_json::to_string(&all).map_err(|_| SyncError::Damaged("trust"))?,
    )?;
    Ok(())
}

fn find(store: &Store, fingerprint: &Fingerprint) -> Option<Paired> {
    let key = fingerprint.to_hex();
    trusted(store).ok()?.into_iter().find(|(k, _)| *k == key).map(|(_, p)| p)
}

/// The code on show, and how it is used up.
struct Showing {
    code: PairingCode,
    until: Instant,
    tries: u32,
}

/// A device listening for the others on its network.
pub struct Listener {
    port: u16,
    showing: Arc<Mutex<Option<Showing>>>,
    task: JoinHandle<()>,
    done: tokio::sync::mpsc::UnboundedReceiver<Result<PeerReport>>,
}

impl Listener {
    /// Listen on `addr` (port 0 for any). Paired devices may sync with it at any time; others may
    /// pair while a code is showing ([`Self::show_code`]).
    pub async fn bind(me: Me, store: Store, addr: SocketAddr) -> Result<Self> {
        let listener = TcpListener::bind(addr).await.map_err(io)?;
        let port = listener.local_addr().map_err(io)?.port();
        let acceptor = TlsAcceptor::from(
            tls::server_config_for(&me.identity, SYNC_ALPN).map_err(pairing)?,
        );
        let showing = Arc::new(Mutex::new(None::<Showing>));
        let (sink, done) = tokio::sync::mpsc::unbounded_channel();
        let task = {
            let showing = showing.clone();
            tokio::spawn(async move {
                loop {
                    let Ok((socket, _)) = listener.accept().await else { return };
                    let (acceptor, me, store, showing, sink) = (
                        acceptor.clone(),
                        me.clone(),
                        store.clone(),
                        showing.clone(),
                        sink.clone(),
                    );
                    tokio::spawn(async move {
                        let outcome = serve(acceptor, socket, &me, &store, &showing).await;
                        let _ = sink.send(outcome);
                    });
                }
            })
        };
        Ok(Self { port, showing, task, done })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Show a new code, replacing any: six digits, good for [`CODE_LIFETIME`] seconds and
    /// [`MAX_TRIES`] tries.
    pub fn show_code(&self) -> String {
        let code = PairingCode::generate();
        let text = code.as_str().to_owned();
        *self.showing.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(Showing {
                code,
                until: Instant::now() + Duration::from_secs(u64::from(CODE_LIFETIME)),
                tries: 0,
            });
        text
    }

    /// Whether a code is showing and still good.
    pub fn code_showing(&self) -> bool {
        self.showing
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|s| s.until > Instant::now() && s.tries < MAX_TRIES)
    }

    /// The next session that ended: what it exchanged, or why it did not.
    pub async fn next_session(&mut self) -> Option<Result<PeerReport>> {
        self.done.recv().await
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(
    acceptor: TlsAcceptor,
    socket: TcpStream,
    me: &Me,
    store: &Store,
    showing: &Mutex<Option<Showing>>,
) -> Result<PeerReport> {
    let mut stream = tokio::time::timeout(ANSWER, acceptor.accept(socket))
        .await
        .map_err(|_| SyncError::Network("no handshake".into()))?
        .map_err(io)?;
    let (peer_fp, exporter) = {
        let (_, conn) = stream.get_ref();
        let fp = tls::peer_fingerprint(conn.peer_certificates())
            .ok_or_else(|| pairing("the other device sent no key"))?;
        let exporter =
            conn.export_keying_material([0u8; 32], EXPORTER_LABEL, None).map_err(pairing)?;
        (fp, exporter)
    };
    let Msg::Hello { pair } = read(&mut stream).await? else {
        return Err(SyncError::Damaged("expected hello"));
    };
    if find(store, &peer_fp).is_some() {
        write(&mut stream, &Msg::Ready).await?;
    } else {
        let Some(start) = pair else {
            write(&mut stream, &Msg::Refused).await?;
            return Err(pairing("an unpaired device came without a code"));
        };
        // The code is used up by trying, whatever the outcome.
        let code = {
            let mut guard = showing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            match guard.as_mut() {
                Some(s) if s.until > Instant::now() && s.tries < MAX_TRIES => {
                    s.tries += 1;
                    Some(PairingCode::from_text(s.code.as_str()))
                }
                _ => None,
            }
        };
        let Some(code) = code else {
            write(&mut stream, &Msg::Refused).await?;
            return Err(pairing("no code is showing"));
        };
        let binding = Binding { phone: peer_fp, tv: me.identity.fingerprint(), exporter };
        let (side, message, mac) =
            TvPairing::respond(&code, binding, &start).map_err(pairing)?;
        write(&mut stream, &Msg::PairReply { message, mac }).await?;
        let Msg::Proof { mac, who } = read(&mut stream).await? else {
            return Err(SyncError::Damaged("expected proof"));
        };
        if side.confirm(&mac).is_err() {
            write(&mut stream, &Msg::Refused).await?;
            return Err(SyncError::Pairing("the code was wrong".into()));
        }
        // Right: the code is spent.
        *showing.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        trust(store, peer_fp, who)?;
        write(
            &mut stream,
            &Msg::Paired {
                who: Paired {
                    device: me.device.clone(),
                    name: me.name.clone(),
                    pubkey: me.pubkey.clone(),
                },
            },
        )
        .await?;
    }
    peer::exchange(store, &me.device, stream).await
}

/// Dial the device at `addr` and sync with it. With `code` it pairs first (the device must be
/// showing that code); without, it must already be paired, and is held to the key it paired with.
pub async fn dial(
    me: &Me,
    store: &Store,
    addr: SocketAddr,
    code: Option<&str>,
    expect: Option<Fingerprint>,
) -> Result<PeerReport> {
    let (config, server_key) =
        tls::client_config_for(&me.identity, expect, SYNC_ALPN).map_err(pairing)?;
    let socket = tokio::time::timeout(ANSWER, TcpStream::connect(addr))
        .await
        .map_err(|_| SyncError::Network("no answer".into()))?
        .map_err(io)?;
    let mut stream = tokio::time::timeout(
        ANSWER,
        TlsConnector::from(config).connect(tls::server_name(), socket),
    )
    .await
    .map_err(|_| SyncError::Network("no handshake".into()))?
    .map_err(|e| {
        if server_key.mismatched() {
            SyncError::Pairing("that device's key is not the one it paired with".into())
        } else {
            io(e)
        }
    })?;
    let their_fp = server_key.seen().ok_or_else(|| pairing("the other device sent no key"))?;
    let exporter = {
        let (_, conn) = stream.get_ref();
        conn.export_keying_material([0u8; 32], EXPORTER_LABEL, None).map_err(pairing)?
    };
    let binding = Binding { phone: me.identity.fingerprint(), tv: their_fp, exporter };
    let started = code.map(|c| PhonePairing::start(c, binding));
    write(&mut stream, &Msg::Hello { pair: started.as_ref().map(|(_, m)| m.clone()) }).await?;
    match read(&mut stream).await? {
        Msg::Ready => {}
        Msg::PairReply { message, mac } => {
            let (side, _) = started.ok_or_else(|| pairing("a code was not asked for"))?;
            let proof = side.finish(&message, &mac).map_err(pairing)?;
            let who = Paired {
                device: me.device.clone(),
                name: me.name.clone(),
                pubkey: me.pubkey.clone(),
            };
            write(&mut stream, &Msg::Proof { mac: proof, who }).await?;
            let Msg::Paired { who } = read(&mut stream).await? else {
                return Err(SyncError::Pairing("the code was wrong".into()));
            };
            trust(store, their_fp, who)?;
        }
        Msg::Refused => {
            return Err(SyncError::Pairing("that device is not showing a code".into()));
        }
        _ => return Err(SyncError::Damaged("unexpected reply")),
    }
    peer::exchange(store, &me.device, stream).await
}

/// A paired device's key fingerprint, for dialing it again.
pub fn fingerprint_of(store: &Store, device: &str) -> Option<Fingerprint> {
    trusted(store)
        .ok()?
        .into_iter()
        .find(|(_, p)| p.device == device)
        .and_then(|(fp, _)| Fingerprint::from_hex(&fp))
}

#[cfg(test)]
mod tests {
    use kaeri_core::ids::AnilistId;
    use kaeri_store::WatchEvent;

    use super::*;
    use crate::keys::DeviceKey;

    fn device(id: &str, name: &str) -> (Me, Store) {
        let store = Store::open_in_memory().unwrap();
        store.sync_enable(id).unwrap();
        let identity = Identity::generate().unwrap();
        let key = DeviceKey::from_identity(&identity.to_pkcs8());
        store.sync_device_add(id, name, &key.public_hex()).unwrap();
        (Me { identity, device: id.into(), name: name.into(), pubkey: key.public_hex() }, store)
    }

    fn watch(store: &Store, episode: &str) {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut e = WatchEvent::new(AnilistId::new(9_001), episode, 100.0);
        e.duration_secs = Some(1440.0);
        e.at = at - 30;
        store.record_event(&e).unwrap();
    }

    fn loopback(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    #[tokio::test]
    async fn a_device_pairs_with_the_code_and_then_syncs_without_one() {
        let (host, host_store) = device("00000000000000a1", "Living room");
        let (guest, guest_store) = device("00000000000000b2", "Phone");
        watch(&host_store, "1");
        watch(&guest_store, "2");
        let mut listener =
            Listener::bind(host.clone(), host_store.clone(), loopback(0)).await.unwrap();
        let code = listener.show_code();
        assert!(listener.code_showing());

        let report = dial(&guest, &guest_store, loopback(listener.port()), Some(&code), None)
            .await
            .unwrap();
        assert!(report.received > 0);
        listener.next_session().await.unwrap().unwrap();
        // The code is spent by a pairing that worked.
        assert!(!listener.code_showing());
        assert_eq!(
            host_store.resume_position(AnilistId::new(9_001), "2").unwrap(),
            Some(100.0)
        );
        assert_eq!(
            guest_store.resume_position(AnilistId::new(9_001), "1").unwrap(),
            Some(100.0)
        );
        // Each is in the other's device list, with the key the folder road wraps for.
        for (store, other) in [(&host_store, "Phone"), (&guest_store, "Living room")] {
            let peers = store.sync_peers().unwrap();
            assert!(
                peers.iter().any(|(_, name, key, _)| {
                    name.as_deref() == Some(other) && key.is_some()
                })
            );
        }

        // Paired: a new watch crosses with no code, to the key it paired with.
        watch(&guest_store, "3");
        let pinned = fingerprint_of(&guest_store, "00000000000000a1");
        assert!(pinned.is_some());
        dial(&guest, &guest_store, loopback(listener.port()), None, pinned).await.unwrap();
        listener.next_session().await.unwrap().unwrap();
        assert_eq!(
            host_store.resume_position(AnilistId::new(9_001), "3").unwrap(),
            Some(100.0)
        );
    }

    #[tokio::test]
    async fn a_wrong_code_does_not_pair_and_three_use_the_code_up() {
        let (host, host_store) = device("00000000000000a1", "Living room");
        let (guest, guest_store) = device("00000000000000b2", "Phone");
        let mut listener = Listener::bind(host, host_store.clone(), loopback(0)).await.unwrap();
        let code = listener.show_code();
        let wrong = if code == "000000" { "111111" } else { "000000" };
        for _ in 0..MAX_TRIES {
            let tried =
                dial(&guest, &guest_store, loopback(listener.port()), Some(wrong), None).await;
            assert!(tried.is_err());
            assert!(listener.next_session().await.unwrap().is_err());
        }
        assert!(!listener.code_showing());
        // Even the right code is refused now.
        let late =
            dial(&guest, &guest_store, loopback(listener.port()), Some(&code), None).await;
        assert!(late.is_err());
        assert!(trusted(&host_store).unwrap().is_empty());
        assert!(trusted(&guest_store).unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_unpaired_device_without_a_code_is_refused() {
        let (host, host_store) = device("00000000000000a1", "Living room");
        let (guest, guest_store) = device("00000000000000b2", "Phone");
        let mut listener = Listener::bind(host, host_store, loopback(0)).await.unwrap();
        listener.show_code();
        let refused = dial(&guest, &guest_store, loopback(listener.port()), None, None).await;
        assert!(refused.is_err());
        assert!(listener.next_session().await.unwrap().is_err());
    }

    #[tokio::test]
    async fn a_paired_device_that_was_forgotten_must_pair_again() {
        let (host, host_store) = device("00000000000000a1", "Living room");
        let (guest, guest_store) = device("00000000000000b2", "Phone");
        let mut listener = Listener::bind(host, host_store.clone(), loopback(0)).await.unwrap();
        let code = listener.show_code();
        dial(&guest, &guest_store, loopback(listener.port()), Some(&code), None).await.unwrap();
        listener.next_session().await.unwrap().unwrap();
        forget(&host_store, "00000000000000b2").unwrap();
        let refused = dial(&guest, &guest_store, loopback(listener.port()), None, None).await;
        assert!(refused.is_err());
    }

    #[tokio::test]
    async fn a_device_holds_the_listener_to_the_key_it_paired_with() {
        let (host, host_store) = device("00000000000000a1", "Living room");
        let (guest, guest_store) = device("00000000000000b2", "Phone");
        let (other, other_store) = device("00000000000000c3", "Stranger");
        let mut listener = Listener::bind(host, host_store, loopback(0)).await.unwrap();
        let code = listener.show_code();
        dial(&guest, &guest_store, loopback(listener.port()), Some(&code), None).await.unwrap();
        listener.next_session().await.unwrap().unwrap();
        // Another device answering at that address is not the one paired with.
        let impostor = Listener::bind(other, other_store, loopback(0)).await.unwrap();
        impostor.show_code();
        let pinned = fingerprint_of(&guest_store, "00000000000000a1");
        let refused = dial(&guest, &guest_store, loopback(impostor.port()), None, pinned).await;
        assert!(matches!(refused, Err(SyncError::Pairing(_))), "{refused:?}");
    }
}
