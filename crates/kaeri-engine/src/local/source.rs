//! Your media as a source: the Provider that plays files from the folders added.

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use kaeri_core::{
    error::ProviderError,
    ids::{AnilistId, ExternalIds, ProviderKey},
    media::{Episode, SearchHit, Translation},
    stream::{Stream, StreamKind},
    traits::{Provider, ProviderKind, ProviderManifest},
};

use super::{Group, Standing};

/// The source's id in `providers.order` and in the chain.
pub const ID: &str = "library-local";

pub struct LocalSource {
    manifest: ProviderManifest,
    groups: Arc<RwLock<Vec<Group>>>,
}

impl LocalSource {
    pub fn new(groups: Vec<Group>) -> Self {
        Self {
            manifest: ProviderManifest {
                id: ID.into(),
                display_name: "Your media".into(),
                version: "1".into(),
                kind: ProviderKind::Library,
                allowed_hosts: Vec::new(),
                translations: vec![Translation::Sub],
                torrents: false,
            },
            groups: Arc::new(RwLock::new(groups)),
        }
    }

    /// Only a match that is sure, or a kept title, plays: a likely match waits to be confirmed.
    fn playable(&self) -> Vec<Group> {
        let groups = self.groups.read().unwrap_or_else(|e| e.into_inner());
        groups
            .iter()
            .filter(|g| matches!(g.standing, Standing::Matched | Standing::Kept))
            .cloned()
            .collect()
    }

    fn group(&self, key: &ProviderKey) -> Option<Group> {
        self.playable().into_iter().find(|g| g.id == key.as_str())
    }
}

fn hit(group: &Group) -> SearchHit {
    SearchHit::new(group.id.as_str(), group.name.as_str())
}

/// The label of each file's episode: its number when the name had one or the viewer set it,
/// else its place. A file waiting for Set episode is left out, and a special the viewer set in
/// season 0 is `SP` and its number.
fn labelled(group: &Group) -> Vec<(String, PathBuf)> {
    group
        .files
        .iter()
        .filter(|f| !group.waits_for_episode(f))
        .enumerate()
        .map(|(n, f)| {
            let number = f.episode.unwrap_or(n as u32 + 1);
            let label = match f.season {
                Some(0) => format!("SP{number}"),
                _ => number.to_string(),
            };
            (label, PathBuf::from(&f.path))
        })
        .collect()
}

#[async_trait]
impl Provider for LocalSource {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _: Translation,
        _hints: &kaeri_core::media::TitleHints,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        let needle = query.trim().to_lowercase();
        Ok(self
            .playable()
            .iter()
            .filter(|g| !needle.is_empty() && g.name.to_lowercase().contains(&needle))
            .map(hit)
            .collect())
    }

    fn titles(&self) -> Vec<AnilistId> {
        // Kept-unmatched titles have ids of their own, which AniList does not know.
        let mut ids: Vec<AnilistId> = self
            .playable()
            .iter()
            .filter_map(|g| g.anilist)
            .filter(|id| !id.is_own())
            .collect();
        ids.sort_unstable_by_key(|id| id.get());
        ids.dedup();
        ids
    }

    async fn lookup(&self, ids: &ExternalIds) -> Result<Option<SearchHit>, ProviderError> {
        Ok(self
            .playable()
            .iter()
            .find(|g| ids.anilist.is_some() && g.anilist == ids.anilist)
            .map(hit))
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        _: Translation,
        _hints: &kaeri_core::media::TitleHints,
    ) -> Result<Vec<Episode>, ProviderError> {
        let group = self.group(key).ok_or(ProviderError::NotFound)?;
        Ok(labelled(&group)
            .into_iter()
            .map(|(label, _)| Episode::new(label.as_str()))
            .collect())
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        _: Translation,
        _hints: &kaeri_core::media::TitleHints,
    ) -> Result<Vec<Stream>, ProviderError> {
        let group = self.group(key).ok_or(ProviderError::NotFound)?;
        let (_, path) = labelled(&group)
            .into_iter()
            .find(|(label, _)| label == episode.trim())
            .ok_or(ProviderError::NotFound)?;
        if !path.is_file() {
            // A drive that is not plugged in, or a file that was moved.
            return Err(ProviderError::Unavailable("the file is not where it was".into()));
        }
        let mut stream = Stream::new(path.to_string_lossy().into_owned(), StreamKind::Mp4);
        stream.provider_id = ID.into();
        Ok(vec![stream])
    }
}

#[cfg(test)]
mod tests {
    use kaeri_core::ids::AnilistId;

    use super::*;
    use crate::local::{FileEntry, Standing};

    fn group(standing: Standing, path: &str) -> Group {
        Group {
            id: "g".into(),
            folder: "f".into(),
            name: "Lantern Harbour".into(),
            files: vec![FileEntry::found(path.into(), Some(2))],
            standing,
            decided: false,
            anilist: Some(AnilistId::new(900_002)),
            matched_title: None,
            offline: false,
            confidence: None,
        }
    }

    #[tokio::test]
    async fn a_matched_file_plays_from_its_path_and_a_missing_one_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Lantern Harbour - 02.mkv");
        std::fs::write(&file, b"").unwrap();
        let source = LocalSource::new(vec![group(Standing::Matched, file.to_str().unwrap())]);
        let key = ProviderKey::new("g");
        let streams =
            source.resolve(&key, "2", Translation::Sub, &Default::default()).await.unwrap();
        assert_eq!(streams[0].url, file.to_str().unwrap());
        std::fs::remove_file(&file).unwrap();
        assert!(matches!(
            source.resolve(&key, "2", Translation::Sub, &Default::default()).await,
            Err(ProviderError::Unavailable(_))
        ));
    }

    #[tokio::test]
    async fn a_file_with_no_number_waits_until_its_episode_is_set() {
        let mut g = group(Standing::Matched, "/m/Lantern Harbour - 02.mkv");
        g.files.push(FileEntry::found("/m/extras/Making of.mkv".into(), None));
        let source = LocalSource::new(vec![g.clone()]);
        let key = ProviderKey::new("g");
        let labels = |eps: Vec<Episode>| {
            eps.into_iter().map(|e| e.number.as_str().to_owned()).collect::<Vec<_>>()
        };
        assert_eq!(
            labels(source.episodes(&key, Translation::Sub, &Default::default()).await.unwrap()),
            ["2"]
        );
        g.files[1] =
            FileEntry { season: Some(0), episode: Some(1), set: true, ..g.files[1].clone() };
        let source = LocalSource::new(vec![g]);
        assert_eq!(
            labels(source.episodes(&key, Translation::Sub, &Default::default()).await.unwrap()),
            ["2", "SP1"]
        );
    }

    #[tokio::test]
    async fn a_match_that_waits_to_be_confirmed_does_not_play() {
        let source = LocalSource::new(vec![group(Standing::NeedsLook, "/x.mkv")]);
        let found =
            source.lookup(&ExternalIds::anilist(AnilistId::new(900_002))).await.unwrap();
        assert!(found.is_none());
    }
}
