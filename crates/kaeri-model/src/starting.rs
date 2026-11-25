//! The steps of a start: what the engine does between a Play and the first frame, said as it
//! does it, for the starting screen.
//!
//! A step is told with [`Event::StartingStep`](crate::event::Event::StartingStep), named by the
//! title and episode it belongs to. It carries the facts a screen puts on its detail line and in
//! its Details list, and, when the step changes what the step line says, that line as a message
//! id, never as English.

use kaeri_core::media::Translation;

/// Why a source came up empty, as the walk through the sources saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SourceMiss {
    /// The source has no such title.
    NoMatch,
    /// The source has the title and not this episode.
    NoEpisode,
    /// The source answered with an error.
    Failed,
    /// The source did not answer in time.
    TimedOut,
    /// The viewer skipped the source while it was being asked.
    Skipped,
}

/// Why local policy keeps a source from being asked right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SourceHold {
    /// Torrent playback is switched off, so a source that names torrents is not asked.
    TorrentsOff,
    /// Torrent playback is on and its session is still starting, so a source that names
    /// torrents is not asked yet.
    TorrentsStarting,
    /// Torrent playback is on and its session could not start, so a source that names torrents
    /// is not asked.
    TorrentsFailed,
    /// A proxy is set for torrents and its check has not passed yet.
    ProxyCheck,
    /// The device is on cellular and torrents don't run on cellular.
    Cellular,
    /// The source says it can't be used right now, for a reason of its own.
    Other,
}

/// Sources held back for the same reason, in the viewer's order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct HeldSources {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub names: Vec<String>,
    pub why: SourceHold,
}

/// Why a source listed no release of an episode, for a source picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ListingMiss {
    /// The source has no such title.
    NoMatch,
    /// The source found titles like it and could not tell which one it is.
    Ambiguous,
    /// The source has the title and lists nothing for this episode.
    NoEpisode,
    /// The source answered with an error.
    Failed,
    /// The source did not answer in time.
    TimedOut,
    /// The source was not asked: local policy holds it back.
    HeldBack { why: SourceHold },
}

/// A source that listed nothing for an episode, by the name it goes by, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MissedListing {
    pub name: String,
    pub miss: ListingMiss,
}

/// Where one source is while `LoadSources` asks it: see [`SourceListing`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ListingState {
    /// The source is being asked. A front end counts the seconds from the event in which the
    /// row first said so.
    Asking,
    /// The source listed these releases, in its own order, the automatic pick marked.
    Listed { releases: Vec<kaeri_core::traits::SourceCandidate> },
    /// The source has nothing to offer, or was not asked because local policy holds it back.
    /// A source still asked when the overall limit runs out is [`ListingMiss::TimedOut`].
    Missed { miss: ListingMiss },
    /// The source is switched off and is never asked.
    Off,
}

/// One source's row in [`Event::SourcesProgress`](crate::event::Event::SourcesProgress), said
/// once as soon as `LoadSources` is taken, with every source in the order set in Sources, switched
/// off and held back ones included, then again each time a row changes, always whole, so a front
/// end draws the last one and never merges. A source still asked when the overall limit runs out
/// is said to have taken too long. `Event::Sources` follows the last one.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SourceListing {
    pub provider_id: String,
    /// The name it goes by.
    pub name: String,
    pub state: ListingState,
}

/// One step of a start. See the module docs.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum StartStep {
    /// The title is being looked up.
    LookingUpTitle,
    /// Sources still being set up are waited for, up to 30 seconds.
    GettingSourcesReady,
    /// One source is asked for the episode. `place` counts from 1 among the `total` sources
    /// this walk will ask, in the viewer's order. `pinned` is whether the show is pinned to
    /// this source.
    AskingSource { name: String, place: u32, total: u32, pinned: bool },
    /// A source came up empty or failed, and the next is asked, if one is left.
    SourceMissed { name: String, miss: SourceMiss },
    /// Sources not asked because local policy holds them back right now, all for `why`. Said
    /// before the first source is asked, once for each reason, and only when there are any. A
    /// source held back is never said as [`StartStep::AskingSource`]. Sources resting after
    /// recent failures are not held back: they are asked last, after every other source.
    SourcesHeldBack { names: Vec<String>, why: SourceHold },
    /// A torrent from this source is looking for peers that can send its metadata (no `name`
    /// for a link the viewer opened, which has no source). The wait is at most 90 seconds. The
    /// peer count is not known until the metadata arrives, so the screen counts the time
    /// itself.
    FindingPeers { name: Option<String> },
    /// The show is pinned to this source and the episode comes from another: the pinned source
    /// does not have it yet. Said once, before [`StartStep::Found`], and never for a release the
    /// viewer picked or a show with no pin. The screen puts "`name` doesn't have episode N yet.
    /// Playing from the next source." on its detail line (`player-start-pin-missed`).
    PinMissed { name: String },
    /// The sources answered. `count` streams, the first of them from `source` (none for a link
    /// the viewer opened) at `quality` (its height in pixels, when it states one), in
    /// `translation`.
    Found { count: u32, source: Option<String>, quality: Option<u32>, translation: Translation },
    /// The first stream's subtitle files are being fetched, for at most 2 seconds.
    FetchingSubtitles,
    /// The player opens stream `attempt` of `total`, counting from 1.
    OpeningStream { attempt: u32, total: u32 },
    /// A stream showed no video and stream `next` of `total` is opened in its place.
    TryingAnotherStream { next: u32, total: u32 },
    /// The playing torrent's live peers and download speed in bytes a second, once a second
    /// until the first frame.
    Swarm { peers: u32, bytes_per_second: u64 },
    /// The download speed of a stream that is not a torrent, in bytes a second, once a second
    /// until the first frame.
    Speed { bytes_per_second: u64 },
    /// The player waits for data, `percent` full. Said only when mpv gives a number, which it
    /// does for torrents (the player pauses to fill its cache before the first frame) and not
    /// for direct streams.
    Buffering { percent: u32 },
    /// The device is offline: the start waits for a connection, up to two minutes, and carries
    /// on by itself when it is back. Said once when the wait begins. The line is the app's:
    /// `player-start-offline`.
    WaitingForConnection,
}

/// Why a start failed, for the failure screen to say and to pick its actions by. `text` is the
/// message the alert notice carries.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StartFailure {
    pub why: crate::party::NoSourceWhy,
    pub text: crate::Msg,
    /// The names of the sources that were asked, in the order they were asked. Those not
    /// unreached answered without the episode, said as "Brook doesn't have episode 7." Empty when
    /// none was asked. A source held back is
    /// never among them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub asked: Vec<String>,
    /// The names of the sources asked that gave no real answer, in the viewer's order: they
    /// failed, took too long or were skipped. When there are any, the failure says "Couldn't
    /// reach" them first and offers Try again first, since none of them said it lacks the
    /// episode. A source held back was never asked and is never among them. Empty when every
    /// source asked said it lacks the show or the episode.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub unreached: Vec<String>,
    /// The sources held back and why, said as "Harbor isn't asked while torrent playback is off."
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub held_back: Vec<HeldSources>,
}

/// A made-up start, for an app's previews, screenshots and tests: the events the engine says
/// for it, with no network or player behind them. See `fixture` in the engine's
/// `playback::starting`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum StartingFixture {
    /// The title is looked up; the episode resumes at 10:06.
    TitleLookup,
    /// Sources still loading are waited for.
    SourcesLoading,
    /// The first of three sources is asked.
    AskingFirst,
    /// The first of three is asked, and the show is pinned to it.
    AskingPinned,
    /// The first source has no such episode and the second is asked.
    SourceMissed,
    /// The first source did not answer and the second is asked.
    SourceFailed,
    /// The first source has no such episode, and a source resting after failures is asked last.
    SourcesResting,
    /// The second of three sources is still asked 24 seconds in.
    Slow,
    /// A torrent finds peers for the second source.
    FindingPeers,
    /// A torrent has peers and a speed, and the player waits for data.
    PeersKnown,
    /// Streams found: the player opens the first of three.
    Found,
    /// The first stream's subtitles are fetched.
    FetchingSubtitles,
    /// The first stream showed no video and the second of three opens.
    AnotherStream,
    /// A torrent's player waits for data, 40% full.
    BufferingTorrent,
    /// A direct stream's player waits for data and knows no percentage.
    BufferingDirect,
    /// The device is offline: the start waits for a connection, which is back at 8 s, and the
    /// first source is asked.
    Offline,
    /// The device stays offline: after two minutes the start fails with no source asked.
    OfflineTimedOut,
    /// Every source has the show and none has the episode.
    FailedNoEpisode,
    /// No source matched the show.
    FailedNoMatch,
    /// One source failed, one has no such episode, and one is held back while torrent playback
    /// is off: the start fails saying which it couldn't reach.
    FailedUnreached,
    /// No source is set up.
    FailedNoSources,
    /// The episode has not aired yet.
    FailedNotAired,
}

/// An event of a made-up start, and how many milliseconds after the start it is said.
#[derive(Debug)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FixtureEvent {
    pub at_ms: u64,
    pub event: crate::event::Event,
}
