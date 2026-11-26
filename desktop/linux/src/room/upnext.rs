//! The room's Up next card (room board, "Up next"): the player's one card, in the bottom trailing
//! corner of the video, with the next episode's art. The count runs from the room's place, so
//! nobody moves on alone; while someone hasn't reached the end yet, it says who it waits for.
//! It has no buttons: pausing stops the count for everyone.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;

use super::{copy as c, model::RoomView};

/// Draws a picture from a url: the hub's art loader.
pub type Art = Rc<dyn Fn(&gtk::Picture, Option<&str>)>;

pub struct UpNext {
    pub widget: gtk::Box,
    picture: gtk::Picture,
    episode: gtk::Label,
    count: gtk::Label,
    art: RefCell<Option<Art>>,
    /// The art's url as last asked for.
    shown_art: RefCell<Option<String>>,
    fader: crate::motion::Fader,
}

impl UpNext {
    pub fn new() -> Self {
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(96)
            .height_request(54)
            .build();
        picture.add_css_class("room-upnext-art");
        picture.set_overflow(gtk::Overflow::Hidden);
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        frame.set_size_request(96, 54);
        frame.set_valign(gtk::Align::Center);
        frame.append(&picture);
        let over = crate::widgets::haze(&c::up_next_for_room(), "caption");
        over.set_xalign(0.0);
        let episode = gtk::Label::new(None);
        episode.set_xalign(0.0);
        episode.set_ellipsize(gtk::pango::EllipsizeMode::End);
        episode.add_css_class("room-upnext-episode");
        let count = crate::widgets::haze("", "caption");
        count.set_xalign(0.0);
        count.add_css_class("numeric");
        let texts = gtk::Box::new(gtk::Orientation::Vertical, 1);
        texts.set_valign(gtk::Align::Center);
        texts.set_hexpand(true);
        texts.append(&over);
        texts.append(&episode);
        texts.append(&count);
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        widget.add_css_class("osd");
        widget.add_css_class("room-upnext");
        widget.set_halign(gtk::Align::End);
        widget.set_valign(gtk::Align::End);
        widget.set_margin_end(20);
        widget.set_margin_bottom(68);
        widget.set_size_request(300, -1);
        widget.append(&frame);
        widget.append(&texts);
        widget.set_visible(false);
        Self {
            widget,
            picture,
            episode,
            count,
            art: RefCell::default(),
            shown_art: RefCell::default(),
            fader: crate::motion::Fader::always(),
        }
    }

    /// How the card loads its art.
    pub fn set_art(&self, art: Art) {
        self.art.replace(Some(art));
    }

    /// The card as `view` has it at `now_ms`: shown while the room has an Up next nobody
    /// cancelled.
    pub fn show(&self, view: Option<&RoomView>, now_ms: i64) {
        let up = view.and_then(|v| v.up_next.as_ref()).filter(|u| u.cancelled_by.is_none());
        let Some(up) = up else {
            self.fader.set(&self.widget, false);
            return;
        };
        let next = &up.episode;
        self.episode.set_label(&match &next.name {
            Some(name) => c::up_next_episode(&next.episode, name),
            None => c::up_next_episode_plain(&next.episode),
        });
        let count = match (up.ends_at_ms, up.waiting_for.first()) {
            (Some(ends), _) => c::plays_for_everyone_in(
                ((ends - now_ms) as f64 / 1000.0).ceil().max(0.0) as i64,
            ),
            (None, Some(name)) => c::up_next_waiting(name),
            (None, None) => String::new(),
        };
        self.count.set_label(&count);
        self.count.set_visible(!count.is_empty());
        let url = next.banner_url.clone().or_else(|| next.cover_url.clone());
        if *self.shown_art.borrow() != url {
            let art = self.art.borrow().clone();
            if let Some(art) = art {
                art(&self.picture, url.as_deref());
            }
            self.shown_art.replace(url);
        }
        self.fader.set(&self.widget, true);
    }
}
