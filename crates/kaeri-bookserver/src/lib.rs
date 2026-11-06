//! Clients for the book servers a person runs: Komga, Kavita and OPDS catalogues.
//!
//! Each is a [`BookSource`](kaeri_core::traits::BookSource), so the reading commands treat a
//! server like any other source. Sign-in material is handed back as one string for the secure
//! store and handed in again to build the source; nothing here writes it anywhere.

mod address;
mod error;
mod http;
pub mod kavita;
mod kept;
pub mod komga;
mod names;
pub mod opds;

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use kaeri_core::{
    books::PageRef,
    error::ProviderError,
    traits::{BookSource, PageError},
};

pub use crate::{
    address::{candidates, host_of, root_of},
    error::{Result, ServerError},
    http::{Auth, Http, basic},
};

/// The kinds of book server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Komga,
    Kavita,
    Opds,
}

impl Kind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Komga => "komga",
            Self::Kavita => "kavita",
            Self::Opds => "opds",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "komga" => Some(Self::Komga),
            "kavita" => Some(Self::Kavita),
            "opds" => Some(Self::Opds),
            _ => None,
        }
    }
}

/// A server that accepted a sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedIn {
    pub kind: Kind,
    pub name: String,
    pub version: String,
    /// How many series it lists, 0 when that was not cheap to learn.
    pub series: u32,
    /// What to keep in the secure store.
    pub credential: String,
}

/// What an address turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// The address that answered, with a scheme.
    pub address: String,
    /// The kind, `None` when the address wants a sign-in before it says.
    pub kind: Option<Kind>,
}

/// Tell what runs at a typed address. Addresses without a scheme are tried as https and then
/// http. A server that refuses everything without a sign-in comes back with no kind.
pub async fn check(client: &reqwest::Client, typed: &str) -> Result<Checked> {
    let mut last = ServerError::NotABookServer;
    for address in candidates(typed) {
        match detect(client, &address).await {
            Ok(kind) => return Ok(Checked { address, kind }),
            Err(error @ ServerError::Unreachable(_)) => last = error,
            Err(ServerError::NotABookServer) => {
                if !matches!(last, ServerError::Unreachable(_)) {
                    last = ServerError::NotABookServer;
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(last)
}

async fn detect(client: &reqwest::Client, address: &str) -> Result<Option<Kind>> {
    let root = root_of(address);
    if address != root && looks_like_opds_address(address) {
        return Ok(Some(Kind::Opds));
    }
    let http = Http::new(client, &root, Auth::None);
    if let Ok(text) = http.text("api/health").await
        && text.trim().eq_ignore_ascii_case("ok")
    {
        return Ok(Some(Kind::Kavita));
    }
    if let Ok(claim) = http.json::<serde_json::Value>("api/v1/claim").await
        && claim.get("isClaimed").is_some()
    {
        return Ok(Some(Kind::Komga));
    }
    // An OPDS catalogue, or something that wants a sign-in before it says.
    let response = client
        .get(http.url(address_path(address, &root)))
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| ServerError::Unreachable(describe_transport(&e)))?;
    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Ok(None);
    }
    if (200..300).contains(&status) {
        let body = response.text().await.unwrap_or_default();
        if opds::is_feed(&body) {
            return Ok(Some(Kind::Opds));
        }
    }
    Err(ServerError::NotABookServer)
}

fn looks_like_opds_address(address: &str) -> bool {
    address::looks_like_opds(address)
}

fn address_path<'a>(address: &'a str, root: &str) -> &'a str {
    address.strip_prefix(root).unwrap_or("")
}

fn describe_transport(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "the request timed out".into()
    } else if error.is_connect() {
        "the connection was refused".into()
    } else {
        "the request failed".into()
    }
}

/// Sign in. With no kind, a Komga sign-in is tried and then an OPDS one.
pub async fn sign_in(
    client: &reqwest::Client,
    address: &str,
    kind: Option<Kind>,
    user: &str,
    password: &str,
) -> Result<SignedIn> {
    let root = root_of(address);
    match kind {
        Some(Kind::Komga) => komga::sign_in(client, &root, user, password).await,
        Some(Kind::Kavita) => kavita::sign_in(client, &root, user, password).await,
        Some(Kind::Opds) => opds::sign_in(client, address, user, password).await,
        None => match komga::sign_in(client, &root, user, password).await {
            Ok(done) => Ok(done),
            Err(ServerError::Unauthorized) => Err(ServerError::Unauthorized),
            Err(_) => opds::sign_in(client, address, user, password).await,
        },
    }
}

/// A server that was signed in to, as the app keeps it.
pub struct Server<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub kind: Kind,
    pub address: &'a str,
    /// What the sign-in kept.
    pub credential: &'a str,
}

/// Build the source for a server that was signed in to. `keep` is where downloaded books go.
pub fn source(
    client: &reqwest::Client,
    server: &Server<'_>,
    keep: PathBuf,
    signed_out: Arc<AtomicBool>,
) -> Option<Arc<dyn BookSource>> {
    let Server { id, name, kind, address, credential } = *server;
    match kind {
        Kind::Komga => {
            let credential = komga::Credential::parse(credential)?;
            let http = Http::new(client, root_of(address), credential.auth());
            Some(Arc::new(komga::KomgaSource::new(id, name, http, keep, signed_out)))
        }
        Kind::Kavita => {
            let tokens = kavita::Tokens::parse(credential)?;
            let http = Http::new(client, root_of(address), Auth::Bearer(tokens.token.clone()));
            Some(Arc::new(kavita::KavitaSource::new(id, name, http, &tokens, keep, signed_out)))
        }
        Kind::Opds => {
            let b64 = credential.strip_prefix("basic:")?;
            let http = Http::new(client, address, Auth::Basic(b64.to_owned()));
            Some(Arc::new(opds::OpdsSource::new(id, name, http, keep, signed_out)))
        }
    }
}

/// Remember that a server refused the sign-in it was given.
pub(crate) fn note_sign_in(flag: &AtomicBool, error: &ServerError) {
    if error.needs_sign_in() {
        flag.store(true, Ordering::Relaxed);
    }
}

/// Whether a server may be asked: not while its sign-in is known to be refused.
pub(crate) fn signed_out_check(flag: &AtomicBool) -> std::result::Result<(), ProviderError> {
    if flag.load(Ordering::Relaxed) {
        Err(ProviderError::Unavailable("signed out".into()))
    } else {
        Ok(())
    }
}

/// Fetch one page image, with the headers its reference carries.
pub(crate) async fn fetch_with(
    http: &Http,
    page: &PageRef,
) -> std::result::Result<Vec<u8>, PageError> {
    let mut request = http.request(reqwest::Method::GET, &page.url);
    for (name, value) in &page.headers {
        request = request.header(name, value);
    }
    let response = request
        .send()
        .await
        .map_err(|e| PageError::Failed(ProviderError::Transport(describe_transport(&e))))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(PageError::Status(status));
    }
    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| PageError::Failed(ProviderError::Transport(describe_transport(&e))))
}
