//! What every target shares: the limits, and a walk that reads everything a reader offers.

#![allow(dead_code)]

use std::ops::ControlFlow;

use kaeri_books::archive::{Archive, ArchiveKind};
use kaeri_books::{Comic, Limits};

/// The default limits with the byte budgets shrunk to fit the fuzzer's memory cap. Ratios,
/// counts and depths stay as they ship.
pub fn limits() -> Limits {
    const MIB: u64 = 1024 * 1024;
    Limits {
        max_entry_bytes: 64 * MIB,
        max_total_bytes: 256 * MIB,
        max_decoder_bytes: 256 * MIB,
        max_pdf_bytes: 64 * MIB,
        max_render_pixels: 1024 * 1024,
        ..Limits::default()
    }
}

/// Opens the bytes as an archive of `kind`, reads every entry one by one, then reads the pages
/// as a comic, one by one and in one extraction pass.
pub fn archive(data: &[u8], kind: ArchiveKind) {
    let limits = limits();
    let Ok(mut a) = Archive::open(Box::new(std::io::Cursor::new(data.to_vec())), kind, &limits)
    else {
        return;
    };
    // Reading one entry of a solid archive decodes everything before it, so reading each in
    // turn is quadratic; the engine extracts solid archives in one pass instead.
    let n = a.entries().len();
    let one_by_one: Vec<usize> =
        if a.is_solid() { [0, n / 2, n.saturating_sub(1)].into() } else { (0..n).collect() };
    for i in one_by_one.into_iter().filter(|&i| i < n) {
        let _ = a.read(i);
    }
    let _ = a.extract(&|_| true, &mut |_, _| ControlFlow::Continue(()));
    let Ok(a) = Archive::open(Box::new(std::io::Cursor::new(data.to_vec())), kind, &limits)
    else {
        return;
    };
    let Ok(mut comic) = Comic::from_archive(a, &limits) else {
        return;
    };
    let _ = comic.info();
    if !comic.is_solid() {
        for i in 0..comic.pages().len() {
            let _ = comic.read_page(i);
        }
    }
    let _ = comic.extract_pages(comic.pages().len() / 2, &mut |_, _| ControlFlow::Continue(()));
}
