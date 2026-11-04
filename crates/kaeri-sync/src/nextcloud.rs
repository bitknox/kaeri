//! Nextcloud sign-in by Login Flow v2: no registration, the server hands back an app password
//! once the person approves in a browser. Underneath it is plain WebDAV.

use std::{fmt, time::Duration};

use serde::Deserialize;

use crate::{
    folder::FolderError,
    webdav::{Password, WebDavTarget},
};

/// How long the person has to approve in the browser.
pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(20 * 60);
/// How often the server is asked whether they have.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum NextcloudError {
    #[error("not a server address: {0}")]
    BadUrl(String),
    #[error("that address is not a Nextcloud server")]
    NotNextcloud,
    #[error("the server could not be reached: {0}")]
    Unreachable(String),
    #[error("the server said: {0}")]
    Server(String),
    #[error("the sign-in was not approved in time")]
    TimedOut,
}

/// What the browser step needs: the address to open on any device, and what to poll with.
#[derive(Clone)]
pub struct LoginStart {
    pub login_url: String,
    pub poll_token: String,
    pub poll_endpoint: String,
}

impl fmt::Debug for LoginStart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginStart").field("login_url", &self.login_url).finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum Poll {
    Pending,
    Done { server: String, login_name: String, app_password: Password },
}

#[derive(Deserialize)]
struct StartReply {
    poll: PollInfo,
    login: String,
}

#[derive(Deserialize)]
struct PollInfo {
    token: String,
    endpoint: String,
}

#[derive(Deserialize)]
struct DoneReply {
    server: String,
    #[serde(rename = "loginName")]
    login_name: String,
    #[serde(rename = "appPassword")]
    app_password: String,
}

fn client() -> Result<reqwest::Client, NextcloudError> {
    reqwest::Client::builder()
        .user_agent("Kaeri")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| NextcloudError::Server(e.to_string()))
}

fn normalize(server_url: &str) -> Result<String, NextcloudError> {
    let trimmed = server_url.trim().trim_end_matches('/');
    let with_scheme =
        if trimmed.contains("://") { trimmed.to_owned() } else { format!("https://{trimmed}") };
    match reqwest::Url::parse(&with_scheme) {
        Ok(u) if matches!(u.scheme(), "http" | "https") && u.host_str().is_some() => {
            Ok(with_scheme)
        }
        _ => Err(NextcloudError::BadUrl(server_url.to_owned())),
    }
}

/// Begin a sign-in. The app password shows as "Kaeri" in the person's Nextcloud security settings.
pub async fn start(server_url: &str) -> Result<LoginStart, NextcloudError> {
    let server = normalize(server_url)?;
    let resp = client()?
        .post(format!("{server}/index.php/login/v2"))
        .send()
        .await
        .map_err(|e| NextcloudError::Unreachable(e.without_url().to_string()))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(NextcloudError::NotNextcloud);
    }
    if !resp.status().is_success() {
        return Err(NextcloudError::Server(resp.status().as_u16().to_string()));
    }
    let reply: StartReply = resp.json().await.map_err(|_| NextcloudError::NotNextcloud)?;
    Ok(LoginStart {
        login_url: reply.login,
        poll_token: reply.poll.token,
        poll_endpoint: reply.poll.endpoint,
    })
}

/// Ask once whether the person has approved. A 404 means not yet.
pub async fn poll(start: &LoginStart) -> Result<Poll, NextcloudError> {
    let resp = client()?
        .post(&start.poll_endpoint)
        .form(&[("token", start.poll_token.as_str())])
        .send()
        .await
        .map_err(|e| NextcloudError::Unreachable(e.without_url().to_string()))?;
    match resp.status().as_u16() {
        404 => Ok(Poll::Pending),
        200 => {
            let done: DoneReply =
                resp.json().await.map_err(|e| NextcloudError::Server(e.to_string()))?;
            Ok(Poll::Done {
                server: done.server,
                login_name: done.login_name,
                app_password: Password::new(done.app_password),
            })
        }
        other => Err(NextcloudError::Server(other.to_string())),
    }
}

/// Poll every `interval` until the person approves or `timeout` passes. A poll that fails to
/// reach the server ends the wait; the caller can start again.
pub async fn wait(
    start: &LoginStart,
    interval: Duration,
    timeout: Duration,
) -> Result<Poll, NextcloudError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let done @ Poll::Done { .. } = poll(start).await? {
            return Ok(done);
        }
        if tokio::time::Instant::now() + interval >= deadline {
            return Err(NextcloudError::TimedOut);
        }
        tokio::time::sleep(interval).await;
    }
}

/// The WebDAV folder for a signed-in Nextcloud: `<server>/remote.php/dav/files/<user>/<root>`.
pub fn webdav_for(
    server: &str,
    login_name: &str,
    app_password: Password,
    root: &str,
) -> Result<WebDavTarget, FolderError> {
    let mut url = reqwest::Url::parse(
        &normalize(server).map_err(|e| FolderError::BadPath(e.to_string()))?,
    )
    .map_err(|e| FolderError::BadPath(e.to_string()))?;
    if let Ok(mut segs) = url.path_segments_mut() {
        segs.pop_if_empty().extend(["remote.php", "dav", "files", login_name]);
    }
    WebDavTarget::new(url.as_str(), login_name, app_password, root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{folder::FolderTarget, webdav::FakeDav};

    #[tokio::test]
    async fn login_flow_pending_then_done_then_files() {
        let dav = FakeDav::start().await;
        let begun = start(&dav.url()).await.unwrap();
        assert!(begun.login_url.starts_with(&dav.url()));
        assert_eq!(dav.login_user_agent().as_deref(), Some("Kaeri"));
        assert!(matches!(poll(&begun).await.unwrap(), Poll::Pending));
        dav.approve_login("ann", "app-pass-1");
        let Poll::Done { server, login_name, app_password } = poll(&begun).await.unwrap()
        else {
            panic!("not done")
        };
        assert_eq!(login_name, "ann");
        assert_eq!(app_password.expose(), "app-pass-1");
        let target = webdav_for(&server, &login_name, app_password, "kaeri-sync").unwrap();
        target.check().await.unwrap();
        target.write_new("devices/a/000001.seg", b"sealed").await.unwrap();
        assert_eq!(target.read("devices/a/000001.seg").await.unwrap(), b"sealed");
        assert_eq!(dav.files(), ["remote.php/dav/files/ann/kaeri-sync/devices/a/000001.seg"]);
    }

    #[tokio::test]
    async fn wait_returns_once_the_person_approves() {
        let dav = FakeDav::start().await;
        let begun = start(&dav.url()).await.unwrap();
        let approver = dav.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            approver.approve_login("bo", "pw2");
        });
        let done =
            wait(&begun, Duration::from_millis(10), Duration::from_secs(5)).await.unwrap();
        assert!(matches!(done, Poll::Done { ref login_name, .. } if login_name == "bo"));
    }

    #[tokio::test]
    async fn wait_times_out_when_nobody_approves() {
        let dav = FakeDav::start().await;
        let begun = start(&dav.url()).await.unwrap();
        let out = wait(&begun, Duration::from_millis(10), Duration::from_millis(80)).await;
        assert!(matches!(out, Err(NextcloudError::TimedOut)));
    }

    #[tokio::test]
    async fn a_server_that_is_not_nextcloud_is_named_so() {
        let dav = FakeDav::start().await;
        dav.fail_next(1, 404);
        assert!(matches!(start(&dav.url()).await, Err(NextcloudError::NotNextcloud)));
    }

    #[tokio::test]
    async fn a_bad_address_and_a_dead_server_are_told_apart() {
        assert!(matches!(start("ftp://x.example").await, Err(NextcloudError::BadUrl(_))));
        assert!(matches!(start("").await, Err(NextcloudError::BadUrl(_))));
        let dav = FakeDav::start().await;
        let url = dav.url();
        drop(dav);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(matches!(start(&url).await, Err(NextcloudError::Unreachable(_))));
    }

    #[tokio::test]
    async fn a_server_error_while_polling_surfaces() {
        let dav = FakeDav::start().await;
        let begun = start(&dav.url()).await.unwrap();
        dav.fail_next(1, 500);
        assert!(matches!(poll(&begun).await, Err(NextcloudError::Server(_))));
    }

    #[test]
    fn the_secrets_do_not_print() {
        let s = LoginStart {
            login_url: "https://c.example/l".into(),
            poll_token: "tok3n".into(),
            poll_endpoint: "e".into(),
        };
        assert!(!format!("{s:?}").contains("tok3n"));
        let p = Poll::Done {
            server: "s".into(),
            login_name: "a".into(),
            app_password: Password::new("hunter2"),
        };
        assert!(!format!("{p:?}").contains("hunter2"));
    }

    #[test]
    fn webdav_for_builds_the_files_path() {
        let t =
            webdav_for("cloud.example/", "ann lee", Password::new("x"), "kaeri-sync").unwrap();
        assert_eq!(t.describe(), "cloud.example");
        assert!(
            format!("{t:?}").contains("https://cloud.example/remote.php/dav/files/ann%20lee")
        );
    }
}
