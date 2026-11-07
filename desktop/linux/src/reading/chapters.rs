//! The reader's Chapters popover, as the reader board draws it for the desktop and as Apple's and
//! Android's sheets have it: for manga the chapter's pages in a grid of five with the open one
//! ringed, laid out from the right in a right-to-left book, and the series' chapters by volume
//! with the open one on the selected fill. A novel has the chapters alone. Picking a page or a
//! chapter goes there and closes it.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use kaeri_core::books::ChapterId;
use kaeri_i18n::t;
use kaeri_model::reader::{Leaf, PageEncoding, PageFile, PixelFormat};
use kaeri_model::reading::{ChapterReadState, ChapterRow, VolumeGroup};

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

pub struct Chapters {
    pub popover: gtk::Popover,
    title: gtk::Label,
    tabs: adw::ToggleGroup,
    stack: gtk::Stack,
    model: gtk::StringList,
    grid: gtk::GridView,
    list: gtk::Box,
    volumes: RefCell<Option<Vec<VolumeGroup>>>,
    here: RefCell<Option<ChapterId>>,
    open_leaf: Cell<usize>,
    files: RefCell<HashMap<u32, PageFile>>,
    /// Thumbnails waiting for their page, by page.
    waiting: RefCell<HashMap<u32, Vec<glib::WeakRef<gtk::Picture>>>>,
    ask: RefCell<Option<Rc<dyn Fn(Ask)>>>,
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

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
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
            volumes: RefCell::default(),
            here: RefCell::default(),
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

    pub fn set_ask(&self, ask: Rc<dyn Fn(Ask)>) {
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
        self.draw_list();
        // The open page and the open chapter in view.
        let open = entries.iter().position(|(l, _)| *l == here.open_leaf);
        let this = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(this) = this.upgrade() else { return };
            if let Some(open) = open {
                this.grid.scroll_to(open as u32, gtk::ListScrollFlags::NONE, None);
            }
            if let Some(row) = find_here(this.list.upcast_ref()) {
                row.grab_focus();
            }
        });
    }

    /// Show `tab` for a shot.
    #[cfg(feature = "fixtures")]
    pub fn shot_tab(&self, tab: &str) {
        self.tabs.set_active_name(Some(tab));
        self.stack.set_visible_child_name(tab);
    }

    /// The series' chapters as the engine last listed them.
    pub fn set_volumes(&self, volumes: Vec<VolumeGroup>) {
        self.volumes.replace(Some(volumes));
        if self.popover.is_visible() {
            self.draw_list();
        }
    }

    /// Forget the series: another work opened.
    pub fn clear(&self) {
        self.volumes.replace(None);
        self.here.replace(None);
        self.files.borrow_mut().clear();
        self.waiting.borrow_mut().clear();
        self.model.splice(0, self.model.n_items(), &[]);
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
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let volumes = self.volumes.borrow().clone();
        let Some(volumes) = volumes else {
            let loading = crate::widgets::haze(&t::reader_chapters_loading(), "body");
            loading.set_margin_top(16);
            self.list.append(&loading);
            return;
        };
        let here = self.here.borrow().clone();
        for group in &volumes {
            let name =
                group.volume.as_deref().map_or_else(t::series_no_volume, t::series_volume);
            let list = gtk::ListBox::new();
            list.set_selection_mode(gtk::SelectionMode::None);
            list.add_css_class("reader-chapter-list");
            // A read volume folds into one line, unless the open chapter is in it.
            let holds_here = group.chapters.iter().any(|c| Some(&c.id) == here.as_ref());
            if group.all_read && volumes.len() > 1 && !holds_here {
                let mut line = Vec::new();
                if let (Some(low), Some(high)) = (group.chapters.first(), group.chapters.last())
                {
                    let number =
                        |c: &ChapterRow| c.number.clone().unwrap_or_else(|| c.label.clone());
                    line.push(t::series_chapter_range(&number(low), &number(high)));
                }
                line.push(t::series_volume_read());
                let fold = adw::ExpanderRow::builder()
                    .title(glib::markup_escape_text(&name))
                    .subtitle(glib::markup_escape_text(&line.join(" \u{b7} ")))
                    .build();
                for chapter in &group.chapters {
                    fold.add_row(&self.row(chapter, false));
                }
                list.append(&fold);
                self.list.append(&list);
                continue;
            }
            if volumes.len() > 1 {
                let heading = crate::widgets::haze(&name, "caption-heading");
                heading.set_margin_top(14);
                heading.set_margin_bottom(6);
                heading.set_accessible_role(gtk::AccessibleRole::Heading);
                self.list.append(&heading);
            }
            for chapter in &group.chapters {
                list.append(&self.row(chapter, Some(&chapter.id) == here.as_ref()));
            }
            self.list.append(&list);
        }
    }

    /// A chapter: its words, its title and state under them, the open one on the selected fill
    /// and a new one with the water dot. No icon on any row.
    fn row(&self, chapter: &ChapterRow, here: bool) -> adw::ActionRow {
        let read = matches!(chapter.state, ChapterReadState::Read);
        let mut under: Vec<String> = Vec::new();
        if let Some(title) = chapter.title.as_deref().filter(|t| !t.is_empty()) {
            under.push(title.to_owned());
        }
        let state = super::series::state_line(chapter);
        if !state.is_empty() {
            under.push(state);
        }
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&chapter.label))
            .subtitle(glib::markup_escape_text(&under.join(" \u{b7} ")))
            .title_lines(1)
            .subtitle_lines(1)
            .activatable(true)
            .build();
        if read {
            row.add_css_class("reader-read");
        }
        if here {
            row.add_css_class("reader-here");
            row.update_state(&[gtk::accessible::State::Selected(Some(true))]);
        }
        if chapter.new_chapter && !read {
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("home-new");
            dot.set_valign(gtk::Align::Center);
            row.add_prefix(&dot);
        }
        let id = chapter.id.clone();
        let popover = self.popover.downgrade();
        let ask = self.ask.borrow().clone();
        row.connect_activated(move |_| {
            if let Some(popover) = popover.upgrade() {
                popover.popdown();
            }
            if !here && let Some(ask) = &ask {
                ask(Ask::Open(id.clone()));
            }
        });
        row
    }
}

/// The open chapter's row among `widget` and what it holds.
fn find_here(widget: &gtk::Widget) -> Option<gtk::Widget> {
    if widget.has_css_class("reader-here") {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(found) = find_here(&c) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
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
    let small = image.thumbnail(width, width.saturating_mul(4)).into_rgba8();
    let (w, h) = small.dimensions();
    Some((w, h, small.into_raw()))
}

#[cfg(test)]
mod tests {
    use kaeri_model::reader::{
        Leaf, LeafPart, PageEncoding, PageFile, PageKind, PixelFormat, UnitRect,
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
    fn a_missing_or_short_page_has_no_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("short.raw");
        std::fs::write(&path, [0_u8; 10]).unwrap();
        let file = raw(&path, PixelFormat::Rgba8, 40, 10, 10);
        assert!(thumbnail(&file, 168).is_none());
    }
}
