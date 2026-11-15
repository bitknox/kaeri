//! The title page while its next episode has aired and no source lists it. Check again and the
//! episode before it lead the button row, and the status sits under the row: when it aired and
//! what the sources said. A check the viewer asks for shows nothing for 600 ms, then Checking
//! inside the button, which keeps its width and holds it at least 0.3 s. Once a source lists the
//! episode, the buttons and the status crossfade to the amber Play in place while the row's width
//! and the status's height settle on ease-out cubic, so the head never passes through empty.
//!
//! The page builds its head again on most events. What runs here is kept as values (how far
//! Checking, the status's words and Play have come in) and laid on whatever widgets the head has
//! now, so a rebuild in the middle of a crossfade carries on from where it was.

use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    missing::{CheckOutcome, EpisodeCheck, MissingCommand, MissingEpisode, MissingPhase},
};

use super::Page;
use crate::{missing::Status, motion, say};

/// How long a check runs before Checking shows: most sources answer inside it.
const QUIET: Duration = Duration::from_millis(600);
/// How long Checking stays once it shows, so it never flashes.
const HELD: Duration = Duration::from_millis(300);
/// The reveal token: the crossfades and the settling width and height.
const REVEAL: u32 = 300;
/// Checking fading in.
const FADE_IN: u32 = 200;

/// The names the head's buttons carry, so focus finds its way back after a rebuild.
const CHECK: &str = "missing-check";
const PREVIOUS: &str = "missing-previous";
pub(super) const PLAY: &str = "title-play";

/// What the page keeps of a missing episode between rebuilds of its head.
#[derive(Default)]
pub(super) struct Missing {
    /// The episode as the status shows it. Kept through the crossfade to Play, and while a check
    /// the viewer asked for has yet to answer.
    shown: RefCell<Option<MissingEpisode>>,
    /// What the last check the viewer asked for came to, for the episode shown.
    answer: Cell<Option<CheckOutcome>>,
    /// A check the viewer asked for is running, since then.
    asked: Cell<Option<Instant>>,
    /// Checking showed, at this moment.
    checking_at: Cell<Option<Instant>>,
    /// How far Checking has come in, 0 to 1.
    busy: Cell<f64>,
    /// The status as it stood before an answer changed its words, while they crossfade.
    status_from: RefCell<Option<Status>>,
    /// How far the new words have come in, 0 to 1.
    status_mix: Cell<f64>,
    /// How far Play has come in, while it crossfades in: `None` the rest of the time.
    found: Cell<Option<f64>>,
    /// The width of the buttons and the height of the status Play takes the place of.
    found_from: Cell<(i32, i32)>,
    /// A source listed the episode while a check ran: Play comes once the check has answered.
    found_waits: Cell<bool>,
    /// The episode whose find was said, so it is said once.
    said_found: Cell<Option<u32>>,
    busy_run: RefCell<Option<adw::TimedAnimation>>,
    status_run: RefCell<Option<adw::TimedAnimation>>,
    found_run: RefCell<Option<adw::TimedAnimation>>,
    /// The widgets the head has now.
    pub(super) parts: RefCell<Option<Parts>>,
}

/// The head's widgets for a missing episode, as last built.
pub(super) struct Parts {
    narrow: bool,
    /// Check again's two faces, one over the other.
    idle: Option<gtk::Widget>,
    busy: Option<gtk::Widget>,
    check: Option<gtk::Button>,
    /// While Play comes in, the frame over the buttons and Play, one over the other, whose
    /// width settles from the buttons' to Play's.
    frame: Option<gtk::ScrolledWindow>,
    miss: gtk::Box,
    play: Option<gtk::Button>,
    /// The status line, and the words it had while they crossfade.
    line: gtk::Overlay,
    label: gtk::Label,
    label_from: Option<gtk::Label>,
    /// The status's frame while Play comes in, which closes its height.
    clip: Option<gtk::ScrolledWindow>,
}

/// Now, in Unix seconds.
pub(super) fn now() -> i64 {
    glib::DateTime::now_utc().map_or(0, |t| t.to_unix())
}

/// A day as the page's rows write it: "7 Nov 2026".
pub(super) fn short_date(at: i64) -> String {
    glib::DateTime::from_unix_local(at)
        .ok()
        .and_then(|d| d.format("%-d %b %Y").ok())
        .map_or_else(String::new, |d| d.to_string())
}

fn lerp(from: i32, to: i32, by: f64) -> i32 {
    (f64::from(from) + f64::from(to - from) * by).round() as i32
}

/// The status line's words: the head in silk and semibold, the rest in haze.
fn status_label(status: &Status) -> gtk::Label {
    let text = status.text();
    let label = gtk::Label::new(Some(&text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_hexpand(true);
    let attrs = gtk::pango::AttrList::new();
    let head = status.head_len() as u32;
    if head > 0 {
        let mut weight = gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Semibold);
        weight.set_start_index(0);
        weight.set_end_index(head);
        attrs.insert(weight);
    }
    let mut haze = gtk::pango::AttrColor::new_foreground(0xA3A3, 0xA9A9, 0xC4C4);
    haze.set_start_index(head);
    haze.set_end_index(text.len() as u32);
    attrs.insert(haze);
    label.set_attributes(Some(&attrs));
    label
}

/// A frame around `child` that clips it along `along` to the size the frame is asked for, and
/// shows no scroll bar: the child keeps its natural size and is cut, never squeezed.
fn clipping(child: &impl IsA<gtk::Widget>, along: gtk::Orientation) -> gtk::ScrolledWindow {
    let viewport = gtk::Viewport::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    viewport.set_child(Some(child));
    viewport.set_hscroll_policy(gtk::ScrollablePolicy::Natural);
    viewport.set_vscroll_policy(gtk::ScrollablePolicy::Natural);
    let horizontal = along == gtk::Orientation::Horizontal;
    let frame = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(if horizontal {
            gtk::PolicyType::External
        } else {
            gtk::PolicyType::Never
        })
        .vscrollbar_policy(if horizontal {
            gtk::PolicyType::Never
        } else {
            gtk::PolicyType::External
        })
        .propagate_natural_width(!horizontal)
        .propagate_natural_height(horizontal)
        .child(&viewport)
        .build();
    frame.set_can_focus(false);
    frame
}

/// The widget named `name` under `root`, when there is one that can take focus.
fn named(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name && root.is_focusable() {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = named(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

impl Page {
    /// The missing episode the rows and the progress say now, set as the one shown. When a
    /// source lists the one shown, Play crossfades in, or once a running check has answered.
    pub(super) fn track_missing(&self) {
        let Some(entry) = self.ctx.hub.entry(self.id) else { return };
        let (computed, plays) = {
            let rows = self.rows.borrow();
            if rows.is_empty() {
                return;
            }
            let item = say::queue_item(&entry, Some(rows.as_slice()));
            let computed = kaeri_model::missing::missing_in(
                &rows,
                item.episode,
                item.resume.is_some(),
                now(),
            );
            (computed, item.button.is_some().then_some(item.episode))
        };
        let m = &self.missing;
        let shown = m.shown.borrow().clone();
        match (shown, computed) {
            (shown, Some(computed)) => {
                if shown.as_ref().map(|s| s.episode) != Some(computed.episode) {
                    m.answer.set(None);
                    m.status_from.replace(None);
                }
                if let Some(run) = m.found_run.take() {
                    run.pause();
                    m.found.set(None);
                }
                m.found_waits.set(false);
                m.shown.replace(Some(computed));
            }
            (Some(shown), None) if m.found.get().is_none() && !m.found_waits.get() => {
                if plays != Some(shown.episode) {
                    // Something else changed what plays next (a mark): the page as it is.
                    m.shown.replace(None);
                    m.answer.set(None);
                } else if m.asked.get().is_some() {
                    m.found_waits.set(true);
                } else {
                    self.start_found();
                }
            }
            _ => {}
        }
    }

    /// Bring the amber Play in: the buttons and the status crossfade to it in place, the row's
    /// width and the status's height settling. Under Reduce Animation the screen crossfades.
    fn start_found(&self) {
        let m = &self.missing;
        m.found_waits.set(false);
        let done = || {
            m.shown.replace(None);
            m.answer.set(None);
            m.status_from.replace(None);
            self.redress();
        };
        if !self.head.is_mapped() {
            done();
            return;
        }
        if motion::reduced() {
            self.crossfade_always(done);
            return;
        }
        // What Play takes the place of, as it stands on screen.
        let from = m.parts.borrow().as_ref().map_or((0, 0), |p| {
            let width = if p.miss.width() > 0 {
                p.miss.width()
            } else {
                p.miss.measure(gtk::Orientation::Horizontal, -1).1
            };
            let height = if p.line.height() > 0 {
                p.line.height() + p.line.margin_top()
            } else {
                let width = self.head.width().max(400);
                p.line.measure(gtk::Orientation::Vertical, width).1 + p.line.margin_top()
            };
            (width, height)
        });
        m.found_from.set(from);
        m.found.set(Some(0.0));
        self.redress();
        let weak = self.me.borrow().clone();
        let run = self.missing_run(0.0, 1.0, REVEAL, |m, v| m.found.set(Some(v)));
        run.connect_done(move |_| {
            if let Some(page) = weak.upgrade() {
                let m = &page.missing;
                m.found_run.take();
                m.found.set(None);
                m.shown.replace(None);
                m.answer.set(None);
                m.status_from.replace(None);
                page.redress();
            }
        });
        run.play();
        if let Some(old) = m.found_run.replace(Some(run)) {
            old.pause();
        }
    }

    /// An animation of one of the values on the reveal curve, laid on the head as it goes.
    fn missing_run(
        &self,
        from: f64,
        to: f64,
        ms: u32,
        set: fn(&Missing, f64),
    ) -> adw::TimedAnimation {
        let weak = self.me.borrow().clone();
        let target = adw::CallbackAnimationTarget::new(move |value| {
            if let Some(page) = weak.upgrade() {
                set(&page.missing, value);
                page.apply_missing();
            }
        });
        let run = motion::timed(&self.head, from, to, ms, target);
        run.set_easing(adw::Easing::EaseOutCubic);
        run
    }

    /// Check again: ask the sources for the episode now. Nothing shows for 600 ms; a slower
    /// answer shows Checking inside the button.
    fn check_again(&self) {
        let Some(missing) = self.missing.shown.borrow().clone() else { return };
        if self.missing.asked.get().is_some() || self.missing.found.get().is_some() {
            return;
        }
        let asked = Instant::now();
        self.missing.asked.set(Some(asked));
        self.ctx.hub.send(Command::Missing(MissingCommand::CheckAgain {
            id: self.id,
            episode: missing.episode,
        }));
        let weak = self.me.borrow().clone();
        glib::timeout_add_local_once(QUIET, move || {
            if let Some(page) = weak.upgrade()
                && page.missing.asked.get() == Some(asked)
            {
                page.show_checking();
            }
        });
    }

    /// Checking fades in inside the button, and a screen reader hears it.
    fn show_checking(&self) {
        let m = &self.missing;
        m.checking_at.set(Some(Instant::now()));
        let run = self.missing_run(m.busy.get(), 1.0, FADE_IN, |m, v| m.busy.set(v));
        // A fade is what Reduce Animation keeps.
        run.set_follow_enable_animations_setting(false);
        run.play();
        if let Some(old) = m.busy_run.replace(Some(run)) {
            old.pause();
        }
        self.say_checking();
        self.announce(&t::action_checking());
    }

    /// What a check came to. One the viewer asked for answers once Checking has shown long
    /// enough; a quiet recheck that found the episode is said, and Play is already on its way.
    pub(super) fn heard_check(&self, check: &EpisodeCheck) {
        if !check.asked {
            if check.outcome == CheckOutcome::Found {
                self.say_found(check.episode);
            }
            return;
        }
        if self.missing.asked.get().is_none() {
            return;
        }
        let held = self
            .missing
            .checking_at
            .get()
            .map_or(Duration::ZERO, |at| HELD.saturating_sub(at.elapsed()));
        if held.is_zero() {
            self.answered(check.episode, check.outcome);
            return;
        }
        let weak = self.me.borrow().clone();
        let (episode, outcome) = (check.episode, check.outcome);
        glib::timeout_add_local_once(held, move || {
            if let Some(page) = weak.upgrade() {
                page.answered(episode, outcome);
            }
        });
    }

    /// The answer lands: Checking crossfades back to Check again, the words change if the answer
    /// changed them, a screen reader hears it once, and a find brings Play in.
    fn answered(&self, episode: u32, outcome: CheckOutcome) {
        let m = &self.missing;
        m.asked.set(None);
        if m.checking_at.take().is_some() {
            let run = self.missing_run(m.busy.get(), 0.0, REVEAL, |m, v| m.busy.set(v));
            run.set_follow_enable_animations_setting(false);
            run.play();
            if let Some(old) = m.busy_run.replace(Some(run)) {
                old.pause();
            }
            self.say_checking();
        }
        if outcome == CheckOutcome::Found || m.found_waits.get() {
            self.say_found(episode);
            if m.found_waits.get() {
                self.start_found();
            }
            return;
        }
        let Some(missing) = m.shown.borrow().clone().filter(|s| s.episode == episode) else {
            return;
        };
        let before = self.status_of(&missing);
        m.answer.set(Some(outcome));
        let after = self.status_of(&missing);
        if before != after {
            self.change_words(before);
        }
        self.announce(&crate::missing::answer_said(episode, outcome));
    }

    /// The status's words crossfade from `before` to what they say now.
    fn change_words(&self, before: Status) {
        let m = &self.missing;
        m.status_from.replace(Some(before));
        m.status_mix.set(0.0);
        self.redress();
        let weak = self.me.borrow().clone();
        let run = self.missing_run(0.0, 1.0, REVEAL, |m, v| m.status_mix.set(v));
        run.set_follow_enable_animations_setting(false);
        run.connect_done(move |_| {
            if let Some(page) = weak.upgrade() {
                page.missing.status_from.replace(None);
                page.missing.status_run.take();
                if let Some(old) =
                    page.missing.parts.borrow().as_ref().and_then(|p| p.label_from.clone())
                {
                    old.set_visible(false);
                }
            }
        });
        run.play();
        if let Some(old) = m.status_run.replace(Some(run)) {
            old.pause();
        }
    }

    /// "Episode 6 is in your sources now", once, while the page is open.
    fn say_found(&self, episode: u32) {
        if self.missing.said_found.replace(Some(episode)) != Some(episode) {
            self.announce(&crate::missing::answer_said(episode, CheckOutcome::Found));
        }
    }

    /// A polite announcement from the page.
    fn announce(&self, words: &str) {
        let from = self.missing.parts.borrow().as_ref().map_or_else(
            || self.head.clone().upcast::<gtk::Widget>(),
            |p| p.label.clone().upcast(),
        );
        from.announce(words, gtk::AccessibleAnnouncementPriority::Medium);
    }

    /// Check again's name for a screen reader, Checking while that shows.
    fn say_checking(&self) {
        let busy = self.missing.checking_at.get().is_some();
        if let Some(check) = self.missing.parts.borrow().as_ref().and_then(|p| p.check.clone())
        {
            let name = if busy { t::action_checking() } else { t::action_check_again() };
            check.update_property(&[gtk::accessible::Property::Label(&name)]);
        }
    }

    /// The device went off or on line: the words follow, and an offline answer is forgotten.
    pub(super) fn connectivity(&self, offline: bool) {
        if !offline && self.missing.answer.get() == Some(CheckOutcome::Offline) {
            self.missing.answer.set(None);
        }
        if self.missing.shown.borrow().is_some() {
            self.redress();
        }
    }

    /// The status of `missing` as the page would say it now.
    fn status_of(&self, missing: &MissingEpisode) -> Status {
        crate::missing::status(
            missing,
            self.missing.answer.get(),
            // The hub hears connectivity before any page.
            self.ctx.hub.known.borrow().offline,
            now(),
            short_date,
        )
    }

    /// The episode shown as missing, for the head to draw.
    pub(super) fn missing_shown(&self) -> Option<MissingEpisode> {
        self.missing.shown.borrow().clone()
    }

    /// Whether the amber Play is crossfading in.
    pub(super) fn found_running(&self) -> bool {
        self.missing.found.get().is_some()
    }

    /// Whether a source lists the episode and Play waits for the check asked for to answer.
    pub(super) fn found_waiting(&self) -> bool {
        self.missing.found_waits.get()
    }

    /// The buttons that lead the row and the status under it, for `missing`. `play` is the amber
    /// Play, while it crossfades in.
    pub(super) fn missing_widgets(
        &self,
        missing: &MissingEpisode,
        play: Option<gtk::Button>,
        narrow: bool,
    ) -> (Option<gtk::Widget>, gtk::Widget) {
        let miss = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let (mut idle, mut busy, mut check) = (None, None, None);
        if missing.checkable() {
            let (button, a, b) = self.check_button(missing);
            button.set_hexpand(narrow);
            miss.append(&button);
            (idle, busy, check) = (Some(a), Some(b), Some(button));
        }
        if let Some(previous) = missing.previous {
            let text = gtk::Label::new(Some(&t::title_button_play(i64::from(previous))));
            text.set_ellipsize(gtk::pango::EllipsizeMode::End);
            let button = gtk::Button::builder().child(&text).build();
            button.add_css_class("pill");
            button.set_widget_name(PREVIOUS);
            button.set_action_target_value(Some(
                &(self.id.get(), previous.to_string()).to_variant(),
            ));
            button.set_action_name(Some("win.play"));
            button.set_hexpand(narrow);
            miss.append(&button);
        }
        miss.set_homogeneous(narrow);

        let found = self.missing.found.get();
        // While Play comes in, both in one cell of a frame that clips them: each keeps its own
        // width, and the frame's settles between the two.
        let frame = found.map(|_| {
            let cell = gtk::Grid::new();
            let align = if narrow { gtk::Align::Fill } else { gtk::Align::Start };
            miss.set_halign(align);
            cell.attach(&miss, 0, 0, 1, 1);
            if let Some(play) = &play {
                play.set_halign(align);
                cell.attach(play, 0, 0, 1, 1);
            }
            miss.set_can_focus(false);
            miss.set_can_target(false);
            clipping(&cell, gtk::Orientation::Horizontal)
        });
        let lead: Option<gtk::Widget> = match &frame {
            Some(frame) => {
                frame.set_hexpand(narrow);
                Some(frame.clone().upcast())
            }
            None => miss.first_child().is_some().then(|| miss.clone().upcast()),
        };
        if let Some(lead) = &lead {
            lead.set_valign(gtk::Align::Center);
            lead.set_hexpand(narrow);
        }

        let status = self.status_of(missing);
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("state-dot");
        if missing.phase == MissingPhase::LongAgo {
            dot.add_css_class("long-ago");
        }
        // On the first line of 20 px.
        dot.set_valign(gtk::Align::Start);
        dot.set_margin_top(6);
        // The words lead the line and give it its size, the dot sits before them on the first
        // line, and the words they had lie over them while they crossfade: an overlay asks each
        // for its height at the width it gets, where a row of boxes would not.
        let label = status_label(&status);
        label.set_margin_start(16);
        let line = gtk::Overlay::new();
        line.set_child(Some(&label));
        dot.set_halign(gtk::Align::Start);
        line.add_overlay(&dot);
        let label_from = self.missing.status_from.borrow().as_ref().map(|before| {
            let old = status_label(before);
            old.set_margin_start(16);
            line.add_overlay(&old);
            line.set_measure_overlay(&old, true);
            old
        });
        // In the clip, so its height closes with the line's.
        line.set_margin_top(10);
        let clip = found.map(|_| clipping(&line, gtk::Orientation::Vertical));
        let under: gtk::Widget =
            clip.as_ref().map_or_else(|| line.clone().upcast(), |c| c.clone().upcast());
        self.missing.parts.replace(Some(Parts {
            narrow,
            idle,
            busy,
            check,
            frame,
            miss,
            play,
            line,
            label,
            label_from,
            clip,
        }));
        (lead, under)
    }

    /// Check again, with Checking and its spinner over it, faded out: the button is as wide as
    /// the wider of the two from the start, so it keeps its width.
    fn check_button(
        &self,
        missing: &MissingEpisode,
    ) -> (gtk::Button, gtk::Widget, gtk::Widget) {
        let idle = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        idle.set_halign(gtk::Align::Center);
        idle.append(&gtk::Image::from_icon_name("view-refresh-symbolic"));
        idle.append(&gtk::Label::new(Some(&t::action_check_again())));
        let busy = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        busy.set_halign(gtk::Align::Center);
        let spinner = adw::Spinner::new();
        spinner.set_size_request(16, 16);
        busy.append(&spinner);
        busy.append(&gtk::Label::new(Some(&t::action_checking())));
        let faces = gtk::Grid::new();
        faces.attach(&idle, 0, 0, 1, 1);
        faces.attach(&busy, 0, 0, 1, 1);
        let button = gtk::Button::builder().child(&faces).build();
        button.add_css_class("pill");
        button.add_css_class("first-focus");
        button.set_widget_name(CHECK);
        let help = crate::missing::check_help(missing);
        button.set_tooltip_text(help.as_deref());
        let busy_now = self.missing.checking_at.get().is_some();
        let name = if busy_now { t::action_checking() } else { t::action_check_again() };
        button.update_property(&[
            gtk::accessible::Property::Label(&name),
            gtk::accessible::Property::Description(help.as_deref().unwrap_or_default()),
        ]);
        let weak = self.me.borrow().clone();
        button.connect_clicked(move |_| {
            if let Some(page) = weak.upgrade() {
                page.check_again();
            }
        });
        (button, idle.upcast(), busy.upcast())
    }

    /// Lay the running values on the head's widgets.
    pub(super) fn apply_missing(&self) {
        let m = &self.missing;
        let parts = m.parts.borrow();
        let Some(p) = parts.as_ref() else { return };
        if let (Some(idle), Some(busy)) = (&p.idle, &p.busy) {
            let by = m.busy.get();
            idle.set_opacity(1.0 - by);
            busy.set_opacity(by);
        }
        if let Some(old) = &p.label_from {
            let by = m.status_mix.get();
            old.set_opacity(1.0 - by);
            p.label.set_opacity(by);
        }
        if let Some(by) = m.found.get() {
            let (width, height) = m.found_from.get();
            p.miss.set_opacity(1.0 - by);
            if let Some(play) = &p.play {
                play.set_opacity(by);
                if let Some(frame) = p.frame.as_ref().filter(|_| !p.narrow) {
                    let to = play.measure(gtk::Orientation::Horizontal, -1).1;
                    frame.set_size_request(lerp(width, to, by), -1);
                }
            }
            if let Some(clip) = &p.clip {
                clip.set_size_request(-1, lerp(height, 0, by));
                clip.set_opacity(1.0 - by);
            }
        }
    }

    /// The name of the head's button that has focus, before the head is built again.
    pub(super) fn lead_focus(&self) -> Option<&'static str> {
        let focus = gtk::prelude::GtkWindowExt::focus(&self.ctx.window)?;
        if !focus.is_ancestor(&self.head) {
            return None;
        }
        let mut at = Some(focus);
        while let Some(widget) = at {
            match widget.widget_name().as_str() {
                CHECK => return Some(CHECK),
                PREVIOUS => return Some(PREVIOUS),
                PLAY => return Some(PLAY),
                _ => {}
            }
            at = widget.parent().filter(|p| p != self.head.upcast_ref::<gtk::Widget>());
        }
        None
    }

    /// Focus back on the button that had it, once the head is built again. Focus on Check again
    /// or the episode before moves to Play once Play comes in.
    pub(super) fn restore_focus(&self, had: Option<&'static str>) {
        let Some(had) = had else { return };
        let to = if self.missing.found.get().is_some() || self.missing.shown.borrow().is_none()
        {
            PLAY
        } else {
            had
        };
        let head: &gtk::Widget = self.head.upcast_ref();
        if let Some(widget) = named(head, to).or_else(|| named(head, had)) {
            widget.grab_focus();
        }
    }

    /// Every few seconds: the time since airing on the minute, with no motion and nothing said,
    /// and new words once the quiet window or the week is over.
    pub(super) fn tick(&self) {
        // The rows that say how long ago their episode aired, or that it is not in the sources
        // yet, which they say for a week.
        let at = now();
        for line in self.lines.borrow().iter() {
            let row = line.shown.borrow().clone();
            let recent =
                row.airs_at.is_some_and(|aired| aired <= at && at - aired < 8 * 86_400);
            if row.unsourced && recent {
                self.dress(line, &row, false);
            }
        }
        let Some(missing) = self.missing_shown() else { return };
        if self.missing.found.get().is_some() || self.missing.status_from.borrow().is_some() {
            return;
        }
        self.track_missing();
        let Some(now_shown) = self.missing_shown() else { return };
        if now_shown.phase != missing.phase || now_shown.episode != missing.episode {
            self.redress();
            return;
        }
        let status = self.status_of(&now_shown);
        let parts = self.missing.parts.borrow();
        if let Some(p) = parts.as_ref()
            && p.label.text() != status.text()
        {
            let fresh = status_label(&status);
            p.label.set_label(&fresh.label());
            p.label.set_attributes(fresh.attributes().as_ref());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_width_and_height_settle_between_their_ends() {
        assert_eq!(lerp(300, 160, 0.0), 300);
        assert_eq!(lerp(300, 160, 0.5), 230);
        assert_eq!(lerp(300, 160, 1.0), 160);
        assert_eq!(lerp(28, 0, 0.25), 21);
    }
}
