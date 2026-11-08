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
    super::super::series::finish(&kit, &work, &mut volumes, NOW);
    let shown: Vec<(&str, u32, Option<&str>)> = rows(&volumes)
        .iter()
        .map(|r| (r.id.as_str(), r.versions, r.version_label.as_deref()))
        .collect();
    assert_eq!(shown, [("12", 2, Some("Group One")), ("13", 1, None)]);

    // The list as kept, shown when the source does not answer, is the same.
    let (_, mut kept) = super::super::series::kept_volumes(&kit, &work);
    super::super::series::finish(&kit, &work, &mut kept, NOW);
    let ids: Vec<&str> = rows(&kept).iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["12", "13"]);
}
