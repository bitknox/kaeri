//! The player's Episodes panel: compact rows, and under them a footer with the title and the
//! synopsis of the row under the pointer or the keyboard focus, else the episode playing.
//!
//! The footer is reserved only when the table has a synopsis as the panel is built, so no text
//! arriving later moves the list. A row spoiler protection holds back shows its title and facts.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::glib;
use kaeri_model::{
    episodes::{EpisodeRow, EpisodeText, episode_text},
    hosted::MenuEpisode,
};

use crate::{synopsis, widgets};

/// How tall the list may grow before it scrolls.
const LIST_MAX: i32 = 360;
/// The panel's width, which long titles and text never widen: they cut or wrap inside it.
const WIDTH: i32 = 236;
/// A footer's height: its heading and three lines of text.
const FOOTER: i32 = 92;

/// The footer's words for `row`: the heading, and the synopsis or, where protection holds it
/// back or there is none, the facts.
fn words(episode: &MenuEpisode, row: Option<&EpisodeRow>, protect: bool) -> (String, String) {
    let heading = match &episode.title {
        Some(title) => [episode.label.as_str(), title.as_str()].join(" · "),
        None => episode.label.clone(),
    };
    let Some(row) = row else { return (heading, String::new()) };
    let now = glib::DateTime::now_utc().map_or(0, |t| t.to_unix());
    let date = |at| {
        glib::DateTime::from_unix_local(at)
            .ok()
            .and_then(|d| d.format("%-d %b %Y").ok())
            .map_or_else(String::new, |d| d.to_string())
    };
    let facts = synopsis::facts(row, now, date, None);
    let held = synopsis::protected(row, protect, false);
    match synopsis::text_of(row) {
        Some(text) if !held => (heading, text.to_owned()),
        _ => (heading, facts),
    }
}

/// Build the panel for `episodes`, with `table` for the synopses, and give it to `button`.
/// `protect` is whether spoiler protection is on for the show.
pub fn fill(
    button: &gtk::MenuButton,
    episodes: &[MenuEpisode],
    table: &[EpisodeRow],
    protect: bool,
) {
    let reserved = episode_text(table) == EpisodeText::Present;
    let playing = episodes.iter().find(|e| e.current).cloned();
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.set_activate_on_single_click(true);
    list.add_css_class("ep-panel-list");

    let foot_title = widgets::haze("", "caption");
    foot_title.set_wrap(false);
    foot_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    foot_title.add_css_class("ep-foot-title");
    // A label's natural width is its whole text: kept at one character, the panel's width rules.
    foot_title.set_max_width_chars(1);
    foot_title.set_hexpand(true);
    let foot_text = gtk::Label::new(None);
    foot_text.set_xalign(0.0);
    foot_text.set_yalign(0.0);
    foot_text.set_wrap(true);
    foot_text.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    foot_text.set_lines(3);
    foot_text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    foot_text.add_css_class("ep-foot-text");
    foot_text.set_max_width_chars(1);
    foot_text.set_hexpand(true);
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 3);
    footer.add_css_class("ep-foot");
    footer.set_size_request(-1, FOOTER);
    footer.append(&foot_title);
    footer.append(&foot_text);
    footer.set_visible(reserved);

    let rows: Rc<Vec<(MenuEpisode, Option<EpisodeRow>)>> = Rc::new(
        episodes
            .iter()
            .take(200)
            .map(|episode| {
                let row = table.iter().find(|row| row.number == episode.number).cloned();
                (episode.clone(), row)
            })
            .collect(),
    );
    // The footer shows one row's words, crossfaded: the text fades out and the new in.
    let show = {
        let (title, text) = (foot_title.clone(), foot_text.clone());
        let last = Rc::new(RefCell::new(String::new()));
        move |episode: &MenuEpisode, row: Option<&EpisodeRow>| {
            let (heading, body) = words(episode, row, protect);
            if *last.borrow() == format!("{heading}\n{body}") {
                return;
            }
            last.replace(format!("{heading}\n{body}"));
            title.set_label(&heading);
            text.set_label(&body);
        }
    };
    let show = Rc::new(show);
    if let Some(playing) = &playing {
        let row = table.iter().find(|row| row.number == playing.number);
        show(playing, row);
    }

    let mut current_row = None;
    for (episode, row) in rows.iter() {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        line.add_css_class("ep-row");
        let number = gtk::Label::new(Some(&episode.number));
        number.set_xalign(1.0);
        number.set_width_chars(3);
        number.add_css_class("tabular");
        let title = gtk::Label::new(Some(episode.title.as_deref().unwrap_or(&episode.label)));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(1);
        line.append(&number);
        line.append(&title);
        if episode.current {
            number.add_css_class("ep-current");
        }
        if episode.watched {
            title.add_css_class("haze");
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.add_css_class("haze");
            line.append(&check);
        }
        let item = gtk::ListBoxRow::new();
        item.set_child(Some(&line));
        item.set_action_name(Some("player.episode"));
        item.set_action_target_value(Some(&glib::Variant::from(episode.number.as_str())));
        item.update_property(&[gtk::accessible::Property::Label(&match &episode.title {
            Some(title) => [episode.label.as_str(), title.as_str()].join(" · "),
            None => episode.label.clone(),
        })]);
        // Pointed at or focused, the footer follows; the pointer leaving it goes back to the one
        // playing.
        let motion = gtk::EventControllerMotion::new();
        {
            let (show, episode, row) = (show.clone(), episode.clone(), row.clone());
            motion.connect_enter(move |_, _, _| show(&episode, row.as_ref()));
        }
        {
            let (show, playing, table) = (show.clone(), playing.clone(), table.to_vec());
            motion.connect_leave(move |_| {
                if let Some(playing) = &playing {
                    let row = table.iter().find(|row| row.number == playing.number);
                    show(playing, row);
                }
            });
        }
        item.add_controller(motion);
        let focus = gtk::EventControllerFocus::new();
        {
            let (show, episode, row) = (show.clone(), episode.clone(), row.clone());
            focus.connect_enter(move |_| show(&episode, row.as_ref()));
        }
        item.add_controller(focus);
        if episode.current {
            current_row = Some(item.downgrade());
        }
        list.append(&item);
    }

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_propagate_natural_height(true);
    scroll.set_max_content_height(LIST_MAX);
    scroll.set_child(Some(&list));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.set_size_request(WIDTH, -1);
    body.append(&scroll);
    body.append(&footer);
    let popover = gtk::Popover::new();
    popover.add_css_class("ep-panel");
    popover.set_child(Some(&body));
    list.connect_row_activated({
        let popover = popover.clone();
        move |_, _| popover.popdown()
    });
    // Opened, the keyboard starts on the episode playing, so the footer says that one first.
    popover.connect_map(move |_| {
        if let Some(row) = current_row.as_ref().and_then(|row| row.upgrade()) {
            row.grab_focus();
        }
    });
    button.set_popover(Some(&popover));
}
