//! Reading's search and browse, the reading lists, and Fix this match for a folder's series.
//!
//! Search asks AniList's books with the filters as AniList arguments. With words typed it asks
//! the person's own sources at the same time, each on its own: every source's group has its own
//! state and is sent as it lands, so a slow source never holds up the rest. A source hit that is
//! the same series as an AniList result joins that result. Browsing with no words asks AniList
//! alone, since a source needs words to search.

use std::{collections::HashSet, time::Duration};

use futures::{StreamExt, stream::FuturesUnordered};
use kaeri_core::{
    books::{BookHints, BookSourceKind, SeriesHit, WorkKey},
    error::ProviderError,
    media::{Medium, ReadingFormat},
};
use kaeri_meta::{
    BrowseFilter,
    anilist::books::{Book, BookAsk},
};
use kaeri_model::{
    book_sources::BookSourceEvent,
    event::Event,
    local::MatchChoice,
    reading::{
        self as words, LibrarySegmentRead, READ_SOURCE_WAIT_SECS, ReadAnilistState,
        ReadSearchQuery, ReadSourceGroup, ReadSourceJoin, ReadSourceKind, ReadSourceSearch,
        ReadSourceState, ReadingAbout, ReadingEvent, ReadingFailure, WorkTile,
    },
};

use super::{Ask, Kit, send, tiles};
use crate::{outbox::Outbox, read_sources::failure};

/// Books a search page holds.
const PER_PAGE: u32 = 30;
/// The tag AniList gives webtoons.
const LONG_STRIP: &str = "Long Strip";
/// How long a source has to answer a search.
const SOURCE_WAIT: Duration = Duration::from_secs(READ_SOURCE_WAIT_SECS as u64);

/// The query as AniList's filter, at `now`. Webtoons are manga drawn as one long strip, so
/// "Manga" without "Webtoon" leaves the strips out and "Webtoon" alone keeps only them.
pub(crate) fn filter_of(query: &ReadSearchQuery, adult: bool, now: i64) -> BrowseFilter {
    let mut filter = BrowseFilter {
        sort: query.sort.iter().cloned().collect(),
        genres: query.genres.clone(),
        genres_excluded: query.exclude_genres.clone(),
        countries: query.origins.clone(),
        adult: (!adult).then_some(false),
        started_after: query.started.as_deref().and_then(|key| started_after(key, now)),
        ..BrowseFilter::default()
    };
    if let Some(status) = query.status.as_deref().and_then(anilist_status) {
        filter.statuses.push(status.to_owned());
    }
    // Manga takes in one-shots, and a `one-shot` saved before that reads as manga.
    let formats = query.format_keys();
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
        many => {
            for format in many {
                let names: &[&str] = match *format {
                    "manga" => &["MANGA", "ONE_SHOT"],
                    "webtoon" => &["MANGA"],
                    "novel" => &["NOVEL"],
                    _ => continue,
                };
                for name in names {
                    if !filter.formats.iter().any(|f| f == name) {
                        filter.formats.push((*name).into());
                    }
                }
            }
            // Manga without webtoons: the strips stay out, one-shots or not.
            if many.contains(&"manga") && !many.contains(&"webtoon") {
                filter.tags_excluded.push(LONG_STRIP.into());
            }
        }
    }
    filter
}

/// The Started filter as AniList's exclusive start date, `YYYYMMDD`: the day 12 months or 3
/// years before `now`, in UTC. `None` for any time.
pub(crate) fn started_after(key: &str, now: i64) -> Option<u32> {
    let years = match key {
        "12-months" => 1,
        "3-years" => 3,
        _ => return None,
    };
    let today =
        jiff::Timestamp::from_second(now).ok()?.to_zoned(jiff::tz::TimeZone::UTC).date();
    let then = today.checked_sub(jiff::Span::new().years(years)).ok()?;
    let (y, m, d) = (u32::try_from(then.year()).ok()?, then.month(), then.day());
    Some(y * 10_000 + u32::try_from(m).ok()? * 100 + u32::try_from(d).ok()?)
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

/// The medium a query keeps to, from its medium or else its formats.
fn medium_of(query: &ReadSearchQuery) -> Option<Medium> {
    if query.medium.is_some() || query.formats.is_empty() {
        return query.medium;
    }
    let novels = query.formats.iter().filter(|f| *f == "novel").count();
    match novels {
        0 => Some(Medium::Manga),
        n if n == query.formats.len() => Some(Medium::Novel),
        _ => None,
    }
}

/// One page of a search. Pages count from 0. With words, the first page also asks the person's
/// sources; "Your sources only" asks them alone and never AniList.
pub fn search(kit: &Kit, ask: Ask, query: ReadSearchQuery, page: u32, tx: &Outbox) {
    let searched = searched(kit, &query);
    let words = !query.text.trim().is_empty();
    let with_sources = words && page == 0;
    let generation = with_sources.then(|| sources_search(kit, ask, &query, tx));
    if query.sources_only {
        // Without words, or past the first page, the sources have nothing to answer either:
        // an empty page says so.
        if !with_sources {
            let tiles = Vec::new();
            let (has_more, other_scope, total) = (false, None, None);
            send(tx, ReadingEvent::Search { query, page, tiles, has_more, other_scope, total });
        }
        return;
    }
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        let filter = filter_of(&query, ask.adult, ask.now);
        let term = query.text.trim().to_owned();
        let browse = BookAsk::browse(&term, &filter, page + 1, PER_PAGE);
        // The other scope's count rides the first page, with words to count.
        let shows = (ask.both() && page == 0 && !term.is_empty())
            .then_some((term.as_str(), filter.adult));
        let answer = kit.meta.books_and_shows(&[browse], shows).await;
        // Newer words or filters were searched while AniList answered: the page shows theirs,
        // and this answer, or failure, is for words no longer there.
        if !still_searched(&kit, searched) {
            return;
        }
        match answer {
            Ok((pages, other_scope)) => {
                let (items, has_more) = pages
                    .into_iter()
                    .next()
                    .map_or((Vec::new(), false), |p| (p.items, p.has_next));
                let shown: Vec<Book> =
                    items.into_iter().filter(|b| ask.adult || !b.adult()).collect();
                let mut tiles: Vec<_> = shown
                    .iter()
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
                // The joins go first, so a hit that joins a result (the demo's always do) has
                // left its group before the result shows: it is never on screen twice.
                if let Some(generation) = generation {
                    let candidates = candidates(&shown, &tiles);
                    update(&kit, generation, &tx, |run| {
                        run.anilist = Some(ReadAnilistState::Answered);
                        run.candidates = candidates;
                    });
                }
                send(
                    &tx,
                    ReadingEvent::Search {
                        query: query.clone(),
                        page,
                        tiles,
                        has_more,
                        other_scope,
                        total,
                    },
                );
            }
            Err(e) => {
                tracing::info!(error = %e, "reading's search could not ask AniList");
                let failure = if kit.offline.load(std::sync::atomic::Ordering::Relaxed) {
                    ReadingFailure::Offline
                } else {
                    ReadingFailure::SourceDown { source: "AniList".into() }
                };
                send(&tx, ReadingEvent::Failed { about: ReadingAbout::Search, failure });
                if let Some(generation) = generation {
                    update(&kit, generation, &tx, |run| {
                        run.anilist = Some(ReadAnilistState::Failed);
                    });
                }
            }
        }
    });
}

/// The words and filters last searched, with a generation that changes when they do. More pages
/// of the same search keep it.
#[derive(Debug, Default)]
pub(crate) struct Searched {
    generation: u64,
    query: ReadSearchQuery,
}

/// The generation of `query`'s search, starting a new one when it differs from the last.
fn searched(kit: &Kit, query: &ReadSearchQuery) -> u64 {
    let mut searched = kit.sources.catalog.searched.lock().unwrap_or_else(|e| e.into_inner());
    if searched.generation == 0 || searched.query != *query {
        searched.generation += 1;
        searched.query = query.clone();
    }
    searched.generation
}

fn still_searched(kit: &Kit, generation: u64) -> bool {
    kit.sources.catalog.searched.lock().unwrap_or_else(|e| e.into_inner()).generation
        == generation
}

/// How many series AniList found, once the last page is in. Before that AniList's own total is
/// a stand-in (5000 for any search with more than a page), so no number is given rather than a
/// wrong one.
fn total(page: u32, on_page: usize, has_more: bool) -> Option<u32> {
    let on_page = u32::try_from(on_page).ok()?;
    (!has_more).then(|| page.saturating_mul(PER_PAGE).saturating_add(on_page))
}

// ---------------------------------------------------------------------------------------------
// The sources' part

/// The search with words the sources are being asked for, kept so each answer and a group's
/// Try again land in it. A newer search replaces it, and answers for the old one are dropped.
#[derive(Debug, Default)]
pub(crate) struct Run {
    generation: u64,
    query: ReadSearchQuery,
    anilist: Option<ReadAnilistState>,
    /// AniList's results, for hits to join.
    candidates: Vec<Candidate>,
    groups: Vec<Group>,
    /// The hits a join was kept for, so it is written once.
    kept_joins: HashSet<(String, String)>,
    now: i64,
}

#[derive(Debug, Clone)]
struct Group {
    id: String,
    name: String,
    kind: Option<ReadSourceKind>,
    /// The source's [`crate::read_sources::BookSources::source_mark`] when it was asked.
    mark: Option<u64>,
    state: ReadSourceState,
    hits: Vec<Hit>,
}

/// A source's hit with what joining needs.
#[derive(Debug, Clone)]
struct Hit {
    hit: SeriesHit,
    tile: WorkTile,
    /// The work a match already kept ties it to, before this search made one.
    stored: Option<String>,
    medium: Medium,
}

/// An AniList result a hit may join.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Candidate {
    pub work: String,
    /// Its titles, folded: romaji, English, native and synonyms.
    pub titles: Vec<String>,
    /// `None` for the demo's works, which join only by their kept match.
    pub format: Option<ReadingFormat>,
}

fn candidates(books: &[Book], results: &[WorkTile]) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = books
        .iter()
        .map(|b| {
            let names = [&b.title.romaji, &b.title.english, &b.title.native];
            let mut titles: Vec<String> = names
                .into_iter()
                .flatten()
                .chain(&b.synonyms)
                .map(|t| fold(t))
                .filter(|t| !t.is_empty())
                .collect();
            titles.dedup();
            Candidate { work: tiles::work_of(b.id).0, titles, format: Some(b.reading_format()) }
        })
        .collect();
    out.extend(results.iter().filter(|t| t.demo).map(|t| Candidate {
        work: t.work.0.clone(),
        titles: Vec::new(),
        format: None,
    }));
    out
}

/// A title folded for comparing: lower case, full-width forms as their plain ones, and
/// punctuation and spacing as single spaces. Apostrophes go, so "Wren's" is "wrens".
pub(crate) fn fold(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for c in title.chars() {
        let c = match u32::from(c) {
            // Full-width ASCII and the ideographic space.
            n @ 0xFF01..=0xFF5E => char::from_u32(n - 0xFEE0).unwrap_or(c),
            0x3000 => ' ',
            _ => c,
        };
        match c {
            '\'' | '\u{2019}' | '`' => {}
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ if !out.is_empty() && !out.ends_with(' ') => out.push(' '),
            _ => {}
        }
    }
    out.trim_end().to_owned()
}

/// The AniList result a hit joins, by its index in `candidates`: the one its kept match names,
/// or else the only one with its title (folded) whose format agrees. Two such, and it joins
/// neither.
pub(crate) fn join_of(
    hit: &SeriesHit,
    medium: Medium,
    stored: Option<&str>,
    candidates: &[Candidate],
) -> Option<usize> {
    if let Some(work) = stored
        && let Some(at) = candidates.iter().position(|c| c.work == work)
    {
        return Some(at);
    }
    let title = fold(&hit.title);
    if title.is_empty() {
        return None;
    }
    // A format the source names agrees when it is the same; one it doesn't name, or names in
    // words of its own, agrees when the medium does.
    let agrees = |c: &Candidate| match (&hit.format, &c.format) {
        (_, None) => false,
        (Some(theirs), Some(ours)) if theirs.medium().is_some() => same_format(theirs, ours),
        (_, Some(ours)) => ours.medium() == Some(medium),
    };
    let mut found = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.titles.contains(&title) && agrees(c))
        .map(|(at, _)| at);
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// Whether a source's format and AniList's are the same: a source that says manga for a one-shot
/// agrees, as one-shots are manga to most sources.
fn same_format(theirs: &ReadingFormat, ours: &ReadingFormat) -> bool {
    use ReadingFormat::{Manga, OneShot};
    theirs == ours || matches!((theirs, ours), (Manga, OneShot) | (OneShot, Manga))
}

/// The sources a search asks, in the person's order: on, and carrying the query's medium.
fn sources_for(
    kit: &Kit,
    medium: Option<Medium>,
) -> Vec<(String, String, Option<ReadSourceKind>)> {
    let registry = &kit.sources.registry;
    registry
        .ids()
        .into_iter()
        .filter(|id| !registry.is_off(id))
        .filter_map(|id| {
            let manifest = registry.manifest(&id)?;
            let carries =
                medium.is_none_or(|m| manifest.media.is_empty() || manifest.media.contains(&m));
            carries.then_some((id, manifest.name, kind_of(manifest.kind)))
        })
        .collect()
}

/// The word beside a group's name. Built-in sources show their name alone, and so do the
/// folders: all of them are one source named Folders, and "Folders  Folder" would say it twice.
fn kind_of(kind: BookSourceKind) -> Option<ReadSourceKind> {
    match kind {
        BookSourceKind::Server => Some(ReadSourceKind::Server),
        BookSourceKind::Plugin => Some(ReadSourceKind::Plugin),
        BookSourceKind::Folder | BookSourceKind::Native => None,
    }
}

/// Start the sources' part of a search with words: every group asking at once, then each
/// answer as it comes. Gives the run's generation.
fn sources_search(kit: &Kit, ask: Ask, query: &ReadSearchQuery, tx: &Outbox) -> u64 {
    kit.sources.adopt_plugins(&kit.plugins);
    let medium = medium_of(query);
    let asked = sources_for(kit, medium);
    let generation = {
        let mut run = kit.sources.catalog.search.lock().unwrap_or_else(|e| e.into_inner());
        let generation = run.generation + 1;
        *run = Run {
            generation,
            query: query.clone(),
            anilist: Some(if query.sources_only {
                ReadAnilistState::Skipped
            } else {
                ReadAnilistState::Asking
            }),
            groups: asked
                .iter()
                .map(|(id, name, kind)| Group {
                    id: id.clone(),
                    name: name.clone(),
                    kind: *kind,
                    mark: kit.sources.source_mark(id),
                    state: ReadSourceState::Asking,
                    hits: Vec::new(),
                })
                .collect(),
            now: ask.now,
            ..Run::default()
        };
        generation
    };
    update(kit, generation, tx, |_| {});
    let named: Vec<(String, String)> =
        asked.into_iter().map(|(id, name, _)| (id, name)).collect();
    ask_sources(kit, ask, generation, query.text.trim().to_owned(), medium, named, tx);
    generation
}

/// Ask each of `named` (id and the name its group shows) for `text`, each answer landing in
/// the run as it comes.
fn ask_sources(
    kit: &Kit,
    ask: Ask,
    generation: u64,
    text: String,
    medium: Option<Medium>,
    named: Vec<(String, String)>,
    tx: &Outbox,
) {
    if named.is_empty() {
        return;
    }
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        let text = &text;
        let mut asking: FuturesUnordered<_> = named
            .iter()
            .map(|(id, name)| {
                let kit = &kit;
                async move { (id.clone(), ask_source(kit, id, name, text, ask.now).await) }
            })
            .collect();
        while let Some((id, answer)) = asking.next().await {
            let local = kit.store.local_book_series().unwrap_or_default();
            let (state, hits) = match answer {
                Ok(hits) if hits.is_empty() => (ReadSourceState::Nothing, Vec::new()),
                Ok(hits) => {
                    let mut seen = HashSet::new();
                    let hits: Vec<Hit> = hits
                        .into_iter()
                        .filter_map(|hit| {
                            let medium = kit.sources.hit_medium(&id, &hit, medium, &local);
                            let tile = kit.sources.hit_tile(
                                &id,
                                hit.clone(),
                                Some(medium),
                                &local,
                                ask.now,
                            )?;
                            // A demo work is its own match: it joins the same work in AniList's
                            // results, where it follows them, and shows once.
                            let stored = if id == kaeri_core::demo::DEMO_SOURCE_ID {
                                Some(tile.work.0.clone())
                            } else {
                                kit.store.book_match(&id, &hit.id).ok().flatten()
                            };
                            seen.insert(tile.work.0.clone()).then_some(Hit {
                                hit,
                                tile,
                                stored,
                                medium,
                            })
                        })
                        .collect();
                    (ReadSourceState::Hits, hits)
                }
                Err(state) => (state, Vec::new()),
            };
            update(&kit, generation, &tx, |run| {
                if let Some(group) = run.groups.iter_mut().find(|g| g.id == id) {
                    group.state = state;
                    group.hits = hits;
                }
            });
        }
    });
}

/// One source's answer to a search, or the state its group shows instead. A failure names the
/// source as its group does.
async fn ask_source(
    kit: &Kit,
    id: &str,
    name: &str,
    text: &str,
    now: i64,
) -> Result<Vec<SeriesHit>, ReadSourceState> {
    let hints = BookHints::default();
    let asked = kit.sources.registry.ask(id, now, |source| {
        let hints = &hints;
        async move { source.search(text, hints).await }
    });
    match tokio::time::timeout(SOURCE_WAIT, asked).await {
        Err(_) => Err(ReadSourceState::TimedOut { seconds: READ_SOURCE_WAIT_SECS }),
        Ok(Ok(hits)) => Ok(hits),
        // A source without the title is not a failure.
        Ok(Err(ProviderError::NotFound)) => Ok(Vec::new()),
        Ok(Err(error)) => Err(ReadSourceState::Failed {
            failure: kit.sources.offline_or(failure(name, &error)),
        }),
    }
}

/// The sources changed while a search with words was last shown: a source that joined, or
/// joined anew since it answered, is asked for the words in its place in the person's order,
/// and a source gone or switched off leaves with its group. The rest keep what they answered.
pub(crate) fn sources_changed(kit: &Kit, ask: Ask, tx: &Outbox) {
    let (generation, text, medium, named) = {
        let mut run = kit.sources.catalog.search.lock().unwrap_or_else(|e| e.into_inner());
        if run.generation == 0 || run.query.text.trim().is_empty() {
            return;
        }
        let medium = medium_of(&run.query);
        let mut before = std::mem::take(&mut run.groups);
        let mut named = Vec::new();
        for (id, name, kind) in sources_for(kit, medium) {
            let mark = kit.sources.source_mark(&id);
            let kept = before
                .iter()
                .position(|g| g.id == id && g.mark == mark)
                .map(|at| before.remove(at));
            let group = kept.unwrap_or_else(|| {
                named.push((id.clone(), name.clone()));
                Group { id, name, kind, mark, state: ReadSourceState::Asking, hits: Vec::new() }
            });
            run.groups.push(group);
        }
        // Nothing joined, and nothing left with its group: the search stands as it was sent.
        if named.is_empty() && before.is_empty() {
            return;
        }
        (run.generation, run.query.text.trim().to_owned(), medium, named)
    };
    update(kit, generation, tx, |_| {});
    ask_sources(kit, ask, generation, text, medium, named, tx);
}

/// A group's Try again: ask its source once more for the search still shown.
pub fn retry_source(kit: &Kit, ask: Ask, query: ReadSearchQuery, source: String, tx: &Outbox) {
    let (generation, name) = {
        let mut run = kit.sources.catalog.search.lock().unwrap_or_else(|e| e.into_inner());
        // A group still asking has an ask under way, which answers for it: a second would
        // ask the source twice and let the slower answer overwrite the other.
        let ready = |g: &Group| g.id == source && g.state != ReadSourceState::Asking;
        if run.query != query || !run.groups.iter().any(ready) {
            return;
        }
        let Some(group) = run.groups.iter_mut().find(|g| g.id == source) else { return };
        group.state = ReadSourceState::Asking;
        group.hits.clear();
        let name = group.name.clone();
        (run.generation, name)
    };
    kit.sources.registry.retry_down();
    update(kit, generation, tx, |_| {});
    let text = query.text.trim().to_owned();
    ask_sources(kit, ask, generation, text, medium_of(&query), vec![(source, name)], tx);
}

/// Change the run, unless a newer search replaced it, and send the sources' part as it stands.
fn update(kit: &Kit, generation: u64, tx: &Outbox, change: impl FnOnce(&mut Run)) {
    let view = {
        let mut run = kit.sources.catalog.search.lock().unwrap_or_else(|e| e.into_inner());
        if run.generation != generation {
            return;
        }
        change(&mut run);
        view(kit, &mut run)
    };
    send(tx, ReadingEvent::SourceSearch(Box::new(view)));
}

/// The sources' part as it stands: each group with the hits that joined nothing, and the AniList
/// results the rest joined.
fn view(kit: &Kit, run: &mut Run) -> ReadSourceSearch {
    let anilist = run.anilist.unwrap_or(ReadAnilistState::Asking);
    // The joins wait for AniList's results: until then every hit shows in its group.
    let candidates: &[Candidate] =
        if anilist == ReadAnilistState::Answered { &run.candidates } else { &[] };
    let mut joins: Vec<ReadSourceJoin> = Vec::new();
    let mut groups = Vec::with_capacity(run.groups.len());
    let mut kept = Vec::new();
    for group in &run.groups {
        let mut tiles = Vec::new();
        let mut joined = 0;
        for hit in &group.hits {
            let Some(at) = join_of(&hit.hit, hit.medium, hit.stored.as_deref(), candidates)
            else {
                tiles.push(hit.tile.clone());
                continue;
            };
            joined += 1;
            let work = candidates[at].work.clone();
            if hit.stored.as_deref() != Some(work.as_str()) {
                kept.push((group.id.clone(), hit.hit.id.clone(), work.clone()));
            }
            let at = match joins.iter().position(|j| j.work.0 == work) {
                Some(at) => at,
                None => {
                    joins.push(ReadSourceJoin {
                        work: kaeri_core::books::WorkId::new(work),
                        source_ids: Vec::new(),
                        source_names: Vec::new(),
                    });
                    joins.len() - 1
                }
            };
            if !joins[at].source_ids.contains(&group.id) {
                joins[at].source_ids.push(group.id.clone());
                joins[at].source_names.push(group.name.clone());
            }
        }
        groups.push(ReadSourceGroup {
            source_id: group.id.clone(),
            source_name: group.name.clone(),
            kind: group.kind,
            state: group.state.clone(),
            tiles,
            joined,
        });
    }
    // A hit joined by its title is matched to the AniList entry, unconfirmed, so opening the
    // entry reads from that source. A match the person made for another work stays.
    for (source, series, work) in kept {
        if !run.kept_joins.insert((source.clone(), series.clone())) {
            continue;
        }
        let theirs = kit.store.book_match_state(&source, &series).ok().flatten();
        if theirs.is_some_and(|(other, confirmed)| confirmed && other != work) {
            continue;
        }
        if let Err(e) =
            kit.store.set_book_match(&source, &series, &work, 90, false, false, run.now)
        {
            tracing::warn!(error = %e, "a search's join was not kept");
        }
    }
    let settled = |state: &ReadSourceState| !matches!(state, ReadSourceState::Asking);
    let done = anilist != ReadAnilistState::Asking && groups.iter().all(|g| settled(&g.state));
    ReadSourceSearch { query: run.query.clone(), anilist, groups, joins, done }
}

/// A reading list: at once from what was kept, then again once the books missing from it land.
pub fn library(kit: &Kit, ask: Ask, segment: LibrarySegmentRead, tx: &Outbox) {
    super::changed::library(kit, segment);
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
        not_in_sources: false,
        origin: None,
        amount: None,
        started: None,
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
    use kaeri_core::{
        books::SeriesHit,
        media::{Medium, ReadingFormat},
    };

    use kaeri_core::books::BookSourceKind;
    use kaeri_model::reading::ReadSourceKind;

    use super::{Candidate, PER_PAGE, fold, join_of, kind_of, total};

    fn candidate(work: &str, titles: &[&str], format: Option<ReadingFormat>) -> Candidate {
        Candidate {
            work: work.into(),
            titles: titles.iter().map(|t| fold(t)).collect(),
            format,
        }
    }

    fn hit(title: &str, format: Option<ReadingFormat>) -> SeriesHit {
        SeriesHit { format, ..SeriesHit::new("s-1", title) }
    }

    #[test]
    fn manga_takes_in_one_shots_and_a_saved_one_shot_reads_as_manga() {
        let formats = |keys: &[&str]| {
            let query = kaeri_model::reading::ReadSearchQuery {
                formats: keys.iter().map(|k| (*k).to_owned()).collect(),
                ..Default::default()
            };
            let filter = super::filter_of(&query, false, 1_700_000_000);
            (filter.formats, filter.tags, filter.tags_excluded)
        };
        let manga = formats(&["manga"]);
        assert_eq!(manga.0, ["MANGA", "ONE_SHOT"]);
        assert_eq!(manga.2, [super::LONG_STRIP]);
        assert_eq!(formats(&["one-shot"]), manga);
        assert_eq!(formats(&["manga", "one-shot"]), manga);
        let webtoons = formats(&["webtoon"]);
        assert_eq!(webtoons.0, ["MANGA"]);
        assert_eq!(webtoons.1, [super::LONG_STRIP]);
        assert_eq!(formats(&["manga", "novel"]).0, ["MANGA", "ONE_SHOT", "NOVEL"]);
        let choices = kaeri_model::reading::read_filters().formats;
        let keys: Vec<&str> = choices.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["manga", "webtoon", "novel"], "one Manga choice, one-shots in it");
    }

    #[test]
    fn a_group_says_what_kind_of_source_it_is_unless_its_name_says_it() {
        assert_eq!(kind_of(BookSourceKind::Server), Some(ReadSourceKind::Server));
        assert_eq!(kind_of(BookSourceKind::Folder), None);
        assert_eq!(kind_of(BookSourceKind::Plugin), Some(ReadSourceKind::Plugin));
        assert_eq!(kind_of(BookSourceKind::Native), None);
    }

    #[test]
    fn the_total_is_given_only_once_the_last_page_is_in() {
        assert_eq!(total(0, 12, false), Some(12));
        assert_eq!(total(2, 7, false), Some(2 * PER_PAGE + 7));
        assert_eq!(total(0, 30, true), None, "AniList's own total is a stand-in until then");
        assert_eq!(total(0, 0, false), Some(0));
    }

    #[test]
    fn folding_ignores_case_width_and_punctuation() {
        assert_eq!(fold("The Lamplighter\u{2019}s ALMANAC!"), "the lamplighters almanac");
        assert_eq!(fold("\u{FF2C}antern\u{3000}Ferry"), "lantern ferry");
        assert_eq!(fold("Wren -- and: the Iron/Bridge "), "wren and the iron bridge");
        assert_eq!(fold("?!"), "");
    }

    #[test]
    fn a_hit_joins_the_one_entry_with_its_title_and_format() {
        let ferry = candidate(
            "a:1",
            &["Lantern Ferry", "The Lantern Ferry"],
            Some(ReadingFormat::Manga),
        );
        let notes = candidate("a:2", &["Lantern Street Notes"], Some(ReadingFormat::Novel));
        let all = [ferry, notes];
        let manga = Some(ReadingFormat::Manga);
        assert_eq!(
            join_of(&hit("the lantern ferry", manga.clone()), Medium::Manga, None, &all),
            Some(0)
        );
        // A source that says manga for a one-shot agrees; one that names no format goes by medium.
        assert_eq!(
            join_of(
                &hit("Lantern Ferry", Some(ReadingFormat::OneShot)),
                Medium::Manga,
                None,
                &all
            ),
            Some(0)
        );
        assert_eq!(
            join_of(&hit("Lantern Street Notes", None), Medium::Novel, None, &all),
            Some(1)
        );
        // The formats disagree, or the title is only close.
        assert_eq!(
            join_of(
                &hit("Lantern Ferry", Some(ReadingFormat::Novel)),
                Medium::Novel,
                None,
                &all
            ),
            None
        );
        assert_eq!(
            join_of(&hit("Lantern Street Notes", None), Medium::Manga, None, &all),
            None
        );
        assert_eq!(join_of(&hit("Lantern Ferry 2", manga), Medium::Manga, None, &all), None);
    }

    #[test]
    fn a_kept_match_joins_and_two_candidates_join_neither() {
        let manga = Some(ReadingFormat::Manga);
        let one = candidate("a:1", &["Saltwind Diary"], manga.clone());
        let other = candidate("a:2", &["Saltwind Diary"], manga.clone());
        let demo = candidate("demo:1", &["The Star Post"], None);
        let all = [one, other, demo];
        assert_eq!(
            join_of(&hit("Saltwind Diary", manga.clone()), Medium::Manga, None, &all),
            None
        );
        assert_eq!(
            join_of(&hit("Saltwind Diary", manga.clone()), Medium::Manga, Some("a:2"), &all),
            Some(1)
        );
        assert_eq!(
            join_of(&hit("Anything", manga.clone()), Medium::Manga, Some("demo:1"), &all),
            Some(2)
        );
        // A demo work joins only by its kept match.
        assert_eq!(join_of(&hit("The Star Post", manga), Medium::Manga, None, &all), None);
    }
}
