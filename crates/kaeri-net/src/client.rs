//! The HTTP client.

use std::time::Duration;

use kaeri_core::config::NetworkConfig;

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("building http client: {0}")]
    Build(String),

    #[error("request to {url} failed: {message}")]
    Request { url: String, message: String },

    #[error("{url} returned HTTP {status}")]
    Status { url: String, status: u16 },
}

/// Shared HTTP access.
///
/// Cheap to clone: the inner clients are reference-counted and pool connections, so a single
/// instance should be created at startup and handed around.
#[derive(Clone)]
pub struct HttpClient {
    plain: reqwest::Client,
    sources: reqwest::Client,
    /// The host names an offline client was asked to resolve and refused, in order; `None` on a
    /// client that resolves.
    refused: Option<Refused>,
}

type Refused = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient").finish_non_exhaustive()
    }
}

impl HttpClient {
    pub fn new(config: &NetworkConfig) -> Result<Self, NetError> {
        let plain = builder(config)
            .build()
            .map_err(|e| NetError::Build(format!("http client: {e}")))?;
        let sources = builder(config)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| NetError::Build(format!("sources client: {e}")))?;
        Ok(Self { plain, sources, refused: None })
    }

    /// A client that can reach no host by name, for tests that stand the app up whole.
    ///
    /// The app's own client, built from `config` as [`Self::new`] builds it, with two changes.
    /// Every name resolves to an error naming this client, at once, so a request for a real
    /// host fails before a packet (a DNS query included) leaves the machine. And no proxy is
    /// used, because a proxy is reached by its own address and resolves the name at its end,
    /// which would let a request past the resolver. An address written as an IP literal is
    /// never resolved, so a server on `127.0.0.1` answers as usual.
    ///
    /// The point is the requests a test did not think of: a tracker built with the real
    /// endpoint, a cover, a skip-time lookup. Each fails loudly in its test instead of
    /// reaching a real service with a made-up token. Hidden because nothing outside a test has
    /// a use for a client that cannot connect.
    #[doc(hidden)]
    pub fn offline(config: &NetworkConfig) -> Result<Self, NetError> {
        let refused = Refused::default();
        let offline = |builder: reqwest::ClientBuilder| {
            builder.no_proxy().dns_resolver(std::sync::Arc::new(Unresolved(refused.clone())))
        };
        let plain = offline(builder(config))
            .build()
            .map_err(|e| NetError::Build(format!("http client: {e}")))?;
        let sources = offline(builder(config).redirect(reqwest::redirect::Policy::none()))
            .build()
            .map_err(|e| NetError::Build(format!("sources client: {e}")))?;
        Ok(Self { plain, sources, refused: Some(refused) })
    }

    /// The client for Kaeri's own requests: AniList, artwork, trackers, stores, datasets.
    /// Follows redirects.
    pub fn plain(&self) -> &reqwest::Client {
        &self.plain
    }

    /// The client for what a source names: plugin fetches, the stream proxy, subtitle tracks
    /// and the cast relay. The same settings as [`Self::plain`], but it follows no redirects:
    /// a plugin's fetch is answered with the redirect itself, so the next hop is checked
    /// against the plugin's hosts again before anything reaches it.
    pub fn sources(&self) -> &reqwest::Client {
        &self.sources
    }

    /// The host names this offline client was asked for and refused, oldest first: empty on a
    /// client that resolves names. A fixture build's tests read it to see that nothing aimed at a
    /// real host.
    pub fn refused_names(&self) -> Vec<String> {
        self.refused
            .as_ref()
            .and_then(|refused| refused.lock().ok().map(|names| names.clone()))
            .unwrap_or_default()
    }
}

/// The builder both constructors start from: the platform's trust, the configured timeout and
/// Kaeri's own user agent. A plugin that wants another sets its own `user-agent` header.
fn builder(config: &NetworkConfig) -> reqwest::ClientBuilder {
    let timeout = Duration::from_secs(config.timeout_secs.max(1));
    trusted_roots(reqwest::Client::builder())
        .timeout(timeout)
        .user_agent(concat!("kaeri/", env!("CARGO_PKG_VERSION")))
}

/// `builder`, trusting what a plain client trusts on this platform. Every plain client is built
/// through this: [`HttpClient`]'s, and the VPN guard's.
///
/// Everywhere but Android, reqwest's own choice, the platform's verifier: the system's trust
/// store, with whatever the user or their organisation installed in it. Desktops and iOS are
/// left exactly as they were.
///
/// On Android that verifier reaches the system's store through JNI, and the app has to hand it
/// the JVM and a context before the first request, or every request fails. So there a plain
/// client trusts the Mozilla roots built into the binary instead, [`bundled_roots`], which need
/// nothing from the app. **The cost: a CA the user installed on the device is not trusted.** A
/// request through a proxy that inspects TLS, or to a server behind a private CA, fails on
/// Android where it works elsewhere.
pub fn trusted_roots(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    if cfg!(target_os = "android") { builder.tls_certs_only(bundled_roots()) } else { builder }
}

/// The Mozilla roots built into the binary, as reqwest takes them: what Android's plain clients
/// trust, see [`trusted_roots`].
fn bundled_roots() -> Vec<reqwest::Certificate> {
    webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .filter_map(|der| reqwest::Certificate::from_der(der).ok())
        .collect()
}

/// The resolver behind [`HttpClient::offline`]: every name is refused, without asking anyone,
/// and kept, for a test to see which hosts something tried to reach.
#[derive(Debug)]
struct Unresolved(Refused);

impl Unresolved {
    fn refuse(&self, name: &str) -> Box<dyn std::error::Error + Send + Sync> {
        if let Ok(mut refused) = self.0.lock() {
            refused.push(name.to_owned());
        }
        Self::refusal(name)
    }

    /// What a refused name fails with. Says which client refused it, so a test that trips over
    /// it learns that the request was for a real host rather than that the network is down.
    fn refusal(name: &str) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{name} was not resolved: an offline client reaches IP addresses only"),
        ))
    }
}

impl reqwest::dns::Resolve for Unresolved {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(std::future::ready(Err(self.refuse(name.as_str()))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_root_is_one_a_client_will_trust() {
        // What Android's plain clients trust, checked where the tests run: one root dropped as
        // unreadable is a host that fails there and nowhere else, and a root rustls refuses
        // fails the client's build, which is the app not starting.
        let roots = bundled_roots();
        assert_eq!(roots.len(), webpki_root_certs::TLS_SERVER_ROOT_CERTS.len());
        assert!(roots.len() > 100, "{} roots", roots.len());
        reqwest::Client::builder().tls_certs_only(roots).build().unwrap();
    }

    #[test]
    fn client_builds_from_default_config() {
        let client = HttpClient::new(&NetworkConfig::default()).unwrap();
        assert!(client.refused_names().is_empty());
    }

    /// Every message in an error's chain, outermost first: the resolver's refusal is several
    /// layers down in both clients.
    fn chain(error: &dyn std::error::Error) -> String {
        let mut said = error.to_string();
        let mut source = error.source();
        while let Some(cause) = source {
            said.push_str(" / ");
            said.push_str(&cause.to_string());
            source = cause.source();
        }
        said
    }

    #[tokio::test]
    async fn an_offline_client_reaches_loopback_and_no_named_host() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let client =
            HttpClient::offline(&NetworkConfig { timeout_secs: 5, ..Default::default() })
                .unwrap();
        let refused = "guard.example was not resolved";

        // A name is refused by the resolver: the request never gets as far as a connection, and
        // the error says why rather than looking like an outage.
        let plain = client.plain().get("http://guard.example/").send().await.unwrap_err();
        assert!(plain.is_connect(), "{plain:?}");
        assert!(chain(&plain).contains(refused), "{}", chain(&plain));
        let sources = client.sources().get("https://guard.example/").send().await.unwrap_err();
        assert!(chain(&sources).contains(refused), "{}", chain(&sources));
        // And kept, both times, for a test to see what was aimed at.
        assert_eq!(client.refused_names(), ["guard.example", "guard.example"]);
        assert!(HttpClient::new(&NetworkConfig::default()).unwrap().refused_names().is_empty());

        // An address is not a name, so a server on loopback answers.
        let listener =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut head = Vec::new();
                let mut byte = [0_u8; 1];
                while !head.ends_with(b"\r\n\r\n") && socket.read(&mut byte).await.unwrap() == 1
                {
                    head.push(byte[0]);
                }
                socket
                    .write_all(
                        b"HTTP/1.1 204 No Content\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    )
                    .await
                    .unwrap();
            }
        });
        let url = format!("http://{addr}/");
        assert_eq!(client.plain().get(&url).send().await.unwrap().status().as_u16(), 204);
        assert_eq!(client.sources().get(&url).send().await.unwrap().status().as_u16(), 204);
        server.await.unwrap();
    }

    /// One request's head as a loopback server received it, with `answer` sent back.
    async fn head_of(client: &reqwest::Client, answer: &'static [u8]) -> (String, u16) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut head = Vec::new();
            let mut byte = [0_u8; 1];
            while !head.ends_with(b"\r\n\r\n") && socket.read(&mut byte).await.unwrap() == 1 {
                head.push(byte[0]);
            }
            socket.write_all(answer).await.unwrap();
            String::from_utf8(head).unwrap()
        });
        let status = client.get(format!("http://{addr}/")).send().await.unwrap().status();
        (server.await.unwrap(), status.as_u16())
    }

    #[tokio::test]
    async fn requests_offer_every_encoding_the_client_decodes() {
        let client =
            HttpClient::offline(&NetworkConfig { timeout_secs: 5, ..Default::default() })
                .unwrap();
        let (head, _) = head_of(
            client.sources(),
            b"HTTP/1.1 204 No Content\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        let offered = head
            .lines()
            .find_map(|line| line.strip_prefix("accept-encoding: "))
            .unwrap_or_default()
            .to_owned();
        for encoding in ["gzip", "br", "deflate", "zstd"] {
            assert!(offered.contains(encoding), "{head}");
        }
    }

    #[tokio::test]
    async fn the_sources_client_hands_a_redirect_back() {
        // A plugin's fetch must see the redirect, so its next hop is checked against the
        // plugin's hosts again before anything reaches it.
        let client =
            HttpClient::offline(&NetworkConfig { timeout_secs: 5, ..Default::default() })
                .unwrap();
        let redirect: &[u8] =
            b"HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:9/\r\ncontent-length: 0\r\n\
              connection: close\r\n\r\n";
        let (_, status) = head_of(client.sources(), redirect).await;
        assert_eq!(status, 302);
    }
}
