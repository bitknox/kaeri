//! CB7 through `sevenz-rust2`, built with LZMA, LZMA2, BCJ and copy only: no encryption, no
//! other codecs. Dictionary sizes are checked against the limits before any block is decoded.

use std::ops::ControlFlow;

use sevenz_rust2::{ArchiveEntry, ArchiveReader, EncoderMethod, Error, Password};

use super::{RawEntry, Source, read_capped};
use crate::{BookError, Format, Limits, Result};

pub(crate) struct SevenZBackend {
    reader: ArchiveReader<Box<dyn Source>>,
}

impl SevenZBackend {
    pub(crate) fn open(
        source: Box<dyn Source>,
        limits: &Limits,
    ) -> Result<(Self, Vec<RawEntry>)> {
        let mut reader = guard(|| ArchiveReader::new(source, Password::empty()))?;
        // A library does not spread one book over every core; the prefetch worker is the thread.
        reader.set_thread_count(1);
        let archive = reader.archive();
        if archive.files.len() > limits.max_entries {
            return Err(BookError::TooLarge {
                what: "entry count",
                limit: limits.max_entries as u64,
            });
        }
        for block in &archive.blocks {
            for coder in &block.coders {
                let id = coder.encoder_method_id();
                if id == EncoderMethod::ID_AES256_SHA256 {
                    return Err(BookError::Protected);
                }
                if let Some(dict) = dictionary_size(id, coder.properties())
                    && dict > limits.max_decoder_bytes
                {
                    return Err(BookError::TooLarge {
                        what: "dictionary size",
                        limit: limits.max_decoder_bytes,
                    });
                }
            }
        }
        let mut raw = Vec::new();
        for (i, f) in archive.files.iter().enumerate() {
            if f.is_directory || f.is_anti_item || !f.has_stream {
                continue;
            }
            // In a solid block every entry before a wanted one is decoded into memory, so one
            // oversized entry anywhere makes the archive unreadable within the limits.
            if archive.is_solid && f.size > limits.max_entry_bytes {
                return Err(BookError::TooLarge {
                    what: "entry size",
                    limit: limits.max_entry_bytes,
                });
            }
            raw.push(RawEntry {
                name: f.name.clone(),
                size: f.size,
                packed: f.compressed_size,
                encrypted: false,
                raw: i,
            });
        }
        Ok((Self { reader }, raw))
    }

    pub(crate) fn is_solid(&self) -> bool {
        self.reader.archive().is_solid
    }

    pub(crate) fn read(&mut self, raw: usize, size: u64, limits: &Limits) -> Result<Vec<u8>> {
        let name = self
            .reader
            .archive()
            .files
            .get(raw)
            .map(|f| f.name.clone())
            .ok_or_else(|| BookError::missing(format!("entry {raw}")))?;
        // `read_file` bounds its output to the declared size; the declared size was checked.
        let data = guard(|| self.reader.read_file(&name))?;
        if data.len() as u64 > size.min(limits.max_entry_bytes) {
            return Err(BookError::TooLarge {
                what: "entry size",
                limit: limits.max_entry_bytes,
            });
        }
        Ok(data)
    }

    pub(crate) fn extract(
        &mut self,
        wanted: &[(usize, usize)],
        limits: &Limits,
        each: &mut dyn FnMut(usize, Vec<u8>) -> ControlFlow<()>,
    ) -> Result<()> {
        // Entries are matched by address: the callback hands out references into the archive
        // the reader holds, which does not change while it runs.
        let files = &self.reader.archive().files;
        let targets: Vec<(*const ArchiveEntry, usize)> = wanted
            .iter()
            .filter_map(|&(raw, index)| {
                files.get(raw).map(|f| (f as *const ArchiveEntry, index))
            })
            .collect();
        let mut left = targets.len();
        let mut total: u64 = 0;
        let mut failure: Option<BookError> = None;
        let result = guard(|| {
            self.reader.for_each_entries(|entry, reader| {
                let Some(&(_, index)) = targets.iter().find(|(p, _)| std::ptr::eq(*p, entry))
                else {
                    return Ok(true);
                };
                let cap = entry.size.min(limits.max_entry_bytes);
                let data = match read_capped(reader, entry.size, cap) {
                    Ok(d) => d,
                    Err(e) => {
                        failure = Some(e);
                        return Ok(false);
                    }
                };
                total = total.saturating_add(data.len() as u64);
                if total > limits.max_total_bytes {
                    failure = Some(BookError::TooLarge {
                        what: "total size",
                        limit: limits.max_total_bytes,
                    });
                    return Ok(false);
                }
                left -= 1;
                Ok(each(index, data).is_continue() && left > 0)
            })
        });
        if let Some(e) = failure {
            return Err(e);
        }
        result
    }
}

/// The dictionary an LZMA or LZMA2 coder asks for, from its properties.
fn dictionary_size(id: &[u8], props: &[u8]) -> Option<u64> {
    if id == EncoderMethod::ID_LZMA {
        let bytes: [u8; 4] = props.get(1..5)?.try_into().ok()?;
        return Some(u64::from(u32::from_le_bytes(bytes)));
    }
    if id == EncoderMethod::ID_LZMA2 {
        let bits = u64::from(*props.first()? & 0x3F);
        if bits > 40 {
            return Some(u64::MAX);
        }
        if bits == 40 {
            return Some(u64::from(u32::MAX));
        }
        return Some((2 | (bits & 1)) << (bits / 2 + 11));
    }
    None
}

fn guard<T>(f: impl FnOnce() -> Result<T, Error>) -> Result<T> {
    #[cfg(not(feature = "fuzzing"))]
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .map_err(|_| BookError::malformed(Format::Cb7, "the 7z decoder failed"))?;
    #[cfg(feature = "fuzzing")]
    let r = f();
    r.map_err(map_err)
}

fn map_err(e: Error) -> BookError {
    match e {
        Error::PasswordRequired | Error::MaybeBadPassword(_) => BookError::Protected,
        Error::UnsupportedCompressionMethod(m) if m.to_ascii_uppercase().contains("AES") => {
            BookError::Protected
        }
        Error::UnsupportedCompressionMethod(m) => {
            BookError::Unsupported { what: format!("7z compression {m}") }
        }
        Error::MaxMemLimited { .. } => BookError::TooLarge { what: "decoder memory", limit: 0 },
        Error::Io(io, _)
            if !matches!(
                io.kind(),
                std::io::ErrorKind::InvalidData | std::io::ErrorKind::UnexpectedEof
            ) =>
        {
            BookError::Io(io)
        }
        other => BookError::malformed(Format::Cb7, other),
    }
}
