//! The calls the Dropbox and OneDrive targets make: a bearer token on every request, one refresh
//! after a 401, and a few waits after a rate limit or a server error. Other statuses come back to
//! the target, which knows what its service means by them.

use std::time::Duration;

use crate::{folder::FolderError, google::TokenSource};

const MAX_RETRIES: u32 = 3;
const MAX_WAIT: Duration = Duration::from_secs(10);

/// An answer that was not a success the caller should retry or give up on.
pub(crate) struct Answer {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub(crate) fn unreachable(e: &reqwest::Error) -> FolderError {
    FolderError::Unreachable(e.to_string())
}

/// Send what `build` makes with a token, handling the cases every service shares.
pub(crate) async fn call<F>(
    http: &reqwest::Client,
    tokens: &dyn TokenSource,
    build: F,
) -> Result<Answer, FolderError>
where
    F: Fn(&reqwest::Client) -> reqwest::RequestBuilder + Send + Sync,
{
    let mut token = tokens.access_token().await?;
    let mut refreshed = false;
    let mut tries = 0u32;
    loop {
        let resp = build(http).bearer_auth(&token).send().await.map_err(|e| unreachable(&e))?;
        let status = resp.status().as_u16();
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let body = resp.bytes().await.map_err(|e| unreachable(&e))?.to_vec();
        if status == 401 {
            if refreshed {
                return Err(FolderError::Unauthorized);
            }
            token = tokens.refresh_access_token(&token).await?;
            refreshed = true;
            continue;
        }
        if (status == 429 || status >= 500) && tries < MAX_RETRIES {
            let wait = retry_after
                .map_or(Duration::from_millis(500) * 2u32.pow(tries), Duration::from_secs);
            tokio::time::sleep(wait.min(MAX_WAIT)).await;
            tries += 1;
            continue;
        }
        return Ok(Answer { status, body });
    }
}

pub(crate) fn client() -> Result<reqwest::Client, FolderError> {
    reqwest::Client::builder()
        .user_agent("Kaeri")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(0)
        .build()
        .map_err(|e| FolderError::Io(e.to_string()))
}
