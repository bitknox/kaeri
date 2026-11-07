//! A server's address, as a person types it.

/// The addresses to try for what was typed, best first. A typed address without a scheme is tried
/// as https and then as http, because a server at home often has no certificate.
pub fn candidates(typed: &str) -> Vec<String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Vec::new();
    }
    let without_fragment = typed.split(['#', '?']).next().unwrap_or(typed);
    let trimmed = without_fragment.trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return vec![trimmed.to_owned()];
    }
    vec![format!("https://{trimmed}"), format!("http://{trimmed}")]
}

/// The server's root: the address with the part that names a catalogue or an API cut off, for
/// kinds whose API lives at the root behind a reverse proxy's path.
pub fn root_of(address: &str) -> String {
    for marker in ["/opds", "/api/"] {
        if let Some(at) = address.find(marker) {
            return address[..at].to_owned();
        }
    }
    address.trim_end_matches('/').to_owned()
}

/// Whether the address names an OPDS catalogue by its path.
pub fn looks_like_opds(address: &str) -> bool {
    address.to_ascii_lowercase().contains("/opds")
}

/// A name to show for a server that gave none: its host.
pub fn host_of(address: &str) -> String {
    url::Url::parse(address)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| address.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_host_is_tried_secure_first() {
        assert_eq!(
            candidates("shelf.example"),
            ["https://shelf.example", "http://shelf.example"]
        );
        assert_eq!(candidates("  http://192.0.2.7:25600/ "), ["http://192.0.2.7:25600"]);
        assert_eq!(
            candidates("https://shelf.example/books?x=1#y"),
            ["https://shelf.example/books"]
        );
        assert!(candidates("   ").is_empty());
    }

    #[test]
    fn the_root_leaves_out_a_catalogue_path() {
        assert_eq!(
            root_of("http://h.example:25600/opds/v1.2/catalog"),
            "http://h.example:25600"
        );
        assert_eq!(root_of("https://h.example/komga/"), "https://h.example/komga");
        assert!(looks_like_opds("http://h.example/OPDS/v1.2"));
        assert_eq!(host_of("http://h.example:25600/x"), "h.example");
    }
}
