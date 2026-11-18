//! The next episode, made ready while the current one's ending plays, for the play that
//! follows to take rather than do again.
//!
//! Always, while Auto-play next is on and Up next was not cancelled: its streams found, the
//! first stream's subtitles fetched and its skip times asked for. On a network that is not
//! limited, or on a limited one when `playback.preload_metered` says so, its first seconds as
//! well: an HLS stream's playlists and first segments held by a live proxy the playback adopts,
//! a torrent's first pieces fetched by the session that will serve them. Offline, nothing.
//!
//! Cancel on Up next, Auto-play next turned off, the player closed or the entry growing stale
//! drops it, and with it the proxy and any torrent started for it, pieces and all. Nothing is
//! made ready at launch: only a playback's ending asks for it.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use kaeri_core::{
    config::Config,
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind},
};
use kaeri_net::HttpClient;
use kaeri_providers::torrent::bridge::{TorrentPlayback, named_only};

use crate::{
    phases::Network,
    playback::{
        PlaybackContext,
        link::PlayerTarget,
        proxy::{ProxyServer, Release},
        resolve::{Resolved, resolve_for_playback},
    },
    services::Services,
    state::EngineState,
};

/// How long what was made ready stays usable. Long enough to cover a viewer who lets the ending
/// and a post-credits scene play out; short enough that a signed stream URL is still good.
const PREFETCH_FRESH: Duration = Duration::from_secs(15 * 60);

/// About how much of the next episode's video is fetched ahead, in seconds of playback.
pub const WARM_SECONDS: f64 = 10.0;

/// The most a warm-up spends fetching video before it settles for what it has.
const WARM_LIMIT: Duration = Duration::from_secs(120);

/// How long the next episode's subtitles may take: nobody waits on them yet.
const SUBTITLES_AHEAD_WAIT: Duration = Duration::from_secs(30);

/// How long after the episode shows its first frame the segments fetched ahead are let go:
/// the player has read them into its own cache by then.
const RELEASE_AFTER: Duration = Duration::from_secs(10);

/// How long a roll-on counts as the hand-over to the play it started. The play is staged at
/// once, so anything older is not that play's.
const HANDOVER_FRESH: Duration = Duration::from_secs(60);

/// The fewest and most bytes of a torrent fetched ahead.
const TORRENT_AHEAD_MIN: u64 = 2 * 1024 * 1024;
const TORRENT_AHEAD_MAX: u64 = 64 * 1024 * 1024;

/// How far the next episode is made ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// The streams, the first stream's subtitles and the skip times. A torrent is looked up
    /// and not started: no peer or tracker is contacted.
    Light,
    /// All of that and about [`WARM_SECONDS`] of video.
    Full,
}

/// How far the next episode is made ready on `network`, as the front end last said it, with
/// `preload_metered` the setting: nothing offline, all of it on a network that is not limited,
/// and on a limited one (cellular, Low Data Mode, or one not said yet) only what the setting
/// allows.
pub fn depth(network: Option<Network>, preload_metered: bool) -> Option<Depth> {
    match network {
        Some(network) if network.offline => None,
        Some(network) if !network.cellular && !network.constrained => Some(Depth::Full),
        _ if preload_metered => Some(Depth::Full),
        _ => Some(Depth::Light),
    }
}

/// The next episode as it was made ready.
pub struct Warm {
    pub resolved: Resolved,
    /// The proxy fronting the first stream, its first seconds fetched, for the first attempt.
    pub proxy: Option<ProxyServer>,
    /// What lets go of the torrents started for it, while nothing plays them.
    torrents: Vec<Held>,
}

impl Warm {
    /// Streams found with nothing more made ready.
    pub fn found(resolved: Resolved) -> Self {
        Self { resolved, proxy: None, torrents: Vec::new() }
    }

    /// Taken by the playback it was for: from here the playback owns what was started.
    pub fn into_parts(mut self) -> (Resolved, Option<ProxyServer>) {
        for held in &mut self.torrents {
            held.0 = None;
        }
        (self.resolved, self.proxy)
    }

    /// The same without its torrents, for a play that may not use them: what was started for
    /// them is let go. `None` when nothing else was found.
    pub fn without_torrents(mut self) -> Option<Self> {
        self.torrents.clear();
        self.resolved
            .0
            .retain(|s| !matches!(s.kind, StreamKind::Torrent | StreamKind::TorrentHttp));
        (!self.resolved.0.is_empty()).then_some(self)
    }

    /// Whether a torrent among its streams was only looked up, and is still to start.
    pub fn names_torrents(&self) -> bool {
        self.resolved.0.iter().any(|s| s.kind == StreamKind::Torrent)
    }
}

/// What lets go of something started ahead when dropped, unless it was taken.
type LetGo = Box<dyn FnOnce() + Send>;

/// Lets go of something started ahead when dropped, unless it was taken.
struct Held(Option<LetGo>);

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(let_go) = self.0.take() {
            let_go();
        }
    }
}

impl Held {
    /// The torrent served at `url`, let go of through `torrents`.
    fn torrent(torrents: &TorrentPlayback, url: &str) -> Self {
        let (torrents, url) = (torrents.clone(), url.to_owned());
        Self(Some(Box::new(move || {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move { torrents.drop_started(&url).await });
            }
        })))
    }
}

/// The next episode as it was made ready, held for the play that follows.
///
/// One entry: only the episode about to play is worth holding, and a newer warm-up replaces
/// an older one. Taken, not copied, by the playback it was for.
#[derive(Clone, Default)]
pub struct Prefetch(Arc<Mutex<Inner>>);

#[derive(Default)]
struct Inner {
    entry: Option<Prefetched>,
    /// Moved on by every warm-up begun and every drop: a warm-up that finishes after either
    /// is let go rather than kept.
    generation: u64,
    /// The episode a warm-up under way is for.
    pending: Option<(AnilistId, String, Translation)>,
    /// The episode a roll-on just handed over to, and when.
    handover: Option<(AnilistId, String, Instant)>,
}

struct Prefetched {
    id: AnilistId,
    episode: String,
    translation: Translation,
    found: Instant,
    warm: Warm,
}

impl Prefetch {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Drop what was made ready, and any warm-up under way: it was for sources that have since
    /// changed, an Up next that was cancelled, or a player that closed.
    pub fn clear(&self) {
        let dropped = {
            let mut inner = self.lock();
            inner.generation += 1;
            inner.pending = None;
            inner.entry.take()
        };
        if dropped.is_some() {
            tracing::info!("what was made ready for the next episode was let go");
        }
    }

    /// Keep streams found for `episode`, with nothing more made ready.
    pub fn keep(
        &self,
        id: AnilistId,
        episode: &str,
        translation: Translation,
        resolved: Resolved,
    ) {
        let generation = self.begin(id, episode, translation);
        self.finish(generation, id, episode, translation, Warm::found(resolved));
    }

    /// A warm-up for `episode` begins, in place of anything held or under way. Returns its
    /// generation, for [`Self::finish`].
    fn begin(&self, id: AnilistId, episode: &str, translation: Translation) -> u64 {
        let (generation, dropped) = {
            let mut inner = self.lock();
            inner.generation += 1;
            inner.pending = Some((id, episode.to_owned(), translation));
            (inner.generation, inner.entry.take())
        };
        drop(dropped);
        generation
    }

    /// The warm-up of `generation` is done: kept while nothing replaced or dropped it, and let
    /// go otherwise.
    fn finish(
        &self,
        generation: u64,
        id: AnilistId,
        episode: &str,
        translation: Translation,
        warm: Warm,
    ) {
        let late = {
            let mut inner = self.lock();
            if inner.generation != generation {
                Some(warm)
            } else {
                tracing::info!(%episode, streams = warm.resolved.0.len(), "next episode made ready");
                inner.pending = None;
                let entry = Prefetched {
                    id,
                    episode: episode.to_owned(),
                    translation,
                    found: Instant::now(),
                    warm,
                };
                inner.entry.replace(entry).map(|old| old.warm)
            }
        };
        drop(late);
        self.expire(generation);
    }

    /// Drop the entry of `generation` once it is stale, if it is still held then.
    fn expire(&self, generation: u64) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else { return };
        let prefetch = self.clone();
        runtime.spawn(async move {
            tokio::time::sleep(PREFETCH_FRESH).await;
            let stale = {
                let mut inner = prefetch.lock();
                if inner.generation == generation { inner.entry.take() } else { None }
            };
            if stale.is_some() {
                tracing::info!("what was made ready for the next episode went stale");
            }
        });
    }

    /// What was made ready for exactly this episode, translation and title, if still fresh.
    /// A warm-up still under way for it is let go: the play is starting without it.
    pub fn take(&self, id: AnilistId, episode: &str, translation: Translation) -> Option<Warm> {
        let (taken, stale) = {
            let mut inner = self.lock();
            let wanted = |(i, e, t): &(AnilistId, String, Translation)| {
                *i == id && e == episode && *t == translation
            };
            if inner.pending.as_ref().is_some_and(wanted) {
                inner.pending = None;
                inner.generation += 1;
            }
            // Only a match is taken. Any other play (a show opened in passing, a source picked
            // for a different episode) used to take the entry with it, so the streams found
            // for the episode about to roll on were gone by the time it did.
            match inner.entry.as_ref() {
                None => (None, None),
                Some(entry) if entry.found.elapsed() >= PREFETCH_FRESH => {
                    (None, inner.entry.take())
                }
                Some(entry)
                    if entry.id == id
                        && entry.episode == episode
                        && entry.translation == translation =>
                {
                    (inner.entry.take(), None)
                }
                Some(_) => (None, None),
            }
        };
        drop(stale);
        taken.map(|entry| entry.warm)
    }

    /// A roll-on handed over to `episode` of `id`: the play it starts measures from now.
    pub fn handed_over(&self, id: AnilistId, episode: &str) {
        self.lock().handover = Some((id, episode.to_owned(), Instant::now()));
    }

    /// Whether a warm-up is under way for, or holds, `episode` of `id`: the episode Up next
    /// names.
    pub fn is_next(&self, id: AnilistId, episode: &str) -> bool {
        let inner = self.lock();
        inner.pending.as_ref().is_some_and(|(i, e, _)| *i == id && e == episode)
            || inner
                .entry
                .as_ref()
                .is_some_and(|entry| entry.id == id && entry.episode == episode)
    }

    /// When a roll-on handed over to `episode` of `id`, if one just did.
    pub fn handover_for(&self, id: AnilistId, episode: &str) -> Option<Instant> {
        let (i, e, at) = self.lock().handover.take()?;
        (i == id && e == episode && at.elapsed() < HANDOVER_FRESH).then_some(at)
    }
}

/// What a playback takes from the warm-up for its episode, and how it says how long the
/// episode it rolled on to took to show.
#[derive(Clone, Default)]
pub struct Ahead {
    /// When the roll-on handed over to this playback, until its first frame is said.
    rolled_at: Arc<Mutex<Option<Instant>>>,
    /// The proxy started ahead for the first stream, until the first attempt adopts it.
    proxy: Arc<Mutex<Option<ProxyServer>>>,
    /// The segments fetched ahead, to let go once the episode shows.
    release: Arc<Mutex<Option<Release>>>,
    /// Where Cancel on Up next drops what is being made ready for the episode after.
    prefetch: Prefetch,
}

impl std::fmt::Debug for Ahead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ahead").finish_non_exhaustive()
    }
}

impl Ahead {
    pub fn new(
        rolled_at: Option<Instant>,
        proxy: Option<ProxyServer>,
        prefetch: Prefetch,
    ) -> Self {
        Self {
            rolled_at: Arc::new(Mutex::new(rolled_at)),
            proxy: Arc::new(Mutex::new(proxy)),
            release: Arc::default(),
            prefetch,
        }
    }

    /// The proxy started ahead for `url`, for the attempt that plays it.
    pub fn adopt(&self, url: &str) -> Option<ProxyServer> {
        let mut proxy = self.proxy.lock().unwrap_or_else(PoisonError::into_inner);
        if proxy.as_ref().is_none_or(|server| server.origin() != url) {
            return None;
        }
        let server = proxy.take()?;
        *self.release.lock().unwrap_or_else(PoisonError::into_inner) = Some(server.release());
        tracing::info!("playing through the proxy started ahead");
        Some(server)
    }

    /// The episode showed its first frame: how long that took from the roll-on is logged, and
    /// the segments fetched ahead are let go shortly after.
    pub fn landed(&self) {
        let rolled_at = self.rolled_at.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(at) = rolled_at {
            let next_episode_ms = at.elapsed().as_millis() as u64;
            tracing::info!(next_episode_ms, "the next episode showed its first frame");
        }
        let release = self.release.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(release) = release.filter(|r| !r.is_empty())
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(async move {
                tokio::time::sleep(RELEASE_AFTER).await;
                tracing::debug!(segments = release.len(), "segments fetched ahead let go");
                release.run();
            });
        }
    }

    /// Up next was cancelled: nothing rolls on, so nothing is kept ready for it.
    pub fn cancelled(&self) {
        self.prefetch.clear();
    }
}

/// Make the next episode ready while this one's ending plays, for the play that follows to
/// take. Asked for by the playback, never at launch.
pub fn prefetch(id: AnilistId, episode: String, engine: &EngineState, services: &Services) {
    let effective = engine.config.effective();
    // Turned off since the episode began: nothing will roll on.
    if !effective.playback.auto_next {
        return;
    }
    let Some(depth) = depth(engine.said_network(), effective.playback.preload_metered) else {
        tracing::info!("offline; the next episode is not made ready");
        return;
    };
    let Services { meta, store, registry, prefetch, http, .. } = services;
    // As the show has it: the streams found are for the translation and quality it plays in.
    let config = crate::show_settings::for_show(effective, store, id);
    let opener = crate::playback::start::opener_for(engine, services);
    let casting = opener.as_ref().is_some_and(|o| matches!(o.target(), PlayerTarget::Cast(_)));
    // A cast that passes torrents over starts none ahead either.
    let registry = if casting && !kaeri_model::settings::while_casting(&config) {
        registry.without_torrents()
    } else {
        registry.clone()
    };
    let scratch = opener.as_ref().map(|o| o.scratch_dir().join(ahead_dir(&episode)));
    let generation = prefetch.begin(id, &episode, config.playback.translation);
    tracing::info!(%episode, ?depth, "making the next episode ready");
    let torrents = services.registry.torrents().clone();
    let (meta, store, http, prefetch) =
        (meta.clone(), store.clone(), http.clone(), prefetch.clone());
    tokio::spawn(async move {
        let playback = config.playback.clone();
        // Quiet: the steps of a search nobody is waiting on must not take the status
        // line from the episode playing. Nothing reads this channel.
        let resolving = resolve_for_playback(
            &meta, &store, &registry, id, &episode, None, &playback, None, None,
        );
        let found = match depth {
            Depth::Light => named_only(resolving).await,
            Depth::Full => resolving.await,
        };
        let resolved = match found {
            Ok(resolved) => resolved,
            Err(reason) => {
                tracing::info!(%reason, "could not find the next episode ahead");
                return;
            }
        };
        let readying = Readying { http: &http, torrents: &torrents, config: &config, casting };
        let warm = readying.warm(resolved, depth, scratch).await;
        prefetch.finish(generation, id, &episode, playback.translation, warm);
    });
}

/// The folder the next episode's subtitles are fetched to, apart from the playing episode's.
fn ahead_dir(episode: &str) -> String {
    let safe: String =
        episode.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    format!("ahead-{safe}")
}

/// What a warm-up works with.
struct Readying<'a> {
    http: &'a HttpClient,
    torrents: &'a TorrentPlayback,
    config: &'a Config,
    casting: bool,
}

impl Readying<'_> {
    /// Everything `depth` asks for, from the streams found. Whatever fails is left as it was,
    /// and the play does it as it would have.
    async fn warm(&self, resolved: Resolved, depth: Depth, scratch: Option<PathBuf>) -> Warm {
        let mut warm = Warm::found(resolved);
        // Whatever torrent the resolve started is let go if nothing plays it.
        warm.torrents = warm
            .resolved
            .0
            .iter()
            .filter(|s| s.kind == StreamKind::TorrentHttp)
            .map(|s| Held::torrent(self.torrents, &s.url))
            .collect();
        let (streams, context) = &mut warm.resolved;
        self.skip_times(context).await;
        if let (Some(scratch), Some(first)) = (scratch, streams.first_mut()) {
            crate::playback::session::localise_ahead(
                self.http,
                &scratch,
                first,
                SUBTITLES_AHEAD_WAIT,
            )
            .await;
        }
        if depth == Depth::Light {
            return warm;
        }
        let Some(first) = streams.first().cloned() else { return warm };
        let from = context.resume_at.filter(|at| *at > 0.0).unwrap_or(0.0);
        match first.kind {
            // A TV reads the stream itself, never through a proxy here.
            StreamKind::Hls if !self.casting => {
                warm.proxy = self.hls(&first, context, from).await;
            }
            StreamKind::TorrentHttp => {
                let fetched =
                    tokio::time::timeout(WARM_LIMIT, torrent_start(&first.url, context));
                match fetched.await {
                    Ok(Ok(bytes)) => {
                        tracing::info!(bytes, "the next episode's first pieces fetched")
                    }
                    Ok(Err(reason)) => {
                        tracing::info!(%reason, "the next episode's first pieces were not fetched")
                    }
                    Err(_) => tracing::info!("the next episode's first pieces took too long"),
                }
            }
            // A direct file is not served through the proxy, which keeps whole resources and
            // not ranges: its streams and subtitles are made ready, and no video.
            _ => {}
        }
        warm
    }

    /// The next episode's skip times, asked for now so the play finds them kept.
    async fn skip_times(&self, context: &PlaybackContext) {
        let settings =
            crate::playback::start::play_settings(self.config, Vec::new(), Default::default());
        crate::playback::SkipTimes::shared()
            .for_playback(
                self.http,
                &settings,
                context.mal_id,
                &context.episode,
                context.runtime_secs,
            )
            .await;
    }

    /// A proxy for `stream` with its first seconds from `from` fetched, or `None` when one
    /// could not start.
    async fn hls(
        &self,
        stream: &Stream,
        context: &PlaybackContext,
        from: f64,
    ) -> Option<ProxyServer> {
        let token = crate::playback::session::proxy_token(context, 0);
        let headers = stream.headers.clone();
        let served =
            crate::playback::proxy::serve(self.http.clone(), &stream.url, headers, &token);
        let mut server = match served.await {
            Ok(server) => server,
            Err(error) => {
                tracing::info!(%error, "no proxy started ahead");
                return None;
            }
        };
        if tokio::time::timeout(WARM_LIMIT, server.warm(from, WARM_SECONDS)).await.is_err() {
            tracing::info!("the next episode's first seconds took too long");
        }
        Some(server)
    }
}

/// Read the first pieces the player will ask a torrent's loopback server for: the start of the
/// file, and the part at the resume point when there is one. Returns the bytes read.
async fn torrent_start(url: &str, context: &PlaybackContext) -> Result<u64, String> {
    use reqwest::header::{CONTENT_RANGE, RANGE};
    // Loopback: never through a proxy the app's own client may be set to use.
    let client = reqwest::Client::builder().no_proxy().build().map_err(|e| e.to_string())?;
    let probe =
        client.get(url).header(RANGE, "bytes=0-0").send().await.map_err(|e| e.to_string())?;
    let total = probe
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit('/').next())
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or("the file's length was not said")?;
    drop(probe);
    let (starts, span) = torrent_spans(total, context);
    let mut read = 0;
    for start in starts {
        let range = format!("bytes={start}-{}", start + span - 1);
        let mut response =
            client.get(url).header(RANGE, range).send().await.map_err(|e| e.to_string())?;
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            read += chunk.len() as u64;
        }
    }
    Ok(read)
}

/// Where the reads ahead start in a file of `total` bytes, and how long each is: about
/// [`WARM_SECONDS`] of it by the episode's runtime.
fn torrent_spans(total: u64, context: &PlaybackContext) -> (Vec<u64>, u64) {
    let runtime = f64::from(context.runtime_secs.filter(|r| *r > 0).unwrap_or(24 * 60));
    let span = ((total as f64) * WARM_SECONDS / runtime) as u64;
    let span = span.clamp(TORRENT_AHEAD_MIN, TORRENT_AHEAD_MAX).min(total).max(1);
    let mut starts = vec![0];
    if let Some(at) = context.resume_at.filter(|at| *at > 0.0) {
        let start = (((total as f64) * at / runtime) as u64).min(total.saturating_sub(span));
        if start >= span {
            starts.push(start);
        }
    }
    (starts, span)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::playback;

    fn context(episode: &str) -> playback::PlaybackContext {
        playback::PlaybackContext {
            anilist_id: AnilistId::new(7),
            mal_id: None,
            episode: episode.into(),
            title: "t".into(),
            translation: Translation::Sub,
            resume_at: None,
            runtime_secs: None,
            speed: None,
            volume: None,
            next: None,
            episodes: Vec::new(),
            cover_url: None,
            banner_url: None,
            episode_total: None,
        }
    }

    fn found() -> Resolved {
        (Vec::new(), context("6"))
    }

    /// A warm-up holding something started ahead, and the flag its letting go sets.
    fn started_ahead() -> (Warm, Arc<AtomicBool>) {
        let let_go = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&let_go);
        let mut warm = Warm::found(found());
        warm.torrents.push(Held(Some(Box::new(move || flag.store(true, Ordering::SeqCst)))));
        (warm, let_go)
    }

    /// An entry for episode 6 holding something started ahead, and the flag its letting go
    /// sets.
    fn holding(prefetch: &Prefetch) -> Arc<AtomicBool> {
        let (warm, let_go) = started_ahead();
        let generation = prefetch.begin(AnilistId::new(7), "6", Translation::Sub);
        prefetch.finish(generation, AnilistId::new(7), "6", Translation::Sub, warm);
        let_go
    }

    #[test]
    fn how_far_the_next_episode_is_made_ready_follows_the_network_and_the_setting() {
        let wifi = Network::default();
        let cellular = Network { cellular: true, ..wifi };
        let low_data = Network { constrained: true, ..wifi };
        let offline = Network { offline: true, ..wifi };

        assert_eq!(depth(Some(wifi), false), Some(Depth::Full));
        assert_eq!(depth(Some(cellular), false), Some(Depth::Light));
        assert_eq!(depth(Some(low_data), false), Some(Depth::Light));
        // Not said yet counts as limited, as everything else that waits for Wi-Fi does.
        assert_eq!(depth(None, false), Some(Depth::Light));

        // The setting lets a limited network have it all.
        assert_eq!(depth(Some(cellular), true), Some(Depth::Full));
        assert_eq!(depth(Some(low_data), true), Some(Depth::Full));
        assert_eq!(depth(None, true), Some(Depth::Full));

        // Offline, nothing, whatever the setting.
        assert_eq!(depth(Some(offline), false), None);
        assert_eq!(depth(Some(offline), true), None);
    }

    #[test]
    fn streams_found_ahead_serve_only_the_episode_they_were_found_for() {
        let prefetch = Prefetch::default();

        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_some());
        assert!(
            prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_none(),
            "taken once"
        );

        // Another episode, the other translation or another title start from scratch.
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(7), "8", Translation::Sub).is_none());
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Dub).is_none());
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(9), "6", Translation::Sub).is_none());

        // Too old to trust a signed stream URL.
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        if let Some(entry) = prefetch.lock().entry.as_mut() {
            entry.found = Instant::now() - PREFETCH_FRESH - Duration::from_secs(1);
        }
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_none());

        // Any other play leaves them for the episode they are for: a show opened in passing
        // while the ending ran used to take them with it.
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(9), "1", Translation::Sub).is_none());
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Dub).is_none());
        assert!(
            prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_some(),
            "still there"
        );
    }

    #[test]
    fn cancel_drops_what_was_started_ahead_and_a_warm_up_still_under_way() {
        let prefetch = Prefetch::default();
        let let_go = holding(&prefetch);
        Ahead::new(None, None, prefetch.clone()).cancelled();
        assert!(let_go.load(Ordering::SeqCst), "the torrent started ahead is let go");
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_none());

        // One that finishes after the cancel is let go as it arrives.
        let generation = prefetch.begin(AnilistId::new(7), "6", Translation::Sub);
        prefetch.clear();
        let (warm, late) = started_ahead();
        prefetch.finish(generation, AnilistId::new(7), "6", Translation::Sub, warm);
        assert!(late.load(Ordering::SeqCst));
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_none());
    }

    #[test]
    fn what_the_playback_takes_is_its_own_and_is_not_let_go() {
        let prefetch = Prefetch::default();
        let let_go = holding(&prefetch);
        let warm = prefetch.take(AnilistId::new(7), "6", Translation::Sub).unwrap();
        let _ = warm.into_parts();
        assert!(!let_go.load(Ordering::SeqCst));
    }

    #[test]
    fn a_play_that_cannot_use_torrents_lets_go_of_the_one_started_for_it() {
        let prefetch = Prefetch::default();
        let let_go = holding(&prefetch);
        let warm = prefetch.take(AnilistId::new(7), "6", Translation::Sub).unwrap();
        assert!(warm.without_torrents().is_none(), "nothing else was found");
        assert!(let_go.load(Ordering::SeqCst));
    }

    #[test]
    fn a_play_starting_while_its_warm_up_runs_lets_the_warm_up_go() {
        let prefetch = Prefetch::default();
        let generation = prefetch.begin(AnilistId::new(7), "6", Translation::Sub);
        assert!(prefetch.is_next(AnilistId::new(7), "6"));
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_none());
        let (warm, let_go) = started_ahead();
        prefetch.finish(generation, AnilistId::new(7), "6", Translation::Sub, warm);
        assert!(let_go.load(Ordering::SeqCst));
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn an_entry_left_untaken_is_let_go_when_it_goes_stale() {
        let prefetch = Prefetch::default();
        let let_go = holding(&prefetch);
        tokio::time::sleep(PREFETCH_FRESH - Duration::from_secs(1)).await;
        assert!(!let_go.load(Ordering::SeqCst), "still fresh");
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(let_go.load(Ordering::SeqCst), "let go once stale, without anything asking");
    }

    #[test]
    fn the_hand_over_is_measured_only_for_the_play_it_started() {
        let prefetch = Prefetch::default();
        prefetch.handed_over(AnilistId::new(7), "6");
        assert!(prefetch.handover_for(AnilistId::new(7), "8").is_none());
        prefetch.handed_over(AnilistId::new(7), "6");
        assert!(prefetch.handover_for(AnilistId::new(7), "6").is_some());
        assert!(prefetch.handover_for(AnilistId::new(7), "6").is_none(), "measured once");
    }

    #[test]
    fn about_ten_seconds_of_a_torrent_are_read_ahead_and_the_resume_point_too() {
        // 24 minutes in 720 MiB: about five MiB ahead.
        let total = 720 * 1024 * 1024;
        let at_start = context("6");
        let (starts, span) = torrent_spans(total, &at_start);
        assert_eq!(starts, [0]);
        assert_eq!(span, total / (24 * 6));

        let resumed = playback::PlaybackContext { resume_at: Some(720.0), ..context("6") };
        let (starts, _) = torrent_spans(total, &resumed);
        assert_eq!(starts, [0, total / 2]);

        // A small file is read whole, once.
        let (starts, span) = torrent_spans(1024, &resumed);
        assert_eq!((starts, span), (vec![0], 1024));
    }

    #[test]
    fn the_next_episode_subtitles_go_apart_from_the_playing_ones() {
        assert_eq!(ahead_dir("6"), "ahead-6");
        assert_eq!(ahead_dir("6.5/x"), "ahead-6-5-x");
    }
}
