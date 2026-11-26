//! The video area when your own player has nothing to show yet: the room before play (the
//! episode's banner until everyone's first frame is ready, with Play for everyone), an invite
//! not yet answered, your own source being chosen, and none of your sources having the episode.
//! It lies over the video at the bottom leading edge, on a dark gradient, and crossfades away
//! when the first frame lands.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;

use super::{
    column::Act,
    copy as c,
    model::{Before, MemberState, RoomView},
};

/// What the banner shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Panel {
    Before,
    Invited,
    /// Your sources for the room's episode: (name, detail, picked).
    Choosing(Vec<(String, String, bool)>),
    NoSource {
        sources: usize,
    },
}

impl Panel {
    /// The panel `view` calls for on this device at `now_ms`, if any.
    pub fn for_view(
        view: &RoomView,
        sources: &[(String, String, bool)],
        count: usize,
        now_ms: i64,
    ) -> Option<Self> {
        if view.invited_by.is_some() {
            return Some(Self::Invited);
        }
        match view.you().map(|m| m.state) {
            Some(MemberState::Choosing) => return Some(Self::Choosing(sources.to_vec())),
            Some(MemberState::NoSource) => return Some(Self::NoSource { sources: count }),
            _ => {}
        }
        matches!(view.before(now_ms), Some(Before::Idle | Before::Waiting)).then_some(Self::Before)
    }
}

/// The panel, the room before play, the title and the episode.
type Shown = (Panel, Option<Before>, String, String);

pub struct Banner {
    pub widget: gtk::Box,
    body: gtk::Box,
    act: Rc<dyn Fn(Act)>,
    fader: crate::motion::Fader,
    /// What it shows now, so the second's tick redraws nothing.
    shown: RefCell<Option<Shown>>,
}

impl Banner {
    pub fn new(act: impl Fn(Act) + 'static) -> Self {
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_valign(gtk::Align::End);
        body.set_halign(gtk::Align::Start);
        body.set_margin_start(56);
        body.set_margin_bottom(96);
        body.set_width_request(560);
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add_css_class("room-banner");
        widget.append(&body);
        body.set_vexpand(true);
        widget.set_visible(false);
        Self {
            widget,
            body,
            act: Rc::new(act),
            fader: crate::motion::Fader::always(),
            shown: RefCell::default(),
        }
    }

    pub fn show(&self, view: &RoomView, panel: Option<&Panel>) {
        let Some(panel) = panel else {
            self.shown.replace(None);
            self.fader.set(&self.widget, false);
            return;
        };
        // Idle or waiting is all the banner tells apart before play.
        let before = view.before_play.then_some(if view.waiting.is_empty() {
            Before::Idle
        } else {
            Before::Waiting
        });
        let key = (panel.clone(), before, view.title.clone(), view.episode.clone());
        if self.shown.borrow().as_ref() == Some(&key) {
            return;
        }
        self.shown.replace(Some(key));
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        let title = crate::widgets::show_title(&view.title, "page");
        title.set_xalign(0.0);
        let episode = c::banner_episode(&view.episode, view.episode_title.as_deref(), view.minutes);
        let button = |label: &str, pri: bool, a: Act| {
            let b = gtk::Button::with_label(label);
            b.add_css_class("pill");
            if pri {
                b.add_css_class("suggested-action");
            }
            let act = Rc::clone(&self.act);
            b.connect_clicked(move |_| act(a.clone()));
            b
        };
        let row = || {
            let r = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            r.set_margin_top(16);
            r
        };
        match panel {
            Panel::Before | Panel::Invited => {
                let over = crate::widgets::haze(
                    &if *panel == Panel::Invited {
                        c::room_is_watching()
                    } else {
                        c::up_in_room()
                    },
                    "heading",
                );
                self.body.append(&over);
                title.set_margin_top(6);
                self.body.append(&title);
                let ep = gtk::Label::new(Some(&episode));
                ep.set_xalign(0.0);
                ep.add_css_class("title-4");
                ep.set_margin_top(4);
                self.body.append(&ep);
                let plays = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                plays.set_margin_top(10);
                let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                dot.add_css_class("state-dot");
                dot.set_valign(gtk::Align::Center);
                plays.append(&dot);
                plays.append(&crate::widgets::haze(&c::plays_from_yours(), ""));
                self.body.append(&plays);
                if *panel == Panel::Before && before == Some(Before::Idle) {
                    let r = row();
                    r.set_margin_top(22);
                    let play = gtk::Button::new();
                    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    inner.append(&gtk::Image::from_icon_name("media-playback-start-symbolic"));
                    inner.append(&gtk::Label::new(Some(&c::play_for_everyone())));
                    play.set_child(Some(&inner));
                    play.add_css_class("pill");
                    play.add_css_class("suggested-action");
                    let act = Rc::clone(&self.act);
                    play.connect_clicked(move |_| act(Act::PlayForEveryone));
                    r.append(&play);
                    let note = crate::widgets::haze(&c::anyone_can_start(), "caption");
                    note.set_valign(gtk::Align::Center);
                    r.append(&note);
                    self.body.append(&r);
                }
            }
            Panel::Choosing(sources) => {
                self.body.append(&title);
                let what = gtk::Label::new(Some(&c::choose_source_for(&view.episode)));
                what.set_xalign(0.0);
                what.add_css_class("title-4");
                self.body.append(&what);
                self.body.append(&crate::widgets::haze(&c::choose_note(), ""));
                let list = crate::widgets::boxed_list();
                list.set_margin_top(14);
                list.set_selection_mode(gtk::SelectionMode::Single);
                for (i, (name, detail, picked)) in sources.iter().enumerate() {
                    let r = adw::ActionRow::builder().title(name).subtitle(detail).build();
                    if *picked {
                        r.add_suffix(&gtk::Image::from_icon_name("object-select-symbolic"));
                    }
                    list.append(&r);
                    if *picked {
                        list.select_row(Some(&r));
                    }
                    let _ = i;
                }
                self.body.append(&list);
                let r = row();
                let picked = sources.iter().position(|s| s.2).unwrap_or(0);
                r.append(&button(&c::play_in_room(), true, Act::PlayInRoom(picked)));
                r.append(&button(&c::not_now(), false, Act::NotNow));
                self.body.append(&r);
            }
            Panel::NoSource { sources } => {
                self.body.append(&title);
                let what = gtk::Label::new(Some(&c::none_has(&view.episode)));
                what.set_xalign(0.0);
                what.add_css_class("title-4");
                what.set_margin_top(6);
                self.body.append(&what);
                let note = crate::widgets::haze(&c::none_has_note(*sources), "");
                note.set_margin_top(4);
                self.body.append(&note);
                let r = row();
                r.append(&button(&c::open_sources(), true, Act::OpenSources));
                let search = gtk::Button::new();
                let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                inner.append(&gtk::Image::from_icon_name("system-search-symbolic"));
                inner.append(&gtk::Label::new(Some(&c::search_titles())));
                search.set_child(Some(&inner));
                search.add_css_class("pill");
                let act = Rc::clone(&self.act);
                search.connect_clicked(move |_| act(Act::SearchTitles));
                r.append(&search);
                self.body.append(&r);
            }
        }
        self.fader.set(&self.widget, true);
    }
}
