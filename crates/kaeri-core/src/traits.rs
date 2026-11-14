//! The three pluggable seams: sources, players and trackers.
//!
//! Sources differ in how they are reached, so each sits behind one of these seams and can be
//! added as a plugin without a new release.
//!
//! [`Provider`] is deliberately the same shape as the `provider` interface in
//! `wit/kaeri-provider.wit`, so a WASM plugin written in Go or TypeScript is
//! indistinguishable from a native Rust source at this boundary.

use async_trait::async_trait;

use crate::{
    books::{
        BookHints, BookIds, BookSourceManifest, ChapterContent, PageRef, SeriesDetail,
        SeriesHit, SourceChapter,
    },
    error::ProviderError,
    ids::{AnilistId, ExternalIds, ProviderKey},
    media::{Episode, SearchHit, TitleHints, Translation},
    stream::Stream,
};

/// What kind of source this is. Drives UI labelling and ranking, and decides whether a
/// resolved stream is playable locally or has to be handed off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    /// An in-process source, such as the test mock.
    Native,
    /// WASM component plugin.
    Plugin,
    /// A licensed service: its episodes open in the service's own app, through an external
    /// deep link.
    Licensed,
    /// Folders on this device, scanned into a library.
    Library,
    /// A media server the user runs, such as Jellyfin.
    Server,
    /// An open catalogue the user added: an Internet Archive collection or a PeerTube channel.
    Catalogue,
    /// The bundled demo, made of works under open licences.
    Demo,
}

impl ProviderKind {
    /// Whether Kaeri itself provides this source, rather than a plugin: what
    /// the user already has, a server they run, or a catalogue they chose.
    pub const fn is_first_party(self) -> bool {
        matches!(
            self,
            Self::Library | Self::Server | Self::Catalogue | Self::Licensed | Self::Demo
        )
    }

    /// Where this kind sits by default, lowest first: Your media, then media servers, then free
    /// catalogues, then everything else, then the demo. It breaks ties between sources the
    /// user's order places alike; the order itself is the user's. Licensed links never play
    /// inside Kaeri, so they take no part and sit with the rest.
    pub const fn tier(self) -> u8 {
        match self {
            Self::Library => 0,
            Self::Server => 1,
            Self::Catalogue => 2,
            Self::Demo => 4,
            _ => 3,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Plugin => "plugin",
            Self::Licensed => "licensed",
            Self::Library => "library",
            Self::Server => "server",
            Self::Catalogue => "catalogue",
            Self::Demo => "demo",
        }
    }
}

/// Static description of a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderManifest {
    /// Stable machine id, used in config and as the health/cache key.
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub kind: ProviderKind,
    /// Hosts this provider is permitted to contact.
    ///
    /// For WASM plugins the host *enforces* this allowlist on every `fetch`, which is
    /// the main containment for what is otherwise a supply-chain surface: a plugin
    /// registry for sources means running other people's code.
    pub allowed_hosts: Vec<String>,
    pub translations: Vec<Translation>,
    /// Whether this source names torrents for the engine to play, as a plugin does by
    /// declaring the `torrents` capability. Such a source plays only while torrent playback is
    /// on, and a cast with torrents while casting off passes it over.
    pub torrents: bool,
}

/// One selectable release a provider can offer for an episode.
///
/// Deliberately descriptive rather than playable: listing must be cheap — no torrent
/// session, no stream negotiation — so a candidate carries an opaque `id` the same
/// provider can turn into streams later via [`Provider::resolve_source`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SourceCandidate {
    /// Opaque identifier, meaningful only to the provider that produced it.
    pub id: String,
    /// The provider that produced this candidate, so a pick can be routed straight
    /// back to it rather than re-entering failover.
    pub provider_id: String,
    /// The release title as listed at the source.
    pub title: String,
    /// Vertical resolution, when the listing states one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub quality: Option<u32>,
    /// One short line the source wrote about this candidate, shown under its title as written.
    /// The engine reads nothing into it and ranks nothing by it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub label: Option<String>,
    /// Payload size as listed at the source, already human-readable (`"1.4 GiB"`).
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub size: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub dual_audio: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub dubbed: bool,
    /// Whether automatic resolution would pick this one, so the list can say which
    /// release the user is currently getting.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub auto_pick: bool,
}

/// A source of episodes and streams.
#[async_trait]
pub trait Provider: Send + Sync {
    fn manifest(&self) -> &ProviderManifest;

    /// Find candidate titles. For plugin sources this is the *primary* identification
    /// path, not a fallback: a source's catalogue keys on its own opaque ids, which
    /// appear in no mapping dataset, so there is nothing to look up.
    async fn search(
        &self,
        query: &str,
        translation: Translation,
        hints: &TitleHints,
    ) -> Result<Vec<SearchHit>, ProviderError>;

    async fn episodes(
        &self,
        key: &ProviderKey,
        translation: Translation,
        hints: &TitleHints,
    ) -> Result<Vec<Episode>, ProviderError>;

    /// Resolve playable streams, best-first where the provider has an opinion.
    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
        hints: &TitleHints,
    ) -> Result<Vec<Stream>, ProviderError>;

    /// Find a title by the ids other services know it by, with no fuzzy matching. A source with
    /// its own index (a media server, the library) answers from it; the default has no index and
    /// answers `None`, so the caller falls back to [`Self::search`].
    async fn lookup(&self, _ids: &ExternalIds) -> Result<Option<SearchHit>, ProviderError> {
        Ok(None)
    }

    /// The AniList titles this source has in its own index, read with no network: what Search's
    /// Available filter keeps to and a result's "In your library" line comes from. A source with
    /// no index of its own has none.
    fn titles(&self) -> Vec<AnilistId> {
        Vec::new()
    }

    /// The selectable releases for an episode, best-first, without starting any of them.
    ///
    /// Optional, and not part of the WIT plugin interface: an empty list means "nothing
    /// to choose between here", which is the honest answer for sources that resolve to
    /// exactly one stream anyway.
    async fn sources(
        &self,
        _key: &ProviderKey,
        _episode: &str,
        _translation: Translation,
    ) -> Result<Vec<SourceCandidate>, ProviderError> {
        Ok(Vec::new())
    }

    /// Resolve one specific candidate from [`Self::sources`] by its id.
    ///
    /// The default refuses rather than falling back to [`Self::resolve`]: silently
    /// substituting the automatic pick for the one the user chose would be worse than
    /// failing.
    async fn resolve_source(
        &self,
        _key: &ProviderKey,
        _episode: &str,
        _translation: Translation,
        _source_id: &str,
    ) -> Result<Vec<Stream>, ProviderError> {
        Err(ProviderError::NotFound)
    }

    /// Cheap liveness probe for the Providers screen. Default implementation runs a
    /// throwaway search, which is a fair proxy for "is this source answering".
    async fn health_check(&self) -> Result<(), ProviderError> {
        self.search("a", Translation::Sub, &Default::default()).await.map(|_| ())
    }

    /// Whether this provider is currently willing to serve.
    ///
    /// Distinct from health: a source held back by local policy, such as the torrent
    /// protection check, is not a broken source. Returning an error keeps it out of
    /// resolution entirely.
    fn is_available(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}

/// A source of books: folders, Komga, Kavita and OPDS servers, and `kaeri:books` plugins.
///
/// The same shape as the `source` interface in `wit/kaeri-books.wit`, as [`Provider`] mirrors
/// `kaeri:provider`, so a plugin is indistinguishable from a built-in source here. The book
/// registry ranks them with the same health and failover rules as shows.
#[async_trait]
pub trait BookSource: Send + Sync {
    fn manifest(&self) -> &BookSourceManifest;

    /// Find series. For a plugin this is how a work is found at all: its ids are its own.
    async fn search(
        &self,
        query: &str,
        hints: &BookHints,
    ) -> Result<Vec<SeriesHit>, ProviderError>;

    /// One series, described.
    async fn series(&self, id: &str, hints: &BookHints) -> Result<SeriesDetail, ProviderError>;

    /// A series' chapters, in any order: every version and language the source lists.
    async fn chapters(
        &self,
        id: &str,
        hints: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError>;

    /// What chapter `chapter` of series `series` is made of: page images to fetch, text, or a
    /// whole book file.
    async fn content(
        &self,
        series: &str,
        chapter: &str,
    ) -> Result<ChapterContent, ProviderError>;

    /// The page images chapter `chapter` of series `series` is made of, found without fetching
    /// a whole book file: `None` for text, or for a chapter only a download would open. The
    /// reader's preview of the next chapter asks this. The default asks [`Self::content`], which
    /// fetches no file for a source that names pages or text; a source that downloads in
    /// `content` answers on its own.
    async fn chapter_pages(
        &self,
        series: &str,
        chapter: &str,
    ) -> Result<Option<Vec<PageRef>>, ProviderError> {
        Ok(match self.content(series, chapter).await? {
            ChapterContent::Images(pages) => Some(pages),
            ChapterContent::Text(_) | ChapterContent::Book(_) => None,
        })
    }

    /// Find a series by the ids other services know it by, with no fuzzy matching. A source
    /// with its own index (a server, a folder) answers from it; the default answers `None`, so
    /// the caller falls back to [`Self::search`].
    async fn lookup(&self, _ids: &BookIds) -> Result<Option<SeriesHit>, ProviderError> {
        Ok(None)
    }

    /// Tell the source a chapter was read, or read no more: Komga's and Kavita's progress
    /// write-back. Plain OPDS and plugins keep the default, which does nothing.
    async fn report_progress(
        &self,
        _series: &str,
        _chapter: &str,
        _read: bool,
    ) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Cheap liveness probe. The default says it is answering.
    async fn health_check(&self) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Whether local policy lets this source be asked right now: a drive that is not
    /// connected, a server signed out here. Not a health event, as for [`Provider`]; the reason
    /// is shown where the source is listed.
    fn is_available(&self) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Fetch one page image this source named, with the headers it carries. A source that
    /// names pages the host can fetch plainly (a server's, with its sign-in header) keeps the
    /// default, which the engine's own client answers; a plugin's goes through its allowlist,
    /// and a page a plugin makes itself ([`PageRef::made`]) comes from the plugin. The status is
    /// told as it came, so the registry can ask for fresh addresses on a 403 or 410.
    async fn fetch_page(&self, _page: &PageRef) -> Result<Vec<u8>, PageError> {
        Err(PageError::Unsupported)
    }
}

/// Why a page image could not be fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageError {
    /// This source does not fetch its own pages: the engine's client does.
    Unsupported,
    /// The address answered with this status. 403 and 410 mean the address has expired.
    Status(u16),
    Failed(ProviderError),
}

impl PageError {
    /// Whether asking the source for fresh addresses may help.
    pub const fn expired(&self) -> bool {
        matches!(self, Self::Status(403 | 410))
    }

    /// Whether the failure was on the way rather than the server's answer, so the same address
    /// may work a moment later: a dropped connection, a timeout, a busy or failing server.
    pub fn transient(&self) -> bool {
        match self {
            Self::Status(status) => matches!(status, 408 | 425 | 429 | 500..=599),
            Self::Failed(ProviderError::Transport(_)) => true,
            _ => false,
        }
    }
}

/// Something that can play a [`Stream`].
///
/// mpv over JSON IPC is the reference implementation, but the trait is what lets the
/// licensed path work at all: episodes of a licensed service resolve to an external deep link and
/// are opened by a handoff player, needing no new machinery anywhere else.
#[async_trait]
pub trait Player: Send + Sync {
    fn id(&self) -> &str;

    /// Whether this player can handle a given stream. The registry uses it to route an
    /// `ExternalDeepLink` to the handoff player and everything else to mpv.
    fn supports(&self, stream: &Stream) -> bool;

    /// Begin playback. Returns once playback has *started*, not once it finishes.
    async fn play(&self, stream: &Stream, session: PlaybackRequest)
    -> Result<(), crate::Error>;
}

/// Context handed to a player alongside the stream.
#[derive(Debug, Clone, Default)]
pub struct PlaybackRequest {
    /// Window/OSD title.
    pub title: String,
    /// Where to resume from, in seconds.
    pub start_at: Option<f64>,
    /// Preferred subtitle language tag.
    pub subtitle_language: Option<String>,
    /// Carried across episodes within a series so a chosen speed sticks.
    pub speed: Option<f64>,
    /// Carried across sessions so a chosen volume sticks, in mpv's 0–100 scale.
    pub volume: Option<f64>,
    /// Whether the viewer wants dubbed audio. Drives track selection: audio in their
    /// own language with signs-only subtitles, instead of original audio with full
    /// subtitles. Track order in the file never decides.
    pub dub: bool,
    /// Open the player fullscreen rather than windowed.
    pub fullscreen: bool,
    /// The viewer's own mpv options, `playback.mpv_args`, as property and value: what a player
    /// a front end hosts sets as it opens. mpv's own process takes them on its command line.
    pub mpv_options: Vec<(String, String)>,
}

/// An external service that holds watch progress.
///
/// Local SQLite history is always the source of truth; a tracker is a *projection* of
/// it. That ordering is what makes the app work with no account and no network, and it
/// is why `push` must be idempotent — the outbox will retry.
#[async_trait]
pub trait Tracker: Send + Sync {
    fn id(&self) -> &str;

    /// Whether credentials are present. `false` means degrade to local-only rather than
    /// erroring.
    fn is_authenticated(&self) -> bool;

    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, crate::Error>;

    /// Apply a batch of operations. Must be safe to call twice with the same input.
    async fn push(&self, ops: &[TrackOp]) -> Result<(), crate::Error>;

    /// [`Self::push`], saying what became of each title: applied, answered for without applying
    /// anything, or failed and why.
    ///
    /// Some services answer a write with success and a list of what they could not match, so
    /// the status alone says nothing about whether the list changed. The drain reports those
    /// titles instead of counting them as sent. A tracker that writes title by title reports a
    /// failure against its title and goes on with the rest. An `Err` is about the whole batch:
    /// the drain then asks again title by title when it was a refusal, so one bad change cannot
    /// sink the others. Default: `push`, trusting its success.
    async fn push_checked(&self, ops: &[TrackOp]) -> Result<Pushed, crate::Error> {
        self.push(ops).await.map(|()| Pushed::default())
    }

    /// Whether this tracker can name the title an op is about, with the id mappings as they
    /// stand.
    ///
    /// A tracker keyed on its own ids reaches a title through the mapping datasets, and a title
    /// they do not cover yet cannot be pushed *yet*. The drain holds such an op back until the
    /// mappings change instead of sending it, so it neither fails the batch nor is lost. Default
    /// `true` for trackers keyed on AniList ids, which can name every title.
    fn can_address(&self, _op: &TrackOp) -> bool {
        true
    }

    /// Whether this tracker keeps manga and novel lists.
    fn serves_reading(&self) -> bool {
        false
    }

    /// Whether this tracker keeps anime lists.
    fn serves_watching(&self) -> bool {
        true
    }

    /// The manga and novel list, as far as it syncs. Trackers without one answer empty.
    async fn pull_read_library(&self) -> Result<Vec<TrackedReadEntry>, crate::Error> {
        Ok(Vec::new())
    }

    /// Apply a batch of reading operations, saying what became of each work, as
    /// [`Self::push_checked`] does. Safe to call twice with the same input.
    async fn push_read(&self, _ops: &[TrackOp]) -> Result<Pushed, crate::Error> {
        Ok(Pushed::default())
    }

    /// Take a manga or novel off this tracker's list. Only the person asks for this: sync never
    /// removes anything. Trackers without a reading list refuse.
    async fn remove_read(
        &self,
        _anilist_id: crate::ids::AnilistId,
    ) -> Result<ReadRemoval, crate::Error> {
        Err(crate::Error::Tracker {
            tracker: self.id().to_owned(),
            message: "this tracker keeps no reading list".into(),
        })
    }

    /// Drop any in-memory credential.
    ///
    /// Signing out clears the stored token, but a tracker that cached one at construction would
    /// keep using it — reporting itself as connected and pushing with a credential the user has
    /// revoked. Default no-op for trackers that read the store on every call.
    async fn forget_credentials(&self) {}

    /// Take a credential obtained while the app was already running.
    ///
    /// The mirror of [`Self::forget_credentials`], and it exists for a reason worth recording:
    /// signing in used to end with "restart to start syncing", because every tracker captured its
    /// token at construction and nothing could hand it a new one. The token reached the keychain and
    /// the running session never saw it.
    async fn accept_credentials(
        &self,
        _access: &str,
        _refresh: Option<&str>,
        _expires_at: Option<i64>,
    ) {
    }
}

/// One title's state on a tracker.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedEntry {
    pub anilist_id: crate::ids::AnilistId,
    pub progress: u32,
    pub status: WatchStatus,
    pub score: Option<f32>,
}

/// A pending change to push to a tracker.
///
/// Serialisable because these are persisted in the durable outbox — progress recorded
/// while offline has to survive a process kill, so the queue lives in SQLite rather than
/// in memory.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TrackOp {
    /// Progress is monotonic, so conflicts resolve to `max(local, remote)` without
    /// needing a timestamp or user intervention.
    SetProgress {
        anilist_id: crate::ids::AnilistId,
        episode: u32,
    },
    /// Status is *not* monotonic, so this carries a timestamp for last-write-wins and
    /// genuine divergence is surfaced rather than silently resolved.
    SetStatus {
        anilist_id: crate::ids::AnilistId,
        status: WatchStatus,
        at: i64,
    },
    SetScore {
        anilist_id: crate::ids::AnilistId,
        score: f32,
        at: i64,
    },
    /// Chapters read, and volumes where the tracker has a field for them. Monotonic like
    /// `SetProgress`, run once for each.
    SetReadProgress {
        anilist_id: crate::ids::AnilistId,
        chapter: u32,
        volume: Option<u32>,
    },
    /// A reading list's status. `rereading` is Read again: MyAnimeList's `is_rereading`,
    /// Kitsu's `reconsuming`, AniList's `REPEATING`.
    SetReadStatus {
        anilist_id: crate::ids::AnilistId,
        status: WatchStatus,
        rereading: bool,
        at: i64,
    },
    SetReadScore {
        anilist_id: crate::ids::AnilistId,
        score: f32,
        at: i64,
    },
}

impl TrackOp {
    pub const fn anilist_id(&self) -> crate::ids::AnilistId {
        match self {
            Self::SetProgress { anilist_id, .. }
            | Self::SetStatus { anilist_id, .. }
            | Self::SetScore { anilist_id, .. }
            | Self::SetReadProgress { anilist_id, .. }
            | Self::SetReadStatus { anilist_id, .. }
            | Self::SetReadScore { anilist_id, .. } => *anilist_id,
        }
    }

    /// Whether this is about reading. Those wait in an outbox of their own, which an older
    /// version never opens, and the watching queue never carries them.
    pub const fn is_reading(&self) -> bool {
        matches!(
            self,
            Self::SetReadProgress { .. }
                | Self::SetReadStatus { .. }
                | Self::SetReadScore { .. }
        )
    }
}

/// Why a tracker did not apply one title's changes, in the terms the outbox acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushFailure {
    /// The tracker refused this title's change for good: an entry removed or merged, a value
    /// it does not accept. The only failure that counts toward the outbox's limit, and it is
    /// the title's alone: the rest of the batch still goes.
    Refused(String),
    /// The credential was rejected. Everything waits for a sign-in; nothing is dropped.
    Auth(String),
    /// The tracker asked for a pause. Waits, and is never counted.
    RateLimited { message: String, retry_in_secs: Option<u64> },
    /// The network, or the tracker's server, is down. Waits, is never counted and never
    /// dropped.
    Unavailable(String),
}

impl PushFailure {
    /// What an error from a tracker call means for the change it carried.
    ///
    /// Only [`crate::Error::Tracker`] is a refusal. Anything else, errors that are not the
    /// tracker's at all included (the local store, the configuration), says nothing about the
    /// change, so it waits rather than counting toward its end.
    pub fn of(error: &crate::Error) -> Self {
        match error {
            crate::Error::Tracker { message, .. } => Self::Refused(message.clone()),
            crate::Error::Auth(message) => Self::Auth(message.clone()),
            crate::Error::RateLimited { message, retry_in_secs, .. } => {
                Self::RateLimited { message: message.clone(), retry_in_secs: *retry_in_secs }
            }
            other => Self::Unavailable(other.to_string()),
        }
    }

    /// Whether this counts toward the outbox's limit on attempts.
    pub const fn counts(&self) -> bool {
        matches!(self, Self::Refused(_))
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Refused(message)
            | Self::Auth(message)
            | Self::RateLimited { message, .. }
            | Self::Unavailable(message) => message,
        }
    }

    /// The error this is, for a caller that wants one, as [`Tracker::push`] does over a
    /// checked push.
    pub fn into_error(self, tracker: &str) -> crate::Error {
        let tracker = tracker.to_owned();
        match self {
            Self::Refused(message) => crate::Error::Tracker { tracker, message },
            Self::Auth(message) => crate::Error::Auth(message),
            Self::RateLimited { message, retry_in_secs } => {
                crate::Error::RateLimited { tracker, message, retry_in_secs }
            }
            Self::Unavailable(message) => crate::Error::Unavailable { tracker, message },
        }
    }
}

/// What a tracker did with each title in a push.
///
/// Every title the batch named that this does not list was applied. A failure is per title, so
/// one change the tracker refuses never takes the others down with it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pushed {
    /// Titles the tracker answered for without applying anything: a title it could not match.
    pub ignored: Vec<crate::ids::AnilistId>,
    /// Titles it did not apply, each with why.
    pub failed: Vec<(crate::ids::AnilistId, PushFailure)>,
}

impl Pushed {
    /// Why `id` was not applied, if it was not.
    pub fn failure_for(&self, id: crate::ids::AnilistId) -> Option<&PushFailure> {
        self.failed.iter().find(|(failed, _)| *failed == id).map(|(_, failure)| failure)
    }

    /// The push as a plain result: the first failure, if any, as its error.
    pub fn into_result(self, tracker: &str) -> Result<(), crate::Error> {
        match self.failed.into_iter().next() {
            Some((_, failure)) => Err(failure.into_error(tracker)),
            None => Ok(()),
        }
    }
}

/// What taking a title off a tracker's reading list did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadRemoval {
    Removed,
    /// The list did not hold it.
    NotOnList,
    /// Which of the tracker's entries this is was never learned, so nothing was asked.
    Unmatched,
}

/// One manga's or novel's state on a tracker.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedReadEntry {
    pub anilist_id: crate::ids::AnilistId,
    pub medium: crate::media::Medium,
    pub chapters: u32,
    /// `None` where the tracker has no volume field (Kitsu).
    pub volumes: Option<u32>,
    pub status: WatchStatus,
    pub rereading: bool,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WatchStatus {
    Current,
    Planning,
    Completed,
    Paused,
    Dropped,
    Repeating,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_ops_expose_their_subject_uniformly() {
        let id = crate::ids::AnilistId::new(154_587);
        let ops = [
            TrackOp::SetProgress { anilist_id: id, episode: 12 },
            TrackOp::SetStatus { anilist_id: id, status: WatchStatus::Current, at: 0 },
            TrackOp::SetScore { anilist_id: id, score: 9.0, at: 0 },
            TrackOp::SetReadProgress { anilist_id: id, chapter: 40, volume: Some(4) },
            TrackOp::SetReadStatus {
                anilist_id: id,
                status: WatchStatus::Current,
                rereading: true,
                at: 0,
            },
            TrackOp::SetReadScore { anilist_id: id, score: 8.0, at: 0 },
        ];
        for op in &ops {
            assert_eq!(op.anilist_id(), id);
        }
        assert_eq!(ops.iter().filter(|op| op.is_reading()).count(), 3);
        let json = serde_json::to_string(&ops[3]).unwrap();
        assert!(json.contains(r#""op":"set_read_progress""#), "{json}");
    }

    #[test]
    fn provider_kind_labels_are_stable() {
        // These strings appear in config files and on the Providers screen, so they are
        // part of the user-facing contract.
        assert_eq!(ProviderKind::Plugin.as_str(), "plugin");
        assert_eq!(ProviderKind::Licensed.as_str(), "licensed");
        assert_eq!(ProviderKind::Library.as_str(), "library");
        assert_eq!(ProviderKind::Server.as_str(), "server");
        assert_eq!(ProviderKind::Catalogue.as_str(), "catalogue");
        assert_eq!(ProviderKind::Demo.as_str(), "demo");
    }

    #[test]
    fn first_party_kinds_are_tried_before_plugins_and_the_demo_last() {
        let tiers: Vec<u8> = [
            ProviderKind::Library,
            ProviderKind::Server,
            ProviderKind::Catalogue,
            ProviderKind::Plugin,
            ProviderKind::Demo,
        ]
        .map(ProviderKind::tier)
        .to_vec();
        assert!(tiers.windows(2).all(|pair| pair[0] < pair[1]), "{tiers:?}");
        assert!(ProviderKind::Server.is_first_party());
        assert!(!ProviderKind::Plugin.is_first_party());
    }
}
