//! Kavita: its REST API, as a book source.
//!
//! Sign-in is `POST /api/Account/login`, which answers a JWT, a refresh token and the account's
//! API key. The JWT signs API calls and is renewed when the server refuses it: with the refresh
//! token, and failing that with the API key, the way a key sign-in works. The API key goes in
//! page image addresses, which an image view fetches without headers.

use std::{future::Future, path::PathBuf, sync::Mutex};

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookFile, BookHints, BookSourceKind, BookSourceManifest, ChapterContent, PageRef,
        SeriesDetail, SeriesHit, SourceChapter,
    },
    error::ProviderError,
    media::Medium,
    traits::{BookSource, PageError},
};
use serde::{Deserialize, Serialize};

use crate::{
    Gate, SignedIn,
    error::{Result, ServerError},
    http::{Auth, Http},
    kept::keep_path,
    komga::{encode, segment},
    names::chapter_of,
};

/// What a sign-in kept: the tokens, as JSON in the secure store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tokens {
    pub token: String,
    #[serde(rename = "refreshToken", default)]
    pub refresh: String,
    #[serde(rename = "apiKey", default)]
    pub api_key: String,
}

impl Tokens {
    pub fn parse(kept: &str) -> Option<Self> {
        serde_json::from_str(kept.strip_prefix("kavita:")?).ok()
    }

    pub fn kept(&self) -> String {
        format!("kavita:{}", serde_json::to_string(self).unwrap_or_default())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Search {
    #[serde(default)]
    series: Vec<SeriesRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeriesRow {
    #[serde(alias = "seriesId")]
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    localized_name: Option<String>,
    #[serde(default)]
    library_id: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Detail {
    #[serde(default)]
    chapters: Vec<ChapterRow>,
    #[serde(default)]
    volumes: Vec<VolumeRow>,
    #[serde(default)]
    specials: Vec<ChapterRow>,
    #[serde(default)]
    storyline_chapters: Vec<ChapterRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VolumeRow {
    #[serde(default)]
    name: String,
    #[serde(default)]
    chapters: Vec<ChapterRow>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChapterRow {
    id: i64,
    #[serde(default)]
    range: String,
    #[serde(default)]
    title_name: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    pages: u32,
    #[serde(default)]
    is_special: bool,
    #[serde(default)]
    release_date: Option<String>,
}

#[derive(Deserialize)]
struct Login {
    #[serde(default)]
    token: String,
    #[serde(rename = "refreshToken", default)]
    refresh: String,
    #[serde(rename = "apiKey", default)]
    api_key: String,
}

/// How many series the signed-in user can see: one row of the series list asked for, and the
/// total from its `Pagination` header. Search is not used, as it stops at a few results.
async fn count_series(signed: &Http) -> Option<u32> {
    let request = signed
        .request(reqwest::Method::POST, "api/Series/all-v2?PageNumber=1&PageSize=1")
        .json(&serde_json::json!({}));
    let response = signed.send(request).await.ok()?;
    pagination_total(response.headers().get("pagination")?.to_str().ok()?)
}

/// How many series the account sees now, for a recheck.
pub async fn count_all(signed: &Http) -> Result<Option<u32>> {
    let request = signed
        .request(reqwest::Method::POST, "api/Series/all-v2?PageNumber=1&PageSize=1")
        .json(&serde_json::json!({}));
    let response = signed.send(request).await?;
    Ok(response
        .headers()
        .get("pagination")
        .and_then(|h| h.to_str().ok())
        .and_then(pagination_total))
}

/// A library, as `api/Library/libraries` lists it.
#[derive(Deserialize)]
struct LibraryRow {
    id: i64,
    #[serde(default)]
    name: String,
}

/// The libraries this account sees. Kavita before 0.8 lists them at `api/Library`.
async fn library_rows(signed: &Http) -> Result<Vec<LibraryRow>> {
    match signed.json("api/Library/libraries").await {
        Ok(rows) => Ok(rows),
        Err(ServerError::NotFound | ServerError::Status(_)) => signed.json("api/Library").await,
        Err(e) => Err(e),
    }
}

/// Kavita's filter field for a series' library, and the comparison "is one of".
const FIELD_LIBRARIES: u8 = 19;
const CONTAINS: u8 = 5;

/// How many series of one library the account sees.
async fn count_in_library(signed: &Http, library: i64) -> Option<u32> {
    let filter = serde_json::json!({
        "statements": [{ "comparison": CONTAINS, "field": FIELD_LIBRARIES, "value": library.to_string() }],
        "combination": 1,
        "limitTo": 0,
    });
    let request = signed
        .request(reqwest::Method::POST, "api/Series/all-v2?PageNumber=1&PageSize=1")
        .json(&filter);
    let response = signed.send(request).await.ok()?;
    pagination_total(response.headers().get("pagination")?.to_str().ok()?)
}

/// The libraries this account sees, each with its series counted.
pub async fn libraries(signed: &Http) -> Result<Vec<crate::Library>> {
    let rows = library_rows(signed).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let series = count_in_library(signed, row.id).await;
        out.push(crate::Library { id: row.id.to_string(), name: row.name, series });
    }
    Ok(out)
}

/// `totalItems` from Kavita's `Pagination` header.
fn pagination_total(header: &str) -> Option<u32> {
    let value: serde_json::Value = serde_json::from_str(header).ok()?;
    u32::try_from(value.get("totalItems")?.as_u64()?).ok()
}

/// Sign in and describe the server. `address` is the server's root.
pub async fn sign_in(
    client: &reqwest::Client,
    address: &str,
    user: &str,
    password: &str,
) -> Result<SignedIn> {
    let http = Http::new(client, address, Auth::None);
    let request = http
        .request(reqwest::Method::POST, "api/Account/login")
        .json(&serde_json::json!({ "username": user, "password": password }));
    let login: Login = crate::http::read_json(http.send(request).await?).await?;
    described(client, address, login).await
}

/// Sign in with an account's API key, the one its OPDS link carries: Kavita's plugin sign-in,
/// which answers the same tokens a password sign-in does. A key it does not know is a 401.
pub async fn sign_in_with_key(
    client: &reqwest::Client,
    address: &str,
    key: &str,
) -> Result<SignedIn> {
    let login = key_login(&Http::new(client, address, Auth::None), key).await?;
    described(client, address, login).await
}

/// Kavita's plugin sign-in with `key`, on a server asked without a sign-in.
async fn key_login(http: &Http, key: &str) -> Result<Login> {
    let path = format!("api/Plugin/authenticate?apiKey={}&pluginName=Kaeri", encode(key));
    let request = http.request(reqwest::Method::POST, &path);
    let mut login: Login = crate::http::read_json(http.send(request).await?).await?;
    if login.api_key.is_empty() {
        login.api_key = key.to_owned();
    }
    Ok(login)
}

/// What `api/Account/refresh-token` answers.
#[derive(Deserialize)]
struct Refreshed {
    #[serde(default)]
    token: String,
    #[serde(rename = "refreshToken", default)]
    refresh: String,
}

/// Whether a failed renewal step is the server turning it down, so the next way is tried,
/// rather than a server that could not answer now.
const fn turned_down(error: &ServerError) -> bool {
    match error {
        ServerError::Unauthorized | ServerError::NotFound | ServerError::Parse(_) => true,
        ServerError::Status(status) => *status < 500,
        _ => false,
    }
}

/// New tokens for ones the server stopped taking: the refresh token first, then the API key.
/// `base` is the server, asked without a sign-in. [`ServerError::Unauthorized`] when neither
/// is taken; a server that cannot answer now says so with its own error instead, as that is
/// not a refusal.
pub(crate) async fn renew(base: &Http, held: &Tokens) -> Result<Tokens> {
    if !held.refresh.is_empty() {
        let request = base
            .request(reqwest::Method::POST, "api/Account/refresh-token")
            .json(&serde_json::json!({ "token": held.token, "refreshToken": held.refresh }));
        let answer = match base.send(request).await {
            Ok(response) => crate::http::read_json::<Refreshed>(response).await,
            Err(e) => Err(e),
        };
        match answer {
            Ok(fresh) if !fresh.token.is_empty() => {
                let refresh =
                    if fresh.refresh.is_empty() { held.refresh.clone() } else { fresh.refresh };
                return Ok(Tokens {
                    token: fresh.token,
                    refresh,
                    api_key: held.api_key.clone(),
                });
            }
            Ok(_) => {}
            Err(e) if turned_down(&e) => {}
            Err(e) => return Err(e),
        }
    }
    if held.api_key.is_empty() {
        return Err(ServerError::Unauthorized);
    }
    match key_login(base, &held.api_key).await {
        Ok(login) if !login.token.is_empty() => {
            Ok(Tokens { token: login.token, refresh: login.refresh, api_key: login.api_key })
        }
        Ok(_) => Err(ServerError::Unauthorized),
        Err(e) if turned_down(&e) => Err(ServerError::Unauthorized),
        Err(e) => Err(e),
    }
}

/// The server as it describes itself to an account that has just signed in.
async fn described(client: &reqwest::Client, address: &str, login: Login) -> Result<SignedIn> {
    if login.token.is_empty() {
        return Err(ServerError::Unauthorized);
    }
    let tokens = Tokens { token: login.token, refresh: login.refresh, api_key: login.api_key };
    let signed = Http::new(client, address, Auth::Bearer(tokens.token.clone()));
    let version = signed.text("api/Server/server-info-slim").await.ok().and_then(|t| {
        serde_json::from_str::<serde_json::Value>(&t)
            .ok()?
            .get("kavitaVersion")?
            .as_str()
            .map(str::to_owned)
    });
    let series = count_series(&signed).await;
    Ok(SignedIn {
        kind: crate::Kind::Kavita,
        name: "Kavita".into(),
        version: version.unwrap_or_default(),
        series,
        credential: tokens.kept(),
    })
}

/// A Kavita server as a book source.
pub struct KavitaSource {
    manifest: BookSourceManifest,
    /// The libraries asked, by id; every one when empty.
    libraries: Vec<String>,
    /// The server, asked without a sign-in: each call adds the JWT held when it starts.
    http: Http,
    /// The tokens held now. A renewal replaces them.
    tokens: Mutex<Tokens>,
    /// Held while the tokens are renewed, so calls refused at once share one renewal.
    renewing: tokio::sync::Mutex<()>,
    keep: PathBuf,
    signed_out: Gate,
}

impl KavitaSource {
    pub fn new(
        id: &str,
        name: &str,
        http: Http,
        tokens: &Tokens,
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
            tokens: Mutex::new(tokens.clone()),
            renewing: tokio::sync::Mutex::new(()),
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

    /// The refresh token currently held, for a caller that wants to keep it.
    pub fn refresh_token(&self) -> String {
        self.held().refresh
    }

    fn held(&self) -> Tokens {
        self.tokens.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The server signed with the JWT held now, and that JWT.
    fn signed_now(&self) -> (Http, String) {
        let token = self.held().token;
        (self.http.with_auth(Auth::Bearer(token.clone())), token)
    }

    fn api_key(&self) -> String {
        self.held().api_key
    }

    /// Run `call` signed with the JWT held. Refused, the tokens are renewed once and `call`
    /// runs again with the new JWT; a refusal that renewing cannot get past comes back.
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

    /// Renew the tokens after the server refused `used`. Calls refused together share one
    /// renewal: a call that waited for another's finds the JWT already new and asks nothing.
    /// New tokens are handed to the keeper; a renewal turned down signs the source out.
    async fn renew(&self, used: &str) -> Result<()> {
        let _turn = self.renewing.lock().await;
        if self.signed_out.is_signed_out() {
            return Err(ServerError::Unauthorized);
        }
        let held = self.held();
        if held.token != used {
            return Ok(());
        }
        match renew(&self.http, &held).await {
            Ok(fresh) => {
                tracing::debug!(source = %self.manifest.id, "the Kavita sign-in was renewed");
                let kept = fresh.kept();
                *self.tokens.lock().unwrap_or_else(|e| e.into_inner()) = fresh;
                self.signed_out.renewed(&kept);
                Ok(())
            }
            Err(error) => {
                crate::note_sign_in(&self.signed_out, &error);
                Err(error)
            }
        }
    }

    fn chapters_of(detail: Detail) -> Vec<ChapterRow> {
        let mut rows = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let from_volumes = detail.volumes.into_iter().flat_map(|v| {
            let name = v.name;
            v.chapters.into_iter().map(move |mut c| {
                if c.range.is_empty() || c.range == "0" {
                    c.range = format!("Volume {name}");
                }
                c
            })
        });
        for c in detail
            .chapters
            .into_iter()
            .chain(detail.storyline_chapters)
            .chain(from_volumes)
            .chain(detail.specials)
        {
            if seen.insert(c.id) {
                rows.push(c);
            }
        }
        rows
    }

    /// What `chapter` is made of. A whole file is downloaded only with `files`; without, such a
    /// chapter is `None`.
    async fn content_of(&self, chapter: &str, files: bool) -> Result<Option<ChapterContent>> {
        let path = &format!("api/Reader/chapter-info?chapterId={}", encode(chapter));
        let info: serde_json::Value =
            self.signed(|http| async move { http.json(path).await }).await?;
        let pages = info.get("pages").and_then(serde_json::Value::as_u64).unwrap_or(0);
        let is_book = info.get("isEpub").and_then(serde_json::Value::as_bool).unwrap_or(false)
            || info.get("fileFormat").and_then(serde_json::Value::as_u64) == Some(3);
        let is_pdf = info.get("fileFormat").and_then(serde_json::Value::as_u64) == Some(4);
        if is_book || is_pdf || pages == 0 {
            if !files {
                return Ok(None);
            }
            let to = keep_path(
                &self.keep,
                &self.manifest.id,
                chapter,
                if is_pdf { "book.pdf" } else { "book.epub" },
            );
            let path = &format!("api/Download/chapter?chapterId={}", encode(chapter));
            let dest = &to;
            self.signed(|http| async move {
                http.download(path, dest, 8 * 1024 * 1024 * 1024).await
            })
            .await?;
            return Ok(Some(ChapterContent::Book(BookFile { path: to, temporary: true })));
        }
        let pages = crate::pages_stated(pages).ok_or_else(|| crate::too_many_pages(pages))?;
        let key = encode(&self.api_key());
        Ok(Some(ChapterContent::Images(
            (0..pages)
                .map(|n| {
                    PageRef::new(format!(
                        "{}?chapterId={}&page={n}&apiKey={}",
                        self.http.url("api/Reader/image"),
                        encode(chapter),
                        key
                    ))
                })
                .collect(),
        )))
    }
}

#[async_trait]
impl BookSource for KavitaSource {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _hints: &BookHints,
    ) -> std::result::Result<Vec<SeriesHit>, ProviderError> {
        let path = &format!("api/Search/search?queryString={}", encode(query));
        let found: Search = self
            .signed(|http| async move { http.json(path).await })
            .await
            .map_err(|e| self.aim(e))?;
        let key = encode(&self.api_key());
        Ok(found
            .series
            .into_iter()
            .filter(|s| {
                self.libraries.is_empty()
                    || s.library_id.is_some_and(|l| self.libraries.contains(&l.to_string()))
            })
            .map(|s| {
                let mut hit = SeriesHit::new(s.id.to_string(), s.name.clone());
                if let Some(local) = s.localized_name.filter(|l| !l.is_empty() && *l != s.name)
                {
                    hit.synonyms = vec![local];
                }
                hit.cover = Some(format!(
                    "{}?seriesId={}&apiKey={}",
                    self.http.url("api/Image/series-cover"),
                    s.id,
                    key
                ));
                hit
            })
            .collect())
    }

    async fn series(
        &self,
        id: &str,
        _hints: &BookHints,
    ) -> std::result::Result<SeriesDetail, ProviderError> {
        let path = &format!("api/Series/{}", segment(id));
        let row: serde_json::Value = self
            .signed(|http| async move { http.json(path).await })
            .await
            .map_err(|e| self.aim(e))?;
        let name = row.get("name").and_then(|v| v.as_str()).unwrap_or(id);
        // Kavita keeps no reading direction of a series' own, so it gives no hint.
        let mut detail = SeriesDetail::new(id, name);
        let path = &format!("api/Series/metadata?seriesId={}", encode(id));
        if let Ok(meta) =
            self.signed(|http| async move { http.json::<serde_json::Value>(path).await }).await
        {
            detail.description = meta
                .get("summary")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            let names = |key: &str| -> Vec<String> {
                meta.get(key)
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|p| p.get("name").or_else(|| p.get("title")))
                            .filter_map(|n| n.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            detail.authors = names("writers");
            detail.genres = names("genres");
        }
        Ok(detail)
    }

    async fn chapters(
        &self,
        id: &str,
        _hints: &BookHints,
    ) -> std::result::Result<Vec<SourceChapter>, ProviderError> {
        let path = &format!("api/Series/series-detail?seriesId={}", encode(id));
        let detail: Detail = self
            .signed(|http| async move { http.json(path).await })
            .await
            .map_err(|e| self.aim(e))?;
        Ok(Self::chapters_of(detail)
            .into_iter()
            .map(|c| {
                let title = [&c.title_name, &c.title, &c.range]
                    .into_iter()
                    .find(|t| !t.is_empty())
                    .cloned()
                    .unwrap_or_default();
                let number = (!c.is_special && c.range != "0" && !c.range.is_empty())
                    .then_some(c.range.as_str());
                let mut chapter = chapter_of(&c.id.to_string(), &title, number);
                chapter.page_count = crate::pages_stated(c.pages);
                chapter.published = c.release_date.as_deref().and_then(crate::names::date);
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
        series: &str,
        chapter: &str,
        read: bool,
    ) -> std::result::Result<(), ProviderError> {
        let path = if read {
            "api/Reader/mark-multiple-read"
        } else {
            "api/Reader/mark-multiple-unread"
        };
        let (Ok(series), Ok(chapter)) = (series.parse::<i64>(), chapter.parse::<i64>()) else {
            return Err(ProviderError::NotFound);
        };
        let body = serde_json::json!({
            "seriesId": series,
            "volumeIds": [],
            "chapterIds": [chapter],
        });
        let body = &body;
        self.signed(|http| async move {
            http.send(http.request(reqwest::Method::POST, path).json(body)).await
        })
        .await
        .map(drop)
        .map_err(|e| self.aim(e))
    }

    async fn health_check(&self) -> std::result::Result<(), ProviderError> {
        self.signed(|http| async move { http.get("api/Account").await })
            .await
            .map(drop)
            .map_err(|e| self.aim(e))
    }

    fn is_available(&self) -> std::result::Result<(), ProviderError> {
        crate::signed_out_check(&self.signed_out)
    }

    async fn fetch_page(&self, page: &PageRef) -> std::result::Result<Vec<u8>, PageError> {
        let (http, used) = self.signed_now();
        match crate::fetch_with(&http, page).await {
            Err(PageError::Status(401 | 403)) => {
                self.renew(&used).await.map_err(|e| PageError::Failed(self.aim(e)))?;
                match crate::fetch_with(&self.signed_now().0, page).await {
                    // Refused with a JWT just handed out: renewing again would not help.
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
    use super::pagination_total;

    #[test]
    fn the_series_count_is_the_pagination_headers_total() {
        let header = r#"{"currentPage":1,"itemsPerPage":1,"totalItems":212,"totalPages":212}"#;
        assert_eq!(pagination_total(header), Some(212));
        assert_eq!(pagination_total(r#"{"currentPage":1}"#), None);
        assert_eq!(pagination_total("not json"), None);
    }
}
