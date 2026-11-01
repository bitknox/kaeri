//! The demo source: a built-in provider for the one work in [`kaeri_core::demo`], played from
//! Wikimedia Commons and nowhere else.
//!
//! It is not a plugin and no outside input shapes it. Every request it makes, and every redirect
//! one is sent on, is checked against the two hosts in [`DEMO_HOSTS`] before it is followed, and
//! every address it hands a player is checked the same way. Before a file is handed over, Commons
//! is asked for the file's SHA-1, and a file that differs from the pin is not played under the
//! demo's credit.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use kaeri_core::{
    demo::{
        self, DEMO_HOSTS, DEMO_SOURCE_ID, DEMO_SOURCE_NAME, DEMO_TITLE, DEMO_TITLE_ROMAJI,
        DEMO_USER_AGENT, DemoPart,
    },
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, MediaFormat, SearchHit, Translation},
    stream::{Stream, StreamKind, Subtitle},
    traits::{Provider, ProviderKind, ProviderManifest},
};

pub use kaeri_core::demo::{DEMO_SOURCE_ID as SOURCE_ID, reserved};

/// The key the catalogue's one title goes by within the demo.
const TITLE_KEY: &str = "koushisu";

/// Where the demo's two hosts are. Commons' own, except in a test that stands a fake in their
/// place: see [`DemoProvider::with_endpoints`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct Endpoints {
    upload: String,
    commons: String,
}

impl Endpoints {
    fn wikimedia() -> Self {
        Self {
            upload: "https://upload.wikimedia.org".into(),
            commons: "https://commons.wikimedia.org".into(),
        }
    }

    /// `url`, a catalogue address, where this endpoint set serves it.
    fn rebase(&self, url: &str) -> String {
        for (host, base) in [(DEMO_HOSTS[0], &self.upload), (DEMO_HOSTS[1], &self.commons)] {
            if let Some(rest) = url.strip_prefix(&format!("https://{host}")) {
                return format!("{}{rest}", base.trim_end_matches('/'));
            }
        }
        url.to_owned()
    }

    /// Whether `url` is on one of the two hosts these endpoints stand for.
    fn allows(&self, url: &str) -> bool {
        if *self == Self::wikimedia() {
            return demo::host_allowed(url);
        }
        [&self.upload, &self.commons].iter().any(|base| {
            url.strip_prefix(base.trim_end_matches('/'))
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '?']))
        })
    }
}

pub struct DemoProvider {
    manifest: ProviderManifest,
    client: reqwest::Client,
    endpoints: Endpoints,
}

impl DemoProvider {
    /// The demo as it ships: Commons' two hosts.
    pub fn new() -> Self {
        Self::build(Endpoints::wikimedia())
    }

    /// The demo with its two hosts standing at `upload` and `commons`, for a test that serves its
    /// own synthetic media. Only with the `testing` feature, which no release build turns on:
    /// without it nothing can repoint the demo.
    #[cfg(feature = "testing")]
    pub fn with_endpoints(upload: impl Into<String>, commons: impl Into<String>) -> Self {
        Self::build(Endpoints { upload: upload.into(), commons: commons.into() })
    }

    fn build(endpoints: Endpoints) -> Self {
        let guard = endpoints.clone();
        let client = kaeri_net::trusted_roots(reqwest::Client::builder())
            .user_agent(DEMO_USER_AGENT)
            .timeout(Duration::from_secs(20))
            // Decided before the redirect is followed, not checked after it: a request that
            // leaves the two hosts never goes out.
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 5 {
                    attempt.error("too many redirects")
                } else if guard.allows(attempt.url().as_str()) {
                    attempt.follow()
                } else {
                    attempt.error("the demo only reaches Wikimedia Commons")
                }
            }))
            .build()
            .expect("a plain client builds");
        Self {
            manifest: ProviderManifest {
                id: DEMO_SOURCE_ID.into(),
                display_name: DEMO_SOURCE_NAME.into(),
                version: env!("CARGO_PKG_VERSION").into(),
                kind: ProviderKind::Demo,
                allowed_hosts: DEMO_HOSTS.iter().map(|h| (*h).to_owned()).collect(),
                translations: vec![Translation::Sub],
                torrents: false,
            },
            client,
            endpoints,
        }
    }

    pub fn shared() -> Arc<dyn Provider> {
        Arc::new(Self::new())
    }

    /// Ask for `url` through the guarded client. Refused before any request when it is not on the
    /// demo's hosts.
    async fn get(&self, url: &str) -> Result<String, ProviderError> {
        let url = self.endpoints.rebase(url);
        if !self.endpoints.allows(&url) {
            return Err(ProviderError::Blocked("not a host the demo may reach".into()));
        }
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        if !response.status().is_success() {
            return Err(ProviderError::Blocked(format!("HTTP {}", response.status().as_u16())));
        }
        response.text().await.map_err(|e| ProviderError::Transport(e.to_string()))
    }

    /// Commons' SHA-1 for the part's file, from its API.
    async fn current_sha1(&self, part: &DemoPart) -> Result<String, ProviderError> {
        let body = self.get(part.hash_url).await?;
        let reply: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| ProviderError::Parse(e.to_string()))?;
        reply["query"]["pages"]
            .as_object()
            .and_then(|pages| pages.values().next())
            .and_then(|page| page["imageinfo"][0]["sha1"].as_str())
            .map(str::to_owned)
            .ok_or_else(|| ProviderError::Parse("no hash in Commons' answer".into()))
    }

    fn stream_of(&self, part: &DemoPart) -> Result<Stream, ProviderError> {
        let url = self.endpoints.rebase(part.file_url);
        let mut subtitles = Vec::new();
        for (language, track) in [("en", part.subtitles_en), ("ja", part.subtitles_ja)] {
            let Some(track) = track else { continue };
            let track = self.endpoints.rebase(track);
            if !self.endpoints.allows(&track) {
                return Err(ProviderError::Blocked("a subtitle off the demo's hosts".into()));
            }
            subtitles.push(Subtitle {
                language: language.into(),
                url: track,
                hard: false,
                format: Some("srt".into()),
            });
        }
        if !self.endpoints.allows(&url) {
            return Err(ProviderError::Blocked("a file off the demo's hosts".into()));
        }
        Ok(Stream {
            quality: Some(part.quality),
            headers: vec![("User-Agent".into(), DEMO_USER_AGENT.into())],
            subtitles,
            provider_id: DEMO_SOURCE_ID.into(),
            // The demo is streamed from Commons and never kept: there is nothing to download
            // from here.
            download_source: None,
            ..Stream::new(url, StreamKind::Mp4)
        })
    }
}

impl Default for DemoProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether `query` is one the demo's title answers to: either spelling of the title or the
/// source's own name, in any case.
pub fn answers_to(query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return false;
    }
    [DEMO_TITLE, DEMO_TITLE_ROMAJI, DEMO_SOURCE_NAME, TITLE_KEY]
        .iter()
        .any(|name| name.to_lowercase().contains(&query))
}

#[async_trait]
impl Provider for DemoProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    /// The demo's one title, with no network: Search's Available filter keeps it as Plays here.
    fn titles(&self) -> Vec<kaeri_core::ids::AnilistId> {
        vec![demo::DEMO_ANILIST_ID]
    }

    async fn search(
        &self,
        query: &str,
        _translation: Translation,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        // The title matcher asks with the title as the metadata names it, which for the demo is
        // its own: either spelling finds it.
        if !answers_to(query) {
            return Ok(Vec::new());
        }
        Ok(vec![SearchHit {
            synonyms: vec![DEMO_TITLE_ROMAJI.into()],
            episode_count: Some(demo::PARTS.len() as u32),
            format: Some(MediaFormat::Tv),
            ..SearchHit::new(ProviderKey::new(TITLE_KEY), DEMO_TITLE)
        }])
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        _translation: Translation,
    ) -> Result<Vec<Episode>, ProviderError> {
        if key.as_str() != TITLE_KEY {
            return Err(ProviderError::NotFound);
        }
        Ok(demo::PARTS
            .iter()
            .map(|part| {
                Episode::new(part.number)
                    .with_title(part.title)
                    .with_duration(Duration::from_secs(u64::from(part.duration_secs)))
                    .with_thumbnail(self.endpoints.rebase(part.still_url))
            })
            .collect())
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        _translation: Translation,
    ) -> Result<Vec<Stream>, ProviderError> {
        if key.as_str() != TITLE_KEY {
            return Err(ProviderError::NotFound);
        }
        let part = demo::part(episode).ok_or(ProviderError::NotFound)?;
        let current = self.current_sha1(part).await?;
        if !current.eq_ignore_ascii_case(part.sha1) {
            tracing::warn!(
                part = part.number,
                "the demo's file on Commons is not the one pinned"
            );
            return Err(ProviderError::Blocked(
                "the file on Commons is not the one this version pins".into(),
            ));
        }
        Ok(vec![self.stream_of(part)?])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn either_spelling_of_the_title_finds_the_work() {
        let demo = DemoProvider::new();
        for query in [DEMO_TITLE, DEMO_TITLE_ROMAJI, "demo", "KOUSHI"] {
            let hits = demo.search(query, Translation::Sub).await.unwrap();
            assert_eq!(hits.len(), 1, "{query}");
        }
        assert!(demo.search("something else", Translation::Sub).await.unwrap().is_empty());
        assert!(demo.search("  ", Translation::Sub).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn every_part_is_an_episode_on_the_demo_hosts() {
        let demo = DemoProvider::new();
        let episodes =
            demo.episodes(&ProviderKey::new(TITLE_KEY), Translation::Sub).await.unwrap();
        assert_eq!(episodes.len(), demo::PARTS.len());
        assert!(demo.episodes(&ProviderKey::new("other"), Translation::Sub).await.is_err());
    }

    #[test]
    fn streams_carry_only_demo_hosts_and_credit_their_subtitles() {
        let demo = DemoProvider::new();
        for part in &demo::PARTS {
            let stream = demo.stream_of(part).unwrap();
            assert!(demo::host_allowed(&stream.url));
            assert!(stream.subtitles.iter().all(|s| demo::host_allowed(&s.url)));
            assert_eq!(stream.provider_id, DEMO_SOURCE_ID);
            assert!(stream.download_source.is_none());
        }
    }

    #[test]
    fn the_manifest_allows_exactly_the_two_hosts() {
        let demo = DemoProvider::new();
        assert_eq!(demo.manifest().allowed_hosts, DEMO_HOSTS.map(String::from));
    }

    /// A release build has no way to repoint the demo: `with_endpoints` does not exist without
    /// the `testing` feature, so the shipped provider is always Commons' two hosts.
    #[cfg(not(feature = "testing"))]
    #[test]
    fn a_build_without_testing_always_reaches_commons() {
        let demo = DemoProvider::new();
        assert_eq!(demo.endpoints, Endpoints::wikimedia());
        for part in &demo::PARTS {
            assert!(
                demo.stream_of(part).unwrap().url.starts_with("https://upload.wikimedia.org/")
            );
        }
    }

    /// A fake Commons: one local listener answering every request with `reply`, so a test plays
    /// the demo without the network. Returns its base address.
    #[cfg(feature = "testing")]
    async fn fake_commons(reply: String) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else { return };
                let reply = reply.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 2048];
                    let _ = socket.read(&mut buf).await;
                    let _ = socket.write_all(reply.as_bytes()).await;
                });
            }
        });
        base
    }

    #[cfg(feature = "testing")]
    fn ok_with_hash(sha1: &str) -> String {
        let body =
            format!(r#"{{"query":{{"pages":{{"1":{{"imageinfo":[{{"sha1":"{sha1}"}}]}}}}}}}}"#);
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[cfg(feature = "testing")]
    #[tokio::test]
    async fn the_pinned_file_resolves_against_a_fake_commons() {
        let part = &demo::PARTS[0];
        let base = fake_commons(ok_with_hash(part.sha1)).await;
        let demo = DemoProvider::with_endpoints(base.clone(), base.clone());
        let streams = demo
            .resolve(&ProviderKey::new(TITLE_KEY), part.number, Translation::Sub)
            .await
            .unwrap();
        assert_eq!(streams.len(), 1);
        assert!(streams[0].url.starts_with(&base), "{}", streams[0].url);
    }

    #[cfg(feature = "testing")]
    #[tokio::test]
    async fn a_file_that_is_not_the_pinned_one_plays_nothing() {
        let part = &demo::PARTS[0];
        let base = fake_commons(ok_with_hash("0000000000000000000000000000000000000000")).await;
        let demo = DemoProvider::with_endpoints(base.clone(), base);
        let err =
            demo.resolve(&ProviderKey::new(TITLE_KEY), part.number, Translation::Sub).await;
        assert!(matches!(err, Err(ProviderError::Blocked(_))), "{err:?}");
    }

    #[cfg(feature = "testing")]
    #[tokio::test]
    async fn a_redirect_off_the_demo_hosts_is_refused_before_it_is_followed() {
        let part = &demo::PARTS[0];
        let redirect =
            "HTTP/1.1 302 Found\r\nlocation: https://elsewhere.example/x\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                .to_owned();
        let base = fake_commons(redirect).await;
        let demo = DemoProvider::with_endpoints(base.clone(), base);
        let err =
            demo.resolve(&ProviderKey::new(TITLE_KEY), part.number, Translation::Sub).await;
        assert!(matches!(err, Err(ProviderError::Transport(_))), "{err:?}");
    }
}
