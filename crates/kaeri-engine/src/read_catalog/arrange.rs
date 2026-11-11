//! A source's chapter list arranged as the person reads it: parts of one chapter merged into
//! one, and one row per chapter from one group.
//!
//! **Parts.** Within one group, entries that are parts of one chapter become one chapter with
//! the parts in order: "Chapter 1 part 1" and "Chapter 1 part 2", "1-1" and "1-2", "1.1" and
//! "1.2". What says so, strongest first:
//! - "part" or "pt" with a number in the label, or in the titles of entries that share one
//!   number (the WIT `chapter` has no part field of its own, so a source says it in words),
//!   with the parts in all when it says ("part 1/3", "part 1 of 3"). A title's part word on a
//!   number of its own names a story told over several numbered chapters, each a chapter. A
//!   volume's parts ("Volume 2 Part 3", a light novel's) stay as they are;
//! - `a-b` labels where the group lists `a-1` and at least one more of `a`'s parts (a lone
//!   `12-13` is a run of two chapters, as [`ChapterRange::parse`] reads it);
//! - decimals the series uses as parts: `n.1`, `n.2`, ... from .1 with no gap and no whole `n`
//!   in the group. Once a group numbers one chapter that way, a lone `n.1` with no `n` is a
//!   first part too;
//! - a whole `n` with `n.2`, `n.3`, ... from .2 with no gap in one group: `n` is the first part
//!   and the decimals the rest ("57", "57.2", "57.3").
//!
//! A decimal beside its whole number otherwise (`10` and `10.5`, or `10` and `10.1`), or one
//! that does not start at .1 (`10.5` alone), is an extra chapter, not a part.
//!
//! **The work's other lists.** A group's own entries do not always say enough: a group that has
//! released only `7.1` so far looks like an extra. What the work's other sources and groups list
//! ([`Numbering`]) settles it, so every source of one work reads the same content as one
//! chapter and its read marks are shared. A decimal run from .1 with no whole `n` in its group
//! is a part of `n` when another list names `n` whole, or when the tenths every list names for
//! `n` together run from .1 with at least two (one group has `7.1`, another `7.2`). It stays an
//! extra when another list names the same decimal beside a whole `n` of its own. Read marks
//! kept on the decimal before it merged count for the whole chapter at read time
//! ([`super::chapters::Folds`]).
//!
//! A source that renumbers parts as whole chapters (its 7, 8, 9 being another's 7.1, 7.2, 7.3)
//! is not detected: nothing in the numbers says so.
//!
//! A chapter whose parts have a gap ("part 1" and "part 3"), or fewer than the source said,
//! still merges, with the missing parts named.
//!
//! **Groups.** Several groups' releases of one chapter show as one row, from the preferred
//! group: the person's pick for the series, else the group in the app's language with the most
//! chapters, else the group with the most chapters. A chapter that group lacks comes from the
//! next group in that order, marked as such; a part it lacks comes from the next group that has
//! that part.

use std::collections::{BTreeSet, HashMap};

use kaeri_core::books::{ChapterId, ChapterKey, ChapterNumber, ChapterRange, SourceChapter};

/// One chapter as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: ChapterId,
    /// The entries it is read from, as indices into the list, in reading order: one, or its
    /// parts.
    pub parts: Vec<usize>,
    /// The group it is listed from.
    pub group: Option<String>,
    /// Not from the preferred group, which lacks it.
    pub other_group: bool,
    /// For a chapter in parts: each entry's part number, beside `parts`.
    pub numbers: Vec<u32>,
    /// For a chapter in parts: how many parts it has in all, as far as is known.
    pub total: u32,
    /// Parts no group lists.
    pub missing: Vec<u32>,
    /// Parts taken from another group than the row's, with that group.
    pub filled: Vec<(u32, String)>,
}

/// A group with how many chapters it has in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub label: String,
    pub chapters: u32,
    /// The language most of its chapters are in.
    pub language: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Arranged {
    /// Each entry's chapter: a part's is its whole chapter's.
    pub ids: Vec<ChapterId>,
    /// Each entry's part number, from 1, for an entry that is a part.
    pub parts: Vec<Option<u32>>,
    /// One per chapter, in the order the list first names them.
    pub rows: Vec<Row>,
    /// The named groups, the most chapters first.
    pub groups: Vec<Group>,
    /// The group rows come from.
    pub preferred: Option<String>,
    /// The person picked it.
    pub chosen: bool,
    /// Each row's place in `rows`, by its id.
    at: HashMap<ChapterId, usize>,
}

impl Arranged {
    pub fn row(&self, id: &ChapterId) -> Option<&Row> {
        self.at.get(id).and_then(|&i| self.rows.get(i))
    }

    /// A group's language, as [`Group::language`] gives it.
    pub fn language_of(&self, group: &str) -> Option<&str> {
        self.groups.iter().find(|g| g.label == group).and_then(|g| g.language.as_deref())
    }

    /// A group's language when it is not the app's, for the line that names the group.
    fn foreign(&self, group: &str, app: Option<&str>) -> Option<String> {
        let its = self.language_of(group)?;
        let app = app?;
        (!same_language(app, its)).then(|| its.to_owned())
    }

    /// Set what a chapter row says of its parts and its group from `row`: the parts in all and
    /// those missing, parts from another group, and the group it comes from.
    pub fn fill(
        &self,
        row: &Row,
        app: Option<&str>,
        out: &mut kaeri_model::reading::ChapterRow,
    ) {
        out.parts = if row.numbers.is_empty() {
            u32::try_from(row.parts.len()).unwrap_or(u32::MAX)
        } else {
            row.total
        };
        out.missing_parts = row.missing.clone();
        out.part_groups = row
            .filled
            .iter()
            .map(|(part, group)| kaeri_model::reading::PartGroup {
                part: *part,
                group: group.clone(),
                language: self.foreign(group, app),
            })
            .collect();
        out.other_group = row.other_group;
        if row.group.is_some() {
            out.version_label = row.group.clone();
        }
        out.group_language = match (&row.group, row.other_group) {
            (Some(group), true) => self.foreign(group, app),
            _ => None,
        };
    }

    /// The part each entry of `row` is, beside its entries: none for a chapter listed whole.
    pub fn in_parts(row: &Row) -> bool {
        !row.numbers.is_empty()
    }
}

/// Whether a source's language tag is the app's: the same language, whatever the region.
pub fn same_language(a: &str, b: &str) -> bool {
    let primary = |t: &str| t.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    primary(a) == primary(b)
}

/// One group's chapter: its entries by part, and the parts in all its source said.
struct Held {
    entries: Vec<(u32, usize)>,
    said: u32,
}

/// Arrange a source's list, with the person's group for the series when they picked one, and
/// the app's language for the group to prefer when they did not, on its own numbers alone.
#[cfg(test)]
pub fn arrange(
    list: &[SourceChapter],
    chosen: Option<&str>,
    language: Option<&str>,
) -> Arranged {
    arrange_in(list, chosen, language, &Numbering::default())
}

/// [`arrange`], with what the work's other lists say of its numbering.
pub fn arrange_in(
    list: &[SourceChapter],
    chosen: Option<&str>,
    language: Option<&str>,
    others: &Numbering,
) -> Arranged {
    let mut numbering = others.clone();
    numbering.add(list);
    let restarts = crate::read_sources::restarts_per_volume(list);
    // Each group's entries, by index, in the list's order.
    let mut by_group: Vec<(Option<&str>, Vec<usize>)> = Vec::new();
    let mut group_at: HashMap<Option<&str>, usize> = HashMap::new();
    for (i, c) in list.iter().enumerate() {
        let group = c.version_label.as_deref();
        let at = *group_at.entry(group).or_insert_with(|| {
            by_group.push((group, Vec::new()));
            by_group.len() - 1
        });
        by_group[at].1.push(i);
    }
    let mut ids: Vec<ChapterId> = vec![ChapterId::new(""); list.len()];
    let mut parts: Vec<Option<u32>> = vec![None; list.len()];
    // Each group's chapters, by id.
    let mut group_rows: Vec<(Option<&str>, HashMap<ChapterId, Held>)> = Vec::new();
    for (group, entries) in &by_group {
        let found = parts_in(list, entries, &numbering);
        let mut rows: HashMap<ChapterId, Held> = HashMap::new();
        for &i in entries {
            let c = &list[i];
            let (key, part, said) = match found.get(&i) {
                Some(&(base, n, said)) => {
                    (key_of(base, c.volume.as_deref(), restarts), Some(n), said)
                }
                None => (c.key(restarts), None, 0),
            };
            let id = key.id();
            ids[i] = id.clone();
            parts[i] = part;
            match rows.get_mut(&id) {
                // Another part of a chapter already listed.
                Some(held)
                    if part.is_some_and(|n| held.entries.iter().all(|(m, _)| *m != n)) =>
                {
                    held.entries.push((part.unwrap_or(0), i));
                    held.said = held.said.max(said);
                }
                // The same chapter listed again by the group: the first stays.
                Some(_) => {}
                None => {
                    rows.insert(id, Held { entries: vec![(part.unwrap_or(0), i)], said });
                }
            }
        }
        for held in rows.values_mut() {
            held.entries.sort_by_key(|(n, _)| *n);
        }
        group_rows.push((*group, rows));
    }
    let mut groups: Vec<Group> = group_rows
        .iter()
        .zip(&by_group)
        .filter_map(|((g, rows), (_, entries))| {
            let label = (*g)?;
            Some(Group {
                label: label.to_owned(),
                chapters: u32::try_from(rows.len()).unwrap_or(u32::MAX),
                language: most_common(
                    entries.iter().filter_map(|&i| list[i].language.as_deref()),
                ),
            })
        })
        .collect();
    groups.sort_by(|a, b| b.chapters.cmp(&a.chapters).then_with(|| a.label.cmp(&b.label)));
    // The order chapters are taken from the groups: the pick, then those in the app's
    // language, then the rest, each by chapters; entries no group is named for last.
    let in_language = |g: &Group| {
        language.zip(g.language.as_deref()).is_some_and(|(app, its)| same_language(app, its))
    };
    let picked = chosen.filter(|c| groups.iter().any(|g| g.label == *c)).map(str::to_owned);
    let mut order: Vec<&str> = Vec::new();
    if let Some(p) = &picked {
        order.push(p.as_str());
    }
    for pass in [true, false] {
        for g in groups.iter().filter(|g| in_language(g) == pass) {
            if !order.contains(&g.label.as_str()) {
                order.push(g.label.as_str());
            }
        }
    }
    let preferred = order.first().map(|s| (*s).to_owned());
    let rank = |g: Option<&str>| match g {
        Some(label) => order.iter().position(|o| *o == label).unwrap_or(order.len()),
        None => order.len() + 1,
    };
    let mut ranked: Vec<&(Option<&str>, HashMap<ChapterId, Held>)> =
        group_rows.iter().collect();
    ranked.sort_by_key(|(g, _)| rank(*g));
    // Where any group first lists each chapter, so a pick of another group keeps each chapter
    // in its place.
    let mut first_at: HashMap<&ChapterId, usize> = HashMap::new();
    for (i, id) in ids.iter().enumerate() {
        first_at.entry(id).or_insert(i);
    }
    let mut order_ids: Vec<&ChapterId> = first_at.keys().copied().collect();
    order_ids.sort_by_key(|id| first_at[id]);
    let rows: Vec<Row> = order_ids
        .into_iter()
        .filter_map(|id| {
            let mut holders = ranked.iter().filter_map(|(g, rows)| Some((*g, rows.get(id)?)));
            let (group, best) = holders.next()?;
            let mut row = Row {
                id: id.clone(),
                parts: best.entries.iter().map(|(_, i)| *i).collect(),
                group: group.map(str::to_owned),
                other_group: preferred.is_some() && group != preferred.as_deref(),
                numbers: Vec::new(),
                total: 0,
                missing: Vec::new(),
                filled: Vec::new(),
            };
            let split = best.entries.iter().any(|(n, _)| *n > 0);
            if !split {
                return Some(row);
            }
            let mut have: Vec<(u32, usize)> = best.entries.clone();
            let mut said = best.said;
            // A part the group lacks, from the next group that has it.
            for (other, held) in holders {
                said = said.max(held.said);
                for &(n, i) in &held.entries {
                    if n > 0 && have.iter().all(|(m, _)| *m != n) {
                        have.push((n, i));
                        if let Some(other) = other {
                            row.filled.push((n, other.to_owned()));
                        }
                    }
                }
            }
            have.sort_by_key(|(n, _)| *n);
            let total = have.last().map_or(0, |(n, _)| *n).max(said);
            row.parts = have.iter().map(|(_, i)| *i).collect();
            row.numbers = have.iter().map(|(n, _)| *n).collect();
            row.total = total;
            row.missing = (1..=total).filter(|n| !row.numbers.contains(n)).collect();
            row.filled.sort();
            Some(row)
        })
        .collect();
    let at = rows.iter().enumerate().map(|(i, r)| (r.id.clone(), i)).collect();
    Arranged { ids, parts, rows, groups, preferred, chosen: picked.is_some(), at }
}

/// The value most often given, the first of equals in name order.
fn most_common<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for v in values {
        *counts.entry(v).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(v, _)| v.to_owned())
}

fn key_of(base: ChapterRange, volume: Option<&str>, restarts: bool) -> ChapterKey {
    match volume {
        Some(volume) if restarts => ChapterKey::in_volume(volume, base),
        _ => ChapterKey::numbered(base),
    }
}

/// The entries of one group that are parts: their whole chapter, part number and the parts in
/// all their source said (0 when it did not).
fn parts_in(
    list: &[SourceChapter],
    entries: &[usize],
    numbering: &Numbering,
) -> HashMap<usize, (ChapterRange, u32, u32)> {
    let mut found = HashMap::new();
    // Hyphenated and decimal candidates, by their whole chapter.
    let mut hyphens: HashMap<ChapterNumber, Vec<(u32, usize)>> = HashMap::new();
    let mut decimals: HashMap<u32, Vec<(u32, usize)>> = HashMap::new();
    let mut wholes: HashMap<u32, usize> = HashMap::new();
    // How many of the group's entries carry each number: parts named only in titles share one.
    let mut sharing: HashMap<ChapterRange, usize> = HashMap::new();
    for &i in entries {
        if let Some(number) = list[i].number {
            *sharing.entry(number).or_default() += 1;
        }
    }
    for &i in entries {
        let c = &list[i];
        if let Some(said) = part_word(&c.number_raw)
            && !said.of_volume()
            && let Some(base) = ChapterRange::parse(&said.rest)
        {
            found.insert(i, (base, said.n, said.total));
            continue;
        }
        if let Some(said) = c.title.as_deref().and_then(part_word)
            && !said.of_volume()
            && let Some(number) = c.number
        {
            match tenths(number) {
                // "1.2" titled "Part 2" is the second part of 1.
                Some((whole, d)) if d == said.n => {
                    found.insert(
                        i,
                        (ChapterRange::single(whole_number(whole)), said.n, said.total),
                    );
                    continue;
                }
                // Two entries numbered 4, titled "Pt. 1" and "Pt. 2", are 4 in parts. A title's
                // part word on a number of its own names a story told over several numbered
                // chapters ("287 The Tale Part 2" after 286 "Part 1"): each is a chapter.
                _ if sharing.get(&number).is_some_and(|n| *n > 1) => {
                    found.insert(i, (number, said.n, said.total));
                    continue;
                }
                _ => {}
            }
        }
        if let Some((a, b)) = hyphenated(&c.number_raw) {
            hyphens.entry(a).or_default().push((b, i));
            continue;
        }
        match c.number {
            Some(number) if number.end.is_none() => match tenths(number) {
                Some((whole, d)) => decimals.entry(whole).or_default().push((d, i)),
                None if number.start.milli() % 1000 == 0 => {
                    wholes.entry(number.start.whole()).or_insert(i);
                }
                None => {}
            },
            _ => {}
        }
    }
    let tenths_of =
        |held: &[(u32, usize)]| held.iter().map(|(n, _)| *n).collect::<BTreeSet<u32>>();
    // Hyphenated parts may have a gap: "1-1" and "1-3" are a chapter missing its second part.
    for (a, held) in &hyphens {
        let numbers: BTreeSet<u32> = held.iter().map(|(n, _)| *n).collect();
        if numbers.len() >= 2
            && numbers.contains(&1)
            && numbers.iter().all(|n| (1..=50).contains(n))
        {
            for &(b, i) in held {
                found.insert(i, (ChapterRange::single(*a), b, 0));
            }
        }
    }
    // The series numbers parts as decimals once one chapter has .1, .2 and no whole number.
    let pattern = decimals.iter().any(|(whole, held)| {
        let tenths = tenths_of(held);
        tenths.len() >= 2 && !wholes.contains_key(whole) && runs_from(&tenths, 1)
    });
    for (whole, held) in &decimals {
        let base = ChapterRange::single(whole_number(*whole));
        let tenths = tenths_of(held);
        match wholes.get(whole) {
            // "57", "57.2", "57.3": the whole number is the first part.
            Some(&first) if runs_from(&tenths, 2) => {
                found.insert(first, (base, 1, 0));
            }
            // Beside its whole number: extras.
            Some(_) => continue,
            None => {
                let own = runs_from(&tenths, 1) && (tenths.len() >= 2 || pattern);
                if !own && !numbering.parts(*whole, &tenths) {
                    continue;
                }
            }
        }
        for &(d, i) in held {
            found.insert(i, (base, d, 0));
        }
    }
    found
}

/// Whether `tenths` run from `from` with no gap.
fn runs_from(tenths: &BTreeSet<u32>, from: u32) -> bool {
    !tenths.is_empty() && tenths.iter().copied().eq(from..from + tenths.len() as u32)
}

/// What a work's lists say of how it numbers its chapters, every source and group together:
/// which whole numbers are named whole, which decimals are named as parts, and which beside
/// their whole number as extras. Built from plain numbers only; parts named in words and
/// hyphenated parts say nothing here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Numbering {
    /// Whole numbers some group lists whole.
    wholes: BTreeSet<u32>,
    /// The tenths of each whole number some group lists without that whole number, or as the
    /// later parts of it ("57.2" beside "57").
    parts: HashMap<u32, BTreeSet<u32>>,
    /// The tenths of each whole number some group lists beside that whole number, as extras.
    extras: HashMap<u32, BTreeSet<u32>>,
}

impl Numbering {
    /// Each list's numbers, as one.
    #[cfg(test)]
    pub fn of<'a>(lists: impl IntoIterator<Item = &'a [SourceChapter]>) -> Self {
        let mut numbering = Self::default();
        for list in lists {
            numbering.add(list);
        }
        numbering
    }

    /// Add a list's numbers, group by group.
    pub fn add(&mut self, list: &[SourceChapter]) {
        // Per group: its whole numbers, and the tenths listed of each whole number.
        type Held = (BTreeSet<u32>, HashMap<u32, BTreeSet<u32>>);
        let mut groups: HashMap<Option<&str>, Held> = HashMap::new();
        for c in list {
            if part_word(&c.number_raw).is_some() || hyphenated(&c.number_raw).is_some() {
                continue;
            }
            let Some(number) = c.number.filter(|n| n.end.is_none()) else { continue };
            let (wholes, decimals) = groups.entry(c.version_label.as_deref()).or_default();
            match tenths(number) {
                Some((whole, d)) => {
                    decimals.entry(whole).or_default().insert(d);
                }
                None if number.start.milli() % 1000 == 0 => {
                    wholes.insert(number.start.whole());
                }
                None => {}
            }
        }
        for (wholes, tenths) in groups.into_values() {
            for (whole, held) in tenths {
                let into = match wholes.contains(&whole) {
                    true if runs_from(&held, 2) => &mut self.parts,
                    true => &mut self.extras,
                    false => &mut self.parts,
                };
                into.entry(whole).or_default().extend(held);
            }
            self.wholes.extend(wholes);
        }
    }

    /// Whether a group's `tenths` of `whole`, listed without `whole`, are its parts by what the
    /// other lists say: never when another list names one of them beside its whole number;
    /// otherwise when they run from .1 and another list names `whole` whole, or when the tenths
    /// every list names for it run from .1 with at least two.
    fn parts(&self, whole: u32, tenths: &BTreeSet<u32>) -> bool {
        if self.extras.get(&whole).is_some_and(|e| !e.is_disjoint(tenths)) {
            return false;
        }
        if runs_from(tenths, 1) && self.wholes.contains(&whole) {
            return true;
        }
        let mut all: BTreeSet<u32> = self.parts.get(&whole).cloned().unwrap_or_default();
        // Named as later parts beside a whole number, that whole number is the first.
        if self.wholes.contains(&whole) && runs_from(&all, 2) {
            all.insert(1);
        }
        all.extend(tenths);
        all.len() >= 2 && runs_from(&all, 1)
    }
}

/// The extras a list has after each whole chapter, by their numbers: 7.5 and 7.6 after 7. Sources
/// number extras their own way (one has 7.1 and 7.2 where another has 7.5 and 7.6), so another
/// source's extras of a chapter that has its own here are the same ones under other numbers,
/// unless a number is shared.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extras(HashMap<(Option<String>, u32), BTreeSet<i64>>);

impl Extras {
    pub fn of<'a>(ids: impl IntoIterator<Item = &'a ChapterId>) -> Self {
        let mut extras = Self::default();
        for id in ids {
            extras.add(id);
        }
        extras
    }

    pub fn add(&mut self, id: &ChapterId) {
        if let Some((after, number)) = extra_of(id) {
            self.0.entry(after).or_default().insert(number);
        }
    }

    /// Whether `id`, an extra of another source's list `there`, is one of the extras this list
    /// numbers its own way: this list has extras after the same chapter and shares no number
    /// with `there`'s.
    pub fn numbered_otherwise(&self, there: &Self, id: &ChapterId) -> bool {
        let Some((after, _)) = extra_of(id) else { return false };
        let Some(here) = self.0.get(&after).filter(|h| !h.is_empty()) else { return false };
        there.0.get(&after).is_none_or(|theirs| here.is_disjoint(theirs))
    }
}

/// A chapter id that is a single number with a fraction: the whole chapter it follows (with
/// its volume where numbers restart per volume) and its number in thousandths.
fn extra_of(id: &ChapterId) -> Option<((Option<String>, u32), i64)> {
    let key = id.key()?;
    let range = key.range()?;
    if range.end.is_some() || range.start.milli() % 1000 == 0 {
        return None;
    }
    Some(((key.volume().map(str::to_owned), range.start.whole()), range.start.milli()))
}

fn whole_number(n: u32) -> ChapterNumber {
    ChapterNumber::whole_number(n)
}

/// A one-place decimal's whole number and tenth: 1.2 is (1, 2). `None` for a whole number or
/// two places (7.25).
fn tenths(number: ChapterRange) -> Option<(u32, u32)> {
    if number.end.is_some() {
        return None;
    }
    let frac = number.start.milli() % 1000;
    (frac != 0 && frac % 100 == 0).then(|| (number.start.whole(), (frac / 100) as u32))
}

/// A part a label or title names: its number, the parts in all when said, and the text
/// without it.
struct PartWord {
    n: u32,
    total: u32,
    rest: String,
}

impl PartWord {
    /// "Volume 2 Part 3": a part of a volume, as a light novel's, not of a chapter.
    fn of_volume(&self) -> bool {
        let lower = self.rest.to_lowercase();
        lower.contains("vol") && !lower.contains("ch")
    }
}

/// "part 2", "pt 2", "pt. 2", "Part2", "part 2/3", "part 2 of 3" in `text`.
fn part_word(text: &str) -> Option<PartWord> {
    let lower = text.to_lowercase();
    let bytes = lower.as_bytes();
    for word in ["part", "pt"] {
        let mut from = 0;
        while let Some(at) = lower[from..].find(word).map(|p| p + from) {
            from = at + word.len();
            // A word of its own: "apart" and "counterpart" are not parts.
            if at > 0 && bytes[at - 1].is_ascii_alphabetic() {
                continue;
            }
            let after = lower[from..].trim_start_matches(['.', ' ', ':', '#']);
            let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
            let Ok(n) = digits.parse::<u32>() else { continue };
            if n == 0 || n > 50 {
                continue;
            }
            let mut end = lower.len() - after.len() + digits.len();
            // The parts in all: "/3" or " of 3".
            let tail = &lower[end..];
            let of = tail.strip_prefix('/').or_else(|| tail.strip_prefix(" of "));
            let mut total = 0;
            if let Some(of) = of {
                let more: String =
                    of.trim_start().chars().take_while(char::is_ascii_digit).collect();
                if let Ok(t) = more.parse::<u32>()
                    && (n..=50).contains(&t)
                {
                    total = t;
                    end = lower.len() - of.trim_start().len() + more.len();
                }
            }
            let rest = format!("{}{}", &text[..at], &text[end..]);
            let rest = rest.trim_end_matches([' ', ',', '-', '(', ')', ':']).trim().to_owned();
            return Some(PartWord { n, total, rest });
        }
    }
    None
}

/// `a-b` at the end of a label, both whole numbers: "1-2", "Chapter 3-1".
fn hyphenated(label: &str) -> Option<(ChapterNumber, u32)> {
    let label = label.trim();
    let (head, tail) = label.rsplit_once('-')?;
    let tail = tail.trim();
    let head = head.trim_end();
    if tail.is_empty() || !tail.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let start = head.rfind(|c: char| !c.is_ascii_digit()).map_or(0, |p| p + 1);
    let a = &head[start..];
    if a.is_empty() || head[..start].ends_with(['.', '-']) {
        return None;
    }
    Some((ChapterNumber::parse(a)?, tail.parse().ok()?))
}

/// A merged chapter's title: the first part's, without its part words. A title that only
/// named the part is no title.
pub fn merged_title(first: &SourceChapter) -> Option<String> {
    let title = first.title.as_deref()?.trim();
    match part_word(title) {
        Some(said) => (!said.rest.is_empty()).then_some(said.rest),
        None => Some(title.to_owned()),
    }
}

/// A merged chapter's own label: the first part's, without its part words, or the chapter's
/// number for parts numbered "1-1" and "1.1".
pub fn merged_label(first: &SourceChapter, id: &ChapterId) -> String {
    if let Some(said) = part_word(&first.number_raw)
        && !said.rest.is_empty()
    {
        return said.rest;
    }
    let number = id.key().and_then(|k| k.range()).map(|r| r.to_string());
    let raw = &first.number_raw;
    match number {
        // Keep the source's own words before the number: "Chapter 1-1" reads "Chapter 1".
        Some(number) => {
            let prefix =
                raw.find(|c: char| c.is_ascii_digit()).map_or("", |p| raw[..p].trim_end());
            if prefix.is_empty() { number } else { format!("{prefix} {number}") }
        }
        None => raw.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, label: &str, group: Option<&str>) -> SourceChapter {
        let mut c = SourceChapter::new(id, label);
        c.version_label = group.map(str::to_owned);
        c
    }

    fn ids(arranged: &Arranged) -> Vec<(&str, usize)> {
        arranged.rows.iter().map(|r| (r.id.as_str(), r.parts.len())).collect()
    }

    #[test]
    fn parts_in_words_hyphens_and_decimals_merge_into_one_chapter() {
        let words = [
            entry("a", "Chapter 1 part 1", None),
            entry("b", "Chapter 1 part 2", None),
            entry("c", "Chapter 2", None),
        ];
        let arranged = arrange(&words, None, None);
        assert_eq!(ids(&arranged), vec![("1", 2), ("2", 1)]);
        assert_eq!(arranged.parts, vec![Some(1), Some(2), None]);
        assert_eq!(merged_label(&words[0], &arranged.rows[0].id), "Chapter 1");

        let mut titled = [entry("a", "4", None), entry("b", "4", None)];
        titled[0].title = Some("The Gate (Pt. 1)".into());
        titled[1].title = Some("The Gate (Pt. 2)".into());
        assert_eq!(ids(&arrange(&titled, None, None)), vec![("4", 2)]);

        let hyphens =
            [entry("a", "1-1", None), entry("b", "1-2", None), entry("c", "2-1", None)];
        let arranged = arrange(&hyphens, None, None);
        // A lone 2-1 has no second part yet, and reads as chapter 2 as before.
        assert_eq!(ids(&arranged), vec![("1", 2), ("2", 1)]);
        assert_eq!(merged_label(&hyphens[0], &arranged.rows[0].id), "1");
        // A run of two chapters is not parts.
        assert_eq!(ids(&arrange(&[entry("a", "12-13", None)], None, None)), vec![("12-13", 1)]);

        let decimals = [
            entry("a", "1.1", None),
            entry("b", "1.2", None),
            entry("c", "2.1", None),
            entry("d", "2.2", None),
            entry("e", "2.3", None),
            entry("f", "3.1", None),
        ];
        let arranged = arrange(&decimals, None, None);
        assert_eq!(ids(&arranged), vec![("1", 2), ("2", 3), ("3", 1)]);
        let trackers: Vec<u32> = arranged
            .rows
            .iter()
            .map(|r| r.id.key().unwrap().range().unwrap().last().whole())
            .collect();
        assert_eq!(trackers, vec![1, 2, 3]);
    }

    /// A story told over several numbered chapters names its parts in the titles; each is a
    /// chapter of its own, never part 4 of 289 with three parts missing.
    #[test]
    fn a_part_word_in_the_title_of_a_number_of_its_own_is_a_chapter() {
        let mut list = [
            entry("a", "285", None),
            entry("b", "286", None),
            entry("c", "287", None),
            entry("d", "288", None),
            entry("e", "289", None),
        ];
        list[1].title = Some("The Long Tale Part 1".into());
        list[2].title = Some("The Long Tale Part 2".into());
        list[3].title = Some("The Long Tale (Pt. 3)".into());
        list[4].title = Some("The Long Tale Part 4".into());
        let arranged = arrange(&list, None, None);
        assert_eq!(
            ids(&arranged),
            vec![("285", 1), ("286", 1), ("287", 1), ("288", 1), ("289", 1)]
        );
        assert!(arranged.parts.iter().all(Option::is_none), "{:?}", arranged.parts);
        assert!(arranged.rows.iter().all(|r| r.missing.is_empty() && r.numbers.is_empty()));
        // A decimal whose title names the same part is still that part.
        let mut decimal = [entry("a", "3.1", None), entry("b", "3.2", None)];
        decimal[1].title = Some("Part 2".into());
        assert_eq!(ids(&arrange(&decimal, None, None)), vec![("3", 2)]);
    }

    #[test]
    fn a_decimal_beside_its_whole_chapter_is_an_extra_not_a_part() {
        // 10.5 beside 10, and alone: extras in a series that numbers nothing as parts.
        let extras = [
            entry("a", "9", None),
            entry("b", "10", None),
            entry("c", "10.5", None),
            entry("d", "11", None),
            entry("e", "11.5", None),
        ];
        let arranged = arrange(&extras, None, None);
        assert_eq!(arranged.rows.len(), 5);
        assert!(arranged.parts.iter().all(Option::is_none));
        assert_eq!(arranged.rows[2].id.as_str(), "10.5");
        // .1 and .2 beside a whole 10 are extras too.
        let beside =
            [entry("a", "10", None), entry("b", "10.1", None), entry("c", "10.2", None)];
        assert_eq!(arrange(&beside, None, None).rows.len(), 3);
        // And in a series that does number parts as decimals, 10.5 stays an extra chapter.
        let mixed = [
            entry("a", "1.1", None),
            entry("b", "1.2", None),
            entry("c", "10.5", None),
            entry("d", "4.1", None),
        ];
        assert_eq!(ids(&arrange(&mixed, None, None)), vec![("1", 2), ("10.5", 1), ("4", 1)]);
        // A source whose labels say Part is believed whatever the number.
        let said = [entry("a", "10.5 Part 1", None), entry("b", "10.5 Part 2", None)];
        assert_eq!(ids(&arrange(&said, None, None)), vec![("10.5", 2)]);
    }

    #[test]
    fn another_list_says_whether_a_lone_first_decimal_is_a_part_or_an_extra() {
        let lone = [entry("a", "6", None), entry("b", "7.1", None), entry("c", "8", None)];
        // Alone it reads as its own chapter, as a lone 10.5 does.
        assert_eq!(ids(&arrange(&lone, None, None)), vec![("6", 1), ("7.1", 1), ("8", 1)]);
        // Another source with a whole 7 makes it 7's first part.
        let whole = [entry("x", "7", None), entry("y", "8", None)];
        let arranged = arrange_in(&lone, None, None, &Numbering::of([&whole[..]]));
        assert_eq!(ids(&arranged), vec![("6", 1), ("7", 1), ("8", 1)]);
        assert_eq!(arranged.parts, vec![None, Some(1), None]);
        // One with 7 and 7.1 beside it numbers 7.1 as an extra, and so does this one.
        let extra = [entry("x", "7", None), entry("y", "7.1", None)];
        let arranged = arrange_in(&lone, None, None, &Numbering::of([&extra[..]]));
        assert_eq!(ids(&arranged), vec![("6", 1), ("7.1", 1), ("8", 1)]);
        // A lone 10.5 stays an extra whatever the others list.
        let half = [entry("a", "10", None), entry("b", "10.5", None)];
        let others = [entry("x", "10", None)];
        let arranged = arrange_in(&half, None, None, &Numbering::of([&others[..]]));
        assert_eq!(ids(&arranged), vec![("10", 1), ("10.5", 1)]);
    }

    #[test]
    fn a_whole_chapter_followed_by_its_second_and_third_parts_is_their_first() {
        let list = [
            entry("a", "56", None),
            entry("b", "57", None),
            entry("c", "57.2", None),
            entry("d", "57.3", None),
            entry("e", "58", None),
        ];
        let arranged = arrange(&list, None, None);
        assert_eq!(ids(&arranged), vec![("56", 1), ("57", 3), ("58", 1)]);
        assert_eq!(arranged.parts, vec![None, Some(1), Some(2), Some(3), None]);
        // A source that lists 57.2 and 57.3 alone has 57's second and third parts.
        let rest = [entry("a", "57.2", None), entry("b", "57.3", None)];
        let arranged = arrange_in(&rest, None, None, &Numbering::of([&list[..]]));
        assert_eq!(ids(&arranged), vec![("57", 2)]);
        assert_eq!(arranged.parts, vec![Some(2), Some(3)]);
    }

    #[test]
    fn parts_merge_within_a_group_never_across_groups() {
        let list = [
            entry("a", "1.1", Some("Group A")),
            entry("b", "1.1", Some("Group B")),
            entry("c", "1.2", Some("Group A")),
            entry("d", "1", Some("Group B")),
        ];
        let arranged = arrange(&list, None, None);
        // Group B has a whole 1 and its 1.1 is an extra; Group A's are parts.
        assert_eq!(arranged.ids[0].as_str(), "1");
        assert_eq!(arranged.ids[1].as_str(), "1.1");
        assert_eq!(arranged.parts[2], Some(2));
    }

    #[test]
    fn the_preferred_group_lists_each_chapter_and_another_fills_its_gaps() {
        let list = [
            entry("a1", "1", Some("Group A")),
            entry("b1", "1", Some("Group B")),
            entry("a2", "2", Some("Group A")),
            entry("b2", "2", Some("Group B")),
            entry("b3", "3", Some("Group B")),
            entry("a4", "4", Some("Group A")),
            entry("a5", "5", Some("Group A")),
            entry("x6", "6", None),
        ];
        let arranged = arrange(&list, None, None);
        assert_eq!(
            arranged.groups,
            vec![
                Group { label: "Group A".into(), chapters: 4, language: None },
                Group { label: "Group B".into(), chapters: 3, language: None },
            ]
        );
        assert_eq!((arranged.preferred.as_deref(), arranged.chosen), (Some("Group A"), false));
        let shown: Vec<(&str, &str, bool)> = arranged
            .rows
            .iter()
            .map(|r| (r.id.as_str(), list[r.parts[0]].id.as_str(), r.other_group))
            .collect();
        assert_eq!(
            shown,
            vec![
                ("1", "a1", false),
                ("2", "a2", false),
                ("3", "b3", true),
                ("4", "a4", false),
                ("5", "a5", false),
                ("6", "x6", true),
            ]
        );
        // The person's pick wins over the count; a pick the list lacks falls back to it.
        let picked = arrange(&list, Some("Group B"), None);
        assert!(picked.chosen);
        assert_eq!(list[picked.rows[0].parts[0]].id, "b1");
        assert!(
            picked.rows[3].other_group && picked.rows[3].group.as_deref() == Some("Group A")
        );
        let gone = arrange(&list, Some("Group Gone"), None);
        assert_eq!((gone.preferred.as_deref(), gone.chosen), (Some("Group A"), false));
        // Read state hangs on the chapter, whichever group lists it.
        assert_eq!(picked.rows[1].id, arranged.rows[1].id);
    }

    #[test]
    fn a_list_without_groups_is_listed_as_it_was() {
        let list = [entry("a", "1", None), entry("b", "1", None), entry("c", "Extra", None)];
        let arranged = arrange(&list, None, None);
        assert_eq!(ids(&arranged), vec![("1", 1), ("x/-/extra", 1)]);
        assert_eq!((arranged.preferred, arranged.rows[0].other_group), (None, false));
        assert_eq!(arranged.rows[0].parts, vec![0]);
    }

    fn in_language(id: &str, label: &str, group: &str, language: &str) -> SourceChapter {
        let mut c = entry(id, label, Some(group));
        c.language = Some(language.into());
        c
    }

    #[test]
    fn the_default_group_is_the_one_in_the_apps_language_then_the_most_chapters() {
        let list = [
            in_language("a1", "1", "Group A", "es"),
            in_language("a2", "2", "Group A", "es"),
            in_language("a3", "3", "Group A", "es"),
            in_language("b1", "1", "Group B", "en"),
            in_language("b2", "2", "Group B", "en"),
            in_language("c3", "3", "Group C", "en-GB"),
        ];
        let english = arrange(&list, None, Some("en-US"));
        assert_eq!(english.preferred.as_deref(), Some("Group B"));
        assert_eq!(english.language_of("Group A"), Some("es"));
        // Chapter 3 comes from the other group in the app's language before the bigger one.
        let third = english.row(&ChapterId::new("3")).unwrap();
        assert_eq!((list[third.parts[0]].id.as_str(), third.other_group), ("c3", true));
        // With no language that matches, the most chapters wins.
        assert_eq!(arrange(&list, None, Some("fr")).preferred.as_deref(), Some("Group A"));
        assert_eq!(arrange(&list, None, None).preferred.as_deref(), Some("Group A"));
    }

    #[test]
    fn a_part_the_group_lacks_comes_from_another_group_and_a_part_none_has_is_missing() {
        let list = [
            entry("a1", "Chapter 1 part 1/3", Some("Group A")),
            entry("a3", "Chapter 1 part 3/3", Some("Group A")),
            entry("a4", "Chapter 2", Some("Group A")),
            entry("b2", "Chapter 1 part 2", Some("Group B")),
            entry("c1", "Chapter 3 part 1 of 3", Some("Group A")),
            entry("c2", "Chapter 3 part 3", Some("Group A")),
        ];
        let arranged = arrange(&list, None, None);
        assert_eq!(arranged.preferred.as_deref(), Some("Group A"));
        let first = arranged.row(&ChapterId::new("1")).unwrap();
        let read: Vec<&str> = first.parts.iter().map(|&i| list[i].id.as_str()).collect();
        assert_eq!(read, vec!["a1", "b2", "a3"]);
        assert_eq!((first.numbers.clone(), first.total), (vec![1, 2, 3], 3));
        assert_eq!(first.filled, vec![(2, "Group B".to_owned())]);
        assert!(first.missing.is_empty() && !first.other_group);
        // No group has chapter 3's second part: it merges and names the gap.
        let third = arranged.row(&ChapterId::new("3")).unwrap();
        assert_eq!((third.parts.len(), third.total, third.missing.clone()), (2, 3, vec![2]));
        // Hyphenated parts with a gap merge too.
        let hyphens = [entry("a", "5-1", None), entry("b", "5-3", None)];
        let row = &arrange(&hyphens, None, None).rows[0];
        assert_eq!((row.id.as_str(), row.missing.clone()), ("5", vec![2]));
    }

    #[test]
    fn a_volumes_parts_stay_as_they_are() {
        let list = [entry("a", "Volume 2 Part 1", None), entry("b", "Volume 2 Part 2", None)];
        let arranged = arrange(&list, None, None);
        assert!(arranged.parts.iter().all(Option::is_none));
    }

    #[test]
    fn a_long_list_with_several_groups_arranges_in_step_with_its_length() {
        let list: Vec<SourceChapter> = (1..=6000)
            .map(|n| {
                entry(&format!("e{n}"), &(n / 2).to_string(), Some(["A", "B"][n as usize % 2]))
            })
            .collect();
        let started = std::time::Instant::now();
        let arranged = arrange(&list, Some("B"), None);
        assert_eq!(arranged.rows.len(), 3001);
        for row in &arranged.rows {
            assert!(arranged.row(&row.id).is_some());
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
