//! The reading demo: two works their makers share, so a new install has something to read before
//! any source is added. It sits beside the watching demo in [`crate::demo`], under the same id
//! and the same rules.
//!
//! - One comic episode under CC BY 4.0. Kaeri doesn't host it: its pages are read from Wikimedia
//!   Commons, whole, as Commons holds them, and each is pinned by its SHA-1. A page whose bytes
//!   differ from the pin is not shown under this credit.
//! - One public-domain story, bundled with the app from a copy with the Project Gutenberg header,
//!   footer and notes stripped (see `assets/demo/NOTICE.md`).
//!
//! Nothing here can be changed from outside: no manifest, no store record, no setting. The only
//! hosts the comic reaches are [`crate::demo::DEMO_HOSTS`].

use crate::{
    books::{WorkId, WorkKey},
    media::Medium,
};

/// The series id of the comic within the demo source.
pub const COMIC_ID: &str = "pepper-carrot";
/// The series id of the story within the demo source.
pub const STORY_ID: &str = "kwaidan";
/// The one chapter of each.
pub const CHAPTER_ID: &str = "1";

/// One comic page, as Commons holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoPage {
    /// The file, on `upload.wikimedia.org`.
    pub file_url: &'static str,
    /// The Commons file page, which names its licence and its maker.
    pub page_url: &'static str,
    /// The SHA-1 of the file's bytes, pinned.
    pub sha1: &'static str,
    /// The file's size in bytes.
    pub size: u64,
    pub width: u32,
    pub height: u32,
}

macro_rules! page {
    ($dir:literal, $n:literal, $sha1:literal, $size:literal) => {
        DemoPage {
            file_url: concat!(
                "https://upload.wikimedia.org/wikipedia/commons/",
                $dir,
                "/Pepper%26Carrot_en_episode_01_page_0",
                $n,
                ".jpg"
            ),
            page_url: concat!(
                "https://commons.wikimedia.org/wiki/File:Pepper%26Carrot_en_episode_01_page_0",
                $n,
                ".jpg"
            ),
            sha1: $sha1,
            size: $size,
            width: 2481,
            height: 3503,
        }
    };
}

/// The comic's pages, in reading order.
pub const COMIC_PAGES: [DemoPage; 3] = [
    page!("c/ce", "1", "e0ca9f749321fbf6c550cb03bdc4ba714e76f37b", 2_200_638),
    page!("f/f9", "2", "0ea09e39cd8c86a858f934b972af62b4e358afc7", 2_211_587),
    page!("f/f8", "3", "2a32559b0835b6764bc7ef6283048c2e13cb5efd", 1_468_203),
];

/// How many pages are fetched ahead when the books page shows: all three.
pub const PREFETCH_PAGES: usize = 3;

/// One work of the demo and its credit, as the licence asks for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoWork {
    /// The series id within the demo source.
    pub id: &'static str,
    /// The series, as its maker writes it.
    pub series: &'static str,
    /// The chapter's own title.
    pub chapter: &'static str,
    pub author: &'static str,
    /// "CC BY 4.0", or `None` for a work in the public domain.
    pub licence: Option<&'static str>,
    pub licence_url: Option<&'static str>,
    /// The maker's site, or where the text came from. Linked, never read from.
    pub maker_url: &'static str,
    /// Where the work is read from: its first Commons file page, or `None` when it is bundled.
    pub file_page_url: Option<&'static str>,
    /// The credit the reader shows, in English. A front end words it from the fields above.
    pub credit: &'static str,
    /// A line written for the demo's series page.
    pub synopsis: &'static str,
    /// A novel, read as text; else a comic, read as pages.
    pub novel: bool,
}

pub const COMIC: DemoWork = DemoWork {
    id: COMIC_ID,
    series: "Pepper&Carrot",
    chapter: "The Potion of Flight",
    author: "David Revoy",
    licence: Some("CC BY 4.0"),
    licence_url: Some("https://creativecommons.org/licenses/by/4.0/"),
    maker_url: "https://www.peppercarrot.com/",
    file_page_url: Some(COMIC_PAGES[0].page_url),
    credit: "Pepper&Carrot by David Revoy. Licensed CC BY 4.0.",
    synopsis: "A young witch and her cat try a potion. Its maker shares the comic under CC BY \
4.0, and the demo reads it from Wikimedia Commons.",
    novel: false,
};

pub const STORY: DemoWork = DemoWork {
    id: STORY_ID,
    series: "Kwaidan",
    chapter: "Yuki-Onna",
    author: "Lafcadio Hearn",
    licence: None,
    licence_url: None,
    maker_url: "https://www.gutenberg.org/ebooks/1210",
    file_page_url: None,
    credit: "Kwaidan by Lafcadio Hearn. Public domain.",
    synopsis: "A woodcutter caught in a snowstorm. The story is in the public domain and comes \
with the app.",
    novel: true,
};

/// Both works, the comic first.
pub const WORKS: [DemoWork; 2] = [COMIC, STORY];

impl DemoWork {
    /// How it reads: a novel, or a comic read as pages.
    pub const fn medium(&self) -> Medium {
        if self.novel { Medium::Novel } else { Medium::Manga }
    }

    /// The format key the catalogue gives it: `manga` or `novel`.
    pub const fn format(&self) -> &'static str {
        if self.novel { "novel" } else { "manga" }
    }

    /// The work it is read under on every device: the key an unmatched series of its name and
    /// medium gets. The demo source's series are matched to it before anything is searched.
    pub fn work_id(&self) -> WorkId {
        WorkKey::local(self.medium(), self.series).id()
    }

    /// Its cover: the comic's first page, as Commons holds it and as pinned. The story has
    /// none, so the apps draw its plate.
    pub const fn cover(&self) -> Option<&'static str> {
        if self.novel { None } else { Some(COMIC_PAGES[0].file_url) }
    }

    /// Whether a search for `query` finds it: by its series, its chapter, its author, or the
    /// demo's own name. An empty query finds nothing, so a browse without words stays AniList's.
    pub fn answers_to(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        !query.is_empty()
            && [self.series, self.chapter, self.author, crate::demo::DEMO_SOURCE_NAME]
                .iter()
                .any(|name| name.to_lowercase().contains(&query))
    }
}

/// The work a demo cover's address is the cover of: only the comic's pinned first page.
pub fn cover_page(url: &str) -> Option<&'static DemoPage> {
    COMIC.cover().filter(|cover| *cover == url).map(|_| &COMIC_PAGES[0])
}

/// The work a demo series id names.
pub fn work(id: &str) -> Option<&'static DemoWork> {
    WORKS.iter().find(|w| w.id == id)
}

/// The story's text: paragraphs separated by a blank line.
pub const STORY_TEXT: &str = include_str!("../../../assets/demo/kwaidan-yuki-onna.txt");

/// The story's paragraphs.
pub fn story_paragraphs() -> impl Iterator<Item = &'static str> {
    STORY_TEXT.split("\n\n").map(str::trim).filter(|p| !p.is_empty())
}

/// Every address the comic holds, for the checks that walk them.
pub fn urls() -> Vec<&'static str> {
    COMIC_PAGES.iter().flat_map(|p| [p.file_url, p.page_url]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::host_allowed;

    #[test]
    fn every_comic_address_is_on_a_demo_host() {
        for url in urls() {
            assert!(host_allowed(url), "{url}");
        }
        for page in COMIC_PAGES {
            assert_eq!(page.sha1.len(), 40);
            assert!(page.sha1.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn the_story_is_the_text_alone() {
        assert!(!STORY_TEXT.contains("Gutenberg"));
        assert!(!STORY_TEXT.contains("[Illustration"));
        assert!(story_paragraphs().count() > 10);
        assert!(story_paragraphs().next().is_some_and(|p| p.starts_with("In a village")));
    }

    #[test]
    fn the_credit_names_the_maker_and_the_licence() {
        assert_eq!(COMIC.credit, "Pepper&Carrot by David Revoy. Licensed CC BY 4.0.");
        assert_eq!(work(COMIC_ID), Some(&COMIC));
        assert_eq!(work(STORY_ID).map(|w| w.novel), Some(true));
        assert_eq!(work("other"), None);
    }

    #[test]
    fn each_work_has_its_own_key_and_only_the_comic_a_cover() {
        assert_ne!(COMIC.work_id(), STORY.work_id());
        assert!(COMIC.work_id().key().is_some_and(|k| k.is_local()));
        assert_eq!(COMIC.work_id(), WorkKey::local(Medium::Manga, "pepper&carrot").id());
        assert_eq!(STORY.work_id(), WorkKey::local(Medium::Novel, "Kwaidan").id());
        assert_eq!(COMIC.cover(), Some(COMIC_PAGES[0].file_url));
        assert_eq!(STORY.cover(), None);
        assert_eq!(cover_page(COMIC_PAGES[0].file_url), Some(&COMIC_PAGES[0]));
        assert_eq!(cover_page(COMIC_PAGES[1].file_url), None);
    }

    #[test]
    fn a_search_finds_a_work_by_its_names_and_never_by_nothing() {
        assert!(COMIC.answers_to("pepper"));
        assert!(COMIC.answers_to("  POTION "));
        assert!(STORY.answers_to("hearn"));
        assert!(STORY.answers_to("demo") && COMIC.answers_to("demo"));
        assert!(!STORY.answers_to("pepper"));
        assert!(!COMIC.answers_to(""));
        assert!(!COMIC.answers_to("   "));
    }
}
