//! Writes what the engine would serve a novel's web view into a folder, for running the
//! paginator in a browser: `cargo run -p kaeri-books --example novel_serve -- <out> [book.epub…]`.
//!
//! `<out>/paginator.js` is the injected script and `<out>/csp.txt` the policy. `<out>/made/`
//! holds documents made here (a long chapter, vertical text, right to left, hostile markup, a
//! broken file); each EPUB goes to `<out>/<file stem>/`, its spine documents cleaned and served
//! with their setup, its stylesheets cleaned and its other files as they are. `spine.txt` in each
//! folder lists the documents in order. Prints how long each document took to clean.

use std::path::Path;
use std::time::Instant;

use kaeri_books::{Book, Limits, novel};

const STYLE: &str = r##""look":{"name":"light","bg":"#F7F5F0","fg":"#1E2033","link":"#1E2033","ink":false},"size":18,"line":1.5,"margin":22,"top":36,"bottom":44,"justify":true,"hyphens":true,"mode":"paged","columns":"auto","column":720,"eink":false,"legacy":false,"fixed":false"##;

fn setup(spine: usize, last: bool) -> String {
    format!("{{\"spine\":{spine},\"last\":{last},{STYLE}}}")
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
}

fn made(out: &Path) {
    let limits = Limits::default();
    let para = |n: usize| {
        format!(
            "<p id=\"p{n}\">Paragraph {n}. The lantern keeper climbed the stair again, counting the steps \
             as the tide came in under the pier and the gulls settled on the harbour wall. \
             <ruby>灯<rt>とう</rt></ruby> marks the light.</p>"
        )
    };
    let long: String = (0..120).map(para).collect();
    let long = format!(
        "<h1 id=\"top\">One</h1>{long}<p><a href=\"#p3\">Back to three</a> \
                        <a epub:type=\"noteref\" href=\"#n1\">1</a></p><aside id=\"n1\" epub:type=\"footnote\">A note.</aside>"
    );
    let tategaki: String = (0..60)
        .map(|n| format!("<p>第{n}段。灯台守は階段を上り、潮が満ちるのを数えた。</p>"))
        .collect();
    let rtl: String =
        (0..40).map(|n| format!("<p>שורה {n}. שומר המגדלור עלה במדרגות.</p>")).collect();
    let docs: [(&str, Option<&str>, String); 5] = [
        ("long.xhtml", Some("en"), wrap("en", "", &long)),
        (
            "vertical.xhtml",
            Some("ja"),
            wrap("ja", "<style>html { writing-mode: vertical-rl; }</style>", &tategaki),
        ),
        ("rtl.xhtml", Some("he"), wrap("he\" dir=\"rtl", "", &rtl)),
        (
            "hostile.xhtml",
            Some("en"),
            wrap(
                "en",
                "<meta http-equiv=\"refresh\" content=\"0;url=https://elsewhere.example/\"/>\
                 <script>window.__pwned=1</script><base href=\"https://elsewhere.example/\"/>\
                 <link rel=\"stylesheet\" href=\"https://elsewhere.example/a.css\"/>\
                 <style>p { background: url(https://tracker.example/bg.png) }</style>",
                "<p onclick=\"window.__pwned=2\" id=\"x\">Harbour lights.</p>\
                 <img src=\"https://tracker.example/pixel.png\" onerror=\"window.__pwned=3\"/>\
                 <a id=\"js\" href=\"javascript:window.__pwned=4\">Go</a>\
                 <a id=\"ext\" href=\"https://elsewhere.example/\">Out</a>\
                 <iframe src=\"https://elsewhere.example/\"></iframe>\
                 <svg xmlns=\"http://www.w3.org/2000/svg\"><script>window.__pwned=5</script></svg>",
            ),
        ),
        (
            "broken.xhtml",
            Some("en"),
            "<html><body><p>Unclosed <b>bold<p>and &nbsp; more".into(),
        ),
    ];
    let mut spine = String::new();
    for (i, (name, lang, text)) in docs.iter().enumerate() {
        let clean = novel::Clean {
            path: name,
            lang: *lang,
            soft_hyphens: novel::HYPHENATES,
            limits: &limits,
        };
        let start = Instant::now();
        let doc = novel::document(text.as_bytes(), &clean);
        let served = doc.served(&setup(i, true));
        println!("{:>7.2} ms  made/{name}", start.elapsed().as_secs_f64() * 1000.0);
        write(&out.join("made").join(name), served.as_bytes());
        spine.push_str(name);
        spine.push('\n');
    }
    write(&out.join("made/spine.txt"), spine.as_bytes());
}

fn wrap(lang: &str, head: &str, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\" \
         xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"{lang}\"><head><title>Made</title>{head}</head>\
         <body>{body}</body></html>"
    )
}

fn book(out: &Path, file: &Path) {
    let limits = Limits::default();
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("book");
    let dir = out.join(stem);
    let Ok(Book::Epub(mut epub)) = kaeri_books::open(file, &limits) else {
        println!("not an EPUB: {}", file.display());
        return;
    };
    let lang = epub.metadata().languages.first().cloned();
    let spine: Vec<String> = epub.spine().map(|(_, item)| item.path.clone()).collect();
    let manifest: Vec<(String, String)> = epub
        .package()
        .manifest
        .iter()
        .map(|m| (m.path.clone(), m.media_type.clone()))
        .collect();
    for (path, media) in manifest {
        let Ok(resource) = epub.resource(&path) else { continue };
        let start = Instant::now();
        let bytes = match media.as_str() {
            "application/xhtml+xml" | "text/html" => {
                let clean = novel::Clean {
                    path: &path,
                    lang: lang.as_deref(),
                    soft_hyphens: novel::HYPHENATES,
                    limits: &limits,
                };
                let doc = novel::document(&resource.data, &clean);
                let index = spine.iter().position(|p| *p == path).unwrap_or(0);
                doc.served(&setup(index, index + 1 == spine.len())).into_bytes()
            }
            "text/css" => {
                novel::stylesheet(&String::from_utf8_lossy(&resource.data), &path, &limits)
                    .into_bytes()
            }
            m if m.contains("javascript") || m.contains("ecmascript") => continue,
            _ => resource.data,
        };
        if spine.contains(&path) {
            println!("{:>7.2} ms  {stem}/{path}", start.elapsed().as_secs_f64() * 1000.0);
        }
        write(&dir.join(&path), &bytes);
    }
    write(&dir.join("spine.txt"), (spine.join("\n") + "\n").as_bytes());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("an output folder");
    let out = Path::new(&out);
    write(&out.join("paginator.js"), novel::script().as_bytes());
    write(&out.join("csp.txt"), novel::csp().as_bytes());
    made(out);
    for file in args {
        book(out, Path::new(&file));
    }
}
