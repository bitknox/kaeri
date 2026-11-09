//! The motion tokens, as libadwaita animations: quick and standard are critically damped
//! springs (no bounce), fade-in and fade-out are timed. libadwaita reads `gtk-enable-animations`
//! itself, so with GNOME's Reduce Animation every one of these jumps to its end.

use adw::prelude::*;

/// A spring of zero bounce with `stiffness`: 1000 for quick, 440 for standard, the stiffness of
/// zero-bounce springs of 0.2 s and 0.3 s. Made with the builder, as `timed` explains.
fn spring(
    widget: &impl IsA<gtk::Widget>,
    from: f64,
    to: f64,
    stiffness: f64,
    target: impl IsA<adw::AnimationTarget>,
) -> adw::SpringAnimation {
    adw::SpringAnimation::builder()
        .widget(widget)
        .value_from(from)
        .value_to(to)
        .spring_params(&adw::SpringParams::new(1.0, 1.0, stiffness))
        .target(&target)
        .clamp(true)
        .build()
}

/// A timed animation of `widget` from `from` to `to` over `ms`. Every timed animation is made
/// here: the builder hands back the only reference, so the animation, its target and whatever
/// its `done` handler holds all go once the last owner lets go, where libadwaita-rs's
/// `TimedAnimation::new` keeps one reference too many and they live as long as the app.
pub fn timed(
    widget: &impl IsA<gtk::Widget>,
    from: f64,
    to: f64,
    ms: u32,
    target: impl IsA<adw::AnimationTarget>,
) -> adw::TimedAnimation {
    adw::TimedAnimation::builder()
        .widget(widget)
        .value_from(from)
        .value_to(to)
        .duration(ms)
        .target(&target)
        .build()
}

/// Selection tint, toggles, a row dimming after it is marked watched.
pub fn quick(
    widget: &impl IsA<gtk::Widget>,
    from: f64,
    to: f64,
    target: impl IsA<adw::AnimationTarget>,
) -> adw::SpringAnimation {
    spring(widget, from, to, 1000.0, target)
}

/// Content changing inside a screen: a crossfade from placeholders, cards appearing.
pub fn standard(
    widget: &impl IsA<gtk::Widget>,
    from: f64,
    to: f64,
    target: impl IsA<adw::AnimationTarget>,
) -> adw::SpringAnimation {
    spring(widget, from, to, 440.0, target)
}

/// `widget`'s opacity, as an animation target.
pub fn opacity(widget: &impl IsA<gtk::Widget>) -> adw::PropertyAnimationTarget {
    adw::PropertyAnimationTarget::new(widget.as_ref(), "opacity")
}

/// Fade `widget` in from clear: ease-out, 200 ms. Art arriving, a line appearing.
pub fn fade_in(widget: &impl IsA<gtk::Widget>) -> adw::TimedAnimation {
    widget.set_opacity(0.0);
    let animation = timed(widget, 0.0, 1.0, 200, opacity(widget));
    animation.set_easing(adw::Easing::EaseOutCubic);
    animation.play();
    animation
}

/// Fade `widget` out: ease-in, 300 ms, then hide it.
pub fn fade_out(widget: &impl IsA<gtk::Widget>) -> adw::TimedAnimation {
    let from = widget.opacity();
    let animation = timed(widget, from, 0.0, 300, opacity(widget));
    animation.set_easing(adw::Easing::EaseInCubic);
    let hidden = widget.as_ref().clone();
    animation.connect_done(move |_| {
        hidden.set_visible(false);
        hidden.set_opacity(1.0);
    });
    animation.play();
    animation
}

/// Fade `widget` out on the fade-out token and leave it clear, then run `done`: for a card whose
/// place closes after it, so it keeps its room until then.
pub fn fade_away(
    widget: &impl IsA<gtk::Widget>,
    done: impl Fn() + 'static,
) -> adw::TimedAnimation {
    let from = widget.opacity();
    let animation = timed(widget, from, 0.0, 300, opacity(widget));
    animation.set_easing(adw::Easing::EaseInCubic);
    animation.connect_done(move |_| done());
    animation.play();
    animation
}

/// Run `start` once `widget` is on screen: libadwaita skips an animation straight to its end
/// when its widget is not mapped, so one started on a widget just made would only jump.
pub fn once_mapped(widget: &impl IsA<gtk::Widget>, start: impl FnOnce() + 'static) {
    let widget = widget.as_ref();
    if widget.is_mapped() {
        start();
        return;
    }
    let start = std::cell::Cell::new(Some(start));
    let handler: std::rc::Rc<std::cell::Cell<Option<gtk::glib::SignalHandlerId>>> =
        std::rc::Rc::default();
    let id = widget.connect_map({
        let handler = std::rc::Rc::clone(&handler);
        move |widget| {
            if let Some(id) = handler.take() {
                widget.disconnect(id);
            }
            if let Some(start) = start.take() {
                start();
            }
        }
    });
    handler.set(Some(id));
}

/// Bring `widget` in with the standard spring on its opacity.
pub fn crossfade_in(widget: &impl IsA<gtk::Widget>) -> adw::SpringAnimation {
    widget.set_opacity(0.0);
    let animation = standard(widget, 0.0, 1.0, opacity(widget));
    animation.play();
    animation
}

/// One widget shown and hidden with the fade tokens: in over 200 ms ease-out, out over 300 ms
/// ease-in and then hidden. A change of mind takes over from wherever the last fade had got to,
/// so a card asked back while it fades out never blinks.
#[derive(Default)]
pub struct Fader {
    running: std::cell::RefCell<Option<adw::TimedAnimation>>,
    shown: std::cell::Cell<bool>,
    always: bool,
}

impl Fader {
    /// A fader that keeps its fades under Reduce Animation, where a fade is what reduced motion
    /// keeps: the reader's chrome.
    pub fn always() -> Self {
        Self { always: true, ..Self::default() }
    }

    /// Whether `widget` is shown or on its way in.
    pub fn shown(&self) -> bool {
        self.shown.get()
    }

    /// Fade `widget` in or out, unless it is already showing or going that way.
    pub fn set(&self, widget: &impl IsA<gtk::Widget>, show: bool) {
        let widget = widget.as_ref();
        if self.shown.get() == show
            && (show == widget.is_visible() || self.running.borrow().is_some())
        {
            return;
        }
        self.shown.set(show);
        if let Some(old) = self.running.take() {
            old.pause();
        }
        let from = if widget.is_visible() { widget.opacity() } else { 0.0 };
        widget.set_opacity(from);
        widget.set_visible(true);
        let (to, ms, easing) = if show {
            (1.0, 200, adw::Easing::EaseOutCubic)
        } else {
            (0.0, 300, adw::Easing::EaseInCubic)
        };
        let animation = timed(widget, from, to, ms, opacity(widget));
        animation.set_easing(easing);
        animation.set_follow_enable_animations_setting(!self.always);
        let done = widget.clone();
        animation.connect_done(move |_| {
            if !show {
                done.set_visible(false);
                done.set_opacity(1.0);
            }
        });
        animation.play();
        self.running.replace(Some(animation));
    }

    /// Hide `widget` at once, with nothing left running.
    pub fn hide_now(&self, widget: &impl IsA<gtk::Widget>) {
        if let Some(old) = self.running.take() {
            old.pause();
        }
        self.shown.set(false);
        widget.as_ref().set_visible(false);
        widget.as_ref().set_opacity(1.0);
    }
}

/// Reduce Animation is on.
pub fn reduced() -> bool {
    !gtk::Settings::default().is_some_and(|s| s.is_gtk_enable_animations())
}

/// `change` is made to `of` with a picture of it as it stood laid over it in `veil`, fading away
/// on the standard token: both sides are drawn at once, so it is never empty. With `always` the
/// crossfade plays under Reduce Animation too, where it is what reduced motion substitutes.
pub fn ghost(
    veil: &gtk::Overlay,
    of: &impl IsA<gtk::Widget>,
    always: bool,
    change: impl FnOnce(),
) {
    let of = of.as_ref();
    let before = gtk::WidgetPaintable::new(Some(of)).current_image();
    let ghost = gtk::Picture::for_paintable(&before);
    ghost.set_can_target(false);
    ghost.set_halign(gtk::Align::Fill);
    ghost.set_valign(gtk::Align::Start);
    ghost.set_content_fit(gtk::ContentFit::Fill);
    change();
    if !of.is_mapped() {
        return;
    }
    veil.add_overlay(&ghost);
    let (veil, gone) = (veil.clone(), ghost.clone());
    let fade = timed(&ghost, 1.0, 0.0, 300, opacity(&ghost));
    fade.set_easing(adw::Easing::EaseOutCubic);
    fade.set_follow_enable_animations_setting(!always);
    fade.connect_done(move |_| veil.remove_overlay(&gone));
    fade.play();
}

/// A page turn in the reader: `change` is made to `of` under a picture of it as it stood, which
/// fades away on the quick token, so the old page stays until the new one is drawn and the turn
/// never passes through an empty frame. A turn that starts while the last one fades takes its
/// place. Under Reduce Animation the change is instant.
pub fn turn(veil: &gtk::Overlay, of: &impl IsA<gtk::Widget>, change: impl FnOnce()) {
    let of = of.as_ref();
    // The last turn's picture goes at once: the new one is taken of what is drawn now.
    let mut child = veil.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if widget.has_css_class("turn-ghost") {
            // Unparenting unmaps it, which ends its fade there and then: the class goes first
            // so the fade's end doesn't remove it a second time from inside this removal.
            widget.remove_css_class("turn-ghost");
            veil.remove_overlay(&widget);
        }
    }
    if reduced() || !of.is_mapped() || of.width() <= 0 {
        change();
        return;
    }
    let before = gtk::WidgetPaintable::new(Some(of)).current_image();
    let ghost = gtk::Picture::for_paintable(&before);
    ghost.add_css_class("turn-ghost");
    ghost.set_can_target(false);
    ghost.set_halign(gtk::Align::Fill);
    ghost.set_valign(gtk::Align::Fill);
    ghost.set_content_fit(gtk::ContentFit::Fill);
    change();
    veil.add_overlay(&ghost);
    let (veil, gone) = (veil.clone(), ghost.clone());
    let fade = quick(&ghost, 1.0, 0.0, opacity(&ghost));
    fade.connect_done(move |_| {
        if gone.has_css_class("turn-ghost") {
            gone.remove_css_class("turn-ghost");
            veil.remove_overlay(&gone);
        }
    });
    fade.play();
}
