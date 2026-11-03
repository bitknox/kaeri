//! The cloud drives of cross-device sync, signed in to from the running engine: Google Drive,
//! Dropbox and OneDrive.
//!
//! Three ways in, by the kind of device ([`ClientKind`]): the code flow through the system's web
//! sheet (iPhone, iPad, Mac, Android phone: `Event::SyncSignIn` with a sheet step, answered by
//! `Command::FinishSyncSignIn`), the code flow through the default browser and a loopback port
//! (desktop), and the device flow (TV: a code to type on another screen). Each ends with a
//! refresh token, kept in the profile's token store under `cloud-<service>`, the place the
//! trackers' tokens are, and never in a file of ours or in a log.
//!
//! A service with no client id in this build says so, with a reason, instead of failing when it
//! is chosen.

use std::{
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use kaeri_model::{
    event::Event,
    group::{
        CloudUnavailable, SyncCloudView, SyncCloudWay, SyncFolderKind, SyncProblem,
        SyncSignInStep,
    },
};
use kaeri_sync::{
    dropbox::{self, DropboxTarget},
    folder::{FolderError, FolderTarget},
    google::{
        self, ClientConfig, ClientKind, DevicePoll, Endpoints, GoogleDriveTarget, OAuthTokens,
        Pkce, Secret, Tokens,
    },
    loopback::{self, Loopback, Params},
    onedrive::{self, OneDriveTarget},
};
use tokio::sync::{Notify, mpsc};

use crate::{outbox::Outbox, syncing::Ctx};

/// How long a sign-in page may take to come back, and the least a device code is waited for.
const PAGE_WAIT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Google,
    Dropbox,
    OneDrive,
}

/// The token ids of the cloud drives, for a profile's promotion and deletion.
pub const TOKEN_IDS: [&str; 3] = ["cloud-google", "cloud-dropbox", "cloud-onedrive"];

impl Service {
    pub const ALL: [Self; 3] = [Self::Google, Self::Dropbox, Self::OneDrive];

    pub fn of(kind: &SyncFolderKind) -> Option<Self> {
        match kind {
            SyncFolderKind::GoogleDrive => Some(Self::Google),
            SyncFolderKind::Dropbox => Some(Self::Dropbox),
            SyncFolderKind::OneDrive => Some(Self::OneDrive),
            _ => None,
        }
    }

    pub fn kind(self) -> SyncFolderKind {
        match self {
            Self::Google => SyncFolderKind::GoogleDrive,
            Self::Dropbox => SyncFolderKind::Dropbox,
            Self::OneDrive => SyncFolderKind::OneDrive,
        }
    }

    pub fn token_id(self) -> &'static str {
        match self {
            Self::Google => TOKEN_IDS[0],
            Self::Dropbox => TOKEN_IDS[1],
            Self::OneDrive => TOKEN_IDS[2],
        }
    }
}

/// Where the services are, and how fast to poll, for a test: its own fakes on loopback.
#[cfg(feature = "testing")]
#[derive(Debug, Clone)]
pub struct CloudTest {
    pub google: Endpoints,
    pub dropbox: Endpoints,
    pub dropbox_content: String,
    pub onedrive: Endpoints,
    /// Between device polls, in place of the service's interval.
    pub poll_every: Option<Duration>,
}

/// A sign-in under way: where its redirect is delivered and how it is stopped.
struct Active {
    id: u64,
    service: SyncFolderKind,
    callback: Option<mpsc::UnboundedSender<String>>,
    cancel: Arc<Notify>,
}

/// What the engine keeps of cloud sign-in between commands.
#[derive(Default)]
pub struct State {
    active: Mutex<Option<Active>>,
    next: AtomicU64,
    #[cfg(feature = "testing")]
    test: Mutex<Option<CloudTest>>,
    #[cfg(feature = "testing")]
    fake: tokio::sync::OnceCell<Arc<google::FakeGoogle>>,
    #[cfg(feature = "testing")]
    memory: kaeri_sync::folder::MemoryTarget,
}

impl State {
    #[cfg(feature = "testing")]
    pub fn use_test(&self, test: CloudTest) {
        *self.test.lock().unwrap_or_else(PoisonError::into_inner) = Some(test);
    }

    /// Begin a sign-in to `service` (a cloud drive, or Nextcloud's login page), ending any
    /// other under way.
    pub(crate) fn begin(&self, service: SyncFolderKind, with_sheet: bool) -> Running {
        let (tx, rx) = mpsc::unbounded_channel();
        let cancel = Arc::new(Notify::new());
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let mine =
            Active { id, service, callback: with_sheet.then_some(tx), cancel: cancel.clone() };
        // A sign-in begun over another one ends it.
        let before = self.active.lock().unwrap_or_else(PoisonError::into_inner).replace(mine);
        if let Some(before) = before {
            before.cancel.notify_one();
        }
        Running { id, rx, cancel }
    }

    pub(crate) fn finish(&self, id: u64) {
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if active.as_ref().is_some_and(|a| a.id == id) {
            *active = None;
        }
    }

    /// Stop the sign-in under way, at whatever step it is on.
    pub fn cancel(&self) {
        let taken = self.active.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(active) = taken {
            active.cancel.notify_one();
        }
    }

    /// The web sheet's redirect, for the sign-in waiting on it.
    pub fn deliver(&self, service: Service, callback: String) {
        let active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        match active
            .as_ref()
            .filter(|a| a.service == service.kind())
            .and_then(|a| a.callback.as_ref())
        {
            Some(tx) => {
                let _ = tx.send(callback);
            }
            None => tracing::info!("a sign-in redirect arrived with no sign-in waiting for it"),
        }
    }
}

pub(crate) struct Running {
    pub(crate) id: u64,
    rx: mpsc::UnboundedReceiver<String>,
    pub(crate) cancel: Arc<Notify>,
}

/// Why a sign-in did not give a token.
#[derive(Debug)]
pub enum Ended {
    /// The person stopped it: nothing to say.
    Cancelled,
    Problem(SyncProblem),
}

impl From<SyncProblem> for Ended {
    fn from(p: SyncProblem) -> Self {
        Self::Problem(p)
    }
}

/// This device is a TV: it shows pages as QR codes, and never adds a group's first folder.
pub(crate) fn on_tv(ctx: &Ctx) -> bool {
    kind_of_device(ctx) == Some(ClientKind::Tv)
}

fn kind_of_device(ctx: &Ctx) -> Option<ClientKind> {
    ctx.clients.cloud_kind.or(if ctx.fixture {
        Some(ClientKind::Ios)
    } else if cfg!(feature = "desktop") {
        Some(ClientKind::Desktop)
    } else {
        None
    })
}

/// Kaeri's own registered cloud clients, used when the app names none of its own.
///
/// Google's desktop and TV clients come with a client secret. Google documents that an installed
/// app can't keep one, so it isn't treated as a secret there:
/// <https://developers.google.com/identity/protocols/oauth2#installed>. Until those secrets are
/// built in, Kaeri has no Google client of its own on desktops and TVs, and Google stays hidden
/// there unless the app names its own. Dropbox (PKCE) and OneDrive (a public client) have none.
mod built_in {
    /// iPhone, iPad and Mac.
    pub const GOOGLE_IOS: &str =
        "206293095288-jg72ijavhr0ja9pi09esua4gbqu11jtd.apps.googleusercontent.com";
    /// Android phones and tablets.
    pub const GOOGLE_ANDROID: &str =
        "206293095288-gitmhcp8qpohddgr0o8qli0e0o8kob4t.apps.googleusercontent.com";
    /// Windows, Linux and the terminal.
    pub const GOOGLE_DESKTOP: &str =
        "206293095288-1rem63756776feni5pd62f283n3a222t.apps.googleusercontent.com";
    pub const GOOGLE_DESKTOP_SECRET: Option<&str> = None;
    /// Android TV, through Google's device flow.
    pub const GOOGLE_TV: &str =
        "206293095288-thhdr2a11mbeq1e125k45ictid2tm1dh.apps.googleusercontent.com";
    pub const GOOGLE_TV_SECRET: Option<&str> = None;
    pub const DROPBOX: &str = "miisqg43cochyzu";
    pub const ONEDRIVE: &str = "1ddf77ce-b123-4483-9c8b-9596e8459a9c";
}

/// Kaeri's own client for `service` on a device of `kind`, if it has one.
fn built_in(service: Service, kind: ClientKind) -> Option<ClientConfig> {
    let (client_id, client_secret) = match service {
        Service::Google => match kind {
            ClientKind::Ios => (built_in::GOOGLE_IOS, None),
            ClientKind::Android => (built_in::GOOGLE_ANDROID, None),
            ClientKind::Desktop => {
                (built_in::GOOGLE_DESKTOP, Some(built_in::GOOGLE_DESKTOP_SECRET?))
            }
            ClientKind::Tv => (built_in::GOOGLE_TV, Some(built_in::GOOGLE_TV_SECRET?)),
        },
        Service::Dropbox => (built_in::DROPBOX, None),
        Service::OneDrive => (built_in::ONEDRIVE, None),
    };
    Some(ClientConfig {
        client_id: client_id.into(),
        client_secret: client_secret.map(Secret::new),
    })
}

/// Whether Kaeri has a Google client of its own for a device of `kind`.
#[cfg(all(test, feature = "testing"))]
pub(crate) fn has_own_google(kind: ClientKind) -> bool {
    built_in(Service::Google, kind).is_some()
}

/// The client `service` signs in with on this device: the app's own when it names one, else
/// Kaeri's. An app's client id never goes with Kaeri's secret.
fn config_of(ctx: &Ctx, service: Service, kind: ClientKind) -> Option<ClientConfig> {
    let present = |v: &Option<String>| v.clone().filter(|v| !v.trim().is_empty());
    if ctx.fixture {
        return Some(ClientConfig {
            client_id: "app.fixture.client".into(),
            client_secret: None,
        });
    }
    let own = match service {
        Service::Google => &ctx.clients.google,
        Service::Dropbox => &ctx.clients.dropbox,
        Service::OneDrive => &ctx.clients.onedrive,
    };
    let Some(client_id) = present(own) else {
        return (!ctx.clients.no_built_in_clients).then(|| built_in(service, kind)).flatten();
    };
    let client_secret = match (service, kind) {
        // Google's iOS and Android clients have no secret; the desktop's has one it needs.
        (Service::Google, ClientKind::Desktop) => Some(present(&ctx.clients.google_secret)?),
        (Service::Google, ClientKind::Tv) => present(&ctx.clients.google_secret),
        _ => None,
    };
    Some(ClientConfig { client_id, client_secret: client_secret.map(Secret::new) })
}

/// How `service` is signed in to on this device, or why it cannot be.
pub fn way(ctx: &Ctx, service: Service) -> SyncCloudWay {
    let Some(kind) = kind_of_device(ctx) else {
        return SyncCloudWay::Unavailable { reason: CloudUnavailable::NoPlatform };
    };
    // Dropbox has no device flow: a TV signs in on a phone.
    if service == Service::Dropbox && kind == ClientKind::Tv {
        return SyncCloudWay::Unavailable { reason: CloudUnavailable::NeedsPhone };
    }
    if config_of(ctx, service, kind).is_none() {
        return SyncCloudWay::Unavailable { reason: CloudUnavailable::NoClientId };
    }
    match kind {
        ClientKind::Ios | ClientKind::Android => SyncCloudWay::Sheet,
        ClientKind::Desktop => SyncCloudWay::Browser,
        ClientKind::Tv => SyncCloudWay::DeviceCode,
    }
}

pub fn signed_in(ctx: &Ctx, service: Service) -> bool {
    ctx.sync.tokens.get(service.token_id()).is_ok_and(|t| !t.trim().is_empty())
}

/// The picker's rows.
pub fn views(ctx: &Ctx) -> Vec<SyncCloudView> {
    Service::ALL
        .into_iter()
        .map(|service| {
            let signed_in = signed_in(ctx, service);
            SyncCloudView {
                service: service.kind(),
                way: way(ctx, service),
                signed_in,
                account: if signed_in { account(&ctx.store, service) } else { None },
            }
        })
        .collect()
}

fn account_key(service: Service) -> String {
    format!("cloud.account.{}", service.token_id())
}

/// The account a sign-in belongs to, as the service named it when it was signed in to.
pub fn account(store: &kaeri_store::Store, service: Service) -> Option<String> {
    store.sync_meta_get(&account_key(service)).ok().flatten().filter(|a| !a.is_empty())
}

/// Keep the account a sign-in belongs to, or forget it with `None`.
pub fn keep_account(store: &kaeri_store::Store, service: Service, account: Option<&str>) {
    let _ = match account {
        Some(account) => store.sync_meta_set(&account_key(service), account),
        None => store.sync_meta_remove(&account_key(service)),
    };
}

/// Ask the service which account the sign-in kept for it belongs to, and keep the answer.
/// Dropbox would need a scope Kaeri does not ask for, so it has none; a failure keeps none.
async fn learn_account(ctx: &Ctx, service: Service) {
    let asked = async {
        let tokens = tokens_for(ctx, service).await.ok()?;
        let http = ctx.sync.http.plain();
        let endpoints = endpoints(ctx, service).await;
        match service {
            Service::Google => google::account(http, &endpoints, tokens.as_ref()).await.ok()?,
            Service::OneDrive => OneDriveTarget::new(http.clone(), &endpoints.api_base, tokens)
                .account()
                .await
                .ok()?,
            Service::Dropbox => None,
        }
    };
    let found = tokio::time::timeout(Duration::from_secs(10), asked).await.ok().flatten();
    keep_account(&ctx.store, service, found.as_deref());
}

/// Forget the sign-ins of every service, and stop one under way.
pub fn forget_all(ctx: &Ctx) {
    ctx.syncing.cloud.cancel();
    for service in Service::ALL {
        forget(ctx, service);
    }
}

pub fn forget(ctx: &Ctx, service: Service) {
    let _ = ctx.sync.tokens.clear(service.token_id());
    keep_account(&ctx.store, service, None);
}

async fn endpoints(ctx: &Ctx, service: Service) -> Endpoints {
    #[cfg(feature = "testing")]
    {
        let test =
            ctx.syncing.cloud.test.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(test) = test {
            return match service {
                Service::Google => test.google,
                Service::Dropbox => test.dropbox,
                Service::OneDrive => test.onedrive,
            };
        }
        if ctx.fixture {
            return fake(ctx).await.endpoints();
        }
    }
    let _ = ctx;
    match service {
        Service::Google => Endpoints::default(),
        Service::Dropbox => dropbox::endpoints(),
        Service::OneDrive => onedrive::endpoints(),
    }
}

/// A fixture build's one fake service, on loopback: its sign-in pages say yes at once, and its
/// device code is approved after a few polls.
#[cfg(feature = "testing")]
async fn fake(ctx: &Ctx) -> Arc<google::FakeGoogle> {
    ctx.syncing
        .cloud
        .fake
        .get_or_init(|| async { Arc::new(google::FakeGoogle::start().await) })
        .await
        .clone()
}

#[cfg(feature = "testing")]
fn poll_every(ctx: &Ctx) -> Option<Duration> {
    let test = ctx.syncing.cloud.test.lock().unwrap_or_else(PoisonError::into_inner).clone();
    match test {
        Some(test) => test.poll_every,
        None => ctx.fixture.then_some(Duration::from_secs(2)),
    }
}

#[cfg(not(feature = "testing"))]
fn poll_every(_: &Ctx) -> Option<Duration> {
    None
}

fn problem(service: Service, e: &FolderError) -> SyncProblem {
    match e {
        FolderError::Unreachable(_) => SyncProblem::Unreachable,
        FolderError::Unauthorized => SyncProblem::SignInDenied { service: service.kind() },
        other => SyncProblem::Other(other.to_string()),
    }
}

fn say(tx: &Outbox, service: Service, step: Option<SyncSignInStep>) {
    say_for(tx, &service.kind(), step);
}

/// Say where a sign-in to `kind` stands: a cloud drive's, or Nextcloud's login page.
pub(crate) fn say_for(tx: &Outbox, kind: &SyncFolderKind, step: Option<SyncSignInStep>) {
    let _ = tx.send(Event::SyncSignIn { service: kind.clone(), step });
}

/// Say the sign-in is over, whether it signed in, was cancelled or failed.
pub fn ended(tx: &Outbox, kind: &SyncFolderKind) {
    if Service::of(kind).is_some() || *kind == SyncFolderKind::Nextcloud {
        say_for(tx, kind, None);
    }
}

/// The ports to listen on for a desktop redirect, and the path it comes back to.
fn loopback_spec(service: Service) -> (&'static [u16], &'static str) {
    match service {
        Service::Google | Service::OneDrive => (&[0], "/"),
        Service::Dropbox => (&dropbox::LOOPBACK_PORTS, "/auth/dropbox"),
    }
}

fn redirect_for(service: Service, kind: ClientKind, client_id: &str, port: u16) -> String {
    match (service, kind) {
        (Service::Google, kind) => kind.redirect(client_id, port).unwrap_or_default(),
        (Service::Dropbox, ClientKind::Desktop) => dropbox::loopback_redirect(port),
        (Service::Dropbox, _) => dropbox::APP_REDIRECT.to_owned(),
        (Service::OneDrive, ClientKind::Desktop) => onedrive::loopback_redirect(port),
        (Service::OneDrive, _) => onedrive::APP_REDIRECT.to_owned(),
    }
}

fn authorize_url(
    service: Service,
    endpoints: &Endpoints,
    config: &ClientConfig,
    redirect: &str,
    challenge: &str,
    state: &str,
) -> Result<String, FolderError> {
    match service {
        Service::Google => google::auth_url(endpoints, config, redirect, challenge, state),
        Service::Dropbox => dropbox::auth_url(endpoints, config, redirect, challenge, state),
        Service::OneDrive => onedrive::auth_url(endpoints, config, redirect, challenge, state),
    }
}

/// Sign in to `service` and keep the refresh token. Says each step as `Event::SyncSignIn`; the
/// caller says it is over.
pub async fn sign_in(ctx: &Ctx, tx: &Outbox, service: Service) -> Result<(), Ended> {
    let way = way(ctx, service);
    let kind = kind_of_device(ctx);
    let (SyncCloudWay::Sheet | SyncCloudWay::Browser | SyncCloudWay::DeviceCode, Some(kind)) =
        (&way, kind)
    else {
        let reason = match way {
            SyncCloudWay::Unavailable { reason } => reason,
            _ => CloudUnavailable::NoPlatform,
        };
        return Err(SyncProblem::CloudUnavailable { service: service.kind(), reason }.into());
    };
    let Some(config) = config_of(ctx, service, kind) else {
        return Err(SyncProblem::CloudUnavailable {
            service: service.kind(),
            reason: CloudUnavailable::NoClientId,
        }
        .into());
    };
    let endpoints = endpoints(ctx, service).await;
    let mut run = ctx.syncing.cloud.begin(service.kind(), way == SyncCloudWay::Sheet);
    let outcome = match way {
        SyncCloudWay::DeviceCode => device(ctx, tx, service, &endpoints, &config, &run).await,
        _ => code(ctx, tx, service, kind, &endpoints, &config, &mut run).await,
    };
    ctx.syncing.cloud.finish(run.id);
    let tokens = outcome?;
    let Some(refresh) = tokens.refresh_token else {
        return Err(
            SyncProblem::Other("the service gave no way to stay signed in".into()).into()
        );
    };
    ctx.sync
        .tokens
        .set(service.token_id(), &refresh)
        .map_err(|e| SyncProblem::Other(format!("the sign-in could not be kept: {e}")))?;
    learn_account(ctx, service).await;
    Ok(())
}

enum Catcher {
    Sheet,
    Loopback(Loopback),
}

async fn code(
    ctx: &Ctx,
    tx: &Outbox,
    service: Service,
    kind: ClientKind,
    endpoints: &Endpoints,
    config: &ClientConfig,
    run: &mut Running,
) -> Result<Tokens, Ended> {
    let pkce = Pkce::generate();
    let state = google::random_state();
    let (mut catcher, port) = if kind == ClientKind::Desktop {
        let (ports, path) = loopback_spec(service);
        let line = kaeri_model::Msg::CloudLoopbackDone.text_in(&ctx.syncing.language());
        let listener = loopback::bind(ports, path, &line)
            .await
            .map_err(|e| Ended::Problem(problem(service, &e)))?;
        let port = listener.port();
        (Catcher::Loopback(listener), port)
    } else {
        (Catcher::Sheet, 0)
    };
    let redirect = redirect_for(service, kind, &config.client_id, port);
    let url = authorize_url(service, endpoints, config, &redirect, &pkce.challenge, &state)
        .map_err(|e| Ended::Problem(problem(service, &e)))?;
    match &catcher {
        Catcher::Sheet => {
            let callback_scheme = redirect.split(':').next().unwrap_or_default().to_owned();
            say(tx, service, Some(SyncSignInStep::Sheet { url, callback_scheme }));
        }
        Catcher::Loopback(_) => {
            ctx.host.open_url(&url).map_err(|e| Ended::Problem(SyncProblem::Other(e)))?;
            say(tx, service, Some(SyncSignInStep::Browser { url }));
        }
    }
    let params = loop {
        let got = tokio::select! {
            () = run.cancel.notified() => return Err(Ended::Cancelled),
            () = tokio::time::sleep(PAGE_WAIT) => {
                return Err(SyncProblem::SignInExpired { service: service.kind() }.into());
            }
            got = next_params(&mut catcher, &mut run.rx) => got,
        };
        let Some(params) = got else {
            return Err(Ended::Cancelled);
        };
        let param =
            |name: &str| params.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str());
        // A redirect for some other request: not this sign-in's to finish, so it goes on waiting.
        if param("state") != Some(state.as_str()) {
            tracing::warn!(
                service = ?service.kind(),
                "a sign-in redirect that does not carry this request's state, refused"
            );
            continue;
        }
        break params;
    };
    let param = |name: &str| params.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
    if let Some(error) = param("error") {
        // Saying no on the service's page is cancelling.
        if error == "access_denied" {
            return Err(Ended::Cancelled);
        }
        let reason = param("error_description").unwrap_or(error);
        return Err(SyncProblem::Other(reason).into());
    }
    let Some(code) = param("code") else {
        return Err(SyncProblem::Other("the redirect carried no code".into()).into());
    };
    google::exchange_code(
        ctx.sync.http.plain(),
        endpoints,
        config,
        &code,
        &redirect,
        &pkce.verifier,
    )
    .await
    .map_err(|e| Ended::Problem(problem(service, &e)))
}

async fn next_params(
    catcher: &mut Catcher,
    sheet: &mut mpsc::UnboundedReceiver<String>,
) -> Option<Params> {
    match catcher {
        Catcher::Sheet => {
            sheet.recv().await.map(|callback| kaeri_track::auth::callback_params(&callback))
        }
        Catcher::Loopback(listener) => listener.next().await,
    }
}

async fn device(
    ctx: &Ctx,
    tx: &Outbox,
    service: Service,
    endpoints: &Endpoints,
    config: &ClientConfig,
    run: &Running,
) -> Result<Tokens, Ended> {
    let http = ctx.sync.http.plain();
    let scope = match service {
        Service::Google => google::SCOPE,
        Service::OneDrive => onedrive::SCOPE,
        Service::Dropbox => {
            return Err(SyncProblem::CloudUnavailable {
                service: service.kind(),
                reason: CloudUnavailable::NeedsPhone,
            }
            .into());
        }
    };
    #[cfg(feature = "testing")]
    if ctx.fixture
        && ctx.syncing.cloud.test.lock().unwrap_or_else(PoisonError::into_inner).is_none()
    {
        use google::DeviceStep::{Done, Pending};
        fake(ctx).await.script_device(&[Pending, Pending, Pending, Pending, Pending, Done]);
    }
    let code = google::device_start_for(http, endpoints, config, scope)
        .await
        .map_err(|e| Ended::Problem(problem(service, &e)))?;
    let expires_at =
        kaeri_store::now() + i64::try_from(code.expires_in).unwrap_or(i64::MAX / 2);
    say(
        tx,
        service,
        Some(SyncSignInStep::Code {
            user_code: code.user_code.clone(),
            verification_url: code.verification_url.clone(),
            expires_at,
            interval: u32::try_from(code.interval).unwrap_or(5),
        }),
    );
    let mut wait = poll_every(ctx).unwrap_or(Duration::from_secs(code.interval.max(1)));
    loop {
        tokio::select! {
            () = run.cancel.notified() => return Err(Ended::Cancelled),
            () = tokio::time::sleep(wait) => {}
        }
        if kaeri_store::now() >= expires_at {
            return Err(SyncProblem::SignInExpired { service: service.kind() }.into());
        }
        match google::device_poll(http, endpoints, config, &code.device_code).await {
            Ok(DevicePoll::Pending) => {}
            Ok(DevicePoll::SlowDown) => wait += Duration::from_secs(5),
            Ok(DevicePoll::Denied) => {
                return Err(SyncProblem::SignInDenied { service: service.kind() }.into());
            }
            Ok(DevicePoll::Expired) => {
                return Err(SyncProblem::SignInExpired { service: service.kind() }.into());
            }
            Ok(DevicePoll::Done(tokens)) => return Ok(tokens),
            // A dropped connection while the code is up: the next poll may work.
            Err(FolderError::Unreachable(_)) => {}
            Err(e) => return Err(Ended::Problem(problem(service, &e))),
        }
    }
}

/// The access to `service` from the refresh token kept for it. No token is
/// [`SyncProblem::SignInNeeded`].
pub async fn tokens_for(ctx: &Ctx, service: Service) -> Result<Arc<OAuthTokens>, SyncProblem> {
    let needed = || SyncProblem::SignInNeeded { service: service.kind() };
    let refresh = ctx.sync.tokens.get(service.token_id()).ok().filter(|t| !t.trim().is_empty());
    let (Some(refresh), Some(kind)) = (refresh, kind_of_device(ctx)) else {
        return Err(needed());
    };
    let Some(config) = config_of(ctx, service, kind) else {
        return Err(SyncProblem::CloudUnavailable {
            service: service.kind(),
            reason: CloudUnavailable::NoClientId,
        });
    };
    let store = ctx.sync.tokens.clone();
    let id = service.token_id();
    let tokens = OAuthTokens::new(
        ctx.sync.http.plain().clone(),
        endpoints(ctx, service).await,
        config,
        refresh,
    )
    // A refresh token the service rotated replaces the one kept.
    .tokens_changed(move |t| {
        if let Some(next) = &t.refresh_token {
            let _ = store.set(id, next);
        }
    });
    Ok(Arc::new(tokens))
}

/// The folder target in `service`. Without a sign-in, `tx` given runs one first; with none,
/// [`SyncProblem::SignInNeeded`].
pub async fn target(
    ctx: &Ctx,
    service: Service,
    tx: Option<&Outbox>,
) -> Result<Arc<dyn FolderTarget>, Ended> {
    #[cfg(feature = "testing")]
    if ctx.fixture
        && ctx.syncing.cloud.test.lock().unwrap_or_else(PoisonError::into_inner).is_none()
    {
        // A fixture build signs in to the fake and keeps its folder in memory.
        if tokens_for(ctx, service).await.is_err() {
            let Some(tx) = tx else {
                return Err(SyncProblem::SignInNeeded { service: service.kind() }.into());
            };
            sign_in(ctx, tx, service).await?;
            say(tx, service, Some(SyncSignInStep::Finishing));
        }
        return Ok(Arc::new(ctx.syncing.cloud.memory.clone()));
    }
    let tokens = match tokens_for(ctx, service).await {
        Ok(tokens) => tokens,
        Err(SyncProblem::SignInNeeded { .. }) if tx.is_some() => {
            let tx = tx.expect("checked above");
            sign_in(ctx, tx, service).await?;
            say(tx, service, Some(SyncSignInStep::Finishing));
            tokens_for(ctx, service).await?
        }
        Err(p) => return Err(p.into()),
    };
    let http = ctx.sync.http.plain().clone();
    let endpoints = endpoints(ctx, service).await;
    Ok(match service {
        Service::Google => Arc::new(GoogleDriveTarget::new(http, endpoints, tokens)),
        Service::Dropbox => {
            #[cfg(feature = "testing")]
            let content = ctx
                .syncing
                .cloud
                .test
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .map_or_else(
                    || dropbox::CONTENT_BASE.to_owned(),
                    |t| t.dropbox_content.clone(),
                );
            #[cfg(not(feature = "testing"))]
            let content = dropbox::CONTENT_BASE.to_owned();
            Arc::new(DropboxTarget::new(http, &endpoints.api_base, &content, tokens))
        }
        Service::OneDrive => Arc::new(OneDriveTarget::new(http, &endpoints.api_base, tokens)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kaeris_own_clients_are_picked_by_the_kind_of_device() {
        let pick = |service, kind| {
            let config = built_in(service, kind).expect("a built-in client");
            (config.client_id, config.client_secret.is_some())
        };
        let google = |kind| pick(Service::Google, kind);
        assert!(google(ClientKind::Ios).0.starts_with("206293095288-jg72"));
        assert!(google(ClientKind::Android).0.starts_with("206293095288-gitm"));
        // The desktop's and the TV's Google clients need their secrets, so until those are
        // built in Kaeri has no Google client of its own there.
        for kind in [ClientKind::Desktop, ClientKind::Tv] {
            let secret = match kind {
                ClientKind::Desktop => built_in::GOOGLE_DESKTOP_SECRET,
                _ => built_in::GOOGLE_TV_SECRET,
            };
            let config = built_in(Service::Google, kind);
            assert_eq!(config.is_some(), secret.is_some());
            if let Some(config) = config {
                assert!(config.client_secret.is_some());
            }
        }
        assert!(!google(ClientKind::Ios).1 && !google(ClientKind::Android).1);
        for kind in [ClientKind::Ios, ClientKind::Desktop] {
            assert_eq!(pick(Service::Dropbox, kind), ("miisqg43cochyzu".into(), false));
            assert_eq!(
                pick(Service::OneDrive, kind),
                ("1ddf77ce-b123-4483-9c8b-9596e8459a9c".into(), false)
            );
        }
    }

    #[test]
    fn each_kind_of_device_comes_back_where_its_client_registered() {
        let redirect = |s, k, port| redirect_for(s, k, "123.apps.example", port);
        assert_eq!(
            redirect(Service::Google, ClientKind::Ios, 0),
            "example.apps.123:/oauth2redirect"
        );
        assert_eq!(
            redirect(Service::Google, ClientKind::Android, 0),
            "app.kaeri.app:/oauth2redirect"
        );
        assert_eq!(
            redirect(Service::Google, ClientKind::Desktop, 5000),
            "http://127.0.0.1:5000"
        );
        assert_eq!(redirect(Service::Dropbox, ClientKind::Ios, 0), "kaeri://auth/dropbox");
        assert_eq!(redirect(Service::Dropbox, ClientKind::Android, 0), "kaeri://auth/dropbox");
        assert_eq!(
            redirect(Service::Dropbox, ClientKind::Desktop, 47_812),
            "http://127.0.0.1:47812/auth/dropbox"
        );
        assert_eq!(redirect(Service::OneDrive, ClientKind::Ios, 0), "kaeri://auth/onedrive");
        assert_eq!(
            redirect(Service::OneDrive, ClientKind::Desktop, 6000),
            "http://localhost:6000"
        );
    }

    #[test]
    fn dropbox_listens_on_its_registered_ports_in_turn_and_the_others_on_any() {
        assert_eq!(
            loopback_spec(Service::Dropbox),
            (&[47_811, 47_812, 47_813][..], "/auth/dropbox")
        );
        assert_eq!(loopback_spec(Service::Google).0, [0]);
        assert_eq!(loopback_spec(Service::OneDrive).0, [0]);
    }

    #[test]
    fn a_service_and_its_folder_kind_name_each_other_and_nextcloud_is_none() {
        for service in Service::ALL {
            assert_eq!(Service::of(&service.kind()), Some(service));
        }
        assert_eq!(Service::of(&SyncFolderKind::Nextcloud), None);
        assert_eq!(TOKEN_IDS.len(), Service::ALL.len());
    }
}
