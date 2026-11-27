//! Watch together: the place in the main window. What it is, your room while you are in one
//! (Open the player, Leave…), and Join a room with an invite link. A room starts from an
//! episode, never empty: Watch together in the player or on a title's page.
//!
//! Recent rooms: the last ten, kept on this device; Rejoin joins one again without an invite.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    room::{RoomCommand, RoomLeftWhy, RoomRecent},
};

use crate::{
    room::{
        copy, dialogs,
        model::RoomView,
        place::{self, Place, PlaceAct, Recent},
        words::{self, Moves},
    },
    widgets,
    window::Ctx,
};

struct Page {
    ctx: Rc<Ctx>,
    holder: gtk::Box,
    /// The invite joined from this page, until the room answers.
    joining: RefCell<Option<String>>,
    /// The pasted invite whose relays didn't answer.
    failed: RefCell<Option<String>>,
    /// The last ten rooms, newest first, as the engine last listed them.
    recent: RefCell<Vec<RoomRecent>>,
    /// The recent room being rejoined, until the room answers.
    rejoining: RefCell<Option<String>>,
    /// The recent room whose Rejoin nobody answered.
    rejoin_failed: RefCell<Option<String>>,
    /// What the last look into each recent room found, by room id.
    here: RefCell<std::collections::HashMap<String, String>>,
    /// What the place last drew, so a tick of the room's clock doesn't rebuild it.
    shown: RefCell<Option<String>>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.set_vexpand(true);
    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        holder: holder.clone(),
        joining: RefCell::default(),
        failed: RefCell::default(),
        recent: RefCell::default(),
        rejoining: RefCell::default(),
        rejoin_failed: RefCell::default(),
        here: RefCell::default(),
        shown: RefCell::default(),
    });
    page.refresh();
    let weak = Rc::downgrade(&page);
    ctx.hub.listen(move |event| {
        let Some(page) = weak.upgrade() else { return };
        match event {
            Event::RoomJoined(_) => {
                page.joining.replace(None);
                page.failed.replace(None);
                page.rejoining.replace(None);
                page.rejoin_failed.replace(None);
            }
            Event::RoomLeft { why } => {
                let joined = page.joining.take();
                let rejoined = page.rejoining.take();
                let failed =
                    matches!(why, RoomLeftWhy::Unreachable | RoomLeftWhy::Failed { .. });
                if failed && joined.is_some() {
                    page.failed.replace(joined);
                }
                if failed && rejoined.is_some() {
                    page.rejoin_failed.replace(rejoined);
                }
                // A room left is a recent room now, or a newer one.
                page.ctx.hub.send(Command::Room(RoomCommand::LoadRecent));
            }
            Event::RoomLookFailed { room_id, .. } => {
                if page.rejoining.borrow().as_deref() == Some(room_id.as_str()) {
                    page.rejoining.replace(None);
                    page.rejoin_failed.replace(Some(room_id.clone()));
                }
            }
            // A look into a recent room: who is there now. A failed Rejoin's row returns to
            // Rejoin on the next look-in.
            Event::RoomGlimpse(glimpse) => {
                // The invite's look came back: it reached the room, so it didn't fail.
                let Some(room_id) = &glimpse.room_id else {
                    page.joining.replace(None);
                    return;
                };
                let names: Vec<&str> = glimpse
                    .members
                    .iter()
                    .filter(|m| !m.you)
                    .map(|m| m.name.as_str())
                    .collect();
                page.here.borrow_mut().insert(room_id.clone(), copy::here(&names));
                if page.rejoin_failed.borrow().as_deref() == Some(room_id.as_str()) {
                    page.rejoin_failed.replace(None);
                }
            }
            Event::RoomRecent(rooms) => {
                page.recent.replace(rooms.clone());
            }
            Event::RoomState(_) | Event::RoomMembers(_) => {}
            _ => return,
        }
        page.refresh();
    });
    ctx.hub.send(Command::Room(RoomCommand::LoadRecent));
    let header = adw::HeaderBar::new();
    widgets::content_page(&t::together_title(), &header, &holder)
}

impl Page {
    fn now_ms() -> i64 {
        gtk::glib::real_time() / 1000
    }

    /// The place as the room stands, rebuilt only when what it says changed.
    fn refresh(self: &Rc<Self>) {
        let room = {
            let known = self.ctx.hub.known.borrow();
            known.room.as_ref().map(|view| RoomView::from_engine(view, &known.room_members))
        };
        let now = Self::now_ms();
        let failed = self.rejoin_failed.borrow().clone();
        let recent = self
            .recent
            .borrow()
            .iter()
            .map(|r| Recent {
                title: r.title.clone(),
                line: recent_line(r, now / 1000),
                cover: r.cover_url.clone(),
                here: self.here.borrow().get(&r.room_id).cloned(),
                failed: failed.as_deref() == Some(r.room_id.as_str()),
            })
            .collect();
        let place = Place { room, join_failed: self.failed.borrow().clone(), recent };
        let key = shown_key(&place, now);
        if self.shown.borrow().as_ref() == Some(&key) {
            return;
        }
        self.shown.replace(Some(key));
        while let Some(child) = self.holder.first_child() {
            self.holder.remove(&child);
        }
        let weak = Rc::downgrade(self);
        let content = place::build(&self.ctx.hub, &place, now, move |act| {
            if let Some(page) = weak.upgrade() {
                page.act(act);
            }
        });
        content.set_vexpand(true);
        self.holder.append(&content);
    }

    fn act(self: &Rc<Self>, act: PlaceAct) {
        match act {
            PlaceAct::OpenPlayer => self.ctx.hub.send(Command::Room(RoomCommand::OpenPlayer)),
            PlaceAct::Leave => {
                let ctx = Rc::clone(&self.ctx);
                dialogs::leave(&self.ctx.window, false, move |answer| {
                    if answer == dialogs::LeaveAnswer::Leave {
                        ctx.hub.send(Command::Room(RoomCommand::Leave));
                    }
                });
            }
            PlaceAct::Join(link) => {
                let link = link.trim().to_owned();
                if link.is_empty() {
                    return;
                }
                // Only an invite the room will look at waits for its answer; the window says
                // what is wrong with any other.
                if joins(&link) {
                    self.joining.replace(Some(link.clone()));
                }
                if self.failed.take().is_some() {
                    self.shown.replace(None);
                    self.refresh();
                }
                let _ = WidgetExt::activate_action(
                    &self.ctx.window,
                    "win.join-room",
                    Some(&link.to_variant()),
                );
            }
            PlaceAct::Rejoin(i) => {
                let Some(room_id) = self.recent.borrow().get(i).map(|r| r.room_id.clone())
                else {
                    return;
                };
                self.rejoining.replace(Some(room_id.clone()));
                if self.rejoin_failed.take().is_some() {
                    self.shown.replace(None);
                    self.refresh();
                }
                self.ctx.hub.send(Command::Room(RoomCommand::Rejoin { room_id }));
            }
            PlaceAct::LookIn(i) => {
                if let Some(room_id) = self.recent.borrow().get(i).map(|r| r.room_id.clone()) {
                    self.ctx.hub.send(Command::Room(RoomCommand::LookIn { room_id }));
                }
            }
            PlaceAct::Forget(i) => {
                let Some(room_id) = self.recent.borrow().get(i).map(|r| r.room_id.clone())
                else {
                    return;
                };
                self.here.borrow_mut().remove(&room_id);
                self.ctx.hub.send(Command::Room(RoomCommand::ForgetRoom { room_id }));
                self.ctx.hub.send(Command::Room(RoomCommand::LoadRecent));
            }
        }
    }
}

/// Whether `link` is an invite the window looks into, rather than one it turns away.
fn joins(link: &str) -> bool {
    kaeri_model::room::RoomTarget::parse(link).is_some_and(|t| t.old.is_none())
}

/// What the place says: the room's line, its people and the failed invite.
fn shown_key(place: &Place, now_ms: i64) -> String {
    let room = place.room.as_ref().map(|view| {
        let line = words::line(view, &Moves::default(), now_ms);
        let people: Vec<String> = words::ordered(view)
            .into_iter()
            .map(|m| format!("{}={}", m.name, words::face(view, m, now_ms).word)) // i18n-skip
            .collect();
        format!("{}|{}|{}|{}", view.title, line.title, line.sub, people.join(",")) // i18n-skip
    });
    format!("{room:?}|{:?}|{:?}", place.join_failed, place.recent) // i18n-skip
}

/// "Episode 4 · with Mika and Ren · 19:25": when is the time today, the weekday this week, and
/// the date before that.
fn recent_line(room: &RoomRecent, now_s: i64) -> String {
    let when = when(room.last_at, now_s);
    if room.names.is_empty() {
        t::room_recent_row_alone(&room.episode, &when)
    } else {
        t::room_recent_row(
            &room.episode,
            &words::join_names(&room.names.iter().map(String::as_str).collect::<Vec<_>>()),
            &when,
        )
    }
}

fn when(at_s: i64, now_s: i64) -> String {
    use gtk::glib::DateTime;
    let (Ok(at), Ok(now)) = (DateTime::from_unix_local(at_s), DateTime::from_unix_local(now_s))
    else {
        return String::new();
    };
    let days = now.ymd() != at.ymd();
    let format = if !days {
        "%H:%M" // i18n-skip
    } else if (now_s - at_s) < 6 * 86_400 {
        "%A" // i18n-skip
    } else {
        "%-d %B" // i18n-skip
    };
    at.format(format).map(|s| s.to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::joins;

    #[test]
    fn only_a_room_invite_waits_for_an_answer() {
        assert!(!joins("not a link"));
        assert!(!joins("https://example.example/room"));
    }
}
