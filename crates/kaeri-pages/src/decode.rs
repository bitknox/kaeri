//! Page bytes to pixels: recognising the format, reading the size from the header, and
//! decoding within a memory cap.
//!
//! JPEG, PNG, WebP and GIF (its first frame) decode here. AVIF is recognised and measured but
//! not decoded: no spike has priced an AVIF decoder on every platform yet, so an AVIF page goes
//! to an LCD screen as its original file for the platform to decode, and
//! [`Error::Unsupported`] tells the engine when a page needs processing.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, ImageReader, Limits};

use crate::{Bitmap, Error, PixelFormat};

/// A page file's image format, read from its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceFormat {
    Jpeg,
    Png,
    WebP,
    Gif,
    Avif,
}

impl SourceFormat {
    /// Recognises a format from the file's magic bytes, whatever its name says.
    pub fn sniff(bytes: &[u8]) -> Option<SourceFormat> {
        match bytes {
            [0xFF, 0xD8, 0xFF, ..] => Some(SourceFormat::Jpeg),
            [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some(SourceFormat::Png),
            [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => Some(SourceFormat::Gif),
            [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => {
                Some(SourceFormat::WebP)
            }
            [_, _, _, _, b'f', b't', b'y', b'p', brand @ ..] if is_avif_brand(brand) => {
                Some(SourceFormat::Avif)
            }
            _ => None,
        }
    }

    /// The MIME type an app's decoder is given with an original file.
    pub fn mime(self) -> &'static str {
        match self {
            SourceFormat::Jpeg => "image/jpeg",
            SourceFormat::Png => "image/png",
            SourceFormat::WebP => "image/webp",
            SourceFormat::Gif => "image/gif",
            SourceFormat::Avif => "image/avif",
        }
    }

    /// Whether [`decode`] can turn it into pixels.
    pub fn decodes(self) -> bool {
        self != SourceFormat::Avif
    }

    fn image_format(self) -> Option<ImageFormat> {
        match self {
            SourceFormat::Jpeg => Some(ImageFormat::Jpeg),
            SourceFormat::Png => Some(ImageFormat::Png),
            SourceFormat::WebP => Some(ImageFormat::WebP),
            SourceFormat::Gif => Some(ImageFormat::Gif),
            SourceFormat::Avif => None,
        }
    }
}

/// The major brand, or any compatible brand, of an ISO media file says AVIF.
fn is_avif_brand(after_ftyp: &[u8]) -> bool {
    // Major brand (4 bytes), minor version (4 bytes), then compatible brands.
    let major = after_ftyp.get(..4);
    let compatible = after_ftyp.get(8..64.min(after_ftyp.len())).unwrap_or(&[]);
    major.is_some_and(|b| b == b"avif" || b == b"avis")
        || compatible.as_chunks::<4>().0.iter().any(|b| b == b"avif" || b == b"avis")
}

/// What the header says, without decoding the pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceInfo {
    pub format: SourceFormat,
    pub width: u32,
    pub height: u32,
}

/// The most a page may decode to. A page over the cap is refused before any pixel is
/// allocated, so one huge scan cannot take a 2 GB reader's memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Width times height.
    pub max_pixels: u64,
}

impl Default for DecodeLimits {
    /// 40 megapixels: a 6200x4650 spread scan (28.8) fits; it decodes to 87 MB as RGB.
    fn default() -> Self {
        DecodeLimits { max_pixels: 40_000_000 }
    }
}

/// Reads the format and size from the file's header.
pub fn inspect(bytes: &[u8]) -> Result<SourceInfo, Error> {
    let format = SourceFormat::sniff(bytes).ok_or(Error::UnknownFormat)?;
    let (width, height) = match format.image_format() {
        Some(f) => ImageReader::with_format(Cursor::new(bytes), f)
            .into_dimensions()
            .map_err(|e| Error::Decode(e.to_string()))?,
        None => {
            avif_size(bytes).ok_or_else(|| Error::Decode("no image size in AVIF".into()))?
        }
    };
    Ok(SourceInfo { format, width, height })
}

/// The largest `ispe` (image spatial extents) property in the file. A grid image lists its
/// tiles' sizes and the whole one, and a thumbnail is smaller, so the largest is the page.
fn avif_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let head = &bytes[..bytes.len().min(256 * 1024)];
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    head.windows(4)
        .enumerate()
        .filter(|(_, w)| *w == b"ispe")
        // 'ispe', version and flags (4), width (4), height (4).
        .filter_map(|(i, _)| head.get(i + 8..i + 16))
        .map(|b| (be(&b[..4]), be(&b[4..])))
        .filter(|&(w, h)| w > 0 && h > 0)
        .max_by_key(|&(w, h)| w as u64 * h as u64)
}

/// Decodes a page to [`PixelFormat::Gray8`] when the file is grey and [`PixelFormat::Rgb8`]
/// otherwise. Transparency is laid over white paper.
pub fn decode(bytes: &[u8], limits: DecodeLimits) -> Result<Bitmap, Error> {
    let info = inspect(bytes)?;
    let Some(format) = info.format.image_format() else {
        return Err(Error::Unsupported(info.format));
    };
    let pixels = info.width as u64 * info.height as u64;
    if pixels > limits.max_pixels {
        return Err(Error::TooLarge {
            width: info.width,
            height: info.height,
            max_pixels: limits.max_pixels,
        });
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut image_limits = Limits::default();
    // Room for the decoded page in its widest form (16-bit RGBA) and the decoder's own rows.
    image_limits.max_alloc = Some(pixels * 8 + (64 << 20));
    reader.limits(image_limits);
    let image = reader.decode().map_err(|e| Error::Decode(e.to_string()))?;
    Ok(from_dynamic(image))
}

fn from_dynamic(image: DynamicImage) -> Bitmap {
    let (w, h) = (image.width(), image.height());
    let colour = image.color();
    let bitmap = match (colour.has_color(), colour.has_alpha()) {
        (false, false) => Bitmap::new(w, h, PixelFormat::Gray8, image.into_luma8().into_raw()),
        (true, false) => Bitmap::new(w, h, PixelFormat::Rgb8, image.into_rgb8().into_raw()),
        (false, true) => {
            let data = image
                .into_luma_alpha8()
                .into_raw()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| over_white(p[0], p[1]))
                .collect();
            Bitmap::new(w, h, PixelFormat::Gray8, data)
        }
        (true, true) => {
            let data = image
                .into_rgba8()
                .into_raw()
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [0, 1, 2].map(|c| over_white(p[c], p[3])))
                .collect();
            Bitmap::new(w, h, PixelFormat::Rgb8, data)
        }
    };
    bitmap.expect("the image crate's buffers match their size")
}

fn over_white(v: u8, alpha: u8) -> u8 {
    let (v, a) = (v as u32, alpha as u32);
    ((v * a + 255 * (255 - a) + 127) / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(image: DynamicImage) -> Vec<u8> {
        let mut out = Vec::new();
        image.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).unwrap();
        out
    }

    #[test]
    fn sniff_reads_magic_bytes() {
        assert_eq!(SourceFormat::sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(SourceFormat::Jpeg));
        assert_eq!(SourceFormat::sniff(b"GIF89a.."), Some(SourceFormat::Gif));
        assert_eq!(SourceFormat::sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(SourceFormat::WebP));
        assert_eq!(
            SourceFormat::sniff(b"\0\0\0\x1cftypavif\0\0\0\0"),
            Some(SourceFormat::Avif)
        );
        assert_eq!(
            SourceFormat::sniff(b"\0\0\0\x20ftypmif1\0\0\0\0mif1avif"),
            Some(SourceFormat::Avif)
        );
        assert_eq!(SourceFormat::sniff(b"\0\0\0\x1cftypmp42\0\0\0\0"), None);
        assert_eq!(SourceFormat::sniff(b"<html>"), None);
        assert_eq!(SourceFormat::sniff(&[]), None);
    }

    #[test]
    fn avif_is_measured_but_not_decoded() {
        // A made-up header: ftyp, then two ispe properties, a thumbnail and the page.
        let mut bytes = b"\0\0\0\x1cftypavif\0\0\0\0mif1avif".to_vec();
        for (w, h) in [(160u32, 213u32), (1600, 2133)] {
            bytes.extend_from_slice(&20u32.to_be_bytes());
            bytes.extend_from_slice(b"ispe\0\0\0\0");
            bytes.extend_from_slice(&w.to_be_bytes());
            bytes.extend_from_slice(&h.to_be_bytes());
        }
        let info = inspect(&bytes).unwrap();
        assert_eq!((info.format, info.width, info.height), (SourceFormat::Avif, 1600, 2133));
        assert!(matches!(
            decode(&bytes, DecodeLimits::default()),
            Err(Error::Unsupported(SourceFormat::Avif))
        ));
    }

    #[test]
    fn grey_files_decode_to_grey_and_alpha_lies_over_white() {
        let grey =
            DynamicImage::ImageLuma8(image::GrayImage::from_raw(2, 1, vec![10, 20]).unwrap());
        let b = decode(&png(grey), DecodeLimits::default()).unwrap();
        assert_eq!((b.format(), b.data()), (PixelFormat::Gray8, &[10u8, 20][..]));

        let rgba = image::RgbaImage::from_raw(2, 1, vec![0, 0, 0, 0, 255, 0, 0, 255]).unwrap();
        let b = decode(&png(DynamicImage::ImageRgba8(rgba)), DecodeLimits::default()).unwrap();
        assert_eq!(b.format(), PixelFormat::Rgb8);
        assert_eq!(b.data(), &[255, 255, 255, 255, 0, 0]);
    }

    #[test]
    fn a_page_over_the_cap_is_refused_before_decoding() {
        let grey = DynamicImage::ImageLuma8(image::GrayImage::new(100, 100));
        let err = decode(&png(grey), DecodeLimits { max_pixels: 9_999 }).unwrap_err();
        assert!(matches!(err, Error::TooLarge { width: 100, height: 100, .. }));
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(matches!(inspect(b"not an image"), Err(Error::UnknownFormat)));
        let mut truncated = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        truncated.extend_from_slice(&[0; 10]);
        assert!(decode(&truncated, DecodeLimits::default()).is_err());
    }
}
