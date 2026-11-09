//! The check an EPUB or KEPUB that Kaeri wrote passes before it leaves the device: the container
//! as the format requires it, every XML document declaring UTF-8 and being UTF-8, the package
//! naming what it holds, and the book opening again in this crate's own reader. A file that
//! fails names the part that failed, so the reason can be said exactly.

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use zip::CompressionMethod;

use crate::xml::PlainAttr;
use crate::{Book, Limits};

/// What is wrong with a part of the book.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemKind {
    /// The part is not in the file.
    Missing,
    /// The part does not declare UTF-8, or is not UTF-8.
    NotUtf8,
    /// The part is there but is not what the format requires.
    Malformed,
}

/// The part of the book that failed the check, as its path inside the file (`mimetype`,
/// `OEBPS/content.opf`), and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub part: String,
    pub kind: ProblemKind,
}

impl Problem {
    fn new(part: impl Into<String>, kind: ProblemKind) -> Self {
        Self { part: part.into(), kind }
    }
}

const MIMETYPE: &[u8] = b"application/epub+zip";
const CONTAINER: &str = "META-INF/container.xml";

/// Checks the EPUB or KEPUB at `path`.
pub fn check_epub(path: &Path, limits: &Limits) -> Result<(), Problem> {
    let whole = || Problem::new("the file", ProblemKind::Malformed);
    let file = File::open(path).map_err(|_| Problem::new("the file", ProblemKind::Missing))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| whole())?;
    if zip.len() > limits.max_entries {
        return Err(whole());
    }

    // The `mimetype` file first, stored, saying exactly what the book is.
    {
        let mut first =
            zip.by_index(0).map_err(|_| Problem::new("mimetype", ProblemKind::Missing))?;
        if !first.name().is_ok_and(|n| n == "mimetype") {
            return Err(Problem::new("mimetype", ProblemKind::Missing));
        }
        if first.compression() != CompressionMethod::Stored {
            return Err(Problem::new("mimetype", ProblemKind::Malformed));
        }
        let mut text = Vec::new();
        first
            .by_ref()
            .take(64)
            .read_to_end(&mut text)
            .map_err(|_| Problem::new("mimetype", ProblemKind::Malformed))?;
        if text != MIMETYPE {
            return Err(Problem::new("mimetype", ProblemKind::Malformed));
        }
    }

    let names: HashSet<String> = zip
        .file_names()
        .map(|n| n.map(std::borrow::Cow::into_owned))
        .collect::<Result<_, _>>()
        .map_err(|_| whole())?;
    let mut read = |name: &str| -> Result<String, Problem> {
        let mut entry =
            zip.by_name(name).map_err(|_| Problem::new(name, ProblemKind::Missing))?;
        if entry.size() > limits.max_xml_bytes {
            return Err(Problem::new(name, ProblemKind::Malformed));
        }
        let mut bytes = Vec::new();
        entry
            .by_ref()
            .take(limits.max_xml_bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Problem::new(name, ProblemKind::Malformed))?;
        String::from_utf8(bytes).map_err(|_| Problem::new(name, ProblemKind::NotUtf8))
    };

    // Every XML document: UTF-8, saying so, and well-formed.
    let mut documents: Vec<&String> = names.iter().filter(|n| is_xml(n)).collect();
    documents.sort();
    for name in documents {
        let text = read(name)?;
        if !declares_utf8(&text) {
            return Err(Problem::new(name.clone(), ProblemKind::NotUtf8));
        }
        if is_xhtml(name) && !has_meta_charset(&text) {
            return Err(Problem::new(name.clone(), ProblemKind::NotUtf8));
        }
        parse(&text, limits).map_err(|_| Problem::new(name.clone(), ProblemKind::Malformed))?;
    }

    // The container names the package, and the package names what is in the file.
    let container = read(CONTAINER)?;
    let rootfile = {
        let doc = parse(&container, limits)
            .map_err(|_| Problem::new(CONTAINER, ProblemKind::Malformed))?;
        doc.descendants()
            .find(|n| n.has_tag_name("rootfile"))
            .and_then(|n| n.plain_attr("full-path"))
            .map(str::to_owned)
            .ok_or_else(|| Problem::new(CONTAINER, ProblemKind::Malformed))?
    };
    let opf = read(&rootfile)?;
    check_package(&rootfile, &opf, &names, limits)?;

    // Last, the book opens in Kaeri's own reader and every document in its spine reads.
    let book = crate::open(path, limits).map_err(|_| whole())?;
    let Book::Epub(mut epub) = book else { return Err(whole()) };
    let spine: Vec<String> = epub.spine().map(|(_, item)| item.path.clone()).collect();
    if spine.is_empty() {
        return Err(Problem::new(rootfile.as_str(), ProblemKind::Malformed));
    }
    for part in spine {
        epub.resource(&part).map_err(|_| Problem::new(part.as_str(), ProblemKind::Missing))?;
    }
    Ok(())
}

fn check_package(
    at: &str,
    opf: &str,
    names: &HashSet<String>,
    limits: &Limits,
) -> Result<(), Problem> {
    let bad = || Problem::new(at, ProblemKind::Malformed);
    let doc = parse(opf, limits).map_err(|_| bad())?;
    let dc = |local: &str| {
        doc.descendants()
            .find(|n| n.tag_name().name() == local)
            .and_then(|n| n.text())
            .is_some_and(|t| !t.trim().is_empty())
    };
    if !dc("title") || !dc("identifier") || !dc("language") {
        return Err(bad());
    }
    let base = at.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut ids = HashSet::new();
    let mut nav = false;
    for item in doc.descendants().filter(|n| n.has_tag_name("item")) {
        let (Some(id), Some(href)) = (item.plain_attr("id"), item.plain_attr("href")) else {
            return Err(bad());
        };
        ids.insert(id.to_owned());
        nav |= item.plain_attr("properties").is_some_and(|p| p.split(' ').any(|p| p == "nav"));
        let href = percent_decode(href);
        let full = if base.is_empty() { href } else { format!("{base}/{href}") };
        if !names.contains(&full) {
            return Err(Problem::new(full, ProblemKind::Missing));
        }
    }
    if !nav {
        return Err(Problem::new(format!("{base}/nav.xhtml"), ProblemKind::Missing));
    }
    let mut spine = 0;
    for itemref in doc.descendants().filter(|n| n.has_tag_name("itemref")) {
        let known = itemref.plain_attr("idref").is_some_and(|id| ids.contains(id));
        if !known {
            return Err(bad());
        }
        spine += 1;
    }
    if spine == 0 {
        return Err(bad());
    }
    Ok(())
}

fn parse<'a>(text: &'a str, limits: &Limits) -> Result<roxmltree::Document<'a>, ()> {
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        nodes_limit: limits.max_xml_nodes,
        entity_resolver: None,
    };
    roxmltree::Document::parse_with_options(text, options).map_err(drop)
}

fn is_xml(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".xml", ".opf", ".ncx", ".xhtml", ".html", ".htm"].iter().any(|e| lower.ends_with(e))
}

fn is_xhtml(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".xhtml", ".html", ".htm"].iter().any(|e| lower.ends_with(e))
}

/// The document starts with an XML declaration naming UTF-8.
fn declares_utf8(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("<?xml") else { return false };
    let Some(end) = rest.find("?>") else { return false };
    let decl = rest[..end].to_ascii_lowercase().replace('\'', "\"");
    decl.contains("encoding=\"utf-8\"")
}

/// The document's head says `<meta charset="utf-8"/>`, which readers that ignore the XML
/// declaration go by.
fn has_meta_charset(text: &str) -> bool {
    let lower = text.to_ascii_lowercase().replace('\'', "\"");
    let head_end = lower.find("</head>").unwrap_or(lower.len());
    lower[..head_end].contains("<meta charset=\"utf-8\"")
}

fn percent_decode(href: &str) -> String {
    let bytes = href.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations() {
        assert!(declares_utf8("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<a/>"));
        assert!(declares_utf8("<?xml version='1.0' encoding='UTF-8'?><a/>"));
        assert!(!declares_utf8("<?xml version=\"1.0\"?><a/>"));
        assert!(!declares_utf8("<?xml version=\"1.0\" encoding=\"iso-8859-1\"?><a/>"));
        assert!(!declares_utf8("<a/>"));
        assert!(has_meta_charset("<html><head><meta charset=\"utf-8\"/></head><body/></html>"));
        assert!(!has_meta_charset(
            "<html><head></head><body><meta charset=\"utf-8\"/></body></html>"
        ));
        assert_eq!(percent_decode("images/a%20b%23%C3%A9.jpg"), "images/a b#é.jpg");
        assert_eq!(percent_decode("x%2"), "x%2");
    }
}
