//! Read from: the sources that hold a series, and "Not found in", the ones searched for it
//! without a match.
//!
//! Opening a series asks every source that does not hold it yet, by the series' names. A series
//! whose name is the same, once normalised, becomes an unconfirmed match and so an edition; the
//! other sources are listed with why. A source asked a moment ago is not asked again: what it
//! answered is kept for a while, and a source the person adds later is asked when it appears.

use std::collections::{HashMap, HashSet};

use futures::{StreamExt, stream::FuturesUnordered};
use jiff::tz::TimeZone;
use kaeri_core::{
    books::{BookHints, SeriesHit, WorkId},
    error::ProviderError,
    media::Medium,
};
use kaeri_model::{
    Msg, ReadingMsg,
    book_sources::{BookSourceCommand, BookSourceEvent, SourceMiss, SourceSeriesChoice},
    event::Event,
};

use super::{Ask, Kit, series, tiles};
use crate::{
    outbox::Outbox,
    read_sources::{FOLDERS, failure},
};

/// How long a source's answer for a series stands before it is asked again.
pub(crate) const ASKED_FOR: i64 = 10 * 60;

/// The misses kept per work, each with when it was found.
#[derive(Debug, Default)]
pub(crate) struct Misses(HashMap<String, Vec<(i64, SourceMiss)>>);

/// A Read from command.
pub(crate) fn command(kit: &Kit, ask: Ask, command: BookSourceCommand, tx: &Outbox) {
    match command {
        BookSourceCommand::LoadForWork { work } => load(kit, ask, work, tx),
        BookSourceCommand::SearchSourceForWork { work, source_id, query } => {
            search_there(kit, ask, work, source_id, query, tx);
        }
        BookSourceCommand::ChooseSourceSeries { work, source_id, series_ref } => {
            choose(kit, ask, work, source_id, series_ref, tx);
        }
        other => tracing::debug!(command = ?other, "not a Read from command"),
    }
}

/// The names a work goes by, the one shown first.
fn names(kit: &Kit, work: &WorkId) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(kept) = tiles::anilist(work.as_str()).and_then(|id| kit.meta.kept_book(id)) {
        let title = &kept.book.title;
        out.push(kit.meta.titles().pick(title).to_owned());
        out.extend(
            [&title.romaji, &title.english, &title.native].into_iter().flatten().cloned(),
        );
        out.extend(kept.book.synonyms.iter().cloned());
    } else if let Some(name) = series::local_name(kit, work) {
        out.push(name);
    }
    let mut seen = HashSet::new();
    out.retain(|n| !n.trim().is_empty() && seen.insert(kaeri_meta::normalise(n)));
    out
}

/// What the work is, so sources that carry none of it are left out.
fn medium(kit: &Kit, work: &WorkId) -> Medium {
    match tiles::anilist(work.as_str()).and_then(|id| kit.meta.kept_book(id)) {
        Some(kept) => kept.book.medium(),
        None => series::local_medium(kit, work),
    }
}

/// How close a hit is to any of the work's names, 0 to 1.
fn closeness(names: &[String], hit: &SeriesHit) -> f64 {
    let theirs = std::iter::once(&hit.title).chain(&hit.synonyms);
    theirs
        .flat_map(|t| names.iter().map(move |n| kaeri_meta::similarity(n, t)))
        .fold(0.0, f64::max)
}

/// The sources worth searching for `work`, in the person's order: on, carrying its medium, not
/// a folder, and not holding it already.
fn searchable(kit: &Kit, work: &WorkId, medium: Medium) -> Vec<String> {
    let registry = &kit.sources.registry;
    let holding: HashSet<String> = kit
        .store
        .book_refs_for_work(work.as_str())
        .unwrap_or_default()
        .into_iter()
        .map(|(source, _)| source)
        .collect();
    registry
        .ids()
        .into_iter()
        .filter(|id| id != FOLDERS && !holding.contains(id) && !registry.is_off(id))
        .filter(|id| {
            registry
                .manifest(id)
                .is_some_and(|m| m.media.is_empty() || m.media.contains(&medium))
        })
        .collect()
}

/// "Not answering since 10:12", or plain "Not answering" when the time is unknown or was on
/// another day.
pub(crate) fn not_answering(since: Option<i64>, now: i64, zone: &TimeZone) -> Msg {
    let at = |t: i64| jiff::Timestamp::from_second(t).ok().map(|t| t.to_zoned(zone.clone()));
    let msg = match (since.and_then(at), at(now)) {
        (Some(since), Some(now)) if since.date() == now.date() => {
            ReadingMsg::ReadingMissDownSince { time: since.strftime("%H:%M").to_string() }
        }
        _ => ReadingMsg::ReadingMissDown,
    };
    Msg::Reading(msg)
}

/// What a source's search came to.
enum Answer {
    /// A series with the work's name, now matched.
    Found,
    Missed(Msg),
}

/// Search one source for the work by its names, matching a series whose name is the same.
async fn ask_one(kit: &Kit, source: &str, work: &WorkId, names: &[String], now: i64) -> Answer {
    let registry = &kit.sources.registry;
    let zone = kit.clock.zone();
    let down = || {
        let since = registry.health().get(source).and_then(|h| h.failing_since);
        Answer::Missed(not_answering(since, now, &zone))
    };
    // Down, and not yet due another ask.
    if !registry.candidates().iter().any(|s| s.manifest().id == source) {
        return down();
    }
    let hints = BookHints::default();
    // The name shown, then the other main one: two asks at most per source.
    for name in names.iter().take(2) {
        let hints = &hints;
        let found = registry.ask(source, now, |s| async move { s.search(name, hints).await });
        let hits = match found.await {
            Ok(hits) => hits,
            Err(ProviderError::NotFound) => Vec::new(),
            Err(ProviderError::Transport(_)) => return down(),
            Err(error) => return Answer::Missed(failure(source, &error).msg()),
        };
        for hit in hits {
            if closeness(names, &hit) < 1.0 {
                continue;
            }
            // A series the person tied to another work stays there.
            match kit.store.book_match_state(source, &hit.id) {
                Ok(Some((other, true))) if other != work.as_str() => continue,
                _ => {}
            }
            let _ =
                kit.store.set_book_match(source, &hit.id, work.as_str(), 90, false, false, now);
            return Answer::Found;
        }
    }
    Answer::Missed(Msg::Reading(ReadingMsg::ReadingMissNoSeries))
}

/// The misses still standing for `work`: fresh, and from sources that still lack it.
fn kept_misses(kit: &Kit, work: &WorkId, wanted: &[String], now: i64) -> Vec<SourceMiss> {
    let kept = kit.sources.catalog.misses.lock().unwrap_or_else(|e| e.into_inner());
    let Some(list) = kept.0.get(work.as_str()) else { return Vec::new() };
    let mut out: Vec<SourceMiss> = list
        .iter()
        .filter(|(at, miss)| now - at < ASKED_FOR && wanted.contains(&miss.source_id))
        .map(|(_, miss)| miss.clone())
        .collect();
    out.sort_by_key(|m| wanted.iter().position(|id| *id == m.source_id));
    out
}

fn keep_miss(kit: &Kit, work: &WorkId, miss: Option<SourceMiss>, source: &str, now: i64) {
    let mut kept = kit.sources.catalog.misses.lock().unwrap_or_else(|e| e.into_inner());
    let list = kept.0.entry(work.as_str().to_owned()).or_default();
    list.retain(|(at, m)| m.source_id != source && now - at < ASKED_FOR);
    if let Some(miss) = miss {
        list.push((now, miss));
    }
}

fn source_name(kit: &Kit, id: &str) -> String {
    kit.sources.registry.manifest(id).map_or_else(|| id.to_owned(), |m| m.name)
}

fn send_for_work(
    kit: &Kit,
    work: &WorkId,
    asking: bool,
    missing: Vec<SourceMiss>,
    tx: &Outbox,
) {
    let editions = kit.sources.editions(work);
    let event = BookSourceEvent::ForWork { work: work.clone(), editions, asking, missing };
    let _ = tx.send(Event::BookSources(event));
}

/// Read from for a work: the editions kept at once, then each source's answer as it comes.
pub(crate) fn load(kit: &Kit, ask: Ask, work: WorkId, tx: &Outbox) {
    kit.sources.adopt_plugins(&kit.plugins);
    let now = ask.now;
    let medium = medium(kit, &work);
    let wanted = searchable(kit, &work, medium);
    let names = names(kit, &work);
    let known: HashSet<String> =
        kept_misses(kit, &work, &wanted, now).into_iter().map(|m| m.source_id).collect();
    // Nothing to search by: the editions kept are the answer.
    let to_ask: Vec<String> = if names.is_empty() {
        Vec::new()
    } else {
        wanted.iter().filter(|id| !known.contains(*id)).cloned().collect()
    };
    send_for_work(kit, &work, !to_ask.is_empty(), kept_misses(kit, &work, &wanted, now), tx);
    if to_ask.is_empty() {
        return;
    }
    let had_edition = kit.sources.edition_of(&work, None).is_some();
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        let mut asking: FuturesUnordered<_> = to_ask
            .iter()
            .map(|source| {
                let (kit, work, names) = (kit.clone(), work.clone(), &names);
                async move { (source.clone(), ask_one(&kit, source, &work, names, now).await) }
            })
            .collect();
        while let Some((source, answer)) = asking.next().await {
            let miss = match answer {
                Answer::Found => None,
                Answer::Missed(reason) => Some(SourceMiss {
                    source_name: source_name(&kit, &source),
                    source_id: source.clone(),
                    reason,
                }),
            };
            keep_miss(&kit, &work, miss, &source, now);
            let wanted = searchable(&kit, &work, medium);
            let missing = kept_misses(&kit, &work, &wanted, now);
            send_for_work(&kit, &work, !asking.is_empty(), missing, &tx);
        }
        // A series that had nowhere to be read from now has: its page fills in.
        if !had_edition && kit.sources.edition_of(&work, None).is_some() {
            series::load(&kit, ask, work, &tx);
        }
    });
}

/// "Search there": one source's series for words the person typed, closest first.
pub(crate) fn search_there(
    kit: &Kit,
    ask: Ask,
    work: WorkId,
    source: String,
    query: String,
    tx: &Outbox,
) {
    kit.sources.adopt_plugins(&kit.plugins);
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        let mut names = names(&kit, &work);
        names.insert(0, query.clone());
        let hints = BookHints::default();
        let asked = kit
            .sources
            .registry
            .ask(&source, ask.now, |s| {
                let query = query.clone();
                let hints = hints.clone();
                async move { s.search(&query, &hints).await }
            })
            .await;
        let (choices, failure) = match asked {
            Ok(hits) => (hits, None),
            Err(ProviderError::NotFound) => (Vec::new(), None),
            Err(error) => (Vec::new(), Some(failure(&source, &error))),
        };
        // Closeness to the series' own names, not to what was typed.
        let names = &names[1..];
        let mut choices: Vec<SourceSeriesChoice> = choices
            .into_iter()
            .map(|hit| SourceSeriesChoice {
                confidence: (closeness(names, &hit) * 100.0).round().clamp(0.0, 100.0) as u8,
                series_ref: hit.id,
                title: hit.title,
                cover: hit.cover,
                year: hit.year,
                language: hit.language,
            })
            .collect();
        choices.sort_by_key(|c| std::cmp::Reverse(c.confidence));
        let event =
            BookSourceEvent::SourceSeries { work, source_id: source, query, choices, failure };
        let _ = tx.send(Event::BookSources(event));
    });
}

/// The person picked a source's series as this work: a confirmed match, then Read from and the
/// page again.
pub(crate) fn choose(
    kit: &Kit,
    ask: Ask,
    work: WorkId,
    source: String,
    series_ref: String,
    tx: &Outbox,
) {
    // A folder's series stays on this device; a source's is shared with the person's others.
    let share = source != FOLDERS;
    if let Err(e) =
        kit.store.set_book_match(&source, &series_ref, work.as_str(), 100, true, share, ask.now)
    {
        tracing::warn!(error = %e, "the chosen series was not kept");
    }
    keep_miss(kit, &work, None, &source, ask.now);
    let wanted = searchable(kit, &work, medium(kit, &work));
    send_for_work(kit, &work, false, kept_misses(kit, &work, &wanted, ask.now), tx);
    series::load(kit, ask, work, tx);
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU32, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use kaeri_core::{
        books::{
            BookSourceKind, BookSourceManifest, ChapterContent, SeriesDetail, SourceChapter,
        },
        config::Paths,
        traits::BookSource,
    };
    use kaeri_store::{Store, book_catalog::BookWork};
    use tokio::sync::mpsc;

    use super::*;
    use crate::read_sources::BookSources;

    const WORK: &str = "a:900301";
    const DAY: i64 = 86_400;
    // Noon UTC on a fixed day, read in UTC, so nothing depends on the machine's clock or zone.
    const NOW: i64 = 1_700_000_000 - 1_700_000_000 % DAY + 12 * 3_600;

    /// A source that answers every search the same way and counts the asks.
    struct Stock {
        manifest: BookSourceManifest,
        answer: Result<Vec<SeriesHit>, ProviderError>,
        asked: AtomicU32,
    }

    impl Stock {
        fn new(
            id: &str,
            media: &[Medium],
            answer: Result<Vec<SeriesHit>, ProviderError>,
        ) -> Arc<Self> {
            let manifest = BookSourceManifest {
                id: id.into(),
                name: format!("{id} shelf"),
                version: "1".into(),
                kind: BookSourceKind::Native,
                media: media.to_vec(),
                languages: vec!["en".into()],
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            };
            Arc::new(Self { manifest, answer, asked: AtomicU32::new(0) })
        }

        fn asks(&self) -> u32 {
            self.asked.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl BookSource for Stock {
        fn manifest(&self) -> &BookSourceManifest {
            &self.manifest
        }

        async fn search(
            &self,
            _: &str,
            _: &BookHints,
        ) -> Result<Vec<SeriesHit>, ProviderError> {
            self.asked.fetch_add(1, Ordering::Relaxed);
            self.answer.clone()
        }

        async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
            Ok(SeriesDetail::new(id, "The Lantern Ferry"))
        }

        async fn chapters(
            &self,
            _: &str,
            _: &BookHints,
        ) -> Result<Vec<SourceChapter>, ProviderError> {
            Ok(Vec::new())
        }

        async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
            Err(ProviderError::NotFound)
        }
    }

    fn hits(titles: &[(&str, &str)]) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(titles.iter().map(|(id, title)| SeriesHit::new(*id, *title)).collect())
    }

    fn kit(dir: &std::path::Path, sources: &[&Arc<Stock>]) -> Kit {
        let store = Store::open_in_memory().unwrap();
        let paths = Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
        };
        let tokens = kaeri_track::TokenStore::new(dir.join("tokens"));
        let books = BookSources::new(store.clone(), tokens, reqwest::Client::new(), &paths);
        for source in sources {
            books.registry.put((*source).clone());
        }
        let http = kaeri_net::HttpClient::offline(&Default::default()).unwrap();
        let meta =
            kaeri_meta::Meta::new(kaeri_meta::AniList::new(http, 30), None, store.clone());
        let meta_row = r#"{"book":{"id":900301,"title":{"romaji":"The Lantern Ferry","english":"Lantern Ferry"},"synonyms":["Ferry of Lanterns"]}}"#;
        let work = BookWork {
            work: WORK.into(),
            anilist_id: Some(900_301),
            medium: "manga".into(),
            titles: "{}".into(),
            meta: Some(meta_row.into()),
            fetched_at: NOW,
            ..BookWork::default()
        };
        store.put_book_work(&work).unwrap();
        Kit {
            meta,
            store,
            sources: books,
            plugins: crate::book_plugins::BookPlugins::default(),
            clock: crate::clock::Clock::in_zone(TimeZone::UTC),
            offline: Arc::new(AtomicBool::new(false)),
        }
    }

    fn ask(now: i64) -> Ask {
        Ask {
            answer: kaeri_model::medium::KaeriIsFor::Read,
            adult: false,
            chapter_notices: false,
            now,
        }
    }

    /// The Read from answers until one says every source has answered.
    async fn answers(
        rx: &mut mpsc::UnboundedReceiver<Event>,
    ) -> Vec<(Vec<String>, bool, Vec<SourceMiss>)> {
        let mut out = Vec::new();
        loop {
            let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("Read from answers")
                .expect("the outbox is open");
            if let Event::BookSources(BookSourceEvent::ForWork {
                editions,
                asking,
                missing,
                ..
            }) = event
            {
                let ids = editions.into_iter().map(|e| e.source_id).collect();
                out.push((ids, asking, missing));
                if !asking {
                    return out;
                }
            }
        }
    }

    fn reasons(missing: &[SourceMiss]) -> Vec<(&str, &str)> {
        missing.iter().map(|m| (m.source_id.as_str(), m.reason.id())).collect()
    }

    #[tokio::test]
    async fn read_from_searches_the_sources_without_the_series_and_says_why_the_rest_missed() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let harbor = Stock::new(
            "harbor",
            &manga,
            hits(&[("h-9", "Lantern Ferry: Side Trips"), ("h-1", "the lantern ferry!")]),
        );
        let attic = Stock::new("attic", &manga, hits(&[("a-1", "Lantern Ferry Side Trips")]));
        let cellar =
            Stock::new("cellar", &manga, Err(ProviderError::Transport("refused".into())));
        let pantry = Stock::new("pantry", &manga, hits(&[("p-1", "The Lantern Ferry")]));
        let novels =
            Stock::new("novels", &[Medium::Novel], hits(&[("n-1", "The Lantern Ferry")]));
        let kit = kit(dir.path(), &[&harbor, &attic, &cellar, &pantry, &novels]);
        kit.sources.registry.set_off(["pantry".to_owned()]);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let out = Outbox::from(tx);
        let work = WorkId::new(WORK);

        load(&kit, ask(NOW), work.clone(), &out);
        let seen = answers(&mut rx).await;
        let (first, last) = (&seen[0], seen.last().unwrap());
        assert!(first.0.is_empty() && first.1 && first.2.is_empty(), "nothing kept, asking");
        assert_eq!(seen.len(), 4, "the start, then one per source asked");
        assert_eq!(last.0, ["harbor"], "the same name, once normalised, is an edition");
        assert_eq!(
            reasons(&last.2),
            [("attic", "reading-miss-no-series"), ("cellar", "reading-miss-down-since")],
            "in the person's order; a source switched off or without manga is not listed"
        );
        assert_eq!(last.2[0].source_name, "attic shelf");
        assert_eq!(
            last.2[1].reason,
            Msg::Reading(ReadingMsg::ReadingMissDownSince { time: "12:00".into() })
        );
        assert_eq!(
            kit.store.book_match_state("harbor", "h-1").unwrap(),
            Some((WORK.into(), false)),
            "matched, not confirmed"
        );
        assert_eq!(pantry.asks() + novels.asks(), 0);
        assert_eq!(attic.asks(), 2, "the name shown, then the other main one");

        // Opened again a minute later: the answers stand and nobody is asked.
        load(&kit, ask(NOW + 60), work.clone(), &out);
        let again = answers(&mut rx).await;
        assert_eq!(again.len(), 1);
        assert_eq!(reasons(&again[0].2).len(), 2);
        assert_eq!((attic.asks(), cellar.asks(), harbor.asks()), (2, 1, 1));

        // A source added since is asked; the others are not, until their answers grow old.
        let loft = Stock::new("loft", &manga, Ok(Vec::new()));
        kit.sources.registry.put(loft.clone());
        load(&kit, ask(NOW + 120), work.clone(), &out);
        let added = answers(&mut rx).await;
        assert_eq!(
            reasons(&added.last().unwrap().2),
            [
                ("attic", "reading-miss-no-series"),
                ("cellar", "reading-miss-down-since"),
                ("loft", "reading-miss-no-series")
            ]
        );
        assert_eq!((attic.asks(), loft.asks()), (2, 2));
        load(&kit, ask(NOW + ASKED_FOR + 60), work, &out);
        let _ = answers(&mut rx).await;
        assert_eq!(attic.asks(), 4, "asked again once its answer is old");
    }

    #[tokio::test]
    async fn search_there_offers_the_closest_first_and_choosing_one_makes_it_an_edition() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let attic = Stock::new(
            "attic",
            &manga,
            hits(&[("a-2", "Paper Cranes"), ("a-1", "Ferry of Lanterns (Colour)")]),
        );
        let cellar =
            Stock::new("cellar", &manga, Err(ProviderError::Transport("refused".into())));
        let kit = kit(dir.path(), &[&attic, &cellar]);
        let work = WorkId::new(WORK);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let out = Outbox::from(tx);

        load(&kit, ask(NOW), work.clone(), &out);
        let _ = answers(&mut rx).await;

        search_there(&kit, ask(NOW), work.clone(), "attic".into(), "ferry".into(), &out);
        let (choices, failure) = loop {
            if let Some(Event::BookSources(BookSourceEvent::SourceSeries {
                choices,
                failure,
                query,
                ..
            })) = rx.recv().await
            {
                assert_eq!(query, "ferry");
                break (choices, failure);
            }
        };
        assert_eq!(failure, None);
        let order: Vec<&str> = choices.iter().map(|c| c.series_ref.as_str()).collect();
        assert_eq!(order, ["a-1", "a-2"], "closest to the series' own names first");
        assert!(choices[0].confidence > choices[1].confidence);

        search_there(&kit, ask(NOW), work.clone(), "cellar".into(), "ferry".into(), &out);
        let failure = loop {
            if let Some(Event::BookSources(BookSourceEvent::SourceSeries { failure, .. })) =
                rx.recv().await
            {
                break failure;
            }
        };
        assert_eq!(failure, Some(kaeri_model::reading::ReadingFailure::Offline));

        choose(&kit, ask(NOW), work.clone(), "attic".into(), "a-1".into(), &out);
        let (editions, asking, missing) = answers(&mut rx).await.remove(0);
        assert_eq!(editions, ["attic"]);
        assert!(!asking);
        assert_eq!(
            reasons(&missing),
            [("cellar", "reading-miss-down-since")],
            "no longer missing there"
        );
        assert_eq!(
            kit.store.book_match_state("attic", "a-1").unwrap(),
            Some((WORK.into(), true))
        );
    }

    #[test]
    fn not_answering_names_the_time_only_on_the_same_day_where_the_person_is() {
        let utc = TimeZone::UTC;
        let since = |at| not_answering(at, NOW, &utc);
        let at =
            |time: &str| Msg::Reading(ReadingMsg::ReadingMissDownSince { time: time.into() });
        assert_eq!(since(Some(NOW - 2 * 3_600 + 12 * 60)), at("10:12"));
        assert_eq!(since(Some(NOW - DAY)), Msg::Reading(ReadingMsg::ReadingMissDown));
        assert_eq!(since(None), Msg::Reading(ReadingMsg::ReadingMissDown));
        let east = TimeZone::fixed(jiff::tz::offset(2));
        assert_eq!(not_answering(Some(NOW - 2 * 3_600 + 12 * 60), NOW, &east), at("12:12"));
    }
}
