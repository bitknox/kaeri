//! EPUB 2 and 3: the container, the package document, the spine, navigation and resources.
//!
//! This module only reads. Serving a book to a web view, with the content security policy and
//! the sanitiser that strips scripts, `on*` attributes, `javascript:` URLs, meta refresh,
//! `<base>`, frames, objects and embeds, is the novel pipeline's job (`read/novel`), built on
//! [`Epub::resource`] and [`resolve`]. Nothing here runs or interprets a script, and no
//! reference is followed off the book: [`resolve`] refuses every URL with a scheme.

mod images;
mod nav;
mod opf;

use std::collections::HashMap;

use sha1::{Digest, Sha1};

use crate::archive::{Archive, ArchiveKind};
use crate::xml::{self, PlainAttr};
use crate::{BookError, Format, Limits, Result, mime, path};

pub use images::ImagePage;
pub use nav::TocEntry;
pub(crate) use nav::html_entities;
pub use opf::{
    Creator, Direction, Layout, ManifestItem, Metadata, Package, PageSpread, Rendition, Series,
    SpineItem,
};

const CONTAINER: &str = "META-INF/container.xml";
const ENCRYPTION: &str = "META-INF/encryption.xml";
const IDPF_OBFUSCATION: &str = "http://www.idpf.org/2008/embedding";
const ADOBE_OBFUSCATION: &str = "http://ns.adobe.com/pdf/enc#RC";

/// How a font is obfuscated. The algorithms are public and only hide a font from casual
/// copying; undoing them is what every reading system does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Obfuscation {
    /// XOR of the first 1040 bytes with the SHA-1 of the unique identifier.
    Idpf,
    /// XOR of the first 1024 bytes with the 16 bytes of the identifier's UUID.
    Adobe,
}

/// A resource read from the book, with the media type to serve it as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub path: String,
    pub media_type: String,
    pub data: Vec<u8>,
}

/// An open EPUB.
pub struct Epub {
    archive: Archive,
    package: Package,
    toc: Vec<TocEntry>,
    page_list: Vec<TocEntry>,
    obfuscated: HashMap<String, Obfuscation>,
    limits: Limits,
}

impl std::fmt::Debug for Epub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Epub")
            .field("package", &self.package.path)
            .field("spine", &self.package.spine.len())
            .field("toc", &self.toc.len())
            .finish_non_exhaustive()
    }
}

/// True when a ZIP archive looks like an EPUB: a `mimetype` entry saying so, or a container
/// file.
pub(crate) fn looks_like_epub(archive: &mut Archive) -> bool {
    if let Some(i) = archive.find("mimetype")
        && archive.entries()[i].size < 64
        && let Ok(m) = archive.read(i)
        && m.trim_ascii() == b"application/epub+zip"
    {
        return true;
    }
    archive.find(CONTAINER).is_some()
}

impl Epub {
    /// Reads the container, the package document and navigation from an open ZIP archive.
    /// Refuses a book with DRM: an `encryption.xml` naming anything but font obfuscation, or
    /// Adobe's `rights.xml` or Apple's `sinf.xml`.
    pub fn from_archive(mut archive: Archive, limits: &Limits) -> Result<Self> {
        if archive.kind() != ArchiveKind::Zip {
            return Err(BookError::Unsupported {
                what: "an EPUB that is not a ZIP".to_owned(),
            });
        }
        if archive.find("META-INF/rights.xml").is_some()
            || archive.find("META-INF/sinf.xml").is_some()
        {
            return Err(BookError::Protected);
        }
        let opf_path = rootfile(&mut archive, limits)?;
        let opf_bytes = read_xml(&mut archive, &opf_path, limits)?;
        let package = opf::parse(&opf_bytes, &opf_path, limits)?;
        let obfuscated = match archive.find(ENCRYPTION) {
            Some(_) => encryption(&mut archive, limits)?,
            None => HashMap::new(),
        };

        let mut nav = nav::Nav::default();
        if let Some(i) = package.nav {
            let p = package.manifest[i].path.clone();
            if let Ok(bytes) = read_xml(&mut archive, &p, limits)
                && let Ok(n) = nav::parse_nav(&bytes, &p, limits)
            {
                nav = n;
            }
        }
        if nav.toc.is_empty()
            && let Some(i) = package.ncx
        {
            let p = package.manifest[i].path.clone();
            if let Ok(bytes) = read_xml(&mut archive, &p, limits)
                && let Ok(n) = nav::parse_ncx(&bytes, &p, limits)
            {
                if nav.page_list.is_empty() {
                    nav.page_list = n.page_list;
                }
                nav.toc = n.toc;
            }
        }
        Ok(Self {
            archive,
            package,
            toc: nav.toc,
            page_list: nav.page_list,
            obfuscated,
            limits: limits.clone(),
        })
    }

    pub fn package(&self) -> &Package {
        &self.package
    }

    pub fn metadata(&self) -> &Metadata {
        &self.package.metadata
    }

    /// The table of contents, from the navigation document or the NCX. Empty when the book has
    /// neither or both are unreadable.
    pub fn toc(&self) -> &[TocEntry] {
        &self.toc
    }

    /// The print page list, when the book maps print pages.
    pub fn page_list(&self) -> &[TocEntry] {
        &self.page_list
    }

    /// The spine's documents in reading order, as manifest items.
    pub fn spine(&self) -> impl Iterator<Item = (&SpineItem, &ManifestItem)> {
        self.package.spine.iter().map(|s| (s, &self.package.manifest[s.item]))
    }

    /// The cover image's manifest item.
    pub fn cover(&self) -> Option<&ManifestItem> {
        self.package.cover.map(|i| &self.package.manifest[i])
    }

    /// Whether the whole book is fixed layout, by the package setting or by every spine item.
    pub fn is_fixed_layout(&self) -> bool {
        let package = self.package.rendition.layout == Some(Layout::PrePaginated);
        self.package
            .spine
            .iter()
            .all(|s| s.layout.map_or(package, |l| l == Layout::PrePaginated))
    }

    /// The layout of one spine item: its own override, else the package's, else reflowable.
    pub fn layout_of(&self, spine_index: usize) -> Layout {
        self.package
            .spine
            .get(spine_index)
            .and_then(|s| s.layout)
            .or(self.package.rendition.layout)
            .unwrap_or(Layout::Reflowable)
    }

    /// Reads a resource by its normalised path in the container. The media type is the
    /// manifest's, or one guessed from the extension for files the manifest leaves out. An
    /// obfuscated font comes back as the plain font.
    pub fn resource(&mut self, path: &str) -> Result<Resource> {
        let index = self.archive.find(path).ok_or_else(|| BookError::missing(path))?;
        let real = self.archive.entries()[index].name.clone();
        let mut data = self.archive.read(index)?;
        if let Some(&how) = self.obfuscated.get(&real) {
            self.deobfuscate(how, &mut data)?;
        }
        let media_type = self
            .package
            .manifest
            .iter()
            .find(|m| m.path == real || m.path.eq_ignore_ascii_case(&real))
            .map(|m| m.media_type.clone())
            .unwrap_or_else(|| mime::guess(&real).to_owned());
        Ok(Resource { path: real, media_type, data })
    }

    /// Whether the container has a file at this normalised path.
    pub fn contains(&self, path: &str) -> bool {
        self.archive.find(path).is_some()
    }

    /// Resolves a URL found in the document at `base` against the book; see [`resolve`].
    pub fn resolve(&self, base: &str, href: &str) -> Option<path::Href> {
        path::resolve(base, href, self.limits.max_path_depth)
    }

    /// The book as a sequence of images, when every spine item is one image and nothing else: a
    /// manga or comic made into an EPUB. `None` when any item has text or more than one image,
    /// or cannot be read, so the book goes to the web view instead.
    pub fn image_pages(&mut self) -> Option<Vec<ImagePage>> {
        images::image_pages(self)
    }

    fn deobfuscate(&self, how: Obfuscation, data: &mut [u8]) -> Result<()> {
        let key: Vec<u8> = match how {
            Obfuscation::Idpf => {
                let id = self.package.metadata.identifier.as_deref().ok_or_else(|| {
                    BookError::malformed(Format::Epub, "an obfuscated font but no identifier")
                })?;
                let id: String =
                    id.chars().filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n')).collect();
                Sha1::digest(id.as_bytes()).to_vec()
            }
            Obfuscation::Adobe => adobe_key(&self.package.metadata.identifiers)
                .ok_or_else(|| {
                    BookError::malformed(Format::Epub, "an obfuscated font but no UUID")
                })?
                .to_vec(),
        };
        let span = match how {
            Obfuscation::Idpf => 1040,
            Obfuscation::Adobe => 1024,
        };
        for (i, b) in data.iter_mut().take(span).enumerate() {
            *b ^= key[i % key.len()];
        }
        Ok(())
    }

    pub(crate) fn archive_mut(&mut self) -> &mut Archive {
        &mut self.archive
    }

    pub(crate) fn limits(&self) -> &Limits {
        &self.limits
    }
}

/// Resolves a URL found in the document at `base` (a normalised path in the container) into a
/// normalised path in the container. Returns `None` for anything off the book: a URL with a
/// scheme, an absolute path, or one that climbs above the container's root.
pub fn resolve(base: &str, href: &str) -> Option<path::Href> {
    path::resolve(base, href, Limits::default().max_path_depth)
}

fn read_xml(archive: &mut Archive, name: &str, limits: &Limits) -> Result<Vec<u8>> {
    let i = archive.find(name).ok_or_else(|| BookError::missing(name))?;
    if archive.entries()[i].size > limits.max_xml_bytes {
        return Err(BookError::TooLarge { what: "XML document", limit: limits.max_xml_bytes });
    }
    archive.read(i)
}

/// The package document's path, from `META-INF/container.xml`, or the only `.opf` file in the
/// book when the container file is missing.
fn rootfile(archive: &mut Archive, limits: &Limits) -> Result<String> {
    if archive.find(CONTAINER).is_none() {
        let mut opfs = archive
            .entries()
            .iter()
            .filter(|e| path::extension(&e.name).as_deref() == Some("opf"));
        return match (opfs.next(), opfs.next()) {
            (Some(e), None) => Ok(e.name.clone()),
            _ => Err(BookError::malformed(Format::Epub, "no container file")),
        };
    }
    let bytes = read_xml(archive, CONTAINER, limits)?;
    let text = xml::text(&bytes, Format::Epub, limits)?;
    let doc = xml::parse(&text, Format::Epub, limits)?;
    let rootfiles: Vec<_> = doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "rootfile")
        .collect();
    let chosen = rootfiles
        .iter()
        .find(|n| n.plain_attr("media-type") == Some("application/oebps-package+xml"))
        .or_else(|| rootfiles.first())
        .and_then(|n| n.plain_attr("full-path"))
        .ok_or_else(|| {
            BookError::malformed(Format::Epub, "the container names no package document")
        })?;
    let decoded = path::percent_decode(chosen).unwrap_or_else(|| chosen.to_owned());
    path::entry_name(&decoded, limits.max_name_bytes, limits.max_path_depth).ok_or_else(|| {
        BookError::malformed(Format::Epub, "the package document's path leaves the book")
    })
}

/// Reads `encryption.xml`: obfuscated fonts are noted, anything else is DRM.
fn encryption(archive: &mut Archive, limits: &Limits) -> Result<HashMap<String, Obfuscation>> {
    let bytes = read_xml(archive, ENCRYPTION, limits)?;
    let text = xml::text(&bytes, Format::Epub, limits)?;
    let doc = xml::parse(&text, Format::Epub, limits)?;
    let mut out = HashMap::new();
    for data in
        doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "EncryptedData")
    {
        let algorithm = data
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "EncryptionMethod")
            .and_then(|n| n.plain_attr("Algorithm"))
            .unwrap_or_default();
        let how = match algorithm.trim() {
            IDPF_OBFUSCATION => Obfuscation::Idpf,
            ADOBE_OBFUSCATION => Obfuscation::Adobe,
            _ => return Err(BookError::Protected),
        };
        let Some(uri) = data
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "CipherReference")
            .and_then(|n| n.plain_attr("URI"))
        else {
            continue;
        };
        // URIs in encryption.xml are relative to the container's root, so they resolve against
        // a file there.
        if let Some(href) = path::resolve("root", uri, limits.max_path_depth)
            && let Some(i) = archive.find(&href.path)
        {
            out.insert(archive.entries()[i].name.clone(), how);
        }
    }
    Ok(out)
}

/// The Adobe obfuscation key: the 16 bytes of the first `urn:uuid:` identifier.
fn adobe_key(identifiers: &[String]) -> Option<[u8; 16]> {
    identifiers.iter().find_map(|id| {
        let hex: String =
            id.trim().strip_prefix("urn:uuid:")?.chars().filter(|c| *c != '-').collect();
        if hex.len() != 32 {
            return None;
        }
        let mut key = [0u8; 16];
        for (i, k) in key.iter_mut().enumerate() {
            *k = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
        }
        Some(key)
    })
}
