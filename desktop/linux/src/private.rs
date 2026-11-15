//! Private watching and reading, as the Linux app shows it: the switch first in Settings ›
//! Accounts, a check item in the main menu with Ctrl+Shift+N, a header bar button that opens a
//! popover, the notice after the menu item or the keys turn it on, the line under a title or
//! series page's main action, and the player's label. The engine holds what is watched and read
//! (`kaeri_engine::private`); this side shows the mode, and keeps the media controls to
//! "Playing in Kaeri" while it applies.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    private::{PrivateMode, PrivateTitle},
};

use crate::{motion::Fader, widgets, window::Ctx};

/// The keys that turn private mode on and off, as GTK reads them.
pub const ACCEL: &str = "<Control><Shift>n"; // i18n-skip
/// The same keys as Settings writes them under the switch.
pub const KEYS: &str = "Ctrl+Shift+N"; // i18n-skip
/// The eye-slash: the badge, the line and the player's label, so colour is never the only sign.
pub const EYE: &str = "view-conceal-symbolic";
/// How long the notice holds, as the board times it: it has a button.
const NOTICE_SECONDS: u32 = 8;

/// The window's own side of private mode: the notice on screen, and what a screen reader last
/// heard of the mode.
#[derive(Default)]
pub struct Private {
    notice: RefCell<Option<glib::WeakRef<adw::Toast>>>,
    said: Cell<bool>,
}

/// Turn private mode on or off. The engine says it back with `Event::Private`, which every
/// part of the window follows.
pub fn set(ctx: &Ctx, on: bool) {
    ctx.hub.send(Command::SetPrivate { on });
}

/// The main menu's item or the keys: the mode the other way, and once it is on, the notice, since
/// no description is in view there.
pub fn toggle(ctx: &Ctx) {
    let on = !ctx.hub.known.borrow().private;
    set(ctx, on);
    if on {
        let toast = ctx.toast_held(
            &t::private_mode_on_notice(),
            &t::action_turn_off(),
            "win.private-off",
            NOTICE_SECONDS,
        );
        ctx.private.notice.replace(Some(toast.downgrade()));
    }
}

/// What the window does as the mode changes: the menu item's check, the notice gone once it is
/// off, and one announcement each way.
pub fn heard(ctx: &Ctx, event: &Event) {
    let Event::Private(on) = event else { return };
    set_badges(*on);
    if let Some(action) =
        ctx.window.lookup_action("private").and_downcast::<gtk::gio::SimpleAction>()
    {
        action.set_state(&on.to_variant());
    }
    if !on && let Some(toast) = ctx.private.notice.take().and_then(|t| t.upgrade()) {
        toast.dismiss();
    }
    if ctx.private.said.replace(*on) != *on {
        let words = if *on {
            t::private_mode_on_announcement()
        } else {
            t::private_mode_off_announcement()
        };
        ctx.window
            .upcast_ref::<gtk::Widget>()
            .announce(&words, gtk::AccessibleAnnouncementPriority::Medium);
    }
}

/// Whether `event` can change what is private: the mode, a title's own flag, or a title's
/// standing, which says whether its place is held in memory.
pub fn changes(event: &Event) -> bool {
    matches!(
        event,
        Event::Private(_)
            | Event::AlwaysPrivate { .. }
            | Event::Progress { .. }
            | Event::ShowSettings(_)
            | Event::PlaybackStaged { .. }
            | Event::Reading(kaeri_model::reading::ReadingEvent::Series(_))
    )
}

/// Run `f` after each event that can change what is private, for as long as `widget` lives.
fn follow(
    ctx: &Rc<Ctx>,
    widget: &impl IsA<gtk::Widget>,
    f: impl Fn(&Rc<Ctx>, &Event) + 'static,
) {
    let weak = Rc::downgrade(ctx);
    crate::reading::listen(ctx, widget, move |event| {
        if changes(event)
            && let Some(ctx) = weak.upgrade()
        {
            f(&ctx, event);
        }
    });
}

/// The line's two parts: its head, "Private" or "Always private", and what follows it.
pub fn line_words(mode: PrivateMode, kept: bool) -> (String, String) {
    match mode {
        PrivateMode::Always => (t::title_always_private(), t::private_line_not_saved()),
        PrivateMode::Session if kept => (t::private_badge(), t::private_line_kept()),
        PrivateMode::Session => (t::private_badge(), t::private_line_not_saved()),
    }
}

/// The line under a title or series page's main action: "Private · Not saved or synced". It
/// grows on the standard token while its words fade in, and goes the same way. A page keeps one
/// and moves it into each new layout of its head.
pub struct Line {
    revealer: gtk::Revealer,
    words: gtk::Box,
    head: gtk::Label,
    tail: gtk::Label,
    shown: Cell<Option<(PrivateMode, bool)>>,
    /// A change waiting for the page to settle.
    due: RefCell<Option<glib::SourceId>>,
}

impl Line {
    pub fn new() -> Rc<Self> {
        let eye = gtk::Image::from_icon_name(EYE);
        eye.add_css_class("private-eye");
        let head = gtk::Label::new(None);
        head.add_css_class("private-line-head");
        let tail = gtk::Label::new(None);
        tail.add_css_class("haze");
        tail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        tail.set_xalign(0.0);
        let words = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        words.add_css_class("private-line");
        words.set_margin_top(10);
        words.append(&eye);
        let text = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        text.append(&head);
        text.append(&tail);
        words.append(&text);
        let revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideDown)
            .transition_duration(300)
            .child(&words)
            .visible(false)
            .build();
        // Out of the layout once it has gone, so its parent's spacing closes up too.
        revealer.connect_child_revealed_notify(|revealer| {
            if !revealer.reveals_child() && !revealer.is_child_revealed() {
                revealer.set_visible(false);
            }
        });
        Rc::new(Self {
            revealer,
            words,
            head,
            tail,
            shown: Cell::new(None),
            due: RefCell::default(),
        })
    }

    pub fn widget(&self) -> &gtk::Revealer {
        &self.revealer
    }

    /// Put the line at the end of `parent`, out of the layout it was in before.
    pub fn place(&self, parent: &gtk::Box) {
        if let Some(old) = self.revealer.parent().and_downcast::<gtk::Box>() {
            old.remove(&self.revealer);
        }
        parent.append(&self.revealer);
    }

    /// Show what `now` says, or no line. `animate` grows it and fades its words in; otherwise it
    /// is simply there, as on a page just opened.
    pub fn set(&self, now: Option<(PrivateMode, bool)>, animate: bool) {
        if let Some((mode, kept)) = now {
            let (head, tail) = line_words(mode, kept);
            self.head.set_label(&head);
            self.tail.set_label(&format!(" \u{b7} {tail}")); // i18n-skip
            self.words.update_property(&[gtk::accessible::Property::Label(&format!(
                "{head} \u{b7} {tail}" // i18n-skip
            ))]);
        }
        let was = self.shown.replace(now);
        if was.is_some() == now.is_some() {
            return;
        }
        let animate = animate && self.revealer.is_mapped() && !crate::motion::reduced();
        self.revealer.set_transition_duration(if animate { 300 } else { 0 });
        if now.is_some() {
            self.revealer.set_visible(true);
        }
        self.revealer.set_reveal_child(now.is_some());
        if now.is_none() && !animate {
            self.revealer.set_visible(false);
        }
        if animate && now.is_some() {
            // The words fade in from 0.1 s, while the slot grows.
            self.words.set_opacity(0.0);
            let words = self.words.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
                crate::motion::fade_in(&words);
            });
        } else {
            self.words.set_opacity(1.0);
        }
    }

    /// Show what `now` says once the page has settled, animated: the engine says the title's
    /// standing right after the mode, and the new layout of the head that brings would cut the
    /// line's growth short.
    pub fn later(self: &Rc<Self>, now: impl Fn() -> Option<(PrivateMode, bool)> + 'static) {
        if let Some(due) = self.due.take() {
            due.remove();
        }
        let line = Rc::downgrade(self);
        let due =
            glib::timeout_add_local_once(std::time::Duration::from_millis(80), move || {
                if let Some(line) = line.upgrade() {
                    line.due.take();
                    line.set(now(), true);
                }
            });
        self.due.replace(Some(due));
    }

    /// Whether the line shows now, for a test.
    #[cfg(test)]
    pub fn shown(&self) -> Option<(PrivateMode, bool)> {
        self.shown.get()
    }
}

thread_local! {
    /// Whether private mode is on, as the engine last said, for the header bars' badges.
    static ON: Cell<bool> = const { Cell::new(false) };
    /// Every header bar's badge, each with its own fade.
    static BADGES: RefCell<Vec<(glib::WeakRef<gtk::MenuButton>, Rc<Fader>)>> =
        const { RefCell::new(Vec::new()) };
}

/// Whether private mode is on, as the engine last said.
pub fn is_on() -> bool {
    ON.get()
}

/// Fade every badge in or out with the mode.
fn set_badges(on: bool) {
    ON.set(on);
    BADGES.with_borrow_mut(|badges| {
        badges.retain(|(b, _)| b.upgrade().is_some());
        for (button, fader) in badges.iter() {
            let Some(button) = button.upgrade() else { continue };
            if !on {
                button.popdown();
            }
            fader.set(&button, on);
        }
    });
}

/// Put the badge at the end of `header`, before its own buttons: packed last, it sits left of
/// them, as the board draws it.
pub fn badge_on(header: &adw::HeaderBar) {
    header.pack_end(&badge());
}

/// The header bar's button while private mode is on: the eye-slash and "Private". It opens a
/// popover with the notice's words and Turn off, and fades in and out with the mode.
pub fn badge() -> gtk::MenuButton {
    let inside = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    inside.append(&gtk::Image::from_icon_name(EYE));
    inside.append(&gtk::Label::new(Some(&t::private_badge())));
    let button = gtk::MenuButton::builder()
        .child(&inside)
        .popover(&popover())
        .valign(gtk::Align::Center)
        .build();
    button.add_css_class("private-badge");
    button.update_property(&[
        gtk::accessible::Property::Label(&t::private_badge_label()),
        gtk::accessible::Property::Description(&t::private_mode_on_notice()),
    ]);
    let fader = Rc::new(Fader::default());
    // Shown as it is, with no fade: a page opened while private has it from the start.
    if is_on() {
        // From full opacity to full: there at once.
        fader.set(&button, true);
    } else {
        fader.hide_now(&button);
    }
    BADGES.with_borrow_mut(|badges| badges.push((button.downgrade(), fader)));
    button
}

/// The badge's popover: the mode named, the notice's words, and Turn off.
fn popover() -> gtk::Popover {
    let eye = gtk::Image::from_icon_name(EYE);
    eye.set_pixel_size(18);
    let name = gtk::Label::new(Some(&t::private_mode()));
    name.add_css_class("heading");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    head.append(&eye);
    head.append(&name);
    let words = widgets::haze(&t::private_mode_on_notice(), "");
    words.set_max_width_chars(34);
    words.set_margin_top(6);
    let off = gtk::Button::with_label(&t::action_turn_off());
    off.set_halign(gtk::Align::End);
    off.set_margin_top(14);
    off.set_action_name(Some("win.private-off"));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_margin_top(10);
    column.set_margin_bottom(10);
    column.set_margin_start(10);
    column.set_margin_end(10);
    column.append(&head);
    column.append(&words);
    column.append(&off);
    let popover = gtk::Popover::new();
    popover.add_css_class("private-popover");
    popover.set_child(Some(&column));
    popover
}

/// Settings › Accounts, first: the switch, its description as the row's subtitle, and under it
/// what it does not stop and the keys.
pub fn settings_group(ctx: &Rc<Ctx>) -> adw::PreferencesGroup {
    let row = adw::SwitchRow::builder()
        .title(t::private_mode())
        .subtitle(t::private_mode_detail())
        .active(ctx.hub.known.borrow().private)
        .build();
    row.connect_active_notify({
        let ctx = Rc::downgrade(ctx);
        move |row| {
            let Some(ctx) = ctx.upgrade() else { return };
            // The engine's word sets the row too; only a change of the viewer's goes out.
            if row.is_active() != ctx.hub.known.borrow().private {
                set(&ctx, row.is_active());
            }
        }
    });
    let shown = row.downgrade();
    follow(ctx, &row, move |_, event| {
        if let (Event::Private(on), Some(row)) = (event, shown.upgrade())
            && row.is_active() != *on
        {
            row.set_active(*on);
        }
    });
    let group = adw::PreferencesGroup::new();
    group.add(&row);
    // Not a row: a group puts it under its list.
    let under = widgets::haze(&limits_line(), "caption");
    under.set_margin_top(8);
    under.set_margin_start(2);
    group.add(&under);
    group
}

/// The words under the switch: what private mode does not hide, and the keys.
pub fn limits_line() -> String {
    format!("{} {}", t::private_mode_limits(), t::private_mode_shortcut_hint(KEYS))
}

/// The player's label after the episode line: the eye-slash and "Private", while the mode is on or
/// the show is always private.
pub fn player_label() -> gtk::Box {
    let label = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    label.add_css_class("private-player");
    label.set_valign(gtk::Align::Center);
    label.append(&gtk::Image::from_icon_name(EYE));
    let word = gtk::Label::new(Some(&t::private_badge()));
    word.add_css_class("caption");
    label.append(&word);
    label.set_visible(false);
    label
}

/// The reader's "· Private" after its line, which hides with the chrome.
pub fn reader_label() -> gtk::Box {
    let label = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    label.add_css_class("private-reader");
    let dot = gtk::Label::new(Some(" \u{b7}")); // i18n-skip
    label.append(&dot);
    let eye = gtk::Image::from_icon_name(EYE);
    eye.set_pixel_size(14);
    label.append(&eye);
    label.append(&gtk::Label::new(Some(&t::private_badge())));
    label.set_visible(false);
    label
}

/// A title's own flag as a stateful action, for its More menu's check item: its state follows
/// the engine, and choosing it asks for the other way.
pub fn always_action(
    ctx: &Rc<Ctx>,
    title: PrivateTitle,
    anchor: &impl IsA<gtk::Widget>,
) -> gtk::gio::SimpleAction {
    let on = ctx.hub.known.borrow().always(&title);
    let action = gtk::gio::SimpleAction::new_stateful("always-private", None, &on.to_variant());
    action.connect_activate({
        let (ctx, title) = (Rc::downgrade(ctx), title.clone());
        move |action, _| {
            let Some(ctx) = ctx.upgrade() else { return };
            let on = !action.state().and_then(|s| s.get::<bool>()).unwrap_or(false);
            action.set_state(&on.to_variant());
            ctx.hub.send(Command::SetAlwaysPrivate { title: title.clone(), on });
            if on {
                ctx.window.upcast_ref::<gtk::Widget>().announce(
                    &t::title_always_private(),
                    gtk::AccessibleAnnouncementPriority::Medium,
                );
            }
        }
    });
    let weak = action.downgrade();
    follow(ctx, anchor, move |ctx, _| {
        if let Some(action) = weak.upgrade() {
            let on = ctx.hub.known.borrow().always(&title);
            if action.state().and_then(|s| s.get::<bool>()) != Some(on) {
                action.set_state(&on.to_variant());
            }
        }
    });
    action
}

/// The switch for a title's own flag, in Settings for this show or the series' sheet, with what
/// it means under it.
pub fn always_row(ctx: &Rc<Ctx>, title: PrivateTitle, detail: &str) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(t::title_always_private())
        .subtitle(detail)
        .active(ctx.hub.known.borrow().always(&title))
        .build();
    row.connect_active_notify({
        let (ctx, title) = (Rc::downgrade(ctx), title.clone());
        move |row| {
            let Some(ctx) = ctx.upgrade() else { return };
            if row.is_active() != ctx.hub.known.borrow().always(&title) {
                ctx.hub.send(Command::SetAlwaysPrivate {
                    title: title.clone(),
                    on: row.is_active(),
                });
            }
        }
    });
    let shown = row.downgrade();
    follow(ctx, &row, move |ctx, _| {
        let on = ctx.hub.known.borrow().always(&title);
        if let Some(row) = shown.upgrade()
            && row.is_active() != on
        {
            row.set_active(on);
        }
    });
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_says_the_titles_own_flag_before_the_place_kept() {
        assert_eq!(
            line_words(PrivateMode::Session, false),
            ("Private".to_owned(), "Not saved or synced".to_owned())
        );
        assert_eq!(
            line_words(PrivateMode::Session, true),
            ("Private".to_owned(), "Your place is kept until Kaeri quits".to_owned())
        );
        assert_eq!(
            line_words(PrivateMode::Always, true),
            ("Always private".to_owned(), "Not saved or synced".to_owned())
        );
    }

    #[test]
    fn under_the_switch_it_says_what_it_does_not_hide_and_the_keys() {
        assert_eq!(
            limits_line(),
            "Your sources and watch parties still see what you open. Shortcut: Ctrl+Shift+N"
        );
    }

    #[test]
    fn what_can_change_the_mode_or_a_title_is_followed_and_nothing_else() {
        assert!(changes(&Event::Private(true)));
        assert!(changes(&Event::AlwaysPrivate {
            title: PrivateTitle::Show { id: kaeri_core::ids::AnilistId::new(3) },
            on: true,
        }));
        assert!(!changes(&Event::Connectivity { offline: true }));
    }
}
