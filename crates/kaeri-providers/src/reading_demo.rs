//! The reading demo as a book source: the two works in [`kaeri_core::reading_demo`].
//!
//! The comic's pages are read from Wikimedia Commons through a client that refuses, before it is
//! sent, any request or redirect off [`DEMO_HOSTS`]. Each page's bytes are checked against the
//! SHA-1 pinned for it, and a page that differs is not shown. Pages that passed are kept in a cache
//! folder, so the demo reads offline once it has been read, and the first pages can be fetched
//! ahead while the books page shows. The story comes with the app and never touches the network.

use std::{path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookSourceKind, BookSourceManifest, ChapterContent, PageRef, SeriesDetail, SeriesHit,
        SourceChapter,
    },
    demo::{self, DEMO_HOSTS, DEMO_SOURCE_ID, DEMO_SOURCE_NAME, DEMO_USER_AGENT},
    error::ProviderError,
    media::{Medium, ReadingFormat},
    reading_demo::{self, COMIC, COMIC_PAGES, DemoPage, DemoWork, STORY},
    traits::{BookSource, PageError},
};
use sha1::{Digest, Sha1};

/// Where the comic's host stands: Commons' own, except in a test that serves a fake.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Upload(String);

impl Upload {
    fn wikimedia() -> Self {
        Self("https://upload.wikimedia.org".into())
    }

    fn rebase(&self, url: &str) -> String {
        match url.strip_prefix(&format!("https://{}", DEMO_HOSTS[0])) {
            Some(rest) => format!("{}{rest}", self.0.trim_end_matches('/')),
            None => url.to_owned(),
        }
    }

    fn allows(&self, url: &str) -> bool {
        if *self == Self::wikimedia() {
            return demo::host_allowed(url);
        }
        url.strip_prefix(self.0.trim_end_matches('/')).is_some_and(|rest| rest.starts_with('/'))
    }
}

/// The reading demo. Cheap to clone.
#[derive(Clone)]
pub struct ReadingDemo {
    manifest: Arc<BookSourceManifest>,
    client: reqwest::Client,
    upload: Upload,
    cache: Option<PathBuf>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The SHA-1 of `bytes`, in lower-case hex.
pub fn sha1_hex(bytes: &[u8]) -> String {
    hex(&Sha1::digest(bytes))
}

impl ReadingDemo {
    /// The demo as it ships, keeping verified pages under `cache` when given.
    pub fn new(cache: Option<PathBuf>) -> Self {
        Self::build(Upload::wikimedia(), cache)
    }

    /// The demo with Commons' upload host standing at `upload`, for a test that serves its own
    /// bytes. Only with the `testing` feature, which no release build turns on.
    #[cfg(feature = "testing")]
    pub fn with_upload(upload: impl Into<String>, cache: Option<PathBuf>) -> Self {
        Self::build(Upload(upload.into()), cache)
    }

    fn build(upload: Upload, cache: Option<PathBuf>) -> Self {
        let guard = upload.clone();
        let client = kaeri_net::trusted_roots(reqwest::Client::builder())
            .user_agent(DEMO_USER_AGENT)
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 5 {
                    attempt.error("too many redirects")
                } else if guard.allows(attempt.url().as_str()) {
                    attempt.follow()
                } else {
                    attempt.error("the demo only reaches Wikimedia Commons")
                }
            }))
            .build()
            .expect("a plain client builds");
        Self {
            manifest: Arc::new(BookSourceManifest {
                id: DEMO_SOURCE_ID.into(),
                name: DEMO_SOURCE_NAME.into(),
                version: env!("CARGO_PKG_VERSION").into(),
                kind: BookSourceKind::Native,
                media: vec![Medium::Manga, Medium::Novel],
                languages: vec!["en".into()],
                allowed_hosts: vec![DEMO_HOSTS[0].to_owned()],
                capabilities: Vec::new(),
            }),
            client,
            upload,
            cache,
        }
    }

    fn page_of(&self, url: &str) -> Option<&'static DemoPage> {
        COMIC_PAGES.iter().find(|p| self.upload.rebase(p.file_url) == url)
    }

    fn cached(&self, page: &DemoPage) -> Option<PathBuf> {
        self.cache.as_ref().map(|dir| dir.join(format!("{}.jpg", page.sha1)))
    }

    /// A page's bytes: from the cache when they are there and still match the pin, else from
    /// Commons, checked, and kept.
    pub async fn page(&self, page: &DemoPage) -> Result<Vec<u8>, PageError> {
        if let Some(path) = self.cached(page)
            && let Ok(bytes) = tokio::fs::read(&path).await
        {
            if sha1_hex(&bytes) == page.sha1 {
                return Ok(bytes);
            }
            let _ = tokio::fs::remove_file(&path).await;
        }
        let url = self.upload.rebase(page.file_url);
        if !self.upload.allows(&url) {
            return Err(PageError::Failed(ProviderError::Blocked(
                "not a host the demo may reach".into(),
            )));
        }
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PageError::Failed(ProviderError::Transport(e.to_string())))?;
        let status = response.status();
        if !status.is_success() {
            return Err(PageError::Status(status.as_u16()));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| PageError::Failed(ProviderError::Transport(e.to_string())))?
            .to_vec();
        if sha1_hex(&bytes) != page.sha1 {
            return Err(PageError::Failed(ProviderError::Blocked(
                "the page is not the one the demo credits".into(),
            )));
        }
        if let Some(path) = self.cached(page) {
            if let Some(dir) = path.parent() {
                let _ = tokio::fs::create_dir_all(dir).await;
            }
            let part = path.with_extension("part");
            if tokio::fs::write(&part, &bytes).await.is_ok() {
                let _ = tokio::fs::rename(&part, &path).await;
            }
        }
        Ok(bytes)
    }

    /// Fetch the first pages ahead, as the books page shows, and say how many are ready.
    pub async fn prefetch(&self) -> usize {
        let mut ready = 0;
        for page in COMIC_PAGES.iter().take(reading_demo::PREFETCH_PAGES) {
            match self.page(page).await {
                Ok(_) => ready += 1,
                Err(e) => {
                    tracing::debug!(error = ?e, "a demo page was not fetched ahead");
                    break;
                }
            }
        }
        ready
    }

    /// Forget the pages kept, as when the demo is removed.
    pub fn clear(&self) {
        if let Some(dir) = &self.cache {
            for page in &COMIC_PAGES {
                let _ = std::fs::remove_file(dir.join(format!("{}.jpg", page.sha1)));
            }
        }
    }

    fn hit(work: &DemoWork) -> SeriesHit {
        SeriesHit {
            format: Some(if work.novel { ReadingFormat::Novel } else { ReadingFormat::Manga }),
            language: Some("en".into()),
            cover: work.cover().map(Into::into),
            // Each work is one chapter.
            amount: Some(kaeri_core::books::HitAmount::Chapters(1)),
            ..SeriesHit::new(work.id, work.series)
        }
    }
}

/// The story as an XHTML body fragment: its title and its paragraphs.
pub fn story_xhtml() -> String {
    let mut out = format!("<h1>{}</h1>", escape(STORY.chapter));
    for paragraph in reading_demo::story_paragraphs() {
        out.push_str("<p>");
        out.push_str(&escape(&paragraph.replace('\n', " ")));
        out.push_str("</p>");
    }
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn work_for(id: &str) -> Result<&'static DemoWork, ProviderError> {
    reading_demo::work(id).ok_or(ProviderError::NotFound)
}

#[async_trait]
impl BookSource for ReadingDemo {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _hints: &kaeri_core::books::BookHints,
    ) -> Result<Vec<SeriesHit>, ProviderError> {
        let query = query.trim().to_lowercase();
        Ok(reading_demo::WORKS
            .iter()
            .filter(|w| {
                query.is_empty()
                    || w.series.to_lowercase().contains(&query)
                    || w.chapter.to_lowercase().contains(&query)
            })
            .map(Self::hit)
            .collect())
    }

    async fn series(
        &self,
        id: &str,
        _hints: &kaeri_core::books::BookHints,
    ) -> Result<SeriesDetail, ProviderError> {
        let work = work_for(id)?;
        Ok(SeriesDetail {
            description: Some(work.synopsis.into()),
            cover: work.cover().map(Into::into),
            authors: vec![work.author.into()],
            format: Some(if work.novel { ReadingFormat::Novel } else { ReadingFormat::Manga }),
            reading_hint: work.reading,
            languages: vec!["en".into()],
            ..SeriesDetail::new(work.id, work.series)
        })
    }

    async fn chapters(
        &self,
        id: &str,
        _hints: &kaeri_core::books::BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        let work = work_for(id)?;
        Ok(vec![SourceChapter {
            title: Some(work.chapter.into()),
            language: Some("en".into()),
            page_count: (!work.novel).then_some(COMIC_PAGES.len() as u32),
            ..SourceChapter::new(reading_demo::CHAPTER_ID, "1")
        }])
    }

    async fn content(
        &self,
        series: &str,
        chapter: &str,
    ) -> Result<ChapterContent, ProviderError> {
        let work = work_for(series)?;
        if chapter != reading_demo::CHAPTER_ID {
            return Err(ProviderError::NotFound);
        }
        if work.novel {
            return Ok(ChapterContent::Text(story_xhtml()));
        }
        Ok(ChapterContent::Images(
            COMIC_PAGES
                .iter()
                .map(|p| PageRef {
                    width: Some(p.width),
                    height: Some(p.height),
                    ..PageRef::new(self.upload.rebase(p.file_url))
                })
                .collect(),
        ))
    }

    async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
        // Only the pinned pages: any other address is refused before a request.
        let Some(pinned) = self.page_of(&page.url) else {
            return Err(PageError::Failed(ProviderError::Blocked(
                "not a page of the demo".into(),
            )));
        };
        self.page(pinned).await
    }
}

/// Whether a series of the demo source is the comic.
pub fn is_comic(series: &str) -> bool {
    series == COMIC.id
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_core::books::BookHints;

    #[tokio::test]
    async fn both_works_list_and_open() {
        let demo = ReadingDemo::new(None);
        let all = demo.search("", &BookHints::default()).await.unwrap();
        assert_eq!(all.len(), 2);
        let story = demo.search("yuki", &BookHints::default()).await.unwrap();
        assert_eq!(story.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), [STORY.id]);
        let Ok(ChapterContent::Text(text)) = demo.content(STORY.id, "1").await else {
            panic!("the story is text")
        };
        assert!(text.starts_with("<h1>Yuki-Onna</h1><p>In a village"));
        assert!(!text.contains('\n'));
        let Ok(ChapterContent::Images(pages)) = demo.content(COMIC.id, "1").await else {
            panic!("the comic is pages")
        };
        assert_eq!(pages.len(), 3);
        assert!(pages.iter().all(|p| demo::host_allowed(&p.url)));
        assert_eq!(demo.content(COMIC.id, "2").await.unwrap_err(), ProviderError::NotFound);
    }

    #[tokio::test]
    async fn an_address_that_is_not_a_pinned_page_is_refused_before_a_request() {
        let demo = ReadingDemo::new(None);
        let other =
            PageRef::new("https://upload.wikimedia.org/wikipedia/commons/a/ab/Other.jpg");
        assert!(matches!(
            demo.fetch_page(&other).await,
            Err(PageError::Failed(ProviderError::Blocked(_)))
        ));
    }

    #[test]
    fn the_story_escapes_what_it_holds() {
        assert_eq!(escape("a < b & c"), "a &lt; b &amp; c");
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }
}
