//! Whether a failed request says the device has no network or that the server did not answer.
//!
//! Read from the error alone, with no request of its own: Kaeri asks nothing of a host the
//! person did not add. No network is no route past the device (the network unreachable or
//! down) or no name server answering (a temporary resolver failure). A name the resolver says
//! does not exist, a host that cannot be reached on a network that works, a refused or reset
//! connection, a TLS failure and a timeout are the server's.
//!
//! An error alone cannot tell everything. A path that is switching (a VPN coming up, Wi-Fi
//! handing over) fails each request as a reset or a timeout, which reads as the server's. What
//! tells it apart is that every host fails at once. [`Ledger`] keeps what the requests through
//! one client lately found, host by host, and what the platform last said of the path, so a
//! failure can be weighed against what the other hosts did meanwhile.

use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex};

use kaeri_core::error::ProviderError;

/// Whether `error` failed because the device has no network.
pub fn no_network(error: &reqwest::Error) -> bool {
    let mut link: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(at) = link {
        if let Some(io) = at.downcast_ref::<std::io::Error>()
            && io_says_no_network(io)
        {
            return true;
        }
        if resolver_says_no_network(&at.to_string()) {
            return true;
        }
        link = at.source();
    }
    false
}

/// `error` as a [`ProviderError::Transport`], marked as no network when it is one.
pub fn transport(error: &reqwest::Error) -> ProviderError {
    ProviderError::Transport(described(error))
}

/// What a [`ProviderError::Transport`] made of `error` says: its words, after
/// [`NO_NETWORK`](kaeri_core::error::NO_NETWORK) when the device has no network.
pub fn described(error: &reqwest::Error) -> String {
    if no_network(error) {
        format!("{}: {error}", kaeri_core::error::NO_NETWORK)
    } else {
        error.to_string()
    }
}

/// How many outcomes a [`Ledger`] keeps: enough for every request a screen and the plugins
/// make together, few enough that nothing grows with time.
const KEPT: usize = 256;

/// What the requests through one client found lately, host by host, and whether the platform
/// says there is no path at all. Shared by every clone of the client that holds it.
///
/// Only whether a host answered is kept: any reply, an error status included, says the device
/// reached it. Nothing here sends a request of its own.
#[derive(Debug, Clone, Default)]
pub struct Ledger(Arc<Mutex<Kept>>);

#[derive(Debug, Default)]
struct Kept {
    /// The number the next outcome gets.
    next: u64,
    /// The platform's word: no usable path.
    offline: bool,
    outcomes: VecDeque<Outcome>,
}

#[derive(Debug)]
struct Outcome {
    at: u64,
    host: String,
    answered: bool,
}

/// A point in a [`Ledger`]: the outcomes noted after it are the ones [`Ledger::others`] reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mark(u64);

/// What hosts other than one did after a [`Mark`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Others {
    /// Some other host answered.
    pub answered: bool,
    /// Some other host could not be reached.
    pub failed: bool,
    /// What the first of them did: `Some(true)` when it answered.
    pub first_answered: Option<bool>,
}

impl Ledger {
    /// The platform says whether it has a usable path, each time that changes.
    pub fn path(&self, offline: bool) {
        if let Ok(mut kept) = self.0.lock() {
            kept.offline = offline;
        }
    }

    /// Whether the platform last said there is no usable path.
    pub fn offline(&self) -> bool {
        self.0.lock().map(|kept| kept.offline).unwrap_or(false)
    }

    /// Now, to read what comes after it with [`Self::others`].
    pub fn mark(&self) -> Mark {
        Mark(self.0.lock().map(|kept| kept.next).unwrap_or(0))
    }

    /// A request to `url` got a reply (`answered`), or failed before one came.
    pub fn note(&self, url: &str, answered: bool) {
        let Some(host) = host(url) else { return };
        let Ok(mut kept) = self.0.lock() else { return };
        let at = kept.next;
        kept.next += 1;
        if kept.outcomes.len() == KEPT {
            kept.outcomes.pop_front();
        }
        kept.outcomes.push_back(Outcome { at, host, answered });
    }

    /// What every host but `url`'s did after `since`.
    pub fn others(&self, url: &str, since: Mark) -> Others {
        let mine = host(url);
        let Ok(kept) = self.0.lock() else { return Others::default() };
        let mut others = Others::default();
        for outcome in kept.outcomes.iter().filter(|o| o.at >= since.0) {
            if mine.as_deref() == Some(outcome.host.as_str()) {
                continue;
            }
            others.first_answered.get_or_insert(outcome.answered);
            if outcome.answered {
                others.answered = true;
            } else {
                others.failed = true;
            }
        }
        others
    }
}

/// The host and port `url` names, which is what tells two services apart.
fn host(url: &str) -> Option<String> {
    let url = reqwest::Url::parse(url).ok()?;
    Some(format!("{}:{}", url.host_str()?, url.port_or_known_default()?))
}

fn io_says_no_network(error: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    // WSATRY_AGAIN: Windows' resolver found no name server to ask.
    const WSATRY_AGAIN: i32 = 11002;
    matches!(error.kind(), ErrorKind::NetworkUnreachable | ErrorKind::NetworkDown)
        || (cfg!(windows) && error.raw_os_error() == Some(WSATRY_AGAIN))
}

/// Whether a resolver's words say no name server answered (`EAI_AGAIN`), as the C libraries
/// on Linux, Android and Apple's systems word it, rather than that the name does not exist.
fn resolver_says_no_network(said: &str) -> bool {
    said.to_ascii_lowercase().contains("temporary failure in name resolution")
}

#[cfg(test)]
mod tests {
    use std::{io, sync::Arc};

    use super::*;

    /// A resolver that fails every name with `error`, standing in for the system's.
    struct Failing(Arc<dyn Fn() -> io::Error + Send + Sync>);

    impl reqwest::dns::Resolve for Failing {
        fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
            let error: Box<dyn Error + Send + Sync> = Box::new((self.0)());
            Box::pin(std::future::ready(Err(error)))
        }
    }

    async fn failed_with(
        resolver: impl Fn() -> io::Error + Send + Sync + 'static,
    ) -> reqwest::Error {
        let client = reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(Arc::new(Failing(Arc::new(resolver))))
            .build()
            .unwrap();
        client.get("http://shelf.example/").send().await.unwrap_err()
    }

    #[test]
    fn a_ledger_reads_what_the_other_hosts_did_after_a_mark() {
        let ledger = Ledger::default();
        ledger.note("https://meta.example/graphql", false);
        let mark = ledger.mark();
        ledger.note("https://meta.example/graphql", false);
        assert_eq!(ledger.others("https://meta.example/graphql", mark), Others::default());

        ledger.note("https://fallback.example/v1/anime", false);
        ledger.note("https://shelf.example:8443/", true);
        let others = ledger.others("https://meta.example/graphql", mark);
        assert_eq!(
            others,
            Others { answered: true, failed: true, first_answered: Some(false) }
        );
        // The same name on another port is another service.
        let others = ledger.others("https://shelf.example/", ledger.mark());
        assert_eq!(others, Others::default());
        assert!(ledger.others("https://shelf.example/", mark).answered);
    }

    #[test]
    fn a_ledger_keeps_the_platforms_word_and_a_bounded_past() {
        let ledger = Ledger::default();
        assert!(!ledger.offline());
        ledger.clone().path(true);
        assert!(ledger.offline(), "clones share it");
        ledger.path(false);
        assert!(!ledger.offline());

        for _ in 0..KEPT * 2 {
            ledger.note("https://fallback.example/", false);
        }
        ledger.note("not a url", true);
        assert_eq!(ledger.0.lock().unwrap().outcomes.len(), KEPT);
        assert!(!ledger.others("https://meta.example/", Mark::default()).answered);
    }

    #[tokio::test]
    async fn no_name_server_answering_is_no_network() {
        let error = failed_with(|| {
            io::Error::other(
                "failed to lookup address information: Temporary failure in name resolution",
            )
        })
        .await;
        assert!(no_network(&error), "{error:?}");
        assert!(transport(&error).is_no_network());
    }

    #[tokio::test]
    async fn no_route_past_the_device_is_no_network() {
        for kind in [io::ErrorKind::NetworkUnreachable, io::ErrorKind::NetworkDown] {
            let error = failed_with(move || io::Error::from(kind)).await;
            assert!(no_network(&error), "{kind:?}: {error:?}");
        }
    }

    #[tokio::test]
    async fn a_name_that_does_not_exist_is_the_source() {
        for said in [
            "failed to lookup address information: Name or service not known",
            "failed to lookup address information: nodename nor servname provided, or not known",
        ] {
            let error = failed_with(move || io::Error::other(said)).await;
            assert!(!no_network(&error), "{said}");
            assert!(!transport(&error).is_no_network());
        }
    }

    #[tokio::test]
    async fn a_host_unreachable_on_a_working_network_is_the_source() {
        let error = failed_with(|| io::Error::from(io::ErrorKind::HostUnreachable)).await;
        assert!(!no_network(&error), "{error:?}");
    }

    #[tokio::test]
    async fn a_refused_connection_is_the_source() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let error = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("http://{address}/"))
            .send()
            .await
            .unwrap_err();
        assert!(!no_network(&error), "{error:?}");
    }

    #[tokio::test]
    async fn a_server_that_never_answers_is_the_source() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let held = tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
        });
        let error = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_millis(200))
            .build()
            .unwrap()
            .get(format!("http://{address}/"))
            .send()
            .await
            .unwrap_err();
        assert!(error.is_timeout(), "{error:?}");
        assert!(!no_network(&error), "{error:?}");
        held.abort();
    }
}
