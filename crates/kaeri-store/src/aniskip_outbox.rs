//! What waits to be sent to AniSkip: see `0019_aniskip_outbox`.
//!
//! The store keeps each request as the JSON the playback layer made of it, and does not read
//! it. A submission names the mark it came from, so the id AniSkip gives it can be kept on the
//! mark once it is sent.

use kaeri_core::ids::AnilistId;

use crate::{Result, Store};

/// One request waiting for AniSkip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AniSkipItem {
    pub id: i64,
    /// The request, as JSON.
    pub request: String,
    /// The mark a submission came from: show, episode and AniSkip's name for the kind.
    pub mark: Option<(AnilistId, String, String)>,
    /// How many times it has been tried.
    pub attempts: u32,
}

impl Store {
    /// Queue `request` to be sent from `at`, naming the `mark` it submits, if it does.
    pub fn queue_aniskip(
        &self,
        request: &str,
        mark: Option<(AnilistId, &str, &str)>,
        at: i64,
    ) -> Result<i64> {
        self.with_conn(|c| {
            let (anilist_id, episode, kind) = match mark {
                Some((id, episode, kind)) => (Some(id.get()), Some(episode), Some(kind)),
                None => (None, None, None),
            };
            c.execute(
                "INSERT INTO aniskip_outbox (request, anilist_id, episode, kind, due_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![request, anilist_id, episode, kind, at],
            )?;
            Ok(c.last_insert_rowid())
        })
    }

    /// Up to `limit` requests due at `now`, oldest first.
    pub fn due_aniskip(&self, now: i64, limit: u32) -> Result<Vec<AniSkipItem>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT id, request, anilist_id, episode, kind, attempts FROM aniskip_outbox
                  WHERE failed = 0 AND due_at <= ?1 ORDER BY due_at, id LIMIT ?2",
            )?;
            let rows = statement.query_map(rusqlite::params![now, limit], |r| {
                let anilist_id: Option<u32> = r.get(2)?;
                let episode: Option<String> = r.get(3)?;
                let kind: Option<String> = r.get(4)?;
                Ok(AniSkipItem {
                    id: r.get(0)?,
                    request: r.get(1)?,
                    mark: match (anilist_id, episode, kind) {
                        (Some(id), Some(episode), Some(kind)) => {
                            Some((AnilistId::new(id), episode, kind))
                        }
                        _ => None,
                    },
                    attempts: r.get(5)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// When the next request is due, if one waits.
    pub fn next_aniskip_due(&self) -> Result<Option<i64>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT MIN(due_at) FROM aniskip_outbox WHERE failed = 0",
                [],
                |r| r.get(0),
            )?)
        })
    }

    /// `id` was sent: it no longer waits.
    pub fn aniskip_sent(&self, id: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM aniskip_outbox WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// `id` was tried and is to be tried again at `at`.
    pub fn aniskip_later(&self, id: i64, at: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE aniskip_outbox SET attempts = attempts + 1, due_at = ?2 WHERE id = ?1",
                rusqlite::params![id, at],
            )?;
            Ok(())
        })
    }

    /// `id` was let go: kept, so Settings can say how many were not sent.
    pub fn aniskip_failed(&self, id: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE aniskip_outbox SET attempts = attempts + 1, failed = 1 WHERE id = ?1",
                [id],
            )?;
            Ok(())
        })
    }

    /// How many requests were let go without being sent.
    pub fn aniskip_failed_count(&self) -> Result<u32> {
        self.with_conn(|c| {
            Ok(c.query_row("SELECT COUNT(*) FROM aniskip_outbox WHERE failed = 1", [], |r| {
                r.get(0)
            })?)
        })
    }

    /// Forget everything waiting: sharing was turned off.
    pub fn clear_aniskip(&self) -> Result<u32> {
        self.with_conn(|c| {
            let gone = c.execute("DELETE FROM aniskip_outbox", [])?;
            Ok(u32::try_from(gone).unwrap_or(u32::MAX))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SkipMark;

    const SHOW: AnilistId = AnilistId::new(900_202);

    fn store() -> Store {
        Store::open_in_memory().expect("store")
    }

    #[test]
    fn a_request_waits_until_it_is_due() {
        let store = store();
        let vote = store.queue_aniskip(r#"{"op":"vote"}"#, None, 100).unwrap();
        let submit =
            store.queue_aniskip(r#"{"op":"submit"}"#, Some((SHOW, "3", "op")), 50).unwrap();
        assert!(store.due_aniskip(40, 10).unwrap().is_empty());
        let due = store.due_aniskip(100, 10).unwrap();
        assert_eq!(due.iter().map(|i| i.id).collect::<Vec<_>>(), [submit, vote]);
        assert_eq!(due[0].mark, Some((SHOW, "3".to_owned(), "op".to_owned())));
        assert_eq!(due[1].mark, None);
        assert_eq!(store.next_aniskip_due().unwrap(), Some(50));
    }

    #[test]
    fn a_request_tried_again_waits_and_counts_the_attempt() {
        let store = store();
        let id = store.queue_aniskip("{}", None, 10).unwrap();
        store.aniskip_later(id, 500).unwrap();
        assert!(store.due_aniskip(499, 10).unwrap().is_empty());
        assert_eq!(store.due_aniskip(500, 10).unwrap()[0].attempts, 1);
        store.aniskip_sent(id).unwrap();
        assert_eq!(store.next_aniskip_due().unwrap(), None);
    }

    #[test]
    fn a_request_let_go_is_counted_and_not_sent_again() {
        let store = store();
        let id = store.queue_aniskip("{}", None, 10).unwrap();
        store.aniskip_failed(id).unwrap();
        assert!(store.due_aniskip(i64::MAX, 10).unwrap().is_empty());
        assert_eq!(store.aniskip_failed_count().unwrap(), 1);
        assert_eq!(store.next_aniskip_due().unwrap(), None);
    }

    #[test]
    fn clearing_the_marks_or_turning_sharing_off_empties_it() {
        let store = store();
        store.queue_aniskip("{}", None, 10).unwrap();
        store
            .put_skip_mark(&SkipMark {
                anilist_id: SHOW,
                episode: "3".into(),
                kind: "op".into(),
                start_ms: 0,
                end_ms: 90_000,
                length_ms: 1_420_000,
                marked_at: 1,
                skip_id: None,
            })
            .unwrap();
        store.clear_skip_marks().unwrap();
        assert!(store.due_aniskip(i64::MAX, 10).unwrap().is_empty());
        store.queue_aniskip("{}", None, 10).unwrap();
        assert_eq!(store.clear_aniskip().unwrap(), 1);
    }
}
