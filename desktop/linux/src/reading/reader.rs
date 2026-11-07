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
    time::{Duration, Instant},
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
        Leaf, NoNext, OpenAt, PageEncoding, PageFile, PixelFormat, ReadDirection,
        ReaderCommand, ReaderEvent, ReaderMode, ReaderSettingKey, ReaderSettingsView,
        ReaderSurface, ReaderValue, ReaderView, SettingScope, StripPos, TurnHow,
    },
    reading::{ChapterRow, ReadingCommand, ReadingEvent},
};

use super::{
    chapters::{Ask, Chapters, Here},
    listen,
    novel::{NovelSays, NovelView},
};
use crate::{motion, say, window::Ctx};

/// The chrome leaves this long after the pointer stops.
const CHROME_REST: Duration = Duration::from_secs(2);
/// The group's width on a wide window (the reader board's Linux frame).
const GROUP_WIDTH: i32 = 640;

/// Nothing for this long, then the loading line.
const SILENT: Duration = Duration::from_millis(600);
/// The loading line stays at least this long once shown.
const AT_LEAST: Duration = Duration::from_millis(300);

#[derive(Default)]
struct State {
    session: u64,
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
    asked: HashSet<u32>,
    index: usize,
    settings: Option<ReaderSettingsView>,
    /// Each strip image and its width over height.
    strip_frames: Vec<(gtk::Picture, f32)>,
}

pub struct Reader {
    pub widget: gtk::Overlay,
    ctx: std::rc::Weak<Ctx>,
    show: Rc<dyn Fn(bool)>,
    state: RefCell<State>,
    stack: gtk::Stack,
    picture: gtk::Picture,
    paged_scroll: gtk::ScrolledWindow,
    strip: gtk::Box,
    strip_scroll: gtk::ScrolledWindow,
    status: adw::StatusPage,
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
    settings: gtk::MenuButton,
    /// Chapters: the pages of this one and the series' chapters.
    chapters: Rc<Chapters>,
    chapters_button: gtk::MenuButton,
    loading: gtk::Label,
    loading_since: Cell<Option<Instant>>,
    loading_timer: RefCell<Option<glib::SourceId>>,
    hide_timer: RefCell<Option<glib::SourceId>>,
    strip_timer: RefCell<Option<glib::SourceId>>,
    novel: RefCell<Option<Rc<NovelView>>>,
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
        // A centred column; each image is sized to the strip width setting.
        let strip = gtk::Box::new(gtk::Orientation::Vertical, 0);
        strip.set_halign(gtk::Align::Center);
        let strip_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&strip)
            .build();
        let status = adw::StatusPage::new();
        status.add_css_class("compact");
        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(200);
        stack.add_named(&paged_scroll, Some("paged"));
        stack.add_named(&strip_scroll, Some("strip"));
        stack.add_named(&status, Some("status"));
        stack.add_css_class("reader-ground");

        // The header: back to the series and the series' name, with the window's buttons.
        let title = gtk::Label::new(None);
        title.add_css_class("heading");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let close = gtk::Button::from_icon_name("go-previous-symbolic");
        close.set_tooltip_text(Some(&t::reader_close()));
        close.add_css_class("reader-back");
        close.update_property(&[gtk::accessible::Property::Label(&t::reader_close())]);
        let head = adw::HeaderBar::new();
        head.add_css_class("flat");
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
        let settings = gtk::MenuButton::builder().icon_name("kaeri-text-look-symbolic").build();
        settings.set_tooltip_text(Some(&t::linux_reader_settings_open()));
        settings.set_direction(gtk::ArrowType::Up);
        let full = gtk::Button::from_icon_name("view-fullscreen-symbolic");
        full.set_tooltip_text(Some(&t::player_fullscreen()));
        let ends = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        ends.append(&settings);
        ends.append(&full);
        let chapters = Chapters::new();
        let chapters_button = gtk::MenuButton::builder()
            .icon_name("view-list-symbolic")
            .popover(&chapters.popover)
            .direction(gtk::ArrowType::Up)
            .build();
        chapters_button.set_tooltip_text(Some(&t::reader_chapters()));
        // The names sit in the middle of the group, whatever is at its ends.
        let row = gtk::CenterBox::new();
        row.set_start_widget(Some(&chapters_button));
        row.set_center_widget(Some(&names));
        row.set_end_widget(Some(&ends));
        let group = gtk::Box::new(gtk::Orientation::Vertical, 2);
        group.add_css_class("reader-group");
        group.append(&scrub_row);
        group.append(&row);
        group.set_halign(gtk::Align::Center);
        group.set_valign(gtk::Align::End);
        group.set_margin_bottom(16);
        group.set_width_request(GROUP_WIDTH);

        let loading = gtk::Label::new(None);
        loading.add_css_class("osd");
        loading.add_css_class("caption");
        loading.set_valign(gtk::Align::End);
        loading.set_halign(gtk::Align::Center);
        loading.set_margin_bottom(24);
        loading.set_opacity(0.0);
        loading.set_can_target(false);

        let widget = gtk::Overlay::new();
        widget.set_child(Some(&stack));
        widget.add_overlay(&head);
        widget.add_overlay(&group);
        widget.add_overlay(&loading);
        widget.set_focusable(true);
        widget.add_css_class("reader");
        let this = Rc::new(Self {
            widget,
            ctx: Rc::downgrade(ctx),
            show,
            state: RefCell::default(),
            stack,
            picture,
            paged_scroll,
            strip,
            strip_scroll,
            status,
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
            settings,
            chapters,
            chapters_button,
            loading,
            loading_since: Cell::new(None),
            loading_timer: RefCell::default(),
            hide_timer: RefCell::default(),
            strip_timer: RefCell::default(),
            novel: RefCell::default(),
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
            let ctx = Rc::downgrade(ctx);
            move |_| {
                if let Some(ctx) = ctx.upgrade() {
                    let window = &ctx.window;
                    window.set_fullscreened(!window.is_fullscreen());
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
                            this.send(ReaderCommand::OpenChapter { session, chapter });
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

    fn hub(&self) -> Option<Rc<crate::hub::Hub>> {
        self.ctx.upgrade().map(|ctx| Rc::clone(&ctx.hub))
    }

    fn send(&self, command: ReaderCommand) {
        if let Some(hub) = self.hub() {
            hub.send(Command::Reader(command));
        }
    }

    fn surface(&self) -> ReaderSurface {
        let scale = self.widget.scale_factor().max(1);
        let dark = adw::StyleManager::default().is_dark();
        let width_px = (self.widget.width().max(800) * scale) as u32;
        let height_px = (self.widget.height().max(600) * scale) as u32;
        ReaderSurface {
            width_px,
            height_px,
            scale: scale as f32,
            eink: false,
            two_pages: width_px >= height_px * 13 / 10,
            dark,
        }
    }

    /// Open a chapter of a work, from where it was left when `chapter` is none.
    pub fn open(self: &Rc<Self>, work: WorkId, chapter: Option<ChapterId>) {
        self.reset();
        self.state.borrow_mut().work = Some(work.clone());
        self.wait();
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

    fn reset(&self) {
        let old = self.state.replace(State::default());
        let _ = old;
        while let Some(child) = self.strip.first_child() {
            self.strip.remove(&child);
        }
        self.picture.set_paintable(gdk::Paintable::NONE);
        if let Some(novel) = self.novel.take() {
            self.stack.remove(&novel.widget);
        }
        self.title.set_label("");
        self.group_title.set_label("");
        self.line.set_label("");
        self.progress(0, 0);
        self.chapters.clear();
    }

    /// Set a reader setting for the shots, on the medium's own value. A choice or `on`/`off`.
    #[cfg(feature = "fixtures")]
    pub fn shot_setting(&self, key: ReaderSettingKey, name: &str) {
        let medium = match key {
            ReaderSettingKey::Look => Medium::Novel,
            _ => Medium::Manga,
        };
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

    /// Leave the reader; the engine saves the place as the session closes.
    pub fn close(&self) {
        let session = self.state.borrow().session;
        if session != 0 {
            self.send(ReaderCommand::Close { session });
        }
        self.stop_wait();
        (self.show)(false);
        if let Some(ctx) = self.ctx.upgrade()
            && let Some(work) = self.state.borrow().work.clone()
        {
            let _ = WidgetExt::activate_action(
                &ctx.window,
                "win.reader-closed",
                Some(&work.0.to_variant()),
            );
        }
    }

    /// The loading line: nothing for 600 ms, then at least 300 ms once it shows.
    fn wait(self: &Rc<Self>) {
        self.stop_wait();
        let this = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(SILENT, move || {
            let Some(this) = this.upgrade() else { return };
            this.loading_timer.borrow_mut().take();
            this.loading.set_label(&t::linux_reader_opening());
            this.loading_since.set(Some(Instant::now()));
            motion::fade_in(&this.loading);
        });
        *self.loading_timer.borrow_mut() = Some(id);
    }

    fn stop_wait(&self) {
        if let Some(id) = self.loading_timer.borrow_mut().take() {
            id.remove();
        }
        if let Some(since) = self.loading_since.take() {
            let left = AT_LEAST.saturating_sub(since.elapsed());
            let label = self.loading.clone();
            glib::timeout_add_local_once(left, move || {
                motion::fade_out(&label);
            });
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
            ReaderEvent::PageReady { session, page, file, .. } if self.mine(*session) => {
                self.page_ready(*page, file.clone());
            }
            ReaderEvent::PageFailed { session, page, .. } if self.mine(*session) => {
                self.state.borrow_mut().failed.insert(*page);
                if self.is_current_page(*page) {
                    self.failed_page(*page);
                }
            }
            ReaderEvent::EndPage { session, next, why_none, .. } if self.mine(*session) => {
                let mut state = self.state.borrow_mut();
                state.next = next.clone();
                state.why_none = *why_none;
            }
            ReaderEvent::NovelOpen(open) if self.mine(open.session) => {
                self.open_novel(open);
            }
            ReaderEvent::Settings(view) => self.settings_view(view.clone()),
            ReaderEvent::GoTo { session, place } if self.mine(*session) => {
                if let Some(page) = place.page {
                    self.go_to_page(page);
                }
            }
            ReaderEvent::Closed { session } if *session == self.state.borrow().session => {
                self.stop_wait();
            }
            ReaderEvent::Opening { .. } | ReaderEvent::Notice(_) => {}
            _ => {}
        }
        if let ReaderEvent::NovelStyle { session, style, count_key, counted } = event
            && self.mine(*session)
            && let Some(novel) = self.novel.borrow().clone()
        {
            novel.style(style, count_key, counted);
        }
    }

    fn mine(&self, session: u64) -> bool {
        session != 0 && session == self.state.borrow().session
    }

    fn opened(self: &Rc<Self>, view: &ReaderView) {
        // The series' chapters for Chapters, with their states as they are now.
        if let Some(ctx) = self.ctx.upgrade() {
            super::send(
                &ctx.hub,
                ReadingCommand::LoadChapters { work: view.work.clone(), edition: None },
            );
        }
        {
            let mut state = self.state.borrow_mut();
            state.session = view.session;
            state.work = Some(view.work.clone());
            state.medium = Some(view.medium);
            state.chapter = Some(view.chapter.clone());
            state.next = view.next.clone();
            state.previous = view.previous.clone();
            state.mode = Some(view.mode);
            state.direction = Some(view.direction);
            state.start_page = view.start.page;
            state.start_strip = view.start.strip;
            state.settings = view.settings.clone();
            state.leaves.clear();
            state.files.clear();
            state.textures.clear();
            state.failed.clear();
            state.asked.clear();
            state.index = 0;
        }
        self.title.set_label(&view.title);
        self.group_title.set_label(&view.title);
        self.line.set_label(&view.chapter.label);
        // The scrubber runs with the book: right to left fills from the right, with the count
        // at the right and the total at the left, as the board draws it. A strip runs top to
        // bottom and fills from the left. The scale keeps its own direction and is inverted.
        let rtl = view.direction == ReadDirection::RightToLeft
            && (view.medium == Medium::Novel || !view.mode.flows());
        if let Some(row) = self.scrub.parent() {
            row.set_direction(if rtl {
                gtk::TextDirection::Rtl
            } else {
                gtk::TextDirection::Ltr
            });
        }
        self.scrub.set_direction(gtk::TextDirection::Ltr);
        self.scrub.set_inverted(rtl);
        self.settings.set_popover(Some(&self.settings_popover()));
        self.apply_look();
        if view.medium == Medium::Novel {
            // The novel's own event follows; the web view takes the stack then.
            return;
        }
        if view.mode.flows() {
            self.stack.set_visible_child_name("strip");
        } else {
            self.stack.set_visible_child_name("paged");
        }
    }

    // ---------------------------------------------------------------------------------------
    // Pages

    fn leaves(self: &Rc<Self>, leaves: Vec<Leaf>) {
        let flows = self.state.borrow().mode.is_some_and(ReaderMode::flows);
        {
            let mut state = self.state.borrow_mut();
            for leaf in &leaves {
                for part in &leaf.parts {
                    if let Some(file) = &part.file {
                        state.files.insert(part.page, file.clone());
                    }
                }
            }
            let start = state.start_page;
            let in_strip =
                state.start_strip.as_ref().map(|s| s.image as usize).filter(|_| flows);
            state.index = in_strip
                .or_else(|| {
                    start.and_then(|p| {
                        leaves.iter().position(|l| l.parts.iter().any(|part| part.page == p))
                    })
                })
                .unwrap_or(0)
                .min(leaves.len().saturating_sub(1));
            state.leaves = leaves;
        }
        if flows {
            self.build_strip();
        } else {
            let index = self.state.borrow().index;
            self.show_leaf(index, TurnHow::Jump);
        }
    }

    fn page_of(&self, index: usize) -> Option<u32> {
        self.state.borrow().leaves.get(index).and_then(|l| l.parts.first()).map(|p| p.page)
    }

    fn is_current_page(&self, page: u32) -> bool {
        let state = self.state.borrow();
        state.leaves.get(state.index).is_some_and(|l| l.parts.iter().any(|p| p.page == page))
    }

    fn texture_for(&self, page: u32) -> Option<gdk::Texture> {
        if let Some(texture) = self.state.borrow().textures.get(&page) {
            return Some(texture.clone());
        }
        let file = self.state.borrow().files.get(&page).cloned()?;
        let texture = load_texture(&file)?;
        self.state.borrow_mut().textures.insert(page, texture.clone());
        Some(texture)
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
            let pages: Vec<u32> = (index.saturating_sub(1)
                ..(index + 3).min(state.leaves.len()))
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
        self.state.borrow_mut().files.insert(page, file);
        self.state.borrow_mut().failed.remove(&page);
        let flows = self.state.borrow().mode.is_some_and(ReaderMode::flows);
        if flows {
            self.fill_strip_page(page);
        } else if self.is_current_page(page) {
            let index = self.state.borrow().index;
            self.show_leaf(index, TurnHow::Jump);
        }
    }

    fn failed_page(self: &Rc<Self>, page: u32) {
        self.status.set_description(None);
        self.status.set_icon_name(Some("dialog-warning-symbolic"));
        self.status.set_title(&t::reader_page_failed(i64::from(page + 1)));
        let again = gtk::Button::with_label(&t::reader_try_again());
        again.add_css_class("pill");
        again.set_halign(gtk::Align::Center);
        again.connect_clicked({
            let this = Rc::downgrade(self);
            move |_| {
                let Some(this) = this.upgrade() else { return };
                let (session, chapter) = {
                    let state = this.state.borrow();
                    (state.session, state.chapter.as_ref().map(|c| c.id.clone()))
                };
                if let Some(chapter) = chapter {
                    this.state.borrow_mut().failed.remove(&page);
                    this.send(ReaderCommand::Retry { session, chapter, page: Some(page) });
                }
            }
        });
        self.status.set_child(Some(&again));
        self.stack.set_visible_child_name("status");
    }

    fn show_leaf(self: &Rc<Self>, index: usize, how: TurnHow) {
        if self.page_of(index).is_none() {
            return;
        }
        self.state.borrow_mut().index = index;
        self.want(index);
        let failed = {
            let state = self.state.borrow();
            state.leaves.get(index).and_then(|l| {
                l.parts.iter().map(|p| p.page).find(|page| state.failed.contains(page))
            })
        };
        if let Some(page) = failed {
            self.failed_page(page);
            return;
        }
        let Some(paintable) = self.compose(index) else {
            self.wait();
            return;
        };
        self.stop_wait();
        self.picture.set_paintable(Some(&paintable));
        self.apply_fit();
        self.scroll_to_start();
        self.stack.set_visible_child_name("paged");
        motion::crossfade_in(&self.picture);
        let total = self.state.borrow().leaves.len();
        self.progress(index, total);
        let (session, chapter) = {
            let state = self.state.borrow();
            (state.session, state.chapter.as_ref().map(|c| c.id.clone()))
        };
        if let Some(chapter) = chapter {
            self.send(ReaderCommand::Shown { session, chapter, leaf: index as u32, how });
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
            self.show_leaf(at, TurnHow::Jump);
        }
    }

    /// One leaf on: right of a right-to-left book is back.
    fn turn(self: &Rc<Self>, forward: bool, how: TurnHow) {
        let (index, total, medium) = {
            let state = self.state.borrow();
            (state.index, state.leaves.len(), state.medium)
        };
        if medium == Some(Medium::Novel) {
            if let Some(novel) = self.novel.borrow().clone() {
                novel.turn(forward);
            }
            return;
        }
        if self.state.borrow().mode.is_some_and(ReaderMode::flows) {
            let adj = self.strip_scroll.vadjustment();
            let by = adj.page_size() * 0.9 * if forward { 1.0 } else { -1.0 };
            adj.set_value((adj.value() + by).clamp(adj.lower(), adj.upper() - adj.page_size()));
            return;
        }
        if matches!(how, TurnHow::Key) && self.scroll_within(forward) {
            return;
        }
        if forward {
            if index + 1 < total {
                self.show_leaf(index + 1, how);
            } else {
                self.end_of_chapter();
            }
        } else if index > 0 {
            self.show_leaf(index - 1, how);
        } else {
            self.send(ReaderCommand::GoBack { session: self.state.borrow().session });
        }
    }

    fn end_of_chapter(self: &Rc<Self>) {
        let (session, next, why) = {
            let state = self.state.borrow();
            (state.session, state.next.clone(), state.why_none)
        };
        match next {
            Some(next) => self.send(ReaderCommand::OpenChapter { session, chapter: next.id }),
            None => {
                let words = match why {
                    Some(NoNext::NotOutYet) => t::linux_reader_end_not_out(),
                    Some(NoNext::Finished) => t::linux_reader_end_finished(),
                    Some(NoNext::Offline) => t::linux_reader_end_offline(),
                    Some(NoNext::NoSourceHasIt) => t::linux_reader_end_no_source(),
                    _ => t::reader_end_caught_up(),
                };
                self.status.set_icon_name(Some("emblem-ok-symbolic"));
                self.status.set_title(&words);
                self.status.set_description(None);
                self.status.set_child(Some(&self.back_button()));
                self.stack.set_visible_child_name("status");
            }
        }
    }

    /// Back to the series, under the end page and under a novel that can't open.
    fn back_button(self: &Rc<Self>) -> gtk::Button {
        let back = gtk::Button::with_label(&t::linux_reader_back_to_series());
        back.add_css_class("pill");
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
        for leaf in &leaves {
            let ratio = leaf
                .parts
                .first()
                .and_then(|p| {
                    p.ratio.or_else(|| {
                        p.file.as_ref().map(|f| f.width as f32 / f.height.max(1) as f32)
                    })
                })
                .unwrap_or(0.7)
                .max(0.05);
            let picture = gtk::Picture::new();
            picture.set_content_fit(gtk::ContentFit::Fill);
            picture.set_can_shrink(true);
            picture.add_css_class("strip-leaf");
            self.strip.append(&picture);
            frames.push((picture, ratio));
        }
        self.state.borrow_mut().strip_frames = frames;
        self.size_strip();
        self.stop_wait();
        self.strip_moved();
        // Back where the place was: the leaf the engine opened at.
        let index = self.state.borrow().index;
        self.scroll_to_leaf(index);
    }

    /// Scroll the strip to a leaf once the layout has room for it, and into it as far as the
    /// place says when it is the image the place was left in, the first one included.
    fn scroll_to_leaf(&self, index: usize) {
        let target = {
            let state = self.state.borrow();
            let heights: Vec<i32> =
                state.strip_frames.iter().map(|(p, _)| p.height_request()).collect();
            let offset = state
                .start_strip
                .as_ref()
                .filter(|s| s.image as usize == index)
                .map(|s| s.offset);
            strip_target(&heights, self.strip.spacing(), index, offset)
        };
        self.scroll_strip_to(target);
    }

    /// Scroll the strip to `target` once the layout reaches that far.
    fn scroll_strip_to(&self, target: f64) {
        let adj = self.strip_scroll.vadjustment();
        if target <= 0.0 {
            adj.set_value(adj.lower());
            return;
        }
        let frames = Cell::new(0);
        self.strip_scroll.add_tick_callback(move |_, _| {
            frames.set(frames.get() + 1);
            if adj.upper() - adj.page_size() >= target || frames.get() > 30 {
                adj.set_value(target.min(adj.upper() - adj.page_size()));
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }

    /// Each strip image at the column's width: the strip width setting's share of 720 px at
    /// the default, never wider than the window. Pages one under another fit the window.
    fn size_strip(&self) {
        let vertical = self.state.borrow().mode == Some(ReaderMode::Vertical);
        let share = self.amount(ReaderSettingKey::StripWidth).unwrap_or(0.6).clamp(0.3, 1.0);
        let gap = self.choice(ReaderSettingKey::StripGap).as_deref() == Some("small");
        let width = f64::from(self.widget.width().max(1));
        let height = f64::from(self.widget.height().max(1));
        self.strip.set_spacing(if gap || vertical { 8 } else { 0 });
        let column = width.min(720.0 * share / 0.6);
        for (picture, ratio) in &self.state.borrow().strip_frames {
            let ratio = f64::from(*ratio);
            let w = if vertical { width.min(height * ratio) } else { column };
            picture.set_size_request(w.round() as i32, (w / ratio).round() as i32);
        }
    }

    fn fill_strip_page(&self, page: u32) {
        let at = self
            .state
            .borrow()
            .leaves
            .iter()
            .position(|l| l.parts.first().is_some_and(|p| p.page == page));
        if let Some(at) = at
            && let Some(texture) = self.texture_for(page)
        {
            let ratio = texture.width() as f32 / texture.height().max(1) as f32;
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
                picture.set_paintable(Some(&texture));
            }
            if resized {
                self.size_strip();
            }
        }
    }

    /// Ask for the pages near the viewport and, a moment after the scrolling stops, say where
    /// it stopped.
    fn strip_moved(self: &Rc<Self>) {
        let adj = self.strip_scroll.vadjustment();
        let (top, bottom) = (adj.value(), adj.value() + adj.page_size());
        let reach = adj.page_size() * 1.5;
        let mut wanted = Vec::new();
        let mut here = (0_usize, 0.0_f32);
        {
            let state = self.state.borrow();
            for (i, (frame, _)) in state.strip_frames.iter().enumerate() {
                let Some(point) =
                    frame.compute_point(&self.strip, &gtk::graphene::Point::zero())
                else {
                    continue;
                };
                let (y, h) = (f64::from(point.y()), f64::from(frame.height()));
                if y + h >= top - reach && y <= bottom + reach {
                    wanted.push(i);
                }
                if y <= top + 1.0 && y + h > top {
                    here = (i, ((top - y) / h.max(1.0)) as f32);
                }
            }
        }
        for i in wanted {
            self.want_strip(i);
        }
        let total = self.state.borrow().leaves.len();
        self.state.borrow_mut().index = here.0;
        self.progress(here.0, total);
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
                this.send(ReaderCommand::StripAt {
                    session,
                    chapter,
                    at: StripPos { image: here.0 as u32, offset: here.1 },
                    foot_visible: false,
                });
            }
        });
        *self.strip_timer.borrow_mut() = Some(id);
    }

    fn want_strip(&self, index: usize) {
        let page = self.page_of(index);
        let Some(page) = page else { return };
        if self.state.borrow().files.contains_key(&page) {
            self.fill_strip_page(page);
            return;
        }
        self.want(index);
    }

    // ---------------------------------------------------------------------------------------
    // Novel

    fn open_novel(self: &Rc<Self>, open: &kaeri_model::reader::NovelOpenView) {
        let Some(ctx) = self.ctx.upgrade() else { return };
        self.stop_wait();
        if let Some(old) = self.novel.take() {
            self.stack.remove(&old.widget);
        }
        let session = open.session;
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
                        this.progress(page as usize, pages as usize);
                    }
                    NovelSays::Middle => this.toggle_chrome(),
                    NovelSays::End => this.end_of_chapter(),
                }
            })
        };
        match NovelView::new(&ctx, open, sender, tell) {
            Ok(novel) => {
                self.stack.add_named(&novel.widget, Some("novel"));
                self.stack.set_visible_child_name("novel");
                *self.novel.borrow_mut() = Some(novel);
            }
            Err(why) => {
                tracing::warn!(%why, "the novel reader could not start");
                self.status.set_icon_name(Some("dialog-warning-symbolic"));
                self.status.set_title(&t::linux_reader_novel_unavailable());
                self.status.set_description(Some(&t::linux_reader_novel_needs_webkit()));
                self.status.set_child(Some(&self.back_button()));
                self.stack.set_visible_child_name("status");
            }
        }
        let _ = session;
    }

    // ---------------------------------------------------------------------------------------
    // Settings

    fn settings_view(self: &Rc<Self>, view: ReaderSettingsView) {
        self.state.borrow_mut().settings = Some(view);
        self.settings.set_popover(Some(&self.settings_popover()));
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

    /// With a page taller than the window, the keys scroll it before they turn. True when they
    /// scrolled.
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
        adj.set_value(adj.value() + if forward { by } else { -by });
        true
    }

    fn settings_popover(self: &Rc<Self>) -> gtk::Popover {
        let popover = gtk::Popover::new();
        let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
        column.set_margin_top(12);
        column.set_margin_bottom(12);
        column.set_margin_start(12);
        column.set_margin_end(12);
        let (view, session, medium, work) = {
            let state = self.state.borrow();
            (state.settings.clone(), state.session, state.medium, state.work.clone())
        };
        let (Some(view), Some(medium)) = (view, medium) else {
            return popover;
        };
        let flows = self.state.borrow().mode.is_some_and(ReaderMode::flows);
        for row in &view.rows {
            let shown = match row.key {
                ReaderSettingKey::Mode
                | ReaderSettingKey::Background
                | ReaderSettingKey::Look
                | ReaderSettingKey::Font
                | ReaderSettingKey::TextSize
                | ReaderSettingKey::Margins
                | ReaderSettingKey::LineSpacing
                | ReaderSettingKey::Justify => true,
                // A strip has no direction, no fit and no spreads; pages have no strip width.
                ReaderSettingKey::Direction
                | ReaderSettingKey::Fit
                | ReaderSettingKey::CropBorders
                | ReaderSettingKey::PageLayout
                | ReaderSettingKey::Spreads => !flows,
                ReaderSettingKey::StripWidth | ReaderSettingKey::StripGap => flows,
                _ => false,
            };
            if !shown {
                continue;
            }
            let scope = row.key.series_scope().unwrap_or_else(|| row.key.home_scope());
            let send = {
                let this = Rc::downgrade(self);
                let (key, work) = (row.key, work.clone());
                move |value: ReaderValue, scope: SettingScope| {
                    if let Some(this) = this.upgrade() {
                        this.send(ReaderCommand::SetSetting {
                            session: Some(session),
                            medium,
                            work: work.clone(),
                            key,
                            scope,
                            value,
                        });
                    }
                }
            };
            let label = gtk::Label::new(Some(&setting_name(row.key)));
            label.set_xalign(0.0);
            label.add_css_class("caption-heading");
            column.append(&label);
            match &row.value {
                ReaderValue::Choice { name } if !row.choices.is_empty() => {
                    let group = adw::ToggleGroup::new();
                    for choice in &row.choices {
                        if let ReaderValue::Choice { name } = choice {
                            group.add(
                                adw::Toggle::builder()
                                    .name(name.as_str())
                                    .label(choice_label(name))
                                    .build(),
                            );
                        }
                    }
                    group.set_active_name(Some(name));
                    group.add_css_class("round");
                    group.connect_active_name_notify(move |group| {
                        if let Some(name) = group.active_name() {
                            send(ReaderValue::Choice { name: name.to_string() }, scope);
                        }
                    });
                    column.append(&group);
                }
                ReaderValue::Amount { value } => {
                    let (low, high) = match row.key {
                        ReaderSettingKey::StripWidth => (0.3, 1.0),
                        ReaderSettingKey::LineSpacing => (1.2, 1.8),
                        ReaderSettingKey::Margins => (0.6, 1.6),
                        _ => (0.5, 3.0),
                    };
                    let scale =
                        gtk::Scale::with_range(gtk::Orientation::Horizontal, low, high, 0.05);
                    scale.set_value(*value);
                    scale.set_size_request(220, -1);
                    scale.connect_value_changed(move |scale| {
                        send(ReaderValue::Amount { value: scale.value() }, scope);
                    });
                    column.append(&scale);
                }
                ReaderValue::Toggle { on } => {
                    let switch = gtk::Switch::new();
                    switch.set_active(*on);
                    switch.set_halign(gtk::Align::Start);
                    switch.connect_active_notify(move |switch| {
                        send(ReaderValue::Toggle { on: switch.is_active() }, scope);
                    });
                    column.append(&switch);
                }
                ReaderValue::Choice { .. } => {}
            }
        }
        popover.set_child(Some(&column));
        popover
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
    /// the group or a shot pinned it.
    fn reveal_chrome(self: &Rc<Self>) {
        self.head_fade.set(&self.head, true);
        self.group_fade.set(&self.group, true);
        if let Some(id) = self.hide_timer.borrow_mut().take() {
            id.remove();
        }
        let this = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(CHROME_REST, move || {
            let Some(this) = this.upgrade() else { return };
            this.hide_timer.borrow_mut().take();
            let open = this.settings.is_active() || this.chapters_button.is_active();
            if open || this.group_hovered() || this.pinned.get() {
                this.reveal_chrome();
                return;
            }
            this.hide_chrome();
        });
        *self.hide_timer.borrow_mut() = Some(id);
    }

    fn hide_chrome(&self) {
        if let Some(id) = self.hide_timer.borrow_mut().take() {
            id.remove();
        }
        if self.pinned.get() {
            return;
        }
        self.head_fade.set(&self.head, false);
        self.group_fade.set(&self.group, false);
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

    /// Open Chapters for a shot, on its `tab`: "pages" or "chapters".
    #[cfg(feature = "fixtures")]
    pub fn shot_chapters(self: &Rc<Self>, tab: &str) {
        self.shot_chrome(true);
        self.chapters_button.popup();
        self.chapters.shot_tab(tab);
    }

    /// Where the place is, for the scrubber: `index` from 0 of `total`.
    fn progress(&self, index: usize, total: usize) {
        let total = total.max(1);
        let at = (index + 1).min(total);
        self.scrub.set_range(1.0, total.max(2) as f64);
        self.scrub.set_sensitive(total > 1);
        if self.scrub_timer.borrow().is_none() {
            self.scrub.set_value(at as f64);
            self.scrub_at.set_label(&at.to_string());
        }
        self.scrub_total.set_label(&total.to_string());
        let said = t::linux_reader_page_of(at as i64, total as i64);
        self.scrub.update_property(&[gtk::accessible::Property::ValueText(&said)]);
    }

    /// The scrubber dragged or keyed to `value`: the number follows at once, the page once the
    /// thumb rests.
    fn scrubbed(self: &Rc<Self>, value: f64) {
        let (lower, upper) = (self.scrub.adjustment().lower(), self.scrub.adjustment().upper());
        let at = value.round().clamp(lower, upper) as usize;
        self.scrub_at.set_label(&at.to_string());
        self.reveal_chrome();
        if let Some(id) = self.scrub_timer.borrow_mut().take() {
            id.remove();
        }
        let this = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_millis(250), move || {
            let Some(this) = this.upgrade() else { return };
            this.scrub_timer.borrow_mut().take();
            this.jump_to(at.saturating_sub(1));
        });
        *self.scrub_timer.borrow_mut() = Some(id);
    }

    /// Go to the scrubber's place: a leaf, a strip image or a novel's page, from 0.
    fn jump_to(self: &Rc<Self>, index: usize) {
        let (medium, flows) = {
            let state = self.state.borrow();
            (state.medium, state.mode.is_some_and(ReaderMode::flows))
        };
        if medium == Some(Medium::Novel) {
            if let Some(novel) = self.novel.borrow().clone() {
                novel.go_to(index as u32);
            }
        } else if flows {
            let target = {
                let state = self.state.borrow();
                let heights: Vec<i32> =
                    state.strip_frames.iter().map(|(p, _)| p.height_request()).collect();
                strip_target(&heights, self.strip.spacing(), index, None)
            };
            self.scroll_strip_to(target);
        } else {
            self.show_leaf(index, TurnHow::Jump);
        }
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
            self.send(ReaderCommand::OpenChapter { session, chapter: chapter.id });
        }
    }

    fn rtl(&self) -> bool {
        matches!(self.state.borrow().direction, Some(ReadDirection::RightToLeft))
    }

    fn controls(self: &Rc<Self>) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let this = Rc::downgrade(self);
            move |_, key, _, _| {
                let Some(this) = this.upgrade() else { return glib::Propagation::Proceed };
                let rtl = this.rtl();
                match key {
                    gdk::Key::Escape => this.close(),
                    gdk::Key::Right => this.turn(!rtl, TurnHow::Key),
                    gdk::Key::Left => this.turn(rtl, TurnHow::Key),
                    gdk::Key::space | gdk::Key::Page_Down | gdk::Key::Down => {
                        this.turn(true, TurnHow::Key)
                    }
                    gdk::Key::Page_Up | gdk::Key::Up => this.turn(false, TurnHow::Key),
                    gdk::Key::Home => this.show_leaf(0, TurnHow::Jump),
                    gdk::Key::End => {
                        let last = this.state.borrow().leaves.len().saturating_sub(1);
                        this.show_leaf(last, TurnHow::Jump);
                    }
                    _ => return glib::Propagation::Proceed,
                }
                this.hide_chrome();
                glib::Propagation::Stop
            }
        });
        self.widget.add_controller(keys);

        // Tap zones: the outer thirds turn, the middle brings the chrome or takes it away.
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        click.connect_released({
            let this = Rc::downgrade(self);
            move |gesture, _, x, _| {
                let Some(this) = this.upgrade() else { return };
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
                }
                if let Some(id) = pending.borrow_mut().take() {
                    id.remove();
                }
                if !first {
                    let (this, done) = (this.clone(), Rc::clone(&pending));
                    let id =
                        glib::timeout_add_local_once(Duration::from_millis(250), move || {
                            done.borrow_mut().take();
                            let Some(this) = this.upgrade() else { return };
                            let session = this.state.borrow().session;
                            if session != 0 && this.widget.is_mapped() {
                                this.send(ReaderCommand::Surface {
                                    session,
                                    surface: this.surface(),
                                });
                            }
                        });
                    *pending.borrow_mut() = Some(id);
                }
                glib::ControlFlow::Continue
            }
        });
        let _ = say::percent;
    }
}

fn load_texture(file: &PageFile) -> Option<gdk::Texture> {
    match &file.encoding {
        PageEncoding::Original { .. } => gdk::Texture::from_filename(&file.path).ok(),
        PageEncoding::Raw { format, stride } => {
            let bytes = std::fs::read(&file.path).ok()?;
            let format = match format {
                PixelFormat::Gray8 => gdk::MemoryFormat::G8,
                PixelFormat::Rgba8 => gdk::MemoryFormat::R8g8b8a8,
            };
            Some(
                gdk::MemoryTexture::new(
                    file.width as i32,
                    file.height as i32,
                    format,
                    &glib::Bytes::from_owned(bytes),
                    *stride as usize,
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
}
