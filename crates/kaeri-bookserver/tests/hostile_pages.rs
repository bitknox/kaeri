//! A server's page counts are its word: one that states more pages than any book has must not
//! make the client build a page ref for each.

use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use kaeri_bookserver::{Kind, MOST_PAGES, Server, source};
use kaeri_core::{books::BookHints, traits::BookSource};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Serve each path that starts with one of `pages`' prefixes with its body; 404 otherwise.
async fn serve(pages: Vec<(&'static str, &'static str, String)>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let pages = pages.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = text.split(' ').nth(1).unwrap_or("").to_owned();
                let found = pages.iter().find(|(p, _, _)| path.starts_with(p));
                let reply = match found {
                    Some((_, kind, body)) => format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    ),
                    None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .into(),
                };
                let _ = stream.write_all(reply.as_bytes()).await;
            });
        }
    });
    address
}

/// An acquisition feed of one book read through a page stream of `count` pages, with no file.
fn books_feed(count: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:pse="http://vaemendis.net/opds-pse/ns">
 <id>urn:x:s1</id><title>Lantern Keeper</title><updated>2026-01-01T00:00:00Z</updated>
 <entry><id>urn:x:b1</id><title>Lantern Keeper v01</title><updated>2026-01-02T00:00:00Z</updated>
  <link rel="http://vaemendis.net/opds-pse/stream" href="/pages/b1/{{pageNumber}}?maxWidth={{maxWidth}}" type="image/jpeg" pse:count="{count}"/></entry>
</feed>"#
    )
}

fn opds(address: &str) -> Arc<dyn BookSource> {
    let server =
        Server { id: "s", name: "Shelf", kind: Kind::Opds, address, credential: "basic:" };
    source(
        &reqwest::Client::new(),
        &server,
        std::env::temp_dir(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/// Every call to a hostile server answers soon: building billions of page refs would not.
async fn soon<T>(work: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(20), work).await.expect("the call answered")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_opds_page_stream_past_any_books_length_is_not_read() {
    let most = MOST_PAGES.to_string();
    let one_more = (MOST_PAGES + 1).to_string();
    for (count, readable) in [
        (most.as_str(), true),
        (one_more.as_str(), false),
        ("4000000000", false),
        ("4294967295", false),
    ] {
        let address =
            serve(vec![("/opds/s1", "application/atom+xml", books_feed(count))]).await;
        let source = opds(&address);
        let series = format!("feed|{address}/opds/s1");

        let chapters = soon(source.chapters(&series, &BookHints::default())).await.unwrap();
        let stated = chapters[0].page_count;
        assert_eq!(
            stated,
            readable.then_some(MOST_PAGES),
            "{count} pages listed as {stated:?}"
        );

        let pages = soon(source.chapter_pages(&series, "urn:x:b1")).await.unwrap();
        assert_eq!(pages.map(|p| p.len()), readable.then_some(MOST_PAGES as usize), "{count}");

        let content = soon(source.content(&series, "urn:x:b1")).await;
        if readable {
            assert!(content.is_ok(), "{count}: {content:?}");
        } else {
            let error = content.expect_err(count);
            assert!(error.to_string().contains("pages"), "{count}: {error}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_kavita_chapter_past_any_books_length_is_not_read() {
    for (count, readable) in [
        (u64::from(MOST_PAGES), true),
        (u64::from(MOST_PAGES) + 1, false),
        (u64::from(u32::MAX), false),
        (u64::MAX, false),
    ] {
        let info = format!(r#"{{"pages":{count},"isEpub":false,"fileFormat":1}}"#);
        let address = serve(vec![("/api/Reader/chapter-info", "application/json", info)]).await;
        let server = Server {
            id: "k",
            name: "Shelf",
            kind: Kind::Kavita,
            address: &address,
            credential: r#"kavita:{"token":"t","apiKey":"k"}"#,
        };
        let source = source(
            &reqwest::Client::new(),
            &server,
            std::env::temp_dir(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();

        let pages = soon(source.chapter_pages("1", "7")).await;
        if readable {
            assert_eq!(pages.unwrap().map(|p| p.len()), Some(MOST_PAGES as usize));
        } else {
            let error = pages.expect_err("too many pages");
            assert!(error.to_string().contains("pages"), "{count}: {error}");
        }
    }
}
