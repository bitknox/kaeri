//! A book's CSS, cleaned before a web view sees it: only URLs inside the book (or images and
//! fonts in `data:` URLs) survive, and nothing that runs code or loads from elsewhere does.
//!
//! The scanner is not a CSS parser. It walks the text token by token, closely enough to find
//! every function call, `@import` and property name, and decodes escapes in names first so an
//! escaped `\75 rl(` is seen as `url(`. Strings are copied as written. What it cannot judge it
//! replaces with `none`, which a browser drops as an invalid value.

/// What a URL in the CSS may point at.
pub(crate) trait Urls {
    /// True for a relative URL that stays inside the book.
    fn in_book(&self, url: &str) -> bool;
}

impl<F: Fn(&str) -> bool> Urls for F {
    fn in_book(&self, url: &str) -> bool {
        self(url)
    }
}

/// Functions whose arguments are URLs as bare strings, or that run code. They are replaced
/// whole.
const REFUSED_FUNCTIONS: &[&str] =
    &["expression", "image", "image-set", "-webkit-image-set", "cross-fade", "element", "src"];

/// Properties left out with their value: they bind code (old IE and Firefox).
const REFUSED_PROPERTIES: &[&str] = &["behavior", "-moz-binding"];

/// Cleans a whole stylesheet, or a `style` attribute's declarations.
pub(crate) fn clean(text: &str, urls: &dyn Urls) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut s = Scan { c: &chars, i: 0, out: String::with_capacity(text.len()), urls };
    s.run(None);
    s.out
}

/// True when a URL is fine to load from `url()`: inside the book, or an image or font inline.
pub(crate) fn allowed_url(url: &str, urls: &dyn Urls) -> bool {
    let compact: String =
        url.chars().filter(|c| !c.is_whitespace() && !c.is_control()).collect();
    let lower = compact.to_ascii_lowercase();
    if let Some(data) = lower.strip_prefix("data:") {
        return data.starts_with("image/")
            || data.starts_with("font/")
            || data.starts_with("application/font")
            || data.starts_with("application/x-font")
            || data.starts_with("application/vnd.ms-fontobject");
    }
    !url.trim().is_empty() && urls.in_book(url.trim())
}

struct Scan<'a> {
    c: &'a [char],
    i: usize,
    out: String,
    urls: &'a dyn Urls,
}

impl Scan<'_> {
    fn peek(&self, at: usize) -> Option<char> {
        self.c.get(self.i + at).copied()
    }

    /// Copies tokens until `close` (not consumed) at this nesting, or the end.
    fn run(&mut self, close: Option<char>) {
        while let Some(c) = self.peek(0) {
            if Some(c) == close {
                return;
            }
            match c {
                '/' if self.peek(1) == Some('*') => self.comment(),
                '"' | '\'' => {
                    let s = self.string();
                    self.out.push_str(&s);
                }
                '@' => self.at_rule(),
                '(' | '[' | '{' => {
                    self.out.push(c);
                    self.i += 1;
                    let end = match c {
                        '(' => ')',
                        '[' => ']',
                        _ => '}',
                    };
                    self.run(Some(end));
                    if self.peek(0) == Some(end) {
                        self.out.push(end);
                        self.i += 1;
                    }
                }
                c if starts_ident(c) || c == '\\' => self.ident_or_function(),
                _ => {
                    self.out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn comment(&mut self) {
        self.i += 2;
        while self.i < self.c.len() {
            if self.c[self.i] == '*' && self.peek(1) == Some('/') {
                self.i += 2;
                break;
            }
            self.i += 1;
        }
        self.out.push(' ');
    }

    /// A quoted string as written, quotes included.
    fn string(&mut self) -> String {
        let q = self.c[self.i];
        let mut s = String::from(q);
        self.i += 1;
        while let Some(c) = self.peek(0) {
            self.i += 1;
            s.push(c);
            if c == '\\' {
                if let Some(n) = self.peek(0) {
                    s.push(n);
                    self.i += 1;
                }
            } else if c == q || c == '\n' {
                break;
            }
        }
        s
    }

    /// An identifier with its escapes decoded.
    fn ident(&mut self) -> String {
        let mut name = String::new();
        while let Some(c) = self.peek(0) {
            if c == '\\' {
                match self.escape() {
                    Some(d) => name.push(d),
                    None => break,
                }
            } else if is_ident(c) {
                name.push(c);
                self.i += 1;
            } else {
                break;
            }
        }
        name
    }

    /// Decodes the escape at the cursor: up to six hex digits and one white space, or the next
    /// character itself.
    fn escape(&mut self) -> Option<char> {
        let next = self.peek(1)?;
        if next == '\n' {
            return None;
        }
        self.i += 1;
        if next.is_ascii_hexdigit() {
            let mut v = 0u32;
            let mut n = 0;
            while n < 6
                && let Some(h) = self.peek(0).and_then(|c| c.to_digit(16))
            {
                v = v * 16 + h;
                self.i += 1;
                n += 1;
            }
            if self.peek(0).is_some_and(char::is_whitespace) {
                self.i += 1;
            }
            Some(char::from_u32(v).filter(|c| *c != '\0').unwrap_or('\u{FFFD}'))
        } else {
            self.i += 1;
            Some(next)
        }
    }

    fn ident_or_function(&mut self) {
        let start = self.i;
        let name = self.ident();
        if name.is_empty() {
            // A lone backslash.
            self.i = start + 1;
            return;
        }
        let lower = name.to_ascii_lowercase();
        if self.peek(0) == Some('(') {
            self.i += 1;
            if lower == "url" {
                self.url();
            } else if REFUSED_FUNCTIONS.contains(&lower.as_str()) || lower.contains("expression")
            {
                self.skip_to(')');
                self.i += 1;
                self.out.push_str("none");
            } else {
                self.out.push_str(&escape_ident(&name));
                self.out.push('(');
                self.run(Some(')'));
                if self.peek(0) == Some(')') {
                    self.out.push(')');
                    self.i += 1;
                }
            }
            return;
        }
        // A property name ends at its colon.
        let mut j = self.i;
        while self.c.get(j).is_some_and(|c| c.is_whitespace()) {
            j += 1;
        }
        if self.c.get(j) == Some(&':') && REFUSED_PROPERTIES.contains(&lower.as_str()) {
            self.skip_declaration();
            return;
        }
        if lower.starts_with("javascript") {
            self.out.push_str("none");
            return;
        }
        self.out.push_str(&escape_ident(&name));
    }

    /// The inside of `url(`, the cursor after the parenthesis. The URL is kept when allowed,
    /// else the call becomes `none`.
    fn url(&mut self) {
        while self.peek(0).is_some_and(char::is_whitespace) {
            self.i += 1;
        }
        let value = match self.peek(0) {
            Some('"' | '\'') => {
                let raw = self.string();
                unquote(&raw)
            }
            _ => {
                let mut v = String::new();
                // A parenthesis inside makes a bad URL, which runs to the matching one.
                let mut depth = 0;
                while let Some(c) = self.peek(0) {
                    if c == ')' && depth == 0 {
                        break;
                    }
                    if c == '(' {
                        depth += 1;
                    } else if c == ')' {
                        depth -= 1;
                    }
                    if c == '\\' {
                        match self.escape() {
                            Some(d) => v.push(d),
                            None => self.i += 1,
                        }
                        continue;
                    }
                    v.push(c);
                    self.i += 1;
                }
                v.trim().to_owned()
            }
        };
        self.skip_to(')');
        if self.peek(0) == Some(')') {
            self.i += 1;
        }
        if allowed_url(&value, self.urls) {
            self.out.push_str("url(\"");
            self.out.push_str(&css_string(&value));
            self.out.push_str("\")");
        } else {
            self.out.push_str("none");
        }
    }

    /// `@import` keeps an in-book stylesheet; `@font-face`, `@media` and the rest pass through
    /// the scanner, so their URLs are checked like any other.
    fn at_rule(&mut self) {
        self.i += 1;
        let name = self.ident();
        let lower = name.to_ascii_lowercase();
        if lower != "import" {
            self.out.push('@');
            self.out.push_str(&escape_ident(&name));
            return;
        }
        // The rule runs to its semicolon, strings and parentheses followed.
        let mut target = None;
        let mut after_target = self.i;
        while let Some(c) = self.peek(0) {
            match c {
                ';' => break,
                '"' | '\'' if target.is_none() => {
                    let raw = self.string();
                    target = Some(unquote(&raw));
                    after_target = self.i;
                    continue;
                }
                '/' if self.peek(1) == Some('*') => {
                    self.comment();
                    continue;
                }
                c if starts_ident(c) || c == '\\' => {
                    let word = self.ident().to_ascii_lowercase();
                    if word == "url" && self.peek(0) == Some('(') && target.is_none() {
                        self.i += 1;
                        let before = self.out.len();
                        self.url();
                        let written = self.out.split_off(before);
                        target = Some(
                            written
                                .strip_prefix("url(\"")
                                .and_then(|w| w.strip_suffix("\")"))
                                .map(unquote_css)
                                .unwrap_or_default(),
                        );
                        after_target = self.i;
                    }
                    continue;
                }
                _ => {}
            }
            self.i += 1;
        }
        let media: String = self.c[after_target.min(self.i)..self.i].iter().collect();
        if self.peek(0) == Some(';') {
            self.i += 1;
        }
        let Some(target) =
            target.filter(|t| !t.starts_with("data:") && allowed_url(t, self.urls))
        else {
            return;
        };
        // Media queries after the URL are kept when they are plain words and sizes.
        let media = media.trim();
        let media = if media.chars().all(|c| c.is_alphanumeric() || " ,()-:.".contains(c)) {
            media
        } else {
            ""
        };
        self.out.push_str("@import url(\"");
        self.out.push_str(&css_string(&target));
        self.out.push_str("\")");
        if !media.is_empty() {
            self.out.push(' ');
            self.out.push_str(&media);
        }
        self.out.push(';');
    }

    /// Skips to `close` at this nesting, strings followed. Leaves the cursor on it.
    fn skip_to(&mut self, close: char) {
        let mut depth = 0;
        while let Some(c) = self.peek(0) {
            match c {
                '"' | '\'' => {
                    self.string();
                    continue;
                }
                '(' => depth += 1,
                ')' if depth > 0 => depth -= 1,
                c if c == close && depth == 0 => return,
                _ => {}
            }
            self.i += 1;
        }
    }

    /// Skips a declaration to its semicolon or the end of its block.
    fn skip_declaration(&mut self) {
        let mut depth = 0;
        while let Some(c) = self.peek(0) {
            match c {
                '"' | '\'' => {
                    self.string();
                    continue;
                }
                '(' => depth += 1,
                ')' if depth > 0 => depth -= 1,
                ';' if depth == 0 => {
                    self.i += 1;
                    return;
                }
                '}' if depth == 0 => return,
                _ => {}
            }
            self.i += 1;
        }
    }
}

fn starts_ident(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '-' || !c.is_ascii()
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || !c.is_ascii()
}

/// A decoded identifier written back so it reads the same: anything but name characters is
/// escaped.
fn escape_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if is_ident(c) && !c.is_control() {
            out.push(c);
        } else {
            out.push_str(&format!("\\{:x} ", c as u32));
        }
    }
    out
}

/// A quoted CSS string's value, escapes decoded.
fn unquote(raw: &str) -> String {
    let mut inner: Vec<char> = raw.chars().collect();
    if inner.len() >= 2 && inner.last() == inner.first() {
        inner.pop();
    }
    if !inner.is_empty() {
        inner.remove(0);
    }
    unquote_css(&inner.iter().collect::<String>())
}

fn unquote_css(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '\\' && i + 1 < c.len() {
            i += 1;
            if c[i].is_ascii_hexdigit() {
                let mut v = 0u32;
                let mut n = 0;
                while n < 6 && i < c.len() && c[i].is_ascii_hexdigit() {
                    v = v * 16 + c[i].to_digit(16).unwrap_or(0);
                    i += 1;
                    n += 1;
                }
                if i < c.len() && c[i].is_whitespace() {
                    i += 1;
                }
                out.push(char::from_u32(v).unwrap_or('\u{FFFD}'));
            } else if c[i] != '\n' {
                out.push(c[i]);
                i += 1;
            } else {
                i += 1;
            }
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

/// A value written inside double quotes.
fn css_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_control() => out.push_str(&format!("\\{:x} ", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The writing mode a stylesheet sets, if it sets a vertical one anywhere: `writing-mode`,
/// `-epub-writing-mode` or `-webkit-writing-mode`.
pub(crate) fn vertical_mode(css: &str) -> Option<super::Flow> {
    let lower = css.to_ascii_lowercase();
    let mut from = 0;
    while let Some(p) = lower[from..].find("writing-mode") {
        let at = from + p + "writing-mode".len();
        from = at;
        let rest = lower[at..].trim_start();
        let Some(value) = rest.strip_prefix(':') else { continue };
        let value: String =
            value.chars().take_while(|c| !matches!(c, ';' | '}' | '!')).collect();
        match value.trim() {
            "vertical-rl" | "tb-rl" | "tb" => return Some(super::Flow::VerticalRl),
            "vertical-lr" | "tb-lr" => return Some(super::Flow::VerticalLr),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(url: &str) -> bool {
        crate::path::resolve("OEBPS/css/a.css", url, 32).is_some()
    }

    fn c(s: &str) -> String {
        clean(s, &book)
    }

    #[test]
    fn book_urls_stay_and_remote_ones_go() {
        assert_eq!(
            c("p{background:url(../img/a.png)}"),
            "p{background:url(\"../img/a.png\")}"
        );
        assert_eq!(c("p{background:url( 'https://x.example/a.png' )}"), "p{background:none}");
        assert_eq!(c("p{background:url(//x.example/a.png)}"), "p{background:none}");
        assert_eq!(c("p{background:URL(/abs.png)}"), "p{background:none}");
        assert_eq!(c("p{background:url(javascript:alert(1))}"), "p{background:none}");
        assert_eq!(c("p{background:url(data:image/png;base64,AAAA)}"), "p{background:url(\"data:image/png;base64,AAAA\")}");
        assert_eq!(c("p{background:url(data:text/html,x)}"), "p{background:none}");
    }

    #[test]
    fn escapes_and_comments_do_not_hide_a_url() {
        assert_eq!(c("p{background:\\75 rl(https://x.example/a)}"), "p{background:none}");
        assert_eq!(c("p{background:u\\72 l('https://x.example/a')}"), "p{background:none}");
        assert_eq!(c("p{b/**/ackground:url(https://x.example)}"), "p{b ackground:none}");
        assert!(!c("p{background:ur/**/l(https://x.example/a)}").contains("url(https"));
    }

    #[test]
    fn imports_keep_the_book_and_drop_the_rest() {
        assert_eq!(c("@import url(\"b.css\");p{}"), "@import url(\"b.css\");p{}");
        assert_eq!(c("@import 'b.css' screen;p{}"), "@import url(\"b.css\") screen;p{}");
        assert_eq!(c("@import url(https://x.example/a.css);p{}"), "p{}");
        assert_eq!(c("@IMPORT \"http://x.example/a.css\";p{}"), "p{}");
        assert_eq!(c("@\\69mport 'http://x.example/a.css';p{}"), "p{}");
    }

    #[test]
    fn code_and_bare_string_urls_are_refused() {
        assert_eq!(c("p{width:expression(alert(1))}"), "p{width:none}");
        assert_eq!(c("p{background:image-set('https://x.example/a.png' 1x)}"), "p{background:none}");
        assert_eq!(c("p{behavior:url(a.htc);color:red}"), "p{color:red}");
        assert_eq!(c("p{-moz-binding:url(x.xml#b)}"), "p{}");
    }

    #[test]
    fn fonts_and_strings_survive() {
        let src = "@font-face{font-family:\"Lantern\";src:url(../fonts/l.woff2) format(\"woff2\")}p::before{content:\"\\201C\\\"\"}";
        let out = c(src);
        assert!(out.contains("src:url(\"../fonts/l.woff2\") format(\"woff2\")"), "{out}");
        assert!(out.contains("content:\"\\201C\\\"\""), "{out}");
        assert_eq!(c("p{color:#333;margin:0 1em}"), "p{color:#333;margin:0 1em}");
    }

    #[test]
    fn vertical_modes_are_found() {
        assert_eq!(vertical_mode("html{-epub-writing-mode: vertical-rl}"), Some(super::super::Flow::VerticalRl));
        assert_eq!(vertical_mode("body{writing-mode:vertical-lr;}"), Some(super::super::Flow::VerticalLr));
        assert_eq!(vertical_mode("body{writing-mode:horizontal-tb}"), None);
    }
}
