//! Where the people the viewer follows on AniList stopped, put on the episode rows a front end
//! is sent ([`EpisodeRow::followed`]).
//!
//! The people come with a title's page, in the request it makes anyway, and its episodes come in
//! a table of their own, before or after it. So the marks are put on here, on the way out: a
//! table is marked with the people its page last brought, and a page that brings people after
//! its table was sent has the table sent again, marked. Nothing is asked for this.

use std::collections::VecDeque;

use kaeri_core::ids::AnilistId;
use kaeri_model::{
    catalog::{Entry, FollowedEntry},
    episodes::{EpisodeRow, followed_marks},
    event::Event,
};

/// How many titles' people are kept for the tables that come after their pages.
const TITLES: usize = 16;

/// The people each recent page brought, and the last table and page sent, to mark again.
#[derive(Debug, Default)]
pub struct Followed {
    /// Each title's people, as its last whole page brought them, newest last.
    titles: VecDeque<(AnilistId, Vec<FollowedEntry>)>,
    /// The last table sent, as it was sent.
    table: Option<(AnilistId, Vec<EpisodeRow>)>,
    /// The last whole page sent that listed people, to send again without them when the switch
    /// is turned off.
    page: Option<Box<Entry>>,
}

impl Followed {
    /// Mark `event` on its way to the front end, and say what has to follow it: the last table
    /// sent, marked again, when its page brings people it was sent without (or loses some).
    pub fn pass(&mut self, event: &mut Event) -> Option<Event> {
        match event {
            Event::Detail(entry) => {
                // A partial page, from a season's kept header, says nothing of the people.
                let detail = entry.detail.as_ref().filter(|detail| !detail.partial)?;
                let (id, following) = (entry.id, detail.following.clone());
                self.page = (!following.is_empty()).then(|| entry.clone());
                self.keep(id, following.clone());
                let (table, rows) = self.table.as_mut().filter(|(table, _)| *table == id)?;
                let before: Vec<_> = rows.iter().map(|row| row.followed.clone()).collect();
                followed_marks(rows, &following);
                let moved = rows.iter().map(|row| &row.followed).ne(before.iter());
                moved.then(|| Event::Episodes { id: *table, rows: rows.clone(), failed: None })
            }
            Event::Episodes { id, rows, .. } => {
                followed_marks(rows, self.of(*id));
                self.table = Some((*id, rows.clone()));
                None
            }
            _ => None,
        }
    }

    /// Show people you follow was switched off: forget them, and say again without them what was
    /// last sent with them, the page and its table, so a page on screen follows the switch.
    pub fn hide(&mut self) -> Vec<Event> {
        self.titles.clear();
        let mut said = Vec::new();
        if let Some(mut page) = self.page.take()
            && let Some(detail) = page.detail.as_mut()
        {
            detail.following.clear();
            said.push(Event::Detail(page));
        }
        if let Some((id, rows)) = self.table.as_mut()
            && rows.iter().any(|row| !row.followed.is_empty())
        {
            followed_marks(rows, &[]);
            said.push(Event::Episodes { id: *id, rows: rows.clone(), failed: None });
        }
        said
    }

    /// The people title `id`'s last page brought, or nobody.
    fn of(&self, id: AnilistId) -> &[FollowedEntry] {
        self.titles.iter().rev().find(|(title, _)| *title == id).map_or(&[], |(_, p)| p)
    }

    fn keep(&mut self, id: AnilistId, following: Vec<FollowedEntry>) {
        self.titles.retain(|(title, _)| *title != id);
        if !following.is_empty() {
            self.titles.push_back((id, following));
        }
        while self.titles.len() > TITLES {
            self.titles.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use kaeri_model::catalog::{LibrarySegment, TitleDetail};

    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_001);

    fn person(
        name: &str,
        status: LibrarySegment,
        progress: u32,
        score: Option<u32>,
    ) -> FollowedEntry {
        FollowedEntry {
            name: name.into(),
            avatar_url: Some(format!("https://img.example/avatar/{name}.png")),
            status: Some(status),
            score,
            progress,
        }
    }

    fn page(following: Vec<FollowedEntry>) -> Event {
        let mut entry = Entry::new(SHOW, "Lighthouse Kites");
        entry.detail = Some(TitleDetail { following, ..TitleDetail::default() });
        Event::Detail(Box::new(entry))
    }

    fn table(count: u32) -> Event {
        let rows = (1..=count)
            .map(|n| EpisodeRow { number: n.to_string(), ..EpisodeRow::default() })
            .collect();
        Event::Episodes { id: SHOW, rows, failed: None }
    }

    fn marks(event: &Event) -> Vec<(String, Vec<String>)> {
        let Event::Episodes { rows, .. } = event else { panic!("not a table: {event:?}") };
        rows.iter()
            .filter(|row| !row.followed.is_empty())
            .map(|row| {
                (row.number.clone(), row.followed.iter().map(|m| m.name.clone()).collect())
            })
            .collect()
    }

    fn people() -> Vec<FollowedEntry> {
        use LibrarySegment::*;
        vec![
            person("Ines", Watching, 7, Some(80)),
            person("Lumi", Watching, 7, Some(70)),
            person("SannaQ", Watching, 9, None),
            person("Rowanpaper", Completed, 12, Some(90)),
            person("TeoVale", Paused, 4, None),
            person("Kofi", Planning, 0, None),
            person("Yunami", Dropped, 2, Some(40)),
            person("Halden", Watching, 0, None),
        ]
    }

    #[test]
    fn a_table_after_its_page_is_marked_where_each_watcher_stopped() {
        let mut followed = Followed::default();
        assert!(followed.pass(&mut page(people())).is_none(), "no table yet");
        let mut sent = table(12);
        assert!(followed.pass(&mut sent).is_none());
        assert_eq!(
            marks(&sent),
            [
                ("4".to_owned(), vec!["TeoVale".to_owned()]),
                ("7".into(), vec!["Ines".into(), "Lumi".into()]),
                ("9".into(), vec!["SannaQ".into()]),
            ],
            "watching and paused only: completed, planning, dropped and nought mark nothing"
        );
        let Event::Episodes { rows, .. } = &sent else { unreachable!() };
        assert_eq!(
            rows[6].followed[0].avatar_url.as_deref(),
            Some("https://img.example/avatar/Ines.png")
        );
    }

    #[test]
    fn a_page_after_its_table_sends_the_table_again_marked() {
        let mut followed = Followed::default();
        let mut sent = table(12);
        followed.pass(&mut sent);
        assert!(marks(&sent).is_empty());
        let again = followed.pass(&mut page(people())).expect("the table again");
        assert_eq!(marks(&again).len(), 3);
        // The same people again move nothing, so nothing is sent again.
        assert!(followed.pass(&mut page(people())).is_none());
        // A page with nobody takes the marks off.
        let cleared = followed.pass(&mut page(Vec::new())).expect("the table without them");
        assert!(marks(&cleared).is_empty());
    }

    #[test]
    fn a_partial_page_or_another_titles_leaves_the_table_alone() {
        let mut followed = Followed::default();
        followed.pass(&mut page(people()));
        followed.pass(&mut table(12));
        let Event::Detail(mut partial) = page(Vec::new()) else { unreachable!() };
        partial.detail.as_mut().unwrap().partial = true;
        assert!(followed.pass(&mut Event::Detail(partial)).is_none());
        let mut other = Entry::new(AnilistId::new(7), "Paper Moon Radio");
        other.detail = Some(TitleDetail::default());
        assert!(followed.pass(&mut Event::Detail(Box::new(other))).is_none());
        let mut later = table(12);
        followed.pass(&mut later);
        assert_eq!(marks(&later).len(), 3, "the show's people still stand");
    }

    #[test]
    fn switching_off_says_the_page_and_its_table_again_without_them() {
        let mut followed = Followed::default();
        followed.pass(&mut page(people()));
        followed.pass(&mut table(12));
        let said = followed.hide();
        assert_eq!(said.len(), 2);
        let Event::Detail(entry) = &said[0] else { panic!("the page first") };
        assert!(entry.detail.as_ref().unwrap().following.is_empty());
        assert!(marks(&said[1]).is_empty());
        let mut next = table(12);
        followed.pass(&mut next);
        assert!(marks(&next).is_empty(), "forgotten");
        assert!(followed.hide().is_empty(), "nothing left to say");
    }
}
