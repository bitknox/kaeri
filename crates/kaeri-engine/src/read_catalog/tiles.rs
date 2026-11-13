//! A book as a cover anywhere: its name in the person's title language, its kind, its status
//! and, where they read it, where they are.

use kaeri_core::{
    books::{ChapterId, WorkId},
    ids::AnilistId,
    media::{MediaStatus, Medium},
};
use kaeri_meta::{KeptBook, Meta, anilist::books::Book};
use kaeri_model::{
    Msg, ReadingMsg,
    reading::{self as words, SeriesAction, WorkTile},
};
use kaeri_store::Store;

use super::chapters::{self, Read};

pub fn work_of(id: AnilistId) -> WorkId {
    WorkId::new(format!("a:{}", id.get()))
}

/// The AniList id behind a work's key, when it has one.
pub fn anilist(work: &str) -> Option<AnilistId> {
    kaeri_model::reading::anilist_of(&WorkId::new(work.to_owned()))
}

/// AniList's status as the model's key: `releasing`, `finished`, `hiatus`, `cancelled`,
/// `not-yet`.
pub fn status_key(status: Option<MediaStatus>) -> Option<&'static str> {
    Some(match status? {
        MediaStatus::Releasing => "releasing",
        MediaStatus::Finished => "finished",
        MediaStatus::Hiatus => "hiatus",
        MediaStatus::Cancelled => "cancelled",
        MediaStatus::NotYetReleased => "not-yet",
        MediaStatus::Unknown => return None,
    })
}

/// The words under a book: its kind, its status, and for a finished one how long it is.
pub fn facts(book: &Book) -> Vec<Msg> {
    let format = book.reading_format();
    let origin = book.origin();
    let mut out = vec![words::kind_words(
        Some(format.as_str()),
        book.medium(),
        origin.as_ref().map(kaeri_core::media::Origin::code),
    )];
    if let Some(status) = status_key(book.status).and_then(words::status_words) {
        out.push(status);
    }
    let done = matches!(book.status, Some(MediaStatus::Finished | MediaStatus::Cancelled));
    if (done || book.medium() == kaeri_core::media::Medium::Novel)
        && let Some(count) = words::count_words(book.medium(), book.chapters, book.volumes)
    {
        out.push(count);
    }
    out
}

/// A book's cover, with nothing of the person's reading on it.
pub fn plain(meta: &Meta, book: &Book) -> WorkTile {
    WorkTile {
        work: work_of(book.id),
        title: meta.titles().pick(&book.title).to_owned(),
        medium: book.medium(),
        cover: book.cover_image.best().map(str::to_owned),
        format_label: Some(book.reading_format().as_str().to_owned()),
        progress: None,
        next: None,
        new_chapter: false,
        adult: book.adult(),
        facts: facts(book),
        demo: false,
        not_in_sources: false,
        origin: origin(book.origin()),
        amount: None,
        started: None,
    }
}

/// The month a series started, as a New row's caption says it: "since March".
pub fn started(book: &Book) -> Option<Msg> {
    let month = book.start_date.as_ref()?.month?;
    Some(Msg::Reading(match month {
        1 => ReadingMsg::ReadingSinceJanuary,
        2 => ReadingMsg::ReadingSinceFebruary,
        3 => ReadingMsg::ReadingSinceMarch,
        4 => ReadingMsg::ReadingSinceApril,
        5 => ReadingMsg::ReadingSinceMay,
        6 => ReadingMsg::ReadingSinceJune,
        7 => ReadingMsg::ReadingSinceJuly,
        8 => ReadingMsg::ReadingSinceAugust,
        9 => ReadingMsg::ReadingSinceSeptember,
        10 => ReadingMsg::ReadingSinceOctober,
        11 => ReadingMsg::ReadingSinceNovember,
        12 => ReadingMsg::ReadingSinceDecember,
        _ => return None,
    }))
}

/// The country a series comes from, in the words of the Origin filter.
pub fn origin(origin: Option<kaeri_core::media::Origin>) -> Option<Msg> {
    use kaeri_core::media::Origin;
    Some(Msg::Reading(match origin? {
        Origin::Japan => ReadingMsg::ReadingOriginJapan,
        Origin::Korea => ReadingMsg::ReadingOriginKorea,
        Origin::China => ReadingMsg::ReadingOriginChina,
        Origin::Taiwan => ReadingMsg::ReadingOriginTaiwan,
        Origin::Other(_) => return None,
    }))
}

/// How much of a series a source holds, as the line under its hit says it.
pub fn amount(amount: &kaeri_core::books::HitAmount) -> Msg {
    use kaeri_core::books::HitAmount;
    let int = |n: &u32| i64::from(*n);
    Msg::Reading(match amount {
        HitAmount::Volumes(n) => ReadingMsg::ReadingVolumes { count: int(n) },
        HitAmount::Chapters(1) => ReadingMsg::ReadingChapters { count: 1 },
        HitAmount::Chapters(n) => ReadingMsg::ReadHitChapters { last: int(n) },
        HitAmount::Episodes(n) => ReadingMsg::ReadHitEpisodes { last: int(n) },
        HitAmount::Parts(n) => ReadingMsg::ReadHitParts { count: int(n) },
        HitAmount::Files { count, format: Some(format) } => {
            ReadingMsg::ReadHitFilesFormat { count: int(count), format: format.clone() }
        }
        HitAmount::Files { count, format: None } => {
            ReadingMsg::ReadHitFiles { count: int(count) }
        }
    })
}

/// A work of the reading demo, with where the person is in it.
pub fn demo(
    work: &str,
    store: &Store,
    sources: &crate::read_sources::BookSources,
) -> Option<WorkTile> {
    let demo = kaeri_core::reading_demo::WORKS.iter().find(|w| w.work_id().as_str() == work)?;
    let mut tile = crate::read_sources::demo_tile(demo);
    mark(&mut tile, store, sources);
    Some(tile)
}

/// A book's cover with where the person is in it: the page they stopped on, or the next
/// chapter, and the dot when one came out that they have not read.
pub fn reading(
    meta: &Meta,
    store: &Store,
    sources: &crate::read_sources::BookSources,
    book: &Book,
) -> WorkTile {
    let mut tile = plain(meta, book);
    mark(&mut tile, store, sources);
    tile
}

/// Put where the person is on a tile.
pub fn mark(tile: &mut WorkTile, store: &Store, sources: &crate::read_sources::BookSources) {
    let read = Read::of(store, tile.work.as_str());
    let list = chapters::of_work(sources, store, &tile.work).map(|(_, _, list)| list);
    let novel = tile.medium == Medium::Novel;
    if let Some((chapter, page, of)) = &read.in_progress {
        let label = number_of(list.as_deref(), chapter);
        // The page count the reader saw, else the one the source lists.
        let of = of.or_else(|| {
            list.as_deref()
                .and_then(|l| l.iter().find(|c| c.id == *chapter))
                .and_then(|c| c.pages)
        });
        // A light novel reads in parts, so its place is a part and how far into it, by the
        // name its row on the series page has.
        tile.next = Some(match page {
            _ if novel => words::part_place(
                &label,
                named_in(list.as_deref(), chapter).as_deref(),
                words::part_percent(*page, of),
            ),
            Some(page) => words::chapter_page(&label, *page),
            None => words::next_chapter(&label),
        });
        tile.progress = match (page, of) {
            (Some(page), Some(of)) if of > 0 => Some((*page as f32 / of as f32).min(1.0)),
            _ => None,
        };
    } else if let Some(list) = list.as_deref() {
        // The dot always has its words: "Chapter 88 is new".
        if let Some(new) = read.new_for_you(list) {
            let number = new.number.clone().unwrap_or_else(|| new.label.clone());
            let named = words::part_named(&new.label, new.number.as_deref());
            tile.next = Some(words::unit_new(tile.medium, &number, named.as_deref()));
            tile.new_chapter = true;
        } else if let Some(next) = read.next(list) {
            tile.next = Some(words::next_unit(
                tile.medium,
                next.number.as_deref().unwrap_or(&next.label),
                words::part_named(&next.label, next.number.as_deref()).as_deref(),
            ));
        }
    }
    no_title_again(tile);
}

/// A line that would only say the cover's title again says nothing, and goes: a book in one
/// file, whose one part is named for the book, would otherwise read "Rooftop Lantern" twice.
fn no_title_again(tile: &mut WorkTile) {
    let same = |text: &str| text.trim().eq_ignore_ascii_case(tile.title.trim());
    if matches!(&tile.next, Some(Msg::Verbatim { text }) if same(text)) {
        tile.next = None;
    }
}

/// A Library cover's line: the chapter the person is at and the newest there is, "Chapter 41
/// of 43", and for a light novel its volume and part, "Volume 6, part 5". Shorter than the
/// other covers' words, which the track under the cover already says.
pub fn library_line(
    tile: &mut WorkTile,
    store: &Store,
    sources: &crate::read_sources::BookSources,
) {
    let Some((_, _, list)) = chapters::of_work(sources, store, &tile.work) else { return };
    let read = Read::of(store, tile.work.as_str());
    let at = read
        .in_progress
        .as_ref()
        .and_then(|(id, _, _)| list.iter().find(|c| c.id == *id))
        .or_else(|| read.next(&list))
        .or_else(|| list.iter().rev().find(|c| read.is_read(&c.id)));
    let Some(at) = at else { return };
    let number = at.number.clone().unwrap_or_else(|| at.label.clone());
    let line = if tile.medium == Medium::Novel {
        let named = words::part_named(&at.label, at.number.as_deref());
        match (&at.volume, named) {
            (Some(volume), Some(label)) => {
                ReadingMsg::ReadingVolumeLabel { volume: volume.clone(), label }
            }
            (Some(volume), None) => {
                ReadingMsg::ReadingVolumePart { volume: volume.clone(), part: number }
            }
            (None, named) => {
                tile.next = Some(words::part_place(&number, named.as_deref(), None));
                no_title_again(tile);
                return;
            }
        }
    } else {
        match newest_number(&list, &number) {
            Some(of) => ReadingMsg::ReadingChapterOf { chapter: number, of },
            None => return,
        }
    };
    tile.next = Some(Msg::Reading(line));
}

/// The newest chapter's number as people read it: the highest of the numbered ones, an extra
/// after it, 115.6, counted as its chapter, 115. Whole when `at`, the chapter the person is at,
/// is not past it.
fn newest_number(list: &[chapters::Kept], at: &str) -> Option<String> {
    let (value, number) = list
        .iter()
        .filter_map(|c| {
            let number = c.number.as_deref()?;
            Some((number.parse::<f64>().ok()?, number))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))?;
    let whole = value.trunc();
    let past = at.parse::<f64>().is_ok_and(|at| at > whole);
    if whole == value || past || !(0.0..=f64::from(u32::MAX)).contains(&whole) {
        return Some(number.to_owned());
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((whole as u32).to_string())
}

/// The source's own name for a light novel's part, when it says more than the number: what its
/// row is called, so the cover and the button name it the same way.
pub fn named_in(list: Option<&[chapters::Kept]>, chapter: &str) -> Option<String> {
    let kept = list?.iter().find(|c| c.id == chapter)?;
    words::part_named(&kept.label, kept.number.as_deref())
}

/// A chapter's number as a person reads it, from the list when it is there.
pub fn number_of(list: Option<&[chapters::Kept]>, chapter: &str) -> String {
    list.and_then(|l| l.iter().find(|c| c.id == chapter))
        .and_then(|c| c.number.clone())
        .unwrap_or_else(|| chapter.rsplit('/').next().unwrap_or(chapter).to_owned())
}

/// The series page's main button: the chapter in progress, the next one, Read again once
/// everything is read, and No source when none of the person's sources has it. While plugins
/// are still `arriving`, a series held in a source not in yet goes on from its kept list rather
/// than being said to be in none.
pub fn action(
    store: &Store,
    sources: &crate::read_sources::BookSources,
    work: &WorkId,
    medium: kaeri_core::media::Medium,
    plugins: &crate::book_plugins::BookPlugins,
) -> SeriesAction {
    let edition = sources
        .edition_of(work, None)
        .or_else(|| plugins.lean_on_arrivals(|| sources.edition_not_in_yet(work)));
    let Some((source, series)) = edition else {
        return SeriesAction::NoSource;
    };
    let read = Read::of(store, work.as_str());
    let list = chapters::kept(store, &source, &series).map(|(l, _)| l);
    if let Some((chapter, page, of)) = read.in_progress.clone() {
        let label = number_of(list.as_deref(), &chapter);
        let named = named_in(list.as_deref(), &chapter);
        // The page count the reader saw, else the one the source lists, as the cover does.
        let of = of.or_else(|| {
            list.as_deref()
                .and_then(|l| l.iter().find(|c| c.id == chapter))
                .and_then(|c| c.pages)
        });
        let chapter = ChapterId::new(chapter);
        return match (medium, page) {
            (Medium::Novel, _) => SeriesAction::ContinuePart {
                chapter,
                label,
                percent: words::part_percent(page, of),
                named,
            },
            (_, Some(page)) => {
                SeriesAction::Continue { chapter, label, page: words::shown_page(page) }
            }
            (_, None) => SeriesAction::Read { chapter, label },
        };
    }
    let Some(list) = list else {
        // The source answered with no chapters: nothing here can be read, so the button offers
        // another source rather than a chapter that is not there.
        if sources.listing_of(&source, &series)
            == Some(kaeri_model::reading::EditionListing::Empty)
        {
            return SeriesAction::NoSource;
        }
        // Not asked yet: the first chapter, until the source answers.
        return read_action(medium, ChapterId::new("1"), "1".into(), None);
    };
    match read.next(&list) {
        Some(next) => read_action(
            medium,
            ChapterId::new(next.id.clone()),
            next.number.clone().unwrap_or_else(|| next.label.clone()),
            words::part_named(&next.label, next.number.as_deref()),
        ),
        None if list.is_empty() => SeriesAction::NoSource,
        None => SeriesAction::ReadAgain,
    }
}

/// Read a chapter not opened yet: "Read chapter 88", or "Read part 3" for a light novel, or the
/// source's own name for the part when it has one (`named`).
pub fn read_action(
    medium: Medium,
    chapter: ChapterId,
    label: String,
    named: Option<String>,
) -> SeriesAction {
    match medium {
        Medium::Novel => SeriesAction::ReadPart { chapter, label, named },
        _ => SeriesAction::Read { chapter, label },
    }
}

/// "On your Planning list", for Up next.
pub fn on_planning() -> Msg {
    Msg::Reading(ReadingMsg::ReadingOnPlanning)
}

/// The books kept for `works`, in the order asked; a local work and one not kept are left out.
pub fn kept_of(meta: &Meta, works: &[String]) -> Vec<KeptBook> {
    let ids: Vec<AnilistId> = works
        .iter()
        .filter_map(|w| kaeri_model::reading::anilist_of(&WorkId::new(w.clone())))
        .collect();
    meta.kept_books(&ids)
}

/// The AniList ids of `works` that nothing at all is kept of: their covers, titles and medium
/// are not known here yet, however old a kept one may be.
pub fn unkept(meta: &Meta, works: &[String]) -> Vec<AnilistId> {
    let ids: Vec<AnilistId> = works
        .iter()
        .filter_map(|w| kaeri_model::reading::anilist_of(&WorkId::new(w.clone())))
        .collect();
    let kept = meta.kept_books(&ids);
    ids.into_iter().filter(|id| !kept.iter().any(|k| k.book.id == *id)).collect()
}

/// The AniList ids of `works` that nothing is kept of, or only something older than a day.
pub fn missing(meta: &Meta, works: &[String], now: i64) -> Vec<AnilistId> {
    let ids: Vec<AnilistId> = works
        .iter()
        .filter_map(|w| kaeri_model::reading::anilist_of(&WorkId::new(w.clone())))
        .collect();
    let kept = meta.kept_books(&ids);
    ids.into_iter()
        .filter(|id| !kept.iter().any(|k| k.book.id == *id && k.fresh(now)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(numbers: &[&str]) -> Vec<chapters::Kept> {
        numbers
            .iter()
            .map(|n| {
                serde_json::from_value(serde_json::json!({
                    "id": n, "source_ref": format!("c-{n}"), "label": format!("Chapter {n}"),
                    "number": n,
                }))
                .unwrap()
            })
            .collect()
    }

    /// A Library cover counts to the newest chapter, an extra after it counted as its chapter.
    #[test]
    fn the_newest_chapter_counts_an_extra_as_its_chapter() {
        assert_eq!(newest_number(&list(&["1", "2", "115"]), "5").as_deref(), Some("115"));
        assert_eq!(newest_number(&list(&["1", "115", "115.6"]), "5").as_deref(), Some("115"));
        // A chapter read past the whole number keeps the extra, never "115.6 of 115".
        assert_eq!(
            newest_number(&list(&["1", "115", "115.6"]), "115.6").as_deref(),
            Some("115.6")
        );
        assert_eq!(newest_number(&list(&[]), "1"), None);
    }

    /// A book in one file names its one part for the book: the cover says the title once.
    #[test]
    fn a_line_that_only_says_the_title_again_goes() {
        let mut tile = WorkTile {
            work: WorkId::new("l:00000000000000000000000000000001"),
            title: "Rooftop Lantern".into(),
            medium: Medium::Novel,
            cover: None,
            format_label: None,
            progress: None,
            next: Some(words::part_place("Rooftop Lantern", Some("rooftop lantern "), None)),
            new_chapter: false,
            adult: false,
            facts: Vec::new(),
            demo: false,
            not_in_sources: false,
            origin: None,
            amount: None,
            started: None,
        };
        no_title_again(&mut tile);
        assert_eq!(tile.next, None);

        // A part of its own name, or the title with how far in, still says something.
        let named = words::part_place("3", Some("The Long Night"), None);
        tile.next = Some(named.clone());
        no_title_again(&mut tile);
        assert_eq!(tile.next, Some(named));
        let far = words::part_place("Rooftop Lantern", Some("Rooftop Lantern"), Some(40));
        tile.next = Some(far.clone());
        no_title_again(&mut tile);
        assert_eq!(tile.next, Some(far));
    }
}
