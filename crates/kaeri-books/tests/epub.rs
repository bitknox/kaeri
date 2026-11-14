//! EPUB 2 and 3: package, spine, navigation, resources, fonts, fixed layout and DRM.

mod common;

use common::*;
use kaeri_books::epub::{Direction, Layout, PageSpread};
use kaeri_books::{Book, BookError, Epub, FailureKind, Limits, open_bytes};

fn epub(bytes: Vec<u8>) -> Epub {
    match open_bytes(bytes, Some("made-up.epub"), &Limits::default()).unwrap() {
        Book::Epub(e) => *e,
        other => panic!("expected an EPUB, got {:?}", other.format()),
    }
}

#[test]
fn epub3_package_and_metadata() {
    let e = epub(epub_novel());
    let p = e.package();
    assert_eq!(p.path, "OEBPS/content.opf");
    assert_eq!(p.version, "3.0");
    let m = e.metadata();
    assert_eq!(m.titles, ["The Clockwork Orchard"]);
    assert_eq!(m.creators[0].name, "Imogen Vale");
    assert_eq!(m.creators[0].role.as_deref(), Some("aut"));
    assert_eq!(m.identifier.as_deref(), Some(UUID));
    assert_eq!(m.languages, ["en"]);
    assert_eq!(m.publisher.as_deref(), Some("Lantern Press"));
    let series = m.series.as_ref().unwrap();
    assert_eq!(series.name, "Orchard Tales");
    assert_eq!(series.position.as_deref(), Some("2"));
    assert_eq!(p.direction, Some(Direction::Ltr));
    assert!(!e.is_fixed_layout());
    assert_eq!(e.cover().unwrap().path, "OEBPS/images/cover.png");
    // A remote resource is noted and never loaded; one outside the book is dropped.
    assert_eq!(p.remote, ["https://cdn.example/remote.css"]);
    assert!(p.manifest.iter().all(|m| !m.path.contains("outside")));
}

#[test]
fn epub3_spine_with_non_linear_items() {
    let e = epub(epub_novel());
    let spine: Vec<(&str, bool)> =
        e.spine().map(|(s, m)| (m.path.as_str(), s.linear)).collect();
    assert_eq!(
        spine,
        [
            ("OEBPS/text/ch1.xhtml", true),
            ("OEBPS/text/ch 2.xhtml", true),
            ("OEBPS/text/notes.xhtml", false)
        ]
    );
}

#[test]
fn epub3_nav_wins_over_ncx() {
    let e = epub(epub_novel());
    let toc = e.toc();
    assert_eq!(toc.len(), 3);
    assert_eq!(toc[0].label, "One Morning");
    assert_eq!(toc[0].href.as_deref(), Some("OEBPS/text/ch1.xhtml"));
    assert_eq!(toc[0].children[0].label, "The Gate");
    assert_eq!(toc[0].children[0].href.as_deref(), Some("OEBPS/text/ch1.xhtml#s2"));
    assert_eq!(toc[1].href.as_deref(), Some("OEBPS/text/ch 2.xhtml"));
    assert_eq!(toc[2].label, "Extras");
    assert_eq!(toc[2].href, None);
    assert_eq!(toc[2].children[0].label, "Notes");
    let pages: Vec<&str> = e.page_list().iter().map(|p| p.label.as_str()).collect();
    assert_eq!(pages, ["1", "2"]);
}

#[test]
fn resources_carry_their_media_type() {
    let mut e = epub(epub_novel());
    let css = e.resource("OEBPS/styles/book.css").unwrap();
    assert_eq!(css.media_type, "text/css");
    assert_eq!(css.data, b"body { margin: 0 }");
    let ch = e.resource("OEBPS/text/ch1.xhtml").unwrap();
    assert_eq!(ch.media_type, "application/xhtml+xml");
    // Not in the manifest: the type comes from the extension.
    let nav = e.resource("OEBPS/toc.ncx").unwrap();
    assert_eq!(nav.media_type, "application/x-dtbncx+xml");
    assert!(matches!(e.resource("OEBPS/nothing.css"), Err(BookError::Missing { .. })));
}

#[test]
fn links_resolve_inside_the_book_only() {
    let e = epub(epub_novel());
    let base = "OEBPS/text/ch1.xhtml";
    let h = e.resolve(base, "../styles/book.css").unwrap();
    assert_eq!(h.path, "OEBPS/styles/book.css");
    let h = e.resolve(base, "ch%202.xhtml#p2").unwrap();
    assert_eq!((h.path.as_str(), h.fragment.as_deref()), ("OEBPS/text/ch 2.xhtml", Some("p2")));
    assert_eq!(e.resolve(base, "#s2").unwrap().path, base);
    for bad in [
        "../../../etc/passwd",
        "/etc/passwd",
        "javascript:alert(1)",
        "https://cdn.example/x.css",
        "file:///etc/passwd",
        "data:text/html,hi",
        "\\\\server\\share",
    ] {
        assert!(e.resolve(base, bad).is_none(), "{bad}");
    }
}

#[test]
fn obfuscated_font_comes_back_plain() {
    let mut e = epub(epub_novel());
    let font = e.resource("OEBPS/fonts/serif.otf").unwrap();
    assert_eq!(font.data, fake_font());
    assert_eq!(font.media_type, "font/otf");
}

#[test]
fn scripts_are_left_as_bytes() {
    // Reading never runs anything; the chapter comes back as it is stored, for the novel
    // pipeline's sanitiser.
    let mut e = epub(epub_novel());
    let ch = e.resource("OEBPS/text/ch1.xhtml").unwrap();
    assert!(String::from_utf8(ch.data).unwrap().contains("<script>"));
    assert!(e.image_pages().is_none(), "a novel is not an image book");
}

#[test]
fn epub2_with_ncx_doctype_and_calibre_series() {
    let e = epub(epub2());
    let m = e.metadata();
    assert_eq!(m.titles, ["Salt and Signal"]);
    assert_eq!(m.creators[0].role.as_deref(), Some("aut"));
    let s = m.series.as_ref().unwrap();
    assert_eq!((s.name.as_str(), s.position.as_deref()), ("Harbour Lights", Some("3")));
    assert_eq!(e.cover().unwrap().path, "cover.jpg");
    let toc = e.toc();
    assert_eq!(toc[0].label, "Departure");
    assert_eq!(toc[0].href.as_deref(), Some("t1.html#top"));
    assert_eq!(toc[0].children[0].label, "The Pier");
}

#[test]
fn fixed_layout_image_book_reads_as_pages() {
    let mut e = epub(epub_manga(6));
    assert!(e.is_fixed_layout());
    assert_eq!(e.package().direction, Some(Direction::Rtl));
    assert_eq!(e.layout_of(0), Layout::PrePaginated);
    assert_eq!(e.package().rendition.spread.as_deref(), Some("landscape"));
    let pages = e.image_pages().expect("an image book");
    let paths: Vec<&str> = pages.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(paths, (1..=6).map(|i| format!("item/images/p{i:03}.jpg")).collect::<Vec<_>>());
    assert!(pages.iter().all(|p| p.media_type == "image/jpeg"));
    assert_eq!(pages[0].spread, Some(PageSpread::Right));
    assert_eq!(pages[1].spread, Some(PageSpread::Left));
    let img = e.resource(&pages[2].path).unwrap();
    assert_eq!(img.data, jpeg(30, 40, 3));
}

#[test]
fn missing_container_falls_back_to_the_only_opf() {
    let opf = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="u">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="u">x</dc:identifier><dc:title>Quiet Hills</dc:title></metadata>
  <manifest><item id="a" href="a.xhtml" media-type="application/xhtml+xml"/></manifest>
  <spine><itemref idref="a"/></spine>
</package>"#;
    let bytes = zip(
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("book/package.opf", opf.as_bytes().to_vec()),
            (
                "book/a.xhtml",
                b"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>Hi</p></body></html>"
                    .to_vec(),
            ),
        ],
        false,
    );
    let e = epub(bytes);
    assert_eq!(e.metadata().titles, ["Quiet Hills"]);
    assert_eq!(e.spine().next().unwrap().1.path, "book/a.xhtml");
}

fn with_extra(extra: &[(&str, &[u8])]) -> Vec<u8> {
    let opf = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="u">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="u">x</dc:identifier><dc:title>Locked Garden</dc:title></metadata>
  <manifest><item id="a" href="a.xhtml" media-type="application/xhtml+xml"/></manifest>
  <spine><itemref idref="a"/></spine>
</package>"#;
    let mut entries: Vec<(&str, Vec<u8>)> = vec![
        ("mimetype", b"application/epub+zip".to_vec()),
        ("META-INF/container.xml", container("content.opf")),
        ("content.opf", opf.as_bytes().to_vec()),
        ("a.xhtml", b"<html/>".to_vec()),
    ];
    entries.extend(extra.iter().map(|(n, d)| (*n, d.to_vec())));
    zip(&entries, false)
}

#[test]
fn drm_is_protected() {
    let aes = br#"<?xml version="1.0"?>
<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#">
  <enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes128-cbc"/>
  <enc:CipherData><enc:CipherReference URI="a.xhtml"/></enc:CipherData></enc:EncryptedData>
</encryption>"#;
    for extra in [
        &[("META-INF/encryption.xml", &aes[..])][..],
        &[("META-INF/rights.xml", &b"<rights/>"[..])][..],
        &[("META-INF/sinf.xml", &b"<sinf/>"[..])][..],
    ] {
        let err = open_bytes(with_extra(extra), None, &Limits::default()).unwrap_err();
        assert!(matches!(err, BookError::Protected), "{err:?}");
        assert_eq!(err.reading_failure(), FailureKind::Protected);
    }
}

#[test]
fn missing_package_is_unreadable() {
    let bytes = zip(
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("META-INF/container.xml", container("nowhere.opf")),
        ],
        false,
    );
    let err = open_bytes(bytes, None, &Limits::default()).unwrap_err();
    assert_eq!(err.reading_failure(), FailureKind::Unreadable, "{err:?}");
}
