//! Folders the device does not reach by path: a folder the platform opens for Kaeri, such as a
//! document tree picked in Android's system folder picker.
//!
//! The platform names every file and folder in such a tree by a reference of its own, a string
//! it gives and takes back, and opens a file for Kaeri to read and seek in. While a [`Tree`] is
//! [registered](register), [`crate::open`] and [`crate::library::scan`] take the references it
//! [claims](Tree::claims) as they take paths, so a book from a tree is read, served and sent like
//! any other. A reference is stored as the book's path and has to stay valid across launches.
//! It ends with the entry's name, so its extension says what kind of file it is, as a path's
//! does.

use crate::Source;
use std::{
    fs::File,
    io,
    path::Path,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// One file or folder in a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// What the tree names it by: what [`Tree::entries`] and [`Tree::open`] take.
    pub reference: String,
    /// Its name, as a folder listing shows it.
    pub name: String,
    pub folder: bool,
    /// Bytes, for a file.
    pub size: u64,
    /// Last modified, seconds since the epoch; 0 when the platform does not say.
    pub modified: i64,
}

/// A folder the platform opens for Kaeri. Its calls may block on the platform's file access for
/// a moment, so Kaeri makes them away from any screen's thread.
pub trait Tree: Send + Sync {
    /// Whether `reference` is one of this tree's: a folder it was given or an entry it listed.
    fn claims(&self, reference: &str) -> bool;

    /// The files and folders directly in the folder `reference` names. An error when it cannot
    /// be read: gone, or no longer open to Kaeri.
    fn entries(&self, reference: &str) -> io::Result<Vec<TreeEntry>>;

    /// What `reference` is now, file or folder, or an error when it is gone.
    fn entry(&self, reference: &str) -> io::Result<TreeEntry>;

    /// The file `reference` names, open for reading from its start.
    fn open(&self, reference: &str) -> io::Result<Box<dyn Source>>;
}

static TREES: RwLock<Vec<(u64, Arc<dyn Tree>)>> = RwLock::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(1);

/// A tree while it is registered: dropping this takes it out again.
#[must_use = "the tree is taken out again when this is dropped"]
pub struct Registered(u64);

impl std::fmt::Debug for Registered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Registered").field(&self.0).finish()
    }
}

impl Drop for Registered {
    fn drop(&mut self) {
        TREES
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|(id, _)| *id != self.0);
    }
}

/// Let [`crate::open`] and [`crate::library::scan`] read what `tree` claims, until the returned
/// guard is dropped.
pub fn register(tree: Arc<dyn Tree>) -> Registered {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    TREES.write().unwrap_or_else(std::sync::PoisonError::into_inner).push((id, tree));
    Registered(id)
}

/// The tree that claims `path`, if one does.
pub fn claiming(path: &Path) -> Option<Arc<dyn Tree>> {
    let text = path.to_str()?;
    TREES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .find(|(_, tree)| tree.claims(text))
        .map(|(_, tree)| tree.clone())
}

/// Whether `path` is a reference a registered tree claims, rather than a path on the device.
pub fn is_tree(path: &Path) -> bool {
    claiming(path).is_some()
}

/// What is at a book's path now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    pub folder: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// What is at `path` now, through the tree that claims it or from the file system.
pub fn stat(path: &Path) -> io::Result<Stat> {
    if let Some(tree) = claiming(path) {
        let entry = tree.entry(&path.to_string_lossy())?;
        let modified = u64::try_from(entry.modified)
            .ok()
            .filter(|s| *s > 0)
            .map(|s| UNIX_EPOCH + Duration::from_secs(s));
        return Ok(Stat { folder: entry.folder, size: entry.size, modified });
    }
    let meta = std::fs::metadata(path)?;
    Ok(Stat { folder: meta.is_dir(), size: meta.len(), modified: meta.modified().ok() })
}

/// The file at `path`, open for reading, through the tree that claims it or from the file
/// system.
pub fn open_file(path: &Path) -> io::Result<Box<dyn Source>> {
    match claiming(path) {
        Some(tree) => tree.open(&path.to_string_lossy()),
        None => Ok(Box::new(File::open(path)?)),
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! A tree over a folder on disk, named by references under a scheme of its own, as a
    //! platform's would be.

    use std::{collections::HashSet, path::PathBuf, sync::Mutex, time::UNIX_EPOCH};

    use super::*;

    pub(crate) struct DiskTree {
        pub scheme: String,
        pub root: PathBuf,
        /// References that answer as if the platform no longer let Kaeri in.
        pub refused: Mutex<HashSet<String>>,
    }

    impl DiskTree {
        pub(crate) fn new(scheme: &str, root: &Path) -> Self {
            Self {
                scheme: format!("{scheme}://"),
                root: root.to_owned(),
                refused: Mutex::default(),
            }
        }

        pub(crate) fn top(&self) -> String {
            format!("{}top", self.scheme)
        }

        fn path_of(&self, reference: &str) -> io::Result<PathBuf> {
            if self.refused.lock().unwrap().contains(reference) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "access withdrawn",
                ));
            }
            let rest = reference
                .strip_prefix(&self.scheme)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not this tree's"))?;
            let rest = rest.strip_prefix("top").unwrap_or(rest);
            Ok(self.root.join(rest.trim_start_matches('/')))
        }

        fn reference_of(&self, path: &Path) -> String {
            let rel = path.strip_prefix(&self.root).unwrap_or(path);
            format!("{}top/{}", self.scheme, rel.to_string_lossy())
        }

        fn describe(&self, path: &Path) -> io::Result<TreeEntry> {
            let meta = std::fs::metadata(path)?;
            Ok(TreeEntry {
                reference: self.reference_of(path),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                folder: meta.is_dir(),
                size: if meta.is_dir() { 0 } else { meta.len() },
                modified: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs() as i64),
            })
        }
    }

    impl Tree for DiskTree {
        fn claims(&self, reference: &str) -> bool {
            reference.starts_with(&self.scheme)
        }

        fn entries(&self, reference: &str) -> io::Result<Vec<TreeEntry>> {
            let dir = self.path_of(reference)?;
            let mut out = Vec::new();
            for entry in std::fs::read_dir(dir)? {
                out.push(self.describe(&entry?.path())?);
            }
            Ok(out)
        }

        fn entry(&self, reference: &str) -> io::Result<TreeEntry> {
            self.describe(&self.path_of(reference)?)
        }

        fn open(&self, reference: &str) -> io::Result<Box<dyn Source>> {
            Ok(Box::new(File::open(self.path_of(reference)?)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::{fake::DiskTree, *};

    #[test]
    fn a_tree_answers_for_what_it_claims_only_while_registered() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Quiet Field.cbz"), b"12345").unwrap();
        let tree = Arc::new(DiskTree::new("tree-stat", dir.path()));
        let file = std::path::PathBuf::from(format!("{}/Quiet Field.cbz", tree.top()));
        assert!(stat(&file).is_err());
        let registered = register(tree.clone());
        assert!(is_tree(&file));
        assert!(!is_tree(dir.path()));
        let at = stat(&file).unwrap();
        assert!(!at.folder);
        assert_eq!(at.size, 5);
        assert!(at.modified.is_some());
        assert!(stat(Path::new(&tree.top())).unwrap().folder);
        let mut text = String::new();
        open_file(&file).unwrap().read_to_string(&mut text).unwrap();
        assert_eq!(text, "12345");
        // A path on the device is still read from the device.
        assert_eq!(stat(&dir.path().join("Quiet Field.cbz")).unwrap().size, 5);
        drop(registered);
        assert!(!is_tree(&file));
    }
}
