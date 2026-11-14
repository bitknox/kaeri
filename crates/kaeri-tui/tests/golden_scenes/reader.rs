//! The reader: a chapter opening, pages one or two at a time, the page between chapters and
//! what it says when there is no next one, pages on their way or failed, the key reference, and
//! the windows too small to read in.
//!
//! The board's kitty screens are drawn with an engine that lays pages out as a graphics
//! terminal would, at a 10 by 20 cell, and draws the pictures through halfblocks, so the golden
//! holds them as blocks. The pages are solid colours, one to a page.

use std::time::Duration;

use image::{DynamicImage, Rgb, RgbImage};
use kaeri_core::{
    books::{ChapterId, WorkId},
    media::Medium,
};
use kaeri_model::{
    Msg, ReadingMsg,
    book_sources::WriteBackResult,
    eink::NovelSupport,
    event::Event,
    reader::{
        Leaf, LeafPart, NoNext, NovelOpenView, NovelPos, NovelSpineItem, NovelTextDoc,
        NovelTocEntry, PageEncoding, PageFile, PageKind, ReadDirection, ReadPlace,
        ReaderAction, ReaderEvent, ReaderMode, ReaderNotice, ReaderSettingKey,
        ReaderSettingRow, ReaderSettingsView, ReaderValue, ReaderView, ReaderWait,
        SettingScope, TextBlock, TextBlockKind, TextRun, UnitRect, WritingMode,
    },
    reading::{
        ChapterDownloadState, ChapterReadState, ChapterRow, ReadingAbout, ReadingEvent,
        ReadingFailure,
    },
};
use kaeri_tui::{
    image::{ImageEngine, ImageStore},
    nav::StageView,
};

use super::{Screen, series_scenes::series_page};

const SESSION: u64 = 1;
const PAGES: u32 = 22;
const TITLE: &str = "Harbor Light Recipes";

/// The chapters the scenes read, by number, with their titles.
fn title_of(n: &str) -> Option<&'static str> {
    Some(match n {
        "14" => "Tide Tables",
        "15" => "The Night Market",
        "16" => "The Lantern Fair",
        "16.5" => "Side story",
        "17" => "Salt and Paper",
        _ => return None,
    })
}

fn row(n: &str, pages: u32, kept: bool) -> ChapterRow {
    ChapterRow {
        id: ChapterId(format!("c{n}")),
        label: format!("Chapter {n}"),
        state: ChapterReadState::InProgress { page: 7, of: Some(pages) },
        number: Some(n.into()),
        title: title_of(n).map(Into::into),
        volume: None,
        published: None,
        pages: Some(pages),
        new_chapter: false,
        download: kept.then_some(ChapterDownloadState::Done),
        versions: 1,
        bookmarked: false,
        version_label: None,
        read_at: None,
        line: Vec::new(),
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

/// The series page from Home, with the cursor `up` chapters above chapter 14, and the reader
/// opened on that chapter, drawn by `engine`. Returns the series.
fn open(s: &mut Screen, engine: ImageEngine, up: usize) -> WorkId {
    s.app.images = ImageStore::new(engine);
    series_page(s);
    for _ in 0..up {
        s.press(&["up"]);
    }
    s.press(&["enter"]);
    match s.app.nav.current() {
        StageView::Reader(work) => work.clone(),
        other => panic!("the reader, not {other:?}"),
    }
}

/// The engine's word that chapter `n` opened, at `page` (from 0).
fn opened(s: &mut Screen, work: &WorkId, n: &str, page: u32) {
    let view = ReaderView {
        session: SESSION,
        work: work.clone(),
        title: TITLE.into(),
        medium: Medium::Manga,
        chapter: row(n, PAGES, true),
        mode: ReaderMode::Paged,
        direction: ReadDirection::RightToLeft,
        start: ReadPlace {
            work: work.clone(),
            chapter: ChapterId(format!("c{n}")),
            page: Some(page),
            strip: None,
            novel: None,
        },
        next: None,
        previous: None,
        settings: None,
        demo_credit: None,
        progress_host: None,
    };
    s.apply(Event::Reader(ReaderEvent::Opened(Box::new(view))));
}

fn part(page: u32, x: f32, width: f32) -> LeafPart {
    LeafPart {
        page,
        kind: PageKind::Story,
        file: None,
        place: UnitRect { x, y: 0.0, width, height: 1.0 },
        rotate: false,
        half: None,
        ratio: Some(2.0 / 3.0),
        strip: None,
    }
}

/// The chapter's leaves as the engine lays them out for the surface: the first page alone,
/// then two at a time with the first on the right where two fit, else one at a time; then the
/// end page.
fn leaves(s: &mut Screen, n: &str, two: bool) {
    let mut leaves = vec![Leaf { index: 0, parts: vec![part(0, 0.0, 1.0)], end_page: false }];
    let mut page = 1;
    while page < PAGES {
        let parts = if two && page + 1 < PAGES {
            vec![part(page, 0.5, 0.5), part(page + 1, 0.0, 0.5)]
        } else {
            vec![part(page, 0.0, 1.0)]
        };
        page += parts.len() as u32;
        leaves.push(Leaf { index: leaves.len() as u32, parts, end_page: false });
    }
    leaves.push(Leaf { index: leaves.len() as u32, parts: Vec::new(), end_page: true });
    s.apply(Event::Reader(ReaderEvent::Leaves {
        session: SESSION,
        chapter: ChapterId(format!("c{n}")),
        leaves,
    }));
}

/// Whether the surface sent says two pages fit.
fn two_fit(s: &Screen) -> bool {
    s.app.reader.as_ref().and_then(|r| r.surface).is_some_and(|surface| surface.two_pages)
}

/// The file the engine made for `page`.
fn file(page: u32) -> PageFile {
    PageFile {
        path: format!("/pages/c14/{page}.png"),
        encoding: PageEncoding::Original { mime: "image/png".into() },
        width: 800,
        height: 1200,
        crop: None,
        colour: true,
        bytes: 1,
    }
}

fn page_ready(s: &mut Screen, page: u32) {
    s.apply(Event::Reader(ReaderEvent::PageReady {
        session: SESSION,
        chapter: ChapterId("c14".into()),
        page,
        half: None,
        file: file(page),
    }));
}

/// A solid page, its colour from its number.
fn picture(seed: u32) -> DynamicImage {
    let colour = Rgb([
        (40 + seed * 37 % 200) as u8,
        (90 + seed * 53 % 150) as u8,
        (60 + seed * 23 % 180) as u8,
    ]);
    DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 60, colour))
}

/// Decode every page file the reader handed out, as the binary does off the loop.
fn decode(s: &mut Screen) {
    for job in s.app.take_page_jobs() {
        let seed = job.key.bytes().map(u32::from).sum::<u32>();
        s.app.page_decoded(&job.key, Some(picture(seed)));
    }
    s.draw();
}

/// Chapter 14 open at page 8, its pages in: two at a time where they fit.
fn reading(s: &mut Screen, engine: ImageEngine) -> WorkId {
    let work = open(s, engine, 0);
    opened(s, &work, "14", 7);
    let two = two_fit(s);
    leaves(s, "14", two);
    for page in 0..PAGES {
        page_ready(s, page);
    }
    decode(s);
    work
}

fn resumed(s: &mut Screen) {
    s.apply(Event::Reader(ReaderEvent::Notice(ReaderNotice {
        msg: Msg::Reading(ReadingMsg::ReaderResumed { page: 8 }),
        action: Some(ReaderAction::StartOver),
        lasts_ms: 6_000,
    })));
}

/// The open on its way at 6 s, getting chapter 14 from the series' source.
pub(super) fn reader_opening(s: &mut Screen) {
    open(s, ImageEngine::drawn_as_blocks(), 0);
    s.apply(Event::Reader(ReaderEvent::Opening {
        chapter: Some(row("14", PAGES, true)),
        wait: Some(ReaderWait::ChapterPages {
            chapter: "14".into(),
            source: "Home library".into(),
        }),
    }));
    s.wait(Duration::from_secs(6));
}

/// Two pages, right to left, with an image protocol, and the resumed notice.
pub(super) fn reader_two_pages(s: &mut Screen) {
    reading(s, ImageEngine::drawn_as_blocks());
    resumed(s);
}

/// Two pages in halfblocks: the status line says Blocks, the status rule says it once.
pub(super) fn reader_two_pages_blocks(s: &mut Screen) {
    reading(s, ImageEngine::halfblocks());
}

/// The resumed notice first, and the blocks notice once it has gone.
pub(super) fn reader_blocks_after_resumed(s: &mut Screen) {
    reading(s, ImageEngine::halfblocks());
    resumed(s);
    s.wait(Duration::from_secs(7));
    s.idle(1);
}

/// One page at a time, as `v` keeps it.
pub(super) fn reader_one_page(s: &mut Screen) {
    let work = open(s, ImageEngine::drawn_as_blocks(), 0);
    opened(s, &work, "14", 7);
    leaves(s, "14", false);
    for page in 0..PAGES {
        page_ready(s, page);
    }
    decode(s);
}

/// Started with --no-images: manga says why there are no pages.
pub(super) fn reader_images_off(s: &mut Screen) {
    let work = open(s, ImageEngine::disabled(), 0);
    opened(s, &work, "14", 7);
    leaves(s, "14", false);
}

/// Page 9 not in yet past 600 ms: its plate says it is loading.
pub(super) fn reader_page_loading(s: &mut Screen) {
    let work = open(s, ImageEngine::drawn_as_blocks(), 0);
    opened(s, &work, "14", 7);
    leaves(s, "14", false);
    page_ready(s, 7);
    decode(s);
    s.press(&["left"]);
    s.wait(Duration::from_secs(1));
}

/// Page 9 failed: why, in its place, with `r` to ask again.
pub(super) fn reader_page_failed(s: &mut Screen) {
    reader_page_loading(s);
    s.apply(Event::Reader(ReaderEvent::PageFailed {
        session: SESSION,
        chapter: ChapterId("c14".into()),
        page: 8,
        failure: ReadingFailure::PageFetch { status: 503 },
    }));
}

/// Chapter 16 could not open.
pub(super) fn reader_chapter_failed(s: &mut Screen) {
    open(s, ImageEngine::drawn_as_blocks(), 2);
    s.wait(Duration::from_secs(1));
    s.apply(Event::Reading(ReadingEvent::Failed {
        about: ReadingAbout::Reader,
        failure: ReadingFailure::SourceDown { source: "Home library".into() },
    }));
}

/// To the end page: the last page, then one more turn.
fn to_the_end(s: &mut Screen) {
    s.press(&["G", " "]);
}

/// The engine's word on the end page, with the read counted by `trackers`.
fn end(s: &mut Screen, work: &WorkId, n: &str, end: EndOf) {
    s.apply(Event::Reader(ReaderEvent::EndPage {
        session: SESSION,
        next_source: end.next.as_ref().map(|_| "Home library".to_owned()),
        next: end.next,
        why_none: end.why_none,
        skip_to: end.skip_to,
        extras: u32::from(end.extras),
    }));
    if let Some(trackers) = end.trackers {
        s.apply(Event::Reader(ReaderEvent::ChapterRead {
            work: work.clone(),
            chapter: ChapterId(format!("c{n}")),
            trackers: trackers.iter().map(|t| (*t).to_owned()).collect(),
        }));
    }
}

#[derive(Default)]
struct EndOf {
    next: Option<ChapterRow>,
    why_none: Option<NoNext>,
    skip_to: Option<ChapterRow>,
    extras: bool,
    trackers: Option<&'static [&'static str]>,
}

/// The end of chapter 14: counted as read, saved to the book server, and chapter 15 next with
/// its first page.
pub(super) fn reader_end_page(s: &mut Screen) {
    let work = reading(s, ImageEngine::drawn_as_blocks());
    to_the_end(s);
    let next = row("15", PAGES, true);
    end(
        s,
        &work,
        "14",
        EndOf { next: Some(next.clone()), trackers: Some(&["AniList"]), ..EndOf::default() },
    );
    s.apply(Event::Reader(ReaderEvent::ProgressSent {
        work: work.clone(),
        chapter: ChapterId("c14".into()),
        server: "Home library".into(),
        host: "books.home.example".into(),
        result: WriteBackResult::Saved,
    }));
    s.apply(Event::Reader(ReaderEvent::NextPreview {
        session: SESSION,
        chapter: next.id.clone(),
        file: file(0),
    }));
    decode(s);
}

/// A chapter at its end, the engine's word on it applied.
fn ended(s: &mut Screen, n: &str, what: EndOf) {
    let work = open(s, ImageEngine::drawn_as_blocks(), 0);
    opened(s, &work, n, 0);
    leaves(s, n, false);
    to_the_end(s);
    end(s, &work, n, what);
}

/// Caught up, with the tracker offline.
pub(super) fn reader_end_caught_up(s: &mut Screen) {
    s.apply(Event::Connectivity { offline: true });
    ended(
        s,
        "17",
        EndOf {
            why_none: Some(NoNext::CaughtUp),
            trackers: Some(&["AniList"]),
            ..EndOf::default()
        },
    );
}

/// The next chapter isn't out yet.
pub(super) fn reader_end_not_out(s: &mut Screen) {
    ended(s, "17", EndOf { why_none: Some(NoNext::NotOutYet), ..EndOf::default() });
}

/// Offline, and the next chapter isn't downloaded.
pub(super) fn reader_end_offline(s: &mut Screen) {
    ended(s, "15", EndOf { why_none: Some(NoNext::Offline), ..EndOf::default() });
}

/// The next chapter is an extra, with the chapter after it to skip to.
pub(super) fn reader_end_extras(s: &mut Screen) {
    let mut extra = row("16.5", 9, false);
    extra.label = "Chapter 16.5".into();
    ended(
        s,
        "16",
        EndOf {
            next: Some(extra),
            skip_to: Some(row("17", PAGES, false)),
            extras: true,
            ..EndOf::default()
        },
    );
}

/// The end of a finished series.
pub(super) fn reader_end_finished(s: &mut Screen) {
    ended(
        s,
        "17",
        EndOf {
            why_none: Some(NoNext::Finished),
            trackers: Some(&["AniList"]),
            ..EndOf::default()
        },
    );
}

/// `?` in the reader: its keys first, then the global ones, and how the arrows go.
pub(super) fn reader_help(s: &mut Screen) {
    reading(s, ImageEngine::drawn_as_blocks());
    s.press(&["?"]);
}

/// The smallest comfortable page.
pub(super) fn reader_small_page(s: &mut Screen) {
    reader_one_page(s);
}

/// Below 40 by 12.
pub(super) fn reader_too_small(s: &mut Screen) {
    reader_one_page(s);
}

/// A page in bands, so the part of it in view shows in the golden: a colour a band, darker on
/// one half, from its seed.
fn banded(seed: u32, (width, height): (u32, u32)) -> DynamicImage {
    let bands = 8;
    DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
        let band = seed + y * bands / height;
        let shade = if x < width / 2 { 0 } else { 40 };
        Rgb([
            (40 + band * 37 % 160 + shade) as u8,
            (60 + band * 53 % 140 + shade) as u8,
            (50 + band * 23 % 150 + shade) as u8,
        ])
    }))
}

/// Decode every page file handed out as a banded page, `size` for each key.
fn decode_banded(s: &mut Screen, size: impl Fn(&str) -> (u32, u32)) {
    for job in s.app.take_page_jobs() {
        let seed = job.key.bytes().map(u32::from).sum::<u32>() % 17;
        let picture = banded(seed, size(&job.key));
        s.app.page_decoded(&job.key, Some(picture));
    }
    s.draw();
}

/// Chapter 14 open at page 8, one page at a time, its banded pages in.
fn reading_one(s: &mut Screen) -> WorkId {
    let work = open(s, ImageEngine::drawn_as_blocks(), 0);
    opened(s, &work, "14", 7);
    leaves(s, "14", false);
    for page in 0..PAGES {
        page_ready(s, page);
    }
    decode_banded(s, |_| (60, 90));
    work
}

/// `f`: page 8 at fit width, from its top, with the bar on the right.
pub(super) fn reader_fit_width(s: &mut Screen) {
    reading_one(s);
    s.press(&["f"]);
}

/// `z` and `+` on page 9: 300%, moved to its left edge and a third of the way down, with the
/// map at the right.
pub(super) fn reader_zoom(s: &mut Screen) {
    reading_one(s);
    s.press(&["left", "z", "+", "h"]);
    for _ in 0..10 {
        s.press(&["j"]);
    }
}

/// Two pages in a wide terminal, with `v` to keep one.
pub(super) fn reader_two_pages_wide(s: &mut Screen) {
    reading(s, ImageEngine::drawn_as_blocks());
}

/// Page 8 printed as a spread, whole across both slots, and Enter for a bare page.
pub(super) fn reader_spread_bare(s: &mut Screen) {
    let work = open(s, ImageEngine::drawn_as_blocks(), 0);
    opened(s, &work, "14", 7);
    let mut leaves: Vec<Leaf> = (0..PAGES)
        .map(|page| {
            let mut part = part(page, 0.0, 1.0);
            if page == 7 {
                part.kind = PageKind::Spread;
                part.ratio = Some(4.0 / 3.0);
            }
            Leaf { index: page, parts: vec![part], end_page: false }
        })
        .collect();
    leaves.push(Leaf { index: PAGES, parts: Vec::new(), end_page: true });
    s.apply(Event::Reader(ReaderEvent::Leaves {
        session: SESSION,
        chapter: ChapterId("c14".into()),
        leaves,
    }));
    for page in 0..PAGES {
        page_ready(s, page);
    }
    decode_banded(s, |key| if key.ends_with("/7") { (120, 90) } else { (60, 90) });
    s.press(&["enter"]);
}

/// The webtoon's title, and the chapters a strip runs through.
const WEBTOON: &str = "Lanterns Upriver";

fn web_row(n: &str, images: u32) -> ChapterRow {
    let mut row = row(n, images, false);
    row.title = match n {
        "14" => Some("Paper Boats".into()),
        "15" => Some("The Ferry at Dusk".into()),
        _ => None,
    };
    row
}

/// The engine's word that webtoon chapter `n` opened as a strip, with `next` after it.
fn web_opened(s: &mut Screen, work: &WorkId, n: &str, images: u32, next: Option<ChapterRow>) {
    let view = ReaderView {
        session: SESSION,
        work: work.clone(),
        title: WEBTOON.into(),
        medium: Medium::Manga,
        chapter: web_row(n, images),
        mode: ReaderMode::Strip,
        direction: ReadDirection::TopToBottom,
        start: ReadPlace {
            work: work.clone(),
            chapter: ChapterId(format!("c{n}")),
            page: None,
            strip: None,
            novel: None,
        },
        next,
        previous: None,
        settings: None,
        demo_credit: None,
        progress_host: None,
    };
    s.apply(Event::Reader(ReaderEvent::Opened(Box::new(view))));
}

/// A strip chapter's images, one to a leaf, then the end page; and each image's file.
fn web_images(s: &mut Screen, n: &str, images: u32) {
    let chapter = ChapterId(format!("c{n}"));
    let mut leaves: Vec<Leaf> = (0..images)
        .map(|image| {
            let mut part = part(image, 0.0, 1.0);
            part.ratio = Some(0.6);
            Leaf { index: image, parts: vec![part], end_page: false }
        })
        .collect();
    leaves.push(Leaf { index: images, parts: Vec::new(), end_page: true });
    s.apply(Event::Reader(ReaderEvent::Leaves {
        session: SESSION,
        chapter: chapter.clone(),
        leaves,
    }));
    for page in 0..images {
        let mut file = file(page);
        file.path = format!("/pages/{}/{page}.png", chapter.0);
        s.apply(Event::Reader(ReaderEvent::PageReady {
            session: SESSION,
            chapter: chapter.clone(),
            page,
            half: None,
            file,
        }));
    }
    decode_banded(s, |_| (60, 100));
}

/// A webtoon run on from chapter 14 into 15: the foot of 14's last image above the divider, 15's
/// first image under it, and the status rule saying 14 was counted.
pub(super) fn reader_strip(s: &mut Screen) {
    let work = open(s, ImageEngine::drawn_as_blocks(), 0);
    let next = web_row("15", 64);
    web_opened(s, &work, "14", 6, Some(next.clone()));
    s.apply(Event::Reader(ReaderEvent::EndPage {
        session: SESSION,
        next: Some(next),
        why_none: None,
        next_source: Some("Northwind".into()),
        skip_to: None,
        extras: 0,
    }));
    web_images(s, "14", 6);
    s.press(&["n"]);
    s.apply(Event::Reader(ReaderEvent::ChapterRead {
        work: work.clone(),
        chapter: ChapterId("c14".into()),
        trackers: vec!["AniList".into()],
    }));
    web_opened(s, &work, "15", 64, Some(web_row("16", 58)));
    web_images(s, "15", 64);
    s.press(&["k", "k", "k", "k"]);
}

/// The light novel's series.
const NOVEL_TITLE: &str = "Lantern Street Notes";

/// The board's paragraphs, with its emphasis between stars, then more of the same evening.
const NOVEL_TEXT: &[&str] = &[
    "The lamp room sat at the top of the stairs, past the door that stuck in wet weather. \
     Mio had learned to lift the handle before she turned it, the way her aunt did, and the \
     door gave with a sigh that sounded almost polite.",
    "Inside, the shelves held every lantern the street had ever sent back for repair. Some \
     were labelled in her aunt's narrow hand. Others had only a date and a house number, as \
     if the lamp itself would explain the rest once it was lit.",
    "\"You don't have to fix them all tonight,\" her aunt said from the doorway.",
    "\"I know.\" Mio set the first one on the bench. Its glass was clouded, and a crack ran \
     from the base to the hook like a river on a map. \"I want to see which ones still \
     *remember* how.\"",
    "Her aunt laughed, then stopped, because Mio had not meant it as a joke. Outside, the \
     rain kept time on the gutter. Down the street someone was singing the closing song from \
     the market, a little behind the beat, and every so often a window answered with light.",
    "She worked until the candle in the first lantern stood straight. Then she wrote the \
     house number on a card, and under it, in her own hand, the word *ready*.",
    "The second lantern was older. Its hinge had been mended with wire, and someone had \
     scratched a small star into the brass where the thumb rests. Mio turned it over twice \
     before she found the date, faint as breath on a window.",
    "By the time the rain eased, there were four cards on the bench and the room smelled of \
     wax and wet wool. Her aunt brought tea and did not ask how many were left.",
    "They drank it standing at the window, watching the street below take its lamps back one \
     by one, each house a little brighter than the last.",
];

/// A paragraph's runs, its words between stars in italics, with their offsets in the text.
fn novel_runs(path: u32, text: &str) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let mut offset = 0;
    for (i, part) in text.split('*').enumerate() {
        if part.is_empty() {
            continue;
        }
        runs.push(TextRun {
            text: part.into(),
            offset,
            path: vec![path],
            emphasis: i % 2 == 1,
            strong: false,
            ruby: false,
        });
        offset += part.encode_utf16().count() as u32;
    }
    runs
}

fn novel_block(kind: TextBlockKind, spine: u32, path: u32, text: &str) -> TextBlock {
    TextBlock {
        kind,
        at: NovelPos { spine, path: vec![path], offset: 0 },
        runs: novel_runs(path, text),
    }
}

fn novel_setting(key: ReaderSettingKey, value: ReaderValue) -> ReaderSettingRow {
    let scope = if key.syncs() { SettingScope::Medium } else { SettingScope::Device };
    ReaderSettingRow {
        key,
        value,
        scope,
        detected: false,
        overridden: false,
        choices: Vec::new(),
    }
}

/// The settings a terminal novel opens with: paged, 66 columns, tight and ragged.
fn novel_settings(work: &WorkId) -> ReaderSettingsView {
    ReaderSettingsView {
        medium: Medium::Novel,
        work: Some(work.clone()),
        rows: vec![
            novel_setting(ReaderSettingKey::Mode, ReaderValue::Choice { name: "paged".into() }),
            novel_setting(ReaderSettingKey::TextWidth, ReaderValue::Amount { value: 66.0 }),
            novel_setting(ReaderSettingKey::LineSpacing, ReaderValue::Amount { value: 1.4 }),
            novel_setting(ReaderSettingKey::Justify, ReaderValue::Toggle { on: false }),
        ],
        tap_zones: None,
    }
}

/// The novel open on its chapter "The Lamp Room", at `restore`: the heading, the board's
/// paragraphs, then a second part of the chapter.
fn novel(s: &mut Screen, restore: Option<NovelPos>) {
    let work = open(s, ImageEngine::drawn_as_blocks(), 0);
    let mut chapter = row("3", 1, true);
    chapter.label = "Part 3".into();
    chapter.title = Some("The Lamp Room".into());
    let view = ReaderView {
        session: SESSION,
        work: work.clone(),
        title: NOVEL_TITLE.into(),
        medium: Medium::Novel,
        chapter,
        mode: ReaderMode::Paged,
        direction: ReadDirection::LeftToRight,
        start: ReadPlace {
            work: work.clone(),
            chapter: ChapterId("c3".into()),
            page: None,
            strip: None,
            novel: restore.clone(),
        },
        next: Some(row("4", 1, true)),
        previous: None,
        settings: Some(novel_settings(&work)),
        demo_credit: None,
        progress_host: None,
    };
    s.apply(Event::Reader(ReaderEvent::Opened(Box::new(view))));
    let mut first =
        vec![novel_block(TextBlockKind::Heading { level: 2 }, 0, 0, "The Lamp Room")];
    first.extend(
        NOVEL_TEXT
            .iter()
            .enumerate()
            .map(|(i, text)| novel_block(TextBlockKind::Paragraph, 0, i as u32 + 1, text)),
    );
    let second = NOVEL_TEXT
        .iter()
        .enumerate()
        .map(|(i, text)| novel_block(TextBlockKind::Paragraph, 1, i as u32, text))
        .collect();
    let spine = |n: u32| NovelSpineItem {
        url: format!("kaeri-book://s1/text/{n}.xhtml"),
        linear: true,
        words: 2000,
    };
    let entry = |label: &str, depth: u32| NovelTocEntry {
        label: label.into(),
        url: "kaeri-book://s1/text/0.xhtml".into(),
        spine: 0,
        depth,
    };
    s.apply(Event::Reader(ReaderEvent::NovelOpen(NovelOpenView {
        session: SESSION,
        chapter: ChapterId("c3".into()),
        entry: "kaeri-book://s1/text/0.xhtml".into(),
        writing_mode: WritingMode::Horizontal,
        progression: ReadDirection::LeftToRight,
        restore,
        fixed_layout: false,
        support: NovelSupport::Full,
        spine: vec![spine(0), spine(1)],
        toc: vec![entry("Part 3", 0), entry("The Lamp Room", 1)],
        style: String::new(),
        count_key: String::new(),
        counted: Vec::new(),
        render_dir: None,
        words_per_minute: 230,
        text: vec![
            NovelTextDoc { spine: 0, blocks: first },
            NovelTextDoc { spine: 1, blocks: second },
        ],
    })));
}

/// A light novel from the top of its chapter, in a column 66 wide.
pub(super) fn reader_novel(s: &mut Screen) {
    novel(s, None);
}

/// A small window: the column takes the stage less two each side, from the first paragraph.
pub(super) fn reader_novel_small(s: &mut Screen) {
    novel(s, Some(NovelPos { spine: 0, path: vec![1], offset: 0 }));
}

/// `,` and the text narrowed to 50 columns from the overlay, with the preview beside it.
pub(super) fn reader_novel_settings(s: &mut Screen) {
    novel(s, None);
    s.press(&[",", "j", "left", "left", "left", "left"]);
}

/// The key reference over a novel: the width keys, and no zoom.
pub(super) fn reader_novel_help(s: &mut Screen) {
    novel(s, None);
    s.press(&["?"]);
}

/// Manga's settings: its own rows, a series' own reading direction on the cursor.
pub(super) fn reader_manga_settings(s: &mut Screen) {
    let work = reading_one(s);
    let choice = |name: &str| ReaderValue::Choice { name: name.into() };
    let mut rows = vec![
        novel_setting(ReaderSettingKey::Mode, choice("paged")),
        novel_setting(ReaderSettingKey::Direction, choice("rtl")),
        novel_setting(ReaderSettingKey::Fit, choice("screen")),
        novel_setting(ReaderSettingKey::PageLayout, choice("auto")),
        novel_setting(ReaderSettingKey::Spreads, choice("split")),
        novel_setting(ReaderSettingKey::CropBorders, ReaderValue::Toggle { on: false }),
        novel_setting(ReaderSettingKey::StripWidth, ReaderValue::Amount { value: 0.6 }),
        novel_setting(ReaderSettingKey::SkipCreditPages, ReaderValue::Toggle { on: true }),
    ];
    rows[1].scope = SettingScope::Series;
    rows[1].overridden = true;
    s.press(&[","]);
    s.apply(Event::Reader(ReaderEvent::Settings(ReaderSettingsView {
        medium: Medium::Manga,
        work: Some(work),
        rows,
        tap_zones: None,
    })));
    s.press(&["j"]);
}

/// A scene, by name, at a size of its own.
pub(super) type BoardScene = (&'static str, fn(&mut Screen), (u16, u16));

/// The scenes the board draws at a size of its own, and that size.
pub(super) const BOARD: &[BoardScene] = &[
    ("reader_end_caught_up", reader_end_caught_up, (64, 15)),
    ("reader_end_not_out", reader_end_not_out, (64, 15)),
    ("reader_end_offline", reader_end_offline, (64, 15)),
    ("reader_end_extras", reader_end_extras, (64, 15)),
    ("reader_end_finished", reader_end_finished, (64, 15)),
    ("reader_small_page", reader_small_page, (60, 18)),
    ("reader_too_small", reader_too_small, (36, 10)),
    ("reader_two_pages_wide", reader_two_pages_wide, (200, 50)),
    ("reader_spread_bare", reader_spread_bare, (200, 50)),
    ("reader_novel_small", reader_novel_small, (60, 20)),
];
