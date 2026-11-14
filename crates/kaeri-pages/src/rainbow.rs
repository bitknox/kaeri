//! The rainbow filter for colour e-ink: fine diagonal screentone beats against a Kaleido
//! panel's colour filter and shows as rainbow fringes. The filter damps exactly those
//! frequencies, as KCC's eraser does: a 2D real FFT of the page, every frequency at least
//! [`RADIUS`] cycles a pixel out and within [`ANGLE`] degrees of either diagonal scaled to
//! [`DAMPING`], then the inverse FFT. Everything else passes untouched.
//!
//! It runs on black and white pages on Kaleido screens only, off by default and offered in
//! the first reader settings sheet there. Planning the FFT and building the mask cost a few
//! milliseconds, so a [`RainbowFilter`] is made once per page size and kept. The FFT is padded
//! to a size of the form 2^a x 3^b x 5^c with c at most 1, which S5 measured up to twice as
//! fast as exact panel sizes such as 1264 (16 x 79). About 0.3 s a page on an A55-class core,
//! so it belongs in the prefetch worker, on two threads at most.

use std::sync::Arc;

use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use crate::{Bitmap, Error, PixelFormat};

/// Frequencies at least this far out, in cycles a pixel, can be damped.
pub const RADIUS: f32 = 0.30;
/// How close to a diagonal, in degrees, a damped frequency lies.
pub const ANGLE: f32 = 10.0;
/// What a damped frequency is scaled to.
pub const DAMPING: f32 = 0.10;

/// The FFT length used for a side of `n` pixels: the next length at or above `n` whose prime
/// factors are 2 and 3, with at most one 5.
pub fn fft_len(n: usize) -> usize {
    let mut m = n.max(1);
    loop {
        let mut r = m;
        while r.is_multiple_of(2) {
            r /= 2;
        }
        while r.is_multiple_of(3) {
            r /= 3;
        }
        if r.is_multiple_of(5) {
            r /= 5;
        }
        if r == 1 {
            return m;
        }
        m += 1;
    }
}

/// The filter for one page size, with its FFT plans and mask.
pub struct RainbowFilter {
    width: usize,
    height: usize,
    pw: usize,
    ph: usize,
    /// Complex columns of a real row transform: pw / 2 + 1.
    cw: usize,
    r2c: Arc<dyn RealToComplex<f32>>,
    c2r: Arc<dyn ComplexToReal<f32>>,
    forward: Arc<dyn Fft<f32>>,
    inverse: Arc<dyn Fft<f32>>,
    /// Per frequency, column major (one column of `ph` per real-transform column).
    mask: Vec<f32>,
}

impl std::fmt::Debug for RainbowFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RainbowFilter")
            .field("size", &(self.width, self.height))
            .field("fft", &(self.pw, self.ph))
            .finish_non_exhaustive()
    }
}

impl RainbowFilter {
    pub fn new(width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1) as usize, height.max(1) as usize);
        let (pw, ph) = (fft_len(width), fft_len(height));
        let cw = pw / 2 + 1;
        let mut real = RealFftPlanner::<f32>::new();
        let mut complex = FftPlanner::<f32>::new();
        let mut mask = vec![1f32; cw * ph];
        let tan_lo = (45.0 - ANGLE).to_radians().tan();
        let tan_hi = (45.0 + ANGLE).to_radians().tan();
        for kx in 0..cw {
            let fx = kx as f32 / pw as f32;
            for ky in 0..ph {
                let fy =
                    if ky <= ph / 2 { ky as f32 } else { ky as f32 - ph as f32 } / ph as f32;
                let r2 = fx * fx + fy * fy;
                // Within ANGLE of a diagonal: |fy| / fx between tan(35) and tan(55).
                let diagonal = fx > 0.0 && (tan_lo..=tan_hi).contains(&(fy.abs() / fx));
                if r2 >= RADIUS * RADIUS && diagonal {
                    mask[kx * ph + ky] = DAMPING;
                }
            }
        }
        RainbowFilter {
            width,
            height,
            pw,
            ph,
            cw,
            r2c: real.plan_fft_forward(pw),
            c2r: real.plan_fft_inverse(pw),
            forward: complex.plan_fft_forward(ph),
            inverse: complex.plan_fft_inverse(ph),
            mask,
        }
    }

    /// The page size this filter is for.
    pub fn size(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    /// The padded FFT size.
    pub fn fft_size(&self) -> (u32, u32) {
        (self.pw as u32, self.ph as u32)
    }

    /// Bytes of working memory one [`apply`](Self::apply) allocates.
    pub fn working_bytes(&self) -> usize {
        self.pw * self.ph * 4 + 2 * self.cw * self.ph * 8
    }

    /// Filters a grey page of this filter's size in place, on up to `threads` threads.
    pub fn apply(&self, page: &mut Bitmap, threads: usize) -> Result<(), Error> {
        if page.format() != PixelFormat::Gray8 {
            return Err(Error::Format { want: PixelFormat::Gray8, got: page.format() });
        }
        if (page.width() as usize, page.height() as usize) != (self.width, self.height) {
            return Err(Error::SizeMismatch {
                want: self.size(),
                got: (page.width(), page.height()),
            });
        }
        let (w, h, pw, ph, cw) = (self.width, self.height, self.pw, self.ph, self.cw);
        let threads = threads.clamp(1, 16);

        // Mirror the page out to the FFT size, so the padding adds no edge.
        let mut real = vec![0f32; pw * ph];
        let src = page.data();
        for (y, row) in real.chunks_exact_mut(pw).enumerate() {
            let s = &src[mirror(y, h) * w..][..w];
            for (x, v) in row.iter_mut().enumerate() {
                *v = s[mirror(x, w)] as f32;
            }
        }

        // Rows: real to complex.
        let mut spectrum = vec![Complex32::default(); ph * cw];
        par_rows(&mut real, pw, &mut spectrum, cw, threads, |r, s| {
            let mut scratch = self.r2c.make_scratch_vec();
            for (r, s) in r.chunks_exact_mut(pw).zip(s.chunks_exact_mut(cw)) {
                self.r2c
                    .process_with_scratch(r, s, &mut scratch)
                    .expect("buffer lengths match the plan");
            }
        });

        // Columns: forward, mask, inverse, on the transposed spectrum.
        let mut columns = vec![Complex32::default(); cw * ph];
        transpose(&spectrum, &mut columns, ph, cw);
        let scratch_len =
            self.forward.get_inplace_scratch_len().max(self.inverse.get_inplace_scratch_len());
        par_bands(&mut columns, ph, threads, |first, c| {
            let mut scratch = vec![Complex32::default(); scratch_len];
            self.forward.process_with_scratch(c, &mut scratch);
            for (v, k) in c.iter_mut().zip(&self.mask[first * ph..]) {
                *v *= *k;
            }
            self.inverse.process_with_scratch(c, &mut scratch);
        });
        transpose(&columns, &mut spectrum, cw, ph);

        // Rows: complex to real.
        par_rows(&mut spectrum, cw, &mut real, pw, threads, |s, r| {
            let mut scratch = self.c2r.make_scratch_vec();
            for (s, r) in s.chunks_exact_mut(cw).zip(r.chunks_exact_mut(pw)) {
                // The DC and Nyquist bins of a real signal are real; rounding leaves a trace.
                s[0].im = 0.0;
                if pw.is_multiple_of(2) {
                    s[cw - 1].im = 0.0;
                }
                self.c2r
                    .process_with_scratch(s, r, &mut scratch)
                    .expect("buffer lengths match the plan");
            }
        });

        let norm = 1.0 / (pw * ph) as f32;
        for (dst, row) in page.data_mut().chunks_exact_mut(w).zip(real.chunks_exact(pw)) {
            for (d, v) in dst.iter_mut().zip(row) {
                *d = (v * norm).round().clamp(0.0, 255.0) as u8;
            }
        }
        Ok(())
    }
}

/// Reflects an index past the end back into `0..n`, without repeating the edge pixel.
fn mirror(i: usize, n: usize) -> usize {
    if n == 1 {
        return 0;
    }
    let period = 2 * n - 2;
    let m = i % period;
    if m < n { m } else { period - m }
}

/// Runs `f` over matching bands of rows of `a` (rows of `wa`) and `b` (rows of `wb`), split
/// across up to `threads` scoped threads.
fn par_rows<A: Send, B: Send>(
    a: &mut [A],
    wa: usize,
    b: &mut [B],
    wb: usize,
    threads: usize,
    f: impl Fn(&mut [A], &mut [B]) + Sync,
) {
    let rows = a.len() / wa;
    let band = rows.div_ceil(threads).max(1);
    if threads == 1 || rows < 2 {
        f(a, b);
        return;
    }
    std::thread::scope(|scope| {
        for (a, b) in a.chunks_mut(band * wa).zip(b.chunks_mut(band * wb)) {
            let f = &f;
            scope.spawn(move || f(a, b));
        }
    });
}

/// Runs `f` over bands of rows of `a` (rows of `wa`) across up to `threads` scoped threads,
/// with the index of each band's first row.
fn par_bands<A: Send>(
    a: &mut [A],
    wa: usize,
    threads: usize,
    f: impl Fn(usize, &mut [A]) + Sync,
) {
    let rows = a.len() / wa;
    let band = rows.div_ceil(threads).max(1);
    if threads == 1 || rows < 2 {
        f(0, a);
        return;
    }
    std::thread::scope(|scope| {
        for (i, a) in a.chunks_mut(band * wa).enumerate() {
            let f = &f;
            scope.spawn(move || f(i * band, a));
        }
    });
}

fn transpose(src: &[Complex32], dst: &mut [Complex32], rows: usize, cols: usize) {
    const BLOCK: usize = 32;
    for r0 in (0..rows).step_by(BLOCK) {
        for c0 in (0..cols).step_by(BLOCK) {
            for r in r0..(r0 + BLOCK).min(rows) {
                for c in c0..(c0 + BLOCK).min(cols) {
                    dst[c * rows + r] = src[r * cols + c];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cosine grating of `f` cycles a pixel along the direction (dx, dy).
    fn grating(w: u32, h: u32, f: f32, (dx, dy): (f32, f32)) -> Bitmap {
        let len = (dx * dx + dy * dy).sqrt();
        let (ux, uy) = (dx / len, dy / len);
        let data = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| {
                let phase = std::f32::consts::TAU * f * (x as f32 * ux + y as f32 * uy);
                (127.5 + 100.0 * phase.cos()).round() as u8
            })
            .collect();
        Bitmap::new(w, h, PixelFormat::Gray8, data).unwrap()
    }

    /// The standard deviation of the middle of the page, away from the mirrored edges.
    fn amplitude(b: &Bitmap) -> f32 {
        let (w, h) = (b.width(), b.height());
        let vals: Vec<f32> = (h / 4..3 * h / 4)
            .flat_map(|y| (w / 4..3 * w / 4).map(move |x| (x, y)))
            .map(|(x, y)| b.luma_at(x, y) as f32)
            .collect();
        let mean = vals.iter().sum::<f32>() / vals.len() as f32;
        (vals.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / vals.len() as f32).sqrt()
    }

    #[test]
    fn fft_lengths_avoid_large_primes() {
        assert_eq!(fft_len(1264), 1280);
        assert_eq!(fft_len(1680), 1728);
        assert_eq!(fft_len(1860), 1920);
        assert_eq!(fft_len(2480), 2560);
        assert_eq!(fft_len(1), 1);
        assert_eq!(fft_len(7), 8);
    }

    #[test]
    fn mirror_reflects_without_repeating_the_edge() {
        let v: Vec<usize> = (0..8).map(|i| mirror(i, 4)).collect();
        assert_eq!(v, [0, 1, 2, 3, 2, 1, 0, 1]);
        assert_eq!(mirror(5, 1), 0);
    }

    #[test]
    fn fine_diagonal_tone_is_damped() {
        let filter = RainbowFilter::new(200, 150);
        for dir in [(1.0, 1.0), (1.0, -1.0)] {
            let mut page = grating(200, 150, 0.4, dir);
            let before = amplitude(&page);
            filter.apply(&mut page, 1).unwrap();
            let after = amplitude(&page);
            assert!(after < before * 0.2, "{dir:?}: {before} to {after}");
        }
    }

    #[test]
    fn axis_tone_coarse_tone_and_flat_paper_pass() {
        let filter = RainbowFilter::new(200, 150);
        for (f, dir) in [(0.4, (1.0, 0.0)), (0.4, (0.0, 1.0)), (0.1, (1.0, 1.0))] {
            let mut page = grating(200, 150, f, dir);
            let before = page.clone();
            filter.apply(&mut page, 2).unwrap();
            // Inside a margin: the mirrored edges fold a coarse grating into a kink, and the
            // kink's fine harmonics on the diagonals are the filter's to damp.
            let worst = (8..142)
                .flat_map(|y| (8..192).map(move |x| (x, y)))
                .map(|(x, y)| before.luma_at(x, y).abs_diff(page.luma_at(x, y)))
                .max()
                .unwrap();
            assert!(worst <= 2, "{f} {dir:?}: off by {worst}");
        }
        let mut paper = Bitmap::filled(64, 48, PixelFormat::Gray8, 250);
        RainbowFilter::new(64, 48).apply(&mut paper, 1).unwrap();
        assert!(paper.data().iter().all(|&v| v == 250));
    }

    #[test]
    fn threads_change_nothing() {
        let filter = RainbowFilter::new(131, 97);
        let mut one = grating(131, 97, 0.37, (1.0, 0.8));
        let mut four = one.clone();
        filter.apply(&mut one, 1).unwrap();
        filter.apply(&mut four, 4).unwrap();
        assert_eq!(one, four);
    }

    #[test]
    fn wrong_size_or_format_is_refused() {
        let filter = RainbowFilter::new(10, 10);
        assert!(matches!(
            filter.apply(&mut Bitmap::filled(10, 11, PixelFormat::Gray8, 0), 1),
            Err(Error::SizeMismatch { .. })
        ));
        assert!(matches!(
            filter.apply(&mut Bitmap::filled(10, 10, PixelFormat::Rgb8, 0), 1),
            Err(Error::Format { .. })
        ));
    }
}
