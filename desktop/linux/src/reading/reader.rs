//! The reader: pages of a chapter, one at a time, or a long strip scrolling, or a novel in a web
//! view. The engine opens the chapter, prepares the pages and keeps the place; this shows the
//! page files it names, turns them on the person's say, and reports what was seen.
//!
//! It sits over the window like the player does: Escape closes it. The chrome, a header over the
//! page and the group at its foot with the scrubber, is out of the way until the pointer moves or
//! the middle of the page is clicked; it leaves 2 s after the pointer stops, on the next click in
//! the middle, or on a turn by key.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::{gdk, glib};
use kaeri_core::{
    books::{ChapterId, WorkId},
    media::Medium,
};
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    event::Event,
    reader::{
        Leaf, NoNext, OpenAt, PageEncoding, PageFile, PixelFormat, ReadDirection, ReaderAction,
        ReaderCommand, ReaderEvent, ReaderMode, ReaderNotice, ReaderSettingKey,
        ReaderSettingsView, ReaderSurface, ReaderValue, ReaderView, ReaderWait, StripPos,
        TurnHow,
    },
    reading::{ChapterRow, ReadingCommand, ReadingEvent, ReadingFailure},
};

use super::{
    chapters::{Ask, Chapters, Here, fit_size as chapters_fit},
    listen,
    novel::{NovelSays, NovelView},
};
use crate::{motion, say, window::Ctx};

mod credits;
mod end;
mod keys;
mod sheet;
mod trail;

/// The chrome leaves this long after the pointer stops.
const CHROME_REST: Duration = Duration::from_secs(2);
/// The group's width on a wide window (the reader board's Linux frame).
const GROUP_WIDTH: i32 = 640;

/// Nothing for this long, then the page plate.
const SILENT: Duration = Duration::from_millis(600);
/// A page still loading says so on its plate from this long.
const SAY_LOADING: Duration = Duration::from_secs(4);
/// How long a chapter's open goes on before its plate offers another way, as the player's start
/// does.
const LONG_WAIT: Duration = Duration::from_secs(15);

/// What the page plate holds the place of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Waiting {
    /// A chapter opening.
    Chapter,
    /// A page, from 0, still loading.
    Page(u32),
}

/// What the failed plate says could not be had, and so what Try again does.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Failed {
    #[default]
    Page,
    /// The chapter could not open: Choose source leads, and Try again opens it again.
    Chapter,
}

#[derive(Default)]
struct State {
    session: u64,
    /// The chapter opening, by its number, for "Couldn't open chapter 13".
    opening_number: Option<String>,
    work: Option<WorkId>,
    medium: Option<Medium>,
    chapter: Option<ChapterRow>,
    next: Option<ChapterRow>,
    previous: Option<ChapterRow>,
    why_none: Option<NoNext>,
    mode: Option<ReaderMode>,
    direction: Option<ReadDirection>,
    start_page: Option<u32>,
    /// Where a strip was left: its image and how far down it.
    start_strip: Option<StripPos>,
    leaves: Vec<Leaf>,
    files: HashMap<u32, PageFile>,
    textures: HashMap<u32, gdk::Texture>,
    failed: HashSet<u32>,
    /// Why each failed page failed, for the line under its heading.
    failures: HashMap<u32, ReadingFailure>,
    asked: HashSet<u32>,
    index: usize,
    /// The page a new layout kept, while the leaf that holds it stays on screen: a spread's
    /// second page shown alone before the spread comes back alone after it.
    held_page: Option<u32>,
    settings: Option<ReaderSettingsView>,
    /// Each strip image and its width over height.
    strip_frames: Vec<(gtk::Picture, f32)>,
    /// The place a strip holds while its images land and change height: where it opened, until
    /// the person scrolls.
    strip_anchor: Option<StripPos>,
    /// The chapter counted as read, with the trackers it went to.
    read: Option<Vec<String>>,
    /// Where the next chapter came from, by name.
    next_source: Option<String>,
    /// When the next chapter is an extra: the first whole chapter after it.
    skip_to: Option<ChapterRow>,
    /// How many extras the skip passes over.
    extras: u32,
    /// The next chapter's first page, for the end page.
    preview: Option<(ChapterId, gdk::Texture)>,
    /// The end page is up over a novel.
    novel_end: bool,
    /// The chapter a strip ran on into, until it has opened.
    flowing: Option<ChapterId>,
    /// The open chapter came in under the strip's marker: it starts where the scroll is.
    flowed: bool,
    /// The mode changed in the reader: its next layout shows the chapter in the new one.
    switched: bool,
    /// The demo's work being read, with its credit capsule in the header on page 1.
    demo: Option<(kaeri_model::sources::DemoWorkInfo, gtk::Widget)>,
    /// The open chapter's credit pages, skipped, from the engine's last layout.
    credits: Vec<kaeri_model::reader::CreditPage>,
    /// The credit page on screen in paged reading, by its place in `credits`.
    credit_on: Option<usize>,
    /// The credit pages at the start, or at the end, were shown: they stay until the chapter
    /// closes.
    shown_start: bool,
    shown_end: bool,
    /// The line on page 1 has had its 4 s, or a turn took it.
    credit_line_done: bool,
    /// The credit page that was on screen when a new layout came: shown again if the layout
    /// still skips it.
    credit_wanted: Option<u64>,
    credit_textures: HashMap<u64, gdk::Texture>,
    /// A strip's credit pages above its first image and after its last.
    lead: Option<credits::CreditBlock>,
    tail: Option<credits::CreditBlock>,
    /// The open chapter's parts, for the scrubber's ticks and its preview.
    parts: Option<credits::Parts>,
    /// Each part preview shown counts up; an older one's timer leaves a newer one alone.
    tip_ask: u32,
    /// A page kept from the credit pages was sent: the next layout numbers pages afresh.
    relay_kept: Option<credits::Relay>,
}

pub struct Reader {
    pub widget: gtk::Overlay,
    /// Itself, for the callbacks of widgets made after it.
    me: std::rc::Weak<Self>,
    ctx: std::rc::Weak<Ctx>,
    show: Rc<dyn Fn(bool)>,
    state: RefCell<State>,
    /// The stack inside its veil: a turn lays the page as it stood over the new one.
    veil: gtk::Overlay,
    stack: gtk::Stack,
    picture: gtk::Picture,
    paged_scroll: gtk::ScrolledWindow,
    strip: gtk::Box,
    /// The end page's place at the foot of a strip.
    strip_foot: gtk::Box,
    /// The chapters a strip ran on from, and the marker into the next one.
    trail: trail::Trail,
    strip_scroll: gtk::ScrolledWindow,
    /// Each scroll the strip asks for; an older one that has not landed yet gives way.
    strip_scroll_ask: Rc<Cell<u32>>,
    /// The strip's running scroll, for Space and Page Down.
    strip_slide: RefCell<Option<adw::SpringAnimation>>,
    status: adw::StatusPage,
    end: Rc<end::EndPage>,
    /// A page that failed, in the page's place.
    failed_plate: gtk::Box,
    failed_title: gtk::Label,
    failed_reason: gtk::Label,
    failed_page_no: Cell<u32>,
    failed_kind: Cell<Failed>,
    failed_again: gtk::Button,
    failed_source: gtk::Button,
    failed_hint: gtk::Label,
    /// The page plate: a page's place on Raised while it, or the chapter, is on its way.
    wait_plate: gtk::Box,
    wait_number: gtk::Label,
    wait_line: gtk::Label,
    /// Under a chapter's plate once its wait has gone on: another source and Go back.
    wait_more: gtk::Box,
    /// Counts a chapter's wait each second, from when its plate shows.
    wait_ticker: RefCell<Option<glib::SourceId>>,
    waiting: Cell<Option<Waiting>>,
    /// The last open asked for, to open again after a failure.
    asked_open: RefCell<Option<(WorkId, Option<ChapterId>)>>,
    /// The surface the engine was last told about, in an open or on its own.
    told: Cell<Option<ReaderSurface>>,
    /// The control that opened the reader, which has the keyboard back when it closes.
    opener: Rc<keys::Opener>,
    /// The toasts over the pages: a shown credit page's notice.
    toasts: adw::ToastOverlay,
    /// Adds the toasts over the pages, so that they can all be dismissed safely.
    notices: crate::notices::Notices,
    /// "1 credit page skipped", top trailing on page 1; the whole pill shows the pages.
    credit_line: gtk::Button,
    credit_fade: motion::Fader,
    credit_timer: RefCell<Option<glib::SourceId>>,
    credit_toast: RefCell<Option<adw::Toast>>,
    /// Not a credit page, held while its Undo shows: sent to the engine when the notice times
    /// out, at a second keep, or as the chapter closes or another opens.
    keeping: RefCell<Option<credits::Keeping>>,
    /// A strip's credit pages: above its first image, and after its last.
    strip_lead: gtk::Box,
    strip_tail: gtk::Box,
    /// "Page 25 · part 2" over the scrubber's thumb on a part's tick.
    part_tip: gtk::Label,
    part_tip_fade: motion::Fader,
    /// A line over the page for a while: resumed, moved, mode detected.
    notice: gtk::Box,
    notice_text: gtk::Label,
    notice_button: gtk::Button,
    notice_action: Cell<Option<ReaderAction>>,
    notice_fade: motion::Fader,
    notice_timer: RefCell<Option<glib::SourceId>>,
    /// The header over the page and the group at its foot: they come and go together.
    head: adw::HeaderBar,
    group: gtk::Box,
    head_fade: motion::Fader,
    group_fade: motion::Fader,
    /// The pointer over the group keeps the chrome.
    group_hover: gtk::EventControllerMotion,
    /// The chrome stays: a shot's.
    pinned: Cell<bool>,
    title: gtk::Label,
    group_title: gtk::Label,
    line: gtk::Label,
    scrub: gtk::Scale,
    scrub_at: gtk::Label,
    scrub_total: gtk::Label,
    scrub_timer: RefCell<Option<glib::SourceId>>,
    /// Back to page N, over the scrubber, while the engine offers it after a jump.
    back_to: gtk::Button,
    back_to_label: gtk::Label,
    back_to_page: Cell<Option<u32>>,
    back_to_fade: motion::Fader,
    settings: gtk::MenuButton,
    /// The settings popover, filled from the engine's settings.
    sheet: gtk::Popover,
    sheet_shape_now: RefCell<sheet::Shape>,
    /// A change came while the popover was open; it is filled again as it closes.
    sheet_stale: Cell<bool>,
    /// A picked reading mode's list is sliding away: the rows change after it.
    sheet_held: Cell<bool>,
    /// Go to page, from G.
    go_to: gtk::Popover,
    go_to_field: gtk::SpinButton,
    /// Chapters: the pages of this one and the series' chapters.
    chapters: Rc<Chapters>,
    chapters_button: gtk::MenuButton,
    /// What the plate says while a chapter opens: the chapter, or the source asked.
    opening_line: RefCell<String>,
    loading_timer: RefCell<Option<glib::SourceId>>,
    loading_line_timer: RefCell<Option<glib::SourceId>>,
    hide_timer: RefCell<Option<glib::SourceId>>,
    strip_timer: RefCell<Option<glib::SourceId>>,
    novel: RefCell<Option<Rc<NovelView>>>,
    /// From `open` until `close`. A closed reader takes no more of the engine's events, and lets
    /// go of its pages and its novel view once it is off screen.
    showing: Cell<bool>,
    /// The window's full screen before the reader first changed it, given back on close.
    full_before: FullBefore,
}

impl Reader {
    pub fn new(ctx: &Rc<Ctx>, show: Rc<dyn Fn(bool)>) -> Rc<Self> {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_can_shrink(true);
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        // Fit screen keeps the page inside the window; width and height let it run past and
        // scroll.
        let paged_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .child(&picture)
            .build();
        // A centred column; each image is sized to the strip width setting, and the end page
        // follows the last one.
        let strip = gtk::Box::new(gtk::Orientation::Vertical, 0);
        strip.set_halign(gtk::Align::Center);
        let strip_foot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        strip_foot.set_margin_top(64);
        strip_foot.set_margin_bottom(160);
        let trail = trail::Trail::new();
        let strip_lead = gtk::Box::new(gtk::Orientation::Vertical, 0);
        strip_lead.set_halign(gtk::Align::Center);
        let strip_tail = gtk::Box::new(gtk::Orientation::Vertical, 0);
        strip_tail.set_halign(gtk::Align::Center);
        let strip_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        strip_column.append(&trail.above);
        strip_column.append(&strip_lead);
        strip_column.append(&strip);
        strip_column.append(&strip_tail);
        strip_column.append(&strip_foot);
        strip_column.append(&trail.foot);
        let strip_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&strip_column)
            .build();
        let status = adw::StatusPage::new();
        status.add_css_class("compact");

        // A failed page, in the page's place: left-aligned on a Raised plate, vermilion only on
        // the glyph, Try again the one action, and turning past it goes on.
        let glyph = gtk::Image::from_icon_name("dialog-warning-symbolic");
        glyph.add_css_class("reader-failed-glyph");
        let failed_title = gtk::Label::new(None);
        failed_title.add_css_class("reader-failed-title");
        failed_title.set_xalign(0.0);
        failed_title.set_wrap(true);
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        heading.append(&glyph);
        heading.append(&failed_title);
        let failed_reason = gtk::Label::new(None);
        failed_reason.add_css_class("haze");
        failed_reason.set_xalign(0.0);
        failed_reason.set_wrap(true);
        let again = gtk::Button::with_label(&t::reader_try_again());
        again.add_css_class("pill");
        again.add_css_class("suggested-action");
        // A chapter that could not open leads with Choose source, the view's one amber.
        let choose_source = gtk::Button::with_label(&t::player_choose_source());
        choose_source.add_css_class("pill");
        choose_source.add_css_class("suggested-action");
        choose_source.set_visible(false);
        let failed_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        failed_buttons.set_halign(gtk::Align::Start);
        failed_buttons.set_margin_top(12);
        failed_buttons.append(&choose_source);
        failed_buttons.append(&again);
        let hint = gtk::Label::new(Some(&t::linux_reader_page_failed_hint()));
        hint.add_css_class("haze");
        hint.add_css_class("caption");
        hint.set_xalign(0.0);
        hint.set_wrap(true);
        hint.set_margin_top(12);
        let failed_column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        failed_column.set_valign(gtk::Align::Center);
        failed_column.set_margin_start(34);
        failed_column.set_margin_end(34);
        failed_column.append(&heading);
        failed_column.append(&failed_reason);
        failed_column.append(&failed_buttons);
        failed_column.append(&hint);
        let failed_plate = gtk::Box::new(gtk::Orientation::Vertical, 0);
        failed_plate.add_css_class("reader-plate");
        failed_plate.set_halign(gtk::Align::Center);
        failed_plate.set_vexpand(true);
        failed_plate.append(&failed_column);
        failed_column.set_vexpand(true);

        // The page plate while a chapter opens or a page loads: the number from 600 ms, a line
        // from 4 s, and the image fades in over it.
        let wait_number = gtk::Label::new(None);
        wait_number.add_css_class("reader-plate-number");
        wait_number.add_css_class("numeric");
        wait_number.set_wrap(true);
        wait_number.set_justify(gtk::Justification::Center);
        let wait_line = gtk::Label::new(None);
        wait_line.add_css_class("reader-plate-line");
        wait_line.set_wrap(true);
        wait_line.set_justify(gtk::Justification::Center);
        wait_line.set_visible(false);
        let wait_words = gtk::Box::new(gtk::Orientation::Vertical, 6);
        wait_words.set_valign(gtk::Align::Center);
        wait_words.set_vexpand(true);
        wait_words.set_margin_start(24);
        wait_words.set_margin_end(24);
        wait_words.append(&wait_number);
        wait_words.append(&wait_line);
        let wait_choose = gtk::Button::with_label(&t::player_choose_source());
        wait_choose.add_css_class("pill");
        let wait_back = gtk::Button::with_label(&t::linux_reader_go_back());
        wait_back.add_css_class("flat");
        let wait_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        wait_buttons.set_halign(gtk::Align::Center);
        wait_buttons.append(&wait_choose);
        wait_buttons.append(&wait_back);
        let wait_long = gtk::Label::new(Some(&t::reader_wait_long()));
        wait_long.add_css_class("reader-plate-line");
        wait_long.set_wrap(true);
        wait_long.set_justify(gtk::Justification::Center);
        let wait_more = gtk::Box::new(gtk::Orientation::Vertical, 12);
        wait_more.set_margin_top(18);
        wait_more.append(&wait_long);
        wait_more.append(&wait_buttons);
        wait_more.set_visible(false);
        wait_words.append(&wait_more);
        let wait_plate = gtk::Box::new(gtk::Orientation::Vertical, 0);
        wait_plate.add_css_class("reader-plate");
        wait_plate.set_halign(gtk::Align::Center);
        wait_plate.set_vexpand(true);
        wait_plate.append(&wait_words);
        wait_plate
            .update_property(&[gtk::accessible::Property::Label(&t::linux_reader_opening())]);

        let stack = gtk::Stack::new();
        // Every change of what the reader shows goes through the veil (motion::turn), which
        // keeps the old view until the new one is drawn.
        stack.set_transition_type(gtk::StackTransitionType::None);
        stack.add_named(&paged_scroll, Some("paged"));
        stack.add_named(&strip_scroll, Some("strip"));
        stack.add_named(&status, Some("status"));
        stack.add_named(&failed_plate, Some("failed"));
        stack.add_named(&wait_plate, Some("plate"));
        stack.add_css_class("reader-ground");
        let veil = gtk::Overlay::new();
        veil.set_child(Some(&stack));

        // The header: back to the series and the series' name, with the window's buttons.
        let title = gtk::Label::new(None);
        title.add_css_class("heading");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let close = gtk::Button::from_icon_name("go-previous-symbolic");
        close.set_tooltip_text(Some(&t::reader_close()));
        close.add_css_class("reader-back");
        close.update_property(&[gtk::accessible::Property::Label(&t::reader_close())]);
        let head = adw::HeaderBar::new();
        head.add_css_class("reader-head");
        head.pack_start(&close);
        head.set_title_widget(Some(&title));
        head.set_valign(gtk::Align::Start);

        // The group: the scrubber with the place at its start and the length at its end, then
        // the series and chapter, Aa and full screen.
        let scrub_at = gtk::Label::new(None);
        let scrub_total = gtk::Label::new(None);
        for label in [&scrub_at, &scrub_total] {
            label.add_css_class("numeric");
            label.add_css_class("caption");
            label.add_css_class("haze");
            label.set_width_chars(3);
        }
        scrub_at.set_xalign(1.0);
        scrub_total.set_xalign(0.0);
        let scrub = gtk::Scale::with_range(gtk::Orientation::Horizontal, 1.0, 2.0, 1.0);
        scrub.set_draw_value(false);
        scrub.set_round_digits(0);
        scrub.set_hexpand(true);
        scrub.add_css_class("reader-scrub");
        let scrub_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        scrub_row.add_css_class("reader-scrub-row");
        scrub_row.append(&scrub_at);
        scrub_row.append(&scrub);
        scrub_row.append(&scrub_total);
        // Back to page: the place left by a jump, above the scrubber, only while offered.
        let back_to_label = gtk::Label::new(None);
        back_to_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let back_to_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        back_to_row.append(&gtk::Image::from_icon_name("edit-undo-symbolic"));
        back_to_row.append(&back_to_label);
        let back_to = gtk::Button::new();
        back_to.set_child(Some(&back_to_row));
        back_to.add_css_class("reader-back-to");
        back_to.set_halign(gtk::Align::Start);
        back_to.set_margin_start(8);
        back_to.set_visible(false);

        let group_title = gtk::Label::new(None);
        group_title.add_css_class("reader-group-title");
        let line = gtk::Label::new(None);
        line.add_css_class("reader-group-line");
        for label in [&group_title, &line] {
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(1);
            label.set_hexpand(true);
        }
        let names = gtk::Box::new(gtk::Orientation::Vertical, 0);
        names.set_valign(gtk::Align::Center);
        names.append(&group_title);
        names.append(&line);
        let sheet = gtk::Popover::new();
        sheet.add_css_class("reader-sheet");
        sheet.set_has_arrow(false);
        // A menu button sets its popover's side from its direction, so Up is what opens it above.
        let settings = gtk::MenuButton::builder()
            .icon_name("kaeri-text-look-symbolic")
            .popover(&sheet)
            .direction(gtk::ArrowType::Up)
            .build();
        settings.set_tooltip_text(Some(&t::linux_reader_settings_open()));
        settings.update_property(&[gtk::accessible::Property::Label(
            &t::linux_reader_settings_open(),
        )]);
        let full = gtk::Button::from_icon_name("view-fullscreen-symbolic");
        full.set_tooltip_text(Some(&t::player_fullscreen()));
        full.update_property(&[gtk::accessible::Property::Label(&t::player_fullscreen())]);
        let ends = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        ends.append(&settings);
        ends.append(&full);
        let chapters = Chapters::new();
        chapters.popover.set_has_arrow(false);
        let chapters_button = gtk::MenuButton::builder()
            .icon_name("kaeri-chapters-symbolic")
            .popover(&chapters.popover)
            .direction(gtk::ArrowType::Up)
            .build();
        chapters_button.set_tooltip_text(Some(&t::reader_chapters()));
        chapters_button
            .update_property(&[gtk::accessible::Property::Label(&t::reader_chapters())]);
        // The names sit in the middle of the group, whatever is at its ends.
        let row = gtk::CenterBox::new();
        row.set_start_widget(Some(&chapters_button));
        row.set_center_widget(Some(&names));
        row.set_end_widget(Some(&ends));
        let group = gtk::Box::new(gtk::Orientation::Vertical, 2);
        group.add_css_class("reader-group");
        group.append(&back_to);
        group.append(&scrub_row);
        group.append(&row);
        group.set_halign(gtk::Align::Center);
        group.set_valign(gtk::Align::End);
        group.set_margin_bottom(16);
        group.set_width_request(GROUP_WIDTH);

        // Go to page: a field over the scrubber.
        let go_to_field = gtk::SpinButton::with_range(1.0, 2.0, 1.0);
        go_to_field.set_numeric(true);
        go_to_field.set_width_chars(5);
        go_to_field.update_property(&[gtk::accessible::Property::Label(
            &t::linux_reader_go_to_page(),
        )]);
        let go_to_title = gtk::Label::new(Some(&t::linux_reader_go_to_page()));
        go_to_title.add_css_class("heading");
        let go_to_column = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        go_to_column.set_margin_top(10);
        go_to_column.set_margin_bottom(10);
        go_to_column.set_margin_start(12);
        go_to_column.set_margin_end(12);
        go_to_column.append(&go_to_title);
        go_to_column.append(&go_to_field);
        let go_to = gtk::Popover::new();
        go_to.add_css_class("reader-sheet");
        go_to.set_has_arrow(false);
        go_to.set_position(gtk::PositionType::Top);
        go_to.set_child(Some(&go_to_column));
        go_to.set_parent(&scrub);

        // A notice: its line and, when it has one, its button, on Raised at the top.
        let notice_text = gtk::Label::new(None);
        notice_text.set_wrap(true);
        let notice_button = gtk::Button::new();
        notice_button.add_css_class("flat");
        notice_button.add_css_class("reader-notice-action");
        let notice = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        notice.add_css_class("reader-notice");
        notice.append(&notice_text);
        notice.append(&notice_button);
        notice.set_halign(gtk::Align::Center);
        notice.set_valign(gtk::Align::Start);
        notice.set_margin_top(60);
        notice.set_visible(false);

        // The credit line: the demo credit's capsule, top trailing, the whole pill the button.
        let credit_line = gtk::Button::new();
        credit_line.add_css_class("reader-credit-line");
        credit_line.set_cursor_from_name(Some("pointer"));
        credit_line.set_halign(gtk::Align::End);
        credit_line.set_valign(gtk::Align::Start);
        credit_line.set_margin_end(16);
        credit_line.set_margin_top(credits::LINE_TOP);
        credit_line.set_visible(false);
        credit_line.update_property(&[gtk::accessible::Property::Description(
            &t::reader_credit_show_hint(),
        )]);
        // The part preview over the scrubber's thumb: a label over the reader, placed above the
        // thumb when it shows.
        let part_tip = gtk::Label::new(None);
        part_tip.add_css_class("reader-part-tip");
        part_tip.add_css_class("numeric");
        part_tip.set_halign(gtk::Align::Start);
        part_tip.set_valign(gtk::Align::Start);
        part_tip.set_can_target(false);
        part_tip.set_visible(false);
        let toasts = adw::ToastOverlay::new();
        toasts.add_css_class("reader-toasts");
        toasts.set_child(Some(&veil));

        let widget = gtk::Overlay::new();
        widget.set_child(Some(&toasts));
        widget.add_overlay(&head);
        widget.add_overlay(&group);
        widget.add_overlay(&notice);
        widget.add_overlay(&credit_line);
        widget.add_overlay(&part_tip);
        widget.set_focusable(true);
        widget.add_css_class("reader");
        let this = Rc::new_cyclic(|weak: &std::rc::Weak<Self>| {
            let go_on: Rc<dyn Fn()> = Rc::new({
                let weak = weak.clone();
                move || {
                    if let Some(this) = weak.upgrade() {
                        this.continue_on();
                    }
                }
            });
            let back: Rc<dyn Fn()> = Rc::new({
                let weak = weak.clone();
                move || {
                    if let Some(this) = weak.upgrade() {
                        this.close();
                    }
                }
            });
            let skip: Rc<dyn Fn()> = Rc::new({
                let weak = weak.clone();
                move || {
                    if let Some(this) = weak.upgrade() {
                        this.skip_extras();
                    }
                }
            });
            let end = end::EndPage::new(go_on, skip, back);
            end.credit_show.connect_clicked({
                let weak = weak.clone();
                move |_| {
                    if let Some(this) = weak.upgrade() {
                        this.show_credits(true);
                    }
                }
            });
            stack.add_named(&end.scroll, Some("end"));
            Self {
                widget,
                me: weak.clone(),
                ctx: Rc::downgrade(ctx),
                show,
                state: RefCell::default(),
                veil,
                stack,
                picture,
                paged_scroll,
                strip,
                strip_foot,
                trail,
                strip_scroll,
                strip_scroll_ask: Rc::default(),
                strip_slide: RefCell::default(),
                status,
                end,
                failed_plate,
                failed_title,
                failed_reason,
                failed_page_no: Cell::new(0),
                failed_kind: Cell::default(),
                failed_again: again.clone(),
                failed_source: choose_source.clone(),
                failed_hint: hint.clone(),
                wait_plate,
                wait_number,
                wait_line,
                wait_more,
                wait_ticker: RefCell::default(),
                waiting: Cell::new(None),
                asked_open: RefCell::default(),
                told: Cell::new(None),
                opener: Rc::default(),
                notices: crate::notices::Notices::new(&toasts),
                toasts,
                credit_line,
                credit_fade: motion::Fader::always(),
                credit_timer: RefCell::default(),
                credit_toast: RefCell::default(),
                keeping: RefCell::default(),
                strip_lead,
                strip_tail,
                part_tip,
                part_tip_fade: motion::Fader::default(),
                notice,
                notice_text,
                notice_button,
                notice_action: Cell::new(None),
                notice_fade: motion::Fader::default(),
                notice_timer: RefCell::default(),
                head,
                group,
                head_fade: motion::Fader::always(),
                group_fade: motion::Fader::always(),
                group_hover: gtk::EventControllerMotion::new(),
                pinned: Cell::new(false),
                title,
                group_title,
                line,
                scrub,
                scrub_at,
                scrub_total,
                scrub_timer: RefCell::default(),
                back_to,
                back_to_label,
                back_to_page: Cell::new(None),
                back_to_fade: motion::Fader::default(),
                settings,
                sheet,
                sheet_shape_now: RefCell::default(),
                sheet_stale: Cell::new(false),
                sheet_held: Cell::new(false),
                go_to,
                go_to_field,
                chapters,
                chapters_button,
                opening_line: RefCell::default(),
                loading_timer: RefCell::default(),
                loading_line_timer: RefCell::default(),
                hide_timer: RefCell::default(),
                strip_timer: RefCell::default(),
                novel: RefCell::default(),
                showing: Cell::new(false),
                full_before: FullBefore::default(),
            }
        });
        close.connect_clicked({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.close();
                }
            }
        });
        full.connect_clicked({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.toggle_fullscreen();
                }
            }
        });
        again.connect_clicked({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.retry_failed();
                }
            }
        });
        choose_source.connect_clicked({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.choose_source();
                }
            }
        });
        wait_choose.connect_clicked({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.choose_source();
                }
            }
        });
        wait_back.connect_clicked({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.close();
                }
            }
        });
        this.back_to.connect_clicked({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.go_back_to();
                }
            }
        });
        this.credit_line.connect_clicked({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.show_credits(false);
                }
            }
        });
        this.notice_button.connect_clicked({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.notice_acted();
                }
            }
        });
        this.scrub.connect_change_value({
            let this = Rc::downgrade(&this);
            move |_, _, value| {
                if let Some(this) = this.upgrade() {
                    this.scrubbed(value);
                }
                glib::Propagation::Proceed
            }
        });
        go_on_return(&this.go_to_field, {
            let this = Rc::downgrade(&this);
            move |page| {
                if let Some(this) = this.upgrade() {
                    this.go_to.popdown();
                    this.jump_to_page(page);
                    this.widget.grab_focus();
                }
            }
        });
        // The popovers open over the group, never on it.
        for (popover, button) in [
            (this.sheet.clone(), this.settings.clone().upcast::<gtk::Widget>()),
            (this.chapters.popover.clone(), this.chapters_button.clone().upcast()),
        ] {
            let this = Rc::downgrade(&this);
            popover.connect_show(move |popover| {
                let Some(this) = this.upgrade() else { return };
                // The window may have changed size since the sheet was filled.
                if popover == &this.sheet
                    && *this.sheet_shape_now.borrow() != this.sheet_shape()
                {
                    this.fill_sheet(true);
                }
                this.aim(popover, &button);
                // The group may still be coming in with the chrome, its place not laid out yet:
                // aim again once it is, so the sheet ends just over it.
                let frames = Cell::new(0);
                let this = Rc::downgrade(&this);
                let button = button.clone();
                popover.add_tick_callback(move |popover, _| {
                    let Some(this) = this.upgrade() else { return glib::ControlFlow::Break };
                    frames.set(frames.get() + 1);
                    let (min, _, _, _) = this.group.measure(gtk::Orientation::Vertical, -1);
                    if this.group.is_mapped() && this.group.height() >= min || frames.get() > 30
                    {
                        this.aim(popover, &button);
                        return glib::ControlFlow::Break;
                    }
                    glib::ControlFlow::Continue
                });
            });
        }
        this.sheet.connect_closed({
            let this = Rc::downgrade(&this);
            move |_| {
                // Closed, the sheet opens again on its first page with the rows as they are.
                if let Some(this) = this.upgrade() {
                    this.sheet_held.set(false);
                    this.fill_sheet(true);
                }
            }
        });
        this.controls();
        this.chapters.set_ask(Rc::new({
            let this = Rc::downgrade(&this);
            move |ask| {
                let Some(this) = this.upgrade() else { return };
                match ask {
                    Ask::Jump(index) => this.jump_to(index),
                    Ask::Open(chapter) => {
                        let session = this.state.borrow().session;
                        if session != 0 {
                            this.open_chapter(session, chapter);
                        }
                    }
                    Ask::Need(page) => this.need_page(page),
                }
            }
        }));
        this.chapters.popover.connect_show({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.fill_chapters();
                }
            }
        });
        listen(ctx, &this.widget, {
            let this = Rc::downgrade(&this);
            move |event| {
                let Some(this) = this.upgrade() else { return };
                match event {
                    Event::Reader(event) => this.take(event),
                    Event::Reading(ReadingEvent::Failed {
                        about: kaeri_model::reading::ReadingAbout::Reader,
                        failure,
                    }) if this.asked_open.borrow().is_some() && this.widget.is_mapped() => {
                        this.open_failed(failure);
                    }
                    Event::Reading(ReadingEvent::Chapters { work, volumes, .. })
                        if this.state.borrow().work.as_ref() == Some(work) =>
                    {
                        this.chapters.set_volumes(volumes.clone());
                    }
                    _ => {}
                }
            }
        });
        this
    }

    /// Point `popover` at a line just over the group, from `button` in it, so it opens above
    /// the group and leaves the scrubber and names in sight.
    fn aim(&self, popover: &gtk::Popover, button: &gtk::Widget) {
        // The group's drawn top, padding included, over the button's own origin.
        let Some(group) = self.group.compute_bounds(button) else {
            return;
        };
        let y = group.y().round() as i32 - 8;
        // Over the button, but slid along so all of it stays in the window: popovers are kept on
        // the screen, not in the window, and a narrow window would cut a centred one off.
        let (x, width) = match button.compute_bounds(&self.widget) {
            Some(at) => {
                let (_, natural, _, _) = popover.measure(gtk::Orientation::Horizontal, -1);
                let middle = popover_middle(
                    f64::from(at.x()) + f64::from(at.width()) / 2.0,
                    f64::from(natural),
                    f64::from(self.widget.width()),
                );
                ((middle - f64::from(at.x())).round() as i32, 1)
            }
            None => (0, button.width().max(1)),
        };
        let aim = gdk::Rectangle::new(x, y, width, 1);
        if popover.pointing_to() != (true, aim) {
            popover.set_pointing_to(Some(&aim));
        }
    }

    fn toggle_fullscreen(&self) {
        if let Some(ctx) = self.ctx.upgrade() {
            let window = &ctx.window;
            let now = window.is_fullscreen();
            self.full_before.toggled(now);
            window.set_fullscreened(!now);
        }
    }

    fn hub(&self) -> Option<Rc<crate::hub::Hub>> {
        self.ctx.upgrade().map(|ctx| Rc::clone(&ctx.hub))
    }

    fn send(&self, command: ReaderCommand) {
        // A held keep goes first when the chapter is left, while the session still has it.
        if matches!(command, ReaderCommand::OpenChapter { .. } | ReaderCommand::Close { .. }) {
            self.commit_keep();
        }
        if let ReaderCommand::Open { surface, .. } | ReaderCommand::Surface { surface, .. } =
            &command
        {
            self.told.set(Some(*surface));
        }
        if let Some(hub) = self.hub() {
            hub.send(Command::Reader(command));
        }
    }

    /// The screen the pages are prepared for. Before the reader's first layout it has no size
    /// of its own, and it takes the size of the page it is about to cover.
    fn surface(&self) -> ReaderSurface {
        let dark = adw::StyleManager::default().is_dark();
        let (size, scale) = match surface_size(self.widget.upcast_ref()) {
            Some((width, height, scale)) => ((width, height), scale),
            None => (
                self.ctx.upgrade().map_or((1, 1), |ctx| ctx.window.default_size()),
                self.widget.scale_factor(),
            ),
        };
        let scale = scale.max(1);
        let width_px = (size.0.max(1) * scale) as u32;
        let height_px = (size.1.max(1) * scale) as u32;
        ReaderSurface {
            width_px,
            height_px,
            scale: scale as f32,
            eink: false,
            two_pages: width_px >= height_px * 13 / 10,
            dark,
        }
    }

    /// The reader's own size, after its first layout or a resize that settled, goes to the
    /// engine when it differs from the surface the engine last heard. An open prepared for the
    /// page the reader covers has the same size, so nothing is laid out again.
    fn surface_settled(&self) {
        let session = self.state.borrow().session;
        if session == 0
            || !self.widget.is_mapped()
            || self.widget.width() <= 0
            || self.widget.height() <= 0
        {
            return;
        }
        let surface = self.surface();
        if self.told.get() != Some(surface) {
            self.send(ReaderCommand::Surface { session, surface });
        }
    }

    /// Open a chapter of a work, from where it was left when `chapter` is none.
    pub fn open(self: &Rc<Self>, work: WorkId, chapter: Option<ChapterId>) {
        self.reset();
        if !self.showing.get()
            && let Some(ctx) = self.ctx.upgrade()
        {
            self.opener.note(&ctx.window, &self.widget);
        }
        self.showing.set(true);
        self.state.borrow_mut().work = Some(work.clone());
        self.asked_open.replace(Some((work.clone(), chapter.clone())));
        self.wait(Waiting::Chapter);
        (self.show)(true);
        self.widget.grab_focus();
        self.send(ReaderCommand::Open {
            work,
            at: OpenAt::Resume,
            chapter,
            surface: self.surface(),
        });
        self.reveal_chrome();
    }

    /// Drop everything the last open held: its pages and textures, the strip, the trail, the
    /// end page's preview and the novel's web view, which takes its WebKit processes with it.
    fn reset(&self) {
        self.forget_credits();
        let old = self.state.replace(State::default());
        // The demo's credit capsule leaves with the work, or the next open adds a second one.
        if let Some((_, capsule)) = old.demo {
            self.head.remove(&capsule);
        }
        while let Some(child) = self.strip.first_child() {
            self.strip.remove(&child);
        }
        self.trail.clear();
        self.picture.set_paintable(gdk::Paintable::NONE);
        if let Some(novel) = self.novel.take() {
            self.stack.remove(&novel.widget);
        }
        self.end.forget();
        self.title.set_label("");
        self.group_title.set_label("");
        self.line.set_label("");
        self.opening_line.replace(t::linux_reader_opening());
        self.hide_notice();
        self.offer_back_to(None);
        self.show_pages(0, 0);
        self.chapters.clear();
    }

    /// Set a reader setting for the shots, on the medium's own value. A choice or `on`/`off`.
    #[cfg(feature = "fixtures")]
    pub fn shot_setting(&self, key: ReaderSettingKey, name: &str) {
        let medium = match key {
            ReaderSettingKey::Look
            | ReaderSettingKey::Font
            | ReaderSettingKey::TextSize
            | ReaderSettingKey::LineSpacing
            | ReaderSettingKey::Margins
            | ReaderSettingKey::Justify => Medium::Novel,
            _ => Medium::Manga,
        };
        self.shot_setting_for(medium, key, name);
    }

    /// A reader setting for the shots on `medium`'s value: a comic's look, say.
    #[cfg(feature = "fixtures")]
    pub fn shot_setting_for(&self, medium: Medium, key: ReaderSettingKey, name: &str) {
        let value = match name {
            "on" => ReaderValue::Toggle { on: true },
            "off" => ReaderValue::Toggle { on: false },
            other => ReaderValue::Choice { name: other.to_owned() },
        };
        self.send(ReaderCommand::SetSetting {
            session: None,
            medium,
            work: None,
            key,
            scope: key.home_scope(),
            value,
        });
    }

    /// Turn to the end page for a shot: the last leaf of a paged chapter, a strip's foot.
    #[cfg(feature = "fixtures")]
    pub fn shot_end(self: &Rc<Self>) {
        if self.flows() {
            // A strip's end is at its foot: scrolled to the bottom, as reading there would.
            self.release_strip();
            let adj = self.strip_scroll.vadjustment();
            adj.set_value(adj.upper() - adj.page_size());
            return;
        }
        let last = self.state.borrow().leaves.len().saturating_sub(1);
        self.show_leaf(last, TurnHow::Jump);
    }

    /// Scroll a strip so the marker into the next chapter has its top `share` of the window
    /// down: past the middle it stays the marker, above it the next chapter runs on under it.
    #[cfg(feature = "fixtures")]
    pub fn shot_marker(self: &Rc<Self>, share: f64) {
        // Let go of the opening place without reading it: a chapter kept at its foot would run
        // on before the scroll lands.
        self.state.borrow_mut().strip_anchor = None;
        let (heights, spacing) = self.strip_heights();
        let foot = strip_target(&heights, spacing, heights.len(), None) - f64::from(spacing);
        let page = self.strip_scroll.vadjustment().page_size();
        self.scroll_strip_to(self.strip_base() + foot + self.tail_height() - page * share);
    }

    /// "Resumed at page 2 · Start over" held over a novel, with the chrome up or gone, where the
    /// novel's text can't be drawn (the shots box has no WebKit): only where the notice sits.
    #[cfg(feature = "fixtures")]
    pub fn shot_notice(self: &Rc<Self>, chrome: bool) {
        use kaeri_model::{Msg, ReadingMsg};
        self.state.borrow_mut().medium = Some(Medium::Novel);
        self.pinned.set(chrome);
        if chrome {
            self.reveal_chrome();
        } else {
            self.hide_chrome();
        }
        self.notice(&ReaderNotice {
            msg: Msg::Reading(ReadingMsg::ReaderResumed { page: 2 }),
            action: Some(ReaderAction::StartOver),
            lasts_ms: 0,
        });
    }

    /// The end page as it is with no first page of the next chapter: the plate with its number.
    #[cfg(feature = "fixtures")]
    pub fn shot_end_plate(self: &Rc<Self>) {
        self.state.borrow_mut().preview = None;
        self.shot_end();
        self.fill_end();
    }

    /// The end of chapter 16 with the next chapter an extra, 16.5, and chapter 17 after it to
    /// skip to. The extra's first page is one of the open chapter's pages.
    #[cfg(feature = "fixtures")]
    pub fn shot_end_skip(self: &Rc<Self>) {
        let first = self.texture_for(0);
        {
            let mut state = self.state.borrow_mut();
            if let Some(chapter) = state.chapter.as_mut() {
                chapter.number = Some("16".into());
                chapter.label = "Chapter 16".into(); // i18n-skip: a shot's chapter label
            }
            let staged = state.next.as_mut().map(|next| {
                next.number = Some("16.5".into());
                next.label = "Chapter 16.5".into(); // i18n-skip: a shot's chapter label
                next.title = Some("Volume 1 extras".into()); // i18n-skip: a shot's chapter title
                let mut skip = next.clone();
                skip.id = ChapterId("shot-skip-17".into());
                skip.number = Some("17".into());
                skip.label = "Chapter 17".into(); // i18n-skip: a shot's chapter label
                skip.title = None;
                (next.id.clone(), skip)
            });
            if let Some((next, skip)) = staged {
                state.skip_to = Some(skip);
                state.extras = 1;
                if let Some(first) = first {
                    state.preview = Some((next, first));
                }
            }
        }
        self.fill_end();
    }

    /// Every strip image as if its file were still coming: its plate with its number, and with
    /// `line` the line under it as from 4 s.
    #[cfg(feature = "fixtures")]
    pub fn shot_strip_plate(&self, line: bool) {
        for (picture, _) in &self.state.borrow().strip_frames {
            picture.set_paintable(gdk::Paintable::NONE);
            let Some(plate) = strip_plate(picture) else { continue };
            plate.set_opacity(1.0);
            plate.set_visible(true);
            if let Some(words) = plate.first_child() {
                words.set_opacity(1.0);
                if let Some(said) = plate_line(&words) {
                    said.set_opacity(1.0);
                    said.set_visible(line);
                }
            }
        }
        self.place_plate_words();
    }

    /// The page plate of page 23, still loading, with its line.
    #[cfg(feature = "fixtures")]
    pub fn shot_plate(self: &Rc<Self>) {
        self.stop_wait();
        self.show_plate(Waiting::Page(22));
        self.wait_line.set_label(&t::reader_page_loading(23));
        self.wait_line.set_visible(true);
    }

    /// Aa open on Reading mode's list.
    #[cfg(feature = "fixtures")]
    pub fn shot_mode_list(self: &Rc<Self>) {
        self.pad(kaeri_model::input::ReaderPad::Settings);
        if let Some(nav) = self.sheet.child().and_downcast::<adw::NavigationView>() {
            nav.push_by_tag("mode");
        }
    }

    /// Leave the reader; the engine saves the place as the session closes.
    pub fn close(&self) {
        let session = self.state.borrow().session;
        if session != 0 {
            self.send(ReaderCommand::Close { session });
        }
        self.stop_wait();
        self.hide_notice();
        self.quiet_credits();
        self.widget.set_cursor(None);
        let was_showing = self.showing.get();
        // Full screen is the reader's alone: the rest of the window has no way out of it.
        if let Some(ctx) = self.ctx.upgrade()
            && let Some(full) = self.full_before.give_back(ctx.window.is_fullscreen())
        {
            ctx.window.set_fullscreened(full);
        }
        (self.show)(false);
        if was_showing && let Some(ctx) = self.ctx.upgrade() {
            self.opener.give_back(&ctx.window, &self.widget);
        }
        // A local: an if-let scrutinee's borrow would live through the action's handler.
        let work = self.state.borrow().work.clone();
        if let Some(ctx) = self.ctx.upgrade()
            && let Some(work) = work
        {
            let _ = WidgetExt::activate_action(
                &ctx.window,
                "win.reader-closed",
                Some(&work.0.to_variant()),
            );
        }
        self.showing.set(false);
        // The page stays drawn while the window crossfades away from it, and goes as the
        // reader is unmapped at the end of that (see `controls`).
        if !self.widget.is_mapped() {
            self.reset();
        }
    }

    /// Alt+Left: back to the page before a jump while that is offered, else out of the reader.
    pub fn back(&self) {
        if !self.go_back_to() {
            self.close();
        }
    }

    /// Back to the page a jump left, while the engine offers it. False when it does not.
    fn go_back_to(&self) -> bool {
        let session = self.state.borrow().session;
        if self.back_to_page.get().is_none() || session == 0 {
            return false;
        }
        self.offer_back_to(None);
        self.send(ReaderCommand::BackTo { session });
        true
    }

    /// Show Back to page `page` above the scrubber, or take it away. It shows with the chrome,
    /// as part of the group, and fades in and out on its own.
    fn offer_back_to(&self, page: Option<u32>) {
        self.back_to_page.set(page);
        if let Some(page) = page {
            let words = t::reader_back_to_page(i64::from(page));
            self.back_to_label.set_label(&words);
            self.back_to.update_property(&[gtk::accessible::Property::Label(&words)]);
        }
        self.back_to_fade.set(&self.back_to, page.is_some());
    }

    /// Hold the place of what is on its way: nothing for 600 ms, then the page plate with the
    /// chapter or the page's number, and for a page a line from 4 s. Whatever comes next
    /// crossfades over it.
    fn wait(self: &Rc<Self>, what: Waiting) {
        if self.waiting.get() == Some(what) {
            return;
        }
        self.stop_wait();
        self.waiting.set(Some(what));
        let this = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(SILENT, move || {
            let Some(this) = this.upgrade() else { return };
            this.loading_timer.borrow_mut().take();
            this.show_plate(what);
        });
        *self.loading_timer.borrow_mut() = Some(id);
        if let Waiting::Page(page) = what {
            let this = Rc::downgrade(self);
            let id = glib::timeout_add_local_once(SAY_LOADING, move || {
                let Some(this) = this.upgrade() else { return };
                this.loading_line_timer.borrow_mut().take();
                this.wait_line.set_label(&t::reader_page_loading(i64::from(page + 1)));
                this.wait_line.set_visible(true);
                motion::fade_in(&this.wait_line);
            });
            *self.loading_line_timer.borrow_mut() = Some(id);
        }
    }

    fn show_plate(self: &Rc<Self>, what: Waiting) {
        let number = match what {
            Waiting::Chapter => self.opening_line.borrow().clone(),
            Waiting::Page(page) => t::reader_page_plate(i64::from(page + 1)),
        };
        if what == Waiting::Chapter {
            self.count_wait();
        }
        self.wait_number.set_label(&number);
        self.wait_line.set_visible(false);
        self.wait_plate.update_property(&[gtk::accessible::Property::Label(&number)]);
        self.page_shape(&self.wait_plate);
        motion::turn(&self.veil, &self.stack, || self.stack.set_visible_child_name("plate"));
    }

    /// Give `plate` a page's shape in the window.
    fn page_shape(&self, plate: &gtk::Box) {
        let height = f64::from(self.widget.height().max(1));
        let width = self.widget.width().max(1);
        plate.set_size_request(((height * 0.7).round() as i32).min(width), -1);
    }

    /// From the plate's 600 ms: the stage with its seconds from 3 s, as the player's start counts,
    /// and from [`LONG_WAIT`] another source and Go back under it.
    fn count_wait(self: &Rc<Self>) {
        let this = Rc::downgrade(self);
        let mut waited = SILENT;
        let id = glib::timeout_add_local(Duration::from_secs(1), move || {
            let Some(this) = this.upgrade() else { return glib::ControlFlow::Break };
            waited += Duration::from_secs(1);
            let line = this.opening_line.borrow().clone();
            if waited >= Duration::from_secs(3) && !line.is_empty() {
                let seconds = i64::try_from(waited.as_secs()).unwrap_or(i64::MAX);
                this.wait_number.set_label(&t::reader_wait_seconds(&line, seconds));
            }
            if waited >= LONG_WAIT && !this.wait_more.is_visible() {
                this.wait_more.set_visible(true);
                motion::fade_in(&this.wait_more);
            }
            glib::ControlFlow::Continue
        });
        if let Some(old) = self.wait_ticker.replace(Some(id)) {
            old.remove();
        }
    }

    fn stop_wait(&self) {
        self.waiting.set(None);
        self.wait_more.set_visible(false);
        for timer in [&self.loading_timer, &self.loading_line_timer, &self.wait_ticker] {
            if let Some(id) = timer.borrow_mut().take() {
                id.remove();
            }
        }
    }

    /// The chapter could not open: why, on the page plate, with Choose source and Try again.
    fn open_failed(self: &Rc<Self>, failure: &ReadingFailure) {
        self.stop_wait();
        self.failed_kind.set(Failed::Chapter);
        let number = self.state.borrow().opening_number.clone();
        self.failed_title.set_label(
            &number.map_or_else(t::linux_reader_open_failed_plain, |n| {
                t::linux_reader_open_failed(&n)
            }),
        );
        let reason = super::failure(failure);
        self.failed_reason.set_label(&reason);
        self.failed_reason.set_visible(!reason.is_empty());
        self.failed_source.set_visible(true);
        self.failed_again.remove_css_class("suggested-action");
        self.failed_hint.set_visible(false);
        self.page_shape(&self.failed_plate);
        motion::turn(&self.veil, &self.stack, || self.stack.set_visible_child_name("failed"));
        self.reveal_chrome();
    }

    /// The series' sources, from a chapter that could not open; a pick opens it again.
    fn choose_source(self: &Rc<Self>) {
        let Some(ctx) = self.ctx.upgrade() else { return };
        let Some((work, _)) = self.asked_open.borrow().clone() else { return };
        let this = Rc::downgrade(self);
        super::series_extra::read_from_then(
            &ctx,
            &work,
            &self.title.label(),
            Some(Rc::new(move || {
                if let Some(this) = this.upgrade() {
                    this.open_again();
                }
            })),
        );
    }

    /// Open the last chapter asked for again, after it failed.
    fn open_again(self: &Rc<Self>) {
        // A local, so the borrow ends before open() replaces asked_open.
        let asked = self.asked_open.borrow().clone();
        if let Some((work, chapter)) = asked {
            self.open(work, chapter);
        }
    }

    fn take(self: &Rc<Self>, event: &ReaderEvent) {
        match event {
            ReaderEvent::Opened(view) => self.opened(view),
            ReaderEvent::Leaves { session, chapter, leaves } if self.mine(*session) => {
                let is_current =
                    self.state.borrow().chapter.as_ref().is_some_and(|c| &c.id == chapter);
                if is_current {
                    self.leaves(leaves.clone());
                }
            }
            ReaderEvent::PageReady { session, chapter, page, file, .. }
                if self.mine(*session) && self.is_open(chapter) =>
            {
                self.page_ready(*page, file.clone());
            }
            ReaderEvent::PageFailed { session, page, failure, .. } if self.mine(*session) => {
                {
                    let mut state = self.state.borrow_mut();
                    state.failed.insert(*page);
                    state.failures.insert(*page, failure.clone());
                }
                if self.flows() {
                    self.fail_strip_page(*page);
                } else if self.is_current_page(*page) {
                    self.failed_page(*page, true);
                }
            }
            ReaderEvent::EndPage { session, next, why_none, next_source, skip_to, extras }
                if self.mine(*session) =>
            {
                {
                    let mut state = self.state.borrow_mut();
                    state.next = next.clone();
                    state.why_none = *why_none;
                    state.next_source = next_source.clone();
                    state.skip_to = skip_to.clone();
                    state.extras = *extras;
                }
                self.fill_end();
            }
            ReaderEvent::ChapterRead { work, chapter, trackers }
                if self.state.borrow().work.as_ref() == Some(work)
                    && self
                        .state
                        .borrow()
                        .chapter
                        .as_ref()
                        .is_some_and(|c| &c.id == chapter) =>
            {
                self.state.borrow_mut().read = Some(trackers.clone());
                self.fill_end();
            }
            ReaderEvent::NextPreview { session, chapter, file } if self.mine(*session) => {
                if let Some(texture) = load_texture_cropped(file) {
                    self.state.borrow_mut().preview = Some((chapter.clone(), texture));
                    self.fill_end();
                }
            }
            ReaderEvent::NovelOpen(open) if self.mine(open.session) => {
                self.open_novel(open);
            }
            ReaderEvent::Settings(view) => self.settings_view(view.clone()),
            ReaderEvent::GoTo { session, place } if self.mine(*session) => {
                let novel = self.novel.borrow().clone();
                if let (Some(pos), Some(novel)) = (&place.novel, novel) {
                    novel.go_to_pos(pos);
                } else if let Some(at) = place.strip.filter(|_| self.flows()) {
                    self.state.borrow_mut().strip_anchor = Some(at);
                    self.hold_strip_place();
                } else if let Some(page) = place.page {
                    self.go_to_page(page);
                }
            }
            ReaderEvent::Closed { session } if *session == self.state.borrow().session => {
                self.stop_wait();
            }
            ReaderEvent::Opening { chapter, wait } => {
                let line = match (wait, chapter) {
                    (Some(ReaderWait::ChapterList { source }), _) => {
                        t::reader_wait_chapter_list(source)
                    }
                    (Some(ReaderWait::ChapterPages { chapter, source }), _) => {
                        t::reader_wait_chapter_pages(chapter, source)
                    }
                    (_, Some(chapter)) => match &chapter.number {
                        Some(number) => t::linux_reader_opening_number(number),
                        None => t::linux_reader_opening(),
                    },
                    (_, None) => t::linux_reader_opening(),
                };
                self.state.borrow_mut().opening_number = chapter
                    .as_ref()
                    .map(|c| c.number.clone().unwrap_or_else(|| c.label.clone()));
                if self.waiting.get() == Some(Waiting::Chapter)
                    && self.stack.visible_child_name().as_deref() == Some("plate")
                {
                    self.wait_number.set_label(&line);
                }
                self.opening_line.replace(line);
            }
            ReaderEvent::BackToPage { session, page } if self.mine(*session) => {
                self.offer_back_to(*page);
            }
            ReaderEvent::Notice(notice) if self.state.borrow().session != 0 => {
                self.notice(notice);
            }
            ReaderEvent::CreditPages { session, chapter, pages, .. }
                if self.mine(*session) && self.is_open(chapter) =>
            {
                self.credit_pages(pages.clone());
            }
            ReaderEvent::ChapterParts { session, chapter, parts, missing, .. }
                if self.mine(*session) && self.is_open(chapter) =>
            {
                self.chapter_parts(parts.clone(), missing.clone());
            }
            _ => {}
        }
        if let ReaderEvent::NovelStyle { session, style, .. } = event
            && self.mine(*session)
            && let Some(novel) = self.novel.borrow().clone()
        {
            novel.style(style);
        }
    }

    fn mine(&self, session: u64) -> bool {
        session != 0 && self.showing.get() && session == self.state.borrow().session
    }

    /// The chapter on screen, not one a strip ran on from or one still to open.
    fn is_open(&self, chapter: &ChapterId) -> bool {
        self.state.borrow().chapter.as_ref().is_some_and(|c| &c.id == chapter)
    }

    fn flows(&self) -> bool {
        self.state.borrow().mode.is_some_and(ReaderMode::flows)
    }

    fn opened(self: &Rc<Self>, view: &ReaderView) {
        // Left before the chapter opened: the reader had no session to close then, so the
        // session that opened late is closed now, and nothing is built in a hidden reader.
        if !self.showing.get() {
            self.send(ReaderCommand::Close { session: view.session });
            return;
        }
        // The series' chapters for Chapters, with their states as they are now.
        if let Some(ctx) = self.ctx.upgrade() {
            super::send(
                &ctx.hub,
                ReadingCommand::LoadChapters { work: view.work.clone(), edition: None },
            );
        }
        // A strip that ran on keeps the chapters above; any other open starts clean.
        let flowed = view.mode == ReaderMode::Strip
            && self.state.borrow().flowing.as_ref() == Some(&view.chapter.id);
        if !flowed {
            self.trail.clear();
        }
        {
            let mut state = self.state.borrow_mut();
            state.flowing = None;
            state.flowed = flowed;
            state.session = view.session;
            state.work = Some(view.work.clone());
            state.medium = Some(view.medium);
            state.chapter = Some(view.chapter.clone());
            state.next = view.next.clone();
            state.previous = view.previous.clone();
            state.why_none = None;
            state.next_source = None;
            state.skip_to = None;
            state.extras = 0;
            state.read = None;
            state.preview = None;
            state.novel_end = false;
            state.mode = Some(view.mode);
            state.direction = Some(view.direction);
            state.start_page = view.start.page;
            state.start_strip = view.start.strip;
            state.strip_anchor = None;
            state.settings = view.settings.clone();
            state.leaves.clear();
            state.files.clear();
            state.textures.clear();
            state.failed.clear();
            state.failures.clear();
            state.asked.clear();
            state.strip_frames.clear();
            state.index = 0;
            state.held_page = None;
        }
        // The open went out before the reader's first layout; its own size may be known now.
        self.surface_settled();
        self.forget_credits();
        self.demo_credit(view.demo_credit.as_ref());
        self.title.set_label(&view.title);
        self.group_title.set_label(&view.title);
        self.line.set_label(&view.chapter.label);
        self.scrub_runs(view.medium, view.mode, view.direction);
        self.fill_sheet(true);
        self.apply_look();
        self.fill_end();
        if view.medium == Medium::Novel {
            // The novel's own event follows; the web view takes the stack then.
            return;
        }
        let name = if view.mode.flows() { "strip" } else { "paged" };
        if self.stack.visible_child_name().as_deref() != Some(name) {
            motion::turn(&self.veil, &self.stack, || self.stack.set_visible_child_name(name));
        }
    }

    /// The scrubber runs with the book: right to left fills from the right, with the count
    /// at the right and the total at the left, as the board draws it. A strip runs top to
    /// bottom and fills from the left. The scale keeps its own direction and is inverted.
    fn scrub_runs(&self, medium: Medium, mode: ReaderMode, direction: ReadDirection) {
        let rtl = direction == ReadDirection::RightToLeft
            && (medium == Medium::Novel || !mode.flows());
        if let Some(row) = self.scrub.parent() {
            row.set_direction(if rtl {
                gtk::TextDirection::Rtl
            } else {
                gtk::TextDirection::Ltr
            });
        }
        self.scrub.set_direction(gtk::TextDirection::Ltr);
        self.scrub.set_inverted(rtl);
    }

    /// A mode or direction picked in the reader: the engine lays the chapter out again
    /// without opening it, and the page on screen stays on screen in the other mode.
    fn follow_settings(self: &Rc<Self>, view: &ReaderSettingsView) {
        let (medium, now) = {
            let state = self.state.borrow();
            let (Some(medium), Some(mode), Some(direction)) =
                (state.medium, state.mode, state.direction)
            else {
                return;
            };
            if state.session == 0 {
                return;
            }
            let Some(now) = followed(medium, state.work.as_ref(), (mode, direction), view)
            else {
                return;
            };
            (medium, now)
        };
        let (mode, direction) = now;
        let was = self.state.borrow().mode;
        if was != Some(mode) {
            // The page on screen: the first of the leaf, or the strip's image at the top.
            let page = if self.flows() {
                let image = self.strip_place().map_or(0, |p| p.image as usize);
                let state = self.state.borrow();
                state
                    .leaves
                    .iter()
                    .filter(|l| !l.end_page)
                    .nth(image)
                    .and_then(|l| l.parts.first())
                    .map(|p| p.page)
            } else {
                let state = self.state.borrow();
                let leaf =
                    state.leaves.get(state.index).filter(|l| !l.end_page).or_else(|| {
                        // On the end page: the chapter's last page.
                        state.leaves.iter().rev().find(|l| !l.end_page)
                    });
                page_kept(leaf, state.held_page)
            };
            let mut state = self.state.borrow_mut();
            state.start_page = page.or(state.start_page);
            state.start_strip = None;
            state.strip_anchor = None;
            state.flowed = false;
            state.held_page = None;
            state.index = 0;
            // The next layout is a first one, in the new mode.
            state.leaves.clear();
            state.switched = true;
        }
        {
            let mut state = self.state.borrow_mut();
            state.mode = Some(mode);
            state.direction = Some(direction);
        }
        self.scrub_runs(medium, mode, direction);
    }

    // ---------------------------------------------------------------------------------------
    // Notices

    /// A line over the page for a while, with its button when it has one.
    fn notice(self: &Rc<Self>, notice: &ReaderNotice) {
        let text = say::msg(&notice.msg);
        if text.is_empty() {
            return;
        }
        let words = match notice.action {
            Some(ReaderAction::StartOver) => Some(t::linux_reader_start_over()),
            Some(ReaderAction::GoBack) => Some(t::linux_reader_go_back()),
            Some(ReaderAction::ChangeMode) => Some(t::linux_reader_change()),
            None => None,
        };
        self.notice_action.set(notice.action);
        self.place_notice(self.head_fade.shown());
        self.notice_text.set_label(&text);
        self.notice_text.set_visible(true);
        self.notice_button.set_visible(words.is_some());
        if let Some(words) = &words {
            self.notice_button.set_label(words);
        }
        self.notice_fade.set(&self.notice, true);
        if let Some(id) = self.notice_timer.borrow_mut().take() {
            id.remove();
        }
        // The engine's rule: longer for long text, and until it is dismissed when it carries a
        // button and a screen reader runs.
        let words = text.split_whitespace().count() as u32;
        let hold = kaeri_model::reader::reader_notice_hold(
            notice.lasts_ms,
            words,
            notice.action.is_some(),
            crate::window::screen_reader_running(),
        );
        if let Some(hold) = hold.filter(|ms| *ms > 0) {
            let this = Rc::downgrade(self);
            let id = glib::timeout_add_local_once(
                Duration::from_millis(u64::from(hold)),
                move || {
                    let Some(this) = this.upgrade() else { return };
                    this.notice_timer.borrow_mut().take();
                    this.notice_action.set(None);
                    this.notice_fade.set(&this.notice, false);
                },
            );
            *self.notice_timer.borrow_mut() = Some(id);
        }
    }

    fn hide_notice(&self) {
        if let Some(id) = self.notice_timer.borrow_mut().take() {
            id.remove();
        }
        self.notice_action.set(None);
        self.notice_fade.hide_now(&self.notice);
    }

    fn notice_acted(self: &Rc<Self>) {
        let session = self.state.borrow().session;
        let Some(action) = self.notice_action.get() else { return };
        match action {
            ReaderAction::StartOver => self.send(ReaderCommand::StartOver { session }),
            ReaderAction::GoBack => self.send(ReaderCommand::GoBack { session }),
            ReaderAction::ChangeMode => {
                self.reveal_chrome();
                self.settings.popup();
            }
        }
        self.notice_action.set(None);
        self.notice_fade.set(&self.notice, false);
    }

    // ---------------------------------------------------------------------------------------
    // Pages

    fn leaves(self: &Rc<Self>, leaves: Vec<Leaf>) {
        let flows = self.flows();
        let first = self.state.borrow().leaves.is_empty();
        let switched = std::mem::take(&mut self.state.borrow_mut().switched);
        // The credit pages and parts that follow a layout are the engine's word on it.
        let on_credit = self.relaid();
        let relay = self.relay_kept();
        // Pages whose file or crop changed, as after crop borders changed: redrawn in place.
        let mut redrawn = Vec::new();
        {
            let mut state = self.state.borrow_mut();
            for leaf in &leaves {
                for part in &leaf.parts {
                    if let Some(file) = &part.file {
                        let old = state.files.insert(part.page, file.clone());
                        if let Some(old) = old
                            && (old.path != file.path || old.crop != file.crop)
                        {
                            if old.path != file.path {
                                state.textures.remove(&part.page);
                            }
                            redrawn.push(part.page);
                        }
                    }
                }
            }
        }
        if flows {
            // The same images again, after a resize: the strip keeps its pictures and its place.
            let same = !first
                && relay.is_none()
                && strip_leaves(&self.state.borrow().leaves) == strip_leaves(&leaves);
            if !first && self.state.borrow().strip_anchor.is_none() {
                let here = self.strip_place();
                self.state.borrow_mut().strip_anchor = here;
            }
            if let Some(relay) = &relay {
                // The images from the kept page on are one further down the strip.
                let mut state = self.state.borrow_mut();
                if let Some(anchor) = state.strip_anchor.as_mut()
                    && anchor.image >= relay.page
                {
                    anchor.image += 1;
                }
            }
            if first && std::mem::take(&mut self.state.borrow_mut().flowed) {
                // Under the marker, where the scroll already is: nothing to hold.
                let mut state = self.state.borrow_mut();
                state.strip_anchor = None;
                state.index = 0;
            } else if first {
                let mut state = self.state.borrow_mut();
                let at = strip_opening(&leaves, state.start_strip, state.start_page);
                state.strip_anchor = Some(at);
                state.index = at.image as usize;
            }
            self.state.borrow_mut().leaves = leaves;
            if same {
                for page in redrawn {
                    self.fill_strip_page(page, true);
                }
                self.size_strip();
            } else {
                self.drop_strip_credits();
                self.build_strip();
            }
            if switched && self.stack.visible_child_name().as_deref() != Some("strip") {
                // From pages: the images in hand at the kept page are drawn first, while the
                // strip is off screen and they take no fade, so the page fades into the strip
                // rather than into its empty frames. The picture goes with the turn.
                let anchor = self.state.borrow().strip_anchor.map_or(0, |a| a.image as usize);
                for index in anchor..anchor + 3 {
                    self.want_strip(index);
                }
                motion::turn(&self.veil, &self.stack, || {
                    self.stack.set_visible_child_name("strip");
                    self.picture.set_paintable(gdk::Paintable::NONE);
                });
            }
            return;
        }
        if let Some(relay) = &relay
            && on_credit.is_some_and(|(print, _)| print == relay.fingerprint)
        {
            // The kept page was on screen: it stays, now one of the chapter's pages.
            {
                let mut state = self.state.borrow_mut();
                state.index = leaves
                    .iter()
                    .position(|l| l.parts.iter().any(|p| p.page == relay.page))
                    .unwrap_or(0);
                state.leaves = leaves;
            }
            let index = self.state.borrow().index;
            self.show_leaf(index, TurnHow::Placed);
            return;
        }
        if let Some((print, at_end)) = on_credit.filter(|_| !first) {
            // A credit page is on screen: it stays if the new layout still skips it, and
            // otherwise its own page takes its place, kept where it was.
            let mut state = self.state.borrow_mut();
            state.index =
                if at_end { leaves.iter().rposition(|l| !l.end_page).unwrap_or(0) } else { 0 };
            state.leaves = leaves;
            state.credit_wanted = Some(print);
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            let start = if first {
                state.start_page
            } else {
                // A new layout keeps the page that was showing, one on from a kept page before
                // it: the page kept by the last layout while its leaf is still on screen, else
                // the leaf's first.
                page_kept(state.leaves.get(state.index), state.held_page).map(|page| {
                    page + u32::from(relay.as_ref().is_some_and(|r| r.page <= page))
                })
            };
            state.index = start
                .and_then(|p| {
                    leaves.iter().position(|l| l.parts.iter().any(|part| part.page == p))
                })
                .unwrap_or(0)
                .min(leaves.len().saturating_sub(1));
            state.held_page = if first { None } else { start };
            state.leaves = leaves;
        }
        let index = self.state.borrow().index;
        // The first layout, or a new one: the reader put the place there, not the person.
        self.show_leaf(index, TurnHow::Placed);
        if switched {
            self.leave_strip();
        }
    }

    /// From the strip to pages: once the pages are on screen the strip's images go, and the
    /// chapters it ran on from with them. A page still on its way leaves the strip up until it
    /// comes, and the strip goes with the next layout or the close.
    fn leave_strip(&self) {
        if self.stack.visible_child_name().as_deref() == Some("strip") {
            return;
        }
        while let Some(child) = self.strip.first_child() {
            self.strip.remove(&child);
        }
        self.state.borrow_mut().strip_frames.clear();
        self.trail.clear();
    }

    fn page_of(&self, index: usize) -> Option<u32> {
        self.state.borrow().leaves.get(index).and_then(|l| l.parts.first()).map(|p| p.page)
    }

    fn is_current_page(&self, page: u32) -> bool {
        let state = self.state.borrow();
        state.leaves.get(state.index).is_some_and(|l| l.parts.iter().any(|p| p.page == page))
    }

    fn texture_for(&self, page: u32) -> Option<gdk::Texture> {
        let cached = self.state.borrow().textures.get(&page).cloned();
        if cached.is_some() {
            return cached;
        }
        let file = self.state.borrow().files.get(&page).cloned()?;
        let Some(texture) = load_texture(&file) else {
            // The file is here and nothing can show it: the page failed, with Try again,
            // rather than waiting on its plate for good.
            let mut state = self.state.borrow_mut();
            state.failed.insert(page);
            state.failures.insert(
                page,
                ReadingFailure::Other { reason: t::linux_reader_page_undecodable() },
            );
            return None;
        };
        self.state.borrow_mut().textures.insert(page, texture.clone());
        Some(texture)
    }

    /// The failed page among a leaf's, if one is.
    fn failed_in(&self, index: usize) -> Option<u32> {
        let state = self.state.borrow();
        state.leaves.get(index).and_then(|l| {
            l.parts.iter().map(|p| p.page).find(|page| state.failed.contains(page))
        })
    }

    /// The leaf drawn as one picture: each part at its place, cropped to what the engine says to
    /// show, a spread turned on its side when it must be. `None` until every part has landed.
    fn compose(&self, index: usize) -> Option<gdk::Paintable> {
        let leaf = self.state.borrow().leaves.get(index).cloned()?;
        let mut drawn = Vec::new();
        for part in &leaf.parts {
            let texture = self.texture_for(part.page)?;
            let file = self.state.borrow().files.get(&part.page).cloned()?;
            let crop = file
                .crop
                .map(|c| (c.x as f32, c.y as f32, c.width as f32, c.height as f32))
                .filter(|c| c.2 > 0.0 && c.3 > 0.0)
                .unwrap_or((0.0, 0.0, texture.width() as f32, texture.height() as f32));
            drawn.push((part, texture, crop));
        }
        if drawn.is_empty() {
            return None;
        }
        // The leaf is as tall as its tallest part, and as wide as the parts side by side.
        let size = |part: &kaeri_model::reader::LeafPart, crop: (f32, f32, f32, f32)| {
            if part.rotate { (crop.3, crop.2) } else { (crop.2, crop.3) }
        };
        let height = drawn.iter().map(|(p, _, c)| size(p, *c).1).fold(1.0_f32, f32::max);
        let width: f32 = drawn
            .iter()
            .map(|(p, _, c)| {
                let (w, h) = size(p, *c);
                w * height / h.max(1.0)
            })
            .sum::<f32>()
            .max(1.0);
        let snapshot = gtk::Snapshot::new();
        for (part, texture, crop) in &drawn {
            let (bx, by) = (part.place.x * width, part.place.y * height);
            let (bw, bh) = (part.place.width * width, part.place.height * height);
            let (w, h) = size(part, *crop);
            let scale = (bw / w.max(1.0)).min(bh / h.max(1.0));
            snapshot.save();
            // Centred in its box.
            snapshot.translate(&gtk::graphene::Point::new(
                bx + (bw - w * scale) / 2.0,
                by + (bh - h * scale) / 2.0,
            ));
            snapshot.scale(scale, scale);
            if part.rotate {
                snapshot.translate(&gtk::graphene::Point::new(w, 0.0));
                snapshot.rotate(90.0);
            }
            snapshot.push_clip(&gtk::graphene::Rect::new(0.0, 0.0, crop.2, crop.3));
            snapshot.translate(&gtk::graphene::Point::new(-crop.0, -crop.1));
            texture.snapshot(
                &snapshot,
                f64::from(texture.width()),
                f64::from(texture.height()),
            );
            snapshot.pop();
            snapshot.restore();
        }
        snapshot.to_paintable(Some(&gtk::graphene::Size::new(width, height)))
    }

    fn want(&self, index: usize) {
        let (session, chapter, pages) = {
            let state = self.state.borrow();
            let Some(chapter) = state.chapter.as_ref().map(|c| c.id.clone()) else { return };
            let pages: Vec<u32> = wanted_leaves(index, state.leaves.len())
                .into_iter()
                .filter_map(|i| state.leaves.get(i))
                .flat_map(|l| l.parts.iter().map(|p| p.page))
                .filter(|p| !state.files.contains_key(p) && !state.asked.contains(p))
                .collect();
            (state.session, chapter, pages)
        };
        for page in pages {
            self.state.borrow_mut().asked.insert(page);
            self.send(ReaderCommand::NeedPage { session, chapter: chapter.clone(), page });
        }
    }

    fn page_ready(self: &Rc<Self>, page: u32, file: PageFile) {
        self.chapters.page_ready(page, &file);
        let redrawn = {
            // Another file under a page's number, as after a kept credit page: drawn afresh.
            let mut state = self.state.borrow_mut();
            let old = state
                .files
                .get(&page)
                .map(|old| (old.path != file.path, old.crop != file.crop));
            if old.is_some_and(|(path, _)| path) {
                state.textures.remove(&page);
            }
            state.files.insert(page, file);
            old.is_some_and(|(path, crop)| path || crop)
        };
        {
            let mut state = self.state.borrow_mut();
            state.failures.remove(&page);
            state.failed.remove(&page);
        }
        if self.flows() {
            self.fill_strip_page(page, redrawn);
        } else if self.is_current_page(page) {
            let index = self.state.borrow().index;
            self.show_leaf(index, TurnHow::Placed);
        }
    }

    /// The story's pages, without the end page: the scrubber's length.
    fn story_pages(&self) -> usize {
        let state = self.state.borrow();
        state
            .leaves
            .iter()
            .flat_map(|l| l.parts.iter().map(|p| p.page as usize + 1))
            .max()
            .unwrap_or(0)
    }

    /// The page a leaf starts on, from 1; the end page counts as the last page.
    fn page_number(&self, index: usize) -> usize {
        let page = self.page_of(index).map(|p| p as usize + 1);
        page.unwrap_or_else(|| self.story_pages())
    }

    /// The leaf that holds page `page`, from 1.
    fn leaf_of_page(&self, page: usize) -> usize {
        let state = self.state.borrow();
        let page = page.saturating_sub(1) as u32;
        state
            .leaves
            .iter()
            .position(|l| l.parts.iter().any(|p| p.page == page))
            .or_else(|| {
                state.leaves.iter().rposition(|l| l.parts.iter().any(|p| p.page < page))
            })
            .unwrap_or(0)
    }

    fn is_end(&self, index: usize) -> bool {
        self.state.borrow().leaves.get(index).is_some_and(|l| l.end_page)
    }

    /// The last leaf with a page on it.
    fn last_page_leaf(&self) -> usize {
        let state = self.state.borrow();
        state.leaves.iter().rposition(|l| !l.end_page).unwrap_or(0)
    }

    /// A page that failed, in its place: the heading with the glyph, the reason, Try again and
    /// the line that says turning goes on.
    fn failed_page(self: &Rc<Self>, page: u32, turn: bool) {
        self.stop_wait();
        self.failed_kind.set(Failed::Page);
        self.failed_source.set_visible(false);
        self.failed_again.add_css_class("suggested-action");
        self.failed_hint.set_visible(true);
        self.failed_page_no.set(page);
        self.failed_title.set_label(&t::reader_page_failed(i64::from(page + 1)));
        let reason = self.state.borrow().failures.get(&page).map(super::failure);
        self.failed_reason.set_label(reason.as_deref().unwrap_or(""));
        self.failed_reason.set_visible(reason.is_some_and(|r| !r.is_empty()));
        self.page_shape(&self.failed_plate);
        let show = || self.stack.set_visible_child_name("failed");
        if turn && self.stack.visible_child_name().as_deref() != Some("failed") {
            motion::turn(&self.veil, &self.stack, show);
        } else {
            show();
        }
    }

    /// The strip picture of image `page`, if the strip has one.
    fn strip_picture(&self, page: u32) -> Option<gtk::Picture> {
        let state = self.state.borrow();
        let at = state
            .leaves
            .iter()
            .filter(|l| !l.end_page)
            .position(|l| l.parts.first().is_some_and(|p| p.page == page))?;
        state.strip_frames.get(at).map(|(picture, _)| picture.clone())
    }

    /// A strip image that failed, from the engine or because nothing could decode it: its plate
    /// says so, with Try again, instead of loading for good.
    fn fail_strip_page(&self, page: u32) {
        let Some(picture) = self.strip_picture(page) else { return };
        let me = self.me.clone();
        strip_frame_failed(&picture, move || {
            if let Some(this) = me.upgrade() {
                this.retry_strip_page(page);
            }
        });
    }

    /// Try again on a strip image: its plate goes back to loading and the engine sends the
    /// page again.
    fn retry_strip_page(&self, page: u32) {
        let (session, chapter) = {
            let mut state = self.state.borrow_mut();
            state.failed.remove(&page);
            state.failures.remove(&page);
            state.textures.remove(&page);
            state.files.remove(&page);
            (state.session, state.chapter.as_ref().map(|c| c.id.clone()))
        };
        if let Some(picture) = self.strip_picture(page) {
            strip_frame_retried(&picture, page);
        }
        if let Some(chapter) = chapter {
            self.send(ReaderCommand::Retry { session, chapter, page: Some(page) });
        }
    }

    fn retry_failed(self: &Rc<Self>) {
        if self.failed_kind.get() == Failed::Chapter {
            self.open_again();
            return;
        }
        let page = self.failed_page_no.get();
        let (session, chapter) = {
            let state = self.state.borrow();
            (state.session, state.chapter.as_ref().map(|c| c.id.clone()))
        };
        if let Some(chapter) = chapter {
            {
                let mut state = self.state.borrow_mut();
                state.failed.remove(&page);
                state.failures.remove(&page);
            }
            self.send(ReaderCommand::Retry { session, chapter, page: Some(page) });
        }
    }

    fn show_leaf(self: &Rc<Self>, index: usize, how: TurnHow) {
        {
            let mut state = self.state.borrow_mut();
            if state.index != index {
                state.held_page = None;
            }
        }
        self.leave_credit();
        if self.is_end(index) {
            self.show_end(index, how);
            return;
        }
        if self.page_of(index).is_none() {
            return;
        }
        self.state.borrow_mut().index = index;
        self.want(index);
        self.show_pages(self.page_number(index), self.story_pages());
        if let Some(page) = self.failed_in(index) {
            self.failed_page(page, true);
            return;
        }
        let Some(paintable) = self.compose(index) else {
            // A part that came in and couldn't be decoded failed as it was composed.
            if let Some(page) = self.failed_in(index) {
                self.failed_page(page, true);
            } else if let Some(page) = self.page_of(index) {
                self.wait(Waiting::Page(page));
            }
            return;
        };
        self.stop_wait();
        // The new page is drawn under the old one, which fades away on the quick token.
        motion::turn(&self.veil, &self.stack, || {
            self.picture.set_paintable(Some(&paintable));
            self.apply_fit();
            self.stack.set_visible_child_name("paged");
        });
        self.scroll_to_start();
        self.shown(index, how);
    }

    /// Tell the engine a leaf was painted.
    fn shown(&self, index: usize, how: TurnHow) {
        // The demo's credit capsule shows on its first page only.
        let capsule = self.state.borrow().demo.as_ref().map(|(_, capsule)| capsule.clone());
        if let Some(capsule) = capsule {
            capsule.set_visible(index == 0);
        }
        self.credit_line_on(index);
        let (session, chapter) = {
            let state = self.state.borrow();
            (state.session, state.chapter.as_ref().map(|c| c.id.clone()))
        };
        if let Some(chapter) = chapter {
            self.send(ReaderCommand::Shown { session, chapter, leaf: index as u32, how });
        }
    }

    /// The page between chapters, after the last page.
    fn show_end(self: &Rc<Self>, index: usize, how: TurnHow) {
        self.state.borrow_mut().index = index;
        self.stop_wait();
        let total = self.story_pages();
        self.show_pages(total, total);
        // After the demo's last chapter, its end card stands in for the end page.
        let last = self.state.borrow().next.is_none();
        if !(last && self.demo_end()) {
            self.end.in_page();
            self.fill_end();
            motion::turn(&self.veil, &self.stack, || self.stack.set_visible_child_name("end"));
        }
        self.widget.grab_focus();
        self.shown(index, how);
    }

    /// The end page from the reader's state.
    fn fill_end(&self) {
        let facts = {
            let state = self.state.borrow();
            let Some(chapter) = state.chapter.as_ref() else { return };
            let preview = state
                .preview
                .as_ref()
                .filter(|(id, _)| state.next.as_ref().is_some_and(|n| &n.id == id))
                .map(|(_, texture)| texture.clone());
            end::EndFacts {
                chapter: chapter.number.clone().unwrap_or_else(|| chapter.label.clone()),
                read: state.read.clone(),
                next: state.next.clone(),
                why: state.why_none,
                source: state.next_source.clone(),
                preview,
                novel: state.medium == Some(Medium::Novel),
                credits: credits::end_line(&state),
                skip_to: state.skip_to.clone(),
                extras: state.extras,
            }
        };
        self.end.fill(&facts);
        self.fill_foot();
    }

    /// A strip's foot: the marker into the next chapter with its first image, or the end page
    /// after the last chapter there is. Pages one under another end on the end page too. While
    /// the strip runs on into a chapter, until its images come, the foot stays as it is.
    fn fill_foot(&self) {
        let (next, source, preview) = {
            let state = self.state.borrow();
            // Until the chapter run into has its images, the plate under the marker holds it.
            if state.flowing.is_some() || state.flowed {
                return;
            }
            let next = state.next.clone().filter(|_| state.mode == Some(ReaderMode::Strip));
            let preview = state
                .preview
                .as_ref()
                .filter(|(id, _)| next.as_ref().is_some_and(|n| &n.id == id))
                .map(|(_, texture)| texture.clone());
            (next, state.next_source.clone(), preview)
        };
        let credits = next.as_ref().and_then(|_| self.marker_credit_line());
        self.trail.show_next(next.as_ref(), source.as_deref(), credits.as_ref());
        self.trail.set_lead(preview.as_ref());
        self.strip_foot.set_visible(next.is_none());
        self.size_strip();
    }

    /// Another chapter in the open session; the plate holds its place if it takes a while.
    fn open_chapter(self: &Rc<Self>, session: u64, chapter: ChapterId) {
        if let Some(work) = self.state.borrow().work.clone() {
            self.asked_open.replace(Some((work, Some(chapter.clone()))));
        }
        self.opening_line.replace(t::linux_reader_opening());
        self.hide_notice();
        self.wait(Waiting::Chapter);
        self.send(ReaderCommand::OpenChapter { session, chapter });
    }

    /// Continue, from the end page: the next chapter.
    fn continue_on(self: &Rc<Self>) {
        let (session, next) = {
            let state = self.state.borrow();
            (state.session, state.next.clone())
        };
        if let Some(next) = next
            && session != 0
        {
            self.open_chapter(session, next.id);
        }
    }

    /// Skip to chapter 17, from the end page: past the extras, which stay unread.
    fn skip_extras(self: &Rc<Self>) {
        let (session, skip) = {
            let state = self.state.borrow();
            (state.session, state.skip_to.clone())
        };
        if let Some(skip) = skip
            && session != 0
        {
            self.open_chapter(session, skip.id);
        }
    }

    fn go_to_page(self: &Rc<Self>, page: u32) {
        let at = self
            .state
            .borrow()
            .leaves
            .iter()
            .position(|l| l.parts.iter().any(|p| p.page == page));
        if let Some(at) = at {
            if self.flows() {
                self.state.borrow_mut().strip_anchor =
                    Some(StripPos { image: at as u32, offset: 0.0 });
                self.hold_strip_place();
            } else {
                // The engine's move: a resume, Back to page, another device's place.
                self.show_leaf(at, TurnHow::Placed);
            }
        }
    }

    /// One leaf on: right of a right-to-left book is back.
    fn turn(self: &Rc<Self>, forward: bool, how: TurnHow) {
        let (index, total, medium) = {
            let state = self.state.borrow();
            (state.index, state.leaves.len(), state.medium)
        };
        let on_end = self.stack.visible_child_name().as_deref() == Some("end");
        if medium == Some(Medium::Novel) {
            // A local, so no borrow of the cell is held while the page turns.
            let novel = self.novel.borrow().clone();
            if on_end {
                // Forward from the end page goes on; back returns to the text.
                if forward {
                    self.continue_on();
                } else if let Some(novel) = novel {
                    self.state.borrow_mut().novel_end = false;
                    motion::turn(&self.veil, &self.stack, || {
                        self.stack.set_visible_child(&novel.widget);
                    });
                }
            } else if let Some(novel) = novel {
                novel.turn(forward);
            }
            return;
        }
        if self.flows() {
            self.slide_strip(forward);
            return;
        }
        if matches!(how, TurnHow::Key) && !on_end && self.scroll_within(forward) {
            return;
        }
        if self.credit_step(forward, on_end) {
            return;
        }
        if on_end && forward {
            self.continue_on();
        } else if forward {
            if index + 1 < total {
                self.show_leaf(index + 1, how);
            }
        } else if on_end {
            self.show_leaf(self.last_page_leaf(), how);
        } else if index > 0 {
            self.show_leaf(index - 1, how);
        } else {
            self.send(ReaderCommand::GoBack { session: self.state.borrow().session });
        }
    }

    /// The end page over a novel, when its last page turns.
    fn novel_end(self: &Rc<Self>) {
        self.state.borrow_mut().novel_end = true;
        self.end.in_page();
        self.fill_end();
        motion::turn(&self.veil, &self.stack, || self.stack.set_visible_child_name("end"));
        self.widget.grab_focus();
    }

    /// The demo's credit capsule in the header, as its licence asks, or none for any other work.
    fn demo_credit(&self, demo: Option<&kaeri_model::sources::DemoWorkInfo>) {
        let old = self.state.borrow_mut().demo.take();
        if let Some((_, capsule)) = old {
            self.head.remove(&capsule);
        }
        if let Some(demo) = demo {
            let capsule = super::demo::credit_capsule(demo);
            self.head.pack_end(&capsule);
            self.state.borrow_mut().demo = Some((demo.clone(), capsule.upcast()));
        }
    }

    /// After the demo's last page: its end card in place of the end page. False for any other
    /// work.
    fn demo_end(self: &Rc<Self>) -> bool {
        let Some(demo) = self.state.borrow().demo.as_ref().map(|(demo, _)| demo.clone()) else {
            return false;
        };
        let Some(ctx) = self.ctx.upgrade() else { return false };
        let close = {
            let this = Rc::downgrade(self);
            move || {
                if let Some(this) = this.upgrade() {
                    this.close();
                }
            }
        };
        let again = {
            let this = Rc::downgrade(self);
            move || {
                if let Some(this) = this.upgrade() {
                    this.show_leaf(0, TurnHow::Jump);
                }
            }
        };
        self.status.set_icon_name(None);
        self.status.set_title("");
        self.status.set_description(None);
        self.status.set_child(Some(&super::demo::end_card(&ctx, &demo, close, again)));
        // The card stands where the last page was: the ground is the look's page, as on the Mac.
        self.status.add_css_class("demo-end-ground");
        motion::turn(&self.veil, &self.stack, || self.stack.set_visible_child_name("status"));
        true
    }

    /// Back to the series, under a novel that can't open.
    fn back_button(self: &Rc<Self>) -> gtk::Button {
        let back = gtk::Button::with_label(&t::linux_reader_back_to_series());
        back.add_css_class("pill");
        back.add_css_class("suggested-action");
        back.set_halign(gtk::Align::Center);
        back.connect_clicked({
            let this = Rc::downgrade(self);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.close();
                }
            }
        });
        back
    }

    // ---------------------------------------------------------------------------------------
    // The strip

    fn build_strip(self: &Rc<Self>) {
        while let Some(child) = self.strip.first_child() {
            self.strip.remove(&child);
        }
        let leaves = self.state.borrow().leaves.clone();
        let mut frames = Vec::new();
        for leaf in leaves.iter().filter(|l| !l.end_page) {
            let ratio = leaf
                .parts
                .first()
                .and_then(|p| {
                    p.ratio.or_else(|| {
                        p.file.as_ref().map(|f| {
                            let (w, h) =
                                f.crop.map_or((f.width, f.height), |c| (c.width, c.height));
                            w as f32 / h.max(1) as f32
                        })
                    })
                })
                .unwrap_or(0.7)
                .max(0.05);
            let page = leaf.parts.first().map_or(0, |p| p.page);
            let (frame, picture) = strip_frame(page);
            self.strip.append(&frame);
            frames.push((picture, ratio));
        }
        self.state.borrow_mut().strip_frames = frames;
        self.end.at_foot(&self.strip_foot);
        self.fill_end();
        self.stop_wait();
        self.size_strip();
        self.hold_strip_place();
        self.strip_moved();
    }

    /// Each strip image's height as laid out, and the gap between them.
    fn strip_heights(&self) -> (Vec<i32>, i32) {
        let heights =
            self.state.borrow().strip_frames.iter().map(|(p, _)| p.height_request()).collect();
        (heights, self.strip.spacing())
    }

    /// Where the top of the viewport is in the strip: the image and how far down it.
    fn strip_place(&self) -> Option<StripPos> {
        let (heights, spacing) = self.strip_heights();
        if heights.is_empty() {
            return None;
        }
        // Above the open chapter, in the trail, there is no place in it.
        let value = self.strip_scroll.vadjustment().value();
        if value < self.trail.height() && !self.trail.is_empty() {
            return None;
        }
        let top = value - self.strip_base();
        let (image, offset) = strip_place_of(&heights, spacing, top);
        Some(StripPos { image: image as u32, offset })
    }

    /// Keep the strip at its anchor while images land: where it opened, or where the engine
    /// sent it, until the person scrolls.
    fn hold_strip_place(&self) {
        let Some(anchor) = self.state.borrow().strip_anchor else { return };
        let total = self.strip_heights().0.len();
        self.show_pages((anchor.image as usize + 1).min(total.max(1)), total);
        self.scroll_strip_to(self.strip_scroll_for(anchor));
    }

    /// The person moved the strip: it no longer holds where it opened, and says where it is.
    fn release_strip(self: &Rc<Self>) {
        if self.state.borrow_mut().strip_anchor.take().is_some() {
            self.strip_moved();
        }
    }

    /// Scroll the strip to `target` once the layout reaches that far. A newer ask replaces it.
    fn scroll_strip_to(&self, target: f64) {
        let ask = self.strip_scroll_ask.get().wrapping_add(1);
        self.strip_scroll_ask.set(ask);
        let adj = self.strip_scroll.vadjustment();
        if target <= 0.0 {
            adj.set_value(adj.lower());
            return;
        }
        let frames = Cell::new(0);
        let latest = Rc::clone(&self.strip_scroll_ask);
        self.strip_scroll.add_tick_callback(move |_, _| {
            if latest.get() != ask {
                return glib::ControlFlow::Break;
            }
            frames.set(frames.get() + 1);
            if adj.upper() - adj.page_size() >= target || frames.get() > 60 {
                adj.set_value(target.min(adj.upper() - adj.page_size()));
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }

    /// Each image at the column's width: the strip width setting's share of 720 px at the
    /// default, never wider than the window. Pages one under another fit the window. The place
    /// stays where it was when the heights change.
    fn size_strip(&self) {
        let vertical = self.state.borrow().mode == Some(ReaderMode::Vertical);
        let share = self.amount(ReaderSettingKey::StripWidth).unwrap_or(0.6).clamp(0.3, 1.0);
        let gap = self.choice(ReaderSettingKey::StripGap).as_deref() == Some("small");
        let width = f64::from(self.widget.width().max(1));
        let height = f64::from(self.widget.height().max(1));
        let keep = self.state.borrow().strip_anchor.or_else(|| self.strip_place());
        let spacing = if gap || vertical { 8 } else { 0 };
        let mut changed = self.strip.spacing() != spacing;
        self.strip.set_spacing(spacing);
        let column = width.min(720.0 * share / 0.6);
        self.trail.size(column.round() as i32, spacing, height.round() as i32);
        for (picture, ratio) in &self.state.borrow().strip_frames {
            let ratio = f64::from(*ratio);
            let w = if vertical { width.min(height * ratio) } else { column };
            let size = (w.round() as i32, (w / ratio).round() as i32);
            if picture.size_request() != size {
                picture.set_size_request(size.0, size.1);
                changed = true;
            }
        }
        changed |= self.size_strip_credits(column.round() as i32);
        if changed && let Some(keep) = keep {
            self.scroll_strip_to(self.strip_scroll_for(keep));
        }
    }

    /// Draws a strip page once its file is here. `redrawn` when the page had another file or
    /// crop before, as after crop borders changed: the picture swaps in place, without a fade.
    fn fill_strip_page(&self, page: u32, redrawn: bool) {
        let at = self
            .state
            .borrow()
            .leaves
            .iter()
            .filter(|l| !l.end_page)
            .position(|l| l.parts.first().is_some_and(|p| p.page == page));
        let Some(at) = at else { return };
        let Some(texture) = self.texture_for(page) else {
            if self.state.borrow().failed.contains(&page) {
                self.fail_strip_page(page);
            }
            return;
        };
        // With crop borders on, the engine sends the original with the part to show.
        let crop = self.state.borrow().files.get(&page).and_then(|f| f.crop);
        let (paintable, ratio) = match crop.filter(|c| c.width > 0 && c.height > 0) {
            Some(c) => (cropped(&texture, c), c.width as f32 / c.height as f32),
            None => (
                texture.clone().upcast::<gdk::Paintable>(),
                texture.width() as f32 / texture.height().max(1) as f32,
            ),
        };
        let (picture, resized) = {
            let mut state = self.state.borrow_mut();
            match state.strip_frames.get_mut(at) {
                Some((picture, guess)) => {
                    let resized = (*guess - ratio).abs() > 0.01;
                    *guess = ratio;
                    (Some(picture.clone()), resized)
                }
                None => (None, false),
            }
        };
        if let Some(picture) = picture {
            fill_strip_frame(&picture, &paintable, redrawn);
        }
        if resized {
            self.size_strip();
        }
    }

    /// Put each waiting strip image's words in the middle of the part of it on screen, as laid
    /// out: on every scroll, and once the strip has been laid out at a new size.
    fn place_plate_words(&self) {
        let window = f64::from(self.strip_scroll.height());
        for (picture, _) in &self.state.borrow().strip_frames {
            if picture.paintable().is_some() {
                continue;
            }
            let top = picture
                .compute_point(&self.strip_scroll, &gtk::graphene::Point::new(0.0, 0.0))
                .map(|at| f64::from(at.y()));
            let seen = top.filter(|_| window > 0.0).map(|top| (-top, window - top));
            strip_frame_seen(picture, seen);
        }
    }

    /// Ask for the pages near the viewport and, a moment after the scrolling stops, say where
    /// it stopped and whether the foot is in sight. While the strip holds its opening place it
    /// only asks. Over a chapter read before, the group names it and nothing is said; the marker
    /// into the next chapter crossing the middle opens that chapter.
    fn strip_moved(self: &Rc<Self>) {
        let adj = self.strip_scroll.vadjustment();
        let base = self.strip_base();
        let middle = adj.value() + adj.page_size() / 2.0;
        let over = self.trail.over(adj.value(), middle);
        let line = match &over {
            trail::Over::Gone { line, .. } => line.clone(),
            trail::Over::Open => self
                .state
                .borrow()
                .chapter
                .as_ref()
                .map(|c| c.label.clone())
                .unwrap_or_default(),
        };
        if self.line.label() != line {
            self.line.set_label(&line);
        }
        self.place_plate_words();
        let (top, bottom) = (adj.value() - base, adj.value() + adj.page_size() - base);
        let reach = adj.page_size() * 1.5;
        let (heights, spacing) = self.strip_heights();
        if heights.is_empty() {
            return;
        }
        let (wanted, foot_top) = strip_wanted(&heights, spacing, top, bottom, reach);
        for i in wanted {
            self.want_strip(i);
        }
        if self.state.borrow().strip_anchor.is_some() {
            return;
        }
        if let trail::Over::Gone { image, count, .. } = over {
            self.show_pages(image + 1, count);
            return;
        }
        if let Some(marker) = self.trail.marker_height()
            && middle - base > foot_top + self.tail_height() + marker / 2.0
        {
            self.flow_on();
            return;
        }
        let (image, offset) = strip_place_of(&heights, spacing, top);
        let foot_visible = bottom >= foot_top + 24.0;
        let total = heights.len();
        self.state.borrow_mut().index = image;
        self.show_pages(if foot_visible { total } else { image + 1 }, total);
        if let Some(id) = self.strip_timer.borrow_mut().take() {
            id.remove();
        }
        let this = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_millis(500), move || {
            let Some(this) = this.upgrade() else { return };
            this.strip_timer.borrow_mut().take();
            let (session, chapter) = {
                let state = this.state.borrow();
                (state.session, state.chapter.as_ref().map(|c| c.id.clone()))
            };
            if let Some(chapter) = chapter {
                // At the foot the place is the last image, all of it read.
                let at = if foot_visible {
                    StripPos { image: total.saturating_sub(1) as u32, offset: 1.0 }
                } else {
                    StripPos { image: image as u32, offset }
                };
                this.send(ReaderCommand::StripAt { session, chapter, at, foot_visible });
            }
        });
        *self.strip_timer.borrow_mut() = Some(id);
    }

    /// The marker crossed the middle: the next chapter opens under it, in the same scroll, with
    /// no plate over the page. The chapter run from is read and stays above.
    fn flow_on(self: &Rc<Self>) {
        self.commit_keep();
        let (session, chapter, next, work) = {
            let state = self.state.borrow();
            (state.session, state.chapter.clone(), state.next.clone(), state.work.clone())
        };
        let (Some(chapter), Some(next)) = (chapter, next) else { return };
        if session == 0 || self.state.borrow().flowing.is_some() {
            return;
        }
        if let Some(id) = self.strip_timer.borrow_mut().take() {
            id.remove();
        }
        // Its foot was in sight: the place is its last image, all of it read.
        let frames = std::mem::take(&mut self.state.borrow_mut().strip_frames);
        let last = frames.len().saturating_sub(1) as u32;
        self.send(ReaderCommand::StripAt {
            session,
            chapter: chapter.id.clone(),
            at: StripPos { image: last, offset: 1.0 },
            foot_visible: true,
        });
        // A notice belongs to the chapter it was about.
        self.hide_notice();
        let (lead, tail) = self.give_strip_credits();
        self.trail.take(chapter.label.clone(), &self.strip, frames, lead, tail);
        self.state.borrow_mut().flowing = Some(next.id.clone());
        if let Some(work) = work {
            self.asked_open.replace(Some((work, Some(next.id.clone()))));
        }
        self.send(ReaderCommand::OpenChapter { session, chapter: next.id });
    }

    fn want_strip(&self, index: usize) {
        let page = self.page_of(index);
        let Some(page) = page else { return };
        if self.state.borrow().files.contains_key(&page) {
            self.fill_strip_page(page, false);
            return;
        }
        self.want(index);
        let at = {
            let state = self.state.borrow();
            let at = state.leaves.iter().take(index).filter(|l| !l.end_page).count();
            let image = state.leaves.get(index).is_some_and(|l| !l.end_page);
            state.strip_frames.get(at).filter(|_| image).map(|(picture, _)| picture.clone())
        };
        if let Some(picture) = at {
            strip_frame_asked(&picture);
        }
    }

    /// Space and Page Down in a strip: nine tenths of the window on, on the quick token, from
    /// where the last one was going.
    fn slide_strip(self: &Rc<Self>, forward: bool) {
        self.release_strip();
        let adj = self.strip_scroll.vadjustment();
        let by = adj.page_size() * 0.9 * if forward { 1.0 } else { -1.0 };
        self.slide(&self.strip_scroll, &adj, by);
    }

    /// Scroll `adj` of `scroller` by `by`, on the quick spring; under Reduce Animation it jumps.
    fn slide(&self, scroller: &gtk::ScrolledWindow, adj: &gtk::Adjustment, by: f64) {
        let base = self
            .strip_slide
            .borrow()
            .as_ref()
            .filter(|a| a.state() == adw::AnimationState::Playing)
            .map_or(adj.value(), adw::SpringAnimation::value_to);
        let to =
            (base + by).clamp(adj.lower(), (adj.upper() - adj.page_size()).max(adj.lower()));
        if let Some(old) = self.strip_slide.take() {
            old.pause();
        }
        let animation = motion::quick(
            scroller,
            adj.value(),
            to,
            adw::PropertyAnimationTarget::new(adj, "value"),
        );
        animation.play();
        self.strip_slide.replace(Some(animation));
    }

    // ---------------------------------------------------------------------------------------
    // Novel

    fn open_novel(self: &Rc<Self>, open: &kaeri_model::reader::NovelOpenView) {
        let Some(ctx) = self.ctx.upgrade() else { return };
        self.stop_wait();
        if let Some(old) = self.novel.take() {
            self.stack.remove(&old.widget);
        }
        let hub = Rc::downgrade(&ctx.hub);
        let sender: Rc<dyn Fn(ReaderCommand)> = Rc::new(move |command| {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::Reader(command));
            }
        });
        let tell: Rc<dyn Fn(NovelSays)> = {
            let this = Rc::downgrade(self);
            Rc::new(move |says| {
                let Some(this) = this.upgrade() else { return };
                match says {
                    NovelSays::Page { page, pages } => {
                        this.show_pages(page as usize + 1, pages as usize);
                    }
                    NovelSays::Middle => this.toggle_chrome(),
                    NovelSays::End => this.novel_end(),
                    // Try again opens the chapter afresh, in a new page.
                    NovelSays::Failed => this.open_failed(&ReadingFailure::Other {
                        reason: t::linux_reader_novel_stopped(),
                    }),
                }
            })
        };
        match NovelView::new(&ctx, open, sender, tell) {
            Ok(novel) => {
                self.stack.add_named(&novel.widget, Some("novel"));
                motion::turn(&self.veil, &self.stack, || {
                    self.stack.set_visible_child_name("novel");
                });
                *self.novel.borrow_mut() = Some(novel);
            }
            Err(why) => {
                tracing::warn!(%why, "the novel reader could not start");
                self.status.set_icon_name(None);
                self.status.set_title(&t::linux_reader_novel_unavailable());
                self.status.set_description(Some(&t::linux_reader_novel_needs_webkit()));
                self.status.set_child(Some(&self.back_button()));
                self.status.remove_css_class("demo-end-ground");
                self.stack.set_visible_child_name("status");
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Settings

    fn settings_view(self: &Rc<Self>, view: ReaderSettingsView) {
        // Settings for another medium (a shot's, or another window's) wait for their reader.
        let medium = self.state.borrow().medium;
        if medium.is_some_and(|m| m != view.medium) {
            return;
        }
        self.follow_settings(&view);
        self.state.borrow_mut().settings = Some(view);
        self.fill_sheet(false);
        self.apply_look();
    }

    /// A choice setting as the engine last sent it.
    fn choice(&self, key: ReaderSettingKey) -> Option<String> {
        let state = self.state.borrow();
        let row = state.settings.as_ref()?.rows.iter().find(|r| r.key == key)?;
        match &row.value {
            ReaderValue::Choice { name } => Some(name.clone()),
            _ => None,
        }
    }

    fn amount(&self, key: ReaderSettingKey) -> Option<f64> {
        let state = self.state.borrow();
        let row = state.settings.as_ref()?.rows.iter().find(|r| r.key == key)?;
        match row.value {
            ReaderValue::Amount { value } => Some(value),
            _ => None,
        }
    }

    /// What the settings change on screen: the ground behind the pages, the fit and the strip.
    fn apply_look(&self) {
        // Dark keeps the app's dark tokens; Paper, Light and Sepia put the chrome on the light
        // ones (the reader board). System follows the app, which is dark.
        let light = chrome_is_light(self.choice(ReaderSettingKey::Look).as_deref());
        if light {
            self.widget.add_css_class("reader-light");
        } else {
            self.widget.remove_css_class("reader-light");
        }
        // The page's own colour in each light look, for what stands in for a page.
        let look = self.choice(ReaderSettingKey::Look);
        for (name, class) in
            [("paper", "look-paper"), ("light", "look-light"), ("sepia", "look-sepia")]
        {
            if look.as_deref() == Some(name) {
                self.status.add_css_class(class);
            } else {
                self.status.remove_css_class(class);
            }
        }
        let manga = self.state.borrow().medium == Some(Medium::Manga);
        let ground = if manga { self.choice(ReaderSettingKey::Background) } else { None };
        for view in
            [self.paged_scroll.upcast_ref::<gtk::Widget>(), self.strip_scroll.upcast_ref()]
        {
            view.remove_css_class("reader-black");
            view.remove_css_class("reader-white");
            match ground.as_deref() {
                Some("black") => view.add_css_class("reader-black"),
                Some("white") => view.add_css_class("reader-white"),
                _ => {}
            }
        }
        self.apply_fit();
        self.size_strip();
    }

    /// Fit screen keeps the whole page in the window; width and height fill that side and let
    /// the other run past, to scroll.
    fn apply_fit(&self) {
        use gtk::PolicyType::{Automatic, Never};
        let fit = self.choice(ReaderSettingKey::Fit).unwrap_or_default();
        let ratio =
            self.picture.paintable().map(|p| p.intrinsic_aspect_ratio()).filter(|r| *r > 0.0);
        let width = f64::from(self.widget.width().max(1));
        let height = f64::from(self.widget.height().max(1));
        let (h, v, request) = match (fit.as_str(), ratio) {
            ("width", Some(r)) => (Never, Automatic, (-1, (width / r).round() as i32)),
            ("height", Some(r)) => (Automatic, Never, ((height * r).round() as i32, -1)),
            _ => (Never, Never, (-1, -1)),
        };
        self.paged_scroll.set_policy(h, v);
        self.picture.set_size_request(request.0, request.1);
    }

    /// A new page starts at its top, and at its right edge in a right-to-left book.
    fn scroll_to_start(&self) {
        let rtl = self.rtl();
        let scroll = self.paged_scroll.clone();
        glib::idle_add_local_once(move || {
            let v = scroll.vadjustment();
            v.set_value(v.lower());
            let h = scroll.hadjustment();
            h.set_value(if rtl { h.upper() - h.page_size() } else { h.lower() });
        });
    }

    /// With a page taller than the window, the keys scroll it before they turn, on the quick
    /// token. True when they scrolled.
    fn scroll_within(&self, forward: bool) -> bool {
        let adj = self.paged_scroll.vadjustment();
        let room = if forward {
            adj.upper() - adj.page_size() - adj.value()
        } else {
            adj.value() - adj.lower()
        };
        if room <= 1.0 {
            return false;
        }
        let by = room.min(adj.page_size() * 0.9);
        self.slide(&self.paged_scroll, &adj, if forward { by } else { -by });
        true
    }

    // ---------------------------------------------------------------------------------------
    // Controls

    /// Fill Chapters for where the reader is, as it opens.
    fn fill_chapters(&self) {
        let here = {
            let state = self.state.borrow();
            let Some(chapter) = state.chapter.as_ref() else { return };
            Here {
                chapter: chapter.id.clone(),
                label: chapter.label.clone(),
                pages: state.medium == Some(Medium::Manga) && !state.leaves.is_empty(),
                rtl: matches!(state.direction, Some(ReadDirection::RightToLeft))
                    && !state.mode.is_some_and(ReaderMode::flows),
                leaves: state.leaves.clone(),
                open_leaf: state.index,
                files: state.files.clone(),
            }
        };
        self.chapters.show_here(here);
    }

    /// Ask the engine for one page, once.
    fn need_page(&self, page: u32) {
        let (session, chapter) = {
            let state = self.state.borrow();
            if state.files.contains_key(&page) || state.asked.contains(&page) {
                return;
            }
            (state.session, state.chapter.as_ref().map(|c| c.id.clone()))
        };
        let Some(chapter) = chapter else { return };
        self.state.borrow_mut().asked.insert(page);
        self.send(ReaderCommand::NeedPage { session, chapter, page });
    }

    /// Bring the chrome, and take it away again `CHROME_REST` after the last call: the pointer
    /// moving calls this as it moves. It stays while the settings are open, the pointer is over
    /// the group, the keyboard is in it or a shot pinned it.
    fn reveal_chrome(self: &Rc<Self>) {
        self.head_fade.set(&self.head, true);
        // The group first: a hidden one measures nothing, and the notice would sit on it.
        self.group_fade.set(&self.group, true);
        self.place_notice(true);
        self.credit_line_chrome(true);
        self.widget.set_cursor(None);
        if let Some(id) = self.hide_timer.borrow_mut().take() {
            id.remove();
        }
        let this = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(CHROME_REST, move || {
            let Some(this) = this.upgrade() else { return };
            this.hide_timer.borrow_mut().take();
            let open = this.settings.is_active()
                || this.chapters_button.is_active()
                || this.go_to.is_visible();
            if open || this.group_hovered() || this.keyboard_in_chrome() || this.pinned.get() {
                this.reveal_chrome();
                return;
            }
            this.hide_chrome();
        });
        *self.hide_timer.borrow_mut() = Some(id);
    }

    /// The chrome goes, and the pointer with it. It stays while the keyboard is in it: a
    /// hidden control would take the focus with it.
    fn hide_chrome(&self) {
        if let Some(id) = self.hide_timer.borrow_mut().take() {
            id.remove();
        }
        if self.pinned.get() || self.keyboard_in_chrome() {
            return;
        }
        self.head_fade.set(&self.head, false);
        self.group_fade.set(&self.group, false);
        self.credit_line_chrome(false);
        self.widget.set_cursor_from_name(Some("none"));
        self.place_notice(false);
    }

    /// Where the notice sits: at the top over pages. A novel's sits in its bottom margin, clear
    /// of the text, and over the group while the group shows, which uses the same margin.
    fn place_notice(&self, chrome: bool) {
        if self.state.borrow().medium == Some(Medium::Novel) {
            self.notice.set_valign(gtk::Align::End);
            self.notice.set_margin_top(0);
            self.notice.set_margin_bottom(notice_lift(&self.group, chrome));
        } else {
            self.notice.set_valign(gtk::Align::Start);
            self.notice.set_margin_top(60);
            self.notice.set_margin_bottom(0);
        }
    }

    /// A click in the middle of the page: the chrome comes, and the next one takes it away.
    fn toggle_chrome(self: &Rc<Self>) {
        if self.head_fade.shown() {
            self.hide_chrome();
        } else {
            self.reveal_chrome();
        }
    }

    fn group_hovered(&self) -> bool {
        self.group_hover.contains_pointer()
    }

    /// The focus is on one of the chrome's controls.
    fn focus_in_chrome(&self) -> bool {
        let Some(focus) = self.widget.root().and_then(|root| root.focus()) else {
            return false;
        };
        focus.is_ancestor(&self.head)
            || focus.is_ancestor(&self.group)
            || focus == *self.head.upcast_ref::<gtk::Widget>()
            || focus == *self.group.upcast_ref::<gtk::Widget>()
    }

    /// The keyboard is walking the chrome: the focus is on a control in it, and shows.
    fn keyboard_in_chrome(&self) -> bool {
        self.widget
            .root()
            .and_downcast::<gtk::Window>()
            .is_some_and(|window| window.property::<bool>("focus-visible"))
            && self.focus_in_chrome()
    }

    /// Tab brings the chrome and walks it: the head's controls, the page, then the group's,
    /// and round again, Shift+Tab the other way. The reader walks it itself, as GTK's own
    /// chain from the head runs back into the page behind it and never reaches the group.
    /// False when the window should move the focus: inside a popover, which walks its own.
    fn tab_into_chrome(self: &Rc<Self>, back: bool) -> bool {
        self.reveal_chrome();
        let Some(window) = self.widget.root().and_downcast::<gtk::Window>() else {
            return false;
        };
        let focus = gtk::prelude::GtkWindowExt::focus(&window);
        if focus.as_ref().is_some_and(|f| f.ancestor(gtk::Popover::static_type()).is_some()) {
            return false;
        }
        let page = self.widget.upcast_ref::<gtk::Widget>();
        let stops = tab_stops(self.head.upcast_ref(), page, self.group.upcast_ref());
        let Some(next) = next_stop(&stops, page, focus.as_ref(), back) else { return false };
        window.set_focus_visible(true);
        next.grab_focus();
        true
    }

    /// Keep the chrome up for a shot, as the board draws it.
    #[cfg(feature = "fixtures")]
    pub fn shot_chrome(self: &Rc<Self>, pinned: bool) {
        self.pinned.set(pinned);
        if pinned {
            self.reveal_chrome();
        } else {
            self.chapters.popover.popdown();
        }
    }

    /// Tab on the page for a shot, after the chrome has gone, as the keyboard presses it. The
    /// window shows focus on a key, as GTK does for a real one.
    #[cfg(feature = "fixtures")]
    pub fn shot_tab(self: &Rc<Self>) {
        if let Some(window) = self.widget.root().and_downcast::<gtk::Window>() {
            window.set_focus_visible(true);
        }
        let took = self.take_key(gdk::Key::Tab, gdk::ModifierType::empty());
        let (shown, inside) =
            (self.head.is_visible() && self.group.is_visible(), self.focus_in_chrome());
        eprintln!("reader tab: took {took}, chrome shown {shown}, focus in chrome {inside}"); // i18n-skip
    }

    /// Press Try again on the plate of a chapter that couldn't open, for a shot.
    #[cfg(feature = "fixtures")]
    pub fn shot_try_again(self: &Rc<Self>) {
        fn find(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
            if let Some(button) = widget.downcast_ref::<gtk::Button>()
                && button.is_drawable()
                && button.label().is_some_and(|words| words.as_str() == label)
            {
                return Some(button.clone());
            }
            std::iter::successors(widget.first_child(), gtk::Widget::next_sibling)
                .find_map(|child| find(&child, label))
        }
        let again = find(self.widget.upcast_ref(), &t::reader_try_again());
        eprintln!("reader try again: found {}", again.is_some()); // i18n-skip
        if let Some(again) = again {
            again.emit_clicked();
            eprintln!("reader try again: pressed"); // i18n-skip
        }
    }

    /// Read a novel's chapter to its end for a shot: turn on until the end page shows, then
    /// Continue, and say what was read and what opened.
    #[cfg(feature = "fixtures")]
    pub fn shot_novel_end(self: &Rc<Self>) {
        let turns = Rc::new(Cell::new(0));
        let this = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(700), move || {
            let Some(this) = this.upgrade() else { return glib::ControlFlow::Break };
            let (ended, chapter) = {
                let state = this.state.borrow();
                (state.novel_end, state.chapter.as_ref().map(|c| c.id.to_string()))
            };
            if !ended && turns.get() < 60 {
                turns.set(turns.get() + 1);
                let novel = this.novel.borrow().clone();
                if let Some(novel) = novel {
                    novel.turn(true);
                }
                return glib::ControlFlow::Continue;
            }
            eprintln!("novel end: shown {ended} after {} turns in {chapter:?}", turns.get()); // i18n-skip
            let later = Rc::downgrade(&this);
            glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                let Some(this) = later.upgrade() else { return };
                let (read, next) = {
                    let state = this.state.borrow();
                    (state.read.is_some(), state.next.as_ref().map(|c| c.id.to_string()))
                };
                eprintln!("novel end: read {read}, next {next:?}"); // i18n-skip
                this.continue_on();
                let after = Rc::downgrade(&this);
                glib::timeout_add_local_once(Duration::from_millis(3000), move || {
                    let Some(this) = after.upgrade() else { return };
                    let chapter =
                        this.state.borrow().chapter.as_ref().map(|c| c.id.to_string());
                    eprintln!("novel end: continued to {chapter:?}"); // i18n-skip
                });
            });
            glib::ControlFlow::Break
        });
    }

    /// Open Chapters for a shot, on its `tab`: "pages" or "chapters".
    #[cfg(feature = "fixtures")]
    pub fn shot_chapters(self: &Rc<Self>, tab: &str) {
        self.shot_chrome(true);
        self.chapters_button.popup();
        self.chapters.shot_tab(tab);
    }

    /// Where the place is, for the scrubber: page `at` from 1 of `total`.
    fn show_pages(&self, at: usize, total: usize) {
        let (at, total) = self.held_pages(at, total);
        let total = total.max(1);
        let at = at.clamp(1, total);
        self.scrub.set_range(1.0, total.max(2) as f64);
        self.scrub.set_sensitive(total > 1);
        if self.scrub_timer.borrow().is_none() {
            self.scrub.set_value(at as f64);
            self.scrub_at.set_label(&at.to_string());
        }
        self.scrub_total.set_label(&total.to_string());
        let said = self
            .part_said(at)
            .unwrap_or_else(|| t::linux_reader_page_of(at as i64, total as i64));
        self.scrub.update_property(&[gtk::accessible::Property::ValueText(&said)]);
        self.go_to_field.set_range(1.0, total as f64);
        let hint = t::linux_reader_go_to_page_field(total as i64);
        self.go_to_field.set_tooltip_text(Some(&hint));
    }

    /// The scrubber dragged or keyed to `value`: the number follows at once, the page once the
    /// thumb rests.
    fn scrubbed(self: &Rc<Self>, value: f64) {
        let (lower, upper) = (self.scrub.adjustment().lower(), self.scrub.adjustment().upper());
        let at = value.round().clamp(lower, upper) as usize;
        self.scrub_at.set_label(&at.to_string());
        self.reveal_chrome();
        self.part_tip_at(at);
        if let Some(id) = self.scrub_timer.borrow_mut().take() {
            id.remove();
        }
        let this = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_millis(250), move || {
            let Some(this) = this.upgrade() else { return };
            this.scrub_timer.borrow_mut().take();
            this.jump_to_page(at);
        });
        *self.scrub_timer.borrow_mut() = Some(id);
    }

    /// Go to page `page`, from 1: the leaf that holds it, the strip image, a novel's page.
    fn jump_to_page(self: &Rc<Self>, page: usize) {
        let novel = self.state.borrow().medium == Some(Medium::Novel);
        if novel {
            let novel = self.novel.borrow().clone();
            if let Some(novel) = novel {
                novel.go_to(page.saturating_sub(1) as u32);
            }
            return;
        }
        let leaf = if self.flows() { page.saturating_sub(1) } else { self.leaf_of_page(page) };
        self.jump_to(leaf);
    }

    /// Go to a leaf or a strip image, from 0: Chapters' page grid and the scrubber.
    fn jump_to(self: &Rc<Self>, index: usize) {
        if self.flows() {
            self.state.borrow_mut().strip_anchor =
                Some(StripPos { image: index as u32, offset: 0.0 });
            self.hold_strip_place();
            // A jump is the person's: it reports once the strip is there, so the engine can
            // offer Back to page, and the place follows from the scroll.
            let this = Rc::downgrade(self);
            glib::timeout_add_local_once(Duration::from_millis(300), move || {
                let Some(this) = this.upgrade() else { return };
                let (session, chapter) = {
                    let state = this.state.borrow();
                    (state.session, state.chapter.as_ref().map(|c| c.id.clone()))
                };
                if let Some(chapter) = chapter {
                    let (leaf, how) = (index as u32, TurnHow::Jump);
                    this.send(ReaderCommand::Shown { session, chapter, leaf, how });
                }
                this.release_strip();
            });
        } else {
            self.show_leaf(index, TurnHow::Jump);
        }
    }

    /// Go to page, from G: a field over the scrubber.
    fn ask_page(self: &Rc<Self>) {
        if self.state.borrow().leaves.is_empty() && self.novel.borrow().is_none() {
            return;
        }
        self.reveal_chrome();
        self.go_to_field.set_value(self.scrub.value());
        self.go_to.popup();
        self.go_to_field.grab_focus();
    }

    /// What the controller is pointed at: the page, or the settings while they are open.
    pub fn pad_context(&self) -> kaeri_model::input::PadContext {
        kaeri_model::input::PadContext::Reader { settings_open: self.settings.is_active() }
    }

    /// A controller press: the D-pad turns as the arrow keys do, the shoulders change the
    /// chapter, A shows or hides the bar, the menu button opens the settings and B closes.
    pub fn pad(self: &Rc<Self>, what: kaeri_model::input::ReaderPad) {
        use kaeri_model::input::{Direction, ReaderPad};
        let rtl = self.rtl();
        match what {
            ReaderPad::Turn(Direction::Left) => self.turn(rtl, TurnHow::Key),
            ReaderPad::Turn(Direction::Right) => self.turn(!rtl, TurnHow::Key),
            ReaderPad::Turn(Direction::Up) => self.turn(false, TurnHow::Key),
            ReaderPad::Turn(Direction::Down) => self.turn(true, TurnHow::Key),
            ReaderPad::Chapter(by) => self.step_chapter(by),
            ReaderPad::Bar => self.toggle_chrome(),
            ReaderPad::Settings => {
                self.reveal_chrome();
                self.settings.popup();
            }
            ReaderPad::Close => self.close(),
        }
    }

    /// The previous (`-1`) or next chapter, when there is one.
    fn step_chapter(self: &Rc<Self>, by: i32) {
        let (session, chapter) = {
            let state = self.state.borrow();
            let chapter = if by < 0 { state.previous.clone() } else { state.next.clone() };
            (state.session, chapter)
        };
        if let Some(chapter) = chapter
            && session != 0
        {
            self.open_chapter(session, chapter.id);
        }
    }

    fn rtl(&self) -> bool {
        matches!(self.state.borrow().direction, Some(ReadDirection::RightToLeft))
    }

    fn controls(self: &Rc<Self>) {
        self.widget.add_controller(tab_keys({
            let this = Rc::downgrade(self);
            move |back| this.upgrade().is_some_and(|this| this.tab_into_chrome(back))
        }));
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let this = Rc::downgrade(self);
            move |_, key, _, modifiers| {
                let Some(this) = this.upgrade() else { return glib::Propagation::Proceed };
                if this.take_key(key, modifiers) {
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
        });
        self.widget.add_controller(keys);
        // Focus that lands on a page behind the reader comes back to it, and keys follow it.
        if let Some(ctx) = self.ctx.upgrade() {
            let this = Rc::downgrade(self);
            keys::hold(
                &ctx.window,
                &self.widget,
                Rc::new(move |key, modifiers| {
                    this.upgrade().is_some_and(|this| this.take_key(key, modifiers))
                }),
            );
            self.opener.watch(&ctx.window, &self.widget);
        }

        // A closed reader lets go of what it held once the window has crossfaded away from it.
        self.widget.connect_unmap({
            let this = Rc::downgrade(self);
            move |_| {
                if let Some(this) = this.upgrade()
                    && !this.showing.get()
                {
                    this.reset();
                }
            }
        });

        // Tap zones: the outer thirds turn, the middle brings the chrome or takes it away.
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        click.connect_released({
            let this = Rc::downgrade(self);
            move |gesture, _, x, _| {
                let Some(this) = this.upgrade() else { return };
                // A click on the page takes the keys back from the chrome, which may go then.
                if this.focus_in_chrome() {
                    this.widget.grab_focus();
                }
                if this.stack.visible_child_name().as_deref() != Some("paged") {
                    this.reveal_chrome();
                    return;
                }
                let width = f64::from(this.widget.width().max(1));
                let rtl = this.rtl();
                if x < width / 3.0 {
                    this.turn(rtl, TurnHow::Turn);
                } else if x > width * 2.0 / 3.0 {
                    this.turn(!rtl, TurnHow::Turn);
                } else {
                    this.toggle_chrome();
                }
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        });
        self.picture.add_controller(click);

        let moved = gtk::EventControllerMotion::new();
        moved.connect_motion({
            let this = Rc::downgrade(self);
            move |_, _, _| {
                if let Some(this) = this.upgrade() {
                    this.reveal_chrome();
                }
            }
        });
        self.widget.add_controller(moved);
        self.group.add_controller(self.group_hover.clone());
        // Focus that comes into the chrome by any key brings it, and the keyboard holds it.
        for part in [self.head.upcast_ref::<gtk::Widget>(), self.group.upcast_ref()] {
            let came = gtk::EventControllerFocus::new();
            came.connect_enter({
                let this = Rc::downgrade(self);
                move |_| {
                    if let Some(this) = this.upgrade() {
                        this.reveal_chrome();
                    }
                }
            });
            part.add_controller(came);
        }

        // A new size or new images move every plate's part on screen.
        self.strip_scroll.vadjustment().connect_changed({
            let this = Rc::downgrade(self);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.place_plate_words();
                }
            }
        });
        self.strip_scroll.vadjustment().connect_value_changed({
            let this = Rc::downgrade(self);
            move |_| {
                if let Some(this) = this.upgrade()
                    && !this.state.borrow().strip_frames.is_empty()
                {
                    this.strip_moved();
                }
            }
        });
        // The person's own scrolling lets go of the place the strip opened at: the wheel, a
        // drag on the scrollbar, a finger.
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        wheel.connect_scroll({
            let this = Rc::downgrade(self);
            move |_, _, _| {
                if let Some(this) = this.upgrade() {
                    this.release_strip();
                }
                glib::Propagation::Proceed
            }
        });
        self.strip_scroll.add_controller(wheel);
        let grab = gtk::GestureClick::new();
        grab.set_propagation_phase(gtk::PropagationPhase::Capture);
        grab.connect_pressed({
            let this = Rc::downgrade(self);
            move |_, _, _, _| {
                if let Some(this) = this.upgrade() {
                    this.release_strip();
                }
            }
        });
        self.strip_scroll.vscrollbar().add_controller(grab);
        let drag = gtk::GestureDrag::new();
        drag.set_touch_only(true);
        drag.set_propagation_phase(gtk::PropagationPhase::Capture);
        drag.connect_drag_begin({
            let this = Rc::downgrade(self);
            move |_, _, _| {
                if let Some(this) = this.upgrade() {
                    this.release_strip();
                }
            }
        });
        self.strip_scroll.add_controller(drag);

        // The window's size is the engine's to know: the pages are prepared for it.
        self.widget.connect_notify_local(Some("scale-factor"), {
            let this = Rc::downgrade(self);
            move |_, _| {
                if let Some(this) = this.upgrade() {
                    let session = this.state.borrow().session;
                    if session != 0 {
                        this.send(ReaderCommand::Surface { session, surface: this.surface() });
                    }
                }
            }
        });
        // A window that changes shape changes whether two pages fit: the engine is told once it
        // has settled.
        let last = Cell::new((0, 0));
        let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
        self.widget.add_tick_callback({
            let this = Rc::downgrade(self);
            move |widget, _| {
                let size = (widget.width(), widget.height());
                if size == last.get() {
                    return glib::ControlFlow::Continue;
                }
                let first = last.replace(size) == (0, 0);
                // The page and the strip follow the window at once; the engine after a pause.
                if let Some(this) = this.upgrade() {
                    this.apply_fit();
                    this.size_strip();
                    this.group.set_width_request(GROUP_WIDTH.min(size.0 - 32).max(240));
                    this.chapters.fit(size);
                    if this.sheet.is_visible()
                        && *this.sheet_shape_now.borrow() != this.sheet_shape()
                    {
                        this.fill_sheet(true);
                    }
                    let plate = ((f64::from(size.1) * 0.7).round() as i32).min(size.0.max(1));
                    this.failed_plate.set_size_request(plate, -1);
                }
                if let Some(id) = pending.borrow_mut().take() {
                    id.remove();
                }
                // The first size is the reader's first layout, after an open that went out
                // without one: the engine hears it at once if it differs.
                if first && let Some(this) = this.upgrade() {
                    this.surface_settled();
                }
                if !first {
                    let (this, done) = (this.clone(), Rc::clone(&pending));
                    let id =
                        glib::timeout_add_local_once(Duration::from_millis(250), move || {
                            done.borrow_mut().take();
                            if let Some(this) = this.upgrade() {
                                this.surface_settled();
                            }
                        });
                    *pending.borrow_mut() = Some(id);
                }
                glib::ControlFlow::Continue
            }
        });
    }

    /// A key in the reader. True when the reader took it. Keys with Alt, and with Ctrl other
    /// than the text size ones, are the window's.
    fn take_key(self: &Rc<Self>, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        use gdk::Key as K;
        let rtl = self.rtl();
        let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        if modifiers.contains(gdk::ModifierType::ALT_MASK) {
            return false;
        }
        if ctrl {
            match key {
                K::plus | K::equal | K::KP_Add => self.step_text_size(1),
                K::minus | K::KP_Subtract => self.step_text_size(-1),
                K::_0 | K::KP_0 => self.step_text_size(0),
                K::z if self.keeping.borrow().is_some() => self.undo_keep(),
                _ => return false,
            }
            return true;
        }
        let mut turned = true;
        match key {
            K::Tab | K::KP_Tab | K::ISO_Left_Tab => {
                let back =
                    key == K::ISO_Left_Tab || modifiers.contains(gdk::ModifierType::SHIFT_MASK);
                return self.tab_into_chrome(back);
            }
            K::Escape => self.close(),
            K::Right => self.turn(!rtl, TurnHow::Key),
            K::Left => self.turn(rtl, TurnHow::Key),
            K::space | K::Page_Down | K::Down => self.turn(true, TurnHow::Key),
            K::Page_Up | K::Up => self.turn(false, TurnHow::Key),
            // A novel's first and last page are the chapter's, across its spine's documents.
            K::Home | K::End if self.state.borrow().medium == Some(Medium::Novel) => {
                let novel = self.novel.borrow().clone();
                if let Some(novel) = novel {
                    // From the end page, back to the text, which then moves.
                    if self.stack.visible_child_name().as_deref() == Some("end") {
                        self.state.borrow_mut().novel_end = false;
                        motion::turn(&self.veil, &self.stack, || {
                            self.stack.set_visible_child(&novel.widget);
                        });
                    }
                    novel.go_to_edge(key == K::End);
                }
            }
            K::Home => self.jump_to(0),
            K::End => {
                let last = if self.flows() {
                    self.state.borrow().strip_frames.len().saturating_sub(1)
                } else {
                    self.last_page_leaf()
                };
                self.jump_to(last);
            }
            K::F11 | K::f => {
                self.toggle_fullscreen();
                turned = false;
            }
            K::g | K::G => {
                self.ask_page();
                turned = false;
            }
            _ => return false,
        }
        if turned {
            self.hide_chrome();
        }
        true
    }
}

/// The part of `texture` that `crop` names, in the texture's pixels, as a picture that size.
fn cropped(texture: &gdk::Texture, crop: kaeri_model::reader::PixelRect) -> gdk::Paintable {
    let (x, y, w, h) = (crop.x as f32, crop.y as f32, crop.width as f32, crop.height as f32);
    let snapshot = gtk::Snapshot::new();
    snapshot.push_clip(&gtk::graphene::Rect::new(0.0, 0.0, w, h));
    snapshot.translate(&gtk::graphene::Point::new(-x, -y));
    texture.snapshot(&snapshot, f64::from(texture.width()), f64::from(texture.height()));
    snapshot.pop();
    snapshot
        .to_paintable(Some(&gtk::graphene::Size::new(w, h)))
        .unwrap_or_else(|| texture.clone().upcast())
}

/// A picture file decoded by the image crate, as RGBA.
fn decode_texture(path: impl AsRef<std::path::Path>) -> Option<gdk::Texture> {
    let image =
        image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    rgba_texture(image.into_rgba8())
}

fn rgba_texture(rgba: image::RgbaImage) -> Option<gdk::Texture> {
    let (width, height) =
        (i32::try_from(rgba.width()).ok()?, i32::try_from(rgba.height()).ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    let stride = rgba.width() as usize * 4;
    Some(
        gdk::MemoryTexture::new(
            width,
            height,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(rgba.into_raw()),
            stride,
        )
        .upcast(),
    )
}

/// As [`load_texture`], cut to the file's crop when it has one: for a picture that is not
/// composed as a leaf is.
fn load_texture_cropped(file: &PageFile) -> Option<gdk::Texture> {
    let Some(c) = file.crop.filter(|c| c.width > 0 && c.height > 0) else {
        return load_texture(file);
    };
    let image =
        image::ImageReader::open(&file.path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    if c.x + c.width > image.width() || c.y + c.height > image.height() {
        return load_texture(file);
    }
    rgba_texture(image.crop_imm(c.x, c.y, c.width, c.height).into_rgba8())
}

/// A page's picture: GTK's own loaders first, then the image crate where they can't (no WebP or
/// AVIF loader, or glycin's sandbox can't start). `None` when nothing can decode it, or a raw
/// file is shorter than its size says.
fn load_texture(file: &PageFile) -> Option<gdk::Texture> {
    match &file.encoding {
        PageEncoding::Original { .. } => {
            gdk::Texture::from_filename(&file.path).ok().or_else(|| decode_texture(&file.path))
        }
        PageEncoding::Raw { format, stride } => {
            let bytes = std::fs::read(&file.path).ok()?;
            let (format, depth) = match format {
                PixelFormat::Gray8 => (gdk::MemoryFormat::G8, 1),
                PixelFormat::Rgba8 => (gdk::MemoryFormat::R8g8b8a8, 4),
            };
            // GTK hands back no texture for a size the bytes don't fill, and drawing that aborts.
            let (width, height, stride) =
                (file.width as usize, file.height as usize, *stride as usize);
            if width == 0
                || height == 0
                || stride < width * depth
                || bytes.len() < stride * (height - 1) + width * depth
                || i32::try_from(width.max(height)).is_err()
            {
                return None;
            }
            Some(
                gdk::MemoryTexture::new(
                    file.width as i32,
                    file.height as i32,
                    format,
                    &glib::Bytes::from_owned(bytes),
                    stride,
                )
                .upcast(),
            )
        }
    }
}

fn setting_name(key: ReaderSettingKey) -> String {
    match key {
        ReaderSettingKey::Mode => t::linux_reader_reading_mode(),
        ReaderSettingKey::Direction => t::linux_reader_direction(),
        ReaderSettingKey::Spreads => t::linux_reader_spreads(),
        ReaderSettingKey::StripWidth => t::linux_reader_strip_width(),
        ReaderSettingKey::StripGap => t::linux_reader_strip_gap(),
        ReaderSettingKey::Fit => t::linux_reader_fit(),
        ReaderSettingKey::CropBorders => t::linux_reader_crop_borders(),
        ReaderSettingKey::PageLayout => t::linux_reader_page_layout(),
        ReaderSettingKey::Background => t::linux_reader_background(),
        ReaderSettingKey::Look => t::linux_reader_look(),
        ReaderSettingKey::Font => t::linux_reader_font(),
        ReaderSettingKey::TextSize => t::linux_reader_text_size(),
        ReaderSettingKey::Margins => t::linux_reader_margins(),
        ReaderSettingKey::LineSpacing => t::linux_reader_line_spacing(),
        ReaderSettingKey::Justify => t::linux_reader_justify(),
        other => other.name().to_owned(),
    }
}

fn choice_label(name: &str) -> String {
    match name {
        "rtl" => t::reader_mode_rtl(),
        "ltr" => t::reader_mode_ltr(),
        "strip" => t::reader_mode_strip(),
        "paged" => t::linux_reader_mode_paged(),
        "scrolled" | "vertical" => t::linux_reader_mode_scrolled(),
        "auto" => t::reader_layout_auto(),
        "single" => t::reader_layout_single(),
        "double" => t::reader_layout_double(),
        "screen" => t::reader_fit_screen(),
        "width" => t::reader_fit_width(),
        "height" => t::linux_reader_fit_height(),
        "split" => t::reader_spreads_split(),
        "rotate" => t::reader_spreads_rotate(),
        "whole" => t::reader_spreads_whole(),
        "none" => t::reader_gap_none(),
        "small" => t::reader_gap_small(),
        "serif" => t::reader_font_serif(),
        "sans" => t::reader_font_sans(),
        "system" => t::reader_look_system(),
        "paper" => t::reader_look_paper(),
        "light" => t::reader_look_light(),
        "sepia" => t::reader_look_sepia(),
        "dark" => t::reader_look_dark(),
        "theme" => t::reader_background_theme(),
        "black" => t::reader_background_black(),
        "white" => t::reader_background_white(),
        other => other.to_owned(),
    }
}

/// Whether a look puts the chrome on the light tokens.
fn chrome_is_light(look: Option<&str>) -> bool {
    matches!(look, Some("paper" | "light" | "sepia"))
}

/// How far down the strip a place sits: the images above `index` with their gaps, and `offset`
/// of that image's height into it.
fn strip_target(heights: &[i32], spacing: i32, index: usize, offset: Option<f32>) -> f64 {
    let above: i32 = heights.iter().take(index).map(|h| h + spacing).sum();
    let into = offset
        .zip(heights.get(index))
        .map_or(0.0, |(o, h)| f64::from(o.clamp(0.0, 1.0)) * f64::from(*h));
    // Whole pixels: the offset comes as an f32, and 0.35 of an image is not 0.35 in f64.
    (f64::from(above) + into).round()
}

/// Each strip leaf's pages, to tell a new layout of the same images from new images.
/// Where a strip opens on its first layout. Opened gives a strip start whenever the place has a
/// page, at the top of that page's image when the place was kept in paged mode, and the engine's
/// "Resumed at image N" names that image, so the strip opens there. Without one, the frame that
/// holds the start page; without either, the top.
fn strip_opening(leaves: &[Leaf], strip: Option<StripPos>, page: Option<u32>) -> StripPos {
    strip.unwrap_or_else(|| StripPos {
        image: page
            .and_then(|p| leaves.iter().position(|l| l.parts.iter().any(|x| x.page == p)))
            .unwrap_or(0) as u32,
        offset: 0.0,
    })
}

fn strip_leaves(leaves: &[Leaf]) -> Vec<Vec<u32>> {
    leaves
        .iter()
        .filter(|l| !l.end_page)
        .map(|l| l.parts.iter().map(|p| p.page).collect())
        .collect()
}

/// The leaves to ask for around leaf `index` of `count`, in the order the engine fetches them:
/// the one wanted first, then the two after it, then the one before. Asked in reading order, a
/// jump far into a chapter on a slow source waited for the page before it.
fn wanted_leaves(index: usize, count: usize) -> Vec<usize> {
    let ahead = (index..(index + 3).min(count)).collect::<Vec<_>>();
    let behind = index.checked_sub(1).filter(|i| *i < count);
    ahead.into_iter().chain(behind).collect()
}

/// The strip images to ask for with the viewport from `top` to `bottom`, given each image's
/// height and the gap between them, and where the foot starts. The images in view come first,
/// then those up to `reach` below it, then those up to `reach` above it, nearest first: a jump
/// far into a long strip fetches what it lands on before the images it passed.
fn strip_wanted(
    heights: &[i32],
    spacing: i32,
    top: f64,
    bottom: f64,
    reach: f64,
) -> (Vec<usize>, f64) {
    let (mut seen, mut ahead, mut behind) = (Vec::new(), Vec::new(), Vec::new());
    let mut y = 0.0;
    for (i, h) in heights.iter().enumerate() {
        let h = f64::from(*h);
        if y + h >= top && y <= bottom {
            seen.push(i);
        } else if y > bottom && y <= bottom + reach {
            ahead.push(i);
        } else if y + h < top && y + h >= top - reach {
            behind.push(i);
        }
        y += h + f64::from(spacing);
    }
    behind.reverse();
    seen.extend(ahead);
    seen.extend(behind);
    (seen, y - f64::from(spacing))
}

/// A strip image's place: the picture, and over it the plate while its file is on its way, as a
/// page waits in the paged reader: nothing for 600 ms, then the plate with the image's number,
/// and a line under it once it has been asked for 4 s ([`strip_frame_asked`]). The words sit in
/// the middle of the part of the image on screen ([`strip_frame_seen`]). A jump far into a long
/// chapter lands among plates, never on an empty window.
fn strip_frame(page: u32) -> (gtk::Overlay, gtk::Picture) {
    let picture = gtk::Picture::new();
    picture.set_content_fit(gtk::ContentFit::Fill);
    picture.set_can_shrink(true);
    picture.add_css_class("strip-leaf");
    let number = gtk::Label::new(Some(&t::reader_image_plate(i64::from(page + 1))));
    number.add_css_class("reader-plate-number");
    number.add_css_class("numeric");
    let line = gtk::Label::new(Some(&t::reader_image_loading(i64::from(page + 1))));
    line.add_css_class("reader-plate-line");
    line.set_wrap(true);
    line.set_justify(gtk::Justification::Center);
    line.set_visible(false);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 6);
    words.set_valign(gtk::Align::Center);
    words.set_vexpand(true);
    words.set_margin_start(24);
    words.set_margin_end(24);
    words.append(&number);
    words.append(&line);
    let plate = gtk::Box::new(gtk::Orientation::Vertical, 0);
    plate.add_css_class("reader-plate");
    plate.set_accessible_role(gtk::AccessibleRole::Presentation);
    plate.set_can_target(false);
    plate.append(&words);
    plate.set_visible(false);
    let frame = gtk::Overlay::new();
    frame.set_child(Some(&picture));
    frame.add_overlay(&plate);
    let (weak_plate, weak_picture) = (plate.downgrade(), picture.downgrade());
    glib::timeout_add_local_once(SILENT, move || {
        if let (Some(plate), Some(picture)) = (weak_plate.upgrade(), weak_picture.upgrade())
            && picture.paintable().is_none()
        {
            plate.set_visible(true);
            motion::fade_in(&plate);
        }
    });
    (frame, picture)
}

/// Centre a strip plate's words on the part of its image on screen, `seen` as the top and bottom
/// of that part in the image's own pixels: margins above and below the words leave that part
/// for them, kept inside the image and never narrower than the words with their line.
fn strip_frame_seen(picture: &gtk::Picture, seen: Option<(f64, f64)>) {
    let Some(words) = strip_plate(picture).and_then(|plate| plate.first_child()) else {
        return;
    };
    let number =
        words.first_child().map_or(24, |n| n.measure(gtk::Orientation::Vertical, -1).1);
    // Room for the number and its line, or more when a failed image's Try again is under them.
    let room =
        (number * 2 + words_gap(&words)).max(words.measure(gtk::Orientation::Vertical, -1).1);
    let (above, below) = strip_words_margins(picture.height_request().max(0), seen, room);
    if words.margin_top() != above {
        words.set_margin_top(above);
    }
    if words.margin_bottom() != below {
        words.set_margin_bottom(below);
    }
}

/// The gap between a plate's number and its line.
fn words_gap(words: &gtk::Widget) -> i32 {
    words.downcast_ref::<gtk::Box>().map_or(6, gtk::Box::spacing)
}

/// The margins above and below a strip plate's words in an image `height` tall, so they centre
/// on the part on screen, `seen` in the image's pixels, while that part leaves them `room`; a
/// thinner part widens to `room` about its middle, inside the image. Off screen, none.
fn strip_words_margins(height: i32, seen: Option<(f64, f64)>, room: i32) -> (i32, i32) {
    let full = f64::from(height);
    let Some((top, bottom)) =
        seen.map(|(t, b)| (t.max(0.0), b.min(full))).filter(|(t, b)| b > t)
    else {
        return (0, 0);
    };
    let (top, bottom) = (top.round() as i32, bottom.round() as i32);
    if bottom - top >= room {
        return (top, height - bottom);
    }
    let above = ((top + bottom) / 2 - room / 2).clamp(0, (height - room).max(0));
    (above, (height - above - room).max(0))
}

/// The plate over a strip picture, made by [`strip_frame`].
fn strip_plate(picture: &gtk::Picture) -> Option<gtk::Widget> {
    picture.next_sibling()
}

/// A widget's size and scale while it is on screen, or else those of the nearest enclosing
/// widget that is. The reader fills the page it covers, so before its first layout, and while
/// it is hidden (its last size may be old), that page's size is the reader's.
fn surface_size(widget: &gtk::Widget) -> Option<(i32, i32, i32)> {
    std::iter::successors(Some(widget.clone()), gtk::Widget::parent)
        .find(|w| w.is_mapped() && w.width() > 0 && w.height() > 0)
        .map(|w| (w.width(), w.height(), w.scale_factor()))
}

/// The line under a plate's number: "Loading image 12", or that it failed.
fn plate_line(words: &gtk::Widget) -> Option<gtk::Label> {
    words.first_child()?.next_sibling().and_downcast()
}

/// A strip image that failed: its plate shows with "Couldn't load" and Try again, and
/// takes clicks while it does.
fn strip_frame_failed(picture: &gtk::Picture, retry: impl Fn() + 'static) {
    let Some(plate) = strip_plate(picture) else { return };
    let Some(words) = plate.first_child().and_downcast::<gtk::Box>() else { return };
    if picture.paintable().is_some() || plate.has_css_class("strip-failed") {
        return;
    }
    plate.add_css_class("strip-failed");
    plate.set_can_target(true);
    if let Some(line) = plate_line(words.upcast_ref()) {
        line.set_label(&t::reader_image_failed());
        line.set_opacity(1.0);
        line.set_visible(true);
    }
    let again = gtk::Button::with_label(&t::reader_try_again());
    again.add_css_class("pill");
    again.set_halign(gtk::Align::Center);
    again.set_margin_top(6);
    again.connect_clicked(move |_| retry());
    words.append(&again);
    if !plate.is_visible() {
        plate.set_opacity(1.0);
        plate.set_visible(true);
        motion::fade_in(&plate);
    }
}

/// Try again on a failed strip image: its plate says it is loading again.
fn strip_frame_retried(picture: &gtk::Picture, page: u32) {
    let Some(plate) = strip_plate(picture) else { return };
    if !plate.has_css_class("strip-failed") {
        return;
    }
    plate.remove_css_class("strip-failed");
    plate.set_can_target(false);
    let Some(words) = plate.first_child().and_downcast::<gtk::Box>() else { return };
    let line = plate_line(words.upcast_ref());
    if let Some(again) = line.as_ref().and_then(gtk::Label::next_sibling) {
        words.remove(&again);
    }
    if let Some(line) = line {
        line.set_label(&t::reader_image_loading(i64::from(page + 1)));
    }
}

/// A strip picture's page was asked for: if its file has not come 4 s later, its plate says it
/// is loading. Only the first ask counts.
fn strip_frame_asked(picture: &gtk::Picture) {
    let Some(plate) = strip_plate(picture) else { return };
    if picture.paintable().is_some() || plate.has_css_class("strip-asked") {
        return;
    }
    plate.add_css_class("strip-asked");
    let line = plate.first_child().as_ref().and_then(plate_line);
    let (Some(line), picture) = (line.map(|l| l.downgrade()), picture.downgrade()) else {
        return;
    };
    glib::timeout_add_local_once(SAY_LOADING, move || {
        if let (Some(line), Some(picture)) = (line.upgrade(), picture.upgrade())
            && picture.paintable().is_none()
        {
            line.set_visible(true);
            motion::fade_in(&line);
        }
    });
}

/// The strip image's file landed: it takes the place, crossfading over its plate when the plate
/// shows, fading in when nothing did yet. `redrawn` swaps a drawn picture in place, as after
/// crop borders changed, without a fade.
fn fill_strip_frame(
    picture: &gtk::Picture,
    paintable: &impl IsA<gdk::Paintable>,
    redrawn: bool,
) {
    if picture.paintable().is_some() {
        if redrawn {
            picture.set_paintable(Some(paintable));
        }
        return;
    }
    picture.set_paintable(Some(paintable));
    let plate = strip_plate(picture).filter(|plate| plate.is_visible());
    match plate {
        Some(plate) => {
            motion::fade_out(&plate);
        }
        None => {
            motion::fade_in(picture);
        }
    }
}

/// How far over the window's foot a novel's notice sits: clear of the group while the chrome
/// shows, which needs the group shown to be measured, else in the text's bottom margin.
fn notice_lift(group: &gtk::Box, chrome: bool) -> i32 {
    if !chrome {
        return 44;
    }
    // As wide as it is laid out, or will be: a narrow window wraps what is in it.
    let width = if group.width() > 0 { group.width() } else { group.width_request() };
    let (_, height, _, _) = group.measure(gtk::Orientation::Vertical, width.max(-1));
    group.margin_bottom() + height + 12
}

/// Where a popover `width` wide centres to sit over `middle`, the middle of its button, while
/// staying 8 px inside a window `window` wide; in the middle of a window it can't fit in.
fn popover_middle(middle: f64, width: f64, window: f64) -> f64 {
    let half = width / 2.0;
    if window <= 0.0 {
        return middle;
    }
    if width + 16.0 >= window {
        return window / 2.0;
    }
    middle.clamp(8.0 + half, window - 8.0 - half)
}

/// Which image `top` falls in, and how far down it, from the images' heights and the gap.
fn strip_place_of(heights: &[i32], spacing: i32, top: f64) -> (usize, f32) {
    let mut y = 0.0;
    for (i, h) in heights.iter().enumerate() {
        let h = f64::from(*h).max(1.0);
        if top < y + h || i + 1 == heights.len() {
            return (i, ((top - y) / h).clamp(0.0, 1.0) as f32);
        }
        y += h + f64::from(spacing);
    }
    (0, 0.0)
}

/// The page a new layout keeps of `leaf`, the leaf on screen: `held`, the page the last layout
/// kept, while the leaf still holds it, as a spread holds a page shown alone before it;
/// otherwise the leaf's first.
fn page_kept(leaf: Option<&Leaf>, held: Option<u32>) -> Option<u32> {
    let leaf = leaf?;
    held.filter(|held| leaf.parts.iter().any(|p| p.page == *held))
        .or_else(|| leaf.parts.first().map(|p| p.page))
}

/// The mode and direction the open book takes from `settings`, when they differ from `now`.
/// A mode or direction changed in the reader lays the chapter out again without opening it
/// anew, so the reader follows the settings, as the engine's plan reads them: settings for
/// another work or medium, and a novel's, leave it as it is.
fn followed(
    medium: Medium,
    work: Option<&WorkId>,
    now: (ReaderMode, ReadDirection),
    settings: &ReaderSettingsView,
) -> Option<(ReaderMode, ReadDirection)> {
    if medium == Medium::Novel || settings.medium != medium || work != settings.work.as_ref() {
        return None;
    }
    let word = |key: ReaderSettingKey| {
        settings.rows.iter().find(|r| r.key == key).and_then(|r| match &r.value {
            ReaderValue::Choice { name } => Some(name.as_str()),
            _ => None,
        })
    };
    let mode = match word(ReaderSettingKey::Mode) {
        Some("strip") => ReaderMode::Strip,
        Some("vertical") => ReaderMode::Vertical,
        Some("paged") => ReaderMode::Paged,
        _ => now.0,
    };
    let direction = if mode.flows() {
        ReadDirection::TopToBottom
    } else {
        match word(ReaderSettingKey::Direction) {
            Some("ltr") => ReadDirection::LeftToRight,
            Some("rtl") => ReadDirection::RightToLeft,
            _ if now.1 == ReadDirection::TopToBottom => ReadDirection::RightToLeft,
            _ => now.1,
        }
    };
    ((mode, direction) != now).then_some((mode, direction))
}

/// The stops Tab walks in the reader, in order: the head's controls, the page, the group's.
/// Hidden, insensitive and unfocusable controls are passed over, and popovers walk their own.
fn tab_stops(head: &gtk::Widget, page: &gtk::Widget, group: &gtk::Widget) -> Vec<gtk::Widget> {
    fn walk(widget: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
        if !widget.is_visible()
            || !widget.is_sensitive()
            || !widget.can_focus()
            || widget.is::<gtk::Popover>()
        {
            return;
        }
        if widget.is_focusable() {
            out.push(widget.clone());
            return;
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            walk(&c, out);
            child = c.next_sibling();
        }
    }
    let mut stops = Vec::new();
    walk(head, &mut stops);
    stops.push(page.clone());
    walk(group, &mut stops);
    stops
}

/// Where Tab, or Shift+Tab with `back`, goes from `focus`: the stop after (or before) the one
/// that holds it, round the ends. Focus on none of the controls is on the page.
fn next_stop(
    stops: &[gtk::Widget],
    page: &gtk::Widget,
    focus: Option<&gtk::Widget>,
    back: bool,
) -> Option<gtk::Widget> {
    let held = |stop: &gtk::Widget| {
        stop != page && focus.is_some_and(|f| f == stop || f.is_ancestor(stop))
    };
    let at = stops.iter().position(held).or_else(|| stops.iter().position(|s| s == page))?;
    let n = stops.len();
    let next = if back { (at + n - 1) % n } else { (at + 1) % n };
    stops.get(next).cloned()
}

/// Tab and Shift+Tab, without Ctrl or Alt, for the reader's own walk: `walk` gets whether the
/// walk goes back and says whether it moved the focus. They are heard before the page's content,
/// as a novel's web view would keep them, walking its own document and the text view inside it.
fn tab_keys(walk: impl Fn(bool) -> bool + 'static) -> gtk::EventControllerKey {
    let tabs = gtk::EventControllerKey::new();
    tabs.set_propagation_phase(gtk::PropagationPhase::Capture);
    tabs.connect_key_pressed(move |_, key, _, modifiers| {
        use gdk::Key as K;
        let tab = matches!(key, K::Tab | K::KP_Tab | K::ISO_Left_Tab);
        let other = gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK;
        if !tab || modifiers.intersects(other) {
            return glib::Propagation::Proceed;
        }
        let back = key == K::ISO_Left_Tab || modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        if walk(back) { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    });
    tabs
}

/// Go to page's field: Return commits what was typed and `go` gets the page, from 1.
fn go_on_return(field: &gtk::SpinButton, go: impl Fn(usize) + 'static) {
    field.connect_activate(move |field| {
        field.update();
        go(field.value().round() as usize);
    });
}

/// The window's full screen from before the reader first changed it, until the reader closes.
#[derive(Default)]
struct FullBefore(Cell<Option<bool>>);

impl FullBefore {
    /// The reader is about to change full screen, which is `now`.
    fn toggled(&self, now: bool) {
        if self.0.get().is_none() {
            self.0.set(Some(now));
        }
    }

    /// The reader closes with the window's full screen at `now`: what to set it to, if anything.
    fn give_back(&self, now: bool) -> Option<bool> {
        self.0.take().filter(|&before| before != now)
    }
}

#[cfg(test)]
mod layout_tests {
    use kaeri_core::{books::WorkId, media::Medium};
    use kaeri_model::reader::{
        Leaf, LeafPart, PageKind, ReadDirection, ReaderMode, ReaderSettingKey,
        ReaderSettingRow, ReaderSettingsView, ReaderValue, SettingScope, UnitRect,
    };

    use super::{StripPos, followed, page_kept, strip_opening};

    fn leaf(pages: &[u32]) -> Leaf {
        let parts = pages
            .iter()
            .map(|&page| LeafPart {
                page,
                kind: PageKind::Story,
                file: None,
                place: UnitRect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                rotate: false,
                half: None,
                ratio: None,
                strip: None,
            })
            .collect();
        Leaf { index: 0, parts, end_page: false }
    }

    #[test]
    fn a_round_trip_through_spreads_keeps_the_second_page_of_the_spread() {
        // Page 3 alone, then the spread of 2 and 3, then 3 alone again.
        let held = page_kept(Some(&leaf(&[3])), None);
        assert_eq!(held, Some(3));
        assert_eq!(page_kept(Some(&leaf(&[2, 3])), held), Some(3));
        // A spread turned to, with nothing held, gives its first page.
        assert_eq!(page_kept(Some(&leaf(&[4, 5])), held), Some(4));
        assert_eq!(page_kept(Some(&leaf(&[4, 5])), None), Some(4));
        assert_eq!(page_kept(None, held), None);
    }

    fn settings(medium: Medium, work: &str, mode: &str, direction: &str) -> ReaderSettingsView {
        let row = |key, name: &str| ReaderSettingRow {
            key,
            value: ReaderValue::Choice { name: name.to_owned() },
            scope: SettingScope::Series,
            detected: false,
            overridden: true,
            choices: Vec::new(),
        };
        ReaderSettingsView {
            medium,
            work: Some(WorkId(work.to_owned())),
            rows: vec![
                row(ReaderSettingKey::Mode, mode),
                row(ReaderSettingKey::Direction, direction),
            ],
            tap_zones: None,
        }
    }

    /// Strip picked in the open chapter's settings: the reader turns to the strip, and back
    /// to pages, with the direction the settings give, as the engine lays the chapter out.
    #[test]
    fn the_open_book_follows_a_mode_picked_in_the_reader() {
        let work = WorkId("a:1".to_owned());
        let paged = (ReaderMode::Paged, ReadDirection::RightToLeft);
        let strip = (ReaderMode::Strip, ReadDirection::TopToBottom);
        let manga = |mode, direction| settings(Medium::Manga, "a:1", mode, direction);
        assert_eq!(
            followed(Medium::Manga, Some(&work), paged, &manga("strip", "rtl")),
            Some(strip)
        );
        assert_eq!(
            followed(Medium::Manga, Some(&work), strip, &manga("paged", "ltr")),
            Some((ReaderMode::Paged, ReadDirection::LeftToRight))
        );
        assert_eq!(
            followed(Medium::Manga, Some(&work), paged, &manga("paged", "ltr")),
            Some((ReaderMode::Paged, ReadDirection::LeftToRight)),
            "a direction alone turns the pages the other way"
        );
        assert_eq!(
            followed(Medium::Manga, Some(&work), paged, &manga("vertical", "rtl")),
            Some((ReaderMode::Vertical, ReadDirection::TopToBottom))
        );
        // Nothing changed, or settings that are not the open book's: the reader stays.
        assert_eq!(followed(Medium::Manga, Some(&work), paged, &manga("paged", "rtl")), None);
        assert_eq!(followed(Medium::Manga, Some(&work), strip, &manga("strip", "ltr")), None);
        let other = settings(Medium::Manga, "a:2", "strip", "rtl");
        assert_eq!(followed(Medium::Manga, Some(&work), paged, &other), None);
        let defaults = ReaderSettingsView { work: None, ..manga("strip", "rtl") };
        assert_eq!(followed(Medium::Manga, Some(&work), paged, &defaults), None);
        let novel = settings(Medium::Novel, "a:1", "strip", "rtl");
        assert_eq!(followed(Medium::Novel, Some(&work), paged, &novel), None);
    }

    /// P18. A place kept in paged mode reaches a strip as a strip start at the top of its
    /// page's image: the strip opens on the frame of that image, the one "Resumed at image N"
    /// names (the engine says N for page N - 1), and not at the first.
    #[test]
    fn a_strip_opens_at_the_start_the_engine_gives() {
        let leaves: Vec<Leaf> = (0..9).map(|p| leaf(&[p])).collect();
        let kept_in_pages = StripPos { image: 6, offset: 0.0 };
        let at = strip_opening(&leaves, Some(kept_in_pages), Some(6));
        assert_eq!(at, kept_in_pages);
        let shown = leaves[at.image as usize].parts[0].page;
        assert_eq!(i64::from(shown) + 1, 7, "Resumed at image 7 is the frame of page 6");

        // A place kept in a strip keeps how far into its image it was.
        let in_strip = StripPos { image: 3, offset: 0.4 };
        assert_eq!(strip_opening(&leaves, Some(in_strip), Some(3)), in_strip);

        // No strip start: the frame of the start page, else the top.
        assert_eq!(strip_opening(&leaves, None, Some(5)), StripPos { image: 5, offset: 0.0 });
        assert_eq!(strip_opening(&leaves, None, None), StripPos { image: 0, offset: 0.0 });
    }
}

#[cfg(test)]
mod strip_tests {
    use super::strip_target;

    #[test]
    fn the_offset_counts_on_the_first_image() {
        assert_eq!(strip_target(&[1000, 800], 8, 0, Some(0.35)), 350.0);
        assert_eq!(strip_target(&[1000, 800], 8, 0, None), 0.0);
    }

    #[test]
    fn the_offset_counts_on_a_later_image() {
        // Two images and their gaps above, then half of the third.
        assert_eq!(strip_target(&[1000, 800, 600], 8, 2, Some(0.5)), 1816.0 + 300.0);
        assert_eq!(strip_target(&[1000, 800, 600], 0, 1, None), 1000.0);
    }

    #[test]
    fn light_looks_put_the_chrome_on_light_tokens() {
        use super::chrome_is_light;
        for look in ["paper", "light", "sepia"] {
            assert!(chrome_is_light(Some(look)), "{look}");
        }
        for look in [Some("dark"), Some("system"), None] {
            assert!(!chrome_is_light(look), "{look:?}");
        }
    }

    #[test]
    fn an_offset_out_of_range_stays_in_its_image() {
        assert_eq!(strip_target(&[1000], 0, 0, Some(1.7)), 1000.0);
        assert_eq!(strip_target(&[1000], 0, 0, Some(-0.2)), 0.0);
        assert_eq!(strip_target(&[], 0, 3, Some(0.5)), 0.0);
    }

    #[test]
    fn a_place_is_found_from_the_heights() {
        use super::strip_place_of;
        let heights = [1000, 800, 600];
        assert_eq!(strip_place_of(&heights, 8, 0.0), (0, 0.0));
        assert_eq!(strip_place_of(&heights, 8, 350.0), (0, 0.35));
        // Two images and their gaps above (1000 + 8 + 800 + 8), then half of the third.
        let (image, offset) = strip_place_of(&heights, 8, 1816.0 + 300.0);
        assert_eq!(image, 2);
        assert!((offset - 0.5).abs() < 1e-6);
        // Back and forth: the target of a place is that place again.
        let target = strip_target(&heights, 8, 1, Some(0.25));
        let (image, offset) = strip_place_of(&heights, 8, target);
        assert_eq!(image, 1);
        assert!((offset - 0.25).abs() < 1e-3);
    }
}

#[cfg(test)]
mod tab_tests {
    use adw::prelude::*;

    use super::{next_stop, tab_keys, tab_stops};
    use crate::gtk_test::{pump, run};

    /// The reader's shape: a page with the head and the group over it, the group's last row
    /// holding Chapters, the names, Aa and Full screen, and Go to page in a popover.
    #[test]
    fn tab_walks_the_head_the_page_and_the_whole_group_both_ways() {
        run(|| {
            let close = gtk::Button::with_label("Close");
            let head = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            head.append(&close);
            head.append(&gtk::Label::new(Some("Chapter 3")));
            let scrub = gtk::Scale::with_range(gtk::Orientation::Horizontal, 1.0, 9.0, 1.0);
            let field = gtk::SpinButton::with_range(1.0, 9.0, 1.0);
            let go_to = gtk::Popover::new();
            go_to.set_child(Some(&field));
            go_to.set_parent(&scrub);
            let back_to = gtk::Button::with_label("Back to page 4");
            back_to.set_visible(false);
            let chapters = gtk::MenuButton::new();
            // A menu button is only sensitive with something to open, as the reader's is.
            chapters.set_popover(Some(&gtk::Popover::new()));
            let aa = gtk::Button::with_label("Aa");
            let full = gtk::Button::with_label("Full screen");
            let off = gtk::Button::with_label("Off");
            off.set_sensitive(false);
            let ends = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            ends.append(&aa);
            ends.append(&full);
            ends.append(&off);
            let row = gtk::CenterBox::new();
            row.set_start_widget(Some(&chapters));
            row.set_center_widget(Some(&gtk::Label::new(Some("A Made-up Title"))));
            row.set_end_widget(Some(&ends));
            let group = gtk::Box::new(gtk::Orientation::Vertical, 0);
            group.append(&back_to);
            group.append(&scrub);
            group.append(&row);
            group.set_valign(gtk::Align::End);
            head.set_valign(gtk::Align::Start);
            let page = gtk::Overlay::new();
            page.set_child(Some(&gtk::Label::new(Some("page 1"))));
            page.add_overlay(&head);
            page.add_overlay(&group);
            page.set_focusable(true);
            let window = gtk::Window::builder().default_width(600).default_height(400).build();
            window.set_child(Some(&page));
            window.present();
            pump(100);

            let page_w = page.upcast_ref::<gtk::Widget>();
            let stops = tab_stops(head.upcast_ref(), page_w, group.upcast_ref());
            let known: [(&gtk::Widget, &str); 6] = [
                (close.upcast_ref(), "Close"),
                (page_w, "page"),
                (scrub.upcast_ref(), "scrubber"),
                (chapters.upcast_ref(), "Chapters"),
                (aa.upcast_ref(), "Aa"),
                (full.upcast_ref(), "Full screen"),
            ];
            let names: Vec<&str> = stops
                .iter()
                .map(|w| {
                    known
                        .iter()
                        .find(|(k, _)| w == *k || (*k != page_w && w.is_ancestor(*k)))
                        .map_or("other", |(_, name)| *name)
                })
                .collect();
            assert_eq!(
                names,
                ["Close", "page", "scrubber", "Chapters", "Aa", "Full screen"],
                "the hidden, the insensitive and the popover's field are passed over"
            );

            // Forward from the page, all the way round.
            let mut at: Option<gtk::Widget> = Some(page_w.clone());
            let mut seen = Vec::new();
            for _ in 0..stops.len() {
                let next = next_stop(&stops, page_w, at.as_ref(), false).expect("a stop");
                next.grab_focus();
                pump(10);
                at = gtk::prelude::GtkWindowExt::focus(&window);
                let focus = at.as_ref().expect("the focus");
                seen.push(
                    stops
                        .iter()
                        .position(|s| focus == s || (s != page_w && focus.is_ancestor(s))),
                );
            }
            assert_eq!(seen, [Some(2), Some(3), Some(4), Some(5), Some(0), Some(1)]);

            // Back from Close: round past the start to Full screen.
            let back = next_stop(&stops, page_w, Some(close.upcast_ref()), true);
            assert_eq!(back.as_ref(), Some(full.upcast_ref()));
            // From Full screen on: Close, never stuck between Chapters and Aa.
            let on = next_stop(&stops, page_w, Some(full.upcast_ref()), false);
            assert_eq!(on.as_ref(), Some(close.upcast_ref()));
            // From the head to the page, from the page to the group.
            let after_close = next_stop(&stops, page_w, Some(close.upcast_ref()), false);
            assert_eq!(after_close.as_ref(), Some(page_w));
            // Focus nowhere counts as on the page.
            assert_eq!(
                next_stop(&stops, page_w, None, false).as_ref(),
                Some(scrub.upcast_ref())
            );
            // Focus inside the page's content, as in a novel's web view and the text view inside
            // it, counts as on the page: Tab leaves it for the group, Shift+Tab for the head.
            let inner = gtk::TextView::new();
            page.add_overlay(&inner);
            let inner_w = inner.upcast_ref::<gtk::Widget>();
            assert_eq!(
                next_stop(&stops, page_w, Some(inner_w), false).as_ref(),
                Some(scrub.upcast_ref())
            );
            assert_eq!(
                next_stop(&stops, page_w, Some(inner_w), true).as_ref(),
                Some(close.upcast_ref())
            );
            go_to.unparent();
            window.destroy();
        });
    }

    /// A novel's web view keeps every key that reaches it, so the reader hears Tab on its way
    /// down to the page's content, before the view does.
    #[test]
    fn tab_is_heard_before_the_pages_content() {
        run(|| {
            let tabs = tab_keys(|_| true);
            assert_eq!(tabs.propagation_phase(), gtk::PropagationPhase::Capture);
        });
    }
}

#[cfg(test)]
mod full_tests {
    use super::FullBefore;

    #[test]
    fn closing_leaves_the_full_screen_the_reader_entered() {
        let full = FullBefore::default();
        full.toggled(false);
        assert_eq!(full.give_back(true), Some(false));
        // Closed once, the next close changes nothing.
        assert_eq!(full.give_back(true), None);
    }

    #[test]
    fn closing_keeps_the_window_as_it_was_when_the_reader_did_not_change_it() {
        let full = FullBefore::default();
        assert_eq!(full.give_back(false), None);
        assert_eq!(full.give_back(true), None);
        // Into full screen and out again: nothing to undo.
        full.toggled(false);
        full.toggled(true);
        assert_eq!(full.give_back(false), None);
    }

    #[test]
    fn a_window_full_screen_before_the_reader_gets_it_back() {
        let full = FullBefore::default();
        full.toggled(true);
        assert_eq!(full.give_back(false), Some(true));
    }
}

#[cfg(test)]
mod plate_tests {
    use adw::prelude::*;
    use gtk::{gdk, glib};

    use super::{
        fill_strip_frame, load_texture, notice_lift, popover_middle, strip_frame,
        strip_frame_asked, strip_frame_failed, strip_frame_retried, strip_frame_seen,
        strip_words_margins, surface_size,
    };
    use crate::{
        gtk_test::{pump, run},
        motion::Fader,
    };

    fn texture() -> gdk::Texture {
        gdk::MemoryTexture::new(
            1,
            1,
            gdk::MemoryFormat::R8g8b8,
            &glib::Bytes::from_owned(vec![0u8, 0, 0]),
            3,
        )
        .upcast()
    }

    fn words(widget: &gtk::Widget, out: &mut Vec<String>) {
        if let Some(label) = widget.downcast_ref::<gtk::Label>()
            && label.is_visible()
        {
            out.push(label.label().to_string());
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            words(&c, out);
            child = c.next_sibling();
        }
    }

    fn page_file(
        path: &std::path::Path,
        encoding: kaeri_model::reader::PageEncoding,
    ) -> kaeri_model::reader::PageFile {
        kaeri_model::reader::PageFile {
            path: path.to_string_lossy().into_owned(),
            encoding,
            width: 3,
            height: 2,
            crop: None,
            colour: true,
            bytes: 0,
        }
    }

    /// A WebP page decodes through the image crate where GTK's loaders can't, and a file
    /// nothing can decode, or a raw page shorter than its size, gives no texture to draw.
    #[test]
    fn a_page_gtk_cannot_decode_falls_back_or_fails() {
        use kaeri_model::reader::{PageEncoding, PixelFormat};
        run(|| {
            let dir = tempfile::tempdir().expect("a temporary folder");
            let webp = dir.path().join("page.webp");
            image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]))
                .save_with_format(&webp, image::ImageFormat::WebP)
                .expect("a WebP page");
            let original = || PageEncoding::Original { mime: "image/webp".into() };
            let texture = load_texture(&page_file(&webp, original())).expect("decoded");
            assert_eq!((texture.width(), texture.height()), (3, 2));
            assert!(super::decode_texture(&webp).is_some(), "the image crate decodes WebP");

            let broken = dir.path().join("page.bin");
            std::fs::write(&broken, b"not a picture at all").expect("a broken page");
            assert!(load_texture(&page_file(&broken, original())).is_none());

            let raw = dir.path().join("page.raw");
            std::fs::write(&raw, [0u8; 7]).expect("a short raw page");
            let rgba = PageEncoding::Raw { format: PixelFormat::Rgba8, stride: 12 };
            assert!(
                load_texture(&page_file(&raw, rgba.clone())).is_none(),
                "short of its size"
            );
            std::fs::write(&raw, [0u8; 24]).expect("a whole raw page");
            assert!(load_texture(&page_file(&raw, rgba)).is_some());
        });
    }

    /// The first open in a run goes out before the reader has been laid out. It is prepared
    /// for the page the reader covers, which is the size the reader then gets, so a portrait
    /// window is not laid out as a landscape one with two pages side by side.
    #[test]
    fn the_first_open_is_prepared_for_the_window_it_shows_in() {
        run(|| {
            assert_eq!(surface_size(gtk::Box::default().upcast_ref()), None);
            let window = gtk::Window::builder().default_width(360).default_height(720).build();
            let top = gtk::Stack::new();
            top.set_hhomogeneous(false);
            top.set_vhomogeneous(false);
            let main = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let reader = gtk::Box::new(gtk::Orientation::Vertical, 0);
            top.add_named(&main, Some("main"));
            top.add_named(&reader, Some("reader"));
            window.set_child(Some(&top));
            window.present();
            pump(300);
            let (width, height, _) =
                surface_size(reader.upcast_ref()).expect("a size before the reader's layout");
            assert!(height > width, "a portrait window gives a portrait surface");
            top.set_visible_child_name("reader");
            pump(300);
            assert_eq!(
                (reader.width(), reader.height()),
                (width, height),
                "the reader's first layout has the size its open was prepared for"
            );
            window.destroy();
        });
    }

    /// A strip image that failed says so on its plate with Try again, which asks again and
    /// puts the plate back to loading; the image then crossfades over it as usual.
    #[test]
    fn a_failed_strip_image_offers_try_again() {
        run(|| {
            let (frame, picture) = strip_frame(11);
            picture.set_size_request(200, 280);
            let window = gtk::Window::builder().default_width(400).default_height(400).build();
            window.set_child(Some(&frame));
            window.present();
            let plate = picture.next_sibling().expect("a plate over the picture");
            let asked = std::rc::Rc::new(std::cell::Cell::new(0));
            strip_frame_failed(&picture, {
                let asked = asked.clone();
                move || asked.set(asked.get() + 1)
            });
            pump(400);
            assert!(
                plate.is_visible() && plate.can_target(),
                "the plate shows and takes clicks"
            );
            let mut said = Vec::new();
            words(&plate, &mut said);
            assert_eq!(said, ["Image 12", "Couldn't load", "Try again"]);
            let again = plate
                .first_child()
                .and_then(|w| w.last_child())
                .and_downcast::<gtk::Button>()
                .expect("Try again");
            again.emit_clicked();
            assert_eq!(asked.get(), 1);
            strip_frame_retried(&picture, 11);
            let mut said = Vec::new();
            words(&plate, &mut said);
            assert_eq!(said, ["Image 12", "Loading image 12"]);
            assert!(!plate.can_target());
            fill_strip_frame(&picture, &texture(), false);
            pump(500);
            assert!(!plate.is_visible(), "the plate is gone once the image is in");
            window.destroy();
        });
    }

    /// A strip image whose file is still coming shows its plate with its number from 600 ms,
    /// adds a line under it 4 s after it was asked for, and its file crossfades over the plate.
    #[test]
    fn a_strip_image_on_its_way_holds_a_plate() {
        run(|| {
            let (frame, picture) = strip_frame(11);
            picture.set_size_request(200, 280);
            let window = gtk::Window::builder().default_width(400).default_height(400).build();
            window.set_child(Some(&frame));
            window.present();
            let plate = picture.next_sibling().expect("a plate over the picture");
            pump(200);
            assert!(!plate.is_visible(), "nothing shows in the first 600 ms");
            pump(600);
            assert!(plate.is_visible(), "the plate shows from 600 ms");
            let mut said = Vec::new();
            words(&plate, &mut said);
            assert_eq!(said, vec!["Image 12".to_owned()]);
            strip_frame_asked(&picture);
            pump(4200);
            let mut said = Vec::new();
            words(&plate, &mut said);
            assert_eq!(said, vec!["Image 12".to_owned(), "Loading image 12".to_owned()]);
            fill_strip_frame(&picture, &texture(), false);
            assert!(picture.paintable().is_some());
            pump(500);
            assert!(!plate.is_visible(), "the plate is gone once the image is in");
            window.destroy();
        });
    }

    /// The words sit in the middle of the part of the image on screen, kept inside the image.
    #[test]
    fn a_strip_plate_centres_its_words_on_the_part_on_screen() {
        run(|| {
            let (frame, picture) = strip_frame(4);
            // At the image's own height, as in the strip's column.
            frame.set_valign(gtk::Align::Start);
            let window = gtk::Window::builder().default_width(400).default_height(400).build();
            window.set_child(Some(&frame));
            window.present();
            pump(800);
            let plate = picture.next_sibling().expect("a plate over the picture");
            let words = plate.first_child().expect("the plate's words");
            let middle = |frame: &gtk::Overlay| {
                let at = words
                    .compute_point(frame, &gtk::graphene::Point::new(0.0, 0.0))
                    .expect("the words are in the frame");
                f64::from(at.y()) + f64::from(words.height()) / 2.0
            };
            picture.set_size_request(200, 300);
            strip_frame_seen(&picture, Some((0.0, 300.0)));
            pump(100);
            assert!(
                (middle(&frame) - 150.0).abs() <= 2.0,
                "centred in an image wholly on screen"
            );
            picture.set_size_request(200, 1600);
            strip_frame_seen(&picture, Some((1000.0, 1400.0)));
            pump(100);
            assert!((middle(&frame) - 1200.0).abs() <= 2.0, "centred on the part on screen");
            window.destroy();
        });
    }

    /// A sliver on screen still leaves the words their room, inside the image; off screen they
    /// take the middle.
    #[test]
    fn strip_plate_margins_keep_the_words_inside_the_image() {
        assert_eq!(strip_words_margins(3000, Some((1600.0, 2400.0)), 50), (1600, 600));
        assert_eq!(strip_words_margins(3000, Some((-500.0, 10.0)), 50), (0, 2950));
        assert_eq!(strip_words_margins(3000, Some((2990.0, 3400.0)), 50), (2950, 0));
        assert_eq!(strip_words_margins(3000, None, 50), (0, 0));
        assert_eq!(strip_words_margins(3000, Some((3200.0, 3600.0)), 50), (0, 0));
    }

    /// A file that comes within 600 ms never shows a plate.
    #[test]
    fn a_quick_strip_image_shows_no_plate() {
        run(|| {
            let (frame, picture) = strip_frame(0);
            let window = gtk::Window::builder().default_width(400).default_height(400).build();
            window.set_child(Some(&frame));
            window.present();
            pump(100);
            fill_strip_frame(&picture, &texture(), false);
            pump(800);
            assert!(!picture.next_sibling().expect("a plate").is_visible());
            window.destroy();
        });
    }

    /// A novel's notice clears the group once the chrome shows, also when the group comes back
    /// from hidden: it is shown before it is measured.
    #[test]
    fn the_notice_clears_a_group_that_was_hidden() {
        run(|| {
            let group = gtk::Box::new(gtk::Orientation::Vertical, 2);
            for _ in 0..3 {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                row.set_size_request(300, 36);
                group.append(&row);
            }
            group.set_margin_bottom(16);
            group.set_valign(gtk::Align::End);
            let window = gtk::Window::builder().default_width(360).default_height(780).build();
            window.set_child(Some(&group));
            window.present();
            let fade = Fader::always();
            fade.hide_now(&group);
            pump(50);
            // Hidden, the group measures nothing: measured before it shows, the notice would
            // sit on it, which is why the chrome shows the group first.
            assert_eq!(notice_lift(&group, true), 16 + 12);
            fade.set(&group, true);
            let lift = notice_lift(&group, true);
            assert!(lift >= 16 + 3 * 36 + 12, "the notice sits {lift} px up, on the group");
            assert_eq!(notice_lift(&group, false), 44);
            window.destroy();
        });
    }

    #[test]
    fn a_page_is_asked_for_before_its_neighbours() {
        use super::wanted_leaves;
        assert_eq!(wanted_leaves(99, 150), vec![99, 100, 101, 98]);
        assert_eq!(wanted_leaves(0, 150), vec![0, 1, 2]);
        assert_eq!(wanted_leaves(149, 150), vec![149, 148]);
        assert_eq!(wanted_leaves(5, 0), Vec::<usize>::new());
    }

    #[test]
    fn a_strip_asks_for_what_is_in_view_first() {
        use super::strip_wanted;
        // Ten images 100 tall with 10 between, the window over the sixth and seventh.
        let heights = [100; 10];
        let (wanted, foot) = strip_wanted(&heights, 10, 560.0, 760.0, 250.0);
        assert_eq!(wanted, vec![5, 6, 7, 8, 9, 4, 3, 2]);
        assert_eq!(foot, 1090.0);
        // At the top nothing lies behind.
        let (wanted, _) = strip_wanted(&heights, 10, 0.0, 150.0, 120.0);
        assert_eq!(wanted, vec![0, 1, 2]);
        assert_eq!(strip_wanted(&[], 10, 0.0, 100.0, 50.0).0, Vec::<usize>::new());
    }

    #[test]
    fn a_popover_slides_to_stay_in_the_window() {
        // Over its button where it fits.
        assert_eq!(popover_middle(500.0, 340.0, 1280.0), 500.0);
        // At the end of a narrow window it slides in, 8 px from the edge.
        assert_eq!(popover_middle(300.0, 300.0, 360.0), 360.0 - 8.0 - 150.0);
        assert_eq!(popover_middle(20.0, 300.0, 360.0), 8.0 + 150.0);
        // Wider than the window: in its middle.
        assert_eq!(popover_middle(300.0, 400.0, 360.0), 180.0);
    }
}
