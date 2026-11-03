//! The sync group, as the Sync section of Settings shows it.

/// Where the folder road goes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SyncFolderKind {
    /// A folder on this device or one it mounts.
    Directory,
    /// A WebDAV address with a user name and password.
    WebDav,
    /// A Nextcloud server, signed in through its login page.
    Nextcloud,
    /// Google Drive's app data folder.
    GoogleDrive,
    /// Dropbox's app folder.
    Dropbox,
    /// OneDrive's app folder.
    OneDrive,
}

/// The folder in use.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SyncFolderView {
    pub kind: SyncFolderKind,
    /// What to show for it: a path, a host or an account.
    pub label: String,
}

/// A device in the group.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SyncDeviceView {
    pub id: String,
    pub name: String,
    /// This device.
    pub this_device: bool,
    /// When it was last heard from, in Unix seconds.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub last_seen: Option<i64>,
}

/// A watch that was kept although another device had marked the episode unwatched.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SyncNoteView {
    pub id: i64,
    pub anilist_id: u32,
    pub episode: String,
    pub watch_device: String,
    pub unwatch_device: String,
    pub at: i64,
}

/// A plugin store another device has joined and this one has not.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SyncStoreOffer {
    pub address: String,
    pub name: String,
}

/// What the Sync section shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SyncStatus {
    /// Sync is set up on this device.
    pub enabled: bool,
    /// This device's name in the group.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub device_name: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub folder: Option<SyncFolderView>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub devices: Vec<SyncDeviceView>,
    /// The last time the folder road finished, in Unix seconds.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub last_synced: Option<i64>,
    /// A pass through the folder is running.
    pub syncing: bool,
    /// Why the last pass failed, in words.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub error: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub notes: Vec<SyncNoteView>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub stores_to_join: Vec<SyncStoreOffer>,
    /// Which cloud drives this build can sign in to, and how. Said whether or not sync is on, so
    /// the picker can hide or grey out what is unavailable.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub clouds: Vec<SyncCloudView>,
    /// The problem the folder road is in now, that a person can fix, until a pass works again.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub problem: Option<SyncProblem>,
}

/// How a cloud drive is signed in to on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SyncCloudWay {
    /// The app opens the sign-in page in the system's sheet or a custom tab and hands the
    /// redirect back with `Command::FinishSyncSignIn`.
    Sheet,
    /// The engine opens the browser itself and catches the redirect on a loopback port.
    Browser,
    /// A code to type on another screen: `Event::SyncSignIn` with `SyncSignInStep::Code`.
    DeviceCode,
    /// Not on this device, and why.
    Unavailable { reason: CloudUnavailable },
}

/// Why a cloud drive cannot be signed in to on this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CloudUnavailable {
    /// This build has no client registered for the service.
    NoClientId,
    /// The app did not say which kind of device it runs on, so no redirect can be chosen.
    NoPlatform,
    /// The service has no sign-in a TV can do: sign in on a phone and pair.
    NeedsPhone,
}

/// One cloud drive in the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SyncCloudView {
    /// `GoogleDrive`, `Dropbox` or `OneDrive`.
    pub service: SyncFolderKind,
    pub way: SyncCloudWay,
    /// This device holds a sign-in for it.
    pub signed_in: bool,
}

/// Where a cloud sign-in stands, for the sheet that shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SyncSignInStep {
    /// Open `url` in the system's web sheet, which ends on a redirect whose scheme is
    /// `callback_scheme`; hand the whole URL back with `Command::FinishSyncSignIn`, or send
    /// `Command::CancelSyncSignIn` when the sheet was closed.
    Sheet { url: String, callback_scheme: String },
    /// The browser was opened; the engine waits for it to come back. Cancel with
    /// `Command::CancelSyncSignIn`.
    Browser,
    /// Show `user_code` and send the person to `verification_url`. The engine polls by itself
    /// until `expires_at` (Unix seconds) and replaces the step when it is done.
    Code { user_code: String, verification_url: String, expires_at: i64, interval: u32 },
    /// Signed in; the folder is being set up.
    Finishing,
}

/// How a pairing or a join went wrong, for the sheet that asked.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SyncProblem {
    /// The recovery key does not open the folder's group.
    WrongRecoveryKey,
    /// The folder holds no group to join.
    NoGroup,
    /// The folder already holds a group; join it with its recovery key.
    GroupExists,
    /// The folder could not be reached.
    Unreachable,
    /// The service no longer accepts this device's sign-in (it was revoked, or the token ran
    /// out): offer "sign in again", which is `Command::SyncSignIn`.
    SignInNeeded { service: SyncFolderKind },
    /// The service asked for the sign-in to be refused, or the code was not accepted.
    SignInDenied { service: SyncFolderKind },
    /// The page or the code was not used in time: start again.
    SignInExpired { service: SyncFolderKind },
    /// The service cannot be signed in to on this device.
    CloudUnavailable { service: SyncFolderKind, reason: CloudUnavailable },
    /// Anything else, in words.
    Other(String),
}

/// What the person gave for a folder.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SyncFolderSpec {
    pub kind: SyncFolderKind,
    /// A path, a WebDAV address, or a Nextcloud server's address. Unused for the cloud drives,
    /// which sign in instead.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub address: Option<String>,
    /// The user name, for WebDAV.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub user: Option<String>,
    /// The password, for WebDAV.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub password: Option<crate::stores::Secret>,
}
