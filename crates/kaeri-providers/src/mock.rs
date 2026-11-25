//! A configurable provider for tests.
//!
//! Exists so the registry's failover semantics — which are the part most likely to be got
//! wrong — can be exercised deterministically, with no network. It also counts calls, which
//! is how "a held-back provider is never contacted" is actually proven rather than assumed.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use kaeri_core::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, TitleHints, Translation},
    stream::Stream,
    traits::{Provider, ProviderKind, ProviderManifest, SourceCandidate},
};

pub struct MockProvider {
    manifest: ProviderManifest,
    hits: Vec<SearchHit>,
    /// Hits for one query, in place of `hits`, matched ignoring case and the space around it. A
    /// mock with any of these finds nothing for a query it has none for.
    hits_for: HashMap<String, Vec<SearchHit>>,
    episodes: Vec<Episode>,
    streams: Vec<Stream>,
    /// Answers for one key, in place of the shared lists above. A key names one title, so a
    /// mock standing in for a catalogue of several has to tell them apart by it.
    episodes_for: HashMap<ProviderKey, Vec<Episode>>,
    /// Listings of one key answered in turns, in place of `episodes_for`: see
    /// [`Self::with_listing_turns`].
    turns_for: HashMap<ProviderKey, Turns>,
    /// What a listing in the dub answers for one key, where it differs from the sub's.
    dub_for: HashMap<ProviderKey, Vec<Episode>>,
    /// A listing in the dub of a key `dub_for` does not name finds nothing, rather than the
    /// sub's list.
    no_dub: bool,
    streams_for: HashMap<ProviderKey, Vec<Stream>>,
    sources: Vec<SourceCandidate>,
    /// How a listing of `sources` answers, apart from every other call: after how long, and
    /// with which error in place of the list. See [`MockProvider::listing_after`].
    sources_after: Duration,
    sources_error: Option<ProviderError>,
    /// The one episode whose listing waits and fails as `sources_after` and `sources_error`
    /// say, when one is named: every other episode's releases answer at once.
    sources_episode: Option<String>,
    /// What each release in `sources` resolves to, by its id, for a pick by hand.
    source_streams: HashMap<String, Vec<Stream>>,
    error: Option<ProviderError>,
    unavailable: Option<String>,
    calls: Arc<AtomicUsize>,
    /// How long each call takes to answer, so tests can tell asking at once from asking in turn.
    latency: Duration,
    /// The hints each search, listing and stream request came with, in order.
    hints_seen: Arc<std::sync::Mutex<Vec<TitleHints>>>,
}

impl MockProvider {
    pub fn new(id: &str) -> Self {
        Self {
            manifest: ProviderManifest {
                id: id.to_owned(),
                display_name: id.to_owned(),
                version: "0".into(),
                kind: ProviderKind::Native,
                allowed_hosts: Vec::new(),
                translations: vec![Translation::Sub],
                torrents: false,
            },
            hits: Vec::new(),
            hits_for: HashMap::new(),
            episodes: Vec::new(),
            streams: Vec::new(),
            episodes_for: HashMap::new(),
            turns_for: HashMap::new(),
            dub_for: HashMap::new(),
            no_dub: false,
            streams_for: HashMap::new(),
            sources: Vec::new(),
            sources_after: Duration::ZERO,
            sources_error: None,
            sources_episode: None,
            source_streams: HashMap::new(),
            error: None,
            unavailable: None,
            calls: Arc::new(AtomicUsize::new(0)),
            latency: Duration::ZERO,
            hints_seen: Arc::default(),
        }
    }

    pub fn with_hits(mut self, hits: Vec<SearchHit>) -> Self {
        self.hits = hits;
        self
    }

    /// What a search for `query` finds. Once a mock has one of these, a search for any query
    /// it has none for finds nothing, as a catalogue that lists only some titles would.
    pub fn with_hits_for(mut self, query: &str, hits: Vec<SearchHit>) -> Self {
        self.hits_for.insert(query.trim().to_lowercase(), hits);
        self
    }

    /// The name a person reads for this source, in place of its id.
    pub fn named(mut self, display_name: &str) -> Self {
        display_name.clone_into(&mut self.manifest.display_name);
        self
    }

    pub fn with_episodes(mut self, episodes: Vec<Episode>) -> Self {
        self.episodes = episodes;
        self
    }

    pub fn with_streams(mut self, streams: Vec<Stream>) -> Self {
        self.streams = streams;
        self
    }

    /// Answer `episodes` for this key with these; every other key keeps the shared list.
    pub fn with_episodes_for(
        mut self,
        key: impl Into<ProviderKey>,
        episodes: Vec<Episode>,
    ) -> Self {
        self.episodes_for.insert(key.into(), episodes);
        self
    }

    /// Answer the listings of this key in turns: each takes the next turn's time and answers its
    /// episodes, and the last turn answers every listing after it. A source that lists a new
    /// episode only some time after it aired, for the checks that look for it.
    pub fn with_listing_turns(
        mut self,
        key: impl Into<ProviderKey>,
        turns: Vec<(Duration, Vec<Episode>)>,
    ) -> Self {
        assert!(!turns.is_empty(), "a listing needs a turn to answer with");
        self.turns_for.insert(key.into(), Turns { taken: Arc::default(), turns });
        self
    }

    /// Answer a listing of this key in the dub with these: a dub that stops short of the sub.
    /// Every other key's dub is its sub's list, unless [`Self::without_dub`].
    pub fn with_dub_for(mut self, key: impl Into<ProviderKey>, episodes: Vec<Episode>) -> Self {
        self.dub_for.insert(key.into(), episodes);
        self
    }

    /// Find nothing in the dub for a key [`Self::with_dub_for`] does not name.
    pub fn without_dub(mut self) -> Self {
        self.no_dub = true;
        self
    }

    /// Answer `resolve` for this key with these, whatever the episode; every other key keeps
    /// the shared list.
    pub fn with_streams_for(
        mut self,
        key: impl Into<ProviderKey>,
        streams: Vec<Stream>,
    ) -> Self {
        self.streams_for.insert(key.into(), streams);
        self
    }

    /// Fail every call with this error.
    pub fn with_sources(mut self, sources: Vec<SourceCandidate>) -> Self {
        self.sources = sources;
        self
    }

    /// Answer a listing of the releases (`sources`) only after `wait`, and with `error` in
    /// place of the list when one is given. Searches and every other call answer as they
    /// otherwise would, so a source can match a title at once and then be slow, or fail, to
    /// list an episode's releases.
    pub fn listing_after(mut self, wait: Duration, error: Option<ProviderError>) -> Self {
        self.sources_after = wait;
        self.sources_error = error;
        self
    }

    /// Keep [`MockProvider::listing_after`]'s wait and error to listings of `episode`, so a
    /// look at another episode's releases, as the source picker's coverage takes, answers at
    /// once.
    pub fn listing_after_only_for(mut self, episode: &str) -> Self {
        self.sources_episode = Some(episode.to_owned());
        self
    }

    /// Answer `resolve_source` for the release `source_id` with these, whatever the key and the
    /// episode. A release not named here is refused, as the trait's default refuses every one.
    pub fn with_source_streams(mut self, source_id: &str, streams: Vec<Stream>) -> Self {
        self.source_streams.insert(source_id.to_owned(), streams);
        self
    }

    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    pub fn failing(mut self, error: ProviderError) -> Self {
        self.error = Some(error);
        self
    }

    /// Report as withheld by local policy, e.g. a proxy check that failed.
    pub fn unavailable(mut self, reason: &str) -> Self {
        self.unavailable = Some(reason.to_owned());
        self
    }

    /// Name torrents for the engine to play, as a plugin with the `torrents` capability does.
    pub fn torrents(mut self) -> Self {
        self.manifest.torrents = true;
        self
    }

    pub fn kind(mut self, kind: ProviderKind) -> Self {
        self.manifest.kind = kind;
        self
    }

    /// Handle to the hints the provider was asked with, taken before the provider is boxed.
    pub fn hints_seen(&self) -> Arc<std::sync::Mutex<Vec<TitleHints>>> {
        Arc::clone(&self.hints_seen)
    }

    /// Handle to the call counter, taken before the provider is boxed.
    pub fn call_count(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.calls)
    }

    pub fn arc(self) -> Arc<dyn Provider> {
        Arc::new(self)
    }

    fn note(&self, hints: &TitleHints) {
        self.hints_seen.lock().unwrap_or_else(|e| e.into_inner()).push(hints.clone());
    }

    async fn record(&self) -> Result<(), ProviderError> {
        if !self.latency.is_zero() {
            crate::still::wait(self.latency).await;
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.error {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}

/// [`MockProvider::with_listing_turns`]: the turns, and how many listings have taken one.
struct Turns {
    taken: Arc<AtomicUsize>,
    turns: Vec<(Duration, Vec<Episode>)>,
}

#[async_trait]
impl Provider for MockProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _translation: Translation,
        hints: &kaeri_core::media::TitleHints,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        self.note(hints);
        self.record().await?;
        if self.hits_for.is_empty() {
            return Ok(self.hits.clone());
        }
        Ok(self.hits_for.get(&query.trim().to_lowercase()).cloned().unwrap_or_default())
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        translation: Translation,
        hints: &kaeri_core::media::TitleHints,
    ) -> Result<Vec<Episode>, ProviderError> {
        self.note(hints);
        if translation == Translation::Sub
            && let Some(turns) = self.turns_for.get(key)
        {
            let taken = turns.taken.fetch_add(1, Ordering::SeqCst);
            let (latency, episodes) = &turns.turns[taken.min(turns.turns.len() - 1)];
            crate::still::wait(*latency).await;
            self.record().await?;
            return Ok(episodes.clone());
        }
        self.record().await?;
        if translation == Translation::Dub {
            if let Some(dub) = self.dub_for.get(key) {
                return Ok(dub.clone());
            }
            if self.no_dub {
                return Err(ProviderError::NotFound);
            }
        }
        Ok(self.episodes_for.get(key).unwrap_or(&self.episodes).clone())
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        _episode: &str,
        _translation: Translation,
        hints: &kaeri_core::media::TitleHints,
    ) -> Result<Vec<Stream>, ProviderError> {
        self.note(hints);
        self.record().await?;
        Ok(self.streams_for.get(key).unwrap_or(&self.streams).clone())
    }

    async fn sources(
        &self,
        _key: &ProviderKey,
        episode: &str,
        _translation: Translation,
    ) -> Result<Vec<SourceCandidate>, ProviderError> {
        let held = self.sources_episode.as_deref().is_none_or(|held| held == episode);
        if held && !self.sources_after.is_zero() {
            tokio::time::sleep(self.sources_after).await;
        }
        self.record().await?;
        match &self.sources_error {
            Some(error) if held => Err(error.clone()),
            _ => Ok(self.sources.clone()),
        }
    }

    async fn resolve_source(
        &self,
        _key: &ProviderKey,
        _episode: &str,
        _translation: Translation,
        source_id: &str,
    ) -> Result<Vec<Stream>, ProviderError> {
        self.record().await?;
        self.source_streams.get(source_id).cloned().ok_or(ProviderError::NotFound)
    }

    fn is_available(&self) -> Result<(), ProviderError> {
        match &self.unavailable {
            Some(reason) => Err(ProviderError::Unavailable(reason.clone())),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_mock_returns_what_it_was_given_and_counts_calls() {
        let p = MockProvider::new("m")
            .with_streams(vec![Stream::new("x", kaeri_core::stream::StreamKind::Hls)]);
        let calls = p.call_count();

        let streams = p
            .resolve(&ProviderKey::new("k"), "1", Translation::Sub, &Default::default())
            .await
            .unwrap();
        assert_eq!(streams.len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn answers_named_for_a_key_are_that_keys_alone() {
        let p = MockProvider::new("m")
            .with_episodes(vec![Episode::new(1)])
            .with_episodes_for("two", vec![Episode::new(1), Episode::new(2)])
            .with_streams_for(
                "two",
                vec![Stream::new("y", kaeri_core::stream::StreamKind::Hls)],
            );

        let two = ProviderKey::new("two");
        assert_eq!(
            p.episodes(&two, Translation::Sub, &Default::default()).await.unwrap().len(),
            2
        );
        assert_eq!(
            p.resolve(&two, "2", Translation::Sub, &Default::default()).await.unwrap()[0].url,
            "y"
        );

        let other = ProviderKey::new("other");
        assert_eq!(
            p.episodes(&other, Translation::Sub, &Default::default()).await.unwrap().len(),
            1,
            "shared list"
        );
        assert!(
            p.resolve(&other, "1", Translation::Sub, &Default::default())
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_failing_mock_reports_its_error_and_still_counts_the_call() {
        let p = MockProvider::new("m").failing(ProviderError::Blocked("x".into()));
        let calls = p.call_count();
        assert!(p.search("q", Translation::Sub, &Default::default()).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn listings_in_turns_keep_the_last_turn() {
        let one = vec![Episode::new(1)];
        let two = vec![Episode::new(1), Episode::new(2)];
        let p = MockProvider::new("m").with_listing_turns(
            "k",
            vec![(Duration::ZERO, one.clone()), (Duration::ZERO, two.clone())],
        );
        let key = ProviderKey::new("k");
        let hints = kaeri_core::media::TitleHints::default();
        let list = || p.episodes(&key, Translation::Sub, &hints);
        assert_eq!(list().await.unwrap(), one);
        assert_eq!(list().await.unwrap(), two);
        assert_eq!(list().await.unwrap(), two, "the last turn stays");
    }

    #[test]
    fn an_unavailable_mock_reports_why() {
        let p = MockProvider::new("m").unavailable("proxy down");
        assert_eq!(p.is_available(), Err(ProviderError::Unavailable("proxy down".into())));
        assert!(MockProvider::new("m").is_available().is_ok());
    }
}
