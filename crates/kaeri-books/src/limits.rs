/// Bounds on what a book file may make this crate do. Every book is untrusted input, so each
/// reader checks these before it allocates or decompresses.
///
/// The defaults fit real volumes (a few hundred pages of up to a few tens of MB each, omnibus
/// archives of a few GB) with room to spare, and keep a hostile file from exhausting memory or
/// disk on a 2 GB e-reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// The largest book file opened at all.
    pub max_file_bytes: u64,
    /// The most entries an archive may list, directories included.
    pub max_entries: usize,
    /// The largest single entry (a page, a chapter, a font) once decompressed.
    pub max_entry_bytes: u64,
    /// The most an archive may declare across all its entries once decompressed, and the most
    /// one extraction pass writes out.
    pub max_total_bytes: u64,
    /// The highest decompressed to compressed ratio an entry may have once it is over 1 MiB.
    /// Deflate tops out near 1032; LZMA and RAR can go far higher on crafted input.
    pub max_ratio: u64,
    /// The longest entry name in bytes.
    pub max_name_bytes: usize,
    /// The most path components an entry name may have.
    pub max_path_depth: usize,
    /// The largest XML document parsed: container, OPF, navigation, ComicInfo, and XHTML looked
    /// into for image pages.
    pub max_xml_bytes: u64,
    /// The most XML nodes one document may have.
    pub max_xml_nodes: u32,
    /// The deepest element nesting in an XML document. The parser recurses once per level, so
    /// this keeps a crafted file from overflowing the stack of a reading thread.
    pub max_xml_depth: usize,
    /// The deepest table of contents kept, and the most entries in it.
    pub max_toc_depth: usize,
    pub max_toc_entries: usize,
    /// The largest decoder dictionary or model an archive may ask for (LZMA, LZMA2, RAR 5 and
    /// later, RAR's PPMd), so a header cannot make the reader allocate gigabytes.
    pub max_decoder_bytes: u64,
    /// The largest PDF read into memory.
    pub max_pdf_bytes: u64,
    /// The most pages a PDF may have.
    pub max_pdf_pages: usize,
    /// The most pixels one rendered PDF page may have.
    pub max_render_pixels: u64,
}

impl Default for Limits {
    fn default() -> Self {
        const MIB: u64 = 1024 * 1024;
        Self {
            max_file_bytes: 8 * 1024 * MIB,
            max_entries: 20_000,
            max_entry_bytes: 256 * MIB,
            max_total_bytes: 8 * 1024 * MIB,
            max_ratio: 1100,
            max_name_bytes: 1024,
            max_path_depth: 32,
            max_xml_bytes: 16 * MIB,
            max_xml_nodes: 1_000_000,
            max_xml_depth: 128,
            max_toc_depth: 16,
            max_toc_entries: 20_000,
            max_decoder_bytes: 512 * MIB,
            max_pdf_bytes: 1024 * MIB,
            max_pdf_pages: 20_000,
            max_render_pixels: 64 * 1024 * 1024,
        }
    }
}

impl Limits {
    /// Checks an entry's declared sizes against the per-entry limits.
    pub(crate) fn check_entry(&self, size: u64, packed: u64) -> crate::Result<()> {
        if size > self.max_entry_bytes {
            return Err(crate::BookError::TooLarge {
                what: "entry size",
                limit: self.max_entry_bytes,
            });
        }
        self.check_ratio(size, packed)
    }

    /// Checks a decompressed size against its compressed size, past the 1 MiB floor where a
    /// high ratio is normal for tiny text files.
    pub(crate) fn check_ratio(&self, size: u64, packed: u64) -> crate::Result<()> {
        if size > 1024 * 1024 && size / packed.max(1) > self.max_ratio {
            return Err(crate::BookError::TooLarge {
                what: "compression ratio",
                limit: self.max_ratio,
            });
        }
        Ok(())
    }
}
