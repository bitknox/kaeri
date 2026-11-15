//! Reader pages made for the screen.
//!
//! A page is kept whole once it is decoded ([`ImageStore::insert_page`]). What the terminal
//! draws is that page made for one box of cells: stretched to the box, which the reader shaped
//! from the page, and encoded for the terminal's protocol once. A paged leaf draws its pages
//! whole. A page at fit width, zoomed, or in a strip is encoded as one tall picture in rows,
//! and a frame draws only the rows in view, so a scroll step makes and sends no new picture.
//! A zoomed page wider than the view is made for the columns in view, so only moving across
//! makes it again.
//!
//! Making one is the slow part: a resize and a protocol encode. In the app it runs off the
//! event loop: a draw that finds nothing made for its box queues an [`EncodeJob`] and draws
//! what was made before where that still fits, and the loop runs the job on the blocking pool
//! and gives back its [`Encoded`] through [`ImageStore::encoded`]. Tests, the preview and the
//! harness make pictures inline, so one draw settles.
//!
//! The store also notes where pictures were drawn in each frame, so a terminal that leaves
//! pixels behind can be cleared when a picture goes from a place the next frame does not
//! cover: see [`ImageStore::left_behind`].

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    sync::Arc,
};

use image::{DynamicImage, imageops::FilterType};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    widgets::Widget,
};
use ratatui_image::{
    Image, Resize,
    picker::{Picker, ProtocolType},
    protocol::{Protocol, halfblocks::Halfblocks},
    sliced::{SignedPosition, SlicedImage, SlicedProtocol},
};

use super::ImageStore;

/// The filter for the scaling left once a page was shrunk to its surface: lettering and
/// screentone stay clean, where nearest would alias them.
const FILTER: FilterType = FilterType::CatmullRom;

/// What a page is made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cut {
    /// The box, in cells.
    pub cols: u16,
    pub rows: u16,
    /// The page's columns of pixels the box shows, from and to, when only part of its width
    /// is in view (zoomed in); `None` is all of it.
    pub across: Option<(u32, u32)>,
    /// Made in rows, to draw only the rows in view.
    pub sliced: bool,
}

impl Cut {
    /// A page drawn whole in `cols` by `rows`.
    pub fn whole(cols: u16, rows: u16) -> Self {
        Self { cols, rows, across: None, sliced: false }
    }

    /// A page `cols` by `rows` drawn a few rows at a time, or only `across` its columns.
    pub fn sliced(cols: u16, rows: u16, across: Option<(u32, u32)>) -> Self {
        Self { cols, rows, across, sliced: true }
    }
}

/// A page made for the terminal.
enum Picture {
    Whole(Protocol),
    Sliced(SlicedProtocol),
}

/// What was made of a page last: for which box, from which decode, and the picture, or `None`
/// when it could not be made.
struct Made {
    cut: Cut,
    generation: u64,
    picture: Option<Picture>,
}

/// A page kept whole, and which decode under its key it is.
pub(super) struct Kept {
    pub(super) generation: u64,
    pub(super) image: Arc<DynamicImage>,
}

/// One page to make off the loop: [`EncodeJob::run`] it anywhere and hand what comes back to
/// [`ImageStore::encoded`].
pub struct EncodeJob {
    key: String,
    cut: Cut,
    generation: u64,
    page: Arc<DynamicImage>,
    picker: Picker,
}

impl EncodeJob {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn cut(&self) -> Cut {
        self.cut
    }

    /// Resize and encode.
    pub fn run(self) -> Encoded {
        let picture = make(&self.picker, &self.page, self.cut);
        Encoded { key: self.key, cut: self.cut, generation: self.generation, picture }
    }
}

/// A page made off the loop, for [`ImageStore::encoded`].
pub struct Encoded {
    key: String,
    cut: Cut,
    generation: u64,
    picture: Option<Picture>,
}

/// The made pages and the work on them.
#[derive(Default)]
pub(super) struct Pictures {
    made: RefCell<HashMap<String, Made>>,
    /// Handed out and not back yet, by key, box and decode.
    asked: RefCell<HashSet<(String, Cut, u64)>>,
    jobs: RefCell<Vec<EncodeJob>>,
    off_loop: Cell<bool>,
    generation: Cell<u64>,
    /// Where pictures were drawn in the frame before, and so far in this one.
    drawn: RefCell<(Vec<Rect>, Vec<Rect>)>,
}

impl Pictures {
    /// A decoded page, numbered so what was made from the one before it is known as old.
    pub(super) fn keep(&self, image: DynamicImage) -> Kept {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        Kept { generation, image: Arc::new(image) }
    }

    pub(super) fn forget_where(&self, gone: &dyn Fn(&str) -> bool) {
        self.made.borrow_mut().retain(|key, _| !gone(key));
        self.asked.borrow_mut().retain(|(key, _, _)| !gone(key));
        self.jobs.borrow_mut().retain(|job| !gone(&job.key));
    }
}

/// `page` stretched to the cut's box and encoded for `picker`'s protocol. The box was shaped
/// from the page, so the stretch is under a cell, and the picture fills its box to the edge.
fn make(picker: &Picker, page: &DynamicImage, cut: Cut) -> Option<Picture> {
    if cut.cols == 0 || cut.rows == 0 || page.width() == 0 || page.height() == 0 {
        return None;
    }
    let band;
    let page = match cut.across {
        Some((from, to)) => {
            let from = from.min(page.width() - 1);
            let to = to.clamp(from + 1, page.width());
            band = page.crop_imm(from, 0, to - from, page.height());
            &band
        }
        None => page,
    };
    let size = Size::new(cut.cols, cut.rows);
    if picker.protocol_type() == ProtocolType::Halfblocks {
        // A pixel a column and two a row, one over the other.
        let fitted = page.resize_exact(u32::from(cut.cols), u32::from(cut.rows) * 2, FILTER);
        let blocks = Halfblocks::new(fitted, size).ok()?;
        return Some(if cut.sliced {
            Picture::Sliced(SlicedProtocol::Halfblocks(blocks))
        } else {
            Picture::Whole(Protocol::Halfblocks(blocks))
        });
    }
    let font = picker.font_size();
    let fitted = page.resize_exact(
        u32::from(cut.cols) * u32::from(font.width),
        u32::from(cut.rows) * u32::from(font.height),
        FILTER,
    );
    // At the box's size already, so `Fit` leaves it as it is.
    if cut.sliced {
        SlicedProtocol::new_with_resize(picker, fitted, size, Resize::Fit(None))
            .ok()
            .map(Picture::Sliced)
    } else {
        picker.new_protocol(fitted, size, Resize::Fit(None)).ok().map(Picture::Whole)
    }
}

/// Whether a cell of `before` is in none of `now`.
fn uncovered(before: &[Rect], now: &[Rect]) -> bool {
    before.iter().any(|rect| rect.positions().any(|cell| !now.iter().any(|r| r.contains(cell))))
}

impl ImageStore {
    /// Make pages off the loop from now on: draws queue [`EncodeJob`]s for
    /// [`Self::take_encode_jobs`].
    pub fn encode_off_loop(&mut self) {
        self.pictures.off_loop.set(true);
    }

    /// The pages to make, for whoever runs the app to do off the loop.
    pub fn take_encode_jobs(&self) -> Vec<EncodeJob> {
        std::mem::take(&mut *self.pictures.jobs.borrow_mut())
    }

    /// A page made off the loop. One made from a decode since replaced, or for a page let go
    /// of, is dropped.
    pub fn encoded(&self, done: Encoded) {
        let Encoded { key, cut, generation, picture } = done;
        self.pictures.asked.borrow_mut().remove(&(key.clone(), cut, generation));
        let current = self.pages.borrow().get(&key).map(|kept| kept.generation);
        if current == Some(generation) {
            self.pictures.made.borrow_mut().insert(key, Made { cut, generation, picture });
        }
    }

    /// Draw a kept page whole, filling `area`; `false` when there is nothing to draw yet. While
    /// the page is made for a new box, what was made before shows, centred, where it fits.
    pub fn render_page(&self, key: &str, area: Rect, buf: &mut Buffer) -> bool {
        if area.width == 0 || area.height == 0 {
            return false;
        }
        self.draw_made(key, Cut::whole(area.width, area.height), |picture, _| {
            let Picture::Whole(protocol) = picture else { return None };
            let size = protocol.size();
            if size.width > area.width || size.height > area.height {
                return None;
            }
            let at = Rect {
                x: area.x + (area.width - size.width) / 2,
                y: area.y + (area.height - size.height) / 2,
                width: size.width,
                height: size.height,
            };
            Image::new(protocol).render(at, buf);
            Some(Some(at))
        })
    }

    /// Draw the rows in view of a kept page made for `cut`'s box: `area` is the screen the box's
    /// columns show in, and the box's top is `top` rows below `area`'s top, or above it when
    /// negative. While the page is made again for another part of its width, what was made
    /// before shows.
    ///
    /// Where it was drawn is not noted: a scroll moves it every step. The view it is drawn in
    /// is noted with [`Self::note_pictures`].
    pub fn render_slice(
        &self,
        key: &str,
        cut: Cut,
        top: i32,
        area: Rect,
        buf: &mut Buffer,
    ) -> bool {
        if area.width == 0 || area.height == 0 || !cut.sliced {
            return false;
        }
        let top = top.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        self.draw_made(key, cut, |picture, made| {
            let Picture::Sliced(sliced) = picture else { return None };
            if (made.cols, made.rows) != (cut.cols, cut.rows) {
                return None;
            }
            SlicedImage::new(sliced, SignedPosition { x: 0, y: top }).render(area, buf);
            Some(None)
        })
    }

    /// Note that pictures show in `area` this frame, for [`Self::left_behind`].
    pub fn note_pictures(&self, area: Rect) {
        if let Ok(mut drawn) = self.pictures.drawn.try_borrow_mut()
            && !area.is_empty()
        {
            drawn.1.push(area);
        }
    }

    /// A new frame starts: where pictures showed so far was the frame before.
    pub fn start_frame(&self) {
        if let Ok(mut drawn) = self.pictures.drawn.try_borrow_mut() {
            drawn.0 = std::mem::take(&mut drawn.1);
        }
    }

    /// Whether the frame just drawn left a picture's pixels on screen: a terminal that keeps
    /// them ([`ImageEngine::leaves_pixels`](super::ImageEngine::leaves_pixels)), and a cell
    /// that showed a picture in the frame before and shows none now. One page after two, a
    /// spread after its halves, a page after zooming out. The screen wants clearing then.
    pub fn left_behind(&self) -> bool {
        if !self.engine.leaves_pixels() {
            return false;
        }
        let drawn = self.pictures.drawn.borrow();
        uncovered(&drawn.0, &drawn.1)
    }

    /// Draw what is made of `key` for `cut`, asking for it to be made first when it is not:
    /// `draw` gets the picture and the box it was made for, which is another while it is made
    /// again, and says whether it drew and where, if it drew whole. Says whether anything was
    /// drawn.
    fn draw_made(
        &self,
        key: &str,
        cut: Cut,
        draw: impl FnOnce(&Picture, Cut) -> Option<Option<Rect>>,
    ) -> bool {
        let Some(generation) = self.pages.borrow().get(key).map(|kept| kept.generation) else {
            return false;
        };
        let fresh = |made: &Made| made.cut == cut && made.generation == generation;
        let ready = self.pictures.made.borrow().get(key).is_some_and(fresh);
        if !ready {
            self.make_for(key, cut, generation);
        }
        let Ok(made) = self.pictures.made.try_borrow() else { return false };
        let Some(made) = made.get(key) else { return false };
        let Some(picture) = made.picture.as_ref() else { return false };
        match draw(picture, made.cut) {
            Some(at) => {
                if let Some(at) = at {
                    self.note_pictures(at);
                }
                true
            }
            None => false,
        }
    }

    /// Make `key`'s page for `cut`: now, or off the loop once.
    fn make_for(&self, key: &str, cut: Cut, generation: u64) {
        let Some(picker) = self.engine.picker.as_ref() else { return };
        let Some(page) = self.pages.borrow().get(key).map(|kept| kept.image.clone()) else {
            return;
        };
        if self.pictures.off_loop.get() {
            if self.pictures.asked.borrow_mut().insert((key.to_owned(), cut, generation)) {
                self.pictures.jobs.borrow_mut().push(EncodeJob {
                    key: key.to_owned(),
                    cut,
                    generation,
                    page,
                    picker: picker.clone(),
                });
            }
            return;
        }
        let picture = make(picker, &page, cut);
        self.pictures
            .made
            .borrow_mut()
            .insert(key.to_owned(), Made { cut, generation, picture });
    }
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::*;
    use crate::image::ImageEngine;

    /// A page in two colours, the top half red and the bottom half blue.
    fn page(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |_, y| {
            if y < height / 2 { Rgb([200, 0, 0]) } else { Rgb([0, 0, 200]) }
        }))
    }

    fn painted(buf: &Buffer, x: u16, y: u16) -> bool {
        buf[(x, y)].bg != ratatui::style::Color::Reset
    }

    /// The columns of row `y` that show a picture.
    fn painted_cols(buf: &Buffer, y: u16) -> Vec<u16> {
        (0..buf.area.width).filter(|x| painted(buf, *x, y)).collect()
    }

    #[test]
    fn a_page_fills_its_box_even_when_its_shape_is_a_little_off() {
        let store = ImageStore::new(ImageEngine::drawn_as_blocks());
        // 60 by 100 shown in a box made for 2:3: the picture still fills all 12 columns.
        store.insert_page("p", page(60, 100));
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        assert!(store.render_page("p", Rect::new(4, 1, 12, 9), &mut buf));
        for y in 1..10 {
            assert_eq!(painted_cols(&buf, y), (4..16).collect::<Vec<_>>(), "row {y}");
        }
        assert!(!painted(&buf, 4, 0));
    }

    #[test]
    fn off_the_loop_a_page_is_made_once_and_drawn_once_it_is_back() {
        let mut store = ImageStore::new(ImageEngine::drawn_as_blocks());
        store.encode_off_loop();
        store.insert_page("p", page(60, 100));
        let area = Rect::new(0, 0, 12, 10);
        let mut buf = Buffer::empty(area);
        assert!(!store.render_page("p", area, &mut buf), "nothing is made on the loop");
        assert!(!store.render_page("p", area, &mut buf));
        let jobs = store.take_encode_jobs();
        assert_eq!(jobs.len(), 1, "asked for once, however often it is drawn");
        assert_eq!(jobs[0].cut(), Cut::whole(12, 10));
        for job in jobs {
            store.encoded(job.run());
        }
        assert!(store.render_page("p", area, &mut buf));
        assert!(store.take_encode_jobs().is_empty());
    }

    #[test]
    fn a_new_decode_shows_the_old_picture_until_its_own_is_made() {
        let mut store = ImageStore::new(ImageEngine::drawn_as_blocks());
        store.encode_off_loop();
        store.insert_page("p", page(60, 100));
        let area = Rect::new(0, 0, 12, 10);
        let mut buf = Buffer::empty(area);
        store.render_page("p", area, &mut buf);
        let old = store.take_encode_jobs();
        // A sharper file came while the first was being made.
        store.insert_page("p", page(120, 200));
        for job in old {
            store.encoded(job.run());
        }
        assert!(!store.render_page("p", area, &mut buf), "made from the replaced decode");
        for job in store.take_encode_jobs() {
            store.encoded(job.run());
        }
        assert!(store.render_page("p", area, &mut buf));
        store.insert_page("p", page(60, 100));
        assert!(store.render_page("p", area, &mut buf), "the last picture shows meanwhile");
        assert_eq!(store.take_encode_jobs().len(), 1);
    }

    #[test]
    fn a_slice_draws_only_the_rows_in_view_and_scrolling_makes_nothing_new() {
        let mut store = ImageStore::new(ImageEngine::drawn_as_blocks());
        store.encode_off_loop();
        store.insert_page("p", page(60, 100));
        let cut = Cut::sliced(12, 20, None);
        let area = Rect::new(2, 0, 12, 8);
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 8));
        assert!(!store.render_slice("p", cut, -6, area, &mut buf));
        for job in store.take_encode_jobs() {
            store.encoded(job.run());
        }
        for top in [-6, -9, -12, 3] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 16, 8));
            assert!(store.render_slice("p", cut, top, area, &mut buf));
            let first = top.max(0) as u16;
            let last = (top + 20).min(8) as u16;
            for y in 0..8 {
                let want: Vec<u16> =
                    if (first..last).contains(&y) { (2..14).collect() } else { Vec::new() };
                assert_eq!(painted_cols(&buf, y), want, "row {y} with the top at {top}");
            }
        }
        assert!(store.take_encode_jobs().is_empty(), "a scroll step makes no new picture");
        // The red half ends at the box's tenth row: row 3 of the view with the top 6 above.
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 8));
        store.render_slice("p", cut, -6, area, &mut buf);
        let red = |y: u16| match buf[(5, y)].bg {
            ratatui::style::Color::Rgb(r, _, b) => r > b,
            _ => false,
        };
        assert!(red(2) && !red(5));
    }

    #[test]
    fn a_zoomed_page_is_made_again_only_for_another_part_of_its_width() {
        let mut store = ImageStore::new(ImageEngine::drawn_as_blocks());
        store.encode_off_loop();
        store.insert_page("p", page(60, 100));
        let area = Rect::new(0, 0, 10, 8);
        let mut buf = Buffer::empty(area);
        let left = Cut::sliced(10, 30, Some((0, 30)));
        store.render_slice("p", left, 0, area, &mut buf);
        for job in store.take_encode_jobs() {
            store.encoded(job.run());
        }
        assert!(store.render_slice("p", left, -5, area, &mut buf));
        assert!(store.take_encode_jobs().is_empty());
        let right = Cut::sliced(10, 30, Some((30, 60)));
        assert!(store.render_slice("p", right, -5, area, &mut buf), "the left shows meanwhile");
        assert_eq!(store.take_encode_jobs().len(), 1);
    }

    #[test]
    fn letting_go_of_a_page_drops_what_is_being_made_of_it() {
        let mut store = ImageStore::new(ImageEngine::drawn_as_blocks());
        store.encode_off_loop();
        store.insert_page("reader/1/c14/0", page(60, 100));
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 10));
        store.render_page("reader/1/c14/0", buf.area, &mut buf);
        let jobs = store.take_encode_jobs();
        store.forget_prefix("reader/1/c14/");
        for job in jobs {
            store.encoded(job.run());
        }
        assert!(!store.has("reader/1/c14/0"));
        assert!(!store.render_page("reader/1/c14/0", buf.area, &mut buf));
    }

    #[test]
    fn a_cell_a_picture_leaves_is_uncovered() {
        let one = [Rect::new(40, 2, 40, 30)];
        let two = [Rect::new(20, 2, 40, 30), Rect::new(60, 2, 40, 30)];
        assert!(!uncovered(&one, &two), "two pages cover the one before");
        assert!(uncovered(&two, &one), "one page leaves the sides of two");
        assert!(!uncovered(&[], &one));
        assert!(uncovered(&one, &[]));
    }

    #[test]
    fn only_a_terminal_that_keeps_pixels_is_cleared() {
        let draw_then = |store: &ImageStore, first: Rect, then: Rect| {
            store.insert_page("p", page(60, 100));
            let mut buf = Buffer::empty(Rect::new(0, 0, 40, 20));
            store.start_frame();
            store.render_page("p", first, &mut buf);
            store.start_frame();
            store.render_page("p", then, &mut buf);
            store.left_behind()
        };
        let wide = Rect::new(0, 0, 30, 20);
        let narrow = Rect::new(10, 0, 12, 20);
        #[allow(deprecated)]
        let mut sixel = Picker::from_fontsize(ratatui_image::FontSize::new(10, 20));
        sixel.set_protocol_type(ProtocolType::Sixel);
        let sixel = ImageStore::new(ImageEngine::with_picker(sixel));
        assert!(draw_then(&sixel, wide, narrow), "the sides of the wide picture stay");
        assert!(!draw_then(&sixel, narrow, wide));
        #[allow(deprecated)]
        let mut kitty = Picker::from_fontsize(ratatui_image::FontSize::new(10, 20));
        kitty.set_protocol_type(ProtocolType::Kitty);
        let kitty = ImageStore::new(ImageEngine::with_picker(kitty));
        assert!(!draw_then(&kitty, wide, narrow), "kitty deletes its own placements");
        let blocks = ImageStore::new(ImageEngine::drawn_as_blocks());
        assert!(!draw_then(&blocks, wide, narrow));
    }
}
