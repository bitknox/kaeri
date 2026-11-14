//! EPUB 3 with an EPUB 2 NCX beside the navigation document, so older readers and Kindle's
//! converter find a table of contents too. Two shapes: reflowable chapters of XHTML (novels,
//! text chapters from a source) and fixed layout, one image per page (manga and comics for an
//! e-reader). KEPUB is the reflowable shape with Kobo's sentence spans.

use std::collections::HashSet;
use std::io::{Seek, Write};

use sha1::{Digest, Sha1};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

use super::xhtml::Writer;
use super::{escape_attr, escape_text, image_kind, plain_text, zip_err};
use crate::epub::{Direction, PageSpread, Series};
use crate::{BookError, Format, Limits, Result, path, xml};

const ROOT: &str = "OEBPS";
const RESERVED: &[&str] = &["content.opf", "nav.xhtml", "toc.ncx"];

/// Plain EPUB, or KEPUB for Kobo readers: the same package with every chapter's text in
/// numbered sentence spans (`koboSpan`), which Kobo uses for its page count, highlights and
/// reading statistics. Name a KEPUB file `*.kepub.epub`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpubFlavor {
    Epub,
    Kepub,
}

/// The book's metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookMeta {
    pub title: String,
    pub creators: Vec<String>,
    /// A BCP 47 tag; `und` when empty.
    pub language: String,
    /// The unique identifier. When `None`, one is derived from the title, creators and series,
    /// so the same book written twice has the same identifier and readers keep its progress.
    pub identifier: Option<String>,
    pub series: Option<Series>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    /// The page progression; right to left for most manga.
    pub direction: Option<Direction>,
    /// `dcterms:modified` as `YYYY-MM-DDThh:mm:ssZ`; now when `None`.
    pub modified: Option<String>,
}

/// Fixed layout: every page is one image at the given size in pixels, the size the pages were
/// prepared for (the target screen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedLayout {
    pub width: u32,
    pub height: u32,
    /// Whether a landscape screen shows two pages side by side.
    pub spreads: bool,
}

struct Item {
    id: String,
    href: String,
    media_type: String,
    properties: Vec<&'static str>,
}

struct SpineRef {
    id: String,
    spread: Option<PageSpread>,
}

/// Writes an EPUB item by item: resources and chapters for a reflowable book, image pages for a
/// fixed-layout one. The package document, navigation and NCX are written by
/// [`EpubWriter::finish`].
pub struct EpubWriter<W: Write + Seek> {
    zip: ZipWriter<W>,
    flavor: EpubFlavor,
    meta: BookMeta,
    fixed: Option<FixedLayout>,
    manifest: Vec<Item>,
    spine: Vec<SpineRef>,
    toc: Vec<(String, String)>,
    paths: HashSet<String>,
    cover: Option<String>,
    count: u32,
    limits: Limits,
}

impl<W: Write + Seek> EpubWriter<W> {
    /// Starts a book: the `mimetype` file first and uncompressed, as the format requires, then
    /// the container.
    pub fn new(
        out: W,
        mut meta: BookMeta,
        flavor: EpubFlavor,
        fixed: Option<FixedLayout>,
    ) -> Result<Self> {
        // A title and authors as plain text; a book needs a title, and no author is empty.
        meta.title = plain_text(&meta.title);
        if meta.title.is_empty() {
            return Err(BookError::Unsupported { what: "a book with no title".to_owned() });
        }
        meta.creators =
            meta.creators.iter().map(|c| plain_text(c)).filter(|c| !c.is_empty()).collect();
        if let Some(f) = fixed
            && (f.width == 0 || f.height == 0)
        {
            return Err(BookError::Unsupported {
                what: "a fixed layout with no size".to_owned(),
            });
        }
        let mut zip = ZipWriter::new(out);
        zip.start_file(
            "mimetype",
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .map_err(zip_err)?;
        zip.write_all(b"application/epub+zip")?;
        zip.start_file("META-INF/container.xml", deflated()).map_err(zip_err)?;
        zip.write_all(
            format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<container version=\"1.0\" \
                 xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n  <rootfiles>\n    \
                 <rootfile full-path=\"{ROOT}/content.opf\" media-type=\"application/oebps-package+xml\"/>\n  \
                 </rootfiles>\n</container>\n"
            )
            .as_bytes(),
        )?;
        Ok(Self {
            zip,
            flavor,
            meta,
            fixed,
            manifest: Vec::new(),
            spine: Vec::new(),
            toc: Vec::new(),
            paths: HashSet::new(),
            cover: None,
            count: 0,
            limits: Limits::default(),
        })
    }

    /// Adds a file chapters refer to: a stylesheet, an image, a font. `path` is relative to the
    /// package root, and chapters link to it by that same path.
    pub fn add_resource(&mut self, path: &str, media_type: &str, data: &[u8]) -> Result<()> {
        let clean =
            path::entry_name(path, self.limits.max_name_bytes, self.limits.max_path_depth)
                .filter(|p| !RESERVED.contains(&p.as_str()))
                .ok_or_else(|| BookError::Unsupported {
                    what: format!("a resource path like {path:?}"),
                })?;
        self.count += 1;
        let id = format!("r{}", self.count);
        let stored = data_is_compressed(media_type);
        self.write_file(&clean, data, stored)?;
        self.manifest.push(Item {
            id,
            href: clean,
            media_type: media_type.to_owned(),
            properties: Vec::new(),
        });
        Ok(())
    }

    /// Sets the cover image of a reflowable book. A fixed-layout book's first page is its cover
    /// unless this is called.
    pub fn set_cover(&mut self, data: &[u8]) -> Result<()> {
        let (ext, mime) = image_kind(data)?;
        let href = format!("cover.{ext}");
        self.write_file(&href, data, true)?;
        self.manifest.push(Item {
            id: "cover".to_owned(),
            href,
            media_type: mime.to_owned(),
            properties: vec!["cover-image"],
        });
        self.cover = Some("cover".to_owned());
        Ok(())
    }

    /// Adds a chapter of a reflowable book. `body` is the inside of the chapter's `<body>` as
    /// XHTML; HTML's named entities such as `&nbsp;` are accepted. It is parsed under the
    /// limits, a chapter that is not well-formed is refused, and anything active is left out.
    pub fn add_chapter(&mut self, title: &str, body: &str) -> Result<()> {
        if self.fixed.is_some() {
            return Err(BookError::Unsupported {
                what: "a text chapter in a fixed-layout book".to_owned(),
            });
        }
        let title = plain_text(title);
        let lang = language(&self.meta.language);
        let source = crate::epub::html_entities(&format!(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" \
             xml:lang=\"{l}\" lang=\"{l}\"><head><meta charset=\"utf-8\"/><title>{t}</title></head><body>{body}</body></html>",
            l = escape_attr(&lang),
            t = escape_text(&title),
        ));
        let doc = xml::parse(&source, Format::Epub, &self.limits)?;
        let out = Writer::new(self.flavor == EpubFlavor::Kepub).document(&doc);
        let n = self.spine.len() + 1;
        let href = format!("c{n:04}.xhtml");
        self.write_file(&href, out.as_bytes(), false)?;
        let id = format!("c{n}");
        self.manifest.push(Item {
            id: id.clone(),
            href: href.clone(),
            media_type: "application/xhtml+xml".to_owned(),
            properties: Vec::new(),
        });
        self.spine.push(SpineRef { id, spread: None });
        self.toc.push((title, href));
        Ok(())
    }

    /// Adds the next page of a fixed-layout book: the image, and a page document that shows it
    /// at the layout's size. `title` adds a table of contents entry pointing at this page.
    pub fn add_image_page(
        &mut self,
        data: &[u8],
        spread: Option<PageSpread>,
        title: Option<&str>,
    ) -> Result<()> {
        let Some(fixed) = self.fixed else {
            return Err(BookError::Unsupported {
                what: "an image page in a reflowable book".to_owned(),
            });
        };
        let (ext, mime) = image_kind(data)?;
        let n = self.spine.len() + 1;
        let image = format!("images/{n:05}.{ext}");
        let page = format!("pages/{n:05}.xhtml");
        self.write_file(&image, data, true)?;
        let (w, h) = (fixed.width, fixed.height);
        let doc = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\">\n\
             <head><meta charset=\"utf-8\"/><title>{n}</title><meta name=\"viewport\" content=\"width={w}, height={h}\"/>\
             <style type=\"text/css\">html,body{{margin:0;padding:0;width:{w}px;height:{h}px;overflow:hidden}}\
             img{{display:block;width:{w}px;height:{h}px}}</style></head>\n\
             <body><img src=\"../{image}\" alt=\"\"/></body>\n</html>\n"
        );
        self.write_file(&page, doc.as_bytes(), false)?;
        let cover = self.cover.is_none() && n == 1;
        self.manifest.push(Item {
            id: format!("i{n}"),
            href: image,
            media_type: mime.to_owned(),
            properties: if cover { vec!["cover-image"] } else { Vec::new() },
        });
        if cover {
            self.cover = Some(format!("i{n}"));
        }
        let id = format!("p{n}");
        self.manifest.push(Item {
            id: id.clone(),
            href: page.clone(),
            media_type: "application/xhtml+xml".to_owned(),
            properties: Vec::new(),
        });
        self.spine.push(SpineRef { id, spread });
        if let Some(t) = title {
            self.toc.push((plain_text(t), page));
        }
        Ok(())
    }

    /// Writes the package document, the navigation document and the NCX, and hands back the
    /// output. A book needs at least one chapter or page.
    pub fn finish(mut self) -> Result<W> {
        let first =
            self.manifest.iter().find(|m| self.spine.first().is_some_and(|s| s.id == m.id));
        let Some(first) = first.map(|m| m.href.clone()) else {
            return Err(BookError::Empty);
        };
        if self.toc.is_empty() {
            self.toc.push((self.meta.title.clone(), first));
        }
        let identifier =
            self.meta.identifier.clone().unwrap_or_else(|| derived_identifier(&self.meta));
        let nav = self.nav();
        self.write_file("nav.xhtml", nav.as_bytes(), false)?;
        let ncx = self.ncx(&identifier);
        self.write_file("toc.ncx", ncx.as_bytes(), false)?;
        let opf = self.opf(&identifier);
        self.write_file("content.opf", opf.as_bytes(), false)?;
        self.zip.finish().map_err(zip_err)
    }

    fn write_file(&mut self, href: &str, data: &[u8], stored: bool) -> Result<()> {
        if !self.paths.insert(href.to_ascii_lowercase()) {
            return Err(BookError::Unsupported { what: format!("two files at {href:?}") });
        }
        let opts = if stored {
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
        } else {
            deflated()
        }
        .large_file(data.len() as u64 >= u64::from(u32::MAX));
        self.zip.start_file(format!("{ROOT}/{href}"), opts).map_err(zip_err)?;
        self.zip.write_all(data)?;
        Ok(())
    }

    fn nav(&self) -> String {
        let lang = escape_attr(&language(&self.meta.language));
        let title = escape_text(&self.meta.title);
        let mut items = String::new();
        for (label, href) in &self.toc {
            items += &format!(
                "      <li><a href=\"{}\">{}</a></li>\n",
                escape_attr(&href_encode(href)),
                escape_text(label)
            );
        }
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" \
             xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{lang}\" lang=\"{lang}\">\n<head><meta charset=\"utf-8\"/><title>{title}</title></head>\n\
             <body>\n  <nav epub:type=\"toc\" id=\"toc\">\n    <h1>{title}</h1>\n    <ol>\n{items}    </ol>\n  </nav>\n</body>\n</html>\n"
        )
    }

    fn ncx(&self, identifier: &str) -> String {
        let mut points = String::new();
        for (i, (label, href)) in self.toc.iter().enumerate() {
            points += &format!(
                "    <navPoint id=\"n{n}\" playOrder=\"{n}\"><navLabel><text>{}</text></navLabel><content src=\"{}\"/></navPoint>\n",
                escape_text(label),
                escape_attr(&href_encode(href)),
                n = i + 1
            );
        }
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\" version=\"2005-1\">\n\
             <head><meta name=\"dtb:uid\" content=\"{}\"/><meta name=\"dtb:depth\" content=\"1\"/>\
             <meta name=\"dtb:totalPageCount\" content=\"0\"/><meta name=\"dtb:maxPageNumber\" content=\"0\"/></head>\n\
             <docTitle><text>{}</text></docTitle>\n<navMap>\n{points}</navMap>\n</ncx>\n",
            escape_attr(identifier),
            escape_text(&self.meta.title)
        )
    }

    fn opf(&self, identifier: &str) -> String {
        let m = &self.meta;
        let lang = language(&m.language);
        let mut meta = String::new();
        meta += &format!(
            "    <dc:identifier id=\"uid\">{}</dc:identifier>\n",
            escape_text(identifier)
        );
        meta += &format!("    <dc:title>{}</dc:title>\n", escape_text(&m.title));
        meta += &format!("    <dc:language>{}</dc:language>\n", escape_text(&lang));
        for (i, c) in m.creators.iter().enumerate() {
            meta +=
                &format!("    <dc:creator id=\"creator{i}\">{}</dc:creator>\n", escape_text(c));
        }
        if let Some(p) = &m.publisher {
            meta += &format!("    <dc:publisher>{}</dc:publisher>\n", escape_text(p));
        }
        if let Some(d) = &m.description {
            meta += &format!("    <dc:description>{}</dc:description>\n", escape_text(d));
        }
        let modified = m.modified.clone().unwrap_or_else(now_utc);
        meta += &format!(
            "    <meta property=\"dcterms:modified\">{}</meta>\n",
            escape_text(&modified)
        );
        if let Some(s) = &m.series {
            meta += &format!(
                "    <meta property=\"belongs-to-collection\" id=\"series\">{}</meta>\n    \
                 <meta refines=\"#series\" property=\"collection-type\">series</meta>\n",
                escape_text(&s.name)
            );
            // Calibre's tags too: Kobo and KOReader read the series from them.
            meta += &format!(
                "    <meta name=\"calibre:series\" content=\"{}\"/>\n",
                escape_attr(&s.name)
            );
            if let Some(pos) = &s.position {
                meta += &format!(
                    "    <meta refines=\"#series\" property=\"group-position\">{}</meta>\n    \
                     <meta name=\"calibre:series_index\" content=\"{}\"/>\n",
                    escape_text(pos),
                    escape_attr(pos)
                );
            }
        }
        if let Some(f) = self.fixed {
            let spread = if f.spreads { "landscape" } else { "none" };
            meta += &format!(
                "    <meta property=\"rendition:layout\">pre-paginated</meta>\n    \
                 <meta property=\"rendition:spread\">{spread}</meta>\n    \
                 <meta property=\"rendition:orientation\">auto</meta>\n"
            );
            // Kindle's own fixed-layout and comic properties.
            meta += &format!(
                "    <meta name=\"fixed-layout\" content=\"true\"/>\n    \
                 <meta name=\"original-resolution\" content=\"{}x{}\"/>\n    \
                 <meta name=\"book-type\" content=\"comic\"/>\n    \
                 <meta name=\"zero-gutter\" content=\"true\"/>\n    \
                 <meta name=\"zero-margin\" content=\"true\"/>\n    \
                 <meta name=\"region-mag\" content=\"false\"/>\n",
                f.width, f.height
            );
            if m.direction == Some(Direction::Rtl) {
                meta += "    <meta name=\"primary-writing-mode\" content=\"horizontal-rl\"/>\n";
            }
        }
        if let Some(c) = &self.cover {
            meta += &format!("    <meta name=\"cover\" content=\"{c}\"/>\n");
        }

        let mut manifest = String::from(
            "    <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n    \
             <item id=\"ncx\" href=\"toc.ncx\" media-type=\"application/x-dtbncx+xml\"/>\n",
        );
        for item in &self.manifest {
            let props = if item.properties.is_empty() {
                String::new()
            } else {
                format!(" properties=\"{}\"", item.properties.join(" "))
            };
            manifest += &format!(
                "    <item id=\"{}\" href=\"{}\" media-type=\"{}\"{props}/>\n",
                item.id,
                escape_attr(&href_encode(&item.href)),
                escape_attr(&item.media_type)
            );
        }
        let mut spine = String::new();
        for s in &self.spine {
            let props = match s.spread {
                Some(PageSpread::Left) => " properties=\"page-spread-left\"",
                Some(PageSpread::Right) => " properties=\"page-spread-right\"",
                Some(PageSpread::Center) => " properties=\"rendition:page-spread-center\"",
                None => "",
            };
            spine += &format!("    <itemref idref=\"{}\"{props}/>\n", s.id);
        }
        let direction = match m.direction {
            Some(Direction::Rtl) => " page-progression-direction=\"rtl\"",
            Some(Direction::Ltr) => " page-progression-direction=\"ltr\"",
            None => "",
        };
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" \
             unique-identifier=\"uid\" xml:lang=\"{l}\" prefix=\"rendition: http://www.idpf.org/vocab/rendition/#\">\n  \
             <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n{meta}  </metadata>\n  \
             <manifest>\n{manifest}  </manifest>\n  <spine toc=\"ncx\"{direction}>\n{spine}  </spine>\n</package>\n",
            l = escape_attr(&lang)
        )
    }
}

fn deflated() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

/// Media types whose data gains nothing from deflate.
pub(super) fn data_is_compressed(media_type: &str) -> bool {
    (media_type.starts_with("image/")
        && media_type != "image/svg+xml"
        && media_type != "image/bmp")
        || media_type.starts_with("audio/")
        || media_type.starts_with("video/")
        || media_type == "font/woff"
        || media_type == "font/woff2"
}

fn language(lang: &str) -> String {
    let l = lang.trim();
    if l.is_empty() { "und".to_owned() } else { l.to_owned() }
}

/// A path as a URL path: unreserved characters and `/` stay, everything else is
/// percent-encoded byte by byte.
fn href_encode(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A name-based UUID (version 5 layout over SHA-1) from the book's title, creators and series.
fn derived_identifier(m: &BookMeta) -> String {
    let mut h = Sha1::new();
    h.update(b"kaeri-book\0");
    h.update(m.title.as_bytes());
    for c in &m.creators {
        h.update(b"\0");
        h.update(c.as_bytes());
    }
    if let Some(s) = &m.series {
        h.update(b"\0series\0");
        h.update(s.name.as_bytes());
        h.update(s.position.as_deref().unwrap_or_default().as_bytes());
    }
    let d = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0F) | 0x50;
    b[8] = (b[8] & 0x3F) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "urn:uuid:{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// The current time as `YYYY-MM-DDThh:mm:ssZ`.
fn now_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format_utc(secs)
}

fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Days to a civil date, after Howard Hinnant's algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(format_utc(1_798_761_599), "2026-12-31T23:59:59Z");
    }

    #[test]
    fn hrefs_and_identifiers() {
        assert_eq!(href_encode("images/a b#é.jpg"), "images/a%20b%23%C3%A9.jpg");
        let m = BookMeta { title: "Quiet Hills".into(), ..BookMeta::default() };
        let id = derived_identifier(&m);
        assert_eq!(id, derived_identifier(&m));
        assert_eq!(id.len(), "urn:uuid:".len() + 36);
        assert_eq!(&id[23..24], "5");
    }
}
