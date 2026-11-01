//! The watch party lobby: what the room is watching, whether it plays from your sources, who is
//! in the room and who is ready, the invite, and the start line. It is a page pushed on Watch
//! together, and it follows the room as the engine says things.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    party::{PartyCommand, PartyFound, PartySources, PartyView},
};

use crate::{pages::together, widgets, window::Ctx};

/// A room opened from an invite or a code, looked into and not yet joined.
#[derive(Clone)]
pub struct Looking {
    pub server: String,
    pub room: String,
    /// Who invited, from the link; absent when the room came from a code.
    pub by: Option<String>,
    /// What the link named, until the room says its own.
    pub title: Option<String>,
    pub episode: Option<String>,
}

struct Lobby {
    ctx: Rc<Ctx>,
    looking: Option<Looking>,
    /// Why the room could not be looked into, until it can.
    failed: RefCell<Option<String>>,
    body: gtk::Box,
    /// The connection line, shown while the room cannot be reached.
    banner: adw::Banner,
}

/// Show the lobby on Watch together, over the place. With `looking`, it looks into that room
/// and joins only on Join watch party.
pub fn push(ctx: &Rc<Ctx>, looking: Option<Looking>) {
    let _ = gtk::prelude::WidgetExt::activate_action(
        &ctx.window,
        "win.go",
        Some(&"together".to_variant()),
    );
    let Some(nav) = ctx.nav() else { return };
    if nav.visible_page().is_some_and(|p| p.tag().as_deref() == Some("lobby")) {
        return;
    }
    nav.push(&build(ctx, looking));
}

fn build(ctx: &Rc<Ctx>, looking: Option<Looking>) -> adw::NavigationPage {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 20);
    let banner = adw::Banner::new("");
    let lobby = Rc::new(Lobby {
        ctx: Rc::clone(ctx),
        looking,
        failed: RefCell::new(None),
        body: body.clone(),
        banner: banner.clone(),
    });
    lobby.show();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.add_top_bar(&banner);
    let scroll = widgets::column_page(&body, 940);
    view.set_content(Some(&scroll));
    let page = adw::NavigationPage::builder()
        .title(t::together_title())
        .tag("lobby")
        .child(&view)
        .build();
    // The listener holds the lobby until the page is hidden and forgets it.
    let weak_page = page.downgrade();
    let id = ctx.hub.listen(move |event| {
        if let Event::PartyLookFailed { reason, .. } = event {
            *lobby.failed.borrow_mut() = Some(crate::say::msg(reason));
            lobby.show();
        }
        if matches!(
            event,
            Event::PartyGlimpse(_)
                | Event::PartyJoined(_)
                | Event::PartyState(_)
                | Event::PartyMembers(_)
                | Event::PartyLastOut { .. }
                | Event::PartyStarted { .. }
        ) {
            lobby.show();
        }
        // The party is over: the lobby goes, wherever the window is.
        if let Event::PartyLeft { .. } = event
            && let Some(page) = weak_page.upgrade()
            && let Some(nav) = page.ancestor(adw::NavigationView::static_type())
            && let Some(nav) = nav.downcast_ref::<adw::NavigationView>()
            && nav.visible_page().is_some_and(|p| p == page)
        {
            nav.pop();
        }
    });
    let ctx2 = Rc::clone(ctx);
    page.connect_hidden(move |_| ctx2.hub.forget(id));
    page
}

impl Lobby {
    /// Looking in: the room's episode (which wins over the link's), who invited, who is in the
    /// room, and Join watch party. Nothing is joined until that button.
    fn show_looking(self: &Rc<Self>, looking: &Looking) {
        let glimpse = self
            .ctx
            .hub
            .known
            .borrow()
            .party_glimpses
            .get(&(looking.server.clone(), looking.room.clone()))
            .cloned();
        let columns = gtk::FlowBox::new();
        columns.set_selection_mode(gtk::SelectionMode::None);
        columns.set_max_children_per_line(2);
        columns.set_column_spacing(32);
        columns.set_row_spacing(24);
        columns.set_homogeneous(true);

        let left = gtk::Box::new(gtk::Orientation::Vertical, 8);
        left.set_valign(gtk::Align::Start);
        if let Some(by) = &looking.by {
            left.append(&widgets::haze(&t::party_invited_by(by), ""));
        }
        left.append(&widgets::haze(&t::party_room_watching(), "caption"));
        let room = glimpse.as_ref().and_then(|g| g.room_episode.as_ref());
        let (title, episode) = match room {
            Some(room) => (Some(room.title.clone()), Some(room.episode.clone())),
            None => (looking.title.clone(), looking.episode.clone()),
        };
        match (title, glimpse.is_some() || self.failed.borrow().is_some()) {
            (Some(title), _) => {
                let label = gtk::Label::new(Some(&title));
                label.add_css_class("title-1");
                label.set_xalign(0.0);
                label.set_wrap(true);
                left.append(&label);
                if let Some(episode) = episode {
                    let line = t::party_form_episode(crate::party::number(&episode));
                    left.append(&widgets::haze(&line, ""));
                }
            }
            (None, true) => {
                left.append(&widgets::haze(&t::party_room_nothing(), ""));
                left.append(&widgets::haze(&t::party_room_nothing_line(), "caption"));
            }
            (None, false) => left.append(&widgets::haze(&t::party_looking_in(), "")),
        }
        if let Some(why) = self.failed.borrow().as_ref() {
            left.append(&widgets::haze(why, "caption"));
        }
        let join = gtk::Button::with_label(&t::party_join());
        join.add_css_class("suggested-action");
        join.set_halign(gtk::Align::Start);
        join.set_margin_top(12);
        join.connect_clicked({
            let (ctx, looking) = (Rc::clone(&self.ctx), looking.clone());
            move |_| {
                ctx.hub.send(Command::Party(PartyCommand::Lobby {
                    server: looking.server.clone(),
                    room: looking.room.clone(),
                    name: None,
                    pick: None,
                }));
            }
        });
        left.append(&join);
        columns.append(&left);

        if let Some(glimpse) = glimpse
            && !glimpse.members.is_empty()
        {
            let right = gtk::Box::new(gtk::Orientation::Vertical, 8);
            right.set_valign(gtk::Align::Start);
            let ready = glimpse.members.iter().filter(|m| m.ready == Some(true)).count();
            let count = gtk::Label::new(Some(&t::party_lobby_count(
                ready as i64,
                glimpse.members.len() as i64,
            )));
            count.add_css_class("title-3");
            count.set_xalign(0.0);
            right.append(&count);
            right.append(&member_list(&glimpse.members));
            columns.append(&right);
        }
        self.body.append(&columns);
    }

    fn show(self: &Rc<Self>) {
        let (view, members, last_out) = {
            let known = self.ctx.hub.known.borrow();
            (
                known.party.clone(),
                known.party_members.clone().unwrap_or_default(),
                known.party_last_out.clone(),
            )
        };
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        if let Some(looking) = &self.looking
            && crate::party::is_looking(view.as_ref(), &looking.server, &looking.room)
        {
            self.banner.set_revealed(false);
            self.show_looking(looking);
            return;
        }
        let Some(view) = view else {
            self.body.append(&widgets::haze(&t::party_looking_in(), ""));
            return;
        };
        self.banner
            .set_revealed(view.connection != kaeri_model::party::PartyConnection::Connected);
        self.banner.set_title(&crate::party::connection_line(&view));

        let columns = gtk::FlowBox::new();
        columns.set_selection_mode(gtk::SelectionMode::None);
        columns.set_max_children_per_line(2);
        columns.set_column_spacing(32);
        columns.set_row_spacing(24);
        columns.set_homogeneous(true);
        columns.append(&self.left(&view, &members, last_out));
        columns.append(&self.right(&view, &members));
        self.body.append(&columns);
    }

    /// The room's episode, the sources line, and the actions.
    fn left(
        self: &Rc<Self>,
        view: &PartyView,
        members: &[kaeri_model::party::PartyMember],
        last_out: Option<(String, Option<kaeri_model::Msg>)>,
    ) -> gtk::Box {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        column.set_valign(gtk::Align::Start);
        let over = widgets::haze(&t::party_room_watching(), "caption");
        column.append(&over);
        match &view.room_episode {
            Some(room) => {
                let title = gtk::Label::new(Some(&room.title));
                title.add_css_class("title-1");
                title.set_xalign(0.0);
                title.set_wrap(true);
                column.append(&title);
                let line = t::party_form_episode(crate::party::number(&room.episode));
                column.append(&widgets::haze(&line, ""));
                match room.found {
                    PartyFound::Name => {
                        column
                            .append(&widgets::haze(&t::party_room_found_by_name(), "caption"));
                    }
                    PartyFound::FileName => {
                        column.append(&widgets::haze(&t::party_room_matched(), "caption"));
                        let not = gtk::Button::with_label(&t::party_room_not_this());
                        not.add_css_class("flat");
                        not.set_halign(gtk::Align::Start);
                        let ctx = Rc::clone(&self.ctx);
                        not.connect_clicked(move |_| {
                            ctx.hub
                                .send(Command::Party(PartyCommand::ChooseTitle { id: None }));
                        });
                        column.append(&not);
                    }
                    PartyFound::Unmatched => {
                        if let Some(by) = &room.plain_by {
                            column.append(&widgets::haze(
                                &t::party_room_plain_file(by, &room.file),
                                "",
                            ));
                        }
                        for guess in &view.guesses {
                            let button = gtk::Button::with_label(&guess.title);
                            button.add_css_class("flat");
                            button.set_halign(gtk::Align::Start);
                            let (ctx, id) = (Rc::clone(&self.ctx), guess.id);
                            button.connect_clicked(move |_| {
                                ctx.hub.send(Command::Party(PartyCommand::ChooseTitle {
                                    id: Some(id),
                                }));
                            });
                            column.append(&button);
                        }
                    }
                    PartyFound::Tag => {}
                }
            }
            None => {
                let title = widgets::haze(&t::party_room_nothing(), "");
                column.append(&title);
                column.append(&widgets::haze(&t::party_room_nothing_line(), "caption"));
                let pick = gtk::Button::with_label(&t::party_room_pick());
                pick.add_css_class("suggested-action");
                pick.set_halign(gtk::Align::Start);
                let ctx = Rc::clone(&self.ctx);
                pick.connect_clicked(move |_| change(&ctx));
                column.append(&pick);
            }
        }
        // The sources line.
        if let Some(sources) = &view.sources {
            let line = match sources {
                PartySources::Checking => t::party_looking_sources(),
                PartySources::Found => t::party_lobby_sources_found(),
                PartySources::Missing { reason, .. } => crate::say::msg(reason),
            };
            column.append(&widgets::haze(&line, "caption"));
            if let PartySources::Missing { .. } = sources {
                let open = gtk::Button::with_label(&t::party_open_sources());
                open.add_css_class("flat");
                open.set_halign(gtk::Align::Start);
                open.set_action_name(Some("win.go"));
                open.set_action_target_value(Some(&"providers".to_variant()));
                column.append(&open);
            }
        }
        if let Some((episode, airs)) = last_out {
            let line = match airs {
                Some(airs) => t::party_last_out_airs(&episode, &crate::say::msg(&airs)),
                None => t::party_last_out(),
            };
            column.append(&widgets::haze(&line, ""));
        }

        // Ready, Start anyway, Change episode, Leave.
        let me_ready = view.ready;
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_margin_top(12);
        let ready = gtk::Button::with_label(&if me_ready {
            format!("{} \u{2713}", t::party_member_ready()) // i18n-skip
        } else {
            t::party_ready_action()
        });
        if !me_ready {
            ready.add_css_class("suggested-action");
        }
        ready.connect_clicked({
            let ctx = Rc::clone(&self.ctx);
            move |_| ctx.hub.send(Command::PartyReady { ready: !me_ready })
        });
        actions.append(&ready);
        if view.room_episode.is_some() {
            let other = gtk::Button::with_label(&t::party_change_episode());
            other.connect_clicked({
                let ctx = Rc::clone(&self.ctx);
                move |_| change(&ctx)
            });
            actions.append(&other);
        }
        let leave = gtk::Button::with_label(&t::party_leave());
        leave.connect_clicked({
            let ctx = Rc::clone(&self.ctx);
            move |_| ctx.hub.send(Command::PartyLeave)
        });
        actions.append(&leave);
        column.append(&actions);

        // The start line, and Start anyway.
        let others = members.len() > 1;
        let line = match view.starting_in {
            Some(seconds) => t::party_starting(i64::from(seconds)),
            None if !others => t::party_alone(),
            None => crate::party::ready_line(members),
        };
        column.append(&widgets::haze(&line, "caption"));
        if view.can_start_anyway && others {
            let anyway = gtk::Button::with_label(&t::party_start_anyway());
            anyway.add_css_class("flat");
            anyway.set_halign(gtk::Align::Start);
            anyway.connect_clicked({
                let ctx = Rc::clone(&self.ctx);
                move |_| ctx.hub.send(Command::Party(PartyCommand::StartAnyway))
            });
            column.append(&anyway);
        }
        column
    }

    /// In the room: the count and the members, then the invite.
    fn right(
        self: &Rc<Self>,
        view: &PartyView,
        members: &[kaeri_model::party::PartyMember],
    ) -> gtk::Box {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        column.set_valign(gtk::Align::Start);
        let ready = members.iter().filter(|m| m.ready == Some(true)).count();
        let count =
            gtk::Label::new(Some(&t::party_lobby_count(ready as i64, members.len() as i64)));
        count.add_css_class("title-3");
        count.set_xalign(0.0);
        column.append(&count);
        column.append(&member_list(members));

        // The invite.
        let invite = gtk::Label::new(Some(&t::party_invite()));
        invite.add_css_class("title-3");
        invite.set_xalign(0.0);
        invite.set_margin_top(12);
        column.append(&invite);
        if let Some(code) = &view.code {
            let label = gtk::Label::new(Some(code));
            label.add_css_class("monospace");
            label.add_css_class("title-1");
            label.set_xalign(0.0);
            label.set_selectable(true);
            column.append(&label);
        }
        if let Some(link) = view.link.clone().or_else(|| view.invite.clone()) {
            let copy = gtk::Button::with_label(&t::party_copy_invite());
            copy.set_halign(gtk::Align::Start);
            copy.connect_clicked({
                let ctx = Rc::clone(&self.ctx);
                move |_| {
                    ctx.window.clipboard().set_text(&link);
                    ctx.toast(&t::party_invite_copied());
                }
            });
            column.append(&copy);
        }
        column.append(&widgets::haze(&format!("{} · {}", view.server, view.room), "caption")); // i18n-skip
        column
    }
}

/// The member rows: a badge with the first letter, the name and what they are doing.
fn member_list(members: &[kaeri_model::party::PartyMember]) -> gtk::ListBox {
    let list = widgets::boxed_list();
    for member in members {
        let row = adw::ActionRow::builder()
            .title(gtk::glib::markup_escape_text(&crate::party::member_name(member)))
            .subtitle(gtk::glib::markup_escape_text(&crate::party::member_state(member)))
            .build();
        let initial = member.name.chars().next().map(|c| c.to_uppercase().to_string());
        let badge = gtk::Label::new(initial.as_deref());
        badge.add_css_class("heading");
        row.add_prefix(&badge);
        list.append(&row);
    }
    list
}

/// Change episode, or pick what the room watches when it watches nothing: the room's playlist
/// is written again and every lobby follows.
fn change(ctx: &Rc<Ctx>) {
    let sender = Rc::clone(ctx);
    together::pick(
        ctx,
        Rc::new(move |id, episode| {
            sender.hub.send(Command::Party(PartyCommand::ChangeEpisode { id, episode }));
        }),
    );
}
