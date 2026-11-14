//! What follows a work's whole chapters, one rule for every list: the series page, the reader's
//! chapters and its end page, a download from another source.
//!
//! **What follows a chapter.** A decimal (`21.1`, `16.5`) follows its whole number, and a chapter
//! with no number ("Extra", "Vol. 1 Extras") follows the last numbered one before it in its list.
//! What follows a chapter is either its later parts or extras beside the story.
//!
//! **Parts and extras within a list** ([`is_extra`]). The decimals that run from `.1` with no gap
//! (`21.1`, `21.2`) are the chapter's later parts: story, never skipped. Any other decimal (`16.5`,
//! or `23.5` and `23.6` with no `23.1`) is an extra, and so is a chapter with no number. A title
//! sets a decimal apart as an extra too (`28.1` "Short Extra"), in a list that leaves its whole
//! chapters untitled: where a list titles its chapters, a part has its own title like the rest.
//!
//! **Across sources** ([`Following::stands_for`]). Sources split and number what follows a chapter
//! their own way: one lists `21.1` and `21.2` where another has `21.5`, or `7.1` and `7.2` where
//! another has `7.5` and `7.6`, or "Extra" where another has `7.5`. So what this list has after a
//! chapter stands for everything another source has after it, and none of that fills in. Only a
//! chapter set apart by its title comes in beside them: one whose title none of this list's
//! chapters after the same chapter carry, where each of those has a title of its own (`16.6`
//! "Vol. 1 Extras 2" beside `16.5` "Vol. 1 Extras"). Where this list has nothing after a chapter,
//! the other source's chapters after it fill in as they are. Rows kept out this way still lend
//! their volume to the rows beside them ([`super::volumes`]).

use std::collections::{HashMap, HashSet};

use kaeri_core::books::ChapterId;

/// The whole chapter something follows: its number, with the volume where numbers restart per
/// volume.
pub type After = (Option<String>, u32);

/// The whole chapter each entry of a list in reading order follows: a decimal's own whole number,
/// a chapter with no number the last numbered one before it. `None` for a whole chapter, a run
/// of chapters, or an unnumbered chapter before any number.
pub fn follows<'a>(ids: impl IntoIterator<Item = &'a ChapterId>) -> Vec<Option<After>> {
    let mut last: Option<After> = None;
    ids.into_iter()
        .map(|id| {
            let key = id.key()?;
            let Some(range) = key.range() else {
                return key.as_str().starts_with("x/").then(|| last.clone()).flatten();
            };
            let after = (key.volume().map(str::to_owned), range.start.whole());
            if range.end.is_some() {
                last = Some((after.0, range.end.map_or(after.1, |e| e.whole())));
                return None;
            }
            last = Some(after.clone());
            range.start.is_split().then_some(after)
        })
        .collect()
}

/// Whether a list titles its whole chapters, at least one in two: then a title says nothing of
/// whether a decimal is an extra.
pub fn titles_chapters<'a>(
    entries: impl IntoIterator<Item = (&'a ChapterId, Option<&'a str>)>,
) -> bool {
    let (mut wholes, mut titled) = (0, 0);
    for (id, title) in entries {
        let whole = id
            .key()
            .and_then(|k| k.range())
            .is_some_and(|r| r.end.is_none() && !r.start.is_split());
        if whole {
            wholes += 1;
            titled += usize::from(title.is_some_and(|t| !t.trim().is_empty()));
        }
    }
    wholes > 0 && titled * 2 >= wholes
}

fn titled(title: Option<&str>) -> Option<String> {
    title.map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty())
}

/// Whether entry `at` of a list in reading order is an extra, as the module says. `listed` holds
/// the list's ids; `titles` is whether the list titles its whole chapters.
pub fn is_extra(
    id: &ChapterId,
    title: Option<&str>,
    listed: impl Fn(&str) -> bool,
    titles: bool,
) -> bool {
    let Some(key) = id.key() else { return false };
    let Some(range) = key.range() else { return key.as_str().starts_with("x/") };
    if range.end.is_some() || !range.start.is_split() {
        return false;
    }
    if !titles && titled(title).is_some() {
        return true;
    }
    let milli = range.start.milli() % 1000;
    if milli % 100 != 0 {
        return true;
    }
    // A restarting work's keys carry the volume: its siblings are in the same volume.
    let text = key.as_str();
    let prefix = text.rsplit_once('/').map_or("", |(volume, _)| volume);
    let whole = range.start.whole();
    let sibling = |tenth: i64| {
        let number = format!("{whole}.{tenth}");
        if prefix.is_empty() { number } else { format!("{prefix}/{number}") }
    };
    !(1..milli / 100).all(|tenth| listed(&sibling(tenth)))
}

/// Where the end page's skip goes: when the chapter at `next` is an extra, the first chapter
/// after it that is not one. `None` for a next chapter that is not an extra, or extras with no
/// whole chapter after them. `list` is the reader's chapters in order, with their titles.
pub fn skip_to(list: &[(ChapterId, Option<String>)], next: usize) -> Option<usize> {
    let ids: HashSet<&str> = list.iter().map(|(id, _)| id.as_str()).collect();
    let listed = |id: &str| ids.contains(id);
    let titles = titles_chapters(list.iter().map(|(id, t)| (id, t.as_deref())));
    let extra = |i: usize| {
        let (id, title) = &list[i];
        is_extra(id, title.as_deref(), listed, titles)
    };
    if next >= list.len() || !extra(next) {
        return None;
    }
    (next + 1..list.len()).find(|&i| !extra(i))
}

/// What a list has after each whole chapter: the titles, and whether any has none.
#[derive(Debug, Clone, Default)]
pub struct Following(HashMap<After, (HashSet<String>, bool)>);

impl Following {
    /// From a list's entries in reading order, with their titles.
    pub fn of<'a>(entries: impl IntoIterator<Item = (&'a ChapterId, Option<&'a str>)>) -> Self {
        let entries: Vec<_> = entries.into_iter().collect();
        let mut following = Self::default();
        for (after, (_, title)) in
            follows(entries.iter().map(|(id, _)| *id)).into_iter().zip(&entries)
        {
            if let Some(after) = after {
                following.add(after, *title);
            }
        }
        following
    }

    /// Note an entry after `after`, as one filled in from another source.
    pub fn add(&mut self, after: After, title: Option<&str>) {
        let (titles, untitled) = self.0.entry(after).or_default();
        match titled(title) {
            Some(t) => {
                titles.insert(t);
            }
            None => *untitled = true,
        }
    }

    /// Whether what this list has after `after` stands for another source's entry after it,
    /// titled `title`, so it stays out. `titles` is whether that source titles its chapters, when
    /// a title says nothing.
    pub fn stands_for(&self, after: &After, title: Option<&str>, titles: bool) -> bool {
        let Some((here, untitled)) = self.0.get(after) else { return false };
        match titled(title).filter(|_| !titles) {
            Some(title) => *untitled || here.contains(&title),
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<ChapterId> {
        list.iter().map(|id| ChapterId::new(*id)).collect()
    }

    fn extra(id: &str, list: &[&str]) -> bool {
        is_extra(&ChapterId::new(id), None, |c| list.contains(&c), false)
    }

    fn untitled(list: &[&str]) -> Vec<(ChapterId, Option<String>)> {
        ids(list).into_iter().map(|id| (id, None)).collect()
    }

    #[test]
    fn a_run_from_one_tenth_is_the_story_and_any_other_decimal_an_extra() {
        assert!(
            !extra("21.1", &["21", "21.1", "21.2", "21.5", "22"]),
            "a part beside its whole"
        );
        assert!(!extra("21.2", &["21", "21.1", "21.2", "21.5", "22"]), "its second part");
        assert!(
            extra("21.5", &["21", "21.1", "21.2", "21.5", "22"]),
            "a half chapter after them"
        );
        assert!(extra("16.5", &["16", "16.5", "17"]));
        assert!(extra("16.5", &["15", "16.5", "17"]), "alone, not from .1");
        assert!(extra("23.6", &["23", "23.5", "23.6", "24"]), "a run from .5");
        assert!(!extra("16.1", &["15", "16.1", "16.2", "17"]), "parts with no whole number");
        assert!(extra("16.2", &["16", "16.2", "17"]), "a gap after the whole number");
        assert!(extra("7.25", &["7", "7.25", "8"]), "two places");
        assert!(!extra("16", &["16", "16.5"]));
        assert!(!extra("12-13", &["11", "12-13"]), "a run of chapters");
        assert!(extra("x/-/extra", &["1", "x/-/extra"]));
        assert!(extra("x/1/vol. 1 extras", &["1", "x/1/vol. 1 extras"]));
    }

    #[test]
    fn a_title_sets_a_decimal_apart_where_the_list_leaves_its_chapters_untitled() {
        let list = ["28", "28.1", "28.2", "28.5", "29"];
        let listed = |c: &str| list.contains(&c);
        let id = ChapterId::new("28.1");
        assert!(is_extra(&id, Some("Short Extra"), listed, false));
        assert!(!is_extra(&id, None, listed, false));
        // Where the list titles its chapters, a part titled like them is still a part.
        assert!(!is_extra(&id, Some("The Rain, Continued"), listed, true));
    }

    #[test]
    fn a_restarting_works_extras_are_judged_within_their_volume() {
        let list = ["v1/3", "v1/3.1", "v2/3.5", "v2/4"];
        assert!(extra("v2/3.5", &list), "no .1 before it in volume 2");
        assert!(!extra("v2/3.1", &["v1/3", "v2/3.1", "v2/3.2"]), "parts, as v1/3 is another 3");
        assert!(extra("v2/3.2", &["v1/3.1", "v2/3", "v2/3.2"]), "v1/3.1 is not volume 2's");
    }

    #[test]
    fn the_skip_goes_past_a_run_of_extras_to_the_next_story_chapter() {
        let list = untitled(&["16", "16.5", "16.6", "x/-/omake", "17", "18"]);
        assert_eq!(skip_to(&list, 1), Some(4));
        assert_eq!(skip_to(&list, 4), None, "a normal next chapter offers no skip");
        // Extras at the end, with no whole chapter after them, offer nothing to skip to.
        assert_eq!(skip_to(&untitled(&["16", "16.5"]), 1), None);
        assert_eq!(skip_to(&list, 9), None);
        // Parts are the story: the skip neither starts at them nor passes them.
        let split = untitled(&["21", "21.1", "21.2", "21.5", "22"]);
        assert_eq!(skip_to(&split, 1), None, "21.1 continues 21");
        assert_eq!(skip_to(&split, 2), None, "so does 21.2");
        assert_eq!(skip_to(&split, 3), Some(4), "21.5 is the extra");
        assert_eq!(skip_to(&untitled(&["15", "16.1", "16.2", "17"]), 1), None);
    }

    #[test]
    fn a_titled_extra_numbered_from_one_tenth_is_skipped() {
        let list: Vec<(ChapterId, Option<String>)> = [
            ("28", None),
            ("28.1", Some("Short Extra")),
            ("28.2", Some("Short Extra")),
            ("28.5", Some("Vol. 3 Extras")),
            ("29", None),
        ]
        .into_iter()
        .map(|(id, t)| (ChapterId::new(id), t.map(str::to_owned)))
        .collect();
        assert_eq!(skip_to(&list, 1), Some(4));
    }

    fn following(list: &[(&str, Option<&str>)]) -> (Following, Vec<Option<After>>) {
        let ids: Vec<ChapterId> = list.iter().map(|(id, _)| ChapterId::new(*id)).collect();
        let f = Following::of(ids.iter().zip(list.iter().map(|(_, t)| *t)));
        (f, follows(&ids))
    }

    #[test]
    fn what_follows_a_chapter_here_stands_for_what_follows_it_there() {
        let (here, _) = following(&[
            ("21", None),
            ("21.1", None),
            ("21.2", None),
            ("22", None),
            ("x/-/extra", None),
            ("28", None),
            ("28.1", Some("Short Extra")),
            ("28.5", Some("Vol. 3 Extras")),
            ("28.6", Some("Shop Extra")),
            ("29", None),
            ("30", None),
        ]);
        let after = |n: u32| (None, n);
        // Another source's half chapter for 21's parts, and its extras of 28 under other
        // numbers, titled as here.
        assert!(here.stands_for(&after(21), None, false));
        assert!(here.stands_for(&after(28), Some("Vol. 3 Extras"), false));
        assert!(here.stands_for(&after(28), None, false));
        // An extra in words here stands for the other's numbered one.
        assert!(here.stands_for(&after(22), None, false), "\"Extra\" after 22");
        // A title none here carries, where each here has a title: a different extra.
        assert!(!here.stands_for(&after(28), Some("April Fools' Special"), false));
        // Nothing after 29 here: the other's fill in.
        assert!(!here.stands_for(&after(29), None, false));
        // Untitled here may be any of the other's.
        assert!(here.stands_for(&after(21), Some("April Fools' Special"), false));
        // A source that titles its chapters says nothing by a title.
        assert!(here.stands_for(&after(28), Some("April Fools' Special"), true));
    }

    #[test]
    fn an_unnumbered_chapter_follows_the_last_number_before_it() {
        let (_, after) = following(&[
            ("x/-/prologue", None),
            ("6", None),
            ("7", None),
            ("x/-/extra", None),
            ("7.5", None),
            ("8", None),
        ]);
        assert_eq!(after, [None, None, None, Some((None, 7)), Some((None, 7)), None]);
    }
}
