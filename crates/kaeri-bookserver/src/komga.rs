//! Komga: its REST API, as a book source.
//!
//! Sign-in is HTTP Basic against `users/me`. A Komga new enough to hand out a session token is
//! asked for one (`X-Auth-Token` with an empty value) and the token is what is kept; an older
//! one gets the Basic credential kept instead. Either way the password itself is not written
//! anywhere readable: what is kept goes to the secure store, never the database.
//!
//! A session token lapses when the server ends the session, and nothing kept can start a new
//! one without the password. A refused sign-in signs the source out, once, and it waits for
//! the person to sign in again.

use std::path::PathBuf;

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookFile, BookHints, BookSourceKind, BookSourceManifest, ChapterContent, PageRef,
        ReadingHint, SeriesDetail, SeriesHit, SeriesStatus, SourceChapter,
    },
    error::ProviderError,
    media::{Medium, ReadingFormat},
    traits::{BookSource, PageError},
};
use serde::Deserialize;

use crate::{
    Gate, SignedIn,
    error::{Result, ServerError},
    http::{Auth, Http, basic},
    kept::keep_path,
    names::chapter_of,
};

/// What a sign-in kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    Basic(String),
    Token(String),
}

impl Credential {
    pub fn parse(kept: &str) -> Option<Self> {
        match kept.split_once(':')? {
            ("basic", b64) => Some(Self::Basic(b64.to_owned())),
            ("token", token) => Some(Self::Token(token.to_owned())),
            _ => None,
        }
    }

    pub fn kept(&self) -> String {
        match self {
            Self::Basic(b64) => format!("basic:{b64}"),
            Self::Token(token) => format!("token:{token}"),
        }
    }

    pub fn auth(&self) -> Auth {
        match self {
            Self::Basic(b64) => Auth::Basic(b64.clone()),
            Self::Token(token) => Auth::Header("X-Auth-Token", token.clone()),
        }
    }
}

#[derive(Deserialize)]
struct Page<T> {
    #[serde(default = "Vec::new")]
    content: Vec<T>,
    #[serde(default, rename = "totalElements")]
    total: u32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Authors {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SeriesMeta {
    status: Option<String>,
    title: String,
    summary: String,
    #[serde(rename = "readingDirection")]
    reading_direction: String,
    language: String,
    genres: Vec<String>,
    #[serde(rename = "alternateTitles")]
    alternate_titles: Vec<AlternateTitle>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AlternateTitle {
    title: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BooksMeta {
    authors: Vec<Authors>,
    summary: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Series {
    id: String,
    name: String,
    metadata: SeriesMeta,
    #[serde(rename = "booksMetadata")]
    books_metadata: BooksMeta,
    /// How many books (files) the series holds; Komga's books are most often volumes.
    #[serde(rename = "booksCount")]
    books_count: u32,
}

impl Series {
    fn title(&self) -> &str {
        if self.metadata.title.is_empty() { &self.name } else { &self.metadata.title }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BookMeta {
    title: String,
    number: String,
    summary: String,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Media {
    status: String,
    #[serde(rename = "mediaType")]
    media_type: String,
    #[serde(rename = "pagesCount")]
    pages: u32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Book {
    id: String,
    name: String,
    media: Media,
    metadata: BookMeta,
    #[serde(rename = "sizeBytes")]
    size: u64,
}

#[derive(Deserialize)]
struct PageInfo {
    #[serde(default)]
    number: u32,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

/// Sign in and describe the server. `address` is the server's root.
pub async fn sign_in(
    client: &reqwest::Client,
    address: &str,
    user: &str,
    password: &str,
) -> Result<SignedIn> {
    let basic = basic(user, password);
    let http = Http::new(client, address, Auth::Basic(basic.clone()));
    // Asking for a token with an empty header is how Komga is told to hand one out.
    let request =
        http.request(reqwest::Method::GET, "api/v1/users/me").header("X-Auth-Token", "");
    let response = http.send(request).await?;
    let token = response
        .headers()
        .get("x-auth-token")
        .and_then(|v| v.to_str().ok())
        .filter(|t| !t.is_empty())
        .map(str::to_owned);
    // The answer must look like Komga's user, or this is some other server.
    let me: serde_json::Value = crate::http::read_json(response).await?;
    if me.get("roles").is_none() {
        return Err(ServerError::Parse("not a Komga user".into()));
    }
    let credential = match token {
        Some(token) => Credential::Token(token),
        None => Credential::Basic(basic),
    };
    let http = Http::new(client, address, credential.auth());
    let version = http
        .json::<serde_json::Value>("actuator/info")
        .await
        .ok()
        .and_then(|v| v.pointer("/build/version").and_then(|v| v.as_str()).map(str::to_owned))
        .unwrap_or_default();
    let series = http
        .json::<Page<serde::de::IgnoredAny>>("api/v1/series?size=1")
        .await
        .ok()
        .map(|p| p.total);
    Ok(SignedIn {
        kind: crate::Kind::Komga,
        name: "Komga".into(),
        version,
        series,
        credential: credential.kept(),
    })
}

/// A library, as `api/v1/libraries` lists it.
#[derive(Deserialize)]
struct LibraryRow {
    id: String,
    #[serde(default)]
    name: String,
}

/// How many series the account sees, from the total of a one-row page.
pub async fn count_series(http: &Http) -> Result<Option<u32>> {
    Ok(http
        .json::<Page<serde::de::IgnoredAny>>("api/v1/series?size=1")
        .await
        .ok()
        .map(|p| p.total))
}

/// The libraries this account sees, each with its series counted.
pub async fn libraries(http: &Http) -> Result<Vec<crate::Library>> {
    let rows: Vec<LibraryRow> = http.json("api/v1/libraries").await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let series = http
            .json::<Page<serde::de::IgnoredAny>>(&format!(
                "api/v1/series?size=1&library_id={}",
                encode(&row.id)
            ))
            .await
            .ok()
            .map(|p| p.total);
        out.push(crate::Library { id: row.id, name: row.name, series });
    }
    Ok(out)
}

/// A Komga server as a book source.
pub struct KomgaSource {
    manifest: BookSourceManifest,
    /// The libraries asked, by id; every one when empty.
    libraries: Vec<String>,
    http: Http,
    keep: PathBuf,
    signed_out: Gate,
}

impl KomgaSource {
    pub fn new(
        id: &str,
        name: &str,
        http: Http,
        keep: PathBuf,
        signed_out: impl Into<Gate>,
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
                capabilities: vec!["progress".into()],
            },
            libraries: Vec::new(),
            http,
            keep,
            signed_out: signed_out.into(),
        }
    }

    /// Ask only these libraries, by id. Empty asks every one.
    #[must_use]
    pub fn in_libraries(mut self, libraries: Vec<String>) -> Self {
        self.libraries = libraries;
        self
    }

    fn aim(&self, error: ServerError) -> ProviderError {
        crate::note_sign_in(&self.signed_out, &error);
        error.into()
    }

    async fn page<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<Page<T>> {
        self.http.json(path).await
    }

    fn detail(series: Series) -> SeriesDetail {
        let mut detail = SeriesDetail::new(series.id.clone(), series.title());
        detail.synonyms =
            series.metadata.alternate_titles.iter().map(|t| t.title.clone()).collect();
        detail.description = [&series.metadata.summary, &series.books_metadata.summary]
            .into_iter()
            .find(|s| !s.is_empty())
            .cloned();
        detail.status = series.metadata.status.as_deref().and_then(SeriesStatus::parse);
        detail.authors = series.books_metadata.authors.iter().map(|a| a.name.clone()).collect();
        detail.genres = series.metadata.genres.clone();
        detail.reading_hint = match series.metadata.reading_direction.as_str() {
            "RIGHT_TO_LEFT" => Some(ReadingHint::RightToLeft),
            "LEFT_TO_RIGHT" => Some(ReadingHint::LeftToRight),
            // Komga's vertical turns page by page downwards; its webtoon is one long strip.
            "VERTICAL" => Some(ReadingHint::Vertical),
            "WEBTOON" => Some(ReadingHint::Strip),
            _ => None,
        };
        if series.metadata.reading_direction == "WEBTOON" {
            detail.format = Some(ReadingFormat::Webtoon);
        }
        if !series.metadata.language.is_empty() {
            detail.languages = vec![series.metadata.language.clone()];
        }
        detail
    }

    async fn books(&self, series: &str) -> Result<Vec<Book>> {
        let page: Page<Book> =
            self.page(&format!("api/v1/series/{}/books?unpaged=true", segment(series))).await?;
        Ok(page.content)
    }

    /// What `book` is made of. A whole file is downloaded only with `files`; without, such a
    /// book is `None`.
    async fn content_of(&self, book: &str, files: bool) -> Result<Option<ChapterContent>> {
        let info: Book = self.http.json(&format!("api/v1/books/{}", segment(book))).await?;
        let file_book = matches!(
            info.media.media_type.as_str(),
            "application/epub+zip" | "application/pdf"
        );
        if info.media.status != "READY" && !info.media.status.is_empty() {
            return Err(ServerError::Parse(format!(
                "the server could not read it: {}",
                info.media.status
            )));
        }
        if file_book || info.media.pages == 0 {
            if !files {
                return Ok(None);
            }
            let name = file_name(&info);
            let to = keep_path(&self.keep, &self.manifest.id, book, &name);
            self.http
                .download(
                    &format!("api/v1/books/{}/file", segment(book)),
                    &to,
                    8 * 1024 * 1024 * 1024,
                )
                .await?;
            return Ok(Some(ChapterContent::Book(BookFile { path: to, temporary: true })));
        }
        let pages: Vec<PageInfo> =
            self.http.json(&format!("api/v1/books/{}/pages", segment(book))).await?;
        if pages.len() > crate::MOST_PAGES as usize {
            return Err(crate::too_many_pages(pages.len()));
        }
        let header: Vec<(String, String)> = self.http.auth().header().into_iter().collect();
        Ok(Some(ChapterContent::Images(
            pages
                .into_iter()
                .map(|p| PageRef {
                    url: self.http.url(&format!(
                        "api/v1/books/{}/pages/{}",
                        segment(book),
                        p.number
                    )),
                    headers: header.clone(),
                    made: None,
                    width: p.width,
                    height: p.height,
                })
                .collect(),
        )))
    }
}

fn file_name(book: &Book) -> String {
    let extension = match book.media.media_type.as_str() {
        "application/epub+zip" => "epub",
        "application/pdf" => "pdf",
        "application/x-cbr" | "application/vnd.rar" => "cbr",
        "application/x-7z-compressed" => "cb7",
        _ => "cbz",
    };
    format!("{}.{extension}", book.id)
}

/// A value for a query string, form-encoded: a space is `+`.
pub(crate) fn encode(id: &str) -> String {
    url::form_urlencoded::byte_serialize(id.as_bytes()).collect()
}

/// An id as one segment of a URL path: every byte but the unreserved ones is percent-encoded,
/// so a space is `%20` and a `/` stays inside the segment.
pub(crate) fn segment(id: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(id.len());
    for byte in id.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[async_trait]
impl BookSource for KomgaSource {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _hints: &BookHints,
    ) -> std::result::Result<Vec<SeriesHit>, ProviderError> {
        let mut path = format!("api/v1/series?size=60&search={}", encode(query));
        for library in &self.libraries {
            path.push_str(&format!("&library_id={}", encode(library)));
        }
        let page: Page<Series> = self.page(&path).await.map_err(|e| self.aim(e))?;
        Ok(page
            .content
            .into_iter()
            .map(|s| {
                let mut hit = SeriesHit::new(s.id.clone(), s.title());
                hit.synonyms =
                    s.metadata.alternate_titles.iter().map(|t| t.title.clone()).collect();
                hit.cover = Some(self.http.url(&format!("api/v1/series/{}/thumbnail", s.id)));
                if !s.metadata.language.is_empty() {
                    hit.language = Some(s.metadata.language.clone());
                }
                if s.books_count > 0 {
                    hit.amount = Some(kaeri_core::books::HitAmount::Volumes(s.books_count));
                }
                hit
            })
            .collect())
    }

    async fn series(
        &self,
        id: &str,
        _hints: &BookHints,
    ) -> std::result::Result<SeriesDetail, ProviderError> {
        let series: Series = self
            .http
            .json(&format!("api/v1/series/{}", segment(id)))
            .await
            .map_err(|e| self.aim(e))?;
        Ok(Self::detail(series))
    }

    async fn chapters(
        &self,
        id: &str,
        _hints: &BookHints,
    ) -> std::result::Result<Vec<SourceChapter>, ProviderError> {
        let books = self.books(id).await.map_err(|e| self.aim(e))?;
        Ok(books
            .into_iter()
            .map(|b| {
                let title =
                    if b.metadata.title.is_empty() { &b.name } else { &b.metadata.title };
                let mut chapter = chapter_of(&b.id, title, Some(&b.metadata.number));
                chapter.page_count = crate::pages_stated(b.media.pages);
                chapter.published =
                    b.metadata.release_date.as_deref().and_then(crate::names::date);
                chapter
            })
            .collect())
    }

    async fn content(
        &self,
        _series: &str,
        chapter: &str,
    ) -> std::result::Result<ChapterContent, ProviderError> {
        match self.content_of(chapter, true).await {
            Ok(Some(content)) => Ok(content),
            Ok(None) => Err(ProviderError::NotFound),
            Err(e) => Err(self.aim(e)),
        }
    }

    async fn chapter_pages(
        &self,
        _series: &str,
        chapter: &str,
    ) -> std::result::Result<Option<Vec<PageRef>>, ProviderError> {
        match self.content_of(chapter, false).await {
            Ok(Some(ChapterContent::Images(pages))) => Ok(Some(pages)),
            Ok(_) => Ok(None),
            Err(e) => Err(self.aim(e)),
        }
    }

    async fn report_progress(
        &self,
        _series: &str,
        chapter: &str,
        read: bool,
    ) -> std::result::Result<(), ProviderError> {
        let path = format!("api/v1/books/{}/read-progress", segment(chapter));
        let request = if read {
            self.http
                .request(reqwest::Method::PATCH, &path)
                .json(&serde_json::json!({ "completed": true }))
        } else {
            self.http.request(reqwest::Method::DELETE, &path)
        };
        match self.http.send(request).await {
            Ok(_) => Ok(()),
            // Taking back what the server never had is not a failure.
            Err(ServerError::NotFound) if !read => Ok(()),
            Err(e) => Err(self.aim(e)),
        }
    }

    async fn health_check(&self) -> std::result::Result<(), ProviderError> {
        self.http.get("api/v1/users/me").await.map(drop).map_err(|e| self.aim(e))
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

    fn hinted(direction: &str) -> (Option<ReadingHint>, Option<ReadingFormat>) {
        let series: Series = serde_json::from_value(serde_json::json!({
            "id": "s1",
            "name": "Lantern Keeper",
            "metadata": { "readingDirection": direction },
        }))
        .unwrap();
        let detail = KomgaSource::detail(series);
        (detail.reading_hint, detail.format)
    }

    /// Every direction Komga keeps for a series becomes the hint the reader opens with.
    #[test]
    fn each_reading_direction_komga_keeps_is_a_hint() {
        assert_eq!(hinted("RIGHT_TO_LEFT"), (Some(ReadingHint::RightToLeft), None));
        assert_eq!(hinted("LEFT_TO_RIGHT"), (Some(ReadingHint::LeftToRight), None));
        assert_eq!(hinted("VERTICAL"), (Some(ReadingHint::Vertical), None));
        assert_eq!(hinted("WEBTOON"), (Some(ReadingHint::Strip), Some(ReadingFormat::Webtoon)));
        // Unset in Komga: the series says nothing, and the reader guesses.
        assert_eq!(hinted(""), (None, None));
    }
}
