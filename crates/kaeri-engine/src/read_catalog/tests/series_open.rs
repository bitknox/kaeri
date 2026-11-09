//! Opening a series page as the apps do, against a source whose first answer is slow: the order
//! of the events the page hears, so it never shows a failure the engine has not settled on.

use std::time::Duration;

use kaeri_core::books::WorkId;
use kaeri_model::book_sources::BookSourceEvent;

use super::*;
use crate::read_catalog::{editions, series};

const WORK_ID: u32 = 913_001;
const TITLE: &str = "The Lantern Ferry";

/// A source that holds the series under its own name and answers its first search only when the
/// test lets it, as a book plugin does while it loads.
struct SlowShelf {
    manifest: BookSourceManifest,
    gate: Arc<Notify>,
    searched: AtomicU32,
}

impl SlowShelf {
    fn put(kit: &Kit) -> Arc<Self> {
        let manifest = BookSourceManifest {
            id: "slow-books".into(),
            name: "Slow Books".into(),
            version: "1".into(),
            kind: BookSourceKind::Native,
            media: vec![Medium::Manga, Medium::Novel],
            languages: vec!["en".into()],
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        let shelf = Arc::new(Self {
            manifest,
            gate: Arc::new(Notify::new()),
            searched: AtomicU32::new(0),
        });
        kit.sources.registry.put(shelf.clone());
        shelf
    }
}

#[async_trait]
impl BookSource for SlowShelf {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        if self.searched.fetch_add(1, Ordering::SeqCst) == 0 {
            self.gate.notified().await;
        }
        Ok(vec![SeriesHit::new("ferry", TITLE)])
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, TITLE))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok((1..=3).map(|n| SourceChapter::new(format!("c-{n}"), n.to_string())).collect())
    }

    async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
        Err(ProviderError::NotFound)
    }
}

/// What the page hears, in a word each.
fn heard(event: &Event) -> Option<String> {
    match event {
        Event::Reading(ReadingEvent::Series(view)) => {
            Some(format!("series({:?}, {:?})", view.titles.display, view.action))
        }
        Event::Reading(ReadingEvent::Chapters { volumes, failure, .. }) => {
            let rows: usize = volumes.iter().map(|v| v.chapters.len()).sum();
            Some(match failure {
                Some(failure) => format!("chapters failed({failure:?})"),
                None => format!("chapters({rows})"),
            })
        }
        Event::BookSources(BookSourceEvent::ForWork { asking, missing, editions, .. }) => {
            Some(format!(
                "read-from(asking: {asking}, editions: {}, missing: {})",
                editions.len(),
                missing.len()
            ))
        }
        _ => None,
    }
}

/// Everything sent until nothing more comes for `quiet`.
async fn drain(rx: &mut mpsc::UnboundedReceiver<Event>, quiet: Duration) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(Some(event)) = tokio::time::timeout(quiet, rx.recv()).await {
        out.extend(heard(&event));
    }
    out
}

/// The apps open a series with LoadSeries and LoadChapters, then Read from's LoadForWork once
/// the header is drawn. A series no source is matched to yet is searched for by name, and the
/// slow source's answer is what makes it readable. Until that check is over, the page must not
/// hear that the chapters failed.
#[tokio::test]
async fn a_series_still_being_looked_for_hears_no_failure_before_its_chapters() {
    let fake = FakeAniList::start().await;
    fake.add_books([book(WORK_ID, TITLE, "MANGA", (2020, 500), &["Drama"], &[])]);
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    // The tile tapped came from AniList, so its book is kept.
    kit.meta.book(kaeri_core::AnilistId(WORK_ID)).await.unwrap();
    let shelf = SlowShelf::put(&kit);
    let work = WorkId::new(format!("a:{WORK_ID}"));
    let (tx, mut rx) = outbox();

    series::load(&kit, ask(), work.clone(), &tx);
    crate::reading::chapters_of(kit.clone(), ask(), work.clone(), None, &tx);
    editions::load(&kit, ask(), work.clone(), &tx);
    let before = drain(&mut rx, Duration::from_millis(300)).await;
    shelf.gate.notify_one();
    let after = drain(&mut rx, Duration::from_millis(300)).await;
    let order =
        [before.clone(), vec!["-- the source answers --".into()], after.clone()].concat();
    eprintln!("{order:#?}");

    assert!(
        !before.iter().any(|e| e.starts_with("chapters")),
        "nothing about the chapters while the source is still asked: {order:#?}"
    );
    assert!(
        !order.iter().any(|e| e.starts_with("chapters failed")),
        "no failure on the way: {order:#?}"
    );
    let chapters: Vec<_> = after.iter().filter(|e| e.starts_with("chapters")).collect();
    assert_eq!(chapters.first().map(|e| e.as_str()), Some("chapters(3)"), "{order:#?}");
    assert!(
        before.first().is_some_and(|e| e.contains(TITLE)),
        "the header shows what is known at once: {order:#?}"
    );
}

/// A series no source has: the failure comes once every source has answered, not before.
#[tokio::test]
async fn a_series_no_source_has_fails_once_the_sources_have_answered() {
    let fake = FakeAniList::start().await;
    fake.add_books([book(WORK_ID, "Harbor of Small Hours", "MANGA", (2020, 500), &[], &[])]);
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    kit.meta.book(kaeri_core::AnilistId(WORK_ID)).await.unwrap();
    let shelf = SlowShelf::put(&kit);
    let work = WorkId::new(format!("a:{WORK_ID}"));
    let (tx, mut rx) = outbox();

    series::load(&kit, ask(), work.clone(), &tx);
    crate::reading::chapters_of(kit.clone(), ask(), work.clone(), None, &tx);
    editions::load(&kit, ask(), work.clone(), &tx);
    let before = drain(&mut rx, Duration::from_millis(300)).await;
    shelf.gate.notify_one();
    let after = drain(&mut rx, Duration::from_millis(300)).await;
    let order =
        [before.clone(), vec!["-- the source answers --".into()], after.clone()].concat();

    assert!(!before.iter().any(|e| e.starts_with("chapters")), "{order:#?}");
    let chapters: Vec<_> = after.iter().filter(|e| e.starts_with("chapters")).collect();
    assert_eq!(chapters, [&"chapters failed(NotFound)".to_owned()], "{order:#?}");
}

/// A source that answers at once: its chapter list of `chapters`, or its search with `broken`.
struct Shelf {
    manifest: BookSourceManifest,
    chapters: u32,
    broken: bool,
}

impl Shelf {
    fn put(kit: &Kit, id: &str, name: &str, chapters: u32, broken: bool) {
        let manifest = BookSourceManifest {
            id: id.into(),
            name: name.into(),
            version: "1".into(),
            kind: BookSourceKind::Native,
            media: vec![Medium::Manga],
            languages: vec!["en".into()],
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        kit.sources.registry.put(Arc::new(Self { manifest, chapters, broken }));
    }
}

#[async_trait]
impl BookSource for Shelf {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        if self.broken {
            return Err(ProviderError::Parse("the page changed".into()));
        }
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, TITLE))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok((1..=self.chapters)
            .map(|n| SourceChapter::new(format!("c-{n}"), n.to_string()))
            .collect())
    }

    async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
        Err(ProviderError::NotFound)
    }
}

/// Read from's lines as the last answer gave them: each edition's source and where its list
/// stands.
fn listings(events: &[Event]) -> Vec<(String, Option<kaeri_model::reading::EditionListing>)> {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::BookSources(BookSourceEvent::ForWork { editions, .. }) => Some(
                editions.iter().map(|e| (e.source_id.clone(), e.listing.clone())).collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

async fn all(rx: &mut mpsc::UnboundedReceiver<Event>, quiet: Duration) -> Vec<Event> {
    let mut out = Vec::new();
    while let Ok(Some(event)) = tokio::time::timeout(quiet, rx.recv()).await {
        out.push(event);
    }
    out
}

/// A source whose search fails is named in Read from's miss as the person knows it, not by its
/// id.
#[tokio::test]
async fn a_source_that_fails_is_named_by_its_name() {
    let fake = FakeAniList::start().await;
    fake.add_books([book(WORK_ID, TITLE, "MANGA", (2020, 500), &[], &[])]);
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    kit.meta.book(kaeri_core::AnilistId(WORK_ID)).await.unwrap();
    Shelf::put(&kit, "quiet-shelf", "Quiet Shelf", 0, true);
    let work = WorkId::new(format!("a:{WORK_ID}"));
    let (tx, mut rx) = outbox();

    editions::load(&kit, ask(), work, &tx);
    let events = all(&mut rx, Duration::from_millis(300)).await;
    let missing = events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::BookSources(BookSourceEvent::ForWork { missing, asking: false, .. }) => {
                Some(missing.clone())
            }
            _ => None,
        })
        .expect("Read from's answer");
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(missing[0].source_name, "Quiet Shelf");
    let words = format!("{:?}", missing[0].reason);
    assert!(words.contains("Quiet Shelf") && !words.contains("quiet-shelf"), "{words}");
}

/// Two sources hold the series; the one picked first lists no chapters. The page hears an
/// empty list, never a failure, the other edition is asked for its list, and Read from says
/// which is empty and which has chapters: never "Chapters 1 to 0".
#[tokio::test]
async fn an_empty_edition_says_so_and_the_others_are_listed() {
    let fake = FakeAniList::start().await;
    fake.add_books([book(WORK_ID, TITLE, "MANGA", (2020, 500), &[], &[])]);
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    kit.meta.book(kaeri_core::AnilistId(WORK_ID)).await.unwrap();
    Shelf::put(&kit, "bare-shelf", "Bare Shelf", 0, false);
    Shelf::put(&kit, "full-shelf", "Full Shelf", 3, false);
    let work = WorkId::new(format!("a:{WORK_ID}"));
    for source in ["bare-shelf", "full-shelf"] {
        kit.store
            .set_book_match(source, "ferry", work.as_str(), 90, false, false, NOW)
            .unwrap();
    }
    let (tx, mut rx) = outbox();

    crate::reading::chapters_of(kit.clone(), ask(), work.clone(), None, &tx);
    editions::load(&kit, ask(), work.clone(), &tx);
    let events = all(&mut rx, Duration::from_millis(300)).await;
    let heard: Vec<String> = events.iter().filter_map(heard).collect();
    eprintln!("{heard:#?}");

    assert!(!heard.iter().any(|e| e.starts_with("chapters failed")), "{heard:#?}");
    assert!(heard.iter().any(|e| e.starts_with("chapters(")), "{heard:#?}");
    let lines = listings(&events);
    use kaeri_model::reading::EditionListing;
    assert!(lines.contains(&("bare-shelf".into(), Some(EditionListing::Empty))), "{lines:?}");
    assert!(lines.contains(&("full-shelf".into(), Some(EditionListing::Listed))), "{lines:?}");
}

/// The source a series reads from is switched off: the page says so by the source's name, not
/// that a sign-in is needed under its id.
#[tokio::test]
async fn a_switched_off_source_is_named_as_switched_off() {
    let fake = FakeAniList::start().await;
    fake.add_books([book(WORK_ID, TITLE, "MANGA", (2020, 500), &[], &[])]);
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    kit.meta.book(kaeri_core::AnilistId(WORK_ID)).await.unwrap();
    Shelf::put(&kit, "night-shelf", "Night Shelf", 3, false);
    let work = WorkId::new(format!("a:{WORK_ID}"));
    kit.store
        .set_book_match("night-shelf", "ferry", work.as_str(), 90, false, false, NOW)
        .unwrap();
    kit.sources.registry.set_off(["night-shelf".to_owned()]);
    let (tx, mut rx) = outbox();

    crate::reading::chapters_of(kit.clone(), ask(), work, None, &tx);
    let events = all(&mut rx, Duration::from_millis(300)).await;
    let failure = events.iter().find_map(|event| match event {
        Event::Reading(ReadingEvent::Chapters { failure: Some(failure), .. }) => {
            Some(failure.clone())
        }
        _ => None,
    });
    assert_eq!(
        failure,
        Some(kaeri_model::reading::ReadingFailure::SourceOff { source: "Night Shelf".into() })
    );
}

/// With the network gone, a source that cannot answer reads as being offline, not as the
/// source being down: the device's network path is what failed.
#[tokio::test]
async fn offline_a_failing_source_reads_as_offline() {
    let fake = FakeAniList::start().await;
    fake.add_books([book(WORK_ID, TITLE, "MANGA", (2020, 500), &[], &[])]);
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    kit.meta.book(kaeri_core::AnilistId(WORK_ID)).await.unwrap();
    Shelf::put(&kit, "quiet-shelf", "Quiet Shelf", 0, true);
    kit.sources.write_back_network(true, NOW);
    let work = WorkId::new(format!("a:{WORK_ID}"));
    let (tx, mut rx) = outbox();

    editions::load(&kit, ask(), work, &tx);
    let events = all(&mut rx, Duration::from_millis(300)).await;
    let missing = events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::BookSources(BookSourceEvent::ForWork { missing, asking: false, .. }) => {
                Some(missing.clone())
            }
            _ => None,
        })
        .expect("Read from's answer");
    assert_eq!(missing.len(), 1, "{missing:?}");
    let words = format!("{:?}", missing[0].reason);
    assert!(words.contains("ReadingFailedOffline"), "{words}");
}

/// A source that lists its chapters at once until `hang` is set, then never answers, as a server
/// that accepts the connection and sends nothing back.
struct StuckShelf {
    manifest: BookSourceManifest,
    hang: std::sync::atomic::AtomicBool,
}

#[async_trait]
impl BookSource for StuckShelf {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, TITLE))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        if self.hang.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        Ok((1..=4).map(|n| SourceChapter::new(format!("c-{n}"), n.to_string())).collect())
    }

    async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
        Err(ProviderError::NotFound)
    }
}

/// A series whose list is kept shows that list at once when its source stops answering, rather
/// than an empty page until the source gives up.
#[tokio::test]
async fn a_kept_list_shows_at_once_while_a_stuck_source_is_asked() {
    let fake = FakeAniList::start().await;
    fake.add_books([book(WORK_ID, TITLE, "MANGA", (2020, 500), &[], &[])]);
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    kit.meta.book(kaeri_core::AnilistId(WORK_ID)).await.unwrap();
    let manifest = BookSourceManifest {
        id: "stuck-shelf".into(),
        name: "Stuck Shelf".into(),
        version: "1".into(),
        kind: BookSourceKind::Native,
        media: vec![Medium::Manga],
        languages: vec!["en".into()],
        allowed_hosts: Vec::new(),
        capabilities: Vec::new(),
    };
    let shelf = Arc::new(StuckShelf { manifest, hang: false.into() });
    kit.sources.registry.put(shelf.clone());
    let work = WorkId::new(format!("a:{WORK_ID}"));
    kit.store
        .set_book_match("stuck-shelf", "ferry", work.as_str(), 90, false, false, NOW)
        .unwrap();
    let (tx, mut rx) = outbox();

    // Listed once, and so kept.
    crate::reading::chapters_of(kit.clone(), ask(), work.clone(), None, &tx);
    all(&mut rx, Duration::from_millis(200)).await;

    shelf.hang.store(true, Ordering::SeqCst);
    crate::reading::chapters_of(kit.clone(), ask(), work, None, &tx);
    let events = all(&mut rx, Duration::from_millis(300)).await;
    let rows: Vec<usize> = events
        .iter()
        .filter_map(|event| match event {
            Event::Reading(ReadingEvent::Chapters { volumes, failure: None, .. }) => {
                Some(volumes.iter().map(|v| v.chapters.len()).sum())
            }
            _ => None,
        })
        .collect();
    assert_eq!(rows, [4], "{events:?}");
}
