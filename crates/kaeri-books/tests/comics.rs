//! Comic archives and folders: page order, ComicInfo, reading pages, and one-pass extraction.

// The fixtures cover every archive format, so the file needs every reader.
#![cfg(all(feature = "cbr", feature = "cb7"))]

mod common;

use std::ops::ControlFlow;

use common::*;
use kaeri_books::comicinfo::ComicPageType;
use kaeri_books::{Book, Comic, Format, Limits, open_bytes};

fn comic(bytes: Vec<u8>, name: &str) -> Comic {
    match open_bytes(bytes, Some(name), &Limits::default()).unwrap() {
        Book::Comic(c) => *c,
        other => panic!("expected a comic, got {:?}", other.format()),
    }
}

fn names(c: &Comic) -> Vec<&str> {
    c.pages().iter().map(|p| p.name.as_str()).collect()
}

/// Reading order for `page_set(n)`.
fn expected(n: u32) -> Vec<String> {
    page_set(n).into_iter().map(|(name, _)| name).collect()
}

fn check_pages(c: &mut Comic, n: u32) {
    assert_eq!(names(c), expected(n));
    let originals = page_set(n);
    for (i, (_, data)) in originals.iter().enumerate() {
        assert_eq!(&c.read_page(i).unwrap(), data, "page {i}");
    }
    // Extraction from the third page on hands back every later page once, with its bytes.
    let mut got = Vec::new();
    c.extract_pages(2, &mut |i, data| {
        got.push((i, data));
        ControlFlow::Continue(())
    })
    .unwrap();
    got.sort_by_key(|(i, _)| *i);
    let want: Vec<(usize, Vec<u8>)> =
        originals.into_iter().enumerate().skip(2).map(|(i, (_, d))| (i, d)).collect();
    assert_eq!(got, want);
}

#[test]
fn cbz_pages_in_natural_order_without_clutter() {
    let mut c = comic(cbz(12, true), "made-up.cbz");
    assert_eq!(c.format(), Format::Cbz);
    assert!(!c.is_solid());
    assert_eq!(c.pages()[1].mime, "image/png");
    assert_eq!(c.pages()[0].mime, "image/jpeg");
    check_pages(&mut c, 12);
}

#[test]
fn cbz_comicinfo_is_read_and_applied_to_pages() {
    let c = comic(cbz(5, true), "made-up.cbz");
    let info = c.info().expect("ComicInfo");
    assert_eq!(info.series.as_deref(), Some("The Lantern Keepers"));
    assert_eq!(info.number.as_deref(), Some("7"));
    assert!(info.right_to_left());
    assert_eq!(info.language.as_deref(), Some("en"));
    assert_eq!(c.pages()[0].kind, Some(ComicPageType::FrontCover));
    assert!(c.pages()[3].double_page);
    assert!(!c.pages()[2].double_page);
    // It serialises for the local file record.
    let json = serde_json::to_string(info).unwrap();
    assert!(json.contains("The Lantern Keepers"));
}

#[test]
fn damaged_comicinfo_keeps_the_pages() {
    let mut entries: Vec<(String, Vec<u8>)> = page_set(3);
    entries.push(("ComicInfo.xml".into(), b"<ComicInfo><Series>unclosed".to_vec()));
    let refs: Vec<(&str, Vec<u8>)> =
        entries.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    let c = comic(zip(&refs, false), "made-up.cbz");
    assert_eq!(c.pages().len(), 3);
    assert!(c.info().is_none());
    assert!(c.info_error().is_some());
}

#[test]
fn deflated_cbz_reads() {
    let entries = page_set(4);
    let refs: Vec<(&str, Vec<u8>)> =
        entries.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    let mut c = comic(zip(&refs, false), "made-up.cbz");
    check_pages(&mut c, 4);
}

#[test]
fn cbr_rar5_stored() {
    let entries: Vec<_> = page_set(6).into_iter().rev().collect();
    let mut c = comic(rar5(&entries), "made-up.cbr");
    assert_eq!(c.format(), Format::Cbr);
    check_pages(&mut c, 6);
}

#[test]
fn cbr_rar4_stored() {
    let entries: Vec<_> = page_set(5).into_iter().rev().collect();
    let mut c = comic(rar4(&entries), "made-up.cbr");
    assert_eq!(c.format(), Format::Cbr);
    check_pages(&mut c, 5);
}

#[test]
fn cbr_with_comicinfo() {
    let mut entries = page_set(4);
    entries.push(("ComicInfo.xml".into(), COMICINFO.as_bytes().to_vec()));
    let c = comic(rar5(&entries), "made-up.cbr");
    assert_eq!(c.info().unwrap().volume.as_deref(), Some("2"));
}

#[test]
fn cb7_non_solid() {
    let entries: Vec<_> = page_set(6).into_iter().rev().collect();
    let mut c = comic(sevenz(&entries, false), "made-up.cb7");
    assert_eq!(c.format(), Format::Cb7);
    assert!(!c.is_solid());
    check_pages(&mut c, 6);
}

#[test]
fn cb7_solid_extracts_in_one_pass() {
    let mut entries: Vec<_> = page_set(8).into_iter().rev().collect();
    entries.push(("ComicInfo.xml".into(), COMICINFO.as_bytes().to_vec()));
    let mut c = comic(sevenz(&entries, true), "made-up.cb7");
    assert!(c.is_solid());
    assert!(c.info().is_some());
    check_pages(&mut c, 8);
}

#[test]
fn extraction_stops_when_asked() {
    let mut c = comic(cbz(10, false), "made-up.cbz");
    let mut seen = 0;
    c.extract_pages(0, &mut |_, _| {
        seen += 1;
        if seen == 3 { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
    })
    .unwrap();
    assert_eq!(seen, 3);
}

#[test]
fn content_wins_over_extension() {
    // A ZIP named .cbr, and a RAR named .cbz.
    assert_eq!(comic(cbz(2, false), "made-up.cbr").format(), Format::Cbz);
    assert_eq!(comic(rar5(&page_set(2)), "made-up.cbz").format(), Format::Cbr);
}

#[test]
fn folder_of_images() {
    let dir = tempfile::tempdir().unwrap();
    for (name, data) in page_set(11) {
        let file = name.rsplit('/').next().unwrap();
        std::fs::write(dir.path().join(file), data).unwrap();
    }
    std::fs::write(dir.path().join("readme.txt"), b"made-up").unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    std::fs::write(dir.path().join("nested/p0.jpg"), jpeg(4, 4, 0)).unwrap();
    let Book::Comic(mut c) = kaeri_books::open(dir.path(), &Limits::default()).unwrap() else {
        panic!("a folder is a comic");
    };
    assert_eq!(c.format(), Format::Folder);
    let want: Vec<String> =
        expected(11).iter().map(|n| n.rsplit('/').next().unwrap().to_owned()).collect();
    assert_eq!(names(&c), want);
    assert_eq!(c.read_page(0).unwrap(), page_set(1)[0].1);
}

#[test]
fn an_archive_with_no_pages_is_empty() {
    let err =
        open_bytes(zip(&[("notes.txt", b"words".to_vec())], false), None, &Limits::default())
            .unwrap_err();
    assert!(matches!(err, kaeri_books::BookError::Empty), "{err:?}");
}
