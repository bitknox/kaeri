//! Writes a starting corpus for every target from the test suite's fixtures:
//! `cargo run --release --bin seeds -- <dir>` makes `<dir>/zip`, `<dir>/rar` and so on.

#[path = "../tests/common/mod.rs"]
mod common;

use std::io::{Cursor, Read};
use std::path::Path;

fn write(dir: &Path, target: &str, name: &str, data: &[u8]) {
    let d = dir.join(target);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(name), data).unwrap();
}

/// The parts of an EPUB fixture in the epub target's input layout.
fn epub_parts(epub: &[u8]) -> Vec<u8> {
    let mut z = zip::ZipArchive::new(Cursor::new(epub)).unwrap();
    let mut find = |suffix: &str| {
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            if f.name().is_ok_and(|n| n.ends_with(suffix)) {
                let mut v = Vec::new();
                f.read_to_end(&mut v).unwrap();
                return v;
            }
        }
        Vec::new()
    };
    let parts = [
        find("container.xml"),
        find(".opf"),
        find("nav.xhtml"),
        find(".ncx"),
        find("encryption.xml"),
        find(".xhtml"),
    ];
    parts.join(&0xFF)
}

fn main() {
    let dir = std::env::args().nth(1).expect("an output folder");
    let dir = Path::new(&dir);
    let pages = common::page_set(4);
    let mut with_info = pages.clone();
    with_info.push(("ComicInfo.xml".into(), common::COMICINFO.as_bytes().to_vec()));

    write(dir, "zip", "cbz", &common::cbz(4, true));
    write(dir, "zip", "epub", &common::epub_novel());
    write(dir, "zip", "manga", &common::epub_manga(3));
    write(dir, "rar", "rar5", &common::rar5(&with_info));
    write(dir, "rar", "rar4", &common::rar4(&with_info));
    write(dir, "sevenz", "solid", &common::sevenz(&with_info, true));
    write(dir, "sevenz", "blocks", &common::sevenz(&pages, false));
    write(dir, "epub", "novel", &epub_parts(&common::epub_novel()));
    write(dir, "epub", "epub2", &epub_parts(&common::epub2()));
    write(dir, "epub", "manga", &epub_parts(&common::epub_manga(3)));
    let jpegs: Vec<Vec<u8>> = (0..2).map(|i| common::jpeg(16, 16, i)).collect();
    write(dir, "pdf", "scan", &common::pdf_of_jpegs(&jpegs, 16, 16));
    write(dir, "pdf", "vector", &common::pdf_vector());
}
