//! Book folders the platform opens for Kaeri rather than by path: a folder picked in Android's
//! system folder picker, which the app reaches through its content resolver.

use std::{
    io::{self, BufReader, Read, Seek, SeekFrom},
    sync::Arc,
};

use kaeri_engine::host::{BookFile, FolderEntry, FolderTree};

/// The app's way into the book folders it opens for Kaeri, implemented by an app whose folders
/// have no path Kaeri can read: Android's, for a folder picked in the system folder picker.
/// Optional: an app that has none passes nothing in [`crate::FfiStart::folder_access`], and a
/// folder added with only a [`kaeri_model::reading::FolderRef::tree_uri`] then can't be read.
///
/// The app names each file and folder by a reference of its own: a string it gives in
/// [`FfiFolderEntry::reference`] and takes back. A reference is kept as the book's place and has
/// to name the same file after the app starts again, and it ends with the file's name, so its
/// extension says what kind of file it is.
///
/// Called on the engine's blocking threads, never the app's main thread. A call that fails,
/// because the file is gone or the app may no longer read the folder, answers `None` or
/// `false`, and the folder is then marked as not found.
#[uniffi::export(with_foreign)]
pub trait FfiFolderAccess: Send + Sync {
    /// Whether `reference` is one of the app's: a folder it gave Kaeri, or anything listed in
    /// one.
    fn claims(&self, reference: String) -> bool;

    /// What is directly in the folder `reference` names, or `None` when it can't be read.
    fn list(&self, folder: String) -> Option<Vec<FfiFolderEntry>>;

    /// What `reference` names now, or `None` when it is gone or can't be read.
    fn entry(&self, reference: String) -> Option<FfiFolderEntry>;

    /// Open the file `reference` names for reading, or `None` when it can't be.
    fn open_file(&self, reference: String) -> Option<FfiOpenedFile>;

    /// Up to `length` bytes of the open `file` (its handle) from `offset`: fewer only at its end,
    /// none past it. `None` when it can't be read.
    fn read(&self, file: u64, offset: u64, length: u32) -> Option<Vec<u8>>;

    /// Let go of the open `file` (its handle). Called once for every handle [`Self::open_file`]
    /// gave.
    fn close(&self, file: u64);
}

/// A file the app opened for Kaeri to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FfiOpenedFile {
    /// What [`FfiFolderAccess::read`] and [`FfiFolderAccess::close`] take.
    pub handle: u64,
    /// Its length in bytes.
    pub size: u64,
}

/// A file or folder in a folder the app opens.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiFolderEntry {
    /// What the app names it by, for [`FfiFolderAccess`]'s calls.
    pub reference: String,
    /// Its name as the folder shows it: `ch001.cbz`.
    pub name: String,
    pub folder: bool,
    /// Bytes, for a file.
    pub size: u64,
    /// Last modified, seconds since the epoch; 0 when the app can't say.
    pub modified: i64,
}

impl From<FfiFolderEntry> for FolderEntry {
    fn from(e: FfiFolderEntry) -> Self {
        Self {
            reference: e.reference,
            name: e.name,
            folder: e.folder,
            size: e.size,
            modified: e.modified,
        }
    }
}

/// An [`FfiFolderAccess`], as the books' [`FolderTree`].
pub(crate) struct Folders(pub(crate) Arc<dyn FfiFolderAccess>);

fn unreadable(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, format!("the app could not {what}"))
}

impl FolderTree for Folders {
    fn claims(&self, reference: &str) -> bool {
        self.0.claims(reference.to_owned())
    }

    fn entries(&self, reference: &str) -> io::Result<Vec<FolderEntry>> {
        let listed =
            self.0.list(reference.to_owned()).ok_or_else(|| unreadable("list the folder"))?;
        Ok(listed.into_iter().map(Into::into).collect())
    }

    fn entry(&self, reference: &str) -> io::Result<FolderEntry> {
        self.0
            .entry(reference.to_owned())
            .map(Into::into)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the app does not find it"))
    }

    fn open(&self, reference: &str) -> io::Result<Box<dyn BookFile>> {
        let opened = self
            .0
            .open_file(reference.to_owned())
            .ok_or_else(|| unreadable("open the file"))?;
        let file =
            Opened { app: self.0.clone(), handle: opened.handle, size: opened.size, at: 0 };
        // Archives read a few bytes at a time while they find their way: each crossing to the
        // app carries a good piece.
        Ok(Box::new(BufReader::with_capacity(CHUNK as usize, file)))
    }
}

/// How much one read asks the app for.
const CHUNK: u32 = 256 * 1024;

/// A file the app opened, read from where it was left.
struct Opened {
    app: Arc<dyn FfiFolderAccess>,
    handle: u64,
    size: u64,
    at: u64,
}

impl Read for Opened {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.at >= self.size {
            return Ok(0);
        }
        let want = u32::try_from(buf.len()).unwrap_or(u32::MAX).min(CHUNK);
        let bytes = self
            .app
            .read(self.handle, self.at, want)
            .ok_or_else(|| unreadable("read the file"))?;
        let n = bytes.len().min(buf.len());
        buf[..n].copy_from_slice(&bytes[..n]);
        self.at += n as u64;
        Ok(n)
    }
}

impl Seek for Opened {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let at = match to {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::Current(d) => self.at.checked_add_signed(d),
            SeekFrom::End(d) => self.size.checked_add_signed(d),
        };
        self.at =
            at.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "before the start"))?;
        Ok(self.at)
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        self.app.close(self.handle);
    }
}
