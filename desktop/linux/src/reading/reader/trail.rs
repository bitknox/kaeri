//! A strip runs on into the next chapter in the same scroll. After the open chapter's last
//! image comes a slim marker naming the next one, with that chapter's first image under it while
//! it opens; once the marker crosses the middle of the window the chapter opens there. The
//! chapters already read stay above the open one, each with its images and its marker, so the
//! scroll goes on both ways.

use std::cell::RefCell;

use adw::prelude::*;
use gtk::gdk;
use kaeri_i18n::t;
use kaeri_model::reading::ChapterRow;

use super::credits::CreditBlock;

/// The marker's height (the reader board's divider).
const MARKER: i32 = 168;

/// A chapter the strip ran on from.
struct Gone {
    /// Its chapter line, for the group while the window is on it.
    line: String,
    /// Its credit pages above its first image and after its last, as they were.
    lead: Option<CreditBlock>,
    tail: Option<CreditBlock>,
    images: gtk::Box,
    frames: Vec<(gtk::Picture, f32)>,
    marker: gtk::Box,
}

pub struct Trail {
    /// The chapters read, above the open one.
    pub above: gtk::Box,
    /// The marker into the next chapter and its first image, after the open one's last image.
    pub foot: gtk::Box,
    gone: RefCell<Vec<Gone>>,
    marker: RefCell<Option<gtk::Box>>,
    lead: gtk::Box,
    lead_picture: gtk::Picture,
    /// The first image's width over height, to size it at the column's width.
    lead_ratio: std::cell::Cell<f32>,
}

/// Where the middle of the window is, as the trail sees it.
pub enum Over {
    /// On a chapter read before, by its place in the trail: its line, the image at the top of
    /// the window and how many it has.
    Gone { line: String, image: usize, count: usize },
    /// On the open chapter.
    Open,
}

impl Trail {
    pub fn new() -> Self {
        let above = gtk::Box::new(gtk::Orientation::Vertical, 0);
        above.set_halign(gtk::Align::Center);
        let lead_picture = gtk::Picture::new();
        lead_picture.set_content_fit(gtk::ContentFit::Fill);
        lead_picture.set_can_shrink(true);
        lead_picture.set_valign(gtk::Align::Start);
        // The page plate holds the first image's place, at once, until the image is here.
        let lead = gtk::Box::new(gtk::Orientation::Vertical, 0);
        lead.add_css_class("reader-plate");
        lead.set_overflow(gtk::Overflow::Hidden);
        lead.append(&lead_picture);
        let foot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        foot.set_halign(gtk::Align::Center);
        foot.append(&lead);
        foot.set_visible(false);
        Self {
            above,
            foot,
            gone: RefCell::default(),
            marker: RefCell::default(),
            lead,
            lead_picture,
            lead_ratio: std::cell::Cell::new(0.0),
        }
    }

    /// Let go of every chapter above: another chapter opened, or the reader closed.
    pub fn clear(&self) {
        for gone in self.gone.borrow_mut().drain(..) {
            for block in [&gone.lead, &gone.tail].into_iter().flatten() {
                self.above.remove(&block.widget);
            }
            self.above.remove(&gone.images);
            self.above.remove(&gone.marker);
        }
        self.show_next(None, None, None);
        self.set_lead(None);
    }

    pub fn is_empty(&self) -> bool {
        self.gone.borrow().is_empty()
    }

    /// The marker at the foot for `next`, or none: the open chapter is the last, or it is not a
    /// strip, and the end page has the foot. `credits` is the line naming the credit pages
    /// skipped at the open chapter's end, with Show.
    pub fn show_next(
        &self,
        next: Option<&ChapterRow>,
        source: Option<&str>,
        credits: Option<&gtk::Widget>,
    ) {
        let old = self.marker.borrow_mut().take();
        if let Some(old) = old {
            self.foot.remove(&old);
        }
        if let Some(next) = next {
            let marker = marker(next, source, credits);
            self.foot.prepend(&marker);
            self.marker.replace(Some(marker));
        }
        self.foot.set_visible(next.is_some());
    }

    /// The next chapter's first image under the marker; none keeps the plate alone.
    pub fn set_lead(&self, texture: Option<&gdk::Texture>) {
        self.lead_ratio
            .set(texture.map_or(0.0, |t| t.width() as f32 / t.height().max(1) as f32));
        self.lead_picture.set_paintable(texture);
        self.lead_picture.set_visible(texture.is_some());
    }

    /// The marker crossed the middle: the open chapter's images and its marker join the trail,
    /// in the place they had, and the foot keeps only the plate until the next chapter's images
    /// take its room.
    pub fn take(
        &self,
        line: String,
        strip: &gtk::Box,
        frames: Vec<(gtk::Picture, f32)>,
        lead: Option<CreditBlock>,
        tail: Option<CreditBlock>,
    ) {
        let images = gtk::Box::new(gtk::Orientation::Vertical, strip.spacing());
        images.set_halign(gtk::Align::Center);
        while let Some(child) = strip.first_child() {
            strip.remove(&child);
            images.append(&child);
        }
        let marker = self.marker.borrow_mut().take().unwrap_or_else(|| {
            gtk::Box::builder().height_request(MARKER).css_classes(["reader-ground"]).build()
        });
        if marker.parent().is_some() {
            self.foot.remove(&marker);
        }
        if let Some(lead) = &lead {
            self.above.append(&lead.widget);
        }
        self.above.append(&images);
        if let Some(tail) = &tail {
            self.above.append(&tail.widget);
        }
        self.above.append(&marker);
        self.gone.borrow_mut().push(Gone { line, lead, tail, images, frames, marker });
    }

    /// Every image above at the column's width with the strip's gap, the markers and the plate
    /// too; the plate is a window tall, or the first image's height if that is more.
    pub fn size(&self, column: i32, spacing: i32, window: i32) {
        for gone in self.gone.borrow().iter() {
            gone.images.set_spacing(spacing);
            for (picture, ratio) in &gone.frames {
                let h = (f64::from(column) / f64::from(ratio.max(0.05))).round() as i32;
                picture.set_size_request(column, h);
            }
            gone.marker.set_size_request(column, MARKER);
            for block in [&gone.lead, &gone.tail].into_iter().flatten() {
                block.size(column);
            }
        }
        if let Some(marker) = self.marker.borrow().as_ref() {
            marker.set_size_request(column, MARKER);
        }
        let ratio = self.lead_ratio.get();
        let image = if ratio > 0.0 { (column as f32 / ratio).round() as i32 } else { 0 };
        self.lead_picture.set_size_request(column, image);
        self.lead.set_size_request(column, window.max(image));
    }

    /// How tall the chapters above are: where the open chapter's first image starts.
    pub fn height(&self) -> f64 {
        self.gone.borrow().iter().map(|g| gone_height(g) + f64::from(tall(&g.marker))).sum()
    }

    /// The marker at the foot's height, or nothing when there is none.
    pub fn marker_height(&self) -> Option<f64> {
        self.marker.borrow().as_ref().map(|m| f64::from(tall(m)))
    }

    /// Which chapter the middle of the window is on, `middle` and `top` down the whole scroll.
    pub fn over(&self, top: f64, middle: f64) -> Over {
        let mut y = 0.0;
        for gone in self.gone.borrow().iter() {
            let images = gone_height(gone);
            let marker = f64::from(tall(&gone.marker));
            let lead = gone.lead.as_ref().map_or(0.0, CreditBlock::height);
            let tail = gone.tail.as_ref().map_or(0.0, CreditBlock::height);
            if middle < y + lead + images + tail + marker / 2.0 {
                let heights: Vec<i32> =
                    gone.frames.iter().map(|(p, _)| p.height_request()).collect();
                let (image, _) =
                    super::strip_place_of(&heights, gone.images.spacing(), top - y - lead);
                return Over::Gone { line: gone.line.clone(), image, count: heights.len() };
            }
            y += lead + images + tail + marker;
        }
        Over::Open
    }
}

/// A chapter's images with the gaps between them, and its credit pages.
fn gone_height(gone: &Gone) -> f64 {
    let n = gone.frames.len() as i32;
    let images: i32 = gone.frames.iter().map(|(p, _)| p.height_request()).sum();
    let credits =
        [&gone.lead, &gone.tail].into_iter().flatten().map(CreditBlock::height).sum::<f64>();
    f64::from(images + gone.images.spacing() * (n - 1).max(0)) + credits
}

/// A widget's height as it will be laid out at its width.
pub(super) fn tall(widget: &impl IsA<gtk::Widget>) -> i32 {
    let width = widget.width_request();
    let (min, natural, _, _) = widget.measure(gtk::Orientation::Vertical, width);
    widget.height_request().max(min).max(natural)
}

/// The slim marker into the next chapter: Next, its title and its facts, on the ground. It
/// scrolls past like an image.
fn marker(next: &ChapterRow, source: Option<&str>, credits: Option<&gtk::Widget>) -> gtk::Box {
    let over = gtk::Label::new(Some(&t::reader_end_next()));
    over.add_css_class("strip-marker-next");
    over.set_xalign(0.0);
    let title = next.title.as_ref().map_or_else(
        || next.label.clone(),
        |title| {
            t::reader_chapter_line_titled(next.number.as_deref().unwrap_or(&next.label), title)
        },
    );
    let title = gtk::Label::new(Some(&title));
    title.add_css_class("strip-marker-title");
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    title.set_lines(3);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 2);
    column.set_valign(gtk::Align::Center);
    column.set_vexpand(true);
    column.set_margin_start(24);
    column.set_margin_end(24);
    column.append(&over);
    column.append(&title);
    let facts = marker_facts(next, source);
    if !facts.is_empty() {
        let facts = gtk::Label::new(Some(&facts));
        facts.add_css_class("strip-marker-facts");
        facts.set_xalign(0.0);
        facts.set_wrap(true);
        facts.set_margin_top(2);
        column.append(&facts);
    }
    if let Some(credits) = credits {
        credits.set_margin_top(6);
        column.append(credits);
    }
    let marker = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["reader-ground", "strip-marker"])
        .height_request(MARKER)
        .accessible_role(gtk::AccessibleRole::Group)
        .build();
    marker.append(&column);
    marker
}

/// "34 images · from books.example", the next chapter's length and where it came from.
fn marker_facts(next: &ChapterRow, source: Option<&str>) -> String {
    let images = next.pages.filter(|p| *p > 0).map(|p| t::reader_divider_images(i64::from(p)));
    match (images, source) {
        (Some(images), Some(source)) => t::reader_divider_facts_source(&images, source),
        (None, Some(source)) => t::reader_end_from(source),
        (Some(images), None) => images,
        (None, None) => String::new(),
    }
}
