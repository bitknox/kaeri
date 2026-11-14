//! A tiny in-process HTTP server for the recorded tests: canned answers by path prefix, every
//! request kept.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

pub type Seen = Arc<Mutex<Vec<(String, String, String)>>>;

/// Serves canned JSON by path prefix; records (method, path, body).
pub async fn serve(routes: HashMap<&'static str, &'static str>) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let log = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let routes = routes.clone();
            let log = log.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let mut first = text.lines().next().unwrap_or_default().split(' ');
                let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
                let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
                log.lock().unwrap().push((method.into(), path.into(), body));
                let answer = routes
                    .iter()
                    .filter(|(prefix, _)| path.starts_with(**prefix))
                    .max_by_key(|(prefix, _)| prefix.len())
                    .map(|(_, json)| *json);
                let response = match answer {
                    Some(answer) => {
                        // An answer may start with header lines, ended by a blank line.
                        let (headers, json) = answer.split_once("\n\n").unwrap_or(("", answer));
                        let headers: String =
                            headers.lines().map(|h| format!("{h}\r\n")).collect();
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{json}",
                            json.len()
                        )
                    }
                    None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
                };
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    (address, seen)
}
