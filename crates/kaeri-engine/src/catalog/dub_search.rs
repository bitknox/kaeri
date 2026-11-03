//! The dub ask of a search with words in it: which of its results a source lists in the dub.
//!
//! AniList does not know dubs, so each source that can search for them is asked once for the
//! words, in the dub, and its hits are matched to the results as a title page matches a source
//! ([`kaeri_meta::rank`]). A hit says the title has a dub, and, when its episode count is below
//! AniList's total, how far it goes: a count equal to the total is the show's, not the dub's, so
//! it is a "Dub" with no number. Each answer is kept in the store under the route `search`, so a
//! card says it afterwards, and a title page, which asks a source for the real extent, is not
//! mistaken by it.
//!
//! Only the first page of results is asked about, each source gets [`SOURCE_WITHIN`], and a
//! source that is slow or fails is counted in the coverage the viewer is told, never in the
//! results. Offline, or for a browse, nothing is asked and the filter uses what was stored.

use std::time::Duration;

use futures::future::join_all;
use kaeri_core::{ids::AnilistId, media::SearchHit, media::Translation};
use kaeri_meta::{Media, rank};
use kaeri_model::browsing::{DubCoverage, DubFound, DubKnown};
use kaeri_store::{DubRow, Store};

use crate::{
    data,
    dispatch::Lent,
    titles::dub::{AIRING_FOR, SETTLED_FOR},
};

/// How long a source has to answer one search.
pub const SOURCE_WITHIN: Duration = Duration::from_secs(8);

/// The route a search's answers are kept under.
pub const ROUTE: &str = "search";

/// Whether any source of the viewer's can be searched for a dub.
pub fn capable(lent: &Lent) -> bool {
    !lent.registry.dub_searchable().is_empty()
}

/// Ask the sources for `text` in the dub, and match their hits to `results`. Nothing is asked
/// when `offline`: the coverage then says so, and `found` is what the store has.
pub async fn ask(
    lent: &Lent,
    text: &str,
    results: &[Media],
    offline: bool,
    now: i64,
) -> (Vec<DubFound>, DubCoverage) {
    let sources = lent.registry.dub_searchable();
    let capable = !sources.is_empty();
    let covered = &results[..results.len().min(kaeri_model::browsing::DUB_COVERS as usize)];
    if !capable {
        return (Vec::new(), DubCoverage::of(false, 0, 0, 0, 0));
    }
    if offline {
        return (known(&lent.store, covered), DubCoverage::stored(true));
    }
    let asks = sources.iter().map(|source| async move {
        tokio::time::timeout(SOURCE_WITHIN, source.search(text, Translation::Dub)).await
    });
    let answers = join_all(asks).await;
    let asked = answers.len() as u32;
    let mut hits: Vec<SearchHit> = Vec::new();
    let mut answered = 0;
    for answer in answers {
        if let Ok(Ok(found)) = answer {
            answered += 1;
            hits.extend(found);
        }
    }
    if answered > 0 {
        for media in covered {
            keep(&lent.store, media, &hits, now);
        }
    }
    let coverage =
        DubCoverage::of(true, covered.len() as u32, covered.len() as u32, asked, answered);
    (known(&lent.store, covered), coverage)
}

/// Say `media` has a dub when a hit matches it confidently, and keep that.
fn keep(store: &Store, media: &Media, hits: &[SearchHit], now: i64) {
    let ranked = rank(&media.match_target(), hits);
    let Some(best) = ranked.first().filter(|best| best.is_confident()) else {
        // Sources answered, and none lists it: kept as an answer, for a while.
        put(
            store,
            media.id,
            DubRow {
                has_dub: false,
                through: None,
                complete: false,
                aired: None,
                said_at: now,
                until: now + SETTLED_FOR,
            },
        );
        return;
    };
    let airing = media.next_airing_episode.is_some();
    let (through, complete) = match (best.hit.episode_count, media.episodes) {
        (Some(count), Some(total)) if count < total => (Some(count), false),
        (Some(count), None) if count > 0 => (Some(count), false),
        _ => (None, false),
    };
    put(
        store,
        media.id,
        DubRow {
            has_dub: true,
            through,
            complete,
            aired: None,
            said_at: now,
            until: now + if airing { AIRING_FOR } else { SETTLED_FOR },
        },
    );
}

fn put(store: &Store, id: AnilistId, row: DubRow) {
    if let Err(e) = store.put_dub(id, ROUTE, &row) {
        tracing::warn!(error = %e, %id, "could not keep what a dub search found");
    }
}

/// What the store holds of the dubs of `results`: the titles with one, with their count lines.
pub fn known(store: &Store, results: &[Media]) -> Vec<DubFound> {
    let ids: Vec<AnilistId> = results.iter().map(|m| m.id).collect();
    let rows = store.dubs_of(&ids).unwrap_or_default();
    results
        .iter()
        .filter_map(|media| {
            let row = rows.get(&media.id).filter(|row| row.has_dub)?;
            let dub = DubKnown { through: row.through, complete: row.complete };
            Some(DubFound {
                id: media.id,
                dub,
                count_line: data::entry_from(media, Some(store)).count_line,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use kaeri_core::{ids::ProviderKey, media::MediaFormat};

    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_201);

    fn media(episodes: Option<u32>) -> Media {
        serde_json::from_value(serde_json::json!({
            "id": SHOW.get(),
            "title": { "romaji": "Tsukiyo no Watashimori", "english": "The Moonlit Ferry" },
            "format": "TV",
            "status": "FINISHED",
            "episodes": episodes,
            "seasonYear": 2026,
        }))
        .unwrap()
    }

    fn hit(title: &str, count: Option<u32>) -> SearchHit {
        SearchHit {
            key: ProviderKey::new("moonlit-ferry"),
            title: title.into(),
            synonyms: vec![],
            episode_count: count,
            year: Some(2026),
            format: Some(MediaFormat::Tv),
        }
    }

    #[test]
    fn a_hit_below_the_total_is_how_far_the_dub_goes_and_a_whole_count_is_only_a_dub() {
        let store = Store::open_in_memory().unwrap();
        keep(&store, &media(Some(12)), &[hit("Tsukiyo no Watashimori", Some(8))], 100);
        let row = store.dub_row(SHOW, ROUTE).unwrap().unwrap();
        assert!(row.has_dub);
        assert_eq!(row.through, Some(8));

        // The show's own count is not the dub's.
        keep(&store, &media(Some(12)), &[hit("Tsukiyo no Watashimori", Some(12))], 100);
        let row = store.dub_row(SHOW, ROUTE).unwrap().unwrap();
        assert!(row.has_dub);
        assert_eq!(row.through, None);
    }

    #[test]
    fn a_hit_that_is_not_the_title_is_no_dub_and_a_page_found_dub_is_not_overwritten() {
        let store = Store::open_in_memory().unwrap();
        keep(&store, &media(Some(12)), &[hit("A Quite Different Show", Some(8))], 100);
        let row = store.dub_row(SHOW, ROUTE).unwrap().unwrap();
        assert!(!row.has_dub);
        assert!(known(&store, &[media(Some(12))]).is_empty());

        // What a title page found sits under its own route, and the card says the best of both.
        store
            .put_dub(
                SHOW,
                "first",
                &DubRow {
                    has_dub: true,
                    through: Some(6),
                    complete: false,
                    aired: Some(6),
                    said_at: 100,
                    until: 200,
                },
            )
            .unwrap();
        let found = known(&store, &[media(Some(12))]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].dub.through, Some(6));
    }
}
