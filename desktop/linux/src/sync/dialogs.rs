//! The dialogs of sync: how devices share, choosing a cloud folder and waiting for the browser,
//! the recovery key, restoring with it, and pairing both ways. Each is an AdwDialog on the
//! window; the choosing ones never close on a click outside. The engine's events move them on:
//! a cloud sign-in's steps turn the picker into a wait in place (a crossfade), and the return
//! closes every dialog together before the recovery key shows.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    group::{
        SyncCommand, SyncCopy, SyncDeviceView, SyncFolderKind, SyncFolderSpec, SyncPairProblem,
        SyncProblem, SyncSignInStep,
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

#[derive(Clone)]
struct Picker {
    dialog: adw::Dialog,
    stack: gtk::Stack,
    title: Title,
    error: gtk::Label,
    wait_title: gtk::Label,
    wait_line: gtk::Label,
    open_again: gtk::Button,
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
    setting_up: bool,
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

pub fn setting_up() -> bool {
    with(|u| u.setting_up)
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

// ---------------------------------------------------------------- road

/// How should your devices share? Over Devices while sync is off.
pub fn road(ctx: &Rc<Ctx>) {
    let body = column();
    let list = boxed();
    let nearby = choice_row(
        &t::sync_road_nearby(),
        &t::sync_road_nearby_line(),
        "network-wireless-symbolic",
    ); // i18n-skip
    let cloud = choice_row(
        &t::sync_road_cloud(),
        &t::sync_road_cloud_line(),
        "weather-overcast-symbolic",
    ); // i18n-skip
    list.append(&nearby);
    list.append(&cloud);
    body.append(&list);
    body.append(&note(&t::sync_road_later()));
    let (dialog, header, _) = frame(&t::sync_road_title(), 480, &body);
    let cancel = button(&t::action_cancel(), None);
    header.pack_start(&cancel);
    cancel.connect_clicked({
        let d = dialog.clone();
        move |_| {
            d.close();
        }
    });
    nearby.connect_activated({
        let (ctx, d) = (Rc::downgrade(ctx), dialog.clone());
        move |_| {
            d.close();
            if let Some(ctx) = ctx.upgrade() {
                pair_show(&ctx, Origin::Devices);
            }
        }
    });
    cloud.connect_activated({
        let (ctx, d) = (Rc::downgrade(ctx), dialog.clone());
        move |_| {
            d.close();
            if let Some(ctx) = ctx.upgrade() {
                picker(&ctx);
            }
        }
    });
    present(ctx, &dialog);
}

// ---------------------------------------------------------------- picker and wait

/// Choose a cloud folder.
pub fn picker(ctx: &Rc<Ctx>) {
    present_picker(ctx, "list");
}

/// For a shot: the sign-in came back, so the picker closes.
#[cfg(feature = "fixtures")]
pub fn finish_for_shot(_ctx: &Rc<Ctx>) {
    if let Some(p) = with(|u| u.picker.clone()) {
        p.dialog.close();
    }
}

/// For a shot: the picker on one of its pages.
#[cfg(feature = "fixtures")]
pub fn picker_page(ctx: &Rc<Ctx>, page: &str) {
    present_picker(ctx, page);
}

/// The wait for the browser, on its own: Sign in again from the cloud row.
pub fn sign_in_again(ctx: &Rc<Ctx>, service: &SyncFolderKind) {
    with(|u| {
        u.by_hand = false;
        u.last = Some(service.clone());
    });
    ctx.hub.known.borrow_mut().sync_problem = None;
    ctx.hub.send(Command::SyncSignIn { service: service.clone() });
    let p = present_picker(ctx, "wait");
    p.title.set_title(&t::cloud_browser_title(&service_name(service)));
    set_wait(&p, service, None);
}

/// For a shot: the wait page for `service`, the browser opened at `url`.
#[cfg(feature = "fixtures")]
pub fn picker_wait(ctx: &Rc<Ctx>, service: &SyncFolderKind, url: &str) {
    let p = present_picker(ctx, "wait");
    set_wait(&p, service, Some(url));
}

fn set_wait(p: &Picker, service: &SyncFolderKind, url: Option<&str>) {
    let name = service_name(service);
    p.title.set_title(&t::cloud_browser_title(&name));
    p.wait_title.set_label(&t::cloud_waiting(&name));
    p.wait_line.set_label(&t::cloud_browser_line(&name));
    p.open_again.set_visible(url.is_some());
    p.open_again.set_widget_name(url.unwrap_or_default());
    p.stack.set_visible_child_name("wait");
}

fn present_picker(ctx: &Rc<Ctx>, page: &str) -> Picker {
    if let Some(p) = with(|u| u.picker.clone()) {
        p.stack.set_visible_child_name(page);
        return p;
    }
    let status = ctx.hub.known.borrow().sync_group.clone().unwrap_or_default();
    let stack = crossfade_stack();

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
        row.connect_activated({
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    choose(&ctx, &kind);
                }
            }
        });
        list.append(&row);
    }
    list_page.append(&list);
    list_page.append(&note(&t::cloud_pick_foot()));
    stack.add_named(&list_page, Some("list"));

    // The wait.
    let wait = column();
    wait.set_halign(gtk::Align::Center);
    let spinner = adw::Spinner::new();
    spinner.set_size_request(40, 40);
    let wait_title = gtk::Label::new(None);
    wait_title.add_css_class("heading");
    let wait_line = line_centered();
    let open_again = button(&t::cloud_open_again(), None);
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
    wait.append(&spinner);
    wait.append(&wait_title);
    wait.append(&wait_line);
    wait.append(&open_again);
    stack.add_named(&wait, Some("wait"));

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
    nc.append(&step_pill(&stack, &service_name(&SyncFolderKind::Nextcloud)));
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
            ctx.hub.send(Command::StartSync { device_name: typed_device_name(), folder: spec });
        }
    });
    stack.add_named(&nc, Some("nextcloud"));

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
            ctx.hub.send(Command::StartSync { device_name: typed_device_name(), folder: spec });
        }
    });
    dav.append(&step_pill(&stack, &t::cloud_webdav()));
    dav.append(&dav_error);
    dav.append(&dav_group);
    dav.append(&dav_connect);
    stack.add_named(&dav, Some("webdav"));
    stack.set_visible_child_name(page);

    let (dialog, header, title) = frame(&t::cloud_pick_title(), 480, &stack);
    let cancel = button(&t::action_cancel(), None);
    header.pack_start(&cancel);
    // The one place Cancel means two things: from the list it closes, from any later step it
    // stops the sign-in quietly and the picker is back as it was.
    let picker = Picker {
        dialog: dialog.clone(),
        stack: stack.clone(),
        title: title.clone(),
        error,
        wait_title,
        wait_line,
        open_again,
        nc_error: nc_error.clone(),
        dav_error,
        nc_host: nc_host.clone(),
    };
    cancel.connect_clicked({
        let (p, ctx) = (picker.clone(), Rc::downgrade(ctx));
        move |_| {
            let page = p.stack.visible_child_name().unwrap_or_default();
            if page == "list" {
                p.dialog.close();
                return;
            }
            if let Some(ctx) = ctx.upgrade() {
                ctx.hub.send(Command::CancelSyncSignIn);
                ctx.hub.known.borrow_mut().sync_signin = None;
            }
            p.stack.set_visible_child_name("list");
            p.title.set_title(&t::cloud_pick_title());
        }
    });
    // Back to the list from the forms.
    let retitle = {
        let title = title.clone();
        move |s: &gtk::Stack| match s.visible_child_name().as_deref() {
            Some("list") => title.set_title(&t::cloud_pick_title()),
            Some("nextcloud") => title.set_title("Nextcloud"), // i18n-skip
            Some("webdav") => title.set_title(&t::cloud_webdav()),
            _ => {}
        }
    };
    retitle(&stack);
    stack.connect_visible_child_name_notify(retitle);
    dialog.connect_closed(|_| with(|u| u.picker = None));
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

/// The pill at the top of a form page: the service this step is for, with a way back to the list.
fn step_pill(stack: &gtk::Stack, name: &str) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&gtk::Image::from_icon_name("go-previous-symbolic")); // i18n-skip
    content.append(&gtk::Label::new(Some(name)));
    let pill = gtk::Button::builder().child(&content).halign(gtk::Align::Start).build();
    pill.add_css_class("step-pill");
    pill.connect_clicked({
        let stack = stack.clone();
        move |_| stack.set_visible_child_name("list")
    });
    pill
}

fn host_of(typed: &str) -> String {
    let t = typed.trim();
    let t = t.split("://").nth(1).unwrap_or(t);
    t.split('/').next().unwrap_or(t).to_owned()
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
            ctx.hub.send(Command::StartSync {
                device_name: typed_device_name(),
                folder: SyncFolderSpec {
                    kind: kind.clone(),
                    address: None,
                    user: None,
                    password: None,
                },
            });
            set_wait(&p, kind, None);
        }
        SyncFolderKind::Nextcloud => p.stack.set_visible_child_name("nextcloud"),
        SyncFolderKind::WebDav => p.stack.set_visible_child_name("webdav"),
        SyncFolderKind::Directory => {
            let chooser = gtk::FileDialog::builder().title(t::cloud_folder_chooser()).build();
            let ctx = Rc::downgrade(ctx);
            let window = ctx.upgrade().map(|c| c.window.clone());
            chooser.select_folder(window.as_ref(), gio::Cancellable::NONE, move |picked| {
                let (Ok(folder), Some(ctx)) = (picked, ctx.upgrade()) else { return };
                let Some(path) = folder.path() else { return };
                ctx.hub.send(Command::StartSync {
                    device_name: typed_device_name(),
                    folder: SyncFolderSpec {
                        kind: SyncFolderKind::Directory,
                        address: Some(path.display().to_string()),
                        user: None,
                        password: None,
                    },
                });
            });
        }
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

/// The recovery key, once the first cloud folder is set; `from_devices` when it is seen again.
pub fn key_dialog(ctx: &Rc<Ctx>) {
    let Some(key) = ctx.hub.known.borrow().recovery_key.clone() else { return };
    if let Some(d) = with(|u| u.key.take()) {
        d.force_close();
    }
    let body = column();
    body.append(&line(&t::rkey_line()));
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("key-card");
    let groups = super::key_groups(&key);
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
    stack.add_named(&ok, Some("paired"));
    let (dialog, header, _) = frame(&t::devices_add(), 440, &stack);
    let done = button(&t::action_done(), Some("suggested-action"));
    header.pack_end(&done);
    done.connect_clicked({
        let d = dialog.clone();
        move |_| {
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
        p.stack.set_visible_child_name("paired");
    }
    if let Some(e) = with(|u| u.enter.clone()) {
        e.paired_title.set_label(&t::pair_ok(device));
        fill_copy(ctx, &e.copy_slot, other.as_ref(), false);
        e.stack.set_visible_child_name("paired");
        e.working.set(false);
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
    group.add(&address);
    body.append(&group);
    let error = error_label();
    body.append(&error);
    body.append(&note(&t::pair_same_wifi()));
    body.append(&note(&t::pair_address_optional()));
    let show = button(&t::pair_show_instead(), Some("flat"));
    show.set_halign(gtk::Align::Start);
    body.append(&show);
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
    body.append(&note(&t::devices_switches_foot(&device.name)));
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
                SyncSignInStep::Browser { url } => set_wait(&p, service, Some(url)),
                SyncSignInStep::Finishing => {
                    with(|u| u.setting_up = true);
                    // The row says it is setting up only past 600 ms.
                    let ctx = Rc::downgrade(ctx);
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(600),
                        move || {
                            if let Some(ctx) = ctx.upgrade()
                                && with(|u| u.setting_up)
                                && let Some(status) = ctx.hub.known.borrow().sync_group.clone()
                            {
                                ctx.hub.take(vec![Event::SyncStatus(status)]);
                            }
                        },
                    );
                    p.dialog.close();
                }
                _ => {}
            }
        }
        Event::SyncSignIn { step: None, .. } => {
            if ctx.hub.known.borrow().sync_problem.is_none()
                && let Some(p) = with(|u| u.picker.clone())
            {
                p.dialog.close();
            }
        }
        Event::SyncRecoveryKey(_) => {
            with(|u| u.setting_up = false);
            if let Some(p) = with(|u| u.picker.clone()) {
                p.dialog.close();
            }
            key_dialog(ctx);
        }
        Event::SyncProblem(problem) => {
            let service = with(|u| u.last.clone());
            let host = with(|u| u.picker.clone())
                .map(|p| p.nc_host.borrow().clone())
                .unwrap_or_default();
            let text = problem_text(problem, service.as_ref(), &host);
            with(|u| u.setting_up = false);
            match problem {
                SyncProblem::GroupExists => {
                    if let Some(p) = with(|u| u.picker.clone()) {
                        p.dialog.close();
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
                                say_error(&p.nc_error, Some(&text))
                            }
                            Some(SyncFolderKind::WebDav) => {
                                say_error(&p.dav_error, Some(&text))
                            }
                            _ => say_error(&p.error, Some(&text)),
                        }
                        let page = match service {
                            Some(SyncFolderKind::Nextcloud) => "nextcloud",
                            Some(SyncFolderKind::WebDav) => "webdav",
                            _ => "list",
                        };
                        p.stack.set_visible_child_name(page);
                    }
                }
            }
        }
        Event::SyncStatus(status) => {
            if status.folder.is_some() && !status.syncing {
                with(|u| u.setting_up = false);
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
