//! KEPUB from an EPUB that is already open: the same files, with every reflowable chapter in
//! the spine written again with Kobo's sentence spans and without anything active.

use std::collections::HashSet;
use std::io::{Seek, Write};

use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

use super::xhtml::Writer;
use super::zip_err;
use crate::epub::{Epub, Layout};
use crate::{Format, Result, xml};

/// Writes `epub` as a KEPUB into `out`. Files are copied as they are, the `mimetype` first;
/// reflowable XHTML in the spine gains sentence spans. A chapter that does not parse, or one
/// that already has spans, is copied unchanged, so a book Kobo reads today still reads.
pub fn kepub_from_epub<W: Write + Seek>(epub: &mut Epub, out: W) -> Result<W> {
    let spine: Vec<(usize, String, bool)> = epub
        .spine()
        .enumerate()
        .map(|(i, (_, item))| {
            (i, item.path.clone(), item.media_type == "application/xhtml+xml")
        })
        .collect();
    let chapters: HashSet<String> = spine
        .into_iter()
        .filter(|(i, _, xhtml)| *xhtml && epub.layout_of(*i) == Layout::Reflowable)
        .map(|(_, p, _)| p)
        .collect();
    let limits = epub.limits().clone();
    let archive = epub.archive_mut();
    let names: Vec<String> = archive.entries().iter().map(|e| e.name.clone()).collect();

    let mut zip = ZipWriter::new(out);
    zip.start_file("mimetype", stored()).map_err(zip_err)?;
    zip.write_all(b"application/epub+zip")?;
    for (i, name) in names.iter().enumerate() {
        if name == "mimetype" {
            continue;
        }
        let data = archive.read(i)?;
        let data = if chapters.contains(name) {
            with_spans(&data, &limits).map_or(data, String::into_bytes)
        } else {
            data
        };
        let opts = if super::epub::data_is_compressed(crate::mime::guess(name)) {
            stored()
        } else {
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
        }
        .large_file(data.len() as u64 >= u64::from(u32::MAX));
        zip.start_file(name.as_str(), opts).map_err(zip_err)?;
        zip.write_all(&data)?;
    }
    zip.finish().map_err(zip_err)
}

fn with_spans(data: &[u8], limits: &crate::Limits) -> Option<String> {
    let text = xml::text(data, Format::Epub, limits).ok()?;
    if text.contains("koboSpan") {
        return None;
    }
    let text = crate::epub::html_entities(&text);
    let doc = xml::parse(&text, Format::Epub, limits).ok()?;
    Some(Writer::new(true).document(&doc))
}

fn stored() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
}
