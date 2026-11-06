//! XHTML out of a parsed document: cleaned of anything active, and for KEPUB with Kobo's
//! sentence spans. Namespaces and prefixes are kept as the source wrote them.

use roxmltree::Node;

use super::{escape_attr, escape_text};

pub(crate) const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";

/// Elements left out with everything inside them.
const DROPPED: &[&str] = &[
    "script", "noscript", "iframe", "frame", "frameset", "object", "embed", "applet", "base",
    "portal",
];

/// Elements written as `<x/>`; every other element gets an end tag, which HTML-minded readers
/// need for `<div/>` or `<a/>`.
pub(crate) const VOID: &[&str] = &[
    "area", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elements that start a new Kobo paragraph number.
const BLOCKS: &[&str] = &[
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "div",
    "blockquote",
    "td",
    "th",
    "dt",
    "dd",
    "figcaption",
    "pre",
    "caption",
    "aside",
    "section",
];

/// Elements whose text gets no spans: not shown as running text, or (ruby) laid out in a way
/// a span would break.
const NO_SPANS: &[&str] =
    &["head", "style", "title", "svg", "math", "ruby", "rt", "rp", "textarea", "option"];

#[derive(Default)]
pub(crate) struct Writer {
    out: String,
    kepub: bool,
    para: u32,
    sentence: u32,
}

impl Writer {
    pub(crate) fn new(kepub: bool) -> Self {
        Self { kepub, ..Self::default() }
    }

    /// Writes a whole XHTML document: the declaration, the doctype and the root element.
    pub(crate) fn document(mut self, doc: &roxmltree::Document<'_>) -> String {
        self.out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n");
        self.node(doc.root_element(), false);
        self.out.push('\n');
        self.out
    }

    fn node(&mut self, n: Node<'_, '_>, no_spans: bool) {
        if n.is_text() {
            let text = n.text().unwrap_or_default();
            if self.kepub && !no_spans && !text.trim().is_empty() {
                self.spans(text);
            } else {
                self.out.push_str(&escape_text(text));
            }
            return;
        }
        if !n.is_element() {
            return;
        }
        let local = n.tag_name().name();
        let html = n.tag_name().namespace().is_none_or(|ns| ns == XHTML_NS);
        if html && (DROPPED.contains(&local) || is_refresh(n)) {
            return;
        }
        let Some(name) = qname(n, n.tag_name().namespace(), local) else {
            return;
        };
        self.out.push('<');
        self.out.push_str(&name);
        self.declarations(n);
        for a in n.attributes() {
            if is_active(a.name(), a.value()) {
                continue;
            }
            let Some(an) = attr_qname(n, a.namespace(), a.name()) else {
                continue;
            };
            self.out.push_str(&format!(" {an}=\"{}\"", escape_attr(a.value())));
        }
        let no_spans = no_spans || (html && NO_SPANS.contains(&local)) || !html;
        let is_body = html && local == "body";
        if n.first_child().is_none() && !(self.kepub && is_body) {
            if html && !VOID.contains(&local) {
                self.out.push_str(&format!("></{name}>"));
            } else {
                self.out.push_str("/>");
            }
            return;
        }
        self.out.push('>');
        if self.kepub && html && local == "head" {
            // Kobo's columns otherwise add a margin above and below every chapter.
            self.out.push_str("<style type=\"text/css\">div#book-inner{margin-top:0;margin-bottom:0;}</style>");
        }
        if self.kepub && is_body {
            self.out.push_str("<div id=\"book-columns\"><div id=\"book-inner\">");
        }
        if self.kepub && html && BLOCKS.contains(&local) {
            self.para += 1;
            self.sentence = 0;
        }
        for c in n.children() {
            self.node(c, no_spans);
        }
        if self.kepub && is_body {
            self.out.push_str("</div></div>");
        }
        self.out.push_str(&format!("</{name}>"));
    }

    fn declarations(&mut self, n: Node<'_, '_>) {
        declarations(&mut self.out, n);
    }

    /// Text as Kobo sentence spans, `kobo.<paragraph>.<sentence>`.
    fn spans(&mut self, text: &str) {
        if self.para == 0 {
            self.para = 1;
        }
        for piece in sentences(text) {
            if piece.trim().is_empty() {
                self.out.push_str(&escape_text(piece));
                continue;
            }
            self.sentence += 1;
            self.out.push_str(&format!(
                "<span class=\"koboSpan\" id=\"kobo.{}.{}\">{}</span>",
                self.para,
                self.sentence,
                escape_text(piece)
            ));
        }
    }
}

/// Namespace declarations this element adds to what its parent has in scope.
pub(crate) fn declarations(out: &mut String, n: Node<'_, '_>) {
    let parent: Vec<(Option<&str>, &str)> = n
        .parent_element()
        .map(|p| p.namespaces().map(|ns| (ns.name(), ns.uri())).collect())
        .unwrap_or_default();
    for ns in n.namespaces() {
        if ns.name() == Some("xml") || parent.contains(&(ns.name(), ns.uri())) {
            continue;
        }
        match ns.name() {
            Some(p) => out.push_str(&format!(" xmlns:{p}=\"{}\"", escape_attr(ns.uri()))),
            None => out.push_str(&format!(" xmlns=\"{}\"", escape_attr(ns.uri()))),
        }
    }
}

/// The element's name with the prefix its namespace has in scope. `None` for a namespace with
/// no prefix in scope, which a well-formed source cannot produce.
pub(crate) fn qname(n: Node<'_, '_>, ns: Option<&str>, local: &str) -> Option<String> {
    let Some(ns) = ns else { return Some(local.to_owned()) };
    match n.lookup_prefix(ns) {
        Some(p) if !p.is_empty() => Some(format!("{p}:{local}")),
        _ => n
            .namespaces()
            .any(|d| d.name().is_none() && d.uri() == ns)
            .then(|| local.to_owned()),
    }
}

/// An attribute's name; an attribute in a namespace needs a prefix, the default one never
/// applies to attributes.
pub(crate) fn attr_qname(n: Node<'_, '_>, ns: Option<&str>, local: &str) -> Option<String> {
    let Some(ns) = ns else { return Some(local.to_owned()) };
    let p = n.lookup_prefix(ns)?;
    (!p.is_empty()).then(|| format!("{p}:{local}"))
}

/// Event handlers, and links that run code when followed.
fn is_active(name: &str, value: &str) -> bool {
    if name.len() > 2 && name[..2].eq_ignore_ascii_case("on") {
        return true;
    }
    if matches!(
        name,
        "href" | "src" | "action" | "formaction" | "data" | "poster" | "background"
    ) {
        let v: String =
            value.chars().filter(|c| !c.is_whitespace() && !c.is_control()).collect();
        let v = v.to_ascii_lowercase();
        return v.starts_with("javascript:")
            || v.starts_with("vbscript:")
            || v.starts_with("data:text/html");
    }
    false
}

fn is_refresh(n: Node<'_, '_>) -> bool {
    n.tag_name().name() == "meta"
        && n.attribute("http-equiv").is_some_and(|v| v.trim().eq_ignore_ascii_case("refresh"))
}

/// Splits text after each sentence end: `.`, `!`, `?` or an ellipsis followed by closing
/// quotes or brackets and white space, or a CJK full stop, which needs no space.
fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (_, c) = chars[i];
        let cjk = matches!(c, '。' | '！' | '？');
        if cjk || matches!(c, '.' | '!' | '?' | '…') {
            let mut j = i + 1;
            while j < chars.len()
                && matches!(chars[j].1, '.' | '!' | '?' | '。' | '！' | '？' | '…')
            {
                j += 1;
            }
            while j < chars.len()
                && matches!(chars[j].1, '"' | '\'' | '”' | '’' | ')' | ']' | '」' | '』' | '）')
            {
                j += 1;
            }
            let spaced = j < chars.len() && chars[j].1.is_whitespace();
            if spaced {
                while j < chars.len() && chars[j].1.is_whitespace() {
                    j += 1;
                }
            }
            if (spaced || cjk) && j < chars.len() {
                let end = chars[j].0;
                out.push(&text[start..end]);
                start = end;
            }
            i = j.max(i + 1);
            continue;
        }
        i += 1;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reparse(out: &str) -> roxmltree::Document<'_> {
        let opts = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
        roxmltree::Document::parse_with_options(out, opts).unwrap()
    }

    fn write(src: &str, kepub: bool) -> String {
        let doc = roxmltree::Document::parse(src).unwrap();
        Writer::new(kepub).document(&doc)
    }

    #[test]
    fn sentence_split() {
        assert_eq!(
            sentences("One. Two! \"Three?\" Four"),
            ["One. ", "Two! ", "\"Three?\" ", "Four"]
        );
        assert_eq!(sentences("3.5 is a number."), ["3.5 is a number."]);
        assert_eq!(sentences("塔。夜！"), ["塔。", "夜！"]);
        assert_eq!(sentences("Wait... what"), ["Wait... ", "what"]);
    }

    #[test]
    fn active_content_is_left_out() {
        let src = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><meta http-equiv="Refresh" content="0;url=x"/><base href="https://x.example/"/><script>alert(1)</script></head>
<body onload="x()"><p onclick="y()" class="a">Hi<a href=" JavaScript:alert(1)">l</a><a href="b.xhtml">ok</a></p><iframe src="https://x.example/"></iframe><object data="x"/><embed src="x"/></body></html>"#;
        let out = write(src, false);
        for bad in [
            "script", "alert", "onload", "onclick", "iframe", "object", "embed", "Refresh",
            "base",
        ] {
            assert!(!out.contains(bad), "{bad} in {out}");
        }
        assert!(
            out.contains(r#"<p class="a">Hi<a>l</a><a href="b.xhtml">ok</a></p>"#),
            "{out}"
        );
    }

    #[test]
    fn namespaces_survive() {
        let src = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="ja"><body><aside epub:type="footnote"><p>x</p></aside><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><image xlink:href="a.jpg"/></svg><div/></body></html>"#;
        let out = write(src, false);
        let again = reparse(&out);
        let aside = again.descendants().find(|n| n.has_tag_name("aside")).unwrap();
        assert_eq!(aside.attribute(("http://www.idpf.org/2007/ops", "type")), Some("footnote"));
        let image = again
            .descendants()
            .find(|n| n.has_tag_name(("http://www.w3.org/2000/svg", "image")))
            .unwrap();
        assert_eq!(image.attribute(("http://www.w3.org/1999/xlink", "href")), Some("a.jpg"));
        assert_eq!(
            again.root_element().attribute(("http://www.w3.org/XML/1998/namespace", "lang")),
            Some("ja")
        );
        assert!(out.contains("<div></div>"));
    }

    #[test]
    fn kobo_spans() {
        let src = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>T. x</title></head><body><h1>Title</h1><p>One. Two <em>three.</em> Four</p><p><ruby>塔<rt>とう</rt></ruby></p></body></html>"#;
        let out = write(src, true);
        assert!(out.contains(r#"<div id="book-columns"><div id="book-inner"><h1><span class="koboSpan" id="kobo.1.1">Title</span></h1>"#), "{out}");
        assert!(out.contains(r#"<p><span class="koboSpan" id="kobo.2.1">One. </span><span class="koboSpan" id="kobo.2.2">Two </span><em><span class="koboSpan" id="kobo.2.3">three.</span></em>"#), "{out}");
        assert!(out.contains("<ruby>塔<rt>とう</rt></ruby>"), "{out}");
        assert!(out.contains("<title>T. x</title>"));
        // Every id once.
        let doc = reparse(&out);
        let ids: Vec<&str> = doc.descendants().filter_map(|n| n.attribute("id")).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len());
    }
}
