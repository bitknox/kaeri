//! Filler and recap episodes: the kinds an episode can have and the list that holds them for one
//! show.
//!
//! Only worth having for long-running shows, which is exactly where it earns its place: nobody
//! needs this for a twelve-episode season, and everybody wants it for a show past its thousandth
//! episode.

use std::collections::BTreeSet;

/// What kind of episode this is, in a filler index's taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EpisodeKind {
    /// Adapted from the manga. The default assumption for anything unlisted.
    MangaCanon,
    /// Canon material with filler padding. **Not skippable** — skipping loses story.
    MixedCanonFiller,
    /// Anime-original filler. The thing people want to skip.
    Filler,
    /// Anime-original but canon within the anime's continuity. Also not skippable.
    AnimeCanon,
}

impl EpisodeKind {
    /// Whether offering to skip this is reasonable.
    ///
    /// Only pure filler. `mixed_canon/filler` is the trap: it *contains* filler, but skipping it
    /// loses canon story — treating it as skippable would silently cost the viewer plot.
    pub const fn is_skippable(self) -> bool {
        matches!(self, Self::Filler)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::MangaCanon => "canon",
            Self::MixedCanonFiller => "mixed",
            Self::Filler => "filler",
            Self::AnimeCanon => "anime canon",
        }
    }

    pub const ALL: [Self; 4] =
        [Self::MangaCanon, Self::MixedCanonFiller, Self::Filler, Self::AnimeCanon];
}

/// One show's episode classification.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FillerList {
    pub filler: BTreeSet<u32>,
    pub mixed: BTreeSet<u32>,
    pub anime_canon: BTreeSet<u32>,
    pub manga_canon: BTreeSet<u32>,
}

impl FillerList {
    /// What kind of episode this is, or `None` if the list says nothing about it.
    ///
    /// `None` rather than a guess: a show whose list is incomplete should produce no prompt, not a
    /// confident wrong one.
    pub fn kind_of(&self, episode: u32) -> Option<EpisodeKind> {
        // Ordered by how much it matters to get right: filler first, because that is the only one
        // that changes behaviour.
        if self.filler.contains(&episode) {
            Some(EpisodeKind::Filler)
        } else if self.mixed.contains(&episode) {
            Some(EpisodeKind::MixedCanonFiller)
        } else if self.anime_canon.contains(&episode) {
            Some(EpisodeKind::AnimeCanon)
        } else if self.manga_canon.contains(&episode) {
            Some(EpisodeKind::MangaCanon)
        } else {
            None
        }
    }

    pub fn is_skippable(&self, episode: u32) -> bool {
        self.kind_of(episode).is_some_and(EpisodeKind::is_skippable)
    }

    pub fn is_empty(&self) -> bool {
        self.filler.is_empty()
            && self.mixed.is_empty()
            && self.anime_canon.is_empty()
            && self.manga_canon.is_empty()
    }

    /// Total episodes classified, for the Title screen's summary line.
    pub fn classified(&self) -> usize {
        self.filler.len() + self.mixed.len() + self.anime_canon.len() + self.manga_canon.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> FillerList {
        FillerList {
            manga_canon: BTreeSet::from([1, 2, 3, 9]),
            mixed: BTreeSet::from([4, 5]),
            filler: BTreeSet::from([6, 7, 8, 11]),
            anime_canon: BTreeSet::from([10]),
        }
    }

    #[test]
    fn every_category_is_counted() {
        // 4 canon + 2 mixed + 4 filler + 1 anime canon.
        assert_eq!(list().classified(), 11);
    }

    #[test]
    fn only_pure_filler_is_offered_as_skippable() {
        // The trap this exists to avoid: mixed episodes contain filler, but skipping them
        // loses canon story. Treating them as skippable would silently cost the viewer plot.
        let list = list();
        assert!(list.is_skippable(6), "episode 6 is pure filler");
        assert!(!list.is_skippable(4), "episode 4 is mixed, skipping it loses story");
        assert!(!list.is_skippable(10), "anime canon is canon");
        assert!(!list.is_skippable(1));
    }

    #[test]
    fn an_unlisted_episode_has_no_opinion_rather_than_a_guess() {
        // A show whose list is incomplete should produce no prompt, not a confident wrong one.
        let list = list();
        assert_eq!(list.kind_of(999), None);
        assert!(!list.is_skippable(999));
    }
}
