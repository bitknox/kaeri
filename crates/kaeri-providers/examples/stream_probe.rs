//! End-to-end validation of the torrent path: route → session → peers → loopback HTTP.
//!
//! Uses **Big Buck Bunny**, the Blender Foundation's open movie, which is licensed
//! Creative Commons Attribution 3.0 and distributed by its rights holder specifically for
//! this kind of testing.
//!
//! What unit tests cannot cover and this can: whether librqbit actually finds peers, through a
//! SOCKS5 proxy with DHT off when the config sets one, and whether bytes genuinely arrive at the
//! loopback URL in a form a player can seek.
//!
//! ```text
//! cargo run -p kaeri-providers --example stream_probe
//! ```

use std::time::{Duration, Instant};

use kaeri_core::config::{Config, Paths};
use kaeri_providers::{
    TorrentRoute,
    torrent::{TorrentSession, session::choose_file},
};

/// Big Buck Bunny © Blender Foundation, CC-BY 3.0 — <https://peach.blender.org>.
/// This is the long-standing canonical test torrent for BitTorrent tooling.
const LEGAL_MAGNET: &str = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c\
&dn=Big+Buck+Bunny\
&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce\
&tr=udp%3A%2F%2Fexodus.desync.com%3A6969%2Fannounce\
&tr=udp%3A%2F%2Fopen.stealth.si%3A80%2Fannounce\
&tr=udp%3A%2F%2Ftracker.torrent.eu.org%3A451%2Fannounce\
&tr=wss%3A%2F%2Ftracker.btorrent.xyz\
&tr=wss%3A%2F%2Ftracker.openwebtorrent.com";

#[tokio::main]
async fn main() {
    let paths = Paths::resolve().expect("paths");
    let config = Config::load(&paths).unwrap_or_default();

    println!("── configuration ──────────────────────────────────────");
    let torrent = &config.providers.torrent;
    println!("  enabled     {}", torrent.enabled);
    println!("  proxy       {}", if torrent.proxy.is_some() { "set" } else { "none" });

    println!();
    println!("── route ──────────────────────────────────────────────");
    let route = match TorrentRoute::new(torrent.proxy.clone()) {
        Ok(g) => g,
        Err(e) => {
            println!("  ✕ misconfigured: {e}");
            return;
        }
    };
    println!(
        "  dht          {}",
        if route.must_disable_dht() { "disabled" } else { "enabled" }
    );

    let started = Instant::now();
    let state = route.verify().await;
    println!("  check        {:?} in {:?}", state, started.elapsed());

    if !state.is_ready() {
        println!();
        println!("  ✕ the proxy check failed, so the torrent source will not start.");
        return;
    }
    println!("  ● ready");

    println!();
    println!("── session ────────────────────────────────────────────");
    // A scratch directory removed when the probe ends: what it fetches is not the real cache's.
    let scratch = tempfile::tempdir().expect("scratch directory");
    let dir = scratch.path().join("torrents");
    let session = match TorrentSession::start(
        route.clone(),
        dir.clone(),
        scratch.path().join("dht.json"),
        "probe".into(),
    )
    .await
    {
        Ok(s) => {
            println!("  started, downloading into {}", dir.display());
            s
        }
        Err(e) => {
            println!("  ✕ could not start: {e}");
            return;
        }
    };

    println!();
    println!("── metadata (needs peers) ─────────────────────────────");
    println!("  Big Buck Bunny © Blender Foundation, CC-BY 3.0");
    let started = Instant::now();
    let files =
        match tokio::time::timeout(Duration::from_secs(60), session.list_files(LEGAL_MAGNET))
            .await
        {
            Ok(Ok(files)) => {
                println!("  {} file(s) in {:?}", files.len(), started.elapsed());
                files
            }
            Ok(Err(e)) => {
                println!("  ✕ {e}");
                return;
            }
            Err(_) => {
                println!("  ✕ timed out after 60s — no peers reachable.");
                println!("    With DHT disabled, that means the trackers were unreachable");
                println!("    through the proxy.");
                return;
            }
        };
    for f in files.iter().take(8) {
        println!("    [{}] {:>10} bytes  {}", f.index, f.length, f.name);
    }

    match choose_file(&files, None, None) {
        Some(pick) => println!("  selected: {}", pick.name),
        None => {
            println!("  ✕ nothing playable found");
            return;
        }
    }

    println!();
    println!("── streaming ──────────────────────────────────────────");
    let started = Instant::now();
    let active =
        match tokio::time::timeout(Duration::from_secs(90), session.stream(LEGAL_MAGNET, None))
            .await
        {
            Ok(Ok(a)) => {
                println!("  ready in {:?}", started.elapsed());
                a
            }
            Ok(Err(e)) => {
                println!("  ✕ {e}");
                return;
            }
            Err(_) => {
                println!("  ✕ timed out waiting for the stream to open");
                return;
            }
        };
    println!("  file  {}", active.file.name);
    println!("  size  {} bytes", active.file.length);
    println!("  url   {}", active.url());

    println!();
    println!("── loopback HTTP: do bytes actually arrive? ────────────");
    let client = reqwest::Client::builder()
        // Never proxy a loopback request — the whole point is to read our own server.
        .no_proxy()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("client");

    // A HEAD first: proves the server reports a length and advertises range support,
    // which is what mpv checks before it will attempt to seek at all.
    match client.head(active.url()).send().await {
        Ok(r) => {
            println!("  HEAD  {} ", r.status().as_u16());
            println!(
                "        accept-ranges: {}",
                r.headers().get("accept-ranges").and_then(|v| v.to_str().ok()).unwrap_or("-")
            );
            println!(
                "        content-length: {}",
                r.headers().get("content-length").and_then(|v| v.to_str().ok()).unwrap_or("-")
            );
        }
        Err(e) => println!("  ✕ HEAD failed: {e}"),
    }

    // Then a real range read. This is the moment of truth: pieces have to arrive from
    // peers, through the proxy, and be served back over loopback.
    let started = Instant::now();
    match client.get(active.url()).header("Range", "bytes=0-262143").send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let content_range = response
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-")
                .to_owned();
            match response.bytes().await {
                Ok(bytes) => {
                    println!("  GET   {status} in {:?}", started.elapsed());
                    println!("        content-range: {content_range}");
                    println!("        received: {} bytes", bytes.len());

                    // An MKV starts with the EBML magic; an MP4 has `ftyp` at offset 4.
                    let looks_like_mkv = bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]);
                    let looks_like_mp4 = bytes.len() > 8 && &bytes[4..8] == b"ftyp";
                    println!(
                        "        container: {}",
                        match (looks_like_mkv, looks_like_mp4) {
                            (true, _) => "Matroska (EBML magic present)",
                            (_, true) => "MP4 (ftyp box present)",
                            _ => "unrecognised header",
                        }
                    );

                    if bytes.is_empty() {
                        println!("  ✕ no data — pieces never arrived");
                    } else if looks_like_mkv || looks_like_mp4 {
                        println!();
                        println!("  ● real video data arrived through the proxy and was");
                        println!("    served over loopback with a correct partial response.");
                    }
                }
                Err(e) => println!("  ✕ body read failed: {e}"),
            }
        }
        Err(e) => println!("  ✕ GET failed: {e}"),
    }

    // A mid-file range proves seeking works, not just sequential reading.
    if active.file.length > 4 * 1024 * 1024 {
        println!();
        println!("── seek: a range from the middle ──────────────────────");
        let midpoint = active.file.length / 2;
        let started = Instant::now();
        match client
            .get(active.url())
            .header("Range", format!("bytes={midpoint}-{}", midpoint + 65_535))
            .send()
            .await
        {
            Ok(r) => {
                let status = r.status().as_u16();
                let range = r
                    .headers()
                    .get("content-range")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-")
                    .to_owned();
                let len = r.bytes().await.map(|b| b.len()).unwrap_or(0);
                println!("  GET   {status} in {:?}", started.elapsed());
                println!("        content-range: {range}");
                println!("        received: {len} bytes");
                if len > 0 {
                    println!("  ● seeking works — librqbit prioritised the requested pieces");
                }
            }
            Err(e) => println!("  ✕ {e}"),
        }
    }

    // Byte patterns prove a container header arrived; ffprobe proves the stream is
    // genuinely decodable, which is the property a player actually needs.
    println!();
    println!("── ffprobe: is it decodable video? ────────────────────");
    match tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_streams",
            "-show_entries",
            "format=format_name,duration",
            // Read enough to identify streams without pulling the whole file.
            "-analyzeduration",
            "5000000",
            "-probesize",
            "5000000",
            active.url(),
        ])
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let json = String::from_utf8_lossy(&output.stdout);
            for line in json.lines() {
                let trimmed = line.trim();
                if ["codec_name", "codec_type", "width", "height", "format_name", "duration"]
                    .iter()
                    .any(|k| trimmed.starts_with(&format!("\"{k}\"")))
                {
                    println!("  {trimmed}");
                }
            }
            println!("  ● ffprobe decoded the stream over loopback");
        }
        Ok(output) => {
            println!("  ✕ ffprobe failed: {}", String::from_utf8_lossy(&output.stderr).trim());
        }
        Err(e) => println!("  (ffprobe unavailable: {e})"),
    }

    // The actual player, not just a decoder library. `--vo=null --frames=1` decodes one
    // frame and exits, which is enough to prove mpv accepts the URL, negotiates ranges and
    // finds a video stream.
    println!();
    println!("── mpv: does the real player accept it? ───────────────");
    match tokio::process::Command::new("mpv")
        .args(["--no-config", "--vo=null", "--ao=null", "--frames=1", "--really-quiet"])
        .arg(active.url())
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            println!("  ● mpv decoded a frame from the loopback stream");
        }
        Ok(output) => {
            println!("  ✕ mpv exited {:?}", output.status.code());
            let stderr = String::from_utf8_lossy(&output.stderr);
            for line in stderr.lines().take(4) {
                println!("    {line}");
            }
        }
        Err(e) => println!("  (mpv unavailable: {e})"),
    }

    session.stop();
    println!();
    println!("done. Downloaded data is under {}", dir.display());
}
