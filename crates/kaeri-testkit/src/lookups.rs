//! Stand-ins for the three public lookups that map a work to tracker entries: MangaBaka's source
//! lookup (`/v1/source/anilist/{id}`), Kitsu's mappings (`/mappings`) and MangaUpdates' series
//! search (`/v1/series/search`). Each answers the body a test gives it, so the test can hold a
//! fixture; none is ever a real service's answer.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, MutexGuard},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

#[derive(Default)]
struct State {
    baka: Option<String>,
    kitsu: Option<String>,
    search: Option<String>,
    seen: Vec<String>,
}

pub struct FakeLookups {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    task: JoinHandle<()>,
}

impl Drop for FakeLookups {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FakeLookups {
    pub async fn start() -> Self {
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let state = Arc::new(Mutex::new(State::default()));
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

    pub fn base(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn kitsu_api(&self) -> String {
        format!("http://{}/api/edge", self.addr)
    }

    pub fn mangaupdates_api(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    /// The body MangaBaka answers, or none for a 404.
    pub fn set_mangabaka(&self, body: Option<&str>) {
        self.state().baka = body.map(str::to_owned);
    }

    pub fn set_kitsu_mappings(&self, body: &str) {
        self.state().kitsu = Some(body.to_owned());
    }

    pub fn set_search(&self, body: &str) {
        self.state().search = Some(body.to_owned());
    }

    /// Every request line received.
    pub fn requests(&self) -> Vec<String> {
        self.state().seen.clone()
    }
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
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await.unwrap_or(0) == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0).min(1 << 20);
        }
    }
    let mut raw = vec![0u8; length];
    let _ = reader.read_exact(&mut raw).await;
    let (status, body) = {
        let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
        state.seen.push(format!("{method} {target}"));
        if method == "GET" && path.starts_with("/v1/source/anilist/") {
            match &state.baka {
                Some(body) => (200, body.clone()),
                None => (404, r#"{"status":404,"message":"NOT_FOUND"}"#.to_owned()),
            }
        } else if method == "GET" && path == "/api/edge/mappings" {
            (200, state.kitsu.clone().unwrap_or_else(|| r#"{"data":[]}"#.to_owned()))
        } else if method == "POST" && path == "/v1/series/search" {
            (200, state.search.clone().unwrap_or_else(|| r#"{"results":[]}"#.to_owned()))
        } else {
            (404, "{}".to_owned())
        }
    };
    let head = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    let _ = write.write_all(head.as_bytes()).await;
    let _ = write.write_all(body.as_bytes()).await;
    let _ = write.shutdown().await;
}
