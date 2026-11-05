//! The dialogs of sync: choosing a cloud folder and waiting for the browser, the recovery key,
//! restoring with it, and pairing both ways. Each is an AdwDialog on the window; the choosing
//! ones never close on a click outside. The engine's events move them on: a cloud sign-in's
//! steps turn the picker into a wait in place and then into Connecting, Setting up the folder,
//! and the recovery key or an error, each a crossfade in the same dialog.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    group::{
        SyncCommand, SyncCopy, SyncDeviceView, SyncFolderKind, SyncFolderSpec, SyncPairProblem,
        SyncProblem, SyncSignInStep, SyncStoreProblem,
    },
    stores::Secret,
};

use super::{offered, service_name};
use crate::window::Ctx;

/// Where a pairing dialog was opened from: Devices, or first launch on a new device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Devices,
    FirstLaunch,
}

/// A dialog's title in its header bar. A new title crossfades in over the old one, as the
/// content below it does, in place of swapping at once.
#[derive(Clone)]
struct Title(gtk::Stack);

impl Title {
    fn new(text: &str) -> Self {
        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(200);
        stack.set_hhomogeneous(false);
        let this = Self(stack);
        this.set_title(text);
        this
    }

    fn set_title(&self, text: &str) {
        if self.0.child_by_name(text).is_none() {
            self.0.add_named(&adw::WindowTitle::new(text, ""), Some(text));
        }
        self.0.set_visible_child_name(text);
    }
}

/// Where the sign-in stands in the picker's wait page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The browser is open on the service's page.
    Browser,
    /// The browser came back with access allowed.
    Connecting,
    /// Signed in; the folder is being made.
    SettingUp,
    /// The sign-in or the set-up failed.
    Error,
    /// The folder is ready and the recovery key shows.
    Key,
}

/// The timing of the return: a state shows only past 600 ms and then stays at least 400 ms.
#[derive(Default)]
struct Pace {
    /// When the browser came back.
    returned: Option<Instant>,
    /// When Connecting or Setting up was put on screen.
    shown: Option<Instant>,
    /// The sign-in got as far as making the folder.
    finishing: bool,
    timer: Option<glib::SourceId>,
}

#[derive(Clone)]
struct Picker {
    dialog: adw::Dialog,
    nav: adw::NavigationView,
    wait_page: adw::NavigationPage,
    error: gtk::Label,
    head: Title,
    body: gtk::Stack,
    spinner: adw::Spinner,
    wait_title: gtk::Label,
    wait_line: gtk::Label,
    open_again: gtk::Button,
    fail_line: gtk::Label,
    key_slot: gtk::Box,
    cancel: gtk::Button,
    primary: gtk::Button,
    stage: Rc<Cell<Stage>>,
    pace: Rc<RefCell<Pace>>,
    nc_error: gtk::Label,
    dav_error: gtk::Label,
    nc_host: Rc<RefCell<String>>,
}

#[derive(Clone)]
struct Pair {
    dialog: adw::Dialog,
    stack: gtk::Stack,
    code: gtk::Label,
    left: gtk::Label,
    qr: gtk::DrawingArea,
    address: gtk::Label,
    paired_title: gtk::Label,
    copy_slot: gtk::Box,
    /// First launch: the plugin stores the paired device made invites for, as switches.
    join_slot: gtk::Box,
    joins: Rc<RefCell<Vec<(String, adw::SwitchRow)>>>,
    /// The device that paired, once it has: its id and its name.
    other: Rc<RefCell<Option<(String, String)>>>,
    origin: Origin,
    started_with: usize,
}

#[derive(Clone)]
struct Enter {
    dialog: adw::Dialog,
    stack: gtk::Stack,
    error: gtk::Label,
    pair: gtk::Button,
    paired_title: gtk::Label,
    copy_slot: gtk::Box,
    working: Rc<std::cell::Cell<bool>>,
}

#[derive(Clone)]
struct Restore {
    dialog: adw::Dialog,
    error: gtk::Label,
}

#[derive(Default)]
struct Ui {
    picker: Option<Picker>,
    pair: Option<Pair>,
    enter: Option<Enter>,
    restore: Option<Restore>,
    key: Option<adw::Dialog>,
    by_hand: bool,
    /// The picker was opened by Use another service: a choice moves the group's folder.
    changing: bool,
    name: Option<String>,
    last: Option<SyncFolderKind>,
    /// The sheet already said why pairing failed, in words of its own: the problem that follows
    /// says the same and stays quiet.
    pair_said: bool,
}

thread_local! {
    static UI: RefCell<Ui> = RefCell::new(Ui::default());
}

fn with<R>(f: impl FnOnce(&mut Ui) -> R) -> R {
    UI.with_borrow_mut(f)
}

pub fn by_hand() -> bool {
    with(|u| u.by_hand)
}

pub fn signed_out_by_hand() {
    with(|u| u.by_hand = true);
}

/// A name typed for this device, else the machine's own.
pub fn typed_device_name() -> String {
    with(|u| u.name.clone()).unwrap_or_else(|| glib::host_name().to_string())
}

pub fn set_device_name(name: &str) {
    with(|u| u.name = Some(name.trim().to_owned()).filter(|n| !n.is_empty()));
}

// ---------------------------------------------------------------- building blocks

/// A dialog with a header bar (`start`, a title, `end`) over `body`.
fn frame(
    title: &str,
    width: i32,
    body: &impl IsA<gtk::Widget>,
) -> (adw::Dialog, adw::HeaderBar, Title) {
    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(false);
    let name = Title::new(title);
    header.set_title_widget(Some(&name.0));
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(body));
    let dialog = adw::Dialog::builder()
        .content_width(width)
        .follows_content_size(true)
        .child(&view)
        .build();
    dialog.set_title(title);
    (dialog, header, name)
}

fn column() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
    b.set_margin_top(6);
    b.set_margin_bottom(24);
    b.set_margin_start(24);
    b.set_margin_end(24);
    b
}

fn note(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class("haze");
    l.add_css_class("caption");
    l.set_wrap(true);
    l.set_max_width_chars(64);
    l.set_xalign(0.0);
    l
}

fn line(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class("haze");
    l.set_wrap(true);
    l.set_max_width_chars(52);
    l.set_xalign(0.0);
    l
}

fn error_label() -> gtk::Label {
    let l = gtk::Label::new(None);
    l.add_css_class("error");
    l.set_wrap(true);
    l.set_xalign(0.0);
    l.set_visible(false);
    l
}

fn say_error(label: &gtk::Label, text: Option<&str>) {
    match text {
        Some(t) => {
            label.set_label(t);
            label.set_visible(true);
        }
        None => label.set_visible(false),
    }
}

fn button(label: &str, class: Option<&str>) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    if let Some(c) = class {
        b.add_css_class(c);
    }
    b
}

fn choice_row(title: &str, subtitle: &str, icon: &str) -> adw::ActionRow {
    let row =
        adw::ActionRow::builder().title(title).subtitle(subtitle).activatable(true).build();
    row.add_prefix(&gtk::Image::from_icon_name(icon));
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic")); // i18n-skip
    row
}

fn boxed() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list
}

fn crossfade_stack() -> gtk::Stack {
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(200);
    stack.set_vhomogeneous(false);
    stack.set_hhomogeneous(false);
    stack.set_interpolate_size(true);
    stack
}

fn open_url(ctx: &Rc<Ctx>, url: &str) {
    gtk::UriLauncher::new(url).launch(Some(&ctx.window), gio::Cancellable::NONE, |_| {});
}

fn present(ctx: &Rc<Ctx>, dialog: &adw::Dialog) {
    dialog.present(Some(&ctx.window));
}

/// A QR code drawn as modules on a plate of its own colour.
pub fn qr_area(text: &str, px: i32) -> gtk::DrawingArea {
    let matrix = kaeri_model::qr::qr_matrix(text);
    let area = gtk::DrawingArea::new();
    area.set_content_width(px);
    area.set_content_height(px);
    area.set_halign(gtk::Align::Center);
    area.update_property(&[gtk::accessible::Property::Label(&t::rkey_qr_short())]);
    area.set_draw_func(move |_, cr, w, h| {
        cr.set_source_rgb(0.894, 0.878, 0.839);
        let _ = cr.paint();
        if matrix.size == 0 {
            return;
        }
        let total = f64::from(matrix.size + 2 * matrix.quiet_zone);
        let cell = f64::from(w.min(h)) / total;
        cr.set_source_rgb(0.086, 0.102, 0.18);
        for y in 0..matrix.size {
            for x in 0..matrix.size {
                if matrix.dark(x, y) {
                    cr.rectangle(
                        (f64::from(x) + f64::from(matrix.quiet_zone)) * cell,
                        (f64::from(y) + f64::from(matrix.quiet_zone)) * cell,
                        cell + 0.5,
                        cell + 0.5,
                    );
                }
            }
        }
        let _ = cr.fill();
    });
    area
}

// ---------------------------------------------------------------- picker and wait

/// Choose a cloud folder.
pub fn picker(ctx: &Rc<Ctx>) {
    with(|u| u.changing = false);
    present_picker(ctx, "list", None);
}

/// Use another service: the picker, with the folder in use marked, whose choice moves the
/// group to the new folder.
pub fn picker_change(ctx: &Rc<Ctx>, in_use: &SyncFolderKind) {
    with(|u| u.changing = true);
    present_picker(ctx, "list", Some(in_use.clone()));
}

/// For a shot: the picker on one of its pages.
#[cfg(feature = "fixtures")]
pub fn picker_page(ctx: &Rc<Ctx>, page: &str) {
    present_picker(ctx, page, None);
}

/// The wait for the browser, on its own: Sign in again from the cloud row.
pub fn sign_in_again(ctx: &Rc<Ctx>, service: &SyncFolderKind) {
    with(|u| {
        u.by_hand = false;
        u.changing = false;
        u.last = Some(service.clone());
    });
    ctx.hub.known.borrow_mut().sync_problem = None;
    ctx.hub.send(Command::SyncSignIn { service: service.clone() });
    let p = present_picker(ctx, "wait", None);
    stage_to(&p, service, Stage::Browser, None, None);
}

/// For a shot: the wait page in one of its states for `service`. The browser's is opened at
/// `url`; "denied", "expired" and "full" are the errors; "key" the recovery key.
#[cfg(feature = "fixtures")]
pub fn picker_state(ctx: &Rc<Ctx>, service: &SyncFolderKind, state: &str, url: &str) {
    let p = present_picker(ctx, "wait", None);
    let name = service_name(service);
    let (stage, error) = match state {
        "connecting" => (Stage::Connecting, None),
        "setting" => (Stage::SettingUp, None),
        "denied" => (
            Stage::Error,
            Some((t::cloud_err_signin_title(&name), t::cloud_err_denied_short(&name))),
        ),
        "expired" => {
            (Stage::Error, Some((t::cloud_err_signin_title(&name), t::cloud_err_timeout())))
        }
        "full" => (Stage::Error, Some((t::cloud_err_setup_title(), t::cloud_err_full(&name)))),
        "key" => (Stage::Key, None),
        _ => (Stage::Browser, None),
    };
    if stage == Stage::Key {
        fill_key(ctx, &p.key_slot, "K7QM4XR9TB2HWN6CP3ZD8FLA");
    }
    show_stage(&p, service, stage, (stage == Stage::Browser).then_some(url), error.as_ref());
}

/// The state the wait page is in, and the pace it came at: Connecting and Setting up show
/// only past 600 ms of the return, and a state shown stays at least 400 ms.
fn stage_to(
    p: &Picker,
    service: &SyncFolderKind,
    stage: Stage,
    url: Option<&str>,
    error: Option<(String, String)>,
) {
    let returning = matches!(stage, Stage::Connecting | Stage::SettingUp);
    let delay = {
        let mut pace = p.pace.borrow_mut();
        if let Some(id) = pace.timer.take() {
            id.remove();
        }
        if stage == Stage::Browser {
            *pace = Pace::default();
        } else {
            if returning {
                // The browser has come back: Cancel and closing are dead from here on, shown
                // state or not.
                p.cancel.set_sensitive(false);
                p.dialog.set_can_close(false);
            }
            pace.returned.get_or_insert_with(Instant::now);
        }
        let mut delay = Duration::ZERO;
        if returning && pace.shown.is_none() {
            let since = pace.returned.map(|r| r.elapsed()).unwrap_or_default();
            delay = Duration::from_millis(600).saturating_sub(since);
        }
        if let Some(shown) = pace.shown {
            delay = delay.max(Duration::from_millis(400).saturating_sub(shown.elapsed()));
        }
        delay
    };
    let (p2, service, url) = (p.clone(), service.clone(), url.map(str::to_owned));
    let apply = move || {
        p2.pace.borrow_mut().shown = returning.then(Instant::now);
        show_stage(&p2, &service, stage, url.as_deref(), error.as_ref());
    };
    if delay.is_zero() {
        apply();
    } else {
        let p3 = p.clone();
        let id = glib::timeout_add_local_once(delay, move || {
            p3.pace.borrow_mut().timer = None;
            apply();
        });
        p.pace.borrow_mut().timer = Some(id);
    }
}

/// Close the picker once the state on screen has had its 400 ms.
fn close_after_stage(p: &Picker) {
    let delay = {
        let mut pace = p.pace.borrow_mut();
        if let Some(id) = pace.timer.take() {
            id.remove();
        }
        pace.shown
            .map_or(Duration::ZERO, |s| Duration::from_millis(400).saturating_sub(s.elapsed()))
    };
    let p2 = p.clone();
    let close = move || {
        p2.dialog.set_can_close(true);
        p2.dialog.force_close();
    };
    if delay.is_zero() {
        close();
    } else {
        let p3 = p.clone();
        let id = glib::timeout_add_local_once(delay, move || {
            p3.pace.borrow_mut().timer = None;
            close();
        });
        p.pace.borrow_mut().timer = Some(id);
    }
}

/// Put the wait page in `stage` at once: the title in the header bar and the content crossfade,
/// the spinner stays. Cancel dims once the browser has come back, as that cannot be stopped.
fn show_stage(
    p: &Picker,
    service: &SyncFolderKind,
    stage: Stage,
    url: Option<&str>,
    error: Option<&(String, String)>,
) {
    let name = service_name(service);
    p.stage.set(stage);
    let (title, line) = match stage {
        Stage::Browser => (t::cloud_browser_title(&name), t::cloud_browser_line(&name)),
        Stage::Connecting => (t::cloud_connecting(&name), t::cloud_connecting_line()),
        Stage::SettingUp => (t::cloud_setting_up(), t::cloud_setting_up_line(&name)),
        Stage::Error => error.cloned().unwrap_or_default(),
        Stage::Key => (t::rkey_title(), String::new()),
    };
    p.wait_page.set_title(&title);
    p.head.set_title(&title);
    p.wait_title.set_label(&t::cloud_waiting(&name));
    p.wait_title.set_visible(stage == Stage::Browser);
    p.wait_line.set_label(&line);
    p.fail_line.set_label(&line);
    p.spinner.set_visible(!matches!(stage, Stage::Error | Stage::Key));
    p.open_again.set_visible(stage == Stage::Browser && url.is_some());
    p.open_again.set_widget_name(url.unwrap_or_default());
    p.body.set_visible_child_name(match stage {
        Stage::Error => "error",
        Stage::Key => "key",
        _ => "progress",
    });
    p.cancel.set_visible(stage != Stage::Key);
    p.cancel.set_sensitive(!matches!(stage, Stage::Connecting | Stage::SettingUp));
    p.primary.set_visible(matches!(stage, Stage::Error | Stage::Key));
    p.primary.set_label(&if stage == Stage::Key {
        t::action_done()
    } else {
        t::action_try_again()
    });
    p.dialog.set_can_close(matches!(stage, Stage::Browser | Stage::Error));
    go(p, "wait");
}

fn present_picker(ctx: &Rc<Ctx>, page: &str, in_use: Option<SyncFolderKind>) -> Picker {
    if let Some(p) = with(|u| u.picker.clone()) {
        go(&p, page);
        return p;
    }
    let status = ctx.hub.known.borrow().sync_group.clone().unwrap_or_default();

    // The list.
    let list_page = column();
    let error = error_label();
    list_page.append(&error);
    let list = boxed();
    for (kind, name) in offered(&status) {
        let (line, icon) = match kind {
            SyncFolderKind::Dropbox => (t::cloud_dropbox_line(), "weather-overcast-symbolic"), // i18n-skip
            SyncFolderKind::GoogleDrive => {
                (t::cloud_google_line(), "weather-overcast-symbolic")
            } // i18n-skip
            SyncFolderKind::OneDrive => (t::cloud_onedrive_line(), "weather-overcast-symbolic"), // i18n-skip
            _ => (t::cloud_nextcloud_line(), "network-server-symbolic"), // i18n-skip
        };
        let row = choice_row(&name, &line, icon);
        mark_in_use(&row, &kind, in_use.as_ref());
        row.connect_activated({
            let ctx = Rc::downgrade(ctx);
            let kind = kind.clone();
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    choose(&ctx, &kind);
                }
            }
        });
        list.append(&row);
    }
    for (kind, title, line, icon) in [
        (
            SyncFolderKind::WebDav,
            t::cloud_webdav(),
            t::cloud_webdav_line(),
            "network-server-symbolic",
        ), // i18n-skip
        (
            SyncFolderKind::Directory,
            t::cloud_folder(&t::device_kind_computer()),
            t::cloud_folder_line(),
            "folder-symbolic", // i18n-skip
        ),
    ] {
        let row = choice_row(&title, &line, icon);
        mark_in_use(&row, &kind, in_use.as_ref());
        row.connect_activated({
            let ctx = Rc::downgrade(ctx);
            let kind = kind.clone();
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    choose(&ctx, &kind);
                }
            }
        });
        list.append(&row);
    }
    list_page.append(&list);
    list_page.append(&note(&match &in_use {
        Some(kind) => {
            format!("{} {}", t::cloud_switch_foot(&service_name(kind)), t::cloud_pick_foot())
        }
        None => t::cloud_pick_foot(),
    }));

    // The wait: one stack whose states crossfade, in the one dialog.
    let progress = gtk::Box::new(gtk::Orientation::Vertical, 12);
    progress.set_halign(gtk::Align::Center);
    let spinner = adw::Spinner::new();
    spinner.set_size_request(40, 40);
    let wait_title = gtk::Label::new(None);
    wait_title.add_css_class("heading");
    let wait_line = line_centered();
    let open_again = button(&t::cloud_open_again(), Some("flat"));
    open_again.set_halign(gtk::Align::Center);
    open_again.set_visible(false);
    open_again.connect_clicked({
        let ctx = Rc::downgrade(ctx);
        move |b| {
            let url = b.widget_name();
            if let (Some(ctx), false) = (ctx.upgrade(), url.is_empty()) {
                open_url(&ctx, &url);
            }
        }
    });
    progress.append(&spinner);
    progress.append(&wait_title);
    progress.append(&wait_line);
    progress.append(&open_again);
    // The error line, with its vermilion glyph, on the content column's left edge.
    let failed = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    failed.set_halign(gtk::Align::Fill);
    failed.set_hexpand(true);
    let fail_glyph = gtk::Image::from_icon_name("dialog-error-symbolic");
    fail_glyph.add_css_class("error");
    fail_glyph.set_valign(gtk::Align::Start);
    fail_glyph.set_accessible_role(gtk::AccessibleRole::Presentation);
    let fail_line = line("");
    fail_line.set_hexpand(true);
    failed.append(&fail_glyph);
    failed.append(&fail_line);
    let key_slot = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let body = crossfade_stack();
    body.add_named(&progress, Some("progress"));
    body.add_named(&failed, Some("error"));
    body.add_named(&key_slot, Some("key"));
    let wait = column();
    wait.append(&body);

    // Nextcloud.
    let nc = column();
    let nc_error = error_label();
    let addr = adw::EntryRow::builder().title(t::cloud_nextcloud_address()).build();
    addr.set_input_purpose(gtk::InputPurpose::Url);
    let nc_group = adw::PreferencesGroup::new();
    nc_group.add(&addr);
    let user = adw::EntryRow::builder().title(t::cloud_user_label()).build();
    let pass = adw::PasswordEntryRow::builder().title(t::cloud_password_label()).build();
    let app_group = adw::PreferencesGroup::new();
    app_group.add(&user);
    app_group.add(&pass);
    app_group.set_visible(false);
    let nc_note = note(&t::cloud_nextcloud_address_note());
    let connect = button(&t::cloud_connect(), Some("suggested-action"));
    connect.set_halign(gtk::Align::End);
    let app_pw = button(&t::cloud_app_password(), Some("flat"));
    app_pw.set_halign(gtk::Align::Start);
    app_pw.connect_clicked({
        let (g, n) = (app_group.clone(), nc_note.clone());
        move |b| {
            g.set_visible(true);
            n.set_label(&t::cloud_app_password_note(&t::device_kind_computer()));
            b.set_visible(false);
        }
    });
    nc.append(&nc_error);
    nc.append(&nc_group);
    nc.append(&nc_note);
    nc.append(&app_group);
    nc.append(&app_pw);
    nc.append(&connect);
    let nc_host: Rc<RefCell<String>> = Rc::default();
    connect.connect_clicked({
        let (ctx, addr, user, pass, g, host) = (
            Rc::downgrade(ctx),
            addr.clone(),
            user.clone(),
            pass.clone(),
            app_group.clone(),
            nc_host.clone(),
        );
        move |_| {
            let Some(ctx) = ctx.upgrade() else { return };
            let typed = addr.text().trim().to_owned();
            if typed.is_empty() {
                return;
            }
            host.replace(host_of(&typed));
            with(|u| u.last = Some(SyncFolderKind::Nextcloud));
            ctx.hub.known.borrow_mut().sync_problem = None;
            let spec = SyncFolderSpec {
                kind: SyncFolderKind::Nextcloud,
                address: Some(typed),
                user: g.is_visible().then(|| user.text().to_string()),
                password: g.is_visible().then(|| Secret(pass.text().to_string())),
            };
            begin(&ctx, spec);
        }
    });

    // WebDAV.
    let dav = column();
    let dav_error = error_label();
    let dav_addr = adw::EntryRow::builder().title(t::cloud_webdav_address_label()).build();
    dav_addr.set_input_purpose(gtk::InputPurpose::Url);
    let dav_user = adw::EntryRow::builder().title(t::cloud_user_label()).build();
    let dav_pass = adw::PasswordEntryRow::builder().title(t::cloud_password_label()).build();
    let dav_group = adw::PreferencesGroup::new();
    for r in
        [dav_addr.upcast_ref::<gtk::Widget>(), dav_user.upcast_ref(), dav_pass.upcast_ref()]
    {
        dav_group.add(r);
    }
    let dav_connect = button(&t::cloud_connect(), Some("suggested-action"));
    dav_connect.set_halign(gtk::Align::End);
    dav_connect.connect_clicked({
        let (ctx, a, u, p) =
            (Rc::downgrade(ctx), dav_addr.clone(), dav_user.clone(), dav_pass.clone());
        move |_| {
            let Some(ctx) = ctx.upgrade() else { return };
            if a.text().trim().is_empty() || u.text().is_empty() {
                return;
            }
            with(|ui| ui.last = Some(SyncFolderKind::WebDav));
            ctx.hub.known.borrow_mut().sync_problem = None;
            let spec = SyncFolderSpec {
                kind: SyncFolderKind::WebDav,
                address: Some(a.text().trim().to_owned()),
                user: Some(u.text().to_string()),
                password: Some(Secret(p.text().to_string())),
            };
            begin(&ctx, spec);
        }
    });
    dav.append(&dav_error);
    dav.append(&dav_group);
    dav.append(&dav_connect);

    // The list is the root page; the forms and the wait are pushed on it, so the header bar's
    // own back button replaces a step label and the push is the system navigation motion.
    let nav = adw::NavigationView::new();
    nav.set_pop_on_escape(false);
    let head = Title::new(&t::cloud_pick_title());
    let primary = button(&t::action_try_again(), Some("suggested-action"));
    primary.set_visible(false);
    let (list_nav, list_cancel) =
        nav_page("list", &t::cloud_pick_title(), &list_page, None, None, None);
    let (nc_nav, nc_cancel) = nav_page("nextcloud", "Nextcloud", &nc, Some(true), None, None); // i18n-skip
    let (dav_nav, dav_cancel) =
        nav_page("webdav", &t::cloud_webdav(), &dav, Some(true), None, None);
    let (wait_page, wait_cancel) = nav_page(
        "wait",
        &t::cloud_browser_title(""),
        &wait,
        Some(false),
        Some(&head),
        Some(&primary),
    );
    // Cancel on a pushed page returns to the list, which stops a sign-in under way.
    for cancel in [&nc_cancel, &dav_cancel, &wait_cancel] {
        cancel.connect_clicked({
            let nav = nav.clone();
            move |_| {
                nav.pop_to_tag("list");
            }
        });
    }
    for page in [&nc_nav, &dav_nav, &wait_page] {
        nav.add(page);
    }
    let pages = match page {
        "nextcloud" => vec![list_nav.clone(), nc_nav.clone()],
        "webdav" => vec![list_nav.clone(), dav_nav.clone()],
        "wait" => vec![list_nav.clone(), wait_page.clone()],
        _ => vec![list_nav.clone()],
    };
    nav.replace(&pages);
    let dialog = adw::Dialog::builder().content_width(480).child(&nav).build();
    dialog.set_title(&t::cloud_pick_title());
    list_cancel.connect_clicked({
        let d = dialog.clone();
        move |_| {
            d.close();
        }
    });
    let picker = Picker {
        dialog: dialog.clone(),
        nav: nav.clone(),
        wait_page,
        error,
        head,
        body,
        spinner,
        wait_title,
        wait_line,
        open_again,
        fail_line,
        key_slot,
        cancel: wait_cancel,
        primary: primary.clone(),
        stage: Rc::new(Cell::new(Stage::Browser)),
        pace: Rc::default(),
        nc_error: nc_error.clone(),
        dav_error,
        nc_host: nc_host.clone(),
    };
    // Leaving a step by Cancel or the back button stops a sign-in that was under way.
    nav.connect_popped({
        let ctx = Rc::downgrade(ctx);
        move |_, _| {
            if let Some(ctx) = ctx.upgrade() {
                ctx.hub.send(Command::CancelSyncSignIn);
                ctx.hub.known.borrow_mut().sync_signin = None;
            }
        }
    });
    primary.connect_clicked({
        let (ctx, p) = (Rc::downgrade(ctx), picker.clone());
        move |_| {
            let Some(ctx) = ctx.upgrade() else { return };
            match p.stage.get() {
                Stage::Key => {
                    ctx.hub.known.borrow_mut().recovery_key = None;
                    p.dialog.set_can_close(true);
                    p.dialog.force_close();
                }
                // Try again starts a new sign-in in the same dialog.
                _ => {
                    if let Some(kind) = with(|u| u.last.clone()) {
                        choose(&ctx, &kind);
                    }
                }
            }
        }
    });
    dialog.connect_closed({
        let p = picker.clone();
        move |_| {
            if let Some(id) = p.pace.borrow_mut().timer.take() {
                id.remove();
            }
            with(|u| u.picker = None);
        }
    });
    with(|u| u.picker = Some(picker.clone()));
    present(ctx, &dialog);
    picker
}

fn line_centered() -> gtk::Label {
    let l = line("");
    l.set_justify(gtk::Justification::Center);
    l.set_xalign(0.5);
    l.set_max_width_chars(44);
    l
}

/// A page of the picker's navigation: its own header bar over `body`, and its Cancel button. The
/// list has Cancel at the start, which closes the dialog; a pushed page has the back button, and
/// Cancel at the end (the wait: at the start). `pushed` is `Some(true)` for a form, whose back button stays, and
/// `Some(false)` for the wait, which only Cancel leaves. The wait gives its header a title that
/// crossfades and a primary button (Try again, Done) beside Cancel.
fn nav_page(
    tag: &str,
    title: &str,
    body: &impl IsA<gtk::Widget>,
    pushed: Option<bool>,
    head: Option<&Title>,
    primary: Option<&gtk::Button>,
) -> (adw::NavigationPage, gtk::Button) {
    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(false);
    let cancel = button(&t::action_cancel(), None);
    match pushed {
        None => header.pack_start(&cancel),
        Some(back) => {
            if let Some(primary) = primary {
                header.pack_end(primary);
            }
            // The wait keeps Cancel at the start, where the list has it, and Try again or Done
            // at the end; a form keeps its back button at the start and Cancel at the end.
            if back {
                header.pack_end(&cancel);
            } else {
                header.pack_start(&cancel);
            }
            header.set_show_back_button(back);
        }
    }
    if let Some(head) = head {
        header.set_title_widget(Some(&head.0));
    }
    // Every page of the picker is 480 wide, the list and each form: a column of 432 between the
    // columns' 24 margins, so the dialog does not change width as the pages push.
    body.set_width_request(432);
    let clamp =
        adw::Clamp::builder().maximum_size(480).tightening_threshold(480).child(body).build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&clamp));
    let page = adw::NavigationPage::builder().title(title).tag(tag).child(&view).build();
    if pushed == Some(false) {
        page.set_can_pop(false);
    }
    (page, cancel)
}

/// Moves the picker to `page`: back to the list, or the form or the wait pushed on it.
fn go(p: &Picker, page: &str) {
    let visible = p.nav.visible_page_tag();
    if visible.as_deref() == Some(page) {
        return;
    }
    if visible.as_deref() != Some("list") {
        p.nav.pop_to_tag("list");
    }
    if page != "list" {
        p.nav.push_by_tag(page);
    }
}

fn host_of(typed: &str) -> String {
    let t = typed.trim();
    let t = t.split("://").nth(1).unwrap_or(t);
    t.split('/').next().unwrap_or(t).to_owned()
}

/// Set a folder up: the first one turns sync on, and Use another service moves the group.
fn begin(ctx: &Rc<Ctx>, folder: SyncFolderSpec) {
    if with(|u| u.changing) {
        ctx.hub
            .send(Command::SyncGroup(SyncCommand::ChangeSyncFolder { folder: Some(folder) }));
    } else {
        ctx.hub.send(Command::StartSync { device_name: typed_device_name(), folder });
    }
}

/// The row of the service in use, in Use another service: dimmed and marked.
fn mark_in_use(row: &adw::ActionRow, kind: &SyncFolderKind, in_use: Option<&SyncFolderKind>) {
    if in_use == Some(kind) {
        row.set_subtitle(&t::cloud_in_use());
        row.set_sensitive(false);
        row.set_activatable(false);
    }
}

/// A row in the picker was chosen.
fn choose(ctx: &Rc<Ctx>, kind: &SyncFolderKind) {
    ctx.hub.known.borrow_mut().sync_problem = None;
    with(|u| {
        u.last = Some(kind.clone());
        u.by_hand = false;
    });
    let Some(p) = with(|u| u.picker.clone()) else { return };
    say_error(&p.error, None);
    match kind {
        SyncFolderKind::GoogleDrive | SyncFolderKind::Dropbox | SyncFolderKind::OneDrive => {
            begin(
                ctx,
                SyncFolderSpec {
                    kind: kind.clone(),
                    address: None,
                    user: None,
                    password: None,
                },
            );
            stage_to(&p, kind, Stage::Browser, None, None);
        }
        SyncFolderKind::Nextcloud => go(&p, "nextcloud"),
        SyncFolderKind::WebDav => go(&p, "webdav"),
        SyncFolderKind::Directory => {
            let chooser = gtk::FileDialog::builder().title(t::cloud_folder_chooser()).build();
            let ctx = Rc::downgrade(ctx);
            let window = ctx.upgrade().map(|c| c.window.clone());
            chooser.select_folder(window.as_ref(), gio::Cancellable::NONE, move |picked| {
                let (Ok(folder), Some(ctx)) = (picked, ctx.upgrade()) else { return };
                let Some(path) = folder.path() else { return };
                begin(
                    &ctx,
                    SyncFolderSpec {
                        kind: SyncFolderKind::Directory,
                        address: Some(path.display().to_string()),
                        user: None,
                        password: None,
                    },
                );
            });
        }
    }
}

/// The title and the line of a failed cloud sign-in: a refusal, a page that timed out, or any
/// other failure before the folder (the details go to Messages) say the sign-in failed; one
/// after it says the folder could not be set up. The engine's own words go to Messages.
fn sign_in_error(
    problem: &SyncProblem,
    service: &SyncFolderKind,
    p: &Picker,
) -> (String, String) {
    let name = service_name(service);
    let signin = t::cloud_err_signin_title(&name);
    match problem {
        SyncProblem::SignInDenied { .. } => (signin, t::cloud_err_denied_short(&name)),
        SyncProblem::SignInExpired { .. } => (signin, t::cloud_err_timeout()),
        SyncProblem::Unreachable => (signin, t::cloud_err_offline(&name)),
        _ if p.pace.borrow().finishing => {
            (t::cloud_err_setup_title(), t::cloud_err_setup_other(&name))
        }
        _ => (signin, t::cloud_err_signin_other(&name)),
    }
}

/// What a problem says in a dialog, for the service last chosen.
pub fn problem_text(
    problem: &SyncProblem,
    service: Option<&SyncFolderKind>,
    host: &str,
) -> String {
    let name = service.map(service_name).unwrap_or_default();
    match problem {
        SyncProblem::Unreachable => match service {
            Some(SyncFolderKind::Nextcloud) => t::cloud_err_no_server(host),
            Some(SyncFolderKind::WebDav) => t::cloud_err_webdav(),
            _ => t::cloud_err_offline(&name),
        },
        SyncProblem::SignInExpired { .. } => t::cloud_err_timeout(),
        SyncProblem::SignInDenied { service } => t::cloud_err_denied(&service_name(service)),
        SyncProblem::GroupExists => t::cloud_err_other_group(),
        SyncProblem::WrongRecoveryKey => t::rkey_wrong(),
        SyncProblem::Other(words) => words.clone(),
        _ => t::devices_row_error(),
    }
}

// ---------------------------------------------------------------- recovery key

/// The recovery key's content, into `body`: its line, the key in groups of four, Copy, Save and
/// the QR code, and the note on how to see it again.
fn fill_key(ctx: &Rc<Ctx>, body: &gtk::Box, key: &str) {
    while let Some(child) = body.first_child() {
        body.remove(&child);
    }
    body.append(&line(&t::rkey_line()));
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("key-card");
    let groups = super::key_groups(key);
    for row in groups.chunks(3) {
        let l = gtk::Label::new(Some(&row.join("  ")));
        l.add_css_class("key-grid");
        l.set_selectable(true);
        card.append(&l);
    }
    let words = groups.join(" ");
    card.update_property(&[gtk::accessible::Property::Label(&t::rkey_title())]);
    body.append(&card);
    let qr_slot = gtk::Revealer::new();
    qr_slot.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    qr_slot.set_child(Some(&qr_area(&groups.concat(), 168)));
    body.append(&qr_slot);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let copy = button(&t::rkey_copy(), None);
    let save = button(&t::rkey_save_as_file(), None);
    let qr = button(&t::rkey_qr_short(), None);
    for b in [&copy, &save, &qr] {
        buttons.append(b);
    }
    body.append(&buttons);
    body.append(&note(&t::rkey_lock_linux()));
    copy.connect_clicked({
        let words = words.clone();
        move |b| b.clipboard().set_text(&words)
    });
    qr.connect_clicked({
        let slot = qr_slot.clone();
        move |_| slot.set_reveal_child(!slot.reveals_child())
    });
    save.connect_clicked({
        let (ctx, words) = (Rc::downgrade(ctx), words.clone());
        move |_| {
            let Some(ctx) = ctx.upgrade() else { return };
            let chooser =
                gtk::FileDialog::builder().initial_name("kaeri-recovery-key.txt").build(); // i18n-skip
            let words = words.clone();
            chooser.save(Some(&ctx.window), gio::Cancellable::NONE, move |file| {
                if let Ok(file) = file
                    && let Some(path) = file.path()
                {
                    let _ = std::fs::write(path, format!("{words}\n"));
                }
            });
        }
    });
}

/// The recovery key on its own, when it is seen again from Devices. The first time, it shows in
/// the picker's dialog, where the folder was made.
pub fn key_dialog(ctx: &Rc<Ctx>) {
    let Some(key) = ctx.hub.known.borrow().recovery_key.clone() else { return };
    if let Some(d) = with(|u| u.key.take()) {
        d.force_close();
    }
    let body = column();
    fill_key(ctx, &body, &key);
    let (dialog, header, _) = frame(&t::rkey_title(), 480, &body);
    let done = button(&t::action_done(), Some("suggested-action"));
    header.pack_end(&done);
    dialog.set_can_close(false);
    done.connect_clicked({
        let (ctx, d) = (Rc::downgrade(ctx), dialog.clone());
        move |_| {
            if let Some(ctx) = ctx.upgrade() {
                ctx.hub.known.borrow_mut().recovery_key = None;
            }
            d.force_close();
        }
    });
    dialog.set_default_widget(Some(&done));
    dialog.connect_closed(|_| with(|u| u.key = None));
    with(|u| u.key = Some(dialog.clone()));
    present(ctx, &dialog);
}

/// Show the key again, after the account password where polkit is there to ask.
pub fn show_key_again(ctx: &Rc<Ctx>) {
    let ctx = Rc::downgrade(ctx);
    super::lock::ask(move || {
        if let Some(ctx) = ctx.upgrade() {
            ctx.hub.send(Command::ShowRecoveryKey);
        }
    });
}

/// Make a new recovery key, after the account password where polkit is there to ask.
pub fn make_new_key(ctx: &Rc<Ctx>) {
    let ctx = Rc::downgrade(ctx);
    super::lock::ask(move || {
        if let Some(ctx) = ctx.upgrade() {
            ctx.hub.send(Command::NewRecoveryKey);
        }
    });
}

/// Restore: a folder that holds history asks for its recovery key.
pub fn restore(ctx: &Rc<Ctx>, service: &SyncFolderKind) {
    if let Some(r) = with(|u| u.restore.take()) {
        r.dialog.force_close();
    }
    let count =
        ctx.hub.known.borrow().sync_group.as_ref().map_or(0, |s| s.devices.len().max(1) as i64);
    let body = column();
    body.append(&line(&t::rkey_enter_line(&service_name(service), count.max(2))));
    let group = adw::PreferencesGroup::new();
    let entry = adw::EntryRow::builder().title(t::rkey_field_label()).build();
    group.add(&entry);
    body.append(&group);
    body.append(&note(&t::rkey_enter_note()));
    let error = error_label();
    body.append(&error);
    let (dialog, header, _) = frame(&t::rkey_enter_title(), 480, &body);
    let cancel = button(&t::action_cancel(), None);
    let go = button(&t::rkey_restore(), Some("suggested-action"));
    go.set_sensitive(false);
    header.pack_start(&cancel);
    header.pack_end(&go);
    entry.connect_changed({
        let go = go.clone();
        move |e| {
            go.set_sensitive(
                e.text().chars().filter(char::is_ascii_alphanumeric).count() >= 24,
            );
        }
    });
    cancel.connect_clicked({
        let d = dialog.clone();
        move |_| {
            d.close();
        }
    });
    go.connect_clicked({
        let (ctx, entry, service, error) =
            (Rc::downgrade(ctx), entry.clone(), service.clone(), error.clone());
        move |_| {
            let Some(ctx) = ctx.upgrade() else { return };
            say_error(&error, None);
            ctx.hub.known.borrow_mut().sync_problem = None;
            ctx.hub.send(Command::JoinSync {
                device_name: typed_device_name(),
                folder: SyncFolderSpec {
                    kind: service.clone(),
                    address: None,
                    user: None,
                    password: None,
                },
                recovery_key: Secret(entry.text().to_string()),
            });
        }
    });
    dialog.connect_closed(|_| with(|u| u.restore = None));
    with(|u| u.restore = Some(Restore { dialog: dialog.clone(), error }));
    present(ctx, &dialog);
}

/// For a shot: the restore dialog with the wrong-key line.
#[cfg(feature = "fixtures")]
pub fn restore_wrong(ctx: &Rc<Ctx>) {
    restore(ctx, &SyncFolderKind::GoogleDrive);
    if let Some(r) = with(|u| u.restore.clone()) {
        say_error(&r.error, Some(&t::rkey_wrong()));
    }
}

// ---------------------------------------------------------------- pairing

fn address_text(port: u16) -> String {
    let host = super::local_address().unwrap_or_else(|| "127.0.0.1".to_owned());
    format!("{host}:{port}")
}

/// Add a device by showing a code for the other one to enter.
pub fn pair_show(ctx: &Rc<Ctx>, origin: Origin) {
    ctx.hub.known.borrow_mut().invite = None;
    ctx.hub.send(Command::PairInvite { device_name: typed_device_name() });
    if origin == Origin::FirstLaunch {
        // A new device asks the one it pairs with for an invite to each plugin store it is in,
        // inside the pairing; what is held is spent when Done says which to join.
        ctx.hub.send(Command::SyncGroup(SyncCommand::JoinStoresFrom {
            device: None,
            stores: Vec::new(),
            address: None,
        }));
    }
    present_pair_show(ctx, origin);
}

pub fn present_pair_show(ctx: &Rc<Ctx>, origin: Origin) {
    if let Some(p) = with(|u| u.pair.take()) {
        p.dialog.force_close();
    }
    let stack = crossfade_stack();
    let showing = column();
    showing.set_halign(gtk::Align::Center);
    let intro = match origin {
        Origin::Devices => format!("{}. {}", t::pair_show_title(), t::pair_show_scan()),
        Origin::FirstLaunch => t::first_setup_line(),
    };
    let lede = line_centered();
    lede.set_label(&intro);
    showing.append(&lede);
    let qr = qr_area("", 168);
    showing.append(&qr);
    let code = gtk::Label::new(Some("\u{2013}\u{2013}\u{2013} \u{2013}\u{2013}\u{2013}"));
    code.add_css_class("pair-code");
    code.set_selectable(true);
    showing.append(&code);
    let left = gtk::Label::new(None);
    left.add_css_class("haze");
    left.add_css_class("caption");
    left.add_css_class("monospace");
    showing.append(&left);
    let address = gtk::Label::new(None);
    address.add_css_class("haze");
    address.add_css_class("caption");
    showing.append(&address);
    stack.add_named(&showing, Some("showing"));
    let ok = column();
    let paired_title = gtk::Label::new(None);
    paired_title.add_css_class("title-3");
    ok.append(&paired_title);
    ok.append(&{
        let l = line_centered();
        l.set_label(&if origin == Origin::FirstLaunch {
            t::first_ok_line()
        } else {
            t::pair_ok_line()
        });
        l
    });
    let copy_slot = column();
    copy_slot.set_margin_top(0);
    copy_slot.set_margin_bottom(0);
    copy_slot.set_margin_start(0);
    copy_slot.set_margin_end(0);
    ok.append(&copy_slot);
    let join_slot = column();
    join_slot.set_margin_top(0);
    join_slot.set_margin_bottom(0);
    join_slot.set_margin_start(0);
    join_slot.set_margin_end(0);
    ok.append(&join_slot);
    stack.add_named(&ok, Some("paired"));
    let (dialog, header, _) = frame(&t::devices_add(), 440, &stack);
    let done = button(&t::action_done(), Some("suggested-action"));
    header.pack_end(&done);
    let joins: Rc<RefCell<Vec<(String, adw::SwitchRow)>>> = Rc::default();
    let other: Rc<RefCell<Option<(String, String)>>> = Rc::default();
    done.connect_clicked({
        let (ctx, d, joins, other) =
            (Rc::downgrade(ctx), dialog.clone(), joins.clone(), other.clone());
        move |_| {
            // Done spends the invites of the stores left on, through the device that paired.
            let chosen: Vec<String> = joins
                .borrow()
                .iter()
                .filter(|(_, row)| row.is_active())
                .map(|(address, _)| address.clone())
                .collect();
            if let (Some(ctx), Some((id, _)), false) =
                (ctx.upgrade(), other.borrow().clone(), chosen.is_empty())
            {
                ctx.hub.send(Command::SyncGroup(SyncCommand::JoinStoresFrom {
                    device: Some(id),
                    stores: chosen,
                    address: None,
                }));
            }
            d.close();
        }
    });
    let started_with =
        ctx.hub.known.borrow().sync_group.as_ref().map_or(0, |s| s.devices.len());
    let pair = Pair {
        dialog: dialog.clone(),
        stack,
        code,
        left,
        qr,
        address,
        paired_title,
        copy_slot,
        join_slot,
        joins,
        other,
        origin,
        started_with,
    };
    dialog.connect_closed({
        let ctx = Rc::downgrade(ctx);
        move |_| {
            with(|u| u.pair = None);
            if let Some(ctx) = ctx.upgrade() {
                // Always: a device that stays advertised lists after its code has run out.
                ctx.hub.send(Command::PairStop);
                ctx.hub.known.borrow_mut().invite = None;
            }
        }
    });
    with(|u| u.pair = Some(pair.clone()));
    refresh_pair(ctx, &pair);
    // The countdown, once a second, for as long as the dialog is up.
    glib::timeout_add_seconds_local(1, {
        let (ctx, dialog) = (Rc::downgrade(ctx), pair.dialog.downgrade());
        move || {
            let (Some(ctx), Some(dialog)) = (ctx.upgrade(), dialog.upgrade()) else {
                return glib::ControlFlow::Break;
            };
            if dialog.parent().is_none() {
                return glib::ControlFlow::Break;
            }
            if let Some(p) = with(|u| u.pair.clone()) {
                refresh_pair(&ctx, &p);
            }
            glib::ControlFlow::Continue
        }
    });
    present(ctx, &dialog);
}

fn refresh_pair(ctx: &Rc<Ctx>, p: &Pair) {
    let invite = ctx.hub.known.borrow().invite.clone();
    let Some((code, port, until)) = invite else { return };
    let left = until - glib::real_time() / 1_000_000;
    p.code.set_label(&super::spaced_code(&code));
    let addr = address_text(port);
    p.address.set_label(&addr);
    let link = format!("kaeri://pair/{code}@{addr}");
    // Redrawn only when the code changed.
    if p.qr.widget_name() != code {
        let fresh = qr_area(&link, 168);
        if let Some(parent) = p.qr.parent().and_downcast::<gtk::Box>() {
            parent.insert_child_after(&fresh, Some(&p.qr));
            parent.remove(&p.qr);
            with(|u| {
                if let Some(pair) = u.pair.as_mut() {
                    pair.qr = fresh.clone();
                }
            });
            fresh.set_widget_name(&code);
        }
    }
    if left <= 0 {
        p.left.set_label(&t::pair_err_expired());
        p.left.remove_css_class("haze");
        p.left.add_css_class("error");
    } else {
        p.left.set_label(&t::pair_good_for(&super::minutes_seconds(left)));
    }
}

/// A pairing finished: the dialog's content crossfades to the success, with the Copy sign-ins
/// switch when the other device holds sign-ins this one could take.
fn paired(ctx: &Rc<Ctx>, device: &str) {
    let other =
        ctx.hub.known.borrow().sync_group.as_ref().and_then(|s| {
            s.devices.iter().find(|d| !d.this_device && d.name == device).cloned()
        });
    if let Some(p) = with(|u| u.pair.clone()) {
        p.paired_title.set_label(&t::pair_ok(device));
        // First launch asks for them from the start; adding from Devices leaves them off.
        fill_copy(ctx, &p.copy_slot, other.as_ref(), p.origin == Origin::FirstLaunch);
        if let Some(other) = &other {
            p.other.replace(Some((other.id.clone(), other.name.clone())));
        }
        fill_joins(ctx, &p);
        p.stack.set_visible_child_name("paired");
    }
    if let Some(e) = with(|u| u.enter.clone()) {
        e.paired_title.set_label(&t::pair_ok(device));
        fill_copy(ctx, &e.copy_slot, other.as_ref(), false);
        e.stack.set_visible_child_name("paired");
        e.working.set(false);
    }
}

/// First launch: a switch for each plugin store the paired device made an invite for, on from the
/// start. The engine says the offers again as they come, so rows are added for new ones only.
fn fill_joins(ctx: &Rc<Ctx>, p: &Pair) {
    if p.origin != Origin::FirstLaunch {
        return;
    }
    let Some((id, name)) = p.other.borrow().clone() else { return };
    let offers: Vec<_> = ctx
        .hub
        .known
        .borrow()
        .sync_group
        .as_ref()
        .map(|s| s.stores_to_join.clone())
        .unwrap_or_default();
    for offer in offers.into_iter().filter(|o| o.invite_from.as_deref() == Some(id.as_str())) {
        if p.joins.borrow().iter().any(|(address, _)| *address == offer.address) {
            continue;
        }
        let row = adw::SwitchRow::builder()
            .title(glib::markup_escape_text(&t::first_join_store(&offer.name)))
            .subtitle(glib::markup_escape_text(&t::first_join_store_line(
                &name,
                &super::kind(),
            )))
            .active(true)
            .build();
        let group = adw::PreferencesGroup::new();
        group.add(&row);
        p.join_slot.append(&group);
        p.joins.borrow_mut().push((offer.address, row));
    }
}

fn fill_copy(ctx: &Rc<Ctx>, slot: &gtk::Box, device: Option<&SyncDeviceView>, on: bool) {
    // Filled once: the engine says the group again while the success shows.
    if slot.first_child().is_some() {
        return;
    }
    if let Some(device) = device
        && let Some(row) = copy_switch(ctx, device, on)
    {
        let group = adw::PreferencesGroup::new();
        group.add(&row);
        slot.append(&group);
    }
}

/// The name a tracker or cloud sign-in goes by.
fn sign_in_name(id: &str) -> String {
    match id {
        "anilist" => "AniList", // i18n-skip
        "mal" => "MyAnimeList", // i18n-skip
        "simkl" => "Simkl",     // i18n-skip
        "trakt" => "Trakt",     // i18n-skip
        other => other,
    }
    .to_owned()
}

/// What `device` holds sign-ins for, as a list a person reads.
fn sign_in_names(device: &SyncDeviceView) -> String {
    let mut names: Vec<String> = device.trackers.iter().map(|id| sign_in_name(id)).collect();
    names.extend(device.sign_ins.iter().map(service_name));
    names.join(", ")
}

/// The Copy sign-ins switch for `device`, when it holds sign-ins. `on` is where it starts when
/// nothing was asked of the device yet.
pub fn copy_switch(ctx: &Rc<Ctx>, device: &SyncDeviceView, on: bool) -> Option<adw::SwitchRow> {
    if device.trackers.is_empty() && device.sign_ins.is_empty() {
        return None;
    }
    let row = adw::SwitchRow::builder()
        .title(glib::markup_escape_text(&t::pair_copy_signins(&device.name)))
        .subtitle(glib::markup_escape_text(&t::pair_copy_signins_line(&sign_in_names(device))))
        .active(on || !device.asked.is_empty())
        .build();
    let (hub, id) = (Rc::downgrade(&ctx.hub), device.id.clone());
    let mut copy = vec![SyncCopy::Trackers];
    copy.extend(device.sign_ins.iter().cloned().map(|service| SyncCopy::Cloud { service }));
    let ask = {
        let (hub, id, copy) = (hub.clone(), id.clone(), copy.clone());
        move |on: bool| {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::SyncGroup(SyncCommand::CopySignIns {
                    device: Some(id.clone()),
                    copy: if on { copy.clone() } else { Vec::new() },
                    address: None,
                }));
            }
        }
    };
    if on && device.asked.is_empty() {
        ask(true);
    }
    row.connect_active_notify(move |r| ask(r.is_active()));
    Some(row)
}

/// For a shot: the paired success on the showing side.
#[cfg(feature = "fixtures")]
pub fn pair_show_paired(ctx: &Rc<Ctx>, device: &str, origin: Origin) {
    present_pair_show(ctx, origin);
    paired(ctx, device);
}

/// Add a device by entering the code the other one shows.
pub fn pair_enter(ctx: &Rc<Ctx>) {
    if let Some(e) = with(|u| u.enter.take()) {
        e.dialog.force_close();
    }
    let stack = crossfade_stack();
    let body = column();
    body.append(&line(&t::pair_enter_line()));
    let group = adw::PreferencesGroup::new();
    let code = adw::EntryRow::builder().title(t::pair_code_field()).build();
    code.set_input_purpose(gtk::InputPurpose::Digits);
    code.set_max_length(7);
    let address = adw::EntryRow::builder().title(t::pair_address_label()).build();
    group.add(&code);
    // The address is for networks where the code's device can't be found by itself, so it waits
    // behind a text button.
    address.set_visible(false);
    group.add(&address);
    body.append(&group);
    let error = error_label();
    body.append(&error);
    body.append(&note(&t::pair_same_wifi()));
    let address_note = note(&t::pair_address_optional());
    address_note.set_visible(false);
    body.append(&address_note);
    let links = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let show = button(&t::pair_show_instead(), Some("flat"));
    show.set_halign(gtk::Align::Start);
    links.append(&show);
    let by_address = button(&t::pair_by_address(), Some("flat"));
    by_address.connect_clicked({
        let (address, address_note) = (address.clone(), address_note.clone());
        move |b| {
            address.set_visible(true);
            address_note.set_visible(true);
            b.set_visible(false);
            address.grab_focus();
        }
    });
    links.append(&by_address);
    body.append(&links);
    stack.add_named(&body, Some("entering"));
    let ok = column();
    let paired_title = gtk::Label::new(None);
    paired_title.add_css_class("title-3");
    ok.append(&paired_title);
    ok.append(&{
        let l = line_centered();
        l.set_label(&t::pair_ok_line());
        l
    });
    let copy_slot_enter = column();
    copy_slot_enter.set_margin_top(0);
    copy_slot_enter.set_margin_bottom(0);
    copy_slot_enter.set_margin_start(0);
    copy_slot_enter.set_margin_end(0);
    ok.append(&copy_slot_enter);
    stack.add_named(&ok, Some("paired"));
    let (dialog, header, _) = frame(&t::devices_add(), 440, &stack);
    let cancel = button(&t::action_cancel(), None);
    let pair = button(&t::pair_button(), Some("suggested-action"));
    pair.set_sensitive(false);
    header.pack_start(&cancel);
    header.pack_end(&pair);
    let check = {
        let (code, pair) = (code.clone(), pair.clone());
        move |_: &adw::EntryRow| {
            let digits = code.text().chars().filter(char::is_ascii_digit).count();
            pair.set_sensitive(digits == 6);
        }
    };
    code.connect_changed(check);
    cancel.connect_clicked({
        let d = dialog.clone();
        move |_| {
            d.close();
        }
    });
    show.connect_clicked({
        let (ctx, d) = (Rc::downgrade(ctx), dialog.clone());
        move |_| {
            d.close();
            if let Some(ctx) = ctx.upgrade() {
                pair_show(&ctx, Origin::Devices);
            }
        }
    });
    let working = Rc::new(std::cell::Cell::new(false));
    pair.connect_clicked({
        let (ctx, code, address, error, working) =
            (Rc::downgrade(ctx), code.clone(), address.clone(), error.clone(), working.clone());
        move |b| {
            let Some(ctx) = ctx.upgrade() else { return };
            say_error(&error, None);
            ctx.hub.known.borrow_mut().sync_problem = None;
            working.set(true);
            b.set_label(&t::pair_working_line());
            b.set_sensitive(false);
            with(|u| u.pair_said = false);
            let code: String = code.text().chars().filter(char::is_ascii_digit).collect();
            let address = address.text().trim().to_owned();
            // Without an address the engine finds the device showing the code on the network.
            ctx.hub.send(if address.is_empty() {
                Command::SyncGroup(SyncCommand::PairWithCode {
                    device_name: typed_device_name(),
                    code,
                })
            } else {
                Command::PairJoin { device_name: typed_device_name(), address, code }
            });
        }
    });
    dialog.connect_closed(|_| with(|u| u.enter = None));
    let enter = Enter {
        dialog: dialog.clone(),
        stack,
        error,
        pair,
        paired_title,
        copy_slot: copy_slot_enter,
        working,
    };
    with(|u| u.enter = Some(enter));
    present(ctx, &dialog);
}

/// For a shot: the enter dialog showing `error`.
#[cfg(feature = "fixtures")]
pub fn pair_enter_error(ctx: &Rc<Ctx>, text: &str) {
    pair_enter(ctx);
    if let Some(e) = with(|u| u.enter.clone()) {
        say_error(&e.error, Some(text));
    }
}

// ---------------------------------------------------------------- alerts

fn alert(
    ctx: &Rc<Ctx>,
    title: &str,
    body: &str,
    action: &str,
    done: impl Fn() + 'static,
    cancelled: impl Fn() + 'static,
) {
    let dialog = adw::AlertDialog::new(Some(title), Some(body));
    dialog.add_response("cancel", &t::action_cancel());
    dialog.add_response("go", action);
    dialog.set_response_appearance("go", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        if response == "go" {
            done();
        } else {
            cancelled();
        }
    });
    dialog.present(Some(&ctx.window));
}

/// Turn off sync on this computer, after asking.
pub fn turn_off(ctx: &Rc<Ctx>) {
    let device = ctx
        .hub
        .known
        .borrow()
        .sync_group
        .as_ref()
        .and_then(|s| s.device_name.clone())
        .unwrap_or_else(typed_device_name);
    let hub = Rc::downgrade(&ctx.hub);
    alert(
        ctx,
        &t::sync_turn_off_alert(&device),
        &t::sync_turn_off_alert_line(&super::kind()),
        &t::devices_turn_off_action(),
        move || {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::LeaveSync);
            }
        },
        || {},
    );
}

/// Remove the cloud folder, after asking what stays. It signs out of the service too.
pub fn remove_folder(ctx: &Rc<Ctx>, kind: &SyncFolderKind) {
    let hub = Rc::downgrade(&ctx.hub);
    let service = service_name(kind);
    alert(
        ctx,
        &t::cloud_remove_title(&service),
        &t::cloud_remove_line(&super::kind(), &service),
        &t::cloud_remove_action(),
        move || {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::SyncGroup(SyncCommand::ChangeSyncFolder { folder: None }));
            }
        },
        || {},
    );
}

/// Remove a device from the group, after asking.
pub fn remove_device(ctx: &Rc<Ctx>, device: &SyncDeviceView, cancelled: impl Fn() + 'static) {
    let (hub, id) = (Rc::downgrade(&ctx.hub), device.id.clone());
    alert(
        ctx,
        &t::devices_remove_alert(&device.name),
        &t::devices_remove_alert_line(&device.name),
        &t::devices_remove(),
        move || {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::RemoveSyncDevice { device: id.clone() });
            }
        },
        cancelled,
    );
}

/// One device's page: its Sync switch, its Remote switch when it and this device are a TV and a
/// device that drives TVs, Copy sign-ins, and Remove this device.
pub fn device_page(ctx: &Rc<Ctx>, device: &SyncDeviceView) {
    let body = column();
    let group = adw::PreferencesGroup::new();
    let send = |ctx: &Rc<Ctx>, command: SyncCommand| ctx.hub.send(Command::SyncGroup(command));
    let sync = adw::SwitchRow::builder()
        .title(t::devices_sync_switch())
        .active(!device.paused)
        .build();
    sync.connect_active_notify({
        let (ctx, id) = (Rc::downgrade(ctx), device.id.clone());
        move |row| {
            if let Some(ctx) = ctx.upgrade() {
                send(
                    &ctx,
                    SyncCommand::SetDeviceSync { device: id.clone(), on: row.is_active() },
                );
            }
        }
    });
    group.add(&sync);
    // Only for a pair that is a TV and a device that drives TVs.
    if device.remote_key.is_some() {
        let remote = adw::SwitchRow::builder()
            .title(t::devices_remote_switch())
            .active(!device.remote_off)
            .build();
        remote.connect_active_notify({
            let (ctx, id) = (Rc::downgrade(ctx), device.id.clone());
            move |row| {
                if let Some(ctx) = ctx.upgrade() {
                    send(
                        &ctx,
                        SyncCommand::SetDeviceRemote {
                            device: id.clone(),
                            on: row.is_active(),
                        },
                    );
                }
            }
        });
        group.add(&remote);
    }
    body.append(&group);
    // The Remote sentence only beside a Remote switch.
    body.append(&note(&if device.remote_key.is_some() {
        t::devices_switches_foot(&t::device_kind_computer(), &device.name)
    } else {
        t::devices_sync_foot()
    }));
    if let Some(row) = copy_switch(ctx, device, false) {
        let copy = adw::PreferencesGroup::new();
        copy.add(&row);
        body.append(&copy);
    }
    let remove = button(&t::devices_remove(), Some("destructive-action"));
    remove.set_halign(gtk::Align::Start);
    body.append(&remove);
    let (dialog, header, _) = frame(&device.name, 440, &body);
    let done = button(&t::action_done(), Some("suggested-action"));
    header.pack_end(&done);
    done.connect_clicked({
        let d = dialog.clone();
        move |_| {
            d.close();
        }
    });
    remove.connect_clicked({
        let (ctx, device, d) = (Rc::downgrade(ctx), device.clone(), dialog.clone());
        move |_| {
            if let Some(ctx) = ctx.upgrade() {
                d.close();
                remove_device(&ctx, &device, || {});
            }
        }
    });
    present(ctx, &dialog);
}

/// Mark a kept watch unwatched after all, and let the note go.
pub fn undo_note(ctx: &Rc<Ctx>, id: i64, anilist_id: u32, episode: &str) {
    ctx.hub.send(Command::SetWatched {
        id: kaeri_core::ids::AnilistId::new(anilist_id),
        episodes: vec![episode.to_owned()],
        watched: false,
    });
    ctx.hub.send(Command::DismissSyncNote { id });
}

/// This device's new name, said to the group (or kept for the next pairing while sync is off).
/// The Play on TV sender takes it the next time it starts.
pub fn rename(ctx: &Rc<Ctx>, name: &str) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    set_device_name(name);
    ctx.prefs.set_device_name(name);
    ctx.hub.send(Command::SyncGroup(SyncCommand::RenameDevice { name: name.to_owned() }));
}

// ---------------------------------------------------------------- events

/// Move the dialogs on with the engine.
pub fn install(ctx: &Rc<Ctx>) {
    let weak = Rc::downgrade(ctx);
    let seen: Rc<RefCell<HashMap<(), ()>>> = Rc::default();
    let _ = seen;
    ctx.hub.listen(move |event| {
        if let Some(ctx) = weak.upgrade() {
            heard(&ctx, event);
        }
    });
}

fn heard(ctx: &Rc<Ctx>, event: &Event) {
    match event {
        Event::SyncSignIn { service, step: Some(step) } => {
            let Some(p) = with(|u| u.picker.clone()) else { return };
            match step {
                SyncSignInStep::Browser { url } => {
                    stage_to(&p, service, Stage::Browser, Some(url), None)
                }
                SyncSignInStep::Connecting => {
                    stage_to(&p, service, Stage::Connecting, None, None)
                }
                SyncSignInStep::Finishing => {
                    p.pace.borrow_mut().finishing = true;
                    stage_to(&p, service, Stage::SettingUp, None, None);
                }
                _ => {}
            }
        }
        Event::SyncSignIn { step: None, .. } => {
            if ctx.hub.known.borrow().sync_problem.is_none()
                && let Some(p) = with(|u| u.picker.clone())
                && p.stage.get() != Stage::Key
            {
                close_after_stage(&p);
            }
        }
        Event::SyncRecoveryKey(key) => {
            match with(|u| u.picker.clone()) {
                // The folder was made from the picker: the key shows in the same dialog.
                Some(p) => {
                    let service =
                        with(|u| u.last.clone()).unwrap_or(SyncFolderKind::GoogleDrive);
                    fill_key(ctx, &p.key_slot, key);
                    stage_to(&p, &service, Stage::Key, None, None);
                }
                None => key_dialog(ctx),
            }
        }
        Event::SyncProblem(problem) => {
            let service = with(|u| u.last.clone());
            let host = with(|u| u.picker.clone())
                .map(|p| p.nc_host.borrow().clone())
                .unwrap_or_default();
            let text = problem_text(problem, service.as_ref(), &host);
            match problem {
                SyncProblem::GroupExists => {
                    if let Some(p) = with(|u| u.picker.clone()) {
                        p.dialog.force_close();
                    }
                    restore(ctx, &service.unwrap_or(SyncFolderKind::GoogleDrive));
                }
                SyncProblem::WrongRecoveryKey => {
                    if let Some(r) = with(|u| u.restore.clone()) {
                        say_error(&r.error, Some(&text));
                    }
                }
                _ => {
                    if let Some(e) = with(|u| u.enter.clone()) {
                        e.working.set(false);
                        let said = with(|u| std::mem::take(&mut u.pair_said));
                        let shown = match problem {
                            SyncProblem::Unreachable => t::pair_err_not_found(),
                            SyncProblem::Other(_) => t::pair_err_code(&t::pair_other_device()),
                            _ => text.clone(),
                        };
                        if !said {
                            say_error(&e.error, Some(&shown));
                        }
                        e.pair.set_label(&t::pair_button());
                        e.pair.set_sensitive(true);
                    }
                    if let Some(p) = with(|u| u.picker.clone()) {
                        match service {
                            Some(SyncFolderKind::Nextcloud) => {
                                say_error(&p.nc_error, Some(&text));
                                go(&p, "nextcloud");
                            }
                            Some(SyncFolderKind::WebDav) => {
                                say_error(&p.dav_error, Some(&text));
                                go(&p, "webdav");
                            }
                            Some(
                                kind @ (SyncFolderKind::GoogleDrive
                                | SyncFolderKind::Dropbox
                                | SyncFolderKind::OneDrive),
                            ) => {
                                // The error is said in the dialog, with Try again.
                                let error = sign_in_error(problem, &kind, &p);
                                stage_to(&p, &kind, Stage::Error, None, Some(error));
                            }
                            _ => {
                                say_error(&p.error, Some(&text));
                                go(&p, "list");
                            }
                        }
                    }
                }
            }
        }
        Event::SyncStoreJoinFailed { device, name, problem, .. } => {
            let device = ctx
                .hub
                .known
                .borrow()
                .sync_group
                .as_ref()
                .and_then(|s| {
                    s.devices.iter().find(|d| d.id == *device).map(|d| d.name.clone())
                })
                .unwrap_or_else(|| device.clone());
            ctx.toast(&match problem {
                SyncStoreProblem::Offline => t::sync_store_err_offline(name, &device),
                SyncStoreProblem::Refused => t::sync_store_err_refused(name),
                SyncStoreProblem::NotAllowed => t::sync_store_err_not_allowed(name, &device),
            });
        }
        Event::SyncStatus(status) => {
            // The first launch's store invites arrive after the pairing says it is done.
            if let Some(p) = with(|u| u.pair.clone()) {
                fill_joins(ctx, &p);
            }
            // A pairing finished when the group has a device it had not.
            if let Some(p) = with(|u| u.pair.clone())
                && status.devices.len() > p.started_with
                && let Some(other) = status.devices.iter().rev().find(|d| !d.this_device)
            {
                paired(ctx, &other.name);
            } else if with(|u| u.enter.as_ref().is_some_and(|e| e.working.get()))
                && let Some(other) = status.devices.iter().rev().find(|d| !d.this_device)
            {
                paired(ctx, &other.name);
            }
        }
        Event::SyncPairFailed { problem, device } => {
            if let Some(e) = with(|u| u.enter.clone()) {
                let text = match problem {
                    SyncPairProblem::WrongCode => {
                        t::pair_err_code(&device.clone().unwrap_or_else(t::pair_other_device))
                    }
                    SyncPairProblem::NotFound => t::pair_err_not_found(),
                };
                e.working.set(false);
                say_error(&e.error, Some(&text));
                e.pair.set_label(&t::pair_button());
                e.pair.set_sensitive(true);
                with(|u| u.pair_said = true);
            }
        }
        Event::SyncInvite { .. } => {
            if let Some(p) = with(|u| u.pair.clone()) {
                refresh_pair(ctx, &p);
            }
        }
        _ => {}
    }
}
