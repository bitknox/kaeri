//! Updates: new chapters from the series being followed, by day, each row a chapter that opens
//! in the reader. Check now and Download new in the header bar.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    reading::{ReadingCommand, ReadingEvent, UpdatesView},
};

use super::{Place, day_label, listen, place, send};
use crate::{say, states::Shape, widgets, window::Ctx};

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let empty =
        crate::states::plain(&t::updates_empty_title(), &t::chapter_updates_empty_line());
    let Place { page, header, states, column } = place(
        ctx,
        &t::nav_updates(),
        Shape::Rows { count: 5, width: 48, height: 72 },
        &empty,
        &t::error_generic(),
        false,
    );
    let download = gtk::Button::with_label(&t::updates_download_new());
    download.connect_clicked({
        let hub = Rc::downgrade(&ctx.hub);
        move |_| {
            if let Some(hub) = hub.upgrade() {
                send(&hub, ReadingCommand::DownloadNew);
            }
        }
    });
    download.set_sensitive(false);
    let check = gtk::Button::from_icon_name("view-refresh-symbolic");
    check.set_tooltip_text(Some(&t::linux_updates_check()));
    check.update_property(&[gtk::accessible::Property::Label(&t::linux_updates_check())]);
    check.connect_clicked({
        let hub = Rc::downgrade(&ctx.hub);
        move |_| {
            if let Some(hub) = hub.upgrade() {
                send(&hub, ReadingCommand::CheckUpdates);
            }
        }
    });
    header.pack_end(&check);
    header.pack_end(&download);
    states.loading(None);
    states.on_retry({
        let hub = Rc::downgrade(&ctx.hub);
        move || {
            if let Some(hub) = hub.upgrade() {
                send(&hub, ReadingCommand::LoadUpdates);
            }
        }
    });
    listen(ctx, &page, {
        let (ctx, states, column, download) =
            (Rc::downgrade(ctx), states.clone(), column.clone(), download.clone());
        move |event| {
            let Some(ctx) = ctx.upgrade() else { return };
            match event {
                Event::Reading(ReadingEvent::Updates(view)) => {
                    super::clear(&column);
                    draw(&ctx, &column, view);
                    let new =
                        view.days.iter().flat_map(|d| &d.rows).any(|r| r.chapter.new_chapter);
                    download.set_sensitive(new);
                    // Sources that failed stay in sight even with no chapters to list.
                    if view.days.is_empty() && view.failures.is_empty() {
                        states.empty();
                    } else {
                        states.content();
                    }
                }
                Event::Reading(ReadingEvent::Failed {
                    about: kaeri_model::reading::ReadingAbout::Updates,
                    failure,
                }) => states.failed(&super::failure(failure)),
                Event::Medium(_) if super::reads(&ctx.hub) => {
                    send(&ctx.hub, ReadingCommand::LoadUpdates)
                }
                _ => {}
            }
        }
    });
    if super::reads(&ctx.hub) {
        send(&ctx.hub, ReadingCommand::LoadUpdates);
    }
    page
}

fn draw(ctx: &Rc<Ctx>, column: &gtk::Box, view: &UpdatesView) {
    let status = if view.checking {
        Some(t::chapter_updates_checking())
    } else {
        view.checked_at.map(|at| t::updates_checked(&super::ago(at)))
    };
    if let Some(status) = status {
        column.append(&widgets::haze(&status, "caption"));
    }
    for day in &view.days {
        let heading = gtk::Label::new(Some(&day_label(day.day)));
        heading.add_css_class("heading");
        heading.set_xalign(0.0);
        column.append(&heading);
        let list = widgets::boxed_list();
        for row in &day.rows {
            let line = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&row.tile.title))
                .subtitle(glib::markup_escape_text(&format!(
                    "{} \u{b7} {}",
                    row.chapter.label, row.source_name
                )))
                .activatable(true)
                .build();
            line.add_prefix(&super::cover(
                &ctx.hub,
                row.tile.cover.as_deref(),
                &row.tile.title,
                40,
            ));
            if row.chapter.new_chapter {
                line.add_suffix(&super::new_words());
            }
            // A chapter already read steps back.
            if matches!(row.chapter.state, kaeri_model::reading::ChapterReadState::Read) {
                line.add_css_class("haze");
            }
            line.set_action_target_value(Some(&super::read_target(
                &row.tile.work,
                Some(&row.chapter.id),
            )));
            line.set_action_name(Some("win.read"));
            super::work_menu(&line, &row.tile.work, None);
            list.append(&line);
        }
        column.append(&list);
    }
    if view.days.is_empty() && !view.failures.is_empty() {
        column.append(&widgets::haze(&t::chapter_updates_empty_line(), "caption"));
    }
    for failure in &view.failures {
        let line = t::series_chapters_failed(&failure.source_name);
        column.append(&widgets::haze(&format!("{} \u{b7} {line}", failure.title), "caption")); // i18n-skip
    }
    let _ = say::msg;
}
