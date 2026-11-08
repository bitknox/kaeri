//! What Kaeri is for on this device: asked once at first run and once when reading arrives,
//! and changed any time in Settings. Each device keeps its own answer; turning a medium off
//! keeps its list, progress and downloads.
//!
//! The three choices are radio rows with a check on the chosen one. The sheet after the update
//! starts on Watch, the one on joining on the other devices' answer, and first run on nothing:
//! first run asks on its own page, so its ask waits for first run and is answered there.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    medium::{KaeriIsFor, MediumAsk, MediumCommand},
};

use crate::window::Ctx;

thread_local! {
    static ASKED: Cell<bool> = const { Cell::new(false) };
    /// First run's ask, held while first run is up or about to be, for its own page.
    static FIRST_RUN: RefCell<Option<MediumAsk>> = const { RefCell::new(None) };
    /// First run is showing or about to show.
    static FIRST_RUN_UP: Cell<bool> = const { Cell::new(false) };
}

pub const ALL: [KaeriIsFor; 3] = [KaeriIsFor::Watch, KaeriIsFor::Read, KaeriIsFor::Both];

pub fn name(answer: KaeriIsFor) -> String {
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

/// The three choices as radio rows, a check on the chosen one. `chosen` runs with each pick;
/// `active` is the one chosen as they show, if any.
pub fn choices(
    chosen: impl Fn(KaeriIsFor) + 'static,
    active: Option<KaeriIsFor>,
) -> gtk::ListBox {
    let list = crate::widgets::boxed_list();
    list.add_css_class("medium-choices");
    list.update_property(&[gtk::accessible::Property::Label(&t::onboarding_medium_title())]);
    let marks: Rc<RefCell<Vec<(gtk::Image, adw::ActionRow)>>> = Rc::default();
    let total = ALL.len() as i64;
    for (at, answer) in ALL.into_iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(name(answer))
            .subtitle(line(answer))
            .activatable(true)
            .accessible_role(gtk::AccessibleRole::Radio)
            .build();
        row.upcast_ref::<gtk::Widget>().update_property(&[
            gtk::accessible::Property::Description(&t::medium_choice_position(
                at as i64 + 1,
                total,
            )),
        ]);
        let check = gtk::Image::from_icon_name("object-select-symbolic");
        check.add_css_class("medium-check");
        check.set_accessible_role(gtk::AccessibleRole::Presentation);
        row.add_suffix(&check);
        list.append(&row);
        marks.borrow_mut().push((check, row));
    }
    let mark = {
        let marks = Rc::clone(&marks);
        move |chosen: Option<KaeriIsFor>| {
            for ((check, row), answer) in marks.borrow().iter().zip(ALL) {
                let on = Some(answer) == chosen;
                check.set_opacity(if on { 1.0 } else { 0.0 });
                let state = if on {
                    gtk::AccessibleTristate::True
                } else {
                    gtk::AccessibleTristate::False
                };
                row.upcast_ref::<gtk::Widget>()
                    .update_state(&[gtk::accessible::State::Checked(state)]);
                if on {
                    row.add_css_class("chosen");
                } else {
                    row.remove_css_class("chosen");
                }
            }
        }
    };
    mark(active);
    list.connect_row_activated(move |_, row| {
        if let Some(answer) = ALL.get(row.index() as usize) {
            mark(Some(*answer));
            chosen(*answer);
        }
    });
    list
}

/// What is chosen as the question opens: nothing at first run, Watch after the update, the
/// other devices' answer on joining.
pub fn preset(ask: &MediumAsk) -> Option<KaeriIsFor> {
    match ask {
        MediumAsk::FirstRun => None,
        MediumAsk::Updated => Some(KaeriIsFor::Watch),
        MediumAsk::Joined { answer, .. } => Some(*answer),
    }
}

/// The question's title, its line, and whether "change it in Settings" goes under the choices.
/// First run's line says Settings already, so only the sheets say it.
pub fn words(ask: &MediumAsk) -> (String, String, bool) {
    let device = t::shell_device_label();
    match ask {
        MediumAsk::FirstRun => {
            (t::onboarding_medium_title(), t::onboarding_medium_line(), false)
        }
        MediumAsk::Updated => {
            (t::onboarding_medium_title(), t::medium_update_line(&device), true)
        }
        MediumAsk::Joined { devices, answer } => {
            let names = names(devices);
            let n = devices.len() as i64;
            let title = match answer {
                KaeriIsFor::Watch => t::medium_join_title_watch(&names, n),
                KaeriIsFor::Read => t::medium_join_title_read(&names, n),
                KaeriIsFor::Both => t::medium_join_title_both(&names, n),
            };
            (title, t::medium_join_line(&device), true)
        }
    }
}

/// Names as a sentence lists them: "Alpha", "Alpha and Brook", "Alpha, Brook and Harbor".
fn names(list: &[String]) -> String {
    match list {
        [] => String::new(),
        [one] => one.clone(),
        [head @ .., last] => {
            let first =
                head[1..].iter().fold(head[0].clone(), |list, next| t::names_list(&list, next));
            t::names_pair(&first, last)
        }
    }
}

/// The amber Continue under the choices.
pub fn continue_button() -> gtk::Button {
    let button = gtk::Button::with_label(&t::onboarding_continue());
    button.add_css_class("pill");
    button.add_css_class("suggested-action");
    button.set_halign(gtk::Align::Center);
    button
}

/// Continue is off until one is chosen, with a hint that says why.
pub fn hold_continue(button: &gtk::Button, chosen: bool) {
    button.set_sensitive(chosen);
    button.set_tooltip_text((!chosen).then(t::onboarding_medium_hint).as_deref());
}

/// Ask what Kaeri is for, once per run, when the engine says to. First run's ask waits for
/// first run's own page.
pub fn ask(ctx: &Rc<Ctx>, ask: &MediumAsk) {
    if matches!(ask, MediumAsk::FirstRun) && (FIRST_RUN_UP.get() || !ctx.prefs.welcomed()) {
        FIRST_RUN.replace(Some(ask.clone()));
        return;
    }
    sheet(ctx, ask);
}

/// First run is about to show, and asks on its own page.
pub fn first_run_opens() {
    FIRST_RUN_UP.set(true);
}

/// First run's page answered it.
pub fn first_run_answered(ctx: &Rc<Ctx>, answer: KaeriIsFor) {
    ASKED.set(true);
    ctx.hub.send(Command::Medium(match FIRST_RUN.take() {
        Some(ask) => MediumCommand::Answer { ask, answer },
        None => MediumCommand::Choose { answer, delete_downloads: false },
    }));
}

/// First run closed or never showed: a question it left unanswered shows as the sheet.
pub fn first_run_closed(ctx: &Rc<Ctx>) {
    FIRST_RUN_UP.set(false);
    if let Some(ask) = FIRST_RUN.take() {
        sheet(ctx, &ask);
    }
}

fn sheet(ctx: &Rc<Ctx>, ask: &MediumAsk) {
    if ASKED.with(|a| a.replace(true)) {
        return;
    }
    let dialog = adw::Dialog::new();
    dialog.set_content_width(480);
    dialog.set_can_close(false);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
    column.set_margin_top(28);
    column.set_margin_bottom(24);
    column.set_margin_start(24);
    column.set_margin_end(24);
    let (title, body, settings) = words(ask);
    let title = gtk::Label::new(Some(&title));
    title.add_css_class("title-2");
    title.set_wrap(true);
    title.set_justify(gtk::Justification::Center);
    title.set_accessible_role(gtk::AccessibleRole::Heading);
    column.append(&title);
    let body = gtk::Label::new(Some(&body));
    body.add_css_class("haze");
    body.set_wrap(true);
    body.set_justify(gtk::Justification::Center);
    body.set_max_width_chars(48);
    column.append(&body);
    let picked: Rc<Cell<Option<KaeriIsFor>>> = Rc::new(Cell::new(preset(ask)));
    let go = continue_button();
    go.set_margin_top(8);
    hold_continue(&go, picked.get().is_some());
    let list = choices(
        {
            let (picked, go) = (Rc::clone(&picked), go.downgrade());
            move |answer| {
                picked.set(Some(answer));
                if let Some(go) = go.upgrade() {
                    hold_continue(&go, true);
                }
            }
        },
        picked.get(),
    );
    list.set_margin_top(6);
    column.append(&list);
    if settings {
        column.append(&crate::widgets::haze(&t::medium_change_later(), "caption"));
    }
    column.append(&go);
    go.connect_clicked({
        let (ask, hub, shown) = (ask.clone(), Rc::downgrade(&ctx.hub), dialog.downgrade());
        let picked = Rc::clone(&picked);
        move |_| {
            let Some(answer) = picked.get() else { return };
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::Medium(MediumCommand::Answer { ask: ask.clone(), answer }));
            }
            if let Some(dialog) = shown.upgrade() {
                dialog.set_can_close(true);
                dialog.close();
            }
        }
    });
    dialog.set_child(Some(&column));
    dialog.set_default_widget(Some(&go));
    dialog.present(Some(&ctx.window));
    // Focus on the chosen row, else the first.
    let at = picked.get().and_then(|p| ALL.iter().position(|a| *a == p)).unwrap_or(0);
    glib::idle_add_local_once({
        let list = list.downgrade();
        move || {
            if let Some(row) = list.upgrade().and_then(|l| l.row_at_index(at as i32)) {
                row.grab_focus();
            }
        }
    });
}

/// For a shot: the sheet for `ask`, whatever was asked before.
#[cfg(feature = "fixtures")]
pub fn shot_sheet(ctx: &Rc<Ctx>, ask: &MediumAsk) {
    ASKED.set(false);
    sheet(ctx, ask);
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
