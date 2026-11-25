//! The room's dialogs, as libadwaita alert dialogs: Leave asks always; closing the player asks
//! Leave, Keep the room or Cancel; Move to a new room ticks who is asked to come along.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;

use super::{copy as c, model::MoveCandidate, words};

/// How a Leave question was answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveAnswer {
    Leave,
    KeepRoom,
    Cancel,
}

/// Leave the room? With `closing`, the player is closing and Keep the room is offered too.
pub fn leave(
    parent: &impl IsA<gtk::Widget>,
    closing: bool,
    answer: impl Fn(LeaveAnswer) + 'static,
) -> adw::AlertDialog {
    let dialog = adw::AlertDialog::new(
        Some(&c::leave_title()),
        Some(&if closing { c::leave_close_body() } else { c::leave_body() }),
    );
    dialog.add_response("cancel", &c::cancel());
    if closing {
        dialog.add_response("keep", &c::keep_room());
        dialog.set_prefer_wide_layout(false);
    }
    dialog.add_response("leave", &c::room_leave());
    dialog.set_response_appearance("leave", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some(if closing { "keep" } else { "cancel" }));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        answer(match response {
            "leave" => LeaveAnswer::Leave,
            "keep" => LeaveAnswer::KeepRoom,
            _ => LeaveAnswer::Cancel,
        })
    });
    dialog.present(Some(parent));
    dialog
}

/// Move to a new room: everyone present, ticked unless muted; you always go. The button counts
/// the others ticked, and answers with their ids.
pub fn move_room(
    parent: &impl IsA<gtk::Widget>,
    you: &str,
    candidates: &[MoveCandidate],
    position: f64,
    answer: impl Fn(Vec<String>) + 'static,
) -> adw::AlertDialog {
    let dialog = adw::AlertDialog::new(Some(&c::move_title()), Some(&c::move_body()));
    let list = crate::widgets::boxed_list();
    let ticked: Rc<RefCell<Vec<(String, bool)>>> =
        Rc::new(RefCell::new(candidates.iter().map(|m| (m.id.clone(), m.ticked)).collect()));
    let face_row = |name: &str, title: &str, note: Option<String>| {
        let face = super::face::FaceView::new(24);
        face.show(
            name,
            &words::Face {
                word: String::new(),
                ring: words::Ring::Water,
                dim: 1.0,
                badge: false,
                error: false,
                loud: false,
            },
        );
        let row = adw::ActionRow::builder().title(title).build();
        if let Some(note) = note {
            row.set_subtitle(&note);
        }
        row.add_prefix(&face.widget);
        row
    };
    let me = face_row(you, &c::you(), Some(c::move_always()));
    let always = gtk::CheckButton::new();
    always.set_active(true);
    always.set_sensitive(false);
    always.set_valign(gtk::Align::Center);
    me.add_suffix(&always);
    list.append(&me);
    let go = c::move_with(candidates.iter().filter(|m| m.ticked).count());
    dialog.add_response("cancel", &c::cancel());
    dialog.add_response("move", &go);
    dialog.set_response_appearance("move", adw::ResponseAppearance::Suggested);
    dialog.set_response_enabled("move", candidates.iter().any(|m| m.ticked));
    for m in candidates {
        let row = face_row(&m.name, &m.name, m.muted.then(c::move_muted));
        let check = gtk::CheckButton::new();
        check.set_active(m.ticked);
        check.set_valign(gtk::Align::Center);
        row.add_suffix(&check);
        row.set_activatable_widget(Some(&check));
        check.connect_toggled({
            let ticked = Rc::clone(&ticked);
            let dialog = dialog.clone();
            let id = m.id.clone();
            move |check| {
                let mut t = ticked.borrow_mut();
                if let Some(e) = t.iter_mut().find(|(i, _)| *i == id) {
                    e.1 = check.is_active();
                }
                let n = t.iter().filter(|(_, on)| *on).count();
                dialog.set_response_label("move", &c::move_with(n));
                dialog.set_response_enabled("move", n > 0);
            }
        });
        list.append(&row);
    }
    let foot = crate::widgets::haze(&c::move_foot(&words::mmss(position)), "caption");
    foot.set_justify(gtk::Justification::Center);
    foot.set_xalign(0.5);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
    body.append(&list);
    body.append(&foot);
    dialog.set_extra_child(Some(&body));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        if response == "move" {
            answer(
                ticked
                    .borrow()
                    .iter()
                    .filter(|(_, on)| *on)
                    .map(|(id, _)| id.clone())
                    .collect(),
            );
        }
    });
    dialog.present(Some(parent));
    dialog
}
