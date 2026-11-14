//! Resampling, dithering and the rainbow filter against a generated zone plate: rings whose
//! frequency grows from the centre outwards, so every frequency and direction is on the page
//! and aliasing shows up as false rings.

use kaeri_pages::rainbow::RainbowFilter;
use kaeri_pages::resize::resize;
use kaeri_pages::testpages::{zone_plate, zone_plate_frequency};
use kaeri_pages::{Bitmap, PixelFormat, dither};

/// The standard deviation of the output pixels whose source frequency lies in `band` (cycles
/// a source pixel) and whose direction from the centre passes `keep`.
fn spread_in_band(
    out: &Bitmap,
    scale: f32,
    (sw, sh, fmax): (u32, u32, f32),
    band: (f32, f32),
    keep: impl Fn(f32, f32) -> bool,
) -> f32 {
    let mut vals = Vec::new();
    for y in 0..out.height() {
        for x in 0..out.width() {
            let (sx, sy) = ((x as f32 + 0.5) * scale, (y as f32 + 0.5) * scale);
            let f = zone_plate_frequency(sw, sh, fmax, sx, sy);
            if f >= band.0 && f <= band.1 && keep(sx - sw as f32 / 2.0, sy - sh as f32 / 2.0) {
                vals.push(out.luma_at(x, y) as f32);
            }
        }
    }
    assert!(vals.len() > 100, "too few samples in the band: {}", vals.len());
    let mean = vals.iter().sum::<f32>() / vals.len() as f32;
    (vals.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / vals.len() as f32).sqrt()
}

/// Point sampling, the platform subsampling the engine resizes to avoid.
fn nearest(src: &Bitmap, step: u32) -> Bitmap {
    let (w, h) = (src.width() / step, src.height() / step);
    let data = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| src.luma_at(x * step + step / 2, y * step + step / 2))
        .collect();
    Bitmap::new(w, h, PixelFormat::Gray8, data).unwrap()
}

const ANY: fn(f32, f32) -> bool = |_, _| true;

#[test]
fn shrinking_past_twice_averages_screentone_away_instead_of_aliasing() {
    let (sw, sh, fmax) = (1200, 1200, 0.5);
    let plate = zone_plate(sw, sh, fmax);
    let ours = resize(&plate, plate.bounds(), 300, 300).unwrap();
    let points = nearest(&plate, 4);
    // Past the output's Nyquist (0.125 cycles a source pixel) by half again and more.
    let band = (0.2, 0.5);
    let ours_spread = spread_in_band(&ours, 4.0, (sw, sh, fmax), band, ANY);
    let point_spread = spread_in_band(&points, 4.0, (sw, sh, fmax), band, ANY);
    assert!(point_spread > 60.0, "point sampling should alias: {point_spread}");
    assert!(ours_spread < point_spread / 4.0, "ours {ours_spread} against {point_spread}");
}

#[test]
fn shrinking_past_twice_keeps_coarse_rings() {
    let (sw, sh, fmax) = (1200, 1200, 0.5);
    let plate = zone_plate(sw, sh, fmax);
    let ours = resize(&plate, plate.bounds(), 300, 300).unwrap();
    // Well under the output's Nyquist the rings keep most of their swing.
    let coarse = spread_in_band(&ours, 4.0, (sw, sh, fmax), (0.0, 0.04), ANY);
    let source = spread_in_band(&plate, 1.0, (sw, sh, fmax), (0.0, 0.04), ANY);
    assert!(coarse > source * 0.8, "coarse rings {coarse} against {source}");
}

#[test]
fn lanczos_shrink_suppresses_frequencies_it_cannot_show() {
    let (sw, sh, fmax) = (900, 900, 0.5);
    let plate = zone_plate(sw, sh, fmax);
    let ours = resize(&plate, plate.bounds(), 600, 600).unwrap(); // 1.5x: Lanczos3
    let points = nearest(&zone_plate(sw * 2, sh * 2, fmax), 3);
    let band = (0.4, 0.5); // the output's Nyquist is 0.33 a source pixel
    let ours_spread = spread_in_band(&ours, 1.5, (sw, sh, fmax), band, ANY);
    let point_spread = spread_in_band(&points, 3.0, (sw * 2, sh * 2, fmax), band, ANY);
    assert!(ours_spread < point_spread / 3.0, "ours {ours_spread} against {point_spread}");
    // And what it can show passes.
    let low = spread_in_band(&ours, 1.5, (sw, sh, fmax), (0.0, 0.15), ANY);
    assert!(low > 80.0, "low rings {low}");
}

#[test]
fn ordered_dither_keeps_the_tone_of_every_ring() {
    let mut plate = zone_plate(512, 512, 0.06); // rings wide enough to compare in 8x8 blocks
    let before = plate.clone();
    dither::ordered(&mut plate).unwrap();
    assert!(plate.data().iter().all(|v| v % 17 == 0));
    let mean = |b: &Bitmap, bx: u32, by: u32| {
        let mut s = 0u32;
        for y in by..by + 8 {
            for x in bx..bx + 8 {
                s += b.luma_at(x, y) as u32;
            }
        }
        s as f32 / 64.0
    };
    let mut worst = 0f32;
    for by in (0..512).step_by(8) {
        for bx in (0..512).step_by(8) {
            worst = worst.max((mean(&before, bx, by) - mean(&plate, bx, by)).abs());
        }
    }
    // Half a grey step, plus the ring's own slope across a block.
    assert!(worst < 12.0, "worst block off by {worst}");
}

#[test]
fn the_rainbow_filter_damps_fine_diagonals_and_nothing_else() {
    let (w, h, fmax) = (640, 640, 0.5);
    let mut plate = zone_plate(w, h, fmax);
    let before = plate.clone();
    RainbowFilter::new(w, h).apply(&mut plate, 2).unwrap();

    // Directions from the centre are the rings' frequency directions.
    let near_diagonal = |dx: f32, dy: f32| {
        let a = dy.abs().atan2(dx.abs()).to_degrees();
        (a - 45.0).abs() < 6.0
    };
    let near_axis = |dx: f32, dy: f32| {
        let a = dy.abs().atan2(dx.abs()).to_degrees();
        !(15.0..=75.0).contains(&a)
    };
    let fine = (0.36, 0.48);
    let diag_before = spread_in_band(&before, 1.0, (w, h, fmax), fine, near_diagonal);
    let diag_after = spread_in_band(&plate, 1.0, (w, h, fmax), fine, near_diagonal);
    assert!(diag_after < diag_before * 0.25, "diagonal {diag_before} to {diag_after}");

    let axis_before = spread_in_band(&before, 1.0, (w, h, fmax), fine, near_axis);
    let axis_after = spread_in_band(&plate, 1.0, (w, h, fmax), fine, near_axis);
    assert!(axis_after > axis_before * 0.85, "axis {axis_before} to {axis_after}");

    let coarse_before = spread_in_band(&before, 1.0, (w, h, fmax), (0.0, 0.2), ANY);
    let coarse_after = spread_in_band(&plate, 1.0, (w, h, fmax), (0.0, 0.2), ANY);
    assert!((coarse_after - coarse_before).abs() < coarse_before * 0.05);
}
