//! PDF through `hayro` (spike S2): a page that is one full-page JPEG is handed back as that JPEG,
//! untouched, so the LCD path keeps the original file; any other page is rendered.
//!
//! hayro opens PDFs whose only password is the empty one (an owner password with no user
//! password); a PDF that needs a password to open is [`BookError::Protected`].
//!
//! Opening is bounded in memory and stack but not in time: a damaged cross-reference table
//! sends hayro through a repair scan, and a crafted file of nested dictionaries makes that scan
//! cost seconds per megabyte. Open PDFs off the interface thread, where the work can be dropped.

use hayro::hayro_syntax::content::ops::TypedInstruction;
use hayro::hayro_syntax::object::{Dict, Name};
use hayro::hayro_syntax::page::Rotation;
use hayro::hayro_syntax::{Filter, LoadPdfError};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};

use crate::{BookError, Format, Limits, Result};

/// An open PDF, held in memory.
pub struct Pdf {
    doc: hayro::hayro_syntax::Pdf,
    limits: Limits,
}

impl std::fmt::Debug for Pdf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pdf").field("pages", &self.page_count()).finish_non_exhaustive()
    }
}

/// A page's own image, passed through without rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfImage {
    pub data: Vec<u8>,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
}

/// A rendered page: straight RGBA, 8 bits a channel, rows `width * 4` bytes apart, on white.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Title and author from the document information dictionary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PdfInfo {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
}

/// The most content operators looked at when deciding whether a page is one image.
const PASSTHROUGH_OPS: usize = 64;

impl Pdf {
    /// Opens a PDF from its bytes.
    pub fn open(bytes: Vec<u8>, limits: &Limits) -> Result<Self> {
        if bytes.len() as u64 > limits.max_pdf_bytes {
            return Err(BookError::TooLarge { what: "PDF size", limit: limits.max_pdf_bytes });
        }
        let doc = guard(|| hayro::hayro_syntax::Pdf::new(bytes))?.map_err(|e| match e {
            LoadPdfError::Decryption(_) => BookError::Protected,
            LoadPdfError::Invalid => BookError::malformed(Format::Pdf, "not a readable PDF"),
        })?;
        let pages = doc.pages().len();
        if pages == 0 {
            return Err(BookError::Empty);
        }
        if pages > limits.max_pdf_pages {
            return Err(BookError::TooLarge {
                what: "page count",
                limit: limits.max_pdf_pages as u64,
            });
        }
        Ok(Self { doc, limits: limits.clone() })
    }

    pub fn page_count(&self) -> usize {
        self.doc.pages().len()
    }

    /// A page's size in points as it is shown, rotation applied.
    pub fn page_size(&self, index: usize) -> Option<(f32, f32)> {
        self.doc.pages().get(index).map(|p| p.render_dimensions())
    }

    pub fn info(&self) -> PdfInfo {
        let m = self.doc.metadata();
        PdfInfo {
            title: m.title.as_deref().and_then(pdf_text),
            author: m.author.as_deref().and_then(pdf_text),
            subject: m.subject.as_deref().and_then(pdf_text),
        }
    }

    /// The page's JPEG, when the page is that one image drawn over the whole page and nothing
    /// else: the usual shape of a scanned or exported comic. `None` means render it.
    pub fn page_image(&self, index: usize) -> Option<PdfImage> {
        guard(|| self.passthrough(index)).ok().flatten()
    }

    fn passthrough(&self, index: usize) -> Option<PdfImage> {
        let page = self.doc.pages().get(index)?;
        if !matches!(page.rotation(), Rotation::None) {
            return None;
        }
        let mut image: Option<Name<'_>> = None;
        let mut matrix = [1.0f64, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mut ops = page.typed_operations();
        let mut seen = 0usize;
        while let Some(op) = ops.next() {
            seen += 1;
            if seen > PASSTHROUGH_OPS {
                return None;
            }
            match op {
                TypedInstruction::SaveState(_) | TypedInstruction::RestoreState(_) => {}
                TypedInstruction::Transform(t) => {
                    if image.is_some() {
                        return None;
                    }
                    let m = [
                        t.0.as_f64(),
                        t.1.as_f64(),
                        t.2.as_f64(),
                        t.3.as_f64(),
                        t.4.as_f64(),
                        t.5.as_f64(),
                    ];
                    matrix = multiply(m, matrix);
                }
                TypedInstruction::XObject(x) => {
                    if image.is_some() {
                        return None;
                    }
                    image = Some(x.0.clone());
                }
                TypedInstruction::BeginMarkedContent(_)
                | TypedInstruction::BeginMarkedContentWithProperties(_)
                | TypedInstruction::EndMarkedContent(_) => {}
                _ => return None,
            }
        }
        let name = image?;
        let stream = page.resources().get_x_object(&name)?;
        let dict: &Dict<'_> = stream.dict();
        if dict.get::<Name<'_>>(b"Subtype")?.as_ref() != b"Image" {
            return None;
        }
        if stream.filters().as_slice() != [Filter::DctDecode] {
            return None;
        }
        // A mask, a decode array or an unusual colour space changes what the JPEG's pixels mean.
        for key in [&b"SMask"[..], b"Mask", b"Decode", b"ImageMask"] {
            if dict.contains_key(key) {
                return None;
            }
        }
        let colour = dict.get::<Name<'_>>(b"ColorSpace")?;
        if !matches!(colour.as_ref(), b"DeviceGray" | b"DeviceRGB") {
            return None;
        }
        let width: u32 = dict.get(b"Width")?;
        let height: u32 = dict.get(b"Height")?;
        // The image must fill the media box: no offset, no skew, no flip.
        let mb = page.media_box();
        let (pw, ph) = (mb.x1 - mb.x0, mb.y1 - mb.y0);
        let close = |a: f64, b: f64| (a - b).abs() <= b.abs().max(1.0) * 0.01;
        let [a, b, c, d, e, f] = matrix;
        if !(close(a, pw)
            && close(d, ph)
            && b.abs() < 1e-6
            && c.abs() < 1e-6
            && close(e, mb.x0)
            && close(f, mb.y0))
        {
            return None;
        }
        let data = stream.raw_data().into_owned();
        if data.len() as u64 > self.limits.max_entry_bytes || !data.starts_with(&[0xFF, 0xD8]) {
            return None;
        }
        Some(PdfImage { data, mime: "image/jpeg", width, height })
    }

    /// Renders a page to fit inside `max_width` by `max_height` pixels, keeping its shape.
    pub fn render(
        &self,
        index: usize,
        max_width: u32,
        max_height: u32,
    ) -> Result<RenderedPage> {
        let page = self
            .doc
            .pages()
            .get(index)
            .ok_or_else(|| BookError::missing(format!("page {}", index + 1)))?;
        let (w, h) = page.render_dimensions();
        if !(w.is_finite() && h.is_finite() && w >= 1.0 && h >= 1.0) {
            return Err(BookError::malformed(Format::Pdf, "a page with no size"));
        }
        let max_width = max_width.clamp(1, u32::from(u16::MAX)) as f32;
        let max_height = max_height.clamp(1, u32::from(u16::MAX)) as f32;
        let mut scale = (max_width / w).min(max_height / h);
        let pixels =
            |s: f32| f64::from((w * s).floor().max(1.0)) * f64::from((h * s).floor().max(1.0));
        if pixels(scale) > self.limits.max_render_pixels as f64 {
            scale *= (self.limits.max_render_pixels as f64 / pixels(scale)).sqrt() as f32;
        }
        let pixmap = guard(|| {
            let cache = RenderCache::new();
            hayro::render(
                page,
                &cache,
                &hayro::hayro_interpret::InterpreterSettings::default(),
                &RenderSettings::default(),
                &PixmapSettings { x_scale: scale, y_scale: scale, bg_color: WHITE },
            )
        })?;
        // On an opaque white background premultiplied and straight alpha are the same bytes.
        Ok(RenderedPage {
            width: u32::from(pixmap.width()),
            height: u32::from(pixmap.height()),
            rgba: pixmap.data_as_u8_slice().to_vec(),
        })
    }
}

fn multiply(m: [f64; 6], n: [f64; 6]) -> [f64; 6] {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

/// A PDF text string: UTF-16 with a byte order mark, UTF-8 with one, or else PDFDocEncoding,
/// read as Latin-1, which it matches for letters.
fn pdf_text(b: &[u8]) -> Option<String> {
    let s = match b {
        [0xFE, 0xFF, rest @ ..] => {
            let units: Vec<u16> =
                rest.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
            String::from_utf16_lossy(&units)
        }
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => b.iter().map(|&c| char::from(c)).collect(),
    };
    let s = s.chars().filter(|c| !c.is_control()).collect::<String>();
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// Runs a hayro call, catching a panic outside fuzzing builds; see the RAR reader.
fn guard<T>(f: impl FnOnce() -> T) -> Result<T> {
    #[cfg(not(feature = "fuzzing"))]
    return std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .map_err(|_| BookError::malformed(Format::Pdf, "the PDF renderer failed"));
    #[cfg(feature = "fuzzing")]
    return Ok(f());
}
