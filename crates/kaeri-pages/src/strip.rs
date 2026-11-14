//! Strip cuts for e-ink: a vertical strip, already resized to the screen's width, cut into
//! screens. Each cut ends in a gap between panels when one lies near the screen's foot.
//! When none does, the cut ends at the foot and the next screen repeats the last
//! [`StripOptions::overlap`] rows (30 dp, converted by the caller), veiled in grey, so the eye
//! finds its place.
//!
//! A strip arrives as several images. [`gap_rows`] marks each image's flat rows, the caller
//! joins the marks in order, [`plan_cuts`] places the cuts, and [`extract`] copies one cut out
//! of the images.

use crate::{Bitmap, Error};

/// A row counts as a gap when its grey levels span this much or less.
pub const GAP_FLATNESS: u8 = 16;
/// A gap has at least this many flat rows; a single flat line inside art is not a gap.
pub const MIN_GAP: usize = 3;
/// How far the veil moves the overlap's greys towards its grey, out of 256.
const VEIL: u32 = 96;
const VEIL_GREY: u32 = 160;

/// For each row of `image`, whether it is flat enough to cut through.
pub fn gap_rows(image: &Bitmap) -> Vec<bool> {
    (0..image.height())
        .map(|y| {
            let (mut lo, mut hi) = (255u8, 0u8);
            for x in 0..image.width() {
                let v = image.luma_at(x, y);
                lo = lo.min(v);
                hi = hi.max(v);
            }
            hi.saturating_sub(lo) <= GAP_FLATNESS
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StripOptions {
    /// The screen's height in pixels: the most one cut holds.
    pub screen_height: u32,
    /// Rows repeated from the previous cut when no gap was found.
    pub overlap: u32,
    /// How far above the screen's foot a gap is looked for.
    pub search: u32,
}

impl StripOptions {
    /// The defaults for a screen: a gap is looked for in the lowest quarter.
    pub fn for_screen(screen_height: u32, overlap: u32) -> Self {
        StripOptions { screen_height, overlap, search: screen_height / 4 }
    }
}

/// One screen of the strip: rows `top..bottom`, of which the first `overlap` repeat the
/// previous cut's foot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StripCut {
    pub top: u32,
    pub bottom: u32,
    pub overlap: u32,
}

impl StripCut {
    pub fn height(&self) -> u32 {
        self.bottom - self.top
    }
}

/// Places the cuts down a strip whose rows are marked in `gaps`.
pub fn plan_cuts(gaps: &[bool], options: StripOptions) -> Vec<StripCut> {
    let total = gaps.len() as u32;
    let screen = options.screen_height.max(1);
    let overlap = options.overlap.min(screen / 2);
    let search = options.search.min(screen / 2);
    let mut cuts = Vec::new();
    let (mut top, mut repeat) = (0u32, 0u32);
    while top < total {
        let foot = top + screen;
        if foot >= total {
            cuts.push(StripCut { top, bottom: total, overlap: repeat });
            break;
        }
        match gap_near(gaps, foot, foot - search) {
            Some(cut) => {
                cuts.push(StripCut { top, bottom: cut, overlap: repeat });
                top = cut;
                repeat = 0;
            }
            None => {
                cuts.push(StripCut { top, bottom: foot, overlap: repeat });
                top = foot - overlap;
                repeat = overlap;
            }
        }
    }
    cuts
}

/// The lowest gap of at least [`MIN_GAP`] rows that reaches into `floor..foot`, cut in the
/// middle of the part of it above the foot.
fn gap_near(gaps: &[bool], foot: u32, floor: u32) -> Option<u32> {
    let flat = |y: u32| gaps.get(y as usize).copied().unwrap_or(false);
    let mut y = foot;
    while y > floor {
        y -= 1;
        if !flat(y) {
            continue;
        }
        // The run through y: its end below (possibly past the foot) and its start above.
        let mut end = y + 1;
        while flat(end) && end < foot + MIN_GAP as u32 {
            end += 1;
        }
        let mut start = y;
        while start > 0 && flat(start - 1) {
            start -= 1;
        }
        if (end - start) as usize >= MIN_GAP {
            let start = start.max(floor);
            let cut = (start + y + 1).div_ceil(2);
            return Some(cut.max(floor + 1));
        }
        y = start;
    }
    None
}

/// Copies rows `cut.top..cut.bottom` out of `images`, stacked in order, and veils the
/// overlap. Every image must have the same width and format.
pub fn extract(images: &[&Bitmap], cut: StripCut) -> Result<Bitmap, Error> {
    let first = images.first().ok_or(Error::EmptyStrip)?;
    let (width, format) = (first.width(), first.format());
    if let Some(odd) = images.iter().find(|i| i.width() != width || i.format() != format) {
        return Err(Error::SizeMismatch {
            want: (width, odd.height()),
            got: (odd.width(), odd.height()),
        });
    }
    let stride = first.stride();
    let mut data = Vec::with_capacity(stride * cut.height() as usize);
    let mut offset = 0u32;
    for image in images {
        let (start, end) = (offset, offset + image.height());
        let (a, b) = (cut.top.max(start), cut.bottom.min(end));
        if a < b {
            let rows =
                &image.data()[(a - start) as usize * stride..(b - start) as usize * stride];
            data.extend_from_slice(rows);
        }
        offset = end;
    }
    let mut out = Bitmap::new(width, cut.height(), format, data)
        .map_err(|_| Error::Region(crate::PixelRect::new(0, cut.top, width, cut.height())))?;
    veil(&mut out, cut.overlap);
    Ok(out)
}

/// Moves the top `rows` rows' values part of the way to a light grey, keeping alpha.
fn veil(page: &mut Bitmap, rows: u32) {
    let bpp = page.format().bytes_per_pixel();
    let n = rows.min(page.height()) as usize * page.stride();
    for (i, v) in page.data_mut()[..n].iter_mut().enumerate() {
        if bpp == 4 && i % 4 == 3 {
            continue;
        }
        *v = ((*v as u32 * (256 - VEIL) + VEIL_GREY * VEIL + 128) >> 8) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;

    /// Rows marked flat: each gap is a start and an end, the end left out.
    fn marks(len: usize, gaps: &[(usize, usize)]) -> Vec<bool> {
        (0..len).map(|y| gaps.iter().any(|&(a, b)| (a..b).contains(&y))).collect()
    }

    #[test]
    fn flat_rows_are_gaps() {
        let mut img = Bitmap::filled(10, 3, PixelFormat::Gray8, 250);
        img.data_mut()[10 + 4] = 0;
        assert_eq!(gap_rows(&img), vec![true, false, true]);
    }

    #[test]
    fn cuts_land_in_gaps_near_the_foot() {
        // Screens of 100 rows; gaps at 85..95 and 170..180; 250 rows in all.
        let gaps = marks(250, &[(85, 95), (170, 180)]);
        let cuts = plan_cuts(&gaps, StripOptions::for_screen(100, 10));
        assert_eq!(cuts.len(), 3);
        assert_eq!(cuts[0], StripCut { top: 0, bottom: 90, overlap: 0 });
        assert_eq!(cuts[1], StripCut { top: 90, bottom: 175, overlap: 0 });
        assert_eq!(cuts[2], StripCut { top: 175, bottom: 250, overlap: 0 });
    }

    #[test]
    fn a_gap_through_the_foot_cuts_above_it() {
        let gaps = marks(300, &[(95, 120)]);
        let cuts = plan_cuts(&gaps, StripOptions::for_screen(100, 10));
        assert!(cuts[0].bottom > 95 && cuts[0].bottom <= 100, "{:?}", cuts[0]);
    }

    #[test]
    fn without_gaps_cuts_overlap() {
        let gaps = marks(250, &[(10, 12)]); // too thin, and too high
        let cuts = plan_cuts(&gaps, StripOptions::for_screen(100, 10));
        assert_eq!(cuts[0], StripCut { top: 0, bottom: 100, overlap: 0 });
        assert_eq!(cuts[1], StripCut { top: 90, bottom: 190, overlap: 10 });
        assert_eq!(cuts[2], StripCut { top: 180, bottom: 250, overlap: 10 });
        // Every row is shown, and cuts never run past a screen.
        assert!(cuts.iter().all(|c| c.height() <= 100));
        assert!(cuts.windows(2).all(|w| w[1].top <= w[0].bottom));
    }

    #[test]
    fn an_empty_strip_has_no_cuts() {
        assert!(plan_cuts(&[], StripOptions::for_screen(100, 10)).is_empty());
    }

    #[test]
    fn extract_spans_images_and_veils_the_overlap() {
        let a = Bitmap::filled(4, 5, PixelFormat::Gray8, 0);
        let b = Bitmap::filled(4, 5, PixelFormat::Gray8, 255);
        let out = extract(&[&a, &b], StripCut { top: 3, bottom: 7, overlap: 1 }).unwrap();
        assert_eq!(out.height(), 4);
        let col: Vec<u8> = (0..4).map(|y| out.luma_at(0, y)).collect();
        assert_eq!(col[1..], [0, 255, 255]);
        assert!(col[0] > 40 && col[0] < 80, "veiled black: {}", col[0]);

        let narrow = Bitmap::filled(3, 5, PixelFormat::Gray8, 0);
        assert!(extract(&[&a, &narrow], StripCut { top: 0, bottom: 6, overlap: 0 }).is_err());
    }
}
