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
    address::{KavitaLink, candidates, host_of, kavita_link, last_four, root_of},
    error::{Result, ServerError},
    http::{Auth, Http, basic},
};

/// The most pages one book from a server may have: as many as an archive may hold entries
/// (`kaeri_books::Limits::max_entries`). A server states a book's page count and every page
/// becomes a [`PageRef`], so a count past this is a broken or hostile answer, not a book.
pub const MOST_PAGES: u32 = 20_000;

/// A page count a server stated, kept only when a book could have it: at least one page and at
/// most [`MOST_PAGES`].
pub(crate) fn pages_stated(count: impl Into<u64>) -> Option<u32> {
    u32::try_from(count.into()).ok().filter(|n| (1..=MOST_PAGES).contains(n))
}

/// A server's book of more pages than any book has.
pub(crate) fn too_many_pages(count: impl std::fmt::Display) -> ServerError {
    ServerError::Parse(format!("a book of {count} pages, more than {MOST_PAGES}"))
}

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
    /// How many series it lists, `None` when it did not say for sure.
    pub series: Option<u32>,
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
    answered(check_each(client, typed)).await
}

async fn check_each(client: &reqwest::Client, typed: &str) -> Result<Checked> {
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
    answered(async {
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
    })
    .await
}

/// Sign in to Kavita with the API key from an account's OPDS link: no user name or password.
/// A key Kavita does not know is [`ServerError::Unauthorized`].
pub async fn sign_in_with_key(client: &reqwest::Client, link: &KavitaLink) -> Result<SignedIn> {
    answered(kavita::sign_in_with_key(client, &link.root, &link.key)).await
}

/// Runs one of the steps a person waits on (a check, a sign-in) so that it always answers: a
/// panic inside it, from an answer no one foresaw, comes back as an answer not understood
/// instead of ending the task that would have told the screen.
async fn answered<T>(step: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    use futures::FutureExt;
    std::panic::AssertUnwindSafe(step).catch_unwind().await.unwrap_or_else(|_| {
        tracing::warn!("a book server step panicked; answering that it was not understood");
        Err(ServerError::Parse("the server's answer could not be read".into()))
    })
}

/// One library on a server, with how many series this account sees in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    /// The server's own id for it.
    pub id: String,
    pub name: String,
    /// `None` when the server did not say.
    pub series: Option<u32>,
}

/// The libraries this account sees on a signed-in server. An OPDS catalogue has none to tell
/// apart and answers an empty list. A library whose count failed is still listed, without one.
pub async fn libraries(
    client: &reqwest::Client,
    address: &str,
    kind: Kind,
    credential: &str,
) -> Result<Vec<Library>> {
    let root = root_of(address);
    match kind {
        Kind::Komga => {
            let credential = komga::Credential::parse(credential)
                .ok_or_else(|| ServerError::Parse("the kept sign-in".into()))?;
            komga::libraries(&Http::new(client, root, credential.auth())).await
        }
        Kind::Kavita => {
            let tokens = kavita::Tokens::parse(credential)
                .ok_or_else(|| ServerError::Parse("the kept sign-in".into()))?;
            kavita::libraries(&Http::new(client, root, Auth::Bearer(tokens.token))).await
        }
        Kind::Opds => Ok(Vec::new()),
    }
}

/// How many series a signed-in server lists for this account now, for the recheck of a server
/// that had none. `None` when it did not say.
pub async fn count_series(
    client: &reqwest::Client,
    address: &str,
    kind: Kind,
    credential: &str,
) -> Result<Option<u32>> {
    count_series_kept(
        client,
        address,
        kind,
        credential,
        &Arc::new(AtomicBool::new(false)).into(),
    )
    .await
}

/// [`count_series`] for a server whose sign-in `gate` keeps: a Kavita JWT the server refuses
/// is renewed as its source renews it, and the new tokens go to the keeper; a refusal that
/// cannot be got past signs the server out.
pub async fn count_series_kept(
    client: &reqwest::Client,
    address: &str,
    kind: Kind,
    credential: &str,
    gate: &Gate,
) -> Result<Option<u32>> {
    let counted = count_once(client, address, kind, credential, gate).await;
    if let Err(error) = &counted {
        note_sign_in(gate, error);
    }
    counted
}

async fn count_once(
    client: &reqwest::Client,
    address: &str,
    kind: Kind,
    credential: &str,
    gate: &Gate,
) -> Result<Option<u32>> {
    match kind {
        Kind::Komga => {
            let credential = komga::Credential::parse(credential)
                .ok_or_else(|| ServerError::Parse("the kept sign-in".into()))?;
            komga::count_series(&Http::new(client, root_of(address), credential.auth())).await
        }
        Kind::Kavita => {
            let tokens = kavita::Tokens::parse(credential)
                .ok_or_else(|| ServerError::Parse("the kept sign-in".into()))?;
            let base = Http::new(client, root_of(address), Auth::None);
            match kavita::count_all(&base.with_auth(Auth::Bearer(tokens.token.clone()))).await {
                Err(ServerError::Unauthorized) => {
                    let fresh = kavita::renew(&base, &tokens).await?;
                    gate.renewed(&fresh.kept());
                    kavita::count_all(&base.with_auth(Auth::Bearer(fresh.token))).await
                }
                counted => counted,
            }
        }
        Kind::Opds => {
            let b64 = credential
                .strip_prefix("basic:")
                .ok_or_else(|| ServerError::Parse("the kept sign-in".into()))?;
            opds::count(client, address, &Auth::Basic(b64.to_owned())).await
        }
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
    signed_out: impl Into<Gate>,
) -> Option<Arc<dyn BookSource>> {
    source_with(client, server, &[], keep, signed_out)
}

/// [`source`], asking only the libraries in `libraries` (their ids), or every one when it is
/// empty. An OPDS catalogue has no libraries and ignores it.
pub fn source_with(
    client: &reqwest::Client,
    server: &Server<'_>,
    libraries: &[String],
    keep: PathBuf,
    signed_out: impl Into<Gate>,
) -> Option<Arc<dyn BookSource>> {
    let Server { id, name, kind, address, credential } = *server;
    let gate = signed_out.into();
    match kind {
        Kind::Komga => {
            let credential = komga::Credential::parse(credential)?;
            let http = Http::new(client, root_of(address), credential.auth());
            let source = komga::KomgaSource::new(id, name, http, keep, gate)
                .in_libraries(libraries.to_vec());
            Some(Arc::new(source))
        }
        Kind::Kavita => {
            let tokens = kavita::Tokens::parse(credential)?;
            let http = Http::new(client, root_of(address), Auth::Bearer(tokens.token.clone()));
            let source = kavita::KavitaSource::new(id, name, http, &tokens, keep, gate)
                .in_libraries(libraries.to_vec());
            Some(Arc::new(source))
        }
        Kind::Opds => {
            let b64 = credential.strip_prefix("basic:")?;
            let http = Http::new(client, address, Auth::Basic(b64.to_owned()));
            Some(Arc::new(opds::OpdsSource::new(id, name, http, keep, gate)))
        }
    }
}

/// Where a server's sign-in is kept, told when it changes while its source runs. The source
/// never writes the sign-in anywhere itself.
pub trait Keeper: Send + Sync {
    /// The server handed out a new sign-in in place of the one kept: keep `credential` instead.
    fn renewed(&self, credential: &str);
    /// The server refused the sign-in and it could not be renewed: it waits for the person to
    /// sign in again.
    fn refused(&self);
}

/// A keeper that keeps nothing, for a source no one keeps the sign-in of.
struct NoKeeper;

impl Keeper for NoKeeper {
    fn renewed(&self, _credential: &str) {}
    fn refused(&self) {}
}

/// A source's sign-in state: whether the server refused it, and the keeper told when that or
/// the sign-in itself changes. Cheap to clone; clones share the state.
#[derive(Clone)]
pub struct Gate {
    signed_out: Arc<AtomicBool>,
    keeper: Arc<dyn Keeper>,
}

impl Gate {
    /// The state behind `signed_out`, telling `keeper` of every change.
    pub fn new(signed_out: Arc<AtomicBool>, keeper: Arc<dyn Keeper>) -> Self {
        Self { signed_out, keeper }
    }

    /// Whether the server refused the sign-in.
    pub fn is_signed_out(&self) -> bool {
        self.signed_out.load(Ordering::Relaxed)
    }

    /// The server refused the sign-in: the keeper is told once, when it was not known before.
    pub(crate) fn refuse(&self) {
        if !self.signed_out.swap(true, Ordering::Relaxed) {
            self.keeper.refused();
        }
    }

    /// The server handed out a new sign-in.
    pub(crate) fn renewed(&self, credential: &str) {
        self.keeper.renewed(credential);
    }
}

impl From<Arc<AtomicBool>> for Gate {
    fn from(signed_out: Arc<AtomicBool>) -> Self {
        Self::new(signed_out, Arc::new(NoKeeper))
    }
}

/// Remember that a server refused the sign-in it was given.
pub(crate) fn note_sign_in(gate: &Gate, error: &ServerError) {
    if error.needs_sign_in() {
        gate.refuse();
    }
}

/// Whether a server may be asked: not while its sign-in is known to be refused.
pub(crate) fn signed_out_check(gate: &Gate) -> std::result::Result<(), ProviderError> {
    if gate.is_signed_out() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_step_that_panics_still_answers() {
        let step = async {
            let text = "\u{3042}";
            // A byte index inside the letter, as a slicing mistake would make.
            let cut = std::hint::black_box(1);
            Ok::<_, ServerError>(text[..cut].len())
        };
        assert!(matches!(answered(step).await, Err(ServerError::Parse(_))));
        assert_eq!(answered(async { Ok::<_, ServerError>(7) }).await, Ok(7));
    }

    #[test]
    fn a_stated_page_count_is_kept_only_when_a_book_could_have_it() {
        assert_eq!(MOST_PAGES as usize, kaeri_books::Limits::default().max_entries);
        assert_eq!(pages_stated(0u32), None);
        assert_eq!(pages_stated(1u32), Some(1));
        assert_eq!(pages_stated(MOST_PAGES), Some(MOST_PAGES));
        assert_eq!(pages_stated(MOST_PAGES + 1), None);
        assert_eq!(pages_stated(u32::MAX), None);
        assert_eq!(pages_stated(u64::MAX), None);
    }
}
