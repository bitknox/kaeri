//! What a folder scan reports beside its series: every file it met, the books Kaeri can't open by
//! format, and the books locked with DRM.

mod common;

use std::fs;

use common::{cbz, container, zip};
use kaeri_books::{Limits, library};

fn locked_epub() -> Vec<u8> {
    let opf = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="u">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="u">x</dc:identifier><dc:title>Locked Garden</dc:title></metadata>
  <manifest><item id="a" href="a.xhtml" media-type="application/xhtml+xml"/></manifest>
  <spine><itemref idref="a"/></spine>
</package>"#;
    zip(
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("META-INF/container.xml", container("content.opf")),
            ("content.opf", opf.as_bytes().to_vec()),
            ("a.xhtml", b"<html/>".to_vec()),
            ("META-INF/rights.xml", b"<rights/>".to_vec()),
        ],
        false,
    )
}

#[test]
fn a_scan_counts_what_it_cannot_open_by_format() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("Lantern Keeper")).unwrap();
    fs::write(root.join("Lantern Keeper/Lantern Keeper v01.cbz"), cbz(3, false)).unwrap();
    fs::write(root.join("Lantern Keeper/Lantern Keeper v02.cbz"), cbz(2, false)).unwrap();
    fs::write(root.join("Paper Harbour.mobi"), b"BOOKMOBI").unwrap();
    fs::write(root.join("Tin Kites.azw3"), b"azw3").unwrap();
    fs::write(root.join("Tin Kites 2.AZW3"), b"azw3").unwrap();
    fs::write(root.join("Locked Garden.epub"), locked_epub()).unwrap();
    fs::write(root.join("notes.txt"), b"not a book").unwrap();
    fs::write(root.join(".hidden.mobi"), b"hidden").unwrap();

    let scan = library::scan(root, &library::Nothing, &Limits::default(), &mut |_| {}).unwrap();

    assert_eq!(scan.seen, 7, "hidden files are not counted");
    assert_eq!(scan.unsupported, [("AZW3".to_owned(), 2), ("MOBI".to_owned(), 1)]);
    assert_eq!(scan.locked, 1);
    assert_eq!(scan.ignored, 1, "only the text file is a stray");
    assert_eq!(scan.unsupported_files.len(), 4);
    assert!(scan.unsupported_files.iter().any(|p| p.ends_with("Locked Garden.epub")));
    let locked = scan.series.iter().flat_map(|s| &s.files).find(|f| f.locked).unwrap();
    assert!(locked.broken.is_some());
    let keeper = scan.series.iter().find(|s| s.name == "Lantern Keeper").unwrap();
    assert_eq!(keeper.files.len(), 2);
}

#[test]
fn an_empty_folder_reports_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let scan =
        library::scan(dir.path(), &library::Nothing, &Limits::default(), &mut |_| {}).unwrap();
    assert_eq!((scan.seen, scan.series.len(), scan.locked), (0, 0, 0));
    assert!(scan.unsupported.is_empty());
}
