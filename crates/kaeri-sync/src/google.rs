//! Google Drive as a folder target: OAuth (code flow with PKCE, and the device flow for TVs), and
//! the Drive v3 REST calls over the hidden `appDataFolder`.
//!
//! Drive addresses files by id and allows several files with one name, so the target keeps a
//! path-to-id cache for folders and treats the smallest id as the file when a race left twins.
//! Tokens are never logged or printed: the client secret is a [`Secret`], and [`Tokens`] redacts
//! itself in `Debug`.

use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::Deserialize;

use crate::folder::{Entry, FolderError, FolderTarget, check_path};

/// Read and write the app's own hidden folder and nothing else.
pub const SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";
const ROOT: &str = "appDataFolder";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const MAX_RETRIES: u32 = 3;
const MAX_WAIT: Duration = Duration::from_secs(10);

/// A string that never prints: `Debug` shows only that it is there.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The value, for the one place that sends it to Google.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// The OAuth client of this build. iOS and Android clients have no secret; Google's desktop and TV
/// clients do.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub client_id: String,
    pub client_secret: Option<Secret>,
}

/// Where Google's services are, so a test can point them at [`FakeGoogle`].
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub auth_url: String,
    pub token_url: String,
    pub device_url: String,
    pub api_base: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            device_url: "https://oauth2.googleapis.com/device/code".into(),
            api_base: "https://www.googleapis.com".into(),
        }
    }
}

/// A PKCE verifier and its S256 challenge.
#[derive(Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl fmt::Debug for Pkce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pkce").field("challenge", &self.challenge).finish_non_exhaustive()
    }
}

impl Pkce {
    pub fn generate() -> Self {
        let mut raw = [0u8; 32];
        // The system's random source failing leaves nothing safe to do.
        SystemRandom::new().fill(&mut raw).expect("system random source");
        let verifier = URL_SAFE_NO_PAD.encode(raw);
        let challenge = challenge_for(&verifier);
        Self { verifier, challenge }
    }
}

/// `BASE64URL(SHA256(verifier))`, no padding.
pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()))
}

/// A random `state` value for the redirect.
pub fn random_state() -> String {
    let mut raw = [0u8; 16];
    SystemRandom::new().fill(&mut raw).expect("system random source");
    URL_SAFE_NO_PAD.encode(raw)
}

/// The four OAuth clients registered for Kaeri, each with its own redirect and secret rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    /// iPhone, iPad and the Mac (bundle `app.kaeri.app`). No secret.
    Ios,
    /// The Android phone (package `app.kaeri.app`, custom URI scheme enabled). No secret.
    Android,
    /// Windows, Linux and the terminal: a loopback redirect, and a secret Google requires in the
    /// token exchange.
    Desktop,
    /// TVs and Limited Input devices: the device flow, no redirect.
    Tv,
}

impl ClientKind {
    /// Where the sign-in comes back to. `port` is the loopback listener's, for the desktop.
    /// `None` for the device flow, which has no redirect.
    pub fn redirect(self, client_id: &str, port: u16) -> Option<String> {
        match self {
            // The reversed client id is the scheme Google registers for an iOS client.
            Self::Ios => {
                let reversed: Vec<&str> = client_id.split('.').rev().collect();
                Some(format!("{}:/oauth2redirect", reversed.join(".")))
            }
            Self::Android => Some("app.kaeri.app:/oauth2redirect".into()),
            Self::Desktop => Some(format!("http://127.0.0.1:{port}")),
            Self::Tv => None,
        }
    }
}

/// The address to open in a browser for the code flow (loopback redirect on desktop, the web
/// authentication sheet on Apple platforms).
pub fn auth_url(
    endpoints: &Endpoints,
    config: &ClientConfig,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> Result<String, FolderError> {
    let mut url = reqwest::Url::parse(&endpoints.auth_url)
        .map_err(|e| FolderError::Server(format!("bad authorisation address: {e}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");
    Ok(url.into())
}

/// What a sign-in or a refresh hands back.
#[derive(Clone)]
pub struct Tokens {
    pub access_token: String,
    /// Absent on a refresh: the old one stays in force.
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

impl fmt::Debug for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tokens")
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "***"))
            .field("expires_in", &self.expires_in)
            .finish_non_exhaustive()
    }
}

/// What the person is shown on the TV.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
    pub user_code: String,
    #[serde(alias = "verification_uri")]
    pub verification_url: String,
    pub device_code: String,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Debug, Clone)]
pub enum DevicePoll {
    Pending,
    /// Wait longer between polls.
    SlowDown,
    Denied,
    Expired,
    Done(Tokens),
}

enum Reply {
    Ok(serde_json::Value),
    Failed(String),
}

fn unreachable(e: &reqwest::Error) -> FolderError {
    FolderError::Unreachable(e.to_string())
}

fn form_base(config: &ClientConfig) -> Vec<(&'static str, String)> {
    let mut form = vec![("client_id", config.client_id.clone())];
    if let Some(secret) = &config.client_secret {
        form.push(("client_secret", secret.expose().to_owned()));
    }
    form
}

async fn token_call(
    http: &reqwest::Client,
    url: &str,
    form: &[(&'static str, String)],
) -> Result<Reply, FolderError> {
    let resp = http.post(url).form(form).send().await.map_err(|e| unreachable(&e))?;
    let status = resp.status();
    let body = resp.bytes().await.map_err(|e| unreachable(&e))?;
    let json: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| FolderError::Server(format!("unreadable reply ({status})")))?;
    if status.is_success() {
        return Ok(Reply::Ok(json));
    }
    if status.is_server_error() {
        return Err(FolderError::Server(format!("sign-in service answered {status}")));
    }
    let error = json.get("error").and_then(|e| e.as_str()).unwrap_or("unknown").to_owned();
    Ok(Reply::Failed(error))
}

fn parse_tokens(json: &serde_json::Value) -> Result<Tokens, FolderError> {
    let text = |key: &str| json.get(key).and_then(|v| v.as_str()).map(str::to_owned);
    Ok(Tokens {
        access_token: text("access_token")
            .ok_or_else(|| FolderError::Server("no access token in the reply".into()))?,
        refresh_token: text("refresh_token"),
        expires_in: json.get("expires_in").and_then(serde_json::Value::as_u64).unwrap_or(3600),
    })
}

fn failed(error: &str) -> FolderError {
    if error == "invalid_grant" || error == "invalid_client" || error == "unauthorized_client" {
        FolderError::Unauthorized
    } else {
        FolderError::Server(format!("sign-in refused: {error}"))
    }
}

/// Trade the code from the redirect for tokens.
pub async fn exchange_code(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    config: &ClientConfig,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<Tokens, FolderError> {
    let mut form = form_base(config);
    form.push(("grant_type", "authorization_code".into()));
    form.push(("code", code.to_owned()));
    form.push(("redirect_uri", redirect_uri.to_owned()));
    form.push(("code_verifier", verifier.to_owned()));
    match token_call(http, &endpoints.token_url, &form).await? {
        Reply::Ok(json) => parse_tokens(&json),
        Reply::Failed(error) => Err(failed(&error)),
    }
}

/// Start the device flow: show `user_code` and `verification_url`, then poll.
pub async fn device_start(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    config: &ClientConfig,
) -> Result<DeviceCode, FolderError> {
    device_start_for(http, endpoints, config, SCOPE).await
}

/// [`device_start`] asking for `scope`: the other services' device flows are this one.
pub async fn device_start_for(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    config: &ClientConfig,
    scope: &str,
) -> Result<DeviceCode, FolderError> {
    let mut form = form_base(config);
    form.push(("scope", scope.to_owned()));
    match token_call(http, &endpoints.device_url, &form).await? {
        Reply::Ok(json) => serde_json::from_value(json)
            .map_err(|e| FolderError::Server(format!("unreadable device code: {e}"))),
        Reply::Failed(error) => Err(failed(&error)),
    }
}

/// Ask once whether the person has approved. The caller waits `interval` seconds between asks,
/// and longer after [`DevicePoll::SlowDown`].
pub async fn device_poll(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    config: &ClientConfig,
    device_code: &str,
) -> Result<DevicePoll, FolderError> {
    let mut form = form_base(config);
    form.push(("grant_type", DEVICE_GRANT.into()));
    form.push(("device_code", device_code.to_owned()));
    Ok(match token_call(http, &endpoints.token_url, &form).await? {
        Reply::Ok(json) => DevicePoll::Done(parse_tokens(&json)?),
        Reply::Failed(error) => match error.as_str() {
            "authorization_pending" => DevicePoll::Pending,
            "slow_down" => DevicePoll::SlowDown,
            "access_denied" | "authorization_declined" => DevicePoll::Denied,
            "expired_token" => DevicePoll::Expired,
            other => return Err(failed(other)),
        },
    })
}

/// Trade a refresh token for a new access token. A refused one is [`FolderError::Unauthorized`].
pub async fn refresh(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    config: &ClientConfig,
    refresh_token: &str,
) -> Result<Tokens, FolderError> {
    let mut form = form_base(config);
    form.push(("grant_type", "refresh_token".into()));
    form.push(("refresh_token", refresh_token.to_owned()));
    match token_call(http, &endpoints.token_url, &form).await? {
        Reply::Ok(json) => parse_tokens(&json),
        Reply::Failed(error) => Err(failed(&error)),
    }
}

/// Where the Drive calls get their access token.
#[async_trait]
pub trait TokenSource: Send + Sync + 'static {
    /// A token to use now, refreshed first when it is about to run out.
    async fn access_token(&self) -> Result<String, FolderError>;

    /// Drive refused `rejected`: a fresh token, or [`FolderError::Unauthorized`] when the sign-in
    /// itself is gone.
    async fn refresh_access_token(&self, rejected: &str) -> Result<String, FolderError>;
}

type OnChange = Arc<dyn Fn(&Tokens) + Send + Sync>;

struct TokenState {
    access: Option<String>,
    refresh: String,
    expires: Instant,
}

/// A [`TokenSource`] that refreshes through Google. `tokens_changed` runs whenever the pair
/// changes, so the engine can keep the refresh token in the host's secret store.
pub struct OAuthTokens {
    http: reqwest::Client,
    endpoints: Endpoints,
    config: ClientConfig,
    state: tokio::sync::Mutex<TokenState>,
    on_change: Option<OnChange>,
}

impl OAuthTokens {
    /// From a stored refresh token: the first call refreshes.
    pub fn new(
        http: reqwest::Client,
        endpoints: Endpoints,
        config: ClientConfig,
        refresh_token: String,
    ) -> Self {
        let state =
            TokenState { access: None, refresh: refresh_token, expires: Instant::now() };
        Self { http, endpoints, config, state: tokio::sync::Mutex::new(state), on_change: None }
    }

    /// From a sign-in just finished. `tokens` needs a refresh token (the consent prompt makes
    /// Google send one).
    pub fn from_signin(
        http: reqwest::Client,
        endpoints: Endpoints,
        config: ClientConfig,
        tokens: &Tokens,
    ) -> Result<Self, FolderError> {
        let refresh_token = tokens.refresh_token.clone().ok_or(FolderError::Unauthorized)?;
        let this = Self::new(http, endpoints, config, refresh_token);
        if let Ok(mut state) = this.state.try_lock() {
            state.access = Some(tokens.access_token.clone());
            state.expires = Instant::now() + Duration::from_secs(tokens.expires_in);
        }
        Ok(this)
    }

    /// Run `f` with the new tokens each time they change.
    #[must_use]
    pub fn tokens_changed(mut self, f: impl Fn(&Tokens) + Send + Sync + 'static) -> Self {
        self.on_change = Some(Arc::new(f));
        self
    }

    async fn refresh_locked(&self, state: &mut TokenState) -> Result<String, FolderError> {
        let fresh = refresh(&self.http, &self.endpoints, &self.config, &state.refresh).await?;
        if let Some(rotated) = &fresh.refresh_token {
            state.refresh.clone_from(rotated);
        }
        state.access = Some(fresh.access_token.clone());
        state.expires = Instant::now() + Duration::from_secs(fresh.expires_in);
        if let Some(on_change) = &self.on_change {
            on_change(&Tokens {
                access_token: fresh.access_token.clone(),
                refresh_token: Some(state.refresh.clone()),
                expires_in: fresh.expires_in,
            });
        }
        Ok(fresh.access_token)
    }
}

#[async_trait]
impl TokenSource for OAuthTokens {
    async fn access_token(&self) -> Result<String, FolderError> {
        let mut state = self.state.lock().await;
        match &state.access {
            Some(token) if state.expires > Instant::now() + Duration::from_secs(30) => {
                Ok(token.clone())
            }
            _ => self.refresh_locked(&mut state).await,
        }
    }

    async fn refresh_access_token(&self, rejected: &str) -> Result<String, FolderError> {
        let mut state = self.state.lock().await;
        // Another task may have refreshed while this one waited for the lock.
        match &state.access {
            Some(token) if token != rejected => Ok(token.clone()),
            _ => self.refresh_locked(&mut state).await,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DriveFile {
    id: String,
    name: String,
    #[serde(default)]
    mime_type: String,
    #[serde(default)]
    size: Option<String>,
}

impl DriveFile {
    fn is_dir(&self) -> bool {
        self.mime_type == FOLDER_MIME
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DriveList {
    #[serde(default)]
    next_page_token: Option<String>,
    #[serde(default)]
    files: Vec<DriveFile>,
}

#[derive(Deserialize)]
struct Created {
    id: String,
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn split(path: &str) -> Result<(&str, &str), FolderError> {
    check_path(path)?;
    if path.is_empty() {
        return Err(FolderError::BadPath(path.to_owned()));
    }
    Ok(path.rsplit_once('/').unwrap_or(("", path)))
}

fn error_message(body: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.pointer("/error/message").and_then(|m| m.as_str()).map(str::to_owned))
        .unwrap_or_default()
}

fn rate_limited(body: &[u8]) -> bool {
    String::from_utf8_lossy(body).contains("ateLimitExceeded")
}

/// The sync folder inside the app's hidden Drive folder.
pub struct GoogleDriveTarget {
    http: reqwest::Client,
    endpoints: Endpoints,
    tokens: Arc<dyn TokenSource>,
    folders: Mutex<HashMap<String, String>>,
}

impl GoogleDriveTarget {
    pub fn new(
        http: reqwest::Client,
        endpoints: Endpoints,
        tokens: Arc<dyn TokenSource>,
    ) -> Self {
        Self { http, endpoints, tokens, folders: Mutex::new(HashMap::new()) }
    }

    fn files_url(&self) -> String {
        format!("{}/drive/v3/files", self.endpoints.api_base)
    }

    fn cached(&self, path: &str) -> Option<String> {
        self.folders.lock().unwrap_or_else(PoisonError::into_inner).get(path).cloned()
    }

    fn forget(&self, prefix: &str) {
        let mut folders = self.folders.lock().unwrap_or_else(PoisonError::into_inner);
        if prefix.is_empty() {
            folders.clear();
        } else {
            let under = format!("{prefix}/");
            folders.retain(|path, _| path != prefix && !path.starts_with(&under));
        }
    }

    /// One call with the token attached: a 401 refreshes once, rate limits and 5xx wait and try
    /// again a few times. Returns the body of a 2xx.
    async fn call<F>(&self, build: F) -> Result<Vec<u8>, FolderError>
    where
        F: Fn(&reqwest::Client) -> reqwest::RequestBuilder + Send + Sync,
    {
        let mut token = self.tokens.access_token().await?;
        let mut refreshed = false;
        let mut tries = 0u32;
        loop {
            let resp = build(&self.http)
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| unreachable(&e))?;
            let status = resp.status();
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok());
            let body = resp.bytes().await.map_err(|e| unreachable(&e))?.to_vec();
            if status.is_success() {
                return Ok(body);
            }
            let code = status.as_u16();
            if code == 401 {
                if refreshed {
                    return Err(FolderError::Unauthorized);
                }
                token = self.tokens.refresh_access_token(&token).await?;
                refreshed = true;
                continue;
            }
            let transient =
                code == 429 || status.is_server_error() || (code == 403 && rate_limited(&body));
            if transient && tries < MAX_RETRIES {
                let wait = retry_after
                    .map_or(Duration::from_millis(500) * 2u32.pow(tries), Duration::from_secs);
                tokio::time::sleep(wait.min(MAX_WAIT)).await;
                tries += 1;
                continue;
            }
            let message = error_message(&body);
            return Err(match code {
                404 => FolderError::NotFound(message),
                _ => FolderError::Server(
                    format!("Drive answered {code} {message}").trim().to_owned(),
                ),
            });
        }
    }

    /// Everything under `parent` that matches, every page of it, smallest id first.
    async fn children(
        &self,
        parent: &str,
        name: Option<&str>,
        folders_only: bool,
    ) -> Result<Vec<DriveFile>, FolderError> {
        let mut q = format!("{} in parents and trashed = false", quote(parent));
        if let Some(name) = name {
            q.push_str(&format!(" and name = {}", quote(name)));
        }
        if folders_only {
            q.push_str(&format!(" and mimeType = {}", quote(FOLDER_MIME)));
        }
        let mut out = Vec::new();
        let mut page: Option<String> = None;
        loop {
            let body = self
                .call(|http| {
                    let mut req = http.get(self.files_url()).query(&[
                        ("spaces", ROOT),
                        ("q", q.as_str()),
                        ("fields", "nextPageToken,files(id,name,mimeType,size)"),
                        ("pageSize", "1000"),
                    ]);
                    if let Some(token) = &page {
                        req = req.query(&[("pageToken", token.as_str())]);
                    }
                    req
                })
                .await?;
            let list: DriveList = serde_json::from_slice(&body)
                .map_err(|e| FolderError::Server(format!("unreadable listing: {e}")))?;
            out.extend(list.files);
            match list.next_page_token {
                Some(next) if !next.is_empty() => page = Some(next),
                _ => break,
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    async fn create_folder(&self, parent: &str, name: &str) -> Result<(), FolderError> {
        let meta =
            serde_json::json!({ "name": name, "mimeType": FOLDER_MIME, "parents": [parent] });
        self.call(|http| http.post(self.files_url()).query(&[("fields", "id")]).json(&meta))
            .await
            .map(drop)
    }

    /// The Drive id of a folder path. With `create`, missing folders are made; without, `None`.
    async fn resolve_dir(
        &self,
        dir: &str,
        create: bool,
    ) -> Result<Option<String>, FolderError> {
        check_path(dir)?;
        let mut id = ROOT.to_owned();
        let mut path = String::new();
        for seg in dir.split('/').filter(|s| !s.is_empty()) {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(seg);
            if let Some(hit) = self.cached(&path) {
                id = hit;
                continue;
            }
            let mut found = self.children(&id, Some(seg), true).await?;
            if found.is_empty() {
                if !create {
                    return Ok(None);
                }
                self.create_folder(&id, seg).await?;
                // Another device may have made it at the same moment: the smallest id is the one.
                found = self.children(&id, Some(seg), true).await?;
            }
            let first = found
                .into_iter()
                .next()
                .ok_or_else(|| FolderError::Server("the new folder is not listed".into()))?;
            id = first.id;
            self.folders
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(path.clone(), id.clone());
        }
        Ok(Some(id))
    }

    async fn delete_id(&self, id: &str) -> Result<(), FolderError> {
        let url = format!("{}/{id}", self.files_url());
        match self.call(|http| http.delete(&url)).await {
            Ok(_) | Err(FolderError::NotFound(_)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn upload(
        &self,
        parent: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<String, FolderError> {
        let mut salt = [0u8; 8];
        SystemRandom::new().fill(&mut salt).expect("system random source");
        let boundary = format!("kaeri-{}", hex::encode(salt));
        let meta = serde_json::json!({ "name": name, "parents": [parent] });
        let mut body = format!(
            "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--").as_bytes());
        let url = format!("{}/upload/drive/v3/files", self.endpoints.api_base);
        let reply = self
            .call(|http| {
                http.post(&url)
                    .query(&[("uploadType", "multipart"), ("fields", "id")])
                    .header(
                        reqwest::header::CONTENT_TYPE,
                        format!("multipart/related; boundary={boundary}"),
                    )
                    .body(body.clone())
            })
            .await?;
        let created: Created = serde_json::from_slice(&reply)
            .map_err(|e| FolderError::Server(format!("unreadable upload reply: {e}")))?;
        Ok(created.id)
    }
}

#[async_trait]
impl FolderTarget for GoogleDriveTarget {
    async fn list(&self, dir: &str) -> Result<Vec<Entry>, FolderError> {
        let Some(id) = self.resolve_dir(dir, false).await? else { return Ok(Vec::new()) };
        let mut by_name: std::collections::BTreeMap<String, Entry> =
            std::collections::BTreeMap::new();
        // Smallest id first, so a twin name keeps the file reads will return.
        for file in self.children(&id, None, false).await? {
            let entry = Entry {
                is_dir: file.is_dir(),
                size: file.size.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0),
                name: file.name.clone(),
            };
            by_name.entry(file.name).or_insert(entry);
        }
        Ok(by_name.into_values().collect())
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, FolderError> {
        let (dir, name) = split(path)?;
        let missing = || FolderError::NotFound(path.to_owned());
        let parent = self.resolve_dir(dir, false).await?.ok_or_else(missing)?;
        let file = self
            .children(&parent, Some(name), false)
            .await?
            .into_iter()
            .find(|f| !f.is_dir())
            .ok_or_else(missing)?;
        let url = format!("{}/{}", self.files_url(), file.id);
        self.call(|http| http.get(&url).query(&[("alt", "media")])).await.map_err(|e| match e {
            FolderError::NotFound(_) => missing(),
            other => other,
        })
    }

    async fn write_new(&self, path: &str, bytes: &[u8]) -> Result<(), FolderError> {
        let (dir, name) = split(path)?;
        let mut attempt = 0;
        let (parent, id) = loop {
            let parent = self
                .resolve_dir(dir, true)
                .await?
                .ok_or_else(|| FolderError::Server("no folder".into()))?;
            if !self.children(&parent, Some(name), false).await?.is_empty() {
                return Err(FolderError::Exists(path.to_owned()));
            }
            match self.upload(&parent, name, bytes).await {
                Ok(id) => break (parent, id),
                // A folder removed elsewhere leaves a stale id in the cache: look it up again.
                Err(FolderError::NotFound(_)) if attempt == 0 => {
                    attempt += 1;
                    self.forget("");
                }
                Err(e) => return Err(e),
            }
        };
        // Two devices can pass the check above together. Both see both files now, and the one
        // with the larger id removes its own.
        let all = self.children(&parent, Some(name), false).await?;
        if all.first().is_some_and(|first| first.id != id) {
            self.delete_id(&id).await?;
            return Err(FolderError::Exists(path.to_owned()));
        }
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FolderError> {
        let (from_dir, from_name) = split(from)?;
        let (to_dir, to_name) = split(to)?;
        let missing = || FolderError::NotFound(from.to_owned());
        let from_parent = self.resolve_dir(from_dir, false).await?.ok_or_else(missing)?;
        let file = self
            .children(&from_parent, Some(from_name), false)
            .await?
            .into_iter()
            .next()
            .ok_or_else(missing)?;
        let to_parent = self
            .resolve_dir(to_dir, true)
            .await?
            .ok_or_else(|| FolderError::Server("no folder".into()))?;
        let old: Vec<DriveFile> = self
            .children(&to_parent, Some(to_name), false)
            .await?
            .into_iter()
            .filter(|f| f.id != file.id)
            .collect();
        let url = format!("{}/{}", self.files_url(), file.id);
        let meta = serde_json::json!({ "name": to_name });
        self.call(|http| {
            let mut req = http.patch(&url).query(&[("fields", "id")]).json(&meta);
            if to_parent != from_parent {
                req = req.query(&[
                    ("addParents", to_parent.as_str()),
                    ("removeParents", from_parent.as_str()),
                ]);
            }
            req
        })
        .await
        .map_err(|e| match e {
            FolderError::NotFound(_) => missing(),
            other => other,
        })?;
        for twin in old {
            self.delete_id(&twin.id).await?;
        }
        Ok(())
    }

    async fn remove(&self, path: &str) -> Result<(), FolderError> {
        let (dir, name) = split(path)?;
        let Some(parent) = self.resolve_dir(dir, false).await? else { return Ok(()) };
        for file in self.children(&parent, Some(name), false).await? {
            self.delete_id(&file.id).await?;
        }
        self.forget(path);
        Ok(())
    }

    fn describe(&self) -> String {
        "Google Drive".into()
    }
}

#[cfg(any(test, feature = "testing"))]
pub use fake::{DeviceStep, FakeGoogle};

/// A Google on the loopback interface: the token endpoints and the Drive calls this target uses,
/// with files in memory.
#[cfg(any(test, feature = "testing"))]
mod fake {
    use std::{
        collections::{BTreeMap, HashMap, HashSet, VecDeque},
        net::SocketAddr,
        sync::{Arc, Mutex, MutexGuard, PoisonError},
    };

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        task::JoinHandle,
    };

    use super::{Endpoints, FOLDER_MIME, ROOT};

    /// One scripted answer to a device poll.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DeviceStep {
        Pending,
        SlowDown,
        Denied,
        Expired,
        Done,
    }

    struct FFile {
        name: String,
        parent: String,
        mime: String,
        data: Vec<u8>,
    }

    struct State {
        files: BTreeMap<String, FFile>,
        next_id: u32,
        next_token: u32,
        valid_access: HashSet<String>,
        refresh_valid: HashMap<String, bool>,
        script: VecDeque<DeviceStep>,
        fail: Option<(u32, u16, Option<u32>)>,
        race_next_upload: bool,
        drive_requests: u32,
        token_requests: Vec<BTreeMap<String, String>>,
        page_size: usize,
    }

    struct Req {
        method: String,
        path: String,
        query: Vec<(String, String)>,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    }

    impl Req {
        fn param(&self, key: &str) -> Option<&str> {
            self.query.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
        }
    }

    struct Resp {
        status: u16,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
    }

    fn json(status: u16, value: &serde_json::Value) -> Resp {
        Resp { status, headers: Vec::new(), body: value.to_string().into_bytes() }
    }

    fn oauth_error(status: u16, error: &str) -> Resp {
        json(status, &serde_json::json!({ "error": error }))
    }

    pub struct FakeGoogle {
        addr: SocketAddr,
        state: Arc<Mutex<State>>,
        task: JoinHandle<()>,
    }

    impl Drop for FakeGoogle {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    impl FakeGoogle {
        /// The authorisation code the token endpoint accepts.
        pub const CODE: &'static str = "fake-code";
        /// The device code the device poll accepts.
        pub const DEVICE_CODE: &'static str = "fake-device-code";

        pub async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
            let addr = listener.local_addr().expect("local address");
            let state = Arc::new(Mutex::new(State {
                files: BTreeMap::new(),
                next_id: 1,
                next_token: 1,
                valid_access: HashSet::new(),
                refresh_valid: HashMap::new(),
                script: VecDeque::new(),
                fail: None,
                race_next_upload: false,
                drive_requests: 0,
                token_requests: Vec::new(),
                page_size: 2,
            }));
            let shared = Arc::clone(&state);
            let base = format!("http://{addr}");
            let task = tokio::spawn(async move {
                loop {
                    let Ok((stream, _)) = listener.accept().await else { break };
                    let shared = Arc::clone(&shared);
                    let base = base.clone();
                    tokio::spawn(async move {
                        let _ = serve(stream, &shared, &base).await;
                    });
                }
            });
            Self { addr, state, task }
        }

        pub fn endpoints(&self) -> Endpoints {
            let base = format!("http://{}", self.addr);
            Endpoints {
                auth_url: format!("{base}/auth"),
                token_url: format!("{base}/token"),
                device_url: format!("{base}/device/code"),
                api_base: base,
            }
        }

        fn st(&self) -> MutexGuard<'_, State> {
            self.state.lock().unwrap_or_else(PoisonError::into_inner)
        }

        /// Every access token handed out so far stops working.
        pub fn expire_access_tokens(&self) {
            self.st().valid_access.clear();
        }

        /// Every refresh token handed out so far is refused from now on.
        pub fn revoke_refresh_tokens(&self) {
            for valid in self.st().refresh_valid.values_mut() {
                *valid = false;
            }
        }

        /// The answers to the next device polls, in order. After the script, polls stay pending.
        pub fn script_device(&self, steps: &[DeviceStep]) {
            self.st().script = steps.iter().copied().collect();
        }

        /// The next `n` Drive requests get `status`, with a Retry-After header when given.
        pub fn fail_drive(&self, n: u32, status: u16, retry_after: Option<u32>) {
            self.st().fail = Some((n, status, retry_after));
        }

        /// The next upload finds a file of the same name already made by another device.
        pub fn race_next_upload(&self) {
            self.st().race_next_upload = true;
        }

        pub fn set_page_size(&self, n: usize) {
            self.st().page_size = n;
        }

        /// A file as another device made it, straight into the table. Returns its id.
        pub fn add_file(&self, parent: &str, name: &str, data: &[u8]) -> String {
            insert(&mut self.st(), parent, name, "application/octet-stream", data.to_vec())
        }

        /// Ids and contents of every file with this name, smallest id first.
        pub fn files_named(&self, name: &str) -> Vec<(String, Vec<u8>)> {
            self.st()
                .files
                .iter()
                .filter(|(_, f)| f.name == name)
                .map(|(id, f)| (id.clone(), f.data.clone()))
                .collect()
        }

        pub fn drive_requests(&self) -> u32 {
            self.st().drive_requests
        }

        /// The form fields of every token-endpoint request so far.
        pub fn token_requests(&self) -> Vec<BTreeMap<String, String>> {
            self.st().token_requests.clone()
        }
    }

    fn insert(st: &mut State, parent: &str, name: &str, mime: &str, data: Vec<u8>) -> String {
        let id = format!("f{:06}", st.next_id);
        st.next_id += 1;
        st.files.insert(
            id.clone(),
            FFile {
                name: name.to_owned(),
                parent: parent.to_owned(),
                mime: mime.to_owned(),
                data,
            },
        );
        id
    }

    fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
        hay.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|p| p + from)
    }

    async fn serve(
        mut stream: TcpStream,
        state: &Mutex<State>,
        _base: &str,
    ) -> std::io::Result<()> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        let head_end = loop {
            if let Some(p) = find(&buf, b"\r\n\r\n", 0) {
                break p;
            }
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                return Ok(());
            }
            buf.extend_from_slice(&chunk[..n]);
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap_or_default();
        let mut parts = request_line.split(' ');
        let method = parts.next().unwrap_or_default().to_owned();
        let target = parts.next().unwrap_or("/").to_owned();
        let headers: HashMap<String, String> = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
            .collect();
        let length: usize =
            headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
        let mut body = buf[head_end + 4..].to_vec();
        while body.len() < length {
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
        let url = reqwest::Url::parse(&format!("http://fake{target}"))
            .unwrap_or_else(|_| reqwest::Url::parse("http://fake/").expect("static url"));
        let req = Req {
            method,
            path: url.path().to_owned(),
            query: url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect(),
            headers,
            body,
        };
        let resp = {
            let mut st = state.lock().unwrap_or_else(PoisonError::into_inner);
            route(&mut st, &req)
        };
        let mut out = format!(
            "HTTP/1.1 {} Status\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            resp.status,
            resp.body.len()
        );
        for (k, v) in &resp.headers {
            out.push_str(&format!("{k}: {v}\r\n"));
        }
        out.push_str("\r\n");
        stream.write_all(out.as_bytes()).await?;
        stream.write_all(&resp.body).await?;
        stream.shutdown().await
    }

    fn route(st: &mut State, req: &Req) -> Resp {
        match (req.method.as_str(), req.path.as_str()) {
            ("POST", "/token") => token(st, req),
            ("POST", "/device/code") => device_code(st, req),
            ("GET", "/auth") => authorize(req),
            _ if req.path.starts_with("/drive/v3/files")
                || req.path.starts_with("/upload/drive/v3/files") =>
            {
                drive(st, req)
            }
            _ => json(404, &serde_json::json!({ "error": { "message": "no such route" } })),
        }
    }

    fn form(req: &Req) -> BTreeMap<String, String> {
        let text = String::from_utf8_lossy(&req.body);
        reqwest::Url::parse(&format!("http://fake/?{text}"))
            .map(|u| u.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect())
            .unwrap_or_default()
    }

    /// The sign-in page of a person who says yes at once: a redirect to where the request asked
    /// to come back to, carrying the code and the request's state.
    fn authorize(req: &Req) -> Resp {
        let Some(redirect) = req.param("redirect_uri") else {
            return oauth_error(400, "invalid_request");
        };
        let mut url = match reqwest::Url::parse(redirect) {
            Ok(url) => url,
            Err(_) => return oauth_error(400, "invalid_request"),
        };
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("code", FakeGoogle::CODE);
            if let Some(state) = req.param("state") {
                query.append_pair("state", state);
            }
        }
        Resp { status: 302, headers: vec![("Location", url.to_string())], body: Vec::new() }
    }

    fn device_code(st: &mut State, req: &Req) -> Resp {
        let form = form(req);
        st.token_requests.push(form);
        json(
            200,
            &serde_json::json!({
                "device_code": FakeGoogle::DEVICE_CODE,
                "user_code": "ABCD-EFGH",
                "verification_url": "https://www.google.example/device",
                "expires_in": 1800,
                "interval": 5,
            }),
        )
    }

    fn issue(st: &mut State, with_refresh: bool) -> Resp {
        let n = st.next_token;
        st.next_token += 1;
        let access = format!("at-{n}");
        st.valid_access.insert(access.clone());
        let mut body = serde_json::json!({
            "access_token": access,
            "expires_in": 3599,
            "token_type": "Bearer",
            "scope": super::SCOPE,
        });
        if with_refresh {
            let refresh = format!("rt-{n}");
            st.refresh_valid.insert(refresh.clone(), true);
            body["refresh_token"] = refresh.into();
        }
        json(200, &body)
    }

    fn token(st: &mut State, req: &Req) -> Resp {
        let form = form(req);
        st.token_requests.push(form.clone());
        let get = |k: &str| form.get(k).map_or("", String::as_str);
        match get("grant_type") {
            "authorization_code" => {
                if get("code") == FakeGoogle::CODE
                    && !get("code_verifier").is_empty()
                    && !get("redirect_uri").is_empty()
                {
                    issue(st, true)
                } else {
                    oauth_error(400, "invalid_grant")
                }
            }
            "refresh_token" => {
                if st.refresh_valid.get(get("refresh_token")).copied().unwrap_or(false) {
                    issue(st, false)
                } else {
                    oauth_error(400, "invalid_grant")
                }
            }
            super::DEVICE_GRANT => {
                if get("device_code") != FakeGoogle::DEVICE_CODE {
                    return oauth_error(400, "invalid_grant");
                }
                match st.script.pop_front().unwrap_or(DeviceStep::Pending) {
                    DeviceStep::Pending => oauth_error(428, "authorization_pending"),
                    DeviceStep::SlowDown => oauth_error(403, "slow_down"),
                    DeviceStep::Denied => oauth_error(403, "access_denied"),
                    DeviceStep::Expired => oauth_error(400, "expired_token"),
                    DeviceStep::Done => issue(st, true),
                }
            }
            _ => oauth_error(400, "unsupported_grant_type"),
        }
    }

    fn api_error(status: u16, reason: &str) -> Resp {
        json(
            status,
            &serde_json::json!({ "error": { "code": status, "message": reason, "errors": [{ "reason": reason }] } }),
        )
    }

    fn drive(st: &mut State, req: &Req) -> Resp {
        st.drive_requests += 1;
        if let Some((n, status, retry_after)) = st.fail {
            st.fail = (n > 1).then_some((n - 1, status, retry_after));
            let reason = if status == 403 { "rateLimitExceeded" } else { "backendError" };
            let mut resp = api_error(status, reason);
            if let Some(secs) = retry_after {
                resp.headers.push(("Retry-After", secs.to_string()));
            }
            return resp;
        }
        let bearer = req.headers.get("authorization").and_then(|v| v.strip_prefix("Bearer "));
        if !bearer.is_some_and(|t| st.valid_access.contains(t)) {
            return api_error(401, "authError");
        }
        let segments: Vec<&str> = req.path.trim_matches('/').split('/').collect();
        match (req.method.as_str(), segments.as_slice()) {
            ("GET", ["drive", "v3", "files"]) => list(st, req),
            ("POST", ["drive", "v3", "files"]) => create(st, req),
            ("POST", ["upload", "drive", "v3", "files"]) => upload(st, req),
            ("GET", ["drive", "v3", "files", id]) => match st.files.get(*id) {
                Some(f) if req.param("alt") == Some("media") => {
                    Resp { status: 200, headers: Vec::new(), body: f.data.clone() }
                }
                Some(f) => json(200, &serde_json::json!({ "id": id, "name": f.name })),
                None => api_error(404, "notFound"),
            },
            ("PATCH", ["drive", "v3", "files", id]) => patch(st, req, id),
            ("DELETE", ["drive", "v3", "files", id]) => {
                if st.files.remove(*id).is_some() {
                    Resp { status: 204, headers: Vec::new(), body: Vec::new() }
                } else {
                    api_error(404, "notFound")
                }
            }
            _ => api_error(404, "notFound"),
        }
    }

    /// The conditions of a `q` string this target writes: parent, name, mime type.
    #[derive(Default)]
    struct Filter {
        parent: Option<String>,
        name: Option<String>,
        mime: Option<String>,
    }

    fn tokenize(q: &str) -> Vec<(bool, String)> {
        let mut out = Vec::new();
        let mut chars = q.chars().peekable();
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else if c == '\'' {
                chars.next();
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => s.extend(chars.next()),
                        '\'' => break,
                        _ => s.push(c),
                    }
                }
                out.push((true, s));
            } else {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                out.push((false, s));
            }
        }
        out
    }

    fn parse_q(q: &str) -> Filter {
        let toks = tokenize(q);
        let mut f = Filter::default();
        for (i, (quoted, text)) in toks.iter().enumerate() {
            let word = |j: usize| toks.get(j).filter(|t| !t.0).map(|t| t.1.as_str());
            let value = |j: usize| toks.get(j).filter(|t| t.0).map(|t| t.1.clone());
            if *quoted && word(i + 1) == Some("in") && word(i + 2) == Some("parents") {
                f.parent = Some(text.clone());
            } else if !*quoted && word(i + 1) == Some("=") {
                match text.as_str() {
                    "name" => f.name = value(i + 2),
                    "mimeType" => f.mime = value(i + 2),
                    _ => {}
                }
            }
        }
        f
    }

    fn list(st: &State, req: &Req) -> Resp {
        let filter = parse_q(req.param("q").unwrap_or_default());
        let asked: usize = req.param("pageSize").and_then(|v| v.parse().ok()).unwrap_or(100);
        let limit = asked.min(st.page_size).max(1);
        let offset: usize = req.param("pageToken").and_then(|v| v.parse().ok()).unwrap_or(0);
        let matching: Vec<_> = st
            .files
            .iter()
            .filter(|(_, f)| {
                filter.parent.as_ref().is_none_or(|p| &f.parent == p)
                    && filter.name.as_ref().is_none_or(|n| &f.name == n)
                    && filter.mime.as_ref().is_none_or(|m| &f.mime == m)
            })
            .collect();
        let page: Vec<serde_json::Value> = matching
            .iter()
            .skip(offset)
            .take(limit)
            .map(|(id, f)| {
                let mut v = serde_json::json!({ "id": id, "name": f.name, "mimeType": f.mime });
                if f.mime != FOLDER_MIME {
                    v["size"] = f.data.len().to_string().into();
                }
                v
            })
            .collect();
        let mut body = serde_json::json!({ "files": page });
        if offset + limit < matching.len() {
            body["nextPageToken"] = (offset + limit).to_string().into();
        }
        json(200, &body)
    }

    fn create(st: &mut State, req: &Req) -> Resp {
        let Ok(meta) = serde_json::from_slice::<serde_json::Value>(&req.body) else {
            return api_error(400, "badRequest");
        };
        let name = meta["name"].as_str().unwrap_or_default();
        let parent = meta["parents"][0].as_str().unwrap_or(ROOT);
        let mime = meta["mimeType"].as_str().unwrap_or("application/octet-stream");
        if parent != ROOT && !st.files.contains_key(parent) {
            return api_error(404, "notFound");
        }
        let id = insert(st, parent, name, mime, Vec::new());
        json(200, &serde_json::json!({ "id": id }))
    }

    fn upload(st: &mut State, req: &Req) -> Resp {
        let Some(boundary) = req
            .headers
            .get("content-type")
            .and_then(|v| v.split("boundary=").nth(1))
            .map(|b| format!("--{}", b.trim()))
        else {
            return api_error(400, "badRequest");
        };
        let body = &req.body;
        let parsed = (|| {
            let first = find(body, boundary.as_bytes(), 0)?;
            let meta_start = find(body, b"\r\n\r\n", first)? + 4;
            let meta_end = find(body, format!("\r\n{boundary}").as_bytes(), meta_start)?;
            let data_start = find(body, b"\r\n\r\n", meta_end)? + 4;
            let data_end = body.len().checked_sub(format!("\r\n{boundary}--").len())?;
            let meta: serde_json::Value =
                serde_json::from_slice(body.get(meta_start..meta_end)?).ok()?;
            Some((meta, body.get(data_start..data_end)?.to_vec()))
        })();
        let Some((meta, data)) = parsed else { return api_error(400, "badRequest") };
        let name = meta["name"].as_str().unwrap_or_default();
        let parent = meta["parents"][0].as_str().unwrap_or(ROOT);
        if parent != ROOT && !st.files.contains_key(parent) {
            return api_error(404, "notFound");
        }
        if std::mem::take(&mut st.race_next_upload) {
            insert(st, parent, name, "application/octet-stream", b"theirs".to_vec());
        }
        let id = insert(st, parent, name, "application/octet-stream", data);
        json(200, &serde_json::json!({ "id": id }))
    }

    fn patch(st: &mut State, req: &Req, id: &str) -> Resp {
        let Ok(meta) = serde_json::from_slice::<serde_json::Value>(&req.body) else {
            return api_error(400, "badRequest");
        };
        let Some(file) = st.files.get_mut(id) else { return api_error(404, "notFound") };
        if let Some(name) = meta["name"].as_str() {
            name.clone_into(&mut file.name);
        }
        if let (Some(add), Some(remove)) = (req.param("addParents"), req.param("removeParents"))
            && file.parent == remove
        {
            add.clone_into(&mut file.parent);
        }
        json(200, &serde_json::json!({ "id": id }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REDIRECT: &str = "http://127.0.0.1:8123/cb";

    #[test]
    fn each_client_kind_redirects_where_it_was_registered() {
        let id = "123-abc.apps.googleusercontent.com";
        assert_eq!(
            ClientKind::Ios.redirect(id, 0).unwrap(),
            "com.googleusercontent.apps.123-abc:/oauth2redirect"
        );
        assert_eq!(
            ClientKind::Android.redirect(id, 0).unwrap(),
            "app.kaeri.app:/oauth2redirect"
        );
        assert_eq!(ClientKind::Desktop.redirect(id, 5555).unwrap(), "http://127.0.0.1:5555");
        assert_eq!(ClientKind::Tv.redirect(id, 0), None);
    }

    #[test]
    fn the_sign_in_asks_for_the_app_data_scope_alone() {
        let url = auth_url(&Endpoints::default(), &config(), REDIRECT, "c", "s").unwrap();
        let url = reqwest::Url::parse(&url).unwrap();
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q["scope"], "https://www.googleapis.com/auth/drive.appdata");
    }

    fn config() -> ClientConfig {
        ClientConfig {
            client_id: "client.example".into(),
            client_secret: Some(Secret::new("test-secret-not-real")),
        }
    }

    fn http() -> reqwest::Client {
        reqwest::Client::builder().no_proxy().build().unwrap()
    }

    async fn signed_in(fake: &FakeGoogle) -> (GoogleDriveTarget, Arc<Mutex<Vec<String>>>) {
        let ep = fake.endpoints();
        let t = exchange_code(&http(), &ep, &config(), FakeGoogle::CODE, REDIRECT, "v")
            .await
            .unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let tokens = OAuthTokens::from_signin(http(), ep.clone(), config(), &t)
            .unwrap()
            .tokens_changed(move |t| log.lock().unwrap().push(t.access_token.clone()));
        (GoogleDriveTarget::new(http(), ep, Arc::new(tokens)), seen)
    }

    async fn target(fake: &FakeGoogle) -> GoogleDriveTarget {
        signed_in(fake).await.0
    }

    #[test]
    fn pkce_challenge_matches_the_rfc_vector() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_pkce_is_long_url_safe_and_fresh() {
        let (a, b) = (Pkce::generate(), Pkce::generate());
        assert_eq!(a.verifier.len(), 43);
        assert_ne!(a.verifier, b.verifier);
        assert_eq!(a.challenge, challenge_for(&a.verifier));
        assert!(a.verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn auth_url_carries_scope_offline_and_pkce() {
        let url = auth_url(&Endpoints::default(), &config(), REDIRECT, "chal", "st").unwrap();
        let url = reqwest::Url::parse(&url).unwrap();
        assert_eq!(url.host_str(), Some("accounts.google.com"));
        let q: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q["client_id"], "client.example");
        assert_eq!(q["redirect_uri"], REDIRECT);
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["scope"], "https://www.googleapis.com/auth/drive.appdata");
        assert_eq!(q["code_challenge"], "chal");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["state"], "st");
        assert_eq!(q["access_type"], "offline");
        assert_eq!(q["prompt"], "consent");
        assert!(!url.as_str().contains("secret"));
    }

    #[test]
    fn secrets_never_print() {
        let cfg = config();
        let tokens = Tokens {
            access_token: "at-visible".into(),
            refresh_token: Some("rt-visible".into()),
            expires_in: 1,
        };
        let all = format!(
            "{cfg:?} {cfg:#?} {:?} {tokens:?} {tokens:#?} {:?}",
            cfg.client_secret,
            Pkce::generate()
        );
        for needle in ["test-secret-not-real", "at-visible", "rt-visible"] {
            assert!(!all.contains(needle), "{needle} leaked");
        }
        assert!(all.contains("Secret(***)"));
    }

    #[tokio::test]
    async fn code_exchange_returns_tokens_and_sends_the_secret() {
        let fake = FakeGoogle::start().await;
        let t = exchange_code(
            &http(),
            &fake.endpoints(),
            &config(),
            FakeGoogle::CODE,
            REDIRECT,
            "verifier",
        )
        .await
        .unwrap();
        assert!(t.refresh_token.is_some());
        assert_eq!(t.expires_in, 3599);
        let sent = &fake.token_requests()[0];
        assert_eq!(sent["client_secret"], "test-secret-not-real");
        assert_eq!(sent["code_verifier"], "verifier");
        assert_eq!(sent["redirect_uri"], REDIRECT);
    }

    #[tokio::test]
    async fn a_public_client_sends_no_secret() {
        let fake = FakeGoogle::start().await;
        let cfg = ClientConfig { client_id: "ios.example".into(), client_secret: None };
        exchange_code(&http(), &fake.endpoints(), &cfg, FakeGoogle::CODE, REDIRECT, "v")
            .await
            .unwrap();
        assert!(!fake.token_requests()[0].contains_key("client_secret"));
    }

    #[tokio::test]
    async fn a_bad_code_is_unauthorized() {
        let fake = FakeGoogle::start().await;
        let r =
            exchange_code(&http(), &fake.endpoints(), &config(), "nope", REDIRECT, "v").await;
        assert!(matches!(r, Err(FolderError::Unauthorized)));
    }

    #[tokio::test]
    async fn device_flow_pends_slows_down_then_finishes() {
        let fake = FakeGoogle::start().await;
        let ep = fake.endpoints();
        let code = device_start(&http(), &ep, &config()).await.unwrap();
        assert_eq!(code.user_code, "ABCD-EFGH");
        assert_eq!(code.verification_url, "https://www.google.example/device");
        assert_eq!((code.expires_in, code.interval), (1800, 5));
        assert!(fake.token_requests()[0]["scope"].ends_with("drive.appdata"));
        fake.script_device(&[DeviceStep::Pending, DeviceStep::SlowDown, DeviceStep::Done]);
        let (client, cfg) = (http(), config());
        let poll = || device_poll(&client, &ep, &cfg, &code.device_code);
        assert!(matches!(poll().await.unwrap(), DevicePoll::Pending));
        assert!(matches!(poll().await.unwrap(), DevicePoll::SlowDown));
        match poll().await.unwrap() {
            DevicePoll::Done(t) => assert!(t.refresh_token.is_some()),
            other => panic!("expected done, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn device_flow_denied_and_expired() {
        let fake = FakeGoogle::start().await;
        let ep = fake.endpoints();
        fake.script_device(&[DeviceStep::Denied, DeviceStep::Expired]);
        let (client, cfg) = (http(), config());
        let poll = || device_poll(&client, &ep, &cfg, FakeGoogle::DEVICE_CODE);
        assert!(matches!(poll().await.unwrap(), DevicePoll::Denied));
        assert!(matches!(poll().await.unwrap(), DevicePoll::Expired));
    }

    #[tokio::test]
    async fn refresh_gives_an_access_token_and_keeps_the_refresh_token() {
        let fake = FakeGoogle::start().await;
        let ep = fake.endpoints();
        let first = exchange_code(&http(), &ep, &config(), FakeGoogle::CODE, REDIRECT, "v")
            .await
            .unwrap();
        let next = refresh(&http(), &ep, &config(), first.refresh_token.as_deref().unwrap())
            .await
            .unwrap();
        assert_ne!(next.access_token, first.access_token);
        assert!(next.refresh_token.is_none());
    }

    #[tokio::test]
    async fn a_revoked_refresh_token_is_unauthorized() {
        let fake = FakeGoogle::start().await;
        let ep = fake.endpoints();
        let first = exchange_code(&http(), &ep, &config(), FakeGoogle::CODE, REDIRECT, "v")
            .await
            .unwrap();
        fake.revoke_refresh_tokens();
        let r = refresh(&http(), &ep, &config(), first.refresh_token.as_deref().unwrap()).await;
        assert!(matches!(r, Err(FolderError::Unauthorized)));
    }

    async fn exercise(t: &dyn FolderTarget) {
        assert!(t.list("").await.unwrap().is_empty());
        t.write_new("a/b/one.seg", b"1").await.unwrap();
        t.write_new("a/two.seg", b"22").await.unwrap();
        assert!(matches!(t.write_new("a/two.seg", b"x").await, Err(FolderError::Exists(_))));
        let root = t.list("").await.unwrap();
        assert_eq!(root.len(), 1);
        assert!(root[0].is_dir);
        let a = t.list("a").await.unwrap();
        assert_eq!(
            a.iter().map(|e| (e.name.as_str(), e.is_dir)).collect::<Vec<_>>(),
            [("b", true), ("two.seg", false)]
        );
        assert_eq!(a[1].size, 2);
        assert_eq!(t.read("a/two.seg").await.unwrap(), b"22");
        t.rename("a/two.seg", "a/three.seg").await.unwrap();
        assert!(matches!(t.read("a/two.seg").await, Err(FolderError::NotFound(_))));
        t.remove("a/three.seg").await.unwrap();
        t.remove("a/three.seg").await.unwrap();
        assert!(matches!(t.read("../x").await, Err(FolderError::BadPath(_))));
    }

    #[tokio::test]
    async fn drive_target_keeps_the_folder_contract() {
        let fake = FakeGoogle::start().await;
        exercise(&target(&fake).await).await;
    }

    #[tokio::test]
    async fn listing_pages_through_every_file() {
        let fake = FakeGoogle::start().await;
        let t = target(&fake).await;
        for i in 0..7 {
            t.write_new(&format!("log/{i}.seg"), b"x").await.unwrap();
        }
        let names: Vec<String> =
            t.list("log").await.unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, (0..7).map(|i| format!("{i}.seg")).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn nested_folders_are_made_once_and_cached() {
        let fake = FakeGoogle::start().await;
        let t = target(&fake).await;
        t.write_new("d/e/f/one.seg", b"1").await.unwrap();
        t.write_new("d/e/f/two.seg", b"2").await.unwrap();
        assert_eq!(fake.files_named("e").len(), 1);
        assert_eq!(fake.files_named("f").len(), 1);
        let before = fake.drive_requests();
        t.write_new("d/e/f/three.seg", b"3").await.unwrap();
        // Cached folders: one name check, one upload, one twin check.
        assert_eq!(fake.drive_requests() - before, 3);
        assert_eq!(t.list("d/e").await.unwrap().len(), 1);
        assert_eq!(t.list("d/e/f").await.unwrap().len(), 3);
        assert!(t.list("d/none").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_lost_duplicate_race_removes_our_copy_and_says_exists() {
        let fake = FakeGoogle::start().await;
        let t = target(&fake).await;
        t.write_new("x/seed.seg", b"s").await.unwrap();
        fake.race_next_upload();
        let r = t.write_new("x/race.seg", b"ours").await;
        assert!(matches!(r, Err(FolderError::Exists(_))));
        let left = fake.files_named("race.seg");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].1, b"theirs");
        assert_eq!(t.read("x/race.seg").await.unwrap(), b"theirs");
    }

    #[tokio::test]
    async fn twins_read_as_the_smallest_id_and_list_once() {
        let fake = FakeGoogle::start().await;
        let t = target(&fake).await;
        t.write_new("x/seed.seg", b"s").await.unwrap();
        let parent = fake.files_named("x")[0].0.clone();
        fake.add_file(&parent, "twin.seg", b"first");
        fake.add_file(&parent, "twin.seg", b"second");
        assert_eq!(t.read("x/twin.seg").await.unwrap(), b"first");
        let names: Vec<_> = t.list("x").await.unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["seed.seg", "twin.seg"]);
        t.remove("x/twin.seg").await.unwrap();
        assert!(fake.files_named("twin.seg").is_empty());
    }

    #[tokio::test]
    async fn rename_across_folders_replaces_the_target() {
        let fake = FakeGoogle::start().await;
        let t = target(&fake).await;
        t.write_new("a/src.seg", b"new").await.unwrap();
        t.write_new("b/dst.seg", b"old").await.unwrap();
        t.rename("a/src.seg", "b/dst.seg").await.unwrap();
        assert_eq!(t.read("b/dst.seg").await.unwrap(), b"new");
        assert!(t.list("a").await.unwrap().is_empty());
        assert_eq!(fake.files_named("dst.seg").len(), 1);
        assert!(matches!(
            t.rename("a/none.seg", "a/y.seg").await,
            Err(FolderError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn an_expired_access_token_refreshes_once_and_reports_it() {
        let fake = FakeGoogle::start().await;
        let (t, seen) = signed_in(&fake).await;
        t.write_new("a.seg", b"1").await.unwrap();
        assert!(seen.lock().unwrap().is_empty());
        fake.expire_access_tokens();
        assert_eq!(t.read("a.seg").await.unwrap(), b"1");
        assert_eq!(seen.lock().unwrap().len(), 1);
        // The new token is in use: no second refresh.
        t.read("a.seg").await.unwrap();
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_revoked_sign_in_is_unauthorized_on_drive_calls() {
        let fake = FakeGoogle::start().await;
        let t = target(&fake).await;
        t.write_new("a.seg", b"1").await.unwrap();
        fake.expire_access_tokens();
        fake.revoke_refresh_tokens();
        assert!(matches!(t.read("a.seg").await, Err(FolderError::Unauthorized)));
        assert!(matches!(t.list("").await, Err(FolderError::Unauthorized)));
    }

    #[tokio::test]
    async fn rate_limits_wait_and_retry() {
        let fake = FakeGoogle::start().await;
        let t = target(&fake).await;
        t.write_new("a.seg", b"1").await.unwrap();
        fake.fail_drive(2, 429, Some(0));
        assert_eq!(t.read("a.seg").await.unwrap(), b"1");
        fake.fail_drive(1, 403, Some(0));
        assert_eq!(t.read("a.seg").await.unwrap(), b"1");
    }

    #[tokio::test]
    async fn retries_are_bounded() {
        let fake = FakeGoogle::start().await;
        let t = target(&fake).await;
        fake.fail_drive(100, 503, Some(0));
        let before = fake.drive_requests();
        assert!(matches!(t.list("").await, Err(FolderError::Server(_))));
        assert_eq!(fake.drive_requests() - before, MAX_RETRIES + 1);
    }

    #[tokio::test]
    async fn a_closed_port_is_unreachable() {
        let fake = FakeGoogle::start().await;
        let ep = fake.endpoints();
        let tokens = exchange_code(&http(), &ep, &config(), FakeGoogle::CODE, REDIRECT, "v")
            .await
            .unwrap();
        drop(fake);
        let source = OAuthTokens::from_signin(http(), ep.clone(), config(), &tokens).unwrap();
        let t = GoogleDriveTarget::new(http(), ep, Arc::new(source));
        assert!(matches!(t.list("").await, Err(FolderError::Unreachable(_))));
    }

    #[tokio::test]
    async fn names_with_quotes_survive() {
        let fake = FakeGoogle::start().await;
        let t = target(&fake).await;
        t.write_new("it's/ab.seg", b"q").await.unwrap();
        assert_eq!(t.read("it's/ab.seg").await.unwrap(), b"q");
    }
}
