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
    /// What kind of device it is, as it says: for its icon, its page's Kind row and `$kind`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub kind: Option<SyncDeviceKind>,
    /// Its Sync switch is off: the group holds back from it, and it from the group, until a
    /// device turns it on again with `SyncCommand::SetDeviceSync`. On this device's own row: one
    /// of the others turned this device's Sync off.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub paused: bool,
    /// The fingerprint (hex) of its remote's key, when it and this device are a TV and a device
    /// that drives TVs: a TV's server key on a phone, a phone's key on the TV. Match it against
    /// `FfiTvSender::paired` (a phone) or `FfiTvRemote::paired` (a TV): a match is the remote
    /// pairing the Remote switch is about. `None`: no Remote switch on its page.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub remote_key: Option<String>,
    /// The pair's Remote switch is off: the TV refuses that phone's commands, and keeps the
    /// pairing so turning it on needs no code. Set with `SyncCommand::SetDeviceRemote`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub remote_off: bool,
}

/// What kind of device a group's device is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SyncDeviceKind {
    Phone,
    Tablet,
    Computer,
    Tv,
}

impl SyncDeviceKind {
    /// The register's word for it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Tablet => "tablet",
            Self::Computer => "computer",
            Self::Tv => "tv",
        }
    }

    pub fn from_word(word: &str) -> Option<Self> {
        Some(match word {
            "phone" => Self::Phone,
            "tablet" => Self::Tablet,
            "computer" => Self::Computer,
            "tv" => Self::Tv,
            _ => return None,
        })
    }
}

/// Why pairing by code did not work, for the Add a device sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SyncPairProblem {
    /// The device showing a code was found, and the code did not match (`pair-err-code`).
    WrongCode,
    /// No device showing a code answered on this network (`pair-err-not-found`).
    NotFound,
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
    /// Pair with the device showing `code` on this network, found through the app's DNS-SD
    /// browse (`FfiEngine::set_sync_network`), then swap histories, as `Command::PairJoin` does
    /// with an address. Sets sync up on this device first if it is not. A miss is
    /// `Event::SyncPairFailed`, then `Event::SyncProblem` in words.
    PairWithCode { device_name: String, code: String },
    /// Rename this device: the name every device in the group lists it under, and the one it
    /// pairs with from now on. Kept while sync is off too, as `SyncStatus::device_name`.
    /// Answered with `Event::SyncStatus`.
    RenameDevice { name: String },
    /// Turn a device's Sync switch on or off. Off, the group holds back from it on both roads
    /// and it from the group, keeping what it has; on again, it catches up. Every device in the
    /// group sees the switch. Not for this device: `LeaveSync` is how it stops.
    SetDeviceSync { device: String, on: bool },
    /// Turn the Remote switch between this device and `device` on or off: a TV and a device
    /// that drives TVs, paired as a remote. Off, the TV refuses that phone's commands and keeps
    /// the pairing. Both devices see the switch.
    SetDeviceRemote { device: String, on: bool },
    /// Join plugin stores through `device`, a paired device that is a member of them: it makes
    /// this device its own invite and hands it over the paired link, and this device spends it,
    /// so it gets a token of its own. `stores` are addresses from `SyncStatus::stores_to_join`;
    /// empty means every store `device` is a member of that this device is not.
    ///
    /// An invite already handed over (`SyncStoreOffer::invite_from` is `device`) is spent at
    /// once. The rest are asked of `device` and come with the next session between the two,
    /// whichever side opens it; with `address` (`host:port`) this device opens one now.
    ///
    /// `device` `None` asks the next device this one pairs with, in the pairing itself, as
    /// first launch does: its invites are held, not spent, and the stores list with
    /// `invite_from` set until this command names them with that device. Held invites live in
    /// memory only and are gone when the app quits.
    ///
    /// Answered per store with `Event::SyncStoreJoined` or `Event::SyncStoreJoinFailed`, then
    /// `Event::SyncStatus`.
    JoinStoresFrom { device: Option<String>, stores: Vec<String>, address: Option<String> },
    /// "Already use Kaeri on another device?": ask `device` (or, with `None`, the next device to
    /// pair, in the pairing itself) for its setup. It hands over an invite to every plugin store
    /// it is a member of, each book server with its sign-in, its answer to Watch, read, or both,
    /// and what it is signed in to, all sealed for this device inside the paired link. Nothing
    /// is joined or added until the person says: `JoinStoresFrom` and `AddBookServersFrom`.
    /// Answered with `Event::SetupFrom`, then `Event::SyncStatus`.
    AskSetupFrom { device: Option<String>, address: Option<String> },
    /// Add book servers `device` handed over (`SyncStatus::book_servers_to_add`), their sign-ins
    /// going to this device's secure store. `servers` are offer ids; empty adds every one held
    /// from `device`. Answered per server with `Event::CameIn`, then the book sources view and
    /// `Event::SyncStatus`.
    AddBookServersFrom { device: String, servers: Vec<String> },
}

/// Why a plugin store could not be joined through a paired device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SyncStoreProblem {
    /// The store could not be reached, from this device or from the paired one, or the paired
    /// device could not be reached (`sync-store-err-offline`). Trying again can work.
    Offline,
    /// The store turned the invite down: it ended or was cancelled before it was spent, or the
    /// store refused to make one (`sync-store-err-refused`).
    Refused,
    /// The paired device can't invite this one: it joined the store with the store's open link,
    /// it is no longer a member, or its Kaeri is too old to make invites for another device
    /// (`sync-store-err-not-allowed`).
    NotAllowed,
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
    /// The paired device whose invite to this store this device holds, ready to spend with
    /// `SyncCommand::JoinStoresFrom`: the first launch success sheet's "Join the plugin store"
    /// switch. `None` when no invite is held.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub invite_from: Option<String>,
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
    /// Book servers a paired device handed over with their sign-ins, not yet added here: one
    /// switch each, on, for `SyncCommand::AddBookServersFrom`. Held in memory only.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub book_servers_to_add: Vec<SyncBookServerOffer>,
}

/// A book server a paired device signed in to, handed over with its sign-in over the paired
/// link. The sign-in itself never leaves the engine: adding it puts it in this device's secure
/// store.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SyncBookServerOffer {
    /// The paired device it came from.
    pub from: String,
    /// What `SyncCommand::AddBookServersFrom` names it by.
    pub id: String,
    pub kind: crate::book_sources::BookServerKind,
    pub name: String,
    /// "Add books.home.example".
    pub host: String,
    pub address: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub user: Option<String>,
    /// It signs in with a Kavita key, of which only these four characters show.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub key_last4: Option<String>,
    /// The libraries the other device asks, by id; empty for all of them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub libraries: Vec<String>,
    /// It already is one of this device's servers.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub added: bool,
}

/// A page of the first run another device's setup can answer. Answered pages are skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum FirstRunPage {
    /// Watch, read, or both.
    Medium,
    /// Lists: the trackers.
    Lists,
    /// Sources: plugin stores, book servers and folders.
    Sources,
}

/// What joining through a paired device brings, for the first run's "From { $device }" group:
/// every plugin store it is a member of (each device still gets its own invite), each book server
/// with its sign-in, and its answer to Watch, read, or both. Folders stay on the device they are
/// on, and are only counted.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct JoinedSetup {
    /// The paired device's id.
    pub device: String,
    /// Its name: "From Desk".
    pub device_name: String,
    /// Its answer to Watch, read, or both: "Reading, as on Desk".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub medium: Option<crate::medium::KaeriIsFor>,
    /// Its plugin stores this device has not joined, each with an invite held for it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub stores: Vec<SyncStoreOffer>,
    /// Its book servers, each with its sign-in held.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub servers: Vec<SyncBookServerOffer>,
    /// Its trackers signed in, by id: "AniList and MangaUpdates".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub signed_in: Vec<String>,
    /// Its folders, which can't sync: "Folders stay on Desk".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub folders: u32,
    /// The first-run pages this answers, which the first run skips.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub answered: Vec<FirstRunPage>,
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
    /// The browser came back to the engine with access allowed, and the sign-in is being
    /// finished. Said on a computer, whose loopback catches the redirect; an app that hands a
    /// sheet's redirect back with `Command::FinishSyncSignIn` knows it then by itself.
    Connecting,
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
