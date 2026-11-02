//! The folder road's five operations, and the targets that implement them.
//!
//! A target stores sealed files and sees nothing else, so a cloud service, a WebDAV server and a
//! directory the person picked are all the same to the log above them. Paths are relative to the
//! sync folder's root, `/`-separated, with no `..`.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum FolderError {
    #[error("{0} is not there")]
    NotFound(String),
    /// A segment is written once and never changed, so a second write to a name is refused.
    #[error("{0} is already there")]
    Exists(String),
    /// The sign-in was refused or has expired: fixed in place by signing in again.
    #[error("the sign-in was refused")]
    Unauthorized,
    #[error("the server could not be reached: {0}")]
    Unreachable(String),
    #[error("the server said: {0}")]
    Server(String),
    #[error("{0}")]
    Io(String),
    #[error("not a path this folder takes: {0}")]
    BadPath(String),
}

impl From<std::io::Error> for FolderError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound(e.to_string()),
            std::io::ErrorKind::AlreadyExists => Self::Exists(e.to_string()),
            _ => Self::Io(e.to_string()),
        }
    }
}

/// One name in a directory listing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// A place the log can be kept. Implementations are safe to call from several tasks.
#[async_trait]
pub trait FolderTarget: Send + Sync + 'static {
    /// The names directly under `dir` (`""` is the root), sorted. A directory that does not exist
    /// lists as empty.
    async fn list(&self, dir: &str) -> Result<Vec<Entry>, FolderError>;

    async fn read(&self, path: &str) -> Result<Vec<u8>, FolderError>;

    /// Create `path` with `bytes`, and its parent directories. All of it or none: a reader never
    /// sees half a file (a temporary name, then a move). [`FolderError::Exists`] when the name is
    /// taken.
    async fn write_new(&self, path: &str, bytes: &[u8]) -> Result<(), FolderError>;

    /// Replace `to` with `from`. Used for the few files that are rewritten, and by targets that
    /// write through a temporary name.
    async fn rename(&self, from: &str, to: &str) -> Result<(), FolderError>;

    async fn remove(&self, path: &str) -> Result<(), FolderError>;

    /// A name for the status line: "Synced 4 min ago · dav.example".
    fn describe(&self) -> String;
}

/// Refuse a path that is empty-segmented, absolute or climbs out of the folder.
pub fn check_path(path: &str) -> Result<(), FolderError> {
    let bad = path.starts_with('/')
        || path.contains('\\')
        || path.split('/').any(|part| part == ".." || part == "." || part.is_empty())
            && !path.is_empty();
    if bad { Err(FolderError::BadPath(path.to_owned())) } else { Ok(()) }
}

/// A folder kept in memory: for tests, and for two simulated devices to share. Clones share it.
#[derive(Clone, Default)]
pub struct MemoryTarget {
    files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    down: Arc<Mutex<bool>>,
}

impl MemoryTarget {
    pub fn new() -> Self {
        Self::default()
    }

    /// Make every call fail as an unreachable server does, and stop again: the offline case.
    pub fn set_down(&self, down: bool) {
        *self.down.lock().unwrap_or_else(PoisonError::into_inner) = down;
    }

    /// Every path held, for a test to look at what a stranger would see.
    pub fn paths(&self) -> Vec<String> {
        self.files.lock().unwrap_or_else(PoisonError::into_inner).keys().cloned().collect()
    }

    fn check_up(&self) -> Result<(), FolderError> {
        if *self.down.lock().unwrap_or_else(PoisonError::into_inner) {
            Err(FolderError::Unreachable("offline".into()))
        } else {
            Ok(())
        }
    }

    /// Overwrite a file as a hostile or broken store would. Tests only.
    pub fn put_raw(&self, path: &str, bytes: Vec<u8>) {
        self.files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(path.to_owned(), bytes);
    }
}

#[async_trait]
impl FolderTarget for MemoryTarget {
    async fn list(&self, dir: &str) -> Result<Vec<Entry>, FolderError> {
        self.check_up()?;
        check_path(dir)?;
        let prefix = if dir.is_empty() { String::new() } else { format!("{dir}/") };
        let files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        let mut out: BTreeMap<String, Entry> = BTreeMap::new();
        for (path, bytes) in files.range(prefix.clone()..) {
            let Some(rest) = path.strip_prefix(&prefix) else { break };
            match rest.split_once('/') {
                Some((name, _)) => {
                    out.entry(name.to_owned()).or_insert(Entry {
                        name: name.to_owned(),
                        is_dir: true,
                        size: 0,
                    });
                }
                None => {
                    out.insert(
                        rest.to_owned(),
                        Entry {
                            name: rest.to_owned(),
                            is_dir: false,
                            size: bytes.len() as u64,
                        },
                    );
                }
            }
        }
        Ok(out.into_values().collect())
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, FolderError> {
        self.check_up()?;
        check_path(path)?;
        let files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        files.get(path).cloned().ok_or_else(|| FolderError::NotFound(path.to_owned()))
    }

    async fn write_new(&self, path: &str, bytes: &[u8]) -> Result<(), FolderError> {
        self.check_up()?;
        check_path(path)?;
        let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        if files.contains_key(path) {
            return Err(FolderError::Exists(path.to_owned()));
        }
        files.insert(path.to_owned(), bytes.to_vec());
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FolderError> {
        self.check_up()?;
        check_path(from)?;
        check_path(to)?;
        let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        let bytes = files.remove(from).ok_or_else(|| FolderError::NotFound(from.to_owned()))?;
        files.insert(to.to_owned(), bytes);
        Ok(())
    }

    async fn remove(&self, path: &str) -> Result<(), FolderError> {
        self.check_up()?;
        check_path(path)?;
        let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        files.remove(path);
        Ok(())
    }

    fn describe(&self) -> String {
        "memory".into()
    }
}

/// A directory on this machine: a folder the person picked, or one something else carries
/// (Syncthing, iCloud Drive, a Nextcloud client). Blocking file calls run on the blocking pool.
#[derive(Clone)]
pub struct DirTarget {
    root: PathBuf,
}

impl DirTarget {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, FolderError> {
        check_path(path)?;
        Ok(path
            .split('/')
            .filter(|p| !p.is_empty())
            .fold(self.root.clone(), |p, part| p.join(part)))
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, FolderError> + Send + 'static,
) -> Result<T, FolderError> {
    tokio::task::spawn_blocking(f).await.map_err(|e| FolderError::Io(e.to_string()))?
}

#[async_trait]
impl FolderTarget for DirTarget {
    async fn list(&self, dir: &str) -> Result<Vec<Entry>, FolderError> {
        let path = self.resolve(dir)?;
        blocking(move || {
            let read = match std::fs::read_dir(&path) {
                Ok(read) => read,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
                Err(e) => return Err(e.into()),
            };
            let mut out = Vec::new();
            for entry in read {
                let entry = entry?;
                let meta = entry.metadata()?;
                let name = entry.file_name().to_string_lossy().into_owned();
                // A temporary name a writer has not moved into place yet is not there.
                if name.starts_with(".kaeri-tmp-") {
                    continue;
                }
                out.push(Entry { name, is_dir: meta.is_dir(), size: meta.len() });
            }
            out.sort();
            Ok(out)
        })
        .await
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, FolderError> {
        let path = self.resolve(path)?;
        blocking(move || Ok(std::fs::read(path)?)).await
    }

    async fn write_new(&self, path: &str, bytes: &[u8]) -> Result<(), FolderError> {
        let target = self.resolve(path)?;
        let bytes = bytes.to_vec();
        blocking(move || {
            if target.exists() {
                return Err(FolderError::Exists(target.display().to_string()));
            }
            let parent = target.parent().ok_or_else(|| FolderError::BadPath("root".into()))?;
            std::fs::create_dir_all(parent)?;
            let name = target
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let tmp = parent.join(format!(".kaeri-tmp-{name}"));
            std::fs::write(&tmp, &bytes)?;
            // `hard_link` fails when the name is taken, which `rename` would silently overwrite.
            match std::fs::hard_link(&tmp, &target) {
                Ok(()) => {
                    let _ = std::fs::remove_file(&tmp);
                    Ok(())
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let _ = std::fs::remove_file(&tmp);
                    Err(FolderError::Exists(name))
                }
                // A file system with no hard links (some synced folders): move it.
                Err(_) => Ok(std::fs::rename(&tmp, &target)?),
            }
        })
        .await
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FolderError> {
        let (from, to) = (self.resolve(from)?, self.resolve(to)?);
        blocking(move || Ok(std::fs::rename(from, to)?)).await
    }

    async fn remove(&self, path: &str) -> Result<(), FolderError> {
        let path = self.resolve(path)?;
        blocking(move || match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        })
        .await
    }

    fn describe(&self) -> String {
        self.root
            .file_name()
            .map_or_else(|| "folder".into(), |n| n.to_string_lossy().into_owned())
    }
}

/// Where a directory target's root is, for tests.
impl AsRef<Path> for DirTarget {
    fn as_ref(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn exercise(t: &dyn FolderTarget) {
        assert!(t.list("").await.unwrap().is_empty());
        t.write_new("a/b/one.seg", b"1").await.unwrap();
        t.write_new("a/two.seg", b"22").await.unwrap();
        assert!(matches!(t.write_new("a/two.seg", b"x").await, Err(FolderError::Exists(_))));
        let root = t.list("").await.unwrap();
        assert_eq!(root.len(), 1);
        assert!(root[0].is_dir);
        let a = t.list("a").await.unwrap();
        assert_eq!(
            a.iter().map(|e| (e.name.as_str(), e.is_dir)).collect::<Vec<_>>(),
            [("b", true), ("two.seg", false)]
        );
        assert_eq!(t.read("a/two.seg").await.unwrap(), b"22");
        t.rename("a/two.seg", "a/three.seg").await.unwrap();
        assert!(matches!(t.read("a/two.seg").await, Err(FolderError::NotFound(_))));
        t.remove("a/three.seg").await.unwrap();
        t.remove("a/three.seg").await.unwrap();
        assert!(matches!(t.read("../x").await, Err(FolderError::BadPath(_))));
    }

    #[tokio::test]
    async fn memory_target_keeps_the_contract() {
        exercise(&MemoryTarget::new()).await;
    }

    #[tokio::test]
    async fn dir_target_keeps_the_contract() {
        let tmp = tempfile::tempdir().unwrap();
        exercise(&DirTarget::new(tmp.path().join("kaeri-sync"))).await;
    }
}
