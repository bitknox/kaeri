//! Send to e-reader: pick the device, prepare the next unread chapters as files that fit its
//! screen, then save them to a folder to copy across. The engine decides the formats and the
//! sizes; this lists the devices and hands the files over.

use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use kaeri_core::books::WorkId;
use kaeri_i18n::t;
use kaeri_model::{
    event::Event,
    reading::{
        DownloadPick, ExportFile, ExportFormat, ExportTarget, ReadingCommand, ReadingEvent,
    },
};

use super::{listen, send};
use crate::window::Ctx;

fn format_for(target: &ExportTarget) -> ExportFormat {
    target.formats.first().copied().unwrap_or(ExportFormat::Epub)
}

pub fn open(ctx: &Rc<Ctx>, work: &WorkId) {
    let dialog = adw::Dialog::new();
    dialog.set_title(&t::linux_send_title());
    dialog.set_content_width(440);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(18);
    column.set_margin_bottom(18);
    column.set_margin_start(18);
    column.set_margin_end(18);
    let line = gtk::Label::new(Some(&t::linux_send_line()));
    line.set_wrap(true);
    line.set_xalign(0.0);
    column.append(&line);
    let list = crate::widgets::boxed_list();
    list.set_selection_mode(gtk::SelectionMode::Single);
    column.append(&list);
    let status = gtk::Label::new(None);
    status.add_css_class("haze");
    status.set_xalign(0.0);
    column.append(&status);
    let prepare = gtk::Button::with_label(&t::linux_send_prepare());
    prepare.add_css_class("suggested-action");
    prepare.add_css_class("pill");
    prepare.set_sensitive(false);
    column.append(&prepare);
    let save = gtk::Button::with_label(&t::linux_send_save());
    save.add_css_class("pill");
    save.set_visible(false);
    column.append(&save);
    dialog.set_child(Some(&column));

    let targets: Rc<std::cell::RefCell<Vec<ExportTarget>>> = Rc::default();
    let files: Rc<std::cell::RefCell<Vec<ExportFile>>> = Rc::default();
    list.connect_row_selected({
        let prepare = prepare.clone();
        move |_, row| prepare.set_sensitive(row.is_some())
    });
    prepare.connect_clicked({
        let (hub, work, list, targets) =
            (Rc::downgrade(&ctx.hub), work.clone(), list.clone(), Rc::clone(&targets));
        move |_| {
            let (Some(hub), Some(row)) = (hub.upgrade(), list.selected_row()) else { return };
            let Some(target) = targets.borrow().get(row.index() as usize).cloned() else {
                return;
            };
            send(
                &hub,
                ReadingCommand::Prepare {
                    work: work.clone(),
                    pick: DownloadPick::NextUnread { n: 10 },
                    format: format_for(&target),
                    target: target.id,
                },
            );
        }
    });
    save.connect_clicked({
        let (files, window) = (Rc::clone(&files), ctx.window.clone());
        move |_| {
            let files = files.borrow().clone();
            let chooser = gtk::FileDialog::new();
            chooser.select_folder(Some(&window), gio::Cancellable::NONE, move |result| {
                let Ok(folder) = result else { return };
                let Some(folder) = folder.path() else { return };
                for file in &files {
                    let to = folder.join(&file.name);
                    if let Err(error) = std::fs::copy(&file.path, &to) {
                        tracing::warn!(%error, "a prepared file was not saved");
                    }
                }
            });
        }
    });
    listen(ctx, &dialog, {
        let (list, status, save, prepare) =
            (list.clone(), status.clone(), save.clone(), prepare.clone());
        let (targets, files) = (Rc::clone(&targets), Rc::clone(&files));
        move |event| {
            let Event::Reading(event) = event else { return };
            match event {
                ReadingEvent::ExportTargets(found) => {
                    while let Some(row) = list.first_child() {
                        list.remove(&row);
                    }
                    for target in found {
                        list.append(&adw::ActionRow::builder().title(&target.name).build());
                    }
                    *targets.borrow_mut() = found.clone();
                }
                ReadingEvent::ExportProgress { done, total, .. } => {
                    status.set_label(&t::linux_send_progress(
                        i64::from(*done),
                        i64::from(*total),
                    ));
                }
                ReadingEvent::ExportReady { files: ready, .. } => {
                    status.set_label("");
                    prepare.set_visible(false);
                    save.set_visible(true);
                    *files.borrow_mut() = ready.clone();
                }
                ReadingEvent::ExportWaiting { line, .. } => {
                    status.set_label(&crate::say::msg(line))
                }
                _ => {}
            }
        }
    });
    send(&ctx.hub, ReadingCommand::LoadExportTargets);
    dialog.present(Some(&ctx.window));
}
