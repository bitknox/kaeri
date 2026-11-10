//! Credit pages and chapter parts in the reader, as the credit pages board draws them on Linux.
//!
//! The engine leaves a page out when a group repeats it at the start or end of its chapters, and
//! names the pages it left out after each layout. On page 1 a pill, top trailing, says how many;
//! the whole pill shows them. A shown page stands before page 1 (or after the last page) with a
//! toast that offers Not a credit page, and stays shown until the chapter closes. A strip has a
//! 44 px row where the image would be, which grows into the image. Pages skipped at the end are
//! named on the end page, or in a strip's marker into the next chapter.
//!
//! A chapter read in parts has one count, with a tick on the scrubber where each later part
//! starts (and where a missing one would be), and a preview naming the part on a tick.

use std::{rc::Rc, time::Duration};

use adw::prelude::*;
use gtk::glib;
use kaeri_i18n::t;
use kaeri_model::{
    Msg, ReadingMsg,
    reader::{ChapterPart, CreditPage, ReaderCommand, ReaderMode, StripPos, TurnHow},
};

use super::{Reader, State, load_texture, strip_target, trail::tall};
use crate::{motion, say};

/// The pill's distance from the top with no chrome over the page.
pub const LINE_TOP: i32 = 14;
/// The pill holds this long on page 1.
const LINE_HOLD: Duration = Duration::from_secs(4);
/// The notice after Not a credit page holds this long, in seconds.
const KEPT_HOLD: u32 = 8;
/// A strip's credit row.
const ROW: i32 = 44;
/// The part preview stays this long after the scrubber last moved.
const TIP_HOLD: Duration = Duration::from_millis(1500);

/// Not a credit page, held while its Undo shows.
pub struct Keeping {
    session: u64,
    fingerprint: u64,
    at_end: bool,
    toast: adw::Toast,
}

/// A kept page the engine was told of: the next layout has it as page `page`, every page from
/// there one on.
pub struct Relay {
    pub fingerprint: u64,
    pub page: u32,
}

/// The open chapter's parts, as the engine last sent them.
pub struct Parts {
    pub parts: Vec<ChapterPart>,
    pub missing: Vec<u32>,
}

/// "2 credit pages skipped" with Show beside it: the end page's line and the marker's. The
/// box, its words and Show.
pub fn show_line() -> (gtk::Box, gtk::Label, gtk::Button) {
    let text = gtk::Label::new(None);
    text.add_css_class("haze");
    text.set_xalign(0.0);
    text.set_wrap(true);
    let show = gtk::Button::with_label(&t::reader_credit_show());
    show.add_css_class("flat");
    show.add_css_class("reader-credit-show");
    show.set_valign(gtk::Align::Center);
    show.update_property(&[gtk::accessible::Property::Description(
        &t::reader_credit_show_hint(),
    )]);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&text);
    row.append(&show);
    (row, text, show)
}

/// The line for the pages skipped at the chapter's end, while they are not shown.
pub fn end_line(state: &State) -> Option<String> {
    let count = state.credits.iter().filter(|c| c.at_end).count();
    (count > 0 && !state.shown_end).then(|| skipped(count))
}

fn skipped(count: usize) -> String {
    say::msg(&Msg::Reading(ReadingMsg::ReaderCreditPagesSkipped { count: count as i64 }))
}

/// A strip's credit pages at its start or its end: at the start, the 44 px row that turns into
/// the first page's caption as the pages grow in under it; at the end, the pages alone, each
/// under its caption, once they are shown.
pub struct CreditBlock {
    pub widget: gtk::Box,
    /// The row: its line, then the first page's caption, in a veil the line fades over. At
    /// the start only.
    row: Option<Row>,
    revealer: gtk::Revealer,
    /// Each page's picture and its width over height.
    frames: Vec<(gtk::Picture, f32)>,
    /// The rows a column wide.
    rows: Vec<gtk::Widget>,
}

impl CreditBlock {
    /// Each row and page at the column's width. True when a size changed.
    pub fn size(&self, column: i32) -> bool {
        let mut changed = false;
        for row in &self.rows {
            if row.size_request() != (column, ROW) {
                row.set_size_request(column, ROW);
                changed = true;
            }
        }
        for (picture, ratio) in &self.frames {
            let size =
                (column, (f64::from(column) / f64::from(ratio.max(0.05))).round() as i32);
            if picture.size_request() != size {
                picture.set_size_request(size.0, size.1);
                changed = true;
            }
        }
        changed
    }

    /// How tall it is now, growing included.
    pub fn height(&self) -> f64 {
        if self.widget.parent().is_none() || !self.widget.is_visible() {
            return 0.0;
        }
        f64::from(tall(&self.widget))
    }

    /// The pages there at once, the row already their caption: shown before a new layout.
    fn reveal_now(&self) {
        if let Some(row) = &self.row {
            row.stack.set_visible_child_name("caption");
        }
        self.revealer.set_transition_type(gtk::RevealerTransitionType::None);
        self.revealer.set_reveal_child(true);
        self.revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
    }

    /// What `grow` needs, held apart from the reader's state: a revealer that does not
    /// animate, under Reduce Motion, says it is revealed at once, and the strip then reads the
    /// state.
    fn grower(&self) -> (Option<Row>, gtk::Revealer) {
        (self.row.clone(), self.revealer.clone())
    }

    fn shown(&self) -> bool {
        self.revealer.reveals_child()
    }
}

/// A strip's credit row: the line and the caption in a stack, and the veil the line fades
/// over the caption in.
#[derive(Clone)]
struct Row {
    veil: gtk::Overlay,
    stack: gtk::Stack,
}

/// The row's line fades into the pages' caption and the pages grow in under it, top first and
/// in place, fading in over 200 ms. Under Reduce Motion they are there at once at full height,
/// still fading in. The keyboard goes from the row to the caption's Not a credit page.
fn grow(row: Option<&Row>, revealer: &gtk::Revealer) {
    if let Some(row) = row {
        let focused = row.stack.child_by_name("line").is_some_and(|line| line.has_focus());
        motion::ghost(&row.veil, &row.stack, true, || {
            row.stack.set_visible_child_name("caption");
        });
        if focused
            && let Some(not) = row
                .stack
                .child_by_name("caption")
                .and_then(|caption| with_class(&caption, "reader-credit-show"))
        {
            not.grab_focus();
        }
    }
    if let Some(pictures) = revealer.child() {
        pictures.set_opacity(0.0);
        let fade = motion::timed(&pictures, 0.0, 1.0, 200, motion::opacity(&pictures));
        fade.set_easing(adw::Easing::EaseOutCubic);
        fade.set_follow_enable_animations_setting(false);
        fade.play();
    }
    revealer.set_reveal_child(true);
}

fn with_class(widget: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
    if widget.has_css_class(class) {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = with_class(&next, class) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

impl Reader {
    // ---------------------------------------------------------------------------------------
    // From the engine

    /// A new layout came: the credit pages and parts after it are the engine's word on it. Gives
    /// back the credit page on screen, when one is, by its print and whether it is at the end.
    pub(super) fn relaid(&self) -> Option<(u64, bool)> {
        let on = {
            let mut state = self.state.borrow_mut();
            let on = state
                .credit_on
                .take()
                .and_then(|i| state.credits.get(i))
                .map(|c| (c.fingerprint, c.at_end));
            state.credits.clear();
            state.parts = None;
            state.credit_wanted = None;
            on
        };
        self.scrub.clear_marks();
        self.credit_fade.hide_now(&self.credit_line);
        if on.is_some() {
            // The page stays until the layout's credit pages say whether it is still one.
            let me = self.me.clone();
            glib::timeout_add_local_once(Duration::from_millis(200), move || {
                let Some(this) = me.upgrade() else { return };
                if this.state.borrow_mut().credit_wanted.take().is_some() {
                    this.page_line_back();
                    let index = this.state.borrow().index;
                    this.show_leaf(index, TurnHow::Placed);
                }
            });
        }
        on
    }

    pub(super) fn credit_pages(self: &Rc<Self>, mut pages: Vec<CreditPage>) {
        pages.sort_by_key(|p| (p.at_end, p.page));
        let wanted = {
            let mut state = self.state.borrow_mut();
            state.credits = pages;
            state.credit_wanted.take()
        };
        self.fill_end();
        if self.flows() {
            self.build_strip_credits();
            return;
        }
        if let Some(print) = wanted {
            let at = self.state.borrow().credits.iter().position(|c| c.fingerprint == print);
            match at {
                Some(at) if self.show_credit(at) => {}
                _ => {
                    self.page_line_back();
                    let index = self.state.borrow().index;
                    self.show_leaf(index, TurnHow::Placed);
                }
            }
            return;
        }
        let first_time = {
            let state = self.state.borrow();
            state.index == 0
                && state.credit_on.is_none()
                && !state.credit_line_done
                && !state.shown_start
        };
        if first_time && self.on_page() && self.fill_credit_line() {
            self.credit_fade.set(&self.credit_line, true);
            self.credit_line.announce(
                &self.credit_line.label().unwrap_or_default(),
                gtk::AccessibleAnnouncementPriority::Medium,
            );
            if let Some(id) = self.credit_timer.borrow_mut().take() {
                id.remove();
            }
            let me = self.me.clone();
            let id = glib::timeout_add_local_once(LINE_HOLD, move || {
                let Some(this) = me.upgrade() else { return };
                this.credit_timer.borrow_mut().take();
                this.state.borrow_mut().credit_line_done = true;
                if !this.head_fade.shown() {
                    this.credit_fade.set(&this.credit_line, false);
                }
            });
            *self.credit_timer.borrow_mut() = Some(id);
        }
    }

    pub(super) fn chapter_parts(&self, parts: Vec<ChapterPart>, missing: Vec<u32>) {
        self.scrub.clear_marks();
        let total = self.story_pages().max(1) as f64;
        let first = parts.first().map(|p| p.number);
        for part in parts.iter().filter(|p| Some(p.number) != first) {
            self.scrub.add_mark(f64::from(part.first_page), gtk::PositionType::Bottom, None);
        }
        for m in &missing {
            self.scrub.add_mark(missing_at(&parts, *m, total), gtk::PositionType::Bottom, None);
        }
        self.state.borrow_mut().parts = Some(Parts { parts, missing });
    }

    /// The chapter closed or another opened: its credit pages and parts go with it.
    pub(super) fn forget_credits(&self) {
        self.commit_keep();
        self.drop_strip_credits();
        {
            let mut state = self.state.borrow_mut();
            state.relay_kept = None;
            state.credits.clear();
            state.credit_on = None;
            state.shown_start = false;
            state.shown_end = false;
            state.credit_line_done = false;
            state.credit_wanted = None;
            state.credit_textures.clear();
            state.parts = None;
        }
        self.scrub.clear_marks();
        self.part_tip_fade.hide_now(&self.part_tip);
        self.quiet_credits();
    }

    /// The pill and the notices go at once: the reader is closing.
    pub(super) fn quiet_credits(&self) {
        if let Some(id) = self.credit_timer.borrow_mut().take() {
            id.remove();
        }
        self.credit_fade.hide_now(&self.credit_line);
        self.credit_toast.take();
        self.notices.dismiss_each();
    }

    // ---------------------------------------------------------------------------------------
    // Paged

    /// The page is up in paged reading, not the end page, a plate or a failed page.
    fn on_page(&self) -> bool {
        !self.flows() && self.stack.visible_child_name().as_deref() == Some("paged")
    }

    /// The pill's words, from the pages skipped at the start. False when there are none.
    fn fill_credit_line(&self) -> bool {
        let count = self.state.borrow().credits.iter().filter(|c| !c.at_end).count();
        if count == 0 {
            return false;
        }
        self.credit_line.set_label(&skipped(count));
        true
    }

    /// The pill or a line was clicked: the pages at the start, or the end, show from now on
    /// until the chapter closes. Paged reading turns to the first of them; a strip grows them
    /// in place.
    pub(super) fn show_credits(self: &Rc<Self>, at_end: bool) {
        {
            let mut state = self.state.borrow_mut();
            if at_end {
                state.shown_end = true;
            } else {
                state.shown_start = true;
            }
            state.credit_line_done = true;
        }
        if let Some(id) = self.credit_timer.borrow_mut().take() {
            id.remove();
        }
        self.credit_fade.set(&self.credit_line, false);
        if self.flows() {
            self.show_strip_credits(at_end);
            return;
        }
        let first = self.state.borrow().credits.iter().position(|c| c.at_end == at_end);
        if let Some(first) = first
            && !self.show_credit(first)
        {
            self.credit_failed(at_end);
        }
        self.fill_end();
    }

    /// The pages could not be shown: they stay skipped, the pill or the line comes back, and a
    /// notice says so.
    fn credit_failed(&self, at_end: bool) {
        {
            let mut state = self.state.borrow_mut();
            if at_end {
                state.shown_end = false;
            } else {
                state.shown_start = false;
            }
        }
        if !at_end && self.on_page() && self.fill_credit_line() {
            self.credit_fade.set(&self.credit_line, true);
        }
        self.fill_end();
        let toast = adw::Toast::builder()
            .title(glib::markup_escape_text(&t::reader_credit_failed()))
            .timeout(crate::say::notice_seconds(
                &t::reader_credit_failed(),
                false,
                false,
                crate::window::screen_reader_running(),
            ))
            .build();
        self.notices.add(&toast);
    }

    /// Credit page `i` on screen in place of a page, by the same crossfade as a turn. False
    /// when its file can't be read.
    fn show_credit(self: &Rc<Self>, i: usize) -> bool {
        let Some(credit) = self.state.borrow().credits.get(i).cloned() else { return false };
        let Some(texture) = self.credit_texture(&credit) else { return false };
        self.state.borrow_mut().credit_on = Some(i);
        self.stop_wait();
        motion::turn(&self.veil, &self.stack, || {
            self.picture.set_paintable(Some(&texture));
            self.apply_fit();
            self.stack.set_visible_child_name("paged");
        });
        self.scroll_to_start();
        self.credit_fade.set(&self.credit_line, false);
        if let Some((_, capsule)) = &self.state.borrow().demo {
            capsule.set_visible(false);
        }
        self.credit_place(i);
        if !self.held(credit.fingerprint) {
            self.credit_notice();
        }
        true
    }

    /// "Chapter 87 · credit page", or "credit page 1 of 2", in the group; the totals stay and
    /// the thumb sits at the start, or at the end for the pages after the last.
    fn credit_place(&self, i: usize) {
        let held = self.state.borrow().credits.get(i).is_some_and(|c| self.held(c.fingerprint));
        if held {
            // Kept, Undo still offered: one of the chapter's pages already.
            let label = self.state.borrow().chapter.as_ref().map(|c| c.label.clone());
            self.line.set_label(&label.unwrap_or_default());
            self.picture.reset_property(gtk::AccessibleProperty::Label);
            let total = self.story_pages();
            self.show_pages(1, total);
            let said = t::linux_reader_page_of(
                self.scrub.value().round() as i64,
                self.scrub.adjustment().upper() as i64,
            );
            self.scrub.update_property(&[gtk::accessible::Property::ValueText(&said)]);
            return;
        }
        self.picture.update_property(&[gtk::accessible::Property::Label(
            &t::reader_credit_page_label(),
        )]);
        let (place, at_end, chapter) = {
            let state = self.state.borrow();
            let Some(credit) = state.credits.get(i) else { return };
            let side: Vec<usize> = (0..state.credits.len())
                .filter(|j| state.credits[*j].at_end == credit.at_end)
                .collect();
            let n = side.len();
            let at = side.iter().position(|j| *j == i).unwrap_or(0) + 1;
            let place = if n > 1 {
                t::reader_credit_page_of(at as i64, n as i64)
            } else {
                t::reader_credit_page()
            };
            let chapter = state.chapter.as_ref().map(|c| c.label.clone()).unwrap_or_default();
            (place, credit.at_end, chapter)
        };
        self.line.set_label(&t::reader_group_line_place(&chapter, &place));
        let total = self.story_pages();
        self.show_pages(if at_end { total } else { 1 }, total);
        self.scrub_at.set_label("");
        self.scrub.update_property(&[gtk::accessible::Property::ValueText(&place)]);
    }

    /// A turn or a jump away from a credit page: the page's own line comes back and the notice
    /// leaves.
    pub(super) fn leave_credit(&self) {
        if self.state.borrow_mut().credit_on.take().is_none() {
            return;
        }
        self.page_line_back();
    }

    /// The chapter's own line, label and no notice, for a page that is not a credit page.
    fn page_line_back(&self) {
        let label = self.state.borrow().chapter.as_ref().map(|c| c.label.clone());
        self.line.set_label(&label.unwrap_or_default());
        self.picture.reset_property(gtk::AccessibleProperty::Label);
        if let Some(toast) = self.credit_toast.take() {
            toast.dismiss();
        }
    }

    /// A leaf was painted: a turn away from page 1 takes the pill with it.
    pub(super) fn credit_line_on(&self, index: usize) {
        if index != 0 && self.credit_fade.shown() {
            if let Some(id) = self.credit_timer.borrow_mut().take() {
                id.remove();
            }
            self.state.borrow_mut().credit_line_done = true;
            self.credit_fade.set(&self.credit_line, false);
        }
    }

    /// The chrome came or went: on page 1 the pill comes back with it, under the header, and
    /// a toast moves above the group.
    pub(super) fn credit_line_chrome(&self, chrome: bool) {
        if chrome {
            self.toasts.add_css_class("over-group");
        } else {
            self.toasts.remove_css_class("over-group");
        }
        let (_, head, _, _) = self.head.measure(gtk::Orientation::Vertical, -1);
        self.credit_line.set_margin_top(if chrome { head + 8 } else { LINE_TOP });
        let back = {
            let state = self.state.borrow();
            state.index == 0 && state.credit_on.is_none() && !state.shown_start
        };
        if chrome {
            if back && self.on_page() && self.fill_credit_line() {
                self.credit_fade.set(&self.credit_line, true);
            }
        } else if self.state.borrow().credit_line_done {
            self.credit_fade.set(&self.credit_line, false);
        }
    }

    /// A notice with its button beside the words, which wrap on a narrow window. The toast's
    /// own button keeps its width and its words stay on one line, so the notice would ask for
    /// more than a phone-width window has.
    fn toast_line(
        &self,
        text: &str,
        label: &str,
        timeout: u32,
        clicked: impl Fn(&Rc<Reader>) + 'static,
    ) -> adw::Toast {
        let toast = adw::Toast::builder()
            .title(glib::markup_escape_text(text))
            .timeout(timeout)
            .priority(adw::ToastPriority::High)
            .build();
        let words = gtk::Label::new(Some(text));
        words.set_wrap(true);
        words.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        words.set_natural_wrap_mode(gtk::NaturalWrapMode::None);
        words.set_xalign(0.0);
        words.set_hexpand(true);
        let button = gtk::Button::with_label(label);
        button.set_valign(gtk::Align::Center);
        button.connect_clicked({
            let me = self.me.clone();
            move |_| {
                if let Some(this) = me.upgrade() {
                    clicked(&this);
                }
            }
        });
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.append(&words);
        line.append(&button);
        toast.set_custom_title(Some(&line));
        self.notices.add(&toast);
        // The custom title may not reach a screen reader as the toast's text, so say it too.
        self.notices
            .overlay()
            .upcast_ref::<gtk::Widget>()
            .announce(text, gtk::AccessibleAnnouncementPriority::High);
        toast
    }

    /// The notice over a shown credit page: no timeout while the page shows, Not a credit page
    /// its action.
    fn credit_notice(&self) {
        if self.credit_toast.borrow().is_some() {
            return;
        }
        let toast =
            self.toast_line(&t::reader_credit_shown(), &t::reader_credit_not(), 0, |this| {
                let print = {
                    let state = this.state.borrow();
                    state.credit_on.and_then(|i| state.credits.get(i)).map(|c| c.fingerprint)
                };
                if let Some(print) = print {
                    this.not_a_credit(print);
                }
            });
        toast.connect_dismissed({
            let me = self.me.clone();
            move |toast| {
                let Some(this) = me.upgrade() else { return };
                if this.credit_toast.borrow().as_ref() == Some(toast) {
                    this.credit_toast.take();
                }
            }
        });
        self.credit_toast.replace(Some(toast));
    }

    /// Not a credit page: the page counts as one of the chapter's at once and the notice offers
    /// Undo for 8 s (16 s while a screen reader runs). The engine hears of it when the notice
    /// times out, at a second keep, or as the chapter closes or another opens; it then keeps
    /// the page in the series and lays the chapter out again with it.
    fn not_a_credit(&self, fingerprint: u64) {
        self.commit_keep();
        if let Some(toast) = self.credit_toast.take() {
            toast.dismiss();
        }
        let (session, at_end) = {
            let state = self.state.borrow();
            let at_end = state.credits.iter().find(|c| c.fingerprint == fingerprint);
            let Some(at_end) = at_end.map(|c| c.at_end) else { return };
            (state.session, at_end)
        };
        let hold =
            if crate::window::screen_reader_running() { KEPT_HOLD * 2 } else { KEPT_HOLD };
        let kept = self.toast_line(&t::reader_credit_kept(), &t::action_undo(), hold, |this| {
            this.undo_keep();
        });
        kept.connect_dismissed({
            let me = self.me.clone();
            move |toast| {
                let Some(this) = me.upgrade() else { return };
                let mine = this.keeping.borrow().as_ref().is_some_and(|k| &k.toast == toast);
                if mine {
                    this.commit_keep();
                }
            }
        });
        self.keeping.replace(Some(Keeping {
            session,
            fingerprint,
            at_end,
            toast: kept.clone(),
        }));
        self.place_again();
    }

    /// The held keep goes to the engine; the next layout numbers the pages afresh.
    pub(super) fn commit_keep(&self) {
        let Some(keep) = self.keeping.take() else { return };
        keep.toast.dismiss();
        let page = kept_number(&self.state.borrow().credits, keep.fingerprint);
        if let Some(page) = page {
            self.state.borrow_mut().relay_kept =
                Some(Relay { fingerprint: keep.fingerprint, page });
        }
        self.send(ReaderCommand::NotACreditPage {
            session: keep.session,
            fingerprint: keep.fingerprint,
        });
    }

    /// Undo, or Ctrl+Z: the page is a credit page again, nothing sent.
    pub(super) fn undo_keep(&self) {
        let Some(keep) = self.keeping.take() else { return };
        keep.toast.dismiss();
        self.place_again();
        let on = {
            let state = self.state.borrow();
            state.credit_on.and_then(|i| state.credits.get(i)).map(|c| c.fingerprint)
        };
        if on == Some(keep.fingerprint) {
            self.credit_notice();
        }
    }

    /// The page is held as kept.
    fn held(&self, fingerprint: u64) -> bool {
        self.keeping.borrow().as_ref().is_some_and(|k| k.fingerprint == fingerprint)
    }

    /// The place and count again, after a keep or its Undo.
    fn place_again(&self) {
        if self.flows() {
            return;
        }
        let (on, index) = {
            let state = self.state.borrow();
            (state.credit_on, state.index)
        };
        if let Some(i) = on {
            self.credit_place(i);
        } else if self.is_end(index) {
            let total = self.story_pages();
            self.show_pages(total, total);
        } else {
            self.show_pages(self.page_number(index), self.story_pages());
        }
    }

    /// The scrubber's place and length with a held keep counted, as the chapter will have it.
    pub(super) fn held_pages(&self, at: usize, total: usize) -> (usize, usize) {
        let Some(at_end) = self.keeping.borrow().as_ref().map(|k| k.at_end) else {
            return (at, total);
        };
        let state = self.state.borrow();
        let on = state
            .credit_on
            .and_then(|i| state.credits.get(i))
            .map(|c| self.held(c.fingerprint));
        held_counts(at, total, at_end, on)
    }

    /// The next layout after a keep: the files and pictures under the old numbers go, and the
    /// kept page's picture stands under its new one so it stays on screen.
    pub(super) fn relay_kept(&self) -> Option<Relay> {
        let relay = self.state.borrow_mut().relay_kept.take()?;
        let mut state = self.state.borrow_mut();
        let kept = state.credits.iter().find(|c| c.fingerprint == relay.fingerprint).cloned();
        let texture = state.credit_textures.get(&relay.fingerprint).cloned();
        state.files.clear();
        state.textures.clear();
        state.failed.clear();
        state.failures.clear();
        state.asked.clear();
        if let (Some(kept), Some(texture)) = (kept, texture) {
            state.files.insert(relay.page, kept.file);
            state.textures.insert(relay.page, texture);
        }
        Some(relay)
    }

    /// A turn onto or past a shown credit page. True when it was one.
    pub(super) fn credit_step(self: &Rc<Self>, forward: bool, on_end: bool) -> bool {
        let (on, starts, ends, shown_start, shown_end, index) = {
            let state = self.state.borrow();
            let side = |end: bool| -> Vec<usize> {
                (0..state.credits.len()).filter(|i| state.credits[*i].at_end == end).collect()
            };
            (
                state.credit_on,
                side(false),
                side(true),
                state.shown_start,
                state.shown_end,
                state.index,
            )
        };
        if let Some(i) = on {
            if let Some(p) = starts.iter().position(|x| *x == i) {
                match (forward, starts.get(p + 1), p.checked_sub(1)) {
                    (true, Some(next), _) => {
                        self.show_credit(*next);
                    }
                    (true, None, _) => self.show_leaf(0, TurnHow::Turn),
                    (false, _, Some(before)) => {
                        self.show_credit(starts[before]);
                    }
                    (false, _, None) => {
                        self.send(ReaderCommand::GoBack {
                            session: self.state.borrow().session,
                        });
                    }
                }
            } else if let Some(p) = ends.iter().position(|x| *x == i) {
                match (forward, ends.get(p + 1), p.checked_sub(1)) {
                    (true, Some(next), _) => {
                        self.show_credit(*next);
                    }
                    (true, None, _) => {
                        let end = self.state.borrow().leaves.iter().position(|l| l.end_page);
                        match end {
                            Some(end) => self.show_leaf(end, TurnHow::Turn),
                            None => self.continue_on(),
                        }
                    }
                    (false, _, Some(before)) => {
                        self.show_credit(ends[before]);
                    }
                    (false, _, None) => self.show_leaf(self.last_page_leaf(), TurnHow::Turn),
                }
            }
            return true;
        }
        if !forward
            && !on_end
            && index == 0
            && shown_start
            && let Some(last) = starts.last()
        {
            return self.show_credit(*last);
        }
        if forward
            && !on_end
            && index == self.last_page_leaf()
            && shown_end
            && let Some(first) = ends.first()
        {
            return self.show_credit(*first);
        }
        if !forward
            && on_end
            && shown_end
            && let Some(last) = ends.last()
        {
            return self.show_credit(*last);
        }
        false
    }

    // ---------------------------------------------------------------------------------------
    // The strip

    /// Where the open chapter's first image starts in the scroll: under the chapters above and
    /// its credit row.
    pub(super) fn strip_base(&self) -> f64 {
        self.trail.height() + self.state.borrow().lead.as_ref().map_or(0.0, CreditBlock::height)
    }

    /// The credit pages after the open chapter's last image, while they show.
    pub(super) fn tail_height(&self) -> f64 {
        self.state.borrow().tail.as_ref().map_or(0.0, CreditBlock::height)
    }

    /// The scroll that puts `pos` at the top. The very top of the chapter is the top of its
    /// credit row, so the row shows as the chapter opens.
    pub(super) fn strip_scroll_for(&self, pos: StripPos) -> f64 {
        if pos.image == 0 && pos.offset <= 0.0 {
            return self.trail.height();
        }
        let (heights, spacing) = self.strip_heights();
        self.strip_base()
            + strip_target(&heights, spacing, pos.image as usize, Some(pos.offset))
    }

    /// Size the credit rows and pages at the column's width. True when a size changed.
    pub(super) fn size_strip_credits(&self, column: i32) -> bool {
        let state = self.state.borrow();
        let mut changed = false;
        for block in [&state.lead, &state.tail].into_iter().flatten() {
            changed |= block.size(column);
        }
        changed
    }

    /// Take the strip's credit rows away: a new layout, or another chapter.
    pub(super) fn drop_strip_credits(&self) {
        let (lead, tail) = {
            let mut state = self.state.borrow_mut();
            (state.lead.take(), state.tail.take())
        };
        for (block, parent) in [(lead, &self.strip_lead), (tail, &self.strip_tail)] {
            if let Some(block) = block
                && block.widget.parent().as_ref() == Some(parent.upcast_ref())
            {
                parent.remove(&block.widget);
            }
        }
    }

    /// The strip ran on into the next chapter: its credit rows go into the trail with it.
    pub(super) fn give_strip_credits(&self) -> (Option<CreditBlock>, Option<CreditBlock>) {
        let (lead, tail) = {
            let mut state = self.state.borrow_mut();
            (state.lead.take(), state.tail.take())
        };
        for (block, parent) in [(&lead, &self.strip_lead), (&tail, &self.strip_tail)] {
            if let Some(block) = block {
                parent.remove(&block.widget);
            }
        }
        (lead, tail)
    }

    /// The rows for the layout's credit pages: the row at the start, and the pages at the end
    /// once they were shown. The place holds while they come.
    fn build_strip_credits(self: &Rc<Self>) {
        self.drop_strip_credits();
        let (starts, ends, shown_start, shown_end) = {
            let state = self.state.borrow();
            let side = |end: bool| -> Vec<CreditPage> {
                state.credits.iter().filter(|c| c.at_end == end).cloned().collect()
            };
            (side(false), side(true), state.shown_start, state.shown_end)
        };
        if let Some(block) = self.credit_block(&starts, true) {
            if shown_start {
                block.reveal_now();
            }
            self.strip_lead.append(&block.widget);
            self.state.borrow_mut().lead = Some(block);
        }
        if shown_end && let Some(block) = self.credit_block(&ends, false) {
            block.reveal_now();
            self.strip_tail.append(&block.widget);
            self.state.borrow_mut().tail = Some(block);
        }
        self.size_strip();
        self.hold_strip_place();
    }

    /// One block of credit pages, at the start (with its row) or at the end.
    /// None when no page's picture can be read: a page that can't be read is left out.
    fn credit_block(self: &Rc<Self>, pages: &[CreditPage], start: bool) -> Option<CreditBlock> {
        let loaded: Vec<(CreditPage, gtk::gdk::Texture)> =
            pages.iter().filter_map(|p| Some((p.clone(), self.credit_texture(p)?))).collect();
        if loaded.is_empty() {
            return None;
        }
        let pages: Vec<CreditPage> = loaded.iter().map(|(p, _)| p.clone()).collect();
        let pages = pages.as_slice();
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add_css_class("strip-credits");
        let mut rows: Vec<gtk::Widget> = Vec::new();
        let mut frames = Vec::new();
        let row = start.then(|| {
            // The row is the button: its words and Show, on the ground, a column wide.
            let words = gtk::Label::new(Some(&skipped(pages.len())));
            words.add_css_class("haze");
            let show = gtk::Label::new(Some(&t::reader_credit_show()));
            show.add_css_class("reader-credit-show-word");
            let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            inner.set_halign(gtk::Align::Center);
            inner.append(&words);
            inner.append(&show);
            let line = gtk::Button::new();
            line.set_child(Some(&inner));
            line.add_css_class("flat");
            line.add_css_class("strip-credit-row");
            line.set_cursor_from_name(Some("pointer"));
            line.update_property(&[
                gtk::accessible::Property::Label(&words.label()),
                gtk::accessible::Property::Description(&t::reader_credit_show_hint()),
            ]);
            line.connect_clicked({
                let me = self.me.clone();
                move |_| {
                    if let Some(this) = me.upgrade() {
                        this.show_credits(false);
                    }
                }
            });
            let caption = self.credit_caption(pages[0].fingerprint);
            // The line fades over the caption by the veil, with or without Reduce Motion.
            let stack = gtk::Stack::new();
            stack.add_named(&line, Some("line"));
            stack.add_named(&caption, Some("caption"));
            let veil = gtk::Overlay::new();
            veil.set_child(Some(&stack));
            rows.push(stack.clone().upcast());
            widget.append(&veil);
            Row { veil, stack }
        });
        let pictures = gtk::Box::new(gtk::Orientation::Vertical, 0);
        for (i, (page, texture)) in loaded.iter().enumerate() {
            if i > 0 || !start {
                let caption = self.credit_caption(page.fingerprint);
                rows.push(caption.clone().upcast());
                pictures.append(&caption);
            }
            let picture = gtk::Picture::new();
            picture.set_content_fit(gtk::ContentFit::Fill);
            picture.set_can_shrink(true);
            picture.add_css_class("strip-leaf");
            picture.update_property(&[gtk::accessible::Property::Label(
                &t::reader_credit_page_label(),
            )]);
            let ratio = texture.width() as f32 / texture.height().max(1) as f32;
            picture.set_paintable(Some(texture));
            pictures.append(&picture);
            frames.push((picture, ratio.max(0.05)));
        }
        // Slide up keeps the pages' top under the row as they grow, so they come in top first
        // and in place; with animations off they are there at once, with no gap before.
        let revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideUp)
            .transition_duration(300)
            .child(&pictures)
            .reveal_child(false)
            .build();
        // Everything under it moves as the pages grow; the strip says where it is after.
        revealer.connect_child_revealed_notify({
            let me = self.me.clone();
            move |_| {
                if let Some(this) = me.upgrade() {
                    this.strip_moved();
                }
            }
        });
        widget.append(&revealer);
        Some(CreditBlock { widget, row, revealer, frames, rows })
    }

    /// A credit page's picture, decoded once and kept through every layout that follows a
    /// resize. None when its file can't be read.
    fn credit_texture(&self, page: &CreditPage) -> Option<gtk::gdk::Texture> {
        let cached = self.state.borrow().credit_textures.get(&page.fingerprint).cloned();
        let texture = cached.or_else(|| load_texture(&page.file))?;
        self.state.borrow_mut().credit_textures.insert(page.fingerprint, texture.clone());
        Some(texture)
    }

    /// "Skipped as a credit page · Not a credit page", above a shown page in a strip.
    fn credit_caption(&self, fingerprint: u64) -> gtk::Box {
        let words = gtk::Label::new(Some(&t::reader_credit_shown()));
        words.add_css_class("haze");
        let not = gtk::Button::with_label(&t::reader_credit_not());
        not.add_css_class("flat");
        not.add_css_class("reader-credit-show");
        not.set_valign(gtk::Align::Center);
        not.connect_clicked({
            let me = self.me.clone();
            move |_| {
                if let Some(this) = me.upgrade() {
                    this.not_a_credit(fingerprint);
                }
            }
        });
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        inner.set_halign(gtk::Align::Center);
        inner.set_valign(gtk::Align::Center);
        inner.set_hexpand(true);
        inner.append(&words);
        inner.append(&not);
        // A column wide, as the row it replaces, with its words in the middle.
        let caption = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        caption.add_css_class("strip-credit-caption");
        caption.append(&inner);
        caption
    }

    /// Show the strip's credit pages: the row grows into them where it is, or the pages after
    /// the last image come back above the marker, and the strip goes to them.
    fn show_strip_credits(self: &Rc<Self>, at_end: bool) {
        if !at_end {
            let lead = self
                .state
                .borrow()
                .lead
                .as_ref()
                .filter(|l| !l.shown())
                .map(CreditBlock::grower);
            if let Some((row, revealer)) = lead {
                grow(row.as_ref(), &revealer);
            }
            // The person's own move: the place follows the scroll from here.
            self.release_strip();
            return;
        }
        let ends: Vec<CreditPage> =
            self.state.borrow().credits.iter().filter(|c| c.at_end).cloned().collect();
        if ends.is_empty() {
            return;
        }
        self.release_strip();
        if self.state.borrow().tail.is_none() {
            let Some(block) = self.credit_block(&ends, false) else {
                self.credit_failed(true);
                return;
            };
            self.strip_tail.append(&block.widget);
            self.state.borrow_mut().tail = Some(block);
            self.size_strip();
        }
        let tail = self.state.borrow().tail.as_ref().map(CreditBlock::grower);
        if let Some((row, revealer)) = tail {
            grow(row.as_ref(), &revealer);
        }
        self.fill_end();
        let (heights, spacing) = self.strip_heights();
        let foot = strip_target(&heights, spacing, heights.len(), None) - f64::from(spacing);
        self.scroll_strip_to(self.strip_base() + foot);
    }

    /// The line in the marker into the next chapter for the pages skipped at the open
    /// chapter's end, with Show.
    pub(super) fn marker_credit_line(&self) -> Option<gtk::Widget> {
        let text = {
            let state = self.state.borrow();
            (state.mode == Some(ReaderMode::Strip)).then(|| end_line(&state)).flatten()?
        };
        let (row, words, show) = show_line();
        words.set_label(&text);
        show.update_property(&[gtk::accessible::Property::Label(&text)]);
        show.connect_clicked({
            let me = self.me.clone();
            move |_| {
                if let Some(this) = me.upgrade() {
                    this.show_credits(true);
                }
            }
        });
        Some(row.upcast())
    }

    // ---------------------------------------------------------------------------------------
    // Parts

    /// What the scrubber says on a part's first page: "Part 2 starts at page 25", and the part
    /// no group has when its place is there.
    pub(super) fn part_said(&self, at: usize) -> Option<String> {
        let total = self.story_pages();
        let state = self.state.borrow();
        part_words(state.parts.as_ref()?, at, total)
    }

    /// The part tip centred over the scrubber's thumb, 8 px above the track, inside the reader.
    fn place_part_tip(&self) {
        let (start, end) = self.scrub.slider_range();
        let thumb = gtk::graphene::Point::new(((start + end) / 2) as f32, 0.0);
        let Some(over) = self.scrub.compute_point(&self.widget, &thumb) else { return };
        // A measure counts the widget's own margins, which hold the last placement.
        let tip = &self.part_tip;
        let (_, width, _, _) = tip.measure(gtk::Orientation::Horizontal, -1);
        let width = width - tip.margin_start() - tip.margin_end();
        let (_, height, _, _) = tip.measure(gtk::Orientation::Vertical, -1);
        let height = height - tip.margin_top() - tip.margin_bottom();
        let room = (self.widget.width() - width).max(0);
        let x = (over.x().round() as i32 - width / 2).clamp(0, room);
        let y = (over.y().round() as i32 - height - 8).max(0);
        if self.part_tip.margin_start() != x {
            self.part_tip.set_margin_start(x);
        }
        if self.part_tip.margin_top() != y {
            self.part_tip.set_margin_top(y);
        }
    }

    /// The preview over the thumb on a tick: "Page 25 · part 2", the group when the part is
    /// another group's, or the part no group has.
    pub(super) fn part_tip_at(&self, at: usize) {
        let words = {
            let state = self.state.borrow();
            state.parts.as_ref().and_then(|parts| tip_words(parts, at, self.story_pages()))
        };
        let Some(words) = words else {
            self.part_tip_fade.set(&self.part_tip, false);
            return;
        };
        self.part_tip.set_label(&words);
        self.place_part_tip();
        self.part_tip_fade.set(&self.part_tip, true);
        let ask = {
            let mut state = self.state.borrow_mut();
            state.tip_ask = state.tip_ask.wrapping_add(1);
            state.tip_ask
        };
        // The thumb settles a frame after the value and the chrome may still be sliding in, so
        // the tip follows it each frame until a newer tip or the hold ends.
        let me = self.me.clone();
        self.part_tip.add_tick_callback(move |_, _| match me.upgrade() {
            Some(this) if this.state.borrow().tip_ask == ask => {
                this.place_part_tip();
                glib::ControlFlow::Continue
            }
            _ => glib::ControlFlow::Break,
        });
        let me = self.me.clone();
        glib::timeout_add_local_once(TIP_HOLD, move || {
            if let Some(this) = me.upgrade()
                && this.state.borrow().tip_ask == ask
            {
                this.part_tip_fade.set(&this.part_tip, false);
            }
        });
    }
}

// -------------------------------------------------------------------------------------------
// Shots

#[cfg(feature = "fixtures")]
impl Reader {
    /// The open chapter as if the engine had skipped `start` credit pages at its start and
    /// `end` at its end, a made-up group's credits page drawn for each.
    pub fn shot_credits(self: &Rc<Self>, start: usize, end: usize) {
        let mut pages = Vec::new();
        for (i, at_end) in
            std::iter::repeat_n(false, start).chain(std::iter::repeat_n(true, end)).enumerate()
        {
            let Some(file) = credit_file(i) else { return };
            pages.push(CreditPage {
                page: if at_end { 200 + i as u32 } else { i as u32 },
                at_end,
                file,
                fingerprint: 0x5eed_0000 + i as u64,
            });
        }
        self.credit_pages(pages);
    }

    /// One credit page skipped at the start whose file can't be read.
    pub fn shot_credits_broken(self: &Rc<Self>) {
        let Some(mut file) = credit_file(0) else { return };
        file.path = std::env::temp_dir()
            .join("kaeri-shot-credit-gone.png") // i18n-skip
            .to_string_lossy()
            .into_owned();
        self.credit_pages(vec![CreditPage {
            page: 0,
            at_end: false,
            file,
            fingerprint: 0x5eed,
        }]);
    }

    /// Undo on the held keep, as its button or Ctrl+Z would.
    pub fn shot_undo_keep(&self) {
        self.undo_keep();
    }

    /// The open chapter in two parts with a third no group has after the last.
    pub fn shot_parts_missing_end(&self, second: u32) {
        self.hide_notice();
        let part = |number, first_page| ChapterPart {
            number,
            first_page,
            group: Some("Lanternlight Scans".to_owned()), // i18n-skip
            other_group: false,
        };
        self.chapter_parts(vec![part(1, 1), part(2, second)], vec![3]);
    }

    /// Page 1, where a chapter opened from its start lands.
    pub fn shot_go_to_first(self: &Rc<Self>) {
        self.hide_notice();
        self.show_leaf(0, TurnHow::Placed);
    }

    /// A strip at its very top, the credit row in sight.
    pub fn shot_strip_top(&self) {
        self.state.borrow_mut().strip_anchor = Some(StripPos { image: 0, offset: 0.0 });
        self.hold_strip_place();
    }

    /// Show the credit pages at the start, or the end, as the pill or Show would.
    pub fn shot_show_credits(self: &Rc<Self>, at_end: bool) {
        self.show_credits(at_end);
    }

    /// Not a credit page, on the credit page on screen.
    pub fn shot_not_a_credit(&self) {
        let print = {
            let state = self.state.borrow();
            state.credit_on.and_then(|i| state.credits.get(i)).map(|c| c.fingerprint)
        };
        if let Some(print) = print {
            self.not_a_credit(print);
        }
    }

    /// The open chapter in two parts, the second from another group, with its second half
    /// at `second` and a third part no group has.
    pub fn shot_parts(&self, second: u32, missing: bool) {
        self.hide_notice();
        let total = self.story_pages() as u32;
        let mut parts = vec![
            ChapterPart {
                number: 1,
                first_page: 1,
                group: Some("Lanternlight Scans".to_owned()), // i18n-skip
                other_group: false,
            },
            ChapterPart {
                number: 2,
                first_page: second,
                group: Some("Saltmarsh Team".to_owned()), // i18n-skip
                other_group: true,
            },
        ];
        let mut gone = Vec::new();
        if missing {
            parts.push(ChapterPart {
                number: 4,
                first_page: (second + total) / 2,
                group: Some("Lanternlight Scans".to_owned()), // i18n-skip
                other_group: false,
            });
            gone.push(3);
        }
        self.chapter_parts(parts, gone);
    }

    /// The scrubber moved to page `at`, as a drag would.
    pub fn shot_scrub(self: &Rc<Self>, at: usize) {
        self.scrub.set_value(at as f64);
        self.scrubbed(at as f64);
    }
}

/// A made-up group's credits page, drawn once into a file: its name over a lantern, and who did
/// what.
#[cfg(feature = "fixtures")]
fn credit_file(i: usize) -> Option<kaeri_model::reader::PageFile> {
    use gtk::cairo;
    let (w, h) = (900, 1280);
    let path = std::env::temp_dir().join(format!("kaeri-shot-credit-{i}.png"));
    if !path.exists() {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).ok()?;
        {
            let cr = cairo::Context::new(&surface).ok()?;
            cr.set_source_rgb(0.973, 0.973, 0.957);
            cr.paint().ok()?;
            cr.set_source_rgb(0.11, 0.11, 0.11);
            cr.set_line_width(4.0);
            cr.rectangle(60.0, 60.0, f64::from(w) - 120.0, f64::from(h) - 120.0);
            cr.stroke().ok()?;
            cr.move_to(f64::from(w) / 2.0, 60.0);
            cr.line_to(f64::from(w) / 2.0, 190.0);
            cr.stroke().ok()?;
            cr.arc(f64::from(w) / 2.0, 330.0, 140.0, 0.0, std::f64::consts::TAU);
            cr.stroke().ok()?;
            for dx in [-70.0, 0.0, 70.0] {
                cr.save().ok()?;
                cr.translate(f64::from(w) / 2.0 + dx * 0.4, 330.0);
                cr.scale(0.45 + dx.abs() / 200.0, 1.0);
                cr.arc(0.0, 0.0, 138.0, 0.0, std::f64::consts::TAU);
                cr.restore().ok()?;
                cr.stroke().ok()?;
            }
            cr.rectangle(f64::from(w) / 2.0 - 60.0, 180.0, 120.0, 24.0);
            cr.rectangle(f64::from(w) / 2.0 - 50.0, 456.0, 100.0, 22.0);
            cr.fill().ok()?;
            const SERIF: &str = "Serif"; // i18n-skip
            const GROUP: &str = "Lanternlight Scans"; // i18n-skip
            const THANKS: &str = "Thanks for reading"; // i18n-skip
            const ROLES: [(&str, &str); 4] = [
                ("Translation", "Ren"),  // i18n-skip
                ("Cleaning", "Ada"),     // i18n-skip
                ("Typesetting", "Juno"), // i18n-skip
                ("Checking", "Ines"),    // i18n-skip
            ];
            cr.select_font_face(SERIF, cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(64.0);
            let centred = |cr: &cairo::Context, text: &str, y: f64| -> Option<()> {
                let extents = cr.text_extents(text).ok()?;
                cr.move_to((f64::from(w) - extents.width()) / 2.0 - extents.x_bearing(), y);
                cr.show_text(text).ok()
            };
            centred(&cr, if i.is_multiple_of(2) { GROUP } else { THANKS }, 620.0)?;
            cr.set_font_size(34.0);
            for (n, (role, who)) in ROLES.iter().enumerate() {
                let y = 740.0 + 58.0 * n as f64;
                cr.select_font_face(SERIF, cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                let extents = cr.text_extents(role).ok()?;
                cr.move_to(f64::from(w) / 2.0 - 24.0 - extents.width(), y);
                cr.show_text(role).ok()?;
                cr.select_font_face(SERIF, cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                cr.move_to(f64::from(w) / 2.0 + 24.0, y);
                cr.show_text(who).ok()?;
            }
            cr.set_line_width(3.0);
            cr.rectangle(170.0, 1030.0, f64::from(w) - 340.0, 120.0);
            cr.stroke().ok()?;
            cr.select_font_face(SERIF, cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            cr.set_font_size(30.0);
            centred(&cr, "lanternlight.example", 1100.0)?; // i18n-skip
        }
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().ok()?.to_vec();
        let texture = gtk::gdk::MemoryTexture::new(
            w,
            h,
            gtk::gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &glib::Bytes::from_owned(data),
            stride,
        );
        texture.save_to_png(&path).ok()?;
    }
    Some(kaeri_model::reader::PageFile {
        path: path.to_string_lossy().into_owned(),
        encoding: kaeri_model::reader::PageEncoding::Original { mime: "image/png".to_owned() }, // i18n-skip
        width: w as u32,
        height: h as u32,
        crop: None,
        colour: false,
        bytes: 0,
    })
}

/// The number a kept page has once the engine lays the chapter out with it: its place in the
/// chapter less the credit pages still left out before it.
fn kept_number(credits: &[CreditPage], fingerprint: u64) -> Option<u32> {
    let kept = credits.iter().find(|c| c.fingerprint == fingerprint)?;
    let before =
        credits.iter().filter(|c| c.fingerprint != fingerprint && c.page < kept.page).count()
            as u32;
    Some(kept.page.saturating_sub(before))
}

/// Page `at` of `total` with a held keep counted: one more page, and one on for the pages
/// after a kept page at the start. `on` says whether a credit page is on screen and whether it
/// is the held one, which reads as the first or last page.
fn held_counts(at: usize, total: usize, at_end: bool, on: Option<bool>) -> (usize, usize) {
    if total == 0 {
        return (at, total);
    }
    match on {
        Some(true) => (if at_end { total + 1 } else { 1 }, total + 1),
        Some(false) => (at, total + 1),
        None => (at + usize::from(!at_end), total + 1),
    }
}

/// Where a missing part would start: at the next part there is, or at the end.
fn missing_at(parts: &[ChapterPart], missing: u32, total: f64) -> f64 {
    parts.iter().find(|p| p.number > missing).map_or(total, |p| f64::from(p.first_page))
}

/// The part that starts at page `at`, past the first, and the part no group has whose place
/// is there: it would start where the next part does, or at the end.
fn parts_at(parts: &Parts, at: usize, total: usize) -> (Option<u32>, Option<u32>) {
    let first = parts.parts.first().map(|p| p.number);
    let part = parts
        .parts
        .iter()
        .find(|p| Some(p.number) != first && p.first_page as usize == at)
        .map(|p| p.number);
    let missing = parts
        .missing
        .iter()
        .find(|m| missing_at(&parts.parts, **m, total as f64) as usize == at)
        .copied();
    (part, missing)
}

fn part_missing(part: u32) -> String {
    say::msg(&Msg::Reading(ReadingMsg::ReadingRowPartMissing {
        count: 1,
        part: i64::from(part),
    }))
}

/// The part preview's words at page `at`, or none off the ticks: "Page 11 · part 2", with the
/// part no group has last, "Page 14 · part 4 · part 3 missing".
fn tip_words(parts: &Parts, at: usize, total: usize) -> Option<String> {
    let (part, missing) = parts_at(parts, at, total);
    let mut words = match part {
        Some(part) => t::reader_part_preview(at as i64, i64::from(part)),
        None => t::chapter_state_page(at as i64),
    };
    match (part, missing) {
        (None, None) => return None,
        (_, Some(missing)) => {
            words.push_str(" \u{b7} ");
            words.push_str(&part_missing(missing));
        }
        _ => {}
    }
    Some(words)
}

/// What the scrubber says there: "Part 4 starts at page 14, part 3 missing".
fn part_words(parts: &Parts, at: usize, total: usize) -> Option<String> {
    let (part, missing) = parts_at(parts, at, total);
    let missing = missing.map(part_missing);
    match (part, missing) {
        (Some(part), Some(missing)) => {
            Some([t::reader_part_starts(i64::from(part), at as i64), missing].join(", "))
        }
        (Some(part), None) => Some(t::reader_part_starts(i64::from(part), at as i64)),
        (None, Some(missing)) => Some(say::capitalized(&missing)),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use kaeri_model::reader::{ChapterPart, CreditPage, PageEncoding, PageFile};

    use super::{Parts, held_counts, kept_number, missing_at, part_words, tip_words};

    fn part(number: u32, first_page: u32, group: Option<&str>, other: bool) -> ChapterPart {
        ChapterPart { number, first_page, group: group.map(str::to_owned), other_group: other }
    }

    #[test]
    fn the_preview_names_the_part_on_its_tick_only() {
        let parts = Parts {
            parts: vec![part(1, 1, None, false), part(2, 25, None, false)],
            missing: vec![],
        };
        assert_eq!(tip_words(&parts, 25, 46).as_deref(), Some("Page 25 \u{b7} part 2"));
        assert_eq!(tip_words(&parts, 24, 46), None);
        assert_eq!(tip_words(&parts, 1, 46), None);
        assert_eq!(part_words(&parts, 25, 46).as_deref(), Some("Part 2 starts at page 25"));
    }

    #[test]
    fn the_preview_leaves_another_group_out() {
        let parts = Parts {
            parts: vec![part(1, 1, Some("Group A"), false), part(2, 25, Some("Group B"), true)],
            missing: vec![],
        };
        assert_eq!(tip_words(&parts, 25, 46).as_deref(), Some("Page 25 \u{b7} part 2"));
    }

    #[test]
    fn a_missing_part_is_named_where_the_next_one_starts() {
        let parts = vec![part(1, 1, None, false), part(4, 14, None, false)];
        assert_eq!(missing_at(&parts, 3, 20.0), 14.0);
        assert_eq!(missing_at(&parts, 5, 20.0), 20.0);
        let parts = Parts { parts, missing: vec![3] };
        assert_eq!(
            tip_words(&parts, 14, 20).as_deref(),
            Some("Page 14 \u{b7} part 4 \u{b7} part 3 missing")
        );
        assert_eq!(
            part_words(&parts, 14, 20).as_deref(),
            Some("Part 4 starts at page 14, part 3 missing")
        );
    }

    #[test]
    fn a_missing_part_after_the_last_sits_at_the_end() {
        let parts = Parts {
            parts: vec![part(1, 1, None, false), part(2, 10, None, false)],
            missing: vec![3],
        };
        assert_eq!(tip_words(&parts, 20, 20).as_deref(), Some("Page 20 \u{b7} part 3 missing"));
        assert_eq!(part_words(&parts, 20, 20).as_deref(), Some("Part 3 missing"));
    }

    fn credit(page: u32, at_end: bool, fingerprint: u64) -> CreditPage {
        CreditPage {
            page,
            at_end,
            fingerprint,
            file: PageFile {
                path: String::new(),
                encoding: PageEncoding::Original { mime: String::new() },
                width: 1,
                height: 1,
                crop: None,
                colour: false,
                bytes: 0,
            },
        }
    }

    #[test]
    fn a_kept_page_takes_its_number_less_the_pages_still_left_out_before_it() {
        // Five pages: page 0 and page 4 left out, three in between.
        let credits = vec![credit(0, false, 1), credit(4, true, 2)];
        assert_eq!(kept_number(&credits, 1), Some(0), "kept at the start: page 0");
        assert_eq!(kept_number(&credits, 2), Some(3), "kept at the end: after the three");
        let two = vec![credit(0, false, 1), credit(1, false, 2)];
        assert_eq!(kept_number(&two, 2), Some(0), "the second of two, the first still out");
        assert_eq!(kept_number(&credits, 9), None);
    }

    #[test]
    fn a_held_keep_counts_at_once() {
        // Kept at the start, on the kept page: page 1 of 38 where 37 were.
        assert_eq!(held_counts(1, 37, false, Some(true)), (1, 38));
        // On a story page after it: one on.
        assert_eq!(held_counts(5, 37, false, None), (6, 38));
        // Kept at the end, on it: the last page.
        assert_eq!(held_counts(37, 37, true, Some(true)), (38, 38));
        assert_eq!(held_counts(5, 37, true, None), (5, 38));
        // Another credit page on screen keeps its place.
        assert_eq!(held_counts(1, 37, false, Some(false)), (1, 38));
        assert_eq!(held_counts(0, 0, false, None), (0, 0));
    }
}
