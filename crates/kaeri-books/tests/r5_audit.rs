//! Round 5 engine audit: generated inputs for the text a book or a source plugin hands the
//! engine. A source's novel chapter is HTML from the web, chapter numbers and names are free
//! text, and page names come from archives; none of it may panic, hang or break an ordering
//! that a sort relies on.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use kaeri_books::{
    Limits, comicinfo, naming, natural,
    novel::{self, Clean},
    path,
};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
    fn text(&mut self, parts: &[&str], most: u64) -> String {
        (0..self.below(most + 1)).map(|_| self.pick(parts)).collect()
    }
}

/// Pieces of web chapters: markup, references, wide and case-changing letters, controls.
const HTML: &[&str] = &[
    "<",
    ">",
    "</",
    "/>",
    "<img",
    "<IMG",
    " src=",
    "src",
    "=",
    "\"",
    "'",
    " ",
    "\n",
    "&",
    "&amp;",
    "&#x",
    "&#",
    ";",
    "1F600",
    "110000",
    "https://a.example/p.png",
    "https://",
    "<p>",
    "</p>",
    "<h1>",
    "</h1>",
    "<h2>",
    "<b>",
    "</b>",
    "<body>",
    "</body>",
    "<html>",
    "<head>",
    "</head>",
    "<div>",
    "</div>",
    "Chapter 12",
    "Ch. 3",
    "第3話",
    "é",
    "İ",
    "ß",
    "\u{1F600}",
    "\u{0}",
    "\u{FEFF}",
    "\u{200B}",
    "<!--",
    "-->",
    "<![CDATA[",
    "]]>",
    "<?xml",
    "?>",
    "<!DOCTYPE html>",
    "<script>",
    "<style>",
    "writing-mode",
    ":",
    "vertical-rl",
    "<svg>",
    "<a href=\"",
    "#x",
    "<br>",
    "<hr/>",
    "\t",
    "\r",
];

const NAMES: &[&str] = &[
    "0",
    "1",
    "9",
    "00",
    "007",
    "10",
    "2",
    ".",
    "5",
    "a",
    "A",
    "b",
    "_",
    "-",
    " ",
    "v",
    "V",
    "c",
    "ch",
    "Ch.",
    "Vol.",
    "volume",
    "chapter",
    "#",
    "é",
    "É",
    "İ",
    "ı",
    "ß",
    "第",
    "話",
    "巻",
    "[",
    "]",
    "(",
    ")",
    "{",
    "}",
    "x",
    "99999999999999999999",
    "\u{3000}",
    "\u{FF11}",
    "½",
    "²",
    "٣",
];

fn timed<T>(what: &str, input: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = catch_unwind(AssertUnwindSafe(f))
        .unwrap_or_else(|_| panic!("{what} panicked on {input:?}"));
    let took = start.elapsed();
    assert!(took < Duration::from_secs(2), "{what} took {took:?} on {} bytes", input.len());
    out
}

/// A source's text chapter goes through every novel step the engine runs on it.
#[test]
fn web_chapters_never_panic_or_hang() {
    let limits = Limits::default();
    let clean =
        Clean { path: "OEBPS/c1.xhtml", lang: Some("en"), soft_hyphens: true, limits: &limits };
    let mut rng = Rng(0x5eed_0101);
    for _ in 0..20_000 {
        let html = rng.text(HTML, 60);
        let found = timed("web_images", &html, || novel::web_images(&html));
        let local: HashMap<String, String> =
            found.iter().map(|u| (u.clone(), "images/w1.png".to_owned())).collect();
        timed("with_local_images", &html, || novel::with_local_images(&html, &local));
        timed("heading_once", &html, || {
            novel::heading_once(&html, &|t: &str| Some(t.to_uppercase()))
        });
        let doc = timed("text_chapter", &html, || novel::text_chapter(&html, "T", &clean));
        timed("served", &html, || doc.served("{}"));
        timed("document", &html, || novel::document(html.as_bytes(), &clean));
        timed("svg", &html, || novel::svg(html.as_bytes(), &clean));
        timed("stylesheet", &html, || novel::stylesheet(&html, "OEBPS/s.css", &limits));
    }
}

/// Large, flat inputs at the size limit stay quick: a chapter of one long unclosed tag, many
/// `<img` with no end, or many `&` without `;`.
#[test]
fn long_degenerate_chapters_stay_linear() {
    let limits = Limits::default();
    let clean = Clean { path: "c.xhtml", lang: None, soft_hyphens: true, limits: &limits };
    // A run of bare `<` is R5E-6's.
    for unit in ["<img ", "<img src='", "&", "<p><b>", "<h1>x</h1>", "a=b ", "\u{1F600}"] {
        let html = unit.repeat(400_000 / unit.len());
        timed("web_images", unit, || novel::web_images(&html));
        timed("with_local_images", unit, || novel::with_local_images(&html, &HashMap::new()));
        timed("heading_once", unit, || novel::heading_once(&html, &|_: &str| None));
        timed("text_chapter", unit, || novel::text_chapter(&html, "T", &clean));
    }
}

/// R5E-6. Finding a chapter's pictures looks for the end of each tag name after every `<`. A
/// chapter of `<` with no space, `/` or `>` after them must still be read in one pass.
#[test]
fn a_chapter_of_bare_angle_brackets_stays_linear() {
    let html = "<".repeat(400_000);
    timed("web_images", "<", || novel::web_images(&html));
    timed("with_local_images", "<", || novel::with_local_images(&html, &HashMap::new()));
}

/// Names and numbers from archives and plugins: no panic anywhere in naming or paths.
#[test]
fn names_and_paths_never_panic() {
    let mut rng = Rng(0x5eed_0102);
    for _ in 0..50_000 {
        let s = rng.text(NAMES, 14);
        timed("naming::parse", &s, || naming::parse(&s));
        timed("naming::stem", &s, || naming::stem(&s));
        timed("naming::tidy", &s, || naming::tidy(&s));
        timed("naming::milli", &s, || naming::milli(&s));
        timed("naming::volume_folder", &s, || naming::volume_folder(&s));
        timed("path::entry_name", &s, || path::entry_name(&s, 64, 4));
        timed("path::resolve", &s, || path::resolve("a/b/c.xhtml", &s, 4));
        timed("path::percent_decode", &s, || path::percent_decode(&s));
        timed("path::extension", &s, || path::extension(&s));
    }
}

/// Page order in an archive is a sort with `natural::compare`; since Rust 1.81 a comparison
/// that is not a total order may make `sort_by` panic, so it must be one.
#[test]
fn natural_order_is_a_total_order() {
    let mut rng = Rng(0x5eed_0103);
    let alphabet =
        ["0", "00", "1", "01", "2", "10", "a", "A", "b", "B", "_", ".", "-", "é", "É", " "];
    for _ in 0..200 {
        let names: Vec<String> = (0..40).map(|_| rng.text(&alphabet, 5)).collect();
        for a in &names {
            assert_eq!(natural::compare(a, a), std::cmp::Ordering::Equal);
            for b in &names {
                let ab = natural::compare(a, b);
                assert_eq!(ab, natural::compare(b, a).reverse(), "{a:?} {b:?}");
                if ab == std::cmp::Ordering::Equal {
                    assert_eq!(a, b, "two names equal but different");
                }
                for c in &names {
                    if ab.is_le() && natural::compare(b, c).is_le() {
                        assert!(natural::compare(a, c).is_le(), "{a:?} <= {b:?} <= {c:?}");
                    }
                }
            }
        }
        let mut sorted = names.clone();
        sorted.sort_by(|a, b| natural::compare(a, b));
    }
}

/// ComicInfo from a book is untrusted XML.
#[test]
fn comic_info_never_panics() {
    let parts: &[&str] = &[
        "<ComicInfo>",
        "</ComicInfo>",
        "<Pages>",
        "</Pages>",
        "<Page ",
        "Image=\"",
        "\"",
        "/>",
        "Type=\"FrontCover\" ",
        "DoublePage=\"true\" ",
        "<Number>",
        "</Number>",
        "<PageCount>",
        "</PageCount>",
        "4294967296",
        "-1",
        "1e9",
        "<Manga>",
        "YesAndRightToLeft",
        "</Manga>",
        "<Volume>",
        "</Volume>",
        "NaN",
        "&amp;",
        "&",
        "<",
        "é",
        "<?xml version=\"1.0\"?>",
    ];
    let limits = Limits::default();
    let mut rng = Rng(0x5eed_0104);
    for _ in 0..20_000 {
        let xml = rng.text(parts, 30);
        timed("comicinfo::parse", &xml, || {
            comicinfo::parse(xml.as_bytes(), kaeri_books::Format::Cbz, &limits).ok().map(
                |info| (info.right_to_left(), info.page(usize::MAX).is_some(), info.to_xml()),
            )
        });
    }
}

/// R5E-4. An attribute whose name starts with a letter of three or more bytes (`<p あa="1">`,
/// well-formed XML) goes through the `on…` handler check in the reader's sanitiser and in the
/// EPUB writer downloads and Send to e-reader use. Neither may panic.
#[test]
fn attributes_named_in_wide_letters_are_cleaned() {
    let limits = Limits::default();
    let clean = Clean { path: "c.xhtml", lang: None, soft_hyphens: false, limits: &limits };
    let body = "<p あa=\"1\">Text of the chapter.</p>";
    let mut panicked = Vec::new();
    if catch_unwind(AssertUnwindSafe(|| novel::text_chapter(body, "One", &clean))).is_err() {
        panicked.push("novel::text_chapter");
    }
    let page = format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>t</title></head><body>{body}</body></html>"
    );
    if catch_unwind(AssertUnwindSafe(|| novel::document(page.as_bytes(), &clean))).is_err() {
        panicked.push("novel::document");
    }
    let wrote = catch_unwind(AssertUnwindSafe(|| {
        let meta = kaeri_books::write::BookMeta { title: "One".into(), ..Default::default() };
        let out = std::io::Cursor::new(Vec::new());
        let mut book = kaeri_books::write::EpubWriter::new(
            out,
            meta,
            kaeri_books::write::EpubFlavor::Epub,
            None,
        )
        .unwrap();
        let _ = book.add_chapter("One", body);
    }));
    if wrote.is_err() {
        panicked.push("write::EpubWriter::add_chapter");
    }
    assert!(panicked.is_empty(), "panicked: {panicked:?}");
}
