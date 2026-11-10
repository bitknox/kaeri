//! Pictures a source's text chapter links on the web. A source names an illustration as
//! `<img src="https://…">` on one of its own hosts; the engine fetches it as it fetches page
//! images and keeps it inside the book, and [`with_local_images`] points the chapter at the kept
//! copy. A link the engine did not fetch is left as it was, and the sanitiser drops it.

use std::collections::HashMap;

/// The most pictures one chapter brings in.
pub const MOST_WEB_IMAGES: usize = 64;

/// The `https` addresses in the chapter's `<img src>`, in order and each once, at most
/// [`MOST_WEB_IMAGES`].
pub fn web_images(fragment: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (_, url) in img_sources(fragment) {
        if is_https(&url) && !out.contains(&url) {
            out.push(url);
            if out.len() == MOST_WEB_IMAGES {
                break;
            }
        }
    }
    out
}

/// The chapter with each `<img src>` found in `local` (address to path in the book) pointing at
/// that path instead.
pub fn with_local_images(fragment: &str, local: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(fragment.len());
    let mut from = 0;
    for (span, url) in img_sources(fragment) {
        let Some(path) = local.get(&url) else { continue };
        out.push_str(&fragment[from..span.start]);
        out.push('"');
        out.push_str(&escape_attr(path));
        out.push('"');
        from = span.end;
    }
    out.push_str(&fragment[from..]);
    out
}

fn is_https(url: &str) -> bool {
    url.get(..8).is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

/// Each `<img>`'s `src`: where its value sits, quotes included, and the value with character
/// references decoded.
fn img_sources(text: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(at) = text[i..].find('<') {
        let start = i + at + 1;
        i = start;
        let name_end = text[start..]
            .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
            .map_or(text.len(), |n| start + n);
        if !text[start..name_end].eq_ignore_ascii_case("img") {
            continue;
        }
        // The attributes, up to the `>` outside quotes.
        let mut j = name_end;
        while j < bytes.len() && bytes[j] != b'>' {
            if bytes[j].is_ascii_whitespace() || bytes[j] == b'/' {
                j += 1;
                continue;
            }
            let name_start = j;
            while j < bytes.len()
                && !bytes[j].is_ascii_whitespace()
                && !matches!(bytes[j], b'=' | b'>' | b'/')
            {
                j += 1;
            }
            let name = &text[name_start..j];
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if bytes.get(j) != Some(&b'=') {
                continue;
            }
            j += 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let value_start = j;
            let (value, value_end) = match bytes.get(j) {
                Some(&quote @ (b'"' | b'\'')) => {
                    let Some(close) = text[j + 1..].find(quote as char) else { break };
                    let end = j + 1 + close;
                    (&text[j + 1..end], end + 1)
                }
                _ => {
                    while j < bytes.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'>'
                    {
                        j += 1;
                    }
                    (&text[value_start..j], j)
                }
            };
            j = value_end;
            if name.eq_ignore_ascii_case("src") {
                out.push((value_start..value_end, decode(value.trim())));
            }
        }
        i = j.min(bytes.len());
    }
    out
}

/// The references an address in markup may carry: the five named ones and numeric ones.
fn decode(value: &str) -> String {
    if !value.contains('&') {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let decoded = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => name.strip_prefix('#').and_then(|n| {
                match n.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => n.parse().ok(),
                }
                .and_then(char::from_u32)
            }),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn escape_attr(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHAPTER: &str = "<p>The ferry left at dusk.</p>\
        <img alt=\"The pier\" src=\"https://img.books.example/a/1.jpg?w=800&amp;q=9\"/>\
        <p>Gulls.</p><IMG SRC='https://img.books.example/a/2.png'>\
        <img src=\"https://img.books.example/a/1.jpg?w=800&#38;q=9\"/>\
        <img src=\"http://img.books.example/a/3.jpg\"/><img src=\"data:image/png;base64,AAAA\"/>\
        <img src=images/4.jpg><imgx src=\"https://img.books.example/a/5.jpg\"/>";

    #[test]
    fn a_chapters_web_pictures_are_its_https_img_sources_each_once() {
        assert_eq!(
            web_images(CHAPTER),
            [
                "https://img.books.example/a/1.jpg?w=800&q=9",
                "https://img.books.example/a/2.png"
            ]
        );
        assert!(web_images("<p>No pictures.</p><p>a < b</p>").is_empty());
    }

    #[test]
    fn kept_pictures_take_the_place_of_their_addresses_and_the_rest_stay() {
        let local = HashMap::from([(
            "https://img.books.example/a/1.jpg?w=800&q=9".to_owned(),
            "images/1.jpg".to_owned(),
        )]);
        let out = with_local_images(CHAPTER, &local);
        assert_eq!(out.matches("src=\"images/1.jpg\"").count(), 2, "{out}");
        assert!(out.contains("SRC='https://img.books.example/a/2.png'"), "{out}");
        assert!(out.contains("alt=\"The pier\""), "{out}");
        assert!(out.contains("<img src=images/4.jpg>"), "{out}");
        assert_eq!(with_local_images(CHAPTER, &HashMap::new()), CHAPTER);
    }

    #[test]
    fn a_chapter_names_no_more_than_the_most_pictures() {
        let many: String = (0..100)
            .map(|n| format!("<img src=\"https://img.books.example/{n}.jpg\"/>"))
            .collect();
        assert_eq!(web_images(&many).len(), MOST_WEB_IMAGES);
    }

    #[test]
    fn markup_that_breaks_off_finds_what_it_can() {
        assert!(web_images("<img src=\"https://img.books.example/1.jpg").is_empty());
        assert!(web_images("<img").is_empty());
        assert_eq!(web_images("<img src=https://img.books.example/1.jpg>").len(), 1);
    }
}
