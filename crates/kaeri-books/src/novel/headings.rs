//! A source's text chapter often opens with its title twice: a heading the source's plugin puts
//! first, then the site's own heading with the same words. [`heading_once`] keeps the first and
//! hides the ones after it that say the same, and lets the caller give the first its chapter's
//! cleaned title.
//!
//! A repeat is hidden, not removed: a place kept in the chapter names its elements by where they
//! stand, so the chapter's elements all stay where they were.

use roxmltree::Node;

use crate::write::escape_text;
use crate::write::xhtml::XHTML_NS;
use crate::{Format, Limits, xml};

/// Looked at before the text starts: a chapter's title and a subtitle or two.
const MOST_HEADINGS: usize = 4;
/// How deep wrappers around the opening headings are looked into.
const MOST_DEPTH: usize = 4;
/// A bold paragraph longer than this is prose, not a title.
const MOST_TITLE_CHARS: usize = 200;

/// Wrappers that hold the chapter rather than say anything themselves.
const WRAPPERS: &[&str] = &["div", "section", "article", "main", "header", "hgroup"];

/// The chapter's body with its opening headings said once. Every heading before the text starts
/// that repeats one above it gets `hidden`; the first is given what `retitle` makes of its text,
/// when that is something else (the chapter's title cleaned, say). A heading is an `h1` to
/// `h6`, or a short paragraph that is all bold. Headings compare by their letters and digits,
/// after `retitle` and without a leading "Chapter 12" or "Ch. 12", so case and punctuation do not
/// matter.
///
/// A whole document is looked at inside its `<body>`, and comes back whole. A body that does not
/// parse comes back as it is: the cleaning that follows reads it as well as it can.
pub fn heading_once(text: &str, retitle: &dyn Fn(&str) -> Option<String>) -> String {
    let body = super::sanitize::body_of(text).unwrap_or(text);
    let Some(once) =
        once_in(body, retitle).or_else(|| once_in(&crate::epub::html_entities(body), retitle))
    else {
        return text.to_owned();
    };
    // Where the body sits in the text: `body_of` gives a slice of it.
    let start = body.as_ptr() as usize - text.as_ptr() as usize;
    format!("{}{once}{}", &text[..start], &text[start + body.len()..])
}

fn once_in(body: &str, retitle: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let open = format!("<kaeri-chapter xmlns=\"{XHTML_NS}\">");
    let close = "</kaeri-chapter>";
    let text = format!("{open}{body}{close}");
    let doc = xml::parse(&text, Format::Epub, &Limits::default()).ok()?;
    let mut headings = Vec::new();
    opening(doc.root_element(), 0, &mut headings);
    let Some(first) = headings.first().copied() else {
        return Some(body.to_owned());
    };
    // Edits as (where, how much is replaced, with what), applied from the end back.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let shown = |n: Node<'_, '_>| {
        let words = words_of(n);
        retitle(&words).unwrap_or(words)
    };
    let mut kept = vec![key(&shown(first))];
    for &n in &headings[1..] {
        let k = key(&shown(n));
        if !k.is_empty() && kept.contains(&k) {
            edits.push((name_end(&text, n), 0, " hidden=\"hidden\"".to_owned()));
        } else {
            kept.push(k);
        }
    }
    let words = words_of(first);
    if let Some(new) = retitle(&words).filter(|t| !t.trim().is_empty() && *t != words)
        && let (Some(a), Some(b)) = (first.first_child(), first.last_child())
    {
        let (start, end) = (a.range().start, b.range().end);
        edits.push((start, end - start, escape_text(new.trim())));
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = text;
    for (at, len, with) in edits {
        out.replace_range(at..at + len, &with);
    }
    Some(out[open.len()..out.len() - close.len()].to_owned())
}

/// The headings at the chapter's start, in order, until something else says anything. False once
/// the text has started.
fn opening<'a, 'i>(parent: Node<'a, 'i>, depth: usize, out: &mut Vec<Node<'a, 'i>>) -> bool {
    for n in parent.children() {
        if out.len() >= MOST_HEADINGS {
            return false;
        }
        if n.is_text() {
            if n.text().is_some_and(|t| !t.trim().is_empty()) {
                return false;
            }
            continue;
        }
        if !n.is_element() {
            continue;
        }
        let name = n.tag_name().name().to_ascii_lowercase();
        if is_heading(n, &name) {
            out.push(n);
        } else if matches!(name.as_str(), "br" | "hr") || is_blank(n) {
            continue;
        } else if WRAPPERS.contains(&name.as_str()) && depth < MOST_DEPTH {
            if !opening(n, depth + 1, out) {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

fn is_heading(n: Node<'_, '_>, name: &str) -> bool {
    if matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
        return !words_of(n).is_empty();
    }
    if !matches!(name, "p" | "div") {
        return false;
    }
    // All bold: every piece of its text sits in a `b` or a `strong`, and nothing but text does.
    let words = words_of(n);
    if words.is_empty() || words.chars().count() > MOST_TITLE_CHARS {
        return false;
    }
    n.descendants().all(|d| {
        if d.is_element() {
            return matches!(
                d.tag_name().name().to_ascii_lowercase().as_str(),
                "p" | "div" | "b" | "strong" | "span" | "br"
            );
        }
        !d.is_text()
            || d.text().is_none_or(|t| t.trim().is_empty())
            || d.ancestors().take_while(|a| *a != n).any(|a| {
                a.is_element() && matches!(a.tag_name().name(), "b" | "strong" | "B" | "STRONG")
            })
    })
}

fn is_blank(n: Node<'_, '_>) -> bool {
    words_of(n).is_empty() && !n.descendants().any(|d| d.is_element() && is_picture(d))
}

fn is_picture(n: Node<'_, '_>) -> bool {
    matches!(
        n.tag_name().name().to_ascii_lowercase().as_str(),
        "img" | "svg" | "image" | "video"
    )
}

fn words_of(n: Node<'_, '_>) -> String {
    xml::trimmed_text(n).unwrap_or_default()
}

/// What a heading says, for comparing: its letters and digits in lower case, without a leading
/// "Chapter 12" or "Ch. 12".
fn key(words: &str) -> String {
    let plain: String =
        words.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
    for word in ["chapter", "ch"] {
        if let Some(rest) = plain.strip_prefix(word) {
            let after = rest.trim_start_matches(|c: char| c.is_ascii_digit());
            if after.len() < rest.len() && !after.is_empty() {
                return after.to_owned();
            }
        }
    }
    plain
}

/// Where an element's start tag's name ends: an attribute goes in there.
fn name_end(text: &str, n: Node<'_, '_>) -> usize {
    let start = n.range().start + 1;
    start + text[start..].find(|c: char| c.is_whitespace() || c == '/' || c == '>').unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn a_title_said_twice_is_said_once() {
        let out = heading_once(
            "<div><h2>Chapter 3: The Orchard Gate</h2> <h4>Chapter 3: the orchard gate</h4><p><i>Notes</i></p><p>The gate stood open.</p></div>",
            &none,
        );
        assert_eq!(
            out,
            "<div><h2>Chapter 3: The Orchard Gate</h2> <h4 hidden=\"hidden\">Chapter 3: the orchard gate</h4><p><i>Notes</i></p><p>The gate stood open.</p></div>"
        );
    }

    #[test]
    fn a_bold_paragraph_and_a_bare_title_are_repeats_too() {
        let out = heading_once(
            "<h1>Chapter 3: The Orchard Gate</h1><p><strong>The Orchard Gate</strong></p><p>Text.</p>",
            &none,
        );
        assert!(
            out.contains("<p hidden=\"hidden\"><strong>The Orchard Gate</strong></p>"),
            "{out}"
        );
    }

    #[test]
    fn the_first_heading_takes_the_cleaned_title() {
        let tidy = |t: &str| t.strip_suffix(" — 3").map(str::to_owned);
        let out = heading_once(
            "<div><h2>Chapter 3: The Orchard &amp; Gate — 3</h2><h4>Chapter 3: The Orchard &amp; Gate — 3</h4><p>Text.</p></div>",
            &tidy,
        );
        assert_eq!(
            out,
            "<div><h2>Chapter 3: The Orchard &amp; Gate</h2><h4 hidden=\"hidden\">Chapter 3: The Orchard &amp; Gate — 3</h4><p>Text.</p></div>"
        );
    }

    #[test]
    fn other_headings_and_later_ones_stay() {
        // A subtitle that says something else, and the title again once the text has started.
        let src = "<h2>Chapter 3</h2><h3>The Orchard Gate</h3><p>Text.</p><h3>Chapter 3</h3>";
        assert_eq!(heading_once(src, &none), src);
        // A bold line that is more than a title is text.
        let src = "<h2>Gate</h2><p><b>Gate</b> stood open.</p>";
        assert_eq!(heading_once(src, &none), src);
        // A picture first: the text has started.
        let src = "<p><img src=\"a.png\"/></p><h2>Gate</h2><h2>Gate</h2>";
        assert_eq!(heading_once(src, &none), src);
    }

    #[test]
    fn a_whole_page_is_looked_at_in_its_body_and_sloppy_markup_is_left_alone() {
        let out = heading_once(
            "<html><head><title>Gate One</title></head><body class=\"x\"><h2>Gate&nbsp;One</h2><h3>Gate One</h3><p>T</p></body></html>",
            &none,
        );
        assert_eq!(
            out,
            "<html><head><title>Gate One</title></head><body class=\"x\"><h2>Gate&#160;One</h2><h3 hidden=\"hidden\">Gate One</h3><p>T</p></body></html>"
        );
        let sloppy = "<h2>Gate</h2><h2>Gate</h2><p>Unclosed <br> text";
        assert_eq!(heading_once(sloppy, &none), sloppy);
    }
}
