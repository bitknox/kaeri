//! The reader's Chapters popover, as the reader board draws it for the desktop and as Apple's and
//! Android's sheets have it: for manga the chapter's pages in a grid of five with the open one
//! ringed, laid out from the right in a right-to-left book, and the series' chapters by volume
//! with the open one on the selected fill. A novel has the chapters alone. Picking a page or a
//! chapter goes there and closes it.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use kaeri_core::books::ChapterId;
use kaeri_i18n::t;
use kaeri_model::reader::{Leaf, PageEncoding, PageFile, PixelFormat};
use kaeri_model::reading::{ChapterReadState, ChapterRow, VolumeGroup};

/// How many times one opening moves the list towards its open chapter before it settles for where
/// the row is.
const CENTRE_MOVES: u8 = 8;

/// One step of centring the open chapter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Centring {
    /// The row is as near the middle as the list's ends allow, or there is nothing to centre.
    Done,
    /// The list moved towards it: the layout this asks for looks again.
    Moved,
    /// Not laid out yet: the window or popover is still being mapped, so GTK has not placed the
    /// row. The next layout looks again.
    Waiting,
}

/// A thumbnail's width in the grid, and what it is decoded to: twice that, for a 2x screen.
const THUMB_WIDTH: i32 = 84;
const THUMB_PIXELS: u32 = 168;

/// What the popover asks of the reader.
pub enum Ask {
    /// Show this leaf (from 0), or this strip image.
    Jump(usize),
    /// Open another chapter.
    Open(ChapterId),
    /// A page the grid shows and the reader does not have yet.
    Need(u32),
}

/// What the reader tells the popover as it opens.
pub struct Here {
    pub chapter: ChapterId,
    pub label: String,
    /// The pages tab: manga, paged or a strip.
    pub pages: bool,
    pub rtl: bool,
    pub leaves: Vec<Leaf>,
    pub open_leaf: usize,
    pub files: HashMap<u32, PageFile>,
}

/// Where the popover sends what is picked.
type Asker = Rc<dyn Fn(Ask)>;

pub struct Chapters {
    pub popover: gtk::Popover,
    title: gtk::Label,
    tabs: adw::ToggleGroup,
    stack: gtk::Stack,
    model: gtk::StringList,
    grid: gtk::GridView,
    /// The chapters, a row each, drawn as they scroll into view: a series can list a thousand.
    list: gtk::ListView,
    entries: gio::ListStore,
    /// The read volumes opened since the popover opened, by their place in `volumes`.
    open_folds: Rc<RefCell<HashSet<usize>>>,
    volumes: RefCell<Option<Vec<VolumeGroup>>>,
    here: RefCell<Option<ChapterId>>,
    /// The open chapter's row while it is bound, which the list centres on as it opens.
    here_row: RefCell<glib::WeakRef<gtk::Widget>>,
    /// The list still has to open on the open chapter: set as the popover fills, done once the
    /// row sits in the middle.
    centring: Cell<bool>,
    open_leaf: Cell<usize>,
    files: RefCell<HashMap<u32, PageFile>>,
    /// Thumbnails waiting for their page, by page.
    waiting: RefCell<HashMap<u32, Vec<glib::WeakRef<gtk::Picture>>>>,
    ask: RefCell<Option<Asker>>,
}

impl Chapters {
    pub fn new() -> Rc<Self> {
        let title = gtk::Label::new(None);
        title.add_css_class("reader-chapters-title");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let tabs = adw::ToggleGroup::new();
        tabs.add(adw::Toggle::builder().name("pages").label(t::linux_reader_pages()).build());
        tabs.add(adw::Toggle::builder().name("chapters").label(t::reader_chapters()).build());
        tabs.add_css_class("round");
        tabs.set_valign(gtk::Align::Center);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        top.append(&title);
        top.append(&tabs);

        let model = gtk::StringList::new(&[]);
        let factory = gtk::SignalListItemFactory::new();
        let grid = gtk::GridView::new(
            Some(gtk::NoSelection::new(Some(model.clone()))),
            Some(factory.clone()),
        );
        grid.set_min_columns(5);
        grid.set_max_columns(5);
        grid.set_single_click_activate(true);
        grid.add_css_class("reader-pages");
        let pages = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&grid)
            .vexpand(true)
            .build();

        let entries = gio::ListStore::new::<glib::BoxedAnyObject>();
        let rows = gtk::SignalListItemFactory::new();
        let list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(entries.clone()))),
            Some(rows.clone()),
        );
        list.set_single_click_activate(true);
        list.add_css_class("reader-chapter-list");
        let chapters = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .vexpand(true)
            .build();

        let stack = gtk::Stack::new();
        stack.add_named(&pages, Some("pages"));
        stack.add_named(&chapters, Some("chapters"));
        let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
        column.add_css_class("reader-chapters");
        column.set_size_request(520, 480);
        column.append(&top);
        column.append(&stack);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&column));

        let this = Rc::new(Self {
            popover,
            title,
            tabs,
            stack,
            model,
            grid,
            list,
            entries,
            open_folds: Rc::default(),
            volumes: RefCell::default(),
            here: RefCell::default(),
            here_row: RefCell::default(),
            centring: Cell::new(false),
            open_leaf: Cell::new(0),
            files: RefCell::default(),
            waiting: RefCell::default(),
            ask: RefCell::default(),
        });
        this.tabs.connect_active_name_notify({
            let stack = this.stack.clone();
            move |tabs| {
                if let Some(name) = tabs.active_name() {
                    stack.set_visible_child_name(&name);
                }
            }
        });
        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            let picture = gtk::Picture::new();
            picture.set_content_fit(gtk::ContentFit::Cover);
            picture.set_can_shrink(true);
            picture
                .set_size_request(THUMB_WIDTH, (f64::from(THUMB_WIDTH) / 0.7).round() as i32);
            picture.add_css_class("reader-thumb");
            picture.set_overflow(gtk::Overflow::Hidden);
            let number = gtk::Label::new(None);
            number.add_css_class("numeric");
            number.add_css_class("caption");
            number.add_css_class("haze");
            let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
            cell.append(&picture);
            cell.append(&number);
            item.set_child(Some(&cell));
        });
        factory.connect_bind({
            let this = Rc::downgrade(&this);
            move |_, item| {
                let (Some(this), Some(item)) =
                    (this.upgrade(), item.downcast_ref::<gtk::ListItem>())
                else {
                    return;
                };
                this.bind(item);
            }
        });
        rows.connect_bind({
            let this = Rc::downgrade(&this);
            move |_, item| {
                let (Some(this), Some(item)) =
                    (this.upgrade(), item.downcast_ref::<gtk::ListItem>())
                else {
                    return;
                };
                let Some(entry) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
                    return;
                };
                let entry = entry.borrow::<Entry>();
                // A chapter is the list's own row; a folded volume's row is inside its line.
                let take = matches!(*entry, Entry::Chapter { .. });
                item.set_activatable(take);
                item.set_focusable(take);
                item.set_accessible_label(&entry_label(&entry));
                let widget = match &*entry {
                    Entry::Fold { group, name, line } => this.fold(*group, name, line),
                    other => entry_widget(other),
                };
                if matches!(*entry, Entry::Chapter { here: true, .. }) {
                    this.here_row.replace(widget.downgrade());
                }
                item.set_child(Some(&widget));
            }
        });
        rows.connect_unbind(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                item.set_child(gtk::Widget::NONE);
            }
        });
        this.list.connect_activate({
            let this = Rc::downgrade(&this);
            move |_, position| {
                if let Some(this) = this.upgrade() {
                    this.picked(position);
                }
            }
        });
        // The chapters tab shown: a manga's list opens on its open chapter as the tab does.
        this.list.connect_map({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade()
                    && this.centring.get()
                {
                    this.centre_when_laid_out();
                }
            }
        });
        this.grid.connect_activate({
            let this = Rc::downgrade(&this);
            move |_, position| {
                let Some(this) = this.upgrade() else { return };
                let entry = this.model.string(position).and_then(|s| entry(&s));
                if let Some((leaf, _)) = entry {
                    this.popover.popdown();
                    this.tell(Ask::Jump(leaf));
                }
            }
        });
        this
    }

    pub fn set_ask(&self, ask: Asker) {
        self.ask.replace(Some(ask));
    }

    fn tell(&self, what: Ask) {
        let ask = self.ask.borrow().clone();
        if let Some(ask) = ask {
            ask(what);
        }
    }

    /// Fill the popover for where the reader is, as it opens.
    pub fn show_here(self: &Rc<Self>, here: Here) {
        self.title.set_label(&here.label);
        self.here.replace(Some(here.chapter));
        self.open_leaf.set(here.open_leaf);
        self.files.replace(here.files);
        self.waiting.borrow_mut().clear();
        self.tabs.set_visible(here.pages);
        self.grid.set_direction(if here.rtl {
            gtk::TextDirection::Rtl
        } else {
            gtk::TextDirection::Ltr
        });
        let entries = page_entries(&here.leaves);
        let strings: Vec<String> = entries.iter().map(|(l, p)| format!("{l}:{p}")).collect();
        let strings: Vec<&str> = strings.iter().map(String::as_str).collect();
        self.model.splice(0, self.model.n_items(), &strings);
        // Pages first for manga, the chapters for a novel.
        let tab = if here.pages { "pages" } else { "chapters" };
        self.tabs.set_active_name(Some(tab));
        self.stack.set_visible_child_name(tab);
        self.open_folds.borrow_mut().clear();
        self.draw_list();
        // The open page in view before the grid is drawn.
        if let Some(open) = entries.iter().position(|(l, _)| *l == here.open_leaf) {
            self.grid.scroll_to(open as u32, gtk::ListScrollFlags::NONE, None);
        }
        // The open chapter in the middle of the list before it is drawn, so nothing scrolls:
        // now when the list shows, else when its tab does.
        self.centring.set(true);
        if let Some(here) = self.here_position() {
            self.list.scroll_to(here, gtk::ListScrollFlags::NONE, None);
        }
        if self.list.is_mapped() {
            self.centre_when_laid_out();
        }
    }

    /// Where the open chapter's row is in the list.
    fn here_position(&self) -> Option<u32> {
        (0..self.entries.n_items()).find(|&i| {
            self.entries.item(i).and_downcast::<glib::BoxedAnyObject>().is_some_and(|e| {
                matches!(*e.borrow::<Entry>(), Entry::Chapter { here: true, .. })
            })
        })
    }

    /// Centre the open chapter once the list is laid out and before the frame is painted: GDK
    /// lays a frame out again, up to four times, while a layout asks for another, so the frame
    /// painted has the row in place. A popover's surface maps a few layouts after the widget
    /// does, and GTK places nothing until then, so those layouts only wait.
    fn centre_when_laid_out(self: &Rc<Self>) {
        let Some(clock) = self.list.frame_clock() else { return };
        let handler: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::default();
        let moves = Cell::new(0u8);
        let this = Rc::downgrade(self);
        // After GTK's own handler, which allocates the popover and binds the rows.
        let id = clock.connect_local("layout", true, {
            let handler = handler.clone();
            let clock = clock.clone();
            move |_| {
                let step = match this.upgrade() {
                    // Closed before it was laid out: the next opening starts again.
                    Some(this) if this.list.is_mapped() => this.centre_here(),
                    _ => Centring::Done,
                };
                if step == Centring::Moved {
                    moves.set(moves.get() + 1);
                }
                if (step == Centring::Done || moves.get() >= CENTRE_MOVES)
                    && let Some(id) = handler.take()
                {
                    clock.disconnect(id);
                }
                None
            }
        });
        handler.replace(Some(id));
        clock.request_phase(gdk::FrameClockPhase::LAYOUT);
    }

    /// One step of centring the open chapter's row. Once it is as near the middle as the list's
    /// ends allow, the row takes the focus.
    fn centre_here(&self) -> Centring {
        if !self.centring.get() {
            return Centring::Done;
        }
        let Some(here) = self.here_position() else {
            // Still loading: the chapters coming in look again. Not listed: the list stays at
            // its top.
            if self.volumes.borrow().is_some() {
                self.centring.set(false);
            }
            return Centring::Done;
        };
        let Some(row) = self.here_row.borrow().upgrade() else {
            // Not bound yet: bring it into the list and look again on the next layout.
            self.list.scroll_to(here, gtk::ListScrollFlags::NONE, None);
            return Centring::Waiting;
        };
        let (Some(bounds), Some(adjustment)) =
            (row.compute_bounds(&self.list), self.list.vadjustment())
        else {
            return Centring::Waiting;
        };
        let page = adjustment.page_size();
        // A row GTK has not placed yet has no size, and the scroll position does not say where
        // it is. A scroll asked for before the list was laid out is dropped, so it is asked
        // again until the row is in view.
        if page <= 0.0 || !row.is_mapped() || bounds.height() <= 0.0 {
            self.list.scroll_to(here, gtk::ListScrollFlags::NONE, None);
            return Centring::Waiting;
        }
        let middle = f64::from(bounds.y() + bounds.height() / 2.0);
        let to = (adjustment.value() + middle - page / 2.0)
            .clamp(adjustment.lower(), (adjustment.upper() - page).max(adjustment.lower()));
        if (to - adjustment.value()).abs() >= 1.0 {
            adjustment.set_value(to);
            return Centring::Moved;
        }
        self.centring.set(false);
        // Keyboard focus and the screen reader start on the open chapter; it is in view, so this
        // moves nothing.
        self.list.scroll_to(here, gtk::ListScrollFlags::FOCUS, None);
        Centring::Done
    }

    /// Show `tab` for a shot.
    #[cfg(feature = "fixtures")]
    pub fn shot_tab(&self, tab: &str) {
        self.tabs.set_active_name(Some(tab));
        self.stack.set_visible_child_name(tab);
    }

    /// The series' chapters as the engine last listed them.
    pub fn set_volumes(self: &Rc<Self>, volumes: Vec<VolumeGroup>) {
        self.volumes.replace(Some(volumes));
        if self.popover.is_visible() {
            self.draw_list();
            // Chapters listed after the popover opened: the list opens on the open one now.
            if self.centring.get() && self.list.is_mapped() {
                self.centre_when_laid_out();
            }
        }
    }

    /// Forget the series: another work opened.
    pub fn clear(&self) {
        self.volumes.replace(None);
        self.here.replace(None);
        self.centring.set(false);
        self.files.borrow_mut().clear();
        self.waiting.borrow_mut().clear();
        self.model.splice(0, self.model.n_items(), &[]);
        self.entries.remove_all();
        self.open_folds.borrow_mut().clear();
    }

    /// A page landed: thumbnails waiting for it show it.
    pub fn page_ready(&self, page: u32, file: &PageFile) {
        self.files.borrow_mut().insert(page, file.clone());
        let waiting = self.waiting.borrow_mut().remove(&page).unwrap_or_default();
        for picture in waiting.iter().filter_map(glib::WeakRef::upgrade) {
            show_thumb(&picture, file.clone());
        }
    }

    fn bind(&self, item: &gtk::ListItem) {
        let Some((leaf, page)) =
            item.item().and_downcast::<gtk::StringObject>().and_then(|s| entry(&s.string()))
        else {
            return;
        };
        let Some(cell) = item.child().and_downcast::<gtk::Box>() else { return };
        let (Some(picture), Some(number)) = (
            cell.first_child().and_downcast::<gtk::Picture>(),
            cell.last_child().and_downcast::<gtk::Label>(),
        ) else {
            return;
        };
        number.set_label(&(page + 1).to_string());
        if leaf == self.open_leaf.get() {
            picture.add_css_class("open");
        } else {
            picture.remove_css_class("open");
        }
        picture.set_paintable(gdk::Paintable::NONE);
        let file = self.files.borrow().get(&page).cloned();
        match file {
            Some(file) => show_thumb(&picture, file),
            None => {
                self.waiting.borrow_mut().entry(page).or_default().push(picture.downgrade());
                self.tell(Ask::Need(page));
            }
        }
    }

    fn draw_list(&self) {
        let volumes = self.volumes.borrow().clone();
        let entries = match volumes {
            Some(volumes) => entries(&volumes, self.here.borrow().as_ref()),
            None => vec![Entry::Loading],
        };
        let entries: Vec<glib::BoxedAnyObject> =
            entries.into_iter().map(glib::BoxedAnyObject::new).collect();
        self.entries.splice(0, self.entries.n_items(), &entries);
    }

    /// A row was picked: a chapter opens, a read volume folds open or shut.
    fn picked(&self, position: u32) {
        let Some(entry) = self.entries.item(position).and_downcast::<glib::BoxedAnyObject>()
        else {
            return;
        };
        let entry = entry.borrow::<Entry>().clone();
        match entry {
            Entry::Chapter { row, here, .. } => {
                self.popover.popdown();
                if !here {
                    self.tell(Ask::Open(row.id));
                }
            }
            Entry::Fold { .. } | Entry::Heading(_) | Entry::Loading => {}
        }
    }

    /// A read volume folded into one line that opens to its chapters, drawn the first time it
    /// opens. It stays open when its line scrolls away and back.
    fn fold(&self, group: usize, name: &str, line: &str) -> gtk::Widget {
        let fold = adw::ExpanderRow::builder()
            .title(glib::markup_escape_text(name))
            .subtitle(glib::markup_escape_text(line))
            .build();
        let chapters = self
            .volumes
            .borrow()
            .as_ref()
            .and_then(|v| v.get(group))
            .map(|g| g.chapters.clone())
            .unwrap_or_default();
        let filled = Cell::new(false);
        let (popover, ask, open) =
            (self.popover.downgrade(), self.ask.borrow().clone(), Rc::clone(&self.open_folds));
        fold.connect_expanded_notify(move |fold| {
            if fold.is_expanded() {
                open.borrow_mut().insert(group);
            } else {
                open.borrow_mut().remove(&group);
            }
            if fold.is_expanded() && !filled.replace(true) {
                for chapter in &chapters {
                    fold.add_row(&fold_row(chapter, &popover, ask.clone()));
                }
            }
        });
        // Before it is shown, so it opens without the slide.
        fold.set_expanded(self.open_folds.borrow().contains(&group));
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("reader-chapter-list");
        list.append(&fold);
        list.upcast()
    }
}

/// A chapter in an opened volume: picking it opens it.
fn fold_row(
    chapter: &ChapterRow,
    popover: &glib::WeakRef<gtk::Popover>,
    ask: Option<Asker>,
) -> adw::ActionRow {
    let read = matches!(chapter.state, ChapterReadState::Read);
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&chapter.label))
        .subtitle(glib::markup_escape_text(&under_line(chapter)))
        .title_lines(1)
        .subtitle_lines(1)
        .activatable(true)
        .build();
    if read {
        row.add_css_class("reader-read");
    }
    // Every row keeps the dot's place, so titles line up with and without it.
    let slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    slot.set_width_request(6);
    slot.set_valign(gtk::Align::Center);
    if chapter.new_chapter && !read {
        slot.add_css_class("home-new");
    }
    row.add_prefix(&slot);
    let (id, popover) = (chapter.id.clone(), popover.clone());
    row.connect_activated(move |_| {
        if let Some(popover) = popover.upgrade() {
            popover.popdown();
        }
        if let Some(ask) = &ask {
            ask(Ask::Open(id.clone()));
        }
    });
    row
}

/// A chapter's title and state, under its words.
fn under_line(chapter: &ChapterRow) -> String {
    let mut under: Vec<String> = Vec::new();
    if let Some(title) = chapter.title.as_deref().filter(|t| !t.is_empty()) {
        under.push(title.to_owned());
    }
    let state = super::series::state_line(chapter);
    if !state.is_empty() {
        under.push(state);
    }
    under.join(" \u{b7} ")
}

/// A line of the Chapters list.
#[derive(Clone)]
enum Entry {
    /// The chapters have not come yet.
    Loading,
    /// A volume's name, when the series has more than one.
    Heading(String),
    /// A read volume folded into one line, unless the open chapter is in it.
    Fold {
        group: usize,
        name: String,
        line: String,
    },
    Chapter {
        row: Box<ChapterRow>,
        here: bool,
    },
}

/// The lines for `volumes`, with the open chapter `here` marked.
fn entries(volumes: &[VolumeGroup], here: Option<&ChapterId>) -> Vec<Entry> {
    let mut entries = Vec::new();
    for (index, group) in volumes.iter().enumerate() {
        let name = group.volume.as_deref().map_or_else(t::series_no_volume, t::series_volume);
        let holds_here = group.chapters.iter().any(|c| Some(&c.id) == here);
        if group.all_read && volumes.len() > 1 && !holds_here {
            let mut line = Vec::new();
            if let (Some(low), Some(high)) = (group.chapters.first(), group.chapters.last()) {
                let number =
                    |c: &ChapterRow| c.number.clone().unwrap_or_else(|| c.label.clone());
                line.push(t::series_chapter_range(&number(low), &number(high)));
            }
            line.push(t::series_volume_read());
            let line = line.join(" \u{b7} ");
            entries.push(Entry::Fold { group: index, name, line });
            continue;
        }
        if volumes.len() > 1 {
            entries.push(Entry::Heading(name));
        }
        entries.extend(group.chapters.iter().map(|chapter| Entry::Chapter {
            row: Box::new(chapter.clone()),
            here: Some(&chapter.id) == here,
        }));
    }
    entries
}

/// What a line shows. A chapter: its words, its title and state under them, the open one on the
/// selected fill and a new one with the water dot in a place every row keeps. No icon on any
/// row. A folded volume: its name and range, with the arrow that says it opens.
fn entry_widget(entry: &Entry) -> gtk::Widget {
    match entry {
        Entry::Loading => {
            let loading = crate::widgets::haze(&t::reader_chapters_loading(), "body");
            loading.set_margin_top(16);
            loading.upcast()
        }
        Entry::Heading(name) => {
            let heading = crate::widgets::haze(name, "caption-heading");
            heading.set_margin_top(14);
            heading.set_margin_bottom(6);
            heading.set_accessible_role(gtk::AccessibleRole::Heading);
            heading.upcast()
        }
        // Drawn by the popover, which opens it: see `Chapters::fold`.
        Entry::Fold { name, .. } => crate::widgets::haze(name, "body").upcast(),
        Entry::Chapter { row: chapter, here } => {
            let read = matches!(chapter.state, ChapterReadState::Read);
            let row = two_lines(&chapter.label, &under_line(chapter));
            // Every row keeps the dot's place, so titles line up with and without it.
            let slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            slot.set_width_request(6);
            slot.set_valign(gtk::Align::Center);
            if chapter.new_chapter && !read {
                slot.add_css_class("home-new");
            }
            row.prepend(&slot);
            if read {
                row.add_css_class("reader-read");
            }
            if *here {
                row.add_css_class("reader-here");
                row.update_state(&[gtk::accessible::State::Selected(Some(true))]);
            }
            row.upcast()
        }
    }
}

/// What a screen reader says for a line: its words, as the row shows them.
fn entry_label(entry: &Entry) -> String {
    match entry {
        Entry::Loading => t::reader_chapters_loading(),
        Entry::Heading(name) => name.clone(),
        Entry::Fold { name, line, .. } => [name.as_str(), line].join(", "), // i18n-skip
        Entry::Chapter { row, .. } => {
            let under = under_line(row);
            if under.is_empty() {
                row.label.clone()
            } else {
                [row.label.as_str(), &under].join(", ") // i18n-skip
            }
        }
    }
}

/// A row with a title and, when there is one, a line under it, each cut to one line.
fn two_lines(title: &str, line: &str) -> gtk::Box {
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_valign(gtk::Align::Center);
    words.set_hexpand(true);
    let title = gtk::Label::new(Some(title));
    title.add_css_class("reader-chapter-title");
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&title);
    if !line.is_empty() {
        let line = gtk::Label::new(Some(line));
        line.add_css_class("reader-chapter-line");
        line.set_xalign(0.0);
        line.set_ellipsize(gtk::pango::EllipsizeMode::End);
        words.append(&line);
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("reader-chapter");
    row.append(&words);
    row
}

/// Every page of the chapter as the person counts them, with the leaf it is on: both halves of
/// a spread are here, the end page is not.
fn page_entries(leaves: &[Leaf]) -> Vec<(usize, u32)> {
    let mut all: Vec<(usize, u32)> = leaves
        .iter()
        .enumerate()
        .filter(|(_, leaf)| !leaf.end_page)
        .flat_map(|(index, leaf)| leaf.parts.iter().map(move |part| (index, part.page)))
        .collect();
    all.sort_by_key(|(_, page)| *page);
    all.dedup_by_key(|(_, page)| *page);
    all
}

/// A grid entry's leaf and page, from its string.
fn entry(text: &str) -> Option<(usize, u32)> {
    let (leaf, page) = text.split_once(':')?;
    Some((leaf.parse().ok()?, page.parse().ok()?))
}

/// Decode `file` small, off the main thread, and show it in `picture` if it still wants it.
fn show_thumb(picture: &gtk::Picture, file: PageFile) {
    let picture = picture.downgrade();
    glib::spawn_future_local(async move {
        let decoded = gio::spawn_blocking(move || thumbnail(&file, THUMB_PIXELS)).await;
        let (Ok(Some((width, height, rgba))), Some(picture)) = (decoded, picture.upgrade())
        else {
            return;
        };
        let texture: gdk::Texture = gdk::MemoryTexture::new(
            width as i32,
            height as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(rgba),
            width as usize * 4,
        )
        .upcast();
        picture.set_paintable(Some(&texture));
        crate::motion::fade_in(&picture);
    });
}

/// A page shrunk to `width` pixels wide, as RGBA rows: the original file decoded, or the
/// engine's raw pixels read past their stride.
fn thumbnail(file: &PageFile, width: u32) -> Option<(u32, u32, Vec<u8>)> {
    let image = match &file.encoding {
        PageEncoding::Original { .. } => image::ImageReader::open(&file.path)
            .ok()?
            .with_guessed_format()
            .ok()?
            .decode()
            .ok()?,
        PageEncoding::Raw { format, stride } => {
            let bytes = std::fs::read(&file.path).ok()?;
            let channels = match format {
                PixelFormat::Gray8 => 1,
                PixelFormat::Rgba8 => 4,
            };
            let row = file.width as usize * channels;
            let mut packed = Vec::with_capacity(row * file.height as usize);
            for y in 0..file.height as usize {
                let start = y * *stride as usize;
                packed.extend_from_slice(bytes.get(start..start + row)?);
            }
            match format {
                PixelFormat::Gray8 => image::DynamicImage::ImageLuma8(
                    image::GrayImage::from_raw(file.width, file.height, packed)?,
                ),
                PixelFormat::Rgba8 => image::DynamicImage::ImageRgba8(
                    image::RgbaImage::from_raw(file.width, file.height, packed)?,
                ),
            }
        }
    };
    // With crop borders on, the original comes with the part to show.
    let image = match file.crop {
        Some(c)
            if c.width > 0
                && c.height > 0
                && c.x + c.width <= image.width()
                && c.y + c.height <= image.height() =>
        {
            image.crop_imm(c.x, c.y, c.width, c.height)
        }
        _ => image,
    };
    let small = image.thumbnail(width, width.saturating_mul(4)).into_rgba8();
    let (w, h) = small.dimensions();
    Some((w, h, small.into_raw()))
}

#[cfg(test)]
mod tests {
    use kaeri_model::reader::{
        Leaf, LeafPart, PageEncoding, PageFile, PageKind, PixelFormat, PixelRect, UnitRect,
    };

    use super::{entry, page_entries, thumbnail};

    fn leaf(index: u32, pages: &[u32], end_page: bool) -> Leaf {
        let part = |page| LeafPart {
            page,
            kind: PageKind::Story,
            file: None,
            place: UnitRect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
            rotate: false,
            half: None,
            ratio: None,
            strip: None,
        };
        Leaf { index, parts: pages.iter().map(|&page| part(page)).collect(), end_page }
    }

    fn raw(
        path: &std::path::Path,
        format: PixelFormat,
        stride: u32,
        w: u32,
        h: u32,
    ) -> PageFile {
        PageFile {
            path: path.to_string_lossy().into_owned(),
            encoding: PageEncoding::Raw { format, stride },
            width: w,
            height: h,
            crop: None,
            colour: false,
            bytes: 0,
        }
    }

    #[test]
    fn every_page_once_in_order_with_its_leaf_and_no_end_page() {
        let leaves = [leaf(0, &[0], false), leaf(1, &[2, 1], false), leaf(2, &[], true)];
        assert_eq!(page_entries(&leaves), vec![(0, 0), (1, 1), (1, 2)]);
    }

    #[test]
    fn an_entry_reads_back() {
        assert_eq!(entry("3:17"), Some((3, 17)));
        assert_eq!(entry("3"), None);
        assert_eq!(entry("a:1"), None);
    }

    #[test]
    fn raw_pages_shrink_past_their_stride() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.raw");
        // 400 by 600 grey, each row padded to 416 bytes.
        let (w, h, stride) = (400_u32, 600_u32, 416_u32);
        let mut bytes = vec![0_u8; (stride * h) as usize];
        for y in 0..h as usize {
            for x in 0..w as usize {
                bytes[y * stride as usize + x] = 200;
            }
        }
        std::fs::write(&path, &bytes).unwrap();
        let file = raw(&path, PixelFormat::Gray8, stride, w, h);
        let (tw, th, rgba) = thumbnail(&file, 168).unwrap();
        assert_eq!((tw, th), (168, 252));
        assert_eq!(rgba.len(), (168 * 252 * 4) as usize);
        assert_eq!(&rgba[..4], &[200, 200, 200, 255]);
    }

    #[test]
    fn an_original_shows_only_its_crop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.png");
        // White borders 40 pixels wide round a dark page.
        let page = image::RgbImage::from_fn(400, 600, |x, y| {
            let inside = (40..360).contains(&x) && (40..560).contains(&y);
            if inside { image::Rgb([30, 30, 30]) } else { image::Rgb([255, 255, 255]) }
        });
        page.save(&path).unwrap();
        let file = PageFile {
            path: path.display().to_string(),
            encoding: PageEncoding::Original { mime: "image/png".into() },
            width: 400,
            height: 600,
            crop: Some(PixelRect { x: 40, y: 40, width: 320, height: 520 }),
            colour: false,
            bytes: 0,
        };
        let (tw, th, rgba) = thumbnail(&file, 160).unwrap();
        assert_eq!((tw, th), (160, 260));
        // The corner is the page, not its border.
        assert_eq!(&rgba[..3], &[30, 30, 30]);
    }

    #[test]
    fn a_missing_or_short_page_has_no_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("short.raw");
        std::fs::write(&path, [0_u8; 10]).unwrap();
        let file = raw(&path, PixelFormat::Rgba8, 40, 10, 10);
        assert!(thumbnail(&file, 168).is_none());
    }
}

#[cfg(test)]
mod centring {
    use std::{cell::Cell, rc::Rc};

    use adw::prelude::*;
    use kaeri_core::books::ChapterId;
    use kaeri_model::reading::{ChapterReadState, ChapterRow, VolumeGroup};

    use super::{Chapters, Here};
    use crate::gtk_test::{pump, run};

    fn chapter(n: usize) -> ChapterRow {
        ChapterRow {
            id: ChapterId(format!("c{n}")),
            label: format!("Chapter {n}"), // i18n-skip
            state: ChapterReadState::Unread,
            number: Some(n.to_string()),
            title: None,
            volume: None,
            published: None,
            pages: None,
            new_chapter: false,
            download: None,
            versions: 1,
            bookmarked: false,
            version_label: None,
            read_at: None,
            line: Vec::new(),
            gone_from_source: false,
            parts: 1,
            missing_parts: Vec::new(),
            part_groups: Vec::new(),
            other_group: false,
            group_language: None,
            other_source: None,
        }
    }

    /// The popover's content opened on chapter `here` of `count`, filled before it shows, as
    /// the reader fills the popover. Drawn in a window of its own, since a headless X server
    /// shows no popover. Gives the list's scroll position when the first frame was painted.
    fn open(count: usize, here: usize) -> (gtk::Window, Rc<Chapters>, Rc<Cell<Option<f64>>>) {
        let chapters = Chapters::new();
        chapters.set_volumes(vec![VolumeGroup {
            volume: None,
            title: None,
            chapters: (1..=count).map(chapter).collect(),
            all_read: false,
            flat: false,
        }]);
        let content = chapters.popover.child().expect("the popover has its content");
        chapters.popover.set_child(gtk::Widget::NONE);
        let window = gtk::Window::builder().default_width(700).default_height(700).build();
        window.set_child(Some(&content));
        chapters.show_here(Here {
            chapter: ChapterId(format!("c{here}")),
            label: format!("Chapter {here}"), // i18n-skip
            pages: false,
            rtl: false,
            leaves: Vec::new(),
            open_leaf: 0,
            files: Default::default(),
        });
        let painted: Rc<Cell<Option<f64>>> = Rc::default();
        chapters.list.connect_map({
            let painted = painted.clone();
            move |list| {
                let Some(clock) = list.frame_clock() else { return };
                let list = list.clone();
                let painted = painted.clone();
                clock.connect_local("after-paint", false, move |_| {
                    if painted.get().is_none() {
                        painted.set(list.vadjustment().map(|a| a.value()));
                    }
                    None
                });
            }
        });
        window.present();
        pump(400);
        (window, chapters, painted)
    }

    fn adjustment(chapters: &Chapters) -> gtk::Adjustment {
        chapters.list.vadjustment().expect("the list scrolls")
    }

    #[test]
    fn the_open_chapter_opens_in_the_middle_with_nothing_scrolled() {
        run(|| {
            let (window, chapters, painted) = open(120, 60);
            let row = chapters.here_row.borrow().upgrade().expect("the open chapter is drawn");
            let bounds = row.compute_bounds(&chapters.list).expect("the row is laid out");
            let page = adjustment(&chapters).page_size();
            let middle = f64::from(bounds.y() + bounds.height() / 2.0);
            assert!(
                (middle - page / 2.0).abs() <= 2.0,
                "the row's middle is {middle} of {page}"
            );
            // The first frame painted already had it there.
            assert_eq!(painted.get(), Some(adjustment(&chapters).value()));
            // Keyboard focus and the screen reader start on it.
            assert!(
                row.parent().is_some_and(|item| item.is_focus()),
                "the open chapter has the focus"
            );
            window.destroy();
        });
    }

    #[test]
    fn a_list_that_fits_stays_at_its_top_and_the_last_chapter_sits_at_the_end() {
        run(|| {
            let (window, chapters, _) = open(5, 3);
            assert_eq!(adjustment(&chapters).value(), 0.0);
            window.destroy();
            let (window, chapters, painted) = open(120, 120);
            let a = adjustment(&chapters);
            assert!(
                (a.value() - (a.upper() - a.page_size())).abs() <= 1.0,
                "{} of {}",
                a.value(),
                a.upper()
            );
            assert_eq!(painted.get(), Some(a.value()));
            window.destroy();
        });
    }
}
