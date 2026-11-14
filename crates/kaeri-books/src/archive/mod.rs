//! Archives: ZIP, RAR and 7z behind one shape. Each lists its files with normalised names and
//! declared sizes, reads one entry by index, and extracts many in archive order, which is the
//! cheap way through a solid archive.

use std::io::{Read, Seek, SeekFrom};
use std::ops::ControlFlow;

use crate::{BookError, Format, Limits, Result, path};

#[cfg(feature = "cbr")]
mod rar;
#[cfg(feature = "cb7")]
mod sevenz;
mod zip;

/// A seekable byte source: an open file, or bytes in memory.
pub trait Source: Read + Seek + Send {}
impl<T: Read + Seek + Send> Source for T {}

/// The archive formats this crate reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveKind {
    Zip,
    Rar,
    SevenZip,
}

impl ArchiveKind {
    /// The book format an archive of images in this container is.
    pub fn format(self) -> Format {
        match self {
            Self::Zip => Format::Cbz,
            Self::Rar => Format::Cbr,
            Self::SevenZip => Format::Cb7,
        }
    }
}

/// One file in an archive. Directories, links and entries whose names would leave the archive
/// are not listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The normalised name, `/`-separated and relative.
    pub name: String,
    /// The declared decompressed size.
    pub size: u64,
    /// The declared compressed size, where the format records one per entry.
    pub packed: u64,
    /// Whether the entry's data is encrypted. Reading it fails with [`BookError::Protected`].
    pub encrypted: bool,
    /// The index the backend knows the entry by.
    pub(crate) raw: usize,
}

/// What the backend reports for each raw entry before names are normalised.
pub(crate) struct RawEntry {
    pub name: String,
    pub size: u64,
    pub packed: u64,
    pub encrypted: bool,
    pub raw: usize,
}

enum Backend {
    Zip(zip::ZipBackend),
    #[cfg(feature = "cbr")]
    Rar(Box<rar::RarBackend>),
    #[cfg(feature = "cb7")]
    SevenZip(Box<sevenz::SevenZBackend>),
}

/// An open archive.
pub struct Archive {
    kind: ArchiveKind,
    backend: Backend,
    entries: Vec<Entry>,
    limits: Limits,
}

impl std::fmt::Debug for Archive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Archive")
            .field("kind", &self.kind)
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

impl Archive {
    /// Opens an archive of the given kind. Checks the file size, the entry count, every declared
    /// size and the overall ratio before anything is decompressed.
    pub fn open(
        mut source: Box<dyn Source>,
        kind: ArchiveKind,
        limits: &Limits,
    ) -> Result<Self> {
        let len = source.seek(SeekFrom::End(0))?;
        source.seek(SeekFrom::Start(0))?;
        if len > limits.max_file_bytes {
            return Err(BookError::TooLarge {
                what: "file size",
                limit: limits.max_file_bytes,
            });
        }
        let (backend, raw) = match kind {
            ArchiveKind::Zip => {
                let (b, raw) = zip::ZipBackend::open(source, limits)?;
                (Backend::Zip(b), raw)
            }
            #[cfg(feature = "cbr")]
            ArchiveKind::Rar => {
                let (b, raw) = rar::RarBackend::open(source, limits)?;
                (Backend::Rar(Box::new(b)), raw)
            }
            #[cfg(feature = "cb7")]
            ArchiveKind::SevenZip => {
                let (b, raw) = sevenz::SevenZBackend::open(source, limits)?;
                (Backend::SevenZip(Box::new(b)), raw)
            }
            #[allow(unreachable_patterns)]
            other => {
                return Err(BookError::Unsupported {
                    what: format!("{} archives in this build", other.format()),
                });
            }
        };
        let mut total: u64 = 0;
        let mut entries = Vec::with_capacity(raw.len());
        for e in raw {
            total = total.saturating_add(e.size);
            if let Some(name) =
                path::entry_name(&e.name, limits.max_name_bytes, limits.max_path_depth)
            {
                entries.push(Entry {
                    name,
                    size: e.size,
                    packed: e.packed,
                    encrypted: e.encrypted,
                    raw: e.raw,
                });
            }
        }
        if total > limits.max_total_bytes {
            return Err(BookError::TooLarge {
                what: "total size",
                limit: limits.max_total_bytes,
            });
        }
        limits.check_ratio(total, len)?;
        Ok(Self { kind, backend, entries, limits: limits.clone() })
    }

    pub fn kind(&self) -> ArchiveKind {
        self.kind
    }

    /// The files in archive order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Finds an entry by its normalised name: an exact match first, then one that differs only in
    /// case, since some books get the case of their own references wrong.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.name == name)
            .or_else(|| self.entries.iter().position(|e| e.name.eq_ignore_ascii_case(name)))
    }

    /// Whether reaching an entry means decoding the ones before it. Such archives are best read
    /// once in order with [`Archive::extract`].
    pub fn is_solid(&self) -> bool {
        match &self.backend {
            Backend::Zip(_) => false,
            #[cfg(feature = "cbr")]
            Backend::Rar(b) => b.is_solid(),
            #[cfg(feature = "cb7")]
            Backend::SevenZip(b) => b.is_solid(),
        }
    }

    /// Reads one entry, by its index in [`Archive::entries`]. In a solid archive this decodes
    /// every entry before it, so reading them all this way is quadratic; see [`Archive::extract`].
    pub fn read(&mut self, index: usize) -> Result<Vec<u8>> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| BookError::missing(format!("entry {index}")))?;
        if entry.encrypted {
            return Err(BookError::Protected);
        }
        self.limits.check_entry(entry.size, packed_for_ratio(self.kind, entry))?;
        let raw = entry.raw;
        let size = entry.size;
        let data = match &mut self.backend {
            Backend::Zip(b) => b.read(raw, size, &self.limits)?,
            #[cfg(feature = "cbr")]
            Backend::Rar(b) => b.read(raw, &self.limits)?,
            #[cfg(feature = "cb7")]
            Backend::SevenZip(b) => b.read(raw, size, &self.limits)?,
        };
        if data.len() as u64 > self.limits.max_entry_bytes {
            return Err(BookError::TooLarge {
                what: "entry size",
                limit: self.limits.max_entry_bytes,
            });
        }
        Ok(data)
    }

    /// Reads one entry by its normalised name.
    pub fn read_name(&mut self, name: &str) -> Result<Vec<u8>> {
        let index = self.find(name).ok_or_else(|| BookError::missing(name))?;
        self.read(index)
    }

    /// Reads the wanted entries in archive order, handing each to `each` as it lands, until
    /// `each` breaks. A solid archive is decoded once from the start; other archives read only
    /// what is wanted. `wanted` takes indices in [`Archive::entries`].
    pub fn extract(
        &mut self,
        wanted: &dyn Fn(usize) -> bool,
        each: &mut dyn FnMut(usize, Vec<u8>) -> ControlFlow<()>,
    ) -> Result<()> {
        let mut by_raw: Vec<(usize, usize)> = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            if wanted(i) {
                if e.encrypted {
                    return Err(BookError::Protected);
                }
                self.limits.check_entry(e.size, packed_for_ratio(self.kind, e))?;
                by_raw.push((e.raw, i));
            }
        }
        if by_raw.is_empty() {
            return Ok(());
        }
        by_raw.sort_unstable();
        if !self.is_solid() {
            for (_, i) in by_raw {
                let data = self.read(i)?;
                if each(i, data).is_break() {
                    break;
                }
            }
            return Ok(());
        }
        match &mut self.backend {
            Backend::Zip(_) => unreachable!("ZIP entries are never solid"),
            #[cfg(feature = "cbr")]
            Backend::Rar(b) => b.extract(&by_raw, &self.limits, each),
            #[cfg(feature = "cb7")]
            Backend::SevenZip(b) => b.extract(&by_raw, &self.limits, each),
        }
    }
}

/// The packed size the ratio check uses. A solid 7z block has no per-entry packed size, so the
/// archive-wide check at open stands for it.
fn packed_for_ratio(kind: ArchiveKind, entry: &Entry) -> u64 {
    match kind {
        ArchiveKind::SevenZip => entry.size,
        _ => entry.packed,
    }
}

/// Reads at most `cap` bytes, failing if there is more. `hint` is the declared size, trusted only
/// up to a modest first allocation.
pub(crate) fn read_capped(mut r: impl Read, hint: u64, cap: u64) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(hint.min(cap).min(16 * 1024 * 1024) as usize);
    (&mut r).take(cap.saturating_add(1)).read_to_end(&mut out)?;
    if out.len() as u64 > cap {
        return Err(BookError::TooLarge { what: "entry size", limit: cap });
    }
    Ok(out)
}
