//! A book's XHTML and SVG, cleaned for the web view: nothing that runs code, loads from
//! outside the book, navigates by itself or submits anything survives.
//!
//! The content security policy already blocks scripts and remote loads in every engine (S3);
//! this is the second wall, and the one that stops what the policy does not cover: a meta
//! refresh, a `<base>` that moves every link, frames, and old engines with gaps in their CSP.
//!
//! Documents are parsed under the [`Limits`]. One that is not well-formed XML is tried again
//! with HTML's named entities turned into characters, and then rebuilt from its text as plain
//! paragraphs, so a sloppy chapter still reads. One over the size limit, or with nothing
//! readable, becomes an empty page whose `<body>` says why, which the paginator reports.

use roxmltree::Node;

use super::{Flow, css, hyphen};
use crate::write::xhtml::{VOID, XHTML_NS, attr_qname, declarations, qname};
use crate::write::{escape_attr, escape_text};
use crate::{Format, Limits, path, xml};

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const MATHML_NS: &str = "http://www.w3.org/1998/Math/MathML";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

/// HTML elements left out with everything inside them.
const DROP_HTML: &[&str] = &[
    "script",
    "noscript",
    "template",
    "iframe",
    "frame",
    "frameset",
    "object",
    "embed",
    "applet",
    "base",
    "portal",
    "fencedframe",
    "input",
    "select",
    "textarea",
    "keygen",
    "dialog",
];

/// HTML elements whose tags go but whose content stays.
const UNWRAP_HTML: &[&str] = &["form"];

/// SVG elements left out: scripts, and animations that can set an attribute (an `href`) to
/// anything.
const DROP_SVG: &[&str] = &[
    "script",
    "set",
    "animate",
    "animateMotion",
    "animateTransform",
    "animateColor",
    "discard",
    "handler",
    "listener",
];

/// Attributes dropped wherever they are.
const DROP_ATTRS: &[&str] = &[
    "srcdoc",
    "ping",
    "target",
    "formaction",
    "action",
    "autofocus",
    "autoplay",
    "srcset",
    "imagesrcset",
    "sizes",
    "http-equiv",
    "nonce",
    "contenteditable",
    "popover",
    "popovertarget",
    "attributionsrc",
];

/// HTML attributes that load what they name.
const RESOURCE_ATTRS: &[&str] = &["src", "poster", "background", "data", "longdesc"];

/// Text in these is never hyphenated: code, ruby annotations, maths and pictures.
const NO_HYPHENS: &[&str] = &[
    "pre", "code", "kbd", "samp", "tt", "var", "style", "title", "head", "ruby", "rt", "rp",
    "textarea",
];

/// Block elements that start a new paragraph when a document is rebuilt from its text.
const BLOCKS: &[&str] = &[
    "p",
    "div",
    "br",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "tr",
    "blockquote",
    "section",
    "article",
    "aside",
    "header",
    "footer",
    "figure",
    "figcaption",
    "hr",
    "dt",
    "dd",
    "pre",
    "table",
];

/// How one document is cleaned.
#[derive(Debug, Clone, Copy)]
pub struct Clean<'a> {
    /// The document's normalised path in the book, which its links resolve against.
    pub path: &'a str,
    /// The book's language, for a document that names none.
    pub lang: Option<&'a str>,
    /// Insert soft hyphens (Windows and Linux). Ignored by a build without the patterns.
    pub soft_hyphens: bool,
    pub limits: &'a Limits,
}

/// Why a document came out empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failed {
    TooLarge,
    Unreadable,
}

impl Failed {
    /// The word on `<body data-kaeri-failed>`, which the paginator passes to the host.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TooLarge => "too-large",
            Self::Unreadable => "unreadable",
        }
    }
}

/// A cleaned document, ready to serve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub text: String,
    /// Just inside `<head>`, where [`Document::served`] puts the reader's settings.
    head: Option<usize>,
    pub failed: Option<Failed>,
    /// Rebuilt from its text because it was not well-formed.
    pub rebuilt: bool,
    /// A vertical writing mode set in the document itself.
    pub flow: Option<Flow>,
    /// The book's stylesheets the document links, as normalised paths.
    pub stylesheets: Vec<String>,
}

impl Document {
    /// The document with the reader's settings in a `<meta name="kaeri-setup">`, which the
    /// paginator reads before its first layout, and a viewport as wide as the window that does
    /// not zoom (WebKit and Android lay a page without one out 980 pixels wide; the text size
    /// is a setting, not a pinch). In `<head>`, so element paths
    /// in `<body>` never change.
    pub fn served(&self, setup: &str) -> String {
        let Some(at) = self.head else { return self.text.clone() };
        let meta = format!(
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, minimum-scale=1, maximum-scale=1, user-scalable=no\"/>\
             <meta name=\"kaeri-setup\" content=\"{}\"/>",
            escape_attr(setup)
        );
        let mut out = String::with_capacity(self.text.len() + meta.len());
        out.push_str(&self.text[..at]);
        out.push_str(&meta);
        out.push_str(&self.text[at..]);
        out
    }

    fn failed(why: Failed, lang: Option<&str>) -> Self {
        let lang = lang
            .map(|l| format!(" lang=\"{0}\" xml:lang=\"{0}\"", escape_attr(l)))
            .unwrap_or_default();
        let text = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"{XHTML_NS}\"{lang}><head><title></title></head><body data-kaeri-failed=\"{}\"></body></html>\n",
            why.as_str()
        );
        let head = text.find("<head>").map(|p| p + "<head>".len());
        Document {
            text,
            head,
            failed: Some(why),
            rebuilt: false,
            flow: None,
            stylesheets: Vec::new(),
        }
    }
}

/// Cleans one XHTML document from the book.
pub fn document(bytes: &[u8], clean: &Clean<'_>) -> Document {
    if bytes.len() as u64 > clean.limits.max_xml_bytes {
        return Document::failed(Failed::TooLarge, clean.lang);
    }
    let Ok(text) = xml::text(bytes, Format::Epub, clean.limits) else {
        return Document::failed(Failed::Unreadable, clean.lang);
    };
    from_text(&text, clean)
}

fn from_text(text: &str, clean: &Clean<'_>) -> Document {
    if let Ok(doc) = xml::parse(text, Format::Epub, clean.limits) {
        return walk(&doc, clean);
    }
    let entities = crate::epub::html_entities(text);
    if let Ok(doc) = xml::parse(&entities, Format::Epub, clean.limits) {
        return walk(&doc, clean);
    }
    let paragraphs = plain_paragraphs(&entities);
    if paragraphs.is_empty() {
        return Document::failed(Failed::Unreadable, clean.lang);
    }
    let mut body = String::new();
    for p in &paragraphs {
        body.push_str("<p>");
        body.push_str(&escape_text(p));
        body.push_str("</p>");
    }
    let rebuilt = wrap("", &body, clean.lang);
    match xml::parse(&rebuilt, Format::Epub, clean.limits) {
        Ok(doc) => Document { rebuilt: true, ..walk(&doc, clean) },
        Err(_) => Document::failed(Failed::Unreadable, clean.lang),
    }
}

/// A text chapter from a source, an XHTML body fragment, as a whole cleaned document.
pub fn text_chapter(fragment: &str, title: &str, clean: &Clean<'_>) -> Document {
    if fragment.len() as u64 > clean.limits.max_xml_bytes {
        return Document::failed(Failed::TooLarge, clean.lang);
    }
    // A fragment with a whole document in it gives its body.
    let body = body_of(fragment).unwrap_or(fragment);
    from_text(&wrap(title, body, clean.lang), clean)
}

/// Cleans a standalone SVG image or page. `None` when it does not parse.
pub fn svg(bytes: &[u8], clean: &Clean<'_>) -> Option<String> {
    if bytes.len() as u64 > clean.limits.max_xml_bytes {
        return None;
    }
    let text = xml::text(bytes, Format::Epub, clean.limits).ok()?;
    let doc = xml::parse(&text, Format::Epub, clean.limits).ok()?;
    let no_hyphens = Clean { soft_hyphens: false, ..*clean };
    Some(walk(&doc, &no_hyphens).text)
}

/// Cleans one of the book's stylesheets, at `path`.
pub fn stylesheet(text: &str, path: &str, limits: &Limits) -> String {
    let depth = limits.max_path_depth;
    let urls = |u: &str| path::resolve(path, u, depth).is_some();
    css::clean(text, &urls)
}

/// What a book's own `<meta name="viewport">` is renamed to: its size, for fixed layout.
const BOOK_VIEWPORT: &str = "kaeri-viewport";

fn wrap(title: &str, body: &str, lang: Option<&str>) -> String {
    let lang = lang
        .map(|l| format!(" lang=\"{0}\" xml:lang=\"{0}\"", escape_attr(l)))
        .unwrap_or_default();
    format!(
        "<html xmlns=\"{XHTML_NS}\" xmlns:epub=\"http://www.idpf.org/2007/ops\"{lang}><head><title>{}</title></head><body>{body}</body></html>",
        escape_text(title)
    )
}

/// The inside of a `<body>` in a fragment that carries a whole document.
fn body_of(fragment: &str) -> Option<&str> {
    let lower = fragment.to_ascii_lowercase();
    let open = lower.find("<body")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = lower.rfind("</body").filter(|e| *e >= start)?;
    Some(&fragment[start..end])
}

fn walk(doc: &roxmltree::Document<'_>, clean: &Clean<'_>) -> Document {
    let mut w = Walk {
        out: String::with_capacity(doc.input_text().len()),
        clean,
        head: None,
        flow: None,
        sheets: Vec::new(),
    };
    w.out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    let root = doc.root_element();
    let html = is_html(root) && root.tag_name().name() == "html";
    if html {
        w.out.push_str("<!DOCTYPE html>\n");
    }
    w.element(root, clean.lang.map(str::to_owned), true, true);
    w.out.push('\n');
    Document {
        text: w.out,
        head: w.head,
        failed: None,
        rebuilt: false,
        flow: w.flow,
        stylesheets: w.sheets,
    }
}

fn is_html(n: Node<'_, '_>) -> bool {
    n.tag_name().namespace().is_none_or(|ns| ns == XHTML_NS)
}

fn lang_of<'a>(n: Node<'a, '_>) -> Option<&'a str> {
    n.attribute((XML_NS, "lang")).or_else(|| n.attribute("lang")).filter(|l| !l.is_empty())
}

struct Walk<'c> {
    out: String,
    clean: &'c Clean<'c>,
    head: Option<usize>,
    flow: Option<Flow>,
    sheets: Vec<String>,
}

impl Walk<'_> {
    fn element(&mut self, n: Node<'_, '_>, lang: Option<String>, hyphens: bool, root: bool) {
        let local = n.tag_name().name();
        let ns = n.tag_name().namespace();
        let html = is_html(n);
        let svg = ns == Some(SVG_NS);
        if html && DROP_HTML.contains(&local) {
            return;
        }
        if html && local == "meta" && n.attributes().any(|a| a.name() == "http-equiv") {
            return;
        }
        if html && local == "link" {
            self.link(n);
            return;
        }
        if svg && DROP_SVG.contains(&local) {
            return;
        }
        let own_lang = lang_of(n).map(str::to_owned);
        let lang = own_lang.clone().or(lang);
        let hyphens = hyphens
            && !(html && NO_HYPHENS.contains(&local))
            && !svg
            && ns != Some(MATHML_NS)
            && n.attribute("translate") != Some("no");
        if html && UNWRAP_HTML.contains(&local) {
            self.children(n, &lang, hyphens);
            return;
        }
        let Some(name) = qname(n, ns, local) else { return };
        self.out.push('<');
        self.out.push_str(&name);
        declarations(&mut self.out, n);
        for a in n.attributes() {
            let Some(value) =
                self.attribute(local, html, svg, a.namespace(), a.name(), a.value())
            else {
                continue;
            };
            let Some(an) = attr_qname(n, a.namespace(), a.name()) else { continue };
            self.out.push_str(&format!(" {an}=\"{}\"", escape_attr(&value)));
        }
        let is_root_html = root && html && local == "html";
        if is_root_html
            && own_lang.is_none()
            && let Some(l) = &lang
        {
            // The engines hyphenate (and pick CJK fonts) by the document's language.
            self.out.push_str(&format!(" lang=\"{0}\" xml:lang=\"{0}\"", escape_attr(l)));
        }
        if html && local == "style" {
            self.out.push('>');
            let text: String = n.children().filter_map(|c| c.text()).collect();
            if self.flow.is_none() {
                self.flow = css::vertical_mode(&text);
            }
            let depth = self.clean.limits.max_path_depth;
            let base = self.clean.path;
            let urls = |u: &str| path::resolve(base, u, depth).is_some();
            self.out.push_str(&escape_text(&css::clean(&text, &urls)));
            self.out.push_str(&format!("</{name}>"));
            return;
        }
        let is_head = html && local == "head";
        let needs_head = is_root_html
            && !n
                .children()
                .any(|c| c.is_element() && is_html(c) && c.tag_name().name() == "head");
        if n.first_child().is_none() && !is_head && !needs_head {
            if html && !VOID.contains(&local) {
                self.out.push_str(&format!("></{name}>"));
            } else {
                self.out.push_str("/>");
            }
            return;
        }
        self.out.push('>');
        if is_head && self.head.is_none() {
            self.head = Some(self.out.len());
        }
        if needs_head {
            self.out.push_str("<head>");
            self.head = Some(self.out.len());
            self.out.push_str("</head>");
        }
        self.children(n, &lang, hyphens);
        self.out.push_str(&format!("</{name}>"));
    }

    fn children(&mut self, n: Node<'_, '_>, lang: &Option<String>, hyphens: bool) {
        for c in n.children() {
            if c.is_element() {
                self.element(c, lang.clone(), hyphens, false);
            } else if c.is_text() {
                let text = c.text().unwrap_or_default();
                let soft = (hyphens && self.clean.soft_hyphens)
                    .then(|| lang.as_deref().and_then(|l| hyphen::soft_hyphens(text, l)))
                    .flatten();
                self.out.push_str(&escape_text(soft.as_deref().unwrap_or(text)));
            }
            // Comments and processing instructions are left out.
        }
    }

    /// A `<link>` stays only as a stylesheet inside the book.
    fn link(&mut self, n: Node<'_, '_>) {
        let rel = n.attribute("rel").unwrap_or_default().to_ascii_lowercase();
        if !rel.split_whitespace().any(|r| r == "stylesheet")
            || rel.split_whitespace().any(|r| r == "alternate")
        {
            return;
        }
        let Some(href) = n.attribute("href") else { return };
        let Some(target) =
            path::resolve(self.clean.path, href, self.clean.limits.max_path_depth)
        else {
            return;
        };
        self.sheets.push(target.path);
        self.out.push_str(&format!(
            "<link rel=\"stylesheet\" type=\"text/css\" href=\"{}\"",
            escape_attr(href.trim())
        ));
        if let Some(media) = n.attribute("media") {
            self.out.push_str(&format!(" media=\"{}\"", escape_attr(media)));
        }
        self.out.push_str("/>");
    }

    /// The attribute's value as served, or `None` to leave it out.
    fn attribute(
        &self,
        element: &str,
        html: bool,
        svg: bool,
        ns: Option<&str>,
        name: &str,
        value: &str,
    ) -> Option<String> {
        if name.len() > 2 && name[..2].eq_ignore_ascii_case("on") {
            return None;
        }
        if ns == Some(XML_NS) && name == "base" {
            return None;
        }
        let plain = ns.is_none();
        if plain && DROP_ATTRS.contains(&name.to_ascii_lowercase().as_str()) {
            return None;
        }
        if html
            && element == "meta"
            && plain
            && name == "name"
            && value.trim().eq_ignore_ascii_case("viewport")
        {
            // The window is the reader's: a fixed-layout page's size is kept for the paginator.
            return Some(BOOK_VIEWPORT.to_owned());
        }
        if plain && name.eq_ignore_ascii_case("style") {
            let depth = self.clean.limits.max_path_depth;
            let base = self.clean.path;
            let urls = |u: &str| path::resolve(base, u, depth).is_some();
            return Some(css::clean(value, &urls));
        }
        let href = name == "href" && (plain || ns == Some(XLINK_NS));
        if href {
            let link = element == "a" || (html && element == "area");
            let ok = if link { self.link_ok(value) } else { self.resource_ok(value) };
            return ok.then(|| value.to_owned());
        }
        if html && plain && RESOURCE_ATTRS.contains(&name) {
            return self.resource_ok(value).then(|| value.to_owned());
        }
        if svg && plain && name == "src" {
            return self.resource_ok(value).then(|| value.to_owned());
        }
        if plain && name == "cite" {
            return self.link_ok(value).then(|| value.to_owned());
        }
        Some(value.to_owned())
    }

    /// Loads: inside the book, the document's own fragments, or an image inline.
    fn resource_ok(&self, value: &str) -> bool {
        let v = value.trim();
        if v.starts_with('#') {
            return true;
        }
        let compact: String = v.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.to_ascii_lowercase().starts_with("data:image/") {
            return true;
        }
        path::resolve(self.clean.path, v, self.clean.limits.max_path_depth).is_some()
    }

    /// Links: what may load, plus web and mail links, which the host opens outside or refuses.
    fn link_ok(&self, value: &str) -> bool {
        let compact: String =
            value.chars().filter(|c| !c.is_whitespace() && !c.is_control()).collect();
        let lower = compact.to_ascii_lowercase();
        if lower.starts_with("data:") {
            return false;
        }
        lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("mailto:")
            || self.resource_ok(value)
    }
}

/// The text of a document that is not XML, as paragraphs: tags dropped, block tags as breaks,
/// scripts and styles left out, the five XML entities and numeric references decoded.
fn plain_paragraphs(text: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let b = text.as_bytes();
    let mut i = 0;
    let lower = text.to_ascii_lowercase();
    let push = |current: &mut String, paragraphs: &mut Vec<String>| {
        let p = xml::collapse(&decode_refs(current));
        if !p.is_empty() {
            paragraphs.push(p);
        }
        current.clear();
    };
    while i < b.len() {
        if b[i] == b'<' {
            let close = text[i..].find('>');
            let end = close.map_or(b.len(), |p| i + p + 1);
            let tag = &lower[i + 1..close.map_or(b.len(), |p| i + p)];
            let closing = tag.starts_with('/');
            let name: String = tag
                .trim_start_matches(['/', '!', '?'])
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if !closing && matches!(name.as_str(), "script" | "style" | "head" | "title") {
                let close = format!("</{name}");
                i = lower[end..].find(&close).map_or(b.len(), |p| end + p);
                i = text[i..].find('>').map_or(b.len(), |p| i + p + 1);
                continue;
            }
            if BLOCKS.contains(&name.as_str()) {
                push(&mut current, &mut paragraphs);
            } else {
                current.push(' ');
            }
            i = end;
            continue;
        }
        let next = text[i..].find('<').map_or(b.len(), |p| i + p);
        current.push_str(&text[i..next]);
        i = next;
    }
    push(&mut current, &mut paragraphs);
    paragraphs
}

fn decode_refs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let Some(semi) = rest.bytes().take(12).position(|b| b == b';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..semi];
        let c = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            n if n.starts_with("#x") || n.starts_with("#X") => {
                u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32)
            }
            n if n.starts_with('#') => n[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
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

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_with(src: &str, lang: Option<&str>, soft: bool) -> Document {
        let limits = Limits::default();
        let clean =
            Clean { path: "OEBPS/text/c1.xhtml", lang, soft_hyphens: soft, limits: &limits };
        document(src.as_bytes(), &clean)
    }

    fn clean(src: &str) -> String {
        clean_with(src, Some("en"), false).text
    }

    fn reparse(out: &str) -> roxmltree::Document<'_> {
        let opts = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
        roxmltree::Document::parse_with_options(out, opts).unwrap()
    }

    const HOSTILE: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:xlink="http://www.w3.org/1999/xlink">
<head>
<meta http-equiv="refresh" content="0;url=https://x.example/"/>
<meta http-equiv="Content-Security-Policy" content="default-src *"/>
<base href="https://x.example/"/>
<link rel="stylesheet" href="https://x.example/a.css"/>
<link rel="stylesheet" href="../css/book.css"/>
<link rel="prefetch" href="https://x.example/p"/>
<script>window.__pwned=1</script>
<script src="../js/a.js"></script>
<style>@import url(https://x.example/i.css); p{background:url(https://x.example/b.png)} h1{background:url(../img/ok.png)}</style>
</head>
<body onload="window.__pwned=2">
<p id="a" onclick="x()" style="background:url('https://x.example/s.png');color:red">Hello <a href="javascript:alert(1)">js</a> <a href=" JaVaScRiPt:alert(1)">js2</a> <a href="https://x.example/out">out</a> <a href="c2.xhtml#n1">next</a></p>
<img id="remote" src="https://x.example/r.png" srcset="https://x.example/r2.png 2x"/>
<img id="local" src="../img/ok.png" onerror="window.__pwned=3"/>
<img id="inline" src="data:image/png;base64,AAAA"/>
<iframe src="https://x.example/f"></iframe><iframe srcdoc="&lt;script&gt;parent.__pwned=4&lt;/script&gt;"></iframe>
<object data="https://x.example/o"></object><embed src="https://x.example/e"/>
<form action="https://x.example/f"><p>Inside a form</p><input type="image" src="https://x.example/i.png"/><button formaction="https://x.example/b">b</button></form>
<video src="https://x.example/v.mp4" autoplay="autoplay" poster="https://x.example/p.png"></video>
<audio src="../audio/ok.mp3"></audio>
<svg xmlns="http://www.w3.org/2000/svg" onload="window.__pwned=5"><script>window.__pwned=6</script><image xlink:href="https://x.example/s.png"/><image xlink:href="../img/ok.png"/><a xlink:href="javascript:alert(1)"><text>t</text></a><set attributeName="href" to="javascript:alert(1)"/><animate attributeName="href" values="javascript:alert(1)"/></svg>
<p xml:base="https://x.example/">based</p>
<!-- a comment --><?pi data?>
</body>
</html>"##;

    #[test]
    fn hostile_content_is_left_out() {
        let out = clean(HOSTILE);
        for bad in [
            "x.example/\"",
            "refresh",
            "Content-Security",
            "<base",
            "x.example/a.css",
            "prefetch",
            "<script",
            "__pwned",
            "onload",
            "onclick",
            "onerror",
            "javascript",
            "JaVaScRiPt",
            "x.example/s.png",
            "x.example/b.png",
            "x.example/i.css",
            "x.example/r.png",
            "srcset",
            "<iframe",
            "srcdoc",
            "<object",
            "<embed",
            "<input",
            "formaction",
            "action=",
            "x.example/v.mp4",
            "autoplay",
            "<set",
            "<animate",
            "xml:base",
            "a comment",
            "<?pi",
        ] {
            assert!(!out.contains(bad), "{bad} in {out}");
        }
        for good in [
            "href=\"../css/book.css\"",
            "url(\"../img/ok.png\")",
            "id=\"local\" src=\"../img/ok.png\"",
            "data:image/png;base64,AAAA",
            "<p>Inside a form</p>",
            "href=\"https://x.example/out\"",
            "href=\"c2.xhtml#n1\"",
            "src=\"../audio/ok.mp3\"",
            "xlink:href=\"../img/ok.png\"",
            "color:red",
        ] {
            assert!(out.contains(good), "{good} not in {out}");
        }
        // Still XHTML, and the remote image is still an element so paths do not move.
        let doc = reparse(&out);
        assert!(doc.descendants().any(|n| n.attribute("id") == Some("remote")));
        assert_eq!(doc.root_element().attribute("lang"), Some("en"));
    }

    #[test]
    fn the_setup_goes_in_the_head_and_the_body_is_untouched() {
        let d = clean_with(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>One</p></body></html>",
            None,
            false,
        );
        let served = d.served("{\"size\":18,\"q\":\"<&>\"}");
        let doc = reparse(&served);
        let head = doc.descendants().find(|n| n.has_tag_name((XHTML_NS, "head"))).unwrap();
        let mut metas = head.children().filter(|n| n.is_element());
        let viewport = metas.next().unwrap();
        assert_eq!(
            viewport.attribute("content"),
            Some(
                "width=device-width, initial-scale=1, minimum-scale=1, maximum-scale=1, user-scalable=no"
            )
        );
        let meta = metas.next().unwrap();
        assert_eq!(meta.attribute("name"), Some("kaeri-setup"));
        assert_eq!(meta.attribute("content"), Some("{\"size\":18,\"q\":\"<&>\"}"));
        let body = doc.descendants().find(|n| n.has_tag_name((XHTML_NS, "body"))).unwrap();
        assert_eq!(body.first_element_child().unwrap().text(), Some("One"));
        // A head that was there gets it first too.
        let d = clean_with(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>T</title><meta name=\"viewport\" content=\"width=1200, height=1600\"/></head><body/></html>",
            None,
            false,
        );
        let served = d.served("{}");
        assert!(
            served.contains("<meta name=\"kaeri-setup\" content=\"{}\"/><title>T</title>"),
            "{served}"
        );
        // The book's own viewport is kept by another name, for a fixed-layout page's size.
        assert!(
            served.contains(
                "<meta name=\"kaeri-viewport\" content=\"width=1200, height=1600\"/>"
            ),
            "{served}"
        );
        assert_eq!(served.matches("name=\"viewport\"").count(), 1, "{served}");
    }

    #[test]
    fn html_entities_and_sloppy_markup_still_read() {
        let d = clean_with(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>A&nbsp;b &mdash; c</p></body></html>",
            None,
            false,
        );
        assert!(!d.rebuilt && d.failed.is_none());
        assert!(d.text.contains("A\u{a0}b \u{2014} c"), "{}", d.text);
        let d = clean_with(
            "<html><body><p>One<br>two<p>Three &amp; four<script>x()</script></body></html>",
            None,
            false,
        );
        assert!(d.rebuilt, "{}", d.text);
        assert!(d.text.contains("<p>One</p><p>two</p><p>Three &amp; four</p>"), "{}", d.text);
        assert!(!d.text.contains("x()"));
    }

    #[test]
    fn deep_nesting_is_flattened_and_huge_files_refused() {
        let deep = format!(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body>{}deep text{}</body></html>",
            "<div>".repeat(5000),
            "</div>".repeat(5000)
        );
        let d = clean_with(&deep, None, false);
        assert!(d.rebuilt && d.text.contains("<p>deep text</p>"), "{}", &d.text[..200]);
        let limits = Limits { max_xml_bytes: 1000, ..Limits::default() };
        let clean =
            Clean { path: "a.xhtml", lang: Some("en"), soft_hyphens: false, limits: &limits };
        let big = format!("<html><body><p>{}</p></body></html>", "x".repeat(2000));
        let d = document(big.as_bytes(), &clean);
        assert_eq!(d.failed, Some(Failed::TooLarge));
        assert!(d.text.contains("data-kaeri-failed=\"too-large\""));
        reparse(&d.served("{}"));
        let d = document(b"\xff\xfe\x00", &clean);
        assert_eq!(d.failed, Some(Failed::Unreadable));
    }

    #[test]
    fn text_chapters_are_wrapped_and_cleaned() {
        let limits = Limits::default();
        let clean = Clean {
            path: "text/3.xhtml",
            lang: Some("en"),
            soft_hyphens: false,
            limits: &limits,
        };
        let d = text_chapter(
            "<p>The tower&nbsp;stood.</p><img src=\"https://x.example/a.png\"/><script>x()</script>",
            "Chapter 3",
            &clean,
        );
        let doc = reparse(&d.text);
        assert_eq!(doc.root_element().attribute("lang"), Some("en"));
        assert!(d.text.contains("<title>Chapter 3</title>"));
        assert!(d.text.contains("<p>The tower\u{a0}stood.</p>"), "{}", d.text);
        assert!(!d.text.contains("x.example") && !d.text.contains("script"));
        // A whole page from a source gives its body.
        let d = text_chapter(
            "<html><head><title>x</title></head><body><p>Only this</p></body></html>",
            "T",
            &clean,
        );
        assert!(d.text.contains("<body><p>Only this</p></body>"), "{}", d.text);
    }

    #[test]
    fn languages_and_vertical_styles_are_noticed() {
        let d = clean_with(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"ja\"><head><style>html{-epub-writing-mode:vertical-rl}</style></head><body><p>塔</p></body></html>",
            Some("en"),
            false,
        );
        assert_eq!(d.flow, Some(Flow::VerticalRl));
        let doc = reparse(&d.text);
        assert_eq!(doc.root_element().attribute((XML_NS, "lang")), Some("ja"));
        assert_eq!(
            doc.root_element().attribute("lang"),
            None,
            "its own language is kept as written"
        );
    }

    #[cfg(feature = "hyphenation")]
    #[test]
    fn soft_hyphens_go_into_prose_only() {
        let d = clean_with(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>extraordinarily</p><pre>extraordinarily</pre><p xml:lang=\"ja\">extraordinarily</p><p><ruby>lantern<rt>extraordinarily</rt></ruby></p></body></html>",
            Some("en"),
            true,
        );
        let doc = reparse(&d.text);
        let texts: Vec<&str> =
            doc.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect();
        assert!(texts[0].contains('\u{AD}'), "{texts:?}");
        assert!(texts[1..].iter().all(|t| !t.contains('\u{AD}')), "{texts:?}");
        let off = clean_with(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>extraordinarily</p></body></html>",
            Some("en"),
            false,
        );
        assert!(!off.text.contains('\u{AD}'));
    }

    #[test]
    fn standalone_svg_is_cleaned() {
        let limits = Limits::default();
        let clean =
            Clean { path: "img/a.svg", lang: None, soft_hyphens: true, limits: &limits };
        let out = svg(br#"<svg xmlns="http://www.w3.org/2000/svg" onload="x()"><script>x()</script><rect width="1" height="1"/></svg>"#, &clean).unwrap();
        assert!(
            !out.contains("script") && !out.contains("onload") && out.contains("<rect"),
            "{out}"
        );
    }
}
