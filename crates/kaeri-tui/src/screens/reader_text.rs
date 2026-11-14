//! A light novel's text on the reader's stage, and the reader settings overlay with its live
//! preview of the text at the width being set.

use kaeri_core::media::Medium;
use kaeri_i18n::t;
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use super::line;
use crate::{
    app::{App, Reader, ReaderScreen, SettingItem, SettingPlace, SettingShown},
    novel::{Shape, TextLayout, lay_out},
    theme::{Role, glyph},
    widgets::{Cells, Hairline, truncate, wrap},
};

/// From this width the settings overlay shows the text beside it.
const PREVIEW_FROM: u16 = 100;
/// The settings column's right edge, where its headings rule to, beside the preview.
const SETTINGS_RIGHT: u16 = 60;
/// Where the values start, after the labels.
const VALUE_AT: u16 = 22;

/// The novel's text in its column: one screen of the document in view, or, scrolled, the
/// lines running on through the documents.
pub(super) fn novel_page(buf: &mut Buffer, app: &App, reader: &Reader, area: Rect) {
    let Some(frame) = app.novel_frame((area.width, area.height)) else { return };
    let Some((layout, top)) = app.novel_text(frame) else { return };
    // Paged, a screen ends with its document: the next one starts on a screen of its own.
    let end = if reader.novel_scrolled() {
        layout.lines.len()
    } else {
        layout.docs.get(layout.doc_of(top)).map_or(layout.lines.len(), |r| r.end)
    };
    text_lines(buf, app, &layout, top..end, frame.column);
}

/// Lines `range` of `layout` from the top of `column`, as many as fit.
fn text_lines(
    buf: &mut Buffer,
    app: &App,
    layout: &TextLayout,
    range: std::ops::Range<usize>,
    column: Rect,
) {
    let p = &app.palette;
    for (row, i) in range.take(usize::from(column.height)).enumerate() {
        let Some(text) = layout.lines.get(i) else { break };
        let y = column.y + row as u16;
        let mut x = column.x + text.x;
        if let Some(mark) = &text.mark {
            line(buf, x.saturating_sub(2), y, mark, p.style(Role::TextDim));
        }
        for span in &text.spans {
            let mut style = p.style(if span.look.dim { Role::TextDim } else { Role::Text });
            if span.look.strong {
                style = style.add_modifier(Modifier::BOLD);
            }
            if span.look.emphasis {
                style = style.add_modifier(Modifier::ITALIC);
            }
            let room = usize::from(column.right().saturating_sub(x));
            let shown = truncate(&span.text, room);
            line(buf, x, y, &shown, style);
            x += shown.cells() as u16;
        }
    }
}

/// The reader settings over the whole window: the header, the rows under where a change is
/// kept, what the terminal leaves to itself, and for a novel in a wide window the text as it
/// will read.
pub(super) fn settings_overlay(buf: &mut Buffer, app: &App, reader: &Reader, area: Rect) {
    let p = &app.palette;
    let bold = p.style(Role::Text).add_modifier(Modifier::BOLD);
    let dim = p.style(Role::TextDim);
    let w = area.width;
    super::reader::header(buf, app, reader, ReaderScreen::Pages, area);
    for y in [1, 2, 4] {
        Hairline::new(p).render(Rect { y: area.y + y, height: 1, ..area }, buf);
    }
    line(buf, area.x, area.y + 3, glyph::OBI.to_string(), p.style(Role::Obi));
    line(buf, area.x + 2, area.y + 3, t::tui_reader_action_settings(), bold);

    let medium = app.settings_medium();
    let novel = medium == Medium::Novel;
    let preview = novel && w >= PREVIEW_FROM;
    let x = area.x + 2;
    let right = if w >= PREVIEW_FROM {
        area.x + SETTINGS_RIGHT
    } else {
        area.right().saturating_sub(2)
    };
    let bottom = area.bottom().saturating_sub(2);
    let items = app.reader_setting_items();
    let note = if novel {
        t::tui_reader_settings_novel_note()
    } else {
        t::tui_reader_settings_manga_note()
    };
    let note = wrap(&note, usize::from(right.saturating_sub(x + 6)).max(10), 6);
    // A blank line between the sections while they all fit with one; else none.
    let sections = 1 + items.windows(2).filter(|w| w[0].place != w[1].place).count();
    let rows = items.len() + 2 * sections + 1 + note.len();
    let gap = u16::from(area.y + 6 + rows as u16 <= bottom);
    let selected = app.cursors.overlay.selected.min(items.len().saturating_sub(1));
    let mut y = area.y + 6;
    let mut place: Option<SettingPlace> = None;
    for (i, item) in items.iter().enumerate() {
        if place != Some(item.place) {
            if place.is_some() {
                y += gap;
            }
            place = Some(item.place);
            heading(buf, app, x, y, &item.place.heading(medium), right, bottom);
            y += 1;
        }
        if y >= bottom {
            break;
        }
        let here = i == selected;
        if here {
            line(buf, area.x, y, glyph::OBI.to_string(), p.style(Role::Obi));
        }
        let label_room = usize::from(VALUE_AT.saturating_sub(3));
        line(
            buf,
            x + 2,
            y,
            truncate(&item.label, label_room),
            if here { p.style(Role::Text) } else { dim },
        );
        value(buf, app, x + VALUE_AT, y, item, right);
        y += 1;
    }
    y += gap;
    heading(buf, app, x, y, &t::tui_reader_settings_not_here(), right, bottom);
    y += 1;
    for text in note {
        if y >= bottom {
            break;
        }
        line(buf, x + 2, y, &text, dim);
        y += 1;
    }

    if preview {
        preview_text(buf, app, reader, area);
    }

    Hairline::new(p).render(Rect { y: area.bottom() - 2, height: 1, ..area }, buf);
    let foot = if novel {
        t::tui_reader_settings_novel_foot()
    } else {
        t::tui_reader_settings_manga_foot()
    };
    super::reader::status(
        buf,
        app,
        Rect { y: area.bottom() - 1, height: 1, ..area },
        &[(foot, Role::TextDim)],
    );
}

/// A heading in the overlay's own style: dim words and a rule to `right`.
fn heading(buf: &mut Buffer, app: &App, x: u16, y: u16, text: &str, right: u16, bottom: u16) {
    if y >= bottom {
        return;
    }
    let shown = truncate(text, usize::from(right.saturating_sub(x)));
    line(buf, x, y, &shown, app.palette.style(Role::TextDim));
    let from = x + shown.cells() as u16 + 2;
    if from < right {
        Hairline::new(&app.palette).render(Rect::new(from, y, right - from, 1), buf);
    }
}

/// A row's value: every choice with the one in effect bold, an amount between arrows, or On
/// or Off.
fn value(buf: &mut Buffer, app: &App, x: u16, y: u16, item: &SettingItem, right: u16) {
    let p = &app.palette;
    let bold = p.style(Role::Text).add_modifier(Modifier::BOLD);
    let dim = p.style(Role::TextDim);
    let runs: Vec<(String, ratatui::style::Style)> = match &item.shown {
        SettingShown::Choices(choices) => choices
            .iter()
            .enumerate()
            .flat_map(|(i, (words, current))| {
                let sep = (i > 0).then(|| ("  ·  ".to_owned(), dim)); // i18n-skip: a separator
                sep.into_iter().chain([(words.clone(), if *current { bold } else { dim })])
            })
            .collect(),
        SettingShown::Amount(words) => vec![
            ("‹ ".to_owned(), dim), // i18n-skip: an arrow
            (words.clone(), bold),
            (" ›".to_owned(), dim), // i18n-skip: an arrow
        ],
        SettingShown::Toggle(on) => {
            vec![(if *on { t::value_on() } else { t::value_off() }, bold)]
        }
    };
    let mut x = x;
    for (text, style) in runs {
        let room = usize::from(right.saturating_sub(x));
        if room == 0 {
            break;
        }
        let shown = truncate(&text, room);
        line(buf, x, y, &shown, style);
        x += shown.cells() as u16;
    }
}

/// The text from the place in view at the width being set, to the right of the settings.
fn preview_text(buf: &mut Buffer, app: &App, reader: &Reader, area: Rect) {
    let p = &app.palette;
    let Some(novel) = reader.novel.as_ref() else { return };
    let Some(frame) = app.novel_frame((area.width, area.height)) else { return };
    let divider = area.x + SETTINGS_RIGHT + 3;
    let x = divider + 3;
    let top = area.y + 6;
    let bottom = area.bottom().saturating_sub(3);
    for y in top..=bottom.min(area.bottom().saturating_sub(4)) {
        line(buf, divider, y, "│", p.style(Role::Rule)); // i18n-skip: a rule
    }
    let label = t::tui_reader_settings_preview();
    line(buf, x, top, &label, p.style(Role::TextDim));
    let from = x + label.cells() as u16 + 2;
    let right = area.right().saturating_sub(2);
    if from < right {
        Hairline::new(p).render(Rect::new(from, top, right - from, 1), buf);
    }
    let room = area.right().saturating_sub(x + 2);
    let width = frame.shape.width.min(room).max(1);
    let layout = lay_out(&novel.open.text, Shape { width, ..frame.shape });
    let mut first = layout.line_of(&novel.open.text, &novel.place).unwrap_or(0);
    // The preview starts with words, not the blank line over a heading.
    while layout.lines.get(first).is_some_and(|l| l.is_blank()) {
        first += 1;
    }
    let column = Rect::new(x, top + 2, width, bottom.saturating_sub(top + 2));
    text_lines(buf, app, &layout, first..layout.lines.len(), column);
}
