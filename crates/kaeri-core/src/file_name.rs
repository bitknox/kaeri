//! File and folder names made from ids, such as a work's id or a chapter's key.
//!
//! A name made here:
//!
//! - fits every file system the apps write to: at most [`MAX`] bytes of ASCII letters, digits,
//!   `-`, `_`, `~` and `+`, with no dot, no space and none of the names Windows reserves, so a
//!   download's path stays well inside Windows' 260 characters and any 255-byte name limit,
//!   with room for the extensions added to it;
//! - tells every two ids apart, also on a file system that ignores case: every name is
//!   lowercase, and two ids never share one;
//! - stays the same from one release and one platform to the next.
//!
//! Most ids keep the name earlier versions gave them ([`legacy`]), so their files are where
//! they were. An id whose old name was too long, could be read as another id's, held capitals
//! or is reserved on Windows gets a shortened name instead: a readable start of the id, `+` and
//! a hash of the whole id. An old name never holds `+`, so the two kinds never meet.

/// The longest name [`of_id`] makes, in bytes. A chapter's pages go in `<name>.pages/<page>`
/// inside `<work name>`, which keeps a download's whole path under 260 characters on Windows.
pub const MAX: usize = 64;

/// The name for `id`, as the module says.
pub fn of_id(id: &str) -> String {
    let old = legacy(id);
    if keeps_legacy(id, &old) { old } else { shortened(id) }
}

/// The name versions before this one gave `id`: every character other than an ASCII letter,
/// digit, `-` or `_` written as `~` and its code point in hex. It can be long, and two ids can
/// share it (`:1` and `Ρ` are both `~3a1`), so it is only for finding files made by those
/// versions.
pub fn legacy(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        if is_literal(ch) {
            out.push(ch);
        } else {
            out.push_str(&format!("~{:x}", u32::from(ch)));
        }
    }
    out
}

/// Whether Windows reserves `name` for a device, such as `CON`, `nul.txt` or `COM1`: a file
/// cannot have it there whatever its extension.
pub fn reserved_on_windows(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches([' ', '.']);
    let lower = stem.to_lowercase();
    if matches!(lower.as_str(), "con" | "prn" | "aux" | "nul" | "conin$" | "conout$") {
        return true;
    }
    let Some(rest) = lower.strip_prefix("com").or_else(|| lower.strip_prefix("lpt")) else {
        return false;
    };
    let mut chars = rest.chars();
    matches!((chars.next(), chars.next()), (Some('0'..='9' | '¹' | '²' | '³'), None))
}

/// `text` cut to at most `max` bytes at a character's edge, without the dots and spaces
/// Windows drops from a name's end, and with `_` after a name Windows reserves. For a name
/// people read, such as a book's title; it does not tell texts apart.
pub fn readable(text: &str, max: usize) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if out.len() + ch.len_utf8() > max {
            break;
        }
        out.push(ch);
    }
    let mut out = out.trim_end_matches(['.', ' ']).to_owned();
    if reserved_on_windows(&out) {
        out.push('_');
    }
    out
}

fn is_literal(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

/// Whether `id` keeps its old name `old`: short enough, all lowercase, not reserved, and read
/// back as `id` alone. Reading back makes the old names kept tell every id apart.
fn keeps_legacy(id: &str, old: &str) -> bool {
    !old.is_empty()
        && old.len() <= MAX
        && !old.bytes().any(|b| b.is_ascii_uppercase())
        && !reserved_on_windows(old)
        && read_back(old).as_deref() == Some(id)
}

/// The one id an old name is read as, or `None` for a name no id's old name could be. An
/// escape of two hex digits that is a character old names escape, such as `~3a` for `:`, is
/// read as that character; any other escape takes every hex digit after its `~`. Each escape
/// read must be how the old name writes that character, so `legacy(read_back(x)) == x`.
fn read_back(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b != b'~' {
            let ch = char::from(b);
            if !is_literal(ch) {
                return None;
            }
            out.push(ch);
            i += 1;
            continue;
        }
        let digits = bytes[i + 1..]
            .iter()
            .take_while(|d| matches!(d, b'0'..=b'9' | b'a'..=b'f'))
            .count();
        let run = &name[i + 1..i + 1 + digits];
        let ascii = run
            .get(..2)
            .and_then(|two| u32::from_str_radix(two, 16).ok())
            .and_then(char::from_u32)
            .filter(|ch| ch.is_ascii() && !is_literal(*ch) && u32::from(*ch) >= 0x10);
        let (ch, used) = match ascii {
            Some(ch) => (ch, 2),
            None => {
                if run.is_empty() || run.len() > 6 || (run.len() > 1 && run.starts_with('0')) {
                    return None;
                }
                let ch = char::from_u32(u32::from_str_radix(run, 16).ok()?)?;
                // Two digits read whole are an ASCII character, which the branch above takes
                // when old names escape it; one they never escape is not an escape.
                if is_literal(ch) || (run.len() == 2 && ch.is_ascii()) {
                    return None;
                }
                (ch, run.len())
            }
        };
        out.push(ch);
        i += 1 + used;
    }
    Some(out)
}

/// A readable start of `id`, lowercased, with each run of other characters as one `_`, then
/// `+` and a hash of the whole id.
fn shortened(id: &str) -> String {
    let hash = crate::hash::stable_hash(id);
    let room = MAX - 1 - hash.len();
    let mut start = String::new();
    for ch in id.chars() {
        let ch = ch.to_ascii_lowercase();
        let ch = if is_literal(ch) { ch } else { '_' };
        if ch == '_' && start.ends_with('_') {
            continue;
        }
        if start.len() == room {
            break;
        }
        start.push(ch);
    }
    format!("{}+{hash}", start.trim_end_matches('_'))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn assert_fits(name: &str) {
        assert!(!name.is_empty() && name.len() <= MAX, "{name:?} is {} bytes", name.len());
        assert!(
            name.bytes().all(|b| b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || matches!(b, b'-' | b'_' | b'~' | b'+')),
            "{name:?}"
        );
        assert!(!reserved_on_windows(name), "{name:?}");
    }

    #[test]
    fn plain_ids_keep_the_names_earlier_versions_gave_them() {
        for (id, name) in [
            ("a:900501", "a~3a900501"),
            ("1", "1"),
            ("1.5", "1~2e5"),
            ("12-13", "12-13"),
            ("v2/3", "v2~2f3"),
            ("x/-/extra", "x~2f-~2fextra"),
            ("x/2/side story", "x~2f2~2fside~20story"),
            ("x/-/ひと", "x~2f-~2f~3072~3068"),
        ] {
            assert_eq!(legacy(id), name);
            assert_eq!(of_id(id), name, "{id}");
        }
    }

    #[test]
    fn a_long_unnumbered_cjk_label_gets_a_short_name() {
        let label: String = std::iter::repeat_n('ひ', 52).collect();
        let id = format!("x/-/{label}");
        assert!(legacy(&id).len() > 255);
        let name = of_id(&id);
        assert_fits(&name);
        assert!(name.starts_with("x_-+"), "{name}");
        assert_eq!(name, of_id(&id), "the same every time");
        let other = format!("x/-/{label}ひ");
        assert_ne!(of_id(&other), name);
    }

    #[test]
    fn a_long_latin_label_keeps_a_readable_start() {
        let id = "x/-/the long road home and everything that came after it, part two";
        let name = of_id(id);
        assert_fits(&name);
        assert!(name.starts_with("x_-_the_long_road_home_and_ev"), "{name}");
    }

    #[test]
    fn ids_whose_old_names_collided_get_names_of_their_own() {
        for (a, b) in [
            (":1", "Ρ"),
            ("1.5", "1˥"),
            ("x/-/a:b", "x/-/aΫ"),
            ("x/-/a:c", "x/-/aά"),
            ("x/-/a:1", "x/-/aΡ"),
        ] {
            assert_eq!(legacy(a), legacy(b), "they collided before");
            assert_ne!(of_id(a), of_id(b), "{a} and {b}");
            assert_fits(&of_id(a));
            assert_fits(&of_id(b));
        }
        // The ASCII one of a pair keeps its old name; the other moves.
        assert_eq!(of_id("1.5"), "1~2e5");
        assert!(of_id("1˥").contains('+'));
    }

    #[test]
    fn ids_differing_only_in_case_differ_where_case_is_ignored() {
        let (a, b) = (of_id("src:AbC"), of_id("src:abc"));
        assert_ne!(a.to_lowercase(), b.to_lowercase());
        assert_fits(&a);
        assert_eq!(b, "src~3aabc");
    }

    #[test]
    fn names_windows_reserves_are_never_made() {
        for id in ["con", "nul", "prn", "aux", "com1", "lpt9", "COM3", "Nul"] {
            let name = of_id(id);
            assert_fits(&name);
            assert!(name.contains('+'), "{id} -> {name}");
        }
        for id in ["x/-/con.", "x/-/nul ", "trailing. ", "..", " "] {
            assert_fits(&of_id(id));
        }
        assert!(reserved_on_windows("CON.cbz"));
        assert!(reserved_on_windows("lpt¹"));
        assert!(!reserved_on_windows("console"));
        assert!(!reserved_on_windows("com10"));
        assert_eq!(of_id("console"), "console");
    }

    #[test]
    fn the_empty_id_has_a_name() {
        assert_fits(&of_id(""));
    }

    #[test]
    fn every_old_name_kept_reads_back_as_its_id() {
        let ids = [
            "1",
            "1.5",
            "~",
            "a~b",
            "x/-/a b",
            "\u{1}",
            "\u{1}a",
            "\u{7f}",
            "é",
            "éa",
            "é1",
            "€",
            "😀",
            "😀f",
            "x/-/第1話",
            "x/-/ひと",
            "-",
            "_",
            "a-_b",
            ":",
            "::",
        ];
        let mut seen: HashMap<String, &str> = HashMap::new();
        for id in ids {
            let name = of_id(id);
            assert_fits(&name);
            if !name.contains('+') {
                assert_eq!(read_back(&name).as_deref(), Some(id));
            }
            if let Some(other) = seen.insert(name.clone(), id) {
                panic!("{id:?} and {other:?} share {name:?}");
            }
        }
    }

    #[test]
    fn names_are_unique_over_many_generated_ids() {
        // Every id of up to three characters from a set made to collide the old way.
        let alphabet =
            ['1', '5', 'a', 'c', 'f', 'z', ':', '.', '/', ' ', 'Ρ', '˥', 'ά', 'A', 'ひ'];
        let mut ids = vec![String::new()];
        for _ in 0..3 {
            let longer: Vec<String> = ids
                .iter()
                .flat_map(|id| alphabet.iter().map(move |ch| format!("{id}{ch}")))
                .collect();
            ids.extend(longer);
        }
        ids.sort();
        ids.dedup();
        let mut seen: HashMap<String, String> = HashMap::new();
        for id in &ids {
            let name = of_id(id);
            assert_fits(&name);
            if let Some(other) = seen.insert(name.clone(), id.clone()) {
                panic!("{id:?} and {other:?} share {name:?}");
            }
        }
    }

    #[test]
    fn readable_names_fit_and_avoid_reserved_ones() {
        let title: String = std::iter::repeat_n('話', 100).collect();
        let name = readable(&title, 150);
        assert!(name.len() <= 150);
        assert!(title.starts_with(&name));
        assert_eq!(readable("CON", 150), "CON_");
        assert_eq!(readable("Title. ", 150), "Title");
    }
}
