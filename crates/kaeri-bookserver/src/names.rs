//! Chapters named the way a server names its books.

use kaeri_books::naming;
use kaeri_core::books::SourceChapter;

/// A book listed as `title` (a file name or a metadata title) with an optional `number` the
/// server gives. The title's own volume and chapter are read first; the server's number is the
/// chapter only when the title says none, and never when the title says the book is a volume.
pub fn chapter_of(id: &str, title: &str, number: Option<&str>) -> SourceChapter {
    let named = naming::parse(naming::stem(title));
    let given = number.map(str::trim).filter(|n| !n.is_empty()).and_then(naming::milli);
    let chapter = match (&named.chapter, named.volume.is_some(), given) {
        (Some(n), _, _) => Some(n.clone()),
        (None, true, _) => None,
        (None, false, Some((_, shown))) => Some(shown),
        (None, false, None) => None,
    };
    let tidy = naming::tidy(naming::stem(title));
    SourceChapter::of(id, named.volume.as_deref(), chapter.as_deref(), Some(&tidy), title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_give_volumes_and_chapters_and_the_servers_number_fills_gaps() {
        let c = chapter_of("1", "Paper Kite Rangers v02 c003", Some("3"));
        assert_eq!((c.number_raw.as_str(), c.volume.as_deref()), ("Chapter 3", Some("2")));
        // A whole-volume book: the server's Number only counts volumes.
        let v = chapter_of("2", "Moonlit Harbor v01", Some("1"));
        assert_eq!((v.number_raw.as_str(), v.number), ("Volume 1", None));
        // A title with no numbers takes the server's.
        let n = chapter_of("3", "The Long Walk", Some("12.5"));
        assert_eq!(n.number_raw, "Chapter 12.5");
        let none = chapter_of("4", "Scan Final", None);
        assert_eq!(none.number, None);
        assert_eq!(none.number_raw, "Scan Final");
    }
}
