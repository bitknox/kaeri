//! A tiny HTTP server on the loopback interface for the cloud targets' tests: one request per
//! connection, a handler that answers it.

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

pub(crate) struct Req {
    pub method: String,
    /// Decoded.
    pub path: String,
    pub query: Vec<(String, String)>,
    /// Names lowercased.
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Req {
    pub fn param(&self, key: &str) -> Option<&str> {
        self.query.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

pub(crate) struct Resp {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Resp {
    pub fn status(status: u16) -> Self {
        Self { status, headers: Vec::new(), body: Vec::new() }
    }

    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Self { status, headers: Vec::new(), body: value.to_string().into_bytes() }
    }

    pub fn bytes(body: &[u8]) -> Self {
        Self { status: 200, headers: Vec::new(), body: body.to_vec() }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

pub(crate) struct Server {
    pub addr: SocketAddr,
    task: JoinHandle<()>,
}

impl Server {
    pub fn base(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub async fn start<H>(handler: H) -> Self
    where
        H: Fn(Req) -> Resp + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
        let addr = listener.local_addr().expect("local address");
        let handler = Arc::new(handler);
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { break };
                let handler = handler.clone();
                tokio::spawn(async move {
                    let _ = serve(stream, &*handler).await;
                });
            }
        });
        Self { addr, task }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

async fn serve<H>(mut stream: TcpStream, handler: &H) -> std::io::Result<()>
where
    H: Fn(Req) -> Resp,
{
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.lines();
    let first = lines.next().unwrap_or_default();
    let mut parts = first.split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default();
    let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    let headers: HashMap<String, String> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let want: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < want {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let query = raw_query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect();
    let resp = handler(Req { method, path: decode(raw_path), query, headers, body });
    let mut out = format!(
        "HTTP/1.1 {} X\r\nConnection: close\r\nContent-Length: {}\r\n",
        resp.status,
        resp.body.len()
    );
    for (k, v) in &resp.headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str("\r\n");
    stream.write_all(out.as_bytes()).await?;
    stream.write_all(&resp.body).await?;
    stream.shutdown().await
}
