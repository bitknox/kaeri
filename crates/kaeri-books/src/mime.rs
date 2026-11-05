//! Media types by extension, for page images and for EPUB resources a manifest leaves untyped.

use crate::path::extension;

/// The media type of a page image, or `None` when the file is not an image Kaeri shows as a page.
pub fn page_image(path: &str) -> Option<&'static str> {
    Some(match extension(path)?.as_str() {
        "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "jxl" => "image/jxl",
        _ => return None,
    })
}

/// The image type a page's bytes actually are, from their first bytes. Names lie often enough
/// (a PNG saved as `.jpg`, a text file named like a page) that the decoder should be told this
/// rather than the extension. `None` means the bytes are no image Kaeri shows.
pub fn sniff_image(head: &[u8]) -> Option<&'static str> {
    Some(match head {
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => "image/png",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => "image/gif",
        [b'B', b'M', ..] => "image/bmp",
        [0xFF, 0x0A, ..] | [0, 0, 0, 0x0C, b'J', b'X', b'L', b' ', ..] => "image/jxl",
        [_, _, _, _, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f' | b's', ..] => "image/avif",
        _ => return None,
    })
}

/// The media type to serve a resource with when the book does not say.
pub fn guess(path: &str) -> &'static str {
    if let Some(image) = page_image(path) {
        return image;
    }
    match extension(path).as_deref() {
        Some("xhtml" | "xht") => "application/xhtml+xml",
        Some("html" | "htm") => "text/html",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        Some("ncx") => "application/x-dtbncx+xml",
        Some("opf") => "application/oebps-package+xml",
        Some("xml") => "application/xml",
        Some("smil") => "application/smil+xml",
        Some("otf") => "font/otf",
        Some("ttf") => "font/ttf",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("js") => "text/javascript",
        Some("mp3") => "audio/mpeg",
        Some("m4a" | "mp4") => "audio/mp4",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

/// True for the image types EPUB allows as core media, which a spine may point at directly.
pub fn is_image(media_type: &str) -> bool {
    media_type.starts_with("image/") && media_type != "image/svg+xml"
}

#[cfg(test)]
mod tests {
    use super::sniff_image;

    #[test]
    fn sniffs_by_content() {
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff_image(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff_image(b"\0\0\0\x1cftypavif"), Some("image/avif"));
        assert_eq!(sniff_image(b"this is not an image at all"), None);
        assert_eq!(sniff_image(b""), None);
    }
}
