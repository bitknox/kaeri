//! Tone: autocontrast that keeps the page's tone, and gamma.
//!
//! Autocontrast stretches the page's levels so its darkest ink reaches black and its paper
//! reaches white, ignoring the outer half percent at each end. It is one curve from the grey
//! levels, applied alike to every channel, so tone is kept. It is left out for colour pages,
//! strips (whose images each have their own levels, and would flicker), and pages whose range
//! is narrow on purpose, such as a faded flashback. Gamma is 1.0 (none) by default.
//!
//! The levels are measured on the source and the curve applied after the resize: a linear
//! stretch gives the same result either way, and the output is the smaller buffer.

use crate::{Bitmap, PixelRect};

/// The share of pixels ignored at each end of the histogram.
pub const CLIP: f32 = 0.005;

/// A page whose levels span less than this is left as it is.
pub const NARROW_RANGE: u8 = 96;

/// A page's darkest and lightest levels, after clipping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Levels {
    pub low: u8,
    pub high: u8,
}

impl Levels {
    /// Measures the grey levels inside `region`, on every second row.
    pub fn measure(page: &Bitmap, region: PixelRect) -> Levels {
        let mut hist = [0u32; 256];
        let mut n = 0u32;
        for y in (region.y..region.bottom()).step_by(2) {
            for x in region.x..region.right() {
                hist[page.luma_at(x, y) as usize] += 1;
                n += 1;
            }
        }
        let cut = (n as f32 * CLIP) as u32;
        let walk = |levels: &mut dyn Iterator<Item = usize>| {
            let mut acc = 0;
            for i in levels {
                acc += hist[i];
                if acc > cut {
                    return i as u8;
                }
            }
            0
        };
        let low = walk(&mut (0..256));
        let high = walk(&mut (0..256).rev());
        Levels { low, high }
    }

    /// Whether the range is narrow enough to be the art's own choice.
    pub fn is_narrow(&self) -> bool {
        self.high < self.low || self.high - self.low < NARROW_RANGE
    }
}

/// A 256-entry lookup for the grey levels, applied alike to every channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToneCurve {
    lut: [u8; 256],
}

impl ToneCurve {
    pub fn identity() -> Self {
        ToneCurve { lut: std::array::from_fn(|i| i as u8) }
    }

    /// A stretch from `levels` to the full range (when given), then `gamma`: output is
    /// input to the power `gamma`, on a 0 to 1 scale, so above 1 darkens the middle tones.
    pub fn new(levels: Option<Levels>, gamma: f32) -> Self {
        let (low, high) = match levels {
            Some(l) if l.high > l.low => (l.low as f32, l.high as f32),
            _ => (0.0, 255.0),
        };
        let gamma = if gamma.is_finite() && gamma > 0.0 { gamma } else { 1.0 };
        let lut = std::array::from_fn(|i| {
            let t = ((i as f32 - low) / (high - low)).clamp(0.0, 1.0);
            let t = if gamma == 1.0 { t } else { t.powf(gamma) };
            (t * 255.0).round() as u8
        });
        ToneCurve { lut }
    }

    pub fn is_identity(&self) -> bool {
        self.lut.iter().enumerate().all(|(i, &v)| i == v as usize)
    }

    pub fn map(&self, v: u8) -> u8 {
        self.lut[v as usize]
    }

    /// Applies the curve to every channel (an RGBA bitmap's alpha too, which stays 255).
    pub fn apply(&self, page: &mut Bitmap) {
        if self.is_identity() {
            return;
        }
        for v in page.data_mut() {
            *v = self.lut[*v as usize];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;

    fn ramp(low: u8, high: u8) -> Bitmap {
        let data = (0..256u32 * 4)
            .map(|i| low + ((i % 256) * (high - low) as u32 / 255) as u8)
            .collect();
        Bitmap::new(256, 4, PixelFormat::Gray8, data).unwrap()
    }

    #[test]
    fn levels_ignore_the_outer_half_percent() {
        let mut page = ramp(40, 220);
        page.data_mut()[0] = 0; // one speck of black
        let l = Levels::measure(&page, page.bounds());
        assert!((40..=42).contains(&l.low), "{l:?}");
        assert!((218..=220).contains(&l.high), "{l:?}");
        assert!(!l.is_narrow());
        assert!(Levels::measure(&ramp(100, 180), PixelRect::full(256, 4)).is_narrow());
    }

    #[test]
    fn the_stretch_reaches_black_and_white_and_keeps_order() {
        let curve = ToneCurve::new(Some(Levels { low: 40, high: 220 }), 1.0);
        assert_eq!((curve.map(40), curve.map(220)), (0, 255));
        assert_eq!((curve.map(0), curve.map(255)), (0, 255));
        assert!((0..255).all(|i| curve.map(i) <= curve.map(i + 1)));
        assert_eq!(curve.map(130), 128);
    }

    #[test]
    fn gamma_one_and_no_levels_is_the_identity() {
        assert!(ToneCurve::new(None, 1.0).is_identity());
        assert!(ToneCurve::new(None, f32::NAN).is_identity());
        let darker = ToneCurve::new(None, 1.8);
        assert!(darker.map(128) < 128);
        assert_eq!((darker.map(0), darker.map(255)), (0, 255));
    }
}
