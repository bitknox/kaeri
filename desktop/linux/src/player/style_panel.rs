//! The subtitle style panel: a popover from a button beside Subtitles and audio, with the same
//! choices as Settings, Playback, Subtitles. A choice is a setting like any other, so it is kept
//! and the player draws it at once.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use kaeri_i18n::t;
use kaeri_model::{command::Command, event::Event, settings::SettingKey as K};

use crate::{catalog, window::Ctx};

const KEYS: [K; 9] = [
    K::SubtitleSize,
    K::SubtitleFont,
    K::SubtitleColour,
    K::SubtitleEdge,
    K::SubtitleThickness,
    K::SubtitleBackground,
    K::SubtitleOpacity,
    K::SubtitleRaise,
    K::StyledSubtitles,
];

/// The button and its popover. The rows follow Settings as it changes.
pub fn button(ctx: &Rc<Ctx>) -> gtk::MenuButton {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    // Set while the panel sets a row from Settings, so that is not sent back.
    let quiet = Rc::new(Cell::new(false));
    let mut rows = Vec::new();
    for key in KEYS {
        let choices = catalog::choices(key);
        let names: Vec<String> = choices.iter().map(catalog::Choice::label).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let row = adw::ComboRow::builder()
            .title(catalog::label(key))
            .model(&gtk::StringList::new(&names))
            .build();
        row.connect_selected_notify({
            let (ctx, quiet) = (Rc::clone(ctx), Rc::clone(&quiet));
            move |row| {
                if quiet.get() {
                    return;
                }
                if let Some(choice) = catalog::choices(key).get(row.selected() as usize) {
                    ctx.hub.send(Command::SetSetting { key, value: choice.value.clone() });
                }
            }
        });
        list.append(&row);
        rows.push((key, row));
    }
    let fill = {
        let (ctx, quiet) = (Rc::clone(ctx), Rc::clone(&quiet));
        move || {
            quiet.set(true);
            for (key, row) in &rows {
                if let Some(at) =
                    ctx.hub.known.borrow().setting(*key).and_then(|r| catalog::current(&r))
                {
                    row.set_selected(at as u32);
                }
            }
            quiet.set(false);
        }
    };
    fill();
    ctx.hub.listen({
        let fill = fill.clone();
        move |event| {
            if matches!(event, Event::Settings(_)) {
                fill();
            }
        }
    });

    let more = gtk::Button::with_label(&t::player_subs_more());
    more.add_css_class("flat");
    more.set_halign(gtk::Align::Start);
    more.set_action_target_value(Some(&"playback".to_variant()));
    more.set_action_name(Some("win.settings-pane"));
    let reset = gtk::Button::with_label(&t::settings_subs_reset());
    reset.add_css_class("flat");
    reset.set_halign(gtk::Align::End);
    reset.connect_clicked({
        let ctx = Rc::clone(ctx);
        move |_| {
            let before =
                ctx.hub.known.borrow().settings.as_ref().and_then(|s| s.subtitle_style.clone());
            ctx.hub.known.borrow_mut().style_before_reset = before;
            ctx.hub.send(Command::ResetSubtitleStyle);
        }
    });
    let foot = gtk::CenterBox::new();
    foot.set_start_widget(Some(&more));
    foot.set_end_widget(Some(&reset));

    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_margin_top(12);
    column.set_margin_bottom(8);
    column.set_margin_start(12);
    column.set_margin_end(12);
    let title = gtk::Label::new(Some(&t::player_subs_style()));
    title.add_css_class("heading");
    title.set_xalign(0.0);
    column.append(&title);
    column.append(&list);
    column.append(&foot);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(520)
        .child(&column)
        .build();
    scroll.set_size_request(360, -1);
    let popover = gtk::Popover::new();
    popover.set_child(Some(&scroll));
    let button = gtk::MenuButton::builder()
        .icon_name("format-text-bold-symbolic")
        .tooltip_text(t::player_subs_style())
        .valign(gtk::Align::Center)
        .popover(&popover)
        .build();
    button.add_css_class("flat");
    button
}
