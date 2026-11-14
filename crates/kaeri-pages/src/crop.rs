//! Crop borders: the blank margins a scan carries, trimmed to at most a tenth of each side.
//!
//! A side's border is the run of lines, from the edge inwards, that are all near white or all
//! near black, allowing a few specks. Spreads are never cropped (the caller leaves them out),
//! since their halves have to meet at the gutter.

use crate::{Bitmap, PixelRect};

/// The most cropped from any one side, as a share of that dimension.
pub const MAX_CROP: f32 = 0.10;

/// A line counts as blank when this share of its pixels or more sits near its border colour.
const BLANK_SHARE: f32 = 0.995;
/// Near white is this bright or brighter, near black this dark or darker.
const NEAR_WHITE: u8 = 225;
const NEAR_BLACK: u8 = 30;

/// The content box: the page with blank borders removed, never more than [`MAX_CROP`] a side.
pub fn content_box(page: &Bitmap) -> PixelRect {
    let (w, h) = (page.width(), page.height());
    if w < 4 || h < 4 {
        return page.bounds();
    }
    let max_x = (w as f32 * MAX_CROP) as u32;
    let max_y = (h as f32 * MAX_CROP) as u32;

    let row = |y: u32| (0..w).map(move |x| page.luma_at(x, y));
    let col = |x: u32| (0..h).map(move |y| page.luma_at(x, y));

    let top = blank_run((0..max_y).map(row));
    let bottom = blank_run((0..max_y).map(|i| row(h - 1 - i)));
    let left = blank_run((0..max_x).map(col));
    let right = blank_run((0..max_x).map(|i| col(w - 1 - i)));

    PixelRect::new(left, top, w - left - right, h - top - bottom)
}

/// How many lines, from the edge, are blank in the same border colour.
fn blank_run<L: Iterator<Item = u8>>(lines: impl Iterator<Item = L>) -> u32 {
    let mut border: Option<bool> = None; // true: white, false: black
    let mut run = 0;
    for line in lines {
        let (mut white, mut black, mut n) = (0u32, 0u32, 0u32);
        for v in line {
            white += (v >= NEAR_WHITE) as u32;
            black += (v <= NEAR_BLACK) as u32;
            n += 1;
        }
        let need = (n as f32 * BLANK_SHARE).ceil() as u32;
        let this = if white >= need {
            Some(true)
        } else if black >= need {
            Some(false)
        } else {
            None
        };
        match (border, this) {
            (_, None) => break,
            (None, Some(c)) => border = Some(c),
            (Some(b), Some(c)) if b != c => break,
            _ => {}
        }
        run += 1;
    }
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;

    /// A `w` by `h` page of mid grey art inside a border of `l, t, r, b` lines of `paper`.
    fn framed(w: u32, h: u32, (l, t, r, b): (u32, u32, u32, u32), paper: u8) -> Bitmap {
        let data = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| {
                let inside = x >= l && x < w - r && y >= t && y < h - b;
                if inside { ((x * 7 + y * 3) % 200) as u8 + 28 } else { paper }
            })
            .collect();
        Bitmap::new(w, h, PixelFormat::Gray8, data).unwrap()
    }

    #[test]
    fn white_margins_are_trimmed() {
        let page = framed(200, 300, (5, 12, 9, 3), 250);
        assert_eq!(content_box(&page), PixelRect::new(5, 12, 186, 285));
    }

    #[test]
    fn black_margins_are_trimmed() {
        let page = framed(200, 300, (6, 6, 6, 6), 8);
        assert_eq!(content_box(&page), PixelRect::new(6, 6, 188, 288));
    }

    #[test]
    fn crop_stops_at_a_tenth_of_each_side() {
        let page = framed(200, 300, (60, 60, 0, 0), 255);
        assert_eq!(content_box(&page), PixelRect::new(20, 30, 180, 270));
    }

    #[test]
    fn a_speck_does_not_stop_the_crop() {
        let mut page = framed(400, 400, (0, 10, 0, 0), 255);
        page.data_mut()[2 * 400 + 100] = 0; // one dark pixel in the third row
        assert_eq!(content_box(&page).y, 10);
    }

    #[test]
    fn art_to_the_edge_is_left_alone() {
        let page = framed(100, 100, (0, 0, 0, 0), 255);
        assert_eq!(content_box(&page), page.bounds());
    }
}
