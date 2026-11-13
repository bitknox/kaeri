//! The AniList [`Tracker`] implementation.
//!
//! The reference tracker, and the one the rest of the app is keyed on — AniList media ids are
//! the primary key everywhere, so this needs no mapping layer at all. Every other tracker will
//! have to go through `kaeri-meta::mapping` to translate ids; this one does not, which is
//! why it lands first.

use kaeri_core::{
    ids::AnilistId,
    traits::{
        Pushed, ReadRemoval, TrackOp, TrackedEntry, TrackedReadEntry, Tracker, WatchStatus,
    },
};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use kaeri_meta::anilist::{AniList, AniListError, Lane, LibraryEntry, TitleLanguage, Viewer};

/// How long one library answer serves every ask for the library.
///
/// The Library screen asks for its five segments at once, and every one of them is cut from
/// the same list: five requests for one answer, the first time it opened. Short, because the
/// list is the remote one and this screen is how its changes are seen.
pub const LIBRARY_REUSE: Duration = Duration::from_secs(30);

/// How often an ask waiting for the viewer or the list raises the request it waits on again:
/// see [`AniListTracker::wait_for`].
const RAISE_EVERY: Duration = Duration::from_millis(250);

/// Where the signed-in viewer's AniList id is kept between launches, so a launch's list can
/// ride with its viewer lookup. See [`AniListTracker::with_store`].
pub const VIEWER_ID_KEY: &str = "anilist:viewer_id";

/// Where the title language that viewer reads AniList in is kept beside the id, so a launch
/// names titles their way from its first frame: `ROMAJI`, `ENGLISH` or `NATIVE`.
pub const VIEWER_LANGUAGE_KEY: &str = "anilist:title_language";

/// The library as last asked for, and for whom.
struct LastLibrary {
    /// [`AniListTracker::changes`] when it was asked for: a sign-in, a sign-out or a change
    /// written to the list since means a different answer.
    changes: u64,
    at: Instant,
    answer: Result<Vec<LibraryEntry>, AniListError>,
}

/// AniList's `MediaListStatus` enum, as strings.
///
/// Kept as an explicit pair of functions rather than serde attributes because the mapping is
/// part of a wire contract with someone else's API — a rename on our side must not silently
/// start writing a different status to somebody's list.
pub const fn status_to_anilist(status: WatchStatus) -> &'static str {
    match status {
        WatchStatus::Current => "CURRENT",
        WatchStatus::Planning => "PLANNING",
        WatchStatus::Completed => "COMPLETED",
        WatchStatus::Paused => "PAUSED",
        WatchStatus::Dropped => "DROPPED",
        WatchStatus::Repeating => "REPEATING",
    }
}

/// Parse AniList's status back.
///
/// An unrecognised value maps to `Current` rather than failing: AniList adding a status we have
/// never heard of should not blank someone's library.
pub fn status_from_anilist(status: &str) -> WatchStatus {
    match status {
        "PLANNING" => WatchStatus::Planning,
        "COMPLETED" => WatchStatus::Completed,
        "PAUSED" => WatchStatus::Paused,
        "DROPPED" => WatchStatus::Dropped,
        "REPEATING" => WatchStatus::Repeating,
        _ => WatchStatus::Current,
    }
}

/// The AniList tracker.
pub struct AniListTracker {
    client: AniList,
    /// Resolved on first use and cached — the library query needs a user id, and asking for it
    /// on every pull would waste one of thirty requests a minute. The same answer carries the
    /// title language the viewer reads in, so asking for that costs nothing more.
    viewer: tokio::sync::Mutex<Option<Viewer>>,
    /// Set when the user signs out.
    ///
    /// The client holds its token from construction, and clearing the *stored* token does not
    /// revoke it at AniList — so without this the tracker would keep reporting itself connected and
    /// keep pushing with a credential the user had just withdrawn. Signing out has to mean
    /// something immediately, not at the next restart.
    signed_out: std::sync::atomic::AtomicBool,
    /// The last library answer. Held across the request, so asks made while one is out wait
    /// for it and take its answer: see [`LIBRARY_REUSE`].
    library: tokio::sync::Mutex<Option<LastLibrary>>,
    /// The lane of the request made under `viewer` or `library` while it is out. A background
    /// pull's request waits its turn in the background queue with the lock held, so an ask
    /// that comes to wait for the lock raises the request to its own class: the Library screen
    /// never waits behind the pull's place in that queue.
    asking: std::sync::Mutex<Option<Lane>>,
    /// Counted up on every sign-in, sign-out and change written to the list, each of which
    /// makes the last library answer someone else's or out of date.
    changes: AtomicU64,
    /// Where the viewer's id is kept between launches, when given: see [`VIEWER_ID_KEY`].
    store: Option<kaeri_store::Store>,
}

impl AniListTracker {
    /// `client` must already carry the token; an unauthenticated one degrades to local-only.
    pub fn new(client: AniList) -> Self {
        Self {
            client,
            viewer: tokio::sync::Mutex::new(None),
            signed_out: std::sync::atomic::AtomicBool::new(false),
            library: tokio::sync::Mutex::new(None),
            asking: std::sync::Mutex::new(None),
            changes: AtomicU64::new(0),
            store: None,
        }
    }

    /// Keep the signed-in viewer's id and title language in `store` between launches, and
    /// nothing else of theirs. With the id, the first viewer lookup of a launch asks for the
    /// list in the same request, and the list's pull takes that answer; every pull asks for the
    /// viewer with the list, so both are kept as AniList last said. A sign-in or a sign-out
    /// drops them, and an answer for someone else replaces them.
    #[must_use]
    pub fn with_store(mut self, store: kaeri_store::Store) -> Self {
        self.store = Some(store);
        self
    }

    /// This tracker, asking against `base`'s budget and endpoint, with its own token: the app has
    /// one AniList budget, and the list's pulls and writes are counted with every other request.
    #[must_use]
    pub fn on_budget_of(&self, base: &AniList) -> Self {
        let tracker = Self::new(self.client.on_budget_of(base));
        match &self.store {
            Some(store) => tracker.with_store(store.clone()),
            None => tracker,
        }
    }

    /// The viewer's id as kept from an earlier launch.
    fn remembered(&self) -> Option<u32> {
        let store = self.store.as_ref()?;
        store.get_meta_text(VIEWER_ID_KEY).ok().flatten()?.parse().ok()
    }

    /// Keep `viewer`'s id and title language, or with `None` drop what was kept.
    fn remember(&self, viewer: Option<&Viewer>) {
        let Some(store) = &self.store else { return };
        let language = viewer.and_then(|v| v.title_language).map(|l| match l {
            TitleLanguage::Romaji => "ROMAJI",
            TitleLanguage::English => "ENGLISH",
            TitleLanguage::Native => "NATIVE",
        });
        let kept = match viewer {
            Some(viewer) => store.set_meta_text(VIEWER_ID_KEY, &viewer.id.to_string()),
            None => store.delete_meta(VIEWER_ID_KEY),
        }
        .and_then(|()| match language {
            Some(language) => store.set_meta_text(VIEWER_LANGUAGE_KEY, language),
            None => store.delete_meta(VIEWER_LANGUAGE_KEY),
        });
        if let Err(e) = kept {
            tracing::warn!(error = %e, "could not keep the AniList viewer's id");
        }
    }

    /// The title language the signed-in viewer read AniList in when last asked, as kept from
    /// an earlier launch, without asking. `None` signed out, or when nothing is kept.
    pub fn kept_title_language(&self) -> Option<TitleLanguage> {
        if !self.is_authenticated() {
            return None;
        }
        self.remembered()?;
        let store = self.store.as_ref()?;
        TitleLanguage::from_wire(&store.get_meta_text(VIEWER_LANGUAGE_KEY).ok().flatten()?)
    }

    /// The title language in the viewer's last answer this run, without asking: `None` when
    /// AniList has not answered for the viewer yet, `Some(None)` when it did not say one.
    pub async fn answered_title_language(&self) -> Option<Option<TitleLanguage>> {
        self.wait_for(&self.viewer).await.as_ref().map(|viewer| viewer.title_language)
    }

    /// The full library, with media attached, for the Library screen.
    ///
    /// [`Tracker::pull_library`] returns only the syncable projection; this returns everything
    /// AniList sent, so the screen does not need a second pass of lookups.
    ///
    /// One request answers every ask for [`LIBRARY_REUSE`], and every ask made while it was
    /// out, a failure included: an ask that waited for one is not made again.
    pub async fn library(&self) -> Result<Vec<LibraryEntry>, kaeri_core::Error> {
        let asked = Instant::now();
        // The viewer first, outside the list's lock: with the id kept from before, the
        // viewer's request brings the list as well, and leaves it as the last answer.
        let viewer = self.viewer().await;
        let mut last = self.wait_for(&self.library).await;
        let changes = self.changes.load(Ordering::SeqCst);
        if let Some(last) = last.as_ref()
            && last.changes == changes
            && (last.at >= asked || (last.answer.is_ok() && last.at.elapsed() < LIBRARY_REUSE))
        {
            return last.answer.clone().map_err(to_core);
        }
        let (answer, fresh) = match viewer {
            Ok(viewer) => self.asking(self.list_with_viewer(viewer.id)).await,
            Err(e) => (Err(e), None),
        };
        *last = Some(LastLibrary { changes, at: Instant::now(), answer: answer.clone() });
        drop(last);
        if let Some(viewer) = fresh {
            self.remember(Some(&viewer));
            *self.wait_for(&self.viewer).await = Some(viewer);
        }
        answer.map_err(to_core)
    }

    /// Take `lock`, raising the request made under it, if one is out, to the caller's class
    /// while waiting.
    async fn wait_for<'a, T>(
        &self,
        lock: &'a tokio::sync::Mutex<T>,
    ) -> tokio::sync::MutexGuard<'a, T> {
        let mut taken = std::pin::pin!(lock.lock());
        loop {
            if let Some(lane) = self.asking.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                lane.raise(kaeri_meta::anilist::priority());
            }
            if let Ok(guard) = tokio::time::timeout(RAISE_EVERY, taken.as_mut()).await {
                return guard;
            }
        }
    }

    /// Make `ask`, a request under one of the list's locks, in a lane of its own in the
    /// caller's class, which [`Self::wait_for`] raises for whoever comes to wait. Its own lane,
    /// so raising it hurries this request and not the rest of the caller's work.
    async fn asking<F: std::future::Future>(&self, ask: F) -> F::Output {
        let lane = Lane::new(kaeri_meta::anilist::priority());
        *self.asking.lock().unwrap_or_else(|e| e.into_inner()) = Some(lane.clone());
        let answer = lane.run(ask).await;
        *self.asking.lock().unwrap_or_else(|e| e.into_inner()) = None;
        answer
    }

    /// The list of the viewer `id`, asked with the viewer, and the viewer as answered. An
    /// answer for someone else has their list asked for instead. Refused together, the list is
    /// asked alone.
    async fn list_with_viewer(
        &self,
        id: u32,
    ) -> (Result<Vec<LibraryEntry>, AniListError>, Option<Viewer>) {
        match self.client.viewer_and_library(id).await {
            Ok((viewer, library)) if viewer.id == id => (Ok(library), Some(viewer)),
            Ok((viewer, _)) => {
                tracing::info!("AniList answered for another viewer than the one asked for");
                (self.client.library(viewer.id).await, Some(viewer))
            }
            Err(AniListError::Api(_) | AniListError::NotFound | AniListError::Decode(_)) => {
                (self.client.library(id).await, None)
            }
            Err(e) => (Err(e), None),
        }
    }

    /// The last library answer is someone else's now, or out of date.
    fn changed(&self) {
        self.changes.fetch_add(1, Ordering::SeqCst);
    }

    /// Remove an entry from the user's list.
    ///
    /// Takes the `MediaList` entry id from [`kaeri_meta::anilist::LibraryEntry::entry_id`].
    /// Not part of the [`Tracker`] trait: deleting is not something sync ever does — the outbox
    /// only ever moves progress forward — so it stays off the interface every tracker must
    /// implement.
    pub async fn delete(&self, entry_id: u32) -> Result<bool, kaeri_core::Error> {
        let deleted = self.client.delete_entry(entry_id).await.map_err(to_core);
        self.changed();
        deleted
    }

    /// The title language the signed-in viewer reads AniList in: romaji, English or native.
    /// `None` when AniList did not say.
    pub async fn title_language(&self) -> Result<Option<TitleLanguage>, kaeri_core::Error> {
        Ok(self.viewer().await.map_err(to_core)?.title_language)
    }

    /// A client for the viewer's own writes outside the list, such as a discussion's replies and
    /// likes: this tracker's token, asking against `base`'s budget, which every screen shares.
    /// `None` while nobody is signed in.
    pub fn acting_on(&self, base: &AniList) -> Option<AniList> {
        self.is_authenticated().then(|| self.client.on_budget_of(base))
    }

    /// The signed-in viewer, asked once and kept: their id tells their own comments apart, and
    /// their name signs a comment on its way. `None` while nobody is signed in, or when AniList
    /// could not say.
    pub async fn signed_in_viewer(&self) -> Option<Viewer> {
        if !self.is_authenticated() {
            return None;
        }
        self.viewer().await.ok()
    }

    async fn viewer(&self) -> Result<Viewer, AniListError> {
        let mut cached = self.wait_for(&self.viewer).await;
        if let Some(viewer) = cached.as_ref() {
            return Ok(viewer.clone());
        }
        if let Some(id) = self.remembered() {
            // Held across the request, so a pull made meanwhile waits for this list.
            let mut last = self.wait_for(&self.library).await;
            let changes = self.changes.load(Ordering::SeqCst);
            match self.asking(self.client.viewer_and_library(id)).await {
                Ok((viewer, library)) => {
                    if viewer.id == id {
                        *last = Some(LastLibrary {
                            changes,
                            at: Instant::now(),
                            answer: Ok(library),
                        });
                    } else {
                        // Someone else is signed in now: their list is asked for anew.
                        tracing::info!("AniList answered for another viewer than the one kept");
                    }
                    self.remember(Some(&viewer));
                    *cached = Some(viewer.clone());
                    return Ok(viewer);
                }
                // Refused, as it would be for another account's private list or one that is
                // gone, or signed out: the id is dropped and the viewer asked alone.
                Err(
                    AniListError::Api(_)
                    | AniListError::NotFound
                    | AniListError::Decode(_)
                    | AniListError::Unauthenticated,
                ) => self.remember(None),
                Err(e) => return Err(e),
            }
        }
        let viewer = self.asking(self.client.viewer()).await?;
        self.remember(Some(&viewer));
        *cached = Some(viewer.clone());
        Ok(viewer)
    }
}

#[async_trait::async_trait]
impl Tracker for AniListTracker {
    fn id(&self) -> &str {
        "anilist"
    }

    fn is_authenticated(&self) -> bool {
        !self.signed_out.load(std::sync::atomic::Ordering::Relaxed)
            && self.client.is_authenticated()
    }

    async fn accept_credentials(
        &self,
        access: &str,
        _refresh: Option<&str>,
        _expires_at: Option<i64>,
    ) {
        self.client.set_token(Some(access.to_owned()));
        self.signed_out.store(false, std::sync::atomic::Ordering::Relaxed);
        // The cached viewer id belonged to whoever was signed in before, and a library pull keyed on
        // it would fetch the wrong account's list. So did the last library answer.
        *self.wait_for(&self.viewer).await = None;
        self.remember(None);
        self.changed();
    }

    async fn forget_credentials(&self) {
        self.signed_out.store(true, std::sync::atomic::Ordering::Relaxed);
        // Drop the cached viewer id too, so signing into a *different* account cannot inherit the
        // previous one's list.
        *self.wait_for(&self.viewer).await = None;
        self.remember(None);
        self.changed();
    }

    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, kaeri_core::Error> {
        Ok(self
            .library()
            .await?
            .into_iter()
            .map(|entry| TrackedEntry {
                anilist_id: entry.media.id,
                progress: entry.progress,
                status: status_from_anilist(&entry.status),
                score: entry.score,
            })
            .collect())
    }

    async fn push(&self, ops: &[TrackOp]) -> Result<(), kaeri_core::Error> {
        self.push_checked(ops).await?.into_result("anilist")
    }

    /// One `SaveMediaListEntry` per title, each with its own outcome: a title AniList refuses
    /// fails alone, and the titles after it still go.
    async fn push_checked(&self, ops: &[TrackOp]) -> Result<Pushed, kaeri_core::Error> {
        // Checked per push, not just at startup: the outbox drains on a timer, and a queued op must
        // not reach an account the user has signed out of.
        if !self.is_authenticated() {
            return Err(kaeri_core::Error::Auth("signed out".into()));
        }
        let mut titles = crate::Titles::default();
        for entry in coalesce(ops) {
            let result = if titles.go() {
                self.client
                    .save_entry(entry.anilist_id, entry.progress, entry.status, entry.score)
                    .await
                    .map(drop)
                    .map_err(to_core)
            } else {
                Ok(())
            };
            titles.record(entry.anilist_id, result);
        }
        // The list as last fetched is out of date once one of these may have reached it, so
        // the next ask is a new one even if a save failed partway.
        if !ops.is_empty() {
            self.changed();
        }
        Ok(titles.finish())
    }

    fn serves_reading(&self) -> bool {
        true
    }

    async fn pull_read_library(&self) -> Result<Vec<TrackedReadEntry>, kaeri_core::Error> {
        let viewer = self.viewer().await.map_err(to_core)?;
        let rows = self.asking(self.client.read_library(viewer.id)).await.map_err(to_core)?;
        Ok(rows
            .into_iter()
            .map(|row| TrackedReadEntry {
                anilist_id: row.anilist_id,
                medium: row.medium,
                chapters: row.chapters,
                volumes: Some(row.volumes),
                status: status_from_anilist(&row.status),
                rereading: row.status == "REPEATING",
                score: row.score,
            })
            .collect())
    }

    async fn remove_read(&self, id: AnilistId) -> Result<ReadRemoval, kaeri_core::Error> {
        let removed = self.client.remove_read_entry(id).await.map_err(to_core);
        self.changed();
        Ok(if removed? { ReadRemoval::Removed } else { ReadRemoval::NotOnList })
    }

    async fn push_read(&self, ops: &[TrackOp]) -> Result<Pushed, kaeri_core::Error> {
        if !self.is_authenticated() {
            return Err(kaeri_core::Error::Auth("signed out".into()));
        }
        let mut titles = crate::Titles::default();
        for entry in coalesce_read(ops) {
            let result = if titles.go() {
                self.client
                    .save_read_entry(
                        entry.anilist_id,
                        entry.chapters,
                        entry.volumes,
                        entry.status,
                        entry.score,
                    )
                    .await
                    .map_err(to_core)
            } else {
                Ok(())
            };
            titles.record(entry.anilist_id, result);
        }
        if ops.iter().any(TrackOp::is_reading) {
            self.changed();
        }
        Ok(titles.finish())
    }
}

/// One manga's or novel's worth of changes, as a single mutation.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadMutation {
    pub anilist_id: AnilistId,
    pub chapters: Option<u32>,
    pub volumes: Option<u32>,
    pub status: Option<&'static str>,
    pub score: Option<f32>,
}

/// Fold a batch of reading ops into one mutation per work. Chapters and volumes keep the
/// highest, and a reread goes out as AniList's `REPEATING`.
pub fn coalesce_read(ops: &[TrackOp]) -> Vec<ReadMutation> {
    let mut batched: Vec<ReadMutation> = Vec::new();
    for op in ops.iter().filter(|op| op.is_reading()) {
        let id = op.anilist_id();
        let at = match batched.iter().position(|m| m.anilist_id == id) {
            Some(at) => at,
            None => {
                batched.push(ReadMutation {
                    anilist_id: id,
                    chapters: None,
                    volumes: None,
                    status: None,
                    score: None,
                });
                batched.len() - 1
            }
        };
        let slot = &mut batched[at];
        match op {
            TrackOp::SetReadProgress { chapter, volume, .. } => {
                slot.chapters = slot.chapters.max(Some(*chapter));
                slot.volumes = slot.volumes.max(*volume);
            }
            TrackOp::SetReadStatus { status, rereading, .. } => {
                slot.status = Some(if *rereading {
                    status_to_anilist(WatchStatus::Repeating)
                } else {
                    status_to_anilist(*status)
                });
            }
            TrackOp::SetReadScore { score, .. } => slot.score = Some(*score),
            _ => {}
        }
    }
    batched
}

/// One title's worth of changes, as a single mutation.
#[derive(Debug, Clone, PartialEq)]
pub struct Mutation {
    pub anilist_id: AnilistId,
    /// `None` fields are omitted from the mutation rather than sent as null, because AniList
    /// treats null as *clear this* — a progress push would otherwise wipe the user's score.
    pub progress: Option<u32>,
    pub status: Option<&'static str>,
    pub score: Option<f32>,
}

/// Fold a batch of ops into one mutation per title.
///
/// `SaveMediaListEntry` sets whichever fields are present, so three ops for one show are one
/// request. Sending them separately would burn three of thirty requests a minute for no benefit.
/// Extracted so the batching rules are testable without a network.
pub fn coalesce(ops: &[TrackOp]) -> Vec<Mutation> {
    let mut batched: Vec<Mutation> = Vec::new();

    for op in ops {
        // Reading waits in its own outbox and is never in a watching batch.
        if op.is_reading() {
            continue;
        }
        let id = op.anilist_id();
        let slot = match batched.iter_mut().find(|m| m.anilist_id == id) {
            Some(slot) => slot,
            None => {
                batched.push(Mutation {
                    anilist_id: id,
                    progress: None,
                    status: None,
                    score: None,
                });
                batched.last_mut().expect("just pushed")
            }
        };
        match op {
            // Highest wins within a batch: progress is monotonic, and a later op carrying a
            // lower episode would otherwise undo the earlier one.
            TrackOp::SetProgress { episode, .. } => {
                slot.progress = Some(slot.progress.map_or(*episode, |c| c.max(*episode)));
            }
            TrackOp::SetStatus { status, .. } => slot.status = Some(status_to_anilist(*status)),
            TrackOp::SetScore { score, .. } => slot.score = Some(*score),
            TrackOp::SetReadProgress { .. }
            | TrackOp::SetReadStatus { .. }
            | TrackOp::SetReadScore { .. } => {}
        }
    }
    batched
}

/// Map an AniList failure onto the core error, preserving whether it is worth retrying.
fn to_core(error: AniListError) -> kaeri_core::Error {
    let tracker = || "anilist".to_owned();
    match error {
        // A bad or expired token will never succeed on retry, so it must not sit in the outbox
        // burning backoff: the user has to re-authorise.
        AniListError::Unauthenticated => {
            kaeri_core::Error::Auth("anilist rejected the token".into())
        }
        // None of these is AniList's verdict on the change, so none may bring a queued change
        // nearer its end: the network, a pause AniList asked for, its own trouble, and an
        // answer that could not be read (a gateway's error page).
        AniListError::Network(message) => kaeri_core::Error::Network(message),
        AniListError::RateLimited { retry_in_secs } => kaeri_core::Error::RateLimited {
            tracker: tracker(),
            message: error.to_string(),
            retry_in_secs: Some(retry_in_secs),
        },
        AniListError::Server { .. } | AniListError::Decode(_) => {
            kaeri_core::Error::Unavailable { tracker: tracker(), message: error.to_string() }
        }
        // GraphQL's own refusal (a validation error, an entry that is not there).
        other @ (AniListError::Api(_) | AniListError::NotFound) => {
            kaeri_core::Error::Tracker { tracker: tracker(), message: other.to_string() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSHIKUZU: AnilistId = AnilistId::new(154_587);

    #[test]
    fn every_status_survives_a_round_trip() {
        // These strings go into someone's list. A mismatch would quietly mark a show dropped.
        for status in [
            WatchStatus::Current,
            WatchStatus::Planning,
            WatchStatus::Completed,
            WatchStatus::Paused,
            WatchStatus::Dropped,
            WatchStatus::Repeating,
        ] {
            assert_eq!(status_from_anilist(status_to_anilist(status)), status);
        }
    }

    #[test]
    fn the_wire_strings_are_anilists_and_not_ours() {
        // Pinned deliberately: these are someone else's enum, so a local rename must not
        // change what goes over the wire.
        assert_eq!(status_to_anilist(WatchStatus::Current), "CURRENT");
        assert_eq!(status_to_anilist(WatchStatus::Repeating), "REPEATING");
    }

    #[test]
    fn an_unknown_status_degrades_rather_than_failing() {
        // If AniList adds a status, a library pull should still work.
        assert_eq!(status_from_anilist("SOMETHING_NEW"), WatchStatus::Current);
        assert_eq!(status_from_anilist(""), WatchStatus::Current);
    }

    fn mutation(
        id: AnilistId,
        progress: Option<u32>,
        status: Option<&'static str>,
        score: Option<f32>,
    ) -> Mutation {
        Mutation { anilist_id: id, progress, status, score }
    }

    #[test]
    fn three_ops_for_one_title_become_one_mutation() {
        // Thirty requests a minute is the whole budget, so a per-op mutation would make a
        // resync of twenty titles take a minute.
        let ops = [
            TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 12 },
            TrackOp::SetStatus { anilist_id: HOSHIKUZU, status: WatchStatus::Completed, at: 1 },
            TrackOp::SetScore { anilist_id: HOSHIKUZU, score: 9.0, at: 1 },
        ];
        let batched = coalesce(&ops);
        assert_eq!(batched.len(), 1);
        assert_eq!(batched[0], mutation(HOSHIKUZU, Some(12), Some("COMPLETED"), Some(9.0)));
    }

    #[test]
    fn batching_keeps_the_highest_progress() {
        // Two progress ops in one batch must not let the later, lower one win.
        let ops = [
            TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 12 },
            TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 4 },
        ];
        assert_eq!(
            coalesce(&ops)[0].progress,
            Some(12),
            "progress walked backwards inside a batch"
        );
    }

    #[test]
    fn different_titles_stay_separate() {
        let other = AnilistId::new(1);
        let ops = [
            TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 3 },
            TrackOp::SetProgress { anilist_id: other, episode: 7 },
        ];
        let batched = coalesce(&ops);
        assert_eq!(batched.len(), 2);
        assert_eq!(batched[0], mutation(HOSHIKUZU, Some(3), None, None));
        assert_eq!(batched[1], mutation(other, Some(7), None, None));
    }

    #[test]
    fn a_progress_only_push_leaves_status_and_score_untouched() {
        // The reason unset fields are omitted rather than nulled: AniList treats null as
        // "clear this", so a progress push would wipe the user's score.
        let ops = [TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 5 }];
        assert_eq!(coalesce(&ops)[0], mutation(HOSHIKUZU, Some(5), None, None));
    }

    #[test]
    fn an_empty_batch_sends_nothing() {
        assert!(coalesce(&[]).is_empty());
    }

    #[test]
    fn a_rejected_token_is_an_auth_error_not_a_retryable_one() {
        // It must not sit in the outbox retrying forever — nothing but re-authorising fixes it.
        assert!(matches!(to_core(AniListError::Unauthenticated), kaeri_core::Error::Auth(_)));
        assert!(matches!(
            to_core(AniListError::Network("timeout".into())),
            kaeri_core::Error::Network(_)
        ));
    }

    #[test]
    fn only_graphqls_own_refusal_counts_as_a_refusal() {
        use kaeri_core::traits::PushFailure;
        let class = |error| PushFailure::of(&to_core(error));
        assert!(matches!(class(AniListError::Api("validation".into())), PushFailure::Refused(_)));
        assert!(matches!(class(AniListError::NotFound), PushFailure::Refused(_)));
        for waits in [
            AniListError::Network("dns".into()),
            AniListError::Decode("expected value".into()),
            AniListError::Server { status: 500, message: "Internal Server Error".into() },
        ] {
            assert!(matches!(class(waits), PushFailure::Unavailable(_)));
        }
        assert!(matches!(
            class(AniListError::RateLimited { retry_in_secs: 60 }),
            PushFailure::RateLimited { retry_in_secs: Some(60), .. }
        ));
    }

    #[test]
    fn reading_ops_fold_into_one_mutation_with_the_highest_numbers() {
        let ops = [
            TrackOp::SetReadProgress { anilist_id: HOSHIKUZU, chapter: 10, volume: Some(1) },
            TrackOp::SetReadProgress { anilist_id: HOSHIKUZU, chapter: 14, volume: None },
            TrackOp::SetReadStatus {
                anilist_id: HOSHIKUZU,
                status: WatchStatus::Current,
                rereading: true,
                at: 1,
            },
            TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 99 },
        ];
        let folded = coalesce_read(&ops);
        assert_eq!(folded.len(), 1);
        assert_eq!(
            (folded[0].chapters, folded[0].volumes, folded[0].status),
            (Some(14), Some(1), Some("REPEATING"))
        );
        assert!(coalesce(&ops).iter().all(|m| m.status.is_none()), "watching ignores reading");
    }
}
