//! A series' page: its cover, titles and facts, the main action (read, continue, read again),
//! its place on the reading lists, and the chapters by volume, each with its state and a menu.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_core::{books::WorkId, media::Medium};
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    reading::{
        ChapterDownloadState, ChapterReadState, ChapterRow, DownloadPick, LibrarySegmentRead,
        ReadingCommand, ReadingEvent, SeriesView, VolumeGroup,
    },
};

use super::series_chapters::SeriesChapters;
use super::{
    action_chapter, cover, dotted, library, listen, read_button, row_line, send, series_extra,
    series_tag,
};
use crate::{
    states::{Shape, States},
    widgets,
    window::{Ctx, Size},
};

pub fn page(ctx: &Rc<Ctx>, work: &WorkId) -> adw::NavigationPage {
    let head = gtk::Box::new(gtk::Orientation::Vertical, 14);
    let chapters = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let details = gtk::Box::new(gtk::Orientation::Vertical, 0);
    // The rows' veil: after a group pick the old rows fade over the new ones.
    let details_veil = gtk::Overlay::new();
    details_veil.set_child(Some(&details));
    details_veil.set_widget_name(series_extra::DETAILS_VEIL);
    let memo: Rc<RefCell<series_extra::Memo>> = Rc::default();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 22);
    column.append(&head);
    column.append(&details_veil);
    column.append(&chapters);
    let scroll = widgets::column_page(&column, 820);
    let empty = states_plain();
    let states = States::new(
        &ctx.hub,
        &scroll,
        Shape::Rows { count: 4, width: 80, height: 120 },
        &empty,
        &t::error_generic(),
    );
    states.loading(None);
    let header = adw::HeaderBar::new();
    // The title is the page's own heading; the bar keeps it only for the back button.
    header.set_show_title(false);
    let menu_button = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(t::action_more())
        .menu_model(&more_menu(work))
        .build();
    header.pack_end(&menu_button);
    let page = widgets::page("", &header, &states.widget);
    page.set_tag(Some(&series_tag(work)));
    // Beside the main action: the list it is on, and what to download.
    let status_button = menu_pill(&t::series_add_to_list());
    let download_button = menu_pill(&t::series_download());
    download_button.set_menu_model(Some(&download_menu(work, &[])));
    let shown: Rc<RefCell<Option<SeriesView>>> = Rc::default();
    let actions = page_actions(ctx, work, &shown);
    // The chapter order, newest first as on Apple; the menu beside the heading flips it and
    // the list redraws from what the engine sent last.
    let newest = Rc::new(Cell::new(true));
    let listed: Rc<RefCell<Listed>> = Rc::default();
    // Drawn as the page scrolls to them, and changed in place when only some rows changed.
    let list = SeriesChapters::new(&chapters, &scroll, work);
    let redraw: Rc<dyn Fn()> = Rc::new({
        let (listed, newest, shown) =
            (Rc::clone(&listed), Rc::clone(&newest), Rc::clone(&shown));
        move || {
            let listed = listed.borrow();
            let Some((volumes, failure)) = listed.as_ref() else { return };
            // Volumes only for a novel whose rows sit in volumes; a novel listed by chapter
            // says Chapters, as its rows do.
            let by_volume = shown.borrow().as_ref().is_some_and(|v| v.medium == Medium::Novel)
                && volumes.iter().any(|group| group.volume.is_some());
            let order = Order { newest: newest.get(), by_volume };
            list.show(volumes, failure.as_deref(), order);
        }
    });
    let order = gio::SimpleAction::new_stateful(
        "order",
        Some(glib::VariantTy::STRING),
        &"newest".to_variant(),
    );
    order.connect_activate({
        let (newest, redraw) = (Rc::clone(&newest), Rc::clone(&redraw));
        move |action, value| {
            let Some(value) = value else { return };
            action.set_state(value);
            newest.set(value.str() == Some("newest"));
            redraw();
        }
    });
    actions.add_action(&order);
    page.insert_action_group("series", Some(&actions));
    states.on_retry({
        let (hub, work) = (Rc::downgrade(&ctx.hub), work.clone());
        move || {
            if let Some(hub) = hub.upgrade() {
                send(&hub, ReadingCommand::LoadSeries { work: work.clone() });
            }
        }
    });
    // A narrow window puts the cover over the words, so the main action has the width.
    let top: Rc<glib::WeakRef<gtk::Box>> = Rc::default();
    ctx.on_size({
        let top = Rc::clone(&top);
        move |size| {
            if let Some(top) = top.upgrade() {
                lay_top(&top, size);
            }
        }
    });
    let wanted = work.clone();
    listen(ctx, &page, {
        let (ctx, states, head, page) =
            (Rc::downgrade(ctx), states.clone(), head.clone(), page.downgrade());
        let (status_button, download_button, shown) =
            (status_button.clone(), download_button.clone(), Rc::clone(&shown));
        let (listed, redraw) = (Rc::clone(&listed), Rc::clone(&redraw));
        let (details, memo, work) = (details.clone(), Rc::clone(&memo), work.clone());
        let details_veil = details_veil.clone();
        let (menu_button, top) = (menu_button.clone(), Rc::clone(&top));
        move |event| {
            let Some(ctx) = ctx.upgrade() else { return };
            if series_extra::follow(&memo, &work, event) {
                let series = matches!(event, Event::Reading(ReadingEvent::Series(_)));
                series_extra::redraw(&ctx, &details_veil, &details, &memo.borrow(), series);
            }
            match event {
                Event::Reading(ReadingEvent::Series(view)) if view.work == wanted => {
                    // The demo's page is its own: credit, its one chapter and the other work.
                    if let Some(demo) = &view.demo {
                        super::clear(&head);
                        head.append(&super::demo::series_body(&ctx, view, demo));
                        for hidden in [&details, &chapters] {
                            hidden.set_visible(false);
                        }
                        menu_button.set_visible(false);
                        shown.replace(Some(SeriesView::clone(view)));
                        if let Some(page) = page.upgrade() {
                            page.set_title(&view.titles.display.clone());
                        }
                        states.content();
                        return;
                    }
                    for button in [&status_button, &download_button] {
                        if let Some(parent) = button.parent().and_downcast::<adw::WrapBox>() {
                            parent.remove(button);
                        }
                    }
                    super::clear(&head);
                    set_pill(
                        &status_button,
                        &view.list.map_or_else(t::series_add_to_list, library::name),
                    );
                    status_button.set_menu_model(Some(&status_menu(view)));
                    let drawn =
                        draw_head(&ctx, &head, view, &[&status_button, &download_button]);
                    top.set(Some(&drawn));
                    shown.replace(Some(SeriesView::clone(view)));
                    // The heading says Volumes for a novel, which the series view tells.
                    if listed.borrow().is_some() {
                        redraw();
                    }
                    if let Some(page) = page.upgrade() {
                        page.set_title(&view.titles.display.clone());
                    }
                    states.content();
                }
                Event::Reading(ReadingEvent::Chapters { work, volumes, failure, .. })
                    if *work == wanted =>
                {
                    download_button.set_menu_model(Some(&download_menu(work, volumes)));
                    listed
                        .replace(Some((volumes.clone(), failure.as_ref().map(super::failure))));
                    redraw();
                }
                Event::Reading(ReadingEvent::Failed {
                    about: kaeri_model::reading::ReadingAbout::Series,
                    failure,
                }) => {
                    states.failed(&super::failure(failure));
                }
                _ => {}
            }
        }
    });
    send(&ctx.hub, ReadingCommand::LoadSeries { work: work.clone() });
    send(&ctx.hub, ReadingCommand::LoadChapters { work: work.clone(), edition: None });
    series_extra::load(ctx, work);
    page
}

fn states_plain() -> adw::StatusPage {
    crate::states::plain(&t::series_no_chapters_title(), &t::series_no_chapters_line())
}

/// A secondary pill that opens a menu: its label and a chevron.
fn menu_pill(label: &str) -> gtk::MenuButton {
    let button =
        gtk::MenuButton::builder().always_show_arrow(true).valign(gtk::Align::Center).build();
    button.add_css_class("pill");
    set_pill(&button, label);
    button
}

fn set_pill(button: &gtk::MenuButton, label: &str) {
    button.set_label(label);
}

/// The lists, the score, and Remove from lists once it is on one.
fn status_menu(view: &SeriesView) -> gio::Menu {
    let work = &view.work;
    let lists = gio::Menu::new();
    for segment in [
        LibrarySegmentRead::Reading,
        LibrarySegmentRead::Planning,
        LibrarySegmentRead::Completed,
        LibrarySegmentRead::Paused,
        LibrarySegmentRead::Dropped,
        LibrarySegmentRead::Rereading,
    ] {
        let item = gio::MenuItem::new(Some(&library::name(segment)), None);
        item.set_action_and_target_value(
            Some("win.read-status"),
            Some(&(work.0.clone(), segment.as_str().to_owned()).to_variant()),
        );
        if view.list == Some(segment) {
            item.set_icon(&gio::ThemedIcon::new("object-select-symbolic"));
        }
        lists.append_item(&item);
    }
    let scores = gio::Menu::new();
    for score in (1..=10).rev() {
        let mut label = score.to_string();
        if view.score.is_some_and(|s| s.round() as i32 == score) {
            label = format!("{label} \u{2713}"); // i18n-skip
        }
        let item = gio::MenuItem::new(Some(&label), None);
        item.set_action_and_target_value(
            Some("win.read-score"),
            Some(&(work.0.clone(), f64::from(score)).to_variant()),
        );
        scores.append_item(&item);
    }
    let menu = gio::Menu::new();
    menu.append_section(None, &lists);
    let score = gio::Menu::new();
    score.append_submenu(Some(&t::series_score()), &scores);
    menu.append_section(None, &score);
    if view.list.is_some() {
        let remove = gio::Menu::new();
        remove.append(Some(&t::series_remove_from_lists()), Some("series.remove"));
        menu.append_section(None, &remove);
    }
    menu
}

/// The volumes Download offers. A flat list, drawn with no volume headings, still says each
/// chapter's volume: its rows by volume, in reading order, and none for rows without one.
fn download_volumes(volumes: &[VolumeGroup]) -> Vec<VolumeGroup> {
    let [list] = volumes else { return volumes.to_vec() };
    if !list.flat {
        return volumes.to_vec();
    }
    let mut groups: Vec<VolumeGroup> = Vec::new();
    for row in &list.chapters {
        let Some(volume) = row.volume.as_ref().filter(|v| !v.is_empty()) else { continue };
        match groups.iter_mut().find(|g| g.volume.as_ref() == Some(volume)) {
            Some(group) => group.chapters.push(row.clone()),
            None => groups.push(VolumeGroup {
                volume: Some(volume.clone()),
                title: None,
                chapters: vec![row.clone()],
                all_read: false,
                flat: false,
            }),
        }
    }
    groups
}

/// The next 5 or 10 unread, every unread chapter, a volume, and deleting the downloads.
fn download_menu(work: &WorkId, volumes: &[VolumeGroup]) -> gio::Menu {
    let pick = |name: &str| Some((work.0.clone(), name.to_owned()).to_variant());
    let first = gio::Menu::new();
    for (label, name) in [
        (t::download_next(5), "next:5"),
        (t::download_next(10), "next:10"),
        (t::download_unread(), "unread"),
    ] {
        let item = gio::MenuItem::new(Some(&label), None);
        item.set_action_and_target_value(Some("win.read-download-pick"), pick(name).as_ref());
        first.append_item(&item);
    }
    let menu = gio::Menu::new();
    menu.append_section(None, &first);
    let by_volume = gio::Menu::new();
    for group in &download_volumes(volumes) {
        let Some(volume) = &group.volume else { continue };
        let missing = group.chapters.iter().filter(|c| c.download.is_none()).count();
        if missing == 0 {
            continue;
        }
        let label = t::download_volume_count(&t::series_volume(volume), missing as i64);
        let item = gio::MenuItem::new(Some(&label), None);
        item.set_action_and_target_value(
            Some("win.read-download-pick"),
            pick(&format!("volume:{volume}")).as_ref(), // i18n-skip
        );
        by_volume.append_item(&item);
    }
    if by_volume.n_items() > 0 {
        menu.append_section(None, &by_volume);
    }
    let downloaded: Vec<String> = volumes
        .iter()
        .flat_map(|g| &g.chapters)
        .filter(|c| c.download.is_some())
        .map(|c| c.id.0.clone())
        .collect();
    if !downloaded.is_empty() {
        let delete = gio::Menu::new();
        let item = gio::MenuItem::new(Some(&t::download_delete_series()), None);
        item.set_action_and_target_value(
            Some("win.read-download-delete"),
            Some(&(work.0.clone(), downloaded).to_variant()),
        );
        delete.append_item(&item);
        menu.append_section(None, &delete);
    }
    menu
}

/// The header's More: Send to e-reader, the source in use, and the trackers.
fn more_menu(work: &WorkId) -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_item(&widgets::menu_item(&t::linux_send_menu(), "win.read-send", &work.0));
    menu.append(Some(&t::series_read_from_menu()), Some("series.read-from"));
    menu.append(Some(&t::series_tracking()), Some("series.trackers"));
    menu
}

/// The page's own actions, which need the window: Read from, the trackers, and Remove from
/// lists, asked first.
fn page_actions(
    ctx: &Rc<Ctx>,
    work: &WorkId,
    shown: &Rc<RefCell<Option<SeriesView>>>,
) -> gio::SimpleActionGroup {
    let title = |shown: &RefCell<Option<SeriesView>>| {
        shown.borrow().as_ref().map(|v| v.titles.display.clone()).unwrap_or_default()
    };
    let group = gio::SimpleActionGroup::new();
    let read_from = gio::SimpleAction::new("read-from", None);
    read_from.connect_activate({
        let (ctx, work, shown) = (Rc::downgrade(ctx), work.clone(), Rc::clone(shown));
        move |_, _| {
            if let Some(ctx) = ctx.upgrade() {
                series_extra::read_from(&ctx, &work, &title(&shown));
            }
        }
    });
    let trackers = gio::SimpleAction::new("trackers", None);
    trackers.connect_activate({
        let (ctx, work, shown) = (Rc::downgrade(ctx), work.clone(), Rc::clone(shown));
        move |_, _| {
            if let Some(ctx) = ctx.upgrade() {
                series_extra::trackers(&ctx, &work, shown.borrow().clone());
            }
        }
    });
    let remove = gio::SimpleAction::new("remove", None);
    remove.connect_activate({
        let (ctx, work, shown) = (Rc::downgrade(ctx), work.clone(), Rc::clone(shown));
        move |_, _| {
            let Some(ctx) = ctx.upgrade() else { return };
            let dialog = adw::AlertDialog::new(
                Some(&t::series_remove_from_lists_title(&title(&shown))),
                Some(&t::series_remove_from_lists_line()),
            );
            dialog.add_response("cancel", &t::action_cancel());
            dialog.add_response("remove", &t::action_remove());
            dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            let (hub, work) = (Rc::downgrade(&ctx.hub), work.clone());
            dialog.connect_response(Some("remove"), move |_, _| {
                if let Some(hub) = hub.upgrade() {
                    send(&hub, ReadingCommand::RemoveFromLists { work: work.clone() });
                }
            });
            dialog.present(Some(&ctx.window));
        }
    });
    group.add_action(&read_from);
    group.add_action(&trackers);
    group.add_action(&remove);
    group
}

/// The cover beside the words, or over them in a narrow window.
fn lay_top(top: &gtk::Box, size: Size) {
    let narrow = size == Size::Narrow;
    top.set_orientation(if narrow {
        gtk::Orientation::Vertical
    } else {
        gtk::Orientation::Horizontal
    });
    top.set_spacing(if narrow { 14 } else { 20 });
}

/// Draw the page's head, and give back the row of the cover and the words.
fn draw_head(
    ctx: &Rc<Ctx>,
    head: &gtk::Box,
    view: &SeriesView,
    menus: &[&gtk::MenuButton],
) -> gtk::Box {
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    lay_top(&top, ctx.size());
    top.append(&cover(&ctx.hub, view.cover.as_deref(), &view.titles.display.clone(), 160));
    let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    text.append(&widgets::show_title(&view.titles.display, "page"));
    // The kind, the status and the count are the engine's facts line; the year and the author
    // come before it.
    let mut facts: Vec<String> = Vec::new();
    facts.extend(view.year.map(|y| y.to_string()));
    facts.extend(view.authors.first().cloned());
    if !facts.is_empty() {
        text.append(&widgets::haze(&facts.join(" \u{b7} "), "caption"));
    }
    let line = dotted(&view.facts);
    if !line.is_empty() {
        text.append(&widgets::haze(&line, "caption"));
    }
    if let Some(rhythm) = &view.rhythm {
        text.append(&widgets::haze(&crate::say::msg(rhythm), "caption"));
    }
    // The main action and its menus go one under the other when a narrow window or large text
    // leaves no room for them side by side.
    let buttons = adw::WrapBox::builder().child_spacing(8).line_spacing(8).build();
    buttons.set_margin_top(8);
    match action_chapter(&view.action) {
        Some((chapter, label)) => buttons.append(&read_button(&label, &view.work, &chapter)),
        None if matches!(view.action, kaeri_model::reading::SeriesAction::ReadAgain) => {
            let again = gtk::Button::with_label(&t::action_read_again());
            again.add_css_class("pill");
            again.set_action_target_value(Some(&view.work.0.to_variant()));
            again.set_action_name(Some("win.read-again"));
            buttons.append(&again);
        }
        None => {}
    }
    for menu in menus {
        buttons.append(*menu);
    }
    text.append(&buttons);
    top.append(&text);
    head.append(&top);
    if let Some(synopsis) = &view.synopsis {
        head.append(&synopsis_more(synopsis));
    }
    if !view.genres.is_empty() {
        head.append(&widgets::haze(&view.genres.join(" \u{b7} "), "caption"));
    }
    top
}

/// The synopsis in four lines, then "More" under it, as on a show's page, only where the text is
/// cut. More opens the rest in place: the place holds its height while the label lets go of its
/// lines, then eases to the new height on the standard spring, so what is under it moves with
/// it. The label's text is whole for a screen reader either way, and focus moves to the text.
fn synopsis_more(synopsis: &str) -> gtk::Widget {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let body = gtk::Label::new(Some(synopsis));
    body.set_wrap(true);
    body.set_xalign(0.0);
    body.set_selectable(true);
    body.set_lines(SYNOPSIS_LINES);
    body.set_ellipsize(gtk::pango::EllipsizeMode::End);
    column.append(&body);
    let more = gtk::Button::with_label(&t::action_more());
    // The page's menu is More too: this one says what it does.
    more.update_property(&[gtk::accessible::Property::Label(&t::title_synopsis_more())]);
    more.add_css_class("flat");
    more.set_halign(gtk::Align::Start);
    more.set_visible(false);
    column.append(&more);
    let place = crate::squeeze::Squeeze::new(&column, gtk::Orientation::Vertical, 0);
    more.connect_clicked({
        let (body, place) = (body.clone(), place.downgrade());
        move |more| {
            let Some(place) = place.upgrade() else { return };
            place.hold();
            body.set_lines(-1);
            body.set_ellipsize(gtk::pango::EllipsizeMode::None);
            more.set_visible(false);
            place.release();
            // Focus stays on the text, without selecting it, now More has gone.
            body.grab_focus();
            body.select_region(0, 0);
        }
    });
    body.add_tick_callback({
        let more = more.clone();
        move |label, _| {
            if label.width() == 0 {
                return glib::ControlFlow::Continue;
            }
            more.set_visible(label.layout().is_ellipsized());
            glib::ControlFlow::Break
        }
    });
    place.upcast()
}

/// How many lines of the synopsis show before More.
const SYNOPSIS_LINES: i32 = 4;

pub(super) fn state_line(chapter: &ChapterRow) -> String {
    // The engine's words say it all: the state, when, and what is downloaded.
    let said = row_line(chapter);
    if !said.is_empty() {
        return said;
    }
    let mut parts: Vec<String> = Vec::new();
    match &chapter.state {
        ChapterReadState::Unread => {}
        ChapterReadState::InProgress { page, of: Some(of) } => {
            parts.push(t::chapter_state_page_of(i64::from(*page), i64::from(*of)));
        }
        ChapterReadState::InProgress { page, of: None } => {
            parts.push(t::chapter_state_page(i64::from(*page)));
        }
        ChapterReadState::Read => parts.push(t::chapter_state_read()),
    }
    match &chapter.download {
        Some(ChapterDownloadState::Done) => parts.push(t::chapter_state_downloaded()),
        Some(ChapterDownloadState::Queued) => parts.push(t::chapter_state_queued()),
        Some(ChapterDownloadState::WaitsForWifi) => parts.push(t::chapter_state_wifi()),
        // No page count yet, or a novel's chapter, which has none: nothing to count.
        Some(ChapterDownloadState::Downloading { total: 0, .. }) => {
            parts.push(t::chapter_state_downloading_plain());
        }
        Some(ChapterDownloadState::Downloading { done, total }) => {
            parts.push(t::chapter_state_downloading(i64::from(*done), i64::from(*total)));
        }
        Some(ChapterDownloadState::Failed { failure }) => {
            parts.push(t::chapter_state_failed(&super::failure(failure)));
        }
        None => {}
    }
    let line = row_line(chapter);
    if !line.is_empty() {
        parts.push(line);
    }
    parts.join(" \u{b7} ")
}

/// The volumes the engine sent last, with what failed, kept to redraw in the other order.
type Listed = Option<(Vec<VolumeGroup>, Option<String>)>;

/// How the chapter list reads: its order, and whether its heading says Volumes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Order {
    pub newest: bool,
    pub by_volume: bool,
}

/// The heading with the order menu at its end, as on the board.
pub(super) fn chapters_head(order: Order) -> gtk::Box {
    let heading = widgets::heading(&if order.by_volume {
        t::series_volumes()
    } else {
        t::series_chapters()
    });
    heading.set_hexpand(true);
    let shown = if order.newest { t::series_newest_first() } else { t::series_oldest_first() };
    let label = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    label.append(&gtk::Label::new(Some(&shown)));
    label.append(&gtk::Image::from_icon_name("pan-down-symbolic"));
    let menu = gio::Menu::new();
    for (name, value) in
        [(t::series_newest_first(), "newest"), (t::series_oldest_first(), "oldest")]
    {
        menu.append_item(&widgets::menu_item(&name, "series.order", value));
    }
    let button = gtk::MenuButton::builder().child(&label).menu_model(&menu).build();
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button.set_tooltip_text(Some(&t::series_order()));
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}, {shown}", // i18n-skip
        t::series_order()
    ))]);
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    head.append(&heading);
    head.append(&button);
    head
}

/// A chapter's row, 52 px: the number in silk and its title in haze, the state under them. A
/// read chapter has both lines in haze; a new one says so in silk after the water dot. A
/// download in progress shows its ring, a failed one Try again.
pub(super) fn chapter_row(work: &WorkId, chapter: &ChapterRow) -> gtk::ListBoxRow {
    let read = matches!(chapter.state, ChapterReadState::Read);
    let first = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let number = gtk::Label::new(Some(&chapter.label));
    number.set_xalign(0.0);
    if read {
        number.add_css_class("haze");
    }
    first.append(&number);
    if let Some(title) = chapter.title.as_deref().filter(|t| !t.is_empty()) {
        let title = widgets::haze(title, "body");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        first.append(&title);
    }
    if chapter.bookmarked {
        let mark = gtk::Image::from_icon_name("user-bookmarks-symbolic");
        mark.add_css_class("haze");
        mark.set_pixel_size(12);
        mark.update_property(&[gtk::accessible::Property::Label(&t::series_bookmarked())]);
        first.append(&mark);
    }
    let second = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    // A chapter from another group or source says so in the engine's words; the row never
    // names the group in use.
    let words = state_line(chapter);
    if chapter.new_chapter && !read {
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("home-new");
        dot.set_valign(gtk::Align::Center);
        second.append(&dot);
        let line = gtk::Label::new(Some(&words));
        line.add_css_class("caption");
        line.set_xalign(0.0);
        second.append(&line);
    } else if !words.is_empty() {
        let line = widgets::haze(&words, "caption");
        line.set_ellipsize(gtk::pango::EllipsizeMode::End);
        second.append(&line);
    }
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    text.append(&first);
    if second.first_child().is_some() {
        text.append(&second);
    }
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    line.set_size_request(-1, 52);
    line.set_margin_start(12);
    line.set_margin_end(6);
    line.append(&text);
    let target = (work.0.clone(), chapter.id.0.clone()).to_variant();
    match &chapter.download {
        Some(ChapterDownloadState::Downloading { done, total }) => {
            line.append(&widgets::ring(*done as f64 / f64::from((*total).max(1))));
        }
        Some(ChapterDownloadState::Queued | ChapterDownloadState::WaitsForWifi) => {
            line.append(&widgets::ring(0.0));
        }
        Some(ChapterDownloadState::Failed { .. }) => {
            let again = gtk::Button::from_icon_name("view-refresh-symbolic");
            again.add_css_class("flat");
            again.set_valign(gtk::Align::Center);
            again.set_tooltip_text(Some(&t::action_try_again()));
            again.set_action_target_value(Some(&target));
            again.set_action_name(Some("win.read-download"));
            line.append(&again);
        }
        _ => {}
    }
    let row = gtk::ListBoxRow::builder().child(&line).activatable(true).build();
    // Right-click, a long press or the Menu key open the chapter's menu; the row has no button.
    widgets::context_menu(&row, &chapter_menu(work, chapter));
    row.set_action_target_value(Some(&super::read_target(work, Some(&chapter.id))));
    row.set_action_name(Some("win.read"));
    row
}

fn chapter_menu(work: &WorkId, chapter: &ChapterRow) -> gio::Menu {
    let menu = gio::Menu::new();
    let read = matches!(chapter.state, ChapterReadState::Read);
    let target = |on: bool| (work.0.clone(), chapter.id.0.clone(), on).to_variant();
    let item = gio::MenuItem::new(
        Some(&if read { t::series_mark_unread() } else { t::series_mark_read() }),
        None,
    );
    item.set_action_and_target_value(Some("win.read-mark"), Some(&target(!read)));
    menu.append_item(&item);
    let up_to = gio::MenuItem::new(Some(&t::series_mark_read_up_to()), None);
    up_to.set_action_and_target_value(
        Some("win.read-mark-up-to"),
        Some(&(work.0.clone(), chapter.id.0.clone()).to_variant()),
    );
    menu.append_item(&up_to);
    let pair = (work.0.clone(), chapter.id.0.clone()).to_variant();
    let (label, action) = match &chapter.download {
        None | Some(ChapterDownloadState::Failed { .. }) => {
            (t::series_download(), "win.read-download")
        }
        Some(ChapterDownloadState::Done) => {
            (t::downloads_delete_one_title(), "win.read-download-delete-one")
        }
        Some(_) => (t::downloads_remove_from_queue(), "win.read-download-cancel"),
    };
    let download = gio::MenuItem::new(Some(&label), None);
    download.set_action_and_target_value(Some(action), Some(&pair));
    menu.append_item(&download);
    let bookmark = gio::MenuItem::new(
        Some(&if chapter.bookmarked {
            t::series_remove_bookmark()
        } else {
            t::series_bookmark()
        }),
        None,
    );
    bookmark.set_action_and_target_value(
        Some("win.read-bookmark"),
        Some(&(work.0.clone(), chapter.id.0.clone(), !chapter.bookmarked).to_variant()),
    );
    menu.append_item(&bookmark);
    menu
}

/// The actions a series page and its rows ask for.
pub fn command(name: &str, hub: &crate::hub::Hub, arg: &glib::Variant) {
    use kaeri_core::books::ChapterId;
    let work = |s: &str| WorkId(s.to_owned());
    match name {
        "read-status" => {
            if let Some((w, s)) = arg.get::<(String, String)>()
                && let Some(status) = LibrarySegmentRead::parse(&s)
            {
                send(hub, ReadingCommand::SetStatus { work: work(&w), status });
            }
        }
        "read-again" => {
            if let Some(w) = arg.get::<String>() {
                send(hub, ReadingCommand::ReadAgain { work: work(&w) });
            }
        }
        "read-mark" => {
            if let Some((w, c, on)) = arg.get::<(String, String, bool)>() {
                send(
                    hub,
                    ReadingCommand::MarkRead {
                        work: work(&w),
                        chapters: vec![ChapterId(c)],
                        read: on,
                    },
                );
            }
        }
        "read-mark-up-to" => {
            if let Some((w, c)) = arg.get::<(String, String)>() {
                send(
                    hub,
                    ReadingCommand::MarkReadUpTo { work: work(&w), chapter: ChapterId(c) },
                );
            }
        }
        "read-download" => {
            if let Some((w, c)) = arg.get::<(String, String)>() {
                send(
                    hub,
                    ReadingCommand::Download {
                        work: work(&w),
                        pick: DownloadPick::Chapters { ids: vec![ChapterId(c)] },
                    },
                );
            }
        }
        "read-score" => {
            if let Some((w, score)) = arg.get::<(String, f64)>() {
                send(hub, ReadingCommand::SetScore { work: work(&w), score: score as f32 });
            }
        }
        "read-download-pick" => {
            if let Some((w, name)) = arg.get::<(String, String)>() {
                let pick = match name.split_once(':') {
                    Some(("next", n)) => DownloadPick::NextUnread { n: n.parse().unwrap_or(5) },
                    Some(("volume", v)) => DownloadPick::Volume { volume: v.to_owned() },
                    _ => DownloadPick::Unread,
                };
                send(hub, ReadingCommand::Download { work: work(&w), pick });
            }
        }
        "read-download-cancel" => {
            if let Some((w, c)) = arg.get::<(String, String)>() {
                send(
                    hub,
                    ReadingCommand::DownloadCancel {
                        work: work(&w),
                        chapters: vec![ChapterId(c)],
                    },
                );
            }
        }
        "read-download-delete-one" => {
            if let Some((w, c)) = arg.get::<(String, String)>() {
                send(
                    hub,
                    ReadingCommand::DownloadDelete {
                        work: work(&w),
                        chapters: vec![ChapterId(c)],
                    },
                );
            }
        }
        "read-download-delete" => {
            if let Some((w, chapters)) = arg.get::<(String, Vec<String>)>() {
                let chapters = chapters.into_iter().map(ChapterId).collect();
                send(hub, ReadingCommand::DownloadDelete { work: work(&w), chapters });
            }
        }
        "read-bookmark" => {
            if let Some((w, c, on)) = arg.get::<(String, String, bool)>() {
                send(
                    hub,
                    ReadingCommand::Bookmark { work: work(&w), chapter: ChapterId(c), on },
                );
            }
        }
        _ => {}
    }
}

/// The names of the actions [`command`] answers.
pub const ACTIONS: [(&str, &str); 11] = [
    ("read-status", "(ss)"),
    ("read-again", "s"),
    ("read-mark", "(ssb)"),
    ("read-mark-up-to", "(ss)"),
    ("read-download", "(ss)"),
    ("read-bookmark", "(ssb)"),
    ("read-score", "(sd)"),
    ("read-download-pick", "(ss)"),
    ("read-download-cancel", "(ss)"),
    ("read-download-delete-one", "(ss)"),
    ("read-download-delete", "(sas)"),
];

/// Open Read from for the shot, once the page has its head.
#[cfg(feature = "fixtures")]
pub fn shot_source_picker(shell: &Rc<crate::window::Shell>) {
    let ctx = Rc::clone(&shell.ctx);
    glib::timeout_add_local_once(std::time::Duration::from_millis(2500), move || {
        series_extra::read_from(
            &ctx,
            &WorkId("a:900301".to_owned()),
            "The Lantern Ferry", // i18n-skip
        );
    });
}

#[cfg(test)]
mod tests {
    use adw::prelude::*;
    use kaeri_core::books::{ChapterId, WorkId};

    use super::{Size, lay_top, menu_pill, read_button};
    use crate::gtk_test::{pump, run};

    /// The head's row in the smallest window, 350 px inside, whose column leaves 326 px: the
    /// cover goes over the words, and the main action and its menus wrap, at twice the text
    /// size too.
    #[test]
    fn the_head_fits_a_narrow_window_at_any_text_size() {
        run(|| {
            let min_width =
                |widget: &gtk::Widget| widget.measure(gtk::Orientation::Horizontal, -1).0;
            let top = gtk::Box::new(gtk::Orientation::Horizontal, 20);
            let cover = gtk::Box::new(gtk::Orientation::Vertical, 0);
            cover.set_size_request(160, 240);
            top.append(&cover);
            let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let buttons = adw::WrapBox::builder().child_spacing(8).line_spacing(8).build();
            let (work, chapter) = (WorkId("a:1".into()), ChapterId("13".into()));
            buttons.append(&read_button("Continue chapter 13 \u{b7} page 10", &work, &chapter));
            buttons.append(&menu_pill("Add to list"));
            buttons.append(&menu_pill("Download"));
            text.append(&buttons);
            top.append(&text);
            let window = gtk::Window::builder().default_width(1280).default_height(800).build();
            window.set_child(Some(&top));
            window.present();

            lay_top(&top, Size::Wide);
            pump(100);
            assert_eq!(top.orientation(), gtk::Orientation::Horizontal);
            lay_top(&top, Size::Narrow);
            pump(100);
            let normal = min_width(top.upcast_ref());
            let settings = gtk::Settings::default().expect("a display");
            let dpi = settings.gtk_xft_dpi();
            settings.set_gtk_xft_dpi(2 * 96 * 1024);
            pump(150);
            let large = min_width(top.upcast_ref());
            settings.set_gtk_xft_dpi(dpi);
            window.destroy();

            assert_eq!(top.orientation(), gtk::Orientation::Vertical);
            assert!(normal <= 326, "the head needs {normal} px at the normal text size");
            assert!(large <= 326, "the head needs {large} px at twice the size");
        });
    }

    /// A flat list offers the volumes its rows say, in reading order, leaving out rows with none.
    #[test]
    fn a_flat_list_downloads_by_the_volumes_its_rows_say() {
        use kaeri_model::reading::{ChapterReadState, ChapterRow, VolumeGroup};
        let row = |n: u32, volume: Option<&str>| ChapterRow {
            id: ChapterId(n.to_string()),
            label: format!("Chapter {n}"), // i18n-skip
            state: ChapterReadState::Unread,
            number: Some(n.to_string()),
            title: None,
            volume: volume.map(str::to_owned),
            published: None,
            pages: None,
            new_chapter: false,
            download: None,
            versions: 1,
            bookmarked: false,
            version_label: None,
            read_at: None,
            line: Vec::new(),
            gone_from_source: false,
            parts: 1,
            missing_parts: Vec::new(),
            part_groups: Vec::new(),
            other_group: false,
            group_language: None,
            other_source: None,
        };
        let flat = VolumeGroup {
            volume: None,
            title: None,
            chapters: vec![
                row(1, Some("1")),
                row(2, Some("1")),
                row(3, Some("2")),
                row(4, None),
            ],
            all_read: false,
            flat: true,
        };
        let groups = super::download_volumes(std::slice::from_ref(&flat));
        let shape: Vec<_> =
            groups.iter().map(|g| (g.volume.clone(), g.chapters.len(), g.flat)).collect();
        assert_eq!(shape, [(Some("1".into()), 2, false), (Some("2".into()), 1, false)]);
        let grouped = VolumeGroup { flat: false, volume: Some("1".into()), ..flat };
        assert_eq!(super::download_volumes(std::slice::from_ref(&grouped)).len(), 1);
    }
}
