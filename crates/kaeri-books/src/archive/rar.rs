//! CBR through the `rars` crate's read side (spike S2, decided 2026-11-05). Every call passes the
//! crate's own resource ceilings: header count and bytes when parsing, and per-member output,
//! total output, dictionary and decoder workspace when decoding.

use std::io::Write;
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use rars::{ArchiveReadOptions, ArchiveReader, ErrorKind, ExtractionDecision};

use super::{RawEntry, Source};
use crate::{BookError, Format, Limits, Result};

pub(crate) struct RarBackend {
    archive: rars::Archive,
    solid: bool,
}

/// Header bytes allowed per listed entry, on top of the main header. A RAR 5 file header with a
/// long name and every extra record stays well under this.
const HEADER_BYTES_PER_ENTRY: u64 = 4096;

fn parse_options(limits: &Limits) -> ArchiveReadOptions<'static> {
    let count = limits.max_entries as u64 + 16;
    ArchiveReadOptions::new()
        .with_max_header_count(count)
        .with_max_header_bytes(count.saturating_mul(HEADER_BYTES_PER_ENTRY))
}

fn read_options(limits: &Limits, member_cap: u64) -> ArchiveReadOptions<'static> {
    ArchiveReadOptions::new()
        .with_max_member_output_bytes(member_cap)
        .with_max_total_output_bytes(limits.max_total_bytes)
        .with_rar50_dictionary_size_limit(limits.max_decoder_bytes)
        .with_max_reader_workspace_bytes(limits.max_decoder_bytes.saturating_mul(2))
}

impl RarBackend {
    pub(crate) fn open(
        source: Box<dyn Source>,
        limits: &Limits,
    ) -> Result<(Self, Vec<RawEntry>)> {
        let archive = ArchiveReader::read_reader_with_options(source, parse_options(limits))
            .map_err(map_err)?;
        let mut raw = Vec::new();
        let mut solid = false;
        for (i, m) in archive.members().enumerate() {
            if i >= limits.max_entries {
                return Err(BookError::TooLarge {
                    what: "entry count",
                    limit: limits.max_entries as u64,
                });
            }
            solid |= m.is_solid();
            let meta = &m.meta;
            // Members split across volumes need the other volumes, which a single file does not
            // have; directories and links are not files.
            if meta.is_directory
                || meta.is_redirection
                || meta.is_split_before
                || meta.is_split_after
            {
                continue;
            }
            let name = match m.decoded_name(None) {
                Ok(n) => String::from_utf8_lossy(&n).into_owned(),
                Err(_) => meta.name_lossy(),
            };
            raw.push(RawEntry {
                name,
                size: meta.unpacked_size,
                packed: meta.packed_size,
                encrypted: meta.is_encrypted,
                raw: i,
            });
        }
        Ok((Self { archive, solid }, raw))
    }

    pub(crate) fn is_solid(&self) -> bool {
        self.solid
    }

    pub(crate) fn read(&mut self, raw: usize, limits: &Limits) -> Result<Vec<u8>> {
        guard(|| {
            self.archive
                .read_member_at_with_options(raw, read_options(limits, limits.max_entry_bytes))
        })?
        .ok_or_else(|| BookError::missing(format!("entry {raw}")))
    }

    /// One pass through the archive. Members before the last wanted one are decoded (a solid
    /// archive needs them for history) and the wanted ones are handed out as each completes.
    pub(crate) fn extract(
        &mut self,
        wanted: &[(usize, usize)],
        limits: &Limits,
        each: &mut dyn FnMut(usize, Vec<u8>) -> ControlFlow<()>,
    ) -> Result<()> {
        let Some(&(last, _)) = wanted.last() else {
            return Ok(());
        };
        let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let mut pending: Option<usize> = None;
        let mut stopped = false;
        let mut member = 0usize;
        let solid = self.solid;
        // `pending` names the wanted entry whose bytes the buffer holds; it is handed out when
        // the next member starts, after `rars` has checked its CRC, or at the end.
        let mut flush = |pending: &mut Option<usize>, stopped: &mut bool| {
            if let Some(index) = pending.take() {
                let data = std::mem::take(&mut *lock(&buffer));
                if each(index, data).is_break() {
                    *stopped = true;
                }
            }
        };
        let options = read_options(limits, limits.max_entry_bytes);
        let result = guard(|| {
            self.archive.extract_with_control(options, |m| {
                let this = member;
                member += 1;
                flush(&mut pending, &mut stopped);
                if stopped || this > last {
                    return Ok(ExtractionDecision::Stop);
                }
                if let Ok(at) = wanted.binary_search_by_key(&this, |&(raw, _)| raw) {
                    pending = Some(wanted[at].1);
                    return Ok(ExtractionDecision::Extract(Box::new(Shared(Arc::clone(
                        &buffer,
                    )))));
                }
                let file = !m.meta.is_directory && !m.meta.is_redirection;
                Ok(if solid && file {
                    ExtractionDecision::Extract(Box::new(std::io::sink()))
                } else {
                    ExtractionDecision::Skip
                })
            })
        });
        result?;
        flush(&mut pending, &mut stopped);
        Ok(())
    }
}

/// A writer into a buffer the extraction loop keeps, since `rars` takes its writers as boxes.
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        lock(&self.0).extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn lock(m: &Mutex<Vec<u8>>) -> std::sync::MutexGuard<'_, Vec<u8>> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Runs a `rars` call and maps its error. Outside fuzzing builds a panic inside the decoder is
/// caught and reported as a damaged file, since the crate is young; fuzzing lets it through so it
/// is found.
fn guard<T>(f: impl FnOnce() -> rars::Result<T>) -> Result<T> {
    #[cfg(not(feature = "fuzzing"))]
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .map_err(|_| BookError::malformed(Format::Cbr, "the RAR decoder failed"))?;
    #[cfg(feature = "fuzzing")]
    let r = f();
    r.map_err(map_err)
}

fn map_err(e: rars::Error) -> BookError {
    match e.kind() {
        ErrorKind::PasswordRequired | ErrorKind::BadPassword => BookError::Protected,
        ErrorKind::ResourceLimit => {
            BookError::TooLarge { what: "RAR decoder resources", limit: 0 }
        }
        ErrorKind::UnsupportedFormat | ErrorKind::UnsupportedFeature => {
            BookError::Unsupported { what: format!("RAR: {e}") }
        }
        _ => BookError::malformed(Format::Cbr, e),
    }
}
