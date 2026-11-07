//! A series' page: its cover, titles and facts, the main action (read, continue, read again),
//! its place on the reading lists, and the chapters by volume, each with its state and a menu.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_core::books::WorkId;
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    reading::{
        ChapterDownloadState, ChapterReadState, ChapterRow, DownloadPick, LibrarySegmentRead,
        ReadingCommand, ReadingEvent, SeriesView, VolumeGroup,
    },
};

use super::{action_chapter, cover, dotted, library, listen, read_button, send, series_tag};
use crate::{
    states::{Shape, States},
    widgets,
    window::Ctx,
};

pub fn page(ctx: &Rc<Ctx>, work: &WorkId) -> adw::NavigationPage {
    let head = gtk::Box::new(gtk::Orientation::Vertical, 14);
    let chapters = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 22);
    column.append(&head);
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
    let menu_button = gtk::MenuButton::builder().icon_name("view-more-symbolic").build();
    header.pack_end(&menu_button);
    let page = widgets::page("", &header, &states.widget);
    page.set_tag(Some(&series_tag(work)));
    states.on_retry({
        let (hub, work) = (Rc::downgrade(&ctx.hub), work.clone());
        move || {
            if let Some(hub) = hub.upgrade() {
                send(&hub, ReadingCommand::LoadSeries { work: work.clone() });
            }
        }
    });
    let wanted = work.clone();
    listen(ctx, &page, {
        let (ctx, states, head, chapters, page, menu_button) = (
            Rc::downgrade(ctx),
            states.clone(),
            head.clone(),
            chapters.clone(),
            page.downgrade(),
            menu_button.clone(),
        );
        move |event| {
            let Some(ctx) = ctx.upgrade() else { return };
            match event {
                Event::Reading(ReadingEvent::Series(view)) if view.work == wanted => {
                    super::clear(&head);
                    draw_head(&ctx, &head, view);
                    menu_button.set_menu_model(Some(&status_menu(&view.work)));
                    if let Some(page) = page.upgrade() {
                        page.set_title(&view.titles.display.clone());
                    }
                    states.content();
                }
                Event::Reading(ReadingEvent::Chapters { work, volumes, failure, .. })
                    if *work == wanted =>
                {
                    super::clear(&chapters);
                    if let Some(failure) = failure {
                        chapters.append(&widgets::haze(&super::failure(failure), "caption"));
                    }
                    draw_chapters(&ctx, &chapters, work, volumes);
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
    page
}

fn states_plain() -> adw::StatusPage {
    crate::states::plain(&t::series_no_chapters_title(), &t::series_no_chapters_line())
}

fn status_menu(work: &WorkId) -> gio::Menu {
    let menu = gio::Menu::new();
    for segment in [
        LibrarySegmentRead::Reading,
        LibrarySegmentRead::Planning,
        LibrarySegmentRead::Completed,
        LibrarySegmentRead::Paused,
        LibrarySegmentRead::Dropped,
    ] {
        let item = gio::MenuItem::new(Some(&library::name(segment)), None);
        item.set_action_and_target_value(
            Some("win.read-status"),
            Some(&(work.0.clone(), segment.as_str().to_owned()).to_variant()),
        );
        menu.append_item(&item);
    }
    menu.append_item(&widgets::menu_item(&t::linux_send_menu(), "win.read-send", &work.0));
    menu.append_item(&widgets::menu_item(
        &t::series_remove_from_lists(),
        "win.read-remove",
        &work.0,
    ));
    menu
}

fn draw_head(ctx: &Rc<Ctx>, head: &gtk::Box, view: &SeriesView) {
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    top.append(&cover(&ctx.hub, view.cover.as_deref(), &view.titles.display.clone(), 160));
    let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    text.append(&widgets::show_title(&view.titles.display.clone(), "large"));
    let mut facts: Vec<String> = Vec::new();
    facts.extend(view.format.clone());
    facts.extend(view.status.clone());
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
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_margin_top(8);
    match action_chapter(&view.action) {
        Some((chapter, label)) => buttons.append(&read_button(&label, &view.work, &chapter)),
        None if matches!(view.action, kaeri_model::reading::SeriesAction::ReadAgain) => {
            let again = gtk::Button::with_label(&t::action_read_again());
            again.add_css_class("pill");
            again.set_action_name(Some("win.read-again"));
            again.set_action_target_value(Some(&view.work.0.to_variant()));
            buttons.append(&again);
        }
        None => {}
    }
    text.append(&buttons);
    top.append(&text);
    head.append(&top);
    if let Some(synopsis) = &view.synopsis {
        let body = gtk::Label::new(Some(synopsis));
        body.set_wrap(true);
        body.set_xalign(0.0);
        body.set_selectable(true);
        head.append(&body);
    }
    if !view.genres.is_empty() {
        head.append(&widgets::haze(&view.genres.join(" \u{b7} "), "caption"));
    }
}

fn state_line(chapter: &ChapterRow) -> String {
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
        Some(ChapterDownloadState::Downloading { done, total }) => {
            parts.push(t::chapter_state_downloading(i64::from(*done), i64::from(*total)));
        }
        Some(ChapterDownloadState::Failed { failure }) => {
            parts.push(t::chapter_state_failed(&super::failure(failure)));
        }
        None => {}
    }
    let line = dotted(&chapter.line);
    if !line.is_empty() {
        parts.push(line);
    }
    parts.join(" \u{b7} ")
}

fn draw_chapters(ctx: &Rc<Ctx>, column: &gtk::Box, work: &WorkId, volumes: &[VolumeGroup]) {
    for group in volumes {
        if let Some(volume) = &group.volume {
            column.append(&widgets::heading(&t::series_volume(volume)));
        } else if volumes.len() > 1 {
            column.append(&widgets::heading(&t::series_no_volume()));
        } else {
            column.append(&widgets::heading(&t::series_chapters()));
        }
        let list = widgets::boxed_list();
        for chapter in &group.chapters {
            let title = chapter.title.clone().filter(|t| !t.is_empty()).map_or_else(
                || chapter.label.clone(),
                |title| format!("{} \u{b7} {title}", chapter.label),
            );
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&title))
                .subtitle(glib::markup_escape_text(&state_line(chapter)))
                .activatable(true)
                .build();
            if matches!(chapter.state, ChapterReadState::Read) {
                row.add_css_class("dim-label");
                row.add_prefix(&gtk::Image::from_icon_name("object-select-symbolic"));
            }
            if chapter.new_chapter {
                row.add_suffix(&super::new_words());
            }
            row.set_action_name(Some("win.read"));
            row.set_action_target_value(Some(&super::read_target(work, Some(&chapter.id))));
            let more = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .valign(gtk::Align::Center)
                .build();
            more.add_css_class("flat");
            more.set_menu_model(Some(&chapter_menu(work, chapter)));
            row.add_suffix(&more);
            list.append(&row);
        }
        column.append(&list);
    }
    let _ = ctx;
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
    let download = gio::MenuItem::new(Some(&t::series_download()), None);
    download.set_action_and_target_value(
        Some("win.read-download"),
        Some(&(work.0.clone(), chapter.id.0.clone()).to_variant()),
    );
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
        "read-remove" => {
            if let Some(w) = arg.get::<String>() {
                send(hub, ReadingCommand::RemoveFromLists { work: work(&w) });
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
pub const ACTIONS: [(&str, &str); 7] = [
    ("read-status", "(ss)"),
    ("read-remove", "s"),
    ("read-again", "s"),
    ("read-mark", "(ssb)"),
    ("read-mark-up-to", "(ss)"),
    ("read-download", "(ss)"),
    ("read-bookmark", "(ssb)"),
];
