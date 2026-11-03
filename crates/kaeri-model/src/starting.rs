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
}

/// Why a start failed, for the failure screen to say and to pick its actions by. `text` is the
/// message the alert notice carries.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StartFailure {
    pub why: crate::party::NoSourceWhy,
    pub text: crate::Msg,
}
