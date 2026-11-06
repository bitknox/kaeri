//! The page pipeline in its order, for one screen: what reaches the app for each page.
//!
//! [`Renderer::open`] decodes a page, checks it for colour, finds its crop box and whether it
//! is a spread. [`Renderer::scale`] crops (or takes a split half) and resizes once to the
//! screen, with the tone curve. [`Renderer::finish`] makes the screen's output: dithered greys
//! on grey e-ink, the rainbow filter on Kaleido when it is on, colour with the panel's
//! saturation on colour screens. [`Renderer::render`] runs all three for a page part.
//!
//! [`delivery`] says when an LCD page goes to the app as its original file instead.

use crate::colour::check_colour;
use crate::crop::content_box;
use crate::decode::{DecodeLimits, SourceFormat, decode_for};
use crate::layout::is_spread;
use crate::rainbow::RainbowFilter;
use crate::resize::{AREA_ABOVE, Fit, resize, shrink_factor};
use crate::tone::{Levels, ToneCurve};
use crate::{Bitmap, Error, PixelFormat, PixelRect, dither};

/// The screen a page is drawn on. Mirrors the model's `Panel`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Panel {
    /// Grey e-ink (Carta and the like): 16 greys, dithered.
    Grey,
    /// Kaleido 3 colour e-ink: a colour filter over grey ink.
    Kaleido3,
    /// Gallery 3 colour e-ink: coloured pigments.
    Gallery3,
    /// E-ink that may be colour: pictures stay in colour until the person says otherwise.
    UnknownColour,
    /// A phone, tablet or computer screen.
    #[default]
    Lcd,
}

impl Panel {
    pub fn is_eink(self) -> bool {
        self != Panel::Lcd
    }

    /// Whether the screen shows colour at all.
    pub fn shows_colour(self) -> bool {
        self != Panel::Grey
    }

    /// The saturation colour pages get by default. Kaleido 3's filter mutes colour, so it
    /// gets 1.2; Gallery 3's pigments do not. Both are to be measured on the devices.
    pub fn default_saturation(self) -> f32 {
        match self {
            Panel::Kaleido3 => 1.2,
            _ => 1.0,
        }
    }
}

/// A series' colour choice: Auto follows each page's colour check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColourChoice {
    #[default]
    Auto,
    Colour,
    Greys,
}

/// How pages look on this screen: the series' and the device's settings that reach the
/// pipeline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub colour: ColourChoice,
    /// Crop borders. Never applied to spreads.
    pub crop: bool,
    /// Autocontrast on e-ink, where it applies (see [`crate::tone`]).
    pub autocontrast: bool,
    /// Gamma on e-ink; 1.0 leaves the page as it is.
    pub gamma: f32,
    /// The rainbow filter, on Kaleido screens. Off by default.
    pub rainbow_filter: bool,
    /// Colour pages' saturation on colour e-ink; `None` takes the panel's default.
    pub saturation: Option<f32>,
    /// Colour pages' contrast on colour e-ink; 1.0 leaves it.
    pub contrast: f32,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            colour: ColourChoice::Auto,
            crop: false,
            autocontrast: true,
            gamma: 1.0,
            rainbow_filter: false,
            saturation: None,
            contrast: 1.0,
        }
    }
}

/// The screen pages are prepared for, in its real pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub panel: Panel,
    pub width: u32,
    pub height: u32,
}

/// How an LCD page reaches the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// The page file as extracted; the platform's decoder and GPU draw it, which keeps pinch
    /// zoom sharp.
    Original,
    /// Processed by the engine to the final size first.
    Processed,
}

/// Whether a page goes to the app as its original file. Only on LCD, and only when it is not
/// shrunk by more than [`AREA_ABOVE`], since a platform's subsampling point-samples and turns
/// screentone to moiré. Pages the engine cannot decode go as originals whatever their size,
/// as there is nothing else to send. Every e-ink page is processed.
pub fn delivery(format: SourceFormat, region: (u32, u32), target: &Target) -> Delivery {
    if target.panel.is_eink() {
        return Delivery::Processed;
    }
    let fitted =
        Fit::Within { width: target.width, height: target.height }.size(region.0, region.1);
    if !format.decodes() || shrink_factor(region, fitted) <= AREA_ABOVE {
        Delivery::Original
    } else {
        Delivery::Processed
    }
}

/// A decoded page and what the pipeline learnt about it.
#[derive(Clone, Debug)]
pub struct SourcePage {
    /// Grey when the page has no colour or will not be shown in colour; RGB otherwise.
    pub bitmap: Bitmap,
    pub format: SourceFormat,
    /// The page has colour (the YCbCr check, or the series' choice).
    pub colour: bool,
    pub spread: bool,
    /// What to show of the page: the crop box when crop is on, else the whole page.
    pub content: PixelRect,
}

/// A processed page, ready to be written to the page cache as a raw file: no header, rows
/// packed, [`PixelFormat::Gray8`] on a grey panel and [`PixelFormat::Rgba8`] on every colour
/// screen.
#[derive(Clone, Debug)]
pub struct RawPage {
    pub bitmap: Bitmap,
    pub colour: bool,
}

impl RawPage {
    pub fn format(&self) -> PixelFormat {
        self.bitmap.format()
    }

    pub fn width(&self) -> u32 {
        self.bitmap.width()
    }

    pub fn height(&self) -> u32 {
        self.bitmap.height()
    }

    /// Bytes per row, as the model's `PageEncoding::Raw` carries it.
    pub fn stride(&self) -> u32 {
        self.bitmap.stride() as u32
    }

    /// The file's contents.
    pub fn bytes(&self) -> &[u8] {
        self.bitmap.data()
    }
}

/// The pipeline for one target and look, keeping the rainbow filter's plans between pages.
#[derive(Debug)]
pub struct Renderer {
    target: Target,
    look: Look,
    limits: DecodeLimits,
    threads: usize,
    /// The last two page sizes' filters: a single page and a spread alternate.
    filters: Vec<RainbowFilter>,
}

impl Renderer {
    pub fn new(target: Target, look: Look) -> Self {
        Renderer {
            target,
            look,
            limits: DecodeLimits::default(),
            threads: 1,
            filters: Vec::new(),
        }
    }

    /// The decode cap; see [`DecodeLimits`].
    pub fn with_limits(mut self, limits: DecodeLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Threads the rainbow filter may use. On an e-reader, two at most, so the screen keeps a
    /// core.
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads.max(1);
        self
    }

    pub fn target(&self) -> &Target {
        &self.target
    }

    pub fn look(&self) -> &Look {
        &self.look
    }

    /// Decodes a page and checks it: colour, spread, crop box.
    pub fn open(&self, bytes: &[u8]) -> Result<SourcePage, Error> {
        let room = (self.target.width, self.target.height);
        let mut bitmap = decode_for(bytes, self.limits, Some(room))?;
        let format = SourceFormat::sniff(bytes).ok_or(Error::UnknownFormat)?;
        let has_rgb = bitmap.format() != PixelFormat::Gray8;
        let colour = match self.look.colour {
            ColourChoice::Greys => false,
            ColourChoice::Colour => has_rgb,
            ColourChoice::Auto => has_rgb && check_colour(&bitmap).colour,
        };
        // Pages shown in grey are worked on in grey: a third of the memory and the resize.
        if !colour || !self.target.panel.shows_colour() {
            bitmap = bitmap.into_gray();
        }
        let spread = is_spread(bitmap.width(), bitmap.height());
        let content =
            if self.look.crop && !spread { content_box(&bitmap) } else { bitmap.bounds() };
        Ok(SourcePage { bitmap, format, colour, spread, content })
    }

    /// Crops to `region` (a split half, in page pixels) or the content box, and resizes once.
    /// Autocontrast and gamma apply on e-ink to grey story pages; `strip` leaves the levels
    /// alone.
    pub fn scale(
        &self,
        page: &SourcePage,
        region: Option<PixelRect>,
        fit: Fit,
        strip: bool,
    ) -> Result<Bitmap, Error> {
        let region = region.unwrap_or(page.content);
        let (w, h) = fit.size(region.width, region.height);
        let mut out = resize(&page.bitmap, region, w, h)?;
        if self.target.panel.is_eink() && out.format() == PixelFormat::Gray8 {
            let levels = (self.look.autocontrast && !page.colour && !strip)
                .then(|| Levels::measure(&page.bitmap, region))
                .filter(|l| !l.is_narrow());
            ToneCurve::new(levels, self.look.gamma).apply(&mut out);
        }
        Ok(out)
    }

    /// Makes the screen's output from a scaled page or strip cut.
    pub fn finish(&mut self, mut page: Bitmap, colour: bool) -> Result<RawPage, Error> {
        let panel = self.target.panel;
        let page = match (panel, page.format()) {
            (Panel::Grey, _) => {
                let mut grey = page.into_gray();
                dither::ordered(&mut grey)?;
                grey
            }
            // Colour screens take RGBA whatever the page: one format for the app to draw.
            (Panel::Kaleido3, PixelFormat::Gray8) if self.look.rainbow_filter => {
                let threads = self.threads;
                self.filter(page.width(), page.height()).apply(&mut page, threads)?;
                page.into_rgba()
            }
            (_, PixelFormat::Gray8) => page.into_rgba(),
            (Panel::Lcd, _) => page.into_rgba(),
            (_, _) => {
                let saturation = self.look.saturation.unwrap_or(panel.default_saturation());
                let mut rgba = page.into_rgba();
                colour_adjust(&mut rgba, saturation, self.look.contrast);
                rgba
            }
        };
        Ok(RawPage { bitmap: page, colour })
    }

    /// The whole pipeline for one part of a leaf: a page, or a split half of a spread, fitted
    /// inside `width` by `height` (the screen, or its share of a two-page leaf; swapped for a
    /// rotated spread, which the app turns).
    pub fn render(
        &mut self,
        page: &SourcePage,
        region: Option<PixelRect>,
        width: u32,
        height: u32,
    ) -> Result<RawPage, Error> {
        let scaled = self.scale(page, region, Fit::Within { width, height }, false)?;
        self.finish(scaled, page.colour)
    }

    fn filter(&mut self, width: u32, height: u32) -> &RainbowFilter {
        match self.filters.iter().position(|f| f.size() == (width, height)) {
            Some(i) => &self.filters[i],
            None => {
                if self.filters.len() == 2 {
                    self.filters.remove(0);
                }
                self.filters.push(RainbowFilter::new(width, height));
                self.filters.last().expect("just pushed")
            }
        }
    }
}

/// Saturation around each pixel's grey, then contrast around the middle grey, on RGBA.
fn colour_adjust(page: &mut Bitmap, saturation: f32, contrast: f32) {
    if saturation == 1.0 && contrast == 1.0 {
        return;
    }
    for p in page.data_mut().as_chunks_mut::<4>().0.iter_mut() {
        let y = 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
        for c in &mut p[..3] {
            let v = y + (*c as f32 - y) * saturation;
            let v = 127.5 + (v - 127.5) * contrast;
            *c = v.round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LCD: Target = Target { panel: Panel::Lcd, width: 1179, height: 2556 };

    #[test]
    fn lcd_sends_originals_unless_shrunk_past_twice() {
        assert_eq!(delivery(SourceFormat::Jpeg, (1600, 2133), &LCD), Delivery::Original);
        assert_eq!(delivery(SourceFormat::Jpeg, (2300, 3066), &LCD), Delivery::Original);
        assert_eq!(delivery(SourceFormat::Jpeg, (2400, 3200), &LCD), Delivery::Processed);
        assert_eq!(delivery(SourceFormat::Jpeg, (6200, 4650), &LCD), Delivery::Processed);
        assert_eq!(delivery(SourceFormat::Avif, (6200, 4650), &LCD), Delivery::Original);
        let grey = Target { panel: Panel::Grey, ..LCD };
        assert_eq!(delivery(SourceFormat::Jpeg, (800, 1000), &grey), Delivery::Processed);
    }

    #[test]
    fn saturation_moves_away_from_grey_and_leaves_grey_alone() {
        let mut b =
            Bitmap::new(2, 1, PixelFormat::Rgba8, vec![200, 100, 100, 255, 90, 90, 90, 255])
                .unwrap();
        colour_adjust(&mut b, 1.2, 1.0);
        let d = b.data();
        assert!(d[0] > 200 && d[1] < 100);
        assert_eq!(&d[4..8], &[90, 90, 90, 255]);
        assert_eq!(d[3], 255);
    }

    #[test]
    fn panel_defaults() {
        assert_eq!(Panel::Kaleido3.default_saturation(), 1.2);
        assert_eq!(Panel::Gallery3.default_saturation(), 1.0);
        assert!(!Panel::Grey.shows_colour() && Panel::UnknownColour.shows_colour());
        assert!(!Panel::Lcd.is_eink());
    }
}
