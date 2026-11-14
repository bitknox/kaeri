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
    /// Sources passed over because they are resting after recent failures. Said once, before
    /// the first is asked, and only when there are any.
    SourcesResting { names: Vec<String> },
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
    /// The names of the sources that were asked, in the order they were asked: what a failure
    /// says as "Alpha and Brook were asked." Empty when none was asked.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub asked: Vec<String>,
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
    /// A resting source is passed over and the first of two is asked.
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
