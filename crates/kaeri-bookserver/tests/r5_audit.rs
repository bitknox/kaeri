//! Round 5 engine audit: what a book server answers is untrusted text.

use kaeri_bookserver::opds;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A page of `len` bytes or a little more whose byte 2048 falls inside a three-byte letter.
fn page_split_at_2048() -> String {
    let mut page = String::from("<!DOCTYPE html><html><body><p>");
    while page.len() < 2047 {
        page.push('a');
    }
    // 2047..2050 is one letter: 2048 is inside it.
    page.push('\u{3042}');
    page.push_str("</p></body></html>");
    assert!(!page.is_char_boundary(2048));
    page
}

/// R5E-3. `opds::is_feed` looks at the first 2048 bytes of an answer, and byte 2048 may fall
/// inside a letter.
#[test]
fn is_feed_takes_any_text() {
    let page = page_split_at_2048();
    let run = std::panic::catch_unwind(|| opds::is_feed(&page));
    assert!(run.is_ok(), "is_feed panicked on a page whose byte 2048 is inside a letter");
}

/// R5E-3 through the public way in: adding a server whose address serves a web page with
/// such a letter near 2 KB (any page in Japanese, or a dash or a quote mark there).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adding_a_server_that_serves_a_web_page_answers() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let page = page_split_at_2048();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let page = page.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let found = !head.contains("/api/");
                let (status, body) =
                    if found { ("200 OK", page) } else { ("404 Not Found", String::new()) };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes()).await;
            });
        }
    });
    let client = reqwest::Client::new();
    let checked =
        tokio::spawn(async move { kaeri_bookserver::check(&client, &address).await }).await;
    assert!(checked.is_ok(), "the check panicked: {:?}", checked.err());
}
