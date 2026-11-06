//! Trending over a period, and how many titles a browse matches.
//!
//! AniList's `trending` is "related activity in the past hour", which is what Trending now has
//! always been. A week or a month is summed from `mediaTrends`, which has a row per title per
//! day: the week's fifty highest rows give the titles and their figures in one request; the
//! month takes its candidates from four pages of the highest rows, then asks each candidate's
//! whole month, two requests in all. Both are approximations of the sum over every title, close
//! enough for a rail: a title with a quiet month cannot reach the highest rows.

use std::collections::HashMap;

use kaeri_core::ids::AnilistId;
use serde_json::Value;

use super::{AniList, BrowseFilter, Media, Result, query};

/// The most titles a period's rail holds.
pub const TREND_TITLES: usize = 50;

/// The most titles AniList's page count is trusted to hold: its `total` may be capped or
/// approximate, so a pick takes pages within this many only.
pub const TOTAL_CEILING: u32 = 2_500;

impl AniList {
    /// The last seven days' trending, most first: one request. `from` is the Unix second the
    /// window opens at.
    pub async fn trending_week(&self, from: i64) -> Result<Vec<Media>> {
        let data: Value =
            self.execute(&query::trend_week(), serde_json::json!({ "from": from })).await?;
        let rows = data["Page"]["mediaTrends"].as_array().cloned().unwrap_or_default();
        let mut scores: Vec<(AnilistId, f64)> = Vec::new();
        let mut cards: HashMap<AnilistId, Media> = HashMap::new();
        for row in &rows {
            let Some(id) = row["mediaId"].as_u64().map(|id| AnilistId::new(id as u32)) else {
                continue;
            };
            let trending = row["trending"].as_f64().unwrap_or(0.0);
            match scores.iter_mut().find(|(seen, _)| *seen == id) {
                Some((_, total)) => *total += trending,
                None => scores.push((id, trending)),
            }
            if !cards.contains_key(&id)
                && let Ok(media) = serde_json::from_value::<Media>(row["media"].clone())
            {
                cards.insert(id, media);
            }
        }
        Ok(ranked(scores, cards))
    }

    /// The last thirty days' trending, most first: two requests. The first names the
    /// candidates, the second brings their cards and every day's figure.
    pub async fn trending_month(&self, from: i64) -> Result<Vec<Media>> {
        let data: Value = self
            .execute(&query::trend_month_candidates(), serde_json::json!({ "from": from }))
            .await?;
        let mut candidates: Vec<AnilistId> = Vec::new();
        for page in 1..=4 {
            for row in data[format!("p{page}")]["mediaTrends"].as_array().into_iter().flatten()
            {
                if let Some(id) = row["mediaId"].as_u64().map(|id| AnilistId::new(id as u32))
                    && !candidates.contains(&id)
                {
                    candidates.push(id);
                }
            }
        }
        candidates.truncate(50);
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<u32> = candidates.iter().map(|id| id.get()).collect();
        let data: Value = self
            .execute(&query::trend_month_series(), serde_json::json!({ "ids": ids }))
            .await?;
        let mut scores: Vec<(AnilistId, f64)> = Vec::new();
        let mut cards: HashMap<AnilistId, Media> = HashMap::new();
        for item in data["Page"]["media"].as_array().into_iter().flatten() {
            let Ok(media) = serde_json::from_value::<Media>(item.clone()) else { continue };
            let total: f64 = item["trends"]["nodes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|node| node["date"].as_i64().is_some_and(|date| date > from))
                .filter_map(|node| node["trending"].as_f64())
                .sum();
            scores.push((media.id, total));
            cards.insert(media.id, media);
        }
        Ok(ranked(scores, cards))
    }

    /// How many titles `filter` matches, as AniList counts them: one request of one title.
    /// `None` when AniList did not say. Held to [`TOTAL_CEILING`], as the count may be
    /// capped or approximate.
    pub async fn browse_total(&self, filter: &BrowseFilter) -> Result<Option<u32>> {
        let arguments = filter.arguments("", 1, 1);
        let (document, variables) = query::media_total(&arguments);
        let data: Value = self.execute(&document, variables).await?;
        Ok(data["r0"]["pageInfo"]["total"]
            .as_u64()
            .map(|total| u32::try_from(total).unwrap_or(u32::MAX).min(TOTAL_CEILING)))
    }
}

/// `scores` highest first, then by id so a tie does not shuffle, as the cards in `cards`,
/// [`TREND_TITLES`] at most.
fn ranked(
    mut scores: Vec<(AnilistId, f64)>,
    mut cards: HashMap<AnilistId, Media>,
) -> Vec<Media> {
    scores.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.get().cmp(&b.0.get())));
    scores.into_iter().filter_map(|(id, _)| cards.remove(&id)).take(TREND_TITLES).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_documents_name_their_variables_and_the_total_leaves_the_page_alone() {
        assert!(query::trend_week().contains("mediaTrends(date_greater: $from"));
        let month = query::trend_month_candidates();
        assert!(month.contains("p4: Page(page: 4"));
        assert!(query::trend_month_series().contains("trends(sort: DATE_DESC, perPage: 31)"));
        let filter = BrowseFilter { genres: vec!["Mecha".into()], ..BrowseFilter::default() };
        let (document, variables) = query::media_total(&filter.arguments("", 3, 50));
        assert!(document.contains("Page(page: 1, perPage: 1) { pageInfo { total }"));
        assert!(document.contains("genre_in: $r0_genre_in"), "{document}");
        assert!(variables.get("r0_page").is_none() && variables.get("r0_perPage").is_none());
    }

    #[test]
    fn ranking_sums_then_breaks_ties_by_id() {
        let card = |id: u32| {
            serde_json::from_value::<Media>(serde_json::json!({ "id": id, "title": {} }))
                .unwrap()
        };
        let scores = vec![
            (AnilistId::new(3), 10.0),
            (AnilistId::new(1), 30.0),
            (AnilistId::new(2), 30.0),
        ];
        let cards = [1, 2, 3].into_iter().map(|id| (AnilistId::new(id), card(id))).collect();
        let order: Vec<u32> = ranked(scores, cards).iter().map(|m| m.id.get()).collect();
        assert_eq!(order, [1, 2, 3]);
    }
}
