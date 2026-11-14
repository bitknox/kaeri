//! What the reading side of the trackers shares: the id mapping from a work to its entry on
//! each tracker, and the helpers for mapping a status.
//!
//! MyAnimeList's and Kitsu's manga ids are their own number spaces, apart from their anime
//! ids, and MangaUpdates' outgrow 32 bits. So a work has one id per tracker, kept per work and
//! tracker by the store (`read_track_map`), and an op for a work with no id yet stays queued.

use kaeri_core::ids::AnilistId;

/// A work's entry on each tracker that keys on something other than AniList's id.
///
/// A trait rather than a store handle, as [`crate::mal::IdMapping`] is, so this crate stays off
/// the engine and tests can give a fixed mapping.
pub trait ReadMapping: Send + Sync {
    fn mal_id(&self, anilist_id: AnilistId) -> Option<u32>;
    fn anilist_for_mal(&self, mal_id: u32) -> Option<AnilistId>;
    fn kitsu_id(&self, anilist_id: AnilistId) -> Option<u32>;
    fn anilist_for_kitsu(&self, kitsu_id: u32) -> Option<AnilistId>;
    fn mangaupdates_id(&self, anilist_id: AnilistId) -> Option<u64>;
    fn anilist_for_mangaupdates(&self, series_id: u64) -> Option<AnilistId>;
}

/// A mapping that knows nothing: every reading op for a tracker keyed on its own ids is held.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoReadMapping;

impl ReadMapping for NoReadMapping {
    fn mal_id(&self, _: AnilistId) -> Option<u32> {
        None
    }
    fn anilist_for_mal(&self, _: u32) -> Option<AnilistId> {
        None
    }
    fn kitsu_id(&self, _: AnilistId) -> Option<u32> {
        None
    }
    fn anilist_for_kitsu(&self, _: u32) -> Option<AnilistId> {
        None
    }
    fn mangaupdates_id(&self, _: AnilistId) -> Option<u64> {
        None
    }
    fn anilist_for_mangaupdates(&self, _: u64) -> Option<AnilistId> {
        None
    }
}
