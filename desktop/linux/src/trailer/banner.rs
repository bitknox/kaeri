//! A title page's trailer: it plays at the trailing end of the band once the page has settled,
//! and the art fades to the page's ground around it when a frame is playing.
//!
//! The page decides once, when it opens, whether a trailer may play here; when one may, the band
//! and the head stand where they will stay, so nothing moves when it plays. After the page's
//! transition has ended and half a second has passed, the trailer loads under the art. The video
//! is 448 by 252 at the band's trailing end, with nothing drawn over it, and the sound button
//! hangs 8 px under its trailing corner, fading with the art. Half a second before the end the art
//! comes back and the view is let go of. Scrolling the page more than 8 px brings the art back and
//! then pauses the trailer; scrolling back to the top plays it again, and the art clears at its
//! first frame. The view is kept until the page closes or the trailer ends. Leaving the page,
//! starting an episode, opening a dialog or the window losing focus lets it go at once.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_core::ids::AnilistId;

use super::{Stage, may_autoplay, preview, stage::Slot};
use crate::window::{Ctx, Size};

/// How long after the page's transition the trailer may load.
const SETTLE: Duration = Duration::from_millis(500);
/// Scrolling further than this pauses it.
const SCROLLED: f64 = 8.0;

pub struct PageTrailer {
    pub stage: Rc<Stage>,
    /// The sound button, for the page to hang under the video's corner.
    pub sound: gtk::ToggleButton,
    ctx: Rc<Ctx>,
    id: AnilistId,
    scrolled: Cell<f64>,
    settled: Cell<bool>,
    settle: RefCell<Option<glib::SourceId>>,
    handlers: RefCell<Vec<glib::SignalHandlerId>>,
}

impl PageTrailer {
    /// The trailer for the page of title `id`, whose band's art is `art`.
    pub fn new(ctx: &Rc<Ctx>, id: AnilistId, art: &impl IsA<gtk::Widget>) -> Rc<Self> {
        let stage = Stage::new(art);
        let sound = preview::sound_button();
        // A plain icon button, 36 px, never the amber one: it is not an action of the title.
        sound.set_size_request(36, 36);
        sound.set_halign(gtk::Align::End);
        sound.set_valign(gtk::Align::Start);
        sound.set_margin_top(Slot::video_bottom() + 8);
        sound.set_margin_end(Slot::video_end());
        let this = Rc::new(Self {
            stage: Rc::clone(&stage),
            sound: sound.clone(),
            ctx: Rc::clone(ctx),
            id,
            scrolled: Cell::new(0.0),
            settled: Cell::new(false),
            settle: RefCell::default(),
            handlers: RefCell::default(),
        });

        sound.connect_toggled({
            let stage = Rc::downgrade(&stage);
            move |button| {
                if let Some(stage) = stage.upgrade() {
                    stage.set_sound(button.is_active());
                }
                button.set_icon_name(preview::sound_icon(button.is_active()));
            }
        });
        // The sound button fades in and out with the art, and is there only while the art is
        // not whole.
        art.as_ref().connect_notify_local(Some("opacity"), {
            let sound = sound.downgrade();
            move |art, _| {
                if let Some(sound) = sound.upgrade() {
                    let clear = 1.0 - art.opacity();
                    sound.set_opacity(clear);
                    sound.set_visible(clear > 0.0);
                }
            }
        });
        stage.on_showing(preview::page_playing);

        stage.widget.connect_unmap({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.stop();
                    this.cancel_settle();
                }
            }
        });

        // The window losing focus or a dialog over the page lets the trailer go.
        let window = ctx.window.clone();
        let stop = {
            let this = Rc::downgrade(&this);
            move || {
                if let Some(this) = this.upgrade() {
                    this.stop();
                }
            }
        };
        let handlers = vec![
            window.connect_is_active_notify({
                let stop = stop.clone();
                move |window| {
                    if !window.is_active() {
                        stop();
                    }
                }
            }),
            window.connect_visible_dialog_notify(move |window| {
                if window.visible_dialog().is_some() {
                    stop();
                }
            }),
        ];
        this.handlers.replace(handlers);
        this
    }

    /// Let go of everything the page holds: the trailer, its timer and its hold on the window.
    pub fn release(&self) {
        self.stop();
        self.cancel_settle();
        for handler in self.handlers.take() {
            self.ctx.window.disconnect(handler);
        }
    }

    /// Let the trailer go now, the art coming back; it may start again when the page settles.
    pub fn stop(&self) {
        self.stage.stop();
    }

    fn cancel_settle(&self) {
        if let Some(source) = self.settle.take() {
            source.remove();
        }
    }

    /// The page's transition has ended: the trailer may load half a second from now.
    pub fn settle_soon(self: &Rc<Self>) {
        self.cancel_settle();
        let this = Rc::downgrade(self);
        self.settle.replace(Some(glib::timeout_add_local_once(SETTLE, move || {
            if let Some(this) = this.upgrade() {
                this.settle.take();
                this.settled.set(true);
                this.maybe_start();
            }
        })));
    }

    /// Start the trailer if everything allows it now: the page has settled and shows its top,
    /// the window is wide, focused and has no dialog, no preview is open, and the engine allows
    /// this title's trailer to start by itself.
    pub fn maybe_start(self: &Rc<Self>) {
        if !self.settled.get()
            || self.stage.active()
            || self.stage.done()
            || self.scrolled.get() > SCROLLED
            || !self.stage.widget.is_mapped()
            || self.ctx.size() != Size::Wide
            || !self.ctx.window.is_active()
            || self.ctx.window.visible_dialog().is_some()
            || preview::is_open()
        {
            return;
        }
        let Some(entry) = self.ctx.hub.entry(self.id) else { return };
        let Some(trailer) = entry.trailer.clone() else { return };
        if may_autoplay(&self.ctx.hub, &entry) {
            // Sound is off at every start.
            self.sound.set_active(false);
            self.stage.start(&trailer, &super::subtitle_language(&self.ctx.hub), Slot::Corner);
        }
    }

    /// The page scrolled to `value`: past 8 px the art comes back and the trailer pauses under
    /// it; back at the top it plays again, or starts if it had not.
    pub fn scrolled(self: &Rc<Self>, value: f64) {
        let was = self.scrolled.replace(value);
        let away = value > SCROLLED;
        if away == (was > SCROLLED) {
            return;
        }
        if away {
            self.stage.pause();
        } else if self.stage.active() {
            self.stage.resume();
        } else {
            self.maybe_start();
        }
    }

    /// The window changed size: below the wide size the trailer stops and the page keeps its art.
    pub fn resized(self: &Rc<Self>, size: Size) {
        if size == Size::Wide {
            self.maybe_start();
        } else {
            self.stop();
        }
    }
}
