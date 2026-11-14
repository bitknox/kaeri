//! The book registry: folders, servers and book plugins, asked in the person's order.
//!
//! The same rules as the show chain in [`crate::registry`], on [`BookSource`]. Health is shared
//! with it through [`HealthTracker`], a source that is held back by local policy is not an
//! unhealthy one, a pin ranks a source first and never takes the others away, and a source
//! switched off keeps its place and is never asked.
//!
//! What differs is that books are not a failover walk. A search asks every source at once,
//! because each keeps its own catalogue and the person wants all their shelves; a series, its
//! chapters and a chapter's content ask the one source an edition names, with that source's
//! health recorded. Only [`BookRegistry::lookup`], by ids another service knows, walks the chain
//! until one source answers.

use std::{
    collections::{BTreeSet, HashMap},
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::future::join_all;
use kaeri_core::{
    books::{
        BookHints, BookIds, BookSourceManifest, ChapterContent, PageRef, SeriesDetail,
        SeriesHit, SourceChapter,
    },
    error::ProviderError,
    media::Medium,
    traits::{BookSource, PageError},
};

use crate::health::HealthTracker;

/// Book sources in the person's order, with shared health.
#[derive(Clone, Default)]
pub struct BookRegistry {
    /// Behind a lock because sources arrive and leave while the app runs: plugins compile in the
    /// background, a server is added, a store plugin is removed.
    sources: Arc<RwLock<Vec<Arc<dyn BookSource>>>>,
    /// The person's order, by id. A source it does not name comes after those it does, by kind
    /// (your files, then servers, then plugins) and then by id.
    order: Arc<RwLock<Vec<String>>>,
    /// The sources switched off by id. Kept in the chain with their place, never asked.
    off: Arc<RwLock<BTreeSet<String>>>,
    health: HealthTracker,
    /// When each source joined, as a count of joins: a source swapped for its update, or a
    /// server put again with what it now holds, joins anew. What a source said before is not
    /// what it says now.
    joined: Arc<RwLock<HashMap<String, u64>>>,
    joins: Arc<AtomicU64>,
}

impl std::fmt::Debug for BookRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BookRegistry").field("sources", &self.ids()).finish_non_exhaustive()
    }
}

/// A chapter in a source's own terms: the source, its series reference and its chapter reference.
#[derive(Debug, Clone, Copy)]
pub struct ChapterAt<'a> {
    pub source: &'a str,
    pub series: &'a str,
    pub chapter: &'a str,
}

/// What a search over every source found.
#[derive(Debug, Default)]
pub struct Searched {
    /// Each source that answered, in order, with what it found. A source that found nothing is
    /// there with an empty list.
    pub found: Vec<(String, Vec<SeriesHit>)>,
    /// Each source that failed, and why. Never dropped, so the screen can name the one that broke.
    pub failures: Vec<(String, ProviderError)>,
}

impl Searched {
    pub fn is_empty(&self) -> bool {
        self.found.iter().all(|(_, hits)| hits.is_empty())
    }
}

fn read<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|e| e.into_inner())
}

fn write<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

impl BookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a source, or swap it for the one with its id: an update installed while the app runs.
    pub fn put(&self, source: Arc<dyn BookSource>) {
        let id = source.manifest().id.clone();
        self.health.register([id.clone()]);
        let turn = self.joins.fetch_add(1, Ordering::Relaxed) + 1;
        write(&self.joined).insert(id.clone(), turn);
        let mut sources = write(&self.sources);
        match sources.iter_mut().find(|s| s.manifest().id == id) {
            Some(slot) => *slot = source,
            None => sources.push(source),
        }
    }

    /// Take a source out, for a plugin that was removed while the app runs. False when it was
    /// not there.
    pub fn remove(&self, id: &str) -> bool {
        let mut sources = write(&self.sources);
        let before = sources.len();
        sources.retain(|s| s.manifest().id != id);
        sources.len() != before
    }

    pub fn health(&self) -> &HealthTracker {
        &self.health
    }

    /// Which join of the source with this id is in the chain, counted across every source: a
    /// newer one for a source swapped for its update or put again. `None` when it is not in.
    pub fn joined(&self, id: &str) -> Option<u64> {
        self.get(id)?;
        read(&self.joined).get(id).copied()
    }

    pub fn is_empty(&self) -> bool {
        read(&self.sources).is_empty()
    }

    /// Set the person's order. Ids the registry does not know are kept, for a source that is
    /// still compiling or a store that is not reachable yet.
    pub fn set_order(&self, ids: Vec<String>) {
        *write(&self.order) = ids;
        // A source moved up is asked, whatever it did before.
        self.health.reset_down();
    }

    /// Switch sources off, replacing the set.
    pub fn set_off(&self, ids: impl IntoIterator<Item = String>) {
        *write(&self.off) = ids.into_iter().collect();
        self.health.reset_down();
    }

    pub fn is_off(&self, id: &str) -> bool {
        read(&self.off).contains(id)
    }

    pub fn off(&self) -> Vec<String> {
        read(&self.off).iter().cloned().collect()
    }

    /// Every source, in the order they are asked: the person's order first, then the rest by
    /// kind and id. Includes those switched off, which keep their place.
    pub fn all(&self) -> Vec<Arc<dyn BookSource>> {
        let mut sources = read(&self.sources).clone();
        let order = read(&self.order);
        sources.sort_by_cached_key(|source| {
            let manifest = source.manifest();
            let place = order.iter().position(|id| *id == manifest.id).unwrap_or(usize::MAX);
            (place, manifest.kind.tier(), manifest.id.clone())
        });
        sources
    }

    pub fn ids(&self) -> Vec<String> {
        self.all().iter().map(|s| s.manifest().id.clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn BookSource>> {
        read(&self.sources).iter().find(|s| s.manifest().id == id).cloned()
    }

    pub fn manifest(&self, id: &str) -> Option<BookSourceManifest> {
        self.get(id).map(|s| s.manifest().clone())
    }

    /// Why a source is being held back: a folder whose drive is gone, a server signed out here.
    pub fn held_back(&self, id: &str) -> Option<String> {
        self.health.get(id).and_then(|h| h.held_back)
    }

    /// The same registry narrowed to one source, for a series pinned to it. A view: the health
    /// record is shared, so a failure while pinned counts everywhere. `None` when the id names
    /// nothing or the source is off, which the caller reads as "use the normal chain".
    pub fn pinned(&self, id: &str) -> Option<Self> {
        if self.is_off(id) {
            return None;
        }
        let source = self.get(id)?;
        Some(self.view(vec![source], vec![id.to_owned()]))
    }

    /// The same registry with `id` first and every other source kept behind it. A pin ranks a
    /// source first; a source with part of a series is no reason to leave the rest unreadable.
    pub fn preferring(&self, id: &str) -> Self {
        let mut order = self.ids();
        if let Some(at) = order.iter().position(|s| s == id) {
            let first = order.remove(at);
            order.insert(0, first);
        }
        self.view(self.all(), order)
    }

    fn view(&self, sources: Vec<Arc<dyn BookSource>>, order: Vec<String>) -> Self {
        Self {
            sources: Arc::new(RwLock::new(sources)),
            order: Arc::new(RwLock::new(order)),
            off: self.off.clone(),
            health: self.health.clone(),
            joined: self.joined.clone(),
            joins: self.joins.clone(),
        }
    }

    /// Give every source that went down another chance, for a change the person made.
    pub fn retry_down(&self) {
        self.health.reset_down();
    }

    /// The sources worth asking right now, in order: none switched off, none down.
    pub fn candidates(&self) -> Vec<Arc<dyn BookSource>> {
        let usable = self.health.usable(&self.ids());
        self.all()
            .into_iter()
            .filter(|s| {
                let id = &s.manifest().id;
                usable.contains(id) && !self.is_off(id)
            })
            .collect()
    }

    /// Whether `source` takes part in a search for `medium`.
    fn covers(source: &dyn BookSource, medium: Option<Medium>) -> bool {
        let media = &source.manifest().media;
        match medium {
            Some(medium) => media.is_empty() || media.contains(&medium),
            None => true,
        }
    }

    /// Ask a source whether local policy lets it be asked. A hold is lifted when the source says
    /// it is available again, so the hold never outlives its reason.
    fn admit(&self, source: &dyn BookSource, now: i64) -> Result<(), ProviderError> {
        let id = &source.manifest().id;
        match source.is_available() {
            Err(reason) => {
                self.health.record_failure(id, &reason, now);
                Err(reason)
            }
            Ok(()) => {
                self.health.release(id);
                Ok(())
            }
        }
    }

    /// Ask one source for `call`, recording its health. A source that is off, held back or gone
    /// answers without being asked.
    pub async fn ask<T, F, Fut>(&self, id: &str, now: i64, call: F) -> Result<T, ProviderError>
    where
        F: FnOnce(Arc<dyn BookSource>) -> Fut,
        Fut: std::future::Future<Output = Result<T, ProviderError>>,
    {
        let source = self.get(id).ok_or(ProviderError::NotFound)?;
        if self.is_off(id) {
            return Err(ProviderError::Unavailable("switched off".into()));
        }
        self.admit(source.as_ref(), now)?;
        let started = kaeri_store::instant_now();
        match call(source).await {
            Ok(value) => {
                self.health.record_success(id, kaeri_store::elapsed(started), now);
                Ok(value)
            }
            Err(error) => {
                self.health.record_failure(id, &error, now);
                Err(error)
            }
        }
    }

    /// Search every source at once. Each keeps its own catalogue, so the answers are gathered, in
    /// the person's order, not walked. `medium` leaves out the sources that do not carry it.
    pub async fn search(
        &self,
        query: &str,
        hints: &BookHints,
        medium: Option<Medium>,
        now: i64,
    ) -> Searched {
        self.search_in(query, hints, medium, None, now).await
    }

    /// [`Self::search`] asking only the sources in `scope`, by id; `None` asks every source.
    /// A source outside it is never asked.
    pub async fn search_in(
        &self,
        query: &str,
        hints: &BookHints,
        medium: Option<Medium>,
        scope: Option<&[String]>,
        now: i64,
    ) -> Searched {
        let scoped = |s: &Arc<dyn BookSource>| {
            scope.is_none_or(|scope| scope.iter().any(|id| *id == s.manifest().id))
        };
        let asked: Vec<_> = self
            .candidates()
            .into_iter()
            .filter(|s| scoped(s) && Self::covers(s.as_ref(), medium))
            .collect();
        let answers = join_all(asked.into_iter().map(|source| async move {
            let id = source.manifest().id.clone();
            let answer = self
                .ask(&id, now, |source| async move { source.search(query, hints).await })
                .await;
            (id, answer)
        }))
        .await;
        let mut out = Searched::default();
        for (id, answer) in answers {
            match answer {
                Ok(hits) => out.found.push((id, hits)),
                // A source without the title is not a failure.
                Err(ProviderError::NotFound) => out.found.push((id, Vec::new())),
                Err(ProviderError::Unavailable(_)) => {}
                Err(error) => out.failures.push((id, error)),
            }
        }
        out
    }

    /// Find a series by the ids other services know it by, walking the chain until one source
    /// has it. Sources without an index answer `None` and are passed over.
    pub async fn lookup(&self, ids: &BookIds, now: i64) -> Option<(String, SeriesHit)> {
        for source in self.candidates() {
            let id = source.manifest().id.clone();
            let found =
                self.ask(&id, now, |source| async move { source.lookup(ids).await }).await;
            if let Ok(Some(hit)) = found {
                return Some((id, hit));
            }
        }
        None
    }

    pub async fn series(
        &self,
        source: &str,
        id: &str,
        hints: &BookHints,
        now: i64,
    ) -> Result<SeriesDetail, ProviderError> {
        self.ask(source, now, |s| async move { s.series(id, hints).await }).await
    }

    pub async fn chapters(
        &self,
        source: &str,
        id: &str,
        hints: &BookHints,
        now: i64,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        self.ask(source, now, |s| async move { s.chapters(id, hints).await }).await
    }

    pub async fn content(
        &self,
        source: &str,
        series: &str,
        chapter: &str,
        now: i64,
    ) -> Result<ChapterContent, ProviderError> {
        self.ask(source, now, |s| async move { s.content(series, chapter).await }).await
    }

    /// A chapter's page images, when it is made of them, found without downloading a whole
    /// book file: what the reader's preview of the next chapter fetches its first page from.
    pub async fn chapter_pages(
        &self,
        source: &str,
        series: &str,
        chapter: &str,
        now: i64,
    ) -> Result<Option<Vec<PageRef>>, ProviderError> {
        self.ask(source, now, |s| async move { s.chapter_pages(series, chapter).await }).await
    }

    /// Tell a source a chapter was read or taken back. A failure is for the log: the person's
    /// own mark is already kept.
    pub async fn report_progress(
        &self,
        source: &str,
        series: &str,
        chapter: &str,
        read: bool,
        now: i64,
    ) -> Result<(), ProviderError> {
        self.ask(source, now, |s| async move { s.report_progress(series, chapter, read).await })
            .await
    }

    /// Fetch page `index` of `pages`, which `source` named for `chapter` of `series`.
    ///
    /// A page that fails on the way (a dropped connection, a timeout, a busy server) is fetched
    /// again after each of [`PAGE_RETRIES`]. A page address that answers 403 or 410 has expired: the source is asked once more for the
    /// chapter, `pages` is replaced with the fresh addresses, and the page is fetched again. A
    /// second refusal is the answer. `plain` fetches an address with the engine's own client,
    /// for a source that does not fetch its own pages.
    pub async fn fetch_page<F, Fut>(
        &self,
        at: &ChapterAt<'_>,
        pages: &mut Vec<PageRef>,
        index: usize,
        now: i64,
        plain: F,
    ) -> Result<Vec<u8>, ProviderError>
    where
        F: Fn(PageRef) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<u8>, PageError>>,
    {
        let ChapterAt { source, series, chapter } = *at;
        let owner = self.get(source).ok_or(ProviderError::NotFound)?;
        let once = |page: PageRef| {
            let owner = owner.clone();
            let plain = &plain;
            async move {
                match owner.fetch_page(&page).await {
                    Err(PageError::Unsupported) => plain(page).await,
                    other => other,
                }
            }
        };
        let fetch = |page: PageRef| {
            let once = &once;
            async move {
                let mut waits = PAGE_RETRIES.iter();
                loop {
                    match once(page.clone()).await {
                        Err(error) if error.transient() => match waits.next() {
                            Some(wait) => {
                                tracing::debug!(
                                    source,
                                    chapter,
                                    ?error,
                                    "a page failed on the way; trying again"
                                );
                                tokio::time::sleep(*wait).await;
                            }
                            None => return Err(error),
                        },
                        other => return other,
                    }
                }
            }
        };
        let page = pages.get(index).cloned().ok_or(ProviderError::NotFound)?;
        let error = match fetch(page).await {
            Ok(bytes) => return Ok(bytes),
            Err(error) => error,
        };
        if !error.expired() {
            return Err(page_failure(error));
        }
        tracing::debug!(source, chapter, "a page address expired; asking for fresh ones");
        match self.content(source, series, chapter, now).await? {
            ChapterContent::Images(fresh) => *pages = fresh,
            _ => return Err(page_failure(error)),
        }
        let page = pages.get(index).cloned().ok_or(ProviderError::NotFound)?;
        fetch(page).await.map_err(page_failure)
    }

    /// One picture a text chapter of `source` links, fetched as its pages are: by the source's
    /// own `fetch_page`, so through a plugin's `allowed-hosts` and the host's pacing, with the
    /// same retries. A source that does not fetch pages itself has it refused; the engine's
    /// own client never fetches it. An address that has expired is not asked for again.
    pub async fn fetch_picture(
        &self,
        source: &str,
        picture: &PageRef,
    ) -> Result<Vec<u8>, ProviderError> {
        let owner = self.get(source).ok_or(ProviderError::NotFound)?;
        let mut waits = PAGE_RETRIES.iter();
        loop {
            match owner.fetch_page(picture).await {
                Ok(bytes) => return Ok(bytes),
                Err(error) if error.transient() => match waits.next() {
                    Some(wait) => tokio::time::sleep(*wait).await,
                    None => return Err(page_failure(error)),
                },
                Err(error) => return Err(page_failure(error)),
            }
        }
    }

    /// Probe every source that is not off, to refresh their health. Held back sources are
    /// skipped without a mark against them.
    pub async fn check_all(&self, now: i64) {
        join_all(self.all().into_iter().filter(|s| !self.is_off(&s.manifest().id)).map(
            |source| async move {
                let id = source.manifest().id.clone();
                let _ = self
                    .ask(&id, now, |source| async move { source.health_check().await })
                    .await;
            },
        ))
        .await;
    }
}

/// The waits before a page that failed on the way is fetched again: two more tries.
pub const PAGE_RETRIES: [Duration; 2] = [Duration::from_millis(500), Duration::from_secs(2)];

fn page_failure(error: PageError) -> ProviderError {
    match error {
        PageError::Status(404) => ProviderError::NotFound,
        PageError::Status(status) => ProviderError::Blocked(format!("page answered {status}")),
        PageError::Failed(e) => e,
        PageError::Unsupported => {
            ProviderError::Other("the source has no way to fetch pages".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    };

    use async_trait::async_trait;
    use kaeri_core::books::BookSourceKind;

    use super::*;

    /// A source with a scripted catalogue.
    struct Fake {
        manifest: BookSourceManifest,
        hits: Vec<SeriesHit>,
        error: Mutex<Option<ProviderError>>,
        held: Mutex<Option<String>>,
        asked: AtomicU32,
        /// Content calls so far, to tell expiring addresses apart.
        contents: AtomicU32,
    }

    impl Fake {
        fn new(id: &str, kind: BookSourceKind, titles: &[&str]) -> Arc<Self> {
            Arc::new(Self {
                manifest: BookSourceManifest {
                    id: id.into(),
                    name: id.into(),
                    version: "1".into(),
                    kind,
                    media: vec![Medium::Manga],
                    languages: Vec::new(),
                    allowed_hosts: Vec::new(),
                    capabilities: Vec::new(),
                },
                hits: titles.iter().map(|t| SeriesHit::new(format!("{id}/{t}"), *t)).collect(),
                error: Mutex::new(None),
                held: Mutex::new(None),
                asked: AtomicU32::new(0),
                contents: AtomicU32::new(0),
            })
        }

        fn failing(self: &Arc<Self>, error: ProviderError) {
            *self.error.lock().unwrap() = Some(error);
        }

        fn hold(self: &Arc<Self>, reason: Option<&str>) {
            *self.held.lock().unwrap() = reason.map(str::to_owned);
        }
    }

    #[async_trait]
    impl BookSource for Fake {
        fn manifest(&self) -> &BookSourceManifest {
            &self.manifest
        }

        async fn search(
            &self,
            _: &str,
            _: &BookHints,
        ) -> Result<Vec<SeriesHit>, ProviderError> {
            self.asked.fetch_add(1, Ordering::SeqCst);
            match self.error.lock().unwrap().clone() {
                Some(error) => Err(error),
                None => Ok(self.hits.clone()),
            }
        }

        async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
            Ok(SeriesDetail::new(id, id))
        }

        async fn chapters(
            &self,
            _: &str,
            _: &BookHints,
        ) -> Result<Vec<SourceChapter>, ProviderError> {
            Ok(Vec::new())
        }

        async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
            let round = self.contents.fetch_add(1, Ordering::SeqCst);
            Ok(ChapterContent::Images(vec![PageRef::new(format!(
                "https://pages.example/{round}"
            ))]))
        }

        fn is_available(&self) -> Result<(), ProviderError> {
            match self.held.lock().unwrap().clone() {
                Some(reason) => Err(ProviderError::Unavailable(reason)),
                None => Ok(()),
            }
        }

        async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
            // Round 0 addresses have expired.
            if page.url.ends_with("/0") {
                Err(PageError::Status(410))
            } else {
                Ok(page.url.clone().into_bytes())
            }
        }
    }

    fn registry(sources: &[&Arc<Fake>]) -> BookRegistry {
        let registry = BookRegistry::new();
        for source in sources {
            registry.put((*source).clone());
        }
        registry
    }

    #[test]
    fn a_source_swapped_for_its_update_joins_anew_and_a_view_shares_the_count() {
        let registry = BookRegistry::new();
        assert_eq!(registry.joined("alpha"), None);
        registry.put(Fake::new("alpha", BookSourceKind::Plugin, &[]));
        registry.put(Fake::new("beta", BookSourceKind::Plugin, &[]));
        let first = registry.joined("alpha").unwrap();
        assert_ne!(registry.joined("beta"), Some(first));
        // The same id again, as an update installed while the app runs.
        registry.put(Fake::new("alpha", BookSourceKind::Plugin, &[]));
        let second = registry.joined("alpha").unwrap();
        assert!(second > first);
        assert_eq!(registry.preferring("beta").joined("alpha"), Some(second));
        registry.remove("alpha");
        assert_eq!(registry.joined("alpha"), None);
    }

    #[tokio::test]
    async fn unordered_sources_rank_by_kind_then_id_and_the_persons_order_wins() {
        let plugin = Fake::new("alpha", BookSourceKind::Plugin, &[]);
        let server = Fake::new("zed", BookSourceKind::Server, &[]);
        let folders = Fake::new("folders", BookSourceKind::Folder, &[]);
        let registry = registry(&[&plugin, &server, &folders]);
        assert_eq!(registry.ids(), ["folders", "zed", "alpha"]);
        registry.set_order(vec!["alpha".into(), "folders".into()]);
        assert_eq!(registry.ids(), ["alpha", "folders", "zed"]);
    }

    #[tokio::test]
    async fn a_pin_ranks_a_source_first_and_never_removes_the_others() {
        let a = Fake::new("a", BookSourceKind::Plugin, &["One"]);
        let b = Fake::new("b", BookSourceKind::Plugin, &["Two"]);
        let registry = registry(&[&a, &b]);
        let pinned = registry.preferring("b");
        assert_eq!(pinned.ids(), ["b", "a"]);
        assert_eq!(registry.ids(), ["a", "b"], "the chain itself is not reordered");
        assert_eq!(registry.pinned("b").unwrap().ids(), ["b"]);
        registry.set_off(["b".to_string()]);
        assert!(registry.pinned("b").is_none(), "a pin on a source that is off falls back");
        assert_eq!(pinned.candidates().len(), 1, "an off switch is shared by the view");
        assert!(registry.pinned("gone").is_none());
    }

    #[tokio::test]
    async fn a_search_asks_every_source_and_names_the_one_that_failed() {
        let a = Fake::new("a", BookSourceKind::Folder, &["Lantern"]);
        let b = Fake::new("b", BookSourceKind::Plugin, &["Harbour"]);
        let c = Fake::new("c", BookSourceKind::Plugin, &[]);
        c.failing(ProviderError::Transport("refused".into()));
        let registry = registry(&[&a, &b, &c]);
        let out = registry.search("l", &BookHints::default(), Some(Medium::Manga), 1).await;
        assert_eq!(out.found.len(), 2);
        assert_eq!(out.found[0].0, "a");
        assert_eq!(out.failures.len(), 1);
        assert_eq!(out.failures[0].0, "c");
    }

    #[tokio::test]
    async fn a_source_switched_off_is_never_asked_and_keeps_its_place() {
        let a = Fake::new("a", BookSourceKind::Plugin, &["One"]);
        let b = Fake::new("b", BookSourceKind::Plugin, &["Two"]);
        let registry = registry(&[&a, &b]);
        registry.set_off(["a".to_string()]);
        let out = registry.search("", &BookHints::default(), None, 1).await;
        assert_eq!(out.found.len(), 1);
        assert_eq!(a.asked.load(Ordering::SeqCst), 0);
        assert_eq!(registry.ids(), ["a", "b"]);
        let direct = registry.series("a", "x", &BookHints::default(), 1).await;
        assert_eq!(direct.unwrap_err(), ProviderError::Unavailable("switched off".into()));
        registry.set_off([]);
        assert!(registry.series("a", "x", &BookHints::default(), 1).await.is_ok());
    }

    #[tokio::test]
    async fn a_held_back_source_is_not_unhealthy_and_is_asked_again_when_it_lifts() {
        let a = Fake::new("a", BookSourceKind::Folder, &["One"]);
        a.hold(Some("the drive is not connected"));
        let registry = registry(&[&a]);
        for _ in 0..5 {
            let out = registry.search("", &BookHints::default(), None, 1).await;
            assert!(out.found.is_empty());
        }
        assert_eq!(a.asked.load(Ordering::SeqCst), 0, "a held source is not contacted");
        let health = registry.health().get("a").unwrap();
        assert_eq!(health.health, kaeri_core::error::Health::Unknown);
        assert_eq!(registry.held_back("a").as_deref(), Some("the drive is not connected"));
        a.hold(None);
        let out = registry.search("", &BookHints::default(), None, 2).await;
        assert_eq!(out.found.len(), 1);
        assert_eq!(registry.held_back("a"), None);
    }

    #[tokio::test]
    async fn a_source_that_keeps_failing_goes_down_and_a_change_gives_it_another_chance() {
        let a = Fake::new("a", BookSourceKind::Plugin, &["One"]);
        a.failing(ProviderError::Transport("down".into()));
        let registry = registry(&[&a]);
        for _ in 0..4 {
            registry.search("", &BookHints::default(), None, 1).await;
        }
        assert_eq!(a.asked.load(Ordering::SeqCst), 3, "down after three, then left alone");
        registry.set_order(vec!["a".into()]);
        registry.search("", &BookHints::default(), None, 2).await;
        assert_eq!(a.asked.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn a_source_that_has_not_the_medium_is_left_out() {
        let a = Fake::new("a", BookSourceKind::Plugin, &["One"]);
        let registry = registry(&[&a]);
        let out = registry.search("", &BookHints::default(), Some(Medium::Novel), 1).await;
        assert!(out.found.is_empty() && out.failures.is_empty());
    }

    #[tokio::test]
    async fn a_page_that_answers_410_is_asked_for_again_once() {
        let a = Fake::new("a", BookSourceKind::Plugin, &[]);
        let registry = registry(&[&a]);
        let ChapterContent::Images(mut pages) =
            registry.content("a", "s", "c", 1).await.unwrap()
        else {
            panic!("images")
        };
        let bytes = registry
            .fetch_page(
                &ChapterAt { source: "a", series: "s", chapter: "c" },
                &mut pages,
                0,
                1,
                |_| async { Err(PageError::Unsupported) },
            )
            .await
            .unwrap();
        assert_eq!(bytes, b"https://pages.example/1");
        assert_eq!(a.contents.load(Ordering::SeqCst), 2, "asked once more, not forever");
        assert_eq!(pages[0].url, "https://pages.example/1");
    }

    #[tokio::test]
    async fn a_second_refusal_is_the_answer() {
        struct Stubborn(Arc<Fake>);
        #[async_trait]
        impl BookSource for Stubborn {
            fn manifest(&self) -> &BookSourceManifest {
                self.0.manifest()
            }
            async fn search(
                &self,
                q: &str,
                h: &BookHints,
            ) -> Result<Vec<SeriesHit>, ProviderError> {
                self.0.search(q, h).await
            }
            async fn series(
                &self,
                i: &str,
                h: &BookHints,
            ) -> Result<SeriesDetail, ProviderError> {
                self.0.series(i, h).await
            }
            async fn chapters(
                &self,
                i: &str,
                h: &BookHints,
            ) -> Result<Vec<SourceChapter>, ProviderError> {
                self.0.chapters(i, h).await
            }
            async fn content(&self, s: &str, c: &str) -> Result<ChapterContent, ProviderError> {
                self.0.content(s, c).await
            }
            async fn fetch_page(&self, _: &PageRef) -> Result<Vec<u8>, PageError> {
                Err(PageError::Status(403))
            }
        }
        let inner = Fake::new("a", BookSourceKind::Plugin, &[]);
        let registry = BookRegistry::new();
        registry.put(Arc::new(Stubborn(inner.clone())));
        let mut pages = vec![PageRef::new("https://pages.example/x")];
        let error = registry
            .fetch_page(
                &ChapterAt { source: "a", series: "s", chapter: "c" },
                &mut pages,
                0,
                1,
                |_| async { Err(PageError::Unsupported) },
            )
            .await
            .unwrap_err();
        assert_eq!(error, ProviderError::Blocked("page answered 403".into()));
        assert_eq!(inner.contents.load(Ordering::SeqCst), 1);
    }

    /// A source whose pages fail with `errors` in turn, then load.
    struct Flaky {
        inner: Arc<Fake>,
        errors: Mutex<Vec<PageError>>,
        fetched: AtomicU32,
    }

    #[async_trait]
    impl BookSource for Flaky {
        fn manifest(&self) -> &BookSourceManifest {
            self.inner.manifest()
        }
        async fn search(
            &self,
            q: &str,
            h: &BookHints,
        ) -> Result<Vec<SeriesHit>, ProviderError> {
            self.inner.search(q, h).await
        }
        async fn series(&self, i: &str, h: &BookHints) -> Result<SeriesDetail, ProviderError> {
            self.inner.series(i, h).await
        }
        async fn chapters(
            &self,
            i: &str,
            h: &BookHints,
        ) -> Result<Vec<SourceChapter>, ProviderError> {
            self.inner.chapters(i, h).await
        }
        async fn content(&self, s: &str, c: &str) -> Result<ChapterContent, ProviderError> {
            self.inner.content(s, c).await
        }
        async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
            self.fetched.fetch_add(1, Ordering::SeqCst);
            let mut errors = self.errors.lock().unwrap();
            if errors.is_empty() {
                Ok(page.url.clone().into_bytes())
            } else {
                Err(errors.remove(0))
            }
        }
    }

    async fn fetch_flaky(errors: Vec<PageError>) -> (Result<Vec<u8>, ProviderError>, u32) {
        let flaky = Arc::new(Flaky {
            inner: Fake::new("a", BookSourceKind::Plugin, &[]),
            errors: Mutex::new(errors),
            fetched: AtomicU32::new(0),
        });
        let registry = BookRegistry::new();
        registry.put(flaky.clone());
        let mut pages = vec![PageRef::new("https://pages.example/p1")];
        let result = registry
            .fetch_page(
                &ChapterAt { source: "a", series: "s", chapter: "c" },
                &mut pages,
                0,
                1,
                |_| async { Err(PageError::Unsupported) },
            )
            .await;
        (result, flaky.fetched.load(Ordering::SeqCst))
    }

    #[tokio::test(start_paused = true)]
    async fn a_page_that_fails_on_the_way_is_fetched_again() {
        let transport = || PageError::Failed(ProviderError::Transport("reset".into()));
        let started = tokio::time::Instant::now();
        let (result, fetched) = fetch_flaky(vec![transport(), PageError::Status(503)]).await;
        assert_eq!(result.unwrap(), b"https://pages.example/p1");
        assert_eq!(fetched, 3);
        assert!(started.elapsed() >= PAGE_RETRIES[0] + PAGE_RETRIES[1]);

        // Three failures in a row are the answer.
        let (result, fetched) =
            fetch_flaky(vec![PageError::Status(502), transport(), PageError::Status(429)])
                .await;
        assert_eq!(result.unwrap_err(), ProviderError::Blocked("page answered 429".into()));
        assert_eq!(fetched, 3);

        // A server that answered is believed at once.
        let (result, fetched) = fetch_flaky(vec![PageError::Status(404)]).await;
        assert_eq!(result.unwrap_err(), ProviderError::NotFound);
        assert_eq!(fetched, 1);
    }

    #[tokio::test]
    async fn a_source_removed_while_running_leaves_the_chain() {
        let a = Fake::new("a", BookSourceKind::Plugin, &["One"]);
        let registry = registry(&[&a]);
        assert!(registry.remove("a"));
        assert!(!registry.remove("a"));
        assert!(registry.is_empty());
        assert_eq!(
            registry.series("a", "x", &BookHints::default(), 1).await.unwrap_err(),
            ProviderError::NotFound
        );
    }
}
