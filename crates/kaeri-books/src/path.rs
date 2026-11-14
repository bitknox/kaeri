//! Names inside a book. Entry names and hrefs come from the file, so each is normalised into a
//! relative path with `/` separators, and anything that could point outside the book is
//! refused: absolute paths, drive letters, `..` above the root, NUL and control characters.
//! Callers that write pages to disk name the files themselves; an entry name never becomes a
//! file name.

/// Normalises an archive entry name. Returns `None` for a name that is empty, too long, too deep,
/// absolute, has a drive letter or a `..` component, or holds control characters.
pub fn entry_name(raw: &str, max_bytes: usize, max_depth: usize) -> Option<String> {
    if raw.is_empty() || raw.len() > max_bytes || raw.chars().any(char::is_control) {
        return None;
    }
    let s = raw.replace('\\', "/");
    if s.starts_with('/') || has_drive(&s) {
        return None;
    }
    let mut parts = Vec::new();
    for part in s.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            p => parts.push(p),
        }
    }
    if parts.is_empty() || parts.len() > max_depth {
        return None;
    }
    Some(parts.join("/"))
}

fn has_drive(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// The directory part of a normalised path, without the trailing slash; empty at the root.
pub fn parent(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

/// The last component of a normalised path.
pub fn file_name(path: &str) -> &str {
    path.rfind('/').map_or(path, |i| &path[i + 1..])
}

/// The lowercase extension of a path, without the dot.
pub fn extension(path: &str) -> Option<String> {
    let name = file_name(path);
    let dot = name.rfind('.')?;
    (dot > 0 && dot + 1 < name.len()).then(|| name[dot + 1..].to_ascii_lowercase())
}

/// A resolved reference inside the book: the normalised path and the fragment, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Href {
    pub path: String,
    pub fragment: Option<String>,
}

impl Href {
    /// `path#fragment`, or the path alone.
    pub fn to_url_path(&self) -> String {
        match &self.fragment {
            Some(f) => format!("{}#{f}", self.path),
            None => self.path.clone(),
        }
    }
}

/// Resolves a relative URL found in a document at `base` (a normalised path of a file in the
/// book) into a normalised path in the book. Returns `None` for a URL with a scheme (`http:`,
/// `data:`, `javascript:`), an absolute or protocol-relative one, one that climbs above the root,
/// or one that is not valid percent-encoded UTF-8. A fragment-only href resolves to `base`.
pub fn resolve(base: &str, href: &str, max_depth: usize) -> Option<Href> {
    let href = href.trim();
    let (rest, fragment) = match href.split_once('#') {
        Some((p, f)) => (p, Some(percent_decode(f)?)),
        None => (href, None),
    };
    let rest = rest.split_once('?').map_or(rest, |(p, _)| p);
    if has_scheme(rest) || rest.starts_with('/') || rest.starts_with('\\') {
        return None;
    }
    let decoded = percent_decode(rest)?;
    if decoded.chars().any(char::is_control) || decoded.contains('\\') {
        return None;
    }
    let mut parts: Vec<&str> = if decoded.is_empty() {
        // `#note-3` points into the document itself.
        base.split('/').filter(|p| !p.is_empty()).collect()
    } else {
        parent(base).split('/').filter(|p| !p.is_empty()).collect()
    };
    if !decoded.is_empty() {
        for part in decoded.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop()?;
                }
                p => parts.push(p),
            }
        }
    }
    if parts.is_empty() || parts.len() > max_depth {
        return None;
    }
    Some(Href { path: parts.join("/"), fragment: fragment.filter(|f| !f.is_empty()) })
}

/// True when the URL starts with `scheme:` (letters, digits, `+`, `-`, `.` before a colon that
/// comes before any slash).
fn has_scheme(s: &str) -> bool {
    let Some(colon) = s.find(':') else {
        return false;
    };
    let scheme = &s[..colon];
    if s[..colon].contains('/') {
        return false;
    }
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Decodes `%xx` escapes. Returns `None` for a bad escape or bytes that are not UTF-8.
pub fn percent_decode(s: &str) -> Option<String> {
    if !s.contains('%') {
        return Some(s.to_owned());
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hi = hex(*b.get(i + 1)?)?;
            let lo = hex(*b.get(i + 2)?)?;
            out.push(hi << 4 | lo);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> Option<String> {
        entry_name(s, 1024, 32)
    }

    #[test]
    fn entry_names_are_normalised() {
        assert_eq!(name("a/b.jpg").as_deref(), Some("a/b.jpg"));
        assert_eq!(name("./a//b.jpg").as_deref(), Some("a/b.jpg"));
        assert_eq!(name("a\\b.jpg").as_deref(), Some("a/b.jpg"));
        assert_eq!(name("a/").as_deref(), Some("a"));
    }

    #[test]
    fn entry_names_that_leave_the_book_are_refused() {
        for bad in [
            "",
            "/etc/passwd",
            "\\server\\share",
            "../a.jpg",
            "a/../../b.jpg",
            "a/../b.jpg",
            "C:\\Windows\\a.jpg",
            "c:a.jpg",
            "a\0b.jpg",
            "a\nb.jpg",
            "./",
        ] {
            assert_eq!(name(bad), None, "{bad:?}");
        }
        assert_eq!(entry_name(&"a/".repeat(40), 1024, 32), None);
        assert_eq!(entry_name(&"a".repeat(2000), 1024, 32), None);
    }

    #[test]
    fn hrefs_resolve_inside_the_book() {
        let r = |base, href| resolve(base, href, 32).map(|h| h.to_url_path());
        assert_eq!(
            r("OEBPS/text/c1.xhtml", "../img/p%201.jpg").as_deref(),
            Some("OEBPS/img/p 1.jpg")
        );
        assert_eq!(
            r("OEBPS/text/c1.xhtml", "c2.xhtml#n3").as_deref(),
            Some("OEBPS/text/c2.xhtml#n3")
        );
        assert_eq!(r("OEBPS/text/c1.xhtml", "#n3").as_deref(), Some("OEBPS/text/c1.xhtml#n3"));
        assert_eq!(r("c1.xhtml", "img/a.png?x=1").as_deref(), Some("img/a.png"));
        assert_eq!(r("OEBPS/c.xhtml", "../a.css").as_deref(), Some("a.css"));
    }

    #[test]
    fn hrefs_that_leave_the_book_are_refused() {
        for bad in [
            "../../a.css",
            "../../../etc/passwd",
            "/abs.css",
            "//cdn.example/a.css",
            "https://cdn.example/a.css",
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:image/png;base64,AAAA",
            "file:///etc/passwd",
            "%2e%2e/%2e%2e/a",
            "a%00b",
            "a%zz",
            "a%5cb",
        ] {
            assert_eq!(resolve("OEBPS/c.xhtml", bad, 32), None, "{bad:?}");
        }
    }

    #[test]
    fn extensions() {
        assert_eq!(extension("a/B.JPG").as_deref(), Some("jpg"));
        assert_eq!(extension("a/.hidden"), None);
        assert_eq!(extension("a/b."), None);
        assert_eq!(extension("a.d/b"), None);
    }
}
