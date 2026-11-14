//! Komga: its REST API, as a book source.
//!
//! Sign-in is HTTP Basic against `users/me` (`api/v2`, or `api/v1` on a Komga older than
//! 0.153). A Komga new enough to hand out a session token is asked for one (`X-Auth-Token` with
//! an empty value) and to remember the sign-in (`remember-me=true`, which Komga has taken since
//! 0.13): the token and the `komga-remember-me` cookie are what is kept. An older one gets the
//! Basic credential kept instead. Either way the password itself is not written anywhere
//! readable: what is kept goes to the secure store, never the database.
//!
//! A session token lapses when the server ends the session. The remember-me cookie starts a new
//! one, once for every call refused at the same time, and a cookie the server hands back in its
//! place is kept instead. A server that turns the cookie down, or a sign-in kept without one,
//! signs the source out, once, and the cookie is forgotten; it waits for the person to sign in
//! again. A server that cannot answer now signs nothing out.

use std::{future::Future, path::PathBuf, sync::Mutex};

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
use serde::{Deserialize, Serialize};

use crate::{
    Gate, SignedIn,
    error::{Result, ServerError},
    http::{Auth, Http, basic},
    kept::keep_path,
    names::chapter_of,
};

/// The cookie Komga remembers a sign-in with.
const REMEMBER_ME: &str = "komga-remember-me";

/// Where Komga answers the account signed in: `api/v2` since 0.153, `api/v1` before (1.0
/// removed it).
const ME: [&str; 2] = ["api/v2/users/me", "api/v1/users/me"];

/// What a sign-in kept. Its `Debug` names the kind only: every field is a secret.
#[derive(Clone, PartialEq, Eq)]
pub enum Credential {
    Basic(String),
    Token(String),
    /// A session token and the remember-me cookie that starts a new session when it lapses.
    Remembered {
        token: String,
        cookie: String,
    },
}

/// How a remembered sign-in is kept: `komga:` and this as JSON.
#[derive(Serialize, Deserialize)]
struct RememberedKept {
    token: String,
    #[serde(rename = "rememberMe")]
    cookie: String,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Basic(_) => "Credential::Basic(..)",
            Self::Token(_) => "Credential::Token(..)",
            Self::Remembered { .. } => "Credential::Remembered(..)",
        })
    }
}

impl Credential {
    pub fn parse(kept: &str) -> Option<Self> {
        match kept.split_once(':')? {
            ("basic", b64) => Some(Self::Basic(b64.to_owned())),
            ("token", token) => Some(Self::Token(token.to_owned())),
            ("komga", json) => {
                let RememberedKept { token, cookie } = serde_json::from_str(json).ok()?;
                Some(if cookie.is_empty() {
                    Self::Token(token)
                } else {
                    Self::Remembered { token, cookie }
                })
            }
            _ => None,
        }
    }

    pub fn kept(&self) -> String {
        match self {
            Self::Basic(b64) => format!("basic:{b64}"),
            Self::Token(token) => format!("token:{token}"),
            Self::Remembered { token, cookie } => {
                let kept = RememberedKept { token: token.clone(), cookie: cookie.clone() };
                format!("komga:{}", serde_json::to_string(&kept).unwrap_or_default())
            }
        }
    }

    pub fn auth(&self) -> Auth {
        match self {
            Self::Basic(b64) => Auth::Basic(b64.clone()),
            Self::Token(token) | Self::Remembered { token, .. } => {
                Auth::Header("X-Auth-Token", token.clone())
            }
        }
    }

    /// What signs a call: the session token, or the Basic credential.
    fn signs_with(&self) -> &str {
        match self {
            Self::Basic(secret)
            | Self::Token(secret)
            | Self::Remembered { token: secret, .. } => secret,
        }
    }

    /// The same sign-in with its remember-me cookie forgotten, for a server that turned it down.
    fn forgetting_cookie(&self) -> Option<Self> {
        match self {
            Self::Remembered { token, .. } => Some(Self::Token(token.clone())),
            _ => None,
        }
    }
}

/// The value of the response's `komga-remember-me` cookie, empty when the server cancels it.
fn remember_cookie(response: &reqwest::Response) -> Option<String> {
    response.headers().get_all(reqwest::header::SET_COOKIE).iter().find_map(|header| {
        let pair = header.to_str().ok()?.split(';').next()?;
        let (name, value) = pair.split_once('=')?;
        (name.trim() == REMEMBER_ME).then(|| value.trim().trim_matches('"').to_owned())
    })
}

/// The session token the response hands out.
fn session_token(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get("x-auth-token")
        .and_then(|v| v.to_str().ok())
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

/// Ask `users/me`, at whichever version the server answers.
async fn me(
    http: &Http,
    request: impl Fn(&str) -> reqwest::RequestBuilder,
) -> Result<reqwest::Response> {
    for path in ME {
        match http.send(request(path)).await {
            Err(ServerError::NotFound) => {}
            done => return done,
        }
    }
    Err(ServerError::NotFound)
}

/// Whether a failed renewal is the server turning the cookie down, rather than a server that
/// could not answer now.
const fn turned_down(error: &ServerError) -> bool {
    match error {
        ServerError::Unauthorized | ServerError::NotFound | ServerError::Parse(_) => true,
        ServerError::Status(status) => *status < 500,
        _ => false,
    }
}

/// A new session for `held`, whose token the server stopped taking, from its remember-me
/// cookie, with the cookie the server hands back in its place. [`ServerError::Unauthorized`]
/// when there is no cookie or the server turns it down; a server that cannot answer now says
/// so with its own error, as that is not a refusal.
async fn renew(base: &Http, held: &Credential) -> Result<Credential> {
    let Credential::Remembered { cookie, .. } = held else {
        return Err(ServerError::Unauthorized);
    };
    let base = base.with_auth(Auth::None);
    let answer = me(&base, |path| {
        base.request(reqwest::Method::GET, path)
            .header("X-Auth-Token", "")
            .header(reqwest::header::COOKIE, format!("{REMEMBER_ME}={cookie}"))
    })
    .await;
    match answer {
        Ok(response) => {
            let rotated = remember_cookie(&response);
            match (session_token(&response), rotated) {
                // The server cancelled the cookie while it answered.
                (_, Some(rotated)) if rotated.is_empty() => Err(ServerError::Unauthorized),
                (Some(token), rotated) => Ok(Credential::Remembered {
                    token,
                    cookie: rotated.unwrap_or_else(|| cookie.clone()),
                }),
                (None, _) => Err(ServerError::Unauthorized),
            }
        }
        Err(error) if turned_down(&error) => Err(ServerError::Unauthorized),
        Err(error) => Err(error),
    }
}

/// [`renew`], telling `gate`: a new sign-in goes to the keeper; one turned down forgets the
/// cookie and signs the server out.
pub(crate) async fn renew_kept(
    base: &Http,
    held: &Credential,
    gate: &Gate,
) -> Result<Credential> {
    match renew(base, held).await {
        Ok(fresh) => {
            gate.renewed(&fresh.kept());
            Ok(fresh)
        }
        Err(error) => {
            if error.needs_sign_in() {
                if let Some(forgotten) = held.forgetting_cookie() {
                    gate.renewed(&forgotten.kept());
                }
                gate.refuse();
            }
            Err(error)
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
    // Asking for a token with an empty header is how Komga is told to hand one out, and
    // `remember-me` how it is told to hand out the cookie that starts the next session.
    let response = me(&http, |path| {
        http.request(reqwest::Method::GET, &format!("{path}?remember-me=true"))
            .header("X-Auth-Token", "")
    })
    .await?;
    let token = session_token(&response);
    let cookie = remember_cookie(&response).filter(|c| !c.is_empty());
    // The answer must look like Komga's user, or this is some other server.
    let me: serde_json::Value = crate::http::read_json(response).await?;
    if me.get("roles").is_none() {
        return Err(ServerError::Parse("not a Komga user".into()));
    }
    // A server that hands out no cookie is signed in as before: a lapse signs it out.
    let credential = match (token, cookie) {
        (Some(token), Some(cookie)) => Credential::Remembered { token, cookie },
        (Some(token), None) => Credential::Token(token),
        (None, _) => Credential::Basic(basic),
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

/// How many series the account sees, from the total of a one-row page. `None` when the server
/// did not say; a refused sign-in is an error, for the caller to renew or sign out.
pub async fn count_series(http: &Http) -> Result<Option<u32>> {
    match http.json::<Page<serde::de::IgnoredAny>>("api/v1/series?size=1").await {
        Ok(page) => Ok(Some(page.total)),
        Err(ServerError::Unauthorized) => Err(ServerError::Unauthorized),
        Err(_) => Ok(None),
    }
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
    /// The server, asked without a sign-in: each call adds the one held when it starts.
    http: Http,
    /// The sign-in held now. A renewal replaces it.
    credential: Mutex<Credential>,
    /// Held while the session is renewed, so calls refused at once share one renewal.
    renewing: tokio::sync::Mutex<()>,
    keep: PathBuf,
    signed_out: Gate,
}

impl KomgaSource {
    pub fn new(
        id: &str,
        name: &str,
        http: Http,
        credential: Credential,
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
            http: http.with_auth(Auth::None),
            credential: Mutex::new(credential),
            renewing: tokio::sync::Mutex::new(()),
            keep,
            signed_out: signed_out.into(),
        }
    }

    fn held(&self) -> Credential {
        self.credential.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The server signed with the sign-in held now, and what signed it.
    fn signed_now(&self) -> (Http, String) {
        let held = self.held();
        (self.http.with_auth(held.auth()), held.signs_with().to_owned())
    }

    /// Run `call` signed with the sign-in held. Refused, the session is renewed once and
    /// `call` runs again with the new one; a refusal that renewing cannot get past comes back.
    async fn signed<T, F, Fut>(&self, call: F) -> Result<T>
    where
        F: Fn(Http) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let (http, used) = self.signed_now();
        match call(http).await {
            Err(ServerError::Unauthorized) => {
                self.renew(&used).await?;
                call(self.signed_now().0).await
            }
            done => done,
        }
    }

    /// Renew the session after the server refused `used`. Calls refused together share one
    /// renewal: a call that waited for another's finds the session already new and asks
    /// nothing.
    async fn renew(&self, used: &str) -> Result<()> {
        let _turn = self.renewing.lock().await;
        if self.signed_out.is_signed_out() {
            return Err(ServerError::Unauthorized);
        }
        let held = self.held();
        if held.signs_with() != used {
            return Ok(());
        }
        match renew_kept(&self.http, &held, &self.signed_out).await {
            Ok(fresh) => {
                tracing::debug!(source = %self.manifest.id, "the Komga session was renewed");
                *self.credential.lock().unwrap_or_else(|e| e.into_inner()) = fresh;
                Ok(())
            }
            Err(error) => {
                if let Some(forgotten) = held.forgetting_cookie()
                    && error.needs_sign_in()
                {
                    *self.credential.lock().unwrap_or_else(|e| e.into_inner()) = forgotten;
                }
                Err(error)
            }
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
        self.signed(|http| async move { http.json(path).await }).await
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
        self.signed(|http| async move { self.content_with(&http, book, files).await }).await
    }

    async fn content_with(
        &self,
        http: &Http,
        book: &str,
        files: bool,
    ) -> Result<Option<ChapterContent>> {
        let info: Book = http.json(&format!("api/v1/books/{}", segment(book))).await?;
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
            http.download(
                &format!("api/v1/books/{}/file", segment(book)),
                &to,
                8 * 1024 * 1024 * 1024,
            )
            .await?;
            return Ok(Some(ChapterContent::Book(BookFile { path: to, temporary: true })));
        }
        let pages: Vec<PageInfo> =
            http.json(&format!("api/v1/books/{}/pages", segment(book))).await?;
        if pages.len() > crate::MOST_PAGES as usize {
            return Err(crate::too_many_pages(pages.len()));
        }
        let header: Vec<(String, String)> = http.auth().header().into_iter().collect();
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
        let path = &format!("api/v1/series/{}", segment(id));
        let series: Series = self
            .signed(|http| async move { http.json(path).await })
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
        let path = &format!("api/v1/books/{}/read-progress", segment(chapter));
        let sent = self
            .signed(|http| async move {
                let request = if read {
                    http.request(reqwest::Method::PATCH, path)
                        .json(&serde_json::json!({ "completed": true }))
                } else {
                    http.request(reqwest::Method::DELETE, path)
                };
                http.send(request).await
            })
            .await;
        match sent {
            Ok(_) => Ok(()),
            // Taking back what the server never had is not a failure.
            Err(ServerError::NotFound) if !read => Ok(()),
            Err(e) => Err(self.aim(e)),
        }
    }

    async fn health_check(&self) -> std::result::Result<(), ProviderError> {
        self.signed(|http| async move {
            me(&http, |path| http.request(reqwest::Method::GET, path)).await
        })
        .await
        .map(drop)
        .map_err(|e| self.aim(e))
    }

    fn is_available(&self) -> std::result::Result<(), ProviderError> {
        crate::signed_out_check(&self.signed_out)
    }

    async fn fetch_page(&self, page: &PageRef) -> std::result::Result<Vec<u8>, PageError> {
        // The page carries the sign-in it was listed with, which a renewal may have replaced:
        // it goes with the one held now instead.
        let mut page = page.clone();
        page.headers.retain(|(name, _)| {
            !name.eq_ignore_ascii_case("x-auth-token")
                && !name.eq_ignore_ascii_case("authorization")
        });
        let (http, used) = self.signed_now();
        match crate::fetch_with(&http, &page).await {
            Err(PageError::Status(401 | 403)) => {
                self.renew(&used).await.map_err(|e| PageError::Failed(self.aim(e)))?;
                match crate::fetch_with(&self.signed_now().0, &page).await {
                    // Refused with a session just handed out: renewing again would not help.
                    Err(PageError::Status(401 | 403)) => {
                        Err(PageError::Failed(self.aim(ServerError::Unauthorized)))
                    }
                    done => done,
                }
            }
            done => done,
        }
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
