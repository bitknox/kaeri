//! Reader pages decoded for the terminal.
//!
//! The engine makes each page as a file: the original, in a format the terminal said it
//! decodes, or raw pixels for anything else. Reading and decoding one takes long enough to
//! stall a turn, so the loop hands the file to the blocking pool, and the picture comes back on
//! a channel of its own, cropped and turned as the leaf says, for the app to draw.
//!
//! A page that does not decode comes back as `None`; the app asks the engine for it once more,
//! then says the page could not load.

use image::{DynamicImage, GrayImage, ImageReader, RgbaImage};
use kaeri_model::reader::{PageEncoding, PageFile, PageHalf, PixelFormat};
use kaeri_tui::{App, PageJob};
use tokio::sync::mpsc;

/// One page, as a picture or as the news that its file was not one.
pub struct Decoded {
    key: String,
    image: Option<DynamicImage>,
}

/// Decode on the blocking pool, and send what came of it to `tx` for [`hand_over`].
pub fn spawn(job: PageJob, tx: &mpsc::UnboundedSender<Decoded>) {
    let tx = tx.clone();
    tokio::task::spawn_blocking(move || {
        let _ = tx.send(now(job));
    });
}

/// Decode on this thread, for what settles the app rather than runs it: `--preview` and the
/// contract tests' harness.
pub fn now(job: PageJob) -> Decoded {
    let image = decode(&job.file).map(|image| shape(image, &job));
    if image.is_none() {
        tracing::debug!(path = %job.file.path, "could not decode a page");
    }
    Decoded { key: job.key, image }
}

/// Give the app what came of one page.
pub fn hand_over(app: &mut App, decoded: Decoded) {
    app.page_decoded(&decoded.key, decoded.image);
}

/// The file's picture, whole: an original through its format, raw pixels row by row.
fn decode(file: &PageFile) -> Option<DynamicImage> {
    match &file.encoding {
        PageEncoding::Original { .. } => {
            ImageReader::open(&file.path).ok()?.with_guessed_format().ok()?.decode().ok()
        }
        PageEncoding::Raw { format, stride } => {
            let bytes = std::fs::read(&file.path).ok()?;
            raw(&bytes, *format, *stride, file.width, file.height)
        }
    }
}

/// Raw pixels, `stride` bytes a row, into a picture `width` by `height`.
fn raw(
    bytes: &[u8],
    format: PixelFormat,
    stride: u32,
    width: u32,
    height: u32,
) -> Option<DynamicImage> {
    let per_pixel = match format {
        PixelFormat::Gray8 => 1,
        PixelFormat::Rgba8 => 4,
    };
    let row = width as usize * per_pixel;
    let stride = (stride as usize).max(row);
    let mut pixels = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        pixels.extend_from_slice(bytes.get(y * stride..y * stride + row)?);
    }
    match format {
        PixelFormat::Gray8 => {
            GrayImage::from_raw(width, height, pixels).map(DynamicImage::from)
        }
        PixelFormat::Rgba8 => {
            RgbaImage::from_raw(width, height, pixels).map(DynamicImage::from)
        }
    }
}

/// The part of the picture the leaf shows: its crop, or the half of a whole spread when no
/// crop says where, then turned a quarter clockwise for a spread on its side.
fn shape(image: DynamicImage, job: &PageJob) -> DynamicImage {
    let (w, h) = (image.width(), image.height());
    let image = match (job.file.crop, job.half) {
        (Some(crop), _) => {
            let x = crop.x.min(w.saturating_sub(1));
            let y = crop.y.min(h.saturating_sub(1));
            let width = crop.width.min(w - x).max(1);
            let height = crop.height.min(h - y).max(1);
            image.crop_imm(x, y, width, height)
        }
        // The engine sends a half as its crop, or as a file holding only that half. A wide
        // picture with no crop is the whole spread, so the half is cut here.
        (None, Some(half)) if w > h => {
            let left = w / 2;
            match half {
                PageHalf::Left => image.crop_imm(0, 0, left, h),
                PageHalf::Right => image.crop_imm(left, 0, w - left, h),
            }
        }
        _ => image,
    };
    if job.rotate { image.rotate90() } else { image }
}

#[cfg(test)]
mod tests {
    use image::{GenericImageView, Rgba};
    use kaeri_model::reader::PixelRect;

    use super::*;

    fn file(encoding: PageEncoding, width: u32, height: u32) -> PageFile {
        PageFile {
            path: String::new(),
            encoding,
            width,
            height,
            crop: None,
            colour: true,
            bytes: 0,
        }
    }

    fn job(file: PageFile, rotate: bool, half: Option<PageHalf>) -> PageJob {
        PageJob { key: "reader/1/c/0".into(), file, rotate, half }
    }

    /// Four pixels across and two down, each with its own grey: 0, 10, 20, ... by position.
    fn grey_rows(stride: usize) -> Vec<u8> {
        let mut bytes = vec![0xEE; stride * 2];
        for y in 0..2 {
            for x in 0..4 {
                bytes[y * stride + x] = (y * 4 + x) as u8 * 10;
            }
        }
        bytes
    }

    #[test]
    fn raw_grey_rows_are_read_by_their_stride() {
        let image = raw(&grey_rows(7), PixelFormat::Gray8, 7, 4, 2).expect("a picture");
        assert_eq!(image.dimensions(), (4, 2));
        let grey = image.to_luma8();
        assert_eq!(grey.get_pixel(3, 0).0, [30]);
        assert_eq!(grey.get_pixel(0, 1).0, [40], "the padding at the row's end is skipped");
    }

    #[test]
    fn raw_rgba_keeps_its_colours_and_a_short_file_is_no_picture() {
        let mut bytes = Vec::new();
        for pixel in [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [9, 9, 9, 255]] {
            bytes.extend_from_slice(&pixel);
        }
        let image = raw(&bytes, PixelFormat::Rgba8, 8, 2, 2).expect("a picture");
        assert_eq!(image.to_rgba8().get_pixel(0, 1), &Rgba([0, 0, 255, 255]));
        assert!(raw(&bytes[..12], PixelFormat::Rgba8, 8, 2, 2).is_none());
    }

    #[test]
    fn a_raw_page_file_decodes_from_disk() {
        let dir = std::env::temp_dir().join(format!("kaeri-pages-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let path = dir.join("page.raw");
        std::fs::write(&path, grey_rows(4)).expect("written");
        let mut page = file(PageEncoding::Raw { format: PixelFormat::Gray8, stride: 4 }, 4, 2);
        page.path = path.display().to_string();
        let decoded = now(job(page, false, None));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(decoded.image.map(|i| i.dimensions()), Some((4, 2)));
        assert_eq!(decoded.key, "reader/1/c/0");
    }

    #[test]
    fn a_crop_keeps_only_its_box_and_a_missing_file_is_no_picture() {
        let image = raw(&grey_rows(4), PixelFormat::Gray8, 4, 4, 2).expect("a picture");
        let mut page = file(PageEncoding::Original { mime: "image/png".into() }, 4, 2);
        page.crop = Some(PixelRect { x: 1, y: 1, width: 2, height: 1 });
        let cropped = shape(image, &job(page.clone(), false, None));
        assert_eq!(cropped.dimensions(), (2, 1));
        assert_eq!(cropped.to_luma8().get_pixel(0, 0).0, [50]);
        page.path = "/nonexistent/kaeri/page.png".into();
        assert!(now(job(page, false, None)).image.is_none());
    }

    #[test]
    fn a_whole_spread_is_cut_to_its_half_and_turned_on_its_side() {
        let image = raw(&grey_rows(4), PixelFormat::Gray8, 4, 4, 2).expect("a picture");
        let page = file(PageEncoding::Original { mime: "image/png".into() }, 4, 2);
        let right = shape(image.clone(), &job(page.clone(), false, Some(PageHalf::Right)));
        assert_eq!(right.dimensions(), (2, 2));
        assert_eq!(right.to_luma8().get_pixel(0, 0).0, [20]);
        // A quarter clockwise: the left column, bottom up, becomes the top row.
        let turned = shape(image, &job(page, true, None));
        assert_eq!(turned.dimensions(), (2, 4));
        let grey = turned.to_luma8();
        assert_eq!((grey.get_pixel(0, 0).0, grey.get_pixel(1, 0).0), ([40], [0]));
    }
}
