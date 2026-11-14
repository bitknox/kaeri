//! Tables of contents: the EPUB 3 navigation document's `toc` and `page-list`, and the EPUB 2
//! NCX as the fallback.

use crate::xml::{self, collapse};
use crate::{Format, Limits, Result, path};

/// One entry in a table of contents or page list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    pub label: String,
    /// `path#fragment` in the container, or `None` for a heading that links nowhere.
    pub href: Option<String>,
    pub children: Vec<TocEntry>,
}

/// The navigation lists one document gives.
#[derive(Debug, Default)]
pub(crate) struct Nav {
    pub toc: Vec<TocEntry>,
    pub page_list: Vec<TocEntry>,
}

struct Budget {
    left: usize,
    max_depth: usize,
    max_path_depth: usize,
}

/// HTML entities that turn up in navigation documents. They are not XML's own, so roxmltree
/// would refuse them; each is swapped for its character reference before parsing.
const ENTITIES: &[(&str, &str)] = &[
    ("&nbsp;", "&#160;"),
    ("&ndash;", "&#8211;"),
    ("&mdash;", "&#8212;"),
    ("&hellip;", "&#8230;"),
    ("&lsquo;", "&#8216;"),
    ("&rsquo;", "&#8217;"),
    ("&ldquo;", "&#8220;"),
    ("&rdquo;", "&#8221;"),
    ("&copy;", "&#169;"),
    ("&middot;", "&#183;"),
];

pub(crate) fn html_entities(text: &str) -> String {
    let mut s = text.to_owned();
    for (from, to) in ENTITIES {
        if s.contains(from) {
            s = s.replace(from, to);
        }
    }
    s
}

/// Parses an EPUB 3 navigation document at `doc_path`.
pub(crate) fn parse_nav(bytes: &[u8], doc_path: &str, limits: &Limits) -> Result<Nav> {
    let text = html_entities(&xml::text(bytes, Format::Epub, limits)?);
    let doc = xml::parse(&text, Format::Epub, limits)?;
    let mut budget = Budget {
        left: limits.max_toc_entries,
        max_depth: limits.max_toc_depth,
        max_path_depth: limits.max_path_depth,
    };
    let mut nav = Nav::default();
    for n in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "nav") {
        let kind = n
            .attributes()
            .find(|a| a.name() == "type")
            .map(|a| a.value().split_whitespace().map(str::to_owned).collect::<Vec<_>>())
            .unwrap_or_default();
        let Some(list) = n.children().find(|c| c.is_element() && c.tag_name().name() == "ol")
        else {
            continue;
        };
        if kind.iter().any(|k| k == "toc") && nav.toc.is_empty() {
            nav.toc = list_items(list, doc_path, &mut budget, 0);
        } else if kind.iter().any(|k| k == "page-list") && nav.page_list.is_empty() {
            nav.page_list = list_items(list, doc_path, &mut budget, 0);
        }
    }
    Ok(nav)
}

fn list_items(
    ol: roxmltree::Node<'_, '_>,
    doc_path: &str,
    budget: &mut Budget,
    depth: usize,
) -> Vec<TocEntry> {
    let mut out = Vec::new();
    if depth >= budget.max_depth {
        return out;
    }
    for li in ol.children().filter(|c| c.is_element() && c.tag_name().name() == "li") {
        if budget.left == 0 {
            break;
        }
        let head = li
            .children()
            .find(|c| c.is_element() && matches!(c.tag_name().name(), "a" | "span"));
        let Some(head) = head else { continue };
        budget.left -= 1;
        let label = xml::trimmed_text(head)
            .or_else(|| head.attribute("title").map(collapse))
            .unwrap_or_default();
        let href = (head.tag_name().name() == "a")
            .then(|| head.attribute("href"))
            .flatten()
            .and_then(|h| path::resolve(doc_path, h, budget.max_path_depth))
            .map(|h| h.to_url_path());
        let children = li
            .children()
            .find(|c| c.is_element() && c.tag_name().name() == "ol")
            .map(|ol| list_items(ol, doc_path, budget, depth + 1))
            .unwrap_or_default();
        out.push(TocEntry { label, href, children });
    }
    out
}

/// Parses an EPUB 2 NCX at `doc_path`.
pub(crate) fn parse_ncx(bytes: &[u8], doc_path: &str, limits: &Limits) -> Result<Nav> {
    let text = html_entities(&xml::text(bytes, Format::Epub, limits)?);
    let doc = xml::parse(&text, Format::Epub, limits)?;
    let mut budget = Budget {
        left: limits.max_toc_entries,
        max_depth: limits.max_toc_depth,
        max_path_depth: limits.max_path_depth,
    };
    let root = doc.root_element();
    let find =
        |name: &str| root.children().find(|c| c.is_element() && c.tag_name().name() == name);
    let mut nav = Nav::default();
    if let Some(map) = find("navMap") {
        nav.toc = nav_points(map, "navPoint", doc_path, &mut budget, 0);
    }
    if let Some(list) = find("pageList") {
        nav.page_list = nav_points(list, "pageTarget", doc_path, &mut budget, 0);
    }
    Ok(nav)
}

fn nav_points(
    parent: roxmltree::Node<'_, '_>,
    tag: &str,
    doc_path: &str,
    budget: &mut Budget,
    depth: usize,
) -> Vec<TocEntry> {
    let mut out = Vec::new();
    if depth >= budget.max_depth {
        return out;
    }
    for p in parent.children().filter(|c| c.is_element() && c.tag_name().name() == tag) {
        if budget.left == 0 {
            break;
        }
        budget.left -= 1;
        let label = p
            .children()
            .find(|c| c.is_element() && matches!(c.tag_name().name(), "navLabel"))
            .and_then(xml::trimmed_text)
            .unwrap_or_default();
        let href = p
            .children()
            .find(|c| c.is_element() && c.tag_name().name() == "content")
            .and_then(|c| c.attribute("src"))
            .and_then(|h| path::resolve(doc_path, h, budget.max_path_depth))
            .map(|h| h.to_url_path());
        let children = nav_points(p, tag, doc_path, budget, depth + 1);
        out.push(TocEntry { label, href, children });
    }
    out
}
