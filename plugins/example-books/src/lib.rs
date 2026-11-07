//! A reference book source plugin.
//!
//! This exists to be read. It uses every part of the `kaeri:books` ABI: the manifest with its
//! media and languages, the lent `fetch`, `regex-captures` and `config-get`, the title hints, both
//! kinds of chapter content, both kinds of page and the error vocabulary. Everything but `search`
//! answers from data kept in the plugin, so the shape of a real book source is visible without a
//! server.
//!
//! It talks to `books.example` for its catalogue and names page images on `pages.example`, two
//! made-up hosts that stand for your own server. Both are in `allowed-hosts`: the host fetches
//! page images through the same allowlist as everything else, so a page host the manifest leaves
//! out is a page that never loads.
//!
//! One chapter, "Extra", comes as a single archive rather than as page images, the way some
//! servers hand chapters over. Its pages are `plugin` pages: the host asks `page-bytes` for each
//! one when it wants it, and the plugin reads that page out of the archive.
//!
//! The two series are made up too: a manga, "The Lantern Cartographer", and a novel, "Saltglass
//! Letters".

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../wit/kaeri-books.wit",
        world: "book-plugin",
    });
}

use bindings::{
    exports::kaeri::books::source::{
        Chapter, Content, Guest, Manifest, Page, PageImage, PageUrl, SeriesDetail, SeriesHit,
        SourceError, TitleHints,
    },
    kaeri::books::host,
};

struct Component;

/// The catalogue's host. Declared here and enforced by the host.
const API_HOST: &str = "books.example";
/// Where page images live. The host fetches them, never this plugin.
const PAGE_HOST: &str = "pages.example";

/// The AniList id the example catalogue files its manga under. A source with an index of its
/// own looks a work up by id instead of searching for it.
const LANTERN_ANILIST: u32 = 990_001;

/// The "Extra" chapter as your server would hand it over: one CBZ, four pages. A real plugin
/// fetches it, `host::fetch` with `https://books.example/lantern/lantern-extra.cbz`; the example
/// keeps a copy so it reads without a server. Its pages are plain made-up pictures.
const EXTRA_ARCHIVE: &[u8] = include_bytes!("../lantern-extra.cbz");

const LANTERN: &str = "example:lantern-cartographer";
const SALTGLASS: &str = "example:saltglass-letters";

impl Guest for Component {
    fn describe() -> Manifest {
        Manifest {
            id: "example-books".into(),
            display_name: "Example (books)".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            allowed_hosts: vec![API_HOST.into(), PAGE_HOST.into()],
            media: vec!["manga".into(), "novel".into()],
            languages: vec!["en".into()],
            // Nothing to declare, which is the normal case.
            capabilities: vec![],
        }
    }

    fn search(query: String, hints: TitleHints) -> Result<Vec<SeriesHit>, SourceError> {
        // A catalogue keyed by AniList id answers without searching at all.
        if hints.anilist_id == Some(LANTERN_ANILIST) {
            return Ok(vec![lantern_hit()]);
        }
        if hints.medium.as_deref() == Some("novel") {
            host::log("debug", "only novels were asked for");
        }

        // A real plugin asks your server's search endpoint. Here the query is echoed back,
        // which still exercises the round trip: the request, the host's allowlist check, the
        // bytes coming back, and the lent regex reading them.
        let body = get(&format!("https://{API_HOST}/search?q={}", encode(&query)))?;
        let echoed = host::regex_captures(r#""q":\s*"([^"]*)""#, &body)
            .into_iter()
            .next()
            .and_then(|groups| groups.into_iter().nth(1))
            .ok_or_else(|| SourceError::Parse("no query echoed back".into()))?;
        Ok(vec![SeriesHit {
            id: format!("example:{echoed}"),
            title: format!("Echo of {echoed}"),
            synonyms: vec![],
            format: Some("manga".into()),
            origin: None,
            year: None,
            cover: None,
            language: Some("en".into()),
        }])
    }

    fn series(id: String, _hints: TitleHints) -> Result<SeriesDetail, SourceError> {
        match id.as_str() {
            LANTERN => Ok(SeriesDetail {
                id,
                title: "The Lantern Cartographer".into(),
                synonyms: vec!["Lantern Cartographer".into()],
                description: Some(
                    "A mapmaker charts a city that only exists after dark.".into(),
                ),
                cover: Some(format!("https://{PAGE_HOST}/lantern/cover.jpg")),
                status: Some("releasing".into()),
                authors: vec!["A. Example".into()],
                genres: vec!["Adventure".into(), "Mystery".into()],
                format: Some("manga".into()),
                origin: Some("JP".into()),
                // Pages read right to left: the reader's first guess, after the person's own.
                reading_hint: Some("rtl".into()),
                languages: vec!["en".into()],
            }),
            SALTGLASS => Ok(SeriesDetail {
                id,
                title: "Saltglass Letters".into(),
                synonyms: vec![],
                description: Some("Letters washed ashore, one chapter each.".into()),
                cover: None,
                status: Some("finished".into()),
                authors: vec!["B. Example".into()],
                genres: vec!["Drama".into()],
                format: Some("novel".into()),
                origin: Some("JP".into()),
                reading_hint: None,
                languages: vec!["en".into()],
            }),
            // `not-found` rather than an empty answer: the host does not try another source for
            // a series that is simply not here.
            _ => Err(SourceError::NotFound),
        }
    }

    fn list_chapters(id: String, _hints: TitleHints) -> Result<Vec<Chapter>, SourceError> {
        // The person's own setting, if they gave one: `[providers.plugins.settings.example-books]`
        // `group = "..."`. A plugin must work with every setting unset.
        let only_group = host::config_get("group");
        let chapters = match id.as_str() {
            LANTERN => vec![
                chapter("lantern-1", "1", Some("1"), Some("The Night Survey"), None, 18),
                // Two versions of the same chapter, from two groups. The host keeps both and
                // the person picks; read marks belong to the chapter, not to a version.
                chapter("lantern-2a", "2", Some("1"), Some("Ink and Oil"), Some("Group A"), 20),
                chapter("lantern-2b", "2", Some("1"), Some("Ink and Oil"), Some("Group B"), 20),
                // A source's own label is kept as written; the host reads the number from it.
                chapter("lantern-10-5", "10.5", Some("2"), None, None, 8),
                chapter("lantern-extra", "Extra", None, Some("Sketches"), None, 4),
            ],
            SALTGLASS => vec![
                chapter("saltglass-1", "1", None, Some("The First Bottle"), None, 0),
                chapter("saltglass-2", "2", None, Some("Low Tide"), None, 0),
            ],
            _ => return Err(SourceError::NotFound),
        };
        Ok(chapters
            .into_iter()
            .filter(|c| match (&only_group, &c.version_label) {
                (Some(group), Some(label)) => label == group,
                _ => true,
            })
            .collect())
    }

    fn chapter_content(series: String, chapter: String) -> Result<Content, SourceError> {
        match (series.as_str(), chapter.as_str()) {
            // The archive's pages, named by their entries. The host hands a name back to
            // `page-bytes` when it wants that page, one page at a time, so a whole chapter is
            // never held in memory at once.
            (LANTERN, "lantern-extra") => Ok(Content::Images(
                stored_entries(EXTRA_ARCHIVE)?
                    .into_iter()
                    .map(|(name, _)| Page {
                        image: PageImage::Plugin(name),
                        width: Some(60),
                        height: Some(90),
                    })
                    .collect(),
            )),
            (LANTERN, id) if id.starts_with("lantern-") => {
                let pages = if id == "lantern-10-5" { 8 } else { 3 };
                Ok(Content::Images(
                    (1..=pages)
                        .map(|n| Page {
                            image: PageImage::Url(PageUrl {
                                url: format!("https://{PAGE_HOST}/lantern/{id}/{n:03}.jpg"),
                                // Headers the image host needs travel with each page, so a
                                // download later sends them too.
                                headers: vec![(
                                    "referer".into(),
                                    format!("https://{API_HOST}/series/lantern-cartographer"),
                                )],
                            }),
                            width: Some(1_200),
                            height: Some(1_800),
                        })
                        .collect(),
                ))
            }
            (SALTGLASS, "saltglass-1" | "saltglass-2") => Ok(Content::Text(format!(
                // An XHTML body fragment. The host sanitises it, so a script here would never run.
                "<h2>{}</h2><p>The bottle was cold, and the letter inside was dry.</p>\
                 <p>It began, as they all did, with the tide.</p>",
                if chapter == "saltglass-1" { "The First Bottle" } else { "Low Tide" }
            ))),
            _ => Err(SourceError::NotFound),
        }
    }

    /// One page of the "Extra" archive: the entry the host names, whole. The host checks what
    /// comes back (its size, that it is an image, and how large it claims to be), so a page
    /// here is a JPEG, PNG, WebP, GIF or AVIF file as the archive holds it.
    fn page_bytes(
        series: String,
        chapter: String,
        page: String,
    ) -> Result<Vec<u8>, SourceError> {
        if (series.as_str(), chapter.as_str()) != (LANTERN, "lantern-extra") {
            // Every other page here is an address the host fetches itself.
            return Err(SourceError::NotFound);
        }
        stored_entries(EXTRA_ARCHIVE)?
            .into_iter()
            .find(|(name, _)| *name == page)
            .map(|(_, bytes)| bytes.to_vec())
            .ok_or(SourceError::NotFound)
    }
}

/// The files in a ZIP archive (a CBZ is one) whose entries are stored rather than compressed,
/// as image archives often are: the images are compressed already. Read from the central
/// directory at the end of the file. A real plugin would use a ZIP crate, which reads compressed
/// entries too.
fn stored_entries(zip: &[u8]) -> Result<Vec<(String, &[u8])>, SourceError> {
    let bad = || SourceError::Parse("the chapter archive is not a ZIP file".into());
    let u16_at = |at: usize| {
        zip.get(at..at + 2)
            .map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])))
            .ok_or_else(bad)
    };
    let u32_at = |at: usize| {
        zip.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
            .ok_or_else(bad)
    };
    // The end of central directory record: its signature, then the entry count at 10 and the
    // directory's offset at 16.
    let end = (0..zip.len().saturating_sub(21))
        .rev()
        .find(|&at| zip[at..].starts_with(b"PK\x05\x06"))
        .ok_or_else(bad)?;
    let count = u16_at(end + 10)?;
    let mut at = u32_at(end + 16)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        if !zip.get(at..).is_some_and(|rest| rest.starts_with(b"PK\x01\x02")) {
            return Err(bad());
        }
        let (method, size, local) = (u16_at(at + 10)?, u32_at(at + 20)?, u32_at(at + 42)?);
        let name_len = u16_at(at + 28)?;
        let name = zip.get(at + 46..at + 46 + name_len).ok_or_else(bad)?;
        let name = String::from_utf8_lossy(name).into_owned();
        at += 46 + name_len + u16_at(at + 30)? + u16_at(at + 32)?;
        if method != 0 {
            return Err(SourceError::Parse(format!("{name} is compressed")));
        }
        // The data follows the entry's local header, whose name and extra field can differ in
        // length from the directory's.
        let start = local + 30 + u16_at(local + 26)? + u16_at(local + 28)?;
        let bytes = zip.get(start..start + size).ok_or_else(bad)?;
        if !name.ends_with('/') {
            entries.push((name, bytes));
        }
    }
    Ok(entries)
}

fn lantern_hit() -> SeriesHit {
    SeriesHit {
        id: LANTERN.into(),
        title: "The Lantern Cartographer".into(),
        // Other titles widen the match: the host scores against every name it is given.
        synonyms: vec!["Lantern Cartographer".into()],
        format: Some("manga".into()),
        origin: Some("JP".into()),
        year: Some(2026),
        cover: Some(format!("https://{PAGE_HOST}/lantern/cover.jpg")),
        language: Some("en".into()),
    }
}

fn chapter(
    id: &str,
    number: &str,
    volume: Option<&str>,
    title: Option<&str>,
    version: Option<&str>,
    pages: u32,
) -> Chapter {
    Chapter {
        id: id.into(),
        number: number.into(),
        volume: volume.map(Into::into),
        title: title.map(Into::into),
        version_label: version.map(Into::into),
        language: Some("en".into()),
        published: Some("2026-01-02".into()),
        // Unknown for a text chapter.
        page_count: (pages > 0).then_some(pages),
    }
}

/// A GET through the host, with the host's failures translated into ours.
fn get(url: &str) -> Result<String, SourceError> {
    let response = host::fetch(&host::HttpRequest {
        method: "GET".into(),
        url: url.into(),
        headers: vec![("accept".into(), "application/json".into())],
        body: None,
    })
    .map_err(|error| match error {
        // A denial means the manifest and the code disagree: a bug in the plugin.
        host::HostError::Denied(m) => SourceError::Other(format!("denied by the host: {m}")),
        host::HostError::Timeout => SourceError::Blocked("timed out".into()),
        host::HostError::Transport(m) => SourceError::Blocked(m),
    })?;
    match response.status {
        200..=299 => String::from_utf8(response.body)
            .map_err(|_| SourceError::Parse("response was not utf-8".into())),
        404 => Err(SourceError::NotFound),
        429 | 503 => Err(SourceError::Blocked(format!("status {}", response.status))),
        status => Err(SourceError::Other(format!("status {status}"))),
    }
}

/// Percent-encode a query value: letters, digits and `-_.~` pass, everything else is escaped.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

bindings::export!(Component with_types_in bindings);
