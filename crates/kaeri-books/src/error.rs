use std::fmt;

/// The container a book came in, for errors and for callers that store the format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// A ZIP archive of images: CBZ, or a plain ZIP.
    Cbz,
    /// A RAR archive of images.
    Cbr,
    /// A 7z archive of images.
    Cb7,
    /// A folder of images on disk.
    Folder,
    Epub,
    Pdf,
}

impl Format {
    /// The lowercase name used in errors and stored rows: `cbz`, `cbr`, `cb7`, `folder`, `epub`,
    /// `pdf`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Cb7 => "cb7",
            Self::Folder => "folder",
            Self::Epub => "epub",
            Self::Pdf => "pdf",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What can go wrong opening or reading a book file.
///
/// The engine maps these onto the model's `ReadingFailure`: [`BookError::Unsupported`] to
/// `Unsupported`, [`BookError::Protected`] to `Protected`, an I/O `NotFound` to `FileMissing`,
/// and everything else to `Unreadable`. [`BookError::reading_failure`] names the case.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BookError {
    /// Not a format this crate reads, or one switched off at build time.
    #[error("not a book format Kaeri reads: {what}")]
    Unsupported { what: String },
    /// Encrypted or carrying DRM. Font obfuscation in an EPUB is not this; it is undone.
    #[error("the book is encrypted or carries DRM")]
    Protected,
    /// The container or its metadata is damaged, or a reader refused it.
    #[error("the {format} file is damaged: {reason}")]
    Malformed { format: Format, reason: String },
    /// Over one of the [`Limits`](crate::Limits).
    #[error("the book is over a limit: {what} (limit {limit})")]
    TooLarge { what: &'static str, limit: u64 },
    /// The book opened but holds nothing to read.
    #[error("the book has no pages")]
    Empty,
    /// A page, resource or entry that is not in the book.
    #[error("no {name} in the book")]
    Missing { name: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// The `ReadingFailure` case a [`BookError`] maps to, so the engine maps in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Unsupported,
    Protected,
    FileMissing,
    Unreadable,
    DiskFull,
}

impl BookError {
    pub(crate) fn malformed(format: Format, reason: impl fmt::Display) -> Self {
        Self::Malformed { format, reason: reason.to_string() }
    }

    pub(crate) fn missing(name: impl Into<String>) -> Self {
        Self::Missing { name: name.into() }
    }

    /// The model's `ReadingFailure` case for this error.
    pub fn reading_failure(&self) -> FailureKind {
        match self {
            Self::Unsupported { .. } => FailureKind::Unsupported,
            Self::Protected => FailureKind::Protected,
            Self::Io(e) if e.kind() == std::io::ErrorKind::NotFound => FailureKind::FileMissing,
            Self::Io(e) if e.kind() == std::io::ErrorKind::StorageFull => FailureKind::DiskFull,
            _ => FailureKind::Unreadable,
        }
    }
}

pub type Result<T, E = BookError> = std::result::Result<T, E>;
