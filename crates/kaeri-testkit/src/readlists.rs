//! Stand-ins for the manga lists of MyAnimeList (`/v2`) and Kitsu (`/api/edge`), on one loopback
//! address. Each holds its list and keeps what the tracker wrote, so a test can read it back.
//! The accounts and titles are made up. Lists page two entries at a time, so a pull has to
//! follow the next link.

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

pub const MAL_TOKEN: &str = "fixture-mal-token";
pub const KITSU_TOKEN: &str = "fixture-kitsu-manga-token";
pub const KITSU_USER: &str = "5150";
const PAGE: usize = 2;

/// One entry on the fake MyAnimeList manga list.
#[derive(Debug, Clone, PartialEq)]
pub struct MalEntry {
    pub id: u32,
    pub media_type: String,
    pub status: String,
    pub chapters: u32,
    pub volumes: u32,
    pub rereading: bool,
    pub score: u32,
}

impl MalEntry {
    pub fn new(id: u32, status: &str, chapters: u32, volumes: u32) -> Self {
        Self {
            id,
            media_type: "manga".into(),
            status: status.into(),
            chapters,
            volumes,
            rereading: false,
            score: 0,
        }
    }
}

/// One entry on the fake Kitsu manga list.
#[derive(Debug, Clone, PartialEq)]
pub struct KitsuEntry {
    pub entry_id: String,
    pub manga: u32,
    pub subtype: String,
    pub status: String,
    pub progress: u32,
    pub reconsuming: bool,
    pub rating_twenty: Option<u32>,
}

impl KitsuEntry {
    pub fn new(manga: u32, status: &str, progress: u32) -> Self {
        Self {
            entry_id: format!("e{manga}"),
            manga,
            subtype: "manga".into(),
            status: status.into(),
            progress,
            reconsuming: false,
            rating_twenty: None,
        }
    }
}

struct State {
    mal: Vec<MalEntry>,
    kitsu: Vec<KitsuEntry>,
    seen: Vec<(String, String)>,
}

pub struct FakeReadLists {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    task: JoinHandle<()>,
}

impl Drop for FakeReadLists {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FakeReadLists {
    pub async fn start() -> Self {
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let state = Arc::new(Mutex::new(State {
            mal: Vec::new(),
            kitsu: Vec::new(),
            seen: Vec::new(),
        }));
        let task = tokio::spawn({
            let state = state.clone();
            async move {
                while let Ok((socket, _)) = listener.accept().await {
                    tokio::spawn(serve(socket, state.clone(), addr));
                }
            }
        });
        Self { addr, state, task }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// For `MalTracker::with_api`.
    pub fn mal_api(&self) -> String {
        format!("http://{}/v2", self.addr)
    }

    /// For `kitsu::Site::at`.
    pub fn kitsu_base(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn add_mal(&self, entry: MalEntry) {
        self.state().mal.push(entry);
    }

    pub fn add_kitsu(&self, entry: KitsuEntry) {
        self.state().kitsu.push(entry);
    }

    pub fn mal(&self, id: u32) -> Option<MalEntry> {
        self.state().mal.iter().find(|e| e.id == id).cloned()
    }

    pub fn kitsu(&self, manga: u32) -> Option<KitsuEntry> {
        self.state().kitsu.iter().find(|e| e.manga == manga).cloned()
    }

    /// `(method, path)` of every request.
    pub fn requests(&self) -> Vec<(String, String)> {
        self.state().seen.clone()
    }
}

struct Request {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    authorization: String,
    content_type: String,
    body: String,
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn pairs(text: &str) -> Vec<(String, String)> {
    text.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect()
}

async fn serve(socket: TcpStream, state: Arc<Mutex<State>>, addr: SocketAddr) {
    let (read, mut write) = socket.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();
    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let (mut length, mut authorization, mut content_type) =
        (0usize, String::new(), String::new());
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await.unwrap_or(0) == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().unwrap_or(0).min(1 << 20),
                "authorization" => authorization = value.trim().to_owned(),
                "content-type" => content_type = value.trim().to_owned(),
                _ => {}
            }
        }
    }
    let mut raw = vec![0u8; length];
    let _ = reader.read_exact(&mut raw).await;
    let request = Request {
        method,
        path: decode(path),
        query: pairs(query),
        authorization,
        content_type,
        body: String::from_utf8_lossy(&raw).into_owned(),
    };
    let (status, reply) = answer(&state, &request, addr);
    let text = reply.to_string();
    let head = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        text.len()
    );
    let _ = write.write_all(head.as_bytes()).await;
    let _ = write.write_all(text.as_bytes()).await;
    let _ = write.shutdown().await;
}

fn answer(state: &Mutex<State>, r: &Request, addr: SocketAddr) -> (u16, Value) {
    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
    state.seen.push((r.method.clone(), r.path.clone()));
    if let Some(path) = r.path.strip_prefix("/v2") {
        if r.authorization != format!("Bearer {MAL_TOKEN}") {
            return (401, json!({ "error": "invalid_token" }));
        }
        return mal(&mut state, r, path, addr);
    }
    if let Some(path) = r.path.strip_prefix("/api/edge") {
        if r.authorization != format!("Bearer {KITSU_TOKEN}") {
            return (401, json!({ "errors": [{ "title": "Unauthorized" }] }));
        }
        return kitsu(&mut state, r, path, addr);
    }
    (404, json!({}))
}

fn query<'a>(r: &'a Request, name: &str) -> Option<&'a str> {
    r.query.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

fn mal(state: &mut State, r: &Request, path: &str, addr: SocketAddr) -> (u16, Value) {
    match (r.method.as_str(), path) {
        ("GET", "/users/@me/mangalist") => {
            let offset: usize = query(r, "offset").and_then(|o| o.parse().ok()).unwrap_or(0);
            let data: Vec<Value> = state
                .mal
                .iter()
                .skip(offset)
                .take(PAGE)
                .map(|e| {
                    json!({
                        "node": { "id": e.id, "title": format!("Lantern Road {}", e.id),
                                  "media_type": e.media_type },
                        "list_status": { "status": e.status, "score": e.score,
                                         "num_volumes_read": e.volumes,
                                         "num_chapters_read": e.chapters,
                                         "is_rereading": e.rereading },
                    })
                })
                .collect();
            let mut paging = json!({});
            if offset + PAGE < state.mal.len() {
                paging["next"] = json!(format!(
                    "http://{addr}/v2/users/@me/mangalist?fields=list_status,media_type&offset={}",
                    offset + PAGE
                ));
            }
            (200, json!({ "data": data, "paging": paging }))
        }
        ("PATCH", p) if p.starts_with("/manga/") && p.ends_with("/my_list_status") => {
            let id: u32 = p
                .trim_start_matches("/manga/")
                .trim_end_matches("/my_list_status")
                .parse()
                .unwrap_or(0);
            if !r.content_type.starts_with("application/x-www-form-urlencoded") {
                return (400, json!({ "error": "invalid_content" }));
            }
            let form = pairs(&r.body);
            let at = match state.mal.iter().position(|e| e.id == id) {
                Some(at) => at,
                None => {
                    state.mal.push(MalEntry::new(id, "reading", 0, 0));
                    state.mal.len() - 1
                }
            };
            let entry = &mut state.mal[at];
            for (k, v) in form {
                match k.as_str() {
                    "status" => entry.status = v,
                    "num_chapters_read" => entry.chapters = v.parse().unwrap_or(0),
                    "num_volumes_read" => entry.volumes = v.parse().unwrap_or(0),
                    "is_rereading" => entry.rereading = v == "true",
                    "score" => entry.score = v.parse().unwrap_or(0),
                    _ => {}
                }
            }
            (200, json!({ "status": entry.status }))
        }
        ("DELETE", p) if p.starts_with("/manga/") && p.ends_with("/my_list_status") => {
            let id: u32 = p
                .trim_start_matches("/manga/")
                .trim_end_matches("/my_list_status")
                .parse()
                .unwrap_or(0);
            let before = state.mal.len();
            state.mal.retain(|e| e.id != id);
            if state.mal.len() == before {
                (404, json!({ "error": "not_found" }))
            } else {
                (200, json!([]))
            }
        }
        _ => (404, json!({})),
    }
}

fn kitsu(state: &mut State, r: &Request, path: &str, addr: SocketAddr) -> (u16, Value) {
    let body: Value = serde_json::from_str(&r.body).unwrap_or(Value::Null);
    match (r.method.as_str(), path) {
        ("GET", "/users") => (200, json!({ "data": [{ "id": KITSU_USER, "type": "users" }] })),
        ("GET", "/library-entries") => {
            if let Some(manga) = query(r, "filter[mangaId]") {
                let found: Vec<Value> = state
                    .kitsu
                    .iter()
                    .filter(|e| e.manga.to_string() == manga)
                    .map(|e| json!({ "id": e.entry_id, "type": "libraryEntries" }))
                    .collect();
                return (200, json!({ "data": found }));
            }
            let offset: usize =
                query(r, "page[offset]").and_then(|o| o.parse().ok()).unwrap_or(0);
            let page: Vec<&KitsuEntry> = state.kitsu.iter().skip(offset).take(PAGE).collect();
            let data: Vec<Value> = page
                .iter()
                .map(|e| {
                    json!({
                        "id": e.entry_id, "type": "libraryEntries",
                        "attributes": { "status": e.status, "progress": e.progress,
                                        "reconsuming": e.reconsuming,
                                        "ratingTwenty": e.rating_twenty },
                        "relationships": { "manga": { "data":
                            { "type": "manga", "id": e.manga.to_string() } } },
                    })
                })
                .collect();
            let included: Vec<Value> = page
                .iter()
                .map(|e| {
                    json!({ "id": e.manga.to_string(), "type": "manga",
                                 "attributes": { "subtype": e.subtype } })
                })
                .collect();
            let mut links = json!({});
            if offset + PAGE < state.kitsu.len() {
                links["next"] = json!(format!(
                    "http://{addr}/api/edge/library-entries?filter[kind]=manga&page[offset]={}",
                    offset + PAGE
                ));
            }
            (200, json!({ "data": data, "included": included, "links": links }))
        }
        ("POST", "/library-entries") => {
            let attrs = &body["data"]["attributes"];
            if attrs["status"].as_str().is_none() {
                return (422, json!({ "errors": [{ "title": "status can't be blank" }] }));
            }
            let Some(manga) = body["data"]["relationships"]["manga"]["data"]["id"]
                .as_str()
                .and_then(|m| m.parse::<u32>().ok())
            else {
                return (422, json!({ "errors": [{ "title": "manga is required" }] }));
            };
            let mut entry = KitsuEntry::new(manga, attrs["status"].as_str().unwrap_or(""), 0);
            apply(&mut entry, attrs);
            state.kitsu.push(entry);
            (201, json!({ "data": { "id": format!("e{manga}") } }))
        }
        ("PATCH", p) if p.starts_with("/library-entries/") => {
            let id = p.trim_start_matches("/library-entries/");
            match state.kitsu.iter_mut().find(|e| e.entry_id == id) {
                Some(entry) => {
                    apply(entry, &body["data"]["attributes"]);
                    (200, json!({ "data": { "id": id } }))
                }
                None => (404, json!({ "errors": [{ "title": "not found" }] })),
            }
        }
        ("DELETE", p) if p.starts_with("/library-entries/") => {
            let id = p.trim_start_matches("/library-entries/");
            let before = state.kitsu.len();
            state.kitsu.retain(|e| e.entry_id != id);
            if state.kitsu.len() == before {
                (404, json!({ "errors": [{ "title": "not found" }] }))
            } else {
                (200, json!({}))
            }
        }
        _ => (404, json!({})),
    }
}

fn apply(entry: &mut KitsuEntry, attrs: &Value) {
    if let Some(status) = attrs["status"].as_str() {
        entry.status = status.to_owned();
    }
    if let Some(progress) = attrs["progress"].as_u64() {
        entry.progress = progress as u32;
    }
    if let Some(flag) = attrs["reconsuming"].as_bool() {
        entry.reconsuming = flag;
    }
    if let Some(rating) = attrs["ratingTwenty"].as_u64() {
        entry.rating_twenty = Some(rating as u32);
    }
}
