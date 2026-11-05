//! Profiles, optional. One person is the default and sees no picker, no names and no prompt:
//! Settings says "Only you" and offers "Add a profile". From the second profile on, the
//! Profiles pane lists them, the sidebar's account row opens a popover to switch, and a switch
//! starts the app again on the profile chosen so nothing of the last stays in memory.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{command::Command, event::Event, profiles::ProfileView};

use crate::window::Ctx;

/// The pane's groups.
pub fn fill(ctx: &Rc<Ctx>, page: &adw::PreferencesPage) {
    let profiles = ctx.hub.known.borrow().profiles.clone();
    let group = adw::PreferencesGroup::builder().title(t::profiles_title()).build();
    if profiles.len() < 2 {
        let row = adw::ActionRow::builder().title(t::profiles_only_you()).build();
        row.add_prefix(&avatar(&t::profiles_only_you(), 32));
        group.add(&row);
        group.set_description(Some(t::profiles_one_foot().as_str()));
        page.add(&group);
        let add = adw::PreferencesGroup::new();
        let row = adw::ActionRow::builder()
            .title(t::profiles_add())
            .subtitle(t::profiles_add_line())
            .activatable(true)
            .build();
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic")); // i18n-skip
        row.connect_activated({
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    edit_dialog(&ctx, None);
                }
            }
        });
        add.add(&row);
        page.add(&add);
        return;
    }
    for p in &profiles {
        group.add(&profile_row(ctx, p));
    }
    group.set_description(Some(t::profiles_shared_foot().as_str()));
    page.add(&group);
    let add = adw::PreferencesGroup::new();
    let row = adw::ButtonRow::builder().title(t::profiles_add_button()).build();
    row.set_start_icon_name(Some("list-add-symbolic")); // i18n-skip
    row.connect_activated({
        let ctx = Rc::downgrade(ctx);
        move |_| {
            if let Some(ctx) = ctx.upgrade() {
                edit_dialog(&ctx, None);
            }
        }
    });
    add.add(&row);
    page.add(&add);
}

fn avatar(name: &str, size: i32) -> adw::Avatar {
    let a = adw::Avatar::new(size, Some(name), true);
    a.add_css_class("neutral-avatar");
    a
}

fn profile_row(ctx: &Rc<Ctx>, p: &ProfileView) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(glib::markup_escape_text(&p.name)).build();
    row.add_prefix(&avatar(&p.name, 36));
    if p.kids {
        let tag = gtk::Label::new(Some(&t::profiles_kids_tag()));
        tag.add_css_class("tag-pill");
        tag.set_valign(gtk::Align::Center);
        row.add_suffix(&tag);
    }
    let menu = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .valign(gtk::Align::Center)
        .build(); // i18n-skip
    menu.add_css_class("flat");
    menu.update_property(&[gtk::accessible::Property::Label(&t::profiles_more_for(&p.name))]);
    let pop = gtk::Popover::new();
    let items = gtk::Box::new(gtk::Orientation::Vertical, 0);
    for (label, kind) in [(t::profiles_edit_action(), 0), (t::profiles_delete(), 1)] {
        if kind == 1 && p.active {
            continue;
        }
        let b = gtk::Button::with_label(&label);
        b.add_css_class("flat");
        if kind == 1 {
            b.add_css_class("destructive-action");
        }
        b.connect_clicked({
            let (ctx, p, pop) = (Rc::downgrade(ctx), p.clone(), pop.clone());
            move |_| {
                pop.popdown();
                let Some(ctx) = ctx.upgrade() else { return };
                if kind == 0 {
                    edit_dialog(&ctx, Some(p.clone()));
                } else {
                    delete_alert(&ctx, &p);
                }
            }
        });
        items.append(&b);
    }
    pop.set_child(Some(&items));
    menu.set_popover(Some(&pop));
    row.add_suffix(&menu);
    if p.active {
        let check = gtk::Image::from_icon_name("object-select-symbolic"); // i18n-skip
        check.add_css_class("accent");
        row.add_suffix(&check);
    } else {
        let switch = gtk::Button::with_label(&t::profiles_switch());
        switch.set_valign(gtk::Align::Center);
        switch.connect_clicked({
            let (hub, id) = (Rc::downgrade(&ctx.hub), p.id.clone());
            move |_| {
                if let Some(hub) = hub.upgrade() {
                    hub.send(Command::SwitchProfile { id: id.clone() });
                }
            }
        });
        row.add_suffix(&switch);
    }
    row
}

/// Add a profile, or edit `existing`.
pub fn edit_dialog(ctx: &Rc<Ctx>, existing: Option<ProfileView>) {
    let first = ctx.hub.known.borrow().profiles.len() < 2 && existing.is_none();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_start(24);
    body.set_margin_end(24);
    body.set_margin_bottom(24);
    body.set_margin_top(6);
    let group = adw::PreferencesGroup::new();
    let name = adw::EntryRow::builder().title(t::profiles_name_field()).build();
    if let Some(p) = &existing {
        name.set_text(&p.name);
    }
    group.add(&name);
    let kids = adw::SwitchRow::builder()
        .title(t::profiles_kids())
        .subtitle(t::profiles_kids_line())
        .build();
    kids.set_active(existing.as_ref().is_some_and(|p| p.kids));
    group.add(&kids);
    body.append(&group);
    let first_name = adw::EntryRow::builder().title(t::profiles_yours()).build();
    if first {
        let known = ctx.hub.known.borrow();
        let user = known.account().and_then(|a| a.user.clone()).unwrap_or_default();
        first_name.set_text(&user);
        let g = adw::PreferencesGroup::new();
        g.add(&first_name);
        g.set_description(Some(t::profiles_yours_note().as_str()));
        body.append(&g);
    }
    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(false);
    let title = if existing.is_some() { t::profiles_edit_action() } else { t::profiles_new() };
    header.set_title_widget(Some(&adw::WindowTitle::new(&title, "")));
    let cancel = gtk::Button::with_label(&t::action_cancel());
    let save = gtk::Button::with_label(&if existing.is_some() {
        t::action_save()
    } else {
        t::profiles_add_button()
    });
    save.add_css_class("suggested-action");
    save.set_sensitive(existing.is_some());
    header.pack_start(&cancel);
    header.pack_end(&save);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&body));
    let dialog = adw::Dialog::builder()
        .content_width(440)
        .follows_content_size(true)
        .child(&view)
        .build();
    dialog.set_title(&title);
    name.connect_changed({
        let save = save.clone();
        move |e| save.set_sensitive(!e.text().trim().is_empty())
    });
    cancel.connect_clicked({
        let d = dialog.clone();
        move |_| {
            d.close();
        }
    });
    save.connect_clicked({
        let (hub, d, name, kids, first_name) = (
            Rc::downgrade(&ctx.hub),
            dialog.clone(),
            name.clone(),
            kids.clone(),
            first_name.clone(),
        );
        move |_| {
            let Some(hub) = hub.upgrade() else { return };
            let typed = name.text().trim().to_owned();
            match &existing {
                Some(p) => hub.send(Command::EditProfile {
                    id: p.id.clone(),
                    name: typed,
                    kids: kids.is_active(),
                }),
                None => hub.send(Command::AddProfile {
                    name: typed,
                    kids: kids.is_active(),
                    first_name: first
                        .then(|| first_name.text().trim().to_owned())
                        .filter(|n| !n.is_empty()),
                }),
            }
            d.close();
        }
    });
    dialog.present(Some(&ctx.window));
}

fn delete_alert(ctx: &Rc<Ctx>, p: &ProfileView) {
    let dialog = adw::AlertDialog::new(
        Some(&t::profiles_delete_alert(&p.name)),
        Some(&t::profiles_delete_alert_line(&p.name)),
    );
    dialog.add_response("cancel", &t::action_cancel());
    dialog.add_response("delete", &t::profiles_delete());
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let (hub, id) = (Rc::downgrade(&ctx.hub), p.id.clone());
    dialog.connect_response(None, move |_, r| {
        if r == "delete"
            && let Some(hub) = hub.upgrade()
        {
            hub.send(Command::DeleteProfile { id: id.clone() });
        }
    });
    dialog.present(Some(&ctx.window));
}

/// The popover the account row opens once there are two profiles: who is on, a switch, Manage.
pub fn popover(ctx: &Rc<Ctx>) -> gtk::Popover {
    let pop = gtk::Popover::new();
    let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
    col.set_margin_top(6);
    col.set_margin_bottom(6);
    col.set_margin_start(6);
    col.set_margin_end(6);
    let heading = gtk::Label::new(Some(&t::profiles_picker()));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    heading.set_margin_bottom(4);
    col.append(&heading);
    for p in ctx.hub.known.borrow().profiles.iter() {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        line.append(&avatar(&p.name, 28));
        let n = gtk::Label::new(Some(&p.name));
        n.set_hexpand(true);
        n.set_xalign(0.0);
        line.append(&n);
        if p.active {
            line.append(&gtk::Image::from_icon_name("object-select-symbolic")); // i18n-skip
        }
        let b = gtk::Button::builder().child(&line).build();
        b.add_css_class("flat");
        b.update_property(&[gtk::accessible::Property::Label(&t::profiles_avatar_desc(
            &p.name,
        ))]);
        b.connect_clicked({
            let (hub, id, pop, active) =
                (Rc::downgrade(&ctx.hub), p.id.clone(), pop.clone(), p.active);
            move |_| {
                pop.popdown();
                if !active && let Some(hub) = hub.upgrade() {
                    hub.send(Command::SwitchProfile { id: id.clone() });
                }
            }
        });
        col.append(&b);
    }
    col.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let manage = gtk::Button::with_label(&t::profiles_manage());
    manage.add_css_class("flat");
    manage.connect_clicked({
        let (window, pop) = (ctx.window.clone(), pop.clone());
        move |_| {
            pop.popdown();
            let _ = gtk::prelude::WidgetExt::activate_action(
                &window,
                "win.settings-pane",
                Some(&"profiles".to_variant()),
            );
        }
    });
    col.append(&manage);
    pop.set_child(Some(&col));
    pop
}

/// The engine said a switch happened: start the app again on that profile.
pub fn install(ctx: &Rc<Ctx>) {
    let weak = Rc::downgrade(ctx);
    ctx.hub.listen(move |event| {
        if matches!(event, Event::ProfileSwitch { .. })
            && let Some(ctx) = weak.upgrade()
        {
            restart(&ctx);
        }
    });
}

fn restart(ctx: &Rc<Ctx>) {
    if crate::hub::in_guard() {
        return;
    }
    if let Ok(exe) = std::env::current_exe() {
        let args = crate::updates::relaunch_args(std::env::args().skip(1));
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(crate::updates::RELAUNCH)
            .arg("kaeri-relaunch") // i18n-skip
            .arg(std::process::id().to_string())
            .arg(exe)
            .args(args)
            .spawn();
    }
    if let Some(app) = ctx.window.application() {
        app.quit();
    }
}
