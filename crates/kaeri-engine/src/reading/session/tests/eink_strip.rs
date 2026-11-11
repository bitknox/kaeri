//! A long strip on e-ink is cut into screens. The places the engine sends name images, so each
//! cut says where it starts, and a front end finds the cut for a place with that.

use super::*;

/// A comic archive of tall strip images, each a flat grey of its own; `broken` holds bytes no
/// decoder reads.
fn strip_cbz(path: &Path, images: u8, broken: Option<u8>) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    for i in 0..images {
        let mut png = Vec::new();
        if broken == Some(i) {
            png.extend_from_slice(b"\x89PNG\r\n\x1a\nnot a picture");
        } else {
            let image = image::GrayImage::from_pixel(300, 1800, image::Luma([40 + i * 30]));
            image
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .unwrap();
        }
        zip.start_file(format!("{i:03}.png"), stored).unwrap();
        zip.write_all(&png).unwrap();
    }
    zip.finish().unwrap();
}

/// One chapter of four tall images, read as a long strip.
fn tall_strip(broken: Option<u8>) -> Harness {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manhwa", |books| {
        let path = books.join("Chapter 1.cbz");
        strip_cbz(&path, 4, broken);
        vec![ShelfFile {
            path: path.display().to_string(),
            chapter: None,
            label: "Chapter 1".into(),
            volume: None,
            number_milli: Some(1000),
            format: "cbz".into(),
            pages: Some(4),
            comicinfo: None,
            source: None,
        }]
    });
    h.failures = broken.is_some();
    h.services.store.set_series_setting(WORK, "mode", Some("strip"), true, 2).unwrap();
    h
}

impl Harness {
    /// Open on an e-ink screen and wait for the view and its leaves.
    async fn open_eink(&mut self, at: OpenAt) -> (ReaderView, Vec<Leaf>) {
        let surface = ReaderSurface { eink: true, ..surface() };
        self.send(ReaderCommand::Open { work: WorkId::new(WORK), chapter: None, at, surface });
        let view = self
            .until(|e| match e {
                ReaderEvent::Opened(view) => Some(view.as_ref().clone()),
                _ => None,
            })
            .await;
        let leaves = self
            .until(|e| match e {
                ReaderEvent::Leaves { leaves, .. } => Some(leaves.clone()),
                _ => None,
            })
            .await;
        (view, leaves)
    }
}

/// Where each cut starts, as its leaf says.
fn starts(leaves: &[Leaf]) -> Vec<StripPos> {
    leaves
        .iter()
        .filter(|l| !l.end_page)
        .map(|l| l.parts[0].strip.expect("a cut's start"))
        .collect()
}

/// The cut a place shows, as a front end finds it: the last one starting at or above the place,
/// or for a page alone the first cut of that image.
fn cut_of(leaves: &[Leaf], place: &ReadPlace) -> usize {
    let starts = starts(leaves);
    match (place.strip, place.page) {
        (Some(at), _) => starts
            .iter()
            .rposition(|c| c.image < at.image || (c.image == at.image && c.offset <= at.offset))
            .unwrap_or(0),
        (None, Some(page)) => starts.iter().position(|c| c.image >= page).unwrap_or(0),
        (None, None) => 0,
    }
}

#[tokio::test]
async fn each_cut_says_where_in_the_strip_it_starts() {
    let mut h = tall_strip(None);
    let (view, leaves) = h.open_eink(OpenAt::Start).await;
    assert_eq!(view.mode, ReaderMode::Strip);
    let starts = starts(&leaves);
    assert!(starts.len() > 4, "four tall images make more screens than images: {starts:?}");
    assert_eq!(starts[0], StripPos { image: 0, offset: 0.0 });
    assert!(
        starts.windows(2).all(|w| (w[0].image, w[0].offset) < (w[1].image, w[1].offset)),
        "{starts:?}"
    );
    let images: std::collections::BTreeSet<u32> = starts.iter().map(|c| c.image).collect();
    assert_eq!(images.into_iter().collect::<Vec<_>>(), [0, 1, 2, 3]);
    // A cut's page is its own number, which is not the image it starts in.
    assert!(
        leaves
            .iter()
            .filter(|l| !l.end_page)
            .enumerate()
            .all(|(i, l)| l.parts[0].page == i as u32)
    );
}

#[tokio::test]
async fn an_image_that_cannot_be_read_keeps_the_numbers_of_those_after_it() {
    let mut h = tall_strip(Some(1));
    let (_, leaves) = h.open_eink(OpenAt::Start).await;
    let images: std::collections::BTreeSet<u32> =
        starts(&leaves).iter().map(|c| c.image).collect();
    assert_eq!(images.into_iter().collect::<Vec<_>>(), [0, 2, 3]);
}

#[tokio::test]
async fn resuming_three_times_keeps_the_place() {
    let mut h = tall_strip(None);
    let (_, leaves) = h.open_eink(OpenAt::Start).await;
    // A cut part way down the third image.
    let leaf = starts(&leaves).iter().position(|c| c.image == 2 && c.offset > 0.0).unwrap();
    let place = starts(&leaves)[leaf];
    h.shown(leaf as u32, TurnHow::Turn).await;
    h.send(ReaderCommand::Close { session: h.session() });
    for round in 1..=3 {
        let (view, leaves) = h.open_eink(OpenAt::Resume).await;
        assert_eq!(view.start.page, Some(2), "round {round}");
        assert_eq!(view.start.strip, Some(place), "round {round}");
        let found = cut_of(&leaves, &view.start);
        assert_eq!(found, leaf, "round {round}");
        assert_ne!(found as u32, 2, "the image's number is not the cut's");
        h.shown(found as u32, TurnHow::Placed).await;
        h.send(ReaderCommand::Close { session: h.session() });
        let kept = h.kept("1").unwrap();
        assert_eq!(kept.last_page, Some(2), "round {round}");
        assert_eq!(kept.strip_image, Some(2), "round {round}");
        assert!(!kept.read, "round {round}");
    }
}
