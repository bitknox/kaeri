//! CBZ: page images stored as they are (they are compressed already), named so that every
//! reader sorts them the same way, with ComicInfo.xml when there is metadata.

use std::io::{Seek, Write};

use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

use super::{image_kind, zip_err};
use crate::{ComicInfo, Result};

/// Writes a CBZ page by page.
pub struct CbzWriter<W: Write + Seek> {
    zip: ZipWriter<W>,
    pages: u32,
}

impl<W: Write + Seek> CbzWriter<W> {
    /// Starts a CBZ, writing ComicInfo.xml first when given.
    pub fn new(out: W, info: Option<&ComicInfo>) -> Result<Self> {
        let mut zip = ZipWriter::new(out);
        if let Some(info) = info {
            let opts =
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("ComicInfo.xml", opts).map_err(zip_err)?;
            zip.write_all(info.to_xml().as_bytes())?;
        }
        Ok(Self { zip, pages: 0 })
    }

    /// Adds the next page. Its type comes from its bytes, and anything that is not an image is
    /// refused. Returns the name it was stored under.
    pub fn add_page(&mut self, data: &[u8]) -> Result<String> {
        let (ext, _) = image_kind(data)?;
        self.pages += 1;
        // Zero-padded to five digits so a plain string sort agrees with the reading order.
        let name = format!("{:05}.{ext}", self.pages);
        let opts = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .large_file(data.len() as u64 >= u64::from(u32::MAX));
        self.zip.start_file(name.as_str(), opts).map_err(zip_err)?;
        self.zip.write_all(data)?;
        Ok(name)
    }

    /// Writes the central directory and hands back the output.
    pub fn finish(self) -> Result<W> {
        self.zip.finish().map_err(zip_err)
    }
}
