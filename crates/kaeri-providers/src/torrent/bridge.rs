//! Torrent playback for the sources that name torrents.
//!
//! Kaeri finds no torrents. A source plugin that declares the `torrents` capability names
//! them, as [`StreamKind::Torrent`] streams carrying a magnet link, and [`TorrentBridge`] plays
//! them through the engine's session: behind the protection check, with the user's trackers added,
//! and served over loopback like any other torrent. The source's own order is kept and nothing is
//! ranked here.
//!
//! The session comes and goes with the settings and the check, so the bridge holds a
//! [`TorrentPlayback`] that the engine fills in once the check passes and empties when it lets
//! the session go. While it is empty, or the check fails, or the network is one torrents may not
//! run on, the source reports itself [`ProviderError::Unavailable`], as the built-in torrent source
//! did: policy, not a failure, so the registry passes it over without contacting it and nothing
//! counts against its health. It is asked again each time, so it comes back with the session.
//!
//! Empty is not always off: with the setting on, the session is on its way at launch and after a
//! settings change, and the hold says so ([`PLAYBACK_STARTING`]) rather than calling it off, as
//! it does when the session could not start ([`PLAYBACK_FAILED`]). A source asked while the
//! session is on its way waits for it briefly: see [`SESSION_WAIT`].

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use kaeri_core::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, Translation},
    stream::{Stream, StreamKind},
    traits::{Provider, ProviderManifest, SourceCandidate},
};

use crate::torrent::session::TorrentSession;

tokio::task_local! {
    /// Set while a resolve should leave torrents as the source named them.
    static NAMED_ONLY: ();
}

/// Run `resolve` with every torrent a source names left as it was named: looked up, not
/// started, so no peer or tracker is contacted. See [`TorrentPlayback::start_first`].
pub async fn named_only<F: std::future::Future>(resolve: F) -> F::Output {
    NAMED_ONLY.scope((), resolve).await
}

/// Whether the resolve under way leaves torrents unstarted.
fn naming_only() -> bool {
    NAMED_ONLY.try_with(|_| ()).is_ok()
}

/// What [`ProviderError::Unavailable`] says while torrent playback is off. The engine words it
/// for the person; this is for the log.
pub const PLAYBACK_OFF: &str = "torrent playback is off";

/// What [`ProviderError::Unavailable`] says while torrent playback is on and its session is still
/// coming up.
pub const PLAYBACK_STARTING: &str = "torrent playback is starting";

/// What [`ProviderError::Unavailable`] says while torrent playback is on and its session could
/// not start.
pub const PLAYBACK_FAILED: &str = "torrent playback could not start";

/// How long a source that names torrents, asked while the session is on its way, waits for it.
/// The session is up within milliseconds of the bring-up, so this covers only a source asked in
/// that moment, and is short because whatever is asked after it waits too.
pub const SESSION_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// The engine's torrent session as sources see it: there while torrent playback is on and the
/// protection check has passed, and gone otherwise.
#[derive(Clone)]
pub struct TorrentPlayback {
    inner: Arc<RwLock<Playback>>,
    /// Where playback stands, watched by a lookup waiting for the session.
    phase: Arc<tokio::sync::watch::Sender<Phase>>,
}

impl Default for TorrentPlayback {
    fn default() -> Self {
        Self {
            inner: Arc::default(),
            phase: Arc::new(tokio::sync::watch::Sender::new(Phase::Off)),
        }
    }
}

/// Where torrent playback stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Switched off: no session is coming.
    Off,
    /// On, and the session is on its way.
    Starting,
    /// On, and waiting for a proxy's check that has failed: it comes up once the check passes,
    /// which may be never.
    Checking,
    /// On, and the session could not start.
    Failed,
    /// The session is up.
    Up,
}

#[derive(Default)]
struct Playback {
    session: Option<Arc<TorrentSession>>,
    /// Added to every torrent played, from `providers.torrent.trackers`.
    trackers: Vec<String>,
}

impl std::fmt::Debug for TorrentPlayback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TorrentPlayback").field("up", &self.session().is_some()).finish()
    }
}

impl TorrentPlayback {
    /// Hand sources the session that has just come up, or take it away. Without one, playback
    /// is off until [`Self::starting`] says a session is on its way.
    pub fn set(&self, session: Option<Arc<TorrentSession>>, trackers: Vec<String>) {
        let phase = if session.is_some() { Phase::Up } else { Phase::Off };
        {
            let mut playback = self.inner.write().unwrap_or_else(|e| e.into_inner());
            playback.session = session;
            playback.trackers = trackers;
        }
        self.phase.send_replace(phase);
    }

    /// Say that torrent playback is on and its session is on its way, as at launch, after a
    /// settings change, or once a proxy's check has passed.
    pub fn starting(&self) {
        self.move_to(Phase::Starting);
    }

    /// Say that the session could not start, and will not until the settings change or the app
    /// starts again.
    pub fn failed(&self) {
        self.move_to(Phase::Failed);
    }

    /// Say that the session waits for a proxy's check that has failed. Sources are held back
    /// for the check, and a lookup does not wait for it.
    pub fn checking(&self) {
        self.move_to(Phase::Checking);
    }

    /// Move to `phase` while no session is up: one that is up stays until [`Self::set`] lets it
    /// go.
    fn move_to(&self, phase: Phase) {
        self.phase.send_if_modified(|now| {
            let moved = *now != Phase::Up && *now != phase;
            if moved {
                *now = phase;
            }
            moved
        });
    }

    /// Whether torrent playback is on and its session is on its way.
    pub fn is_starting(&self) -> bool {
        *self.phase.borrow() == Phase::Starting
    }

    /// Wait for a session on its way, for at most `within`. Returns whether one is up.
    pub async fn up_within(&self, within: std::time::Duration) -> bool {
        let mut phase = self.phase.subscribe();
        let _ = tokio::time::timeout(within, phase.wait_for(|phase| *phase != Phase::Starting))
            .await;
        self.session().is_some()
    }

    /// The session, or why there is none, as a source that names torrents reports it.
    fn up(&self) -> Result<Arc<TorrentSession>, ProviderError> {
        if let Some(session) = self.session() {
            return Ok(session);
        }
        let reason = match *self.phase.borrow() {
            Phase::Starting => PLAYBACK_STARTING,
            Phase::Checking => crate::proxy::CHECK_PENDING,
            Phase::Failed => PLAYBACK_FAILED,
            // Up with no session is a session let go of a moment ago.
            Phase::Off | Phase::Up => PLAYBACK_OFF,
        };
        Err(ProviderError::Unavailable(reason.into()))
    }

    pub fn session(&self) -> Option<Arc<TorrentSession>> {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).session.clone()
    }

    fn trackers(&self) -> Vec<String> {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).trackers.clone()
    }

    /// `link` with the user's trackers it lacks, as the session is to add it.
    pub fn with_trackers(&self, link: &str) -> String {
        with_trackers(link, &self.trackers())
    }

    /// Start one torrent a source named and wrap it as the stream the player gets.
    ///
    /// The magnet, not the loopback URL, is what a download keeps: the loopback address stops
    /// existing the moment the session does.
    pub async fn start(&self, named: &Stream, episode: &str) -> Result<Stream, ProviderError> {
        let session = self.up()?;
        let magnet = self.with_trackers(&named.url);
        // The metadata comes from peers, which can take a minute and a half: the walk asking
        // this source says so, so the screen is not left saying it searches.
        crate::walk::finding_peers();
        let active = session.stream(&magnet, episode.trim().parse().ok()).await?;
        tracing::info!(provider = %named.provider_id, file = %active.file.name, "starting torrent");
        Ok(Stream {
            url: active.url().to_owned(),
            kind: StreamKind::TorrentHttp,
            download_source: Some(magnet),
            // Never `url` for thumbnails: every read of that is a stream librqbit fetches ahead
            // for, and a thumbnailer seeking around would starve playback.
            thumbnail_url: active.thumbnail_url().map(str::to_owned),
            ..named.clone()
        })
    }
}

impl TorrentPlayback {
    /// The streams with the first torrent that starts in place of the torrents named among
    /// them.
    ///
    /// One torrent, not every one: each is a swarm joined. The rest are tried in the source's
    /// order only when the one before fails to start. Streams that are not torrents keep their
    /// places.
    pub async fn start_first(
        &self,
        streams: Vec<Stream>,
        episode: &str,
    ) -> Result<Vec<Stream>, ProviderError> {
        if !streams.iter().any(|s| s.kind == StreamKind::Torrent) {
            return Ok(streams);
        }
        let mut out = Vec::with_capacity(streams.len());
        let mut started = false;
        let mut failure = None;
        for stream in streams {
            if stream.kind != StreamKind::Torrent {
                out.push(stream);
                continue;
            }
            if started {
                continue;
            }
            match self.start(&stream, episode).await {
                Ok(playing) => {
                    started = true;
                    out.push(playing);
                }
                Err(error) => {
                    tracing::info!(provider = %stream.provider_id, %error, "torrent did not start");
                    failure = Some(error);
                }
            }
        }
        match (out.is_empty(), failure) {
            (true, Some(error)) => Err(error),
            _ => Ok(out),
        }
    }

    /// Let go of a torrent started ahead for an episode that never played: its loopback
    /// server, and the torrent with what it fetched unless another stream or a download still
    /// uses it.
    pub async fn drop_started(&self, url: &str) {
        if let Some(session) = self.session() {
            session.drop_stream(url).await;
        }
    }
}

/// Add each tracker `link` does not already carry.
fn with_trackers(link: &str, trackers: &[String]) -> String {
    let mut link = link.to_owned();
    for tracker in trackers.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let encoded = urlencode(tracker);
        if !link.contains(&format!("tr={encoded}")) && !link.contains(&format!("tr={tracker}"))
        {
            link.push_str("&tr=");
            link.push_str(&encoded);
        }
    }
    link
}

fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A source with torrent playback lent: its torrents start through the engine's session, and
/// everything else it answers passes through as it was.
pub struct TorrentBridge {
    inner: Arc<dyn Provider>,
    playback: TorrentPlayback,
}

impl TorrentBridge {
    pub fn new(inner: Arc<dyn Provider>, playback: TorrentPlayback) -> Self {
        Self { inner, playback }
    }

    /// Lend `playback` to `source` when it names torrents; a source that does not is returned as
    /// it is.
    pub fn lend(source: Arc<dyn Provider>, playback: &TorrentPlayback) -> Arc<dyn Provider> {
        if source.manifest().torrents {
            Arc::new(Self::new(source, playback.clone()))
        } else {
            source
        }
    }

    /// The streams with the source's first torrent that starts in place of the torrents it
    /// named, or as named inside [`named_only`]. See [`TorrentPlayback::start_first`].
    async fn started(
        &self,
        streams: Vec<Stream>,
        episode: &str,
    ) -> Result<Vec<Stream>, ProviderError> {
        if naming_only() {
            return Ok(streams);
        }
        self.playback.start_first(streams, episode).await
    }
}

#[async_trait]
impl Provider for TorrentBridge {
    fn manifest(&self) -> &ProviderManifest {
        self.inner.manifest()
    }

    async fn search(
        &self,
        query: &str,
        translation: Translation,
        hints: &kaeri_core::media::TitleHints,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        self.inner.search(query, translation, hints).await
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        translation: Translation,
        hints: &kaeri_core::media::TitleHints,
    ) -> Result<Vec<Episode>, ProviderError> {
        self.inner.episodes(key, translation, hints).await
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
        hints: &kaeri_core::media::TitleHints,
    ) -> Result<Vec<Stream>, ProviderError> {
        let streams = self.inner.resolve(key, episode, translation, hints).await?;
        self.started(streams, episode).await
    }

    async fn sources(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
    ) -> Result<Vec<SourceCandidate>, ProviderError> {
        self.inner.sources(key, episode, translation).await
    }

    async fn resolve_source(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
        source_id: &str,
    ) -> Result<Vec<Stream>, ProviderError> {
        let streams = self.inner.resolve_source(key, episode, translation, source_id).await?;
        self.started(streams, episode).await
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        self.inner.health_check().await
    }

    fn is_available(&self) -> Result<(), ProviderError> {
        self.inner.is_available()?;
        let session = self.playback.up()?;
        session.route().permit()?;
        session.network_permit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockProvider;

    const LINK: &str = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567";

    fn torrent(link: &str) -> Stream {
        Stream { provider_id: "index".into(), ..Stream::new(link, StreamKind::Torrent) }
    }

    fn bridged(streams: Vec<Stream>) -> TorrentBridge {
        let source = MockProvider::new("index").torrents().with_streams(streams).arc();
        TorrentBridge::new(source, TorrentPlayback::default())
    }

    #[test]
    fn the_users_trackers_are_added_once() {
        let trackers = vec!["udp://tracker.example:1337/announce".to_owned(), " ".into()];
        let once = with_trackers(LINK, &trackers);
        assert_eq!(once, format!("{LINK}&tr=udp%3A%2F%2Ftracker.example%3A1337%2Fannounce"));
        assert_eq!(with_trackers(&once, &trackers), once, "not twice");
        assert_eq!(with_trackers(LINK, &[]), LINK);
    }

    #[test]
    fn with_playback_off_a_source_that_names_torrents_is_held_back_as_policy() {
        let err = bridged(vec![torrent(LINK)]).is_available().unwrap_err();
        assert_eq!(err, ProviderError::Unavailable(PLAYBACK_OFF.into()));
        assert!(!err.counts_against_health(), "policy is not a broken source");
    }

    #[tokio::test(start_paused = true)]
    async fn on_and_starting_is_held_as_starting_and_waited_for_within_a_limit() {
        let source = bridged(vec![torrent(LINK)]);
        source.playback.starting();
        assert!(source.playback.is_starting());
        let err = source.is_available().unwrap_err();
        assert_eq!(err, ProviderError::Unavailable(PLAYBACK_STARTING.into()));
        assert!(!err.counts_against_health(), "policy is not a broken source");
        let began = tokio::time::Instant::now();
        assert!(!source.playback.up_within(std::time::Duration::from_secs(5)).await);
        assert!(began.elapsed() >= std::time::Duration::from_secs(5), "waited the whole limit");

        // A failed proxy check holds it for the check, and is not waited for.
        source.playback.checking();
        assert!(!source.playback.is_starting());
        assert_eq!(
            source.is_available(),
            Err(ProviderError::Unavailable(crate::proxy::CHECK_PENDING.into()))
        );
        let began = tokio::time::Instant::now();
        assert!(!source.playback.up_within(std::time::Duration::from_secs(5)).await);
        assert_eq!(began.elapsed(), std::time::Duration::ZERO);

        // A session that could not start is said so.
        source.playback.failed();
        assert_eq!(
            source.is_available(),
            Err(ProviderError::Unavailable(PLAYBACK_FAILED.into()))
        );

        // Let go of, it is off again.
        source.playback.set(None, Vec::new());
        assert_eq!(source.is_available(), Err(ProviderError::Unavailable(PLAYBACK_OFF.into())));
    }

    #[tokio::test]
    async fn a_torrent_never_reaches_the_player_unstarted() {
        // The session went as the call was under way: the torrent is not handed on as it was.
        let streams = bridged(vec![torrent(LINK)])
            .resolve(&ProviderKey::new("k"), "1", Translation::Sub, &Default::default())
            .await;
        assert_eq!(streams, Err(ProviderError::Unavailable(PLAYBACK_OFF.into())));

        let web = Stream {
            provider_id: "index".into(),
            ..Stream::new("https://cdn.example/1.m3u8", StreamKind::Hls)
        };
        let streams = bridged(vec![torrent(LINK), web.clone()])
            .resolve(&ProviderKey::new("k"), "1", Translation::Sub, &Default::default())
            .await
            .unwrap();
        assert_eq!(streams, vec![web], "what is not a torrent keeps its place");
    }

    #[tokio::test]
    async fn looked_up_ahead_a_torrent_is_named_and_not_started() {
        // No session to start it with: a start would fail, and a lookup names it as it was.
        let looked_up = named_only(bridged(vec![torrent(LINK)]).resolve(
            &ProviderKey::new("k"),
            "1",
            Translation::Sub,
            &Default::default(),
        ))
        .await;
        assert_eq!(looked_up, Ok(vec![torrent(LINK)]));

        // Starting it later goes the usual way, behind the session.
        let started = TorrentPlayback::default().start_first(vec![torrent(LINK)], "1").await;
        assert_eq!(started, Err(ProviderError::Unavailable(PLAYBACK_OFF.into())));
    }

    #[test]
    fn only_a_source_that_names_torrents_is_lent_playback() {
        let playback = TorrentPlayback::default();
        let plain: Arc<dyn Provider> = MockProvider::new("plain").arc();
        let lent = TorrentBridge::lend(Arc::clone(&plain), &playback);
        assert!(Arc::ptr_eq(&plain, &lent));
        let named: Arc<dyn Provider> = MockProvider::new("index").torrents().arc();
        let lent = TorrentBridge::lend(Arc::clone(&named), &playback);
        assert!(!Arc::ptr_eq(&named, &lent));
        assert_eq!(lent.manifest().id, "index");
    }
}
