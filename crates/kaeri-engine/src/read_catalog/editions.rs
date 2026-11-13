//! Read from: the sources that hold a series, and "Not found in", the ones searched for it
//! without a match.
//!
//! Opening a series asks every source that does not hold it yet, by the series' names: the name
//! shown, its romaji and English names, the names before their subtitles and a key word, a few
//! searches a source at most. A series is taken by the rules in [`fit`]: the same name with
//! nothing against it, or the name spelt another way with the year or a maker to back it. One
//! that may be another edition ("Full Color") is offered as a possible match for the person to
//! confirm, never taken. A source asked a moment ago is not asked again: what it answered is
//! kept for a while, and a source the person adds later is asked when it appears.

use std::collections::{HashMap, HashSet, VecDeque};

use futures::{StreamExt, stream::FuturesUnordered};
use jiff::tz::TimeZone;
use kaeri_core::{
    books::{BookHints, BookSourceKind, SeriesHit, WorkId},
    error::ProviderError,
    media::Medium,
};
use kaeri_model::{
    Msg, ReadingMsg,
    book_sources::{BookSourceCommand, BookSourceEvent, EditionMiss, SourceSeriesChoice},
    event::Event,
};

use super::{
    Ask, Kit,
    chapters::{self, Asked},
    fit::{self, Facts, Fit, Title},
    series, tiles,
};
use crate::{outbox::Outbox, read_sources::FOLDERS};

/// How long a source's answer for a series stands before it is asked again.
pub(crate) const ASKED_FOR: i64 = 10 * 60;

/// Searches one source gets for a work, at most: its three main names, each again as words
/// alone when the source found nothing with the punctuation in, and the key word.
const SEARCHES: usize = 6;

/// Series one source is asked about beyond its search, at most: their chapters, and their page
/// for a name spelt another way.
const LOOKS: usize = 3;

/// How sure a match the name rules made by themselves is.
pub(crate) const MATCHED: u8 = 92;

/// How sure a match made by itself before the name rules was: its series' name is checked once.
const OLDER: u8 = 90;

/// The misses kept per work, each with when it was found and the
/// [`crate::read_sources::BookSources::source_mark`] of the source that missed.
#[derive(Debug, Default)]
pub(crate) struct Misses(HashMap<String, Vec<(i64, u64, EditionMiss)>>);

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

/// What is known of a work to judge a source's series by: its names, the one shown first, and
/// its medium, year, chapters and makers.
pub(crate) fn about(kit: &Kit, work: &WorkId) -> fit::Work {
    let Some(kept) = tiles::anilist(work.as_str()).and_then(|id| kit.meta.kept_book(id)) else {
        let names = series::local_name(kit, work).into_iter().collect();
        return fit::Work::named(names, series::local_medium(kit, work));
    };
    let book = &kept.book;
    let title = &book.title;
    let mut names = vec![kit.meta.titles().pick(title).to_owned()];
    names.extend([&title.romaji, &title.english, &title.native].into_iter().flatten().cloned());
    names.extend(book.synonyms.iter().cloned());
    let mut seen = HashSet::new();
    names.retain(|n| !n.trim().is_empty() && seen.insert(kaeri_meta::normalise(n)));
    let authors = book
        .staff
        .edges
        .iter()
        .filter(|e| e.role.as_deref().is_some_and(fit::is_maker))
        .filter_map(|e| e.node.as_ref()?.name.full.clone())
        .collect();
    fit::Work {
        names,
        romaji: title.romaji.clone(),
        english: title.english.clone(),
        native: title.native.clone(),
        medium: book.medium(),
        one_shot: book.format.as_deref() == Some("ONE_SHOT"),
        year: book.start_date.as_ref().and_then(|d| d.year).map(u32::from),
        chapters: book.chapters,
        authors,
    }
}

/// What to search a source by, in order: the name shown, the romaji and English names, the key
/// word, the names before their subtitles, then the native name where the source reads its
/// language, and the other names. Each is searched once.
fn queries(work: &fit::Work, languages: &[String]) -> Vec<String> {
    let main = [work.names.first().cloned(), work.romaji.clone(), work.english.clone()];
    let mut out: Vec<String> = main.iter().flatten().cloned().collect();
    out.extend(fit::key_word(work));
    out.extend(main.iter().flatten().filter_map(|n| fit::head(n)).map(fit::plain));
    let native = |l: &String| ["ja", "ko", "zh"].iter().any(|p| l.starts_with(p));
    let reads_native = languages.is_empty() || languages.iter().any(native);
    if reads_native {
        out.extend(work.native.clone());
    }
    out.extend(
        work.names
            .iter()
            .skip(1)
            .filter(|n| reads_native || Some(*n) != work.native.as_ref())
            .cloned(),
    );
    let mut seen = HashSet::new();
    out.retain(|q| !q.trim().is_empty() && seen.insert(q.trim().to_lowercase()));
    out
}

/// What a source's search came to.
enum Answer {
    /// A series that is the work, now matched.
    Found(String),
    /// No series that is the work, and one that may be another edition of it.
    Possible { choice: SourceSeriesChoice, query: String },
    /// Nothing, and why; with the words that came closest, for Search there.
    Missed { reason: Msg, query: Option<String> },
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

/// Whether a source lists chapters, so its count says something: a plugin's or a fixture's. A
/// server's or a folder's books may each hold many chapters.
pub(crate) fn counts_chapters(
    sources: &crate::read_sources::BookSources,
    source: &str,
) -> bool {
    source != FOLDERS
        && sources
            .registry
            .manifest(source)
            .is_some_and(|m| matches!(m.kind, BookSourceKind::Plugin | BookSourceKind::Native))
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

/// A series the person tied to another work stays there.
fn tied_elsewhere(kit: &Kit, source: &str, series: &str, work: &WorkId) -> bool {
    matches!(
        kit.store.book_match_state(source, series),
        Ok(Some((other, true))) if other != work.as_str()
    )
}

fn choice(hit: SeriesHit, confidence: u8) -> SourceSeriesChoice {
    SourceSeriesChoice {
        confidence,
        series_ref: hit.id,
        title: hit.title,
        cover: hit.cover,
        year: hit.year,
        language: hit.language,
    }
}

fn rank(title: Title) -> u8 {
    match title {
        Title::Same => 0,
        Title::Spelt => 1,
        Title::Subtitle => 2,
        Title::Edition(_) => 3,
        Title::Other => 4,
    }
}

/// What a source says of a series beyond its search: its chapter list, unless the search
/// counted them, and its page for a name that is not the same, which needs the year or a maker
/// to agree.
async fn look(
    kit: &Kit,
    source: &str,
    hit: &SeriesHit,
    title: Title,
    work: &fit::Work,
    now: i64,
) -> Facts {
    let registry = &kit.sources.registry;
    let hints = BookHints::default();
    let mut facts = Facts::of_hit(hit);
    if !counts_chapters(&kit.sources, source) {
        facts.chapters = None;
    } else if facts.chapters.is_none()
        && let Ok(list) = registry.chapters(source, &hit.id, &hints, now).await
    {
        facts.count(&list);
        chapters::keep(&kit.store, source, &hit.id, &list, now);
    }
    let year_backs = work.year.is_some() && facts.year.is_some();
    if title != Title::Same
        && (title == Title::Subtitle || !year_backs)
        && let Ok(detail) = registry.series(source, &hit.id, &hints, now).await
    {
        facts.authors = detail.authors;
        if facts.format.is_none() {
            facts.format = detail.format;
        }
    }
    facts
}

/// Search one source for the work by its names, matching the series that is the work.
async fn ask_one(kit: &Kit, source: &str, id: &WorkId, work: &fit::Work, now: i64) -> Answer {
    let registry = &kit.sources.registry;
    let zone = kit.clock.zone();
    let down = || {
        let since = registry.health().get(source).and_then(|h| h.failing_since);
        Answer::Missed { reason: not_answering(since, now, &zone), query: None }
    };
    // Down, and not yet due another ask.
    if !registry.candidates().iter().any(|s| s.manifest().id == source) {
        return down();
    }
    let languages = registry.manifest(source).map(|m| m.languages).unwrap_or_default();
    let mut queue: VecDeque<String> = queries(work, &languages).into();
    let hints = BookHints::default();
    let (mut searched, mut looked) = (0, 0);
    let mut tried = HashSet::new();
    let mut judged = HashSet::new();
    let mut possible: Option<(SeriesHit, String)> = None;
    // The words that found something like the work, for Search there.
    let mut closest: Option<String> = None;
    while searched < SEARCHES
        && let Some(query) = queue.pop_front()
    {
        if !tried.insert(query.to_lowercase()) {
            continue;
        }
        searched += 1;
        let asked = registry.ask(source, now, |s| {
            let (query, hints) = (query.clone(), hints.clone());
            async move { s.search(&query, &hints).await }
        });
        let hits = match asked.await {
            Ok(hits) => hits,
            Err(ProviderError::NotFound) => Vec::new(),
            Err(ProviderError::Transport(_)) => return down(),
            Err(error) => {
                let reason = kit.sources.failure(source, &error).msg();
                return Answer::Missed { reason, query: None };
            }
        };
        // Some sources find nothing once there is punctuation in the words.
        if hits.is_empty() {
            let plain = fit::plain(&query);
            if !plain.is_empty() && plain != query {
                queue.push_front(plain);
            }
            continue;
        }
        let mut near: Vec<(Title, SeriesHit)> = Vec::new();
        for hit in hits {
            if !judged.insert(hit.id.clone()) || tied_elsewhere(kit, source, &hit.id, id) {
                continue;
            }
            match fit::hit_title(work, &hit) {
                Title::Other => {}
                Title::Edition(_) => {
                    if possible.is_none() && fit::fit(work, &Facts::of_hit(&hit)) != Fit::Wrong
                    {
                        possible = Some((hit, query.clone()));
                    }
                }
                title => near.push((title, hit)),
            }
        }
        if !near.is_empty() || possible.is_some() {
            closest.get_or_insert_with(|| query.clone());
        }
        near.sort_by_key(|(title, _)| rank(*title));
        let mut taken: Vec<(SeriesHit, Option<u32>)> = Vec::new();
        for (title, hit) in near {
            if looked >= LOOKS {
                break;
            }
            looked += 1;
            let facts = look(kit, source, &hit, title, work, now).await;
            if fit::taken(title, fit::fit(work, &facts)) {
                taken.push((hit, facts.chapters));
            }
        }
        // Of two that are the work, as a series beside a colour release that kept its name, the
        // one whose chapters are nearest the work's.
        if let Some((hit, _)) = taken.into_iter().min_by_key(|(_, n)| fit::distance(work, *n)) {
            let _ = kit.store.set_book_match(
                source,
                &hit.id,
                id.as_str(),
                MATCHED,
                false,
                false,
                now,
            );
            return Answer::Found(hit.id);
        }
    }
    if let Some((hit, query)) = possible {
        return Answer::Possible { choice: choice(hit, 70), query };
    }
    let query = closest.or_else(|| work.romaji.clone()).or_else(|| work.names.first().cloned());
    Answer::Missed { reason: Msg::Reading(ReadingMsg::ReadingMissNoSeries), query }
}

/// The series matched to the work by itself whose kept chapters show it is not the work: a
/// one-shot or pilot of a serialised work, or far more chapters than it has. A series the person
/// chose stays. Each stays matched until its source, searched again, answers (see [`let_go`]), so
/// the edition read from does not move to another source meanwhile. One whose source cannot be
/// searched (no name to search by, or switched off) is let go at once.
fn doubted(kit: &Kit, id: &WorkId, work: &fit::Work) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (source, series) in kit.store.book_refs_for_work(id.as_str()).unwrap_or_default() {
        if !counts_chapters(&kit.sources, &source)
            || !matches!(kit.store.book_match_state(&source, &series), Ok(Some((_, false))))
        {
            continue;
        }
        let Some((list, _)) = chapters::kept(&kit.store, &source, &series) else { continue };
        let mut facts = Facts::default();
        facts.count_labels(
            list.iter().map(|c| (c.number.clone(), c.label.as_str(), c.title.as_deref())),
        );
        if matches!(fit::fit(work, &facts), Fit::OneShot | Fit::Wrong) {
            tracing::info!(
                source,
                "a series matched by itself is not the work; searching again"
            );
            if work.names.is_empty() || kit.sources.registry.is_off(&source) {
                let _ = kit.store.forget_book_match(&source, &series);
            } else {
                out.push((source, series));
            }
        }
    }
    out
}

/// A doubted match's source answered: the match goes, unless the search found that same series.
fn let_go(kit: &Kit, source: &str, series: &str, answer: &Answer) {
    if !matches!(answer, Answer::Found(found) if found == series) {
        let _ = kit.store.forget_book_match(source, series);
    }
}

/// The matches made by themselves before the name rules, on sources that name their series.
fn older(kit: &Kit, id: &WorkId) -> Vec<(String, String)> {
    kit.store
        .book_refs_for_work(id.as_str())
        .unwrap_or_default()
        .into_iter()
        .filter(|(source, series)| {
            counts_chapters(&kit.sources, source)
                && matches!(
                    kit.store.book_match_confidence(source, series),
                    Ok(Some((OLDER, false)))
                )
        })
        .collect()
}

/// Check each older match by its series' name once: another edition of the work, as a
/// colour release, or a one-shot is doubted, to be let go once its source is searched again
/// (one on a source switched off goes at once). The rest are kept as matched by the rules.
/// Answers the matches doubted.
async fn recheck(kit: &Kit, id: &WorkId, work: &fit::Work, now: i64) -> Vec<(String, String)> {
    let mut doubted = Vec::new();
    for (source, series) in older(kit, id) {
        let hints = BookHints::default();
        let asked = kit.sources.registry.series(&source, &series, &hints, now);
        let Ok(detail) = asked.await else { continue };
        let facts = Facts { format: detail.format, ..Facts::default() };
        let edition = matches!(fit::title(work, &detail.title), Title::Edition(_));
        if edition || fit::fit(work, &facts) == Fit::OneShot {
            tracing::info!(
                source,
                "an older match is another edition of the work; searching again"
            );
            if kit.sources.registry.is_off(&source) {
                let _ = kit.store.forget_book_match(&source, &series);
            } else {
                doubted.push((source, series));
            }
        } else {
            let _ = kit.store.set_book_match(
                &source,
                &series,
                id.as_str(),
                MATCHED,
                false,
                false,
                now,
            );
        }
    }
    doubted
}

/// The misses still standing for `work`: fresh, from sources that still lack it, and said by
/// the source as it stands. A source updated or put again since it missed is asked anew.
fn kept_misses(kit: &Kit, work: &WorkId, wanted: &[String], now: i64) -> Vec<EditionMiss> {
    let kept = kit.sources.catalog.misses.lock().unwrap_or_else(|e| e.into_inner());
    let Some(list) = kept.0.get(work.as_str()) else { return Vec::new() };
    let mut out: Vec<EditionMiss> = list
        .iter()
        .filter(|(at, mark, miss)| {
            now - at < ASKED_FOR
                && wanted.contains(&miss.source_id)
                && kit.sources.source_mark(&miss.source_id) == Some(*mark)
        })
        .map(|(_, _, miss)| miss.clone())
        .collect();
    out.sort_by_key(|m| wanted.iter().position(|id| *id == m.source_id));
    out
}

fn keep_miss(kit: &Kit, work: &WorkId, miss: Option<EditionMiss>, source: &str, now: i64) {
    let mark = kit.sources.source_mark(source);
    let mut kept = kit.sources.catalog.misses.lock().unwrap_or_else(|e| e.into_inner());
    let list = kept.0.entry(work.as_str().to_owned()).or_default();
    list.retain(|(at, _, m)| m.source_id != source && now - at < ASKED_FOR);
    if let (Some(miss), Some(mark)) = (miss, mark) {
        list.push((now, mark, miss));
    }
}

fn source_name(kit: &Kit, id: &str) -> String {
    kit.sources.source_name(id)
}

fn send_for_work(
    kit: &Kit,
    work: &WorkId,
    asking: bool,
    missing: Vec<EditionMiss>,
    tx: &Outbox,
) {
    let editions = kit.sources.editions(work);
    let event = BookSourceEvent::ForWork { work: work.clone(), editions, asking, missing };
    let _ = tx.send(Event::BookSources(event));
}

/// A search of a work's sources by name, under way: it ends when the sender goes.
pub(crate) type Check = tokio::sync::watch::Receiver<()>;

/// How long a check waits for book plugins still compiling before it searches without them.
pub(crate) const PLUGINS_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// The check under way for `work`, if one is.
fn running(kit: &Kit, work: &WorkId) -> Option<Check> {
    kit.sources
        .catalog
        .checks
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(work.as_str())
        .cloned()
}

/// Wait for a check to end.
pub(crate) async fn ended(mut check: Check) {
    // The sender going is the end; nothing is ever sent on it.
    let _ = check.changed().await;
}

/// Search the sources for `work` unless that is under way or was done lately: the check to wait
/// for before saying no source has it, or `None` when nothing is being asked.
pub(crate) fn check(kit: &Kit, ask: Ask, work: &WorkId, tx: &Outbox) -> Option<Check> {
    if let Some(check) = running(kit, work) {
        return Some(check);
    }
    load(kit, ask, work.clone(), tx);
    running(kit, work)
}

/// The sources still to search for `work` by name, in the person's order, and those holding a
/// `doubted` match.
fn to_ask(
    kit: &Kit,
    work: &WorkId,
    about: &fit::Work,
    now: i64,
    doubted: &[(String, String)],
) -> Vec<String> {
    let wanted = searchable(kit, work, about.medium);
    let known: HashSet<String> =
        kept_misses(kit, work, &wanted, now).into_iter().map(|m| m.source_id).collect();
    // Nothing to search by: the editions kept are the answer.
    if about.names.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = wanted.into_iter().filter(|id| !known.contains(id)).collect();
    for (source, _) in doubted {
        if !out.contains(source) {
            out.push(source.clone());
        }
    }
    out
}

/// Read from for a work: the editions kept at once, then each source's answer as it comes. A
/// search already under way for it is not started twice: its answers are on their way.
pub(crate) fn load(kit: &Kit, ask: Ask, work: WorkId, tx: &Outbox) {
    let now = ask.now;
    kit.sources.adopt_plugins(&kit.plugins);
    let about = about(kit, &work);
    let medium = about.medium;
    let wanted = searchable(kit, &work, medium);
    if running(kit, &work).is_some() {
        send_for_work(kit, &work, true, kept_misses(kit, &work, &wanted, now), tx);
        return;
    }
    let doubted = doubted(kit, &work, &about);
    let to_ask = to_ask(kit, &work, &about, now, &doubted);
    // Matches made by themselves before the name rules are checked by their series' name once.
    let recheck_older = !about.names.is_empty() && !older(kit, &work).is_empty();
    let arriving = kit.plugins.is_arriving();
    send_for_work(
        kit,
        &work,
        arriving || recheck_older || !to_ask.is_empty(),
        kept_misses(kit, &work, &wanted, now),
        tx,
    );
    list_unlisted(kit, &work, now, tx);
    if !arriving && !recheck_older && to_ask.is_empty() {
        settled(kit, ask, &work, tx);
        return;
    }
    let before = kit.sources.edition_of(&work, None);
    let (ended, check) = tokio::sync::watch::channel(());
    kit.sources
        .catalog
        .checks
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(work.as_str().to_owned(), check);
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        // A plugin still compiling is asked once it has joined.
        let mut to_ask = if arriving {
            kit.plugins.arrived(PLUGINS_WAIT).await;
            kit.sources.adopt_plugins(&kit.plugins);
            self::to_ask(&kit, &work, &about, now, &doubted)
        } else {
            to_ask
        };
        let mut doubted = doubted;
        if recheck_older {
            let more = recheck(&kit, &work, &about, now).await;
            if !more.is_empty() {
                for pair in more {
                    if !doubted.contains(&pair) {
                        doubted.push(pair);
                    }
                }
                to_ask = self::to_ask(&kit, &work, &about, now, &doubted);
            }
        }
        if to_ask.is_empty() {
            let wanted = searchable(&kit, &work, medium);
            send_for_work(&kit, &work, false, kept_misses(&kit, &work, &wanted, now), &tx);
        }
        let mut asking: FuturesUnordered<_> = to_ask
            .iter()
            .map(|source| {
                let (kit, work, about) = (kit.clone(), work.clone(), &about);
                async move { (source.clone(), ask_one(&kit, source, &work, about, now).await) }
            })
            .collect();
        while let Some((source, answer)) = asking.next().await {
            if let Some((_, series)) = doubted.iter().find(|(s, _)| *s == source) {
                let_go(&kit, &source, series, &answer);
            }
            let miss = |reason, possible, query| EditionMiss {
                source_name: source_name(&kit, &source),
                source_id: source.clone(),
                reason,
                possible,
                query,
            };
            let miss = match answer {
                Answer::Found(_) => None,
                Answer::Possible { choice, query } => Some(miss(
                    Msg::Reading(ReadingMsg::ReadingMissPossible),
                    Some(choice),
                    Some(query),
                )),
                Answer::Missed { reason, query } => Some(miss(reason, None, query)),
            };
            keep_miss(&kit, &work, miss, &source, now);
            let wanted = searchable(&kit, &work, medium);
            let missing = kept_misses(&kit, &work, &wanted, now);
            send_for_work(&kit, &work, !asking.is_empty(), missing, &tx);
        }
        settled(&kit, ask, &work, &tx);
        kit.sources
            .catalog
            .checks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(work.as_str());
        // A page waiting on the check asks for its own chapters once it ends.
        let waited = ended.receiver_count() > 0;
        // Its waiters hear the check end as the sender goes.
        drop(ended);
        // A series that had nowhere to be read from now has, or a doubted match gave way to
        // the series its source found: its page fills in again.
        let after = kit.sources.edition_of(&work, None);
        let moved = match &before {
            None => after.is_some(),
            Some(_) => !doubted.is_empty() && after != before,
        };
        if !waited && moved {
            series::load(&kit, ask, work, &tx);
        }
    });
}

/// The sources changed while `work`'s page may be open: the page again, and Read from asking
/// the sources that have not answered for the work as they stand. A page read from an edition
/// moves to a better one found meanwhile; a page with none waits on the search, as on opening.
pub(crate) fn again(kit: &Kit, ask: Ask, work: WorkId, tx: &Outbox) {
    let before = kit.sources.edition_of(&work, None);
    series::load(kit, ask, work.clone(), tx);
    load(kit, ask, work.clone(), tx);
    if before.is_none() {
        return;
    }
    let Some(check) = running(kit, &work) else { return };
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        ended(check).await;
        if kit.sources.edition_of(&work, None) != before {
            series::load(&kit, ask, work, &tx);
        }
    });
}

/// The editions whose chapter lists were never asked for, or not lately, so Read from can say
/// what each holds: asked in the background, each line updated as its answer comes.
fn list_unlisted(kit: &Kit, work: &WorkId, now: i64, tx: &Outbox) {
    let due = unlisted(kit, work, now);
    if due.is_empty() {
        return;
    }
    let (kit, work, tx) = (kit.clone(), work.clone(), tx.clone());
    tokio::spawn(async move { list_editions(&kit, &work, due, now, &tx).await });
}

/// The editions of `work` with no list kept and none asked for within [`ASKED_FOR`].
fn unlisted(kit: &Kit, work: &WorkId, now: i64) -> Vec<String> {
    let refs = kit.store.book_refs_for_work(work.as_str()).unwrap_or_default();
    let mut seen = HashSet::new();
    refs.into_iter()
        .filter(|(source, _)| seen.insert(source.clone()))
        .filter(|(source, _)| kit.sources.registry.manifest(source).is_some())
        .filter(|(source, series)| {
            chapters::kept(&kit.store, source, series).is_none_or(|(l, _)| l.is_empty())
                && kit
                    .sources
                    .listing_asked_at(source, series)
                    .is_none_or(|at| now - at >= ASKED_FOR)
        })
        .map(|(source, _)| source)
        .collect()
}

/// Ask each of `sources` for its list of `work`, sending Read from's lines as they change.
async fn list_editions(kit: &Kit, work: &WorkId, sources: Vec<String>, now: i64, tx: &Outbox) {
    // Each line says it is being asked before any answer comes.
    for source in &sources {
        if let Some((source, series)) = kit.sources.edition_of(work, Some(source.as_str())) {
            kit.sources.note_listing(&source, &series, Some(Asked::Asking), now);
        }
    }
    let wanted = searchable(kit, work, medium_of(kit, work));
    let send = || {
        let missing = kept_misses(kit, work, &wanted, now);
        send_for_work(kit, work, running(kit, work).is_some(), missing, tx);
    };
    send();
    let mut asking: FuturesUnordered<_> = sources
        .iter()
        .map(|source| kit.sources.chapters(work, Some(source.as_str()), now))
        .collect();
    while asking.next().await.is_some() {
        send();
    }
}

/// The edition in use answered with no chapters: ask the work's other editions for theirs,
/// unless asked lately, so the edition picked can move to one that has chapters and Read from
/// says what each holds. True when the edition in use changed meanwhile.
pub(crate) async fn list_others(kit: &Kit, work: &WorkId, now: i64, tx: &Outbox) -> bool {
    let before = kit.sources.edition_of(work, None);
    let in_use = before.as_ref().map(|(source, _)| source.clone());
    let due: Vec<String> =
        unlisted(kit, work, now).into_iter().filter(|s| Some(s) != in_use.as_ref()).collect();
    if !due.is_empty() {
        list_editions(kit, work, due, now, tx).await;
    }
    kit.sources.edition_of(work, None) != before
}

/// The check of a series' sources is done: a series none of them has is kept as not found, so
/// its Home tile says so for the rest of its row's 30 minutes, and Home is sent again when that
/// changed. Only an AniList entry can be on a Home row.
fn settled(kit: &Kit, ask: Ask, work: &WorkId, tx: &Outbox) {
    if tiles::anilist(work.as_str()).is_none() {
        return;
    }
    let missing = kit.sources.edition_of(work, None).is_none();
    let changed = {
        let mut kept = kit.sources.catalog.not_found.lock().unwrap_or_else(|e| e.into_inner());
        if missing {
            let mark = kit.sources.sources_mark();
            kept.insert(work.as_str().to_owned(), (ask.now, mark)).is_none()
        } else {
            kept.remove(work.as_str()).is_some()
        }
    };
    if changed {
        super::home::resend(kit, ask, tx);
    }
}

/// How sure Search there is that a series is the work, 0 to 100: the same name first, then the
/// name spelt another way or missing its subtitle, then another edition of it, then the rest by
/// the words they share.
fn confidence(work: &fit::Work, hit: &SeriesHit) -> u8 {
    match fit::hit_title(work, hit) {
        Title::Same => 100,
        Title::Spelt => 90,
        Title::Subtitle => 80,
        Title::Edition(_) => 70,
        Title::Other => {
            let theirs = std::iter::once(&hit.title).chain(&hit.synonyms);
            let close = theirs
                .flat_map(|t| work.names.iter().map(move |n| kaeri_meta::similarity(n, t)))
                .fold(0.0, f64::max);
            (close * 60.0).round().clamp(0.0, 60.0) as u8
        }
    }
}

/// "Search there": one source's series for words the person typed, the work itself first, then
/// other editions of it, then the rest by how close they are.
pub(crate) fn search_there(
    kit: &Kit,
    ask: Ask,
    work: WorkId,
    source: String,
    query: String,
    tx: &Outbox,
) {
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        // At launch the source's plugin may not be in yet: it is waited for, not told it
        // has nothing.
        kit.sources.plugins_in(&kit.plugins, Some(&source)).await;
        kit.sources.adopt_plugins(&kit.plugins);
        let about = about(&kit, &work);
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
        let (hits, failure) = match asked {
            Ok(hits) => (hits, None),
            Err(ProviderError::NotFound) => (Vec::new(), None),
            Err(error) => (Vec::new(), Some(kit.sources.failure(&source, &error))),
        };
        // Closeness to the series' own names, not to what was typed.
        let mut choices: Vec<SourceSeriesChoice> = hits
            .into_iter()
            .map(|hit| {
                let sure = confidence(&about, &hit);
                choice(hit, sure)
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
    let wanted = searchable(kit, &work, medium_of(kit, &work));
    send_for_work(kit, &work, false, kept_misses(kit, &work, &wanted, ask.now), tx);
    series::load(kit, ask, work, tx);
}

fn medium_of(kit: &Kit, work: &WorkId) -> Medium {
    match tiles::anilist(work.as_str()).and_then(|id| kit.meta.kept_book(id)) {
        Some(kept) => kept.book.medium(),
        None => series::local_medium(kit, work),
    }
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

    /// What a fake source finds for the words it is asked.
    type Finds = Box<dyn Fn(&str) -> Result<Vec<SeriesHit>, ProviderError> + Send + Sync>;

    /// A source that answers searches by its `finds` and counts the asks, with chapters and a
    /// page for the series it is given them for.
    struct Stock {
        manifest: BookSourceManifest,
        finds: Finds,
        chapters: std::sync::Mutex<HashMap<String, Vec<SourceChapter>>>,
        names: std::sync::Mutex<HashMap<String, String>>,
        authors: Vec<String>,
        asked: AtomicU32,
        queries: std::sync::Mutex<Vec<String>>,
    }

    impl Stock {
        /// A source that answers every search the same way.
        fn new(
            id: &str,
            media: &[Medium],
            answer: Result<Vec<SeriesHit>, ProviderError>,
        ) -> Arc<Self> {
            Self::finding(id, media, Box::new(move |_| answer.clone()), &[])
        }

        fn finding(id: &str, media: &[Medium], finds: Finds, authors: &[&str]) -> Arc<Self> {
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
            Arc::new(Self {
                manifest,
                finds,
                chapters: std::sync::Mutex::default(),
                names: std::sync::Mutex::default(),
                authors: authors.iter().map(|a| (*a).to_owned()).collect(),
                asked: AtomicU32::new(0),
                queries: std::sync::Mutex::default(),
            })
        }

        /// The series lists `count` numbered chapters, or one named `label` with no number.
        fn listing(
            self: &Arc<Self>,
            series: &str,
            count: u32,
            label: Option<&str>,
        ) -> Arc<Self> {
            let list = match label {
                Some(label) => vec![SourceChapter::new(format!("{series}-0"), label)],
                None => (1..=count)
                    .map(|n| SourceChapter::new(format!("{series}-{n}"), n.to_string()))
                    .collect(),
            };
            self.chapters.lock().unwrap().insert(series.to_owned(), list);
            self.clone()
        }

        /// The series is called `title` on its page.
        fn naming(self: &Arc<Self>, series: &str, title: &str) -> Arc<Self> {
            self.names.lock().unwrap().insert(series.to_owned(), title.to_owned());
            self.clone()
        }

        fn asks(&self) -> u32 {
            self.asked.load(Ordering::Relaxed)
        }

        fn queries(&self) -> Vec<String> {
            self.queries.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl BookSource for Stock {
        fn manifest(&self) -> &BookSourceManifest {
            &self.manifest
        }

        async fn search(
            &self,
            query: &str,
            _: &BookHints,
        ) -> Result<Vec<SeriesHit>, ProviderError> {
            self.asked.fetch_add(1, Ordering::Relaxed);
            self.queries.lock().unwrap().push(query.to_owned());
            (self.finds)(query)
        }

        async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
            let name = self.names.lock().unwrap().get(id).cloned();
            let mut detail =
                SeriesDetail::new(id, name.as_deref().unwrap_or("The Lantern Ferry"));
            detail.authors = self.authors.clone();
            Ok(detail)
        }

        async fn chapters(
            &self,
            id: &str,
            _: &BookHints,
        ) -> Result<Vec<SourceChapter>, ProviderError> {
            Ok(self.chapters.lock().unwrap().get(id).cloned().unwrap_or_default())
        }

        async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
            Err(ProviderError::NotFound)
        }
    }

    fn hits(titles: &[(&str, &str)]) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(titles.iter().map(|(id, title)| SeriesHit::new(*id, *title)).collect())
    }

    fn hit(id: &str, title: &str, year: Option<u32>) -> SeriesHit {
        let mut hit = SeriesHit::new(id, title);
        hit.year = year;
        hit
    }

    /// The work every test asks about: "The Lantern Ferry".
    const LANTERN: &str = r#"{"book":{"id":900301,"title":{"romaji":"The Lantern Ferry","english":"Lantern Ferry"},"synonyms":["Ferry of Lanterns"]}}"#;

    /// A finished work with a romaji name, a year, 120 chapters and a maker.
    const COUNCIL: &str = r#"{"book":{"id":900301,"format":"MANGA","title":{"romaji":"Hoshizora-tachi no Ren'ai Kaigi: Yoru no Bu","english":"Starlit Council: Beyond the Night Hours"},"chapters":120,"startDate":{"year":2015},"staff":{"edges":[{"role":"Story & Art","node":{"name":{"full":"Mira Tamsin"}}},{"role":"Translator (English)","node":{"name":{"full":"Oren Vale"}}}]}}}"#;

    fn kit(dir: &std::path::Path, sources: &[&Arc<Stock>]) -> Kit {
        kit_of(dir, sources, LANTERN)
    }

    fn kit_of(dir: &std::path::Path, sources: &[&Arc<Stock>], meta_row: &str) -> Kit {
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
        let work = BookWork {
            work: WORK.into(),
            anilist_id: Some(900_301),
            medium: "manga".into(),
            titles: "{}".into(),
            meta: Some(meta_row.into()),
            chapters_total:
                serde_json::from_str::<serde_json::Value>(meta_row).unwrap()["book"]["chapters"]
                    .as_u64()
                    .map(|n| n as u32),
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
    ) -> Vec<(Vec<String>, bool, Vec<EditionMiss>)> {
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

    fn reasons(missing: &[EditionMiss]) -> Vec<(&str, &str)> {
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
        assert_eq!(
            attic.queries(),
            ["Lantern Ferry", "The Lantern Ferry", "Lantern", "Ferry of Lanterns"],
            "the name shown, the romaji, the key word they share, then the other names"
        );

        // Opened again a minute later: the answers stand and nobody is asked.
        load(&kit, ask(NOW + 60), work.clone(), &out);
        let again = answers(&mut rx).await;
        assert_eq!(again.len(), 1);
        assert_eq!(reasons(&again[0].2).len(), 2);
        assert_eq!((attic.asks(), cellar.asks(), harbor.asks()), (4, 1, 1));

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
        assert_eq!((attic.asks(), loft.asks()), (4, 4));
        load(&kit, ask(NOW + ASKED_FOR + 60), work, &out);
        let _ = answers(&mut rx).await;
        assert_eq!(attic.asks(), 8, "asked again once its answer is old");
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
        // A source not reached is not answering; the device is offline only when the network
        // says so.
        let down =
            kaeri_model::reading::ReadingFailure::SourceDown { source: "cellar shelf".into() };
        assert_eq!(failure, Some(down));
        kit.sources.write_back_network(true, NOW);
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

    fn finds(pairs: Vec<(&'static str, Vec<SeriesHit>)>) -> Finds {
        Box::new(move |query| {
            Ok(pairs.iter().filter(|(q, _)| *q == query).flat_map(|(_, h)| h.clone()).collect())
        })
    }

    #[tokio::test]
    async fn a_romaji_spelt_another_way_is_matched_when_the_year_or_maker_backs_it() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let romaji = "Hoshizora-tachi no Ren'ai Kaigi: Yoru no Bu";
        let spelt = "Hoshizoratachi no Renai Kaigi - Yoru no Bu";
        // Spelt another way, and from the year the work started.
        let slate = Stock::finding(
            "slate",
            &manga,
            finds(vec![(romaji, vec![hit("s-1", spelt, Some(2015))])]),
            &[],
        )
        .listing("s-1", 118, None);
        // Spelt another way, with no year, by the work's maker written surname first.
        let marl = Stock::finding(
            "marl",
            &manga,
            finds(vec![(romaji, vec![hit("m-1", spelt, None)])]),
            &["TAMSIN Mira"],
        )
        .listing("m-1", 120, None);
        // Spelt another way, nine years before the work: another work.
        let shale = Stock::finding(
            "shale",
            &manga,
            finds(vec![(romaji, vec![hit("h-1", spelt, Some(2006))])]),
            &[],
        )
        .listing("h-1", 40, None);
        // Spelt another way, and nothing says it is the work.
        let chalk = Stock::finding(
            "chalk",
            &manga,
            finds(vec![(romaji, vec![hit("c-1", spelt, None)])]),
            &[],
        )
        .listing("c-1", 120, None);
        let kit = kit_of(dir.path(), &[&slate, &marl, &shale, &chalk], COUNCIL);
        let (tx, mut rx) = mpsc::unbounded_channel();
        load(&kit, ask(NOW), WorkId::new(WORK), &Outbox::from(tx));
        let last = answers(&mut rx).await.pop().unwrap();

        let matched =
            |source: &str, series: &str| kit.store.book_match_state(source, series).unwrap();
        assert_eq!(matched("slate", "s-1"), Some((WORK.into(), false)), "the year backs it");
        assert_eq!(matched("marl", "m-1"), Some((WORK.into(), false)), "the maker backs it");
        assert_eq!(matched("shale", "h-1"), None, "its year says it is another work");
        assert_eq!(matched("chalk", "c-1"), None, "nothing backs a name spelt another way");
        assert_eq!(
            reasons(&last.2),
            [("chalk", "reading-miss-no-series"), ("shale", "reading-miss-no-series")]
        );
        assert_eq!(
            last.2[0].query.as_deref(),
            Some(romaji),
            "Search there starts from the words that came closest"
        );
        assert!(slate.queries().contains(&romaji.to_owned()), "the romaji is always searched");
    }

    #[tokio::test]
    async fn the_romaji_a_key_word_or_plain_words_find_what_the_english_name_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let romaji = "Hoshizora-tachi no Ren'ai Kaigi: Yoru no Bu";
        let found = |id: &'static str| vec![hit(id, romaji, Some(2015))];
        // Finds the series only by its romaji.
        let reed = Stock::finding("reed", &manga, finds(vec![(romaji, found("r-1"))]), &[])
            .listing("r-1", 120, None);
        // Finds it only by one word.
        let moss =
            Stock::finding("moss", &manga, finds(vec![("Hoshizora", found("m-1"))]), &[])
                .listing("m-1", 120, None);
        // Finds nothing once there is punctuation in the words, and finds it by the words alone.
        let flint = Stock::finding(
            "flint",
            &manga,
            Box::new(move |q| {
                let plain = q.chars().all(|c| c.is_alphanumeric() || c == ' ');
                Ok(if plain && q.starts_with("Starlit") {
                    vec![hit("f-1", "Starlit Council - Beyond the Night Hours", None)]
                } else {
                    Vec::new()
                })
            }),
            &[],
        )
        .listing("f-1", 120, None);
        let kit = kit_of(dir.path(), &[&reed, &moss, &flint], COUNCIL);
        let (tx, mut rx) = mpsc::unbounded_channel();
        load(&kit, ask(NOW), WorkId::new(WORK), &Outbox::from(tx));
        let last = answers(&mut rx).await.pop().unwrap();

        assert_eq!(last.0, ["flint", "moss", "reed"], "found on each");
        assert!(last.2.is_empty());
        assert!(moss.asks() <= 6, "six searches at most");
        assert!(moss.queries().contains(&"Hoshizora".to_owned()), "{:?}", moss.queries());
        assert!(
            flint.queries().contains(&"Starlit Council Beyond the Night Hours".to_owned()),
            "{:?}",
            flint.queries()
        );
    }

    #[tokio::test]
    async fn another_edition_is_a_possible_match_and_never_matched() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let english = "Starlit Council: Beyond the Night Hours";
        let colour = Stock::new(
            "colour",
            &manga,
            Ok(vec![
                hit("c-9", "Paper Cranes", None),
                hit("c-1", "Starlit Council: Beyond the Night Hours - Full Color", Some(2015)),
            ]),
        )
        .listing("c-1", 28, None);
        let kit = kit_of(dir.path(), &[&colour], COUNCIL);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let out = Outbox::from(tx);
        load(&kit, ask(NOW), WorkId::new(WORK), &out);
        let last = answers(&mut rx).await.pop().unwrap();

        assert!(last.0.is_empty(), "no edition");
        assert_eq!(reasons(&last.2), [("colour", "reading-miss-possible")]);
        let possible = last.2[0].possible.as_ref().expect("offered");
        assert_eq!(possible.series_ref, "c-1");
        assert_eq!(kit.store.book_match_state("colour", "c-1").unwrap(), None, "never matched");
        assert!(last.2[0].query.is_some());

        // Search there puts the work's own name before its other edition and the rest.
        search_there(&kit, ask(NOW), WorkId::new(WORK), "colour".into(), english.into(), &out);
        let choices = loop {
            if let Some(Event::BookSources(BookSourceEvent::SourceSeries { choices, .. })) =
                rx.recv().await
            {
                break choices;
            }
        };
        let order: Vec<&str> = choices.iter().map(|c| c.series_ref.as_str()).collect();
        assert_eq!(order, ["c-1", "c-9"]);

        // One tap confirms it.
        choose(&kit, ask(NOW), WorkId::new(WORK), "colour".into(), "c-1".into(), &out);
        let (editions, _, missing) = answers(&mut rx).await.remove(0);
        assert_eq!(editions, ["colour"]);
        assert!(missing.is_empty());
    }

    #[tokio::test]
    async fn a_one_shot_beside_the_series_is_never_matched_and_a_kept_one_is_let_go() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let english = "Starlit Council: Beyond the Night Hours";
        let both = || Ok(vec![hit("o-1", english, None), hit("s-1", english, None)]);
        // The pilot one-shot first, then the series, under one name.
        let kiln = Stock::new("kiln", &manga, both())
            .listing("o-1", 1, Some("Oneshot"))
            .listing("s-1", 120, None);
        // Only the one-shot.
        let tile = Stock::new("tile", &manga, Ok(vec![hit("t-1", english, None)])).listing(
            "t-1",
            1,
            Some("Oneshot"),
        );
        // Matched to its one-shot before, by itself; and one the person chose stays.
        let brick = Stock::new("brick", &manga, both())
            .listing("o-1", 1, Some("Oneshot"))
            .listing("s-1", 120, None);
        let clay = Stock::new("clay", &manga, both())
            .listing("o-1", 1, Some("Pilot"))
            .listing("s-1", 120, None);
        let kit = kit_of(dir.path(), &[&kiln, &tile, &brick, &clay], COUNCIL);
        kit.store.set_book_match("brick", "o-1", WORK, 90, false, false, NOW).unwrap();
        kit.store.set_book_match("clay", "o-1", WORK, 100, true, true, NOW).unwrap();
        let oneshot = [SourceChapter::new("o-1-0", "Oneshot")];
        chapters::keep(&kit.store, "brick", "o-1", &oneshot, NOW);
        chapters::keep(&kit.store, "clay", "o-1", &oneshot, NOW);
        let (tx, mut rx) = mpsc::unbounded_channel();
        load(&kit, ask(NOW), WorkId::new(WORK), &Outbox::from(tx));
        let last = answers(&mut rx).await.pop().unwrap();

        let matched =
            |source: &str, series: &str| kit.store.book_match_state(source, series).unwrap();
        assert_eq!(matched("kiln", "s-1"), Some((WORK.into(), false)), "the series");
        assert_eq!(matched("kiln", "o-1"), None, "never the one-shot");
        assert_eq!(matched("tile", "t-1"), None);
        assert_eq!(reasons(&last.2), [("tile", "reading-miss-no-series")]);
        assert_eq!(matched("brick", "o-1"), None, "let go");
        assert_eq!(matched("brick", "s-1"), Some((WORK.into(), false)), "and the series found");
        assert_eq!(
            matched("clay", "o-1"),
            Some((WORK.into(), true)),
            "the person's choice stays"
        );
        assert_eq!(clay.asks(), 0);
    }

    /// The edition the person picked stays the one read from while its source is searched again
    /// for a one-shot it matched by itself, and moves to the series that search finds.
    #[tokio::test]
    async fn a_picked_edition_stays_in_use_while_its_one_shot_is_searched_again() {
        picked_edition_while_searched_again(false).await;
    }

    /// The same for an older match that its series' name shows is another edition of the work.
    #[tokio::test]
    async fn a_picked_edition_stays_in_use_while_its_older_match_is_searched_again() {
        picked_edition_while_searched_again(true).await;
    }

    async fn picked_edition_while_searched_again(older_by_name: bool) {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let english = "Starlit Council: Beyond the Night Hours";
        let brick = Stock::new(
            "brick",
            &manga,
            Ok(vec![hit("o-1", english, None), hit("s-1", english, None)]),
        )
        .listing("o-1", 1, Some("Oneshot"))
        .listing("s-1", 120, None);
        if older_by_name {
            brick.naming("o-1", "Starlit Council: Beyond the Night Hours (Official Colored)");
        }
        let kiln = Stock::new("kiln", &manga, Ok(Vec::new()));
        let kit = kit_of(dir.path(), &[&brick, &kiln], COUNCIL);
        kit.store.set_book_match("brick", "o-1", WORK, OLDER, false, false, NOW).unwrap();
        if !older_by_name {
            let oneshot = [SourceChapter::new("o-1-0", "Oneshot")];
            chapters::keep(&kit.store, "brick", "o-1", &oneshot, NOW);
        }
        kit.store.set_book_match("kiln", "k-1", WORK, MATCHED, false, false, NOW).unwrap();
        let list: Vec<SourceChapter> =
            (1..=60).map(|i| SourceChapter::new(format!("k{i}"), i.to_string())).collect();
        chapters::keep(&kit.store, "kiln", "k-1", &list, NOW);
        let pref = kaeri_store::reading::EditionPref {
            source_id: "brick".into(),
            version_label: None,
            language: None,
        };
        kit.store.set_edition_pref(WORK, Some(&pref)).unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        load(&kit, ask(NOW), WorkId::new(WORK), &Outbox::from(tx));
        let mut in_use = Vec::new();
        loop {
            let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("Read from answers")
                .expect("the outbox is open");
            if let Event::BookSources(BookSourceEvent::ForWork { editions, asking, .. }) = event
            {
                in_use.push(editions.iter().find(|e| e.in_use).map(|e| e.source_id.clone()));
                if !asking {
                    break;
                }
            }
        }
        assert!(in_use.iter().all(|s| s.as_deref() == Some("brick")), "{in_use:?}");
        let work = WorkId::new(WORK);
        assert_eq!(kit.sources.edition_of(&work, None), Some(("brick".into(), "s-1".into())));
        assert_eq!(kit.store.book_match_state("brick", "o-1").unwrap(), None, "let go");
    }

    #[tokio::test]
    async fn a_series_tied_to_another_work_stays_there() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let harbor = Stock::new("harbor", &manga, hits(&[("h-1", "The Lantern Ferry")]));
        let kit = kit(dir.path(), &[&harbor]);
        kit.store.set_book_match("harbor", "h-1", "a:900999", 100, true, false, NOW).unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        load(&kit, ask(NOW), WorkId::new(WORK), &Outbox::from(tx));
        let last = answers(&mut rx).await.pop().unwrap();
        assert!(last.0.is_empty());
        assert_eq!(
            kit.store.book_match_state("harbor", "h-1").unwrap(),
            Some(("a:900999".into(), true))
        );
    }

    #[test]
    fn the_edition_read_from_is_the_one_nearest_the_works_chapters() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let first = Stock::new("first", &manga, Ok(Vec::new()));
        let second = Stock::new("second", &manga, Ok(Vec::new()));
        let third = Stock::new("third", &manga, Ok(Vec::new()));
        let kit = kit_of(dir.path(), &[&first, &second, &third], COUNCIL);
        kit.sources.registry.set_order(vec!["first".into(), "second".into(), "third".into()]);
        let list = |n: u32| -> Vec<SourceChapter> {
            (1..=n).map(|i| SourceChapter::new(format!("c{i}"), i.to_string())).collect()
        };
        let work = WorkId::new(WORK);
        for (source, series, n) in
            [("first", "f-1", 1), ("second", "s-1", 30), ("third", "t-1", 119)]
        {
            kit.store.set_book_match(source, series, WORK, 90, false, false, NOW).unwrap();
            chapters::keep(&kit.store, source, series, &list(n), NOW);
        }
        assert_eq!(kit.sources.edition_of(&work, None), Some(("third".into(), "t-1".into())));
        // Two series on one source: the one with the chapters.
        kit.store.set_book_match("third", "t-0", WORK, 95, false, false, NOW).unwrap();
        chapters::keep(&kit.store, "third", "t-0", &list(1), NOW);
        assert_eq!(kit.sources.edition_of(&work, None), Some(("third".into(), "t-1".into())));
        // The person's choice comes first.
        kit.store.set_book_match("second", "s-1", WORK, 100, true, false, NOW).unwrap();
        assert_eq!(kit.sources.edition_of(&work, None), Some(("second".into(), "s-1".into())));
        assert_eq!(
            kit.sources.edition_of(&work, Some("first")),
            Some(("first".into(), "f-1".into()))
        );
    }

    #[tokio::test]
    async fn an_older_match_to_another_edition_is_let_go_and_its_source_asked_again() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let series = vec![hit("p-2", "The Lantern Ferry", None)];
        // Matched by itself before the name rules to the colour release.
        let pier =
            Stock::finding("pier", &manga, finds(vec![("The Lantern Ferry", series)]), &[])
                .naming("p-1", "The Lantern Ferry (Official Colored)")
                .listing("p-1", 30, None)
                .listing("p-2", 40, None);
        // Matched before the rules to the series itself.
        let quay = Stock::new("quay", &manga, Ok(Vec::new())).listing("q-1", 40, None);
        let kit = kit(dir.path(), &[&pier, &quay]);
        for (source, series) in [("pier", "p-1"), ("quay", "q-1")] {
            kit.store.set_book_match(source, series, WORK, 90, false, false, NOW).unwrap();
        }
        // One the person chose stays whatever its name.
        kit.store.set_book_match("quay", "q-9", WORK, 90, true, false, NOW).unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        load(&kit, ask(NOW), WorkId::new(WORK), &Outbox::from(tx));
        let last = answers(&mut rx).await.pop().unwrap();

        let state = |source: &str, series: &str| {
            kit.store.book_match_confidence(source, series).unwrap()
        };
        assert_eq!(state("pier", "p-1"), None, "the colour release is let go");
        assert_eq!(
            state("pier", "p-2"),
            Some((MATCHED, false)),
            "and the series found in its place"
        );
        assert_eq!(state("quay", "q-1"), Some((MATCHED, false)), "checked once, and kept");
        assert_eq!(state("quay", "q-9"), Some((90, true)));
        assert!(last.2.is_empty());
        assert_eq!(quay.asks(), 0);

        // Opened again: nothing is checked or asked a second time.
        let asked = pier.asks();
        let (tx, mut rx) = mpsc::unbounded_channel();
        load(&kit, ask(NOW + 60), WorkId::new(WORK), &Outbox::from(tx));
        let _ = answers(&mut rx).await;
        assert_eq!(pier.asks(), asked);
    }

    #[test]
    fn the_edition_read_from_has_the_first_chapters() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let later = Stock::new("later", &manga, Ok(Vec::new()));
        let start = Stock::new("start", &manga, Ok(Vec::new()));
        // A work whose chapter count AniList does not know yet.
        let kit = kit(dir.path(), &[&later, &start]);
        kit.sources.registry.set_order(vec!["later".into(), "start".into()]);
        let list = |from: u32, to: u32| -> Vec<SourceChapter> {
            (from..=to).map(|i| SourceChapter::new(format!("c{i}"), i.to_string())).collect()
        };
        let work = WorkId::new(WORK);
        // First in the order, and the most chapters, but from chapter 47 on.
        kit.store.set_book_match("later", "l-1", WORK, 90, false, false, NOW).unwrap();
        chapters::keep(&kit.store, "later", "l-1", &list(47, 100), NOW);
        kit.store.set_book_match("start", "s-1", WORK, 90, false, false, NOW).unwrap();
        chapters::keep(&kit.store, "start", "s-1", &list(1, 50), NOW);
        assert_eq!(kit.sources.edition_of(&work, None), Some(("start".into(), "s-1".into())));
    }

    #[test]
    fn extras_swell_a_count_but_not_how_far_an_edition_reaches() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let side = Stock::new("side", &manga, Ok(Vec::new()));
        let whole = Stock::new("whole", &manga, Ok(Vec::new()));
        let kit = kit(dir.path(), &[&side, &whole]);
        kit.sources.registry.set_order(vec!["side".into(), "whole".into()]);
        let list = |to: u32, extras: u32| -> Vec<SourceChapter> {
            (1..=to)
                .map(|i| SourceChapter::new(format!("c{i}"), i.to_string()))
                .chain(
                    (1..=extras)
                        .map(|i| SourceChapter::new(format!("x{i}"), format!("Extra {i}"))),
                )
                .collect()
        };
        let work = WorkId::new(WORK);
        // First in the order and listing more, but its chapters stop at 28 under a pile of extras.
        kit.store.set_book_match("side", "d-1", WORK, 90, false, false, NOW).unwrap();
        chapters::keep(&kit.store, "side", "d-1", &list(28, 40), NOW);
        kit.store.set_book_match("whole", "w-1", WORK, 90, false, false, NOW).unwrap();
        chapters::keep(&kit.store, "whole", "w-1", &list(51, 13), NOW);
        assert_eq!(kit.sources.edition_of(&work, None), Some(("whole".into(), "w-1".into())));
    }

    #[test]
    fn a_source_switched_off_is_never_read_from_unless_chosen() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let dim = Stock::new("dim", &manga, Ok(Vec::new()));
        let lit = Stock::new("lit", &manga, Ok(Vec::new()));
        let kit = kit(dir.path(), &[&dim, &lit]);
        kit.sources.registry.set_order(vec!["dim".into(), "lit".into()]);
        let list = |to: u32| -> Vec<SourceChapter> {
            (1..=to).map(|i| SourceChapter::new(format!("c{i}"), i.to_string())).collect()
        };
        let work = WorkId::new(WORK);
        // First in the order and the fuller list, but switched off.
        kit.store.set_book_match("dim", "d-1", WORK, 90, false, false, NOW).unwrap();
        chapters::keep(&kit.store, "dim", "d-1", &list(80), NOW);
        kit.sources.registry.set_off(["dim".to_owned()]);
        assert_eq!(kit.sources.edition_of(&work, None), None, "nothing else is held");
        kit.store.set_book_match("lit", "l-1", WORK, 90, false, false, NOW).unwrap();
        chapters::keep(&kit.store, "lit", "l-1", &list(20), NOW);
        assert_eq!(kit.sources.edition_of(&work, None), Some(("lit".into(), "l-1".into())));
        // Asked for by name, it is still the one read from.
        assert_eq!(
            kit.sources.edition_of(&work, Some("dim")),
            Some(("dim".into(), "d-1".into()))
        );
    }

    #[tokio::test]
    async fn a_source_whose_list_came_back_empty_is_never_the_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let manga = [Medium::Manga];
        let bare = Stock::new("bare", &manga, Ok(Vec::new()));
        let full = Stock::new("full", &manga, Ok(Vec::new())).listing("f-1", 12, None);
        let kit = kit(dir.path(), &[&bare, &full]);
        kit.sources.registry.set_order(vec!["bare".into(), "full".into()]);
        let work = WorkId::new(WORK);
        // Its only match, and the list it gives is empty.
        kit.store.set_book_match("bare", "b-1", WORK, 90, false, false, NOW).unwrap();
        let (_, groups) = kit.sources.chapters(&work, None, NOW).await.unwrap();
        assert!(groups.is_empty());
        assert_eq!(kit.sources.edition_of(&work, None), None, "nothing else is held");
        // First in the order, against one whose list is not known yet.
        kit.store.set_book_match("full", "f-1", WORK, 90, false, false, NOW).unwrap();
        assert_eq!(kit.sources.edition_of(&work, None), Some(("full".into(), "f-1".into())));
        // Asked for by name, it is still the one read from.
        assert_eq!(
            kit.sources.edition_of(&work, Some("bare")),
            Some(("bare".into(), "b-1".into()))
        );
    }

    /// The person's pick on a source not installed, or switched off, while another source
    /// lists the work: the work reads from that one, and the pick is kept for the source's
    /// return.
    #[tokio::test]
    async fn a_picked_edition_whose_source_is_gone_or_off_gives_way_until_it_is_back() {
        for off in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let manga = [Medium::Manga];
            let brick = Stock::new("brick", &manga, Ok(Vec::new())).listing("s-1", 30, None);
            let kiln = Stock::new("kiln", &manga, Ok(Vec::new())).listing("k-1", 30, None);
            let sources: Vec<&Arc<Stock>> = if off { vec![&brick, &kiln] } else { vec![&kiln] };
            let kit = kit_of(dir.path(), &sources, COUNCIL);
            if off {
                kit.sources.registry.set_off(["brick".to_owned()]);
            }
            kit.store.set_book_match("brick", "s-1", WORK, MATCHED, true, false, NOW).unwrap();
            kit.store.set_book_match("kiln", "k-1", WORK, MATCHED, true, false, NOW).unwrap();
            let pref = kaeri_store::reading::EditionPref {
                source_id: "brick".into(),
                version_label: None,
                language: None,
            };
            kit.store.set_edition_pref(WORK, Some(&pref)).unwrap();
            let work = WorkId::new(WORK);
            assert_eq!(
                kit.sources.edition_of(&work, None),
                Some(("kiln".into(), "k-1".into())),
                "off={off}"
            );
            let (row, groups) = kit.sources.chapters(&work, None, NOW).await.unwrap();
            assert_eq!(row.map(|r| r.source_id).as_deref(), Some("kiln"));
            assert_eq!(groups.iter().map(|g| g.chapters.len()).sum::<usize>(), 30);
            let kept = kit.store.edition_pref(WORK).unwrap().map(|p| p.source_id);
            assert_eq!(kept.as_deref(), Some("brick"), "the pick is kept");
            if off {
                kit.sources.registry.set_off(Vec::<String>::new());
            } else {
                kit.sources.registry.put(brick.clone());
            }
            assert_eq!(
                kit.sources.edition_of(&work, None),
                Some(("brick".into(), "s-1".into())),
                "back (off={off})"
            );
        }
    }
}
