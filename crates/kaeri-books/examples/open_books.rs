//! Opens every file under a folder as a book and reads all of it, printing one line per file:
//! the format, what it found, and how long it took. For checking the readers against a local
//! library: `cargo run -p kaeri-books --example open_books -- <folder>`. Every book is also
//! written again in memory (comics as CBZ and fixed-layout EPUB, EPUBs as KEPUB) and the copy
//! opened, so the writers meet the same files.

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::Instant;

use std::io::Cursor;

use kaeri_books::write::{
    BookMeta, CbzWriter, EpubFlavor, EpubWriter, FixedLayout, kepub_from_epub,
};
use kaeri_books::{Book, ComicInfo, Limits, open_bytes};

fn main() {
    let root = std::env::args().nth(1).expect("a folder");
    let mut files = Vec::new();
    walk(Path::new(&root), &mut files);
    files.sort();
    let mut failed = 0;
    for f in &files {
        let start = Instant::now();
        let line = describe(f);
        if line.starts_with("error") {
            failed += 1;
        }
        println!(
            "{:>7.1} ms  {}  {}",
            start.elapsed().as_secs_f64() * 1000.0,
            f.display(),
            line
        );
    }
    println!("{} files, {} refused", files.len(), failed);
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn describe(path: &Path) -> String {
    let book = match kaeri_books::open(path, &Limits::default()) {
        Ok(b) => b,
        Err(e) => return format!("error ({:?}): {e}", e.reading_failure()),
    };
    match book {
        Book::Comic(mut c) => {
            let n = c.pages().len();
            let info = c.info().map(|i| {
                format!(
                    " series={:?} number={:?} volume={:?} rtl={}",
                    i.series,
                    i.number,
                    i.volume,
                    i.right_to_left()
                )
            });
            let mut bytes = 0usize;
            let mut errors = 0;
            let mut not_images = 0;
            let mut data = Vec::new();
            if c.is_solid() {
                let r = c.extract_pages(0, &mut |_, d| {
                    bytes += d.len();
                    data.push(d.clone());
                    not_images += usize::from(kaeri_books::mime::sniff_image(&d).is_none());
                    ControlFlow::Continue(())
                });
                errors += usize::from(r.is_err());
            } else {
                for i in 0..n {
                    match c.read_page(i) {
                        Ok(d) => {
                            bytes += d.len();
                            not_images +=
                                usize::from(kaeri_books::mime::sniff_image(&d).is_none());
                            data.push(d);
                        }
                        Err(_) => errors += 1,
                    }
                }
            }
            let rewritten = rewrite_comic(&data, c.info());
            let first = c.pages().first().map(|p| p.name.clone()).unwrap_or_default();
            format!(
                "{} {n} pages ({} KB, {errors} unreadable, {not_images} not images), first {first:?}{}{}; {rewritten}",
                c.format(),
                bytes / 1024,
                info.unwrap_or_default(),
                c.info_error().map(|e| format!(" comicinfo error: {e}")).unwrap_or_default()
            )
        }
        Book::Epub(mut e) => {
            let m = e.metadata().clone();
            let spine = e.spine().count();
            let toc = e.toc().len();
            let fixed = e.is_fixed_layout();
            let dir = e.package().direction;
            let paths: Vec<String> =
                e.package().manifest.iter().map(|m| m.path.clone()).collect();
            let bad = paths.iter().filter(|p| e.resource(p).is_err()).count();
            let images = e.image_pages().map(|p| p.len());
            let kepub = match kepub_from_epub(&mut e, Cursor::new(Vec::new())) {
                Ok(out) => match open_bytes(
                    out.into_inner(),
                    Some("copy.kepub.epub"),
                    &Limits::default(),
                ) {
                    Ok(Book::Epub(k)) if k.spine().count() == spine && k.toc().len() == toc => {
                        "kepub ok".to_owned()
                    }
                    Ok(_) => "kepub differs".to_owned(),
                    Err(err) => format!("kepub unreadable: {err}"),
                },
                Err(err) => format!("kepub failed: {err}"),
            };
            format!(
                "epub {:?} by {:?} lang {:?}, spine {spine}, toc {toc}, fixed {fixed}, dir {dir:?}, \
                 {} resources ({bad} missing), image pages {images:?}, cover {:?}; {kepub}",
                m.titles.first(),
                m.creators.first().map(|c| &c.name),
                m.languages.first(),
                paths.len(),
                e.cover().map(|c| c.path.clone())
            )
        }
        #[cfg(feature = "pdf")]
        Book::Pdf(p) => {
            let n = p.page_count();
            let passthrough = (0..n).filter(|&i| p.page_image(i).is_some()).count();
            let render = p.render(0, 300, 300).map(|r| (r.width, r.height));
            format!(
                "pdf {n} pages, {passthrough} pass through, page 1 renders {render:?}, info {:?}",
                p.info()
            )
        }
    }
}

/// Writes the pages as a CBZ and a fixed-layout EPUB, opens both, and says whether every page
/// that is an image came back.
fn rewrite_comic(pages: &[Vec<u8>], info: Option<&ComicInfo>) -> String {
    let images: Vec<&Vec<u8>> =
        pages.iter().filter(|p| kaeri_books::mime::sniff_image(p).is_some()).collect();
    let cbz = (|| {
        let mut w = CbzWriter::new(Cursor::new(Vec::new()), info)?;
        for p in &images {
            w.add_page(p)?;
        }
        let bytes = w.finish()?.into_inner();
        match open_bytes(bytes, Some("copy.cbz"), &Limits::default())? {
            Book::Comic(mut c) => {
                let same = c.pages().len() == images.len()
                    && (0..images.len())
                        .all(|i| c.read_page(i).ok().as_ref() == Some(images[i]))
                    && c.info() == info;
                Ok(if same { "cbz ok" } else { "cbz differs" })
            }
            _ => Ok("cbz opened as something else"),
        }
    })();
    let epub = (|| {
        let meta = BookMeta {
            title: info.and_then(|i| i.title.clone()).unwrap_or_else(|| "Untitled".to_owned()),
            ..BookMeta::default()
        };
        let fixed = FixedLayout { width: 1072, height: 1448, spreads: false };
        let mut w =
            EpubWriter::new(Cursor::new(Vec::new()), meta, EpubFlavor::Epub, Some(fixed))?;
        for p in &images {
            w.add_image_page(p, None, None)?;
        }
        let bytes = w.finish()?.into_inner();
        match open_bytes(bytes, Some("copy.epub"), &Limits::default())? {
            Book::Epub(mut e) => {
                let back = e.image_pages().unwrap_or_default();
                let same = back.len() == images.len()
                    && back.iter().zip(&images).all(|(b, i)| {
                        e.resource(&b.path).ok().map(|r| r.data).as_ref() == Some(*i)
                    });
                Ok(if same { "fixed epub ok" } else { "fixed epub differs" })
            }
            _ => Ok("epub opened as something else"),
        }
    })();
    let show =
        |r: kaeri_books::Result<&str>| r.map_or_else(|e| format!("failed: {e}"), str::to_owned);
    format!("{}, {}", show(cbz), show(epub))
}
