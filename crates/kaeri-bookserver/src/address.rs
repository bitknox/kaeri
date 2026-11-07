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

/// A Kavita account's OPDS link, `https://host/api/opds/{apiKey}`, taken apart: the server's root
/// and the account's API key. Kavita shows this link on the account's settings page, and the key
/// in it signs in on its own, so a person who pastes it needs no password.
#[derive(Clone, PartialEq, Eq)]
pub struct KavitaLink {
    /// The server's root: the link with `/api/opds/...` cut off, as [`root_of`] gives it.
    pub root: String,
    /// The account's API key. A secret: it goes to the secure store and is shown only as
    /// [`last_four`].
    pub key: String,
}

impl std::fmt::Debug for KavitaLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the key: it signs in.
        f.debug_struct("KavitaLink")
            .field("root", &self.root)
            .field("key", &format_args!("…{}", last_four(&self.key)))
            .finish()
    }
}

/// Recognise a pasted Kavita OPDS link. The path may sit under a reverse proxy's prefix, and a
/// link without a scheme is taken as https. Anything else, an OPDS address of another kind
/// included, is `None`.
pub fn kavita_link(typed: &str) -> Option<KavitaLink> {
    let typed = typed.trim();
    let with_scheme = if typed.starts_with("http://") || typed.starts_with("https://") {
        typed.to_owned()
    } else {
        format!("https://{typed}")
    };
    let url = url::Url::parse(&with_scheme).ok()?;
    url.host_str()?;
    if !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let segments: Vec<&str> = url.path_segments()?.collect();
    let at = segments.windows(2).position(|pair| {
        pair[0].eq_ignore_ascii_case("api") && pair[1].eq_ignore_ascii_case("opds")
    })?;
    let key = segments.get(at + 2).copied().unwrap_or_default();
    let fits = key.len() >= 8
        && key.len() <= 128
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !fits {
        return None;
    }
    let mut root = format!("{}://{}", url.scheme(), url.host_str()?);
    if let Some(port) = url.port() {
        root.push_str(&format!(":{port}"));
    }
    for segment in &segments[..at] {
        if !segment.is_empty() {
            root.push('/');
            root.push_str(segment);
        }
    }
    Some(KavitaLink { root, key: key.to_owned() })
}

/// The last four characters of a key, which is all of it a screen shows.
pub fn last_four(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    chars[chars.len().saturating_sub(4)..].iter().collect()
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

    #[test]
    fn a_kavita_opds_link_gives_its_root_and_key() {
        let link = kavita_link(
            "https://books.home.example/api/opds/0f1e2d3c-4b5a-6978-8a9b-acbdcedf1234",
        )
        .unwrap();
        assert_eq!(link.root, "https://books.home.example");
        assert_eq!(link.key, "0f1e2d3c-4b5a-6978-8a9b-acbdcedf1234");
        assert_eq!(last_four(&link.key), "1234");
        // Under a proxy's path, with a port, and with what Kavita adds after the key.
        let link = kavita_link(" http://192.0.2.7:5000/kavita/api/opds/abcdef123456/catalog ")
            .unwrap();
        assert_eq!(link.root, "http://192.0.2.7:5000/kavita");
        assert_eq!(link.key, "abcdef123456");
        // No scheme: https.
        assert_eq!(
            kavita_link("books.example/api/opds/abcdef123456").unwrap().root,
            "https://books.example"
        );
        // The key never shows in a debug print.
        assert!(!format!("{link:?}").contains("abcdef12"));
    }

    #[test]
    fn other_addresses_are_not_kavita_links() {
        for typed in [
            "https://books.example",
            "https://books.example/opds/v1.2/catalog",
            "https://books.example/api/opds/",
            "https://books.example/api/opds/short",
            "https://books.example/api/opds/has%20space1",
            "https://user:pw@books.example/api/opds/abcdef123456",
            "",
        ] {
            assert!(kavita_link(typed).is_none(), "{typed}");
        }
        assert_eq!(last_four("ab"), "ab");
    }
}
