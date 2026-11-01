//! Watch together: the place. Start a party or join one, the party you are in, the last ten
//! rooms. The lobby is its own page, pushed from here.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use kaeri_core::ids::AnilistId;
use kaeri_i18n::t;
use kaeri_model::{
    catalog::{ContentFor, Entry},
    command::Command,
    event::Event,
    party::{PartyCommand, PartyEpisode, PartyRecentRoom, PartyTarget},
    search::SearchQuery,
};

use crate::{pages::lobby, widgets, window::Ctx};

/// The sample code the shape message gives.
const EXAMPLE_CODE: &str = "kite-7qm4-xr9d";

struct Page {
    ctx: Rc<Ctx>,
    field: gtk::Entry,
    problem: gtk::Label,
    party: gtk::Box,
    party_line: gtk::Label,
    party_open: gtk::Button,
    recent: gtk::ListBox,
    recent_empty: gtk::Label,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 24);
    let line = widgets::haze(&t::together_line(), "");
    column.append(&line);

    // Start and Join: two columns that stack when the window is narrow.
    let columns = gtk::FlowBox::new();
    columns.set_selection_mode(gtk::SelectionMode::None);
    columns.set_max_children_per_line(2);
    columns.set_min_children_per_line(1);
    columns.set_column_spacing(40);
    columns.set_row_spacing(24);
    columns.set_homogeneous(true);

    let start = gtk::Box::new(gtk::Orientation::Vertical, 8);
    start.set_valign(gtk::Align::Start);
    let heading = gtk::Label::new(Some(&t::together_start()));
    heading.add_css_class("title-3");
    heading.set_xalign(0.0);
    start.append(&heading);
    start.append(&widgets::haze(&t::together_start_line(), ""));
    let start_button = gtk::Button::with_label(&t::together_start());
    start_button.add_css_class("suggested-action");
    start_button.add_css_class("pill");
    start_button.set_halign(gtk::Align::Start);
    start_button.set_margin_top(8);
    start.append(&start_button);
    columns.append(&start);

    let join = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let heading = gtk::Label::new(Some(&t::together_join()));
    heading.add_css_class("title-3");
    heading.set_xalign(0.0);
    join.append(&heading);
    join.append(&widgets::haze(&t::together_join_line(), ""));
    let field = gtk::Entry::new();
    field.set_placeholder_text(Some(&t::together_join_field()));
    field.set_hexpand(true);
    field.update_property(&[gtk::accessible::Property::Label(&t::together_join_field())]);
    let paste = gtk::Button::with_label(&t::together_paste());
    let go = gtk::Button::with_label(&t::together_join_action());
    go.add_css_class("suggested-action");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_margin_top(8);
    row.append(&field);
    row.append(&paste);
    row.append(&go);
    join.append(&row);
    let problem = widgets::haze("", "caption");
    problem.add_css_class("error");
    problem.set_visible(false);
    join.append(&problem);
    columns.append(&join);
    column.append(&columns);

    // The party you are in.
    let party = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let heading = gtk::Label::new(Some(&t::together_in_party()));
    heading.add_css_class("title-3");
    heading.set_xalign(0.0);
    party.append(&heading);
    let party_line = widgets::haze("", "");
    party.append(&party_line);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let party_open = gtk::Button::with_label(&t::together_open_lobby());
    party_open.add_css_class("suggested-action");
    let leave = gtk::Button::with_label(&t::party_leave());
    buttons.append(&party_open);
    buttons.append(&leave);
    party.append(&buttons);
    party.set_visible(false);
    column.append(&party);

    // Recent rooms.
    let recent_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let heading = gtk::Label::new(Some(&t::together_recent()));
    heading.add_css_class("title-3");
    heading.set_xalign(0.0);
    recent_box.append(&heading);
    let recent = widgets::boxed_list();
    recent_box.append(&recent);
    let recent_empty = widgets::haze(&t::together_recent_empty(), "");
    recent_box.append(&recent_empty);
    column.append(&recent_box);

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        field: field.clone(),
        problem: problem.clone(),
        party,
        party_line,
        party_open: party_open.clone(),
        recent,
        recent_empty,
    });

    start_button.connect_clicked({
        let ctx = Rc::clone(ctx);
        move |_| pick_and_start(&ctx)
    });
    let join_it = {
        let weak = Rc::downgrade(&page);
        move || {
            if let Some(page) = weak.upgrade() {
                page.join();
            }
        }
    };
    go.connect_clicked({
        let join_it = join_it.clone();
        move |_| join_it()
    });
    field.connect_activate(move |_| join_it());
    field.connect_changed({
        let problem = problem.clone();
        move |_| problem.set_visible(false)
    });
    paste.connect_clicked({
        let weak = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = weak.upgrade() {
                page.paste();
            }
        }
    });
    party_open.connect_clicked({
        let ctx = Rc::clone(ctx);
        move |_| {
            let playing = ctx.hub.known.borrow().party.as_ref().is_some_and(|p| p.playing);
            if playing {
                ctx.hub.send(Command::Party(PartyCommand::OpenPlayer));
            } else {
                lobby::push(&ctx, None);
            }
        }
    });
    leave.connect_clicked({
        let ctx = Rc::clone(ctx);
        move |_| ctx.hub.send(Command::PartyLeave)
    });

    // Ctrl+V anywhere on the page, outside a field, pastes into Join.
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed({
        let weak = Rc::downgrade(&page);
        move |_, key, _, state| {
            let Some(page) = weak.upgrade() else { return gtk::glib::Propagation::Proceed };
            let editing = gtk::prelude::GtkWindowExt::focus(&page.ctx.window)
                .is_some_and(|w| w.is::<gtk::Text>() || w.is::<gtk::Editable>());
            if !editing
                && key == gtk::gdk::Key::v
                && state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
            {
                page.paste();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        }
    });
    column.add_controller(keys);

    page.refresh();
    let weak = Rc::downgrade(&page);
    ctx.hub.listen(move |event| {
        let Some(page) = weak.upgrade() else { return };
        if matches!(
            event,
            Event::PartyJoined(_)
                | Event::PartyState(_)
                | Event::PartyMembers(_)
                | Event::PartyLeft { .. }
                | Event::PartyRecent(_)
                | Event::PartyGlimpse(_)
                | Event::PartyLookFailed { .. }
        ) {
            page.refresh();
        }
    });
    ctx.hub.send(Command::Party(PartyCommand::LoadRecent));
    ctx.hub.send(Command::LoadPartyForm);

    let header = adw::HeaderBar::new();
    let scroll = widgets::column_page(&column, 860);
    widgets::content_page(&t::together_title(), &header, &scroll)
}

impl Page {
    fn paste(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.ctx.window.clipboard().read_text_async(gtk::gio::Cancellable::NONE, move |text| {
            if let (Some(page), Ok(Some(text))) = (weak.upgrade(), text) {
                page.field.set_text(text.trim());
                page.field.set_position(-1);
                page.field.grab_focus();
            }
        });
    }

    /// Join what is in the field: a link or a code.
    fn join(&self) {
        let text = self.field.text();
        let Some(target) = PartyTarget::parse(&text) else {
            let words = if text.trim().is_empty() {
                t::party_needs_room()
            } else {
                t::party_code_shape(EXAMPLE_CODE)
            };
            self.problem.set_label(&words);
            self.problem.set_visible(true);
            self.field.grab_focus();
            return;
        };
        self.field.set_text("");
        open_target(&self.ctx, &target);
    }

    fn refresh(&self) {
        let (party, members, recent, glimpses) = {
            let known = self.ctx.hub.known.borrow();
            (
                known.party.clone(),
                known.party_members.clone(),
                known.party_recent.clone(),
                known.party_glimpses.clone(),
            )
        };
        self.party.set_visible(party.is_some());
        if let Some(view) = &party {
            let room = view.room_episode.as_ref();
            let members = members.unwrap_or_default();
            let ready = members.iter().filter(|m| m.ready == Some(true)).count();
            let title = room.map(|r| r.title.clone()).unwrap_or_default();
            let episode = room.map_or_else(String::new, |r| r.episode.clone());
            let line = t::together_in_party_line(&episode, members.len() as i64, ready as i64);
            self.party_line.set_label(&if title.is_empty() {
                line
            } else {
                format!("{title} · {line}") // i18n-skip
            });
            self.party_open.set_label(&if view.playing {
                t::together_open_player()
            } else {
                t::together_open_lobby()
            });
        }
        while let Some(child) = self.recent.first_child() {
            self.recent.remove(&child);
        }
        self.recent_empty.set_visible(recent.is_empty());
        self.recent.set_visible(!recent.is_empty());
        for room in &recent {
            self.recent.append(&recent_row(&self.ctx, room, &glimpses));
        }
    }
}

/// A recent room's row: its title, "Episode 8 · with Mika and Ren · 2 days ago", and Rejoin.
/// Selecting it looks in, and the line says who is there.
fn recent_row(
    ctx: &Rc<Ctx>,
    room: &PartyRecentRoom,
    glimpses: &std::collections::HashMap<(String, String), kaeri_model::party::PartyGlimpse>,
) -> adw::ActionRow {
    let when = {
        let now = gtk::glib::DateTime::now_utc().map_or(room.last_at, |n| n.to_unix());
        crate::say::ago((now - room.last_at).max(0))
    };
    let names = room.names.join(", ");
    let subtitle = match glimpses.get(&(room.server.clone(), room.room.clone())) {
        Some(glimpse) => {
            let others: Vec<String> =
                glimpse.members.iter().filter(|m| !m.you).map(|m| m.name.clone()).collect();
            if others.is_empty() {
                t::together_nobody_here()
            } else {
                t::together_someone_here(&others.join(", "), others.len() as i64)
            }
        }
        None => format!(
            "{} · {} · {}",
            t::party_form_episode(crate::party::number(&room.episode)),
            names,
            when
        ), // i18n-skip
    };
    let row = adw::ActionRow::builder()
        .title(gtk::glib::markup_escape_text(&room.title))
        .subtitle(gtk::glib::markup_escape_text(&subtitle))
        .activatable(true)
        .build();
    let rejoin = gtk::Button::with_label(&t::together_rejoin());
    rejoin.set_valign(gtk::Align::Center);
    let forget = gtk::Button::from_icon_name("user-trash-symbolic");
    forget.add_css_class("flat");
    forget.set_valign(gtk::Align::Center);
    forget.set_tooltip_text(Some(&t::together_forget_room()));
    forget.update_property(&[gtk::accessible::Property::Label(&t::together_forget_room())]);
    row.add_suffix(&rejoin);
    row.add_suffix(&forget);
    let (server, name) = (room.server.clone(), room.room.clone());
    row.connect_activated({
        let (ctx, server, name) = (Rc::clone(ctx), server.clone(), name.clone());
        move |_| {
            ctx.hub.send(Command::Party(PartyCommand::LookIn {
                server: server.clone(),
                room: name.clone(),
                name: None,
            }));
        }
    });
    rejoin.connect_clicked({
        let (ctx, server, name) = (Rc::clone(ctx), server.clone(), name.clone());
        move |_| {
            open_room(&ctx, &server, &name);
        }
    });
    forget.connect_clicked({
        let ctx = Rc::clone(ctx);
        move |_| {
            ctx.hub.send(Command::Party(PartyCommand::ForgetRoom {
                server: server.clone(),
                room: name.clone(),
            }));
        }
    });
    row
}

/// A link or code, opened as the lobby of the room it names.
pub fn open_target(ctx: &Rc<Ctx>, target: &PartyTarget) {
    look_into(
        ctx,
        lobby::Looking {
            server: target.server.clone(),
            room: target.room.clone(),
            by: target.by.clone(),
            title: target.title.clone(),
            episode: target.episode.clone(),
        },
    );
}

/// The room's lobby, looking in: the engine reads who is there and what it watches and leaves;
/// only Join watch party joins.
pub fn open_room(ctx: &Rc<Ctx>, server: &str, room: &str) {
    look_into(
        ctx,
        lobby::Looking {
            server: server.to_owned(),
            room: room.to_owned(),
            by: None,
            title: None,
            episode: None,
        },
    );
}

fn look_into(ctx: &Rc<Ctx>, looking: lobby::Looking) {
    let in_it = !crate::party::is_looking(
        ctx.hub.known.borrow().party.as_ref(),
        &looking.server,
        &looking.room,
    );
    if !in_it {
        ctx.hub.send(Command::Party(PartyCommand::LookIn {
            server: looking.server.clone(),
            room: looking.room.clone(),
            name: None,
        }));
    }
    lobby::push(ctx, Some(looking));
}

/// Start a party: what are you watching, then the lobby, with the server and name asked the
/// first time only.
fn pick_and_start(ctx: &Rc<Ctx>) {
    let ctx2 = Rc::clone(ctx);
    pick(ctx, Rc::new(move |id, episode| start_with(&ctx2, id, episode)));
}

fn start_with(ctx: &Rc<Ctx>, id: AnilistId, episode: String) {
    let form = ctx.hub.known.borrow().party_form.clone();
    let Some(form) = form else { return };
    let server = form.recent_servers.first().cloned();
    if let (Some(server), false) = (server, form.name.trim().is_empty()) {
        begin(ctx, &server, &form.room, Some(form.name.clone()), id, episode);
        return;
    }
    first_time(ctx, form, id, episode);
}

fn begin(
    ctx: &Rc<Ctx>,
    server: &str,
    room: &str,
    name: Option<String>,
    id: AnilistId,
    episode: String,
) {
    ctx.hub.send(Command::Party(PartyCommand::Lobby {
        server: server.to_owned(),
        room: room.to_owned(),
        name,
        pick: Some(PartyEpisode { id, episode }),
    }));
    lobby::push(ctx, None);
}

/// The server step, first time only: the server (with the public one to use), and your name.
fn first_time(
    ctx: &Rc<Ctx>,
    form: kaeri_model::party::PartyForm,
    id: AnilistId,
    episode: String,
) {
    let dialog = adw::Dialog::new();
    dialog.set_title(&t::party_first_server_title());
    dialog.set_content_width(460);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();
    let server = adw::EntryRow::builder().title(t::party_server()).build();
    let name = adw::EntryRow::builder().title(t::party_your_name()).build();
    name.set_text(&form.name);
    group.add(&server);
    group.set_description(Some(&t::party_server_suggest(
        &form.suggested_server,
        &form.suggested_run_by,
    )));
    let use_it = gtk::Button::with_label(&t::party_use_it());
    use_it.set_valign(gtk::Align::Center);
    use_it.add_css_class("flat");
    server.add_suffix(&use_it);
    use_it.connect_clicked({
        let (server, suggested) = (server.clone(), form.suggested_server.clone());
        move |_| server.set_text(&suggested)
    });
    group.add(&name);
    page.add(&group);
    let go = adw::ButtonRow::builder().title(t::party_start_party()).build();
    go.add_css_class("suggested-action");
    let last = adw::PreferencesGroup::new();
    last.add(&go);
    page.add(&last);
    view.set_content(Some(&page));
    dialog.set_child(Some(&view));
    go.connect_activated({
        let (ctx, dialog, room) = (Rc::clone(ctx), dialog.clone(), form.room.clone());
        move |_| {
            let typed = server.text().trim().to_owned();
            if typed.is_empty() {
                return;
            }
            let who = name.text().trim().to_owned();
            dialog.close();
            begin(
                &ctx,
                &typed,
                &room,
                Some(who).filter(|w| !w.is_empty()),
                id,
                episode.clone(),
            );
        }
    });
    dialog.present(Some(&ctx.window));
}

/// What are you watching: a search field, the titles you are partway through with each next
/// episode, then search results. Choosing one calls `chosen` with the title and episode.
pub fn pick(ctx: &Rc<Ctx>, chosen: Rc<dyn Fn(AnilistId, String)>) {
    let dialog = adw::Dialog::new();
    dialog.set_title(&t::party_pick_title());
    dialog.set_content_width(460);
    dialog.set_content_height(520);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(6);
    column.set_margin_bottom(12);
    column.set_margin_start(12);
    column.set_margin_end(12);
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some(&t::party_pick_search()));
    column.append(&search);
    let list = widgets::boxed_list();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&list)
        .build();
    column.append(&scroll);
    view.set_content(Some(&column));
    dialog.set_child(Some(&view));

    let shown: Rc<RefCell<Vec<Entry>>> = Rc::default();
    let fill = {
        let (list, shown, chosen, dialog) =
            (list.clone(), Rc::clone(&shown), Rc::clone(&chosen), dialog.clone());
        Rc::new(move |entries: Vec<Entry>| {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            for entry in &entries {
                let next = entry.progress.map_or(1, |p| p.next.max(1));
                let row = adw::ActionRow::builder()
                    .title(gtk::glib::markup_escape_text(&entry.title))
                    .subtitle(t::party_pick_next(&next.to_string()))
                    .activatable(true)
                    .build();
                row.connect_activated({
                    let (chosen, dialog, id) = (Rc::clone(&chosen), dialog.clone(), entry.id);
                    move |_| {
                        dialog.close();
                        chosen(id, next.to_string());
                    }
                });
                list.append(&row);
            }
            *shown.borrow_mut() = entries;
        })
    };
    // What you are watching, to begin with.
    let mut continuing: Vec<Entry> = ctx
        .hub
        .known
        .borrow()
        .entries
        .values()
        .filter(|e| e.progress.is_some())
        .cloned()
        .collect();
    continuing.sort_by(|a, b| a.title.cmp(&b.title));
    fill(continuing.clone());

    // Then what the search finds, from the engine's own search.
    let id = ctx.hub.listen({
        let (fill, search) = (Rc::clone(&fill), search.clone());
        move |event| {
            if let Event::Content { view: ContentFor::Search { query }, content } = event
                && query.text == search.text().as_str()
                && !query.text.is_empty()
            {
                fill(content.entries().to_vec());
            }
        }
    });
    search.connect_search_changed({
        let (ctx, fill) = (Rc::clone(ctx), Rc::clone(&fill));
        move |field| {
            let text = field.text().to_string();
            if text.trim().is_empty() {
                fill(continuing.clone());
            } else {
                ctx.hub.send(Command::Search {
                    query: SearchQuery { text, ..SearchQuery::default() },
                });
            }
        }
    });
    dialog.connect_closed({
        let ctx = Rc::clone(ctx);
        move |_| ctx.hub.forget(id)
    });
    dialog.present(Some(&ctx.window));
    search.grab_focus();
}
