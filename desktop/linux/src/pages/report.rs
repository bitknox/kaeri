//! Report a bug: what the report carries, the exact text, Copy report and Open on GitHub. Saved
//! crash reports are listed to include; while saving is off a line says so and links to the
//! setting. Player log, last in Included, saves the engine's log of the last playbacks through
//! the portal's file chooser, for attaching to the issue by hand: Save log… is the row's suffix,
//! the saved or failed line takes its subtitle, and the attach sentence sits under the list.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    bug_report::{
        BugReport, DeviceClass, PlayerLogStats, issue_url_with, player_log_ticked, report_text,
    },
    crash::{CrashRecord, when_text},
};

use crate::{motion, say, widgets, window::Ctx};

const NAME: &str = "report-bug";

/// How long Save log… and the attach sentence wait after the line under the row starts to
/// open, so no words show in a gap too small for them.
const WORDS_LATE: Duration = Duration::from_millis(60);

/// Open the sheet.
pub fn open(ctx: &Rc<Ctx>) {
    if ctx.window.visible_dialog().is_some_and(|d| d.widget_name() == NAME) {
        return;
    }
    let saving = kaeri_engine::crash::saving();
    let saved = kaeri_engine::crash::saved();
    let chosen: Rc<RefCell<Vec<String>>> =
        Rc::new(RefCell::new(saved.iter().map(|s| s.id.clone()).collect()));

    let header = adw::HeaderBar::new();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(6);
    column.set_margin_bottom(18);
    column.set_margin_start(18);
    column.set_margin_end(18);
    column.set_size_request(440, -1);
    let detail = widgets::haze(&t::report_detail(), "");
    detail.set_xalign(0.0);
    column.append(&detail);

    let group = adw::PreferencesGroup::builder().title(t::report_included()).build();
    let versions = adw::ActionRow::builder().title(t::report_versions()).build();
    versions.add_prefix(&gtk::Image::from_icon_name("object-select-symbolic"));
    group.add(&versions);
    let sent = gtk::Label::new(None);
    sent.set_selectable(true);
    sent.set_wrap(true);
    sent.set_xalign(0.0);
    sent.set_yalign(0.0);
    sent.add_css_class("monospace");

    let log = LogRow::new(kaeri_engine::player_log::stats());

    let build = {
        let ctx = Rc::clone(ctx);
        let chosen = Rc::clone(&chosen);
        let log = Rc::downgrade(&log);
        move || -> (BugReport, Vec<CrashRecord>) {
            let report = BugReport {
                app_version: kaeri_engine::updates::CURRENT.to_owned(),
                platform: "Linux".to_owned(), // i18n-skip
                os_version: String::new(),
                device: DeviceClass::Computer,
                build: if cfg!(feature = "fixtures") { "fixtures" } else { "release" }
                    .to_owned(), // i18n-skip
                providers: ctx.hub.known.borrow().providers.clone(),
                player_log: log.upgrade().is_some_and(|log| log.ticked()),
                ..Default::default()
            };
            let records = kaeri_engine::crash::armed_store()
                .map(|store| chosen.borrow().iter().filter_map(|id| store.get(id)).collect())
                .unwrap_or_default();
            (report, records)
        }
    };
    let build = Rc::new(build);
    let redraw: Rc<dyn Fn()> = {
        let (sent, build) = (sent.clone(), Rc::clone(&build));
        Rc::new(move || {
            let (report, records) = build();
            sent.set_label(&report_text(&report, &records));
        })
    };

    if saving {
        for summary in &saved {
            let row = adw::ActionRow::builder()
                .title(t::report_crash_item(&when_text(summary.at_ms), summary.place.word()))
                .build();
            let check = gtk::CheckButton::new();
            check.set_active(true);
            check.set_valign(gtk::Align::Center);
            row.add_prefix(&check);
            row.set_activatable_widget(Some(&check));
            let (id, chosen, redraw) =
                (summary.id.clone(), Rc::clone(&chosen), Rc::clone(&redraw));
            check.connect_toggled(move |check| {
                let mut ids = chosen.borrow_mut();
                ids.retain(|i| *i != id);
                if check.is_active() {
                    ids.push(id.clone());
                }
                drop(ids);
                redraw();
            });
            group.add(&row);
        }
    } else {
        let off = adw::ActionRow::builder().title(t::report_crash_off()).build();
        off.set_title_lines(0);
        let settings = gtk::Button::with_label(&t::report_crash_settings());
        settings.add_css_class("flat");
        settings.set_valign(gtk::Align::Center);
        settings.connect_clicked({
            let weak = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = weak.upgrade() {
                    if let Some(dialog) = ctx.window.visible_dialog() {
                        dialog.force_close();
                    }
                    let _ = WidgetExt::activate_action(
                        &ctx.window,
                        "win.settings-pane",
                        Some(&"about".to_variant()),
                    );
                }
            }
        });
        off.add_suffix(&settings);
        group.add(&off);
    }
    group.add(&log.row);
    // The list in a veil, for the crossfade to the saved line, and the attach sentence under it.
    log.veil.set_child(Some(&group));
    let included = gtk::Box::new(gtk::Orientation::Vertical, 0);
    included.append(&log.veil);
    included.append(&log.attach);
    column.append(&included);

    // What is sent: the report's text, and with Player log ticked, segments for the log's.
    let segments = adw::ToggleGroup::new();
    segments.add(adw::Toggle::builder().name("report").label(t::report_sent_report()).build());
    segments.add(adw::Toggle::builder().name("log").label(t::report_player_log()).build());
    segments.set_active_name(Some("report"));
    segments.set_halign(gtk::Align::Start);
    segments.set_margin_bottom(10);
    let segments_line = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .transition_duration(300)
        .child(&segments)
        .build();
    let log_text = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .build();
    log_text.add_css_class("report-log-text");
    log_text.update_property(&[gtk::accessible::Property::Label(&t::report_player_log())]);
    let log_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(200)
        .child(&log_text)
        .build();
    // The two pages share one height, so switching between them moves nothing below.
    let pages = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(300)
        .vhomogeneous(true)
        .build();
    pages.add_named(&sent, Some("report"));
    pages.add_named(&log_scroll, Some("log"));
    let sent_body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sent_body.set_margin_top(8);
    sent_body.append(&segments_line);
    sent_body.append(&pages);

    let reveal = gtk::Expander::new(Some(&t::report_show_sent()));
    reveal.set_child(Some(&sent_body));
    reveal.connect_expanded_notify(|reveal| {
        reveal.set_label(Some(&if reveal.is_expanded() {
            t::report_hide_sent()
        } else {
            t::report_show_sent()
        }));
    });
    column.append(&reveal);

    let buttons = adw::WrapBox::builder().child_spacing(8).line_spacing(8).build();
    let copy = gtk::Button::with_label(&t::report_copy());
    copy.add_css_class("pill");
    let open = gtk::Button::with_label(&t::report_open_github());
    open.add_css_class("pill");
    open.add_css_class("suggested-action");
    buttons.append(&copy);
    buttons.append(&open);
    column.append(&buttons);
    let privacy = widgets::haze(&t::report_privacy(), "caption");
    privacy.set_xalign(0.0);
    column.append(&privacy);

    // The log's text is read off the main thread each time its segment shows: it is redacted
    // whole, up to a megabyte, and a playback may have added to it since.
    let show_log = {
        let (build, log_text) = (Rc::clone(&build), log_text.clone());
        let reading = Rc::new(Cell::new(0_u64));
        move || {
            let (report, _) = build();
            let turn = reading.get() + 1;
            reading.set(turn);
            let (reading, log_text) = (Rc::clone(&reading), log_text.clone());
            glib::spawn_future_local(async move {
                let text =
                    gio::spawn_blocking(move || kaeri_engine::player_log::text(&report)).await;
                if reading.get() != turn {
                    return;
                }
                if let Ok(text) = text {
                    let buffer = log_text.buffer();
                    buffer.set_text(&text);
                    buffer.place_cursor(&buffer.start_iter());
                    log_text.scroll_to_iter(&mut buffer.start_iter(), 0.0, false, 0.0, 0.0);
                }
            });
        }
    };
    segments.connect_active_name_notify({
        let pages = pages.clone();
        move |segments| {
            if segments.active_name().as_deref() == Some("log") {
                show_log();
                pages.set_visible_child_name("log");
            } else {
                pages.set_visible_child_name("report");
            }
        }
    });

    log.check.connect_toggled({
        let (log, redraw) = (Rc::downgrade(&log), Rc::clone(&redraw));
        let (segments, segments_line) = (segments.clone(), segments_line.clone());
        move |check| {
            let on = check.is_active();
            if let Some(log) = log.upgrade() {
                log.show_line(on);
            }
            segments_line.set_reveal_child(on);
            if !on {
                segments.set_active_name(Some("report"));
            }
            redraw();
        }
    });
    log.save_button.connect_clicked({
        let (weak, log, build, redraw) =
            (Rc::downgrade(ctx), Rc::downgrade(&log), Rc::clone(&build), Rc::clone(&redraw));
        move |_| {
            if let (Some(ctx), Some(log)) = (weak.upgrade(), log.upgrade()) {
                log.save(&ctx, build().0, Rc::clone(&redraw), None);
            }
        }
    });
    log.show_button.connect_clicked({
        let (weak, log) = (Rc::downgrade(ctx), Rc::downgrade(&log));
        move |_| {
            if let (Some(ctx), Some(log)) = (weak.upgrade(), log.upgrade()) {
                log.show_in_files(&ctx);
            }
        }
    });

    // After a crash in the player the log starts ticked, with the keys on its row.
    let starts_ticked = log.enabled() && {
        let (report, records) = build();
        player_log_ticked(&report, &records)
    };
    if starts_ticked {
        log.check.set_active(true);
        let row = log.row.clone();
        motion::once_mapped(&log.row, move || {
            row.grab_focus();
        });
    }
    redraw();

    copy.connect_clicked({
        let (weak, build) = (Rc::downgrade(ctx), Rc::clone(&build));
        move |button| {
            let (report, records) = build();
            if let Some(display) = gdk::Display::default() {
                display.clipboard().set_text(&report_text(&report, &records));
            }
            if let Some(ctx) = weak.upgrade() {
                ctx.toast(&t::report_copied());
            }
            let _ = button;
        }
    });
    open.connect_clicked({
        let (weak, build, log, redraw) =
            (Rc::downgrade(ctx), Rc::clone(&build), Rc::downgrade(&log), Rc::clone(&redraw));
        move |_| {
            let Some(ctx) = weak.upgrade() else { return };
            let go = {
                let (weak, build) = (Rc::downgrade(&ctx), Rc::clone(&build));
                move || {
                    let (report, records) = build();
                    let url = issue_url_with(&report, &records);
                    if let Some(ctx) = weak.upgrade() {
                        crate::portal::open_uri(&ctx.window, &url, || {});
                    }
                }
            };
            // A ticked log not saved yet is saved first; Cancel in the chooser opens nothing.
            match log.upgrade() {
                Some(log) if log.ticked() && !log.is_saved() => {
                    log.save(&ctx, build().0, Rc::clone(&redraw), Some(Box::new(go)));
                }
                _ => go(),
            }
        }
    });

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(640)
        .child(&column)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .name(NAME)
        .title(t::report_title())
        .follows_content_size(true)
        .child(&view)
        .build();
    #[cfg(feature = "fixtures")]
    STAGE.with(|stage| {
        let (log, reveal, segments) =
            (Rc::downgrade(&log), reveal.downgrade(), segments.downgrade());
        stage.replace(Some(Box::new(move |state: &str| {
            let (Some(log), Some(reveal), Some(segments)) =
                (log.upgrade(), reveal.upgrade(), segments.upgrade())
            else {
                return;
            };
            log.check.set_active(true);
            match state {
                "sent" => {
                    reveal.set_expanded(true);
                    segments.set_active_name(Some("log"));
                }
                "saved" => log.settle(Saved::As(log.stats.file_name.clone())),
                "failed" => log.settle(Saved::Failed),
                _ => {}
            }
        })));
    });
    widgets::keep(&dialog, log);
    dialog.present(Some(&ctx.window));
}

#[cfg(feature = "fixtures")]
thread_local! {
    /// The open sheet's Player log, for the shots to put in a state.
    static STAGE: RefCell<Option<Box<dyn Fn(&str)>>> = RefCell::new(None);
}

/// For the shots: tick the open sheet's Player log and put it in `state`: `ticked`, `sent`
/// (What is sent open on the log's segment), `saved` or `failed`. `empty` leaves it as it is.
#[cfg(feature = "fixtures")]
pub fn stage(state: &str) {
    if state == "empty" {
        return;
    }
    STAGE.with(|stage| {
        if let Some(stage) = &*stage.borrow() {
            stage(state);
        }
    });
}

/// Where the log's file stands.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Saved {
    Not,
    /// Written, under this name.
    As(String),
    /// The last try to write it failed.
    Failed,
}

/// The Player log row, its suffix and the attach sentence under the list.
struct LogRow {
    stats: PlayerLogStats,
    row: adw::ActionRow,
    check: gtk::CheckButton,
    /// Save log…, then Show in Files once saved, crossfaded in place.
    actions: gtk::Stack,
    save_button: gtk::Button,
    show_button: gtk::Button,
    actions_fade: motion::Fader,
    attach: gtk::Revealer,
    attach_words: gtk::Label,
    attach_fade: motion::Fader,
    /// Around the boxed list: the saved line crossfades in under a picture of the list as it
    /// stood.
    veil: gtk::Overlay,
    state: RefCell<Saved>,
    file: RefCell<Option<gio::File>>,
    saving: Cell<bool>,
}

impl LogRow {
    fn new(stats: PlayerLogStats) -> Rc<Self> {
        let row =
            adw::ActionRow::builder().title(t::report_player_log()).use_markup(false).build();
        let check = gtk::CheckButton::new();
        check.set_valign(gtk::Align::Center);
        check.update_property(&[
            gtk::accessible::Property::Label(&t::report_player_log()),
            gtk::accessible::Property::Description(&t::report_player_log_a11y_hint()),
        ]);
        row.add_prefix(&check);
        row.set_activatable_widget(Some(&check));

        let save_button = gtk::Button::with_label(&t::report_player_log_save());
        save_button.set_valign(gtk::Align::Center);
        let show_button = gtk::Button::with_label(&t::settings_show_in_files());
        show_button.add_css_class("flat");
        show_button.set_valign(gtk::Align::Center);
        let actions = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(300)
            .hhomogeneous(false)
            .interpolate_size(true)
            .valign(gtk::Align::Center)
            .build();
        actions.add_named(&save_button, Some("save"));
        actions.add_named(&show_button, Some("show"));
        actions.set_visible(false);
        row.add_suffix(&actions);

        let attach_words = widgets::haze(&t::report_player_log_attach(), "");
        attach_words.set_margin_top(8);
        attach_words.set_margin_start(2);
        attach_words.set_visible(false);
        let attach = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideDown)
            .transition_duration(300)
            .child(&attach_words)
            .build();

        let log = Rc::new(Self {
            stats,
            row,
            check,
            actions,
            save_button,
            show_button,
            actions_fade: motion::Fader::default(),
            attach,
            attach_words,
            attach_fade: motion::Fader::default(),
            veil: gtk::Overlay::new(),
            state: RefCell::new(Saved::Not),
            file: RefCell::new(None),
            saving: Cell::new(false),
        });
        log.row.set_sensitive(log.enabled());
        log.refresh();
        log
    }

    /// Whether anything has been logged: before the first playback the row is disabled.
    fn enabled(&self) -> bool {
        self.stats.playbacks > 0
    }

    /// Whether the log goes with the report: ticked, on a row that is not disabled.
    fn ticked(&self) -> bool {
        self.enabled() && self.check.is_active()
    }

    fn is_saved(&self) -> bool {
        matches!(*self.state.borrow(), Saved::As(_))
    }

    /// The row's subtitle and suffix for where the file stands.
    fn refresh(&self) {
        let state = self.state.borrow().clone();
        self.row.set_subtitle(&subtitle(&self.stats, &state));
        set_class(&self.row, "report-log-saved", matches!(state, Saved::As(_)));
        set_class(&self.row, "report-log-failed", state == Saved::Failed);
        self.actions.set_visible_child_name(if matches!(state, Saved::As(_)) {
            "show"
        } else {
            "save"
        });
    }

    /// Open or close the line the ticked row brings: the attach sentence slides open under the
    /// list on the standard token, and Save log… and the sentence's words fade in a moment
    /// later. Closing fades them out as the line closes.
    fn show_line(self: &Rc<Self>, on: bool) {
        self.attach.set_reveal_child(on);
        if !on {
            self.actions_fade.set(&self.actions, false);
            self.attach_fade.set(&self.attach_words, false);
            return;
        }
        let log = Rc::downgrade(self);
        glib::timeout_add_local_once(WORDS_LATE, move || {
            let Some(log) = log.upgrade() else { return };
            if log.check.is_active() {
                log.actions_fade.set(&log.actions, true);
                log.attach_fade.set(&log.attach_words, true);
            }
        });
    }

    /// Ask where to save the log in the portal's file chooser, with the engine's name filled
    /// in, then write it. `then` runs once the file is written: Open on GitHub, when it asked.
    /// Cancel leaves everything as it was, and `then` never runs.
    fn save(
        self: &Rc<Self>,
        ctx: &Rc<Ctx>,
        report: BugReport,
        redraw: Rc<dyn Fn()>,
        then: Option<Box<dyn FnOnce()>>,
    ) {
        if self.saving.replace(true) {
            return;
        }
        let chooser = gtk::FileDialog::builder()
            .initial_name(self.stats.file_name.as_str())
            .modal(true)
            .build();
        let log = Rc::clone(self);
        chooser.save(Some(&ctx.window), gio::Cancellable::NONE, move |chosen| {
            let file = match chosen {
                Ok(file) => file,
                Err(e) => {
                    tracing::info!(error = %e, "no file chosen for the player log");
                    log.saving.set(false);
                    return;
                }
            };
            glib::spawn_future_local(async move {
                let written = write_log(&file, report).await;
                log.saving.set(false);
                match written {
                    Ok(()) => {
                        let name = file
                            .basename()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| log.stats.file_name.clone());
                        log.file.replace(Some(file));
                        log.settle(Saved::As(name));
                        log.row.upcast_ref::<gtk::Widget>().announce(
                            &t::report_player_log_a11y_saved(),
                            gtk::AccessibleAnnouncementPriority::Medium,
                        );
                        redraw();
                        if let Some(then) = then {
                            then();
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "the player log was not written");
                        log.settle(Saved::Failed);
                        log.row.upcast_ref::<gtk::Widget>().announce(
                            &t::report_player_log_failed(),
                            gtk::AccessibleAnnouncementPriority::Medium,
                        );
                    }
                }
            });
        });
    }

    /// Change where the file stands. The saved line crossfades in over the list as it stood,
    /// on the reveal token, which reduced motion keeps.
    fn settle(&self, state: Saved) {
        let saved = matches!(state, Saved::As(_));
        let change = || {
            self.state.replace(state);
            self.refresh();
        };
        match self.veil.child() {
            Some(list) if saved => motion::ghost(&self.veil, &list, true, change),
            _ => change(),
        }
    }

    /// Show the saved file in the file manager, selected.
    fn show_in_files(&self, ctx: &Ctx) {
        let Some(file) = self.file.borrow().clone() else { return };
        gtk::FileLauncher::new(Some(&file)).open_containing_folder(
            Some(&ctx.window),
            gio::Cancellable::NONE,
            |done| {
                if let Err(e) = done {
                    tracing::warn!(error = %e, "the player log's folder did not open");
                }
            },
        );
    }
}

/// The Player log row's subtitle: how much the log holds, the saved line, the failed line, or
/// that nothing has played yet.
fn subtitle(stats: &PlayerLogStats, state: &Saved) -> String {
    if stats.playbacks == 0 {
        return t::report_player_log_empty();
    }
    match state {
        Saved::As(name) => t::report_player_log_saved(name),
        Saved::Failed => t::report_player_log_failed(),
        Saved::Not => {
            t::report_player_log_detail(i64::from(stats.playbacks), &say::bytes(stats.bytes))
        }
    }
}

/// Write the log for `report` to `file`: redacted off the main thread, then written through
/// GIO, which also reaches a file the portal hands over without a local path.
async fn write_log(file: &gio::File, report: BugReport) -> Result<(), glib::Error> {
    let text = gio::spawn_blocking(move || kaeri_engine::player_log::text(&report))
        .await
        .map_err(|_| glib::Error::new(gio::IOErrorEnum::Failed, "the log was not read"))?; // i18n-skip
    file.replace_contents_future(
        text.into_bytes(),
        None,
        false,
        gio::FileCreateFlags::REPLACE_DESTINATION,
    )
    .await
    .map(|_| ())
    .map_err(|(_, e)| e)
}

fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(playbacks: u32) -> PlayerLogStats {
        PlayerLogStats {
            playbacks,
            bytes: 412_000,
            file_name: "kaeri-player-log-2026-11-15-1432.txt".into(), // i18n-skip
        }
    }

    #[test]
    fn the_row_says_how_much_the_log_holds_until_it_is_saved() {
        assert_eq!(
            subtitle(&stats(3), &Saved::Not),
            t::report_player_log_detail(3, &say::bytes(412_000))
        );
        let name = "kaeri-player-log-2026-11-15-1432.txt"; // i18n-skip
        assert_eq!(
            subtitle(&stats(3), &Saved::As(name.into())),
            t::report_player_log_saved(name)
        );
        assert_eq!(subtitle(&stats(3), &Saved::Failed), t::report_player_log_failed());
    }

    #[test]
    fn before_anything_has_played_the_row_says_so_whatever_else_stands() {
        for state in [Saved::Not, Saved::Failed, Saved::As("x.txt".into())] {
            assert_eq!(subtitle(&stats(0), &state), t::report_player_log_empty());
        }
    }
}
