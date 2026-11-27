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
    /// The episode last said to a screen reader, so each card is said once.
    said: RefCell<Option<String>>,
    fader: crate::motion::Fader,
}

impl UpNext {
    pub fn new() -> Self {
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .can_shrink(true)
            .build();
        picture.add_css_class("room-upnext-art");
        picture.set_overflow(gtk::Overflow::Hidden);
        // 96 × 54 whatever the art's own size: a size request is only a minimum.
        let frame = crate::fit::Fit::new(&picture, 16.0 / 9.0, 96, true, 0);
        frame.set_valign(gtk::Align::Center);
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
            said: RefCell::default(),
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
            self.said.replace(None);
            self.fader.set(&self.widget, false);
            return;
        };
        let next = &up.episode;
        let episode = match &next.name {
            Some(name) => c::up_next_episode(&next.episode, name),
            None => c::up_next_episode_plain(&next.episode),
        };
        self.episode.set_label(&episode);
        let count = match (up.ends_at_ms, up.waiting_for.first()) {
            (Some(ends), _) => c::plays_for_everyone_in(
                ((ends - now_ms) as f64 / 1000.0).ceil().max(0.0) as i64,
            ),
            (None, Some(name)) => c::up_next_waiting(name),
            (None, None) => String::new(),
        };
        self.count.set_label(&count);
        // Said once as the card comes, in one announcement: what, and when or who it waits
        // for. The count isn't said again each second.
        if self.said.borrow().as_deref() != Some(episode.as_str()) {
            let said = spoken(&c::up_next_for_room(), &episode, &count);
            self.widget.announce(&said, gtk::AccessibleAnnouncementPriority::Medium);
            self.said.replace(Some(episode));
        }
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

/// The card's announcement: its parts as lines, the empty ones left out.
fn spoken(heading: &str, episode: &str, count: &str) -> String {
    [heading, episode, count]
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod spoken_tests {
    #[test]
    fn up_next_is_said_once_with_its_count() {
        assert_eq!(
            super::spoken("Up next", "Episode 10", "In 8 s"),
            "Up next\nEpisode 10\nIn 8 s"
        );
        assert_eq!(super::spoken("Up next", "Episode 10", ""), "Up next\nEpisode 10");
    }
}
