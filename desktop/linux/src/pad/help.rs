//! The controller's help: what each button does where focus is now, in the shortcuts window's
//! place while the last input came from a controller. Buttons appear nowhere else.

use adw::prelude::*;
use kaeri_i18n::t;
use kaeri_model::input::{PadButton, PadContext, pad_help};

use crate::window::Ctx;

/// The button as its cap reads. The letters and shoulder names are the controller's own marks.
fn name(button: PadButton) -> String {
    match button {
        PadButton::South => "A".to_owned(),          // i18n-skip
        PadButton::East => "B".to_owned(),           // i18n-skip
        PadButton::West => "X".to_owned(),           // i18n-skip
        PadButton::North => "Y".to_owned(),          // i18n-skip
        PadButton::LeftShoulder => "L1".to_owned(),  // i18n-skip
        PadButton::RightShoulder => "R1".to_owned(), // i18n-skip
        PadButton::LeftTrigger => "L2".to_owned(),   // i18n-skip
        PadButton::RightTrigger => "R2".to_owned(),  // i18n-skip
        PadButton::DpadUp => t::pad_button_dpad_up(),
        PadButton::DpadDown => t::pad_button_dpad_down(),
        PadButton::DpadLeft => t::pad_button_dpad_left(),
        PadButton::DpadRight => t::pad_button_dpad_right(),
        PadButton::Start => t::pad_button_menu(),
        PadButton::Select => t::pad_button_view(),
        PadButton::LeftStick => t::pad_button_left_stick(),
        PadButton::RightStick => t::pad_button_right_stick(),
    }
}

/// The rows for one context. The four D-pad directions that all do the same read as one row.
fn rows(context: &PadContext) -> Vec<(String, String)> {
    let help = pad_help(context);
    let dpad_words: Vec<&String> =
        [PadButton::DpadUp, PadButton::DpadDown, PadButton::DpadLeft, PadButton::DpadRight]
            .iter()
            .filter_map(|b| help.iter().find(|(button, _)| button == b).map(|(_, w)| w))
            .collect();
    let all_move = dpad_words.len() == 4 && dpad_words.iter().all(|w| *w == dpad_words[0]);
    let mut rows = Vec::new();
    if all_move {
        rows.push((t::pad_button_dpad(), dpad_words[0].clone()));
    }
    for (button, words) in help {
        let dpad = matches!(
            button,
            PadButton::DpadUp
                | PadButton::DpadDown
                | PadButton::DpadLeft
                | PadButton::DpadRight
        );
        if dpad && all_move {
            continue;
        }
        rows.push((name(button), words));
    }
    if matches!(context, PadContext::Browse) {
        rows.push((t::pad_button_right_stick(), t::pad_help_scroll()));
    }
    rows
}

/// Show the help for the window as it is now.
pub fn show(ctx: &Ctx, context: &PadContext) {
    let page = adw::PreferencesPage::new();
    let reader = (t::pad_help_reader(), PadContext::Reader { settings_open: false });
    let answer = crate::pages::answer();
    let groups: Vec<(String, PadContext)> = match context {
        PadContext::Browse => {
            let mut groups = vec![(t::settings_group_general(), PadContext::Browse)];
            if answer.watches() {
                groups.push((
                    t::settings_group_player(),
                    PadContext::Player { controls_shown: false, card: None },
                ));
            }
            if answer.reads() {
                groups.push(reader);
            }
            groups
        }
        PadContext::Reader { .. } => {
            vec![reader, (t::settings_group_general(), PadContext::Browse)]
        }
        PadContext::Player { .. } => vec![
            (
                t::settings_group_player(),
                PadContext::Player { controls_shown: false, card: None },
            ),
            (
                t::pad_help_controls_shown(),
                PadContext::Player { controls_shown: true, card: None },
            ),
        ],
    };
    for (title, context) in groups {
        let group = adw::PreferencesGroup::builder().title(title).build();
        for (button, words) in rows(&context) {
            let row = adw::ActionRow::builder().title(words).build();
            let cap = gtk::Label::new(Some(&button));
            cap.add_css_class("dim-label");
            row.add_suffix(&cap);
            group.add(&row);
        }
        page.add(&group);
    }
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&page));
    let dialog = adw::Dialog::builder()
        .title(t::controller_help_title())
        .content_width(520)
        .content_height(640)
        .child(&view)
        .build();
    crate::widgets::present_light_dismiss(&dialog, &ctx.window);
}
