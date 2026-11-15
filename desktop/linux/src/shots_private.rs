//! The private mode shots, one per state the board draws for Linux: the switch off and on in
//! Settings › Accounts, the main menu's check item, the header bar's badge and its popover, the
//! notice after the keys, the title page's line, and the title's More menu with Always private.

use std::{rc::Rc, time::Duration};

use adw::prelude::*;
use gtk::glib;

use kaeri_model::{
    command::Command,
    medium::{KaeriIsFor, MediumAsk, MediumCommand},
};

use crate::{pages::Dest, window::Shell};

thread_local! {
    static ANSWERED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Every private mode shot, in order.
pub const NAMES: [&str; 8] = [
    "private-switch-off",
    "private-switch-on",
    "private-menu",
    "private-badge",
    "private-badge-popover",
    "private-toast",
    "private-title-line",
    "private-title-menu",
];

/// The shots taken with a popover open over the window.
pub const POPOVERS: [&str; 3] = ["private-menu", "private-badge-popover", "private-title-menu"];

/// The fixture title the title shots open.
const TITLE: u32 = 900_001;

/// Put the window in the state `shot` shows, and say how long to let it settle.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    let ctx = &shell.ctx;
    let window = &ctx.window;
    if !shot.starts_with("private-") {
        return None;
    }
    // The fixture scene asks what Kaeri is for: answered once, so no sheet covers the shots.
    if !ANSWERED.replace(true) {
        ctx.hub.send(Command::Medium(MediumCommand::Answer {
            ask: MediumAsk::Updated,
            answer: KaeriIsFor::Watch,
        }));
    }
    if let Some(dialog) = window.visible_dialog() {
        dialog.force_close();
    }
    Some(match shot {
        "private-switch-off" => {
            crate::private::set(ctx, false);
            shell.settings_pane("accounts");
            1.5
        }
        "private-switch-on" => {
            crate::private::set(ctx, true);
            shell.settings_pane("accounts");
            1.5
        }
        // The main menu open before the item is chosen, so it shows unchecked.
        "private-menu" => {
            crate::private::set(ctx, false);
            shell.go(Dest::Home);
            let ctx = Rc::clone(ctx);
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                ctx.open_main_menu();
            });
            1.6
        }
        "private-badge" => {
            crate::private::set(ctx, true);
            shell.go(Dest::Home);
            1.5
        }
        "private-badge-popover" => {
            crate::private::set(ctx, true);
            shell.go(Dest::Home);
            let window = window.clone();
            glib::timeout_add_local_once(Duration::from_millis(800), move || {
                if let Some(badge) = mapped_badge(window.upcast_ref()) {
                    badge.popup();
                }
            });
            1.8
        }
        // The keys turn it on: the notice shows, and the badge with it.
        "private-toast" => {
            crate::private::set(ctx, false);
            shell.go(Dest::Home);
            let window = window.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                let _ = WidgetExt::activate_action(&window, "win.private", None);
            });
            1.8
        }
        "private-title-line" => {
            crate::private::set(ctx, true);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            3.5
        }
        "private-title-menu" => {
            crate::private::set(ctx, false);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            let window = window.clone();
            glib::timeout_add_local_once(Duration::from_millis(3000), move || {
                if let Some(more) = mapped_more(window.upcast_ref()) {
                    more.popup();
                }
            });
            4.0
        }
        _ => return None,
    })
}

/// Back to where the next shot starts: no notice, no popover, no title page.
pub fn unstage(shell: &Rc<Shell>, shot: &str) {
    if !shot.starts_with("private-") {
        return;
    }
    shell.ctx.notices.dismiss_each();
    // Whichever menu is open: the main menu, the badge's popover or a title's More.
    while let Some(button) = find(shell.ctx.window.upcast_ref(), &|button| {
        button.popover().is_some_and(|p| p.is_visible())
    }) {
        button.popdown();
        if let Some(popover) = button.popover() {
            popover.set_visible(false);
        }
    }
    if shot.starts_with("private-title") {
        let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
    }
}

/// The badge in the header bar on screen.
fn mapped_badge(widget: &gtk::Widget) -> Option<gtk::MenuButton> {
    find(widget, &|button| button.has_css_class("private-badge"))
}

/// The title page's More, beside its main action.
fn mapped_more(widget: &gtk::Widget) -> Option<gtk::MenuButton> {
    find(widget, &|button| {
        button.icon_name().as_deref() == Some("view-more-symbolic")
            && button.has_css_class("circular")
    })
}

fn find(
    widget: &gtk::Widget,
    wanted: &dyn Fn(&gtk::MenuButton) -> bool,
) -> Option<gtk::MenuButton> {
    if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
        && button.is_mapped()
        && wanted(button)
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = find(&next, wanted) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}
