//! Novels in a web view: the book's documents cleaned for serving, soft hyphens for the
//! engines that need them, and the one paginator script and stylesheet every platform injects.
//!
//! The engine serves a book's files to the platform web view through a custom scheme
//! (`kaeri-book://<session>/<path>`, or `https://appassets.androidplatform.net/book/...` on
//! Android). Every document goes through [`document`] (or [`text_chapter`] for a source's text
//! chapter), every stylesheet through [`stylesheet`], every SVG through [`svg`], and every
//! response carries [`csp`]. The paginator ([`script`]) lays the pages out and reports
//! positions; its messages and calls are described at the top of `web/paginator.js`.

mod css;
mod hyphen;
mod sanitize;

pub use hyphen::{BUILT_IN as HYPHENATES, SHY};
pub use sanitize::{Clean, Document, Failed, READER_CSS, document, stylesheet, svg, text_chapter};

/// The paginator as written, with a placeholder where [`script`] puts the stylesheet.
const PAGINATOR: &str = include_str!("../../web/paginator.js");

/// Kaeri's novel stylesheet. The paginator carries it; hosts that inject a stylesheet
/// separately can use this.
pub const STYLESHEET: &str = include_str!("../../web/reader.css");

/// The book origins every platform serves from, for the content security policy.
pub const ORIGINS: &str = "kaeri-book: https://appassets.androidplatform.net";

/// The paginator script with the stylesheet inside, ready to inject as one user script.
pub fn script() -> String {
    PAGINATOR.replacen("\"__KAERI_CSS__\"", &js_string(STYLESHEET), 1)
}

/// The content security policy every book response carries (S3): no scripts at all, and
/// styles, images, fonts and media only from the book (or inline images and fonts).
pub fn csp() -> String {
    let book = ORIGINS;
    format!(
        "default-src 'none'; script-src 'none'; style-src {book} 'unsafe-inline'; \
         img-src {book} data:; font-src {book} data:; media-src {book}; object-src 'none'; \
         frame-src 'none'; child-src 'none'; worker-src 'none'; connect-src 'none'; \
         manifest-src 'none'; base-uri 'none'; form-action 'none'"
    )
}

/// The direction lines run in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Horizontal,
    VerticalRl,
    VerticalLr,
}

/// The vertical writing mode a stylesheet sets anywhere, if any.
pub fn vertical_flow(css: &str) -> Option<Flow> {
    css::vertical_mode(css)
}

/// A string as a JavaScript (and JSON) string literal.
pub fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Line and paragraph separators end a line in older JavaScript; `<` keeps a
            // `</script>` from closing an inline script.
            '\u{2028}' | '\u{2029}' | '<' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_carries_the_stylesheet() {
        let s = script();
        assert!(!s.contains("__KAERI_CSS__"));
        assert!(s.contains("html.kaeri.k-paged"));
        assert!(s.contains("window.kaeriReader"));
    }

    #[test]
    fn the_policy_blocks_scripts_and_names_both_origins() {
        let p = csp();
        assert!(p.contains("script-src 'none'"));
        assert!(p.contains("img-src kaeri-book: https://appassets.androidplatform.net data:"));
        assert!(!p.contains("  "));
    }

    #[test]
    fn strings_escape_for_javascript() {
        assert_eq!(js_string("a\"b\\c\n</s>\u{2028}"), "\"a\\\"b\\\\c\\n\\u003c/s>\\u2028\"");
    }
}
