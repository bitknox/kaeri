//! Soft hyphens from `hypher`'s patterns, for the web views that have no hyphenation of their
//! own (WebView2 and WebKitGTK). Only the languages the reading spec names are built in:
//! English, German, French, Dutch, Spanish, Portuguese, Italian and Russian.

/// The soft hyphen, which the paginator leaves out of positions.
pub const SHY: char = '\u{AD}';

/// Words shorter than this are left whole.
const MIN_LETTERS: usize = 5;
/// Longer runs of letters are not words worth breaking (a URL, a pasted hash).
const MAX_BYTES: usize = 64;

/// Whether this build carries the patterns.
pub const BUILT_IN: bool = cfg!(feature = "hyphenation");

/// The patterns for a language tag (`en`, `en-GB`, `pt_BR`), when built in.
#[cfg(feature = "hyphenation")]
pub(crate) fn language(tag: &str) -> Option<hypher::Lang> {
    let primary = tag.trim().split(['-', '_']).next()?.to_ascii_lowercase();
    let code: [u8; 2] = primary.as_bytes().try_into().ok()?;
    hypher::Lang::from_iso(code)
}

#[cfg(not(feature = "hyphenation"))]
pub(crate) fn language(_tag: &str) -> Option<()> {
    None
}

/// The text with soft hyphens between the syllables of each long word, or `None` when nothing
/// changed.
#[cfg(feature = "hyphenation")]
pub(crate) fn hyphenate(text: &str, lang: hypher::Lang) -> Option<String> {
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    let mut changed = false;
    let mut word_start: Option<usize> = None;
    let mut flush = |out: &mut String, word: &str| {
        let letters = word.chars().count();
        if letters < MIN_LETTERS || word.len() > MAX_BYTES || word.contains(SHY) {
            out.push_str(word);
            return;
        }
        let mut first = true;
        for syllable in hypher::hyphenate(word, lang) {
            if !first {
                out.push(SHY);
                changed = true;
            }
            out.push_str(syllable);
            first = false;
        }
    };
    for (i, c) in text.char_indices() {
        if c.is_alphabetic() || c == SHY {
            word_start.get_or_insert(i);
            continue;
        }
        if let Some(s) = word_start.take() {
            flush(&mut out, &text[s..i]);
        }
        out.push(c);
    }
    if let Some(s) = word_start {
        flush(&mut out, &text[s..]);
    }
    changed.then_some(out)
}

/// `text` with soft hyphens for the language `tag`, when this build has its patterns and a
/// word changed.
pub(crate) fn soft_hyphens(text: &str, tag: &str) -> Option<String> {
    #[cfg(feature = "hyphenation")]
    {
        language(tag).and_then(|lang| hyphenate(text, lang))
    }
    #[cfg(not(feature = "hyphenation"))]
    {
        let _ = (text, tag, language(tag), MIN_LETTERS, MAX_BYTES);
        None
    }
}

#[cfg(all(test, feature = "hyphenation"))]
mod tests {
    use super::*;

    #[test]
    fn long_words_break_and_short_ones_do_not() {
        let en = language("en-GB").unwrap();
        let out = hyphenate("The lantern keeper climbed extraordinarily slowly.", en).unwrap();
        assert!(out.contains("ex\u{AD}tra"), "{out}");
        assert!(out.starts_with("The "), "{out}");
        assert_eq!(out.replace(SHY, ""), "The lantern keeper climbed extraordinarily slowly.");
        assert_eq!(hyphenate("a cat sat", en), None);
    }

    #[test]
    fn only_the_spec_languages_are_built_in() {
        for tag in ["en", "de", "fr", "nl", "es", "pt-BR", "it", "ru"] {
            assert!(language(tag).is_some(), "{tag}");
        }
        for tag in ["ja", "pl", "x", ""] {
            assert!(language(tag).is_none(), "{tag}");
        }
        let de = language("de").unwrap();
        assert!(hyphenate("Leuchtturmwärterin", de).is_some());
    }
}
