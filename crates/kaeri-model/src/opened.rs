//! A magnet link or torrent file the viewer opened themselves, and what the engine guesses it
//! is: a title and an episode read from its name, for the viewer to confirm, change or play
//! without tracking.
//!
//! The flow, in [`Command::Opened`] commands and events:
//!
//! 1. [`OpenedCommand::OpenLink`] or [`OpenedCommand::OpenTorrentFile`]; the engine answers with
//!    [`Event::LinkOpened`], or a notice when the input is not a magnet link, an info hash or a
//!    torrent file.
//! 2. The front end asks "Is this {title}, episode {N}?" with Change and Play without tracking.
//!    Change lists [`OpenedLink::candidates`], and [`OpenedCommand::Find`] searches other words,
//!    answered with [`Event::OpenedChoices`].
//! 3. [`OpenedCommand::Play`] plays it as that episode: history, resume, trackers and sync, as
//!    any episode. [`OpenedCommand::Download`] queues it for offline. Or
//!    [`OpenedCommand::PlayUntracked`] plays it with nothing recorded.
//!
//! [`Command::Opened`]: crate::command::Command::Opened
//! [`Event::LinkOpened`]: crate::event::Event::LinkOpened
//! [`Event::OpenedChoices`]: crate::event::Event::OpenedChoices

use kaeri_core::ids::AnilistId;

use crate::catalog::Entry;

/// The id a playback played without tracking goes by in its events, in place of a title's:
/// no AniList title has it. A front end shows such a playback by the name it was staged with,
/// and opens no title page for it.
pub const UNTRACKED: AnilistId = AnilistId::new(0);

/// What the engine read from a link the viewer opened.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct OpenedLink {
    /// The magnet link to hand back in the commands that play or download it. A torrent file
    /// becomes the magnet link for what it holds, with its trackers.
    pub link: String,
    /// The name the link gives itself (a file's name, or a season's folder), when it gives one.
    /// A bare info hash has none.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub name: Option<String>,
    /// The engine's guess, when the name matched a title well enough to ask about it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub guess: Option<OpenedGuess>,
    /// Titles the name could be, best first, the guess's own among them: what Change lists.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub candidates: Vec<Entry>,
    /// Whether torrent playback is on. While it is off the front end offers to turn it on in
    /// place of Play, and the play and download commands are refused with a notice.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub playback_on: bool,
}

/// A title and episode a link's name was matched to.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct OpenedGuess {
    pub title: Entry,
    /// The episode the name states. `None` for a name that states none, as a season's folder
    /// has: the front end asks which episode with the title confirmed.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episode: Option<String>,
}

/// What to do with a link the viewer opened, sent as
/// [`Command::Opened`](crate::command::Command::Opened).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum OpenedCommand {
    /// Read a magnet link, an info hash or, on a desktop, a torrent file's path the viewer
    /// opened, and guess what it is from its name: answered with
    /// [`Event::LinkOpened`](crate::event::Event::LinkOpened).
    OpenLink { input: String },
    /// [`OpenedCommand::OpenLink`] for a torrent file's bytes, as a document picker or a share
    /// sheet hands them over.
    OpenTorrentFile { bytes: Vec<u8> },
    /// Change: search the catalogue for `query`, for the title a link the viewer opened is.
    /// Answered with [`Event::OpenedChoices`](crate::event::Event::OpenedChoices).
    Find { link: String, query: String },
    /// Play an opened link as episode `episode` of title `id`, as the viewer confirmed it:
    /// history, resume, trackers and sync, as any episode. `title` names it while it opens.
    Play { link: String, id: AnilistId, episode: String, title: String },
    /// Play an opened link with nothing recorded: no history, resume point, tracker progress,
    /// sync or presence. `name` names it in the player.
    PlayUntracked { link: String, name: String },
    /// Queue an opened link for offline as episode `episode` of title `id`, as any episode is.
    Download { link: String, id: AnilistId, episode: String },
}

impl OpenedCommand {
    /// What the status line says while it runs.
    pub fn activity(&self) -> String {
        match self {
            Self::OpenLink { .. } | Self::OpenTorrentFile { .. } => "reading the link".into(),
            Self::Find { query, .. } => format!("searching for \u{201c}{query}\u{201d}"),
            Self::Play { episode, .. } => format!("starting ep {episode}"),
            Self::PlayUntracked { .. } => "starting the link".into(),
            Self::Download { episode, .. } => format!("queueing ep {episode}"),
        }
    }
}
