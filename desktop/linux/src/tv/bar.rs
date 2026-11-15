//! A cast's bar along the bottom of the pages: the episode's art, the show, where the cast
//! stands, Play or Pause and Next, with the TV's playhead as a line along its top edge. It is
//! the bottom bar of the window's `AdwBottomSheet`: a click grows it into the cast's controls,
//! and closing them shrinks them back into it.

use std::{rc::Rc, time::Duration};

use adw::prelude::*;
use gtk::glib;
use kaeri_engine::remote::protocol::command::RemoteCommand;
use kaeri_i18n::t;

use super::{Sheet, Tv, cast::Stage};
use crate::{widgets, window::Ctx};

/// The bar's progress moves on this often while it shows.
const TICK: Duration = Duration::from_millis(500);

struct Bar {
    root: gtk::Overlay,
    art: gtk::Picture,
    title: gtk::Label,
    status: gtk::Box,
    play: gtk::Button,
    next: gtk::Button,
    progress: gtk::ProgressBar,
    /// The art's address drawn, so an update draws it once.
    art_url: std::cell::RefCell<Option<String>>,
    /// The status line drawn, so an echo changes nothing.
    shown: std::cell::RefCell<(String, &'static str)>,
}

impl Bar {
    fn new() -> Self {
        let art = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(64)
            .height_request(36)
            .valign(gtk::Align::Center)
            .overflow(gtk::Overflow::Hidden)
            .build();
        art.add_css_class("cast-bar-art");
        let title = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .width_chars(1)
            .build();
        title.add_css_class("heading");
        let status = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
        text.set_hexpand(true);
        text.set_valign(gtk::Align::Center);
        text.append(&title);
        text.append(&status);
        let play = gtk::Button::from_icon_name("media-playback-pause-symbolic");
        play.add_css_class("circular");
        play.add_css_class("flat");
        play.set_valign(gtk::Align::Center);
        let next = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        next.add_css_class("flat");
        next.set_valign(gtk::Align::Center);
        next.set_tooltip_text(Some(&t::player_up_next()));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("cast-bar");
        row.append(&art);
        row.append(&text);
        row.append(&play);
        row.append(&next);
        let progress = gtk::ProgressBar::builder().valign(gtk::Align::Start).build();
        progress.add_css_class("cast-bar-progress");
        let root = gtk::Overlay::builder().child(&row).build();
        root.add_overlay(&progress);
        root.set_tooltip_text(Some(&t::cast_mini_opens()));
        Self {
            root,
            art,
            title,
            status,
            play,
            next,
            progress,
            art_url: std::cell::RefCell::default(),
            shown: std::cell::RefCell::default(),
        }
    }

    /// Draw the cast as the model holds it. Every part is changed in place: the TV's status
    /// echoes never take a widget down.
    fn update(&self, ctx: &Ctx, tv: &Tv) {
        let Some(view) = tv.cast_view() else { return };
        let Some(now) = view.now.as_ref() else { return };
        let entry = ctx.hub.entry(now.id);
        let private = ctx.hub.known.borrow().holds_show(now.id);
        let show = if private { t::now_playing_private() } else { now.title.clone() };
        self.title.set_label(&show);
        let url = entry.and_then(|e| e.banner_url.or(e.cover_url)).filter(|_| !private);
        if *self.art_url.borrow() != url {
            ctx.hub.art(&self.art, url.as_deref());
            self.art_url.replace(url);
        }
        let (line, mark) = match view.stage {
            Stage::Starting => (view.starting.clone(), "haze"),
            Stage::Playing if now.paused => (t::cast_status_paused(&view.name), "haze"),
            Stage::Playing => (view.name.clone(), "water"),
            Stage::Failed => (t::tv_cast_couldnt_start(&view.name), "vermilion"),
            Stage::Lost => (t::tv_cast_stopped(), "vermilion"),
        };
        if *self.shown.borrow() != (line.clone(), mark) {
            match self.status.first_child().and_downcast::<gtk::Box>() {
                Some(standing) => widgets::restand(&standing, &line, mark),
                None => self.status.append(&widgets::standing(&line, mark)),
            }
            self.shown.replace((line, mark));
        }
        let playing = view.stage == Stage::Playing;
        // Dimmed, never hidden, while the TV starts: the bar keeps its shape.
        self.play.set_sensitive(playing);
        self.play.set_opacity(if playing || view.stage.ended() { 1.0 } else { 0.38 });
        self.play.set_visible(!view.stage.ended());
        self.play.set_icon_name(if now.paused {
            "media-playback-start-symbolic"
        } else {
            "media-playback-pause-symbolic"
        });
        let tip = if now.paused { t::action_play() } else { t::action_pause() };
        self.play.set_tooltip_text(Some(&tip));
        self.next.set_visible(playing && view.up_next.is_some());
        self.show_playhead(tv);
    }

    fn show_playhead(&self, tv: &Tv) {
        let length = tv.cast_view().and_then(|v| v.now).and_then(|n| n.duration);
        let at = tv.cast_playhead().unwrap_or(0.0);
        let fraction = length.filter(|l| *l > 0.0).map_or(0.0, |l| (at / l).clamp(0.0, 1.0));
        self.progress.set_fraction(fraction);
    }
}

/// Put the bar under the pages and keep it with the cast: shown while one runs or has ended and
/// is kept, gone once it is let go. The controls open from it and close back into it.
pub fn attach(ctx: &Rc<Ctx>) {
    let bar = Rc::new(Bar::new());
    let sheet = &ctx.cast_sheet;
    sheet.set_bottom_bar(Some(&bar.root));
    sheet.set_reveal_bottom_bar(false);
    sheet.set_can_open(false);

    let tv = Rc::downgrade(&ctx.tv);
    bar.play.connect_clicked({
        let tv = tv.clone();
        move |_| {
            if let Some(tv) = tv.upgrade() {
                tv.cast_send(RemoteCommand::TogglePause);
            }
        }
    });
    bar.next.connect_clicked({
        let tv = tv.clone();
        move |_| {
            if let Some(tv) = tv.upgrade() {
                tv.cast_play_next();
            }
        }
    });
    // A click on the bar opens the controls, before the sheet has them to grow into.
    let click = gtk::GestureClick::new();
    click.connect_released({
        let tv = tv.clone();
        move |gesture, _, _, _| {
            if let Some(tv) = tv.upgrade().filter(|tv| tv.casting() && tv.sheet().is_none()) {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                tv.open_remote();
            }
        }
    });
    bar.root.add_controller(click);
    // Opened by the bar or closed by a swipe, Escape or a click beside it: the model follows.
    sheet.connect_open_notify({
        let tv = tv.clone();
        move |sheet| {
            let Some(tv) = tv.upgrade() else { return };
            match (sheet.is_open(), tv.sheet()) {
                (true, None) if tv.casting() => tv.open_remote(),
                (false, Some(Sheet::Remote)) if tv.casting() => tv.sheet_closed(),
                _ => {}
            }
        }
    });

    // TVs are looked for while the window is in front, so the header's button shows only
    // with one there.
    let browsing = Rc::new(std::cell::Cell::new(false));
    let follow = {
        let tv = tv.clone();
        move |active: bool| {
            let Some(tv) = tv.upgrade() else { return };
            if browsing.replace(active) != active {
                if active { tv.start_browsing() } else { tv.stop_browsing() }
            }
        }
    };
    follow(ctx.window.is_active());
    ctx.window.connect_is_active_notify(move |window| follow(window.is_active()));

    let weak = Rc::downgrade(ctx);
    let draw = {
        let bar = Rc::clone(&bar);
        move || {
            let Some(ctx) = weak.upgrade() else { return };
            let casting = ctx.tv.casting();
            ctx.cast_sheet.set_reveal_bottom_bar(casting);
            ctx.cast_sheet.set_can_open(casting);
            if casting {
                bar.update(&ctx, &ctx.tv);
            }
        }
    };
    draw();
    ctx.tv.listen(draw);
    glib::timeout_add_local(TICK, {
        let bar = Rc::downgrade(&bar);
        move || {
            let (Some(bar), Some(tv)) = (bar.upgrade(), tv.upgrade()) else {
                return glib::ControlFlow::Break;
            };
            if tv.casting() && bar.root.is_mapped() {
                bar.show_playhead(&tv);
            }
            glib::ControlFlow::Continue
        }
    });
}
