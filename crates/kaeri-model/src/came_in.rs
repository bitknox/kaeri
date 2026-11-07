//! What came in after a source was added: an invite to a plugin store, a book server or a folder.
//!
//! The engine says what arrived, counted (plugins by medium, libraries and series, files by
//! format), or when nothing did, a plain reason the app words: the server could not be reached, it
//! turned the password or key down, it holds nothing yet, or the folder holds only files Kaeri
//! can't open. Every event here is [`crate::event::Event::CameIn`].

use crate::book_sources::BookServerKind;

/// A count of files of one format: "14 MOBI".
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FormatCount {
    /// The format as people know it, upper case: `MOBI`, `AZW3`.
    pub format: String,
    pub count: u32,
}

/// One library on a book server, to pick before the server is added.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ServerLibrary {
    pub id: String,
    pub name: String,
    /// `None` when the server did not say.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub series: Option<u32>,
}

/// A book server signed in to and not yet added: "Found 212 series in 2 libraries". Send
/// `BookSourceCommand::AddServer` with `pending` and the libraries picked to add it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ServerFound {
    /// What `AddServer` names this server by. Held in memory only, until it is added or the app
    /// quits.
    pub pending: String,
    pub kind: BookServerKind,
    pub name: String,
    /// The host, for "Kavita at books.home.example".
    pub host: String,
    pub address: String,
    /// Every series this account sees, `None` when the server did not say.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub series: Option<u32>,
    /// Its libraries, all picked to start with. Empty for an OPDS catalogue, which has none to
    /// tell apart.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub libraries: Vec<ServerLibrary>,
    /// The last four characters of the Kavita key it signed in with: "Key ••••1234".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub key_last4: Option<String>,
}

/// A book server was added: "Added Kavita · 212 series".
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct AddedServer {
    /// Its id in Your sources.
    pub id: String,
    pub kind: BookServerKind,
    pub name: String,
    pub host: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub series: Option<u32>,
    /// How many libraries were added.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub libraries: u32,
    /// It showed no series: Kaeri looks again every 30 minutes and on refresh.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub empty: bool,
}

/// What a folder's scan found: "Found 41 series in 312 files".
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FolderReport {
    /// Its id in Your sources.
    pub id: String,
    pub name: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub path: Option<String>,
    pub series: u32,
    /// Books Kaeri reads, in those series.
    pub books: u32,
    /// Every file the scan met, books or not.
    pub files: u32,
    /// Series named by their own details (ComicInfo, the EPUB's package) rather than their
    /// folder: "matched 38 of them by their details".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub matched: u32,
    /// Books Kaeri can't open, by format, most first: "14 MOBI, 6 AZW3".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub unsupported: Vec<FormatCount>,
    /// Books locked with DRM or a password: "3 EPUB locked with DRM".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub locked: Vec<FormatCount>,
    /// The files behind `unsupported` and `locked`, for Show files. At most 200.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub listed: Vec<String>,
}

impl FolderReport {
    /// Files counted in `unsupported` and `locked`.
    pub fn skipped(&self) -> u32 {
        self.unsupported.iter().chain(&self.locked).map(|f| f.count).sum()
    }
}

/// What a plugin store holds for this device, after joining it: the plugins grouped by medium.
/// The rows themselves are in `Event::Stores`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StoreReport {
    /// Its id, as `Event::StoreJoined` has it.
    pub store: String,
    pub name: String,
    /// Who runs it, when the store says: "{ $owner } offers 6 plugins".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub owner: Option<String>,
    /// The ids of the plugins for reading, then for watching, that this device shows.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub for_reading: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub for_watching: Vec<String>,
    /// Plugins for a medium this device doesn't show: "2 more are for watching, which is off
    /// on this phone".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub other_medium: u32,
    /// The medium `other_medium` counts.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub other_is_reading: bool,
    /// Offer "Install all for reading": two or more reading plugins fit and none is installed.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub install_all: bool,
}

/// What `Command::InstallAllForReading` installed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PluginsInstalled {
    pub store: String,
    pub name: String,
    /// "4 plugins installed".
    pub installed: u32,
    /// The ids that did not install; each said in a toast of its own.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub failed: Vec<String>,
}

/// Why nothing came in.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NothingCameIn {
    /// "Couldn't reach { $host }".
    Unreachable { host: String },
    /// "{ $host } didn't accept that password".
    WrongPassword { host: String, kind: BookServerKind },
    /// "{ $host } didn't accept that key": a Kavita OPDS link whose key was reset.
    WrongKey { host: String },
    /// Something answered, and it is not a book server Kaeri knows.
    NotABookServer { host: String },
    /// "{ $host } shows this account no series". It can still be added with `pending`, and is
    /// looked at again every 30 minutes.
    EmptyCatalogue { found: ServerFound },
    /// The folder holds books, none of which Kaeri can open.
    UnsupportedFiles { folder: FolderReport },
    /// "Nothing in { $folder } that Kaeri can open": no books at all.
    EmptyFolder { folder: FolderReport },
    /// "{ $store } has no plugins yet".
    EmptyStore { store: StoreReport },
    /// "Every plugin in { $store } is for watching": none for what this device does.
    OtherMedium { store: StoreReport },
    /// The invite looks right and the store isn't answering.
    StoreUnreachable { host: String },
}

/// What came in, or why nothing did.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CameIn {
    /// A server signed in: pick its libraries, then add it.
    Found(ServerFound),
    ServerAdded(AddedServer),
    Folder(FolderReport),
    Store(StoreReport),
    Installed(PluginsInstalled),
    /// Nothing came in, and why. Not named `Nothing`, which is a Kotlin type.
    Empty(NothingCameIn),
}
