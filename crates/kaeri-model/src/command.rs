//! Commands: the work a front end asks for, and the controls it sends to whatever is playing.

use kaeri_core::ids::AnilistId;

use crate::{
    catalog::LibrarySegment,
    episodes::EpisodeRow,
    hosted::{PlayerChoice, PlayerReport, StatsAbout},
    playback::{PlaybackId, SkipEdge, SkipKind, Successor},
    proxy::TorrentProxy,
    search::SearchQuery,
    settings::SettingKey,
    show::ShowSettingKey,
    stores::{InviteLasts, InviteUses, Secret},
};

impl Command {
    /// Play `episode` of `id` from where it was left, or the start when it was not: see
    /// [`Command::Play`].
    pub fn play(id: AnilistId, episode: impl Into<String>) -> Self {
        Self::Play { id, episode: episode.into(), from_start: false }
    }

    /// Play `episode` of `id` with a party set up in the settings: see [`Command::PlayParty`].
    pub fn play_party(id: AnilistId, episode: impl Into<String>) -> Self {
        Self::PlayParty { id, episode: episode.into(), server: None, room: None, name: None }
    }

    /// What to call this in the activity row, or `None` for work nobody waits on: writes that
    /// land in the outbox, player commands, and anything that finishes as it is asked.
    pub fn activity(&self) -> Option<String> {
        Some(match self {
            Self::LoadContinue => "loading continue watching".into(),
            Self::LoadSeasonal => "loading this season".into(),
            Self::LoadCalendar => "loading the calendar".into(),
            Self::LoadCalendarPast { .. } => "loading earlier broadcasts".into(),
            Self::RefreshCalendar => "loading the calendar".into(),
            Self::LoadCalendarAhead { .. } => "loading later broadcasts".into(),
            // A live search has no job: it is sent on every keystroke, and most are replaced
            // before they ask anything.
            Self::Search { query } | Self::SearchMore { query, .. } if query.is_browse() => {
                "browsing".into()
            }
            Self::Search { query } | Self::SearchMore { query, .. } => {
                format!("searching \u{201c}{}\u{201d}", query.trimmed_text())
            }
            Self::LoadBrowse => "loading trending titles".into(),
            Self::LoadUpNext => "finding what is up next".into(),
            Self::LoadHome { .. } => "loading home".into(),
            Self::LoadDetail(_) => "loading the title".into(),
            Self::LoadCards { .. } | Self::PrepareCards { .. } => "loading hover cards".into(),
            Self::LoadEpisodes(_) | Self::LoadStills(_) => "loading episodes".into(),
            Self::ClearMatch { .. } | Self::FixMatch { .. } => "rematching sources".into(),
            Self::CheckProviders => "checking providers".into(),
            Self::LoadLibrary(segment) => format!("loading your {} list", segment.label()),
            Self::SyncNow => "syncing".into(),
            Self::Connect { tracker } | Self::FinishSignIn { tracker, .. } => {
                format!("signing in to {tracker}")
            }
            Self::ConnectKitsu { .. } => "signing in to kitsu".into(),
            Self::SyncSignIn { .. } | Self::FinishSyncSignIn { .. } => {
                "signing in to a cloud drive".into()
            }
            Self::Play { episode, .. } | Self::PlaySource { episode, .. } => {
                format!("starting ep {episode}")
            }
            Self::PlayParty { episode, .. } => format!("starting ep {episode} for the party"),
            Self::CastTo { episode, .. } => format!("casting ep {episode}"),
            Self::LoadSources { episode, .. } => format!("listing sources for ep {episode}"),
            Self::LoadSourceOptions(_) => "checking sources".into(),
            Self::PinRelease { .. } => "choosing a release".into(),
            Self::ManualSearch { query, .. } => {
                format!("searching sources for \u{201c}{query}\u{201d}")
            }
            Self::Opened(command) => command.activity(),
            Self::Reading(command) => return command.activity(),
            Self::Reader(command) => return command.activity(),
            Self::BookSources(command) => return command.activity(),
            Self::DownloadEpisode { episode, .. } => format!("queueing ep {episode}"),
            Self::DownloadMany { episodes, .. } => {
                format!("queueing {}", crate::wording::plural(episodes.len(), "episode"))
            }
            Self::PlayTrailer { .. } => "opening the trailer".into(),
            Self::LoadStores => "checking plugin stores".into(),
            Self::PlayDemo => "starting the demo".into(),
            Self::PreviewStore { .. } => "checking the invite".into(),
            Self::JoinStore { .. } => "joining the store".into(),
            Self::LeaveStore { .. } => "leaving the store".into(),
            Self::InstallPlugin { plugin, .. } => format!("installing {plugin}"),
            Self::CreateInvite { .. } => "making an invite".into(),
            Self::LoadMembers { .. } => "loading members".into(),
            Self::RemoveMember { .. } => "removing access".into(),
            Self::LoadLocalPlugins { .. } => "reading local plugins".into(),
            Self::PublishPlugin { plugin, .. } => format!("publishing {plugin}"),
            Self::UnpublishPlugin { plugin, .. } => format!("withdrawing {plugin}"),
            Self::OpenAdmin { .. } => "opening the admin page".into(),
            Self::SetOpenJoin { open: true, .. } => "opening the store to its link".into(),
            Self::SetOpenJoin { open: false, .. } => "limiting the store to invites".into(),
            Self::RemoveAnonymous { .. } => "removing devices".into(),
            _ => return None,
        })
    }
}

/// Asynchronous work requested by the reducer.
// uniffi packs this enum's names, types and `///` docs into one 32 KiB metadata buffer, and it
// is full: a new group of commands goes in an enum of its own, as `Player` and `Opened` do. The
// longest variant docs are plain `//` comments for the same reason.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Command {
    /// Resolve an episode's streams ahead of playing it, quietly, for a [`Command::Play`] of the
    /// same episode to pick up.
    ///
    /// The next episode the playing one rolls on to is found ahead by the engine itself, when
    /// the playback says its ending has begun ([`Event::PrefetchNext`]): a front end never has
    /// to ask for that one.
    ///
    /// [`Event::PrefetchNext`]: crate::event::Event::PrefetchNext
    Prefetch {
        id: AnilistId,
        episode: String,
    },
    /// What you were watching, from local history.
    ///
    /// Trending and continuing are different questions and the app has separate sections for
    /// each; one must not stand in for the other.
    LoadContinue,
    LoadSeasonal,
    LoadCalendar,
    /// [`Command::LoadCalendar`] that skips the window the engine keeps for ten minutes and asks
    /// AniList again: for a refresh the person asked for.
    RefreshCalendar,
    // One page further back in the calendar, asked for because the user scrolled near the top.
    //
    // Carries the anchor the screen was loaded against rather than letting the fetch read the
    // clock again. Page numbers only mean anything relative to a fixed upper bound: re-reading
    // `now` between pages would shift every boundary and let a broadcast appear twice.
    //
    // With `through`, the job keeps walking back from `page` until a broadcast at or before that
    // instant (epoch seconds) is loaded, the floor is reached or AniList has no more, and
    // answers once with every page it fetched: [`Event::CalendarBackfill::page`] is the last.
    // A week grid passes the Monday it is stepping to. At most 16 pages are walked a job; the
    // answer says whether more is behind.
    //
    // [`Event::CalendarBackfill::page`]: crate::event::Event::CalendarBackfill
    LoadCalendarPast {
        anchor: i64,
        page: u32,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        through: Option<i64>,
    },
    /// One page further ahead in the calendar, mirroring [`Command::LoadCalendarPast`]. The
    /// ceiling is fixed at `anchor` plus 28 days, so page numbers stay stable for as long as the
    /// anchor holds. `through` walks pages until a broadcast at or after that instant is loaded,
    /// the ceiling is reached or AniList has no more.
    LoadCalendarAhead {
        anchor: i64,
        page: u32,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        through: Option<i64>,
    },
    /// Search now: Return in the field, a recent search, a rail's See All. Always runs, even
    /// when the AniList budget is low. Answered with [`Event::SearchPaging`] and then the
    /// [`Event::Content`] for [`ContentFor::Search`], and remembered as a recent search.
    ///
    /// [`Event::SearchPaging`]: crate::event::Event::SearchPaging
    /// [`Event::Content`]: crate::event::Event::Content
    /// [`ContentFor::Search`]: crate::catalog::ContentFor::Search
    Search {
        query: SearchQuery,
    },
    /// Search as the query changes: sent on every keystroke and every filter change. The engine
    /// waits until the changes stop (400 ms after the text changed, 250 ms after a filter did)
    /// and searches once, and a newer one replaces any still waiting. Text shorter than two
    /// characters (one for Chinese, Japanese and Korean) is not searched yet. While the AniList
    /// budget is low, text changes wait for a [`Command::Search`], and
    /// [`Event::LiveSearch`](crate::event::Event::LiveSearch) says so.
    SearchLive {
        query: SearchQuery,
    },
    /// The next page of the results on screen, asked for as the end comes near. Answered with
    /// [`Event::SearchMore`](crate::event::Event::SearchMore).
    SearchMore {
        query: SearchQuery,
        page: u32,
    },
    /// Search's start, as rails, in one request: answered with
    /// [`Event::Browse`](crate::event::Event::Browse), at once from what was kept and again once
    /// AniList answers.
    LoadBrowse,
    /// "Up next for you" under Continue: send it after [`Command::LoadContinue`]. Answered with
    /// [`Event::UpNext`](crate::event::Event::UpNext), at once from what was kept for half an
    /// hour and otherwise after a few lookups by id. Never sends more than the queue leaves
    /// room for, so every app shows the same count.
    LoadUpNext,
    /// Not interested in a suggestion. It stays hidden until it starts airing; a title that
    /// already airs or has finished stays hidden. The reducer drops the row first; this
    /// persists the dismissal.
    DismissUpNext {
        id: AnilistId,
    },
    /// Undo of [`Command::DismissUpNext`], for a notice with Undo: the title may be suggested
    /// again, and a fresh [`Event::UpNext`](crate::event::Event::UpNext) follows.
    RestoreUpNext {
        id: AnilistId,
    },
    /// What the filters choose from: answered with
    /// [`Event::Facets`](crate::event::Event::Facets). Asked of AniList at most once a week.
    LoadSearchFacets,
    /// Home, answered with `Event::Home`, again as each part lands. `refresh` asks afresh.
    LoadHome {
        refresh: bool,
    },
    /// Whether a source has title `id`: answered with `Event::TitlePlayable`.
    ProbeTitle {
        id: AnilistId,
    },
    /// The hidden titles, answered with `Event::HiddenTitles`.
    LoadHiddenTitles,
    /// The recent searches, answered with
    /// [`Event::RecentSearches`](crate::event::Event::RecentSearches).
    LoadRecentSearches,
    /// Keep a search among the recent ones, as when a title is opened from a live search's
    /// results. A [`Command::Search`] is kept by itself.
    RememberSearch(SearchQuery),
    ForgetRecentSearch(SearchQuery),
    ClearRecentSearches,
    LoadDetail(AnilistId),
    /// What a hover card or preview shows for these covers (at most fifty): the synopsis, the
    /// trailer and the airing and episode facts. Answered with one [`Event::Cards`], at once from
    /// what lists already brought and the metadata cache kept, and for the titles that
    /// leaves out by one batched lookup at low priority. Costs no request on a hover when the
    /// list that showed the cover carried them. The page still asks [`Command::LoadDetail`] on
    /// opening.
    ///
    /// [`Event::Cards`]: crate::event::Event::Cards
    LoadCards {
        ids: Vec<AnilistId>,
    },
    /// The covers a screen shows (at most fifty), for their hover cards: [`Command::LoadCards`]
    /// with what each title's episode table needs from AniList, in one batched request at low
    /// priority. Answered with [`Event::Cards`]; a [`Command::LoadEpisodes`] or
    /// [`Command::LoadStills`] for one of them then asks AniList nothing. Sent when the covers
    /// on screen change, so a hover costs no request once a screen is loaded.
    ///
    /// [`Event::Cards`]: crate::event::Event::Cards
    PrepareCards {
        ids: Vec<AnilistId>,
    },
    /// Ask the signed-in AniList viewer's title language, answered by
    /// [`Event::ViewerTitleLanguage`](crate::event::Event::ViewerTitleLanguage). Costs no request
    /// when the viewer was already looked up for their library.
    LoadTitleLanguage,
    LoadEpisodes(AnilistId),
    /// [`Command::LoadEpisodes`] for the stills that stand in for a missing banner on a card
    /// nobody has opened: the same answer, asked of AniList at low priority so a screen's own
    /// requests go first.
    LoadStills(AnilistId),
    // How far a title's dub goes, for the title page's "Dub to episode 6": answered by one
    // [`Event::TitleDub`](crate::event::Event::TitleDub), with no job. Sent when the page
    // opens, beside [`Command::LoadEpisodes`], and again when it comes back into view.
    //
    // Never in the way of the page: it waits for the title's episode table to load when one is
    // loading, so the two never ask the source at once, and a play, or the page of another
    // title asking, stops it. It lists the title in the dub from the source a dubbed play would
    // use, the show's own pick where it has one, within 30 seconds, and the answer is kept for
    // the title and that source: six hours for a show still airing, a day for any other, and a
    // quarter of an hour for a source that could not say. Asked again within that, it answers
    // from what was kept and asks nothing; asked while offline, it answers from what was kept
    // or with `None`, and asks again once back.
    LoadDub(AnilistId),
    /// Trending over `period`, for Search's Trending rail and its period menu: answered by one
    /// [`Event::Trending`](crate::event::Event::Trending), with no job. The choice is remembered
    /// on this device, and Search's start shows the rail for it. Today is AniList's live
    /// trending; a week or a month is summed from its daily figures, one request for the week
    /// and two for the month, kept for six hours and a day. A kept answer shows at once, with its
    /// age, and is asked again behind the screen once old. Never sent at launch.
    LoadTrending(crate::browsing::TrendPeriod),
    /// Surprise me, or Another: pick a title from `pool`, and answer with one
    /// [`Event::Surprise`](crate::event::Event::Surprise) that says which to open. The pick skips
    /// titles completed, dropped, hidden or adult (adult unless the setting is on), and the last
    /// twenty picks of the session. The Planning list asks nothing; the filters ask AniList for
    /// how many titles match, then for one page of them.
    Surprise(crate::browsing::SurprisePool),
    CheckProviders,
    /// Check the torrent proxy again now, as Sources' Check now asks after a check failed.
    /// Answered with [`Event::TorrentProxy`](crate::event::Event::TorrentProxy): `Checking`,
    /// then what it found, or `None` when torrents are off. Torrents start once it passes.
    CheckProxy,
    /// Fetch one segment of the tracker's list.
    LoadLibrary(LibrarySegment),
    /// Drain the outbox and pull the library now, rather than waiting for the interval.
    SyncNow,
    /// The window is on screen (`true`) or hidden or minimised (`false`): for a desktop app,
    /// which has no [`Command::AppPhase`] to send because downloads and torrents go on while
    /// the window is away. Only the tracker pull loop rests while hidden, and one pull runs on
    /// return. Nothing else is held.
    SyncVisible {
        visible: bool,
    },
    /// Set a title's list status.
    SetStatus {
        id: AnilistId,
        status: LibrarySegment,
    },
    /// Score a title out of ten.
    SetScore {
        id: AnilistId,
        score: f32,
    },
    /// Play a title's trailer: mpv when it can stream the host, the browser otherwise.
    PlayTrailer {
        url: String,
        title: String,
    },
    /// Pin a source for one title, or return it to automatic with `None`.
    SetProviderPreference {
        id: AnilistId,
        provider: Option<String>,
    },
    /// Settle a divergence by keeping one side.
    ResolveConflict {
        id: AnilistId,
        keep_local: bool,
    },
    /// Run a tracker's sign-in flow. For a device-code tracker, sent again while its code is up
    /// it asks for a new code, which replaces the one before: what "Get a new code" sends, and a
    /// TV once the code's [`DeviceCodePrompt::expires_at`] passes.
    ///
    /// [`DeviceCodePrompt::expires_at`]: crate::sync::DeviceCodePrompt::expires_at
    Connect {
        tracker: String,
    },
    /// Finish signing in to `tracker` with the redirect the system's sign-in sheet came back
    /// with, after [`Event::Authorize`](crate::event::Event::Authorize): the whole URL, its query
    /// and its fragment. A secret, since it can carry the token itself.
    FinishSignIn {
        tracker: String,
        callback: Secret,
    },
    /// The sign-in sheet for `tracker` was closed without signing in, or its device code's
    /// screen was: that flow stops, and its code comes down with `Event::DeviceCode(None)`.
    CancelSignIn {
        tracker: String,
    },
    /// Exchange Kitsu credentials for a token, from the in-app prompt.
    ConnectKitsu {
        username: String,
        password: Secret,
    },
    /// Say what the profiles are: answered with `Event::Profiles`.
    LoadProfiles,
    /// Add a profile. The first one added also makes today's data the first profile, named
    /// `first_name` when it has to be, and the new profile is switched to.
    AddProfile {
        name: String,
        kids: bool,
        first_name: Option<String>,
    },
    /// Rename a profile or change whether it is a kids profile.
    EditProfile {
        id: String,
        name: String,
        kids: bool,
    },
    /// Stop what plays, keeping its place, and use another profile. Answered with
    /// `Event::Profiles`, after which the front end reloads its screens.
    SwitchProfile {
        id: String,
    },
    /// Delete a profile and its data. The one in use cannot be deleted; switch first.
    DeleteProfile {
        id: String,
    },
    /// Say how sync stands: answered with `Event::SyncStatus`.
    LoadSync,
    /// Set sync up in an empty folder, naming this device. Answered with
    /// `Event::SyncRecoveryKey`, shown once, then `Event::SyncStatus`.
    StartSync {
        device_name: String,
        folder: crate::group::SyncFolderSpec,
    },
    /// Join the group a folder holds, with its recovery key.
    JoinSync {
        device_name: String,
        folder: crate::group::SyncFolderSpec,
        recovery_key: Secret,
    },
    /// Sign in to a cloud drive (`GoogleDrive`, `Dropbox` or `OneDrive`) and keep the sign-in,
    /// without choosing a folder: "Sign in again" after `SyncProblem::SignInNeeded`, or another
    /// account. Steps come as `Event::SyncSignIn`; when sync already uses the service it runs a
    /// pass after.
    SyncSignIn {
        service: crate::group::SyncFolderKind,
    },
    /// The redirect the web sheet ended on, after `SyncSignInStep::Sheet`: the whole URL. A
    /// secret, since it carries the authorisation code.
    FinishSyncSignIn {
        service: crate::group::SyncFolderKind,
        callback: Secret,
    },
    /// Stop the cloud sign-in under way, at any step: the sheet was closed or the code screen
    /// was left. Nothing is said as a problem.
    CancelSyncSignIn,
    /// Forget this device's sign-in to a cloud drive. When sync uses it, the folder road stops
    /// with `SyncProblem::SignInNeeded` until `SyncSignIn`.
    SyncSignOut {
        service: crate::group::SyncFolderKind,
    },
    /// Run the folder road now.
    SyncFolderNow,
    /// Leave the group: this device stops syncing and keeps its data.
    LeaveSync,
    /// Remove another device from the group.
    RemoveSyncDevice {
        device: String,
    },
    /// Make a new recovery key and rotate the group key: answered with `Event::SyncRecoveryKey`.
    NewRecoveryKey,
    /// Show a pairing code and wait for another device on the network to enter it. Sets sync up
    /// on this device first if it is not, naming it. Answered with `Event::SyncInvite`; the
    /// code is good for two minutes and three tries, and `Event::SyncStatus` follows a pairing.
    PairInvite {
        device_name: String,
    },
    /// Stop waiting for a device to pair.
    PairStop,
    /// Pair with the device showing `code` at `address` (`host:port`, from its QR code or the
    /// network), then swap histories. Sets sync up on this device first if it is not.
    PairJoin {
        device_name: String,
        address: String,
        code: String,
    },
    /// Swap histories with a device already paired, at `address`.
    SyncWithDevice {
        device: String,
        address: String,
    },
    /// Say the recovery key again, for the sheet the device's own lock guards. Answered with
    /// `Event::SyncRecoveryKey`. A TV never asks.
    ShowRecoveryKey,
    /// Close a note about a watch that was kept.
    DismissSyncNote {
        id: i64,
    },
    /// Forget a tracker's token.
    Disconnect {
        tracker: String,
    },
    /// Resolve a stream and play it. The eyecatch covers this.
    ///
    /// It opens where the episode was left, said with
    /// [`Event::Resumed`](crate::event::Event::Resumed), unless `from_start`: then it opens at
    /// the beginning, as "Play from the beginning" asks, and says nothing of where it was left.
    /// Either way the stored position follows the playback from there, as it plays.
    Play {
        id: AnilistId,
        episode: String,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        from_start: bool,
    },
    /// Cast an episode to a TV with Chromecast built in, as a play does: the streams are found
    /// here, and the TV plays what this device serves it or fetches by itself. `device` is the
    /// Cast device's TXT `id`, as the TV picker's row names it. Carried over from `position`
    /// when the episode was playing here; from where it was left otherwise. Every play after it
    /// casts to the same TV until [`Command::StopCasting`], so auto-next keeps going there.
    CastTo {
        device: String,
        id: AnilistId,
        episode: String,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        position: Option<f64>,
    },
    /// Stop casting: the TV goes back to what it showed, and plays open here again.
    StopCasting,
    /// Play an episode with a watch party: join the room on a Syncplay server, then resolve the
    /// episode from this device's own sources and play it, kept with the room. Answered with
    /// [`Event::PartyJoined`] once in the room, and [`Event::PartyLeft`] with why when the server
    /// can't be reached or refuses.
    ///
    /// An episode none of the sources has leaves this device in the room without playing, with
    /// [`Event::PartyNoSource`] for the dialog that offers Choose source; any play after that
    /// plays with the room.
    ///
    /// Each of `server`, `room` and `name` left out is the party's already joined, or else the
    /// settings': the last server used, a room set there or a new one, and the name. What the
    /// terminal sends, from its settings.
    ///
    /// [`Event::PartyJoined`]: crate::event::Event::PartyJoined
    /// [`Event::PartyLeft`]: crate::event::Event::PartyLeft
    /// [`Event::PartyNoSource`]: crate::event::Event::PartyNoSource
    PlayParty {
        id: AnilistId,
        episode: String,
        /// `host:port`, or a host on Syncplay's port.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        server: Option<String>,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        room: Option<String>,
        /// The name the room sees.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        name: Option<String>,
    },
    /// Ask for what the form that starts a party opens with: answered with
    /// [`Event::PartyForm`](crate::event::Event::PartyForm).
    LoadPartyForm,
    /// Join a party without starting anything: what is playing now plays with the room, and so
    /// does every play after, until [`Command::PartyLeave`]. What the player's people button
    /// starts. Joining another room leaves the one before.
    PartyJoin {
        server: String,
        room: String,
        /// The settings' name when `None`.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        name: Option<String>,
    },
    /// Say whether this member is ready. When every member is, play starts.
    PartyReady {
        ready: bool,
    },
    /// Leave the party. What is playing goes on from where it is, as a private session.
    PartyLeave,
    /// The lobby and the rooms kept: see [`PartyCommand`](crate::party::PartyCommand).
    Party(crate::party::PartyCommand),
    /// List the selectable releases for one episode, for the Sources overlay.
    LoadSources {
        id: AnilistId,
        episode: String,
    },
    /// List the sources that could serve a title with how many of its episodes each has, for
    /// the source picker. Answered with [`Event::SourceOptions`](crate::event::Event::SourceOptions).
    /// Picking one is [`Command::SetProviderPreference`]; one of a source's releases is
    /// [`Command::PinRelease`]; `SetProviderPreference` with `None` returns to automatic.
    LoadSourceOptions(AnilistId),
    /// Pin one release of a source for a title, such as a batch among a torrent source's
    /// results: the source is pinned as by [`Command::SetProviderPreference`], and the release
    /// is played for every episode it holds. The others come from the other sources, in order.
    PinRelease {
        id: AnilistId,
        provider_id: String,
        release_id: String,
        /// The release as the source titles it, kept to say what is pinned.
        title: String,
    },
    /// Search providers with the user's own words when the automatic match is wrong.
    /// Results come back as [`Event::MatchChoices`](crate::event::Event::MatchChoices),
    /// reusing the Disambiguate flow.
    ManualSearch {
        id: AnilistId,
        query: String,
        /// The source to search. `None` searches the sources in order and answers from the
        /// first that replies.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        provider_id: Option<String>,
    },
    /// Forget the pinned match and cached resolution for a title, then re-resolve
    /// automatically — the undo for [`Command::FixMatch`].
    ClearMatch {
        id: AnilistId,
    },
    /// Mark episodes watched or not: written to history, with the tracker push queued when
    /// marking, then answered with [`Event::Progress`](crate::event::Event::Progress) for the
    /// title, as the store then holds it, when the engine knows the title.
    SetWatched {
        id: AnilistId,
        episodes: Vec<String>,
        watched: bool,
    },
    /// Take a title off the CONTINUE rail. The reducer drops the row first, so the list
    /// answers immediately; this persists the dismissal. History is untouched, and the
    /// next recorded watch brings the title back.
    HideFromContinue {
        id: AnilistId,
    },
    /// The licensed services page and the links to them. See [`LicensedCommand`].
    Licensed(LicensedCommand),
    // Media servers; see `ServerCommand`. A plain comment: variant docs fill uniffi's buffer.
    Server(crate::servers::ServerCommand),
    // Your media on this computer; see `LocalCommand`.
    Local(crate::local::LocalCommand),
    // Catalogue, series, library, updates, trackers, downloads, export, feed; see
    // `ReadingCommand`.
    Reading(crate::reading::ReadingCommand),
    // One open reader session; see `ReaderCommand`.
    Reader(crate::reader::ReaderCommand),
    // Folders, servers and book plugins; see `BookSourceCommand`.
    BookSources(crate::book_sources::BookSourceCommand),
    // Watch, read or both; see `MediumCommand`.
    Medium(crate::medium::MediumCommand),
    /// Open a URL in the system browser.
    OpenExternal {
        url: String,
    },
    /// Open the discussion of `episode` of `media`: AniList's forum thread for it, answered
    /// with [`Event::Discussion`] as it loads and on every change after, until
    /// [`Command::CloseDiscussion`]. One at a time: opening another closes this one.
    ///
    /// Refused, with [`DiscussionState::NoThread`] and no request, while spoiler protection is
    /// on for the show and the episode is after the viewer's progress, and while discussions
    /// are switched off in Settings. Costs AniList one request, or two when the thread was not
    /// looked up in the last hour. Sending it again is Try again.
    ///
    /// [`Event::Discussion`]: crate::event::Event::Discussion
    /// [`DiscussionState::NoThread`]: crate::discussion::DiscussionState::NoThread
    OpenDiscussion {
        media: AnilistId,
        episode: u32,
    },
    /// The open discussion's next page of comments, as the viewer scrolls to the end.
    MoreComments,
    /// Post `text` to the open discussion, as a reply to comment `parent` when set. It shows at
    /// once as [`PostState::Sending`], then posted, or failed with the text kept; sending the
    /// same text again tries a failed one again.
    ///
    /// [`PostState::Sending`]: crate::discussion::PostState::Sending
    PostComment {
        parent: Option<u32>,
        text: String,
    },
    /// Put `text` in place of the viewer's own comment `id`.
    EditComment {
        id: u32,
        text: String,
    },
    /// Delete the viewer's own comment `id`, once the front end has asked. A comment that never
    /// posted is only taken away.
    DeleteComment {
        id: u32,
    },
    /// Like comment `id`, or take the like back.
    LikeComment {
        id: u32,
    },
    /// The discussion's sheet or pane closed: nothing more is said of it.
    CloseDiscussion,
    /// Play the exact release the user picked from the Sources overlay. The eyecatch
    /// covers this the same way it covers [`Command::Play`].
    PlaySource {
        id: AnilistId,
        episode: String,
        provider_id: String,
        source_id: String,
    },
    /// Something for the live mpv session.
    Player(PlayerCommand),
    /// What the player a front end hosts says of `playback`: see [`crate::hosted`]. Taken in
    /// as it is sent, ahead of the commands waiting, as mpv's reports are read off its socket.
    PlayerReport {
        playback: PlaybackId,
        report: PlayerReport,
    },
    /// Pin a title to a provider result the user picked, then load its episodes.
    FixMatch {
        id: AnilistId,
        provider_id: String,
        key: kaeri_core::ids::ProviderKey,
        /// The source's name for the entry, kept to say what the title is matched to.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        title: Option<String>,
    },
    /// Queue an episode for offline download.
    DownloadEpisode {
        id: AnilistId,
        episode: String,
    },
    /// Queue a whole range at once. The queue's own concurrency limit paces the fetches.
    DownloadMany {
        id: AnilistId,
        episodes: Vec<String>,
    },
    /// Pause or resume one download, whichever it is not.
    DownloadPause {
        id: i64,
    },
    /// Start a queued or paused download now: it goes to the front of the queue and starts as
    /// soon as the queue next looks, within a second. When the queue already runs as many as it
    /// may at once, the one that would have been last to start goes back to waiting, behind it,
    /// keeping what it fetched. A download already running, finished or failed is left as it is.
    DownloadStartNow {
        id: i64,
    },
    /// Remove a download from the queue, stopping it if it is running.
    DownloadCancel {
        id: i64,
    },
    /// Remove a download *and* delete its file from disk. Cancel keeps a finished file;
    /// this is the explicit way to not keep it.
    DownloadDelete {
        id: i64,
    },
    DownloadClearCompleted,
    /// Read the queue and publish it — for opening the screen.
    LoadDownloads,
    /// A link the viewer opened: see [`crate::opened`].
    Opened(crate::opened::OpenedCommand),
    // More sync commands; see `SyncCommand`.
    SyncGroup(crate::group::SyncCommand),
    /// Play a file already on disk — with the identity that lets it record history,
    /// resume, and sync exactly as a streamed episode would.
    PlayLocal {
        id: AnilistId,
        episode: String,
        title: String,
        path: String,
    },
    /// Step a setting through its values, toward `delta`'s sign: a ladder one rung, stopping at
    /// its ends, and a two-state value over to the other. The arrows on a Settings row, and Enter.
    ///
    /// The engine puts the new value in effect and writes it, and answers with
    /// [`Event::Settings`](crate::event::Event::Settings): what the file then holds, with the
    /// setting put back when the write was refused, and a notice saying so.
    StepSetting {
        key: SettingKey,
        delta: i64,
    },
    /// Set a setting to `value`, as the file takes it: a typed one, or the source order moved
    /// on Providers. Answered as [`Command::StepSetting`] is; a value the setting does not take
    /// changes nothing.
    SetSetting {
        key: SettingKey,
        value: kaeri_core::settings::SettingValue,
    },
    /// Sub or dub for this session only: put in effect at once and not written, so the next
    /// launch goes by the saved choice. Answered with
    /// [`Event::Settings`](crate::event::Event::Settings).
    SetTranslation {
        translation: kaeri_core::media::Translation,
    },
    /// The app moved to the foreground or the background, as the system says. In the
    /// background torrents wait, but for the episode playing when `playing`; back in the
    /// foreground the connection check runs again before they resume, a loopback server the
    /// system took is bound again on its own port, and what plays is opened again where it was.
    AppPhase {
        phase: AppState,
    },
    /// The network the device uses now, as the platform's monitor says each time it changes:
    /// `cellular`, `constrained` for Low Data Mode, and `offline` when there is no path at all.
    /// On cellular, torrents and downloads run only as `TorrentsOnCellular` and
    /// `DownloadsOnCellular` let them, the torrent source being unavailable there while the
    /// first is off; in Low Data Mode downloads and seeding wait, whatever those say.
    ///
    /// The engine owns being offline: it says each change with
    /// [`Event::Connectivity`](crate::event::Event::Connectivity), and when the path comes
    /// back it asks once more for what the screens asked for, or failed to load, while it was
    /// gone. Nothing is refused while offline. The engine counts as online until told it is not.
    NetworkPath {
        cellular: bool,
        constrained: bool,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        offline: bool,
    },
    // What the device says about itself, once at launch and on every change: the system's
    // Reduce Motion, iOS's Auto-Play Video Previews (`true` where the system has none), Low
    // Power Mode or a battery saver, a serious thermal state, a screen reader running, and a
    // low-memory device. With [`Command::NetworkPath`] and the Play trailers setting it decides
    // whether trailers start by themselves, which the engine says with
    // [`Event::Autoplay`](crate::event::Event::Autoplay). Shared with what else reads the
    // device: Picture in Picture and the gamepad read `screen_reader` and `low_power` too.
    //
    // The engine assumes a device that holds nothing back until told, so a front end that
    // plays trailers says this before its first title page settles. A field added later takes
    // a default, so a front end that does not know it keeps building.
    DeviceState {
        #[cfg_attr(feature = "ffi", uniffi(default))]
        reduce_motion: bool,
        #[cfg_attr(feature = "ffi", uniffi(default = true))]
        autoplay_previews: bool,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        low_power: bool,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        thermal_serious: bool,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        screen_reader: bool,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        low_memory: bool,
    },
    /// Route torrent traffic through `proxy`, or with `None` connect directly: the proxy's URL
    /// written to the file, its password to the host's secret store and never to the file.
    ///
    /// Answered as [`Command::SetSetting`] is, with the torrent source brought back up on what
    /// was written.
    SetTorrentProxy {
        proxy: Option<TorrentProxy>,
    },
    /// Check `proxy` before it is saved, as the torrent sheet asks: the check torrents make, made
    /// once. A proxy given with no password is checked with the one kept, as saving it would
    /// keep it.
    ///
    /// Writes nothing: not the file, not the secret store, not the torrent source, which
    /// carries on as it was. Answered, with no job, with
    /// [`Event::ProxyChecked`](crate::event::Event::ProxyChecked): `Passed` or `Failed`, and a
    /// notice saying why when the proxy cannot be checked at all, as one with no address.
    CheckTorrentProxy {
        proxy: TorrentProxy,
    },
    /// Read one show's settings sheet: answered with
    /// [`Event::ShowSettings`](crate::event::Event::ShowSettings).
    LoadShowSettings {
        id: AnilistId,
    },
    /// Set `key` for show `id` to `value`, or with `None` have it follow Settings again.
    /// Answered with the show's sheet, and the settings with their counts of shows that set
    /// each; a value the setting does not take changes nothing and is said.
    SetShowSetting {
        id: AnilistId,
        key: ShowSettingKey,
        value: Option<kaeri_core::settings::SettingValue>,
    },
    /// Put show `id` back on Settings: everything it sets for itself, the source it keeps and
    /// the tracks the player remembers for it, forgotten. Answered as
    /// [`Command::SetShowSetting`] is.
    ResetShowSettings {
        id: AnilistId,
    },
    /// Choose the subtitles show `id` plays with in `translation`, outside the player: one of
    /// the choices its sheet lists as remembered, or `None` to forget the one remembered, so
    /// Settings' subtitles apply again. The audio and the delay remembered are kept. Answered
    /// with the show's sheet.
    SetShowSubtitles {
        id: AnilistId,
        translation: kaeri_core::media::Translation,
        subtitles: Option<crate::show::RememberedSubtitles>,
    },
    /// Forget every skip time the viewer marked, and their reports and confirmations of
    /// AniSkip's times: Settings' "Clear skip times you marked". Answered with a notice of how
    /// many there were. A playback under way keeps the times it started with.
    ClearSkipMarks,
    /// Steer the playback `playback`, and only it: a command for one that is no longer the
    /// playback the engine started last is let go. What a hosted player sends, which names the
    /// playback its interface shows, so a Play Now or an Undo that arrives as one episode rolls
    /// on to the next cannot reach the next one. [`Command::Player`] steers whatever plays.
    Control {
        playback: PlaybackId,
        command: PlayerCommand,
    },
    /// Add the built-in demo to the sources, last in the chain. Written to `[sources] demo`.
    AddDemo,
    /// Take the demo out of the sources. Raises Demo removed, with Undo.
    RemoveDemo,
    /// Play part 1 of the demo's title, through the ordinary play path.
    PlayDemo,
    /// Put every subtitle setting back to its default. Raises Subtitle style reset, with Undo.
    ResetSubtitleStyle,
    /// Read the joined stores, then refresh each one's plugin list.
    LoadStores,
    /// Check a pasted invite link against the store it names, before spending it.
    PreviewStore {
        link: Secret,
    },
    /// Spend an invite link for a token and join.
    JoinStore {
        link: Secret,
        /// What the store's owner sees this device called: the confirmation's field, which
        /// starts as [`InvitedStore::device_label`]. `None`, or only spaces, for that kind of
        /// device. An open link sends no name, and this is not sent with it.
        ///
        /// [`InvitedStore::device_label`]: crate::stores::InvitedStore::device_label
        #[cfg_attr(feature = "ffi", uniffi(default))]
        label: Option<String>,
        // The joiner's name when `InvitedStore::names_joiner`; otherwise `None`.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        name: Option<String>,
    },
    /// Forget a store and revoke this device's token there.
    LeaveStore {
        store: String,
    },
    /// Install a store's plugin, or update it.
    InstallPlugin {
        store: String,
        plugin: String,
    },
    /// Delete an installed plugin from this machine.
    RemovePlugin {
        plugin: String,
    },
    /// Switch the plugin `id` off, or back on, without removing it: it keeps its place and its
    /// row, and is not tried while off. Answered with the settings, the Providers rows and a
    /// notice; anything but a plugin is refused in words.
    DisablePlugin {
        id: String,
        disabled: bool,
    },
    /// Make an invite link: for someone new with a name, for another device of yours without.
    CreateInvite {
        store: String,
        name: Option<String>,
        // For someone new; `None` is once and a week. A device link ignores both.
        #[cfg_attr(feature = "ffi", uniffi(default))]
        uses: Option<InviteUses>,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        lasts: Option<InviteLasts>,
    },
    LoadMembers {
        store: String,
    },
    RemoveMember {
        store: String,
        member: String,
    },
    CancelInvite {
        store: String,
        invite: String,
    },
    /// List local plugins an owner could publish.
    LoadLocalPlugins {
        store: String,
    },
    PublishPlugin {
        store: String,
        plugin: String,
    },
    UnpublishPlugin {
        store: String,
        plugin: String,
    },
    /// Ask a store for a one-time sign-in link and open its admin page in the browser.
    OpenAdmin {
        store: String,
    },
    /// Let anyone with the store's link join, or only people its owners invite. Opening makes a
    /// new link; going back to invites only stops new joins and keeps the devices that joined.
    /// Answered with [`Event::OpenJoin`](crate::event::Event::OpenJoin) and a notice.
    SetOpenJoin {
        store: String,
        open: bool,
    },
    /// Remove one device that joined with the store's link, or all of them when `device` is
    /// `None`. Answered with [`Event::OpenJoin`](crate::event::Event::OpenJoin) and a notice.
    RemoveAnonymous {
        store: String,
        device: Option<String>,
    },
    /// Fetch a cover or banner the front end is about to show, from the cache when it is fresh
    /// there. Answered with [`Event::ArtworkReady`](crate::event::Event::ArtworkReady), or with
    /// [`Event::ImageFailed`](crate::event::Event::ImageFailed), and never with a job: covers
    /// arrive while the screen is already usable.
    ///
    /// Asked for by the front end, because only it knows what is on screen. It asks once per
    /// URL, so a redraw never fetches what is already on its way.
    FetchArtwork {
        url: String,
    },
    // Look for episodes of the shows followed that aired since they were last looked at:
    // answered with exactly one [`Event::NewEpisodes`], empty when there is nothing new, with
    // nothing to follow, or when AniList could not be asked. Never with a job.
    //
    // The shows followed are those on the signed-in tracker's list as watching or rewatching,
    // as its last pull left the list here, and those watched here in the last 30 days, unless
    // the list has them as dropped or paused. A show planned is not, unless it turns
    // [`ShowSettingKey::NotifyNewEpisodes`] on for itself, which takes in any show; one that
    // turns it off is left out. While [`SettingKey::EpisodeNotifications`] is off, only the
    // shows switched on for themselves are followed, and with none of those either, the check
    // asks nothing and forgets what it kept.
    //
    // A show watched dubbed is said when its dub is out rather than at the broadcast, which is
    // all AniList dates: once a broadcast is ahead of the dub said, each check lists the show in
    // the dub from the source a play would use, as [`Command::LoadDub`] does, and says the
    // episodes it lists up to that broadcast, with [`Translation::Dub`]. One listing per such
    // show, cut off at 8 seconds, and none for a show whose dub has caught up.
    //
    // Each episode is said once, and only one that aired after its show was first followed:
    // the first check after notifications are turned on, or after a show is first followed,
    // only notes where things stand and says nothing, so a new install never fires a backlog.
    // An episode already watched here is not said, and nor is one that aired more than a week
    // ago, after a long time without a check.
    //
    // It costs AniList one request at most, for every show at once, and none when nothing
    // can have aired: when the last check was under five minutes ago, or when the metadata
    // cache has every show's next episode still to come. It answers within 25 seconds whatever
    // the network does, 15 for AniList and 8 for the dubs, so it fits in a background task's
    // time.
    //
    // **In the background.** On iOS, from a `BGAppRefreshTask`; on Android, from a periodic
    // `WorkManager` job, scheduled while the setting is on or any show is switched on for
    // itself, as [`SettingsView::notify_switched_on`] counts them, and cancelled only when
    // neither is. Start the engine if the app is not running, with every
    // `FfiBackground` switch off, send this, and read events until the
    // [`Event::NewEpisodes`]. Post one notification for each episode in it, identified by the
    // show and the episode, so that one posted twice replaces itself. Then send
    // [`Command::WidgetSnapshot`] and write what it answers for the widgets, before completing
    // the task. Nothing else needs to run: the check reads the list the last pull in the
    // foreground kept, and the settings and title form the engine has on file.
    //
    // [`Event::NewEpisodes`]: crate::event::Event::NewEpisodes
    // [`SettingsView::notify_switched_on`]: crate::settings::SettingsView::notify_switched_on
    // [`Translation::Dub`]: kaeri_core::media::Translation::Dub
    CheckNewEpisodes,
    /// The record the home-screen widgets show: answered with one
    /// [`Event::WidgetSnapshot`](crate::event::Event::WidgetSnapshot), never with a job.
    ///
    /// Continue's first five titles, from local history, with their titles and art from the
    /// metadata cache, asking AniList only for one the cache does not have; and the broadcasts
    /// from the start of today in the device's time zone to a week on, a request for each page
    /// of fifty, used again for five minutes. What could not be fetched is left out rather than
    /// failing the rest.
    ///
    /// Ask after anything that changes Continue, such as an episode ending, as the app goes to
    /// the background, and from the background task after [`Command::CheckNewEpisodes`].
    WidgetSnapshot,
    // What will notify of new episodes in the next seven days, and what is left out and why:
    // answered with one [`Event::UpcomingNotices`](crate::event::Event::UpcomingNotices), never
    // with a job.
    //
    // For an app that schedules its notifications ahead, to fire at air time with no code of
    // its own running then, as Apple's local notifications do; and for its Notifications pane's
    // list. The week's broadcasts are the widgets' record's, fetched once for both and used
    // again for a few minutes, so asking after [`Command::WidgetSnapshot`] costs AniList
    // nothing. The shows are those [`Command::CheckNewEpisodes`] follows with the setting on,
    // each with why: being watched, or switched on or off for itself; and whether it notifies,
    // which with the setting off only a show switched on does. Each carries the translation
    // the show is watched in: one watched dubbed is the check's to say, when its dub is out, and
    // not to schedule at the broadcast.
    //
    // Ask on launch, on coming to the foreground, after a sync or a change of progress, and
    // from the background task, after [`Command::CheckNewEpisodes`].
    UpcomingNotices,
    /// Ask GitHub for the newest release now, rather than waiting for the daily check: Settings'
    /// Check Now. Answered with [`Event::Update`](crate::event::Event::Update), `Checking` and
    /// then what it found, never with a job. Asks whether or not the daily check is switched on,
    /// and keeps what it found for the daily check to go by.
    CheckForUpdates,
}

/// What the viewer does with licensed services: the page, the title's availability, the links
/// out, and the return. Each is answered with an [`Event`](crate::event::Event) named in its
/// comment. It is its own enum because `Command` has nearly filled uniffi's metadata buffer.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum LicensedCommand {
    /// Read the Licensed services page: the viewer's choices and the services AniList lists,
    /// answered with [`Event::Licensed`](crate::event::Event::Licensed). `system_region` is the
    /// device's country code, which Region follows until the viewer chooses one. The services
    /// list is kept for 7 days.
    Load {
        #[cfg_attr(feature = "ffi", uniffi(default))]
        system_region: Option<String>,
    },
    /// Set the services the viewer pays for. Answered with
    /// [`Event::Licensed`](crate::event::Event::Licensed).
    SetServices { services: Vec<String> },
    /// Choose the region links are hidden by, or `None` to follow the system. Answered with
    /// [`Event::Licensed`](crate::event::Event::Licensed).
    SetRegion {
        #[cfg_attr(feature = "ffi", uniffi(default))]
        region: Option<String>,
    },
    /// Turn the line that names services the viewer does not use on or off. Answered with
    /// [`Event::Licensed`](crate::event::Event::Licensed).
    SetShowOthers { show: bool },
    /// Work out what title `id`'s page says about licensed services, answered with
    /// [`Event::Availability`](crate::event::Event::Availability). `plays_here` is whether the
    /// title has episodes in a source, which the page knows from its episode list.
    LoadAvailability { id: AnilistId, plays_here: bool },
    /// Open a title or an episode in a licensed service, in the system's browser or app, and
    /// remember that the viewer left for it. `episode` is the episode the link is for, or `None`
    /// for the title's link, which stands for the next one. See [`Command::ReturnedFromLicensed`].
    Open {
        id: AnilistId,
        service: String,
        url: String,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        episode: Option<u32>,
    },
    /// The app came back to the front. If the viewer opened a licensed link at least 10 minutes
    /// ago, answers with [`Event::LicensedReturn`](crate::event::Event::LicensedReturn); either
    /// way the remembered link is forgotten. Mark as watched is [`Command::SetWatched`].
    Returned,
}

/// A control sent to whatever is currently playing.
///
/// Kept as data rather than a call into the player so the reducer stays pure and testable —
/// the binary routes these to the live session.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayerCommand {
    TogglePause,
    /// Seek by a relative number of seconds; negative goes back.
    Seek(f64),
    /// Seek to an absolute position — how the skip prompt is taken.
    SeekTo(f64),
    /// Multiply the current speed by this factor, clamped by the player: a step faster or
    /// slower, as the terminal's `]` and `[` take it (1.1 and 1/1.1, mpv's own steps).
    Speed(f64),
    /// Set the speed, clamped by the player: what a player's menu of speeds picks, 1.5 for 1.5×,
    /// whatever the speed was.
    SpeedTo(f64),
    /// Step volume by a relative amount.
    Volume(f64),
    /// Leave mpv running and return to browsing.
    Detach,
    Stop,
    /// The title's episode list, loaded after playback started because the episode table on
    /// screen was not this title's — what the player's Episodes menu and Up Next read. `next`
    /// is the episode that follows the one playing, when the list has it.
    Episodes {
        rows: Vec<EpisodeRow>,
        next: Option<String>,
    },
    /// A newer playback is starting: hand the player window over to it rather than closing it.
    ///
    /// Sent by the task runner, never by a key — it is how pressing N inside the player, or
    /// picking an episode from its menu, keeps the same window instead of opening a second one.
    /// It names what is coming, so the window can say so while that is found.
    Handover(Successor),
    /// Step to the next (`1`) or previous (`-1`) episode of what is playing: the terminal's n
    /// and N. Answered by the playback, which holds the playing title's episode list.
    Step(i64),
    /// Toggle the player's fullscreen state.
    Fullscreen,
    /// Say something on the video. Sent by the task runner when the episode asked for from the
    /// player could not be found, so the viewer, who is looking at the player, learns why the
    /// episode they were watching is still on.
    Alert(String),
    /// Take the skip on offer.
    TakeSkip,
    /// Undo an automatic skip, back to `to`, where it started.
    UndoSkip {
        to: f64,
    },
    /// Play the next episode now: Up Next's button, and what a hosted player sends when its
    /// countdown reaches zero.
    PlayNext,
    /// Do not roll on when this episode ends.
    CancelNext,
    /// A pick in the player's menus.
    Choose(PlayerChoice),
    /// Skip the source being asked while a play looks for its stream: the walk moves on to the
    /// next source in the viewer's order. Heard only while the stream is being found; once the
    /// player opens, it does nothing.
    SkipSource,
    /// Begin the episode being started at 0, in place of its resume point, without asking the
    /// sources again. Heard while the walk runs (the file opens at 0), and once the file has
    /// loaded (it seeks to 0 at once). The stored position is kept until the episode has played.
    PlayFromBeginning,
    /// The stall prompt's answers: another stream, or keep waiting.
    StallSwitch,
    StallWait,
    /// Night mode on or off.
    NightMode(bool),
    /// An upscaling look, by id: `off`, `natural` or `sharp`.
    Upscaling(String),
    /// The viewer picked a track by its id in the file's `track-list`, or with `None` turned
    /// the subtitles off. The engine selects it and remembers it for the series, as it
    /// remembers a pick made with mpv's own keys, without waiting for the player to report it.
    ChooseTrack {
        kind: crate::hosted::TrackKind,
        id: Option<i64>,
    },
    /// The viewer set the subtitle delay to `seconds`. Remembered for the series once it stops
    /// moving, as a delay set with mpv's own keys is.
    SubtitleDelay {
        seconds: f64,
    },
    /// Open the file again where it plays now: the loopback server it is read from was bound
    /// again, after the system took it while the app was in the background.
    Reload,
    /// Show (`true`) or hide the Stats for nerds panel. While it shows, the engine reads the
    /// player's figures once a second and says them as `Event::PlayerStats`. The window keeps it
    /// from one episode to the next; a new window starts with it hidden.
    Stats(bool),
    /// Open the marking bar for `kind`, or with `None` for the kind the playhead's place
    /// suggests: an opening in the first third, an ending in the last. Where the viewer marked
    /// that kind before, the bar opens on the review of that mark, to edit it; where "Was that
    /// the opening?" is up, Mark it opens the review on the stretch skipped. Refused in a watch
    /// party. Answered with `Event::PlayerSkipMark`.
    BeginSkipMark {
        kind: Option<SkipKind>,
    },
    /// The marking bar's segmented control: mark `kind` instead. The edges stay.
    SetSkipMarkKind {
        kind: SkipKind,
    },
    /// Mark start, or Mark end: the edge the step names, at the playhead.
    MarkSkipEdge,
    /// Move `edge` by `by_ms` milliseconds, negative for back: the review's -1 s and +1 s and
    /// the arrow keys. Before the review, where no edge is marked to move, the playhead moves
    /// by as much instead.
    NudgeSkipEdge {
        edge: SkipEdge,
        by_ms: i64,
    },
    /// Watch the edges: play from 3 s before the start to 3 s after it, then the same around
    /// the end.
    PreviewSkipEdges,
    /// Save the mark on the review. It is used at once, and kept for this episode.
    SaveSkipMark,
    /// Esc or Back: one step back, and from the first step, close the bar.
    CancelSkipMark,
    /// The Skip times panel's Clear: forget the viewer's own mark of `kind` for this episode.
    ClearMySkip {
        kind: SkipKind,
    },
    /// Report: AniSkip's times of `kind` for this episode are wrong. They are not used on this
    /// device again.
    ReportSkip {
        kind: SkipKind,
    },
    /// The skip prompt was answered without acting on it, or went by: it comes down.
    DismissSkipPrompt,
    /// Don't ask again, on "Was that the opening?" or on the found times' card: turned off in
    /// Settings, `playback.ask_was_opening`, which covers both. "Was that the opening?" comes
    /// down; the found times' card says `SkipPrompt::FoundNever`.
    NeverAskWasOpening,
    /// Yes on the found times' card: the found stretch of `kind` becomes the person's own mark
    /// for this episode, is shared with AniSkip for this one episode, and found stretches of
    /// `kind` are skipped by themselves for this show from now on. The card says
    /// `SkipPrompt::FoundSaved`.
    ConfirmFoundSkip {
        kind: SkipKind,
    },
    /// Adjust, on the found times' card or on a found row of the Skip times panel: the marking
    /// bar opens on the review of the found stretch of `kind`, with no kind picker, and plays
    /// the edges once. Its Save confirms and shares as Yes does; Cancel leaves the found times
    /// as they were.
    AdjustFoundSkip {
        kind: SkipKind,
    },
    /// The answer to "Share your skip times with AniSkip?": Share or Keep on my devices. Kept
    /// as Settings' `playback.share_skip_times`; with Share, the mark just saved is sent.
    AnswerShareSkipTimes {
        share: bool,
    },
    /// Copy stats: the engine answers with `Event::Clipboard`, the panel's rows as plain lines,
    /// with the app's version, the system and the device from `about`.
    CopyStats {
        about: StatsAbout,
    },
}

/// Where the app is, as the system has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum AppState {
    /// On screen.
    #[default]
    Foreground,
    /// Off screen, still playing the episode on when `playing`: its audio, or a screen locked
    /// on the player.
    Background { playing: bool },
    /// Off screen while the system lets the app's downloads go on, as iOS's continued-processing
    /// task does: downloads and what plays go on, seeding waits. Downloads on a metered network
    /// wait, as on screen. Sent when the window opens, and `Background` again when it ends.
    BackgroundDownloads,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_never_renders_through_debug() {
        // The task router logs tasks on a routing mistake; this is what keeps that log
        // safe to read.
        let task = Command::ConnectKitsu {
            username: "nagare".into(),
            password: Secret("hunter2".into()),
        };
        let printed = format!("{task:?}");
        assert!(!printed.contains("hunter2"), "leaked: {printed}");
        assert!(printed.contains("redacted"));
    }
}
