//! The rooms this profile was in: the last ten, newest first, to go back to one.
//!
//! A row keeps the room's secret and this device's identity key in it: membership tokens, kept
//! here like the rest of a profile's history and never in a log, a bug report or a TV handoff.
//! It holds no source and no stream. The rooms are personal: each profile sees only its own.

use kaeri_core::ids::AnilistId;

use crate::{Result, Store};

/// How many rooms are kept.
pub const KEPT: usize = 10;

/// One room this profile was in.
#[derive(Clone, PartialEq)]
pub struct RecentRoom {
    /// The room's local id (from its secret), hex. Never sent anywhere.
    pub room_id: String,
    /// The room's secret, base64url: a membership token.
    pub secret: String,
    /// The room's relays when they aren't the built-in set.
    pub relays: Vec<String>,
    /// This device's identity key in the room, hex: a membership token.
    pub identity: String,
    /// The name this device used there.
    pub name: String,
    pub anilist_id: AnilistId,
    pub title: String,
    /// The last episode the room reached.
    pub episode: String,
    /// Who was there besides this device.
    pub names: Vec<String>,
    pub cover_url: Option<String>,
    pub last_at: i64,
}

impl std::fmt::Debug for RecentRoom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the secret or the identity key.
        f.debug_struct("RecentRoom")
            .field("room_id", &self.room_id)
            .field("title", &self.title)
            .field("episode", &self.episode)
            .field("names", &self.names)
            .field("last_at", &self.last_at)
            .finish_non_exhaustive()
    }
}

fn lines(text: &str) -> Vec<String> {
    text.lines().filter(|n| !n.is_empty()).map(str::to_owned).collect()
}

const COLUMNS: &str = "room_id, secret, relays, identity, name, anilist_id, title, episode, \
                       names, cover_url, last_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RecentRoom> {
    Ok(RecentRoom {
        room_id: r.get(0)?,
        secret: r.get(1)?,
        relays: lines(&r.get::<_, String>(2)?),
        identity: r.get(3)?,
        name: r.get(4)?,
        anilist_id: AnilistId::new(r.get::<_, u32>(5)?),
        title: r.get(6)?,
        episode: r.get(7)?,
        names: lines(&r.get::<_, String>(8)?),
        cover_url: r.get(9)?,
        last_at: r.get(10)?,
    })
}

impl Store {
    /// Keep `room` as the newest, in place of the same room before it, and let go of all but
    /// the last ten.
    pub fn remember_room(&self, room: &RecentRoom) -> Result<()> {
        self.with_personal_tx(|tx| {
            tx.execute(
                &format!(
                    "INSERT INTO room_recent ({COLUMNS})
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                     ON CONFLICT(room_id) DO UPDATE SET
                        secret     = excluded.secret,
                        relays     = excluded.relays,
                        identity   = excluded.identity,
                        name       = excluded.name,
                        anilist_id = excluded.anilist_id,
                        title      = excluded.title,
                        episode    = excluded.episode,
                        names      = excluded.names,
                        cover_url  = excluded.cover_url,
                        last_at    = excluded.last_at"
                ),
                rusqlite::params![
                    room.room_id,
                    room.secret,
                    room.relays.join("\n"),
                    room.identity,
                    room.name,
                    room.anilist_id.get(),
                    room.title,
                    room.episode,
                    room.names.join("\n"),
                    room.cover_url,
                    room.last_at,
                ],
            )?;
            tx.execute(
                "DELETE FROM room_recent WHERE rowid NOT IN
                    (SELECT rowid FROM room_recent ORDER BY last_at DESC, rowid DESC LIMIT ?1)",
                [KEPT as i64],
            )?;
            Ok(())
        })
    }

    /// The rooms kept, newest first.
    pub fn recent_rooms(&self) -> Result<Vec<RecentRoom>> {
        self.with_personal(|c| {
            let mut statement = c.prepare(&format!(
                "SELECT {COLUMNS} FROM room_recent ORDER BY last_at DESC, rowid DESC"
            ))?;
            let rows = statement.query_map([], row)?.collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// One room kept, by its id.
    pub fn recent_room(&self, room_id: &str) -> Result<Option<RecentRoom>> {
        self.with_personal(|c| {
            let mut statement =
                c.prepare(&format!("SELECT {COLUMNS} FROM room_recent WHERE room_id = ?1"))?;
            let mut rows = statement.query_map([room_id], row)?;
            Ok(rows.next().transpose()?)
        })
    }

    /// Let go of one room.
    pub fn forget_room(&self, room_id: &str) -> Result<()> {
        self.with_personal(|c| {
            c.execute("DELETE FROM room_recent WHERE room_id = ?1", [room_id])?;
            Ok(())
        })
    }
}

#[cfg(test)]
pub(crate) fn sample(room_id: &str, title: &str, names: &[&str], at: i64) -> RecentRoom {
    RecentRoom {
        room_id: room_id.into(),
        secret: "MDEyMzQ1Njc4OWFiY2RlZg".into(),
        relays: vec!["wss://relay.home.example".into()],
        identity: "11".repeat(32),
        name: "Ada".into(),
        anilist_id: AnilistId::new(900_741),
        title: title.into(),
        episode: "3".into(),
        names: names.iter().map(|n| (*n).to_owned()).collect(),
        cover_url: None,
        last_at: at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_ten_rooms_are_kept_newest_first() {
        let store = Store::open_in_memory().unwrap();
        for n in 0..12 {
            let room =
                sample(&format!("{n:032x}"), &format!("Title {n}"), &["Mika", "Ren"], 100 + n);
            store.remember_room(&room).unwrap();
        }
        let rooms = store.recent_rooms().unwrap();
        assert_eq!(rooms.len(), KEPT);
        assert_eq!(rooms[0].title, "Title 11");
        assert_eq!(rooms[9].title, "Title 2");
        assert_eq!(rooms[0].names, ["Mika", "Ren"]);
        assert_eq!(rooms[0].relays, ["wss://relay.home.example"]);
    }

    #[test]
    fn a_room_seen_again_is_one_row_with_its_latest_episode() {
        let store = Store::open_in_memory().unwrap();
        store.remember_room(&sample("a", "One", &[], 10)).unwrap();
        store.remember_room(&sample("b", "Two", &[], 20)).unwrap();
        let mut again = sample("a", "One", &[], 30);
        again.episode = "4".into();
        store.remember_room(&again).unwrap();
        let rooms = store.recent_rooms().unwrap();
        assert_eq!(rooms.len(), 2);
        assert_eq!((rooms[0].room_id.as_str(), rooms[0].episode.as_str()), ("a", "4"));
        assert_eq!(store.recent_room("a").unwrap().unwrap().identity, "11".repeat(32));
        store.forget_room("a").unwrap();
        assert_eq!(store.recent_rooms().unwrap().len(), 1);
        assert!(store.recent_room("a").unwrap().is_none());
    }

    #[test]
    fn its_secrets_never_show_in_a_debug_print() {
        let room = sample("a", "One", &[], 1);
        let printed = format!("{room:?}");
        assert!(!printed.contains("MDEyMzQ1") && !printed.contains("1111"), "{printed}");
    }
}
