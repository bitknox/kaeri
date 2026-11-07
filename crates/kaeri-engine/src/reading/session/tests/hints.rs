//! How a series' source says its pages read, through the session: the person's own choice for
//! the series comes first, then the source's hint, then the reader's guess from the book, then
//! the medium.

use std::io::Cursor;

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, ReadingHint,
        SeriesDetail, SeriesHit, SourceChapter,
    },
    error::ProviderError,
};
use kaeri_model::reader::{ReadDirection, ReaderValue};

use super::*;

/// A source of one chapter of pictures that says how its series reads.
struct HintedSource {
    manifest: BookSourceManifest,
    hint: Option<ReadingHint>,
}

impl HintedSource {
    fn new(id: &str, kind: BookSourceKind, hint: Option<ReadingHint>) -> Arc<Self> {
        Arc::new(Self {
            manifest: BookSourceManifest {
                id: id.into(),
                name: "Hinted".into(),
                version: "1".into(),
                kind,
                media: vec![Medium::Manga],
                languages: vec!["en".into()],
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            },
            hint,
        })
    }
}

#[async_trait]
impl kaeri_core::traits::BookSource for HintedSource {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail {
            reading_hint: self.hint,
            ..SeriesDetail::new(id, "The Tin Lighthouse")
        })
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok(vec![SourceChapter::new("c1", "1")])
    }

    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        Ok(ChapterContent::Images(
            (1..=3)
                .map(|n| {
                    kaeri_core::books::PageRef::new(format!(
                        "https://pages.example/{chapter}/{n}"
                    ))
                })
                .collect(),
        ))
    }

    async fn fetch_page(
        &self,
        page: &kaeri_core::books::PageRef,
    ) -> Result<Vec<u8>, kaeri_core::traits::PageError> {
        let shade = page.url.bytes().last().unwrap_or(b'1');
        let picture = image::GrayImage::from_pixel(300, 450, image::Luma([shade]));
        let mut png = Vec::new();
        picture.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        Ok(png)
    }
}

const HINTED: &str = "a:900777";

/// A reader over a work only `source` holds, with nothing on the shelf.
fn hinted(source: Arc<HintedSource>) -> Harness {
    let h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
    let id = source.manifest.id.clone();
    h.services.book_sources.registry.put(source);
    h.services.store.set_book_match(&id, "series-1", HINTED, 90, true, false, 1).unwrap();
    h
}

impl Harness {
    /// The settings sheet's row for `key`, for the hinted work.
    fn row(&mut self, key: Key) -> kaeri_model::reader::ReaderSettingRow {
        let cx = self.reader.cx(&mut self.config, &self.services, &self.tx);
        let view = cx.settings_view(Medium::Manga, Some(&WorkId::new(HINTED)));
        view.rows.into_iter().find(|r| r.key == key).unwrap()
    }
}

/// A comic archive of three portrait pages whose ComicInfo says it is no manga, so the reader
/// guesses left to right.
fn western_cbz(books: &Path) -> Vec<ShelfFile> {
    let path = books.join("Chapter 1.cbz");
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    zip.start_file("ComicInfo.xml", stored).unwrap();
    zip.write_all(b"<?xml version=\"1.0\"?><ComicInfo><Manga>No</Manga></ComicInfo>").unwrap();
    for i in 0..3u8 {
        let page = image::GrayImage::from_pixel(300, 450, image::Luma([40 + i * 20]));
        let mut png = Vec::new();
        page.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        zip.start_file(format!("{i:03}.png"), stored).unwrap();
        zip.write_all(&png).unwrap();
    }
    zip.finish().unwrap();
    vec![ShelfFile {
        path: path.display().to_string(),
        chapter: None,
        label: "Chapter 1".into(),
        volume: None,
        number_milli: Some(1000),
        format: "cbz".into(),
        pages: Some(3),
        comicinfo: None,
        source: None,
    }]
}

/// A server series marked left to right opens left to right, though a manga's default is right
/// to left, and the sheet says the direction came from the series' details. A series marked
/// webtoon opens as the Webtoons default.
#[tokio::test(start_paused = true)]
async fn a_server_series_opens_the_way_its_server_says() {
    let mut h = hinted(HintedSource::new(
        "komga-1",
        BookSourceKind::Server,
        Some(ReadingHint::LeftToRight),
    ));
    let (view, _, said) = h.open_work(WorkId::new(HINTED), OpenAt::Start).await;
    assert_eq!(view.direction, ReadDirection::LeftToRight);
    assert_eq!(view.mode, ReaderMode::Paged);
    assert!(notices(&said).is_empty(), "the server said it, nothing was guessed: {said:?}");
    let row = h.row(Key::Direction);
    assert_eq!(row.value, ReaderValue::Choice { name: "ltr".into() });
    assert!(row.detected && !row.overridden, "{row:?}");
    let kept = h.services.store.book_series_hint("komga-1", "series-1").unwrap();
    assert_eq!(kept.and_then(|(hint, _)| hint).as_deref(), Some("ltr"));

    let mut h =
        hinted(HintedSource::new("komga-1", BookSourceKind::Server, Some(ReadingHint::Strip)));
    let (view, _, _) = h.open_work(WorkId::new(HINTED), OpenAt::Start).await;
    assert_eq!(view.mode, ReaderMode::Strip, "Long strip is the Webtoons default");
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    h.config.effective_mut().reading.webtoon.mode = kaeri_core::config::WebtoonMode::Paged;
    let (view, _, _) = h.open_work(WorkId::new(HINTED), OpenAt::Start).await;
    assert_eq!(view.mode, ReaderMode::Paged, "the Webtoons default is read at each open");
}

/// A plugin's hint, a string in the ABI, opens as it says: a long strip as the Webtoons
/// default, pages one under another as the vertical mode.
#[tokio::test(start_paused = true)]
async fn a_plugin_series_opens_the_way_its_plugin_says() {
    let strip = ReadingHint::parse("long strip");
    let mut h = hinted(HintedSource::new("plugin-books", BookSourceKind::Plugin, strip));
    let (view, _, said) = h.open_work(WorkId::new(HINTED), OpenAt::Start).await;
    assert_eq!(view.mode, ReaderMode::Strip);
    assert!(notices(&said).is_empty(), "{said:?}");
    let row = h.row(Key::Mode);
    assert_eq!(row.value, ReaderValue::Choice { name: "strip".into() });
    assert!(row.detected, "{row:?}");

    let vertical = ReadingHint::parse("vertical");
    let mut h = hinted(HintedSource::new("plugin-books", BookSourceKind::Plugin, vertical));
    let (view, _, _) = h.open_work(WorkId::new(HINTED), OpenAt::Start).await;
    assert_eq!(view.mode, ReaderMode::Vertical);
}

/// The person's own direction for the series wins over the source's hint; the reader's kept
/// guess does not.
#[tokio::test(start_paused = true)]
async fn the_persons_own_direction_wins_over_the_hint_and_the_hint_over_a_guess() {
    let mut h = hinted(HintedSource::new(
        "komga-1",
        BookSourceKind::Server,
        Some(ReadingHint::LeftToRight),
    ));
    h.services.store.set_series_guess(HINTED, "direction", "rtl", 1).unwrap();
    let (view, _, _) = h.open_work(WorkId::new(HINTED), OpenAt::Start).await;
    assert_eq!(view.direction, ReadDirection::LeftToRight, "the hint beats a kept guess");
    let session = h.session();
    h.send(ReaderCommand::Close { session });

    h.services.store.set_series_setting(HINTED, "direction", Some("rtl"), false, 2).unwrap();
    let (view, _, _) = h.open_work(WorkId::new(HINTED), OpenAt::Start).await;
    assert_eq!(
        view.direction,
        ReadDirection::RightToLeft,
        "the person's choice beats the hint"
    );
    let row = h.row(Key::Direction);
    assert!(row.overridden && !row.detected, "{row:?}");
}

/// With no word from the source the reader guesses from the book, and says so once; a hint
/// kept for the series later wins over that guess.
#[tokio::test(start_paused = true)]
async fn with_no_hint_the_book_decides_and_a_hint_wins_over_it() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", western_cbz);
    let (view, _, said) = h.open(OpenAt::Start).await;
    assert_eq!(view.direction, ReadDirection::LeftToRight, "ComicInfo says it is no manga");
    assert_eq!(notices(&said), [ReadingMsg::ReaderDetectedLeftToRight]);
    let session = h.session();
    h.send(ReaderCommand::Close { session });
    h.drain();

    // A server holds the series too, and said it reads right to left when last asked.
    h.services.book_sources.registry.put(HintedSource::new(
        "komga-1",
        BookSourceKind::Server,
        None,
    ));
    h.services.store.set_book_match("komga-1", "series-1", WORK, 90, true, false, 1).unwrap();
    h.services.store.put_book_series_hint("komga-1", "series-1", Some("rtl"), 2).unwrap();
    let (view, _, said) = h.open(OpenAt::Start).await;
    assert_eq!(view.direction, ReadDirection::RightToLeft, "the source's hint beats the guess");
    assert!(notices(&said).is_empty(), "{said:?}");
}
