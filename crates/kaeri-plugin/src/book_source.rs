//! A book plugin, as an ordinary [`BookSource`].
//!
//! The book world's counterpart of [`crate::provider`], and as boring: the conversions below are
//! mechanical, with the same two rules. `not-found` stays `not-found`, so a missing chapter never
//! walks every source, and a guest that traps or overruns becomes `Parse`, so a broken plugin
//! counts against its health and fails over.
//!
//! Open sets arrive as strings and are read here with the core's own parsers. A value this
//! version does not know is kept where the core keeps one (`Other`) and dropped where it has
//! nowhere to go (a medium, a reading hint).

use kaeri_core::{
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, MadePage, PageRef,
        ReadingHint, SeriesDetail, SeriesHit, SeriesStatus, SourceChapter,
    },
    error::ProviderError,
    media::{Medium, Origin, ReadingFormat},
    traits::{BookSource, PageError},
};

use crate::{
    books::{self, LoadedBookPlugin, SourceError},
    engine::PluginError,
    host::{self, Capabilities, Reach},
};

/// Wraps a loaded book component as a [`BookSource`].
pub struct WasmBookSource {
    plugin: LoadedBookPlugin,
    manifest: BookSourceManifest,
}

impl WasmBookSource {
    pub fn new(plugin: LoadedBookPlugin) -> Self {
        let guest = plugin.manifest();
        let mut media: Vec<Medium> = guest
            .media
            .iter()
            .filter_map(|m| Medium::parse(m))
            .filter(|m| m.is_reading())
            .collect();
        media.dedup();
        let manifest = BookSourceManifest {
            id: guest.id.clone(),
            name: guest.display_name.clone(),
            version: guest.version.clone(),
            kind: BookSourceKind::Plugin,
            media,
            languages: guest.languages.clone(),
            allowed_hosts: guest.allowed_hosts.clone(),
            capabilities: guest.capabilities.iter().map(|c| c.trim().to_owned()).collect(),
        };
        Self { plugin, manifest }
    }

    pub fn plugin(&self) -> &LoadedBookPlugin {
        &self.plugin
    }

    /// One page image the plugin named: fetched through its own `allowed-hosts` with the page's
    /// headers, or made by the plugin itself and checked. The bytes come back to the host only.
    pub async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, ProviderError> {
        match <Self as BookSource>::fetch_page(self, page).await {
            Ok(bytes) => Ok(bytes),
            Err(PageError::Status(404)) => Err(ProviderError::NotFound),
            Err(PageError::Status(status)) => {
                Err(ProviderError::Blocked(format!("page answered {status}")))
            }
            Err(PageError::Failed(error)) => Err(error),
            Err(PageError::Unsupported) => Err(ProviderError::Other("no such page".into())),
        }
    }

    /// A page the plugin makes itself, from `page-bytes`, or `None` for one the host fetches. Its
    /// errors are the plugin's errors, so they fail over and count against its health as any
    /// other call's do.
    async fn made_page(
        &self,
        made: &MadePage,
        url: &str,
    ) -> Result<Option<Vec<u8>>, PageError> {
        let (lent, reach) = self.lend();
        flatten(
            self.plugin.page_bytes_with(lent, &made.series, &made.chapter, url).await,
            &reach,
        )
        .map_err(PageError::Failed)
    }

    /// Fresh capabilities for one call, and the handle that tells afterwards whether any of its
    /// requests got out.
    fn lend(&self) -> (Capabilities, Reach) {
        let lent = self.plugin.capabilities();
        let reach = lent.reach();
        (lent, reach)
    }
}

#[async_trait::async_trait]
impl BookSource for WasmBookSource {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        hints: &BookHints,
    ) -> Result<Vec<SeriesHit>, ProviderError> {
        let (lent, reach) = self.lend();
        let hits =
            flatten(self.plugin.search_with(lent, query, &hints_to(hints)).await, &reach)?;
        Ok(hits.into_iter().map(hit_from).collect())
    }

    async fn series(&self, id: &str, hints: &BookHints) -> Result<SeriesDetail, ProviderError> {
        let (lent, reach) = self.lend();
        flatten(self.plugin.series_with(lent, id, &hints_to(hints)).await, &reach)
            .map(detail_from)
    }

    async fn chapters(
        &self,
        id: &str,
        hints: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        let (lent, reach) = self.lend();
        let chapters =
            flatten(self.plugin.list_chapters_with(lent, id, &hints_to(hints)).await, &reach)?;
        Ok(chapters.into_iter().map(chapter_from).collect())
    }

    async fn content(
        &self,
        series: &str,
        chapter: &str,
    ) -> Result<ChapterContent, ProviderError> {
        let (lent, reach) = self.lend();
        let content =
            flatten(self.plugin.chapter_content_with(lent, series, chapter).await, &reach)?;
        // Only a plugin built for `book-plugin-with-pages` is asked for its pages.
        let made = self.plugin.makes_pages().then(|| MadePage::new(series, chapter));
        Ok(content_from(content, made))
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        // `describe`, which needs no network, as for show sources.
        self.plugin.describe().await.map(|_| ()).map_err(|e: PluginError| e.as_provider_error())
    }

    async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
        if let Some(made) = &page.made
            && let Some(bytes) = self.made_page(made, &page.url).await?
        {
            return Ok(bytes);
        }
        let response =
            self.plugin.fetch_page(&page.url, &page.headers).await.map_err(|e| match e {
                host::HostError::Denied(reason) => {
                    PageError::Failed(ProviderError::Blocked(reason))
                }
                // Kept as the network's, so a page cut off on the way is tried again and one
                // fetched with no network reads as offline.
                host::HostError::Transport(said) => {
                    PageError::Failed(ProviderError::Transport(said))
                }
                other => PageError::Failed(ProviderError::Other(other.to_string())),
            })?;
        match response.status {
            200..=299 => Ok(response.body),
            status => Err(PageError::Status(status)),
        }
    }
}

fn from_guest(error: SourceError) -> ProviderError {
    match error {
        SourceError::NotFound => ProviderError::NotFound,
        SourceError::Blocked(reason) => ProviderError::Blocked(reason),
        SourceError::Parse(reason) => ProviderError::Parse(reason),
        SourceError::Other(reason) => ProviderError::Other(reason),
    }
}

/// The plugin's answer in the provider vocabulary. When none of the call's requests reached a
/// server, the network failed, whatever the plugin made of it: a plugin sees only a failed fetch
/// and most answer that with `other`, which would read as the source being down.
fn flatten<T>(
    result: Result<Result<T, SourceError>, PluginError>,
    reach: &Reach,
) -> Result<T, ProviderError> {
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) if reach.no_network() => {
            Err(ProviderError::no_network("no request left the device"))
        }
        Ok(Err(_)) if reach.never_reached() => {
            Err(ProviderError::Transport("no request reached the source".into()))
        }
        Ok(Err(guest)) => Err(from_guest(guest)),
        Err(host) => Err(host.as_provider_error()),
    }
}

fn hints_to(hints: &BookHints) -> books::TitleHints {
    books::TitleHints {
        anilist_id: hints.anilist_id.map(|id| id.get()),
        mal_id: hints.mal_id.map(|id| id.get()),
        romaji: hints.romaji.clone(),
        english: hints.english.clone(),
        native: hints.native.clone(),
        synonyms: hints.synonyms.clone(),
        medium: hints.medium.map(|m| m.as_str().to_owned()),
    }
}

/// An empty string is no value.
fn some(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

fn hit_from(hit: books::SeriesHit) -> SeriesHit {
    SeriesHit {
        id: hit.id,
        title: hit.title,
        synonyms: hit.synonyms,
        format: hit.format.as_deref().and_then(ReadingFormat::parse),
        origin: hit.origin.as_deref().and_then(Origin::parse),
        year: hit.year,
        cover: some(hit.cover),
        language: some(hit.language),
        amount: None,
    }
}

fn detail_from(detail: books::SeriesDetail) -> SeriesDetail {
    SeriesDetail {
        id: detail.id,
        title: detail.title,
        synonyms: detail.synonyms,
        description: some(detail.description),
        cover: some(detail.cover),
        status: detail.status.as_deref().and_then(SeriesStatus::parse),
        authors: detail.authors,
        genres: detail.genres,
        format: detail.format.as_deref().and_then(ReadingFormat::parse),
        origin: detail.origin.as_deref().and_then(Origin::parse),
        reading_hint: detail.reading_hint.as_deref().and_then(ReadingHint::parse),
        languages: detail.languages,
    }
}

fn chapter_from(chapter: books::Chapter) -> SourceChapter {
    let mut out = SourceChapter {
        volume: some(chapter.volume),
        title: some(chapter.title),
        version_label: some(chapter.version_label),
        language: some(chapter.language),
        page_count: chapter.page_count,
        ..SourceChapter::new(chapter.id, chapter.number)
    };
    // RFC 3339, a date and time, or a bare date, which stays that day wherever the person is.
    out.dated(chapter.published.as_deref());
    out
}

fn content_from(content: books::Content, made: Option<MadePage>) -> ChapterContent {
    match content {
        books::Content::Images(pages) => ChapterContent::Images(
            pages
                .into_iter()
                .map(|page| PageRef {
                    url: page.url,
                    headers: page.headers,
                    width: page.width,
                    height: page.height,
                    made: made.clone(),
                })
                .collect(),
        ),
        books::Content::Text(text) => ChapterContent::Text(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_survives_the_conversion() {
        let mapped = from_guest(SourceError::NotFound);
        assert_eq!(mapped, ProviderError::NotFound);
        assert!(!mapped.should_failover(), "a missing chapter must not fail over");
        assert!(from_guest(SourceError::Parse("changed".into())).should_failover());
    }

    /// Offline, a plugin sees only failed fetches and answers `other`; the host knows no request
    /// got out, so the answer reads as the network, never as the source being down.
    #[tokio::test]
    async fn a_call_no_request_got_out_of_is_the_network() {
        let lent = host::Capabilities::new(
            "quiet-books",
            vec!["books.example".into()],
            crate::sandbox::Limits::default(),
            None,
        );
        let reach = lent.reach();
        let request = |url: &str| host::Request {
            method: "GET".into(),
            url: url.into(),
            headers: Vec::new(),
            body: None,
        };
        let failed = lent.fetch(request("https://books.example/list")).await;
        assert!(matches!(failed, Err(host::HostError::Transport(_))), "{failed:?}");
        let answer: Result<Result<(), SourceError>, PluginError> =
            Ok(Err(SourceError::Other("fetch failed".into())));
        assert!(matches!(flatten(answer, &reach), Err(ProviderError::Transport(_))));

        // A denied request is the plugin's doing, not the network's: its answer stands.
        let denied = Capabilities::new(
            "quiet-books",
            vec!["books.example".into()],
            crate::sandbox::Limits::default(),
            None,
        );
        let refused = denied.reach();
        let _ = denied.fetch(request("https://elsewhere.example/")).await;
        let answer: Result<Result<(), SourceError>, PluginError> =
            Ok(Err(SourceError::Other("denied".into())));
        assert_eq!(flatten(answer, &refused), Err(ProviderError::Other("denied".into())));
        // With no request asked, the plugin's own answer stands too.
        let answer: Result<Result<(), SourceError>, PluginError> =
            Ok(Err(SourceError::Other("broken".into())));
        assert_eq!(
            flatten(answer, &Reach::default()),
            Err(ProviderError::Other("broken".into()))
        );
    }

    #[test]
    fn a_trapped_or_stopped_plugin_fails_over() {
        let stopped: Result<Result<(), SourceError>, PluginError> =
            Err(PluginError::Deadline {
                plugin: "p".into(),
                limit: std::time::Duration::from_secs(1),
            });
        assert!(flatten(stopped, &Reach::default()).unwrap_err().should_failover());
    }

    #[test]
    fn a_chapter_date_is_a_day_or_a_moment() {
        let dated = |text: &str| {
            let chapter = chapter_from(books::Chapter {
                id: "c-1".into(),
                number: "1".into(),
                volume: None,
                title: None,
                version_label: None,
                language: None,
                published: Some(text.into()),
                page_count: None,
            });
            (chapter.published, chapter.published_day)
        };
        let day = Some(jiff::civil::date(2026, 1, 2));
        // A bare date is that day wherever the person is, not the evening before west of UTC.
        assert_eq!(dated("2026-01-02"), (Some(1_767_312_000), day));
        assert_eq!(dated("2026-01-02T00:00:00Z"), (Some(1_767_312_000), day));
        // A moment is only a moment, read where the person is.
        assert_eq!(dated("2026-01-02T01:00:00+01:00"), (Some(1_767_312_000), None));
        assert_eq!(dated("last tuesday"), (None, None));
    }

    #[test]
    fn a_chapter_keeps_its_label_and_reads_its_number() {
        let chapter = chapter_from(books::Chapter {
            id: "c-105".into(),
            number: "10.5".into(),
            volume: Some("2".into()),
            title: Some(" ".into()),
            version_label: None,
            language: Some("en".into()),
            published: Some("2026-01-02".into()),
            page_count: Some(18),
        });
        assert_eq!(chapter.number_raw, "10.5");
        assert_eq!(chapter.number.map(|n| n.to_string()).as_deref(), Some("10.5"));
        assert_eq!(chapter.title, None, "a blank title is none");
        assert_eq!(chapter.volume.as_deref(), Some("2"));
        assert_eq!(chapter.page_count, Some(18));
    }

    #[test]
    fn open_sets_are_read_with_the_core_s_parsers() {
        let detail = detail_from(books::SeriesDetail {
            id: "s".into(),
            title: "S".into(),
            synonyms: Vec::new(),
            description: None,
            cover: None,
            status: Some("finished".into()),
            authors: Vec::new(),
            genres: Vec::new(),
            format: Some("webtoon".into()),
            origin: Some("KR".into()),
            reading_hint: Some("sideways".into()),
            languages: vec!["en".into()],
        });
        assert_eq!(detail.status, Some(SeriesStatus::Finished));
        assert_eq!(detail.format, Some(ReadingFormat::Webtoon));
        assert_eq!(detail.origin, Some(Origin::Korea));
        assert_eq!(detail.reading_hint, None, "a hint this version does not know is no hint");
    }
}
