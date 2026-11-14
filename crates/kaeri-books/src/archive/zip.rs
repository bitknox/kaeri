use std::borrow::Cow;

use zip::ZipArchive;
use zip::result::ZipError;

use super::{RawEntry, Source, read_capped};
use crate::{BookError, Format, Limits, Result};

pub(crate) struct ZipBackend {
    archive: ZipArchive<Box<dyn Source>>,
}

impl ZipBackend {
    pub(crate) fn open(
        source: Box<dyn Source>,
        limits: &Limits,
    ) -> Result<(Self, Vec<RawEntry>)> {
        let archive = ZipArchive::new(source).map_err(map_err)?;
        if archive.len() > limits.max_entries {
            return Err(BookError::TooLarge {
                what: "entry count",
                limit: limits.max_entries as u64,
            });
        }
        let meta = archive.metadata();
        let mut raw = Vec::with_capacity(archive.len());
        for i in 0..archive.len() {
            let e = meta.entry(i).map_err(map_err)?;
            if e.is_dir() || e.is_symlink() {
                continue;
            }
            let name = match e.name() {
                Ok(n) => n.into_owned(),
                Err(_) => match String::from_utf8_lossy(e.name_raw()) {
                    Cow::Borrowed(s) => s.to_owned(),
                    Cow::Owned(s) => s,
                },
            };
            raw.push(RawEntry {
                name,
                size: e.size(),
                packed: e.compressed_size(),
                encrypted: e.encrypted(),
                raw: i,
            });
        }
        Ok((Self { archive }, raw))
    }

    pub(crate) fn read(&mut self, raw: usize, size: u64, limits: &Limits) -> Result<Vec<u8>> {
        // The ZIP reader stops at the declared size and checks the CRC at the end, so the cap is
        // a second fence, not the first.
        let file = self.archive.by_index(raw).map_err(map_err)?;
        read_capped(file, size, limits.max_entry_bytes.min(size)).map_err(|e| match e {
            BookError::Io(io) => map_err(ZipError::Io(io)),
            other => other,
        })
    }
}

fn map_err(e: ZipError) -> BookError {
    match e {
        ZipError::UnsupportedArchive(m) if m.to_ascii_lowercase().contains("password") => {
            BookError::Protected
        }
        ZipError::InvalidPassword => BookError::Protected,
        ZipError::Io(io)
            if matches!(
                io.kind(),
                std::io::ErrorKind::InvalidData | std::io::ErrorKind::UnexpectedEof
            ) =>
        {
            BookError::malformed(Format::Cbz, io)
        }
        ZipError::Io(io) => BookError::Io(io),
        other => BookError::malformed(Format::Cbz, other),
    }
}
