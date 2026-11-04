//! Devices, under Settings: this device's name, the others it syncs with, the cloud folder and
//! the profiles, in the Settings screen's grammar of ruled headings and a note for the row under
//! the cursor. Its questions are drawn in the overlay band from a [`Body`] made here.

use kaeri_i18n::t;
use kaeri_model::{
    group::{SyncFolderKind, SyncSignInStep},
    qr::{qr_half_blocks, qr_matrix},
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use super::chrome::mark_selected;
use crate::{
    app::{
        App, DeviceConfirm, DeviceLine, DevicePageItem, Field, FolderState, code_groups,
        countdown_text, is_drive, key_groups, pair_link, service_line, service_name,
    },
    nav::{Focus, Overlay},
    theme::{Role, glyph},
    widgets::{Cells, Hairline, truncate, wrap},
};

/// How many days unseen before a device says so rather than when it last synced.
const UNSEEN_DAYS: i64 = 7;

/// One visual line of the Devices screen.
enum Line {
    Blank,
    /// A group's name, ruled to the right edge.
    Heading(String),
    /// Prose the cursor does not stop on.
    Prose(String, Role),
    /// A row the cursor stops on.
    Row {
        at: usize,
        label: String,
        value: String,
        alert: bool,
    },
}

/// "Synced 4m ago", or "Synced just now".
fn synced(app: &App, epoch: i64) -> String {
    let secs = app.clock.now_epoch() - epoch;
    let ago =
        if secs < 60 { t::time_just_now_inline() } else { t::time_ago(&countdown_text(secs)) };
    t::devices_row_synced(&ago)
}

/// A date as the recovery key's row gives it: 4 November.
fn day(app: &App, epoch: i64) -> String {
    app.clock.local(epoch).map(|d| d.format("%-d %B").to_string()).unwrap_or_default()
}

/// Where a server's or a folder's sync lives: its address or path. A drive's account is on the
/// Sign out row.
fn folder_place(app: &App, kind: &SyncFolderKind) -> Option<String> {
    let folder = app.devices.status.as_ref()?.folder.as_ref()?;
    (!is_drive(kind) && !folder.label.is_empty()).then(|| folder.label.clone())
}

/// Every line, and which of them the cursor stops on, in order.
fn lines(app: &App) -> Vec<Line> {
    let mut out = Vec::new();
    let status = app.devices.status.as_ref();
    let row =
        |at: usize, label: String, value: String| Line::Row { at, label, value, alert: false };
    for (at, device_line) in app.device_lines().into_iter().enumerate() {
        match device_line {
            DeviceLine::Name => {
                out.push(Line::Heading(t::devices_this_device()));
                out.push(row(at, t::devices_name(), app.device_name()));
                out.push(Line::Blank);
            }
            DeviceLine::NewKey => out.push(row(at, t::rkey_new(), String::new())),
            DeviceLine::Note(i) => {
                let Some(note) = status.and_then(|s| s.notes.get(i)) else { continue };
                if !out.iter().any(|l| matches!(l, Line::Heading(h) if *h == t::sync_notes())) {
                    out.push(Line::Blank);
                    out.push(Line::Heading(t::sync_notes()));
                }
                let title =
                    app.devices.note_titles.get(&note.anilist_id).cloned().unwrap_or_default();
                let episode = note.episode.parse::<i64>().unwrap_or(0);
                out.push(Line::Prose(
                    t::sync_note_kept_watched(
                        episode,
                        &title,
                        &note.unwatch_device,
                        &note.watch_device,
                    ),
                    Role::TextDim,
                ));
                out.push(row(at, t::sync_note_undo(), String::new()));
            }
            DeviceLine::TurnOn => {
                out.push(Line::Heading(t::sync_group()));
                out.push(Line::Prose(t::sync_off_title(), Role::Text));
                out.push(Line::Prose(t::sync_off_line(), Role::TextDim));
                out.push(Line::Blank);
                let value = if app.devices.working || app.devices.setting_up {
                    t::cloud_setting_up()
                } else {
                    t::devices_row_off()
                };
                out.push(row(at, t::sync_turn_on(), value));
            }
            DeviceLine::Device(i) => {
                if !matches!(out.last(), Some(Line::Row { .. })) {
                    out.push(Line::Heading(t::devices_yours()));
                }
                let Some(device) = status.and_then(|s| s.devices.get(i)) else { continue };
                let value = match device.last_seen {
                    None => t::sync_tui_device_never(),
                    Some(at) if (app.clock.now_epoch() - at) / 86_400 >= UNSEEN_DAYS => {
                        t::devices_not_seen((app.clock.now_epoch() - at) / 86_400)
                    }
                    Some(at) => synced(app, at),
                };
                out.push(row(at, device.name.clone(), value));
            }
            DeviceLine::ShowCode => {
                let alone = status.is_none_or(|s| s.devices.iter().all(|d| d.this_device));
                if alone {
                    out.push(Line::Heading(t::devices_yours()));
                    out.push(Line::Prose(t::devices_only_this(), Role::Text));
                    out.push(Line::Prose(t::devices_only_this_line(), Role::TextDim));
                    out.push(Line::Blank);
                }
                out.push(row(at, t::pair_show_code(), String::new()));
            }
            DeviceLine::EnterCode => out.push(row(at, t::sync_tui_enter_code(), String::new())),
            DeviceLine::AddCloud => {
                out.push(Line::Blank);
                out.push(Line::Heading(t::cloud_group()));
                out.push(row(at, t::cloud_add(), t::sync_tui_no_cloud()));
            }
            DeviceLine::Folder => {
                out.push(Line::Blank);
                out.push(Line::Heading(t::cloud_group()));
                let Some(kind) = app.folder_kind() else { continue };
                let state = app.folder_state();
                let value = match state {
                    FolderState::SettingUp => t::cloud_setting_up(),
                    FolderState::Syncing => t::sync_syncing(),
                    FolderState::SignedOut => t::cloud_sign_in_again(),
                    FolderState::Failed => t::devices_row_error(),
                    FolderState::Synced => match status.and_then(|s| s.last_synced) {
                        Some(at) => synced(app, at),
                        None => t::cloud_sync_now(),
                    },
                };
                let alert = matches!(state, FolderState::SignedOut | FolderState::Failed);
                out.push(Line::Row { at, label: service_name(&kind), value, alert });
                if let Some(place) = folder_place(app, &kind) {
                    out.push(Line::Prose(place, Role::TextDim));
                }
                if let Some(problem) = app.folder_problem() {
                    out.push(Line::Prose(problem, Role::Alert));
                }
            }
            DeviceLine::SignOut => {
                let account = status
                    .and_then(|s| {
                        s.clouds
                            .iter()
                            .find(|c| Some(&c.service) == app.signed_in_drive().as_ref())
                    })
                    .and_then(|c| c.account.clone())
                    .unwrap_or_default();
                out.push(row(at, t::cloud_sign_out(), account));
            }
            DeviceLine::RecoveryKey => {
                let made = status
                    .and_then(|s| s.recovery_key_made)
                    .map(|at| t::rkey_made(&day(app, at)))
                    .unwrap_or_default();
                out.push(row(at, t::rkey_row(), made));
            }
            DeviceLine::TurnOff => {
                out.push(Line::Blank);
                out.push(row(at, t::sync_turn_off(&t::sync_tui_kind()), String::new()));
            }
            DeviceLine::Profile(i) => {
                if i == 0 {
                    out.push(Line::Blank);
                    out.push(Line::Heading(t::profiles_title()));
                }
                let Some(profile) = app.devices.profiles.get(i) else { continue };
                let value = if profile.active {
                    t::sync_tui_profile_in_use()
                } else if profile.kids {
                    t::profiles_kids_tag()
                } else {
                    String::new()
                };
                out.push(row(at, profile.name.clone(), value));
            }
            DeviceLine::AddProfile => out.push(row(at, t::profiles_add(), String::new())),
        }
    }
    out
}

/// The note under the screen for the row under the cursor.
fn note(app: &App) -> Option<String> {
    Some(match app.device_line()? {
        DeviceLine::ShowCode | DeviceLine::EnterCode => t::pair_same_wifi(),
        DeviceLine::AddCloud | DeviceLine::Folder | DeviceLine::SignOut => t::cloud_foot(),
        DeviceLine::RecoveryKey => t::rkey_line(),
        DeviceLine::NewKey => t::rkey_new_foot(),
        DeviceLine::Note(_) => t::sync_notes_foot(),
        DeviceLine::Name => return None,
        DeviceLine::TurnOff => t::sync_turn_off_alert_line(&t::sync_tui_kind()),
        DeviceLine::AddProfile => t::profiles_add_line(),
        DeviceLine::Profile(_) => t::profiles_shared_foot(),
        DeviceLine::TurnOn => t::sync_road_later(),
        DeviceLine::Device(_) => return None,
    })
}

pub(super) fn render_devices(buf: &mut Buffer, app: &App, area: Rect) {
    if area.height == 0 || area.width < 24 {
        return;
    }
    let focused = app.nav.focus() == Focus::Stage;
    let value_x = area.left() + (area.width / 2).clamp(14, 30);
    let prose_width = area.width.saturating_sub(4) as usize;

    // Prose wraps, so a paragraph is as many lines as it needs.
    let mut drawn: Vec<Line> = Vec::new();
    for line in lines(app) {
        match line {
            Line::Prose(text, role) => {
                for part in wrap(&text, prose_width, 3) {
                    drawn.push(Line::Prose(part, role));
                }
            }
            other => drawn.push(other),
        }
    }

    let selected = app.cursors.devices.selected;
    let selected_line = drawn
        .iter()
        .position(|l| matches!(l, Line::Row { at, .. } if *at == selected))
        .unwrap_or(0);
    let note = note(app);
    let note_lines = note.as_deref().map_or(1, |n| wrap(n, prose_width, 2).len().max(1)) as u16;
    let reserved = if area.height >= 10 { 1 + note_lines.max(2) } else { 0 };
    let visible = (area.height - reserved) as usize;
    let offset = selected_line.saturating_sub(visible.saturating_sub(1));

    for (slot, line) in drawn.iter().skip(offset).take(visible).enumerate() {
        let y = area.top() + slot as u16;
        match line {
            Line::Blank => {}
            Line::Heading(name) => {
                let label = truncate(name, area.width as usize);
                buf.set_string(area.left(), y, &label, app.palette.style(Role::Text));
                let from = area.left() + label.cells() as u16 + 2;
                for x in from..area.right().saturating_sub(1) {
                    buf[(x, y)]
                        .set_char(glyph::RULE_H)
                        .set_style(app.palette.style(Role::Rule));
                }
            }
            Line::Prose(text, role) => {
                buf.set_string(
                    area.left() + 2,
                    y,
                    truncate(text, prose_width),
                    app.palette.style(*role),
                );
            }
            Line::Row { at, label, value, alert } => {
                let is_selected = *at == selected;
                if is_selected {
                    mark_selected(buf, app, area.left(), y, focused);
                }
                // A row with no value is an action, and its words may run on.
                let label_room = if value.is_empty() {
                    area.width.saturating_sub(3) as usize
                } else {
                    value_x.saturating_sub(area.left() + 3) as usize
                };
                let label_style = if is_selected {
                    app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
                } else {
                    app.palette.style(Role::Text)
                };
                buf.set_string(area.left() + 2, y, truncate(label, label_room), label_style);
                let value_style = if *alert {
                    app.palette.style(Role::Alert)
                } else {
                    app.palette.style(Role::TextDim)
                };
                let value_room = area.right().saturating_sub(value_x) as usize;
                buf.set_string(value_x, y, truncate(value, value_room), value_style);
            }
        }
    }

    if let Some(note) = note
        && reserved > 0
    {
        let note_y = area.bottom().saturating_sub(note_lines);
        Hairline::new(&app.palette)
            .render(Rect { y: note_y.saturating_sub(1), height: 1, ..area }, buf);
        for (i, text) in wrap(&note, prose_width, note_lines as usize).iter().enumerate() {
            buf.set_string(
                area.left() + 2,
                note_y + i as u16,
                truncate(text, prose_width),
                app.palette.style(Role::TextDim),
            );
        }
    }
}

/// How a row of a question is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tone {
    Dim,
    Text,
    /// What the question is about: the key, the code.
    Strong,
    Alert,
}

/// A question of the Devices screen's, for the overlay band: its heading, its rows with a key
/// at the right where one does something, and a QR code drawn to the right of them.
pub(super) struct Body {
    pub heading: String,
    pub rows: Vec<(String, String)>,
    pub tones: Vec<Tone>,
    /// The row of the choice under the cursor, in a list question.
    pub selected: Option<usize>,
    /// A QR code, in half blocks, dark on light.
    pub art: Vec<String>,
}

impl Body {
    fn new(heading: String) -> Self {
        Self { heading, rows: Vec::new(), tones: Vec::new(), selected: None, art: Vec::new() }
    }

    fn push(&mut self, key: &str, text: String, tone: Tone) {
        self.rows.push((key.to_owned(), text));
        self.tones.push(tone);
    }

    /// A paragraph, wrapped to `width`.
    fn prose(&mut self, text: &str, width: usize, tone: Tone) {
        for part in wrap(text, width.max(8), 6) {
            self.push("", part, tone);
        }
    }

    fn blank(&mut self) {
        self.push("", String::new(), Tone::Dim);
    }

    /// A choice the cursor stops on, and the line under it.
    fn choice(
        &mut self,
        at: usize,
        cursor: usize,
        label: String,
        line: Option<&str>,
        width: usize,
    ) {
        if at == cursor {
            self.selected = Some(self.rows.len());
        }
        self.push("", label, Tone::Text);
        if let Some(line) = line {
            for part in wrap(line, width.saturating_sub(2).max(8), 2) {
                self.push("", format!("  {part}"), Tone::Dim); // i18n-skip
            }
        }
    }

    /// The problem the question last met, if any.
    fn problem(&mut self, app: &App, width: usize) {
        if let Some(problem) = &app.devices.problem {
            self.blank();
            self.prose(problem, width, Tone::Alert);
        }
    }
}

/// A heading with what is typed in `field` and the caret after it.
fn typed(app: &App, title: &str, field: Field) -> String {
    format!("{title}   {}▏", super::overlays::field_text(app, field)) // i18n-skip
}

/// Whether the QR code fits beside rows in `room` rows and `width` cells.
fn fits(art: &[String], room: usize, width: usize, text: usize) -> bool {
    let wide = art.first().map_or(0, |l| l.chars().count());
    art.len() <= room && wide + text + 4 <= width
}

/// The body of the Devices question on top, laid out for a band `width` cells wide with `room`
/// rows under its heading.
pub(super) fn devices_body(app: &App, overlay: &Overlay, width: u16, room: usize) -> Body {
    let width = width.saturating_sub(4) as usize;
    let text = width.min(72);
    let cursor = app.cursors.overlay.selected;
    let d = &app.devices;
    match overlay {
        Overlay::SyncRoad => {
            let mut body = Body::new(overlay.title().to_string());
            body.choice(
                0,
                cursor,
                t::sync_road_nearby(),
                Some(&t::sync_road_nearby_line()),
                text,
            );
            body.blank();
            body.choice(
                1,
                cursor,
                t::sync_road_cloud(),
                Some(&t::sync_road_cloud_line()),
                text,
            );
            body.blank();
            body.prose(&t::sync_road_later(), text, Tone::Dim);
            body
        }
        Overlay::SyncNearby => {
            let mut body = Body::new(overlay.title().to_string());
            body.choice(0, cursor, t::pair_show_code(), None, text);
            body.choice(1, cursor, t::sync_tui_enter_code(), None, text);
            body.blank();
            body.prose(&t::pair_same_wifi(), text, Tone::Dim);
            body
        }
        Overlay::SyncClouds => {
            let mut body = Body::new(overlay.title().to_string());
            for (at, kind) in app.cloud_choices().iter().enumerate() {
                body.choice(at, cursor, service_name(kind), Some(&service_line(kind)), text);
            }
            body.blank();
            body.prose(&t::cloud_pick_foot(), text, Tone::Dim);
            body
        }
        Overlay::SyncWait => {
            let kind = app.waiting_for().unwrap_or(SyncFolderKind::GoogleDrive);
            let name = service_name(&kind);
            let host = d.pending.as_ref().and_then(|p| p.address.as_deref()).map(host_of);
            let heading = match (&kind, host) {
                (SyncFolderKind::Nextcloud, Some(host)) => t::cloud_nextcloud_wait_title(&host),
                _ => t::cloud_waiting(&name),
            };
            let mut body = Body::new(heading);
            let step = d.sign_in.as_ref().and_then(|(_, step)| step.as_ref());
            match step {
                Some(SyncSignInStep::Finishing) => {
                    body.prose(&t::cloud_setting_up(), text, Tone::Dim);
                    return body;
                }
                Some(SyncSignInStep::Code { user_code, .. }) => {
                    body.push("", user_code.clone(), Tone::Strong);
                    body.blank();
                }
                _ => {}
            }
            body.prose(&t::sync_tui_browser_line(&name), text, Tone::Dim);
            body.blank();
            if let Some(url) = app.wait_url() {
                body.push("↵", t::cloud_open_again(), Tone::Text);
                body.blank();
                body.push("", t::sync_tui_browser_address(), Tone::Dim);
                for chunk in chunks(&url, width.saturating_sub(2)) {
                    body.push("", chunk, Tone::Text);
                }
            } else {
                body.push("", t::sync_tui_signing_in(), Tone::Dim);
            }
            if kind == SyncFolderKind::Nextcloud {
                body.blank();
                body.push("a", t::cloud_app_password(), Tone::Text);
            }
            body
        }
        Overlay::SyncAddress => {
            let (title, note) = match d.kind {
                Some(SyncFolderKind::WebDav) => {
                    (t::sync_tui_webdav_address(), t::sync_tui_webdav_address_note())
                }
                Some(SyncFolderKind::Directory) => {
                    (t::sync_tui_folder_path(), t::sync_tui_folder_path_note())
                }
                _ => (t::cloud_nextcloud_address(), t::cloud_nextcloud_address_note()),
            };
            let mut body = Body::new(typed(app, &title, Field::SyncAddress));
            body.prose(&note, text, Tone::Dim);
            working(&mut body, app, text);
            body.problem(app, text);
            body
        }
        Overlay::SyncUser => {
            let mut body = Body::new(typed(app, &t::sync_tui_user(), Field::SyncUser));
            if d.kind == Some(SyncFolderKind::Nextcloud) {
                body.prose(&t::sync_tui_nextcloud_user_note(), text, Tone::Dim);
            } else {
                body.prose(app.text(Field::SyncAddress), text, Tone::Dim);
            }
            body.problem(app, text);
            body
        }
        Overlay::SyncPassword => {
            let nextcloud = d.kind == Some(SyncFolderKind::Nextcloud);
            let title =
                if nextcloud { t::sync_tui_app_password() } else { t::sync_tui_password() };
            let mut body = Body::new(typed(app, &title, Field::SyncPassword));
            if nextcloud {
                body.prose(&t::cloud_app_password_note(&t::sync_tui_kind()), text, Tone::Dim);
            } else {
                let at =
                    format!("{} · {}", app.text(Field::SyncUser), app.text(Field::SyncAddress)); // i18n-skip
                body.prose(&at, text, Tone::Dim);
            }
            working(&mut body, app, text);
            body.problem(app, text);
            body
        }
        Overlay::SyncRecoveryKey => {
            let mut body = Body::new(overlay.title().to_string());
            let key = d.recovery_key.as_deref().map(key_groups).unwrap_or_default();
            body.push("", key.clone(), Tone::Strong);
            body.blank();
            let qr = if d.show_qr { qr_half_blocks(&qr_matrix(&key)) } else { Vec::new() };
            let room_for_qr = fits(&qr, room, width, 40);
            let beside = if room_for_qr {
                width.saturating_sub(qr_width(&qr) + 4).min(text)
            } else {
                text
            };
            body.prose(&t::rkey_line(), beside, Tone::Dim);
            body.prose(&t::sync_tui_rkey_again(), beside, Tone::Dim);
            body.blank();
            let copy = if d.key_copied { t::action_copied() } else { t::rkey_copy() };
            body.push("c", copy, Tone::Text);
            body.push("v", t::rkey_qr(), Tone::Text);
            if d.show_qr && !room_for_qr {
                body.blank();
                body.prose(&t::sync_tui_qr_no_room(), beside, Tone::Dim);
            } else if room_for_qr {
                body.art = qr;
            }
            body
        }
        Overlay::SyncRestore => {
            let mut body = Body::new(typed(app, overlay.title(), Field::RecoveryKey));
            body.prose(&t::cloud_err_other_group(), text, Tone::Dim);
            body.blank();
            body.prose(&t::rkey_enter_note(), text, Tone::Dim);
            working(&mut body, app, text);
            body.problem(app, text);
            body
        }
        Overlay::SyncConfirm => {
            let (heading, line) = match &d.confirm {
                Some(DeviceConfirm::Remove { name, .. }) => {
                    (t::devices_remove_alert(name), t::devices_remove_alert_line(name))
                }
                Some(DeviceConfirm::NewKey) => (t::rkey_new(), t::rkey_new_foot()),
                _ => (
                    t::sync_turn_off_alert(&app.device_name()),
                    t::sync_turn_off_alert_line(&t::sync_tui_kind()),
                ),
            };
            let mut body = Body::new(heading);
            body.prose(&line, text, Tone::Dim);
            body
        }
        Overlay::SyncPairShow => {
            let mut body = Body::new(overlay.title().to_string());
            let Some(invite) = &d.invite else {
                body.prose(&t::sync_tui_pair_making(), text, Tone::Dim);
                body.problem(app, text);
                return body;
            };
            let labels = [t::sync_tui_code(), t::sync_tui_address()];
            let indent = labels.iter().map(|l| l.cells()).max().unwrap_or(0) + 3;
            let [code_label, address_label] = labels;
            let labelled = |label: &str, value: &str| format!("{label:<indent$}{value}"); // i18n-skip
            body.push("", labelled(&code_label, &code_groups(&invite.code)), Tone::Strong);
            if let Some(address) = &d.address {
                body.push(
                    "",
                    labelled(&address_label, &format!("{address}:{}", invite.port)), // i18n-skip
                    Tone::Text,
                ); // i18n-skip
            }
            body.blank();
            let qr = d
                .address
                .as_ref()
                .map(|address| {
                    qr_half_blocks(&qr_matrix(&pair_link(&invite.code, address, invite.port)))
                })
                .unwrap_or_default();
            // A spent code is no use to a camera.
            let room_for_qr = !app.pair_code_expired() && fits(&qr, room, width, 36);
            let beside = if room_for_qr {
                width.saturating_sub(qr_width(&qr) + 4).min(text)
            } else {
                text
            };
            if app.pair_code_expired() {
                body.prose(&t::pair_err_expired(), beside, Tone::Alert);
                body.blank();
                body.push("↵", t::pair_show_code(), Tone::Text);
            } else {
                let left = app.pair_code_left().unwrap_or(0);
                let left = format!("{}:{:02}", left / 60, left % 60); // i18n-skip
                body.prose(&t::pair_good_for(&left), beside, Tone::Dim);
                body.prose(&t::pair_same_wifi(), beside, Tone::Dim);
                if room_for_qr {
                    body.prose(&t::pair_show_scan(), beside, Tone::Dim);
                }
            }
            body.problem(app, beside);
            if room_for_qr {
                body.art = qr;
            }
            body
        }
        Overlay::SyncPairCode => {
            let mut body = Body::new(typed(app, overlay.title(), Field::PairCode));
            body.prose(&t::pair_enter_line(), text, Tone::Dim);
            body.blank();
            body.prose(&t::pair_same_wifi(), text, Tone::Dim);
            body.problem(app, text);
            body
        }
        Overlay::SyncPairAddress => {
            let mut body = Body::new(typed(app, overlay.title(), Field::PairAddress));
            body.prose(&t::sync_tui_pair_address_note(), text, Tone::Dim);
            if d.working {
                body.blank();
                body.prose(&t::sync_tui_pairing(), text, Tone::Dim);
            }
            body.problem(app, text);
            body
        }
        Overlay::SyncName => Body::new(typed(app, overlay.title(), Field::DeviceName)),
        Overlay::SyncDevice => {
            let Some(device) = app.page_device() else { return Body::new(app.device_name()) };
            let mut body = Body::new(device.name.clone());
            let state = |on: bool| if on { t::value_on() } else { t::value_off() };
            for (at, item) in app.device_page_items().into_iter().enumerate() {
                match item {
                    DevicePageItem::Sync => body.choice(
                        at,
                        cursor,
                        format!("{}   {}", t::devices_sync_switch(), state(!device.paused)), // i18n-skip
                        None,
                        text,
                    ),
                    DevicePageItem::Remote => body.choice(
                        at,
                        cursor,
                        format!(
                            "{}   {}",
                            t::devices_remote_switch(),
                            state(!device.remote_off)
                        ), // i18n-skip
                        None,
                        text,
                    ),
                    DevicePageItem::CopySignIns => body.choice(
                        at,
                        cursor,
                        format!(
                            "{}   {}", // i18n-skip
                            t::pair_copy_signins(&device.name),
                            state(!device.asked.is_empty())
                        ),
                        None,
                        text,
                    ),
                    DevicePageItem::Remove => {
                        body.choice(at, cursor, t::devices_remove(), None, text);
                    }
                }
            }
            if device.remote_key.is_some() {
                body.blank();
                body.prose(
                    &t::devices_switches_foot(&t::device_kind_computer(), &device.name),
                    text,
                    Tone::Dim,
                );
            }
            body
        }
        Overlay::ProfileName => {
            let mut body = Body::new(typed(app, overlay.title(), Field::ProfileName));
            body.prose(&t::profiles_add_line(), text, Tone::Dim);
            body
        }
        Overlay::ProfileKind => {
            let name = app.text(Field::ProfileName).trim().to_owned();
            let mut body = Body::new(format!("{}   {name}", overlay.title())); // i18n-skip
            body.choice(0, cursor, t::profiles_add_button(), None, text);
            body.choice(
                1,
                cursor,
                t::sync_tui_add_kids(),
                Some(&t::profiles_kids_line()),
                text,
            );
            body
        }
        _ => Body::new(overlay.title().to_string()),
    }
}

/// "Setting up the folder", while a form waits on the engine.
fn working(body: &mut Body, app: &App, width: usize) {
    if app.devices.working {
        body.blank();
        body.prose(&t::cloud_setting_up(), width, Tone::Dim);
    }
}

fn qr_width(art: &[String]) -> usize {
    art.first().map_or(0, |l| l.chars().count())
}

/// The host of an address, for the Nextcloud wait's heading.
fn host_of(address: &str) -> String {
    let rest = address.split_once("://").map_or(address, |(_, rest)| rest);
    rest.split('/').next().unwrap_or(rest).to_owned()
}

/// A long address in pieces of `width`, so all of it can be read off the screen.
fn chunks(text: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    chars.chunks(width.max(8)).map(|c| c.iter().collect()).collect()
}

/// Draw `art` dark on light at the right of the band, from `top`. A QR code is read by a camera,
/// so it keeps its own colours on every theme.
pub(super) fn draw_art(buf: &mut Buffer, art: &[String], band: Rect, top: u16) {
    use ratatui::style::{Color, Style};
    let wide = qr_width(art) as u16;
    let x = band.right().saturating_sub(wide + 2);
    let style = Style::default().fg(Color::Black).bg(Color::White);
    for (i, row) in art.iter().enumerate() {
        let y = top + i as u16;
        if y >= band.bottom() || x < band.left() {
            break;
        }
        buf.set_string(x, y, row, style);
    }
}

/// The style a row of a [`Body`] is drawn in.
pub(super) fn tone_style(app: &App, tone: Tone, selected: bool) -> ratatui::style::Style {
    let bold = |role| app.palette.style(role).add_modifier(Modifier::BOLD);
    match tone {
        _ if selected => bold(Role::Text),
        Tone::Strong => bold(Role::Text),
        Tone::Text => app.palette.style(Role::Text),
        Tone::Alert => app.palette.style(Role::Alert),
        Tone::Dim => app.palette.style(Role::TextDim),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nextcloud_address_names_its_host() {
        assert_eq!(host_of("https://cloud.example/nextcloud"), "cloud.example");
        assert_eq!(host_of("cloud.example"), "cloud.example");
    }

    #[test]
    fn a_long_address_is_cut_into_pieces_that_keep_every_character() {
        let url = "https://auth.example/authorize?client=kaeri&state=abcdef";
        assert_eq!(chunks(url, 20).concat(), url);
        assert!(chunks(url, 20).iter().all(|c| c.chars().count() <= 20));
    }
}
