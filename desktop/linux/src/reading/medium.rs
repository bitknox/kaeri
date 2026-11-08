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

/// Settings' rows: what Kaeri is for on this device, one radio check each. Turning watching or
/// reading off asks first, with deleting its downloads as a switch that starts off.
pub fn settings_group(ctx: &Rc<Ctx>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t::settings_kaeri_is_for())
        .description(t::settings_medium_footer())
        .build();
    let current = ctx.hub.known.borrow().medium.clone();
    let fixed = current.as_ref().is_some_and(|m| m.fixed);
    let list = crate::widgets::boxed_list();
    list.set_selection_mode(gtk::SelectionMode::None);
    let all = [KaeriIsFor::Watch, KaeriIsFor::Read, KaeriIsFor::Both];
    let mut checks: Vec<gtk::CheckButton> = Vec::new();
    for answer in all {
        let check = gtk::CheckButton::new();
        check.set_valign(gtk::Align::Center);
        // The row answers a click; the check only shows the pick.
        check.set_can_target(false);
        check.set_can_focus(false);
        if let Some(first) = checks.first() {
            check.set_group(Some(first));
        }
        check.set_active(current.as_ref().is_some_and(|m| m.answer == answer));
        let row = adw::ActionRow::builder()
            .title(name(answer))
            .subtitle(line(answer))
            .activatable(true)
            .build();
        row.add_prefix(&check);
        list.append(&row);
        checks.push(check);
    }
    let checks = Rc::new(checks);
    list.connect_row_activated({
        let ctx = Rc::downgrade(ctx);
        let checks = Rc::clone(&checks);
        move |_, row| {
            let (Some(ctx), Some(&answer)) = (ctx.upgrade(), all.get(row.index() as usize))
            else {
                return;
            };
            let now = ctx.hub.known.borrow().medium.as_ref().map(|m| m.answer);
            if now == Some(answer) {
                return;
            }
            let picked = {
                let checks = Rc::clone(&checks);
                let index = row.index() as usize;
                move || {
                    if let Some(check) = checks.get(index) {
                        check.set_active(true);
                    }
                }
            };
            let leaves = match answer {
                KaeriIsFor::Both => None,
                KaeriIsFor::Watch => Some(KaeriIsFor::Read),
                KaeriIsFor::Read => Some(KaeriIsFor::Watch),
            };
            match leaves {
                Some(leaves) if now.is_some_and(|n| n == leaves || n == KaeriIsFor::Both) => {
                    turn_off(&ctx, leaves, answer, picked);
                }
                _ => {
                    choose(&ctx, answer, false);
                    picked();
                }
            }
        }
    });
    list.set_sensitive(!fixed);
    group.add(&list);
    group
}

fn choose(ctx: &Ctx, answer: KaeriIsFor, delete_downloads: bool) {
    ctx.hub.send(Command::Medium(MediumCommand::Choose { answer, delete_downloads }));
}

/// "Turn off reading?": what leaves, that the list, progress and downloads stay, and deleting
/// the downloads as its own switch, off.
fn turn_off(
    ctx: &Rc<Ctx>,
    leaves: KaeriIsFor,
    answer: KaeriIsFor,
    picked: impl Fn() + 'static,
) {
    let device = t::shell_device_label();
    let watch = leaves == KaeriIsFor::Watch;
    let (title, body, delete, confirm) = if watch {
        (
            t::medium_off_title_watch(),
            t::medium_off_watch_body(&device),
            t::medium_off_delete_watch(),
            t::medium_off_confirm_watch(),
        )
    } else {
        (
            t::medium_off_title_read(),
            t::medium_off_read_body(&device),
            t::medium_off_delete_read(),
            t::medium_off_confirm_read(),
        )
    };
    let bytes: u64 = {
        let known = ctx.hub.known.borrow();
        if watch {
            known
                .downloads
                .iter()
                .filter(|r| r.state == kaeri_model::downloads::DownloadState::Done)
                .map(|r| r.total.max(r.downloaded))
                .sum()
        } else {
            known.book_downloads.as_ref().map_or(0, |v| v.meter.used)
        }
    };
    let switch = adw::SwitchRow::builder().title(delete).build();
    if bytes > 0 {
        switch
            .set_subtitle(&t::medium_off_delete_size(&gtk::glib::format_size(bytes), &device));
    }
    let rows = crate::widgets::boxed_list();
    rows.set_selection_mode(gtk::SelectionMode::None);
    rows.append(&switch);
    let dialog = adw::AlertDialog::new(Some(&title), Some(&body));
    dialog.set_extra_child(Some(&rows));
    let cancel = t::action_cancel();
    dialog.add_responses(&[("cancel", cancel.as_str()), ("off", confirm.as_str())]);
    dialog.set_response_appearance("off", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let ctx_weak = Rc::downgrade(ctx);
    dialog.connect_response(None, move |_, response| {
        if response != "off" {
            return;
        }
        if let Some(ctx) = ctx_weak.upgrade() {
            choose(&ctx, answer, switch.is_active());
            picked();
        }
    });
    dialog.present(Some(&ctx.window));
}

/// The shots' turn-off sheet: watching going, as the board's figure draws it.
#[cfg(feature = "fixtures")]
pub fn shot_turn_off(ctx: &Rc<Ctx>) {
    turn_off(ctx, KaeriIsFor::Watch, KaeriIsFor::Read, || {});
}
