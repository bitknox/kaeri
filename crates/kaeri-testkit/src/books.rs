//! The pages and books the reading scenes read: made on the spot, from shapes and made-up
//! sentences, so a scene's reader has something to turn through and no real work is named. Comic
//! chapters are CBZ archives of drawn pages, strip chapters are tall drawn images, and the novel's
//! chapters are small EPUBs.

use std::{
    fs::File,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};

use image::{ImageFormat, Rgb, RgbImage};
use kaeri_store::{
    Store,
    shelf::{LocalSeries, ShelfFile},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// What a series of generated books is, and the work it stands for.
pub struct Series {
    pub id: &'static str,
    pub work: &'static str,
    pub name: &'static str,
    /// `manga` or `novel`.
    pub medium: &'static str,
    /// AniList's format, as the catalogue would keep it.
    pub format: &'static str,
    /// The country the work comes from, when it says how it reads.
    pub origin: Option<&'static str>,
}

/// The ink and the paper the page art is drawn in, per chapter.
const TONES: [([u8; 3], [u8; 3]); 5] = [
    ([226, 112, 84], [243, 238, 228]),
    ([78, 140, 160], [240, 241, 236]),
    ([160, 120, 190], [244, 240, 232]),
    ([200, 160, 70], [242, 239, 230]),
    ([90, 150, 110], [238, 241, 234]),
];

/// Digits three squares wide and five high, one row of bits each.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

fn fill(img: &mut RgbImage, x: u32, y: u32, w: u32, h: u32, colour: [u8; 3]) {
    for yy in y..(y + h).min(img.height()) {
        for xx in x..(x + w).min(img.width()) {
            img.put_pixel(xx, yy, Rgb(colour));
        }
    }
}

fn disc(img: &mut RgbImage, cx: i64, cy: i64, r: i64, colour: [u8; 3]) {
    for yy in (cy - r).max(0)..(cy + r).min(i64::from(img.height())) {
        for xx in (cx - r).max(0)..(cx + r).min(i64::from(img.width())) {
            if (xx - cx).pow(2) + (yy - cy).pow(2) <= r * r {
                img.put_pixel(xx as u32, yy as u32, Rgb(colour));
            }
        }
    }
}

fn number(img: &mut RgbImage, x: u32, y: u32, cell: u32, n: u32, colour: [u8; 3]) {
    let mut at = x;
    for digit in n.to_string().bytes() {
        let rows = DIGITS[usize::from(digit - b'0')];
        for (r, bits) in rows.iter().enumerate() {
            for c in 0..3u32 {
                if bits >> (2 - c) & 1 == 1 {
                    fill(img, at + c * cell, y + r as u32 * cell, cell, cell, colour);
                }
            }
        }
        at += 4 * cell;
    }
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = |i: usize| (f32::from(a[i]) * (1.0 - t) + f32::from(b[i]) * t) as u8;
    [m(0), m(1), m(2)]
}

fn png(img: &RgbImage) -> Vec<u8> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).expect("encoding a drawn page");
    out
}

/// One comic page, 600 by 900: panels in a layout of its own, a shape in each, its number.
fn comic_page(chapter: usize, page: u32) -> RgbImage {
    let (ink, paper) = TONES[chapter % TONES.len()];
    let mut img = RgbImage::from_pixel(600, 900, Rgb(paper));
    let layouts: [&[(u32, u32, u32, u32)]; 4] = [
        &[(30, 30, 540, 400), (30, 460, 260, 410), (310, 460, 260, 410)],
        &[(30, 30, 260, 400), (310, 30, 260, 400), (30, 460, 540, 410)],
        &[(30, 30, 540, 250), (30, 310, 540, 250), (30, 590, 540, 280)],
        &[(30, 30, 175, 840), (225, 30, 175, 840), (420, 30, 150, 840)],
    ];
    for (i, &(x, y, w, h)) in layouts[(page as usize + chapter) % 4].iter().enumerate() {
        fill(&mut img, x, y, w, h, [24, 26, 40]);
        fill(&mut img, x + 4, y + 4, w - 8, h - 8, mix(paper, ink, 0.12 + 0.1 * (i as f32)));
        let r = i64::from(w.min(h)) / 3;
        disc(
            &mut img,
            i64::from(x + w / 2),
            i64::from(y + h / 2),
            r,
            mix(ink, [24, 26, 40], 0.1 * i as f32),
        );
        disc(
            &mut img,
            i64::from(x + w / 2) + r / 3,
            i64::from(y + h / 2) - r / 3,
            r / 3,
            paper,
        );
    }
    number(&mut img, 44, 44, 8, page + 1, [24, 26, 40]);
    img
}

/// One strip image, 720 by 1800: bands of the chapter's tone with a shape and its number.
fn strip_image(chapter: usize, index: u32) -> RgbImage {
    let (ink, paper) = TONES[chapter % TONES.len()];
    let mut img = RgbImage::new(720, 1800);
    for y in 0..1800u32 {
        let t = (y as f32 / 1800.0 + index as f32 * 0.17).fract();
        let row = mix(paper, ink, 0.15 + 0.5 * t);
        for x in 0..720u32 {
            img.put_pixel(x, y, Rgb(row));
        }
    }
    for k in 0..5u32 {
        let cx = 140 + ((k * 211 + index * 97) % 460);
        disc(
            &mut img,
            i64::from(cx),
            i64::from(220 + k * 340),
            90 + i64::from(k % 3) * 30,
            mix(ink, [24, 26, 40], 0.45),
        );
    }
    fill(&mut img, 0, 0, 720, 12, [24, 26, 40]);
    number(&mut img, 40, 60, 16, index + 1, [24, 26, 40]);
    img
}

fn stored() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
}

fn archive(path: &Path, images: impl Iterator<Item = RgbImage>) {
    let mut zip = ZipWriter::new(File::create(path).expect("creating a chapter"));
    for (i, img) in images.enumerate() {
        zip.start_file(format!("{i:03}.png"), stored()).expect("a page entry");
        zip.write_all(&png(&img)).expect("a page");
    }
    zip.finish().expect("closing a chapter");
}

/// Made-up sentences, a paragraph of them per call.
fn paragraph(chapter: usize, n: usize) -> String {
    const LINES: [&str; 8] = [
        "The orchard had no gate, only a row of glass lamps that someone lit each dusk.",
        "Mira counted the lamps twice and came to a different number each time.",
        "Past the last tree the path went on as if it had been waiting for her.",
        "A kettle sang somewhere behind the wall, and she followed the sound.",
        "Nobody in the village could say who kept the lamps, only that they were never dark.",
        "She wrote the number on her sleeve in ink, so that morning could argue with it.",
        "The wind carried the smell of pears and warm glass across the open yard.",
        "By the time the bell rang, the orchard had rearranged itself around her.",
    ];
    (0..4)
        .map(|k| LINES[(chapter * 3 + n * 5 + k * 3) % LINES.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn epub(path: &Path, chapter: usize, title: &str) {
    let mut zip = ZipWriter::new(File::create(path).expect("creating a novel"));
    let mut put = |name: &str, body: &str| {
        zip.start_file(name, stored()).expect("an entry");
        zip.write_all(body.as_bytes()).expect("an entry's body");
    };
    put("mimetype", "application/epub+zip");
    put(
        "META-INF/container.xml",
        r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
    );
    let parts = 6;
    let mut manifest = String::from(
        r#"<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>"#,
    );
    let mut spine = String::new();
    let mut toc = String::new();
    for i in 0..parts {
        manifest.push_str(&format!(
            r#"<item id="p{i}" href="p{i}.xhtml" media-type="application/xhtml+xml"/>"#
        ));
        spine.push_str(&format!(r#"<itemref idref="p{i}"/>"#));
        toc.push_str(&format!(r#"<li><a href="p{i}.xhtml">Part {}</a></li>"#, i + 1));
    }
    put(
        "OEBPS/content.opf",
        &format!(
            r#"<?xml version="1.0" encoding="utf-8"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">fixture-orchard-{chapter}</dc:identifier><dc:title>{title}</dc:title><dc:language>en</dc:language><meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
        ),
    );
    put(
        "OEBPS/nav.xhtml",
        &format!(
            r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>{title}</title></head><body><nav epub:type="toc"><ol>{toc}</ol></nav></body></html>"#
        ),
    );
    for i in 0..parts {
        let mut body = format!("<h2>{title}</h2>");
        // A chapter kept from a source opens with its title twice, the source's own heading
        // after the one its plugin gives: the reader says it once.
        if i == 0 {
            body.push_str(&format!("<p><strong>{title}</strong></p>"));
        }
        for n in 0..14 {
            // Two spans, so a saved place such as 0.4.1 names an element that is there.
            body.push_str(&format!(
                "<p><span>{}</span> <span>{}</span></p>",
                paragraph(chapter + i, n),
                paragraph(chapter + i, n + 14)
            ));
        }
        put(
            &format!("OEBPS/p{i}.xhtml"),
            &format!(
                r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>{title}</title></head><body><div>{body}</div></body></html>"#
            ),
        );
    }
    zip.finish().expect("closing a novel");
}

fn register(
    store: &Store,
    folder: &Path,
    series: &Series,
    files: &[(PathBuf, u32, &str, u32)],
    now: i64,
) {
    let local = LocalSeries {
        id: series.id.into(),
        folder: folder.display().to_string(),
        name: series.name.into(),
        medium: series.medium.into(),
        work: Some(series.work.into()),
        state: "matched".into(),
    };
    if let Err(e) = store.put_local_series(&local) {
        tracing::warn!(error = %e, "a reading scene's series was not seeded");
    }
    for (path, chapter, format, pages) in files {
        let file = ShelfFile {
            path: path.display().to_string(),
            chapter: None,
            label: format!("Chapter {chapter}"),
            volume: None,
            number_milli: Some(i64::from(*chapter) * 1_000),
            format: (*format).into(),
            pages: Some(*pages),
            comicinfo: None,
            source: None,
        };
        let size = std::fs::metadata(path).map_or(0, |m| m.len());
        if let Err(e) = store.put_local_file(series.id, &file, size, now) {
            tracing::warn!(error = %e, "a reading scene's chapter was not seeded");
        }
    }
    let _ = store.seed_book_work(
        series.work,
        series.medium,
        Some(series.format),
        series.origin,
        series.name,
        now,
    );
}

/// The comic chapters of `series`, `chapters` of `pages` pages each.
pub fn comics(
    store: &Store,
    data: &Path,
    series: &Series,
    chapters: &[u32],
    pages: u32,
    now: i64,
) {
    let folder = data.join("fixture-books").join(series.id);
    let _ = std::fs::create_dir_all(&folder);
    let mut files = Vec::new();
    for &chapter in chapters {
        let path = folder.join(format!("Chapter {chapter}.cbz"));
        archive(&path, (0..pages).map(|p| comic_page(chapter as usize, p)));
        files.push((path, chapter, "cbz", pages));
    }
    register(store, &folder, series, &files, now);
}

/// The strip chapters of `series`: tall images, `images` to a chapter.
pub fn strips(
    store: &Store,
    data: &Path,
    series: &Series,
    chapters: &[u32],
    images: u32,
    now: i64,
) {
    let folder = data.join("fixture-books").join(series.id);
    let _ = std::fs::create_dir_all(&folder);
    let mut files = Vec::new();
    for &chapter in chapters {
        let path = folder.join(format!("Chapter {chapter}.cbz"));
        archive(&path, (0..images).map(|i| strip_image(chapter as usize, i)));
        files.push((path, chapter, "cbz", images));
    }
    register(store, &folder, series, &files, now);
}

/// The novel's chapters, each a small EPUB of six parts.
pub fn novels(store: &Store, data: &Path, series: &Series, chapters: &[u32], now: i64) {
    let folder = data.join("fixture-books").join(series.id);
    let _ = std::fs::create_dir_all(&folder);
    let mut files = Vec::new();
    for &chapter in chapters {
        let path = folder.join(format!("Chapter {chapter}.epub"));
        epub(&path, chapter as usize, &format!("{}, chapter {chapter}", series.name));
        files.push((path, chapter, "epub", 0));
    }
    register(store, &folder, series, &files, now);
}
