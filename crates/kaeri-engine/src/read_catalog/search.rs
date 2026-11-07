//! Reading's search and browse, the reading lists, and Fix this match for a folder's series.
//!
//! Search asks AniList's books with the filters as AniList arguments. When AniList cannot be
//! reached it asks the person's own sources instead, so a search still finds what they have.

use kaeri_core::{books::WorkKey, media::Medium};
use kaeri_meta::{BrowseFilter, anilist::books::BookAsk};
use kaeri_model::{
    book_sources::BookSourceEvent,
    event::Event,
    local::MatchChoice,
    reading::{self as words, LibrarySegmentRead, ReadSearchQuery, ReadingEvent, WorkTile},
};

use super::{Ask, Kit, send, tiles};
use crate::outbox::Outbox;

/// Books a search page holds.
const PER_PAGE: u32 = 30;
/// The tag AniList gives webtoons.
const LONG_STRIP: &str = "Long Strip";

/// The query as AniList's filter. Webtoons are manga drawn as one long strip, so "Manga" alone
/// leaves the strips out and "Webtoon" alone keeps only them.
pub(crate) fn filter_of(query: &ReadSearchQuery, adult: bool) -> BrowseFilter {
    let mut filter = BrowseFilter {
        sort: query.sort.iter().cloned().collect(),
        genres: query.genres.clone(),
        genres_excluded: query.exclude_genres.clone(),
        countries: query.origins.clone(),
        adult: (!adult).then_some(false),
        ..BrowseFilter::default()
    };
    if let Some(status) = query.status.as_deref().and_then(anilist_status) {
        filter.statuses.push(status.to_owned());
    }
    let formats: Vec<&str> = query.formats.iter().map(String::as_str).collect();
    match formats.as_slice() {
        [] => match query.medium {
            Some(Medium::Novel) => filter.formats.push("NOVEL".into()),
            Some(Medium::Manga) => filter.formats.extend(["MANGA".into(), "ONE_SHOT".into()]),
            _ => {}
        },
        ["webtoon"] => {
            filter.formats.push("MANGA".into());
            filter.tags.push(LONG_STRIP.into());
        }
        ["manga"] => {
            filter.formats.push("MANGA".into());
            filter.tags_excluded.push(LONG_STRIP.into());
        }
        many => {
            for format in many {
                let name = match *format {
                    "manga" | "webtoon" => "MANGA",
                    "one-shot" => "ONE_SHOT",
                    "novel" => "NOVEL",
                    _ => continue,
                };
                if !filter.formats.iter().any(|f| f == name) {
                    filter.formats.push(name.into());
                }
            }
        }
    }
    filter
}

fn anilist_status(key: &str) -> Option<&'static str> {
    Some(match key {
        "releasing" => "RELEASING",
        "finished" => "FINISHED",
        "hiatus" => "HIATUS",
        "cancelled" => "CANCELLED",
        "not-yet" => "NOT_YET_RELEASED",
        _ => return None,
    })
}

/// One page of a search. Pages count from 0.
pub fn search(kit: &Kit, ask: Ask, query: ReadSearchQuery, page: u32, tx: &Outbox) {
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        let filter = filter_of(&query, ask.adult);
        let term = query.text.trim().to_owned();
        let browse = BookAsk::browse(&term, &filter, page + 1, PER_PAGE);
        // The other scope's count rides the first page, with words to count.
        let shows = (ask.both() && page == 0 && !term.is_empty())
            .then_some((term.as_str(), filter.adult));
        match kit.meta.books_and_shows(&[browse], shows).await {
            Ok((pages, other_scope)) => {
                let (items, has_more) = pages
                    .into_iter()
                    .next()
                    .map_or((Vec::new(), false), |p| (p.items, p.has_next));
                let mut tiles: Vec<_> = items
                    .iter()
                    .filter(|b| ask.adult || !b.adult())
                    .map(|b| tiles::reading(&kit.meta, &kit.store, &kit.sources, b))
                    .collect();
                // AniList has no word of the demo: its works follow AniList's on the first page,
                // while it is on.
                if page == 0 {
                    for mut tile in kit.sources.demo_tiles(&query) {
                        tiles::mark(&mut tile, &kit.store, &kit.sources);
                        tiles.push(tile);
                    }
                }
                let total = total(page, tiles.len(), has_more);
                send(
                    &tx,
                    ReadingEvent::Search { query, page, tiles, has_more, other_scope, total },
                );
            }
            Err(e) => {
                tracing::info!(error = %e, "reading's search fell back to the sources");
                let tiles = if page == 0 && !term.is_empty() {
                    kit.sources.adopt_plugins(&kit.plugins);
                    kit.sources.search(&term, query.medium, ask.now).await
                } else {
                    Vec::new()
                };
                send(
                    &tx,
                    ReadingEvent::Search {
                        query,
                        page,
                        tiles,
                        has_more: false,
                        other_scope: None,
                        total: None,
                    },
                );
            }
        }
    });
}

/// How many series AniList found, once the last page is in. Before that AniList's own total is
/// a stand-in (5000 for any search with more than a page), so no number is given rather than a
/// wrong one.
fn total(page: u32, on_page: usize, has_more: bool) -> Option<u32> {
    let on_page = u32::try_from(on_page).ok()?;
    (!has_more).then(|| page.saturating_mul(PER_PAGE).saturating_add(on_page))
}

/// A reading list: at once from what was kept, then again once the books missing from it land.
pub fn library(kit: &Kit, ask: Ask, segment: LibrarySegmentRead, tx: &Outbox) {
    send(tx, ReadingEvent::Library { segment, tiles: list(kit, ask, segment) });
    if segment == LibrarySegmentRead::YourFiles {
        return;
    }
    let works = kit.store.read_status_works(segment.as_str()).unwrap_or_default();
    let missing = tiles::missing(&kit.meta, &works, ask.now);
    if missing.is_empty() {
        return;
    }
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        match kaeri_meta::on_screen(kit.meta.books_many(&missing)).await {
            Ok(_) => {
                send(&tx, ReadingEvent::Library { segment, tiles: list(&kit, ask, segment) })
            }
            Err(e) => tracing::info!(error = %e, "a reading list's books were not asked for"),
        }
    });
}

fn list(kit: &Kit, ask: Ask, segment: LibrarySegmentRead) -> Vec<WorkTile> {
    if segment == LibrarySegmentRead::YourFiles {
        return your_files(kit);
    }
    let works = kit.store.read_status_works(segment.as_str()).unwrap_or_default();
    let kept = tiles::kept_of(&kit.meta, &works);
    let local = kit.store.local_book_series().unwrap_or_default();
    works
        .iter()
        .filter_map(|work| {
            let mut tile = match kept
                .iter()
                .find(|k| tiles::work_of(k.book.id).as_str() == work)
            {
                Some(k) if !ask.adult && k.book.adult() => return None,
                Some(k) => tiles::reading(&kit.meta, &kit.store, &kit.sources, &k.book),
                None => match local.iter().find(|s| s.work.as_deref() == Some(work.as_str())) {
                    Some(series) => local_tile(kit, work, &series.name, &series.medium),
                    None => tiles::demo(work, &kit.store, &kit.sources)?,
                },
            };
            tiles::library_line(&mut tile, &kit.store, &kit.sources);
            Some(tile)
        })
        .collect()
}

/// Series in the person's folders that AniList does not list.
fn your_files(kit: &Kit) -> Vec<WorkTile> {
    kit.store
        .local_book_series()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.work.as_deref().is_none_or(|w| tiles::anilist(w).is_none()))
        .map(|s| {
            let medium = Medium::parse(&s.medium).unwrap_or(Medium::Manga);
            let work =
                s.work.clone().unwrap_or_else(|| WorkKey::local(medium, &s.name).to_string());
            local_tile(kit, &work, &s.name, &s.medium)
        })
        .collect()
}

fn local_tile(kit: &Kit, work: &str, name: &str, medium: &str) -> WorkTile {
    let medium = Medium::parse(medium).unwrap_or(Medium::Manga);
    let mut tile = WorkTile {
        work: kaeri_core::books::WorkId::new(work.to_owned()),
        title: name.to_owned(),
        medium,
        cover: None,
        format_label: None,
        progress: None,
        next: None,
        new_chapter: false,
        adult: false,
        facts: vec![words::kind_words(None, medium, None)],
        demo: false,
    };
    tiles::mark(&mut tile, &kit.store, &kit.sources);
    tile
}

/// Fix this match for a folder's series: AniList's books for its name, or for what the person
/// typed, each with how well it fits.
pub fn find_match(kit: &Kit, ask: Ask, group: String, query: String, tx: &Outbox) {
    let series = kit.store.local_book_series().unwrap_or_default();
    let name = series.iter().find(|s| s.id == group).map(|s| s.name.clone());
    let term = match query.trim() {
        "" => name.unwrap_or_default(),
        typed => typed.to_owned(),
    };
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        let filter =
            BrowseFilter { adult: (!ask.adult).then_some(false), ..BrowseFilter::default() };
        let asked = kit.meta.books(&[BookAsk::browse(&term, &filter, 1, 10)]).await;
        let (choices, offline) = match asked {
            Ok(pages) => {
                let books = pages.into_iter().next().map(|p| p.items).unwrap_or_default();
                let choices = books
                    .iter()
                    .map(|b| {
                        let names = [&b.title.romaji, &b.title.english, &b.title.native]
                            .into_iter()
                            .flatten()
                            .chain(&b.synonyms);
                        let best = names
                            .map(|n| kaeri_meta::similarity(&term, n))
                            .fold(0.0_f64, f64::max);
                        let kind = words::kind_words(
                            Some(b.reading_format().as_str()),
                            b.medium(),
                            b.origin().as_ref().map(kaeri_core::media::Origin::code),
                        );
                        MatchChoice {
                            anilist: b.id,
                            title: kit.meta.titles().pick(&b.title).to_owned(),
                            format: Some(kind.english()),
                            year: b.start_date.as_ref().and_then(|d| d.year),
                            episodes: b.chapters,
                            confidence: (best * 100.0).round().clamp(0.0, 100.0) as u8,
                            fits: true,
                        }
                    })
                    .collect();
                (choices, false)
            }
            Err(e) => {
                tracing::info!(error = %e, "Fix this match could not ask AniList");
                (Vec::new(), true)
            }
        };
        let _ = tx.send(Event::BookSources(BookSourceEvent::Matches {
            group,
            query,
            choices,
            offline,
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::{PER_PAGE, total};

    #[test]
    fn the_total_is_given_only_once_the_last_page_is_in() {
        assert_eq!(total(0, 12, false), Some(12));
        assert_eq!(total(2, 7, false), Some(2 * PER_PAGE + 7));
        assert_eq!(total(0, 30, true), None, "AniList's own total is a stand-in until then");
        assert_eq!(total(0, 0, false), Some(0));
    }
}
