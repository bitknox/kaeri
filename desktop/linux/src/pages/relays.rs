//! Settings ▸ Playback ▸ Watch together ▸ Relays: the relays new rooms use, the spares a room
//! swaps in, and Restore the defaults. The list for new rooms keeps at least one relay; the
//! spares may all go. A room in progress keeps its own list, edited in its Details.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    room::{RoomCommand, RoomRelaySettings, valid_relay},
};

use crate::{widgets, window::Ctx};

/// The relays new rooms use: the ones chosen in Settings, or the defaults.
pub fn for_new_rooms(s: &RoomRelaySettings) -> &[String] {
    if s.chosen.is_empty() { &s.defaults } else { &s.chosen }
}

/// `list` with `url` gone; `None` when it would leave `list` empty and `keep_one` is set.
fn without(list: &[String], url: &str, keep_one: bool) -> Option<Vec<String>> {
    let rest: Vec<String> = list.iter().filter(|u| *u != url).cloned().collect();
    (!(keep_one && rest.is_empty()) && rest.len() < list.len()).then_some(rest)
}

/// `list` with the typed address added at its end; `None` for one that isn't a relay's
/// address or is already there.
fn with(list: &[String], typed: &str) -> Option<Vec<String>> {
    let url = typed.trim();
    if !valid_relay(url) || list.iter().any(|u| u.eq_ignore_ascii_case(url)) {
        return None;
    }
    let mut list = list.to_vec();
    list.push(url.to_owned());
    Some(list)
}

/// Whether anything differs from the built-in relays and spares.
fn changed(s: &RoomRelaySettings) -> bool {
    !s.chosen.is_empty() && s.chosen != s.defaults || s.spares != s.default_spares
}

/// What a relay's row shows: its address without the usual scheme.
fn shown(url: &str) -> &str {
    url.strip_prefix("wss://").unwrap_or(url) // i18n-skip
}

/// The Relays row of Settings' Watch together group: how many relays new rooms use, and Edit.
pub fn row(ctx: &Rc<Ctx>) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(t::room_relays()).activatable(true).build();
    let edit = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    edit.set_valign(gtk::Align::Center);
    let word = gtk::Label::new(Some(&t::room_settings_relays_edit()));
    word.add_css_class("dim-label");
    edit.append(&word);
    edit.append(&gtk::Image::from_icon_name("go-next-symbolic"));
    row.add_suffix(&edit);
    let count = {
        let row = row.downgrade();
        move |s: &RoomRelaySettings| {
            if let Some(row) = row.upgrade() {
                let n = i64::try_from(for_new_rooms(s).len()).unwrap_or(0);
                row.set_subtitle(&t::room_settings_relays_count(n));
            }
        }
    };
    let listening = ctx.hub.listen(move |event| {
        if let Event::RoomRelays(s) = event {
            count(s);
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    row.connect_destroy(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
    let weak = Rc::downgrade(ctx);
    row.connect_activated(move |_| {
        if let Some(ctx) = weak.upgrade() {
            push(&ctx);
        }
    });
    ctx.hub.send(Command::Room(RoomCommand::LoadRelays));
    row
}

/// The Relays page, pushed over Settings.
pub fn push(ctx: &Rc<Ctx>) {
    let Some(nav) = ctx.nav() else { return };
    let hub = Rc::downgrade(&ctx.hub);
    let relays = Relays::new(move |command| {
        if let Some(hub) = hub.upgrade() {
            hub.send(Command::Room(command));
        }
    });
    let header = adw::HeaderBar::new();
    let page =
        widgets::page(&t::room_relays(), &header, &widgets::column_page(&relays.column, 640));
    let listening = ctx.hub.listen({
        let relays = Rc::clone(&relays);
        move |event| {
            if let Event::RoomRelays(s) = event {
                relays.heard(s);
            }
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    page.connect_destroy(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
    ctx.hub.send(Command::Room(RoomCommand::LoadRelays));
    nav.push(&page);
}

/// The page's content, drawn from what the engine last said.
pub struct Relays {
    pub column: gtk::Box,
    new_rooms: adw::PreferencesGroup,
    spares: adw::PreferencesGroup,
    field: gtk::Entry,
    add: gtk::Button,
    restore: gtk::Button,
    /// The rows drawn in each group, to take out when the lists change.
    drawn: RefCell<Vec<(adw::PreferencesGroup, gtk::Widget)>>,
    /// The lists the rows show, as the engine last said them.
    drawn_for: RefCell<Option<RoomRelaySettings>>,
    /// The lists as they stand with the edits sent since: each edit builds on the one before,
    /// so two quick ones don't undo each other before the engine answers.
    now: RefCell<Option<RoomRelaySettings>>,
    send: Box<dyn Fn(RoomCommand)>,
}

impl Relays {
    pub fn new(send: impl Fn(RoomCommand) + 'static) -> Rc<Self> {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
        let intro =
            gtk::Label::builder().label(t::room_relays_intro()).wrap(true).xalign(0.0).build();
        intro.add_css_class("dim-label");
        column.append(&intro);
        let new_rooms = adw::PreferencesGroup::builder()
            .title(t::room_relays_new_rooms())
            .description(t::room_relays_new_rooms_line())
            .build();
        // The address field and Add, as the list's last row.
        let field = gtk::Entry::builder().placeholder_text("wss://").hexpand(true).build(); // i18n-skip
        field.add_css_class("monospace");
        field.update_property(&[gtk::accessible::Property::Label(&t::room_relays_new_rooms())]);
        let add = gtk::Button::with_label(&t::room_relay_add());
        add.set_sensitive(false);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        line.set_margin_top(6);
        line.set_margin_bottom(6);
        line.set_margin_start(12);
        line.set_margin_end(12);
        line.append(&field);
        line.append(&add);
        let add_row = gtk::ListBoxRow::builder().activatable(false).child(&line).build();
        new_rooms.add(&add_row);
        column.append(&new_rooms);
        let spares = adw::PreferencesGroup::builder()
            .title(t::room_relays_spares())
            .description(t::room_relays_spares_line())
            .build();
        column.append(&spares);
        let restore = gtk::Button::with_label(&t::room_relays_restore());
        restore.set_halign(gtk::Align::Start);
        restore.set_sensitive(false);
        column.append(&restore);
        let me = Rc::new(Self {
            column,
            new_rooms,
            spares,
            field,
            add,
            restore,
            drawn: RefCell::default(),
            drawn_for: RefCell::default(),
            now: RefCell::default(),
            send: Box::new(send),
        });
        let weak = Rc::downgrade(&me);
        me.field.connect_changed(move |field| {
            if let Some(me) = weak.upgrade() {
                me.add.set_sensitive(me.adds(&field.text()).is_some());
            }
        });
        let weak = Rc::downgrade(&me);
        me.field.connect_activate(move |_| {
            if let Some(me) = weak.upgrade() {
                me.add_typed();
            }
        });
        let weak = Rc::downgrade(&me);
        me.add.connect_clicked(move |_| {
            if let Some(me) = weak.upgrade() {
                me.add_typed();
            }
        });
        let weak = Rc::downgrade(&me);
        me.restore.connect_clicked(move |_| {
            if let Some(me) = weak.upgrade() {
                me.restore_defaults();
            }
        });
        me
    }

    /// The list for new rooms with the typed address, when it adds one.
    fn adds(&self, typed: &str) -> Option<Vec<String>> {
        let now = self.now.borrow();
        with(for_new_rooms(now.as_ref()?), typed)
    }

    fn add_typed(&self) {
        if let Some(relays) = self.adds(&self.field.text()) {
            self.field.set_text("");
            self.set_new_rooms(relays);
        }
    }

    /// Send the list for new rooms, and build the next edit on it.
    fn set_new_rooms(&self, relays: Vec<String>) {
        if let Some(now) = self.now.borrow_mut().as_mut() {
            now.chosen.clone_from(&relays);
        }
        (self.send)(RoomCommand::SetRelays { relays });
    }

    fn set_spares(&self, relays: Vec<String>) {
        if let Some(now) = self.now.borrow_mut().as_mut() {
            now.spares.clone_from(&relays);
        }
        (self.send)(RoomCommand::SetSpareRelays { relays });
    }

    /// × on a relay: gone from the lists as they stand now, not as they were drawn.
    fn remove(&self, url: &str, spare: bool) {
        let Some(now) = self.now.borrow().clone() else { return };
        if spare {
            if let Some(rest) = without(&now.spares, url, false) {
                self.set_spares(rest);
            }
        } else if let Some(rest) = without(for_new_rooms(&now), url, true) {
            self.set_new_rooms(rest);
        }
    }

    fn restore_defaults(&self) {
        let Some(now) = self.now.borrow().clone() else { return };
        self.set_new_rooms(Vec::new());
        self.set_spares(now.default_spares);
    }

    /// What the engine says the relays are now.
    pub fn heard(self: &Rc<Self>, s: &RoomRelaySettings) {
        self.now.replace(Some(s.clone()));
        if self.drawn_for.borrow().as_ref() == Some(s) {
            return;
        }
        self.drawn_for.replace(Some(s.clone()));
        for (group, row) in self.drawn.take() {
            group.remove(&row);
        }
        let list = for_new_rooms(s).to_vec();
        // Above the address field: the field stays the list's last row.
        let field_row = self.field.ancestor(gtk::ListBoxRow::static_type());
        if let Some(field_row) = &field_row {
            self.new_rooms.remove(field_row);
        }
        for url in &list {
            let me = Rc::downgrade(self);
            let gone = url.clone();
            let row = relay_row(url, list.len() > 1, move || {
                if let Some(me) = me.upgrade() {
                    me.remove(&gone, false);
                }
            });
            self.new_rooms.add(&row);
            self.drawn.borrow_mut().push((self.new_rooms.clone(), row.upcast()));
        }
        if let Some(field_row) = &field_row {
            self.new_rooms.add(field_row);
        }
        for url in &s.spares {
            let me = Rc::downgrade(self);
            let gone = url.clone();
            let row = relay_row(url, true, move || {
                if let Some(me) = me.upgrade() {
                    me.remove(&gone, true);
                }
            });
            self.spares.add(&row);
            self.drawn.borrow_mut().push((self.spares.clone(), row.upcast()));
        }
        self.restore.set_sensitive(changed(s));
        self.add.set_sensitive(self.adds(&self.field.text()).is_some());
    }
}

/// A relay's row: its address, and × that removes it (off for the last relay for new rooms).
fn relay_row(url: &str, removable: bool, remove: impl Fn() + 'static) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(shown(url)).use_markup(false).build();
    row.add_css_class("monospace");
    let x = gtk::Button::from_icon_name("window-close-symbolic");
    x.add_css_class("flat");
    x.add_css_class("circular");
    x.set_valign(gtk::Align::Center);
    x.set_sensitive(removable);
    x.set_tooltip_text(Some(&t::room_relay_remove(url)));
    x.update_property(&[gtk::accessible::Property::Label(&t::room_relay_remove(url))]);
    x.connect_clicked(move |_| remove());
    row.add_suffix(&x);
    row
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use adw::prelude::*;
    use kaeri_model::room::{RoomCommand, RoomRelaySettings};

    use super::{Relays, changed, for_new_rooms, with, without};
    use crate::gtk_test::run;

    fn urls(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| format!("wss://{n}.example")).collect()
    }

    fn settings() -> RoomRelaySettings {
        RoomRelaySettings {
            defaults: urls(&["relay-1", "relay-2", "relay-3"]),
            spares: urls(&["spare-1", "spare-2"]),
            default_spares: urls(&["spare-1", "spare-2"]),
            chosen: Vec::new(),
        }
    }

    #[test]
    fn new_rooms_use_the_chosen_list_or_the_defaults() {
        let mut s = settings();
        assert_eq!(for_new_rooms(&s), urls(&["relay-1", "relay-2", "relay-3"]).as_slice());
        assert!(!changed(&s));
        s.chosen = urls(&["home"]);
        assert_eq!(for_new_rooms(&s), urls(&["home"]).as_slice());
        assert!(changed(&s));
    }

    #[test]
    fn the_last_relay_for_new_rooms_stays_and_spares_can_all_go() {
        let one = urls(&["home"]);
        assert_eq!(without(&one, &one[0], true), None);
        assert_eq!(without(&one, &one[0], false), Some(Vec::new()));
        let two = urls(&["a", "b"]);
        assert_eq!(without(&two, &two[0], true), Some(urls(&["b"])));
    }

    #[test]
    fn only_a_new_relay_s_address_adds() {
        let list = urls(&["a"]);
        assert_eq!(with(&list, " wss://b.example "), Some(urls(&["a", "b"])));
        assert_eq!(with(&list, "wss://A.example"), None);
        assert_eq!(with(&list, "ws://far.example"), None);
        assert_eq!(with(&list, "not a relay"), None);
        assert!(with(&list, "ws://localhost:7777").is_some());
    }

    /// Every button under `root` whose accessible label or tooltip names `url`'s removal.
    fn remove_button(root: &gtk::Widget, url: &str) -> gtk::Button {
        let want = kaeri_i18n::t::room_relay_remove(url);
        let mut stack = vec![root.clone()];
        while let Some(w) = stack.pop() {
            if let Some(b) = w.downcast_ref::<gtk::Button>()
                && b.tooltip_text().as_deref() == Some(want.as_str())
            {
                return b.clone();
            }
            let mut child = w.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                stack.push(c);
            }
        }
        panic!("no remove button for {url}");
    }

    #[test]
    fn the_page_removes_adds_and_restores() {
        run(|| {
            let sent: Rc<RefCell<Vec<RoomCommand>>> = Rc::default();
            let relays = Relays::new({
                let sent = Rc::clone(&sent);
                move |c| sent.borrow_mut().push(c)
            });
            let mut s = settings();
            relays.heard(&s);
            assert!(!relays.restore.is_sensitive());
            let root: gtk::Widget = relays.column.clone().upcast();
            // Quick edits, before the engine answers: each builds on the one before.
            remove_button(&root, &s.defaults[1]).emit_clicked();
            remove_button(&root, &s.defaults[2]).emit_clicked();
            remove_button(&root, &s.spares[0]).emit_clicked();
            relays.field.set_text("wss://home.example");
            relays.add.emit_clicked();
            assert_eq!(
                *sent.borrow(),
                vec![
                    RoomCommand::SetRelays { relays: urls(&["relay-1", "relay-3"]) },
                    RoomCommand::SetRelays { relays: urls(&["relay-1"]) },
                    RoomCommand::SetSpareRelays { relays: urls(&["spare-2"]) },
                    RoomCommand::SetRelays { relays: urls(&["relay-1", "home"]) },
                ]
            );
            // The last relay for new rooms stays, even before the rows are redrawn.
            sent.borrow_mut().clear();
            relays.set_new_rooms(urls(&["relay-1"]));
            sent.borrow_mut().clear();
            remove_button(&root, &s.defaults[0]).emit_clicked();
            assert!(sent.borrow().is_empty());
            // One relay left for new rooms: its × is off. The spares changed: Restore is on.
            s.chosen = urls(&["home"]);
            s.spares = Vec::new();
            relays.heard(&s);
            assert!(!remove_button(&root, &s.chosen[0]).is_sensitive());
            assert!(relays.restore.is_sensitive());
            sent.borrow_mut().clear();
            relays.restore.emit_clicked();
            assert_eq!(
                *sent.borrow(),
                vec![
                    RoomCommand::SetRelays { relays: Vec::new() },
                    RoomCommand::SetSpareRelays { relays: urls(&["spare-1", "spare-2"]) },
                ]
            );
        });
    }
}
