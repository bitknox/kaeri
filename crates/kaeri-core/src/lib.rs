//! Domain types, traits and configuration shared by every Kaeri crate.
//!
//! The architecture layers outward from here in increasing order of volatility:
//! metadata (AniList) is stable, ID mapping is stable glue, video providers are
//! hostile and replaceable, and the player is stable again. This crate owns the
//! vocabulary all of those layers speak, and depends on none of them.

pub mod books;
pub mod config;
pub mod demo;
pub mod error;
pub mod file_name;
pub mod hash;
pub mod ids;
pub mod media;
pub mod profile;
pub mod reading_demo;
pub mod settings;
pub mod shared_settings;
pub mod stream;
pub mod track_memory;
pub mod traits;

/// The app's name, as it names itself to services outside it: the `providerName` AniSkip is
/// sent with each skip time. The one place it is written, so a rename changes it once.
pub const APP_NAME: &str = "kaeri";

pub use error::{Error, ProviderError, Result};
pub use ids::{AnidbId, AnilistId, ExternalIds, KitsuId, MalId, ProviderKey, TmdbId, TvdbId};
pub use media::{Episode, EpisodeNumber, MediaFormat, MediaStatus, SearchHit, Translation};
pub use stream::{Stream, StreamKind, Subtitle};
pub use traits::{Player, Provider, ProviderKind, ProviderManifest, Tracker};

// What the UniFFI conversions of the types the model carries register with, under the `ffi`
// feature only. It compiles under the workspace's `unsafe_code = "forbid"`, as the model's does:
// the lint is not reported inside another crate's macros, and every unsafe block and exported
// symbol here comes out of UniFFI's.
#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!();
