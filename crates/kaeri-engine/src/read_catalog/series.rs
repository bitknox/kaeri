//! A series' page: the header from what was kept at once, then AniList's page and the
//! edition's chapters as they land, and each chapter's versions across the person's sources.

use kaeri_core::{
    books::{ChapterId, WorkId},
    media::Medium,
};
use kaeri_meta::{KeptBook, anilist::books::Book};
use kaeri_model::{
    Msg, ReadingMsg,
    reading::{
        BookEditionRow, ChapterReadState, ChapterVersion, CrossMediumRow, LibrarySegmentRead,
        ReadingEvent, SentChapters, SeriesView, VolumeGroup, WorkTitles,
    },
};

use super::{Ask, Kit, chapters, send, tiles};
use crate::outbox::Outbox;

/// The page: the header at once, then AniList's page and the chapters.
pub fn load(kit: &Kit, ask: Ask, work: WorkId, tx: &Outbox) {
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
        let listed = async {
            kit.sources.adopt_plugins(&kit.plugins);
            if kit.sources.edition_of(&work, None).is_none() {
                return;
            }
            let event = match kit.sources.chapters(&work, None, ask.now).await {
                Ok((edition, mut volumes)) => {
                    finish(&kit, &work, &mut volumes, ask.now);
                    ReadingEvent::Chapters {
                        work: work.clone(),
                        edition,
                        volumes,
                        failure: None,
                    }
                }
                // The source did not answer: the list it gave last, with why.
                Err(failure) => {
                    let (edition, mut volumes) = kept_volumes(&kit, &work);
                    finish(&kit, &work, &mut volumes, ask.now);
                    ReadingEvent::Chapters {
                        work: work.clone(),
                        edition,
                        volumes,
                        failure: Some(failure),
                    }
                }
            };
            send(&tx, event);
            // The main button follows the list that came.
            send(&tx, ReadingEvent::Series(Box::new(view(&kit, ask, &work, false))));
        };
        tokio::join!(page, listed);
    });
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
        for found in list.iter().filter(|c| c.id == chapter.as_str()) {
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
    for chapter in &list {
        let mut row = super::updates::row_of(chapter, &read);
        row.new_chapter = false;
        match groups.iter_mut().find(|g| g.volume == chapter.volume) {
            Some(group) => group.chapters.push(row),
            None => groups.push(VolumeGroup {
                volume: chapter.volume.clone(),
                title: None,
                chapters: vec![row],
                all_read: false,
            }),
        }
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
    super::rows::fill(volumes, super::rows::Kept::Source, now, &kit.clock.zone());
}

/// How many versions each chapter has across the person's editions.
fn count_versions(kit: &Kit, work: &WorkId, volumes: &mut [VolumeGroup]) {
    let refs = kit.store.book_refs_for_work(work.as_str()).unwrap_or_default();
    let lists: Vec<Vec<chapters::Kept>> = refs
        .iter()
        .filter_map(|(source, series)| chapters::kept(&kit.store, source, series).map(|k| k.0))
        .collect();
    for row in volumes.iter_mut().flat_map(|v| v.chapters.iter_mut()) {
        let count = lists.iter().flatten().filter(|c| c.id == row.id.as_str()).count();
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
    let new: Vec<&str> = read
        .unread(&list)
        .into_iter()
        .filter(|c| c.seen_at.is_some())
        .map(|c| c.id.as_str())
        .collect();
    for row in volumes.iter_mut().flat_map(|v| v.chapters.iter_mut()) {
        row.new_chapter = new.contains(&row.id.as_str());
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
    let demo = kit
        .sources
        .edition_of(work, None)
        .filter(|(source, _)| source == kaeri_core::demo::DEMO_SOURCE_ID)
        .and_then(|(_, series)| kaeri_core::reading_demo::work(&series));
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
        demo: demo.map(kaeri_model::sources::DemoWorkInfo::of),
        demo_other: demo
            .and_then(|this| {
                kaeri_core::reading_demo::WORKS.iter().find(|other| other.id != this.id)
            })
            .map(|other| kit.sources.demo_work_id(other)),
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

pub(crate) fn local_medium(kit: &Kit, work: &WorkId) -> Medium {
    kit.store
        .local_book_series()
        .ok()
        .and_then(|all| all.into_iter().find(|s| s.work.as_deref() == Some(work.as_str())))
        .and_then(|s| Medium::parse(&s.medium))
        .unwrap_or(Medium::Manga)
}
