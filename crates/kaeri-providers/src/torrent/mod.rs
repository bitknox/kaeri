//! Torrent playback.
//!
//! Kaeri provides the transport: a librqbit session, directly or through a proxy, streaming
//! over loopback HTTP. It finds no torrents. A source plugin that declares the `torrents`
//! capability names them, and [`bridge::TorrentBridge`] plays what it names; a link the person
//! opens plays the same way. Nothing here searches an index, ranks releases or picks one.
//!
//! Routed by [`crate::proxy::TorrentRoute`]. Through a proxy, no session exists, and no torrent
//! starts, until the proxy's check passes.

pub mod bridge;
pub mod cache;
pub mod http;
pub mod names;
pub mod open;
pub mod pieces;
pub mod session;

pub use bridge::{TorrentBridge, TorrentPlayback};
pub use http::{ReadySource, StreamServer, StreamSource};
pub use session::TorrentSession;
