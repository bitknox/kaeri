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
