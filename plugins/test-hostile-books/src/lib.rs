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
//! | `chapter-content` | Names page images on an undeclared host and reads a setting it was never granted: the host must refuse the pages and answer `none`. |
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
        Chapter, Content, Guest, Manifest, Page, SeriesDetail, SeriesHit, SourceError,
        TitleHints,
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
        Ok(Content::Images(vec![
            Page {
                url: "https://exfiltrate.example/page-1.jpg".into(),
                headers: vec![],
                width: None,
                height: None,
            },
            Page {
                // A reserved header: the host sets these itself.
                url: "https://allowed.example/page-2.jpg".into(),
                headers: vec![("host".into(), "exfiltrate.example".into())],
                width: None,
                height: None,
            },
        ]))
    }
}

bindings::export!(Component with_types_in bindings);
