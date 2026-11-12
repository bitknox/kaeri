//! Each source's chapter list for a series, as it last answered, kept in `book_chapter_cache`:
//! what Updates, Continue's next chapter, versions and the edition counts read without asking
//! the source again. A chapter seen for the first time on a later answer is dated then, so a
//! source that dates nothing still shows what is new.

use std::collections::{HashMap, HashSet};

use kaeri_core::books::{ChapterId, ChapterRange, SourceChapter};
use kaeri_store::{
    Store,
    book_catalog::BookRhythm,
    reading::{ReadChapter, ReadWork},
};
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
///
/// A title that starts by naming a chapter ("Chapter 15: The Far Shore", "Ch. 15 - The Far
/// Shore") keeps only its words, as the row names the chapter already; and a number after a dash
/// at its end ("The Far Shore - 15", any dash) goes when it only repeats that chapter's number or the
/// row's. The row's number is the source's, which every place Kaeri names the chapter uses, so a
/// title whose own number differs from it (a source that counts its extras as chapters) does not
/// put a second number beside it. The row's number in brackets at the end, "The Far Shore (15)",
/// goes too.
///
/// The novel reader gives a text chapter's opening heading this too (`kaeri_books::novel::
/// heading_once`), so the chapter list, the series page and the page itself say the same.
pub fn own_title<'a>(
    title: Option<&'a str>,
    label: &str,
    number: Option<&str>,
) -> Option<&'a str> {
    let title = title?.trim();
    let title = number.map_or(title, |n| without_bracketed(title, n));
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
    let (named, rest) = match chapter_named(title) {
        Some((named, rest)) => (Some(named), rest),
        None => (None, title),
    };
    let repeats = |n: &str| {
        let n = same_number(n);
        n.is_some()
            && (named.and_then(same_number) == n
                || named.and_then(whole_part).and_then(same_number) == n
                || number.and_then(same_number) == n
                || same_number(label) == n)
    };
    let rest = match number_after_dash(rest) {
        Some((words, n)) if repeats(n) => words,
        // Only the number again, "Chapter 15 - 15": nothing of its own.
        None if named.is_some() && repeats(rest) => "",
        _ => rest,
    };
    (!rest.is_empty()).then_some(rest)
}

/// The words that name a chapter at the start of a title, longest first.
const CHAPTER_WORDS: [&str; 5] = ["chapter", "chap.", "chap", "ch.", "ch"];

/// A title that starts by naming a chapter, "Chapter 15: The Far Shore": the number, and the
/// words after it with the separator gone ("" when there are none).
fn chapter_named(title: &str) -> Option<(&str, &str)> {
    let lower = title.to_ascii_lowercase();
    let word = CHAPTER_WORDS.iter().find(|w| lower.starts_with(**w))?;
    let after = title[word.len()..].trim_start_matches([' ', '.']);
    let end = after.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(after.len());
    let number = after[..end].trim_end_matches('.');
    if !number.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let rest = &after[number.len()..];
    // The number ends the name: "Chapter 15th Night" is not chapter 15.
    if rest.starts_with(char::is_alphanumeric) {
        return None;
    }
    let rest = rest.trim_start_matches(|c: char| {
        c.is_whitespace() || matches!(c, ':' | '-' | '\u{2013}' | '\u{2014}' | '.' | ',' | '|')
    });
    Some((number, rest.trim_end()))
}

/// Words ending in a dash and a number, "The Far Shore - 15": the words and the number.
fn number_after_dash(title: &str) -> Option<(&str, &str)> {
    let at = title.rfind(['-', '\u{2013}', '\u{2014}'])?;
    let dash = title[at..].chars().next()?.len_utf8();
    let (words, n) = (&title[..at], title[at + dash..].trim());
    // A dash inside a word ("Re-15") is not a separator; one at the start is.
    if !(words.is_empty() || words.ends_with(char::is_whitespace)) || n.is_empty() {
        return None;
    }
    n.chars().all(|c| c.is_ascii_digit() || c == '.').then(|| (words.trim_end(), n))
}

/// A chapter number as a value to compare, so "05" and "5" are one number.
fn same_number(text: &str) -> Option<u64> {
    let value: f64 = text.trim().parse().ok()?;
    (value.is_finite() && value >= 0.0).then(|| (value * 1000.0).round() as u64)
}

/// The whole chapter of a part's number: "196" of "196.1".
fn whole_part(number: &str) -> Option<&str> {
    number.split_once('.').map(|(whole, _)| whole)
}

/// `title` without `number` in brackets at its end: "The Far Shore (15)" in chapter 15 is "The
/// Far Shore". Anything else comes back as it is; a number after a dash is [`own_title`]'s.
fn without_bracketed<'a>(title: &'a str, number: &str) -> &'a str {
    let number = number.trim();
    if number.is_empty() {
        return title;
    }
    let Some(inner) = title.strip_suffix(')').or_else(|| title.strip_suffix(']')) else {
        return title;
    };
    let Some(before) = inner.trim_end().strip_suffix(number) else { return title };
    if before.ends_with(|c: char| c.is_ascii_digit() || c == '.') {
        return title;
    }
    let rest = before.trim_end();
    match rest.strip_suffix('(').or_else(|| rest.strip_suffix('[')).map(str::trim_end) {
        Some(rest) if rest.chars().any(char::is_alphanumeric) => rest,
        _ => title,
    }
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

/// The work a source's series is matched to.
fn work_of(store: &Store, source: &str, series: &str) -> Option<String> {
    store.book_match_state(source, series).ok().flatten().map(|(work, _)| work)
}

/// Every kept list of a work, with its source and series, oldest chapter first.
pub fn lists_of(store: &Store, work: &str) -> Vec<(String, String, Vec<Kept>)> {
    store
        .book_refs_for_work(work)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(source, series)| {
            let (list, _) = kept(store, &source, &series)?;
            Some((source, series, list))
        })
        .collect()
}

/// What a work's kept lists say of its numbering, but for the list of `leave_out`.
pub fn numbering(
    store: &Store,
    work: &str,
    leave_out: Option<(&str, &str)>,
) -> super::arrange::Numbering {
    let mut numbering = super::arrange::Numbering::default();
    for (source, series, list) in lists_of(store, work) {
        if leave_out == Some((source.as_str(), series.as_str())) {
            continue;
        }
        let entries: Vec<SourceChapter> = list.iter().map(Kept::source_chapter).collect();
        numbering.add(&entries);
    }
    numbering
}

/// Keep a source's answer, and give the chapters it lists that no answer listed before. The
/// first answer for a series gives none: everything in it was out before.
///
/// An empty answer is no answer: a server part way through a scan or a scrape that broke says
/// nothing of what came out. It is not kept, so the next full answer is not all new; and a
/// list kept empty before counts as none. A short answer (a later page of the source's list
/// that failed) is kept as the list, and the chapters it left out are remembered: back in a
/// later answer, they are known, not new.
///
/// Each entry keeps the date it was first seen by the source's own id for it, so a part that
/// joins a chapter ("7.2" beside a "7.1" listed before) leaves the chapter known: it is not
/// new again. Entries are arranged with what the work's other lists say ([`numbering`]), and
/// those lists are arranged again with this one, so every list of a work keys a chapter alike.
/// A downloaded chapter that has gained a part since is fetched again, and one whose entry now
/// reads as part of a whole chapter has that chapter queued in its place (see
/// [`follow_downloads`]).
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
    // By the source's own id and by chapter, so a long list is looked through once rather than
    // once a chapter.
    let by_ref: Option<HashMap<&str, &Kept>> =
        before.as_ref().map(|list| list.iter().map(|k| (k.source_ref.as_str(), k)).collect());
    let by_id: Option<HashMap<&str, &Kept>> =
        before.as_ref().map(|list| list.iter().map(|k| (k.id.as_str(), k)).collect());
    let key = left_key(source, series);
    let left: HashMap<String, Option<i64>> = store
        .get_meta_text(&key)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let work = work_of(store, source, series);
    let others = work
        .as_deref()
        .map(|w| numbering(store, w, Some((source, series))))
        .unwrap_or_default();
    // Parts are kept under their whole chapter, so what is new is a chapter, not a part.
    let arranged = super::arrange::arrange_in(chapters, None, None, &others);
    // Each entry with when an earlier answer first listed it, if one did.
    let mut list: Vec<Kept> = Vec::with_capacity(chapters.len());
    let mut earliers: Vec<Option<Option<Option<i64>>>> = Vec::with_capacity(chapters.len());
    for (i, c) in chapters.iter().enumerate() {
        let id = arranged.ids[i].as_str().to_owned();
        let earlier = by_ref.as_ref().map(|b| match b.get(c.id.as_str()) {
            Some(k) => Some(k.seen_at),
            None => match by_id.as_ref().and_then(|b| b.get(id.as_str())) {
                Some(k) => Some(k.seen_at),
                None => left.get(&id).copied(),
            },
        });
        let seen_at = match earlier {
            None => None,
            Some(Some(seen_at)) => seen_at,
            Some(None) => Some(now),
        };
        list.push(Kept {
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
        });
        earliers.push(earlier);
    }
    // A chapter one of whose entries was listed before is not new, whatever joined it.
    let known: HashSet<&str> = list
        .iter()
        .zip(&earliers)
        .filter(|(_, e)| !matches!(e, Some(None)))
        .map(|(k, _)| k.id.as_str())
        .collect();
    let mut fresh: Vec<Kept> = Vec::new();
    let mut fresh_ids = HashSet::new();
    for (kept, earlier) in list.iter().zip(&earliers) {
        // Two versions of a new chapter are one chapter come out.
        if matches!(earlier, Some(None))
            && !known.contains(kept.id.as_str())
            && fresh_ids.insert(kept.id.clone())
        {
            fresh.push(kept.clone());
        }
    }
    let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
    if let Err(e) = store.put_book_chapters(source, series, &json, now) {
        tracing::warn!(error = %e, "a chapter list was not kept");
    }
    if let Some(work) = work.as_deref() {
        if let Some(before) = before.as_deref() {
            follow_downloads(store, work, source, before, &list, now);
        }
        rearrange_others(store, work, (source, series));
    }
    // What this answer left out of what was listed before, or left out already.
    let listed: HashSet<&str> = list.iter().map(|k| k.id.as_str()).collect();
    let mut still_left: HashMap<String, Option<i64>> = left
        .iter()
        .filter(|(id, _)| !listed.contains(id.as_str()))
        .map(|(id, seen)| (id.clone(), *seen))
        .collect();
    for (id, k) in by_id.iter().flatten() {
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

/// Arrange the work's other kept lists again with `this` one's numbers, and keep those whose
/// chapters now read otherwise: a lone "7.1" kept before another source named a whole 7 is
/// part of 7 from here. Their dates stay as they were.
fn rearrange_others(store: &Store, work: &str, this: (&str, &str)) {
    let lists = lists_of(store, work);
    if lists.len() < 2 {
        return;
    }
    for (source, series, list) in &lists {
        if (source.as_str(), series.as_str()) == this {
            continue;
        }
        let others = numbering(store, work, Some((source, series)));
        let entries: Vec<SourceChapter> = list.iter().map(Kept::source_chapter).collect();
        let arranged = super::arrange::arrange_in(&entries, None, None, &others);
        let moved = list
            .iter()
            .enumerate()
            .any(|(i, k)| k.id != arranged.ids[i].as_str() || k.part != arranged.parts[i]);
        if !moved {
            continue;
        }
        let list: Vec<Kept> = list
            .iter()
            .enumerate()
            .map(|(i, k)| Kept {
                id: arranged.ids[i].as_str().to_owned(),
                part: arranged.parts[i],
                ..k.clone()
            })
            .collect();
        let Ok(Some((_, at))) = store.book_chapters(source, series) else { continue };
        let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
        if let Err(e) = store.put_book_chapters(source, series, &json, at) {
            tracing::warn!(error = %e, "a chapter list arranged again was not kept");
        }
    }
}

/// Downloads that a new answer changed. A finished download of a chapter whose release has
/// gained a part since holds only the parts it had, so it is fetched again whole. One whose
/// entry now reads as part of a whole chapter ("7.1" joined by "7.2") stays where it is, and
/// the whole chapter is queued as that download was; the old one lists under the whole
/// chapter's read marks (see [`Folds`]) and goes with them.
fn follow_downloads(
    store: &Store,
    work: &str,
    source: &str,
    before: &[Kept],
    now: &[Kept],
    at: i64,
) {
    use kaeri_store::book_downloads::{BookDownloadAsk, BookDownloadState};
    let downloads: HashMap<String, kaeri_store::book_downloads::BookDownload> = store
        .book_downloads(Some(work))
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.state == BookDownloadState::Done && d.source_id == source)
        .map(|d| (d.chapter.clone(), d))
        .collect();
    if downloads.is_empty() {
        return;
    }
    // Each chapter's entries in one group, by the source's own ids.
    let release = |list: &[Kept], id: &str, group: Option<&str>| -> HashSet<String> {
        list.iter()
            .filter(|k| k.id == id && k.version_label.as_deref() == group)
            .map(|k| k.source_ref.clone())
            .collect()
    };
    let was: HashMap<&str, &str> =
        before.iter().map(|k| (k.source_ref.as_str(), k.id.as_str())).collect();
    for (chapter, d) in &downloads {
        let group = d.version_label.as_deref();
        let then = release(before, chapter, group);
        let gained = release(now, chapter, group);
        if !then.is_empty() && gained.len() > then.len() && gained.is_superset(&then) {
            if let Err(e) = store.book_download_refetch(work, chapter, at) {
                tracing::warn!(error = %e, "a download that gained a part was not queued again");
            }
            continue;
        }
        // The entry this download was of now belongs to another chapter.
        let moved = now.iter().find(|k| {
            then.contains(&k.source_ref)
                && was.get(k.source_ref.as_str()) != Some(&k.id.as_str())
        });
        if let Some(moved) = moved
            && !downloads.contains_key(&moved.id)
        {
            let ask = BookDownloadAsk {
                work,
                chapter: &moved.id,
                source_id: source,
                version_label: group,
                keep_ready: d.keep_ready,
            };
            if let Err(e) = store.queue_book_download(&ask, at) {
                tracing::warn!(error = %e, "a chapter that took in a download was not queued");
            }
        }
    }
}

/// One release of a chapter in parts: each part's own key, with its pages when listed.
type Release = Vec<(String, Option<u32>)>;

/// Read marks kept on a chapter's parts under their own numbers, folded into the chapter at
/// read time. A lone "7.1" read before "7.2" came out keeps its marks on "7.1"; once the two
/// read as chapter 7, those marks count for 7 rather than being lost, and nothing kept is
/// rewritten:
/// - a whole chapter counts as read when every part of one release of it is read, and each part
///   counts as read when the whole chapter is;
/// - a whole chapter with only its first parts read is in progress, at the start of the first
///   part not read (at its own kept page when it has one), counted in pages where the list
///   gives them.
#[derive(Debug, Clone, Default)]
pub struct Folds {
    /// Each part's own key ("7.1"), with its whole chapter's id.
    up: HashMap<String, String>,
    /// Each whole chapter's releases in parts: their parts' own keys in order, with pages.
    releases: HashMap<String, Vec<Release>>,
}

impl Folds {
    /// From every kept list of a work.
    pub fn of_work(store: &Store, work: &str) -> Self {
        let lists: Vec<Vec<Kept>> = lists_of(store, work).into_iter().map(|l| l.2).collect();
        Self::of(lists.iter().map(Vec::as_slice))
    }

    pub fn of<'a>(lists: impl IntoIterator<Item = &'a [Kept]>) -> Self {
        let mut folds = Self::default();
        for list in lists {
            // Each release, by chapter and group, in part order.
            type Parts = Vec<(u32, String, Option<u32>)>;
            let mut releases: HashMap<(&str, Option<&str>), Parts> = HashMap::new();
            for k in list {
                let Some(part) = k.part else { continue };
                // Only a numbered chapter's parts carry a number of their own.
                if k.id.contains('/') {
                    continue;
                }
                let Some(own) = k.number.as_deref().and_then(ChapterRange::parse) else {
                    continue;
                };
                let own = kaeri_core::books::ChapterKey::numbered(own).as_str().to_owned();
                if own != k.id {
                    folds.up.insert(own.clone(), k.id.clone());
                }
                releases
                    .entry((k.id.as_str(), k.version_label.as_deref()))
                    .or_default()
                    .push((part, own, k.pages));
            }
            for ((id, _), mut parts) in releases {
                parts.sort_by_key(|(n, _, _)| *n);
                let parts: Vec<(String, Option<u32>)> =
                    parts.into_iter().map(|(_, key, pages)| (key, pages)).collect();
                let held = folds.releases.entry(id.to_owned()).or_default();
                if !held.contains(&parts) {
                    held.push(parts);
                }
            }
        }
        folds
    }

    /// Whether a key is a part's own, kept under its whole chapter's.
    pub fn is_part(&self, key: &str) -> bool {
        self.up.contains_key(key)
    }

    /// The whole chapter a part's own key is kept under.
    pub fn whole_of(&self, key: &str) -> Option<String> {
        self.up.get(key).cloned()
    }

    /// The chapters to count as read beside those `is_read` says: whole chapters whose parts
    /// are all read, and the parts of whole chapters read.
    pub fn read(&self, is_read: impl Fn(&str) -> bool) -> Vec<String> {
        let mut more = Vec::new();
        for (whole, releases) in &self.releases {
            if is_read(whole) {
                for (part, _) in releases.iter().flatten() {
                    if part != whole && !is_read(part) {
                        more.push(part.clone());
                    }
                }
            } else if releases.iter().any(|r| r.iter().all(|(part, _)| is_read(part))) {
                more.push(whole.clone());
            }
        }
        more
    }

    /// The store's marks with the folds counted.
    pub fn marks(
        &self,
        mut marks: kaeri_store::reading::ReadMarks,
    ) -> kaeri_store::reading::ReadMarks {
        for chapter in self.read(|c| marks.is_read(c)) {
            marks.count_read(chapter);
        }
        marks
    }

    /// The kept places with each whole chapter read in part under its parts' own numbers
    /// given a place of its own, unless it has one.
    pub fn places(
        &self,
        mut places: Vec<ReadChapter>,
        marks: &kaeri_store::reading::ReadMarks,
    ) -> Vec<ReadChapter> {
        for (whole, releases) in &self.releases {
            if marks.is_read(whole) || places.iter().any(|p| p.chapter == *whole) {
                continue;
            }
            let found = releases.iter().find_map(|parts| {
                let at = parts.iter().position(|(part, _)| !marks.is_read(part))?;
                let own = places.iter().find(|p| p.chapter == parts[at].0 && !p.read);
                if at == 0 && own.is_none_or(|p| p.last_page.is_none()) {
                    return None;
                }
                // A part's pages as its source lists them, else as the reader counted them.
                let pages_of = |(part, pages): &(String, Option<u32>)| {
                    pages.or_else(|| {
                        places.iter().find(|p| p.chapter == *part).and_then(|p| p.of_pages)
                    })
                };
                let before: Option<u32> = parts[..at].iter().map(pages_of).sum();
                let page = before.unwrap_or(0) + own.and_then(|p| p.last_page).unwrap_or(0);
                let of: Option<u32> = parts.iter().map(pages_of).sum();
                let stamp = places
                    .iter()
                    .filter(|p| parts.iter().any(|(part, _)| *part == p.chapter))
                    .map(|p| p.stamp)
                    .max()
                    .unwrap_or(0);
                Some(ReadChapter {
                    chapter: whole.clone(),
                    read: false,
                    read_at: None,
                    last_page: Some(page),
                    of_pages: of,
                    strip_image: None,
                    strip_offset: None,
                    novel_pos: None,
                    stamp,
                    device: None,
                })
            });
            places.extend(found);
        }
        places
    }
}

/// A work's read marks and kept places, with the folds of [`Folds`] counted: what every list of
/// chapters reads them from.
pub fn read_state(
    store: &Store,
    work: &str,
) -> (kaeri_store::reading::ReadMarks, Vec<ReadChapter>) {
    let folds = Folds::of_work(store, work);
    let marks = folds.marks(store.read_marks(work).unwrap_or_default());
    let places = folds.places(store.read_chapters(work).unwrap_or_default(), &marks);
    (marks, places)
}

/// A work's read marks with the folds of [`Folds`] counted.
pub fn read_marks(store: &Store, work: &str) -> kaeri_store::reading::ReadMarks {
    Folds::of_work(store, work).marks(store.read_marks(work).unwrap_or_default())
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
        let folds = Folds::of_work(store, work);
        let marks = folds.marks(store.read_marks(work).unwrap_or_default());
        let chapters = folds.places(store.read_chapters(work).unwrap_or_default(), &marks);
        let work_row = store.read_work(work).ok().flatten();
        let mut marked: HashSet<String> =
            chapters.iter().filter(|c| c.read).map(|c| c.chapter.clone()).collect();
        let adopted = |c: &str| work_row.as_ref().is_some_and(|w| w.adopted(c));
        let more = folds.read(|c| marked.contains(c) || adopted(c));
        marked.extend(more);
        let last = work_row.as_ref().and_then(|w| w.last_chapter.clone());
        // A part read under its own number points at its whole chapter's place.
        let last = last.map(|l| folds.whole_of(&l).unwrap_or(l));
        let range = |id: &str| ChapterId::new(id).key().and_then(|k| k.range());
        // A part read alone says nothing of how far the chapter it belongs to was read.
        let mut furthest =
            marked.iter().filter(|id| !folds.is_part(id)).filter_map(|id| range(id)).max();
        if let Some(milli) = work_row.as_ref().and_then(|w| w.adopt_milli)
            && let Some(adopted) = ChapterRange::parse(&milli_label(milli))
        {
            furthest = furthest.max(Some(adopted));
        }
        let behind = |id: &str| furthest.is_some_and(|f| range(id).is_some_and(|r| r <= f));
        let extra = |id: &str| range(id).is_some_and(|r| r.end.is_none() && r.start.is_split());
        let in_progress = chapters
            .iter()
            .filter(|c| !c.read && !marked.contains(&c.chapter))
            .filter(|c| c.last_page.is_some() || c.novel_pos.is_some())
            .filter(|c| !folds.is_part(&c.chapter))
            // A chapter left part way and passed over: once a later chapter is read, Continue
            // goes on from there, and the chapter stays unread in the list. A whole chapter keeps
            // its place while it is the last one opened, as when the person goes back to it; an
            // extra is passed over even then.
            .filter(|c| {
                !behind(&c.chapter) || (!extra(&c.chapter) && Some(&c.chapter) == last.as_ref())
            })
            .max_by_key(|c| (Some(&c.chapter) == last.as_ref(), c.stamp))
            .map(|c| (c.chapter.clone(), c.last_page, c.of_pages));
        Self { marked, work: work_row, in_progress, furthest }
    }

    pub fn is_read(&self, id: &str) -> bool {
        self.marked.contains(id) || self.work.as_ref().is_some_and(|w| w.adopted(id))
    }

    pub fn any(&self) -> bool {
        !self.marked.is_empty() || self.in_progress.is_some() || self.furthest.is_some()
    }

    /// The first chapter of `list` after the furthest read that is not read. Extras behind the
    /// furthest read, the person passed over: a decimal one by its number, one with no number
    /// when a later volume has been read.
    pub fn next<'a>(&self, list: &'a [Kept]) -> Option<&'a Kept> {
        list.iter().find(|c| {
            !self.is_read(&c.id)
                && match (self.furthest, c.range()) {
                    (Some(furthest), Some(range)) => range > furthest,
                    (Some(furthest), None) => !self.passed_volume(c, list, furthest),
                    (None, _) => true,
                }
        })
    }

    /// Whether `extra`, a chapter with no number in a volume, sits in a volume before one the
    /// person has read a chapter of.
    fn passed_volume(&self, extra: &Kept, list: &[Kept], furthest: ChapterRange) -> bool {
        let Some(its) = extra.volume.as_deref().and_then(volume_number) else { return false };
        list.iter().any(|c| {
            c.range().is_some_and(|r| r <= furthest)
                && c.volume.as_deref().and_then(volume_number).is_some_and(|v| v > its)
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

/// A volume's number as a source names it: "2", "Vol. 2", "Volume 2.5".
fn volume_number(volume: &str) -> Option<kaeri_core::books::ChapterNumber> {
    let digits = volume.trim_start_matches(|c: char| !c.is_ascii_digit());
    let end = digits.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(digits.len());
    kaeri_core::books::ChapterNumber::parse(digits[..end].trim_end_matches('.'))
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
    let numbering = work_of(store, source, series)
        .map(|work| numbering(store, &work, None))
        .unwrap_or_default();
    let arranged = super::arrange::arrange_in(&entries, chosen, app_language, &numbering);
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
        assert_eq!(titled("15", "The 15th Lantern").own_title(), Some("The 15th Lantern"));
        // The number said again at the end goes; another number stays.
        assert_eq!(titled("15", "The Far Shore — 15").own_title(), Some("The Far Shore"));
        assert_eq!(titled("15", "The Far Shore - 15").own_title(), Some("The Far Shore"));
        assert_eq!(titled("15", "The Far Shore (15)").own_title(), Some("The Far Shore"));
        assert_eq!(
            titled("15", "Chapter 15: The Far Shore — 15").own_title(),
            Some("The Far Shore")
        );
        assert_eq!(titled("15", "Chapter 15 — 15").own_title(), None);
        assert_eq!(
            titled("15", "The Far Shore — 115").own_title(),
            Some("The Far Shore — 115")
        );
        assert_eq!(titled("15", "The Far Shore — 2").own_title(), Some("The Far Shore — 2"));
        assert_eq!(titled("15", "Vol. 1-15").own_title(), Some("Vol. 1-15"));
        assert_eq!(titled("15", "— 15").own_title(), None);
    }

    #[test]
    fn a_title_keeps_its_words_without_the_chapter_it_names_again() {
        let own = |number: &str, title: &str| {
            own_title(Some(title), number, Some(number)).map(str::to_owned)
        };
        let far = Some("The Far Shore".to_owned());
        assert_eq!(own("15", "Chapter 15: The Far Shore"), far);
        assert_eq!(own("15", "Ch. 15 - The Far Shore"), far);
        assert_eq!(own("15", "ch.15 \u{2013} The Far Shore"), far);
        assert_eq!(own("15", "Chapter 15 The Far Shore"), far);
        assert_eq!(own("15", "The Far Shore \u{2014} 15"), far);
        assert_eq!(own("15", "Chapter 15: The Far Shore \u{2014} 15"), far);
        // A source that counts its extras among its chapters runs ahead of the titles' own
        // numbers: the row's number is the source's, and the title keeps only its words.
        assert_eq!(own("19", "Chapter 15: The Far Shore \u{2014} 15"), far);
        assert_eq!(
            own("16", "Chapter 12.1: Lanterns \u{2014} 12"),
            Some("Lanterns".to_owned())
        );
        assert_eq!(
            own("17", "Chapter 12.2: A Ferryman Alone"),
            Some("A Ferryman Alone".to_owned())
        );
        assert_eq!(own("15", "Chapter 15"), None);
        assert_eq!(own("15", "Ch. 15."), None);
        assert_eq!(own("15", "Chapter 15 \u{2014} 15"), None);
        // Numbers that belong to the title stay.
        for kept in [
            "Chapter 15th Night",
            "Chapterhouse 3",
            "Changes in 15",
            "The Far Shore \u{2014} 2",
            "Countdown - 3",
            "Re-15 and After",
        ] {
            assert_eq!(own("15", kept).as_deref(), Some(kept));
        }
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

    fn read(store: &Store, work: &str, chapter: &str, page: Option<u32>, at: i64) {
        let mut event = kaeri_store::reading::ReadEvent::new(work, chapter, at);
        event.completed = page.is_none();
        event.page = page;
        event.of_pages = Some(20);
        store.record_read(&event, Vec::new()).unwrap();
    }

    #[test]
    fn an_extra_skipped_for_the_next_whole_chapter_stays_unread_and_is_never_next_again() {
        const WORK: &str = "a:900302";
        let store = Store::open_in_memory().unwrap();
        let list: Vec<_> = ["15", "16", "16.5", "17", "18"]
            .into_iter()
            .map(|n| SourceChapter::new(format!("c-{n}"), n))
            .collect();
        keep(&store, "shelf", "s", &list, 1);
        let (kept, _) = kept(&store, "shelf", "s").unwrap();
        let next = |store: &Store| {
            let read = Read::of(store, WORK);
            let at = read.in_progress.as_ref().map(|(id, _, _)| id.clone());
            at.or_else(|| read.next(&kept).map(|c| c.id.clone()))
        };
        read(&store, WORK, "15", None, 10);
        read(&store, WORK, "16", None, 20);
        assert_eq!(next(&store).as_deref(), Some("16.5"), "the extra is next until skipped");
        // A look into the extra, then the skip to 17: 17 is where the person is.
        read(&store, WORK, "16.5", Some(2), 30);
        read(&store, WORK, "17", Some(4), 40);
        assert_eq!(next(&store).as_deref(), Some("17"));
        // Once 17 is read, 18 is next, never 16.5 again, and 16.5 stays unread.
        read(&store, WORK, "17", None, 50);
        assert_eq!(next(&store).as_deref(), Some("18"));
        let after = Read::of(&store, WORK);
        assert!(!after.is_read("16.5"));
        // Read later by choice, it counts as any chapter does.
        read(&store, WORK, "16.5", None, 60);
        assert!(Read::of(&store, WORK).is_read("16.5"));
        assert_eq!(next(&store).as_deref(), Some("18"));
    }

    #[test]
    fn a_chapter_left_part_way_is_passed_over_once_later_chapters_are_read() {
        const WORK: &str = "a:900304";
        let store = Store::open_in_memory().unwrap();
        let list: Vec<_> = ["1", "2", "3", "3.5", "4", "4.5", "5"]
            .into_iter()
            .map(|n| SourceChapter::new(format!("c-{n}"), n))
            .collect();
        keep(&store, "shelf", "s", &list, 1);
        let (kept, _) = kept(&store, "shelf", "s").unwrap();
        let next = |store: &Store| {
            let read = Read::of(store, WORK);
            let at = read.in_progress.as_ref().map(|(id, page, _)| (id.clone(), *page));
            at.or_else(|| read.next(&kept).map(|c| (c.id.clone(), None)))
        };
        read(&store, WORK, "1", Some(20), 10);
        assert_eq!(next(&store), Some(("1".into(), Some(20))));
        read(&store, WORK, "2", None, 20);
        read(&store, WORK, "3", None, 30);
        read(&store, WORK, "4", Some(2), 40);
        assert_eq!(next(&store), Some(("4".into(), Some(2))), "the place in 4 wins");
        // Once 4 is read to its end, Continue goes on past it, not back to page 20 of 1.
        read(&store, WORK, "4", None, 50);
        assert_eq!(next(&store), Some(("4.5".into(), None)));
        assert!(!Read::of(&store, WORK).is_read("1"), "chapter 1 stays unread");
        // Going back to chapter 1 by choice keeps its place while it is the last one opened.
        read(&store, WORK, "1", Some(21), 60);
        assert_eq!(next(&store), Some(("1".into(), Some(21))));
    }

    #[test]
    fn an_extra_with_no_number_is_passed_over_once_a_later_volume_is_read() {
        const WORK: &str = "a:900303";
        let store = Store::open_in_memory().unwrap();
        let entry = |id: &str, label: &str, volume: &str| {
            let mut c = SourceChapter::new(id, label);
            c.volume = Some(volume.into());
            c
        };
        let list = vec![
            entry("c-1", "1", "1"),
            entry("c-2", "2", "1"),
            entry("c-x", "Volume Extras", "Vol. 1"),
            entry("c-3", "3", "2"),
            entry("c-4", "4", "2"),
            entry("c-y", "Afterword", "2"),
        ];
        keep(&store, "shelf", "s", &list, 1);
        let (kept, _) = kept(&store, "shelf", "s").unwrap();
        let next = |store: &Store| Read::of(store, WORK).next(&kept).map(|c| c.id.clone());
        read(&store, WORK, "1", None, 10);
        read(&store, WORK, "2", None, 20);
        assert_eq!(next(&store).as_deref(), Some("3"));
        read(&store, WORK, "3", None, 30);
        read(&store, WORK, "4", None, 40);
        // Volume 1's extras sit behind volume 2, read; volume 2's afterword is still ahead.
        assert_eq!(next(&store).as_deref(), Some("x/2/afterword"));
        assert!(!Read::of(&store, WORK).is_read("x/1/volume extras"));
    }
}
