//! A page's fingerprint: a 64-bit difference hash (dHash) of a small greyscale copy, for
//! finding the same credit page again in another chapter.
//!
//! dHash rather than pHash. The pages it compares are the same file a group puts into each
//! release, re-encoded or resized by the source at most, never cropped, rotated or retouched,
//! which is where pHash's frequency domain pays. dHash reads the page's brightness gradient on
//! a 9 by 8 grid: a resize keeps the grid's proportions, a re-encode moves the cells' means by
//! a level or two, and neither turns a gradient around. It needs no DCT, so a page decoded at
//! an eighth of its size inside the JPEG decoder (as [`fingerprint`] asks) costs well under a
//! millisecond more.
//!
//! Two refinements make it hold on credit pages, which are mostly white paper:
//! - each cell is the mean of every pixel under it (an area average, not a sample), so text
//!   and JPEG noise average out instead of landing on one pixel or another;
//! - a pair of cells sets its bit only when the left is brighter by more than [`MARGIN`]
//!   levels, so two cells of the same white read 0 whatever noise a re-encode adds.
//!
//! A page with fewer than [`MIN_DETAIL`] such gradients either way (a blank page, a flat
//! colour) has no fingerprint: every such page would match every other.

use crate::{DecodeLimits, decode_for};

/// The Hamming distance at or under which two fingerprints are the same page. Tuned on the
/// tests' generated pages: a credit page re-encoded at JPEG quality 60 and resized by 7% stays
/// within 4, and story pages, or a credit page beside a story page, sit at 18 and over.
pub const SAME_PAGE: u32 = 8;

/// Levels (of 255) a cell must be brighter than its right-hand neighbour to set its bit.
const MARGIN: f32 = 3.0;

/// Gradients a page needs to be fingerprinted.
const MIN_DETAIL: u32 = 6;

const COLS: usize = 9;
const ROWS: usize = 8;

/// The fingerprint of a page's encoded bytes. `None` for bytes that do not decode, and for a
/// page too flat to tell apart.
pub fn fingerprint(bytes: &[u8]) -> Option<u64> {
    // Room this small lets a JPEG decode at an eighth of its size.
    let bitmap =
        decode_for(bytes, DecodeLimits::default(), Some((COLS as u32 * 4, 32))).ok()?;
    let (w, h) = (bitmap.width(), bitmap.height());
    if w < COLS as u32 || h < ROWS as u32 {
        return None;
    }
    let mut sums = [[0f32; COLS]; ROWS];
    let mut counts = [[0u32; COLS]; ROWS];
    for y in 0..h {
        let row = (y as usize * ROWS) / h as usize;
        for x in 0..w {
            let col = (x as usize * COLS) / w as usize;
            sums[row][col] += f32::from(bitmap.luma_at(x, y));
            counts[row][col] += 1;
        }
    }
    let mut print = 0u64;
    let mut detail = 0;
    for row in 0..ROWS {
        for col in 0..COLS - 1 {
            let mean = |c: usize| sums[row][c] / counts[row][c].max(1) as f32;
            let (left, right) = (mean(col), mean(col + 1));
            if (left - right).abs() > MARGIN {
                detail += 1;
            }
            if left > right + MARGIN {
                print |= 1 << (row * (COLS - 1) + col);
            }
        }
    }
    (detail >= MIN_DETAIL).then_some(print)
}

/// How many bits two fingerprints differ in.
pub fn distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Whether two fingerprints are the same page.
pub fn same_page(a: u64, b: u64) -> bool {
    distance(a, b) <= SAME_PAGE
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, ImageEncoder, Luma, codecs::jpeg::JpegEncoder, imageops};

    /// A credit page: white paper, a dark banner with a logo block, and lines of "text".
    fn credit_page(w: u32, h: u32) -> GrayImage {
        let mut page = GrayImage::from_pixel(w, h, Luma([250]));
        let fill = |page: &mut GrayImage, x0: u32, y0: u32, x1: u32, y1: u32, v: u8| {
            for y in y0..y1.min(h) {
                for x in x0..x1.min(w) {
                    page.put_pixel(x, y, Luma([v]));
                }
            }
        };
        fill(&mut page, 0, h / 10, w, h / 4, 30);
        fill(&mut page, w / 3, h / 8, w * 2 / 3, h * 2 / 9, 200);
        for line in 0..9 {
            let y = h / 3 + line * h / 16;
            let end = w / 6 + (line * 37 % 5 + 3) * w / 10;
            fill(&mut page, w / 8, y, end.min(w * 7 / 8), y + h / 40, 40);
        }
        page
    }

    /// A story page: panels of different greys with dark borders, laid out by `seed`.
    fn story_page(w: u32, h: u32, seed: u32) -> GrayImage {
        let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(12_345);
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        let mut page = GrayImage::from_pixel(w, h, Luma([245]));
        let rows = 2 + next() % 3;
        for r in 0..rows {
            let cols = 1 + next() % 3;
            for c in 0..cols {
                let (x0, x1) = (c * w / cols + 6, (c + 1) * w / cols - 6);
                let (y0, y1) = (r * h / rows + 6, (r + 1) * h / rows - 6);
                let tone = (next() % 200) as u8 + 20;
                let slope = (next() % 3) as i32 - 1;
                // A figure in the panel: a dark or light block somewhere inside it.
                let (pw, ph) = (x1 - x0, y1 - y0);
                let fx0 = x0 + next() % (pw / 2);
                let fy0 = y0 + next() % (ph / 2);
                let (fx1, fy1) =
                    (fx0 + pw / 4 + next() % (pw / 4), fy0 + ph / 4 + next() % (ph / 4));
                let figure = if next() % 2 == 0 { 15 } else { 235 };
                for y in y0..y1 {
                    for x in x0..x1 {
                        let edge = x < x0 + 4 || x + 4 > x1 || y < y0 + 4 || y + 4 > y1;
                        let inside = (fx0..fx1).contains(&x) && (fy0..fy1).contains(&y);
                        let shade = i32::from(tone) + slope * ((x - x0) as i32 * 60 / w as i32);
                        let v = if edge {
                            10
                        } else if inside {
                            figure
                        } else {
                            shade.clamp(0, 255) as u8
                        };
                        page.put_pixel(x, y, Luma([v]));
                    }
                }
            }
        }
        page
    }

    fn jpeg(page: &GrayImage, quality: u8) -> Vec<u8> {
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, quality)
            .write_image(page, page.width(), page.height(), image::ExtendedColorType::L8)
            .unwrap();
        out
    }

    fn png(page: &GrayImage) -> Vec<u8> {
        let mut out = Vec::new();
        image::codecs::png::PngEncoder::new(&mut out)
            .write_image(page, page.width(), page.height(), image::ExtendedColorType::L8)
            .unwrap();
        out
    }

    #[test]
    fn the_same_credit_page_re_encoded_and_resized_matches() {
        let original = credit_page(900, 1300);
        let first = fingerprint(&png(&original)).unwrap();
        let resized = imageops::resize(&original, 837, 1209, imageops::FilterType::Triangle);
        for bytes in [jpeg(&original, 92), jpeg(&original, 60), jpeg(&resized, 75)] {
            let again = fingerprint(&bytes).unwrap();
            assert!(distance(first, again) <= 4, "drifted {}", distance(first, again));
            assert!(same_page(first, again));
        }
    }

    #[test]
    fn different_story_pages_and_a_credit_page_do_not_match() {
        let credit = fingerprint(&jpeg(&credit_page(900, 1300), 85)).unwrap();
        let stories: Vec<u64> = (1..=12)
            .map(|seed| fingerprint(&jpeg(&story_page(900, 1300, seed), 85)).unwrap())
            .collect();
        for (i, a) in stories.iter().enumerate() {
            let d = distance(*a, credit);
            assert!(d > SAME_PAGE * 2, "story {i} {d} from the credit page");
            for b in &stories[i + 1..] {
                assert!(!same_page(*a, *b), "two story pages {}", distance(*a, *b));
            }
        }
    }

    #[test]
    fn a_blank_page_and_noise_have_no_fingerprint() {
        let blank = GrayImage::from_pixel(800, 1200, Luma([255]));
        assert_eq!(fingerprint(&jpeg(&blank, 80)), None);
        assert_eq!(fingerprint(b"not an image"), None);
    }
}
