//! One resize to the screen's real size.
//!
//! Shrinking by more than [`AREA_ABOVE`] averages areas in linear light, which keeps
//! screentones from turning into moiré and keeps their brightness: half black and half white
//! dots average to the grey the eye sees, not a darker one. Anything closer to the original
//! size, enlargements included, uses Lanczos3 on the coded values, which is sharp and needs no
//! linear light (S5).

use std::sync::OnceLock;

use fast_image_resize as fr;

use crate::{Bitmap, Error, PixelFormat, PixelRect};

/// Shrink factors above this average areas in linear light; at or below it Lanczos3 is used.
pub const AREA_ABOVE: f32 = 2.0;

/// The size a region is resized to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// As large as fits inside the box, keeping the aspect.
    Within { width: u32, height: u32 },
    /// This width, the height following the aspect: a strip on an e-ink screen.
    Width(u32),
    /// Exactly this size.
    Exact { width: u32, height: u32 },
}

impl Fit {
    /// The output size for a `width` by `height` region. Never zero on either side.
    pub fn size(self, width: u32, height: u32) -> (u32, u32) {
        let (w, h) = (width.max(1) as f64, height.max(1) as f64);
        let (ow, oh) = match self {
            Fit::Exact { width, height } => (width as f64, height as f64),
            Fit::Width(bw) => (bw as f64, (h * bw as f64 / w).round()),
            Fit::Within { width: bw, height: bh } => {
                let scale = (bw as f64 / w).min(bh as f64 / h);
                ((w * scale).round(), (h * scale).round())
            }
        };
        (ow.max(1.0) as u32, oh.max(1.0) as u32)
    }
}

/// How much a region shrinks to reach an output size: above 1 shrinks, below 1 enlarges.
pub fn shrink_factor(region: (u32, u32), output: (u32, u32)) -> f32 {
    let sx = region.0 as f32 / output.0.max(1) as f32;
    let sy = region.1 as f32 / output.1.max(1) as f32;
    sx.max(sy)
}

/// Resizes `region` of `source` to `width` by `height`, in the source's pixel format.
pub fn resize(
    source: &Bitmap,
    region: PixelRect,
    width: u32,
    height: u32,
) -> Result<Bitmap, Error> {
    if region.is_empty() || !region.fits_in(source.width(), source.height()) {
        return Err(Error::Region(region));
    }
    if width == 0 || height == 0 {
        return Err(Error::Region(PixelRect::full(width, height)));
    }
    let (u8_type, u16_type) = match source.format() {
        PixelFormat::Gray8 => (fr::PixelType::U8, fr::PixelType::U16),
        PixelFormat::Rgb8 => (fr::PixelType::U8x3, fr::PixelType::U16x3),
        PixelFormat::Rgba8 => (fr::PixelType::U8x4, fr::PixelType::U16x4),
    };
    let src =
        fr::images::ImageRef::new(source.width(), source.height(), source.data(), u8_type)
            .map_err(resize_error)?;
    let shrink = shrink_factor((region.width, region.height), (width, height));
    let options = |filter| {
        fr::ResizeOptions::new()
            .resize_alg(fr::ResizeAlg::Convolution(filter))
            .crop(region.x as f64, region.y as f64, region.width as f64, region.height as f64)
            // Pages are opaque; an RGBA source's alpha is 255 and needs no premultiplying.
            .use_alpha(false)
    };
    let mut resizer = fr::Resizer::new();
    let data = if shrink > AREA_ABOVE {
        let mapper = srgb_mapper();
        let mut linear = fr::images::Image::new(source.width(), source.height(), u16_type);
        mapper.forward_map(&src, &mut linear).map_err(resize_error)?;
        let mut small = fr::images::Image::new(width, height, u16_type);
        resizer
            .resize(&linear, &mut small, &options(fr::FilterType::Box))
            .map_err(resize_error)?;
        let mut out = fr::images::Image::new(width, height, u8_type);
        mapper.backward_map(&small, &mut out).map_err(resize_error)?;
        out.into_vec()
    } else {
        let mut out = fr::images::Image::new(width, height, u8_type);
        resizer
            .resize(&src, &mut out, &options(fr::FilterType::Lanczos3))
            .map_err(resize_error)?;
        out.into_vec()
    };
    Bitmap::new(width, height, source.format(), data)
}

fn srgb_mapper() -> &'static fr::PixelComponentMapper {
    static MAPPER: OnceLock<fr::PixelComponentMapper> = OnceLock::new();
    MAPPER.get_or_init(fr::create_srgb_mapper)
}

fn resize_error(e: impl std::fmt::Display) -> Error {
    Error::Resize(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_the_aspect() {
        let panel = Fit::Within { width: 1264, height: 1680 };
        assert_eq!(panel.size(1600, 2133), (1260, 1680));
        assert_eq!(panel.size(3200, 2400), (1264, 948));
        assert_eq!(Fit::Width(1264).size(800, 12000), (1264, 18960));
        assert_eq!(Fit::Within { width: 10, height: 10 }.size(1, 100_000), (1, 10));
    }

    #[test]
    fn shrink_factor_takes_the_larger_axis() {
        assert_eq!(shrink_factor((2000, 1000), (1000, 1000)), 2.0);
        assert!(shrink_factor((500, 500), (1000, 1000)) < 1.0);
    }

    #[test]
    fn a_fine_checker_averages_to_linear_grey_when_shrunk_far() {
        // One-pixel black and white checks average to half the light: sRGB 188, not 128.
        let data = (0..64u32 * 64)
            .map(|i| if (i % 64 + i / 64).is_multiple_of(2) { 0 } else { 255 })
            .collect();
        let checker = Bitmap::new(64, 64, PixelFormat::Gray8, data).unwrap();
        let small = resize(&checker, checker.bounds(), 16, 16).unwrap();
        for &v in small.data() {
            assert!((186..=190).contains(&v), "{v}");
        }
    }

    #[test]
    fn crop_region_is_honoured() {
        let mut page = Bitmap::filled(40, 40, PixelFormat::Rgb8, 255);
        // Black in the right half only.
        for y in 0..40 {
            for x in 20..40 {
                let i = (y * 40 + x) * 3;
                page.data_mut()[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        // The filter reads past the crop's edge, as it would inside any page, so only the
        // first column may lighten.
        let out = resize(&page, PixelRect::new(20, 0, 20, 40), 10, 20).unwrap();
        for y in 0..20 {
            assert!(out.row(y)[3..].iter().all(|&v| v < 2), "row {y}: {:?}", out.row(y));
        }
        assert!(resize(&page, PixelRect::new(30, 0, 20, 40), 10, 20).is_err());
    }
}
