//! A light novel's text laid out in cells: the engine's text blocks wrapped at a column width,
//! each line kept with the place in the book it starts at.
//!
//! Words wrap at the width and a word longer than the width is cut where it has to be.
//! Chinese, Japanese and Korean text breaks between its characters, but never before closing
//! punctuation or after opening punctuation. Paragraphs start with a two-column indent, headings
//! are bold with a blank line before and after, emphasis is italic and strong text bold.
//! Quotes are set in from both sides, list items carry a bullet, preformatted text keeps its
//! spaces and lines, a scene break is a centred line, and a picture is its description on a line
//! of its own. A `<br>` starts a new line.
//!
//! Line spacing in cells is a choice between Tight, no line between paragraphs, and Open, a
//! blank line between every block. Justify spreads the spaces of every line that wraps so it
//! ends at the width; the last line of a paragraph and a line that ends at a `<br>` stay ragged.
//!
//! Places are the engine's [`NovelPos`]: a line knows its block and the UTF-16 offset in that
//! block's text it starts at, and a place finds its line through [`novel_text_place`].

use std::ops::Range;

use kaeri_i18n::t;
use kaeri_model::reader::{NovelPos, NovelTextDoc, TextBlock, TextBlockKind, novel_text_place};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A paragraph's first line starts this far in.
pub const INDENT: u16 = 2;
/// A quote is set in this far from both sides.
const QUOTE: u16 = 4;
/// A list item's text starts this far in, its bullet two columns before.
const ITEM: u16 = 4;
/// A tab in preformatted text runs to the next multiple of this.
const TAB: usize = 4;

/// How a stretch of text is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Look {
    pub emphasis: bool,
    pub strong: bool,
    /// Quieter than the text: a ruby reading, a picture's line, a scene break.
    pub dim: bool,
}

/// A stretch of a line in one look.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub look: Look,
}

/// One line of the laid out text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLine {
    /// The document, as an index into the text's documents.
    pub doc: usize,
    /// The block the line belongs to; a blank line belongs to the block after it.
    pub block: usize,
    /// Where in the block's text the line starts, in UTF-16 units.
    pub at: u32,
    /// The column in the text column the line starts at.
    pub x: u16,
    /// A list item's bullet, drawn two columns before `x`.
    pub mark: Option<String>,
    /// Empty for a blank line.
    pub spans: Vec<Span>,
}

impl TextLine {
    pub fn is_blank(&self) -> bool {
        self.spans.is_empty()
    }

    /// The line's text, without its look: for tests and for measuring.
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}

/// What the text is laid out for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    /// The text column's width in cells.
    pub width: u16,
    /// A blank line between every block, not only around headings and scene breaks.
    pub open: bool,
    pub justify: bool,
}

/// The text of every document, laid out at one shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLayout {
    pub shape: Shape,
    pub lines: Vec<TextLine>,
    /// Each document's lines.
    pub docs: Vec<Range<usize>>,
}

impl TextLayout {
    /// The document line `line` is in.
    pub fn doc_of(&self, line: usize) -> usize {
        self.docs.iter().position(|r| r.contains(&line)).unwrap_or_else(|| {
            // Past the end: the last document with lines.
            self.docs.iter().rposition(|r| !r.is_empty()).unwrap_or(0)
        })
    }

    /// The place line `line` starts at.
    pub fn pos_of(&self, docs: &[NovelTextDoc], line: usize) -> Option<NovelPos> {
        let line = self.lines.get(line)?;
        let doc = docs.get(line.doc)?;
        match doc.blocks.get(line.block) {
            Some(block) => Some(NovelPos {
                spine: doc.spine,
                path: block.at.path.clone(),
                offset: block.at.offset + line.at,
            }),
            None => Some(NovelPos { spine: doc.spine, path: Vec::new(), offset: 0 }),
        }
    }

    /// The line a place falls on: the line of its block that holds its offset. A place in a
    /// block that has no lines falls on the next line there is.
    pub fn line_of(&self, docs: &[NovelTextDoc], pos: &NovelPos) -> Option<usize> {
        let (doc, block, offset) = novel_text_place(docs, pos)?;
        let range = self.docs.get(doc)?.clone();
        let text = |i: &usize| !self.lines[*i].is_blank();
        let held = range
            .clone()
            .rev()
            .filter(text)
            .find(|i| self.lines[*i].block == block && self.lines[*i].at <= offset);
        held.or_else(|| range.clone().filter(text).find(|i| self.lines[*i].block >= block))
            .or_else(|| range.clone().rev().find(text))
            .or_else(|| (!range.is_empty()).then_some(range.start))
    }
}

/// Lay `docs` out at `shape`.
pub fn lay_out(docs: &[NovelTextDoc], shape: Shape) -> TextLayout {
    let width = shape.width.max(1);
    let mut lines = Vec::new();
    let mut ranges = Vec::with_capacity(docs.len());
    for (d, doc) in docs.iter().enumerate() {
        let start = lines.len();
        let mut first = true;
        for (b, block) in doc.blocks.iter().enumerate() {
            let set = set_block(block, Shape { width, ..shape });
            if set.is_empty() {
                continue;
            }
            let spaced =
                matches!(block.kind, TextBlockKind::Heading { .. } | TextBlockKind::SceneBreak);
            let previous_spaced = lines[start..].last().is_some_and(|l: &TextLine| {
                l.is_blank() || doc.blocks.get(l.block).is_some_and(|p| spaced_kind(&p.kind))
            });
            // A heading opens with a blank line even at the top of its document.
            let blank_before = if first {
                matches!(block.kind, TextBlockKind::Heading { .. })
            } else {
                shape.open || spaced || previous_spaced
            };
            if blank_before && !lines[start..].last().is_some_and(TextLine::is_blank) {
                lines.push(blank(d, b));
            }
            for line in set {
                lines.push(TextLine { doc: d, block: b, ..line });
            }
            first = false;
        }
        // No blank line ends a document.
        while lines.len() > start && lines.last().is_some_and(TextLine::is_blank) {
            lines.pop();
        }
        ranges.push(start..lines.len());
    }
    TextLayout { shape: Shape { width, ..shape }, lines, docs: ranges }
}

/// A heading and a scene break keep a blank line after them, in either spacing.
fn spaced_kind(kind: &TextBlockKind) -> bool {
    matches!(kind, TextBlockKind::Heading { .. } | TextBlockKind::SceneBreak)
}

fn blank(doc: usize, block: usize) -> TextLine {
    TextLine { doc, block, at: 0, x: 0, mark: None, spans: Vec::new() }
}

/// One grapheme of a block, or a break.
#[derive(Debug, Clone, PartialEq)]
enum Piece {
    Glyph {
        text: String,
        cells: usize,
        at: u32,
        look: Look,
        wide: bool,
    },
    Space {
        at: u32,
    },
    /// A `<br>`, or a book's own newline in preformatted text.
    Break {
        at: u32,
    },
}

/// A block's lines, with `doc` and `block` left for the caller.
fn set_block(block: &TextBlock, shape: Shape) -> Vec<TextLine> {
    let width = usize::from(shape.width);
    match &block.kind {
        TextBlockKind::Preformatted => preformatted(block, width),
        TextBlockKind::SceneBreak => {
            let text: String =
                block.runs.iter().filter(|r| !r.ruby).map(|r| r.text.as_str()).collect();
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            let text = if text.is_empty() || text.width() > width {
                "*   *   *".to_owned() // i18n-skip: an ornament
            } else {
                text
            };
            let x = width.saturating_sub(text.width()) / 2;
            let look = Look { dim: true, ..Look::default() };
            vec![TextLine {
                doc: 0,
                block: 0,
                at: 0,
                x: x as u16,
                mark: None,
                spans: vec![Span { text, look }],
            }]
        }
        TextBlockKind::Picture { alt } => {
            let words = match alt.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
                Some(alt) => t::tui_reader_novel_picture(alt),
                None => t::tui_reader_novel_picture_plain(),
            };
            let look = Look { dim: true, emphasis: true, strong: false };
            let pieces = plain_pieces(&words, look);
            let mut lines = wrap(&pieces, width, 0, 0, false);
            for line in &mut lines {
                let cells: usize = line.spans.iter().map(|s| s.text.width()).sum();
                line.x = (width.saturating_sub(cells) / 2) as u16;
                line.at = 0;
            }
            lines
        }
        kind => {
            let pieces = pieces_of(block, matches!(kind, TextBlockKind::Heading { .. }));
            let (first, rest, right) = match kind {
                TextBlockKind::Paragraph => (INDENT, 0, 0),
                TextBlockKind::Quote => (QUOTE, QUOTE, QUOTE),
                TextBlockKind::ListItem => (ITEM, ITEM, 0),
                _ => (0, 0, 0),
            };
            let room = width.saturating_sub(usize::from(right)).max(1);
            let mut lines = wrap(
                &pieces,
                room,
                usize::from(first).min(room - 1),
                usize::from(rest).min(room - 1),
                shape.justify,
            );
            if matches!(kind, TextBlockKind::ListItem)
                && let Some(line) = lines.first_mut()
            {
                line.mark = Some("•".to_owned()); // i18n-skip: a bullet
            }
            lines
        }
    }
}

/// A block's runs as pieces: graphemes with their offsets and looks, spaces collapsed as a web
/// page collapses them, `<br>` as breaks and ruby readings in brackets after their text.
fn pieces_of(block: &TextBlock, heading: bool) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let runs = &block.runs;
    let mut i = 0;
    while i < runs.len() {
        let run = &runs[i];
        // A `<br>` takes no room in the count: the run after it starts where it does.
        if run.text == "\n" && runs.get(i + 1).is_some_and(|next| next.offset == run.offset) {
            pieces.push(Piece::Break { at: run.offset });
            i += 1;
            continue;
        }
        if run.ruby {
            // The reading and its brackets come as ruby runs together; the brackets go, and
            // the reading shows quieter in brackets of the terminal's own.
            let at = run.offset;
            let mut reading = String::new();
            while let Some(r) = runs.get(i).filter(|r| r.ruby) {
                reading.push_str(&r.text);
                i += 1;
            }
            let reading: String = reading
                .chars()
                .filter(|c| !matches!(c, '(' | ')' | '（' | '）') && !c.is_whitespace())
                .collect();
            if !reading.is_empty() {
                let text = format!("({reading})"); // i18n-skip: brackets round a reading
                let cells = text.width();
                let look = Look { dim: true, ..Look::default() };
                // Glued to its text: no break before it.
                pieces.push(Piece::Glyph { text, cells, at, look, wide: false });
            }
            continue;
        }
        let look = Look { emphasis: run.emphasis, strong: run.strong || heading, dim: false };
        let mut at = run.offset;
        for g in run.text.graphemes(true) {
            if g.chars().all(|c| c.is_whitespace() && c != '\u{a0}') {
                pieces.push(Piece::Space { at });
            } else {
                let text = if g == "\u{a0}" { " ".to_owned() } else { g.to_owned() };
                let cells = text.width();
                pieces.push(Piece::Glyph { text, cells, at, look, wide: cells > 1 });
            }
            at += g.encode_utf16().count() as u32;
        }
        i += 1;
    }
    pieces
}

/// Words in one look, for a line of the terminal's own.
fn plain_pieces(text: &str, look: Look) -> Vec<Piece> {
    text.graphemes(true)
        .map(|g| {
            if g.chars().all(char::is_whitespace) {
                Piece::Space { at: 0 }
            } else {
                Piece::Glyph {
                    text: g.to_owned(),
                    cells: g.width(),
                    at: 0,
                    look,
                    wide: g.width() > 1,
                }
            }
        })
        .collect()
}

/// Closing punctuation a line never starts with.
fn closes(text: &str) -> bool {
    text.chars().next().is_some_and(|c| {
        "。、，．！？：；」』）〉》】〕］｝ー…‥・ぁぃぅぇぉっゃゅょァィゥェォッャュョ,.!?:;)]}"
            .contains(c)
    })
}

/// Opening punctuation a line never ends with.
fn opens(text: &str) -> bool {
    text.chars().next().is_some_and(|c| "「『（〈《【〔［｛([{".contains(c))
}

/// Whether a line may break between two glyphs that have no space between them: around a wide
/// character, and after a hyphen or a dash, but never before closing punctuation or after
/// opening punctuation.
fn breaks_between(before: &Piece, after: &Piece) -> bool {
    let (
        Piece::Glyph { text: a, wide: a_wide, look: a_look, .. },
        Piece::Glyph { text: b, wide: b_wide, look: b_look, .. },
    ) = (before, after)
    else {
        return false;
    };
    let ruby = |text: &str, look: &Look| look.dim && text.starts_with('(');
    // A ruby reading stays with its text.
    if ruby(b, b_look) || closes(b) || (opens(a) && !ruby(a, a_look)) {
        return false;
    }
    *a_wide || *b_wide || ((a == "-" || a == "—" || a == "–") && !closes(b))
}

/// A word: glyphs no line may break inside, unless the word is wider than a line.
struct Word<'a> {
    glyphs: Vec<&'a Piece>,
    cells: usize,
}

/// One thing in a line being filled.
enum Item<'a> {
    Glyph(&'a Piece),
    Gap,
}

/// Fill lines `width` wide from `pieces`: the first `first` columns in, the rest `rest` in.
fn wrap(
    pieces: &[Piece],
    width: usize,
    first: usize,
    rest: usize,
    justify: bool,
) -> Vec<TextLine> {
    // Words, with whether a space goes before each and whether a break comes after.
    let mut words: Vec<(bool, Word<'_>, bool)> = Vec::new();
    let mut space = false;
    let mut current: Option<Word<'_>> = None;
    let mut last: Option<&Piece> = None;
    for piece in pieces {
        match piece {
            Piece::Space { .. } => {
                if let Some(word) = current.take() {
                    words.push((std::mem::take(&mut space), word, false));
                }
                space = true;
            }
            Piece::Break { .. } => {
                if let Some(word) = current.take() {
                    words.push((std::mem::take(&mut space), word, true));
                } else if let Some(prev) = words.last_mut() {
                    prev.2 = true;
                }
                space = false;
            }
            Piece::Glyph { cells, .. } => {
                if let Some(word) = current.as_mut() {
                    if last.is_some_and(|l| breaks_between(l, piece)) {
                        let word =
                            current.take().unwrap_or(Word { glyphs: Vec::new(), cells: 0 });
                        words.push((std::mem::take(&mut space), word, false));
                        current = Some(Word { glyphs: vec![piece], cells: *cells });
                    } else {
                        word.glyphs.push(piece);
                        word.cells += cells;
                    }
                } else {
                    current = Some(Word { glyphs: vec![piece], cells: *cells });
                }
            }
        }
        last = Some(piece);
    }
    if let Some(word) = current.take() {
        words.push((space, word, false));
    }

    let mut lines: Vec<(usize, Vec<Item<'_>>, bool)> = Vec::new();
    let mut indent = first;
    let mut line: Vec<Item<'_>> = Vec::new();
    let mut used = indent;
    for (space, word, broken) in words {
        if !line.is_empty() && used + usize::from(space) + word.cells > width {
            lines.push((indent, std::mem::take(&mut line), true));
            indent = rest;
            used = indent;
        }
        // The space a line breaks at goes with the break.
        let gap = usize::from(space && !line.is_empty());
        if line.is_empty() && used + word.cells > width {
            // Longer than a line: cut at the width, as many times as it takes.
            for glyph in word.glyphs {
                let Piece::Glyph { cells, .. } = glyph else { continue };
                if !line.is_empty() && used + cells > width {
                    lines.push((indent, std::mem::take(&mut line), false));
                    indent = rest;
                    used = indent;
                }
                line.push(Item::Glyph(glyph));
                used += cells;
            }
        } else {
            if gap == 1 {
                line.push(Item::Gap);
                used += 1;
            }
            line.extend(word.glyphs.into_iter().map(Item::Glyph));
            used += word.cells;
        }
        if broken {
            lines.push((indent, std::mem::take(&mut line), false));
            indent = rest;
            used = indent;
        }
    }
    if !line.is_empty() {
        lines.push((indent, line, false));
    }

    lines
        .into_iter()
        .map(|(x, items, wrapped)| {
            let cells: usize = items
                .iter()
                .map(|i| match i {
                    Item::Glyph(Piece::Glyph { cells, .. }) => *cells,
                    _ => 1,
                })
                .sum();
            let gaps = items.iter().filter(|i| matches!(i, Item::Gap)).count();
            let extra = if justify && wrapped && gaps > 0 {
                width.saturating_sub(x + cells)
            } else {
                0
            };
            let at = items
                .iter()
                .find_map(|i| match i {
                    Item::Glyph(Piece::Glyph { at, .. }) => Some(*at),
                    _ => None,
                })
                .unwrap_or(0);
            let mut spans: Vec<Span> = Vec::new();
            let mut gap_index = 0;
            for item in items {
                let (text, look) = match item {
                    Item::Glyph(Piece::Glyph { text, look, .. }) => (text.clone(), *look),
                    Item::Glyph(_) => continue,
                    Item::Gap => {
                        let wide = 1 + extra / gaps + usize::from(gap_index < extra % gaps);
                        gap_index += 1;
                        (" ".repeat(wide), Look::default())
                    }
                };
                match spans.last_mut() {
                    Some(span) if span.look == look => span.text.push_str(&text),
                    _ => spans.push(Span { text, look }),
                }
            }
            TextLine { doc: 0, block: 0, at, x: x as u16, mark: None, spans }
        })
        .collect()
}

/// Preformatted text: its spaces kept, a line for each of its own, cut where a line is wider
/// than the column.
fn preformatted(block: &TextBlock, width: usize) -> Vec<TextLine> {
    let mut lines: Vec<TextLine> = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut cells = 0;
    let mut start: Option<u32> = None;
    let mut push = |spans: &mut Vec<Span>, cells: &mut usize, start: &mut Option<u32>| {
        lines.push(TextLine {
            doc: 0,
            block: 0,
            at: start.take().unwrap_or(0),
            x: 0,
            mark: None,
            spans: if spans.is_empty() {
                // A blank line of the text's own still takes its row.
                vec![Span { text: String::new(), look: Look::default() }]
            } else {
                std::mem::take(spans)
            },
        });
        *cells = 0;
    };
    let runs = &block.runs;
    for (i, run) in runs.iter().enumerate() {
        if run.ruby {
            continue;
        }
        let look = Look { emphasis: run.emphasis, strong: run.strong, dim: false };
        if run.text == "\n" && runs.get(i + 1).is_some_and(|n| n.offset == run.offset) {
            start.get_or_insert(run.offset);
            push(&mut spans, &mut cells, &mut start);
            continue;
        }
        let mut at = run.offset;
        for g in run.text.graphemes(true) {
            let units = g.encode_utf16().count() as u32;
            if g == "\n" || g == "\r\n" {
                start.get_or_insert(at);
                push(&mut spans, &mut cells, &mut start);
                at += units;
                continue;
            }
            let text = if g == "\t" { " ".repeat(TAB - cells % TAB) } else { g.to_owned() };
            let w = text.width();
            if cells + w > width && cells > 0 {
                push(&mut spans, &mut cells, &mut start);
            }
            start.get_or_insert(at);
            match spans.last_mut() {
                Some(span) if span.look == look => span.text.push_str(&text),
                _ => spans.push(Span { text, look }),
            }
            cells += w;
            at += units;
        }
    }
    if !spans.is_empty() {
        push(&mut spans, &mut cells, &mut start);
    }
    // The text's own blank lines at its ends are its margins, which the layout already keeps.
    while lines.last().is_some_and(|l| l.text().trim().is_empty()) {
        lines.pop();
    }
    while lines.first().is_some_and(|l| l.text().trim().is_empty()) {
        lines.remove(0);
    }
    lines
}

#[cfg(test)]
mod tests {
    use kaeri_model::reader::TextRun;

    use super::*;

    fn run(text: &str, offset: u32) -> TextRun {
        TextRun {
            text: text.into(),
            offset,
            path: Vec::new(),
            emphasis: false,
            strong: false,
            ruby: false,
        }
    }

    fn block(kind: TextBlockKind, path: u32, runs: Vec<TextRun>) -> TextBlock {
        TextBlock { kind, at: NovelPos { spine: 2, path: vec![path], offset: 0 }, runs }
    }

    fn para(path: u32, text: &str) -> TextBlock {
        block(TextBlockKind::Paragraph, path, vec![run(text, 0)])
    }

    fn doc(blocks: Vec<TextBlock>) -> Vec<NovelTextDoc> {
        vec![NovelTextDoc { spine: 2, blocks }]
    }

    fn shape(width: u16) -> Shape {
        Shape { width, open: false, justify: false }
    }

    fn texts(layout: &TextLayout) -> Vec<String> {
        layout
            .lines
            .iter()
            .map(|l| {
                let mark = l.mark.as_deref().map_or(String::new(), |m| format!("{m} "));
                let pad = usize::from(l.x).saturating_sub(mark.width());
                format!("{}{mark}{}", " ".repeat(pad), l.text())
            })
            .collect()
    }

    #[test]
    fn words_wrap_at_the_width_and_a_paragraph_starts_two_in() {
        let docs =
            doc(vec![para(0, "The lamp room sat at the top of the stairs, past the door.")]);
        let layout = lay_out(&docs, shape(20));
        assert_eq!(
            texts(&layout),
            ["  The lamp room sat", "at the top of the", "stairs, past the", "door."]
        );
        assert!(layout.lines.iter().all(|l| l.text().width() + usize::from(l.x) <= 20));
    }

    #[test]
    fn spaces_collapse_and_a_br_starts_a_new_line() {
        let runs = vec![run("One  two\n three", 0), run("\n", 15), run("four", 15)];
        let docs = doc(vec![block(TextBlockKind::Paragraph, 0, runs)]);
        let layout = lay_out(&docs, shape(40));
        assert_eq!(texts(&layout), ["  One two three", "four"]);
        assert_eq!(layout.lines[1].at, 15, "the line after the break starts where it does");
    }

    #[test]
    fn headings_are_bold_with_blank_lines_and_emphasis_is_italic() {
        let heading =
            block(TextBlockKind::Heading { level: 2 }, 0, vec![run("The Lamp Room", 0)]);
        let mut runs = vec![run("which ones still ", 0), run("remember", 17), run(" how.", 25)];
        runs[1].emphasis = true;
        let docs =
            doc(vec![heading, block(TextBlockKind::Paragraph, 1, runs), para(2, "Next.")]);
        let layout = lay_out(&docs, shape(40));
        assert_eq!(
            texts(&layout),
            ["", "The Lamp Room", "", "  which ones still remember how.", "  Next."]
        );
        assert!(layout.lines[1].spans[0].look.strong);
        let spans = &layout.lines[3].spans;
        assert_eq!(
            spans[1],
            Span { text: "remember".into(), look: Look { emphasis: true, ..Look::default() } }
        );
        assert!(!spans[0].look.emphasis && !spans[2].look.emphasis);
        // Open spacing: a blank line between the paragraphs too.
        let open = lay_out(&docs, Shape { open: true, ..shape(40) });
        assert_eq!(open.lines.len(), 6);
        assert!(open.lines[4].is_blank());
    }

    #[test]
    fn quotes_list_items_scene_breaks_and_pictures_have_lines_of_their_own() {
        let docs = doc(vec![
            block(TextBlockKind::Quote, 0, vec![run("A quoted line that wraps here.", 0)]),
            block(TextBlockKind::ListItem, 1, vec![run("An item", 0)]),
            block(TextBlockKind::SceneBreak, 2, Vec::new()),
            block(TextBlockKind::Picture { alt: Some("A lantern".into()) }, 3, Vec::new()),
            block(TextBlockKind::Preformatted, 4, vec![run("a  b\n\tc", 0)]),
        ]);
        kaeri_i18n::set_language("en");
        let layout = lay_out(&docs, shape(24));
        assert_eq!(
            texts(&layout),
            [
                "    A quoted line",
                "    that wraps here.",
                "  • An item",
                "",
                "       *   *   *",
                "",
                "   Picture: A lantern",
                "a  b",
                "    c",
            ]
        );
        assert!(layout.lines[6].spans[0].look.dim);
        assert_eq!(layout.lines[8].at, 5, "preformatted lines keep their offsets");
    }

    #[test]
    fn justify_spreads_the_spaces_of_wrapped_lines_only() {
        let docs = doc(vec![para(0, "aa bb cc dd ee ff gg hh")]);
        let layout = lay_out(&docs, Shape { justify: true, ..shape(12) });
        assert_eq!(texts(&layout), ["  aa  bb  cc", "dd  ee ff gg", "hh"]);
    }

    #[test]
    fn a_line_maps_to_its_place_and_back() {
        let docs = doc(vec![
            block(TextBlockKind::Heading { level: 1 }, 0, vec![run("Title", 0)]),
            para(1, "alpha beta gamma delta epsilon zeta eta theta"),
        ]);
        let layout = lay_out(&docs, shape(16));
        for (i, line) in layout.lines.iter().enumerate().filter(|(_, l)| !l.is_blank()) {
            let pos = layout.pos_of(&docs, i).unwrap();
            assert_eq!(pos.path, vec![line.block as u32]);
            assert_eq!(layout.line_of(&docs, &pos), Some(i), "line {i} round trips");
        }
        // A place in the middle of a line falls on that line.
        let third = layout.lines.iter().position(|l| l.text().starts_with("epsilon")).unwrap();
        let pos = NovelPos { spine: 2, path: vec![1], offset: layout.lines[third].at + 3 };
        assert_eq!(layout.line_of(&docs, &pos), Some(third));
        // The same place at another width finds the line that holds it now.
        let wide = lay_out(&docs, shape(40));
        let at = wide.line_of(&docs, &pos).unwrap();
        assert!(wide.lines[at].text().contains("epsilon"));
        // A place in no document of the text has no line.
        assert_eq!(
            layout.line_of(&docs, &NovelPos { spine: 9, path: vec![1], offset: 0 }),
            None
        );
    }

    #[test]
    fn offsets_count_utf16_units_past_wide_characters_and_emoji() {
        let docs = doc(vec![para(0, "日本語の文章です。😀 next word")]);
        let layout = lay_out(&docs, shape(12));
        // Chinese, Japanese and Korean text breaks between characters, never before 。
        assert_eq!(texts(&layout)[0], "  日本語の文");
        assert_eq!(texts(&layout)[1], "章です。😀");
        assert_eq!(layout.lines[1].at, 5);
        let next = layout.lines.iter().position(|l| l.text().starts_with("next")).unwrap();
        // 9 characters of one unit, the emoji's two, then a space.
        assert_eq!(layout.lines[next].at, 12);
        assert!(layout.lines.iter().all(|l| l.text().width() + usize::from(l.x) <= 12));
    }

    #[test]
    fn combining_marks_stay_on_their_letter() {
        let text = "Cafe\u{301} u\u{308}ber re\u{301}sume\u{301}";
        let docs = doc(vec![para(0, text)]);
        let layout = lay_out(&docs, shape(13));
        assert_eq!(texts(&layout), ["  Cafe\u{301} u\u{308}ber", "re\u{301}sume\u{301}"]);
        // "Café" is five units and "über" five, each with a space after: the third word
        // starts at 12.
        assert_eq!(layout.lines[1].at, 12);
    }

    #[test]
    fn a_word_longer_than_the_line_is_cut_where_it_has_to_be() {
        let docs = doc(vec![para(0, "go abcdefghijklmnopqrstuvwxyz end")]);
        let layout = lay_out(&docs, shape(10));
        assert_eq!(texts(&layout), ["  go", "abcdefghij", "klmnopqrst", "uvwxyz end"]);
        assert_eq!(layout.lines[2].at, 13);
        assert!(layout.lines.iter().all(|l| l.text().width() + usize::from(l.x) <= 10));
    }

    #[test]
    fn ruby_shows_in_brackets_after_its_text_and_still_counts() {
        let mut reading = run("(", 2);
        reading.ruby = true;
        let mut rt = run("かんじ", 3);
        rt.ruby = true;
        let mut close = run(")", 6);
        close.ruby = true;
        let docs = doc(vec![block(
            TextBlockKind::Paragraph,
            0,
            vec![run("漢字", 0), reading, rt, close, run("を読む", 7)],
        )]);
        let layout = lay_out(&docs, shape(40));
        assert_eq!(texts(&layout), ["  漢字(かんじ)を読む"]);
        assert!(layout.lines[0].spans.iter().any(|s| s.look.dim && s.text == "(かんじ)"));
    }

    #[test]
    fn documents_keep_their_own_lines() {
        let docs = vec![
            NovelTextDoc { spine: 1, blocks: vec![para(0, "First part.")] },
            NovelTextDoc { spine: 2, blocks: Vec::new() },
            NovelTextDoc { spine: 3, blocks: vec![para(0, "Second part.")] },
        ];
        let layout = lay_out(&docs, shape(40));
        assert_eq!(layout.docs, vec![0..1, 1..1, 1..2]);
        assert_eq!(layout.doc_of(1), 2);
        let pos = layout.pos_of(&docs, 1).unwrap();
        assert_eq!((pos.spine, pos.offset), (3, 0));
    }
}
