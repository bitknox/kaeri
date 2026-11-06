//! A stand-in for MangaUpdates on loopback.
//!
//! It speaks the part of the v1 API the tracker uses: the session login, the person's lists, a
//! list search, reading one series' entry, adding and updating entries as arrays, and ratings.
//! It keeps what it was told, so a test can read back what the tracker wrote. The five-second
//! update delay is counted in calls rather than time: [`FakeMangaUpdates::delay_writes`] makes
//! the next writes answer 412. The account, series and ids are made up.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, MutexGuard},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

pub const USER: &str = "wayfarer";
pub const PASSWORD: &str = "paper-lantern";
pub const TOKEN: &str = "fixture-mangaupdates-token";

/// The list ids a fresh account has, and one custom list.
pub const READING: u64 = 0;
pub const WISH: u64 = 1;
pub const COMPLETE: u64 = 2;
pub const UNFINISHED: u64 = 3;
pub const HOLD: u64 = 4;
pub const CUSTOM: u64 = 100;

/// One series on a list.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub series: u64,
    pub title: String,
    pub novel: bool,
    pub list_id: u64,
    pub chapter: u32,
    pub volume: u32,
    pub rating: Option<f64>,
}

/// A request as the fake saw it.
#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub body: Value,
}

struct State {
    items: Vec<Item>,
    seen: Vec<Seen>,
    delayed: u32,
    /// Writes that were turned away with 412.
    turned_away: u32,
}

pub struct FakeMangaUpdates {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    task: JoinHandle<()>,
}

impl Drop for FakeMangaUpdates {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FakeMangaUpdates {
    pub async fn start() -> Self {
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let state = Arc::new(Mutex::new(State {
            items: Vec::new(),
            seen: Vec::new(),
            delayed: 0,
            turned_away: 0,
        }));
        let task = tokio::spawn({
            let state = state.clone();
            async move {
                while let Ok((socket, _)) = listener.accept().await {
                    tokio::spawn(serve(socket, state.clone()));
                }
            }
        });
        Self { addr, state, task }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Where the tracker is pointed, for `with_api`.
    pub fn base(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    pub fn add(&self, item: Item) {
        self.state().items.push(item);
    }

    /// Answer 412 to the next `n` list writes.
    pub fn delay_writes(&self, n: u32) {
        self.state().delayed = n;
    }

    pub fn turned_away(&self) -> u32 {
        self.state().turned_away
    }

    pub fn items(&self) -> Vec<Item> {
        self.state().items.clone()
    }

    pub fn item(&self, series: u64) -> Option<Item> {
        self.state().items.iter().find(|i| i.series == series).cloned()
    }

    pub fn requests(&self) -> Vec<Seen> {
        self.state().seen.clone()
    }

    /// List writes that got through (adds and updates), by path.
    pub fn writes(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .map(|s| s.path)
            .filter(|p| p.ends_with("/lists/series") || p.ends_with("/lists/series/update"))
            .collect()
    }
}

struct Request {
    method: String,
    path: String,
    authorization: String,
    body: Value,
}

async fn serve(socket: TcpStream, state: Arc<Mutex<State>>) {
    let (read, mut write) = socket.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();
    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let path = target.split('?').next().unwrap_or_default().to_owned();
    let (mut length, mut authorization) = (0usize, String::new());
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await.unwrap_or(0) == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().unwrap_or(0).min(1 << 20),
                "authorization" => authorization = value.trim().to_owned(),
                _ => {}
            }
        }
    }
    let mut raw = vec![0u8; length];
    let _ = reader.read_exact(&mut raw).await;
    let body = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    let request = Request { method, path, authorization, body };
    let (status, reply) = answer(&state, &request);
    let text = reply.to_string();
    let head = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        text.len()
    );
    let _ = write.write_all(head.as_bytes()).await;
    let _ = write.write_all(text.as_bytes()).await;
    let _ = write.shutdown().await;
}

fn record(item: &Item) -> Value {
    json!({
        "series": { "id": item.series, "title": item.title,
                    "type": if item.novel { "Novel" } else { "Manga" } },
        "list_id": item.list_id,
        "status": { "chapter": item.chapter, "volume": item.volume },
    })
}

fn answer(state: &Mutex<State>, request: &Request) -> (u16, Value) {
    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
    state.seen.push(Seen {
        method: request.method.clone(),
        path: request.path.clone(),
        body: request.body.clone(),
    });
    let path = request.path.strip_prefix("/v1").unwrap_or(&request.path).to_owned();
    if (request.method.as_str(), path.as_str()) == ("PUT", "/account/login") {
        return if request.body["username"] == USER && request.body["password"] == PASSWORD {
            (
                200,
                json!({ "status": "success", "reason": "Logged in",
                        "context": { "session_token": TOKEN, "uid": 1 } }),
            )
        } else {
            (401, json!({ "status": "exception", "reason": "Invalid username or password" }))
        };
    }
    if request.authorization != format!("Bearer {TOKEN}") {
        return (401, json!({ "status": "exception", "reason": "Unauthorized" }));
    }
    let write =
        request.method == "POST" && (path == "/lists/series" || path == "/lists/series/update");
    if write && state.delayed > 0 {
        state.delayed -= 1;
        state.turned_away += 1;
        return (412, json!({ "status": "exception", "reason": "Five second update delay" }));
    }
    match (request.method.as_str(), path.as_str()) {
        ("GET", "/lists") => {
            let lists = [
                (READING, "Reading List", "read", false),
                (WISH, "Wish List", "wish", false),
                (COMPLETE, "Complete List", "complete", false),
                (UNFINISHED, "Unfinished List", "unfinished", false),
                (HOLD, "On Hold List", "hold", false),
                (CUSTOM, "Reread", "", true),
            ];
            let lists: Vec<Value> = lists
                .iter()
                .map(|(id, title, kind, custom)| {
                    json!({ "list_id": id, "title": title, "type": kind, "custom": custom })
                })
                .collect();
            (200, Value::Array(lists))
        }
        ("POST", p) if p.starts_with("/lists/") && p.ends_with("/search") => {
            let id: u64 = p
                .trim_start_matches("/lists/")
                .trim_end_matches("/search")
                .parse()
                .unwrap_or(u64::MAX);
            let page = request.body["page"].as_u64().unwrap_or(1).max(1) as usize;
            let per = request.body["perpage"].as_u64().unwrap_or(25).max(1) as usize;
            let all: Vec<&Item> = state.items.iter().filter(|i| i.list_id == id).collect();
            let results: Vec<Value> = all
                .iter()
                .skip((page - 1) * per)
                .take(per)
                .map(|i| {
                    json!({ "record": record(i),
                            "metadata": { "user_list": { "rating": i.rating } } })
                })
                .collect();
            (
                200,
                json!({ "total_hits": all.len(), "page": page, "per_page": per,
                          "results": results }),
            )
        }
        ("GET", p) if p.starts_with("/lists/series/") => {
            let id: u64 = p.trim_start_matches("/lists/series/").parse().unwrap_or(0);
            match state.items.iter().find(|i| i.series == id) {
                Some(item) => (200, record(item)),
                None => (404, json!({ "status": "exception", "reason": "Not on a list" })),
            }
        }
        ("POST", "/lists/series") => {
            for add in request.body.as_array().into_iter().flatten() {
                let series = add["series"]["id"].as_u64().unwrap_or(0);
                let list_id = add["list_id"].as_u64().unwrap_or(READING);
                if !state.items.iter().any(|i| i.series == series) {
                    state.items.push(Item {
                        series,
                        title: format!("Series {series}"),
                        novel: false,
                        list_id,
                        chapter: 0,
                        volume: 0,
                        rating: None,
                    });
                }
            }
            (200, json!({ "status": "success" }))
        }
        ("POST", "/lists/series/update") => {
            for change in request.body.as_array().into_iter().flatten() {
                let series = change["series"]["id"].as_u64().unwrap_or(0);
                let Some(item) = state.items.iter_mut().find(|i| i.series == series) else {
                    return (404, json!({ "status": "exception", "reason": "Not on a list" }));
                };
                if let Some(list_id) = change["list_id"].as_u64() {
                    item.list_id = list_id;
                }
                if let Some(chapter) = change["status"]["chapter"].as_u64() {
                    item.chapter = chapter as u32;
                }
                if let Some(volume) = change["status"]["volume"].as_u64() {
                    item.volume = volume as u32;
                }
            }
            (200, json!({ "status": "success" }))
        }
        ("PUT", p) if p.starts_with("/series/") && p.ends_with("/rating") => {
            let id: u64 = p
                .trim_start_matches("/series/")
                .trim_end_matches("/rating")
                .parse()
                .unwrap_or(0);
            let rating = request.body["rating"].as_f64();
            match state.items.iter_mut().find(|i| i.series == id) {
                Some(item) => {
                    item.rating = rating;
                    (200, json!({ "status": "success" }))
                }
                None => (404, json!({ "status": "exception", "reason": "Unknown series" })),
            }
        }
        _ => (404, json!({ "status": "exception", "reason": "Unknown route" })),
    }
}
