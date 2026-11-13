//! Leaving the reader while its open still waits for a source to list the chapters.

use std::time::Duration;

use async_trait::async_trait;
use kaeri_core::{
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, SeriesDetail, SeriesHit,
        SourceChapter,
    },
    error::ProviderError,
};

use super::*;

/// A source of one chapter of pictures that lists its chapters after `slow`.
struct Slow {
    manifest: BookSourceManifest,
    slow: Duration,
}

#[async_trait]
impl kaeri_core::traits::BookSource for Slow {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }
    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }
    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, "The Brass Meadow"))
    }
    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        tokio::time::sleep(self.slow).await;
        Ok(vec![SourceChapter::new("c1", "1")])
    }
    async fn content(&self, _: &str, chapter: &str) -> Result<ChapterContent, ProviderError> {
        Ok(ChapterContent::Images(vec![kaeri_core::books::PageRef::new(format!(
            "https://pages.example/{chapter}/1"
        ))]))
    }
}

const SLOW_WORK: &str = "a:900888";

/// The person opens a series only a slow source holds, sees "Finding source", and leaves with
/// the session the open becomes. When the source answers, nothing opens and nothing is held.
#[tokio::test(start_paused = true)]
async fn leaving_while_finding_the_source_opens_nothing() {
    leave_while_finding(true).await;
}

/// The same with Close and no session, as an app sends while it has none: whatever open waits
/// is dropped.
#[tokio::test(start_paused = true)]
async fn leaving_with_no_session_while_finding_the_source_opens_nothing() {
    leave_while_finding(false).await;
}

async fn leave_while_finding(named: bool) {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
    let source = Arc::new(Slow {
        manifest: BookSourceManifest {
            id: "slow-books".into(),
            name: "Slow".into(),
            version: "1".into(),
            kind: BookSourceKind::Plugin,
            media: vec![Medium::Manga],
            languages: vec!["en".into()],
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        },
        slow: Duration::from_secs(8),
    });
    h.services.book_sources.registry.put(source);
    h.services
        .store
        .set_book_match("slow-books", "series-1", SLOW_WORK, 90, true, false, 1)
        .unwrap();
    h.send(ReaderCommand::Open {
        work: WorkId::new(SLOW_WORK),
        chapter: None,
        at: OpenAt::Start,
        surface: surface(),
    });
    let wait = h
        .until(|e| match e {
            ReaderEvent::Opening { wait, .. } => Some(wait.clone()),
            _ => None,
        })
        .await;
    // The source is known by then: the line names it and what it is asked for.
    assert_eq!(wait, Some(ReaderWait::ChapterList { source: "Slow".into() }));
    let pending = h.reader.listing.as_ref().map(|l| l.id).expect("the open waits");
    let session = if named { pending } else { 0 };
    h.send(ReaderCommand::Close { session });
    let closed = h
        .until(|e| match e {
            ReaderEvent::Closed { session } => Some(*session),
            _ => None,
        })
        .await;
    assert_eq!(closed, session);
    // The source answers after 8 s; a minute is long past it. Nothing opens meanwhile.
    let opened = tokio::time::timeout(
        Duration::from_secs(60),
        h.until(|e| match e {
            ReaderEvent::Opened(view) => Some(view.session),
            _ => None,
        }),
    )
    .await;
    assert!(opened.is_err(), "the reader opened session {opened:?} after the person left");
    assert!(h.reader.session.is_none(), "no session is held");
    assert!(h.reader.listing.is_none(), "no open waits");
}

/// An open of a series only a slow source holds says what it waits on, in order: the chapter
/// list from the source, then the chapter's pages from it, each naming the source as the
/// person knows it.
#[tokio::test(start_paused = true)]
async fn an_open_names_each_stage_it_waits_on() {
    let mut h = harness_of(Store::open_in_memory().unwrap(), "manga", |_| Vec::new());
    let source = Arc::new(Slow {
        manifest: BookSourceManifest {
            id: "slow-books".into(),
            name: "Slow Books".into(),
            version: "1".into(),
            kind: BookSourceKind::Plugin,
            media: vec![Medium::Manga],
            languages: vec!["en".into()],
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        },
        slow: Duration::from_secs(8),
    });
    h.services.book_sources.registry.put(source);
    h.services
        .store
        .set_book_match("slow-books", "series-1", SLOW_WORK, 90, true, false, 1)
        .unwrap();
    h.send(ReaderCommand::Open {
        work: WorkId::new(SLOW_WORK),
        chapter: None,
        at: OpenAt::Start,
        surface: surface(),
    });
    let mut stages = Vec::new();
    while stages.len() < 2 {
        let wait = h
            .until(|e| match e {
                ReaderEvent::Opening { wait, .. } => Some(wait.clone()),
                _ => None,
            })
            .await;
        stages.push(wait);
    }
    assert_eq!(
        stages,
        [
            Some(ReaderWait::ChapterList { source: "Slow Books".into() }),
            Some(ReaderWait::ChapterPages { chapter: "1".into(), source: "Slow Books".into() }),
        ]
    );
}
