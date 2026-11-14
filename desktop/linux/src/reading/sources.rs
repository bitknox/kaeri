//! Your sources, Reading side: the demo, folders, book servers and book plugins, and how to add
//! one. Adding offers a folder, a server or a plugin store invite and nothing else. A server's
//! row says how many series it holds and, when it takes progress, whether progress went back,
//! with Try again in place of the chevron while it did not.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use kaeri_i18n::t;
use kaeri_model::{
    book_sources::{
        BookFolderRow, BookFolderScan, BookPluginRow, BookServerRow, BookSourceCommand,
        BookSourceEvent, BookSourcesView,
    },
    command::Command,
    event::Event,
};

use super::{
    Place, came_in, listen, place,
    server_progress::{self, RowLine},
};
use crate::{nagare, states::Shape, widgets, window::Ctx};

fn send(hub: &crate::hub::Hub, command: BookSourceCommand) {
    hub.send(Command::BookSources(command));
}

/// The add menu's actions, on the page that holds the menu.
fn add_actions(ctx: &Rc<Ctx>, page: &adw::NavigationPage) {
    let group = gio::SimpleActionGroup::new();
    for (name, run) in [
        ("add-folder", came_in::add_folder as fn(&Rc<Ctx>)),
        ("add-server", |ctx: &Rc<Ctx>| came_in::add_server(ctx, None)),
        ("join-store", |ctx: &Rc<Ctx>| came_in::join_store(ctx, None)),
    ] {
        let action = gio::SimpleAction::new(name, None);
        let ctx = Rc::downgrade(ctx);
        action.connect_activate(move |_, _| {
            if let Some(ctx) = ctx.upgrade() {
                run(&ctx);
            }
        });
        group.add_action(&action);
    }
    page.insert_action_group("sources", Some(&group));
}

/// The add menu: a folder, a server, a plugin store invite.
pub fn add_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some(&t::book_add_folder()), Some("sources.add-folder"));
    menu.append(Some(&t::book_add_server()), Some("sources.add-server"));
    menu.append(Some(&t::onboarding_books_invite_heading()), Some("sources.join-store"));
    menu
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let empty = nagare::status_page(&t::sources_add_title(), &t::book_add_line(), None);
    let Place { page, header, states, column } = place(
        ctx,
        &t::sources_title(),
        Shape::Rows { count: 3, width: 40, height: 40 },
        &empty,
        &t::error_generic(),
        false,
    );
    add_actions(ctx, &page);
    let add = gtk::MenuButton::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text(t::sources_add_title())
        .build();
    add.set_menu_model(Some(&add_menu()));
    header.pack_end(&add);
    states.loading(None);
    states.on_retry({
        let hub = Rc::downgrade(&ctx.hub);
        move || {
            if let Some(hub) = hub.upgrade() {
                send(&hub, BookSourceCommand::Load);
            }
        }
    });
    let progress = server_progress::model(ctx);
    let redraw = {
        let (ctx, states, column) = (Rc::downgrade(ctx), states.clone(), column.clone());
        move || {
            let Some(ctx) = ctx.upgrade() else { return };
            let view = ctx.hub.known.borrow().book_sources.clone();
            let Some(view) = view else { return };
            let demo = ctx.hub.known.borrow().demo.reading;
            super::clear(&column);
            let shown = Shown::of(&view);
            if shown.is_empty() && !demo {
                states.empty();
            } else {
                draw(&ctx, &column, &view, demo);
                states.content();
            }
        }
    };
    let redraw = Rc::new(redraw);
    let heard = progress.on_change({
        let redraw = Rc::clone(&redraw);
        move || redraw()
    });
    page.connect_destroy({
        let progress = Rc::downgrade(&progress);
        move |_| {
            if let Some(progress) = progress.upgrade() {
                progress.forget(heard);
            }
        }
    });
    listen(ctx, &page, {
        let (ctx, redraw) = (Rc::downgrade(ctx), Rc::clone(&redraw));
        move |event| {
            let Some(ctx) = ctx.upgrade() else { return };
            match event {
                Event::BookSources(BookSourceEvent::View(_)) | Event::Demo(_) => redraw(),
                Event::Medium(_) if super::reads(&ctx.hub) => {
                    send(&ctx.hub, BookSourceCommand::Load);
                }
                _ => {}
            }
        }
    });
    if super::reads(&ctx.hub) {
        send(&ctx.hub, BookSourceCommand::Load);
    }
    page
}

/// What the page lists: removed folders wait out their Undo off the page.
struct Shown<'a> {
    folders: Vec<&'a BookFolderRow>,
    servers: &'a [BookServerRow],
    plugins: &'a [BookPluginRow],
}

impl<'a> Shown<'a> {
    fn of(view: &'a BookSourcesView) -> Self {
        Self {
            folders: view.folders.iter().filter(|f| !f.removed).collect(),
            servers: &view.servers,
            plugins: &view.plugins,
        }
    }

    fn is_empty(&self) -> bool {
        self.folders.is_empty() && self.servers.is_empty() && self.plugins.is_empty()
    }
}

/// The demo first, then each kind of source that has one: folders, servers, plugins.
fn draw(ctx: &Rc<Ctx>, column: &gtk::Box, view: &BookSourcesView, demo: bool) {
    let shown = Shown::of(view);
    if demo {
        let list = widgets::boxed_list();
        let row = row(
            "accessories-dictionary-symbolic",
            &t::sources_demo_name(),
            &t::sources_demo_books_line(),
            None,
        );
        row.suffix.append(&widgets::chevron());
        row.row.set_activatable(true);
        list.append(&row.row);
        list.connect_row_activated({
            let ctx = Rc::downgrade(ctx);
            move |_, _| {
                if let (Some(ctx), Some(nav)) =
                    (ctx.upgrade(), ctx.upgrade().and_then(|c| c.nav()))
                {
                    nav.push(&super::demo::about_page(&ctx));
                }
            }
        });
        column.append(&list);
    }
    if !shown.folders.is_empty() {
        draw_folders(ctx, column, &shown.folders);
    }
    if !shown.servers.is_empty() {
        draw_servers(ctx, column, shown.servers);
    }
    if !shown.plugins.is_empty() {
        draw_plugins(ctx, column, shown.plugins);
    }
}

/// A source's row: its tile, name, line and a third line for a state, with room at its end.
pub struct Row {
    pub row: gtk::ListBoxRow,
    pub suffix: gtk::Box,
}

/// The third line under a row: a glyph for a problem, then the words.
pub enum Third<'a> {
    Plain(&'a str),
    Problem(&'a str),
}

pub fn row(icon: &str, title: &str, subtitle: &str, third: Option<Third<'_>>) -> Row {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    line.add_css_class("source-row");
    line.append(&came_in::tile(icon));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let name = gtk::Label::new(Some(title));
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&name);
    let sub = widgets::haze(subtitle, "caption");
    words.append(&sub);
    let mut label = format!("{title}. {subtitle}"); // i18n-skip: the row's accessible name
    match third {
        Some(Third::Plain(text)) => {
            words.append(&widgets::haze(text, "caption"));
            label = format!("{label}. {text}"); // i18n-skip
        }
        Some(Third::Problem(text)) => {
            let problem = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let glyph = gtk::Image::from_icon_name("dialog-warning-symbolic");
            glyph.add_css_class("came-in-failed");
            glyph.set_pixel_size(12);
            problem.append(&glyph);
            let words_problem = gtk::Label::new(Some(text));
            words_problem.add_css_class("caption");
            words_problem.set_xalign(0.0);
            words_problem.set_wrap(true);
            problem.append(&words_problem);
            words.append(&problem);
            label = format!("{label}. {text}"); // i18n-skip
        }
        None => {}
    }
    line.append(&words);
    let suffix = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    suffix.set_valign(gtk::Align::Center);
    line.append(&suffix);
    let row = gtk::ListBoxRow::builder().child(&line).activatable(false).build();
    row.update_property(&[gtk::accessible::Property::Label(&label)]);
    Row { row, suffix }
}

fn draw_folders(ctx: &Rc<Ctx>, column: &gtk::Box, folders: &[&BookFolderRow]) {
    column.append(&widgets::heading(&t::book_sources_folders()));
    let list = widgets::boxed_list();
    for folder in folders {
        let path = folder.path.as_deref().unwrap_or(&folder.name);
        let line = match folder.scan {
            BookFolderScan::Scanning { found } => t::book_sources_scanning(i64::from(found)),
            BookFolderScan::Missing => t::book_sources_missing(),
            BookFolderScan::Done if folder.skipped > 0 => {
                t::source_row_folder(path, i64::from(folder.series), i64::from(folder.skipped))
            }
            BookFolderScan::Done => {
                format!("{path} \u{b7} {}", t::book_sources_series(i64::from(folder.series))) // i18n-skip
            }
        };
        let row = row("folder-symbolic", &folder.name, &line, None);
        let remove = gtk::Button::from_icon_name("user-trash-symbolic");
        remove.add_css_class("flat");
        remove.set_tooltip_text(Some(&t::action_remove()));
        remove.connect_clicked({
            let (ctx, id, name) = (Rc::downgrade(ctx), folder.id.clone(), folder.name.clone());
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    remove_folder(&ctx, &id, &name);
                }
            }
        });
        row.suffix.append(&remove);
        list.append(&row.row);
    }
    column.append(&list);
}

/// Remove a folder: nothing on disk is touched, and a notice holds Undo for 8 seconds.
pub fn remove_folder(ctx: &Rc<Ctx>, id: &str, name: &str) {
    if ctx.window.lookup_action("book-folder-restore").is_none() {
        let action =
            gio::SimpleAction::new("book-folder-restore", Some(glib::VariantTy::STRING));
        let hub = Rc::downgrade(&ctx.hub);
        action.connect_activate(move |_, target| {
            if let (Some(hub), Some(id)) =
                (hub.upgrade(), target.and_then(String::from_variant))
            {
                send(&hub, BookSourceCommand::RestoreFolder { id });
            }
        });
        ctx.window.add_action(&action);
    }
    send(&ctx.hub, BookSourceCommand::RemoveFolder { id: id.to_owned() });
    let text =
        format!("{} \u{b7} {}", t::sources_removed(name), t::sources_removed_files_stay()); // i18n-skip
    ctx.toast_to(&text, &t::action_undo(), "win.book-folder-restore", id);
}

/// "Kavita · 212 series in 2 libraries", or that it shows no series yet.
fn server_line(server: &BookServerRow) -> String {
    let kind = came_in::kind_words(server.kind);
    if server.empty {
        t::source_row_server_empty(&kind)
    } else if came_in::just_added(&server.id) {
        t::source_row_server(
            &kind,
            i64::from(server.series),
            i64::from(server.libraries.max(1)),
        )
    } else {
        t::source_row_server_series(
            &kind,
            i64::from(server.series),
            i64::from(server.libraries.max(1)),
        )
    }
}

fn draw_servers(ctx: &Rc<Ctx>, column: &gtk::Box, servers: &[BookServerRow]) {
    column.append(&widgets::heading(&t::book_sources_servers()));
    let list = widgets::boxed_list();
    let progress = server_progress::model(ctx);
    for server in servers {
        let (words, retry) = server_progress::row_parts(&progress, &server.id);
        let problem = matches!(progress.row_line(&server.id), Some(RowLine::Failed(..)));
        let health = if server.signed_out {
            Some(t::linux_book_server_signed_out())
        } else if server.off {
            Some(t::book_sources_off())
        } else {
            None
        };
        let third = health.as_deref().map(Third::Plain).or_else(|| {
            words.as_deref().map(|w| if problem { Third::Problem(w) } else { Third::Plain(w) })
        });
        let host = came_in::host_of(&server.address);
        let row = row("network-server-symbolic", &host, &server_line(server), third);
        match retry {
            Some(retry) if health.is_none() => row.suffix.append(&retry),
            _ => row.suffix.append(&widgets::chevron()),
        }
        row.row.set_activatable(true);
        row.row.set_widget_name(&server.id);
        list.append(&row.row);
    }
    list.connect_row_activated({
        let ctx = Rc::downgrade(ctx);
        move |_, row| {
            let Some(ctx) = ctx.upgrade() else { return };
            let id = row.widget_name().to_string();
            if let Some(nav) = ctx.nav() {
                nav.push(&server_page(&ctx, &id));
            }
        }
    });
    column.append(&list);
}

fn plugin_line(plugin: &BookPluginRow) -> String {
    let medium = t::medium_answer_read().to_lowercase();
    match &plugin.store {
        Some(store) => t::source_row_plugin(store, &medium, &plugin.version),
        None => t::book_sources_version(&plugin.version),
    }
}

fn draw_plugins(ctx: &Rc<Ctx>, column: &gtk::Box, plugins: &[BookPluginRow]) {
    column.append(&widgets::heading(&t::book_sources_plugins()));
    let list = widgets::boxed_list();
    for plugin in plugins {
        let line = plugin_line(plugin);
        let third = plugin.off.then(t::book_sources_off);
        let row = row(
            "application-x-addon-symbolic",
            &plugin.name,
            &line,
            third.as_deref().map(Third::Plain),
        );
        row.suffix.append(&widgets::chevron());
        row.row.set_activatable(true);
        row.row.set_widget_name(&plugin.id);
        list.append(&row.row);
    }
    list.connect_row_activated({
        let ctx = Rc::downgrade(ctx);
        move |_, row| {
            let Some(ctx) = ctx.upgrade() else { return };
            let id = row.widget_name().to_string();
            if let Some(nav) = ctx.nav() {
                nav.push(&plugin_page(&ctx, &id));
            }
        }
    });
    column.append(&list);
    column.append(&widgets::haze(&t::book_sources_plugins_footer(), "caption"));
}

/// A book plugin's own page: where it came from, its version, and whether it is used.
fn plugin_page(ctx: &Rc<Ctx>, id: &str) -> adw::NavigationPage {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
    let header = adw::HeaderBar::new();
    let page = widgets::read_content_page("", &header, &widgets::column_page(&column, 640));
    let draw = {
        let (ctx, column, page, id) =
            (Rc::downgrade(ctx), column.clone(), page.clone(), id.to_owned());
        move || {
            let Some(ctx) = ctx.upgrade() else { return };
            let plugin = ctx
                .hub
                .known
                .borrow()
                .book_sources
                .as_ref()
                .and_then(|v| v.plugins.iter().find(|p| p.id == id).cloned());
            let Some(plugin) = plugin else {
                if let Some(nav) =
                    page.parent().and_then(|p| p.downcast::<adw::NavigationView>().ok())
                {
                    nav.pop();
                }
                return;
            };
            page.set_title(&plugin.name);
            super::clear(&column);
            draw_plugin(&ctx, &column, &plugin);
        }
    };
    let draw = Rc::new(draw);
    listen(ctx, &page, {
        let draw = Rc::clone(&draw);
        move |event| {
            if matches!(event, Event::BookSources(BookSourceEvent::View(_))) {
                draw();
            }
        }
    });
    draw();
    page
}

fn draw_plugin(ctx: &Rc<Ctx>, column: &gtk::Box, plugin: &BookPluginRow) {
    let top = widgets::boxed_list();
    top.append(
        &row("application-x-addon-symbolic", &plugin.name, &plugin_line(plugin), None).row,
    );
    column.append(&top);
    let settings = widgets::boxed_list();
    let used =
        adw::SwitchRow::builder().title(t::linux_book_plugin_use()).active(!plugin.off).build();
    used.connect_active_notify({
        let (hub, id) = (Rc::downgrade(&ctx.hub), plugin.id.clone());
        move |row| {
            if let Some(hub) = hub.upgrade() {
                send(&hub, BookSourceCommand::SetOff { id: id.clone(), off: !row.is_active() });
            }
        }
    });
    settings.append(&used);
    column.append(&settings);
    column.append(&widgets::haze(&t::book_sources_plugins_footer(), "caption"));
}

/// A server's own page: what it holds, Send progress, whether it is used, Edit and Remove.
fn server_page(ctx: &Rc<Ctx>, id: &str) -> adw::NavigationPage {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
    let header = adw::HeaderBar::new();
    let page = widgets::read_content_page("", &header, &widgets::column_page(&column, 640));
    let draw = {
        let (ctx, column, page, id) =
            (Rc::downgrade(ctx), column.clone(), page.clone(), id.to_owned());
        move || {
            let Some(ctx) = ctx.upgrade() else { return };
            let server = ctx
                .hub
                .known
                .borrow()
                .book_sources
                .as_ref()
                .and_then(|v| v.servers.iter().find(|s| s.id == id).cloned());
            let Some(server) = server else {
                // Removed: back to the list.
                if let Some(nav) =
                    page.parent().and_then(|p| p.downcast::<adw::NavigationView>().ok())
                {
                    nav.pop();
                }
                return;
            };
            page.set_title(&came_in::host_of(&server.address));
            super::clear(&column);
            draw_server(&ctx, &column, &server);
        }
    };
    let draw = Rc::new(draw);
    listen(ctx, &page, {
        let draw = Rc::clone(&draw);
        move |event| {
            if matches!(event, Event::BookSources(BookSourceEvent::View(_))) {
                draw();
            }
        }
    });
    let progress = server_progress::model(ctx);
    let heard = progress.on_change({
        let draw = Rc::clone(&draw);
        move || draw()
    });
    page.connect_destroy({
        let progress = Rc::downgrade(&progress);
        move |_| {
            if let Some(progress) = progress.upgrade() {
                progress.forget(heard);
            }
        }
    });
    draw();
    page
}

fn draw_server(ctx: &Rc<Ctx>, column: &gtk::Box, server: &BookServerRow) {
    let top = widgets::boxed_list();
    let host = came_in::host_of(&server.address);
    top.append(&row("network-server-symbolic", &host, &server_line(server), None).row);
    column.append(&top);

    let settings = widgets::boxed_list();
    let used =
        adw::SwitchRow::builder().title(t::linux_book_server_use()).active(!server.off).build();
    used.connect_active_notify({
        let (hub, id) = (Rc::downgrade(&ctx.hub), server.id.clone());
        move |row| {
            if let Some(hub) = hub.upgrade() {
                send(&hub, BookSourceCommand::SetOff { id: id.clone(), off: !row.is_active() });
            }
        }
    });
    settings.append(&used);
    if server.kind != kaeri_model::book_sources::BookServerKind::Opds {
        let progress = adw::SwitchRow::builder()
            .title(t::book_server_sync_progress())
            .subtitle(glib::markup_escape_text(&t::book_server_sync_progress_line(&host)))
            .active(server.report_progress)
            .build();
        progress.connect_active_notify({
            let (hub, id) = (Rc::downgrade(&ctx.hub), server.id.clone());
            move |row| {
                if let Some(hub) = hub.upgrade() {
                    send(
                        &hub,
                        BookSourceCommand::SetReportProgress {
                            id: id.clone(),
                            on: row.is_active(),
                        },
                    );
                }
            }
        });
        settings.append(&progress);
        let model = server_progress::model(ctx);
        let (words, retry) = server_progress::row_parts(&model, &server.id);
        if let Some(words) = words {
            let line =
                adw::ActionRow::builder().title(glib::markup_escape_text(&words)).build();
            line.add_css_class("property");
            if let Some(retry) = retry {
                line.add_suffix(&retry);
            }
            settings.append(&line);
        }
    }
    column.append(&settings);

    let actions = widgets::boxed_list();
    let edit = adw::ButtonRow::builder().title(t::action_edit()).build();
    edit.connect_activated({
        let (ctx, address) = (Rc::downgrade(ctx), server.address.clone());
        move |_| {
            if let Some(ctx) = ctx.upgrade() {
                came_in::add_server(&ctx, Some(&address));
            }
        }
    });
    actions.append(&edit);
    let remove = adw::ButtonRow::builder().title(t::action_remove()).build();
    remove.add_css_class("destructive-action");
    remove.connect_activated({
        let (ctx, id, host) = (Rc::downgrade(ctx), server.id.clone(), host.clone());
        move |_| {
            if let Some(ctx) = ctx.upgrade() {
                confirm_remove(&ctx, &id, &host);
            }
        }
    });
    actions.append(&remove);
    column.append(&actions);
}

/// Removing a server asks first: its series leave the library.
fn confirm_remove(ctx: &Rc<Ctx>, id: &str, host: &str) {
    let dialog = adw::AlertDialog::new(
        Some(&t::linux_book_server_remove_title(host)),
        Some(&t::servers_remove_line()),
    );
    dialog.add_response("cancel", &t::action_cancel());
    dialog.add_response("remove", &t::action_remove());
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let (hub, id) = (Rc::downgrade(&ctx.hub), id.to_owned());
    dialog.connect_response(None, move |_, response| {
        if response == "remove"
            && let Some(hub) = hub.upgrade()
        {
            send(&hub, BookSourceCommand::RemoveServer { id: id.clone() });
        }
    });
    dialog.present(Some(&ctx.window));
}

/// Choose a folder to read from, and say what came in.
pub fn add_folder(ctx: &Rc<Ctx>) {
    came_in::add_folder(ctx);
}

/// Add a book server by its address, and say what came in.
pub fn add_server(ctx: &Rc<Ctx>) {
    came_in::add_server(ctx, None);
}

/// For a shot: a server's page.
#[cfg(feature = "fixtures")]
pub fn shot_server_page(ctx: &Rc<Ctx>, id: &str) -> adw::NavigationPage {
    server_page(ctx, id)
}

/// For a shot: a plugin's page.
#[cfg(feature = "fixtures")]
pub fn shot_plugin_page(ctx: &Rc<Ctx>, id: &str) -> adw::NavigationPage {
    plugin_page(ctx, id)
}

/// For a shot: the scene's first folder removed, its Undo showing.
#[cfg(feature = "fixtures")]
pub fn shot_removed(ctx: &Rc<Ctx>) {
    let folder = ctx
        .hub
        .known
        .borrow()
        .book_sources
        .as_ref()
        .and_then(|v| v.folders.first().map(|f| (f.id.clone(), f.name.clone())));
    if let Some((id, name)) = folder {
        remove_folder(ctx, &id, &name);
    }
}
