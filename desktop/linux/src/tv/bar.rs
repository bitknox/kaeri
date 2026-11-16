//! A cast's bar along the bottom of the pages: the episode's art, the show, where the cast
//! stands, Play or Pause and Next, with the TV's playhead as a line along its top edge. It is
//! the bottom bar of the window's `AdwBottomSheet`: a click grows it into the cast's controls,
//! and closing them shrinks them back into it.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_engine::remote::protocol::command::RemoteCommand;
use kaeri_i18n::t;

use super::{
    Sheet, Tv,
    cast::{SPIN, Stage, playing_line},
};
use crate::window::Ctx;

/// The bar's progress moves on this often while it shows.
const TICK: Duration = Duration::from_millis(500);
/// A control that waits on the TV is dimmed to this, never hidden: the bar keeps its shape.
pub(super) const DIMMED: f64 = 0.38;
/// How long a status line's change crossfades: the fade-in and fade-out tokens, overlapped.
const CROSSFADE_MS: u32 = 250;

/// A cast's status line: a dot or a small spinner, then the words. A new stage crossfades both
/// in place; the words of the same stage (a count, a pause) change in place. Nothing beside it
/// moves, as the spinner keeps the dot's room.
pub(super) struct Line {
    pub root: gtk::Box,
    lead: gtk::Stack,
    dot: gtk::Box,
    words: gtk::Stack,
    /// The two labels the words crossfade between, and which of them shows.
    labels: [gtk::Label; 2],
    front: Cell<usize>,
    shown: RefCell<(String, &'static str)>,
}

impl Line {
    /// `wrap`: a long line wraps, as on the controls; else it ends in an ellipsis, as on the bar.
    pub fn new(wrap: bool, spinner: i32) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        root.set_valign(gtk::Align::Center);
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("mark");
        dot.set_halign(gtk::Align::Center);
        dot.set_valign(gtk::Align::Center);
        let spin = adw::Spinner::new();
        spin.set_size_request(spinner, spinner);
        spin.add_css_class("haze");
        let lead = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(CROSSFADE_MS)
            .valign(gtk::Align::Center)
            .build();
        lead.add_named(&dot, Some("dot"));
        lead.add_named(&spin, Some(SPIN));
        let label = || {
            let label = gtk::Label::builder().xalign(0.0).build();
            if wrap {
                label.set_wrap(true);
                label.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
            } else {
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.set_width_chars(1);
            }
            label
        };
        let labels = [label(), label()];
        let words = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(CROSSFADE_MS)
            .hhomogeneous(false)
            .hexpand(true)
            .build();
        words.add_named(&labels[0], Some("0"));
        words.add_named(&labels[1], Some("1"));
        root.append(&lead);
        root.append(&words);
        Self {
            root,
            lead,
            dot,
            words,
            labels,
            front: Cell::new(0),
            shown: RefCell::new((String::new(), "")),
        }
    }

    /// Say `text` with `mark`: a dot's colour, or [`SPIN`]. A change of mark crossfades when
    /// `animate`; the same mark's new words change in place. True when the mark changed.
    pub fn set(&self, text: &str, mark: &'static str, animate: bool) -> bool {
        let (was_text, was_mark) = self.shown.borrow().clone();
        if was_text == text && was_mark == mark {
            return false;
        }
        let moved = was_mark != mark;
        let instant = !animate || was_mark.is_empty();
        let haze = matches!(mark, "haze" | "ring" | SPIN);
        let front = self.front.get();
        let into = if moved && !instant { 1 - front } else { front };
        let label = &self.labels[into];
        label.set_label(text);
        label.remove_css_class(if haze { "standing" } else { "haze" });
        label.add_css_class(if haze { "haze" } else { "standing" });
        let transition = if instant {
            gtk::StackTransitionType::None
        } else {
            gtk::StackTransitionType::Crossfade
        };
        if into != front {
            self.words.set_visible_child_full(&into.to_string(), transition);
            self.front.set(into);
        }
        if mark != SPIN {
            for class in ["water", "vermilion", "haze", "ring"] {
                self.dot.remove_css_class(class);
            }
            self.dot.add_css_class(mark);
        }
        self.lead.set_visible_child_full(if mark == SPIN { SPIN } else { "dot" }, transition);
        self.shown.replace((text.to_owned(), mark));
        moved
    }
}

/// Dim `widget` to [`DIMMED`] and take its input while the TV can't be reached, or bring it back:
/// on the quick token when `animate`, else at once.
pub(super) fn dim(widget: &impl IsA<gtk::Widget>, dimmed: bool, animate: bool) {
    let widget = widget.as_ref();
    widget.set_sensitive(!dimmed);
    let to = if dimmed { DIMMED } else { 1.0 };
    if (widget.opacity() - to).abs() < f64::EPSILON {
        return;
    }
    if animate && widget.is_mapped() {
        crate::motion::quick(widget, widget.opacity(), to, crate::motion::opacity(widget))
            .play();
    } else {
        widget.set_opacity(to);
    }
}

/// Move `progress` to `to` on the standard token: the TV's playhead after a rejoin, ahead of
/// where it stopped as the TV played on.
fn glide(progress: &gtk::ProgressBar, to: f64) {
    let target = adw::PropertyAnimationTarget::new(progress, "fraction");
    crate::motion::standard(progress, progress.fraction(), to, target).play();
}

struct Bar {
    root: gtk::Overlay,
    art: gtk::Picture,
    title: gtk::Label,
    line: Line,
    play: gtk::Button,
    next: gtk::Button,
    progress: gtk::ProgressBar,
    /// The art's address drawn, so an update draws it once.
    art_url: RefCell<Option<String>>,
    /// It was reconnecting when last drawn: a rejoin is said once, and the playhead glides.
    reconnecting: Cell<bool>,
    /// The playhead glides to the TV's position until then, and the tick leaves it alone.
    gliding: Cell<Option<std::time::Instant>>,
}

impl Bar {
    fn new() -> Self {
        // The art sits over a 64 x 36 plate, so a large picture never sets the bar's size.
        let art = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .can_shrink(true)
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        let plate = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        plate.set_size_request(64, 36);
        let frame = gtk::Overlay::new();
        frame.set_child(Some(&plate));
        frame.add_overlay(&art);
        frame.set_overflow(gtk::Overflow::Hidden);
        frame.set_valign(gtk::Align::Center);
        frame.add_css_class("cast-bar-art");
        let title = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .width_chars(1)
            .build();
        title.add_css_class("heading");
        let line = Line::new(false, 12);
        line.root.add_css_class("caption");
        let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
        text.set_hexpand(true);
        text.set_valign(gtk::Align::Center);
        text.append(&title);
        text.append(&line.root);
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
        row.append(&frame);
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
            line,
            play,
            next,
            progress,
            art_url: RefCell::default(),
            reconnecting: Cell::new(false),
            gliding: Cell::new(None),
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
            Stage::Playing => playing_line(&view, || view.name.clone()),
            Stage::Failed => (t::tv_cast_couldnt_start(&view.name), "vermilion"),
            Stage::Lost => (t::tv_cast_stopped(), "vermilion"),
        };
        let mapped = self.root.is_mapped();
        self.line.set(&line, mark, mapped);
        // While it reconnects the bar has no hint, as a click does nothing.
        let reconnecting = view.reconnecting;
        let hint = (!reconnecting).then(t::cast_mini_opens);
        self.root.set_tooltip_text(hint.as_deref());
        if self.reconnecting.replace(reconnecting) != reconnecting {
            let said = if reconnecting {
                t::tv_status_reconnecting(&view.name)
            } else {
                t::tv_status_playing(&view.name)
            };
            if mapped && view.stage == Stage::Playing {
                self.root.announce(&said, gtk::AccessibleAnnouncementPriority::Medium);
            }
            // Back: the playhead moves on to where the TV got to meanwhile.
            if !reconnecting && mapped {
                self.gliding.set(Some(std::time::Instant::now() + Duration::from_millis(400)));
                glide(&self.progress, fraction(tv));
            }
        }
        let playing = view.stage == Stage::Playing && !reconnecting;
        // Dimmed, never hidden, while the TV starts or can't be heard: the bar keeps its shape.
        dim(&self.play, !playing && !view.stage.ended(), mapped);
        self.play.set_sensitive(playing);
        self.play.set_visible(!view.stage.ended());
        self.play.set_icon_name(if now.paused {
            "media-playback-start-symbolic"
        } else {
            "media-playback-pause-symbolic"
        });
        let tip = if now.paused { t::action_play() } else { t::action_pause() };
        self.play.set_tooltip_text(Some(&tip));
        let up_next = view.stage == Stage::Playing && view.up_next.is_some();
        self.next.set_visible(up_next);
        dim(&self.next, reconnecting, mapped);
        self.show_playhead(tv);
    }

    fn show_playhead(&self, tv: &Tv) {
        if self.gliding.get().is_some_and(|until| std::time::Instant::now() < until) {
            return;
        }
        self.gliding.set(None);
        self.progress.set_fraction(fraction(tv));
    }
}

/// How far the TV's playhead is through the episode, 0 before its length is known.
fn fraction(tv: &Tv) -> f64 {
    let length = tv.cast_view().and_then(|v| v.now).and_then(|n| n.duration);
    let at = tv.cast_playhead().unwrap_or(0.0);
    length.filter(|l| *l > 0.0).map_or(0.0, |l| (at / l).clamp(0.0, 1.0))
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
            if let Some(tv) = tv
                .upgrade()
                .filter(|tv| tv.casting() && tv.sheet().is_none() && !tv.cast_reconnecting())
            {
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
            // While it reconnects a click or a swipe does nothing; controls already open stay.
            let open = ctx.tv.sheet() == Some(Sheet::Remote);
            ctx.cast_sheet.set_can_open(casting && (open || !ctx.tv.cast_reconnecting()));
            // A notice shows above the bar, and at the window's foot over open controls.
            let overlay = ctx.notices.overlay();
            if casting && !open {
                overlay.add_css_class("cast-bar-up");
            } else {
                overlay.remove_css_class("cast-bar-up");
            }
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
