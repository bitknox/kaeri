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
    /// The account signed in to it, a name or an email, where the service says one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub account: Option<String>,
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
    /// The trackers it is signed in to, by id (`anilist`, `mal`, ...).
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub trackers: Vec<String>,
    /// The cloud drives, Nextcloud and WebDAV logins it holds a sign-in for, which this device
    /// can ask it for with `SyncCommand::CopySignIns`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub sign_ins: Vec<SyncFolderKind>,
    /// The kind of folder it syncs through, if any.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub folder: Option<SyncFolderKind>,
    /// What this device has asked it for and not yet received: it comes on the next session
    /// with it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub asked: Vec<SyncCopy>,
}

/// A sign-in one device can ask another for, over the paired link.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SyncCopy {
    /// Every tracker the other device is signed in to and this one is not.
    Trackers,
    /// The other device's sign-in to a cloud drive (`Dropbox`), or its Nextcloud or WebDAV login
    /// (only for the folder it syncs through).
    Cloud { service: SyncFolderKind },
}

/// Sync commands that came after `Command` was full.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SyncCommand {
    /// Ask `device` for sign-ins. They are sealed for this device and carried by the next
    /// session between the two, whichever side opens it; with `address` (`host:port`) this
    /// device opens one now. `device` `None` asks the next device this one pairs with, in the
    /// pairing itself. An empty `copy` withdraws what was asked. Answered with
    /// `Event::SyncSignInsCopied` when the answer comes, then `Event::SyncStatus`.
    CopySignIns { device: Option<String>, copy: Vec<SyncCopy>, address: Option<String> },
    /// Move the group to another folder, keeping the group, its keys and its recovery key, or
    /// with `None` stop using a folder and keep syncing nearby. The new folder must be empty or
    /// hold this group already; one holding another group is `SyncProblem::GroupExists`.
    ChangeSyncFolder { folder: Option<SyncFolderSpec> },
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
    /// The kind of folder the group syncs through: this device's, else the one the other
    /// devices say they use. A TV marks it in its picker.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub group_folder: Option<SyncFolderKind>,
    /// This device holds the recovery key and can say it again (`Command::ShowRecoveryKey`).
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub recovery_key: bool,
    /// When this device's recovery key was made, in Unix seconds.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub recovery_key_made: Option<i64>,
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
    /// The account signed in to, a name or an email, where the service says one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub account: Option<String>,
}

/// Where a cloud sign-in stands, for the sheet that shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SyncSignInStep {
    /// Open `url` in the system's web sheet, which ends on a redirect whose scheme is
    /// `callback_scheme`; hand the whole URL back with `Command::FinishSyncSignIn`, or send
    /// `Command::CancelSyncSignIn` when the sheet was closed.
    Sheet { url: String, callback_scheme: String },
    /// The browser was opened at `url`; the engine waits for it to come back. Offer to open
    /// `url` again; cancel with `Command::CancelSyncSignIn`.
    Browser { url: String },
    /// Show `url` for a phone to open, as a QR code (a TV's Nextcloud sign-in). The engine waits
    /// by itself; cancel with `Command::CancelSyncSignIn`.
    Page { url: String },
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
