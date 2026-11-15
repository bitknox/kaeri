//! The reader over the whole window: the header with the series and chapter, the page or pages
//! on the stage, the status rule with the reader's notices, and the status line with the meter,
//! the page count and the keys for the open mode.

use std::time::Instant;

use kaeri_i18n::t;
use kaeri_model::reader::{Leaf, NoNext, ReaderAction, ReaderWait};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use super::{chrome::header_chips, line, overlays::render_overlay};
use crate::{
    app::{
        App, CanvasBox, FrameReport, MIN_COLUMNS, MIN_ROWS, PLATE_AFTER, PartKey, Piece,
        Reader, ReaderScreen, SECONDS_AFTER, Viewport, ZOOM_PANEL, chapter_name, reader_hints,
        say, stage_of,
    },
    image::{Cut, Graphics},
    keymap::{Action, said},
    layout,
    nav::{Overlay, RailWidth},
    theme::{Role, glyph},
    widgets::{Cells, Hairline, StatusLine, truncate, wrap},
};

/// Where the header's title starts: after "Kaeri" and three spaces.
const TITLE_X: u16 = 8;
/// From this width the header names the chapter's title, and the status line the direction.
const WIDE: u16 = 100;
/// From this width the header keeps its chips and the status line its meter.
const ROOMY: u16 = 72;
/// The end page's column at full size, and the width its Next heading rules to.
const END_WIDTH: u16 = 60;
/// From this width the meter is 24 cells long.
const VERY_WIDE: u16 = 160;
/// The zoom's map at most this many rows tall, and this many columns wide.
const MAP_ROWS: u16 = 9;
const MAP_COLUMNS: u16 = 13;

/// Paint the reader over the whole window, and say the window's size for the surface.
pub(super) fn render_reader(buf: &mut Buffer, app: &App, area: Rect) -> FrameReport {
    let report =
        FrameReport { window: Some((area.width, area.height)), ..FrameReport::default() };
    // The screen follows the window this frame has, not the one the last frame had: a shrink
    // between frames must not lay out the old size.
    let window = (area.width, area.height);
    let (Some(reader), Some(screen)) = (app.reader_here(), app.reader_screen_at(window)) else {
        return report;
    };
    if screen == ReaderScreen::TooSmall {
        too_small(buf, app, reader, area);
        return report;
    }
    if app.nav.overlay() == Some(&Overlay::Help) {
        help(buf, app, reader, area);
        return report;
    }
    if app.nav.overlay() == Some(&Overlay::ReaderSettings) {
        super::reader_text::settings_overlay(buf, app, reader, area);
        return report;
    }
    let now = app.clock.now_instant();
    let bare = reader.bare && matches!(screen, ReaderScreen::Pages);
    let stage = stage_of(area, bare);
    match screen {
        ReaderScreen::Opening => opening(buf, app, reader, stage, now),
        ReaderScreen::ChapterFailed => chapter_failed(buf, app, reader, stage),
        ReaderScreen::ImagesOff => images_off(buf, app, stage),
        ReaderScreen::Pages if reader.novel.is_some() => {
            super::reader_text::novel_page(buf, app, reader, area);
        }
        ReaderScreen::Pages => pages(buf, app, reader, stage, now),
        ReaderScreen::End => end_page(buf, app, reader, stage),
        ReaderScreen::TooSmall => {}
    }
    if !bare {
        chrome(buf, app, reader, screen, area);
    }
    // Other overlays sit over the reader as they sit over any screen: the palette, the logs.
    let overlay = if app.nav.has_overlay() {
        render_overlay(buf, app, area, &layout::compute(area, RailWidth::Hidden))
    } else {
        FrameReport::default()
    };
    FrameReport { window: report.window, ..overlay }
}

/// The header, its rule, the status rule with a notice on it, and the status line.
fn chrome(buf: &mut Buffer, app: &App, reader: &Reader, screen: ReaderScreen, area: Rect) {
    let w = area.width;
    let (top, bottom) = (area.y, area.bottom());
    header(buf, app, reader, screen, area);
    Hairline::new(&app.palette).render(Rect { y: top + 1, height: 1, ..area }, buf);
    let rule = Rect { y: bottom - 2, height: 1, ..area };
    Hairline::new(&app.palette).render(rule, buf);
    if !notice(buf, app, rule) {
        super::chrome::render_toasts(buf, app, rule, area.x);
    }
    let left = status_left(app, reader, screen, w);
    status(buf, app, Rect { y: bottom - 1, height: 1, ..area }, &left);
}

/// "Kaeri", the series and the chapter, and the chips on the right while there is room.
pub(super) fn header(
    buf: &mut Buffer,
    app: &App,
    reader: &Reader,
    screen: ReaderScreen,
    area: Rect,
) {
    let p = &app.palette;
    let w = area.width;
    line(buf, area.x, area.y, "Kaeri", p.style(Role::Text).add_modifier(Modifier::BOLD)); // i18n-skip
    // The header's right side goes first: below a classic terminal's width the chips go.
    let chips = if w >= ROOMY { header_chips(app) } else { Vec::new() };
    let chips_text: Vec<(String, Role)> = chips
        .iter()
        .enumerate()
        .flat_map(|(i, (text, role))| {
            let sep = (i > 0).then(|| ("  ".to_owned(), Role::TextDim));
            sep.into_iter().chain([(text.clone(), *role)])
        })
        .collect();
    let chips_width: u16 = chips_text.iter().map(|(t, _)| t.cells() as u16).sum();
    let room = w.saturating_sub(TITLE_X + if chips_width > 0 { chips_width + 2 } else { 0 });
    let series = reader.series_title().to_owned();
    let chapter = reader.chapter();
    let novel = app.novel_progress().filter(|_| reader.novel.is_some());
    let chapter_words = match (chapter, w >= WIDE) {
        // A novel's part from its contents, with the chapter's title where there is room.
        _ if let Some(progress) = novel => match (w >= WIDE, progress.title) {
            (true, Some(title)) => format!("{} · {title}", progress.part), // i18n-skip: layout
            _ if w >= ROOMY => progress.part,
            _ => String::new(),
        },
        (Some(row), true) => chapter_name(row),
        (Some(row), false) => {
            t::reader_chapter_line(&row.number.clone().unwrap_or_else(|| row.label.clone()))
        }
        (None, _) => {
            reader.chapter_number().map(|n| t::reader_chapter_line(&n)).unwrap_or_default()
        }
    };
    let rest =
        if chapter_words.is_empty() { String::new() } else { format!(" · {chapter_words}") }; // i18n-skip: layout
    // Opening, and in a small window, the whole title is quiet.
    let strong = w >= ROOMY && screen != ReaderScreen::Opening;
    let whole = format!("{series}{rest}"); // i18n-skip: layout
    // Once the chips have gone the title keeps to half the header, so a small window's header
    // stays quiet over the page. The page between chapters is text, and keeps it whole.
    let room = if w < ROOMY && screen != ReaderScreen::End {
        room.min(w.saturating_sub(TITLE_X) / 2)
    } else {
        room
    };
    let room = room as usize;
    if strong && whole.cells() <= room {
        let x = TITLE_X + series.cells() as u16;
        line(buf, area.x + TITLE_X, area.y, &series, p.style(Role::Text));
        line(buf, area.x + x, area.y, &rest, p.style(Role::TextDim));
    } else {
        line(buf, area.x + TITLE_X, area.y, truncate(&whole, room), p.style(Role::TextDim));
    }
    let mut x = area.right().saturating_sub(chips_width);
    for (text, role) in chips_text {
        line(buf, x, area.y, &text, p.style(role));
        x += text.cells() as u16;
    }
}

/// The reader's notice on the status rule, as a toast sits there: the engine's words and the
/// key that acts on them. Says whether one was drawn.
fn notice(buf: &mut Buffer, app: &App, rule: Rect) -> bool {
    let Some(notice) = app.reader_notice() else { return false };
    let p = &app.palette;
    let mut runs: Vec<(String, Role)> = vec![(notice.text.clone(), Role::Text)];
    let act = notice.action.and_then(|action| {
        let (key, word) = match action {
            ReaderAction::StartOver => (Action::ReaderFirst, t::tui_reader_start_over()),
            ReaderAction::GoBack => (Action::ReaderBackToPage, t::tui_reader_go_back()),
            ReaderAction::ChangeMode => (Action::ReaderSettings, t::tui_reader_change()),
        };
        Some((app.reader_key_name(key)?, word))
    });
    if let Some((key, word)) = act {
        runs[0].0.push_str(" · ");
        runs.push((key, Role::Text));
        runs.push((format!(" {}", word.to_lowercase()), Role::TextDim));
    }
    let width: usize = runs.iter().map(|(t, _)| t.cells()).sum();
    // A gap, the obi and its space before; a space and one cell of rule after.
    let room = (rule.width as usize).saturating_sub(8);
    if room < 8 {
        return false;
    }
    if width > room {
        runs.truncate(1);
        runs[0].0 = truncate(&runs[0].0, room);
    }
    let width: u16 = runs.iter().map(|(t, _)| t.cells() as u16).sum();
    let x0 = rule.right().saturating_sub(width + 4);
    line(buf, x0.saturating_sub(1), rule.y, " ", p.style(Role::Text));
    line(buf, x0, rule.y, glyph::OBI.to_string(), p.style(Role::Obi));
    line(buf, x0 + 1, rule.y, " ", p.style(Role::Text));
    let mut x = x0 + 2;
    for (text, role) in &runs {
        line(buf, x, rule.y, text, p.style(*role));
        x += text.cells() as u16;
    }
    line(buf, x, rule.y, " ─", p.style(Role::Rule)); // i18n-skip: the rule's end
    true
}

/// The status line's left side for the screen up: the meter and the page count, or what the
/// screen is about.
fn status_left(
    app: &App,
    reader: &Reader,
    screen: ReaderScreen,
    w: u16,
) -> Vec<(String, Role)> {
    let count = page_count(reader);
    match screen {
        ReaderScreen::Opening => {
            let words = reader
                .chapter_number()
                .map_or_else(t::tui_reader_finding, |n| t::tui_reader_opening(&n));
            vec![(words, Role::TextDim)]
        }
        ReaderScreen::ChapterFailed => reader
            .chapter_number()
            .map(|n| vec![(t::reader_chapter_line(&n), Role::TextDim)])
            .unwrap_or_default(),
        ReaderScreen::ImagesOff | ReaderScreen::TooSmall => {
            count.map(|c| vec![(c, Role::TextDim)]).unwrap_or_default()
        }
        ReaderScreen::End => {
            if w < ROOMY {
                return Vec::new();
            }
            let number = reader.chapter_number().unwrap_or_default();
            let mut runs = meter(reader, 1.0, w);
            runs.push((
                format!("  {}", t::reader_end_of_chapter_number(&number)),
                Role::TextDim,
            ));
            runs
        }
        ReaderScreen::Pages if reader.novel.is_some() => {
            let roomy = w >= ROOMY;
            let words = app.novel_status_words(roomy).unwrap_or_default();
            if !roomy {
                return vec![(words, Role::TextDim)];
            }
            let fraction = app.novel_progress().map_or(0.0, |p| p.fraction);
            let mut runs = meter(reader, fraction, w);
            runs.push((format!("  {words}"), Role::TextDim)); // i18n-skip: layout
            runs
        }
        ReaderScreen::Pages if reader.strips() => strip_status(app, reader, w),
        ReaderScreen::Pages if reader.zoom.is_some() => {
            let level = reader.zoom.map_or(1.0, |z| z.level);
            let percent = (level * 100.0).round() as i64;
            let page = zoom_page(app, reader).map_or_else(String::new, |page| {
                t::tui_reader_page_of(i64::from(page) + 1, i64::from(reader.story_pages()))
            });
            let mut words = t::tui_reader_zoom(percent);
            if !page.is_empty() {
                words.push_str(&format!(" · {page}")); // i18n-skip: layout
            }
            vec![(blocks_said(app, words), Role::TextDim)]
        }
        ReaderScreen::Pages if reader.fits_side() => {
            let Some(count) = count else { return Vec::new() };
            let fit = if reader.fits_height() {
                t::tui_reader_fit_height()
            } else {
                t::tui_reader_fit_width()
            };
            let words = format!("{count} · {fit}"); // i18n-skip: layout
            if w < WIDE {
                return vec![(blocks_said(app, words), Role::TextDim)];
            }
            let pages = reader.story_pages().max(1);
            let last = reader.pages_in_view().map_or(0, |(a, b)| b.unwrap_or(a));
            let mut runs = meter(reader, f64::from(last) / f64::from(pages), w);
            runs.push((blocks_said(app, format!("  {words}")), Role::TextDim)); // i18n-skip: layout
            runs
        }
        ReaderScreen::Pages => {
            let Some(count) = count else { return Vec::new() };
            if w < ROOMY {
                return vec![(count, Role::TextDim)];
            }
            let pages = reader.story_pages().max(1);
            let last = reader.pages_in_view().map_or(0, |(a, b)| b.unwrap_or(a));
            let mut runs = meter(reader, f64::from(last) / f64::from(pages), w);
            let mut words = format!("  {count}"); // i18n-skip: layout
            if w >= WIDE && reader.rtl() {
                words.push_str(&format!(" · {}", t::reader_mode_rtl()));
            }
            if app.images.engine().graphics() == Graphics::Halfblocks {
                words.push_str(&format!(" · {}", t::tui_reader_blocks()));
            }
            runs.push((words, Role::TextDim));
            runs
        }
    }
}

/// "Page 8 of 22", or "Pages 8 and 9 of 22" with two in view.
fn page_count(reader: &Reader) -> Option<String> {
    let pages = i64::from(reader.story_pages());
    let (first, second) = reader.pages_in_view().or_else(|| {
        // The end page and a resize below the smallest window still say the last page.
        (pages > 0).then_some((pages as u32, None))
    })?;
    Some(match second {
        Some(second) => t::tui_reader_pages_of(i64::from(first), i64::from(second), pages),
        None => t::tui_reader_page_of(i64::from(first), pages),
    })
}

/// The meter, 16 cells wide in a wide window and 10 in a classic one, filling from the right in a
/// right-to-left chapter, as the page moves.
fn meter(reader: &Reader, fraction: f64, w: u16) -> Vec<(String, Role)> {
    let width = if w >= VERY_WIDE {
        24
    } else if w >= WIDE {
        16
    } else {
        10
    };
    let full = ((fraction.clamp(0.0, 1.0)) * f64::from(width as u32)) as usize;
    let mut cells = "█".repeat(full) + &"░".repeat(width - full); // i18n-skip: the meter
    // Text reads left to right here, whichever way the book's pages turn.
    if reader.rtl() && reader.novel.is_none() {
        cells = cells.chars().rev().collect();
    }
    vec![(cells, Role::Text)]
}

/// The status line: the left side as runs, cut to fit, and the hints on the right.
pub(super) fn status(buf: &mut Buffer, app: &App, area: Rect, left: &[(String, Role)]) {
    let hints = reader_hints(app, area.width as usize);
    StatusLine::new(&app.palette, "").hints(&hints).render(area, buf);
    let hints_width: usize =
        hints.iter().map(|(key, label)| key.cells() + 1 + label.cells()).sum::<usize>()
            + 3 * hints.len().saturating_sub(1);
    let mut room = (area.width as usize).saturating_sub(hints_width + 2);
    let mut x = area.x;
    // Runs are never cut mid-word: one that doesn't fit keeps the sentences that do, or goes
    // with every run after it.
    for (text, role) in left {
        let Some(text) = fitting_sentences(text, room) else { break };
        line(buf, x, area.y, &text, app.palette.style(*role));
        x += text.cells() as u16;
        room = room.saturating_sub(text.cells());
    }
}

/// `text` when it fits in `room` cells, else its leading whole sentences that do, else `None`.
fn fitting_sentences(text: &str, room: usize) -> Option<String> {
    if text.cells() <= room {
        return Some(text.to_owned());
    }
    let mut kept = None;
    for (i, _) in text.match_indices(". ") {
        let head = &text[..=i];
        if head.cells() > room {
            break;
        }
        kept = Some(head.to_owned());
    }
    kept.filter(|k| !k.trim().is_empty())
}

/// The open on its way: nothing for 600 ms, then what it waits on, with the seconds from 4 s.
fn opening(buf: &mut Buffer, app: &App, reader: &Reader, stage: Rect, now: Instant) {
    let Some(since) = reader.opening else { return };
    let waited = now.saturating_duration_since(since);
    if waited < PLATE_AFTER {
        return;
    }
    let number = reader.chapter_number();
    let words = match &reader.wait {
        Some(ReaderWait::FindingSource) => t::tui_reader_finding(),
        Some(ReaderWait::ChapterList { source }) => t::reader_wait_chapter_list(source),
        Some(ReaderWait::ChapterPages { chapter, source }) => {
            t::reader_wait_chapter_pages(chapter, source)
        }
        None => number.map_or_else(t::tui_reader_finding, |n| t::tui_reader_opening(&n)),
    };
    let words = if waited >= SECONDS_AFTER {
        t::reader_wait_seconds(&words, waited.as_secs() as i64)
    } else {
        words
    };
    let pulse = glyph::pulse(app.pulse_frame());
    let text = format!("  {words}"); // i18n-skip: layout
    let width = pulse.cells() + text.cells();
    let room = stage.width.saturating_sub(4) as usize;
    let x = stage.x + (stage.width.saturating_sub(width.min(room) as u16)) / 2;
    let y = middle(stage);
    line(buf, x, y, &pulse, app.palette.style(Role::State));
    let text = truncate(&text, room.saturating_sub(pulse.cells()));
    line(buf, x + pulse.cells() as u16, y, text, app.palette.style(Role::TextDim));
}

/// The stage's middle row, a row above the centre when the rows are even.
fn middle(stage: Rect) -> u16 {
    stage.y + stage.height.saturating_sub(1) / 2
}

/// The chapter could not open: what happened, in the page's place.
fn chapter_failed(buf: &mut Buffer, app: &App, reader: &Reader, stage: Rect) {
    let Some(failure) = &reader.chapter_failure else { return };
    let p = &app.palette;
    let heading = format!(
        "  {}",
        t::tui_reader_chapter_failed(&reader.chapter_number().unwrap_or_default())
    );
    let reason = chapter_failed_reason(failure, reader.failed_after);
    let room = stage.width.saturating_sub(4) as usize;
    let widest = (1 + heading.cells()).max(3 + reason.cells()).min(room);
    let x = stage.x + (stage.width.saturating_sub(widest as u16)) / 2;
    let y = middle(stage).saturating_sub(1);
    line(buf, x, y, "▲", p.style(Role::Alert)); // i18n-skip: the alert mark
    line(
        buf,
        x + 1,
        y,
        truncate(&heading, room.saturating_sub(1)),
        p.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    line(buf, x + 3, y + 1, truncate(&reason, room.saturating_sub(3)), p.style(Role::TextDim));
}

/// Why the chapter could not open, in the board's words where it has them: a source that didn't
/// answer, with the seconds the open waited once they were worth saying. The engine says which
/// source but not how long its request ran, so the seconds are the terminal's own count from the
/// open. Anything else in the engine's words.
fn chapter_failed_reason(
    failure: &kaeri_model::reading::ReadingFailure,
    waited: Option<std::time::Duration>,
) -> String {
    use kaeri_model::reading::ReadingFailure;
    match (failure, waited) {
        (ReadingFailure::SourceDown { source }, Some(waited)) if waited >= SECONDS_AFTER => {
            t::tui_reader_no_answer_in(source, waited.as_secs() as i64)
        }
        (ReadingFailure::SourceDown { source }, _) => t::tui_reader_no_answer(source),
        _ => say(&failure.msg()),
    }
}

/// Manga with images turned off: why there are no pages, centred.
fn images_off(buf: &mut Buffer, app: &App, stage: Rect) {
    let p = &app.palette;
    let y = middle(stage).saturating_sub(1);
    let lines = [
        (y, t::tui_reader_images_off(), p.style(Role::Text).add_modifier(Modifier::BOLD)),
        (y + 2, t::tui_reader_images_off_how(), p.style(Role::TextDim)),
        (y + 3, t::tui_reader_images_off_novels(), p.style(Role::TextDim)),
    ];
    for (y, text, style) in lines {
        let text = truncate(&text, stage.width as usize);
        let x = stage.x + (stage.width.saturating_sub(text.cells() as u16)) / 2;
        line(buf, x, y, text, style);
    }
}

/// The leaf in view: each part in its box, the picture once decoded, else a plate the size of
/// the page that says it is loading from 600 ms, or why it failed.
fn pages(buf: &mut Buffer, app: &App, reader: &Reader, stage: Rect, now: Instant) {
    if reader.strips() {
        strip(buf, app, reader, stage, now);
        return;
    }
    if let Some(view) = app.zoom_view(reader, stage) {
        viewport(buf, app, reader, &view, now);
        zoom_map(buf, app, &view, stage);
        return;
    }
    if reader.fits_side()
        && let Some(view) = app.fit_view(reader, stage)
    {
        viewport(buf, app, reader, &view, now);
        if reader.fits_width() {
            scroll_bar(buf, app, &view, stage);
        }
        return;
    }
    let Some(leaf) = reader.leaf() else { return };
    let waited = now.saturating_duration_since(reader.shown_at);
    for (part, rect) in leaf_boxes(app, reader, leaf, stage) {
        let key = PartKey { page: part.page, half: part.half };
        // A picture from an older file shows until the new one is decoded.
        if app.images.render_page(&reader.picture_key(key), rect, buf) {
            continue;
        }
        plate(buf, app, rect);
        if let Some(failure) = reader.failures.get(&part.page) {
            page_failed(buf, app, rect, part.page, failure);
        } else if waited >= PLATE_AFTER {
            page_loading(buf, app, rect, part.page);
        }
    }
}

/// Each part's box in cells: the leaf at full height, or full width when it is wider than the
/// stage, centred, its parts side by side in the order their places say.
fn leaf_boxes<'a>(
    app: &App,
    reader: &Reader,
    leaf: &'a Leaf,
    stage: Rect,
) -> Vec<(&'a kaeri_model::reader::LeafPart, Rect)> {
    let (cw, ch) = app.images.engine().font_size();
    let (cw, ch) = (f32::from(cw.max(1)), f32::from(ch.max(1)));
    let mut parts: Vec<_> = leaf.parts.iter().collect();
    parts.sort_by(|a, b| a.place.x.total_cmp(&b.place.x));
    let sizes: Vec<(f32, f32)> = parts.iter().map(|p| reader.part_size(p)).collect();
    // Every part at the height of the tallest, in pixels.
    let height = sizes.iter().map(|(_, h)| *h).fold(1.0_f32, f32::max);
    let widths: Vec<f32> = sizes.iter().map(|(w, h)| w * height / h.max(1.0)).collect();
    let width: f32 = widths.iter().sum::<f32>().max(1.0);
    let stage_w = f32::from(stage.width) * cw;
    let stage_h = f32::from(stage.height) * ch;
    let scale = (stage_w / width).min(stage_h / height);
    let rows = ((height * scale / ch).round() as u16).clamp(1, stage.height);
    let cols: Vec<u16> =
        widths.iter().map(|w| ((w * scale / cw).round() as u16).max(1)).collect();
    let total: u16 = cols.iter().sum::<u16>().min(stage.width);
    let mut x = stage.x + (stage.width - total) / 2;
    let y = stage.y + (stage.height - rows) / 2;
    parts
        .into_iter()
        .zip(cols)
        .map(|(part, w)| {
            let w = w.min(stage.right().saturating_sub(x));
            let rect = Rect { x, y, width: w, height: rows };
            x += w;
            (part, rect)
        })
        .collect()
}

/// A box on a canvas drawn where the view shows it, the rows in view of its picture, or a plate
/// that says the page is loading or why it failed. Says where the picture shows, if it does.
fn canvas_box(
    buf: &mut Buffer,
    app: &App,
    reader: &Reader,
    item: &CanvasBox,
    view: &Viewport,
    now: Instant,
) -> Option<Rect> {
    let shown = box_in_view(app, item, view)?;
    let rect = shown.rect;
    if let Some((cut, top, columns)) = shown.picture
        && app.images.render_slice(&item.key, cut, top, columns, buf)
    {
        return Some(rect);
    }
    plate(buf, app, rect);
    let part = item.part?;
    if let Some(failure) = reader.failures.get(&part.page) {
        page_failed(buf, app, rect, part.page, failure);
    } else if now.saturating_duration_since(reader.shown_at) >= PLATE_AFTER {
        page_loading(buf, app, rect, part.page);
    }
    None
}

/// Where a box shows in a view.
struct InView {
    /// The part of the box on screen.
    rect: Rect,
    /// Once its page is kept: what the page is made for, the box's top from the view's, and
    /// the screen the box's columns in view take, top to bottom of the view.
    picture: Option<(Cut, i32, Rect)>,
}

/// Where a box shows in the view. The page is made for the whole box, in rows, so a scroll draws
/// other rows of the same picture; a box wider than the view is made for the columns in view.
fn box_in_view(app: &App, item: &CanvasBox, view: &Viewport) -> Option<InView> {
    let area = view.area;
    let (vx0, vy0) = (view.x, view.y);
    let (vx1, vy1) = (vx0 + i32::from(area.width), vy0 + i32::from(area.height));
    let (bx1, by1) = (item.x + item.width as i32, item.y + item.height as i32);
    let (x0, x1) = (item.x.max(vx0), bx1.min(vx1));
    let (y0, y1) = (item.y.max(vy0), by1.min(vy1));
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let rect = Rect {
        x: area.x + (x0 - vx0) as u16,
        y: area.y + (y0 - vy0) as u16,
        width: (x1 - x0) as u16,
        height: (y1 - y0) as u16,
    };
    let picture = app.images.page_size(&item.key).map(|(pw, _)| {
        let across = |c: i32| {
            (f64::from(c - item.x) / f64::from(item.width.max(1)) * f64::from(pw)).round()
                as u32
        };
        let part = (x0 > item.x || x1 < bx1).then(|| (across(x0), across(x1)));
        let rows = u16::try_from(item.height).unwrap_or(u16::MAX);
        let cut = Cut::sliced(rect.width, rows, part);
        (cut, item.y - vy0, Rect { y: area.y, height: area.height, ..rect })
    });
    Some(InView { rect, picture })
}

/// The part of a canvas in view: a page at fit width, or zoomed in.
fn viewport(buf: &mut Buffer, app: &App, reader: &Reader, view: &Viewport, now: Instant) {
    for item in &view.boxes {
        if let Some(shown) = canvas_box(buf, app, reader, item, view, now) {
            app.images.note_pictures(shown);
        }
    }
}

/// The bar at the right at fit width: the rule's line the page's height, and the thumb the
/// part in view.
fn scroll_bar(buf: &mut Buffer, app: &App, view: &Viewport, stage: Rect) {
    let x = stage.right().saturating_sub(2);
    let rows = u32::from(stage.height);
    let total = view.height.max(1);
    let (thumb, at) = if total <= rows {
        (rows, 0)
    } else {
        let thumb =
            ((f64::from(rows) * f64::from(rows) / f64::from(total)).round() as u32).max(1);
        let at = (f64::from(view.y.max(0) as u32) * f64::from(rows) / f64::from(total)).round()
            as u32;
        (thumb, at.min(rows - thumb))
    };
    for y in 0..rows {
        let (mark, role) =
            if y >= at && y < at + thumb { ("┃", Role::Text) } else { ("│", Role::Rule) }; // i18n-skip: the bar
        line(buf, x, stage.y + y as u16, mark, app.palette.style(role));
    }
}

/// The box under the middle of the view, or the nearest to it.
fn centre_box(view: &Viewport) -> Option<&CanvasBox> {
    let cx = view.x + i32::from(view.area.width) / 2;
    view.boxes.iter().min_by_key(|b| {
        if cx < b.x { b.x - cx } else { (cx - (b.x + b.width as i32 - 1)).max(0) }
    })
}

/// The page under the middle of the zoomed view.
fn zoom_page(app: &App, reader: &Reader) -> Option<u32> {
    let stage = app.reader_stage()?;
    let view = app.zoom_view(reader, stage)?;
    centre_box(&view).and_then(|b| b.part).map(|p| p.page)
}

/// The zoom's map at the right: the page in halfblocks, the part in view bright and the rest
/// dimmed, and the page under it.
fn zoom_map(buf: &mut Buffer, app: &App, view: &Viewport, stage: Rect) {
    let Some(item) = centre_box(view) else { return };
    let (cw, ch) = app.images.engine().font_size();
    let ratio = (item.width as f32 * f32::from(cw.max(1)))
        / (item.height.max(1) as f32 * f32::from(ch.max(1)));
    let rows = MAP_ROWS.min(stage.height / 2).max(1);
    let room = ZOOM_PANEL.saturating_sub(3).min(MAP_COLUMNS);
    let cols = ((f32::from(rows) * ratio * f32::from(ch.max(1)) / f32::from(cw.max(1))).round()
        as u16)
        .clamp(1, room);
    let x = view.area.right() + 2;
    let y = stage.y + 1;
    if x + cols > stage.right() {
        return;
    }
    // The view on the page, in the map's pixels: two to a cell, one over the other.
    let px = |c: i32, size: u32, n: u16| f64::from(c) / f64::from(size.max(1)) * f64::from(n);
    let vx0 = px(view.x - item.x, item.width, cols).floor() as i32;
    let vx1 = (px(view.x + i32::from(view.area.width) - item.x, item.width, cols) + 0.5) as i32;
    let vy0 = px(view.y - item.y, item.height, rows * 2).floor() as i32;
    let vy1 =
        (px(view.y + i32::from(view.area.height) - item.y, item.height, rows * 2) + 0.5) as i32;
    match app.images.thumbnail(&item.key, (cols, rows)) {
        Some(thumb) => {
            let colour = |cx: u32, py: u32| {
                let [r, g, b] = thumb.get_pixel(cx, py).0;
                let inside =
                    (vx0..vx1).contains(&(cx as i32)) && (vy0..vy1).contains(&(py as i32));
                let dim = |v: u8| if inside { v } else { (u16::from(v) * 2 / 5) as u8 };
                ratatui::style::Color::Rgb(dim(r), dim(g), dim(b))
            };
            for cy in 0..rows {
                for cx in 0..cols {
                    let cell = &mut buf[(x + cx, y + cy)];
                    cell.set_symbol("▀"); // i18n-skip: a halfblock
                    cell.set_fg(colour(u32::from(cx), u32::from(cy) * 2));
                    cell.set_bg(colour(u32::from(cx), u32::from(cy) * 2 + 1));
                }
            }
        }
        None => plate(buf, app, Rect { x, y, width: cols, height: rows }),
    }
    if let Some(part) = item.part {
        let words = t::tui_reader_zoom_page(i64::from(part.page) + 1);
        let room = stage.right().saturating_sub(x) as usize;
        line(buf, x, y + rows + 1, truncate(&words, room), app.palette.style(Role::TextDim));
    }
}

/// A strip: a column of images from the chapters it ran on from, the open chapter and the
/// next chapter's first image, with a divider naming each chapter between them.
fn strip(buf: &mut Buffer, app: &App, reader: &Reader, stage: Rect, now: Instant) {
    let cols = crate::app::strip_cols(reader, stage.width);
    let layout = app.strip_layout(reader, cols);
    let x = stage.x + (stage.width - cols) / 2;
    let top = reader.strip.top;
    let bottom = top + u32::from(stage.height);
    let view = Viewport {
        boxes: Vec::new(),
        width: u32::from(cols),
        height: layout.total,
        x: 0,
        y: top as i32,
        area: Rect { x, width: cols, ..stage },
    };
    // The column, however the strip scrolls: images move up it a few rows at a time.
    app.images.note_pictures(view.area);
    let p = &app.palette;
    let width = cols as usize;
    for item in &layout.items {
        if item.top + item.rows <= top || item.top >= bottom {
            continue;
        }
        let row = |r: u32| {
            let at = item.top + r;
            (at >= top && at < bottom).then(|| stage.y + (at - top) as u16)
        };
        let centred = |buf: &mut Buffer, y: u16, text: &str, style| {
            let text = truncate(text, width);
            let left = x + (cols.saturating_sub(text.cells() as u16)) / 2;
            line(buf, left, y, text, style);
        };
        match &item.piece {
            Piece::Image { key, part, .. } => {
                let shown = CanvasBox {
                    key: key.clone(),
                    part: *part,
                    x: 0,
                    y: item.top as i32,
                    width: u32::from(cols),
                    height: item.rows,
                };
                canvas_box(buf, app, reader, &shown, &view, now);
            }
            Piece::Divider { next, source } => {
                if let Some(y) = row(1) {
                    let bold = p.style(Role::Text).add_modifier(Modifier::BOLD);
                    centred(buf, y, &chapter_name(next), bold);
                }
                if let Some(y) = row(2) {
                    centred(
                        buf,
                        y,
                        &divider_facts(next, source.as_ref()),
                        p.style(Role::TextDim),
                    );
                }
            }
            Piece::Foot => {
                let Some((heading, words)) = why_none_words(reader) else { continue };
                if let Some(y) = row(1) {
                    let bold = p.style(Role::Text).add_modifier(Modifier::BOLD);
                    centred(buf, y, &heading, bold);
                }
                for (i, text) in wrap(&words, width.max(1), 3).iter().enumerate() {
                    if let Some(y) = row(2 + i as u32) {
                        centred(buf, y, text, p.style(Role::TextDim));
                    }
                }
            }
        }
    }
}

/// The divider's facts: the next chapter's images and its source.
fn divider_facts(next: &kaeri_model::reading::ChapterRow, source: Option<&String>) -> String {
    let images = next.pages.filter(|p| *p > 0).map(|p| t::reader_divider_images(i64::from(p)));
    match (images, source) {
        (Some(images), Some(source)) => t::reader_divider_facts_source(&images, source),
        (Some(images), None) => images,
        (None, Some(source)) => t::reader_end_from(source),
        (None, None) => String::new(),
    }
}

/// A strip's status: the meter in a wide window, the chapter and the image at the top.
fn strip_status(app: &App, reader: &Reader, w: u16) -> Vec<(String, Role)> {
    let images = reader.leaves.iter().filter(|l| !l.end_page).count() as i64;
    let number = reader.chapter_number().unwrap_or_default();
    let mode = t::reader_mode_strip();
    let at = (reader.at as i64 + 1).min(images);
    let words = if images > 0 {
        format!("{} · {mode}", t::tui_reader_strip_at(&number, at, images)) // i18n-skip: layout
    } else {
        format!("{} · {mode}", t::reader_chapter_line(&number)) // i18n-skip: layout
    };
    if w < WIDE {
        return vec![(blocks_said(app, words), Role::TextDim)];
    }
    let fraction = if images > 0 { at as f64 / images as f64 } else { 0.0 };
    let mut runs = meter(reader, fraction, w);
    runs.push((blocks_said(app, format!("  {words}")), Role::TextDim)); // i18n-skip: layout
    runs
}

/// The status words, with Blocks after them when pages show as blocks.
fn blocks_said(app: &App, mut words: String) -> String {
    if app.images.engine().graphics() == Graphics::Halfblocks {
        words.push_str(&format!(" · {}", t::tui_reader_blocks()));
    }
    words
}

/// A page's place before its picture lands, in the rule's weight.
fn plate(buf: &mut Buffer, app: &App, rect: Rect) {
    let row = "░".repeat(rect.width as usize); // i18n-skip: the plate
    for y in rect.top()..rect.bottom() {
        line(buf, rect.x, y, &row, app.palette.style(Role::Rule));
    }
}

/// "Loading page 9" on the plate, with the pulse over it.
fn page_loading(buf: &mut Buffer, app: &App, rect: Rect, page: u32) {
    let y = middle(rect);
    let text = format!(" {} ", t::reader_page_loading(i64::from(page) + 1));
    let room = rect.width.saturating_sub(4) as usize;
    let x = rect.x + 4.min(rect.width / 4);
    line(buf, x, y, glyph::pulse(app.pulse_frame()), app.palette.style(Role::State));
    line(buf, x, y + 1, truncate(&text, room), app.palette.style(Role::Text));
}

/// A page that could not load: the alert, the page and the first sentence of the reason.
fn page_failed(
    buf: &mut Buffer,
    app: &App,
    rect: Rect,
    page: u32,
    failure: &kaeri_model::reading::ReadingFailure,
) {
    let p = &app.palette;
    let heading = t::reader_page_failed(i64::from(page) + 1);
    // The board's words for a server's error; anything else, the engine's first sentence.
    let first = match failure {
        kaeri_model::reading::ReadingFailure::PageFetch { status } => {
            t::tui_reader_page_failed_status(i64::from(*status))
        }
        _ => {
            let reason = say(&failure.msg());
            match reason.find(". ") {
                Some(at) => reason[..=at].to_owned(),
                None => reason,
            }
        }
    };
    let room = rect.width.saturating_sub(4) as usize;
    let wrap_at = 20.min(room.saturating_sub(2));
    let reasons = wrap(&first, wrap_at.max(1), 2);
    let width = (heading.cells() + 2).max(wrap_at + 2).min(room);
    let pad = |text: &str| {
        let text = truncate(text, width.saturating_sub(1));
        format!(" {text}{}", " ".repeat(width.saturating_sub(1 + text.cells()))) // i18n-skip: layout
    };
    let x = rect.x + 2.min(rect.width / 4);
    let y = middle(rect).saturating_sub(1);
    line(buf, x, y, " ▲ ", p.style(Role::Alert)); // i18n-skip: the alert mark
    line(buf, x, y + 1, pad(&heading), p.style(Role::Text));
    for (i, reason) in reasons.iter().enumerate() {
        line(buf, x, y + 2 + i as u16, pad(reason), p.style(Role::TextDim));
    }
}

/// The page between chapters: what happened to progress, then the next chapter with its first
/// page, or why there is none.
fn end_page(buf: &mut Buffer, app: &App, reader: &Reader, stage: Rect) {
    let p = &app.palette;
    let bold = p.style(Role::Text).add_modifier(Modifier::BOLD);
    let wide = stage.width >= WIDE && stage.height >= 24;
    let x = if wide { stage.x + (stage.width - END_WIDTH) / 2 } else { stage.x + 4 };
    let width = if wide {
        END_WIDTH as usize
    } else {
        (stage.width as usize).saturating_sub(8).min(40)
    };
    let mut y = stage.y + if wide { 2 } else { 1 };
    let number = reader.chapter_number().unwrap_or_default();
    let finished = reader.end.next.is_none() && reader.end.why_none == Some(NoNext::Finished);
    if finished {
        line(buf, x, y, truncate(&t::tui_reader_end_series(), width), bold);
        line(
            buf,
            x,
            y + 1,
            truncate(&t::tui_reader_end_no_more(), width),
            p.style(Role::TextDim),
        );
        y += 3;
    } else {
        line(buf, x, y, truncate(&t::reader_end_of_chapter_number(&number), width), bold);
        y += 1;
        if wide && let Some(chapter) = reader.chapter() {
            let mut facts: Vec<String> = Vec::new();
            facts.push(chapter_name(chapter));
            let pages = reader.story_pages();
            if pages > 0 {
                facts.push(t::reader_end_pages(i64::from(pages)));
            }
            line(buf, x, y, truncate(&facts.join(" · "), width), p.style(Role::TextDim));
            y += 1;
        }
        y += 1;
    }
    // What became of the read: the trackers, then the book server.
    let mut said_any = false;
    let read = reader.end.read.as_ref().map(|trackers| read_line(app, trackers));
    for text in read.into_iter().chain(reader.end.server.clone()) {
        let lines = wrap(&text, width.saturating_sub(2).max(1), 3);
        for (i, text) in lines.iter().enumerate() {
            if i == 0 {
                line(buf, x, y, "●", p.style(Role::State)); // i18n-skip: the state mark
            }
            line(buf, x + 2, y, text, p.style(Role::Text));
            y += 1;
        }
        said_any = true;
    }
    if finished {
        return;
    }
    if said_any {
        y += 1;
    }
    match &reader.end.next {
        Some(next) if wide => next_wide(buf, app, reader, next, stage, x, stage.y + 9),
        Some(next) => next_compact(buf, app, reader, next, stage, y),
        None => why_none(buf, app, reader, x, y, width),
    }
}

/// "Counted as read", with the trackers updated or waiting for the network.
fn read_line(app: &App, trackers: &[String]) -> String {
    if trackers.is_empty() {
        return t::reader_end_read();
    }
    let names = trackers.join(", ");
    if app.offline {
        t::reader_end_tracker_offline(&names)
    } else {
        t::reader_end_tracker(&names)
    }
}

/// The next chapter's facts: its pages, its source, and whether it is on this computer.
fn next_facts(next: &kaeri_model::reading::ChapterRow, source: Option<&String>) -> String {
    let mut facts: Vec<String> = Vec::new();
    if let Some(pages) = next.pages.filter(|p| *p > 0) {
        facts.push(t::reader_end_pages(i64::from(pages)));
    }
    if let Some(source) = source {
        facts.push(t::reader_end_from(source));
    }
    if crate::app::downloaded(next) {
        facts.push(t::chapter_state_downloaded());
    }
    facts.join(" · ")
}

/// Next at full size: a heading ruled across, the first page as a picture, and the chapter
/// beside it.
fn next_wide(
    buf: &mut Buffer,
    app: &App,
    reader: &Reader,
    next: &kaeri_model::reading::ChapterRow,
    stage: Rect,
    x: u16,
    y: u16,
) {
    let p = &app.palette;
    let heading = t::reader_end_next();
    line(buf, x, y, &heading, p.style(Role::TextDim));
    let from = x + heading.cells() as u16 + 2;
    let rule_end = x + END_WIDTH;
    if from < rule_end {
        let rule = "─".repeat((rule_end - from) as usize); // i18n-skip: a rule
        line(buf, from, y, rule, p.style(Role::Rule));
    }
    let top = y + 2;
    let rows = 13.min(stage.bottom().saturating_sub(top + 1));
    let (cw, ch) = app.images.engine().font_size();
    let ratio = reader
        .end
        .preview
        .as_ref()
        .filter(|(id, _)| *id == next.id)
        .map_or(2.0 / 3.0, |(_, (w, h))| *w as f32 / (*h).max(1) as f32);
    let cols = ((f32::from(rows) * f32::from(ch) * ratio / f32::from(cw.max(1))).round()
        as u16)
        .max(1);
    let picture = Rect { x, y: top, width: cols, height: rows };
    let key = reader.preview_key(&next.id);
    let drawn = reader.end.preview.as_ref().is_some_and(|(id, _)| *id == next.id)
        && app.images.render_page(&key, picture, buf);
    if !drawn {
        plate(buf, app, picture);
    }
    let tx = x + cols + 3;
    let room = stage.right().saturating_sub(tx + 1) as usize;
    let bold = p.style(Role::Text).add_modifier(Modifier::BOLD);
    line(buf, tx - 2, top, glyph::OBI.to_string(), p.style(Role::Obi));
    line(buf, tx, top, truncate(&chapter_name(next), room), bold);
    let facts = next_facts(next, reader.end.next_source.as_ref());
    line(buf, tx, top + 1, truncate(&facts, room), p.style(Role::TextDim));
    if let Some(key) = app.reader_key_name(Action::ReaderBare) {
        line(buf, tx, top + 3, &key, p.style(Role::Text));
        let word = format!(" {}", said(t::tui_read_hint_continue()).to_lowercase());
        line(buf, tx + key.cells() as u16, top + 3, word, p.style(Role::TextDim));
    }
    if let Some(skip) = &reader.end.skip_to {
        let number = skip.number.clone().unwrap_or_else(|| skip.label.clone());
        line(
            buf,
            tx,
            top + 5,
            truncate(&t::tui_reader_end_skip_unread(&number), room),
            p.style(Role::TextDim),
        );
    }
    let number = next.number.clone().unwrap_or_else(|| next.label.clone());
    let foot = t::tui_reader_end_first_page(&number);
    line(buf, tx, top + rows.saturating_sub(2), truncate(&foot, room), p.style(Role::TextDim));
}

/// Next in a small window: the obi, the chapter and its facts, and what a skip leaves unread.
fn next_compact(
    buf: &mut Buffer,
    app: &App,
    reader: &Reader,
    next: &kaeri_model::reading::ChapterRow,
    stage: Rect,
    y: u16,
) {
    let p = &app.palette;
    let x = stage.x + 4;
    let room = stage.width.saturating_sub(6) as usize;
    line(buf, stage.x + 2, y, glyph::OBI.to_string(), p.style(Role::Obi));
    line(
        buf,
        x,
        y,
        truncate(&chapter_name(next), room),
        p.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    let facts = next_facts(next, reader.end.next_source.as_ref());
    line(buf, x, y + 1, truncate(&facts, room), p.style(Role::TextDim));
    if let Some(skip) = &reader.end.skip_to {
        let number = skip.number.clone().unwrap_or_else(|| skip.label.clone());
        line(
            buf,
            x,
            y + 3,
            truncate(&t::tui_reader_end_skip_unread(&number), room),
            p.style(Role::TextDim),
        );
    }
}

/// Why there is no next chapter, and what happens next.
fn why_none(buf: &mut Buffer, app: &App, reader: &Reader, x: u16, y: u16, width: usize) {
    let p = &app.palette;
    let Some((heading, words)) = why_none_words(reader) else { return };
    line(
        buf,
        x,
        y,
        truncate(&heading, width),
        p.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    for (i, text) in wrap(&words, width.max(1), 3).iter().enumerate() {
        line(buf, x, y + 1 + i as u16, text, p.style(Role::TextDim));
    }
}

/// Why there is no next chapter: a heading and a line.
fn why_none_words(reader: &Reader) -> Option<(String, String)> {
    let next_number = reader
        .chapter()
        .and_then(|c| c.number.as_deref())
        .and_then(|n| n.parse::<u32>().ok())
        .map(|n| (n + 1).to_string());
    let (heading, words) = match reader.end.why_none {
        Some(NoNext::NotOutYet) => (
            next_number
                .as_ref()
                .map_or_else(t::tui_reader_end_not_out_next, |n| t::tui_reader_end_not_out(n)),
            t::tui_reader_end_not_out_line(),
        ),
        Some(NoNext::NoSourceHasIt) => (
            next_number.as_ref().map_or_else(t::tui_reader_end_no_source_next, |n| {
                t::tui_reader_end_no_source(n)
            }),
            t::tui_reader_end_caught_up_line(),
        ),
        Some(NoNext::Offline) => (
            next_number
                .as_ref()
                .map_or_else(t::tui_reader_end_offline_next, |n| t::tui_reader_end_offline(n)),
            t::tui_reader_end_offline_line(),
        ),
        Some(NoNext::CaughtUp) | None => {
            (t::reader_end_caught_up(), t::tui_reader_end_caught_up_line())
        }
        Some(NoNext::Finished) => return None,
    };
    Some((heading, words))
}

/// Below 40 by 12: what size the reader needs. The place is kept for when the window grows.
fn too_small(buf: &mut Buffer, app: &App, reader: &Reader, area: Rect) {
    let p = &app.palette;
    let bold = p.style(Role::Text).add_modifier(Modifier::BOLD);
    if area.height >= 3 {
        line(buf, area.x, area.y, "Kaeri", bold); // i18n-skip: the product name
        Hairline::new(p).render(Rect { y: area.y + 1, height: 1, ..area }, buf);
    }
    let y = area.y + (area.height / 2).saturating_sub(1);
    let lines = [
        (y, t::tui_reader_too_small(), bold),
        (
            y + 1,
            t::tui_reader_too_small_line(i64::from(MIN_COLUMNS), i64::from(MIN_ROWS)),
            p.style(Role::TextDim),
        ),
    ];
    for (y, text, style) in lines {
        let text = truncate(&text, area.width as usize);
        let x = area.x + (area.width.saturating_sub(text.cells() as u16)) / 2;
        line(buf, x, y, text, style);
    }
    if area.height >= 5 {
        Hairline::new(p)
            .render(Rect { y: area.bottom().saturating_sub(2), height: 1, ..area }, buf);
        let left = page_count(reader).map(|c| vec![(c, Role::TextDim)]).unwrap_or_default();
        status(buf, app, Rect { y: area.bottom().saturating_sub(1), height: 1, ..area }, &left);
    }
}

/// The key reference over the reader: its own keys and how it shows pages, then the global
/// keys, in two columns, with how the turn keys go in a right-to-left book beside them.
fn help(buf: &mut Buffer, app: &App, reader: &Reader, area: Rect) {
    let p = &app.palette;
    let bold = p.style(Role::Text).add_modifier(Modifier::BOLD);
    let (w, h) = (area.width, area.height);
    line(buf, area.x, area.y, "Kaeri", bold); // i18n-skip: the product name
    let chips = if w >= ROOMY { header_chips(app) } else { Vec::new() };
    let mut x = area.right();
    for (i, (text, role)) in chips.iter().enumerate().rev() {
        x = x.saturating_sub(text.cells() as u16);
        line(buf, x, area.y, text, p.style(*role));
        if i > 0 {
            x = x.saturating_sub(2);
        }
    }
    for y in [1, 2, 4] {
        Hairline::new(p).render(Rect { y: area.y + y, height: 1, ..area }, buf);
    }
    line(buf, area.x, area.y + 3, glyph::OBI.to_string(), p.style(Role::Obi));
    line(buf, area.x + 2, area.y + 3, t::nav_overlay_help(), bold);
    // Two columns 44 wide, and the aside after a rule when the window has room for it.
    let aside_at = 2 + 2 * 44 + 2;
    let aside = w >= aside_at + 24;
    let col = if aside { 44 } else { ((w.saturating_sub(4)) / 2).min(44) };
    let groups = app.keymap.reader_help(app.reads_novel());
    let bottom = area.bottom().saturating_sub(3);
    // The reader's own keys fill the first column, the rest the second.
    let mut columns: [Vec<(Option<&str>, String)>; 2] = [Vec::new(), Vec::new()];
    for (i, (heading, rows)) in groups.iter().enumerate() {
        let column = &mut columns[usize::from(i > 0)];
        if !column.is_empty() {
            column.push((None, String::new()));
        }
        column.push((None, (*heading).to_owned()));
        for (keys, label) in rows {
            column.push((Some(label), keys.clone()));
        }
    }
    for (ci, column) in columns.iter().enumerate() {
        let x = area.x + 2 + ci as u16 * col;
        for (row, (label, text)) in column.iter().enumerate() {
            let y = area.y + 5 + row as u16;
            if y >= bottom {
                break;
            }
            match label {
                None => line(buf, x, y, truncate(text, col as usize), p.style(Role::TextDim)),
                Some(label) => {
                    let keys_at = x + col.saturating_sub(4);
                    let room = col.saturating_sub(8) as usize;
                    // The keys keep what the label leaves them, and at least half the row.
                    let keys_room = room.saturating_sub(label.cells() + 1).max(room / 2);
                    let keys = truncate(text, keys_room);
                    let label_room = room.saturating_sub(keys.cells() + 1);
                    line(buf, x + 2, y, truncate(label, label_room), p.style(Role::Text));
                    let kx = keys_at.saturating_sub(keys.cells() as u16);
                    line(buf, kx, y, &keys, p.style(Role::Text));
                }
            }
        }
    }
    if aside {
        let divider = area.x + aside_at - 2;
        for y in area.y + 5..=bottom.saturating_sub(1).max(area.y + 5) {
            line(buf, divider, y, "│", p.style(Role::Rule)); // i18n-skip: a rule
        }
        let ax = area.x + aside_at;
        let room = area.right().saturating_sub(ax + 1) as usize;
        let mut y = area.y + 5;
        // The turn keys' note is about manga's pages; text always reads on to the right.
        let notes = if reader.novel.is_some() {
            vec![t::tui_reader_keys_left_out()]
        } else {
            vec![t::tui_reader_keys_rtl(), t::tui_reader_keys_left_out()]
        };
        for note in notes {
            for text in wrap(&note, room, 6) {
                if y >= bottom {
                    break;
                }
                line(buf, ax, y, text, p.style(Role::TextDim));
                y += 1;
            }
            y += 1;
        }
    }
    Hairline::new(p)
        .render(Rect { y: area.bottom().saturating_sub(2), height: 1, ..area }, buf);
    let count =
        if reader.novel.is_some() { app.novel_status_words(false) } else { page_count(reader) };
    let left = count.map(|c| vec![(c, Role::TextDim)]).unwrap_or_default();
    status(buf, app, Rect { y: area.bottom().saturating_sub(1), height: 1, ..area }, &left);
    let _ = h;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use kaeri_model::reading::ReadingFailure;

    use super::{chapter_failed_reason, fitting_sentences};

    #[test]
    fn a_source_that_did_not_answer_says_the_wait_once_it_was_long() {
        kaeri_i18n::set_language("en");
        let down = ReadingFailure::SourceDown { source: "Home library".into() };
        assert_eq!(
            chapter_failed_reason(&down, Some(Duration::from_secs(15))),
            "Home library didn't answer. No answer in 15 s."
        );
        assert_eq!(
            chapter_failed_reason(&down, Some(Duration::from_secs(1))),
            "Home library didn't answer."
        );
        assert_eq!(chapter_failed_reason(&down, None), "Home library didn't answer.");
        assert_eq!(
            chapter_failed_reason(&ReadingFailure::Offline, None),
            "You're offline. Downloaded chapters still open."
        );
    }

    #[test]
    fn a_status_run_keeps_whole_sentences_or_goes() {
        let foot = "Changes apply at once. The place in the text stays.";
        assert_eq!(fitting_sentences(foot, 80).as_deref(), Some(foot));
        assert_eq!(fitting_sentences(foot, 30).as_deref(), Some("Changes apply at once."));
        assert_eq!(fitting_sentences(foot, 10), None);
        assert_eq!(fitting_sentences("  End of chapter 14", 6), None);
    }
}
