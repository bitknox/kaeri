//! The first seconds of the next episode, fetched through its proxy while this one's ending
//! plays, so the player finds them in the proxy's cache when the episode starts.
//!
//! The walk is the player's own first steps: the playlist it opens, the variant mpv picks by
//! default (the highest bandwidth) with its default audio rendition, then the segments from the
//! resume point on. A playlist addressed by byte ranges is passed through rather than cached
//! (see the proxy's range rule), so nothing of one is fetched ahead.

use std::sync::Arc;

use super::{ProxyServer, Upstream, lock, playlist::resolve_url};

impl ProxyServer {
    /// Fetch the playlists and about `seconds` of segments from `from` on into this proxy's
    /// cache, the way the player will ask for them first. A failure stops the walk where it
    /// is; what was fetched by then stays, and the player fetches the rest as it would have.
    pub async fn warm(&mut self, from: f64, seconds: f64) {
        let mut fetched = Vec::new();
        if let Err(reason) =
            walk(&self.upstream, &self.origin, from, seconds, &mut fetched).await
        {
            tracing::info!(%reason, "the next episode's first seconds were not all fetched");
        }
        tracing::info!(segments = fetched.len(), "the next episode's first seconds fetched");
        self.warmed = fetched;
    }

    /// What frees the segments [`Self::warm`] fetched, once the player has its own copy.
    pub fn release(&self) -> Release {
        Release { upstream: Arc::clone(&self.upstream), urls: self.warmed.clone() }
    }

    /// The stream this proxy fronts.
    pub fn origin(&self) -> &str {
        &self.origin
    }
}

/// The segments fetched ahead for an episode, to drop from its proxy's cache once it plays.
pub struct Release {
    upstream: Arc<Upstream>,
    urls: Vec<String>,
}

impl Release {
    /// Drop them. The player has read them by now; a later read fetches again.
    pub fn run(self) {
        let mut cache = lock(&self.upstream.cache);
        for url in &self.urls {
            cache.remove(url);
        }
    }

    /// How many segments it frees.
    pub fn len(&self) -> usize {
        self.urls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.urls.is_empty()
    }
}

/// Walk from `origin` to its first segments, pushing each segment fetched on `fetched`.
async fn walk(
    upstream: &Upstream,
    origin: &str,
    from: f64,
    seconds: f64,
    fetched: &mut Vec<String>,
) -> Result<(), String> {
    let top = text(upstream, origin).await?;
    let media = if top.contains("#EXT-X-STREAM-INF") {
        let (variant, audio) = best_variant(&top, origin).ok_or("no variant listed")?;
        let mut media = vec![(text(upstream, &variant).await?, variant)];
        if let Some(rendition) = audio.and_then(|group| audio_rendition(&top, origin, &group)) {
            media.push((text(upstream, &rendition).await?, rendition));
        }
        media
    } else {
        vec![(top, origin.to_owned())]
    };
    for (body, base) in media {
        if body.contains("#EXT-X-BYTERANGE") {
            continue;
        }
        for segment in first_segments(&body, &base, from, seconds) {
            upstream.for_playback(&segment).await?;
            fetched.push(segment);
        }
    }
    Ok(())
}

/// A playlist, fetched for playback, as text.
async fn text(upstream: &Upstream, url: &str) -> Result<String, String> {
    let fetched = upstream.for_playback(url).await?;
    Ok(String::from_utf8_lossy(&fetched.body).into_owned())
}

/// One attribute of a tag's attribute list, unquoted.
fn attribute(list: &str, name: &str) -> Option<String> {
    let mut rest = list.trim();
    while !rest.is_empty() {
        let (key, after) = rest.split_once('=')?;
        let (value, next) = match after.strip_prefix('"') {
            Some(quoted) => {
                let end = quoted.find('"')?;
                (&quoted[..end], quoted[end + 1..].trim_start_matches(','))
            }
            None => after.split_once(',').unwrap_or((after, "")),
        };
        if key.trim().eq_ignore_ascii_case(name) {
            return Some(value.to_owned());
        }
        rest = next.trim_start();
    }
    None
}

/// The variant mpv opens by default, the highest bandwidth, and the audio group it names.
fn best_variant(body: &str, base: &str) -> Option<(String, Option<String>)> {
    let mut best: Option<(u64, String, Option<String>)> = None;
    let mut pending = None;
    for line in body.lines().map(str::trim) {
        if let Some(list) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let bandwidth =
                attribute(list, "BANDWIDTH").and_then(|b| b.parse().ok()).unwrap_or(0);
            pending = Some((bandwidth, attribute(list, "AUDIO")));
        } else if !line.is_empty()
            && !line.starts_with('#')
            && let Some((bandwidth, audio)) = pending.take()
            && best.as_ref().is_none_or(|(most, ..)| bandwidth > *most)
        {
            best = Some((bandwidth, resolve_url(base, line), audio));
        }
    }
    best.map(|(_, url, audio)| (url, audio))
}

/// The audio rendition of `group` that plays by default, or its first, when it is a playlist
/// of its own.
fn audio_rendition(body: &str, base: &str, group: &str) -> Option<String> {
    let renditions: Vec<(bool, String)> = body
        .lines()
        .filter_map(|line| line.trim().strip_prefix("#EXT-X-MEDIA:"))
        .filter(|list| attribute(list, "TYPE").as_deref() == Some("AUDIO"))
        .filter(|list| attribute(list, "GROUP-ID").as_deref() == Some(group))
        .filter_map(|list| {
            let uri = attribute(list, "URI")?;
            Some((attribute(list, "DEFAULT").as_deref() == Some("YES"), uri))
        })
        .collect();
    let (_, uri) = renditions.iter().find(|(default, _)| *default).or(renditions.first())?;
    Some(resolve_url(base, uri))
}

/// The segments of a media playlist that cover `seconds` from `from`, in order, each after the
/// initialisation segment and key it needs.
fn first_segments(body: &str, base: &str, from: f64, seconds: f64) -> Vec<String> {
    let until = from + seconds;
    let (mut at, mut duration) = (0.0, None);
    let (mut map, mut key) = (None, None);
    let mut out: Vec<String> = Vec::new();
    for line in body.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            duration = rest.split(',').next().and_then(|d| d.trim().parse::<f64>().ok());
        } else if let Some(list) = line.strip_prefix("#EXT-X-MAP:") {
            map = attribute(list, "URI").map(|uri| resolve_url(base, &uri));
        } else if let Some(list) = line.strip_prefix("#EXT-X-KEY:") {
            key = attribute(list, "URI").map(|uri| resolve_url(base, &uri));
        } else if !line.is_empty() && !line.starts_with('#') {
            let end = at + duration.take().unwrap_or(0.0);
            if end > from {
                for needed in [map.take(), key.take()].into_iter().flatten() {
                    if !out.contains(&needed) {
                        out.push(needed);
                    }
                }
                out.push(resolve_url(base, line));
            }
            at = end;
            if at >= until {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::{
        net::Ipv4Addr,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use kaeri_net::HttpClient;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };

    use super::*;

    const MASTER: &str = "#EXTM3U\n\
        #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"one\",URI=\"audio/one.m3u8\"\n\
        #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"two\",DEFAULT=YES,URI=\"audio/two.m3u8\"\n\
        #EXT-X-STREAM-INF:BANDWIDTH=800000,AUDIO=\"a\"\n\
        low/index.m3u8\n\
        #EXT-X-STREAM-INF:BANDWIDTH=4000000,RESOLUTION=1920x1080,AUDIO=\"a\"\n\
        high/index.m3u8\n";

    /// A media playlist of `count` four-second segments.
    fn media(count: usize) -> String {
        let mut body = String::from(
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI=\"init.mp4\"\n\
             #EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n",
        );
        for n in 0..count {
            body.push_str(&format!("#EXTINF:4.0,\nseg{n}.m4s\n"));
        }
        body.push_str("#EXT-X-ENDLIST\n");
        body
    }

    #[test]
    fn the_variant_mpv_opens_is_the_one_warmed_with_its_default_audio() {
        let base = "https://cdn.example/show/master.m3u8";
        let (variant, group) = best_variant(MASTER, base).unwrap();
        assert_eq!(variant, "https://cdn.example/show/high/index.m3u8");
        assert_eq!(
            audio_rendition(MASTER, base, &group.unwrap()).as_deref(),
            Some("https://cdn.example/show/audio/two.m3u8")
        );
    }

    #[test]
    fn about_ten_seconds_from_the_resume_point_are_planned() {
        let base = "https://cdn.example/show/high/index.m3u8";
        let from_start = first_segments(&media(20), base, 0.0, 10.0);
        let names: Vec<_> = from_start.iter().map(|u| u.rsplit('/').next().unwrap()).collect();
        assert_eq!(names, ["init.mp4", "key.bin", "seg0.m4s", "seg1.m4s", "seg2.m4s"]);

        // Resumed at 41 s: the segment holding 41 s and those after it, with the init segment
        // and key still first.
        let resumed = first_segments(&media(20), base, 41.0, 10.0);
        let names: Vec<_> = resumed.iter().map(|u| u.rsplit('/').next().unwrap()).collect();
        assert_eq!(names, ["init.mp4", "key.bin", "seg10.m4s", "seg11.m4s", "seg12.m4s"]);

        // Past the end: nothing.
        assert!(first_segments(&media(3), base, 100.0, 10.0).is_empty());
    }

    #[test]
    fn attributes_are_read_quoted_or_bare() {
        let list = "METHOD=AES-128,URI=\"k,ey.bin\",IV=0x01";
        assert_eq!(attribute(list, "URI").as_deref(), Some("k,ey.bin"));
        assert_eq!(attribute(list, "IV").as_deref(), Some("0x01"));
        assert_eq!(attribute(list, "METHOD").as_deref(), Some("AES-128"));
        assert_eq!(attribute(list, "KEYFORMAT"), None);
    }

    /// A host serving the master, the media playlists and their segments, counting each
    /// request it answers.
    async fn host(asked: Arc<AtomicUsize>) -> String {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let asked = Arc::clone(&asked);
                tokio::spawn(async move {
                    let mut request = [0u8; 4096];
                    let n = socket.read(&mut request).await.unwrap_or(0);
                    let line = String::from_utf8_lossy(&request[..n]).into_owned();
                    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                    asked.fetch_add(1, Ordering::SeqCst);
                    let body = if path.ends_with("master.m3u8") {
                        MASTER.to_owned().into_bytes()
                    } else if path.ends_with(".m3u8") {
                        media(20).into_bytes()
                    } else {
                        vec![7u8; 1024]
                    };
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                });
            }
        });
        format!("http://{address}/show/master.m3u8")
    }

    /// Ask the proxy for `url` as the player would, through its playback entry.
    async fn play(server: &ProxyServer, url: &str) -> String {
        let entry = server.upstream.proxied(url, super::super::Class::Playback);
        let authority =
            entry.trim_start_matches("http://").split('/').next().unwrap().to_owned();
        let path = &entry[entry.find("/m/").unwrap()..];
        let mut socket = TcpStream::connect(&authority).await.unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).await.unwrap();
        String::from_utf8_lossy(&raw).lines().next().unwrap_or_default().to_owned()
    }

    #[tokio::test]
    async fn what_was_warmed_is_served_without_asking_the_host_again_until_released() {
        let asked = Arc::new(AtomicUsize::new(0));
        let origin = host(Arc::clone(&asked)).await;
        let http = HttpClient::new(&kaeri_core::config::NetworkConfig::default()).unwrap();
        let mut server = super::super::serve(http, &origin, Vec::new(), "tok").await.unwrap();

        server.warm(0.0, 10.0).await;
        // Master, variant, audio rendition; then each: init, key and three segments.
        assert_eq!(asked.load(Ordering::SeqCst), 3 + 2 * 5);
        let release = server.release();
        assert_eq!(release.len(), 2 * 5);

        let segment = origin.replace("master.m3u8", "high/seg0.m4s");
        assert!(play(&server, &segment).await.starts_with("HTTP/1.1 200"));
        assert_eq!(asked.load(Ordering::SeqCst), 13, "served from what was warmed");

        // Once the episode shows, the warm-up's segments are let go.
        release.run();
        assert!(play(&server, &segment).await.starts_with("HTTP/1.1 200"));
        assert_eq!(asked.load(Ordering::SeqCst), 14, "fetched again after the release");
    }
}
