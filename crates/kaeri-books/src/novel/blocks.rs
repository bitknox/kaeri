//! A cleaned document as text blocks, for a front end with no web view that lays the text out
//! itself, as the terminal does.
//!
//! Places are named as the web view's paginator names them (`posOf` and `resolve` in
//! `web/paginator.js`), so a place kept on one device opens at the same words on another. A
//! place is an element path from `<body>`, counting element children only, and an offset in the
//! text of everything under that element, counted as JavaScript counts a string's length (UTF-16
//! units) with soft hyphens left out. Every text node counts: whitespace, ruby readings and the
//! text of a hidden heading too, though only what shows is put in a run.
//!
//! The documents are served as XHTML, so the web view's parser keeps every text node as the
//! cleaned text has it, and so does roxmltree here.

use roxmltree::Node;

use super::hyphen::SHY;
use crate::write::xhtml::XHTML_NS;
use crate::{Format, Limits, xml};

/// What a block is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    /// A heading, 1 the highest.
    Heading(u8),
    /// A paragraph of a block quotation.
    Quote,
    /// A list item, or the first paragraph of one.
    ListItem,
    /// Text whose spaces and lines are kept as they are.
    Preformatted,
    /// A rule, or a paragraph of only ornaments such as `* * *`.
    SceneBreak,
    /// A picture, with its description when the book gives one.
    Picture {
        alt: Option<String>,
    },
}

/// A block of a document's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    /// The element the block's place is counted in, from `<body>`: the block's own element, or
    /// for text with no element of its own (text loose in a `<div>` beside paragraphs), the
    /// element it sits in.
    pub path: Vec<u32>,
    /// Where the block's text starts in that element's text: 0 for a block that is its own
    /// element.
    pub offset: u32,
    pub runs: Vec<Run>,
}

/// A run of a block's text in one style, from one text node of the document.
///
/// Every text node of the block is a run, whitespace included, so `offset` counts as the
/// paginator counts. Text that never shows (a style sheet's, a hidden element's, an SVG's) has
/// no run but is counted, so the run after it starts later. Ruby text, `<rt>` and `<rp>` alike,
/// is in runs marked `ruby`: the paginator counts both.
///
/// A `<br>` is a run whose text is `"\n"` and which takes no room in the count: the run after
/// it starts at the same offset. That is how to tell it from a newline in the book's own text,
/// which counts one. A break at a block's end shows nothing and is left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The text, without soft hyphens.
    pub text: String,
    /// Where the run starts in the block's text, in UTF-16 units: `block.offset + offset` is its
    /// place under `block.path`.
    pub offset: u32,
    /// The path of the element the text node sits in.
    pub path: Vec<u32>,
    pub emphasis: bool,
    pub strong: bool,
    pub ruby: bool,
}

/// Elements that make blocks of their own, or hold blocks.
const BLOCKS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "body",
    "caption",
    "center",
    "dd",
    "details",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hgroup",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
];

/// Elements whose text never shows: counted, never put in a run.
const UNSEEN: &[&str] = &["style", "script", "template", "title", "noscript"];

/// A cleaned document's body as blocks, in document order. Empty for a document that does not
/// parse or has no body, as a failed one has nothing in it.
pub fn text_blocks(xhtml: &str, limits: &Limits) -> Vec<Block> {
    let Ok(doc) = xml::parse(xhtml, Format::Epub, limits) else { return Vec::new() };
    let body = doc.root_element().children().find(|c| html(*c) == Some("body"));
    let Some(body) = body else { return Vec::new() };
    let mut out = Vec::new();
    container(body, &mut Vec::new(), &mut out);
    out
}

/// An element's local name, when it is an XHTML element.
fn html<'a>(n: Node<'a, '_>) -> Option<&'a str> {
    (n.is_element() && n.tag_name().namespace().is_none_or(|ns| ns == XHTML_NS))
        .then(|| n.tag_name().name())
}

fn is_block(n: Node<'_, '_>) -> bool {
    html(n).is_some_and(|name| BLOCKS.contains(&name))
}

/// An `<img>`, an `<svg>` or an SVG `<image>`.
fn is_picture(n: Node<'_, '_>) -> bool {
    n.is_element() && matches!(n.tag_name().name(), "img" | "svg" | "image")
}

fn hidden(n: Node<'_, '_>) -> bool {
    n.is_element() && n.attribute("hidden").is_some()
}

/// A string's length as JavaScript counts it, soft hyphens left out.
fn units(s: &str) -> u32 {
    s.chars().filter(|&c| c != SHY).map(char::len_utf16).sum::<usize>() as u32
}

/// The length of all the text under `n`, as the paginator counts it.
fn text_len(n: Node<'_, '_>) -> u32 {
    n.descendants().filter(Node::is_text).map(|t| units(t.text().unwrap_or_default())).sum()
}

fn words(s: &str) -> bool {
    s.chars().any(|c| !c.is_whitespace() && c != SHY)
}

/// Whether `n` has words of its own that show, outside its pictures.
fn shown_words(n: Node<'_, '_>) -> bool {
    n.children().any(|c| {
        if c.is_text() {
            return words(c.text().unwrap_or_default());
        }
        let unseen = html(c).is_some_and(|name| UNSEEN.contains(&name));
        c.is_element() && !is_picture(c) && !hidden(c) && !unseen && shown_words(c)
    })
}

/// An element whose children are blocks to walk into rather than one block's text: one with a
/// block element in it, or one of pictures and no words.
fn holds_blocks(n: Node<'_, '_>) -> bool {
    n.children().any(is_block) || (n.children().any(is_picture) && !shown_words(n))
}

/// A picture's description: its `alt`, its label or its title.
fn alt_of(n: Node<'_, '_>) -> Option<String> {
    let title = || {
        n.children()
            .find(|c| c.is_element() && c.tag_name().name() == "title")
            .and_then(|t| xml::trimmed_text(t))
    };
    ["alt", "aria-label", "title"]
        .iter()
        .find_map(|a| n.attribute(*a).map(xml::collapse).filter(|t| !t.is_empty()))
        .or_else(title)
}

/// Whether a picture shows anything: an `<img>` whose source the cleaning dropped does not.
fn shows(n: Node<'_, '_>) -> bool {
    match n.tag_name().name() {
        "img" => n.attribute("src").is_some(),
        "image" => n.attributes().any(|a| a.name() == "href"),
        _ => true,
    }
}

/// The style text takes from the elements it is in.
#[derive(Debug, Clone, Copy, Default)]
struct Style {
    emphasis: bool,
    strong: bool,
    ruby: bool,
    /// Counted, never shown.
    unseen: bool,
}

impl Style {
    fn inside(self, n: Node<'_, '_>) -> Self {
        let name = n.tag_name().name();
        let xhtml = html(n).is_some();
        Self {
            emphasis: self.emphasis || (xhtml && matches!(name, "em" | "i" | "cite" | "dfn")),
            strong: self.strong || (xhtml && matches!(name, "strong" | "b")),
            ruby: self.ruby || (xhtml && matches!(name, "rt" | "rp")),
            unseen: self.unseen
                || hidden(n)
                || is_picture(n)
                || (xhtml && UNSEEN.contains(&name)),
        }
    }
}

/// Text loose in a container, gathered until the next block.
struct Loose {
    /// Where it starts in the container's text.
    start: u32,
    /// How far into it the next run starts.
    at: u32,
    runs: Vec<Run>,
}

/// Walks an element that holds blocks: each block child is a block, and the text loose between
/// them is a paragraph placed in this element at the offset it starts at.
fn container(n: Node<'_, '_>, path: &mut Vec<u32>, out: &mut Vec<Block>) {
    let mut at = 0u32;
    let mut loose: Option<Loose> = None;
    let mut index = 0u32;
    for c in n.children() {
        if c.is_element() {
            path.push(index);
            index += 1;
            if is_block(c) || is_picture(c) {
                flush(loose.take(), path[..path.len() - 1].to_vec(), out);
                block(c, path, out);
            } else {
                let l = loose.get_or_insert(Loose { start: at, at: 0, runs: Vec::new() });
                inline(c, path, Style::default(), &mut l.at, &mut l.runs);
            }
            path.pop();
            at += text_len(c);
        } else if c.is_text() {
            let l = loose.get_or_insert(Loose { start: at, at: 0, runs: Vec::new() });
            let text = c.text().unwrap_or_default();
            run(text, path, Style::default(), &mut l.at, &mut l.runs);
            at += units(text);
        }
    }
    flush(loose, path.clone(), out);
}

/// Loose text as a paragraph, when it has words.
fn flush(loose: Option<Loose>, path: Vec<u32>, out: &mut Vec<Block>) {
    let Some(Loose { start, at, mut runs }) = loose else { return };
    trim_breaks(&mut runs, at);
    if !runs.iter().any(|r| words(&r.text)) {
        return;
    }
    let kind = if ornament(&runs) { BlockKind::SceneBreak } else { BlockKind::Paragraph };
    out.push(Block { kind, path, offset: start, runs });
}

/// One block element, at `path`.
fn block(n: Node<'_, '_>, path: &mut Vec<u32>, out: &mut Vec<Block>) {
    if hidden(n) {
        return;
    }
    if is_picture(n) {
        if shows(n) {
            out.push(own(path, BlockKind::Picture { alt: alt_of(n) }, Vec::new()));
        }
        return;
    }
    let name = n.tag_name().name();
    match name {
        "hr" => out.push(own(path, BlockKind::SceneBreak, Vec::new())),
        "pre" => leaf(n, path, BlockKind::Preformatted, out),
        _ if holds_blocks(n) => {
            let first = out.len();
            container(n, path, out);
            match name {
                "blockquote" => {
                    for b in &mut out[first..] {
                        if b.kind == BlockKind::Paragraph {
                            b.kind = BlockKind::Quote;
                        }
                    }
                }
                "li" => {
                    if let Some(b) = out.get_mut(first)
                        && b.kind == BlockKind::Paragraph
                    {
                        b.kind = BlockKind::ListItem;
                    }
                }
                _ => {}
            }
        }
        _ => {
            let kind = match name.as_bytes() {
                [b'h', level @ b'1'..=b'6'] => BlockKind::Heading(*level - b'0'),
                b"blockquote" => BlockKind::Quote,
                b"li" => BlockKind::ListItem,
                _ => BlockKind::Paragraph,
            };
            leaf(n, path, kind, out);
        }
    }
}

/// A block element holding text only: one block of its runs. One with no words is the picture
/// it holds, if any, and nothing otherwise.
fn leaf(n: Node<'_, '_>, path: &mut Vec<u32>, kind: BlockKind, out: &mut Vec<Block>) {
    let mut runs = Vec::new();
    let mut at = 0;
    children(n, path, Style::default(), &mut at, &mut runs);
    trim_breaks(&mut runs, at);
    if !runs.iter().any(|r| words(&r.text)) {
        let picture = n.descendants().find(|d| is_picture(*d) && shows(*d));
        if let Some(picture) = picture.filter(|p| !p.ancestors().any(hidden)) {
            out.push(own(path, BlockKind::Picture { alt: alt_of(picture) }, Vec::new()));
        }
        return;
    }
    let kind = match kind {
        BlockKind::Paragraph | BlockKind::Quote if ornament(&runs) => BlockKind::SceneBreak,
        kind => kind,
    };
    out.push(own(path, kind, runs));
}

/// A block that is its own element.
fn own(path: &[u32], kind: BlockKind, runs: Vec<Run>) -> Block {
    Block { kind, path: path.to_vec(), offset: 0, runs }
}

/// The runs of an inline element and everything in it, `path` being the element's own.
fn inline(
    n: Node<'_, '_>,
    path: &mut Vec<u32>,
    style: Style,
    at: &mut u32,
    runs: &mut Vec<Run>,
) {
    let style = style.inside(n);
    if html(n) == Some("br") {
        if !style.unseen {
            let parent = path[..path.len().saturating_sub(1)].to_vec();
            runs.push(Run { text: "\n".into(), offset: *at, path: parent, ..plain(style) });
        }
        return;
    }
    children(n, path, style, at, runs);
}

fn children(
    n: Node<'_, '_>,
    path: &mut Vec<u32>,
    style: Style,
    at: &mut u32,
    runs: &mut Vec<Run>,
) {
    let mut index = 0u32;
    for c in n.children() {
        if c.is_element() {
            path.push(index);
            index += 1;
            inline(c, path, style, at, runs);
            path.pop();
        } else if c.is_text() {
            run(c.text().unwrap_or_default(), path, style, at, runs);
        }
    }
}

/// One text node: a run when it shows, counted either way.
fn run(text: &str, path: &[u32], style: Style, at: &mut u32, runs: &mut Vec<Run>) {
    let len = units(text);
    if !style.unseen && len > 0 {
        let text: String = text.chars().filter(|&c| c != SHY).collect();
        runs.push(Run { text, offset: *at, path: path.to_vec(), ..plain(style) });
    }
    *at += len;
}

fn plain(style: Style) -> Run {
    Run {
        text: String::new(),
        offset: 0,
        path: Vec::new(),
        emphasis: style.emphasis,
        strong: style.strong,
        ruby: style.ruby,
    }
}

/// Line breaks at a block's end show nothing. A `<br>` takes no room, so one last of all starts
/// where the text ends, `total`; a newline of the book's own text starts one before.
fn trim_breaks(runs: &mut Vec<Run>, total: u32) {
    while runs.last().is_some_and(|last| last.text == "\n" && last.offset == total) {
        runs.pop();
    }
}

/// A paragraph of only ornaments, a scene break: `* * *`, `◇`, `———`.
fn ornament(runs: &[Run]) -> bool {
    let marks: Vec<char> =
        runs.iter().flat_map(|r| r.text.chars()).filter(|c| !c.is_whitespace()).collect();
    !marks.is_empty()
        && marks.len() <= 40
        && marks.iter().all(|&c| {
            matches!(
                c,
                '*' | '#'
                    | '~'
                    | '='
                    | '+'
                    | '_'
                    | '-'
                    | '·'
                    | '•'
                    | '§'
                    | '※'
                    | '⁂'
                    | '⁑'
                    | '∴'
                    | '∵'
                    | '＊'
                    | '＃'
                    | '～'
                    | '–'
                    | '—'
                    | '―'
            ) || ('\u{2500}'..='\u{27BF}').contains(&c)
                || ('\u{2B00}'..='\u{2BFF}').contains(&c)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(body: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"{XHTML_NS}\"><head><title>A made-up book</title></head><body>{body}</body></html>\n"
        )
    }

    fn blocks(body: &str) -> Vec<Block> {
        text_blocks(&doc(body), &Limits::default())
    }

    fn text(block: &Block) -> String {
        block.runs.iter().map(|r| r.text.as_str()).collect()
    }

    fn run(text: &str, offset: u32, path: &[u32]) -> Run {
        Run {
            text: text.into(),
            offset,
            path: path.to_vec(),
            emphasis: false,
            strong: false,
            ruby: false,
        }
    }

    /// The paginator's `resolve`: the text node a place names and the offset in its clean text.
    fn resolve<'a, 'i>(
        body: Node<'a, 'i>,
        path: &[u32],
        offset: u32,
    ) -> Option<(Node<'a, 'i>, u32)> {
        let mut el = body;
        for &i in path {
            match el.children().filter(Node::is_element).nth(i as usize) {
                Some(next) => el = next,
                None => break,
            }
        }
        let mut left = offset;
        for t in el.descendants().filter(Node::is_text) {
            let len = units(t.text().unwrap_or_default());
            if left < len {
                return Some((t, left));
            }
            left -= len;
        }
        None
    }

    /// The paginator's `posOf` for a text node at an offset into its clean text.
    fn pos_of(body: Node<'_, '_>, node: Node<'_, '_>, offset: u32) -> (Vec<u32>, u32) {
        let el = node.parent().unwrap();
        let mut path = Vec::new();
        let mut e = el;
        while e != body {
            path.insert(0, e.prev_siblings().filter(Node::is_element).count() as u32 - 1);
            e = e.parent().unwrap();
        }
        let before: u32 = el
            .descendants()
            .filter(Node::is_text)
            .take_while(|t| *t != node)
            .map(|t| units(t.text().unwrap_or_default()))
            .sum();
        (path, before + offset)
    }

    /// Every run's place, as the block and as the run names it, is where the paginator finds
    /// the run's text.
    fn places_agree(xhtml: &str) {
        let parsed = roxmltree::Document::parse_with_options(
            xhtml,
            roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() },
        )
        .unwrap();
        let body = parsed.descendants().find(|n| n.has_tag_name((XHTML_NS, "body"))).unwrap();
        for block in text_blocks(xhtml, &Limits::default()) {
            for (i, r) in block.runs.iter().enumerate() {
                let next_same = block.runs.get(i + 1).is_some_and(|n| n.offset == r.offset);
                if r.text == "\n" && next_same {
                    continue;
                }
                let (node, at) = resolve(body, &block.path, block.offset + r.offset)
                    .unwrap_or_else(|| panic!("{r:?} in {block:?} names no text"));
                let clean: String =
                    node.text().unwrap().chars().filter(|&c| c != SHY).collect();
                let rest: Vec<u16> = clean.encode_utf16().skip(at as usize).collect();
                let want: Vec<u16> = r.text.encode_utf16().collect();
                assert!(rest.starts_with(&want), "{r:?} in {block:?} finds {clean:?} at {at}");
                let (path, _) = pos_of(body, node, at);
                assert_eq!(path, r.path, "{r:?} names its element");
            }
        }
    }

    #[test]
    fn paragraphs_and_headings_name_their_places_as_the_paginator_does() {
        let b = blocks(
            "\n<h1>One</h1>\n<p>The <em>lantern</em> keep\u{AD}er's stair.</p>\n<p>Two <strong>bells</strong> 😀 <i>rang</i>.</p>",
        );
        assert_eq!(b.len(), 3, "{b:#?}");
        assert_eq!(b[0].kind, BlockKind::Heading(1));
        assert_eq!((b[0].path.as_slice(), b[0].offset), (&[0][..], 0));
        assert_eq!(b[0].runs, vec![run("One", 0, &[0])]);
        assert_eq!(b[1].kind, BlockKind::Paragraph);
        assert_eq!(b[1].path, [1]);
        assert_eq!(
            b[1].runs,
            vec![
                run("The ", 0, &[1]),
                Run { emphasis: true, ..run("lantern", 4, &[1, 0]) },
                // The soft hyphen is neither in the text nor counted.
                run(" keeper's stair.", 11, &[1]),
            ]
        );
        // The emoji is two units, as JavaScript counts it.
        let rang = b[2].runs.iter().find(|r| r.text == "rang").unwrap();
        assert_eq!((rang.offset, rang.path.as_slice(), rang.emphasis), (13, &[2, 1][..], true));
        assert!(b[2].runs.iter().any(|r| r.strong && r.text == "bells"));
    }

    #[test]
    fn text_loose_beside_blocks_is_a_paragraph_placed_in_its_container() {
        let body = "<div>Loose words <b>here</b><p>Inner</p>after <i>it</i></div>";
        let b = blocks(body);
        assert_eq!(b.len(), 3, "{b:#?}");
        assert_eq!(
            (b[0].kind.clone(), b[0].path.as_slice(), b[0].offset),
            (BlockKind::Paragraph, &[0][..], 0)
        );
        assert_eq!(
            b[0].runs,
            vec![
                run("Loose words ", 0, &[0]),
                Run { strong: true, ..run("here", 12, &[0, 0]) }
            ]
        );
        assert_eq!(
            (b[1].path.as_slice(), b[1].offset, text(&b[1])),
            (&[0, 1][..], 0, "Inner".into())
        );
        // After "Loose words here" and "Inner": 21 units into the div's text.
        assert_eq!((b[2].path.as_slice(), b[2].offset), (&[0][..], 21));
        assert_eq!(
            b[2].runs,
            vec![run("after ", 0, &[0]), Run { emphasis: true, ..run("it", 6, &[0, 2]) }]
        );
        places_agree(&doc(body));
    }

    #[test]
    fn a_line_break_takes_no_room_and_one_at_the_end_is_left_out() {
        let b = blocks("<p>Line one<br/>Line two<br/></p>");
        assert_eq!(
            b[0].runs,
            vec![run("Line one", 0, &[0]), run("\n", 8, &[0]), run("Line two", 8, &[0])]
        );
        // A newline of the book's own text counts.
        let pre = blocks("<pre>a\nb<b>c</b>\n</pre>");
        assert_eq!(pre[0].kind, BlockKind::Preformatted);
        assert_eq!(
            pre[0].runs,
            vec![
                run("a\nb", 0, &[0]),
                Run { strong: true, ..run("c", 3, &[0, 0]) },
                run("\n", 4, &[0])
            ]
        );
    }

    #[test]
    fn rules_and_lines_of_ornaments_are_scene_breaks() {
        let b = blocks("<p>Before.</p><hr/><p>* * *</p><p class=\"c\">◇</p><p>…</p><p>No.</p>");
        let kinds: Vec<_> = b.iter().map(|b| b.kind.clone()).collect();
        assert_eq!(
            kinds,
            [
                BlockKind::Paragraph,
                BlockKind::SceneBreak,
                BlockKind::SceneBreak,
                BlockKind::SceneBreak,
                BlockKind::Paragraph,
                BlockKind::Paragraph,
            ]
        );
        assert_eq!(text(&b[2]), "* * *", "the ornaments stay, to be drawn or not");
    }

    #[test]
    fn pictures_say_their_description_and_a_caption_follows() {
        let b = blocks(
            "<figure><img src=\"lamp.png\" alt=\" A  lamp \"/><figcaption>The lamp</figcaption></figure><p><img src=\"stair.png\"/></p><img/><div><span><img src=\"tide.png\" title=\"The tide\"/></span></div>",
        );
        assert_eq!(b.len(), 4, "{b:#?}");
        assert_eq!(b[0].kind, BlockKind::Picture { alt: Some("A lamp".into()) });
        assert_eq!(b[0].path, [0, 0]);
        assert_eq!(
            (b[1].kind.clone(), b[1].path.as_slice()),
            (BlockKind::Paragraph, &[0, 1][..])
        );
        assert_eq!(
            (b[2].kind.clone(), b[2].path.as_slice()),
            (BlockKind::Picture { alt: None }, &[1, 0][..])
        );
        // A picture with no source shows nothing; one wrapped in inline elements is its block's.
        assert_eq!(b[3].kind, BlockKind::Picture { alt: Some("The tide".into()) });
        assert_eq!(b[3].path, [3]);
    }

    #[test]
    fn ruby_text_is_counted_and_marked() {
        let body = "<p><ruby>灯<rp>(</rp><rt>ひ</rt><rp>)</rp></ruby>を見る</p>";
        let b = blocks(body);
        let ruby =
            |text: &str, offset, path: &[u32]| Run { ruby: true, ..run(text, offset, path) };
        assert_eq!(
            b[0].runs,
            vec![
                run("灯", 0, &[0, 0]),
                ruby("(", 1, &[0, 0, 0]),
                ruby("ひ", 2, &[0, 0, 1]),
                ruby(")", 3, &[0, 0, 2]),
                run("を見る", 4, &[0]),
            ]
        );
        places_agree(&doc(body));
    }

    #[test]
    fn hidden_text_and_styles_are_counted_but_not_shown() {
        let body = "<h4 hidden=\"hidden\">Gone</h4><p>a<span hidden=\"hidden\">bc</span>d<style>p{}</style>e</p>";
        let b = blocks(body);
        assert_eq!(b.len(), 1, "{b:#?}");
        assert_eq!(b[0].path, [1]);
        assert_eq!(b[0].runs, vec![run("a", 0, &[1]), run("d", 3, &[1]), run("e", 7, &[1])]);
        places_agree(&doc(body));
    }

    #[test]
    fn quotes_and_lists_keep_what_they_are_when_they_hold_paragraphs() {
        let b = blocks(
            "<blockquote><p>Said once.</p><p>Said twice.</p></blockquote><blockquote>Short.</blockquote><ul><li>Plain</li><li><p>First</p><p>Second</p></li></ul>",
        );
        let kinds: Vec<_> = b.iter().map(|b| (b.kind.clone(), text(b))).collect();
        assert_eq!(
            kinds,
            [
                (BlockKind::Quote, "Said once.".into()),
                (BlockKind::Quote, "Said twice.".into()),
                (BlockKind::Quote, "Short.".into()),
                (BlockKind::ListItem, "Plain".into()),
                (BlockKind::ListItem, "First".into()),
                (BlockKind::Paragraph, "Second".into()),
            ]
        );
    }

    #[test]
    fn a_whole_chapter_s_places_agree_with_the_paginator() {
        places_agree(&doc(
            "\n  <section>\n    <h2>The <em>Pier</em></h2>\n    Loose <a href=\"#n\">note</a> text\n    <p>Wa\u{AD}ter <b>under <i>the</i></b> boards.<br/>Then 😀 quiet.</p>\n    <div><p>Deep</p> tail</div>\n  </section>\n  <p>* * *</p>\n",
        ));
    }

    #[test]
    fn a_document_that_does_not_parse_has_no_blocks() {
        assert!(text_blocks("<html><body><p>unclosed", &Limits::default()).is_empty());
    }
}
