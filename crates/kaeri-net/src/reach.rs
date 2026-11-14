//! Whether a failed request says the device has no network or that the server did not answer.
//!
//! Read from the error alone, with no request of its own: Kaeri asks nothing of a host the
//! person did not add. No network is no route past the device (the network unreachable or
//! down) or no name server answering (a temporary resolver failure). A name the resolver says
//! does not exist, a host that cannot be reached on a network that works, a refused or reset
//! connection, a TLS failure and a timeout are the server's.

use std::error::Error;

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
