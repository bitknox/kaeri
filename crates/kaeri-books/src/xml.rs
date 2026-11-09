//! XML from a book: decoded to text within the size limit, then parsed with a node limit.
//! roxmltree resolves no external entities, never fetches anything and stops entity loops, so a
//! DOCTYPE (common in EPUB 2 files) is allowed.
//!
//! roxmltree recurses once per level of element nesting, so before it sees a document a scan
//! here measures the nesting and refuses anything deeper than [`Limits::max_xml_depth`]. Entity
//! values with markup in them could nest further than the scan sees, so a DOCTYPE that declares
//! one is refused too; real books use entities only for characters.

use crate::{BookError, Format, Limits, Result};

/// Decodes an XML document's bytes: UTF-8 with or without a byte order mark, or UTF-16 with one.
pub(crate) fn text(bytes: &[u8], format: Format, limits: &Limits) -> Result<String> {
    if bytes.len() as u64 > limits.max_xml_bytes {
        return Err(BookError::TooLarge { what: "XML document", limit: limits.max_xml_bytes });
    }
    let utf16 = |be: bool| -> Result<String> {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| if be { u16::from_be_bytes(*c) } else { u16::from_le_bytes(*c) })
            .collect();
        String::from_utf16(&units)
            .map_err(|_| BookError::malformed(format, "bad UTF-16 in XML"))
    };
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => std::str::from_utf8(rest)
            .map(str::to_owned)
            .map_err(|_| BookError::malformed(format, "bad UTF-8 in XML")),
        [0xFF, 0xFE, ..] => utf16(false),
        [0xFE, 0xFF, ..] => utf16(true),
        _ => std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| BookError::malformed(format, "bad UTF-8 in XML")),
    }
}

/// Parses XML text with the node limit, allowing a DOCTYPE.
pub(crate) fn parse<'a>(
    text: &'a str,
    format: Format,
    limits: &Limits,
) -> Result<roxmltree::Document<'a>> {
    // A UTF-16 file decoded to text still declares its encoding; roxmltree only reads UTF-8 text
    // and ignores the declaration, so nothing to strip.
    check_depth(text, limits.max_xml_depth)
        .map_err(|reason| BookError::malformed(format, reason))?;
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        nodes_limit: limits.max_xml_nodes,
        entity_resolver: None,
    };
    roxmltree::Document::parse_with_options(text, options)
        .map_err(|e| BookError::malformed(format, e))
}

/// An element's attribute that has no namespace. roxmltree's `attribute("href")` matches any
/// attribute with that local name, `xlink:href` and `xml:lang` included, and returns the first
/// in document order. The reader and the sanitizer need the plain one, the one a renderer reads.
pub(crate) trait PlainAttr<'a> {
    fn plain_attr(&self, name: &str) -> Option<&'a str>;
}

impl<'a> PlainAttr<'a> for roxmltree::Node<'a, '_> {
    fn plain_attr(&self, name: &str) -> Option<&'a str> {
        self.attributes()
            .find(|a| a.namespace().is_none() && a.name() == name)
            .map(|a| a.value())
    }
}

/// The text inside an element, trimmed, or `None` when there is none.
pub(crate) fn trimmed_text(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let mut s = String::new();
    for d in node.descendants().filter(roxmltree::Node::is_text) {
        s.push_str(d.text().unwrap_or_default());
    }
    let t = collapse(&s);
    (!t.is_empty()).then_some(t)
}

/// Folds runs of whitespace into single spaces and trims the ends.
pub(crate) fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Measures element nesting without building anything, and errs when it passes `max`. It counts
/// on the safe side: anything it cannot make sense of counts as an opening tag.
fn check_depth(text: &str, max: usize) -> std::result::Result<(), &'static str> {
    let b = text.as_bytes();
    let find = |from: usize, needle: &[u8]| {
        b.get(from..)
            .and_then(|rest| rest.windows(needle.len()).position(|w| w == needle))
            .map_or(b.len(), |p| from + p + needle.len())
    };
    let mut depth = 0usize;
    let mut i = 0;
    while let Some(p) = b[i..].iter().position(|&c| c == b'<') {
        i += p;
        let rest = &b[i..];
        if rest.starts_with(b"<!--") {
            i = find(i + 4, b"-->");
        } else if rest.starts_with(b"<![CDATA[") {
            i = find(i + 9, b"]]>");
        } else if rest.starts_with(b"<?") {
            i = find(i + 2, b"?>");
        } else if rest.starts_with(b"<!") {
            i = skip_doctype(b, i + 2)?;
        } else if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
            i = find(i + 2, b">");
        } else {
            // A start tag: attribute values may hold `>` and `/`, so quotes are followed.
            let mut j = i + 1;
            let mut quote = None;
            let mut last = 0u8;
            while j < b.len() {
                let c = b[j];
                match quote {
                    Some(q) if c == q => quote = None,
                    Some(_) => {}
                    None if c == b'"' || c == b'\'' => quote = Some(c),
                    None if c == b'>' => break,
                    None => {}
                }
                if quote.is_none() && !c.is_ascii_whitespace() {
                    last = c;
                }
                j += 1;
            }
            if last != b'/' {
                depth += 1;
                if depth > max {
                    return Err("elements nested too deeply");
                }
            }
            i = (j + 1).min(b.len());
        }
    }
    Ok(())
}

/// Skips a DOCTYPE from just after `<!`, returning where it ends. Refuses an internal subset
/// whose literals carry markup.
fn skip_doctype(b: &[u8], mut i: usize) -> std::result::Result<usize, &'static str> {
    let mut subset = false;
    while i < b.len() {
        let rest = &b[i..];
        match b[i] {
            b'"' | b'\'' => {
                let q = b[i];
                let end =
                    b[i + 1..].iter().position(|&c| c == q).map_or(b.len(), |p| i + 1 + p);
                if b[i + 1..end].contains(&b'<') {
                    return Err("markup inside a DTD declaration");
                }
                i = end + 1;
                continue;
            }
            b'<' if subset && rest.starts_with(b"<!--") => {
                i = rest.windows(3).position(|w| w == b"-->").map_or(b.len(), |p| i + p + 3);
                continue;
            }
            b'<' if subset && rest.starts_with(b"<?") => {
                i = rest.windows(2).position(|w| w == b"?>").map_or(b.len(), |p| i + p + 2);
                continue;
            }
            b'[' if !subset => subset = true,
            b']' if subset => subset = false,
            b'>' if !subset => return Ok(i + 1),
            _ => {}
        }
        i += 1;
    }
    Ok(b.len())
}

#[cfg(test)]
mod tests {
    use super::check_depth;

    fn nested(n: usize) -> String {
        format!("{}{}", "<a x='/>'>".repeat(n), "</a>".repeat(n))
    }

    #[test]
    fn depth_is_measured() {
        assert!(check_depth(&nested(10), 10).is_ok());
        assert!(check_depth(&nested(11), 10).is_err());
        // Siblings and empty elements do not add depth.
        let flat = "<r>".to_owned() + &"<a/><b></b>".repeat(1000) + "</r>";
        assert!(check_depth(&flat, 2).is_ok());
    }

    #[test]
    fn comments_cdata_and_instructions_do_not_hide_tags() {
        let s = format!("<?xml version='1.0'?><!-- </a></a> --><![CDATA[</a>]]>{}", nested(5));
        assert!(check_depth(&s, 4).is_err());
        assert!(check_depth(&s, 5).is_ok());
    }

    #[test]
    fn doctype_is_skipped_and_markup_entities_refused() {
        let ok = "<!DOCTYPE html [<!ENTITY nb '&#160;'><!-- ] > --><!ATTLIST a b CDATA \"]>\">]><a/>";
        assert!(check_depth(ok, 1).is_ok());
        let bad = "<!DOCTYPE r [<!ENTITY e '<a><a><a></a></a></a>'>]><r>&e;</r>";
        assert!(check_depth(bad, 100).is_err());
        let public = r#"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd"><html/>"#;
        assert!(check_depth(public, 1).is_ok());
    }
}
