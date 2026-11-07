//! Dithering to the 16 greys an e-ink panel shows, and the 4-bit PNG it is exported as.
//!
//! On a grey panel the reader uses an 8x8 ordered dither: it is fast, it is the same for the
//! same page every time, and its fixed pattern does not crawl when a page is redrawn.
//! Floyd-Steinberg is kept for 4-bit PNG export, where its finer result is worth its time
//! (S5: 13 ms against 1 ms on the Mac for a 1264x1680 page).

use crate::{Bitmap, Error, PixelFormat};

/// The greys a panel shows: 0, 17, 34, up to 255.
pub const LEVELS: u8 = 16;
const STEP: u32 = 255 / (LEVELS as u32 - 1);

const BAYER8: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

fn require_gray(page: &Bitmap) -> Result<(), Error> {
    match page.format() {
        PixelFormat::Gray8 => Ok(()),
        other => Err(Error::Format { want: PixelFormat::Gray8, got: other }),
    }
}

/// Ordered 8x8 dither of a grey page to [`LEVELS`] greys, in place. A grey already on a level
/// stays flat.
pub fn ordered(page: &mut Bitmap) -> Result<(), Error> {
    require_gray(page)?;
    let w = page.width() as usize;
    if w == 0 {
        return Ok(());
    }
    for (y, row) in page.data_mut().chunks_exact_mut(w).enumerate() {
        let bayer = &BAYER8[y & 7];
        for (x, v) in row.iter_mut().enumerate() {
            // The level in 64ths of a grey step, plus the threshold.
            let l = (*v as u32 * (LEVELS as u32 - 1) * 64 + 127) / 255 + bayer[x & 7] as u32;
            *v = ((l / 64).min(LEVELS as u32 - 1) * STEP) as u8;
        }
    }
    Ok(())
}

/// Floyd-Steinberg error diffusion of a grey page to [`LEVELS`] greys, in place.
pub fn floyd_steinberg(page: &mut Bitmap) -> Result<(), Error> {
    require_gray(page)?;
    let (w, h) = (page.width() as usize, page.height() as usize);
    // Errors in sixteenths, one row ahead, with a pixel of margin on each side.
    let mut cur = vec![0i32; w + 2];
    let mut next = vec![0i32; w + 2];
    let data = page.data_mut();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let v = (data[i] as i32 * 16 + cur[x + 1]).clamp(0, 255 * 16);
            let q = ((v + STEP as i32 * 8) / (STEP as i32 * 16)) * STEP as i32;
            data[i] = q as u8;
            let e = v - q * 16;
            cur[x + 2] += e * 7 / 16;
            next[x] += e * 3 / 16;
            next[x + 1] += e * 5 / 16;
            next[x + 2] += e / 16;
        }
        std::mem::swap(&mut cur, &mut next);
        next.fill(0);
    }
    Ok(())
}

/// Encodes a page already on the [`LEVELS`] greys as a 4-bit grey PNG, for export to an
/// e-reader. Values between levels are rounded to the nearest.
pub fn png_grey4(page: &Bitmap) -> Result<Vec<u8>, Error> {
    require_gray(page)?;
    let (w, h) = (page.width() as usize, page.height() as usize);
    let row_bytes = w.div_ceil(2);
    let mut packed = vec![0u8; row_bytes * h];
    for (src, dst) in
        page.data().chunks_exact(w.max(1)).zip(packed.chunks_exact_mut(row_bytes.max(1)))
    {
        for (x, &v) in src.iter().enumerate() {
            let nibble = ((v as u32 + STEP / 2) / STEP) as u8;
            dst[x / 2] |= if x.is_multiple_of(2) { nibble << 4 } else { nibble };
        }
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, page.width(), page.height());
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Four);
    let mut writer = encoder.write_header().map_err(|e| Error::Encode(e.to_string()))?;
    writer.write_image_data(&packed).map_err(|e| Error::Encode(e.to_string()))?;
    writer.finish().map_err(|e| Error::Encode(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> Bitmap {
        let data =
            (0..h).flat_map(|_| (0..w).map(move |x| (x * 255 / (w - 1)) as u8)).collect();
        Bitmap::new(w, h, PixelFormat::Gray8, data).unwrap()
    }

    fn on_levels(page: &Bitmap) -> bool {
        page.data().iter().all(|&v| (v as u32).is_multiple_of(STEP))
    }

    /// The mean of each 8x8 block, before and after, agree to within half a grey step.
    fn keeps_local_means(before: &Bitmap, after: &Bitmap) {
        let w = before.width() as usize;
        for by in (0..before.height() as usize).step_by(8) {
            for bx in (0..w).step_by(8) {
                let mean = |b: &Bitmap| {
                    let mut s = 0u32;
                    for y in by..by + 8 {
                        for x in bx..bx + 8 {
                            s += b.data()[y * w + x] as u32;
                        }
                    }
                    s as f32 / 64.0
                };
                let d = (mean(before) - mean(after)).abs();
                assert!(d <= STEP as f32 / 2.0 + 1.0, "block {bx},{by}: {d}");
            }
        }
    }

    #[test]
    fn ordered_lands_on_levels_and_keeps_the_tone() {
        let before = gradient(256, 64);
        let mut after = before.clone();
        ordered(&mut after).unwrap();
        assert!(on_levels(&after));
        keeps_local_means(&before, &after);
        assert_eq!(after.data()[0], 0);
        assert_eq!(after.data()[255], 255);
    }

    #[test]
    fn ordered_leaves_exact_levels_flat() {
        let mut flat = Bitmap::filled(16, 16, PixelFormat::Gray8, 8 * 17);
        ordered(&mut flat).unwrap();
        assert!(flat.data().iter().all(|&v| v == 136));
    }

    #[test]
    fn floyd_steinberg_lands_on_levels_and_keeps_the_tone() {
        let before = gradient(256, 64);
        let mut after = before.clone();
        floyd_steinberg(&mut after).unwrap();
        assert!(on_levels(&after));
        // Error diffusion moves tone between neighbouring blocks, so compare whole rows.
        for y in 0..64 {
            let sum =
                |b: &Bitmap| b.row(y).iter().map(|&v| v as u32).sum::<u32>() as f32 / 256.0;
            assert!((sum(&before) - sum(&after)).abs() < 2.0);
        }
    }

    #[test]
    fn colour_bitmaps_are_refused() {
        let mut rgb = Bitmap::filled(2, 2, PixelFormat::Rgb8, 0);
        assert!(matches!(ordered(&mut rgb), Err(Error::Format { .. })));
    }

    #[test]
    fn a_4_bit_png_round_trips() {
        let mut page = gradient(33, 5); // an odd width packs half a byte at each row's end
        ordered(&mut page).unwrap();
        let png = png_grey4(&page).unwrap();
        let back = image::load_from_memory(&png).unwrap().into_luma8();
        assert_eq!(back.dimensions(), (33, 5));
        assert_eq!(back.as_raw(), page.data());
    }
}
