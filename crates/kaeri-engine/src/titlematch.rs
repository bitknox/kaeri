//! Which AniList entry a name is: shared by media servers and Your media.

use kaeri_core::{ids::AnilistId, media::SearchHit};
use kaeri_meta::{
    CONFIDENCE_FLOOR, MatchTarget, Meta,
    anilist::{BrowseFilter, Media},
    rank,
};

/// An entry a name could be, and how well the name fits.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Candidate {
    pub id: AnilistId,
    pub title: String,
    pub score: f64,
}

/// The entries `name` fits at or above the confidence floor, best first. `Err` when AniList could
/// not be asked, so the caller can say so instead of calling it a miss.
pub(crate) async fn candidates(
    meta: &Meta,
    name: &str,
    year: Option<u16>,
) -> Result<Vec<Candidate>, ()> {
    let found = scored(meta, name, year, 5).await?;
    let out = found
        .into_iter()
        .filter(|(_, score)| *score >= CONFIDENCE_FLOOR)
        .map(|(media, score)| Candidate {
            id: media.id,
            title: media.title.display().to_owned(),
            score,
        })
        .collect();
    Ok(out)
}

/// Every entry AniList finds for `name`, with how well the name fits (0 for one the ranking
/// rejects), best first. For Fix this match, which lists them all.
pub(crate) async fn scored(
    meta: &Meta,
    name: &str,
    year: Option<u16>,
    count: u32,
) -> Result<Vec<(Media, f64)>, ()> {
    let found = meta.find(name, &BrowseFilter::default(), 1, count).await.map_err(|_| ())?;
    let mut hit = SearchHit::new("name", name);
    hit.year = year;
    let mut out: Vec<(Media, f64)> = found
        .page
        .items
        .into_iter()
        .map(|media| {
            // The name carries neither an episode count nor a format to compare.
            let target =
                MatchTarget { episode_count: None, format: None, ..media.match_target() };
            let score = rank(&target, std::slice::from_ref(&hit))
                .first()
                .filter(|best| best.rejected.is_none())
                .map_or(0.0, |best| best.score);
            (media, score)
        })
        .collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}
