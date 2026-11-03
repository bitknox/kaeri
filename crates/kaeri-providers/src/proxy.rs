//! The proxy torrent traffic can be routed through, and its check.
//!
//! Routing torrent traffic through a proxy is an optional privacy setting: unset, torrents
//! connect directly. Set, the session is built on it: librqbit's `socks_proxy_url` carries the
//! peer connections, and DHT is off, because DHT runs over UDP, which a SOCKS5 proxy often does
//! not carry. librqbit has no option to bind to a network interface, so a proxy is the one route
//! it can be given.
//!
//! The check asks the proxy itself whether it takes connections: a SOCKS5 greeting, and its
//! user name and password when the URL has them. It looks nothing up anywhere else. Torrents
//! wait for it to pass before a session exists, so a proxy that is down or refuses the
//! credentials is said once, as "The proxy check failed", rather than torrents finding no peers.

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use kaeri_core::error::ProviderError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// How long the check waits for the proxy, from connecting to its last answer.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// What the check last found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyState {
    /// A proxy is set and has not been checked yet. Torrents wait for it.
    Unchecked,
    /// No proxy is set: torrents connect directly, with nothing to check.
    Direct,
    /// The proxy took a connection, and its credentials when it asked for them.
    Passed,
    /// The proxy could not be reached, or refused the connection or its credentials. Torrents
    /// wait for the next check. The reason is for the log; it never carries the password.
    Failed { reason: String },
}

impl ProxyState {
    /// Whether torrents may run.
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Direct | Self::Passed)
    }

    /// Why torrents wait, when they do.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Failed { reason } => Some(reason),
            Self::Unchecked => Some("not checked yet"),
            Self::Direct | Self::Passed => None,
        }
    }
}

/// Where torrent traffic goes, shared with the torrent session and the interface: directly, or
/// through the proxy, with the check's latest answer.
#[derive(Clone)]
pub struct TorrentRoute {
    /// The proxy's URL, its password in it when there is one. Never logged or shown.
    url: Option<String>,
    state: Arc<RwLock<ProxyState>>,
    /// A fixture build's route: its check passes without connecting anywhere.
    fixed: bool,
}

impl std::fmt::Debug for TorrentRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TorrentRoute")
            .field("proxy", &self.url.is_some())
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl TorrentRoute {
    /// The route `proxy` names: directly when it is `None`, else through that SOCKS5 URL.
    pub fn new(proxy: Option<String>) -> Result<Self, String> {
        let url = proxy.filter(|url| !url.trim().is_empty());
        if let Some(url) = &url {
            target(url)?;
        }
        let state = if url.is_some() { ProxyState::Unchecked } else { ProxyState::Direct };
        Ok(Self { url, state: Arc::new(RwLock::new(state)), fixed: false })
    }

    /// A route for a fixture build: its check passes without connecting anywhere, so a
    /// screenshot never depends on a proxy being there.
    pub fn fixed(proxy: Option<String>) -> Result<Self, String> {
        Ok(Self { fixed: true, ..Self::new(proxy)? })
    }

    /// The proxy's URL, password and all: for the torrent session's options and nothing else.
    pub fn proxy_url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// Whether torrents go through a proxy.
    pub fn proxied(&self) -> bool {
        self.url.is_some()
    }

    /// Whether DHT is off: through a proxy, yes. See the module's own words.
    pub fn must_disable_dht(&self) -> bool {
        self.proxied()
    }

    pub fn state(&self) -> ProxyState {
        match self.state.read() {
            Ok(g) => g.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn set(&self, next: ProxyState) {
        match self.state.write() {
            Ok(mut g) => *g = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
    }

    /// Whether the torrent source may run right now.
    ///
    /// Returns [`ProviderError::Unavailable`] rather than a health error: a source waiting for
    /// its proxy is not a broken source.
    pub fn permit(&self) -> Result<(), ProviderError> {
        let state = self.state();
        if state.is_ready() {
            return Ok(());
        }
        Err(ProviderError::Unavailable("The proxy check failed".to_owned()))
    }

    /// Check the proxy and keep what it found. Directly, there is nothing to check.
    pub async fn verify(&self) -> ProxyState {
        let next = match (&self.url, self.fixed) {
            (None, _) => ProxyState::Direct,
            (Some(_), true) => ProxyState::Passed,
            (Some(url), false) => match handshake(url, CHECK_TIMEOUT).await {
                Ok(()) => ProxyState::Passed,
                Err(reason) => ProxyState::Failed { reason },
            },
        };
        tracing::debug!(state = ?next, "torrent proxy checked");
        self.set(next.clone());
        next
    }
}

/// Where a SOCKS5 URL points, and the user name and password in it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    host: String,
    port: u16,
    credentials: Option<(String, String)>,
}

/// `url`'s target, or why it is not a SOCKS5 URL the check can use. The reason never repeats
/// the URL, which may hold a password.
fn target(url: &str) -> Result<Target, String> {
    let rest = url
        .trim()
        .strip_prefix("socks5h://")
        .or_else(|| url.trim().strip_prefix("socks5://"))
        .ok_or("the proxy is not a socks5:// address")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let (credentials, address) = match authority.rsplit_once('@') {
        Some((credentials, address)) => (Some(credentials), address),
        None => (None, authority),
    };
    let (host, port) = address.rsplit_once(':').ok_or("the proxy has no port")?;
    let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    let port: u16 =
        port.parse().ok().filter(|p| *p != 0).ok_or("the proxy's port is not a port")?;
    if host.is_empty() {
        return Err("the proxy has no host".into());
    }
    let credentials = credentials.map(|c| {
        let (user, password) = c.split_once(':').unwrap_or((c, ""));
        (unescape(user), unescape(password))
    });
    Ok(Target { host: host.to_owned(), port, credentials })
}

/// Greet the proxy at `url` as SOCKS5 asks (RFC 1928), and sign in with its user name and
/// password when it wants them (RFC 1929). Nothing is asked of it beyond that.
async fn handshake(url: &str, timeout: Duration) -> Result<(), String> {
    let target = target(url)?;
    tokio::time::timeout(timeout, greet(&target))
        .await
        .map_err(|_| "the proxy did not answer in time".to_owned())?
}

async fn greet(target: &Target) -> Result<(), String> {
    let mut stream = tokio::net::TcpStream::connect((target.host.as_str(), target.port))
        .await
        .map_err(|e| format!("could not connect to the proxy: {e}"))?;
    // Version 5, offering no authentication, and user name and password when there is one.
    let offer: &[u8] = if target.credentials.is_some() { &[5, 2, 0, 2] } else { &[5, 1, 0] };
    stream
        .write_all(offer)
        .await
        .map_err(|e| format!("the proxy closed the connection: {e}"))?;
    let mut choice = [0u8; 2];
    stream
        .read_exact(&mut choice)
        .await
        .map_err(|e| format!("the proxy did not answer as SOCKS5: {e}"))?;
    match choice {
        [5, 0] => Ok(()),
        [5, 2] => {
            let Some((user, password)) = &target.credentials else {
                return Err("the proxy asks for a user name and password".into());
            };
            let (user, password) = (user.as_bytes(), password.as_bytes());
            let (Ok(user_len), Ok(password_len)) =
                (u8::try_from(user.len()), u8::try_from(password.len()))
            else {
                return Err("the proxy's user name or password is too long".into());
            };
            let mut login = vec![1, user_len];
            login.extend_from_slice(user);
            login.push(password_len);
            login.extend_from_slice(password);
            stream
                .write_all(&login)
                .await
                .map_err(|e| format!("the proxy closed the connection: {e}"))?;
            let mut status = [0u8; 2];
            stream
                .read_exact(&mut status)
                .await
                .map_err(|e| format!("the proxy did not answer the sign-in: {e}"))?;
            if status[1] == 0 {
                Ok(())
            } else {
                Err("the proxy refused the user name or password".into())
            }
        }
        [5, 0xff] => Err("the proxy accepts none of the ways to sign in".into()),
        _ => Err("the proxy did not answer as SOCKS5".into()),
    }
}

/// Percent escapes undone. A `%` that starts no escape is kept as it is.
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let escaped = (bytes[at] == b'%')
            .then(|| text.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                at += 3;
            }
            None => {
                out.push(bytes[at]);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-connection SOCKS5 proxy on loopback that answers the greeting with `method` and,
    /// for user name and password, accepts only `mika:lantern`.
    async fn proxy(method: u8) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut head = [0u8; 2];
            socket.read_exact(&mut head).await.unwrap();
            let mut methods = vec![0u8; head[1] as usize];
            socket.read_exact(&mut methods).await.unwrap();
            socket.write_all(&[5, method]).await.unwrap();
            if method == 2 {
                let mut version_len = [0u8; 2];
                socket.read_exact(&mut version_len).await.unwrap();
                let mut user = vec![0u8; version_len[1] as usize];
                socket.read_exact(&mut user).await.unwrap();
                let mut len = [0u8; 1];
                socket.read_exact(&mut len).await.unwrap();
                let mut password = vec![0u8; len[0] as usize];
                socket.read_exact(&mut password).await.unwrap();
                let ok = user == b"mika" && password == b"lantern";
                socket.write_all(&[1, u8::from(!ok)]).await.unwrap();
            }
        });
        format!("127.0.0.1:{port}")
    }

    #[test]
    fn with_no_proxy_torrents_connect_directly_and_need_no_check() {
        let route = TorrentRoute::new(None).unwrap();
        assert_eq!(route.state(), ProxyState::Direct);
        assert!(route.permit().is_ok());
        assert!(!route.must_disable_dht());
        assert!(TorrentRoute::new(Some("  ".into())).unwrap().proxy_url().is_none());
    }

    #[test]
    fn a_proxy_not_checked_yet_holds_torrents_and_turns_dht_off() {
        let route = TorrentRoute::new(Some("socks5://127.0.0.1:1080".into())).unwrap();
        assert_eq!(route.state(), ProxyState::Unchecked);
        assert!(route.must_disable_dht());
        match route.permit() {
            Err(ProviderError::Unavailable(reason)) => {
                assert_eq!(reason, "The proxy check failed")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_address_that_is_not_socks5_is_refused_without_repeating_it() {
        let err =
            TorrentRoute::new(Some("http://mika:lantern@10.0.0.1:8080".into())).unwrap_err();
        assert!(!err.contains("lantern"), "{err}");
        assert!(TorrentRoute::new(Some("socks5://10.0.0.1".into())).is_err(), "no port");
        let parsed = target("socks5h://mika%40home:p%3Ass@[fe80::1]:1080").unwrap();
        assert_eq!(parsed.host, "fe80::1");
        assert_eq!(parsed.credentials, Some(("mika@home".into(), "p:ss".into())));
    }

    #[tokio::test]
    async fn a_proxy_that_takes_the_connection_passes() {
        let route = TorrentRoute::new(Some(format!("socks5://{}", proxy(0).await))).unwrap();
        assert_eq!(route.verify().await, ProxyState::Passed);
        assert!(route.permit().is_ok());
    }

    #[tokio::test]
    async fn the_proxy_checks_the_user_name_and_password() {
        let good = TorrentRoute::new(Some(format!("socks5://mika:lantern@{}", proxy(2).await)));
        assert_eq!(good.unwrap().verify().await, ProxyState::Passed);
        let bad = TorrentRoute::new(Some(format!("socks5://mika:wrong@{}", proxy(2).await)));
        let state = bad.unwrap().verify().await;
        assert!(matches!(&state, ProxyState::Failed { reason } if !reason.contains("wrong")));
        let none = TorrentRoute::new(Some(format!("socks5://{}", proxy(2).await)));
        assert!(matches!(none.unwrap().verify().await, ProxyState::Failed { .. }));
    }

    #[tokio::test]
    async fn a_proxy_nothing_listens_on_fails_and_holds_torrents() {
        let route = TorrentRoute::new(Some("socks5://127.0.0.1:1".into())).unwrap();
        assert!(matches!(route.verify().await, ProxyState::Failed { .. }));
        assert!(route.permit().is_err());
    }

    #[tokio::test]
    async fn a_fixture_route_passes_without_connecting() {
        let route = TorrentRoute::fixed(Some("socks5://127.0.0.1:1".into())).unwrap();
        assert_eq!(route.verify().await, ProxyState::Passed);
    }

    #[test]
    fn the_state_is_shared_across_clones() {
        let route = TorrentRoute::new(Some("socks5://127.0.0.1:1080".into())).unwrap();
        let clone = route.clone();
        route.set(ProxyState::Passed);
        assert_eq!(clone.state(), ProxyState::Passed);
    }
}
