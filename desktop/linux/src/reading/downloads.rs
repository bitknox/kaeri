//! Downloads, Reading side: the storage meter, then each series with chapters kept.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    reading::{BookDownloadsView, ReadingCommand, ReadingEvent},
};

use super::{Dest, Place, dotted, listen, place, scope, send};
use crate::{nagare, states::Shape, widgets, window::Ctx};

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let empty = nagare::status_page(
        &t::downloads_empty_title(),
        &t::linux_read_downloads_empty(),
        None,
    );
    let Place { page, header, states, column } = place(
        ctx,
        &t::nav_downloads(),
        Shape::Rows { count: 3, width: 48, height: 72 },
        &empty,
        &t::downloads_load_failed(),
        false,
    );
    header.pack_end(&scope(ctx, Dest::Downloads, true));
    states.loading(Some(t::activity_downloads()));
    states.on_retry({
        let hub = Rc::downgrade(&ctx.hub);
        move || {
            if let Some(hub) = hub.upgrade() {
                send(&hub, ReadingCommand::LoadDownloads);
            }
        }
    });
    listen(ctx, &page, {
        let (ctx, states, column) = (Rc::downgrade(ctx), states.clone(), column.clone());
        move |event| {
            let Some(ctx) = ctx.upgrade() else { return };
            match event {
                Event::Reading(ReadingEvent::Downloads(view)) => {
                    super::clear(&column);
                    draw(&ctx, &column, view);
                    if view.series.is_empty() && view.rows.is_empty() {
                        states.empty();
                    } else {
                        states.content();
                    }
                }
                Event::Medium(_) if super::reads(&ctx.hub) => {
                    send(&ctx.hub, ReadingCommand::LoadDownloads)
                }
                _ => {}
            }
        }
    });
    if super::reads(&ctx.hub) {
        send(&ctx.hub, ReadingCommand::LoadDownloads);
    }
    page
}

fn draw(ctx: &Rc<Ctx>, column: &gtk::Box, view: &BookDownloadsView) {
    let total = dotted(&view.total);
    let level = gtk::LevelBar::new();
    level.set_hexpand(true);
    level.set_valign(gtk::Align::Center);
    level.set_size_request(80, -1);
    let all = (view.meter.used + view.meter.free).max(1);
    level.set_value(view.meter.used as f64 / all as f64);
    let meter =
        adw::ActionRow::builder().title(t::settings_group_storage()).subtitle(total).build();
    meter.add_suffix(&level);
    let list = widgets::boxed_list();
    list.append(&meter);
    column.append(&list);
    let series = widgets::boxed_list();
    for item in &view.series {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&item.title))
            .subtitle(glib::markup_escape_text(&dotted(&item.line)))
            .activatable(true)
            .build();
        row.add_prefix(&super::cover(&ctx.hub, item.cover.as_deref(), &item.title, 40));
        if let Some(progress) = item.progress {
            let bar = super::track(progress, 80);
            bar.set_valign(gtk::Align::Center);
            row.add_suffix(&bar);
        }
        let delete = gtk::Button::from_icon_name("user-trash-symbolic");
        delete.add_css_class("flat");
        delete.set_valign(gtk::Align::Center);
        delete.set_tooltip_text(Some(&t::read_downloads_delete_read()));
        delete.connect_clicked({
            let (hub, work) = (Rc::downgrade(&ctx.hub), item.work.clone());
            move |_| {
                if let Some(hub) = hub.upgrade() {
                    send(&hub, ReadingCommand::DownloadDeleteRead { work: work.clone() });
                }
            }
        });
        row.add_suffix(&delete);
        row.set_action_target_value(Some(&item.work.0.to_variant()));
        row.set_action_name(Some("win.open-series"));
        series.append(&row);
    }
    column.append(&series);
}
