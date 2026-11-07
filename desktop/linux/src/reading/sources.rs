//! Your sources, Reading side: folders, book servers and book plugins, and how to add one.
//! Adding offers a folder, a server or a plugin store invite and nothing else.

use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    book_sources::{
        BookFolderScan, BookSourceCommand, BookSourceEvent, BookSourcesView, FolderRef,
    },
    command::Command,
    event::Event,
    stores::Secret,
};

use super::{Place, listen, place};
use crate::{nagare, states::Shape, widgets, window::Ctx};

fn send(hub: &crate::hub::Hub, command: BookSourceCommand) {
    hub.send(Command::BookSources(command));
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
    let add = gtk::MenuButton::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text(t::sources_add_title())
        .build();
    let menu = gio::Menu::new();
    menu.append(Some(&t::book_add_folder()), Some("win.book-add-folder"));
    menu.append(Some(&t::book_add_server()), Some("win.book-add-server"));
    add.set_menu_model(Some(&menu));
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
    listen(ctx, &page, {
        let (ctx, states, column) = (Rc::downgrade(ctx), states.clone(), column.clone());
        move |event| {
            let Some(ctx) = ctx.upgrade() else { return };
            if let Event::BookSources(BookSourceEvent::View(view)) = event {
                super::clear(&column);
                if view.folders.is_empty() && view.servers.is_empty() && view.plugins.is_empty()
                {
                    states.empty();
                } else {
                    draw(&ctx, &column, view);
                    states.content();
                }
            }
            if matches!(event, Event::Medium(_)) && super::reads(&ctx.hub) {
                send(&ctx.hub, BookSourceCommand::Load);
            }
        }
    });
    if super::reads(&ctx.hub) {
        send(&ctx.hub, BookSourceCommand::Load);
    }
    page
}

/// Each kind of source that has one: folders, servers, plugins.
fn draw(ctx: &Rc<Ctx>, column: &gtk::Box, view: &BookSourcesView) {
    if !view.folders.is_empty() {
        draw_folders(ctx, column, view);
    }
    if !view.servers.is_empty() {
        draw_servers(ctx, column, view);
    }
    if !view.plugins.is_empty() {
        draw_plugins(ctx, column, view);
    }
}

fn draw_folders(ctx: &Rc<Ctx>, column: &gtk::Box, view: &BookSourcesView) {
    column.append(&widgets::heading(&t::book_sources_folders()));
    let folders = widgets::boxed_list();
    for folder in &view.folders {
        let state = match folder.scan {
            BookFolderScan::Scanning { found } => t::book_sources_scanning(i64::from(found)),
            BookFolderScan::Done => t::book_sources_series(i64::from(folder.series)),
            BookFolderScan::Missing => t::book_sources_missing(),
        };
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&folder.name))
            .subtitle(glib::markup_escape_text(&state))
            .build();
        let remove = gtk::Button::from_icon_name("user-trash-symbolic");
        remove.add_css_class("flat");
        remove.set_valign(gtk::Align::Center);
        remove.connect_clicked({
            let (hub, id) = (Rc::downgrade(&ctx.hub), folder.id.clone());
            move |_| {
                if let Some(hub) = hub.upgrade() {
                    send(&hub, BookSourceCommand::RemoveFolder { id: id.clone() });
                }
            }
        });
        row.add_suffix(&remove);
        folders.append(&row);
    }
    column.append(&folders);
}

fn draw_servers(ctx: &Rc<Ctx>, column: &gtk::Box, view: &BookSourcesView) {
    column.append(&widgets::heading(&t::book_sources_servers()));
    let servers = widgets::boxed_list();
    for server in &view.servers {
        let row = adw::SwitchRow::builder()
            .title(glib::markup_escape_text(&server.name))
            .subtitle(glib::markup_escape_text(&server.address))
            .active(!server.off)
            .build();
        row.connect_active_notify({
            let (hub, id) = (Rc::downgrade(&ctx.hub), server.id.clone());
            move |row| {
                if let Some(hub) = hub.upgrade() {
                    send(
                        &hub,
                        BookSourceCommand::SetOff { id: id.clone(), off: !row.is_active() },
                    );
                }
            }
        });
        servers.append(&row);
    }
    column.append(&servers);
}

fn draw_plugins(ctx: &Rc<Ctx>, column: &gtk::Box, view: &BookSourcesView) {
    {
        column.append(&widgets::heading(&t::book_sources_plugins()));
        let plugins = widgets::boxed_list();
        for plugin in &view.plugins {
            let row = adw::SwitchRow::builder()
                .title(glib::markup_escape_text(&plugin.name))
                .subtitle(glib::markup_escape_text(&t::book_sources_version(&plugin.version)))
                .active(!plugin.off)
                .build();
            row.connect_active_notify({
                let (hub, id) = (Rc::downgrade(&ctx.hub), plugin.id.clone());
                move |row| {
                    if let Some(hub) = hub.upgrade() {
                        send(
                            &hub,
                            BookSourceCommand::SetOff { id: id.clone(), off: !row.is_active() },
                        );
                    }
                }
            });
            plugins.append(&row);
        }
        column.append(&plugins);
        column.append(&widgets::haze(&t::book_sources_plugins_footer(), "caption"));
    }
}

/// Choose a folder to read from.
pub fn add_folder(ctx: &Rc<Ctx>) {
    let dialog = gtk::FileDialog::new();
    dialog.set_title(&t::book_add_folder());
    let hub = Rc::downgrade(&ctx.hub);
    dialog.select_folder(Some(&ctx.window), gio::Cancellable::NONE, move |result| {
        let (Ok(file), Some(hub)) = (result, hub.upgrade()) else { return };
        let path = file.path().map(|p| p.to_string_lossy().into_owned());
        let name = file.basename().map(|p| p.to_string_lossy().into_owned());
        send(
            &hub,
            BookSourceCommand::AddFolder {
                folder: FolderRef { path, tree_uri: None, bookmark: None, name },
            },
        );
    });
}

/// Add a book server by its address and sign-in.
pub fn add_server(ctx: &Rc<Ctx>) {
    let dialog =
        adw::AlertDialog::new(Some(&t::book_add_server_title()), Some(&t::book_server_note()));
    let form = adw::PreferencesGroup::new();
    let address = adw::EntryRow::builder().title(t::book_add_server_title()).build();
    let user = adw::EntryRow::builder().title(t::accounts_username()).build();
    let password = adw::PasswordEntryRow::builder().title(t::accounts_password()).build();
    form.add(&address);
    form.add(&user);
    form.add(&password);
    dialog.set_extra_child(Some(&form));
    dialog.add_response("cancel", &t::action_cancel());
    dialog.add_response("add", &t::book_add_server_button());
    dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("add"));
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_response(None, move |_, response| {
        let Some(hub) = hub.upgrade() else { return };
        if response == "add" {
            send(
                &hub,
                BookSourceCommand::SignIn {
                    address: address.text().to_string(),
                    user: user.text().to_string(),
                    password: Secret(password.text().to_string()),
                },
            );
        }
    });
    dialog.present(Some(&ctx.window));
}
