//! Skip times found from the audio: each episode's fingerprints, the stretches found with them,
//! and per show and kind whether the person trusts them. See `0023_skip_detect`.
//!
//! Fingerprints stay on this device. A print is kept for the last three episodes of a show that
//! were fingerprinted, and none longer than 60 days.

use kaeri_core::ids::AnilistId;

use crate::{Result, Store};

/// How many episodes of a show keep their prints.
pub const PRINTS_PER_SHOW: i64 = 3;
/// How long a print is kept, in seconds.
pub const PRINT_DAYS_SECS: i64 = 60 * 24 * 60 * 60;

/// One part of one episode's fingerprint: its head or its tail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipPrint {
    pub anilist_id: AnilistId,
    pub episode: String,
    /// `head` or `tail`.
    pub part: String,
    /// The episode's length.
    pub length_ms: i64,
    /// Where in the episode the first item is.
    pub start_ms: i64,
    /// The items, little-endian.
    pub items: Vec<u8>,
    pub taken_at: i64,
}

/// What became of a found stretch.
pub mod state {
    /// Found and offered.
    pub const FOUND: &str = "found";
    /// Skipped at least once and not undone.
    pub const SKIPPED: &str = "skipped";
    /// The person said Yes, or saved it in review: it is a mark of theirs now.
    pub const CONFIRMED: &str = "confirmed";
    /// Undone after a skip: the person wanted to watch it.
    pub const REJECTED: &str = "rejected";
    /// Confirmed and handed to the AniSkip outbox.
    pub const SHARED: &str = "shared";
}

/// A stretch found in one episode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundSkip {
    pub anilist_id: AnilistId,
    pub episode: String,
    /// AniSkip's name for the kind: `op` or `ed`.
    pub kind: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub length_ms: i64,
    /// The episodes whose prints agreed, comma-separated.
    pub agreed: String,
    /// One of [`state`].
    pub state: String,
    pub found_at: i64,
}

/// Per show and kind: how found stretches have gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FoundShow {
    /// Found stretches skipped without an undo since the last undo.
    pub skips: u32,
    /// The person confirmed one.
    pub confirmed: bool,
    /// The person was asked about one, whatever they answered.
    pub asked: bool,
}

impl FoundShow {
    /// Whether this kind's found stretches are skipped by themselves: confirmed once, or skipped
    /// twice without going back.
    pub const fn trusted(&self) -> bool {
        self.confirmed || self.skips >= 2
    }
}

impl Store {
    /// Keep `print` in place of the episode's earlier one for its part, then drop prints of the
    /// show's older episodes past the last three, and any older than 60 days.
    pub fn put_skip_print(&self, print: &SkipPrint) -> Result<()> {
        self.with_tx(|tx| {
            tx.execute(
                "INSERT INTO skip_prints
                    (anilist_id, episode, part, length_ms, start_ms, items, taken_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(anilist_id, episode, part) DO UPDATE SET
                    length_ms = excluded.length_ms,
                    start_ms  = excluded.start_ms,
                    items     = excluded.items,
                    taken_at  = excluded.taken_at",
                rusqlite::params![
                    print.anilist_id.get(),
                    print.episode,
                    print.part,
                    print.length_ms,
                    print.start_ms,
                    print.items,
                    print.taken_at,
                ],
            )?;
            tx.execute(
                "DELETE FROM skip_prints WHERE anilist_id = ?1 AND episode NOT IN (
                    SELECT episode FROM skip_prints WHERE anilist_id = ?1
                     GROUP BY episode ORDER BY MAX(taken_at) DESC LIMIT ?2)",
                rusqlite::params![print.anilist_id.get(), PRINTS_PER_SHOW],
            )?;
            tx.execute(
                "DELETE FROM skip_prints WHERE taken_at < ?1",
                [print.taken_at - PRINT_DAYS_SECS],
            )?;
            Ok(())
        })
    }

    /// The show's prints of `part`, newest first.
    pub fn skip_prints(&self, anilist_id: AnilistId, part: &str) -> Result<Vec<SkipPrint>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT anilist_id, episode, part, length_ms, start_ms, items, taken_at
                   FROM skip_prints WHERE anilist_id = ?1 AND part = ?2
                  ORDER BY taken_at DESC",
            )?;
            let rows = statement.query_map(rusqlite::params![anilist_id.get(), part], |r| {
                Ok(SkipPrint {
                    anilist_id: AnilistId::new(r.get(0)?),
                    episode: r.get(1)?,
                    part: r.get(2)?,
                    length_ms: r.get(3)?,
                    start_ms: r.get(4)?,
                    items: r.get(5)?,
                    taken_at: r.get(6)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Keep a found stretch, unless the person already answered for that episode and kind.
    pub fn put_found_skip(&self, found: &FoundSkip) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO skip_found
                    (anilist_id, episode, kind, start_ms, end_ms, length_ms, agreed, state, found_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(anilist_id, episode, kind) DO UPDATE SET
                    start_ms  = excluded.start_ms,
                    end_ms    = excluded.end_ms,
                    length_ms = excluded.length_ms,
                    agreed    = excluded.agreed,
                    found_at  = excluded.found_at
                 WHERE skip_found.state IN ('found', 'skipped')",
                rusqlite::params![
                    found.anilist_id.get(),
                    found.episode,
                    found.kind,
                    found.start_ms,
                    found.end_ms,
                    found.length_ms,
                    found.agreed,
                    found.state,
                    found.found_at,
                ],
            )?;
            Ok(())
        })
    }

    /// The stretches found in one episode, by kind.
    pub fn found_skips(&self, anilist_id: AnilistId, episode: &str) -> Result<Vec<FoundSkip>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT anilist_id, episode, kind, start_ms, end_ms, length_ms, agreed, state,
                        found_at
                   FROM skip_found WHERE anilist_id = ?1 AND episode = ?2 ORDER BY kind",
            )?;
            let rows =
                statement.query_map(rusqlite::params![anilist_id.get(), episode], |r| {
                    Ok(FoundSkip {
                        anilist_id: AnilistId::new(r.get(0)?),
                        episode: r.get(1)?,
                        kind: r.get(2)?,
                        start_ms: r.get(3)?,
                        end_ms: r.get(4)?,
                        length_ms: r.get(5)?,
                        agreed: r.get(6)?,
                        state: r.get(7)?,
                        found_at: r.get(8)?,
                    })
                })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Note what became of the found stretch of `kind` in one episode.
    pub fn set_found_skip_state(
        &self,
        anilist_id: AnilistId,
        episode: &str,
        kind: &str,
        state: &str,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE skip_found SET state = ?4
                  WHERE anilist_id = ?1 AND episode = ?2 AND kind = ?3",
                rusqlite::params![anilist_id.get(), episode, kind, state],
            )?;
            Ok(())
        })
    }

    /// How found stretches of `kind` have gone for one show.
    pub fn found_skip_show(&self, anilist_id: AnilistId, kind: &str) -> Result<FoundShow> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT skips, confirmed, asked FROM skip_found_show
                  WHERE anilist_id = ?1 AND kind = ?2",
            )?;
            let mut rows = statement.query(rusqlite::params![anilist_id.get(), kind])?;
            Ok(match rows.next()? {
                Some(r) => FoundShow {
                    skips: r.get(0)?,
                    confirmed: r.get::<_, i64>(1)? != 0,
                    asked: r.get::<_, i64>(2)? != 0,
                },
                None => FoundShow::default(),
            })
        })
    }

    fn update_found_show(&self, anilist_id: AnilistId, kind: &str, set: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO skip_found_show (anilist_id, kind) VALUES (?1, ?2)
                 ON CONFLICT(anilist_id, kind) DO NOTHING",
                rusqlite::params![anilist_id.get(), kind],
            )?;
            c.execute(
                &format!(
                    "UPDATE skip_found_show SET {set} WHERE anilist_id = ?1 AND kind = ?2"
                ),
                rusqlite::params![anilist_id.get(), kind],
            )?;
            Ok(())
        })
    }

    /// A found stretch of `kind` was skipped.
    pub fn note_found_skip(&self, anilist_id: AnilistId, kind: &str) -> Result<()> {
        self.update_found_show(anilist_id, kind, "skips = skips + 1")
    }

    /// A skip over a found stretch of `kind` was undone: the count starts again.
    pub fn undo_found_skip(&self, anilist_id: AnilistId, kind: &str) -> Result<()> {
        self.update_found_show(anilist_id, kind, "skips = 0")
    }

    /// The person confirmed a found stretch of `kind`.
    pub fn confirm_found_skips(&self, anilist_id: AnilistId, kind: &str) -> Result<()> {
        self.update_found_show(anilist_id, kind, "confirmed = 1")
    }

    /// The person was asked about a found stretch of `kind`. Whether they had not been yet.
    pub fn ask_found_skip(&self, anilist_id: AnilistId, kind: &str) -> Result<bool> {
        let before = self.found_skip_show(anilist_id, kind)?;
        self.update_found_show(anilist_id, kind, "asked = 1")?;
        Ok(!before.asked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_202);

    fn store() -> Store {
        Store::open_in_memory().expect("store")
    }

    fn print(episode: &str, part: &str, taken_at: i64) -> SkipPrint {
        SkipPrint {
            anilist_id: SHOW,
            episode: episode.into(),
            part: part.into(),
            length_ms: 1_420_000,
            start_ms: if part == "head" { 0 } else { 1_180_000 },
            items: vec![1, 2, 3, 4],
            taken_at,
        }
    }

    #[test]
    fn prints_are_kept_for_the_last_three_episodes_and_sixty_days() {
        let store = store();
        for (k, episode) in ["1", "2", "3", "4"].iter().enumerate() {
            let at = 1_000 + k as i64;
            store.put_skip_print(&print(episode, "head", at)).unwrap();
            store.put_skip_print(&print(episode, "tail", at)).unwrap();
        }
        let heads = store.skip_prints(SHOW, "head").unwrap();
        let episodes: Vec<&str> = heads.iter().map(|p| p.episode.as_str()).collect();
        assert_eq!(episodes, ["4", "3", "2"]);
        assert_eq!(heads[0], print("4", "head", 1_003));
        assert_eq!(store.skip_prints(SHOW, "tail").unwrap().len(), 3);

        let later = 1_003 + PRINT_DAYS_SECS + 1;
        store.put_skip_print(&print("5", "head", later)).unwrap();
        let heads = store.skip_prints(SHOW, "head").unwrap();
        assert_eq!(heads.len(), 1, "the old ones are gone");
        assert!(store.skip_prints(SHOW, "tail").unwrap().is_empty());
    }

    fn found(state: &str, start_ms: i64) -> FoundSkip {
        FoundSkip {
            anilist_id: SHOW,
            episode: "3".into(),
            kind: "op".into(),
            start_ms,
            end_ms: start_ms + 88_000,
            length_ms: 1_420_000,
            agreed: "2".into(),
            state: state.into(),
            found_at: 10,
        }
    }

    #[test]
    fn a_found_stretch_is_found_again_until_the_person_answers() {
        let store = store();
        store.put_found_skip(&found(state::FOUND, 41_000)).unwrap();
        store.put_found_skip(&found(state::FOUND, 42_000)).unwrap();
        assert_eq!(store.found_skips(SHOW, "3").unwrap(), [found(state::FOUND, 42_000)]);
        store.set_found_skip_state(SHOW, "3", "op", state::CONFIRMED).unwrap();
        store.put_found_skip(&found(state::FOUND, 50_000)).unwrap();
        assert_eq!(store.found_skips(SHOW, "3").unwrap(), [found(state::CONFIRMED, 42_000)]);
    }

    #[test]
    fn a_kind_is_trusted_after_a_yes_or_two_skips_and_an_undo_starts_over() {
        let store = store();
        assert_eq!(store.found_skip_show(SHOW, "op").unwrap(), FoundShow::default());
        store.note_found_skip(SHOW, "op").unwrap();
        assert!(!store.found_skip_show(SHOW, "op").unwrap().trusted());
        store.undo_found_skip(SHOW, "op").unwrap();
        store.note_found_skip(SHOW, "op").unwrap();
        assert!(!store.found_skip_show(SHOW, "op").unwrap().trusted());
        store.note_found_skip(SHOW, "op").unwrap();
        assert!(store.found_skip_show(SHOW, "op").unwrap().trusted());
        assert!(!store.found_skip_show(SHOW, "ed").unwrap().trusted(), "per kind");

        assert!(store.ask_found_skip(SHOW, "ed").unwrap());
        assert!(!store.ask_found_skip(SHOW, "ed").unwrap(), "once per show and kind");
        store.confirm_found_skips(SHOW, "ed").unwrap();
        let ed = store.found_skip_show(SHOW, "ed").unwrap();
        assert!(ed.trusted() && ed.asked);
    }

    #[test]
    fn clearing_skip_times_clears_what_was_found() {
        let store = store();
        store.put_skip_print(&print("1", "head", 10)).unwrap();
        store.put_found_skip(&found(state::FOUND, 41_000)).unwrap();
        store.note_found_skip(SHOW, "op").unwrap();
        store.clear_skip_marks().unwrap();
        assert!(store.skip_prints(SHOW, "head").unwrap().is_empty());
        assert!(store.found_skips(SHOW, "3").unwrap().is_empty());
        assert_eq!(store.found_skip_show(SHOW, "op").unwrap(), FoundShow::default());
    }
}
