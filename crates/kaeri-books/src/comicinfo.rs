//! ComicInfo.xml, the metadata file many CBZ and CBR files carry (the Anansi schema, 2.0 and
//! 2.1). Folders read series, number and volume from it before they fall back to names, and the
//! reader takes `Manga=YesAndRightToLeft` as a direction hint.

use serde::{Deserialize, Serialize};

use crate::xml;
use crate::{Format, Limits, Result};

/// The parts of ComicInfo.xml Kaeri uses. Every field is optional; values are trimmed and empty
/// ones dropped.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ComicInfo {
    pub title: Option<String>,
    pub series: Option<String>,
    /// The chapter or issue number as written: `12`, `10.5`, `Extra`.
    pub number: Option<String>,
    pub volume: Option<String>,
    /// How many issues or chapters the series has.
    pub count: Option<u32>,
    pub summary: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u8>,
    pub day: Option<u8>,
    pub writer: Option<String>,
    pub penciller: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub tags: Option<String>,
    pub web: Option<String>,
    /// A language code such as `en` or `ja`.
    pub language: Option<String>,
    pub manga: Option<MangaFlag>,
    pub black_and_white: Option<bool>,
    pub age_rating: Option<String>,
    pub page_count: Option<u32>,
    pub pages: Vec<ComicPage>,
}

/// The `Manga` element: whether the book is manga, and whether it reads right to left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MangaFlag {
    Unknown,
    No,
    Yes,
    YesAndRightToLeft,
}

impl ComicInfo {
    /// True when the file says the book reads right to left.
    pub fn right_to_left(&self) -> bool {
        self.manga == Some(MangaFlag::YesAndRightToLeft)
    }

    /// The page entry describing the page at `index` in reading order, if the file lists one.
    pub fn page(&self, index: usize) -> Option<&ComicPage> {
        self.pages.iter().find(|p| p.image as usize == index)
    }
}

/// One `<Page>` in `<Pages>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComicPage {
    /// The page's index in reading order, from 0.
    pub image: u32,
    pub kind: Option<ComicPageType>,
    /// A two-page spread.
    pub double_page: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// The `Type` of a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComicPageType {
    FrontCover,
    InnerCover,
    Roundup,
    Story,
    Advertisement,
    Editorial,
    Letters,
    Preview,
    BackCover,
    Other,
    Deleted,
}

/// The most `<Page>` elements kept.
const MAX_PAGES: usize = 100_000;

/// Parses ComicInfo.xml. Unknown elements are ignored; a document that is not XML, or whose
/// root is not `ComicInfo`, is an error.
pub fn parse(bytes: &[u8], format: Format, limits: &Limits) -> Result<ComicInfo> {
    let text = xml::text(bytes, format, limits)?;
    let doc = xml::parse(&text, format, limits)?;
    let root = doc.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("ComicInfo") {
        return Err(crate::BookError::malformed(format, "ComicInfo.xml has another root"));
    }
    let mut info = ComicInfo::default();
    for node in root.children().filter(roxmltree::Node::is_element) {
        let value = || xml::trimmed_text(node);
        match node.tag_name().name() {
            "Title" => info.title = value(),
            "Series" => info.series = value(),
            "Number" => info.number = value(),
            "Volume" => info.volume = value(),
            "Count" => info.count = value().and_then(|v| v.parse().ok()),
            "Summary" => info.summary = value(),
            "Year" => info.year = value().and_then(|v| v.parse().ok()),
            "Month" => {
                info.month =
                    value().and_then(|v| v.parse().ok()).filter(|m| (1..=12).contains(m))
            }
            "Day" => {
                info.day = value().and_then(|v| v.parse().ok()).filter(|d| (1..=31).contains(d))
            }
            "Writer" => info.writer = value(),
            "Penciller" => info.penciller = value(),
            "Publisher" => info.publisher = value(),
            "Genre" => info.genre = value(),
            "Tags" => info.tags = value(),
            "Web" => info.web = value(),
            "LanguageISO" => info.language = value(),
            "Manga" => {
                info.manga = value().map(|v| match v.as_str() {
                    "Yes" => MangaFlag::Yes,
                    "YesAndRightToLeft" => MangaFlag::YesAndRightToLeft,
                    "No" => MangaFlag::No,
                    _ => MangaFlag::Unknown,
                })
            }
            "BlackAndWhite" => info.black_and_white = value().and_then(|v| yes_no(&v)),
            "AgeRating" => info.age_rating = value(),
            "PageCount" => info.page_count = value().and_then(|v| v.parse().ok()),
            "Pages" => {
                for page in node.children().filter(|n| n.has_tag_name("Page")).take(MAX_PAGES) {
                    if let Some(p) = parse_page(page) {
                        info.pages.push(p);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(info)
}

fn parse_page(node: roxmltree::Node<'_, '_>) -> Option<ComicPage> {
    let image = node.attribute("Image")?.trim().parse().ok()?;
    let kind = node.attribute("Type").and_then(|t| {
        Some(match t.trim() {
            "FrontCover" => ComicPageType::FrontCover,
            "InnerCover" => ComicPageType::InnerCover,
            "Roundup" => ComicPageType::Roundup,
            "Story" => ComicPageType::Story,
            "Advertisement" => ComicPageType::Advertisement,
            "Editorial" => ComicPageType::Editorial,
            "Letters" => ComicPageType::Letters,
            "Preview" => ComicPageType::Preview,
            "BackCover" => ComicPageType::BackCover,
            "Other" => ComicPageType::Other,
            "Deleted" => ComicPageType::Deleted,
            _ => return None,
        })
    });
    let num = |name| node.attribute(name).and_then(|v| v.trim().parse().ok());
    Some(ComicPage {
        image,
        kind,
        double_page: node.attribute("DoublePage").and_then(yes_no).unwrap_or(false),
        width: num("ImageWidth"),
        height: num("ImageHeight"),
    })
}

fn yes_no(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" => Some(true),
        "no" | "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_fields_folders_match_on() {
        let xml = br#"<?xml version="1.0" encoding="utf-8"?>
<ComicInfo xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <Series> The Paper Lantern Club </Series>
  <Number>10.5</Number>
  <Volume>3</Volume>
  <Count>40</Count>
  <LanguageISO>en</LanguageISO>
  <Manga>YesAndRightToLeft</Manga>
  <BlackAndWhite>Yes</BlackAndWhite>
  <Month>13</Month>
  <Pages>
    <Page Image="0" Type="FrontCover" ImageWidth="1200" ImageHeight="1800"/>
    <Page Image="5" DoublePage="True"/>
    <Page Image="x"/>
  </Pages>
</ComicInfo>"#;
        let info = parse(xml, Format::Cbz, &Limits::default()).unwrap();
        assert_eq!(info.series.as_deref(), Some("The Paper Lantern Club"));
        assert_eq!(info.number.as_deref(), Some("10.5"));
        assert_eq!(info.volume.as_deref(), Some("3"));
        assert_eq!(info.count, Some(40));
        assert_eq!(info.month, None);
        assert!(info.right_to_left());
        assert_eq!(info.black_and_white, Some(true));
        assert_eq!(info.pages.len(), 2);
        assert_eq!(info.page(0).unwrap().kind, Some(ComicPageType::FrontCover));
        assert!(info.page(5).unwrap().double_page);
    }

    #[test]
    fn another_root_is_refused() {
        assert!(parse(b"<Comic/>", Format::Cbz, &Limits::default()).is_err());
        assert!(parse(b"not xml", Format::Cbz, &Limits::default()).is_err());
    }
}
