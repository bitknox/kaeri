//! PDF: one-JPEG pages pass through untouched, other pages render.

#![cfg(feature = "pdf")]

mod common;

use common::*;
use kaeri_books::{Book, BookError, FailureKind, Limits, Pdf, open_bytes};

fn pdf_book(bytes: Vec<u8>) -> Pdf {
    match open_bytes(bytes, Some("made-up.pdf"), &Limits::default()).unwrap() {
        Book::Pdf(p) => p,
        other => panic!("expected a PDF, got {:?}", other.format()),
    }
}

#[test]
fn scanned_pages_pass_through_as_jpeg() {
    let jpegs: Vec<Vec<u8>> = (0..3).map(|i| jpeg(60, 80, i)).collect();
    let pdf = pdf_book(pdf_of_jpegs(&jpegs, 60, 80));
    assert_eq!(pdf.page_count(), 3);
    assert_eq!(pdf.page_size(0), Some((60.0, 80.0)));
    for (i, j) in jpegs.iter().enumerate() {
        let img = pdf.page_image(i).expect("passthrough");
        assert_eq!(&img.data, j, "page {i} is the original file");
        assert_eq!((img.mime, img.width, img.height), ("image/jpeg", 60, 80));
    }
    let info = pdf.info();
    assert_eq!(info.title.as_deref(), Some("The Paper Crane Almanac"));
    assert_eq!(info.author.as_deref(), Some("Made-up Studio"));
}

#[test]
fn scanned_page_also_renders() {
    let pdf = pdf_book(pdf_of_jpegs(&[jpeg(60, 80, 1)], 60, 80));
    let page = pdf.render(0, 120, 120).unwrap();
    assert_eq!((page.width, page.height), (90, 120));
    assert_eq!(page.rgba.len(), 90 * 120 * 4);
}

#[test]
fn vector_page_renders_and_does_not_pass_through() {
    let pdf = pdf_book(pdf_vector());
    assert!(pdf.page_image(0).is_none());
    let page = pdf.render(0, 100, 100).unwrap();
    assert_eq!((page.width, page.height), (100, 100));
    let px = |x: u32, y: u32| {
        let o = ((y * page.width + x) * 4) as usize;
        &page.rgba[o..o + 4]
    };
    // PDF y runs up: the square sits from 10 to 60 in pixels on both axes, at the bottom left.
    assert_eq!(px(5, 5), [255, 255, 255, 255], "white outside");
    let red = px(30, 70);
    assert!(red[0] > 200 && red[1] < 40 && red[2] < 40, "red inside: {red:?}");
}

#[test]
fn render_respects_the_pixel_limit() {
    let limits = Limits { max_render_pixels: 10_000, ..Limits::default() };
    let Book::Pdf(pdf) = open_bytes(pdf_vector(), None, &limits).unwrap() else { panic!() };
    let page = pdf.render(0, 4000, 4000).unwrap();
    assert!(u64::from(page.width) * u64::from(page.height) <= 10_000);
    assert!(pdf.render(5, 10, 10).is_err());
}

#[test]
fn damaged_pdf_is_unreadable() {
    let err =
        open_bytes(b"%PDF-1.7\nnot really a pdf at all".to_vec(), None, &Limits::default())
            .unwrap_err();
    assert_eq!(err.reading_failure(), FailureKind::Unreadable, "{err:?}");
}

#[test]
fn too_many_pages_is_refused() {
    let jpegs: Vec<Vec<u8>> = (0..3).map(|i| jpeg(8, 8, i)).collect();
    let limits = Limits { max_pdf_pages: 2, ..Limits::default() };
    let err = open_bytes(pdf_of_jpegs(&jpegs, 8, 8), None, &limits).unwrap_err();
    assert!(matches!(err, BookError::TooLarge { .. }), "{err:?}");
}

#[test]
fn deeply_nested_objects_do_not_overflow() {
    // Far deeper than any parser could recurse on a thread's stack. Nested dictionaries send
    // hayro through its repair path, which is slow per level, so they are fewer.
    let nested = format!("{}{}", "[".repeat(200_000), "]".repeat(200_000));
    let dict = format!("{}{}", "<< /A ".repeat(10_000), ">>".repeat(10_000));
    for deep in [nested, dict] {
        let objects = vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 50 50] /Contents 4 0 R /Deep {deep} >>")
                .into_bytes(),
            format!("<< /Length 10 >>\nstream\n{deep}\nendstream").into_bytes(),
        ];
        if let Ok(Book::Pdf(pdf)) = open_bytes(pdf(&objects, 1, None), None, &Limits::default())
        {
            let _ = pdf.page_image(0);
            let _ = pdf.render(0, 50, 50);
        }
    }
}
