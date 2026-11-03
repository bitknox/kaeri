//! The loopback listener a desktop sign-in comes back to: the browser is sent to
//! `http://127.0.0.1:<port>/…` with the authorisation code in the query, and this catches that
//! one request.
//!
//! The port is bound before the browser opens, so a sign-in that finishes at once finds it open.
//! `localhost` may resolve to the IPv6 address, so the same port is bound there too when the
//! machine allows.

use std::{sync::Arc, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinHandle,
};

use crate::folder::FolderError;

/// How long a connection may take to send its request line.
const READ_WAIT: Duration = Duration::from_secs(5);

const PAGE_HEAD: &str = concat!(
    "<!doctype html><meta charset=utf-8><title>Kaeri</title>",
    "<style>:root{color-scheme:light dark}",
    "body{font:15px/1.6 ui-monospace,monospace;margin:12vh auto;max-width:30rem;",
    "padding:0 1.5rem}p{color:#8B90AD}</style>",
    "<h1>Kaeri</h1><p>"
);

/// The page the browser shows once the redirect is caught, saying `line` (the engine's
/// translated text).
fn page(line: &str) -> String {
    let escaped = line
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    format!("{PAGE_HEAD}{escaped}</p>")
}

/// What the redirect carried: its query, decoded.
pub type Params = Vec<(String, String)>;

pub struct Loopback {
    port: u16,
    callbacks: mpsc::UnboundedReceiver<Params>,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for Loopback {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// Listen on the first of `ports` that is free (`0` is any port), for requests to `path`. The
/// page the browser is left on says `line`.
pub async fn bind(ports: &[u16], path: &str, line: &str) -> Result<Loopback, FolderError> {
    let mut bound = None;
    for port in ports {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", *port)).await {
            bound = Some(listener);
            break;
        }
    }
    let first = bound.ok_or_else(|| {
        FolderError::Unreachable("no port on this computer is free for the sign-in".into())
    })?;
    let port = first.local_addr().map(|a| a.port()).unwrap_or_default();
    let mut listeners = vec![first];
    if let Ok(v6) = TcpListener::bind(("::1", port)).await {
        listeners.push(v6);
    }
    let (tx, callbacks) = mpsc::unbounded_channel();
    let body: Arc<str> = page(line).into();
    let tasks = listeners
        .into_iter()
        .map(|listener| {
            let (tx, path, body) = (tx.clone(), path.to_owned(), body.clone());
            tokio::spawn(async move {
                while let Ok((socket, _)) = listener.accept().await {
                    let (tx, path, body) = (tx.clone(), path.clone(), body.clone());
                    tokio::spawn(async move {
                        let _ = serve(socket, &path, &body, &tx).await;
                    });
                }
            })
        })
        .collect();
    Ok(Loopback { port, callbacks, tasks })
}

impl Loopback {
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The next request to the path that carries a `code` or an `error`. A browser's request for
    /// its icon, or for anything else, is answered and skipped.
    pub async fn next(&mut self) -> Option<Params> {
        self.callbacks.recv().await
    }
}

async fn serve(
    mut socket: TcpStream,
    path: &str,
    page: &str,
    tx: &mpsc::UnboundedSender<Params>,
) -> std::io::Result<()> {
    let mut head = Vec::new();
    let mut chunk = [0u8; 2048];
    let read = async {
        while !head.windows(2).any(|w| w == b"\r\n") && head.len() < 16 * 1024 {
            let n = socket.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            head.extend_from_slice(&chunk[..n]);
        }
        Ok::<_, std::io::Error>(())
    };
    if tokio::time::timeout(READ_WAIT, read).await.is_err() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&head).into_owned();
    let target = text.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("/");
    let mut hit = false;
    if let Ok(url) = reqwest::Url::parse(&format!("http://localhost{target}")) {
        let params: Params =
            url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
        if url.path() == path && params.iter().any(|(k, _)| k == "code" || k == "error") {
            hit = true;
            let _ = tx.send(params);
        }
    }
    let (status, body) = if hit { ("200 OK", page) } else { ("404 Not Found", "") };
    let reply = format!(
        "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(reply.as_bytes()).await?;
    socket.shutdown().await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn get(port: u16, target: &str) -> u16 {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(format!("GET {target} HTTP/1.1\r\nhost: x\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out.split_whitespace().nth(1).unwrap().parse().unwrap()
    }

    #[tokio::test]
    async fn the_request_with_a_code_comes_through_and_others_are_skipped() {
        let mut listener = bind(&[0], "/auth/x", "Fermez <cet> onglet.").await.unwrap();
        let port = listener.port();
        assert_eq!(get(port, "/favicon.ico").await, 404);
        assert_eq!(get(port, "/auth/x").await, 404);
        assert_eq!(get(port, "/auth/x?code=a%20b&state=s").await, 200);
        // The page says the line it was given, as text.
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(b"GET /auth/x?code=c&state=s HTTP/1.1\r\nhost: x\r\n\r\n").await.unwrap();
        let mut page = String::new();
        s.read_to_string(&mut page).await.unwrap();
        assert!(page.contains("<p>Fermez &lt;cet&gt; onglet.</p>"), "{page}");
        let params = listener.next().await.unwrap();
        assert_eq!(params, vec![("code".into(), "a b".into()), ("state".into(), "s".into())]);
    }

    #[tokio::test]
    async fn a_taken_port_falls_through_to_the_next() {
        let held = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let taken = held.local_addr().unwrap().port();
        let listener = bind(&[taken, 0], "/", "").await.unwrap();
        assert_ne!(listener.port(), taken);
        assert!(bind(&[taken], "/", "").await.is_err());
    }
}
