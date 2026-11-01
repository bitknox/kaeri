//! Report a bug: what the report carries, the exact text, Copy report and Open on GitHub. Saved
//! crash reports are listed to include; while saving is off a line says so and links to the
//! setting.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::gdk;
use kaeri_i18n::t;
use kaeri_model::{
    bug_report::{BugReport, DeviceClass, issue_url_with, report_text},
    crash::{CrashRecord, when_text},
};

use crate::{widgets, window::Ctx};

const NAME: &str = "report-bug";

/// Open the sheet.
pub fn open(ctx: &Rc<Ctx>) {
    if ctx.window.visible_dialog().is_some_and(|d| d.widget_name() == NAME) {
        return;
    }
    let saving = kaeri_engine::crash::saving();
    let saved = kaeri_engine::crash::saved();
    let chosen: Rc<RefCell<Vec<String>>> =
        Rc::new(RefCell::new(saved.iter().map(|s| s.id.clone()).collect()));

    let header = adw::HeaderBar::new();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(6);
    column.set_margin_bottom(18);
    column.set_margin_start(18);
    column.set_margin_end(18);
    column.set_size_request(440, -1);
    let detail = widgets::haze(&t::report_detail(), "");
    detail.set_xalign(0.0);
    column.append(&detail);

    let group = adw::PreferencesGroup::builder().title(t::report_included()).build();
    let versions = adw::ActionRow::builder().title(t::report_versions()).build();
    versions.add_prefix(&gtk::Image::from_icon_name("object-select-symbolic"));
    group.add(&versions);
    let sent = gtk::Label::new(None);
    sent.set_selectable(true);
    sent.set_wrap(true);
    sent.set_xalign(0.0);
    sent.add_css_class("monospace");

    let build = {
        let ctx = Rc::clone(ctx);
        let chosen = Rc::clone(&chosen);
        move || -> (BugReport, Vec<CrashRecord>) {
            let report = BugReport {
                app_version: kaeri_engine::updates::CURRENT.to_owned(),
                platform: "Linux".to_owned(), // i18n-skip
                os_version: String::new(),
                device: DeviceClass::Computer,
                build: if cfg!(feature = "fixtures") { "fixtures" } else { "release" }
                    .to_owned(), // i18n-skip
                providers: ctx.hub.known.borrow().providers.clone(),
                ..Default::default()
            };
            let records = kaeri_engine::crash::armed_store()
                .map(|store| chosen.borrow().iter().filter_map(|id| store.get(id)).collect())
                .unwrap_or_default();
            (report, records)
        }
    };
    let build = Rc::new(build);
    let redraw = {
        let (sent, build) = (sent.clone(), Rc::clone(&build));
        Rc::new(move || {
            let (report, records) = build();
            sent.set_label(&report_text(&report, &records));
        })
    };

    if saving {
        for summary in &saved {
            let row = adw::ActionRow::builder()
                .title(t::report_crash_item(&when_text(summary.at_ms), summary.place.word()))
                .build();
            let check = gtk::CheckButton::new();
            check.set_active(true);
            check.set_valign(gtk::Align::Center);
            row.add_prefix(&check);
            row.set_activatable_widget(Some(&check));
            let (id, chosen, redraw) =
                (summary.id.clone(), Rc::clone(&chosen), Rc::clone(&redraw));
            check.connect_toggled(move |check| {
                let mut ids = chosen.borrow_mut();
                ids.retain(|i| *i != id);
                if check.is_active() {
                    ids.push(id.clone());
                }
                drop(ids);
                redraw();
            });
            group.add(&row);
        }
    } else {
        let off = adw::ActionRow::builder().title(t::report_crash_off()).build();
        off.set_title_lines(0);
        let settings = gtk::Button::with_label(&t::report_crash_settings());
        settings.add_css_class("flat");
        settings.set_valign(gtk::Align::Center);
        settings.connect_clicked({
            let weak = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = weak.upgrade() {
                    if let Some(dialog) = ctx.window.visible_dialog() {
                        dialog.force_close();
                    }
                    let _ = WidgetExt::activate_action(
                        &ctx.window,
                        "win.settings-pane",
                        Some(&"about".to_variant()),
                    );
                }
            }
        });
        off.add_suffix(&settings);
        group.add(&off);
    }
    column.append(&group);

    let reveal = gtk::Expander::new(Some(&t::report_show_sent()));
    reveal.set_child(Some(&sent));
    reveal.connect_expanded_notify(|reveal| {
        reveal.set_label(Some(&if reveal.is_expanded() {
            t::report_hide_sent()
        } else {
            t::report_show_sent()
        }));
    });
    column.append(&reveal);

    let buttons = adw::WrapBox::builder().child_spacing(8).line_spacing(8).build();
    let copy = gtk::Button::with_label(&t::report_copy());
    copy.add_css_class("pill");
    let open = gtk::Button::with_label(&t::report_open_github());
    open.add_css_class("pill");
    open.add_css_class("suggested-action");
    buttons.append(&copy);
    buttons.append(&open);
    column.append(&buttons);
    let privacy = widgets::haze(&t::report_privacy(), "caption");
    privacy.set_xalign(0.0);
    column.append(&privacy);
    redraw();

    copy.connect_clicked({
        let (weak, build) = (Rc::downgrade(ctx), Rc::clone(&build));
        move |button| {
            let (report, records) = build();
            if let Some(display) = gdk::Display::default() {
                display.clipboard().set_text(&report_text(&report, &records));
            }
            if let Some(ctx) = weak.upgrade() {
                ctx.toast(&t::report_copied());
            }
            let _ = button;
        }
    });
    open.connect_clicked({
        let (weak, build) = (Rc::downgrade(ctx), Rc::clone(&build));
        move |_| {
            let (report, records) = build();
            let url = issue_url_with(&report, &records);
            if let Some(ctx) = weak.upgrade() {
                crate::portal::open_uri(&ctx.window, &url, || {});
            }
        }
    });

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(640)
        .child(&column)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .name(NAME)
        .title(t::report_title())
        .follows_content_size(true)
        .child(&view)
        .build();
    dialog.present(Some(&ctx.window));
}
