//! The mapping lookups: their answers decode strictly from fixtures (made-up titles, `.example`
//! hosts, shaped like what the services sent when the shapes were checked), and the asks go to
//! stand-ins. Nothing here reaches a real service.

use kaeri_testkit::lookups::FakeLookups;
use kaeri_track::lookup::{self, Candidate, Lookups, SourceIds};

const BAKA: &str = include_str!("fixtures/mangabaka_source.json");
const BAKA_MERGED: &str = include_str!("fixtures/mangabaka_merged.json");
const KITSU: &str = include_str!("fixtures/kitsu_mappings.json");
const KITSU_EMPTY: &str = include_str!("fixtures/kitsu_mappings_empty.json");
const SEARCH: &str = include_str!("fixtures/mangaupdates_search.json");

fn lookups(fake: &FakeLookups) -> Lookups {
    Lookups::new(reqwest::Client::new()).at(
        &fake.base(),
        &fake.kitsu_api(),
        &fake.mangaupdates_api(),
    )
}

#[test]
fn mangabaka_ids_decode_and_the_updates_id_is_base_36() {
    let ids = lookup::decode_mangabaka(BAKA).unwrap().unwrap();
    assert_eq!(
        ids,
        SourceIds {
            my_anime_list: Some(5151),
            kitsu: Some(4242),
            manga_updates: Some(u64::from_str_radix("abc123x", 36).unwrap()),
        }
    );
}

#[test]
fn a_merged_mangabaka_series_is_not_trusted() {
    assert_eq!(lookup::decode_mangabaka(BAKA_MERGED).unwrap(), None);
}

#[test]
fn a_mangabaka_answer_without_its_series_list_is_an_error() {
    assert!(lookup::decode_mangabaka(r#"{"data":{}}"#).is_err());
    assert!(lookup::decode_mangabaka("[]").is_err());
}

#[test]
fn a_kitsu_mapping_names_the_manga() {
    assert_eq!(lookup::decode_kitsu_mapping(KITSU).unwrap(), Some(4242));
    assert_eq!(lookup::decode_kitsu_mapping(KITSU_EMPTY).unwrap(), None);
    assert!(lookup::decode_kitsu_mapping(r#"{"nope":1}"#).is_err());
}

#[test]
fn search_hits_become_candidates_without_doujinshi() {
    let found = lookup::decode_search(SEARCH).unwrap();
    assert_eq!(
        found,
        [
            Candidate {
                series_id: 100_000_000_001,
                title: "Lantern Road".into(),
                year: Some(2011),
                novel: false
            },
            Candidate {
                series_id: 100_000_000_002,
                title: "Lantern Road Gaiden".into(),
                year: None,
                novel: true
            },
        ]
    );
    assert!(lookup::decode_search(r#"{"results":[{"record":{"title":"x"}}]}"#).is_err());
}

#[tokio::test]
async fn the_asks_reach_each_service_and_a_404_is_no_answer() {
    let fake = FakeLookups::start().await;
    fake.set_mangabaka(Some(BAKA));
    fake.set_kitsu_mappings(KITSU);
    fake.set_search(SEARCH);
    let l = lookups(&fake);
    assert_eq!(l.mangabaka(7).await.unwrap().unwrap().kitsu, Some(4242));
    assert_eq!(l.kitsu_manga("anilist/manga", 7).await.unwrap(), Some(4242));
    assert_eq!(l.search_mangaupdates("Lantern Road").await.unwrap().len(), 2);
    fake.set_mangabaka(None);
    assert_eq!(l.mangabaka(8).await.unwrap(), None);
    let seen = fake.requests();
    assert!(seen[0].starts_with("GET /v1/source/anilist/7"));
    assert!(
        seen[1].contains("filter%5BexternalSite%5D=anilist%2Fmanga")
            || seen[1].contains("anilist/manga")
    );
    assert!(seen[2].starts_with("POST /v1/series/search"));
}
