//! Reading's Home and Search as text: Home's shelves as headed lines, and Search's field, its
//! filter line, AniList's results and then each of the person's sources with a line per hit.
//! What a line says comes from [`App::read_lines`]; this sets it in columns and windows it.

use kaeri_i18n::t;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Widget,
};

use crate::{
    app::{App, Field, ReadLine, read_filter_summary},
    keymap::Action,
    layout::NARROW_WIDTH,
    nav::{Focus, Section},
    theme::{Role, glyph},
    widgets::{Cells, ObiList, ObiRow, truncate, wrap},
};

use super::{
    line,
    lists::{plate_row, render_loading},
    overlays::field_text,
};

/// The widest Home's lines run, so a title and its line stay within a glance of each other.
const HOME_WIDTH: u16 = 96;

/// Reading's Home or Search in `area`.
pub(super) fn render_reading(buf: &mut Buffer, app: &App, area: Rect) {
    if area.height == 0 || area.width < 8 {
        return;
    }
    match app.nav.section() {
        Section::Home => render_home(buf, app, area),
        _ => render_search(buf, app, area),
    }
}

/// Home: the person's shelves and the discovery rows, then the lines that say reading itself is in
/// the apps.
fn render_home(buf: &mut Buffer, app: &App, area: Rect) {
    if app.reading.home.is_none() {
        if app.read_waited() {
            render_loading(buf, app, area);
        }
        return;
    }
    let area = Rect { width: area.width.min(HOME_WIDTH), ..area };
    let width = area.width.saturating_sub(3) as usize;
    let selected = app.cursors.home.selected;
    let waited = app.read_waited();
    let mut rows: Vec<ObiRow> = Vec::new();
    let mut drawn: Vec<(usize, Option<usize>)> = Vec::new();
    let mut head: Option<usize> = None;
    for item in app.read_lines() {
        let n = drawn.len();
        match item {
            ReadLine::Gap => rows.push(ObiRow::new(String::new())),
            ReadLine::Shelf { title, side } => {
                head = Some(rows.len());
                let text = match side {
                    Some(side) => format!("{title} · {side}"), // i18n-skip
                    None => title,
                };
                rows.push(ObiRow::heading(text).ruled());
            }
            ReadLine::Quiet(text) | ReadLine::Alert(text) => {
                for text in wrap(&text, width, 3) {
                    rows.push(ObiRow::new(text).muted(true));
                }
            }
            // Held at the size the titles will take, so nothing below moves when they come.
            ReadLine::Placeholder => {
                for share in [60, 44] {
                    let text =
                        if waited { plate_row(width * share / 100) } else { String::new() };
                    rows.push(ObiRow::new(text).muted(true));
                }
            }
            ReadLine::Work { title, facts, fresh, .. } => {
                drawn.push((rows.len(), head.take()));
                let mut row = ObiRow::new(title).selected(n == selected).fresh(fresh);
                if !facts.is_empty() {
                    row = row.trailing(truncate(&facts, width / 2));
                }
                rows.push(row);
            }
            ReadLine::ViewAll { .. } => {
                drawn.push((rows.len(), head.take()));
                rows.push(
                    ObiRow::new(t::home_view_all())
                        .selected(n == selected)
                        .muted(n != selected),
                );
            }
            _ => {}
        }
    }
    // The terminal has no reader: where reading is, and the key that changes what Kaeri is for.
    if !rows.is_empty() {
        rows.push(ObiRow::new(String::new()));
    }
    for text in wrap(&t::tui_medium_read_line(), width, 3) {
        rows.push(ObiRow::new(text).muted(true));
    }
    rows.push(
        ObiRow::new(t::tui_medium_read_change(&app.key_for(Action::ChooseMedium))).muted(true),
    );

    let start =
        window(&drawn, selected, app.cursors.home.offset, rows.len(), area.height as usize);
    let shown: Vec<ObiRow> = rows.into_iter().skip(start).collect();
    ObiList::new(&shown, &app.palette)
        .focused(app.nav.focus() == Focus::Stage)
        .render(area, buf);
}

/// The first row drawn: from the cursor's offset, moved as little as keeps the highlighted line in
/// view with its heading, everything above it while the cursor is on the first line.
fn window(
    drawn: &[(usize, Option<usize>)],
    selected: usize,
    offset: usize,
    rows: usize,
    visible: usize,
) -> usize {
    let top_of = |n: usize| drawn.get(n).map_or(0, |(row, head)| head.unwrap_or(*row));
    let at = drawn.get(selected).map_or(0, |(row, _)| *row);
    let mut start = top_of(offset.min(selected));
    if top_of(selected) < start {
        start = top_of(selected);
    }
    if at >= start + visible {
        start = at + 1 - visible;
    }
    if selected == 0 {
        start = 0;
    }
    start.min(rows.saturating_sub(visible))
}

/// One drawn row of Search's results: its runs of text, each at a column.
#[derive(Default)]
struct Row {
    runs: Vec<(u16, String, Style)>,
    /// The line the cursor walks, when it is one.
    item: Option<usize>,
}

impl Row {
    fn run(mut self, x: u16, text: impl Into<String>, style: Style) -> Self {
        self.runs.push((x, text.into(), style));
        self
    }
}

/// Search: the field, the filter line when there is room, then the results.
fn render_search(buf: &mut Buffer, app: &App, area: Rect) {
    let wide = buf.area().width >= NARROW_WIDTH;
    let palette = &app.palette;
    let style = |role: Role| palette.style(role);
    let bold = |role: Role| palette.style(role).add_modifier(Modifier::BOLD);
    let x0 = area.left();
    let room = area.width as usize;

    // "/ lantern▏": the slash in the accent, the words in bold.
    let mut y = area.top();
    line(buf, x0, y, "/", style(Role::Obi)); // i18n-skip
    let text = field_text(app, Field::Search);
    let typing = app.is_typing();
    if text.is_empty() && !typing {
        line(
            buf,
            x0 + 2,
            y,
            truncate(&t::read_search_placeholder(), room.saturating_sub(2)),
            style(Role::TextDim),
        );
    } else {
        let shown = truncate(&text, room.saturating_sub(3));
        line(buf, x0 + 2, y, &shown, bold(Role::Text));
        if typing {
            let caret = x0 + 2 + shown.cells() as u16;
            line(buf, caret, y, glyph::OBI_THIN.to_string(), style(Role::Obi));
        }
    }
    y += 1;
    // The filters and Your sources only; a narrow screen leaves the line out, and `s` still works.
    if wide {
        let summary = read_filter_summary(&app.reading.filter);
        let filters = if summary.is_empty() {
            t::tui_read_filters_none()
        } else {
            t::tui_read_filters(&summary)
        };
        let mut x = x0;
        let mut put = |text: String, style: Style| {
            line(buf, x, y, &text, style);
            x += text.cells() as u16;
        };
        put(filters, style(Role::TextDim));
        put("  ·  ".to_string(), style(Role::Rule)); // i18n-skip
        put(format!("{} ", t::tui_read_sources_only()), style(Role::TextDim)); // i18n-skip
        let state =
            if app.reading.sources_only() { t::tui_read_on() } else { t::tui_read_off() };
        put(state, bold(Role::Text));
        y += 1;
    }
    y += 1;
    let body = Rect { y, height: area.bottom().saturating_sub(y), ..area };
    if body.height == 0 {
        return;
    }

    // Columns: the obi, the title, the facts, and beside them the sources that have it.
    let title_x = x0 + 2;
    let title_w: usize = if wide { 36 } else { 32 };
    let facts_x = title_x + title_w as u16;
    let facts_w: usize = if wide { 32 } else { room.saturating_sub(title_w + 2) };
    let waited = app.read_waited();
    let frame = app.pulse_frame();
    let selected = app.cursors.search.selected;

    let mut rows: Vec<Row> = Vec::new();
    let mut drawn: Vec<(usize, Option<usize>)> = Vec::new();
    let mut head: Option<usize> = None;
    let item = |rows: &mut Vec<Row>,
                drawn: &mut Vec<(usize, Option<usize>)>,
                head: &mut Option<usize>,
                row: Row| {
        let n = drawn.len();
        drawn.push((rows.len(), head.take()));
        rows.push(Row { item: Some(n), ..row });
    };
    for read in app.read_lines() {
        match read {
            ReadLine::Gap => rows.push(Row::default()),
            ReadLine::AnilistHeading(text) => {
                head = Some(rows.len());
                rows.push(Row::default().run(x0, text, style(Role::TextDim)));
            }
            ReadLine::AnilistAsking => {
                let row = if waited {
                    Row::default()
                        .run(title_x, glyph::spinner(frame).to_string(), style(Role::State))
                        .run(
                            title_x + 2,
                            t::read_search_asking(&t::tui_read_anilist()),
                            style(Role::TextDim),
                        )
                } else {
                    Row::default()
                };
                rows.push(row);
            }
            ReadLine::Alert(text) => {
                rows.push(Row::default().run(x0, truncate(&text, room), style(Role::Alert)));
            }
            ReadLine::Quiet(text) => {
                for text in wrap(&text, room, 3) {
                    rows.push(Row::default().run(x0, text, style(Role::TextDim)));
                }
            }
            ReadLine::Work { title, facts, joined, .. } => {
                let n = drawn.len();
                let title_style =
                    if n == selected { bold(Role::Text) } else { style(Role::Text) };
                let mut row = Row::default()
                    .run(title_x, truncate(&title, title_w - 1), title_style)
                    .run(facts_x, truncate(&facts, facts_w), style(Role::TextDim));
                let tag_w = if wide {
                    room.saturating_sub(title_w + facts_w + 4)
                } else {
                    room.saturating_sub(6)
                };
                let tag = joined.map(|joined| truncate(&joined, tag_w));
                if wide && let Some(tag) = &tag {
                    row =
                        row.run(facts_x + facts_w as u16 + 2, tag.clone(), style(Role::State));
                }
                item(&mut rows, &mut drawn, &mut head, row);
                // Narrow, the sources that have it take a line of their own under it.
                if !wide && let Some(joined) = joined_line(&tag) {
                    rows.push(Row::default().run(title_x + 2, joined, style(Role::State)));
                }
            }
            ReadLine::MoreAnilist => {
                let row = Row::default().run(
                    title_x,
                    t::read_search_more_anilist(),
                    style(Role::TextDim),
                );
                item(&mut rows, &mut drawn, &mut head, row);
            }
            ReadLine::SourcesHeading => {
                head = Some(rows.len());
                rows.push(Row::default().run(
                    x0,
                    t::read_search_from_sources(),
                    bold(Role::Text),
                ));
            }
            ReadLine::Group { name, kind } => {
                if head.is_none() {
                    head = Some(rows.len());
                }
                let mut row = Row::default().run(x0, truncate(&name, room), bold(Role::Text));
                if let Some(kind) = kind {
                    row = row.run(x0 + name.cells() as u16 + 2, kind, style(Role::TextDim));
                }
                rows.push(row);
            }
            ReadLine::Asking { name } => {
                let row = if waited {
                    Row::default()
                        .run(title_x, glyph::spinner(frame).to_string(), style(Role::State))
                        .run(
                            title_x + 2,
                            truncate(&t::read_search_asking(&name), room.saturating_sub(4)),
                            style(Role::TextDim),
                        )
                } else {
                    Row::default()
                };
                rows.push(row);
            }
            ReadLine::SourceFailed { reason, .. } => {
                let key = app.key_for(Action::Refresh);
                let retry = format!("{key} {}", t::tui_read_try_again()); // i18n-skip
                let reason_w = room.saturating_sub(4 + retry.cells() + 3);
                let reason = truncate(&reason, reason_w);
                let retry_x = title_x + 2 + reason.cells() as u16 + 3;
                let row = Row::default()
                    .run(title_x, glyph::STATE_DEGRADED.to_string(), style(Role::Alert))
                    .run(title_x + 2, reason, style(Role::Text))
                    .run(retry_x, key.clone(), bold(Role::Text))
                    .run(
                        retry_x + key.cells() as u16 + 1,
                        t::tui_read_try_again(),
                        style(Role::TextDim),
                    );
                item(&mut rows, &mut drawn, &mut head, row);
            }
            ReadLine::ShowMore { n, .. } => {
                let row = Row::default().run(
                    title_x,
                    t::read_search_source_more(n as i64),
                    style(Role::TextDim),
                );
                item(&mut rows, &mut drawn, &mut head, row);
            }
            _ => {}
        }
    }

    let visible = body.height as usize;
    let start = window(&drawn, selected, app.cursors.search.offset, rows.len(), visible);
    let focused = app.nav.focus() == Focus::Stage;
    for (i, row) in rows.iter().skip(start).take(visible).enumerate() {
        let y = body.top() + i as u16;
        if row.item == Some(selected) {
            let role = if focused { Role::Obi } else { Role::TextDim };
            line(buf, x0, y, glyph::OBI.to_string(), style(role));
        }
        for (x, text, run_style) in &row.runs {
            if *x < body.right() {
                let fit = truncate(text, (body.right() - x) as usize);
                line(buf, *x, y, fit, *run_style);
            }
        }
    }
}

/// The narrow screen's line under a joined result.
fn joined_line(tag: &Option<String>) -> Option<String> {
    tag.clone().filter(|tag| !tag.is_empty())
}
