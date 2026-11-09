//! Image-only EPUBs: manga and comics packed one image per spine item, usually as fixed layout.
//! Read as a list of images, they go through the same page path as a CBZ, which on e-ink is far
//! cheaper than rendering each page in a web view.

use super::{Epub, Layout, PageSpread, nav::html_entities};
use crate::xml::{self, PlainAttr};
use crate::{Format, mime, path};

/// One page of an image-only EPUB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePage {
    /// The page's place in the spine.
    pub spine: usize,
    /// The image's normalised path in the container.
    pub path: String,
    pub media_type: String,
    pub spread: Option<PageSpread>,
    pub layout: Layout,
}

pub(super) fn image_pages(epub: &mut Epub) -> Option<Vec<ImagePage>> {
    let limits = epub.limits().clone();
    let items: Vec<(usize, String, String, Option<PageSpread>)> = epub
        .package()
        .spine
        .iter()
        .enumerate()
        .filter(|(_, s)| s.linear)
        .map(|(i, s)| {
            let m = &epub.package().manifest[s.item];
            (i, m.path.clone(), m.media_type.clone(), s.spread)
        })
        .collect();
    if items.is_empty() {
        return None;
    }
    let mut pages = Vec::with_capacity(items.len());
    for (spine, doc_path, media_type, spread) in items {
        let layout = epub.layout_of(spine);
        let image = if mime::is_image(&media_type) {
            doc_path
        } else if media_type == "application/xhtml+xml" || media_type == "image/svg+xml" {
            let i = epub.archive_mut().find(&doc_path)?;
            if epub.archive_mut().entries()[i].size > limits.max_xml_bytes {
                return None;
            }
            let bytes = epub.archive_mut().read(i).ok()?;
            only_image(&bytes, &doc_path, &limits)?
        } else {
            return None;
        };
        if !epub.contains(&image) {
            return None;
        }
        let media_type = epub
            .package()
            .manifest
            .iter()
            .find(|m| m.path == image)
            .map(|m| m.media_type.clone())
            .filter(|t| mime::is_image(t))
            .or_else(|| mime::page_image(&image).map(str::to_owned))?;
        pages.push(ImagePage { spine, path: image, media_type, spread, layout });
    }
    Some(pages)
}

/// The one image an XHTML or SVG document shows, when it shows exactly one and no text.
fn only_image(bytes: &[u8], doc_path: &str, limits: &crate::Limits) -> Option<String> {
    let text = html_entities(&xml::text(bytes, Format::Epub, limits).ok()?);
    let doc = xml::parse(&text, Format::Epub, limits).ok()?;
    let body = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "body")
        .unwrap_or_else(|| doc.root_element());
    let mut found: Option<String> = None;
    for n in body.descendants() {
        if n.is_text() {
            // Text in <title> or <style> is outside <body>; any visible text here means a page
            // with words on it, which belongs in the web view.
            let in_style = n
                .ancestors()
                .any(|a| a.is_element() && matches!(a.tag_name().name(), "style" | "title"));
            if !in_style && n.text().is_some_and(|t| !t.trim().is_empty()) {
                return None;
            }
            continue;
        }
        if !n.is_element() {
            continue;
        }
        let src = match n.tag_name().name() {
            "img" => n.plain_attr("src"),
            "image" => n.attributes().find(|a| a.name() == "href").map(|a| a.value()),
            _ => continue,
        };
        let resolved = path::resolve(doc_path, src?, limits.max_path_depth)?;
        if found.as_ref().is_some_and(|f| *f != resolved.path) {
            return None;
        }
        found = Some(resolved.path);
    }
    found
}
