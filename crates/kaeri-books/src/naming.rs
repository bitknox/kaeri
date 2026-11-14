//! What a book file's name says: its series, volume and chapter number.
//!
//! ComicInfo and EPUB metadata come first when a book has them; this is for the rest, and for
//! the books that carry metadata for only some of a series. Numbers are read as thousandths
//! (`10.5` is 10 500) so a half chapter stays a half chapter without a float.
//!
//! The rules are deliberately narrow. A number counts only after a marker (`v`, `vol`, `volume`,
//! `c`, `ch`, `chapter`, `#`) or as the last word of a name, never from inside a word, so
//! `scan_final_FINAL_2` stays a name with no number in it.

/// What a name says. Every part is optional: a name with nothing to read leaves them empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Named {
    /// The series, as written before the first marker, tags and separators removed.
    pub series: Option<String>,
    /// The volume as written without its leading zeros: `2`, `2.5`.
    pub volume: Option<String>,
    /// The chapter number in thousandths.
    pub chapter_milli: Option<i64>,
    /// The chapter as written without its leading zeros: `10.5`.
    pub chapter: Option<String>,
}

/// File extensions the library scan reads as books.
pub const BOOK_EXTENSIONS: &[&str] = &["cbz", "cbr", "cb7", "zip", "rar", "7z", "epub", "pdf"];

/// Image extensions that make a folder a book of its own.
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "avif"];

/// The name without a book extension. Other dots stay: `ch010.5` is a chapter.
pub fn stem(file_name: &str) -> &str {
    match file_name.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty()
                && BOOK_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) =>
        {
            stem
        }
        _ => file_name,
    }
}

/// `name` with bracketed tags removed: `[Digital]`, `(2019)`, `(copy)`, `{x}`.
pub fn without_tags(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0u32;
    for c in name.chars() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A name for people to read: tags gone, underscores spaced, separators trimmed.
pub fn tidy(name: &str) -> String {
    let spaced = without_tags(name).replace('_', " ");
    spaced
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c == '-' || c == '.' || c == ',' || c.is_whitespace())
        .to_owned()
}

/// A number's text in thousandths and in its shortest written form: `010.5` is 10 500 and
/// `10.5`.
pub fn milli(text: &str) -> Option<(i64, String)> {
    let (whole, frac) = match text.split_once('.') {
        Some((w, f)) => (w, f),
        None => (text, ""),
    };
    if whole.is_empty()
        || whole.len() > 6
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || frac.len() > 3
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let whole: i64 = whole.parse().ok()?;
    let mut thousandths = 0;
    for (i, b) in frac.bytes().enumerate() {
        thousandths += i64::from(b - b'0') * [100, 10, 1][i];
    }
    let value = whole * 1000 + thousandths;
    let shown = if thousandths == 0 {
        whole.to_string()
    } else {
        let frac = format!("{thousandths:03}");
        format!("{whole}.{}", frac.trim_end_matches('0'))
    };
    Some((value, shown))
}

/// The number starting at `at` in `chars`: digits with an optional decimal part. Returns the
/// text and where it ends.
fn number_at(chars: &[char], at: usize) -> Option<(String, usize)> {
    let mut end = at;
    while end < chars.len() && chars[end].is_ascii_digit() {
        end += 1;
    }
    if end == at {
        return None;
    }
    if end + 1 < chars.len() && chars[end] == '.' && chars[end + 1].is_ascii_digit() {
        end += 1;
        while end < chars.len() && chars[end].is_ascii_digit() {
            end += 1;
        }
    }
    Some((chars[at..end].iter().collect(), end))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Marker {
    Volume,
    Chapter,
}

/// A marker word at `at`, and the number after it. `at` must start a word.
fn marker_at(chars: &[char], at: usize) -> Option<(Marker, String, usize, usize)> {
    const WORDS: &[(&str, Marker, bool)] = &[
        // (word, kind, may be followed by a space or a dot before its number)
        ("volume", Marker::Volume, true),
        ("vol", Marker::Volume, true),
        ("v", Marker::Volume, false),
        ("chapter", Marker::Chapter, true),
        ("ch", Marker::Chapter, true),
        ("c", Marker::Chapter, false),
        ("#", Marker::Chapter, true),
    ];
    let rest: String = chars[at..].iter().take(8).collect::<String>().to_lowercase();
    for (word, kind, loose) in WORDS {
        if !rest.starts_with(word) {
            continue;
        }
        let mut i = at + word.chars().count();
        if *loose {
            if i < chars.len() && chars[i] == '.' {
                i += 1;
            }
            while i < chars.len() && chars[i] == ' ' {
                i += 1;
            }
        }
        let Some((number, end)) = number_at(chars, i) else { continue };
        // The number must end the word: `ch12abc` is not a chapter. `v01c001` is two markers.
        let next = chars.get(end).copied();
        let ends = next.is_none_or(|c| !c.is_alphanumeric() || marker_follows(chars, end));
        if ends {
            return Some((*kind, number, at, end));
        }
    }
    None
}

/// Whether a marker with its number starts at `at` (`v01c001`).
fn marker_follows(chars: &[char], at: usize) -> bool {
    matches!(chars.get(at).map(|c| c.to_ascii_lowercase()), Some('c' | 'v'))
        && marker_at(chars, at).is_some()
}

/// Read a name: the file's name without its extension, or a folder's.
pub fn parse(name: &str) -> Named {
    let cleaned = without_tags(name);
    let chars: Vec<char> = cleaned.chars().collect();
    let mut named = Named::default();
    let mut first = None;
    let mut i = 0;
    while i < chars.len() {
        let word_start = i == 0
            || !chars[i - 1].is_alphanumeric() && chars[i - 1] != '_'
            || marker_follows_prior(&chars, i);
        if word_start
            && let Some((kind, number, start, end)) = marker_at(&chars, i)
            && let Some((m, shown)) = milli(&number)
        {
            match kind {
                Marker::Volume if named.volume.is_none() => named.volume = Some(shown),
                Marker::Chapter if named.chapter.is_none() => {
                    named.chapter_milli = Some(m);
                    named.chapter = Some(shown);
                }
                _ => {}
            }
            first.get_or_insert(start);
            i = end;
            continue;
        }
        i += 1;
    }
    // A last word that is only a number is a chapter: `Some Name 012`, `Some Name - 12.5`.
    if named.chapter.is_none() && named.volume.is_none() {
        let words: Vec<&str> = cleaned.split_whitespace().collect();
        if words.len() >= 2
            && let Some(last) = words.last()
            && last.chars().all(|c| c.is_ascii_digit() || c == '.')
            && let Some((m, shown)) = milli(last)
        {
            named.chapter_milli = Some(m);
            named.chapter = Some(shown);
            let at = cleaned.len() - last.len();
            first = Some(cleaned[..at].chars().count());
        }
    }
    let head: String = match first {
        Some(at) => chars[..at].iter().collect(),
        None => cleaned.clone(),
    };
    let series = tidy(&head);
    named.series = (!series.is_empty()).then_some(series);
    named
}

/// `v01c001`: a marker right after another marker's number.
fn marker_follows_prior(chars: &[char], i: usize) -> bool {
    i > 0 && chars[i - 1].is_ascii_digit() && matches!(chars[i].to_ascii_lowercase(), 'c' | 'v')
}

/// A folder name read as `Vol. 2`, `Volume 02`, `v2` or a bare `2`: the volume it holds.
pub fn volume_folder(name: &str) -> Option<String> {
    let parsed = parse(name);
    if parsed.series.is_none() && parsed.chapter.is_none() {
        return parsed.volume;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str) -> (Option<String>, Option<String>, Option<String>) {
        let n = parse(stem(name));
        (n.series, n.volume, n.chapter)
    }

    fn s(text: &str) -> Option<String> {
        Some(text.to_owned())
    }

    #[test]
    fn volumes_chapters_and_the_series_before_them() {
        assert_eq!(p("Moonlit Harbor v01.cbz"), (s("Moonlit Harbor"), s("1"), None));
        assert_eq!(
            p("Paper Kite Rangers v02 c003.cbz"),
            (s("Paper Kite Rangers"), s("2"), s("3"))
        );
        assert_eq!(p("Tin Lantern Club ch001.cbz"), (s("Tin Lantern Club"), None, s("1")));
        assert_eq!(p("Tin Lantern Club ch010.5.cbz"), (s("Tin Lantern Club"), None, s("10.5")));
        assert_eq!(p("Name Vol. 3 Ch. 12.cbz"), (s("Name"), s("3"), s("12")));
        assert_eq!(p("Name Volume 02 Chapter 7.5.cbz"), (s("Name"), s("2"), s("7.5")));
        assert_eq!(p("Name v01c001.cbz"), (s("Name"), s("1"), s("1")));
        assert_eq!(p("Name #12.cbz"), (s("Name"), None, s("12")));
        assert_eq!(p("Name - 012.cbz"), (s("Name"), None, s("12")));
    }

    #[test]
    fn decimal_chapters_keep_their_fraction_in_thousandths() {
        assert_eq!(parse("Name c10.5").chapter_milli, Some(10_500));
        assert_eq!(parse("Name c10.25").chapter_milli, Some(10_250));
        assert_eq!(parse("Name c010").chapter_milli, Some(10_000));
        assert_eq!(parse("Name c010").chapter.as_deref(), Some("10"));
    }

    #[test]
    fn tags_are_dropped_and_underscores_are_not_numbers() {
        assert_eq!(p("Name [Digital] (2019) v03.cbz"), (s("Name"), s("3"), None));
        let odd = parse(stem("scan_final_FINAL_2 (copy) [x].cbz"));
        assert_eq!((odd.volume, odd.chapter), (None, None));
        assert_eq!(odd.series.as_deref(), Some("scan final FINAL 2"));
    }

    #[test]
    fn a_name_with_nothing_to_read_is_just_a_series() {
        assert_eq!(p("Loose Single Volume.cbz"), (s("Loose Single Volume"), None, None));
        assert_eq!(
            p("Harbour Lights and Letters.epub"),
            (s("Harbour Lights and Letters"), None, None)
        );
        // Words that start like markers are words.
        assert_eq!(p("Cover Story.cbz"), (s("Cover Story"), None, None));
        assert_eq!(p("Vivid Colours.cbz"), (s("Vivid Colours"), None, None));
        assert_eq!(p("Chapterhouse.cbz"), (s("Chapterhouse"), None, None));
    }

    #[test]
    fn a_bare_title_with_a_year_or_a_number_word_is_not_a_chapter() {
        // Only a last word that is purely a number counts, and a one-word name has no series.
        assert_eq!(p("2001.cbz"), (s("2001"), None, None));
        assert_eq!(p("Name Two.cbz"), (s("Name Two"), None, None));
    }

    #[test]
    fn numbers_in_thousandths() {
        assert_eq!(milli("010.5"), Some((10_500, "10.5".into())));
        assert_eq!(milli("7"), Some((7_000, "7".into())));
        assert_eq!(milli("7.050"), Some((7_050, "7.05".into())));
        assert_eq!(milli("7.1234"), None);
        assert_eq!(milli(""), None);
        assert_eq!(milli("1e3"), None);
    }

    #[test]
    fn stems_drop_only_book_extensions() {
        assert_eq!(stem("a ch10.5.cbz"), "a ch10.5");
        assert_eq!(stem("a ch10.5"), "a ch10.5");
        assert_eq!(stem("notes.txt"), "notes.txt");
    }

    #[test]
    fn volume_folders() {
        assert_eq!(volume_folder("Vol. 2").as_deref(), Some("2"));
        assert_eq!(volume_folder("v03").as_deref(), Some("3"));
        assert_eq!(volume_folder("Extras"), None);
    }
}
