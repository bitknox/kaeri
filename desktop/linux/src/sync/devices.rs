//! Settings, Devices: this device's name, the devices in the group, the cloud folder, the recovery
//! key and Turn off sync last; or, while sync is off, the one button that turns it on. The TVs
//! paired for Play on TV sit here too, and keep working with sync off.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    group::{SyncDeviceView, SyncStatus},
};

use super::{CloudState, Mark, ProblemFix, dialogs};
use crate::window::Ctx;

fn now() -> i64 {
    glib::real_time() / 1_000_000
}

/// The pane's groups for the engine's last word on sync.
pub fn fill(ctx: &Rc<Ctx>, page: &adw::PreferencesPage) {
    let status = ctx.hub.known.borrow().sync_group.clone().unwrap_or_default();
    page.add(&this_device(ctx, &status));
    if status.enabled {
        page.add(&your_devices(ctx, &status));
        page.add(&cloud_folder(ctx, &status));
        if let Some(group) = recovery_and_off(ctx, &status) {
            page.add(&group);
        }
        if let Some(group) = crate::tv::paired::group(ctx) {
            page.add(&group);
        }
    } else {
        let group = adw::PreferencesGroup::builder().title(t::sync_group()).build();
        let row = adw::ActionRow::builder()
            .title(t::sync_off_title())
            .subtitle(t::sync_off_line())
            .build();
        let on = gtk::Button::with_label(&t::sync_turn_on());
        on.add_css_class("suggested-action");
        on.set_valign(gtk::Align::Center);
        on.connect_clicked({
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    dialogs::road(&ctx);
                }
            }
        });
        row.add_suffix(&on);
        group.add(&row);
        page.add(&group);
        if let Some(group) = crate::tv::paired::group(ctx) {
            group.set_description(Some(t::devices_paired_tvs_foot().as_str()));
            page.add(&group);
        }
    }
}

fn this_device(ctx: &Rc<Ctx>, status: &SyncStatus) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(t::devices_this_device()).build();
    let row = adw::ActionRow::builder().title(t::devices_name()).build();
    let name = gtk::Entry::new();
    name.add_css_class("flat");
    name.set_valign(gtk::Align::Center);
    gtk::prelude::EditableExt::set_alignment(&name, 1.0);
    name.set_width_chars(22);
    name.set_text(&device_name(ctx, status));
    name.set_editable(!status.enabled);
    name.set_sensitive(!status.enabled);
    name.set_widget_name("device-name");
    name.update_property(&[gtk::accessible::Property::Label(&t::devices_name())]);
    name.connect_changed(|entry| dialogs::set_device_name(&entry.text()));
    row.add_suffix(&name);
    group.add(&row);
    group
}

/// The name this device takes: the group's when sync is on, else what was typed, else the
/// machine's own.
pub fn device_name(ctx: &Rc<Ctx>, status: &SyncStatus) -> String {
    let _ = ctx;
    status.device_name.clone().unwrap_or_else(dialogs::typed_device_name)
}

fn device_row(ctx: &Rc<Ctx>, device: &SyncDeviceView) -> adw::ActionRow {
    let (line, mark) = super::seen_line(device, now());
    let remote_only = device.folder.is_none()
        && ctx.tv.paired().iter().any(|tv| tv.name == device.name)
        && device.last_seen.is_none();
    let mut text = line;
    if remote_only {
        text = t::devices_remote_only();
    } else if ctx.tv.paired().iter().any(|tv| tv.name == device.name) {
        text = format!("{text} · {}", t::devices_remote_switch());
    }
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&device.name))
        .subtitle(mark.markup(&text))
        .build();
    row.add_prefix(&gtk::Image::from_icon_name(device_icon(&device.name)));
    let sync = gtk::Switch::new();
    sync.set_valign(gtk::Align::Center);
    sync.set_active(true);
    sync.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} {}",
        t::devices_sync_switch(),
        device.name
    ))]);
    // Switching a device off asks, then removes it: the engine has no half way.
    sync.connect_state_set({
        let ctx = Rc::downgrade(ctx);
        let device = device.clone();
        move |switch, on| {
            if !on && let Some(ctx) = ctx.upgrade() {
                dialogs::remove_device(&ctx, &device, switch.clone());
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    });
    row.add_suffix(&sync);
    row
}

fn device_icon(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if lower.contains("tv") {
        "video-display-symbolic" // i18n-skip
    } else if lower.contains("phone") || lower.contains("iphone") {
        "phone-symbolic" // i18n-skip
    } else {
        "computer-symbolic" // i18n-skip
    }
}

fn your_devices(ctx: &Rc<Ctx>, status: &SyncStatus) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(t::devices_yours()).build();
    let others: Vec<_> = status.devices.iter().filter(|d| !d.this_device).collect();
    if others.is_empty() {
        group.add(
            &adw::ActionRow::builder()
                .title(t::devices_only_this())
                .subtitle(t::devices_only_this_line())
                .build(),
        );
    }
    for device in others {
        group.add(&device_row(ctx, device));
    }
    let add = adw::ActionRow::builder().title(t::devices_add()).build();
    add.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic")); // i18n-skip
    let show = gtk::Button::with_label(&t::pair_show_code());
    let enter = gtk::Button::with_label(&t::devices_enter_code());
    for b in [&show, &enter] {
        b.set_valign(gtk::Align::Center);
        add.add_suffix(b);
    }
    show.connect_clicked({
        let ctx = Rc::downgrade(ctx);
        move |_| {
            if let Some(ctx) = ctx.upgrade() {
                dialogs::pair_show(&ctx, dialogs::Origin::Devices);
            }
        }
    });
    enter.connect_clicked({
        let ctx = Rc::downgrade(ctx);
        move |_| {
            if let Some(ctx) = ctx.upgrade() {
                dialogs::pair_enter(&ctx);
            }
        }
    });
    group.add(&add);
    group
}

fn cloud_folder(ctx: &Rc<Ctx>, status: &SyncStatus) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t::cloud_group())
        .description(t::cloud_foot())
        .build();
    let button = |label: &str, class: Option<&str>| {
        let b = gtk::Button::with_label(label);
        b.set_valign(gtk::Align::Center);
        if let Some(c) = class {
            b.add_css_class(c);
        }
        b
    };
    let Some(folder) = status.folder.clone() else {
        let row = adw::ActionRow::builder().title(t::cloud_add()).build();
        row.add_prefix(&gtk::Image::from_icon_name("weather-overcast-symbolic")); // i18n-skip
        let add = button(&t::cloud_add(), None);
        add.connect_clicked({
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    dialogs::picker(&ctx);
                }
            }
        });
        row.set_title("");
        row.add_suffix(&add);
        row.set_activatable_widget(Some(&add));
        group.add(&row);
        return group;
    };
    let state = super::cloud_state(status, now(), dialogs::setting_up(), dialogs::by_hand());
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&super::service_name(&folder.kind)))
        .build();
    let (line, mark) = match &state {
        CloudState::Synced(s) => (s.clone(), Mark::Water),
        CloudState::Syncing => (t::sync_syncing(), Mark::Water),
        CloudState::SettingUp => (t::cloud_setting_up(), Mark::None),
        CloudState::SignedOut => (t::cloud_signed_out_line(&super::kind()), Mark::Hollow),
        CloudState::Problem(text, _) => (text.clone(), Mark::Vermilion),
        CloudState::Offline => (t::cloud_offline(&super::kind()), Mark::Amber),
    };
    if let CloudState::Problem(_, _) = &state {
        row.set_title(&glib::markup_escape_text(&t::cloud_err_title(&super::service_name(
            &folder.kind,
        ))));
    }
    let mut line = mark.markup(&line);
    if let (Some(account), CloudState::Synced(_)) = (&folder.account, &state) {
        line.push_str(&format!(" · {}", glib::markup_escape_text(account)));
    }
    row.set_subtitle(&line);
    if state == CloudState::SettingUp {
        let spinner = adw::Spinner::new();
        spinner.set_size_request(24, 24);
        row.add_prefix(&spinner);
    } else {
        row.add_prefix(&gtk::Image::from_icon_name("weather-overcast-symbolic")); // i18n-skip
    }
    let service = folder.kind.clone();
    let cloud = matches!(
        service,
        kaeri_model::group::SyncFolderKind::GoogleDrive
            | kaeri_model::group::SyncFolderKind::Dropbox
            | kaeri_model::group::SyncFolderKind::OneDrive
    );
    match state {
        CloudState::SettingUp => {
            let b = button(&t::cloud_sync_now(), None);
            b.set_sensitive(false);
            row.add_suffix(&b);
        }
        CloudState::SignedOut | CloudState::Problem(_, ProblemFix::SignInAgain) => {
            let again = matches!(state, CloudState::Problem(..));
            let label = if again { t::cloud_sign_in_again() } else { t::cloud_sign_in() };
            let b = button(&label, again.then_some("suggested-action"));
            b.connect_clicked({
                let ctx = Rc::downgrade(ctx);
                let service = service.clone();
                move |_| {
                    if let Some(ctx) = ctx.upgrade() {
                        dialogs::sign_in_again(&ctx, &service);
                    }
                }
            });
            row.add_suffix(&b);
        }
        CloudState::Problem(_, ProblemFix::ChooseFolder) => {
            let b = button(&t::cloud_choose_folder(), Some("suggested-action"));
            b.connect_clicked({
                let ctx = Rc::downgrade(ctx);
                move |_| {
                    if let Some(ctx) = ctx.upgrade() {
                        dialogs::picker(&ctx);
                    }
                }
            });
            row.add_suffix(&b);
        }
        _ => {
            let now_button = button(&t::cloud_sync_now(), None);
            now_button.set_sensitive(!status.syncing);
            now_button.connect_clicked({
                let hub = Rc::downgrade(&ctx.hub);
                move |_| {
                    if let Some(hub) = hub.upgrade() {
                        hub.send(Command::SyncFolderNow);
                    }
                }
            });
            row.add_suffix(&now_button);
            if cloud {
                let out = button(&t::cloud_sign_out(), Some("flat"));
                out.connect_clicked({
                    let hub = Rc::downgrade(&ctx.hub);
                    move |_| {
                        dialogs::signed_out_by_hand();
                        if let Some(hub) = hub.upgrade() {
                            hub.send(Command::SyncSignOut { service: service.clone() });
                        }
                    }
                });
                row.add_suffix(&out);
            }
        }
    }
    group.add(&row);
    group
}

/// The recovery key row (once a folder holds one) and Turn off sync last, in one box.
fn recovery_and_off(ctx: &Rc<Ctx>, status: &SyncStatus) -> Option<adw::PreferencesGroup> {
    let group = adw::PreferencesGroup::new();
    if status.recovery_key {
        let made = status
            .recovery_key_made
            .and_then(|at| glib::DateTime::from_unix_local(at).ok())
            .and_then(|d| d.format("%-d %B").ok())
            .map(|d| t::rkey_made(&d));
        let row = adw::ActionRow::builder().title(t::rkey_row()).build();
        if let Some(made) = made {
            row.set_subtitle(&made);
        }
        row.add_prefix(&gtk::Image::from_icon_name("dialog-password-symbolic")); // i18n-skip
        let show = gtk::Button::with_label(&t::devices_show_key());
        show.set_valign(gtk::Align::Center);
        show.connect_clicked({
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    dialogs::show_key_again(&ctx);
                }
            }
        });
        row.add_suffix(&show);
        group.add(&row);
    }
    let off = gtk::Button::with_label(&t::sync_turn_off(&super::kind()));
    off.add_css_class("flat");
    off.set_margin_top(6);
    off.set_margin_bottom(6);
    off.connect_clicked({
        let ctx = Rc::downgrade(ctx);
        move |_| {
            if let Some(ctx) = ctx.upgrade() {
                dialogs::turn_off(&ctx);
            }
        }
    });
    let row = gtk::ListBoxRow::builder().child(&off).activatable(false).build();
    group.add(&row);
    Some(group)
}
