//! Extras: chapters beside the story's whole chapters, which a person may pass over. The end
//! page offers to skip a run of them, and what comes next to read moves past those behind the
//! furthest chapter read, while they stay unread in the list.
//!
//! An extra is, as [`crate::read_catalog::arrange`] reads a list:
//! - a decimal beside its whole number (`16.5` beside `16`), or one with no `.1` before it
//!   (`16.5` alone). A run from `.1` with no whole number is a chapter's parts, not extras;
//! - a chapter with no number ("Extra", "Vol. 1 Extras").

use kaeri_core::books::ChapterId;

/// Whether `id` is an extra in a list that holds the ids `listed` says it does.
pub(crate) fn is_extra(id: &ChapterId, listed: impl Fn(&str) -> bool) -> bool {
    let Some(key) = id.key() else { return false };
    let Some(range) = key.range() else { return key.as_str().starts_with("x/") };
    if range.end.is_some() || !range.start.is_split() {
        return false;
    }
    // A restarting work's keys carry the volume: its siblings are in the same volume.
    let text = key.as_str();
    let prefix = text.rsplit_once('/').map_or("", |(volume, _)| volume);
    let sibling = |number: String| {
        if prefix.is_empty() { number } else { format!("{prefix}/{number}") }
    };
    let whole = range.start.whole();
    listed(&sibling(whole.to_string())) || !listed(&sibling(format!("{whole}.1")))
}

/// Where the end page's skip goes: when the chapter at `next` is an extra, the first chapter
/// after it that is not one. `None` for a next chapter that is not an extra, or extras with no
/// whole chapter after them.
pub(crate) fn skip_to(ids: &[ChapterId], next: usize) -> Option<usize> {
    let listed = |id: &str| ids.iter().any(|c| c.as_str() == id);
    if !is_extra(ids.get(next)?, listed) {
        return None;
    }
    (next + 1..ids.len()).find(|&i| !is_extra(&ids[i], listed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<ChapterId> {
        list.iter().map(|id| ChapterId::new(*id)).collect()
    }

    fn extra(id: &str, list: &[&str]) -> bool {
        is_extra(&ChapterId::new(id), |c| list.contains(&c))
    }

    #[test]
    fn a_decimal_beside_its_whole_number_or_alone_is_an_extra_and_parts_are_not() {
        assert!(extra("16.5", &["16", "16.5", "17"]));
        assert!(extra("16.1", &["16", "16.1", "17"]), "beside its whole 16");
        assert!(extra("16.5", &["15", "16.5", "17"]), "alone, not from .1");
        assert!(!extra("16.1", &["15", "16.1", "16.2", "17"]), "a first part");
        assert!(!extra("16.2", &["15", "16.1", "16.2", "17"]), "a second part");
        assert!(!extra("16", &["16", "16.5"]));
        assert!(!extra("12-13", &["11", "12-13"]), "a run of chapters");
        assert!(extra("x/-/extra", &["1", "x/-/extra"]));
        assert!(extra("x/1/vol. 1 extras", &["1", "x/1/vol. 1 extras"]));
    }

    #[test]
    fn a_restarting_works_extras_are_judged_within_their_volume() {
        let list = ["v1/3", "v2/3.5", "v2/4"];
        assert!(extra("v2/3.5", &list), "no .1 before it in volume 2");
        assert!(extra("v2/3.5", &["v2/3", "v2/3.5"]));
        assert!(!extra("v2/3.1", &["v1/3", "v2/3.1", "v2/3.2"]), "parts, as v1/3 is another 3");
    }

    #[test]
    fn the_skip_goes_past_a_run_of_extras_to_the_next_whole_chapter() {
        let list = ids(&["16", "16.5", "16.6", "x/-/omake", "17", "18"]);
        assert_eq!(skip_to(&list, 1), Some(4));
        assert_eq!(skip_to(&list, 4), None, "a normal next chapter offers no skip");
        // Extras at the end, with no whole chapter after them, offer nothing to skip to.
        assert_eq!(skip_to(&ids(&["16", "16.5"]), 1), None);
        assert_eq!(skip_to(&list, 9), None);
        // Parts are the story, not extras.
        assert_eq!(skip_to(&ids(&["15", "16.1", "16.2", "17"]), 1), None);
    }
}
