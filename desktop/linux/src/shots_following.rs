//! People you follow's shots, one per state the board draws, on the following scenes: the title
//! with ten people and Show all, the list grown, the airing title's five, a title nobody followed
//! has, the switch under the AniList account and the page with it off. The slow scene
//! (`--scene following-slow`) takes the placeholders and what they settle into.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use kaeri_core::settings::SettingValue;
use kaeri_model::{command::Command, settings::SettingKey};
use kaeri_testkit::fixtures::{AIRING, FINISHED, ONA};

use crate::{pages::Dest, window::Shell};

/// The following scene's shots, in order.
pub const NAMES: [&str; 6] = [
    "following",
    "following-all",
    "following-five",
    "following-nobody",
    "following-accounts",
    "following-off",
];

/// The slow scene's shots, in order: each placeholder shot is followed by what it settles into.
pub const SLOW: [&str; 4] =
    ["following-loading", "following-settled", "following-loading-nobody", "following-gone"];

fn open(shell: &Shell, id: kaeri_core::ids::AnilistId) {
    shell.go(Dest::Home);
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.open-title",
        Some(&id.get().to_variant()),
    );
}

fn show_following(shell: &Shell, on: bool) {
    shell.ctx.hub.send(Command::SetSetting {
        key: SettingKey::ShowFollowing,
        value: SettingValue::Bool(on),
    });
}

/// The first button under `widget` with the style class `class`.
fn button_with(widget: &gtk::Widget, class: &str) -> Option<gtk::Button> {
    if let Some(button) = widget.downcast_ref::<gtk::Button>()
        && button.has_css_class(class)
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = button_with(&next, class) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// Put the window in the state `shot` shows, and say how long to let it settle; `None` for a
/// shot that is not one of these.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    Some(match shot {
        "following" => {
            open(shell, FINISHED);
            4.0
        }
        "following-all" => {
            open(shell, FINISHED);
            let window = shell.ctx.window.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(3000), move || {
                if let Some(button) = button_with(window.upcast_ref(), "following-all") {
                    button.emit_clicked();
                }
            });
            4.5
        }
        "following-five" => {
            open(shell, AIRING);
            4.0
        }
        "following-nobody" => {
            open(shell, ONA);
            4.0
        }
        "following-accounts" => {
            shell.settings_pane("accounts");
            1.5
        }
        "following-off" => {
            show_following(shell, false);
            open(shell, FINISHED);
            4.0
        }
        // AniList holds the page for 2 s: the placeholders show from 0.6 s.
        "following-loading" => {
            open(shell, FINISHED);
            1.2
        }
        "following-loading-nobody" => {
            open(shell, ONA);
            1.2
        }
        // The same page, once the answer has come and the crossfade or fade out has run.
        "following-settled" | "following-gone" => 3.0,
        _ => return None,
    })
}

/// Undo what `shot` left: the title page closed, the switch back on.
pub fn unstage(shell: &Rc<Shell>, shot: &str) {
    match shot {
        "following" | "following-all" | "following-five" | "following-nobody"
        | "following-settled" | "following-gone" => {
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "following-off" => {
            show_following(shell, true);
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        _ => {}
    }
}
