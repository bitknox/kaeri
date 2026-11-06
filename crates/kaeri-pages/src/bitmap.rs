//! The buffers the pipeline passes along: a bitmap in one of three pixel formats, and a pixel
//! rectangle inside one.

use crate::Error;

/// How a bitmap's bytes are laid out. Rows are packed: the stride is the width times
/// [`PixelFormat::bytes_per_pixel`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// One byte of sRGB-coded grey per pixel. Grey e-ink pages, and every page without colour.
    Gray8,
    /// Red, green and blue, one byte each. Decoded colour pages, while they are worked on.
    Rgb8,
    /// Red, green, blue and an opaque alpha. Colour pages as they leave for a colour screen.
    Rgba8,
}

impl PixelFormat {
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Gray8 => 1,
            PixelFormat::Rgb8 => 3,
            PixelFormat::Rgba8 => 4,
        }
    }
}

/// A rectangle of pixels, its origin at the top left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        PixelRect { x, y, width, height }
    }

    /// The whole of a `width` by `height` image.
    pub const fn full(width: u32, height: u32) -> Self {
        PixelRect::new(0, 0, width, height)
    }

    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub const fn right(&self) -> u32 {
        self.x + self.width
    }

    pub const fn bottom(&self) -> u32 {
        self.y + self.height
    }

    /// Width over height; 0 for an empty rectangle.
    pub fn aspect(&self) -> f32 {
        if self.height == 0 { 0.0 } else { self.width as f32 / self.height as f32 }
    }

    /// Whether `self` lies inside a `width` by `height` image.
    pub const fn fits_in(&self, width: u32, height: u32) -> bool {
        self.right() <= width && self.bottom() <= height
    }
}

/// Pixels in memory, rows packed with no padding.
#[derive(Clone, PartialEq, Eq)]
pub struct Bitmap {
    width: u32,
    height: u32,
    format: PixelFormat,
    data: Vec<u8>,
}

impl std::fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bitmap")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}

impl Bitmap {
    /// Wraps `data`, which must hold exactly `width * height` pixels of `format`.
    pub fn new(
        width: u32,
        height: u32,
        format: PixelFormat,
        data: Vec<u8>,
    ) -> Result<Self, Error> {
        let want = width as usize * height as usize * format.bytes_per_pixel();
        if data.len() != want {
            return Err(Error::BufferSize { width, height, format, want, got: data.len() });
        }
        Ok(Bitmap { width, height, format, data })
    }

    /// A bitmap with every byte set to `value`: white paper at 255.
    pub fn filled(width: u32, height: u32, format: PixelFormat, value: u8) -> Self {
        let mut data = vec![value; width as usize * height as usize * format.bytes_per_pixel()];
        if format == PixelFormat::Rgba8 {
            data.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p[3] = 255);
        }
        Bitmap { width, height, format, data }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> PixelFormat {
        self.format
    }

    /// Bytes per row.
    pub fn stride(&self) -> usize {
        self.width as usize * self.format.bytes_per_pixel()
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    pub fn bounds(&self) -> PixelRect {
        PixelRect::full(self.width, self.height)
    }

    /// One row's bytes.
    pub fn row(&self, y: u32) -> &[u8] {
        let s = self.stride();
        &self.data[y as usize * s..(y as usize + 1) * s]
    }

    /// The grey value of one pixel (BT.601 luma for colour).
    pub fn luma_at(&self, x: u32, y: u32) -> u8 {
        let bpp = self.format.bytes_per_pixel();
        let i = (y as usize * self.width as usize + x as usize) * bpp;
        match self.format {
            PixelFormat::Gray8 => self.data[i],
            PixelFormat::Rgb8 | PixelFormat::Rgba8 => {
                luma(self.data[i], self.data[i + 1], self.data[i + 2])
            }
        }
    }

    /// The same pixels in grey. A grey bitmap is returned as it is.
    pub fn into_gray(self) -> Bitmap {
        if self.format == PixelFormat::Gray8 {
            return self;
        }
        let bpp = self.format.bytes_per_pixel();
        let data = self.data.chunks_exact(bpp).map(|p| luma(p[0], p[1], p[2])).collect();
        Bitmap { width: self.width, height: self.height, format: PixelFormat::Gray8, data }
    }

    /// The same pixels as opaque RGBA.
    pub fn into_rgba(self) -> Bitmap {
        let data = match self.format {
            PixelFormat::Rgba8 => return self,
            PixelFormat::Gray8 => self.data.iter().flat_map(|&v| [v, v, v, 255]).collect(),
            PixelFormat::Rgb8 => self
                .data
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
        };
        Bitmap { width: self.width, height: self.height, format: PixelFormat::Rgba8, data }
    }

    /// A copy of the pixels inside `rect`.
    pub fn crop(&self, rect: PixelRect) -> Result<Bitmap, Error> {
        if rect.is_empty() || !rect.fits_in(self.width, self.height) {
            return Err(Error::Region(rect));
        }
        let bpp = self.format.bytes_per_pixel();
        let mut data = Vec::with_capacity(rect.width as usize * rect.height as usize * bpp);
        for y in rect.y..rect.bottom() {
            let row = self.row(y);
            data.extend_from_slice(&row[rect.x as usize * bpp..rect.right() as usize * bpp]);
        }
        Bitmap::new(rect.width, rect.height, self.format, data)
    }
}

/// BT.601 luma from sRGB-coded components, in fixed point.
pub(crate) fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((77 * r as u32 + 150 * g as u32 + 29 * b as u32 + 128) >> 8) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_checks_the_buffer_length() {
        assert!(Bitmap::new(2, 2, PixelFormat::Rgb8, vec![0; 12]).is_ok());
        assert!(matches!(
            Bitmap::new(2, 2, PixelFormat::Rgb8, vec![0; 11]),
            Err(Error::BufferSize { want: 12, got: 11, .. })
        ));
    }

    #[test]
    fn grey_and_rgba_conversions_keep_neutral_values() {
        let rgb = Bitmap::new(2, 1, PixelFormat::Rgb8, vec![0, 0, 0, 200, 200, 200]).unwrap();
        let grey = rgb.clone().into_gray();
        assert_eq!(grey.data(), &[0, 200]);
        let rgba = grey.into_rgba();
        assert_eq!(rgba.data(), &[0, 0, 0, 255, 200, 200, 200, 255]);
        assert_eq!(rgb.into_rgba().stride(), 8);
    }

    #[test]
    fn crop_copies_the_rectangle_and_refuses_outside_ones() {
        let data = (0..16).collect();
        let b = Bitmap::new(4, 4, PixelFormat::Gray8, data).unwrap();
        let c = b.crop(PixelRect::new(1, 2, 2, 2)).unwrap();
        assert_eq!(c.data(), &[9, 10, 13, 14]);
        assert!(b.crop(PixelRect::new(3, 0, 2, 1)).is_err());
        assert!(b.crop(PixelRect::new(0, 0, 0, 1)).is_err());
    }
}
