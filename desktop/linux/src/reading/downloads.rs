//! Downloads, Reading: a section of each series with chapters kept, as the app board's Downloads
//! with Both draws it. With Both it sits on the one Downloads page beside Shows; with Read alone
//! it is the page. A row says what its chapters come to, or how far the one downloading is with
//! the ring, or what failed with Retry; Delete read clears chapters already read.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    reading::{
        BookDownloadSeries, BookDownloadsView, DownloadPick, ReadingCommand, ReadingEvent,
    },
};

use super::{Place, dotted, listen, place, send};
use crate::{states::Shape, widgets, window::Ctx};

/// The Reading section: its heading with the total, then a row a series.
pub struct Section {
    pub widget: gtk::Box,
    total: gtk::Label,
    list: gtk::ListBox,
    view: RefCell<Option<BookDownloadsView>>,
    changed: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Section {
    pub fn new(ctx: &Rc<Ctx>, heading: bool) -> Rc<Self> {
        let title = gtk::Label::new(Some(&t::library_scope_reading()));
        title.add_css_class("heading");
        title.set_xalign(0.0);
        let total = widgets::haze("", "caption");
        total.set_xalign(0.0);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        head.append(&title);
        head.append(&total);
        head.set_visible(heading);
        let list = widgets::boxed_list();
        list.set_selection_mode(gtk::SelectionMode::None);
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 8);
        widget.append(&head);
        widget.append(&list);
        widget.set_visible(false);
        let section = Rc::new(Self {
            widget,
            total,
            list,
            view: RefCell::default(),
            changed: RefCell::default(),
        });
        listen(ctx, &section.widget, {
            let (ctx, section) = (Rc::downgrade(ctx), Rc::downgrade(&section));
            move |event| {
                let (Some(ctx), Some(section)) = (ctx.upgrade(), section.upgrade()) else {
                    return;
                };
                match event {
                    Event::Reading(ReadingEvent::Downloads(view)) => section.draw(&ctx, view),
                    Event::Medium(_) if super::reads(&ctx.hub) => {
                        send(&ctx.hub, ReadingCommand::LoadDownloads);
                    }
                    _ => {}
                }
            }
        });
        if super::reads(&ctx.hub) {
            send(&ctx.hub, ReadingCommand::LoadDownloads);
        }
        section
    }

    /// Run `changed` each time the downloads change, for the page around the section.
    pub fn on_change(&self, changed: impl Fn() + 'static) {
        self.changed.replace(Some(Rc::new(changed)));
    }

    /// Whether any series has chapters kept or on their way.
    pub fn has_any(&self) -> bool {
        self.view.borrow().as_ref().is_some_and(|v| !v.series.is_empty() || !v.rows.is_empty())
    }

    /// The bytes the chapters take.
    pub fn used(&self) -> u64 {
        self.view.borrow().as_ref().map_or(0, |v| v.meter.used)
    }

    fn draw(&self, ctx: &Rc<Ctx>, view: &BookDownloadsView) {
        self.total.set_label(&dotted(&view.total));
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        for item in &view.series {
            self.list.append(&series_row(ctx, item));
        }
        self.view.replace(Some(view.clone()));
        self.widget.set_visible(self.has_any());
        let changed = self.changed.borrow().clone();
        if let Some(changed) = changed {
            changed();
        }
    }
}

/// A series' row: cover, title, its line, and at the end the ring, Retry or Delete read.
fn series_row(ctx: &Rc<Ctx>, item: &BookDownloadSeries) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&item.title))
        .subtitle(glib::markup_escape_text(&dotted(&item.line)))
        .activatable(true)
        .build();
    row.set_subtitle_lines(2);
    row.add_prefix(&super::cover(&ctx.hub, item.cover.as_deref(), &item.title, 40));
    let hub = Rc::downgrade(&ctx.hub);
    if let Some(progress) = item.progress {
        let ring = widgets::ring(f64::from(progress));
        let percent = (progress.clamp(0.0, 1.0) * 100.0).round() as i64;
        ring.update_property(&[gtk::accessible::Property::Label(&t::send_progress_spoken(
            percent,
        ))]);
        row.add_suffix(&ring);
    } else if !item.failed.is_empty() {
        let retry = gtk::Button::with_label(&t::chapter_retry());
        retry.add_css_class("pill");
        retry.add_css_class("small");
        retry.set_valign(gtk::Align::Center);
        retry.connect_clicked({
            let (hub, work, chapters) = (hub.clone(), item.work.clone(), item.failed.clone());
            move |_| {
                if let Some(hub) = hub.upgrade() {
                    let pick = DownloadPick::Chapters { ids: chapters.clone() };
                    send(&hub, ReadingCommand::Download { work: work.clone(), pick });
                }
            }
        });
        row.add_suffix(&retry);
    } else if item.read > 0 {
        let delete = gtk::Button::with_label(&t::read_downloads_delete_read());
        delete.add_css_class("flat");
        delete.set_valign(gtk::Align::Center);
        delete.connect_clicked({
            let (hub, work) = (hub.clone(), item.work.clone());
            move |_| {
                if let Some(hub) = hub.upgrade() {
                    send(&hub, ReadingCommand::DownloadDeleteRead { work: work.clone() });
                }
            }
        });
        row.add_suffix(&delete);
    }
    // The same and more from a right-click or a long press.
    let menu = gio::Menu::new();
    menu.append_item(&widgets::menu_item(
        &t::chapter_updates_open_series_menu(),
        "win.open-series",
        &item.work.0,
    ));
    if item.read > 0 {
        menu.append_item(&widgets::menu_item(
            &t::read_downloads_delete_read(),
            "win.read-delete-read",
            &item.work.0,
        ));
    }
    widgets::context_menu(&row, &menu);
    row.set_action_target_value(Some(&item.work.0.to_variant()));
    row.set_action_name(Some("win.open-series"));
    row
}

/// Downloads with Read alone: what the chapters take, then the Reading section without its
/// heading.
pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let empty =
        crate::states::plain(&t::downloads_empty_title(), &t::linux_read_downloads_empty());
    let Place { page, states, column, .. } = place(
        ctx,
        &t::nav_downloads(),
        Shape::Rows { count: 3, width: 48, height: 72 },
        &empty,
        &t::downloads_load_failed(),
        false,
    );
    let meter = adw::ActionRow::builder().title(t::settings_group_storage()).build();
    meter.add_css_class("property");
    let meter_list = widgets::boxed_list();
    meter_list.append(&meter);
    column.append(&meter_list);
    let section = Section::new(ctx, false);
    column.append(&section.widget);
    states.loading(Some(t::activity_downloads()));
    states.on_retry({
        let hub = Rc::downgrade(&ctx.hub);
        move || {
            if let Some(hub) = hub.upgrade() {
                send(&hub, ReadingCommand::LoadDownloads);
            }
        }
    });
    section.on_change({
        let (states, section, meter) = (states.clone(), Rc::downgrade(&section), meter.clone());
        move || {
            let Some(section) = section.upgrade() else { return };
            // What the chapters take, never the disk's own fill.
            meter.set_subtitle(&t::downloads_space_used(&glib::format_size(section.used())));
            if section.has_any() {
                states.content();
            } else {
                states.empty();
            }
        }
    });
    widgets::keep(&page, section);
    page
}
