//! A banner that plays its title's trailer behind it, for the title page and the hover preview.
//!
//! The banner shows at once and stays on top. The web view sits underneath, transparent, and the
//! banner fades away only when the page says a frame is playing, so the wait for the video is
//! never seen. Nothing is ever drawn over the video: the banner is clear by then. The web view is
//! exactly the video's 16:9, so the player never letterboxes, and it sits where the [`Slot`]
//! says: filling the stage in the hover preview, at the band's trailing end on the title page,
//! where the page's own ground shows around it once the art has gone.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use gtk::glib;
use kaeri_model::{
    catalog::Trailer,
    trailer::{AppIdentity, EmbedStyle, TrailerControl, TrailerMessage, page as engine_page},
};

use super::webkit::{Signal, Web};
use crate::motion;

/// The title page's video: 448 by 252, YouTube's 16:9, over its 200 by 200 floor.
const VIDEO_WIDTH: i32 = 448;
const VIDEO_HEIGHT: i32 = 252;
/// Where the title page's video sits: this far below the page's top (under the header bar) and
/// in from the band's trailing end.
const VIDEO_TOP: i32 = 58;
const VIDEO_END: i32 = 28;

/// The view is let go of this long after the page says the trailer is half a second from its
/// end, once the art is back over it.
const TAIL: Duration = Duration::from_millis(600);

/// Where the video sits in the stage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    /// The whole stage: the hover preview's 16:9 banner.
    Fill,
    /// The trailing end of the title page's band, at the video's own size.
    Corner,
}

impl Slot {
    /// The bottom edge of the corner video, where the sound button hangs below it.
    pub const fn video_bottom() -> i32 {
        VIDEO_TOP + VIDEO_HEIGHT
    }

    /// The trailing margin of the corner video.
    pub const fn video_end() -> i32 {
        VIDEO_END
    }
}

/// A trailer that has not played in this long is let go, with nothing said.
const GIVE_UP: Duration = Duration::from_secs(8);

/// What hears the banner clear and return.
type Showing = Rc<dyn Fn(bool)>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    /// No web view.
    Idle,
    /// A view is loading the page; the banner is still up.
    Loading,
    /// A frame is on screen and the banner is clear.
    Playing,
    /// The view is held, paused, with the banner back over it.
    Paused,
    /// It ended or was refused: not again on this visit.
    Done,
}

pub struct Stage {
    pub widget: gtk::Overlay,
    video: gtk::Box,
    banner: gtk::Widget,
    state: Cell<State>,
    web: RefCell<Option<Rc<Web>>>,
    timeout: RefCell<Option<glib::SourceId>>,
    changed: RefCell<Option<Showing>>,
    fade: RefCell<Option<adw::TimedAnimation>>,
    /// The pause has not reached the player yet: the banner is still coming back over it.
    pausing: Cell<bool>,
}

impl Stage {
    /// A stage whose size is `banner`'s: the banner is the top layer, the video the one under.
    pub fn new(banner: &impl IsA<gtk::Widget>) -> Rc<Self> {
        let widget = gtk::Overlay::new();
        let video = gtk::Box::new(gtk::Orientation::Vertical, 0);
        video.set_can_target(false);
        video.set_overflow(gtk::Overflow::Hidden);
        widget.set_child(Some(&video));
        widget.add_overlay(banner);
        widget.set_measure_overlay(banner, true);
        widget.set_clip_overlay(banner, true);
        Rc::new(Self {
            widget,
            video,
            banner: banner.clone().upcast(),
            state: Cell::new(State::Idle),
            web: RefCell::default(),
            timeout: RefCell::default(),
            changed: RefCell::default(),
            fade: RefCell::default(),
            pausing: Cell::new(false),
        })
    }

    /// Run `f` with `true` when the banner clears for a playing trailer and `false` when it
    /// returns.
    pub fn on_showing(&self, f: impl Fn(bool) + 'static) {
        self.changed.replace(Some(Rc::new(f)));
    }

    /// Whether a trailer is on screen.
    pub fn showing(&self) -> bool {
        self.state.get() == State::Playing
    }

    /// Whether a view is held at all: loading, playing or paused.
    pub fn active(&self) -> bool {
        matches!(self.state.get(), State::Loading | State::Playing | State::Paused)
    }

    /// Whether the trailer has ended or was refused on this visit.
    pub fn done(&self) -> bool {
        self.state.get() == State::Done
    }

    /// Load `trailer` under the banner, in `slot`. False when it cannot: no WebKitGTK, a trailer
    /// the player does not play, or one already held.
    /// Its captions follow `subtitles`, the playback setting's language.
    pub fn start(self: &Rc<Self>, trailer: &Trailer, subtitles: &str, slot: Slot) -> bool {
        if self.web.borrow().is_some() || self.state.get() == State::Done {
            return false;
        }
        let app = AppIdentity::new(AppIdentity::LINUX);
        let Some(page) = page(trailer, &app, subtitles) else { return false };
        let weak = Rc::downgrade(self);
        let id = trailer.id.clone();
        let started = Instant::now();
        // In a preview YouTube's own controls are hidden in its frame.
        let web = Web::new(&page, &app.base_url(), false, false, true, move |signal| {
            log(&id, started, &signal);
            if let Some(stage) = weak.upgrade() {
                stage.heard(signal);
            }
        });
        let Some(web) = web else { return false };
        match slot {
            Slot::Fill => {
                self.video.set_halign(gtk::Align::Fill);
                self.video.set_valign(gtk::Align::Fill);
                self.video.set_margin_top(0);
                self.video.set_margin_end(0);
                self.video.set_size_request(-1, -1);
            }
            Slot::Corner => {
                self.video.set_halign(gtk::Align::End);
                self.video.set_valign(gtk::Align::Start);
                self.video.set_margin_top(VIDEO_TOP);
                self.video.set_margin_end(VIDEO_END);
                self.video.set_size_request(VIDEO_WIDTH, VIDEO_HEIGHT);
            }
        }
        web.widget.set_vexpand(true);
        self.video.append(&web.widget);
        self.web.replace(Some(web));
        self.state.set(State::Loading);
        let weak = Rc::downgrade(self);
        let gave_up = (trailer.id.clone(), started);
        self.timeout.replace(Some(glib::timeout_add_local_once(GIVE_UP, move || {
            if let Some(stage) = weak.upgrade() {
                // The source is the one running: it is gone once this returns.
                stage.timeout.take();
                if stage.state.get() == State::Loading {
                    tracing::warn!(
                        trailer = %gave_up.0,
                        "gave up after {} ms, never playing",
                        gave_up.1.elapsed().as_millis()
                    );
                    stage.release(State::Idle);
                }
            }
        })));
        true
    }

    fn heard(self: &Rc<Self>, signal: Signal) {
        match signal {
            Signal::Message(TrailerMessage::Playing) => {
                let resumed = self.state.get() == State::Paused && !self.pausing.get();
                if self.state.get() == State::Loading || resumed {
                    self.cancel_timeout();
                    self.state.set(State::Playing);
                    self.fade_banner(false);
                    self.say(true);
                }
            }
            Signal::Ending => self.ending(),
            Signal::Message(TrailerMessage::Ended) => self.release(State::Done),
            Signal::Message(TrailerMessage::Error { .. }) | Signal::Failed => {
                self.release(State::Done);
            }
            Signal::Message(_) => {}
        }
    }

    /// The trailer is half a second from its end: the banner comes back over it now, and the
    /// view is let go of once the last frame has gone.
    fn ending(self: &Rc<Self>) {
        if self.state.get() != State::Playing {
            return;
        }
        self.state.set(State::Done);
        self.fade_banner(true);
        self.say(false);
        let weak = Rc::downgrade(self);
        self.timeout.replace(Some(glib::timeout_add_local_once(TAIL, move || {
            if let Some(stage) = weak.upgrade() {
                stage.timeout.take();
                stage.release(State::Done);
            }
        })));
    }

    /// Hold the trailer: the banner comes back over it, and the player pauses once it has, so its
    /// paused screen never shows.
    pub fn pause(self: &Rc<Self>) {
        if self.state.get() != State::Playing {
            return;
        }
        self.state.set(State::Paused);
        self.pausing.set(true);
        let weak = Rc::downgrade(self);
        self.fade_banner_then(true, move || {
            if let Some(stage) = weak.upgrade()
                && stage.pausing.replace(false)
            {
                stage.control(TrailerControl::Pause);
            }
        });
        self.say(false);
    }

    /// Play it again from where it paused; the banner clears when the page says a frame is
    /// playing.
    pub fn resume(self: &Rc<Self>) {
        if self.state.get() != State::Paused {
            return;
        }
        if self.pausing.replace(false) {
            // The player never paused: it is still playing, so the banner clears again.
            self.state.set(State::Playing);
            self.fade_banner(false);
            self.say(true);
            return;
        }
        self.control(TrailerControl::Play);
    }

    /// Sound on or off, for this trailer only.
    pub fn set_sound(&self, on: bool) {
        self.control(if on { TrailerControl::Unmute } else { TrailerControl::Mute });
    }

    /// Let go of the view, and bring the banner back if it was clear.
    pub fn stop(&self) {
        if self.active() {
            self.release(State::Idle);
        }
    }

    fn control(&self, control: TrailerControl) {
        if let Some(web) = self.web.borrow().as_ref() {
            web.control(control);
        }
    }

    fn release(&self, then: State) {
        self.cancel_timeout();
        self.pausing.set(false);
        let was = self.state.replace(then);
        if let Some(web) = self.web.take() {
            self.video.remove(&web.widget);
        }
        self.video.set_size_request(-1, -1);
        if matches!(was, State::Playing | State::Paused) && self.banner.opacity() < 1.0 {
            self.fade_banner(true);
        }
        if was == State::Playing {
            self.say(false);
        }
    }

    fn cancel_timeout(&self) {
        if let Some(source) = self.timeout.take() {
            source.remove();
        }
    }

    /// Fade the banner back in (`back`) or away: back over 300 ms on fade-out's curve,
    /// cubic-bezier(0.42, 0, 1, 1), and away over 200 ms on fade-in's, cubic-bezier(0, 0, 0.58, 1),
    /// the canvas tokens (libadwaita's ease-in and ease-out are those two curves).
    fn fade_banner(&self, back: bool) {
        self.fade_banner_then(back, || {});
    }

    fn fade_banner_then(&self, back: bool, then: impl Fn() + 'static) {
        if let Some(old) = self.fade.take() {
            old.pause();
        }
        let (from, to, ms, easing) = if back {
            (self.banner.opacity(), 1.0, 300, adw::Easing::EaseIn)
        } else {
            (self.banner.opacity(), 0.0, 200, adw::Easing::EaseOut)
        };
        let animation =
            motion::timed(&self.banner, from, to, ms, motion::opacity(&self.banner));
        animation.set_easing(easing);
        animation.connect_done(move |_| then());
        animation.play();
        self.fade.replace(Some(animation));
    }

    fn say(&self, showing: bool) {
        let changed = self.changed.borrow().clone();
        if let Some(changed) = changed {
            changed(showing);
        }
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if let Some(source) = self.timeout.take() {
            source.remove();
        }
    }
}

/// The page to load: the engine's, and in the test kit's scene a local stand-in that plays a
/// made-up clip with no network.
fn page(trailer: &Trailer, app: &AppIdentity, subtitles: &str) -> Option<String> {
    #[cfg(feature = "fixtures")]
    if crate::fixtures::active() {
        return Some(fixture_page());
    }
    engine_page(Some(trailer), EmbedStyle::Banner, app, subtitles)
}

/// What the page said, for the log: a playing frame and every refusal.
pub(super) fn log(id: &str, started: Instant, signal: &Signal) {
    let ms = started.elapsed().as_millis();
    match signal {
        Signal::Message(TrailerMessage::Playing) => {
            tracing::debug!(trailer = %id, "playing, shown after {ms} ms");
        }
        Signal::Message(TrailerMessage::Error { code }) => {
            tracing::warn!(
                trailer = %id,
                "failed after {ms} ms: error {code}, {}",
                super::meaning(*code)
            );
        }
        Signal::Failed => tracing::warn!(trailer = %id, "the trailer's web view failed"),
        _ => {}
    }
}

/// A page that says what the real one would, for the shots and for trying the app offline: the
/// boards' flat stand-in in place of the video (a pale sun crossing grey hills), `ready` at once,
/// `playing` a moment later, `ending` half a second before its ten seconds are up and `ended` at
/// the end. Pause and play stop and start its clock, and each play says `playing` again.
#[cfg(feature = "fixtures")]
const FIXTURE_PAGE: &str = r##"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}
#clip{position:absolute;inset:0;overflow:hidden;background:#4a4f63}
#sun{position:absolute;left:6%;top:16%;width:15%;height:26.67%;border-radius:50%;background:#8c92a7;
animation:sun 12s linear infinite}
#hills{position:absolute;left:0;bottom:0;width:200%;height:46%;animation:drift 6s linear infinite}
#hills svg{display:block;width:100%;height:100%}
.held #sun,.held #hills{animation-play-state:paused}
@keyframes sun{from{transform:translateX(0)}to{transform:translateX(480%)}}
@keyframes drift{from{transform:translateX(0)}to{transform:translateX(-50%)}}
</style></head><body><div id="clip" class="held"><div id="sun"></div><div id="hills"><svg viewBox="0 0 400 100" preserveAspectRatio="none"><path d="M0 60 L40 35 L80 55 L120 25 L160 50 L200 60 L240 35 L280 55 L320 25 L360 50 L400 60 V100 H0 Z" fill="#3b4054"/><path d="M0 80 L50 68 L100 82 L150 70 L200 80 L250 68 L300 82 L350 70 L400 80 V100 H0 Z" fill="#2f3345"/></svg></div></div><script>
function say(m){try{window.webkit.messageHandlers.kaeri.postMessage(m)}catch(e){}}
var LENGTH=10000,played=0,since=null,timers=[],clip=document.getElementById("clip");
function at(ms,f){timers.push(setTimeout(f,Math.max(0,ms-played)))}
function play(){if(since!==null||played>=LENGTH)return;since=Date.now();clip.className="";
at(0,function(){say("playing")});
at(LENGTH-500,function(){say("ending")});
at(LENGTH,function(){hold();say("ended")})}
function hold(){if(since===null)return;played+=Date.now()-since;since=null;clip.className="held";
timers.forEach(clearTimeout);timers=[]}
window.trailer={play:play,pause:function(){hold();say("paused")},mute:function(){},unmute:function(){}};
say("ready");setTimeout(play,700);
</script></body></html>"##; // i18n-skip

#[cfg(feature = "fixtures")]
pub fn fixture_page() -> String {
    FIXTURE_PAGE.to_owned()
}
