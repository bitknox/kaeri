//! The package document (OPF): metadata, manifest, spine and rendition properties, for EPUB 2
//! and 3.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::xml::{self, PlainAttr, collapse};
use crate::{BookError, Format, Limits, Result, mime, path};

/// The parsed package document.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    /// The OPF's own path in the container.
    pub path: String,
    /// The `version` attribute: `2.0`, `3.0`, `3.3`.
    pub version: String,
    pub metadata: Metadata,
    pub manifest: Vec<ManifestItem>,
    pub spine: Vec<SpineItem>,
    /// The spine's `page-progression-direction`.
    pub direction: Option<Direction>,
    /// The package-wide rendition properties.
    pub rendition: Rendition,
    /// The cover image, as an index into the manifest.
    pub cover: Option<usize>,
    /// The EPUB 3 navigation document, as an index into the manifest.
    pub nav: Option<usize>,
    /// The EPUB 2 NCX, as an index into the manifest.
    pub ncx: Option<usize>,
    /// Manifest items that point off the book (an `http` href). They are never loaded.
    pub remote: Vec<String>,
}

/// Dublin Core metadata and the series, from EPUB 3 collections or Calibre's tags.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metadata {
    pub titles: Vec<String>,
    pub creators: Vec<Creator>,
    pub languages: Vec<String>,
    /// The identifier the package's `unique-identifier` names.
    pub identifier: Option<String>,
    /// Every identifier, the unique one included.
    pub identifiers: Vec<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub date: Option<String>,
    pub subjects: Vec<String>,
    pub series: Option<Series>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Creator {
    pub name: String,
    /// A MARC relator code such as `aut` or `ill`, when given.
    pub role: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Series {
    pub name: String,
    /// The book's place in the series as written: `3`, `2.5`.
    pub position: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestItem {
    pub id: String,
    /// The normalised path in the container.
    pub path: String,
    pub media_type: String,
    pub properties: Vec<String>,
    pub fallback: Option<String>,
}

impl ManifestItem {
    pub fn has_property(&self, p: &str) -> bool {
        self.properties.iter().any(|x| x == p)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpineItem {
    /// An index into the manifest.
    pub item: usize,
    /// `linear="no"` items (notes, pop-ups) are reachable by links but not in the reading flow.
    pub linear: bool,
    /// A per-item `rendition:layout-*` override.
    pub layout: Option<Layout>,
    /// `page-spread-left`, `page-spread-right` or `rendition:page-spread-center`.
    pub spread: Option<PageSpread>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layout {
    Reflowable,
    /// Fixed layout: each spine item is one page of a set size.
    PrePaginated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageSpread {
    Left,
    Right,
    Center,
}

/// The package-wide `rendition:*` metadata of EPUB 3 fixed layout.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendition {
    pub layout: Option<Layout>,
    /// `rendition:spread`: `none`, `landscape`, `both`, `auto`.
    pub spread: Option<String>,
    /// `rendition:orientation`: `auto`, `landscape`, `portrait`.
    pub orientation: Option<String>,
}

/// The most manifest and spine items kept.
const MAX_ITEMS: usize = 50_000;

/// Parses the OPF at `opf_path`.
pub(crate) fn parse(bytes: &[u8], opf_path: &str, limits: &Limits) -> Result<Package> {
    let bad = |why: &str| BookError::malformed(Format::Epub, why);
    let text = xml::text(bytes, Format::Epub, limits)?;
    let doc = xml::parse(&text, Format::Epub, limits)?;
    let root = doc.root_element();
    if root.tag_name().name() != "package" {
        return Err(bad("the package document's root is not <package>"));
    }
    let version = root.plain_attr("version").unwrap_or("2.0").trim().to_owned();
    let unique_id = root.plain_attr("unique-identifier");
    let child =
        |name: &str| root.children().find(|n| n.is_element() && n.tag_name().name() == name);

    let mut manifest = Vec::new();
    let mut remote = Vec::new();
    let mut by_id: HashMap<String, usize> = HashMap::new();
    if let Some(m) = child("manifest") {
        for item in m.children().filter(|n| n.has_tag_name_local("item")).take(MAX_ITEMS) {
            let (Some(id), Some(href)) = (item.plain_attr("id"), item.plain_attr("href"))
            else {
                continue;
            };
            let Some(resolved) = path::resolve(opf_path, href, limits.max_path_depth) else {
                if href.contains(':') {
                    remote.push(href.to_owned());
                }
                continue;
            };
            let media_type = item
                .plain_attr("media-type")
                .map(|t| t.trim().to_ascii_lowercase())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| mime::guess(&resolved.path).to_owned());
            by_id.entry(id.to_owned()).or_insert(manifest.len());
            manifest.push(ManifestItem {
                id: id.to_owned(),
                path: resolved.path,
                media_type,
                properties: words(item.plain_attr("properties")),
                fallback: item.plain_attr("fallback").map(str::to_owned),
            });
        }
    }
    if manifest.is_empty() {
        return Err(bad("the manifest is empty"));
    }

    let spine_node = child("spine").ok_or_else(|| bad("there is no spine"))?;
    let direction = match spine_node.plain_attr("page-progression-direction") {
        Some("rtl") => Some(Direction::Rtl),
        Some("ltr") => Some(Direction::Ltr),
        _ => None,
    };
    let mut spine = Vec::new();
    for itemref in
        spine_node.children().filter(|n| n.has_tag_name_local("itemref")).take(MAX_ITEMS)
    {
        let Some(&item) = itemref.plain_attr("idref").and_then(|id| by_id.get(id)) else {
            continue;
        };
        let props = words(itemref.plain_attr("properties"));
        let has = |p: &str| props.iter().any(|x| x == p);
        let layout = if has("rendition:layout-pre-paginated") {
            Some(Layout::PrePaginated)
        } else if has("rendition:layout-reflowable") {
            Some(Layout::Reflowable)
        } else {
            None
        };
        let spread = if has("page-spread-left") || has("rendition:page-spread-left") {
            Some(PageSpread::Left)
        } else if has("page-spread-right") || has("rendition:page-spread-right") {
            Some(PageSpread::Right)
        } else if has("rendition:page-spread-center") || has("page-spread-center") {
            Some(PageSpread::Center)
        } else {
            None
        };
        spine.push(SpineItem {
            item,
            linear: itemref.plain_attr("linear").map(str::trim) != Some("no"),
            layout,
            spread,
        });
    }
    if spine.is_empty() {
        return Err(bad("the spine is empty"));
    }
    let ncx = spine_node
        .plain_attr("toc")
        .and_then(|id| by_id.get(id).copied())
        .or_else(|| manifest.iter().position(|m| m.media_type == "application/x-dtbncx+xml"));
    let nav = manifest.iter().position(|m| m.has_property("nav"));

    let (metadata, rendition, cover_id) = match child("metadata") {
        Some(m) => metadata(m, unique_id),
        None => (Metadata::default(), Rendition::default(), None),
    };
    let cover = manifest
        .iter()
        .position(|m| m.has_property("cover-image"))
        .or_else(|| cover_id.and_then(|id| by_id.get(&id).copied()))
        .filter(|&i| mime::is_image(&manifest[i].media_type));

    Ok(Package {
        path: opf_path.to_owned(),
        version,
        metadata,
        manifest,
        spine,
        direction,
        rendition,
        cover,
        nav,
        ncx,
        remote,
    })
}

fn words(s: Option<&str>) -> Vec<String> {
    s.map(|s| s.split_whitespace().map(str::to_owned).collect()).unwrap_or_default()
}

trait LocalName {
    fn has_tag_name_local(&self, name: &str) -> bool;
}

impl LocalName for roxmltree::Node<'_, '_> {
    fn has_tag_name_local(&self, name: &str) -> bool {
        self.is_element() && self.tag_name().name() == name
    }
}

/// Reads `<metadata>`. Returns the metadata, the rendition properties and the EPUB 2 cover id
/// from `<meta name="cover">`.
fn metadata(
    node: roxmltree::Node<'_, '_>,
    unique_id: Option<&str>,
) -> (Metadata, Rendition, Option<String>) {
    let mut md = Metadata::default();
    let mut rendition = Rendition::default();
    let mut cover = None;
    // EPUB 3 refinements: `<meta refines="#id" property="...">value</meta>`.
    let mut refines: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut calibre_series = None;
    let mut calibre_index = None;
    let mut collections: Vec<(Option<String>, String)> = Vec::new();

    for n in node.children().filter(roxmltree::Node::is_element) {
        if n.tag_name().name() == "meta"
            && let (Some(target), Some(prop)) =
                (n.plain_attr("refines"), n.plain_attr("property"))
        {
            let value = xml::trimmed_text(n).unwrap_or_default();
            refines
                .entry(target.trim_start_matches('#').to_owned())
                .or_default()
                .push((prop.to_owned(), value));
        }
    }
    let refined = |id: Option<&str>, prop: &str| -> Option<String> {
        refines.get(id?)?.iter().find(|(p, _)| p == prop).map(|(_, v)| v.clone())
    };

    for n in node.children().filter(roxmltree::Node::is_element) {
        let text = || xml::trimmed_text(n);
        match n.tag_name().name() {
            "title" => md.titles.extend(text()),
            "creator" => {
                if let Some(name) = text() {
                    // EPUB 2 puts the role on the element; EPUB 3 refines it.
                    let role = n
                        .attributes()
                        .find(|a| a.name() == "role")
                        .map(|a| a.value().to_owned())
                        .or_else(|| refined(n.plain_attr("id"), "role"));
                    md.creators.push(Creator { name, role });
                }
            }
            "language" => md.languages.extend(text()),
            "identifier" => {
                if let Some(v) = text() {
                    if unique_id.is_some() && n.plain_attr("id") == unique_id {
                        md.identifier = Some(v.clone());
                    }
                    md.identifiers.push(v);
                }
            }
            "publisher" => md.publisher = md.publisher.take().or_else(text),
            "description" => md.description = md.description.take().or_else(text),
            "date" => md.date = md.date.take().or_else(text),
            "subject" => md.subjects.extend(text()),
            "meta" => {
                if let Some(prop) = n.plain_attr("property") {
                    if n.plain_attr("refines").is_some() {
                        continue;
                    }
                    let value = text();
                    match prop {
                        "rendition:layout" => {
                            rendition.layout = match value.as_deref() {
                                Some("pre-paginated") => Some(Layout::PrePaginated),
                                Some("reflowable") => Some(Layout::Reflowable),
                                _ => None,
                            }
                        }
                        "rendition:spread" => rendition.spread = value,
                        "rendition:orientation" => rendition.orientation = value,
                        "belongs-to-collection" => {
                            if let Some(name) = value {
                                collections.push((n.plain_attr("id").map(str::to_owned), name));
                            }
                        }
                        _ => {}
                    }
                } else if let (Some(name), Some(content)) =
                    (n.plain_attr("name"), n.plain_attr("content"))
                {
                    match name {
                        "cover" => cover = Some(content.trim().to_owned()),
                        "calibre:series" => calibre_series = Some(collapse(content)),
                        "calibre:series_index" => calibre_index = Some(collapse(content)),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if md.identifier.is_none() {
        md.identifier = md.identifiers.first().cloned();
    }
    // An EPUB 3 collection typed `series` wins; an untyped one counts; Calibre's tags come last.
    let series = collections
        .iter()
        .find(|(id, _)| refined(id.as_deref(), "collection-type").as_deref() == Some("series"))
        .or_else(|| {
            collections
                .iter()
                .find(|(id, _)| refined(id.as_deref(), "collection-type").is_none())
        })
        .map(|(id, name)| Series {
            name: name.clone(),
            position: refined(id.as_deref(), "group-position"),
        })
        .or_else(|| {
            calibre_series
                .filter(|s| !s.is_empty())
                .map(|name| Series { name, position: calibre_index.filter(|s| !s.is_empty()) })
        });
    md.series = series;
    (md, rendition, cover)
}
