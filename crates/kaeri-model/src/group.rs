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
