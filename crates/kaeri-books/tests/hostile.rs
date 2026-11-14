//! Hostile files: names that leave the book, bombs, oversized entries, encryption, links, and
//! XML built to exhaust a parser.

// The fixtures cover every archive format, so the file needs every reader.
#![cfg(all(feature = "cbr", feature = "cb7"))]

mod common;

use common::*;
use kaeri_books::archive::{Archive, ArchiveKind};
use kaeri_books::{Book, BookError, FailureKind, Limits, open_bytes};

fn pages_of(bytes: Vec<u8>) -> Vec<String> {
    match open_bytes(bytes, None, &Limits::default()).unwrap() {
        Book::Comic(c) => c.pages().iter().map(|p| p.name.clone()).collect(),
        other => panic!("expected a comic, got {:?}", other.format()),
    }
}

fn hostile_names() -> Vec<(String, Vec<u8>)> {
    [
        "../escape.jpg",
        "pages/../../escape2.jpg",
        "/abs/escape3.jpg",
        "C:/drive/escape4.jpg",
        "c:escape5.jpg",
        "..\\escape6.jpg",
        "pages/\u{1}ctl.jpg",
        "pages/ok.jpg",
    ]
    .iter()
    .enumerate()
    .map(|(i, n)| (n.to_string(), jpeg(4, 4, i as u32)))
    .collect()
}

#[test]
fn names_leaving_the_book_are_dropped_in_every_archive() {
    let entries = hostile_names();
    let refs: Vec<(&str, Vec<u8>)> =
        entries.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    for (label, bytes) in [
        ("zip", zip(&refs, true)),
        ("rar5", rar5(&entries)),
        ("rar4", rar4(&entries)),
        ("7z", sevenz(&entries, false)),
    ] {
        assert_eq!(pages_of(bytes), ["pages/ok.jpg"], "{label}");
    }
}

#[test]
fn backslashes_become_folders() {
    let entries = vec![("pages\\p1.jpg".to_owned(), jpeg(4, 4, 1))];
    assert_eq!(pages_of(rar4(&entries)), ["pages/p1.jpg"]);
}

#[test]
fn zip_bomb_is_refused_by_ratio() {
    let zeros = vec![0u8; 4 * 1024 * 1024];
    let bytes = zip(&[("p1.jpg", jpeg(4, 4, 1)), ("p2.jpg", zeros)], false);
    let limits = Limits { max_ratio: 50, ..Limits::default() };
    let err = open_bytes(bytes, None, &limits).unwrap_err();
    assert!(matches!(err, BookError::TooLarge { what: "compression ratio", .. }), "{err:?}");
}

#[test]
fn oversized_entry_is_refused_on_read() {
    let big = jpeg(256, 256, 3);
    let limits = Limits { max_entry_bytes: (big.len() - 1) as u64, ..Limits::default() };
    let entries = vec![("p1.jpg".to_owned(), jpeg(4, 4, 1)), ("p2.jpg".to_owned(), big)];
    let refs: Vec<(&str, Vec<u8>)> =
        entries.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    for (label, bytes) in
        [("zip", zip(&refs, false)), ("rar5", rar5(&entries)), ("7z", sevenz(&entries, false))]
    {
        let Book::Comic(mut c) = open_bytes(bytes, None, &limits).unwrap() else { panic!() };
        assert!(c.read_page(0).is_ok(), "{label}");
        let err = c.read_page(1).unwrap_err();
        assert!(matches!(err, BookError::TooLarge { .. }), "{label}: {err:?}");
    }
    // A solid 7z decodes every entry on the way, so it is refused when opened.
    let err = open_bytes(sevenz(&entries, true), None, &limits).unwrap_err();
    assert!(matches!(err, BookError::TooLarge { .. }), "{err:?}");
}

#[test]
fn total_size_and_entry_count_are_bounded() {
    let entries: Vec<(String, Vec<u8>)> =
        (0..40).map(|i| (format!("p{i}.jpg"), vec![i as u8; 64])).collect();
    let refs: Vec<(&str, Vec<u8>)> =
        entries.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    let few = Limits { max_entries: 10, ..Limits::default() };
    let small = Limits { max_total_bytes: 1000, ..Limits::default() };
    for (label, bytes) in [
        ("zip", zip(&refs, true)),
        ("rar5", rar5(&entries)),
        ("rar4", rar4(&entries)),
        ("7z", sevenz(&entries, false)),
    ] {
        let err = open_bytes(bytes.clone(), None, &few).unwrap_err();
        assert!(matches!(err, BookError::TooLarge { .. }), "{label} count: {err:?}");
        let err = open_bytes(bytes.clone(), None, &small).unwrap_err();
        assert!(matches!(err, BookError::TooLarge { .. }), "{label} total: {err:?}");
        let tiny = Limits { max_file_bytes: 100, ..Limits::default() };
        let err = open_bytes(bytes, None, &tiny).unwrap_err();
        assert!(matches!(err, BookError::TooLarge { .. }), "{label} file: {err:?}");
    }
}

/// Sets the "encrypted" flag on every entry of a ZIP, in local and central headers.
fn mark_encrypted(mut zip: Vec<u8>) -> Vec<u8> {
    let mut i = 0;
    while i + 4 <= zip.len() {
        match &zip[i..i + 4] {
            b"PK\x03\x04" => zip[i + 6] |= 1,
            b"PK\x01\x02" => zip[i + 8] |= 1,
            _ => {}
        }
        i += 1;
    }
    zip
}

#[test]
fn encrypted_zip_entries_are_protected() {
    let bytes = mark_encrypted(zip(&[("p1.jpg", jpeg(4, 4, 1))], true));
    let mut archive = Archive::open(
        Box::new(std::io::Cursor::new(bytes)),
        ArchiveKind::Zip,
        &Limits::default(),
    )
    .unwrap();
    assert!(archive.entries()[0].encrypted);
    let err = archive.read(0).unwrap_err();
    assert!(matches!(err, BookError::Protected), "{err:?}");
    assert_eq!(err.reading_failure(), FailureKind::Protected);
}

#[test]
fn unknown_and_truncated_files() {
    let err = open_bytes(b"just some words".to_vec(), Some("made-up.cbt"), &Limits::default())
        .unwrap_err();
    assert!(matches!(err, BookError::Unsupported { .. }), "{err:?}");
    assert_eq!(err.reading_failure(), FailureKind::Unsupported);
    for (label, bytes) in [
        ("zip", cbz(3, true)),
        ("rar5", rar5(&page_set(3))),
        ("rar4", rar4(&page_set(3))),
        ("7z", sevenz(&page_set(3), true)),
    ] {
        for cut in [8, 40, bytes.len() / 2, bytes.len() - 3] {
            // Either refused when opened, or a page fails to read; never a panic or a hang.
            if let Ok(Book::Comic(mut c)) =
                open_bytes(bytes[..cut].to_vec(), None, &Limits::default())
            {
                let _ = (0..c.pages().len()).map(|i| c.read_page(i)).collect::<Vec<_>>();
            }
            let _ = label;
        }
    }
}

#[cfg(unix)]
#[test]
fn folder_links_are_skipped() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.jpg"), jpeg(4, 4, 9)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("p1.jpg"), jpeg(4, 4, 1)).unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret.jpg"), dir.path().join("p2.jpg"))
        .unwrap();
    let Book::Comic(c) = kaeri_books::open(dir.path(), &Limits::default()).unwrap() else {
        panic!()
    };
    let names: Vec<&str> = c.pages().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["p1.jpg"]);
}

fn epub_with_opf(opf: &str) -> Vec<u8> {
    zip(
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("META-INF/container.xml", container("content.opf")),
            ("content.opf", opf.as_bytes().to_vec()),
        ],
        false,
    )
}

#[test]
fn entity_expansion_is_refused() {
    let opf = r#"<?xml version="1.0"?>
<!DOCTYPE package [
  <!ENTITY a "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
  <!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;">
  <!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;">
  <!ENTITY d "&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;">
  <!ENTITY e "&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;">
  <!ENTITY f "&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;">
  <!ENTITY g "&f;&f;&f;&f;&f;&f;&f;&f;&f;&f;&f;&f;&f;&f;&f;&f;">
]>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>&g;</dc:title></metadata>
  <manifest/><spine/>
</package>"#;
    let err = open_bytes(epub_with_opf(opf), None, &Limits::default()).unwrap_err();
    assert_eq!(err.reading_failure(), FailureKind::Unreadable, "{err:?}");

    let looped = r#"<?xml version="1.0"?>
<!DOCTYPE package [<!ENTITY a "&b;"><!ENTITY b "&a;">]>
<package xmlns="http://www.idpf.org/2007/opf"><metadata>&a;</metadata></package>"#;
    let err = open_bytes(epub_with_opf(looped), None, &Limits::default()).unwrap_err();
    assert_eq!(err.reading_failure(), FailureKind::Unreadable, "{err:?}");
}

#[test]
fn external_entities_are_not_fetched() {
    let opf = r#"<?xml version="1.0"?>
<!DOCTYPE package [<!ENTITY x SYSTEM "file:///etc/passwd">]>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="u">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="u">x</dc:identifier><dc:title>A&x;B</dc:title></metadata>
  <manifest/><spine/>
</package>"#;
    // roxmltree has no way to fetch anything; the reference is an error or expands to nothing.
    if let Ok(Book::Epub(e)) = open_bytes(epub_with_opf(opf), None, &Limits::default()) {
        assert!(e.metadata().titles.iter().all(|t| !t.contains("root:")));
    }
}

#[test]
fn deep_and_huge_xml_is_bounded() {
    let deep = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf"><metadata>{}{}</metadata><manifest/><spine/></package>"#,
        "<x>".repeat(100_000),
        "</x>".repeat(100_000)
    );
    // Too many nodes, or too deep for the parser: an error either way, never a stack overflow.
    let limits = Limits { max_xml_nodes: 50_000, ..Limits::default() };
    assert!(open_bytes(epub_with_opf(&deep), None, &limits).is_err());

    let small = Limits { max_xml_bytes: 1024, ..Limits::default() };
    let big = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf"><metadata><!--{}--></metadata></package>"#,
        "x".repeat(4096)
    );
    let err = open_bytes(epub_with_opf(&big), None, &small).unwrap_err();
    assert!(matches!(err, BookError::TooLarge { .. }), "{err:?}");
}

fn epub_with_nav_depth(depth: usize) -> Vec<u8> {
    let mut nav = String::from(
        r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc">"#,
    );
    for i in 0..depth {
        nav += &format!(r#"<ol><li><a href="a.xhtml#d{i}">Level {i}</a>"#);
    }
    for _ in 0..depth {
        nav += "</li></ol>";
    }
    nav += "</nav></body></html>";
    let opf = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="u">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="u">x</dc:identifier><dc:title>Deep Wells</dc:title></metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="a" href="a.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="a"/></spine>
</package>"#;
    zip(
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("META-INF/container.xml", container("content.opf")),
            ("content.opf", opf.as_bytes().to_vec()),
            ("nav.xhtml", nav.into_bytes()),
            ("a.xhtml", b"<html/>".to_vec()),
        ],
        false,
    )
}

fn toc_depth(bytes: Vec<u8>) -> usize {
    let Book::Epub(e) = open_bytes(bytes, None, &Limits::default()).unwrap() else { panic!() };
    let mut level = e.toc();
    let mut seen = 0;
    while let Some(first) = level.first() {
        seen += 1;
        level = &first.children;
    }
    seen
}

#[test]
fn deep_table_of_contents_is_cut_off() {
    assert_eq!(toc_depth(epub_with_nav_depth(50)), Limits::default().max_toc_depth);
    // Nesting past the XML depth limit: the navigation document is refused and the book still
    // opens, with no table of contents.
    assert_eq!(toc_depth(epub_with_nav_depth(5000)), 0);
}

#[test]
fn an_epub_cannot_point_outside_itself() {
    let opf = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="u">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="u">x</dc:identifier><dc:title>Far Shore</dc:title></metadata>
  <manifest>
    <item id="a" href="../../../../etc/hosts" media-type="application/xhtml+xml"/>
    <item id="b" href="/etc/hosts" media-type="application/xhtml+xml"/>
    <item id="c" href="file:///etc/hosts" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="a"/><itemref idref="b"/><itemref idref="c"/></spine>
</package>"#;
    match open_bytes(epub_with_opf(opf), None, &Limits::default()) {
        Ok(Book::Epub(mut e)) => {
            assert!(e.package().manifest.is_empty());
            assert!(e.resource("../../../../etc/hosts").is_err());
            assert!(e.resource("/etc/hosts").is_err());
        }
        Ok(_) => panic!("not an EPUB"),
        Err(err) => assert_eq!(err.reading_failure(), FailureKind::Unreadable, "{err:?}"),
    }
}
