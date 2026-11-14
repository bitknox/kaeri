//! Manga, manhwa, manhua and light novels: AniList's `MANGA` type.
//!
//! AniList keeps every book under one type and tells them apart by two fields: `format`
//! (`MANGA`, `NOVEL`, `ONE_SHOT`) and `countryOfOrigin` (`JP`, `KR`, `CN`, `TW`). A webtoon is a
//! manga with the `Long Strip` tag. Ids are shared with anime, so one id never names both.
//!
//! The requests follow the anime side's rules: one request a screen, several pages and id lists
//! under aliases in it (see [`query::together`]), each under the same rate limit and lanes.

use kaeri_core::{
    ids::AnilistId,
    media::{MediaStatus, Medium, Origin, ReadingFormat},
};
use serde::{Deserialize, Serialize};

use super::{
    AniList, AniListError, BrowseFilter, MANY_PER_REQUEST, Page, Result,
    model::{CoverImage, FuzzyDate, StaffConnection, Title},
    query,
};

/// The tag AniList gives a series drawn as one long vertical strip.
pub const LONG_STRIP: &str = "Long Strip";

/// What every list shows of a book: a cover anywhere, and the facts under it.
pub const BOOK_FIELDS: &str = r"
    id
    idMal
    title { romaji english native }
    format
    status
    description
    chapters
    volumes
    countryOfOrigin
    isAdult
    genres
    synonyms
    coverImage { extraLarge large color }
    bannerImage
    averageScore
    popularity
    startDate { year month day }
    tags { name rank }
";

/// A related or recommended title, as a card shows it.
const BOOK_LINK: &str = r"
    id type format title { romaji english native } coverImage { large color }
";

/// What only the series page needs: who made it, what it relates to (an anime among them), and
/// what people who liked it rated highest.
pub const BOOK_DETAIL_FIELDS: &str = r"
    staff(sort: RELEVANCE, perPage: 6) { edges { role node { name { full } } } }
    relations { edges { relationType node { BOOK_LINK } } }
    recommendations(sort: RATING_DESC, perPage: 10) {
        nodes { mediaRecommendation { BOOK_LINK } }
    }
";

/// How many recommendations a series page asks for.
pub const RECOMMENDED_PER_BOOK: usize = 10;

/// One book.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub id: AnilistId,
    #[serde(default)]
    pub id_mal: Option<u32>,
    #[serde(default)]
    pub title: Title,
    /// AniList's: `MANGA`, `NOVEL`, `ONE_SHOT`.
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub status: Option<MediaStatus>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub chapters: Option<u32>,
    #[serde(default)]
    pub volumes: Option<u32>,
    #[serde(default)]
    pub country_of_origin: Option<String>,
    #[serde(default)]
    pub is_adult: Option<bool>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(default)]
    pub cover_image: CoverImage,
    #[serde(default)]
    pub banner_image: Option<String>,
    #[serde(default)]
    pub average_score: Option<u32>,
    #[serde(default)]
    pub popularity: Option<u32>,
    #[serde(default)]
    pub start_date: Option<FuzzyDate>,
    #[serde(default, deserialize_with = "null_list")]
    pub tags: Vec<BookTag>,
    /// The series page's: empty on lists.
    #[serde(default)]
    pub staff: StaffConnection,
    #[serde(default)]
    pub relations: BookRelations,
    #[serde(default)]
    pub recommendations: BookRecommendations,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BookTag {
    pub name: String,
    #[serde(default)]
    pub rank: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct BookRelations {
    #[serde(default)]
    pub edges: Vec<BookRelationEdge>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookRelationEdge {
    /// AniList's: `ADAPTATION`, `SOURCE`, `SEQUEL`, `SPIN_OFF`.
    #[serde(default)]
    pub relation_type: Option<String>,
    pub node: BookLink,
}

/// A related or recommended title: a book, or a show.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookLink {
    pub id: AnilistId,
    /// `ANIME` or `MANGA`.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub title: Title,
    #[serde(default)]
    pub cover_image: CoverImage,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct BookRecommendations {
    #[serde(default)]
    pub nodes: Vec<BookRecommendation>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookRecommendation {
    #[serde(default)]
    pub media_recommendation: Option<BookLink>,
}

/// AniList sends `null` for a list it has nothing in on some titles.
fn null_list<'de, D, T>(deserializer: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

impl Book {
    /// A light novel is a novel; everything else AniList lists as a book is drawn.
    pub fn medium(&self) -> Medium {
        match self.format.as_deref() {
            Some("NOVEL") => Medium::Novel,
            _ => Medium::Manga,
        }
    }

    /// Drawn as one long strip.
    pub fn is_strip(&self) -> bool {
        self.tags.iter().any(|t| t.name == LONG_STRIP)
    }

    pub fn reading_format(&self) -> ReadingFormat {
        match self.format.as_deref() {
            Some("NOVEL") => ReadingFormat::Novel,
            Some("ONE_SHOT") => ReadingFormat::OneShot,
            _ if self.is_strip() => ReadingFormat::Webtoon,
            _ => ReadingFormat::Manga,
        }
    }

    pub fn origin(&self) -> Option<Origin> {
        self.country_of_origin.as_deref().and_then(Origin::parse)
    }

    pub fn adult(&self) -> bool {
        self.is_adult == Some(true)
    }

    /// The people who wrote and drew it, each once, in AniList's order of relevance.
    pub fn authors(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for edge in &self.staff.edges {
            let role = edge.role.as_deref().unwrap_or_default();
            let wrote_or_drew =
                ["Story", "Art", "Original"].iter().any(|r| role.starts_with(r));
            let Some(name) = edge.node.as_ref().and_then(|p| p.name.full.clone()) else {
                continue;
            };
            if wrote_or_drew && !out.contains(&name) {
                out.push(name);
            }
        }
        out
    }

    /// The shows this book is told as, or was told from: the first is the closest.
    pub fn shows(&self) -> Vec<(&BookLink, &str)> {
        let mut shows: Vec<(&BookLink, &str)> = self
            .relations
            .edges
            .iter()
            .filter(|e| e.node.kind.as_deref() == Some("ANIME"))
            .map(|e| (&e.node, e.relation_type.as_deref().unwrap_or("OTHER")))
            .collect();
        // An adaptation before a source, and a series before a film.
        shows.sort_by_key(|(node, relation)| {
            (*relation != "ADAPTATION", node.format.as_deref() != Some("TV"))
        });
        shows
    }

    /// What it is about, as plain text, with what AniList marks a spoiler left out.
    pub fn synopsis(&self) -> Option<String> {
        let raw = self.description.as_deref()?;
        let text = super::model::spoiler_runs(raw)
            .into_iter()
            .filter(|(_, spoiler)| !spoiler)
            .map(|(run, _)| super::model::plain_text(run))
            .collect::<String>();
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    /// The books people who liked this one rated highest, best first.
    pub fn recommended(&self) -> Vec<AnilistId> {
        self.recommendations
            .nodes
            .iter()
            .filter_map(|n| n.media_recommendation.as_ref())
            .filter(|n| n.kind.as_deref() != Some("ANIME"))
            .map(|n| n.id)
            .collect()
    }
}

/// One part of a request for books: a browse page, or books by id (at most fifty).
#[derive(Debug, Clone, PartialEq)]
pub enum BookAsk {
    Browse { term: String, filter: Box<BrowseFilter>, page: u32, per_page: u32 },
    Ids(Vec<AnilistId>),
}

impl BookAsk {
    pub fn browse(term: &str, filter: &BrowseFilter, page: u32, per_page: u32) -> Self {
        Self::Browse { term: term.to_owned(), filter: Box::new(filter.clone()), page, per_page }
    }

    /// The query of one `Page` root for this part, with its variables named for the arguments.
    fn part(&self) -> (String, serde_json::Value) {
        let arguments = match self {
            Self::Browse { term, filter, page, per_page } => {
                filter.arguments(term, *page, *per_page)
            }
            Self::Ids(ids) => {
                let raw: Vec<u32> = ids.iter().map(|id| id.get()).collect();
                vec![
                    (("perPage", "Int"), serde_json::json!(ids.len().clamp(1, 50))),
                    (("id_in", "[Int]"), serde_json::json!(raw)),
                ]
            }
        };
        book_page(&arguments)
    }
}

/// One page of books with `arguments`, as one query of one `Page` root: the shape
/// [`query::together`] puts several of under aliases.
pub fn book_page(
    arguments: &[(query::Argument, serde_json::Value)],
) -> (String, serde_json::Value) {
    let mut declared = Vec::new();
    let mut page_args = Vec::new();
    let mut media_args = vec!["type: MANGA".to_owned()];
    let mut variables = serde_json::Map::new();
    for ((name, kind), value) in arguments {
        declared.push(format!("${name}: {kind}"));
        variables.insert((*name).to_owned(), value.clone());
        match *name {
            "page" | "perPage" => page_args.push(format!("{name}: ${name}")),
            _ => media_args.push(format!("{name}: ${name}")),
        }
    }
    let document = format!(
        "query ({}) {{ Page({}) {{ pageInfo {{ hasNextPage }} media({}) {{ {BOOK_FIELDS} }} }} }}",
        declared.join(", "),
        page_args.join(", "),
        media_args.join(", "),
    );
    (document, serde_json::Value::Object(variables))
}

/// How many shows match `term`, as one `Page` root.
pub fn show_count(term: &str, adult: Option<bool>) -> (String, serde_json::Value) {
    let mut declared = vec!["$search: String"];
    let mut media_args = vec!["type: ANIME", "search: $search"];
    let mut variables = serde_json::json!({ "search": term });
    if let Some(adult) = adult {
        declared.push("$isAdult: Boolean");
        media_args.push("isAdult: $isAdult");
        variables["isAdult"] = serde_json::json!(adult);
    }
    let document = format!(
        "query ({}) {{ Page(perPage: 1) {{ pageInfo {{ total }} media({}) {{ id }} }} }}",
        declared.join(", "),
        media_args.join(", "),
    );
    (document, variables)
}

/// One book with what its page shows.
pub fn book_by_id() -> String {
    let detail = BOOK_DETAIL_FIELDS.replace("BOOK_LINK", BOOK_LINK);
    format!("query ($id: Int) {{ Media(id: $id, type: MANGA) {{ {BOOK_FIELDS} {detail} }} }}")
}

fn decode_book_page(page: &serde_json::Value) -> Page<Book> {
    let items = page["media"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|m| match serde_json::from_value::<Book>(m.clone()) {
                    Ok(book) => Some(book),
                    Err(e) => {
                        tracing::warn!(error = %e, "skipping undecodable book");
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Page { items, has_next: page["pageInfo"]["hasNextPage"].as_bool().unwrap_or(false) }
}

impl AniList {
    /// Several parts in one request, each answered as a page in the order asked. Books asked
    /// by id come in the order asked too, and without the ones AniList does not have.
    pub async fn books(&self, asks: &[BookAsk]) -> Result<Vec<Page<Book>>> {
        Ok(self.books_and_shows(asks, None).await?.0)
    }

    /// [`Self::books`], with how many shows match a term in the same request: Search's count
    /// in the other scope. `shows` is the term and the adult filter.
    pub async fn books_and_shows(
        &self,
        asks: &[BookAsk],
        shows: Option<(&str, Option<bool>)>,
    ) -> Result<(Vec<Page<Book>>, Option<u32>)> {
        let mut parts: Vec<_> = asks.iter().map(BookAsk::part).collect();
        if let Some((term, adult)) = shows {
            parts.push(show_count(term, adult));
        }
        let mut answers = self.execute_together(&parts).await?;
        let count = match shows {
            Some(_) => answers.pop().and_then(|a| a["Page"]["pageInfo"]["total"].as_u64()),
            None => None,
        };
        let pages = asks
            .iter()
            .zip(answers)
            .map(|(ask, answer)| {
                let mut page = decode_book_page(&answer["Page"]);
                if let BookAsk::Ids(ids) = ask {
                    page.items.sort_by_key(|b| ids.iter().position(|id| *id == b.id));
                }
                page
            })
            .collect();
        Ok((pages, count.and_then(|c| u32::try_from(c).ok())))
    }

    /// Pages of several browses in one request.
    pub async fn book_pages(
        &self,
        browses: &[(&str, &BrowseFilter, u32, u32)],
    ) -> Result<Vec<Page<Book>>> {
        let asks: Vec<_> = browses
            .iter()
            .map(|(term, filter, page, per_page)| {
                BookAsk::browse(term, filter, *page, *per_page)
            })
            .collect();
        self.books(&asks).await
    }

    /// Books by id, in the order asked: pages of fifty, [`MANY_PER_REQUEST`] to a request.
    pub async fn books_many(&self, ids: &[AnilistId]) -> Result<Vec<Book>> {
        let mut found = Vec::with_capacity(ids.len());
        for request in ids.chunks(MANY_PER_REQUEST) {
            let asks: Vec<_> = request.chunks(50).map(|c| BookAsk::Ids(c.to_vec())).collect();
            for page in self.books(&asks).await? {
                found.extend(page.items);
            }
        }
        Ok(found)
    }

    /// Several books with their makers, relations and recommendations, in one request with each
    /// under an alias, in the order asked. A book AniList does not have is left out.
    pub async fn book_details(&self, ids: &[AnilistId]) -> Result<Vec<Book>> {
        if let [id] = ids {
            return match self.book(*id).await {
                Ok(book) => Ok(vec![book]),
                Err(AniListError::NotFound) => Ok(Vec::new()),
                Err(e) => Err(e),
            };
        }
        let parts: Vec<_> = ids
            .iter()
            .map(|id| (book_by_id(), serde_json::json!({ "id": id.get() })))
            .collect();
        let answers = self.execute_together(&parts).await?;
        Ok(answers
            .into_iter()
            .filter_map(|answer| {
                let node = answer.get("Media").filter(|m| !m.is_null())?;
                serde_json::from_value(node.clone())
                    .map_err(|e| tracing::warn!(error = %e, "skipping undecodable book"))
                    .ok()
            })
            .collect())
    }

    /// One book with its makers, relations and recommendations.
    pub async fn book(&self, id: AnilistId) -> Result<Book> {
        let data: serde_json::Value =
            self.execute(&book_by_id(), serde_json::json!({ "id": id.get() })).await?;
        let node = data.get("Media").filter(|m| !m.is_null()).ok_or(AniListError::NotFound)?;
        serde_json::from_value(node.clone()).map_err(|e| AniListError::Decode(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(json: &str) -> Book {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn format_country_and_the_strip_tag_tell_books_apart() {
        let manhwa = fixture(
            r#"{"id": 900303, "format": "MANGA", "countryOfOrigin": "KR",
                "tags": [{"name": "Long Strip", "rank": 95}],
                "title": {"english": "Paper Kites Over Juniper Row"}}"#,
        );
        assert_eq!(manhwa.medium(), Medium::Manga);
        assert_eq!(manhwa.reading_format(), ReadingFormat::Webtoon);
        assert_eq!(manhwa.origin(), Some(Origin::Korea));

        let novel = fixture(r#"{"id": 900302, "format": "NOVEL", "countryOfOrigin": "JP"}"#);
        assert_eq!(
            (novel.medium(), novel.reading_format()),
            (Medium::Novel, ReadingFormat::Novel)
        );
        let manhua = fixture(r#"{"id": 900305, "format": "MANGA", "countryOfOrigin": "CN"}"#);
        assert_eq!(manhua.reading_format(), ReadingFormat::Manga);
        assert_eq!(manhua.origin(), Some(Origin::China));
        let one = fixture(r#"{"id": 900309, "format": "ONE_SHOT", "tags": null}"#);
        assert_eq!(one.reading_format(), ReadingFormat::OneShot);
    }

    #[test]
    fn a_page_names_its_makers_shows_and_recommendations() {
        let book = fixture(
            r#"{"id": 900301, "format": "MANGA",
                "staff": {"edges": [
                    {"role": "Story & Art", "node": {"name": {"full": "Tsukasa Mori"}}},
                    {"role": "Assistant", "node": {"name": {"full": "Someone Else"}}}]},
                "relations": {"edges": [
                    {"relationType": "SPIN_OFF", "node": {"id": 900309, "type": "MANGA"}},
                    {"relationType": "ADAPTATION", "node": {"id": 900002, "type": "ANIME",
                     "format": "TV", "title": {"english": "The Library Above the Clouds"}}}]},
                "recommendations": {"nodes": [
                    {"mediaRecommendation": {"id": 900304, "type": "MANGA"}},
                    {"mediaRecommendation": null},
                    {"mediaRecommendation": {"id": 900005, "type": "ANIME"}}]}}"#,
        );
        assert_eq!(book.authors(), vec!["Tsukasa Mori".to_owned()]);
        let shows = book.shows();
        assert_eq!(shows.len(), 1);
        assert_eq!((shows[0].0.id, shows[0].1), (AnilistId::new(900_002), "ADAPTATION"));
        assert_eq!(book.recommended(), vec![AnilistId::new(900_304)]);
    }

    #[test]
    fn the_documents_parse_and_ask_for_books_only() {
        let ids = BookAsk::Ids(vec![AnilistId::new(1), AnilistId::new(2)]);
        let browse = BookAsk::browse("ferry", &BrowseFilter::every_argument(), 2, 20);
        let (together, variables) = query::together(&[ids.part(), browse.part()]);
        for doc in [book_by_id(), ids.part().0, browse.part().0, together.clone()] {
            if let Err(e) = graphql_parser::parse_query::<String>(&doc) {
                panic!("does not parse: {e}\n{doc}");
            }
            assert!(doc.contains("type: MANGA") && !doc.contains("ANIME"), "{doc}");
        }
        assert_eq!(variables["q0_id_in"], serde_json::json!([1, 2]));
        assert_eq!(variables["q1_search"], serde_json::json!("ferry"));
        assert!(together.contains("q1: Page("), "{together}");
        let detail = book_by_id();
        assert!(detail.contains("relations {") && detail.contains("recommendations("));
        assert!(!BOOK_FIELDS.contains("staff"), "makers ride the page only");
    }
}
