//! A series page's chapter list against a source whose list the test gives.

use kaeri_core::books::WorkId;
use kaeri_model::reading::{ChapterRow, VolumeGroup};

use super::*;

/// A source whose chapter list is whatever the test gives.
struct Lister {
    manifest: BookSourceManifest,
    chapters: Vec<SourceChapter>,
}

impl Lister {
    fn put(kit: &Kit, id: &str, chapters: Vec<SourceChapter>) {
        let manifest = BookSourceManifest {
            id: id.into(),
            name: "Lister".into(),
            version: "1".into(),
            kind: BookSourceKind::Plugin,
            media: vec![Medium::Manga],
            languages: vec!["en".into()],
            allowed_hosts: Vec::new(),
            capabilities: Vec::new(),
        };
        kit.sources.registry.put(Arc::new(Self { manifest, chapters }));
    }
}

#[async_trait]
impl BookSource for Lister {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }
    async fn search(&self, _: &str, _: &BookHints) -> Result<Vec<SeriesHit>, ProviderError> {
        Ok(Vec::new())
    }
    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        Ok(SeriesDetail::new(id, "The Copper Orchard"))
    }
    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        Ok(self.chapters.clone())
    }
    async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
        Err(ProviderError::NotFound)
    }
}

fn rows(volumes: &[VolumeGroup]) -> Vec<&ChapterRow> {
    volumes.iter().flat_map(|v| v.chapters.iter()).collect()
}

/// Two groups' versions of chapter 12 are one row, which counts both: every front end keys the
/// rows by id.
#[tokio::test]
async fn two_versions_of_a_chapter_are_one_row_with_both_counted() {
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(None, dir.path());
    let mut a = SourceChapter::new("c12a", "Chapter 12");
    a.version_label = Some("Group One".into());
    let mut b = SourceChapter::new("c12b", "Chapter 12");
    b.version_label = Some("Group Two".into());
    Lister::put(&kit, "lister", vec![a, b, SourceChapter::new("c13", "Chapter 13")]);
    let work = WorkId::new("a:900555");
    kit.store
        .set_book_match("lister", "series-1", work.as_str(), 90, true, false, NOW)
        .unwrap();

    let (_, mut volumes) = kit.sources.chapters(&work, None, NOW).await.unwrap();
    super::super::series::finish(&kit, &work, None, &mut volumes, NOW);
    let shown: Vec<(&str, u32, Option<&str>)> = rows(&volumes)
        .iter()
        .map(|r| (r.id.as_str(), r.versions, r.version_label.as_deref()))
        .collect();
    assert_eq!(shown, [("12", 2, Some("Group One")), ("13", 1, None)]);

    // The list as kept, shown when the source does not answer, is the same.
    let (_, mut kept) = super::super::series::kept_volumes(&kit, &work);
    super::super::series::finish(&kit, &work, None, &mut kept, NOW);
    let ids: Vec<&str> = rows(&kept).iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["12", "13"]);
}

/// The list as kept, which the page shows when the source does not answer, carries each
/// chapter's download: offline after a relaunch, the downloaded chapters still list and open.
#[tokio::test]
async fn the_kept_list_carries_the_downloads() {
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(None, dir.path());
    let list: Vec<SourceChapter> =
        (1..=3).map(|i| SourceChapter::new(format!("c{i}"), format!("Chapter {i}"))).collect();
    Lister::put(&kit, "lister", list);
    let work = WorkId::new("a:900557");
    kit.store
        .set_book_match("lister", "series-1", work.as_str(), 90, true, false, NOW)
        .unwrap();
    kit.sources.chapters(&work, None, NOW).await.unwrap();
    let ask = kaeri_store::book_downloads::BookDownloadAsk {
        work: work.as_str(),
        chapter: "2",
        source_id: "lister",
        version_label: None,
        keep_ready: false,
    };
    kit.store.queue_book_download(&ask, NOW).unwrap();
    let id = kit.store.book_download(work.as_str(), "2").unwrap().unwrap().id;
    let folder = tempfile::tempdir().unwrap();
    kit.store.book_download_done(id, folder.path().to_str().unwrap(), Some(3), 1, NOW).unwrap();

    let (_, mut kept) = super::super::series::kept_volumes(&kit, &work);
    super::super::series::finish(&kit, &work, None, &mut kept, NOW);
    let shown: Vec<(&str, bool)> = rows(&kept)
        .iter()
        .map(|r| {
            (
                r.id.as_str(),
                matches!(r.download, Some(kaeri_model::reading::ChapterDownloadState::Done)),
            )
        })
        .collect();
    assert_eq!(shown, [("1", false), ("2", true), ("3", false)]);
}

/// How long a series page's list of `n` chapters takes: kept from a first answer, kept again
/// from a second, and built into rows each time. The quickest of three runs, so a busy machine
/// stretches neither size more than the other.
async fn time_to_list(n: u32) -> std::time::Duration {
    let mut quickest = std::time::Duration::MAX;
    for _ in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let kit = kit(None, dir.path());
        let list: Vec<SourceChapter> = (1..=n)
            .map(|i| SourceChapter::new(format!("c{i}"), format!("Chapter {i}")))
            .collect();
        Lister::put(&kit, "lister", list);
        let work = WorkId::new("a:900556");
        kit.store
            .set_book_match("lister", "series-1", work.as_str(), 90, true, false, NOW)
            .unwrap();
        let started = std::time::Instant::now();
        for round in 0..2 {
            let (_, mut volumes) =
                kit.sources.chapters(&work, None, NOW + round).await.unwrap();
            super::super::series::finish(&kit, &work, None, &mut volumes, NOW);
            assert_eq!(rows(&volumes).len(), n as usize);
        }
        quickest = quickest.min(started.elapsed());
    }
    quickest
}

/// A very long list grows the work with its length, not with its square: sixteen times the
/// chapters take well under forty times as long (a square would take 256 times). The budget is
/// the small list's own time, so no machine is too slow for it.
#[tokio::test]
async fn a_long_chapter_list_takes_time_in_step_with_its_length() {
    let small = time_to_list(1_000).await;
    let large = time_to_list(16_000).await;
    let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-6);
    assert!(
        ratio < 40.0,
        "16 times the chapters took {ratio:.1} times as long ({small:?}, {large:?})"
    );
}
