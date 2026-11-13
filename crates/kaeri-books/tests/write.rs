//! The writers, held to the readers: every book written here is opened again and compared.

mod common;

use std::io::Cursor;

use common::*;
use kaeri_books::comicinfo::{ComicPage, ComicPageType, MangaFlag};
use kaeri_books::epub::{Direction, Layout, PageSpread, Series};
use kaeri_books::write::{
    BookMeta, CbzWriter, EpubFlavor, EpubWriter, FixedLayout, KINDLE_PART_BYTES, check_epub,
    kepub_from_epub, split_parts,
};
use kaeri_books::{Book, BookError, Comic, ComicInfo, Epub, Limits, open_bytes};

fn open(bytes: Vec<u8>, name: &str) -> Book {
    open_bytes(bytes, Some(name), &Limits::default()).unwrap()
}

fn comic(bytes: Vec<u8>) -> Comic {
    match open(bytes, "made-up.cbz") {
        Book::Comic(c) => *c,
        other => panic!("expected a comic, got {:?}", other.format()),
    }
}

fn epub(bytes: Vec<u8>) -> Epub {
    match open(bytes, "made-up.epub") {
        Book::Epub(e) => *e,
        other => panic!("expected an EPUB, got {:?}", other.format()),
    }
}

fn info() -> ComicInfo {
    ComicInfo {
        title: Some("Chapter 3: Lanterns & <Rain>".into()),
        series: Some("Paper Harbour".into()),
        number: Some("3".into()),
        volume: Some("1".into()),
        count: Some(12),
        summary: Some("Two \"friends\" find a map.".into()),
        year: Some(2026),
        month: Some(4),
        day: Some(9),
        writer: Some("R. Ando".into()),
        language: Some("ja".into()),
        manga: Some(MangaFlag::YesAndRightToLeft),
        page_count: Some(3),
        pages: vec![
            ComicPage {
                image: 0,
                kind: Some(ComicPageType::FrontCover),
                double_page: false,
                width: Some(40),
                height: Some(60),
            },
            ComicPage {
                image: 2,
                kind: None,
                double_page: true,
                width: Some(80),
                height: Some(60),
            },
        ],
        ..ComicInfo::default()
    }
}

#[test]
fn cbz_round_trip() {
    let pages = [jpeg(40, 60, 1), png(40, 60, 2), jpeg(80, 60, 3)];
    let mut w = CbzWriter::new(Cursor::new(Vec::new()), Some(&info())).unwrap();
    let names: Vec<String> = pages.iter().map(|p| w.add_page(p).unwrap()).collect();
    assert_eq!(names, ["00001.jpg", "00002.png", "00003.jpg"]);
    let bytes = w.finish().unwrap().into_inner();

    let mut c = comic(bytes);
    let read = c.info().unwrap();
    assert_eq!(read, &info());
    assert!(read.right_to_left());
    assert_eq!(c.pages().len(), 3);
    assert_eq!(c.pages()[0].kind, Some(ComicPageType::FrontCover));
    assert!(c.pages()[2].double_page);
    for (i, p) in pages.iter().enumerate() {
        assert_eq!(&c.read_page(i).unwrap(), p);
    }
}

#[test]
fn cbz_refuses_what_is_not_an_image() {
    let mut w = CbzWriter::new(Cursor::new(Vec::new()), None).unwrap();
    assert!(w.add_page(b"<html>not a page</html>").is_err());
    w.add_page(&png(4, 4, 0)).unwrap();
    let c = comic(w.finish().unwrap().into_inner());
    assert!(c.info().is_none());
    assert_eq!(c.pages().len(), 1);
}

fn meta() -> BookMeta {
    BookMeta {
        title: "The Glass Orchard".into(),
        creators: vec!["Imogen Vale".into()],
        language: "en".into(),
        series: Some(Series { name: "Orchard Tales".into(), position: Some("2".into()) }),
        publisher: Some("Lantern Press".into()),
        description: Some("Apples & <ghosts>".into()),
        modified: Some("2026-11-06T10:00:00Z".into()),
        ..BookMeta::default()
    }
}

fn novel(flavor: EpubFlavor) -> Vec<u8> {
    let mut w = EpubWriter::new(Cursor::new(Vec::new()), meta(), flavor, None).unwrap();
    w.add_resource("styles/book.css", "text/css", b"p { margin: 0 }").unwrap();
    w.set_cover(&png(30, 40, 7)).unwrap();
    w.add_chapter(
        "One",
        "<h1>One</h1><p onclick=\"steal()\">It rained.&nbsp;The orchard was glass. Was it?</p>\
         <script>alert(1)</script><p><a href=\"javascript:alert(2)\">x</a><a href=\"c0002.xhtml\">on</a></p>",
    )
    .unwrap();
    w.add_chapter("Two & <more>", "<p>Morning <ruby>朝<rt>あさ</rt></ruby> came.</p><br/>")
        .unwrap();
    w.finish().unwrap().into_inner()
}

fn chapter_text(e: &mut Epub, spine: usize) -> String {
    let path = e.spine().nth(spine).unwrap().1.path.clone();
    String::from_utf8(e.resource(&path).unwrap().data).unwrap()
}

#[test]
fn reflowable_epub_round_trip() {
    let mut e = epub(novel(EpubFlavor::Epub));
    let m = e.metadata();
    assert_eq!(m.titles, ["The Glass Orchard"]);
    assert_eq!(m.creators[0].name, "Imogen Vale");
    assert_eq!(m.languages, ["en"]);
    assert_eq!(m.publisher.as_deref(), Some("Lantern Press"));
    assert_eq!(m.description.as_deref(), Some("Apples & <ghosts>"));
    assert_eq!(m.series, meta().series);
    assert!(m.identifier.as_deref().unwrap().starts_with("urn:uuid:"));
    assert!(!e.is_fixed_layout());
    assert_eq!(e.spine().count(), 2);
    let toc: Vec<_> = e.toc().iter().map(|t| (t.label.as_str(), t.href.as_deref())).collect();
    assert_eq!(
        toc,
        [("One", Some("OEBPS/c0001.xhtml")), ("Two & <more>", Some("OEBPS/c0002.xhtml"))]
    );
    assert_eq!(e.cover().unwrap().path, "OEBPS/cover.png");
    assert_eq!(e.resource("OEBPS/cover.png").unwrap().data, png(30, 40, 7));
    let css = e.resource("OEBPS/styles/book.css").unwrap();
    assert_eq!(css.media_type, "text/css");
    assert_eq!(css.data, b"p { margin: 0 }");

    let one = chapter_text(&mut e, 0);
    for bad in ["script", "alert", "onclick", "steal", "javascript"] {
        assert!(!one.contains(bad), "{bad} in {one}");
    }
    assert!(one.contains("It rained.\u{a0}The orchard was glass."), "{one}");
    assert!(one.contains("<a href=\"c0002.xhtml\">on</a>"), "{one}");
    assert!(!one.contains("koboSpan"));
    let two = chapter_text(&mut e, 1);
    assert!(two.contains("<ruby>朝<rt>あさ</rt></ruby>"), "{two}");
    assert!(two.contains("<title>Two &amp; &lt;more&gt;</title>"), "{two}");
}

#[test]
fn same_book_same_identifier() {
    let a = epub(novel(EpubFlavor::Epub)).metadata().identifier.clone();
    let b = epub(novel(EpubFlavor::Kepub)).metadata().identifier.clone();
    assert_eq!(a, b);
}

#[test]
fn kepub_has_spans() {
    let mut e = epub(novel(EpubFlavor::Kepub));
    let one = chapter_text(&mut e, 0);
    assert!(one.contains("<div id=\"book-columns\"><div id=\"book-inner\">"), "{one}");
    assert!(one.contains("<span class=\"koboSpan\" id=\"kobo.1.1\">One</span>"), "{one}");
    assert!(one.contains("<span class=\"koboSpan\" id=\"kobo.2.3\">Was it?</span>"), "{one}");
    assert!(!one.contains("script"));
    let two = chapter_text(&mut e, 1);
    assert!(two.contains("<ruby>朝<rt>あさ</rt></ruby>"), "{two}");
}

#[test]
fn chapter_that_is_not_xhtml_is_refused() {
    let mut w =
        EpubWriter::new(Cursor::new(Vec::new()), meta(), EpubFlavor::Epub, None).unwrap();
    assert!(w.add_chapter("Bad", "<p>open <b>tags</p>").is_err());
    assert!(w.add_resource("../escape.css", "text/css", b"").is_err());
    assert!(w.add_resource("content.opf", "text/css", b"").is_err());
    assert!(matches!(w.finish(), Err(BookError::Empty)));
}

#[test]
fn fixed_layout_round_trip() {
    let pages = [jpeg(60, 90, 1), jpeg(60, 90, 2), png(120, 90, 3), jpeg(60, 90, 4)];
    let spreads =
        [None, Some(PageSpread::Right), Some(PageSpread::Center), Some(PageSpread::Left)];
    let fixed = FixedLayout { width: 60, height: 90, spreads: true };
    let m = BookMeta { direction: Some(Direction::Rtl), language: "ja".into(), ..meta() };
    let mut w =
        EpubWriter::new(Cursor::new(Vec::new()), m, EpubFlavor::Epub, Some(fixed)).unwrap();
    assert!(w.add_chapter("No", "<p>x</p>").is_err());
    for (i, (p, s)) in pages.iter().zip(spreads).enumerate() {
        w.add_image_page(p, s, (i == 2).then_some("The map")).unwrap();
    }
    let mut e = epub(w.finish().unwrap().into_inner());
    assert!(e.is_fixed_layout());
    assert_eq!(e.package().direction, Some(Direction::Rtl));
    assert_eq!(e.layout_of(0), Layout::PrePaginated);
    assert_eq!(e.cover().unwrap().path, "OEBPS/images/00001.jpg");
    assert_eq!(e.toc()[0].label, "The map");
    let image_pages = e.image_pages().unwrap();
    assert_eq!(image_pages.len(), 4);
    for (i, ip) in image_pages.iter().enumerate() {
        assert_eq!(ip.spread, spreads[i]);
        assert_eq!(e.resource(&ip.path).unwrap().data, pages[i]);
    }
    assert_eq!(image_pages[2].media_type, "image/png");
}

#[test]
fn kepub_from_an_existing_epub() {
    let mut source = epub(epub_novel());
    let bytes = kepub_from_epub(&mut source, Cursor::new(Vec::new())).unwrap().into_inner();
    assert_eq!(&bytes[30..38], b"mimetype");
    let mut k = epub(bytes);
    assert_eq!(k.metadata(), source.metadata());
    assert_eq!(k.spine().count(), source.spine().count());
    assert_eq!(k.toc(), source.toc());
    let first = chapter_text(&mut k, 0);
    assert!(first.contains("koboSpan"), "{first}");
    // Everything else comes across byte for byte.
    let cover = source.cover().unwrap().path.clone();
    assert_eq!(k.resource(&cover).unwrap().data, source.resource(&cover).unwrap().data);
    // Converting a KEPUB again changes nothing in its chapters.
    let again = epub(kepub_from_epub(&mut k, Cursor::new(Vec::new())).unwrap().into_inner());
    let mut again = again;
    assert_eq!(chapter_text(&mut again, 0), first);
}

#[test]
fn parts_for_kindle() {
    let sizes = [90_000_000, 90_000_000, 90_000_000, 300_000_000, 10];
    let parts = split_parts(&sizes, KINDLE_PART_BYTES);
    assert_eq!(parts, [0..2, 2..3, 3..4, 4..5]);
    assert_eq!(split_parts(&[], KINDLE_PART_BYTES), Vec::<std::ops::Range<usize>>::new());
}

#[test]
fn an_empty_cbz_is_damaged_not_unsupported() {
    let err = open_bytes(Vec::new(), Some("made-up.cbz"), &Limits::default()).unwrap_err();
    assert!(matches!(err, BookError::Malformed { .. }), "{err:?}");
    let err = open_bytes(b"plain text".to_vec(), Some("made-up.epub"), &Limits::default())
        .unwrap_err();
    assert!(matches!(err, BookError::Malformed { .. }), "{err:?}");
    let err =
        open_bytes(b"plain text".to_vec(), Some("notes.txt"), &Limits::default()).unwrap_err();
    assert!(matches!(err, BookError::Unsupported { .. }), "{err:?}");
}

/// The title parts of the novel fuzz's crash inputs (R7E-15): names made only of control
/// characters, which escaping empties.
fn control_titles() -> Vec<String> {
    let nul = |n: usize| "\0".repeat(n);
    vec![
        "\u{15}".into(),
        "\u{1e}".into(),
        format!("{}\u{15}", nul(3)),
        format!("\u{11}{}\u{b}", nul(7)),
        format!("\u{15}{}", nul(18)),
        format!("\u{15}{}", nul(83)),
    ]
}

/// R7E-15. A title of only control characters is refused up front rather than written as a
/// book that fails its own check; such an author is left out, and such a chapter title still
/// makes a book that passes it.
#[test]
fn names_of_only_control_characters_never_make_a_malformed_book() {
    let dir = tempfile::tempdir().unwrap();
    for (n, name) in control_titles().iter().enumerate() {
        let meta =
            BookMeta { title: name.clone(), language: "ja".into(), ..BookMeta::default() };
        let refused = EpubWriter::new(Cursor::new(Vec::new()), meta, EpubFlavor::Kepub, None);
        assert!(matches!(refused, Err(BookError::Unsupported { .. })), "{name:?}");

        let meta = BookMeta {
            title: format!("Paper Moth Garden{name}"),
            creators: vec![name.clone(), "Imogen Vale".into()],
            language: "ja".into(),
            ..BookMeta::default()
        };
        let mut w =
            EpubWriter::new(Cursor::new(Vec::new()), meta, EpubFlavor::Kepub, None).unwrap();
        w.add_chapter(name, "<p>Moths.</p>").unwrap();
        let bytes = w.finish().unwrap().into_inner();
        let path = dir.path().join(format!("{n}.kepub.epub"));
        std::fs::write(&path, &bytes).unwrap();
        check_epub(&path, &Limits::default()).unwrap();
        let m = epub(bytes).metadata().clone();
        assert_eq!(m.titles, ["Paper Moth Garden"]);
        let names: Vec<&str> = m.creators.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Imogen Vale"]);
    }
}
