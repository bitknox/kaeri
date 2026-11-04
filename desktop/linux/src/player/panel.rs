//! The player's Episodes panel: compact rows, and under them a footer with the title and the
//! synopsis of the row under the pointer or the keyboard focus, else the episode playing.
//!
//! The footer is reserved only when the table has a synopsis as the panel is built, so no text
//! arriving later moves the list. A row spoiler protection holds back shows its title and facts.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_model::{
    episodes::{EpisodeRow, EpisodeText, episode_ranges, episode_text},
    hosted::MenuEpisode,
};

use crate::{motion, synopsis, widgets};

/// A switch of the panel to a range, by its index.
type Switch = Rc<dyn Fn(usize)>;

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

    // A long series splits into the title page's ranges: the panel shows one at a time, opening on
    // the range of the episode playing, and a picker in its heading row switches.
    let now = glib::DateTime::now_utc().map_or(0, |t| t.to_unix());
    let ranges = episode_ranges(table, now);
    let range_of = |number: &str| {
        let at = table.iter().position(|row| row.number == number)? as u32;
        ranges.iter().position(|r| (r.start..r.end).contains(&at))
    };
    let shown =
        Rc::new(Cell::new(playing.as_ref().and_then(|p| range_of(&p.number)).unwrap_or(0)));
    let owners: Rc<Vec<usize>> =
        Rc::new(rows.iter().map(|(e, _)| range_of(&e.number).unwrap_or(0)).collect());

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
    // The row to focus as the panel opens: the one playing, or the one Go to episode went to.
    let target: Rc<RefCell<Option<glib::WeakRef<gtk::ListBoxRow>>>> = Rc::default();
    let switch: Rc<RefCell<Option<Switch>>> = Rc::default();
    if ranges.len() > 1 {
        list.set_filter_func({
            let (shown, owners) = (shown.clone(), owners.clone());
            move |row| owners.get(row.index() as usize).is_some_and(|at| *at == shown.get())
        });
        // Go to episode, the range menu's last item: the dialog, then the panel again on the
        // range that holds the row, the keyboard on it.
        let go: Rc<dyn Fn()> = Rc::new({
            let (button, list, table, owners, episodes) = (
                button.clone(),
                list.clone(),
                table.to_vec(),
                owners.clone(),
                episodes.to_vec(),
            );
            let (target, switch) = (target.clone(), switch.clone());
            move || {
                button.popdown();
                let (button, list, owners, episodes) =
                    (button.clone(), list.clone(), owners.clone(), episodes.clone());
                let (target, switch) = (target.clone(), switch.clone());
                let parent = button
                    .root()
                    .map_or_else(|| button.clone().upcast(), |r| r.upcast::<gtk::Widget>());
                crate::pages::title::go_to_dialog(&parent, &table, move |typed| {
                    let number = typed.trim_start_matches('0');
                    let Some(at) = episodes.iter().position(|e| e.number == number) else {
                        return;
                    };
                    if let Some(to) = owners.get(at)
                        && let Some(switch) = switch.borrow().as_ref()
                    {
                        switch(*to);
                    }
                    target.replace(list.row_at_index(at as i32).map(|row| row.downgrade()));
                    button.popup();
                });
            }
        });
        let (heading, to_range) = range_heading(&list, &scroll, table, &ranges, &shown, go);
        switch.replace(Some(to_range));
        body.append(&heading);
    }
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
        let went = target.take().and_then(|row| row.upgrade());
        if let Some(row) = went.or_else(|| current_row.as_ref().and_then(|row| row.upgrade())) {
            row.grab_focus();
        }
    });
    button.set_popover(Some(&popover));
}

/// The heading row of a long series' panel: a flat button naming the range shown, whose menu has
/// one item per range and ends with Go to episode. The list crossfades into the range chosen,
/// with both drawn. Also returns the switch to a range, for Go to episode.
fn range_heading(
    list: &gtk::ListBox,
    scroll: &gtk::ScrolledWindow,
    table: &[EpisodeRow],
    ranges: &[kaeri_model::episodes::EpisodeRange],
    shown: &Rc<Cell<usize>>,
    go: Rc<dyn Fn()>,
) -> (gtk::Box, Switch) {
    let items = synopsis::range_items(table, ranges);
    let button = gtk::MenuButton::new();
    button.add_css_class("flat");
    button.set_halign(gtk::Align::Start);
    button.set_tooltip_text(Some(&kaeri_i18n::t::episode_range_label()));
    button.update_property(&[gtk::accessible::Property::Label(
        &kaeri_i18n::t::episode_range_label(),
    )]);
    button.set_label(&items[shown.get().min(items.len() - 1)].label);
    let menu = gtk::ListBox::new();
    menu.set_selection_mode(gtk::SelectionMode::Single);
    menu.add_css_class("navigation-sidebar");
    for item in &items {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let label = gtk::Label::new(Some(&item.label));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        line.append(&label);
        if let Some(state) = &item.state {
            line.append(&widgets::haze(state, "caption"));
        }
        menu.append(&line);
    }
    let rule = gtk::ListBoxRow::builder()
        .child(&gtk::Separator::new(gtk::Orientation::Horizontal))
        .activatable(false)
        .selectable(false)
        .build();
    menu.append(&rule);
    let go_line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let go_label = gtk::Label::new(Some(&kaeri_i18n::t::episode_go_to()));
    go_label.set_xalign(0.0);
    go_label.set_hexpand(true);
    go_line.append(&go_label);
    go_line.append(&widgets::haze("Ctrl+G", "caption")); // i18n-skip
    menu.append(&go_line);
    menu.select_row(menu.row_at_index(shown.get() as i32).as_ref());
    let pop = gtk::Popover::new();
    let menu_scroll = gtk::ScrolledWindow::new();
    menu_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    menu_scroll.set_propagate_natural_height(true);
    menu_scroll.set_max_content_height(LIST_MAX);
    menu_scroll.set_child(Some(&menu));
    pop.set_child(Some(&menu_scroll));
    button.set_popover(Some(&pop));
    let count = items.len();
    let switch: Switch = Rc::new({
        let (list, scroll, shown, button, menu) =
            (list.clone(), scroll.clone(), shown.clone(), button.clone(), menu.clone());
        move |to: usize| {
            if to == shown.get() || to >= items.len() {
                return;
            }
            shown.set(to);
            button.set_label(&items[to].label);
            menu.select_row(menu.row_at_index(to as i32).as_ref());
            list.invalidate_filter();
            scroll.vadjustment().set_value(0.0);
            // The new range arrives over the old one's last frame: never empty between them.
            let fade =
                adw::TimedAnimation::new(&scroll, 0.4, 1.0, 200, motion::opacity(&scroll));
            fade.set_easing(adw::Easing::EaseOutCubic);
            fade.play();
        }
    });
    menu.connect_row_activated({
        let (pop, switch) = (pop.clone(), switch.clone());
        move |_, row| {
            pop.popdown();
            let to = row.index() as usize;
            if to > count {
                go();
            } else {
                switch(to);
            }
        }
    });
    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    heading.add_css_class("ep-range-heading");
    heading.set_margin_start(4);
    heading.set_margin_top(4);
    heading.append(&button);
    (heading, switch)
}
