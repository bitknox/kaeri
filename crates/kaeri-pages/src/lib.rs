//! The reader's image pipeline: what turns a comic or manga page into what a reader shows.
//!
//! Pure functions over buffers: page bytes in, pixels and their metadata out. No file, socket,
//! clock or runtime, and no other Kaeri crate, so the engine runs it on its prefetch worker
//! and every platform links the same code; `tools/check-boundaries.sh` holds it to that. The
//! engine writes what comes out to the page cache and hands the apps a path with the page's
//! metadata (the reading protocol, section 7.1). The apps never call this crate.
//!
//! One order for every page:
//!
//! 1. [`decode`]: JPEG, PNG, WebP and GIF, within a memory cap. AVIF is recognised and
//!    measured, not decoded.
//! 2. [`colour`]: whether the page has colour, in YCbCr, allowing for yellowed paper. Black
//!    and white pages are worked on in grey from here.
//! 3. [`crop`]: blank borders off, at most a tenth a side, never on spreads.
//! 4. [`layout`]: a page wider than 1.16 is a spread; leaves pair pages, split or rotate
//!    spreads.
//! 5. [`strip`]: e-ink strips cut at gaps between panels, overlapping when no gap is near.
//! 6. [`tone`]: autocontrast that keeps tone, skipped for colour, strips and narrow ranges;
//!    gamma 1.0 by default.
//! 7. [`resize`]: one resize to the screen's real size, an area average in linear light when
//!    shrinking past 2x and Lanczos3 otherwise.
//! 8. The screen's output ([`render`]): 16 greys with an 8x8 ordered [`dither`] on grey
//!    e-ink; no dither on Kaleido, where the [`rainbow`] filter can run on black and white
//!    pages (off by default); colour pages with the panel's saturation on colour e-ink.
//!    Floyd-Steinberg is for 4-bit PNG export only.
//!
//! On LCD screens [`render::delivery`] sends a page that is not shrunk past 2x as its original
//! file, for the platform to decode and draw; the rest are processed like e-ink pages, minus
//! the e-ink steps.
//!
//! The types that cross to the apps (`PageFile`, `Leaf`, `Panel`) live in `kaeri-model`; the
//! mirrors here ([`render::Panel`], [`layout::Leaf`], [`render::RawPage`]) map onto them one to
//! one in the engine.

pub mod bitmap;
pub mod colour;
pub mod crop;
pub mod decode;
pub mod dither;
pub mod export;
pub mod layout;
pub mod rainbow;
pub mod render;
pub mod resize;
pub mod strip;
#[cfg(feature = "testing")]
pub mod testpages;
pub mod tone;

pub use bitmap::{Bitmap, PixelFormat, PixelRect};
pub use decode::{DecodeLimits, SourceFormat, SourceInfo, decode, decode_for, inspect};
pub use render::{
    ColourChoice, Delivery, Look, Panel, RawPage, Renderer, SourcePage, Target, delivery,
};

/// Why a page could not be processed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not an image format Kaeri reads")]
    UnknownFormat,
    #[error("{0:?} pages are not decoded by the engine")]
    Unsupported(SourceFormat),
    #[error("the page is {width}x{height}, over the {max_pixels} pixel limit")]
    TooLarge { width: u32, height: u32, max_pixels: u64 },
    #[error("the page could not be decoded: {0}")]
    Decode(String),
    #[error("the page could not be resized: {0}")]
    Resize(String),
    #[error("the page could not be encoded: {0}")]
    Encode(String),
    #[error("a {width}x{height} {format:?} bitmap needs {want} bytes, not {got}")]
    BufferSize { width: u32, height: u32, format: PixelFormat, want: usize, got: usize },
    #[error("{0:?} is empty or outside the page")]
    Region(PixelRect),
    #[error("needs a {want:?} bitmap, not {got:?}")]
    Format { want: PixelFormat, got: PixelFormat },
    #[error("needs a page of {want:?} pixels, not {got:?}")]
    SizeMismatch { want: (u32, u32), got: (u32, u32) },
    #[error("a strip needs at least one image")]
    EmptyStrip,
}
