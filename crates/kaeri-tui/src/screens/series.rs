//! A reading series' page: the title and its facts, the synopsis, where it is read from and the
//! list it is on, how far it is read, then its chapters newest first. Offline, a line says only
//! downloaded chapters open, and the parts that need a connection step aside.

use kaeri_i18n::t;
use kaeri_model::reading::{ChapterReadState, ChapterRow, SeriesAction};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

use super::{chrome::clock, line, lists::plate_row, lists::render_loading};
use crate::{
    app::{
        App, LOADING_LINE_AFTER, SeriesPage, action_words, chapter_line, chapter_name,
        downloaded, read_list_name, say, series_facts,
    },
    keymap::Action,
    nav::Focus,
    theme::{Role, glyph},
    widgets::{Cells, truncate, wrap},
};

/// The synopsis wraps at a reading width, however wide the window.
const SYNOPSIS_WIDTH: usize = 72;
/// Its lines until `i` shows the rest.
const SYNOPSIS_LINES: usize = 2;
const SYNOPSIS_LINES_OPEN: usize = 12;
/// The progress meter's cells.
const METER_WIDTH: usize = 20;

/// Draw the series page on top. Returns the first chapter the window showed.
pub(super) fn render_series(buf: &mut Buffer, app: &App, area: Rect) -> Option<usize> {
    if area.height == 0 || area.width < 8 {
        return None;
    }
    let page = app.series_here()?;
    let now = app.clock.now_instant();
    let palette = &app.palette;
    let style = |role: Role| palette.style(role);
    let bold = |role: Role| palette.style(role).add_modifier(Modifier::BOLD);
    let x = area.left() + 2;
    let width = area.width.saturating_sub(3) as usize;
    let bottom = area.bottom();
    let retry = || format!("{} {}", app.key_for(Action::Refresh), t::tui_read_try_again()); // i18n-skip

    let Some(view) = page.view.as_ref() else {
        // Why it could not be had, with the key that asks again; else the loading line, once it
        // has waited.
        if let Some(failure) = &page.failure {
            failed_line(buf, app, x, area.top(), width, &say(&failure.msg()), &retry());
        } else if page.wait.waited(now) {
            render_loading(buf, app, area);
        }
        return None;
    };
    if page.wait.held(now) {
        render_loading(buf, app, area);
        return None;
    }

    let offline = page.offline();
    let mut y = area.top();
    if offline {
        line(buf, x, y, truncate(&t::tui_read_series_offline(), width), style(Role::TextDim));
        y += 2;
    }
    line(buf, x, y, truncate(&view.titles.display, width), bold(Role::Text));
    y += 1;
    let facts = series_facts(view, page.rows().len());
    if !facts.is_empty() {
        line(buf, x, y, truncate(&facts, width), style(Role::TextDim));
        y += 1;
    }

    if !offline {
        let synopsis = view.synopsis.as_deref().map(str::trim).filter(|s| !s.is_empty());
        if let Some(synopsis) = synopsis {
            let lines =
                if app.synopsis_expanded { SYNOPSIS_LINES_OPEN } else { SYNOPSIS_LINES };
            y += 1;
            for text in wrap(synopsis, width.min(SYNOPSIS_WIDTH), lines) {
                line(buf, x, y, &text, style(Role::TextDim));
                y += 1;
            }
        }
        if let Some(text) = where_line(page) {
            y += 1;
            line(buf, x, y, truncate(&text, width), style(Role::TextDim));
            y += 1;
        }
        if let Some(rows) = page.chapters.as_ref().filter(|rows| !rows.is_empty()) {
            y += 1;
            y = progress(buf, app, x, y, width, page, rows.len(), &view.action);
        }
    }

    // "Chapters", ruled to the right edge.
    y += 1;
    if y >= bottom {
        return None;
    }
    let heading = t::series_chapters();
    line(buf, x, y, &heading, style(Role::TextDim));
    let rule_x = x + heading.cells() as u16 + 2;
    let right = area.right().saturating_sub(1);
    if rule_x < right {
        line(
            buf,
            rule_x,
            y,
            glyph::RULE_H.to_string().repeat((right - rule_x) as usize),
            style(Role::Rule),
        );
    }
    y += 1;

    // What stands in for the chapters until they come, or when there are none.
    let Some(rows) = &page.chapters else {
        if page.chapters_wait.waited(now) {
            for share in [60, 44] {
                if y < bottom {
                    line(buf, x, y, plate_row(width * share / 100), style(Role::TextDim));
                    y += 1;
                }
            }
            let waited = page.chapters_wait.since(now);
            if waited.is_some_and(|w| w >= LOADING_LINE_AFTER) && y + 1 < bottom {
                let asking = match page.edition() {
                    Some(edition) => t::read_search_asking(&edition.source_name),
                    None => t::series_read_from_checking(),
                };
                let text = format!("{asking}  {}", clock(waited.unwrap_or_default())); // i18n-skip
                line(buf, x, y + 1, truncate(&text, width), style(Role::TextDim));
            }
        }
        return None;
    };
    if let Some(failure) = page.chapters_failure.as_ref().filter(|_| !offline) {
        let reason = match page.edition() {
            Some(edition) => t::series_chapters_failed(&edition.source_name),
            None => say(&failure.msg()),
        };
        failed_line(buf, app, x, y, width, &reason, &retry());
        y += 2;
    } else if rows.is_empty() {
        let (title, words) = if view.action == SeriesAction::NoSource {
            (t::series_no_source_title(), t::series_no_source_line())
        } else {
            (t::series_no_chapters_title(), t::series_no_chapters_line())
        };
        line(buf, x, y, truncate(&title, width), style(Role::Text));
        for (n, text) in wrap(&words, width, 3).into_iter().enumerate() {
            line(buf, x, y + 1 + n as u16, &text, style(Role::TextDim));
        }
        return None;
    }
    if y >= bottom || rows.is_empty() {
        return None;
    }

    let visible = (bottom - y) as usize;
    let cursor = app.cursors.chapters;
    let selected = cursor.selected.min(rows.len() - 1);
    let start = cursor.offset.clamp((selected + 1).saturating_sub(visible), selected);
    let focused = app.nav.focus() == Focus::Stage;
    for (n, row) in rows.iter().enumerate().skip(start).take(visible) {
        chapter(buf, app, area, y, row, n == selected, focused, offline);
        y += 1;
    }
    Some(start)
}

/// "Reading from Home library  ·  Reading on AniList": the source, then the list it is on.
fn where_line(page: &SeriesPage) -> Option<String> {
    let view = page.view.as_ref()?;
    let from = page.edition().map(|edition| t::tui_read_series_from(&edition.source_name));
    let list = view.list.map(|list| {
        let trackers = page.trackers();
        if trackers.is_empty() {
            t::tui_read_series_list(&read_list_name(list))
        } else {
            t::tui_read_series_list_on(&read_list_name(list), &trackers)
        }
    });
    let parts: Vec<String> = from.into_iter().chain(list).collect();
    (!parts.is_empty()).then(|| parts.join("  ·  ")) // i18n-skip
}

/// The meter, how many chapters are read, and what Enter on the next one does: on one line when
/// it fits, the last on its own below when not. Returns the row after them.
#[allow(clippy::too_many_arguments)]
fn progress(
    buf: &mut Buffer,
    app: &App,
    x: u16,
    y: u16,
    width: usize,
    page: &SeriesPage,
    total: usize,
    action: &SeriesAction,
) -> u16 {
    let style = |role: Role| app.palette.style(role);
    let (read, _) = page.read_count();
    let (filled, track) = glyph::meter_parts(read as f64 / total as f64, METER_WIDTH);
    let mut at = x;
    line(buf, at, y, &filled, style(Role::Text));
    at += filled.cells() as u16;
    line(buf, at, y, &track, style(Role::Rule));
    at += track.cells() as u16;
    let count = format!("  {}", t::tui_read_series_read(read as i64, total as i64)); // i18n-skip
    line(buf, at, y, &count, style(Role::TextDim));
    at += count.cells() as u16;
    let Some(next) = action_words(action) else { return y + 1 };
    let used = (at - x) as usize;
    if used + 3 + next.cells() <= width {
        line(buf, at, y, format!("   {next}"), style(Role::Text)); // i18n-skip
        y + 1
    } else {
        line(buf, x, y + 1, truncate(&next, width), style(Role::Text));
        y + 2
    }
}

/// One chapter: its name left, the engine's line right. Read chapters recede; offline, the ones
/// not downloaded do. A new chapter and the one in progress carry the state role, a new one with
/// its mark as well, which NO_COLOR keeps.
#[allow(clippy::too_many_arguments)]
fn chapter(
    buf: &mut Buffer,
    app: &App,
    area: Rect,
    y: u16,
    row: &ChapterRow,
    selected: bool,
    focused: bool,
    offline: bool,
) {
    let style = |role: Role| app.palette.style(role);
    let x = area.left() + 2;
    let width = area.width.saturating_sub(3) as usize;
    if selected {
        let role = if focused { Role::Obi } else { Role::TextDim };
        line(buf, area.left(), y, glyph::OBI.to_string(), style(role));
    }
    let in_progress = matches!(row.state, ChapterReadState::InProgress { .. });
    let fresh = row.new_chapter && !offline;
    let mut trailing = chapter_line(row);
    if fresh && !trailing.is_empty() {
        trailing = format!("{} {trailing}", glyph::STATE_READY); // i18n-skip
    }
    let trailing = truncate(&trailing, width / 2);
    let trailing_role = if fresh || in_progress { Role::State } else { Role::TextDim };
    let receded = if offline { !downloaded(row) } else { row.state == ChapterReadState::Read };
    let label_style = if selected {
        style(Role::Text).add_modifier(Modifier::BOLD)
    } else if receded {
        style(Role::TextDim)
    } else {
        style(Role::Text)
    };
    let room = width.saturating_sub(trailing.cells() + 2);
    line(buf, x, y, truncate(&chapter_name(row), room), label_style);
    if !trailing.is_empty() {
        let tx = x + (width - trailing.cells()) as u16;
        line(buf, tx, y, &trailing, style(trailing_role));
    }
}

/// "▲ Couldn't load chapters from Home library.   r try again".
fn failed_line(
    buf: &mut Buffer,
    app: &App,
    x: u16,
    y: u16,
    width: usize,
    reason: &str,
    retry: &str,
) {
    let style = |role: Role| app.palette.style(role);
    let reason = format!("{} {reason}", glyph::STATE_DEGRADED); // i18n-skip
    let room = width.saturating_sub(retry.cells() + 3);
    let reason = truncate(&reason, room);
    line(buf, x, y, &reason, style(Role::TextDim));
    line(buf, x + reason.cells() as u16 + 3, y, retry, style(Role::TextDim));
}
