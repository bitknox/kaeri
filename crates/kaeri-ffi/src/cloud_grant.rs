//! A platform's own grant of access to a cloud drive, which the app implements as
//! [`FfiCloudAuthorizer`] and hands over in [`crate::FfiStart::cloud_authorizer`]: Android's
//! authorization through Google Play services for Google Drive, on a phone.
//!
//! With one, the engine signs in to that service through the platform's sheet instead of a web
//! page, keeps no refresh token for it, and asks the app for a short-lived token whenever it
//! needs one. Every other service, and every app that passes none, signs in as before.

use std::sync::Arc;

use kaeri_engine::host::{CloudAuthorizer, CloudGrant, GrantAsked};
use kaeri_model::group::SyncFolderKind;

/// The platform's grant of access to a cloud drive, implemented by an app whose platform keeps
/// such grants itself. Optional: an app that has none passes nothing in
/// [`crate::FfiStart::cloud_authorizer`].
///
/// Called on the engine's blocking threads, never the app's main thread, so a call may wait on
/// the platform. An interactive `authorize` may wait for as long as the person has the
/// platform's sheet open.
#[uniffi::export(with_foreign)]
pub trait FfiCloudAuthorizer: Send + Sync {
    /// Whether it grants access to `service`. The engine signs in to any other itself.
    fn serves(&self, service: SyncFolderKind) -> bool;

    /// Access to `service`. With `interactive` the person asked to sign in, and the platform's
    /// consent sheet may show; without, the answer comes with no sheet, and is
    /// [`FfiCloudGrant::ConsentNeeded`] where one would be needed. `account` is the account the
    /// grant was given for, when the engine knows it. `rejected` is a token the service refused,
    /// for the platform to drop from its cache so the answer is a fresh one.
    fn authorize(
        &self,
        service: SyncFolderKind,
        interactive: bool,
        account: Option<String>,
        rejected: Option<String>,
    ) -> FfiCloudGrant;

    /// The person signed out of `service`: take the grant back, for `account` when the engine
    /// knows it. Start the revocation and return without waiting for it.
    fn signed_out(&self, service: SyncFolderKind, account: Option<String>);
}

/// What an [`FfiCloudAuthorizer`] answered.
#[derive(Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiCloudGrant {
    /// Access now: a short-lived token, and the account it is for when the platform says.
    Granted { access_token: String, account: Option<String> },
    /// The grant is not there, or was taken back, and asking without a sheet cannot give it.
    ConsentNeeded,
    /// The person closed the platform's sheet.
    Cancelled,
    /// The person did not give the access asked for.
    Denied,
    /// The platform could not reach the service.
    Unreachable,
    /// The platform cannot grant access on this device, such as one without Google Play services.
    Unavailable,
    /// Anything else, in words for the log.
    Failed { reason: String },
}

impl std::fmt::Debug for FfiCloudGrant {
    // Never the token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        CloudGrant::from(self.clone()).fmt(f)
    }
}

impl From<FfiCloudGrant> for CloudGrant {
    fn from(grant: FfiCloudGrant) -> Self {
        match grant {
            FfiCloudGrant::Granted { access_token, account } => {
                Self::Granted { access_token, account }
            }
            FfiCloudGrant::ConsentNeeded => Self::ConsentNeeded,
            FfiCloudGrant::Cancelled => Self::Cancelled,
            FfiCloudGrant::Denied => Self::Denied,
            FfiCloudGrant::Unreachable => Self::Unreachable,
            FfiCloudGrant::Unavailable => Self::Unavailable,
            FfiCloudGrant::Failed { reason } => Self::Failed(reason),
        }
    }
}

/// An [`FfiCloudAuthorizer`], as the engine's [`CloudAuthorizer`].
pub(crate) struct Authorizer(pub(crate) Arc<dyn FfiCloudAuthorizer>);

impl CloudAuthorizer for Authorizer {
    fn serves(&self, service: &SyncFolderKind) -> bool {
        self.0.serves(service.clone())
    }

    fn authorize(&self, asked: GrantAsked) -> CloudGrant {
        self.0.authorize(asked.service, asked.interactive, asked.account, asked.rejected).into()
    }

    fn signed_out(&self, service: SyncFolderKind, account: Option<String>) {
        self.0.signed_out(service, account);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// What an ask carried: whether it was interactive, the account and the refused token.
    type Ask = (bool, Option<String>, Option<String>);

    /// An app's authorizer that serves Google Drive and says what it was asked.
    #[derive(Default)]
    struct App {
        asked: Mutex<Vec<Ask>>,
        signed_out: Mutex<Vec<Option<String>>>,
    }

    impl FfiCloudAuthorizer for App {
        fn serves(&self, service: SyncFolderKind) -> bool {
            service == SyncFolderKind::GoogleDrive
        }

        fn authorize(
            &self,
            _: SyncFolderKind,
            interactive: bool,
            account: Option<String>,
            rejected: Option<String>,
        ) -> FfiCloudGrant {
            self.asked.lock().unwrap().push((interactive, account, rejected));
            if interactive {
                FfiCloudGrant::Granted {
                    access_token: "at-secret".into(),
                    account: Some("kim@mail.example".into()),
                }
            } else {
                FfiCloudGrant::ConsentNeeded
            }
        }

        fn signed_out(&self, _: SyncFolderKind, account: Option<String>) {
            self.signed_out.lock().unwrap().push(account);
        }
    }

    #[test]
    fn the_apps_authorizer_answers_the_engine_as_it_asked() {
        let app = Arc::new(App::default());
        let engine = Authorizer(app.clone());
        assert!(engine.serves(&SyncFolderKind::GoogleDrive));
        assert!(!engine.serves(&SyncFolderKind::Dropbox));
        let grant = engine.authorize(GrantAsked {
            service: SyncFolderKind::GoogleDrive,
            interactive: true,
            account: None,
            rejected: None,
        });
        assert_eq!(
            grant,
            CloudGrant::Granted {
                access_token: "at-secret".into(),
                account: Some("kim@mail.example".into())
            }
        );
        // The token never shows in a log line.
        assert!(!format!("{grant:?}").contains("at-secret"));
        let grant = engine.authorize(GrantAsked {
            service: SyncFolderKind::GoogleDrive,
            interactive: false,
            account: Some("kim@mail.example".into()),
            rejected: Some("at-old".into()),
        });
        assert_eq!(grant, CloudGrant::ConsentNeeded);
        assert_eq!(
            app.asked.lock().unwrap()[1],
            (false, Some("kim@mail.example".into()), Some("at-old".into()))
        );
        engine.signed_out(SyncFolderKind::GoogleDrive, Some("kim@mail.example".into()));
        assert_eq!(*app.signed_out.lock().unwrap(), [Some("kim@mail.example".to_owned())]);
        assert_eq!(
            CloudGrant::from(FfiCloudGrant::Failed { reason: "no".into() }),
            CloudGrant::Failed("no".into())
        );
    }
}
