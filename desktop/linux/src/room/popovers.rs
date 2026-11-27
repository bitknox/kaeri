//! The column's two popovers. Invite: the link that carries the room's key, Copy link, New
//! invite, and Move to a new room under them. Details: encryption and the room's relays, each
//! with how it answers, and Edit relays, which changes this room's list for everyone in it.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;

use super::{column::Act, copy as c, model::RoomView};

const WIDTH: i32 = 300;

fn note(text: &str) -> gtk::Label {
    let l = crate::widgets::haze(text, "caption");
    l.set_max_width_chars(36);
    l.set_width_chars(36);
    l
}

fn small(label: &str) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.set_halign(gtk::Align::Start);
    b
}

pub struct Invite {
    pub popover: gtk::Popover,
    link: gtk::Label,
    copy: gtk::Button,
}

impl Invite {
    pub fn new(act: impl Fn(Act) + 'static) -> Self {
        let act = Rc::new(act);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_width_request(WIDTH);
        body.set_margin_top(6);
        body.set_margin_bottom(6);
        body.set_margin_start(6);
        body.set_margin_end(6);
        body.append(&crate::widgets::heading(&c::room_invite()));
        body.append(&note(&c::invite_body()));
        let link = gtk::Label::new(None);
        link.add_css_class("monospace");
        link.add_css_class("caption");
        link.add_css_class("room-link");
        link.set_xalign(0.0);
        link.set_hexpand(true);
        link.set_selectable(true);
        link.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        link.set_width_chars(1);
        link.set_max_width_chars(24);
        let copy = gtk::Button::with_label(&c::copy_link());
        copy.add_css_class("suggested-action");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&link);
        row.append(&copy);
        body.append(&row);
        let renew = small(&c::new_invite());
        body.append(&renew);
        body.append(&note(&c::new_invite_note()));
        body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let mv = small(&c::move_room_button());
        body.append(&mv);
        body.append(&note(&c::move_room_note()));
        let popover = gtk::Popover::new();
        popover.set_child(Some(&body));
        popover.add_css_class("room-popover");
        copy.connect_clicked({
            let act = Rc::clone(&act);
            let copy = copy.clone();
            move |_| {
                act(Act::CopyLink);
                copy.set_label(&c::link_copied());
                copy.announce(&c::link_copied(), gtk::AccessibleAnnouncementPriority::Medium);
            }
        });
        // Copied says so until the popover closes; it opens again on Copy link.
        popover.connect_closed({
            let copy = copy.clone();
            move |_| copy.set_label(&c::copy_link())
        });
        renew.connect_clicked({
            let act = Rc::clone(&act);
            move |_| act(Act::NewInvite)
        });
        mv.connect_clicked({
            let popover = popover.clone();
            move |_| {
                popover.popdown();
                act(Act::MoveRoom);
            }
        });
        Self { popover, link, copy }
    }

    pub fn show(&self, view: &RoomView) {
        let link = view.link.as_deref().unwrap_or("");
        let shown = link.trim_start_matches("https://");
        // A new invite is a new link: Copy link again.
        if self.link.label() != shown {
            self.link.set_label(shown);
            self.copy.set_label(&c::copy_link());
        }
    }
}

pub struct Details {
    pub popover: gtk::Popover,
    stack: gtk::Stack,
    answering: gtk::Label,
    relays: gtk::ListBox,
    edit_list: gtk::ListBox,
    edited: Rc<RefCell<Vec<String>>>,
    /// The relays as last drawn, so a tick of the room's clock doesn't rebuild the rows.
    drawn: RefCell<Vec<(String, bool, Option<u32>)>>,
}

impl Details {
    pub fn new(act: impl Fn(Act) + 'static) -> Self {
        let act = Rc::new(act);
        let pad = |b: &gtk::Box| {
            b.set_width_request(WIDTH);
            b.set_margin_top(6);
            b.set_margin_bottom(6);
            b.set_margin_start(6);
            b.set_margin_end(6);
        };
        // Details.
        let info = gtk::Box::new(gtk::Orientation::Vertical, 8);
        pad(&info);
        info.append(&crate::widgets::heading(&c::room_details()));
        let pair = |k: &str, v: &gtk::Label| {
            let r = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let key = crate::widgets::haze(k, "");
            key.set_hexpand(true);
            r.append(&key);
            r.append(v);
            r
        };
        info.append(&pair(&c::encryption(), &gtk::Label::new(Some(&c::end_to_end()))));
        let answering = gtk::Label::new(None);
        info.append(&pair(&c::relays(), &answering));
        let relays = crate::widgets::boxed_list();
        info.append(&relays);
        info.append(&note(&c::relays_note()));
        let edit = small(&c::edit_relays_button());
        info.append(&edit);

        // Edit relays.
        let editor = gtk::Box::new(gtk::Orientation::Vertical, 8);
        pad(&editor);
        editor.append(&crate::widgets::heading(&c::edit_relays()));
        editor.append(&note(&c::edit_relays_body()));
        let edit_list = crate::widgets::boxed_list();
        editor.append(&edit_list);
        let field = gtk::Entry::builder().placeholder_text("wss://").hexpand(true).build(); // i18n-skip
        field.add_css_class("monospace");
        let add = gtk::Button::with_label(&c::add());
        let add_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        add_row.append(&field);
        add_row.append(&add);
        editor.append(&add_row);
        editor.append(&note(&c::relays_settings_note()));
        let done = gtk::Button::with_label(&c::done());
        done.add_css_class("suggested-action");
        done.set_halign(gtk::Align::End);
        editor.append(&done);

        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_vhomogeneous(false);
        stack.set_interpolate_size(true);
        stack.add_named(&info, Some("info"));
        stack.add_named(&editor, Some("edit"));
        let popover = gtk::Popover::new();
        popover.set_child(Some(&stack));
        popover.add_css_class("room-popover");
        popover.connect_closed({
            let stack = stack.clone();
            move |_| stack.set_visible_child_name("info")
        });
        let edited: Rc<RefCell<Vec<String>>> = Rc::default();
        edit.connect_clicked({
            let stack = stack.clone();
            move |_| stack.set_visible_child_name("edit")
        });
        let me = Self {
            popover,
            stack,
            answering,
            relays,
            edit_list,
            edited,
            drawn: RefCell::default(),
        };
        add.connect_clicked({
            let edited = Rc::clone(&me.edited);
            let list = me.edit_list.clone();
            move |_| {
                let url = field.text().trim().to_owned();
                if url.starts_with("wss://") && url.len() > 6 {
                    // i18n-skip
                    edited.borrow_mut().push(url);
                    fill_edit(&list, &edited);
                    field.set_text("");
                }
            }
        });
        done.connect_clicked({
            let edited = Rc::clone(&me.edited);
            let popover = me.popover.clone();
            move |_| {
                act(Act::SaveRelays(edited.borrow().clone()));
                popover.popdown();
            }
        });
        me
    }

    /// Open on the relay editor (or Details).
    pub fn edit(&self, on: bool) {
        self.stack.set_visible_child_name(if on { "edit" } else { "info" });
    }

    pub fn show(&self, view: &RoomView) {
        self.answering.set_label(&c::relays_answering(view.relays_up(), view.relays.len()));
        let drawn: Vec<_> =
            view.relays.iter().map(|r| (r.url.clone(), r.healthy, r.rtt_ms)).collect();
        if *self.drawn.borrow() != drawn {
            self.draw_relays(view);
            self.drawn.replace(drawn);
        }
        // Closed, the editor starts again from the room's list: redrawn only when it differs.
        let urls: Vec<String> = view.relays.iter().map(|r| r.url.clone()).collect();
        if !self.popover.is_visible() && *self.edited.borrow() != urls {
            *self.edited.borrow_mut() = urls;
            fill_edit(&self.edit_list, &self.edited);
        }
    }

    fn draw_relays(&self, view: &RoomView) {
        while let Some(child) = self.relays.first_child() {
            self.relays.remove(&child);
        }
        for relay in &view.relays {
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("state-dot");
            if !relay.healthy {
                dot.add_css_class("long-ago");
            }
            dot.set_valign(gtk::Align::Center);
            let url = gtk::Label::new(Some(relay.url.trim_start_matches("wss://")));
            url.add_css_class("monospace");
            url.add_css_class("caption");
            url.set_xalign(0.0);
            url.set_hexpand(true);
            url.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            let how = crate::widgets::haze(
                &if relay.healthy {
                    relay.rtt_ms.map(c::relay_rtt).unwrap_or_default()
                } else {
                    c::relay_down()
                },
                "caption",
            );
            how.set_wrap(false);
            let b = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            b.set_margin_start(10);
            b.set_margin_end(10);
            b.set_height_request(32);
            b.append(&dot);
            b.append(&url);
            b.append(&how);
            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);
            row.set_child(Some(&b));
            self.relays.append(&row);
        }
    }
}

fn fill_edit(list: &gtk::ListBox, edited: &Rc<RefCell<Vec<String>>>) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for url in edited.borrow().iter() {
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("state-dot");
        dot.set_valign(gtk::Align::Center);
        let label = gtk::Label::new(Some(url.trim_start_matches("wss://")));
        label.add_css_class("monospace");
        label.add_css_class("caption");
        label.set_xalign(0.0);
        label.set_hexpand(true);
        let remove = gtk::Button::from_icon_name("window-close-symbolic");
        remove.add_css_class("flat");
        remove.add_css_class("circular");
        remove.set_tooltip_text(Some(&c::remove_relay(url)));
        remove.update_property(&[gtk::accessible::Property::Label(&c::remove_relay(url))]);
        remove.connect_clicked({
            let edited = Rc::clone(edited);
            let list = list.clone();
            let url = url.clone();
            move |_| {
                edited.borrow_mut().retain(|u| *u != url);
                let list = list.clone();
                let edited = Rc::clone(&edited);
                gtk::glib::idle_add_local_once(move || fill_edit(&list, &edited));
            }
        });
        let b = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        b.set_margin_start(10);
        b.set_margin_end(4);
        b.set_height_request(34);
        b.append(&dot);
        b.append(&label);
        b.append(&remove);
        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.set_child(Some(&b));
        list.append(&row);
    }
}
