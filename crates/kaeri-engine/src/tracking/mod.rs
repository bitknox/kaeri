//! Wiring sync to the UI.
//!
//! The engine in `kaeri-track` is deliberately ignorant of the interface; this is the part
//! that builds trackers from config, runs the background loops, and turns their reports into
//! [`Event`]s. Two rules shape it:
//!
//! - **Sync never blocks anything.** Every loop lives on its own task, and a tracker being down
//!   produces one badge change rather than a stream of toasts.
//! - **Signing in is the only interactive part**, and it opens a browser, so it runs off the UI
//!   thread like everything else.

pub mod commands;
pub mod device;
pub mod native;

use kaeri_model::Msg;
use std::sync::Arc;

use kaeri_core::{
    config::{Config, Paths},
    ids::AnilistId,
    traits::{TrackOp, Tracker},
};
use kaeri_meta::anilist::AniList;
use kaeri_model::{
    catalog::{ContentFor, LibrarySegment},
    event::Event,
    notice::{Notice, NoticeHint},
    progress::TitleProgress,
    sync::{ConflictRow, PullError, SyncState},
};
use kaeri_store::Store;
#[cfg(feature = "desktop")]
use kaeri_track::auth;
use kaeri_track::{AniListTracker, TokenStore, sync};

use crate::{host::Host, jobs::JobGuard, outbox::Outbox};

/// Everything the sync loops need.
#[derive(Clone)]
pub struct Sync {
    pub trackers: Vec<Arc<dyn Tracker>>,
    /// The AniList tracker as its concrete type.
    ///
    /// The `Tracker` trait returns only the syncable projection — ids, progress, status, score —
    /// which is right for sync but not enough for the Library screen, which needs titles and
    /// cover art. Keeping the concrete handle is cheaper and clearer than putting `as_any` on a
    /// public trait for one caller's benefit.
    pub anilist: Option<Arc<AniListTracker>>,
    /// The MAL tracker as its concrete type, for the same reason as `anilist`: refresh is not on
    /// the `Tracker` trait — AniList has no such concept, and putting it there would make every
    /// implementation carry it.
    pub mal: Option<Arc<kaeri_track::MalTracker>>,
    /// The Kitsu tracker as its concrete type, for the same reason as `mal`: its token
    /// refreshes silently mid-call and the renewed pair has to be written back.
    pub kitsu: Option<Arc<kaeri_track::KitsuTracker>>,
    /// Where a Kitsu sign-in and the Kitsu tracker go: Kitsu itself, or a fixture build's fake.
    pub kitsu_site: kaeri_track::kitsu::Site,
    /// The MangaUpdates tracker as its concrete type: its session has no refresh, but a sign-in
    /// hands it the token.
    pub mangaupdates: Option<Arc<kaeri_track::MangaUpdatesTracker>>,
    /// Where a MangaUpdates sign-in goes: the service, or a fixture build's fake.
    pub mangaupdates_api: String,
    /// The public lookups that match a work to tracker entries: asked only while a tracker that
    /// needs the answer is signed in.
    pub lookups: Arc<kaeri_track::lookup::Lookups>,
    /// The Simkl tracker as its concrete type, for the same reason: a sign-in through its
    /// OAuth 2.0 lasts a week, and the pair renewed mid-call has to be written back.
    pub simkl: Option<Arc<kaeri_track::SimklTracker>>,
    pub tokens: TokenStore,
    pub store: Store,
    pub config: Config,
    /// Kept so the OAuth token exchange goes through the same configured client — including its
    /// proxy settings — as every other request.
    pub http: kaeri_net::HttpClient,
    /// Each tracker's last library pull failure and when it happened, until a pull succeeds.
    ///
    /// Held here rather than sent once, because every state this module publishes replaces
    /// the previous one whole: the next drain tick would otherwise wipe the error off the
    /// Accounts screen a minute after it appeared.
    pub pull_errors: Arc<std::sync::Mutex<std::collections::HashMap<String, PullError>>>,
    /// When the library was last pulled and whether the app is away: the 15-minute loop rests
    /// while it is, and Sync now does not pull again within [`PULL_COALESCE`] of a pull.
    pub pulls: Arc<PullGate>,
    /// Whether a front end has asked for the Library this run: a status set then has the
    /// Library's lists answered again, so the one on screen moves the title at once.
    pub library_shown: Arc<std::sync::atomic::AtomicBool>,
    /// The sign-ins a native app's sheet is showing, each waiting for its redirect.
    pub pending: native::PendingSignIns,
    /// The device-code sign-ins running, one per tracker: see [`device`].
    pub device: device::DeviceFlows,
    /// How titles are named, which under `anilist` follows what each pull's viewer says: see
    /// [`Self::with_titles`].
    pub titles: Option<kaeri_meta::titles::Titles>,
    /// When trackers disagree on reading progress, the latest change wins rather than the
    /// furthest: the setting as it stands, changed from Settings while the loops run.
    pub latest_wins: Arc<std::sync::atomic::AtomicBool>,
}

impl Sync {
    /// Build trackers from config.
    ///
    /// A tracker with no stored token is still constructed. That is deliberate: it makes the
    /// Accounts screen able to offer a sign-in. Nothing is queued or pulled for it until it
    /// is signed in — Kitsu is enabled by default, so "enabled" no longer implies intent —
    /// and watching now and signing in later still syncs, because the first pull derives
    /// progress from the watch log and reconciles it outward.
    ///
    /// With the desktop's tokens, for the command line: see [`token_store`]. The engine builds
    /// its trackers [`Self::with_tokens`], over wherever its host keeps them.
    #[cfg(feature = "desktop")]
    pub fn build(
        config: &Config,
        paths: &Paths,
        store: &Store,
        http: &kaeri_net::HttpClient,
    ) -> Self {
        Self::with_tokens(config, store, http, token_store(config, paths))
    }

    /// Build against an existing token store.
    ///
    /// The store caches reads, so sharing one means the OS keychain is consulted once per process
    /// instead of once per caller — which on macOS is the difference between one password prompt
    /// and five.
    pub fn with_tokens(
        config: &Config,
        store: &Store,
        http: &kaeri_net::HttpClient,
        tokens: TokenStore,
    ) -> Self {
        let mut trackers: Vec<Arc<dyn Tracker>> = Vec::new();
        let mut anilist = None;
        let mut mal = None;
        let mut kitsu = None;
        let mut simkl = None;
        let mut mangaupdates = None;
        let reads: Arc<dyn kaeri_track::read::ReadMapping> =
            Arc::new(StoreReadMapping { store: store.clone() });

        if config.trackers.is_enabled("anilist") {
            let token = tokens.get("anilist").ok();
            if token.is_none() {
                tracing::info!("anilist enabled but not signed in; queueing locally");
            }
            let client =
                AniList::new(http.clone(), config.network.anilist_rate_limit).with_token(token);
            let tracker = Arc::new(AniListTracker::new(client).with_store(store.clone()));
            trackers.push(tracker.clone());
            anilist = Some(tracker);
        }

        if config.trackers.is_enabled("mal")
            && let Some(client_id) = config.trackers.mal.client_id.clone()
        {
            // MAL keys on `mal_id`, so this is the first tracker that needs the mapping layer —
            // exactly what the plan said would make a second tracker cheap rather than a new
            // subsystem.
            let tokens_for_mal = tokens.get_pair("mal").ok();
            if tokens_for_mal.is_none() {
                tracing::info!("mal enabled but not signed in; queueing locally");
            }
            // Given the store so it stays in step with another process signed in through it,
            // the app beside the terminal: see `kaeri_track`'s `held` module.
            let tracker = Arc::new(
                kaeri_track::MalTracker::new(
                    client_id,
                    http.plain().clone(),
                    Arc::new(StoreMapping { store: store.clone() }),
                    tokens_for_mal,
                )
                .with_read_mapping(reads.clone())
                .with_token_store(tokens.clone()),
            );
            trackers.push(tracker.clone());
            mal = Some(tracker);
        }

        // Kitsu needs no client registration at all — its token endpoint takes the password
        // grant bare — so unlike every other tracker there is no config to check first.
        if config.trackers.is_enabled("kitsu") {
            let tokens_for_kitsu = tokens.get_pair("kitsu").ok();
            if tokens_for_kitsu.is_none() {
                tracing::info!("kitsu enabled but not signed in; queueing locally");
            }
            let tracker = kitsu_tracker(
                http,
                store,
                &tokens,
                kaeri_track::kitsu::Site::default(),
                tokens_for_kitsu,
            );
            trackers.push(tracker.clone());
            kitsu = Some(tracker);
        }

        // MangaUpdates needs no registration either: a form signs in with a name and password.
        if config.trackers.is_enabled("mangaupdates") {
            let tracker = Arc::new(
                kaeri_track::MangaUpdatesTracker::new(
                    http.plain().clone(),
                    reads,
                    tokens.get_pair("mangaupdates").ok(),
                )
                .with_token_store(tokens.clone()),
            );
            trackers.push(tracker.clone());
            mangaupdates = Some(tracker);
        }

        // Simkl bridges through `mal_id`, which the mapping table already holds for MAL's sake —
        // so the third tracker cost an auth flow and a push call, exactly as the plan predicted a
        // second identity system would not be needed.
        if config.trackers.is_enabled("simkl")
            && let Some(client_id) = config.trackers.simkl.client_id.clone()
        {
            let tracker = Arc::new(
                kaeri_track::SimklTracker::new(
                    client_id,
                    http.plain().clone(),
                    Arc::new(StoreMapping { store: store.clone() }),
                    tokens.get_pair("simkl").ok(),
                )
                .with_client_secret(config.trackers.simkl.client_secret.clone())
                .with_token_store(tokens.clone()),
            );
            trackers.push(tracker.clone());
            simkl = Some(tracker);
        }

        // Trakt keys on TVDB, which the datasets cover far less completely — see the module docs.
        if config.trackers.is_enabled("trakt")
            && let Some(client_id) = config.trackers.trakt.client_id.clone()
        {
            trackers.push(Arc::new(kaeri_track::TraktTracker::new(
                client_id,
                http.plain().clone(),
                Arc::new(StoreMapping { store: store.clone() }),
                tokens.get_pair("trakt").ok(),
            )));
        }

        Self {
            trackers,
            anilist,
            mal,
            kitsu,
            kitsu_site: kaeri_track::kitsu::Site::default(),
            mangaupdates,
            mangaupdates_api: kaeri_track::mangaupdates::API.to_owned(),
            lookups: Arc::new(kaeri_track::lookup::Lookups::new(http.plain().clone())),
            simkl,
            tokens,
            store: store.clone(),
            config: config.clone(),
            http: http.clone(),
            pull_errors: Arc::default(),
            pulls: Arc::default(),
            library_shown: Arc::default(),
            pending: native::PendingSignIns::default(),
            device: device::DeviceFlows::default(),
            titles: None,
            latest_wins: Arc::new(std::sync::atomic::AtomicBool::new(
                config.reading.when_trackers_disagree
                    == kaeri_core::config::WhenTrackersDisagree::Latest,
            )),
        }
    }

    /// How reading progress settles when a tracker disagrees, as the person set it.
    pub fn read_rule(&self) -> kaeri_core::config::WhenTrackersDisagree {
        if self.latest_wins.load(std::sync::atomic::Ordering::Relaxed) {
            kaeri_core::config::WhenTrackersDisagree::Latest
        } else {
            kaeri_core::config::WhenTrackersDisagree::Furthest
        }
    }

    /// The setting changed: pulls from now on settle progress by `rule`.
    pub fn set_read_rule(&self, rule: kaeri_core::config::WhenTrackersDisagree) {
        let latest = rule == kaeri_core::config::WhenTrackersDisagree::Latest;
        self.latest_wins.store(latest, std::sync::atomic::Ordering::Relaxed);
    }

    /// Titles named as AniList's viewer reads them follow each pull's answer: every pull asks
    /// for the viewer with the list.
    #[must_use]
    pub fn with_titles(mut self, titles: kaeri_meta::titles::Titles) -> Self {
        self.titles = Some(titles);
        self
    }

    /// The AniList tracker asking against `base`'s budget: one for the tracker and the metadata
    /// source, as AniList counts requests per address and not per caller. Its token is kept, so a
    /// sign-in made through the earlier client still reaches it.
    #[must_use]
    pub fn on_budget_of(mut self, base: &kaeri_meta::anilist::AniList) -> Self {
        if let Some(at) = self.trackers.iter().position(|t| t.id() == "anilist")
            && let Some(old) = self.anilist.as_ref()
        {
            let tracker = Arc::new(old.on_budget_of(base));
            self.trackers[at] = tracker.clone();
            self.anilist = Some(tracker);
        }
        self
    }

    /// Kitsu at `site` in place of Kitsu itself, its sign-in and its tracker alike, signed in or
    /// not: a fixture build's fake, which no password typed into it leaves the machine for.
    #[must_use]
    pub fn with_kitsu_at(mut self, site: kaeri_track::kitsu::Site) -> Self {
        if let Some(at) = self.trackers.iter().position(|t| t.id() == "kitsu") {
            let pair = self.tokens.get_pair("kitsu").ok();
            let tracker =
                kitsu_tracker(&self.http, &self.store, &self.tokens, site.clone(), pair);
            self.trackers[at] = tracker.clone();
            self.kitsu = Some(tracker);
        }
        self.kitsu_site = site;
        self
    }

    /// MangaUpdates at `api` in place of the service, its sign-in and its tracker alike: a
    /// fixture build's fake, which no password typed into it leaves the machine for.
    #[must_use]
    pub fn with_mangaupdates_at(mut self, api: &str) -> Self {
        if let Some(at) = self.trackers.iter().position(|t| t.id() == "mangaupdates") {
            let pair = self.tokens.get_pair("mangaupdates").ok();
            let tracker = Arc::new(
                kaeri_track::MangaUpdatesTracker::new(
                    self.http.plain().clone(),
                    Arc::new(StoreReadMapping { store: self.store.clone() }),
                    pair,
                )
                .with_api(api)
                .with_token_store(self.tokens.clone()),
            );
            self.trackers[at] = tracker.clone();
            self.mangaupdates = Some(tracker);
        }
        self.mangaupdates_api = api.to_owned();
        self
    }

    /// The lookups answered by stand-ins: a test's.
    #[must_use]
    pub fn with_lookups(mut self, lookups: kaeri_track::lookup::Lookups) -> Self {
        self.lookups = Arc::new(lookups);
        self
    }

    fn pull_error(&self, tracker_id: &str) -> Option<PullError> {
        self.pull_errors.lock().ok()?.get(tracker_id).cloned()
    }

    fn note_pull(&self, tracker_id: &str, error: Option<PullError>) {
        if let Ok(mut errors) = self.pull_errors.lock() {
            match error {
                Some(error) => errors.insert(tracker_id.to_owned(), error),
                None => errors.remove(tracker_id),
            };
        }
    }

    /// Whether Accounts lists a tracker for what Kaeri is for: MangaUpdates only with reading,
    /// the watching-only trackers only with watching.
    pub fn shown_for(tracker: &dyn Tracker, answer: kaeri_model::medium::KaeriIsFor) -> bool {
        use kaeri_model::medium::KaeriIsFor as F;
        match answer {
            F::Both => true,
            F::Read => tracker.serves_reading(),
            F::Watch => tracker.serves_watching(),
        }
    }

    /// The initial state for each tracker, so the badge is right before any sync runs.
    pub fn initial_states(&self) -> Vec<SyncState> {
        self.states_shown(kaeri_model::medium::KaeriIsFor::Both)
    }

    /// The states of the trackers Accounts lists for this answer.
    pub fn states_shown(&self, answer: kaeri_model::medium::KaeriIsFor) -> Vec<SyncState> {
        self.trackers
            .iter()
            .filter(|tracker| Self::shown_for(tracker.as_ref(), answer))
            .map(|tracker| {
                let id = tracker.id();
                let storage = self.tokens.storage_for(id);
                SyncState {
                    tracker: id.to_owned(),
                    connected: tracker.is_authenticated(),
                    user: None,
                    storage: Some(storage.describe().to_owned()),
                    storage_degraded: storage.is_degraded(),
                    outbox: self.store.outbox_waiting(id).unwrap_or(0),
                    deferred: self.store.outbox_deferred(id).unwrap_or(0),
                    // A credential rejected before this launch is still dead: said from the
                    // first state, not after the first drain asks again.
                    needs_reauth: tracker.is_authenticated()
                        && sync::credential_rejected(&self.store, id),
                    last: None,
                    pull_error: self.pull_error(id),
                }
            })
            .collect()
    }

    fn state_for(
        &self,
        tracker: &dyn Tracker,
        needs_reauth: bool,
        last: Option<String>,
    ) -> SyncState {
        let id = tracker.id();
        let storage = self.tokens.storage_for(id);
        SyncState {
            tracker: id.to_owned(),
            connected: tracker.is_authenticated(),
            user: None,
            storage: Some(storage.describe().to_owned()),
            storage_degraded: storage.is_degraded(),
            outbox: self.store.outbox_waiting(id).unwrap_or(0),
            deferred: self.store.outbox_deferred(id).unwrap_or(0),
            needs_reauth: needs_reauth
                || (tracker.is_authenticated() && sync::credential_rejected(&self.store, id)),
            last,
            pull_error: self.pull_error(id),
        }
    }

    /// The state of every tracker a status or score was just queued for, with fresh depths.
    ///
    /// Used right after queueing something, so each `⇅` badge moves the instant you act rather
    /// than at the next drain tick. Signed-in trackers only, the set [`set_status`] queues for.
    /// With none there is nothing to report — a blank default state here once became a
    /// nameless row on Accounts and an empty chip in the header.
    pub fn state_after_enqueue(&self) -> Vec<SyncState> {
        self.trackers
            .iter()
            .filter(|t| t.is_authenticated() && t.serves_watching())
            .map(|t| self.state_for(t.as_ref(), false, Some("queued".into())))
            .collect()
    }

    /// The same after a reading change was queued: the trackers that keep a reading list.
    pub fn state_after_read_enqueue(&self) -> Vec<SyncState> {
        self.trackers
            .iter()
            .filter(|t| t.is_authenticated() && t.serves_reading())
            .map(|t| self.state_for(t.as_ref(), false, Some("queued".into())))
            .collect()
    }
}

/// The materialised ID mapping, as trackers that key on something other than AniList need it.
///
/// Reads both directions out of the `mapping` table the dataset refresh fills. A miss is `None`
/// rather than an error: the tracker turns that into a named failure, because a title silently not
/// syncing is the thing the mapping layer exists to prevent.
struct StoreMapping {
    store: Store,
}

impl StoreMapping {
    /// Episodes in the seasons before this AniList entry, as the datasets report it.
    ///
    /// Clamped at zero: `episode_offset` is stored signed and a negative value would mean "this
    /// entry starts before the series does", which is not a thing.
    fn offset_of(&self, anilist_id: AnilistId) -> u32 {
        self.store
            .mapping_for(anilist_id)
            .ok()
            .flatten()
            .and_then(|m| m.episode_offset)
            .unwrap_or(0)
            .max(0) as u32
    }
}

/// Trakt needs season-relative numbering, which is what the datasets' `episode_offset` is for.
///
/// This is the field Fribb's corpus carries and ThaUnknown's does not — the reason both are merged.
/// Where there is no offset the answer is season 1, which is right for the great majority of titles
/// and wrong in a knowable way for split cours.
impl kaeri_track::SeasonMapping for StoreMapping {
    fn tvdb_id(&self, anilist_id: AnilistId) -> Option<u32> {
        self.store.mapping_for(anilist_id).ok().flatten().and_then(|m| m.tvdb_id)
    }

    fn season_episode(&self, anilist_id: AnilistId, absolute: u32) -> Option<(u32, u32)> {
        let offset = self.offset_of(anilist_id);
        if offset > 0 {
            // An offset *is* the statement "this entry starts partway into a TVDB series", so it
            // both identifies a later season and gives the number to subtract. Saturating, because
            // an offset larger than the episode number would otherwise wrap.
            return Some((2, absolute.saturating_sub(offset).max(1)));
        }

        // No offset. That only means season one if this AniList entry is the *whole* TVDB series —
        // and measured against the real table, for 3,606 titles it is not. Refusing there is the
        // difference between skipping a push and writing S3E02 onto S1E02.
        let tvdb_id = self.tvdb_id(anilist_id)?;
        match self.store.anilist_entries_for_tvdb(tvdb_id) {
            Ok(1) => Some((1, absolute.max(1))),
            Ok(_) => None,
            // A failed lookup is treated as ambiguous. Guessing on a read error would be the worst
            // of both: wrong data, and no idea why.
            Err(e) => {
                tracing::warn!(error = %e, "could not check tvdb season ambiguity");
                None
            }
        }
    }

    fn absolute_episode(&self, anilist_id: AnilistId, season: u32, episode: u32) -> u32 {
        if season <= 1 {
            return episode;
        }
        episode + self.offset_of(anilist_id)
    }

    fn anilist_for_tvdb(&self, tvdb_id: u32) -> Option<AnilistId> {
        self.store.anilist_id_for_tvdb(tvdb_id).ok().flatten()
    }
}

impl kaeri_track::mal::IdMapping for StoreMapping {
    fn mal_id(&self, anilist_id: AnilistId) -> Option<u32> {
        self.store.mapping_for(anilist_id).ok().flatten().and_then(|m| m.mal_id)
    }

    fn anilist_id(&self, mal_id: u32) -> Option<AnilistId> {
        self.store.anilist_id_for_mal(mal_id).ok().flatten()
    }
}

/// The Kitsu tracker at `site`, on the store's id mapping and the shared tokens.
fn kitsu_tracker(
    http: &kaeri_net::HttpClient,
    store: &Store,
    tokens: &TokenStore,
    site: kaeri_track::kitsu::Site,
    pair: Option<kaeri_track::TokenPair>,
) -> Arc<kaeri_track::KitsuTracker> {
    Arc::new(
        kaeri_track::KitsuTracker::new(
            http.plain().clone(),
            Arc::new(StoreMapping { store: store.clone() }),
            pair,
        )
        .at(site)
        .with_read_mapping(Arc::new(StoreReadMapping { store: store.clone() }))
        .with_token_store(tokens.clone()),
    )
}

/// The reading side of the id mapping: a work's entry on each tracker, kept in `read_track_map`
/// and looked up the other way for a pull. A work is `a:{anilist id}`.
struct StoreReadMapping {
    store: Store,
}

impl StoreReadMapping {
    fn id(&self, anilist_id: AnilistId, tracker: &str) -> Option<String> {
        self.store.track_map_id(&sync::read_work(anilist_id), tracker).ok().flatten()
    }

    fn work(&self, tracker: &str, tracker_id: &str) -> Option<AnilistId> {
        let work = self.store.track_map_work(tracker, tracker_id).ok().flatten()?;
        work.strip_prefix("a:")?.parse().ok().map(AnilistId::new)
    }
}

impl kaeri_track::read::ReadMapping for StoreReadMapping {
    fn mal_id(&self, anilist_id: AnilistId) -> Option<u32> {
        self.id(anilist_id, "mal")?.parse().ok()
    }

    fn anilist_for_mal(&self, mal_id: u32) -> Option<AnilistId> {
        self.work("mal", &mal_id.to_string())
    }

    fn kitsu_id(&self, anilist_id: AnilistId) -> Option<u32> {
        self.id(anilist_id, "kitsu")?.parse().ok()
    }

    fn anilist_for_kitsu(&self, kitsu_id: u32) -> Option<AnilistId> {
        self.work("kitsu", &kitsu_id.to_string())
    }

    fn mangaupdates_id(&self, anilist_id: AnilistId) -> Option<u64> {
        self.id(anilist_id, "mangaupdates")?.parse().ok()
    }

    fn anilist_for_mangaupdates(&self, series_id: u64) -> Option<AnilistId> {
        self.work("mangaupdates", &series_id.to_string())
    }
}

impl kaeri_track::kitsu::IdMapping for StoreMapping {
    fn kitsu_id(&self, anilist_id: AnilistId) -> Option<u32> {
        self.store.mapping_for(anilist_id).ok().flatten().and_then(|m| m.kitsu_id)
    }

    fn anilist_id(&self, kitsu_id: u32) -> Option<AnilistId> {
        self.store.anilist_id_for_kitsu(kitsu_id).ok().flatten()
    }
}

/// Fetch one segment of the tracker's list for the Library screen.
///
/// Titles are remembered as they pass through, which is what later lets a sync conflict name the
/// show instead of printing a bare id.
pub async fn load_library(
    sync: &Sync,
    store: &Store,
    meta: &kaeri_meta::Meta,
    segment: LibrarySegment,
) -> Event {
    sync.library_shown.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut answers = library_answers(sync, store, meta, &[segment]).await;
    answers.pop().expect("one answer for each segment asked")
}

/// The Library's `segments`, each answered as [`load_library`] answers it, cut from one answer
/// of the tracker's.
///
/// A status set here and still on its way to the tracker counts over what the tracker's list
/// says: the list as it is once the outbox drains, and where the title page already has the
/// title. Without that, a title put on Planning stayed off the Planning list until the drain
/// sent it and the list was asked for again.
pub async fn library_answers(
    sync: &Sync,
    store: &Store,
    meta: &kaeri_meta::Meta,
    segments: &[LibrarySegment],
) -> Vec<Event> {
    use kaeri_model::catalog::ContentList;

    // Tagged with its segment: switching tabs while a pull is in flight must not show this one
    // under the next tab's heading.
    let every = |content: ContentList| -> Vec<Event> {
        segments
            .iter()
            .map(|&segment| Event::Content {
                view: ContentFor::Library(segment),
                content: content.clone(),
            })
            .collect()
    };

    let Some(tracker) = sync.trackers.first() else {
        return every(ContentList::Entries(Vec::new()));
    };
    if !tracker.is_authenticated() {
        // Not a failure. The Library screen renders its own "no account" state, which explains
        // that history works regardless.
        return every(ContentList::Entries(Vec::new()));
    }

    // The concrete handle, kept alongside the trait object rather than reached by downcasting:
    // the trait returns only the syncable projection, but this screen needs covers and titles
    // too, and fetching those per title would cost a request each against thirty a minute.
    let Some(anilist) = sync.anilist.as_ref() else {
        return every(ContentList::Entries(Vec::new()));
    };

    let entries = match anilist.library().await {
        Ok(entries) => entries,
        Err(kaeri_core::Error::Auth(_)) => {
            return every(ContentList::Failed("sign in again on the Accounts screen".into()));
        }
        Err(e) => return every(ContentList::Failed(e.to_string())),
    };

    let pending = store.pending_statuses(tracker.id()).unwrap_or_else(|e| {
        tracing::debug!(error = %e, "could not read the statuses on their way");
        Default::default()
    });
    let status_of = |id: AnilistId, said: &str| -> String {
        pending
            .get(&id)
            .map_or_else(|| said.to_owned(), |status| status_wire(*status).to_owned())
    };
    // Titles put on the list here that the tracker's list does not have yet, described from
    // what the cache holds and asked for otherwise.
    let mut added: Vec<AnilistId> = pending
        .keys()
        .filter(|id| !entries.iter().any(|e| e.media.id == **id))
        .copied()
        .collect();
    added.sort_unstable_by_key(|id| std::cmp::Reverse(id.get()));
    let mut added_media = meta.cached(&added);
    let uncached: Vec<_> =
        added.iter().filter(|id| !added_media.iter().any(|m| m.id == **id)).copied().collect();
    if !uncached.is_empty() {
        added_media.extend(meta.media_many(&uncached).await.unwrap_or_default());
    }

    // The whole list, every segment's, kept for the cards that say "Watched".
    let statuses: Vec<(AnilistId, String)> = entries
        .iter()
        .map(|e| (e.media.id, status_of(e.media.id, &e.status)))
        .chain(added.iter().map(|id| (*id, status_wire(pending[id]).to_owned())))
        .collect();
    let kept: Vec<_> = statuses.iter().map(|(id, status)| (*id, status.as_str())).collect();
    if let Err(e) = store.replace_library_statuses(tracker.id(), &kept, kaeri_store::now()) {
        tracing::debug!(error = %e, "could not keep the library's statuses");
    }

    let titles = meta.titles();
    segments
        .iter()
        .map(|&segment| {
            let wanted = segment.wire();
            let mut rows = Vec::new();
            for entry in entries.iter().filter(|e| status_of(e.media.id, &e.status) == wanted) {
                let _ = store.remember_title(entry.media.id, titles.pick(&entry.media.title));
                let mut row = crate::data::entry_from(&entry.media, Some(store));
                // The tracker's progress, not the local projection: this screen is a view of
                // the remote list, and showing local numbers here would hide a divergence
                // rather than reveal it.
                row.progress = Some(TitleProgress {
                    done: entry.progress,
                    next: next_after_progress(entry.progress, row.episodes),
                });
                // The one place the tracker's status for a title is known, so `a` opens on it.
                row.list_status = Some(segment);
                row.recount();
                rows.push(row);
            }
            for media in added_media.iter().filter(|m| status_wire(pending[&m.id]) == wanted) {
                let _ = store.remember_title(media.id, titles.pick(&media.title));
                let mut row = crate::data::entry_from(media, Some(store));
                // What the tracker will say once the outbox drains: the progress queued for
                // it, and none when nothing is.
                let done =
                    store.pending_progress(tracker.id(), media.id).ok().flatten().unwrap_or(0);
                row.progress =
                    Some(TitleProgress { done, next: next_after_progress(done, row.episodes) });
                row.list_status = Some(segment);
                row.recount();
                rows.push(row);
            }
            Event::Content {
                view: ContentFor::Library(segment),
                content: ContentList::Entries(rows),
            }
        })
        .collect()
}

/// A status as AniList words it, the way the Library's lists and the kept statuses say it.
fn status_wire(status: kaeri_core::traits::WatchStatus) -> &'static str {
    use kaeri_core::traits::WatchStatus as W;
    match status {
        W::Current => "CURRENT",
        // Kept apart, as AniList's own list says it: keep next follows a rewatch from the
        // episode rewatched, not from the last one ever watched.
        W::Repeating => "REPEATING",
        W::Planning => "PLANNING",
        W::Completed => "COMPLETED",
        W::Paused => "PAUSED",
        W::Dropped => "DROPPED",
    }
}

/// The episode after `progress` on the tracker's list. A finished show with a known count
/// starts over at 1; an unknown count offers the one after the last watched.
fn next_after_progress(progress: u32, total: Option<u32>) -> u32 {
    match total {
        Some(total) if total > 0 && progress >= total => 1,
        _ => progress.saturating_add(1),
    }
}

/// Start the background drain and pull loops.
///
/// Both run forever. The drain is cheap when idle — one indexed count against SQLite — so it can
/// tick often; a pull costs a real request out of thirty a minute, so it does not.
/// Keep where each title is on `tracker`'s list, as the tracker's words for it, in place of
/// what was kept: see `kaeri_store::library_status`.
fn keep_statuses(
    store: &Store,
    tracker: &str,
    remote: &[kaeri_core::traits::TrackedEntry],
    now: i64,
) {
    // A status set here and not sent yet counts over the list's, as the Library shows it.
    let pending = store.pending_statuses(tracker).unwrap_or_default();
    let statuses: Vec<_> = remote
        .iter()
        .map(|entry| (entry.anilist_id, status_wire(entry.status)))
        .chain(pending.iter().map(|(id, status)| (*id, status_wire(*status))))
        .collect();
    if let Err(e) = store.replace_library_statuses(tracker, &statuses, now) {
        tracing::debug!(error = %e, "could not keep the library's statuses");
    }
}

/// A pull this recent answers Sync now without another.
pub const PULL_COALESCE: std::time::Duration = std::time::Duration::from_secs(30);

/// Whether the app is on screen, when the library was last pulled, and the wake-up for a return.
#[derive(Debug, Default)]
pub struct PullGate {
    away: std::sync::atomic::AtomicBool,
    last: std::sync::Mutex<Option<std::time::Instant>>,
    back: tokio::sync::Notify,
}

impl PullGate {
    /// The app went off screen or came back; a return wakes the loop for one pull.
    pub fn set_away(&self, away: bool) {
        use std::sync::atomic::Ordering;
        let was = self.away.swap(away, Ordering::SeqCst);
        if was && !away {
            self.back.notify_one();
        }
    }

    fn is_away(&self) -> bool {
        self.away.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn mark(&self) {
        if let Ok(mut last) = self.last.lock() {
            *last = Some(std::time::Instant::now());
        }
    }

    /// Whether a pull ran within [`PULL_COALESCE`].
    pub fn recent(&self) -> bool {
        self.last
            .lock()
            .ok()
            .and_then(|last| *last)
            .is_some_and(|at| at.elapsed() < PULL_COALESCE)
    }
}

/// The drain and the pull, each on its tick. The launch's pull waits for the launch's screens,
/// `launch` being the AniList client whose launch they are, and goes in the background queue.
pub fn spawn_loops(sync: Sync, launch: kaeri_meta::AniList, tx: Outbox) {
    if sync.trackers.is_empty() {
        return;
    }

    let drain_every =
        std::time::Duration::from_secs(sync.config.trackers.drain_interval_secs.max(5));
    let pull_every =
        std::time::Duration::from_secs(sync.config.trackers.pull_interval_secs.max(60));

    {
        let sync = sync.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(drain_every);
            loop {
                ticker.tick().await;
                // Nobody waits on these: they leave the last of the AniList budget to Search.
                kaeri_meta::in_background(drain_once(&sync, &tx)).await;
            }
        });
    }

    tokio::spawn(async move {
        // A pull at startup is what makes "watched two on my phone" show up when you open the
        // app, so launch always pulls: once Home's first screen is in.
        launch.launch_over().await;
        kaeri_meta::in_background(pull_once(&sync, &tx)).await;
        loop {
            let gate = sync.pulls.clone();
            let returned = tokio::select! {
                () = tokio::time::sleep(pull_every) => false,
                () = gate.back.notified() => true,
            };
            // Hidden or in the background the loop rests: the return pulls once instead.
            if gate.is_away() || (returned && gate.recent()) {
                continue;
            }
            kaeri_meta::in_background(pull_once(&sync, &tx)).await;
        }
    });
}

/// Write back any token a tracker renewed during a call and could not store itself.
///
/// MAL's tokens last about a month and refresh silently mid-call; without this the renewed pair
/// would live only in memory, and the next process would present the expired one — refreshing on
/// every single run until the *refresh* token itself expired. A tracker given the store writes
/// its renewed pair at once, so what reaches here is a write that failed then, tried again.
async fn persist_renewed_tokens(sync: &Sync) {
    // Taken rather than peeked, so a write failure does not silently discard it: the tracker keeps
    // the in-memory copy and offers it again on the next pass.
    if let Some(mal) = sync.mal.as_ref()
        && let Some(pair) = mal.take_renewed().await
    {
        match sync.tokens.set_pair(
            "mal",
            &pair.access,
            pair.refresh.as_deref(),
            pair.expires_at,
        ) {
            Ok(_) => tracing::info!("stored the renewed myanimelist token"),
            Err(e) => tracing::warn!(error = %e, "could not store the renewed token"),
        }
    }
    if let Some(kitsu) = sync.kitsu.as_ref()
        && let Some(pair) = kitsu.take_renewed().await
    {
        match sync.tokens.set_pair(
            "kitsu",
            &pair.access,
            pair.refresh.as_deref(),
            pair.expires_at,
        ) {
            Ok(_) => tracing::info!("stored the renewed kitsu token"),
            Err(e) => tracing::warn!(error = %e, "could not store the renewed kitsu token"),
        }
    }
    if let Some(simkl) = sync.simkl.as_ref()
        && let Some(pair) = simkl.take_renewed().await
    {
        match sync.tokens.set_pair(
            "simkl",
            &pair.access,
            pair.refresh.as_deref(),
            pair.expires_at,
        ) {
            Ok(_) => tracing::info!("stored the renewed simkl token"),
            Err(e) => tracing::warn!(error = %e, "could not store the renewed simkl token"),
        }
    }
}

/// A tracker's name as a person knows it. An id it has no name for is shown as it is.
pub(crate) fn tracker_name(id: &str) -> String {
    match id {
        "anilist" => "AniList",
        "mal" => "MyAnimeList",
        "kitsu" => "Kitsu",
        "mangaupdates" => "MangaUpdates",
        "simkl" => "Simkl",
        "trakt" => "Trakt",
        other => other,
    }
    .to_owned()
}

/// A finished sign-in. A token kept in a file rather than the keychain is said, not hidden.
fn signed_in(tracker_id: &str, token_in_file: bool) -> Notice {
    let tracker = tracker_name(tracker_id);
    Notice::info(if token_in_file {
        Msg::NoticeSignedInFile { tracker }
    } else {
        Msg::NoticeSignedInKeychain { tracker }
    })
}

fn sign_in_failed(tracker_id: &str, reason: &dyn std::fmt::Display) -> Notice {
    let (tracker, reason) = (tracker_name(tracker_id), reason.to_string());
    Notice::alert(Msg::NoticeSignInFailed { tracker, reason })
}

#[cfg(feature = "desktop")]
fn sign_in_stopped(tracker_id: &str, reason: &dyn std::fmt::Display) -> Notice {
    let (tracker, reason) = (tracker_name(tracker_id), reason.to_string());
    Notice::alert(Msg::NoticeSignInStopped { tracker, reason })
}

#[cfg(feature = "desktop")]
fn token_exchange_failed(tracker_id: &str, reason: &dyn std::fmt::Display) -> Notice {
    let (tracker, reason) = (tracker_name(tracker_id), reason.to_string());
    Notice::alert(Msg::NoticeTokenExchangeFailed { tracker, reason })
}

fn token_not_saved(reason: &dyn std::fmt::Display) -> Notice {
    Notice::alert(Msg::NoticeTokenNotSaved { reason: reason.to_string() })
}

#[cfg(feature = "desktop")]
fn browser_not_opened(reason: &dyn std::fmt::Display) -> Notice {
    Notice::alert(Msg::NoticeBrowserNotOpened { reason: reason.to_string() })
}

/// One drain pass across every tracker.
pub async fn drain_once(sync: &Sync, tx: &Outbox) {
    let now = kaeri_store::now();
    persist_renewed_tokens(sync).await;
    // Every tracker at once. Each drains only its own outbox rows to its own host, so nothing
    // is shared between them, and a progress update reaches all of them in the time the
    // slowest takes rather than the sum.
    let drains = sync.trackers.iter().map(|tracker| async move {
        drain_reading(sync, tracker.as_ref(), now, tx).await;
        if !tracker.serves_watching() {
            return;
        }
        match sync::drain(&sync.store, tracker.as_ref(), now).await {
            Ok(report) => {
                let last = if report.ignored > 0 {
                    // Said rather than folded into "sent": the tracker answered for these
                    // titles and changed nothing.
                    Some(format!("sent {} · {} not recognised", report.sent, report.ignored))
                } else if report.sent > 0 {
                    Some(format!("sent {}", report.sent))
                } else if report.failed > 0 {
                    Some("retrying".to_string())
                } else {
                    None
                };
                // Silent when nothing happened: a badge that changes on every idle tick is
                // noise, and there is no news to deliver.
                if !report.did_nothing() {
                    let _ = tx.send(Event::Sync(Box::new(sync.state_for(
                        tracker.as_ref(),
                        report.needs_reauth,
                        last,
                    ))));
                }
            }
            Err(e) => tracing::warn!(tracker = tracker.id(), error = %e, "drain failed"),
        }
    });
    futures::future::join_all(drains).await;
}

/// A tracker's reading outbox, drained to its manga and novel list.
async fn drain_reading(sync: &Sync, tracker: &dyn Tracker, now: i64, tx: &Outbox) {
    if !tracker.serves_reading() {
        return;
    }
    match sync::drain_read(&sync.store, tracker, now).await {
        Ok(report) => {
            if !report.did_nothing() {
                let last = (report.sent > 0).then(|| format!("sent {}", report.sent));
                let state = sync.state_for(tracker, report.needs_reauth, last);
                let _ = tx.send(Event::Sync(Box::new(state)));
            }
        }
        Err(e) => tracing::warn!(tracker = tracker.id(), error = %e, "reading drain failed"),
    }
}

/// A tracker's manga and novel list, pulled and merged with what was read here.
async fn pull_reading(sync: &Sync, tracker: &dyn Tracker, now: i64, tx: &Outbox) {
    if !tracker.serves_reading() || !tracker.is_authenticated() {
        return;
    }
    let last_pull = sync
        .store
        .get_meta_i64(&format!("{}:read", pull_key(tracker.id())))
        .unwrap_or(None)
        .unwrap_or(0);
    match sync::pull_read(&sync.store, tracker, now, last_pull, sync.read_rule()).await {
        Ok(report) => {
            let _ = sync.store.set_meta_i64(&format!("{}:read", pull_key(tracker.id())), now);
            sync.note_pull(tracker.id(), None);
            if !report.conflicts.is_empty() {
                let rows = report
                    .conflicts
                    .iter()
                    .map(|c| ConflictRow {
                        anilist_id: c.anilist_id,
                        title: sync
                            .store
                            .cached_title(c.anilist_id)
                            .unwrap_or(None)
                            .unwrap_or_else(|| format!("anilist {}", c.anilist_id.get())),
                        field: c.field.label().to_owned(),
                        local: c.local.clone(),
                        remote: c.remote.clone(),
                    })
                    .collect();
                let _ = tx.send(Event::Conflicts(rows));
            }
            for conflict in &report.conflicts {
                crate::read_tracking::note_conflict(
                    &sync.store,
                    conflict.anilist_id,
                    tracker.id(),
                );
            }
            let state = sync.state_for(tracker, false, Some("pulled".into()));
            let _ = tx.send(Event::Sync(Box::new(state)));
        }
        Err(kaeri_core::Error::Auth(_)) => {
            let state = sync.state_for(tracker, true, Some("sign in again".into()));
            let _ = tx.send(Event::Sync(Box::new(state)));
        }
        Err(e) => {
            tracing::warn!(tracker = tracker.id(), error = %e, "reading pull failed");
            sync.note_pull(tracker.id(), Some(PullError { reason: e.to_string(), at: now }));
            let _ = tx.send(Event::Sync(Box::new(sync.state_for(tracker, false, None))));
        }
    }
}

/// One library pull across every tracker.
pub async fn pull_once(sync: &Sync, tx: &Outbox) {
    let now = kaeri_store::now();
    sync.pulls.mark();
    // The libraries are fetched at once — separate hosts, and a pull can be several pages —
    // but merged one at a time in tracker order. Every merge reads and writes the same local
    // history, so merging in arrival order would let network timing decide which tracker's
    // disagreement is reported as the conflict.
    let pulling: Vec<_> =
        sync.trackers.iter().filter(|t| t.is_authenticated() && t.serves_watching()).collect();
    let libraries = futures::future::join_all(pulling.iter().map(|t| t.pull_library())).await;
    // AniList's answer carries the viewer, whose title language may have changed since.
    if let (Some(titles), Some(anilist)) = (&sync.titles, &sync.anilist)
        && anilist.is_authenticated()
        && let Some(language) = anilist.answered_title_language().await
        && titles.viewer_reads(language)
    {
        let _ = tx.send(Event::TitlesChanged(crate::data::title_language(titles.language())));
    }

    for (tracker, library) in pulling.into_iter().zip(libraries) {
        let last_pull =
            sync.store.get_meta_i64(&pull_key(tracker.id())).unwrap_or(None).unwrap_or(0);
        let reconciled = library.and_then(|remote| {
            // The Library's own tracker's list, kept for the cards that say "Watched".
            if sync.trackers.first().is_some_and(|first| first.id() == tracker.id()) {
                keep_statuses(&sync.store, tracker.id(), &remote, now);
            }
            sync::reconcile(&sync.store, tracker.id(), &remote, now, last_pull)
        });

        match reconciled {
            Ok(report) => {
                sync.note_pull(tracker.id(), None);
                let _ = sync.store.set_meta_i64(&pull_key(tracker.id()), now);
                let _ = sync::credential_accepted(&sync.store, tracker.id());
                // Said after every pull that went through, changed or not: the Accounts row's
                // "Synced" time and Sync now's answer come from it, and a pull can move the
                // list's statuses or the outbox without adopting a title.
                let _ = tx.send(Event::Sync(Box::new(sync.state_for(
                    tracker.as_ref(),
                    false,
                    Some(format!(
                        "pulled {}",
                        kaeri_model::wording::plural(report.seen as usize, "title")
                    )),
                ))));

                // Conflicts are titles, so they need names — which means resolving each id
                // against what we already have locally rather than making more requests.
                if !report.conflicts.is_empty() {
                    let rows = report
                        .conflicts
                        .iter()
                        .map(|c| ConflictRow {
                            anilist_id: c.anilist_id,
                            title: sync
                                .store
                                .cached_title(c.anilist_id)
                                .unwrap_or(None)
                                .unwrap_or_else(|| format!("anilist {}", c.anilist_id.get())),
                            field: c.field.label().to_owned(),
                            local: c.local.clone(),
                            remote: c.remote.clone(),
                        })
                        .collect();
                    let _ = tx.send(Event::Conflicts(rows));
                }
            }
            Err(kaeri_core::Error::Auth(message)) => {
                // Held as a rejected push is, so the drain stops presenting the same credential
                // and the next launch still says to sign in again.
                let _ = sync::note_rejected(&sync.store, tracker.id(), now);
                tracing::warn!(
                    tracker = tracker.id(),
                    %message,
                    "the tracker rejected the credential on a library pull"
                );
                let _ = tx.send(Event::Sync(Box::new(sync.state_for(
                    tracker.as_ref(),
                    true,
                    Some("sign in again".into()),
                ))));
            }
            // A tracker outage is expected and must not produce a toast every interval. The
            // Accounts row carries it, with when, until a pull succeeds.
            Err(e) => {
                tracing::warn!(tracker = tracker.id(), error = %e, "library pull failed");
                sync.note_pull(
                    tracker.id(),
                    Some(PullError { reason: e.to_string(), at: now }),
                );
                let _ = tx.send(Event::Sync(Box::new(sync.state_for(
                    tracker.as_ref(),
                    false,
                    None,
                ))));
            }
        }
    }

    for tracker in &sync.trackers {
        pull_reading(sync, tracker.as_ref(), now, tx).await;
    }
}

/// Run a tracker's sign-in flow, then report the result.
///
/// Opens a browser and waits. Everything here is best-effort and every failure is reported as a
/// toast rather than propagated — a failed sign-in must leave the app exactly as usable as it
/// was, because local history never needed an account.
pub async fn connect(
    sync: Sync,
    host: Arc<dyn Host>,
    clients: crate::options::AppClients,
    tracker_id: String,
    job: Option<&JobGuard>,
    tx: Outbox,
) {
    // An app's own client signs in through the system's sheet, on the app's own scheme.
    if native::begin(&sync, &clients, &tracker_id, &tx) {
        return;
    }
    // Simkl and Trakt use the device flow: no redirect URI to register, and it works over SSH.
    if let Some(endpoints) = device_endpoints(&tracker_id) {
        connect_device(sync, host, tracker_id, endpoints, tx).await;
        return;
    }
    if tracker_id == "mal" {
        connect_mal(sync, host, job, tx).await;
        return;
    }
    // Kitsu never reaches here: the Accounts screen opens its credential prompt directly
    // and dispatches `ConnectKitsu`. This is the fallback for any other path that still
    // says `Connect { "kitsu" }`, and it points at the screen that does it right.
    if tracker_id == "kitsu" {
        let notice = Notice::info(Msg::NoticeKitsuSignsInOnAccounts);
        let _ = tx.send(Event::Toast(notice.with_hint(NoticeHint::KitsuCredentials)));
        return;
    }
    if tracker_id != "anilist" {
        let tracker = tracker_name(&tracker_id);
        let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeSignInUnsupported { tracker })));
        return;
    }
    connect_anilist(sync, host, job, tx).await;
}

/// AniList and MyAnimeList sign in through a redirect to a listener on this machine, which only
/// the desktop has: an app on a phone takes the redirect itself, through a scheme of its own.
/// So without the `desktop` feature their sign-in says so, rather than opening a browser onto
/// a sign-in that cannot finish.
#[cfg(not(feature = "desktop"))]
async fn connect_anilist(_: Sync, _: Arc<dyn Host>, _: Option<&JobGuard>, tx: Outbox) {
    refuse_redirect("anilist", &tx);
}

/// See [`connect_anilist`].
#[cfg(not(feature = "desktop"))]
async fn connect_mal(_: Sync, _: Arc<dyn Host>, _: Option<&JobGuard>, tx: Outbox) {
    refuse_redirect("mal", &tx);
}

#[cfg(not(feature = "desktop"))]
fn refuse_redirect(tracker_id: &str, tx: &Outbox) {
    let tracker = tracker_name(tracker_id);
    let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeSignInNeedsRedirect { tracker })));
}

/// Sign in to AniList: the authorization code, caught on the loopback redirect, exchanged for
/// a token.
#[cfg(feature = "desktop")]
async fn connect_anilist(sync: Sync, host: Arc<dyn Host>, job: Option<&JobGuard>, tx: Outbox) {
    let tracker_id = "anilist";
    let auth_config = &sync.config.trackers.anilist;
    // A token arriving in a fragment never reaches a loopback port.
    if auth_config.flow == "implicit" {
        let setting = "trackers.anilist.flow".to_owned();
        let _ =
            tx.send(Event::Toast(Notice::alert(Msg::NoticeAnilistImplicitFlow { setting })));
        return;
    }
    let blank = |field: &Option<String>| field.as_deref().unwrap_or("").trim().is_empty();
    if blank(&auth_config.client_id) || blank(&auth_config.client_secret) {
        // The terminal's grant is the authorization code, which needs both halves.
        let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeAnilistClientMissing {
            client: "trackers.anilist.client_id".to_owned(),
            secret: "trackers.anilist.client_secret".to_owned(),
        })));
        return;
    }
    let (client_id, client_secret) = (
        auth_config.client_id.clone().unwrap_or_default(),
        auth_config.client_secret.clone().unwrap_or_default(),
    );
    let flow =
        if auth_config.flow == "paste" { auth::Flow::Paste } else { auth::Flow::Loopback };
    let port = auth_config.redirect_port;

    let url = match auth::authorize_url(&client_id, flow, port) {
        Ok(url) => url,
        Err(e) => {
            let _ = tx.send(Event::Toast(Notice::alert(Msg::verbatim(e.to_string()))));
            return;
        }
    };

    // Bind *before* opening the browser, or a fast redirect could arrive before the socket
    // exists. Only the wait is spawned: a spawned bind may not have run by the time the browser opens.
    let listening = if matches!(flow, auth::Flow::Loopback) {
        match auth::listen(port).await {
            Ok(loopback) => Some(tokio::spawn(loopback.wait_for_code("AniList"))),
            Err(e) => {
                let _ = tx.send(Event::Toast(sign_in_failed(tracker_id, &e)));
                return;
            }
        }
    } else {
        None
    };

    if let Err(e) = host.open_url(&url) {
        let _ = tx.send(Event::Toast(browser_not_opened(&e)));
        tracing::info!(%url, "open this to sign in");
    }

    if let (Some(job), Some(_)) = (job, &listening) {
        job.note(Msg::StatusWaitingForRedirect { tracker: tracker_name(tracker_id) }, None);
    }
    let Some(listening) = listening else {
        // The paste flow needs a terminal to paste into, which the TUI is currently occupying.
        let _ = tx.send(Event::Toast(Notice::info(Msg::NoticeAnilistPasteFlow)));
        return;
    };

    let code = match listening.await {
        Ok(Ok(code)) => code,
        Ok(Err(e)) => {
            let _ = tx.send(Event::Toast(sign_in_failed(tracker_id, &e)));
            return;
        }
        Err(e) => {
            let _ = tx.send(Event::Toast(sign_in_stopped(tracker_id, &e)));
            return;
        }
    };

    // The code is single-use and short-lived, so it is exchanged immediately rather than stored.
    let token = match auth::exchange_code(
        sync.http.plain(),
        &client_id,
        &client_secret,
        &auth::redirect_uri(port),
        &code,
    )
    .await
    {
        Ok(token) => token,
        Err(e) => {
            let _ = tx.send(Event::Toast(token_exchange_failed(tracker_id, &e)));
            return;
        }
    };

    match sync.tokens.set(tracker_id, &token) {
        Ok(storage) => {
            // A token in a plain file is said, not hidden: someone on a shared machine should know.
            let _ = tx.send(Event::Toast(signed_in(tracker_id, storage.is_degraded())));
            // This used to say "restart to start syncing", which was true and terrible: the token
            // was in the keychain and nothing running could see it. Handing it to the live tracker
            // is the whole fix.
            let pair = kaeri_track::TokenPair {
                access: token.clone(),
                refresh: None,
                expires_at: auth::token_expiry(&token),
            };
            activate(&sync, tracker_id, &pair, &tx).await;
        }
        Err(e) => {
            let _ = tx.send(Event::Toast(token_not_saved(&e)));
        }
    }
}

/// Sign in to Kitsu from inside the app.
///
/// The password arrives from the prompt, goes to Kitsu's token endpoint, and is dropped
/// when this function returns — the same one-way journey `--login` gives it. Success
/// hands the pair to the live tracker through the same [`activate`] every flow uses.
pub async fn connect_kitsu(sync: Sync, username: String, password: String, tx: Outbox) {
    let pair = match kaeri_track::kitsu::login(
        sync.http.plain(),
        &sync.kitsu_site,
        &username,
        &password,
        kaeri_store::now(),
    )
    .await
    {
        Ok(pair) => pair,
        Err(e) => {
            let _ = tx.send(Event::Toast(sign_in_failed("kitsu", &e)));
            return;
        }
    };

    match sync.tokens.set_pair("kitsu", &pair.access, pair.refresh.as_deref(), pair.expires_at)
    {
        Ok(storage) => {
            // A token in a plain file is said, not hidden: someone on a shared machine should know.
            let _ = tx.send(Event::Toast(signed_in("kitsu", storage.is_degraded())));
            activate(&sync, "kitsu", &pair, &tx).await;
        }
        Err(e) => {
            let _ = tx.send(Event::Toast(token_not_saved(&e)));
        }
    }
}

/// Sign in to MangaUpdates from inside the app, with the name and password from the form.
///
/// The password goes to the login call once and is dropped when this returns, as Kitsu's does.
pub async fn connect_mangaupdates(sync: Sync, username: String, password: String, tx: Outbox) {
    let pair = match kaeri_track::mangaupdates::login(
        sync.http.plain(),
        &sync.mangaupdates_api,
        &username,
        &password,
    )
    .await
    {
        Ok(pair) => pair,
        Err(e) => {
            let _ = tx.send(Event::Toast(sign_in_failed("mangaupdates", &e)));
            return;
        }
    };
    match sync.tokens.set_pair(
        "mangaupdates",
        &pair.access,
        pair.refresh.as_deref(),
        pair.expires_at,
    ) {
        Ok(storage) => {
            let _ = tx.send(Event::Toast(signed_in("mangaupdates", storage.is_degraded())));
            activate(&sync, "mangaupdates", &pair, &tx).await;
        }
        Err(e) => {
            let _ = tx.send(Event::Toast(token_not_saved(&e)));
        }
    }
}

/// Sign in to MyAnimeList from inside the app.
///
/// There was no interactive flow at all — the Accounts screen answered "mal: no sign-in flow yet"
/// while `--login --tracker mal` worked perfectly from the command line. The PKCE machinery already
/// existed; only this wiring was missing.
#[cfg(feature = "desktop")]
async fn connect_mal(sync: Sync, host: Arc<dyn Host>, job: Option<&JobGuard>, tx: Outbox) {
    use kaeri_track::mal;

    let auth = &sync.config.trackers.mal;
    let Some(client_id) = auth.client_id.clone().filter(|id| !id.trim().is_empty()) else {
        let setting = "trackers.mal.client_id".to_owned();
        let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeMalClientMissing { setting })));
        return;
    };

    // The verifier has to survive until the exchange, so it is made before the browser opens and
    // held across the wait. MAL supports only `plain`, so it doubles as the challenge.
    let pkce = mal::Pkce::generate();
    let redirect = auth::redirect_uri(auth.redirect_port);
    let url = mal::authorize_url(&client_id, &pkce, &redirect);

    // Bind before the browser opens, or a fast redirect arrives before the socket does. Only the
    // wait is spawned: a spawned bind may not have run by then.
    let loopback = match auth::listen(auth.redirect_port).await {
        Ok(loopback) => loopback,
        Err(e) => {
            let _ = tx.send(Event::Toast(Notice::alert(Msg::verbatim(e.to_string()))));
            return;
        }
    };
    let waiting = tokio::spawn(loopback.wait_for_code("MyAnimeList"));
    if let Err(e) = host.open_url(&url) {
        let _ = tx.send(Event::Toast(browser_not_opened(&e)));
        tracing::info!(%url, "open this to sign in");
    }
    // On the sign-in's row in the activity list, for as long as the browser has it.
    if let Some(job) = job {
        job.note(Msg::StatusWaitingForRedirect { tracker: tracker_name("mal") }, None);
    }

    let code = match waiting.await {
        Ok(Ok(code)) => code,
        Ok(Err(e)) => {
            let _ = tx.send(Event::Toast(Notice::alert(Msg::verbatim(e.to_string()))));
            return;
        }
        Err(e) => {
            let _ = tx.send(Event::Toast(sign_in_stopped("mal", &e)));
            return;
        }
    };

    let pair = match mal::exchange_code(
        sync.http.plain(),
        &client_id,
        &code,
        &pkce.verifier,
        &redirect,
        kaeri_store::now(),
    )
    .await
    {
        Ok(pair) => pair,
        Err(e) => {
            let _ = tx.send(Event::Toast(token_exchange_failed("mal", &e)));
            return;
        }
    };

    match sync.tokens.set_pair("mal", &pair.access, pair.refresh.as_deref(), pair.expires_at) {
        Ok(storage) => {
            let _ = tx.send(Event::Toast(signed_in("mal", storage.is_degraded())));
            if pair.refresh.is_none() {
                // MAL's tokens last 30 days, so no refresh token means sync stops in a month with
                // nothing but a re-sign-in to fix it. Worth saying at the time.
                let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeMalNoRefreshToken)));
            }
            activate(&sync, "mal", &pair, &tx).await;
        }
        Err(e) => {
            let _ = tx.send(Event::Toast(token_not_saved(&e)));
        }
    }
}

/// Make a sign-in take effect immediately, everywhere.
///
/// Three things have to happen and all three were missing from at least one flow: the *live* tracker
/// has to be handed the credential (it captured one at construction, which is why signing in used to
/// end with "restart to start syncing"), the badge and Accounts screen have to be rebuilt from what
/// is now true, and a first pull has to be kicked off so the Library is not empty until the next
/// interval.
pub(crate) async fn activate(
    sync: &Sync,
    tracker_id: &str,
    pair: &kaeri_track::TokenPair,
    tx: &Outbox,
) {
    if let Some(tracker) = sync.trackers.iter().find(|t| t.id() == tracker_id) {
        tracker
            .accept_credentials(&pair.access, pair.refresh.as_deref(), pair.expires_at)
            .await;
    }
    // The queue is held while a credential is known to be dead, and this is the moment it
    // stops being dead — without this the user signs in and then waits out a hold they have
    // already resolved.
    let _ = kaeri_track::sync::clear_reauth_hold(&sync.store, tracker_id);
    // Possibly another account: what was sent to the last one says nothing about this one.
    kaeri_track::sync::forget_pushed(&sync.store, tracker_id);
    kaeri_track::sync::forget_pushed_read(&sync.store, tracker_id);
    for state in sync.initial_states() {
        let _ = tx.send(Event::Sync(Box::new(state)));
    }
    // A pull now rather than at the next interval: having just signed in, an empty Library reads as
    // a failed sign-in.
    pull_once(sync, tx).await;
}

/// The device-flow endpoints for a tracker, if it uses one.
fn device_endpoints(tracker_id: &str) -> Option<kaeri_track::DeviceEndpoints> {
    match tracker_id {
        "simkl" => Some(kaeri_track::device::SIMKL),
        "trakt" => Some(kaeri_track::device::TRAKT),
        _ => None,
    }
}

/// Sign in with the OAuth device flow.
///
/// The user's part is typing a five-character code into a web page, so the code is the *only* thing
/// worth putting in front of them — it goes in the status line where it stays put, not in a toast
/// that fades after nine seconds while they are still reaching for the keyboard.
async fn connect_device(
    sync: Sync,
    host: Arc<dyn Host>,
    tracker_id: String,
    endpoints: kaeri_track::DeviceEndpoints,
    tx: Outbox,
) {
    let auth = match tracker_id.as_str() {
        "simkl" => &sync.config.trackers.simkl,
        _ => &sync.config.trackers.trakt,
    };
    let (client_id, client_secret) = (auth.client_id.clone(), auth.client_secret.clone());
    let Some(client_id) = client_id.filter(|id| !id.trim().is_empty()) else {
        let setting = format!("trackers.{tracker_id}.client_id");
        let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeClientIdMissing { setting })));
        return;
    };

    // Replacing the code on screen, if one is: see `device`.
    let flow = sync.device.begin(&tracker_id);
    let http = sync.http.plain().clone();
    let asked = flow.run(kaeri_track::device::request_code(&http, &endpoints, &client_id));
    let code = match asked.await {
        // Stopped: what stopped it says what follows.
        None => return,
        Some(Ok(code)) => code,
        // Its own sentence, naming the service and saying what it said: not its JSON.
        Some(Err(e)) => {
            let _ = tx.send(Event::Toast(Notice::alert(Msg::verbatim(e.to_string()))));
            return;
        }
    };

    // Its own persistent prompt on the Accounts screen, not the status line. The status line was
    // wrong twice over: it is drawn in the dimmest role in the palette, and any background task
    // setting a status — a rate-limit notice, a finished download — would wipe the one thing the
    // user has to read. Reported as the code never being displayed, which it effectively was not.
    // With when it stops working, for the TV's countdown, which asks for a new code then.
    let _ = tx.send(Event::DeviceCode(Some(device::prompt(
        &tracker_id,
        &code.user_code,
        &code.verification_url,
        code.expires_in,
        crate::now_epoch(),
    ))));
    if let Err(e) = host.open_url(&code.verification_url) {
        tracing::info!(url = %code.verification_url, error = %e, "open this to approve");
    }

    let polled = flow
        .run(kaeri_track::device::poll_for_token(
            &http,
            &client_id,
            client_secret.as_deref(),
            &code,
        ))
        .await;
    // A new code asked for, or the sign-in closed: the code on screen is the next flow's, or
    // nothing, and this one has nothing to say over it.
    let Some(polled) = polled else {
        tracing::info!(tracker = %tracker_id, "device sign-in stopped");
        return;
    };
    match polled {
        Ok(pair) => {
            let storage = sync.tokens.set_pair(
                &tracker_id,
                &pair.access,
                pair.refresh.as_deref(),
                pair.expires_at,
            );
            let notice = match storage {
                Ok(storage) => signed_in(&tracker_id, storage.is_degraded()),
                // Signed in but unable to store it: the session works and the next launch will not,
                // which is worth saying rather than reporting a plain success.
                Err(e) => Notice::info(Msg::NoticeSignedInUnsaved {
                    tracker: tracker_name(&tracker_id),
                    reason: e.to_string(),
                }),
            };
            let _ = tx.send(Event::Toast(notice));
            activate(&sync, &tracker_id, &pair, &tx).await;
        }
        Err(e) => {
            let _ = tx.send(Event::Toast(Notice::alert(Msg::verbatim(e.to_string()))));
        }
    }
    // Cleared however the flow ended — approved, refused or expired. A prompt left up after a
    // successful sign-in would be asking for a code that no longer does anything. Not when a
    // new code has replaced this one since, which is the prompt on screen now.
    if flow.current() {
        let _ = tx.send(Event::DeviceCode(None));
    }
}

/// Forget a tracker's token.
pub fn disconnect(sync: &Sync, tracker_id: &str, tx: &Outbox) {
    let update = match sync.tokens.clear(tracker_id) {
        // The queue is deliberately left alone. Signing out is not "discard my progress", and
        // signing back in should send what accumulated.
        Ok(()) => {
            let tracker = tracker_name(tracker_id);
            let count = sync.store.outbox_waiting(tracker_id).unwrap_or(0);
            Event::Toast(Notice::info(if count == 0 {
                Msg::NoticeSignedOut { tracker }
            } else {
                Msg::NoticeSignedOutQueue { tracker, count: i64::from(count) }
            }))
        }
        Err(e) => Event::Toast(Notice::alert(Msg::NoticeSignOutFailed {
            tracker: tracker_name(tracker_id),
            reason: e.to_string(),
        })),
    };
    let _ = tx.send(update);

    // Then make every part of the app agree, immediately. Clearing the stored token is not the
    // whole of signing out: the header badge and the Accounts list would go on showing a connected
    // account until the next restart, and the trackers would go on pushing with the credential
    // they cached at construction.
    kaeri_track::sync::forget_pushed(&sync.store, tracker_id);
    let (sync, tx, tracker_id) = (sync.clone(), tx.clone(), tracker_id.to_owned());
    tokio::spawn(async move {
        if let Some(tracker) = sync.trackers.iter().find(|t| t.id() == tracker_id) {
            tracker.forget_credentials().await;
        }
        // Rebuilt from what is now true rather than patched, so there is one description of the
        // state and no way for the badge and the list to disagree.
        for state in sync.initial_states() {
            let _ = tx.send(Event::Sync(Box::new(state)));
        }
    });
}

/// Queue a status change for every signed-in tracker.
///
/// Signed-in, not merely enabled: Kitsu is enabled out of the box, and queueing for an
/// account that may never exist would grow a badge nobody can act on. A tracker signed
/// in but offline still queues — its credentials are present — which is the case the
/// outbox exists for.
pub fn set_status(sync: &Sync, id: AnilistId, status: LibrarySegment) {
    let now = kaeri_store::now();
    let watch = segment_to_status(status);
    let op = TrackOp::SetStatus { anilist_id: id, status: watch, at: now };
    let library = sync.trackers.first().map(|t| t.id());
    for tracker in sync.trackers.iter().filter(|t| t.is_authenticated() && t.serves_watching())
    {
        match sync.store.enqueue(tracker.id(), &op, now) {
            // The Library's tracker's list as kept says it now, as the Library does: see
            // `library_answers`. Its next answer replaces it.
            Ok(Some(_)) if Some(tracker.id()) == library => {
                if let Err(e) =
                    sync.store.set_library_status(tracker.id(), id, status_wire(watch), now)
                {
                    tracing::debug!(error = %e, "could not keep the status set");
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "could not queue a status change"),
        }
    }
}

/// Queue a score for every tracker.
///
/// Out of ten, because it is the one scale every connected service understands; each
/// tracker converts on push if it stores something else.
pub fn set_score(sync: &Sync, id: AnilistId, score: f32) {
    let now = kaeri_store::now();
    let op = TrackOp::SetScore { anilist_id: id, score, at: now };
    // Signed-in only, for the same reason as `set_status`.
    for tracker in sync.trackers.iter().filter(|t| t.is_authenticated() && t.serves_watching())
    {
        if let Err(e) = sync.store.enqueue(tracker.id(), &op, now) {
            tracing::warn!(error = %e, "could not queue a score");
        }
    }
}

/// Settle a divergence.
///
/// Keeping the local value queues a push; keeping the remote one is simply dropping the row,
/// because the tracker already holds it.
pub fn resolve_conflict(sync: &Sync, id: AnilistId, keep_local: bool) {
    if !keep_local {
        return;
    }
    let now = kaeri_store::now();
    let episodes = sync.store.tracker_progress(id).unwrap_or(0);
    if episodes == 0 {
        return;
    }
    let op = TrackOp::SetProgress { anilist_id: id, episode: episodes };
    for tracker in sync.trackers.iter().filter(|t| t.is_authenticated() && t.serves_watching())
    {
        let _ = sync.store.enqueue(tracker.id(), &op, now);
    }
}

fn segment_to_status(segment: LibrarySegment) -> kaeri_core::traits::WatchStatus {
    use kaeri_core::traits::WatchStatus as W;
    match segment {
        LibrarySegment::Watching => W::Current,
        LibrarySegment::Planning => W::Planning,
        LibrarySegment::Completed => W::Completed,
        LibrarySegment::Paused => W::Paused,
        LibrarySegment::Dropped => W::Dropped,
    }
}

fn pull_key(tracker_id: &str) -> String {
    format!("last_pull:{tracker_id}")
}

/// The desktop's token store: honouring `KAERI_TOKEN_STORAGE`, then
/// `trackers.token_storage`, as [`desktop_secrets`] decides.
///
/// For the entry points that run on the desktop with no engine — `--login`, `--sync`, the
/// probes — which have to agree with the app about where tokens live. Disagreeing would mean
/// signing in through one path and being unauthenticated through another.
///
/// [`desktop_secrets`]: crate::host::desktop_secrets
#[cfg(feature = "desktop")]
pub fn token_store(config: &Config, paths: &Paths) -> TokenStore {
    TokenStore::over(crate::host::desktop_secrets(config, paths))
}

/// Where the `0600` token file lives when the file backend is in use: under the data directory
/// the engine was given, rather than one it looks up for itself, so a front end with a
/// directory of its own, and a test with a temporary one, keep their tokens there.
pub fn token_dir(paths: &Paths) -> std::path::PathBuf {
    paths.data_dir.join("tokens")
}

#[cfg(test)]
mod pull_gate_tests {
    use super::PullGate;

    #[test]
    fn a_pull_just_made_answers_sync_now() {
        let gate = PullGate::default();
        assert!(!gate.recent(), "nothing pulled yet");
        gate.mark();
        assert!(gate.recent());
    }

    #[tokio::test]
    async fn only_a_return_wakes_the_loop() {
        let gate = PullGate::default();
        gate.set_away(false);
        let quiet =
            tokio::time::timeout(std::time::Duration::from_millis(50), gate.back.notified());
        assert!(quiet.await.is_err(), "staying on screen wakes nothing");
        gate.set_away(true);
        assert!(gate.is_away());
        gate.set_away(false);
        let woken =
            tokio::time::timeout(std::time::Duration::from_millis(500), gate.back.notified());
        assert!(woken.await.is_ok(), "coming back wakes the loop once");
    }
}

#[cfg(test)]
mod tests {
    use kaeri_track::secret::Backend;

    use super::*;

    #[test]
    fn the_lists_next_episode_never_passes_a_known_count() {
        assert_eq!(next_after_progress(5, Some(13)), 6);
        assert_eq!(next_after_progress(12, Some(13)), 13);
        assert_eq!(next_after_progress(13, Some(13)), 1, "finished, so watch again");
        assert_eq!(next_after_progress(15, Some(13)), 1, "bad remote data");
        assert_eq!(next_after_progress(5, None), 6, "an airing show of unknown length");
        assert_eq!(next_after_progress(0, None), 1);
        assert_eq!(next_after_progress(5, Some(0)), 6, "zero is no count");
    }

    #[test]
    fn every_segment_maps_to_a_status() {
        // These go into someone's list, so a wrong mapping silently marks a show dropped.
        use kaeri_core::traits::WatchStatus as W;
        assert_eq!(segment_to_status(LibrarySegment::Watching), W::Current);
        assert_eq!(segment_to_status(LibrarySegment::Planning), W::Planning);
        assert_eq!(segment_to_status(LibrarySegment::Completed), W::Completed);
        assert_eq!(segment_to_status(LibrarySegment::Paused), W::Paused);
        assert_eq!(segment_to_status(LibrarySegment::Dropped), W::Dropped);
    }

    /// Tokens in files under `dir`, so no tracker a test builds looks for its token where the
    /// user's are, in their keychain least of all.
    fn files_in(dir: &std::path::Path) -> TokenStore {
        TokenStore::with_backend(dir, Backend::File)
    }

    #[test]
    fn a_tracker_with_no_token_is_still_built() {
        // So the Accounts overlay can offer a sign-in, and so the outbox accumulates in the
        // meantime — watch now, sign in later.
        let mut config = Config::default();
        config.trackers.enabled = vec!["anilist".into()];
        config.trackers.anilist.client_id = Some("12345".into());
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();

        let sync = Sync::with_tokens(&config, &store, &http, files_in(dir.path()));
        assert_eq!(sync.trackers.len(), 1);
        assert_eq!(sync.trackers[0].id(), "anilist");
    }

    #[test]
    fn the_default_config_builds_kitsu_and_nothing_else() {
        // Kitsu needs no registration, so it is enabled out of the box — constructed
        // unauthenticated, which is what puts it on the Accounts screen. Everything
        // else stays opt-in. Built against an empty token store on purpose: reading the
        // developer's real one made this test flip the day someone actually signed in.
        let config = Config::default();
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let tokens = files_in(dir.path());
        let sync = Sync::with_tokens(&config, &store, &http, tokens);
        let ids: Vec<_> = sync.trackers.iter().map(|t| t.id()).collect();
        assert_eq!(ids, ["kitsu", "mangaupdates"]);
        assert!(sync.trackers.iter().all(|t| !t.is_authenticated()));
    }

    #[test]
    fn accounts_lists_each_tracker_for_the_media_it_serves() {
        use kaeri_model::medium::KaeriIsFor as F;
        let config = Config::default();
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let sync = Sync::with_tokens(&config, &store, &http, files_in(dir.path()));
        let ids = |answer| -> Vec<String> {
            sync.states_shown(answer).into_iter().map(|s| s.tracker).collect()
        };
        assert_eq!(ids(F::Watch), ["kitsu"]);
        assert_eq!(ids(F::Read), ["kitsu", "mangaupdates"]);
        assert_eq!(ids(F::Both), ["kitsu", "mangaupdates"]);
    }

    #[test]
    fn the_reading_mapping_reads_the_store_both_ways() {
        use kaeri_track::read::ReadMapping;
        let store = Store::open_in_memory().unwrap();
        store.set_track_map("a:7", "mangaupdates", Some("9000000001"), "hand", 1).unwrap();
        store.set_track_map("a:7", "mal", Some("55"), "anilist", 1).unwrap();
        let map = StoreReadMapping { store };
        let id = AnilistId::new(7);
        assert_eq!(map.mangaupdates_id(id), Some(9_000_000_001));
        assert_eq!(map.anilist_for_mangaupdates(9_000_000_001), Some(id));
        assert_eq!(map.mal_id(id), Some(55));
        assert_eq!(map.anilist_for_mal(55), Some(id));
        assert_eq!(map.kitsu_id(id), None);
    }

    #[test]
    fn an_emptied_enabled_list_builds_no_trackers() {
        // The default is a default, not a mandate: clearing `enabled` clears the screen.
        let mut config = Config::default();
        config.trackers.enabled.clear();
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        assert!(
            Sync::with_tokens(&config, &store, &http, files_in(dir.path())).trackers.is_empty()
        );
    }

    #[test]
    fn a_signed_out_tracker_gets_nothing_queued() {
        // Enabled no longer implies intent, so status and score changes queue only for
        // trackers with credentials. Without this, the default Kitsu entry would grow
        // a queue badge for an account that may never exist.
        let config = Config::default();
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let tokens = files_in(dir.path());
        let sync = Sync::with_tokens(&config, &store, &http, tokens);

        set_status(&sync, AnilistId::new(1), LibrarySegment::Completed);
        set_score(&sync, AnilistId::new(1), 8.0);
        assert_eq!(store.outbox_depth(Some("kitsu")).unwrap(), 0);
    }

    /// A tracker whose library pull fails, or succeeds, as the test sets it.
    struct Flaky {
        down: std::sync::atomic::AtomicBool,
    }

    #[async_trait::async_trait]
    impl Tracker for Flaky {
        fn id(&self) -> &str {
            "flaky"
        }
        fn is_authenticated(&self) -> bool {
            true
        }
        async fn pull_library(
            &self,
        ) -> Result<Vec<kaeri_core::traits::TrackedEntry>, kaeri_core::Error> {
            if self.down.load(std::sync::atomic::Ordering::Relaxed) {
                Err(kaeri_core::Error::Network("timed out".into()))
            } else {
                Ok(Vec::new())
            }
        }
        async fn push(&self, _: &[TrackOp]) -> Result<(), kaeri_core::Error> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn a_failed_pull_reaches_the_accounts_row_and_a_good_one_clears_it() {
        // Pull failures only reached the log, so Accounts went on saying "pulled 40 titles"
        // from the last success for the length of an outage.
        let mut config = Config::default();
        config.trackers.enabled.clear();
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let flaky = Arc::new(Flaky { down: true.into() });
        let dir = tempfile::tempdir().unwrap();
        let sync = Sync {
            trackers: vec![flaky.clone()],
            ..Sync::with_tokens(&config, &store, &http, files_in(dir.path()))
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let tx = Outbox::from(tx);

        pull_once(&sync, &tx).await;
        let Some(Event::Sync(state)) = rx.try_recv().ok() else { panic!("no state sent") };
        let error = state.pull_error.expect("the failure is on the row").reason;
        assert!(error.contains("timed out"), "{error}");

        // It survives the next drain's state, which used to replace it whole.
        assert!(
            sync.state_for(flaky.as_ref(), false, Some("sent 1".into())).pull_error.is_some()
        );

        flaky.down.store(false, std::sync::atomic::Ordering::Relaxed);
        pull_once(&sync, &tx).await;
        let Some(Event::Sync(state)) = rx.try_recv().ok() else { panic!("no state sent") };
        assert_eq!(state.pull_error, None);
        assert_eq!(state.last.as_deref(), Some("pulled 0 titles"));
    }

    /// A signed-in tracker for the stuck-badge tests: it holds a token, may have no id for some
    /// titles, and may reject the credential, as the test sets it.
    struct Fake {
        id: &'static str,
        unmapped: Vec<AnilistId>,
        rejects: std::sync::atomic::AtomicBool,
    }

    impl Fake {
        fn new(id: &'static str, unmapped: Vec<AnilistId>, rejects: bool) -> Arc<Self> {
            Arc::new(Self { id, unmapped, rejects: rejects.into() })
        }

        fn answer(&self) -> Result<(), kaeri_core::Error> {
            if self.rejects.load(std::sync::atomic::Ordering::Relaxed) {
                Err(kaeri_core::Error::Auth("401".into()))
            } else {
                Ok(())
            }
        }
    }

    #[async_trait::async_trait]
    impl Tracker for Fake {
        fn id(&self) -> &str {
            self.id
        }
        fn is_authenticated(&self) -> bool {
            true
        }
        fn can_address(&self, op: &TrackOp) -> bool {
            !self.unmapped.contains(&op.anilist_id())
        }
        async fn pull_library(
            &self,
        ) -> Result<Vec<kaeri_core::traits::TrackedEntry>, kaeri_core::Error> {
            self.answer().map(|()| Vec::new())
        }
        async fn push(&self, _: &[TrackOp]) -> Result<(), kaeri_core::Error> {
            self.answer()
        }
    }

    fn progress(id: u32) -> TrackOp {
        TrackOp::SetProgress { anilist_id: AnilistId::new(id), episode: 3 }
    }

    /// The trackers of a library found stuck, in made-up titles: AniList clear; Kitsu with
    /// three changes for titles it has no id for; Trakt with seven changes for titles it has no
    /// id for either. The test goes on to break Trakt's credential and queue seven more.
    fn stuck_library(store: &Store) -> (Arc<Fake>, Arc<Fake>, Arc<Fake>) {
        let kitsu_unmapped: Vec<_> = (990_101..=990_103).map(AnilistId::new).collect();
        let trakt_unmapped: Vec<_> = (990_201..=990_207).map(AnilistId::new).collect();
        for id in &kitsu_unmapped {
            store.enqueue("kitsu", &progress(id.get()), 1_000).unwrap();
        }
        for id in &trakt_unmapped {
            store.enqueue("trakt", &progress(id.get()), 1_000).unwrap();
        }
        (
            Fake::new("anilist", Vec::new(), false),
            Fake::new("kitsu", kitsu_unmapped, false),
            Fake::new("trakt", trakt_unmapped, false),
        )
    }

    /// The old badge: every queued row of every signed-in tracker, as the apps summed it.
    fn badge_before(store: &Store, states: &[SyncState]) -> u32 {
        states
            .iter()
            .filter(|s| s.connected)
            .map(|s| store.outbox_depth(Some(&s.tracker)).unwrap())
            .sum()
    }

    /// What the apps sum now: what is on its way, from trackers whose credential stands.
    fn badge_after(states: &[SyncState]) -> u32 {
        states.iter().filter(|s| s.connected && !s.needs_reauth).map(|s| s.outbox).sum()
    }

    fn state<'a>(states: &'a [SyncState], tracker: &str) -> &'a SyncState {
        states.iter().find(|s| s.tracker == tracker).expect("a state for every tracker")
    }

    #[tokio::test]
    async fn a_held_change_and_a_rejected_credential_are_not_counted_as_waiting() {
        // Seventeen changes "waiting to sync" for days: three a tracker had no id for, held until
        // a mapping arrives, and fourteen behind a credential the tracker kept rejecting.
        let mut config = Config::default();
        config.trackers.enabled.clear();
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let (anilist, kitsu, trakt) = stuck_library(&store);
        let sync = Sync {
            trackers: vec![anilist, kitsu, trakt.clone()],
            ..Sync::with_tokens(&config, &store, &http, files_in(dir.path()))
        };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let tx = Outbox::from(tx);

        // Trakt's credential worked once: its drain held the seven it has no id for.
        drain_once(&sync, &tx).await;
        assert_eq!(store.outbox_deferred("kitsu").unwrap(), 3);
        assert_eq!(store.outbox_deferred("trakt").unwrap(), 7);
        // Then it stopped working. Seven more episodes were watched behind it, and a mapping
        // refresh woke the held seven.
        trakt.rejects.store(true, std::sync::atomic::Ordering::Relaxed);
        for id in 990_301..=990_307 {
            store.enqueue("trakt", &progress(id), 2_000).unwrap();
        }
        drain_once(&sync, &tx).await;
        store.wake_deferred().unwrap();
        drain_once(&sync, &tx).await;
        assert_eq!(store.outbox_depth(Some("trakt")).unwrap(), 14);
        assert_eq!(
            store.outbox_deferred("trakt").unwrap(),
            0,
            "woken, and never drained again"
        );

        let states = sync.initial_states();
        assert_eq!(badge_before(&store, &states), 17, "the pill said 17 changes waiting");
        assert_eq!(badge_after(&states), 0, "nothing is on its way");

        let kitsu = state(&states, "kitsu");
        assert_eq!((kitsu.outbox, kitsu.deferred, kitsu.needs_reauth), (0, 3, false));
        let trakt_state = state(&states, "trakt");
        assert!(
            trakt_state.needs_reauth,
            "the rejected credential is said from the first state"
        );
        assert_eq!(trakt_state.outbox, 14, "kept for when it is signed in again");

        // Signing in again lifts it, and the queue is waiting again until the drain sends it.
        trakt.rejects.store(false, std::sync::atomic::Ordering::Relaxed);
        kaeri_track::sync::clear_reauth_hold(&store, "trakt").unwrap();
        assert!(!state(&sync.initial_states(), "trakt").needs_reauth);
        drain_once(&sync, &tx).await;
        let states = sync.initial_states();
        let trakt_state = state(&states, "trakt");
        assert_eq!(
            (trakt_state.outbox, trakt_state.deferred),
            (0, 7),
            "sent, or held for an id"
        );
    }

    #[tokio::test]
    async fn a_pull_the_tracker_rejects_holds_the_queue_and_one_that_goes_through_lifts_it() {
        // A rejected pull only told the open window, so a restart forgot it and the queue behind
        // it showed as waiting again.
        let mut config = Config::default();
        config.trackers.enabled.clear();
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let trakt = Fake::new("trakt", Vec::new(), true);
        let sync = Sync {
            trackers: vec![trakt.clone()],
            ..Sync::with_tokens(&config, &store, &http, files_in(dir.path()))
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let tx = Outbox::from(tx);

        pull_once(&sync, &tx).await;
        let Some(Event::Sync(state)) = rx.try_recv().ok() else { panic!("no state sent") };
        assert!(state.needs_reauth);
        assert!(kaeri_track::sync::credential_rejected(&store, "trakt"));
        assert!(sync.initial_states()[0].needs_reauth, "and a launch still knows");

        // A tracker's passing trouble dressed as a rejection: the next pull goes through.
        trakt.rejects.store(false, std::sync::atomic::Ordering::Relaxed);
        pull_once(&sync, &tx).await;
        assert!(!kaeri_track::sync::credential_rejected(&store, "trakt"));
        assert!(!sync.initial_states()[0].needs_reauth);
    }

    #[test]
    fn queueing_with_no_tracker_signed_in_reports_no_state() {
        // A default state stood in for "no tracker", and became a nameless Accounts row.
        let config = Config::default();
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let tokens = files_in(dir.path());
        let sync = Sync::with_tokens(&config, &store, &http, tokens);
        assert!(sync.state_after_enqueue().is_empty(), "signed-out kitsu is not queued for");

        let none = Sync { trackers: Vec::new(), ..sync };
        assert!(none.state_after_enqueue().is_empty());
    }

    #[tokio::test]
    async fn kitsu_asked_to_sign_in_the_browser_way_points_at_its_row_without_naming_a_key() {
        // The key that asks for credentials is the front end's to say, as a hint.
        let config = Config::default();
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let tokens = files_in(dir.path());
        let sync = Sync::with_tokens(&config, &store, &http, tokens);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let host = Arc::new(crate::host::QuietHost::in_dir(dir.path()));
        let clients = crate::options::AppClients::default();
        connect(sync, host, clients, "kitsu".into(), None, Outbox::from(tx)).await;
        let Ok(Event::Toast(notice)) = rx.try_recv() else { panic!("no notice") };
        assert_eq!(notice.msg, Some(Msg::NoticeKitsuSignsInOnAccounts));
        assert_eq!(notice.hint, Some(NoticeHint::KitsuCredentials));
        assert!(rx.try_recv().is_err(), "and nothing else: no browser, no state");
    }

    #[cfg(not(feature = "desktop"))]
    #[tokio::test]
    async fn without_the_desktop_a_redirect_sign_in_is_refused_in_words() {
        // No listener on this machine for the redirect to reach, so no browser opened onto a
        // sign-in that cannot finish.
        let mut config = Config::default();
        config.trackers.enabled = vec!["anilist".into(), "mal".into()];
        config.trackers.anilist.client_id = Some("1".into());
        config.trackers.anilist.client_secret = Some("2".into());
        config.trackers.mal.client_id = Some("3".into());
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::offline(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        for tracker in ["anilist", "mal"] {
            let sync = Sync::with_tokens(&config, &store, &http, files_in(dir.path()));
            let host = Arc::new(crate::host::QuietHost::in_dir(dir.path()));
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            // No client of the app's own, so no sheet to sign in through either.
            let clients = crate::options::AppClients::default();
            connect(sync, host, clients, tracker.into(), None, Outbox::from(tx)).await;
            let Ok(Event::Toast(notice)) = rx.try_recv() else { panic!("no notice") };
            let name = tracker_name(tracker);
            assert_eq!(notice.msg, Some(Msg::NoticeSignInNeedsRedirect { tracker: name }));
            assert!(rx.try_recv().is_err(), "and nothing else");
        }
    }

    #[test]
    fn the_pull_cursor_key_is_per_tracker() {
        // Shared, one tracker's pull would reset the other's last-write-wins baseline.
        assert_ne!(pull_key("anilist"), pull_key("mal"));
    }

    #[tokio::test]
    async fn signing_out_keeps_the_queue() {
        // "Sign out" is not "throw away what I watched". Async because signing out now also tells
        // the trackers themselves to drop their cached credentials and republishes the state — the
        // token store alone was never the whole of it.
        let mut config = Config::default();
        config.trackers.enabled = vec!["anilist".into()];
        config.trackers.anilist.client_id = Some("1".into());
        let store = Store::open_in_memory().unwrap();
        let http = kaeri_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let sync = Sync::with_tokens(&config, &store, &http, files_in(dir.path()));

        store
            .enqueue(
                "anilist",
                &TrackOp::SetProgress { anilist_id: AnilistId::new(1), episode: 5 },
                0,
            )
            .unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let tx = Outbox::from(tx);
        disconnect(&sync, "anilist", &tx);
        assert_eq!(store.outbox_depth(Some("anilist")).unwrap(), 1);
    }
}
