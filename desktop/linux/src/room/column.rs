//! The room column beside the video (room board, Linux frame): a header bar with Watch
//! together, Invite, Details and Leave; the room's line with the one button the moment needs;
//! the people in a boxed list, each a face and a word; and the conversation with the linked
//! entry and Send. 336 px wide on the sidebar fill, in the player's split view.
//!
//! It draws the [`RoomView`] it is handed and says what was pressed through `act`; the player
//! sends the commands.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use adw::prelude::*;
use gtk::glib;

use super::{
    copy as c,
    face::FaceView,
    model::{ConvLine, MemberState, RoomView},
    words::{self, LineButtons, Moves},
};

/// The column's width, from the board.
pub const WIDTH: i32 = 336;
/// More people than this scroll in the list.
const ROWS: i32 = 5;
/// The least a row is: two lines and the 28 px face. The list measures its rows for the real one.
const ROW_HEIGHT: i32 = 54;

/// A handler of the room's buttons, and the cell that holds one.
pub type ActFn = Rc<dyn Fn(Act)>;
pub type ActCell = Rc<RefCell<Option<ActFn>>>;

/// What the column asks the player to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    CarryOn,
    DontWait,
    Join,
    NotNow,
    GoAlong,
    Stay,
    Leave,
    CopyLink,
    NewInvite,
    /// Move to a new room…: the dialog.
    MoveRoom,
    /// Move with these member ids.
    Move(Vec<String>),
    SaveRelays(Vec<String>),
    Say(String),
    PlayForEveryone,
    PlayInRoom(usize),
    OpenSources,
    SearchTitles,
}

struct Row {
    row: gtk::ListBoxRow,
    face: FaceView,
    name: gtk::Label,
    word: gtk::Label,
    trail: gtk::Label,
}

pub struct Column {
    pub widget: adw::ToolbarView,
    line_stack: gtk::Stack,
    lines: [(gtk::Label, gtk::Label); 2],
    front: Cell<usize>,
    buttons: gtk::Box,
    list: gtk::ListBox,
    list_scroll: gtk::ScrolledWindow,
    rows: RefCell<HashMap<String, Row>>,
    conv: gtk::Box,
    conv_scroll: gtk::ScrolledWindow,
    entry: gtk::Entry,
    pub invite: gtk::MenuButton,
    pub details: gtk::MenuButton,
    invite_pop: super::popovers::Invite,
    details_pop: super::popovers::Details,
    act: ActCell,
    shown: RefCell<Option<(String, String, LineButtons)>>,
    conv_shown: RefCell<Vec<ConvLine>>,
}

impl Column {
    pub fn new() -> Rc<Self> {
        let act: ActCell = Rc::default();
        let send = {
            let act = Rc::clone(&act);
            move |a: Act| {
                let f = act.borrow().clone();
                if let Some(f) = f {
                    f(a);
                }
            }
        };
        let send = Rc::new(send);

        let title = gtk::Label::new(Some(&c::room_title()));
        title.add_css_class("heading");
        let header = adw::HeaderBar::new();
        header.add_css_class("room-header");
        header.set_show_title(false);
        // Only close, as the board's header bar: the column stays at its width.
        header.set_decoration_layout(Some(":close"));
        header.pack_start(&title);
        let leave = gtk::Button::with_label(&c::room_leave());
        leave.set_valign(gtk::Align::Center);
        leave.connect_clicked({
            let send = Rc::clone(&send);
            move |_| send(Act::Leave)
        });
        let invite_pop = super::popovers::Invite::new({
            let send = Rc::clone(&send);
            move |a| send(a)
        });
        let details_pop = super::popovers::Details::new({
            let send = Rc::clone(&send);
            move |a| send(a)
        });
        let invite = gtk::MenuButton::builder()
            .icon_name("contact-new-symbolic")
            .tooltip_text(c::room_invite())
            .valign(gtk::Align::Center)
            .popover(&invite_pop.popover)
            .build();
        invite.update_property(&[gtk::accessible::Property::Label(&c::room_invite())]);
        let details = gtk::MenuButton::builder()
            .icon_name("info-outline-symbolic")
            .tooltip_text(c::room_details())
            .valign(gtk::Align::Center)
            .popover(&details_pop.popover)
            .build();
        details.update_property(&[gtk::accessible::Property::Label(&c::room_details())]);
        header.pack_end(&leave);
        header.pack_end(&details);
        header.pack_end(&invite);

        // The room's line: two copies in a crossfading stack, so a change of words fades in
        // over the old ones and the column never goes empty.
        let make_line = || {
            let title = gtk::Label::new(None);
            title.add_css_class("room-line");
            title.set_xalign(0.0);
            title.set_wrap(true);
            let sub = crate::widgets::haze("", "room-line-sub");
            sub.add_css_class("numeric");
            let b = gtk::Box::new(gtk::Orientation::Vertical, 2);
            b.append(&title);
            b.append(&sub);
            (b, title, sub)
        };
        let (b0, t0, s0) = make_line();
        let (b1, t1, s1) = make_line();
        let line_stack = gtk::Stack::new();
        line_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        line_stack.set_transition_duration(200);
        line_stack.set_interpolate_size(true);
        line_stack.set_vhomogeneous(false);
        line_stack.add_named(&b0, Some("0"));
        line_stack.add_named(&b1, Some("1"));
        line_stack.update_property(&[gtk::accessible::Property::Label(&c::room_title())]);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_margin_top(10);
        buttons.set_visible(false);

        let list = crate::widgets::boxed_list();
        list.add_css_class("room-people");
        list.update_property(&[gtk::accessible::Property::Label(&c::room_people())]);
        let list_scroll = gtk::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(ROWS * ROW_HEIGHT + 2)
            .build();
        list_scroll.set_margin_top(12);

        let conv = gtk::Box::new(gtk::Orientation::Vertical, 6);
        conv.set_valign(gtk::Align::End);
        conv.add_css_class("room-conv");
        let conv_scroll = gtk::ScrolledWindow::builder()
            .child(&conv)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        conv_scroll.set_margin_top(12);

        let entry = gtk::Entry::builder().placeholder_text(c::room_say()).hexpand(true).build();
        entry.update_property(&[gtk::accessible::Property::Label(&c::room_say())]);
        let send_button = gtk::Button::with_label(&kaeri_i18n::t::party_chat_send());
        send_button.set_sensitive(false);
        let field = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        field.add_css_class("linked");
        field.set_margin_top(12);
        field.set_margin_bottom(14);
        field.append(&entry);
        field.append(&send_button);
        entry.connect_changed({
            let send_button = send_button.clone();
            move |e| send_button.set_sensitive(!e.text().trim().is_empty())
        });
        let submit = {
            let send = Rc::clone(&send);
            let entry = entry.clone();
            move || {
                let text = entry.text().trim().to_owned();
                if !text.is_empty() {
                    send(Act::Say(text));
                    entry.set_text("");
                }
            }
        };
        let submit = Rc::new(submit);
        entry.connect_activate({
            let submit = Rc::clone(&submit);
            move |_| submit()
        });
        send_button.connect_clicked(move |_| submit());

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_top(4);
        body.append(&line_stack);
        body.append(&buttons);
        body.append(&list_scroll);
        body.append(&conv_scroll);
        body.append(&field);

        let widget = adw::ToolbarView::new();
        widget.add_css_class("room-column");
        widget.add_top_bar(&header);
        widget.set_content(Some(&body));
        widget.set_width_request(WIDTH);

        Rc::new(Self {
            widget,
            line_stack,
            lines: [(t0, s0), (t1, s1)],
            front: Cell::new(0),
            buttons,
            list,
            list_scroll,
            rows: RefCell::default(),
            conv,
            conv_scroll,
            entry,
            invite,
            details,
            invite_pop,
            details_pop,
            act,
            shown: RefCell::default(),
            conv_shown: RefCell::default(),
        })
    }

    pub fn on_act(&self, f: impl Fn(Act) + 'static) {
        self.act.replace(Some(Rc::new(f)));
    }

    fn act(&self, a: Act) {
        let f = self.act.borrow().clone();
        if let Some(f) = f {
            f(a);
        }
    }

    /// Draw `view` as it stands at `now_ms`.
    pub fn show(self: &Rc<Self>, view: &RoomView, moves: &Moves, now_ms: i64) {
        let line = words::line(view, moves, now_ms);
        self.show_line(&line);
        self.show_people(view, now_ms);
        self.invite_pop.show(view);
        self.details_pop.show(view);
        let joined = view.invited_by.is_none();
        self.entry.set_sensitive(joined);
        self.invite.set_sensitive(joined);
    }

    fn show_line(self: &Rc<Self>, line: &words::Line) {
        let mut shown = self.shown.borrow_mut();
        let same_words =
            shown.as_ref().is_some_and(|(t, _, b)| *t == line.title && *b == line.buttons);
        let front = self.front.get();
        if same_words {
            // The seconds count in place.
            self.lines[front].1.set_label(&line.sub);
        } else {
            let back = if shown.is_none() { front } else { 1 - front };
            self.lines[back].0.set_label(&line.title);
            self.lines[back].1.set_label(&line.sub);
            self.line_stack.set_visible_child_name(if back == 0 { "0" } else { "1" });
            self.front.set(back);
            self.show_buttons(&line.buttons);
        }
        *shown = Some((line.title.clone(), line.sub.clone(), line.buttons.clone()));
    }

    fn show_buttons(self: &Rc<Self>, buttons: &LineButtons) {
        while let Some(child) = self.buttons.first_child() {
            self.buttons.remove(&child);
        }
        let add = |label: &str, pri: bool, a: Act| {
            let b = gtk::Button::with_label(label);
            b.add_css_class("pill");
            b.add_css_class("room-button");
            if pri {
                b.add_css_class("suggested-action");
            }
            let me = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                if let Some(me) = me.upgrade() {
                    me.act(a.clone());
                }
            });
            self.buttons.append(&b);
            b
        };
        match buttons {
            LineButtons::None => {}
            LineButtons::CarryOn(label) => {
                let b = add(label, true, Act::CarryOn);
                crate::motion::once_mapped(&b, {
                    let b = b.clone();
                    move || {
                        crate::motion::fade_in(&b);
                    }
                });
            }
            LineButtons::DontWait => {
                add(&c::dont_wait(), false, Act::DontWait);
            }
            LineButtons::Join { anyway } => {
                add(&if *anyway { c::join_anyway() } else { c::join_room() }, true, Act::Join);
                add(&c::not_now(), false, Act::NotNow);
            }
            LineButtons::Move => {
                add(&c::go_along(), true, Act::GoAlong);
                add(&c::stay_here(), false, Act::Stay);
            }
        }
        self.buttons.set_visible(*buttons != LineButtons::None);
    }

    fn show_people(&self, view: &RoomView, now_ms: i64) {
        let members = words::ordered(view);
        let mut rows = self.rows.borrow_mut();
        rows.retain(|id, row| {
            let keep = members.iter().any(|m| &m.id == id);
            if !keep {
                self.list.remove(&row.row);
            }
            keep
        });
        for (i, m) in members.iter().enumerate() {
            let look = words::face(view, m, now_ms);
            let row = rows.entry(m.id.clone()).or_insert_with(|| {
                let r = new_row();
                self.list.insert(&r.row, i as i32);
                crate::motion::once_mapped(&r.row, {
                    let w = r.row.clone();
                    move || {
                        crate::motion::fade_in(&w);
                    }
                });
                r
            });
            if row.row.index() != i as i32 {
                self.list.remove(&row.row);
                self.list.insert(&row.row, i as i32);
            }
            row.face.show(&m.name, &look);
            let name = if m.you {
                format!(
                    "{} <span weight=\"normal\" foreground=\"#A3A9C4\">{}</span>",
                    glib::markup_escape_text(&m.name),
                    glib::markup_escape_text(&c::you_tag())
                ) // i18n-skip
            } else {
                glib::markup_escape_text(&m.name).to_string()
            };
            row.name.set_markup(&name);
            row.word.set_label(&look.word);
            if look.loud {
                row.word.remove_css_class("haze");
            } else {
                row.word.add_css_class("haze");
            }
            let trail = (m.state == MemberState::Behind && !m.you)
                .then(|| words::mmss(view.position - f64::from(m.behind_s.unwrap_or(0.0))));
            row.trail.set_visible(trail.is_some());
            row.trail.set_label(trail.as_deref().unwrap_or(""));
            row.row.set_opacity(if m.state == MemberState::Left { 0.6 } else { 1.0 });
            row.row.update_property(&[gtk::accessible::Property::Label(&format!(
                "{}, {}",
                m.name, look.word
            ))]); // i18n-skip
        }
        let many = members.len() as i32 > ROWS;
        // Five whole rows, as tall as a row draws at this font, then the list scrolls.
        if many && let Some(first) = self.list.row_at_index(0) {
            let (_, natural, _, _) = first.measure(gtk::Orientation::Vertical, WIDTH);
            let row = natural.max(ROW_HEIGHT) + 1;
            self.list_scroll.set_max_content_height(ROWS * row + 2);
        }
        self.list_scroll.set_vscrollbar_policy(if many {
            gtk::PolicyType::Automatic
        } else {
            gtk::PolicyType::Never
        });
    }

    /// The conversation, newest last, scrolled to its end.
    pub fn show_conversation(&self, lines: &[ConvLine]) {
        if *self.conv_shown.borrow() == lines {
            return;
        }
        while let Some(child) = self.conv.first_child() {
            self.conv.remove(&child);
        }
        for line in lines {
            let label = gtk::Label::new(None);
            label.set_xalign(0.0);
            label.set_wrap(true);
            label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            match line {
                ConvLine::System(text) => {
                    label.set_label(text);
                    label.add_css_class("haze");
                    label.add_css_class("room-sys");
                    label.add_css_class("numeric");
                }
                ConvLine::Said { who, text, you } => {
                    let who = glib::markup_escape_text(who);
                    let text = glib::markup_escape_text(text);
                    label.set_markup(&if *you {
                        format!(
                            "<span weight=\"600\" foreground=\"#A3A9C4\">{who}</span>  {text}"
                        ) // i18n-skip
                    } else {
                        format!("<span weight=\"600\">{who}</span>  {text}") // i18n-skip
                    });
                    label.add_css_class("room-said");
                }
            }
            self.conv.append(&label);
        }
        self.conv_shown.replace(lines.to_vec());
        let adj = self.conv_scroll.vadjustment();
        glib::idle_add_local_once(move || adj.set_value(adj.upper()));
    }

    /// For a shot: open the Invite or Details popover, or Details' relay editor.
    pub fn shot_popover(&self, which: &str) {
        match which {
            "invite" => self.invite.popup(),
            "details" => self.details.popup(),
            "edit-relays" => {
                self.details_pop.edit(true);
                self.details.popup();
            }
            _ => {}
        }
    }
}

fn new_row() -> Row {
    let face = FaceView::new(28);
    let name = gtk::Label::new(None);
    name.add_css_class("room-name");
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let word = gtk::Label::new(None);
    word.add_css_class("caption");
    word.add_css_class("numeric");
    word.set_xalign(0.0);
    word.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let texts = gtk::Box::new(gtk::Orientation::Vertical, 0);
    texts.set_valign(gtk::Align::Center);
    texts.set_hexpand(true);
    texts.append(&name);
    texts.append(&word);
    let trail = crate::widgets::haze("", "caption");
    trail.add_css_class("numeric");
    trail.set_visible(false);
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    b.set_margin_start(12);
    b.set_margin_end(12);
    b.set_height_request(ROW_HEIGHT);
    b.append(&face.widget);
    b.append(&texts);
    b.append(&trail);
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_child(Some(&b));
    Row { row, face, name, word, trail }
}
