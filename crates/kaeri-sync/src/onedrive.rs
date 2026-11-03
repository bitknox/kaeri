//! OneDrive as a folder target, in the app's own folder (`Files.ReadWrite.AppFolder`, addressed
//! as Graph's `approot`): OAuth code flow with PKCE, the device flow for TVs through the
//! `consumers` tenant, and Graph's path-addressed calls.
//!
//! A file is written with one simple upload (a segment is far below the size that needs an
//! upload session), with `conflictBehavior=fail` for a new file and `replace` for a rewritten one,
//! so a reader never sees half of it.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    folder::{Entry, FolderError, FolderTarget, check_path},
    google::{ClientConfig, Endpoints, TokenSource},
    rest::{self, Answer},
};

/// Read and write the app's own folder, and keep the sign-in alive.
pub const SCOPE: &str = "Files.ReadWrite.AppFolder offline_access";
pub const GRAPH: &str = "https://graph.microsoft.com/v1.0";

/// Where Microsoft's sign-in is. `consumers` is personal accounts only, which is what has a
/// OneDrive of its own and the device flow without a tenant.
pub fn endpoints() -> Endpoints {
    let login = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
    Endpoints {
        auth_url: format!("{login}/authorize"),
        token_url: format!("{login}/token"),
        device_url: format!("{login}/devicecode"),
        api_base: GRAPH.into(),
    }
}

/// The address to open in a browser for the code flow.
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
        .append_pair("state", state);
    Ok(url.into())
}

#[derive(Deserialize)]
struct Children {
    #[serde(default)]
    value: Vec<Item>,
    #[serde(default, rename = "@odata.nextLink")]
    next: Option<String>,
}

#[derive(Deserialize)]
struct Item {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    folder: Option<serde_json::Value>,
}

/// A path in the app folder as Graph spells it, percent-encoded by segment.
fn encoded(path: &str) -> String {
    let mut out = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn message(answer: &Answer) -> String {
    serde_json::from_slice::<serde_json::Value>(&answer.body)
        .ok()
        .and_then(|v| v.pointer("/error/code").and_then(|m| m.as_str()).map(str::to_owned))
        .unwrap_or_else(|| answer.text())
}

fn failed(answer: &Answer) -> FolderError {
    FolderError::Server(format!("OneDrive answered {} {}", answer.status, message(answer)))
}

pub struct OneDriveTarget {
    http: reqwest::Client,
    base: String,
    tokens: Arc<dyn TokenSource>,
}

impl OneDriveTarget {
    pub fn new(http: reqwest::Client, graph_base: &str, tokens: Arc<dyn TokenSource>) -> Self {
        Self { http, base: graph_base.trim_end_matches('/').to_owned(), tokens }
    }

    /// With the real services.
    pub fn standard(tokens: Arc<dyn TokenSource>) -> Result<Self, FolderError> {
        Ok(Self::new(rest::client()?, GRAPH, tokens))
    }

    fn root(&self) -> String {
        format!("{}/me/drive/special/approot", self.base)
    }

    fn item(&self, path: &str, tail: &str) -> String {
        if path.is_empty() {
            format!("{}{tail}", self.root())
        } else {
            format!("{}:/{}:{tail}", self.root(), encoded(path))
        }
    }

    async fn put(
        &self,
        path: &str,
        bytes: &[u8],
        behavior: &str,
    ) -> Result<Answer, FolderError> {
        let url = format!(
            "{}?@microsoft.graph.conflictBehavior={behavior}",
            self.item(path, "/content")
        );
        rest::call(&self.http, &*self.tokens, |http| {
            http.put(&url)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .body(bytes.to_vec())
        })
        .await
    }
}

#[async_trait]
impl FolderTarget for OneDriveTarget {
    async fn list(&self, dir: &str) -> Result<Vec<Entry>, FolderError> {
        check_path(dir)?;
        let mut out = Vec::new();
        let mut url =
            format!("{}?$top=200&$select=name,size,folder", self.item(dir, "/children"));
        loop {
            let answer = rest::call(&self.http, &*self.tokens, |http| http.get(&url)).await?;
            match answer.status {
                200 => {}
                404 => return Ok(Vec::new()),
                _ => return Err(failed(&answer)),
            }
            let page: Children = serde_json::from_slice(&answer.body)
                .map_err(|e| FolderError::Server(format!("unreadable listing: {e}")))?;
            out.extend(page.value.into_iter().map(|i| Entry {
                is_dir: i.folder.is_some(),
                name: i.name,
                size: i.size,
            }));
            match page.next {
                Some(next) if !next.is_empty() => url = next,
                _ => break,
            }
        }
        out.sort();
        Ok(out)
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, FolderError> {
        check_path(path)?;
        // The answer is a redirect to a pre-authorised address; the token is not sent on to it.
        let url = self.item(path, "/content");
        let answer = rest::call(&self.http, &*self.tokens, |http| http.get(&url)).await?;
        match answer.status {
            200 => Ok(answer.body),
            404 => Err(FolderError::NotFound(path.to_owned())),
            _ => Err(failed(&answer)),
        }
    }

    async fn write_new(&self, path: &str, bytes: &[u8]) -> Result<(), FolderError> {
        check_path(path)?;
        let answer = self.put(path, bytes, "fail").await?;
        match answer.status {
            200 | 201 => Ok(()),
            409 => Err(FolderError::Exists(path.to_owned())),
            _ => Err(failed(&answer)),
        }
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FolderError> {
        // The bytes go over the destination in one upload that replaces, and the source is
        // removed after: the destination is never missing in between.
        let bytes = self.read(from).await?;
        check_path(to)?;
        let answer = self.put(to, &bytes, "replace").await?;
        if !answer.ok() {
            return Err(failed(&answer));
        }
        self.remove(from).await
    }

    async fn remove(&self, path: &str) -> Result<(), FolderError> {
        check_path(path)?;
        let url = self.item(path, "");
        let answer = rest::call(&self.http, &*self.tokens, |http| http.delete(&url)).await?;
        match answer.status {
            // Removing what is not there is done.
            200 | 204 | 404 => Ok(()),
            _ => Err(failed(&answer)),
        }
    }

    fn describe(&self) -> String {
        "OneDrive".into()
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

    const ROOT: &str = "/me/drive/special/approot";

    fn handle(files: &Mutex<BTreeMap<String, Vec<u8>>>, base: &str, req: Req) -> Resp {
        if req.headers.get("authorization").map(String::as_str) != Some("Bearer good") {
            return Resp::status(401);
        }
        let mut files = files.lock().unwrap();
        let Some(rest) = req.path.strip_prefix(ROOT) else { return Resp::status(404) };
        // `:/a/b:/content`, `:/a/b`, `:/dir:/children`, `/children`.
        let (path, tail) = match rest.strip_prefix(":/") {
            Some(after) => match after.split_once(":/") {
                Some((p, t)) => (p.to_owned(), format!("/{t}")),
                None => (after.trim_end_matches(':').to_owned(), String::new()),
            },
            None => (String::new(), rest.to_owned()),
        };
        match (req.method.as_str(), tail.as_str()) {
            ("PUT", "/content") => {
                let fail = req.param("@microsoft.graph.conflictBehavior") == Some("fail");
                if fail && files.contains_key(&path) {
                    return Resp::status(409);
                }
                files.insert(path, req.body);
                Resp::json(201, &serde_json::json!({}))
            }
            ("GET", "/content") => match files.get(&path) {
                Some(bytes) => Resp::bytes(bytes),
                None => Resp::status(404),
            },
            ("DELETE", "") => {
                let prefix = format!("{path}/");
                let before = files.len();
                files.retain(|k, _| *k != path && !k.starts_with(&prefix));
                if files.len() == before { Resp::status(404) } else { Resp::status(204) }
            }
            ("GET", "/children") => {
                let prefix = if path.is_empty() { String::new() } else { format!("{path}/") };
                let mut names: BTreeMap<String, bool> = BTreeMap::new();
                for k in files.keys() {
                    if let Some(rest) = k.strip_prefix(&prefix) {
                        match rest.split_once('/') {
                            Some((first, _)) => names.insert(first.to_owned(), true),
                            None => names.insert(rest.to_owned(), false),
                        };
                    }
                }
                if names.is_empty() && !path.is_empty() {
                    return Resp::status(404);
                }
                let skip: usize = req.param("skip").and_then(|s| s.parse().ok()).unwrap_or(0);
                let all: Vec<_> = names.into_iter().collect();
                let value: Vec<_> = all
                    .iter()
                    .skip(skip)
                    .take(2)
                    .map(|(name, dir)| {
                        let size = files.get(&format!("{prefix}{name}")).map_or(0, Vec::len);
                        if *dir {
                            serde_json::json!({ "name": name, "folder": {} })
                        } else {
                            serde_json::json!({ "name": name, "size": size })
                        }
                    })
                    .collect();
                let mut body = serde_json::json!({ "value": value });
                if skip + 2 < all.len() {
                    body["@odata.nextLink"] = format!(
                        "{base}{ROOT}{}/children?skip={}",
                        if path.is_empty() { String::new() } else { format!(":/{path}:") },
                        skip + 2
                    )
                    .into();
                }
                Resp::json(200, &body)
            }
            _ => Resp::status(404),
        }
    }

    async fn fake() -> Server {
        let files = Mutex::new(BTreeMap::new());
        // The next link has to name the server, which is only known once it listens.
        let base = Arc::new(Mutex::new(String::new()));
        let seen = base.clone();
        let server = Server::start(move |req| {
            let b = seen.lock().unwrap().clone();
            handle(&files, &b, req)
        })
        .await;
        *base.lock().unwrap() = server.base();
        server
    }

    fn target(server: &Server, token: &str) -> (OneDriveTarget, Arc<Tokens>) {
        let tokens = Arc::new(Tokens {
            current: Mutex::new(token.to_owned()),
            refreshed: AtomicU32::new(0),
        });
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        (OneDriveTarget::new(http, &server.base(), tokens.clone()), tokens)
    }

    #[tokio::test]
    async fn the_target_keeps_the_folder_contract() {
        let server = fake().await;
        exercise(&target(&server, "good").0).await;
    }

    #[tokio::test]
    async fn a_listing_follows_the_next_link_to_every_file() {
        let server = fake().await;
        let (t, _) = target(&server, "good");
        for i in 0..5 {
            t.write_new(&format!("log/{i}.seg"), b"x").await.unwrap();
        }
        let names: Vec<_> = t.list("log").await.unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["0.seg", "1.seg", "2.seg", "3.seg", "4.seg"]);
    }

    #[tokio::test]
    async fn names_with_spaces_and_symbols_survive_the_path() {
        let server = fake().await;
        let (t, _) = target(&server, "good");
        t.write_new("a b/c#d.seg", b"1").await.unwrap();
        assert_eq!(t.read("a b/c#d.seg").await.unwrap(), b"1");
    }

    #[tokio::test]
    async fn a_rejected_token_is_refreshed_once_and_the_call_goes_through() {
        let server = fake().await;
        let (t, tokens) = target(&server, "stale");
        t.write_new("a/one.seg", b"1").await.unwrap();
        assert_eq!(tokens.refreshed.load(Ordering::Relaxed), 1);
        assert_eq!(t.read("a/one.seg").await.unwrap(), b"1");
        assert_eq!(tokens.refreshed.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn a_rename_replaces_the_destination() {
        let server = fake().await;
        let (t, _) = target(&server, "good");
        t.write_new("keys.tmp", b"new").await.unwrap();
        t.write_new("keys", b"old").await.unwrap();
        t.rename("keys.tmp", "keys").await.unwrap();
        assert_eq!(t.read("keys").await.unwrap(), b"new");
        assert!(matches!(t.read("keys.tmp").await, Err(FolderError::NotFound(_))));
    }

    #[test]
    fn the_sign_in_address_asks_for_the_app_folder_and_offline_access() {
        let config = ClientConfig { client_id: "client.example".into(), client_secret: None };
        let url = auth_url(&endpoints(), &config, "kaeri://auth/sync", "chal", "st").unwrap();
        let url = reqwest::Url::parse(&url).unwrap();
        assert!(url.path().contains("/consumers/"));
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q["scope"], "Files.ReadWrite.AppFolder offline_access");
        assert_eq!(q["code_challenge_method"], "S256");
    }
}
