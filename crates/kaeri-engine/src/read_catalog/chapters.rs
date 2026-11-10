//! Each source's chapter list for a series, as it last answered, kept in `book_chapter_cache`:
//! what Updates, Continue's next chapter, versions and the edition counts read without asking
//! the source again. A chapter seen for the first time on a later answer is dated then, so a
//! source that dates nothing still shows what is new.

use std::collections::{HashMap, HashSet};

use kaeri_core::books::{ChapterId, ChapterRange, SourceChapter};
use kaeri_store::{Store, book_catalog::BookRhythm, reading::ReadWork};
use serde::{Deserialize, Serialize};

/// One chapter as a source listed it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    /// The canonical id: "12", "10.5", "v3/2".
    pub id: String,
    /// The source's own id for it.
    pub source_ref: String,
    pub label: String,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub volume: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub version_label: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub published: Option<i64>,
    #[serde(default)]
    pub pages: Option<u32>,
    /// When a later answer first listed it. `None` for what the first answer listed.
    #[serde(default)]
    pub seen_at: Option<i64>,
    /// Its part of the chapter, from 1, for an entry that is one part of a chapter (see
    /// [`super::arrange`]); `id` is then the whole chapter's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<u32>,
}

impl Kept {
    /// When it came out, or when it was first seen.
    pub fn out_at(&self) -> Option<i64> {
        self.published.or(self.seen_at)
    }

    pub fn range(&self) -> Option<ChapterRange> {
        ChapterId::new(self.id.clone()).key().and_then(|k| k.range())
    }

    /// The chapter as the source listed it, read back from the list as kept: its number as the
    /// source gave it, as a part's ("1.2") is not its chapter's.
    pub fn source_chapter(&self) -> SourceChapter {
        SourceChapter {
            id: self.source_ref.clone(),
            number_raw: self.label.clone(),
            number: self
                .number
                .as_deref()
                .and_then(ChapterRange::parse)
                .or_else(|| self.range()),
            volume: self.volume.clone(),
            title: self.title.clone(),
            version_label: self.version_label.clone(),
            language: self.language.clone(),
            published: self.published,
            page_count: self.pages,
        }
    }

    /// Its title, unless the title only names the chapter again (see [`own_title`]).
    pub fn own_title(&self) -> Option<&str> {
        own_title(self.title.as_deref(), &self.label, self.number.as_deref())
    }
}

/// A chapter's title, unless it only names the chapter again: the label itself, the number, or
/// the number beside one word, as "Chapter 15" or "Ch. 15" in a book's own details.
pub fn own_title<'a>(
    title: Option<&'a str>,
    label: &str,
    number: Option<&str>,
) -> Option<&'a str> {
    let title = title?.trim();
    let same = |other: &str| title.eq_ignore_ascii_case(other.trim());
    if title.is_empty() || same(label) || number.is_some_and(same) {
        return None;
    }
    if let Some(number) = number.filter(|n| !n.is_empty()) {
        let rest = title.replacen(number, "", 1);
        let rest = rest.trim_matches(|c: char| c.is_whitespace() || c.is_ascii_punctuation());
        if rest.len() < title.len() && !rest.contains(char::is_whitespace) {
            return None;
        }
    }
    Some(title)
}

/// A source's list as kept, oldest chapter first, and when it was asked.
pub fn kept(store: &Store, source: &str, series: &str) -> Option<(Vec<Kept>, i64)> {
    let (json, at) = store.book_chapters(source, series).ok()??;
    let mut list: Vec<Kept> = serde_json::from_str(&json).ok()?;
    sort(&mut list);
    Some((list, at))
}

/// In reading order: by number, then those without one in the order the source gave.
pub fn sort(list: &mut [Kept]) {
    list.sort_by_cached_key(|c| (c.range().is_none(), c.range()));
}

/// Where the chapters an answer left out are remembered, by id with when each was first seen.
fn left_key(source: &str, series: &str) -> String {
    format!("book_chapters_left:{source}/{series}")
}

/// Keep a source's answer, and give the chapters it lists that no answer listed before. The
/// first answer for a series gives none: everything in it was out before.
///
/// An empty answer is no answer: a server part way through a scan or a scrape that broke says
/// nothing of what came out. It is not kept, so the next full answer is not all new; and a
/// list kept empty before counts as none. A short answer (a later page of the source's list
/// that failed) is kept as the list, and the chapters it left out are remembered: back in a
/// later answer, they are known, not new.
pub fn keep(
    store: &Store,
    source: &str,
    series: &str,
    chapters: &[SourceChapter],
    now: i64,
) -> Vec<Kept> {
    if chapters.is_empty() {
        return Vec::new();
    }
    let before = kept(store, source, series).map(|(list, _)| list).filter(|l| !l.is_empty());
    // By id, so a long list is looked through once rather than once a chapter.
    let before: Option<HashMap<&str, &Kept>> =
        before.as_ref().map(|list| list.iter().map(|k| (k.id.as_str(), k)).collect());
    let key = left_key(source, series);
    let left: HashMap<String, Option<i64>> = store
        .get_meta_text(&key)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    // Parts are kept under their whole chapter, so what is new is a chapter, not a part.
    let arranged = super::arrange::arrange(chapters, None, None);
    let mut fresh: Vec<Kept> = Vec::new();
    let mut fresh_ids = HashSet::new();
    let list: Vec<Kept> = chapters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let id = arranged.ids[i].as_str().to_owned();
            // When an earlier answer first listed it, if one did.
            let earlier = before.as_ref().map(|b| match b.get(id.as_str()) {
                Some(k) => Some(k.seen_at),
                None => left.get(&id).copied(),
            });
            let seen_at = match earlier {
                None => None,
                Some(Some(seen_at)) => seen_at,
                Some(None) => Some(now),
            };
            let kept = Kept {
                id,
                source_ref: c.id.clone(),
                label: c.number_raw.clone(),
                number: c.number.map(|n| n.to_string()),
                volume: c.volume.clone(),
                title: c.title.clone(),
                version_label: c.version_label.clone(),
                language: c.language.clone(),
                published: c.published,
                pages: c.page_count,
                seen_at,
                part: arranged.parts[i],
            };
            // Two versions of a new chapter are one chapter come out.
            if matches!(earlier, Some(None)) && fresh_ids.insert(kept.id.clone()) {
                fresh.push(kept.clone());
            }
            kept
        })
        .collect();
    let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
    if let Err(e) = store.put_book_chapters(source, series, &json, now) {
        tracing::warn!(error = %e, "a chapter list was not kept");
    }
    // What this answer left out of what was listed before, or left out already.
    let listed: HashSet<&str> = list.iter().map(|k| k.id.as_str()).collect();
    let mut still_left: HashMap<String, Option<i64>> = left
        .iter()
        .filter(|(id, _)| !listed.contains(id.as_str()))
        .map(|(id, seen)| (id.clone(), *seen))
        .collect();
    for (id, k) in before.iter().flatten() {
        if !listed.contains(id) {
            still_left.insert((*id).to_owned(), k.seen_at);
        }
    }
    if still_left != left {
        let kept = if still_left.is_empty() {
            store.delete_meta(&key)
        } else {
            let text = serde_json::to_string(&still_left).unwrap_or_default();
            store.set_meta_text(&key, &text)
        };
        if let Err(e) = kept {
            tracing::warn!(error = %e, "the chapters an answer left out were not kept");
        }
    }
    fresh
}

/// What the person has read of a work: the chapters marked, and the watermark.
#[derive(Debug, Clone, Default)]
pub struct Read {
    pub marked: HashSet<String>,
    pub work: Option<ReadWork>,
    /// The chapter they are part way into, with the page and the pages.
    pub in_progress: Option<(String, Option<u32>, Option<u32>)>,
    /// The furthest chapter read, by number.
    pub furthest: Option<ChapterRange>,
}

impl Read {
    pub fn of(store: &Store, work: &str) -> Self {
        let chapters = store.read_chapters(work).unwrap_or_default();
        let work_row = store.read_work(work).ok().flatten();
        let marked: HashSet<String> =
            chapters.iter().filter(|c| c.read).map(|c| c.chapter.clone()).collect();
        let last = work_row.as_ref().and_then(|w| w.last_chapter.clone());
        let in_progress = chapters
            .iter()
            .filter(|c| !c.read && (c.last_page.is_some() || c.novel_pos.is_some()))
            .max_by_key(|c| (Some(&c.chapter) == last.as_ref(), c.stamp))
            .map(|c| (c.chapter.clone(), c.last_page, c.of_pages));
        let range = |id: &str| ChapterId::new(id).key().and_then(|k| k.range());
        let mut furthest = marked.iter().filter_map(|id| range(id)).max();
        if let Some(milli) = work_row.as_ref().and_then(|w| w.adopt_milli)
            && let Some(adopted) = ChapterRange::parse(&milli_label(milli))
        {
            furthest = furthest.max(Some(adopted));
        }
        Self { marked, work: work_row, in_progress, furthest }
    }

    pub fn is_read(&self, id: &str) -> bool {
        self.marked.contains(id) || self.work.as_ref().is_some_and(|w| w.adopted(id))
    }

    pub fn any(&self) -> bool {
        !self.marked.is_empty() || self.in_progress.is_some() || self.furthest.is_some()
    }

    /// The first chapter of `list` after the furthest read that is not read.
    pub fn next<'a>(&self, list: &'a [Kept]) -> Option<&'a Kept> {
        list.iter().find(|c| {
            !self.is_read(&c.id)
                && match (self.furthest, c.range()) {
                    (Some(furthest), Some(range)) => range > furthest,
                    _ => true,
                }
        })
    }

    /// The chapter that came out new for someone caught up: nothing in progress, and the first
    /// unread chapter is one a later answer listed. Someone still behind gets their place
    /// instead.
    pub fn new_for_you<'a>(&self, list: &'a [Kept]) -> Option<&'a Kept> {
        if !self.any() || self.in_progress.is_some() {
            return None;
        }
        self.unread(list).first().copied().filter(|c| c.seen_at.is_some())
    }

    /// The chapters of `list` past the furthest read and not read.
    pub fn unread<'a>(&self, list: &'a [Kept]) -> Vec<&'a Kept> {
        list.iter()
            .filter(|c| !self.is_read(&c.id))
            .filter(|c| match (self.furthest, c.range()) {
                (Some(furthest), Some(range)) => range > furthest,
                (Some(_), None) => false,
                (None, _) => true,
            })
            .collect()
    }
}

/// A chapter number kept in thousandths, as the watermark keeps it, back in words: "12", "10.5".
fn milli_label(milli: i64) -> String {
    if milli % 1000 == 0 {
        (milli / 1000).to_string()
    } else {
        format!("{}", milli as f64 / 1000.0)
    }
}

/// An edition's chapter-list ask that left no list kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Asked {
    Asking,
    Empty,
    Failed(kaeri_model::reading::ReadingFailure),
}

/// The list a work is read from: the edition in use, as kept.
pub fn of_work(
    sources: &crate::read_sources::BookSources,
    store: &Store,
    work: &kaeri_core::books::WorkId,
) -> Option<(String, String, Vec<Kept>)> {
    let (source, series) = sources.edition_of(work, None)?;
    let (list, _) = kept(store, &source, &series)?;
    Some((source, series, list))
}

/// An edition's line from its kept list: how many chapters, the newest, the language most of
/// them are in, and the groups, with the one its chapters are listed from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub chapters: u32,
    pub latest: Option<String>,
    pub language: Option<String>,
    /// The group the list is read from: the person's pick, else the one with the most
    /// chapters.
    pub version_label: Option<String>,
    pub groups: Vec<super::arrange::Group>,
    /// The person picked `version_label`.
    pub chosen: bool,
}

/// The summary of a source's kept list, with the person's group for the series, when picked.
pub fn summary(
    store: &Store,
    source: &str,
    series: &str,
    chosen: Option<&str>,
    app_language: Option<&str>,
) -> Summary {
    let Some((list, _)) = kept(store, source, series) else { return Summary::default() };
    let mut counts: HashMap<&String, usize> = HashMap::new();
    for c in &list {
        if let Some(v) = c.language.as_ref() {
            *counts.entry(v).or_default() += 1;
        }
    }
    let language = counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(v, _)| v.clone());
    let entries: Vec<SourceChapter> = list.iter().map(Kept::source_chapter).collect();
    let arranged = super::arrange::arrange(&entries, chosen, app_language);
    Summary {
        // One per chapter: its parts and its groups' releases count once.
        chapters: u32::try_from(arranged.rows.len()).unwrap_or(u32::MAX),
        latest: list.last().map(|c| c.number.clone().unwrap_or_else(|| c.label.clone())),
        language,
        version_label: arranged.preferred,
        groups: arranged.groups,
        chosen: arranged.chosen,
    }
}

/// The group the person picked for a work's chapters in `source`: the version the edition was
/// chosen in, by `ChooseGroup` or a chapter's Prefer this version.
pub fn chosen_group(store: &Store, work: &str, source: &str) -> Option<String> {
    store
        .edition_pref(work)
        .ok()
        .flatten()
        .filter(|p| p.source_id == source)
        .and_then(|p| p.version_label)
}

/// How a series comes out, learnt from the dates of its newest chapters: a weekday when most of
/// them came out on one day about a week apart, and the usual days between two chapters.
pub fn rhythm(list: &[Kept], zone: &jiff::tz::TimeZone, now: i64) -> Option<BookRhythm> {
    let mut dates: Vec<i64> = list.iter().filter_map(Kept::out_at).collect();
    dates.sort_unstable();
    dates.dedup_by(|a, b| (*a - *b).abs() < 6 * 3_600);
    let newest: Vec<i64> = dates.iter().rev().take(8).copied().collect();
    if newest.len() < 3 {
        return None;
    }
    let mut gaps: Vec<i64> = newest.windows(2).map(|w| (w[0] - w[1]) / 86_400).collect();
    gaps.sort_unstable();
    let every = gaps[gaps.len() / 2].max(1);
    let weekdays: Vec<u8> = newest.iter().filter_map(|at| weekday(*at, zone)).collect();
    let mut best = None;
    for day in 0..7u8 {
        let count = weekdays.iter().filter(|d| **d == day).count();
        if count * 3 >= weekdays.len() * 2 && best.is_none_or(|(_, c)| count > c) {
            best = Some((day, count));
        }
    }
    let weekday = best.map(|(day, _)| day).filter(|_| (6..=8).contains(&every));
    Some(BookRhythm {
        weekday,
        every_days: u32::try_from(every).ok(),
        last_published: newest.first().copied(),
        checked_at: now,
    })
}

/// The day of the week `at` falls on where the person is, from Monday as 0.
pub fn weekday(at: i64, zone: &jiff::tz::TimeZone) -> Option<u8> {
    let zoned = jiff::Timestamp::from_second(at).ok()?.to_zoned(zone.clone());
    u8::try_from(zoned.weekday().to_monday_zero_offset()).ok()
}

/// The start of the day `at` falls on where the person is.
pub fn day_start(at: i64, zone: &jiff::tz::TimeZone) -> Option<i64> {
    let zoned = jiff::Timestamp::from_second(at).ok()?.to_zoned(zone.clone());
    Some(zoned.start_of_day().ok()?.timestamp().as_second())
}

/// The words for a rhythm: "Usually Fridays", "About every 2 weeks".
pub fn rhythm_words(rhythm: &BookRhythm) -> Option<kaeri_model::Msg> {
    let weeks = rhythm.every_days.map_or(0, |d| (d + 3) / 7);
    kaeri_model::reading::rhythm(rhythm.weekday, weeks)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;
    /// A Friday at noon, UTC.
    const FRIDAY: i64 = 1_793_361_600;

    fn chapter(n: u32, published: Option<i64>) -> SourceChapter {
        let mut c = SourceChapter::new(format!("c-{n}"), n.to_string());
        c.published = published;
        c
    }

    #[test]
    fn a_later_answer_dates_what_is_new_and_the_first_dates_nothing() {
        let store = Store::open_in_memory().unwrap();
        let first: Vec<_> = (1..=3).map(|n| chapter(n, None)).collect();
        assert!(keep(&store, "shelf", "s", &first, 100).is_empty());
        let mut more = first.clone();
        more.push(chapter(4, None));
        let fresh = keep(&store, "shelf", "s", &more, 200);
        assert_eq!(fresh.len(), 1);
        assert_eq!((fresh[0].id.as_str(), fresh[0].seen_at), ("4", Some(200)));
        let (list, at) = kept(&store, "shelf", "s").unwrap();
        assert_eq!((list.len(), at, list[0].seen_at), (4, 200, None));
        // Asked again, chapter 4 keeps the date it was first seen.
        keep(&store, "shelf", "s", &more, 300);
        assert_eq!(kept(&store, "shelf", "s").unwrap().0[3].seen_at, Some(200));
    }

    #[test]
    fn a_title_that_only_names_the_chapter_again_is_not_its_own() {
        let titled = |label: &str, title: &str| Kept {
            id: "15".into(),
            source_ref: "15".into(),
            label: label.into(),
            number: Some("15".into()),
            volume: None,
            title: Some(title.into()),
            version_label: None,
            language: None,
            published: None,
            pages: None,
            seen_at: None,
            part: None,
        };
        assert_eq!(titled("Chapter 15", "Chapter 15").own_title(), None);
        assert_eq!(titled("15", "Chapter 15").own_title(), None);
        assert_eq!(titled("15", "Ch. 15").own_title(), None);
        assert_eq!(titled("15", "15").own_title(), None);
        assert_eq!(titled("15", "The Far Shore").own_title(), Some("The Far Shore"));
        assert_eq!(
            titled("15", "Chapter 15: The Far Shore").own_title(),
            Some("Chapter 15: The Far Shore")
        );
        assert_eq!(titled("15", "The 15th Lantern").own_title(), Some("The 15th Lantern"));
    }

    #[test]
    fn a_weekly_series_is_learnt_by_its_day_and_a_slow_one_by_its_span() {
        let zone = jiff::tz::TimeZone::UTC;
        assert_eq!(weekday(FRIDAY, &zone), Some(4));
        let weekly: Vec<Kept> = (0..5)
            .map(|n| Kept {
                id: n.to_string(),
                source_ref: n.to_string(),
                label: n.to_string(),
                number: None,
                volume: None,
                title: None,
                version_label: None,
                language: None,
                published: Some(FRIDAY - (4 - n) * 7 * DAY),
                pages: None,
                seen_at: None,
                part: None,
            })
            .collect();
        let found = rhythm(&weekly, &zone, FRIDAY).unwrap();
        assert_eq!((found.weekday, found.every_days), (Some(4), Some(7)));
        assert_eq!(rhythm_words(&found).unwrap().english(), "Usually Fridays");

        let slow: Vec<Kept> = weekly
            .iter()
            .enumerate()
            .map(|(n, k)| Kept { published: Some(FRIDAY - n as i64 * 15 * DAY), ..k.clone() })
            .collect();
        let found = rhythm(&slow, &zone, FRIDAY).unwrap();
        assert_eq!(found.weekday, None);
        assert_eq!(rhythm_words(&found).unwrap().english(), "About every 2 weeks");
        assert_eq!(rhythm(&weekly[..2], &zone, FRIDAY), None);
    }

    #[test]
    fn the_next_chapter_is_past_the_furthest_read() {
        let store = Store::open_in_memory().unwrap();
        store.adopt_read("a:900301", "12", None, 10).unwrap();
        let list: Vec<_> = (11..=15).map(|n| chapter(n, None)).collect();
        keep(&store, "shelf", "s", &list, 1);
        let (kept, _) = kept(&store, "shelf", "s").unwrap();
        let read = Read::of(&store, "a:900301");
        assert_eq!(read.next(&kept).map(|c| c.id.as_str()), Some("13"));
        assert_eq!(read.unread(&kept).len(), 3);
    }
}
