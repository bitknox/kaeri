//! A web link the TV hands a phone to open there: a service's page the TV cannot open itself, as
//! when the service's app is not installed on the TV.
//!
//! Only an `https://` link crosses, checked on both sides: the TV refuses to send anything else,
//! and a phone drops anything else it is sent, so a link from the TV can never be a file, a
//! custom scheme or a page sent in the clear.

/// The longest link that crosses. A service's episode page is well under it.
pub const MAX_LINK: usize = 2048;

/// `text` as a link a phone may open, trimmed: `https://`, a host, no user name in front of the
/// host (which can make one host read as another), and no spaces or control characters. `None`
/// for anything else.
pub fn https_link(text: &str) -> Option<String> {
    let link = text.trim();
    if link.len() > MAX_LINK || link.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let scheme = link.get(..8)?;
    if !scheme.eq_ignore_ascii_case("https://") {
        return None;
    }
    let rest = &link[8..];
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if host.is_empty() || host.contains('@') || host.starts_with(':') {
        return None;
    }
    Some(link.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_https_link_passes_trimmed() {
        assert_eq!(
            https_link("  https://hikari.example/t/1?ep=4#play "),
            Some("https://hikari.example/t/1?ep=4#play".into())
        );
        assert_eq!(
            https_link("HTTPS://hikari.example:8443"),
            Some("HTTPS://hikari.example:8443".into())
        );
    }

    #[test]
    fn anything_else_is_refused() {
        for text in [
            "http://hikari.example/t/1",
            "hikari.example/t/1",
            "file:///etc/hosts",
            "intent://hikari.example#Intent;end",
            "javascript:alert(1)",
            "https://",
            "https:///t/1",
            "https://:443/t/1",
            "https://hikari.example@other.example/",
            "https://hikari.example/a b",
            "https://hikari.example/\u{0}",
            "",
        ] {
            assert_eq!(https_link(text), None, "{text:?}");
        }
        let long = format!("https://hikari.example/{}", "a".repeat(MAX_LINK));
        assert_eq!(https_link(&long), None);
    }
}
