//! Writing books: CBZ, EPUB (reflowable or fixed layout) and KEPUB, for downloads and for Send
//! to e-reader. Every writer streams into any `Write + Seek` (a file, or memory) and produces
//! files this crate's own readers open, which the tests hold it to.
//!
//! What goes into a book from a source (a chapter's text, its page images) is untrusted, so
//! chapter XHTML is parsed under the [`Limits`](crate::Limits), and scripts, event handlers,
//! `javascript:` links, frames, objects and embeds are left out of what is written.

mod cbz;
mod epub;
mod kepub;
pub(crate) mod xhtml;

use std::io;
use std::ops::Range;

pub use cbz::CbzWriter;
pub use epub::{BookMeta, EpubFlavor, EpubWriter, FixedLayout};
pub use kepub::kepub_from_epub;

use crate::BookError;

/// The size each part of a book is kept under for Kindle, whose limit for a sent document is
/// 200 MB; the rest leaves room for the package files.
pub const KINDLE_PART_BYTES: u64 = 195_000_000;

/// What one page or chapter adds to a book besides its own bytes: a wrapper document, a
/// manifest line and two ZIP headers, rounded up.
const ITEM_OVERHEAD: u64 = 2048;
/// What every part carries regardless of its items: the package document, navigation, NCX
/// and container, for a book of a few thousand items.
const PART_OVERHEAD: u64 = 512 * 1024;

/// Splits items of the given sizes, in order, into consecutive parts that each stay under
/// `budget` bytes once written. An item too big for any part gets a part of its own.
pub fn split_parts(sizes: &[u64], budget: u64) -> Vec<Range<usize>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut used = PART_OVERHEAD;
    for (i, &size) in sizes.iter().enumerate() {
        let cost = size.saturating_add(ITEM_OVERHEAD);
        if i > start && used.saturating_add(cost) > budget {
            parts.push(start..i);
            start = i;
            used = PART_OVERHEAD;
        }
        used = used.saturating_add(cost);
    }
    if start < sizes.len() {
        parts.push(start..sizes.len());
    }
    parts
}

/// Escapes text for an XML element's content.
pub fn escape_text(s: &str) -> String {
    escape(s, false)
}

/// Escapes text for a double-quoted XML attribute.
pub fn escape_attr(s: &str) -> String {
    escape(s, true)
}

fn escape(s: &str, attr: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            '\n' if attr => out.push_str("&#10;"),
            '\t' if attr => out.push_str("&#9;"),
            '\r' => out.push_str("&#13;"),
            // Characters XML 1.0 cannot carry at all are dropped.
            c if (c as u32) < 0x20 && !matches!(c, '\n' | '\t') => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

fn zip_err(e: zip::result::ZipError) -> BookError {
    match e {
        zip::result::ZipError::Io(io) => BookError::Io(io),
        other => BookError::Io(io::Error::other(other)),
    }
}

/// The extension and media type for a page image's bytes, or an error when they are no image.
fn image_kind(data: &[u8]) -> crate::Result<(&'static str, &'static str)> {
    let mime = crate::mime::sniff_image(data).ok_or_else(|| BookError::Unsupported {
        what: "a page that is not an image".to_owned(),
    })?;
    let ext = match mime {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/avif" => "avif",
        _ => "jxl",
    };
    Ok((ext, mime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_stay_under_budget() {
        let mb = 1_000_000;
        let sizes = vec![60 * mb; 10];
        let parts = split_parts(&sizes, KINDLE_PART_BYTES);
        assert_eq!(parts, [0..3, 3..6, 6..9, 9..10]);
        for p in &parts {
            let total: u64 =
                sizes[p.clone()].iter().map(|s| s + ITEM_OVERHEAD).sum::<u64>() + PART_OVERHEAD;
            assert!(total <= KINDLE_PART_BYTES);
        }
        assert_eq!(split_parts(&[300 * mb, 1], KINDLE_PART_BYTES), [0..1, 1..2]);
        assert_eq!(split_parts(&[], KINDLE_PART_BYTES), Vec::<Range<usize>>::new());
        assert_eq!(
            split_parts(&[1, 2, 3], KINDLE_PART_BYTES),
            vec![std::ops::Range { start: 0, end: 3 }]
        );
    }

    #[test]
    fn escaping() {
        assert_eq!(escape_text("a<b>&\"c\u{1}"), "a&lt;b&gt;&amp;\"c");
        assert_eq!(escape_attr("x\"y\nz"), "x&quot;y&#10;z");
    }
}
