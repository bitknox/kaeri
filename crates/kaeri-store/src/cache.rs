//! The media cache: the last successful metadata answer for each title.
//!
//! Present in the schema since 0001 and wired the day AniList's instability made the
//! reason obvious: metadata should be fetched fresh whenever a fresh source answers,
//! but "no source answered" must degrade to *yesterday's* data rather than to a screen
//! that cannot play an episode watched thirty times. Every successful fetch rewrites
//! its row, so the cache is exactly as stale as the last outage is long.

use kaeri_core::ids::AnilistId;

use crate::{Result, Store};

impl Store {
    /// Upsert one title's serialized metadata.
    pub fn cache_media(&self, anilist_id: AnilistId, payload: &str, now: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO media_cache (anilist_id, payload, fetched_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(anilist_id) DO UPDATE
                    SET payload = excluded.payload, fetched_at = excluded.fetched_at",
                rusqlite::params![anilist_id.get(), payload, now],
            )?;
            Ok(())
        })
    }

    /// How many titles the cache can answer for — the Providers screen's number.
    pub fn cached_media_count(&self) -> Result<u32> {
        self.with_conn(|c| {
            Ok(c.query_row("SELECT COUNT(*) FROM media_cache", [], |r| r.get(0))?)
        })
    }

    /// Take the people the viewer follows out of every cached page: after a sign-out from
    /// AniList, or once Show people you follow is switched off, nothing of them stays on the
    /// device. The rest of each page stays as it was, its fetch time too.
    pub fn forget_following(&self) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE media_cache SET payload = json_remove(payload, '$.following')
                 WHERE json_valid(payload) AND json_type(payload, '$.following') IS NOT NULL",
                [],
            )?;
            Ok(())
        })
    }

    /// The last cached payload and when it was fetched.
    pub fn cached_media(&self, anilist_id: AnilistId) -> Result<Option<(String, i64)>> {
        self.with_conn(|c| {
            let row = c
                .query_row(
                    "SELECT payload, fetched_at FROM media_cache WHERE anilist_id = ?1",
                    [anilist_id.get()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            Ok(row)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSHIKUZU: AnilistId = AnilistId::new(154_587);

    #[test]
    fn forgetting_the_people_followed_leaves_the_rest_of_each_page() {
        let store = Store::open_in_memory().unwrap();
        let with = r#"{"id":154587,"title":"Kept","following":[{"user":{"name":"Mika"}}]}"#;
        store.cache_media(HOSHIKUZU, with, 1_000).unwrap();
        let other = AnilistId::new(7);
        store.cache_media(other, r#"{"id":7,"title":"Other"}"#, 2_000).unwrap();
        store.forget_following().unwrap();
        let (payload, at) = store.cached_media(HOSHIKUZU).unwrap().unwrap();
        assert!(!payload.contains("Mika") && !payload.contains("following"), "{payload}");
        assert!(payload.contains("Kept"));
        assert_eq!(at, 1_000, "its fetch time stands");
        assert_eq!(
            store.cached_media(other).unwrap().unwrap().0,
            r#"{"id":7,"title":"Other"}"#
        );
    }

    #[test]
    fn a_fresh_fetch_replaces_the_stale_row() {
        let store = Store::open_in_memory().unwrap();
        store.cache_media(HOSHIKUZU, r#"{"id":154587,"old":true}"#, 1_000).unwrap();
        store.cache_media(HOSHIKUZU, r#"{"id":154587,"old":false}"#, 2_000).unwrap();

        let (payload, at) = store.cached_media(HOSHIKUZU).unwrap().expect("cached");
        assert!(payload.contains("false"));
        assert_eq!(at, 2_000);
    }

    #[test]
    fn a_title_never_fetched_is_simply_absent() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.cached_media(HOSHIKUZU).unwrap().is_none());
    }
}
