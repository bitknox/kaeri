//! One server's HTTP: its base address, how requests sign in, and the errors told plainly.

use std::{path::Path, time::Duration};

use serde::de::DeserializeOwned;

use crate::error::{Result, ServerError};

/// How long one request may take. Page images and downloads get longer.
const TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);
/// The most redirects one request follows.
const MAX_REDIRECTS: usize = 10;

/// How requests to a server sign in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Auth {
    None,
    /// `Authorization: Basic <base64(user:password)>`, as the secure store keeps it.
    Basic(String),
    /// `Authorization: Bearer <token>`.
    Bearer(String),
    /// A named header, as Komga's `X-Auth-Token`.
    Header(&'static str, String),
}

impl Auth {
    /// The header this adds to a request, for a page the engine fetches later.
    pub fn header(&self) -> Option<(String, String)> {
        match self {
            Self::None => None,
            Self::Basic(b64) => Some(("Authorization".into(), format!("Basic {b64}"))),
            Self::Bearer(token) => Some(("Authorization".into(), format!("Bearer {token}"))),
            Self::Header(name, value) => Some(((*name).into(), value.clone())),
        }
    }

    /// The header names that carry a sign-in, which a hop to another origin must not carry.
    fn names(&self) -> [&str; 4] {
        let own = match self {
            Self::Header(name, _) => *name,
            _ => "authorization",
        };
        ["authorization", "cookie", "proxy-authorization", own]
    }
}

/// `user:password` as Basic wants it.
pub fn basic(user: &str, password: &str) -> String {
    base64(format!("{user}:{password}").as_bytes())
}

/// Standard base64 with padding. A dozen lines is cheaper than a dependency.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A server's address and the client that talks to it.
///
/// The sign-in goes only to the server's own origin: the scheme, host and port of `base`. An
/// address a feed names on another host, or on the same host over another scheme, is asked
/// without it, and so is every hop of a redirect that leaves the origin. Redirects are followed
/// here, one hop at a time, so the client handed in should follow none itself (the engine's
/// sources client); one that does is still safe for `Authorization`, which reqwest drops on a
/// change of host or port, but not for a named header or a change of scheme alone.
#[derive(Debug, Clone)]
pub struct Http {
    client: reqwest::Client,
    base: String,
    origin: Option<url::Origin>,
    auth: Auth,
}

/// The origin of `address` when it is an http or https address with a host.
fn origin_of(address: &str) -> Option<url::Origin> {
    let url = url::Url::parse(address).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| url.origin()).filter(url::Origin::is_tuple)
}

impl Http {
    /// `base` has no trailing slash.
    pub fn new(client: &reqwest::Client, base: impl Into<String>, auth: Auth) -> Self {
        let base = base.into().trim_end_matches('/').to_owned();
        Self { client: client.clone(), origin: origin_of(&base), base, auth }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn auth(&self) -> &Auth {
        &self.auth
    }

    pub fn with_auth(&self, auth: Auth) -> Self {
        Self { auth, ..self.clone() }
    }

    /// `path` on the server, or `path` itself when it is already an address.
    pub fn url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_owned()
        } else {
            format!("{}/{}", self.base, path.trim_start_matches('/'))
        }
    }

    /// Whether `address` is on the server's own origin: the same scheme, host and port.
    pub fn same_origin(&self, address: &str) -> bool {
        self.origin.is_some() && origin_of(address) == self.origin
    }

    /// The sign-in header for a request to `address`: only on the server's own origin.
    pub fn auth_for(&self, address: &str) -> Option<(String, String)> {
        self.auth.header().filter(|_| self.same_origin(address))
    }

    /// A request for `path`, signed in when it is on the server's own origin. Send it with
    /// [`Self::send`] or [`Self::fetch`], which follow redirects without the sign-in leaving
    /// the origin.
    pub fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = self.url(path);
        let mut request = self.client.request(method, &url).timeout(TIMEOUT);
        if let Some((name, value)) = self.auth_for(&url) {
            request = request.header(name, value);
        }
        request
    }

    pub async fn send(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        check(self.fetch(request).await?)
    }

    /// Send `request` and follow its redirects, any status kept. Each hop is a new request:
    /// one to another origin carries no sign-in, cookie or the server's named header, and one
    /// back on the server's origin is signed in again. A 303, or a 301 or 302 after anything
    /// but GET or HEAD, goes on as a GET without the body, as browsers do.
    pub async fn fetch(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let unreachable = |e: reqwest::Error| ServerError::Unreachable(describe(&e));
        let mut request = request.build().map_err(unreachable)?;
        for _ in 0..MAX_REDIRECTS {
            let again = request.try_clone();
            let response = self.client.execute(request).await.map_err(unreachable)?;
            let (Some(target), Some(next)) = (redirect_target(&response), again) else {
                return Ok(response);
            };
            request = self.hop(next, target, response.status());
        }
        self.client.execute(request).await.map_err(unreachable)
    }

    /// `request` sent on to `target` after a redirect with `status`.
    fn hop(
        &self,
        mut request: reqwest::Request,
        target: url::Url,
        status: reqwest::StatusCode,
    ) -> reqwest::Request {
        use reqwest::{Method, StatusCode};
        let to_get = (status == StatusCode::SEE_OTHER && request.method() != Method::HEAD)
            || (matches!(status, StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND)
                && !matches!(*request.method(), Method::GET | Method::HEAD));
        if to_get {
            *request.method_mut() = Method::GET;
            *request.body_mut() = None;
            let headers = request.headers_mut();
            headers.remove(reqwest::header::CONTENT_TYPE);
            headers.remove(reqwest::header::CONTENT_LENGTH);
        }
        let headers = request.headers_mut();
        for name in self.auth.names() {
            headers.remove(name);
        }
        if let Some((name, value)) = self.auth_for(target.as_str())
            && let (Ok(name), Ok(value)) = (
                reqwest::header::HeaderName::try_from(name),
                reqwest::header::HeaderValue::try_from(value),
            )
        {
            headers.insert(name, value);
        }
        *request.url_mut() = target;
        request
    }

    pub async fn get(&self, path: &str) -> Result<reqwest::Response> {
        self.send(self.request(reqwest::Method::GET, path)).await
    }

    pub async fn json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        read_json(self.get(path).await?).await
    }

    pub async fn text(&self, path: &str) -> Result<String> {
        self.get(path).await?.text().await.map_err(|e| ServerError::Unreachable(describe(&e)))
    }

    /// Fetch a page image or any other body whole.
    pub async fn bytes(&self, path: &str) -> Result<Vec<u8>> {
        let response = self.send(self.request(reqwest::Method::GET, path)).await?;
        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| ServerError::Unreachable(describe(&e)))
    }

    /// Download a body to `to`, a chunk at a time, refusing more than `limit` bytes. The file is
    /// removed when the download fails.
    pub async fn download(&self, path: &str, to: &Path, limit: u64) -> Result<u64> {
        use tokio::io::AsyncWriteExt;
        let mut response = self
            .send(self.request(reqwest::Method::GET, path).timeout(DOWNLOAD_TIMEOUT))
            .await?;
        if let Some(parent) = to.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ServerError::Disk(e.to_string()))?;
        }
        let mut file =
            tokio::fs::File::create(to).await.map_err(|e| ServerError::Disk(e.to_string()))?;
        let mut written = 0u64;
        let result = async {
            while let Some(chunk) =
                response.chunk().await.map_err(|e| ServerError::Unreachable(describe(&e)))?
            {
                written += chunk.len() as u64;
                if written > limit {
                    return Err(ServerError::Disk(
                        "the file is larger than Kaeri opens".into(),
                    ));
                }
                file.write_all(&chunk).await.map_err(|e| ServerError::Disk(e.to_string()))?;
            }
            file.flush().await.map_err(|e| ServerError::Disk(e.to_string()))
        }
        .await;
        drop(file);
        if let Err(error) = result {
            let _ = tokio::fs::remove_file(to).await;
            return Err(error);
        }
        Ok(written)
    }
}

/// Where a redirect answer sends the request: an http or https address, made absolute against
/// the address that answered. `None` for any other answer.
fn redirect_target(response: &reqwest::Response) -> Option<url::Url> {
    use reqwest::StatusCode;
    let status = response.status();
    if !matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    ) {
        return None;
    }
    let location = response.headers().get(reqwest::header::LOCATION)?.to_str().ok()?;
    let target = response.url().join(location).ok()?;
    matches!(target.scheme(), "http" | "https").then_some(target)
}

/// Map a status to a [`ServerError`].
pub fn check(response: reqwest::Response) -> Result<reqwest::Response> {
    match response.status().as_u16() {
        200..=299 => Ok(response),
        401 | 403 => Err(ServerError::Unauthorized),
        404 | 410 => Err(ServerError::NotFound),
        status => Err(ServerError::Status(status)),
    }
}

pub async fn read_json<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    let bytes = response.bytes().await.map_err(|e| ServerError::Unreachable(describe(&e)))?;
    serde_json::from_slice(&bytes).map_err(|e| ServerError::Parse(e.to_string()))
}

/// A transport error without the address in it: it may carry a credential in a query.
fn describe(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "the request timed out".into()
    } else if error.is_connect() {
        "the connection was refused".into()
    } else {
        // The first cause is the readable one; the url is left out.
        std::error::Error::source(error)
            .map_or_else(|| "the request failed".into(), ToString::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(basic("test", "test"), "dGVzdDp0ZXN0");
    }

    #[test]
    fn the_sign_in_goes_only_to_the_server_s_own_scheme_host_and_port() {
        let client = reqwest::Client::new();
        let http =
            Http::new(&client, "https://shelf.example:8443/opds/", Auth::Bearer("t".into()));
        assert!(http.auth_for("https://shelf.example:8443/opds/series").is_some());
        assert!(http.auth_for("https://SHELF.example:8443/pages/1").is_some());
        assert!(http.auth_for("http://shelf.example:8443/pages/1").is_none(), "plain http");
        assert!(http.auth_for("https://shelf.example/pages/1").is_none(), "another port");
        assert!(http.auth_for("https://pages.shelf.example:8443/1").is_none(), "another host");
        assert!(http.auth_for("https://shelf.example:8443@cdn.example/1").is_none());
        assert!(http.auth_for("file:///etc/hosts").is_none());
        assert!(http.auth_for("not an address").is_none());
        let request = http.request(reqwest::Method::GET, "http://shelf.example:8443/1");
        assert!(request.build().unwrap().headers().get("authorization").is_none());
        let request = http.request(reqwest::Method::GET, "series");
        assert!(request.build().unwrap().headers().get("authorization").is_some());

        // A server written with its default port is the same origin as one without.
        let http = Http::new(&client, "https://shelf.example:443", Auth::Bearer("t".into()));
        assert!(http.auth_for("https://shelf.example/x").is_some());
    }

    #[test]
    fn a_redirect_to_plain_http_on_the_same_host_and_port_drops_the_sign_in() {
        let client = reqwest::Client::new();
        for auth in [Auth::Basic(basic("u", "p")), Auth::Header("X-Auth-Token", "t".into())] {
            let http = Http::new(&client, "https://shelf.example:8443", auth);
            let request = http.request(reqwest::Method::GET, "page").build().unwrap();
            assert_eq!(request.headers().len(), 1);
            let down = url::Url::parse("http://shelf.example:8443/page").unwrap();
            let hop = http.hop(request, down, reqwest::StatusCode::FOUND);
            assert!(hop.headers().is_empty(), "{:?}", hop.headers());
            let home = url::Url::parse("https://shelf.example:8443/other").unwrap();
            let back = http.hop(hop, home, reqwest::StatusCode::FOUND);
            assert_eq!(back.headers().len(), 1);
        }
    }

    #[test]
    fn a_see_other_after_a_post_goes_on_as_a_get_without_the_body() {
        let client = reqwest::Client::new();
        let http = Http::new(&client, "https://shelf.example", Auth::None);
        let request = http
            .request(reqwest::Method::POST, "login")
            .header("content-type", "application/json")
            .body("{}")
            .build()
            .unwrap();
        let next = url::Url::parse("https://shelf.example/home").unwrap();
        let hop = http.hop(
            request.try_clone().unwrap(),
            next.clone(),
            reqwest::StatusCode::SEE_OTHER,
        );
        assert_eq!(hop.method(), reqwest::Method::GET);
        assert!(hop.body().is_none() && hop.headers().get("content-type").is_none());
        let kept = http.hop(request, next, reqwest::StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(kept.method(), reqwest::Method::POST);
        assert!(kept.body().is_some());
    }
}
