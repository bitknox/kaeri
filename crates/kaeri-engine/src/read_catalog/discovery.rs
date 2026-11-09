//! Which discovery rows reading's Home shows, in what order, and what each one asks AniList.
//!
//! The reading mix decides: series on the person's lists plus chapters read in the last 90 days,
//! counted by format. The format read most gets Trending, New and Finished, the second Trending
//! and New, the third Trending. Because you finished follows the Trending rows and More of a
//! genre follows New. A format never read still gets its Trending row, near the end. With no
//! history at all, Home shows each format's Trending row and New manga.

use std::collections::{HashMap, HashSet};

use kaeri_core::media::ReadingFormat;
use kaeri_model::{
    Msg, ReadingMsg,
    reading::{READ_HOME_ROWS, ReadRowFormat, ReadRowKind, ReadSearchQuery, WorkTile},
};

/// How far back chapters read count toward the mix.
pub(crate) const MIX_DAYS: i64 = 90;

/// A row Home plans to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Planned {
    Trending(ReadRowFormat),
    New(ReadRowFormat),
    Finished(ReadRowFormat),
    Because,
    Genre,
}

/// The row a book of AniList's is counted in.
pub(crate) fn format_of(format: ReadingFormat) -> ReadRowFormat {
    match format {
        ReadingFormat::Novel => ReadRowFormat::Novels,
        ReadingFormat::Webtoon => ReadRowFormat::Webtoons,
        ReadingFormat::Manga | ReadingFormat::OneShot | ReadingFormat::Other(_) => {
            ReadRowFormat::Manga
        }
    }
}

/// The formats read, most first; between two read as much, in the order manga, webtoons, light
/// novels. A format never read is not in it.
pub(crate) fn ranked(counts: &HashMap<ReadRowFormat, u32>) -> Vec<ReadRowFormat> {
    let mut read: Vec<ReadRowFormat> = ReadRowFormat::ALL
        .into_iter()
        .filter(|f| counts.get(f).copied().unwrap_or_default() > 0)
        .collect();
    // A stable sort keeps the fixed order between equals.
    read.sort_by_key(|f| std::cmp::Reverse(counts.get(f).copied().unwrap_or_default()));
    read
}

/// The rows in order, at most [`READ_HOME_ROWS`]. `because` and `genre` say whether there is a
/// finished series and a genre read most to base those rows on.
pub(crate) fn plan(ranked: &[ReadRowFormat], because: bool, genre: bool) -> Vec<Planned> {
    if ranked.is_empty() {
        let mut out: Vec<Planned> =
            ReadRowFormat::ALL.into_iter().map(Planned::Trending).collect();
        out.push(Planned::New(ReadRowFormat::Manga));
        return out;
    }
    let mut out: Vec<Planned> = ranked.iter().copied().map(Planned::Trending).collect();
    if because {
        out.push(Planned::Because);
    }
    out.extend(ranked.iter().take(2).copied().map(Planned::New));
    if genre {
        out.push(Planned::Genre);
    }
    out.push(Planned::Finished(ranked[0]));
    out.extend(
        ReadRowFormat::ALL.into_iter().filter(|f| !ranked.contains(f)).map(Planned::Trending),
    );
    out.truncate(READ_HOME_ROWS as usize);
    out
}

impl Planned {
    /// The row's id, which is also where its titles are kept: `trending-manga`, `because`.
    pub fn id(self) -> String {
        match self {
            Self::Trending(f) => format!("trending-{}", f.key()),
            Self::New(f) => format!("new-{}", f.key()),
            Self::Finished(f) => format!("finished-{}", f.key()),
            Self::Because => "because".into(),
            Self::Genre => "genre".into(),
        }
    }

    pub fn kind(self) -> ReadRowKind {
        match self {
            Self::Trending(_) => ReadRowKind::Trending,
            Self::New(_) => ReadRowKind::New,
            Self::Finished(_) => ReadRowKind::Finished,
            Self::Because => ReadRowKind::Because,
            Self::Genre => ReadRowKind::MoreGenre,
        }
    }

    pub fn format(self) -> Option<ReadRowFormat> {
        match self {
            Self::Trending(f) | Self::New(f) | Self::Finished(f) => Some(f),
            Self::Because | Self::Genre => None,
        }
    }

    /// Whether the screen's own request brings it: the Trending rows do, the rest come from the
    /// paced queue.
    pub fn on_screen(self) -> bool {
        matches!(self, Self::Trending(_))
    }

    /// The search the row is, which View all opens: its first results are the row's titles in
    /// the same order. `None` for Because you finished, a short list of suggestions.
    pub fn query(self, genre: Option<&str>) -> Option<ReadSearchQuery> {
        let sorted = |sort: &str| ReadSearchQuery {
            sort: Some(sort.to_owned()),
            ..ReadSearchQuery::default()
        };
        Some(match self {
            Self::Trending(f) => {
                ReadSearchQuery { formats: f.search_formats(), ..sorted("TRENDING_DESC") }
            }
            Self::New(f) => ReadSearchQuery {
                formats: f.search_formats(),
                started: Some("12-months".into()),
                ..sorted("POPULARITY_DESC")
            },
            Self::Finished(f) => ReadSearchQuery {
                formats: f.search_formats(),
                status: Some("finished".into()),
                ..sorted("POPULARITY_DESC")
            },
            Self::Genre => {
                ReadSearchQuery { genres: vec![genre?.to_owned()], ..sorted("POPULARITY_DESC") }
            }
            Self::Because => return None,
        })
    }

    /// The heading. Because you finished needs the series' title and More of a genre the genre.
    pub fn title(self, because: Option<&str>, genre: Option<&str>) -> Option<Msg> {
        use ReadRowFormat::{Manga, Novels, Webtoons};
        let r = Msg::Reading;
        Some(match self {
            Self::Trending(Manga) => r(ReadingMsg::ReadHomeTrendingManga),
            Self::Trending(Webtoons) => r(ReadingMsg::ReadHomeTrendingWebtoons),
            Self::Trending(Novels) => r(ReadingMsg::ReadHomeTrendingNovels),
            Self::New(Manga) => r(ReadingMsg::ReadHomeNewManga),
            Self::New(Webtoons) => r(ReadingMsg::ReadHomeNewWebtoons),
            Self::New(Novels) => r(ReadingMsg::ReadHomeNewNovels),
            Self::Finished(Manga) => r(ReadingMsg::ReadHomeFinishedManga),
            Self::Finished(Webtoons) => r(ReadingMsg::ReadHomeFinishedWebtoons),
            Self::Finished(Novels) => r(ReadingMsg::ReadHomeFinishedNovels),
            Self::Because => Msg::HomeBecause { title: because?.to_owned() },
            Self::Genre => kaeri_model::search::more_genre(genre?),
        })
    }

    /// The line over View all's results, before the count: the format and the order in words.
    /// A genre row's is its heading. `None` where there is no View all.
    pub fn view_all_title(self, genre: Option<&str>) -> Option<Msg> {
        use ReadRowFormat::{Manga, Novels, Webtoons};
        let r = Msg::Reading;
        Some(match self {
            Self::Trending(Manga) => r(ReadingMsg::ReadViewAllTrendingManga),
            Self::Trending(Webtoons) => r(ReadingMsg::ReadViewAllTrendingWebtoons),
            Self::Trending(Novels) => r(ReadingMsg::ReadViewAllTrendingNovels),
            Self::New(Manga) => r(ReadingMsg::ReadViewAllNewManga),
            Self::New(Webtoons) => r(ReadingMsg::ReadViewAllNewWebtoons),
            Self::New(Novels) => r(ReadingMsg::ReadViewAllNewNovels),
            Self::Finished(Manga) => r(ReadingMsg::ReadViewAllFinishedManga),
            Self::Finished(Webtoons) => r(ReadingMsg::ReadViewAllFinishedWebtoons),
            Self::Finished(Novels) => r(ReadingMsg::ReadViewAllFinishedNovels),
            Self::Genre => kaeri_model::search::more_genre(genre?),
            Self::Because => return None,
        })
    }

    /// The side text beside the heading.
    pub fn side(self) -> Option<Msg> {
        match self {
            Self::New(_) => Some(Msg::Reading(ReadingMsg::ReadHomeNewSide)),
            Self::Finished(_) => Some(Msg::Reading(ReadingMsg::ReadHomeFinishedSide)),
            _ => None,
        }
    }
}

/// Each title once: every row loses what `seen` holds and what an earlier row shows, in order.
pub(crate) fn once_each(rows: &mut [Vec<WorkTile>], seen: &mut HashSet<String>) {
    for row in rows {
        row.retain(|tile| seen.insert(tile.work.0.clone()));
    }
}

#[cfg(test)]
mod tests {
    use kaeri_core::{books::WorkId, media::Medium};

    use super::*;
    use ReadRowFormat::{Manga, Novels, Webtoons};

    fn counts(of: &[(ReadRowFormat, u32)]) -> HashMap<ReadRowFormat, u32> {
        of.iter().copied().collect()
    }

    fn ids(plan: &[Planned]) -> Vec<String> {
        plan.iter().map(|p| p.id()).collect()
    }

    #[test]
    fn the_mix_ranks_formats_read_most_first_and_leaves_out_the_unread() {
        assert_eq!(
            ranked(&counts(&[(Manga, 3), (Novels, 7), (Webtoons, 1)])),
            [Novels, Manga, Webtoons]
        );
        assert_eq!(ranked(&counts(&[(Novels, 4)])), [Novels]);
        assert_eq!(ranked(&counts(&[(Manga, 0), (Webtoons, 0)])), []);
        // Equals keep the fixed order.
        assert_eq!(
            ranked(&counts(&[(Novels, 2), (Webtoons, 2), (Manga, 2)])),
            [Manga, Webtoons, Novels]
        );
        // One-shots count as manga, strips as webtoons.
        assert_eq!(format_of(ReadingFormat::OneShot), Manga);
        assert_eq!(format_of(ReadingFormat::Webtoon), Webtoons);
        assert_eq!(format_of(ReadingFormat::Novel), Novels);
    }

    #[test]
    fn three_formats_read_give_the_boards_eight_rows() {
        let plan = plan(&[Manga, Novels, Webtoons], true, true);
        assert_eq!(
            ids(&plan),
            [
                "trending-manga",
                "trending-novels",
                "trending-webtoons",
                "because",
                "new-manga",
                "new-novels",
                "genre",
                "finished-manga",
            ]
        );
        assert_eq!(plan.len(), READ_HOME_ROWS as usize);
    }

    #[test]
    fn a_format_never_read_keeps_its_trending_row_near_the_end() {
        let plan = plan(&[Novels], true, true);
        assert_eq!(
            ids(&plan),
            [
                "trending-novels",
                "because",
                "new-novels",
                "genre",
                "finished-novels",
                "trending-manga",
                "trending-webtoons",
            ]
        );
        // No finished series and no genre: those rows are not planned.
        assert_eq!(
            ids(&super::plan(&[Webtoons, Manga], false, false)),
            [
                "trending-webtoons",
                "trending-manga",
                "new-webtoons",
                "new-manga",
                "finished-webtoons",
                "trending-novels",
            ]
        );
    }

    #[test]
    fn a_first_run_gets_each_trending_row_and_new_manga() {
        assert_eq!(
            ids(&plan(&[], true, true)),
            ["trending-manga", "trending-webtoons", "trending-novels", "new-manga",]
        );
    }

    #[test]
    fn each_row_carries_the_search_view_all_opens() {
        let trending = Planned::Trending(Manga).query(None).unwrap();
        assert_eq!(trending.formats, ["manga", "one-shot"]);
        assert_eq!(trending.sort.as_deref(), Some("TRENDING_DESC"));
        let new = Planned::New(Webtoons).query(None).unwrap();
        assert_eq!(new.formats, ["webtoon"]);
        assert_eq!(new.started.as_deref(), Some("12-months"));
        assert_eq!(new.sort.as_deref(), Some("POPULARITY_DESC"));
        let finished = Planned::Finished(Novels).query(None).unwrap();
        assert_eq!(finished.status.as_deref(), Some("finished"));
        assert_eq!(finished.formats, ["novel"]);
        assert_eq!(Planned::Genre.query(Some("Romance")).unwrap().genres, ["Romance"]);
        assert_eq!(Planned::Genre.query(None), None);
        assert_eq!(Planned::Because.query(None), None);
        assert!(Planned::Trending(Novels).on_screen() && !Planned::New(Novels).on_screen());
        assert_eq!(
            Planned::Because.title(Some("Wren and the Iron Bridge"), None).unwrap().english(),
            "Because you finished Wren and the Iron Bridge"
        );
        assert_eq!(
            Planned::New(Novels).title(None, None).unwrap().english(),
            "New light novels"
        );
    }

    #[test]
    fn a_title_shows_once_and_never_one_of_the_persons_own() {
        let tile = |work: &str| WorkTile {
            work: WorkId::new(work),
            title: work.into(),
            medium: Medium::Manga,
            cover: None,
            format_label: None,
            progress: None,
            next: None,
            new_chapter: false,
            adult: false,
            facts: Vec::new(),
            demo: false,
            not_in_sources: false,
            origin: None,
            amount: None,
            started: None,
        };
        let mut rows = vec![
            vec![tile("a:1"), tile("a:2"), tile("a:3")],
            vec![tile("a:2"), tile("a:4")],
            vec![tile("a:4"), tile("a:5"), tile("a:1")],
        ];
        let mut seen: HashSet<String> = ["a:3".to_owned()].into_iter().collect();
        once_each(&mut rows, &mut seen);
        let works = |row: &[WorkTile]| row.iter().map(|t| t.work.0.clone()).collect::<Vec<_>>();
        assert_eq!(works(&rows[0]), ["a:1", "a:2"]);
        assert_eq!(works(&rows[1]), ["a:4"]);
        assert_eq!(works(&rows[2]), ["a:5"]);
    }
}
