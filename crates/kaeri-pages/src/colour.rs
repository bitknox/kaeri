//! Whether a page has colour, judged in YCbCr.
//!
//! A black and white scan is rarely neutral: paper yellows, and a scanner tints. Such a page is
//! close to one colour, the paper's, scaled by how dark each pixel is. The check estimates the
//! paper's tint from the bright pixels and counts the pixels whose chroma is clearly away from
//! that tint at their brightness. A page with more than a small share of those is a colour
//! page. The share is low on purpose: a colour title on an otherwise grey page keeps the page
//! in colour, since showing a grey page in colour costs nothing and the other way loses the
//! colour.

use crate::{Bitmap, PixelFormat};

/// How far (on the 8-bit Cb and Cr scales) a pixel's chroma must sit from the paper's tint at
/// its brightness to count as coloured. JPEG noise on a grey scan stays well under it.
pub const CHROMA_THRESHOLD: f32 = 12.0;

/// The share of pixels that must count as coloured for the page to be a colour page.
pub const COLOUR_SHARE: f32 = 0.005;

/// What the check found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColourCheck {
    pub colour: bool,
    /// The share of sampled pixels away from the paper's tint, 0 to 1.
    pub share: f32,
}

impl ColourCheck {
    pub const GREY: ColourCheck = ColourCheck { colour: false, share: 0.0 };
}

/// Checks a page. Grey bitmaps have no colour by construction; colour ones are sampled on
/// every second row, which halves the cost and changes no verdict that matters.
pub fn check_colour(page: &Bitmap) -> ColourCheck {
    let bpp = page.format().bytes_per_pixel();
    if page.format() == PixelFormat::Gray8 || page.width() == 0 || page.height() == 0 {
        return ColourCheck::GREY;
    }
    let ycc = |p: &[u8]| {
        let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
        let y = 0.299 * r + 0.587 * g + 0.114 * b;
        let cb = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
        let cr = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
        (y, cb, cr)
    };

    // The paper: the mean of the bright pixels, on every eighth row.
    let (mut sy, mut scb, mut scr, mut n) = (0f64, 0f64, 0f64, 0u64);
    for y in (0..page.height()).step_by(8) {
        for p in page.row(y).chunks_exact(bpp) {
            let (l, cb, cr) = ycc(p);
            if l >= 128.0 {
                sy += l as f64;
                scb += cb as f64;
                scr += cr as f64;
                n += 1;
            }
        }
    }
    let (paper_y, paper_cb, paper_cr) = if n == 0 {
        (255.0, 0.0, 0.0)
    } else {
        let n = n as f64;
        ((sy / n) as f32, (scb / n) as f32, (scr / n) as f32)
    };

    let (mut coloured, mut sampled) = (0u64, 0u64);
    for y in (0..page.height()).step_by(2) {
        for p in page.row(y).chunks_exact(bpp) {
            let (l, cb, cr) = ycc(p);
            let k = (l / paper_y).min(1.0);
            let dcb = (cb - paper_cb * k).abs();
            let dcr = (cr - paper_cr * k).abs();
            coloured += (dcb.max(dcr) > CHROMA_THRESHOLD) as u64;
            sampled += 1;
        }
    }
    let share = coloured as f32 / sampled as f32;
    ColourCheck { colour: share > COLOUR_SHARE, share }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Bitmap {
        let data = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| f(x, y))
            .collect();
        Bitmap::new(width, height, PixelFormat::Rgb8, data).unwrap()
    }

    /// Ink and screentone over a paper colour: every pixel a fraction of the paper.
    fn on_paper(paper: [u8; 3]) -> Bitmap {
        rgb(200, 200, move |x, y| {
            let k = match (x / 10 + y / 10) % 4 {
                0 => 0.05,
                1 => 0.5,
                _ => 1.0,
            };
            paper.map(|c| (c as f32 * k) as u8)
        })
    }

    #[test]
    fn a_grey_bitmap_is_grey() {
        let b = Bitmap::filled(4, 4, PixelFormat::Gray8, 128);
        assert_eq!(check_colour(&b), ColourCheck::GREY);
    }

    #[test]
    fn neutral_and_yellowed_scans_are_grey() {
        assert!(!check_colour(&on_paper([255, 255, 255])).colour);
        assert!(!check_colour(&on_paper([240, 228, 196])).colour, "yellowed paper");
        assert!(!check_colour(&on_paper([226, 236, 246])).colour, "a cold scanner tint");
    }

    #[test]
    fn jpeg_like_noise_on_grey_stays_grey() {
        let page = rgb(200, 200, |x, y| {
            let v = if (x + y).is_multiple_of(7) { 30u8 } else { 230 };
            let n = ((x * 31 + y * 17) % 9) as u8; // 0 to 8
            [v.saturating_add(n), v, v.saturating_sub(n / 2)]
        });
        assert!(!check_colour(&page).colour);
    }

    #[test]
    fn a_small_coloured_title_makes_a_colour_page() {
        // 2% of the page in red, the rest neutral.
        let page =
            rgb(
                200,
                200,
                |x, y| if y < 4 { [200, 30, 30] } else { [x as u8, x as u8, x as u8] },
            );
        let check = check_colour(&page);
        assert!(check.colour, "{check:?}");
        assert!(check.share > 0.015 && check.share < 0.03);
    }

    #[test]
    fn a_full_colour_page_is_colour() {
        let page = rgb(100, 100, |x, y| [(x * 2) as u8, (y * 2) as u8, 128]);
        assert!(check_colour(&page).colour);
    }
}
