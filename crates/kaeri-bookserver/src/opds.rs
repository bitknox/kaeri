//! OPDS 1.x catalogues, with page streaming, as a book source.
//!
//! A catalogue is a tree of Atom feeds. Series are found where the catalogue has them: a
//! navigation feed of series when the root offers one, else the catalogue's own search, with the
//! books it returns grouped by category. A series' chapters are the books of its feed. A book
//! with a page-stream link (OPDS-PSE) is read page by page; any other is downloaded whole.
//! Plain OPDS has no way to tell the server what was read.

use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use async_trait::async_trait;
use kaeri_books::opds::{Entry, Feed, FeedKind, Link, Paging};
use kaeri_core::{
    books::{
        BookFile, BookHints, BookSourceKind, BookSourceManifest, ChapterContent, PageRef,
        SeriesDetail, SeriesHit, SourceChapter,
    },
    error::ProviderError,
    media::Medium,
    traits::{BookSource, PageError},
};

use crate::{
    SignedIn,
    error::{Result, ServerError},
    http::{Auth, Http, basic},
    kept::keep_path,
    komga::encode,
    names::chapter_of,
};

/// The most pages of one feed followed, so a catalogue that never ends cannot hold a search.
const MAX_PAGES: usize = 40;
/// The largest feed document read.
const MAX_FEED_BYTES: usize = 16 * 1024 * 1024;
/// How deep a feed's elements may nest.
const MAX_DEPTH: usize = 64;

fn local<'a>(node: roxmltree::Node<'a, '_>) -> &'a str {
    node.tag_name().name()
}

fn text_of(node: roxmltree::Node<'_, '_>) -> String {
    node.text().unwrap_or_default().trim().to_owned()
}

/// Elements nested deeper than `max` anywhere in `text`.
fn too_deep(text: &str, max: usize) -> bool {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    for (at, &b) in bytes.iter().enumerate() {
        if b != b'<' {
            continue;
        }
        match bytes.get(at + 1) {
            Some(b'/') => depth = depth.saturating_sub(1),
            Some(b'!' | b'?') => {}
            _ => {
                let close = bytes[at..].iter().position(|&c| c == b'>').map(|n| at + n);
                if !close.is_some_and(|c| bytes[c - 1] == b'/') {
                    depth += 1;
                }
            }
        }
        if depth > max {
            return true;
        }
    }
    false
}

/// Whether `text` looks like an Atom feed: `<feed` within its first 2 KB. The window is
/// counted in bytes, so it may end inside a letter; it is searched as bytes for that reason.
pub fn is_feed(text: &str) -> bool {
    let head = &text.as_bytes()[..text.len().min(2048)];
    head.windows(b"<feed".len()).any(|w| w == b"<feed")
}

/// Read an Atom/OPDS document. Hrefs are made absolute against `base`.
pub fn parse(text: &str, base: &str) -> Result<Feed> {
    if text.len() > MAX_FEED_BYTES || too_deep(text, MAX_DEPTH) {
        return Err(ServerError::Parse("the feed is too large or too deeply nested".into()));
    }
    let doc =
        roxmltree::Document::parse(text).map_err(|e| ServerError::Parse(e.to_string()))?;
    let root = doc.root_element();
    if local(root) != "feed" {
        return Err(ServerError::Parse("not an Atom feed".into()));
    }
    let base = url::Url::parse(base).ok();
    let absolute = |href: &str| -> String {
        // PSE templates carry braces that Url::join would escape: keep them.
        match &base {
            Some(base) if !href.starts_with("http://") && !href.starts_with("https://") => {
                join_keeping_braces(base, href)
            }
            _ => href.to_owned(),
        }
    };
    let link = |node: roxmltree::Node<'_, '_>| -> Link {
        let attr = |name: &str| node.attribute(name).unwrap_or_default();
        let ns = |name: &str| {
            node.attributes().find(|a| a.name() == name).map(|a| a.value().to_owned())
        };
        Link {
            rel: attr("rel").to_owned(),
            href: absolute(attr("href")),
            media_type: attr("type").to_owned(),
            title: node.attribute("title").map(str::to_owned),
            pse_count: ns("count").and_then(|v| v.parse().ok()),
            pse_last_read: ns("lastRead").and_then(|v| v.parse().ok()),
        }
    };
    let mut feed = Feed::new("", "", "", FeedKind::Acquisition);
    for child in root.children().filter(roxmltree::Node::is_element) {
        match local(child) {
            "id" => feed.id = text_of(child),
            "title" => feed.title = text_of(child),
            "updated" => feed.updated = text_of(child),
            "link" => feed.links.push(link(child)),
            "author" => {
                feed.author = child
                    .children()
                    .find(|n| n.is_element() && local(*n) == "name")
                    .map(text_of);
            }
            "totalResults" => {
                let total = text_of(child).parse().unwrap_or(0);
                feed.paging =
                    Some(Paging { total_results: total, items_per_page: 0, start_index: 1 });
            }
            "entry" => {
                let mut entry = Entry::default();
                for part in child.children().filter(roxmltree::Node::is_element) {
                    match local(part) {
                        "id" => entry.id = text_of(part),
                        "title" => entry.title = text_of(part),
                        "updated" => entry.updated = text_of(part),
                        "summary" | "content" => {
                            let text = text_of(part);
                            if entry.summary.is_none() && !text.is_empty() {
                                entry.summary = Some(text);
                            }
                        }
                        "language" => entry.language = Some(text_of(part)),
                        "author" => {
                            if let Some(name) = part
                                .children()
                                .find(|n| n.is_element() && local(*n) == "name")
                                .map(text_of)
                            {
                                entry.authors.push(name);
                            }
                        }
                        "category" => {
                            let label = part
                                .attribute("label")
                                .or_else(|| part.attribute("term"))
                                .unwrap_or_default();
                            if !label.is_empty() {
                                entry.categories.push(label.to_owned());
                            }
                        }
                        "link" => entry.links.push(link(part)),
                        _ => {}
                    }
                }
                feed.entries.push(entry);
            }
            _ => {}
        }
    }
    let acquisition = feed.entries.iter().any(|e| {
        e.links.iter().any(|l| {
            l.rel.starts_with("http://opds-spec.org/acquisition")
                || l.rel.contains("opds-pse/stream")
        })
    });
    feed.kind = if acquisition { FeedKind::Acquisition } else { FeedKind::Navigation };
    Ok(feed)
}

fn join_keeping_braces(base: &url::Url, href: &str) -> String {
    // Join with the braces swapped for placeholders that survive, then swap back.
    let safe = href.replace('{', "%7B").replace('}', "%7D");
    match base.join(&safe) {
        Ok(joined) => joined.to_string().replace("%7B", "{").replace("%7D", "}"),
        Err(_) => href.to_owned(),
    }
}

fn link_with<'a>(links: &'a [Link], rel: &str) -> Option<&'a Link> {
    links.iter().find(|l| l.rel == rel)
}

/// A root entry that leads to the catalogue's feed of series.
fn is_series_index(entry: &Entry) -> bool {
    let title = entry.title.to_lowercase();
    (title.contains("series") || title.contains("serie"))
        && subsection(entry).is_some_and(|l| l.media_type.contains("kind=navigation"))
}

fn subsection(entry: &Entry) -> Option<&Link> {
    entry.links.iter().find(|l| {
        l.rel == "subsection"
            || l.rel == "http://opds-spec.org/sort/new"
            || l.rel == "alternate"
    })
}

/// The first feed of a catalogue at `address`: `Ok(None)` when it answers 401 with a Basic
/// challenge, which says it is a catalogue that wants a sign-in.
pub async fn probe(
    client: &reqwest::Client,
    address: &str,
    auth: Auth,
) -> Result<(Feed, bool)> {
    let http = Http::new(client, address, auth);
    let text = http.text("").await?;
    if !is_feed(&text) {
        return Err(ServerError::NotABookServer);
    }
    Ok((parse(&text, address)?, true))
}

/// Sign in to a catalogue: Basic credentials, tested against its first feed.
pub async fn sign_in(
    client: &reqwest::Client,
    address: &str,
    user: &str,
    password: &str,
) -> Result<SignedIn> {
    let b64 = basic(user, password);
    let (feed, _) = probe(client, address, Auth::Basic(b64.clone())).await?;
    let series = count_series(client, address, &Auth::Basic(b64.clone()), &feed).await;
    Ok(SignedIn {
        kind: crate::Kind::Opds,
        name: if feed.title.is_empty() { crate::address::host_of(address) } else { feed.title },
        version: String::new(),
        series,
        credential: format!("basic:{b64}"),
    })
}

/// How many series the catalogue lists now, read again from its root.
pub async fn count(
    client: &reqwest::Client,
    address: &str,
    auth: &Auth,
) -> Result<Option<u32>> {
    let (feed, _) = probe(client, address, auth.clone()).await?;
    Ok(count_series(client, address, auth, &feed).await)
}

async fn count_series(
    client: &reqwest::Client,
    address: &str,
    auth: &Auth,
    root: &Feed,
) -> Option<u32> {
    let source = OpdsSource::new(
        "probe",
        "probe",
        Http::new(client, address, auth.clone()),
        PathBuf::new(),
        Arc::new(AtomicBool::new(false)),
    );
    source.count_series_index(root).await
}

/// An OPDS catalogue as a book source.
pub struct OpdsSource {
    manifest: BookSourceManifest,
    http: Http,
    keep: PathBuf,
    signed_out: crate::Gate,
}

/// How a series hit's id points back into the catalogue.
enum Handle {
    /// A feed that lists the series' books.
    Feed(String),
    /// Books in the catalogue's search that share a category.
    Category(String),
    /// One book in a feed, a series of its own.
    Book { feed: String, entry: String },
}

impl Handle {
    fn parse(id: &str) -> Option<Self> {
        let (kind, rest) = id.split_once('|')?;
        match kind {
            "feed" => Some(Self::Feed(rest.to_owned())),
            "cat" => Some(Self::Category(rest.to_owned())),
            "book" => {
                let (feed, entry) = rest.split_once('|')?;
                Some(Self::Book { feed: feed.to_owned(), entry: entry.to_owned() })
            }
            _ => None,
        }
    }
}

impl OpdsSource {
    pub fn new(
        id: &str,
        name: &str,
        http: Http,
        keep: PathBuf,
        signed_out: impl Into<crate::Gate>,
    ) -> Self {
        Self {
            manifest: BookSourceManifest {
                id: id.to_owned(),
                name: name.to_owned(),
                version: String::new(),
                kind: BookSourceKind::Server,
                media: vec![Medium::Manga, Medium::Novel],
                languages: Vec::new(),
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            },
            http,
            keep,
            signed_out: signed_out.into(),
        }
    }

    fn aim(&self, error: ServerError) -> ProviderError {
        crate::note_sign_in(&self.signed_out, &error);
        error.into()
    }

    async fn feed(&self, address: &str) -> Result<Feed> {
        let text = self.http.text(address).await?;
        parse(&text, &self.http.url(address))
    }

    /// A feed and the pages after it, up to a limit.
    async fn feed_all(&self, address: &str) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        let mut next = Some(address.to_owned());
        for _ in 0..MAX_PAGES {
            let Some(at) = next.take() else { break };
            let feed = self.feed(&at).await?;
            next = link_with(&feed.links, "next").map(|l| l.href.clone());
            entries.extend(feed.entries);
        }
        Ok(entries)
    }

    /// The navigation feed of series the catalogue offers from its root, if it offers one.
    async fn series_index(&self, root: &Feed) -> Result<Option<Vec<Entry>>> {
        let entry = root.entries.iter().find(|e| is_series_index(e));
        let Some(link) = entry.and_then(subsection) else { return Ok(None) };
        Ok(Some(self.feed_all(&link.href).await?))
    }

    /// How many series the catalogue's series feed lists: the total its first page states,
    /// else the entries of a walk that reached the last page. `None` when it has no series
    /// feed, or the walk stopped short.
    async fn count_series_index(&self, root: &Feed) -> Option<u32> {
        let entry = root.entries.iter().find(|e| is_series_index(e))?;
        let mut next = Some(subsection(entry)?.href.clone());
        let mut count = 0usize;
        for _ in 0..MAX_PAGES {
            let Some(at) = next.take() else { break };
            let feed = self.feed(&at).await.ok()?;
            if let Some(total) =
                feed.paging.as_ref().map(|p| p.total_results).filter(|t| *t > 0)
            {
                return u32::try_from(total).ok();
            }
            next = link_with(&feed.links, "next").map(|l| l.href.clone());
            count += feed.entries.len();
        }
        if next.is_some() {
            return None;
        }
        u32::try_from(count).ok()
    }

    fn hit(&self, entry: &Entry, handle: &str) -> SeriesHit {
        let mut hit = SeriesHit::new(handle, entry.title.clone());
        hit.cover = entry
            .links
            .iter()
            .find(|l| {
                l.rel == kaeri_books::opds::REL_THUMBNAIL
                    || l.rel == kaeri_books::opds::REL_IMAGE
            })
            .map(|l| l.href.clone());
        hit.language.clone_from(&entry.language);
        hit
    }

    async fn books_of(&self, handle: &Handle) -> Result<Vec<Entry>> {
        match handle {
            Handle::Feed(feed) => self.feed_all(feed).await,
            Handle::Category(label) => {
                let search = self.search_entries(label).await?;
                Ok(search
                    .into_iter()
                    .filter(|e| e.categories.iter().any(|c| c == label))
                    .collect())
            }
            Handle::Book { feed, entry } => {
                Ok(self.feed_all(feed).await?.into_iter().filter(|e| &e.id == entry).collect())
            }
        }
    }

    /// The books the catalogue's own search finds.
    async fn search_entries(&self, term: &str) -> Result<Vec<Entry>> {
        let root = self.feed("").await?;
        let template = link_with(&root.links, "search")
            .map(|l| l.href.clone())
            .ok_or(ServerError::NotFound)?;
        let address = template.replace("{searchTerms}", &encode(term));
        self.feed_all(&address).await
    }

    async fn search_inner(&self, query: &str) -> Result<Vec<SeriesHit>> {
        let root = self.feed("").await?;
        let wanted = query.trim().to_lowercase();
        if let Some(index) = self.series_index(&root).await? {
            return Ok(index
                .iter()
                .filter(|e| wanted.is_empty() || e.title.to_lowercase().contains(&wanted))
                .filter_map(|e| {
                    let link = subsection(e)?;
                    Some(self.hit(e, &format!("feed|{}", link.href)))
                })
                .collect());
        }
        // No series listing: the catalogue's search, with books grouped by category.
        let entries = self.search_entries(query).await?;
        let mut seen: Vec<String> = Vec::new();
        let mut hits = Vec::new();
        for entry in &entries {
            match entry.categories.first() {
                Some(label) if !seen.contains(label) => {
                    seen.push(label.clone());
                    let mut hit = self.hit(entry, &format!("cat|{label}"));
                    hit.title.clone_from(label);
                    hits.push(hit);
                }
                Some(_) => {}
                None => {
                    let search = link_with(&root.links, "search")
                        .map(|l| l.href.replace("{searchTerms}", &encode(query)))
                        .unwrap_or_default();
                    hits.push(self.hit(entry, &format!("book|{search}|{}", entry.id)));
                }
            }
        }
        Ok(hits)
    }

    fn chapter(entry: &Entry) -> SourceChapter {
        let mut chapter = chapter_of(&entry.id, &entry.title, None);
        chapter.language.clone_from(&entry.language);
        chapter.published = crate::names::date(&entry.updated);
        chapter.page_count =
            entry.links.iter().find_map(|l| l.pse_count).and_then(crate::pages_stated);
        chapter
    }

    /// What `chapter` is made of. A whole file is downloaded only with `files`; without, such a
    /// chapter is `None`.
    async fn content_of(
        &self,
        series: &str,
        chapter: &str,
        files: bool,
    ) -> Result<Option<ChapterContent>> {
        let handle = Handle::parse(series).ok_or(ServerError::NotFound)?;
        let entries = self.books_of(&handle).await?;
        let entry = entries.iter().find(|e| e.id == chapter).ok_or(ServerError::NotFound)?;
        let file = entry.links.iter().find(|l| {
            l.rel.starts_with("http://opds-spec.org/acquisition")
                && (l.media_type.contains("epub") || l.media_type.contains("pdf"))
        });
        let stream = entry.links.iter().find(|l| l.rel == kaeri_books::opds::REL_PAGE_STREAM);
        // A stream's page count is the server's word: past what a book holds, the stream is not
        // read, as each page would become a page ref.
        let stated = stream.and_then(|s| s.pse_count);
        if file.is_none()
            && let Some(stream) = stream
            && let Some(count) = stated.and_then(crate::pages_stated)
        {
            let header: Vec<(String, String)> = self.http.auth().header().into_iter().collect();
            return Ok(Some(ChapterContent::Images(
                (0..count)
                    .map(|n| PageRef {
                        url: stream
                            .href
                            .replace("{pageNumber}", &n.to_string())
                            .replace("{maxWidth}", "2000"),
                        headers: header.clone(),
                        made: None,
                        width: None,
                        height: None,
                    })
                    .collect(),
            )));
        }
        if !files {
            return Ok(None);
        }
        let link = file
            .or_else(|| {
                entry
                    .links
                    .iter()
                    .find(|l| l.rel.starts_with("http://opds-spec.org/acquisition"))
            })
            .ok_or_else(|| match stated {
                Some(count) if count > crate::MOST_PAGES => crate::too_many_pages(count),
                _ => ServerError::NotFound,
            })?;
        let extension = extension_for(&link.media_type);
        let to =
            keep_path(&self.keep, &self.manifest.id, &entry.id, &format!("book.{extension}"));
        self.http.download(&link.href, &to, 8 * 1024 * 1024 * 1024).await?;
        Ok(Some(ChapterContent::Book(BookFile { path: to, temporary: true })))
    }
}

fn extension_for(media_type: &str) -> &'static str {
    let media_type = media_type.to_ascii_lowercase();
    if media_type.contains("epub") {
        "epub"
    } else if media_type.contains("pdf") {
        "pdf"
    } else if media_type.contains("rar") || media_type.contains("cbr") {
        "cbr"
    } else if media_type.contains("7z") {
        "cb7"
    } else {
        "cbz"
    }
}

#[async_trait]
impl BookSource for OpdsSource {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _hints: &BookHints,
    ) -> std::result::Result<Vec<SeriesHit>, ProviderError> {
        self.search_inner(query).await.map_err(|e| self.aim(e))
    }

    async fn series(
        &self,
        id: &str,
        _hints: &BookHints,
    ) -> std::result::Result<SeriesDetail, ProviderError> {
        let handle = Handle::parse(id).ok_or(ProviderError::NotFound)?;
        let books = self.books_of(&handle).await.map_err(|e| self.aim(e))?;
        let first = books.first().ok_or(ProviderError::NotFound)?;
        let title = match &handle {
            Handle::Category(label) => label.clone(),
            Handle::Feed(feed) => {
                self.feed(feed).await.map(|f| f.title).unwrap_or_else(|_| first.title.clone())
            }
            Handle::Book { .. } => first.title.clone(),
        };
        let mut detail = SeriesDetail::new(id, title);
        detail.description = books.iter().find_map(|b| b.summary.clone());
        detail.authors = first.authors.clone();
        detail.languages = first.language.iter().cloned().collect();
        detail.cover = first
            .links
            .iter()
            .find(|l| l.rel == kaeri_books::opds::REL_IMAGE)
            .map(|l| l.href.clone());
        Ok(detail)
    }

    async fn chapters(
        &self,
        id: &str,
        _hints: &BookHints,
    ) -> std::result::Result<Vec<SourceChapter>, ProviderError> {
        let handle = Handle::parse(id).ok_or(ProviderError::NotFound)?;
        let books = self.books_of(&handle).await.map_err(|e| self.aim(e))?;
        Ok(books.iter().map(Self::chapter).collect())
    }

    async fn content(
        &self,
        series: &str,
        chapter: &str,
    ) -> std::result::Result<ChapterContent, ProviderError> {
        match self.content_of(series, chapter, true).await {
            Ok(Some(content)) => Ok(content),
            Ok(None) => Err(ProviderError::NotFound),
            Err(e) => Err(self.aim(e)),
        }
    }

    async fn chapter_pages(
        &self,
        series: &str,
        chapter: &str,
    ) -> std::result::Result<Option<Vec<PageRef>>, ProviderError> {
        match self.content_of(series, chapter, false).await {
            Ok(Some(ChapterContent::Images(pages))) => Ok(Some(pages)),
            Ok(_) => Ok(None),
            Err(e) => Err(self.aim(e)),
        }
    }

    async fn health_check(&self) -> std::result::Result<(), ProviderError> {
        self.feed("").await.map(drop).map_err(|e| self.aim(e))
    }

    fn is_available(&self) -> std::result::Result<(), ProviderError> {
        crate::signed_out_check(&self.signed_out)
    }

    async fn fetch_page(&self, page: &PageRef) -> std::result::Result<Vec<u8>, PageError> {
        crate::fetch_with(&self.http, page).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:pse="http://vaemendis.net/opds-pse/ns">
 <id>urn:x:root</id><title>Shelf</title><updated>2026-01-01T00:00:00Z</updated>
 <link rel="search" href="/opds/search?q={searchTerms}" type="application/atom+xml"/>
 <entry><id>urn:x:series</id><title>All series</title><updated>2026-01-01T00:00:00Z</updated>
  <link rel="subsection" href="/opds/series" type="application/atom+xml;profile=opds-catalog;kind=navigation"/></entry>
</feed>"#;

    const BOOKS: &str = r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:pse="http://vaemendis.net/opds-pse/ns" xmlns:dc="http://purl.org/dc/terms/">
 <id>urn:x:s1</id><title>Lantern Keeper</title><updated>2026-01-01T00:00:00Z</updated>
 <link rel="next" href="?page=2" type="application/atom+xml"/>
 <entry><id>urn:x:b1</id><title>Lantern Keeper v01</title><updated>2026-01-02T00:00:00Z</updated>
  <author><name>A. Example</name></author><dc:language>en</dc:language>
  <category term="Lantern Keeper" label="Lantern Keeper"/>
  <summary>Said once.</summary>
  <link rel="http://opds-spec.org/acquisition" href="/files/b1.cbz" type="application/vnd.comicbook+zip"/>
  <link rel="http://vaemendis.net/opds-pse/stream" href="/pages/b1/{pageNumber}?maxWidth={maxWidth}" type="image/jpeg" pse:count="3" pse:lastRead="2"/></entry>
</feed>"#;

    #[test]
    fn a_navigation_feed_and_an_acquisition_feed_are_told_apart_and_hrefs_resolve() {
        let root = parse(ROOT, "http://shelf.example/opds").unwrap();
        assert_eq!(root.kind, FeedKind::Navigation);
        assert_eq!(root.title, "Shelf");
        assert_eq!(
            link_with(&root.links, "search").unwrap().href,
            "http://shelf.example/opds/search?q={searchTerms}"
        );
        let books = parse(BOOKS, "http://shelf.example/opds/series/s1").unwrap();
        assert_eq!(books.kind, FeedKind::Acquisition);
        assert_eq!(
            link_with(&books.links, "next").unwrap().href,
            "http://shelf.example/opds/series/s1?page=2"
        );
        let entry = &books.entries[0];
        assert_eq!(entry.categories, ["Lantern Keeper"]);
        assert_eq!(entry.language.as_deref(), Some("en"));
        let stream = entry.links.iter().find(|l| l.pse_count.is_some()).unwrap();
        assert_eq!((stream.pse_count, stream.pse_last_read), (Some(3), Some(2)));
        assert_eq!(
            stream.href,
            "http://shelf.example/pages/b1/{pageNumber}?maxWidth={maxWidth}"
        );
    }

    #[test]
    fn a_hostile_feed_is_refused_not_followed() {
        let deep = format!("<feed>{}{}</feed>", "<a>".repeat(200), "</a>".repeat(200));
        assert!(parse(&deep, "http://x.example/").is_err());
        assert!(parse("<html></html>", "http://x.example/").is_err());
        assert!(parse("not xml", "http://x.example/").is_err());
        assert!(!is_feed("<html><body>hello</body></html>"));
    }

    #[test]
    fn chapters_read_their_numbers_from_titles() {
        let books = parse(BOOKS, "http://shelf.example/opds/series/s1").unwrap();
        let chapter = OpdsSource::chapter(&books.entries[0]);
        assert_eq!(chapter.number_raw, "Volume 1");
        assert_eq!(chapter.page_count, Some(3));
        assert_eq!(chapter.language.as_deref(), Some("en"));
    }

    #[test]
    fn handles_round_trip() {
        assert!(matches!(Handle::parse("feed|http://x.example/a"), Some(Handle::Feed(_))));
        assert!(matches!(Handle::parse("cat|Some Name"), Some(Handle::Category(_))));
        assert!(matches!(Handle::parse("book|http://x/s|urn:b"), Some(Handle::Book { .. })));
        assert!(Handle::parse("nonsense").is_none());
    }

    /// Serve `pages` by path on a loopback address, each with the given body.
    async fn serve(pages: Vec<(&'static str, String)>) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else { return };
                let pages = pages.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let path = text.split(' ').nth(1).unwrap_or("").to_owned();
                    let body = pages.iter().find(|(p, _)| *p == path).map(|(_, b)| b.clone());
                    let response = match body {
                        Some(body) => format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/atom+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        ),
                        None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
                    };
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });
        address
    }

    fn series_page(entries: &[&str], next: Option<&str>, total: Option<u32>) -> String {
        let next = next.map_or(String::new(), |n| {
            format!(r#"<link rel="next" href="{n}" type="application/atom+xml"/>"#)
        });
        let total = total.map_or(String::new(), |t| {
            format!("<opensearch:totalResults>{t}</opensearch:totalResults>")
        });
        let entries: String = entries
            .iter()
            .map(|e| {
                format!(
                    r#"<entry><id>urn:x:{e}</id><title>{e}</title><updated>2026-01-01T00:00:00Z</updated>
  <link rel="subsection" href="/opds/s/{e}" type="application/atom+xml;profile=opds-catalog;kind=acquisition"/></entry>"#
                )
            })
            .collect();
        format!(
            r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opensearch="http://a9.com/-/spec/opensearch/1.1/">
 <id>urn:x:series</id><title>All series</title><updated>2026-01-01T00:00:00Z</updated>{total}{next}{entries}</feed>"#
        )
    }

    async fn count(pages: Vec<(&'static str, String)>) -> Option<u32> {
        let address = serve(pages).await;
        let client = reqwest::Client::new();
        let root = parse(ROOT, &format!("{address}/opds")).unwrap();
        count_series(&client, &format!("{address}/opds"), &Auth::None, &root).await
    }

    #[tokio::test]
    async fn the_series_count_is_the_feeds_stated_total_or_a_walk_that_reached_the_end() {
        let walked = count(vec![
            (
                "/opds/series",
                series_page(
                    &["Lantern Keeper", "Tide Clock"],
                    Some("/opds/series?page=2"),
                    None,
                ),
            ),
            ("/opds/series?page=2", series_page(&["Paper Moon Road"], None, None)),
        ])
        .await;
        assert_eq!(walked, Some(3));

        let stated = count(vec![(
            "/opds/series",
            series_page(&["Lantern Keeper"], Some("/opds/series?page=2"), Some(212)),
        )])
        .await;
        assert_eq!(stated, Some(212), "the stated total, with no walk");

        // A feed that never ends is not counted: a number from part of it would be wrong.
        let endless = count(vec![(
            "/opds/series",
            series_page(&["Lantern Keeper"], Some("/opds/series"), None),
        )])
        .await;
        assert_eq!(endless, None);

        // A catalogue without a series feed says nothing.
        let address = serve(Vec::new()).await;
        let bare = parse(BOOKS, &format!("{address}/opds")).unwrap();
        let none = count_series(&reqwest::Client::new(), &address, &Auth::None, &bare).await;
        assert_eq!(none, None);
    }
}
