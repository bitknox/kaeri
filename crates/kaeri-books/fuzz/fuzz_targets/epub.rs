#![no_main]

//! The input is split on a 0xFF byte into the container, package document, navigation
//! document, NCX, encryption file and one XHTML page, which are stored in a fresh ZIP so the
//! fuzzer works on the XML rather than on ZIP framing (the zip target covers that). The package
//! names the other files by fixed paths, or does not, as the fuzzer pleases.

mod common;

use std::io::{Cursor, Write};

use kaeri_books::{Book, open_bytes};
use libfuzzer_sys::fuzz_target;
use zip::write::{SimpleFileOptions, ZipWriter};

const NAMES: [&str; 6] = [
    "META-INF/container.xml",
    "OEBPS/content.opf",
    "OEBPS/nav.xhtml",
    "OEBPS/toc.ncx",
    "META-INF/encryption.xml",
    "OEBPS/p1.xhtml",
];

fuzz_target!(|data: &[u8]| {
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let _ = z.start_file("mimetype", opts);
    let _ = z.write_all(b"application/epub+zip");
    for (name, part) in NAMES.iter().zip(data.split(|&b| b == 0xFF)) {
        if z.start_file(*name, opts).is_err() || z.write_all(part).is_err() {
            return;
        }
    }
    let _ = z.start_file("OEBPS/i1.jpg", opts);
    let _ = z.write_all(&[0xFF, 0xD8, 0xFF, 0xD9]);
    let _ = z.start_file("OEBPS/font.otf", opts);
    let _ = z.write_all(&[7; 2048]);
    let Ok(bytes) = z.finish().map(Cursor::into_inner) else {
        return;
    };
    let Ok(Book::Epub(mut e)) = open_bytes(bytes, None, &common::limits()) else {
        return;
    };
    let _ = (e.metadata(), e.toc(), e.page_list(), e.cover(), e.is_fixed_layout());
    let paths: Vec<String> = e.package().manifest.iter().map(|m| m.path.clone()).collect();
    for i in 0..e.spine().count() {
        let _ = e.layout_of(i);
    }
    for p in paths.iter().take(32) {
        let _ = e.resource(p);
        let _ = e.resolve(p, "../x/./y.css#z");
    }
    let _ = e.image_pages();
});
