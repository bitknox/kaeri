//! A Nostr relay on loopback, enough NIP-01 for the room: `EVENT`, `REQ` with kinds, `#z` and
//! `since`, `CLOSE`, `OK`, `EOSE`, `CLOSED`, ephemeral fan-out to every matching subscription
//! (the sender's own included), optional retention, NIP-42 AUTH challenges, scripted refusals and
//! a hang-up. It answers NIP-11 over plain HTTP with a `Date` header that can be skewed. Testing
//! builds only: tests never touch a public relay.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use futures::{SinkExt, StreamExt};
use nostr::event::{Event, Kind};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, watch},
};
use tokio_tungstenite::tungstenite::Message as Ws;

static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
struct Conn {
    tx: Option<mpsc::UnboundedSender<String>>,
    /// Subscription id → filter.
    subs: HashMap<String, Value>,
    authed: bool,
    challenge: String,
}

#[derive(Default)]
struct State {
    conns: HashMap<u64, Conn>,
    /// Every event taken, with when (epoch seconds), for retention and for observers.
    seen: Vec<(Event, i64)>,
    retention_s: i64,
    auth: bool,
    /// After a good AUTH, refuse everything with `restricted:`.
    restricted: bool,
    /// Refuse events with this reason (`rate-limited: …`, `blocked: …`).
    refuse: Option<String>,
    /// Refuse only this many more, then take them again.
    refuse_left: Option<u32>,
    /// Refuse connections: the relay is down.
    down: bool,
    /// Seconds added to the `Date` header.
    skew_s: i64,
    /// An `Age` header on NIP-11 answers.
    age_s: Option<i64>,
    /// Don't send a sender its own events back.
    no_echo: bool,
    /// Drop every event silently (no fan-out, but OK true).
    black_hole: bool,
    /// Take events this long after they arrive, ms.
    ingest_ms: u64,
    /// Subscriptions asked for, `REQ`s, all connections together.
    reqs: usize,
}

/// A running fake relay. Dropping it stops it.
pub struct FakeRelay {
    port: u16,
    state: Arc<Mutex<State>>,
    hang_up: watch::Sender<u64>,
    _stop: tokio::sync::oneshot::Sender<()>,
}

impl FakeRelay {
    /// A relay on a free loopback port. Call inside a tokio runtime.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
        let port = listener.local_addr().expect("bound").port();
        let state = Arc::new(Mutex::new(State::default()));
        let (hang_up, hang_rx) = watch::channel(0);
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let accept_state = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stopped => return,
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { continue };
                        let state = Arc::clone(&accept_state);
                        let hang = hang_rx.clone();
                        tokio::spawn(serve(stream, state, hang));
                    }
                }
            }
        });
        Self { port, state, hang_up, _stop: stop }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `ws://localhost:<port>`.
    pub fn url(&self) -> String {
        format!("ws://localhost:{}", self.port)
    }

    /// Keep events this long and send them to new subscriptions (strfry keeps 300 s).
    pub fn retain(&self, seconds: i64) {
        self.lock().retention_s = seconds;
    }

    /// Ask every connection for NIP-42 AUTH before taking events or subscriptions.
    pub fn require_auth(&self, auth: bool) {
        self.lock().auth = auth;
    }

    /// After a good AUTH, say `restricted:` to everything.
    pub fn restrict(&self, restricted: bool) {
        self.lock().restricted = restricted;
    }

    /// Refuse events with `reason` (`rate-limited: …`, `blocked: …`), `count` times or always.
    pub fn refuse(&self, reason: Option<&str>, count: Option<u32>) {
        let mut state = self.lock();
        state.refuse = reason.map(str::to_owned);
        state.refuse_left = count;
    }

    /// Close every connection now.
    pub fn hang_up(&self) {
        self.hang_up.send_modify(|n| *n += 1);
    }

    /// Down: refuse connections (and close the open ones). Up again with `false`.
    pub fn down(&self, down: bool) {
        self.lock().down = down;
        if down {
            self.hang_up();
        }
    }

    /// Skew the `Date` header of NIP-11 answers, and add an `Age` header.
    pub fn date(&self, skew_s: i64, age_s: Option<i64>) {
        let mut state = self.lock();
        state.skew_s = skew_s;
        state.age_s = age_s;
    }

    /// Don't echo events to their sender's own connection.
    pub fn no_echo(&self, no_echo: bool) {
        self.lock().no_echo = no_echo;
    }

    /// Say OK to events and pass none on.
    pub fn black_hole(&self, on: bool) {
        self.lock().black_hole = on;
    }

    /// Take each event `ms` after it arrives, as a public relay's ingester does: one from a
    /// connection that closed in the meantime is never taken.
    pub fn ingest_after(&self, ms: u64) {
        self.lock().ingest_ms = ms;
    }

    /// How many subscriptions were asked for (`REQ`s), on every connection.
    pub fn reqs(&self) -> usize {
        self.lock().reqs
    }

    /// Every event the relay took, in order: what a relay operator sees.
    pub fn events(&self) -> Vec<Event> {
        self.lock().seen.iter().map(|(e, _)| e.clone()).collect()
    }

    /// Open connections.
    pub fn connections(&self) -> usize {
        self.lock().conns.len()
    }
}

fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

async fn serve(stream: TcpStream, state: Arc<Mutex<State>>, mut hang: watch::Receiver<u64>) {
    if state.lock().unwrap_or_else(PoisonError::into_inner).down {
        return;
    }
    // Plain HTTP (NIP-11) or a WebSocket upgrade: look at the headers without taking them.
    let mut head = [0u8; 2048];
    let Ok(n) = stream.peek(&mut head).await else { return };
    let text = String::from_utf8_lossy(&head[..n]).to_ascii_lowercase();
    if !text.contains("upgrade: websocket") {
        nip11(stream, &state).await;
        return;
    }
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    let (mut write, mut read) = ws.split();
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    {
        let mut s = state.lock().unwrap_or_else(PoisonError::into_inner);
        let challenge = format!("challenge-{id}");
        if s.auth {
            let _ = tx.send(json!(["AUTH", challenge]).to_string());
        }
        s.conns.insert(id, Conn { tx: Some(tx), challenge, ..Conn::default() });
    }
    hang.borrow_and_update();
    loop {
        tokio::select! {
            _ = hang.changed() => break,
            // What a client sent before it went is still read, as a relay does, though a reply
            // to it fails.
            out = rx.recv() => match out {
                Some(text) => { let _ = write.send(Ws::text(text)).await; }
                None => break,
            },
            frame = read.next() => match frame {
                Some(Ok(Ws::Text(text))) => {
                    let later = state.lock().unwrap_or_else(PoisonError::into_inner).ingest_ms;
                    if later > 0 && text.as_str().starts_with("[\"EVENT\"") {
                        let state = Arc::clone(&state);
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(later)).await;
                            take(id, text.as_str(), &state);
                        });
                    } else {
                        take(id, text.as_str(), &state);
                    }
                }
                Some(Ok(Ws::Ping(p))) => { let _ = write.send(Ws::Pong(p)).await; }
                Some(Ok(Ws::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
    let _ = write.close().await;
    state.lock().unwrap_or_else(PoisonError::into_inner).conns.remove(&id);
}

async fn nip11(mut stream: TcpStream, state: &Arc<Mutex<State>>) {
    let mut buf = [0u8; 2048];
    let _ = stream.read(&mut buf).await;
    let (skew, age) = {
        let s = state.lock().unwrap_or_else(PoisonError::into_inner);
        (s.skew_s, s.age_s)
    };
    let date = http_date(now_s() + skew);
    let body = json!({"name": "fake relay", "supported_nips": [1, 11, 40, 42]}).to_string();
    let age = age.map(|a| format!("Age: {a}\r\n")).unwrap_or_default();
    let reply = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/nostr+json\r\nDate: {date}\r\n{age}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(reply.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// An RFC 7231 date.
pub(crate) fn http_date(secs: i64) -> String {
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    format!(
        "{}, {d:02} {} {y} {:02}:{:02}:{:02} GMT",
        DAYS[days.rem_euclid(7) as usize],
        MONTHS[(m - 1) as usize],
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since the epoch to (year, month, day), after Howard Hinnant's algorithm.
pub(crate) fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn matches(filter: &Value, event: &Event) -> bool {
    if let Some(kinds) = filter.get("kinds").and_then(Value::as_array)
        && !kinds.iter().any(|k| k.as_u64() == Some(u64::from(event.kind.as_u16())))
    {
        return false;
    }
    if let Some(tags) = filter.get("#z").and_then(Value::as_array) {
        let theirs: Vec<&str> = event
            .tags
            .iter()
            .filter(|t| t.as_slice().first().map(String::as_str) == Some("z"))
            .filter_map(|t| t.as_slice().get(1).map(String::as_str))
            .collect();
        if !tags.iter().any(|t| t.as_str().is_some_and(|t| theirs.contains(&t))) {
            return false;
        }
    }
    if let Some(since) = filter.get("since").and_then(Value::as_u64)
        && event.created_at.as_secs() < since
    {
        return false;
    }
    true
}

fn send(conn: &Conn, message: Value) {
    if let Some(tx) = &conn.tx {
        let _ = tx.send(message.to_string());
    }
}

fn take(id: u64, text: &str, state: &Arc<Mutex<State>>) {
    let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(text) else { return };
    let mut s = state.lock().unwrap_or_else(PoisonError::into_inner);
    let auth = s.auth;
    let restricted = s.restricted;
    let Some(conn) = s.conns.get(&id) else { return };
    let authed = conn.authed;
    match parts.first().and_then(Value::as_str) {
        Some("AUTH") => {
            let Some(event) =
                parts.get(1).and_then(|e| serde_json::from_value::<Event>(e.clone()).ok())
            else {
                return;
            };
            let good = event.verify().is_ok()
                && event.kind == Kind::Authentication
                && event.tags.iter().any(|t| {
                    t.as_slice().first().map(String::as_str) == Some("challenge")
                        && t.as_slice().get(1) == Some(&conn.challenge)
                });
            send(
                conn,
                json!([
                    "OK",
                    event.id.to_hex(),
                    good,
                    if good { "" } else { "invalid: bad auth" }
                ]),
            );
            if good && let Some(conn) = s.conns.get_mut(&id) {
                conn.authed = true;
            }
        }
        Some("REQ") => {
            s.reqs += 1;
            let Some(conn) = s.conns.get(&id) else { return };
            let Some(sub) = parts.get(1).and_then(Value::as_str).map(str::to_owned) else {
                return;
            };
            if auth && !authed {
                send(conn, json!(["CLOSED", sub, "auth-required: say who you are"]));
                return;
            }
            if restricted {
                send(conn, json!(["CLOSED", sub, "restricted: not for you"]));
                return;
            }
            let filter = parts.get(2).cloned().unwrap_or(Value::Null);
            let now = now_s();
            let keep = s.retention_s;
            let held: Vec<Event> = s
                .seen
                .iter()
                .filter(|(e, at)| keep > 0 && now - at <= keep && matches(&filter, e))
                .map(|(e, _)| e.clone())
                .collect();
            let conn = s.conns.get_mut(&id).expect("looked up");
            for event in held {
                send(conn, json!(["EVENT", sub, event]));
            }
            send(conn, json!(["EOSE", sub]));
            conn.subs.insert(sub, filter);
        }
        Some("CLOSE") => {
            if let Some(sub) = parts.get(1).and_then(Value::as_str)
                && let Some(conn) = s.conns.get_mut(&id)
            {
                conn.subs.remove(sub);
            }
        }
        Some("EVENT") => {
            let Some(event) =
                parts.get(1).and_then(|e| serde_json::from_value::<Event>(e.clone()).ok())
            else {
                return;
            };
            let eid = event.id.to_hex();
            if auth && !authed {
                send(conn, json!(["OK", eid, false, "auth-required: say who you are"]));
                return;
            }
            if restricted {
                send(conn, json!(["OK", eid, false, "restricted: not for you"]));
                return;
            }
            if event.verify().is_err() {
                send(conn, json!(["OK", eid, false, "invalid: bad signature"]));
                return;
            }
            if let Some(reason) = s.refuse.clone() {
                if let Some(left) = s.refuse_left.as_mut() {
                    *left = left.saturating_sub(1);
                    if *left == 0 {
                        s.refuse = None;
                        s.refuse_left = None;
                    }
                }
                let conn = s.conns.get(&id).expect("looked up");
                send(conn, json!(["OK", eid, false, reason]));
                return;
            }
            send(conn, json!(["OK", eid, true, ""]));
            s.seen.push((event.clone(), now_s()));
            if s.black_hole {
                return;
            }
            let no_echo = s.no_echo;
            for (cid, conn) in &s.conns {
                if no_echo && *cid == id {
                    continue;
                }
                for (sub, filter) in &conn.subs {
                    if matches(filter, &event) {
                        send(conn, json!(["EVENT", sub, event]));
                    }
                }
            }
        }
        _ => send(conn, json!(["NOTICE", "unknown message"])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_read_as_http_writes_them() {
        assert_eq!(http_date(0), "Thu, 01 Jan 1970 00:00:00 GMT");
        assert_eq!(http_date(1_790_000_000), "Mon, 21 Sep 2026 14:13:20 GMT");
    }
}
