//! Settings, Devices: this device's name, the devices in the group, the cloud folder, the recovery
//! key and Turn off sync last; or, while sync is off, the one button that turns it on. The TVs
//! paired for Play on TV sit here too, and keep working with sync off.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    group::{SyncCommand, SyncDeviceKind, SyncDeviceView, SyncStatus},
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
    if !super::bare(&status) {
        page.add(&your_devices(ctx, &status));
        if let Some(group) = sync_notes(ctx, &status) {
            page.add(&group);
        }
        page.add(&cloud_folder(ctx, &status));
        if let Some(group) = recovery_and_off(ctx, &status) {
            page.add(&group);
        }
        if let Some(group) = crate::tv::paired::group(ctx) {
            page.add(&group);
        }
    } else {
        page.add(&two_ways_in(ctx));
        if let Some(group) = crate::tv::paired::group(ctx) {
            group.set_description(Some(t::devices_paired_tvs_foot().as_str()));
            page.add(&group);
        }
    }
}

/// While sync is off: the two ways in, in one boxed list under what sync shares, and a footnote
/// under it. Pairing a device or adding a cloud folder turns sync on by itself.
fn two_ways_in(ctx: &Rc<Ctx>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t::sync_group())
        .description(t::devices_sync_foot())
        .build();
    let button = |label: &str| {
        let b = gtk::Button::with_label(label);
        b.set_valign(gtk::Align::Center);
        b
    };
    let nearby = adw::ActionRow::builder()
        .title(t::sync_road_nearby())
        .subtitle(t::sync_road_nearby_line())
        .subtitle_lines(0)
        .build();
    let show = button(&t::pair_show_code());
    let enter = button(&t::devices_enter_code());
    nearby.add_suffix(&show);
    nearby.add_suffix(&enter);
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
    let cloud = adw::ActionRow::builder()
        .title(t::sync_road_cloud())
        .subtitle(t::sync_road_cloud_line())
        .subtitle_lines(0)
        .build();
    let add = button(&t::cloud_add());
    cloud.add_suffix(&add);
    add.connect_clicked({
        let ctx = Rc::downgrade(ctx);
        move |_| {
            if let Some(ctx) = ctx.upgrade() {
                dialogs::picker(&ctx);
            }
        }
    });
    group.add(&nearby);
    group.add(&cloud);
    let foot = gtk::Label::new(Some(&t::sync_start_foot()));
    foot.add_css_class("haze");
    foot.add_css_class("caption");
    foot.set_wrap(true);
    foot.set_xalign(0.0);
    foot.set_margin_top(6);
    group.add(&foot);
    group
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
    name.set_widget_name("device-name");
    name.update_property(&[gtk::accessible::Property::Label(&t::devices_name())]);
    name.set_max_length(64);
    // Said to the group on Enter, or with the tick that shows once the text differs.
    let saved = name.text().to_string();
    name.connect_changed(move |entry| {
        let changed = entry.text() != saved;
        entry.set_icon_from_icon_name(
            gtk::EntryIconPosition::Secondary,
            changed.then_some("object-select-symbolic"), // i18n-skip
        );
    });
    name.set_icon_tooltip_text(gtk::EntryIconPosition::Secondary, Some(&t::action_save()));
    name.set_icon_activatable(gtk::EntryIconPosition::Secondary, true);
    name.connect_icon_release({
        let ctx = Rc::downgrade(ctx);
        move |entry, _| {
            if let Some(ctx) = ctx.upgrade() {
                dialogs::rename(&ctx, &entry.text());
            }
        }
    });
    name.connect_activate({
        let ctx = Rc::downgrade(ctx);
        move |entry| {
            if let Some(ctx) = ctx.upgrade() {
                dialogs::rename(&ctx, &entry.text());
            }
        }
    });
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
    let paired_tv = ctx.tv.paired().iter().any(|tv| tv.name == device.name);
    let remote_only = device.folder.is_none() && paired_tv && device.last_seen.is_none();
    let mut text = line;
    if remote_only {
        text = t::devices_remote_only();
    } else if device.remote_key.is_some() || paired_tv {
        text = t::devices_line_remote(&text);
    }
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&device.name))
        .subtitle(mark.markup(&text))
        .activatable(true)
        .build();
    row.add_prefix(&gtk::Image::from_icon_name(device_icon(device)));
    let sync = gtk::Switch::new();
    sync.set_valign(gtk::Align::Center);
    sync.set_active(!device.paused);
    sync.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} {}",
        t::devices_sync_switch(),
        device.name
    ))]);
    // The group's switch: off holds the group back from the device, and it from the group. Taking
    // the device out is on its page.
    sync.connect_state_set({
        let (ctx, id) = (Rc::downgrade(ctx), device.id.clone());
        move |_, on| {
            if let Some(ctx) = ctx.upgrade() {
                ctx.hub.send(Command::SyncGroup(SyncCommand::SetDeviceSync {
                    device: id.clone(),
                    on,
                }));
            }
            glib::Propagation::Proceed
        }
    });
    row.add_suffix(&sync);
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic")); // i18n-skip
    row.connect_activated({
        let (ctx, device) = (Rc::downgrade(ctx), device.clone());
        move |_| {
            if let Some(ctx) = ctx.upgrade() {
                dialogs::device_page(&ctx, &device);
            }
        }
    });
    row
}

fn device_icon(device: &SyncDeviceView) -> &'static str {
    match device.kind {
        Some(SyncDeviceKind::Tv) => "video-display-symbolic", // i18n-skip
        Some(SyncDeviceKind::Phone | SyncDeviceKind::Tablet) => "phone-symbolic", // i18n-skip
        Some(SyncDeviceKind::Computer) => "computer-symbolic", // i18n-skip
        None => {
            let lower = device.name.to_lowercase();
            if lower.contains("tv") {
                "video-display-symbolic" // i18n-skip
            } else if lower.contains("phone") {
                "phone-symbolic" // i18n-skip
            } else {
                "computer-symbolic" // i18n-skip
            }
        }
    }
}

thread_local! {
    /// Titles of kept watches the pane asked for and has not shown yet.
    static WAITING: std::cell::RefCell<std::collections::HashSet<u32>> = Default::default();
}

/// Whether the cards that just arrived hold a title a sync note is waiting for, so the pane
/// shows it.
pub fn waits_for(cards: &[kaeri_model::catalog::Entry]) -> bool {
    WAITING.with_borrow_mut(|w| {
        let mut arrived = false;
        for card in cards {
            arrived |= w.remove(&card.id.get());
        }
        arrived
    })
}

/// What the rules decided against someone's action: a watch kept although another device marked
/// the episode unwatched, with its undo, for 7 days.
fn sync_notes(ctx: &Rc<Ctx>, status: &SyncStatus) -> Option<adw::PreferencesGroup> {
    if status.notes.is_empty() {
        return None;
    }
    let mut notes = status.notes.clone();
    notes.sort_by_key(|n| std::cmp::Reverse(n.at));
    let ids: Vec<u32> = notes.iter().map(|n| n.anilist_id).collect();
    let missing: Vec<_> = {
        let known = ctx.hub.known.borrow();
        ids.iter()
            .filter(|id| !known.entries.contains_key(id))
            .map(|id| kaeri_core::ids::AnilistId::new(*id))
            .collect()
    };
    if !missing.is_empty() {
        WAITING.with_borrow_mut(|w| w.extend(missing.iter().map(|id| id.get())));
        ctx.hub.send(Command::LoadCards { ids: missing });
    }
    let group = adw::PreferencesGroup::builder()
        .title(t::sync_notes())
        .description(t::sync_notes_foot())
        .build();
    for note in notes {
        let title = ctx
            .hub
            .known
            .borrow()
            .entries
            .get(&note.anilist_id)
            .map(|e| e.title.clone())
            .unwrap_or_default();
        let episode = note.episode.parse::<i64>().unwrap_or(0);
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&t::sync_note_kept_watched(
                episode,
                &title,
                &note.unwatch_device,
                &note.watch_device,
            )))
            .title_lines(0)
            .build();
        let undo = gtk::Button::with_label(&t::sync_note_undo());
        undo.set_valign(gtk::Align::Center);
        undo.connect_clicked({
            let (ctx, note) = (Rc::downgrade(ctx), note.clone());
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    dialogs::undo_note(&ctx, note.id, note.anilist_id, &note.episode);
                }
            }
        });
        row.add_suffix(&undo);
        group.add(&row);
    }
    Some(group)
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
    let state = super::cloud_state(status, now(), dialogs::by_hand());
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&super::service_name(&folder.kind)))
        .build();
    let (line, mark) = match &state {
        CloudState::Synced(s) => (s.clone(), Mark::Water),
        CloudState::Syncing => (t::sync_syncing(), Mark::Water),
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
    row.add_prefix(&gtk::Image::from_icon_name("weather-overcast-symbolic")); // i18n-skip
    let service = folder.kind.clone();
    let cloud = matches!(
        service,
        kaeri_model::group::SyncFolderKind::GoogleDrive
            | kaeri_model::group::SyncFolderKind::Dropbox
            | kaeri_model::group::SyncFolderKind::OneDrive
    );
    match state {
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
        }
    }
    row.add_suffix(&more_menu(ctx, &folder.kind, cloud));
    group.add(&row);
    group
}

thread_local! {
    /// The cloud row's More button last drawn, for the shot that opens it.
    static MORE: std::cell::RefCell<Option<glib::WeakRef<gtk::MenuButton>>> = const { std::cell::RefCell::new(None) };
}

/// For a shot: open the cloud row's More menu.
#[cfg(feature = "fixtures")]
pub fn pop_more() {
    if let Some(button) = MORE.with_borrow(|m| m.as_ref().and_then(|w| w.upgrade())) {
        button.popup();
    }
}

/// For a shot: the open More menu's popover with where it sits in `window`, to draw on top of
/// the window, which leaves a popover's own surface out.
#[cfg(feature = "fixtures")]
pub fn shot_more_popover(window: &adw::ApplicationWindow) -> Option<(gtk::Widget, f32, f32)> {
    let button = MORE.with_borrow(|m| m.as_ref().and_then(|w| w.upgrade()))?;
    let popover = button.popover()?;
    if !popover.is_visible() {
        return None;
    }
    let bounds = button.compute_bounds(window)?;
    let width = popover.width() as f32;
    let x = (bounds.x() + bounds.width() / 2.0 - width / 2.0)
        .clamp(0.0, (window.width() as f32 - width).max(0.0));
    Some((popover.upcast(), x, bounds.y() + bounds.height()))
}

/// The cloud row's More button: Use another service, Sign out for a cloud drive and, apart
/// below them, Remove cloud folder, which asks first.
fn more_menu(
    ctx: &Rc<Ctx>,
    kind: &kaeri_model::group::SyncFolderKind,
    cloud: bool,
) -> gtk::MenuButton {
    let actions = gio::SimpleActionGroup::new();
    let another = gio::SimpleAction::new("another", None);
    another.connect_activate({
        let (ctx, kind) = (Rc::downgrade(ctx), kind.clone());
        move |_, _| {
            if let Some(ctx) = ctx.upgrade() {
                dialogs::picker_change(&ctx, &kind);
            }
        }
    });
    actions.add_action(&another);
    let sign_out = gio::SimpleAction::new("signout", None);
    sign_out.connect_activate({
        let (hub, kind) = (Rc::downgrade(&ctx.hub), kind.clone());
        move |_, _| {
            dialogs::signed_out_by_hand();
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::SyncSignOut { service: kind.clone() });
            }
        }
    });
    actions.add_action(&sign_out);
    let remove = gio::SimpleAction::new("remove", None);
    remove.connect_activate({
        let (ctx, kind) = (Rc::downgrade(ctx), kind.clone());
        move |_, _| {
            if let Some(ctx) = ctx.upgrade() {
                dialogs::remove_folder(&ctx, &kind);
            }
        }
    });
    actions.add_action(&remove);

    let menu = gio::Menu::new();
    let ways = gio::Menu::new();
    ways.append(Some(&t::cloud_use_another()), Some("cloud.another"));
    if cloud {
        ways.append(Some(&t::cloud_sign_out()), Some("cloud.signout"));
    }
    menu.append_section(None, &ways);
    // Remove sits apart and is the one item in vermilion, so it is a custom child of the popover.
    let apart = gio::Menu::new();
    let item = gio::MenuItem::new(None, None);
    item.set_attribute_value("custom", Some(&"remove".to_variant()));
    apart.append_item(&item);
    menu.append_section(None, &apart);
    let popover = gtk::PopoverMenu::from_model(Some(&menu));
    let label = gtk::Label::new(Some(&t::cloud_remove()));
    label.add_css_class("error");
    label.set_xalign(0.0);
    let remove_row = gtk::Button::new();
    remove_row.set_child(Some(&label));
    remove_row.add_css_class("flat");
    remove_row.connect_clicked({
        let (popover, remove) = (popover.clone(), remove);
        move |_| {
            popover.popdown();
            remove.activate(None);
        }
    });
    popover.add_child(&remove_row, "remove");
    let more = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic") // i18n-skip
        .popover(&popover)
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    more.update_property(&[gtk::accessible::Property::Label(&t::cloud_more(
        &super::service_name(kind),
    ))]);
    more.set_tooltip_text(Some(&t::cloud_more(&super::service_name(kind))));
    more.insert_action_group("cloud", Some(&actions));
    MORE.with_borrow_mut(|m| *m = Some(more.downgrade()));
    more
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
    if status.recovery_key {
        let fresh = adw::ActionRow::builder()
            .title(t::rkey_new())
            .subtitle(t::rkey_new_foot())
            .subtitle_lines(0)
            .activatable(true)
            .build();
        fresh.add_prefix(&gtk::Image::from_icon_name("view-refresh-symbolic")); // i18n-skip
        fresh.connect_activated({
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    dialogs::make_new_key(&ctx);
                }
            }
        });
        group.add(&fresh);
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
