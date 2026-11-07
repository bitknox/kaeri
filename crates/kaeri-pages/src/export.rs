//! Pages prepared for a file sent to an e-reader: the same pipeline the reader runs, ending in
//! an image file instead of raw pixels.
//!
//! A page is decoded, checked for colour, and fitted inside the target's screen once. A spread
//! is split into its two halves, in reading order, since a one-page screen shows one half at a
//! time. Each result is set on a paper-white canvas of exactly the screen's size, so a fixed
//! layout book shows it undistorted. Grey screens get the 16 greys as a 4-bit PNG, which stays
//! small and is what such screens show; the rest get a JPEG.

use crate::layout::Direction;
use crate::render::Renderer;
use crate::{Bitmap, Error, PixelFormat, PixelRect, dither};

/// One page, or one half of a spread, as the file it goes into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPage {
    pub bytes: Vec<u8>,
    pub media_type: &'static str,
    /// The pixel size of the file: the target's screen.
    pub width: u32,
    pub height: u32,
}

/// JPEG quality for colour and non-e-ink grey pages.
const QUALITY: u8 = 85;

/// Prepares one source page for the renderer's target. Spreads come out as two pages, the half
/// that is read first first: the left with `LeftToRight`, the right with `RightToLeft`.
pub fn prepare(
    renderer: &mut Renderer,
    source: &[u8],
    direction: Direction,
) -> Result<Vec<ExportPage>, Error> {
    let page = renderer.open(source)?;
    let (screen_w, screen_h) = {
        let t = renderer.target();
        (t.width, t.height)
    };
    let regions: Vec<Option<PixelRect>> = if page.spread {
        let c = page.content;
        let left = c.width / 2;
        let a = PixelRect::new(c.x, c.y, left, c.height);
        let b = PixelRect::new(c.x + left, c.y, c.width - left, c.height);
        match direction {
            Direction::LeftToRight => vec![Some(a), Some(b)],
            Direction::RightToLeft => vec![Some(b), Some(a)],
        }
    } else {
        vec![None]
    };
    let mut out = Vec::with_capacity(regions.len());
    for region in regions {
        let raw = renderer.render(&page, region, screen_w, screen_h)?;
        let canvas = on_canvas(&raw.bitmap, screen_w, screen_h);
        out.push(encode(renderer, canvas));
    }
    out.into_iter().collect()
}

/// Sets `page` in the middle of a `width` by `height` canvas of white paper.
fn on_canvas(page: &Bitmap, width: u32, height: u32) -> Bitmap {
    if page.width() == width && page.height() == height {
        return page.clone();
    }
    let bpp = page.format().bytes_per_pixel();
    let mut canvas = Bitmap::filled(width, height, page.format(), 255);
    let (dx, dy) = ((width - page.width()) / 2, (height - page.height()) / 2);
    let stride = width as usize * bpp;
    for y in 0..page.height() {
        let start = (y + dy) as usize * stride + dx as usize * bpp;
        canvas.data_mut()[start..start + page.stride()].copy_from_slice(page.row(y));
    }
    canvas
}

fn encode(renderer: &Renderer, page: Bitmap) -> Result<ExportPage, Error> {
    let (width, height) = (page.width(), page.height());
    if page.format() == PixelFormat::Gray8 && renderer.target().panel == crate::Panel::Grey {
        let bytes = dither::png_grey4(&page)?;
        return Ok(ExportPage { bytes, media_type: "image/png", width, height });
    }
    let (data, colour) = match page.format() {
        PixelFormat::Gray8 => (page.into_data(), image::ExtendedColorType::L8),
        PixelFormat::Rgb8 => (page.into_data(), image::ExtendedColorType::Rgb8),
        PixelFormat::Rgba8 => {
            // JPEG has no alpha; the pipeline's pages are opaque.
            let rgb: Vec<u8> = page
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            (rgb, image::ExtendedColorType::Rgb8)
        }
    };
    let mut bytes = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, QUALITY);
    image::ImageEncoder::write_image(encoder, &data, width, height, colour)
        .map_err(|e| Error::Encode(e.to_string()))?;
    Ok(ExportPage { bytes, media_type: "image/jpeg", width, height })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Look, Panel, Target};
    use crate::testpages;

    fn renderer(panel: Panel) -> Renderer {
        Renderer::new(Target { panel, width: 120, height: 160 }, Look::default())
    }

    #[test]
    fn a_story_page_is_one_file_of_the_screens_size() {
        let src = testpages::jpeg(&testpages::manga_page(300, 420, 1), 90);
        let pages = prepare(&mut renderer(Panel::Lcd), &src, Direction::RightToLeft).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!((pages[0].width, pages[0].height), (120, 160));
        assert_eq!(pages[0].media_type, "image/jpeg");
        let info = crate::inspect(&pages[0].bytes).unwrap();
        assert_eq!((info.width, info.height), (120, 160));
    }

    #[test]
    fn a_spread_is_two_pages_in_reading_order() {
        let src = testpages::png(&testpages::manga_page(600, 400, 2));
        let ltr = prepare(&mut renderer(Panel::Lcd), &src, Direction::LeftToRight).unwrap();
        let rtl = prepare(&mut renderer(Panel::Lcd), &src, Direction::RightToLeft).unwrap();
        assert_eq!((ltr.len(), rtl.len()), (2, 2));
        assert_ne!(ltr[0].bytes, rtl[0].bytes);
        assert!(!ltr[0].bytes.is_empty());
    }

    #[test]
    fn grey_e_ink_gets_a_four_bit_png() {
        let src = testpages::png(&testpages::manga_page(300, 420, 3));
        let pages = prepare(&mut renderer(Panel::Grey), &src, Direction::LeftToRight).unwrap();
        assert_eq!(pages[0].media_type, "image/png");
        assert_eq!(&pages[0].bytes[1..4], b"PNG");
        assert_eq!(pages[0].bytes[24], 4, "bit depth");
    }

    #[test]
    fn colour_stays_colour_on_colour_screens() {
        let src = testpages::jpeg(&testpages::colour_page(300, 420, 4), 90);
        let pages =
            prepare(&mut renderer(Panel::Kaleido3), &src, Direction::LeftToRight).unwrap();
        let decoded = crate::decode(&pages[0].bytes, crate::DecodeLimits::default()).unwrap();
        assert_eq!(decoded.format(), PixelFormat::Rgb8);
    }

    #[test]
    fn something_that_is_not_an_image_is_refused() {
        assert!(prepare(&mut renderer(Panel::Lcd), b"nope", Direction::LeftToRight).is_err());
    }
}
