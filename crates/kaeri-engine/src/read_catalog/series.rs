//! A series' page: the header from what was kept at once, then AniList's page and the
//! edition's chapters as they land, and each chapter's versions across the person's sources.

use std::collections::{HashMap, HashSet};

use kaeri_core::{
    books::{ChapterId, WorkId},
    media::Medium,
};
use kaeri_meta::{KeptBook, anilist::books::Book};
use kaeri_model::{
    Msg, ReadingMsg,
    reading::{
        BookEditionRow, ChapterReadState, ChapterRow, ChapterVersion, CrossMediumRow,
        LibrarySegmentRead, ReadingEvent, ReadingFailure, SentChapters, SeriesView,
        VolumeGroup, WorkTitles,
    },
};

use super::{Ask, Kit, chapters, editions, send, tiles};
use crate::outbox::Outbox;

/// The page: the header at once, then AniList's page and the chapters.
pub fn load(kit: &Kit, ask: Ask, work: WorkId, tx: &Outbox) {
    super::changed::page(kit, &work);
    let id = tiles::anilist(work.as_str());
    let kept = id.and_then(|id| kit.meta.kept_book(id));
    let wants_page = id.is_some() && kept.as_ref().is_none_or(|k| !k.detail_fresh(ask.now));
    send(tx, ReadingEvent::Series(Box::new(view(kit, ask, &work, wants_page))));
    let (kit, tx) = (kit.clone(), tx.clone());
    tokio::spawn(async move {
        let page = async {
            let Some(id) = id.filter(|_| wants_page) else { return };
            match kit.meta.book(id).await {
                Ok(book) => {
                    crate::read_tracking::note_book(&kit.store, book.id, book.id_mal, ask.now);
                    send(&tx, ReadingEvent::Series(Box::new(view(&kit, ask, &work, false))));
                    // More like this: the ones nothing is kept of, behind the page.
                    let works: Vec<String> =
                        book.recommended().iter().map(|id| tiles::work_of(*id).0).collect();
                    let missing = tiles::missing(&kit.meta, &works, ask.now);
                    if !missing.is_empty()
                        && kaeri_meta::on_screen(kit.meta.books_many(&missing)).await.is_ok()
                    {
                        send(
                            &tx,
                            ReadingEvent::Series(Box::new(view(&kit, ask, &work, false))),
                        );
                    }
                }
                Err(e) => {
                    tracing::info!(error = %e, "a book's page was not asked for");
                    send(&tx, ReadingEvent::Series(Box::new(view(&kit, ask, &work, false))));
                }
            }
        };
        let listed = chapters(&kit, ask, &work, None, &tx);
        // With nothing kept of the book, its medium is not known until the page lands, and
        // the rows' words depend on it: a novel's part says a percent, a chapter its page.
        if kept.is_none() && wants_page {
            page.await;
            listed.await;
        } else {
            tokio::join!(page, listed);
        }
    });
}

/// A page's chapter list, sent once the engine knows it: from the edition, from the files on
/// this device, or why there is none. Nothing is sent while the answer is still on its way, so
/// a page never shows a failure the engine has not settled on:
///
/// - Book plugins still compiling are waited for, as a series one of them holds is not found
///   without them.
/// - A series no source is matched to yet is searched for by name first (see
///   [`editions::check`]), and found there it is read from that source.
/// - An edition in use that lists no chapters has the work's other editions asked first, so
///   the edition picked can move to one that has them.
///
/// A list kept from before is sent first, as it stands, and the source's answer after it.
///
/// A second ask for the same list while one is on its way sends nothing more: the first one's
/// answer reaches the page.
pub(crate) async fn chapters(
    kit: &Kit,
    ask: Ask,
    work: &WorkId,
    edition: Option<String>,
    tx: &Outbox,
) {
    let key = (work.as_str().to_owned(), edition.clone());
    let Some(_turn) = Turn::take(kit, key) else { return };
    let now = ask.now;
    kit.plugins.arrived(editions::PLUGINS_WAIT).await;
    kit.sources.adopt_plugins(&kit.plugins);
    if kit.sources.edition_of(work, edition.as_deref()).is_none() {
        // The files on this device, when there are any, are the list.
        let list = chapters::of_work(&kit.sources, &kit.store, work);
        let volumes = crate::reading::shelf::volumes(
            &kit.store,
            work.as_str(),
            list.as_ref().map(|(_, _, l)| l.as_slice()),
            medium_of(kit, work),
            now,
            &kit.clock.zone(),
        );
        if volumes.is_empty()
            && let Some(check) = editions::check(kit, ask, work, tx)
        {
            editions::ended(check).await;
        }
        if volumes.is_empty() && kit.sources.edition_of(work, edition.as_deref()).is_some() {
            // Found by name: read from there, below.
        } else {
            // A source switched off still holds it: the page says so by the source's name.
            let failure = volumes.is_empty().then(|| {
                kit.sources
                    .switched_off_holder(work)
                    .map_or(ReadingFailure::NoSource, |source| ReadingFailure::SourceOff {
                        source,
                    })
            });
            send(
                tx,
                ReadingEvent::Chapters { work: work.clone(), edition: None, volumes, failure },
            );
            send(tx, ReadingEvent::Series(Box::new(view(kit, ask, work, false))));
            return;
        }
    }
    // The list kept from before shows at once while the source is asked: a source slow to
    // answer, or not answering at all, never leaves a known list empty. Its answer replaces it.
    let asking = kit.sources.edition_of(work, edition.as_deref()).map(|(source, _)| source);
    let (kept_edition, mut kept) = kept_volumes(kit, work);
    if !kept.is_empty() && kept_edition.as_ref().map(|e| &e.source_id) == asking.as_ref() {
        finish(kit, work, &mut kept, now);
        send(
            tx,
            ReadingEvent::Chapters {
                work: work.clone(),
                edition: kept_edition,
                volumes: kept,
                failure: None,
            },
        );
    }
    let mut asked = kit.sources.chapters(work, edition.as_deref(), now).await;
    if edition.is_none()
        && matches!(&asked, Ok((_, volumes)) if volumes.is_empty())
        && editions::list_others(kit, work, now, tx).await
    {
        asked = kit.sources.chapters(work, None, now).await;
    }
    let event = match asked {
        Ok((edition, mut volumes)) => {
            finish(kit, work, &mut volumes, now);
            ReadingEvent::Chapters { work: work.clone(), edition, volumes, failure: None }
        }
        // The source did not answer: the list it gave last, with why.
        Err(failure) => {
            let (edition, mut volumes) = kept_volumes(kit, work);
            finish(kit, work, &mut volumes, now);
            ReadingEvent::Chapters {
                work: work.clone(),
                edition,
                volumes,
                failure: Some(failure),
            }
        }
    };
    send(tx, event);
    // The main button follows the list that came.
    send(tx, ReadingEvent::Series(Box::new(view(kit, ask, work, false))));
}

/// One chapter-list ask on its way, by work and edition: let go when it is dropped.
struct Turn {
    loads: super::ChapterLoads,
    key: (String, Option<String>),
}

impl Turn {
    fn take(kit: &Kit, key: (String, Option<String>)) -> Option<Self> {
        let loads = kit.sources.catalog.chapter_loads.clone();
        let fresh = loads.lock().unwrap_or_else(|e| e.into_inner()).insert(key.clone());
        fresh.then_some(Self { loads, key })
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        self.loads.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.key);
    }
}

/// The header again, after a change to the list or the score.
pub fn header(kit: &Kit, ask: Ask, work: &WorkId, tx: &Outbox) {
    send(tx, ReadingEvent::Series(Box::new(view(kit, ask, work, false))));
}

/// One chapter as every edition the person has lists it.
pub fn versions(kit: &Kit, work: WorkId, chapter: ChapterId, tx: &Outbox) {
    let editions = kit.sources.editions(&work);
    let refs = kit.store.book_refs_for_work(work.as_str()).unwrap_or_default();
    let mut versions = Vec::new();
    for (source, series) in refs {
        let Some(edition) = editions.iter().find(|e| e.source_id == source) else { continue };
        let Some((list, _)) = chapters::kept(&kit.store, &source, &series) else { continue };
        // One version per group: a chapter in parts is one version, from its first part.
        let mut groups = HashSet::new();
        for found in list
            .iter()
            .filter(|c| c.id == chapter.as_str() && groups.insert(c.version_label.clone()))
        {
            let mut row = edition.clone();
            row.version_label = found.version_label.clone().or(row.version_label);
            row.language = found.language.clone().or(row.language);
            versions.push(ChapterVersion {
                edition: row,
                chapter_ref: found.source_ref.clone(),
                published: found.published,
            });
        }
    }
    send(tx, ReadingEvent::Versions { work, chapter, versions });
}

/// One row per chapter of a source's kept list, as every screen lists it: its parts as one,
/// from the preferred group, or another where that one lacks it. Each with the index in `list`
/// of its first entry.
pub(crate) fn kept_rows(
    kit: &Kit,
    work: &WorkId,
    source: &str,
    list: &[chapters::Kept],
    read: &chapters::Read,
) -> Vec<(usize, ChapterRow)> {
    let entries: Vec<_> = list.iter().map(chapters::Kept::source_chapter).collect();
    let chosen = chapters::chosen_group(&kit.store, work.as_str(), source);
    let language = kit.sources.language();
    let arranged = super::arrange::arrange(&entries, chosen.as_deref(), language.as_deref());
    arranged
        .rows
        .iter()
        .map(|arranged_row| {
            let first = arranged_row.parts[0];
            let mut row = super::updates::row_of(&list[first], read);
            row.new_chapter = false;
            arranged.fill(arranged_row, language.as_deref(), &mut row);
            if super::arrange::Arranged::in_parts(arranged_row) {
                row.label = super::arrange::merged_label(&entries[first], &row.id);
                row.pages = arranged_row.parts.iter().map(|&i| list[i].pages).sum();
                row.number =
                    row.id.key().and_then(|k| k.range()).map(|r| r.to_string()).or(row.number);
                let title = super::arrange::merged_title(&entries[first]);
                row.title =
                    chapters::own_title(title.as_deref(), &row.label, row.number.as_deref())
                        .map(str::to_owned);
            }
            (first, row)
        })
        .collect()
}

/// The edition's chapters as last kept, grouped by volume.
pub(crate) fn kept_volumes(
    kit: &Kit,
    work: &WorkId,
) -> (Option<BookEditionRow>, Vec<VolumeGroup>) {
    let Some((source, _, list)) = chapters::of_work(&kit.sources, &kit.store, work) else {
        return (None, Vec::new());
    };
    let read = chapters::Read::of(&kit.store, work.as_str());
    let mut groups: Vec<VolumeGroup> = Vec::new();
    let mut group_of: HashMap<Option<String>, usize> = HashMap::new();
    for (first, row) in kept_rows(kit, work, &source, &list, &read) {
        let chapter = &list[first];
        let at = *group_of.entry(chapter.volume.clone()).or_insert_with(|| {
            groups.push(VolumeGroup {
                volume: chapter.volume.clone(),
                title: None,
                chapters: Vec::new(),
                all_read: false,
            });
            groups.len() - 1
        });
        groups[at].chapters.push(row);
    }
    for group in &mut groups {
        group.all_read =
            group.chapters.iter().all(|c| matches!(c.state, ChapterReadState::Read));
    }
    let edition = kit.sources.editions(work).into_iter().find(|e| e.source_id == source);
    (edition, groups)
}

/// The rows as every screen shows them: how many versions each has, the new ones, and the
/// words on their second line.
pub(crate) fn finish(kit: &Kit, work: &WorkId, volumes: &mut [VolumeGroup], now: i64) {
    count_versions(kit, work, volumes);
    mark_new(kit, work, volumes);
    let medium = medium_of(kit, work);
    super::rows::fill(volumes, super::rows::Kept::Source, medium, now, &kit.clock.zone());
}

/// How many versions each chapter has across the person's editions.
fn count_versions(kit: &Kit, work: &WorkId, volumes: &mut [VolumeGroup]) {
    let refs = kit.store.book_refs_for_work(work.as_str()).unwrap_or_default();
    // A version is a chapter as one group in one source has it, its parts and all.
    let mut seen: HashSet<(String, String, Option<String>)> = HashSet::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (source, series) in &refs {
        for chapter in
            chapters::kept(&kit.store, source, series).map(|k| k.0).unwrap_or_default()
        {
            if seen.insert((source.clone(), chapter.id.clone(), chapter.version_label)) {
                *counts.entry(chapter.id).or_default() += 1;
            }
        }
    }
    for row in volumes.iter_mut().flat_map(|v| v.chapters.iter_mut()) {
        let count = counts.get(row.id.as_str()).copied().unwrap_or(0);
        row.versions = u32::try_from(count.max(1)).unwrap_or(u32::MAX);
    }
}

/// The dot on chapters that came out since the person last read and are not read.
fn mark_new(kit: &Kit, work: &WorkId, volumes: &mut [VolumeGroup]) {
    let Some((_, _, list)) = chapters::of_work(&kit.sources, &kit.store, work) else { return };
    let read = chapters::Read::of(&kit.store, work.as_str());
    if !read.any() {
        return;
    }
    let new: HashSet<&str> = read
        .unread(&list)
        .into_iter()
        .filter(|c| c.seen_at.is_some())
        .map(|c| c.id.as_str())
        .collect();
    for row in volumes.iter_mut().flat_map(|v| v.chapters.iter_mut()) {
        row.new_chapter = new.contains(row.id.as_str());
    }
}

/// The header from what is kept: AniList's book when there is one, the folder's name when not.
pub(crate) fn view(kit: &Kit, ask: Ask, work: &WorkId, partial: bool) -> SeriesView {
    let kept = tiles::anilist(work.as_str()).and_then(|id| kit.meta.kept_book(id));
    let (list, score) = kit.store.read_status(work.as_str()).unwrap_or_default();
    let list = list.as_deref().and_then(LibrarySegmentRead::parse);
    let score = score.map(|s| s as f32);
    let editions = kit.sources.editions(work);
    let edition = kit
        .sources
        .edition_of(work, None)
        .and_then(|(source, _)| editions.into_iter().find(|e| e.source_id == source));
    let ask_where_stopped =
        kit.store.pending_sent(work.as_str()).ok().flatten().map(|sent| SentChapters {
            target: sent.target,
            sent_at: sent.at,
            chapters: sent.chapters.into_iter().map(ChapterId::new).collect(),
        });
    let rhythm = kit.store.book_rhythm(work.as_str()).ok().flatten();
    let demo = kit
        .sources
        .edition_of(work, None)
        .filter(|(source, _)| source == kaeri_core::demo::DEMO_SOURCE_ID)
        .and_then(|(_, series)| kaeri_core::reading_demo::work(&series));
    let medium = match (&kept, demo) {
        (Some(k), _) => k.book.medium(),
        (None, Some(demo)) => demo.medium(),
        (None, None) => local_medium(kit, work),
    };
    let action = tiles::action(&kit.store, &kit.sources, work, medium);
    let mut view = SeriesView {
        work: work.clone(),
        medium,
        titles: WorkTitles {
            display: local_name(kit, work).unwrap_or_default(),
            romaji: None,
            english: None,
            native: None,
            synonyms: Vec::new(),
        },
        action,
        cover: None,
        banner: None,
        format: None,
        origin: None,
        status: None,
        synopsis: None,
        genres: Vec::new(),
        authors: Vec::new(),
        chapters_total: None,
        volumes_total: None,
        edition,
        list,
        score,
        tracking: Vec::new(),
        other_medium: None,
        ask_where_stopped,
        rhythm: rhythm.as_ref().and_then(chapters::rhythm_words),
        facts: Vec::new(),
        year: None,
        adult: false,
        recommended: Vec::new(),
        partial,
        demo: None,
        demo_other: None,
    };
    if let Some(kept) = kept {
        fill(kit, ask, &mut view, &kept);
    } else if let Some(demo) = demo {
        fill_demo(&mut view, demo);
    }
    view
}

/// A work of the reading demo: its header from the demo itself, with its credit and the demo's
/// other work.
fn fill_demo(view: &mut SeriesView, demo: &kaeri_core::reading_demo::DemoWork) {
    use kaeri_core::reading_demo::WORKS;
    view.titles.display = demo.series.to_owned();
    view.cover = demo.cover().map(str::to_owned);
    view.format = Some(demo.format().to_owned());
    view.synopsis = Some(demo.synopsis.to_owned());
    view.authors = vec![demo.author.to_owned()];
    view.chapters_total = Some(1);
    view.facts =
        vec![kaeri_model::reading::kind_words(Some(demo.format()), demo.medium(), None)];
    view.demo = Some(kaeri_model::sources::DemoWorkInfo::of(demo));
    view.demo_other = WORKS.iter().find(|other| other.id != demo.id).map(|o| o.work_id());
}

fn fill(kit: &Kit, ask: Ask, view: &mut SeriesView, kept: &KeptBook) {
    let book: &Book = &kept.book;
    view.titles = WorkTitles {
        display: kit.meta.titles().pick(&book.title).to_owned(),
        romaji: book.title.romaji.clone(),
        english: book.title.english.clone(),
        native: book.title.native.clone(),
        synonyms: book.synonyms.clone(),
    };
    view.cover = book.cover_image.best().map(str::to_owned);
    view.banner = book.banner_image.clone();
    view.format = Some(book.reading_format().as_str().to_owned());
    view.origin = book.origin().map(|o| o.code().to_owned());
    view.status = tiles::status_key(book.status).map(str::to_owned);
    view.synopsis = book.synopsis();
    view.genres = book.genres.clone();
    view.authors = book.authors();
    view.chapters_total = book.chapters;
    view.volumes_total = book.volumes;
    view.facts = tiles::facts(book);
    view.year = book.start_date.as_ref().and_then(|d| d.year).map(u32::from);
    view.adult = book.adult();
    view.recommended = kit
        .meta
        .kept_books(&book.recommended())
        .iter()
        .filter(|k| ask.adult || !k.book.adult())
        .map(|k| tiles::plain(&kit.meta, &k.book))
        .collect();
    // Also a show: only to someone who both watches and reads.
    if ask.both()
        && let Some((show, _)) = book.shows().first()
    {
        view.other_medium = Some(CrossMediumRow {
            work: tiles::work_of(show.id),
            medium: Medium::Anime,
            title: kit.meta.titles().pick(&show.title).to_owned(),
            relation: Msg::Reading(ReadingMsg::ReadingAlsoAShow),
        });
    }
}

/// A work AniList does not list: its folder's name.
pub(crate) fn local_name(kit: &Kit, work: &WorkId) -> Option<String> {
    kit.store
        .local_book_series()
        .ok()?
        .into_iter()
        .find(|s| s.work.as_deref() == Some(work.as_str()))
        .map(|s| s.name)
}

/// The series' medium: the catalogue's, else its folder's.
pub(crate) fn medium_of(kit: &Kit, work: &WorkId) -> Medium {
    tiles::anilist(work.as_str())
        .and_then(|id| kit.meta.kept_book(id))
        .map_or_else(|| local_medium(kit, work), |k| k.book.medium())
}

pub(crate) fn local_medium(kit: &Kit, work: &WorkId) -> Medium {
    kit.store
        .local_book_series()
        .ok()
        .and_then(|all| all.into_iter().find(|s| s.work.as_deref() == Some(work.as_str())))
        .and_then(|s| Medium::parse(&s.medium))
        .unwrap_or(Medium::Manga)
}
