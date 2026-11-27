//! Watch together, the place in the main window: what it is, your room when you are in one with
//! the player closed (Open the player, Leave…), Join a room with an invite link, and Recent rooms,
//! which keep their key so Rejoin works without asking anyone. Joining that fails says why in the
//! place it failed: under the field, or in the recent room's row.

use std::rc::Rc;

use adw::prelude::*;

use super::{
    copy as c,
    face::FaceView,
    model::RoomView,
    words::{self, Moves},
};

/// One recent room.
#[derive(Debug, Clone, PartialEq)]
pub struct Recent {
    pub title: String,
    pub line: String,
    /// The show's cover, drawn 64×36 at the row's start.
    pub cover: Option<String>,
    /// What a look into the room found: "Mika and Ren are in the room", "Nobody here now".
    pub here: Option<String>,
    /// Nobody answered the last Rejoin.
    pub failed: bool,
}

/// What the place shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Place {
    pub room: Option<RoomView>,
    /// The pasted link whose relays didn't answer.
    pub join_failed: Option<String>,
    pub recent: Vec<Recent>,
}

/// What the place's buttons ask for.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaceAct {
    OpenPlayer,
    Leave,
    Join(String),
    Rejoin(usize),
    /// Selecting a recent room looks in without anyone seeing.
    LookIn(usize),
    /// Remove from recent rooms.
    Forget(usize),
}

pub fn build(
    hub: &crate::hub::Hub,
    place: &Place,
    now_ms: i64,
    act: impl Fn(PlaceAct) + 'static,
) -> gtk::ScrolledWindow {
    let act = Rc::new(act);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.append(&crate::widgets::haze(&c::place_intro(), ""));

    if let Some(view) = &place.room {
        let line = words::line(view, &Moves::default(), now_ms);
        let card = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        card.add_css_class("card");
        card.add_css_class("room-card");
        card.set_margin_top(22);
        let texts = gtk::Box::new(gtk::Orientation::Vertical, 2);
        texts.set_hexpand(true);
        texts.append(&crate::widgets::haze(&c::your_room(), "caption-heading"));
        let title = crate::widgets::show_title(&view.title, "row");
        title.set_xalign(0.0);
        texts.append(&title);
        let l = gtk::Label::new(Some(&line.title));
        l.add_css_class("heading");
        l.set_xalign(0.0);
        l.set_margin_top(4);
        texts.append(&l);
        let sub = crate::widgets::haze(&line.sub, "caption");
        sub.add_css_class("numeric");
        texts.append(&sub);
        let people = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        people.set_margin_top(10);
        for m in words::ordered(view) {
            let look = words::face(view, m, now_ms);
            let face = FaceView::new(22);
            face.show(&m.name, &look);
            let who = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            who.append(&face.widget);
            let name = gtk::Label::new(Some(&if m.you { c::you() } else { m.name.clone() }));
            name.add_css_class("caption");
            who.append(&name);
            who.append(&crate::widgets::haze(&look.word, "caption"));
            people.append(&who);
        }
        texts.append(&people);
        card.append(&texts);
        let buttons = gtk::Box::new(gtk::Orientation::Vertical, 8);
        buttons.set_valign(gtk::Align::Center);
        let open = gtk::Button::with_label(&c::open_player());
        open.add_css_class("suggested-action");
        open.add_css_class("pill");
        let leave = gtk::Button::with_label(&c::leave_ellipsis());
        leave.add_css_class("pill");
        let a = Rc::clone(&act);
        open.connect_clicked(move |_| a(PlaceAct::OpenPlayer));
        let a = Rc::clone(&act);
        leave.connect_clicked(move |_| a(PlaceAct::Leave));
        buttons.append(&open);
        buttons.append(&leave);
        card.append(&buttons);
        column.append(&card);
    }

    let heading = |text: &str| {
        let h = gtk::Label::new(Some(text));
        h.add_css_class("title-4");
        h.set_xalign(0.0);
        h.set_margin_top(26);
        h
    };
    column.append(&heading(&c::join_a_room()));
    let field = gtk::Entry::builder().placeholder_text(c::invite_link()).hexpand(true).build();
    field.update_property(&[gtk::accessible::Property::Label(&c::invite_link())]);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_margin_top(8);
    row.append(&field);
    if let Some(link) = &place.join_failed {
        field.set_text(link);
        field.add_css_class("monospace");
        let again = gtk::Button::with_label(&c::try_again());
        let a = Rc::clone(&act);
        let f = field.clone();
        again.connect_clicked(move |_| a(PlaceAct::Join(f.text().into())));
        row.append(&again);
        column.append(&row);
        let why = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        why.set_margin_top(8);
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("mark");
        dot.add_css_class("vermilion");
        dot.set_valign(gtk::Align::Start);
        dot.set_margin_top(6);
        why.append(&dot);
        let text = gtk::Label::new(None);
        text.set_markup(&format!(
            "<b>{}</b> <span foreground=\"#A3A9C4\">{}</span>", // i18n-skip
            gtk::glib::markup_escape_text(&c::unreachable()),
            gtk::glib::markup_escape_text(&c::unreachable_note()),
        ));
        text.set_wrap(true);
        text.set_xalign(0.0);
        why.append(&text);
        column.append(&why);
    } else {
        let paste = gtk::Button::with_label(&c::paste());
        // Secondary like Paste, as the board draws it: the place has no amber until a room.
        let join = gtk::Button::with_label(&c::join());
        join.set_sensitive(false);
        field.connect_changed({
            let join = join.clone();
            move |f| join.set_sensitive(!f.text().trim().is_empty())
        });
        paste.connect_clicked({
            let field = field.clone();
            move |b| {
                let field = field.clone();
                b.clipboard().read_text_async(gtk::gio::Cancellable::NONE, move |text| {
                    if let Ok(Some(text)) = text {
                        field.set_text(&text);
                    }
                });
            }
        });
        let a = Rc::clone(&act);
        let f = field.clone();
        join.connect_clicked(move |_| a(PlaceAct::Join(f.text().into())));
        row.append(&paste);
        row.append(&join);
        column.append(&row);
    }

    let recent_head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let h = heading(&c::recent_rooms());
    recent_head.append(&h);
    let note = crate::widgets::haze(&c::recent_note(), "caption");
    note.set_wrap(false);
    note.set_valign(gtk::Align::End);
    note.set_margin_bottom(2);
    recent_head.append(&note);
    column.append(&recent_head);
    let list = crate::widgets::boxed_list();
    list.set_margin_top(8);
    for (i, r) in place.recent.iter().enumerate() {
        let esc = |s: &str| gtk::glib::markup_escape_text(s).to_string();
        let line = match &r.here {
            Some(here) => format!("{} · {}", esc(&r.line), esc(here)), // i18n-skip
            None => esc(&r.line),
        };
        let row = adw::ActionRow::builder()
            .title(esc(&r.title))
            .subtitle(&line)
            .activatable(true)
            .build();
        // 64 × 36 as the board draws it, cropped from the show's cover.
        let (frame, _) = crate::widgets::art(hub, r.cover.as_deref(), None, "cover");
        frame.add_css_class("small");
        let art = crate::fit::Fit::new(&frame, 64.0 / 36.0, 64, true, 0);
        art.set_valign(gtk::Align::Center);
        art.set_hexpand(false);
        art.set_vexpand(false);
        row.add_prefix(&art);
        let a = Rc::clone(&act);
        row.connect_activated(move |_| a(PlaceAct::LookIn(i)));
        if r.failed {
            // The lead in bold and the hint after it, on one line under the row's own.
            row.set_subtitle(&format!(
                "{}\n<b>{}</b> {}",
                line,
                esc(&c::nobody_answered()),
                esc(&c::nobody_answered_note())
            )); // i18n-skip
            row.set_subtitle_lines(4);
        }
        let b = gtk::Button::with_label(&if r.failed { c::try_again() } else { c::rejoin() });
        b.set_valign(gtk::Align::Center);
        let a = Rc::clone(&act);
        b.connect_clicked(move |_| a(PlaceAct::Rejoin(i)));
        row.add_suffix(&b);
        // More: Remove from recent rooms.
        let menu = gtk::gio::Menu::new();
        menu.append(Some(&c::forget_room()), Some("recent.forget"));
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .menu_model(&menu)
            .valign(gtk::Align::Center)
            .tooltip_text(c::more())
            .build();
        more.add_css_class("flat");
        more.update_property(&[gtk::accessible::Property::Label(&c::more())]);
        let actions = gtk::gio::SimpleActionGroup::new();
        let forget = gtk::gio::SimpleAction::new("forget", None);
        let a = Rc::clone(&act);
        forget.connect_activate(move |_, _| a(PlaceAct::Forget(i)));
        actions.add_action(&forget);
        row.insert_action_group("recent", Some(&actions));
        row.add_suffix(&more);
        list.append(&row);
    }
    column.append(&list);
    crate::widgets::column_page(&column, 820)
}
