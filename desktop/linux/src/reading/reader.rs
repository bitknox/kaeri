//! The reader: pages of a chapter, one at a time, or a long strip scrolling, or a novel in a web
//! view. The engine opens the chapter, prepares the pages and keeps the place; this shows the
//! page files it names, turns them on the person's say, and reports what was seen.
//!
//! It sits over the window like the player does: Escape closes it, the toolbar is out of the
//! way until the pointer moves or a key is pressed.

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
    reading::ChapterRow,
};

use super::{listen, novel::NovelView};
use crate::{motion, say, window::Ctx};

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
    why_none: Option<NoNext>,
    mode: Option<ReaderMode>,
    direction: Option<ReadDirection>,
    start_page: Option<u32>,
    leaves: Vec<Leaf>,
    files: HashMap<u32, PageFile>,
    textures: HashMap<u32, gdk::Texture>,
    failed: HashSet<u32>,
    asked: HashSet<u32>,
    index: usize,
    settings: Option<ReaderSettingsView>,
    strip_frames: Vec<(gtk::AspectFrame, gtk::Picture)>,
}

pub struct Reader {
    pub widget: gtk::Overlay,
    ctx: std::rc::Weak<Ctx>,
    show: Rc<dyn Fn(bool)>,
    state: RefCell<State>,
    stack: gtk::Stack,
    picture: gtk::Picture,
    strip: gtk::Box,
    strip_scroll: gtk::ScrolledWindow,
    status: adw::StatusPage,
    toolbar: gtk::Revealer,
    title: gtk::Label,
    counter: gtk::Label,
    settings: gtk::MenuButton,
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
        stack.add_named(&picture, Some("paged"));
        stack.add_named(&strip_scroll, Some("strip"));
        stack.add_named(&status, Some("status"));
        stack.add_css_class("reader-ground");

        let title = gtk::Label::new(None);
        title.add_css_class("heading");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let counter = gtk::Label::new(None);
        counter.add_css_class("numeric");
        counter.add_css_class("haze");
        let close = gtk::Button::from_icon_name("go-previous-symbolic");
        close.set_tooltip_text(Some(&t::reader_close()));
        close.update_property(&[gtk::accessible::Property::Label(&t::reader_close())]);
        let settings = gtk::MenuButton::builder().icon_name("emblem-system-symbolic").build();
        settings.set_tooltip_text(Some(&t::linux_reader_settings_open()));
        settings.set_direction(gtk::ArrowType::Down);
        let chapters = gtk::Button::from_icon_name("view-list-symbolic");
        chapters.set_tooltip_text(Some(&t::reader_chapters()));
        let bar = adw::HeaderBar::new();
        bar.add_css_class("flat");
        bar.pack_start(&close);
        bar.pack_end(&settings);
        bar.pack_end(&counter);
        bar.set_title_widget(Some(&title));
        let toolbar = gtk::Revealer::new();
        toolbar.set_transition_type(gtk::RevealerTransitionType::Crossfade);
        toolbar.set_valign(gtk::Align::Start);
        toolbar.set_child(Some(&bar));
        toolbar.set_reveal_child(true);

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
        widget.add_overlay(&toolbar);
        widget.add_overlay(&loading);
        widget.set_focusable(true);

        let _ = chapters;
        let this = Rc::new(Self {
            widget,
            ctx: Rc::downgrade(ctx),
            show,
            state: RefCell::default(),
            stack,
            picture,
            strip,
            strip_scroll,
            status,
            toolbar,
            title,
            counter,
            settings,
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
        this.controls();
        listen(ctx, &this.widget, {
            let this = Rc::downgrade(&this);
            move |event| {
                if let (Event::Reader(event), Some(this)) = (event, this.upgrade()) {
                    this.take(event);
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
        ReaderSurface {
            width_px: (self.widget.width().max(800) * scale) as u32,
            height_px: (self.widget.height().max(600) * scale) as u32,
            scale: scale as f32,
            eink: false,
            two_pages: false,
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
        self.reveal_toolbar();
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
        self.counter.set_label("");
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
        {
            let mut state = self.state.borrow_mut();
            state.session = view.session;
            state.work = Some(view.work.clone());
            state.medium = Some(view.medium);
            state.chapter = Some(view.chapter.clone());
            state.next = view.next.clone();
            state.mode = Some(view.mode);
            state.direction = Some(view.direction);
            state.start_page = view.start.page;
            state.settings = view.settings.clone();
            state.leaves.clear();
            state.files.clear();
            state.textures.clear();
            state.failed.clear();
            state.asked.clear();
            state.index = 0;
        }
        self.title.set_label(&format!("{} \u{b7} {}", view.title, view.chapter.label));
        self.settings.set_popover(Some(&self.settings_popover()));
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
            state.index = start
                .and_then(|p| {
                    leaves.iter().position(|l| l.parts.iter().any(|part| part.page == p))
                })
                .unwrap_or(0);
            state.leaves = leaves;
        }
        if flows {
            self.build_strip();
        } else {
            self.show_leaf(self.state.borrow().index, TurnHow::Jump);
        }
    }

    fn page_of(&self, index: usize) -> Option<u32> {
        self.state.borrow().leaves.get(index).and_then(|l| l.parts.first()).map(|p| p.page)
    }

    fn is_current_page(&self, page: u32) -> bool {
        let index = self.state.borrow().index;
        self.page_of(index) == Some(page)
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
        self.state.borrow_mut().files.insert(page, file);
        self.state.borrow_mut().failed.remove(&page);
        let flows = self.state.borrow().mode.is_some_and(ReaderMode::flows);
        if flows {
            self.fill_strip_page(page);
        } else if self.is_current_page(page) {
            self.show_leaf(self.state.borrow().index, TurnHow::Jump);
        }
    }

    fn failed_page(self: &Rc<Self>, page: u32) {
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
        let Some(page) = self.page_of(index) else { return };
        self.state.borrow_mut().index = index;
        self.want(index);
        if self.state.borrow().failed.contains(&page) {
            self.failed_page(page);
            return;
        }
        let Some(texture) = self.texture_for(page) else {
            self.wait();
            return;
        };
        self.stop_wait();
        self.picture.set_paintable(Some(&texture));
        self.stack.set_visible_child_name("paged");
        motion::crossfade_in(&self.picture);
        let total = self.state.borrow().leaves.len();
        self.counter.set_label(&format!("{} / {total}", index + 1)); // i18n-skip
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
                self.status.set_child(Some(&back));
                self.stack.set_visible_child_name("status");
            }
        }
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
            let frame = gtk::AspectFrame::new(0.5, 0.5, ratio, false);
            frame.set_child(Some(&picture));
            frame.add_css_class("strip-leaf");
            self.strip.append(&frame);
            frames.push((frame, picture));
        }
        self.state.borrow_mut().strip_frames = frames;
        let this = Rc::downgrade(self);
        self.strip_scroll.vadjustment().connect_value_changed(move |_| {
            if let Some(this) = this.upgrade() {
                this.strip_moved();
            }
        });
        // The strip keeps to a column as wide as the window allows, up to a comfortable width.
        let width_cap = 900;
        self.strip.set_size_request(-1, -1);
        let this = Rc::downgrade(self);
        self.strip_scroll.connect_notify_local(Some("default-width"), |_, _| {});
        self.strip_scroll.add_tick_callback({
            let done = Cell::new(false);
            move |scroll, _| {
                if done.get() || this.upgrade().is_none() {
                    return glib::ControlFlow::Break;
                }
                let width = scroll.width();
                if width > 0
                    && let Some(this) = this.upgrade()
                {
                    this.strip.set_size_request(width.min(width_cap), -1);
                    done.set(true);
                }
                glib::ControlFlow::Continue
            }
        });
        self.strip_moved();
        // Back where the place was: the leaf the engine opened at.
        let index = self.state.borrow().index;
        if index > 0 {
            let this = Rc::downgrade(self);
            glib::idle_add_local_once(move || {
                if let Some(this) = this.upgrade() {
                    this.scroll_to_leaf(index);
                }
            });
        }
    }

    fn scroll_to_leaf(&self, index: usize) {
        let frame = self.state.borrow().strip_frames.get(index).map(|(f, _)| f.clone());
        if let Some(frame) = frame
            && let Some(point) = frame.compute_point(&self.strip, &gtk::graphene::Point::zero())
        {
            self.strip_scroll.vadjustment().set_value(f64::from(point.y()));
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
            let picture = self.state.borrow().strip_frames.get(at).map(|(_, p)| p.clone());
            if let Some(picture) = picture {
                picture.set_paintable(Some(&texture));
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
        self.counter.set_label(&format!(
            "{} / {}",
            here.0 + 1,
            self.state.borrow().leaves.len()
        ));
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
        let ended: Rc<dyn Fn()> = {
            let this = Rc::downgrade(self);
            Rc::new(move || {
                if let Some(this) = this.upgrade() {
                    this.end_of_chapter();
                }
            })
        };
        match NovelView::new(&ctx, open, sender, ended) {
            Ok(novel) => {
                self.stack.add_named(&novel.widget, Some("novel"));
                self.stack.set_visible_child_name("novel");
                *self.novel.borrow_mut() = Some(novel);
            }
            Err(why) => {
                tracing::warn!(%why, "the novel reader could not start");
                self.status.set_icon_name(Some("dialog-warning-symbolic"));
                self.status.set_title(&t::linux_reader_chapter_failed());
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
        for row in &view.rows {
            if !matches!(
                row.key,
                ReaderSettingKey::Mode
                    | ReaderSettingKey::Direction
                    | ReaderSettingKey::Fit
                    | ReaderSettingKey::Background
                    | ReaderSettingKey::Look
                    | ReaderSettingKey::Font
                    | ReaderSettingKey::TextSize
                    | ReaderSettingKey::Margins
                    | ReaderSettingKey::LineSpacing
                    | ReaderSettingKey::Justify
            ) {
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
                    let scale =
                        gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.6, 2.0, 0.05);
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

    fn reveal_toolbar(self: &Rc<Self>) {
        self.toolbar.set_reveal_child(true);
        if let Some(id) = self.hide_timer.borrow_mut().take() {
            id.remove();
        }
        let this = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_secs(3), move || {
            let Some(this) = this.upgrade() else { return };
            this.hide_timer.borrow_mut().take();
            if !this.settings.is_active() {
                this.toolbar.set_reveal_child(false);
            }
        });
        *self.hide_timer.borrow_mut() = Some(id);
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
                this.reveal_toolbar();
                glib::Propagation::Stop
            }
        });
        self.widget.add_controller(keys);

        // Tap zones: the outer thirds turn, the middle brings the toolbar.
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        click.connect_released({
            let this = Rc::downgrade(self);
            move |gesture, _, x, _| {
                let Some(this) = this.upgrade() else { return };
                if this.stack.visible_child_name().as_deref() != Some("paged") {
                    this.reveal_toolbar();
                    return;
                }
                let width = f64::from(this.widget.width().max(1));
                let rtl = this.rtl();
                if x < width / 3.0 {
                    this.turn(rtl, TurnHow::Turn);
                } else if x > width * 2.0 / 3.0 {
                    this.turn(!rtl, TurnHow::Turn);
                } else {
                    this.reveal_toolbar();
                }
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        });
        self.picture.add_controller(click);

        let moved = gtk::EventControllerMotion::new();
        moved.connect_motion({
            let this = Rc::downgrade(self);
            move |_, _, _| {
                if let Some(this) = this.upgrade()
                    && !this.toolbar.reveals_child()
                {
                    this.reveal_toolbar();
                }
            }
        });
        self.widget.add_controller(moved);

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
        ReaderSettingKey::Direction => t::linux_reader_reading_mode(),
        ReaderSettingKey::Fit => t::linux_reader_fit(),
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
        "screen" => t::reader_fit_screen(),
        "width" => t::reader_fit_width(),
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
