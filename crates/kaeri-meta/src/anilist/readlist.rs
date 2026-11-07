//! The signed-in viewer's manga and novel lists: what a tracker sync reads and writes.
//!
//! A light request beside the anime library: only the entry's state and the title's id and
//! format come back, since the sync matches by id and the series page already holds the rest.

use kaeri_core::{ids::AnilistId, media::Medium};

use super::{AniList, AniListError, Result, query};

/// One row of the viewer's reading list.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadListEntry {
    pub anilist_id: AnilistId,
    /// A light novel is a `NOVEL` on the same list.
    pub medium: Medium,
    /// AniList's own status string: `CURRENT`, `REPEATING`, `COMPLETED`, ...
    pub status: String,
    pub chapters: u32,
    pub volumes: u32,
    /// On the 10-point decimal scale, `None` when unscored.
    pub score: Option<f32>,
    pub updated_at: i64,
}

impl AniList {
    /// The viewer's whole manga and novel list in one request.
    pub async fn read_library(&self, user_id: u32) -> Result<Vec<ReadListEntry>> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: serde_json::Value = self
            .execute(query::USER_READ_LIBRARY, serde_json::json!({ "userId": user_id }))
            .await?;
        let lists = data["MediaListCollection"]["lists"]
            .as_array()
            .ok_or_else(|| AniListError::Decode("reading list had no lists".into()))?;
        let mut out = Vec::new();
        for list in lists {
            for entry in list["entries"].as_array().into_iter().flatten() {
                match decode_entry(entry) {
                    Some(row) => out.push(row),
                    None => tracing::warn!("skipping an undecodable reading list entry"),
                }
            }
        }
        Ok(out)
    }

    /// Write the chapters and volumes read, the status and the score of one title. `None`
    /// fields are left out, since AniList reads null as clear this. The numbers are whole: a
    /// fraction is refused in validation.
    pub async fn save_read_entry(
        &self,
        id: AnilistId,
        chapters: Option<u32>,
        volumes: Option<u32>,
        status: Option<&str>,
        score: Option<f32>,
    ) -> Result<()> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let mut variables = serde_json::Map::new();
        variables.insert("mediaId".into(), id.get().into());
        if let Some(chapters) = chapters {
            variables.insert("progress".into(), chapters.into());
        }
        if let Some(volumes) = volumes {
            variables.insert("progressVolumes".into(), volumes.into());
        }
        if let Some(status) = status {
            variables.insert("status".into(), status.into());
        }
        if let Some(score) = score {
            variables.insert("scoreRaw".into(), ((score * 10.0).round() as u32).into());
        }
        let _: serde_json::Value = self
            .execute(query::SAVE_READ_PROGRESS, serde_json::Value::Object(variables))
            .await?;
        Ok(())
    }

    /// Take one manga or novel off the viewer's list. `Ok(false)` when it was not on it.
    pub async fn remove_read_entry(&self, id: AnilistId) -> Result<bool> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: serde_json::Value = match self
            .execute(query::READ_LIST_ENTRY, serde_json::json!({ "id": id.get() }))
            .await
        {
            Ok(data) => data,
            // No such title: nothing of it can be on the list.
            Err(AniListError::NotFound) => return Ok(false),
            Err(e) => return Err(e),
        };
        let Some(entry) = list_entry_id(&data) else { return Ok(false) };
        self.delete_entry(entry).await
    }
}

/// The entry id in a [`query::READ_LIST_ENTRY`] answer.
fn list_entry_id(data: &serde_json::Value) -> Option<u32> {
    u32::try_from(data["Media"]["mediaListEntry"]["id"].as_u64()?).ok()
}

fn decode_entry(entry: &serde_json::Value) -> Option<ReadListEntry> {
    let id = u32::try_from(entry["media"]["id"].as_u64()?).ok()?;
    let novel = entry["media"]["format"].as_str() == Some("NOVEL");
    Some(ReadListEntry {
        anilist_id: AnilistId::new(id),
        medium: if novel { Medium::Novel } else { Medium::Manga },
        status: entry["status"].as_str().unwrap_or("CURRENT").to_owned(),
        chapters: entry["progress"].as_u64().unwrap_or(0) as u32,
        volumes: entry["progressVolumes"].as_u64().unwrap_or(0) as u32,
        score: entry["score"].as_f64().map(|s| s as f32).filter(|s| *s > 0.0),
        updated_at: entry["updatedAt"].as_i64().unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_novel_is_told_from_a_manga_by_its_format() {
        let row = serde_json::json!({"status":"REPEATING","progress":12,"progressVolumes":2,
            "score":0,"updatedAt":9,"media":{"id":900001,"format":"NOVEL"}});
        let entry = decode_entry(&row).unwrap();
        assert_eq!(entry.medium, Medium::Novel);
        assert_eq!((entry.chapters, entry.volumes, entry.score), (12, 2, None));
        let row = serde_json::json!({"media":{"id":3,"format":"MANGA"}});
        assert_eq!(decode_entry(&row).unwrap().medium, Medium::Manga);
        assert!(decode_entry(&serde_json::json!({"status":"CURRENT"})).is_none());
    }

    #[test]
    fn the_entry_to_remove_is_the_list_entry_and_not_the_title() {
        let on = serde_json::json!({"Media":{"mediaListEntry":{"id":4_200_001}}});
        assert_eq!(list_entry_id(&on), Some(4_200_001));
        let off = serde_json::json!({"Media":{"mediaListEntry":null}});
        assert_eq!(list_entry_id(&off), None);
    }

    #[test]
    fn the_documents_parse() {
        for doc in [query::USER_READ_LIBRARY, query::SAVE_READ_PROGRESS, query::READ_LIST_ENTRY]
        {
            graphql_parser::parse_query::<String>(doc).unwrap();
        }
    }
}
