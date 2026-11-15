//! The series page: a reading series' header and its chapters, online, offline, while its
//! chapters are on their way, and when the page could not be had.

use std::time::Duration;

use kaeri_core::{
    books::{ChapterId, WorkId},
    media::Medium,
};
use kaeri_model::{
    Msg, ReadingMsg,
    event::Event,
    reading::{
        BookEditionRow, ChapterDownloadState, ChapterReadState, ChapterRow, LibrarySegmentRead,
        ReadingAbout, ReadingEvent, ReadingFailure, SeriesAction, SeriesView, TrackerMapping,
        TrackerReadRow, VolumeGroup, WorkTitles, kind_words, status_words,
    },
};

use super::{Screen, read, reading_on};

/// The made-up chapters, oldest first as the engine lists them: their titles, and whether this
/// computer has them.
const CHAPTERS: [(&str, bool); 17] = [
    ("Driftwood Stock", false),
    ("The First Lamp", false),
    ("Bread for the Fog", false),
    ("Galley Hours", false),
    ("A Map of Small Fires", false),
    ("Brine and Butter", false),
    ("The Keeper's Ledger", false),
    ("Shore Leave", false),
    ("What the Gulls Took", false),
    ("Rain on the Pier", false),
    ("Low Tide", false),
    ("Borrowed Knives", false),
    ("A Kettle for Two", true),
    ("Tide Tables", true),
    ("The Night Market", true),
    ("The Lantern Fair", false),
    ("Salt and Paper", false),
];

const DAY: i64 = 86_400;

fn edition() -> BookEditionRow {
    BookEditionRow {
        source_id: "home-library".into(),
        source_name: "Home library".into(),
        language: None,
        version_label: None,
        chapters: 17,
        latest: Some("17".into()),
        preferred: true,
        in_use: true,
        listing: None,
        groups: Vec::new(),
        group_chosen: false,
    }
}

fn view(work: &WorkId) -> SeriesView {
    let r = Msg::Reading;
    SeriesView {
        work: work.clone(),
        medium: Medium::Manga,
        titles: WorkTitles {
            display: "Harbor Light Recipes".into(),
            romaji: None,
            english: None,
            native: None,
            synonyms: Vec::new(),
        },
        action: SeriesAction::Continue {
            chapter: ChapterId("c14".into()),
            label: "14".into(),
            page: 8,
        },
        cover: None,
        banner: None,
        format: Some("manga".into()),
        origin: Some("JP".into()),
        status: Some("releasing".into()),
        synopsis: Some(
            "A line cook inherits a lighthouse that runs a night kitchen for ships nobody else \
             can see. Every recipe in it was written for a storm."
                .into(),
        ),
        genres: Vec::new(),
        authors: Vec::new(),
        chapters_total: None,
        volumes_total: None,
        edition: Some(edition()),
        list: Some(LibrarySegmentRead::Reading),
        score: None,
        tracking: Vec::new(),
        other_medium: None,
        ask_where_stopped: None,
        rhythm: Some(r(ReadingMsg::ReadingUsuallyFridays)),
        facts: [
            Some(kind_words(Some("manga"), Medium::Manga, Some("JP"))),
            status_words("releasing"),
        ]
        .into_iter()
        .flatten()
        .collect(),
        year: None,
        adult: false,
        recommended: Vec::new(),
        partial: false,
        demo: None,
        demo_other: None,
    }
}

/// Chapter `n`, counted from 1, with its line as the engine words it.
fn chapter(n: usize, title: &str, kept: bool) -> ChapterRow {
    let r = Msg::Reading;
    let ago = |days: i64| {
        r(match days {
            1 => ReadingMsg::ReadingAgoYesterday,
            2..7 => ReadingMsg::ReadingAgoDays { days },
            _ => ReadingMsg::ReadingAgoWeeks { weeks: days / 7 },
        })
    };
    // Chapter 13 read 4 days ago, and one a week before each of the others.
    let read_days = 4 + 7 * (13 - n as i64);
    let (state, new_chapter, mut line) = match n {
        1..=13 => {
            (ChapterReadState::Read, false, vec![r(ReadingMsg::ReadingRowRead), ago(read_days)])
        }
        14 => (
            ChapterReadState::InProgress { page: 8, of: Some(22) },
            false,
            vec![r(ReadingMsg::ReadingRowPageOf { page: 8, of: 22 })],
        ),
        16 => (ChapterReadState::Unread, false, vec![r(ReadingMsg::ReadingRowNew), ago(6)]),
        17 => (ChapterReadState::Unread, true, vec![r(ReadingMsg::ReadingRowNew), ago(1)]),
        _ => (ChapterReadState::Unread, false, Vec::new()),
    };
    if kept {
        line.push(r(ReadingMsg::ReadingRowDownloaded));
    } else if line.is_empty() {
        line.push(r(ReadingMsg::ReadingRowNotDownloaded));
    }
    ChapterRow {
        id: ChapterId(format!("c{n}")),
        label: format!("Chapter {n}"),
        state,
        number: Some(n.to_string()),
        title: Some(title.into()),
        volume: None,
        published: Some(super::NOW - DAY * (17 - n as i64)),
        pages: Some(22),
        new_chapter,
        download: kept.then_some(ChapterDownloadState::Done),
        versions: 1,
        bookmarked: false,
        version_label: None,
        read_at: (n <= 13).then(|| super::NOW - DAY * read_days),
        line,
        gone_from_source: false,
        parts: 0,
        missing_parts: Vec::new(),
        part_groups: Vec::new(),
        other_group: false,
        group_language: None,
        other_source: None,
        on_device: false,
    }
}

fn chapters(work: &WorkId, failure: Option<ReadingFailure>) -> Event {
    let rows = CHAPTERS
        .iter()
        .enumerate()
        .map(|(n, (title, kept))| chapter(n + 1, title, *kept))
        .collect();
    Event::Reading(ReadingEvent::Chapters {
        work: work.clone(),
        edition: Some(edition()),
        volumes: vec![VolumeGroup {
            volume: None,
            title: None,
            chapters: rows,
            all_read: false,
            flat: true,
        }],
        failure,
    })
}

fn tracking(work: &WorkId) -> Event {
    Event::Reading(ReadingEvent::Tracking {
        work: work.clone(),
        rows: vec![TrackerReadRow {
            tracker: "anilist".into(),
            chapters: 13,
            volumes: None,
            status: Some(LibrarySegmentRead::Reading),
            score: None,
            divergent: false,
            mapping: TrackerMapping::Mapped,
            split_note: None,
        }],
    })
}

/// Reading's Home with history, and `e` on the first title in Continue: its series page, asked
/// for. Returns the series.
fn opened(s: &mut Screen) -> WorkId {
    reading_on(s, 1);
    s.apply(read::home_history());
    s.press(&["e"]);
    match s.app.nav.current() {
        kaeri_tui::nav::StageView::Series(work) => work.clone(),
        other => panic!("the series page, not {other:?}"),
    }
}

/// The series page: header, where it is read and tracked, the meter, and the chapters newest
/// first with the cursor on the one in progress.
pub(super) fn series_page(s: &mut Screen) {
    let work = opened(s);
    s.apply(Event::Reading(ReadingEvent::Series(Box::new(view(&work)))));
    s.apply(tracking(&work));
    s.apply(chapters(&work, None));
}

/// Offline: the kept chapters, the ones not downloaded dimmed, the header's sources crossed out.
pub(super) fn series_offline(s: &mut Screen) {
    let work = opened(s);
    s.apply(Event::Reading(ReadingEvent::Series(Box::new(view(&work)))));
    s.apply(chapters(&work, Some(ReadingFailure::Offline)));
}

/// The header in, the chapters still on their way past 4 s: the placeholder and the source asked.
pub(super) fn series_chapters_loading(s: &mut Screen) {
    let work = opened(s);
    s.apply(Event::Reading(ReadingEvent::Series(Box::new(view(&work)))));
    s.apply(tracking(&work));
    s.wait(Duration::from_secs(5));
}

/// The page could not be had: why, and the key that asks again.
pub(super) fn series_failed(s: &mut Screen) {
    opened(s);
    s.wait(Duration::from_secs(1));
    s.apply(Event::Reading(ReadingEvent::Failed {
        about: ReadingAbout::Series,
        failure: ReadingFailure::SourceDown { source: "Home library".into() },
    }));
}
