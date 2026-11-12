//! The pages of a chapter fetch, kept one file each beside where the chapter's file goes, so a
//! fetch cut short (the app closed, the queue held, a page that failed) starts again from the
//! pages it lacks.
//!
//! A page is written under a temporary name and renamed once whole, to a name that carries its
//! length and checksum: `00003-48213-<fnv64 hex>.page`. A page kept from before is used again only
//! when its length and checksum still match and it still reads as an image; any other is
//! deleted and fetched again. `count` holds how many pages the chapter had: a source that now
//! names another number starts the chapter over.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

/// FNV-1a, 64 bits: a checksum for a page against a torn or changed file, not against tampering.
fn checksum(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn name_of(index: u32, bytes: &[u8]) -> String {
    format!("{index:05}-{}-{:016x}.page", bytes.len(), checksum(bytes))
}

/// `(index, length, checksum)` from a page's name.
fn parse(name: &str) -> Option<(u32, usize, u64)> {
    let stem = name.strip_suffix(".page")?;
    let mut parts = stem.splitn(3, '-');
    let index = parts.next()?.parse().ok()?;
    let len = parts.next()?.parse().ok()?;
    let sum = u64::from_str_radix(parts.next()?, 16).ok()?;
    Some((index, len, sum))
}

/// The folder a chapter's pages are kept in, for the file at `stem`.
pub(crate) fn pages_of(stem: &Path) -> PathBuf {
    let mut name = stem.file_name().map(std::ffi::OsStr::to_os_string).unwrap_or_default();
    name.push(".pages");
    stem.with_file_name(name)
}

/// How a page is written to its file: as the disk does, or as a test's disk.
pub(crate) type Write = fn(&Path, &[u8]) -> io::Result<()>;

/// The pages kept for one chapter.
pub(crate) struct KeptPages {
    dir: PathBuf,
    good: BTreeMap<u32, PathBuf>,
    write: Write,
}

impl KeptPages {
    /// The pages already kept for a chapter of `total` pages, each checked; the folder is made
    /// when there is none.
    pub(crate) fn open(dir: &Path, total: u32) -> io::Result<Self> {
        let count = dir.join("count");
        let same = fs::read_to_string(&count).ok().and_then(|t| t.trim().parse::<u32>().ok())
            == Some(total);
        if !same && dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        fs::create_dir_all(dir)?;
        if !same {
            fs::write(&count, total.to_string())?;
        }
        let mut good = BTreeMap::new();
        for entry in fs::read_dir(dir)?.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "count" {
                continue;
            }
            let sound = parse(&name).filter(|(index, len, sum)| {
                *index < total
                    && fs::read(&path).is_ok_and(|bytes| {
                        bytes.len() == *len
                            && checksum(&bytes) == *sum
                            && kaeri_books::mime::sniff_image(&bytes).is_some()
                    })
            });
            match sound {
                Some((index, ..)) if !good.contains_key(&index) => {
                    good.insert(index, path);
                }
                // Torn, changed, a temporary file a write left, or a second copy.
                _ => {
                    let _ = fs::remove_file(&path);
                }
            }
        }
        Ok(Self { dir: dir.to_path_buf(), good, write: |path, bytes| fs::write(path, bytes) })
    }

    pub(crate) fn has(&self, index: u32) -> bool {
        self.good.contains_key(&index)
    }

    /// Where page `index` is kept, once it is.
    pub(crate) fn path(&self, index: u32) -> Option<&Path> {
        self.good.get(&index).map(PathBuf::as_path)
    }

    /// Page `index` is no longer kept as it was: its file went.
    pub(crate) fn forget(&mut self, index: u32) {
        self.good.remove(&index);
    }

    /// Pages are written with `write` from now on.
    #[cfg(test)]
    pub(crate) fn write_with(&mut self, write: Write) {
        self.write = write;
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// How many pages are kept and sound.
    pub(crate) fn len(&self) -> usize {
        self.good.len()
    }

    /// Keep page `index`, whole or not at all.
    pub(crate) fn put(&mut self, index: u32, bytes: &[u8]) -> io::Result<()> {
        let temp = self.dir.join(format!("{index:05}.tmp"));
        (self.write)(&temp, bytes)?;
        let path = self.dir.join(name_of(index, bytes));
        fs::rename(&temp, &path)?;
        self.good.insert(index, path);
        Ok(())
    }

    pub(crate) fn read(&self, index: u32) -> io::Result<Vec<u8>> {
        let path = self.good.get(&index).ok_or(io::ErrorKind::NotFound)?;
        fs::read(path)
    }

    /// The chapter's file is written, or the fetch is no longer wanted: the pages go.
    pub(crate) fn remove(dir: &Path) {
        if let Err(e) = fs::remove_dir_all(dir)
            && e.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(error = %e, "a chapter's kept pages were not removed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(fill: u8) -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        bytes.extend_from_slice(&[fill; 64]);
        bytes
    }

    #[test]
    fn kept_pages_are_checked_before_they_are_used_again() {
        let root = tempfile::tempdir().unwrap();
        let dir = pages_of(&root.path().join("c1"));
        let mut kept = KeptPages::open(&dir, 4).unwrap();
        kept.put(0, &png(1)).unwrap();
        kept.put(1, &png(2)).unwrap();
        kept.put(2, &png(3)).unwrap();
        drop(kept);

        // Page 1 is torn, page 2 is no image any more, and a write left a temporary file.
        let names: BTreeMap<u32, PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                parse(&name).map(|(i, ..)| (i, e.path()))
            })
            .collect();
        fs::write(&names[&1], &png(2)[..20]).unwrap();
        let mut not_image = png(3);
        not_image[0] = b'x';
        fs::write(&names[&2], not_image).unwrap();
        fs::write(dir.join("00003.tmp"), png(4)).unwrap();

        let kept = KeptPages::open(&dir, 4).unwrap();
        assert!(kept.has(0));
        assert!(!kept.has(1) && !kept.has(2) && !kept.has(3));
        assert_eq!(kept.len(), 1);
        assert_eq!(kept.read(0).unwrap(), png(1));
        // What was not sound is gone from the folder.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2, "count and page 0");

        // The source now names five pages: the chapter starts over.
        let kept = KeptPages::open(&dir, 5).unwrap();
        assert_eq!(kept.len(), 0);
        KeptPages::remove(&dir);
        assert!(!dir.exists());
    }
}
