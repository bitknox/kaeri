//! The Together screen: the watch party this terminal is in, a line to join another, and the
//! last rooms. Only metadata crosses the server: names, a file name and where it is up to.

use kaeri_i18n::t;
use kaeri_model::party::{
    PartyChatBody, PartyChatLine, PartyConnection, PartyRecentRoom, PartySources, Reaction,
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::{App, say},
    nav::{Focus, Overlay},
    theme::{Role, glyph},
    widgets::{Cells, Hairline, truncate, wrap},
};

use super::{
    MIN_TABLE_HEIGHT,
    chrome::{mark_selected, render_placeholder},
};

/// The width from which the stage keeps a detail pane beside the list.
const DETAIL_FROM: u16 = 80;
/// The list's width when the detail pane is beside it: the party, its members, the Join line
/// and the recent rooms. The pane takes the rest, for the party and its conversation.
const LIST_WIDTH: u16 = 36;
/// The widest a name's column grows in the conversation.
const NAME_COLUMN: usize = 12;

/// The Together screen.
pub(super) fn render_together(buf: &mut Buffer, app: &App, area: Rect) {
    // The Say line takes the stage's last two rows; the conversation ends above it.
    let area = if app.nav.overlay() == Some(&Overlay::Say) {
        Rect { height: area.height.saturating_sub(2), ..area }
    } else {
        area
    };
    if area.height < MIN_TABLE_HEIGHT || area.width < 40 {
        render_placeholder(
            buf,
            app,
            area,
            &t::tui_together_section(),
            &t::lists_terminal_too_small(),
        );
        return;
    }
    let (main, detail) = if area.width >= DETAIL_FROM {
        let main = Rect { width: LIST_WIDTH, ..area };
        let detail = Rect { x: main.right() + 3, width: area.width - LIST_WIDTH - 3, ..area };
        (main, Some(detail))
    } else {
        (area, None)
    };
    let focused = app.nav.focus() == Focus::Stage;
    let dim = app.palette.style(Role::TextDim);
    let text = app.palette.style(Role::Text);
    let width = main.width as usize;
    let mut y = main.top();

    buf.set_string(main.left(), y, truncate(&t::together_title(), width), dim);
    y += 1;
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..main }, buf);
    y += 1;

    // The party this terminal is in, or what to do to be in one.
    match &app.party {
        Some(party) => {
            let title = party
                .room_episode
                .as_ref()
                .map_or_else(|| party.room.clone(), |room| room.title.clone());
            buf.set_string(
                main.left(),
                y,
                truncate(&title, width),
                text.add_modifier(Modifier::BOLD),
            );
            y += 1;
            if detail.is_none()
                && let Some(line) = episode_line(app)
            {
                buf.set_string(main.left(), y, truncate(&line, width), dim);
                y += 1;
            }
            let code = party.code.as_deref().unwrap_or(&party.room);
            buf.set_string(main.left(), y, truncate(code, width), dim);
            y += 2;
            y = render_members(buf, app, main, y, focused);
            if app.party_members.len() <= 1 {
                buf.set_string(main.left() + 2, y, truncate(&t::party_alone(), width), dim);
                y += 1;
            }
            y += 1;
            if detail.is_none() {
                y = render_standing(buf, app, main, y) + 1;
            }
        }
        None => {
            for line in wrap(&t::together_start_line(), width, 3) {
                buf.set_string(main.left(), y, line, dim);
                y += 1;
            }
            y += 1;
        }
    }

    // Without the detail pane the conversation sits here, above the Join line, as much as fits.
    // While the Say line is open it takes every row left: the Join line and the rooms can wait.
    if detail.is_none() && app.party.is_some() {
        let saying = app.nav.overlay() == Some(&Overlay::Say);
        if saying && y + 3 < main.bottom() {
            render_chat(buf, app, Rect { y, height: main.bottom() - y, ..main });
            return;
        }
        if y + 8 < main.bottom() {
            let room = Rect { y, height: (main.bottom() - y).saturating_sub(6).min(8), ..main };
            y += render_chat(buf, app, room) + 1;
        }
    }

    // The Join line.
    if y >= main.bottom() {
        return;
    }
    let selected = app.cursors.together.selected == app.join_row();
    if selected {
        mark_selected(buf, app, main.left(), y, focused);
    }
    let label = t::together_join();
    buf.set_string(
        main.left() + 2,
        y,
        truncate(&label, width),
        if selected { text.add_modifier(Modifier::BOLD) } else { text },
    );
    let x = main.left() + 2 + label.chars().count() as u16 + 2;
    if x + 4 < main.right() {
        buf.set_string(
            x,
            y,
            truncate(
                &format!("[{}]", t::tui_together_join_prompt()),
                main.right().saturating_sub(x) as usize,
            ),
            dim,
        );
    }
    y += 2;

    // The last rooms.
    if y + 1 >= main.bottom() {
        if let Some(detail) = detail {
            render_detail(buf, app, detail);
        }
        return;
    }
    buf.set_string(main.left(), y, truncate(&t::together_recent(), width), dim);
    y += 1;
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..main }, buf);
    y += 1;
    if app.party_recent.is_empty() {
        buf.set_string(main.left(), y, truncate(&t::together_recent_empty(), width), dim);
    }
    let first = app.join_row() + 1;
    for (i, room) in app.party_recent.iter().enumerate() {
        // Two rows each: the title, and who was there when.
        if y + 1 >= main.bottom() {
            break;
        }
        let selected = app.cursors.together.selected == first + i;
        if selected {
            mark_selected(buf, app, main.left(), y, focused);
        }
        buf.set_string(
            main.left() + 2,
            y,
            truncate(&room.title, width.saturating_sub(2)),
            if selected { text.add_modifier(Modifier::BOLD) } else { text },
        );
        buf.set_string(
            main.left() + 2,
            y + 1,
            truncate(&recent_line(app, room), width.saturating_sub(2)),
            dim,
        );
        y += 2;
    }

    if let Some(detail) = detail {
        render_detail(buf, app, detail);
    }
}

/// The members, this device's first: a mark for their ready state, the name, and the state,
/// or "Muted". The others are rows the cursor walks, so `M` can mute one. Returns the next row.
fn render_members(buf: &mut Buffer, app: &App, area: Rect, mut y: u16, focused: bool) -> u16 {
    let dim = app.palette.style(Role::TextDim);
    let text = app.palette.style(Role::Text);
    let width = area.width as usize;
    let state_x = area.left() + (width.saturating_sub(14).max(14)) as u16;
    let mut row = 0;
    let members = app
        .party_members
        .iter()
        .filter(|m| m.you)
        .chain(app.party_members.iter().filter(|m| !m.you));
    for member in members {
        if y >= area.bottom().saturating_sub(5) {
            break;
        }
        let selected = !member.you && app.cursors.together.selected == row;
        if !member.you {
            row += 1;
        }
        if selected {
            mark_selected(buf, app, area.left(), y, focused);
        }
        let muted = !member.you && app.is_muted(&member.name);
        let (mark, state, role) = match member.ready {
            _ if muted => (glyph::STATE_OFF, t::party_member_muted(), Role::TextDim),
            Some(true) => (glyph::STATE_READY, t::party_member_ready(), Role::State),
            Some(false) => (glyph::STATE_OFF, t::party_member_not_ready(), Role::TextDim),
            None => (glyph::STATE_UNKNOWN, t::party_member_no_ready_state(), Role::TextDim),
        };
        let name =
            if member.you { t::party_member_you(&member.name) } else { member.name.clone() };
        buf.set_string(area.left() + 2, y, mark.to_string(), app.palette.style(role));
        let name_style = match (selected, muted) {
            (_, true) => dim,
            (true, false) => text.add_modifier(Modifier::BOLD),
            (false, false) => text,
        };
        buf.set_string(
            area.left() + 4,
            y,
            truncate(&name, (state_x - area.left()).saturating_sub(5) as usize),
            name_style,
        );
        if state_x + 4 < area.right() {
            buf.set_string(
                state_x,
                y,
                truncate(&state, area.right().saturating_sub(state_x) as usize),
                app.palette.style(role),
            );
        }
        y += 1;
    }
    y
}

/// "Episode 9 · 3 in the room · 1 ready".
fn episode_line(app: &App) -> Option<String> {
    let room = app.party.as_ref()?.room_episode.as_ref()?;
    let (count, ready) = (
        app.party_members.len(),
        app.party_members.iter().filter(|m| m.ready == Some(true)).count(),
    );
    Some(t::together_in_party_line(&room.episode, count as i64, ready as i64))
}

/// How the room stands and whether this device's sources have its episode. Returns the next
/// row.
fn render_standing(buf: &mut Buffer, app: &App, area: Rect, mut y: u16) -> u16 {
    let width = area.width as usize;
    if let Some(line) = status_line(app)
        && y < area.bottom()
    {
        buf.set_string(area.left(), y, truncate(&line, width), app.palette.style(Role::Text));
        y += 1;
    }
    if let Some(sources) = sources_line(app)
        && y < area.bottom()
    {
        buf.set_string(
            area.left(),
            y,
            truncate(&sources, width),
            app.palette.style(Role::TextDim),
        );
        y += 1;
    }
    y
}

/// How the room stands: counting down, everyone ready, or who it waits for.
fn status_line(app: &App) -> Option<String> {
    let party = app.party.as_ref()?;
    if let Some(seconds) = party.starting_in {
        return Some(t::party_starting(i64::from(seconds)));
    }
    if party.playing {
        return None;
    }
    let waiting: Vec<&str> = app
        .party_members
        .iter()
        .filter(|m| m.ready == Some(false))
        .map(|m| m.name.as_str())
        .collect();
    if waiting.is_empty() {
        return (!app.party_members.is_empty()).then(t::party_all_ready);
    }
    if app.party_members.iter().any(|m| m.you && m.ready == Some(false)) {
        return Some(t::party_wait_you());
    }
    Some(match waiting.as_slice() {
        [one] => t::party_wait_one(one),
        [first, second] => t::party_wait_two(first, second),
        many => t::party_wait_many(many.len() as i64),
    })
}

/// Whether this device's sources have the room's episode.
fn sources_line(app: &App) -> Option<String> {
    Some(match &app.party.as_ref()?.sources.as_ref()? {
        PartySources::Checking => t::party_looking_sources(),
        PartySources::Found => t::party_lobby_sources_found(),
        PartySources::Missing { reason, .. } => say(reason),
    })
}

/// A recent room's second line: the episode it reached, who was there and when.
fn recent_line(app: &App, room: &PartyRecentRoom) -> String {
    let when = when(app, room.last_at);
    if room.names.is_empty() {
        return t::tui_together_recent_alone(&room.episode, &when);
    }
    t::together_recent_row(&room.episode, &room.names.join(", "), &when)
}

/// A day, by the clock the app runs on: today, yesterday, a weekday, or the date.
fn when(app: &App, epoch: i64) -> String {
    let Some(day) = app.clock.date_of(epoch) else { return String::new() };
    match (day - app.clock.today()).num_days() {
        0 => t::lists_day_today(),
        -1 => t::lists_day_yesterday(),
        -6..=-2 => day.format("%A").to_string(),
        _ => day.format("%-d %b").to_string(),
    }
}

/// The pane beside the list at wide sizes. In a party: what the room plays, where it is and how
/// the connection is, how the room stands, then the conversation. Otherwise where the
/// highlighted recent room was.
fn render_detail(buf: &mut Buffer, app: &App, area: Rect) {
    let dim = app.palette.style(Role::TextDim);
    let text = app.palette.style(Role::Text);
    let width = area.width as usize;
    let mut y = area.top();
    let put = |buf: &mut Buffer, y: &mut u16, line: &str, style| {
        if *y < area.bottom() {
            buf.set_string(area.left(), *y, truncate(line, width), style);
            *y += 1;
        }
    };
    if let Some(party) = &app.party {
        let title = party
            .room_episode
            .as_ref()
            .map_or_else(|| party.room.clone(), |room| room.title.clone());
        put(buf, &mut y, &title, text.add_modifier(Modifier::BOLD));
        if let Some(line) = episode_line(app) {
            put(buf, &mut y, &line, dim);
        }
        let link = match party.connection {
            PartyConnection::Connected if party.encrypted => t::party_connected_encrypted(),
            PartyConnection::Connected => t::party_connected_plain(),
            PartyConnection::Reconnecting { attempt, of } => {
                t::party_connection_lost(i64::from(attempt), i64::from(of))
            }
        };
        let link = match party.latency_ms {
            Some(ms) => t::party_connected_latency(&link, i64::from(ms)),
            None => link,
        };
        // The server and the connection on one line when they fit, else one under the other.
        let both = format!("{} · {link}", party.server); // i18n-skip
        if both.cells() <= width {
            put(buf, &mut y, &both, dim);
        } else {
            put(buf, &mut y, &party.server, dim);
            put(buf, &mut y, &link, dim);
        }
        y += 1;
        let before = y;
        y = render_standing(buf, app, area, y);
        if y > before {
            y += 1;
        }
        if y + 3 < area.bottom() {
            let room = Rect { y, height: area.bottom() - y, ..area };
            render_chat(buf, app, room);
        }
    } else if let Some(room) = app.selected_room() {
        put(buf, &mut y, &t::party_server(), dim);
        put(buf, &mut y, &room.server, text);
        y += 1;
        put(buf, &mut y, &t::party_room(), dim);
        put(buf, &mut y, &room.room, text);
    }
}

/// A reaction line as the conversation says it: "Ren and Mika laughed at 12:04".
fn reaction_line(line: &PartyChatLine, who: &str) -> Option<String> {
    let PartyChatBody::Reacted { reaction, at, with } = &line.body else { return None };
    let names = match with.as_slice() {
        [] => who.to_owned(),
        [second] => t::party_names_two(who, second),
        [second, third] => t::party_names_three(who, second, third),
        [second, rest @ ..] => t::party_names_others(who, second, rest.len() as i64),
    };
    let did = match reaction {
        Reaction::Laugh => t::party_reacted_laugh(&names),
        Reaction::Gasp => t::party_reacted_gasp(&names),
        Reaction::Clap => t::party_reacted_clap(&names),
        Reaction::Cry => t::party_reacted_cry(&names),
    };
    let total = at.max(0.0).round() as i64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    let time = if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") };
    Some(t::party_reacted_at(&did, &time))
}

/// One row of the conversation: the name in its column (bold, "You" dim), then the words; or
/// a whole dim line.
enum ChatRow {
    Said { who: String, you: bool, words: String },
    Quiet(String),
}

/// The conversation: the newest lines that fit, each name in a column and its words wrapped
/// under the words, a reaction as a dim line, a muted member's line as "Ren is muted",
/// "Couldn't send" after a line the server never echoed, and the privacy line when there is
/// room. When the room's server has no chat it says so and nothing else; with Hide chat on, it
/// says that instead. Returns the rows used.
fn render_chat(buf: &mut Buffer, app: &App, area: Rect) -> u16 {
    let dim = app.palette.style(Role::TextDim);
    let text = app.palette.style(Role::Text);
    let width = area.width as usize;
    let mut y = area.top();
    buf.set_string(area.left(), y, truncate(&t::party_chat_title(), width), dim);
    y += 1;
    if y < area.bottom() {
        Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
        y += 1;
    }
    let quiet = |buf: &mut Buffer, y: &mut u16, line: &str, style| {
        for row in wrap(line, width, 2) {
            if *y < area.bottom() {
                buf.set_string(area.left(), *y, row, style);
                *y += 1;
            }
        }
    };
    if app.party_chat_state.as_ref().is_some_and(|s| !s.allowed) {
        quiet(buf, &mut y, &t::party_chat_off(), dim);
        return y - area.top();
    }
    if app.chat_hidden() {
        quiet(buf, &mut y, &t::party_chat_hidden(), text);
        quiet(buf, &mut y, &t::tui_together_chat_hidden_line(), dim);
        return y - area.top();
    }
    // Rows for the lines: what is left after the failure and the privacy line.
    let failed = app.party_chat.failed.is_some();
    let reserve = u16::from(failed) + if area.height >= 6 { 3 } else { 0 };
    let budget = area.bottom().saturating_sub(y).saturating_sub(reserve) as usize;

    let lines: Vec<(String, bool, &PartyChatLine)> = app
        .party_chat
        .lines
        .iter()
        .map(|line| {
            let who = if line.you { t::party_chat_you() } else { line.from.clone() };
            (who, line.you, line)
        })
        .collect();
    let column = lines
        .iter()
        .filter(|(_, _, line)| !line.muted && matches!(line.body, PartyChatBody::Said { .. }))
        .map(|(who, _, _)| who.cells())
        .max()
        .unwrap_or(0)
        .min(NAME_COLUMN);
    let words_width = width.saturating_sub(column + 2).max(8);
    let mut rows: Vec<ChatRow> = Vec::new();
    for (who, you, line) in &lines {
        if line.muted {
            rows.push(ChatRow::Quiet(t::party_muted_line(&line.from)));
            continue;
        }
        match &line.body {
            PartyChatBody::Said { text } => {
                for (i, part) in wrap(text, words_width, 3).into_iter().enumerate() {
                    let who = if i == 0 { who.clone() } else { String::new() };
                    rows.push(ChatRow::Said { who, you: *you, words: part });
                }
            }
            PartyChatBody::Reacted { .. } => {
                if let Some(said) = reaction_line(line, who) {
                    rows.push(ChatRow::Quiet(said));
                }
            }
        }
    }
    if rows.is_empty() && budget > 0 {
        buf.set_string(area.left(), y, truncate(&t::party_chat_field(), width), dim);
        y += 1;
    }
    let skip = rows.len().saturating_sub(budget);
    for row in rows.into_iter().skip(skip) {
        match row {
            ChatRow::Said { who, you, words } => {
                let style = if you { dim } else { text.add_modifier(Modifier::BOLD) };
                buf.set_string(area.left(), y, truncate(&who, column), style);
                buf.set_string(area.left() + column as u16 + 2, y, words, text);
            }
            ChatRow::Quiet(line) => {
                buf.set_string(area.left(), y, truncate(&line, width), dim);
            }
        }
        y += 1;
    }
    if failed && y < area.bottom() {
        let line = format!("{} · {}", t::party_chat_not_sent(), t::party_chat_retry()); // i18n-skip
        buf.set_string(area.left(), y, truncate(&line, width), app.palette.style(Role::Alert));
        y += 1;
    }
    if area.height >= 6 && y + 1 < area.bottom() {
        y += 1;
        quiet(buf, &mut y, &t::party_chat_privacy(), dim);
    }
    y - area.top()
}
