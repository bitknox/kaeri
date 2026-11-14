//! The window's notices: libadwaita toasts, each added through [`Notices::add`] so that
//! [`Notices::dismiss_each`] can dismiss them all.
//!
//! libadwaita 1.7 can free a toast while one of its animations still runs, and the animation's
//! next frame then reads the freed toast and the app segfaults. `AdwToastOverlay::dismiss_all`
//! does it to a toast an alert has just put back in the queue, mid-hide. Dismissing that toast by
//! itself does it too when its hide ended before its slide-in: the overlay then drops the slide-in
//! without disconnecting its end. So `dismiss_each` waits until none of the toasts' animations can
//! be running (none lasts over 500 ms) before it dismisses anything.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::glib;

/// Frame time after the last alert came or toast went, in microseconds, by which every toast
/// animation it started has ended: libadwaita's longest is the 500 ms slide-in that replaces
/// another.
const SETTLED: i64 = 700_000;

#[derive(Clone)]
pub struct Notices {
    overlay: adw::ToastOverlay,
    shown: Rc<RefCell<Vec<glib::WeakRef<adw::Toast>>>>,
    /// The frame time an alert was last added or a toast dismissed at: either can start an
    /// animation on a toast already there. A plain notice only shows itself or waits its turn.
    stirred: Rc<Cell<i64>>,
}

impl Notices {
    pub fn new(overlay: &adw::ToastOverlay) -> Self {
        Self { overlay: overlay.clone(), shown: Rc::default(), stirred: Rc::default() }
    }

    /// The overlay the notices show in, for saying a notice's text to a screen reader.
    pub fn overlay(&self) -> &adw::ToastOverlay {
        &self.overlay
    }

    fn stir(overlay: &adw::ToastOverlay, stirred: &Cell<i64>) {
        if let Some(clock) = overlay.frame_clock() {
            stirred.set(clock.frame_time());
        }
    }

    pub fn add(&self, toast: &adw::Toast) {
        {
            let mut shown = self.shown.borrow_mut();
            shown.retain(|toast| toast.upgrade().is_some());
            shown.push(toast.downgrade());
        }
        if toast.priority() == adw::ToastPriority::High {
            Self::stir(&self.overlay, &self.stirred);
        }
        toast.connect_dismissed({
            let overlay = self.overlay.downgrade();
            let stirred = Rc::clone(&self.stirred);
            move |_| {
                if let Some(overlay) = overlay.upgrade() {
                    Self::stir(&overlay, &stirred);
                }
            }
        });
        #[allow(clippy::disallowed_methods)] // the one place a toast is added
        self.overlay.add_toast(toast.clone());
    }

    /// Dismisses every notice shown so far, once none of their animations can be running: at
    /// once when the overlay is not on screen, as libadwaita then skips its animations, or else
    /// from the first frame at least [`SETTLED`] after the last alert came or toast went.
    pub fn dismiss_each(&self) {
        let shown: Vec<adw::Toast> =
            self.shown.borrow_mut().drain(..).filter_map(|toast| toast.upgrade()).collect();
        if shown.is_empty() {
            return;
        }
        if !self.overlay.is_mapped() {
            dismiss_in_order(&shown);
            return;
        }
        let stirred = Rc::clone(&self.stirred);
        let shown = Cell::new(Some(shown));
        self.overlay.add_tick_callback(move |_, clock| {
            if clock.frame_time() < stirred.get() + SETTLED {
                return glib::ControlFlow::Continue;
            }
            // After this frame, whose ticks end the animations that are due.
            if let Some(shown) = shown.take() {
                glib::idle_add_local_once(move || dismiss_in_order(&shown));
            }
            glib::ControlFlow::Break
        });
    }
}

/// The queued ones first and the one on screen last, so that none from the queue comes up in its
/// place. An alert always takes the screen and puts the one it covers back in the queue, and
/// plain notices wait their turn, so the one on screen is the newest alert, or with none the
/// oldest plain notice: plain notices go newest first, then alerts oldest first.
fn dismiss_in_order(shown: &[adw::Toast]) {
    let (alerts, plain): (Vec<_>, Vec<_>) =
        shown.iter().partition(|toast| toast.priority() == adw::ToastPriority::High);
    for toast in plain.iter().rev().chain(&alerts) {
        toast.dismiss();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gtk_test::pump;

    fn toast(notices: &Notices, text: &str, alert: bool) -> adw::Toast {
        let toast = adw::Toast::new(text);
        if alert {
            toast.set_priority(adw::ToastPriority::High);
        }
        notices.add(&toast);
        toast
    }

    /// Two orders that free a toast while one of its animations still runs, which segfaults on
    /// the next frame. Every toast must be let go once the animations end. Freed memory is
    /// poisoned on the GTK thread, so a read of a freed toast crashes every time.
    #[test]
    fn dismissing_around_an_alert_frees_each_toast_once() {
        crate::gtk_test::run(dismissing_around_an_alert);
    }

    fn dismissing_around_an_alert() {
        let overlay = adw::ToastOverlay::new();
        overlay.set_child(Some(&gtk::Label::new(Some("page"))));
        let window = gtk::Window::builder().default_width(640).default_height(480).build();
        window.set_child(Some(&overlay));
        window.present();
        let notices = Notices::new(&overlay);
        pump(200);

        // A plain notice, then two alerts at once, as a party's joins come, then everything
        // dismissed during the hides of the two toasts the alerts covered: the shot harness's
        // order, where `dismiss_all` freed them mid-animation.
        let made: Vec<glib::WeakRef<adw::Toast>> = [
            ("You are in the room", false),
            ("Mika joined", true),
            ("Ren joined", true),
            ("Still queued", false),
        ]
        .iter()
        .map(|(text, alert)| toast(&notices, text, *alert).downgrade())
        .collect();
        pump(60);
        notices.dismiss_each();
        pump(2000);
        assert!(made.iter().all(|toast| toast.upgrade().is_none()), "a toast was kept");

        // A plain notice shown while another hides, so it slides in for 500 ms; an alert covers
        // it 50 ms in, and its 300 ms hide ends first. Dismissed then, it has no hide left, but
        // libadwaita still holds its show animation, whose end reads it.
        let first = toast(&notices, "Downloads finished", false);
        pump(100);
        first.dismiss();
        let made =
            vec![first.downgrade(), toast(&notices, "You are in the room", false).downgrade()];
        drop(first);
        pump(50);
        let made = [made, vec![toast(&notices, "Mika joined", true).downgrade()]].concat();
        pump(380);
        notices.dismiss_each();
        pump(2000);
        assert!(made.iter().all(|toast| toast.upgrade().is_none()), "a toast was kept");
        window.destroy();
    }
}
