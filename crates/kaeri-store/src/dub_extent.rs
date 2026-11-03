//! What this device's sources said of a title's dub, kept between launches: see
//! `0021_dub_extent`.
//!
//! A card reads these to say "Dub 8" with nothing asked. A row is shown however old it is, as a
//! dub does not take an episode back; `until` says only when the answer is due to be asked
//! again.

use std::collections::HashMap;

use kaeri_core::ids::AnilistId;
use rusqlite::OptionalExtension;

use crate::{Result, Store};

/// How long a stored answer is kept at all, shown or not.
pub const KEEP_FOR: i64 = 90 * 24 * 3_600;

/// One answer of the sources asked, for a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DubRow {
    /// A source listed a dub of the title. `false` is a source that listed the title and found
    /// none.
    pub has_dub: bool,
    /// The last whole episode the dub has, when the answer counts them.
    pub through: Option<u32>,
    /// The dub has every episode aired.
    pub complete: bool,
    /// How far the dub goes in AniList's own numbers, when they line up.
    pub aired: Option<u32>,
    /// When it was said, in Unix seconds.
    pub said_at: i64,
    /// When it is due to be asked again.
    pub until: i64,
}

impl Store {
    /// Keep `row` as what the sources named `route` said of title `id`, in place of what they
    /// said before.
    pub fn put_dub(&self, id: AnilistId, route: &str, row: &DubRow) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO dub_extent
                     (anilist_id, route, has_dub, through, complete, aired, said_at, until)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(anilist_id, route) DO UPDATE SET
                    has_dub  = excluded.has_dub,
                    through  = excluded.through,
                    complete = excluded.complete,
                    aired    = excluded.aired,
                    said_at  = excluded.said_at,
                    until    = excluded.until",
                rusqlite::params![
                    id.get(),
                    route,
                    row.has_dub,
                    row.through,
                    row.complete,
                    row.aired,
                    row.said_at,
                    row.until
                ],
            )?;
            Ok(())
        })
    }

    /// What the sources named `route` last said of title `id`.
    pub fn dub_row(&self, id: AnilistId, route: &str) -> Result<Option<DubRow>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT has_dub, through, complete, aired, said_at, until
                   FROM dub_extent WHERE anilist_id = ?1 AND route = ?2",
                rusqlite::params![id.get(), route],
                row_of,
            )
            .optional()?)
        })
    }

    /// What is known of each of `ids` from any sources: the answer with a dub that goes
    /// furthest, else the newest that said none. A title nothing was said of is left out.
    pub fn dubs_of(&self, ids: &[AnilistId]) -> Result<HashMap<AnilistId, DubRow>> {
        let mut out: HashMap<AnilistId, DubRow> = HashMap::new();
        for chunk in ids.chunks(400) {
            let marks = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT anilist_id, has_dub, through, complete, aired, said_at, until
                   FROM dub_extent WHERE anilist_id IN ({marks})"
            );
            self.with_conn(|c| {
                let mut statement = c.prepare(&sql)?;
                let rows = statement.query_map(
                    rusqlite::params_from_iter(chunk.iter().map(|id| id.get())),
                    |r| {
                        Ok((
                            AnilistId::new(r.get::<_, u32>(0)?),
                            DubRow {
                                has_dub: r.get(1)?,
                                through: r.get(2)?,
                                complete: r.get(3)?,
                                aired: r.get(4)?,
                                said_at: r.get(5)?,
                                until: r.get(6)?,
                            },
                        ))
                    },
                )?;
                for row in rows {
                    let (id, row) = row?;
                    match out.get(&id) {
                        Some(kept) if !better(&row, kept) => {}
                        _ => {
                            out.insert(id, row);
                        }
                    }
                }
                Ok(())
            })?;
        }
        Ok(out)
    }

    /// Whether any sources' answer for title `id` is still fresh at `now`.
    pub fn dub_fresh(&self, id: AnilistId, now: i64) -> Result<bool> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT EXISTS(SELECT 1 FROM dub_extent
                                WHERE anilist_id = ?1 AND route <> 'search' AND until > ?2)",
                rusqlite::params![id.get(), now],
                |r| r.get(0),
            )?)
        })
    }

    /// Forget everything said of title `id`: its match to a source changed.
    pub fn forget_dub(&self, id: AnilistId) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM dub_extent WHERE anilist_id = ?1", [id.get()])?;
            Ok(())
        })
    }

    /// Let go of the answers older than [`KEEP_FOR`] at `now`.
    pub fn prune_dubs(&self, now: i64) -> Result<usize> {
        self.with_conn(|c| {
            Ok(c.execute("DELETE FROM dub_extent WHERE said_at < ?1", [now - KEEP_FOR])?)
        })
    }
}

fn row_of(r: &rusqlite::Row<'_>) -> rusqlite::Result<DubRow> {
    Ok(DubRow {
        has_dub: r.get(0)?,
        through: r.get(1)?,
        complete: r.get(2)?,
        aired: r.get(3)?,
        said_at: r.get(4)?,
        until: r.get(5)?,
    })
}

/// Whether `a` says more of a title's dub than `b`: a dub over none, then the further one,
/// then the newer.
fn better(a: &DubRow, b: &DubRow) -> bool {
    let key = |row: &DubRow| (row.has_dub, row.complete, row.through, row.said_at);
    key(a) > key(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: AnilistId = AnilistId::new(900_201);
    const B: AnilistId = AnilistId::new(900_202);

    fn row(has_dub: bool, through: Option<u32>, said_at: i64) -> DubRow {
        DubRow {
            has_dub,
            through,
            complete: false,
            aired: through,
            said_at,
            until: said_at + 100,
        }
    }

    #[test]
    fn an_answer_is_kept_and_read_back_by_title_and_route() {
        let store = Store::open_in_memory().unwrap();
        store.put_dub(A, "pinned:one", &row(true, Some(8), 10)).unwrap();
        assert_eq!(store.dub_row(A, "pinned:one").unwrap(), Some(row(true, Some(8), 10)));
        assert_eq!(store.dub_row(A, "other").unwrap(), None);
        assert_eq!(store.dub_row(B, "pinned:one").unwrap(), None);
        store.put_dub(A, "pinned:one", &row(true, Some(9), 20)).unwrap();
        assert_eq!(store.dub_row(A, "pinned:one").unwrap().unwrap().through, Some(9));
    }

    #[test]
    fn a_card_reads_the_furthest_dub_any_sources_found() {
        let store = Store::open_in_memory().unwrap();
        store.put_dub(A, "one", &row(true, Some(4), 10)).unwrap();
        store.put_dub(A, "two", &row(true, Some(8), 5)).unwrap();
        store.put_dub(A, "three", &row(false, None, 30)).unwrap();
        store.put_dub(B, "one", &row(false, None, 30)).unwrap();
        let known = store.dubs_of(&[A, B, AnilistId::new(1)]).unwrap();
        assert_eq!(known.len(), 2);
        assert_eq!(known[&A].through, Some(8));
        assert!(!known[&B].has_dub);
    }

    #[test]
    fn freshness_ignores_a_searchs_answers_and_old_ones_are_let_go() {
        let store = Store::open_in_memory().unwrap();
        store.put_dub(A, "search", &row(true, None, 10)).unwrap();
        assert!(!store.dub_fresh(A, 50).unwrap(), "a search says a dub exists, not how far");
        store.put_dub(A, "one", &row(true, Some(2), 10)).unwrap();
        assert!(store.dub_fresh(A, 50).unwrap());
        assert!(!store.dub_fresh(A, 500).unwrap());
        assert_eq!(store.prune_dubs(10 + KEEP_FOR).unwrap(), 0);
        assert_eq!(store.prune_dubs(11 + KEEP_FOR).unwrap(), 2);
        store.put_dub(B, "one", &row(true, Some(2), 10)).unwrap();
        store.forget_dub(B).unwrap();
        assert!(store.dubs_of(&[B]).unwrap().is_empty());
    }
}
