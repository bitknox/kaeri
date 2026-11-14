//! Book files made in code for the tests. Every title, name and word in them is made up; pages
//! are generated gradients and noise, not artwork.

#![allow(dead_code)]

use std::io::{Cursor, Write};

use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

/// A small greyscale JPEG with a pattern that depends on `seed`.
pub fn jpeg(width: u32, height: u32, seed: u32) -> Vec<u8> {
    let img = image::GrayImage::from_fn(width, height, |x, y| {
        image::Luma([((x * 7 + y * 3 + seed * 31) % 251) as u8])
    });
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80);
    img.write_with_encoder(enc).unwrap();
    out
}

/// A small colour PNG.
pub fn png(width: u32, height: u32, seed: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x * 5 + seed) as u8, (y * 3) as u8, (seed * 17) as u8])
    });
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

/// A ZIP of the given entries, in order. `stored` leaves every entry uncompressed.
pub fn zip(entries: &[(&str, Vec<u8>)], stored: bool) -> Vec<u8> {
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let method = if stored { CompressionMethod::Stored } else { CompressionMethod::Deflated };
    for (name, data) in entries {
        let opts = SimpleFileOptions::default().compression_method(method);
        z.start_file(*name, opts).unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap().into_inner()
}

pub const COMICINFO: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ComicInfo>
  <Series>The Lantern Keepers</Series>
  <Number>7</Number>
  <Volume>2</Volume>
  <Manga>YesAndRightToLeft</Manga>
  <LanguageISO>en</LanguageISO>
  <Pages>
    <Page Image="0" Type="FrontCover"/>
    <Page Image="3" DoublePage="true"/>
  </Pages>
</ComicInfo>"#;

/// Pages named so that a plain string sort would put them in the wrong order.
pub fn page_set(n: u32) -> Vec<(String, Vec<u8>)> {
    (1..=n)
        .map(|i| {
            if i == 2 {
                (format!("chapter/p{i}.png"), png(24, 32, i))
            } else {
                (format!("chapter/p{i}.jpg"), jpeg(24, 32, i))
            }
        })
        .collect()
}

/// A CBZ of `n` pages, written in reverse so archive order differs from reading order, with
/// clutter beside the pages and, optionally, ComicInfo.xml.
pub fn cbz(n: u32, comicinfo: bool) -> Vec<u8> {
    let mut entries: Vec<(String, Vec<u8>)> = page_set(n).into_iter().rev().collect();
    entries.push(("__MACOSX/chapter/._p1.jpg".into(), vec![0; 16]));
    entries.push(("chapter/.DS_Store".into(), vec![0; 16]));
    entries.push(("chapter/notes.txt".into(), b"made-up notes".to_vec()));
    if comicinfo {
        entries.push(("ComicInfo.xml".into(), COMICINFO.as_bytes().to_vec()));
    }
    let refs: Vec<(&str, Vec<u8>)> =
        entries.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    zip(&refs, true)
}

/// A 7z of the entries, solid (one block) or one block per entry.
pub fn sevenz(entries: &[(String, Vec<u8>)], solid: bool) -> Vec<u8> {
    let mut w = sevenz_rust2::ArchiveWriter::new(Cursor::new(Vec::new())).unwrap();
    if solid {
        let meta: Vec<_> =
            entries.iter().map(|(n, _)| sevenz_rust2::ArchiveEntry::new_file(n)).collect();
        let readers: Vec<_> = entries
            .iter()
            .map(|(_, d)| sevenz_rust2::SourceReader::new(Cursor::new(d.clone())))
            .collect();
        w.push_archive_entries(meta, readers).unwrap();
    } else {
        for (n, d) in entries {
            w.push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file(n),
                Some(Cursor::new(d.clone())),
            )
            .unwrap();
        }
    }
    w.finish().unwrap().into_inner()
}

/// CRC-32 (IEEE), as ZIP, RAR and 7z use it.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn vint(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// One RAR 5 header: CRC32, size, then the body (type, flags and fields).
fn rar5_header(body: &[u8], out: &mut Vec<u8>) {
    let mut sized = Vec::new();
    vint(body.len() as u64, &mut sized);
    sized.extend_from_slice(body);
    out.extend_from_slice(&crc32(&sized).to_le_bytes());
    out.extend_from_slice(&sized);
}

/// A RAR 5 archive of stored (uncompressed) entries, written from the format's published
/// description. Storing is not compression, so no RAR compressor is involved.
pub fn rar5(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = b"Rar!\x1a\x07\x01\x00".to_vec();
    // Main archive header: type 1, no header flags, no archive flags.
    rar5_header(&[1, 0, 0], &mut out);
    for (name, data) in entries {
        let mut body = Vec::new();
        vint(2, &mut body); // file header
        vint(0x0002, &mut body); // a data area follows
        vint(data.len() as u64, &mut body); // data size
        vint(0x0004, &mut body); // file flags: data CRC32 present
        vint(data.len() as u64, &mut body); // unpacked size
        vint(0x20, &mut body); // attributes: archive
        body.extend_from_slice(&crc32(data).to_le_bytes());
        vint(0, &mut body); // compression: version 0, stored, smallest dictionary
        vint(1, &mut body); // host OS: Unix
        vint(name.len() as u64, &mut body);
        body.extend_from_slice(name.as_bytes());
        rar5_header(&body, &mut out);
        out.extend_from_slice(data);
    }
    // End of archive: type 5, no flags, last volume.
    rar5_header(&[5, 0, 0], &mut out);
    out
}

/// A RAR 4 (1.5 to 4.x family) archive of stored entries.
pub fn rar4(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    fn block(rest: &[u8], out: &mut Vec<u8>) {
        let crc = (crc32(rest) & 0xFFFF) as u16;
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(rest);
    }
    let mut out = b"Rar!\x1a\x07\x00".to_vec();
    // Archive header: type 0x73, flags 0, size 13, reserved.
    let mut main = vec![0x73];
    main.extend_from_slice(&0u16.to_le_bytes());
    main.extend_from_slice(&13u16.to_le_bytes());
    main.extend_from_slice(&[0; 6]);
    block(&main, &mut out);
    for (name, data) in entries {
        let mut h = vec![0x74];
        h.extend_from_slice(&0x8000u16.to_le_bytes()); // long block: data follows
        h.extend_from_slice(&((32 + name.len()) as u16).to_le_bytes());
        h.extend_from_slice(&(data.len() as u32).to_le_bytes()); // packed size
        h.extend_from_slice(&(data.len() as u32).to_le_bytes()); // unpacked size
        h.push(3); // host OS: Unix
        h.extend_from_slice(&crc32(data).to_le_bytes());
        h.extend_from_slice(&0x5A21_0000u32.to_le_bytes()); // DOS time
        h.push(29); // version needed: 2.9
        h.push(0x30); // method: store
        h.extend_from_slice(&(name.len() as u16).to_le_bytes());
        h.extend_from_slice(&0o100644u32.to_le_bytes());
        h.extend_from_slice(name.as_bytes());
        block(&h, &mut out);
        out.extend_from_slice(data);
    }
    let mut end = vec![0x7B];
    end.extend_from_slice(&0x4000u16.to_le_bytes());
    end.extend_from_slice(&7u16.to_le_bytes());
    block(&end, &mut out);
    out
}

pub const UUID: &str = "urn:uuid:3f2a5c1e-9b7d-4e2a-8c61-0d4f5e6a7b8c";

/// A font-shaped blob: an OpenType tag and a pattern, long enough for obfuscation to matter.
pub fn fake_font() -> Vec<u8> {
    let mut f = b"OTTO".to_vec();
    f.extend((0..3000u32).map(|i| (i % 253) as u8));
    f
}

/// The IDPF obfuscation of a font for the given identifier.
pub fn idpf_obfuscate(font: &[u8], identifier: &str) -> Vec<u8> {
    use sha1::{Digest, Sha1};
    let key = Sha1::digest(identifier.as_bytes());
    let mut out = font.to_vec();
    for (i, b) in out.iter_mut().take(1040).enumerate() {
        *b ^= key[i % 20];
    }
    out
}

pub fn container(opf: &str) -> Vec<u8> {
    format!(
        r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="{opf}" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#
    )
    .into_bytes()
}

fn chapter(title: &str, body: &str) -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="en">
<head><title>{title}</title><link rel="stylesheet" href="../styles/book.css"/></head>
<body>{body}</body>
</html>"#
    )
    .into_bytes()
}

/// An EPUB 3 novel: nav document and NCX, a series collection, a cover, an obfuscated font, a
/// non-linear notes page, and a chapter carrying a script and an event handler that nothing in
/// this crate runs.
pub fn epub_novel() -> Vec<u8> {
    let opf = format!(
        r##"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid" xml:lang="en">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">{UUID}</dc:identifier>
    <dc:identifier>isbn-like-0000</dc:identifier>
    <dc:title>The Clockwork Orchard</dc:title>
    <dc:creator id="a1">Imogen Vale</dc:creator>
    <meta refines="#a1" property="role" scheme="marc:relators">aut</meta>
    <dc:language>en</dc:language>
    <dc:publisher>Lantern Press</dc:publisher>
    <meta property="belongs-to-collection" id="c1">Orchard Tales</meta>
    <meta refines="#c1" property="collection-type">series</meta>
    <meta refines="#c1" property="group-position">2</meta>
    <meta property="dcterms:modified">2026-01-01T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="cover" href="images/cover.png" media-type="image/png" properties="cover-image"/>
    <item id="css" href="styles/book.css" media-type="text/css"/>
    <item id="font" href="fonts/serif.otf" media-type="font/otf"/>
    <item id="c1" href="text/ch1.xhtml" media-type="application/xhtml+xml" properties="scripted"/>
    <item id="c2" href="text/ch%202.xhtml" media-type="application/xhtml+xml"/>
    <item id="notes" href="text/notes.xhtml" media-type="application/xhtml+xml"/>
    <item id="far" href="../../outside.css" media-type="text/css"/>
    <item id="web" href="https://cdn.example/remote.css" media-type="text/css"/>
  </manifest>
  <spine toc="ncx" page-progression-direction="ltr">
    <itemref idref="c1"/>
    <itemref idref="c2"/>
    <itemref idref="notes" linear="no"/>
  </spine>
</package>"##
    );
    let nav = r##"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Contents</title></head>
<body>
  <nav epub:type="toc"><ol>
    <li><a href="text/ch1.xhtml">One&nbsp;Morning</a>
      <ol><li><a href="text/ch1.xhtml#s2">The Gate</a></li></ol></li>
    <li><a href="text/ch%202.xhtml">Two</a></li>
    <li><span>Extras</span><ol><li><a href="text/notes.xhtml">Notes</a></li></ol></li>
  </ol></nav>
  <nav epub:type="page-list"><ol>
    <li><a href="text/ch1.xhtml#p1">1</a></li>
    <li><a href="text/ch%202.xhtml#p2">2</a></li>
  </ol></nav>
</body></html>"##;
    let ncx = r#"<?xml version="1.0" encoding="utf-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <navMap><navPoint id="n1"><navLabel><text>From the NCX</text></navLabel><content src="text/ch1.xhtml"/></navPoint></navMap>
</ncx>"#;
    let encryption = r#"<?xml version="1.0"?>
<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#">
  <enc:EncryptedData>
    <enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/>
    <enc:CipherData><enc:CipherReference URI="OEBPS/fonts/serif.otf"/></enc:CipherData>
  </enc:EncryptedData>
</encryption>"#;
    let ch1 = chapter(
        "One Morning",
        r#"<h1 id="p1">One Morning</h1><p onclick="alert(1)">The orchard woke before the town did.</p>
<script>document.title = "ran";</script><h2 id="s2">The Gate</h2><p>A short walk to the gate.</p>"#,
    );
    let ch2 = chapter(
        "Two",
        r#"<h1 id="p2">Two</h1><p>Rain on the copper leaves.<a epub:type="noteref" href="notes.xhtml#n1">1</a></p>"#,
    );
    let notes = chapter(
        "Notes",
        r#"<aside id="n1" epub:type="footnote"><p>A made-up note.</p></aside>"#,
    );
    zip(
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("META-INF/container.xml", container("OEBPS/content.opf")),
            ("META-INF/encryption.xml", encryption.as_bytes().to_vec()),
            ("OEBPS/content.opf", opf.into_bytes()),
            ("OEBPS/nav.xhtml", nav.as_bytes().to_vec()),
            ("OEBPS/toc.ncx", ncx.as_bytes().to_vec()),
            ("OEBPS/images/cover.png", png(20, 30, 1)),
            ("OEBPS/styles/book.css", b"body { margin: 0 }".to_vec()),
            ("OEBPS/fonts/serif.otf", idpf_obfuscate(&fake_font(), UUID)),
            ("OEBPS/text/ch1.xhtml", ch1),
            ("OEBPS/text/ch 2.xhtml", ch2),
            ("OEBPS/text/notes.xhtml", notes),
        ],
        false,
    )
}

/// An EPUB 2 book with only an NCX, a DOCTYPE in every file, Calibre's series tags and an EPUB 2
/// cover.
pub fn epub2() -> Vec<u8> {
    let opf = r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE package>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Salt and Signal</dc:title>
    <dc:creator opf:role="aut">Tomas Reed</dc:creator>
    <dc:identifier id="bookid">made-up-0001</dc:identifier>
    <dc:language>en</dc:language>
    <meta name="calibre:series" content="Harbour Lights"/>
    <meta name="calibre:series_index" content="3"/>
    <meta name="cover" content="cov"/>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="cov" href="cover.jpg" media-type="image/jpeg"/>
    <item id="t1" href="t1.html" media-type="application/xhtml+xml"/>
  </manifest>
  <spine toc="ncx"><itemref idref="t1"/></spine>
</package>"#;
    let ncx = r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd">
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <navMap>
    <navPoint id="a" playOrder="1"><navLabel><text>Departure</text></navLabel><content src="t1.html#top"/>
      <navPoint id="b" playOrder="2"><navLabel><text>The Pier</text></navLabel><content src="t1.html#pier"/></navPoint>
    </navPoint>
  </navMap>
</ncx>"#;
    zip(
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("META-INF/container.xml", container("content.opf")),
            ("content.opf", opf.as_bytes().to_vec()),
            ("toc.ncx", ncx.as_bytes().to_vec()),
            ("cover.jpg", jpeg(20, 30, 2)),
            (
                "t1.html",
                chapter(
                    "Departure",
                    "<p id=\"top\">The boat left at dawn.</p><p id=\"pier\">The pier.</p>",
                ),
            ),
        ],
        false,
    )
}

/// A fixed-layout, right-to-left EPUB of `n` image pages: SVG wrappers, `<img>` pages and one
/// spine item that is the image itself.
pub fn epub_manga(n: u32) -> Vec<u8> {
    let mut manifest = String::new();
    let mut spine = String::new();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 1..=n {
        let img = format!("images/p{i:03}.jpg");
        files.push((format!("item/{img}"), jpeg(30, 40, i)));
        manifest += &format!(r#"<item id="i{i}" href="{img}" media-type="image/jpeg"/>"#);
        let side = if i % 2 == 1 { "page-spread-right" } else { "page-spread-left" };
        if i == n {
            // EPUB 3 lets a spine point at an image directly.
            spine += &format!(r#"<itemref idref="i{i}" properties="{side}"/>"#);
            continue;
        }
        let page = if i % 2 == 0 {
            format!(
                r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>p</title><meta name="viewport" content="width=30, height=40"/><style>body{{margin:0}}</style></head><body><div><img src="../{img}" alt=""/></div></body></html>"#
            )
        } else {
            format!(
                r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>p</title></head><body><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 30 40"><image width="30" height="40" xlink:href="../{img}"/></svg></body></html>"#
            )
        };
        files.push((format!("item/xhtml/p{i:03}.xhtml"), page.into_bytes()));
        manifest += &format!(
            r#"<item id="x{i}" href="xhtml/p{i:03}.xhtml" media-type="application/xhtml+xml"/>"#
        );
        spine += &format!(r#"<itemref idref="x{i}" properties="{side}"/>"#);
    }
    let opf = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">made-up-manga-01</dc:identifier>
    <dc:title>Moth Lantern, Volume 1</dc:title>
    <dc:language>ja</dc:language>
    <meta property="rendition:layout">pre-paginated</meta>
    <meta property="rendition:spread">landscape</meta>
  </metadata>
  <manifest>{manifest}</manifest>
  <spine page-progression-direction="rtl">{spine}</spine>
</package>"#
    );
    let mut entries: Vec<(String, Vec<u8>)> = vec![
        ("mimetype".into(), b"application/epub+zip".to_vec()),
        ("META-INF/container.xml".into(), container("item/standard.opf")),
        ("item/standard.opf".into(), opf.into_bytes()),
    ];
    entries.extend(files);
    let refs: Vec<(&str, Vec<u8>)> =
        entries.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    zip(&refs, true)
}

/// Writes a PDF from its object bodies (object 1 first), with a cross-reference table.
pub fn pdf(objects: &[Vec<u8>], root: usize, info: Option<usize>) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    let info = info.map(|i| format!(" /Info {i} 0 R")).unwrap_or_default();
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root {root} 0 R{info} >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut v = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    v.extend_from_slice(data);
    v.extend_from_slice(b"\nendstream");
    v
}

/// A PDF whose pages are each one full-page greyscale JPEG, like a scanned comic.
pub fn pdf_of_jpegs(pages: &[Vec<u8>], width: u32, height: u32) -> Vec<u8> {
    jpeg_pdf(pages, width, height, Wrap::None)
}

/// How the JPEGs in [`pdf_of_wrapped_jpegs`] are wrapped in a text filter.
#[derive(Clone, Copy)]
pub enum Wrap {
    None,
    Ascii85,
    Hex,
}

/// Like [`pdf_of_jpegs`], with each JPEG behind ASCII85 or hex, as some scanners write them.
pub fn pdf_of_wrapped_jpegs(pages: &[Vec<u8>], width: u32, height: u32, wrap: Wrap) -> Vec<u8> {
    jpeg_pdf(pages, width, height, wrap)
}

fn ascii85(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in data.chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        let mut v = u32::from_be_bytes(word);
        if chunk.len() == 4 && v == 0 {
            out.push(b'z');
            continue;
        }
        let mut digits = [0u8; 5];
        for d in digits.iter_mut().rev() {
            *d = (v % 85) as u8 + b'!';
            v /= 85;
        }
        out.extend_from_slice(&digits[..chunk.len() + 1]);
    }
    out.extend_from_slice(b"~>");
    out
}

fn jpeg_pdf(pages: &[Vec<u8>], width: u32, height: u32, wrap: Wrap) -> Vec<u8> {
    let n = pages.len();
    // 1 catalog, 2 pages, 3 info, then per page: page, content, image.
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        Vec::new(),
        b"<< /Title (The Paper Crane Almanac) /Author (Made-up Studio) >>".to_vec(),
    ];
    let mut kids = String::new();
    for (i, jpeg) in pages.iter().enumerate() {
        let page = 4 + i * 3;
        kids += &format!("{page} 0 R ");
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Contents {} 0 R /Resources << /XObject << /Im0 {} 0 R >> >> >>",
                page + 1,
                page + 2
            )
            .into_bytes(),
        );
        objects.push(stream("", format!("q {width} 0 0 {height} 0 0 cm /Im0 Do Q").as_bytes()));
        let (filter, data) = match wrap {
            Wrap::None => ("/DCTDecode", jpeg.clone()),
            Wrap::Ascii85 => ("[/ASCII85Decode /DCTDecode]", ascii85(jpeg)),
            Wrap::Hex => (
                "[/AHx /DCT]",
                jpeg.iter().map(|b| format!("{b:02X}\n")).collect::<String>().into_bytes(),
            ),
        };
        objects.push(stream(
            &format!(
                "/Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter {filter}"
            ),
            &data,
        ));
    }
    objects[1] = format!("<< /Type /Pages /Kids [{kids}] /Count {n} >>").into_bytes();
    pdf(&objects, 1, Some(3))
}

/// A one-page PDF drawn with vectors: a red square on a white 200 by 200 page.
pub fn pdf_vector() -> Vec<u8> {
    let objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R >>".to_vec(),
        stream("", b"1 0 0 rg 20 20 100 100 re f"),
    ];
    pdf(&objects, 1, None)
}
