//! The Together screen: the watch party this terminal is in, a line to join another, and the
//! last rooms. Only metadata crosses the server: names, a file name and where it is up to.

use kaeri_i18n::t;
use kaeri_model::party::{
    PartyChatBody, PartyChatLine, PartyConnection, PartyRecentRoom, PartySources, Reaction,
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::{App, say},
    nav::Focus,
    theme::{Role, glyph},
    widgets::{Hairline, truncate, wrap},
};

use super::{
    MIN_TABLE_HEIGHT,
    chrome::{mark_selected, render_placeholder},
};

/// The width from which the stage keeps a detail pane beside the party.
const DETAIL_FROM: u16 = 80;
/// The detail pane's width.
const DETAIL_WIDTH: u16 = 38;

/// The Together screen.
pub(super) fn render_together(buf: &mut Buffer, app: &App, area: Rect) {
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
        let main = Rect { width: area.width - DETAIL_WIDTH - 3, ..area };
        let detail = Rect { x: main.right() + 3, width: DETAIL_WIDTH, ..area };
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
            let (count, ready) = (
                app.party_members.len(),
                app.party_members.iter().filter(|m| m.ready == Some(true)).count(),
            );
            if let Some(room) = &party.room_episode {
                let line = t::together_in_party_line(&room.episode, count as i64, ready as i64);
                buf.set_string(main.left(), y, truncate(&line, width), dim);
                y += 1;
            }
            let code = party.code.as_deref().unwrap_or(&party.room);
            buf.set_string(main.left(), y, truncate(code, width), text);
            y += 2;
            for member in app.party_members.iter().take(4) {
                if y >= main.bottom().saturating_sub(5) {
                    break;
                }
                let (mark, state, role) = match member.ready {
                    Some(true) => (glyph::STATE_READY, t::party_member_ready(), Role::State),
                    Some(false) => {
                        (glyph::STATE_OFF, t::party_member_not_ready(), Role::TextDim)
                    }
                    None => {
                        (glyph::STATE_UNKNOWN, t::party_member_no_ready_state(), Role::TextDim)
                    }
                };
                let name = if member.you {
                    t::party_member_you(&member.name)
                } else {
                    member.name.clone()
                };
                buf.set_string(main.left(), y, mark.to_string(), app.palette.style(role));
                buf.set_string(
                    main.left() + 2,
                    y,
                    truncate(&name, width.saturating_sub(16).max(8)),
                    text,
                );
                let x = main.left() + (width.saturating_sub(14).max(10)) as u16;
                if x + 4 < main.right() {
                    buf.set_string(
                        x,
                        y,
                        truncate(&state, main.right().saturating_sub(x) as usize),
                        app.palette.style(role),
                    );
                }
                y += 1;
            }
            if app.party_members.len() <= 1 {
                buf.set_string(main.left(), y, truncate(&t::party_alone(), width), dim);
                y += 1;
            }
            y += 1;
            if let Some(line) = status_line(app) {
                buf.set_string(main.left(), y, truncate(&line, width), text);
                y += 1;
            }
            if let Some(sources) = sources_line(app) {
                buf.set_string(main.left(), y, truncate(&sources, width), dim);
                y += 1;
            }
            y += 1;
        }
        None => {
            for line in wrap(&t::together_start_line(), width, 2) {
                buf.set_string(main.left(), y, line, dim);
                y += 1;
            }
            y += 1;
        }
    }

    // Without the detail pane the conversation sits here, above the Join line, as much as fits.
    if detail.is_none() && app.party.is_some() && y + 8 < main.bottom() {
        let room = Rect { y, height: (main.bottom() - y).saturating_sub(6).min(8), ..main };
        y += render_chat(buf, app, room) + 1;
    }

    // Row 0: the Join line.
    if y >= main.bottom() {
        return;
    }
    let selected = app.cursors.together.selected == 0;
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
        return;
    }
    buf.set_string(main.left(), y, truncate(&t::together_recent(), width), dim);
    y += 1;
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..main }, buf);
    y += 1;
    if app.party_recent.is_empty() {
        buf.set_string(main.left(), y, truncate(&t::together_recent_empty(), width), dim);
    }
    for (i, room) in app.party_recent.iter().enumerate() {
        // Two rows each: the title, and who was there when.
        if y + 1 >= main.bottom() {
            break;
        }
        let selected = app.cursors.together.selected == i + 1;
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

/// The pane beside the party at wide sizes: where to find it, and how the connection is.
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
        put(buf, &mut y, &t::party_copy_invite(), dim);
        if let Some(link) = party.link.as_deref().or(party.invite.as_deref()) {
            for line in wrap(link, width, 4) {
                put(buf, &mut y, &line, text);
            }
        }
        y += 1;
        put(buf, &mut y, &t::party_server(), dim);
        put(buf, &mut y, &party.server, text);
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
        put(buf, &mut y, &link, dim);
        y += 1;
        for line in wrap(&t::tui_together_keys(), width, 3) {
            put(buf, &mut y, &line, dim);
        }
        y += 1;
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

/// The conversation: the newest lines that fit, a reaction as a dim line, a muted member's line
/// as "Ren is muted", "Couldn't send" after a line the server never echoed, and the privacy
/// line when there is room. When the room's server has no chat it says so and nothing else.
/// Returns the rows used.
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
    if app.party_chat_state.as_ref().is_some_and(|s| !s.allowed) {
        if y < area.bottom() {
            buf.set_string(area.left(), y, truncate(&t::party_chat_off(), width), dim);
            y += 1;
        }
        return y - area.top();
    }
    // Rows for the lines: what is left after the failure and the privacy line.
    let failed = app.party_chat.failed.is_some();
    let reserve = u16::from(failed) + u16::from(area.height >= 6);
    let rows = area.bottom().saturating_sub(y).saturating_sub(reserve) as usize;
    let mut shown: Vec<(String, bool)> = Vec::new();
    for line in &app.party_chat.lines {
        let who = if line.you { t::party_chat_you() } else { line.from.clone() };
        if line.muted {
            shown.push((t::party_muted_line(&line.from), true));
            continue;
        }
        match &line.body {
            PartyChatBody::Said { text } => shown.push((format!("{who}  {text}"), false)),
            PartyChatBody::Reacted { .. } => {
                if let Some(said) = reaction_line(line, &who) {
                    shown.push((said, true));
                }
            }
        }
    }
    if shown.is_empty() && rows > 0 {
        buf.set_string(area.left(), y, truncate(&t::party_chat_field(), width), dim);
        y += 1;
    }
    let skip = shown.len().saturating_sub(rows);
    for (line, quiet) in shown.into_iter().skip(skip) {
        buf.set_string(area.left(), y, truncate(&line, width), if quiet { dim } else { text });
        y += 1;
    }
    if failed && y < area.bottom() {
        let line = format!("{} · {}", t::party_chat_not_sent(), t::party_chat_retry());
        buf.set_string(area.left(), y, truncate(&line, width), app.palette.style(Role::Alert));
        y += 1;
    }
    if area.height >= 6 && y < area.bottom() {
        buf.set_string(area.left(), y, truncate(&t::party_chat_privacy(), width), dim);
        y += 1;
    }
    y - area.top()
}
