//! The documents of an OPDS 1.2 catalogue, the feed Kaeri serves to e-readers on the local
//! network: navigation feeds (series, shelves), acquisition feeds (the books, as downloads),
//! links for the Page Streaming Extension so a reader can fetch one page image at a time, and
//! the OpenSearch description behind search.
//!
//! This module only writes documents. Serving them, the password and which books a feed lists
//! belong to the engine. Every string is escaped, so a title from a source cannot add elements
//! to a feed.

use std::fmt::Write as _;

use crate::write::{escape_attr, escape_text};

/// The media type of a navigation feed.
pub const NAVIGATION_TYPE: &str = "application/atom+xml;profile=opds-catalog;kind=navigation";
/// The media type of an acquisition feed.
pub const ACQUISITION_TYPE: &str = "application/atom+xml;profile=opds-catalog;kind=acquisition";
/// The media type of an OpenSearch description.
pub const OPENSEARCH_TYPE: &str = "application/opensearchdescription+xml";

/// A download of the book.
pub const REL_ACQUISITION: &str = "http://opds-spec.org/acquisition";
/// A download that needs no account or payment, which some readers look for.
pub const REL_OPEN_ACCESS: &str = "http://opds-spec.org/acquisition/open-access";
/// The cover.
pub const REL_IMAGE: &str = "http://opds-spec.org/image";
/// A small cover for lists.
pub const REL_THUMBNAIL: &str = "http://opds-spec.org/image/thumbnail";
/// The Page Streaming Extension's link to the book's page images.
pub const REL_PAGE_STREAM: &str = "http://vaemendis.net/opds-pse/stream";
/// A link from a navigation entry to the feed it opens.
pub const REL_SUBSECTION: &str = "subsection";

const ATOM_NS: &str = "http://www.w3.org/2005/Atom";
const OPDS_NS: &str = "http://opds-spec.org/2010/catalog";
const PSE_NS: &str = "http://vaemendis.net/opds-pse/ns";
const OPENSEARCH_NS: &str = "http://a9.com/-/spec/opensearch/1.1/";
const DC_NS: &str = "http://purl.org/dc/terms/";

/// Navigation feeds lead to other feeds; acquisition feeds list books.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedKind {
    Navigation,
    Acquisition,
}

impl FeedKind {
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Navigation => NAVIGATION_TYPE,
            Self::Acquisition => ACQUISITION_TYPE,
        }
    }
}

/// An Atom link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub rel: String,
    pub href: String,
    pub media_type: String,
    pub title: Option<String>,
    /// For a page-stream link: the number of pages.
    pub pse_count: Option<u32>,
    /// For a page-stream link: the last page read, counted from 1, so a reader resumes there.
    pub pse_last_read: Option<u32>,
}

impl Link {
    pub fn new(rel: &str, href: &str, media_type: &str) -> Self {
        Self {
            rel: rel.to_owned(),
            href: href.to_owned(),
            media_type: media_type.to_owned(),
            title: None,
            pse_count: None,
            pse_last_read: None,
        }
    }

    /// A page-stream link. `template` holds the placeholders the reader fills in:
    /// `{pageNumber}`, counted from 0, and optionally `{maxWidth}` in pixels. `media_type` is
    /// what the pages are served as, such as `image/jpeg`.
    pub fn page_stream(
        template: &str,
        media_type: &str,
        count: u32,
        last_read: Option<u32>,
    ) -> Self {
        Self {
            pse_count: Some(count),
            pse_last_read: last_read,
            ..Self::new(REL_PAGE_STREAM, template, media_type)
        }
    }

    /// A link to another feed of the catalogue.
    pub fn feed(rel: &str, href: &str, kind: FeedKind) -> Self {
        Self::new(rel, href, kind.media_type())
    }

    fn write(&self, out: &mut String, indent: &str) {
        let _ = write!(
            out,
            "{indent}<link rel=\"{}\" href=\"{}\" type=\"{}\"",
            escape_attr(&self.rel),
            escape_attr(&self.href),
            escape_attr(&self.media_type)
        );
        if let Some(t) = &self.title {
            let _ = write!(out, " title=\"{}\"", escape_attr(t));
        }
        if let Some(c) = self.pse_count {
            let _ = write!(out, " pse:count=\"{c}\"");
        }
        if let Some(l) = self.pse_last_read {
            let _ = write!(out, " pse:lastRead=\"{l}\"");
        }
        out.push_str("/>\n");
    }
}

/// One entry of a feed: a book in an acquisition feed, a section in a navigation feed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// A stable URI, such as `urn:uuid:...`, the same every time the entry is served.
    pub id: String,
    pub title: String,
    /// RFC 3339, such as `2026-11-06T10:00:00Z`.
    pub updated: String,
    pub authors: Vec<String>,
    /// Plain text.
    pub summary: Option<String>,
    /// A BCP 47 tag.
    pub language: Option<String>,
    /// Shown as categories, such as genres.
    pub categories: Vec<String>,
    pub links: Vec<Link>,
}

/// OpenSearch paging, for a feed served in pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paging {
    pub total_results: u64,
    pub items_per_page: u64,
    /// The position of this page's first entry, counted from 1.
    pub start_index: u64,
}

/// A feed document. Give it `self` and `start` links, `next` and `previous` when it is paged,
/// and a `search` link to the OpenSearch description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feed {
    pub id: String,
    pub title: String,
    /// RFC 3339.
    pub updated: String,
    pub kind: FeedKind,
    pub author: Option<String>,
    pub links: Vec<Link>,
    pub entries: Vec<Entry>,
    pub paging: Option<Paging>,
}

impl Feed {
    pub fn new(id: &str, title: &str, updated: &str, kind: FeedKind) -> Self {
        Self {
            id: id.to_owned(),
            title: title.to_owned(),
            updated: updated.to_owned(),
            kind,
            author: None,
            links: Vec::new(),
            entries: Vec::new(),
            paging: None,
        }
    }

    /// The media type to serve this feed as.
    pub fn media_type(&self) -> &'static str {
        self.kind.media_type()
    }

    /// The feed as an Atom document.
    pub fn to_xml(&self) -> String {
        let mut out = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<feed xmlns=\"{ATOM_NS}\" xmlns:opds=\"{OPDS_NS}\" \
             xmlns:pse=\"{PSE_NS}\" xmlns:opensearch=\"{OPENSEARCH_NS}\" xmlns:dcterms=\"{DC_NS}\">\n"
        );
        let _ = writeln!(out, "  <id>{}</id>", escape_text(&self.id));
        let _ = writeln!(out, "  <title>{}</title>", escape_text(&self.title));
        let _ = writeln!(out, "  <updated>{}</updated>", escape_text(&self.updated));
        if let Some(a) = &self.author {
            let _ = writeln!(out, "  <author><name>{}</name></author>", escape_text(a));
        }
        if let Some(p) = self.paging {
            let _ = writeln!(
                out,
                "  <opensearch:totalResults>{}</opensearch:totalResults>",
                p.total_results
            );
            let _ = writeln!(
                out,
                "  <opensearch:itemsPerPage>{}</opensearch:itemsPerPage>",
                p.items_per_page
            );
            let _ = writeln!(
                out,
                "  <opensearch:startIndex>{}</opensearch:startIndex>",
                p.start_index
            );
        }
        for l in &self.links {
            l.write(&mut out, "  ");
        }
        for e in &self.entries {
            write_entry(&mut out, e);
        }
        out.push_str("</feed>\n");
        out
    }
}

fn write_entry(out: &mut String, e: &Entry) {
    out.push_str("  <entry>\n");
    let _ = writeln!(out, "    <id>{}</id>", escape_text(&e.id));
    let _ = writeln!(out, "    <title>{}</title>", escape_text(&e.title));
    let _ = writeln!(out, "    <updated>{}</updated>", escape_text(&e.updated));
    for a in &e.authors {
        let _ = writeln!(out, "    <author><name>{}</name></author>", escape_text(a));
    }
    if let Some(l) = &e.language {
        let _ = writeln!(out, "    <dcterms:language>{}</dcterms:language>", escape_text(l));
    }
    for c in &e.categories {
        let _ = writeln!(out, "    <category term=\"{0}\" label=\"{0}\"/>", escape_attr(c));
    }
    if let Some(s) = &e.summary {
        let _ = writeln!(out, "    <summary type=\"text\">{}</summary>", escape_text(s));
    }
    for l in &e.links {
        l.write(out, "    ");
    }
    out.push_str("  </entry>\n");
}

/// The OpenSearch description a feed's `search` link points at. `template` holds
/// `{searchTerms}`, which the reader fills in, such as `/opds/search?q={searchTerms}`.
pub fn opensearch_description(short_name: &str, description: &str, template: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<OpenSearchDescription xmlns=\"{OPENSEARCH_NS}\">\n  \
         <ShortName>{}</ShortName>\n  <Description>{}</Description>\n  <InputEncoding>UTF-8</InputEncoding>\n  \
         <OutputEncoding>UTF-8</OutputEncoding>\n  <Url type=\"{ACQUISITION_TYPE}\" template=\"{}\"/>\n\
         </OpenSearchDescription>\n",
        escape_text(short_name),
        escape_text(description),
        escape_attr(template)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::PlainAttr;

    fn sample() -> Feed {
        let mut feed = Feed::new(
            "urn:kaeri:shelf",
            "Shelf <&>",
            "2026-11-06T10:00:00Z",
            FeedKind::Acquisition,
        );
        feed.links.push(Link::feed("self", "/opds/shelf?page=2&n=20", FeedKind::Acquisition));
        feed.links.push(Link::feed("start", "/opds", FeedKind::Navigation));
        feed.links.push(Link::new("search", "/opds/search.xml", OPENSEARCH_TYPE));
        feed.paging = Some(Paging { total_results: 41, items_per_page: 20, start_index: 21 });
        feed.entries.push(Entry {
            id: "urn:uuid:00000000-0000-5000-8000-000000000001".into(),
            title: "Quiet Hills</title><script>x</script>".into(),
            updated: "2026-11-06T10:00:00Z".into(),
            authors: vec!["A. Writer".into()],
            summary: Some("Two \"towns\" & a river".into()),
            language: Some("en".into()),
            categories: vec!["Slice \"of\" life".into()],
            links: vec![
                Link::new(REL_ACQUISITION, "/opds/book/1.cbz", "application/vnd.comicbook+zip"),
                Link::new(REL_THUMBNAIL, "/opds/book/1/cover.jpg", "image/jpeg"),
                Link::page_stream(
                    "/opds/book/1/page/{pageNumber}?width={maxWidth}",
                    "image/jpeg",
                    32,
                    Some(5),
                ),
            ],
        });
        feed
    }

    fn child<'a, 'i>(
        parent: roxmltree::Node<'a, 'i>,
        ns: &str,
        name: &str,
    ) -> roxmltree::Node<'a, 'i> {
        parent.children().find(|c| c.has_tag_name((ns, name))).unwrap()
    }

    #[test]
    fn feed_parses_and_escapes() {
        let feed = sample();
        assert_eq!(feed.media_type(), ACQUISITION_TYPE);
        let xml = feed.to_xml();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let root = doc.root_element();
        assert!(root.has_tag_name((ATOM_NS, "feed")));
        assert_eq!(child(root, ATOM_NS, "title").text(), Some("Shelf <&>"));
        assert_eq!(child(root, OPENSEARCH_NS, "totalResults").text(), Some("41"));
        assert_eq!(child(root, OPENSEARCH_NS, "startIndex").text(), Some("21"));
        let this = root.children().find(|c| c.plain_attr("rel") == Some("self")).unwrap();
        assert_eq!(this.plain_attr("href"), Some("/opds/shelf?page=2&n=20"));
        assert_eq!(this.plain_attr("type"), Some(ACQUISITION_TYPE));

        let entries: Vec<_> =
            root.children().filter(|c| c.has_tag_name((ATOM_NS, "entry"))).collect();
        assert_eq!(entries.len(), 1);
        let e = entries[0];
        assert_eq!(
            child(e, ATOM_NS, "title").text(),
            Some("Quiet Hills</title><script>x</script>")
        );
        assert!(!doc.descendants().any(|n| n.has_tag_name("script")));
        assert_eq!(child(e, ATOM_NS, "summary").text(), Some("Two \"towns\" & a river"));
        assert_eq!(child(e, DC_NS, "language").text(), Some("en"));
        assert_eq!(child(e, ATOM_NS, "category").plain_attr("term"), Some("Slice \"of\" life"));
        let stream =
            e.children().find(|c| c.plain_attr("rel") == Some(REL_PAGE_STREAM)).unwrap();
        assert_eq!(
            stream.plain_attr("href"),
            Some("/opds/book/1/page/{pageNumber}?width={maxWidth}")
        );
        assert_eq!(stream.attribute((PSE_NS, "count")), Some("32"));
        assert_eq!(stream.attribute((PSE_NS, "lastRead")), Some("5"));
    }

    #[test]
    fn navigation_feed() {
        let mut feed =
            Feed::new("urn:kaeri:root", "Kaeri", "2026-11-06T10:00:00Z", FeedKind::Navigation);
        feed.entries.push(Entry {
            id: "urn:kaeri:series:1".into(),
            title: "Paper Harbour".into(),
            updated: "2026-11-06T10:00:00Z".into(),
            links: vec![Link::feed(REL_SUBSECTION, "/opds/series/1", FeedKind::Acquisition)],
            ..Entry::default()
        });
        let xml = feed.to_xml();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let link =
            doc.descendants().find(|n| n.plain_attr("rel") == Some(REL_SUBSECTION)).unwrap();
        assert_eq!(link.plain_attr("type"), Some(ACQUISITION_TYPE));
        assert!(doc.descendants().all(|n| !n.has_tag_name((OPENSEARCH_NS, "totalResults"))));
    }

    #[test]
    fn opensearch_parses() {
        let xml = opensearch_description(
            "Kaeri",
            "Books \"here\" & <there>",
            "/opds/search?q={searchTerms}&x=1",
        );
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let root = doc.root_element();
        assert_eq!(
            child(root, OPENSEARCH_NS, "Description").text(),
            Some("Books \"here\" & <there>")
        );
        let url = child(root, OPENSEARCH_NS, "Url");
        assert_eq!(url.plain_attr("template"), Some("/opds/search?q={searchTerms}&x=1"));
        assert_eq!(url.plain_attr("type"), Some(ACQUISITION_TYPE));
    }
}
