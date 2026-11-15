//! The reader past one page fitted to the window: a page at fit width scrolled with `j` and `k`,
//! a page zoomed in and moved about with a map of where the view is, and a webtoon strip that
//! runs on from one chapter into the next.
//!
//! Fit width, fit height and zoom lay the leaf out on a canvas larger than the stage, in cells,
//! and show the part of it the view is on. A strip is one canvas from the chapters it ran on from down to the
//! next chapter's first image. The engine hears where a strip settled with `StripAt`, half a
//! second after it stopped, and the next chapter opens once the divider into it crosses the
//! middle of the window, with the chapter left kept above.

use std::time::{Duration, Instant};

use kaeri_core::books::ChapterId;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    reader::{
        Leaf, LeafPart, ReadDirection, ReaderCommand, ReaderMode, ReaderSettingKey,
        ReaderSettingRow, ReaderSettingsView, ReaderValue, SettingScope, StripPos, TurnHow,
    },
    reading::ChapterRow,
};
use ratatui::layout::Rect;

use super::{App, CHROME_ROWS, PartKey, Reader, ReaderScreen};

/// The zoom's steps, as a share of the page fitted to the window.
pub const ZOOM_STEPS: [f32; 3] = [1.5, 2.0, 3.0];
/// Where `z` zooms to.
const ZOOM_IN: f32 = 2.0;
/// How far `j` and `k` scroll.
pub const SCROLL_ROWS: i64 = 3;
/// A divider between chapters in a strip: a blank row, the chapter, its facts, a blank row.
pub const DIVIDER_ROWS: u32 = 4;
/// A strip says where it settled this long after it stopped.
const REPORT_AFTER: Duration = Duration::from_millis(500);
/// The columns the zoom keeps at the right for its map: a margin, the view's gap, the map.
pub const ZOOM_PANEL: u16 = 16;
/// Fit width leaves a column at the left, and the scroll bar with a column each side of it.
pub const FIT_MARGIN: u16 = 4;
/// The Strip width setting's share when the settings have not come.
const STRIP_SHARE: f64 = 0.6;
/// A strip's column for each part of the Strip width setting: the board's 48 of 120 columns at
/// the setting's 0.6, as cells are twice as tall as they are wide.
const STRIP_COLUMNS: f64 = 2.0 / 3.0;
/// The narrowest strip column, where the window has it.
const STRIP_MIN: u16 = 24;
/// A strip image's width over its height before its picture is known.
const IMAGE_RATIO: f32 = 0.6;
/// The blank rows between two images of a strip with the Strip gap setting at small.
pub const STRIP_GAP_ROWS: u32 = 1;

/// The zoom while it is on: its step, and where the view's centre is on the leaf.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zoom {
    pub level: f32,
    /// The view's centre, each from 0 to 1 across the leaf.
    pub x: f32,
    pub y: f32,
}

/// The strip's place and the chapters it ran on from.
#[derive(Debug, Clone, Default)]
pub struct Strip {
    /// The row at the top of the view, from the top of the strip.
    pub top: u32,
    /// The column width `top` was worked out at.
    pub cols: u16,
    /// The chapters read before in this scroll, oldest first, kept above the open one.
    pub trail: Vec<Gone>,
    /// When to tell the engine where the strip settled.
    pub report: Option<Instant>,
    /// The next chapter's first image from its preview, drawn in its place until its own
    /// lands: the chapter, the picture's key and its width over height.
    pub lead: Option<(ChapterId, String, f32)>,
    /// The leaves near the view, to have pages for.
    pub near: Vec<usize>,
}

/// A chapter the strip ran on from: its images, and the divider into the one after it.
#[derive(Debug, Clone, PartialEq)]
pub struct Gone {
    pub chapter: ChapterRow,
    /// Each image's picture key, and its width over its height.
    pub images: Vec<(String, f32)>,
    pub next: ChapterRow,
    pub source: Option<String>,
}

/// One box on a canvas, in cells: a page, or a strip image.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasBox {
    pub key: String,
    /// The open chapter's part, for a plate that says it is loading or failed.
    pub part: Option<PartKey>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A canvas and the part of it in view.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewport {
    pub boxes: Vec<CanvasBox>,
    pub width: u32,
    pub height: u32,
    /// The view's offset on the canvas; negative centres a canvas smaller than the view.
    pub x: i32,
    pub y: i32,
    /// Where the view is drawn.
    pub area: Rect,
}

/// A strip laid out: its pieces top to bottom and its height, in rows.
#[derive(Debug, Clone)]
pub struct StripLayout {
    pub items: Vec<StripItem>,
    pub total: u32,
    pub cols: u16,
}

#[derive(Debug, Clone)]
pub struct StripItem {
    pub top: u32,
    pub rows: u32,
    pub piece: Piece,
}

#[derive(Debug, Clone)]
pub enum Piece {
    /// An image. `part` and `leaf` say which of the open chapter's it is; an image of a chapter
    /// the strip ran on from, or the next chapter's first under the divider, has neither.
    Image { key: String, part: Option<PartKey>, leaf: Option<usize> },
    /// Between chapters: the chapter after it, and where it comes from.
    Divider { next: Box<ChapterRow>, source: Option<String> },
    /// After the last chapter there is: what the end page would say.
    Foot,
}

impl StripLayout {
    /// The open chapter's images.
    fn open(&self) -> impl Iterator<Item = (&StripItem, usize, PartKey)> {
        self.items.iter().filter_map(|item| match &item.piece {
            Piece::Image { leaf: Some(leaf), part: Some(part), .. } => {
                Some((item, *leaf, *part))
            }
            _ => None,
        })
    }

    /// The row a place in the open chapter starts on.
    fn row_of(&self, page: u32, offset: f32) -> Option<u32> {
        self.open().find(|(_, _, part)| part.page == page).map(|(item, _, _)| {
            item.top + (offset.clamp(0.0, 1.0) * item.rows as f32).round() as u32
        })
    }

    /// The image of the open chapter at `top`, and how far into it.
    fn place_at(&self, top: u32) -> Option<(u32, f32)> {
        let (item, _, part) = self.open().find(|(item, _, _)| item.top + item.rows > top)?;
        (top >= item.top)
            .then(|| (part.page, (top - item.top) as f32 / item.rows.max(1) as f32))
            .or(Some((part.page, 0.0)))
            .filter(|_| self.open().next().is_some_and(|(first, _, _)| top >= first.top))
    }

    /// The first image showing in a view from `top`: the one at the top, or the first below it.
    pub fn image_at(&self, top: u32) -> Option<&StripItem> {
        self.items
            .iter()
            .filter(|item| matches!(item.piece, Piece::Image { .. }))
            .find(|item| item.top + item.rows > top)
    }

    /// The divider into the next chapter, after the open chapter's last image.
    fn next_divider(&self) -> Option<&StripItem> {
        let last = self.open().last()?.0.top;
        self.items
            .iter()
            .find(|item| item.top > last && matches!(item.piece, Piece::Divider { .. }))
    }
}

/// The stage of a window: under the header and above the status rule, or all of it bare.
pub fn stage_of(area: Rect, bare: bool) -> Rect {
    if bare {
        area
    } else {
        Rect { y: area.y + 2, height: area.height.saturating_sub(CHROME_ROWS), ..area }
    }
}

/// A strip column's width in a window `width` wide: the Strip width setting, read in columns.
pub fn strip_cols(reader: &Reader, width: u16) -> u16 {
    let share =
        reader.amount(ReaderSettingKey::StripWidth).unwrap_or(STRIP_SHARE).clamp(0.3, 1.0);
    let cols = (f64::from(width) * share * STRIP_COLUMNS).round() as u16;
    cols.clamp(STRIP_MIN.min(width), width.max(1))
}

/// An image's height in rows at `cols` wide, from its width over its height.
fn image_rows(cols: u16, ratio: f32, (cw, ch): (f32, f32)) -> u32 {
    let ratio = if ratio.is_finite() && ratio > 0.01 { ratio } else { IMAGE_RATIO };
    ((f32::from(cols) * cw / ratio / ch).round() as u32).max(1)
}

/// The leaf's parts side by side at one height, the tallest's, in its pixels: each part with its
/// x and width, then the whole's width and height.
fn leaf_shape(reader: &Reader, leaf: &Leaf) -> (Vec<(PartKey, f32, f32)>, f32, f32) {
    let mut parts: Vec<&LeafPart> = leaf.parts.iter().collect();
    parts.sort_by(|a, b| a.place.x.total_cmp(&b.place.x));
    let sizes: Vec<(f32, f32)> = parts.iter().map(|p| reader.part_size(p)).collect();
    let height = sizes.iter().map(|(_, h)| *h).fold(1.0_f32, f32::max);
    let mut x = 0.0;
    let mut out = Vec::new();
    for (part, (w, h)) in parts.iter().zip(sizes) {
        let w = w * height / h.max(1.0);
        out.push((PartKey { page: part.page, half: part.half }, x, w));
        x += w;
    }
    (out, x.max(1.0), height)
}

/// The leaf's boxes with each of its pixels `scale` pixels on screen, and the canvas's size.
fn leaf_canvas(
    reader: &Reader,
    leaf: &Leaf,
    (cw, ch): (f32, f32),
    scale: f32,
) -> (Vec<CanvasBox>, u32, u32) {
    let (parts, width, height) = leaf_shape(reader, leaf);
    let rows = ((height * scale / ch).round() as u32).max(1);
    let cols = ((width * scale / cw).round() as u32).max(1);
    let boxes = parts
        .into_iter()
        .map(|(part, x, w)| {
            let x0 = (x * scale / cw).round() as i32;
            let x1 = ((x + w) * scale / cw).round() as i32;
            CanvasBox {
                key: reader.picture_key(part),
                part: Some(part),
                x: x0,
                y: 0,
                width: (x1 - x0).max(1) as u32,
                height: rows,
            }
        })
        .collect();
    (boxes, cols, rows)
}

/// The view's offset on a canvas `size` long for a view `view` long centred at `centre`: kept
/// on the canvas, or centring a canvas smaller than the view.
fn offset_of(centre: f32, size: u32, view: u16) -> i32 {
    let view = u32::from(view);
    if size <= view {
        return -(((view - size) / 2) as i32);
    }
    let at = (centre * size as f32 - view as f32 / 2.0).round();
    at.clamp(0.0, (size - view) as f32) as i32
}

/// The centre a view at `offset` has; `None` where the canvas fits and the view can't move.
fn centre_of(offset: i32, size: u32, view: u16) -> Option<f32> {
    (size > u32::from(view)).then(|| (offset as f32 + f32::from(view) / 2.0) / size as f32)
}

impl Reader {
    /// How the chapter shows: paged unless the engine said otherwise.
    pub fn mode(&self) -> ReaderMode {
        self.view.as_ref().map_or(ReaderMode::Paged, |v| v.mode)
    }

    /// Pages run one under another as a strip.
    pub fn strips(&self) -> bool {
        self.mode().flows()
    }

    /// A setting's row, from the settings last sent or the chapter's own.
    fn setting(&self, key: ReaderSettingKey) -> Option<&ReaderValue> {
        let rows =
            self.settings.as_ref().or(self.view.as_ref().and_then(|v| v.settings.as_ref()))?;
        rows.rows.iter().find(|r| r.key == key).map(|r| &r.value)
    }

    /// A choice setting's value, such as the page layout's "auto".
    pub fn choice(&self, key: ReaderSettingKey) -> Option<&str> {
        match self.setting(key)? {
            ReaderValue::Choice { name } => Some(name.as_str()),
            _ => None,
        }
    }

    pub(super) fn amount(&self, key: ReaderSettingKey) -> Option<f64> {
        match self.setting(key)? {
            ReaderValue::Amount { value } => Some(*value),
            _ => None,
        }
    }

    pub(super) fn toggle(&self, key: ReaderSettingKey) -> Option<bool> {
        match self.setting(key)? {
            ReaderValue::Toggle { on } => Some(*on),
            _ => None,
        }
    }

    /// Set a choice here at once, for the screen to follow before the engine's settings come.
    fn set_choice(&mut self, key: ReaderSettingKey, name: &str, scope: SettingScope) {
        self.set_value(key, ReaderValue::Choice { name: name.to_owned() }, scope);
    }

    /// Set a value here at once, for the screen to follow before the engine's settings come.
    pub(super) fn set_value(
        &mut self,
        key: ReaderSettingKey,
        value: ReaderValue,
        scope: SettingScope,
    ) {
        if self.settings.is_none() {
            let from_view = self.view.as_ref().and_then(|v| v.settings.clone());
            self.settings = from_view.or_else(|| {
                Some(ReaderSettingsView {
                    medium: self.medium()?,
                    work: Some(self.work.clone()),
                    rows: Vec::new(),
                    tap_zones: None,
                })
            });
        }
        for rows in
            [self.settings.as_mut(), self.view.as_mut().and_then(|v| v.settings.as_mut())]
                .into_iter()
                .flatten()
        {
            match rows.rows.iter_mut().find(|r| r.key == key) {
                Some(row) => row.value = value.clone(),
                None => rows.rows.push(ReaderSettingRow {
                    key,
                    value: value.clone(),
                    scope,
                    detected: false,
                    overridden: false,
                    choices: Vec::new(),
                }),
            }
        }
    }

    /// The page fits the window's width and scrolls: paged, not zoomed, Fit set to width.
    pub fn fits_width(&self) -> bool {
        self.fits_side() && self.choice(ReaderSettingKey::Fit) == Some("width") // i18n-skip: a value
    }

    /// The page fills the stage's height and a page wider than the stage moves along it: paged,
    /// not zoomed, Fit set to height.
    pub fn fits_height(&self) -> bool {
        self.fits_side() && self.choice(ReaderSettingKey::Fit) == Some("height") // i18n-skip: a value
    }

    /// The page fills one side of the stage and moves along the other: fit width or height.
    pub fn fits_side(&self) -> bool {
        !self.strips()
            && self.zoom.is_none()
            && matches!(self.choice(ReaderSettingKey::Fit), Some("width" | "height")) // i18n-skip: values
    }

    /// The page in view, for a layout in another mode to keep.
    fn page_here(&self) -> Option<u32> {
        self.leaf().filter(|l| !l.end_page).and_then(|l| l.parts.iter().map(|p| p.page).min())
    }

    /// The chapter's number in a strip's lines: its number, else its label.
    fn number_of(row: &ChapterRow) -> String {
        row.number.clone().unwrap_or_else(|| row.label.clone())
    }
}

impl App {
    /// The stage in the window the last frame was drawn in.
    pub fn reader_stage(&self) -> Option<Rect> {
        let reader = self.reader_here()?;
        let (w, h) = self.window?;
        Some(stage_of(Rect::new(0, 0, w, h), reader.bare))
    }

    /// A cell's size in the pixels pages are laid out in.
    fn cell(&self) -> (f32, f32) {
        let (cw, ch) = self.images.engine().font_size();
        (f32::from(cw.max(1)), f32::from(ch.max(1)))
    }

    /// The leaf in view at fit width on `stage`: as wide as the stage less its margins, scrolled.
    /// At fit height: as tall as the stage, moved along from its reading side.
    pub fn fit_view(&self, reader: &Reader, stage: Rect) -> Option<Viewport> {
        let leaf = reader.leaf().filter(|l| !l.end_page && !l.parts.is_empty())?;
        let cell = self.cell();
        if reader.fits_height() {
            let (_, _, h) = leaf_shape(reader, leaf);
            let scale = f32::from(stage.height) * cell.1 / h;
            let (boxes, cols, rows) = leaf_canvas(reader, leaf, cell, scale);
            let max = cols.saturating_sub(u32::from(stage.width));
            let along = reader.scroll.min(max);
            let x = if max == 0 {
                offset_of(0.5, cols, stage.width)
            } else if reader.rtl() {
                (max - along) as i32
            } else {
                along as i32
            };
            return Some(Viewport {
                boxes,
                width: cols,
                height: rows,
                x,
                y: offset_of(0.5, rows, stage.height),
                area: stage,
            });
        }
        let width = stage.width.saturating_sub(FIT_MARGIN).max(1);
        let (_, w, _) = leaf_shape(reader, leaf);
        let scale = f32::from(width) * cell.0 / w;
        let (boxes, cols, rows) = leaf_canvas(reader, leaf, cell, scale);
        let max = rows.saturating_sub(u32::from(stage.height));
        let y = if rows < u32::from(stage.height) {
            offset_of(0.5, rows, stage.height)
        } else {
            reader.scroll.min(max) as i32
        };
        Some(Viewport {
            boxes,
            width: cols,
            height: rows,
            x: offset_of(0.5, cols, width),
            y,
            area: Rect { x: stage.x + 1, width, ..stage },
        })
    }

    /// The leaf in view zoomed on `stage`: the view at the left, the map's columns at the right.
    pub fn zoom_view(&self, reader: &Reader, stage: Rect) -> Option<Viewport> {
        let zoom = reader.zoom?;
        let leaf = reader.leaf().filter(|l| !l.end_page && !l.parts.is_empty())?;
        let cell = self.cell();
        let (_, w, h) = leaf_shape(reader, leaf);
        let fit =
            (f32::from(stage.width) * cell.0 / w).min(f32::from(stage.height) * cell.1 / h);
        let (boxes, cols, rows) = leaf_canvas(reader, leaf, cell, fit * zoom.level);
        let view_w = stage.width.saturating_sub(ZOOM_PANEL).max(1);
        Some(Viewport {
            boxes,
            width: cols,
            height: rows,
            x: offset_of(zoom.x, cols, view_w),
            y: offset_of(zoom.y, rows, stage.height),
            area: Rect { x: stage.x + 1, width: view_w, ..stage },
        })
    }

    /// The strip as it stands, at the window's column width, and the rows in view.
    pub fn strip_now(&self) -> Option<(StripLayout, u32)> {
        let reader = self.reader_here()?;
        let stage = self.reader_stage()?;
        let cols = strip_cols(reader, stage.width);
        Some((self.strip_layout(reader, cols), u32::from(stage.height)))
    }

    /// The strip at `cols` wide: the chapters ran on from with their dividers, the open
    /// chapter's images, then the divider into the next chapter with its first image, or the
    /// foot after the last chapter there is. With the Strip gap setting at small, blank rows
    /// part two images that follow each other; a divider has blank rows of its own.
    pub fn strip_layout(&self, reader: &Reader, cols: u16) -> StripLayout {
        let cell = self.cell();
        let small = reader.choice(ReaderSettingKey::StripGap) == Some("small"); // i18n-skip: a value
        let gap = if small { STRIP_GAP_ROWS } else { 0 };
        let mut items: Vec<StripItem> = Vec::new();
        let mut top = 0;
        let mut push = |piece: Piece, rows: u32| {
            let after_image =
                items.last().is_some_and(|i| matches!(i.piece, Piece::Image { .. }));
            if after_image && matches!(piece, Piece::Image { .. }) {
                top += gap;
            }
            items.push(StripItem { top, rows, piece });
            top += rows;
        };
        for gone in &reader.strip.trail {
            for (key, ratio) in &gone.images {
                let piece = Piece::Image { key: key.clone(), part: None, leaf: None };
                push(piece, image_rows(cols, *ratio, cell));
            }
            push(
                Piece::Divider {
                    next: Box::new(gone.next.clone()),
                    source: gone.source.clone(),
                },
                DIVIDER_ROWS,
            );
        }
        let images: Vec<(usize, &LeafPart)> = reader
            .leaves
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.end_page)
            .filter_map(|(i, l)| l.parts.first().map(|p| (i, p)))
            .collect();
        let lead = reader.strip.lead.as_ref().filter(|(id, _, _)| {
            reader.flowing.as_ref() == Some(id) || reader.chapter().is_some_and(|c| c.id == *id)
        });
        if images.is_empty()
            && let Some((_, key, ratio)) = lead
        {
            let piece = Piece::Image { key: key.clone(), part: None, leaf: None };
            push(piece, image_rows(cols, *ratio, cell));
        }
        for (index, (leaf, part)) in images.iter().enumerate() {
            let key = PartKey { page: part.page, half: part.half };
            let mut picture = reader.picture_key(key);
            // The first image from the preview it flowed on with, until its own lands.
            if index == 0
                && !self.images.has(&picture)
                && let Some((_, lead_key, _)) = lead
                && self.images.has(lead_key)
            {
                picture = lead_key.clone();
            }
            let (w, h) = reader.part_size(part);
            let piece = Piece::Image { key: picture, part: Some(key), leaf: Some(*leaf) };
            push(piece, image_rows(cols, w / h.max(1.0), cell));
        }
        if reader.flowing.is_none() && !images.is_empty() {
            if let Some(next) = &reader.end.next {
                let source = reader.end.next_source.clone();
                push(Piece::Divider { next: Box::new(next.clone()), source }, DIVIDER_ROWS);
                let ratio = reader
                    .end
                    .preview
                    .as_ref()
                    .filter(|(id, _)| *id == next.id)
                    .map_or(IMAGE_RATIO, |(_, (w, h))| *w as f32 / (*h).max(1) as f32);
                let piece =
                    Piece::Image { key: reader.preview_key(&next.id), part: None, leaf: None };
                push(piece, image_rows(cols, ratio, cell));
            } else if reader.end.why_none.is_some() {
                push(Piece::Foot, DIVIDER_ROWS + 2);
            }
        }
        StripLayout { items, total: top, cols }
    }

    /// Keys that mean something else at fit width, zoomed or in a strip. `None` leaves the key
    /// to the paged reader.
    pub(super) fn mode_key(
        &mut self,
        screen: ReaderScreen,
        action: crate::keymap::Action,
    ) -> Option<Option<Command>> {
        use crate::keymap::Action as A;
        let reader = self.reader_here()?;
        if reader.strips() && matches!(screen, ReaderScreen::Pages | ReaderScreen::End) {
            let screenful = i64::from(self.reader_stage()?.height) * 9 / 10;
            return Some(match action {
                A::ReaderScrollDown => self.strip_scroll(SCROLL_ROWS),
                A::ReaderScrollUp => self.strip_scroll(-SCROLL_ROWS),
                A::ReaderNext | A::ReaderRight => self.strip_scroll(screenful.max(1)),
                A::ReaderPrevious | A::ReaderLeft => self.strip_scroll(-screenful.max(1)),
                A::ReaderFirst => self.strip_jump(false),
                A::ReaderLast => self.strip_jump(true),
                A::ReaderNextChapter => self.strip_next_chapter(),
                A::ReaderFit | A::ReaderZoom | A::ReaderZoomIn | A::ReaderZoomOut => None,
                _ => return None,
            });
        }
        if screen != ReaderScreen::Pages {
            return None;
        }
        if reader.zoom.is_some() {
            let rtl = reader.rtl();
            return Some(match action {
                A::ReaderLeave | A::ReaderZoom => self.leave_zoom(),
                A::ReaderZoomIn => self.step_zoom(true),
                A::ReaderZoomOut => self.step_zoom(false),
                A::ReaderLeft => self.zoom_side(true, rtl),
                A::ReaderRight => self.zoom_side(false, rtl),
                A::ReaderScrollDown => {
                    self.zoom_move(0, SCROLL_ROWS as i32);
                    None
                }
                A::ReaderScrollUp => {
                    self.zoom_move(0, -SCROLL_ROWS as i32);
                    None
                }
                A::ReaderNext => self.zoom_part(true),
                A::ReaderPrevious => self.zoom_part(false),
                _ => return None,
            });
        }
        if reader.fits_side() {
            return match action {
                A::ReaderScrollDown => Some(self.fit_scroll(SCROLL_ROWS)),
                A::ReaderScrollUp => Some(self.fit_scroll(-SCROLL_ROWS)),
                A::ReaderNext => Some(self.fit_screen(true)),
                A::ReaderPrevious => Some(self.fit_screen(false)),
                _ => None,
            };
        }
        None
    }

    /// `f`: fit screen or fit width, kept on this computer through the engine's Fit setting. Fit
    /// height, set in the reader settings, goes back to fit screen.
    pub(super) fn switch_fit(&mut self) -> Option<Command> {
        let reader = self.reader_mut()?;
        if reader.strips() || reader.session == 0 {
            return None;
        }
        let name = if reader.fits_side() { "screen" } else { "width" }; // i18n-skip: values
        reader.set_choice(ReaderSettingKey::Fit, name, SettingScope::Device);
        reader.zoom = None;
        reader.scroll = 0;
        let (session, medium) = (reader.session, reader.medium().unwrap_or_default_manga());
        self.send_surface();
        Some(Command::Reader(ReaderCommand::SetSetting {
            session: Some(session),
            medium,
            work: None,
            key: ReaderSettingKey::Fit,
            scope: SettingScope::Device,
            value: ReaderValue::Choice { name: name.to_owned() },
        }))
    }

    /// `m`: paged or strip, the series' own choice, as the apps keep it.
    pub(super) fn switch_mode(&mut self) -> Option<Command> {
        let reader = self.reader_here()?;
        let medium = reader.medium()?;
        if medium == kaeri_core::media::Medium::Novel || reader.session == 0 {
            return None;
        }
        let name = if reader.strips() { "paged" } else { "strip" }; // i18n-skip: values
        Some(Command::Reader(ReaderCommand::SetSetting {
            session: Some(reader.session),
            medium,
            work: Some(reader.work.clone()),
            key: ReaderSettingKey::Mode,
            scope: SettingScope::Series,
            value: ReaderValue::Choice { name: name.to_owned() },
        }))
    }

    /// The settings came: a mode or direction picked for this series is followed, and the page
    /// in view stays in view in the new mode once the engine lays it out.
    pub(super) fn follow_settings(&mut self) {
        let page = self.page_now();
        let Some(reader) = self.reader.as_mut() else { return };
        let Some(view) = reader.view.as_ref() else { return };
        let Some(settings) = reader.settings.as_ref() else { return };
        if view.medium == kaeri_core::media::Medium::Novel
            || settings.medium != view.medium
            || settings.work.as_ref() != Some(&reader.work)
        {
            return;
        }
        let word = |key: ReaderSettingKey| {
            settings.rows.iter().find(|r| r.key == key).and_then(|r| match &r.value {
                ReaderValue::Choice { name } => Some(name.clone()),
                _ => None,
            })
        };
        let mode = match word(ReaderSettingKey::Mode).as_deref() {
            Some("strip") => ReaderMode::Strip, // i18n-skip: setting values
            Some("vertical") => ReaderMode::Vertical, // i18n-skip: setting values
            Some("paged") => ReaderMode::Paged, // i18n-skip: setting values
            _ => view.mode,
        };
        let direction = if mode.flows() {
            ReadDirection::TopToBottom
        } else {
            match word(ReaderSettingKey::Direction).as_deref() {
                Some("ltr") => ReadDirection::LeftToRight, // i18n-skip: setting values
                Some("rtl") => ReadDirection::RightToLeft, // i18n-skip: setting values
                _ if view.direction == ReadDirection::TopToBottom => ReadDirection::RightToLeft,
                _ => view.direction,
            }
        };
        let switched = mode != view.mode;
        if let Some(view) = reader.view.as_mut() {
            view.mode = mode;
            view.direction = direction;
        }
        if switched {
            reader.switched = page.or(Some(0));
            reader.zoom = None;
            reader.scroll = 0;
            reader.strip = Strip::default();
        }
        if switched {
            self.send_surface();
        }
    }

    /// The page in view: the leaf's first, or in a strip the image at the top.
    fn page_now(&self) -> Option<u32> {
        let reader = self.reader_here()?;
        if reader.strips() {
            let (layout, _) = self.strip_now()?;
            return layout.place_at(reader.strip.top).map(|(page, _)| page);
        }
        reader.page_here()
    }

    /// How far the view at fit width or height moves along the page, and a screen of that.
    pub(super) fn fit_extent(&self) -> Option<(u32, i64)> {
        let stage = self.reader_stage()?;
        let reader = self.reader_here()?;
        let view = self.fit_view(reader, stage)?;
        Some(if reader.fits_height() {
            let max = view.width.saturating_sub(u32::from(stage.width));
            (max, i64::from(stage.width) * 9 / 10)
        } else {
            let max = view.height.saturating_sub(u32::from(stage.height));
            (max, i64::from(stage.height) * 9 / 10)
        })
    }

    /// `j` and `k` at fit width: three rows. At fit height: six columns along the reading side,
    /// as cells are twice as tall as they are wide.
    fn fit_scroll(&mut self, by: i64) -> Option<Command> {
        let (max, _) = self.fit_extent()?;
        let reader = self.reader_mut()?;
        let by = if reader.fits_height() { by * 2 } else { by };
        reader.scroll =
            (i64::from(reader.scroll.min(max)) + by).clamp(0, i64::from(max)) as u32;
        None
    }

    /// Space and Backspace at fit width: most of a screen down or up, and at the foot of the
    /// page the next page at its top, as the turn keys go. At fit height the same along the
    /// page from its reading side.
    fn fit_screen(&mut self, forward: bool) -> Option<Command> {
        let (max, by) = self.fit_extent()?;
        let reader = self.reader_here()?;
        let at = reader.scroll.min(max);
        if (forward && at < max) || (!forward && at > 0) {
            let reader = self.reader_mut()?;
            let by = if forward { by.max(1) } else { -by.max(1) };
            reader.scroll = (i64::from(at) + by).clamp(0, i64::from(max)) as u32;
            return None;
        }
        self.turn(forward)
    }

    /// `z` on a page: zoomed in at 200%, at the top of the page on its reading side.
    pub(super) fn toggle_zoom(&mut self) -> Option<Command> {
        let reader = self.reader_mut()?;
        if reader.zoom.is_some() {
            return self.leave_zoom();
        }
        if reader.strips() || reader.leaf().is_none_or(|l| l.end_page || l.parts.is_empty()) {
            return None;
        }
        let x = if reader.rtl() { 1.0 } else { 0.0 };
        reader.zoom = Some(Zoom { level: ZOOM_IN, x, y: 0.0 });
        self.settle_zoom();
        self.send_surface();
        None
    }

    /// `z` or Esc while zoomed: the page fitted again, and pages at the surface's size.
    fn leave_zoom(&mut self) -> Option<Command> {
        self.reader_mut()?.zoom = None;
        self.send_surface();
        None
    }

    /// `+` and `-`: a step in or out, about the view's centre. `+` zooms in from the page.
    pub(super) fn step_zoom(&mut self, closer: bool) -> Option<Command> {
        let reader = self.reader_mut()?;
        if reader.strips() || reader.leaf().is_none_or(|l| l.end_page || l.parts.is_empty()) {
            return None;
        }
        let Some(zoom) = reader.zoom.as_mut() else {
            if closer {
                let x = if reader.rtl() { 1.0 } else { 0.0 };
                reader.zoom = Some(Zoom { level: ZOOM_STEPS[0], x, y: 0.0 });
                self.settle_zoom();
                self.send_surface();
            }
            return None;
        };
        let at = ZOOM_STEPS.iter().position(|s| (*s - zoom.level).abs() < 0.01).unwrap_or(1);
        let to = if closer { (at + 1).min(ZOOM_STEPS.len() - 1) } else { at.saturating_sub(1) };
        zoom.level = ZOOM_STEPS[to];
        self.settle_zoom();
        self.send_surface();
        None
    }

    /// The zoom's centre kept where the view can be, so a move from an edge moves at once.
    fn settle_zoom(&mut self) {
        let Some(stage) = self.reader_stage() else { return };
        let Some(view) = self.reader_here().and_then(|r| self.zoom_view(r, stage)) else {
            return;
        };
        if let Some(zoom) = self.reader_mut().and_then(|r| r.zoom.as_mut()) {
            // A side that fits keeps where it was asked to be, for the step after.
            zoom.x = centre_of(view.x, view.width, view.area.width).unwrap_or(zoom.x);
            zoom.y = centre_of(view.y, view.height, view.area.height).unwrap_or(zoom.y);
        }
    }

    /// Move the zoomed view by `dx` columns and `dy` rows; says whether it moved.
    fn zoom_move(&mut self, dx: i32, dy: i32) -> bool {
        let Some(stage) = self.reader_stage() else { return false };
        let Some(view) = self.reader_here().and_then(|r| self.zoom_view(r, stage)) else {
            return false;
        };
        let clamp = |at: i32, size: u32, room: u16| {
            if size <= u32::from(room) {
                at
            } else {
                at.clamp(0, (size - u32::from(room)) as i32)
            }
        };
        let x = clamp(view.x + dx, view.width, view.area.width);
        let y = clamp(view.y + dy, view.height, view.area.height);
        if let Some(zoom) = self.reader_mut().and_then(|r| r.zoom.as_mut()) {
            zoom.x = centre_of(x, view.width, view.area.width).unwrap_or(zoom.x);
            zoom.y = centre_of(y, view.height, view.area.height).unwrap_or(zoom.y);
        }
        (x, y) != (view.x, view.y)
    }

    /// `h` and `l` or the arrows while zoomed: the view a quarter across, and at the page's
    /// edge a turn the way the arrow goes.
    fn zoom_side(&mut self, left: bool, rtl: bool) -> Option<Command> {
        let stage = self.reader_stage()?;
        let step = i32::from(stage.width.saturating_sub(ZOOM_PANEL) / 4).max(1);
        if self.zoom_move(if left { -step } else { step }, 0) {
            return None;
        }
        self.zoom_turn(left == rtl)
    }

    /// Space and Backspace while zoomed: the next part of the page in reading order, down
    /// first, then across the way the book reads; past the last part, the next page.
    fn zoom_part(&mut self, forward: bool) -> Option<Command> {
        let stage = self.reader_stage()?;
        let reader = self.reader_here()?;
        let rtl = reader.rtl();
        let view = self.zoom_view(reader, stage)?;
        let (vw, vh) = (i32::from(view.area.width), i32::from(view.area.height));
        let (w, h) = (view.width as i32, view.height as i32);
        let down = (vh * 9 / 10).max(1);
        let across = (vw * 9 / 10).max(1);
        // Across the way the book reads: leftwards in a right-to-left book.
        let ahead = if rtl { view.x > 0 } else { view.x + vw < w };
        let behind = if rtl { view.x + vw < w } else { view.x > 0 };
        let toward = if rtl { -across } else { across };
        if forward {
            if view.y + vh < h {
                self.zoom_move(0, down);
            } else if ahead {
                self.zoom_move(toward, -h);
            } else {
                return self.zoom_turn(true);
            }
        } else if view.y > 0 {
            self.zoom_move(0, -down);
        } else if behind {
            self.zoom_move(-toward, h);
        } else {
            return self.zoom_turn(false);
        }
        None
    }

    /// A turn while zoomed: on to the next page's first part, or back to the last part of the
    /// page before.
    fn zoom_turn(&mut self, forward: bool) -> Option<Command> {
        let task = self.turn(forward);
        let reader = self.reader_mut()?;
        let rtl = reader.rtl();
        if let Some(zoom) = reader.zoom.as_mut() {
            let start = if rtl { 1.0 } else { 0.0 };
            if forward {
                (zoom.x, zoom.y) = (start, 0.0);
            } else {
                (zoom.x, zoom.y) = (1.0 - start, 1.0);
            }
        }
        self.settle_zoom();
        task
    }

    /// `j`, `k`, Space and Backspace in a strip.
    fn strip_scroll(&mut self, by: i64) -> Option<Command> {
        let (layout, rows) = self.strip_now()?;
        let max = layout.total.saturating_sub(rows);
        let reader = self.reader_mut()?;
        reader.strip.top = (i64::from(reader.strip.top) + by).clamp(0, i64::from(max)) as u32;
        reader.strip.cols = layout.cols;
        self.strip_moved(true)
    }

    /// `g` and `G` in a strip: the open chapter's first image at the top, or its last at the
    /// foot of the window. The engine hears of the jump once it has landed.
    fn strip_jump(&mut self, last: bool) -> Option<Command> {
        let (layout, rows) = self.strip_now()?;
        let mut open = layout.open();
        let (item, leaf, _) = if last { open.last()? } else { open.next()? };
        let top = if last { (item.top + item.rows).saturating_sub(rows) } else { item.top };
        self.strip_land(&layout, rows, top, leaf)
    }

    /// Go to page in a strip: image `leaf` of the open chapter at the top of the window, told
    /// to the engine as a jump.
    pub(super) fn strip_to_leaf(&mut self, leaf: usize) -> Option<Command> {
        let (layout, rows) = self.strip_now()?;
        let top = layout.open().find(|(_, at, _)| *at == leaf)?.0.top;
        self.strip_land(&layout, rows, top, leaf)
    }

    /// The strip at row `top`, as far as it goes, with image `leaf` the jump's place.
    fn strip_land(
        &mut self,
        layout: &StripLayout,
        rows: u32,
        top: u32,
        leaf: usize,
    ) -> Option<Command> {
        let max = layout.total.saturating_sub(rows);
        let reader = self.reader_mut()?;
        reader.strip.top = top.min(max);
        reader.strip.cols = layout.cols;
        let (session, chapter) = (reader.session, reader.view.as_ref()?.chapter.id.clone());
        if let Some(task) = self.strip_moved(true) {
            return Some(task);
        }
        Some(Command::Reader(ReaderCommand::Shown {
            session,
            chapter,
            leaf: leaf as u32,
            how: TurnHow::Jump,
        }))
    }

    /// `n` in a strip: on to the divider into the next chapter, which opens it under the
    /// divider as scrolling there would.
    fn strip_next_chapter(&mut self) -> Option<Command> {
        let (layout, _) = self.strip_now()?;
        let divider = layout.next_divider()?.top;
        let reader = self.reader_mut()?;
        reader.end.next.as_ref()?;
        reader.strip.top = divider;
        reader.strip.cols = layout.cols;
        self.strip_moved(false).or_else(|| self.flow_on())
    }

    /// After the strip moved or was laid out: the images near the view are asked for and the
    /// image at the top is noted; then the next chapter opens once the divider into it crosses
    /// the middle of the window, or else where the strip settled is said in a moment.
    pub(super) fn strip_moved(&mut self, report: bool) -> Option<Command> {
        let now = self.clock.now_instant();
        let (layout, rows) = self.strip_now()?;
        let reader = self.reader_here()?;
        let top = reader.strip.top;
        // The pages near the view first: a jump past the divider still has the chapter's last
        // images to keep above.
        let reach = rows * 3 / 2;
        let (from, to) = (top.saturating_sub(reach), top + rows + reach);
        let near: Vec<usize> = layout
            .open()
            .filter(|(item, _, _)| item.top < to && item.top + item.rows > from)
            .map(|(_, leaf, _)| leaf)
            .collect();
        let at = layout
            .open()
            .find(|(item, _, _)| item.top + item.rows > top)
            .or_else(|| layout.open().last())
            .map(|(_, leaf, _)| leaf);
        let reader = self.reader_mut()?;
        reader.strip.cols = layout.cols;
        reader.strip.near = near.clone();
        if let Some(at) = at
            && at != reader.at
        {
            reader.at = at;
            reader.shown_at = now;
        }
        self.ask_pages(near);
        self.decode_in_view();
        let reader = self.reader_here()?;
        if reader.flowing.is_none()
            && reader.session != 0
            && reader.end.next.is_some()
            && let Some(divider) = layout.next_divider()
            && divider.top + divider.rows / 2 < top + rows / 2
        {
            return self.flow_on();
        }
        if report && let Some(reader) = self.reader_mut() {
            reader.strip.report = Some(now + REPORT_AFTER);
        }
        None
    }

    /// Where the strip is, as the engine keeps it: the image at the top and how far into it,
    /// or the last image whole once the chapter's foot is in view. Nothing over a chapter it ran
    /// on from.
    fn strip_at(&self) -> Option<Command> {
        let (layout, rows) = self.strip_now()?;
        let reader = self.reader_here()?;
        let chapter = reader.view.as_ref()?.chapter.id.clone();
        let (last, _, last_part) = layout.open().last()?;
        let top = reader.strip.top;
        let foot_visible = top + rows >= last.top + last.rows;
        let at = if foot_visible {
            StripPos { image: last_part.page, offset: 1.0 }
        } else {
            let (image, offset) = layout.place_at(top)?;
            StripPos { image, offset }
        };
        Some(Command::Reader(ReaderCommand::StripAt {
            session: reader.session,
            chapter,
            at,
            foot_visible,
        }))
    }

    /// On the idle tick: say where the strip settled, once it has been still for a moment.
    pub(super) fn tick_strip(&mut self) {
        let now = self.clock.now_instant();
        let Some(reader) = self.reader_here() else { return };
        if reader.strip.report.is_none_or(|due| due > now) {
            return;
        }
        let task = self.strip_at();
        if let Some(reader) = self.reader_mut() {
            reader.strip.report = None;
        }
        if let Some(task) = task {
            self.commands.push(task);
        }
    }

    /// The divider crossed the middle: the chapter's foot was read, it goes above, and the next
    /// chapter opens under the divider in the same scroll, with no opening screen. Only that
    /// chapter stays above: the ones before it go with their pictures, and the strip's place
    /// moves up by the rows they took, so the view does not move.
    fn flow_on(&mut self) -> Option<Command> {
        let reader = self.reader_here()?;
        if reader.session == 0 || reader.flowing.is_some() {
            return None;
        }
        let next = reader.end.next.clone()?;
        let chapter = reader.view.as_ref()?.chapter.clone();
        let last = reader
            .leaves
            .iter()
            .filter(|l| !l.end_page)
            .filter_map(|l| l.parts.first())
            .map(|p| p.page)
            .max()?;
        let images: Vec<(String, f32)> = reader
            .leaves
            .iter()
            .filter(|l| !l.end_page)
            .filter_map(|l| l.parts.first())
            .map(|p| {
                let (w, h) = reader.part_size(p);
                (reader.picture_key(PartKey { page: p.page, half: p.half }), w / h.max(1.0))
            })
            .collect();
        let lead =
            reader.end.preview.as_ref().filter(|(id, _)| *id == next.id).map(|(_, (w, h))| {
                (next.id.clone(), reader.preview_key(&next.id), *w as f32 / (*h).max(1) as f32)
            });
        let session = reader.session;
        let (cols, cell) = (self.strip_now()?.0.cols, self.cell());
        self.commands.push(Command::Reader(ReaderCommand::StripAt {
            session,
            chapter: chapter.id.clone(),
            at: StripPos { image: last, offset: 1.0 },
            foot_visible: true,
        }));
        let reader = self.reader_mut()?;
        let older = std::mem::take(&mut reader.strip.trail);
        let taken: u32 = older
            .iter()
            .map(|gone| {
                let images: u32 =
                    gone.images.iter().map(|(_, ratio)| image_rows(cols, *ratio, cell)).sum();
                images + DIVIDER_ROWS
            })
            .sum();
        reader.strip.top = reader.strip.top.saturating_sub(taken);
        let mut gone_keys: Vec<String> =
            older.into_iter().flat_map(|gone| gone.images).map(|(key, _)| key).collect();
        // The first image this chapter showed from the preview before its own came.
        gone_keys.extend(
            reader
                .strip
                .lead
                .take()
                .map(|(_, key, _)| key)
                .filter(|key| lead.as_ref().is_none_or(|(_, new, _)| new != key)),
        );
        for key in &gone_keys {
            reader.decoding.remove(key);
        }
        reader.strip.trail.push(Gone {
            chapter,
            images,
            next: next.clone(),
            source: reader.end.next_source.clone(),
        });
        reader.strip.lead = lead;
        reader.strip.report = None;
        reader.strip.near.clear();
        reader.flowing = Some(next.id.clone());
        reader.asked = Some(next.id.clone());
        reader.asked_number = Some(Reader::number_of(&next));
        reader.forget_pages();
        for key in &gone_keys {
            self.images.forget(key);
        }
        Some(Command::Reader(ReaderCommand::OpenChapter { session, chapter: next.id }))
    }

    /// A strip's leaves came: the place is where the engine said on the first layout, where
    /// the strip was on a new one, and where the divider left it when it flowed on.
    pub(super) fn place_strip(&mut self, first: bool, kept: Option<(u32, f32)>) {
        let Some(reader) = self.reader_mut() else { return };
        // Ran on from the chapter above: the strip stays where the divider left it.
        let flowed = reader.flowing.take().is_some();
        let Some((layout, rows)) = self.strip_now() else { return };
        let Some(reader) = self.reader_mut() else { return };
        if !flowed {
            let start = reader.view.as_ref().map(|v| &v.start);
            let place = if first {
                start
                    .and_then(|s| {
                        s.strip.map(|p| (p.image, p.offset)).or(s.page.map(|p| (p, 0.0)))
                    })
                    .or(kept)
            } else {
                kept
            };
            let (page, offset) = place.unwrap_or((0, 0.0));
            reader.strip.top = layout.row_of(page, offset).unwrap_or(0);
        }
        let max = layout.total.saturating_sub(rows);
        reader.strip.top = reader.strip.top.min(max);
        reader.strip.cols = layout.cols;
        let session = reader.session;
        let chapter = reader.view.as_ref().map(|v| v.chapter.id.clone());
        if let Some(task) = self.strip_moved(false) {
            self.commands.push(task);
            return;
        }
        if let (Some(chapter), Some(reader)) = (chapter, self.reader_here()) {
            self.commands.push(Command::Reader(ReaderCommand::Shown {
                session,
                chapter,
                leaf: reader.at as u32,
                how: TurnHow::Placed,
            }));
        }
    }

    /// The strip's place in the open chapter, for a new layout to keep.
    pub(super) fn strip_place(&self) -> Option<(u32, f32)> {
        let reader = self.reader_here()?;
        if !reader.strips() {
            return None;
        }
        let (layout, _) = self.strip_now()?;
        layout.place_at(reader.strip.top)
    }

    /// Put the strip's top back on `place`, an image of the open chapter and how far into it,
    /// once the strip is laid out again.
    pub(super) fn keep_strip_place_at(&mut self, place: Option<(u32, f32)>) {
        let Some((page, offset)) = place else { return };
        let Some((layout, rows)) = self.strip_now() else { return };
        let Some(top) = layout.row_of(page, offset) else { return };
        let max = layout.total.saturating_sub(rows);
        if let Some(reader) = self.reader_mut() {
            reader.strip.top = top.min(max);
            reader.strip.cols = layout.cols;
        }
    }

    /// The window changed: a strip keeps its place when its column changes width.
    pub(super) fn keep_strip_place(&mut self, window: (u16, u16)) {
        let Some(reader) = self.reader_here() else { return };
        if !reader.strips() {
            return;
        }
        let cols = strip_cols(reader, window.0);
        if reader.strip.cols == cols || reader.strip.cols == 0 {
            return;
        }
        let old = self.strip_layout(reader, reader.strip.cols);
        let place = old.place_at(reader.strip.top);
        let new = self.strip_layout(reader, cols);
        let top = match place {
            Some((page, offset)) => new.row_of(page, offset).unwrap_or(0),
            None => {
                (u64::from(reader.strip.top) * u64::from(new.total)
                    / u64::from(old.total.max(1))) as u32
            }
        };
        if let Some(reader) = self.reader_mut() {
            reader.strip.top = top;
            reader.strip.cols = cols;
        }
    }

    /// A chapter counted as read in a strip says so on the status rule, with the trackers.
    pub(super) fn strip_read(&mut self, chapter: &ChapterId, trackers: &[String]) {
        let now = self.clock.now_instant();
        let offline = self.offline;
        let Some(reader) = self.reader.as_mut() else { return };
        if !reader.strips() {
            return;
        }
        let row = reader
            .strip
            .trail
            .iter()
            .map(|g| &g.chapter)
            .chain(reader.view.as_ref().map(|v| &v.chapter))
            .find(|c| c.id == *chapter);
        let Some(number) = row.map(Reader::number_of) else { return };
        let names = trackers.join(", ");
        let text = if trackers.is_empty() {
            t::tui_reader_strip_read(&number)
        } else if offline {
            t::tui_reader_strip_read_offline(&number, &names)
        } else {
            t::tui_reader_strip_read_tracker(&number, &names)
        };
        reader.notice = super::reader::own_notice(text, now);
    }
}

/// The medium a reader setting is set for when the chapter has not said.
trait OrManga {
    fn unwrap_or_default_manga(self) -> kaeri_core::media::Medium;
}

impl OrManga for Option<kaeri_core::media::Medium> {
    fn unwrap_or_default_manga(self) -> kaeri_core::media::Medium {
        self.unwrap_or(kaeri_core::media::Medium::Manga)
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;
    use kaeri_core::{books::WorkId, media::Medium};
    use kaeri_model::{
        event::Event,
        reader::{ReaderEvent, ReaderSurface},
    };

    use super::super::reader::tests::{SESSION, key, leaves, reading, row, view};
    use super::*;
    use crate::app::{FrameReport, fixtures::*};

    fn press(a: &mut App, code: KeyCode) -> Option<Command> {
        a.press(key(code), 20)
    }

    /// The surfaces sent since the last look.
    fn surfaces(a: &mut App) -> Vec<ReaderSurface> {
        a.take_commands()
            .into_iter()
            .filter_map(|c| match c {
                Command::Reader(ReaderCommand::Surface { surface, .. }) => Some(surface),
                _ => None,
            })
            .collect()
    }

    fn reader(a: &App) -> &Reader {
        a.reader_here().expect("the reader")
    }

    fn fit(name: &str) -> Option<Command> {
        Some(Command::Reader(ReaderCommand::SetSetting {
            session: Some(SESSION),
            medium: Medium::Manga,
            work: None,
            key: ReaderSettingKey::Fit,
            scope: SettingScope::Device,
            value: ReaderValue::Choice { name: name.into() },
        }))
    }

    #[test]
    fn f_switches_fit_width_on_this_computer_and_sends_one_page_across() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        assert_eq!(press(&mut a, KeyCode::Char('f')), fit("width"));
        assert!(reader(&a).fits_width());
        let sent = surfaces(&mut a);
        assert!(sent.last().is_some_and(|s| !s.two_pages), "{sent:?}");
        assert_eq!(press(&mut a, KeyCode::Char('f')), fit("screen"));
        assert!(!reader(&a).fits_width());
    }

    #[test]
    fn at_fit_width_j_and_k_scroll_and_a_turn_starts_the_next_page_at_its_top() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        press(&mut a, KeyCode::Char('f'));
        a.take_commands();
        assert_eq!(press(&mut a, KeyCode::Char('j')), None);
        assert_eq!(reader(&a).scroll, 3);
        press(&mut a, KeyCode::Char('j'));
        press(&mut a, KeyCode::Char('k'));
        assert_eq!(reader(&a).scroll, 3);
        press(&mut a, KeyCode::Char('k'));
        press(&mut a, KeyCode::Char('k'));
        assert_eq!(reader(&a).scroll, 0, "not above the top");
        press(&mut a, KeyCode::Char('j'));
        let turned = press(&mut a, KeyCode::Left);
        assert!(
            matches!(turned, Some(Command::Reader(ReaderCommand::Shown { leaf: 2, .. }))),
            "{turned:?}"
        );
        assert_eq!(reader(&a).scroll, 0);
    }

    #[test]
    fn at_fit_width_space_scrolls_a_screen_and_turns_at_the_foot() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        press(&mut a, KeyCode::Char('f'));
        let stage = a.reader_stage().unwrap();
        let view = a.fit_view(reader(&a), stage).unwrap();
        let max = view.height - u32::from(stage.height);
        let mut turned = None;
        for _ in 0..20 {
            turned = press(&mut a, KeyCode::Char(' '));
            if turned.is_some() {
                break;
            }
            assert!(reader(&a).scroll <= max);
        }
        assert!(
            matches!(turned, Some(Command::Reader(ReaderCommand::Shown { leaf: 2, .. }))),
            "{turned:?}"
        );
        assert_eq!(reader(&a).scroll, 0);
    }

    #[test]
    fn z_zooms_to_200_and_plus_and_minus_step_through_150_200_and_300() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        press(&mut a, KeyCode::Char('z'));
        assert_eq!(reader(&a).zoom.map(|z| z.level), Some(2.0));
        assert_eq!(surfaces(&mut a).last().map(|s| s.zoom), Some(2.0), "the zoomed surface");
        let mut levels = Vec::new();
        for code in ['+', '+', '-', '-', '-'] {
            press(&mut a, KeyCode::Char(code));
            levels.push(reader(&a).zoom.map(|z| z.level).unwrap());
        }
        assert_eq!(levels, vec![3.0, 3.0, 2.0, 1.5, 1.5]);
        press(&mut a, KeyCode::Char('z'));
        assert_eq!(reader(&a).zoom, None);
        assert_eq!(surfaces(&mut a).last().map(|s| s.zoom), Some(1.0), "dropped on leave");
    }

    #[test]
    fn plus_zooms_in_from_the_page_and_esc_leaves_zoom_not_the_reader() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        press(&mut a, KeyCode::Char('+'));
        assert_eq!(reader(&a).zoom.map(|z| z.level), Some(1.5));
        press(&mut a, KeyCode::Esc);
        assert_eq!(reader(&a).zoom, None);
        assert!(a.reader_here().is_some(), "still reading");
    }

    #[test]
    fn zoomed_h_j_k_l_move_and_the_arrow_turns_at_the_page_s_edge() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        press(&mut a, KeyCode::Char('z'));
        press(&mut a, KeyCode::Char('+'));
        let start = reader(&a).zoom.unwrap();
        assert!(start.x > 0.5 && start.y < 0.5, "a right-to-left page starts top right");
        press(&mut a, KeyCode::Char('j'));
        let down = reader(&a).zoom.unwrap();
        assert!(down.y > start.y, "{down:?}");
        press(&mut a, KeyCode::Char('k'));
        assert!((reader(&a).zoom.unwrap().y - start.y).abs() < 1e-4);
        a.take_commands();
        let mut turned = None;
        for _ in 0..20 {
            turned = press(&mut a, KeyCode::Char('h'));
            if turned.is_some() {
                break;
            }
        }
        assert!(
            matches!(turned, Some(Command::Reader(ReaderCommand::Shown { leaf: 2, .. }))),
            "left is forward in a right-to-left book: {turned:?}"
        );
        let landed = reader(&a).zoom.unwrap();
        assert_eq!(landed.level, 3.0, "the zoom stays on the next page");
        assert!(landed.x > 0.5 && landed.y < 0.5, "at its first part: {landed:?}");
        let back = press(&mut a, KeyCode::Char('l'));
        assert!(
            matches!(back, Some(Command::Reader(ReaderCommand::Shown { leaf: 1, .. }))),
            "{back:?}"
        );
    }

    /// Chapter 14 as a strip of four images at its second, in a 120 by 34 window.
    fn strip() -> App {
        kaeri_i18n::set_language("en");
        let engine = crate::image::ImageEngine::halfblocks();
        let mut a =
            App::with_images(SettingsView::default(), Palette::dark(), Keymap::new(), engine);
        a.clock = crate::clock::Clock::fixed(1_793_000_000, crate::clock::Zone::utc());
        a.took_frame(FrameReport { window: Some((120, 34)), ..FrameReport::default() });
        a.open_reader(WorkId::new("a:1"), Some(ChapterId("c14".into())));
        let mut strip_view = view(ReadDirection::TopToBottom);
        strip_view.mode = ReaderMode::Strip;
        a.apply(Event::Reader(ReaderEvent::Opened(Box::new(strip_view))));
        let mut images = leaves();
        for part in images.iter_mut().flat_map(|l| l.parts.iter_mut()) {
            part.ratio = Some(0.6);
        }
        a.apply(Event::Reader(ReaderEvent::Leaves {
            session: SESSION,
            chapter: ChapterId("c14".into()),
            leaves: images,
        }));
        a
    }

    fn strip_at(a: &mut App) -> Vec<(StripPos, bool)> {
        a.take_commands()
            .into_iter()
            .filter_map(|c| match c {
                Command::Reader(ReaderCommand::StripAt { at, foot_visible, .. }) => {
                    Some((at, foot_visible))
                }
                _ => None,
            })
            .collect()
    }

    /// The reader settings open on manga's rows: mode, fit and the strip gap as given.
    fn manga_settings(a: &mut App, mode: &str) {
        press(a, KeyCode::Char(','));
        let row = |key, name: &str| ReaderSettingRow {
            key,
            value: ReaderValue::Choice { name: name.into() },
            scope: SettingScope::Device,
            detected: false,
            overridden: false,
            choices: Vec::new(),
        };
        a.apply(Event::Reader(ReaderEvent::Settings(ReaderSettingsView {
            medium: Medium::Manga,
            work: Some(WorkId::new("a:1")),
            rows: vec![
                row(ReaderSettingKey::Mode, mode),
                row(ReaderSettingKey::Fit, "screen"),
                row(ReaderSettingKey::StripGap, "none"),
            ],
            tap_zones: None,
        })));
        a.take_commands();
    }

    #[test]
    fn the_settings_offer_every_fit_and_at_fit_height_a_wide_page_moves_from_its_reading_side()
    {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        let mut wide = leaves();
        for part in wide.iter_mut().flat_map(|l| l.parts.iter_mut()) {
            part.ratio = Some(3.0);
        }
        a.apply(Event::Reader(ReaderEvent::Leaves {
            session: SESSION,
            chapter: ChapterId("c14".into()),
            leaves: wide,
        }));
        manga_settings(&mut a, "paged");
        let items = a.reader_setting_items();
        let row = items.iter().find(|i| i.key == ReaderSettingKey::Fit).unwrap();
        let words: Vec<&str> = match &row.shown {
            crate::app::SettingShown::Choices(c) => c.iter().map(|(w, _)| w.as_str()).collect(),
            other => panic!("{other:?}"),
        };
        assert_eq!(words, ["Screen", "Width", "Height"]);
        let at = items.iter().position(|i| i.key == ReaderSettingKey::Fit).unwrap();
        for _ in 0..at {
            press(&mut a, KeyCode::Char('j'));
        }
        let task = press(&mut a, KeyCode::Left);
        assert!(reader(&a).fits_height());
        assert!(
            matches!(
                &task,
                Some(Command::Reader(ReaderCommand::SetSetting {
                    key: ReaderSettingKey::Fit,
                    scope: SettingScope::Device,
                    value: ReaderValue::Choice { name },
                    ..
                })) if name == "height"
            ),
            "{task:?}"
        );
        press(&mut a, KeyCode::Esc);

        let stage = a.reader_stage().unwrap();
        let view = a.fit_view(reader(&a), stage).unwrap();
        assert_eq!(view.height, u32::from(stage.height), "as tall as the stage");
        let max = view.width - u32::from(stage.width);
        assert!(max > 0, "wider than the stage: {view:?}");
        assert_eq!(view.x, max as i32, "right to left starts at the right");
        press(&mut a, KeyCode::Char('j'));
        let view = a.fit_view(reader(&a), stage).unwrap();
        assert_eq!(view.x, max as i32 - 6, "six columns along");
        let mut turned = None;
        for _ in 0..20 {
            turned = press(&mut a, KeyCode::Char(' '));
            if turned.is_some() {
                break;
            }
        }
        assert!(
            matches!(turned, Some(Command::Reader(ReaderCommand::Shown { leaf: 2, .. }))),
            "{turned:?}"
        );
        assert_eq!(
            press(&mut a, KeyCode::Char('f')),
            fit("screen"),
            "f goes back to the screen"
        );
    }

    #[test]
    fn a_small_strip_gap_parts_the_images_and_the_strip_keeps_its_place() {
        let mut a = strip();
        for _ in 0..4 {
            press(&mut a, KeyCode::Char('j'));
        }
        let place = a.strip_place();
        let (before, _) = a.strip_now().unwrap();
        let tops = |layout: &StripLayout| -> Vec<(u32, u32)> {
            layout.open().map(|(item, _, _)| (item.top, item.rows)).collect()
        };
        for pair in tops(&before).windows(2) {
            assert_eq!(pair[1].0, pair[0].0 + pair[0].1, "no gap at first");
        }
        manga_settings(&mut a, "strip");
        let at = a
            .reader_setting_items()
            .iter()
            .position(|i| i.key == ReaderSettingKey::StripGap)
            .unwrap();
        for _ in 0..at {
            press(&mut a, KeyCode::Char('j'));
        }
        let task = press(&mut a, KeyCode::Right);
        assert!(
            matches!(
                &task,
                Some(Command::Reader(ReaderCommand::SetSetting {
                    key: ReaderSettingKey::StripGap,
                    scope: SettingScope::Device,
                    value: ReaderValue::Choice { name },
                    ..
                })) if name == "small"
            ),
            "{task:?}"
        );
        let (after, _) = a.strip_now().unwrap();
        for pair in tops(&after).windows(2) {
            assert_eq!(pair[1].0, pair[0].0 + pair[0].1 + STRIP_GAP_ROWS, "a row between");
        }
        assert_eq!(a.strip_place(), place, "the same image, as far into it");
    }

    #[test]
    fn a_strip_opens_at_its_place_and_asks_for_the_images_near_it() {
        let mut a = strip();
        let (layout, _) = a.strip_now().unwrap();
        assert_eq!(layout.cols, 48, "the board's 48 columns at 120");
        let second = layout.image_at(0).map(|i| i.rows).unwrap();
        assert_eq!(reader(&a).strip.top, second, "at the second image's top");
        let asked: Vec<u32> = a
            .take_commands()
            .iter()
            .filter_map(|c| match c {
                Command::Reader(ReaderCommand::NeedPage { page, .. }) => Some(*page),
                _ => None,
            })
            .collect();
        assert!(asked.contains(&0) && asked.contains(&1) && asked.contains(&2), "{asked:?}");
    }

    #[test]
    fn in_a_strip_j_and_k_scroll_three_rows_and_space_most_of_a_screen() {
        let mut a = strip();
        let top = reader(&a).strip.top;
        press(&mut a, KeyCode::Char('j'));
        assert_eq!(reader(&a).strip.top, top + 3);
        press(&mut a, KeyCode::Char('k'));
        press(&mut a, KeyCode::Char('k'));
        assert_eq!(reader(&a).strip.top, top - 3);
        let rows = u32::from(a.reader_stage().unwrap().height);
        press(&mut a, KeyCode::Char(' '));
        assert_eq!(reader(&a).strip.top, top - 3 + rows * 9 / 10);
        press(&mut a, KeyCode::Backspace);
        assert_eq!(reader(&a).strip.top, top - 3);
    }

    #[test]
    fn go_to_page_in_a_strip_puts_that_image_at_the_top() {
        let mut a = strip();
        a.take_commands();
        a.handle(crate::keymap::Action::ReaderGoToPage, 20);
        assert_eq!(a.go_to_count(), 4, "the images, not the end page");
        a.type_char('3');
        let task = a.handle(crate::keymap::Action::Open, 20);
        let (layout, _) = a.strip_now().unwrap();
        let third = layout.open().nth(2).map(|(item, _, _)| item.top).unwrap();
        let max = layout.total.saturating_sub(u32::from(a.reader_stage().unwrap().height));
        assert_eq!(reader(&a).strip.top, third.min(max));
        assert_eq!(
            task,
            Some(Command::Reader(ReaderCommand::Shown {
                session: SESSION,
                chapter: ChapterId("c14".into()),
                leaf: 2,
                how: TurnHow::Jump,
            }))
        );
    }

    #[test]
    fn a_strip_says_where_it_settled_half_a_second_after_it_stopped() {
        let mut a = strip();
        a.take_commands();
        press(&mut a, KeyCode::Char('j'));
        press(&mut a, KeyCode::Char('j'));
        a.tick_toasts();
        assert!(strip_at(&mut a).is_empty(), "not while it moves");
        a.clock.advance(Duration::from_millis(500));
        a.tick_toasts();
        let at = strip_at(&mut a);
        assert_eq!(at.len(), 1, "{at:?}");
        let (pos, foot) = at[0];
        assert_eq!(pos.image, 1);
        assert!(pos.offset > 0.1 && pos.offset < 0.2, "{pos:?}");
        assert!(!foot);
        a.tick_toasts();
        assert!(strip_at(&mut a).is_empty(), "once");
    }

    #[test]
    fn n_in_a_strip_reads_the_foot_and_runs_on_into_the_next_chapter() {
        let mut a = strip();
        a.take_commands();
        let task = press(&mut a, KeyCode::Char('n'));
        assert_eq!(
            task,
            Some(Command::Reader(ReaderCommand::OpenChapter {
                session: SESSION,
                chapter: ChapterId("c15".into()),
            }))
        );
        let at = strip_at(&mut a);
        assert_eq!(at, vec![(StripPos { image: 3, offset: 1.0 }, true)], "the foot was read");
        let r = reader(&a);
        assert_eq!(r.strip.trail.len(), 1, "chapter 14 stays above");
        assert_eq!(r.flowing, Some(ChapterId("c15".into())));
        assert_eq!(a.reader_screen(), Some(ReaderScreen::Pages), "no opening screen");
        // Chapter 15 opens under the divider, where the strip is.
        let top = reader(&a).strip.top;
        let mut next = view(ReadDirection::TopToBottom);
        next.mode = ReaderMode::Strip;
        next.chapter = row(15);
        next.next = Some(row(16));
        a.apply(Event::Reader(ReaderEvent::Opened(Box::new(next))));
        a.apply(Event::Reader(ReaderEvent::Leaves {
            session: SESSION,
            chapter: ChapterId("c15".into()),
            leaves: leaves(),
        }));
        let r = reader(&a);
        assert_eq!(r.flowing, None);
        assert_eq!(r.strip.top, top);
        assert_eq!(r.strip.trail.len(), 1);
        assert_eq!(r.at, 0, "chapter 15's first image");
    }

    #[test]
    fn a_strip_keeps_only_the_chapter_before_above_and_lets_go_of_older_pictures() {
        let mut a = strip();
        let keep = |a: &mut App, chapter: &str| {
            for page in 0..4 {
                let key = format!("reader/{SESSION}/{chapter}/{page}");
                a.images.insert_page(&key, image::DynamicImage::new_rgb8(6, 10));
            }
        };
        keep(&mut a, "c14");
        press(&mut a, KeyCode::Char('n'));
        let mut next = view(ReadDirection::TopToBottom);
        next.mode = ReaderMode::Strip;
        next.chapter = row(15);
        next.next = Some(row(16));
        a.apply(Event::Reader(ReaderEvent::Opened(Box::new(next))));
        a.apply(Event::Reader(ReaderEvent::Leaves {
            session: SESSION,
            chapter: ChapterId("c15".into()),
            leaves: leaves(),
        }));
        keep(&mut a, "c15");
        let (layout, _) = a.strip_now().expect("a strip");
        // Chapter 14 and its divider take the rows above chapter 15's first image.
        let above = layout.open().next().map(|(item, _, _)| item.top).expect("an image");
        let divider = layout.next_divider().expect("a divider into 16").top;
        assert!(above > 0);
        assert!(a.images.has(&format!("reader/{SESSION}/c14/0")), "14 above 15 still");
        press(&mut a, KeyCode::Char('n'));
        let r = reader(&a);
        let trail: Vec<&str> = r.strip.trail.iter().map(|g| g.chapter.id.0.as_str()).collect();
        assert_eq!(trail, vec!["c15"], "only the chapter before stays above");
        assert_eq!(r.strip.top, divider - above, "the view stays on the divider into 16");
        assert!(!a.images.has(&format!("reader/{SESSION}/c14/0")), "14's pictures went");
        assert!(a.images.has(&format!("reader/{SESSION}/c15/3")), "15's stay for the trail");
    }

    #[test]
    fn the_read_in_a_strip_shows_on_the_status_rule_with_the_trackers() {
        let mut a = strip();
        press(&mut a, KeyCode::Char('n'));
        a.apply(Event::Reader(ReaderEvent::ChapterRead {
            work: WorkId::new("a:1"),
            chapter: ChapterId("c14".into()),
            trackers: vec!["AniList".into()],
        }));
        let notice = a.reader_notice().map(|n| n.text.clone());
        assert_eq!(notice.as_deref(), Some("Chapter 14 counted as read · AniList updated"));
    }

    #[test]
    fn m_keeps_paged_or_strip_for_the_series_and_the_settings_switch_the_mode() {
        let mut a = reading(ReadDirection::RightToLeft, 120, 34);
        a.take_commands();
        assert_eq!(
            press(&mut a, KeyCode::Char('m')),
            Some(Command::Reader(ReaderCommand::SetSetting {
                session: Some(SESSION),
                medium: Medium::Manga,
                work: Some(WorkId::new("a:1")),
                key: ReaderSettingKey::Mode,
                scope: SettingScope::Series,
                value: ReaderValue::Choice { name: "strip".into() },
            }))
        );
        a.apply(Event::Reader(ReaderEvent::Settings(ReaderSettingsView {
            medium: Medium::Manga,
            work: Some(WorkId::new("a:1")),
            rows: vec![ReaderSettingRow {
                key: ReaderSettingKey::Mode,
                value: ReaderValue::Choice { name: "strip".into() },
                scope: SettingScope::Series,
                detected: false,
                overridden: true,
                choices: Vec::new(),
            }],
            tap_zones: None,
        })));
        assert!(reader(&a).strips());
        assert_eq!(
            reader(&a).view.as_ref().map(|v| v.direction),
            Some(ReadDirection::TopToBottom)
        );
        assert!(surfaces(&mut a).last().is_some_and(|s| !s.two_pages), "one image across");
        // The engine lays it out again as a strip; the page in view stays at the top.
        let mut images = leaves();
        for part in images.iter_mut().flat_map(|l| l.parts.iter_mut()) {
            part.ratio = Some(0.6);
        }
        a.apply(Event::Reader(ReaderEvent::Leaves {
            session: SESSION,
            chapter: ChapterId("c14".into()),
            leaves: images,
        }));
        assert_eq!(reader(&a).at, 1, "the second page, as it was");
    }
}
