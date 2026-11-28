//! One relay connection: it connects, subscribes, sends what the pool gives it and reports what
//! the relay says. It reconnects after 1, 2, 4, 8 and then every 15 s for as long as the room
//! lasts, pings every 20 s, and answers NIP-42 AUTH with the session key: after `auth-required:`
//! it sends the subscription again and the refused event once, if it's under 60 s old.
//! `restricted:` after a good AUTH makes the relay unusable for this room, with no reconnect loop.
//! It never sends a refused event again for any other reason.

use std::{collections::HashMap, sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt};
use nostr::{
    event::{Event, EventBuilder, Kind, Tag},
    prelude::FinalizeEvent,
};
use serde_json::{Value, json};
use tokio::{sync::mpsc, time::Instant};
use tokio_tungstenite::{Connector, tungstenite::Message as Ws};

use crate::keys::Session;

/// The waits between attempts, then every 15 s.
pub const BACKOFF_S: [u64; 5] = [1, 2, 4, 8, 15];
/// A ping this often in the foreground.
pub const PING: Duration = Duration::from_secs(20);
/// No frame at all for this long: the socket is half open.
pub const SILENT_SOCKET: Duration = Duration::from_secs(45);
/// How long a connect may take.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
/// A refused event older than this isn't sent again after AUTH.
pub const AUTH_RETRY_LIMIT: Duration = Duration::from_secs(60);
/// The one subscription's id.
pub const SUB: &str = "room";
/// The debug log's target for what goes over the socket: every event sent and heard, and every
/// answer, with the relay and this session's `pk8`. The live probe reads it.
pub const WIRE: &str = "kaeri_room::wire";

/// What the pool tells a relay.
#[derive(Debug)]
pub enum ToRelay {
    Send(Box<Event>),
    /// Subscribe with this filter (and again after every reconnect).
    Subscribe(Value),
    /// Close the socket and open it again now.
    Refresh,
    Stop,
}

/// What a relay tells the pool.
#[derive(Debug, Clone, PartialEq)]
pub enum FromRelay {
    Up,
    /// The socket closed or couldn't open; `failures` in a row.
    Down {
        failures: u32,
    },
    Event(Box<Event>),
    Ok {
        id: String,
        accepted: bool,
        message: String,
    },
    /// `restricted:` after a good AUTH, or for the subscription: no use to this room.
    Unusable(String),
}

/// A running relay connection.
pub struct RelayHandle {
    pub url: String,
    tx: mpsc::UnboundedSender<ToRelay>,
}

impl RelayHandle {
    pub fn send(&self, message: ToRelay) {
        let _ = self.tx.send(message);
    }
}

impl Drop for RelayHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(ToRelay::Stop);
    }
}

/// Rustls on ring with the web's public roots, as the rest of the app.
fn tls() -> Connector {
    static CONFIG: std::sync::LazyLock<Arc<rustls::ClientConfig>> =
        std::sync::LazyLock::new(|| {
            let roots =
                rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            let config = rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .expect("ring supports the default versions")
            .with_root_certificates(roots)
            .with_no_client_auth();
            Arc::new(config)
        });
    Connector::Rustls(Arc::clone(&CONFIG))
}

/// Start relay `i` at `url`; it reports to `reports` as `(i, …)`.
pub fn spawn(
    i: usize,
    url: String,
    session: Session,
    reports: mpsc::UnboundedSender<(usize, FromRelay)>,
) -> RelayHandle {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(run(i, url.clone(), session, rx, reports));
    RelayHandle { url, tx }
}

struct Conn {
    i: usize,
    url: String,
    session: Session,
    reports: mpsc::UnboundedSender<(usize, FromRelay)>,
    filter: Option<Value>,
    /// The latest AUTH challenge, and whether AUTH went through.
    challenge: Option<String>,
    authed: bool,
    auth_id: Option<String>,
    /// Events sent on this socket and not yet answered, for the AUTH retry.
    sent: HashMap<String, (Event, Instant)>,
    /// Refused with auth-required, to send again once AUTH is through.
    after_auth: Vec<Event>,
    resubscribe_after_auth: bool,
    /// Up was reported for this socket.
    announced: bool,
}

enum Ended {
    Stop,
    Refresh,
    Lost,
    Unusable,
}

async fn run(
    i: usize,
    url: String,
    session: Session,
    mut rx: mpsc::UnboundedReceiver<ToRelay>,
    reports: mpsc::UnboundedSender<(usize, FromRelay)>,
) {
    let mut conn = Conn {
        i,
        url,
        session,
        reports,
        filter: None,
        challenge: None,
        authed: false,
        auth_id: None,
        sent: HashMap::new(),
        after_auth: Vec::new(),
        resubscribe_after_auth: false,
        announced: false,
    };
    let mut failures: u32 = 0;
    loop {
        let connected = tokio::time::timeout(
            CONNECT_TIMEOUT,
            tokio_tungstenite::connect_async_tls_with_config(
                conn.url.as_str(),
                None,
                false,
                Some(tls()),
            ),
        )
        .await;
        let ended = match connected {
            Ok(Ok((ws, _))) => {
                failures = 0;
                tracing::debug!(target: WIRE, relay = %conn.url, me = %conn.session.pk8(), "open");
                conn.session_on(ws, &mut rx).await
            }
            Ok(Err(why)) => {
                tracing::debug!(target: WIRE, relay = %conn.url, me = %conn.session.pk8(), why = %why, "unreachable");
                Ended::Lost
            }
            Err(_) => {
                tracing::debug!(target: WIRE, relay = %conn.url, me = %conn.session.pk8(), why = "connect timed out", "unreachable");
                Ended::Lost
            }
        };
        match ended {
            Ended::Stop | Ended::Unusable => return,
            Ended::Refresh => {
                conn.report(FromRelay::Down { failures: 0 });
                continue;
            }
            Ended::Lost => {
                failures += 1;
                conn.report(FromRelay::Down { failures });
            }
        }
        let wait = BACKOFF_S[(failures as usize).saturating_sub(1).min(BACKOFF_S.len() - 1)];
        let until = tokio::time::sleep(Duration::from_secs(wait));
        tokio::pin!(until);
        loop {
            tokio::select! {
                () = &mut until => break,
                command = rx.recv() => match command {
                    None | Some(ToRelay::Stop) => return,
                    Some(ToRelay::Refresh) => break,
                    Some(ToRelay::Subscribe(filter)) => conn.filter = Some(filter),
                    // Down: the pool counts it as not sent here.
                    Some(ToRelay::Send(_)) => {}
                },
            }
        }
    }
}

type Socket = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

impl Conn {
    fn report(&self, report: FromRelay) {
        let _ = self.reports.send((self.i, report));
    }

    fn log_send(&self, event: &Event) {
        tracing::debug!(target: WIRE, relay = %self.url, me = %self.session.pk8(), id = %event.id.to_hex(), "send");
    }

    fn announce(&mut self) {
        if !std::mem::replace(&mut self.announced, true) {
            self.report(FromRelay::Up);
        }
    }

    async fn session_on(
        &mut self,
        ws: Socket,
        rx: &mut mpsc::UnboundedReceiver<ToRelay>,
    ) -> Ended {
        let (mut write, mut read) = ws.split();
        self.challenge = None;
        self.authed = false;
        self.auth_id = None;
        self.sent.clear();
        self.after_auth.clear();
        self.resubscribe_after_auth = false;
        self.announced = false;
        // A subscription waiting in the queue goes in the first REQ; events sent while this
        // socket was being opened (right after a refresh) go out after it.
        let mut waiting = Vec::new();
        while let Ok(command) = rx.try_recv() {
            match command {
                ToRelay::Subscribe(filter) => self.filter = Some(filter),
                ToRelay::Stop => return Ended::Stop,
                ToRelay::Send(event) => waiting.push(event),
                ToRelay::Refresh => {}
            }
        }
        // Up once the subscription is live (its EOSE), so nothing sent after Up is missed.
        if self.filter.is_none() {
            self.announce();
        }
        if let Some(filter) = &self.filter {
            let req = json!(["REQ", SUB, filter]).to_string();
            if write.send(Ws::text(req)).await.is_err() {
                return Ended::Lost;
            }
        }
        for event in waiting {
            self.log_send(&event);
            let text = json!(["EVENT", *event]).to_string();
            self.sent.insert(event.id.to_hex(), (*event, Instant::now()));
            if write.send(Ws::text(text)).await.is_err() {
                return Ended::Lost;
            }
        }
        let mut ping = tokio::time::interval(PING);
        ping.tick().await;
        let mut heard_at = Instant::now();
        loop {
            let mut out: Vec<String> = Vec::new();
            tokio::select! {
                _ = ping.tick() => {
                    if heard_at.elapsed() > SILENT_SOCKET {
                        return Ended::Lost;
                    }
                    if write.send(Ws::Ping(Vec::new().into())).await.is_err() {
                        return Ended::Lost;
                    }
                }
                command = rx.recv() => match command {
                    None | Some(ToRelay::Stop) => {
                        let _ = write.close().await;
                        return Ended::Stop;
                    }
                    Some(ToRelay::Refresh) => {
                        let _ = write.close().await;
                        return Ended::Refresh;
                    }
                    Some(ToRelay::Subscribe(filter)) => {
                        out.push(json!(["REQ", SUB, filter]).to_string());
                        self.filter = Some(filter);
                    }
                    Some(ToRelay::Send(event)) => {
                        self.log_send(&event);
                        out.push(json!(["EVENT", *event]).to_string());
                        self.sent.retain(|_, (_, at)| at.elapsed() < AUTH_RETRY_LIMIT);
                        self.sent.insert(event.id.to_hex(), (*event, Instant::now()));
                    }
                },
                frame = read.next() => {
                    heard_at = Instant::now();
                    match frame {
                        Some(Ok(Ws::Text(text))) => {
                            if let Some(ended) = self.take(text.as_str(), &mut out) {
                                let _ = write.close().await;
                                return ended;
                            }
                        }
                        Some(Ok(Ws::Ping(p))) => {
                            if write.send(Ws::Pong(p)).await.is_err() {
                                return Ended::Lost;
                            }
                        }
                        Some(Ok(Ws::Close(why))) => {
                            let why = why.map(|f| format!("{} {}", f.code, f.reason)).unwrap_or_default();
                            tracing::debug!(target: WIRE, relay = %self.url, me = %self.session.pk8(), %why, "lost");
                            return Ended::Lost;
                        }
                        Some(Err(why)) => {
                            tracing::debug!(target: WIRE, relay = %self.url, me = %self.session.pk8(), why = %why, "lost");
                            return Ended::Lost;
                        }
                        None => return Ended::Lost,
                        Some(Ok(_)) => {}
                    }
                }
            }
            for text in out {
                if write.send(Ws::text(text)).await.is_err() {
                    return Ended::Lost;
                }
            }
        }
    }

    fn auth(&mut self, out: &mut Vec<String>) {
        let Some(challenge) = self.challenge.clone() else { return };
        let Ok(event) = EventBuilder::new(Kind::Authentication, "")
            .tag(Tag::custom("relay", [self.url.clone()]))
            .tag(Tag::custom("challenge", [challenge]))
            .finalize(self.session.keys())
        else {
            return;
        };
        self.auth_id = Some(event.id.to_hex());
        out.push(json!(["AUTH", event]).to_string());
    }

    /// A frame from the relay. `Some` ends the socket.
    fn take(&mut self, text: &str, out: &mut Vec<String>) -> Option<Ended> {
        let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(text) else { return None };
        let word =
            |n: usize| parts.get(n).and_then(Value::as_str).unwrap_or_default().to_owned();
        let me = self.session.pk8();
        match parts.first().and_then(Value::as_str) {
            Some("EVENT") => {
                if let Some(event) =
                    parts.get(2).and_then(|e| serde_json::from_value::<Event>(e.clone()).ok())
                {
                    tracing::debug!(target: WIRE, relay = %self.url, %me, id = %event.id.to_hex(), "heard");
                    self.report(FromRelay::Event(Box::new(event)));
                }
            }
            Some("EOSE") => self.announce(),
            Some("NOTICE") => {
                tracing::debug!(target: WIRE, relay = %self.url, %me, answer = %word(1), "notice");
            }
            Some("AUTH") => {
                tracing::debug!(target: WIRE, relay = %self.url, %me, "auth asked");
                self.challenge = Some(word(1));
                self.authed = false;
            }
            Some("OK") => {
                let id = word(1);
                let accepted = parts.get(2).and_then(Value::as_bool).unwrap_or(false);
                let message = word(3);
                tracing::debug!(target: WIRE, relay = %self.url, %me, %id, accepted, answer = %message, "ok");
                if self.auth_id.as_deref() == Some(id.as_str()) {
                    self.authed = accepted;
                    if accepted {
                        if std::mem::take(&mut self.resubscribe_after_auth)
                            && let Some(filter) = &self.filter
                        {
                            out.push(json!(["REQ", SUB, filter]).to_string());
                        }
                        for event in std::mem::take(&mut self.after_auth) {
                            out.push(json!(["EVENT", event]).to_string());
                        }
                    }
                    return None;
                }
                if !accepted && message.starts_with("auth-required:") && !self.authed {
                    if let Some((event, at)) = self.sent.remove(&id)
                        && at.elapsed() < AUTH_RETRY_LIMIT
                    {
                        self.after_auth.push(event);
                    }
                    if self.auth_id.is_none() {
                        self.auth(out);
                    }
                    return None;
                }
                self.sent.remove(&id);
                if !accepted && message.starts_with("restricted:") && self.authed {
                    self.report(FromRelay::Unusable(message));
                    return Some(Ended::Unusable);
                }
                self.report(FromRelay::Ok { id, accepted, message });
            }
            Some("CLOSED") => {
                let message = word(2);
                tracing::debug!(target: WIRE, relay = %self.url, %me, answer = %message, "closed");
                if message.starts_with("auth-required:") && !self.authed {
                    self.resubscribe_after_auth = true;
                    if self.auth_id.is_none() {
                        self.auth(out);
                    }
                } else if message.starts_with("restricted:") || message.starts_with("blocked:")
                {
                    self.report(FromRelay::Unusable(message));
                    return Some(Ended::Unusable);
                }
            }
            _ => {}
        }
        None
    }
}
