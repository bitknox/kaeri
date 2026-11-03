//! Dropbox as a folder target, in the app's own folder (the "App folder" access type): OAuth code
//! flow with PKCE and offline refresh tokens, and the v2 calls for listing, reading and writing.
//! Dropbox has no device flow, so a TV signs in through a phone.
//!
//! Paths in the app folder are Dropbox paths: a leading slash, and the root is the empty string.
//! A file is written with one upload (a segment is far below the size that needs a session), so
//! a reader never sees half of it.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    folder::{Entry, FolderError, FolderTarget, check_path},
    google::{ClientConfig, Endpoints, TokenSource},
    rest::{self, Answer},
};

/// Where Dropbox's services are, for the sign-in and for the calls.
pub fn endpoints() -> Endpoints {
    Endpoints {
        auth_url: "https://www.dropbox.com/oauth2/authorize".into(),
        token_url: "https://api.dropboxapi.com/oauth2/token".into(),
        device_url: String::new(),
        api_base: "https://api.dropboxapi.com".into(),
    }
}

/// The content host, which carries the bytes of uploads and downloads.
pub const CONTENT_BASE: &str = "https://content.dropboxapi.com";

/// The address to open in a browser for the code flow. Offline access is what gives a refresh
/// token, so the device stays signed in.
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
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("token_access_type", "offline")
        .append_pair("state", state);
    Ok(url.into())
}

#[derive(Deserialize)]
struct Listing {
    #[serde(default)]
    entries: Vec<Item>,
    #[serde(default)]
    cursor: String,
    #[serde(default)]
    has_more: bool,
}

#[derive(Deserialize)]
struct Item {
    #[serde(rename = ".tag")]
    tag: String,
    name: String,
    #[serde(default)]
    size: u64,
}

fn dbx(path: &str) -> String {
    if path.is_empty() { String::new() } else { format!("/{path}") }
}

fn summary(answer: &Answer) -> String {
    serde_json::from_slice::<serde_json::Value>(&answer.body)
        .ok()
        .and_then(|v| v.get("error_summary").and_then(|s| s.as_str()).map(str::to_owned))
        .unwrap_or_else(|| answer.text())
}

/// What a 409 means: the name Dropbox gives its reason is a path of words.
fn conflict(answer: &Answer, path: &str) -> FolderError {
    let why = summary(answer);
    if why.contains("not_found") {
        FolderError::NotFound(path.to_owned())
    } else if why.contains("conflict") {
        FolderError::Exists(path.to_owned())
    } else {
        FolderError::Server(format!("Dropbox refused: {why}"))
    }
}

fn failed(answer: &Answer) -> FolderError {
    FolderError::Server(format!("Dropbox answered {} {}", answer.status, summary(answer)))
}

pub struct DropboxTarget {
    http: reqwest::Client,
    api: String,
    content: String,
    tokens: Arc<dyn TokenSource>,
}

impl DropboxTarget {
    pub fn new(
        http: reqwest::Client,
        api_base: &str,
        content_base: &str,
        tokens: Arc<dyn TokenSource>,
    ) -> Self {
        Self {
            http,
            api: api_base.trim_end_matches('/').to_owned(),
            content: content_base.trim_end_matches('/').to_owned(),
            tokens,
        }
    }

    /// With the real services.
    pub fn standard(tokens: Arc<dyn TokenSource>) -> Result<Self, FolderError> {
        Ok(Self::new(rest::client()?, &endpoints().api_base, CONTENT_BASE, tokens))
    }

    async fn rpc(&self, route: &str, body: &serde_json::Value) -> Result<Answer, FolderError> {
        rest::call(&self.http, &*self.tokens, |http| {
            http.post(format!("{}/2/{route}", self.api)).json(body)
        })
        .await
    }

    async fn upload(
        &self,
        path: &str,
        bytes: &[u8],
        mode: &str,
    ) -> Result<Answer, FolderError> {
        let arg = serde_json::json!({
            "path": dbx(path), "mode": mode, "autorename": false, "mute": true,
        })
        .to_string();
        rest::call(&self.http, &*self.tokens, |http| {
            http.post(format!("{}/2/files/upload", self.content))
                .header("Dropbox-API-Arg", &arg)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .body(bytes.to_vec())
        })
        .await
    }
}

#[async_trait]
impl FolderTarget for DropboxTarget {
    async fn list(&self, dir: &str) -> Result<Vec<Entry>, FolderError> {
        check_path(dir)?;
        let mut out = Vec::new();
        let mut answer = self
            .rpc(
                "files/list_folder",
                &serde_json::json!({ "path": dbx(dir), "recursive": false, "limit": 1000 }),
            )
            .await?;
        loop {
            match answer.status {
                200 => {}
                409 => match conflict(&answer, dir) {
                    FolderError::NotFound(_) => return Ok(Vec::new()),
                    other => return Err(other),
                },
                _ => return Err(failed(&answer)),
            }
            let page: Listing = serde_json::from_slice(&answer.body)
                .map_err(|e| FolderError::Server(format!("unreadable listing: {e}")))?;
            out.extend(page.entries.into_iter().map(|i| Entry {
                is_dir: i.tag == "folder",
                name: i.name,
                size: i.size,
            }));
            if !page.has_more {
                break;
            }
            answer = self
                .rpc(
                    "files/list_folder/continue",
                    &serde_json::json!({ "cursor": page.cursor }),
                )
                .await?;
        }
        out.sort();
        Ok(out)
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, FolderError> {
        check_path(path)?;
        let arg = serde_json::json!({ "path": dbx(path) }).to_string();
        let answer = rest::call(&self.http, &*self.tokens, |http| {
            http.post(format!("{}/2/files/download", self.content))
                .header("Dropbox-API-Arg", &arg)
        })
        .await?;
        match answer.status {
            200 => Ok(answer.body),
            409 => Err(conflict(&answer, path)),
            _ => Err(failed(&answer)),
        }
    }

    async fn write_new(&self, path: &str, bytes: &[u8]) -> Result<(), FolderError> {
        check_path(path)?;
        let answer = self.upload(path, bytes, "add").await?;
        match answer.status {
            200 => Ok(()),
            409 => Err(conflict(&answer, path)),
            _ => Err(failed(&answer)),
        }
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FolderError> {
        // Dropbox's move refuses to replace, so the bytes go over the destination in one upload
        // and the source is removed after: the destination is never missing in between.
        let bytes = self.read(from).await?;
        check_path(to)?;
        let answer = self.upload(to, &bytes, "overwrite").await?;
        if answer.status != 200 {
            return Err(match answer.status {
                409 => conflict(&answer, to),
                _ => failed(&answer),
            });
        }
        self.remove(from).await
    }

    async fn remove(&self, path: &str) -> Result<(), FolderError> {
        check_path(path)?;
        let answer =
            self.rpc("files/delete_v2", &serde_json::json!({ "path": dbx(path) })).await?;
        match answer.status {
            200 => Ok(()),
            409 => match conflict(&answer, path) {
                // Removing what is not there is done.
                FolderError::NotFound(_) => Ok(()),
                other => Err(other),
            },
            _ => Err(failed(&answer)),
        }
    }

    fn describe(&self) -> String {
        "Dropbox".into()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{
            Mutex,
            atomic::{AtomicU32, Ordering},
        },
    };

    use super::*;
    use crate::{
        fakehttp::{Req, Resp, Server},
        folder::contract::exercise,
    };

    struct Tokens {
        current: Mutex<String>,
        refreshed: AtomicU32,
    }

    #[async_trait]
    impl TokenSource for Tokens {
        async fn access_token(&self) -> Result<String, FolderError> {
            Ok(self.current.lock().unwrap().clone())
        }

        async fn refresh_access_token(&self, _rejected: &str) -> Result<String, FolderError> {
            self.refreshed.fetch_add(1, Ordering::Relaxed);
            *self.current.lock().unwrap() = "good".into();
            Ok("good".into())
        }
    }

    #[derive(Default)]
    struct World {
        files: BTreeMap<String, Vec<u8>>,
        throttle: u32,
        calls: u32,
    }

    fn conflict(summary: &str) -> Resp {
        Resp::json(409, &serde_json::json!({ "error_summary": summary }))
    }

    fn handle(world: &Mutex<World>, req: Req) -> Resp {
        let mut w = world.lock().unwrap();
        w.calls += 1;
        if req.headers.get("authorization").map(String::as_str) != Some("Bearer good") {
            return Resp::status(401);
        }
        if w.throttle > 0 {
            w.throttle -= 1;
            return Resp::status(429).header("Retry-After", "0");
        }
        let arg: serde_json::Value = req
            .headers
            .get("dropbox-api-arg")
            .and_then(|a| serde_json::from_str(a).ok())
            .unwrap_or_else(|| serde_json::from_slice(&req.body).unwrap_or_default());
        let path = arg["path"].as_str().unwrap_or_default().trim_start_matches('/').to_owned();
        match req.path.as_str() {
            "/2/files/upload" => {
                if arg["mode"] == "add" && w.files.contains_key(&path) {
                    return conflict("path/conflict/file/..");
                }
                w.files.insert(path, req.body);
                Resp::json(200, &serde_json::json!({}))
            }
            "/2/files/download" => match w.files.get(&path) {
                Some(bytes) => Resp::bytes(bytes),
                None => conflict("path/not_found/.."),
            },
            "/2/files/delete_v2" => {
                let prefix = format!("{path}/");
                let before = w.files.len();
                w.files.retain(|k, _| *k != path && !k.starts_with(&prefix));
                if w.files.len() == before {
                    conflict("path/not_found/..")
                } else {
                    Resp::json(200, &serde_json::json!({}))
                }
            }
            "/2/files/list_folder" | "/2/files/list_folder/continue" => {
                let (dir, skip) = if req.path.ends_with("continue") {
                    let cursor = arg["cursor"].as_str().unwrap_or_default();
                    let (d, n) = cursor.rsplit_once('#').unwrap_or_default();
                    (d.to_owned(), n.parse::<usize>().unwrap_or(0))
                } else {
                    (path, 0)
                };
                let prefix = if dir.is_empty() { String::new() } else { format!("{dir}/") };
                let mut names: BTreeMap<String, bool> = BTreeMap::new();
                for (k, v) in &w.files {
                    if let Some(rest) = k.strip_prefix(&prefix) {
                        match rest.split_once('/') {
                            Some((first, _)) => {
                                names.insert(first.to_owned(), true);
                            }
                            None => {
                                names.insert(rest.to_owned(), false);
                                let _ = v;
                            }
                        }
                    }
                }
                if names.is_empty() && !dir.is_empty() {
                    return conflict("path/not_found/..");
                }
                let all: Vec<_> = names.into_iter().collect();
                let page: Vec<_> = all.iter().skip(skip).take(2).collect();
                let entries: Vec<_> = page
                    .iter()
                    .map(|(name, is_dir)| {
                        let size = w.files.get(&format!("{prefix}{name}")).map_or(0, Vec::len);
                        serde_json::json!({
                            ".tag": if *is_dir { "folder" } else { "file" },
                            "name": name, "size": size,
                        })
                    })
                    .collect();
                let more = skip + 2 < all.len();
                Resp::json(
                    200,
                    &serde_json::json!({
                        "entries": entries, "has_more": more, "cursor": format!("{dir}#{}", skip + 2),
                    }),
                )
            }
            _ => Resp::status(404),
        }
    }

    async fn fake() -> (Server, Arc<Mutex<World>>) {
        let world = Arc::new(Mutex::new(World::default()));
        let shared = world.clone();
        (Server::start(move |req| handle(&shared, req)).await, world)
    }

    fn target(server: &Server, token: &str) -> (DropboxTarget, Arc<Tokens>) {
        let tokens = Arc::new(Tokens {
            current: Mutex::new(token.to_owned()),
            refreshed: AtomicU32::new(0),
        });
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        (DropboxTarget::new(http, &server.base(), &server.base(), tokens.clone()), tokens)
    }

    #[tokio::test]
    async fn the_target_keeps_the_folder_contract() {
        let (server, _) = fake().await;
        exercise(&target(&server, "good").0).await;
    }

    #[tokio::test]
    async fn a_listing_pages_through_every_file() {
        let (server, _) = fake().await;
        let (t, _) = target(&server, "good");
        for i in 0..5 {
            t.write_new(&format!("log/{i}.seg"), b"x").await.unwrap();
        }
        let names: Vec<_> = t.list("log").await.unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["0.seg", "1.seg", "2.seg", "3.seg", "4.seg"]);
    }

    #[tokio::test]
    async fn a_rejected_token_is_refreshed_once_and_the_call_goes_through() {
        let (server, _) = fake().await;
        let (t, tokens) = target(&server, "stale");
        t.write_new("a/one.seg", b"1").await.unwrap();
        assert_eq!(tokens.refreshed.load(Ordering::Relaxed), 1);
        assert_eq!(t.read("a/one.seg").await.unwrap(), b"1");
        assert_eq!(tokens.refreshed.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn a_sign_in_that_is_gone_is_unauthorized() {
        let server = Server::start(|_| Resp::status(401)).await;
        let (t, _) = target(&server, "stale");
        assert!(matches!(t.read("a/x.seg").await, Err(FolderError::Unauthorized)));
    }

    #[tokio::test]
    async fn a_rate_limit_is_waited_out() {
        let (server, world) = fake().await;
        let (t, _) = target(&server, "good");
        world.lock().unwrap().throttle = 2;
        t.write_new("a/one.seg", b"1").await.unwrap();
        assert_eq!(t.read("a/one.seg").await.unwrap(), b"1");
    }

    #[tokio::test]
    async fn a_rewritten_file_is_never_missing() {
        let (server, world) = fake().await;
        let (t, _) = target(&server, "good");
        t.write_new("keys.bin.tmp", b"new").await.unwrap();
        t.write_new("keys.bin", b"old").await.unwrap();
        t.rename("keys.bin.tmp", "keys.bin").await.unwrap();
        let w = world.lock().unwrap();
        assert_eq!(w.files.get("keys.bin").map(Vec::as_slice), Some(&b"new"[..]));
        assert!(!w.files.contains_key("keys.bin.tmp"));
    }

    #[test]
    fn the_sign_in_address_asks_for_offline_access_with_pkce() {
        let config = ClientConfig { client_id: "client.example".into(), client_secret: None };
        let url = auth_url(&endpoints(), &config, "kaeri://auth/sync", "chal", "st").unwrap();
        let url = reqwest::Url::parse(&url).unwrap();
        assert_eq!(url.host_str(), Some("www.dropbox.com"));
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q["token_access_type"], "offline");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], "chal");
        assert_eq!(q["response_type"], "code");
    }
}
