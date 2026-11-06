//! One server's HTTP: its base address, how requests sign in, and the errors told plainly.

use std::{path::Path, time::Duration};

use serde::de::DeserializeOwned;

use crate::error::{Result, ServerError};

/// How long one request may take. Page images and downloads get longer.
const TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

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
#[derive(Debug, Clone)]
pub struct Http {
    client: reqwest::Client,
    base: String,
    auth: Auth,
}

impl Http {
    /// `base` has no trailing slash.
    pub fn new(client: &reqwest::Client, base: impl Into<String>, auth: Auth) -> Self {
        Self {
            client: client.clone(),
            base: base.into().trim_end_matches('/').to_owned(),
            auth,
        }
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

    pub fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut request = self.client.request(method, self.url(path)).timeout(TIMEOUT);
        if let Some((name, value)) = self.auth.header() {
            request = request.header(name, value);
        }
        request
    }

    pub async fn send(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response =
            request.send().await.map_err(|e| ServerError::Unreachable(describe(&e)))?;
        check(response)
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
}
