//! The Together screen: the room this terminal is in, a line to join another by its invite
//! link, and the last rooms. Each person plays from their own sources; the room's messages are
//! sealed end to end, and relays only pass them along.

use kaeri_i18n::t;
use kaeri_model::room::{
    MemberState, Reaction, RoomChatBody, RoomChatLine, RoomConnection, RoomMember, RoomRecent,
    RoomSources, RoomView,
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::{App, say},
    keymap::Action,
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
/// The list's width when the detail pane is beside it: the room, its people, the Join line and
/// the recent rooms. The pane takes the rest, for the room's line, its invite and the chat.
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
    // A line of the list on its row, or nothing when that row is below the stage. The screen
    // draws on a stage of four rows, which the room's lines and the start line outgrow, and
    // under the stage are the status rows or the end of the terminal.
    let put_line = |buf: &mut Buffer, x: u16, y: u16, line: &str, style| {
        if y < main.bottom() {
            buf.set_string(x, y, truncate(line, width), style);
        }
    };

    buf.set_string(main.left(), y, truncate(&t::tui_together_section(), width), dim);
    y += 1;
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..main }, buf);
    y += 1;

    // The room this terminal is in, or how to be in one.
    match &app.room {
        Some(room) => {
            put_line(buf, main.left(), y, &room_title(room), text.add_modifier(Modifier::BOLD));
            y += 1;
            if detail.is_none()
                && let Some(line) = header_line(app, room)
            {
                put_line(buf, main.left(), y, &line, dim);
                y += 1;
            }
            y += 1;
            y = render_members(buf, app, main, y, focused);
            y += 1;
            if detail.is_none() {
                y = render_standing(buf, app, room, main, y) + 1;
            }
        }
        None => {
            let key = app.key_for(Action::StartRoom);
            for line in wrap(&t::tui_room_start_line(&key), width, 4) {
                put_line(buf, main.left(), y, &line, dim);
                y += 1;
            }
            y += 1;
        }
    }

    // Without the detail pane the conversation sits here, above the Join line, as much as fits.
    // While the Say line is open it takes every row left: the Join line and the rooms can wait.
    if detail.is_none() && app.room.is_some() {
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

    // The Join line: Join anyway once a room said it was full.
    if y >= main.bottom() {
        return;
    }
    let selected = app.cursors.together.selected == app.join_row();
    if selected {
        mark_selected(buf, app, main.left(), y, focused);
    }
    let label =
        if app.room_full.is_some() { t::room_join_anyway() } else { t::room_place_join() };
    buf.set_string(
        main.left() + 2,
        y,
        truncate(&label, width),
        if selected { text.add_modifier(Modifier::BOLD) } else { text },
    );
    let x = main.left() + 2 + label.chars().count() as u16 + 2;
    if app.room_full.is_none() && x + 4 < main.right() {
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
    buf.set_string(main.left(), y, truncate(&t::room_recent(), width), dim);
    y += 1;
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..main }, buf);
    y += 1;
    if app.room_recent.is_empty() {
        put_line(buf, main.left(), y, &t::tui_room_recent_empty(), dim);
    }
    let first = app.join_row() + 1;
    for (i, room) in app.room_recent.iter().enumerate() {
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

/// What the room watches, or "Your room" while it is still being read.
fn room_title(room: &RoomView) -> String {
    room.room_episode.as_ref().map_or_else(t::room_place_yours, |e| e.title.clone())
}

/// "Episode 9 · 3 in the room".
fn header_line(app: &App, room: &RoomView) -> Option<String> {
    let episode = room.room_episode.as_ref()?;
    Some(t::room_header_line(number(&episode.episode), app.room_members.len() as i64))
}

/// Now, in the epoch milliseconds the room's times are in.
fn now_ms(app: &App) -> i64 {
    app.clock.now_epoch() * 1000
}

/// Whole seconds from `since_ms` to now; never below nothing.
fn seconds_since(app: &App, since_ms: i64) -> i64 {
    ((now_ms(app) - since_ms) / 1000).max(0)
}

/// An episode as the catalogue takes it: its label, "9" or "9.5".
fn number(episode: &str) -> &str {
    episode.trim()
}

/// A place in an episode: "12:04", "1:02:03".
fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as i64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Names as a sentence lists them: "Mika", "Mika and Ren", "Mika, Ren and Sol".
fn names(list: &[&str]) -> String {
    match list {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [before @ .., last] => {
            let joined = before
                .iter()
                .skip(1)
                .fold(before[0].to_owned(), |acc, name| t::room_names_comma(&acc, name));
            t::room_names_and(&joined, last)
        }
    }
}

/// The people the room waits for, inside "Paused for …": one or two by name, more counted.
fn waited_names(list: &[&str]) -> String {
    match list {
        [_, _, _, ..] => t::room_names_people(list.len() as i64),
        _ => names(list),
    }
}

/// The members, this device's first: a mark for how they are, the name, and their word. The
/// others are rows the cursor walks, so `M` can mute one. Returns the next row.
fn render_members(buf: &mut Buffer, app: &App, area: Rect, mut y: u16, focused: bool) -> u16 {
    let dim = app.palette.style(Role::TextDim);
    let text = app.palette.style(Role::Text);
    let width = area.width as usize;
    let state_x = area.left() + (width.saturating_sub(16).max(16)) as u16;
    let mut row = 0;
    let members = app
        .room_members
        .iter()
        .filter(|m| m.you)
        .chain(app.room_members.iter().filter(|m| !m.you));
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
        let (mark, role) = match member.state {
            MemberState::Watching | MemberState::Ready => (glyph::STATE_READY, Role::State),
            MemberState::Loading
            | MemberState::Buffering
            | MemberState::Behind
            | MemberState::Choosing
            | MemberState::Reconnecting => (glyph::STATE_UNKNOWN, Role::TextDim),
            MemberState::NoSource => (glyph::STATE_OFF, Role::Alert),
            MemberState::Paused
            | MemberState::OtherEpisode
            | MemberState::PlayerClosed
            | MemberState::Left => (glyph::STATE_OFF, Role::TextDim),
        };
        let role = if member.muted { Role::TextDim } else { role };
        let word = state_word(app, member);
        let word = if member.muted { t::room_state_and_muted(&word) } else { word };
        let name = if member.you {
            format!("{} {}", member.name, t::room_you_marker()) // i18n-skip: a name and its mark
        } else {
            member.name.clone()
        };
        buf.set_string(area.left() + 2, y, mark.to_string(), app.palette.style(role));
        let name_style = match (selected, member.muted || member.state == MemberState::Left) {
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
                truncate(&word, area.right().saturating_sub(state_x) as usize),
                app.palette.style(role),
            );
        }
        y += 1;
    }
    y
}

/// A person's one word: Watching, Loading · 4 s, On episode 8, Paused it.
fn state_word(app: &App, member: &RoomMember) -> String {
    let seconds = || member.since_ms.map_or(0, |since| seconds_since(app, since));
    let room_episode =
        app.room.as_ref().and_then(|r| r.room_episode.as_ref()).map(|e| e.episode.as_str());
    match member.state {
        MemberState::Watching => t::room_state_watching(),
        MemberState::Ready => t::room_state_ready(),
        MemberState::Paused if member.paused_room => t::room_state_paused_it(),
        MemberState::Paused => t::room_state_paused(),
        MemberState::Loading => t::room_state_loading(seconds()),
        MemberState::Buffering => t::room_state_buffering(seconds()),
        MemberState::Behind => {
            t::room_state_behind(member.behind_s.map_or(0, |s| s.round() as i64))
        }
        MemberState::OtherEpisode => t::room_state_other_episode(number(
            member.episode.as_ref().map_or("", |e| e.episode.as_str()),
        )),
        MemberState::Choosing => t::room_state_choosing(),
        MemberState::NoSource => t::room_state_no_source(number(room_episode.unwrap_or(""))),
        MemberState::PlayerClosed => t::room_state_player_closed(),
        MemberState::Reconnecting => t::room_state_reconnecting(),
        MemberState::Left => t::room_state_left(),
    }
}

/// The room's line: what it is doing, in a first line and a quieter second one.
fn room_lines(app: &App, room: &RoomView) -> (Option<String>, Option<String>) {
    let now = now_ms(app);
    if let RoomConnection::Reconnecting { .. } = room.connection {
        return (Some(t::room_line_reconnecting()), Some(t::room_line_reconnecting_detail()));
    }
    if let Some(moved) = &room.moved {
        let went: Vec<&str> = moved.went.iter().map(String::as_str).collect();
        let second = (!went.is_empty()).then(|| t::room_line_went_along(&names(&went)));
        return (Some(t::room_line_moved_away(&moved.by)), second);
    }
    if !room.waiting.is_empty() && now >= room.wait_shown_at_ms.unwrap_or(0) {
        let since = room.waiting.iter().map(|w| w.since_ms).min().unwrap_or(now);
        let seconds = seconds_since(app, since);
        let others: Vec<&str> =
            room.waiting.iter().filter(|w| !w.you).map(|w| w.name.as_str()).collect();
        if room.waiting.iter().any(|w| w.you) {
            return (
                Some(t::room_line_with_seconds(&t::room_line_paused_for_you(), seconds)),
                None,
            );
        }
        if room.can_carry_on {
            return (
                Some(t::room_line_still_loading(&names(&others), others.len() as i64)),
                Some(t::room_line_waited(seconds)),
            );
        }
        let first = t::room_line_paused_for(&waited_names(&others));
        return (Some(t::room_line_with_seconds(&first, seconds)), None);
    }
    if let Some(carried) = &room.carried_on
        && !carried.past.is_empty()
    {
        let carrier = if carried.by_you { t::room_you() } else { carried.by.clone() };
        let second = match carried.past.as_slice() {
            [one] => t::room_line_carried_on_one(&carrier, one),
            _ => t::room_line_carried_on_many(&carrier),
        };
        return (Some(t::room_line_watching()), Some(second));
    }
    if let Some(up_next) = &room.up_next
        && up_next.cancelled_by.is_none()
    {
        let episode = number(&up_next.episode.episode);
        return match (up_next.ends_at_ms, up_next.waiting_for.first()) {
            (Some(ends), _) => (
                Some(t::room_line_up_next(((ends - now) / 1000).max(0))),
                Some(t::room_line_up_next_detail(episode)),
            ),
            (None, Some(name)) => (Some(t::room_up_next_waiting(name)), None),
            (None, None) => (None, None),
        };
    }
    if room.before_play {
        return match room.starts_at_ms {
            Some(at) => (Some(t::room_line_starting_in(((at - now) / 1000).max(0))), None),
            None => (Some(t::room_line_not_started()), Some(t::room_line_play_starts())),
        };
    }
    let at = room.position.map(|position| {
        let moved =
            if room.playing { (now - room.position_at_ms) as f64 / 1000.0 } else { 0.0 };
        clock(position + moved.max(0.0))
    });
    if room.playing {
        let detail = room
            .room_episode
            .as_ref()
            .zip(at)
            .map(|(e, at)| t::room_line_watching_detail(number(&e.episode), &at));
        return (Some(t::room_line_watching()), detail);
    }
    let first = if room.paused_by_you {
        t::room_line_you_paused()
    } else if let Some(by) = &room.paused_by {
        t::room_line_paused_by(by)
    } else {
        return (None, None);
    };
    (Some(first), at.map(|at| t::room_line_paused_at(&at)))
}

/// How the room stands and whether this device's sources have its episode. Returns the next
/// row.
fn render_standing(
    buf: &mut Buffer,
    app: &App,
    room: &RoomView,
    area: Rect,
    mut y: u16,
) -> u16 {
    let width = area.width as usize;
    let mut put = |line: &str, role| {
        if y < area.bottom() {
            buf.set_string(area.left(), y, truncate(line, width), app.palette.style(role));
            y += 1;
        }
    };
    let (first, second) = room_lines(app, room);
    if let Some(first) = first {
        put(&first, Role::Text);
    }
    if let Some(second) = second {
        put(&second, Role::TextDim);
    }
    match &room.sources {
        Some(RoomSources::Found) => put(&t::room_plays_from_sources(), Role::TextDim),
        Some(RoomSources::Missing { reason, .. }) => put(&say(reason), Role::Alert),
        Some(RoomSources::Checking) | None => {}
    }
    y
}

/// A recent room's second line: the episode it reached, who was there and when.
fn recent_line(app: &App, room: &RoomRecent) -> String {
    let when = when(app, room.last_at);
    let episode = number(&room.episode);
    if room.names.is_empty() {
        return t::room_recent_row_alone(episode, &when);
    }
    let list: Vec<&str> = room.names.iter().map(String::as_str).collect();
    t::room_recent_row(episode, &names(&list), &when)
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

/// The room's connection: sealed end to end, how many of its relays answer, the round trip.
fn connection_line(room: &RoomView) -> String {
    let total = room.relays.len() as i64;
    let answering = room.relays.iter().filter(|r| r.healthy).count() as i64;
    let line = if total == 0 {
        t::room_details_end_to_end()
    } else {
        t::tui_room_relays_line(answering, total)
    };
    match room.latency_ms {
        // i18n-skip: joins the line and the round trip
        Some(ms) => format!("{line} · {}", t::room_relay_latency(i64::from(ms))),
        None => line,
    }
}

/// The pane beside the list at wide sizes. In a room: what it plays and how it is connected,
/// its line, the invite, then the conversation. Otherwise who was in the highlighted recent
/// room.
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
    if let Some(room) = &app.room {
        put(buf, &mut y, &room_title(room), text.add_modifier(Modifier::BOLD));
        if let Some(line) = header_line(app, room) {
            put(buf, &mut y, &line, dim);
        }
        put(buf, &mut y, &connection_line(room), dim);
        y += 1;
        let before = y;
        y = render_standing(buf, app, room, area, y);
        if y > before {
            y += 1;
        }
        // The invite: the link to copy, and what having it means.
        if let Some(link) = &room.link
            && y + 6 < area.bottom()
        {
            put(buf, &mut y, &t::room_invite(), dim);
            put(buf, &mut y, link, text);
            put(buf, &mut y, &t::room_invite_anyone(), dim);
            y += 1;
        }
        if y + 3 < area.bottom() {
            let room = Rect { y, height: area.bottom() - y, ..area };
            render_chat(buf, app, room);
        }
    } else if let Some(room) = app.selected_room() {
        put(buf, &mut y, &room.title, text.add_modifier(Modifier::BOLD));
        put(buf, &mut y, &recent_line(app, room), dim);
        y += 1;
        put(buf, &mut y, &t::room_recent_note(), dim);
    }
}

/// A reaction line as the conversation says it: "Ren and Mika laughed at 12:04".
fn reaction_line(line: &RoomChatLine, who: &str) -> Option<String> {
    let RoomChatBody::Reacted { reaction, at, with } = &line.body else { return None };
    let mut list = vec![who];
    list.extend(with.iter().map(String::as_str));
    let who = names(&list);
    let at = clock(*at);
    Some(match reaction {
        Reaction::Laugh => t::room_reacted_laugh(&who, &at),
        Reaction::Gasp => t::room_reacted_gasp(&who, &at),
        Reaction::Clap => t::room_reacted_clap(&who, &at),
        Reaction::Cry => t::room_reacted_cry(&who, &at),
    })
}

/// One row of the conversation: the name in its column (bold, "You" dim), then the words; or
/// a whole dim line.
enum ChatRow {
    Said { who: String, you: bool, words: String },
    Quiet(String),
}

/// The conversation: the newest lines that fit, each name in a column and its words wrapped
/// under the words, a reaction or the room's own word as a dim line, a muted member's line as
/// "Ren is muted", "Couldn't send" after a line no relay echoed, and the privacy line when there
/// is room. With Hide chat on, it says that instead. Returns the rows used.
fn render_chat(buf: &mut Buffer, app: &App, area: Rect) -> u16 {
    let dim = app.palette.style(Role::TextDim);
    let text = app.palette.style(Role::Text);
    let width = area.width as usize;
    let mut y = area.top();
    buf.set_string(area.left(), y, truncate(&t::room_chat(), width), dim);
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
    if app.chat_hidden() {
        quiet(buf, &mut y, &t::tui_room_chat_hidden(), text);
        quiet(buf, &mut y, &t::tui_together_chat_hidden_line(), dim);
        return y - area.top();
    }
    // Rows for the lines: what is left after the failure and the privacy line.
    let failed = app.room_chat.failed.is_some();
    let reserve = u16::from(failed) + if area.height >= 6 { 3 } else { 0 };
    let budget = area.bottom().saturating_sub(y).saturating_sub(reserve) as usize;

    let lines: Vec<(String, &RoomChatLine)> = app
        .room_chat
        .lines
        .iter()
        .map(|line| (if line.you { t::room_you() } else { line.from.clone() }, line))
        .collect();
    let column = lines
        .iter()
        .filter(|(_, line)| !line.muted && matches!(line.body, RoomChatBody::Said { .. }))
        .map(|(who, _)| who.cells())
        .max()
        .unwrap_or(0)
        .min(NAME_COLUMN);
    let words_width = width.saturating_sub(column + 2).max(8);
    let mut rows: Vec<ChatRow> = Vec::new();
    for (who, line) in &lines {
        if line.muted {
            rows.push(ChatRow::Quiet(t::tui_room_muted_line(&line.from)));
            continue;
        }
        match &line.body {
            RoomChatBody::Said { text } => {
                for (i, part) in wrap(text, words_width, 3).into_iter().enumerate() {
                    let who = if i == 0 { who.clone() } else { String::new() };
                    rows.push(ChatRow::Said { who, you: line.you, words: part });
                }
            }
            RoomChatBody::Reacted { .. } => {
                if let Some(said) = reaction_line(line, who) {
                    rows.push(ChatRow::Quiet(said));
                }
            }
            RoomChatBody::Room { message } => rows.push(ChatRow::Quiet(message.text.clone())),
        }
    }
    if rows.is_empty() && budget > 0 {
        buf.set_string(area.left(), y, truncate(&t::room_chat_field(), width), dim);
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
        buf.set_string(
            area.left(),
            y,
            truncate(&t::room_chat_not_sent(), width),
            app.palette.style(Role::Alert),
        );
        y += 1;
    }
    if area.height >= 6 && y + 1 < area.bottom() {
        y += 1;
        quiet(buf, &mut y, &t::tui_room_chat_privacy(), dim);
    }
    y - area.top()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_core::ids::AnilistId;
    use kaeri_model::{
        catalog::ContentList,
        event::Event,
        room::{PausedFor, RoomConversation, RoomEpisodeInfo, WaitReason},
    };

    use crate::{
        nav::Section,
        screens::{
            fixtures::{app_with, text_of},
            render_to_buffer,
        },
    };

    /// A room whose episode is known, as starting one from an episode leaves it.
    fn room() -> RoomView {
        RoomView {
            room_id: "room-1".into(),
            name: "Ada".into(),
            connection: RoomConnection::Connected,
            room_episode: Some(RoomEpisodeInfo {
                id: AnilistId::new(104_321),
                episode: "9".into(),
                title: "Natsukaze Relay".into(),
                name: None,
                banner_url: None,
                cover_url: None,
            }),
            link: Some("https://kaeri.app/party#v=3&k=BwcHBwcHBwcHBwcHBwcHBw".into()),
            before_play: true,
            latency_ms: Some(42),
            ..Default::default()
        }
    }

    fn member(name: &str, you: bool) -> RoomMember {
        RoomMember {
            id: name.into(),
            name: name.into(),
            you,
            state: MemberState::Ready,
            since_ms: None,
            behind_s: None,
            episode: None,
            paused_room: false,
            waited: false,
            background: false,
            muted: false,
        }
    }

    fn recent(room_id: &str) -> RoomRecent {
        RoomRecent {
            room_id: room_id.into(),
            id: AnilistId::new(104_321),
            title: "Lantern Harbour".into(),
            episode: "3".into(),
            names: vec!["Mika".into()],
            last_at: 0,
            cover_url: None,
        }
    }

    /// Together, before any room.
    fn outside() -> App {
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Together);
        app
    }

    /// In a room nobody else has joined yet.
    fn alone() -> App {
        let mut app = outside();
        app.apply(Event::RoomJoined(room()));
        app.apply(Event::RoomMembers(vec![member("Ada", true)]));
        app
    }

    /// In a full room with a conversation, and rooms from before.
    fn full() -> App {
        let mut app = alone();
        app.apply(Event::RoomMembers(vec![
            member("Ada", true),
            member("Mika", false),
            member("Ren", false),
            member("Sol", false),
        ]));
        let said = |id: u64, from: &str, text: &str| RoomChatLine {
            id,
            from: from.into(),
            you: from == "Ada",
            body: RoomChatBody::Said { text: text.into() },
            muted: false,
        };
        app.apply(Event::RoomChat(RoomConversation {
            lines: vec![said(1, "Mika", "ok starting"), said(2, "Ada", "back in 2")],
            marks: Vec::new(),
            failed: None,
        }));
        app.apply(Event::RoomRecent(vec![recent("room-2"), recent("room-3")]));
        app
    }

    /// Outside a room, with rooms from before.
    fn outside_with_rooms() -> App {
        let mut app = outside();
        app.apply(Event::RoomRecent(vec![recent("room-2")]));
        app
    }

    #[test]
    fn a_room_waiting_to_start_draws_in_an_eight_row_terminal() {
        let mut app = alone();
        let text = text_of(&render_to_buffer(&mut app, 80, 8));
        assert!(text.contains("Natsukaze Relay"), "{text}");
        assert!(text.contains("Episode 9"), "{text}");
    }

    #[test]
    fn names_read_as_a_sentence() {
        assert_eq!(names(&["Mika"]), "Mika");
        assert_eq!(names(&["Mika", "Ren"]), "Mika and Ren");
        assert_eq!(names(&["Mika", "Ren", "Sol"]), "Mika, Ren and Sol");
        assert_eq!(waited_names(&["Mika", "Ren", "Sol"]), "3 people");
        assert_eq!(number(" 9.5 "), "9.5");
        assert_eq!(clock(724.0), "12:04");
    }

    #[test]
    fn the_rooms_line_says_who_it_waits_for_once_the_wait_shows() {
        let mut app = alone();
        let now = app.clock.now_epoch() * 1000;
        let waiting = |shown: i64| RoomView {
            before_play: false,
            waiting: vec![PausedFor {
                id: "Ren".into(),
                name: "Ren".into(),
                you: false,
                reason: WaitReason::Loading,
                since_ms: now - 4_000,
            }],
            wait_shown_at_ms: Some(shown),
            ..room()
        };
        let view = waiting(now + 1_000);
        assert_eq!(room_lines(&app, &view).0, None, "not before a second has passed");
        app.apply(Event::RoomState(waiting(now - 3_000)));
        let view = app.room.clone().unwrap();
        assert_eq!(room_lines(&app, &view).0.as_deref(), Some("Paused for Ren · 4 s"));
    }

    /// A state to draw, by name.
    type State = (&'static str, fn() -> App);

    #[test]
    fn nothing_is_written_below_the_stage_at_any_size() {
        // Drawn into a buffer exactly the stage's size: a line on a row below it lands outside
        // the buffer and panics here, where in a frame it would cover the status line or, at
        // the terminal's last row, take the app down.
        let states: [State; 4] = [
            ("outside a room", outside),
            ("outside a room, with rooms", outside_with_rooms),
            ("alone in a room", alone),
            ("in a full room", full),
        ];
        for (name, state) in states {
            for say in [false, true] {
                for height in 1..=16u16 {
                    for width in [20, 40, 56, 72, 79, 80, 100, 120] {
                        let mut app = state();
                        if say {
                            app.nav.open_overlay(Overlay::Say);
                        }
                        let area = Rect::new(0, 0, width, height);
                        let mut buf = Buffer::empty(area);
                        let drawn =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                render_together(&mut buf, &app, area);
                            }));
                        assert!(
                            drawn.is_ok(),
                            "{name} at {width}x{height}{}",
                            if say { " with the Say line" } else { "" }
                        );
                    }
                }
            }
        }
    }
}
