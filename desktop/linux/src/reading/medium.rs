//! What Kaeri is for on this device: asked once at first run and once when reading arrives,
//! and changed any time in Settings. Each device keeps its own answer; turning a medium off
//! keeps its list, progress and downloads.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    medium::{KaeriIsFor, MediumAsk, MediumCommand},
};

use crate::window::Ctx;

thread_local! {
    static ASKED: Cell<bool> = const { Cell::new(false) };
}

fn name(answer: KaeriIsFor) -> String {
    match answer {
        KaeriIsFor::Watch => t::medium_watch(),
        KaeriIsFor::Read => t::medium_read(),
        KaeriIsFor::Both => t::medium_both(),
    }
}

fn line(answer: KaeriIsFor) -> String {
    match answer {
        KaeriIsFor::Watch => t::medium_watch_line(),
        KaeriIsFor::Read => t::medium_read_line(),
        KaeriIsFor::Both => t::medium_both_line(),
    }
}

/// The three choices as boxed rows with a line each; `chosen` runs with the pick.
fn choices(chosen: impl Fn(KaeriIsFor) + 'static, active: Option<KaeriIsFor>) -> gtk::ListBox {
    let list = crate::widgets::boxed_list();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("medium-choices");
    let all = [KaeriIsFor::Watch, KaeriIsFor::Read, KaeriIsFor::Both];
    for answer in all {
        let row = adw::ActionRow::builder()
            .title(name(answer))
            .subtitle(line(answer))
            .activatable(true)
            .build();
        list.append(&row);
        if Some(answer) == active {
            list.select_row(Some(&row));
        }
    }
    list.connect_row_activated(move |_, row| {
        if let Some(answer) = all.get(row.index() as usize) {
            chosen(*answer);
        }
    });
    list
}

/// Ask what Kaeri is for, once per run, when the engine says to.
pub fn ask(ctx: &Rc<Ctx>, ask: &MediumAsk) {
    if ASKED.with(|a| a.replace(true)) {
        return;
    }
    let dialog = adw::Dialog::new();
    dialog.set_content_width(460);
    dialog.set_can_close(false);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
    column.set_margin_top(24);
    column.set_margin_bottom(24);
    column.set_margin_start(24);
    column.set_margin_end(24);
    let title = gtk::Label::new(Some(&t::onboarding_medium_title()));
    title.add_css_class("title-2");
    title.set_wrap(true);
    column.append(&title);
    let body = match ask {
        MediumAsk::Updated => t::medium_update_line(&t::shell_device_label()),
        _ => t::onboarding_medium_line(),
    };
    let body = gtk::Label::new(Some(&body));
    body.set_wrap(true);
    body.set_xalign(0.0);
    column.append(&body);
    let ask = ask.clone();
    let hub = Rc::downgrade(&ctx.hub);
    let shown = dialog.downgrade();
    column.append(&choices(
        move |answer| {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::Medium(MediumCommand::Answer { ask: ask.clone(), answer }));
            }
            if let Some(dialog) = shown.upgrade() {
                dialog.set_can_close(true);
                dialog.close();
            }
        },
        None,
    ));
    column.append(&crate::widgets::haze(&t::medium_change_later(), "caption"));
    dialog.set_child(Some(&column));
    dialog.present(Some(&ctx.window));
}

/// Settings' row: what Kaeri is for on this device.
pub fn settings_group(ctx: &Rc<Ctx>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t::settings_kaeri_is_for())
        .description(t::settings_medium_footer())
        .build();
    let current = ctx.hub.known.borrow().medium.clone();
    let fixed = current.as_ref().is_some_and(|m| m.fixed);
    let hub = Rc::downgrade(&ctx.hub);
    let list = choices(
        move |answer| {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::Medium(MediumCommand::Choose {
                    answer,
                    delete_downloads: false,
                }));
            }
        },
        current.as_ref().map(|m| m.answer),
    );
    list.set_sensitive(!fixed);
    group.add(&list);
    group
}
