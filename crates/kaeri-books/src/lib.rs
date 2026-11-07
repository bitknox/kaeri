//! Book files, opened as untrusted input: CBZ, CBR, CB7 and folders of images, EPUB 2 and 3,
//! and PDF.
//!
//! Every reader checks the [`Limits`] before it allocates or decompresses: file size, entry
//! count, declared and actual entry sizes, total size, compression ratio, decoder dictionary
//! size, XML size and node count, and rendered pixels. Entry names are normalised and any that
//! would leave the book are dropped ([`path`]). Nothing here touches the network, runs a
//! script or writes a file named by the book; the writers stream into whatever they are given.
//!
//! [`open`] sniffs the file's first bytes and opens it as the right [`Book`]. The readers can
//! also be used one by one: [`Archive`], [`Comic`], [`Epub`] and [`Pdf`].
//!
//! [`novel`] cleans EPUB and text chapters for the web view and carries the paginator.
//!
//! [`write`](mod@write) makes books for downloads and Send to e-reader (CBZ, EPUB, fixed-layout EPUB and
//! KEPUB), and [`opds`] makes the documents of the OPDS catalogue served to e-readers.
//!
//! Everything here is synchronous and blocking. Archives, EPUBs and comics finish in time
//! proportional to what they decode, which the limits cap. PDFs do not: a crafted PDF can keep
//! the parser busy for minutes, and the parser cannot be cancelled, so PDFs are opened and
//! rendered on a worker with a time budget. The [`pdf`] module says how.

pub mod archive;
pub mod comic;
pub mod comicinfo;
pub mod epub;
mod error;
pub mod library;
mod limits;
pub mod mime;
pub mod naming;
pub mod natural;
pub mod novel;
pub mod opds;
pub mod path;
#[cfg(feature = "pdf")]
pub mod pdf;
pub mod worker;
pub mod write;
mod xml;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub use archive::{Archive, ArchiveKind, Entry, Source};
pub use comic::{Comic, Page};
pub use comicinfo::ComicInfo;
pub use epub::Epub;
pub use error::{BookError, FailureKind, Format, Result};
pub use limits::Limits;
#[cfg(feature = "pdf")]
pub use pdf::Pdf;

/// An open book.
#[derive(Debug)]
pub enum Book {
    /// Pages of images: an archive or a folder.
    Comic(Box<Comic>),
    Epub(Box<Epub>),
    #[cfg(feature = "pdf")]
    Pdf(Pdf),
}

impl Book {
    pub fn format(&self) -> Format {
        match self {
            Self::Comic(c) => c.format(),
            Self::Epub(_) => Format::Epub,
            #[cfg(feature = "pdf")]
            Self::Pdf(_) => Format::Pdf,
        }
    }
}

/// What a file's first bytes say it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sniffed {
    Zip,
    Rar,
    SevenZip,
    Pdf,
}

/// How many leading bytes [`sniff`] looks at.
pub const SNIFF_BYTES: usize = 1024;

/// Recognises a container from its first bytes. The extension is never trusted over the
/// content: plenty of `.cbr` files are ZIPs.
pub fn sniff(head: &[u8]) -> Option<Sniffed> {
    if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        return Some(Sniffed::Zip);
    }
    if head.starts_with(b"Rar!\x1a\x07\x00") || head.starts_with(b"Rar!\x1a\x07\x01\x00") {
        return Some(Sniffed::Rar);
    }
    if head.starts_with(&[b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C]) {
        return Some(Sniffed::SevenZip);
    }
    // The PDF header may follow some junk; readers accept it within the first kilobyte.
    let window = &head[..head.len().min(SNIFF_BYTES)];
    if window.windows(5).any(|w| w == b"%PDF-") {
        return Some(Sniffed::Pdf);
    }
    None
}

/// Opens a book file, or a folder of images.
pub fn open(path: &Path, limits: &Limits) -> Result<Book> {
    let meta = path.metadata()?;
    if meta.is_dir() {
        return Comic::open_folder(path, limits).map(|c| Book::Comic(Box::new(c)));
    }
    if meta.len() > limits.max_file_bytes {
        return Err(BookError::TooLarge { what: "file size", limit: limits.max_file_bytes });
    }
    let name = path.file_name().and_then(|n| n.to_str());
    open_source(Box::new(File::open(path)?), name, limits)
}

/// Opens a book from any seekable source. `name` is the file name, used only when the content
/// is not recognised, to say what the file claimed to be.
pub fn open_source(
    mut source: Box<dyn Source>,
    name: Option<&str>,
    limits: &Limits,
) -> Result<Book> {
    let mut head = Vec::with_capacity(SNIFF_BYTES);
    (&mut source).take(SNIFF_BYTES as u64).read_to_end(&mut head)?;
    source.seek(SeekFrom::Start(0))?;
    // Content that matches no format: a damaged book when the name says it is one (an empty or
    // truncated download), otherwise a file Kaeri does not read.
    let unrecognised = || {
        let ext = name.and_then(path::extension);
        match ext.as_deref().and_then(claimed_format) {
            Some(format) if head.is_empty() => {
                BookError::malformed(format, "the file is empty")
            }
            Some(format) => BookError::malformed(format, "the content is not that format"),
            None => BookError::Unsupported {
                what: ext.map_or_else(
                    || "a file of unknown type".to_owned(),
                    |e| format!(".{e} files"),
                ),
            },
        }
    };
    match sniff(&head).ok_or_else(unrecognised)? {
        Sniffed::Zip => {
            let mut archive = Archive::open(source, ArchiveKind::Zip, limits)?;
            if epub::looks_like_epub(&mut archive) {
                Epub::from_archive(archive, limits).map(|e| Book::Epub(Box::new(e)))
            } else {
                Comic::from_archive(archive, limits).map(|c| Book::Comic(Box::new(c)))
            }
        }
        Sniffed::Rar => {
            Comic::from_archive(Archive::open(source, ArchiveKind::Rar, limits)?, limits)
                .map(|c| Book::Comic(Box::new(c)))
        }
        Sniffed::SevenZip => {
            Comic::from_archive(Archive::open(source, ArchiveKind::SevenZip, limits)?, limits)
                .map(|c| Book::Comic(Box::new(c)))
        }
        #[cfg(feature = "pdf")]
        Sniffed::Pdf => {
            let len = source.seek(SeekFrom::End(0))?;
            source.seek(SeekFrom::Start(0))?;
            if len > limits.max_pdf_bytes {
                return Err(BookError::TooLarge {
                    what: "PDF size",
                    limit: limits.max_pdf_bytes,
                });
            }
            let mut bytes = Vec::with_capacity(len as usize);
            source.take(limits.max_pdf_bytes).read_to_end(&mut bytes)?;
            Pdf::open(bytes, limits).map(Book::Pdf)
        }
        #[cfg(not(feature = "pdf"))]
        Sniffed::Pdf => Err(BookError::Unsupported { what: "PDF in this build".to_owned() }),
    }
}

/// The format a file's extension claims.
fn claimed_format(ext: &str) -> Option<Format> {
    Some(match ext {
        "cbz" | "zip" => Format::Cbz,
        "cbr" | "rar" => Format::Cbr,
        "cb7" | "7z" => Format::Cb7,
        "epub" | "kepub" => Format::Epub,
        "pdf" => Format::Pdf,
        _ => return None,
    })
}

/// Opens a book held in memory.
pub fn open_bytes(bytes: Vec<u8>, name: Option<&str>, limits: &Limits) -> Result<Book> {
    open_source(Box::new(std::io::Cursor::new(bytes)), name, limits)
}
